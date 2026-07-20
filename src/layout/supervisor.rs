use std::ffi::{OsStr, OsString};
use std::fs::TryLockError;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::process::{Command, Stdio};

use crate::core::{PRIVATE_FILE_MODE, RootedFs, authenticate_layout_supervisor_owner};
use crate::{CorpusLocation, GeneratorError, GeneratorErrorKind, Result};

use super::browser::{TrustedBrowser, fixed_environment, validate_received_switches};
use super::manifest;
use super::profile::{
    LaunchCapsule, ProfilePurpose, lock_transition, publish_running, validate_capsule_records,
};

pub(super) const CAPSULE_ENV: &str = "SURGEIST_LAYOUT_LAUNCH_CAPSULE";
#[cfg(test)]
const TEST_MODE_ENV: &str = "SURGEIST_LAYOUT_TEST_SUPERVISOR_MODE";
const MUTEX: &str = ".surgeist-generator/leases/layout/mutex.lock";

#[derive(Clone, Copy)]
enum SupervisorMode {
    Production,
    #[cfg(test)]
    Test(TestBrowserMode),
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TestBrowserMode {
    Success,
    DelayedSupervisorExit,
    Failure,
    Hang,
    HoldTransition,
}

#[cfg(test)]
impl TestBrowserMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::DelayedSupervisorExit => "delayed-supervisor-exit",
            Self::Failure => "failure",
            Self::Hang => "hang",
            Self::HoldTransition => "hold-transition",
        }
    }

    fn parse(value: &str) -> Result<Self> {
        match value {
            "success" => Ok(Self::Success),
            "delayed-supervisor-exit" => Ok(Self::DelayedSupervisorExit),
            "failure" => Ok(Self::Failure),
            "hang" => Ok(Self::Hang),
            "hold-transition" => Ok(Self::HoldTransition),
            _ => Err(cli_error("unknown crate-owned test supervisor mode")),
        }
    }

    const fn browser_test(self) -> Option<&'static str> {
        match self {
            Self::Success => Some("layout::profile_tests::layout_fake_browser_success_process"),
            Self::DelayedSupervisorExit => None,
            Self::Failure => Some("layout::profile_tests::layout_fake_browser_failure_process"),
            Self::Hang => Some("layout::profile_tests::layout_fake_browser_hang_process"),
            Self::HoldTransition => None,
        }
    }
}

pub(super) fn run_from_env_if_present() -> Option<Result<()>> {
    let value = std::env::var_os(CAPSULE_ENV)?;
    Some(
        value
            .into_string()
            .map_err(|_| cli_error("private launch capsule must be UTF-8"))
            .and_then(|value| LaunchCapsule::parse_canonical(&value))
            .and_then(|capsule| run(capsule, SupervisorMode::Production)),
    )
}

