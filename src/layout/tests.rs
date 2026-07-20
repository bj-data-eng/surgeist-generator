use std::path::Path;

use crate::core::{ObjectFormat, SnapshotEntry, VerifiedSourceSnapshot};
use crate::{GeneratorErrorKind, PinnedSource, RelativePath, Sha256Digest, SourceRevision};

use super::{manifest, sidecar};

mod preserved_schema2 {
    use std::collections::BTreeSet;

    use serde::Deserialize;
    use sha2::{Digest, Sha256};

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub(super) struct EffectiveCase {
        pub(super) id: String,
        pub(super) source: String,
        pub(super) status: String,
        pub(super) reason: String,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub(super) struct CompatibilityRecord {
        pub(super) id: String,
        pub(super) source: String,
        pub(super) status: String,
        pub(super) reason: Option<String>,
    }

    #[derive(Clone, Debug, Eq, PartialEq)]
    pub(super) struct Contract {
        pub(super) revision: String,
        pub(super) expected_count: usize,
        pub(super) authored_cases: Vec<EffectiveCase>,
        pub(super) compatibility_records: Vec<CompatibilityRecord>,
        pub(super) launch_digest: String,
        pub(super) effective_launch_keys: BTreeSet<String>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RawManifest {
        schema_version: u64,
        browser: RawBrowser,
        generation_reports: RawGenerationReports,
        source_roots: RawSourceRoots,
        imports: RawImports,
        #[serde(default)]
        cases: Vec<RawCase>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RawBrowser {
        source: String,
        version: String,
        version_output: String,
        cache_root: String,
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
        filter: String,
        file: String,
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
        path: String,
        #[serde(default)]
        upstream_commit: Option<String>,
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
        commit: String,
        source_dir: String,
        destination: String,
        expected_count: u64,
        excluded_destination_dirs: Vec<String>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RawCase {
        id: String,
        source_root: String,
        source: String,
        generator: String,
        status: String,
        reason: Option<String>,
    }

    pub(super) fn parse(text: &str) -> Result<Contract, String> {
        let raw: RawManifest = toml::from_str(text).map_err(|error| error.to_string())?;
        if raw.schema_version != 2 {
            return Err("schema_version must be 2".to_owned());
        }
        validate_browser(&raw.browser)?;
        validate_reports(&raw.generation_reports)?;
        validate_source_roots(&raw.source_roots)?;
        let imported = &raw.imports.taffy;
        if imported.repo != "https://github.com/DioxusLabs/taffy.git"
            || imported.source_dir != "test_fixtures"
            || imported.destination != "html"
            || imported.expected_count == 0
            || !valid_revision(&imported.commit)
        {
            return Err("invalid Taffy import contract".to_owned());
        }
        if raw.source_roots.taffy.upstream_commit.as_deref() != Some(&imported.commit) {
            return Err("Taffy revision fields differ".to_owned());
        }
        if imported
            .excluded_destination_dirs
            .iter()
            .any(|path| !strict_relative(path))
        {
            return Err("invalid legacy exclusion path".to_owned());
        }

        let mut ids = BTreeSet::new();
        let mut sources = BTreeSet::new();
        let mut authored_cases = Vec::new();
        let mut compatibility_records = Vec::new();
        for case in raw.cases {
            if !ids.insert(case.id.clone()) || case.generator != "constrained-html" {
                return Err("invalid or duplicate case identity".to_owned());
            }
            let source = legacy_case_source(&case.source)?;
            if !sources.insert(source.clone()) {
                return Err("duplicate normalized case source".to_owned());
            }
            if !matches!(
                case.status.as_str(),
                "active" | "expected-fail" | "unsupported" | "quarantined"
            ) {
                return Err("invalid case status".to_owned());
            }
            match case.source_root.as_str() {
                "taffy" => compatibility_records.push(CompatibilityRecord {
                    id: case.id,
                    source,
                    status: case.status,
                    reason: case.reason,
                }),
                "surgeist" => {
                    if case.id.is_empty() || case.id.trim() != case.id || !source.ends_with(".html")
                    {
                        return Err("invalid authored case".to_owned());
                    }
                    let reason = if case.status == "active" {
                        String::new()
                    } else {
                        case.reason
                            .unwrap_or_else(|| format!("manifest marks case {}", case.status))
                    };
                    authored_cases.push(EffectiveCase {
                        id: case.id,
                        source,
                        status: case.status,
                        reason,
                    });
                }
                _ => return Err("invalid case source root".to_owned()),
            }
        }
        authored_cases.sort_by(|left, right| left.source.cmp(&right.source));

        let launch_bytes = serde_json::to_vec(&(
            1_u8,
            raw.browser.launch.batch_size,
            raw.browser.launch.navigation_timeout_ms,
            raw.browser.launch.dom_poll_interval_ms,
            raw.browser.launch.retry_count,
            &raw.browser.launch.job_order,
            &raw.browser.launch.retry_error_class,
            &raw.browser.launch.profile_scope,
            &raw.browser.launch.page_scope,
            raw.browser.launch.disable_default_args,
            raw.browser.launch.disable_cache,
            &raw.browser.launch.arguments,
        ))
        .map_err(|error| error.to_string())?;
        let launch_digest = format!("{:x}", Sha256::digest(launch_bytes));
        let effective_launch_keys = raw
            .browser
            .launch
            .arguments
            .iter()
            .map(|argument| {
                let normalized = argument.strip_prefix("--").unwrap_or(argument);
                normalized
                    .split_once('=')
                    .map_or(normalized, |(key, _)| key)
                    .to_owned()
            })
            .collect();
        let expected_count = usize::try_from(imported.expected_count)
            .map_err(|_| "expected_count overflows usize".to_owned())?;
        Ok(Contract {
            revision: imported.commit.clone(),
            expected_count,
            authored_cases,
            compatibility_records,
            launch_digest,
            effective_launch_keys,
        })
    }

    fn validate_browser(browser: &RawBrowser) -> Result<(), String> {
        if browser.source != "chrome-for-testing"
            || !trimmed(&browser.version)
            || !trimmed(&browser.version_output)
            || !strict_relative(&browser.cache_root)
            || browser
                .cache_root
                .split('/')
                .any(|part| part == ".surgeist-generator" || part.starts_with("._surgeist-"))
            || !browser.provenance_format.contains("{version}")
            || !browser
                .provenance_format
                .contains("{repository_relative_executable}")
        {
            return Err("invalid preserved browser contract".to_owned());
        }
        let launch = &browser.launch;
        if launch.batch_size == 0
            || launch.navigation_timeout_ms == 0
            || launch.dom_poll_interval_ms == 0
            || launch.retry_count != 1
            || launch.job_order != "sorted-sequential"
            || launch.retry_error_class != "open-load-reset-timeout"
            || launch.profile_scope != "per-batch-and-retry"
            || launch.page_scope != "per-job"
            || !launch.disable_default_args
            || !launch.disable_cache
            || launch.arguments.len() != 28
            || !launch
                .arguments
                .iter()
                .any(|argument| argument == "use-mock-keychain")
        {
            return Err("invalid preserved launch contract".to_owned());
        }
        Ok(())
    }

    fn validate_reports(reports: &RawGenerationReports) -> Result<(), String> {
        if reports.full.file != "all.json" {
            return Err("invalid full report file".to_owned());
        }
        let _ = (
            reports.full.generated,
            reports.full.unsupported,
            reports.full.expected_fail,
            reports.full.quarantined,
            reports.full.failed_to_generate,
        );
        let mut filters = BTreeSet::new();
        let mut files = BTreeSet::from(["all.json".to_owned()]);
        for scoped in &reports.scoped {
            let _ = scoped.generated;
            if !strict_relative(&scoped.filter)
                || !strict_relative(&scoped.file)
                || scoped.file.contains('/')
                || scoped.file == "all.json"
                || scoped.file == ".json"
                || !scoped.file.ends_with(".json")
                || !filters.insert(scoped.filter.clone())
                || !files.insert(scoped.file.clone())
            {
                return Err("invalid scoped report contract".to_owned());
            }
        }
        Ok(())
    }

    fn validate_source_roots(roots: &RawSourceRoots) -> Result<(), String> {
        if roots.taffy.kind != "taffy"
            || roots.taffy.path != "html"
            || !roots
                .taffy
                .upstream_commit
                .as_deref()
                .is_some_and(valid_revision)
            || !trimmed(&roots.taffy.description)
            || roots.surgeist.kind != "surgeist"
            || roots.surgeist.path != "html"
            || roots.surgeist.upstream_commit.is_some()
            || !trimmed(&roots.surgeist.description)
        {
            return Err("invalid source roots".to_owned());
        }
        Ok(())
    }

    fn valid_revision(value: &str) -> bool {
        matches!(value.len(), 40 | 64)
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    }

    fn trimmed(value: &str) -> bool {
        !value.is_empty() && value.trim() == value && !value.chars().any(char::is_control)
    }

    fn strict_relative(value: &str) -> bool {
        !value.is_empty()
            && !value.starts_with('/')
            && !value.ends_with('/')
            && !value.contains('\\')
            && value
                .split('/')
                .all(|component| !component.is_empty() && !matches!(component, "." | ".."))
    }

    fn legacy_case_source(value: &str) -> Result<String, String> {
        if value.is_empty()
            || value.starts_with('/')
            || value.ends_with('/')
            || value.contains('\\')
        {
            return Err("invalid legacy case source".to_owned());
        }
        let mut components = Vec::new();
        for component in value.split('/') {
            match component {
                "." => {}
                "" | ".." => return Err("invalid legacy case source".to_owned()),
                component => components.push(component),
            }
        }
        if components.is_empty() {
            return Err("invalid legacy case source".to_owned());
        }
        Ok(components.join("/"))
    }
}

pub(super) const SHA1_REVISION: &str = "1111111111111111111111111111111111111111";
pub(super) const SHA256_REVISION: &str =
    "1111111111111111111111111111111111111111111111111111111111111111";

pub(super) fn manifest_text(revision: &str, expected_count: usize, cases: &str) -> String {
    format!(
        r#"schema_version = 2

[browser]
source = "chrome-for-testing"
version = "123.0.1"
version_output = "Chrome for Testing 123.0.1"
cache_root = "browser-cache"
provenance_format = "Chrome {{version}} {{repository_relative_executable}}"

[browser.launch]
batch_size = 4
navigation_timeout_ms = 20000
dom_poll_interval_ms = 25
retry_count = 1
job_order = "sorted-sequential"
retry_error_class = "open-load-reset-timeout"
profile_scope = "per-batch-and-retry"
page_scope = "per-job"
disable_default_args = true
disable_cache = true
arguments = [
  "use-mock-keychain",
  "headless",
  "no-sandbox",
  "disable-setuid-sandbox",
  "disable-gpu",
  "hide-scrollbars",
  "mute-audio",
  "no-first-run",
  "no-default-browser-check",
  "disable-background-networking",
  "disable-background-timer-throttling",
  "disable-client-side-phishing-detection",
  "disable-component-update",
  "disable-default-apps",
  "disable-dev-shm-usage",
  "disable-features=Translate",
  "disable-hang-monitor",
  "disable-popup-blocking",
  "disable-prompt-on-repost",
  "disable-renderer-backgrounding",
  "disable-sync",
  "metrics-recording-only",
  "password-store=basic",
  "safebrowsing-disable-auto-update",
  "enable-automation",
  "force-color-profile=srgb",
  "disable-blink-features=AutomationControlled",
  "window-size=1280,720",
]

[generation_reports.full]
file = "all.json"
generated = 4
unsupported = 0
expected_fail = 0
quarantined = 0
failed_to_generate = 0

[[generation_reports.scoped]]
filter = "grid"
file = "grid.json"
generated = 4

[source_roots.taffy]
kind = "taffy"
path = "html"
upstream_commit = "{revision}"
description = "Pinned upstream Taffy fixtures"

[source_roots.surgeist]
kind = "surgeist"
path = "html"
description = "Surgeist-authored fixtures"

[imports.taffy]
repo = "https://github.com/DioxusLabs/taffy.git"
commit = "{revision}"
source_dir = "test_fixtures"
destination = "html"
expected_count = {expected_count}
excluded_destination_dirs = ["grid-lanes", "subgrid"]

{cases}"#
    )
}

pub(super) fn authored_cases() -> &'static str {
    r#"[[cases]]
id = "authored/first"
source_root = "surgeist"
source = "authored/first.html"
generator = "constrained-html"
status = "active"

[[cases]]
id = "authored/second"
source_root = "surgeist"
source = "authored/second.html"
generator = "constrained-html"
status = "expected-fail"
reason = " known padded reason "

[[cases]]
id = ""
source_root = "taffy"
source = "compatibility/record.any"
generator = "constrained-html"
status = "quarantined"
reason = ""
"#
}

#[test]
fn layout_schema2_full_field_golden_is_accepted() {
    let (parsed, digest) = manifest::parse_with_launch_digest(
        manifest_text(SHA1_REVISION, 3, authored_cases()).as_bytes(),
        Path::new("corpus.toml"),
    )
    .expect("full schema-2 manifest");
    assert_eq!(parsed.revision.as_str(), SHA1_REVISION);
    assert_eq!(parsed.expected_source_files, 3);
    assert_eq!(
        parsed
            .authored_files
            .iter()
            .map(RelativePath::as_str)
            .collect::<Vec<_>>(),
        ["authored/first.html", "authored/second.html"]
    );
    assert_eq!(
        digest.as_str(),
        "1732a55f43798a0e9d8659ce35631291b5bf2bdb8a83faaee195de7ac45d374b"
    );
}

#[test]
fn layout_schema2_accepts_manifest_owned_taffy_pin_and_count() {
    for (revision, count) in [(SHA1_REVISION, 1), (SHA256_REVISION, 17)] {
        let text = manifest_text(revision, count, "");
        let preserved =
            preserved_schema2::parse(&text).expect("preserved manifest-owned pin and count");
        let parsed = manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
            .expect("new manifest-owned pin and count");
        assert_eq!(preserved.revision, revision);
        assert_eq!(preserved.expected_count, count);
        assert_eq!(parsed.revision.as_str(), revision);
        assert_eq!(parsed.expected_source_files, count);
    }
}

#[test]
fn layout_schema2_preserves_taffy_compatibility_records() {
    let text = manifest_text(SHA1_REVISION, 1, authored_cases());
    let preserved = preserved_schema2::parse(&text).expect("preserved compatibility contract");
    let parsed = manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
        .expect("new compatibility representation");
    assert_eq!(preserved.compatibility_records.len(), 1);
    assert_eq!(preserved.compatibility_records[0].id, "");
    assert_eq!(
        preserved.compatibility_records[0].source,
        "compatibility/record.any"
    );
    assert_eq!(preserved.compatibility_records[0].status, "quarantined");
    assert_eq!(
        preserved.compatibility_records[0].reason.as_deref(),
        Some("")
    );
    assert_eq!(parsed.authored_files.len(), 2);
    assert_eq!(parsed.authored_cases.len(), 2);
}

#[test]
fn layout_schema2_preserves_raw_ids_and_reason_defaults() {
    let cases = r#"[[cases]]
id = "raw-active"
source_root = "surgeist"
source = "authored/active.html"
generator = "constrained-html"
status = "active"
reason = "ignored active reason"

[[cases]]
id = "raw-expected"
source_root = "surgeist"
source = "authored/expected.html"
generator = "constrained-html"
status = "expected-fail"

[[cases]]
id = "raw-unsupported"
source_root = "surgeist"
source = "authored/unsupported.html"
generator = "constrained-html"
status = "unsupported"
reason = ""

[[cases]]
id = "raw-quarantined"
source_root = "surgeist"
source = "authored/quarantined.html"
generator = "constrained-html"
status = "quarantined"
reason = " padded reason "
"#;
    let text = manifest_text(SHA1_REVISION, 1, cases);
    let preserved = preserved_schema2::parse(&text).expect("preserved reason contract");
    let parsed = manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
        .expect("new reason representation");
    let actual = parsed
        .authored_cases
        .iter()
        .map(|case| preserved_schema2::EffectiveCase {
            id: case.id.clone(),
            source: case.source.as_str().to_owned(),
            status: match case.status {
                super::case::LayoutCaseStatus::Active => "active",
                super::case::LayoutCaseStatus::ExpectedFail => "expected-fail",
                super::case::LayoutCaseStatus::Unsupported => "unsupported",
                super::case::LayoutCaseStatus::Quarantined => "quarantined",
            }
            .to_owned(),
            reason: case.reason.clone(),
        })
        .collect::<Vec<_>>();
    assert_eq!(actual, preserved.authored_cases);
    assert_eq!(actual[0].reason, "");
    assert_eq!(actual[1].reason, "manifest marks case expected-fail");
    assert_eq!(actual[2].reason, " padded reason ");
    assert_eq!(actual[3].reason, "");
}

#[test]
fn layout_schema2_rejects_mismatched_taffy_revision_fields() {
    let text = manifest_text(SHA1_REVISION, 1, "").replacen(
        &format!("commit = \"{SHA1_REVISION}\""),
        &format!("commit = \"{}\"", "2".repeat(40)),
        1,
    );
    assert!(preserved_schema2::parse(&text).is_err());
    let error = manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
        .expect_err("mismatched revisions");
    assert_eq!(error.kind(), GeneratorErrorKind::InvalidManifest);
}

#[test]
fn layout_schema2_rejects_noncanonical_retry_count() {
    let text = manifest_text(SHA1_REVISION, 1, "").replace("retry_count = 1", "retry_count = 2");
    let error = manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
        .expect_err("noncanonical retry count");
    assert_eq!(error.kind(), GeneratorErrorKind::InvalidManifest);
}

#[test]
fn layout_schema2_rejects_html_source_root() {
    let cases =
        authored_cases().replacen("source_root = \"surgeist\"", "source_root = \"html\"", 1);
    let text = manifest_text(SHA1_REVISION, 1, &cases);
    assert!(preserved_schema2::parse(&text).is_err());
    let error = manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
        .expect_err("invalid source root");
    assert_eq!(error.kind(), GeneratorErrorKind::InvalidManifest);
}

#[test]
fn layout_schema2_rejects_duplicate_ids_and_sources() {
    let base = authored_cases();
    for cases in [
        format!(
            "{base}\n[[cases]]\nid = \"authored/first\"\nsource_root = \"taffy\"\nsource = \"other.html\"\ngenerator = \"constrained-html\"\nstatus = \"active\"\n"
        ),
        format!(
            "{base}\n[[cases]]\nid = \"other\"\nsource_root = \"taffy\"\nsource = \"authored/first.html\"\ngenerator = \"constrained-html\"\nstatus = \"active\"\n"
        ),
    ] {
        let text = manifest_text(SHA1_REVISION, 1, &cases);
        assert!(preserved_schema2::parse(&text).is_err());
        let error = manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
            .expect_err("duplicate case identity");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidManifest);
    }
}

#[test]
fn layout_schema2_rejects_only_declared_tightenings() {
    let valid = manifest_text(SHA1_REVISION, 1, authored_cases());
    let declared_divergences = [
        (
            "strict case source",
            valid.replace(
                "source = \"authored/first.html\"",
                "source = \"authored/./first.html\"",
            ),
        ),
        (
            "exact exclusion set",
            valid.replace(
                "excluded_destination_dirs = [\"grid-lanes\", \"subgrid\"]",
                "excluded_destination_dirs = [\"grid-lanes\", \"grid-lanes\"]",
            ),
        ),
        (
            "exact exclusion set",
            valid.replace(
                "excluded_destination_dirs = [\"grid-lanes\", \"subgrid\"]",
                "excluded_destination_dirs = [\"grid-lanes\", \"grid-lanes\", \"subgrid\"]",
            ),
        ),
        (
            "normalized launch switch set",
            valid.replace("  \"headless\",", "  \"--use-mock-keychain\","),
        ),
        (
            "normalized launch switch set",
            valid.replace("  \"headless\",", "  \"use-mock-keychain=value\","),
        ),
        (
            "normalized launch switch set",
            valid.replace("  \"headless\",", "  \"path/bearing\","),
        ),
        (
            "normalized launch switch set",
            valid.replace("  \"headless\",", "  \"remote-debugging-port=1\","),
        ),
        (
            "normalized launch switch set",
            valid.replace("  \"headless\",", "  \"---headless\","),
        ),
        (
            "normalized launch switch set",
            valid.replace("  \"headless\",", "  \"headless\\u007f\","),
        ),
        (
            "unambiguous provenance placeholders",
            valid.replace(
                "Chrome {version} {repository_relative_executable}",
                "Chrome {version} {version} {repository_relative_executable}",
            ),
        ),
        (
            "unambiguous provenance placeholders",
            valid.replace(
                "Chrome {version} {repository_relative_executable}",
                "Chrome {version} {repository_relative_executable} {unknown}",
            ),
        ),
    ];
    for (tightening, text) in declared_divergences {
        preserved_schema2::parse(&text)
            .unwrap_or_else(|error| panic!("preserved contract rejected {tightening}: {error}"));
        let error = manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
            .expect_err("declared tightening");
        assert_eq!(
            error.kind(),
            GeneratorErrorKind::InvalidManifest,
            "{tightening}"
        );
    }

    for compatible in [
        valid.replace(
            "excluded_destination_dirs = [\"grid-lanes\", \"subgrid\"]",
            "excluded_destination_dirs = [\"subgrid\", \"grid-lanes\"]",
        ),
        valid.replace(
            "  \"headless\",\n  \"no-sandbox\",",
            "  \"no-sandbox\",\n  \"headless\",",
        ),
        manifest_text(SHA256_REVISION, 17, authored_cases()),
    ] {
        preserved_schema2::parse(&compatible).expect("preserved compatible manifest");
        manifest::parse(compatible.as_bytes(), Path::new("corpus.toml"))
            .expect("new compatible manifest");
    }
}

#[test]
fn layout_schema2_rejects_three_exclusions_with_duplicate_member() {
    let text = manifest_text(SHA1_REVISION, 1, "").replace(
        "excluded_destination_dirs = [\"grid-lanes\", \"subgrid\"]",
        "excluded_destination_dirs = [\"grid-lanes\", \"grid-lanes\", \"subgrid\"]",
    );
    let error = manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
        .expect_err("three-entry exclusion list with duplicate member");
    assert_eq!(error.kind(), GeneratorErrorKind::InvalidManifest);
}

#[test]
fn layout_schema2_rejects_noncanonical_scoped_report_files() {
    for replacement in [r#"file = "dir\\name.json""#, r#"file = " scoped.json""#] {
        let text =
            manifest_text(SHA1_REVISION, 1, "").replacen(r#"file = "grid.json""#, replacement, 1);
        let error = manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
            .expect_err("noncanonical scoped report file");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidManifest);
    }
}

#[test]
fn layout_schema2_launch_digest_preserves_manifest_order() {
    let original = manifest_text(SHA1_REVISION, 1, "");
    let reordered = original.replace(
        "  \"headless\",\n  \"no-sandbox\",",
        "  \"no-sandbox\",\n  \"headless\",",
    );
    let preserved_original = preserved_schema2::parse(&original).expect("preserved original");
    let preserved_reordered = preserved_schema2::parse(&reordered).expect("preserved reordered");
    let (_, original_digest) =
        manifest::parse_with_launch_digest(original.as_bytes(), Path::new("corpus.toml"))
            .expect("original launch");
    let (_, reordered_digest) =
        manifest::parse_with_launch_digest(reordered.as_bytes(), Path::new("corpus.toml"))
            .expect("reordered launch");
    assert_eq!(preserved_original.launch_digest, original_digest.as_str());
    assert_eq!(preserved_reordered.launch_digest, reordered_digest.as_str());
    assert_ne!(
        preserved_original.launch_digest,
        preserved_reordered.launch_digest
    );
    assert_ne!(original_digest, reordered_digest);
}

#[test]
fn layout_schema2_launch_switch_set_is_order_independent() {
    let original = manifest_text(SHA1_REVISION, 1, "");
    let reordered = original.replace(
        "  \"headless\",\n  \"no-sandbox\",",
        "  \"no-sandbox\",\n  \"headless\",",
    );
    let preserved_original = preserved_schema2::parse(&original).expect("preserved original");
    let preserved_reordered = preserved_schema2::parse(&reordered).expect("preserved reordered");
    let parsed_original = manifest::parse(original.as_bytes(), Path::new("corpus.toml"))
        .expect("new original representation");
    let parsed_reordered = manifest::parse(reordered.as_bytes(), Path::new("corpus.toml"))
        .expect("new reordered representation");
    assert_eq!(
        preserved_original.effective_launch_keys,
        preserved_reordered.effective_launch_keys
    );
    assert_eq!(
        parsed_original.effective_launch_keys,
        preserved_original.effective_launch_keys
    );
    assert_eq!(
        parsed_reordered.effective_launch_keys,
        preserved_reordered.effective_launch_keys
    );
}

fn snapshot(object_format: ObjectFormat, object_id: &str) -> VerifiedSourceSnapshot {
    let bytes = b"<div>fixture</div>\n".to_vec();
    VerifiedSourceSnapshot {
        object_format,
        entries: vec![SnapshotEntry {
            path: RelativePath::new("grid/basic.html").expect("fixture path"),
            git_mode: "100644".to_owned(),
            blob_object_id: object_id.to_owned(),
            digest: Sha256Digest::from_bytes(&bytes),
            bytes,
        }],
    }
}

fn pin(revision: &str) -> PinnedSource {
    PinnedSource::new(
        "taffy",
        manifest::TAFFY_REPOSITORY,
        SourceRevision::new(revision).expect("source revision"),
        RelativePath::new(manifest::TAFFY_SOURCE_DIRECTORY).expect("fixture root"),
    )
    .expect("Taffy pin")
}

#[test]
fn layout_taffy_sidecar_sha1_golden() {
    let snapshot = snapshot(
        ObjectFormat::Sha1,
        "2222222222222222222222222222222222222222",
    );
    let digest = snapshot.entries[0].digest.as_str();
    let expected = format!(
        "{{\"schema_version\":1,\"source\":{{\"label\":\"taffy\",\"repository_url\":\"{}\",\"revision\":\"{SHA1_REVISION}\",\"source_subdirectory\":\"test_fixtures\"}},\"object_format\":\"sha1\",\"source_file_count\":3,\"excluded_destination_dirs\":[\"grid-lanes\",\"subgrid\"],\"imported_file_count\":1,\"files\":[{{\"path\":\"grid/basic.html\",\"git_mode\":\"100644\",\"blob_object_id\":\"2222222222222222222222222222222222222222\",\"sha256\":\"{digest}\"}}]}}\n",
        manifest::TAFFY_REPOSITORY
    );
    assert_eq!(
        sidecar::canonical_bytes(&pin(SHA1_REVISION), 3, &snapshot).expect("SHA-1 sidecar"),
        expected.as_bytes()
    );
}

#[test]
fn layout_taffy_sidecar_sha256_golden() {
    let snapshot = snapshot(
        ObjectFormat::Sha256,
        "2222222222222222222222222222222222222222222222222222222222222222",
    );
    let bytes =
        sidecar::canonical_bytes(&pin(SHA256_REVISION), 1, &snapshot).expect("SHA-256 sidecar");
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("sidecar JSON");
    assert_eq!(value["object_format"], "sha256");
    assert_eq!(value["source"]["revision"].as_str().unwrap().len(), 64);
    assert_eq!(
        value["files"][0]["blob_object_id"].as_str().unwrap().len(),
        64
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod imports {
    use std::collections::{BTreeMap, BTreeSet};
    use std::ffi::{OsStr, OsString};
    use std::fs;
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::core::{Domain, GenerationLease};
    use crate::layout::LayoutRequest;
    use crate::{CorpusLocation, GeneratorErrorKind, RunScope};

    use super::{manifest, manifest_text};
    use crate::layout::importer;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        corpus: PathBuf,
        source: PathBuf,
        location: CorpusLocation,
        revision: String,
        authored: Vec<(&'static str, &'static [u8])>,
        source_files: Vec<(&'static str, &'static [u8], bool)>,
    }

    impl Fixture {
        fn new(source_files: Vec<(&'static str, &'static [u8], bool)>) -> Self {
            Self::new_with_object_format(source_files, false)
        }

        fn new_sha256(source_files: Vec<(&'static str, &'static [u8], bool)>) -> Self {
            Self::new_with_object_format(source_files, true)
        }

        fn new_with_object_format(
            source_files: Vec<(&'static str, &'static [u8], bool)>,
            sha256: bool,
        ) -> Self {
            let root = std::env::temp_dir().join(format!(
                "surgeist-generator-layout-import-{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            let owner = root.join("owner");
            let corpus = owner.join("corpus");
            let source = root.join("checkout");
            fs::create_dir_all(&corpus).expect("create corpus");
            fs::create_dir(&source).expect("create source root");
            if sha256 {
                run_git(
                    &source,
                    &[
                        OsStr::new("init"),
                        OsStr::new("--quiet"),
                        OsStr::new("--object-format=sha256"),
                    ],
                );
            } else {
                run_git(&source, &[OsStr::new("init"), OsStr::new("--quiet")]);
            }
            for (key, value) in [
                ("user.name", "Layout Test"),
                ("user.email", "layout@example.invalid"),
            ] {
                run_git(
                    &source,
                    &[OsStr::new("config"), OsStr::new(key), OsStr::new(value)],
                );
            }
            run_git(
                &source,
                &[
                    OsStr::new("remote"),
                    OsStr::new("add"),
                    OsStr::new("origin"),
                    OsStr::new(manifest::TAFFY_REPOSITORY),
                ],
            );
            write_source_files(&source, &source_files);
            run_git(
                &source,
                &[
                    OsStr::new("add"),
                    OsStr::new(manifest::TAFFY_SOURCE_DIRECTORY),
                ],
            );
            run_git(
                &source,
                &[
                    OsStr::new("commit"),
                    OsStr::new("--quiet"),
                    OsStr::new("-m"),
                    OsStr::new("fixtures"),
                ],
            );
            let revision = run_git(&source, &[OsStr::new("rev-parse"), OsStr::new("HEAD")]);
            let authored = vec![
                ("authored/first.html", b"<p>authored first</p>\n".as_slice()),
                (
                    "authored/second.html",
                    b"<p>authored second</p>\n".as_slice(),
                ),
            ];
            let fixture = Self {
                root,
                corpus,
                source,
                location: CorpusLocation::new(&owner, owner.join("corpus"))
                    .expect("corpus location"),
                revision,
                authored,
                source_files,
            };
            fixture.seed_authored();
            fixture.write_manifest();
            fixture
        }

        fn request(&self) -> LayoutRequest {
            LayoutRequest::import_taffy(self.location.clone(), self.source.clone())
                .expect("import request")
        }

        fn import(&self) -> crate::Result<()> {
            crate::layout::run(self.request())
        }

        fn check_request(&self) -> LayoutRequest {
            LayoutRequest::check_taffy_corpus(self.location.clone(), self.source.clone())
                .expect("Taffy check request")
        }

        fn check(&self) -> crate::Result<()> {
            crate::layout::run(self.check_request())
        }

        fn seed_authored(&self) {
            for (relative, bytes) in &self.authored {
                write_corpus_file(&self.corpus.join("html").join(relative), bytes);
            }
        }

        fn expected_count(&self) -> usize {
            self.source_files
                .iter()
                .filter(|(path, _, _)| Path::new(path).extension() == Some(OsStr::new("html")))
                .count()
        }

        fn write_manifest(&self) {
            write_corpus_file(
                &self.corpus.join("corpus.toml"),
                manifest_text(
                    &self.revision,
                    self.expected_count(),
                    super::authored_cases(),
                )
                .as_bytes(),
            );
        }

        fn replace_source(&mut self, files: Vec<(&'static str, &'static [u8], bool)>) {
            fs::remove_dir_all(self.source.join(manifest::TAFFY_SOURCE_DIRECTORY))
                .expect("remove old source fixtures");
            write_source_files(&self.source, &files);
            run_git(&self.source, &[OsStr::new("add"), OsStr::new("--all")]);
            run_git(
                &self.source,
                &[
                    OsStr::new("commit"),
                    OsStr::new("--quiet"),
                    OsStr::new("-m"),
                    OsStr::new("replace fixtures"),
                ],
            );
            self.revision = run_git(&self.source, &[OsStr::new("rev-parse"), OsStr::new("HEAD")]);
            self.source_files = files;
            self.write_manifest();
        }

        fn move_source_to_non_utf8_checkout_root(&mut self) -> bool {
            let destination = self
                .root
                .join(OsString::from_vec(b"checkout-native-\xff".to_vec()));
            match fs::rename(&self.source, &destination) {
                Ok(()) => {}
                Err(error) => {
                    // APFS rejects invalid UTF-8 names before a real checkout can
                    // be created; each caller test still proves domain forwarding.
                    assert_eq!(
                        error.raw_os_error(),
                        Some(92),
                        "unexpected failure creating non-UTF-8 checkout root: {error}"
                    );
                    self.source = destination;
                    return false;
                }
            }
            self.source = destination;
            assert!(self.source.to_str().is_none());
            assert_eq!(
                self.source
                    .file_name()
                    .expect("native checkout file name")
                    .as_bytes(),
                b"checkout-native-\xff"
            );
            true
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).expect("remove layout fixture");
        }
    }

    fn run_git(directory: &Path, arguments: &[&OsStr]) -> String {
        let output = Command::new("/usr/bin/git")
            .arg("-C")
            .arg(directory)
            .args(arguments)
            .output()
            .expect("run installed Git");
        assert!(
            output.status.success(),
            "Git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("UTF-8 Git output")
            .trim_end()
            .to_owned()
    }

    fn write_source_files(root: &Path, files: &[(&str, &[u8], bool)]) {
        for (relative, bytes, executable) in files {
            let path = root.join(manifest::TAFFY_SOURCE_DIRECTORY).join(relative);
            fs::create_dir_all(path.parent().expect("source parent"))
                .expect("create source parent");
            fs::write(&path, bytes).expect("write source fixture");
            if *executable {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
                    .expect("set executable mode");
            }
        }
    }

    fn write_corpus_file(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().expect("corpus parent")).expect("create corpus parent");
        fs::write(path, bytes).expect("write corpus file");
        fs::set_permissions(path, fs::Permissions::from_mode(0o644)).expect("set corpus file mode");
        let mut parent = path.parent().expect("corpus parent");
        while parent.file_name().is_some() {
            fs::set_permissions(parent, fs::Permissions::from_mode(0o755))
                .expect("set corpus directory mode");
            parent = parent.parent().expect("parent");
            if parent.ends_with("owner") {
                break;
            }
        }
    }

    fn snapshot_tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        fn visit(base: &Path, current: &Path, output: &mut BTreeMap<PathBuf, Vec<u8>>) {
            let mut entries = fs::read_dir(current)
                .expect("read snapshot directory")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("snapshot entries");
            entries.sort_by_key(fs::DirEntry::file_name);
            for entry in entries {
                let path = entry.path();
                let relative = path.strip_prefix(base).expect("relative snapshot path");
                if entry.file_type().expect("file type").is_dir() {
                    visit(base, &path, output);
                } else {
                    output.insert(
                        relative.to_path_buf(),
                        fs::read(path).expect("snapshot file"),
                    );
                }
            }
        }
        let mut output = BTreeMap::new();
        if root.exists() {
            visit(root, root, &mut output);
        }
        output
    }

    fn path_identity(path: &Path) -> (u64, u64) {
        let metadata = fs::symlink_metadata(path).expect("path identity");
        (metadata.dev(), metadata.ino())
    }

    fn snapshot_path_identities(root: &Path) -> BTreeMap<PathBuf, (u64, u64)> {
        fn visit(base: &Path, current: &Path, output: &mut BTreeMap<PathBuf, (u64, u64)>) {
            let metadata = fs::symlink_metadata(current).expect("snapshot path identity");
            output.insert(
                current
                    .strip_prefix(base)
                    .expect("relative snapshot identity path")
                    .to_path_buf(),
                (metadata.dev(), metadata.ino()),
            );
            if metadata.is_dir() {
                let mut entries = fs::read_dir(current)
                    .expect("read identity snapshot directory")
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .expect("identity snapshot entries");
                entries.sort_by_key(fs::DirEntry::file_name);
                for entry in entries {
                    visit(base, &entry.path(), output);
                }
            }
        }

        let mut output = BTreeMap::new();
        visit(root, root, &mut output);
        output
    }

    fn assert_check_preserves(fixture: &Fixture, expected: Option<GeneratorErrorKind>) {
        let before_bytes = snapshot_tree(&fixture.root);
        let before_identities = snapshot_path_identities(&fixture.root);
        let before_root_identity = path_identity(&fixture.root);

        let result = fixture.check();
        match expected {
            Some(kind) => {
                let error = result.expect_err("Taffy corpus check must reject state");
                assert_eq!(error.kind(), kind, "unexpected check diagnostic: {error}");
            }
            None => result.expect("Taffy corpus check must accept current state"),
        }

        assert_eq!(snapshot_tree(&fixture.root), before_bytes);
        assert_eq!(snapshot_path_identities(&fixture.root), before_identities);
        assert_eq!(path_identity(&fixture.root), before_root_identity);
    }

    fn sidecar_paths(fixture: &Fixture) -> BTreeSet<String> {
        let bytes = fs::read(fixture.corpus.join("html/.surgeist-taffy-source.json"))
            .expect("read Taffy sidecar");
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("sidecar JSON");
        value["files"]
            .as_array()
            .expect("sidecar files")
            .iter()
            .map(|file| file["path"].as_str().expect("sidecar path").to_owned())
            .collect()
    }

    #[test]
    fn layout_import_taffy_accepts_non_utf8_checkout_root_and_preserves_source_bytes() {
        let mut fixture = Fixture::new(vec![(
            "grid/native.html",
            b"<div>native checkout fixture</div>\n",
            false,
        )]);
        if !fixture.move_source_to_non_utf8_checkout_root() {
            let before_bytes = snapshot_tree(&fixture.root);
            let before_identities = snapshot_path_identities(&fixture.root);
            let error = fixture
                .import()
                .expect_err("unsupported filesystem still forwards native checkout bytes");
            assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);
            assert!(
                error
                    .to_string()
                    .starts_with("resolve caller checkout root:"),
                "native checkout was rejected before OS canonicalization: {error}"
            );
            assert_eq!(snapshot_tree(&fixture.root), before_bytes);
            assert_eq!(snapshot_path_identities(&fixture.root), before_identities);
            return;
        }
        let source_bytes = snapshot_tree(&fixture.source);
        let source_identities = snapshot_path_identities(&fixture.source);
        let source_root_identity = path_identity(&fixture.source);
        let authored = fixture
            .authored
            .iter()
            .map(|(path, bytes)| ((*path).to_owned(), bytes.to_vec()))
            .collect::<BTreeMap<_, _>>();

        fixture
            .import()
            .expect("import from valid non-UTF-8 checkout root");

        assert_eq!(snapshot_tree(&fixture.source), source_bytes);
        assert_eq!(snapshot_path_identities(&fixture.source), source_identities);
        assert_eq!(path_identity(&fixture.source), source_root_identity);
        assert_eq!(
            fs::read(fixture.corpus.join("html/grid/native.html"))
                .expect("read imported native fixture"),
            b"<div>native checkout fixture</div>\n"
        );
        for (path, expected) in authored {
            assert_eq!(
                fs::read(fixture.corpus.join("html").join(path))
                    .expect("read preserved authored fixture"),
                expected
            );
        }
        assert_eq!(
            sidecar_paths(&fixture),
            BTreeSet::from(["grid/native.html".to_owned()])
        );
    }

    #[test]
    fn layout_check_taffy_accepts_non_utf8_checkout_root_and_is_read_only() {
        let mut fixture = Fixture::new(vec![(
            "grid/native.html",
            b"<div>native checkout fixture</div>\n",
            false,
        )]);
        fixture.import().expect("seed current Taffy import");
        let native_checkout_supported = fixture.move_source_to_non_utf8_checkout_root();
        let sidecar = fixture.corpus.join("html/.surgeist-taffy-source.json");
        let fixture_path = fixture.corpus.join("html/grid/native.html");
        let sidecar_identity = path_identity(&sidecar);
        let fixture_identity = path_identity(&fixture_path);
        let before_bytes = snapshot_tree(&fixture.root);
        let before_identities = snapshot_path_identities(&fixture.root);
        let before_root_identity = path_identity(&fixture.root);

        let result = fixture.check();
        if native_checkout_supported {
            result.expect("check valid non-UTF-8 checkout root read-only");
        } else {
            let error = result.expect_err("unsupported filesystem forwards native checkout bytes");
            assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);
            assert!(
                error
                    .to_string()
                    .starts_with("resolve caller checkout root:"),
                "native checkout was rejected before OS canonicalization: {error}"
            );
        }

        assert_eq!(snapshot_tree(&fixture.root), before_bytes);
        assert_eq!(snapshot_path_identities(&fixture.root), before_identities);
        assert_eq!(path_identity(&fixture.root), before_root_identity);
        assert_eq!(path_identity(&sidecar), sidecar_identity);
        assert_eq!(path_identity(&fixture_path), fixture_identity);
    }

    #[test]
    fn layout_taffy_legacy_nonempty_migration_adds_sidecar_and_preserves_authored_and_downstream() {
        let fixture = Fixture::new(vec![
            ("grid/current.html", b"<div>current</div>\n", false),
            ("nested/missing.html", b"<div>missing</div>\n", false),
            ("grid-lanes/excluded.html", b"excluded\n", false),
            ("notes.txt", b"not imported\n", false),
        ]);
        write_corpus_file(
            &fixture.corpus.join("html/grid/current.html"),
            b"<div>stale</div>\n",
        );
        write_corpus_file(&fixture.corpus.join("xml/sentinel.xml"), b"xml sentinel\n");
        let downstream_before = snapshot_tree(&fixture.corpus.join("xml"));
        let downstream_identity = path_identity(&fixture.corpus.join("xml"));
        let authored_before = fixture
            .authored
            .iter()
            .map(|(path, _)| {
                (
                    *path,
                    fs::read(fixture.corpus.join("html").join(path)).expect("authored bytes"),
                )
            })
            .collect::<Vec<_>>();

        fixture.import().expect("partition-safe import");

        assert_eq!(
            fs::read(fixture.corpus.join("html/grid/current.html")).unwrap(),
            b"<div>current</div>\n"
        );
        assert_eq!(
            fs::read(fixture.corpus.join("html/nested/missing.html")).unwrap(),
            b"<div>missing</div>\n"
        );
        assert!(
            !fixture
                .corpus
                .join("html/grid-lanes/excluded.html")
                .exists()
        );
        assert_eq!(
            sidecar_paths(&fixture),
            BTreeSet::from([
                "grid/current.html".to_owned(),
                "nested/missing.html".to_owned()
            ])
        );
        for (path, bytes) in authored_before {
            assert_eq!(
                fs::read(fixture.corpus.join("html").join(path)).unwrap(),
                bytes
            );
        }
        assert_eq!(
            snapshot_tree(&fixture.corpus.join("xml")),
            downstream_before
        );
        assert_eq!(
            path_identity(&fixture.corpus.join("xml")),
            downstream_identity
        );
    }

    #[test]
    fn layout_taffy_sidecar_mode_removes_only_classified_stale_taffy_files() {
        let mut fixture = Fixture::new(vec![
            ("old/removed.html", b"old\n", false),
            ("same/changed.html", b"before\n", false),
        ]);
        fixture.import().expect("initial import");
        fixture.replace_source(vec![
            ("new/added.html", b"new\n", false),
            ("same/changed.html", b"after\n", false),
        ]);
        fixture.import().expect("updated import");

        assert!(!fixture.corpus.join("html/old/removed.html").exists());
        assert_eq!(
            fs::read(fixture.corpus.join("html/new/added.html")).unwrap(),
            b"new\n"
        );
        assert_eq!(
            fs::read(fixture.corpus.join("html/same/changed.html")).unwrap(),
            b"after\n"
        );
        assert_eq!(
            sidecar_paths(&fixture),
            BTreeSet::from(["new/added.html".to_owned(), "same/changed.html".to_owned()])
        );
    }

    #[test]
    fn layout_taffy_legacy_unknown_file_is_not_guessed() {
        let fixture = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        write_corpus_file(&fixture.corpus.join("html/unknown.html"), b"unknown\n");
        let before = snapshot_tree(&fixture.corpus.join("html"));
        let error = fixture.import().expect_err("unknown legacy file");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert_eq!(snapshot_tree(&fixture.corpus.join("html")), before);
    }

    #[test]
    fn layout_taffy_authored_destination_collision_is_rejected() {
        let fixture = Fixture::new(vec![(
            "authored/first.html",
            b"upstream collision\n",
            false,
        )]);
        let before = snapshot_tree(&fixture.corpus.join("html"));
        let error = fixture.import().expect_err("authored/Taffy collision");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert_eq!(snapshot_tree(&fixture.corpus.join("html")), before);
    }

    #[test]
    fn layout_taffy_malformed_sidecar_never_falls_back() {
        let fixture = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        write_corpus_file(
            &fixture.corpus.join("html/.surgeist-taffy-source.json"),
            b"{}\n",
        );
        write_corpus_file(
            &fixture.corpus.join("html/grid/current.html"),
            b"legacy bytes\n",
        );
        let before = snapshot_tree(&fixture.corpus.join("html"));
        let error = fixture.import().expect_err("malformed sidecar");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert_eq!(snapshot_tree(&fixture.corpus.join("html")), before);
    }

    #[test]
    fn layout_import_unknown_inventory_precedes_dirty_source_verification() {
        let fixture = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        fixture.import().expect("seed sidecar-owned import");
        write_corpus_file(&fixture.corpus.join("html/unknown.html"), b"unknown\n");
        fs::write(
            fixture.source.join("test_fixtures/grid/current.html"),
            b"dirty source bytes\n",
        )
        .expect("drift source snapshot");
        let before = snapshot_tree(&fixture.corpus.join("html"));

        let error = fixture
            .import()
            .expect_err("unknown current inventory must precede source drift");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert_eq!(
            error.to_string(),
            "validate layout Taffy import inventory: unknown entry in layout HTML root: unknown.html"
        );
        assert_eq!(snapshot_tree(&fixture.corpus.join("html")), before);
    }

    #[test]
    fn layout_taffy_source_pin_and_snapshot_drift_are_source_verification() {
        let fixture = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        fs::write(
            fixture.source.join("test_fixtures/grid/current.html"),
            b"dirty\n",
        )
        .expect("dirty source fixture");
        let error = fixture.import().expect_err("dirty source snapshot");
        assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
        assert!(
            !fixture
                .corpus
                .join("html/.surgeist-taffy-source.json")
                .exists()
        );
    }

    #[test]
    fn layout_taffy_source_replacement_after_preflight_fails_before_intent() {
        let fixture = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        let original = fixture.source.clone();
        let moved = fixture.root.join("moved-checkout");
        let request = fixture.request();
        let error = importer::run_with_pre_lease_hook(&request, || {
            fs::rename(&original, &moved).expect("move verified source");
            fs::create_dir(&original).expect("replace source name");
        })
        .expect_err("source replacement");
        assert!(matches!(
            error.kind(),
            GeneratorErrorKind::SourceVerification | GeneratorErrorKind::InvalidPath
        ));
        assert!(
            !fixture
                .corpus
                .join("html/.surgeist-taffy-source.json")
                .exists()
        );
    }

    #[test]
    fn layout_taffy_authored_revalidation_precedes_import_intent() {
        let fixture = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        let authored = fixture.corpus.join("html/authored/first.html");
        let request = fixture.request();
        let error = importer::run_with_pre_lease_hook(&request, || {
            fs::remove_file(&authored).expect("remove authored path");
            write_corpus_file(&authored, b"replacement\n");
        })
        .expect_err("authored replacement");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert!(
            !fixture
                .corpus
                .join("html/.surgeist-taffy-source.json")
                .exists()
        );
    }

    #[test]
    fn layout_taffy_wrong_mode_link_and_count_fail_before_publication() {
        let executable = Fixture::new(vec![("grid/current.html", b"current\n", true)]);
        assert_eq!(
            executable.import().unwrap_err().kind(),
            GeneratorErrorKind::InvalidInventory
        );

        let linked = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        let link = linked.source.join("test_fixtures/grid/link.html");
        symlink("current.html", &link).expect("create source symlink");
        run_git(&linked.source, &[OsStr::new("add"), OsStr::new("--all")]);
        run_git(
            &linked.source,
            &[
                OsStr::new("commit"),
                OsStr::new("--quiet"),
                OsStr::new("-m"),
                OsStr::new("link"),
            ],
        );
        let revision = run_git(
            &linked.source,
            &[OsStr::new("rev-parse"), OsStr::new("HEAD")],
        );
        write_corpus_file(
            &linked.corpus.join("corpus.toml"),
            manifest_text(&revision, 2, super::authored_cases()).as_bytes(),
        );
        assert_eq!(
            linked.import().unwrap_err().kind(),
            GeneratorErrorKind::InvalidInventory
        );

        let count = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        write_corpus_file(
            &count.corpus.join("corpus.toml"),
            manifest_text(&count.revision, 2, super::authored_cases()).as_bytes(),
        );
        assert_eq!(
            count.import().unwrap_err().kind(),
            GeneratorErrorKind::InvalidInventory
        );
    }

    #[test]
    fn layout_taffy_exclusion_alias_is_rejected_before_publication() {
        let aliased = Fixture::new(vec![("Grid-Lanes/aliased.html", b"alias\n", false)]);
        let error = aliased.import().expect_err("target-aliased exclusion");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert!(
            !aliased
                .corpus
                .join("html/.surgeist-taffy-source.json")
                .exists()
        );
    }

    #[test]
    fn layout_taffy_closing_inventory_revalidation_rejects_unknown_entry() {
        let fixture = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        let unknown = fixture.corpus.join("html/unknown.html");
        let request = fixture.request();
        let error = importer::run_with_inter_scan_hook(&request, || {
            write_corpus_file(&unknown, b"raced unknown\n");
        })
        .expect_err("inventory race");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert!(
            !fixture
                .corpus
                .join("html/.surgeist-taffy-source.json")
                .exists()
        );
    }

    #[test]
    fn layout_check_taffy_matching_sha1_and_sha256_imports_are_read_only() {
        for fixture in [
            Fixture::new(vec![("grid/sha1.html", b"sha1\n", false)]),
            Fixture::new_sha256(vec![("grid/sha256.html", b"sha256\n", false)]),
        ] {
            fixture.import().expect("seed current Taffy import");
            fs::remove_dir_all(fixture.corpus.join(".surgeist-generator"))
                .expect("remove completed coordination before read-only check");
            fs::write(
                fixture.root.join("outside-sentinel"),
                b"outside bytes remain unchanged\n",
            )
            .expect("write outside sentinel");

            assert_check_preserves(&fixture, None);
            assert!(!fixture.corpus.join(".surgeist-generator").exists());
        }
    }

    #[test]
    fn layout_check_taffy_checkout_pin_object_and_snapshot_drift_are_source_verification() {
        let dirty = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        dirty.import().expect("seed current import");
        fs::write(
            dirty.source.join("test_fixtures/grid/current.html"),
            b"dirty source bytes\n",
        )
        .expect("drift source snapshot");
        assert_check_preserves(&dirty, Some(GeneratorErrorKind::SourceVerification));

        let pin_drift = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        pin_drift.import().expect("seed current import");
        write_source_files(
            &pin_drift.source,
            &[("grid/added.html", b"new commit\n", false)],
        );
        run_git(&pin_drift.source, &[OsStr::new("add"), OsStr::new("--all")]);
        run_git(
            &pin_drift.source,
            &[
                OsStr::new("commit"),
                OsStr::new("--quiet"),
                OsStr::new("-m"),
                OsStr::new("advance checkout without manifest"),
            ],
        );
        assert_check_preserves(&pin_drift, Some(GeneratorErrorKind::SourceVerification));

        let sha1 = Fixture::new(vec![("grid/sha1.html", b"sha1\n", false)]);
        sha1.import().expect("seed SHA-1 import");
        let sha256 = Fixture::new_sha256(vec![("grid/sha256.html", b"sha256\n", false)]);
        let before_bytes = snapshot_tree(&sha1.root);
        let before_identities = snapshot_path_identities(&sha1.root);
        let request =
            LayoutRequest::check_taffy_corpus(sha1.location.clone(), sha256.source.clone())
                .expect("cross-format check request");
        let error = crate::layout::run(request).expect_err("object-format/pin drift");
        assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
        assert_eq!(snapshot_tree(&sha1.root), before_bytes);
        assert_eq!(snapshot_path_identities(&sha1.root), before_identities);
    }

    #[test]
    fn layout_check_taffy_absent_and_stale_known_imports_are_verification() {
        let absent = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        assert_check_preserves(&absent, Some(GeneratorErrorKind::Verification));

        let missing = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        missing.import().expect("seed current import");
        fs::remove_file(missing.corpus.join("html/grid/current.html"))
            .expect("remove known imported fixture");
        assert_check_preserves(&missing, Some(GeneratorErrorKind::Verification));

        let stale = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        stale.import().expect("seed current import");
        fs::write(
            stale.corpus.join("html/grid/current.html"),
            b"stale imported bytes\n",
        )
        .expect("drift imported fixture bytes");
        assert_check_preserves(&stale, Some(GeneratorErrorKind::Verification));
    }

    #[test]
    fn layout_check_taffy_missing_sidecar_precedes_dirty_source_with_import_instruction() {
        let fixture = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        fs::write(
            fixture.source.join("test_fixtures/grid/current.html"),
            b"dirty source bytes\n",
        )
        .expect("drift source snapshot");
        let before_bytes = snapshot_tree(&fixture.root);
        let before_identities = snapshot_path_identities(&fixture.root);
        let before_root_identity = path_identity(&fixture.root);

        let error = fixture.check().expect_err("missing Taffy sidecar");
        assert_eq!(
            error.kind(),
            GeneratorErrorKind::Verification,
            "unexpected check diagnostic: {error}"
        );
        assert_eq!(
            error.to_string(),
            "check Taffy corpus: Taffy import sidecar is absent; run import-taffy with the named source"
        );
        assert_eq!(snapshot_tree(&fixture.root), before_bytes);
        assert_eq!(snapshot_path_identities(&fixture.root), before_identities);
        assert_eq!(path_identity(&fixture.root), before_root_identity);
    }

    #[test]
    fn layout_check_taffy_sidecar_free_legacy_with_taffy_html_is_verification_and_read_only() {
        let fixture = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        write_corpus_file(&fixture.corpus.join("html/grid/current.html"), b"current\n");
        let before_bytes = snapshot_tree(&fixture.root);
        let before_identities = snapshot_path_identities(&fixture.root);
        let before_root_identity = path_identity(&fixture.root);

        let error = fixture.check().expect_err("sidecar-free legacy import");
        assert_eq!(
            error.kind(),
            GeneratorErrorKind::Verification,
            "unexpected check diagnostic: {error}"
        );
        assert_eq!(
            error.to_string(),
            "check Taffy corpus: Taffy import sidecar is absent; run import-taffy with the named source"
        );
        assert_eq!(snapshot_tree(&fixture.root), before_bytes);
        assert_eq!(snapshot_path_identities(&fixture.root), before_identities);
        assert_eq!(path_identity(&fixture.root), before_root_identity);
    }

    #[test]
    fn layout_check_taffy_malformed_and_unknown_inventory_are_invalid_inventory() {
        let malformed = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        malformed.import().expect("seed current import");
        fs::write(
            malformed.corpus.join("html/.surgeist-taffy-source.json"),
            b"{}\n",
        )
        .expect("malform Taffy sidecar");
        assert_check_preserves(&malformed, Some(GeneratorErrorKind::InvalidInventory));

        let unknown = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        unknown.import().expect("seed current import");
        write_corpus_file(&unknown.corpus.join("html/unknown.html"), b"unknown\n");
        assert_check_preserves(&unknown, Some(GeneratorErrorKind::InvalidInventory));
    }

    #[test]
    fn layout_check_taffy_malformed_sidecar_precedes_dirty_source_verification() {
        let malformed = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        malformed.import().expect("seed current import");
        fs::write(
            malformed.corpus.join("html/.surgeist-taffy-source.json"),
            b"{}\n",
        )
        .expect("malform Taffy sidecar");
        fs::write(
            malformed.source.join("test_fixtures/grid/current.html"),
            b"dirty source bytes\n",
        )
        .expect("drift source snapshot");

        assert_check_preserves(&malformed, Some(GeneratorErrorKind::InvalidInventory));
    }

    #[test]
    fn layout_check_taffy_unknown_inventory_precedes_wrong_checkout_verification() {
        let unknown = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        unknown.import().expect("seed current import");
        write_corpus_file(&unknown.corpus.join("html/unknown.html"), b"unknown\n");
        let wrong = Fixture::new(vec![("other/wrong.html", b"wrong checkout\n", false)]);
        let before_unknown_bytes = snapshot_tree(&unknown.root);
        let before_unknown_identities = snapshot_path_identities(&unknown.root);
        let before_wrong_bytes = snapshot_tree(&wrong.root);
        let before_wrong_identities = snapshot_path_identities(&wrong.root);
        let request =
            LayoutRequest::check_taffy_corpus(unknown.location.clone(), wrong.source.clone())
                .expect("wrong-checkout request");

        let error = crate::layout::run(request).expect_err("unknown import inventory");
        assert_eq!(
            error.kind(),
            GeneratorErrorKind::InvalidInventory,
            "unexpected check diagnostic: {error}"
        );
        assert_eq!(snapshot_tree(&unknown.root), before_unknown_bytes);
        assert_eq!(
            snapshot_path_identities(&unknown.root),
            before_unknown_identities
        );
        assert_eq!(snapshot_tree(&wrong.root), before_wrong_bytes);
        assert_eq!(
            snapshot_path_identities(&wrong.root),
            before_wrong_identities
        );
    }

    #[test]
    fn layout_read_only_taffy_coordination_states_are_verification_and_byte_identical() {
        let active = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        active.import().expect("seed current import");
        let lease = GenerationLease::acquire(
            &active.location,
            Domain::Layout,
            "surgeist-layout-generate",
            &RunScope::Full,
            "import-taffy",
        )
        .expect("hold active exclusive layout lease");
        assert_check_preserves(&active, Some(GeneratorErrorKind::Verification));
        drop(lease);

        let resumable = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        resumable.import().expect("seed current import");
        let active_transaction = resumable
            .corpus
            .join(".surgeist-generator/transactions/layout/active-read-only-test");
        fs::create_dir(&active_transaction).expect("create resumable transaction residue");
        fs::set_permissions(&active_transaction, fs::Permissions::from_mode(0o700))
            .expect("set private residue mode");
        assert_check_preserves(&resumable, Some(GeneratorErrorKind::Verification));

        let malformed = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        malformed.import().expect("seed current import");
        fs::write(
            malformed
                .corpus
                .join(".surgeist-generator/acquisition.lock"),
            b"malformed lock header\n",
        )
        .expect("malform immutable acquisition lock");
        assert_check_preserves(&malformed, Some(GeneratorErrorKind::Verification));
    }

    #[test]
    fn layout_taffy_pin_and_count_update_requires_reimport_not_generator_change() {
        let mut fixture = Fixture::new(vec![("grid/first.html", b"first\n", false)]);
        fixture.import().expect("import first manifest-owned pair");
        fixture.check().expect("check first manifest-owned pair");
        let first_sidecar = fs::read(fixture.corpus.join("html/.surgeist-taffy-source.json"))
            .expect("read first sidecar");

        fixture.replace_source(vec![
            ("grid/second.html", b"second\n", false),
            ("nested/third.html", b"third\n", false),
        ]);
        assert_check_preserves(&fixture, Some(GeneratorErrorKind::Verification));
        assert_eq!(
            fs::read(fixture.corpus.join("html/.surgeist-taffy-source.json"))
                .expect("old sidecar remains"),
            first_sidecar
        );

        fixture
            .import()
            .expect("reimport second manifest-owned pair");
        assert_check_preserves(&fixture, None);
        let second_sidecar = fs::read(fixture.corpus.join("html/.surgeist-taffy-source.json"))
            .expect("read second sidecar");
        let value: serde_json::Value =
            serde_json::from_slice(&second_sidecar).expect("second sidecar JSON");
        assert_ne!(second_sidecar, first_sidecar);
        assert_eq!(value["source"]["revision"], fixture.revision);
        assert_eq!(value["source_file_count"], 2);
        assert_eq!(value["imported_file_count"], 2);
    }
}
