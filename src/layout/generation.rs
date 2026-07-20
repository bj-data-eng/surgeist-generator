use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::Stdio;

use futures::FutureExt;
use tokio::io::AsyncReadExt;

#[cfg(test)]
use crate::core::PRIVATE_DIRECTORY_MODE;
use crate::core::{
    ArtifactPlan, ArtifactReservation, Domain, GenerationLease, NamespaceDisjointness,
    PublicationInventory, PublicationPolicy, RootedFs,
};
use crate::{GeneratorError, GeneratorErrorKind, RelativePath, Result, Sha256Digest};

use super::browser::TrustedBrowser;
use super::measurement::{BrowserExecution, MeasurementResults, VariantOutcome};
use super::profile::{
    ProfileAttempt, ProfileCreateContext, ProfileJournal, SupervisorTermination, classify_pending,
    resolve_terminalization,
};
use super::report::{GenerationLedger, GenerationMetadata, HistoricalAuthority};
use super::selection::{CurrentInputs, Fixture, FixtureDisposition, SelectionLedger};
use super::xml::{Provenance, Variant};
use super::{LayoutRequest, manifest, measurement, report, selection, supervisor};

const GENERATOR: &str = "surgeist-layout-generate";
const COMMAND: &str = "generate";
type GeneratedArtifact = (RelativePath, Vec<u8>);
type DerivedArtifacts = (GenerationLedger, Vec<GeneratedArtifact>);

struct GenerationHost {
    executable: PathBuf,
    execution: BrowserExecution,
}

pub(super) fn run(request: LayoutRequest) -> Result<()> {
    let executable = std::env::current_exe().map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::Generation,
            "resolve layout generator executable",
            "current executable is unavailable",
            source,
        )
    })?;
    let executable = std::fs::canonicalize(&executable).map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::Generation,
            "canonicalize layout generator executable",
            executable.display().to_string(),
            source,
        )
    })?;
    if executable.file_name() != Some(OsStr::new(GENERATOR)) {
        return Err(generation_error(
            "generation requires the packaged surgeist-layout-generate host",
        ));
    }
    run_with_host(
        request,
        GenerationHost {
            executable,
            execution: BrowserExecution::Production,
        },
    )
}

fn run_with_host(request: LayoutRequest, host: GenerationHost) -> Result<()> {
    let worker = std::thread::Builder::new()
        .name("surgeist-layout-generation".to_owned())
        .spawn(move || {
            catch_unwind(AssertUnwindSafe(|| {
                tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(1)
                    .enable_all()
                    .build()
                    .map_err(|source| {
                        GeneratorError::with_source(
                            GeneratorErrorKind::Generation,
                            "build layout generation runtime",
                            source.to_string(),
                            source,
                        )
                    })?
                    .block_on(generate(&request, &host))
            }))
        })
        .map_err(|source| {
            GeneratorError::with_source(
                GeneratorErrorKind::Generation,
                "spawn layout generation worker",
                source.to_string(),
                source,
            )
        })?;
    match worker.join() {
        Ok(Ok(result)) => result,
        Ok(Err(payload)) | Err(payload) => std::panic::resume_unwind(payload),
    }
}

#[cfg(test)]
pub(super) fn run_with_test_host(
    request: LayoutRequest,
    test_host: super::measurement::TestGenerationHost,
) -> Result<()> {
    let executable = std::env::current_exe().map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::Generation,
            "resolve crate-owned generation test host",
            source.to_string(),
            source,
        )
    })?;
    let executable = std::fs::canonicalize(&executable).map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::Generation,
            "canonicalize crate-owned generation test host",
            executable.display().to_string(),
            source,
        )
    })?;
    run_with_host(
        request,
        GenerationHost {
            executable,
            execution: BrowserExecution::Test(test_host),
        },
    )
}

