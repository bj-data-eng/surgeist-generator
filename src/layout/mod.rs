//! Browser-free Taffy import and checking over explicit corpus and checkout roots.
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

use std::path::{Path, PathBuf};

use crate::{CorpusLocation, GeneratorError, GeneratorErrorKind, Result};

mod case;
mod cli;
mod importer;
mod manifest;
mod sidecar;

#[cfg(test)]
mod tests;

/// Browser-free layout corpus operation available in this capability set.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum LayoutCommand {
    /// Verify the manifest-pinned checkout and persisted Taffy import read-only.
    CheckTaffyCorpus,
    /// Import the manifest-pinned Taffy `test_fixtures` tree.
    ImportTaffy,
}

/// Checked request for one browser-free layout corpus operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayoutRequest {
    location: CorpusLocation,
    command: LayoutCommand,
    source_root: PathBuf,
}

impl LayoutRequest {
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
        Some(&self.source_root)
    }
}

/// Executes one browser-free layout operation synchronously.
pub fn run(request: LayoutRequest) -> Result<()> {
    match request.command() {
        LayoutCommand::CheckTaffyCorpus => importer::check(&request),
        LayoutCommand::ImportTaffy => importer::run(&request),
    }
}

/// Reads only `args_os` and executes one browser-free layout operation.
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
        source_root,
    })
}
