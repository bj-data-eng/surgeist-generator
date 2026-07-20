//! Taffy maintenance and trusted-browser layout generation over explicit roots.
//!
//! Import verifies an existing clean checkout at the manifest-owned pin. It
//! never downloads, installs, or executes the source:
//!
//! ```no_run
//! # use std::path::PathBuf;
//! # use surgeist_generator::{CorpusLocation, Result};
//! # use surgeist_generator::layout::{self, LayoutRequest};
//! # fn example(location: CorpusLocation, checkout: PathBuf) -> Result<()> {
//! let request = LayoutRequest::import_taffy(location, checkout)?;
//! layout::run(request)
//! # }
//! ```
//!
//! Generation accepts one existing executable below the manifest-declared cache
//! root. It never downloads or installs a browser. The executable is a trusted
//! capability whose path, identity, digest, and version are authenticated:
//!
//! ```no_run
//! # use surgeist_generator::{CorpusLocation, RelativePath, Result};
//! # use surgeist_generator::layout::{self, LayoutRequest};
//! # fn example(location: CorpusLocation) -> Result<()> {
//! let browser = RelativePath::new("cache/chrome")?;
//! let filter = Some(RelativePath::new("grid/case.html")?);
//! layout::run(LayoutRequest::generate(location, browser, filter)?)
//! # }
//! ```
//!
//! ```compile_fail
//! use std::path::PathBuf;
//! use surgeist_generator::CorpusLocation;
//! use surgeist_generator::layout::{LayoutCommand, LayoutRequest};
//! fn mismatched_payload(location: CorpusLocation, source: PathBuf) {
//!     let _ = LayoutRequest::new(location, LayoutCommand::CheckCorpus, Some(source));
//! }
//! ```
//!
//! ```compile_fail
//! use std::path::PathBuf;
//! use surgeist_generator::CorpusLocation;
//! use surgeist_generator::layout::{LayoutCommand, LayoutRequest};
//! fn forged_payload(location: CorpusLocation, source: PathBuf) {
//!     let _ = LayoutRequest {
//!         location,
//!         command: LayoutCommand::CheckCorpus,
//!         source_root: Some(source),
//!     };
//! }
//! ```
//!
//! ```compile_fail
//! use surgeist_generator::layout::LayoutManifest;
//! ```
//!
//! ```compile_fail
//! use surgeist_generator::layout::ArtifactPlan;
//! ```
//!
//! Checking verifies the checkout, persisted sidecar, and imported files without
//! acquiring a mutation lease or repairing coordination:
//!
//! ```no_run
//! # use std::path::PathBuf;
//! # use surgeist_generator::{CorpusLocation, Result};
//! # use surgeist_generator::layout::{self, LayoutRequest};
//! # fn example(location: CorpusLocation, checkout: PathBuf) -> Result<()> {
//! let request = LayoutRequest::check_taffy_corpus(location, checkout)?;
//! layout::run(request)
//! # }
//! ```
//!
//! Complete corpus checking is fully offline. Browser provenance in XML and
//! reports is validated as a consistent historical attestation; no browser
//! cache or executable is opened or authenticated:
//!
//! ```no_run
//! # use surgeist_generator::{CorpusLocation, Result};
//! # use surgeist_generator::layout::{self, LayoutRequest};
//! # fn example(location: CorpusLocation) -> Result<()> {
//! layout::run(LayoutRequest::check_corpus(location))
//! # }
//! ```

use std::path::{Path, PathBuf};

use crate::{CorpusLocation, GeneratorError, GeneratorErrorKind, RelativePath, Result};

// C04-only preservation handoff: add the Generate API/CLI payload atomically
// with the chromiumoxide/futures/tokio/url edge; trusted executable validation
// and the cleared launch environment; supervisor, process-group, profile,
// cleanup, recovery, timeout, and panic lifecycles; helper/HTML injection,
// batching, retry, four-variant measurement, XML rendering, report
// serialization, filtering, and CleanFull/DiagnosticFull/Filtered publication;
// then retire the preservation source, finish repository guidance/policy, and
// execute the separately authorized ignored diagnostic body exactly once.
mod browser;
mod case;
mod checker;
mod cli;
mod generation;
mod importer;
mod manifest;
mod measurement;
mod profile;
mod report;
mod selection;
mod sidecar;
mod supervisor;
mod xml;

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
mod checker_tests;
#[cfg(test)]
mod profile_tests;
#[cfg(test)]
mod tests;

/// Layout corpus operation available in this capability set.
///
/// Its exact trait contract excludes open-ended ordering, hashing, defaults,
/// and serialization:
///
/// ```compile_fail
/// use std::hash::Hash;
/// use surgeist_generator::layout::LayoutCommand;
/// fn require<T: Hash>() {}
/// require::<LayoutCommand>();
/// ```
///
/// ```compile_fail
/// use surgeist_generator::layout::LayoutCommand;
/// fn require<T: Ord>() {}
/// require::<LayoutCommand>();
/// ```
///
/// ```compile_fail
/// use surgeist_generator::layout::LayoutCommand;
/// fn require<T: Default>() {}
/// require::<LayoutCommand>();
/// ```
///
/// ```compile_fail
/// use surgeist_generator::layout::LayoutCommand;
/// fn require<T: serde::Serialize>() {}
/// require::<LayoutCommand>();
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LayoutCommand {
    /// Verify imported HTML and generated XML/report attestations offline.
    CheckCorpus,
    /// Verify the manifest-pinned checkout and persisted Taffy import read-only.
    CheckTaffyCorpus,
    /// Import the manifest-pinned Taffy `test_fixtures` tree.
    ImportTaffy,
    /// Generate complete or selected XML with a trusted existing browser.
    Generate,
}

