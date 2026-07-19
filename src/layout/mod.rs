//! Browser-free Taffy import over caller-supplied corpus and checkout roots.
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
    /// Constructs a Taffy import request without filesystem access.
    ///
    /// The caller supplies an existing checkout. An empty source path is a
    /// [`GeneratorErrorKind::Cli`] error.
    pub fn import_taffy(location: CorpusLocation, source_root: PathBuf) -> Result<Self> {
        if source_root.as_os_str().is_empty() {
            return Err(cli_error(
                "construct layout request",
                "import-taffy requires a nonempty source root",
            ));
        }
        Ok(Self {
            location,
            command: LayoutCommand::ImportTaffy,
            source_root,
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

    /// Returns the source checkout supplied by this import request.
    #[must_use]
    pub fn source_root(&self) -> Option<&Path> {
        Some(&self.source_root)
    }
}

/// Executes one browser-free layout operation synchronously.
pub fn run(request: LayoutRequest) -> Result<()> {
    match request.command() {
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
