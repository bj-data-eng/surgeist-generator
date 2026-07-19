//! Synchronous, acquisition-free CSSTree corpus operations.
//!
//! The caller supplies an existing, clean checkout at the manifest's exact pin:
//!
//! ```no_run
//! # use std::path::PathBuf;
//! # use surgeist_generator::{CorpusLocation, Result};
//! # use surgeist_generator::css::{self, CssCommand, CssRequest};
//! # fn example(location: CorpusLocation, checkout: PathBuf) -> Result<()> {
//! let request = CssRequest::new(
//!     location.clone(),
//!     CssCommand::ImportCsstree,
//!     Some(checkout),
//!     None,
//! )?;
//! css::run(request)?;
//! let request = CssRequest::new(location.clone(), CssCommand::Generate, None, None)?;
//! css::run(request)?;
//! let check = CssRequest::new(location, CssCommand::CheckCorpus, None, None)?;
//! css::run(check)
//! # }
//! ```

use std::path::{Path, PathBuf};

use crate::{CorpusLocation, GeneratorError, GeneratorErrorKind, RelativePath, Result};

mod case;
mod check;
mod cli;
mod expectation;
mod filter;
mod fixture;
mod full_generation;
mod historical;
mod importer;
mod manifest;
mod report;
mod sidecar;

#[cfg(test)]
mod tests;

/// Complete CSS operation available in this feature increment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CssCommand {
    /// Import the manifest-pinned `fixtures/ast` tree from an existing checkout.
    ImportCsstree,
    /// Generate neutral expectations from the current import, optionally filtered.
    ///
    /// Filtered generation updates only matching expectations already owned by the
    /// historical full report and preserves every other expectation and report.
    Generate,
    /// Verify the current import and expectation corpus without changing it.
    CheckCorpus,
}

/// Checked request for one CSS corpus operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CssRequest {
    location: CorpusLocation,
    command: CssCommand,
    source_root: Option<PathBuf>,
    filter: Option<RelativePath>,
}

impl CssRequest {
    /// Constructs a request without filesystem access.
    pub fn new(
        location: CorpusLocation,
        command: CssCommand,
        source_root: Option<PathBuf>,
        filter: Option<RelativePath>,
    ) -> Result<Self> {
        match command {
            CssCommand::ImportCsstree => {
                if source_root
                    .as_ref()
                    .is_none_or(|root| root.as_os_str().is_empty())
                {
                    return Err(cli_error(
                        "construct CSS request",
                        "import-csstree requires a nonempty source root",
                    ));
                }
                if filter.is_some() {
                    return Err(cli_error(
                        "construct CSS request",
                        "import-csstree forbids a filter",
                    ));
                }
            }
            CssCommand::Generate => {
                if let Some(filter) = &filter {
                    self::filter::validate_request_filter(filter)?;
                }
                if source_root.is_some() {
                    return Err(cli_error(
                        "construct CSS request",
                        "generate forbids a source root",
                    ));
                }
            }
            CssCommand::CheckCorpus => {
                if source_root.is_some() {
                    return Err(cli_error(
                        "construct CSS request",
                        "check-corpus forbids a source root",
                    ));
                }
                if filter.is_some() {
                    return Err(cli_error(
                        "construct CSS request",
                        "check-corpus forbids a filter",
                    ));
                }
            }
        }
        Ok(Self {
            location,
            command,
            source_root,
            filter,
        })
    }

    /// Returns the explicit corpus location.
    #[must_use]
    pub const fn location(&self) -> &CorpusLocation {
        &self.location
    }

    /// Returns the selected operation.
    #[must_use]
    pub const fn command(&self) -> CssCommand {
        self.command
    }

    /// Returns the source checkout supplied by import operations.
    #[must_use]
    pub fn source_root(&self) -> Option<&Path> {
        self.source_root.as_deref()
    }

    /// Returns the optional fixture filter supplied by generation operations.
    #[must_use]
    pub const fn filter(&self) -> Option<&RelativePath> {
        self.filter.as_ref()
    }
}

/// Executes one CSS operation synchronously on the caller's thread.
pub fn run(request: CssRequest) -> Result<()> {
    match request.command() {
        CssCommand::ImportCsstree => importer::run(&request),
        CssCommand::Generate => full_generation::run(&request),
        CssCommand::CheckCorpus => check::run(&request),
    }
}

/// Parses `args_os` and synchronously executes one CSS operation.
pub fn run_from_env() -> Result<()> {
    cli::run_from_env()
}

fn cli_error(operation: &str, detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(GeneratorErrorKind::Cli, operation, detail)
}

fn invalid_inventory(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidInventory,
        "revalidate CSS generation inputs",
        detail,
    )
}
