use serde::{Serialize, Serializer};

use crate::core::validate_disposition_reason;
use crate::{CaseDisposition, RelativePath, Result};

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) struct CssCaseId(String);

impl CssCaseId {
    pub(super) fn new(value: impl Into<String>, source_path: &RelativePath) -> Result<Self> {
        let value = value.into();
        if !validate_for_source(&value, source_path) {
            return Err(super::invalid_inventory(format!(
                "CSS case ID {value} does not belong to fixture {}",
                source_path.as_str()
            )));
        }
        Ok(Self(value))
    }

    pub(super) fn as_str(&self) -> &str {
        &self.0
    }
}

impl Serialize for CssCaseId {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CssCaseDispositionRecord {
    case_id: CssCaseId,
    disposition: CaseDisposition,
    reason: Option<String>,
}

impl CssCaseDispositionRecord {
    pub(super) fn new(
        case_id: impl Into<String>,
        source_path: &RelativePath,
        disposition: CaseDisposition,
        reason: Option<impl Into<String>>,
    ) -> Result<Self> {
        let case_id = CssCaseId::new(case_id, source_path)?;
        let reason = reason.map(Into::into);
        if !validate_disposition_reason(disposition, reason.as_deref()) {
            return Err(super::invalid_inventory(
                "CSS case disposition and reason combination is invalid",
            ));
        }
        Ok(Self {
            case_id,
            disposition,
            reason,
        })
    }

    pub(super) const fn case_id(&self) -> &CssCaseId {
        &self.case_id
    }

    pub(super) const fn disposition(&self) -> CaseDisposition {
        self.disposition
    }

    pub(super) fn reason(&self) -> Option<&str> {
        self.reason.as_deref()
    }
}

pub(super) fn has_bindable_syntax(value: &str) -> bool {
    validate_text(value)
        && value.match_indices('#').any(|(delimiter, _)| {
            let (source, suffix) = value.split_at(delimiter);
            RelativePath::with_extension(source, "json").is_ok()
                && suffix.strip_prefix('#').is_some_and(validate_json_pointer)
        })
}

fn validate_for_source(value: &str, source_path: &RelativePath) -> bool {
    validate_text(value)
        && RelativePath::with_extension(source_path.as_str(), "json").is_ok()
        && value
            .strip_prefix(source_path.as_str())
            .and_then(|suffix| suffix.strip_prefix('#'))
            .is_some_and(validate_json_pointer)
}

fn validate_text(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 4096
        && value.trim() == value
        && !value.chars().any(char::is_control)
        && !value.contains('\\')
}

fn validate_json_pointer(pointer: &str) -> bool {
    if !pointer.starts_with('/') {
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
