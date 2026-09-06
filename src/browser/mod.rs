//! Browser corpus generation with caller-owned fixture semantics.
//!
//! The adapter prepares a browser job and lowers its opaque result to artifact
//! bytes. This module owns execution, provenance, checking, and publication.
//! Browser/source acquisition is an explicit operation; generation and checking
//! never download or install their inputs.
//!
//! Reports record exact current input hashes, source import receipts, and opaque
//! generated artifact hashes. Host, browser, and executed-job digests describe
//! the historical execution; offline checking does not reopen those executables
//! or reconstruct location-dependent document URLs. It prepares current inputs
//! again to verify the complete linked-resource set without running a browser.
//!
//! Filtered generation preserves full reports. A separate transactional ownership
//! record allows the next full run to reconcile filtered or diagnostic outputs;
//! a diagnostic report never qualifies as a clean current corpus.

mod acquisition;
#[path = "browser.rs"]
mod browser_runtime;
mod engine;
mod measurement;
mod model;
mod profile;
mod report;
mod source_import;
mod supervisor;
mod version;

pub use acquisition::{AcquisitionMode, acquire_browser, acquire_source};
pub use engine::{check_corpus, generate};
pub use model::{
    BrowserCorpus, BrowserCorpusAdapter, BrowserLaunch, BrowserLocation, BrowserSettings,
    CaseOutcome, CaseSpec, FixtureInput, FixtureSpec, FixtureStatus, GenerationError,
    GenerationRequest, PreparedFixture, PriorOwnership, ReportScope, ResourceDependencies,
};
pub use report::{BrowserGeneratedArtifact, BrowserGenerationReport, BrowserReportSummary};

/// Handles an authenticated internal browser supervisor invocation, if present.
///
/// A hosting executable calls this before interpreting its own command line.
/// A present but invalid capsule is an error and must not fall through to the
/// ordinary application command.
pub fn run_supervisor_from_env() -> Option<crate::Result<()>> {
    supervisor::run_from_env_if_present()
}

pub use source_import::{
    ImportedSourceFile, SourceAttestation, SourceImportProvenance, SourceImportSpec, import_source,
    import_source_with_expected_inputs, verify_import, verify_source_import,
};

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
mod profile_tests;
