use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use chromiumoxide::browser::BrowserConfig;
use sha2::{Digest, Sha256};

use crate::core::{BoundPath, HeldIdentity, NodeKind};
use crate::{
    CorpusLocation, GeneratorError, GeneratorErrorKind, RelativePath, Result, Sha256Digest,
};

use super::manifest::LayoutManifest;

const DRIVER_KEYS: [&str; 3] = [
    "remote-debugging-port",
    "disable-extensions",
    "user-data-dir",
];

/// One path/descriptor-bound trusted external browser capability.
#[derive(Debug)]
pub(super) struct TrustedBrowser {
    owner: BoundPath,
    cache: BoundPath,
    executable: BoundPath,
    relative: RelativePath,
    identity: HeldIdentity,
    digest: Sha256Digest,
}

impl TrustedBrowser {
    pub(super) fn validate(
        location: &CorpusLocation,
        manifest: &LayoutManifest,
        relative: &RelativePath,
    ) -> Result<Self> {
        let cache_prefix = format!("{}/", manifest.browser.cache_root.as_str());
        if relative.as_str() == manifest.browser.cache_root.as_str()
            || !relative.as_str().starts_with(&cache_prefix)
        {
            return Err(invalid_path(
                "trusted browser executable must be a strict descendant of browser.cache_root",
            ));
        }

        let owner = BoundPath::bind(location.owner_root())?;
        owner.require_existing_directory("bind trusted browser owner root")?;
        let cache_path = manifest.browser.cache_root.join(location.owner_root());
        let executable_path = relative.join(location.owner_root());
        let cache = BoundPath::bind(&cache_path)?;
        cache.require_existing_directory("bind trusted browser cache root")?;
        if !cache.is_strict_descendant_of(&owner) {
            return Err(invalid_path(
                "trusted browser cache root does not resolve to a strict descendant of its owner root",
            ));
        }
        let executable = BoundPath::bind(&executable_path)?;
        if !executable.is_strict_descendant_of(&cache) {
            return Err(invalid_path(
                "trusted browser executable does not resolve to a strict descendant of browser.cache_root",
            ));
        }
        let identity = executable.existing_identity().clone();
        if identity.kind() != NodeKind::Regular
            || identity.link_count() != Some(1)
            || identity.mode() & 0o111 == 0
        {
            return Err(invalid_path(
                "trusted browser executable must be an executable single-link regular file",
            ));
        }
        if cache.overlaps(&BoundPath::bind(location.corpus_root())?)? {
            return Err(invalid_path(
                "trusted browser cache root must be disjoint from the complete corpus root",
            ));
        }
        let digest = digest_held(&executable).map_err(source_verification_revalidation)?;
        Ok(Self {
            owner,
            cache,
            executable,
            relative: relative.clone(),
            identity,
            digest,
        })
    }

    pub(super) fn closing_revalidate(&self) -> Result<()> {
        self.owner.revalidate()?;
        self.cache.revalidate()?;
        if !self.cache.is_strict_descendant_of(&self.owner) {
            return Err(invalid_path(
                "trusted browser cache root no longer resolves beneath its owner root",
            ));
        }
        self.executable
            .revalidate()
            .map_err(source_verification_revalidation)?;
        let contained = self.executable.is_strict_descendant_of(&self.cache);
        let identity_matches = self
            .identity
            .matches_recovery(self.executable.existing_identity());
        let digest_matches =
            digest_held(&self.executable).map_err(source_verification_revalidation)? == self.digest;
        if !contained || !identity_matches || !digest_matches {
            return Err(GeneratorError::new(
                GeneratorErrorKind::SourceVerification,
                "revalidate trusted browser executable",
                format!(
                    "browser containment, identity, or raw bytes changed after preflight \
                     (contained={contained}, identity_matches={identity_matches}, \
                     digest_matches={digest_matches})"
                ),
            ));
        }
        Ok(())
    }

    pub(super) const fn identity(&self) -> &HeldIdentity {
        &self.identity
    }

    pub(super) const fn digest(&self) -> &Sha256Digest {
        &self.digest
    }

    pub(super) const fn relative(&self) -> &RelativePath {
        &self.relative
    }

    pub(super) fn absolute_path(&self) -> &Path {
        self.executable.canonical_path()
    }

    pub(super) fn provenance(&self, manifest: &LayoutManifest) -> String {
        manifest
            .browser
            .provenance_format
            .replace("{version}", &manifest.browser.version)
            .replace("{repository_relative_executable}", self.relative.as_str())
    }
}