async fn generate(request: &LayoutRequest, host: &GenerationHost) -> Result<()> {
    let location = request.location();
    let manifest_path = location.corpus_root().join(manifest::MANIFEST_FILE);
    let manifest_bytes = manifest::read_file(&manifest_path)?;
    let manifest = manifest::parse(&manifest_bytes, &manifest_path)?;
    let browser = TrustedBrowser::validate(
        location,
        &manifest,
        request
            .browser_path()
            .expect("generation requests carry a browser path"),
    )?;
    let preflight_rooted = RootedFs::open_corpus(location)?;
    let inputs = selection::inspect(&preflight_rooted, &manifest)?;
    let historical = report::inspect_historical(&preflight_rooted, &manifest)?;
    let desired = desired_paths(&inputs, &manifest)?;
    historical.validate_union(&desired)?;
    let selection = SelectionLedger::new(&inputs, request.filter())?;
    if selection.is_filtered() {
        if selection.is_disposition_only() {
            return Ok(());
        }
        historical.require_filtered_ownership(selection.scheduled_outputs())?;
    }
    drop(preflight_rooted);

    let reservation = ArtifactReservation::new(Domain::Layout)?;
    let xml_root = location.corpus_root().join("xml");
    let stage = reservation.external_stage().join(location.corpus_root());
    let cache = manifest.browser.cache_root.join(location.owner_root());
    let html = location.corpus_root().join("html");
    let helper = location.corpus_root().join(selection::HELPER_SCRIPT);
    let base_style = location.corpus_root().join(selection::BASE_STYLE);
    let protection = NamespaceDisjointness::for_mutation(
        location,
        &[
            ("layout XML publication root", xml_root.as_path()),
            ("layout transaction stage", stage.as_path()),
        ],
        &[
            ("layout corpus manifest", manifest_path.as_path()),
            ("layout HTML input root", html.as_path()),
            ("layout helper script", helper.as_path()),
            ("layout base style", base_style.as_path()),
            ("trusted browser cache", cache.as_path()),
            ("trusted browser executable", browser.absolute_path()),
        ],
    )?;

    let lease = GenerationLease::acquire_with_revalidation(
        location,
        Domain::Layout,
        GENERATOR,
        selection.scope(),
        COMMAND,
        |rooted| {
            let pending = classify_pending(rooted)?;
            #[cfg(test)]
            test_before_closing_revalidation(&host.execution, location)?;
            protection.revalidate(rooted)?;
            manifest::revalidate(rooted, &manifest_bytes)?;
            inputs.revalidate(rooted, &manifest)?;
            browser.closing_revalidate()?;
            let current_historical = report::inspect_historical(rooted, &manifest)?;
            current_historical.validate_union(&desired)?;
            if current_historical != historical {
                return Err(invalid_inventory(
                    "layout historical authority changed before lease installation",
                ));
            }
            if selection.is_filtered() {
                current_historical.require_filtered_ownership(selection.scheduled_outputs())?;
            }
            if let Some(pending) = pending {
                #[cfg(test)]
                test_before_pending_profile_cleanup(&host.execution, rooted)?;
                pending.execute(rooted)?;
            }
            Ok(())
        },
    )?;

    browser.closing_revalidate()?;
    let normalized_version =
        run_version_supervisor(location, &lease, &browser, &manifest, host).await?;
    if normalized_version != manifest.browser.version_output {
        return Err(GeneratorError::new(
            GeneratorErrorKind::SourceVerification,
            "verify trusted browser version",
            format!(
                "expected {:?}, received {:?}",
                manifest.browser.version_output, normalized_version
            ),
        ));
    }
    browser.closing_revalidate()?;

    let scheduled = selection
        .fixtures(&inputs)
        .into_iter()
        .filter(|fixture| fixture.schedules_browser())
        .collect::<Vec<_>>();
    let measurements = measurement::measure(
        measurement::MeasurementContext {
            location,
            lease: &lease,
            browser: &browser,
            manifest: &manifest,
            current_executable: &host.executable,
            helper: inputs.helper(),
            base_style: inputs.base_style(),
            execution: &host.execution,
        },
        &scheduled,
    )
    .await?;
    browser.closing_revalidate()?;

    let manifest_digest = Sha256Digest::from_bytes(&manifest_bytes);
    let base_style_digest = inputs.base_style_digest();
    let provenance = browser.provenance(&manifest);
    let (ledger, mut artifacts) = derive_artifacts(
        DerivationContext {
            selection: &selection,
            inputs: &inputs,
            manifest: &manifest,
            browser: &browser,
            manifest_digest: &manifest_digest,
            base_style_digest: &base_style_digest,
            browser_provenance: &provenance,
        },
        &measurements,
    )?;
    let diagnostic = ledger.has_failures();
    if selection.is_filtered() && diagnostic {
        return Err(generation_error(
            "filtered layout generation exhausted a fixture retry",
        ));
    }

    let policy = if selection.is_filtered() {
        PublicationPolicy::Filtered
    } else if diagnostic {
        PublicationPolicy::DiagnosticFull
    } else {
        PublicationPolicy::CleanFull
    };
    if !selection.is_filtered() {
        artifacts.extend(report::render_generation_reports(
            &GenerationMetadata {
                manifest: &manifest,
                browser_provenance: &provenance,
                browser_executable_sha256: browser.digest(),
                helper_sha256: inputs.helper_digest(),
                base_style_sha256: &base_style_digest,
                corpus_manifest_sha256: &manifest_digest,
                taffy_sidecar_sha256: inputs.sidecar_digest(),
            },
            ledger.clone(),
        )?);
    }

    let (classified, retained, reports) = publication_inventory(
        &historical,
        &desired,
        &selection,
        &ledger,
        &manifest,
        &artifacts,
    )?;
    let inventory = PublicationInventory::new(classified, retained, reports)?;
    let plan = ArtifactPlan::new(
        location,
        Domain::Layout,
        &lease,
        RelativePath::new("xml")?,
        policy,
        artifacts,
        inventory,
    )?
    .with_reservation(reservation)?;
    for (path, digest) in ledger.generated_artifacts() {
        let relative = strip_xml(path)?;
        if plan.artifact_digest(&relative) != Some(digest) {
            return Err(generation_error(
                "planned XML digest differs from the complete generation ledger",
            ));
        }
    }

    let revalidate = |rooted: &RootedFs| {
        protection.revalidate(rooted)?;
        manifest::revalidate(rooted, &manifest_bytes)?;
        inputs.revalidate(rooted, &manifest)?;
        browser.closing_revalidate()?;
        let current_historical = report::inspect_historical(rooted, &manifest)?;
        current_historical.validate_union(&desired)?;
        if current_historical != historical {
            return Err(invalid_inventory(
                "layout historical authority changed before publication intent",
            ));
        }
        if selection.is_filtered() {
            current_historical.require_filtered_ownership(selection.scheduled_outputs())?;
        }
        Ok(())
    };
    #[cfg(test)]
    plan.install_with_revalidation_and_inter_scan_hook(revalidate, || {})?;
    #[cfg(not(test))]
    plan.install_with_revalidation(revalidate)?;
    if diagnostic {
        Err(generation_error(
            "layout diagnostic generation published one or more failed fixtures",
        ))
    } else {
        Ok(())
    }
}

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
        let path = location.corpus_root().join(selection::HELPER_SCRIPT);
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
        .ok_or_else(|| generation_error("pending profile disappeared before test drift"))?;
    let path = format!("{}/{name}", super::profile::PROFILE_PARENT);
    let displaced = format!("{path}-displaced");
    let identity = rooted.identity_at(&path)?.ok_or_else(|| {
        generation_error("pending profile identity disappeared before test drift")
    })?;
    rooted.rename_exclusive_bound(&path, &displaced, &identity)?;
    rooted.create_dir_exclusive(&path, PRIVATE_DIRECTORY_MODE)?;
    Ok(())
}

