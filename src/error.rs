use std::error::Error;
use std::fmt;

/// Result type returned by generator contracts.
pub type Result<T> = std::result::Result<T, GeneratorError>;

/// Stable semantic category for a generator failure.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum GeneratorErrorKind {
    Cli,
    InvalidPath,
    InvalidManifest,
    InvalidInventory,
    SourceVerification,
    LeaseActive,
    Process,
    Io,
    ArtifactTransaction,
    Generation,
    Verification,
}

/// Generator failure with an operation-oriented diagnostic and optional source.
#[derive(Debug)]
pub struct GeneratorError {
    kind: GeneratorErrorKind,
    operation: String,
    detail: String,
    source: Option<Box<dyn Error + Send + Sync>>,
}

impl GeneratorError {
    pub(crate) fn new(
        kind: GeneratorErrorKind,
        operation: impl Into<String>,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            operation: operation.into(),
            detail: detail.into(),
            source: None,
        }
    }

    pub(crate) fn with_source<E>(
        kind: GeneratorErrorKind,
        operation: impl Into<String>,
        detail: impl Into<String>,
        source: E,
    ) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            kind,
            operation: operation.into(),
            detail: detail.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Returns the semantic category of this failure.
    #[must_use]
    pub const fn kind(&self) -> GeneratorErrorKind {
        self.kind
    }

    /// Returns the process exit code assigned to this category.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        match self.kind {
            GeneratorErrorKind::Cli
            | GeneratorErrorKind::InvalidPath
            | GeneratorErrorKind::InvalidManifest
            | GeneratorErrorKind::InvalidInventory
            | GeneratorErrorKind::SourceVerification => 2,
            GeneratorErrorKind::LeaseActive => 3,
            GeneratorErrorKind::Process
            | GeneratorErrorKind::Io
            | GeneratorErrorKind::ArtifactTransaction
            | GeneratorErrorKind::Generation => 4,
            GeneratorErrorKind::Verification => 5,
        }
    }
}

impl fmt::Display for GeneratorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.operation, self.detail)
    }
}

impl Error for GeneratorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn Error + 'static))
    }
}
