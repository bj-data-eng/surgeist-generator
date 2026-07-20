//! Checked, domain-neutral contracts for Surgeist corpus generators.
//!
//! The default feature set exposes only shared value, provenance, rooted-path,
//! and report contracts. The two executable drivers are opt-in:
//!
//! - `css-corpus` exposes `css` and builds `surgeist-css-generate`.
//! - `layout-browser` exposes `layout` and builds
//!   `surgeist-layout-generate`.
//!
//! Callers supply an existing owner root and a contained corpus root through
//! [`CorpusLocation`]. Corpus manifests, not this crate, own mutable source pins,
//! inventory counts, artifact roots, and browser provenance. The drivers contain
//! no downloader or installer: source imports verify caller-supplied checkouts,
//! while layout generation authenticates and executes one caller-selected,
//! already-present browser as a trusted external capability.
//!
//! Mutation is supported on Apple-Silicon macOS. The default value/read library
//! remains free of driver dependencies and is checked for native and
//! `wasm32-unknown-unknown` targets. Production Surgeist crates do not normally
//! depend on this tooling crate; root `surgeist` owns cross-crate composition,
//! gitlinks, and generated API audit artifacts.

#![forbid(unsafe_code)]

mod core;
mod error;

#[cfg(feature = "css-corpus")]
pub mod css;

#[cfg(feature = "layout-browser")]
pub mod layout;

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