async fn run_version_supervisor(
    location: &crate::CorpusLocation,
    lease: &GenerationLease,
    browser: &TrustedBrowser,
    manifest: &manifest::LayoutManifest,
    host: &GenerationHost,
) -> Result<String> {
    let journal = ProfileJournal::create(
        ProfileCreateContext {
            location,
            lease,
            browser,
            manifest,
        },
        ProfileAttempt::Version {
            launch_strings: vec!["version".to_owned()],
        },
    )?;
    let capsule = match (|| {
        let capsule = journal.capsule_json()?;
        journal.validates_prefix(lease.rooted())?;
        Ok::<_, GeneratorError>(capsule)
    })() {
        Ok(capsule) => capsule,
        Err(error) => {
            journal.terminalize_owned_supervisor(lease.rooted(), None)?;
            return Err(error);
        }
    };
    match &host.execution {
        BrowserExecution::Production => {
            let outcome = AssertUnwindSafe(run_version_process(&host.executable, capsule))
                .catch_unwind()
                .await;
            match outcome {
                Ok(Ok(mut execution)) => {
                    let terminal = tokio::task::block_in_place(|| {
                        journal.terminalize_owned_supervisor(
                            lease.rooted(),
                            Some(&mut execution.child),
                        )
                    });
                    let forced = matches!(&terminal, Ok(SupervisorTermination::Forced));
                    let primary = if terminal.is_ok() {
                        execution.finish(forced).await
                    } else {
                        execution.abort_output().await
                    };
                    resolve_terminalization(primary, terminal.map(|_| ()))
                }
                Ok(Err(error)) => {
                    let terminal = journal.terminalize_owned_supervisor(lease.rooted(), None);
                    resolve_terminalization::<String>(Err(error), terminal.map(|_| ()))
                }
                Err(payload) => {
                    let _ = journal.terminalize_owned_supervisor(lease.rooted(), None);
                    std::panic::resume_unwind(payload)
                }
            }
        }
        #[cfg(test)]
        BrowserExecution::Test(test_host) => {
            let mode = if test_host.plan() == super::measurement::TestBrowserPlan::BrowserFailure {
                supervisor::TestBrowserMode::Failure
            } else {
                supervisor::TestBrowserMode::Success
            };
            let outcome = AssertUnwindSafe(measurement::run_test_supervisor(
                &host.executable,
                &capsule,
                mode,
            ))
            .catch_unwind()
            .await;
            match outcome {
                Ok(Ok(mut execution)) => {
                    let terminal = journal
                        .terminalize_owned_supervisor(lease.rooted(), Some(&mut execution.child));
                    let primary = execution
                        .result
                        .map(|()| manifest.browser.version_output.clone());
                    resolve_terminalization(primary, terminal.map(|_| ()))
                }
                Ok(Err(error)) => {
                    let terminal = journal.terminalize_owned_supervisor(lease.rooted(), None);
                    resolve_terminalization::<String>(Err(error), terminal.map(|_| ()))
                }
                Err(payload) => {
                    let _ = journal.terminalize_owned_supervisor(lease.rooted(), None);
                    std::panic::resume_unwind(payload)
                }
            }
        }
    }
}

