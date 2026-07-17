use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use std::fs::File;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use std::io::Read;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use std::path::Component;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Visitor};

use crate::{GeneratorError, GeneratorErrorKind, RelativePath, Result, Sha256Digest};

use super::{validate_identifier, validate_repository_url};

/// Exact lowercase hexadecimal Git object revision.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceRevision(String);

impl SourceRevision {
    pub fn new(value: impl AsRef<str>) -> Result<Self> {
        let value = value.as_ref();
        if !matches!(value.len(), 40 | 64)
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(GeneratorError::new(
                GeneratorErrorKind::SourceVerification,
                "validate source revision",
                "revision must be a full 40- or 64-character lowercase hexadecimal object ID",
            ));
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for SourceRevision {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SourceRevision {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SourceRevisionVisitor;

        impl Visitor<'_> for SourceRevisionVisitor {
            type Value = SourceRevision;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a full lowercase hexadecimal Git object ID")
            }

            fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                SourceRevision::new(value).map_err(|error| E::custom(error.serde_message()))
            }

            fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                SourceRevision::new(value).map_err(|error| E::custom(error.serde_message()))
            }
        }

        deserializer.deserialize_str(SourceRevisionVisitor)
    }
}

/// Manifest-declared identity and subdirectory of an external source checkout.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PinnedSource {
    label: String,
    repository_url: String,
    revision: SourceRevision,
    source_subdirectory: RelativePath,
}

impl PinnedSource {
    pub fn new(
        label: impl Into<String>,
        repository_url: impl Into<String>,
        revision: SourceRevision,
        source_subdirectory: RelativePath,
    ) -> Result<Self> {
        let label = label.into();
        let repository_url = repository_url.into();
        if !validate_identifier(&label) {
            return Err(invalid_source("source label is not a canonical identifier"));
        }
        if !validate_repository_url(&repository_url) {
            return Err(invalid_source(
                "source repository URL is not canonical HTTPS Git",
            ));
        }
        Ok(Self {
            label,
            repository_url,
            revision,
            source_subdirectory,
        })
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn repository_url(&self) -> &str {
        &self.repository_url
    }

    #[must_use]
    pub const fn revision(&self) -> &SourceRevision {
        &self.revision
    }

    #[must_use]
    pub const fn source_subdirectory(&self) -> &RelativePath {
        &self.source_subdirectory
    }
}

impl<'de> Deserialize<'de> for PinnedSource {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawPinnedSource {
            label: String,
            repository_url: String,
            revision: SourceRevision,
            source_subdirectory: RelativePath,
        }

        let raw = RawPinnedSource::deserialize(deserializer)?;
        Self::new(
            raw.label,
            raw.repository_url,
            raw.revision,
            raw.source_subdirectory,
        )
        .map_err(|error| serde::de::Error::custom(error.serde_message()))
    }
}

/// A source checkout proven to match a complete clean pin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSource {
    canonical_root: PathBuf,
    canonical_source_root: PathBuf,
    revision: SourceRevision,
    pub(crate) snapshot: VerifiedSourceSnapshot,
    pub(crate) protection: Vec<ProtectionEntry>,
}

