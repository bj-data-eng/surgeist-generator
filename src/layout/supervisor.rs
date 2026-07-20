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
const MUTEX: &str = ".surgeist-generator/leases/layout/mutex.lock";

pub(super) fn run_from_env_if_present() -> Option<Result<()>> {
    let value = std::env::var_os(CAPSULE_ENV)?;
    Some(
        value
            .into_string()
            .map_err(|_| cli_error("private launch capsule must be UTF-8"))
            .and_then(|value| LaunchCapsule::parse_canonical(&value))
            .and_then(run),
    )
}

fn run(capsule: LaunchCapsule) -> Result<()> {
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
            command.arg("--version");
            command.stdout(Stdio::inherit()).stderr(Stdio::inherit());
        }
        ProfilePurpose::Measurement => {
            let received = std::env::args_os().skip(1).collect::<Vec<_>>();
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
            for argument in received {
                if switch_key(&argument) == Some("user-data-dir") {
                    command.arg(native_user_data_dir(&profile_path));
                } else {
                    command.arg(argument);
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
