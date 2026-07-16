use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize};

use crate::{GeneratorError, GeneratorErrorKind, RelativePath, Result};

/// Manifest disposition for a generated case.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "kebab-case")]
pub enum CaseDisposition {
    Active,
    ExpectedFail,
    Unsupported,
    Quarantined,
}

/// Checked association of a case identity, source, disposition, and reason.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CaseDispositionRecord {
    case_id: String,
    source_path: RelativePath,
    disposition: CaseDisposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

impl CaseDispositionRecord {
    pub fn new(
        case_id: impl Into<String>,
        source_path: RelativePath,
        disposition: CaseDisposition,
        reason: Option<impl Into<String>>,
    ) -> Result<Self> {
        let case_id = case_id.into();
        if case_id.is_empty() || case_id.trim() != case_id || case_id.contains('\0') {
            return Err(invalid_inventory("case ID must be nonempty, trimmed UTF-8"));
        }
        let reason = reason.map(Into::into);
        match disposition {
            CaseDisposition::Active if reason.is_some() => {
                return Err(invalid_inventory("active case must not have a reason"));
            }
            CaseDisposition::ExpectedFail
            | CaseDisposition::Unsupported
            | CaseDisposition::Quarantined => {
                if reason
                    .as_deref()
                    .is_none_or(|value| value.is_empty() || value.trim() != value)
                {
                    return Err(invalid_inventory(
                        "non-active case must have a nonempty trimmed reason",
                    ));
                }
            }
            CaseDisposition::Active => {}
        }
        Ok(Self {
            case_id,
            source_path,
            disposition,
            reason,
        })
    }

    #[must_use]
    pub fn case_id(&self) -> &str {
        &self.case_id
    }

    #[must_use]
    pub const fn source_path(&self) -> &RelativePath {
        &self.source_path
    }

    #[must_use]
    pub const fn disposition(&self) -> CaseDisposition {
        self.disposition
    }

    #[must_use]
    pub fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

impl<'de> Deserialize<'de> for CaseDispositionRecord {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawRecord {
            case_id: String,
            source_path: RelativePath,
            disposition: CaseDisposition,
            reason: Option<String>,
        }

        let raw = RawRecord::deserialize(deserializer)?;
        Self::new(raw.case_id, raw.source_path, raw.disposition, raw.reason)
            .map_err(serde::de::Error::custom)
    }
}

/// Validates unique case and source identities and returns deterministic ordering.
pub fn validate_disposition_records(
    mut records: Vec<CaseDispositionRecord>,
) -> Result<Vec<CaseDispositionRecord>> {
    let mut case_ids = BTreeSet::new();
    let mut sources = BTreeSet::new();
    for record in &records {
        if !case_ids.insert(record.case_id.clone()) {
            return Err(invalid_inventory(format!(
                "duplicate case ID: {}",
                record.case_id
            )));
        }
        if !sources.insert(record.source_path.clone()) {
            return Err(invalid_inventory(format!(
                "duplicate case source: {}",
                record.source_path.as_str()
            )));
        }
    }
    records.sort_by(|left, right| {
        left.case_id
            .cmp(&right.case_id)
            .then_with(|| left.source_path.cmp(&right.source_path))
    });
    Ok(records)
}

fn invalid_inventory(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidInventory,
        "validate case dispositions",
        detail,
    )
}

#[cfg(test)]
mod tests {
    use super::{CaseDisposition, CaseDispositionRecord, validate_disposition_records};
    use crate::RelativePath;

    fn source(value: &str) -> RelativePath {
        RelativePath::new(value).expect("strict source path")
    }

    #[test]
    fn dispositions_require_reasons_exactly_and_reject_duplicates() {
        assert!(
            CaseDispositionRecord::new(
                "active",
                source("a.json"),
                CaseDisposition::Active,
                Some("unexpected")
            )
            .is_err()
        );
        assert!(
            CaseDispositionRecord::new(
                "unsupported",
                source("b.json"),
                CaseDisposition::Unsupported,
                None::<String>
            )
            .is_err()
        );
        assert!(
            CaseDispositionRecord::new(
                "quarantined",
                source("c.json"),
                CaseDisposition::Quarantined,
                Some(" reason ")
            )
            .is_err()
        );

        let first = CaseDispositionRecord::new(
            "same",
            source("a.json"),
            CaseDisposition::Active,
            None::<String>,
        )
        .expect("active record");
        let duplicate_id = CaseDispositionRecord::new(
            "same",
            source("b.json"),
            CaseDisposition::ExpectedFail,
            Some("known failure"),
        )
        .expect("non-active record");
        assert!(validate_disposition_records(vec![first, duplicate_id]).is_err());

        let first_source = CaseDispositionRecord::new(
            "first",
            source("same.json"),
            CaseDisposition::Active,
            None::<String>,
        )
        .expect("first source");
        let duplicate_source = CaseDispositionRecord::new(
            "second",
            source("same.json"),
            CaseDisposition::Quarantined,
            Some("isolated"),
        )
        .expect("duplicate source record");
        assert!(validate_disposition_records(vec![first_source, duplicate_source]).is_err());
    }
}
