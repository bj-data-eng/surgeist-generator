use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    CorpusLocation, GeneratorError, GeneratorErrorKind, RelativePath, Result, Sha256Digest,
};

pub(super) fn invalid(operation: &str, detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(GeneratorErrorKind::InvalidInventory, operation, detail)
}

fn text(value: &str, label: &str) -> Result<()> {
    if value.is_empty() || value.trim() != value || value.chars().any(char::is_control) {
        return Err(invalid(
            "validate browser corpus contract",
            format!("{label} must be nonempty trimmed text"),
        ));
    }
    Ok(())
}

/// The fixed browser lifecycle and its caller-pinned numeric/launch settings.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawLaunch")]
pub struct BrowserLaunch {
    pub(super) batch_size: usize,
    pub(super) navigation_timeout_ms: u64,
    pub(super) dom_poll_interval_ms: u64,
    pub(super) arguments: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLaunch {
    batch_size: usize,
    navigation_timeout_ms: u64,
    dom_poll_interval_ms: u64,
    arguments: Vec<String>,
}

impl TryFrom<RawLaunch> for BrowserLaunch {
    type Error = GeneratorError;
    fn try_from(raw: RawLaunch) -> Result<Self> {
        Self::new(
            raw.batch_size,
            raw.navigation_timeout_ms,
            raw.dom_poll_interval_ms,
            raw.arguments,
        )
    }
}

impl BrowserLaunch {
    pub fn new(
        batch_size: usize,
        navigation_timeout_ms: u64,
        dom_poll_interval_ms: u64,
        arguments: Vec<String>,
    ) -> Result<Self> {
        if batch_size == 0 || navigation_timeout_ms == 0 || dom_poll_interval_ms == 0 {
            return Err(invalid(
                "validate browser launch",
                "batch size and timing values must be positive",
            ));
        }
        let mut keys = BTreeSet::new();
        let mut normalized = Vec::with_capacity(arguments.len());
        for argument in arguments {
            let value = argument.strip_prefix("--").unwrap_or(&argument);
            let key = value.split_once('=').map_or(value, |(key, _)| key);
            const FORBIDDEN: &[&str] = &[
                "user-data-dir",
                "disk-cache-dir",
                "media-cache-dir",
                "data-path",
                "homedir",
                "log-file",
                "log-net-log",
                "crash-dumps-dir",
                "crash-dump-dir",
                "breakpad-dump-location",
                "download-default-directory",
                "ssl-key-log-file",
                "trace-startup-file",
                "profiling-file",
                "print-to-pdf",
                "screenshot",
                "remote-debugging-port",
                "remote-debugging-address",
                "remote-debugging-pipe",
                "disable-extensions",
                "load-extension",
                "disable-extensions-except",
            ];
            if key.is_empty()
                || key.starts_with('-')
                || !value.bytes().all(|b| (0x20..=0x7e).contains(&b))
                || value.contains(['/', '\\'])
                || FORBIDDEN.contains(&key)
                || !keys.insert(key.to_owned())
            {
                return Err(invalid(
                    "validate browser launch",
                    "duplicate, malformed, redirecting, or driver-owned launch switch",
                ));
            }
            normalized.push(value.to_owned());
        }
        Ok(Self {
            batch_size,
            navigation_timeout_ms,
            dom_poll_interval_ms,
            arguments: normalized,
        })
    }
    #[must_use]
    pub const fn batch_size(&self) -> usize {
        self.batch_size
    }
    #[must_use]
    pub const fn navigation_timeout_ms(&self) -> u64 {
        self.navigation_timeout_ms
    }
    #[must_use]
    pub const fn dom_poll_interval_ms(&self) -> u64 {
        self.dom_poll_interval_ms
    }
    #[must_use]
    pub fn arguments(&self) -> &[String] {
        &self.arguments
    }
    pub(super) fn digest(&self) -> Result<Sha256Digest> {
        serde_json::to_vec(&(
            1_u8,
            self,
            "sorted-sequential",
            "one-retry",
            "per-batch-and-retry",
            "per-job",
            true,
            true,
        ))
        .map(Sha256Digest::from_bytes)
        .map_err(|error| invalid("hash normalized browser launch", error.to_string()))
    }
}

/// A browser pin and its owner-relative cache, independent of fixture semantics.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawSettings")]
pub struct BrowserSettings {
    pub(super) source: String,
    pub(super) version: String,
    pub(super) version_output: String,
    pub(super) cache_root: RelativePath,
    pub(super) provenance_format: String,
    pub(super) launch: BrowserLaunch,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSettings {
    source: String,
    version: String,
    version_output: String,
    cache_root: RelativePath,
    provenance_format: String,
    launch: BrowserLaunch,
}
impl TryFrom<RawSettings> for BrowserSettings {
    type Error = GeneratorError;
    fn try_from(raw: RawSettings) -> Result<Self> {
        Self::new(
            raw.source,
            raw.version,
            raw.version_output,
            raw.cache_root,
            raw.provenance_format,
            raw.launch,
        )
    }
}

impl BrowserSettings {
    pub fn new(
        source: String,
        version: String,
        version_output: String,
        cache_root: RelativePath,
        provenance_format: String,
        launch: BrowserLaunch,
    ) -> Result<Self> {
        text(&source, "browser source")?;
        text(&version, "browser version")?;
        text(&version_output, "browser version output")?;
        if cache_root
            .as_str()
            .split('/')
            .any(|part| part == ".surgeist-generator" || part.starts_with("._surgeist-"))
        {
            return Err(invalid(
                "validate browser cache",
                "reserved cache component",
            ));
        }
        if provenance_format.matches("{version}").count() != 1
            || provenance_format
                .matches("{repository_relative_executable}")
                .count()
                != 1
        {
            return Err(invalid(
                "validate browser provenance",
                "provenance requires one version and one executable placeholder",
            ));
        }
        text(&provenance_format, "browser provenance format")?;
        Ok(Self {
            source,
            version,
            version_output,
            cache_root,
            provenance_format,
            launch,
        })
    }
    #[must_use]
    pub fn source(&self) -> &str {
        &self.source
    }
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }
    #[must_use]
    pub fn version_output(&self) -> &str {
        &self.version_output
    }
    #[must_use]
    pub const fn cache_root(&self) -> &RelativePath {
        &self.cache_root
    }
    #[must_use]
    pub fn provenance_format(&self) -> &str {
        &self.provenance_format
    }
    #[must_use]
    pub const fn launch(&self) -> &BrowserLaunch {
        &self.launch
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct EngineManifest {
    pub(super) browser: BrowserSettings,
    pub(super) launch_digest: Sha256Digest,
    pub(super) browser_owner: PathBuf,
}
impl EngineManifest {
    pub(super) fn new(browser: BrowserSettings, browser_owner: PathBuf) -> Result<Self> {
        Ok(Self {
            launch_digest: browser.launch.digest()?,
            browser,
            browser_owner,
        })
    }
    pub(super) fn validate(&self) -> Result<()> {
        if self.browser.launch.digest()? != self.launch_digest {
            return Err(invalid(
                "validate browser journal configuration",
                "launch digest differs from normalized configuration",
            ));
        }
        Ok(())
    }
}

/// One adapter-owned case and its corpus-relative output, including the output root.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CaseSpec {
    id: String,
    variant: String,
    output: RelativePath,
}
impl CaseSpec {
    pub fn new(id: String, variant: String, output: RelativePath) -> Result<Self> {
        text(&id, "case ID")?;
        text(&variant, "case variant")?;
        Ok(Self {
            id,
            variant,
            output,
        })
    }
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
    #[must_use]
    pub fn variant(&self) -> &str {
        &self.variant
    }
    #[must_use]
    pub const fn output(&self) -> &RelativePath {
        &self.output
    }
}

/// A manifest's fixture-level disposition. Runtime unsupported cases are separate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum FixtureStatus {
    Active,
    ExpectedFail { reason: String },
    Unsupported { reason: String },
    Quarantined { reason: String },
}
impl FixtureStatus {
    pub(super) fn schedules_browser(&self) -> bool {
        matches!(self, Self::Active | Self::ExpectedFail { .. })
    }
    fn validate(&self) -> Result<()> {
        match self {
            Self::Active => Ok(()),
            Self::ExpectedFail { reason }
            | Self::Unsupported { reason }
            | Self::Quarantined { reason } => text(reason, "disposition reason"),
        }
    }
}

/// One source fixture with its complete potential case inventory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FixtureSpec {
    name: String,
    source: RelativePath,
    cases: Vec<CaseSpec>,
    status: FixtureStatus,
}
impl FixtureSpec {
    pub fn new(
        name: String,
        source: RelativePath,
        cases: Vec<CaseSpec>,
        status: FixtureStatus,
    ) -> Result<Self> {
        text(&name, "fixture name")?;
        status.validate()?;
        if cases.is_empty() {
            return Err(invalid(
                "declare fixture",
                "fixture must declare at least one case",
            ));
        }
        let mut ids = BTreeSet::new();
        let mut outputs = BTreeSet::new();
        for case in &cases {
            if !ids.insert(case.id()) || !outputs.insert(case.output()) {
                return Err(invalid(
                    "declare fixture",
                    "duplicate case identity or output",
                ));
            }
        }
        Ok(Self {
            name,
            source,
            cases,
            status,
        })
    }
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub const fn source(&self) -> &RelativePath {
        &self.source
    }
    #[must_use]
    pub fn cases(&self) -> &[CaseSpec] {
        &self.cases
    }
    #[must_use]
    pub const fn status(&self) -> &FixtureStatus {
        &self.status
    }
}