impl VerifiedSource {
    #[must_use]
    pub fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }

    #[must_use]
    pub fn canonical_source_root(&self) -> &Path {
        &self.canonical_source_root
    }

    #[must_use]
    pub const fn revision(&self) -> &SourceRevision {
        &self.revision
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ObjectFormat {
    Sha1,
    Sha256,
}

impl ObjectFormat {
    fn for_revision(revision: &SourceRevision) -> Self {
        if revision.as_str().len() == 40 {
            Self::Sha1
        } else {
            Self::Sha256
        }
    }

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
        }
    }

    const fn object_id_len(self) -> usize {
        match self {
            Self::Sha1 => 40,
            Self::Sha256 => 64,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct VerifiedSourceSnapshot {
    pub(crate) object_format: ObjectFormat,
    pub(crate) entries: Vec<SnapshotEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SnapshotEntry {
    pub(crate) path: RelativePath,
    pub(crate) git_mode: String,
    pub(crate) blob_object_id: String,
    pub(crate) bytes: Vec<u8>,
    pub(crate) digest: Sha256Digest,
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct ObjectIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(not(unix))]
    length: u64,
    #[cfg(not(unix))]
    modified: Option<std::time::SystemTime>,
}

impl ObjectIdentity {
    fn from_metadata(metadata: &fs::Metadata) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;

            Self {
                device: metadata.dev(),
                inode: metadata.ino(),
            }
        }
        #[cfg(not(unix))]
        {
            Self {
                length: metadata.len(),
                modified: metadata.modified().ok(),
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProtectionEntry {
    pub(crate) path: PathBuf,
    pub(crate) identity: ObjectIdentity,
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[derive(Clone, Debug, Eq, PartialEq)]
enum InstallEntryKind {
    Directory,
    Regular,
    Symlink(PathBuf),
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
type InstallEntryKind = std::convert::Infallible;

#[derive(Clone, Debug, Eq, PartialEq)]
struct InstallEntry {
    path: PathBuf,
    identity: ObjectIdentity,
    kind: InstallEntryKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TrustedGit {
    shim_path: PathBuf,
    program_path: PathBuf,
    exec_path: PathBuf,
    developer_usr: PathBuf,
    inventory: Vec<InstallEntry>,
}

/// Verifies an existing Git checkout without fetching or mutating Git state.
pub fn verify_git_source(checkout: impl AsRef<Path>, pin: &PinnedSource) -> Result<VerifiedSource> {
    verify_git_source_impl(checkout.as_ref(), pin, None)
}

#[cfg(test)]
fn verify_git_source_with_test_hook<F>(
    checkout: impl AsRef<Path>,
    pin: &PinnedSource,
    hook: F,
) -> Result<VerifiedSource>
where
    F: FnOnce() + 'static,
{
    verify_git_source_impl(checkout.as_ref(), pin, Some(Box::new(hook)))
}

fn verify_git_source_impl(
    checkout: &Path,
    pin: &PinnedSource,
    closing_hook: Option<Box<dyn FnOnce()>>,
) -> Result<VerifiedSource> {
    let checkout = canonical_checkout_directory(checkout)?;
    let trust = TrustedGit::discover()?;
    let initial_runner = GitRunner::new(trust.clone(), checkout.clone());
    require_line(
        &initial_runner.line(&["rev-parse", "--is-inside-work-tree"])?,
        "true",
        "checkout is not a Git worktree",
    )?;
    let canonical_root = canonical_source_directory(
        Path::new(&initial_runner.line(&["rev-parse", "--show-toplevel"])?),
        "resolve Git worktree root",
    )?;
    if !checkout.starts_with(&canonical_root) {
        return Err(invalid_source(
            "checkout does not remain inside its Git worktree root",
        ));
    }
    let runner = GitRunner::new(trust, canonical_root.clone());

    let worktree_git_dir = protected_directory_from_line(
        &runner.line(&["rev-parse", "--absolute-git-dir"])?,
        "resolve per-worktree Git directory",
    )?;
    let common_git_dir = protected_directory_from_line(
        &runner.line(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?,
        "resolve common Git directory",
    )?;
    let primary_objects = protected_directory_from_line(
        &runner.line(&[
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "objects",
        ])?,
        "resolve primary Git object directory",
    )?;

    let config_before = read_config_snapshot(
        &runner,
        &canonical_root,
        &common_git_dir.path,
        &worktree_git_dir.path,
        pin.repository_url(),
    )?;
    let object_format = ObjectFormat::for_revision(pin.revision());
    require_line(
        &runner.line(&["rev-parse", "--show-object-format=storage"])?,
        object_format.name(),
        "Git object storage format does not match the full pin width",
    )?;
    require_line(
        &runner.line(&["cat-file", "-t", pin.revision().as_str()])?,
        "commit",
        "pinned Git object is not a commit",
    )?;
    require_line(
        &runner.line(&[
            "rev-parse",
            "--verify",
            &format!("{}^{{commit}}", pin.revision().as_str()),
        ])?,
        pin.revision().as_str(),
        "pinned Git object peels to a different commit",
    )?;
    require_line(
        &runner.line(&["rev-parse", "--verify", "HEAD^{commit}"])?,
        pin.revision().as_str(),
        "HEAD does not equal the exact pinned revision",
    )?;

    verify_raw_cleanliness(&runner, &canonical_root, pin.revision(), object_format)?;

    let source_candidate = pin.source_subdirectory().join(&canonical_root);
    let canonical_source_root =
        canonical_source_directory(&source_candidate, "resolve pinned source subdirectory")?;
    if !canonical_source_root.starts_with(&canonical_root) {
        return Err(invalid_checkout_path(format!(
            "source subdirectory escapes the caller checkout root: {}",
            pin.source_subdirectory().as_str()
        )));
    }

    let mut protection = vec![
        protected_directory(&canonical_root, "protect Git worktree root")?,
        protected_directory(&canonical_source_root, "protect pinned source directory")?,
        worktree_git_dir,
        common_git_dir,
        primary_objects.clone(),
    ];
    collect_alternate_protection(&primary_objects.path, &mut protection)?;
    deduplicate_protection(&mut protection);
    let snapshot = build_snapshot(&runner, pin, object_format)?;

    if let Some(hook) = closing_hook {
        hook();
    }
    revalidate_protection(&protection)?;
    let config_after = read_config_snapshot(
        &runner,
        &canonical_root,
        &config_before.common_directory,
        &config_before.worktree_directory,
        pin.repository_url(),
    )?;
    if config_after != config_before {
        return Err(invalid_source(
            "Git configuration identity or inventory changed during verification",
        ));
    }
    runner.trust.revalidate()?;

    Ok(VerifiedSource {
        canonical_root,
        canonical_source_root,
        revision: pin.revision.clone(),
        snapshot,
        protection,
    })
}

impl TrustedGit {
    fn discover() -> Result<Self> {
        discover_trusted_git()
    }

    fn revalidate(&self) -> Result<()> {
        let current = inventory_trusted_installation(
            &self.shim_path,
            &self.program_path,
            &self.exec_path,
            &self.developer_usr,
        )?;
        if current != self.inventory {
            return Err(invalid_source(
                "installed Git executable unit changed during verification",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    fn shim_path(&self) -> &Path {
        &self.shim_path
    }

    #[cfg(test)]
    fn program_path(&self) -> &Path {
        &self.program_path
    }

    #[cfg(test)]
    fn exec_path(&self) -> &Path {
        &self.exec_path
    }

    #[cfg(test)]
    fn developer_usr(&self) -> &Path {
        &self.developer_usr
    }

    #[cfg(test)]
    fn inventory_contains(&self, path: &Path) -> bool {
        self.inventory.iter().any(|entry| entry.path == path)
    }

    #[cfg(test)]
    fn inventory_paths(&self) -> Vec<&Path> {
        self.inventory
            .iter()
            .map(|entry| entry.path.as_path())
            .collect()
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn discover_trusted_git() -> Result<TrustedGit> {
    let shim_path = PathBuf::from("/usr/bin/git");
    validate_trusted_regular(&shim_path, true, "validate installed Git shim")?;
    let shim_identity = object_identity(&shim_path, "identify installed Git shim")?;
    let exec_text = run_discovery_git(&shim_path, &["--exec-path"])?;
    require_identity(&shim_path, &shim_identity, "revalidate installed Git shim")?;
    let exec_path =
        canonical_source_directory(Path::new(&exec_text), "resolve installed Git exec path")?;
    let Some(libexec) = exec_path.parent() else {
        return Err(invalid_source(
            "installed Git exec path has no libexec parent",
        ));
    };
    if exec_path.file_name() != Some(OsStr::new("git-core"))
        || libexec.file_name() != Some(OsStr::new("libexec"))
    {
        return Err(invalid_source(
            "installed Git exec path is not developer-usr/libexec/git-core",
        ));
    }
    let Some(developer_usr) = libexec.parent() else {
        return Err(invalid_source(
            "installed Git exec path has no developer usr root",
        ));
    };
    if developer_usr.file_name() != Some(OsStr::new("usr")) {
        return Err(invalid_source(
            "installed Git exec path is outside a developer usr root",
        ));
    }
    let developer_usr = developer_usr.to_path_buf();
    let program_path = developer_usr.join("bin/git");
    validate_trusted_regular(&program_path, true, "validate installed Git executable")?;
    let actual_exec = run_discovery_git(&program_path, &["--exec-path"])?;
    if Path::new(&actual_exec) != exec_path {
        return Err(invalid_source(
            "installed Git shim and executable report different exec paths",
        ));
    }
    let inventory =
        inventory_trusted_installation(&shim_path, &program_path, &exec_path, &developer_usr)?;
    Ok(TrustedGit {
        shim_path,
        program_path,
        exec_path,
        developer_usr,
        inventory,
    })
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn discover_trusted_git() -> Result<TrustedGit> {
    Err(invalid_source(
        "installed Apple Git trust is unavailable on this target",
    ))
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn inventory_trusted_installation(
    shim: &Path,
    program: &Path,
    exec_path: &Path,
    developer_usr: &Path,
) -> Result<Vec<InstallEntry>> {
    let mut entries = BTreeMap::new();
    for ancestor in developer_usr.ancestors() {
        validate_trusted_directory(ancestor, "validate developer Git ancestry")?;
        insert_install_entry(ancestor, developer_usr, &mut entries)?;
    }
    validate_trusted_regular(shim, true, "validate installed Git shim")?;
    insert_install_entry(shim, developer_usr, &mut entries)?;
    validate_trusted_regular(program, true, "validate installed Git executable")?;
    insert_install_entry(program, developer_usr, &mut entries)?;
    let program_directory = program
        .parent()
        .ok_or_else(|| invalid_source("installed Git executable has no parent directory"))?;
    validate_trusted_directory(program_directory, "validate installed Git bin directory")?;
    insert_install_entry(program_directory, developer_usr, &mut entries)?;
    let exec_parent = exec_path
        .parent()
        .ok_or_else(|| invalid_source("installed Git exec directory has no parent"))?;
    validate_trusted_directory(exec_parent, "validate installed Git libexec directory")?;
    insert_install_entry(exec_parent, developer_usr, &mut entries)?;
    inventory_install_tree(exec_path, developer_usr, &mut entries)?;
    let symlink_targets = entries
        .values()
        .filter_map(|entry| match &entry.kind {
            InstallEntryKind::Symlink(target) => Some(target.clone()),
            InstallEntryKind::Directory | InstallEntryKind::Regular => None,
        })
        .collect::<BTreeSet<_>>();
    for target in symlink_targets {
        validate_trusted_regular(&target, true, "validate installed Git symlink target")?;
        insert_install_entry(&target, developer_usr, &mut entries)?;
    }
    Ok(entries.into_values().collect())
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn inventory_trusted_installation(
    _shim: &Path,
    _program: &Path,
    _exec_path: &Path,
    _developer_usr: &Path,
) -> Result<Vec<InstallEntry>> {
    Err(invalid_source(
        "installed Apple Git inventory is unavailable on this target",
    ))
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn inventory_install_tree(
    directory: &Path,
    developer_usr: &Path,
    entries: &mut BTreeMap<PathBuf, InstallEntry>,
) -> Result<()> {
    validate_trusted_directory(directory, "validate installed Git directory")?;
    insert_install_entry(directory, developer_usr, entries)?;
    let mut children = fs::read_dir(directory)
        .map_err(|error| source_io("read installed Git inventory", directory, error))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| source_io("read installed Git inventory entry", directory, error))?;
    children.sort_by_key(fs::DirEntry::file_name);
    for child in children {
        let path = child.path();
        if path.to_str().is_none() {
            return Err(invalid_source(
                "installed Git inventory contains a non-UTF-8 path",
            ));
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| source_io("inspect installed Git inventory", &path, error))?;
        validate_trusted_metadata(&path, &metadata, false, "validate installed Git inventory")?;
        insert_install_entry(&path, developer_usr, entries)?;
        if metadata.is_dir() {
            inventory_install_tree(&path, developer_usr, entries)?;
        }
    }
    Ok(())
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn insert_install_entry(
    path: &Path,
    developer_usr: &Path,
    entries: &mut BTreeMap<PathBuf, InstallEntry>,
) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| source_io("inspect installed Git entry", path, error))?;
    let kind = if metadata.is_dir() {
        InstallEntryKind::Directory
    } else if metadata.is_file() {
        InstallEntryKind::Regular
    } else if metadata.file_type().is_symlink() {
        let target = fs::canonicalize(path)
            .map_err(|error| source_io("resolve installed Git symlink", path, error))?;
        if !target.starts_with(developer_usr) {
            return Err(invalid_source(format!(
                "installed Git symlink escapes developer usr: {}",
                path.display()
            )));
        }
        InstallEntryKind::Symlink(target)
    } else {
        return Err(invalid_source(format!(
            "installed Git inventory contains a special entry: {}",
            path.display()
        )));
    };
    entries.insert(
        path.to_path_buf(),
        InstallEntry {
            path: path.to_path_buf(),
            identity: ObjectIdentity::from_metadata(&metadata),
            kind,
        },
    );
    Ok(())
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn validate_trusted_directory(path: &Path, operation: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| source_io(operation, path, error))?;
    if !metadata.is_dir() {
        return Err(invalid_source(format!(
            "{} is not a directory",
            path.display()
        )));
    }
    validate_trusted_metadata(path, &metadata, true, operation)
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn validate_trusted_regular(path: &Path, executable: bool, operation: &str) -> Result<()> {
    let metadata = fs::symlink_metadata(path).map_err(|error| source_io(operation, path, error))?;
    if !metadata.is_file() {
        return Err(invalid_source(format!(
            "{} is not a regular file",
            path.display()
        )));
    }
    validate_trusted_metadata(path, &metadata, executable, operation)
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn validate_trusted_metadata(
    path: &Path,
    metadata: &fs::Metadata,
    require_search_or_execute: bool,
    _operation: &str,
) -> Result<()> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    let mode = metadata.permissions().mode() & 0o7777;
    if metadata.uid() != 0 || mode & 0o022 != 0 {
        return Err(invalid_source(format!(
            "installed Git entry is not root-owned and write-protected: {}",
            path.display()
        )));
    }
    if require_search_or_execute && mode & 0o111 == 0 {
        return Err(invalid_source(format!(
            "installed Git entry is not executable/searchable: {}",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn run_discovery_git(program: &Path, arguments: &[&str]) -> Result<String> {
    let output = Command::new(program)
        .current_dir("/")
        .env_clear()
        .env("LC_ALL", "C")
        .args(arguments)
        .output()
        .map_err(|error| {
            GeneratorError::with_source(
                GeneratorErrorKind::Process,
                "discover installed Git",
                program.display().to_string(),
                error,
            )
        })?;
    if !output.status.success() {
        return Err(invalid_source(format!(
            "{} --exec-path failed with {}",
            program.display(),
            output.status
        )));
    }
    strict_stdout_line(&output, "discover installed Git exec path").map(str::to_owned)
}

#[derive(Clone)]
struct GitRunner {
    trust: TrustedGit,
    checkout: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ConfigSnapshot {
    raw_inventory: Vec<u8>,
    common_directory: PathBuf,
    worktree_directory: PathBuf,
    common_file: FileState,
    worktree_file: FileState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FileState {
    Absent,
    Present {
        path: PathBuf,
        identity: ObjectIdentity,
        length: u64,
    },
}

#[derive(Debug)]
struct ConfigRecord {
    scope: String,
    origin: String,
    key: String,
    value: String,
}

fn read_config_snapshot(
    runner: &GitRunner,
    canonical_root: &Path,
    common_directory: &Path,
    worktree_directory: &Path,
    expected_origin: &str,
) -> Result<ConfigSnapshot> {
    let common_path = common_directory.join("config");
    let worktree_path = worktree_directory.join("config.worktree");
    let common_before = required_config_file(&common_path, "protect common Git config")?;
    let worktree_before = optional_config_file(&worktree_path, "protect worktree Git config")?;
    let arguments = [
        "config",
        "--null",
        "--list",
        "--show-origin",
        "--show-scope",
        "--no-includes",
    ]
    .into_iter()
    .map(OsString::from)
    .collect::<Vec<_>>();
    let output = runner.run(&arguments)?;
    let records = parse_config_inventory(&output.stdout)?;
    validate_config_records(
        &records,
        canonical_root,
        &common_before,
        &worktree_before,
        expected_origin,
    )?;
    let common_after = required_config_file(&common_path, "revalidate common Git config")?;
    let worktree_after = optional_config_file(&worktree_path, "revalidate worktree Git config")?;
    if common_after != common_before || worktree_after != worktree_before {
        return Err(invalid_source(
            "Git configuration identity changed while it was inventoried",
        ));
    }
    Ok(ConfigSnapshot {
        raw_inventory: output.stdout,
        common_directory: common_directory.to_path_buf(),
        worktree_directory: worktree_directory.to_path_buf(),
        common_file: common_before,
        worktree_file: worktree_before,
    })
}

fn parse_config_inventory(bytes: &[u8]) -> Result<Vec<ConfigRecord>> {
    let mut fields = bytes.split(|byte| *byte == 0).collect::<Vec<_>>();
    if fields.last().is_some_and(|field| field.is_empty()) {
        fields.pop();
    }
    if fields.len() % 3 != 0 {
        return Err(invalid_source(
            "sanitized Git configuration inventory is not a NUL-delimited triple sequence",
        ));
    }
    fields
        .chunks_exact(3)
        .map(|triple| {
            let scope = std::str::from_utf8(triple[0])
                .map_err(|_| invalid_source("Git configuration scope is not UTF-8"))?;
            let origin = std::str::from_utf8(triple[1])
                .map_err(|_| invalid_source("Git configuration origin is not UTF-8"))?;
            let key_value = std::str::from_utf8(triple[2])
                .map_err(|_| invalid_source("Git configuration record is not UTF-8"))?;
            let (key, value) = key_value
                .split_once('\n')
                .ok_or_else(|| invalid_source("Git configuration record has no value separator"))?;
            if key.is_empty() || key.bytes().any(|byte| byte.is_ascii_uppercase()) {
                return Err(invalid_source(
                    "Git configuration emitted a noncanonical key",
                ));
            }
            Ok(ConfigRecord {
                scope: scope.to_owned(),
                origin: origin.to_owned(),
                key: key.to_owned(),
                value: value.to_owned(),
            })
        })
        .collect()
}

fn validate_config_records(
    records: &[ConfigRecord],
    canonical_root: &Path,
    common_file: &FileState,
    worktree_file: &FileState,
    expected_origin: &str,
) -> Result<()> {
    let expected_command = BTreeMap::from([
        ("core.attributesfile", "/dev/null"),
        ("core.excludesfile", "/dev/null"),
        ("core.fsmonitor", "false"),
        ("core.untrackedcache", "false"),
        ("submodule.recurse", "false"),
    ]);
    let mut command_records = BTreeMap::new();
    let mut extension = None;
    let mut origin_urls = Vec::new();
    let mut saw_worktree_scope = false;
    for record in records {
        match record.scope.as_str() {
            "command" => {
                if record.origin != "command line:"
                    || command_records
                        .insert(record.key.as_str(), record.value.as_str())
                        .is_some()
                {
                    return Err(invalid_source(
                        "sanitized command configuration has a wrong origin or duplicate",
                    ));
                }
                continue;
            }
            "local" => require_config_origin(&record.origin, canonical_root, common_file, "local")?,
            "worktree" => {
                saw_worktree_scope = true;
                require_config_origin(&record.origin, canonical_root, worktree_file, "worktree")?;
            }
            _ => {
                return Err(invalid_source(format!(
                    "disallowed Git configuration scope: {}",
                    record.scope
                )));
            }
        }

        let key = record.key.as_str();
        if key == "extensions.worktreeconfig" {
            if record.scope != "local" || extension.is_some() {
                return Err(invalid_source(
                    "extensions.worktreeConfig has a wrong scope or duplicate",
                ));
            }
            extension = Some(match record.value.as_str() {
                "true" => true,
                "false" => false,
                _ => {
                    return Err(invalid_source(
                        "extensions.worktreeConfig is not exactly true or false",
                    ));
                }
            });
        }
        if forbidden_config_record(key, &record.value) {
            return Err(invalid_source(format!(
                "disallowed Git configuration record: {key}"
            )));
        }
        if key == "remote.origin.url" {
            origin_urls.push(record.value.as_str());
        }
    }
    if command_records != expected_command {
        return Err(invalid_source(
            "sanitized Git command configuration does not match the fixed override set",
        ));
    }
    if origin_urls.as_slice() != [expected_origin] {
        return Err(invalid_source(
            "Git configuration must contain exactly one unrewritten remote.origin.url",
        ));
    }
    match extension.unwrap_or(false) {
        false => {
            if !matches!(worktree_file, FileState::Absent) || saw_worktree_scope {
                return Err(invalid_source(
                    "config.worktree exists or contributes records while worktreeConfig is disabled",
                ));
            }
        }
        true => {
            if matches!(worktree_file, FileState::Absent) && saw_worktree_scope {
                return Err(invalid_source(
                    "worktree-scoped configuration has no protected config.worktree file",
                ));
            }
        }
    }
    Ok(())
}

fn forbidden_config_record(key: &str, value: &str) -> bool {
    let remote_exec = key.starts_with("remote.")
        && (key.ends_with(".uploadpack") || key.ends_with(".receivepack") || key.ends_with(".vcs"));
    let credential_helper =
        key == "credential.helper" || (key.starts_with("credential.") && key.ends_with(".helper"));
    key.starts_with("include.")
        || key.starts_with("includeif.")
        || key.starts_with("url.")
        || key == "core.sshcommand"
        || credential_helper
        || key.starts_with("protocol.")
        || remote_exec
        || (key.starts_with("remote.")
            && key.ends_with(".url")
            && selects_custom_remote_helper(value))
}

fn selects_custom_remote_helper(value: &str) -> bool {
    if value
        .split_once("::")
        .is_some_and(|(transport, _)| is_git_transport_name(transport))
    {
        return true;
    }
    value.split_once("://").is_some_and(|(transport, _)| {
        is_git_transport_name(transport)
            && !matches!(
                transport,
                "file" | "git" | "ssh" | "http" | "https" | "ftp" | "ftps"
            )
    })
}

fn is_git_transport_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    bytes
        .next()
        .is_some_and(|byte| byte.is_ascii_alphanumeric())
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
}

fn require_config_origin(
    origin: &str,
    canonical_root: &Path,
    expected: &FileState,
    scope: &str,
) -> Result<()> {
    let path = origin
        .strip_prefix("file:")
        .ok_or_else(|| invalid_source(format!("{scope} configuration origin is not a file")))?;
    let path = Path::new(path);
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        canonical_root.join(path)
    };
    let canonical = fs::canonicalize(&candidate)
        .map_err(|error| source_io("resolve Git configuration origin", &candidate, error))?;
    let FileState::Present {
        path,
        identity,
        length,
    } = expected
    else {
        return Err(invalid_source(format!(
            "{scope} configuration record has no admissible protected file"
        )));
    };
    if &canonical != path {
        return Err(invalid_source(format!(
            "{scope} configuration origin escapes its protected file"
        )));
    }
    let current = required_config_file(path, "revalidate Git configuration origin")?;
    if current
        != (FileState::Present {
            path: path.clone(),
            identity: identity.clone(),
            length: *length,
        })
    {
        return Err(invalid_source(format!(
            "{scope} configuration origin identity changed"
        )));
    }
    Ok(())
}

fn required_config_file(path: &Path, operation: &str) -> Result<FileState> {
    match optional_config_file(path, operation)? {
        state @ FileState::Present { .. } => Ok(state),
        FileState::Absent => Err(invalid_source(format!(
            "required Git configuration file is absent: {}",
            path.display()
        ))),
    }
}

fn optional_config_file(path: &Path, operation: &str) -> Result<FileState> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(FileState::Absent),
        Err(error) => return Err(source_io(operation, path, error)),
    };
    if !metadata.is_file() {
        return Err(invalid_source(format!(
            "Git configuration is not a no-follow regular file: {}",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        if metadata.nlink() != 1 || metadata.permissions().mode() & 0o400 == 0 {
            return Err(invalid_source(format!(
                "Git configuration is hard-linked or not owner-readable: {}",
                path.display()
            )));
        }
    }
    let canonical = fs::canonicalize(path)
        .map_err(|error| source_io("canonicalize Git configuration", path, error))?;
    if canonical != path {
        return Err(invalid_source(format!(
            "Git configuration final path is not canonical: {}",
            path.display()
        )));
    }
    let _ = read_absolute_regular_nofollow(path)?;
    Ok(FileState::Present {
        path: path.to_path_buf(),
        identity: ObjectIdentity::from_metadata(&metadata),
        length: metadata.len(),
    })
}

impl GitRunner {
    fn new(trust: TrustedGit, checkout: PathBuf) -> Self {
        Self { trust, checkout }
    }

    fn line(&self, arguments: &[&str]) -> Result<String> {
        let arguments: Vec<_> = arguments.iter().map(OsString::from).collect();
        let output = self.run(&arguments)?;
        strict_stdout_line(&output, "read sanitized Git output").map(str::to_owned)
    }

    fn run(&self, arguments: &[OsString]) -> Result<Output> {
        self.run_with(arguments, |command| command.output())
    }

    fn run_with<F>(&self, arguments: &[OsString], execute: F) -> Result<Output>
    where
        F: FnOnce(&mut Command) -> std::io::Result<Output>,
    {
        let subcommand = validate_read_only_git_arguments(arguments)?;
        self.trust.revalidate()?;
        let mut command = Command::new(&self.trust.program_path);
        command
            .current_dir("/")
            .env_clear()
            .env("GIT_EXEC_PATH", &self.trust.exec_path)
            .env("LC_ALL", "C")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("GIT_NO_REPLACE_OBJECTS", "1")
            .env("GIT_NO_LAZY_FETCH", "1")
            .env("GIT_LITERAL_PATHSPECS", "1")
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .env("GIT_CONFIG_GLOBAL", "/dev/null")
            .args([
                "--no-pager",
                "--no-optional-locks",
                "--no-replace-objects",
                "--no-lazy-fetch",
                "--literal-pathspecs",
                "-c",
                "core.fsmonitor=false",
                "-c",
                "core.untrackedCache=false",
                "-c",
                "core.attributesFile=/dev/null",
                "-c",
                "core.excludesFile=/dev/null",
                "-c",
                "submodule.recurse=false",
                "-C",
            ])
            .arg(&self.checkout)
            .args(arguments);
        let result = execute(&mut command);
        let identity_result = self.trust.revalidate();
        let output = result.map_err(|error| {
            GeneratorError::with_source(
                GeneratorErrorKind::Process,
                "run installed Git",
                format!("{} {subcommand}", self.trust.program_path.display()),
                error,
            )
        })?;
        identity_result?;
        if !output.status.success() {
            return Err(invalid_source(format!(
                "sanitized Git {subcommand} exited with {}; stdout={:?}; stderr={:?}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        if !output.stderr.is_empty() {
            return Err(invalid_source(format!(
                "sanitized Git {subcommand} returned unexpected stderr: {:?}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }
        Ok(output)
    }
}

fn validate_read_only_git_arguments(arguments: &[OsString]) -> Result<&'static str> {
    let arguments = arguments
        .iter()
        .map(|argument| {
            argument
                .to_str()
                .ok_or_else(|| invalid_source("sanitized Git argv contains a non-UTF-8 token"))
        })
        .collect::<Result<Vec<_>>>()?;
    let allowed_subcommand = match arguments.as_slice() {
        ["rev-parse", option]
            if matches!(
                *option,
                "--is-inside-work-tree"
                    | "--show-toplevel"
                    | "--absolute-git-dir"
                    | "--show-object-format=storage"
            ) =>
        {
            Some("rev-parse")
        }
        ["rev-parse", "--path-format=absolute", "--git-common-dir"]
        | [
            "rev-parse",
            "--path-format=absolute",
            "--git-path",
            "objects",
        ]
        | ["rev-parse", "--verify", "HEAD^{commit}"] => Some("rev-parse"),
        ["rev-parse", "--verify", expression] if is_full_commit_expression(expression) => {
            Some("rev-parse")
        }
        ["ls-tree", "-r", "-z", "--full-tree", revision] if is_full_object_id(revision) => {
            Some("ls-tree")
        }
        ["ls-tree", "-r", "-z", "--full-tree", revision, "--", path]
            if is_full_object_id(revision) && RelativePath::new(path).is_ok() =>
        {
            Some("ls-tree")
        }
        ["ls-files", "--stage", "-z"]
        | ["ls-files", "-v", "-z"]
        | ["ls-files", "--others", "--exclude-standard", "-z"] => Some("ls-files"),
        ["cat-file", "-t", object_id] | ["cat-file", "blob", object_id]
            if is_full_object_id(object_id) =>
        {
            Some("cat-file")
        }
        [
            "config",
            "--null",
            "--list",
            "--show-origin",
            "--show-scope",
            "--no-includes",
        ] => Some("config"),
        _ => None,
    };
    allowed_subcommand.ok_or_else(|| {
        invalid_source(format!(
            "sanitized Git runner rejected unlisted argv shape: {arguments:?}"
        ))
    })
}

fn is_full_object_id(value: &str) -> bool {
    matches!(value.len(), 40 | 64)
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_full_commit_expression(value: &str) -> bool {
    value
        .strip_suffix("^{commit}")
        .is_some_and(is_full_object_id)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GitEntry {
    mode: String,
    object_id: String,
}

fn protected_directory_from_line(line: &str, operation: &str) -> Result<ProtectionEntry> {
    let path = Path::new(line);
    if !path.is_absolute() {
        return Err(invalid_source(format!(
            "{operation} returned a non-absolute path"
        )));
    }
    protected_directory(path, operation)
}

fn protected_directory(path: &Path, operation: &str) -> Result<ProtectionEntry> {
    let canonical = canonical_source_directory(path, operation)?;
    let metadata = fs::symlink_metadata(&canonical)
        .map_err(|error| source_io(operation, &canonical, error))?;
    if !metadata.is_dir() {
        return Err(invalid_source(format!(
            "{operation} did not resolve a directory: {}",
            canonical.display()
        )));
    }
    Ok(ProtectionEntry {
        path: canonical,
        identity: ObjectIdentity::from_metadata(&metadata),
    })
}

fn collect_alternate_protection(
    primary_objects: &Path,
    protection: &mut Vec<ProtectionEntry>,
) -> Result<()> {
    fn visit(
        object_directory: &Path,
        protection: &mut Vec<ProtectionEntry>,
        active: &mut BTreeSet<ObjectIdentity>,
        complete: &mut BTreeSet<ObjectIdentity>,
    ) -> Result<()> {
        let current = protected_directory(object_directory, "protect Git object directory")?;
        if active.contains(&current.identity) {
            return Err(invalid_source(
                "recursive Git alternate object directories contain a cycle",
            ));
        }
        if complete.contains(&current.identity) {
            return Ok(());
        }
        active.insert(current.identity.clone());
        protection.push(current.clone());

        let alternates_path = current.path.join("info/alternates");
        if let Some(bytes) = read_optional_nofollow_regular(&alternates_path)? {
            let text = std::str::from_utf8(&bytes)
                .map_err(|_| invalid_source("Git alternates inventory is not UTF-8"))?;
            if text.contains('\0') || text.contains('\r') {
                return Err(invalid_source(
                    "Git alternates inventory contains an invalid character",
                ));
            }
            let mut lines = text.split('\n').collect::<Vec<_>>();
            if lines.last() == Some(&"") {
                lines.pop();
            }
            for line in lines {
                if line.is_empty() {
                    return Err(invalid_source(
                        "Git alternates inventory contains an empty entry",
                    ));
                }
                let candidate = Path::new(line);
                let candidate = if candidate.is_absolute() {
                    candidate.to_path_buf()
                } else {
                    current.path.join(candidate)
                };
                let alternate = protected_directory(
                    &candidate,
                    "resolve recursive Git alternate object directory",
                )?;
                if active.contains(&alternate.identity) {
                    return Err(invalid_source(
                        "recursive Git alternate object directories contain a cycle",
                    ));
                }
                protection.push(alternate.clone());
                visit(&alternate.path, protection, active, complete)?;
            }
        }

        active.remove(&current.identity);
        complete.insert(current.identity);
        Ok(())
    }

    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    visit(primary_objects, protection, &mut active, &mut complete)
}

fn deduplicate_protection(protection: &mut Vec<ProtectionEntry>) {
    protection.sort_by(|left, right| {
        left.path
            .cmp(&right.path)
            .then_with(|| left.identity.cmp(&right.identity))
    });
    let mut identities = BTreeSet::new();
    protection.retain(|entry| identities.insert(entry.identity.clone()));
}

pub(crate) fn revalidate_protection(protection: &[ProtectionEntry]) -> Result<()> {
    for entry in protection {
        let current = protected_directory(&entry.path, "revalidate protected source directory")?;
        if current != *entry {
            return Err(invalid_source(format!(
                "protected source directory identity changed: {}",
                entry.path.display()
            )));
        }
    }
    Ok(())
}

fn read_optional_nofollow_regular(path: &Path) -> Result<Option<Vec<u8>>> {
    let before = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(source_io("inspect Git alternates file", path, error)),
    };
    if !before.is_file() {
        return Err(invalid_source(format!(
            "Git alternates path is not a no-follow regular file: {}",
            path.display()
        )));
    }
    let identity = ObjectIdentity::from_metadata(&before);
    let bytes = read_absolute_regular_nofollow(path)?;
    let after = fs::symlink_metadata(path)
        .map_err(|error| source_io("revalidate Git alternates file", path, error))?;
    if !after.is_file()
        || ObjectIdentity::from_metadata(&after) != identity
        || after.len() != before.len()
    {
        return Err(invalid_source(
            "Git alternates file identity changed while it was read",
        ));
    }
    Ok(Some(bytes))
}

fn verify_raw_cleanliness(
    runner: &GitRunner,
    canonical_root: &Path,
    revision: &SourceRevision,
    object_format: ObjectFormat,
) -> Result<()> {
    let tree_arguments = ["ls-tree", "-r", "-z", "--full-tree", revision.as_str()]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let tree = parse_tree_inventory(
        &runner.run(&tree_arguments)?.stdout,
        object_format,
        "pinned commit tree",
    )?;

    let index_arguments = ["ls-files", "--stage", "-z"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let index = parse_index_inventory(&runner.run(&index_arguments)?.stdout, object_format)?;
    if index != tree {
        return Err(invalid_source(
            "stage-zero Git index inventory does not exactly match the pinned commit tree",
        ));
    }

    let visibility_arguments = ["ls-files", "-v", "-z"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let visibility = parse_visibility_inventory(&runner.run(&visibility_arguments)?.stdout)?;
    let expected_paths = index.keys().cloned().collect::<BTreeSet<_>>();
    if visibility != expected_paths {
        return Err(invalid_source(
            "Git index visibility contains hidden, removed, or unknown entries",
        ));
    }

    let untracked_arguments = ["ls-files", "--others", "--exclude-standard", "-z"]
        .into_iter()
        .map(OsString::from)
        .collect::<Vec<_>>();
    let untracked = runner.run(&untracked_arguments)?;
    if !nul_records(&untracked.stdout, "nonignored untracked inventory")?.is_empty() {
        return Err(invalid_source(
            "Git worktree contains a nonignored untracked path",
        ));
    }

    for (path, entry) in index {
        let blob = cat_file_blob(runner, &entry.object_id)?;
        match entry.mode.as_str() {
            "100644" | "100755" => {
                let (bytes, executable) = read_worktree_regular(canonical_root, &path)?;
                let expected_executable = entry.mode == "100755";
                if bytes != blob || executable != expected_executable {
                    return Err(invalid_source(format!(
                        "materialized tracked file differs from its index blob or mode: {}",
                        path.as_str()
                    )));
                }
            }
            "120000" => {
                let target = read_worktree_symlink(canonical_root, &path)?;
                if std::str::from_utf8(&target).is_err() || target != blob {
                    return Err(invalid_source(format!(
                        "materialized tracked symlink differs from its index blob: {}",
                        path.as_str()
                    )));
                }
            }
            _ => {
                return Err(invalid_source(format!(
                    "unsupported tracked Git mode {} for {}",
                    entry.mode,
                    path.as_str()
                )));
            }
        }
    }
    Ok(())
}

fn build_snapshot(
    runner: &GitRunner,
    pin: &PinnedSource,
    object_format: ObjectFormat,
) -> Result<VerifiedSourceSnapshot> {
    let arguments = [
        OsString::from("ls-tree"),
        OsString::from("-r"),
        OsString::from("-z"),
        OsString::from("--full-tree"),
        OsString::from(pin.revision().as_str()),
        OsString::from("--"),
        OsString::from(pin.source_subdirectory().as_str()),
    ];
    let inventory = parse_tree_inventory(
        &runner.run(&arguments)?.stdout,
        object_format,
        "pinned source snapshot",
    )?;
    let prefix = format!("{}/", pin.source_subdirectory().as_str());
    let mut entries = Vec::with_capacity(inventory.len());
    for (full_path, entry) in inventory {
        if !matches!(entry.mode.as_str(), "100644" | "100755") {
            return Err(invalid_source(format!(
                "source snapshot contains a non-regular mode at {}",
                full_path.as_str()
            )));
        }
        let relative = full_path.as_str().strip_prefix(&prefix).ok_or_else(|| {
            invalid_source("source snapshot path lacks the exact declared component prefix")
        })?;
        let path = RelativePath::new(relative)
            .map_err(|_| invalid_source("source snapshot contains a noncanonical relative path"))?;
        if Path::new(path.as_str()).extension() != Some(OsStr::new("json")) {
            return Err(invalid_source(format!(
                "source snapshot contains a non-JSON entry: {}",
                path.as_str()
            )));
        }
        let bytes = cat_file_blob(runner, &entry.object_id)?;
        entries.push(SnapshotEntry {
            path,
            git_mode: entry.mode,
            blob_object_id: entry.object_id,
            digest: Sha256Digest::from_bytes(&bytes),
            bytes,
        });
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    if entries.windows(2).any(|pair| pair[0].path == pair[1].path) {
        return Err(invalid_source(
            "source snapshot contains duplicate normalized paths",
        ));
    }
    Ok(VerifiedSourceSnapshot {
        object_format,
        entries,
    })
}

fn parse_tree_inventory(
    bytes: &[u8],
    object_format: ObjectFormat,
    label: &str,
) -> Result<BTreeMap<RelativePath, GitEntry>> {
    let mut entries = BTreeMap::new();
    for record in nul_records(bytes, label)? {
        let tab = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| invalid_source(format!("{label} record has no path separator")))?;
        let header = std::str::from_utf8(&record[..tab])
            .map_err(|_| invalid_source(format!("{label} header is not UTF-8")))?;
        let path = std::str::from_utf8(&record[tab + 1..])
            .map_err(|_| invalid_source(format!("{label} path is not UTF-8")))?;
        let mut fields = header.split(' ');
        let mode = fields
            .next()
            .ok_or_else(|| invalid_source(format!("{label} record omits its mode")))?;
        let object_type = fields
            .next()
            .ok_or_else(|| invalid_source(format!("{label} record omits its object type")))?;
        let object_id = fields
            .next()
            .ok_or_else(|| invalid_source(format!("{label} record omits its object ID")))?;
        if fields.next().is_some() || object_type != "blob" {
            return Err(invalid_source(format!(
                "{label} contains a non-blob or malformed entry"
            )));
        }
        validate_git_mode(mode, label)?;
        validate_object_id(object_id, object_format, label)?;
        let path = RelativePath::new(path)
            .map_err(|_| invalid_source(format!("{label} contains a noncanonical path")))?;
        if entries
            .insert(
                path,
                GitEntry {
                    mode: mode.to_owned(),
                    object_id: object_id.to_owned(),
                },
            )
            .is_some()
        {
            return Err(invalid_source(format!("{label} contains a duplicate path")));
        }
    }
    Ok(entries)
}

fn parse_index_inventory(
    bytes: &[u8],
    object_format: ObjectFormat,
) -> Result<BTreeMap<RelativePath, GitEntry>> {
    let mut entries = BTreeMap::new();
    for record in nul_records(bytes, "Git index inventory")? {
        let tab = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| invalid_source("Git index record has no path separator"))?;
        let header = std::str::from_utf8(&record[..tab])
            .map_err(|_| invalid_source("Git index header is not UTF-8"))?;
        let path = std::str::from_utf8(&record[tab + 1..])
            .map_err(|_| invalid_source("Git index path is not UTF-8"))?;
        let mut fields = header.split(' ');
        let mode = fields
            .next()
            .ok_or_else(|| invalid_source("Git index record omits its mode"))?;
        let object_id = fields
            .next()
            .ok_or_else(|| invalid_source("Git index record omits its object ID"))?;
        let stage = fields
            .next()
            .ok_or_else(|| invalid_source("Git index record omits its stage"))?;
        if fields.next().is_some() || stage != "0" {
            return Err(invalid_source(
                "Git index contains an unmerged or malformed entry",
            ));
        }
        validate_git_mode(mode, "Git index inventory")?;
        validate_object_id(object_id, object_format, "Git index inventory")?;
        let path = RelativePath::new(path)
            .map_err(|_| invalid_source("Git index contains a noncanonical path"))?;
        if entries
            .insert(
                path,
                GitEntry {
                    mode: mode.to_owned(),
                    object_id: object_id.to_owned(),
                },
            )
            .is_some()
        {
            return Err(invalid_source("Git index contains a duplicate path"));
        }
    }
    Ok(entries)
}

fn parse_visibility_inventory(bytes: &[u8]) -> Result<BTreeSet<RelativePath>> {
    let mut paths = BTreeSet::new();
    for record in nul_records(bytes, "Git index visibility inventory")? {
        let path = record
            .strip_prefix(b"H ")
            .ok_or_else(|| invalid_source("Git index entry has a nonordinary visibility tag"))?;
        let path = std::str::from_utf8(path)
            .map_err(|_| invalid_source("Git index visibility path is not UTF-8"))?;
        let path = RelativePath::new(path)
            .map_err(|_| invalid_source("Git index visibility contains a noncanonical path"))?;
        if !paths.insert(path) {
            return Err(invalid_source(
                "Git index visibility contains a duplicate path",
            ));
        }
    }
    Ok(paths)
}

fn validate_git_mode(mode: &str, label: &str) -> Result<()> {
    if !matches!(mode, "100644" | "100755" | "120000") {
        return Err(invalid_source(format!(
            "{label} contains unsupported Git mode {mode}"
        )));
    }
    Ok(())
}

fn validate_object_id(object_id: &str, object_format: ObjectFormat, label: &str) -> Result<()> {
    if object_id.len() != object_format.object_id_len()
        || !object_id
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid_source(format!(
            "{label} contains an object ID with the wrong format"
        )));
    }
    Ok(())
}

fn nul_records<'a>(bytes: &'a [u8], label: &str) -> Result<Vec<&'a [u8]>> {
    if bytes.is_empty() {
        return Ok(Vec::new());
    }
    if bytes.last() != Some(&0) {
        return Err(invalid_source(format!("{label} omitted its terminal NUL")));
    }
    let mut records = bytes.split(|byte| *byte == 0).collect::<Vec<_>>();
    records.pop();
    if records.iter().any(|record| record.is_empty()) {
        return Err(invalid_source(format!("{label} contains an empty record")));
    }
    Ok(records)
}

fn cat_file_blob(runner: &GitRunner, object_id: &str) -> Result<Vec<u8>> {
    let arguments = [
        OsString::from("cat-file"),
        OsString::from("blob"),
        OsString::from(object_id),
    ];
    runner.run(&arguments).map(|output| output.stdout)
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn opened_parent_beneath(root: &Path, relative: &Path) -> Result<(std::os::fd::OwnedFd, OsString)> {
    use rustix::fs::{Mode, OFlags, open, openat};

    let mut components = relative.components().collect::<Vec<_>>();
    let Some(Component::Normal(file_name)) = components.pop() else {
        return Err(invalid_source("rooted source path has no final component"));
    };
    if components
        .iter()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(invalid_source(
            "rooted source path is not strictly relative",
        ));
    }
    let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC;
    let mut directory = open(root, flags, Mode::empty()).map_err(|error| {
        invalid_source(format!(
            "open rooted source directory {}: {error}",
            root.display()
        ))
    })?;
    for component in components {
        let Component::Normal(name) = component else {
            return Err(invalid_source(
                "rooted source path has an invalid component",
            ));
        };
        directory = openat(&directory, name, flags, Mode::empty()).map_err(|error| {
            invalid_source(format!(
                "open rooted source path {}: {error}",
                relative.display()
            ))
        })?;
    }
    Ok((directory, file_name.to_os_string()))
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn read_worktree_regular(root: &Path, path: &RelativePath) -> Result<(Vec<u8>, bool)> {
    use rustix::fs::{FileType, Mode, OFlags, fstat, openat};

    let relative = Path::new(path.as_str());
    let (directory, file_name) = opened_parent_beneath(root, relative)?;
    let descriptor = openat(
        &directory,
        &file_name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        invalid_source(format!(
            "open tracked worktree file {}: {error}",
            path.as_str()
        ))
    })?;
    let stat = fstat(&descriptor).map_err(|error| {
        invalid_source(format!(
            "inspect tracked worktree file {}: {error}",
            path.as_str()
        ))
    })?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err(invalid_source(format!(
            "tracked worktree path is not a regular file: {}",
            path.as_str()
        )));
    }
    let mut file = File::from(descriptor);
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| source_io("read tracked worktree file", &root.join(relative), error))?;
    Ok((bytes, stat.st_mode & 0o111 != 0))
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn read_worktree_symlink(root: &Path, path: &RelativePath) -> Result<Vec<u8>> {
    use rustix::fs::{AtFlags, FileType, readlinkat, statat};

    let relative = Path::new(path.as_str());
    let (directory, file_name) = opened_parent_beneath(root, relative)?;
    let stat = statat(&directory, &file_name, AtFlags::SYMLINK_NOFOLLOW).map_err(|error| {
        invalid_source(format!(
            "inspect tracked worktree symlink {}: {error}",
            path.as_str()
        ))
    })?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::Symlink {
        return Err(invalid_source(format!(
            "tracked worktree path is not a symlink: {}",
            path.as_str()
        )));
    }
    readlinkat(&directory, &file_name, Vec::new())
        .map(std::ffi::CString::into_bytes)
        .map_err(|error| {
            invalid_source(format!(
                "read tracked worktree symlink {}: {error}",
                path.as_str()
            ))
        })
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn read_absolute_regular_nofollow(path: &Path) -> Result<Vec<u8>> {
    use rustix::fs::{FileType, Mode, OFlags, fstat, openat};

    let relative = path
        .strip_prefix(Path::new("/"))
        .map_err(|_| invalid_source("no-follow file path is not absolute"))?;
    let (directory, file_name) = opened_parent_beneath(Path::new("/"), relative)?;
    let descriptor = openat(
        &directory,
        &file_name,
        OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
        Mode::empty(),
    )
    .map_err(|error| {
        invalid_source(format!(
            "open no-follow regular file {}: {error}",
            path.display()
        ))
    })?;
    let stat = fstat(&descriptor).map_err(|error| {
        invalid_source(format!(
            "inspect no-follow regular file {}: {error}",
            path.display()
        ))
    })?;
    if FileType::from_raw_mode(stat.st_mode) != FileType::RegularFile {
        return Err(invalid_source(format!(
            "path is not a no-follow regular file: {}",
            path.display()
        )));
    }
    let mut file = File::from(descriptor);
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)
        .map_err(|error| source_io("read no-follow regular file", path, error))?;
    Ok(bytes)
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn read_worktree_regular(root: &Path, path: &RelativePath) -> Result<(Vec<u8>, bool)> {
    let candidate = root.join(path.as_str());
    let metadata = fs::symlink_metadata(&candidate)
        .map_err(|error| source_io("inspect tracked worktree file", &candidate, error))?;
    if !metadata.is_file() {
        return Err(invalid_source(
            "tracked worktree path is not a regular file",
        ));
    }
    #[cfg(unix)]
    let executable = {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    };
    #[cfg(not(unix))]
    let executable = false;
    fs::read(&candidate)
        .map(|bytes| (bytes, executable))
        .map_err(|error| source_io("read tracked worktree file", &candidate, error))
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn read_worktree_symlink(root: &Path, path: &RelativePath) -> Result<Vec<u8>> {
    let candidate = root.join(path.as_str());
    let metadata = fs::symlink_metadata(&candidate)
        .map_err(|error| source_io("inspect tracked worktree symlink", &candidate, error))?;
    if !metadata.file_type().is_symlink() {
        return Err(invalid_source("tracked worktree path is not a symlink"));
    }
    fs::read_link(&candidate)
        .map(|target| target.to_string_lossy().as_bytes().to_vec())
        .map_err(|error| source_io("read tracked worktree symlink", &candidate, error))
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn read_absolute_regular_nofollow(path: &Path) -> Result<Vec<u8>> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| source_io("inspect no-follow regular file", path, error))?;
    if !metadata.is_file() {
        return Err(invalid_source("path is not a no-follow regular file"));
    }
    fs::read(path).map_err(|error| source_io("read no-follow regular file", path, error))
}

fn strict_stdout_line<'a>(output: &'a Output, operation: &str) -> Result<&'a str> {
    let value = std::str::from_utf8(&output.stdout)
        .map_err(|_| invalid_source(format!("{operation} returned non-UTF-8 output")))?;
    let value = value
        .strip_suffix('\n')
        .ok_or_else(|| invalid_source(format!("{operation} omitted its terminal newline")))?;
    if value
        .chars()
        .any(|character| matches!(character, '\n' | '\r' | '\0'))
    {
        return Err(invalid_source(format!(
            "{operation} returned multiple or invalid lines"
        )));
    }
    Ok(value)
}

fn require_line(actual: &str, expected: &str, detail: impl Into<String>) -> Result<()> {
    if actual != expected {
        return Err(invalid_source(detail));
    }
    Ok(())
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn object_identity(path: &Path, operation: &str) -> Result<ObjectIdentity> {
    fs::symlink_metadata(path)
        .map(|metadata| ObjectIdentity::from_metadata(&metadata))
        .map_err(|error| source_io(operation, path, error))
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn require_identity(path: &Path, expected: &ObjectIdentity, operation: &str) -> Result<()> {
    let actual = object_identity(path, operation)?;
    if &actual != expected {
        return Err(invalid_source(format!(
            "identity changed for {}",
            path.display()
        )));
    }
    Ok(())
}

fn canonical_source_directory(path: &Path, operation: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|error| source_io(operation, path, error))?;
    if canonical.to_str().is_none() || !canonical.is_dir() {
        return Err(invalid_source(format!(
            "{operation} did not resolve an existing UTF-8 directory: {}",
            path.display()
        )));
    }
    Ok(canonical)
}

fn canonical_checkout_directory(path: &Path) -> Result<PathBuf> {
    if path.to_str().is_none() {
        return Err(invalid_checkout_path("caller checkout root is not UTF-8"));
    }
    let canonical = fs::canonicalize(path).map_err(|error| {
        GeneratorError::with_source(
            GeneratorErrorKind::InvalidPath,
            "resolve caller checkout root",
            path.display().to_string(),
            error,
        )
    })?;
    if canonical.to_str().is_none() || !canonical.is_dir() {
        return Err(invalid_checkout_path(format!(
            "caller checkout root is not an existing UTF-8 directory: {}",
            path.display()
        )));
    }
    Ok(canonical)
}

fn source_io(operation: &str, path: &Path, error: std::io::Error) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::SourceVerification,
        operation,
        path.display().to_string(),
        error,
    )
}

fn invalid_source(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::SourceVerification,
        "verify pinned source",
        detail,
    )
}

fn invalid_checkout_path(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidPath,
        "validate caller checkout root",
        detail,
    )
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{
        GitRunner, PinnedSource, SourceRevision, TrustedGit, validate_read_only_git_arguments,
        verify_git_source,
    };
    use crate::{GeneratorErrorKind, RelativePath, Sha256Digest};

    struct TestDirectory(PathBuf);

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos();
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "surgeist-generator-{label}-{}-{nonce}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove test directory");
        }
    }

    fn run_git(directory: &Path, arguments: &[&OsStr]) -> String {
        let output = Command::new("/usr/bin/git")
            .arg("-C")
            .arg(directory)
            .args(arguments)
            .output()
            .expect("run installed git");
        assert!(
            output.status.success(),
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("UTF-8 Git output")
            .trim_end()
            .to_owned()
    }

    fn run_git_program(arguments: &[&OsStr]) -> String {
        let output = Command::new("/usr/bin/git")
            .args(arguments)
            .output()
            .expect("run installed git");
        assert!(
            output.status.success(),
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("UTF-8 Git output")
            .trim_end()
            .to_owned()
    }

    fn repository() -> (TestDirectory, String, SourceRevision) {
        let directory = TestDirectory::new("git-source");
        run_git(
            directory.path(),
            &[OsStr::new("init"), OsStr::new("--quiet")],
        );
        run_git(
            directory.path(),
            &[
                OsStr::new("config"),
                OsStr::new("user.name"),
                OsStr::new("Test"),
            ],
        );
        run_git(
            directory.path(),
            &[
                OsStr::new("config"),
                OsStr::new("user.email"),
                OsStr::new("test@example.invalid"),
            ],
        );
        let origin = "https://example.invalid/source.git".to_owned();
        run_git(
            directory.path(),
            &[
                OsStr::new("remote"),
                OsStr::new("add"),
                OsStr::new("origin"),
                OsStr::new(&origin),
            ],
        );
        fs::create_dir(directory.path().join("fixtures")).expect("create fixture directory");
        fs::write(directory.path().join("fixtures/case.json"), b"{}\n").expect("write fixture");
        run_git(
            directory.path(),
            &[OsStr::new("add"), OsStr::new("fixtures/case.json")],
        );
        run_git(
            directory.path(),
            &[
                OsStr::new("commit"),
                OsStr::new("--quiet"),
                OsStr::new("-m"),
                OsStr::new("fixture"),
            ],
        );
        let revision = SourceRevision::new(run_git(
            directory.path(),
            &[OsStr::new("rev-parse"), OsStr::new("HEAD")],
        ))
        .expect("full revision");
        (directory, origin, revision)
    }

    fn pin(origin: &str, revision: SourceRevision, source: &str) -> PinnedSource {
        PinnedSource::new(
            "fixtures",
            origin,
            revision,
            RelativePath::new(source).expect("strict path"),
        )
        .expect("valid pin")
    }

    fn different_full_revision(revision: &SourceRevision) -> SourceRevision {
        let mut different = revision.as_str().to_owned();
        let replacement = if different.ends_with('0') { "1" } else { "0" };
        different.replace_range(different.len() - 1.., replacement);
        SourceRevision::new(different).expect("guaranteed-different full revision")
    }

    fn os_arguments(arguments: &[&str]) -> Vec<OsString> {
        arguments.iter().map(OsString::from).collect()
    }

    #[test]
    fn wrong_revision_fixture_is_distinct_when_revision_ends_in_zero() {
        let revision = SourceRevision::new(format!("{}0", "a".repeat(39)))
            .expect("synthetic full revision ending in zero");
        assert_ne!(different_full_revision(&revision), revision);
    }

    #[test]
    fn sanitized_runner_rejects_unlisted_mutating_and_helper_capable_argv_before_spawn() {
        const SHA1: &str = "0123456789abcdef0123456789abcdef01234567";
        let (directory, _origin, _revision) = repository();
        let runner = GitRunner::new(
            TrustedGit::discover().expect("supported installed Apple Git"),
            fs::canonicalize(directory.path()).expect("canonical checkout"),
        );
        let config_path = directory.path().join(".git/config");
        let config_before = fs::read(&config_path).expect("read config before rejection probes");
        let spawn_attempts = Cell::new(0_u32);
        let spawn_marker = directory.path().join("invalid-command-spawned");
        let rejected: &[&[&str]] = &[
            &["config", "core.fsmonitor", "true"],
            &["config", "--local", "credential.helper", "sentinel"],
            &["cat-file", "--filters", SHA1],
            &["cat-file", "--textconv", SHA1],
            &["rev-parse", "--exec-path"],
            &["ls-files", "--debug"],
        ];
        for arguments in rejected {
            let error = runner
                .run_with(&os_arguments(arguments), |_| {
                    spawn_attempts.set(spawn_attempts.get() + 1);
                    fs::write(&spawn_marker, b"spawned\n")?;
                    Err(std::io::Error::other("invalid command reached spawn"))
                })
                .expect_err("unlisted Git argv must fail before spawn");
            assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
        }
        assert_eq!(spawn_attempts.get(), 0, "invalid Git argv reached spawn");
        assert!(!spawn_marker.exists(), "invalid Git argv ran a helper path");
        assert_eq!(
            fs::read(config_path).expect("read config after rejection probes"),
            config_before,
            "invalid Git argv mutated repository configuration"
        );
    }

    #[test]
    fn sanitized_runner_binds_every_dynamic_revision_object_and_path_token() {
        const SHA1: &str = "0123456789abcdef0123456789abcdef01234567";
        const SHA1_COMMIT: &str = "0123456789abcdef0123456789abcdef01234567^{commit}";
        const SHA256: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        const SHA256_COMMIT: &str =
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef^{commit}";

        let allowed: &[&[&str]] = &[
            &["rev-parse", "--is-inside-work-tree"],
            &["rev-parse", "--show-toplevel"],
            &["rev-parse", "--absolute-git-dir"],
            &["rev-parse", "--path-format=absolute", "--git-common-dir"],
            &[
                "rev-parse",
                "--path-format=absolute",
                "--git-path",
                "objects",
            ],
            &["rev-parse", "--show-object-format=storage"],
            &["rev-parse", "--verify", "HEAD^{commit}"],
            &["rev-parse", "--verify", SHA1_COMMIT],
            &["rev-parse", "--verify", SHA256_COMMIT],
            &["ls-tree", "-r", "-z", "--full-tree", SHA1],
            &["ls-tree", "-r", "-z", "--full-tree", SHA256],
            &[
                "ls-tree",
                "-r",
                "-z",
                "--full-tree",
                SHA1,
                "--",
                "fixtures/[literal]",
            ],
            &["ls-files", "--stage", "-z"],
            &["ls-files", "-v", "-z"],
            &["ls-files", "--others", "--exclude-standard", "-z"],
            &["cat-file", "-t", SHA1],
            &["cat-file", "-t", SHA256],
            &["cat-file", "blob", SHA1],
            &["cat-file", "blob", SHA256],
            &[
                "config",
                "--null",
                "--list",
                "--show-origin",
                "--show-scope",
                "--no-includes",
            ],
        ];
        for arguments in allowed {
            assert!(
                validate_read_only_git_arguments(&os_arguments(arguments)).is_ok(),
                "rejected required Git argv {arguments:?}"
            );
        }

        let rejected: &[&[&str]] = &[
            &["rev-parse", "--verify", "0123456789ab^{commit}"],
            &["rev-parse", "--verify", "HEAD~1^{commit}"],
            &["rev-parse", "--verify", "HEAD^{tree}"],
            &["cat-file", "-t", "0123456789ABCDEF0123456789ABCDEF01234567"],
            &[
                "cat-file",
                "blob",
                "0123456789abcdef0123456789abcdef0123456g",
            ],
            &[
                "ls-tree",
                "-r",
                "-z",
                "--full-tree",
                SHA1,
                "--",
                "../fixtures",
            ],
            &[
                "ls-tree",
                "-r",
                "-z",
                "--full-tree",
                SHA1,
                "--",
                "fixtures\\ast",
            ],
            &[
                "ls-tree",
                "-r",
                "-z",
                "--full-tree",
                SHA1,
                "--",
                "fixtures",
                "other",
            ],
        ];
        for arguments in rejected {
            assert!(
                validate_read_only_git_arguments(&os_arguments(arguments)).is_err(),
                "accepted unbound Git argv {arguments:?}"
            );
        }
    }

    #[test]
    fn raw_cleanliness_never_executes_repository_filters_or_textconv() {
        use std::os::unix::fs::PermissionsExt;

        let (directory, origin, _revision) = repository();
        fs::write(
            directory.path().join(".gitattributes"),
            b"*.json filter=surgeist-sentinel diff=surgeist-sentinel\n",
        )
        .expect("write attributes");
        run_git(
            directory.path(),
            &[OsStr::new("add"), OsStr::new(".gitattributes")],
        );
        run_git(
            directory.path(),
            &[
                OsStr::new("commit"),
                OsStr::new("--quiet"),
                OsStr::new("-m"),
                OsStr::new("attributes"),
            ],
        );
        let revision = SourceRevision::new(run_git(
            directory.path(),
            &[OsStr::new("rev-parse"), OsStr::new("HEAD")],
        ))
        .expect("attributes revision");

        let marker = directory.path().join("sentinel-ran");
        let sentinel = directory.path().join("sentinel.sh");
        fs::write(
            directory.path().join(".git/info/exclude"),
            b"sentinel-ran\nsentinel.sh\n",
        )
        .expect("exclude test sentinels");
        fs::write(
            &sentinel,
            format!("#!/bin/sh\ntouch '{}'\ncat\n", marker.display()),
        )
        .expect("write sentinel");
        fs::set_permissions(&sentinel, fs::Permissions::from_mode(0o755))
            .expect("make sentinel executable");
        run_git(
            directory.path(),
            &[
                OsStr::new("config"),
                OsStr::new("filter.surgeist-sentinel.clean"),
                sentinel.as_os_str(),
            ],
        );
        run_git(
            directory.path(),
            &[
                OsStr::new("config"),
                OsStr::new("diff.surgeist-sentinel.textconv"),
                sentinel.as_os_str(),
            ],
        );

        verify_git_source(directory.path(), &pin(&origin, revision, "fixtures"))
            .expect("raw verification accepts byte-identical checkout");
        assert!(
            !marker.exists(),
            "verification executed a configured filter/textconv"
        );
    }

    #[test]
    fn ordinary_config_worktree_is_rejected_when_extension_is_not_enabled() {
        let (directory, origin, revision) = repository();
        let config_worktree = directory.path().join(".git/config.worktree");
        fs::write(&config_worktree, b"[surgeist]\n\ttest = true\n")
            .expect("write disabled config.worktree");

        let error = verify_git_source(directory.path(), &pin(&origin, revision, "fixtures"))
            .expect_err("disabled config.worktree must fail closed");
        assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
    }

    #[test]
    fn local_config_rejects_unknown_transport_remote_helper_before_sentinel_runs() {
        let (directory, origin, revision) = repository();
        let marker = directory.path().join("remote-helper-ran");
        let helper_url = format!("sentinel://{}", marker.display());
        run_git(
            directory.path(),
            &[
                OsStr::new("config"),
                OsStr::new("remote.sentinel.url"),
                OsStr::new(&helper_url),
            ],
        );

        let result = verify_git_source(directory.path(), &pin(&origin, revision, "fixtures"));
        assert!(!marker.exists(), "remote helper sentinel was executed");
        let error = result.expect_err("unknown transport remote helper must fail closed");
        assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
    }

    #[test]
    fn ordinary_remote_url_forms_remain_inert_and_accepted() {
        let (directory, origin, revision) = repository();
        for (name, url) in [
            ("https", "https://example.invalid/other.git"),
            ("http", "http://example.invalid/other.git"),
            ("ssh", "ssh://git@example.invalid/other.git"),
            ("git", "git://example.invalid/other.git"),
            ("ftp", "ftp://example.invalid/other.git"),
            ("ftps", "ftps://example.invalid/other.git"),
            ("file", "file:///tmp/other.git"),
            ("scp", "git@example.invalid:/other.git"),
            ("local", "/tmp/other.git"),
            ("colon", "./relative::local.git"),
        ] {
            let key = format!("remote.{name}.url");
            run_git(
                directory.path(),
                &[OsStr::new("config"), OsStr::new(&key), OsStr::new(url)],
            );
        }

        verify_git_source(directory.path(), &pin(&origin, revision, "fixtures"))
            .expect("ordinary Git remote URL forms remain inert");
    }

    #[test]
    fn custom_remote_helper_selector_uses_git_transport_grammar() {
        for value in [
            "sentinel://payload",
            "sentinel+v1://payload",
            "sentinel-v1://payload",
            "sentinel.v1://payload",
            "1sentinel://payload",
            "Sentinel://payload",
            "sentinel::payload",
            "https:://payload",
        ] {
            assert!(
                super::selects_custom_remote_helper(value),
                "missed custom remote-helper selector {value:?}"
            );
        }
        for value in [
            "file:///tmp/repository.git",
            "git://example.invalid/repository.git",
            "ssh://git@example.invalid/repository.git",
            "http://example.invalid/repository.git",
            "https://example.invalid/repository.git",
            "ftp://example.invalid/repository.git",
            "ftps://example.invalid/repository.git",
            "git@example.invalid:/repository.git",
            "/tmp/repository::copy.git",
            "./relative::repository.git",
            "https://example.invalid/repository::copy.git",
            "sentinel_name://payload",
        ] {
            assert!(
                !super::selects_custom_remote_helper(value),
                "rejected ordinary or non-helper Git URL form {value:?}"
            );
        }
    }

    #[test]
    fn linked_worktree_config_rejects_unknown_transport_remote_helper_before_sentinel_runs() {
        let (directory, origin, revision) = repository();
        run_git(
            directory.path(),
            &[
                OsStr::new("config"),
                OsStr::new("extensions.worktreeConfig"),
                OsStr::new("true"),
            ],
        );
        let linked_parent = TestDirectory::new("linked-helper");
        let linked = linked_parent.path().join("linked");
        run_git(
            directory.path(),
            &[
                OsStr::new("worktree"),
                OsStr::new("add"),
                OsStr::new("--quiet"),
                OsStr::new("--detach"),
                linked.as_os_str(),
                OsStr::new("HEAD"),
            ],
        );
        let marker = linked_parent.path().join("remote-helper-ran");
        let helper_url = format!("sentinel://{}", marker.display());
        run_git(
            &linked,
            &[
                OsStr::new("config"),
                OsStr::new("--worktree"),
                OsStr::new("remote.sentinel.url"),
                OsStr::new(&helper_url),
            ],
        );

        let result = verify_git_source(&linked, &pin(&origin, revision, "fixtures"));
        assert!(!marker.exists(), "remote helper sentinel was executed");
        let error =
            result.expect_err("worktree-scoped unknown transport remote helper must fail closed");
        assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
    }

    #[cfg(unix)]
    #[test]
    fn caller_checkout_path_failures_are_invalid_paths() {
        use std::os::unix::ffi::OsStringExt;

        let parent = TestDirectory::new("invalid-checkout-root");
        let revision = SourceRevision::new("0".repeat(40)).unwrap();
        let source = pin("https://example.invalid/source.git", revision, "fixtures");

        let missing = parent.path().join("missing");
        let error = verify_git_source(&missing, &source).expect_err("missing caller root");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);

        let regular = parent.path().join("regular-file");
        fs::write(&regular, b"not a directory\n").unwrap();
        let error = verify_git_source(&regular, &source).expect_err("non-directory caller root");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);

        let non_utf8 = parent.path().join(OsString::from_vec(vec![0xff]));
        let error = verify_git_source(&non_utf8, &source).expect_err("non-UTF-8 caller root");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);
    }

    #[test]
    fn raw_cleanliness_rejects_skip_worktree_and_assume_unchanged_flags() {
        for flag in ["--skip-worktree", "--assume-unchanged"] {
            let (directory, origin, revision) = repository();
            run_git(
                directory.path(),
                &[
                    OsStr::new("update-index"),
                    OsStr::new(flag),
                    OsStr::new("fixtures/case.json"),
                ],
            );
            let error = verify_git_source(directory.path(), &pin(&origin, revision, "fixtures"))
                .expect_err("hidden index visibility must fail closed");
            assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
        }
    }

    #[test]
    fn installed_apple_git_inventory_is_absolute_and_developer_usr_confined() {
        let trusted = TrustedGit::discover().expect("supported installed Apple Git");
        assert_eq!(trusted.shim_path(), Path::new("/usr/bin/git"));
        assert!(trusted.program_path().is_absolute());
        assert!(trusted.exec_path().is_absolute());
        assert!(trusted.program_path().starts_with(trusted.developer_usr()));
        assert!(trusted.exec_path().starts_with(trusted.developer_usr()));
        assert!(trusted.inventory_contains(trusted.program_path()));
        assert!(
            trusted
                .inventory_paths()
                .iter()
                .filter(|path| path.starts_with(trusted.exec_path()))
                .all(|path| {
                    fs::symlink_metadata(path)
                        .map(|metadata| !metadata.file_type().is_symlink())
                        .unwrap_or(false)
                        || fs::canonicalize(path)
                            .map(|target| target.starts_with(trusted.developer_usr()))
                            .unwrap_or(false)
                })
        );
    }

    #[test]
    fn linked_worktree_config_union_accepts_benign_records_and_rejects_includes() {
        let (directory, origin, revision) = repository();
        run_git(
            directory.path(),
            &[
                OsStr::new("config"),
                OsStr::new("extensions.worktreeConfig"),
                OsStr::new("true"),
            ],
        );
        let linked_parent = TestDirectory::new("linked-parent");
        let linked = linked_parent.path().join("linked");
        run_git(
            directory.path(),
            &[
                OsStr::new("worktree"),
                OsStr::new("add"),
                OsStr::new("--quiet"),
                OsStr::new("--detach"),
                linked.as_os_str(),
                OsStr::new("HEAD"),
            ],
        );
        run_git(
            &linked,
            &[
                OsStr::new("config"),
                OsStr::new("--worktree"),
                OsStr::new("surgeist.benign"),
                OsStr::new("true"),
            ],
        );
        verify_git_source(&linked, &pin(&origin, revision.clone(), "fixtures"))
            .expect("benign linked-worktree configuration");

        let included = linked_parent.path().join("included.config");
        fs::write(&included, b"[surgeist]\n\tincluded = true\n").unwrap();
        run_git(
            &linked,
            &[
                OsStr::new("config"),
                OsStr::new("--worktree"),
                OsStr::new("include.path"),
                included.as_os_str(),
            ],
        );
        let error = verify_git_source(&linked, &pin(&origin, revision, "fixtures"))
            .expect_err("worktree include must fail closed");
        assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
    }

    #[test]
    fn config_identity_and_inventory_changes_fail_the_closing_recheck() {
        let (directory, origin, revision) = repository();
        let config = directory.path().join(".git/config");
        let error = super::verify_git_source_with_test_hook(
            directory.path(),
            &pin(&origin, revision, "fixtures"),
            move || {
                let mut bytes = fs::read(&config).expect("read config");
                bytes.extend_from_slice(b"\n[surgeist]\n\tchanged = true\n");
                fs::write(&config, bytes).expect("change config before closing recheck");
            },
        )
        .expect_err("closing configuration change must fail");
        assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
    }

    #[cfg(unix)]
    #[test]
    fn source_root_replacement_and_symlink_escape_fail_the_closing_recheck() {
        use std::os::unix::fs::symlink;

        let (directory, origin, revision) = repository();
        let outside = TestDirectory::new("source-root-replacement");
        fs::write(outside.path().join("case.json"), b"replacement\n").unwrap();
        let source_root = directory.path().join("fixtures");
        let displaced = directory.path().join("fixtures-displaced");
        let outside_path = outside.path().to_path_buf();
        let error = super::verify_git_source_with_test_hook(
            directory.path(),
            &pin(&origin, revision, "fixtures"),
            move || {
                fs::rename(&source_root, &displaced).expect("displace pinned source root");
                symlink(&outside_path, &source_root).expect("replace source root with escape");
            },
        )
        .expect_err("closing source-root replacement must fail");
        assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
    }

    #[test]
    fn commit_tree_snapshot_is_literal_recursive_replacement_free_and_immutable() {
        let (directory, origin, _initial_revision) = repository();
        fs::create_dir_all(directory.path().join("fixtures/nested")).unwrap();
        fs::write(
            directory.path().join("fixtures/nested/[literal].json"),
            b"{\"nested\":true}\n",
        )
        .unwrap();
        run_git(
            directory.path(),
            &[
                OsStr::new("add"),
                OsStr::new("fixtures/nested/[literal].json"),
            ],
        );
        run_git(
            directory.path(),
            &[
                OsStr::new("commit"),
                OsStr::new("--quiet"),
                OsStr::new("-m"),
                OsStr::new("nested fixture"),
            ],
        );
        let original_revision = SourceRevision::new(run_git(
            directory.path(),
            &[OsStr::new("rev-parse"), OsStr::new("HEAD")],
        ))
        .unwrap();

        fs::write(
            directory.path().join("fixtures/case.json"),
            b"replacement\n",
        )
        .unwrap();
        run_git(
            directory.path(),
            &[OsStr::new("add"), OsStr::new("fixtures/case.json")],
        );
        run_git(
            directory.path(),
            &[
                OsStr::new("commit"),
                OsStr::new("--quiet"),
                OsStr::new("-m"),
                OsStr::new("replacement commit"),
            ],
        );
        let replacement = run_git(
            directory.path(),
            &[OsStr::new("rev-parse"), OsStr::new("HEAD")],
        );
        run_git(
            directory.path(),
            &[
                OsStr::new("reset"),
                OsStr::new("--hard"),
                OsStr::new(original_revision.as_str()),
            ],
        );
        run_git(
            directory.path(),
            &[
                OsStr::new("replace"),
                OsStr::new(original_revision.as_str()),
                OsStr::new(&replacement),
            ],
        );

        let verified = verify_git_source(
            directory.path(),
            &pin(&origin, original_revision, "fixtures"),
        )
        .expect("replacement refs do not alter the pinned snapshot");
        assert_eq!(
            verified
                .snapshot
                .entries
                .iter()
                .map(|entry| entry.path.as_str())
                .collect::<Vec<_>>(),
            ["case.json", "nested/[literal].json"]
        );
        let case = verified
            .snapshot
            .entries
            .iter()
            .find(|entry| entry.path.as_str() == "case.json")
            .unwrap();
        assert_eq!(case.bytes, b"{}\n");
        assert_eq!(case.digest, Sha256Digest::from_bytes(b"{}\n"));

        fs::write(
            directory.path().join("fixtures/case.json"),
            b"changed afterward\n",
        )
        .unwrap();
        assert_eq!(
            case.bytes, b"{}\n",
            "snapshot bytes followed the checkout pathname"
        );
    }

    #[test]
    fn recursive_local_alternates_are_protected_and_cycles_fail_closed() {
        let (base, _origin, revision) = repository();
        let middle = TestDirectory::new("alternate-middle");
        let leaf = TestDirectory::new("alternate-leaf");
        fs::remove_dir(middle.path()).unwrap();
        fs::remove_dir(leaf.path()).unwrap();
        run_git_program(&[
            OsStr::new("clone"),
            OsStr::new("--quiet"),
            OsStr::new("--shared"),
            base.path().as_os_str(),
            middle.path().as_os_str(),
        ]);
        run_git_program(&[
            OsStr::new("clone"),
            OsStr::new("--quiet"),
            OsStr::new("--shared"),
            middle.path().as_os_str(),
            leaf.path().as_os_str(),
        ]);
        let origin = "https://example.invalid/source.git";
        run_git(
            leaf.path(),
            &[
                OsStr::new("remote"),
                OsStr::new("set-url"),
                OsStr::new("origin"),
                OsStr::new(origin),
            ],
        );
        let verified = verify_git_source(leaf.path(), &pin(origin, revision.clone(), "fixtures"))
            .expect("recursive local alternates");
        let base_objects = fs::canonicalize(base.path().join(".git/objects")).unwrap();
        let middle_objects = fs::canonicalize(middle.path().join(".git/objects")).unwrap();
        assert!(
            verified
                .protection
                .iter()
                .any(|entry| entry.path == base_objects)
        );
        assert!(
            verified
                .protection
                .iter()
                .any(|entry| entry.path == middle_objects)
        );

        fs::create_dir_all(base.path().join(".git/objects/info")).unwrap();
        fs::write(
            base.path().join(".git/objects/info/alternates"),
            format!("{}\n", leaf.path().join(".git/objects").display()),
        )
        .unwrap();
        let error = verify_git_source(leaf.path(), &pin(origin, revision, "fixtures"))
            .expect_err("alternate recursion cycle must fail");
        assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
    }

    #[test]
    fn locally_missing_promisor_blob_fails_without_fetching_or_repopulating() {
        let (remote, _origin, revision) = repository();
        let missing = TestDirectory::new("promisor-missing");
        run_git(missing.path(), &[OsStr::new("init"), OsStr::new("--quiet")]);

        let commit = revision.as_str().to_owned();
        let root_tree = run_git(
            remote.path(),
            &[OsStr::new("rev-parse"), OsStr::new("HEAD^{tree}")],
        );
        let fixture_tree = run_git(
            remote.path(),
            &[OsStr::new("rev-parse"), OsStr::new("HEAD:fixtures")],
        );
        let blob = run_git(
            remote.path(),
            &[
                OsStr::new("rev-parse"),
                OsStr::new("HEAD:fixtures/case.json"),
            ],
        );
        for object in [&commit, &root_tree, &fixture_tree] {
            let (directory, file) = object.split_at(2);
            let target_directory = missing.path().join(".git/objects").join(directory);
            fs::create_dir_all(&target_directory).unwrap();
            fs::copy(
                remote
                    .path()
                    .join(".git/objects")
                    .join(directory)
                    .join(file),
                target_directory.join(file),
            )
            .expect("copy non-blob loose object");
        }
        fs::copy(
            remote.path().join(".git/index"),
            missing.path().join(".git/index"),
        )
        .expect("copy index referencing missing blob");
        fs::create_dir(missing.path().join("fixtures")).unwrap();
        fs::write(missing.path().join("fixtures/case.json"), b"{}\n").unwrap();
        run_git(
            missing.path(),
            &[
                OsStr::new("update-ref"),
                OsStr::new("refs/heads/main"),
                OsStr::new(&commit),
            ],
        );
        run_git(
            missing.path(),
            &[
                OsStr::new("symbolic-ref"),
                OsStr::new("HEAD"),
                OsStr::new("refs/heads/main"),
            ],
        );
        let origin = "https://example.invalid/source.git";
        run_git(
            missing.path(),
            &[
                OsStr::new("remote"),
                OsStr::new("add"),
                OsStr::new("origin"),
                OsStr::new(origin),
            ],
        );
        run_git(
            missing.path(),
            &[
                OsStr::new("remote"),
                OsStr::new("add"),
                OsStr::new("promisor"),
                remote.path().as_os_str(),
            ],
        );
        for (key, value) in [
            ("core.repositoryformatversion", "1"),
            ("extensions.partialClone", "promisor"),
            ("remote.promisor.promisor", "true"),
            ("remote.promisor.partialCloneFilter", "blob:none"),
        ] {
            run_git(
                missing.path(),
                &[OsStr::new("config"), OsStr::new(key), OsStr::new(value)],
            );
        }

        let (blob_directory, blob_file) = blob.split_at(2);
        let missing_blob = missing
            .path()
            .join(".git/objects")
            .join(blob_directory)
            .join(blob_file);
        assert!(
            !missing_blob.exists(),
            "test setup accidentally copied the blob"
        );
        let error = verify_git_source(missing.path(), &pin(origin, revision, "fixtures"))
            .expect_err("locally absent promisor blob must not lazy-fetch");
        assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
        assert!(
            !missing_blob.exists(),
            "verification repopulated the missing blob"
        );
    }

    #[test]
    fn source_verification_does_not_refresh_git_index() {
        let (directory, origin, revision) = repository();
        let object = run_git(
            directory.path(),
            &[
                OsStr::new("rev-parse"),
                OsStr::new("HEAD:fixtures/case.json"),
            ],
        );
        let cache_entry = format!("100644,{object},fixtures/case.json");
        run_git(
            directory.path(),
            &[
                OsStr::new("update-index"),
                OsStr::new("--cacheinfo"),
                OsStr::new(&cache_entry),
            ],
        );
        let index_text = run_git(
            directory.path(),
            &[
                OsStr::new("rev-parse"),
                OsStr::new("--git-path"),
                OsStr::new("index"),
            ],
        );
        let index_path = Path::new(&index_text);
        let index_path = if index_path.is_absolute() {
            index_path.to_path_buf()
        } else {
            directory.path().join(index_path)
        };
        let before = fs::read(&index_path).expect("read deliberately stale Git index");

        verify_git_source(directory.path(), &pin(&origin, revision, "fixtures"))
            .expect("verify clean source");

        let after = fs::read(index_path).expect("read Git index after verification");
        assert_eq!(after, before, "source verification refreshed the Git index");
    }

    #[test]
    fn pinned_source_requires_exact_clean_revision() {
        let (directory, origin, revision) = repository();
        assert!(SourceRevision::new(&revision.as_str()[..12]).is_err());
        let verified = verify_git_source(
            directory.path(),
            &pin(&origin, revision.clone(), "fixtures"),
        )
        .expect("exact clean source");
        assert_eq!(verified.revision(), &revision);

        let prefix = different_full_revision(&revision);
        assert_ne!(prefix, revision, "wrong revision fixture must be distinct");
        assert_eq!(
            verify_git_source(directory.path(), &pin(&origin, prefix, "fixtures"))
                .expect_err("wrong revision")
                .kind(),
            GeneratorErrorKind::SourceVerification
        );

        fs::write(directory.path().join("untracked.json"), b"{}\n").expect("dirty checkout");
        assert!(verify_git_source(directory.path(), &pin(&origin, revision, "fixtures")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn pinned_source_rejects_wrong_origin_and_escaped_source_root() {
        use std::os::unix::fs::symlink;

        let (directory, origin, revision) = repository();
        assert!(
            verify_git_source(
                directory.path(),
                &pin(
                    "https://example.invalid/wrong.git",
                    revision.clone(),
                    "fixtures"
                )
            )
            .is_err()
        );

        symlink(std::env::temp_dir(), directory.path().join("escape")).expect("create symlink");
        run_git(directory.path(), &[OsStr::new("add"), OsStr::new("escape")]);
        run_git(
            directory.path(),
            &[
                OsStr::new("commit"),
                OsStr::new("--quiet"),
                OsStr::new("-m"),
                OsStr::new("escape"),
            ],
        );
        let escaped_revision = SourceRevision::new(run_git(
            directory.path(),
            &[OsStr::new("rev-parse"), OsStr::new("HEAD")],
        ))
        .expect("full revision");
        let error = verify_git_source(directory.path(), &pin(&origin, escaped_revision, "escape"))
            .expect_err("source root symlink escape");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);
    }
}