struct VersionProcessCompletion {
    stdout_task: tokio::task::JoinHandle<Result<Vec<u8>>>,
    stderr_task: tokio::task::JoinHandle<Result<Vec<u8>>>,
}

struct VersionProcessRun {
    child: tokio::process::Child,
    completion: Option<VersionProcessCompletion>,
    immediate_error: Option<GeneratorError>,
}

impl VersionProcessRun {
    fn owned_error(child: tokio::process::Child, error: GeneratorError) -> Self {
        Self {
            child,
            completion: None,
            immediate_error: Some(error),
        }
    }

    fn completed(
        child: tokio::process::Child,
        stdout_task: tokio::task::JoinHandle<Result<Vec<u8>>>,
        stderr_task: tokio::task::JoinHandle<Result<Vec<u8>>>,
    ) -> Self {
        Self {
            child,
            completion: Some(VersionProcessCompletion {
                stdout_task,
                stderr_task,
            }),
            immediate_error: None,
        }
    }

    async fn finish(mut self, forced: bool) -> Result<String> {
        if let Some(error) = self.immediate_error.take() {
            return Err(error);
        }
        let status = self
            .child
            .try_wait()
            .map_err(process_source)?
            .ok_or_else(|| {
                process_error(
                    "trusted browser version supervisor was not reaped before output validation",
                )
            })?;
        let completion = self
            .completion
            .take()
            .expect("version process completion is present");
        let stdout = completion.stdout_task.await.map_err(process_source)??;
        let stderr = completion.stderr_task.await.map_err(process_source)??;
        if forced {
            return Err(process_error(format!(
                "trusted browser version command exceeded the five-second graceful exit bound and required SIGKILL; stderr={}",
                String::from_utf8_lossy(&stderr)
            )));
        }
        if !status.success() {
            return Err(process_error(format!(
                "trusted browser version command failed: {status}; stderr={}",
                String::from_utf8_lossy(&stderr)
            )));
        }
        let stdout = std::str::from_utf8(&stdout)
            .map_err(|_| process_error("trusted browser version output is not UTF-8"))?;
        Ok(stdout.split_whitespace().collect::<Vec<_>>().join(" "))
    }

