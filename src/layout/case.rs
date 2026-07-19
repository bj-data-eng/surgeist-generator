use std::collections::BTreeSet;

use serde::Deserialize;

use crate::{GeneratorError, GeneratorErrorKind, RelativePath, Result};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum LayoutCaseStatus {
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

pub(super) fn validate(cases: Vec<RawCase>) -> Result<BTreeSet<RelativePath>> {
    let mut ids = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut authored = BTreeSet::new();

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
                authored.insert(case.source);
            }
        }

        // Status and reason are deliberately compatibility data in T01. All four
        // statuses admit an absent or byte-preserved UTF-8 reason.
        let _ = (case.status, case.reason);
    }
    Ok(authored)
}

fn invalid_manifest(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidManifest,
        "validate layout corpus cases",
        detail,
    )
}
