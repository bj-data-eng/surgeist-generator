use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use super::browser_runtime::TrustedBrowser;
use super::measurement::{self, BrowserExecution};
use super::model::{
    BrowserCorpus, BrowserCorpusAdapter, CaseOutcome, EngineManifest, FixtureInput, FixtureSpec,
    FixtureStatus, GenerationError, GenerationRequest, PreparedFixture, ResourceDependencies,
    invalid, selected,
};
use super::profile::classify_pending;
use super::report::{
    self, BrowserGeneratedArtifact, BrowserGenerationReport, Disposition, Metadata,
};
use crate::core::{
    ArtifactPlan, ArtifactReservation, CORPUS_FILE_MODE, Domain, GenerationCheck, GenerationLease,
    HeldIdentity, Inventory, InventoryPolicy, NamespaceDisjointness, NodeKind,
    PublicationInventory, PublicationPolicy, RootedFs,
};
use crate::{GeneratorError, GeneratorErrorKind, RelativePath, Result, RunScope, Sha256Digest};

const OWNERSHIP_FILE: &str = "generation-ownership.json";
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Ownership {
    schema_version: u8,
    artifacts: BTreeMap<RelativePath, Sha256Digest>,
}
impl Ownership {
    fn bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = serde_json::to_vec_pretty(self)
            .map_err(|error| invalid("serialize output ownership", error.to_string()))?;
        bytes.push(b'\n');
        Ok(bytes)
    }
    fn parse(bytes: &[u8]) -> Result<Self> {
        let value: Self = serde_json::from_slice(bytes)
            .map_err(|error| invalid("parse output ownership", error.to_string()))?;
        if value.schema_version != 1 || value.bytes()? != bytes {
            return Err(invalid(
                "parse output ownership",
                "noncanonical ownership record",
            ));
        }
        Ok(value)
    }
}

