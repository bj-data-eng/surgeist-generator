use std::collections::BTreeMap;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::time::Duration;

#[cfg(test)]
use std::sync::{Arc, Mutex};

use chromiumoxide::browser::Browser;
use futures::{FutureExt, StreamExt};

use crate::core::GenerationLease;
#[cfg(test)]
use crate::core::PRIVATE_FILE_MODE;
use crate::{CorpusLocation, GeneratorError, GeneratorErrorKind, RelativePath, Result};

use super::browser_runtime::{TrustedBrowser, chromium_config, effective_switches};
use super::engine::PreparedInput;
use super::model::EngineManifest;
use super::profile::{
    OwnedSupervisorChild, ProfileAttempt, ProfileCreateContext, ProfileJournal,
    SupervisorTermination, resolve_terminalization,
};
use serde_json::Value;

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct MeasurementResults {
    pub(super) outcomes: BTreeMap<RelativePath, Value>,
    pub(super) failures: BTreeMap<RelativePath, String>,
}

pub(super) struct MeasurementContext<'a> {
    pub(super) location: &'a CorpusLocation,
    pub(super) lease: &'a GenerationLease,
    pub(super) browser: &'a TrustedBrowser,
    pub(super) manifest: &'a EngineManifest,
    pub(super) current_executable: &'a Path,
    pub(super) execution: &'a BrowserExecution,
}

pub(super) enum BrowserExecution {
    Production,
    #[cfg(test)]
    Test(TestGenerationHost),
}

struct RetainedBrowser(Option<Browser>);

impl RetainedBrowser {
    fn new(browser: Browser) -> Self {
        Self(Some(browser))
    }

    fn browser(&self) -> &Browser {
        self.0.as_ref().expect("retained browser is present")
    }

    fn browser_mut(&mut self) -> &mut Browser {
        self.0.as_mut().expect("retained browser is present")
    }

    fn supervisor_child(&mut self) -> Option<&mut dyn OwnedSupervisorChild> {
        self.browser_mut()
            .get_mut_child()
            .map(|child| child.as_mut_inner() as &mut dyn OwnedSupervisorChild)
    }

    fn release_after_terminalization(mut self) {
        drop(self.0.take());
    }

    fn release_if_exited(mut self) {
        let exited = self
            .browser_mut()
            .get_mut_child()
            .is_none_or(|child| child.try_wait().is_ok_and(|status| status.is_some()));
        if exited {
            drop(self.0.take());
        }
    }
}

impl Drop for RetainedBrowser {
    fn drop(&mut self) {
        if let Some(browser) = self.0.take() {
            // Chromiumoxide enables kill-on-drop for its child. Until normal
            // terminalization proves that child still owns the recorded group,
            // detaching is the only signal-free failure and unwind behavior.
            std::mem::forget(browser);
        }
    }
}

enum AttemptSupervisor {
    Production(Box<RetainedBrowser>),
    #[cfg(test)]
    Test(std::process::Child),
}

impl AttemptSupervisor {
    fn child(&mut self) -> Option<&mut dyn OwnedSupervisorChild> {
        match self {
            Self::Production(browser) => browser.supervisor_child(),
            #[cfg(test)]
            Self::Test(child) => Some(child),
        }
    }

    fn finish_terminalization(self, succeeded: bool) {
        match self {
            Self::Production(browser) if succeeded => (*browser).release_after_terminalization(),
            Self::Production(browser) => (*browser).release_if_exited(),
            #[cfg(test)]
            Self::Test(child) => drop(child),
        }
    }
}

struct AttemptExecution {
    completion: AttemptCompletion,
    supervisor: Option<AttemptSupervisor>,
}

enum AttemptCompletion {
    Result(Result<AttemptOutcomes>),
    Panic(Box<dyn std::any::Any + Send>),
}

