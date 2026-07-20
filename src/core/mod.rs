#[cfg(any(test, feature = "css-corpus", feature = "layout-browser"))]
mod artifact;
mod case;
#[cfg(any(test, feature = "css-corpus", feature = "layout-browser"))]
mod coordination;
mod corpus;
mod fs;
mod hash;
#[cfg(any(test, feature = "css-corpus", feature = "layout-browser"))]
mod inventory;
#[cfg(any(test, feature = "css-corpus", feature = "layout-browser"))]
mod lease;
mod manifest;
#[cfg(any(test, feature = "css-corpus", feature = "layout-browser"))]
mod protection;
mod report;
mod source;
#[cfg(any(test, feature = "css-corpus", feature = "layout-browser"))]
mod transaction;

pub use case::{CaseDisposition, CaseDispositionRecord, validate_disposition_records};
pub use corpus::{CorpusLocation, RelativePath, RunScope, collect_regular_files};
pub use hash::Sha256Digest;
pub use manifest::{ManifestVersion, parse_manifest};
pub use report::{ArtifactProvenance, GenerationCounts, GenerationReport, ReportArtifact};
pub use source::{PinnedSource, SourceRevision, VerifiedSource, verify_git_source};

#[cfg(any(feature = "css-corpus", feature = "layout-browser"))]
pub(crate) use artifact::{
    ArtifactPlan, ArtifactReservation, PublicationInventory, PublicationPolicy,
};
#[cfg(feature = "css-corpus")]
pub(crate) use case::validate_disposition_reason;
#[cfg(any(feature = "css-corpus", feature = "layout-browser"))]
pub(crate) use coordination::Domain;
#[cfg(feature = "layout-browser")]
pub(crate) use coordination::{
    authenticate_layout_supervisor_owner, corpus_authority_key, new_token,
};
#[cfg(feature = "layout-browser")]
pub(crate) use fs::{
    BoundPath, HeldIdentity, OpaqueTreeSnapshot, PRIVATE_DIRECTORY_MODE, PRIVATE_FILE_MODE,
};
#[cfg(any(feature = "css-corpus", feature = "layout-browser"))]
pub(crate) use fs::{CORPUS_FILE_MODE, NodeKind, RootedFs};
#[cfg(all(test, feature = "layout-browser"))]
pub(crate) use fs::{DurabilityEvent, DurabilityPhase, DurabilityPrimitive, RootedObserver};
#[cfg(feature = "layout-browser")]
pub(crate) use inventory::InventoryEntry;
#[cfg(any(feature = "css-corpus", feature = "layout-browser"))]
pub(crate) use inventory::{Inventory, InventoryPolicy};
#[cfg(any(feature = "css-corpus", feature = "layout-browser"))]
pub(crate) use lease::GenerationCheck;
#[cfg(any(feature = "css-corpus", feature = "layout-browser"))]
pub(crate) use lease::GenerationLease;
#[cfg(any(feature = "css-corpus", feature = "layout-browser"))]
pub(crate) use protection::NamespaceDisjointness;
#[cfg(any(feature = "css-corpus", feature = "layout-browser"))]
pub(crate) use protection::ProtectedSourceDisjointness;
#[cfg(any(feature = "css-corpus", feature = "layout-browser"))]
pub(crate) use source::{
    ObjectFormat, ProtectedSource, ProtectedSourceInventory, ProtectedTreeEntryKind, SnapshotEntry,
    VerifiedSourceSnapshot, verify_protected_git_source_inventory,
};

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