    async fn abort_output(mut self) -> Result<String> {
        if let Some(error) = self.immediate_error.take() {
            return Err(error);
        }
        let completion = self
            .completion
            .take()
            .expect("version process completion is present");
        completion.stdout_task.abort();
        completion.stderr_task.abort();
        let _ = completion.stdout_task.await;
        let _ = completion.stderr_task.await;
        Ok(String::new())
    }
}

async fn run_version_process(executable: &Path, capsule: String) -> Result<VersionProcessRun> {
    let mut command = tokio::process::Command::new(executable);
    command
        .env_clear()
        .env(supervisor::CAPSULE_ENV, capsule)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().map_err(process_source)?;
    let stdout = match child.stdout.take() {
        Some(stdout) => stdout,
        None => {
            return Ok(VersionProcessRun::owned_error(
                child,
                process_error("version supervisor stdout is unavailable"),
            ));
        }
    };
    let stderr = match child.stderr.take() {
        Some(stderr) => stderr,
        None => {
            return Ok(VersionProcessRun::owned_error(
                child,
                process_error("version supervisor stderr is unavailable"),
            ));
        }
    };
    let stdout_task = tokio::spawn(read_capped(stdout));
    let stderr_task = tokio::spawn(read_capped(stderr));
    Ok(VersionProcessRun::completed(
        child,
        stdout_task,
        stderr_task,
    ))
}

async fn read_capped(reader: impl tokio::io::AsyncRead + Unpin) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(65_537)
        .read_to_end(&mut bytes)
        .await
        .map_err(process_source)?;
    if bytes.len() > 65_536 {
        return Err(process_error(
            "trusted browser version output exceeds 64 KiB",
        ));
    }
    Ok(bytes)
}

struct DerivationContext<'a> {
    selection: &'a SelectionLedger,
    inputs: &'a CurrentInputs,
    manifest: &'a manifest::LayoutManifest,
    browser: &'a TrustedBrowser,
    manifest_digest: &'a Sha256Digest,
    base_style_digest: &'a Sha256Digest,
    browser_provenance: &'a str,
}

fn derive_artifacts(
    context: DerivationContext<'_>,
    measurements: &MeasurementResults,
) -> Result<DerivedArtifacts> {
    let mut ledger = GenerationLedger::default();
    let mut artifacts = Vec::new();
    let linked = BTreeMap::new();
    for fixture in context.selection.fixtures(context.inputs) {
        match fixture.disposition() {
            FixtureDisposition::ExpectedFail { name, reason } => {
                ledger.expected_fail(name.clone(), fixture.source().clone(), reason.clone());
            }
            FixtureDisposition::Unsupported { name, reason } => {
                ledger.unsupported(
                    name.clone(),
                    fixture.source().clone(),
                    "manifest",
                    reason.clone(),
                );
                continue;
            }
            FixtureDisposition::Quarantined { name, reason } => {
                ledger.quarantined(name.clone(), fixture.source().clone(), reason.clone());
                continue;
            }
            FixtureDisposition::Active => {}
        }
        if let Some(reason) = measurements.failure(fixture.source()) {
            ledger.failed(
                fixture_stem(fixture)?,
                fixture.source().clone(),
                reason.to_owned(),
            );
            continue;
        }
        for variant in Variant::ALL {
            match measurements.outcome(fixture.source(), variant) {
                Some(VariantOutcome::Generated(measurement)) => {
                    let output = variant.output_path(fixture.source())?;
                    let bytes = super::xml::render(
                        variant,
                        measurement,
                        &Provenance {
                            source: fixture.source(),
                            source_sha256: fixture.digest(),
                            linked_resources: &linked,
                            helper_sha256: context.inputs.helper_digest(),
                            base_style_sha256: fixture
                                .uses_base_style()
                                .then_some(context.base_style_digest),
                            browser: context.browser_provenance,
                            browser_executable_sha256: context.browser.digest(),
                            launch_profile_sha256: &context.manifest.launch_digest,
                            corpus_manifest_sha256: context.manifest_digest,
                            taffy_revision: &context.manifest.revision,
                            taffy_sidecar_sha256: context.inputs.sidecar_digest(),
                        },
                    )?;
                    let digest = Sha256Digest::from_bytes(&bytes);
                    ledger.generated(
                        variant.test_name(fixture.source())?,
                        fixture.source().clone(),
                        output.clone(),
                        digest,
                        variant.name(),
                    );
                    artifacts.push((strip_xml(&output)?, bytes));
                }
                Some(VariantOutcome::Unsupported(reason)) => ledger.unsupported(
                    variant.test_name(fixture.source())?,
                    fixture.source().clone(),
                    variant.name(),
                    reason.clone(),
                ),
                None => {
                    return Err(generation_error(format!(
                        "fixture measurement omitted {}",
                        variant.name()
                    )));
                }
            }
        }
    }
    Ok((ledger, artifacts))
}