impl AttemptExecution {
    fn without_supervisor(result: Result<AttemptOutcomes>) -> Self {
        Self {
            completion: AttemptCompletion::Result(result),
            supervisor: None,
        }
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestBrowserPlan {
    Success,
    BrowserFailure,
    RetryOnce,
    AlwaysFail,
    HelperFailure,
    MeasurementFailure,
    CloseFailure,
    DependencyPanic,
    OwnedPanic,
    OwnedPanicWithCleanupFailure,
    ClosingRevalidationFailure,
    ProfileIdentityDrift,
}

#[cfg(test)]
#[derive(Clone)]
pub(super) struct TestGenerationHost {
    plan: TestBrowserPlan,
    attempts: Arc<Mutex<Vec<(u64, u64)>>>,
    fixture_attempts: Arc<Mutex<Vec<Vec<RelativePath>>>>,
}

#[cfg(test)]
impl TestGenerationHost {
    pub(super) fn new(plan: TestBrowserPlan) -> Self {
        Self {
            plan,
            attempts: Arc::new(Mutex::new(Vec::new())),
            fixture_attempts: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub(super) const fn plan(&self) -> TestBrowserPlan {
        self.plan
    }

    pub(super) fn attempts(&self) -> Vec<(u64, u64)> {
        self.attempts
            .lock()
            .expect("test generation attempt trace lock")
            .clone()
    }

    pub(super) fn fixture_attempts(&self) -> Vec<Vec<RelativePath>> {
        self.fixture_attempts
            .lock()
            .expect("fixture attempt trace")
            .clone()
    }

    fn record_attempt(&self, batch: u64, retry: u64, fixtures: &[&PreparedInput]) {
        self.fixture_attempts
            .lock()
            .expect("fixture attempt trace")
            .push(
                fixtures
                    .iter()
                    .map(|fixture| fixture.source().clone())
                    .collect(),
            );
        self.attempts
            .lock()
            .expect("test generation attempt trace lock")
            .push((batch, retry));
    }
}

pub(super) async fn measure(
    context: MeasurementContext<'_>,
    fixtures: &[&PreparedInput],
) -> Result<MeasurementResults> {
    let mut results = MeasurementResults::default();
    for (batch_ordinal, batch) in fixtures
        .chunks(context.manifest.browser.launch.batch_size)
        .enumerate()
    {
        let batch_ordinal = u64::try_from(batch_ordinal)
            .map_err(|_| generation_error("browser batch ordinal exceeds u64"))?;
        context.browser.closing_revalidate()?;
        let mut outcomes = run_attempt(
            &context,
            MeasurementAttempt {
                batch_ordinal,
                retry_ordinal: 0,
                fixtures: batch,
            },
        )
        .await?;
        for fixture in batch {
            let outcome = outcomes.remove(fixture.source()).ok_or_else(|| {
                generation_error("measurement attempt omitted a scheduled fixture")
            })?;
            let outcome = match outcome {
                Err(failure @ PageFailure::NavigationTimeout(_)) => {
                    // Every failed fixture receives a separately owned browser/profile.
                    // Terminal page failures never enter this navigation-only retry path.
                    context.browser.closing_revalidate()?;
                    let mut retried = run_attempt(
                        &context,
                        MeasurementAttempt {
                            batch_ordinal,
                            retry_ordinal: 1,
                            fixtures: std::slice::from_ref(fixture),
                        },
                    )
                    .await?;
                    retried
                        .remove(fixture.source())
                        .ok_or_else(|| generation_error("retry omitted its scheduled fixture"))?
                        .map_err(|retry| format!("{failure}; retry failed: {retry}"))
                }
                result => result.map_err(|error| error.to_string()),
            };
            match outcome {
                Ok(value) => {
                    results.outcomes.insert(fixture.source().clone(), value);
                }
                Err(reason) => {
                    results.failures.insert(fixture.source().clone(), reason);
                }
            }
        }
    }
    Ok(results)
}

/// Retry eligibility belongs to the page operation that observed the failure.
/// Only document navigation/readiness timeouts permit another browser attempt.
#[derive(Debug)]
enum PageFailure {
    NavigationTimeout(GeneratorError),
    Terminal(GeneratorError),
}

impl std::fmt::Display for PageFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NavigationTimeout(error) | Self::Terminal(error) => error.fmt(formatter),
        }
    }
}

impl From<GeneratorError> for PageFailure {
    fn from(error: GeneratorError) -> Self {
        Self::Terminal(error)
    }
}

type PageResult<T> = std::result::Result<T, PageFailure>;
type AttemptOutcomes = BTreeMap<RelativePath, PageResult<Value>>;

struct MeasurementAttempt<'a> {
    batch_ordinal: u64,
    retry_ordinal: u64,
    fixtures: &'a [&'a PreparedInput],
}

