use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::{BoundPath, NodeKind, RootedFs, verify_protected_git_source_inventory};
use crate::{GeneratorError, GeneratorErrorKind, PinnedSource, Result, VerifiedSource};

/// Whether a missing pinned cache may be acquired.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcquisitionMode {
    /// Acquire an absent cache; never replace an invalid complete cache.
    Managed,
    /// Verify an existing cache without creating directories or fetching inputs.
    ExistingOnly,
}

/// Returns an exact, clean source checkout in the owner's durable `tmp` cache.
pub fn acquire_source(
    owner: &Path,
    pin: &PinnedSource,
    mode: AcquisitionMode,
) -> Result<VerifiedSource> {
    acquire_source_with(owner, pin, mode, fetch_source)
}

fn acquire_source_with(
    owner: &Path,
    pin: &PinnedSource,
    mode: AcquisitionMode,
    fetch: impl FnOnce(&Path, &PinnedSource) -> Result<()>,
) -> Result<VerifiedSource> {
    let relative = format!(
        "tmp/surgeist-sources/{}/{}",
        pin.label(),
        pin.revision().as_str()
    );
    let cache = CacheEntry::new(owner, &relative)?;
    if cache.exists()? {
        return verify_cached_source(&cache.path, pin);
    }
    let lock = cache.prepare(mode)?;
    if cache.exists()? {
        return verify_cached_source(&cache.path, pin);
    }
    let stage = cache.reset_stage()?;
    fetch(&stage, pin)?;
    verify_cached_source(&stage, pin)?;
    cache.promote(&stage)?;
    let verified = verify_cached_source(&cache.path, pin)?;
    drop(lock);
    Ok(verified)
}

fn verify_cached_source(path: &Path, pin: &PinnedSource) -> Result<VerifiedSource> {
    let inventory = verify_source_inventory(path, pin)?;
    let verified = inventory.verified().clone();
    let snapshot = crate::core::VerifiedSourceSnapshot {
        object_format: inventory.snapshot().object_format,
        entries: vec![],
    };
    inventory
        .into_protected_source(snapshot)?
        .closing_revalidate()?;
    Ok(verified)
}

pub(super) fn verify_source_inventory(
    path: &Path,
    pin: &PinnedSource,
) -> Result<crate::core::ProtectedSourceInventory> {
    use crate::core::ProtectedTreeEntryKind;
    use std::io::Read;
    let inventory = verify_protected_git_source_inventory(path, pin)?;
    for entry in &inventory.snapshot().entries {
        if entry.kind != ProtectedTreeEntryKind::Blob
            || !matches!(entry.git_mode.as_str(), "100644" | "100755")
        {
            return Err(invalid(
                "source cache contains a link or unsupported Git object",
            ));
        }
        let relative = entry
            .path
            .to_relative_path()
            .ok_or_else(|| invalid("source cache path is not canonical UTF-8"))?;
        let binding =
            BoundPath::bind(&relative.join(inventory.verified().canonical_source_root()))?;
        let mut file = binding.held_regular_file()?;
        let identity = binding.existing_identity();
        if identity.link_count() != Some(1)
            || (identity.mode() & 0o111 != 0) != (entry.git_mode == "100755")
        {
            return Err(invalid(
                "source cache file mode or link count differs from its pin",
            ));
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).map_err(io_error)?;
        binding.revalidate()?;
        if entry.bytes.as_ref() != Some(&bytes) {
            return Err(invalid(
                "source cache bytes differ from the pinned Git blob",
            ));
        }
    }
    Ok(inventory)
}

fn fetch_source(stage: &Path, pin: &PinnedSource) -> Result<()> {
    fs::create_dir_all(stage).map_err(io_error)?;
    git(stage, &["init", "--quiet"])?;
    git(stage, &["remote", "add", "origin", pin.repository_url()])?;
    git(
        stage,
        &[
            "fetch",
            "--quiet",
            "--no-tags",
            "--depth=1",
            "origin",
            pin.revision().as_str(),
        ],
    )?;
    git(
        stage,
        &[
            "-c",
            "core.hooksPath=/dev/null",
            "checkout",
            "--quiet",
            "--detach",
            pin.revision().as_str(),
        ],
    )
}

