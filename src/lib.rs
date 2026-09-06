//! Checked, domain-neutral contracts for Surgeist corpus generators.
//!
//! The default feature set exposes shared value, provenance, rooted-path, and
//! report contracts without browser dependencies. Optional capabilities are:
//!
//! - `css-corpus` exposes `css` and builds `surgeist-css-generate`.
//! - `browser-corpus` exposes `browser` for hosting applications that supply
//!   fixture preparation and measurement lowering through an adapter.
//!
//! Callers own mutable source pins, inventory counts, artifact roots, and browser
//! provenance. Browser generation and checking consume existing inputs; managed
//! browser and source acquisition are separate, explicit operations. The browser
//! library contains no fixture-domain serializer or domain-specific executable.
//!
//! Mutation is supported on Apple-Silicon macOS. The default value/read library
//! is checked for native and `wasm32-unknown-unknown` targets. Production Surgeist
//! crates do not normally depend on this tooling crate; root `surgeist` owns
//! cross-crate composition, gitlinks, and generated API audit artifacts.

#![forbid(unsafe_code)]

mod core;
mod error;

#[cfg(feature = "css-corpus")]
pub mod css;

#[cfg(feature = "browser-corpus")]
pub mod browser;

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