async fn run_attempt(
    context: &MeasurementContext<'_>,
    attempt: MeasurementAttempt<'_>,
) -> Result<AttemptOutcomes> {
    let launch_strings = effective_switches(context.manifest, Path::new("profile"))?
        .into_iter()
        .map(|(key, value)| value.map_or(key.clone(), |value| format!("{key}={value}")))
        .collect();
    let journal = ProfileJournal::create(
        ProfileCreateContext {
            location: context.location,
            lease: context.lease,
            browser: context.browser,
            manifest: context.manifest,
        },
        ProfileAttempt::Measurement {
            batch_ordinal: attempt.batch_ordinal,
            retry_ordinal: attempt.retry_ordinal,
            launch_strings,
        },
    )?;
    let prepared = (|| {
        let capsule = journal.capsule_json()?;
        journal.validates_prefix(context.lease.rooted())?;
        let config = match context.execution {
            BrowserExecution::Production => Some(chromium_config(
                context.current_executable,
                journal.profile_path(),
                context.manifest,
                &capsule,
            )?),
            #[cfg(test)]
            BrowserExecution::Test(_) => None,
        };
        Ok::<_, GeneratorError>((capsule, config))
    })();
    let (_capsule, config) = match prepared {
        Ok(prepared) => prepared,
        Err(error) => {
            journal.terminalize_owned_supervisor(context.lease.rooted(), None)?;
            return Err(error);
        }
    };

    let outcome = match context.execution {
        BrowserExecution::Production => {
            AssertUnwindSafe(browser_attempt(
                config.expect("production measurement prepared Chromiumoxide config"),
                context.manifest,
                attempt.fixtures,
            ))
            .catch_unwind()
            .await
        }
        #[cfg(test)]
        BrowserExecution::Test(host) => {
            AssertUnwindSafe(test_browser_attempt(
                context,
                &_capsule,
                host,
                &attempt,
                journal.journal_path(),
            ))
            .catch_unwind()
            .await
        }
    };
    match outcome {
        Ok(mut execution) => {
            let terminal = journal.terminalize_owned_supervisor(
                context.lease.rooted(),
                execution
                    .supervisor
                    .as_mut()
                    .and_then(AttemptSupervisor::child),
            );
            let terminal_succeeded = terminal.is_ok();
            let termination = terminal.as_ref().copied().ok();
            let terminal = terminal.map(|_| ());
            let AttemptExecution {
                completion,
                supervisor,
            } = execution;
            if let Some(supervisor) = supervisor {
                supervisor.finish_terminalization(terminal_succeeded);
            }
            match completion {
                AttemptCompletion::Result(result) => {
                    let result = match termination {
                        Some(termination) => apply_supervisor_termination(result, termination),
                        None => result,
                    };
                    resolve_terminalization(result, terminal)
                }
                AttemptCompletion::Panic(payload) => {
                    let _ = terminal;
                    std::panic::resume_unwind(payload)
                }
            }
        }
        Err(payload) => {
            let _ = journal.terminalize_owned_supervisor(context.lease.rooted(), None);
            std::panic::resume_unwind(payload)
        }
    }
}