fn publication_inventory(
    historical: &HistoricalAuthority,
    desired: &BTreeSet<RelativePath>,
    selection: &SelectionLedger,
    ledger: &GenerationLedger,
    manifest: &manifest::LayoutManifest,
    artifacts: &[(RelativePath, Vec<u8>)],
) -> Result<(Vec<RelativePath>, Vec<RelativePath>, Vec<RelativePath>)> {
    let classified_full = historical
        .classified_paths()
        .union(desired)
        .map(strip_xml)
        .collect::<Result<BTreeSet<_>>>()?;
    let report_paths = current_report_paths(manifest)?
        .into_iter()
        .map(|path| strip_xml(&path))
        .collect::<Result<Vec<_>>>()?;
    let retained = if selection.is_filtered() {
        let selected = selection
            .scheduled_outputs()
            .iter()
            .map(strip_xml)
            .collect::<Result<BTreeSet<_>>>()?;
        let mut retained = classified_full
            .iter()
            .filter(|path| !selected.contains(*path))
            .cloned()
            .collect::<BTreeSet<_>>();
        retained.extend(artifacts.iter().map(|(path, _)| path.clone()));
        retained.into_iter().collect()
    } else {
        artifacts.iter().map(|(path, _)| path.clone()).collect()
    };
    let _unsupported = ledger.unsupported_outputs()?;
    Ok((
        classified_full.into_iter().collect(),
        retained,
        report_paths,
    ))
}

fn desired_paths(
    inputs: &CurrentInputs,
    manifest: &manifest::LayoutManifest,
) -> Result<BTreeSet<RelativePath>> {
    let mut paths = inputs.all_output_paths()?;
    paths.extend(current_report_paths(manifest)?);
    Ok(paths)
}

fn current_report_paths(manifest: &manifest::LayoutManifest) -> Result<BTreeSet<RelativePath>> {
    let mut paths = BTreeSet::from([RelativePath::new("xml/generation-reports/all.json")?]);
    for scoped in &manifest.reports.scoped {
        paths.insert(RelativePath::new(format!(
            "xml/generation-reports/{}",
            scoped.file.as_str()
        ))?);
    }
    Ok(paths)
}

fn strip_xml(path: &RelativePath) -> Result<RelativePath> {
    RelativePath::new(
        path.as_str()
            .strip_prefix("xml/")
            .ok_or_else(|| invalid_inventory("layout publication path is outside xml"))?,
    )
}

fn fixture_stem(fixture: &Fixture) -> Result<String> {
    fixture
        .source()
        .as_str()
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_suffix(".html"))
        .map(str::to_owned)
        .ok_or_else(|| generation_error("fixture has no canonical .html stem"))
}

fn process_source<E>(source: E) -> GeneratorError
where
    E: std::error::Error + Send + Sync + 'static,
{
    GeneratorError::with_source(
        GeneratorErrorKind::Process,
        "run layout browser supervisor",
        source.to_string(),
        source,
    )
}

fn process_error(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::Process,
        "run layout browser supervisor",
        detail,
    )
}

fn generation_error(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::Generation,
        "generate layout corpus",
        detail,
    )
}

fn invalid_inventory(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidInventory,
        "classify layout publication inventory",
        detail,
    )
}