fn git(path: &Path, arguments: &[&str]) -> Result<()> {
    let output = Command::new("/usr/bin/git")
        .current_dir(path)
        .args(arguments)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", path)
        .env("LANG", "C")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .output()
        .map_err(io_error)?;
    if !output.status.success() {
        return Err(GeneratorError::new(
            GeneratorErrorKind::Process,
            "acquire pinned source",
            String::from_utf8_lossy(&output.stderr),
        ));
    }
    Ok(())
}

/// A cache entry has an independent lock and unpublished staging sibling. Neither
/// namespace belongs to the corpus transaction or browser profile lifecycle.
struct CacheEntry {
    rooted: RootedFs,
    relative: crate::RelativePath,
    path: PathBuf,
}

impl CacheEntry {
    fn new(owner: &Path, relative: &str) -> Result<Self> {
        let location = crate::CorpusLocation::new(owner, owner)?;
        let rooted = RootedFs::open_corpus(&location)?;
        let relative = crate::RelativePath::new(relative)?;
        let path = relative.join(rooted.canonical_root());
        Ok(Self {
            rooted,
            relative,
            path,
        })
    }

    fn exists(&self) -> Result<bool> {
        self.rooted.revalidate_root()?;
        if !self.rooted.exists(self.relative.as_str())? {
            return Ok(false);
        }
        if self
            .rooted
            .identity_at(self.relative.as_str())?
            .is_none_or(|identity| identity.kind() != NodeKind::Directory)
        {
            return Err(invalid("complete cache is not a real directory"));
        }
        Ok(true)
    }

    fn prepare(&self, mode: AcquisitionMode) -> Result<File> {
        if mode == AcquisitionMode::ExistingOnly {
            return Err(GeneratorError::new(
                GeneratorErrorKind::SourceVerification,
                "open pinned cache",
                "the requested complete cache is absent; existing-only acquisition never fetches",
            ));
        }
        let parent = self
            .relative
            .as_str()
            .rsplit_once('/')
            .expect("cache parent")
            .0;
        self.rooted.ensure_dir(parent, 0o755)?;
        let lock_path = self.sibling("lock");
        let lock = if self.rooted.exists(&lock_path)? {
            self.rooted.open_file_handle(&lock_path, 0o644, true)?
        } else {
            self.rooted
                .create_file_handle_exclusive(&lock_path, b"", 0o644)?
        };
        lock.try_lock().map_err(|error| {
            GeneratorError::new(
                GeneratorErrorKind::LeaseActive,
                "lock pinned cache",
                error.to_string(),
            )
        })?;
        self.rooted.validate_handle_at(&lock_path, &lock, 0o644)?;
        self.rooted.revalidate_root()?;
        Ok(lock)
    }

    fn sibling(&self, suffix: &str) -> String {
        format!("{}.{suffix}", self.relative.as_str())
    }

    fn reset_stage(&self) -> Result<PathBuf> {
        let stage = self.sibling("partial");
        if self.rooted.exists(&stage)? {
            let identity = self
                .rooted
                .identity_at(&stage)?
                .ok_or_else(|| invalid("cache stage disappeared"))?;
            if identity.kind() != NodeKind::Directory {
                return Err(invalid("cache stage is not a real directory"));
            }
            self.rooted.erase_opaque_directory(&stage, &identity)?;
        }
        self.rooted.create_dir_exclusive(&stage, 0o755)?;
        Ok(self.rooted.canonical_root().join(stage))
    }

    fn promote(&self, stage: &Path) -> Result<()> {
        let relative_stage = self.sibling("partial");
        if self.rooted.canonical_root().join(&relative_stage) != stage {
            return Err(invalid("unexpected cache promotion source"));
        }
        self.rooted.revalidate_root()?;
        let identity = self
            .rooted
            .identity_at(&relative_stage)?
            .ok_or_else(|| invalid("cache stage disappeared before promotion"))?;
        sync_tree(stage)?;
        self.rooted
            .rename_exclusive_bound(&relative_stage, self.relative.as_str(), &identity)
    }
}

