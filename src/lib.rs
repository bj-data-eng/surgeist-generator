//! Checked, domain-neutral contracts for Surgeist corpus generators.

#![forbid(unsafe_code)]

mod core;
mod error;

#[cfg(feature = "css-corpus")]
pub mod css;

pub use core::{
    ArtifactProvenance, CaseDisposition, CaseDispositionRecord, CorpusLocation, GenerationCounts,
    GenerationReport, ManifestVersion, PinnedSource, RelativePath, ReportArtifact, RunScope,
    Sha256Digest, SourceRevision, VerifiedSource, collect_regular_files, parse_manifest,
    validate_disposition_records, verify_git_source,
};
pub use error::{GeneratorError, GeneratorErrorKind, Result};

/// Crate identity string used by smoke tests.
pub const CRATE_NAME: &str = "surgeist-generator";

#[cfg(test)]
mod tests {
    use super::CRATE_NAME;

    #[test]
    fn exposes_crate_identity() {
        assert_eq!(CRATE_NAME, "surgeist-generator");
    }
}
