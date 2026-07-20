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

use super::browser::{TrustedBrowser, chromium_config, effective_switches};
use super::manifest::LayoutManifest;
use super::profile::{
    ProfileAttempt, ProfileCreateContext, ProfileJournal, resolve_terminalization,
};
use super::selection::Fixture;
use super::xml::{MeasuredLayout, Variant};

#[derive(Clone, Debug, PartialEq)]
pub(super) enum VariantOutcome {
    Generated(MeasuredLayout),
    Unsupported(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct MeasurementResults {
    outcomes: BTreeMap<(RelativePath, Variant), VariantOutcome>,
    failures: BTreeMap<RelativePath, String>,
}

impl MeasurementResults {
    pub(super) fn outcome(
        &self,
        source: &RelativePath,
        variant: Variant,
    ) -> Option<&VariantOutcome> {
        self.outcomes.get(&(source.clone(), variant))
    }

    pub(super) fn failure(&self, source: &RelativePath) -> Option<&str> {
        self.failures.get(source).map(String::as_str)
    }
}

pub(super) struct MeasurementContext<'a> {
    pub(super) location: &'a CorpusLocation,
    pub(super) lease: &'a GenerationLease,
    pub(super) browser: &'a TrustedBrowser,
    pub(super) manifest: &'a LayoutManifest,
    pub(super) current_executable: &'a Path,
    pub(super) helper: &'a [u8],
    pub(super) base_style: &'a [u8],
    pub(super) execution: &'a BrowserExecution,
}

pub(super) enum BrowserExecution {
    Production,
    #[cfg(test)]
    Test(TestGenerationHost),
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestBrowserPlan {
    Success,
    BrowserFailure,
    RetryOnce,
    AlwaysFail,
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
}

#[cfg(test)]
impl TestGenerationHost {
    pub(super) fn new(plan: TestBrowserPlan) -> Self {
        Self {
            plan,
            attempts: Arc::new(Mutex::new(Vec::new())),
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

    fn record_attempt(&self, batch: u64, retry: u64) {
        self.attempts
            .lock()
            .expect("test generation attempt trace lock")
            .push((batch, retry));
    }
}

pub(super) async fn measure(
    context: MeasurementContext<'_>,
    fixtures: &[&Fixture],
) -> Result<MeasurementResults> {
    let mut results = MeasurementResults::default();
    for (batch_ordinal, batch) in fixtures
        .chunks(context.manifest.browser.launch.batch_size)
        .enumerate()
    {
        let mut pending = batch.to_vec();
        for retry_ordinal in 0..=1_u64 {
            if pending.is_empty() {
                break;
            }
            context.browser.closing_revalidate()?;
            let attempt = run_attempt(
                &context,
                MeasurementAttempt {
                    batch_ordinal: u64::try_from(batch_ordinal)
                        .map_err(|_| generation_error("layout batch ordinal exceeds u64"))?,
                    retry_ordinal,
                    fixtures: &pending,
                },
            )
            .await;
            let mut retry = Vec::new();
            match attempt {
                Ok(outcomes) => {
                    for fixture in pending {
                        match outcomes.get(fixture.source()) {
                            Some(Ok(variants)) => {
                                for (variant, outcome) in variants {
                                    results.outcomes.insert(
                                        (fixture.source().clone(), *variant),
                                        outcome.clone(),
                                    );
                                }
                            }
                            Some(Err(reason)) => {
                                if retry_ordinal == 0 {
                                    retry.push(fixture);
                                } else {
                                    results
                                        .failures
                                        .insert(fixture.source().clone(), reason.clone());
                                }
                            }
                            None => {
                                return Err(generation_error(
                                    "measurement attempt omitted a scheduled fixture",
                                ));
                            }
                        }
                    }
                }
                Err(error) => return Err(error),
            }
            pending = retry;
        }
    }
    Ok(results)
}

type AttemptOutcomes =
    BTreeMap<RelativePath, std::result::Result<Vec<(Variant, VariantOutcome)>, String>>;

struct MeasurementAttempt<'a> {
    batch_ordinal: u64,
    retry_ordinal: u64,
    fixtures: &'a [&'a Fixture],
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
            journal.terminalize_with_forced_group_kill(context.lease.rooted())?;
            return Err(error);
        }
    };