#[cfg(test)]
async fn test_browser_attempt(
    context: &MeasurementContext<'_>,
    capsule: &str,
    host: &TestGenerationHost,
    attempt: &MeasurementAttempt<'_>,
    journal_path: &str,
) -> AttemptExecution {
    host.record_attempt(
        attempt.batch_ordinal,
        attempt.retry_ordinal,
        attempt.fixtures,
    );
    let mode = if host.plan() == TestBrowserPlan::BrowserFailure {
        super::supervisor::TestBrowserMode::Failure
    } else {
        super::supervisor::TestBrowserMode::Success
    };
    let supervisor = match run_test_supervisor(context.current_executable, capsule, mode).await {
        Ok(supervisor) => supervisor,
        Err(error) => return AttemptExecution::without_supervisor(Err(error)),
    };
    let TestSupervisorRun { result, child } = supervisor;
    if let Err(error) = result {
        return AttemptExecution {
            completion: AttemptCompletion::Result(Err(error)),
            supervisor: Some(AttemptSupervisor::Test(child)),
        };
    }
    if host.plan() == TestBrowserPlan::OwnedPanicWithCleanupFailure
        && let Err(error) = context.lease.rooted().create_file_exclusive(
            &format!("{journal_path}/unexpected"),
            b"retained cleanup evidence",
            PRIVATE_FILE_MODE,
        )
    {
        return AttemptExecution {
            completion: AttemptCompletion::Result(Err(error)),
            supervisor: Some(AttemptSupervisor::Test(child)),
        };
    }
    match host.plan() {
        TestBrowserPlan::DependencyPanic => {
            return AttemptExecution {
                completion: AttemptCompletion::Result(Err(dependency_panic(
                    "crate-owned fake browser dependency",
                    Box::new("synthetic dependency panic"),
                ))),
                supervisor: Some(AttemptSupervisor::Test(child)),
            };
        }
        TestBrowserPlan::OwnedPanic | TestBrowserPlan::OwnedPanicWithCleanupFailure => {
            return AttemptExecution {
                completion: AttemptCompletion::Panic(Box::new("synthetic owned generation panic")),
                supervisor: Some(AttemptSupervisor::Test(child)),
            };
        }
        _ => {}
    }

    let retryable_failure = host.plan() == TestBrowserPlan::AlwaysFail
        || (host.plan() == TestBrowserPlan::RetryOnce && attempt.retry_ordinal == 0);
    let result = Ok(attempt
        .fixtures
        .iter()
        .map(|fixture| {
            let outcome = if retryable_failure {
                Err(PageFailure::NavigationTimeout(generation_error(
                    "synthetic open-load-reset-timeout failure",
                )))
            } else if matches!(
                host.plan(),
                TestBrowserPlan::HelperFailure
                    | TestBrowserPlan::MeasurementFailure
                    | TestBrowserPlan::CloseFailure
            ) {
                Err(PageFailure::Terminal(generation_error(format!(
                    "synthetic terminal {:?} failure",
                    host.plan()
                ))))
            } else {
                Ok(serde_json::json!({"synthetic": true}))
            };
            (fixture.source().clone(), outcome)
        })
        .collect());
    AttemptExecution {
        completion: AttemptCompletion::Result(result),
        supervisor: Some(AttemptSupervisor::Test(child)),
    }
}

#[cfg(test)]
pub(super) struct TestSupervisorRun {
    pub(super) result: Result<()>,
    pub(super) child: std::process::Child,
}

#[cfg(test)]
pub(super) async fn run_test_supervisor(
    executable: &Path,
    capsule: &str,
    mode: super::supervisor::TestBrowserMode,
) -> Result<TestSupervisorRun> {
    let mut command = super::supervisor::test_process_command(executable, capsule, mode);
    let child = command.spawn().map_err(process_source)?;
    let (child, status) = tokio::task::spawn_blocking(move || {
        let mut child = child;
        let status = child.wait();
        (child, status)
    })
    .await
    .map_err(process_source)?;
    let result = status.map_err(process_source).and_then(|status| {
        if status.success() {
            Ok(())
        } else {
            Err(GeneratorError::new(
                GeneratorErrorKind::Process,
                "run crate-owned fake browser supervisor",
                format!("supervisor exited unsuccessfully: {status}"),
            ))
        }
    });
    Ok(TestSupervisorRun { result, child })
}

async fn browser_attempt(
    config: chromiumoxide::browser::BrowserConfig,
    manifest: &EngineManifest,
    fixtures: &[&PreparedInput],
) -> AttemptExecution {
    let launched = AssertUnwindSafe(Browser::launch(config))
        .catch_unwind()
        .await;
    match launched {
        Ok(Ok((browser, mut handler))) => {
            let mut browser = RetainedBrowser::new(browser);
            let handler_task = tokio::spawn(async move {
                let handled = AssertUnwindSafe(async move {
                    while let Some(event) = handler.next().await {
                        event.map_err(process_source)?;
                    }
                    Ok::<(), GeneratorError>(())
                })
                .catch_unwind()
                .await;
                match handled {
                    Ok(result) => result,
                    Err(payload) => Err(dependency_panic("Chromiumoxide handler", payload)),
                }
            });
            let measured = AssertUnwindSafe(measure_pages(browser.browser(), manifest, fixtures))
                .catch_unwind()
                .await;
            let close = tokio::time::timeout(
                Duration::from_secs(5),
                AssertUnwindSafe(browser.browser_mut().close()).catch_unwind(),
            )
            .await;
            let close_result = match close {
                Ok(Ok(Ok(_))) => Ok(()),
                Ok(Ok(Err(source))) => Err(process_source(source)),
                Ok(Err(payload)) => Err(dependency_panic("Chromiumoxide close", payload)),
                Err(source) => Err(process_timeout("close Chromiumoxide browser", source)),
            };
            handler_task.abort();
            let handler_result =
                match tokio::time::timeout(Duration::from_secs(5), handler_task).await {
                    Ok(Ok(result)) => result,
                    Ok(Err(source)) if source.is_cancelled() => Ok(()),
                    Ok(Err(source)) => Err(process_source(source)),
                    Err(source) => Err(process_timeout("join Chromiumoxide handler", source)),
                };
            let completion = match measured {
                Ok(measured) => {
                    AttemptCompletion::Result(close_result.and(handler_result).and(measured))
                }
                Err(payload) => AttemptCompletion::Panic(payload),
            };
            AttemptExecution {
                completion,
                supervisor: Some(AttemptSupervisor::Production(Box::new(browser))),
            }
        }
        Ok(Err(source)) => AttemptExecution::without_supervisor(Err(process_source(source))),
        Err(payload) => AttemptExecution::without_supervisor(Err(dependency_panic(
            "Chromiumoxide launch",
            payload,
        ))),
    }
}