/// Captured fixture input supplied to the synchronous adapter.
pub struct FixtureInput<'a> {
    pub(super) fixture: &'a FixtureSpec,
    pub(super) bytes: &'a [u8],
    pub(super) base_url: &'a str,
    pub(super) inputs: &'a BTreeMap<RelativePath, Vec<u8>>,
}
impl FixtureInput<'_> {
    #[must_use]
    pub const fn fixture(&self) -> &FixtureSpec {
        self.fixture
    }
    #[must_use]
    pub const fn source_bytes(&self) -> &[u8] {
        self.bytes
    }
    #[must_use]
    pub const fn base_url(&self) -> &str {
        self.base_url
    }
    #[must_use]
    pub fn input_bytes(&self, path: &RelativePath) -> Option<&[u8]> {
        self.inputs.get(path).map(Vec::as_slice)
    }
}

/// Resource discovery may be invalid for a fixture that proves wholly unsupported.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourceDependencies {
    Paths(Vec<RelativePath>),
    Invalid { reason: String },
}

/// One page job. Expressions are interpreted by the trusted browser, never Rust.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedFixture {
    pub(super) document: String,
    pub(super) readiness_expression: String,
    pub(super) setup_expression: String,
    pub(super) measurement_expression: String,
    pub(super) resources: ResourceDependencies,
}
impl PreparedFixture {
    pub fn new(
        document: String,
        readiness_expression: String,
        setup_expression: String,
        measurement_expression: String,
        resources: ResourceDependencies,
    ) -> Result<Self> {
        if readiness_expression.trim().is_empty() || measurement_expression.trim().is_empty() {
            return Err(invalid(
                "prepare browser fixture",
                "readiness and measurement expressions must be nonempty",
            ));
        }
        Ok(Self {
            document,
            readiness_expression,
            setup_expression,
            measurement_expression,
            resources,
        })
    }
}

