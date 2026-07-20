use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::process::ExitStatus;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[cfg(test)]
use crate::core::DurabilityPhase;
use crate::core::{
    Domain, GenerationLease, HeldIdentity, NodeKind, OpaqueTreeSnapshot, PRIVATE_DIRECTORY_MODE,
    PRIVATE_FILE_MODE, RootedFs, corpus_authority_key, new_token,
};
use crate::{
    CorpusLocation, GeneratorError, GeneratorErrorKind, RelativePath, Result, Sha256Digest,
};

use super::browser::TrustedBrowser;
use super::manifest::LayoutManifest;

pub(super) const PROFILE_PARENT: &str = ".surgeist-generator/profiles/layout";
const LOCK_HEADER: &[u8] = b"surgeist-generator-lock-v1\n";
const INTENT_TEMPORARY: &str = "intent.json.temporary";
const TRANSITION_TEMPORARY: &str = "transition.lock.temporary";
const PROFILE_TEMPORARY: &str = "profile.json.temporary";
const RUNNING_TEMPORARY: &str = "running.json.temporary";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum ProfilePurpose {
    Version,
    Measurement,
}

pub(super) struct ProfileCreateContext<'a> {
    pub(super) location: &'a CorpusLocation,
    pub(super) lease: &'a GenerationLease,
    pub(super) browser: &'a TrustedBrowser,
    pub(super) manifest: &'a LayoutManifest,
}

pub(super) enum ProfileAttempt {
    Version {
        launch_strings: Vec<String>,
    },
    Measurement {
        batch_ordinal: u64,
        retry_ordinal: u64,
        launch_strings: Vec<String>,
    },
}

impl ProfileAttempt {
    fn into_parts(self) -> Result<ProfileAttemptParts> {
        match self {
            Self::Version { launch_strings } => Ok(ProfileAttemptParts {
                purpose: ProfilePurpose::Version,
                batch_ordinal: None,
                retry_ordinal: None,
                launch_strings,
            }),
            Self::Measurement {
                batch_ordinal,
                retry_ordinal,
                launch_strings,
            } => {
                validate_ordinals(
                    ProfilePurpose::Measurement,
                    Some(batch_ordinal),
                    Some(retry_ordinal),
                )?;
                Ok(ProfileAttemptParts {
                    purpose: ProfilePurpose::Measurement,
                    batch_ordinal: Some(batch_ordinal),
                    retry_ordinal: Some(retry_ordinal),
                    launch_strings,
                })
            }
        }
    }
}