fn source_verification_revalidation(source: GeneratorError) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::SourceVerification,
        "revalidate trusted browser executable",
        source.to_string(),
        source,
    )
}

pub(super) fn fixed_environment(profile: &Path) -> Vec<(OsString, OsString)> {
    let entries = [
        ("HOME", profile.join("home").into_os_string()),
        ("TMPDIR", profile.join("tmp").into_os_string()),
        ("TMP", profile.join("tmp").into_os_string()),
        ("TEMP", profile.join("tmp").into_os_string()),
        (
            "XDG_CONFIG_HOME",
            profile.join("xdg-config").into_os_string(),
        ),
        ("XDG_CACHE_HOME", profile.join("xdg-cache").into_os_string()),
        ("XDG_DATA_HOME", profile.join("xdg-data").into_os_string()),
        ("PATH", OsString::from("/usr/bin:/bin")),
        ("TZ", OsString::from("UTC")),
        ("LANG", OsString::from("C")),
        ("LC_ALL", OsString::from("C")),
        ("HTTP_PROXY", OsString::new()),
        ("HTTPS_PROXY", OsString::new()),
        ("ALL_PROXY", OsString::new()),
        ("http_proxy", OsString::new()),
        ("https_proxy", OsString::new()),
        ("all_proxy", OsString::new()),
        ("NO_PROXY", OsString::from("*")),
        ("no_proxy", OsString::from("*")),
    ];
    entries
        .into_iter()
        .map(|(key, value)| (OsString::from(key), value))
        .collect()
}

pub(super) fn effective_switches(
    manifest: &LayoutManifest,
    profile: &Path,
) -> Result<BTreeMap<String, Option<String>>> {
    let mut switches = BTreeMap::new();
    for argument in &manifest.browser.launch.arguments {
        let (key, value) = split_switch(argument)?;
        if switches
            .insert(key.to_owned(), value.map(str::to_owned))
            .is_some()
        {
            return Err(invalid_manifest("duplicate normalized browser switch"));
        }
    }
    switches.insert(DRIVER_KEYS[0].to_owned(), Some("0".to_owned()));
    switches.insert(DRIVER_KEYS[1].to_owned(), None);
    switches.insert(
        DRIVER_KEYS[2].to_owned(),
        Some(profile.to_string_lossy().into_owned()),
    );
    Ok(switches)
}

pub(super) fn validate_received_switches(
    manifest: &LayoutManifest,
    arguments: &[OsString],
) -> Result<BTreeMap<String, Option<OsString>>> {
    let expected = effective_switches(manifest, Path::new("profile"))?;
    let expected_keys = expected.keys().cloned().collect::<BTreeSet<_>>();
    let mut received = BTreeMap::new();
    for argument in arguments {
        let text = argument
            .to_str()
            .ok_or_else(|| invalid_manifest("supervisor switch is not UTF-8"))?;
        let normalized = text.strip_prefix("--").unwrap_or(text);
        let (key, value) = split_switch(normalized)?;
        if received
            .insert(key.to_owned(), value.map(OsString::from))
            .is_some()
        {
            return Err(invalid_manifest(
                "supervisor received a duplicate switch key",
            ));
        }
    }
    if received.keys().cloned().collect::<BTreeSet<_>>() != expected_keys {
        return Err(invalid_manifest(
            "supervisor received a switch set other than manifest-plus-driver authority",
        ));
    }
    for (key, expected_value) in expected {
        let actual = received
            .get(&key)
            .expect("equal switch key sets have every expected member");
        let matches = if key == "user-data-dir" {
            actual.as_ref().is_some_and(|value| !value.is_empty())
        } else {
            actual.as_ref().and_then(|value| value.to_str()) == expected_value.as_deref()
        };
        if !matches {
            return Err(invalid_manifest(format!(
                "supervisor switch value differs from manifest authority: {key}"
            )));
        }
    }
    Ok(received)
}

pub(super) fn chromium_config(
    supervisor: &Path,
    profile: &Path,
    manifest: &LayoutManifest,
    capsule: &str,
) -> Result<BrowserConfig> {
    BrowserConfig::builder()
        .chrome_executable(supervisor)
        .with_head()
        .disable_default_args()
        .disable_cache()
        .user_data_dir(profile)
        .args(manifest.browser.launch.arguments.clone())
        .env(super::supervisor::CAPSULE_ENV, capsule)
        .build()
        .map_err(|detail| {
            GeneratorError::new(
                GeneratorErrorKind::Generation,
                "construct pinned Chromiumoxide configuration",
                detail,
            )
        })
}