/// A complete case result produced by a downstream domain adapter.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CaseOutcome {
    Generated { case_id: String, bytes: Vec<u8> },
    Unsupported { case_id: String, reason: String },
}
impl CaseOutcome {
    pub(super) fn case_id(&self) -> &str {
        match self {
            Self::Generated { case_id, .. } | Self::Unsupported { case_id, .. } => case_id,
        }
    }
}

/// Intentional downstream fixture-domain boundary; browser execution stays owned.
pub trait BrowserCorpusAdapter: Send + 'static {
    type Error: Error + Send + Sync + 'static;
    fn prepare(&self, input: FixtureInput<'_>)
    -> std::result::Result<PreparedFixture, Self::Error>;
    fn lower(
        &self,
        fixture: &FixtureSpec,
        measurement: Value,
    ) -> std::result::Result<Vec<CaseOutcome>, Self::Error>;
}

/// A full or source-prefix scoped report relative to the corpus output root.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReportScope {
    pub(super) path: RelativePath,
    pub(super) filter: Option<RelativePath>,
    pub(super) expected_generated: Option<usize>,
}
impl ReportScope {
    pub fn new(path: RelativePath, filter: Option<RelativePath>) -> Result<Self> {
        Ok(Self {
            path,
            filter,
            expected_generated: None,
        })
    }
    pub fn with_expected_generated(mut self, count: usize) -> Result<Self> {
        if self.filter.is_none() {
            return Err(invalid(
                "declare scoped report count",
                "full report uses complete expected counts",
            ));
        }
        self.expected_generated = Some(count);
        Ok(self)
    }
    #[must_use]
    pub const fn path(&self) -> &RelativePath {
        &self.path
    }
    #[must_use]
    pub const fn filter(&self) -> Option<&RelativePath> {
        self.filter.as_ref()
    }
}

