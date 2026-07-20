use std::collections::BTreeSet;

use serde::Deserialize;

use crate::{GeneratorError, GeneratorErrorKind, RelativePath, Result};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub(super) enum LayoutCaseStatus {
    Active,
    ExpectedFail,
    Unsupported,
    Quarantined,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
enum LayoutSourceRoot {
    Taffy,
    Surgeist,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawCase {
    id: String,
    source_root: LayoutSourceRoot,
    source: RelativePath,
    generator: String,
    status: LayoutCaseStatus,
    reason: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LayoutCase {
    pub(super) id: String,
    pub(super) source: RelativePath,
    pub(super) status: LayoutCaseStatus,
    pub(super) reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LayoutCases {
    pub(super) authored_files: BTreeSet<RelativePath>,
    pub(super) authored_cases: Vec<LayoutCase>,
}

pub(super) fn validate(cases: Vec<RawCase>) -> Result<LayoutCases> {
    let mut ids = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut authored = BTreeSet::new();
    let mut authored_cases = Vec::new();

    for case in cases {
        if !ids.insert(case.id.clone()) {
            return Err(invalid_manifest("case IDs must be unique"));
        }
        if !sources.insert(case.source.clone()) {
            return Err(invalid_manifest("case sources must be unique"));
        }
        if case.generator != "constrained-html" {
            return Err(invalid_manifest(
                "case generator must be exactly constrained-html",
            ));
        }

        match case.source_root {
            LayoutSourceRoot::Taffy => {}
            LayoutSourceRoot::Surgeist => {
                if case.id.is_empty() || case.id.trim() != case.id {
                    return Err(invalid_manifest(
                        "Surgeist case ID must be nonempty and trimmed",
                    ));
                }
                RelativePath::with_extension(case.source.as_str(), "html").map_err(|_| {
                    invalid_manifest("Surgeist case source must be a strict .html path")
                })?;
                authored.insert(case.source.clone());
                authored_cases.push(LayoutCase {
                    id: case.id,
                    source: case.source,
                    status: case.status,
                    reason: effective_reason(case.status, case.reason),
                });
            }
        }
    }
    authored_cases.sort_by(|left, right| left.source.cmp(&right.source));
    Ok(LayoutCases {
        authored_files: authored,
        authored_cases,
    })
}

fn effective_reason(status: LayoutCaseStatus, reason: Option<String>) -> String {
    if matches!(status, LayoutCaseStatus::Active) {
        return String::new();
    }
    reason.unwrap_or_else(|| {
        match status {
            LayoutCaseStatus::Active => unreachable!("active status returned above"),
            LayoutCaseStatus::ExpectedFail => "manifest marks case expected-fail",
            LayoutCaseStatus::Unsupported => "manifest marks case unsupported",
            LayoutCaseStatus::Quarantined => "manifest marks case quarantined",
        }
        .to_owned()
    })
}

fn invalid_manifest(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidManifest,
        "validate layout corpus cases",
        detail,
    )
}
