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

use super::model::EngineManifest;

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
        manifest: &EngineManifest,
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

        let owner = BoundPath::bind(manifest.browser_owner.as_path())?;
        owner.require_existing_directory("bind trusted browser owner root")?;
        let cache_path = manifest
            .browser
            .cache_root
            .join(manifest.browser_owner.as_path());
        let executable_path = relative.join(manifest.browser_owner.as_path());
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

    pub(super) fn provenance(&self, manifest: &EngineManifest) -> String {
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
    manifest: &EngineManifest,
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
    manifest: &EngineManifest,
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
    manifest: &EngineManifest,
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
