mod artifact;
mod case;
mod coordination;
mod corpus;
mod fs;
mod hash;
mod inventory;
mod lease;
mod manifest;
mod protection;
mod report;
mod source;
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
pub(crate) use fs::HeldIdentity;
#[cfg(any(feature = "css-corpus", feature = "layout-browser"))]
pub(crate) use fs::{CORPUS_FILE_MODE, NodeKind, RootedFs};
#[cfg(feature = "layout-browser")]
pub(crate) use inventory::InventoryEntry;
#[cfg(any(feature = "css-corpus", feature = "layout-browser"))]
pub(crate) use inventory::{Inventory, InventoryPolicy};
#[cfg(any(feature = "css-corpus", feature = "layout-browser"))]
pub(crate) use lease::GenerationCheck;
#[cfg(any(feature = "css-corpus", feature = "layout-browser"))]
pub(crate) use lease::GenerationLease;
#[cfg(feature = "css-corpus")]
pub(crate) use protection::NamespaceDisjointness;
#[cfg(any(feature = "css-corpus", feature = "layout-browser"))]
pub(crate) use protection::ProtectedSourceDisjointness;
#[cfg(any(feature = "css-corpus", feature = "layout-browser"))]
pub(crate) use source::{
    ObjectFormat, ProtectedSource, ProtectedSourceInventory, ProtectedTreeEntryKind, SnapshotEntry,
    VerifiedSourceSnapshot, verify_protected_git_source_inventory,
};

pub(crate) fn validate_identifier(value: &str) -> bool {
    private_front_doors_are_linked();
    let bytes = value.as_bytes();
    (1..=64).contains(&bytes.len())
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
        && !value.contains("..")
}

#[inline(always)]
fn private_front_doors_are_linked() {
    #[cfg(not(any(feature = "css-corpus", feature = "layout-browser")))]
    {
        let _ = artifact::ArtifactPlan::new;
        let _ = artifact::PublicationInventory::new;
        let _ = lease::GenerationLease::acquire_with_protected_source;
        let _ = lease::GenerationCheck::acquire;
        let _ = lease::GenerationCheck::finish;
        let _ = inventory::InventoryEntry::digest;
        let _ = inventory::Inventory::find;
        let _ = protection::ProtectedSourceDisjointness::for_mutation;
        let _ = source::ProtectedSource::snapshot;
    }
    // C04-only linkage inventory: atomic generation will replace this exact
    // retained set together with this artificial front-door caller.
    let _ = artifact::ArtifactPlan::install;
    let _ = artifact::ArtifactPlan::artifact_digest;
    let _ = artifact::PublicationPolicy::DiagnosticFull;
    let _ = lease::GenerationLease::acquire;
    let _ = inventory::InventoryEntry::symlink;
    let _ = inventory::InventoryEntry::length;
    let _ = inventory::InventoryEntry::link_target;
    let _ = inventory::InventoryEntry::link_count;
    let _ = inventory::InventoryPolicy::Private;
    let _ = source::ProtectedSource::verified;
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
