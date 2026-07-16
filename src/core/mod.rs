mod case;
mod corpus;
mod hash;
mod manifest;
mod report;
mod source;

pub use case::{CaseDisposition, CaseDispositionRecord, validate_disposition_records};
pub use corpus::{CorpusLocation, RelativePath, RunScope, collect_regular_files};
pub use hash::Sha256Digest;
pub use manifest::{ManifestVersion, parse_manifest};
pub use report::{ArtifactProvenance, GenerationCounts, GenerationReport, ReportArtifact};
pub use source::{PinnedSource, SourceRevision, VerifiedSource, verify_git_source};

pub(crate) fn validate_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    (1..=64).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
        && !value.contains("..")
}

pub(crate) fn validate_repository_url(value: &str) -> bool {
    if !value.is_ascii() || !value.starts_with("https://") {
        return false;
    }
    let Some((authority, path)) = value[8..].split_once('/') else {
        return false;
    };
    let labels: Vec<_> = authority.split('.').collect();
    if labels.len() < 2
        || labels.iter().any(|label| {
            label.is_empty()
                || !label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
    {
        return false;
    }
    let segments: Vec<_> = path.split('/').collect();
    !path.is_empty()
        && path.ends_with(".git")
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && !matches!(*segment, "." | "..")
                && segment.bytes().all(|byte| {
                    (0x21..=0x7e).contains(&byte) && !matches!(byte, b'%' | b'?' | b'#' | b'\\')
                })
        })
}

pub(crate) fn validate_generated_extension(value: &str) -> bool {
    (1..=16).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
}
