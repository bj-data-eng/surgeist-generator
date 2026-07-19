use std::collections::BTreeSet;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Visitor};

use crate::{GeneratorError, GeneratorErrorKind, RelativePath, Result};

/// Manifest disposition for a generated case.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
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
        if !validate_case_id(&case_id) {
            return Err(invalid_inventory(
                "case ID does not match the canonical grammar",
            ));
        }
        let reason = reason.map(Into::into);
        if !validate_disposition_reason(disposition, reason.as_deref()) {
            return match disposition {
                CaseDisposition::Active => {
                    Err(invalid_inventory("active case must not have a reason"))
                }
                CaseDisposition::ExpectedFail
                | CaseDisposition::Unsupported
                | CaseDisposition::Quarantined => Err(invalid_inventory(
                    "non-active case must have a nonempty trimmed reason",
                )),
            };
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
            .map_err(|error| serde::de::Error::custom(error.serde_message()))
    }
}

/// Validates unique case identities and returns deterministic case-ID ordering.
pub fn validate_disposition_records(
    mut records: Vec<CaseDispositionRecord>,
) -> Result<Vec<CaseDispositionRecord>> {
    let mut case_ids = BTreeSet::new();
    for record in &records {
        if !case_ids.insert(record.case_id.clone()) {
            return Err(invalid_inventory(format!(
                "duplicate case ID: {}",
                record.case_id
            )));
        }
    }
    records.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    Ok(records)
}

impl Serialize for CaseDisposition {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(match self {
            Self::Active => "active",
            Self::ExpectedFail => "expected-fail",
            Self::Unsupported => "unsupported",
            Self::Quarantined => "quarantined",
        })
    }
}

impl<'de> Deserialize<'de> for CaseDisposition {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct DispositionVisitor;

        impl Visitor<'_> for DispositionVisitor {
            type Value = CaseDisposition;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a canonical case-disposition string")
            }

            fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                match value {
                    "active" => Ok(CaseDisposition::Active),
                    "expected-fail" => Ok(CaseDisposition::ExpectedFail),
                    "unsupported" => Ok(CaseDisposition::Unsupported),
                    "quarantined" => Ok(CaseDisposition::Quarantined),
                    _ => Err(E::custom("InvalidInventory: noncanonical case disposition")),
                }
            }
        }

        deserializer.deserialize_str(DispositionVisitor)
    }
}

fn validate_case_id(value: &str) -> bool {
    if value.is_empty()
        || value.len() > 4096
        || value.trim() != value
        || value.chars().any(char::is_control)
        || value.contains('\\')
    {
        return false;
    }
    let mut parts = value.split('#');
    let Some(path) = parts.next() else {
        return false;
    };
    let suffix = parts.next();
    if parts.next().is_some() || RelativePath::new(path).is_err() {
        return false;
    }
    let Some(pointer) = suffix else {
        return true;
    };
    if !pointer.is_empty() && !pointer.starts_with('/') {
        return false;
    }
    let bytes = pointer.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'~' {
            if bytes
                .get(index + 1)
                .is_none_or(|next| !matches!(next, b'0' | b'1'))
            {
                return false;
            }
            index += 2;
        } else {
            index += 1;
        }
    }
    true
}

pub(crate) fn validate_disposition_reason(
    disposition: CaseDisposition,
    reason: Option<&str>,
) -> bool {
    match disposition {
        CaseDisposition::Active => reason.is_none(),
        CaseDisposition::ExpectedFail
        | CaseDisposition::Unsupported
        | CaseDisposition::Quarantined => reason.is_some_and(validate_reason),
    }
}

fn validate_reason(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 2048
        && value.trim() == value
        && !value.chars().any(char::is_control)
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
    fn dispositions_require_reasons_reject_duplicate_ids_and_accept_repeated_sources() {
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
        let records = validate_disposition_records(vec![duplicate_source, first_source])
            .expect("distinct case IDs may share one source path");
        assert_eq!(records[0].case_id(), "first");
        assert_eq!(records[1].case_id(), "second");
    }

    #[test]
    fn case_ids_reject_multiple_hash_delimiters_and_remain_source_independent() {
        let source = source("nested#context/Fixture#.json");
        for id in [
            "nested/Fixture.json#/",
            "nested/Fixture.json#/label~1tail",
            "nested/Fixture.json#",
            "nested/Other.json#/label",
        ] {
            CaseDispositionRecord::new(id, source.clone(), CaseDisposition::Active, None::<String>)
                .unwrap_or_else(|error| panic!("rejected canonical case ID {id:?}: {error}"));
        }

        for id in [
            "nested/Fixture.json##/extra-delimiter",
            "nested/Fixture.json#/label#tail",
            "nested/Fixture.json#not-a-pointer",
            "nested/Fixture.json#/bad~",
            "nested/Fixture.json#/bad~2escape",
        ] {
            assert!(
                CaseDispositionRecord::new(
                    id,
                    source.clone(),
                    CaseDisposition::Active,
                    None::<String>,
                )
                .is_err(),
                "accepted malformed case ID {id:?}"
            );
        }
    }
}