    let outcome = match context.execution {
        BrowserExecution::Production => {
            AssertUnwindSafe(browser_attempt(
                config.expect("production measurement prepared Chromiumoxide config"),
                context.location,
                context.manifest,
                context.helper,
                context.base_style,
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
    let terminal = journal.terminalize_with_forced_group_kill(context.lease.rooted());
    match outcome {
        Ok(result) => resolve_terminalization(result, terminal),
        Err(payload) => {
            let _ = terminal;
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
) -> Result<AttemptOutcomes> {
    host.record_attempt(attempt.batch_ordinal, attempt.retry_ordinal);
    let mode = if host.plan() == TestBrowserPlan::BrowserFailure {
        super::supervisor::TestBrowserMode::Failure
    } else {
        super::supervisor::TestBrowserMode::Success
    };
    run_test_supervisor(context.current_executable, capsule, mode).await?;
    if host.plan() == TestBrowserPlan::OwnedPanicWithCleanupFailure {
        context.lease.rooted().create_file_exclusive(
            &format!("{journal_path}/unexpected"),
            b"retained cleanup evidence",
            PRIVATE_FILE_MODE,
        )?;
    }
    match host.plan() {
        TestBrowserPlan::DependencyPanic => {
            return Err(dependency_panic(
                "crate-owned fake browser dependency",
                Box::new("synthetic dependency panic"),
            ));
        }
        TestBrowserPlan::OwnedPanic | TestBrowserPlan::OwnedPanicWithCleanupFailure => {
            std::panic::panic_any("synthetic owned generation panic");
        }
        _ => {}
    }

    let retryable_failure = host.plan() == TestBrowserPlan::AlwaysFail
        || (host.plan() == TestBrowserPlan::RetryOnce && attempt.retry_ordinal == 0);
    Ok(attempt
        .fixtures
        .iter()
        .map(|fixture| {
            let outcome = if retryable_failure {
                Err("synthetic open-load-reset-timeout failure".to_owned())
            } else {
                Ok(Variant::ALL
                    .into_iter()
                    .map(|variant| (variant, VariantOutcome::Generated(MeasuredLayout::zero())))
                    .collect())
            };
            (fixture.source().clone(), outcome)
        })
        .collect())
}

#[cfg(test)]
pub(super) async fn run_test_supervisor(
    executable: &Path,
    capsule: &str,
    mode: super::supervisor::TestBrowserMode,
) -> Result<()> {
    let mut command = super::supervisor::test_process_command(executable, capsule, mode);
    let status = tokio::task::spawn_blocking(move || command.status())
        .await
        .map_err(process_source)?
        .map_err(process_source)?;
    if status.success() {
        Ok(())
    } else {
        Err(GeneratorError::new(
            GeneratorErrorKind::Process,
            "run crate-owned fake browser supervisor",
            format!("supervisor exited unsuccessfully: {status}"),
        ))
    }
}

async fn browser_attempt(
    config: chromiumoxide::browser::BrowserConfig,
    location: &CorpusLocation,
    manifest: &LayoutManifest,
    helper: &[u8],
    base_style: &[u8],
    fixtures: &[&Fixture],
) -> Result<AttemptOutcomes> {
    let launched = AssertUnwindSafe(Browser::launch(config))
        .catch_unwind()
        .await;
    match launched {
        Ok(Ok((browser, mut handler))) => {
            let mut browser = browser;
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
            let measured =
                measure_pages(&browser, location, manifest, helper, base_style, fixtures).await;
            let close = tokio::time::timeout(
                Duration::from_secs(5),
                AssertUnwindSafe(browser.close()).catch_unwind(),
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
            close_result?;
            handler_result?;
            measured
        }
        Ok(Err(source)) => Err(process_source(source)),
        Err(payload) => Err(dependency_panic("Chromiumoxide launch", payload)),
    }
}

async fn measure_pages(
    browser: &Browser,
    location: &CorpusLocation,
    manifest: &LayoutManifest,
    helper: &[u8],
    base_style: &[u8],
    fixtures: &[&Fixture],
) -> Result<AttemptOutcomes> {
    let helper = std::str::from_utf8(helper)
        .map_err(|_| generation_error("layout helper script is not UTF-8"))?;
    let base_style = std::str::from_utf8(base_style)
        .map_err(|_| generation_error("layout base style is not UTF-8"))?;
    let mut outcomes = BTreeMap::new();
    for fixture in fixtures {
        let outcome = measure_page(browser, location, manifest, helper, base_style, fixture).await;
        outcomes.insert(
            fixture.source().clone(),
            outcome.map_err(|error| error.to_string()),
        );
    }
    Ok(outcomes)
}

async fn measure_page(
    browser: &Browser,
    location: &CorpusLocation,
    manifest: &LayoutManifest,
    helper: &str,
    base_style: &str,
    fixture: &Fixture,
) -> Result<Vec<(Variant, VariantOutcome)>> {
    let html = std::str::from_utf8(fixture.bytes())
        .map_err(|_| generation_error("layout fixture is not UTF-8"))?;
    let base_directory = fixture
        .source()
        .as_str()
        .rsplit_once('/')
        .map_or("html", |(parent, _)| parent);
    let base_url = url::Url::from_directory_path(location.corpus_root().join(base_directory))
        .map_err(|_| generation_error("cannot construct layout fixture base URL"))?;
    let style = if fixture.uses_base_style() {
        format!("<style>{base_style}</style>")
    } else {
        String::new()
    };
    let document = format!(
        "<base href=\"{}\">{style}{html}",
        escape_html_attribute(base_url.as_str())
    );
    let page =
        dependency_future("open Chromiumoxide page", browser.new_page("about:blank")).await?;
    dependency_future("set Chromiumoxide page content", page.set_content(document)).await?;
    dependency_future("evaluate layout helper", page.evaluate(helper)).await?;
    let timeout = Duration::from_millis(manifest.browser.launch.navigation_timeout_ms);
    let poll = Duration::from_millis(manifest.browser.launch.dom_poll_interval_ms);
    tokio::time::timeout(timeout, async {
        loop {
            let ready = dependency_future(
                "poll layout document readiness",
                page.evaluate(
                    "document.readyState === 'complete' || document.readyState === 'interactive'",
                ),
            )
            .await?
            .into_value::<bool>()
            .map_err(generation_source)?;
            if ready {
                break Ok::<(), GeneratorError>(());
            }
            tokio::time::sleep(poll).await;
        }
    })
    .await
    .map_err(|source| process_timeout("wait for layout document readiness", source))??;

    let mut outcomes = Vec::with_capacity(4);
    for variant in Variant::ALL {
        let expression = format!(
            "globalThis[{}] ?? null",
            json_string(variant.browser_key())?
        );
        let result =
            dependency_future("read layout measurement", page.evaluate(expression)).await?;
        let value = result
            .value()
            .cloned()
            .ok_or_else(|| generation_error("layout measurement has no protocol value"))?;
        if value.is_null() {
            outcomes.push((
                variant,
                VariantOutcome::Unsupported("browser produced no variant measurement".to_owned()),
            ));
        } else {
            let measurement = serde_json::from_value(value).map_err(generation_source)?;
            outcomes.push((variant, VariantOutcome::Generated(measurement)));
        }
    }
    dependency_future("close Chromiumoxide page", page.close()).await?;
    Ok(outcomes)
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

fn generation_source<E>(source: E) -> GeneratorError
where
    E: std::error::Error + Send + Sync + 'static,
{
    GeneratorError::with_source(
        GeneratorErrorKind::Generation,
        "convert layout measurement",
        source.to_string(),
        source,
    )
}

fn generation_error(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::Generation,
        "measure layout fixture",
        detail,
    )
}

fn escape_html_attribute(value: &str) -> String {
    value.replace('&', "&amp;").replace('"', "&quot;")
}

fn json_string(value: &str) -> Result<String> {
    serde_json::to_string(value).map_err(generation_source)
}

#[cfg(test)]
mod tests {
    use super::dependency_panic;
    use crate::GeneratorErrorKind;

    #[test]
    fn layout_generate_dependency_panic_maps_to_process() {
        let error = dependency_panic("test", Box::new("boom"));
        assert_eq!(error.kind(), GeneratorErrorKind::Process);
        assert!(error.to_string().contains("boom"));
    }
}