/// Checked request for one layout corpus operation.
///
/// Its private payload and exact trait contract exclude copying, ordering,
/// hashing, defaults, and serialization:
///
/// ```compile_fail
/// use surgeist_generator::layout::LayoutRequest;
/// fn require<T: Copy>() {}
/// require::<LayoutRequest>();
/// ```
///
/// ```compile_fail
/// use std::hash::Hash;
/// use surgeist_generator::layout::LayoutRequest;
/// fn require<T: Hash>() {}
/// require::<LayoutRequest>();
/// ```
///
/// ```compile_fail
/// use surgeist_generator::layout::LayoutRequest;
/// fn require<T: Ord>() {}
/// require::<LayoutRequest>();
/// ```
///
/// ```compile_fail
/// use surgeist_generator::layout::LayoutRequest;
/// fn require<T: Default>() {}
/// require::<LayoutRequest>();
/// ```
///
/// ```compile_fail
/// use surgeist_generator::layout::LayoutRequest;
/// fn require<T: serde::Serialize>() {}
/// require::<LayoutRequest>();
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayoutRequest {
    location: CorpusLocation,
    command: LayoutCommand,
    payload: LayoutPayload,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum LayoutPayload {
    None,
    Source(PathBuf),
    Generate {
        browser_path: RelativePath,
        filter: Option<RelativePath>,
    },
}

impl LayoutRequest {
    /// Constructs a complete offline corpus check without filesystem access.
    #[must_use]
    pub fn check_corpus(location: CorpusLocation) -> Self {
        Self {
            location,
            command: LayoutCommand::CheckCorpus,
            payload: LayoutPayload::None,
        }
    }

    /// Constructs a read-only Taffy corpus check without filesystem access.
    ///
    /// The caller supplies an existing checkout. An empty source path is a
    /// [`GeneratorErrorKind::Cli`] error.
    pub fn check_taffy_corpus(location: CorpusLocation, source_root: PathBuf) -> Result<Self> {
        source_request(
            location,
            LayoutCommand::CheckTaffyCorpus,
            source_root,
            "check-taffy-corpus",
        )
    }

    /// Constructs a Taffy import request without filesystem access.
    ///
    /// The caller supplies an existing checkout. An empty source path is a
    /// [`GeneratorErrorKind::Cli`] error.
    pub fn import_taffy(location: CorpusLocation, source_root: PathBuf) -> Result<Self> {
        source_request(
            location,
            LayoutCommand::ImportTaffy,
            source_root,
            "import-taffy",
        )
    }

    /// Constructs a trusted-browser generation request without filesystem access.
    pub fn generate(
        location: CorpusLocation,
        browser_path: RelativePath,
        filter: Option<RelativePath>,
    ) -> Result<Self> {
        if let Some(filter) = &filter {
            selection::validate_request_filter(filter)?;
        }
        Ok(Self {
            location,
            command: LayoutCommand::Generate,
            payload: LayoutPayload::Generate {
                browser_path,
                filter,
            },
        })
    }

    /// Returns the explicit corpus location.
    #[must_use]
    pub const fn location(&self) -> &CorpusLocation {
        &self.location
    }

    /// Returns the selected operation.
    #[must_use]
    pub const fn command(&self) -> LayoutCommand {
        self.command
    }

    /// Returns the source checkout supplied by this Taffy request.
    #[must_use]
    pub fn source_root(&self) -> Option<&Path> {
        match &self.payload {
            LayoutPayload::Source(path) => Some(path),
            LayoutPayload::None | LayoutPayload::Generate { .. } => None,
        }
    }

    /// Returns the owner-relative trusted-browser path for generation only.
    #[must_use]
    pub const fn browser_path(&self) -> Option<&RelativePath> {
        match &self.payload {
            LayoutPayload::Generate { browser_path, .. } => Some(browser_path),
            LayoutPayload::None | LayoutPayload::Source(_) => None,
        }
    }

    /// Returns the optional HTML-relative selection for generation only.
    #[must_use]
    pub const fn filter(&self) -> Option<&RelativePath> {
        match &self.payload {
            LayoutPayload::Generate { filter, .. } => filter.as_ref(),
            LayoutPayload::None | LayoutPayload::Source(_) => None,
        }
    }
}

/// Executes one layout operation synchronously.
pub fn run(request: LayoutRequest) -> Result<()> {
    match request.command() {
        LayoutCommand::CheckCorpus => checker::run(&request),
        LayoutCommand::CheckTaffyCorpus => importer::check(&request),
        LayoutCommand::ImportTaffy => importer::run(&request),
        LayoutCommand::Generate => generation::run(request),
    }
}

/// Executes one interface invocation or authenticated internal supervisor.
pub fn run_from_env() -> Result<()> {
    cli::run_from_env()
}

fn cli_error(operation: &str, detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(GeneratorErrorKind::Cli, operation, detail)
}

fn source_request(
    location: CorpusLocation,
    command: LayoutCommand,
    source_root: PathBuf,
    command_name: &str,
) -> Result<LayoutRequest> {
    if source_root.as_os_str().is_empty() {
        return Err(cli_error(
            "construct layout request",
            format!("{command_name} requires a nonempty source root"),
        ));
    }
    Ok(LayoutRequest {
        location,
        command,
        payload: LayoutPayload::Source(source_root),
    })
}