async fn measure_pages(
    browser: &Browser,
    manifest: &EngineManifest,
    fixtures: &[&PreparedInput],
) -> Result<AttemptOutcomes> {
    let mut outcomes = BTreeMap::new();
    for fixture in fixtures {
        let outcome = measure_page(browser, manifest, fixture).await;
        outcomes.insert(fixture.source().clone(), outcome);
    }
    Ok(outcomes)
}

async fn measure_page(
    browser: &Browser,
    manifest: &EngineManifest,
    fixture: &PreparedInput,
) -> PageResult<Value> {
    let job = fixture.job();
    let page = navigation_future("open browser page", browser.new_page("about:blank")).await?;
    // The adapter owns document construction. document.write preserves the existing
    // HTML loading semantics, including authored doctype and script execution.
    let document = serde_json::to_string(&job.document).map_err(generation_source)?;
    let write = format!(
        "(() => {{ document.open(); document.write({document}); document.close(); return true; }})()"
    );
    let result = async {
        navigation_future("load browser fixture", page.evaluate_expression(write)).await?;
        let timeout = Duration::from_millis(manifest.browser.launch.navigation_timeout_ms);
        let poll = Duration::from_millis(manifest.browser.launch.dom_poll_interval_ms);
        let mut last_readiness_error = None;
        tokio::time::timeout(timeout, async {
            loop {
                match dependency_future(
                    "poll fixture readiness",
                    page.evaluate_expression(job.readiness_expression.clone()),
                )
                .await
                {
                    Ok(value) => {
                        let ready = value.into_value::<bool>().map_err(generation_source)?;
                        if ready {
                            break Ok::<(), GeneratorError>(());
                        }
                    }
                    Err(error) => last_readiness_error = Some(error.to_string()),
                }
                tokio::time::sleep(poll).await;
            }
        })
        .await
        .map_err(|source| {
            PageFailure::NavigationTimeout(GeneratorError::with_source(
                GeneratorErrorKind::Process,
                "wait for fixture readiness",
                last_readiness_error.map_or_else(
                    || "timed out waiting for fixture readiness".to_owned(),
                    |error| format!("timed out waiting for fixture readiness; last error: {error}"),
                ),
                source,
            ))
        })??;
        if !job.setup_expression.trim().is_empty() {
            dependency_future(
                "initialize fixture protocol",
                page.evaluate_expression(job.setup_expression.clone()),
            )
            .await?;
        }
        let value = dependency_future(
            "measure browser fixture",
            page.evaluate_expression(job.measurement_expression.clone()),
        )
        .await?;
        value.value().cloned().ok_or_else(|| {
            PageFailure::Terminal(generation_error(
                "browser measurement omitted its JSON value",
            ))
        })
    }
    .await;
    let closed = dependency_future("close browser page", page.close()).await;
    finish_page(result, closed)
}

fn finish_page<T>(result: PageResult<T>, closed: Result<()>) -> PageResult<T> {
    match (result, closed) {
        (result, Ok(())) => result,
        (Ok(_), Err(error)) => Err(PageFailure::Terminal(error)),
        (Err(primary), Err(error)) => Err(PageFailure::Terminal(GeneratorError::with_source(
            GeneratorErrorKind::Process,
            "close browser page",
            format!("{primary}; page cleanup failed: {error}"),
            error,
        ))),
    }
}