fn split_switch(argument: &str) -> Result<(&str, Option<&str>)> {
    let (key, value) = argument
        .split_once('=')
        .map_or((argument, None), |(key, value)| (key, Some(value)));
    if key.is_empty() || key.starts_with('-') {
        return Err(invalid_manifest("browser switch has a malformed key"));
    }
    Ok((key, value))
}

fn digest_held(path: &BoundPath) -> Result<Sha256Digest> {
    let mut file = path.held_regular_file()?;
    file.seek(SeekFrom::Start(0)).map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::SourceVerification,
            "rewind held trusted browser executable",
            path.canonical_path().display().to_string(),
            source,
        )
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer).map_err(|source| {
            GeneratorError::with_source(
                GeneratorErrorKind::SourceVerification,
                "hash held trusted browser executable",
                path.canonical_path().display().to_string(),
                source,
            )
        })?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    path.revalidate()?;
    Sha256Digest::from_text(format!("{:x}", hasher.finalize()))
}

fn invalid_path(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidPath,
        "validate trusted browser capability",
        detail,
    )
}

fn invalid_manifest(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidManifest,
        "validate trusted browser launch switches",
        detail,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::ffi::OsString;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        DRIVER_KEYS, TrustedBrowser, effective_switches, fixed_environment,
        validate_received_switches,
    };
    use crate::layout::{manifest, tests};
    use crate::{CorpusLocation, GeneratorErrorKind, RelativePath};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "surgeist-layout-browser-test-{}-{sequence:016x}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create browser test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove browser test directory");
        }
    }

    fn executable(path: &Path) {
        fs::create_dir_all(path.parent().expect("browser parent")).expect("create browser parent");
        fs::write(path, b"synthetic trusted browser\n").expect("write browser");
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .expect("make browser executable");
    }

    fn parsed_manifest(corpus: &Path) -> super::LayoutManifest {
        let text = tests::manifest_text(tests::SHA1_REVISION, 1, "");
        manifest::parse(text.as_bytes(), &corpus.join("corpus.toml")).expect("manifest")
    }

    #[test]
    fn layout_browser_driver_switch_keys_are_exact() {
        assert_eq!(
            DRIVER_KEYS.into_iter().collect::<BTreeSet<_>>(),
            [
                "disable-extensions",
                "remote-debugging-port",
                "user-data-dir"
            ]
            .into_iter()
            .collect()
        );
    }

    #[test]
    fn layout_browser_cleared_environment_is_exact() {
        let profile = Path::new("/private/profile");
        let environment = fixed_environment(profile)
            .into_iter()
            .collect::<std::collections::BTreeMap<_, _>>();
        assert_eq!(environment.len(), 19);
        assert_eq!(environment[&OsString::from("HOME")], profile.join("home"));
        assert_eq!(environment[&OsString::from("TMP")], profile.join("tmp"));
        assert_eq!(environment[&OsString::from("PATH")], "/usr/bin:/bin");
        assert_eq!(environment[&OsString::from("NO_PROXY")], "*");
        assert_eq!(environment[&OsString::from("HTTP_PROXY")], "");
    }

    #[test]
    fn layout_browser_manifest_plus_driver_switch_set_is_exact() {
        let text = tests::manifest_text(tests::SHA1_REVISION, 1, "");
        let manifest =
            manifest::parse(text.as_bytes(), Path::new("corpus.toml")).expect("manifest");
        let effective = effective_switches(&manifest, Path::new("/private/profile"))
            .expect("effective switches");
        assert_eq!(effective.len(), 31);
        let mut arguments = effective
            .iter()
            .rev()
            .map(|(key, value)| {
                OsString::from(
                    value
                        .as_ref()
                        .map_or_else(|| format!("--{key}"), |value| format!("--{key}={value}")),
                )
            })
            .collect::<Vec<_>>();
        let received = validate_received_switches(&manifest, &arguments).expect("permutation");
        assert_eq!(received.len(), 31);
        let value_index = arguments
            .iter()
            .position(|argument| {
                argument
                    .to_str()
                    .is_some_and(|value| value.starts_with("--disable-features="))
            })
            .expect("manifest value switch");
        let original = arguments[value_index].clone();
        arguments[value_index] = OsString::from("--disable-features=Different");
        validate_received_switches(&manifest, &arguments).expect_err("value drift rejected");
        arguments[value_index] = original;
        arguments.push(OsString::from("--unexpected"));
        validate_received_switches(&manifest, &arguments).expect_err("extra switch rejected");
    }

    #[test]
    fn layout_browser_user_data_profile_is_driver_owned() {
        let text = tests::manifest_text(tests::SHA1_REVISION, 1, "");
        let manifest = manifest::parse(text.as_bytes(), PathBuf::from("corpus.toml").as_path())
            .expect("manifest");
        let first = effective_switches(&manifest, Path::new("/one")).expect("first profile");
        let second = effective_switches(&manifest, Path::new("/two")).expect("second profile");
        assert_eq!(
            first.keys().collect::<Vec<_>>(),
            second.keys().collect::<Vec<_>>()
        );
        assert_ne!(first["user-data-dir"], second["user-data-dir"]);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn layout_browser_intermediate_symlink_escape_is_invalid_path() {
        let temporary = TestDirectory::new();
        let owner = temporary.path().join("owner");
        let corpus = owner.join("corpus");
        let cache = owner.join("browser-cache");
        let outside = temporary.path().join("outside");
        fs::create_dir_all(&corpus).expect("create corpus");
        fs::create_dir(&cache).expect("create cache");
        fs::create_dir(&outside).expect("create outside");
        executable(&outside.join("chromium"));
        symlink(&outside, cache.join("escape")).expect("create intermediate symlink");
        let location = CorpusLocation::new(&owner, &corpus).expect("location");
        let manifest = parsed_manifest(&corpus);
        let relative = RelativePath::new("browser-cache/escape/chromium").expect("browser path");

        let error = TrustedBrowser::validate(&location, &manifest, &relative)
            .expect_err("real path outside cache must be rejected");

        assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn layout_browser_cache_root_symlink_escape_is_invalid_path() {
        let temporary = TestDirectory::new();
        let owner = temporary.path().join("owner");
        let corpus = owner.join("corpus");
        let outside = temporary.path().join("outside");
        fs::create_dir_all(&corpus).expect("create corpus");
        fs::create_dir(&outside).expect("create outside cache");
        executable(&outside.join("chromium"));
        symlink(&outside, owner.join("browser-cache")).expect("alias cache outside owner");
        let location = CorpusLocation::new(&owner, &corpus).expect("location");
        let manifest = parsed_manifest(&corpus);
        let relative = RelativePath::new("browser-cache/chromium").expect("browser path");

        let error = TrustedBrowser::validate(&location, &manifest, &relative)
            .expect_err("cache root outside its owner must be rejected");

        assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn layout_browser_intermediate_path_drift_fails_closing_revalidation() {
        let temporary = TestDirectory::new();
        let owner = temporary.path().join("owner");
        let corpus = owner.join("corpus");
        let cache = owner.join("browser-cache");
        let original = cache.join("live");
        let displaced = cache.join("held");
        let outside = temporary.path().join("outside");
        fs::create_dir_all(&corpus).expect("create corpus");
        fs::create_dir_all(&original).expect("create cache child");
        fs::create_dir(&outside).expect("create outside");
        executable(&original.join("chromium"));
        executable(&outside.join("chromium"));
        let location = CorpusLocation::new(&owner, &corpus).expect("location");
        let manifest = parsed_manifest(&corpus);
        let relative = RelativePath::new("browser-cache/live/chromium").expect("browser path");
        let browser = TrustedBrowser::validate(&location, &manifest, &relative)
            .expect("initial trusted browser");

        fs::rename(&original, &displaced).expect("displace held cache child");
        symlink(&outside, &original).expect("replace path with escaping symlink");
        let error = browser
            .closing_revalidate()
            .expect_err("closing validation must detect intermediate drift");

        assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn layout_browser_cache_root_replacement_is_invalid_path_at_close() {
        let temporary = TestDirectory::new();
        let owner = temporary.path().join("owner");
        let corpus = owner.join("corpus");
        let cache = owner.join("browser-cache");
        let displaced = owner.join("held-browser-cache");
        let outside = temporary.path().join("outside");
        fs::create_dir_all(&corpus).expect("create corpus");
        fs::create_dir(&cache).expect("create cache");
        fs::create_dir(&outside).expect("create outside cache");
        executable(&cache.join("chromium"));
        executable(&outside.join("chromium"));
        let location = CorpusLocation::new(&owner, &corpus).expect("location");
        let manifest = parsed_manifest(&corpus);
        let relative = RelativePath::new("browser-cache/chromium").expect("browser path");
        let browser = TrustedBrowser::validate(&location, &manifest, &relative)
            .expect("initial trusted browser");

        fs::rename(&cache, &displaced).expect("displace cache root");
        symlink(&outside, &cache).expect("replace cache root with escaping symlink");
        let error = browser
            .closing_revalidate()
            .expect_err("closing validation must reject cache-root replacement");

        assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);
    }
}
