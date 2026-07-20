use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::core::{CORPUS_FILE_MODE, RootedFs};
use crate::{
    GeneratorError, GeneratorErrorKind, ManifestVersion, RelativePath, Result, Sha256Digest,
    SourceRevision, parse_manifest,
};

use super::case;

pub(super) const MANIFEST_FILE: &str = "corpus.toml";
pub(super) const TAFFY_REPOSITORY: &str = "https://github.com/DioxusLabs/taffy.git";
pub(super) const TAFFY_SOURCE_DIRECTORY: &str = "test_fixtures";
pub(super) const HTML_ROOT: &str = "html";
pub(super) const SIDECAR_FILE: &str = ".surgeist-taffy-source.json";
pub(super) const EXCLUDED_DIRECTORIES: [&str; 2] = ["grid-lanes", "subgrid"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LayoutManifest {
    pub(super) revision: SourceRevision,
    pub(super) expected_source_files: usize,
    pub(super) authored_files: BTreeSet<RelativePath>,
    pub(super) authored_cases: Vec<case::LayoutCase>,
    pub(super) browser: BrowserManifest,
    pub(super) reports: GenerationReportManifest,
    pub(super) launch_digest: Sha256Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct BrowserManifest {
    pub(super) source: String,
    pub(super) version: String,
    pub(super) cache_root: RelativePath,
    pub(super) provenance_format: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FullReportExpectations {
    pub(super) generated: usize,
    pub(super) unsupported: usize,
    pub(super) expected_fail: usize,
    pub(super) quarantined: usize,
    pub(super) failed_to_generate: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ScopedReportManifest {
    pub(super) filter: RelativePath,
    pub(super) file: RelativePath,
    pub(super) generated: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct GenerationReportManifest {
    pub(super) full: FullReportExpectations,
    pub(super) scoped: Vec<ScopedReportManifest>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    schema_version: ManifestVersion,
    browser: RawBrowser,
    generation_reports: RawGenerationReports,
    source_roots: RawSourceRoots,
    imports: RawImports,
    #[serde(default)]
    cases: Vec<case::RawCase>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawBrowser {
    source: String,
    version: String,
    version_output: String,
    cache_root: RelativePath,
    provenance_format: String,
    launch: RawLaunch,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLaunch {
    batch_size: u64,
    navigation_timeout_ms: u64,
    dom_poll_interval_ms: u64,
    retry_count: u64,
    job_order: String,
    retry_error_class: String,
    profile_scope: String,
    page_scope: String,
    disable_default_args: bool,
    disable_cache: bool,
    arguments: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawGenerationReports {
    full: RawFullReport,
    #[serde(default)]
    scoped: Vec<RawScopedReport>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawFullReport {
    file: String,
    generated: u64,
    unsupported: u64,
    expected_fail: u64,
    quarantined: u64,
    failed_to_generate: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawScopedReport {
    filter: RelativePath,
    file: RelativePath,
    generated: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSourceRoots {
    taffy: RawSourceRoot,
    surgeist: RawSourceRoot,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSourceRoot {
    kind: String,
    path: RelativePath,
    #[serde(default)]
    upstream_commit: Option<SourceRevision>,
    description: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawImports {
    taffy: RawTaffyImport,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawTaffyImport {
    repo: String,
    commit: SourceRevision,
    source_dir: RelativePath,
    destination: RelativePath,
    expected_count: u64,
    excluded_destination_dirs: Vec<RelativePath>,
}

pub(super) fn parse(bytes: &[u8], path: &Path) -> Result<LayoutManifest> {
    parse_complete(bytes, path).map(|(manifest, _)| manifest)
}

#[cfg(test)]
pub(super) fn parse_with_launch_digest(
    bytes: &[u8],
    path: &Path,
) -> Result<(LayoutManifest, Sha256Digest)> {
    parse_complete(bytes, path)
}

fn parse_complete(bytes: &[u8], path: &Path) -> Result<(LayoutManifest, Sha256Digest)> {
    let text = std::str::from_utf8(bytes).map_err(|_| invalid_manifest("manifest is not UTF-8"))?;
    let raw: RawManifest = parse_manifest(text, path)?;
    raw.schema_version.require(ManifestVersion::new(2)?, path)?;
    validate_browser(&raw.browser)?;
    validate_reports(&raw.generation_reports)?;
    validate_source_roots(&raw.source_roots)?;
    let launch_digest = launch_digest(&raw.browser.launch)?;
    let cases = case::validate(raw.cases)?;
    let authored_files = cases.authored_files;
    let import = raw.imports.taffy;
    if import.repo != TAFFY_REPOSITORY {
        return Err(invalid_manifest(
            "imports.taffy.repo must be the canonical Taffy HTTPS Git URL",
        ));
    }
    if import.source_dir.as_str() != TAFFY_SOURCE_DIRECTORY {
        return Err(invalid_manifest(
            "imports.taffy.source_dir must be exactly test_fixtures",
        ));
    }
    if import.destination.as_str() != HTML_ROOT {
        return Err(invalid_manifest(
            "imports.taffy.destination must be exactly html",
        ));
    }
    let source_revision = raw
        .source_roots
        .taffy
        .upstream_commit
        .expect("validated Taffy source root has an upstream commit");
    if source_revision != import.commit {
        return Err(invalid_manifest(
            "Taffy revision fields must contain the same full object ID",
        ));
    }
    let expected_source_files = positive_usize(import.expected_count, "expected_count")?;
    validate_exclusions(import.excluded_destination_dirs)?;
    for authored in &authored_files {
        if paths_target_equal(authored.as_str(), SIDECAR_FILE) {
            return Err(invalid_manifest(
                "a Surgeist-authored case collides with the reserved Taffy sidecar",
            ));
        }
    }
    Ok((
        LayoutManifest {
            revision: import.commit,
            expected_source_files,
            authored_files,
            authored_cases: cases.authored_cases,
            browser: BrowserManifest {
                source: raw.browser.source,
                version: raw.browser.version,
                cache_root: raw.browser.cache_root,
                provenance_format: raw.browser.provenance_format,
            },
            reports: report_manifest(raw.generation_reports)?,
            launch_digest: launch_digest.clone(),
        },
        launch_digest,
    ))
}

fn report_manifest(raw: RawGenerationReports) -> Result<GenerationReportManifest> {
    Ok(GenerationReportManifest {
        full: FullReportExpectations {
            generated: nonnegative_usize(raw.full.generated, "generation_reports.full.generated")?,
            unsupported: nonnegative_usize(
                raw.full.unsupported,
                "generation_reports.full.unsupported",
            )?,
            expected_fail: nonnegative_usize(
                raw.full.expected_fail,
                "generation_reports.full.expected_fail",
            )?,
            quarantined: nonnegative_usize(
                raw.full.quarantined,
                "generation_reports.full.quarantined",
            )?,
            failed_to_generate: nonnegative_usize(
                raw.full.failed_to_generate,
                "generation_reports.full.failed_to_generate",
            )?,
        },
        scoped: raw
            .scoped
            .into_iter()
            .map(|scoped| {
                Ok(ScopedReportManifest {
                    filter: scoped.filter,
                    file: scoped.file,
                    generated: nonnegative_usize(
                        scoped.generated,
                        "generation_reports.scoped.generated",
                    )?,
                })
            })
            .collect::<Result<Vec<_>>>()?,
    })
}

fn validate_browser(browser: &RawBrowser) -> Result<()> {
    if browser.source != "chrome-for-testing" {
        return Err(invalid_manifest(
            "browser.source must be exactly chrome-for-testing",
        ));
    }
    require_trimmed_text(&browser.version, "browser.version")?;
    require_trimmed_text(&browser.version_output, "browser.version_output")?;
    if browser
        .cache_root
        .as_str()
        .split('/')
        .any(reserved_component)
    {
        return Err(invalid_manifest(
            "browser.cache_root uses a reserved component",
        ));
    }
    validate_provenance_format(&browser.provenance_format)?;
    validate_launch(&browser.launch)
}

fn validate_launch(launch: &RawLaunch) -> Result<()> {
    if launch.batch_size == 0
        || launch.navigation_timeout_ms == 0
        || launch.dom_poll_interval_ms == 0
    {
        return Err(invalid_manifest(
            "browser launch numeric values must be positive",
        ));
    }
    if launch.retry_count != 1
        || launch.job_order != "sorted-sequential"
        || launch.retry_error_class != "open-load-reset-timeout"
        || launch.profile_scope != "per-batch-and-retry"
        || launch.page_scope != "per-job"
        || !launch.disable_default_args
        || !launch.disable_cache
    {
        return Err(invalid_manifest(
            "browser launch lifecycle contract is noncanonical",
        ));
    }
    if launch.arguments.len() != 28 {
        return Err(invalid_manifest(
            "browser.launch.arguments must contain exactly 28 entries",
        ));
    }
    let mut keys = BTreeSet::new();
    let mut has_mock_keychain = false;
    for argument in &launch.arguments {
        let key = normalized_launch_key(argument)?;
        has_mock_keychain |= argument
            .strip_prefix("--")
            .unwrap_or(argument)
            .eq("use-mock-keychain");
        if !keys.insert(key.to_owned()) {
            return Err(invalid_manifest(
                "browser launch arguments have duplicate normalized keys",
            ));
        }
    }
    if !has_mock_keychain {
        return Err(invalid_manifest(
            "browser launch arguments omit use-mock-keychain",
        ));
    }
    Ok(())
}

fn normalized_launch_key(argument: &str) -> Result<&str> {
    if argument.is_empty()
        || !argument.bytes().all(|byte| (0x20..=0x7e).contains(&byte))
        || argument.contains(['/', '\\'])
    {
        return Err(invalid_manifest(
            "browser launch argument must be printable path-free ASCII",
        ));
    }
    let normalized = argument.strip_prefix("--").unwrap_or(argument);
    let key = normalized
        .split_once('=')
        .map_or(normalized, |(key, _)| key);
    if key.is_empty() || key.starts_with('-') || !key.is_ascii() {
        return Err(invalid_manifest(
            "browser launch argument has a malformed normalized key",
        ));
    }
    const FORBIDDEN: [&str; 22] = [
        "user-data-dir",
        "disk-cache-dir",
        "media-cache-dir",
        "data-path",
        "homedir",
        "log-file",
        "log-net-log",
        "crash-dumps-dir",
        "crash-dump-dir",
        "breakpad-dump-location",
        "download-default-directory",
        "ssl-key-log-file",
        "trace-startup-file",
        "profiling-file",
        "print-to-pdf",
        "screenshot",
        "remote-debugging-port",
        "remote-debugging-address",
        "remote-debugging-pipe",
        "disable-extensions",
        "load-extension",
        "disable-extensions-except",
    ];
    if FORBIDDEN.contains(&key) {
        return Err(invalid_manifest(
            "browser launch argument uses a driver-owned or redirecting key",
        ));
    }
    Ok(key)
}

fn launch_digest(launch: &RawLaunch) -> Result<Sha256Digest> {
    let bytes = serde_json::to_vec(&(
        1_u8,
        launch.batch_size,
        launch.navigation_timeout_ms,
        launch.dom_poll_interval_ms,
        launch.retry_count,
        &launch.job_order,
        &launch.retry_error_class,
        &launch.profile_scope,
        &launch.page_scope,
        launch.disable_default_args,
        launch.disable_cache,
        &launch.arguments,
    ))
    .map_err(|error| {
        GeneratorError::with_source(
            GeneratorErrorKind::InvalidManifest,
            "derive layout launch digest",
            "launch tuple serialization failed",
            error,
        )
    })?;
    Ok(Sha256Digest::from_bytes(bytes))
}

fn validate_reports(reports: &RawGenerationReports) -> Result<()> {
    if reports.full.file != "all.json" {
        return Err(invalid_manifest(
            "generation_reports.full.file must be exactly all.json",
        ));
    }
    let _ = (
        reports.full.generated,
        reports.full.unsupported,
        reports.full.expected_fail,
        reports.full.quarantined,
        reports.full.failed_to_generate,
    );
    let mut filters = BTreeSet::new();
    let mut files = BTreeSet::from([reports.full.file.as_str()]);
    for scoped in &reports.scoped {
        if paths_target_equal(scoped.filter.as_str(), SIDECAR_FILE) {
            return Err(invalid_manifest(
                "scoped report filter collides with the Taffy sidecar",
            ));
        }
        let scoped_file = scoped.file.as_str();
        if scoped_file.split('/').count() != 1
            || scoped_file == "all.json"
            || !scoped_file.ends_with(".json")
            || scoped_file.len() == ".json".len()
        {
            return Err(invalid_manifest(
                "scoped report file must be a unique one-component .json name",
            ));
        }
        if !filters.insert(scoped.filter.clone()) || !files.insert(scoped_file) {
            return Err(invalid_manifest(
                "scoped report filters and files must be unique",
            ));
        }
        let _ = scoped.generated;
    }
    Ok(())
}

fn validate_source_roots(roots: &RawSourceRoots) -> Result<()> {
    if roots.taffy.kind != "taffy"
        || roots.taffy.path.as_str() != HTML_ROOT
        || roots.taffy.upstream_commit.is_none()
    {
        return Err(invalid_manifest(
            "source_roots.taffy must name the pinned Taffy html root",
        ));
    }
    if roots.surgeist.kind != "surgeist"
        || roots.surgeist.path.as_str() != HTML_ROOT
        || roots.surgeist.upstream_commit.is_some()
    {
        return Err(invalid_manifest(
            "source_roots.surgeist must name the unpinned Surgeist html root",
        ));
    }
    require_trimmed_text(&roots.taffy.description, "source_roots.taffy.description")?;
    require_trimmed_text(
        &roots.surgeist.description,
        "source_roots.surgeist.description",
    )
}

fn validate_exclusions(exclusions: Vec<RelativePath>) -> Result<()> {
    if exclusions.len() != EXCLUDED_DIRECTORIES.len() {
        return Err(invalid_manifest(
            "imports.taffy.excluded_destination_dirs must contain grid-lanes and subgrid once each",
        ));
    }
    let actual = exclusions
        .into_iter()
        .map(|path| path.as_str().to_owned())
        .collect::<BTreeSet<_>>();
    let expected = EXCLUDED_DIRECTORIES
        .into_iter()
        .map(str::to_owned)
        .collect::<BTreeSet<_>>();
    if actual != expected || actual.len() != EXCLUDED_DIRECTORIES.len() {
        return Err(invalid_manifest(
            "imports.taffy.excluded_destination_dirs must contain grid-lanes and subgrid once each",
        ));
    }
    Ok(())
}

fn validate_provenance_format(value: &str) -> Result<()> {
    for placeholder in ["{version}", "{repository_relative_executable}"] {
        if value.match_indices(placeholder).count() != 1 {
            return Err(invalid_manifest(
                "browser.provenance_format must contain each canonical placeholder once",
            ));
        }
    }
    let remainder = value
        .replace("{version}", "")
        .replace("{repository_relative_executable}", "");
    if remainder.contains(['{', '}']) {
        return Err(invalid_manifest(
            "browser.provenance_format contains an unknown or unmatched brace",
        ));
    }
    Ok(())
}

fn require_trimmed_text(value: &str, label: &str) -> Result<()> {
    if value.is_empty() || value.trim() != value || value.chars().any(char::is_control) {
        return Err(invalid_manifest(format!(
            "{label} must be nonempty trimmed text"
        )));
    }
    Ok(())
}

fn positive_usize(value: u64, label: &str) -> Result<usize> {
    if value == 0 {
        return Err(invalid_manifest(format!(
            "imports.taffy.{label} must be positive"
        )));
    }
    usize::try_from(value)
        .map_err(|_| invalid_manifest(format!("imports.taffy.{label} overflows usize")))
}

fn nonnegative_usize(value: u64, label: &str) -> Result<usize> {
    usize::try_from(value).map_err(|_| invalid_manifest(format!("{label} overflows usize")))
}

fn reserved_component(component: &str) -> bool {
    component == ".surgeist-generator" || component.starts_with("._surgeist-")
}

pub(super) fn paths_target_equal(left: &str, right: &str) -> bool {
    let mut left = left.split('/');
    let mut right = right.split('/');
    loop {
        match (left.next(), right.next()) {
            (Some(left), Some(right)) if target_component_equal(left, right) => {}
            (None, None) => return true,
            _ => return false,
        }
    }
}

fn target_component_equal(left: &str, right: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        left.eq_ignore_ascii_case(right)
    }
    #[cfg(not(target_os = "macos"))]
    {
        left == right
    }
}

pub(super) fn read_file(path: &Path) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path).map_err(|error| {
        GeneratorError::with_source(
            GeneratorErrorKind::InvalidManifest,
            "read layout corpus manifest",
            path.display().to_string(),
            error,
        )
    })?;
    require_file_metadata(path, &before)?;
    let bytes = fs::read(path).map_err(|error| {
        GeneratorError::with_source(
            GeneratorErrorKind::InvalidManifest,
            "read layout corpus manifest",
            path.display().to_string(),
            error,
        )
    })?;
    let after = fs::symlink_metadata(path).map_err(|error| {
        GeneratorError::with_source(
            GeneratorErrorKind::InvalidManifest,
            "revalidate layout corpus manifest",
            path.display().to_string(),
            error,
        )
    })?;
    require_file_metadata(path, &after)?;
    if !same_file_identity(&before, &after) || before.len() != after.len() {
        return Err(invalid_manifest(
            "manifest identity changed while it was read",
        ));
    }
    Ok(bytes)
}

fn require_file_metadata(path: &Path, metadata: &fs::Metadata) -> Result<()> {
    if !metadata.is_file() {
        return Err(invalid_manifest(format!(
            "manifest is not a regular file: {}",
            path.display()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        if metadata.nlink() != 1 || metadata.permissions().mode() & 0o7777 != CORPUS_FILE_MODE {
            return Err(invalid_manifest(
                "manifest must be a single-link mode-0644 regular file",
            ));
        }
    }
    Ok(())
}

fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(unix))]
    {
        left.len() == right.len() && left.modified().ok() == right.modified().ok()
    }
}

pub(super) fn revalidate(rooted: &RootedFs, expected: &[u8]) -> Result<()> {
    let bytes = rooted
        .read_file(MANIFEST_FILE, CORPUS_FILE_MODE)
        .map_err(|error| {
            GeneratorError::with_source(
                GeneratorErrorKind::InvalidManifest,
                "revalidate layout corpus manifest",
                "held manifest is unavailable",
                error,
            )
        })?;
    if bytes != expected {
        return Err(invalid_manifest("manifest bytes changed after preflight"));
    }
    Ok(())
}

fn invalid_manifest(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidManifest,
        "validate layout corpus manifest",
        detail,
    )
}
