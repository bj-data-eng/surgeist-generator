use super::browser_runtime::TrustedBrowser;
use super::engine::GenerationHost;
#[cfg(test)]
use super::measurement;
use super::measurement::BrowserExecution;
use super::model::EngineManifest;
use super::profile::{
    ProfileAttempt, ProfileCreateContext, ProfileJournal, SupervisorTermination,
    resolve_terminalization,
};
use super::supervisor;
use crate::core::GenerationLease;
use crate::{GeneratorError, GeneratorErrorKind, Result};
use futures::FutureExt;
use std::panic::AssertUnwindSafe;
use std::path::Path;
use std::process::Stdio;
use tokio::io::AsyncReadExt;

pub(super) async fn run_version_supervisor(
    location: &crate::CorpusLocation,
    lease: &GenerationLease,
    browser: &TrustedBrowser,
    manifest: &EngineManifest,
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
fn process_source<E>(source: E) -> GeneratorError
where
    E: std::error::Error + Send + Sync + 'static,
{
    GeneratorError::with_source(
        GeneratorErrorKind::Process,
        "run browser supervisor",
        source.to_string(),
        source,
    )
}

fn process_error(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::Process,
        "run browser supervisor",
        detail,
    )
}