/// Independent exact roots for corpus publication and an owner-relative browser cache.
#[derive(Clone, Debug)]
pub struct BrowserLocation {
    corpus: CorpusLocation,
    browser_owner: PathBuf,
}
impl BrowserLocation {
    pub fn new(browser_owner: &Path, corpus_root: &Path) -> Result<Self> {
        let owner = CorpusLocation::new(browser_owner, browser_owner)?;
        let corpus = CorpusLocation::new(corpus_root, corpus_root)?;
        Ok(Self {
            corpus,
            browser_owner: owner.owner_root().to_path_buf(),
        })
    }
    #[must_use]
    pub const fn corpus(&self) -> &CorpusLocation {
        &self.corpus
    }
    #[must_use]
    pub const fn location(&self) -> &CorpusLocation {
        &self.corpus
    }
    #[must_use]
    pub fn browser_owner(&self) -> &Path {
        &self.browser_owner
    }
}
impl From<CorpusLocation> for BrowserLocation {
    fn from(corpus: CorpusLocation) -> Self {
        Self {
            browser_owner: corpus.owner_root().to_path_buf(),
            corpus,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub(super) struct InputContract {
    pub(super) expected: BTreeMap<RelativePath, Sha256Digest>,
    pub(super) exact_roots: Vec<RelativePath>,
    pub(super) imports: Vec<super::source_import::SourceImportProvenance>,
}

/// Caller-declared domain policy checked before any browser or filesystem mutation.
#[derive(Clone, Debug)]
pub struct BrowserCorpus {
    pub(super) location: CorpusLocation,
    pub(super) browser_owner: PathBuf,
    pub(super) manifest_path: RelativePath,
    pub(super) generator: String,
    pub(super) output_root: RelativePath,
    pub(super) report_paths: Vec<ReportScope>,
    pub(super) browser: BrowserSettings,
    pub(super) fixtures: Vec<FixtureSpec>,
    pub(super) global_inputs: Vec<RelativePath>,
    pub(super) import_attestations: Vec<RelativePath>,
    pub(super) expected_counts: Option<super::report::BrowserReportSummary>,
    pub(super) input_contract: InputContract,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum DeclaredPathKind {
    Directory,
    File,
}

fn validate_declared_paths(paths: &BTreeSet<RelativePath>) -> Result<()> {
    let mut prefixes: BTreeMap<String, (&str, DeclaredPathKind)> = BTreeMap::new();
    for path in paths {
        let path = path.as_str();
        if path.split('/').any(|component| {
            component.eq_ignore_ascii_case(".surgeist-generator")
                || component.to_ascii_lowercase().starts_with("._surgeist-")
        }) {
            return Err(invalid(
                "declare browser corpus",
                "declared path uses a generator-reserved component",
            ));
        }
        for (end, kind) in path
            .match_indices('/')
            .map(|(index, _)| (index, DeclaredPathKind::Directory))
            .chain(std::iter::once((path.len(), DeclaredPathKind::File)))
        {
            let prefix = &path[..end];
            let key = prefix.to_ascii_lowercase();
            if let Some((prior, prior_kind)) = prefixes.get(&key) {
                if *prior != prefix
                    || kind != DeclaredPathKind::Directory
                    || *prior_kind != DeclaredPathKind::Directory
                {
                    return Err(invalid(
                        "declare browser corpus",
                        format!(
                            "declared paths alias or overlap as files and directories: {prior}, {prefix}"
                        ),
                    ));
                }
            } else {
                prefixes.insert(key, (prefix, kind));
            }
        }
    }
    Ok(())
}

impl BrowserCorpus {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        location: impl Into<BrowserLocation>,
        manifest_path: RelativePath,
        generator: String,
        output_root: RelativePath,
        report_paths: Vec<ReportScope>,
        browser: BrowserSettings,
        mut fixtures: Vec<FixtureSpec>,
        global_inputs: Vec<RelativePath>,
        import_attestations: Vec<RelativePath>,
    ) -> Result<Self> {
        let location = location.into();
        if !crate::core::validate_identifier(&generator)
            || output_root.as_str().contains('/')
            || output_root.as_str().starts_with('.')
        {
            return Err(invalid(
                "declare browser corpus",
                "invalid generator identity or output root",
            ));
        }
        if report_paths
            .iter()
            .filter(|scope| scope.filter.is_none())
            .count()
            != 1
        {
            return Err(invalid(
                "declare browser corpus",
                "exactly one full report is required",
            ));
        }
        let mut outputs = BTreeSet::from([RelativePath::new(format!(
            "{}/generation-ownership.json",
            output_root.as_str()
        ))?]);
        let mut sources = BTreeSet::new();
        for report in &report_paths {
            if !outputs.insert(RelativePath::new(format!(
                "{}/{}",
                output_root.as_str(),
                report.path.as_str()
            ))?) {
                return Err(invalid("declare browser corpus", "duplicate report path"));
            }
        }
        for fixture in &fixtures {
            if !sources.insert(fixture.source.clone()) {
                return Err(invalid("declare browser corpus", "duplicate source path"));
            }
            for case in &fixture.cases {
                if !case
                    .output
                    .as_str()
                    .starts_with(&format!("{}/", output_root.as_str()))
                {
                    return Err(invalid(
                        "declare browser corpus",
                        "case output is outside output root",
                    ));
                }
                if !outputs.insert(case.output.clone()) {
                    return Err(invalid(
                        "declare browser corpus",
                        "duplicate case ID or publication path",
                    ));
                }
            }
        }
        let prefix = format!("{}/", output_root.as_str());
        let mut declared_paths = outputs;
        for input in global_inputs
            .iter()
            .chain(&import_attestations)
            .chain(sources.iter())
            .chain(std::iter::once(&manifest_path))
        {
            if input.as_str() == output_root.as_str()
                || input.as_str().starts_with(&prefix)
                || input
                    .as_str()
                    .split('/')
                    .any(|part| part == ".surgeist-generator" || part.starts_with("._surgeist-"))
            {
                return Err(invalid(
                    "declare browser corpus",
                    "protected input overlaps publication or coordination",
                ));
            }
            declared_paths.insert(input.clone());
        }
        validate_declared_paths(&declared_paths)?;
        fixtures.sort_by(|a, b| a.source.cmp(&b.source));
        Ok(Self {
            location: location.corpus,
            browser_owner: location.browser_owner,
            manifest_path,
            generator,
            output_root,
            report_paths,
            browser,
            fixtures,
            global_inputs,
            import_attestations,
            expected_counts: None,
            input_contract: InputContract::default(),
        })
    }
    /// Binds previously parsed or independently verified declarations to captured inputs.
    pub fn with_expected_inputs(
        mut self,
        expected: BTreeMap<RelativePath, Sha256Digest>,
    ) -> Result<Self> {
        let declared: BTreeSet<_> = std::iter::once(&self.manifest_path)
            .chain(&self.global_inputs)
            .chain(&self.import_attestations)
            .chain(self.fixtures.iter().map(FixtureSpec::source))
            .collect();
        if expected.keys().any(|path| !declared.contains(path)) {
            return Err(invalid(
                "bind verified corpus inputs",
                "expected digest path is not a declared input",
            ));
        }
        for (path, digest) in expected {
            if self
                .input_contract
                .expected
                .get(&path)
                .is_some_and(|prior| prior != &digest)
            {
                return Err(invalid(
                    "bind verified corpus inputs",
                    "verified input digests disagree",
                ));
            }
            self.input_contract.expected.insert(path, digest);
        }
        Ok(self)
    }
    /// Records source pins and canonical import receipts, binding their exact input bytes.
    pub fn with_import_provenance(
        mut self,
        mut imports: Vec<super::source_import::SourceImportProvenance>,
    ) -> Result<Self> {
        imports.sort_by(|a, b| a.sidecar().cmp(b.sidecar()));
        let paths: BTreeSet<_> = imports.iter().map(|import| import.sidecar()).collect();
        if paths.len() != imports.len() || paths != self.import_attestations.iter().collect() {
            return Err(invalid(
                "declare source import provenance",
                "provenance must cover every declared import attestation exactly once",
            ));
        }
        let mut expected = self.input_contract.expected.clone();
        for import in &imports {
            let imported = import
                .files()
                .iter()
                .map(|file| {
                    RelativePath::new(format!(
                        "{}/{}",
                        import.specification().destination().as_str(),
                        file.path().as_str()
                    ))
                    .map(|path| (path, file.sha256().clone()))
                })
                .collect::<Result<Vec<_>>>()?;
            for (path, digest) in imported
                .into_iter()
                .chain([(import.sidecar().clone(), import.receipt_sha256().clone())])
            {
                if expected
                    .get(&path)
                    .is_some_and(|previous| previous != &digest)
                {
                    return Err(invalid(
                        "declare source import provenance",
                        "verified input digests disagree",
                    ));
                }
                expected.insert(path, digest);
            }
        }
        self = self.with_expected_inputs(expected)?;
        self.input_contract.imports = imports;
        Ok(self)
    }
    /// Requires these helper directories to contain exactly their declared global inputs.
    pub fn with_exact_input_roots(mut self, mut roots: Vec<RelativePath>) -> Result<Self> {
        roots.sort();
        for (index, root) in roots.iter().enumerate() {
            if root
                .as_str()
                .split('/')
                .any(|part| part == ".surgeist-generator" || part.starts_with("._surgeist-"))
                || selected(root, &self.output_root)
                || selected(&self.output_root, root)
                || roots[..index].iter().any(|prior| selected(root, prior))
                || !self
                    .global_inputs
                    .iter()
                    .any(|path| path != root && selected(path, root))
                || std::iter::once(&self.manifest_path)
                    .chain(&self.import_attestations)
                    .chain(self.fixtures.iter().map(FixtureSpec::source))
                    .any(|path| selected(path, root))
            {
                return Err(invalid(
                    "declare exact input roots",
                    "roots must be disjoint helper directories containing declared global inputs",
                ));
            }
        }
        self.input_contract.exact_roots = roots;
        Ok(self)
    }
    #[must_use]
    pub fn with_expected_counts(mut self, counts: super::report::BrowserReportSummary) -> Self {
        self.expected_counts = Some(counts);
        self
    }
    #[must_use]
    pub const fn location(&self) -> &CorpusLocation {
        &self.location
    }
    #[must_use]
    pub fn browser_owner(&self) -> &Path {
        &self.browser_owner
    }
    #[must_use]
    pub const fn browser(&self) -> &BrowserSettings {
        &self.browser
    }
    #[must_use]
    pub fn fixtures(&self) -> &[FixtureSpec] {
        &self.fixtures
    }
    #[must_use]
    pub const fn output_root(&self) -> &RelativePath {
        &self.output_root
    }
}

/// Exact legacy report evidence and artifact ownership, valid only for migration.
#[derive(Clone, Debug)]
pub struct PriorOwnership {
    pub(super) evidence: Vec<(RelativePath, Sha256Digest)>,
    pub(super) artifacts: Vec<(RelativePath, Sha256Digest)>,
}
impl PriorOwnership {
    pub fn new(
        evidence: Vec<(RelativePath, Sha256Digest)>,
        artifacts: Vec<(RelativePath, Sha256Digest)>,
    ) -> Result<Self> {
        if evidence.is_empty() {
            return Err(invalid(
                "declare prior ownership",
                "legacy report evidence is required",
            ));
        }
        let mut paths = BTreeSet::new();
        for (path, _) in evidence.iter().chain(&artifacts) {
            if !paths.insert(path) {
                return Err(invalid(
                    "declare prior ownership",
                    "duplicate legacy ownership path",
                ));
            }
        }
        Ok(Self {
            evidence,
            artifacts,
        })
    }
}

/// Existing-browser generation request. Acquisition is a separate operation.
#[derive(Clone, Debug)]
pub struct GenerationRequest {
    pub(super) corpus: BrowserCorpus,
    pub(super) browser_path: RelativePath,
    pub(super) filter: Option<RelativePath>,
    pub(super) prior: Option<PriorOwnership>,
}
impl GenerationRequest {
    pub fn new(
        corpus: BrowserCorpus,
        browser_path: RelativePath,
        filter: Option<RelativePath>,
        prior: Option<PriorOwnership>,
    ) -> Result<Self> {
        if filter.is_some() && prior.is_some() {
            return Err(invalid(
                "construct generation request",
                "legacy ownership cannot authorize filtered generation",
            ));
        }
        if let Some(filter) = &filter
            && !corpus
                .fixtures
                .iter()
                .any(|fixture| selected(fixture.source(), filter))
        {
            return Err(invalid(
                "construct generation request",
                "filter selects no fixture",
            ));
        }
        Ok(Self {
            corpus,
            browser_path,
            filter,
            prior,
        })
    }
}
pub(super) fn selected(source: &RelativePath, filter: &RelativePath) -> bool {
    source == filter
        || source
            .as_str()
            .strip_prefix(filter.as_str())
            .is_some_and(|rest| rest.starts_with('/'))
}

/// Infrastructure errors preserve their category; adapter errors retain their source.
#[derive(Debug)]
#[non_exhaustive]
pub enum GenerationError<E: Error + 'static> {
    Generator(GeneratorError),
    Adapter {
        source: RelativePath,
        stage: &'static str,
        error: E,
    },
}
impl<E: Error + 'static> From<GeneratorError> for GenerationError<E> {
    fn from(error: GeneratorError) -> Self {
        Self::Generator(error)
    }
}
impl<E: Error + 'static> fmt::Display for GenerationError<E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Generator(error) => error.fmt(f),
            Self::Adapter {
                source,
                stage,
                error,
            } => write!(f, "{stage} {}: {error}", source.as_str()),
        }
    }
}
impl<E: Error + 'static> Error for GenerationError<E> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(match self {
            Self::Generator(error) => error,
            Self::Adapter { error, .. } => error,
        })
    }
}

#[cfg(test)]
#[path = "model_path_tests.rs"]
mod path_tests;