fn sync_tree(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if metadata.file_type().is_symlink() {
        return Ok(());
    }
    if metadata.is_dir() {
        for entry in fs::read_dir(path).map_err(io_error)? {
            sync_tree(&entry.map_err(io_error)?.path())?;
        }
    } else if !metadata.is_file() {
        return Err(invalid(
            "cache stage contains an unsupported filesystem object",
        ));
    }
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(io_error)
}

fn check_directory_chain(path: &Path, create: bool) -> Result<bool> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
            Ok(_) => {
                return Err(invalid(format!(
                    "cache ancestor is not a real directory: {}",
                    current.display()
                )));
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                if !create {
                    return Ok(false);
                }
                fs::create_dir(&current).map_err(io_error)?;
            }
            Err(error) => return Err(io_error(error)),
        }
    }
    Ok(true)
}

#[cfg(unix)]
fn single_link(metadata: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;
    metadata.nlink() == 1
}
#[cfg(not(unix))]
fn single_link(_metadata: &fs::Metadata) -> bool {
    false
}

fn invalid(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::SourceVerification,
        "validate pinned cache",
        detail,
    )
}
fn io_error(error: io::Error) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::Io,
        "access pinned cache",
        error.to_string(),
        error,
    )
}

#[cfg(feature = "browser-corpus")]
mod browser_cache {
    use super::super::BrowserSettings;
    use super::*;
    use crate::{RelativePath, Sha256Digest};
    use chromiumoxide::fetcher::{
        BrowserFetcher, BrowserFetcherOptions, BrowserKind, BrowserVersion,
    };
    use serde::{Deserialize, Serialize};
    use std::io::Write;

    const SIDECAR: &str = ".surgeist-browser.json";