struct ProfileAttemptParts {
    purpose: ProfilePurpose,
    batch_ordinal: Option<u64>,
    retry_ordinal: Option<u64>,
    launch_strings: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct IntentRecord {
    pub(super) schema_version: u8,
    pub(super) purpose: ProfilePurpose,
    pub(super) authority_key: String,
    pub(super) parent_pid: u32,
    pub(super) lease_token: String,
    pub(super) profile_token: String,
    pub(super) batch_ordinal: Option<u64>,
    pub(super) retry_ordinal: Option<u64>,
    pub(super) browser_path: RelativePath,
    pub(super) browser_identity: HeldIdentity,
    pub(super) browser_sha256: Sha256Digest,
    pub(super) launch_profile_sha256: Sha256Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ProfileRecord {
    schema_version: u8,
    profile_token: String,
    profile_path: RelativePath,
    identity: HeldIdentity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RunningRecord {
    schema_version: u8,
    profile_token: String,
    parent_pid: u32,
    supervisor_pid: u32,
    process_group_id: u32,
}

// A live, unwaited child pins its PID against reuse while the parent verifies
// that the recorded process group is still led by that exact child.
pub(super) trait OwnedSupervisorChild {
    fn id(&self) -> Option<u32>;
    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OwnedSupervisorState {
    LiveGroupLeader,
    Reaped,
}

impl OwnedSupervisorChild for std::process::Child {
    fn id(&self) -> Option<u32> {
        Some(std::process::Child::id(self))
    }

    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        std::process::Child::try_wait(self)
    }
}

impl OwnedSupervisorChild for tokio::process::Child {
    fn id(&self) -> Option<u32> {
        tokio::process::Child::id(self)
    }

    fn try_wait(&mut self) -> std::io::Result<Option<ExitStatus>> {
        tokio::process::Child::try_wait(self)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LaunchCapsule {
    pub(super) schema_version: u8,
    pub(super) owner_root_hex: String,
    pub(super) corpus_root_hex: String,
    pub(super) journal_path: RelativePath,
    pub(super) intent_sha256: Sha256Digest,
    pub(super) profile_sha256: Sha256Digest,
    pub(super) parent_pid: u32,
    pub(super) profile_token: String,
    pub(super) browser_path: RelativePath,
    pub(super) purpose: ProfilePurpose,
    pub(super) launch_strings: Vec<String>,
}

impl LaunchCapsule {
    pub(super) fn parse_canonical(value: &str) -> Result<Self> {
        let capsule: Self = serde_json::from_str(value).map_err(|source| {
            GeneratorError::with_source(
                GeneratorErrorKind::Cli,
                "parse private layout launch capsule",
                "capsule is not canonical schema-1 JSON",
                source,
            )
        })?;
        let canonical = serde_json::to_string(&capsule)
            .map_err(|source| artifact_source("serialize private layout launch capsule", source))?;
        if canonical != value
            || capsule.schema_version != 1
            || capsule.parent_pid == 0
            || !valid_token(&capsule.profile_token)
        {
            return Err(cli_error("private layout launch capsule is noncanonical"));
        }
        Ok(capsule)
    }

    pub(super) fn owner_root(&self) -> Result<PathBuf> {
        decode_path(&self.owner_root_hex, "owner_root_hex")
    }

    pub(super) fn corpus_root(&self) -> Result<PathBuf> {
        decode_path(&self.corpus_root_hex, "corpus_root_hex")
    }
}

#[derive(Debug)]
pub(super) struct ProfileJournal {
    path: String,
    identity: HeldIdentity,
    profile_path: PathBuf,
    intent_bytes: Vec<u8>,
    profile_bytes: Vec<u8>,
    capsule: LaunchCapsule,
}

impl ProfileJournal {
    pub(super) fn create(
        context: ProfileCreateContext<'_>,
        attempt: ProfileAttempt,
    ) -> Result<Self> {
        Self::create_at(&context, attempt, context.lease.rooted())
    }

    fn create_at(
        context: &ProfileCreateContext<'_>,
        attempt: ProfileAttempt,
        rooted: &RootedFs,
    ) -> Result<Self> {
        #[cfg(test)]
        let _observation = rooted.begin_observation_phase(DurabilityPhase::ProfileCreate);
        let ProfileAttemptParts {
            purpose,
            batch_ordinal,
            retry_ordinal,
            launch_strings,
        } = attempt.into_parts()?;
        rooted.ensure_dir(".surgeist-generator/profiles", PRIVATE_DIRECTORY_MODE)?;
        rooted.ensure_dir(PROFILE_PARENT, PRIVATE_DIRECTORY_MODE)?;
        if !rooted.list_dir(PROFILE_PARENT)?.is_empty() {
            return Err(artifact_error(
                "create layout browser profile",
                "another layout profile journal already exists",
            ));
        }

        let profile_token = new_token()?;
        let suffix = match purpose {
            ProfilePurpose::Version => {
                format!("{}-version-{profile_token}", context.lease.token())
            }
            ProfilePurpose::Measurement => format!(
                "{}-batch-{}-retry-{}-{profile_token}",
                context.lease.token(),
                batch_ordinal.expect("validated measurement batch"),
                retry_ordinal.expect("validated measurement retry")
            ),
        };
        let name = format!("active-{suffix}");
        let path = format!("{PROFILE_PARENT}/{name}");
        let identity = rooted.create_dir_exclusive(&path, PRIVATE_DIRECTORY_MODE)?;

        let intent = IntentRecord {
            schema_version: 1,
            purpose,
            authority_key: context.lease.authority_key().to_owned(),
            parent_pid: std::process::id(),
            lease_token: context.lease.token().to_owned(),
            profile_token: profile_token.clone(),
            batch_ordinal,
            retry_ordinal,
            browser_path: context.browser.relative().clone(),
            browser_identity: context.browser.identity().clone(),
            browser_sha256: context.browser.digest().clone(),
            launch_profile_sha256: context.manifest.launch_digest.clone(),
        };
        let intent_bytes = canonical_json_line(&intent, "serialize layout profile intent")?;
        rooted.publish_file_exclusive(
            &path,
            "intent.json",
            INTENT_TEMPORARY,
            &intent_bytes,
            PRIVATE_FILE_MODE,
        )?;
        rooted.publish_file_exclusive(
            &path,
            "transition.lock",
            TRANSITION_TEMPORARY,
            LOCK_HEADER,
            PRIVATE_FILE_MODE,
        )?;
        let profile_relative = format!("{path}/profile");
        let profile_identity =
            rooted.create_dir_exclusive(&profile_relative, PRIVATE_DIRECTORY_MODE)?;
        for directory in ["home", "tmp", "xdg-config", "xdg-cache", "xdg-data"] {
            rooted.create_dir_exclusive(
                &format!("{profile_relative}/{directory}"),
                PRIVATE_DIRECTORY_MODE,
            )?;
        }
        let profile = ProfileRecord {
            schema_version: 1,
            profile_token: profile_token.clone(),
            profile_path: RelativePath::new("profile")?,
            identity: profile_identity,
        };
        let profile_bytes = canonical_json_line(&profile, "serialize layout profile record")?;
        rooted.publish_file_exclusive(
            &path,
            "profile.json",
            PROFILE_TEMPORARY,
            &profile_bytes,
            PRIVATE_FILE_MODE,
        )?;
        rooted.sync_dir(&path)?;
        rooted.sync_dir(PROFILE_PARENT)?;

        let profile_path = context.location.corpus_root().join(&profile_relative);
        let capsule = LaunchCapsule {
            schema_version: 1,
            owner_root_hex: encode_path(context.location.owner_root()),
            corpus_root_hex: encode_path(context.location.corpus_root()),
            journal_path: RelativePath::new(&path)?,
            intent_sha256: Sha256Digest::from_bytes(&intent_bytes),
            profile_sha256: Sha256Digest::from_bytes(&profile_bytes),
            parent_pid: std::process::id(),
            profile_token,
            browser_path: context.browser.relative().clone(),
            purpose,
            launch_strings,
        };
        Ok(Self {
            path,
            identity,
            profile_path,
            intent_bytes,
            profile_bytes,
            capsule,
        })
    }

    #[cfg(test)]
    pub(super) fn create_observed(
        context: ProfileCreateContext<'_>,
        attempt: ProfileAttempt,
        rooted: &RootedFs,
    ) -> Result<Self> {
        Self::create_at(&context, attempt, rooted)
    }

    pub(super) fn capsule_json(&self) -> Result<String> {
        serde_json::to_string(&self.capsule)
            .map_err(|source| artifact_source("serialize layout launch capsule", source))
    }

    pub(super) fn profile_path(&self) -> &Path {
        &self.profile_path
    }

    #[cfg(test)]
    pub(super) fn journal_path(&self) -> &str {
        &self.path
    }

    pub(super) fn terminalize(self, rooted: &RootedFs) -> Result<()> {
        #[cfg(test)]
        let _observation = rooted.begin_observation_phase(DurabilityPhase::ProfileTerminalization);
        let snapshot = snapshot_tree(rooted, &self.path, &self.identity)?;
        if let Some(running) =
            read_optional_record::<RunningRecord>(rooted, &self.path, "running.json")?
            && probe_group(running.process_group_id)? != GroupState::Dead
        {
            return Err(process_error(
                "terminalize layout browser profile",
                "recorded browser process group remains live or inconclusive",
            ));
        }
        self.finish_terminalization(rooted, snapshot)
    }

    fn finish_terminalization(self, rooted: &RootedFs, snapshot: TreeSnapshot) -> Result<()> {
        let _transition = lock_transition(rooted, &self.path, false)?;
        let cleanup = cleanup_path(&self.path)?;
        rooted.rename_exclusive_bound(&self.path, &cleanup, &self.identity)?;
        rooted.sync_dir(PROFILE_PARENT)?;
        if snapshot_tree(rooted, &cleanup, &self.identity)? != snapshot.with_root_name(&cleanup) {
            return Err(artifact_error(
                "terminalize layout browser profile",
                "profile journal changed before cleanup",
            ));
        }
        erase_validated_journal(rooted, &cleanup)?;
        rooted.sync_dir(PROFILE_PARENT)
    }

    pub(super) fn terminalize_with_forced_group_kill(
        self,
        rooted: &RootedFs,
        owned_supervisor: Option<&mut dyn OwnedSupervisorChild>,
    ) -> Result<()> {
        #[cfg(test)]
        let _observation = rooted.begin_observation_phase(DurabilityPhase::ProfileTerminalization);
        let _verified_snapshot = snapshot_tree(rooted, &self.path, &self.identity)?;
        if let Some(running) =
            read_optional_record::<RunningRecord>(rooted, &self.path, "running.json")?
        {
            validate_running(&running)?;
            match probe_group(running.process_group_id)? {
                GroupState::Dead => {}
                GroupState::Inconclusive => {
                    return Err(process_error(
                        "terminalize layout browser profile",
                        "recorded browser process group ownership is inconclusive; no signal was sent",
                    ));
                }
                GroupState::Live => {
                    let child = owned_supervisor.ok_or_else(|| {
                        process_error(
                            "terminalize layout browser profile",
                            "recorded browser process group has no retained owned supervisor child; no signal was sent",
                        )
                    })?;
                    match verify_owned_supervisor(&running, child)? {
                        OwnedSupervisorState::LiveGroupLeader => {
                            force_kill_group(running.process_group_id)?;
                            wait_for_owned_group_exit(running.process_group_id, child)?;
                        }
                        OwnedSupervisorState::Reaped
                            if probe_group(running.process_group_id)? == GroupState::Dead => {}
                        OwnedSupervisorState::Reaped => {
                            return Err(process_error(
                                "verify owned layout browser supervisor",
                                "retained supervisor child exited while its recorded process group remained live or inconclusive; no signal was sent",
                            ));
                        }
                    }
                }
            }
        }
        self.terminalize(rooted)
    }

    pub(super) fn validates_prefix(&self, rooted: &RootedFs) -> Result<()> {
        if rooted.read_file(&format!("{}/intent.json", self.path), PRIVATE_FILE_MODE)?
            != self.intent_bytes
            || rooted.read_file(&format!("{}/profile.json", self.path), PRIVATE_FILE_MODE)?
                != self.profile_bytes
        {
            return Err(artifact_error(
                "validate layout profile prefix",
                "immutable profile records changed",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn test_publish_running(
        &self,
        rooted: &RootedFs,
        supervisor_pid: u32,
    ) -> Result<()> {
        publish_running(
            rooted,
            &self.path,
            &self.capsule.profile_token,
            self.capsule.parent_pid,
            supervisor_pid,
        )
    }
}

#[derive(Debug)]
pub(super) struct PendingRecovery {
    path: String,
    identity: HeldIdentity,
    snapshot: TreeSnapshot,
    _transition: Option<fs::File>,
}

impl PendingRecovery {
    pub(super) fn execute(self, rooted: &RootedFs) -> Result<()> {
        #[cfg(test)]
        let _observation = rooted.begin_observation_phase(DurabilityPhase::ProfileRecovery);
        if snapshot_tree(rooted, &self.path, &self.identity)? != self.snapshot {
            return Err(artifact_error(
                "recover layout browser profile",
                "profile identity or bytes changed after classification",
            ));
        }
        let cleanup = if self
            .path
            .rsplit('/')
            .next()
            .is_some_and(|name| name.starts_with("active-"))
        {
            let cleanup = cleanup_path(&self.path)?;
            rooted.rename_exclusive_bound(&self.path, &cleanup, &self.identity)?;
            rooted.sync_dir(PROFILE_PARENT)?;
            cleanup
        } else {
            self.path
        };
        erase_validated_journal(rooted, &cleanup)?;
        rooted.sync_dir(PROFILE_PARENT)
    }
}

pub(super) fn classify_pending(rooted: &RootedFs) -> Result<Option<PendingRecovery>> {
    if !rooted.exists(PROFILE_PARENT)? {
        return Ok(None);
    }
    let names = rooted.list_dir(PROFILE_PARENT)?;
    if names.is_empty() {
        return Ok(None);
    }
    if names.len() != 1 {
        return Err(artifact_error(
            "classify layout browser profiles",
            "more than one profile journal exists",
        ));
    }
    let name = &names[0];
    if !name.starts_with("active-") && !name.starts_with("cleanup-") {
        return Err(artifact_error(
            "classify layout browser profiles",
            format!("unknown profile journal: {name}"),
        ));
    }
    validate_journal_name(name)?;
    let path = format!("{PROFILE_PARENT}/{name}");
    let identity = rooted
        .identity_at(&path)?
        .ok_or_else(|| artifact_error("classify layout browser profiles", "journal disappeared"))?;
    require_private_directory(rooted, &identity, &path)?;
    validate_journal_prefix(rooted, &path)?;
    let transition = if rooted.exists(&format!("{path}/transition.lock"))? {
        Some(lock_transition(rooted, &path, true)?)
    } else {
        None
    };
    if let Some(running) = read_optional_record::<RunningRecord>(rooted, &path, "running.json")? {
        validate_running(&running)?;
        match probe_group(running.process_group_id)? {
            GroupState::Dead => {}
            GroupState::Live | GroupState::Inconclusive => {
                return Err(GeneratorError::new(
                    GeneratorErrorKind::LeaseActive,
                    "classify layout browser profiles",
                    "recorded browser process group may still be live; terminate it and retry",
                ));
            }
        }
    }
    Ok(Some(PendingRecovery {
        snapshot: snapshot_tree(rooted, &path, &identity)?,
        path,
        identity,
        _transition: transition,
    }))
}

pub(super) fn publish_running(
    rooted: &RootedFs,
    journal: &str,
    profile_token: &str,
    parent_pid: u32,
    supervisor_pid: u32,
) -> Result<()> {
    #[cfg(test)]
    let _observation = rooted.begin_observation_phase(DurabilityPhase::ProfileRunningPublication);
    if supervisor_pid == 0 {
        return Err(process_error(
            "register layout browser supervisor",
            "supervisor PID is zero",
        ));
    }
    let record = RunningRecord {
        schema_version: 1,
        profile_token: profile_token.to_owned(),
        parent_pid,
        supervisor_pid,
        process_group_id: supervisor_pid,
    };
    rooted.publish_file_exclusive(
        journal,
        "running.json",
        RUNNING_TEMPORARY,
        &canonical_json_line(&record, "serialize running browser group")?,
        PRIVATE_FILE_MODE,
    )?;
    rooted.sync_dir(journal)
}

pub(super) fn validate_capsule_records(
    rooted: &RootedFs,
    capsule: &LaunchCapsule,
) -> Result<(IntentRecord, ProfileRecord)> {
    let journal = capsule.journal_path.as_str();
    if !journal.starts_with(&format!("{PROFILE_PARENT}/active-")) {
        return Err(cli_error(
            "capsule journal is outside the active layout profile root",
        ));
    }
    validate_journal_prefix(rooted, journal).map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::Cli,
            "authenticate layout supervisor capsule",
            source.to_string(),
            source,
        )
    })?;
    let intent_bytes = rooted.read_file(&format!("{journal}/intent.json"), PRIVATE_FILE_MODE)?;
    let profile_bytes = rooted.read_file(&format!("{journal}/profile.json"), PRIVATE_FILE_MODE)?;
    if Sha256Digest::from_bytes(&intent_bytes) != capsule.intent_sha256
        || Sha256Digest::from_bytes(&profile_bytes) != capsule.profile_sha256
    {
        return Err(cli_error(
            "capsule record digest does not match its journal",
        ));
    }
    let intent: IntentRecord = parse_canonical_line(&intent_bytes, "layout profile intent")?;
    let profile: ProfileRecord = parse_canonical_line(&profile_bytes, "layout profile record")?;
    if intent.schema_version != 1
        || profile.schema_version != 1
        || intent.profile_token != capsule.profile_token
        || profile.profile_token != capsule.profile_token
        || intent.parent_pid != capsule.parent_pid
        || intent.browser_path != capsule.browser_path
        || intent.purpose != capsule.purpose
        || profile.profile_path.as_str() != "profile"
    {
        return Err(cli_error(
            "capsule fields do not match immutable profile records",
        ));
    }
    let profile_identity = rooted
        .identity_at(&format!("{journal}/profile"))?
        .ok_or_else(|| cli_error("capsule profile directory is absent"))?;
    if !profile.identity.matches_recovery(&profile_identity) {
        return Err(cli_error("capsule profile identity changed"));
    }
    Ok((intent, profile))
}

fn validate_journal_prefix(rooted: &RootedFs, path: &str) -> Result<()> {
    let names = rooted.list_dir(path)?.into_iter().collect::<BTreeSet<_>>();
    let set = |values: &[&str]| {
        values
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<BTreeSet<_>>()
    };
    let active_prefix = [
        set(&[]),
        set(&[INTENT_TEMPORARY]),
        set(&["intent.json"]),
        set(&["intent.json", TRANSITION_TEMPORARY]),
        set(&["intent.json", "transition.lock"]),
        set(&["intent.json", "transition.lock", "profile"]),
        set(&[
            "intent.json",
            "transition.lock",
            "profile",
            PROFILE_TEMPORARY,
        ]),
        set(&["intent.json", "transition.lock", "profile", "profile.json"]),
        set(&[
            "intent.json",
            "transition.lock",
            "profile",
            "profile.json",
            RUNNING_TEMPORARY,
        ]),
        set(&[
            "intent.json",
            "transition.lock",
            "profile",
            "profile.json",
            "running.json",
        ]),
    ];
    let cleanup_prefix = [
        set(&[]),
        set(&[INTENT_TEMPORARY]),
        set(&["intent.json"]),
        set(&["intent.json", TRANSITION_TEMPORARY]),
        set(&["intent.json", "transition.lock"]),
        set(&["intent.json", "transition.lock", PROFILE_TEMPORARY]),
        set(&["intent.json", "transition.lock", "profile.json"]),
        set(&[
            "intent.json",
            "transition.lock",
            "profile.json",
            RUNNING_TEMPORARY,
        ]),
        set(&[
            "intent.json",
            "transition.lock",
            "profile.json",
            "running.json",
        ]),
    ];
    let is_active = path
        .rsplit('/')
        .next()
        .is_some_and(|name| name.starts_with("active-"));
    if !(if is_active {
        active_prefix.contains(&names)
    } else {
        active_prefix.contains(&names) || cleanup_prefix.contains(&names)
    }) {
        return Err(artifact_error(
            "validate layout profile journal",
            "journal contains an unknown or out-of-order member",
        ));
    }
    for temporary in [
        INTENT_TEMPORARY,
        TRANSITION_TEMPORARY,
        PROFILE_TEMPORARY,
        RUNNING_TEMPORARY,
    ] {
        if names.contains(temporary) {
            let _ = rooted.read_file(&format!("{path}/{temporary}"), PRIVATE_FILE_MODE)?;
        }
    }
    let mut intent_record = None;
    if names.contains("intent.json") {
        let intent: IntentRecord = read_record(rooted, path, "intent.json")?;
        validate_intent(rooted, path, &intent)?;
        intent_record = Some(intent);
    }
    if names.contains("profile.json") {
        let record: ProfileRecord = read_record(rooted, path, "profile.json")?;
        if record.schema_version != 1
            || record.profile_path.as_str() != "profile"
            || intent_record
                .as_ref()
                .is_none_or(|intent| intent.profile_token != record.profile_token)
        {
            return Err(artifact_error(
                "validate layout profile journal",
                "profile record fields are noncanonical",
            ));
        }
        if names.contains("profile") {
            let actual = rooted
                .identity_at(&format!("{path}/profile"))?
                .ok_or_else(|| {
                    artifact_error("validate layout profile journal", "profile disappeared")
                })?;
            require_private_directory(rooted, &actual, &format!("{path}/profile"))?;
            if !record.identity.matches_recovery(&actual) {
                return Err(artifact_error(
                    "validate layout profile journal",
                    "profile directory identity differs from profile.json",
                ));
            }
        }
    }
    if names.contains("transition.lock")
        && rooted.read_file(&format!("{path}/transition.lock"), PRIVATE_FILE_MODE)? != LOCK_HEADER
    {
        return Err(artifact_error(
            "validate layout profile journal",
            "transition lock header is invalid",
        ));
    }
    if names.contains("running.json") {
        let running: RunningRecord = read_record(rooted, path, "running.json")?;
        validate_running(&running)?;
        if intent_record.as_ref().is_none_or(|intent| {
            running.profile_token != intent.profile_token || running.parent_pid != intent.parent_pid
        }) {
            return Err(artifact_error(
                "validate layout profile journal",
                "running record differs from immutable intent",
            ));
        }
    }
    Ok(())
}

fn validate_intent(rooted: &RootedFs, path: &str, intent: &IntentRecord) -> Result<()> {
    validate_ordinals(intent.purpose, intent.batch_ordinal, intent.retry_ordinal)?;
    if intent.schema_version != 1
        || intent.parent_pid == 0
        || !valid_token(&intent.lease_token)
        || !valid_token(&intent.profile_token)
        || intent.authority_key != corpus_authority_key(rooted, Domain::Layout)
        || intent.browser_identity.kind() != NodeKind::Regular
        || intent.browser_identity.link_count() != Some(1)
        || intent.browser_identity.mode() & 0o111 == 0
    {
        return Err(artifact_error(
            "validate layout profile intent",
            "intent fields are noncanonical or unauthenticated",
        ));
    }
    let name = path.rsplit('/').next().ok_or_else(|| {
        artifact_error("validate layout profile intent", "journal path has no name")
    })?;
    let suffix = name
        .strip_prefix("active-")
        .or_else(|| name.strip_prefix("cleanup-"))
        .ok_or_else(|| artifact_error("validate layout profile intent", "unknown journal name"))?;
    let expected = match intent.purpose {
        ProfilePurpose::Version => {
            format!("{}-version-{}", intent.lease_token, intent.profile_token)
        }
        ProfilePurpose::Measurement => format!(
            "{}-batch-{}-retry-{}-{}",
            intent.lease_token,
            intent.batch_ordinal.expect("validated measurement batch"),
            intent.retry_ordinal.expect("validated measurement retry"),
            intent.profile_token
        ),
    };
    if suffix != expected {
        return Err(artifact_error(
            "validate layout profile intent",
            "journal name differs from intent purpose, ordinals, or tokens",
        ));
    }
    Ok(())
}

fn erase_validated_journal(rooted: &RootedFs, path: &str) -> Result<()> {
    validate_journal_prefix(rooted, path)?;
    if rooted.exists(&format!("{path}/profile"))? {
        let profile = rooted
            .identity_at(&format!("{path}/profile"))?
            .ok_or_else(|| {
                artifact_error("erase layout profile", "profile directory disappeared")
            })?;
        erase_opaque(rooted, &format!("{path}/profile"), &profile)?;
    }
    for name in [
        "running.json",
        RUNNING_TEMPORARY,
        "profile.json",
        PROFILE_TEMPORARY,
        "transition.lock",
        TRANSITION_TEMPORARY,
        "intent.json",
        INTENT_TEMPORARY,
    ] {
        let member = format!("{path}/{name}");
        if let Some(identity) = rooted.identity_at(&member)? {
            rooted.remove_file_exact(&member, &identity)?;
        }
    }
    let identity = rooted
        .identity_at(path)?
        .ok_or_else(|| artifact_error("erase layout profile", "profile journal disappeared"))?;
    rooted.remove_dir_exact(path, &identity)
}

fn erase_opaque(rooted: &RootedFs, relative: &str, expected: &HeldIdentity) -> Result<()> {
    rooted.erase_opaque_directory(relative, expected)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TreeSnapshot {
    root_name: String,
    raw: OpaqueTreeSnapshot,
}

impl TreeSnapshot {
    fn with_root_name(&self, root_name: &str) -> Self {
        Self {
            root_name: root_name.to_owned(),
            raw: self.raw.clone(),
        }
    }
}

fn snapshot_tree(
    rooted: &RootedFs,
    relative: &str,
    expected: &HeldIdentity,
) -> Result<TreeSnapshot> {
    Ok(TreeSnapshot {
        root_name: relative.to_owned(),
        raw: rooted.snapshot_opaque_directory(relative, expected)?,
    })
}

pub(super) fn lock_transition(
    rooted: &RootedFs,
    journal: &str,
    recovery: bool,
) -> Result<fs::File> {
    use std::fs::TryLockError;

    let path = format!("{journal}/transition.lock");
    let file = rooted.open_file_handle(&path, PRIVATE_FILE_MODE, false)?;
    let mut copy = file.try_clone().map_err(|source| {
        artifact_io(
            "clone layout profile transition lock",
            Path::new(&path),
            source,
        )
    })?;
    copy.seek(SeekFrom::Start(0)).map_err(|source| {
        artifact_io(
            "seek layout profile transition lock",
            Path::new(&path),
            source,
        )
    })?;
    let mut bytes = Vec::new();
    copy.read_to_end(&mut bytes).map_err(|source| {
        artifact_io(
            "read layout profile transition lock",
            Path::new(&path),
            source,
        )
    })?;
    if bytes != LOCK_HEADER {
        return Err(artifact_error(
            "lock layout profile transition",
            "transition lock header is invalid",
        ));
    }
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) if recovery => Err(GeneratorError::new(
            GeneratorErrorKind::LeaseActive,
            "classify layout browser profiles",
            "profile transition lock is held",
        )),
        Err(TryLockError::WouldBlock) => Err(process_error(
            "terminalize layout browser profile",
            "profile transition is still active",
        )),
        Err(TryLockError::Error(source)) => Err(artifact_io(
            "lock layout profile transition",
            Path::new(&path),
            source,
        )),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GroupState {
    Dead,
    Live,
    Inconclusive,
}

fn probe_group(group: u32) -> Result<GroupState> {
    let raw = i32::try_from(group)
        .map_err(|_| process_error("probe browser process group", "group ID exceeds i32"))?;
    let pid = rustix::process::Pid::from_raw(raw)
        .ok_or_else(|| process_error("probe browser process group", "group ID is zero"))?;
    match rustix::process::test_kill_process_group(pid) {
        Ok(()) => Ok(GroupState::Live),
        Err(rustix::io::Errno::SRCH) => Ok(GroupState::Dead),
        Err(rustix::io::Errno::PERM) => Ok(GroupState::Inconclusive),
        Err(source) => Err(GeneratorError::with_source(
            GeneratorErrorKind::Process,
            "probe browser process group",
            "group liveness is inconclusive",
            source,
        )),
    }
}

fn verify_owned_supervisor(
    running: &RunningRecord,
    child: &mut dyn OwnedSupervisorChild,
) -> Result<OwnedSupervisorState> {
    let owned_pid = child.id().ok_or_else(|| {
        process_error(
            "verify owned layout browser supervisor",
            "owned supervisor child has no live process identity; no signal was sent",
        )
    })?;
    if owned_pid != running.supervisor_pid || owned_pid != running.process_group_id {
        return Err(process_error(
            "verify owned layout browser supervisor",
            "running record differs from the retained owned supervisor child; no signal was sent",
        ));
    }
    if child
        .try_wait()
        .map_err(|source| {
            GeneratorError::with_source(
                GeneratorErrorKind::Process,
                "verify owned layout browser supervisor",
                "cannot prove the retained supervisor child is still live; no signal was sent",
                source,
            )
        })?
        .is_some()
    {
        return Ok(OwnedSupervisorState::Reaped);
    }
    let pid = process_pid(owned_pid, "verify owned layout browser supervisor")?;
    let actual_group = rustix::process::getpgid(Some(pid)).map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::Process,
            "verify owned layout browser supervisor",
            "cannot prove the retained supervisor child still leads the recorded process group; no signal was sent",
            source,
        )
    })?;
    if actual_group != pid {
        return Err(process_error(
            "verify owned layout browser supervisor",
            "retained supervisor child no longer leads the recorded process group; no signal was sent",
        ));
    }
    Ok(OwnedSupervisorState::LiveGroupLeader)
}

fn wait_for_owned_group_exit(group: u32, child: &mut dyn OwnedSupervisorChild) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        child.try_wait().map_err(|source| {
            GeneratorError::with_source(
                GeneratorErrorKind::Process,
                "reap owned layout browser supervisor",
                "failed to reap the retained supervisor child after SIGKILL",
                source,
            )
        })?;
        if probe_group(group)? == GroupState::Dead {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(process_error(
                "terminalize layout browser profile",
                "recorded browser process group remained live or inconclusive after SIGKILL",
            ));
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn process_pid(value: u32, operation: &str) -> Result<rustix::process::Pid> {
    let raw =
        i32::try_from(value).map_err(|_| process_error(operation, "process ID exceeds i32"))?;
    rustix::process::Pid::from_raw(raw)
        .ok_or_else(|| process_error(operation, "process ID is zero"))
}

pub(super) fn force_kill_group(group: u32) -> Result<()> {
    let raw = i32::try_from(group)
        .map_err(|_| process_error("kill browser process group", "group ID exceeds i32"))?;
    let pid = rustix::process::Pid::from_raw(raw)
        .ok_or_else(|| process_error("kill browser process group", "group ID is zero"))?;
    match rustix::process::kill_process_group(pid, rustix::process::Signal::KILL) {
        Ok(()) | Err(rustix::io::Errno::SRCH) => Ok(()),
        Err(source) => Err(GeneratorError::with_source(
            GeneratorErrorKind::Process,
            "kill browser process group",
            "failed to signal the recorded browser process group",
            source,
        )),
    }
}

fn validate_running(record: &RunningRecord) -> Result<()> {
    if record.schema_version != 1
        || !valid_token(&record.profile_token)
        || record.parent_pid == 0
        || record.supervisor_pid == 0
        || record.supervisor_pid != record.process_group_id
    {
        return Err(artifact_error(
            "validate running browser group record",
            "running record fields are noncanonical",
        ));
    }
    Ok(())
}

fn validate_ordinals(
    purpose: ProfilePurpose,
    batch: Option<u64>,
    retry: Option<u64>,
) -> Result<()> {
    let valid = match purpose {
        ProfilePurpose::Version => batch.is_none() && retry.is_none(),
        ProfilePurpose::Measurement => batch.is_some() && matches!(retry, Some(0 | 1)),
    };
    if valid {
        Ok(())
    } else {
        Err(artifact_error(
            "construct layout profile journal",
            "purpose and batch/retry ordinals do not match",
        ))
    }
}

fn validate_journal_name(name: &str) -> Result<()> {
    let suffix = name
        .strip_prefix("active-")
        .or_else(|| name.strip_prefix("cleanup-"))
        .ok_or_else(|| artifact_error("validate layout profile journal", "unknown journal name"))?;
    let tokens = suffix.split('-').collect::<Vec<_>>();
    let valid = (tokens.len() == 3
        && valid_token(tokens[0])
        && tokens[1] == "version"
        && valid_token(tokens[2]))
        || (tokens.len() == 6
            && valid_token(tokens[0])
            && tokens[1] == "batch"
            && tokens[2].parse::<u64>().is_ok()
            && tokens[3] == "retry"
            && matches!(tokens[4], "0" | "1")
            && valid_token(tokens[5]));
    if valid {
        Ok(())
    } else {
        Err(artifact_error(
            "validate layout profile journal",
            "journal name does not match its purpose/token grammar",
        ))
    }
}

fn cleanup_path(active: &str) -> Result<String> {
    let (parent, name) = active.rsplit_once('/').ok_or_else(|| {
        artifact_error("derive profile cleanup path", "journal path has no parent")
    })?;
    let suffix = name
        .strip_prefix("active-")
        .ok_or_else(|| artifact_error("derive profile cleanup path", "journal is not active"))?;
    Ok(format!("{parent}/cleanup-{suffix}"))
}

fn require_private_directory(rooted: &RootedFs, identity: &HeldIdentity, path: &str) -> Result<()> {
    if identity.kind() != NodeKind::Directory
        || identity.mode() != PRIVATE_DIRECTORY_MODE
        || identity.owner() != rooted.identity().owner()
        || identity.device() != rooted.identity().device()
        || identity.fsid() != rooted.identity().fsid()
    {
        return Err(artifact_error(
            "validate layout profile journal",
            format!("wrong journal type, mode, owner, or mount: {path}"),
        ));
    }
    Ok(())
}

fn read_record<T: DeserializeOwned + Serialize>(
    rooted: &RootedFs,
    journal: &str,
    name: &str,
) -> Result<T> {
    parse_canonical_line(
        &rooted.read_file(&format!("{journal}/{name}"), PRIVATE_FILE_MODE)?,
        name,
    )
}

fn read_optional_record<T: DeserializeOwned + Serialize>(
    rooted: &RootedFs,
    journal: &str,
    name: &str,
) -> Result<Option<T>> {
    if rooted.exists(&format!("{journal}/{name}"))? {
        read_record(rooted, journal, name).map(Some)
    } else {
        Ok(None)
    }
}

fn parse_canonical_line<T: DeserializeOwned + Serialize>(bytes: &[u8], label: &str) -> Result<T> {
    let value: T = serde_json::from_slice(bytes).map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::ArtifactTransaction,
            "parse layout profile metadata",
            format!("invalid {label}"),
            source,
        )
    })?;
    if canonical_json_line(&value, "reserialize layout profile metadata")? != bytes {
        return Err(artifact_error(
            "parse layout profile metadata",
            format!("{label} is not compact canonical JSON plus LF"),
        ));
    }
    Ok(value)
}

fn canonical_json_line<T: Serialize>(value: &T, operation: &str) -> Result<Vec<u8>> {
    let mut bytes =
        serde_json::to_vec(value).map_err(|source| artifact_source(operation, source))?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn valid_token(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn encode_path(path: &Path) -> String {
    path.as_os_str()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn decode_path(value: &str, label: &str) -> Result<PathBuf> {
    if value.is_empty()
        || !value.len().is_multiple_of(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(cli_error(format!(
            "{label} is not lowercase even-width hex"
        )));
    }
    let mut bytes = Vec::with_capacity(value.len() / 2);
    for pair in value.as_bytes().chunks_exact(2) {
        let text = std::str::from_utf8(pair).expect("hex pairs are ASCII");
        bytes.push(
            u8::from_str_radix(text, 16)
                .map_err(|_| cli_error(format!("{label} contains an invalid hex byte")))?,
        );
    }
    let path = PathBuf::from(OsString::from_vec(bytes));
    if !path.is_absolute() {
        return Err(cli_error(format!(
            "{label} does not encode an absolute path"
        )));
    }
    Ok(path)
}

fn artifact_error(operation: &str, detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(GeneratorErrorKind::ArtifactTransaction, operation, detail)
}

fn artifact_source<E>(operation: &str, source: E) -> GeneratorError
where
    E: std::error::Error + Send + Sync + 'static,
{
    GeneratorError::with_source(
        GeneratorErrorKind::ArtifactTransaction,
        operation,
        source.to_string(),
        source,
    )
}

fn artifact_io(operation: &str, path: &Path, source: std::io::Error) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::ArtifactTransaction,
        operation,
        path.display().to_string(),
        source,
    )
}

fn process_error(operation: &str, detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(GeneratorErrorKind::Process, operation, detail)
}

pub(super) fn resolve_terminalization<T>(primary: Result<T>, cleanup: Result<()>) -> Result<T> {
    match (primary, cleanup) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(primary), Ok(())) => Err(primary),
        (Ok(_), Err(cleanup)) => Err(cleanup),
        (Err(primary), Err(cleanup)) => Err(GeneratorError::with_source(
            cleanup.kind(),
            "terminalize layout browser attempt",
            format!("primary failure: {primary}; cleanup failure: {cleanup}"),
            cleanup,
        )),
    }
}

fn cli_error(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::Cli,
        "validate private layout launch capsule",
        detail,
    )
}

#[cfg(test)]
pub(super) fn test_validate_journal_name(name: &str) -> Result<()> {
    validate_journal_name(name)
}

#[cfg(test)]
pub(super) fn test_cleanup_path(path: &str) -> Result<String> {
    cleanup_path(path)
}

#[cfg(test)]
pub(super) fn test_group_is_dead(group: u32) -> Result<bool> {
    probe_group(group).map(|state| state == GroupState::Dead)
}