pub(super) struct GenerationHost {
    pub(super) executable: PathBuf,
    pub(super) execution: BrowserExecution,
    digest: Sha256Digest,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CapturedFile {
    identity: HeldIdentity,
    bytes: Vec<u8>,
    digest: Sha256Digest,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct Snapshot {
    files: BTreeMap<RelativePath, CapturedFile>,
    exact_roots: BTreeMap<RelativePath, Inventory>,
}
impl Snapshot {
    fn capture(rooted: &RootedFs, paths: impl IntoIterator<Item = RelativePath>) -> Result<Self> {
        let mut files = BTreeMap::new();
        for path in paths {
            if files.contains_key(&path) {
                continue;
            }
            let identity = rooted.identity_at(path.as_str())?.ok_or_else(|| {
                invalid("capture corpus input", format!("missing {}", path.as_str()))
            })?;
            if identity.kind() != NodeKind::Regular {
                return Err(invalid(
                    "capture corpus input",
                    "input must be a regular file",
                ));
            }
            let bytes = rooted.read_file(path.as_str(), CORPUS_FILE_MODE)?;
            if rooted.identity_at(path.as_str())?.as_ref() != Some(&identity) {
                return Err(invalid(
                    "capture corpus input",
                    "input identity changed during read",
                ));
            }
            let digest = Sha256Digest::from_bytes(&bytes);
            files.insert(
                path,
                CapturedFile {
                    identity,
                    bytes,
                    digest,
                },
            );
        }
        Ok(Self {
            files,
            exact_roots: BTreeMap::new(),
        })
    }
    fn revalidate(&self, rooted: &RootedFs) -> Result<()> {
        if Self::capture(rooted, self.files.keys().cloned())?.files != self.files {
            return Err(invalid(
                "revalidate corpus inputs",
                "input bytes or identity changed",
            ));
        }
        for (path, expected) in &self.exact_roots {
            if Inventory::scan(rooted, path.as_str(), InventoryPolicy::FinalCorpus)?.as_ref()
                != Some(expected)
            {
                return Err(invalid(
                    "revalidate exact input inventory",
                    "declared helper directory changed",
                ));
            }
        }
        Ok(())
    }
    fn digests(&self) -> BTreeMap<RelativePath, Sha256Digest> {
        self.files
            .iter()
            .map(|(path, file)| (path.clone(), file.digest.clone()))
            .collect()
    }
    fn bytes(&self) -> BTreeMap<RelativePath, Vec<u8>> {
        self.files
            .iter()
            .map(|(path, file)| (path.clone(), file.bytes.clone()))
            .collect()
    }
}

pub(super) struct PreparedInput {
    fixture: FixtureSpec,
    job: PreparedFixture,
    source_digest: Sha256Digest,
    resources: Result<Snapshot>,
}
impl PreparedInput {
    pub(super) fn source(&self) -> &RelativePath {
        self.fixture.source()
    }
    pub(super) const fn job(&self) -> &PreparedFixture {
        &self.job
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Historical {
    inventory: Option<Inventory>,
    snapshot: Snapshot,
    classified: BTreeSet<RelativePath>,
    report: Option<BrowserGenerationReport>,
}

/// Runs an existing-browser generation with generator-owned runtime and cleanup.
pub fn generate<A: BrowserCorpusAdapter>(
    request: GenerationRequest,
    adapter: A,
) -> std::result::Result<BrowserGenerationReport, GenerationError<A::Error>> {
    let executable = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map_err(|error| invalid("resolve generation host", error.to_string()))?;
    let digest = Sha256Digest::from_bytes(
        std::fs::read(&executable)
            .map_err(|error| invalid("hash generation host", error.to_string()))?,
    );
    run_with_host(
        request,
        adapter,
        GenerationHost {
            executable,
            digest,
            execution: BrowserExecution::Production,
        },
    )
}

#[cfg(test)]
pub(super) fn run_with_test_host<A: BrowserCorpusAdapter>(
    request: GenerationRequest,
    adapter: A,
    execution: super::measurement::TestGenerationHost,
) -> std::result::Result<BrowserGenerationReport, GenerationError<A::Error>> {
    let executable = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map_err(|error| invalid("resolve test host", error.to_string()))?;
    let digest = Sha256Digest::from_bytes(
        std::fs::read(&executable).map_err(|error| invalid("hash test host", error.to_string()))?,
    );
    run_with_host(
        request,
        adapter,
        GenerationHost {
            executable,
            digest,
            execution: BrowserExecution::Test(execution),
        },
    )
}

fn run_with_host<A: BrowserCorpusAdapter>(
    request: GenerationRequest,
    adapter: A,
    host: GenerationHost,
) -> std::result::Result<BrowserGenerationReport, GenerationError<A::Error>> {
    let worker = std::thread::Builder::new()
        .name("surgeist-browser-generation".to_owned())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(1)
                .enable_all()
                .build()
                .map_err(|error| invalid("create browser runtime", error.to_string()))?;
            runtime.block_on(generate_inner(request, adapter, host))
        })
        .map_err(|error| invalid("spawn browser generation", error.to_string()))?;
    match worker.join() {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

fn adapter_failure_reason(display: &str, stage: &str) -> String {
    let reason = display.trim();
    if reason.is_empty() {
        format!("adapter {stage} failed")
    } else {
        reason.to_owned()
    }
}

async fn generate_inner<A: BrowserCorpusAdapter>(
    request: GenerationRequest,
    adapter: A,
    host: GenerationHost,
) -> std::result::Result<BrowserGenerationReport, GenerationError<A::Error>> {
    let corpus = &request.corpus;
    let manifest = EngineManifest::new(corpus.browser.clone(), corpus.browser_owner.clone())?;
    let browser = TrustedBrowser::validate(&corpus.location, &manifest, &request.browser_path)?;
    let rooted = RootedFs::open_corpus(&corpus.location)?;
    let inputs = capture_inputs(&rooted, corpus)?;
    let input_bytes = inputs.bytes();
    let historical = inspect_historical(&rooted, corpus, request.prior.as_ref())?;
    if request.filter.is_some() && historical.report.is_none() {
        return Err(invalid(
            "authorize filtered generation",
            "a version-four full report must establish output ownership",
        )
        .into());
    }
    let fixtures: Vec<_> = corpus
        .fixtures
        .iter()
        .filter(|fixture| {
            request
                .filter
                .as_ref()
                .is_none_or(|filter| selected(fixture.source(), filter))
        })
        .collect();
    let mut prepared = Vec::new();
    let mut first_error = None;
    let mut fixture_failures = Vec::new();
    for fixture in &fixtures {
        if !fixture.status().schedules_browser() {
            continue;
        }
        let source = &inputs.files[fixture.source()];
        let source_path = fixture.source().join(corpus.location.corpus_root());
        let base_url =
            url::Url::from_directory_path(source_path.parent().expect("source has corpus parent"))
                .map_err(|()| {
                    invalid(
                        "prepare fixture URL",
                        "source parent cannot form a file URL",
                    )
                })?;
        let job = match adapter.prepare(FixtureInput {
            fixture,
            bytes: &source.bytes,
            base_url: base_url.as_str(),
            inputs: &input_bytes,
        }) {
            Ok(job) => job,
            Err(error) => {
                fixture_failures.push(Disposition {
                    name: fixture.name().to_owned(),
                    source: fixture.source().clone(),
                    variant: None,
                    reason: adapter_failure_reason(&error.to_string(), "prepare"),
                });
                if first_error.is_none() {
                    first_error = Some(GenerationError::Adapter {
                        source: fixture.source().clone(),
                        stage: "prepare",
                        error,
                    });
                }
                continue;
            }
        };
        let resources = capture_resources(&rooted, corpus, &job.resources);
        prepared.push(PreparedInput {
            fixture: (*fixture).clone(),
            job,
            source_digest: source.digest.clone(),
            resources,
        });
    }
    let reservation = ArtifactReservation::new(Domain::Browser)?;
    let output_directory = corpus.output_root.join(corpus.location.corpus_root());
    let stage_path = reservation
        .external_stage()
        .join(corpus.location.corpus_root());
    let mut protected_paths: Vec<_> = inputs
        .files
        .keys()
        .map(|path| path.join(corpus.location.corpus_root()))
        .collect();
    protected_paths.push(corpus.browser.cache_root.join(&corpus.browser_owner));
    protected_paths.push(browser.absolute_path().to_path_buf());
    protected_paths.push(host.executable.clone());
    // Candidate resource failures are intentionally deferred until lowering proves
    // there is a generated artifact that needs their provenance.
    for fixture in &prepared {
        if let Ok(resources) = &fixture.resources {
            protected_paths.extend(
                resources
                    .files
                    .keys()
                    .map(|path| path.join(corpus.location.corpus_root())),
            );
        }
    }
    let protected: Vec<_> = protected_paths
        .iter()
        .map(|path| ("captured generation input", path.as_path()))
        .collect();
    let protection = NamespaceDisjointness::for_mutation(
        &corpus.location,
        &[
            ("browser artifacts", output_directory.as_path()),
            ("browser transaction stage", stage_path.as_path()),
        ],
        &protected,
    )?;
    let scope = match &request.filter {
        Some(filter) => RunScope::Filtered(filter.clone()),
        None => RunScope::Full,
    };
    let lease = GenerationLease::acquire_with_revalidation(
        &corpus.location,
        Domain::Browser,
        &corpus.generator,
        &scope,
        "generate",
        |rooted| {
            let pending = classify_pending(rooted)?;
            #[cfg(test)]
            test_before_closing_revalidation(&host.execution, &corpus.location)?;
            protection.revalidate(rooted)?;
            inputs.revalidate(rooted)?;
            browser.closing_revalidate()?;
            historical.revalidate(rooted, corpus)?;
            if let Some(pending) = pending {
                #[cfg(test)]
                test_before_pending_profile_cleanup(&host.execution, rooted)?;
                pending.execute(rooted)?;
            }
            Ok(())
        },
    )?;
    let version = super::version::run_version_supervisor(
        &corpus.location,
        &lease,
        &browser,
        &manifest,
        &host,
    )
    .await?;
    if version != corpus.browser.version_output {
        return Err(GeneratorError::new(
            GeneratorErrorKind::SourceVerification,
            "authenticate browser version",
            format!(
                "expected {:?}, received {version:?}",
                corpus.browser.version_output
            ),
        )
        .into());
    }
    let scheduled: Vec<_> = prepared.iter().collect();
    let measurements = measurement::measure(
        measurement::MeasurementContext {
            location: &corpus.location,
            lease: &lease,
            browser: &browser,
            manifest: &manifest,
            current_executable: &host.executable,
            execution: &host.execution,
        },
        &scheduled,
    )
    .await?;
    browser.closing_revalidate()?;
    let metadata = Metadata {
        schema_version: 4,
        generator: corpus.generator.clone(),
        engine_version: env!("CARGO_PKG_VERSION").to_owned(),
        host_executable_sha256: host.digest.clone(),
        browser_executable: request.browser_path.clone(),
        browser_executable_sha256: browser.digest().clone(),
        browser_provenance: browser.provenance(&manifest),
        contract_sha256: report::contract_digest(corpus)?,
        inputs: inputs.digests(),
        source_imports: corpus.input_contract.imports.clone(),
    };
    let mut report = BrowserGenerationReport::new(metadata, request.filter.clone());
    report.failed_to_generate = fixture_failures;
    let mut artifacts = Vec::new();
    let mut used_resources = Vec::new();
    for fixture in &fixtures {
        record_manifest_disposition(&mut report, fixture);
    }
    for fixture in &prepared {
        if let Some(reason) = measurements.failures.get(fixture.source()) {
            report.failed_to_generate.push(Disposition {
                name: fixture.fixture.name().to_owned(),
                source: fixture.source().clone(),
                variant: None,
                reason: reason.clone(),
            });
            continue;
        }
        let measurement = measurements
            .outcomes
            .get(fixture.source())
            .cloned()
            .ok_or_else(|| invalid("lower fixture", "browser omitted a scheduled fixture"))?;
        let outcomes = match adapter.lower(&fixture.fixture, measurement) {
            Ok(outcomes) => outcomes,
            Err(error) => {
                report.failed_to_generate.push(Disposition {
                    name: fixture.fixture.name().to_owned(),
                    source: fixture.source().clone(),
                    variant: None,
                    reason: adapter_failure_reason(&error.to_string(), "lower"),
                });
                if first_error.is_none() {
                    first_error = Some(GenerationError::Adapter {
                        source: fixture.source().clone(),
                        stage: "lower",
                        error,
                    });
                }
                continue;
            }
        };
        validate_outcomes(&fixture.fixture, &outcomes)?;
        let generated = outcomes
            .iter()
            .any(|outcome| matches!(outcome, CaseOutcome::Generated { .. }));
        let resource_digests = if generated {
            let resources = match &fixture.resources {
                Ok(resources) => resources,
                Err(error) => {
                    report.failed_to_generate.push(Disposition {
                        name: fixture.fixture.name().to_owned(),
                        source: fixture.source().clone(),
                        variant: None,
                        reason: error.to_string(),
                    });
                    if first_error.is_none() {
                        first_error = Some(GenerationError::Generator(invalid(
                            "capture generated resource provenance",
                            error.to_string(),
                        )));
                    }
                    continue;
                }
            };
            resources.revalidate(lease.rooted())?;
            used_resources.push(resources);
            resources.digests()
        } else {
            BTreeMap::new()
        };
        let job_digest = Sha256Digest::from_bytes(
            serde_json::to_vec(&(
                &fixture.job.document,
                &fixture.job.readiness_expression,
                &fixture.job.setup_expression,
                &fixture.job.measurement_expression,
            ))
            .map_err(|error| invalid("hash prepared browser job", error.to_string()))?,
        );
        for outcome in outcomes {
            let case = fixture
                .fixture
                .cases()
                .iter()
                .find(|case| case.id() == outcome.case_id())
                .expect("validated case identity");
            match outcome {
                CaseOutcome::Generated { bytes, .. } => {
                    let output_digest = Sha256Digest::from_bytes(&bytes);
                    report.generated.push(BrowserGeneratedArtifact {
                        name: case.id().to_owned(),
                        source: fixture.source().clone(),
                        output: case.output().clone(),
                        variant: case.variant().to_owned(),
                        source_sha256: fixture.source_digest.clone(),
                        linked_resources: resource_digests.clone(),
                        job_sha256: job_digest.clone(),
                        output_sha256: output_digest,
                    });
                    artifacts.push((strip_output(corpus, case.output())?, bytes));
                }
                CaseOutcome::Unsupported { reason, .. } => report.unsupported.push(Disposition {
                    name: case.id().to_owned(),
                    source: fixture.source().clone(),
                    variant: Some(case.variant().to_owned()),
                    reason,
                }),
            }
        }
    }
    report.canonicalize();
    report.validate()?;
    let diagnostic = report.summary().failed_to_generate() != 0;
    if diagnostic && request.filter.is_some() {
        return Err(first_error.unwrap_or_else(|| {
            GeneratorError::new(
                GeneratorErrorKind::Generation,
                "generate filtered corpus",
                "a selected browser fixture exhausted its retry",
            )
            .into()
        }));
    }
    if !diagnostic && request.filter.is_none() {
        validate_expected_counts(corpus, &report)?;
    }
    if request.filter.is_none() {
        for scope in &corpus.report_paths {
            let scoped = scope
                .filter
                .as_ref()
                .map_or_else(|| report.clone(), |filter| report.scoped(filter));
            artifacts.push((scope.path.clone(), scoped.to_bytes()?));
        }
    }
    let mut classified = historical.classified.clone();
    classified.extend(
        corpus
            .fixtures
            .iter()
            .flat_map(|fixture| fixture.cases())
            .map(|case| strip_output(corpus, case.output()))
            .collect::<Result<Vec<_>>>()?,
    );
    classified.extend(corpus.report_paths.iter().map(|scope| scope.path.clone()));
    let mut retained: BTreeSet<_> = artifacts.iter().map(|(path, _)| path.clone()).collect();
    if request.filter.is_some() {
        let selected_outputs: BTreeSet<_> = fixtures
            .iter()
            .flat_map(|fixture| fixture.cases())
            .map(|case| strip_output(corpus, case.output()))
            .collect::<Result<_>>()?;
        retained.extend(historical.classified.difference(&selected_outputs).cloned());
    } else if diagnostic {
        retained.extend(historical.classified.iter().cloned());
    }
    let ownership_path = RelativePath::new(OWNERSHIP_FILE)?;
    let mut owned = BTreeMap::new();
    for (path, file) in &historical.snapshot.files {
        if let Ok(relative) = strip_output(corpus, path)
            && relative != ownership_path
            && retained.contains(&relative)
        {
            owned.insert(path.clone(), file.digest.clone());
        }
    }
    for (path, bytes) in &artifacts {
        owned.insert(output_path(corpus, path)?, Sha256Digest::from_bytes(bytes));
    }
    artifacts.push((
        ownership_path.clone(),
        Ownership {
            schema_version: 1,
            artifacts: owned,
        }
        .bytes()?,
    ));
    classified.insert(ownership_path.clone());
    retained.insert(ownership_path);
    let policy = if request.filter.is_some() {
        PublicationPolicy::Filtered
    } else if diagnostic {
        PublicationPolicy::DiagnosticFull
    } else {
        PublicationPolicy::CleanFull
    };
    let inventory = PublicationInventory::new(
        classified.into_iter().collect(),
        retained.into_iter().collect(),
        corpus
            .report_paths
            .iter()
            .map(|scope| scope.path.clone())
            .collect(),
    )?;
    let plan = ArtifactPlan::new(
        &corpus.location,
        Domain::Browser,
        &lease,
        corpus.output_root.clone(),
        policy,
        artifacts,
        inventory,
    )?
    .with_reservation(reservation)?;
    let revalidate = |rooted: &RootedFs| {
        protection.revalidate(rooted)?;
        inputs.revalidate(rooted)?;
        browser.closing_revalidate()?;
        historical.revalidate(rooted, corpus)?;
        for resources in &used_resources {
            resources.revalidate(rooted)?;
        }
        let current = std::fs::read(&host.executable)
            .map_err(|error| invalid("revalidate generation host", error.to_string()))?;
        if Sha256Digest::from_bytes(current) != host.digest {
            return Err(invalid(
                "revalidate generation host",
                "hosting executable changed",
            ));
        }
        Ok(())
    };
    #[cfg(test)]
    plan.install_with_revalidation_and_inter_scan_hook(revalidate, || {})?;
    #[cfg(not(test))]
    plan.install_with_revalidation(revalidate)?;
    if diagnostic {
        return Err(first_error.unwrap_or_else(|| {
            GeneratorError::new(
                GeneratorErrorKind::Generation,
                "generate browser corpus",
                "diagnostic full generation published failed fixture accounting",
            )
            .into()
        }));
    }
    Ok(report)
}

fn capture_inputs(rooted: &RootedFs, corpus: &BrowserCorpus) -> Result<Snapshot> {
    let mut snapshot = Snapshot::capture(
        rooted,
        std::iter::once(corpus.manifest_path.clone())
            .chain(corpus.global_inputs.iter().cloned())
            .chain(corpus.import_attestations.iter().cloned())
            .chain(
                corpus
                    .fixtures
                    .iter()
                    .map(|fixture| fixture.source().clone()),
            ),
    )?;
    for (path, expected) in &corpus.input_contract.expected {
        if snapshot.files.get(path).map(|file| &file.digest) != Some(expected) {
            return Err(invalid(
                "capture verified corpus input",
                format!(
                    "{} changed after declaration or source verification",
                    path.as_str()
                ),
            ));
        }
    }
    for root in &corpus.input_contract.exact_roots {
        let inventory = Inventory::scan(rooted, root.as_str(), InventoryPolicy::FinalCorpus)?
            .ok_or_else(|| {
                invalid(
                    "capture exact input inventory",
                    "declared helper directory is missing",
                )
            })?;
        let prefix = format!("{}/", root.as_str());
        let expected = corpus
            .global_inputs
            .iter()
            .filter_map(|path| path.as_str().strip_prefix(&prefix))
            .map(RelativePath::new)
            .collect::<Result<BTreeSet<_>>>()?;
        validate_inventory(Some(&inventory), &expected)?;
        snapshot.exact_roots.insert(root.clone(), inventory);
    }
    Ok(snapshot)
}
fn capture_resources(
    rooted: &RootedFs,
    corpus: &BrowserCorpus,
    dependencies: &ResourceDependencies,
) -> Result<Snapshot> {
    match dependencies {
        ResourceDependencies::Invalid { reason } => {
            Err(invalid("discover fixture resources", reason.clone()))
        }
        ResourceDependencies::Paths(paths) => {
            for path in paths {
                if path.as_str() == corpus.output_root.as_str()
                    || path
                        .as_str()
                        .starts_with(&format!("{}/", corpus.output_root.as_str()))
                    || path.as_str().split('/').any(|part| {
                        part == ".surgeist-generator" || part.starts_with("._surgeist-")
                    })
                {
                    return Err(invalid(
                        "capture fixture resources",
                        "resource overlaps publication or coordination",
                    ));
                }
            }
            Snapshot::capture(rooted, paths.iter().cloned())
        }
    }
}
fn validate_outcomes(fixture: &FixtureSpec, outcomes: &[CaseOutcome]) -> Result<()> {
    let mut seen = BTreeSet::new();
    for outcome in outcomes {
        if !fixture
            .cases()
            .iter()
            .any(|case| case.id() == outcome.case_id())
            || !seen.insert(outcome.case_id())
        {
            return Err(invalid(
                "validate adapter outcomes",
                "unknown or duplicate case outcome",
            ));
        }
        if let CaseOutcome::Unsupported { reason, .. } = outcome
            && (reason.trim().is_empty() || reason.trim() != reason)
        {
            return Err(invalid(
                "validate adapter outcome",
                "unsupported outcome requires a trimmed reason",
            ));
        }
    }
    if seen.len() != fixture.cases().len() {
        return Err(invalid(
            "validate adapter outcomes",
            "adapter omitted a declared case",
        ));
    }
    Ok(())
}
fn record_manifest_disposition(report: &mut BrowserGenerationReport, fixture: &FixtureSpec) {
    let (bucket, reason) = match fixture.status() {
        FixtureStatus::Active => return,
        FixtureStatus::ExpectedFail { reason } => (&mut report.expected_fail, reason),
        FixtureStatus::Unsupported { reason } => (&mut report.unsupported, reason),
        FixtureStatus::Quarantined { reason } => (&mut report.quarantined, reason),
    };
    bucket.push(Disposition {
        name: fixture.name().to_owned(),
        source: fixture.source().clone(),
        variant: None,
        reason: reason.clone(),
    });
}

fn strip_output(corpus: &BrowserCorpus, path: &RelativePath) -> Result<RelativePath> {
    path.as_str()
        .strip_prefix(&format!("{}/", corpus.output_root.as_str()))
        .ok_or_else(|| {
            invalid(
                "validate output ownership",
                "path is outside declared output root",
            )
        })
        .and_then(RelativePath::new)
}
fn output_path(corpus: &BrowserCorpus, path: &RelativePath) -> Result<RelativePath> {
    RelativePath::new(format!("{}/{}", corpus.output_root.as_str(), path.as_str()))
}

fn inspect_historical(
    rooted: &RootedFs,
    corpus: &BrowserCorpus,
    prior: Option<&super::model::PriorOwnership>,
) -> Result<Historical> {
    let inventory = Inventory::scan(
        rooted,
        corpus.output_root.as_str(),
        InventoryPolicy::FinalCorpus,
    )?;
    let mut expected = BTreeMap::new();
    let mut report = None;
    let ownership_path = output_path(corpus, &RelativePath::new(OWNERSHIP_FILE)?)?;
    if rooted.exists(ownership_path.as_str())? {
        if prior.is_some() {
            return Err(invalid(
                "inspect ownership",
                "legacy migration cannot replace existing generic ownership",
            ));
        }
        let bytes = rooted.read_file(ownership_path.as_str(), CORPUS_FILE_MODE)?;
        let ownership = Ownership::parse(&bytes)?;
        for (path, digest) in ownership.artifacts {
            strip_output(corpus, &path)?;
            if path == ownership_path {
                return Err(invalid(
                    "inspect ownership",
                    "ownership record cannot attest itself",
                ));
            }
            expected.insert(path, digest);
        }
        expected.insert(ownership_path, Sha256Digest::from_bytes(bytes));
        let full = corpus
            .report_paths
            .iter()
            .find(|scope| scope.filter.is_none())
            .expect("validated full report");
        let full_path = output_path(corpus, &full.path)?;
        if expected.contains_key(&full_path) {
            let parsed = BrowserGenerationReport::parse(
                &rooted.read_file(full_path.as_str(), CORPUS_FILE_MODE)?,
            )?;
            if parsed.filter.is_some() {
                return Err(invalid(
                    "inspect historical report",
                    "full report carries a filter",
                ));
            }
            report = Some(parsed);
        }
    } else if let Some(prior) = prior {
        for (path, digest) in &prior.evidence {
            expected.insert(path.clone(), digest.clone());
        }
        for (path, digest) in &prior.artifacts {
            strip_output(corpus, path)?;
            expected.insert(path.clone(), digest.clone());
        }
    }
    let snapshot = Snapshot::capture(rooted, expected.keys().cloned())?;
    for (path, digest) in &expected {
        if &snapshot.files[path].digest != digest {
            return Err(invalid(
                "inspect historical ownership",
                format!("{} differs from its attested digest", path.as_str()),
            ));
        }
    }
    let classified = expected
        .keys()
        .filter_map(|path| strip_output(corpus, path).ok())
        .collect::<BTreeSet<_>>();
    validate_inventory(inventory.as_ref(), &classified)?;
    Ok(Historical {
        inventory,
        snapshot,
        classified,
        report,
    })
}
impl Historical {
    fn revalidate(&self, rooted: &RootedFs, corpus: &BrowserCorpus) -> Result<()> {
        self.snapshot.revalidate(rooted)?;
        if Inventory::scan(
            rooted,
            corpus.output_root.as_str(),
            InventoryPolicy::FinalCorpus,
        )? != self.inventory
        {
            return Err(invalid(
                "revalidate publication ownership",
                "output inventory changed",
            ));
        }
        Ok(())
    }
}
fn validate_inventory(
    inventory: Option<&Inventory>,
    expected: &BTreeSet<RelativePath>,
) -> Result<()> {
    if let Some(inventory) = inventory {
        for entry in inventory.entries() {
            let admitted = match entry.identity().kind() {
                NodeKind::Regular => expected.contains(entry.path()),
                NodeKind::Directory => expected.iter().any(|path| {
                    path.as_str()
                        .starts_with(&format!("{}/", entry.path().as_str()))
                }),
                NodeKind::Symlink => false,
            };
            if !admitted {
                return Err(invalid(
                    "inspect declared inventory",
                    format!("unowned entry {}", entry.path().as_str()),
                ));
            }
        }
    }
    Ok(())
}

fn validate_expected_counts(
    corpus: &BrowserCorpus,
    report: &BrowserGenerationReport,
) -> Result<()> {
    if corpus
        .expected_counts
        .as_ref()
        .is_some_and(|expected| expected != report.summary())
    {
        return Err(invalid(
            "validate full report accounting",
            "generation counts differ from declared expectations",
        ));
    }
    for scope in &corpus.report_paths {
        if let (Some(filter), Some(expected)) = (&scope.filter, scope.expected_generated)
            && report.scoped(filter).summary().generated() != expected
        {
            return Err(invalid(
                "validate scoped report accounting",
                format!(
                    "{} generated count differs from declared expectation",
                    scope.path.as_str()
                ),
            ));
        }
    }
    Ok(())
}

/// Verifies current version-four artifact lineage entirely offline.
pub fn check_corpus<A: BrowserCorpusAdapter>(
    corpus: &BrowserCorpus,
    adapter: &A,
) -> std::result::Result<BrowserGenerationReport, GenerationError<A::Error>> {
    let inspect = || {
        let rooted = RootedFs::open_corpus(&corpus.location)?;
        let inputs = capture_inputs(&rooted, corpus)?;
        let input_bytes = inputs.bytes();
        let history = inspect_historical(&rooted, corpus, None)?;
        let report = history.report.clone().ok_or_else(|| {
            GeneratorError::new(
                GeneratorErrorKind::Verification,
                "check browser corpus",
                "current full report is absent; run full generation",
            )
        })?;
        if report.summary().failed_to_generate() != 0
            || report.metadata.inputs != inputs.digests()
            || report.metadata.contract_sha256 != report::contract_digest(corpus)?
        {
            return Err(GeneratorError::new(
                GeneratorErrorKind::Verification,
                "check browser corpus",
                "corpus report is diagnostic or stale against current inputs/contracts",
            )
            .into());
        }
        let browser_prefix = format!("{}/", corpus.browser.cache_root.as_str());
        let expected_provenance = corpus
            .browser
            .provenance_format
            .replace("{version}", &corpus.browser.version)
            .replace(
                "{repository_relative_executable}",
                report.metadata.browser_executable.as_str(),
            );
        if report.metadata.source_imports != corpus.input_contract.imports
            || report.metadata.generator != corpus.generator
            || !report
                .metadata
                .browser_executable
                .as_str()
                .starts_with(&browser_prefix)
            || report.metadata.browser_provenance != expected_provenance
        {
            return Err(invalid("check historical browser provenance","generator, cache-relative browser identity, or provenance differs from declared settings").into());
        }
        validate_expected_counts(corpus, &report)?;
        validate_case_accounting(corpus, &report)?;
        let mut current_outputs = report
            .generated
            .iter()
            .map(|entry| strip_output(corpus, &entry.output))
            .collect::<Result<BTreeSet<_>>>()?;
        current_outputs.extend(corpus.report_paths.iter().map(|scope| scope.path.clone()));
        current_outputs.insert(RelativePath::new(OWNERSHIP_FILE)?);
        // Repair ownership can outlive a filtered or diagnostic run. A clean
        // check admits only outputs accounted for by the current full report.
        validate_inventory(history.inventory.as_ref(), &current_outputs)?;
        for scope in &corpus.report_paths {
            let path = output_path(corpus, &scope.path)?;
            let actual = BrowserGenerationReport::parse(
                &rooted.read_file(path.as_str(), CORPUS_FILE_MODE)?,
            )?;
            let wanted = scope
                .filter
                .as_ref()
                .map_or_else(|| report.clone(), |filter| report.scoped(filter));
            if actual != wanted {
                return Err(invalid(
                    "check scoped report",
                    "scoped report differs from full report projection",
                )
                .into());
            }
        }
        let mut resources = Vec::new();
        for fixture in &corpus.fixtures {
            let entries: Vec<_> = report
                .generated
                .iter()
                .filter(|entry| &entry.source == fixture.source())
                .collect();
            if entries.is_empty() {
                continue;
            }
            let source = &inputs.files[fixture.source()];
            let source_path = fixture.source().join(corpus.location.corpus_root());
            let base_url =
                url::Url::from_directory_path(source_path.parent().expect("source parent"))
                    .map_err(|()| invalid("check fixture URL", "cannot construct source URL"))?;
            let job = adapter
                .prepare(FixtureInput {
                    fixture,
                    bytes: &source.bytes,
                    base_url: base_url.as_str(),
                    inputs: &input_bytes,
                })
                .map_err(|error| GenerationError::Adapter {
                    source: fixture.source().clone(),
                    stage: "check preparation",
                    error,
                })?;
            let snapshot = capture_resources(&rooted, corpus, &job.resources)?;
            for entry in entries {
                if snapshot.digests() != entry.linked_resources
                    || source.digest != entry.source_sha256
                    || history
                        .snapshot
                        .files
                        .get(&entry.output)
                        .map(|file| &file.digest)
                        != Some(&entry.output_sha256)
                {
                    return Err(invalid("check artifact lineage", "artifact, prepared job, source, or complete linked-resource lineage is stale").into());
                }
            }
            resources.push(snapshot);
        }
        Ok::<_, GenerationError<A::Error>>((inputs, history, resources, report))
    };
    let initial = inspect()?;
    let check = GenerationCheck::acquire(&corpus.location, Domain::Browser)?;
    let repeated = inspect();
    let finished = check.finish();
    finished?;
    let repeated = repeated?;
    if initial != repeated {
        return Err(invalid(
            "check browser corpus",
            "inputs or output ownership changed during verification",
        )
        .into());
    }
    Ok(initial.3)
}

fn validate_case_accounting(
    corpus: &BrowserCorpus,
    report: &BrowserGenerationReport,
) -> Result<()> {
    let mut wanted_generated = BTreeSet::new();
    let mut wanted_unsupported = BTreeSet::new();
    for fixture in &corpus.fixtures {
        if fixture.status().schedules_browser() {
            for case in fixture.cases() {
                let generated = report
                    .generated
                    .iter()
                    .find(|entry| &entry.source == fixture.source() && entry.name == case.id());
                let unsupported = report.unsupported.iter().find(|entry| {
                    &entry.source == fixture.source()
                        && entry.name == case.id()
                        && entry.variant.as_deref() == Some(case.variant())
                });
                match (generated, unsupported) {
                    (Some(entry), None)
                        if &entry.output == case.output() && entry.variant == case.variant() =>
                    {
                        wanted_generated.insert((fixture.source(), case.id()));
                    }
                    (None, Some(_)) => {
                        wanted_unsupported.insert((fixture.source(), case.id()));
                    }
                    _ => {
                        return Err(invalid(
                            "check case accounting",
                            "declared case does not have exactly one matching outcome",
                        ));
                    }
                }
            }
        }
    }
    if wanted_generated.len() != report.generated.len()
        || wanted_unsupported.len()
            != report
                .unsupported
                .iter()
                .filter(|entry| entry.variant.is_some())
                .count()
    {
        return Err(invalid(
            "check case accounting",
            "report contains undeclared outcomes",
        ));
    }
    let mut expected = BrowserGenerationReport::new(report.metadata.clone(), None);
    for fixture in &corpus.fixtures {
        record_manifest_disposition(&mut expected, fixture);
    }
    expected.canonicalize();
    let manifest_unsupported: Vec<_> = report
        .unsupported
        .iter()
        .filter(|entry| entry.variant.is_none())
        .cloned()
        .collect();
    if expected.expected_fail != report.expected_fail
        || expected.quarantined != report.quarantined
        || expected.unsupported != manifest_unsupported
    {
        return Err(invalid(
            "check manifest dispositions",
            "report disposition differs from corpus declarations",
        ));
    }
    Ok(())
}

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
mod tests;

#[cfg(test)]
fn test_before_closing_revalidation(
    execution: &BrowserExecution,
    location: &crate::CorpusLocation,
) -> Result<()> {
    if matches!(
        execution,
        BrowserExecution::Test(host)
            if host.plan() == super::measurement::TestBrowserPlan::ClosingRevalidationFailure
    ) {
        let path = location
            .corpus_root()
            .join("scripts/gentest/test_helper.js");
        std::fs::write(&path, b"synthetic protected-input drift\n").map_err(|source| {
            GeneratorError::with_source(
                GeneratorErrorKind::Io,
                "inject crate-owned closing-revalidation test drift",
                path.display().to_string(),
                source,
            )
        })?;
    }
    Ok(())
}

#[cfg(test)]
fn test_before_pending_profile_cleanup(
    execution: &BrowserExecution,
    rooted: &RootedFs,
) -> Result<()> {
    if !matches!(
        execution,
        BrowserExecution::Test(host)
            if host.plan() == super::measurement::TestBrowserPlan::ProfileIdentityDrift
    ) {
        return Ok(());
    }
    let name = rooted
        .list_dir(super::profile::PROFILE_PARENT)?
        .into_iter()
        .next()
        .ok_or_else(|| {
            invalid(
                "test profile drift",
                "pending profile disappeared before test drift",
            )
        })?;
    let path = format!("{}/{name}", super::profile::PROFILE_PARENT);
    let displaced = format!("{path}-displaced");
    let identity = rooted.identity_at(&path)?.ok_or_else(|| {
        invalid(
            "test profile drift",
            "pending profile identity disappeared before test drift",
        )
    })?;
    rooted.rename_exclusive_bound(&path, &displaced, &identity)?;
    rooted.create_dir_exclusive(&path, crate::core::PRIVATE_DIRECTORY_MODE)?;
    Ok(())
}