fn run(capsule: LaunchCapsule, mode: SupervisorMode) -> Result<()> {
    let owner = capsule.owner_root()?;
    let corpus = capsule.corpus_root()?;
    let location = CorpusLocation::new(&owner, &corpus)?;
    if location.owner_root() != owner || location.corpus_root() != corpus {
        return Err(cli_error("capsule roots are not canonical"));
    }
    let actual_parent = rustix::process::getppid()
        .map(|pid| pid.as_raw_nonzero().get() as u32)
        .ok_or_else(|| cli_error("supervisor has no live parent"))?;
    if actual_parent != capsule.parent_pid {
        return Err(cli_error("capsule parent PID differs from the live parent"));
    }

    let rooted = RootedFs::open_corpus(&location)?;
    let mutex = rooted.open_file_handle(MUTEX, PRIVATE_FILE_MODE, false)?;
    match mutex.try_lock() {
        Ok(()) => {
            let _ = mutex.unlock();
            return Err(cli_error("layout mutex is not held by the capsule parent"));
        }
        Err(TryLockError::WouldBlock) => {}
        Err(TryLockError::Error(source)) => {
            return Err(GeneratorError::with_source(
                GeneratorErrorKind::Cli,
                "authenticate layout supervisor capsule",
                "cannot prove the layout mutex is held",
                source,
            ));
        }
    }
    authenticate_layout_supervisor_owner(&rooted, &location, actual_parent).map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::Cli,
            "authenticate layout supervisor capsule",
            source.to_string(),
            source,
        )
    })?;
    let (intent, _profile) = validate_capsule_records(&rooted, &capsule)?;
    if intent.parent_pid != actual_parent
        || intent.profile_token != capsule.profile_token
        || intent.browser_path != capsule.browser_path
        || intent.purpose != capsule.purpose
    {
        return Err(cli_error("capsule and authenticated intent differ"));
    }

    let manifest_path = location.corpus_root().join(manifest::MANIFEST_FILE);
    let manifest_bytes = manifest::read_file(&manifest_path)?;
    let manifest = manifest::parse(&manifest_bytes, &manifest_path)?;
    manifest::revalidate(&rooted, &manifest_bytes)?;
    let browser = TrustedBrowser::validate(&location, &manifest, &capsule.browser_path)?;
    if browser.identity() != &intent.browser_identity
        || browser.digest() != &intent.browser_sha256
        || manifest.launch_digest != intent.launch_profile_sha256
    {
        return Err(cli_error(
            "capsule browser identity, digest, or launch profile changed",
        ));
    }

    let journal = capsule.journal_path.as_str();
    let profile_path = location.corpus_root().join(journal).join("profile");
    let transition = lock_transition(&rooted, journal, false)?;
    rustix::process::setsid().map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::Process,
            "create layout browser process group",
            "internal supervisor could not become a session leader",
            source,
        )
    })?;
    let supervisor_pid = std::process::id();
    publish_running(
        &rooted,
        journal,
        &capsule.profile_token,
        capsule.parent_pid,
        supervisor_pid,
    )?;

    #[cfg(test)]
    if matches!(mode, SupervisorMode::Test(TestBrowserMode::HoldTransition)) {
        std::thread::sleep(std::time::Duration::from_secs(30));
        return Err(process_error(
            "crate-owned transition-race supervisor was not interrupted",
        ));
    }

    #[cfg(test)]
    if matches!(
        mode,
        SupervisorMode::Test(TestBrowserMode::DelayedSupervisorExit)
    ) {
        transition.unlock().map_err(|source| {
            GeneratorError::with_source(
                GeneratorErrorKind::ArtifactTransaction,
                "release layout profile transition lock",
                journal.to_owned(),
                source,
            )
        })?;
        drop(transition);
        std::thread::sleep(std::time::Duration::from_millis(250));
        return Ok(());
    }

    let mut command = Command::new(browser.absolute_path());
    command
        .env_clear()
        .envs(fixed_environment(&profile_path))
        .current_dir(&profile_path)
        .stdin(Stdio::null());
    match capsule.purpose {
        ProfilePurpose::Version => {
            if capsule.launch_strings != ["version"] {
                return Err(cli_error("version capsule launch strings are noncanonical"));
            }
            match mode {
                SupervisorMode::Production => {
                    command.arg("--version");
                }
                #[cfg(test)]
                SupervisorMode::Test(test_mode) => {
                    command.args([
                        "--exact",
                        test_mode
                            .browser_test()
                            .expect("transition mode returned before browser construction"),
                        "--nocapture",
                    ]);
                }
            }
            command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        }
        ProfilePurpose::Measurement => {
            let received = match mode {
                SupervisorMode::Production => std::env::args_os().skip(1).collect::<Vec<_>>(),
                #[cfg(test)]
                SupervisorMode::Test(_) => capsule
                    .launch_strings
                    .iter()
                    .map(OsString::from)
                    .collect::<Vec<_>>(),
            };
            let switches = validate_received_switches(&manifest, &received)?;
            let capsule_arguments = capsule
                .launch_strings
                .iter()
                .map(OsString::from)
                .collect::<Vec<_>>();
            let capsule_switches = validate_received_switches(&manifest, &capsule_arguments)?;
            if capsule_switches.get("user-data-dir") != Some(&Some(OsString::from("profile")))
                || capsule_switches
                    .keys()
                    .collect::<std::collections::BTreeSet<_>>()
                    != switches.keys().collect::<std::collections::BTreeSet<_>>()
            {
                return Err(cli_error(
                    "measurement capsule differs from the received switch set",
                ));
            }
            match mode {
                SupervisorMode::Production => {
                    for argument in received {
                        if switch_key(&argument) == Some("user-data-dir") {
                            command.arg(native_user_data_dir(&profile_path));
                        } else {
                            command.arg(argument);
                        }
                    }
                }
                #[cfg(test)]
                SupervisorMode::Test(test_mode) => {
                    command.args([
                        "--exact",
                        test_mode
                            .browser_test()
                            .expect("transition mode returned before browser construction"),
                        "--nocapture",
                    ]);
                }
            }
            command.stdout(Stdio::null()).stderr(Stdio::inherit());
        }
    }
    browser.closing_revalidate()?;
    let mut child = command.spawn().map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::Process,
            "spawn trusted layout browser",
            browser.absolute_path().display().to_string(),
            source,
        )
    })?;
    transition.unlock().map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::ArtifactTransaction,
            "release layout profile transition lock",
            journal.to_owned(),
            source,
        )
    })?;
    drop(transition);
    let status = child.wait().map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::Process,
            "wait for trusted layout browser",
            browser.absolute_path().display().to_string(),
            source,
        )
    })?;
    browser.closing_revalidate()?;
    if status.success() {
        Ok(())
    } else {
        Err(process_error(format!(
            "trusted browser exited unsuccessfully: {status}"
        )))
    }
}

#[cfg(test)]
pub(super) fn test_run_from_env() -> Result<()> {
    let capsule = std::env::var(CAPSULE_ENV)
        .map_err(|_| cli_error("crate-owned test supervisor is missing its capsule"))
        .and_then(|value| LaunchCapsule::parse_canonical(&value))?;
    let mode = std::env::var(TEST_MODE_ENV)
        .map_err(|_| cli_error("crate-owned test supervisor is missing its mode"))
        .and_then(|value| TestBrowserMode::parse(&value))?;
    run(capsule, SupervisorMode::Test(mode))
}

#[cfg(test)]
pub(super) fn test_process_command(
    executable: &std::path::Path,
    capsule: &str,
    mode: TestBrowserMode,
) -> Command {
    let mut command = Command::new(executable);
    command
        .args([
            "--exact",
            "layout::profile_tests::layout_fake_supervisor_process",
            "--nocapture",
        ])
        .env_clear()
        .env(CAPSULE_ENV, capsule)
        .env(TEST_MODE_ENV, mode.as_str())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
}

fn native_user_data_dir(profile: &std::path::Path) -> OsString {
    let mut bytes = b"--user-data-dir=".to_vec();
    bytes.extend_from_slice(profile.as_os_str().as_bytes());
    OsString::from_vec(bytes)
}

fn switch_key(argument: &OsStr) -> Option<&str> {
    let text = argument.to_str()?;
    let normalized = text.strip_prefix("--").unwrap_or(text);
    Some(
        normalized
            .split_once('=')
            .map_or(normalized, |(key, _)| key),
    )
}

fn cli_error(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::Cli,
        "authenticate layout supervisor capsule",
        detail,
    )
}

fn process_error(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::Process,
        "run trusted layout browser",
        detail,
    )
}
