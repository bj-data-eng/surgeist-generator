use std::collections::BTreeSet;
use std::path::Path;

use serde::Deserialize;

use crate::core::{validate_disposition_reason, validate_json_case_id_syntax};
use crate::{
    CaseDisposition, CaseDispositionRecord, GeneratorError, GeneratorErrorKind, ManifestVersion,
    RelativePath, Result, SourceRevision, parse_manifest,
};

pub(super) const CSSTREE_REPOSITORY: &str = "https://github.com/csstree/csstree.git";
pub(super) const FIXTURE_ROOT: &str = "fixtures/ast";
pub(super) const SIDECAR_FILE: &str = ".surgeist-source.json";
pub(super) const REPORT_RELATIVE: &str = "generation-reports/all.json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CssManifest {
    pub(super) repository: String,
    pub(super) revision: SourceRevision,
    pub(super) fixture_root: RelativePath,
    pub(super) import_root: RelativePath,
    pub(super) expected_files: usize,
    pub(super) expected_cases: usize,
    pub(super) expectation_root: RelativePath,
    pub(super) report_file: RelativePath,
    pub(super) cases: Vec<CssCaseOverride>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CssCaseOverride {
    id: String,
    status: CaseDisposition,
    reason: Option<String>,
}

impl CssCaseOverride {
    pub(super) fn id(&self) -> &str {
        &self.id
    }

    pub(super) fn bind(&self, source_path: &RelativePath) -> Result<CaseDispositionRecord> {
        CaseDispositionRecord::new(
            &self.id,
            source_path.clone(),
            self.status,
            self.reason.clone(),
        )
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    schema_version: ManifestVersion,
    source: RawSource,
    artifacts: RawArtifacts,
    #[serde(default)]
    cases: Vec<RawCase>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSource {
    kind: String,
    repository: String,
    revision: SourceRevision,
    fixture_root: RelativePath,
    import_root: RelativePath,
    expected_files: u64,
    expected_cases: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawArtifacts {
    expectation_root: RelativePath,
    report_file: RelativePath,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCase {
    id: String,
    status: CaseDisposition,
    reason: Option<String>,
}

pub(super) fn parse(bytes: &[u8], path: &Path) -> Result<CssManifest> {
    let text = std::str::from_utf8(bytes).map_err(|_| invalid_manifest("manifest is not UTF-8"))?;
    let raw: RawManifest = parse_manifest(text, path)?;
    raw.schema_version.require(ManifestVersion::new(1)?, path)?;
    if raw.source.kind != "csstree" {
        return Err(invalid_manifest("source.kind must be exactly csstree"));
    }
    if raw.source.repository != CSSTREE_REPOSITORY {
        return Err(invalid_manifest(
            "source.repository must be the canonical CSSTree HTTPS Git URL",
        ));
    }
    if raw.source.fixture_root.as_str() != FIXTURE_ROOT {
        return Err(invalid_manifest(
            "source.fixture_root must be exactly fixtures/ast",
        ));
    }
    require_root(&raw.source.import_root, "source.import_root")?;
    require_root(
        &raw.artifacts.expectation_root,
        "artifacts.expectation_root",
    )?;
    if raw.source.import_root == raw.artifacts.expectation_root {
        return Err(invalid_manifest(
            "import and expectation roots must be distinct",
        ));
    }
    let expected_report = format!(
        "{}/{}",
        raw.artifacts.expectation_root.as_str(),
        REPORT_RELATIVE
    );
    if raw.artifacts.report_file.as_str() != expected_report {
        return Err(invalid_manifest(
            "artifacts.report_file must be <expectation_root>/generation-reports/all.json",
        ));
    }

    let expected_files = positive_count(raw.source.expected_files, "expected_files")?;
    let expected_cases = positive_count(raw.source.expected_cases, "expected_cases")?;
    if raw.cases.len() > expected_cases {
        return Err(invalid_manifest(
            "case override count exceeds source.expected_cases",
        ));
    }
    let mut case_ids = BTreeSet::new();
    let mut cases = Vec::with_capacity(raw.cases.len());
    for case in raw.cases {
        if !validate_json_case_id_syntax(&case.id) {
            return Err(invalid_manifest(
                "case ID must contain a strict JSON source and canonical JSON pointer",
            ));
        }
        if !validate_disposition_reason(case.status, case.reason.as_deref()) {
            return Err(invalid_manifest(
                "case disposition and reason combination is invalid",
            ));
        }
        if !case_ids.insert(case.id.clone()) {
            return Err(invalid_manifest("case IDs must be unique"));
        }
        cases.push(CssCaseOverride {
            id: case.id,
            status: case.status,
            reason: case.reason,
        });
    }
    cases.sort_by(|left, right| left.id.cmp(&right.id));

    Ok(CssManifest {
        repository: raw.source.repository,
        revision: raw.source.revision,
        fixture_root: raw.source.fixture_root,
        import_root: raw.source.import_root,
        expected_files,
        expected_cases,
        expectation_root: raw.artifacts.expectation_root,
        report_file: raw.artifacts.report_file,
        cases,
    })
}

fn positive_count(value: u64, label: &str) -> Result<usize> {
    if value == 0 || value > u64::from(u32::MAX) {
        return Err(invalid_manifest(format!(
            "source.{label} must be 1 through u32::MAX"
        )));
    }
    usize::try_from(value).map_err(|_| invalid_manifest(format!("source.{label} overflows")))
}

fn require_root(path: &RelativePath, label: &str) -> Result<()> {
    let value = path.as_str();
    if value.contains('/')
        || matches!(value, "corpus.toml" | ".surgeist-generator" | SIDECAR_FILE)
        || value.starts_with("._surgeist-")
    {
        return Err(invalid_manifest(format!(
            "{label} must be one non-reserved component"
        )));
    }
    Ok(())
}

fn invalid_manifest(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidManifest,
        "validate CSS corpus manifest",
        detail,
    )
}