    #[derive(Debug, Eq, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct BrowserCacheAttestation {
        schema_version: u8,
        source: String,
        version: String,
        expected_version_output: String,
        executable: RelativePath,
        sha256: Sha256Digest,
    }

    /// Locates or acquires browser bytes beneath the owner's durable `tmp` cache.
    /// The result has checked cache structure and bytes. The declared version is
    /// an acquisition pin, not an observed process result; generation separately
    /// authenticates the executable version through its supervisor.
    pub fn acquire_browser(
        owner: &Path,
        settings: &BrowserSettings,
        mode: AcquisitionMode,
    ) -> Result<RelativePath> {
        acquire_browser_with(owner, settings, mode, |stage| {
            let runtime = tokio::runtime::Runtime::new().map_err(io_error)?;
            runtime.block_on(async {
                fs::create_dir_all(stage).map_err(io_error)?;
                let version = settings
                    .version()
                    .parse::<BrowserVersion>()
                    .map_err(|error| invalid(error.to_string()))?;
                let options = BrowserFetcherOptions::builder()
                    .with_kind(BrowserKind::Chrome)
                    .with_path(stage)
                    .with_version(version)
                    .build()
                    .map_err(|error| invalid(error.to_string()))?;
                let fetched = BrowserFetcher::new(options)
                    .fetch()
                    .await
                    .map_err(|error| invalid(error.to_string()))?;
                let relative = fetched
                    .executable_path
                    .strip_prefix(&fetched.folder_path)
                    .map_err(|_| invalid("fetcher executable escapes installation"))?
                    .to_path_buf();
                for entry in fs::read_dir(&fetched.folder_path).map_err(io_error)? {
                    let entry = entry.map_err(io_error)?;
                    fs::rename(entry.path(), stage.join(entry.file_name())).map_err(io_error)?;
                }
                fs::remove_dir(&fetched.folder_path).map_err(io_error)?;
                Ok(stage.join(relative))
            })
        })
    }

    pub(super) fn acquire_browser_with(
        owner: &Path,
        settings: &BrowserSettings,
        mode: AcquisitionMode,
        fetch: impl FnOnce(&Path) -> Result<PathBuf>,
    ) -> Result<RelativePath> {
        if settings.source() != "chrome-for-testing"
            || !settings
                .version()
                .bytes()
                .all(|b| b.is_ascii_digit() || b == b'.')
            || settings.cache_root().as_str() != "tmp/surgeist-browser"
        {
            return Err(invalid(
                "managed browser requires chrome-for-testing, a numeric version, and the owner tmp/surgeist-browser cache directory",
            ));
        }
        let cache = CacheEntry::new(
            owner,
            &format!(
                "{}/mac_arm-{}",
                settings.cache_root().as_str(),
                settings.version()
            ),
        )?;
        if cache.exists()? {
            return inspect(&cache.path, settings);
        }
        let lock = cache.prepare(mode)?;
        if cache.exists()? {
            return inspect(&cache.path, settings);
        }
        let stage = cache.reset_stage()?;
        let executable = fetch(&stage)?;
        let relative = executable
            .strip_prefix(&stage)
            .map_err(|_| invalid("downloaded executable escapes cache stage"))?;
        let relative = RelativePath::new(
            relative
                .to_str()
                .ok_or_else(|| invalid("non-UTF-8 browser executable path"))?,
        )?;
        let digest = executable_digest(&stage, &relative)?;
        let attestation = BrowserCacheAttestation {
            schema_version: 1,
            source: settings.source().into(),
            version: settings.version().into(),
            expected_version_output: settings.version_output().into(),
            executable: relative,
            sha256: digest,
        };
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(stage.join(SIDECAR))
            .map_err(io_error)?;
        file.write_all(
            &serde_json::to_vec(&attestation).map_err(|error| invalid(error.to_string()))?,
        )
        .map_err(io_error)?;
        file.sync_all().map_err(io_error)?;
        inspect(&stage, settings)?;
        cache.promote(&stage)?;
        let selected = inspect(&cache.path, settings)?;
        drop(lock);
        Ok(selected)
    }

    fn inspect(root: &Path, settings: &BrowserSettings) -> Result<RelativePath> {
        let marker = root.join(SIDECAR);
        let metadata = match fs::symlink_metadata(&marker) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                // Chrome-for-Testing caches produced before acquisition attestations
                // remain immutable. The generation supervisor authenticates this
                // exact executable's version before any browser work is admitted.
                let executable = RelativePath::new(
                    "chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing",
                )?;
                executable_digest(root, &executable)?;
                return RelativePath::new(format!(
                    "{}/mac_arm-{}/{}",
                    settings.cache_root().as_str(),
                    settings.version(),
                    executable.as_str()
                ));
            }
            Err(error) => return Err(io_error(error)),
        };
        if !metadata.is_file() || metadata.file_type().is_symlink() || !single_link(&metadata) {
            return Err(invalid("invalid browser cache attestation file"));
        }
        let attestation: BrowserCacheAttestation =
            serde_json::from_slice(&fs::read(marker).map_err(io_error)?)
                .map_err(|error| invalid(error.to_string()))?;
        if attestation.schema_version != 1
            || attestation.source != settings.source()
            || attestation.version != settings.version()
            || attestation.expected_version_output != settings.version_output()
            || executable_digest(root, &attestation.executable)? != attestation.sha256
        {
            return Err(invalid("browser cache differs from its pinned attestation"));
        }
        RelativePath::new(format!(
            "{}/mac_arm-{}/{}",
            settings.cache_root().as_str(),
            settings.version(),
            attestation.executable.as_str()
        ))
    }

    fn executable_digest(root: &Path, relative: &RelativePath) -> Result<Sha256Digest> {
        let path = relative.join(root);
        check_directory_chain(path.parent().expect("executable parent"), false)?;
        let metadata = fs::symlink_metadata(&path).map_err(io_error)?;
        #[cfg(unix)]
        let executable = {
            use std::os::unix::fs::PermissionsExt;
            metadata.permissions().mode() & 0o111 != 0
        };
        #[cfg(not(unix))]
        let executable = false;
        if !metadata.is_file()
            || metadata.file_type().is_symlink()
            || !single_link(&metadata)
            || !executable
        {
            return Err(invalid(
                "browser executable must be an executable regular single-link file",
            ));
        }
        Ok(Sha256Digest::from_bytes(fs::read(path).map_err(io_error)?))
    }
}
#[cfg(feature = "browser-corpus")]
pub use browser_cache::acquire_browser;

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
#[path = "acquisition_tests.rs"]
mod tests;