async fn navigation_future<T>(
    operation: &'static str,
    future: impl std::future::Future<Output = chromiumoxide::error::Result<T>>,
) -> PageResult<T> {
    match AssertUnwindSafe(future).catch_unwind().await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(source)) => {
            let timed_out = matches!(source, chromiumoxide::error::CdpError::Timeout);
            let error = GeneratorError::with_source(
                GeneratorErrorKind::Process,
                operation,
                source.to_string(),
                source,
            );
            Err(if timed_out {
                PageFailure::NavigationTimeout(error)
            } else {
                PageFailure::Terminal(error)
            })
        }
        Err(payload) => Err(PageFailure::Terminal(dependency_panic(operation, payload))),
    }
}

async fn dependency_future<T, E>(
    operation: &'static str,
    future: impl std::future::Future<Output = std::result::Result<T, E>>,
) -> Result<T>
where
    E: std::error::Error + Send + Sync + 'static,
{
    match AssertUnwindSafe(future).catch_unwind().await {
        Ok(Ok(value)) => Ok(value),
        Ok(Err(source)) => Err(GeneratorError::with_source(
            GeneratorErrorKind::Process,
            operation,
            source.to_string(),
            source,
        )),
        Err(payload) => Err(dependency_panic(operation, payload)),
    }
}

pub(super) fn dependency_panic(
    operation: &str,
    payload: Box<dyn std::any::Any + Send>,
) -> GeneratorError {
    let detail = payload
        .downcast_ref::<&str>()
        .map(|value| (*value).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "non-string dependency panic".to_owned());
    GeneratorError::new(GeneratorErrorKind::Process, operation, detail)
}

fn process_source<E>(source: E) -> GeneratorError
where
    E: std::error::Error + Send + Sync + 'static,
{
    GeneratorError::with_source(
        GeneratorErrorKind::Process,
        "drive Chromiumoxide browser",
        source.to_string(),
        source,
    )
}

fn process_timeout(operation: &str, source: tokio::time::error::Elapsed) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::Process,
        operation,
        "trusted browser operation timed out",
        source,
    )
}

fn supervisor_timeout() -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::Process,
        "wait for browser supervisor",
        "supervisor exceeded the five-second graceful exit bound and required SIGKILL",
    )
}

fn apply_supervisor_termination<T>(
    primary: Result<T>,
    termination: SupervisorTermination,
) -> Result<T> {
    match (primary, termination) {
        (Ok(_), SupervisorTermination::Forced) => Err(supervisor_timeout()),
        (primary, _) => primary,
    }
}

fn generation_source<E>(source: E) -> GeneratorError
where
    E: std::error::Error + Send + Sync + 'static,
{
    GeneratorError::with_source(
        GeneratorErrorKind::Generation,
        "convert browser measurement",
        source.to_string(),
        source,
    )
}

fn generation_error(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::Generation,
        "measure browser fixture",
        detail,
    )
}

#[cfg(test)]
mod retry_classification_tests {
    use super::*;
    use chromiumoxide::error::CdpError;

    #[tokio::test]
    async fn navigation_retries_only_the_typed_timeout() {
        for operation in [
            "open fixture document",
            "load browser fixture",
            "reset fixture document",
        ] {
            let timeout = navigation_future(operation, async { Err::<(), _>(CdpError::Timeout) })
                .await
                .unwrap_err();
            assert!(matches!(timeout, PageFailure::NavigationTimeout(_)));
            let text = navigation_future(operation, async {
                Err::<(), _>(CdpError::msg("Request timed out."))
            })
            .await
            .unwrap_err();
            assert!(
                matches!(text, PageFailure::Terminal(_)),
                "diagnostic text cannot grant retry eligibility"
            );
        }
    }

    #[tokio::test]
    async fn helper_measurement_and_close_timeouts_remain_terminal() {
        for operation in [
            "initialize fixture protocol",
            "measure browser fixture",
            "close browser page",
        ] {
            let failure = dependency_future(operation, async { Err::<(), _>(CdpError::Timeout) })
                .await
                .unwrap_err();
            assert!(matches!(
                PageFailure::from(failure),
                PageFailure::Terminal(_)
            ));
        }
    }

    #[test]
    fn page_cleanup_failure_prevents_retry_without_hiding_the_navigation_failure() {
        let result = finish_page::<()>(
            Err(PageFailure::NavigationTimeout(generation_error(
                "navigation timed out",
            ))),
            Err(generation_error("close failed")),
        )
        .unwrap_err();
        assert!(matches!(result, PageFailure::Terminal(_)));
        let diagnostic = result.to_string();
        assert!(diagnostic.contains("navigation timed out"));
        assert!(diagnostic.contains("close failed"));
    }
}
