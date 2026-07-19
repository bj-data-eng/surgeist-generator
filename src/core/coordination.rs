use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{CorpusLocation, GeneratorError, GeneratorErrorKind, Result, RunScope, Sha256Digest};

#[cfg(test)]
use super::fs::{DurabilityPhase, RootedObserver};
use super::fs::{
    HeldIdentity, MutationTarget, NodeKind, PRIVATE_DIRECTORY_MODE, PRIVATE_FILE_MODE, RootedFs,
};
use super::transaction::TransactionEngine;

pub(crate) const LOCK_HEADER: &[u8] = b"surgeist-generator-lock-v1\n";
const COORDINATION_ROOT: &str = ".surgeist-generator";
const BOOTSTRAP_LOCKS: &str = ".surgeist-generator/bootstrap/locks";
const ACQUISITION_LOCK: &str = ".surgeist-generator/acquisition.lock";
const OWNER_RECORD: &str = "owner.json";
const OWNER_TRANSACTIONS: &str = "owner-transactions";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Domain {
    Layout,
    Css,
}

impl Domain {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Layout => "layout",
            Self::Css => "css",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CoordinationAccess {
    Shared,
    Exclusive,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum BootstrapStep {
    ReserveIntentDirectory,
    PublishIntent,
    CreateZeroStage,
    PublishStageIdentity,
    WriteAndSyncHeader,
    LockStage,
    RenameExclusive,
    SyncParents,
    ReleaseStageBeforeLostMarker,
    PublishLostContended,
    ClaimRecoveryName,
    PublishCleanupStarted,
    RemoveVerifiedMembers,
    RemoveCleanupMarker,
    RemoveClaimedDirectory,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct BootstrapProtocol {
    domain: Domain,
}

#[cfg(test)]
impl BootstrapProtocol {
    pub(crate) const fn new(domain: Domain) -> Self {
        Self { domain }
    }

    pub(crate) const fn steps(&self) -> &'static [BootstrapStep] {
        &[
            BootstrapStep::ReserveIntentDirectory,
            BootstrapStep::PublishIntent,
            BootstrapStep::CreateZeroStage,
            BootstrapStep::PublishStageIdentity,
            BootstrapStep::WriteAndSyncHeader,
            BootstrapStep::LockStage,
            BootstrapStep::RenameExclusive,
            BootstrapStep::SyncParents,
            BootstrapStep::ReleaseStageBeforeLostMarker,
            BootstrapStep::PublishLostContended,
            BootstrapStep::ClaimRecoveryName,
            BootstrapStep::PublishCleanupStarted,
            BootstrapStep::RemoveVerifiedMembers,
            BootstrapStep::RemoveCleanupMarker,
            BootstrapStep::RemoveClaimedDirectory,
        ]
    }

    pub(crate) fn steps_are_journaled(&self) -> bool {
        self.steps()[..8].contains(&BootstrapStep::PublishIntent)
            && self.steps()[..8].contains(&BootstrapStep::PublishStageIdentity)
            && self
                .steps()
                .iter()
                .position(|step| *step == BootstrapStep::ReleaseStageBeforeLostMarker)
                < self
                    .steps()
                    .iter()
                    .position(|step| *step == BootstrapStep::PublishLostContended)
            && self.domain.as_str().len() == 3 + usize::from(self.domain == Domain::Layout) * 3
    }
}

#[derive(Debug)]
pub(crate) struct CoordinationState {
    rooted: RootedFs,
    canonical_corpus: std::path::PathBuf,
    corpus_identity: HeldIdentity,
    domain: Domain,
    token: Option<String>,
    authority_key: String,
    transaction_parent: String,
    live: AtomicBool,
    operation_active: AtomicBool,
    _mutex: Option<File>,
    _prebootstrap_gate: Option<File>,
}

impl CoordinationState {
    pub(crate) fn rooted(&self) -> &RootedFs {
        &self.rooted
    }

    pub(crate) fn canonical_corpus(&self) -> &Path {
        &self.canonical_corpus
    }

    pub(crate) const fn corpus_identity(&self) -> &HeldIdentity {
        &self.corpus_identity
    }

    pub(crate) const fn domain(&self) -> Domain {
        self.domain
    }

    pub(crate) fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    pub(crate) fn authority_key(&self) -> &str {
        &self.authority_key
    }

    pub(crate) fn transaction_parent(&self) -> &str {
        &self.transaction_parent
    }

    pub(crate) fn is_live(&self) -> bool {
        self.live.load(Ordering::Acquire)
    }

    pub(crate) fn try_begin_operation(&self) -> bool {
        self.operation_active
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    pub(crate) fn finish_operation(&self) {
        self.operation_active.store(false, Ordering::Release);
    }
}

#[derive(Debug)]
pub(crate) struct CoordinationGuard {
    state: Arc<CoordinationState>,
    access: CoordinationAccess,
    absent_coordination: bool,
}

impl CoordinationGuard {
    pub(crate) fn state(&self) -> &Arc<CoordinationState> {
        &self.state
    }

    pub(crate) const fn access(&self) -> CoordinationAccess {
        self.access
    }

    pub(crate) fn finish_check(self) -> Result<()> {
        if self.access != CoordinationAccess::Shared {
            return Err(transaction_error(
                "finish generation check guard",
                "exclusive lease cannot finish as a check",
            ));
        }
        self.state
            .rooted
            .revalidate_root()
            .map_err(verification_from)?;
        if self.absent_coordination
            && self
                .state
                .rooted
                .exists(COORDINATION_ROOT)
                .map_err(verification_from)?
        {
            return Err(verification_error(
                "finish generation check guard",
                "coordination appeared during a read-only check",
            ));
        }
        if !self.absent_coordination {
            inspect_read_only_residue(&self.state.rooted, self.state.domain)?;
        }
        self.state.live.store(false, Ordering::Release);
        Ok(())
    }
}

impl Drop for CoordinationGuard {
    fn drop(&mut self) {
        self.state.live.store(false, Ordering::Release);
    }
}

#[derive(Clone, Debug)]
pub(crate) struct LeaseMetadata {
    pub(crate) generator: String,
    pub(crate) scope: String,
    pub(crate) command: String,
}

impl LeaseMetadata {
    pub(crate) fn new(
        generator: impl Into<String>,
        scope: &RunScope,
        command: impl Into<String>,
    ) -> Result<Self> {
        let generator = generator.into();
        let command = command.into();
        validate_identifier(&generator, "generator")?;
        validate_identifier(&command, "command")?;
        let scope = match scope {
            RunScope::Full => "full".to_owned(),
            RunScope::Filtered(path) => format!("filtered:{}", path.as_str()),
        };
        Ok(Self {
            generator,
            scope,
            command,
        })
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProbeCapabilityFault {
    FailExclusiveRename,
    FailSwapRename,
}

#[cfg(test)]
pub(crate) struct ProbeInstallControl {
    fault: ProbeCapabilityFault,
    fail_cleanup_at: Option<usize>,
    cleanup_trace: Vec<String>,
}

#[cfg(test)]
impl ProbeInstallControl {
    pub(crate) fn new(fault: ProbeCapabilityFault) -> Self {
        Self {
            fault,
            fail_cleanup_at: None,
            cleanup_trace: Vec::new(),
        }
    }

    pub(crate) fn failing_cleanup(fault: ProbeCapabilityFault, cleanup_index: usize) -> Self {
        Self {
            fault,
            fail_cleanup_at: Some(cleanup_index),
            cleanup_trace: Vec::new(),
        }
    }

    pub(crate) fn cleanup_trace(&self) -> &[String] {
        &self.cleanup_trace
    }
}

#[cfg(test)]
pub(crate) struct ProbeRecoveryControl<'a> {
    before_mutation: &'a mut dyn FnMut(&str) -> Result<()>,
}

#[cfg(test)]
impl<'a> ProbeRecoveryControl<'a> {
    pub(crate) fn new(before_mutation: &'a mut dyn FnMut(&str) -> Result<()>) -> Self {
        Self { before_mutation }
    }
}

#[cfg(test)]
pub(crate) struct ExclusiveAcquisitionControl {
    token: String,
    observer: RootedObserver,
    fault: Option<ProbeCapabilityFault>,
    probe_cleanup_failure_at: Option<usize>,
}

#[cfg(test)]
impl ExclusiveAcquisitionControl {
    pub(crate) fn new(
        token: &str,
        observer: RootedObserver,
        fault: Option<ProbeCapabilityFault>,
    ) -> Self {
        Self {
            token: token.to_owned(),
            observer,
            fault,
            probe_cleanup_failure_at: None,
        }
    }

    pub(crate) fn failing_probe_cleanup(
        token: &str,
        observer: RootedObserver,
        fault: ProbeCapabilityFault,
        cleanup_index: usize,
    ) -> Self {
        Self {
            token: token.to_owned(),
            observer,
            fault: Some(fault),
            probe_cleanup_failure_at: Some(cleanup_index),
        }
    }
}

pub(crate) fn acquire_exclusive(
    location: &CorpusLocation,
    domain: Domain,
    metadata: LeaseMetadata,
    protected_revalidation: impl FnOnce(&RootedFs) -> Result<()>,
) -> Result<CoordinationGuard> {
    #[cfg(test)]
    let result = acquire_exclusive_inner(location, domain, metadata, protected_revalidation, None);
    #[cfg(not(test))]
    let result = acquire_exclusive_inner(location, domain, metadata, protected_revalidation);
    result
}

fn acquire_exclusive_inner(
    location: &CorpusLocation,
    domain: Domain,
    metadata: LeaseMetadata,
    protected_revalidation: impl FnOnce(&RootedFs) -> Result<()>,
    #[cfg(test)] control: Option<&mut ExclusiveAcquisitionControl>,
) -> Result<CoordinationGuard> {
    MutationTarget::current().require_supported("acquire generation mutation lease")?;
    #[cfg(test)]
    let rooted = if let Some(control) = control.as_ref() {
        RootedFs::open_corpus_observed(location, control.observer.clone())?
    } else {
        RootedFs::open_corpus(location)?
    };
    #[cfg(not(test))]
    let rooted = RootedFs::open_corpus(location)?;
    if rooted.exists(COORDINATION_ROOT)? {
        validate_coordination_tree(&rooted, domain, false)?;
    }
    rooted.ensure_dir(COORDINATION_ROOT, PRIVATE_DIRECTORY_MODE)?;
    rooted.ensure_dir(".surgeist-generator/bootstrap", PRIVATE_DIRECTORY_MODE)?;
    rooted.ensure_dir(BOOTSTRAP_LOCKS, PRIVATE_DIRECTORY_MODE)?;
    validate_coordination_tree(&rooted, domain, false)?;
    recover_bootstrap(&rooted)?;
    #[cfg(test)]
    let token = if let Some(control) = control.as_ref() {
        validate_token(&control.token)?;
        control.token.clone()
    } else {
        new_token()?
    };
    #[cfg(not(test))]
    let token = new_token()?;
    let gate = open_or_bootstrap_lock(
        &rooted,
        ACQUISITION_LOCK,
        "acquisition",
        &token,
        CoordinationAccess::Exclusive,
    )?;
    validate_coordination_tree(&rooted, domain, false)?;
    require_one_domain(&rooted, domain, false)?;

    rooted.ensure_dir(".surgeist-generator/leases", PRIVATE_DIRECTORY_MODE)?;
    rooted.ensure_dir(
        &format!(".surgeist-generator/leases/{}", domain.as_str()),
        PRIVATE_DIRECTORY_MODE,
    )?;
    rooted.ensure_dir(
        &format!(
            ".surgeist-generator/leases/{}/{}",
            domain.as_str(),
            OWNER_TRANSACTIONS
        ),
        PRIVATE_DIRECTORY_MODE,
    )?;
    rooted.ensure_dir(".surgeist-generator/transactions", PRIVATE_DIRECTORY_MODE)?;
    let transaction_parent = format!(".surgeist-generator/transactions/{}", domain.as_str());
    rooted.ensure_dir(&transaction_parent, PRIVATE_DIRECTORY_MODE)?;
    rooted.ensure_dir(".surgeist-generator/probes", PRIVATE_DIRECTORY_MODE)?;
    let probe_parent = format!(".surgeist-generator/probes/{}", domain.as_str());
    rooted.ensure_dir(&probe_parent, PRIVATE_DIRECTORY_MODE)?;

    let mutex_path = mutex_path(domain);
    let mutex = match open_or_bootstrap_lock(
        &rooted,
        &mutex_path,
        &format!("{}-mutex", domain.as_str()),
        &token,
        CoordinationAccess::Exclusive,
    ) {
        Ok(mutex) => mutex,
        Err(error) => {
            drop(gate);
            return Err(error);
        }
    };

    let authority_key = corpus_authority_key(&rooted, domain);
    let engine = TransactionEngine::new(
        &rooted,
        &transaction_parent,
        &authority_key,
        domain.as_str(),
    )?;
    engine.recover_all()?;
    recover_owner_transactions(&rooted, domain, &authority_key)?;
    recover_probe_journals(&rooted, domain)?;
    #[cfg(test)]
    if let Some(fault) = control.as_ref().and_then(|control| control.fault) {
        let mut probe_control = control
            .as_ref()
            .and_then(|control| control.probe_cleanup_failure_at)
            .map_or_else(
                || ProbeInstallControl::new(fault),
                |cleanup_index| ProbeInstallControl::failing_cleanup(fault, cleanup_index),
            );
        run_rename_probe_controlled(&rooted, domain, &token, &mut probe_control)?;
    } else {
        run_rename_probe(&rooted, domain, &token)?;
    }
    #[cfg(not(test))]
    run_rename_probe(&rooted, domain, &token)?;
    protected_revalidation(&rooted)?;
    install_owner_record(&rooted, location, domain, &metadata, &token, &authority_key)?;
    drop(gate);

    let state = Arc::new(CoordinationState {
        canonical_corpus: location.corpus_root().to_path_buf(),
        corpus_identity: rooted.identity().clone(),
        domain,
        token: Some(token),
        authority_key,
        transaction_parent,
        rooted,
        live: AtomicBool::new(true),
        operation_active: AtomicBool::new(false),
        _mutex: Some(mutex),
        _prebootstrap_gate: None,
    });
    Ok(CoordinationGuard {
        state,
        access: CoordinationAccess::Exclusive,
        absent_coordination: false,
    })
}

#[cfg(test)]
pub(crate) fn acquire_exclusive_controlled(
    location: &CorpusLocation,
    domain: Domain,
    metadata: LeaseMetadata,
    protected_revalidation: impl FnOnce(&RootedFs) -> Result<()>,
    control: &mut ExclusiveAcquisitionControl,
) -> Result<CoordinationGuard> {
    acquire_exclusive_inner(
        location,
        domain,
        metadata,
        protected_revalidation,
        Some(control),
    )
}

pub(crate) fn acquire_shared_check(
    location: &CorpusLocation,
    domain: Domain,
) -> Result<CoordinationGuard> {
    MutationTarget::current().require_supported("acquire generation check guard")?;
    let rooted = RootedFs::open_corpus(location)?;
    let authority_key = corpus_authority_key(&rooted, domain);
    let transaction_parent = format!(".surgeist-generator/transactions/{}", domain.as_str());
    if !rooted
        .exists(COORDINATION_ROOT)
        .map_err(verification_from)?
    {
        let state = Arc::new(CoordinationState {
            canonical_corpus: location.corpus_root().to_path_buf(),
            corpus_identity: rooted.identity().clone(),
            domain,
            token: None,
            authority_key,
            transaction_parent,
            rooted,
            live: AtomicBool::new(true),
            operation_active: AtomicBool::new(false),
            _mutex: None,
            _prebootstrap_gate: None,
        });
        return Ok(CoordinationGuard {
            state,
            access: CoordinationAccess::Shared,
            absent_coordination: true,
        });
    }
    validate_coordination_tree(&rooted, domain, true)?;
    require_one_domain(&rooted, domain, true)?;
    if !rooted.exists(ACQUISITION_LOCK).map_err(verification_from)? {
        return Err(verification_error(
            "acquire generation check guard",
            "coordination exists without an immutable acquisition gate",
        ));
    }
    let gate = open_existing_lock(&rooted, ACQUISITION_LOCK, CoordinationAccess::Shared, true)?;
    let mutex_path = mutex_path(domain);
    if !rooted.exists(&mutex_path).map_err(verification_from)? {
        inspect_read_only_residue(&rooted, domain)?;
        let state = Arc::new(CoordinationState {
            canonical_corpus: location.corpus_root().to_path_buf(),
            corpus_identity: rooted.identity().clone(),
            domain,
            token: None,
            authority_key,
            transaction_parent,
            rooted,
            live: AtomicBool::new(true),
            operation_active: AtomicBool::new(false),
            _mutex: None,
            _prebootstrap_gate: Some(gate),
        });
        return Ok(CoordinationGuard {
            state,
            access: CoordinationAccess::Shared,
            absent_coordination: false,
        });
    }
    let mutex = match open_existing_lock(&rooted, &mutex_path, CoordinationAccess::Shared, true) {
        Ok(mutex) => mutex,
        Err(error) => {
            drop(gate);
            return Err(error);
        }
    };
    drop(gate);
    inspect_read_only_residue(&rooted, domain)?;
    let state = Arc::new(CoordinationState {
        canonical_corpus: location.corpus_root().to_path_buf(),
        corpus_identity: rooted.identity().clone(),
        domain,
        token: None,
        authority_key,
        transaction_parent,
        rooted,
        live: AtomicBool::new(true),
        operation_active: AtomicBool::new(false),
        _mutex: Some(mutex),
        _prebootstrap_gate: None,
    });
    Ok(CoordinationGuard {
        state,
        access: CoordinationAccess::Shared,
        absent_coordination: false,
    })
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BootstrapIntent {
    schema_version: u8,
    creator_pid: u32,
    token: String,
    final_path: String,
    header_digest: Sha256Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BootstrapStage {
    schema_version: u8,
    identity: HeldIdentity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct LostContended {
    schema_version: u8,
    final_path: String,
    final_identity: HeldIdentity,
    header_digest: Sha256Digest,
    stage_released: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BootstrapCleanupMember {
    name: String,
    identity: HeldIdentity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct BootstrapCleanup {
    schema_version: u8,
    journal_identity: HeldIdentity,
    members: Vec<BootstrapCleanupMember>,
}

#[cfg(test)]
struct BootstrapInstallControl<'a> {
    creator_pid: u32,
    before_final_rename: Option<&'a mut dyn FnMut() -> Result<()>>,
}

#[cfg(test)]
impl<'a> BootstrapInstallControl<'a> {
    fn new(
        creator_pid: u32,
        before_final_rename: Option<&'a mut dyn FnMut() -> Result<()>>,
    ) -> Self {
        Self {
            creator_pid,
            before_final_rename,
        }
    }
}

#[cfg(test)]
struct BootstrapRecoveryControl<'a> {
    claimant_pid: u32,
    claim_token: &'a str,
    liveness: &'a mut dyn FnMut(u32) -> Result<bool>,
}

#[cfg(test)]
impl<'a> BootstrapRecoveryControl<'a> {
    fn new(
        claimant_pid: u32,
        claim_token: &'a str,
        liveness: &'a mut dyn FnMut(u32) -> Result<bool>,
    ) -> Self {
        Self {
            claimant_pid,
            claim_token,
            liveness,
        }
    }
}

fn open_or_bootstrap_lock(
    rooted: &RootedFs,
    final_path: &str,
    label: &str,
    token: &str,
    access: CoordinationAccess,
) -> Result<File> {
    #[cfg(test)]
    let result = open_or_bootstrap_lock_inner(rooted, final_path, label, token, access, None);
    #[cfg(not(test))]
    let result = open_or_bootstrap_lock_inner(rooted, final_path, label, token, access);
    result
}

#[cfg(test)]
fn open_or_bootstrap_lock_controlled(
    rooted: &RootedFs,
    final_path: &str,
    label: &str,
    token: &str,
    access: CoordinationAccess,
    control: &mut BootstrapInstallControl<'_>,
) -> Result<File> {
    open_or_bootstrap_lock_inner(rooted, final_path, label, token, access, Some(control))
}

fn open_or_bootstrap_lock_inner(
    rooted: &RootedFs,
    final_path: &str,
    label: &str,
    token: &str,
    access: CoordinationAccess,
    #[cfg(test)] control: Option<&mut BootstrapInstallControl<'_>>,
) -> Result<File> {
    #[cfg(test)]
    let _observation_phase = rooted.begin_observation_phase(DurabilityPhase::BootstrapInstall);
    if rooted.exists(final_path)? {
        return open_existing_lock(rooted, final_path, access, false);
    }
    #[cfg(test)]
    let pid = control
        .as_ref()
        .map_or_else(std::process::id, |control| control.creator_pid);
    #[cfg(not(test))]
    let pid = std::process::id();
    let active_name = format!("active-{pid}-{token}");
    let active = format!("{BOOTSTRAP_LOCKS}/{active_name}");
    rooted.create_dir_exclusive(&active, PRIVATE_DIRECTORY_MODE)?;
    let intent = BootstrapIntent {
        schema_version: 1,
        creator_pid: pid,
        token: token.to_owned(),
        final_path: final_path.to_owned(),
        header_digest: Sha256Digest::from_bytes(LOCK_HEADER),
    };
    rooted.publish_file_exclusive(
        &active,
        "intent.json",
        &format!("intent-{token}.tmp"),
        &canonical_json(&intent, "serialize bootstrap intent")?,
        PRIVATE_FILE_MODE,
    )?;
    let stage_path = format!("{active}/lock.stage");
    let mut stage = rooted.create_file_handle_exclusive(&stage_path, b"", PRIVATE_FILE_MODE)?;
    let stage_identity = rooted.identity_of_handle(&stage)?;
    #[cfg(test)]
    rooted.observe_handle_identity(&stage_path);
    let stage_record = BootstrapStage {
        schema_version: 1,
        identity: stage_identity,
    };
    rooted.publish_file_exclusive(
        &active,
        "stage-created",
        &format!("stage-created-{token}.tmp"),
        &canonical_json(&stage_record, "serialize bootstrap stage identity")?,
        PRIVATE_FILE_MODE,
    )?;
    rooted
        .write_file_handle_all(&stage_path, &mut stage, LOCK_HEADER)
        .map_err(|source| {
            transaction_source("write immutable generation lock header", final_path, source)
        })?;
    rooted
        .flush_file_handle(&stage_path, &mut stage)
        .map_err(|source| {
            transaction_source("flush immutable generation lock header", final_path, source)
        })?;
    rooted
        .sync_file_handle(&stage_path, &stage)
        .map_err(|source| {
            transaction_source("sync immutable generation lock header", final_path, source)
        })?;
    rooted.validate_handle_at(&stage_path, &stage, PRIVATE_FILE_MODE)?;
    lock_file(&stage, access, final_path)?;
    #[cfg(test)]
    if let Some(control) = control
        && let Some(before_final_rename) = control.before_final_rename.as_deref_mut()
    {
        before_final_rename()?;
    }
    match rooted.rename_exclusive_bound(&stage_path, final_path, &stage_record.identity) {
        Ok(()) => {
            rooted.validate_handle_at(final_path, &stage, PRIVATE_FILE_MODE)?;
            cleanup_bootstrap_directory(rooted, &active, &active_name, Some("lock.stage"))?;
            Ok(stage)
        }
        Err(_rename_error) if rooted.exists(final_path)? => {
            rooted.drop_file_handle(&stage_path, stage);
            let final_file = match open_existing_lock(rooted, final_path, access, false) {
                Ok(file) => file,
                Err(error) if error.kind() == GeneratorErrorKind::LeaseActive => {
                    let final_handle =
                        rooted.open_file_handle(final_path, PRIVATE_FILE_MODE, false)?;
                    validate_lock_header(rooted, final_path, &final_handle, false)?;
                    let final_identity = rooted.identity_of_handle(&final_handle)?;
                    #[cfg(test)]
                    rooted.observe_handle_identity(final_path);
                    rooted.drop_file_handle(final_path, final_handle);
                    let marker = LostContended {
                        schema_version: 1,
                        final_path: final_path.to_owned(),
                        final_identity,
                        header_digest: Sha256Digest::from_bytes(LOCK_HEADER),
                        stage_released: true,
                    };
                    rooted.publish_file_exclusive(
                        &active,
                        "lost-contended",
                        &format!("lost-contended-{token}.tmp"),
                        &canonical_json(&marker, "serialize lost-contended marker")?,
                        PRIVATE_FILE_MODE,
                    )?;
                    cleanup_bootstrap_directory(rooted, &active, &active_name, None)?;
                    return Err(error);
                }
                Err(error) => return Err(error),
            };
            cleanup_bootstrap_directory(rooted, &active, &active_name, None)?;
            Ok(final_file)
        }
        Err(error) => Err(transaction_error(
            "publish immutable generation lock",
            format!("{label}: {error}"),
        )),
    }
}

fn open_existing_lock(
    rooted: &RootedFs,
    path: &str,
    access: CoordinationAccess,
    verification: bool,
) -> Result<File> {
    let file = rooted
        .open_file_handle(
            path,
            PRIVATE_FILE_MODE,
            access == CoordinationAccess::Exclusive,
        )
        .map_err(|error| {
            if verification {
                verification_from(error)
            } else {
                error
            }
        })?;
    validate_lock_header(rooted, path, &file, verification)?;
    lock_file(&file, access, path)?;
    rooted
        .validate_handle_at(path, &file, PRIVATE_FILE_MODE)
        .map_err(|error| {
            if verification {
                verification_from(error)
            } else {
                error
            }
        })?;
    Ok(file)
}

fn validate_lock_header(
    rooted: &RootedFs,
    path: &str,
    file: &File,
    verification: bool,
) -> Result<()> {
    rooted
        .validate_handle_at(path, file, PRIVATE_FILE_MODE)
        .map_err(|error| {
            if verification {
                verification_from(error)
            } else {
                error
            }
        })?;
    let mut copy = file
        .try_clone()
        .map_err(|source| transaction_source("clone immutable generation lock", path, source))?;
    copy.seek(SeekFrom::Start(0))
        .map_err(|source| transaction_source("seek immutable generation lock", path, source))?;
    let mut bytes = Vec::new();
    copy.read_to_end(&mut bytes)
        .map_err(|source| transaction_source("read immutable generation lock", path, source))?;
    if bytes != LOCK_HEADER {
        let error = transaction_error(
            "validate immutable generation lock",
            format!("unknown or partial lock header: {path}"),
        );
        return Err(if verification {
            verification_from(error)
        } else {
            error
        });
    }
    rooted
        .validate_handle_at(path, file, PRIVATE_FILE_MODE)
        .map_err(|error| {
            if verification {
                verification_from(error)
            } else {
                error
            }
        })?;
    Ok(())
}

fn lock_file(file: &File, access: CoordinationAccess, context: &str) -> Result<()> {
    use std::fs::TryLockError;

    let result = match access {
        CoordinationAccess::Shared => file.try_lock_shared(),
        CoordinationAccess::Exclusive => file.try_lock(),
    };
    match result {
        Ok(()) => Ok(()),
        Err(TryLockError::WouldBlock) => Err(GeneratorError::new(
            GeneratorErrorKind::LeaseActive,
            "acquire generation coordination lock",
            context,
        )),
        Err(TryLockError::Error(source)) => Err(transaction_source(
            "acquire generation coordination lock",
            context,
            source,
        )),
    }
}

fn recover_bootstrap(rooted: &RootedFs) -> Result<()> {
    #[cfg(test)]
    let result = recover_bootstrap_inner(rooted, None);
    #[cfg(not(test))]
    let result = recover_bootstrap_inner(rooted);
    result
}

#[cfg(test)]
fn recover_bootstrap_controlled(
    rooted: &RootedFs,
    control: &mut BootstrapRecoveryControl<'_>,
) -> Result<()> {
    recover_bootstrap_inner(rooted, Some(control))
}

fn recover_bootstrap_inner(
    rooted: &RootedFs,
    #[cfg(test)] mut control: Option<&mut BootstrapRecoveryControl<'_>>,
) -> Result<()> {
    #[cfg(test)]
    let _observation_phase = rooted.begin_observation_phase(DurabilityPhase::BootstrapRecovery);
    for _ in 0..16 {
        let names = rooted.list_dir(BOOTSTRAP_LOCKS)?;
        if names.is_empty() {
            return Ok(());
        }
        let mut raced = false;
        for name in names {
            let parsed = parse_bootstrap_name(&name)?;
            let path = format!("{BOOTSTRAP_LOCKS}/{name}");
            let relinquished = validate_bootstrap_state(rooted, &path, &parsed)?;
            #[cfg(test)]
            let owner_is_live = if let Some(control) = control.as_deref_mut() {
                (control.liveness)(parsed.owner_pid)?
            } else {
                process_is_live(parsed.owner_pid)?
            };
            #[cfg(not(test))]
            let owner_is_live = process_is_live(parsed.owner_pid)?;
            if owner_is_live && !relinquished {
                return Err(GeneratorError::new(
                    GeneratorErrorKind::LeaseActive,
                    "recover generation lock bootstrap",
                    "a live bootstrap owner is active",
                ));
            }
            #[cfg(test)]
            let (claim_token, claimant_pid) = if let Some(control) = control.as_deref() {
                (control.claim_token.to_owned(), control.claimant_pid)
            } else {
                (new_token()?, std::process::id())
            };
            #[cfg(not(test))]
            let (claim_token, claimant_pid) = (new_token()?, std::process::id());
            let claim_name = format!(
                "recovering-{}-{}-by-{claimant_pid}-{claim_token}",
                parsed.origin_pid, parsed.origin_token
            );
            let claim_path = format!("{BOOTSTRAP_LOCKS}/{claim_name}");
            let journal_identity = rooted.identity_at(&path)?.ok_or_else(|| {
                transaction_error("claim bootstrap recovery", "bootstrap journal disappeared")
            })?;
            if let Err(error) = rooted.rename_exclusive_bound(&path, &claim_path, &journal_identity)
            {
                if !rooted.exists(&path)? {
                    raced = true;
                    continue;
                }
                return Err(error);
            }
            let claimed = parse_bootstrap_name(&claim_name)?;
            validate_bootstrap_state(rooted, &claim_path, &claimed)?;
            recover_bootstrap_stage(rooted, &claim_path, &claimed)?;
            cleanup_bootstrap_directory(rooted, &claim_path, &claim_name, None)?;
        }
        if !raced {
            return Ok(());
        }
    }
    Err(transaction_error(
        "recover generation lock bootstrap",
        "bootstrap recovery exceeded the 16-claim retry bound",
    ))
}

struct ParsedBootstrapName<'a> {
    origin_pid: u32,
    origin_token: &'a str,
    owner_pid: u32,
}

fn validate_bootstrap_state(
    rooted: &RootedFs,
    path: &str,
    parsed: &ParsedBootstrapName<'_>,
) -> Result<bool> {
    let identity = rooted.identity_at(path)?.ok_or_else(|| {
        transaction_error(
            "validate bootstrap journal",
            "bootstrap journal disappeared",
        )
    })?;
    if identity.kind() != NodeKind::Directory || identity.mode() != PRIVATE_DIRECTORY_MODE {
        return Err(transaction_error(
            "validate bootstrap journal",
            "bootstrap journal has the wrong type or mode",
        ));
    }
    let names = rooted.list_dir(path)?;
    let allowed = [
        "intent.json",
        "lock.stage",
        "stage-created",
        "lost-contended",
        "cleanup-started",
    ];
    for name in &names {
        if !allowed.contains(&name.as_str())
            && !(name.ends_with(".tmp")
                && (name.starts_with("intent-")
                    || name.starts_with("stage-created-")
                    || name.starts_with("lost-contended-")
                    || name.starts_with("cleanup-started-")))
        {
            return Err(transaction_error(
                "validate bootstrap journal",
                format!("unknown bootstrap member: {name}"),
            ));
        }
    }
    let intent = if rooted.exists(&format!("{path}/intent.json"))? {
        let intent: BootstrapIntent = serde_json::from_slice(
            &rooted.read_file(&format!("{path}/intent.json"), PRIVATE_FILE_MODE)?,
        )
        .map_err(|error| {
            transaction_error(
                "validate bootstrap intent",
                format!("invalid bootstrap intent: {error}"),
            )
        })?;
        let final_allowed = intent.final_path == ACQUISITION_LOCK
            || intent.final_path == mutex_path(Domain::Layout)
            || intent.final_path == mutex_path(Domain::Css);
        if intent.schema_version != 1
            || intent.creator_pid != parsed.origin_pid
            || intent.token != parsed.origin_token
            || !final_allowed
            || intent.header_digest != Sha256Digest::from_bytes(LOCK_HEADER)
        {
            return Err(transaction_error(
                "validate bootstrap intent",
                "bootstrap intent schema, name binding, path, or header differs",
            ));
        }
        Some(intent)
    } else {
        None
    };
    let stage_record = if rooted.exists(&format!("{path}/stage-created"))? {
        let record: BootstrapStage = serde_json::from_slice(
            &rooted.read_file(&format!("{path}/stage-created"), PRIVATE_FILE_MODE)?,
        )
        .map_err(|error| {
            transaction_error(
                "validate bootstrap stage record",
                format!("invalid stage record: {error}"),
            )
        })?;
        if record.schema_version != 1
            || record.identity.kind() != NodeKind::Regular
            || record.identity.mode() != PRIVATE_FILE_MODE
            || record.identity.link_count() != Some(1)
            || record.identity.owner() != rooted.identity().owner()
            || record.identity.device() != rooted.identity().device()
            || record.identity.fsid() != rooted.identity().fsid()
        {
            return Err(transaction_error(
                "validate bootstrap stage record",
                "bootstrap stage identity policy differs",
            ));
        }
        Some(record)
    } else {
        None
    };
    let stage_path = format!("{path}/lock.stage");
    if let Some(actual) = rooted.identity_at(&stage_path)? {
        if let Some(record) = &stage_record {
            if !record.identity.matches_recovery(&actual) {
                return Err(transaction_error(
                    "validate bootstrap stage",
                    "registered bootstrap stage identity changed",
                ));
            }
            let bytes = rooted.read_file(&stage_path, PRIVATE_FILE_MODE)?;
            if !LOCK_HEADER.starts_with(&bytes) {
                return Err(transaction_error(
                    "validate bootstrap stage",
                    "registered stage bytes are not a lock-header prefix",
                ));
            }
        } else if !rooted.read_file(&stage_path, PRIVATE_FILE_MODE)?.is_empty() {
            return Err(transaction_error(
                "validate bootstrap stage",
                "nonempty bootstrap stage exists without registration",
            ));
        }
    } else if let (Some(stage_record), Some(intent)) = (&stage_record, &intent) {
        let final_identity = rooted.identity_at(&intent.final_path)?;
        if final_identity
            .as_ref()
            .is_none_or(|final_identity| !stage_record.identity.same_object(final_identity))
            && !rooted.exists(&format!("{path}/cleanup-started"))?
        {
            return Err(transaction_error(
                "validate bootstrap stage",
                "registered stage disappeared from both stage and final names",
            ));
        }
        if final_identity.is_some() {
            validate_lock_file_bytes(rooted, &intent.final_path)?;
        }
    }
    let cleanup_started = rooted.exists(&format!("{path}/cleanup-started"))?;
    if cleanup_started {
        let receipt: BootstrapCleanup = serde_json::from_slice(
            &rooted.read_file(&format!("{path}/cleanup-started"), PRIVATE_FILE_MODE)?,
        )
        .map_err(|error| {
            transaction_error(
                "validate bootstrap cleanup receipt",
                format!("invalid cleanup receipt: {error}"),
            )
        })?;
        if receipt.schema_version != 1 || !receipt.journal_identity.matches_recovery(&identity) {
            return Err(transaction_error(
                "validate bootstrap cleanup receipt",
                "cleanup receipt journal identity differs",
            ));
        }
    }
    if !rooted.exists(&format!("{path}/lost-contended"))? {
        return Ok(cleanup_started);
    }
    let intent = intent.ok_or_else(|| {
        transaction_error(
            "validate lost-contended marker",
            "lost marker exists without bootstrap intent",
        )
    })?;
    let lost: LostContended = serde_json::from_slice(
        &rooted.read_file(&format!("{path}/lost-contended"), PRIVATE_FILE_MODE)?,
    )
    .map_err(|error| {
        transaction_error(
            "validate lost-contended marker",
            format!("invalid lost marker: {error}"),
        )
    })?;
    let final_identity = rooted.identity_at(&intent.final_path)?.ok_or_else(|| {
        transaction_error(
            "validate lost-contended marker",
            "bound final lock is absent",
        )
    })?;
    if lost.schema_version != 1
        || lost.final_path != intent.final_path
        || lost.final_identity != final_identity
        || lost.header_digest != Sha256Digest::from_bytes(LOCK_HEADER)
        || !lost.stage_released
    {
        return Err(transaction_error(
            "validate lost-contended marker",
            "lost marker fields or bound final identity differ",
        ));
    }
    validate_lock_file_bytes(rooted, &intent.final_path)?;
    if rooted.exists(&stage_path)? {
        let stage = rooted.open_file_handle(&stage_path, PRIVATE_FILE_MODE, false)?;
        lock_file(
            &stage,
            CoordinationAccess::Exclusive,
            "released bootstrap stage",
        )?;
    }
    Ok(true)
}

fn recover_bootstrap_stage(
    rooted: &RootedFs,
    path: &str,
    parsed: &ParsedBootstrapName<'_>,
) -> Result<()> {
    let intent_path = format!("{path}/intent.json");
    let stage_record_path = format!("{path}/stage-created");
    let stage_path = format!("{path}/lock.stage");
    if !rooted.exists(&intent_path)? || !rooted.exists(&stage_record_path)? {
        return Ok(());
    }
    let intent: BootstrapIntent =
        serde_json::from_slice(&rooted.read_file(&intent_path, PRIVATE_FILE_MODE)?)
            .map_err(|error| transaction_error("recover bootstrap stage", error.to_string()))?;
    if intent.creator_pid != parsed.origin_pid || intent.token != parsed.origin_token {
        return Err(transaction_error(
            "recover bootstrap stage",
            "claimed bootstrap intent differs from its name",
        ));
    }
    if !rooted.exists(&stage_path)? {
        if rooted.exists(&intent.final_path)? {
            validate_lock_file_bytes(rooted, &intent.final_path)?;
        }
        return Ok(());
    }
    let bytes = rooted.read_file(&stage_path, PRIVATE_FILE_MODE)?;
    if bytes != LOCK_HEADER {
        return Ok(());
    }
    let stage = rooted.open_file_handle(&stage_path, PRIVATE_FILE_MODE, false)?;
    lock_file(
        &stage,
        CoordinationAccess::Exclusive,
        "recover bootstrap stage",
    )?;
    if rooted.exists(&intent.final_path)? {
        validate_lock_file_bytes(rooted, &intent.final_path)?;
        return Ok(());
    }
    let stage_record: BootstrapStage =
        serde_json::from_slice(&rooted.read_file(&stage_record_path, PRIVATE_FILE_MODE)?)
            .map_err(|error| transaction_error("recover bootstrap stage", error.to_string()))?;
    rooted.rename_exclusive_bound(&stage_path, &intent.final_path, &stage_record.identity)?;
    rooted.validate_handle_at(&intent.final_path, &stage, PRIVATE_FILE_MODE)?;
    Ok(())
}

fn parse_bootstrap_name(name: &str) -> Result<ParsedBootstrapName<'_>> {
    if let Some(rest) = name.strip_prefix("active-") {
        let Some((pid, token)) = rest.split_once('-') else {
            return Err(transaction_error("parse bootstrap name", name));
        };
        validate_token(token)?;
        let pid = parse_pid(pid)?;
        return Ok(ParsedBootstrapName {
            origin_pid: pid,
            origin_token: token,
            owner_pid: pid,
        });
    }
    if let Some(rest) = name.strip_prefix("recovering-") {
        let Some((origin, claimant)) = rest.split_once("-by-") else {
            return Err(transaction_error("parse bootstrap name", name));
        };
        let Some((origin_pid, origin_token)) = origin.split_once('-') else {
            return Err(transaction_error("parse bootstrap name", name));
        };
        let Some((claimant_pid, claimant_token)) = claimant.split_once('-') else {
            return Err(transaction_error("parse bootstrap name", name));
        };
        validate_token(origin_token)?;
        validate_token(claimant_token)?;
        return Ok(ParsedBootstrapName {
            origin_pid: parse_pid(origin_pid)?,
            origin_token,
            owner_pid: parse_pid(claimant_pid)?,
        });
    }
    Err(transaction_error(
        "parse bootstrap name",
        format!("unknown bootstrap entry: {name}"),
    ))
}

fn process_is_live(pid: u32) -> Result<bool> {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        let raw = i32::try_from(pid)
            .map_err(|_| transaction_error("probe bootstrap owner", "PID exceeds i32"))?;
        let pid = rustix::process::Pid::from_raw(raw)
            .ok_or_else(|| transaction_error("probe bootstrap owner", "PID is zero"))?;
        match rustix::process::test_kill_process(pid) {
            Ok(()) | Err(rustix::io::Errno::PERM) => Ok(true),
            Err(rustix::io::Errno::SRCH) => Ok(false),
            Err(source) => Err(transaction_error(
                "probe bootstrap owner",
                format!("PID liveness is inconclusive: {source}"),
            )),
        }
    }
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        let _ = pid;
        MutationTarget::Unsupported.require_supported("probe bootstrap owner")?;
        unreachable!("unsupported mutation target returned success")
    }
}

fn cleanup_bootstrap_directory(
    rooted: &RootedFs,
    path: &str,
    _name: &str,
    _moved_member: Option<&str>,
) -> Result<()> {
    #[cfg(test)]
    let _observation_phase = rooted.begin_observation_phase(DurabilityPhase::BootstrapCleanup);
    let identity = rooted.identity_at(path)?.ok_or_else(|| {
        transaction_error(
            "clean bootstrap directory",
            "bootstrap directory disappeared",
        )
    })?;
    if identity.kind() != NodeKind::Directory || identity.mode() != PRIVATE_DIRECTORY_MODE {
        return Err(transaction_error(
            "clean bootstrap directory",
            "bootstrap directory has the wrong type or mode",
        ));
    }
    let allowed = [
        "intent.json",
        "lock.stage",
        "stage-created",
        "lost-contended",
        "cleanup-started",
    ];
    let names = rooted.list_dir(path)?;
    for name in &names {
        if !allowed.contains(&name.as_str())
            && !(name.ends_with(".tmp")
                && (name.starts_with("intent-")
                    || name.starts_with("stage-created-")
                    || name.starts_with("lost-contended-")
                    || name.starts_with("cleanup-started-")))
        {
            return Err(transaction_error(
                "clean bootstrap directory",
                format!("unknown bootstrap member: {name}"),
            ));
        }
    }
    let cleanup_path = format!("{path}/cleanup-started");
    let receipt = if rooted.exists(&cleanup_path)? {
        let receipt: BootstrapCleanup = serde_json::from_slice(
            &rooted.read_file(&cleanup_path, PRIVATE_FILE_MODE)?,
        )
        .map_err(|error| {
            transaction_error(
                "parse bootstrap cleanup receipt",
                format!("invalid cleanup receipt: {error}"),
            )
        })?;
        receipt
    } else {
        let mut members = Vec::new();
        for name in &names {
            if name == "cleanup-started" || name == "cleanup-started-receipt.tmp" {
                continue;
            }
            let member_path = format!("{path}/{name}");
            let member_identity = rooted.identity_at(&member_path)?.ok_or_else(|| {
                transaction_error("clean bootstrap directory", "member disappeared")
            })?;
            if member_identity.kind() != NodeKind::Regular
                || member_identity.mode() != PRIVATE_FILE_MODE
                || member_identity.link_count() != Some(1)
            {
                return Err(transaction_error(
                    "clean bootstrap directory",
                    format!("bootstrap member has the wrong policy: {name}"),
                ));
            }
            members.push(BootstrapCleanupMember {
                name: name.clone(),
                identity: member_identity,
            });
        }
        members.sort_by(|left, right| left.name.cmp(&right.name));
        let receipt = BootstrapCleanup {
            schema_version: 1,
            journal_identity: identity.clone(),
            members,
        };
        rooted.publish_file_exclusive(
            path,
            "cleanup-started",
            "cleanup-started-receipt.tmp",
            &canonical_json(&receipt, "serialize bootstrap cleanup receipt")?,
            PRIVATE_FILE_MODE,
        )?;
        receipt
    };
    if receipt.schema_version != 1 || !receipt.journal_identity.matches_recovery(&identity) {
        return Err(transaction_error(
            "validate bootstrap cleanup receipt",
            "cleanup receipt journal identity differs",
        ));
    }
    let mut receipt_names = BTreeSet::new();
    for member in &receipt.members {
        if !receipt_names.insert(member.name.as_str())
            || !allowed.contains(&member.name.as_str())
                && !(member.name.ends_with(".tmp")
                    && (member.name.starts_with("intent-")
                        || member.name.starts_with("stage-created-")
                        || member.name.starts_with("lost-contended-")
                        || member.name.starts_with("cleanup-started-")))
        {
            return Err(transaction_error(
                "validate bootstrap cleanup receipt",
                format!("invalid receipt member: {}", member.name),
            ));
        }
    }
    for name in rooted.list_dir(path)? {
        if name != "cleanup-started" && !receipt_names.contains(name.as_str()) {
            return Err(transaction_error(
                "validate bootstrap cleanup receipt",
                format!("unknown member after receipt: {name}"),
            ));
        }
    }
    for member in &receipt.members {
        let member_path = format!("{path}/{}", member.name);
        if let Some(actual) = rooted.identity_at(&member_path)? {
            if !member.identity.matches_recovery(&actual) {
                return Err(transaction_error(
                    "clean bootstrap directory",
                    format!("receipt member identity changed: {}", member.name),
                ));
            }
            rooted.remove_file_exact(&member_path, &member.identity)?;
        }
    }
    if let Some(cleanup_identity) = rooted.identity_at(&cleanup_path)? {
        rooted.remove_file_exact(&cleanup_path, &cleanup_identity)?;
    }
    rooted.remove_dir_exact(path, &identity)
}

fn validate_coordination_tree(rooted: &RootedFs, domain: Domain, verification: bool) -> Result<()> {
    validate_coordination_tree_inner(rooted, domain).map_err(|error| {
        if verification {
            verification_from(error)
        } else {
            error
        }
    })
}

fn validate_coordination_tree_inner(rooted: &RootedFs, domain: Domain) -> Result<()> {
    validate_private_directory(rooted, COORDINATION_ROOT)?;
    let allowed = [
        "acquisition.lock",
        "bootstrap",
        "leases",
        "transactions",
        "probes",
    ];
    for name in rooted.list_dir(COORDINATION_ROOT)? {
        if !allowed.contains(&name.as_str()) {
            return Err(transaction_error(
                "validate generation coordination tree",
                format!("unknown coordination entry: {name}"),
            ));
        }
    }
    if rooted.exists(".surgeist-generator/acquisition.lock")? {
        validate_lock_file_bytes(rooted, ".surgeist-generator/acquisition.lock")?;
    }
    if rooted.exists(".surgeist-generator/bootstrap")? {
        validate_private_directory(rooted, ".surgeist-generator/bootstrap")?;
        validate_exact_children(rooted, ".surgeist-generator/bootstrap", &["locks"])?;
    }
    if rooted.exists(BOOTSTRAP_LOCKS)? {
        validate_private_directory(rooted, BOOTSTRAP_LOCKS)?;
        for name in rooted.list_dir(BOOTSTRAP_LOCKS)? {
            if !name.starts_with("active-") && !name.starts_with("recovering-") {
                return Err(transaction_error(
                    "validate generation coordination tree",
                    format!("unknown bootstrap journal: {name}"),
                ));
            }
            validate_private_directory(rooted, &format!("{BOOTSTRAP_LOCKS}/{name}"))?;
        }
    }
    require_one_domain_inner(rooted, domain)?;
    for parent in [
        ".surgeist-generator/leases",
        ".surgeist-generator/transactions",
        ".surgeist-generator/probes",
    ] {
        if rooted.exists(parent)? {
            validate_private_directory(rooted, parent)?;
        }
    }
    let lease = format!(".surgeist-generator/leases/{}", domain.as_str());
    if rooted.exists(&lease)? {
        validate_private_directory(rooted, &lease)?;
        validate_exact_children(
            rooted,
            &lease,
            &["mutex.lock", OWNER_RECORD, OWNER_TRANSACTIONS],
        )?;
        let mutex = format!("{lease}/mutex.lock");
        if rooted.exists(&mutex)? {
            validate_lock_file_bytes(rooted, &mutex)?;
        }
        let owner = format!("{lease}/{OWNER_RECORD}");
        if rooted.exists(&owner)? {
            validate_private_file(rooted, &owner)?;
            let bytes = rooted.read_file(&owner, PRIVATE_FILE_MODE)?;
            validate_owner_record_bytes(
                rooted,
                &bytes,
                "validate generation coordination tree",
                "visible owner record",
            )?;
        }
        let owner_transactions = format!("{lease}/{OWNER_TRANSACTIONS}");
        if rooted.exists(&owner_transactions)? {
            validate_private_directory(rooted, &owner_transactions)?;
            for name in rooted.list_dir(&owner_transactions)? {
                if !name.starts_with("active-") {
                    return Err(transaction_error(
                        "validate generation coordination tree",
                        format!("unknown owner transaction: {name}"),
                    ));
                }
                validate_private_directory(rooted, &format!("{owner_transactions}/{name}"))?;
            }
        }
    }
    for parent in [
        format!(".surgeist-generator/transactions/{}", domain.as_str()),
        format!(".surgeist-generator/probes/{}", domain.as_str()),
    ] {
        if rooted.exists(&parent)? {
            validate_private_directory(rooted, &parent)?;
            for name in rooted.list_dir(&parent)? {
                let valid = if parent.contains("/transactions/") {
                    name.starts_with("active-") || name.starts_with("completed-")
                } else {
                    name.starts_with("active-")
                };
                if !valid {
                    return Err(transaction_error(
                        "validate generation coordination tree",
                        format!("unknown durable journal: {parent}/{name}"),
                    ));
                }
                validate_private_directory(rooted, &format!("{parent}/{name}"))?;
            }
        }
    }
    Ok(())
}

fn require_one_domain(rooted: &RootedFs, domain: Domain, verification: bool) -> Result<()> {
    require_one_domain_inner(rooted, domain).map_err(|error| {
        if verification {
            verification_from(error)
        } else {
            error
        }
    })
}

fn require_one_domain_inner(rooted: &RootedFs, domain: Domain) -> Result<()> {
    for parent in [
        ".surgeist-generator/leases",
        ".surgeist-generator/transactions",
        ".surgeist-generator/probes",
    ] {
        if !rooted.exists(parent)? {
            continue;
        }
        for name in rooted.list_dir(parent)? {
            if name != domain.as_str() {
                return Err(transaction_error(
                    "validate one-domain corpus coordination",
                    format!(
                        "persistent {} state conflicts with requested {} domain",
                        name,
                        domain.as_str()
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn validate_exact_children(rooted: &RootedFs, parent: &str, allowed: &[&str]) -> Result<()> {
    for name in rooted.list_dir(parent)? {
        if !allowed.contains(&name.as_str()) {
            return Err(transaction_error(
                "validate generation coordination tree",
                format!("unknown entry: {parent}/{name}"),
            ));
        }
    }
    Ok(())
}

fn validate_private_directory(rooted: &RootedFs, path: &str) -> Result<()> {
    let identity = rooted.identity_at(path)?.ok_or_else(|| {
        transaction_error("validate private coordination directory", path.to_owned())
    })?;
    if identity.kind() != NodeKind::Directory
        || identity.mode() != PRIVATE_DIRECTORY_MODE
        || identity.owner() != rooted.identity().owner()
        || identity.device() != rooted.identity().device()
        || identity.fsid() != rooted.identity().fsid()
    {
        return Err(transaction_error(
            "validate private coordination directory",
            format!("wrong type, mode, owner, or mount: {path}"),
        ));
    }
    Ok(())
}

fn validate_private_file(rooted: &RootedFs, path: &str) -> Result<()> {
    let identity = rooted
        .identity_at(path)?
        .ok_or_else(|| transaction_error("validate private coordination file", path.to_owned()))?;
    if identity.kind() != NodeKind::Regular
        || identity.mode() != PRIVATE_FILE_MODE
        || identity.owner() != rooted.identity().owner()
        || identity.link_count() != Some(1)
        || identity.device() != rooted.identity().device()
        || identity.fsid() != rooted.identity().fsid()
    {
        return Err(transaction_error(
            "validate private coordination file",
            format!("wrong type, mode, owner, link count, or mount: {path}"),
        ));
    }
    Ok(())
}

fn validate_lock_file_bytes(rooted: &RootedFs, path: &str) -> Result<()> {
    validate_private_file(rooted, path)?;
    if rooted.read_file(path, PRIVATE_FILE_MODE)? != LOCK_HEADER {
        return Err(transaction_error(
            "validate immutable generation lock",
            format!("unknown or partial lock header: {path}"),
        ));
    }
    Ok(())
}

fn inspect_read_only_residue(rooted: &RootedFs, domain: Domain) -> Result<()> {
    require_one_domain(rooted, domain, true)?;
    for parent in [
        format!(".surgeist-generator/transactions/{}", domain.as_str()),
        format!(".surgeist-generator/probes/{}", domain.as_str()),
        format!(
            ".surgeist-generator/leases/{}/{}",
            domain.as_str(),
            OWNER_TRANSACTIONS
        ),
        BOOTSTRAP_LOCKS.to_owned(),
    ] {
        if rooted.exists(&parent).map_err(verification_from)?
            && !rooted
                .list_dir(&parent)
                .map_err(verification_from)?
                .is_empty()
        {
            return Err(verification_error(
                "inspect generation coordination",
                format!("unresolved durable state: {parent}"),
            ));
        }
    }
    let owner = owner_path(domain);
    if rooted.exists(&owner).map_err(verification_from)? {
        let bytes = rooted
            .read_file(&owner, PRIVATE_FILE_MODE)
            .map_err(verification_from)?;
        validate_owner_record_bytes(
            rooted,
            &bytes,
            "inspect historical generation owner",
            "visible owner record",
        )
        .map_err(verification_from)?;
    }
    Ok(())
}

fn validate_owner_record(rooted: &RootedFs, record: &OwnerRecord) -> Result<()> {
    let scope_valid = record.scope == "full"
        || record
            .scope
            .strip_prefix("filtered:")
            .is_some_and(|path| crate::RelativePath::new(path).is_ok());
    let owner_root = Path::new(&record.owner_root);
    let corpus_root = Path::new(&record.corpus_root);
    if record.schema_version != 1
        || record.pid == 0
        || record.unix_start_time == 0
        || !super::validate_identifier(&record.generator)
        || !super::validate_identifier(&record.command)
        || !scope_valid
        || !owner_root.is_absolute()
        || corpus_root != rooted.canonical_root()
        || !corpus_root.starts_with(owner_root)
    {
        return Err(transaction_error(
            "validate historical generation owner",
            "owner record fields are not canonical",
        ));
    }
    Ok(())
}

fn validate_owner_record_bytes(
    rooted: &RootedFs,
    bytes: &[u8],
    operation: &str,
    label: &str,
) -> Result<()> {
    let record: OwnerRecord = serde_json::from_slice(bytes)
        .map_err(|error| transaction_error(operation, format!("{label} is invalid: {error}")))?;
    validate_owner_record(rooted, &record)?;
    validate_canonical_owner_json(bytes, &record, operation, label)
}

fn run_rename_probe(rooted: &RootedFs, domain: Domain, token: &str) -> Result<()> {
    #[cfg(test)]
    let result = run_rename_probe_inner(rooted, domain, token, None);
    #[cfg(not(test))]
    let result = run_rename_probe_inner(rooted, domain, token);
    result
}

fn run_rename_probe_inner(
    rooted: &RootedFs,
    domain: Domain,
    token: &str,
    #[cfg(test)] mut control: Option<&mut ProbeInstallControl>,
) -> Result<()> {
    #[cfg(test)]
    let _observation_phase = rooted.begin_observation_phase(DurabilityPhase::ProbeInstall);
    let parent = format!(".surgeist-generator/probes/{}", domain.as_str());
    let active = format!("{parent}/active-{token}");
    let active_identity = rooted.create_dir_exclusive(&active, PRIVATE_DIRECTORY_MODE)?;
    let intent = ProbeIntent {
        schema_version: 1,
        domain,
        token: token.to_owned(),
        journal_path: active.clone(),
        journal_identity: active_identity.clone(),
    };
    let intent_bytes = canonical_json(&intent, "serialize rename probe intent")?;
    rooted.publish_file_exclusive(
        &active,
        "intent.json",
        &format!("intent-{token}.tmp"),
        &intent_bytes,
        PRIVATE_FILE_MODE,
    )?;
    #[cfg(test)]
    let probe_result =
        probe_rename_flags_journaled(rooted, &intent, &intent_bytes, control.as_deref_mut());
    #[cfg(not(test))]
    let probe_result = probe_rename_flags_journaled(rooted, &intent, &intent_bytes);

    let probe_result = probe_result.map_err(|error| {
        if error.kind() == GeneratorErrorKind::UnsupportedPlatform {
            GeneratorError::with_source(
                GeneratorErrorKind::UnsupportedPlatform,
                "probe rooted rename capability",
                error.to_string(),
                error,
            )
        } else {
            error
        }
    });
    match probe_result {
        Ok(()) => cleanup_probe_install_journal(
            rooted,
            &intent,
            &intent_bytes,
            #[cfg(test)]
            control,
        )
        .map_err(|cleanup| {
            probe_artifact_error(
                "clean successful rename capability probe",
                "the capability probe completed but journal cleanup failed",
                cleanup,
            )
        }),
        Err(capability) if capability.kind() == GeneratorErrorKind::UnsupportedPlatform => {
            match cleanup_probe_install_journal(
                rooted,
                &intent,
                &intent_bytes,
                #[cfg(test)]
                control,
            ) {
                Ok(()) => Err(capability),
                Err(cleanup) => Err(probe_cleanup_failure(capability, cleanup)),
            }
        }
        Err(error) => Err(if error.kind() == GeneratorErrorKind::ArtifactTransaction {
            error
        } else {
            probe_artifact_error(
                "run rename capability probe",
                "rename capability probing left resumable evidence",
                error,
            )
        }),
    }
}

#[cfg(test)]
fn run_rename_probe_controlled(
    rooted: &RootedFs,
    domain: Domain,
    token: &str,
    control: &mut ProbeInstallControl,
) -> Result<()> {
    run_rename_probe_inner(rooted, domain, token, Some(control))
}

fn recover_probe_journals(rooted: &RootedFs, domain: Domain) -> Result<()> {
    #[cfg(test)]
    let result = recover_probe_journals_inner(rooted, domain, None);
    #[cfg(not(test))]
    let result = recover_probe_journals_inner(rooted, domain);
    result.map_err(normalize_probe_recovery_error)
}

fn recover_probe_journals_inner(
    rooted: &RootedFs,
    domain: Domain,
    #[cfg(test)] mut control: Option<&mut ProbeRecoveryControl<'_>>,
) -> Result<()> {
    #[cfg(test)]
    let _observation_phase = rooted.begin_observation_phase(DurabilityPhase::ProbeRecovery);
    let parent = format!(".surgeist-generator/probes/{}", domain.as_str());
    for name in rooted.list_dir(&parent)? {
        let Some(token) = name.strip_prefix("active-") else {
            return Err(transaction_error(
                "recover rename capability probe",
                format!("unknown probe journal: {name}"),
            ));
        };
        validate_token(token)?;
        let active = format!("{parent}/{name}");
        let active_identity = rooted.identity_at(&active)?.ok_or_else(|| {
            transaction_error(
                "recover rename capability probe",
                "probe journal disappeared",
            )
        })?;
        let mut plan =
            ProbeRecoveryPlan::capture(rooted, domain, token, &active, &active_identity)?;
        while let Some(member) = plan.members.first().cloned() {
            #[cfg(test)]
            if let Some(control) = control.as_deref_mut() {
                (control.before_mutation)(&member.name)?;
            }
            plan.revalidate(rooted, &active)?;
            let member_path = format!("{active}/{}", member.name);
            match member.evidence {
                ProbeRecoveryMemberEvidence::File(_) => {
                    rooted.remove_file_exact(&member_path, &member.identity)?;
                }
                ProbeRecoveryMemberEvidence::Directory(_) => {
                    rooted.remove_dir_exact(&member_path, &member.identity)?;
                }
            }
            plan.members.remove(0);
        }
        #[cfg(test)]
        if let Some(control) = control.as_deref_mut() {
            (control.before_mutation)("journal-directory")?;
        }
        plan.revalidate(rooted, &active)?;
        rooted.remove_dir_exact(&active, &active_identity)?;
    }
    Ok(())
}

#[cfg(test)]
fn recover_probe_journals_controlled(
    rooted: &RootedFs,
    domain: Domain,
    control: &mut ProbeRecoveryControl<'_>,
) -> Result<()> {
    recover_probe_journals_inner(rooted, domain, Some(control))
        .map_err(normalize_probe_recovery_error)
}

fn normalize_probe_recovery_error(error: GeneratorError) -> GeneratorError {
    if error.kind() == GeneratorErrorKind::ArtifactTransaction {
        error
    } else {
        probe_artifact_error(
            "recover rename capability probe",
            "probe recovery stopped with retained evidence",
            error,
        )
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ProbeIntent {
    schema_version: u8,
    domain: Domain,
    token: String,
    journal_path: String,
    journal_identity: HeldIdentity,
}

#[derive(Clone, Debug)]
enum ProbeRecoveryMemberEvidence {
    File(Vec<u8>),
    Directory(Vec<String>),
}

#[derive(Clone, Debug)]
struct ProbeRecoveryMember {
    name: String,
    identity: HeldIdentity,
    evidence: ProbeRecoveryMemberEvidence,
}

#[derive(Clone, Debug)]
struct ProbeRecoveryPlan {
    journal_identity: HeldIdentity,
    members: Vec<ProbeRecoveryMember>,
}

impl ProbeRecoveryPlan {
    fn capture(
        rooted: &RootedFs,
        domain: Domain,
        token: &str,
        active: &str,
        active_identity: &HeldIdentity,
    ) -> Result<Self> {
        validate_probe_journal_identity(rooted, active_identity)?;
        let expected_active = format!(
            ".surgeist-generator/probes/{}/active-{token}",
            domain.as_str()
        );
        if active != expected_active {
            return Err(transaction_error(
                "recover rename capability probe",
                "active probe journal path differs from its domain and token",
            ));
        }
        let names = rooted.list_dir(active)?;
        let intent_name = "intent.json";
        let temporary_name = format!("intent-{token}.tmp");
        let left_name = format!("probe-left-{token}");
        let right_name = format!("probe-right-{token}");
        let moved_name = format!("probe-moved-{token}");
        let allowed = [
            intent_name.to_owned(),
            temporary_name.clone(),
            left_name.clone(),
            right_name.clone(),
            moved_name.clone(),
        ];
        if names.iter().any(|name| !allowed.contains(name)) {
            return Err(transaction_error(
                "recover rename capability probe",
                "probe journal contains an unknown member",
            ));
        }
        let has_intent = names.iter().any(|name| name == intent_name);
        let has_temporary = names.iter().any(|name| name == &temporary_name);
        let has_left = names.iter().any(|name| name == &left_name);
        let has_right = names.iter().any(|name| name == &right_name);
        let has_moved = names.iter().any(|name| name == &moved_name);
        if has_intent && has_temporary {
            return Err(transaction_error(
                "recover rename capability probe",
                "probe intent and its publication temporary coexist",
            ));
        }
        if (has_left || has_right || has_moved) && !has_intent {
            return Err(transaction_error(
                "recover rename capability probe",
                "probe directories have no durable intent",
            ));
        }
        if has_left && has_moved {
            return Err(transaction_error(
                "recover rename capability probe",
                "probe journal contains mutually exclusive left and moved names",
            ));
        }

        let expected_intent = ProbeIntent {
            schema_version: 1,
            domain,
            token: token.to_owned(),
            journal_path: active.to_owned(),
            journal_identity: active_identity.clone(),
        };
        let expected_intent_bytes =
            canonical_json(&expected_intent, "serialize recovered probe intent")?;
        let mut members = Vec::with_capacity(names.len());
        for name in names {
            let path = format!("{active}/{name}");
            let identity = rooted.identity_at(&path)?.ok_or_else(|| {
                transaction_error(
                    "recover rename capability probe",
                    format!("probe member disappeared: {name}"),
                )
            })?;
            if name == intent_name || name == temporary_name {
                validate_probe_file_identity(rooted, &identity)?;
                let bytes = rooted.read_file(&path, PRIVATE_FILE_MODE)?;
                if name == intent_name {
                    let intent: ProbeIntent = serde_json::from_slice(&bytes).map_err(|error| {
                        transaction_error(
                            "recover rename capability probe",
                            format!("invalid probe intent: {error}"),
                        )
                    })?;
                    validate_canonical_owner_json(
                        &bytes,
                        &intent,
                        "recover rename capability probe",
                        "probe intent",
                    )?;
                    if intent != expected_intent {
                        return Err(transaction_error(
                            "recover rename capability probe",
                            "probe intent differs from its journal identity, domain, or token",
                        ));
                    }
                } else if !expected_intent_bytes.starts_with(&bytes) {
                    return Err(transaction_error(
                        "recover rename capability probe",
                        "probe intent temporary is not a canonical publication prefix",
                    ));
                }
                members.push(ProbeRecoveryMember {
                    name,
                    identity,
                    evidence: ProbeRecoveryMemberEvidence::File(bytes),
                });
            } else {
                let directory_inventory = capture_empty_probe_directory_inventory(
                    rooted,
                    &path,
                    &identity,
                    "recover rename capability probe",
                )?;
                members.push(ProbeRecoveryMember {
                    name,
                    identity,
                    evidence: ProbeRecoveryMemberEvidence::Directory(directory_inventory),
                });
            }
        }
        members.sort_by_key(|member| probe_recovery_order(&member.name, token));
        let plan = Self {
            journal_identity: active_identity.clone(),
            members,
        };
        plan.revalidate(rooted, active)?;
        Ok(plan)
    }

    fn revalidate(&self, rooted: &RootedFs, active: &str) -> Result<()> {
        let actual_journal = rooted.identity_at(active)?.ok_or_else(|| {
            transaction_error(
                "recover rename capability probe",
                "probe journal disappeared after validation",
            )
        })?;
        validate_probe_journal_identity(rooted, &actual_journal)?;
        if !self.journal_identity.matches_recovery(&actual_journal) {
            return Err(transaction_error(
                "recover rename capability probe",
                "probe journal identity changed after validation",
            ));
        }
        let mut expected_names = self
            .members
            .iter()
            .map(|member| member.name.clone())
            .collect::<Vec<_>>();
        expected_names.sort();
        if rooted.list_dir(active)? != expected_names {
            return Err(transaction_error(
                "recover rename capability probe",
                "probe inventory changed after validation",
            ));
        }
        for member in &self.members {
            let path = format!("{active}/{}", member.name);
            let actual = rooted.identity_at(&path)?.ok_or_else(|| {
                transaction_error(
                    "recover rename capability probe",
                    format!("probe member disappeared after validation: {}", member.name),
                )
            })?;
            if !member.identity.matches_recovery(&actual) {
                return Err(transaction_error(
                    "recover rename capability probe",
                    format!(
                        "probe member identity changed after validation: {}",
                        member.name
                    ),
                ));
            }
            match &member.evidence {
                ProbeRecoveryMemberEvidence::File(bytes) => {
                    validate_probe_file_identity(rooted, &actual)?;
                    if rooted.read_file(&path, PRIVATE_FILE_MODE)? != *bytes {
                        return Err(transaction_error(
                            "recover rename capability probe",
                            format!(
                                "probe member bytes changed after validation: {}",
                                member.name
                            ),
                        ));
                    }
                }
                ProbeRecoveryMemberEvidence::Directory(inventory) => {
                    validate_probe_directory_inventory(
                        rooted,
                        &path,
                        &actual,
                        inventory,
                        "recover rename capability probe",
                    )?;
                }
            }
        }
        Ok(())
    }
}

fn probe_recovery_order(name: &str, token: &str) -> usize {
    if name == format!("probe-left-{token}") {
        0
    } else if name == format!("probe-moved-{token}") {
        1
    } else if name == format!("probe-right-{token}") {
        2
    } else if name == format!("intent-{token}.tmp") {
        3
    } else {
        4
    }
}

fn validate_probe_journal_identity(rooted: &RootedFs, identity: &HeldIdentity) -> Result<()> {
    validate_probe_directory_identity(rooted, identity).map_err(|error| {
        probe_artifact_error(
            "validate rename capability probe journal",
            "probe journal has the wrong type, mode, identity, owner, alias, or mount",
            error,
        )
    })
}

fn validate_probe_directory_identity(rooted: &RootedFs, identity: &HeldIdentity) -> Result<()> {
    if identity.kind() != NodeKind::Directory
        || identity.mode() != PRIVATE_DIRECTORY_MODE
        || identity.owner() != rooted.identity().owner()
        || identity.device() != rooted.identity().device()
        || identity.fsid() != rooted.identity().fsid()
    {
        return Err(transaction_error(
            "validate rename capability probe directory",
            "probe directory has the wrong type, mode, identity, owner, alias, or mount",
        ));
    }
    Ok(())
}

fn capture_empty_probe_directory_inventory(
    rooted: &RootedFs,
    path: &str,
    identity: &HeldIdentity,
    operation: &str,
) -> Result<Vec<String>> {
    validate_probe_directory_identity(rooted, identity)?;
    let inventory = rooted.list_dir(path)?;
    if !inventory.is_empty() {
        return Err(transaction_error(
            operation,
            format!("probe directory contains unknown nested evidence: {path}"),
        ));
    }
    Ok(inventory)
}

fn validate_probe_directory_inventory(
    rooted: &RootedFs,
    path: &str,
    identity: &HeldIdentity,
    expected_inventory: &[String],
    operation: &str,
) -> Result<()> {
    validate_probe_directory_identity(rooted, identity)?;
    if rooted.list_dir(path)? != expected_inventory {
        return Err(transaction_error(
            operation,
            format!("probe directory inventory changed after classification: {path}"),
        ));
    }
    Ok(())
}

fn validate_probe_file_identity(rooted: &RootedFs, identity: &HeldIdentity) -> Result<()> {
    if identity.kind() != NodeKind::Regular
        || identity.mode() != PRIVATE_FILE_MODE
        || identity.owner() != rooted.identity().owner()
        || identity.link_count() != Some(1)
        || identity.device() != rooted.identity().device()
        || identity.fsid() != rooted.identity().fsid()
    {
        return Err(transaction_error(
            "validate rename capability probe file",
            "probe file has the wrong type, mode, identity, owner, alias, or mount",
        ));
    }
    Ok(())
}

#[derive(Clone)]
struct ProbeInstallMember<'a> {
    name: &'a str,
    path: &'a str,
    identity: &'a HeldIdentity,
    directory_inventory: Vec<String>,
}

impl<'a> ProbeInstallMember<'a> {
    fn capture(
        rooted: &RootedFs,
        name: &'a str,
        path: &'a str,
        identity: &'a HeldIdentity,
    ) -> Result<Self> {
        let directory_inventory = capture_empty_probe_directory_inventory(
            rooted,
            path,
            identity,
            "classify rename capability probe cleanup",
        )?;
        Ok(Self {
            name,
            path,
            identity,
            directory_inventory,
        })
    }
}

fn probe_rename_flags_journaled(
    rooted: &RootedFs,
    intent: &ProbeIntent,
    intent_bytes: &[u8],
    #[cfg(test)] mut control: Option<&mut ProbeInstallControl>,
) -> Result<()> {
    let active = &intent.journal_path;
    let token = &intent.token;
    let left_name = format!("probe-left-{token}");
    let right_name = format!("probe-right-{token}");
    let moved_name = format!("probe-moved-{token}");
    let left = format!("{active}/{left_name}");
    let right = format!("{active}/{right_name}");
    let moved = format!("{active}/{moved_name}");
    let left_identity = rooted.create_dir_exclusive(&left, PRIVATE_DIRECTORY_MODE)?;
    let right_identity = rooted.create_dir_exclusive(&right, PRIVATE_DIRECTORY_MODE)?;
    let before_exclusive = [
        ProbeInstallMember::capture(rooted, &left_name, &left, &left_identity)?,
        ProbeInstallMember::capture(rooted, &right_name, &right, &right_identity)?,
    ];

    #[cfg(test)]
    if control
        .as_ref()
        .is_some_and(|control| control.fault == ProbeCapabilityFault::FailExclusiveRename)
    {
        return finish_probe_capability_failure(
            rooted,
            intent,
            intent_bytes,
            "exclusive",
            injected_probe_capability_error("exclusive"),
            &before_exclusive,
            control.as_deref_mut(),
        );
    }
    if let Err(rename) = rooted.rename_exclusive_bound(&left, &moved, &left_identity) {
        return finish_probe_capability_failure(
            rooted,
            intent,
            intent_bytes,
            "exclusive",
            rename,
            &before_exclusive,
            #[cfg(test)]
            control.as_deref_mut(),
        );
    }

    let before_swap = [
        ProbeInstallMember::capture(rooted, &moved_name, &moved, &left_identity)?,
        ProbeInstallMember::capture(rooted, &right_name, &right, &right_identity)?,
    ];
    #[cfg(test)]
    if control
        .as_ref()
        .is_some_and(|control| control.fault == ProbeCapabilityFault::FailSwapRename)
    {
        return finish_probe_capability_failure(
            rooted,
            intent,
            intent_bytes,
            "swap",
            injected_probe_capability_error("swap"),
            &before_swap,
            control.as_deref_mut(),
        );
    }
    if let Err(rename) = rooted.rename_swap_bound(&moved, &right, &left_identity, &right_identity) {
        return finish_probe_capability_failure(
            rooted,
            intent,
            intent_bytes,
            "swap",
            rename,
            &before_swap,
            #[cfg(test)]
            control.as_deref_mut(),
        );
    }

    let after_swap = [
        ProbeInstallMember::capture(rooted, &right_name, &right, &left_identity)?,
        ProbeInstallMember::capture(rooted, &moved_name, &moved, &right_identity)?,
    ];
    cleanup_probe_members(
        rooted,
        intent,
        intent_bytes,
        &after_swap,
        #[cfg(test)]
        control,
    )
}

fn finish_probe_capability_failure(
    rooted: &RootedFs,
    intent: &ProbeIntent,
    intent_bytes: &[u8],
    rename_kind: &str,
    rename: GeneratorError,
    members: &[ProbeInstallMember<'_>],
    #[cfg(test)] control: Option<&mut ProbeInstallControl>,
) -> Result<()> {
    if let Err(validation) = validate_probe_install_members(rooted, intent, intent_bytes, members) {
        return Err(probe_state_failure(rename_kind, rename, validation));
    }
    let capability = if rename.kind() == GeneratorErrorKind::UnsupportedPlatform {
        rename
    } else {
        GeneratorError::with_source(
            GeneratorErrorKind::UnsupportedPlatform,
            format!("probe rooted {rename_kind} rename"),
            rename.to_string(),
            rename,
        )
    };
    match cleanup_probe_members(
        rooted,
        intent,
        intent_bytes,
        members,
        #[cfg(test)]
        control,
    ) {
        Ok(()) => Err(capability),
        Err(cleanup) => Err(probe_cleanup_failure(capability, cleanup)),
    }
}

fn cleanup_probe_members(
    rooted: &RootedFs,
    intent: &ProbeIntent,
    intent_bytes: &[u8],
    members: &[ProbeInstallMember<'_>],
    #[cfg(test)] mut control: Option<&mut ProbeInstallControl>,
) -> Result<()> {
    validate_probe_install_members(rooted, intent, intent_bytes, members)?;
    for (index, member) in members.iter().enumerate() {
        #[cfg(test)]
        if let Some(control) = control.as_deref_mut() {
            control.before_cleanup(member.path)?;
        }
        validate_probe_install_members(rooted, intent, intent_bytes, &members[index..])?;
        rooted.remove_dir_exact(member.path, member.identity)?;
    }
    validate_probe_install_members(rooted, intent, intent_bytes, &[])?;
    #[cfg(test)]
    if let Some(control) = control {
        control.before_cleanup("probe-directory-sync")?;
    }
    rooted.sync_dir(&intent.journal_path)
}

fn validate_probe_install_members(
    rooted: &RootedFs,
    intent: &ProbeIntent,
    intent_bytes: &[u8],
    members: &[ProbeInstallMember<'_>],
) -> Result<()> {
    let active = &intent.journal_path;
    let active_identity = rooted.identity_at(active)?.ok_or_else(|| {
        transaction_error(
            "validate rename capability probe cleanup",
            "active probe journal disappeared",
        )
    })?;
    validate_probe_journal_identity(rooted, &active_identity)?;
    if !intent.journal_identity.matches_recovery(&active_identity) {
        return Err(transaction_error(
            "validate rename capability probe cleanup",
            "active probe journal identity changed",
        ));
    }

    let mut expected_names = members
        .iter()
        .map(|member| member.name.to_owned())
        .chain(std::iter::once("intent.json".to_owned()))
        .collect::<Vec<_>>();
    expected_names.sort();
    if rooted.list_dir(active)? != expected_names {
        return Err(transaction_error(
            "validate rename capability probe cleanup",
            "probe journal inventory changed",
        ));
    }

    let intent_path = format!("{active}/intent.json");
    let intent_identity = rooted.identity_at(&intent_path)?.ok_or_else(|| {
        transaction_error(
            "validate rename capability probe cleanup",
            "probe intent disappeared",
        )
    })?;
    validate_probe_file_identity(rooted, &intent_identity)?;
    if rooted.read_file(&intent_path, PRIVATE_FILE_MODE)? != intent_bytes {
        return Err(transaction_error(
            "validate rename capability probe cleanup",
            "probe intent bytes changed",
        ));
    }
    for member in members {
        let actual = rooted.identity_at(member.path)?.ok_or_else(|| {
            transaction_error(
                "validate rename capability probe cleanup",
                format!("probe member disappeared: {}", member.name),
            )
        })?;
        if !member.identity.matches_recovery(&actual) {
            return Err(transaction_error(
                "validate rename capability probe cleanup",
                format!("probe member identity changed: {}", member.name),
            ));
        }
        validate_probe_directory_inventory(
            rooted,
            member.path,
            &actual,
            &member.directory_inventory,
            "validate rename capability probe cleanup",
        )?;
    }
    Ok(())
}

#[cfg(test)]
impl ProbeInstallControl {
    fn before_cleanup(&mut self, label: &str) -> Result<()> {
        let cleanup_index = self.cleanup_trace.len();
        self.cleanup_trace.push(label.to_owned());
        if self.fail_cleanup_at == Some(cleanup_index) {
            return Err(transaction_error(
                "inject rename-probe cleanup failure",
                format!("cleanup primitive rejected before mutation: {label}"),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
fn injected_probe_capability_error(rename: &str) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::UnsupportedPlatform,
        format!("probe rooted {rename} rename"),
        format!("injected {rename} rename capability failure"),
        std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            format!("injected {rename} rename capability failure"),
        ),
    )
}

fn cleanup_probe_install_journal(
    rooted: &RootedFs,
    intent: &ProbeIntent,
    intent_bytes: &[u8],
    #[cfg(test)] mut control: Option<&mut ProbeInstallControl>,
) -> Result<()> {
    let active = &intent.journal_path;
    let active_identity = rooted.identity_at(active)?.ok_or_else(|| {
        transaction_error(
            "clean rename capability probe journal",
            "active probe journal disappeared",
        )
    })?;
    if !intent.journal_identity.matches_recovery(&active_identity) {
        return Err(transaction_error(
            "clean rename capability probe journal",
            "active probe journal identity changed",
        ));
    }
    validate_probe_journal_identity(rooted, &active_identity)?;
    if rooted.list_dir(active)? != ["intent.json"] {
        return Err(transaction_error(
            "clean rename capability probe journal",
            "probe member cleanup did not leave the exact intent-only journal",
        ));
    }
    let intent_path = format!("{active}/intent.json");
    let intent_identity = rooted.identity_at(&intent_path)?.ok_or_else(|| {
        transaction_error(
            "clean rename capability probe journal",
            "probe intent disappeared",
        )
    })?;
    validate_probe_file_identity(rooted, &intent_identity)?;
    if rooted.read_file(&intent_path, PRIVATE_FILE_MODE)? != intent_bytes {
        return Err(transaction_error(
            "clean rename capability probe journal",
            "probe intent bytes changed before cleanup",
        ));
    }
    #[cfg(test)]
    if let Some(control) = control.as_deref_mut() {
        control.before_cleanup("intent.json")?;
    }
    rooted.remove_file_exact(&intent_path, &intent_identity)?;
    if !rooted.list_dir(active)?.is_empty() {
        return Err(transaction_error(
            "clean rename capability probe journal",
            "probe journal gained a member before final removal",
        ));
    }
    #[cfg(test)]
    if let Some(control) = control {
        control.before_cleanup("journal-directory")?;
    }
    let actual_active = rooted.identity_at(active)?.ok_or_else(|| {
        transaction_error(
            "clean rename capability probe journal",
            "probe journal disappeared before final removal",
        )
    })?;
    if !active_identity.matches_recovery(&actual_active) {
        return Err(transaction_error(
            "clean rename capability probe journal",
            "probe journal identity changed before final removal",
        ));
    }
    rooted.remove_dir_exact(active, &active_identity)
}

#[derive(Debug)]
struct ProbeCleanupFailure {
    capability: GeneratorError,
    cleanup: GeneratorError,
}

impl std::fmt::Display for ProbeCleanupFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "capability failure: {}; cleanup failure: {}",
            self.capability, self.cleanup
        )
    }
}

impl std::error::Error for ProbeCleanupFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.cleanup)
    }
}

fn probe_cleanup_failure(capability: GeneratorError, cleanup: GeneratorError) -> GeneratorError {
    let context = ProbeCleanupFailure {
        capability,
        cleanup,
    };
    GeneratorError::with_source(
        GeneratorErrorKind::ArtifactTransaction,
        "clean failed rename capability probe",
        context.to_string(),
        context,
    )
}

#[derive(Debug)]
struct ProbeStateFailure {
    rename: GeneratorError,
    validation: GeneratorError,
}

impl std::fmt::Display for ProbeStateFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "rename failure: {}; retained-state validation failure: {}",
            self.rename, self.validation
        )
    }
}

impl std::error::Error for ProbeStateFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.rename)
    }
}

fn probe_state_failure(
    rename_kind: &str,
    rename: GeneratorError,
    validation: GeneratorError,
) -> GeneratorError {
    let context = ProbeStateFailure { rename, validation };
    GeneratorError::with_source(
        GeneratorErrorKind::ArtifactTransaction,
        format!("classify failed rooted {rename_kind} rename"),
        context.to_string(),
        context,
    )
}

fn probe_artifact_error(
    operation: &str,
    detail: impl Into<String>,
    source: GeneratorError,
) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::ArtifactTransaction,
        operation,
        detail,
        source,
    )
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct OwnerRecord {
    schema_version: u8,
    generator: String,
    pid: u32,
    owner_root: String,
    corpus_root: String,
    scope: String,
    command: String,
    unix_start_time: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct OwnerIntent {
    schema_version: u8,
    authority_key: String,
    token: String,
    owner_path: String,
    stage_path: String,
    old_digest: Option<Sha256Digest>,
    old_identity: Option<HeldIdentity>,
    new_digest: Sha256Digest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum OwnerOutcomeKind {
    Aborted,
    Committed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct OwnerOutcomeMarker {
    schema_version: u8,
    authority_key: String,
    token: String,
    owner_path: String,
    outcome: OwnerOutcomeKind,
    old_digest: Option<Sha256Digest>,
    old_identity: Option<HeldIdentity>,
    new_digest: Sha256Digest,
    visible_digest: Option<Sha256Digest>,
    visible_identity: Option<HeldIdentity>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug)]
struct OwnerRecordStamp {
    pid: u32,
    unix_start_time: u64,
}

#[derive(Clone, Debug)]
struct OwnerVisibility {
    digest: Option<Sha256Digest>,
    identity: Option<HeldIdentity>,
}

#[derive(Clone, Debug)]
struct OwnerTemporaryEvidence {
    name: String,
    identity: HeldIdentity,
    bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
struct OwnerRecoveryMember {
    name: String,
    identity: HeldIdentity,
    bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
struct OwnerRecoveryPlan {
    journal_identity: HeldIdentity,
    owner_path: String,
    visibility: OwnerVisibility,
    visible_bytes: Option<Vec<u8>>,
    members: Vec<OwnerRecoveryMember>,
}

#[derive(Clone, Copy, Debug)]
struct OwnerTemporaryContext<'a> {
    token: &'a str,
    intent: &'a OwnerIntent,
    registration: Option<&'a HeldIdentity>,
    prepared: Option<&'a Sha256Digest>,
    stage_identity: Option<&'a HeldIdentity>,
    stage_digest: Option<&'a Sha256Digest>,
    existing_outcome: Option<OwnerOutcomeKind>,
    expected_outcome: OwnerOutcomeKind,
    visibility: &'a OwnerVisibility,
}

impl OwnerRecoveryPlan {
    fn capture(
        rooted: &RootedFs,
        journal: &str,
        journal_identity: &HeldIdentity,
        owner_path: &str,
        visibility: OwnerVisibility,
        names: &[String],
    ) -> Result<Self> {
        validate_owner_cleanup_journal_identity(rooted, journal, journal_identity)?;
        validate_owner_visibility(rooted, owner_path, &visibility)?;
        let visible_bytes = read_owner_visibility_bytes(rooted, owner_path, &visibility)?;
        let mut expected_names = names.to_vec();
        expected_names.sort();
        if rooted.list_dir(journal)? != expected_names {
            return Err(transaction_error(
                "recover owner transaction",
                "owner recovery inventory changed while capturing its plan",
            ));
        }
        let mut members = Vec::with_capacity(names.len());
        for name in names {
            let path = format!("{journal}/{name}");
            let identity = rooted.identity_at(&path)?.ok_or_else(|| {
                transaction_error(
                    "recover owner transaction",
                    format!("owner recovery member disappeared: {name}"),
                )
            })?;
            validate_owner_cleanup_member_identity(rooted, name, &identity)?;
            let bytes = read_owner_cleanup_file(rooted, &path, &identity)?;
            members.push(OwnerRecoveryMember {
                name: name.clone(),
                identity,
                bytes,
            });
        }
        let plan = Self {
            journal_identity: journal_identity.clone(),
            owner_path: owner_path.to_owned(),
            visibility,
            visible_bytes,
            members,
        };
        plan.revalidate(rooted, journal)?;
        Ok(plan)
    }

    fn member(&self, name: &str) -> Result<&OwnerRecoveryMember> {
        self.members
            .iter()
            .find(|member| member.name == name)
            .ok_or_else(|| {
                transaction_error(
                    "recover owner transaction",
                    format!("owner recovery member disappeared: {name}"),
                )
            })
    }

    fn member_optional(&self, name: &str) -> Option<&OwnerRecoveryMember> {
        self.members.iter().find(|member| member.name == name)
    }

    fn remove_member(&mut self, name: &str) -> Result<OwnerRecoveryMember> {
        let index = self
            .members
            .iter()
            .position(|member| member.name == name)
            .ok_or_else(|| {
                transaction_error(
                    "recover owner transaction",
                    format!("owner recovery removed an unplanned member: {name}"),
                )
            })?;
        Ok(self.members.remove(index))
    }

    fn add_member(&mut self, name: &str, identity: HeldIdentity, bytes: Vec<u8>) -> Result<()> {
        if self.member_optional(name).is_some() {
            return Err(transaction_error(
                "recover owner transaction",
                format!("owner recovery published an already-planned member: {name}"),
            ));
        }
        self.members.push(OwnerRecoveryMember {
            name: name.to_owned(),
            identity,
            bytes,
        });
        Ok(())
    }

    fn revalidate(&self, rooted: &RootedFs, journal: &str) -> Result<()> {
        validate_owner_cleanup_journal_identity(rooted, journal, &self.journal_identity)?;
        validate_owner_visibility(rooted, &self.owner_path, &self.visibility)?;
        if read_owner_visibility_bytes(rooted, &self.owner_path, &self.visibility)?
            != self.visible_bytes
        {
            return Err(transaction_error(
                "recover owner transaction",
                "visible owner bytes changed after recovery-plan validation",
            ));
        }
        let mut expected_names = self
            .members
            .iter()
            .map(|member| member.name.clone())
            .collect::<Vec<_>>();
        expected_names.sort();
        if rooted.list_dir(journal)? != expected_names {
            return Err(transaction_error(
                "recover owner transaction",
                "owner recovery inventory changed after plan validation",
            ));
        }
        for member in &self.members {
            validate_owner_cleanup_member_identity(rooted, &member.name, &member.identity)?;
            let path = format!("{journal}/{}", member.name);
            if read_owner_cleanup_file(rooted, &path, &member.identity)? != member.bytes {
                return Err(transaction_error(
                    "recover owner transaction",
                    format!(
                        "owner recovery member bytes changed after validation: {}",
                        member.name
                    ),
                ));
            }
        }
        Ok(())
    }
}

fn install_owner_record(
    rooted: &RootedFs,
    location: &CorpusLocation,
    domain: Domain,
    metadata: &LeaseMetadata,
    token: &str,
    authority_key: &str,
) -> Result<()> {
    let unix_start_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            transaction_error(
                "construct historical generation owner",
                format!("system clock precedes Unix epoch: {error}"),
            )
        })?
        .as_secs();
    let owner = OwnerRecord {
        schema_version: 1,
        generator: metadata.generator.clone(),
        pid: std::process::id(),
        owner_root: location.owner_root().display().to_string(),
        corpus_root: location.corpus_root().display().to_string(),
        scope: metadata.scope.clone(),
        command: metadata.command.clone(),
        unix_start_time,
    };
    let owner_bytes = canonical_json(&owner, "serialize historical generation owner")?;
    install_owner_record_bytes(rooted, domain, token, authority_key, &owner_bytes)
}

#[cfg(test)]
fn install_owner_record_controlled(
    rooted: &RootedFs,
    location: &CorpusLocation,
    domain: Domain,
    metadata: &LeaseMetadata,
    token: &str,
    authority_key: &str,
    stamp: OwnerRecordStamp,
) -> Result<()> {
    let owner = OwnerRecord {
        schema_version: 1,
        generator: metadata.generator.clone(),
        pid: stamp.pid,
        owner_root: location.owner_root().display().to_string(),
        corpus_root: location.corpus_root().display().to_string(),
        scope: metadata.scope.clone(),
        command: metadata.command.clone(),
        unix_start_time: stamp.unix_start_time,
    };
    let owner_bytes = canonical_json(&owner, "serialize historical generation owner")?;
    install_owner_record_bytes(rooted, domain, token, authority_key, &owner_bytes)
}

fn install_owner_record_bytes(
    rooted: &RootedFs,
    domain: Domain,
    token: &str,
    authority_key: &str,
    owner_bytes: &[u8],
) -> Result<()> {
    #[cfg(test)]
    let _observation_phase = rooted.begin_observation_phase(DurabilityPhase::OwnerInstall);
    let parent = format!(
        ".surgeist-generator/leases/{}/{}",
        domain.as_str(),
        OWNER_TRANSACTIONS
    );
    let active = format!("{parent}/active-{token}");
    let owner_path = owner_path(domain);
    let historical_visibility = read_owner_visibility(rooted, &owner_path)?;
    let old_digest = historical_visibility.digest;
    let old_identity = historical_visibility.identity;
    let active_identity = rooted.create_dir_exclusive(&active, PRIVATE_DIRECTORY_MODE)?;
    let stage_path = format!("{active}/owner.stage");
    let intent = OwnerIntent {
        schema_version: 1,
        authority_key: authority_key.to_owned(),
        token: token.to_owned(),
        owner_path: owner_path.clone(),
        stage_path: stage_path.clone(),
        old_digest,
        old_identity,
        new_digest: Sha256Digest::from_bytes(owner_bytes),
    };
    let mut stage = rooted.create_file_handle_exclusive(&stage_path, b"", PRIVATE_FILE_MODE)?;
    let stage_identity = rooted.identity_of_handle(&stage)?;
    #[cfg(test)]
    rooted.observe_handle_identity(&stage_path);
    rooted.publish_file_exclusive(
        &active,
        "stage-registration.json",
        &format!("stage-registration-{token}.tmp"),
        &canonical_json(&stage_identity, "serialize owner-stage registration")?,
        PRIVATE_FILE_MODE,
    )?;
    rooted
        .write_file_handle_all(&stage_path, &mut stage, owner_bytes)
        .map_err(|source| {
            transaction_source(
                "write historical generation owner stage",
                &stage_path,
                source,
            )
        })?;
    rooted
        .flush_file_handle(&stage_path, &mut stage)
        .map_err(|source| {
            transaction_source(
                "flush historical generation owner stage",
                &stage_path,
                source,
            )
        })?;
    rooted
        .sync_file_handle(&stage_path, &stage)
        .map_err(|source| {
            transaction_source(
                "sync historical generation owner stage",
                &stage_path,
                source,
            )
        })?;
    rooted.validate_handle_at(&stage_path, &stage, PRIVATE_FILE_MODE)?;
    rooted.drop_file_handle(&stage_path, stage);
    rooted.publish_file_exclusive(
        &active,
        "intent.json",
        &format!("intent-{token}.tmp"),
        &canonical_json(&intent, "serialize owner-record intent")?,
        PRIVATE_FILE_MODE,
    )?;
    rooted.publish_file_exclusive(
        &active,
        "prepared.json",
        &format!("prepared-{token}.tmp"),
        &canonical_json(&intent.new_digest, "serialize owner prepared marker")?,
        PRIVATE_FILE_MODE,
    )?;
    if let Some(old_identity) = &intent.old_identity {
        rooted.rename_swap_bound(&stage_path, &owner_path, &stage_identity, old_identity)?;
    } else {
        rooted.rename_exclusive_bound(&stage_path, &owner_path, &stage_identity)?;
    }
    rooted.sync_dir(&format!(".surgeist-generator/leases/{}", domain.as_str()))?;
    ensure_owner_outcome(
        rooted,
        &active,
        &active_identity,
        token,
        &intent,
        OwnerOutcomeKind::Committed,
        &OwnerVisibility {
            digest: Some(intent.new_digest.clone()),
            identity: Some(stage_identity.clone()),
        },
    )?;
    if let Some(old_stage) = rooted.identity_at(&stage_path)? {
        let expected_old = intent.old_identity.as_ref().ok_or_else(|| {
            transaction_error(
                "clean historical generation owner",
                "unexpected stage remains after exclusive owner commit",
            )
        })?;
        if !expected_old.matches_recovery(&old_stage) {
            return Err(transaction_error(
                "clean historical generation owner",
                "swapped old owner identity changed",
            ));
        }
        rooted.remove_file_exact(&stage_path, expected_old)?;
    }
    cleanup_owner_journal(
        rooted,
        domain,
        authority_key,
        token,
        &active,
        active_identity,
    )
}

fn recover_owner_transactions(
    rooted: &RootedFs,
    domain: Domain,
    authority_key: &str,
) -> Result<()> {
    #[cfg(test)]
    let result = recover_owner_transactions_inner(rooted, domain, authority_key, None);
    #[cfg(not(test))]
    let result = recover_owner_transactions_inner(rooted, domain, authority_key);
    result
}

#[cfg(test)]
struct OwnerRecoveryControl<'a> {
    before_mutation: &'a mut dyn FnMut(&str) -> Result<()>,
}

#[cfg(test)]
impl<'a> OwnerRecoveryControl<'a> {
    fn new(before_mutation: &'a mut dyn FnMut(&str) -> Result<()>) -> Self {
        Self { before_mutation }
    }
}

#[cfg(test)]
fn recover_owner_transactions_controlled(
    rooted: &RootedFs,
    domain: Domain,
    authority_key: &str,
    control: &mut OwnerRecoveryControl<'_>,
) -> Result<()> {
    recover_owner_transactions_inner(rooted, domain, authority_key, Some(control))
}

fn recover_owner_transactions_inner(
    rooted: &RootedFs,
    domain: Domain,
    authority_key: &str,
    #[cfg(test)] mut control: Option<&mut OwnerRecoveryControl<'_>>,
) -> Result<()> {
    #[cfg(test)]
    let _observation_phase = rooted.begin_observation_phase(DurabilityPhase::OwnerRecovery);
    let parent = format!(
        ".surgeist-generator/leases/{}/{}",
        domain.as_str(),
        OWNER_TRANSACTIONS
    );
    read_owner_visibility(rooted, &owner_path(domain))?;
    for name in rooted.list_dir(&parent)? {
        let Some(token) = name.strip_prefix("active-") else {
            return Err(transaction_error(
                "recover owner transaction",
                format!("unknown owner journal: {name}"),
            ));
        };
        validate_token(token)?;
        let active = format!("{parent}/{name}");
        let active_identity = rooted.identity_at(&active)?.ok_or_else(|| {
            transaction_error("recover owner transaction", "owner journal disappeared")
        })?;
        validate_owner_cleanup_journal_identity(rooted, &active, &active_identity)?;
        let names = rooted.list_dir(&active)?;
        if names.is_empty() {
            let owner_path = owner_path(domain);
            let visibility = read_owner_visibility(rooted, &owner_path)?;
            let plan = OwnerRecoveryPlan::capture(
                rooted,
                &active,
                &active_identity,
                &owner_path,
                visibility,
                &names,
            )?;
            #[cfg(test)]
            if let Some(control) = control.as_deref_mut() {
                (control.before_mutation)("empty-journal-unlink")?;
            }
            plan.revalidate(rooted, &active)?;
            rooted.remove_dir_exact(&active, &active_identity)?;
            continue;
        }
        if !names.iter().any(|name| name == "intent.json") {
            if names.len() == 1 && matches!(names[0].as_str(), "committed" | "aborted") {
                validate_standalone_owner_outcome(
                    rooted,
                    domain,
                    authority_key,
                    token,
                    &active,
                    &names[0],
                )?;
                cleanup_owner_journal(
                    rooted,
                    domain,
                    authority_key,
                    token,
                    &active,
                    active_identity,
                )?;
                continue;
            }
            recover_pre_intent_owner_journal(
                rooted,
                domain,
                authority_key,
                token,
                &active,
                &active_identity,
                &names,
            )?;
            continue;
        }
        let expected_owner = owner_path(domain);
        let allowed = [
            "intent.json".to_owned(),
            "stage-registration.json".to_owned(),
            "prepared.json".to_owned(),
            "committed".to_owned(),
            "aborted".to_owned(),
            "owner.stage".to_owned(),
            format!("intent-{token}.tmp"),
            format!("stage-registration-{token}.tmp"),
            format!("prepared-{token}.tmp"),
            format!("committed-{token}.tmp"),
            format!("aborted-{token}.tmp"),
        ];
        if names.iter().any(|member| !allowed.contains(member)) {
            return Err(transaction_error(
                "recover owner transaction",
                "owner journal contains an unknown member",
            ));
        }
        let visibility = read_owner_visibility(rooted, &expected_owner)?;
        let mut recovery_plan = OwnerRecoveryPlan::capture(
            rooted,
            &active,
            &active_identity,
            &expected_owner,
            visibility,
            &names,
        )?;
        let intent_bytes = &recovery_plan.member("intent.json")?.bytes;
        let intent: OwnerIntent = serde_json::from_slice(intent_bytes).map_err(|error| {
            transaction_error(
                "recover owner transaction",
                format!("invalid owner intent: {error}"),
            )
        })?;
        let expected_stage = format!("{active}/owner.stage");
        if intent.schema_version != 1
            || intent.authority_key != authority_key
            || intent.token != token
            || intent.owner_path != expected_owner
            || intent.stage_path != expected_stage
            || intent.old_digest.is_some() != intent.old_identity.is_some()
        {
            return Err(transaction_error(
                "recover owner transaction",
                "owner intent authority, token, or fixed paths differ",
            ));
        }
        validate_canonical_owner_json(
            intent_bytes,
            &intent,
            "recover owner transaction",
            "owner intent",
        )?;
        let registration =
            if let Some(member) = recovery_plan.member_optional("stage-registration.json") {
                let registration_bytes = &member.bytes;
                let registration: HeldIdentity = serde_json::from_slice(registration_bytes)
                    .map_err(|error| {
                        transaction_error(
                            "recover owner transaction",
                            format!("invalid owner stage registration: {error}"),
                        )
                    })?;
                validate_canonical_owner_json(
                    registration_bytes,
                    &registration,
                    "recover owner transaction",
                    "owner stage registration",
                )?;
                Some(registration)
            } else {
                None
            };
        if let Some(registration) = &registration
            && (registration.kind() != NodeKind::Regular
                || registration.mode() != PRIVATE_FILE_MODE
                || registration.link_count() != Some(1)
                || registration.owner() != rooted.identity().owner()
                || registration.device() != rooted.identity().device()
                || registration.fsid() != rooted.identity().fsid())
        {
            return Err(transaction_error(
                "recover owner transaction",
                "owner stage registration has the wrong identity policy",
            ));
        }
        let prepared = if let Some(member) = recovery_plan.member_optional("prepared.json") {
            let prepared_bytes = &member.bytes;
            let prepared: Sha256Digest =
                serde_json::from_slice(prepared_bytes).map_err(|error| {
                    transaction_error(
                        "recover owner transaction",
                        format!("invalid owner prepared marker: {error}"),
                    )
                })?;
            validate_canonical_owner_json(
                prepared_bytes,
                &prepared,
                "recover owner transaction",
                "owner prepared marker",
            )?;
            Some(prepared)
        } else {
            None
        };
        if prepared
            .as_ref()
            .is_some_and(|digest| digest != &intent.new_digest)
            || (prepared.is_some() && registration.is_none())
        {
            return Err(transaction_error(
                "recover owner transaction",
                "owner prepared marker differs from its registration",
            ));
        }
        let visibility = recovery_plan.visibility.clone();
        let owner_identity = visibility.identity.clone();
        let owner_digest = visibility.digest.clone();
        let stage_member = recovery_plan.member_optional("owner.stage");
        let stage_identity = stage_member.map(|member| member.identity.clone());
        let stage_bytes = stage_member.map(|member| member.bytes.clone());
        let stage_digest = stage_bytes.as_ref().map(Sha256Digest::from_bytes);
        let existing_outcome = existing_owner_outcome_from_names(&names)?;
        if let Some(outcome) = existing_outcome {
            let outcome_name = match outcome {
                OwnerOutcomeKind::Aborted => "aborted",
                OwnerOutcomeKind::Committed => "committed",
            };
            let recorded = parse_owner_outcome_bytes(&recovery_plan.member(outcome_name)?.bytes)?;
            validate_existing_owner_outcome_record(
                rooted,
                &intent,
                outcome,
                &visibility,
                registration.as_ref(),
                &recorded,
                recovery_plan
                    .member_optional(&format!("{outcome_name}-{token}.tmp"))
                    .is_some(),
            )?;
        }
        if existing_outcome.is_none() {
            let registration = registration.as_ref().ok_or_else(|| {
                transaction_error(
                    "recover owner transaction",
                    "owner intent has no durable stage registration",
                )
            })?;
            let registered_owner_is_visible = owner_digest.as_ref() == Some(&intent.new_digest)
                && owner_identity
                    .as_ref()
                    .is_some_and(|actual| registration.matches_recovery(actual));
            if !registered_owner_is_visible {
                let actual = stage_identity.as_ref().ok_or_else(|| {
                    transaction_error(
                        "recover owner transaction",
                        "pre-commit owner intent has no registered stage",
                    )
                })?;
                if !registration.matches_recovery(actual)
                    || stage_digest.as_ref() != Some(&intent.new_digest)
                {
                    return Err(transaction_error(
                        "recover owner transaction",
                        "pre-commit owner stage differs from its registration or intent",
                    ));
                }
                validate_owner_record_bytes(
                    rooted,
                    stage_bytes.as_deref().ok_or_else(|| {
                        transaction_error(
                            "recover owner transaction",
                            "registered owner stage bytes disappeared",
                        )
                    })?,
                    "recover owner transaction",
                    "registered pre-commit owner stage",
                )?;
            }
        } else if stage_identity.is_some() {
            if registration.is_none() {
                return Err(transaction_error(
                    "recover owner transaction",
                    "terminal owner outcome retains a stage after its registration was removed",
                ));
            }
            if existing_outcome == Some(OwnerOutcomeKind::Committed) && prepared.is_none() {
                return Err(transaction_error(
                    "recover owner transaction",
                    "committed owner outcome retains a stage after its prepared marker was removed",
                ));
            }
        }
        let owner_matches_new = owner_digest == Some(intent.new_digest.clone())
            && match existing_outcome {
                Some(OwnerOutcomeKind::Committed) => true,
                Some(OwnerOutcomeKind::Aborted) => false,
                None => registration
                    .as_ref()
                    .zip(owner_identity.as_ref())
                    .is_some_and(|(expected, actual)| expected.matches_recovery(actual)),
            };
        let owner_matches_old = owner_digest == intent.old_digest
            && match existing_outcome {
                Some(OwnerOutcomeKind::Aborted) => true,
                Some(OwnerOutcomeKind::Committed) => false,
                None => match (&intent.old_identity, owner_identity.as_ref()) {
                    (Some(expected), Some(actual)) => expected.matches_recovery(actual),
                    (None, None) => true,
                    _ => false,
                },
            };
        let (outcome, removable_stage) = if owner_matches_new {
            if existing_outcome.is_none() {
                if registration.is_none() {
                    return Err(transaction_error(
                        "recover owner transaction",
                        "new owner is visible without stage registration",
                    ));
                }
                if prepared.is_none() {
                    return Err(transaction_error(
                        "recover owner transaction",
                        "new owner is visible without prepared marker",
                    ));
                }
            }
            let old_stage = match (
                &intent.old_digest,
                &intent.old_identity,
                stage_digest.as_ref(),
                stage_identity.as_ref(),
            ) {
                (Some(old_digest), Some(old_identity), Some(stage_digest), Some(actual))
                    if stage_digest == old_digest =>
                {
                    if !old_identity.matches_recovery(actual) {
                        return Err(transaction_error(
                            "recover owner transaction",
                            "old owner stage identity changed",
                        ));
                    }
                    validate_owner_record_bytes(
                        rooted,
                        stage_bytes.as_deref().ok_or_else(|| {
                            transaction_error(
                                "recover owner transaction",
                                "post-commit owner stage bytes disappeared",
                            )
                        })?,
                        "recover owner transaction",
                        "post-commit historical owner stage",
                    )?;
                    Some(old_identity)
                }
                (Some(_), Some(_), None, None)
                    if existing_outcome == Some(OwnerOutcomeKind::Committed) =>
                {
                    None
                }
                (None, None, None, None) => None,
                _ => {
                    return Err(transaction_error(
                        "recover owner transaction",
                        "post-commit owner stage differs from the durable old owner",
                    ));
                }
            };
            (OwnerOutcomeKind::Committed, old_stage.cloned())
        } else if owner_matches_old {
            let removable_stage = if let (Some(stage_digest), Some(actual)) =
                (stage_digest.as_ref(), stage_identity.as_ref())
            {
                if let Some(registration) = registration.as_ref() {
                    if !registration.matches_recovery(actual) {
                        return Err(transaction_error(
                            "recover owner transaction",
                            "pre-commit owner stage identity changed",
                        ));
                    }
                    if stage_digest != &intent.new_digest {
                        return Err(transaction_error(
                            "recover owner transaction",
                            "registered owner stage bytes differ from its intent",
                        ));
                    }
                    validate_owner_record_bytes(
                        rooted,
                        stage_bytes.as_deref().ok_or_else(|| {
                            transaction_error(
                                "recover owner transaction",
                                "registered owner stage bytes disappeared",
                            )
                        })?,
                        "recover owner transaction",
                        "registered owner stage",
                    )?;
                } else {
                    if prepared.is_some()
                        || !stage_bytes.as_deref().is_some_and(|bytes| bytes.is_empty())
                    {
                        return Err(transaction_error(
                            "recover owner transaction",
                            "nonempty owner stage exists without registration",
                        ));
                    }
                }
                Some(actual)
            } else if registration.is_some() && existing_outcome != Some(OwnerOutcomeKind::Aborted)
            {
                return Err(transaction_error(
                    "recover owner transaction",
                    "registered owner stage disappeared before abort",
                ));
            } else {
                None
            };
            (OwnerOutcomeKind::Aborted, removable_stage.cloned())
        } else {
            return Err(transaction_error(
                "recover owner transaction",
                "owner/stage contents match neither durable outcome",
            ));
        };
        let temporaries = validate_owner_transaction_temporaries(
            &recovery_plan,
            OwnerTemporaryContext {
                token,
                intent: &intent,
                registration: registration.as_ref(),
                prepared: prepared.as_ref(),
                stage_identity: stage_identity.as_ref(),
                stage_digest: stage_digest.as_ref(),
                existing_outcome,
                expected_outcome: outcome,
                visibility: &visibility,
            },
        )?;
        #[cfg(test)]
        remove_owner_temporaries(
            rooted,
            &active,
            &mut recovery_plan,
            &temporaries,
            control.as_deref_mut(),
        )?;
        #[cfg(not(test))]
        remove_owner_temporaries(rooted, &active, &mut recovery_plan, &temporaries)?;
        if existing_outcome.is_none() {
            #[cfg(test)]
            if let Some(control) = control.as_deref_mut() {
                (control.before_mutation)("outcome-publication")?;
            }
            publish_recovered_owner_outcome(
                rooted,
                &active,
                token,
                &intent,
                outcome,
                &visibility,
                &mut recovery_plan,
            )?;
        }
        if let Some(actual) = removable_stage {
            #[cfg(test)]
            if let Some(control) = control.as_deref_mut() {
                (control.before_mutation)("stage-removal")?;
            }
            recovery_plan.revalidate(rooted, &active)?;
            let planned_stage = recovery_plan.member("owner.stage")?.clone();
            if !actual.matches_recovery(&planned_stage.identity)
                || stage_bytes.as_ref() != Some(&planned_stage.bytes)
            {
                return Err(transaction_error(
                    "recover owner transaction",
                    "owner stage bytes changed before removal",
                ));
            }
            rooted.remove_file_exact(&intent.stage_path, &planned_stage.identity)?;
            recovery_plan.remove_member("owner.stage")?;
        }
        recovery_plan.revalidate(rooted, &active)?;
        cleanup_owner_journal(
            rooted,
            domain,
            authority_key,
            token,
            &active,
            active_identity,
        )?;
    }
    Ok(())
}

fn recover_pre_intent_owner_journal(
    rooted: &RootedFs,
    domain: Domain,
    authority_key: &str,
    token: &str,
    active: &str,
    active_identity: &HeldIdentity,
    names: &[String],
) -> Result<()> {
    let intent_temporary = format!("intent-{token}.tmp");
    let registration_temporary = format!("stage-registration-{token}.tmp");
    let stage_name = "owner.stage";
    let registration_name = "stage-registration.json";
    if names.iter().any(|name| {
        !matches!(
            name.as_str(),
            candidate
                if candidate == intent_temporary
                    || candidate == registration_temporary
                    || candidate == stage_name
                    || candidate == registration_name
        )
    }) {
        return Err(transaction_error(
            "recover owner transaction",
            "owner journal has unknown pre-intent state",
        ));
    }
    let has_stage = names.iter().any(|name| name == stage_name);
    let has_registration = names.iter().any(|name| name == registration_name);
    let has_registration_temporary = names.iter().any(|name| name == &registration_temporary);
    let has_intent_temporary = names.iter().any(|name| name == &intent_temporary);
    if !has_stage
        || (has_registration && has_registration_temporary)
        || (has_intent_temporary && (!has_registration || has_registration_temporary))
    {
        return Err(transaction_error(
            "recover owner transaction",
            "owner journal has an unbound pre-intent state",
        ));
    }

    validate_owner_cleanup_journal_identity(rooted, active, active_identity)?;
    let owner_path = owner_path(domain);
    let visibility = read_owner_visibility(rooted, &owner_path)?;
    let visible_bytes = read_owner_visibility_bytes(rooted, &owner_path, &visibility)?;
    let evidence = names
        .iter()
        .map(|name| read_owner_temporary(rooted, active, name))
        .collect::<Result<Vec<_>>>()?;
    let member = |name: &str| -> Result<&OwnerTemporaryEvidence> {
        evidence
            .iter()
            .find(|member| member.name == name)
            .ok_or_else(|| {
                transaction_error(
                    "recover owner transaction",
                    format!("owner pre-intent member disappeared: {name}"),
                )
            })
    };
    let stage = member(stage_name)?;
    let stage_digest = Sha256Digest::from_bytes(&stage.bytes);

    if has_registration_temporary {
        validate_owner_temporary_prefix(
            member(&registration_temporary)?,
            &canonical_json(
                &stage.identity,
                "serialize recovered pre-intent owner-stage registration",
            )?,
        )?;
    }
    if has_registration {
        let registration_evidence = member(registration_name)?;
        let registration: HeldIdentity = serde_json::from_slice(&registration_evidence.bytes)
            .map_err(|error| {
                transaction_error(
                    "recover owner transaction",
                    format!("invalid pre-intent owner stage registration: {error}"),
                )
            })?;
        validate_recorded_owner_identity(rooted, Some(&registration))?;
        validate_canonical_owner_json(
            &registration_evidence.bytes,
            &registration,
            "recover owner transaction",
            "pre-intent owner stage registration",
        )?;
        if !registration.matches_recovery(&stage.identity) {
            return Err(transaction_error(
                "recover owner transaction",
                "pre-intent owner stage registration differs from its stage",
            ));
        }
    }
    if has_intent_temporary {
        validate_owner_intent_temporary_prefix(
            member(&intent_temporary)?,
            authority_key,
            token,
            &owner_path,
            &format!("{active}/{stage_name}"),
            &visibility,
            &stage_digest,
        )?;
    }

    let mut removal_names = Vec::new();
    if has_intent_temporary {
        removal_names.push(intent_temporary);
    }
    if has_registration_temporary {
        removal_names.push(registration_temporary);
    } else if has_registration {
        removal_names.push(registration_name.to_owned());
    }
    removal_names.push(stage_name.to_owned());
    if removal_names.len() != names.len() {
        return Err(transaction_error(
            "recover owner transaction",
            "owner journal contains an unclassified pre-intent member",
        ));
    }
    let removal_order = removal_names
        .iter()
        .map(|name| member(name).cloned())
        .collect::<Result<Vec<_>>>()?;
    for (index, member) in removal_order.iter().enumerate() {
        revalidate_owner_pre_intent_cleanup(
            rooted,
            active,
            active_identity,
            &owner_path,
            &visibility,
            &visible_bytes,
            &removal_order[index..],
        )?;
        rooted.remove_file_exact(&format!("{active}/{}", member.name), &member.identity)?;
    }
    validate_owner_cleanup_journal_identity(rooted, active, active_identity)?;
    validate_owner_visibility(rooted, &owner_path, &visibility)?;
    if read_owner_visibility_bytes(rooted, &owner_path, &visibility)? != visible_bytes {
        return Err(transaction_error(
            "recover owner transaction",
            "visible owner bytes changed before pre-intent directory removal",
        ));
    }
    if !rooted.list_dir(active)?.is_empty() {
        return Err(transaction_error(
            "recover owner transaction",
            "owner pre-intent journal changed before removal",
        ));
    }
    rooted.remove_dir_exact(active, active_identity)
}

fn revalidate_owner_pre_intent_cleanup(
    rooted: &RootedFs,
    active: &str,
    active_identity: &HeldIdentity,
    owner_path: &str,
    visibility: &OwnerVisibility,
    visible_bytes: &Option<Vec<u8>>,
    remaining: &[OwnerTemporaryEvidence],
) -> Result<()> {
    validate_owner_cleanup_journal_identity(rooted, active, active_identity)?;
    validate_owner_visibility(rooted, owner_path, visibility)?;
    if &read_owner_visibility_bytes(rooted, owner_path, visibility)? != visible_bytes {
        return Err(transaction_error(
            "recover owner transaction",
            "visible owner bytes changed during pre-intent cleanup",
        ));
    }
    let mut expected_names = remaining
        .iter()
        .map(|member| member.name.clone())
        .collect::<Vec<_>>();
    expected_names.sort();
    if rooted.list_dir(active)? != expected_names {
        return Err(transaction_error(
            "recover owner transaction",
            "owner pre-intent inventory changed during validation",
        ));
    }
    for member in remaining {
        revalidate_owner_temporary(rooted, active, member)?;
    }
    Ok(())
}

fn validate_owner_transaction_temporaries(
    plan: &OwnerRecoveryPlan,
    context: OwnerTemporaryContext<'_>,
) -> Result<Vec<OwnerTemporaryEvidence>> {
    let OwnerTemporaryContext {
        token,
        intent,
        registration,
        prepared,
        stage_identity,
        stage_digest,
        existing_outcome,
        expected_outcome,
        visibility,
    } = context;
    let intent_temporary = format!("intent-{token}.tmp");
    let registration_temporary = format!("stage-registration-{token}.tmp");
    let prepared_temporary = format!("prepared-{token}.tmp");
    let committed_temporary = format!("committed-{token}.tmp");
    let aborted_temporary = format!("aborted-{token}.tmp");
    let temporary_names: Vec<_> = plan
        .members
        .iter()
        .map(|member| &member.name)
        .filter(|name| {
            matches!(
                name.as_str(),
                candidate
                    if candidate == intent_temporary
                        || candidate == registration_temporary
                        || candidate == prepared_temporary
                        || candidate == committed_temporary
                        || candidate == aborted_temporary
            )
        })
        .collect();
    if temporary_names.len() > 1 {
        return Err(transaction_error(
            "recover owner transaction",
            "owner journal contains conflicting publication temporaries",
        ));
    }
    let Some(name) = temporary_names.first() else {
        return Ok(Vec::new());
    };
    if name.as_str() == intent_temporary {
        return Err(transaction_error(
            "recover owner transaction",
            "published owner intent retains its temporary",
        ));
    }

    let member = plan.member(name)?;
    let evidence = OwnerTemporaryEvidence {
        name: member.name.clone(),
        identity: member.identity.clone(),
        bytes: member.bytes.clone(),
    };
    if name.as_str() == registration_temporary {
        if registration.is_some() || prepared.is_some() {
            return Err(transaction_error(
                "recover owner transaction",
                "owner stage-registration temporary conflicts with later state",
            ));
        }
        let stage_identity = stage_identity.ok_or_else(|| {
            transaction_error(
                "recover owner transaction",
                "owner stage-registration temporary has no stage",
            )
        })?;
        if stage_digest != Some(&Sha256Digest::from_bytes(b"")) {
            return Err(transaction_error(
                "recover owner transaction",
                "owner stage-registration temporary has a nonempty stage",
            ));
        }
        validate_owner_temporary_prefix(
            &evidence,
            &canonical_json(
                stage_identity,
                "serialize recovered owner-stage registration",
            )?,
        )?;
    } else if name.as_str() == prepared_temporary {
        let registration = registration.ok_or_else(|| {
            transaction_error(
                "recover owner transaction",
                "owner prepared temporary has no stage registration",
            )
        })?;
        let stage_identity = stage_identity.ok_or_else(|| {
            transaction_error(
                "recover owner transaction",
                "owner prepared temporary has no stage",
            )
        })?;
        if prepared.is_some()
            || !registration.matches_recovery(stage_identity)
            || stage_digest != Some(&intent.new_digest)
        {
            return Err(transaction_error(
                "recover owner transaction",
                "owner prepared temporary differs from its staged generation",
            ));
        }
        validate_owner_temporary_prefix(
            &evidence,
            &canonical_json(
                &intent.new_digest,
                "serialize recovered owner prepared marker",
            )?,
        )?;
    } else {
        if existing_outcome.is_some() {
            return Err(transaction_error(
                "recover owner transaction",
                "published owner outcome retains its temporary marker",
            ));
        }
        let temporary_outcome = if name.as_str() == committed_temporary {
            OwnerOutcomeKind::Committed
        } else if name.as_str() == aborted_temporary {
            OwnerOutcomeKind::Aborted
        } else {
            return Err(transaction_error(
                "recover owner transaction",
                "owner journal contains an unclassified temporary",
            ));
        };
        if temporary_outcome != expected_outcome {
            return Err(transaction_error(
                "recover owner transaction",
                "owner outcome temporary conflicts with visible state",
            ));
        }
        let marker = expected_owner_outcome(intent, expected_outcome, visibility);
        validate_owner_temporary_prefix(
            &evidence,
            &canonical_json(&marker, "serialize recovered owner outcome")?,
        )?;
    }
    Ok(vec![evidence])
}

fn remove_owner_temporaries(
    rooted: &RootedFs,
    active: &str,
    plan: &mut OwnerRecoveryPlan,
    temporaries: &[OwnerTemporaryEvidence],
    #[cfg(test)] mut control: Option<&mut OwnerRecoveryControl<'_>>,
) -> Result<()> {
    if temporaries.is_empty() {
        return Ok(());
    }
    for temporary in temporaries {
        #[cfg(test)]
        if let Some(control) = control.as_deref_mut() {
            (control.before_mutation)("temporary-removal")?;
        }
        plan.revalidate(rooted, active)?;
        let planned = plan.member(&temporary.name)?.clone();
        if planned.bytes != temporary.bytes
            || !planned.identity.matches_recovery(&temporary.identity)
        {
            return Err(transaction_error(
                "recover owner transaction",
                format!(
                    "owner temporary differs from its recovery plan: {}",
                    temporary.name
                ),
            ));
        }
        rooted.remove_file_exact(&format!("{active}/{}", temporary.name), &planned.identity)?;
        plan.remove_member(&temporary.name)?;
    }
    Ok(())
}

fn read_owner_visibility(rooted: &RootedFs, owner_path: &str) -> Result<OwnerVisibility> {
    let identity = rooted.identity_at(owner_path)?;
    validate_recorded_owner_identity(rooted, identity.as_ref())?;
    let digest = identity
        .as_ref()
        .map(|_| {
            let bytes = rooted.read_file(owner_path, PRIVATE_FILE_MODE)?;
            validate_owner_record_bytes(
                rooted,
                &bytes,
                "recover owner transaction",
                "visible owner record",
            )?;
            Ok(Sha256Digest::from_bytes(bytes))
        })
        .transpose()?;
    Ok(OwnerVisibility { digest, identity })
}

fn read_owner_temporary(
    rooted: &RootedFs,
    active: &str,
    name: &str,
) -> Result<OwnerTemporaryEvidence> {
    let path = format!("{active}/{name}");
    let identity = rooted.identity_at(&path)?.ok_or_else(|| {
        transaction_error(
            "recover owner transaction",
            format!("owner temporary disappeared: {name}"),
        )
    })?;
    validate_owner_cleanup_member_identity(rooted, name, &identity)?;
    let bytes = read_owner_cleanup_file(rooted, &path, &identity)?;
    Ok(OwnerTemporaryEvidence {
        name: name.to_owned(),
        identity,
        bytes,
    })
}

fn validate_owner_temporary_prefix(
    evidence: &OwnerTemporaryEvidence,
    expected: &[u8],
) -> Result<()> {
    if !expected.starts_with(&evidence.bytes) {
        return Err(transaction_error(
            "recover owner transaction",
            format!(
                "owner temporary is not a canonical publication prefix: {}",
                evidence.name
            ),
        ));
    }
    Ok(())
}

fn validate_owner_intent_temporary_prefix(
    evidence: &OwnerTemporaryEvidence,
    authority_key: &str,
    token: &str,
    owner_path: &str,
    stage_path: &str,
    visibility: &OwnerVisibility,
    new_digest: &Sha256Digest,
) -> Result<()> {
    let intent = OwnerIntent {
        schema_version: 1,
        authority_key: authority_key.to_owned(),
        token: token.to_owned(),
        owner_path: owner_path.to_owned(),
        stage_path: stage_path.to_owned(),
        old_digest: visibility.digest.clone(),
        old_identity: visibility.identity.clone(),
        new_digest: new_digest.clone(),
    };
    validate_owner_temporary_prefix(
        evidence,
        &canonical_json(&intent, "serialize recovered owner intent")?,
    )
}

fn revalidate_owner_temporary(
    rooted: &RootedFs,
    active: &str,
    evidence: &OwnerTemporaryEvidence,
) -> Result<()> {
    let path = format!("{active}/{}", evidence.name);
    let bytes = read_owner_cleanup_file(rooted, &path, &evidence.identity)?;
    if bytes != evidence.bytes {
        return Err(transaction_error(
            "recover owner transaction",
            format!(
                "owner temporary changed after validation: {}",
                evidence.name
            ),
        ));
    }
    Ok(())
}

fn existing_owner_outcome(rooted: &RootedFs, active: &str) -> Result<Option<OwnerOutcomeKind>> {
    let committed = rooted.exists(&format!("{active}/committed"))?;
    let aborted = rooted.exists(&format!("{active}/aborted"))?;
    existing_owner_outcome_presence(committed, aborted)
}

fn existing_owner_outcome_from_names(names: &[String]) -> Result<Option<OwnerOutcomeKind>> {
    existing_owner_outcome_presence(
        names.iter().any(|name| name == "committed"),
        names.iter().any(|name| name == "aborted"),
    )
}

fn existing_owner_outcome_presence(
    committed: bool,
    aborted: bool,
) -> Result<Option<OwnerOutcomeKind>> {
    match (committed, aborted) {
        (true, false) => Ok(Some(OwnerOutcomeKind::Committed)),
        (false, true) => Ok(Some(OwnerOutcomeKind::Aborted)),
        (false, false) => Ok(None),
        (true, true) => Err(transaction_error(
            "recover owner transaction",
            "owner journal contains conflicting outcome markers",
        )),
    }
}

fn expected_owner_outcome(
    intent: &OwnerIntent,
    outcome: OwnerOutcomeKind,
    visibility: &OwnerVisibility,
) -> OwnerOutcomeMarker {
    OwnerOutcomeMarker {
        schema_version: 2,
        authority_key: intent.authority_key.clone(),
        token: intent.token.clone(),
        owner_path: intent.owner_path.clone(),
        outcome,
        old_digest: intent.old_digest.clone(),
        old_identity: intent.old_identity.clone(),
        new_digest: intent.new_digest.clone(),
        visible_digest: visibility.digest.clone(),
        visible_identity: visibility.identity.clone(),
    }
}

fn validate_recorded_owner_identity(
    rooted: &RootedFs,
    identity: Option<&HeldIdentity>,
) -> Result<()> {
    if let Some(identity) = identity
        && (identity.kind() != NodeKind::Regular
            || identity.mode() != PRIVATE_FILE_MODE
            || identity.link_count() != Some(1)
            || identity.owner() != rooted.identity().owner()
            || identity.device() != rooted.identity().device()
            || identity.fsid() != rooted.identity().fsid())
    {
        return Err(transaction_error(
            "recover owner transaction",
            "owner outcome identity has the wrong policy",
        ));
    }
    Ok(())
}

fn owner_bindings_match(
    left_digest: Option<&Sha256Digest>,
    left_identity: Option<&HeldIdentity>,
    right_digest: Option<&Sha256Digest>,
    right_identity: Option<&HeldIdentity>,
) -> bool {
    left_digest == right_digest
        && match (left_identity, right_identity) {
            (Some(left), Some(right)) => left.matches_recovery(right),
            (None, None) => true,
            _ => false,
        }
}

fn validate_owner_outcome_binding(
    rooted: &RootedFs,
    marker: &OwnerOutcomeMarker,
    operation: &str,
) -> Result<()> {
    if marker.schema_version != 2
        || marker.old_digest.is_some() != marker.old_identity.is_some()
        || marker.visible_digest.is_some() != marker.visible_identity.is_some()
    {
        return Err(transaction_error(
            operation,
            "owner outcome marker has an unsupported schema or incomplete binding",
        ));
    }
    validate_recorded_owner_identity(rooted, marker.old_identity.as_ref())?;
    validate_recorded_owner_identity(rooted, marker.visible_identity.as_ref())?;
    let binding_matches_outcome = match marker.outcome {
        OwnerOutcomeKind::Aborted => owner_bindings_match(
            marker.old_digest.as_ref(),
            marker.old_identity.as_ref(),
            marker.visible_digest.as_ref(),
            marker.visible_identity.as_ref(),
        ),
        OwnerOutcomeKind::Committed => {
            marker.visible_digest.as_ref() == Some(&marker.new_digest)
                && marker.visible_identity.is_some()
        }
    };
    if !binding_matches_outcome {
        return Err(transaction_error(
            operation,
            "owner outcome marker does not bind the visibility required by its outcome",
        ));
    }
    Ok(())
}

fn validate_owner_visibility(
    rooted: &RootedFs,
    owner_path: &str,
    visibility: &OwnerVisibility,
) -> Result<()> {
    if visibility.digest.is_some() != visibility.identity.is_some() {
        return Err(transaction_error(
            "recover owner transaction",
            "visible owner digest and identity presence differ",
        ));
    }
    validate_recorded_owner_identity(rooted, visibility.identity.as_ref())?;
    let actual_identity = rooted.identity_at(owner_path)?;
    let actual_digest = actual_identity
        .as_ref()
        .map(|_| {
            let bytes = rooted.read_file(owner_path, PRIVATE_FILE_MODE)?;
            validate_owner_record_bytes(
                rooted,
                &bytes,
                "recover owner transaction",
                "visible owner record",
            )?;
            Ok(Sha256Digest::from_bytes(bytes))
        })
        .transpose()?;
    let identity_matches = match (visibility.identity.as_ref(), actual_identity.as_ref()) {
        (Some(expected), Some(actual)) => expected.matches_recovery(actual),
        (None, None) => true,
        _ => false,
    };
    if visibility.digest != actual_digest || !identity_matches {
        return Err(transaction_error(
            "recover owner transaction",
            "visible owner differs from its recorded outcome",
        ));
    }
    Ok(())
}

fn read_owner_visibility_bytes(
    rooted: &RootedFs,
    owner_path: &str,
    visibility: &OwnerVisibility,
) -> Result<Option<Vec<u8>>> {
    match (visibility.digest.as_ref(), visibility.identity.as_ref()) {
        (None, None) => Ok(None),
        (Some(expected_digest), Some(expected_identity)) => {
            let bytes = read_owner_cleanup_file(rooted, owner_path, expected_identity)?;
            validate_owner_record_bytes(
                rooted,
                &bytes,
                "recover owner transaction",
                "visible owner record",
            )?;
            if &Sha256Digest::from_bytes(&bytes) != expected_digest {
                return Err(transaction_error(
                    "recover owner transaction",
                    "visible owner bytes differ from their recovery binding",
                ));
            }
            Ok(Some(bytes))
        }
        _ => Err(transaction_error(
            "recover owner transaction",
            "visible owner digest and identity presence differ",
        )),
    }
}

fn read_owner_outcome(
    rooted: &RootedFs,
    active: &str,
    outcome: OwnerOutcomeKind,
) -> Result<OwnerOutcomeMarker> {
    let name = match outcome {
        OwnerOutcomeKind::Aborted => "aborted",
        OwnerOutcomeKind::Committed => "committed",
    };
    let bytes = rooted.read_file(&format!("{active}/{name}"), PRIVATE_FILE_MODE)?;
    parse_owner_outcome_bytes(&bytes)
}

fn parse_owner_outcome_bytes(bytes: &[u8]) -> Result<OwnerOutcomeMarker> {
    let marker: OwnerOutcomeMarker = serde_json::from_slice(bytes).map_err(|error| {
        transaction_error(
            "recover owner transaction",
            format!("invalid owner outcome marker: {error}"),
        )
    })?;
    validate_canonical_owner_json(
        bytes,
        &marker,
        "recover owner transaction",
        "owner outcome marker",
    )?;
    Ok(marker)
}

fn validate_existing_owner_outcome(
    rooted: &RootedFs,
    active: &str,
    token: &str,
    intent: &OwnerIntent,
    outcome: OwnerOutcomeKind,
    visibility: &OwnerVisibility,
    registration: Option<&HeldIdentity>,
) -> Result<()> {
    let recorded = read_owner_outcome(rooted, active, outcome)?;
    let temporary = format!(
        "{active}/{}-{token}.tmp",
        match outcome {
            OwnerOutcomeKind::Aborted => "aborted",
            OwnerOutcomeKind::Committed => "committed",
        }
    );
    validate_existing_owner_outcome_record(
        rooted,
        intent,
        outcome,
        visibility,
        registration,
        &recorded,
        rooted.exists(&temporary)?,
    )
}

fn validate_existing_owner_outcome_record(
    rooted: &RootedFs,
    intent: &OwnerIntent,
    outcome: OwnerOutcomeKind,
    visibility: &OwnerVisibility,
    registration: Option<&HeldIdentity>,
    recorded: &OwnerOutcomeMarker,
    retains_temporary: bool,
) -> Result<()> {
    if visibility.digest.is_some() != visibility.identity.is_some() {
        return Err(transaction_error(
            "recover owner transaction",
            "visible owner digest and identity presence differ",
        ));
    }
    validate_recorded_owner_identity(rooted, visibility.identity.as_ref())?;
    match outcome {
        OwnerOutcomeKind::Committed => {
            if visibility.digest.as_ref() != Some(&intent.new_digest)
                || visibility.identity.is_none()
                || registration.is_some_and(|expected| {
                    !visibility
                        .identity
                        .as_ref()
                        .is_some_and(|actual| expected.matches_recovery(actual))
                })
            {
                return Err(transaction_error(
                    "recover owner transaction",
                    "committed owner outcome differs from the staged generation",
                ));
            }
        }
        OwnerOutcomeKind::Aborted => {
            let identity_matches = match (&intent.old_identity, visibility.identity.as_ref()) {
                (Some(expected), Some(actual)) => expected.matches_recovery(actual),
                (None, None) => true,
                _ => false,
            };
            if visibility.digest != intent.old_digest || !identity_matches {
                return Err(transaction_error(
                    "recover owner transaction",
                    "aborted owner outcome differs from the historical generation",
                ));
            }
        }
    }
    let expected = expected_owner_outcome(intent, outcome, visibility);
    if recorded != &expected {
        return Err(transaction_error(
            "recover owner transaction",
            "owner outcome marker differs from visible state",
        ));
    }
    if retains_temporary {
        return Err(transaction_error(
            "recover owner transaction",
            "published owner outcome retains its temporary marker",
        ));
    }
    Ok(())
}

fn ensure_owner_outcome(
    rooted: &RootedFs,
    active: &str,
    active_identity: &HeldIdentity,
    token: &str,
    intent: &OwnerIntent,
    outcome: OwnerOutcomeKind,
    visibility: &OwnerVisibility,
) -> Result<()> {
    validate_owner_cleanup_journal_identity(rooted, active, active_identity)?;
    validate_owner_visibility(rooted, &intent.owner_path, visibility)?;
    if let Some(existing) = existing_owner_outcome(rooted, active)? {
        if existing != outcome {
            return Err(transaction_error(
                "recover owner transaction",
                "owner outcome marker conflicts with visible state",
            ));
        }
        return validate_existing_owner_outcome(
            rooted, active, token, intent, outcome, visibility, None,
        );
    }
    let name = match outcome {
        OwnerOutcomeKind::Aborted => "aborted",
        OwnerOutcomeKind::Committed => "committed",
    };
    let opposite_temporary = format!(
        "{active}/{}-{token}.tmp",
        match outcome {
            OwnerOutcomeKind::Aborted => "committed",
            OwnerOutcomeKind::Committed => "aborted",
        }
    );
    if rooted.exists(&opposite_temporary)? {
        return Err(transaction_error(
            "recover owner transaction",
            "owner outcome temporary conflicts with visible state",
        ));
    }
    let temporary_name = format!("{name}-{token}.tmp");
    let temporary = format!("{active}/{temporary_name}");
    let marker = expected_owner_outcome(intent, outcome, visibility);
    let marker_bytes = canonical_json(&marker, "serialize recovered owner outcome")?;
    if rooted.identity_at(&temporary)?.is_some() {
        let evidence = read_owner_temporary(rooted, active, &temporary_name)?;
        validate_owner_temporary_prefix(&evidence, &marker_bytes)?;
        validate_owner_cleanup_journal_identity(rooted, active, active_identity)?;
        revalidate_owner_temporary(rooted, active, &evidence)?;
        rooted.remove_file_exact(&temporary, &evidence.identity)?;
    }
    validate_owner_cleanup_journal_identity(rooted, active, active_identity)?;
    validate_owner_visibility(rooted, &intent.owner_path, visibility)?;
    rooted.publish_file_exclusive(
        active,
        name,
        &temporary_name,
        &marker_bytes,
        PRIVATE_FILE_MODE,
    )?;
    Ok(())
}

fn publish_recovered_owner_outcome(
    rooted: &RootedFs,
    active: &str,
    token: &str,
    intent: &OwnerIntent,
    outcome: OwnerOutcomeKind,
    visibility: &OwnerVisibility,
    plan: &mut OwnerRecoveryPlan,
) -> Result<()> {
    plan.revalidate(rooted, active)?;
    let name = match outcome {
        OwnerOutcomeKind::Aborted => "aborted",
        OwnerOutcomeKind::Committed => "committed",
    };
    let opposite = match outcome {
        OwnerOutcomeKind::Aborted => "committed",
        OwnerOutcomeKind::Committed => "aborted",
    };
    let temporary_name = format!("{name}-{token}.tmp");
    let opposite_temporary = format!("{opposite}-{token}.tmp");
    if plan.member_optional(name).is_some()
        || plan.member_optional(opposite).is_some()
        || plan.member_optional(&temporary_name).is_some()
        || plan.member_optional(&opposite_temporary).is_some()
    {
        return Err(transaction_error(
            "recover owner transaction",
            "owner recovery plan conflicts with outcome publication",
        ));
    }
    let marker = expected_owner_outcome(intent, outcome, visibility);
    let marker_bytes = canonical_json(&marker, "serialize recovered owner outcome")?;
    let identity = rooted.publish_file_exclusive(
        active,
        name,
        &temporary_name,
        &marker_bytes,
        PRIVATE_FILE_MODE,
    )?;
    plan.add_member(name, identity, marker_bytes)
}

fn validate_standalone_owner_outcome(
    rooted: &RootedFs,
    domain: Domain,
    authority_key: &str,
    token: &str,
    active: &str,
    name: &str,
) -> Result<()> {
    let outcome = match name {
        "aborted" => OwnerOutcomeKind::Aborted,
        "committed" => OwnerOutcomeKind::Committed,
        _ => {
            return Err(transaction_error(
                "recover owner transaction",
                "owner cleanup marker has an unknown name",
            ));
        }
    };
    let marker = read_owner_outcome(rooted, active, outcome)?;
    validate_owner_outcome_binding(rooted, &marker, "recover owner transaction")?;
    if marker.authority_key != authority_key
        || marker.token != token
        || marker.owner_path != owner_path(domain)
        || marker.outcome != outcome
    {
        return Err(transaction_error(
            "recover owner transaction",
            "standalone owner outcome marker is not canonical",
        ));
    }
    let actual = read_owner_visibility(rooted, &marker.owner_path)?;
    let identity_matches = match (marker.visible_identity.as_ref(), actual.identity.as_ref()) {
        (Some(expected), Some(actual)) => expected.matches_recovery(actual),
        (None, None) => true,
        _ => false,
    };
    if marker.visible_digest != actual.digest || !identity_matches {
        return Err(transaction_error(
            "recover owner transaction",
            "standalone owner outcome differs from visible owner",
        ));
    }
    Ok(())
}

#[derive(Clone, Debug)]
struct OwnerCleanupMember {
    name: String,
    identity: HeldIdentity,
    bytes: Vec<u8>,
}

#[derive(Clone, Debug)]
struct OwnerCleanupPlan {
    outcome: OwnerOutcomeKind,
    marker: OwnerOutcomeMarker,
    visible_bytes: Option<Vec<u8>>,
    removal_order: Vec<OwnerCleanupMember>,
}

#[derive(Clone, Copy)]
struct OwnerCleanupRevalidation<'a> {
    domain: Domain,
    authority_key: &'a str,
    token: &'a str,
    journal: &'a str,
    journal_identity: &'a HeldIdentity,
    outcome: OwnerOutcomeKind,
    marker: &'a OwnerOutcomeMarker,
    visible_bytes: &'a Option<Vec<u8>>,
}

#[cfg(test)]
struct OwnerCleanupControl<'a> {
    before_unlink: &'a mut dyn FnMut(&str) -> Result<()>,
}

#[cfg(test)]
impl<'a> OwnerCleanupControl<'a> {
    fn new(before_unlink: &'a mut dyn FnMut(&str) -> Result<()>) -> Self {
        Self { before_unlink }
    }
}

fn cleanup_owner_journal(
    rooted: &RootedFs,
    domain: Domain,
    authority_key: &str,
    token: &str,
    journal: &str,
    journal_identity: HeldIdentity,
) -> Result<()> {
    #[cfg(test)]
    let result = cleanup_owner_journal_inner(
        rooted,
        domain,
        authority_key,
        token,
        journal,
        journal_identity,
        None,
    );
    #[cfg(not(test))]
    let result = cleanup_owner_journal_inner(
        rooted,
        domain,
        authority_key,
        token,
        journal,
        journal_identity,
    );
    result
}

#[cfg(test)]
fn cleanup_owner_journal_controlled(
    rooted: &RootedFs,
    domain: Domain,
    authority_key: &str,
    token: &str,
    journal: &str,
    journal_identity: HeldIdentity,
    control: &mut OwnerCleanupControl<'_>,
) -> Result<()> {
    cleanup_owner_journal_inner(
        rooted,
        domain,
        authority_key,
        token,
        journal,
        journal_identity,
        Some(control),
    )
}

fn cleanup_owner_journal_inner(
    rooted: &RootedFs,
    domain: Domain,
    authority_key: &str,
    token: &str,
    journal: &str,
    journal_identity: HeldIdentity,
    #[cfg(test)] mut control: Option<&mut OwnerCleanupControl<'_>>,
) -> Result<()> {
    #[cfg(test)]
    let _observation_phase = rooted.begin_observation_phase(DurabilityPhase::OwnerCleanup);
    let plan = validate_owner_cleanup(
        rooted,
        domain,
        authority_key,
        token,
        journal,
        &journal_identity,
    )?;
    for (index, member) in plan.removal_order.iter().enumerate() {
        #[cfg(test)]
        if let Some(control) = control.as_deref_mut() {
            (control.before_unlink)(&member.name)?;
        }
        revalidate_owner_cleanup_before_unlink(
            rooted,
            OwnerCleanupRevalidation {
                domain,
                authority_key,
                token,
                journal,
                journal_identity: &journal_identity,
                outcome: plan.outcome,
                marker: &plan.marker,
                visible_bytes: &plan.visible_bytes,
            },
            &plan.removal_order[index..],
        )?;
        rooted.remove_file_exact(&format!("{journal}/{}", member.name), &member.identity)?;
    }
    #[cfg(test)]
    if let Some(control) = control {
        (control.before_unlink)("journal-directory")?;
    }
    revalidate_owner_cleanup_before_unlink(
        rooted,
        OwnerCleanupRevalidation {
            domain,
            authority_key,
            token,
            journal,
            journal_identity: &journal_identity,
            outcome: plan.outcome,
            marker: &plan.marker,
            visible_bytes: &plan.visible_bytes,
        },
        &[],
    )?;
    rooted.remove_dir_exact(journal, &journal_identity)
}

fn validate_owner_cleanup(
    rooted: &RootedFs,
    domain: Domain,
    authority_key: &str,
    token: &str,
    journal: &str,
    journal_identity: &HeldIdentity,
) -> Result<OwnerCleanupPlan> {
    validate_token(token)?;
    let expected_journal = format!(
        ".surgeist-generator/leases/{}/{OWNER_TRANSACTIONS}/active-{token}",
        domain.as_str()
    );
    if journal != expected_journal {
        return Err(transaction_error(
            "clean private journal",
            "owner cleanup journal path differs from its domain and token",
        ));
    }
    validate_owner_cleanup_journal_identity(rooted, journal, journal_identity)?;

    let names = rooted.list_dir(journal)?;
    let outcome = match (
        names.iter().any(|name| name == "aborted"),
        names.iter().any(|name| name == "committed"),
    ) {
        (true, false) => OwnerOutcomeKind::Aborted,
        (false, true) => OwnerOutcomeKind::Committed,
        (false, false) => {
            return Err(transaction_error(
                "clean private journal",
                "owner journal has no durable outcome marker",
            ));
        }
        (true, true) => {
            return Err(transaction_error(
                "clean private journal",
                "owner journal contains conflicting outcome markers",
            ));
        }
    };
    let outcome_name = match outcome {
        OwnerOutcomeKind::Aborted => "aborted",
        OwnerOutcomeKind::Committed => "committed",
    };
    let has_intent = names.iter().any(|name| name == "intent.json");
    let has_registration = names.iter().any(|name| name == "stage-registration.json");
    let has_prepared = names.iter().any(|name| name == "prepared.json");

    for name in &names {
        if !matches!(
            name.as_str(),
            "intent.json" | "stage-registration.json" | "prepared.json" | "aborted" | "committed"
        ) {
            return Err(transaction_error(
                "clean private journal",
                format!("owner journal contains an unexpected cleanup member: {name}"),
            ));
        }
    }
    if !has_intent && (names.len() != 1 || names[0] != outcome_name) {
        return Err(transaction_error(
            "clean private journal",
            "standalone owner outcome retains other cleanup members",
        ));
    }
    if has_prepared && !has_registration {
        return Err(transaction_error(
            "clean private journal",
            "owner prepared marker has no stage registration",
        ));
    }

    let mut members = Vec::with_capacity(names.len());
    for name in &names {
        let path = format!("{journal}/{name}");
        let identity = rooted.identity_at(&path)?.ok_or_else(|| {
            transaction_error(
                "clean private journal",
                format!("owner cleanup member disappeared: {name}"),
            )
        })?;
        validate_owner_cleanup_member_identity(rooted, name, &identity)?;
        let bytes = read_owner_cleanup_file(rooted, &path, &identity)?;
        members.push(OwnerCleanupMember {
            name: name.clone(),
            identity,
            bytes,
        });
    }

    let marker_member = owner_cleanup_member(&members, outcome_name)?;
    let marker: OwnerOutcomeMarker =
        serde_json::from_slice(&marker_member.bytes).map_err(|error| {
            transaction_error(
                "clean private journal",
                format!("invalid owner cleanup outcome marker: {error}"),
            )
        })?;
    validate_owner_cleanup_marker(rooted, domain, authority_key, token, outcome, &marker)?;
    validate_canonical_owner_json(
        &marker_member.bytes,
        &marker,
        "clean private journal",
        "owner cleanup outcome marker",
    )?;
    let marker_visibility = OwnerVisibility {
        digest: marker.visible_digest.clone(),
        identity: marker.visible_identity.clone(),
    };
    let visible_bytes =
        read_owner_visibility_bytes(rooted, &marker.owner_path, &marker_visibility)?;

    if has_intent {
        validate_owner_cleanup_intent(
            rooted,
            authority_key,
            token,
            journal,
            outcome,
            &marker,
            &members,
        )?;
    }

    validate_owner_cleanup_journal_identity(rooted, journal, journal_identity)?;
    if rooted.list_dir(journal)? != names {
        return Err(transaction_error(
            "clean private journal",
            "owner cleanup inventory changed during validation",
        ));
    }
    revalidate_owner_cleanup_before_unlink(
        rooted,
        OwnerCleanupRevalidation {
            domain,
            authority_key,
            token,
            journal,
            journal_identity,
            outcome,
            marker: &marker,
            visible_bytes: &visible_bytes,
        },
        &members,
    )?;

    let mut removal_names = Vec::new();
    for name in ["prepared.json", "stage-registration.json", "intent.json"] {
        if names.iter().any(|member| member == name) {
            removal_names.push(name.to_owned());
        }
    }
    removal_names.push(outcome_name.to_owned());
    if removal_names.len() != names.len()
        || removal_names.iter().collect::<BTreeSet<_>>().len() != names.len()
    {
        return Err(transaction_error(
            "clean private journal",
            "owner journal contains an unclassified cleanup member",
        ));
    }
    let removal_order = removal_names
        .into_iter()
        .map(|name| owner_cleanup_member(&members, &name).cloned())
        .collect::<Result<Vec<_>>>()?;
    Ok(OwnerCleanupPlan {
        outcome,
        marker,
        visible_bytes,
        removal_order,
    })
}

fn revalidate_owner_cleanup_before_unlink(
    rooted: &RootedFs,
    context: OwnerCleanupRevalidation<'_>,
    remaining: &[OwnerCleanupMember],
) -> Result<()> {
    let OwnerCleanupRevalidation {
        domain,
        authority_key,
        token,
        journal,
        journal_identity,
        outcome,
        marker,
        visible_bytes,
    } = context;
    validate_owner_cleanup_journal_identity(rooted, journal, journal_identity)?;
    let mut expected_names = remaining
        .iter()
        .map(|member| member.name.clone())
        .collect::<Vec<_>>();
    expected_names.sort();
    if rooted.list_dir(journal)? != expected_names {
        return Err(transaction_error(
            "clean private journal",
            "owner cleanup inventory changed before unlink",
        ));
    }
    validate_owner_cleanup_marker(rooted, domain, authority_key, token, outcome, marker)?;
    let marker_visibility = OwnerVisibility {
        digest: marker.visible_digest.clone(),
        identity: marker.visible_identity.clone(),
    };
    if &read_owner_visibility_bytes(rooted, &marker.owner_path, &marker_visibility)?
        != visible_bytes
    {
        return Err(transaction_error(
            "clean private journal",
            "visible owner bytes changed before owner cleanup unlink",
        ));
    }
    for member in remaining {
        let path = format!("{journal}/{}", member.name);
        let bytes = read_owner_cleanup_file(rooted, &path, &member.identity)?;
        if bytes != member.bytes {
            return Err(transaction_error(
                "clean private journal",
                format!(
                    "owner cleanup member bytes changed before unlink: {}",
                    member.name
                ),
            ));
        }
    }
    Ok(())
}

fn validate_owner_cleanup_journal_identity(
    rooted: &RootedFs,
    journal: &str,
    expected: &HeldIdentity,
) -> Result<()> {
    let actual = rooted.identity_at(journal)?.ok_or_else(|| {
        transaction_error("clean private journal", "owner cleanup journal disappeared")
    })?;
    if expected.kind() != NodeKind::Directory
        || expected.mode() != PRIVATE_DIRECTORY_MODE
        || expected.link_count().is_some()
        || expected.owner() != rooted.identity().owner()
        || expected.device() != rooted.identity().device()
        || expected.fsid() != rooted.identity().fsid()
        || !expected.matches_recovery(&actual)
    {
        return Err(transaction_error(
            "clean private journal",
            "owner cleanup journal identity or policy changed",
        ));
    }
    Ok(())
}

fn validate_owner_cleanup_member_identity(
    rooted: &RootedFs,
    name: &str,
    identity: &HeldIdentity,
) -> Result<()> {
    if identity.kind() != NodeKind::Regular
        || identity.mode() != PRIVATE_FILE_MODE
        || identity.link_count() != Some(1)
        || identity.owner() != rooted.identity().owner()
        || identity.device() != rooted.identity().device()
        || identity.fsid() != rooted.identity().fsid()
    {
        return Err(transaction_error(
            "clean private journal",
            format!("owner cleanup member has the wrong identity or policy: {name}"),
        ));
    }
    Ok(())
}

fn owner_cleanup_member<'a>(
    members: &'a [OwnerCleanupMember],
    name: &str,
) -> Result<&'a OwnerCleanupMember> {
    members
        .iter()
        .find(|member| member.name == name)
        .ok_or_else(|| {
            transaction_error(
                "clean private journal",
                format!("owner cleanup member disappeared: {name}"),
            )
        })
}

fn read_owner_cleanup_file(
    rooted: &RootedFs,
    path: &str,
    expected: &HeldIdentity,
) -> Result<Vec<u8>> {
    let mut file = rooted.open_file_handle(path, PRIVATE_FILE_MODE, false)?;
    let opened = rooted.identity_of_handle(&file)?;
    if !expected.matches_recovery(&opened) {
        return Err(transaction_error(
            "clean private journal",
            format!("owner cleanup member changed before read: {path}"),
        ));
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|source| {
        transaction_source("read owner cleanup member", path.to_owned(), source)
    })?;
    let after = rooted.identity_of_handle(&file)?;
    let named = rooted.identity_at(path)?.ok_or_else(|| {
        transaction_error(
            "clean private journal",
            format!("owner cleanup member disappeared after read: {path}"),
        )
    })?;
    if !expected.matches_recovery(&after) || !expected.matches_recovery(&named) {
        return Err(transaction_error(
            "clean private journal",
            format!("owner cleanup member changed during read: {path}"),
        ));
    }
    Ok(bytes)
}

fn validate_owner_cleanup_marker(
    rooted: &RootedFs,
    domain: Domain,
    authority_key: &str,
    token: &str,
    outcome: OwnerOutcomeKind,
    marker: &OwnerOutcomeMarker,
) -> Result<()> {
    validate_owner_outcome_binding(rooted, marker, "clean private journal")?;
    if marker.authority_key != authority_key
        || marker.token != token
        || marker.owner_path != owner_path(domain)
        || marker.outcome != outcome
    {
        return Err(transaction_error(
            "clean private journal",
            "owner cleanup outcome marker is not canonical",
        ));
    }
    match (&marker.visible_digest, &marker.visible_identity) {
        (None, None) => {
            if rooted.identity_at(&marker.owner_path)?.is_some() {
                return Err(transaction_error(
                    "clean private journal",
                    "owner cleanup outcome records absence but an owner is visible",
                ));
            }
        }
        (Some(expected_digest), Some(expected_identity)) => {
            let actual_identity = rooted.identity_at(&marker.owner_path)?.ok_or_else(|| {
                transaction_error(
                    "clean private journal",
                    "owner cleanup outcome records a missing visible owner",
                )
            })?;
            if !expected_identity.matches_recovery(&actual_identity) {
                return Err(transaction_error(
                    "clean private journal",
                    "visible owner identity differs from the cleanup outcome",
                ));
            }
            let bytes = read_owner_cleanup_file(rooted, &marker.owner_path, expected_identity)?;
            validate_owner_record_bytes(
                rooted,
                &bytes,
                "clean private journal",
                "visible owner record",
            )?;
            if &Sha256Digest::from_bytes(bytes) != expected_digest {
                return Err(transaction_error(
                    "clean private journal",
                    "visible owner bytes differ from the cleanup outcome",
                ));
            }
        }
        _ => {
            return Err(transaction_error(
                "clean private journal",
                "visible owner digest and identity presence differ",
            ));
        }
    }
    Ok(())
}

fn validate_owner_cleanup_intent(
    rooted: &RootedFs,
    authority_key: &str,
    token: &str,
    journal: &str,
    outcome: OwnerOutcomeKind,
    marker: &OwnerOutcomeMarker,
    members: &[OwnerCleanupMember],
) -> Result<()> {
    let intent_member = owner_cleanup_member(members, "intent.json")?;
    let intent: OwnerIntent = serde_json::from_slice(&intent_member.bytes).map_err(|error| {
        transaction_error(
            "clean private journal",
            format!("invalid owner cleanup intent: {error}"),
        )
    })?;
    if intent.schema_version != 1
        || intent.authority_key != authority_key
        || intent.token != token
        || intent.owner_path != marker.owner_path
        || intent.stage_path != format!("{journal}/owner.stage")
        || intent.old_digest.is_some() != intent.old_identity.is_some()
    {
        return Err(transaction_error(
            "clean private journal",
            "owner cleanup intent authority, token, or fixed paths differ",
        ));
    }
    validate_canonical_owner_json(
        &intent_member.bytes,
        &intent,
        "clean private journal",
        "owner cleanup intent",
    )?;
    validate_recorded_owner_identity(rooted, intent.old_identity.as_ref())?;
    let expected_visibility = match outcome {
        OwnerOutcomeKind::Aborted => OwnerVisibility {
            digest: intent.old_digest.clone(),
            identity: intent.old_identity.clone(),
        },
        OwnerOutcomeKind::Committed => OwnerVisibility {
            digest: marker.visible_digest.clone(),
            identity: marker.visible_identity.clone(),
        },
    };
    let expected_marker = expected_owner_outcome(&intent, outcome, &expected_visibility);
    if marker != &expected_marker {
        return Err(transaction_error(
            "clean private journal",
            "owner cleanup outcome marker differs from its intent",
        ));
    }

    let registration = if let Some(member) = members
        .iter()
        .find(|member| member.name == "stage-registration.json")
    {
        let registration: HeldIdentity =
            serde_json::from_slice(&member.bytes).map_err(|error| {
                transaction_error(
                    "clean private journal",
                    format!("invalid owner cleanup stage registration: {error}"),
                )
            })?;
        validate_recorded_owner_identity(rooted, Some(&registration))?;
        validate_canonical_owner_json(
            &member.bytes,
            &registration,
            "clean private journal",
            "owner cleanup stage registration",
        )?;
        Some(registration)
    } else {
        None
    };
    if outcome == OwnerOutcomeKind::Committed
        && registration.as_ref().is_some_and(|expected| {
            !marker
                .visible_identity
                .as_ref()
                .is_some_and(|actual| expected.matches_recovery(actual))
        })
    {
        return Err(transaction_error(
            "clean private journal",
            "committed owner cleanup differs from its stage registration",
        ));
    }

    if let Some(member) = members.iter().find(|member| member.name == "prepared.json") {
        let prepared: Sha256Digest = serde_json::from_slice(&member.bytes).map_err(|error| {
            transaction_error(
                "clean private journal",
                format!("invalid owner cleanup prepared marker: {error}"),
            )
        })?;
        if registration.is_none() || prepared != intent.new_digest {
            return Err(transaction_error(
                "clean private journal",
                "owner cleanup prepared marker differs from its registration",
            ));
        }
        validate_canonical_owner_json(
            &member.bytes,
            &prepared,
            "clean private journal",
            "owner cleanup prepared marker",
        )?;
    }
    Ok(())
}

fn corpus_authority_key(rooted: &RootedFs, domain: Domain) -> String {
    Sha256Digest::from_bytes(format!(
        "surgeist-corpus-authority-v1\0{}\0{}\0{}\0{}\0{}",
        rooted.canonical_root().display(),
        rooted.identity().device(),
        rooted.identity().inode(),
        rooted.identity().fsid(),
        domain.as_str()
    ))
    .to_string()
}

fn mutex_path(domain: Domain) -> String {
    format!(".surgeist-generator/leases/{}/mutex.lock", domain.as_str())
}

fn owner_path(domain: Domain) -> String {
    format!(
        ".surgeist-generator/leases/{}/{}",
        domain.as_str(),
        OWNER_RECORD
    )
}

pub(crate) fn new_token() -> Result<String> {
    MutationTarget::current().require_supported("generate transaction token")?;
    let mut random = File::open("/dev/urandom").map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::UnsupportedPlatform,
            "open transaction randomness source",
            "/dev/urandom",
            source,
        )
    })?;
    let mut bytes = [0_u8; 16];
    random.read_exact(&mut bytes).map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::UnsupportedPlatform,
            "read transaction randomness source",
            "/dev/urandom",
            source,
        )
    })?;
    let mut token = String::with_capacity(32);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut token, "{byte:02x}")
            .map_err(|error| transaction_error("format transaction token", error.to_string()))?;
    }
    Ok(token)
}

fn validate_token(token: &str) -> Result<()> {
    if token.len() != 32
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(transaction_error(
            "validate coordination token",
            "token must be 32 lowercase hexadecimal bytes",
        ));
    }
    Ok(())
}

fn parse_pid(value: &str) -> Result<u32> {
    let pid = value
        .parse::<u32>()
        .map_err(|_| transaction_error("parse bootstrap PID", format!("invalid PID: {value}")))?;
    if pid == 0 {
        return Err(transaction_error("parse bootstrap PID", "PID is zero"));
    }
    Ok(pid)
}

fn validate_identifier(value: &str, label: &str) -> Result<()> {
    if !super::validate_identifier(value) {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidInventory,
            "validate generation lease metadata",
            format!("invalid {label}"),
        ));
    }
    Ok(())
}

fn canonical_json<T: Serialize>(value: &T, operation: &str) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(value).map_err(|error| {
        transaction_error(operation, format!("JSON serialization failed: {error}"))
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

fn validate_canonical_owner_json<T: Serialize>(
    actual: &[u8],
    value: &T,
    operation: &str,
    label: &str,
) -> Result<()> {
    if actual != canonical_json(value, operation)? {
        return Err(transaction_error(
            operation,
            format!("{label} bytes are not canonical JSON"),
        ));
    }
    Ok(())
}

fn verification_from(error: GeneratorError) -> GeneratorError {
    match error.kind() {
        GeneratorErrorKind::LeaseActive | GeneratorErrorKind::Io => error,
        _ => verification_error("inspect generation coordination", error.to_string()),
    }
}

fn verification_error(operation: &str, detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(GeneratorErrorKind::Verification, operation, detail)
}

fn transaction_error(operation: &str, detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(GeneratorErrorKind::ArtifactTransaction, operation, detail)
}

fn transaction_source(
    operation: &str,
    detail: impl Into<String>,
    source: std::io::Error,
) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::ArtifactTransaction,
        operation,
        detail,
        source,
    )
}

#[cfg(test)]
mod tests {
    use super::{BootstrapProtocol, BootstrapStep, Domain, LOCK_HEADER};

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    use std::collections::{BTreeMap, BTreeSet};
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    use std::fs::{self, File};
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    use std::os::unix::fs::PermissionsExt;
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    use std::panic::{AssertUnwindSafe, catch_unwind};
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    use std::path::{Path, PathBuf};
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    use std::sync::atomic::{AtomicU64, Ordering};

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    use crate::core::fs::{
        DurabilityEvent, DurabilityPhase, DurabilityPrimitive, HeldIdentity,
        PRIVATE_DIRECTORY_MODE, PRIVATE_FILE_MODE, RootedFs, RootedObserver,
    };
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    use crate::{CorpusLocation, GeneratorError, GeneratorErrorKind, Result, Sha256Digest};

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    use super::{
        ACQUISITION_LOCK, BOOTSTRAP_LOCKS, BootstrapInstallControl, BootstrapRecoveryControl,
        COORDINATION_ROOT, CoordinationAccess, CoordinationGuard, LeaseMetadata,
        OWNER_TRANSACTIONS, OwnerCleanupControl, OwnerOutcomeKind, OwnerOutcomeMarker, OwnerRecord,
        OwnerRecordStamp, OwnerRecoveryControl, ProbeCapabilityFault, ProbeInstallControl,
        ProbeInstallMember, ProbeIntent, ProbeRecoveryControl, ProbeRecoveryPlan,
        acquire_exclusive, acquire_shared_check, canonical_json, cleanup_owner_journal,
        cleanup_owner_journal_controlled, corpus_authority_key, finish_probe_capability_failure,
        injected_probe_capability_error, install_owner_record_bytes,
        install_owner_record_controlled, mutex_path, open_existing_lock, open_or_bootstrap_lock,
        open_or_bootstrap_lock_controlled, owner_path, process_is_live,
        recover_bootstrap_controlled, recover_owner_transactions,
        recover_owner_transactions_controlled, recover_probe_journals,
        recover_probe_journals_controlled, run_rename_probe, run_rename_probe_controlled,
    };

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    static NEXT_BOOTSTRAP_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    const SYNTHETIC_ABANDONED_PID: u32 = u32::MAX;
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    const INSTALL_TOKEN: &str = "11111111111111111111111111111111";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    const CLAIM_TOKEN_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    const CLAIM_TOKEN_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    const CLAIM_TOKEN_C: &str = "cccccccccccccccccccccccccccccccc";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    const LATER_TOKEN: &str = "99999999999999999999999999999999";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    const OWNER_INSTALL_TOKEN: &str = "22222222222222222222222222222222";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    const OWNER_PID: u32 = 4242;
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    const OWNER_START_TIME: u64 = 1_700_000_000;

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[derive(Clone, Debug, Eq, PartialEq)]
    enum SnapshotEntry {
        Directory(u32),
        Regular(u32, Vec<u8>),
        Symlink(u32, PathBuf),
        Other(u32),
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    struct BootstrapFixture {
        owner: PathBuf,
        corpus: PathBuf,
        location: CorpusLocation,
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    impl BootstrapFixture {
        fn new(label: &str) -> Self {
            let sequence = NEXT_BOOTSTRAP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let owner = std::env::temp_dir().join(format!(
                "surgeist-generator-bootstrap-{label}-{}-{sequence}",
                std::process::id()
            ));
            let corpus = owner.join("corpus");
            fs::create_dir(&owner).expect("create bootstrap fixture owner");
            fs::create_dir(&corpus).expect("create bootstrap fixture corpus");
            let location =
                CorpusLocation::new(&owner, &corpus).expect("bootstrap fixture location");
            let rooted = RootedFs::open_corpus(&location).expect("open bootstrap fixture");
            rooted
                .ensure_dir(COORDINATION_ROOT, PRIVATE_DIRECTORY_MODE)
                .expect("create coordination root");
            rooted
                .ensure_dir(".surgeist-generator/bootstrap", PRIVATE_DIRECTORY_MODE)
                .expect("create bootstrap root");
            rooted
                .ensure_dir(BOOTSTRAP_LOCKS, PRIVATE_DIRECTORY_MODE)
                .expect("create bootstrap locks root");
            Self {
                owner,
                corpus,
                location,
            }
        }

        fn rooted(&self) -> RootedFs {
            RootedFs::open_corpus(&self.location).expect("open fresh bootstrap authority")
        }

        fn snapshot(&self) -> BTreeMap<PathBuf, SnapshotEntry> {
            snapshot(&self.corpus)
        }

        fn lock_identity(&self) -> Option<HeldIdentity> {
            self.rooted()
                .identity_at(ACQUISITION_LOCK)
                .expect("inspect immutable bootstrap lock")
        }

        fn assert_lock(&self, expected: Option<&HeldIdentity>) {
            let rooted = self.rooted();
            let actual = rooted
                .identity_at(ACQUISITION_LOCK)
                .expect("inspect immutable bootstrap lock");
            match (expected, actual) {
                (None, None) => {}
                (Some(expected), Some(actual)) => {
                    assert!(
                        expected.matches_recovery(&actual),
                        "immutable bootstrap winner identity changed"
                    );
                    assert_eq!(
                        rooted
                            .read_file(ACQUISITION_LOCK, PRIVATE_FILE_MODE)
                            .expect("read immutable bootstrap lock"),
                        LOCK_HEADER
                    );
                }
                (None, Some(_)) => panic!("bootstrap lock published before its commit boundary"),
                (Some(_), None) => panic!("committed bootstrap lock disappeared"),
            }
        }

        fn assert_clean(&self, expected: Option<&HeldIdentity>) {
            self.assert_lock(expected);
            let mut clean = BTreeMap::from([
                (
                    PathBuf::from(COORDINATION_ROOT),
                    SnapshotEntry::Directory(PRIVATE_DIRECTORY_MODE),
                ),
                (
                    PathBuf::from(".surgeist-generator/bootstrap"),
                    SnapshotEntry::Directory(PRIVATE_DIRECTORY_MODE),
                ),
                (
                    PathBuf::from(BOOTSTRAP_LOCKS),
                    SnapshotEntry::Directory(PRIVATE_DIRECTORY_MODE),
                ),
            ]);
            if expected.is_some() {
                clean.insert(
                    PathBuf::from(ACQUISITION_LOCK),
                    SnapshotEntry::Regular(PRIVATE_FILE_MODE, LOCK_HEADER.to_vec()),
                );
            }
            assert_eq!(self.snapshot(), clean, "bootstrap residue was not cleaned");
        }

        fn later_acquire_same_lock(&self) -> HeldIdentity {
            let before = self.lock_identity();
            let rooted = self.rooted();
            let lock = open_or_bootstrap_lock(
                &rooted,
                ACQUISITION_LOCK,
                "acquisition",
                LATER_TOKEN,
                CoordinationAccess::Exclusive,
            )
            .expect("later production acquisition");
            let acquired = rooted
                .identity_of_handle(&lock)
                .expect("inspect later acquisition handle");
            if let Some(before) = before {
                assert!(
                    before.matches_recovery(&acquired),
                    "later acquisition replaced the immutable lock"
                );
            }
            drop(lock);
            self.assert_clean(Some(&acquired));
            acquired
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    impl Drop for BootstrapFixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.owner).expect("remove bootstrap fixture");
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    struct SeededWinner {
        identity: HeldIdentity,
        handle: Option<File>,
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    impl SeededWinner {
        fn assert_held(&self, fixture: &BootstrapFixture) {
            assert!(self.handle.is_some(), "winner fixture is not held");
            fixture.assert_lock(Some(&self.identity));
            let error = open_existing_lock(
                &fixture.rooted(),
                ACQUISITION_LOCK,
                CoordinationAccess::Exclusive,
                false,
            )
            .expect_err("independent winner must remain held");
            assert_eq!(error.kind(), GeneratorErrorKind::LeaseActive);
        }

        fn release(mut self) -> HeldIdentity {
            drop(self.handle.take());
            self.identity
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn mode(metadata: &fs::Metadata) -> u32 {
        metadata.permissions().mode() & 0o7777
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn snapshot(root: &Path) -> BTreeMap<PathBuf, SnapshotEntry> {
        fn visit(root: &Path, directory: &Path, entries: &mut BTreeMap<PathBuf, SnapshotEntry>) {
            let mut children: Vec<_> = fs::read_dir(directory)
                .expect("snapshot bootstrap directory")
                .map(|entry| entry.expect("snapshot bootstrap entry"))
                .collect();
            children.sort_by_key(|entry| entry.file_name());
            for child in children {
                let path = child.path();
                let relative = path
                    .strip_prefix(root)
                    .expect("bootstrap snapshot relative path")
                    .to_path_buf();
                let metadata = fs::symlink_metadata(&path).expect("bootstrap entry metadata");
                let entry = if metadata.is_dir() {
                    SnapshotEntry::Directory(mode(&metadata))
                } else if metadata.is_file() {
                    SnapshotEntry::Regular(
                        mode(&metadata),
                        fs::read(&path).expect("read bootstrap snapshot file"),
                    )
                } else if metadata.file_type().is_symlink() {
                    SnapshotEntry::Symlink(
                        mode(&metadata),
                        fs::read_link(&path).expect("read bootstrap snapshot symlink"),
                    )
                } else {
                    SnapshotEntry::Other(mode(&metadata))
                };
                let is_directory = metadata.is_dir();
                entries.insert(relative, entry);
                if is_directory {
                    visit(root, &path, entries);
                }
            }
        }

        let mut entries = BTreeMap::new();
        visit(root, root, &mut entries);
        entries
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn active_path(owner_pid: u32) -> String {
        format!("{BOOTSTRAP_LOCKS}/active-{owner_pid}-{INSTALL_TOKEN}")
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn stage_path(owner_pid: u32) -> String {
        format!("{}/lock.stage", active_path(owner_pid))
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn claim_path(claim_token: &str) -> String {
        format!(
            "{BOOTSTRAP_LOCKS}/recovering-{SYNTHETIC_ABANDONED_PID}-{INSTALL_TOKEN}-by-{SYNTHETIC_ABANDONED_PID}-{claim_token}"
        )
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn run_install(
        fixture: &BootstrapFixture,
        observer: RootedObserver,
        creator_pid: u32,
        before_final_rename: Option<&mut dyn FnMut() -> Result<()>>,
    ) -> Result<File> {
        let rooted = RootedFs::open_corpus_observed(&fixture.location, observer)?;
        let mut control = BootstrapInstallControl::new(creator_pid, before_final_rename);
        open_or_bootstrap_lock_controlled(
            &rooted,
            ACQUISITION_LOCK,
            "acquisition",
            INSTALL_TOKEN,
            CoordinationAccess::Exclusive,
            &mut control,
        )
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn recover_fixture(
        fixture: &BootstrapFixture,
        observer: Option<RootedObserver>,
        claim_token: &'static str,
        abandoned_pid: u32,
    ) -> Result<()> {
        let rooted = if let Some(observer) = observer {
            RootedFs::open_corpus_observed(&fixture.location, observer)?
        } else {
            RootedFs::open_corpus(&fixture.location)?
        };
        let mut liveness = |pid| {
            if pid == abandoned_pid {
                Ok(false)
            } else {
                process_is_live(pid)
            }
        };
        let mut control =
            BootstrapRecoveryControl::new(SYNTHETIC_ABANDONED_PID, claim_token, &mut liveness);
        recover_bootstrap_controlled(&rooted, &mut control)
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn expect_interruption<T: std::fmt::Debug>(operation: impl FnOnce() -> Result<T>) {
        let payload = catch_unwind(AssertUnwindSafe(operation))
            .expect_err("observed bootstrap operation must interrupt");
        assert!(RootedObserver::is_interruption(payload.as_ref()));
        assert!(payload.downcast_ref::<GeneratorError>().is_none());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn assert_event_prefix(
        observer: &RootedObserver,
        trace: &[DurabilityEvent],
        event_index: usize,
        label: &str,
    ) {
        assert_eq!(
            observer.events(),
            trace[..=event_index],
            "{label} trace changed at prefix {event_index}"
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn event_indices(
        events: &[DurabilityEvent],
        phase: DurabilityPhase,
        primitive: DurabilityPrimitive,
        path: &str,
    ) -> Vec<usize> {
        events
            .iter()
            .enumerate()
            .filter(|(_, event)| {
                event.phase() == phase && event.primitive() == primitive && event.path() == path
            })
            .map(|(index, _)| index)
            .collect()
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn one_event_index(
        events: &[DurabilityEvent],
        phase: DurabilityPhase,
        primitive: DurabilityPrimitive,
        path: &str,
    ) -> usize {
        let matches = event_indices(events, phase, primitive, path);
        assert_eq!(
            matches.len(),
            1,
            "expected one {primitive:?} event for {path}"
        );
        matches[0]
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn assert_event_exists(
        events: &[DurabilityEvent],
        phase: DurabilityPhase,
        primitive: DurabilityPrimitive,
        path: &str,
    ) {
        assert!(
            events.iter().any(|event| {
                event.phase() == phase && event.primitive() == primitive && event.path() == path
            }),
            "missing {phase:?} {primitive:?} event for {path}"
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn header_write_indices(events: &[DurabilityEvent]) -> Vec<usize> {
        let stage = stage_path(SYNTHETIC_ABANDONED_PID);
        let writes: Vec<_> = events
            .iter()
            .enumerate()
            .filter(|(_, event)| {
                event.phase() == DurabilityPhase::BootstrapInstall
                    && event.path() == stage
                    && matches!(
                        event.primitive(),
                        DurabilityPrimitive::WritePartial | DurabilityPrimitive::WriteFull
                    )
            })
            .map(|(index, _)| index)
            .collect();
        assert_eq!(writes.len(), LOCK_HEADER.len());
        for index in &writes[..writes.len() - 1] {
            assert_eq!(
                events[*index].primitive(),
                DurabilityPrimitive::WritePartial
            );
        }
        assert_eq!(
            events[*writes.last().expect("complete header write")].primitive(),
            DurabilityPrimitive::WriteFull
        );
        writes
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn record_uncontended_trace() -> Vec<DurabilityEvent> {
        let fixture = BootstrapFixture::new("uncontended-trace");
        let observer = RootedObserver::recording();
        let lock = run_install(&fixture, observer.clone(), SYNTHETIC_ABANDONED_PID, None)
            .expect("trace uncontended bootstrap");
        let identity = fixture
            .rooted()
            .identity_of_handle(&lock)
            .expect("inspect traced bootstrap lock");
        drop(lock);
        fixture.assert_clean(Some(&identity));
        let events = observer.events();
        assert!(!events.is_empty(), "uncontended bootstrap trace is empty");
        events
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn interrupt_uncontended_install(
        fixture: &BootstrapFixture,
        trace: &[DurabilityEvent],
        event_index: usize,
    ) {
        let observer = RootedObserver::interrupt_after(event_index);
        expect_interruption(|| {
            run_install(fixture, observer.clone(), SYNTHETIC_ABANDONED_PID, None)
        });
        assert_event_prefix(&observer, trace, event_index, "uncontended install");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn publish_winner(fixture: &BootstrapFixture, held: bool) -> Result<SeededWinner> {
        let rooted = fixture.rooted();
        let identity = rooted.publish_file_exclusive(
            COORDINATION_ROOT,
            "acquisition.lock",
            "winner-publication.tmp",
            LOCK_HEADER,
            PRIVATE_FILE_MODE,
        )?;
        let handle = if held {
            Some(open_existing_lock(
                &rooted,
                ACQUISITION_LOCK,
                CoordinationAccess::Exclusive,
                false,
            )?)
        } else {
            None
        };
        Ok(SeededWinner { identity, handle })
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn run_winner_install(
        fixture: &BootstrapFixture,
        observer: RootedObserver,
        held: bool,
        winner: &mut Option<SeededWinner>,
    ) -> Result<File> {
        let mut before_final_rename = || {
            assert!(winner.is_none(), "winner hook invoked more than once");
            *winner = Some(publish_winner(fixture, held)?);
            Ok(())
        };
        run_install(
            fixture,
            observer,
            SYNTHETIC_ABANDONED_PID,
            Some(&mut before_final_rename),
        )
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn record_winner_trace(held: bool) -> (Vec<DurabilityEvent>, HeldIdentity) {
        let fixture = BootstrapFixture::new(if held {
            "winner-held-trace"
        } else {
            "winner-released-trace"
        });
        let observer = RootedObserver::recording();
        let mut winner = None;
        let result = run_winner_install(&fixture, observer.clone(), held, &mut winner);
        let mut winner = winner.expect("winner hook must publish before local rename");
        let winner_identity = winner.identity.clone();
        if held {
            let error = result.expect_err("held winner must reject local bootstrap");
            assert_eq!(error.kind(), GeneratorErrorKind::LeaseActive);
            winner.assert_held(&fixture);
        } else {
            let adopted = result.expect("released winner must be adopted");
            let adopted_identity = fixture
                .rooted()
                .identity_of_handle(&adopted)
                .expect("inspect adopted winner");
            assert!(winner_identity.matches_recovery(&adopted_identity));
            let error = open_existing_lock(
                &fixture.rooted(),
                ACQUISITION_LOCK,
                CoordinationAccess::Exclusive,
                false,
            )
            .expect_err("adopted winner must remain exclusively held");
            assert_eq!(error.kind(), GeneratorErrorKind::LeaseActive);
            drop(adopted);
        }
        fixture.assert_clean(Some(&winner_identity));
        drop(winner.handle.take());
        let later = fixture.later_acquire_same_lock();
        assert!(winner_identity.matches_recovery(&later));
        let events = observer.events();
        assert!(!events.is_empty(), "winner bootstrap trace is empty");
        (events, winner_identity)
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn interrupt_winner_install(
        fixture: &BootstrapFixture,
        trace: &[DurabilityEvent],
        event_index: usize,
        held: bool,
    ) -> Option<SeededWinner> {
        let observer = RootedObserver::interrupt_after(event_index);
        let mut winner = None;
        expect_interruption(|| run_winner_install(fixture, observer.clone(), held, &mut winner));
        assert_event_prefix(
            &observer,
            trace,
            event_index,
            if held {
                "held-winner install"
            } else {
                "released-winner install"
            },
        );
        winner
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn recover_and_assert_idempotent(fixture: &BootstrapFixture, expected: Option<&HeldIdentity>) {
        recover_fixture(fixture, None, CLAIM_TOKEN_B, SYNTHETIC_ABANDONED_PID)
            .expect("fresh production bootstrap recovery");
        fixture.assert_clean(expected);
        let stable = fixture.snapshot();
        recover_fixture(fixture, None, CLAIM_TOKEN_C, SYNTHETIC_ABANDONED_PID)
            .expect("repeat production bootstrap recovery");
        fixture.assert_clean(expected);
        assert_eq!(fixture.snapshot(), stable, "repeat recovery changed state");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn assert_cleanup_inventory(events: &[DurabilityEvent], journal: &str) {
        let receipt = format!("{journal}/cleanup-started");
        let member_prefix = format!("{journal}/");
        assert_event_exists(
            events,
            DurabilityPhase::BootstrapCleanup,
            DurabilityPrimitive::RenameExclusive,
            &receipt,
        );
        assert_event_exists(
            events,
            DurabilityPhase::BootstrapCleanup,
            DurabilityPrimitive::RemoveFile,
            &receipt,
        );
        assert!(
            events.iter().any(|event| {
                event.phase() == DurabilityPhase::BootstrapCleanup
                    && event.primitive() == DurabilityPrimitive::RemoveFile
                    && event.path().starts_with(&member_prefix)
                    && event.path() != receipt
            }),
            "cleanup trace omitted receipt-bound member removal for {journal}"
        );
        assert_event_exists(
            events,
            DurabilityPhase::BootstrapCleanup,
            DurabilityPrimitive::RemoveDirectory,
            journal,
        );
        assert_event_exists(
            events,
            DurabilityPhase::BootstrapCleanup,
            DurabilityPrimitive::SyncDirectory,
            BOOTSTRAP_LOCKS,
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn record_recovery_trace(
        install_trace: &[DurabilityEvent],
        install_event: usize,
        expected_lock: bool,
    ) -> Vec<DurabilityEvent> {
        let fixture = BootstrapFixture::new("recovery-trace");
        interrupt_uncontended_install(&fixture, install_trace, install_event);
        let observer = RootedObserver::recording();
        recover_fixture(
            &fixture,
            Some(observer.clone()),
            CLAIM_TOKEN_A,
            SYNTHETIC_ABANDONED_PID,
        )
        .expect("trace production bootstrap recovery");
        let identity = fixture.lock_identity();
        assert_eq!(identity.is_some(), expected_lock);
        fixture.assert_clean(identity.as_ref());
        let events = observer.events();
        assert!(!events.is_empty(), "bootstrap recovery trace is empty");
        events
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn exercise_uncontended_recovery_prefixes(
        install_trace: &[DurabilityEvent],
        install_event: usize,
        expected_lock: bool,
    ) {
        let recovery_trace = record_recovery_trace(install_trace, install_event, expected_lock);
        let claim = claim_path(CLAIM_TOKEN_A);
        assert_event_exists(
            &recovery_trace,
            DurabilityPhase::BootstrapRecovery,
            DurabilityPrimitive::RenameExclusive,
            &claim,
        );
        assert_cleanup_inventory(&recovery_trace, &claim);
        for event_index in 0..recovery_trace.len() {
            let fixture = BootstrapFixture::new("recovery-prefix");
            interrupt_uncontended_install(&fixture, install_trace, install_event);
            let observer = RootedObserver::interrupt_after(event_index);
            expect_interruption(|| {
                recover_fixture(
                    &fixture,
                    Some(observer.clone()),
                    CLAIM_TOKEN_A,
                    SYNTHETIC_ABANDONED_PID,
                )
            });
            assert_event_prefix(
                &observer,
                &recovery_trace,
                event_index,
                "uncontended recovery",
            );
            recover_fixture(&fixture, None, CLAIM_TOKEN_B, SYNTHETIC_ABANDONED_PID)
                .expect("complete interrupted uncontended recovery");
            let after_recovery = fixture.lock_identity();
            assert_eq!(after_recovery.is_some(), expected_lock);
            fixture.assert_clean(after_recovery.as_ref());
            let stable = fixture.snapshot();
            recover_fixture(&fixture, None, CLAIM_TOKEN_C, SYNTHETIC_ABANDONED_PID)
                .expect("repeat completed uncontended recovery");
            fixture.assert_clean(after_recovery.as_ref());
            assert_eq!(fixture.snapshot(), stable, "repeat recovery changed state");
            let later = fixture.later_acquire_same_lock();
            if let Some(after_recovery) = after_recovery {
                assert!(after_recovery.matches_recovery(&later));
            }
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn record_winner_recovery_trace(
        install_trace: &[DurabilityEvent],
        install_event: usize,
        held: bool,
    ) -> Vec<DurabilityEvent> {
        let fixture = BootstrapFixture::new(if held {
            "held-recovery-trace"
        } else {
            "released-recovery-trace"
        });
        let winner = interrupt_winner_install(&fixture, install_trace, install_event, held)
            .expect("winner must exist at recovery seed");
        if held {
            winner.assert_held(&fixture);
        }
        let observer = RootedObserver::recording();
        recover_fixture(
            &fixture,
            Some(observer.clone()),
            CLAIM_TOKEN_A,
            SYNTHETIC_ABANDONED_PID,
        )
        .expect("trace winner bootstrap recovery");
        fixture.assert_clean(Some(&winner.identity));
        if held {
            winner.assert_held(&fixture);
        }
        let identity = winner.release();
        let later = fixture.later_acquire_same_lock();
        assert!(identity.matches_recovery(&later));
        let events = observer.events();
        assert!(!events.is_empty(), "winner recovery trace is empty");
        events
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn exercise_winner_recovery_prefixes(
        install_trace: &[DurabilityEvent],
        install_event: usize,
        held: bool,
    ) {
        let recovery_trace = record_winner_recovery_trace(install_trace, install_event, held);
        let claim = claim_path(CLAIM_TOKEN_A);
        assert_event_exists(
            &recovery_trace,
            DurabilityPhase::BootstrapRecovery,
            DurabilityPrimitive::RenameExclusive,
            &claim,
        );
        assert_cleanup_inventory(&recovery_trace, &claim);
        for event_index in 0..recovery_trace.len() {
            let fixture = BootstrapFixture::new(if held {
                "held-recovery-prefix"
            } else {
                "released-recovery-prefix"
            });
            let winner = interrupt_winner_install(&fixture, install_trace, install_event, held)
                .expect("winner must exist at recovery prefix seed");
            if held {
                winner.assert_held(&fixture);
            }
            let observer = RootedObserver::interrupt_after(event_index);
            expect_interruption(|| {
                recover_fixture(
                    &fixture,
                    Some(observer.clone()),
                    CLAIM_TOKEN_A,
                    SYNTHETIC_ABANDONED_PID,
                )
            });
            assert_event_prefix(
                &observer,
                &recovery_trace,
                event_index,
                if held {
                    "held-winner recovery"
                } else {
                    "released-winner recovery"
                },
            );
            if held {
                winner.assert_held(&fixture);
            }
            recover_and_assert_idempotent(&fixture, Some(&winner.identity));
            if held {
                winner.assert_held(&fixture);
            }
            let identity = winner.release();
            let later = fixture.later_acquire_same_lock();
            assert!(identity.matches_recovery(&later));
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum InitialOwner {
        Absent,
        Old,
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[derive(Clone, Copy, Debug)]
    struct OwnerRecoverySeed {
        initial: InitialOwner,
        committed: bool,
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    struct OwnerFixture {
        owner: PathBuf,
        corpus: PathBuf,
        location: CorpusLocation,
        initial: InitialOwner,
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    impl OwnerFixture {
        fn new(initial: InitialOwner) -> Self {
            let sequence = NEXT_BOOTSTRAP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let owner = std::env::temp_dir().join(format!(
                "surgeist-generator-owner-{}-{sequence:016x}",
                std::process::id()
            ));
            let corpus = owner.join("corpus");
            fs::create_dir(&owner).expect("create owner fixture root");
            fs::create_dir(&corpus).expect("create owner fixture corpus");
            let location = CorpusLocation::new(&owner, &corpus).expect("owner fixture location");
            let fixture = Self {
                owner,
                corpus,
                location,
                initial,
            };
            let rooted = fixture.rooted();
            rooted
                .ensure_dir(COORDINATION_ROOT, PRIVATE_DIRECTORY_MODE)
                .expect("create owner coordination root");
            rooted
                .ensure_dir(".surgeist-generator/leases", PRIVATE_DIRECTORY_MODE)
                .expect("create owner leases root");
            rooted
                .ensure_dir(".surgeist-generator/leases/layout", PRIVATE_DIRECTORY_MODE)
                .expect("create owner domain root");
            rooted
                .ensure_dir(&fixture.transaction_parent(), PRIVATE_DIRECTORY_MODE)
                .expect("create owner transaction root");
            if initial == InitialOwner::Old {
                let bytes = fixture.record_bytes(&old_owner_metadata());
                rooted
                    .publish_file_exclusive(
                        ".surgeist-generator/leases/layout",
                        "owner.json",
                        "old-owner.tmp",
                        &bytes,
                        PRIVATE_FILE_MODE,
                    )
                    .expect("seed historical owner");
            }
            fixture
        }

        fn rooted(&self) -> RootedFs {
            RootedFs::open_corpus(&self.location).expect("open fresh owner authority")
        }

        fn observed_rooted(&self, observer: RootedObserver) -> RootedFs {
            RootedFs::open_corpus_observed(&self.location, observer)
                .expect("open observed owner authority")
        }

        fn transaction_parent(&self) -> String {
            format!(".surgeist-generator/leases/layout/{OWNER_TRANSACTIONS}")
        }

        fn active_path(&self) -> String {
            format!("{}/active-{OWNER_INSTALL_TOKEN}", self.transaction_parent())
        }

        fn record(&self, metadata: &LeaseMetadata) -> OwnerRecord {
            OwnerRecord {
                schema_version: 1,
                generator: metadata.generator.clone(),
                pid: OWNER_PID,
                owner_root: self.location.owner_root().display().to_string(),
                corpus_root: self.location.corpus_root().display().to_string(),
                scope: metadata.scope.clone(),
                command: metadata.command.clone(),
                unix_start_time: OWNER_START_TIME,
            }
        }

        fn record_bytes(&self, metadata: &LeaseMetadata) -> Vec<u8> {
            canonical_json(&self.record(metadata), "serialize owner fixture record")
                .expect("serialize owner fixture record")
        }

        fn expected_initial_bytes(&self) -> Option<Vec<u8>> {
            match self.initial {
                InitialOwner::Absent => None,
                InitialOwner::Old => Some(self.record_bytes(&old_owner_metadata())),
            }
        }

        fn expected_new_bytes(&self) -> Vec<u8> {
            self.record_bytes(&new_owner_metadata())
        }

        fn visible_owner(&self) -> Option<Vec<u8>> {
            let rooted = self.rooted();
            let path = owner_path(Domain::Layout);
            if rooted.exists(&path).expect("inspect owner visibility") {
                Some(
                    rooted
                        .read_file(&path, PRIVATE_FILE_MODE)
                        .expect("read visible owner"),
                )
            } else {
                None
            }
        }

        fn visible_owner_identity(&self) -> Option<HeldIdentity> {
            self.rooted()
                .identity_at(&owner_path(Domain::Layout))
                .expect("inspect visible owner identity")
        }

        fn install(&self, observer: RootedObserver) -> Result<()> {
            let rooted = self.observed_rooted(observer);
            let authority_key = corpus_authority_key(&rooted, Domain::Layout);
            install_owner_record_controlled(
                &rooted,
                &self.location,
                Domain::Layout,
                &new_owner_metadata(),
                OWNER_INSTALL_TOKEN,
                &authority_key,
                OwnerRecordStamp {
                    pid: OWNER_PID,
                    unix_start_time: OWNER_START_TIME,
                },
            )
        }

        fn install_bytes(&self, observer: RootedObserver, owner_bytes: &[u8]) -> Result<()> {
            let rooted = self.observed_rooted(observer);
            let authority_key = corpus_authority_key(&rooted, Domain::Layout);
            install_owner_record_bytes(
                &rooted,
                Domain::Layout,
                OWNER_INSTALL_TOKEN,
                &authority_key,
                owner_bytes,
            )
        }

        fn recover(&self, observer: Option<RootedObserver>) -> Result<()> {
            let rooted = if let Some(observer) = observer {
                self.observed_rooted(observer)
            } else {
                self.rooted()
            };
            let authority_key = corpus_authority_key(&rooted, Domain::Layout);
            recover_owner_transactions(&rooted, Domain::Layout, &authority_key)
        }

        fn assert_visibility(&self, expected: Option<&[u8]>) {
            assert_eq!(
                self.visible_owner().as_deref(),
                expected,
                "owner visibility differs"
            );
        }

        fn assert_clean(&self, expected: Option<&[u8]>) {
            self.assert_visibility(expected);
            assert!(
                self.rooted()
                    .list_dir(&self.transaction_parent())
                    .expect("list owner transaction residue")
                    .is_empty(),
                "owner transaction residue remains"
            );
        }

        fn snapshot(&self) -> BTreeMap<PathBuf, SnapshotEntry> {
            snapshot(&self.corpus)
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    impl Drop for OwnerFixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.owner).expect("remove owner fixture");
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn old_owner_metadata() -> LeaseMetadata {
        LeaseMetadata {
            generator: "layout-generator".to_owned(),
            scope: "full".to_owned(),
            command: "historical-generate".to_owned(),
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn new_owner_metadata() -> LeaseMetadata {
        LeaseMetadata {
            generator: "layout-generator".to_owned(),
            scope: "filtered:cases/new.html".to_owned(),
            command: "replacement-generate".to_owned(),
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn owner_commit_index(events: &[DurabilityEvent], initial: InitialOwner) -> usize {
        one_event_index(
            events,
            DurabilityPhase::OwnerInstall,
            match initial {
                InitialOwner::Absent => DurabilityPrimitive::RenameExclusive,
                InitialOwner::Old => DurabilityPrimitive::RenameSwap,
            },
            &owner_path(Domain::Layout),
        )
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn owner_marker_index(events: &[DurabilityEvent], name: &str) -> usize {
        one_event_index(
            events,
            DurabilityPhase::OwnerInstall,
            DurabilityPrimitive::RenameExclusive,
            &format!(
                ".surgeist-generator/leases/layout/{OWNER_TRANSACTIONS}/active-{OWNER_INSTALL_TOKEN}/{name}"
            ),
        )
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn assert_owner_marker_publication(
        events: &[DurabilityEvent],
        phase: DurabilityPhase,
        active: &str,
        marker: &str,
        temporary: &str,
    ) {
        let temporary = format!("{active}/{temporary}");
        for primitive in [
            DurabilityPrimitive::CreateFile,
            DurabilityPrimitive::WritePartial,
            DurabilityPrimitive::WriteFull,
            DurabilityPrimitive::SetPermissions,
            DurabilityPrimitive::FlushFile,
            DurabilityPrimitive::SyncFile,
            DurabilityPrimitive::ValidateIdentity,
        ] {
            assert_event_exists(events, phase, primitive, &temporary);
        }
        assert_event_exists(
            events,
            phase,
            DurabilityPrimitive::RenameExclusive,
            &format!("{active}/{marker}"),
        );
        assert_event_exists(events, phase, DurabilityPrimitive::SyncDirectory, active);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn assert_owner_install_trace(
        events: &[DurabilityEvent],
        initial: InitialOwner,
        owner_bytes: &[u8],
    ) {
        let parent = format!(".surgeist-generator/leases/layout/{OWNER_TRANSACTIONS}");
        let active = format!("{parent}/active-{OWNER_INSTALL_TOKEN}");
        let stage = format!("{active}/owner.stage");
        assert_event_exists(
            events,
            DurabilityPhase::OwnerInstall,
            DurabilityPrimitive::CreateDirectory,
            &active,
        );
        for (marker, temporary) in [
            ("intent.json", format!("intent-{OWNER_INSTALL_TOKEN}.tmp")),
            (
                "stage-registration.json",
                format!("stage-registration-{OWNER_INSTALL_TOKEN}.tmp"),
            ),
            (
                "prepared.json",
                format!("prepared-{OWNER_INSTALL_TOKEN}.tmp"),
            ),
            ("committed", format!("committed-{OWNER_INSTALL_TOKEN}.tmp")),
        ] {
            assert_owner_marker_publication(
                events,
                DurabilityPhase::OwnerInstall,
                &active,
                marker,
                &temporary,
            );
        }
        for primitive in [
            DurabilityPrimitive::CreateFile,
            DurabilityPrimitive::WritePartial,
            DurabilityPrimitive::WriteFull,
            DurabilityPrimitive::FlushFile,
            DurabilityPrimitive::SyncFile,
            DurabilityPrimitive::ValidateIdentity,
            DurabilityPrimitive::DropHandle,
        ] {
            assert_event_exists(events, DurabilityPhase::OwnerInstall, primitive, &stage);
        }
        let stage_writes: Vec<_> = events
            .iter()
            .filter(|event| {
                event.phase() == DurabilityPhase::OwnerInstall
                    && event.path() == stage
                    && matches!(
                        event.primitive(),
                        DurabilityPrimitive::WritePartial | DurabilityPrimitive::WriteFull
                    )
            })
            .collect();
        assert_eq!(
            stage_writes.len(),
            owner_bytes.len(),
            "owner stage trace omitted a byte prefix"
        );
        assert!(
            stage_writes[..stage_writes.len() - 1]
                .iter()
                .all(|event| event.primitive() == DurabilityPrimitive::WritePartial)
        );
        assert_eq!(
            stage_writes
                .last()
                .expect("complete owner stage write")
                .primitive(),
            DurabilityPrimitive::WriteFull
        );
        assert_event_exists(
            events,
            DurabilityPhase::OwnerInstall,
            match initial {
                InitialOwner::Absent => DurabilityPrimitive::RenameExclusive,
                InitialOwner::Old => DurabilityPrimitive::RenameSwap,
            },
            &owner_path(Domain::Layout),
        );
        assert_event_exists(
            events,
            DurabilityPhase::OwnerInstall,
            DurabilityPrimitive::SyncDirectory,
            ".surgeist-generator/leases/layout",
        );
        if initial == InitialOwner::Old {
            assert_event_exists(
                events,
                DurabilityPhase::OwnerInstall,
                DurabilityPrimitive::RemoveFile,
                &stage,
            );
        }
        assert!(
            events.iter().any(|event| {
                event.phase() == DurabilityPhase::OwnerCleanup
                    && event.primitive() == DurabilityPrimitive::RemoveFile
                    && event.path().starts_with(&format!("{active}/"))
            }),
            "owner cleanup omitted individual journal-member removal"
        );
        assert_event_exists(
            events,
            DurabilityPhase::OwnerCleanup,
            DurabilityPrimitive::RemoveDirectory,
            &active,
        );
        assert_event_exists(
            events,
            DurabilityPhase::OwnerCleanup,
            DurabilityPrimitive::SyncDirectory,
            &parent,
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn record_owner_install_trace(initial: InitialOwner) -> Vec<DurabilityEvent> {
        let fixture = OwnerFixture::new(initial);
        let observer = RootedObserver::recording();
        fixture
            .install(observer.clone())
            .expect("record production owner install");
        let expected = fixture.expected_new_bytes();
        fixture.assert_clean(Some(&expected));
        let events = observer.events();
        assert!(!events.is_empty(), "owner install trace is empty");
        assert_owner_install_trace(&events, initial, &expected);
        events
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn interrupt_owner_install(
        fixture: &OwnerFixture,
        trace: &[DurabilityEvent],
        event_index: usize,
    ) {
        let observer = RootedObserver::interrupt_after(event_index);
        expect_interruption(|| fixture.install(observer.clone()));
        assert_event_prefix(&observer, trace, event_index, "owner install");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn recover_owner_and_assert_idempotent(fixture: &OwnerFixture, expected: Option<&[u8]>) {
        fixture
            .recover(None)
            .expect("complete production owner recovery");
        fixture.assert_clean(expected);
        let stable = fixture.snapshot();
        fixture
            .recover(None)
            .expect("repeat production owner recovery");
        fixture.assert_clean(expected);
        assert_eq!(
            fixture.snapshot(),
            stable,
            "repeat owner recovery changed state"
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn exercise_owner_install_prefixes(initial: InitialOwner) {
        let trace = record_owner_install_trace(initial);
        let commit = owner_commit_index(&trace, initial);
        for event_index in 0..trace.len() {
            let fixture = OwnerFixture::new(initial);
            interrupt_owner_install(&fixture, &trace, event_index);
            let expected = if event_index >= commit {
                Some(fixture.expected_new_bytes())
            } else {
                fixture.expected_initial_bytes()
            };
            fixture.assert_visibility(expected.as_deref());
            recover_owner_and_assert_idempotent(&fixture, expected.as_deref());
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn seed_owner_recovery(
        fixture: &OwnerFixture,
        install_trace: &[DurabilityEvent],
        committed: bool,
    ) {
        let commit = owner_commit_index(install_trace, fixture.initial);
        let seed_index = if committed {
            commit
        } else {
            commit
                .checked_sub(1)
                .expect("owner commit has a predecessor")
        };
        interrupt_owner_install(fixture, install_trace, seed_index);
        let expected = if committed {
            Some(fixture.expected_new_bytes())
        } else {
            fixture.expected_initial_bytes()
        };
        fixture.assert_visibility(expected.as_deref());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn owner_cleanup_intent_removal_index(events: &[DurabilityEvent], active: &str) -> usize {
        one_event_index(
            events,
            DurabilityPhase::OwnerCleanup,
            DurabilityPrimitive::RemoveFile,
            &format!("{active}/intent.json"),
        )
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn interrupt_owner_recovery(
        fixture: &OwnerFixture,
        trace: &[DurabilityEvent],
        event_index: usize,
    ) {
        let observer = RootedObserver::interrupt_after(event_index);
        expect_interruption(|| fixture.recover(Some(observer.clone())));
        assert_event_prefix(&observer, trace, event_index, "owner recovery");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn seed_standalone_aborted_owner_outcome(
        fixture: &OwnerFixture,
        install_trace: &[DurabilityEvent],
        recovery_trace: &[DurabilityEvent],
    ) {
        seed_owner_recovery(fixture, install_trace, false);
        let event_index =
            owner_cleanup_intent_removal_index(recovery_trace, &fixture.active_path());
        interrupt_owner_recovery(fixture, recovery_trace, event_index);
        assert_eq!(
            fixture
                .rooted()
                .list_dir(&fixture.active_path())
                .expect("inspect standalone aborted owner outcome"),
            vec!["aborted".to_owned()]
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn seed_standalone_committed_owner_outcome(
        fixture: &OwnerFixture,
        install_trace: &[DurabilityEvent],
    ) {
        let event_index = owner_cleanup_intent_removal_index(install_trace, &fixture.active_path());
        interrupt_owner_install(fixture, install_trace, event_index);
        assert_eq!(
            fixture
                .rooted()
                .list_dir(&fixture.active_path())
                .expect("inspect standalone committed owner outcome"),
            vec!["committed".to_owned()]
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn replace_standalone_committed_with_aborted(fixture: &OwnerFixture) {
        let active = fixture.active_path();
        let committed = fixture.corpus.join(format!("{active}/committed"));
        let mut marker: OwnerOutcomeMarker =
            serde_json::from_slice(&fs::read(&committed).expect("read committed owner outcome"))
                .expect("parse committed owner outcome");
        marker.outcome = OwnerOutcomeKind::Aborted;
        fs::remove_file(&committed).expect("remove committed owner outcome");
        corrupt_file(
            &fixture.corpus.join(format!("{active}/aborted")),
            &canonical_json(&marker, "serialize forged standalone aborted owner outcome")
                .expect("serialize forged standalone aborted owner outcome"),
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn recovery_visibility(fixture: &OwnerFixture, committed: bool) -> Option<Vec<u8>> {
        if committed {
            Some(fixture.expected_new_bytes())
        } else {
            fixture.expected_initial_bytes()
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn record_owner_recovery_trace(
        seed: OwnerRecoverySeed,
        install_trace: &[DurabilityEvent],
    ) -> Vec<DurabilityEvent> {
        let fixture = OwnerFixture::new(seed.initial);
        seed_owner_recovery(&fixture, install_trace, seed.committed);
        let observer = RootedObserver::recording();
        fixture
            .recover(Some(observer.clone()))
            .expect("record production owner recovery");
        let expected = recovery_visibility(&fixture, seed.committed);
        fixture.assert_clean(expected.as_deref());
        let events = observer.events();
        assert!(!events.is_empty(), "owner recovery trace is empty");
        let active = fixture.active_path();
        let outcome_name = if seed.committed {
            "committed"
        } else {
            "aborted"
        };
        assert_owner_marker_publication(
            &events,
            DurabilityPhase::OwnerRecovery,
            &active,
            outcome_name,
            &format!("{outcome_name}-{OWNER_INSTALL_TOKEN}.tmp"),
        );
        if seed.initial == InitialOwner::Old || !seed.committed {
            assert_event_exists(
                &events,
                DurabilityPhase::OwnerRecovery,
                DurabilityPrimitive::RemoveFile,
                &format!("{active}/owner.stage"),
            );
        }
        assert!(
            events.iter().any(|event| {
                event.phase() == DurabilityPhase::OwnerCleanup
                    && event.primitive() == DurabilityPrimitive::RemoveFile
                    && event.path().starts_with(&format!("{active}/"))
            }),
            "owner recovery omitted journal-member cleanup"
        );
        assert_event_exists(
            &events,
            DurabilityPhase::OwnerCleanup,
            DurabilityPrimitive::RemoveDirectory,
            &active,
        );
        events
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn exercise_owner_recovery_prefixes(
        seed: OwnerRecoverySeed,
        install_trace: &[DurabilityEvent],
    ) {
        let recovery_trace = record_owner_recovery_trace(seed, install_trace);
        for event_index in 0..recovery_trace.len() {
            let fixture = OwnerFixture::new(seed.initial);
            seed_owner_recovery(&fixture, install_trace, seed.committed);
            let observer = RootedObserver::interrupt_after(event_index);
            expect_interruption(|| fixture.recover(Some(observer.clone())));
            assert_event_prefix(&observer, &recovery_trace, event_index, "owner recovery");
            let expected = recovery_visibility(&fixture, seed.committed);
            fixture.assert_visibility(expected.as_deref());
            recover_owner_and_assert_idempotent(&fixture, expected.as_deref());
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn corrupt_file(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).expect("write owner corruption");
        let mut permissions = fs::metadata(path)
            .expect("inspect corrupted owner file")
            .permissions();
        permissions.set_mode(PRIVATE_FILE_MODE);
        fs::set_permissions(path, permissions).expect("restore private owner mode");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[derive(Clone, Copy, Debug)]
    enum OwnerJsonNearMiss {
        MissingFinalNewline,
        ReorderedFields,
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn owner_json_near_miss(bytes: &[u8], near_miss: OwnerJsonNearMiss) -> Vec<u8> {
        let value: serde_json::Value =
            serde_json::from_slice(bytes).expect("parse canonical owner protocol record");
        let mut changed = match near_miss {
            OwnerJsonNearMiss::MissingFinalNewline => {
                let mut changed = bytes.to_vec();
                assert_eq!(changed.pop(), Some(b'\n'), "canonical record ends in LF");
                changed
            }
            OwnerJsonNearMiss::ReorderedFields => {
                assert!(
                    value.is_object(),
                    "field reordering requires an object record"
                );
                let mut changed =
                    serde_json::to_vec(&value).expect("serialize reordered owner protocol record");
                changed.push(b'\n');
                changed
            }
        };
        assert_ne!(changed, bytes, "near miss must change canonical bytes");
        let changed_value: serde_json::Value =
            serde_json::from_slice(&changed).expect("near miss remains valid JSON");
        assert_eq!(changed_value, value, "near miss remains semantically equal");
        changed.shrink_to_fit();
        changed
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn interrupt_at_complete_owner_intent_temporary(
        fixture: &OwnerFixture,
        trace: &[DurabilityEvent],
    ) -> String {
        let temporary = format!("{}/intent-{OWNER_INSTALL_TOKEN}.tmp", fixture.active_path());
        let event_index = *event_indices(
            trace,
            DurabilityPhase::OwnerInstall,
            DurabilityPrimitive::WritePartial,
            &temporary,
        )
        .last()
        .expect("owner intent publication has a complete-JSON prefix before final LF");
        interrupt_owner_install(fixture, trace, event_index);
        temporary
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn assert_owner_corruption_preserved(fixture: &OwnerFixture, label: &str) {
        let visible = fixture.visible_owner();
        let before = fixture.snapshot();
        let error = fixture
            .recover(None)
            .expect_err("corrupt owner state must fail recovery");
        assert_eq!(
            error.kind(),
            GeneratorErrorKind::ArtifactTransaction,
            "{label}"
        );
        assert_eq!(
            fixture.visible_owner(),
            visible,
            "{label} changed visible owner"
        );
        assert_eq!(
            fixture.snapshot(),
            before,
            "{label} did not preserve evidence"
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn assert_owner_recovery_race_preserved(
        fixture: &OwnerFixture,
        result: Result<()>,
        raced_snapshot: Option<BTreeMap<PathBuf, SnapshotEntry>>,
        label: &str,
    ) {
        let error = result.expect_err("owner recovery race must stop recovery");
        assert_eq!(
            error.kind(),
            GeneratorErrorKind::ArtifactTransaction,
            "{label}"
        );
        assert_eq!(
            fixture.snapshot(),
            raced_snapshot.expect("owner recovery race callback captured its state"),
            "owner recovery mutated evidence after {label}"
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn seed_owner_intent_without_prepared(fixture: &OwnerFixture, trace: &[DurabilityEvent]) {
        interrupt_owner_install(fixture, trace, owner_marker_index(trace, "intent.json"));
        let active = fixture.active_path();
        let names = fixture
            .rooted()
            .list_dir(&active)
            .expect("inspect intent-present owner journal");
        for required in ["intent.json", "owner.stage", "stage-registration.json"] {
            assert!(
                names.iter().any(|name| name == required),
                "intent-present owner journal omitted {required}"
            );
        }
        assert!(
            !names.iter().any(|name| name == "prepared.json"),
            "owner intent seed unexpectedly includes a prepared marker"
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn replace_owner_journal_identity(fixture: &OwnerFixture) {
        let active = fixture.corpus.join(fixture.active_path());
        let displaced = fixture.corpus.join(format!(
            "{}/displaced-owner-journal",
            fixture.transaction_parent()
        ));
        fs::rename(&active, &displaced).expect("displace validated owner journal");
        fs::create_dir(&active).expect("create replacement owner journal");
        fs::set_permissions(&active, fs::Permissions::from_mode(PRIVATE_DIRECTORY_MODE))
            .expect("set replacement owner-journal mode");
        let mut members = fs::read_dir(&displaced)
            .expect("list displaced owner journal")
            .map(|entry| entry.expect("read displaced owner member"))
            .collect::<Vec<_>>();
        members.sort_by_key(|entry| entry.file_name());
        for member in members {
            let destination = active.join(member.file_name());
            fs::copy(member.path(), &destination).expect("copy replacement owner member");
            fs::set_permissions(&destination, fs::Permissions::from_mode(PRIVATE_FILE_MODE))
                .expect("set replacement owner-member mode");
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn assert_standalone_owner_canonical_corruption_preserved(near_miss: OwnerJsonNearMiss) {
        let fixture = OwnerFixture::new(InitialOwner::Absent);
        seed_historical_acquisition(&fixture);
        let owner = fixture.corpus.join(owner_path(Domain::Layout));
        let canonical = fs::read(&owner).expect("read standalone canonical owner");
        corrupt_file(&owner, &owner_json_near_miss(&canonical, near_miss));
        assert!(
            fixture
                .rooted()
                .list_dir(&fixture.transaction_parent())
                .expect("inspect standalone owner journals")
                .is_empty(),
            "standalone owner fixture retained a journal"
        );
        let before = fixture.snapshot();

        let read_only: Result<CoordinationGuard> =
            acquire_shared_check(&fixture.location, Domain::Layout);
        let error = read_only.expect_err("read-only check must reject noncanonical owner bytes");
        assert_eq!(error.kind(), GeneratorErrorKind::Verification);
        assert_eq!(
            fixture.snapshot(),
            before,
            "read-only check changed owner evidence"
        );

        let error = fixture
            .recover(None)
            .expect_err("owner recovery must reject noncanonical standalone owner bytes");
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_eq!(
            fixture.snapshot(),
            before,
            "owner recovery changed owner evidence"
        );

        let exclusive: Result<CoordinationGuard> = acquire_exclusive(
            &fixture.location,
            Domain::Layout,
            new_owner_metadata(),
            |_| Ok(()),
        );
        let error = exclusive.expect_err("acquisition must reject noncanonical owner bytes");
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_eq!(
            fixture.snapshot(),
            before,
            "acquisition changed owner evidence"
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn assert_corrupt_owner_install_temporary_preserved(
        initial: InitialOwner,
        temporary_name: &str,
    ) {
        let trace = record_owner_install_trace(initial);
        let fixture = OwnerFixture::new(initial);
        let temporary = format!("{}/{}", fixture.active_path(), temporary_name);
        let event_index = *event_indices(
            &trace,
            DurabilityPhase::OwnerInstall,
            DurabilityPrimitive::WritePartial,
            &temporary,
        )
        .first()
        .expect("owner install records a temporary write prefix");
        interrupt_owner_install(&fixture, &trace, event_index);
        corrupt_file(
            &fixture.corpus.join(&temporary),
            b"corrupt owner publication temporary\n",
        );
        assert_owner_corruption_preserved(&fixture, temporary_name);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn assert_corrupt_owner_outcome_temporary_preserved(committed: bool) {
        let install_trace = record_owner_install_trace(InitialOwner::Old);
        let seed = OwnerRecoverySeed {
            initial: InitialOwner::Old,
            committed,
        };
        let recovery_trace = record_owner_recovery_trace(seed, &install_trace);
        let fixture = OwnerFixture::new(seed.initial);
        seed_owner_recovery(&fixture, &install_trace, committed);
        let outcome = if committed { "committed" } else { "aborted" };
        let temporary = format!(
            "{}/{outcome}-{OWNER_INSTALL_TOKEN}.tmp",
            fixture.active_path()
        );
        let event_index = *event_indices(
            &recovery_trace,
            DurabilityPhase::OwnerRecovery,
            DurabilityPrimitive::WritePartial,
            &temporary,
        )
        .first()
        .expect("owner recovery records an outcome write prefix");
        let observer = RootedObserver::interrupt_after(event_index);
        expect_interruption(|| fixture.recover(Some(observer.clone())));
        assert_event_prefix(
            &observer,
            &recovery_trace,
            event_index,
            "owner outcome recovery",
        );
        corrupt_file(
            &fixture.corpus.join(&temporary),
            b"corrupt owner outcome temporary\n",
        );
        assert_owner_corruption_preserved(&fixture, outcome);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn seed_historical_acquisition(fixture: &OwnerFixture) -> Vec<u8> {
        let guard = acquire_exclusive(
            &fixture.location,
            Domain::Layout,
            old_owner_metadata(),
            |_| Ok(()),
        )
        .expect("seed historical acquisition");
        drop(guard);
        fixture.visible_owner().expect("historical owner record")
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn assert_mutex_released(fixture: &OwnerFixture) {
        let rooted = fixture.rooted();
        let mutex = open_existing_lock(
            &rooted,
            &mutex_path(Domain::Layout),
            CoordinationAccess::Exclusive,
            false,
        )
        .expect("failed acquisition must release domain mutex");
        drop(mutex);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn assert_no_owner_transaction(fixture: &OwnerFixture, historical: &[u8]) {
        fixture.assert_visibility(Some(historical));
        assert!(
            fixture
                .rooted()
                .list_dir(&fixture.transaction_parent())
                .expect("inspect owner transaction parent")
                .is_empty(),
            "failed prerequisite began an owner transaction"
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn invalid_revalidation() -> GeneratorError {
        GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "revalidate protected owner fixture",
            "synthetic protected identity changed",
        )
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    #[ignore = "exhaustive opt-in diagnostic"]
    fn owner_record_install_every_prefix_recovers_absent() {
        exercise_owner_install_prefixes(InitialOwner::Absent);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    #[ignore = "exhaustive opt-in diagnostic"]
    fn owner_record_install_every_prefix_recovers_swap() {
        exercise_owner_install_prefixes(InitialOwner::Old);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    #[ignore = "exhaustive opt-in diagnostic"]
    fn owner_record_recovery_every_prefix_is_idempotent() {
        let absent_trace = record_owner_install_trace(InitialOwner::Absent);
        let old_trace = record_owner_install_trace(InitialOwner::Old);
        for seed in [
            OwnerRecoverySeed {
                initial: InitialOwner::Absent,
                committed: false,
            },
            OwnerRecoverySeed {
                initial: InitialOwner::Absent,
                committed: true,
            },
        ] {
            exercise_owner_recovery_prefixes(seed, &absent_trace);
        }
        for seed in [
            OwnerRecoverySeed {
                initial: InitialOwner::Old,
                committed: false,
            },
            OwnerRecoverySeed {
                initial: InitialOwner::Old,
                committed: true,
            },
        ] {
            exercise_owner_recovery_prefixes(seed, &old_trace);
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn standalone_aborted_owner_outcome_recovers_historical_absence() {
        let install_trace = record_owner_install_trace(InitialOwner::Absent);
        let recovery_trace = record_owner_recovery_trace(
            OwnerRecoverySeed {
                initial: InitialOwner::Absent,
                committed: false,
            },
            &install_trace,
        );
        let fixture = OwnerFixture::new(InitialOwner::Absent);
        seed_standalone_aborted_owner_outcome(&fixture, &install_trace, &recovery_trace);

        recover_owner_and_assert_idempotent(&fixture, None);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn standalone_aborted_owner_outcome_recovers_historical_record() {
        let install_trace = record_owner_install_trace(InitialOwner::Old);
        let recovery_trace = record_owner_recovery_trace(
            OwnerRecoverySeed {
                initial: InitialOwner::Old,
                committed: false,
            },
            &install_trace,
        );
        let fixture = OwnerFixture::new(InitialOwner::Old);
        let expected = fixture.expected_initial_bytes();
        seed_standalone_aborted_owner_outcome(&fixture, &install_trace, &recovery_trace);

        recover_owner_and_assert_idempotent(&fixture, expected.as_deref());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn standalone_committed_owner_outcome_recovers_new_record() {
        let install_trace = record_owner_install_trace(InitialOwner::Old);
        let fixture = OwnerFixture::new(InitialOwner::Old);
        let expected = fixture.expected_new_bytes();
        seed_standalone_committed_owner_outcome(&fixture, &install_trace);

        recover_owner_and_assert_idempotent(&fixture, Some(&expected));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn standalone_owner_outcome_rejects_legacy_marker_schema() {
        let install_trace = record_owner_install_trace(InitialOwner::Old);
        let fixture = OwnerFixture::new(InitialOwner::Old);
        seed_standalone_committed_owner_outcome(&fixture, &install_trace);
        let marker_path = fixture
            .corpus
            .join(format!("{}/committed", fixture.active_path()));
        let mut marker: OwnerOutcomeMarker =
            serde_json::from_slice(&fs::read(&marker_path).expect("read current owner outcome"))
                .expect("parse current owner outcome");
        marker.schema_version = 1;
        let legacy_bytes = canonical_json(&marker, "serialize legacy owner outcome version")
            .expect("serialize legacy owner outcome version");
        corrupt_file(&marker_path, &legacy_bytes);

        assert_owner_corruption_preserved(&fixture, "legacy standalone owner outcome schema");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn standalone_aborted_owner_outcome_rejects_visible_new_forgery() {
        let install_trace = record_owner_install_trace(InitialOwner::Old);
        let fixture = OwnerFixture::new(InitialOwner::Old);
        seed_standalone_committed_owner_outcome(&fixture, &install_trace);
        let expected_new = fixture.expected_new_bytes();
        fixture.assert_visibility(Some(&expected_new));
        replace_standalone_committed_with_aborted(&fixture);

        assert_owner_corruption_preserved(&fixture, "standalone visible-new aborted forgery");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn standalone_aborted_owner_outcome_rejects_equal_digest_new_identity() {
        let trace_fixture = OwnerFixture::new(InitialOwner::Old);
        let trace_bytes = trace_fixture
            .expected_initial_bytes()
            .expect("trace fixture historical owner bytes");
        let observer = RootedObserver::recording();
        trace_fixture
            .install_bytes(observer.clone(), &trace_bytes)
            .expect("record equal-digest owner install");
        let install_trace = observer.events();

        let fixture = OwnerFixture::new(InitialOwner::Old);
        let historical_bytes = fixture
            .expected_initial_bytes()
            .expect("historical owner bytes");
        let historical_identity = fixture
            .visible_owner_identity()
            .expect("historical owner identity");
        let event_index =
            owner_cleanup_intent_removal_index(&install_trace, &fixture.active_path());
        let observer = RootedObserver::interrupt_after(event_index);
        expect_interruption(|| fixture.install_bytes(observer.clone(), &historical_bytes));
        assert_event_prefix(
            &observer,
            &install_trace,
            event_index,
            "equal-digest owner install",
        );
        assert_eq!(
            fixture
                .rooted()
                .list_dir(&fixture.active_path())
                .expect("inspect equal-digest standalone outcome"),
            vec!["committed".to_owned()]
        );
        fixture.assert_visibility(Some(&historical_bytes));
        let visible_identity = fixture
            .visible_owner_identity()
            .expect("replacement owner identity");
        assert!(
            !historical_identity.matches_recovery(&visible_identity),
            "equal owner bytes must still belong to the newly staged identity"
        );
        replace_standalone_committed_with_aborted(&fixture);

        assert_owner_corruption_preserved(
            &fixture,
            "standalone equal-digest different-identity aborted forgery",
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_intent_only_shape_preserves_evidence() {
        let trace = record_owner_install_trace(InitialOwner::Old);
        let fixture = OwnerFixture::new(InitialOwner::Old);
        seed_owner_intent_without_prepared(&fixture, &trace);
        let active = fixture.corpus.join(fixture.active_path());
        fs::remove_file(active.join("owner.stage")).expect("remove impossible owner stage");
        fs::remove_file(active.join("stage-registration.json"))
            .expect("remove impossible owner stage registration");

        assert_owner_corruption_preserved(&fixture, "intent-only owner journal");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_intent_with_missing_stage_preserves_evidence() {
        let trace = record_owner_install_trace(InitialOwner::Old);
        let fixture = OwnerFixture::new(InitialOwner::Old);
        seed_owner_intent_without_prepared(&fixture, &trace);
        fs::remove_file(
            fixture
                .corpus
                .join(format!("{}/owner.stage", fixture.active_path())),
        )
        .expect("remove registered owner stage");

        assert_owner_corruption_preserved(&fixture, "intent with missing registered stage");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_intent_with_empty_unregistered_stage_preserves_evidence() {
        let trace = record_owner_install_trace(InitialOwner::Old);
        let fixture = OwnerFixture::new(InitialOwner::Old);
        seed_owner_intent_without_prepared(&fixture, &trace);
        let active = fixture.corpus.join(fixture.active_path());
        fs::remove_file(active.join("stage-registration.json"))
            .expect("remove owner stage registration");
        corrupt_file(&active.join("owner.stage"), b"");

        assert_owner_corruption_preserved(&fixture, "intent with empty unregistered stage");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_committed_outcome_with_stage_but_missing_prepared_preserves_evidence() {
        let trace = record_owner_install_trace(InitialOwner::Old);
        let fixture = OwnerFixture::new(InitialOwner::Old);
        interrupt_owner_install(&fixture, &trace, owner_marker_index(&trace, "committed"));
        fs::remove_file(
            fixture
                .corpus
                .join(format!("{}/prepared.json", fixture.active_path())),
        )
        .expect("remove impossible committed prepared marker");

        assert_owner_corruption_preserved(
            &fixture,
            "committed owner outcome retained a stage after prepared removal",
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_cleanup_corruption_preserves_complete_journal() {
        let trace = record_owner_install_trace(InitialOwner::Absent);
        for case in ["replaced-marker", "unexpected-temporary", "linked-member"] {
            let fixture = OwnerFixture::new(InitialOwner::Absent);
            interrupt_owner_install(&fixture, &trace, owner_marker_index(&trace, "committed"));
            let active = fixture.active_path();
            match case {
                "replaced-marker" => {
                    let marker = fixture.corpus.join(format!("{active}/committed"));
                    fs::remove_file(&marker).expect("replace owner outcome marker");
                    corrupt_file(&marker, b"{invalid\n");
                }
                "unexpected-temporary" => corrupt_file(
                    &fixture.corpus.join(format!("{active}/injected.tmp")),
                    b"injected cleanup temporary\n",
                ),
                "linked-member" => fs::hard_link(
                    fixture.corpus.join(format!("{active}/prepared.json")),
                    fixture.corpus.join("linked-owner-prepared"),
                )
                .expect("link owner cleanup member"),
                _ => unreachable!("fixed corruption cases"),
            }
            let rooted = fixture.rooted();
            let journal_identity = rooted
                .identity_at(&active)
                .expect("inspect owner cleanup journal")
                .expect("owner cleanup journal remains");
            let authority_key = corpus_authority_key(&rooted, Domain::Layout);
            let before = fixture.snapshot();

            let error = cleanup_owner_journal(
                &rooted,
                Domain::Layout,
                &authority_key,
                OWNER_INSTALL_TOKEN,
                &active,
                journal_identity,
            )
            .expect_err("corrupt owner cleanup journal must be rejected");

            assert_eq!(
                error.kind(),
                GeneratorErrorKind::ArtifactTransaction,
                "{case}"
            );
            assert_eq!(
                fixture.snapshot(),
                before,
                "direct owner cleanup did not preserve every {case} journal byte"
            );
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_cleanup_rejects_aborted_outcome_bound_to_visible_new_owner() {
        for initial in [InitialOwner::Absent, InitialOwner::Old] {
            let trace = record_owner_install_trace(initial);
            let fixture = OwnerFixture::new(initial);
            let active = fixture.active_path();
            let seed_index = match initial {
                InitialOwner::Absent => owner_marker_index(&trace, "committed"),
                InitialOwner::Old => one_event_index(
                    &trace,
                    DurabilityPhase::OwnerInstall,
                    DurabilityPrimitive::RemoveFile,
                    &format!("{active}/owner.stage"),
                ),
            };
            interrupt_owner_install(&fixture, &trace, seed_index);
            let expected_new = fixture.expected_new_bytes();
            fixture.assert_visibility(Some(&expected_new));

            let committed = fixture.corpus.join(format!("{active}/committed"));
            let mut marker: OwnerOutcomeMarker = serde_json::from_slice(
                &fs::read(&committed).expect("read committed owner outcome"),
            )
            .expect("parse committed owner outcome");
            marker.outcome = OwnerOutcomeKind::Aborted;
            fs::remove_file(&committed).expect("replace committed owner outcome");
            corrupt_file(
                &fixture.corpus.join(format!("{active}/aborted")),
                &canonical_json(&marker, "serialize forged aborted owner outcome")
                    .expect("serialize forged aborted owner outcome"),
            );

            let rooted = fixture.rooted();
            let journal_identity = rooted
                .identity_at(&active)
                .expect("inspect owner cleanup journal")
                .expect("owner cleanup journal remains");
            let authority_key = corpus_authority_key(&rooted, Domain::Layout);
            let before = fixture.snapshot();

            let error = cleanup_owner_journal(
                &rooted,
                Domain::Layout,
                &authority_key,
                OWNER_INSTALL_TOKEN,
                &active,
                journal_identity,
            )
            .expect_err("aborted outcome bound to the visible new owner must be rejected");

            assert_eq!(
                error.kind(),
                GeneratorErrorKind::ArtifactTransaction,
                "{initial:?}"
            );
            assert_eq!(
                fixture.snapshot(),
                before,
                "direct owner cleanup did not preserve the complete {initial:?} journal"
            );
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_published_protocol_records_require_canonical_bytes() {
        let trace = record_owner_install_trace(InitialOwner::Absent);
        for (name, near_miss) in [
            ("intent.json", OwnerJsonNearMiss::MissingFinalNewline),
            (
                "stage-registration.json",
                OwnerJsonNearMiss::MissingFinalNewline,
            ),
            ("prepared.json", OwnerJsonNearMiss::MissingFinalNewline),
            ("committed", OwnerJsonNearMiss::MissingFinalNewline),
            ("intent.json", OwnerJsonNearMiss::ReorderedFields),
            (
                "stage-registration.json",
                OwnerJsonNearMiss::ReorderedFields,
            ),
            ("committed", OwnerJsonNearMiss::ReorderedFields),
        ] {
            let fixture = OwnerFixture::new(InitialOwner::Absent);
            interrupt_owner_install(&fixture, &trace, owner_marker_index(&trace, "committed"));
            let path = fixture
                .corpus
                .join(format!("{}/{name}", fixture.active_path()));
            let canonical = fs::read(&path).expect("read published owner protocol record");
            corrupt_file(&path, &owner_json_near_miss(&canonical, near_miss));
            assert_owner_corruption_preserved(
                &fixture,
                &format!("{name} {near_miss:?} canonical-byte near miss"),
            );
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_intent_without_prepared_rejects_altered_or_replaced_registered_stage() {
        for initial in [InitialOwner::Absent, InitialOwner::Old] {
            let trace = record_owner_install_trace(initial);
            for corruption in ["altered-bytes", "replaced-identity"] {
                let fixture = OwnerFixture::new(initial);
                seed_owner_intent_without_prepared(&fixture, &trace);
                let stage = fixture
                    .corpus
                    .join(format!("{}/owner.stage", fixture.active_path()));
                match corruption {
                    "altered-bytes" => {
                        corrupt_file(&stage, &fixture.record_bytes(&old_owner_metadata()));
                    }
                    "replaced-identity" => {
                        let canonical = fs::read(&stage).expect("read registered owner stage");
                        fs::remove_file(&stage).expect("remove registered owner stage");
                        corrupt_file(&stage, &canonical);
                    }
                    _ => unreachable!("fixed owner-stage corruption cases"),
                }
                assert_owner_corruption_preserved(
                    &fixture,
                    &format!("{initial:?} owner stage {corruption} without prepared marker"),
                );
            }
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_recovery_revalidates_active_journal_identity_before_outcome_mutation() {
        let trace = record_owner_install_trace(InitialOwner::Absent);
        let fixture = OwnerFixture::new(InitialOwner::Absent);
        seed_owner_intent_without_prepared(&fixture, &trace);
        let rooted = fixture.rooted();
        let authority_key = corpus_authority_key(&rooted, Domain::Layout);
        let mut raced_snapshot = None;
        let result = {
            let mut before_mutation = |mutation: &str| {
                assert_eq!(mutation, "outcome-publication");
                assert!(
                    raced_snapshot.is_none(),
                    "owner recovery outcome hook ran more than once"
                );
                replace_owner_journal_identity(&fixture);
                raced_snapshot = Some(fixture.snapshot());
                Ok(())
            };
            let mut control = OwnerRecoveryControl::new(&mut before_mutation);
            recover_owner_transactions_controlled(
                &rooted,
                Domain::Layout,
                &authority_key,
                &mut control,
            )
        };
        let error = result.expect_err("replaced active owner journal must stop recovery");
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_eq!(
            fixture.snapshot(),
            raced_snapshot.expect("owner-journal race captured its replacement state"),
            "owner recovery mutated evidence after active-journal replacement"
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_recovery_revalidates_empty_active_journal_before_unlink() {
        let fixture = OwnerFixture::new(InitialOwner::Absent);
        let rooted = fixture.rooted();
        let active = fixture.active_path();
        rooted
            .create_dir_exclusive(&active, PRIVATE_DIRECTORY_MODE)
            .expect("seed empty active owner journal");
        let authority_key = corpus_authority_key(&rooted, Domain::Layout);
        let mut raced_snapshot = None;
        let result = {
            let mut before_mutation = |mutation: &str| {
                assert_eq!(mutation, "empty-journal-unlink");
                assert!(
                    raced_snapshot.is_none(),
                    "empty owner-journal hook ran more than once"
                );
                replace_owner_journal_identity(&fixture);
                raced_snapshot = Some(fixture.snapshot());
                Ok(())
            };
            let mut control = OwnerRecoveryControl::new(&mut before_mutation);
            recover_owner_transactions_controlled(
                &rooted,
                Domain::Layout,
                &authority_key,
                &mut control,
            )
        };
        let error = result.expect_err("replaced empty owner journal must stop recovery");
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_eq!(
            fixture.snapshot(),
            raced_snapshot.expect("empty owner-journal race captured replacement state"),
            "owner recovery unlinked evidence after empty-journal replacement"
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_recovery_plan_revalidates_inventory_before_outcome_publication() {
        let trace = record_owner_install_trace(InitialOwner::Old);
        let fixture = OwnerFixture::new(InitialOwner::Old);
        seed_owner_intent_without_prepared(&fixture, &trace);
        let rooted = fixture.rooted();
        let authority_key = corpus_authority_key(&rooted, Domain::Layout);
        let mut raced_snapshot = None;
        let result = {
            let mut before_mutation = |mutation: &str| {
                if mutation == "outcome-publication" {
                    assert!(raced_snapshot.is_none(), "outcome race ran more than once");
                    corrupt_file(
                        &fixture
                            .corpus
                            .join(format!("{}/unknown", fixture.active_path())),
                        b"unknown recovery member\n",
                    );
                    raced_snapshot = Some(fixture.snapshot());
                }
                Ok(())
            };
            let mut control = OwnerRecoveryControl::new(&mut before_mutation);
            recover_owner_transactions_controlled(
                &rooted,
                Domain::Layout,
                &authority_key,
                &mut control,
            )
        };

        assert_owner_recovery_race_preserved(
            &fixture,
            result,
            raced_snapshot,
            "unknown inventory appeared before outcome publication",
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_recovery_plan_revalidates_member_bytes_before_temporary_removal() {
        let install_trace = record_owner_install_trace(InitialOwner::Old);
        let recovery_trace = record_owner_recovery_trace(
            OwnerRecoverySeed {
                initial: InitialOwner::Old,
                committed: false,
            },
            &install_trace,
        );
        let fixture = OwnerFixture::new(InitialOwner::Old);
        seed_owner_recovery(&fixture, &install_trace, false);
        let temporary = format!(
            "{}/aborted-{OWNER_INSTALL_TOKEN}.tmp",
            fixture.active_path()
        );
        let event_index = *event_indices(
            &recovery_trace,
            DurabilityPhase::OwnerRecovery,
            DurabilityPrimitive::WritePartial,
            &temporary,
        )
        .first()
        .expect("owner recovery outcome temporary has a write prefix");
        interrupt_owner_recovery(&fixture, &recovery_trace, event_index);

        let rooted = fixture.rooted();
        let authority_key = corpus_authority_key(&rooted, Domain::Layout);
        let mut raced_snapshot = None;
        let result = {
            let mut before_mutation = |mutation: &str| {
                if mutation == "temporary-removal" {
                    assert!(
                        raced_snapshot.is_none(),
                        "temporary race ran more than once"
                    );
                    let intent = fixture
                        .corpus
                        .join(format!("{}/intent.json", fixture.active_path()));
                    let canonical = fs::read(&intent).expect("read validated owner intent");
                    corrupt_file(
                        &intent,
                        &owner_json_near_miss(&canonical, OwnerJsonNearMiss::MissingFinalNewline),
                    );
                    raced_snapshot = Some(fixture.snapshot());
                }
                Ok(())
            };
            let mut control = OwnerRecoveryControl::new(&mut before_mutation);
            recover_owner_transactions_controlled(
                &rooted,
                Domain::Layout,
                &authority_key,
                &mut control,
            )
        };

        assert_owner_recovery_race_preserved(
            &fixture,
            result,
            raced_snapshot,
            "intent bytes changed before temporary removal",
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_recovery_plan_revalidates_member_bytes_before_stage_removal() {
        let trace = record_owner_install_trace(InitialOwner::Old);
        let fixture = OwnerFixture::new(InitialOwner::Old);
        seed_owner_intent_without_prepared(&fixture, &trace);
        let rooted = fixture.rooted();
        let authority_key = corpus_authority_key(&rooted, Domain::Layout);
        let mut raced_snapshot = None;
        let result = {
            let mut before_mutation = |mutation: &str| {
                if mutation == "stage-removal" {
                    assert!(raced_snapshot.is_none(), "stage race ran more than once");
                    let registration = fixture
                        .corpus
                        .join(format!("{}/stage-registration.json", fixture.active_path()));
                    let canonical =
                        fs::read(&registration).expect("read validated stage registration");
                    corrupt_file(
                        &registration,
                        &owner_json_near_miss(&canonical, OwnerJsonNearMiss::MissingFinalNewline),
                    );
                    raced_snapshot = Some(fixture.snapshot());
                }
                Ok(())
            };
            let mut control = OwnerRecoveryControl::new(&mut before_mutation);
            recover_owner_transactions_controlled(
                &rooted,
                Domain::Layout,
                &authority_key,
                &mut control,
            )
        };

        assert_owner_recovery_race_preserved(
            &fixture,
            result,
            raced_snapshot,
            "registration bytes changed before stage removal",
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_recovery_plan_revalidates_visibility_before_empty_directory_removal() {
        let fixture = OwnerFixture::new(InitialOwner::Old);
        let rooted = fixture.rooted();
        let active = fixture.active_path();
        rooted
            .create_dir_exclusive(&active, PRIVATE_DIRECTORY_MODE)
            .expect("seed empty active owner journal");
        let authority_key = corpus_authority_key(&rooted, Domain::Layout);
        let mut raced_snapshot = None;
        let result = {
            let mut before_mutation = |mutation: &str| {
                if mutation == "empty-journal-unlink" {
                    assert!(raced_snapshot.is_none(), "empty race ran more than once");
                    corrupt_file(
                        &fixture.corpus.join(owner_path(Domain::Layout)),
                        &fixture.expected_new_bytes(),
                    );
                    raced_snapshot = Some(fixture.snapshot());
                }
                Ok(())
            };
            let mut control = OwnerRecoveryControl::new(&mut before_mutation);
            recover_owner_transactions_controlled(
                &rooted,
                Domain::Layout,
                &authority_key,
                &mut control,
            )
        };

        assert_owner_recovery_race_preserved(
            &fixture,
            result,
            raced_snapshot,
            "visible owner changed before empty journal removal",
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn standalone_owner_missing_canonical_newline_is_preserved() {
        assert_standalone_owner_canonical_corruption_preserved(
            OwnerJsonNearMiss::MissingFinalNewline,
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn standalone_owner_reordered_fields_are_preserved() {
        assert_standalone_owner_canonical_corruption_preserved(OwnerJsonNearMiss::ReorderedFields);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_pre_intent_rejects_alternate_digest_cross_state_prefix() {
        let trace = record_owner_install_trace(InitialOwner::Old);
        let fixture = OwnerFixture::new(InitialOwner::Old);
        let temporary = interrupt_at_complete_owner_intent_temporary(&fixture, &trace);
        let path = fixture.corpus.join(&temporary);
        let mut bytes = fs::read(&path).expect("read owner intent temporary prefix");
        let intended = Sha256Digest::from_bytes(fixture.expected_new_bytes());
        let alternate = Sha256Digest::from_bytes(b"alternate valid owner protocol state");
        assert_ne!(alternate, intended);
        let digest_start = bytes
            .windows(intended.as_str().len())
            .position(|window| window == intended.as_str().as_bytes())
            .expect("intent temporary contains intended new digest");
        bytes[digest_start..digest_start + intended.as_str().len()]
            .copy_from_slice(alternate.as_str().as_bytes());
        corrupt_file(&path, &bytes);

        assert_owner_corruption_preserved(&fixture, "alternate digest cross-state intent prefix");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_pre_intent_canonical_prefix_remains_recoverable() {
        let trace = record_owner_install_trace(InitialOwner::Old);
        let fixture = OwnerFixture::new(InitialOwner::Old);
        interrupt_at_complete_owner_intent_temporary(&fixture, &trace);
        let expected = fixture.expected_initial_bytes();
        recover_owner_and_assert_idempotent(&fixture, expected.as_deref());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_cleanup_revalidates_member_and_visibility_bytes_before_unlink() {
        let trace = record_owner_install_trace(InitialOwner::Absent);
        for target in ["member", "visibility"] {
            let fixture = OwnerFixture::new(InitialOwner::Absent);
            interrupt_owner_install(&fixture, &trace, owner_marker_index(&trace, "committed"));
            let rooted = fixture.rooted();
            let active = fixture.active_path();
            let journal_identity = rooted
                .identity_at(&active)
                .expect("inspect owner cleanup journal")
                .expect("owner cleanup journal remains");
            let authority_key = corpus_authority_key(&rooted, Domain::Layout);
            let mut post_race = None;
            let result = {
                let mut before_unlink = |member: &str| -> Result<()> {
                    if post_race.is_some() {
                        return Ok(());
                    }
                    assert_eq!(member, "prepared.json", "cleanup order changed");
                    let path = if target == "member" {
                        fixture.corpus.join(format!("{active}/prepared.json"))
                    } else {
                        fixture.corpus.join(owner_path(Domain::Layout))
                    };
                    let canonical = fs::read(&path).expect("read validated owner bytes");
                    corrupt_file(
                        &path,
                        &owner_json_near_miss(&canonical, OwnerJsonNearMiss::MissingFinalNewline),
                    );
                    post_race = Some(fixture.snapshot());
                    Ok(())
                };
                let mut control = OwnerCleanupControl::new(&mut before_unlink);
                cleanup_owner_journal_controlled(
                    &rooted,
                    Domain::Layout,
                    &authority_key,
                    OWNER_INSTALL_TOKEN,
                    &active,
                    journal_identity,
                    &mut control,
                )
            };
            let error = result.expect_err("cleanup must reject post-preflight byte changes");
            assert_eq!(
                error.kind(),
                GeneratorErrorKind::ArtifactTransaction,
                "{target}"
            );
            assert_eq!(
                fixture.snapshot(),
                post_race.expect("race callback captured the exact mutated state"),
                "cleanup deleted evidence after the {target} byte race"
            );
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_cleanup_revalidates_empty_inventory_before_directory_removal() {
        let trace = record_owner_install_trace(InitialOwner::Absent);
        let fixture = OwnerFixture::new(InitialOwner::Absent);
        interrupt_owner_install(&fixture, &trace, owner_marker_index(&trace, "committed"));
        let rooted = fixture.rooted();
        let active = fixture.active_path();
        let journal_identity = rooted
            .identity_at(&active)
            .expect("inspect owner cleanup journal")
            .expect("owner cleanup journal remains");
        let authority_key = corpus_authority_key(&rooted, Domain::Layout);
        let mut raced_snapshot = None;
        let result = {
            let mut before_unlink = |member: &str| -> Result<()> {
                if member == "journal-directory" {
                    assert!(
                        raced_snapshot.is_none(),
                        "directory race ran more than once"
                    );
                    corrupt_file(
                        &fixture.corpus.join(format!("{active}/unknown")),
                        b"unknown final cleanup member\n",
                    );
                    raced_snapshot = Some(fixture.snapshot());
                }
                Ok(())
            };
            let mut control = OwnerCleanupControl::new(&mut before_unlink);
            cleanup_owner_journal_controlled(
                &rooted,
                Domain::Layout,
                &authority_key,
                OWNER_INSTALL_TOKEN,
                &active,
                journal_identity,
                &mut control,
            )
        };

        let error = result.expect_err("unknown final member must stop directory removal");
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_eq!(
            fixture.snapshot(),
            raced_snapshot.expect("directory race captured its state"),
            "owner cleanup mutated evidence after the final inventory race"
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_temporary_corruption_preserves_intent_evidence() {
        assert_corrupt_owner_install_temporary_preserved(
            InitialOwner::Absent,
            &format!("intent-{OWNER_INSTALL_TOKEN}.tmp"),
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_temporary_corruption_preserves_registration_evidence() {
        assert_corrupt_owner_install_temporary_preserved(
            InitialOwner::Absent,
            &format!("stage-registration-{OWNER_INSTALL_TOKEN}.tmp"),
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_temporary_corruption_preserves_prepared_evidence() {
        assert_corrupt_owner_install_temporary_preserved(
            InitialOwner::Old,
            &format!("prepared-{OWNER_INSTALL_TOKEN}.tmp"),
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_temporary_corruption_preserves_aborted_outcome_evidence() {
        assert_corrupt_owner_outcome_temporary_preserved(false);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_temporary_corruption_preserves_committed_outcome_evidence() {
        assert_corrupt_owner_outcome_temporary_preserved(true);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn owner_record_corruption_preserves_evidence() {
        let trace = record_owner_install_trace(InitialOwner::Old);
        let active = format!(
            ".surgeist-generator/leases/layout/{OWNER_TRANSACTIONS}/active-{OWNER_INSTALL_TOKEN}"
        );

        let recovery_seed = OwnerRecoverySeed {
            initial: InitialOwner::Old,
            committed: false,
        };
        let recovery_trace = record_owner_recovery_trace(recovery_seed, &trace);
        let outcome_temporary = format!("{active}/aborted-{OWNER_INSTALL_TOKEN}.tmp");
        let outcome_partial = *event_indices(
            &recovery_trace,
            DurabilityPhase::OwnerRecovery,
            DurabilityPrimitive::WritePartial,
            &outcome_temporary,
        )
        .first()
        .expect("owner recovery records an outcome-marker write prefix");
        let prepared_cleanup = one_event_index(
            &recovery_trace,
            DurabilityPhase::OwnerCleanup,
            DurabilityPrimitive::RemoveFile,
            &format!("{active}/prepared.json"),
        );
        for event_index in [outcome_partial, prepared_cleanup] {
            let resumable = OwnerFixture::new(InitialOwner::Old);
            seed_owner_recovery(&resumable, &trace, false);
            let observer = RootedObserver::interrupt_after(event_index);
            expect_interruption(|| resumable.recover(Some(observer.clone())));
            assert_event_prefix(
                &observer,
                &recovery_trace,
                event_index,
                "selected owner recovery",
            );
            let expected = resumable.expected_initial_bytes();
            recover_owner_and_assert_idempotent(&resumable, expected.as_deref());
        }
        let committed_seed = OwnerRecoverySeed {
            initial: InitialOwner::Old,
            committed: true,
        };
        let committed_trace = record_owner_recovery_trace(committed_seed, &trace);
        let committed_temporary = format!("{active}/committed-{OWNER_INSTALL_TOKEN}.tmp");
        let committed_partial = *event_indices(
            &committed_trace,
            DurabilityPhase::OwnerRecovery,
            DurabilityPrimitive::WritePartial,
            &committed_temporary,
        )
        .first()
        .expect("owner recovery records a committed-marker write prefix");
        let committed_cleanup = one_event_index(
            &committed_trace,
            DurabilityPhase::OwnerCleanup,
            DurabilityPrimitive::RemoveFile,
            &format!("{active}/prepared.json"),
        );
        for event_index in [committed_partial, committed_cleanup] {
            let resumable = OwnerFixture::new(InitialOwner::Old);
            seed_owner_recovery(&resumable, &trace, true);
            let observer = RootedObserver::interrupt_after(event_index);
            expect_interruption(|| resumable.recover(Some(observer.clone())));
            assert_event_prefix(
                &observer,
                &committed_trace,
                event_index,
                "selected committed owner recovery",
            );
            let expected = resumable.expected_new_bytes();
            recover_owner_and_assert_idempotent(&resumable, Some(&expected));
        }

        let malformed_intent = OwnerFixture::new(InitialOwner::Old);
        interrupt_owner_install(
            &malformed_intent,
            &trace,
            owner_marker_index(&trace, "intent.json"),
        );
        corrupt_file(
            &malformed_intent
                .corpus
                .join(format!("{active}/intent.json")),
            b"{invalid\n",
        );
        assert_owner_corruption_preserved(&malformed_intent, "malformed intent marker");

        let malformed_outcome = OwnerFixture::new(InitialOwner::Old);
        interrupt_owner_install(
            &malformed_outcome,
            &trace,
            owner_marker_index(&trace, "committed"),
        );
        corrupt_file(
            &malformed_outcome.corpus.join(format!("{active}/committed")),
            b"{invalid\n",
        );
        assert_owner_corruption_preserved(&malformed_outcome, "malformed outcome marker");

        let digest_mismatch = OwnerFixture::new(InitialOwner::Old);
        interrupt_owner_install(
            &digest_mismatch,
            &trace,
            owner_marker_index(&trace, "prepared.json"),
        );
        let wrong_digest = Sha256Digest::from_bytes(b"different owner generation");
        corrupt_file(
            &digest_mismatch
                .corpus
                .join(format!("{active}/prepared.json")),
            &canonical_json(&wrong_digest, "serialize wrong owner digest")
                .expect("serialize wrong owner digest"),
        );
        assert_owner_corruption_preserved(&digest_mismatch, "owner digest mismatch");

        let identity_replacement = OwnerFixture::new(InitialOwner::Old);
        interrupt_owner_install(
            &identity_replacement,
            &trace,
            owner_marker_index(&trace, "prepared.json"),
        );
        let stage = identity_replacement
            .corpus
            .join(format!("{active}/owner.stage"));
        let stage_bytes = fs::read(&stage).expect("read registered owner stage");
        fs::remove_file(&stage).expect("remove registered owner stage");
        corrupt_file(&stage, &stage_bytes);
        assert_owner_corruption_preserved(
            &identity_replacement,
            "owner stage identity replacement",
        );

        let owner_identity_replacement = OwnerFixture::new(InitialOwner::Old);
        interrupt_owner_install(
            &owner_identity_replacement,
            &trace,
            owner_marker_index(&trace, "committed"),
        );
        let owner = owner_identity_replacement
            .corpus
            .join(owner_path(Domain::Layout));
        let owner_bytes = fs::read(&owner).expect("read committed owner record");
        fs::remove_file(&owner).expect("remove committed owner record");
        corrupt_file(&owner, &owner_bytes);
        assert_owner_corruption_preserved(
            &owner_identity_replacement,
            "visible owner identity replacement",
        );

        let unknown_member = OwnerFixture::new(InitialOwner::Old);
        interrupt_owner_install(
            &unknown_member,
            &trace,
            owner_marker_index(&trace, "intent.json"),
        );
        corrupt_file(
            &unknown_member.corpus.join(format!("{active}/unknown")),
            b"unknown\n",
        );
        assert_owner_corruption_preserved(&unknown_member, "unknown owner member");

        let visibility_mismatch = OwnerFixture::new(InitialOwner::Old);
        interrupt_owner_install(
            &visibility_mismatch,
            &trace,
            owner_marker_index(&trace, "prepared.json"),
        );
        corrupt_file(
            &visibility_mismatch.corpus.join(owner_path(Domain::Layout)),
            b"neither durable owner outcome\n",
        );
        assert_owner_corruption_preserved(&visibility_mismatch, "owner visibility mismatch");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn lease_revalidation_failure_preserves_historical_owner() {
        let fixture = OwnerFixture::new(InitialOwner::Absent);
        let historical = seed_historical_acquisition(&fixture);
        let result: Result<CoordinationGuard> = acquire_exclusive(
            &fixture.location,
            Domain::Layout,
            new_owner_metadata(),
            |_| Err(invalid_revalidation()),
        );
        let error = result.expect_err("protected revalidation must reject acquisition");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);
        assert_no_owner_transaction(&fixture, &historical);
        assert_mutex_released(&fixture);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn lease_owner_install_begins_only_after_revalidation() {
        let recovery_failure = OwnerFixture::new(InitialOwner::Absent);
        let historical = seed_historical_acquisition(&recovery_failure);
        let rooted = recovery_failure.rooted();
        let active_transaction =
            ".surgeist-generator/transactions/layout/active-33333333333333333333333333333333";
        rooted
            .create_dir_exclusive(active_transaction, PRIVATE_DIRECTORY_MODE)
            .expect("seed corrupt transaction journal");
        rooted
            .publish_file_exclusive(
                active_transaction,
                "unknown",
                "unknown.tmp",
                b"unknown\n",
                PRIVATE_FILE_MODE,
            )
            .expect("seed unknown transaction member");
        let result: Result<CoordinationGuard> = acquire_exclusive(
            &recovery_failure.location,
            Domain::Layout,
            new_owner_metadata(),
            |_| panic!("revalidation ran after transaction recovery failure"),
        );
        let error = result.expect_err("transaction recovery must reject acquisition");
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_no_owner_transaction(&recovery_failure, &historical);
        assert_mutex_released(&recovery_failure);

        let probe_failure = OwnerFixture::new(InitialOwner::Absent);
        let historical = seed_historical_acquisition(&probe_failure);
        let rooted = probe_failure.rooted();
        let active_probe =
            ".surgeist-generator/probes/layout/active-44444444444444444444444444444444";
        rooted
            .create_dir_exclusive(active_probe, PRIVATE_DIRECTORY_MODE)
            .expect("seed corrupt probe journal");
        rooted
            .publish_file_exclusive(
                active_probe,
                "unknown",
                "unknown.tmp",
                b"unknown\n",
                PRIVATE_FILE_MODE,
            )
            .expect("seed unknown probe member");
        let result: Result<CoordinationGuard> = acquire_exclusive(
            &probe_failure.location,
            Domain::Layout,
            new_owner_metadata(),
            |_| panic!("revalidation ran after probe recovery failure"),
        );
        let error = result.expect_err("probe recovery must reject acquisition");
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_no_owner_transaction(&probe_failure, &historical);
        assert_mutex_released(&probe_failure);

        let success = OwnerFixture::new(InitialOwner::Absent);
        let historical = seed_historical_acquisition(&success);
        let mut revalidated = false;
        let guard = acquire_exclusive(
            &success.location,
            Domain::Layout,
            new_owner_metadata(),
            |rooted| {
                assert_eq!(
                    rooted.read_file(&owner_path(Domain::Layout), PRIVATE_FILE_MODE)?,
                    historical,
                    "owner changed before protected revalidation"
                );
                assert!(
                    rooted.list_dir(&success.transaction_parent())?.is_empty(),
                    "owner transaction began before protected revalidation"
                );
                let error = open_existing_lock(
                    rooted,
                    &mutex_path(Domain::Layout),
                    CoordinationAccess::Exclusive,
                    false,
                )
                .expect_err("protected revalidation must run while mutex is held");
                assert_eq!(error.kind(), GeneratorErrorKind::LeaseActive);
                revalidated = true;
                Ok(())
            },
        )
        .expect("acquire after protected revalidation");
        assert!(revalidated);
        assert_ne!(
            success.visible_owner().as_deref(),
            Some(historical.as_slice()),
            "successful acquisition did not install the new owner"
        );
        assert!(
            success
                .rooted()
                .list_dir(&success.transaction_parent())
                .expect("inspect completed owner transaction")
                .is_empty()
        );
        let held_error = open_existing_lock(
            &success.rooted(),
            &mutex_path(Domain::Layout),
            CoordinationAccess::Exclusive,
            false,
        )
        .expect_err("returned guard must retain the domain mutex");
        assert_eq!(held_error.kind(), GeneratorErrorKind::LeaseActive);
        drop(guard);
        assert_mutex_released(&success);
    }

    #[test]
    fn bootstrap_protocol_publishes_complete_immutable_header_before_adoption() {
        assert_eq!(LOCK_HEADER, b"surgeist-generator-lock-v1\n");
        for domain in [Domain::Layout, Domain::Css] {
            let protocol = BootstrapProtocol::new(domain);
            assert!(protocol.steps_are_journaled());
            let release = protocol
                .steps()
                .iter()
                .position(|step| *step == BootstrapStep::ReleaseStageBeforeLostMarker)
                .unwrap();
            let marker = protocol
                .steps()
                .iter()
                .position(|step| *step == BootstrapStep::PublishLostContended)
                .unwrap();
            assert!(release < marker);
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    #[ignore = "exhaustive opt-in diagnostic"]
    fn bootstrap_header_every_byte_prefix_recovers() {
        let trace = record_uncontended_trace();
        let writes = header_write_indices(&trace);
        let zero_prefix = writes[0]
            .checked_sub(1)
            .expect("header write has a preceding registered-stage event");
        for byte_count in 0..=LOCK_HEADER.len() {
            let fixture = BootstrapFixture::new("header-prefix");
            let event_index = if byte_count == 0 {
                zero_prefix
            } else {
                writes[byte_count - 1]
            };
            interrupt_uncontended_install(&fixture, &trace, event_index);
            fixture.assert_lock(None);
            recover_fixture(&fixture, None, CLAIM_TOKEN_B, SYNTHETIC_ABANDONED_PID)
                .expect("recover immutable header prefix");
            let recovered = fixture.lock_identity();
            assert_eq!(
                recovered.is_some(),
                byte_count == LOCK_HEADER.len(),
                "only a complete immutable header may publish"
            );
            fixture.assert_clean(recovered.as_ref());
            recover_and_assert_idempotent(&fixture, recovered.as_ref());
            let later = fixture.later_acquire_same_lock();
            if let Some(recovered) = recovered {
                assert!(recovered.matches_recovery(&later));
            }
        }

        let corrupt = BootstrapFixture::new("header-corruption");
        interrupt_uncontended_install(&corrupt, &trace, writes[0]);
        fs::write(
            corrupt.corpus.join(stage_path(SYNTHETIC_ABANDONED_PID)),
            b"not-a-header-prefix\n",
        )
        .expect("corrupt registered bootstrap stage");
        let corrupt_before = corrupt.snapshot();
        let error = recover_fixture(&corrupt, None, CLAIM_TOKEN_B, SYNTHETIC_ABANDONED_PID)
            .expect_err("corrupt bootstrap header must remain evidence");
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_eq!(corrupt.snapshot(), corrupt_before);
        corrupt.assert_lock(None);

        let live = BootstrapFixture::new("live-owner");
        let observer = RootedObserver::interrupt_after(0);
        expect_interruption(|| run_install(&live, observer, std::process::id(), None));
        let live_before = live.snapshot();
        let error = recover_fixture(&live, None, CLAIM_TOKEN_B, SYNTHETIC_ABANDONED_PID)
            .expect_err("genuinely live bootstrap owner must block recovery");
        assert_eq!(error.kind(), GeneratorErrorKind::LeaseActive);
        assert_eq!(live.snapshot(), live_before);
        live.assert_lock(None);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    #[ignore = "exhaustive opt-in diagnostic"]
    fn bootstrap_uncontended_every_prefix_recovers() {
        let trace = record_uncontended_trace();
        let stage = stage_path(SYNTHETIC_ABANDONED_PID);
        let writes = header_write_indices(&trace);
        let full_header = *writes.last().expect("complete header event");
        let commit = one_event_index(
            &trace,
            DurabilityPhase::BootstrapInstall,
            DurabilityPrimitive::RenameExclusive,
            ACQUISITION_LOCK,
        );
        assert!(full_header < commit);
        let active = active_path(SYNTHETIC_ABANDONED_PID);
        assert_cleanup_inventory(&trace, &active);
        assert_event_exists(
            &trace,
            DurabilityPhase::BootstrapInstall,
            DurabilityPrimitive::SyncFile,
            &stage,
        );
        for event_index in 0..trace.len() {
            let fixture = BootstrapFixture::new("uncontended-prefix");
            interrupt_uncontended_install(&fixture, &trace, event_index);
            let committed = fixture.lock_identity();
            assert_eq!(committed.is_some(), event_index >= commit);
            fixture.assert_lock(committed.as_ref());
            recover_fixture(&fixture, None, CLAIM_TOKEN_B, SYNTHETIC_ABANDONED_PID)
                .expect("recover uncontended bootstrap prefix");
            let recovered = fixture.lock_identity();
            assert_eq!(recovered.is_some(), event_index >= full_header);
            if let (Some(committed), Some(recovered)) = (&committed, &recovered) {
                assert!(committed.matches_recovery(recovered));
            }
            fixture.assert_clean(recovered.as_ref());
            recover_and_assert_idempotent(&fixture, recovered.as_ref());
            let later = fixture.later_acquire_same_lock();
            if let Some(recovered) = recovered {
                assert!(recovered.matches_recovery(&later));
            }
        }

        exercise_uncontended_recovery_prefixes(&trace, writes[0], false);
        exercise_uncontended_recovery_prefixes(&trace, full_header, true);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    #[ignore = "exhaustive opt-in diagnostic"]
    fn bootstrap_winner_held_every_prefix_recovers() {
        let (trace, _) = record_winner_trace(true);
        let stage = stage_path(SYNTHETIC_ABANDONED_PID);
        let release = one_event_index(
            &trace,
            DurabilityPhase::BootstrapInstall,
            DurabilityPrimitive::DropHandle,
            &stage,
        );
        let active = active_path(SYNTHETIC_ABANDONED_PID);
        let lost = format!("{active}/lost-contended");
        let lost_publication = one_event_index(
            &trace,
            DurabilityPhase::BootstrapInstall,
            DurabilityPrimitive::RenameExclusive,
            &lost,
        );
        assert!(release < lost_publication);
        assert_cleanup_inventory(&trace, &active);
        assert_event_exists(
            &trace,
            DurabilityPhase::BootstrapCleanup,
            DurabilityPrimitive::RemoveFile,
            &lost,
        );

        for event_index in 0..trace.len() {
            let fixture = BootstrapFixture::new("held-winner-prefix");
            let winner = interrupt_winner_install(&fixture, &trace, event_index, true);
            if event_index < release {
                assert!(winner.is_none());
                recover_fixture(&fixture, None, CLAIM_TOKEN_B, SYNTHETIC_ABANDONED_PID)
                    .expect("recover pre-winner bootstrap prefix");
                let recovered = fixture.lock_identity();
                fixture.assert_clean(recovered.as_ref());
                recover_and_assert_idempotent(&fixture, recovered.as_ref());
                fixture.later_acquire_same_lock();
                continue;
            }
            let winner = winner.expect("held winner published before stage release");
            winner.assert_held(&fixture);
            recover_fixture(&fixture, None, CLAIM_TOKEN_B, SYNTHETIC_ABANDONED_PID)
                .expect("recover losing bootstrap while winner is held");
            fixture.assert_clean(Some(&winner.identity));
            winner.assert_held(&fixture);
            recover_and_assert_idempotent(&fixture, Some(&winner.identity));
            winner.assert_held(&fixture);
            let identity = winner.release();
            let later = fixture.later_acquire_same_lock();
            assert!(identity.matches_recovery(&later));
        }

        exercise_winner_recovery_prefixes(&trace, lost_publication, true);

        let corrupt = BootstrapFixture::new("held-winner-corruption");
        let winner = interrupt_winner_install(&corrupt, &trace, lost_publication, true)
            .expect("held winner corruption seed");
        fs::write(corrupt.corpus.join(&lost), b"{invalid\n")
            .expect("corrupt lost-contended marker");
        let before = corrupt.snapshot();
        let error = recover_fixture(&corrupt, None, CLAIM_TOKEN_B, SYNTHETIC_ABANDONED_PID)
            .expect_err("corrupt lost-contended marker must remain evidence");
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_eq!(corrupt.snapshot(), before);
        winner.assert_held(&corrupt);
        drop(winner);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    #[ignore = "exhaustive opt-in diagnostic"]
    fn bootstrap_winner_released_every_prefix_recovers() {
        let (trace, _) = record_winner_trace(false);
        let stage = stage_path(SYNTHETIC_ABANDONED_PID);
        let release = one_event_index(
            &trace,
            DurabilityPhase::BootstrapInstall,
            DurabilityPrimitive::DropHandle,
            &stage,
        );
        let active = active_path(SYNTHETIC_ABANDONED_PID);
        assert_cleanup_inventory(&trace, &active);
        assert!(
            event_indices(
                &trace,
                DurabilityPhase::BootstrapInstall,
                DurabilityPrimitive::RenameExclusive,
                &format!("{active}/lost-contended"),
            )
            .is_empty(),
            "released winner adoption must not publish a lost marker"
        );

        for event_index in 0..trace.len() {
            let fixture = BootstrapFixture::new("released-winner-prefix");
            let winner = interrupt_winner_install(&fixture, &trace, event_index, false);
            if event_index < release {
                assert!(winner.is_none());
                recover_fixture(&fixture, None, CLAIM_TOKEN_B, SYNTHETIC_ABANDONED_PID)
                    .expect("recover pre-winner bootstrap prefix");
                let recovered = fixture.lock_identity();
                fixture.assert_clean(recovered.as_ref());
                recover_and_assert_idempotent(&fixture, recovered.as_ref());
                fixture.later_acquire_same_lock();
                continue;
            }
            let winner = winner.expect("released winner published before stage release");
            fixture.assert_lock(Some(&winner.identity));
            recover_and_assert_idempotent(&fixture, Some(&winner.identity));
            let identity = winner.release();
            let later = fixture.later_acquire_same_lock();
            assert!(identity.matches_recovery(&later));
        }

        exercise_winner_recovery_prefixes(&trace, release, false);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    const PROBE_TOKEN: &str = "55555555555555555555555555555555";
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    const LATER_PROBE_TOKEN: &str = "66666666666666666666666666666666";

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    struct ProbeFixture {
        owner: OwnerFixture,
        protected_before: BTreeMap<PathBuf, SnapshotEntry>,
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[derive(Clone, Debug, Eq, PartialEq)]
    struct ProbeEvidenceSnapshot {
        root_identity: HeldIdentity,
        entries: BTreeMap<PathBuf, SnapshotEntry>,
        identities: BTreeMap<PathBuf, HeldIdentity>,
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    impl ProbeFixture {
        fn new() -> Self {
            let owner = OwnerFixture::new(InitialOwner::Old);
            let rooted = owner.rooted();
            rooted
                .ensure_dir(".surgeist-generator/probes", PRIVATE_DIRECTORY_MODE)
                .expect("create probe root");
            rooted
                .ensure_dir(".surgeist-generator/probes/layout", PRIVATE_DIRECTORY_MODE)
                .expect("create probe domain root");
            fs::write(
                owner.corpus.join("domain-artifact.bin"),
                b"domain artifact must remain byte-identical\n",
            )
            .expect("seed protected domain artifact");
            fs::set_permissions(
                owner.corpus.join("domain-artifact.bin"),
                fs::Permissions::from_mode(0o644),
            )
            .expect("set protected domain artifact mode");
            let protected_before = protected_probe_snapshot(&owner);
            Self {
                owner,
                protected_before,
            }
        }

        fn rooted(&self) -> RootedFs {
            self.owner.rooted()
        }

        fn observed_rooted(&self, observer: RootedObserver) -> RootedFs {
            self.owner.observed_rooted(observer)
        }

        fn active_path(&self) -> String {
            probe_active_path(PROBE_TOKEN)
        }

        fn run_unhooked(&self, observer: RootedObserver) -> Result<()> {
            run_rename_probe(&self.observed_rooted(observer), Domain::Layout, PROBE_TOKEN)
        }

        fn run_controlled(
            &self,
            observer: RootedObserver,
            control: &mut ProbeInstallControl,
        ) -> Result<()> {
            run_rename_probe_controlled(
                &self.observed_rooted(observer),
                Domain::Layout,
                PROBE_TOKEN,
                control,
            )
        }

        fn recover(&self, observer: Option<RootedObserver>) -> Result<()> {
            let rooted =
                observer.map_or_else(|| self.rooted(), |observer| self.observed_rooted(observer));
            recover_probe_journals(&rooted, Domain::Layout)
        }

        fn recover_controlled(&self, control: &mut ProbeRecoveryControl<'_>) -> Result<()> {
            recover_probe_journals_controlled(&self.rooted(), Domain::Layout, control)
        }

        fn assert_protected(&self) {
            assert_eq!(
                protected_probe_snapshot(&self.owner),
                self.protected_before,
                "rename probe changed protected owner or domain bytes"
            );
        }

        fn assert_clean(&self) {
            assert!(
                self.rooted()
                    .list_dir(".surgeist-generator/probes/layout")
                    .expect("inspect probe residue")
                    .is_empty(),
                "rename probe residue remains"
            );
            self.assert_protected();
        }

        fn assert_fresh_recovery_and_reprobe(&self) {
            self.recover(None).expect("recover genuine probe prefix");
            self.assert_clean();
            let recovered = snapshot(&self.owner.corpus);
            self.recover(None).expect("repeat probe recovery");
            assert_eq!(
                snapshot(&self.owner.corpus),
                recovered,
                "probe recovery is not idempotent"
            );
            run_rename_probe(&self.rooted(), Domain::Layout, LATER_PROBE_TOKEN)
                .expect("subsequent production rename probe");
            self.assert_clean();
        }

        fn evidence_snapshot(&self) -> ProbeEvidenceSnapshot {
            let rooted = self.rooted();
            let entries = snapshot(&self.owner.corpus);
            let identities = entries
                .keys()
                .map(|path| {
                    let relative = path.to_str().expect("probe snapshot path is UTF-8");
                    let identity = rooted
                        .identity_at(relative)
                        .expect("inspect probe snapshot identity")
                        .expect("probe snapshot entry remains present");
                    (path.clone(), identity)
                })
                .collect();
            ProbeEvidenceSnapshot {
                root_identity: rooted.identity().clone(),
                entries,
                identities,
            }
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn protected_probe_snapshot(owner: &OwnerFixture) -> BTreeMap<PathBuf, SnapshotEntry> {
        let complete = snapshot(&owner.corpus);
        complete
            .into_iter()
            .filter(|(path, _)| {
                path == Path::new("domain-artifact.bin")
                    || path == Path::new(&owner_path(Domain::Layout))
            })
            .collect()
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn probe_active_path(token: &str) -> String {
        format!(".surgeist-generator/probes/layout/active-{token}")
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn probe_member_path(name: &str) -> String {
        format!("{}/{name}-{PROBE_TOKEN}", probe_active_path(PROBE_TOKEN))
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn record_probe_install_trace() -> Vec<DurabilityEvent> {
        let fixture = ProbeFixture::new();
        let observer = RootedObserver::recording();
        fixture
            .run_unhooked(observer.clone())
            .expect("record production rename-probe install");
        fixture.assert_clean();
        let trace = observer.events();
        assert_probe_install_trace(&trace);
        trace
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn record_probe_capability_trace(
        fault: ProbeCapabilityFault,
    ) -> (Vec<DurabilityEvent>, Vec<String>) {
        let fixture = ProbeFixture::new();
        let observer = RootedObserver::recording();
        let mut control = ProbeInstallControl::new(fault);
        let error = fixture
            .run_controlled(observer.clone(), &mut control)
            .expect_err("injected rename capability must be unsupported");
        assert_capability_error(&error);
        fixture.assert_clean();
        (observer.events(), control.cleanup_trace().to_vec())
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn assert_capability_error(error: &GeneratorError) {
        assert_eq!(error.kind(), GeneratorErrorKind::UnsupportedPlatform);
        assert!(
            std::error::Error::source(error).is_some(),
            "capability error lost its safe source"
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn assert_probe_install_trace(trace: &[DurabilityEvent]) {
        let active = probe_active_path(PROBE_TOKEN);
        assert_event_exists(
            trace,
            DurabilityPhase::ProbeInstall,
            DurabilityPrimitive::CreateDirectory,
            &active,
        );
        assert_event_exists(
            trace,
            DurabilityPhase::ProbeInstall,
            DurabilityPrimitive::RenameExclusive,
            &format!("{active}/intent.json"),
        );
        for name in ["probe-left", "probe-right"] {
            assert_event_exists(
                trace,
                DurabilityPhase::ProbeInstall,
                DurabilityPrimitive::CreateDirectory,
                &probe_member_path(name),
            );
        }
        assert_event_exists(
            trace,
            DurabilityPhase::ProbeInstall,
            DurabilityPrimitive::RenameExclusive,
            &probe_member_path("probe-moved"),
        );
        assert_event_exists(
            trace,
            DurabilityPhase::ProbeInstall,
            DurabilityPrimitive::RenameSwap,
            &probe_member_path("probe-right"),
        );
        for name in ["probe-right", "probe-moved"] {
            assert_event_exists(
                trace,
                DurabilityPhase::ProbeInstall,
                DurabilityPrimitive::RemoveDirectory,
                &probe_member_path(name),
            );
        }
        assert_event_exists(
            trace,
            DurabilityPhase::ProbeInstall,
            DurabilityPrimitive::RemoveFile,
            &format!("{active}/intent.json"),
        );
        assert_event_exists(
            trace,
            DurabilityPhase::ProbeInstall,
            DurabilityPrimitive::RemoveDirectory,
            &active,
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn interrupt_probe_install(
        fixture: &ProbeFixture,
        trace: &[DurabilityEvent],
        event_index: usize,
        fault: Option<ProbeCapabilityFault>,
    ) {
        let observer = RootedObserver::interrupt_after(event_index);
        if let Some(fault) = fault {
            let mut control = ProbeInstallControl::new(fault);
            expect_interruption(|| fixture.run_controlled(observer.clone(), &mut control));
        } else {
            expect_interruption(|| fixture.run_unhooked(observer.clone()));
        }
        assert_event_prefix(&observer, trace, event_index, "rename probe install");
        fixture.assert_protected();
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
    enum ProbeRecoverySeedStage {
        JournalBeforeIntent,
        IntentTemporaryPrefix,
        IntentBeforeProbeMembers,
        PreExclusiveLeftOnly,
        PreExclusiveLeftRight,
        PostExclusiveMovedIsLeftRightIsRight,
        PostSwapMovedIsRightRightIsLeft,
        PostSwapMovedIsRight,
        IntentAfterProbeMembers,
        JournalAfterIntent,
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    impl ProbeRecoverySeedStage {
        fn label(self) -> &'static str {
            match self {
                Self::JournalBeforeIntent => "journal-before-intent",
                Self::IntentTemporaryPrefix => "intent-publication-temporary-prefix",
                Self::IntentBeforeProbeMembers => "intent-before-probe-members",
                Self::PreExclusiveLeftOnly => "pre-exclusive-left-only",
                Self::PreExclusiveLeftRight => "pre-exclusive-left=left-right=right",
                Self::PostExclusiveMovedIsLeftRightIsRight => {
                    "post-exclusive-moved=left-right=right"
                }
                Self::PostSwapMovedIsRightRightIsLeft => "post-swap-moved=right-right=left",
                Self::PostSwapMovedIsRight => "partial-cleanup-moved=right",
                Self::IntentAfterProbeMembers => "partial-cleanup-intent-only",
                Self::JournalAfterIntent => "partial-cleanup-empty-journal",
            }
        }

        fn expected_names(self) -> Vec<String> {
            let intent = "intent.json".to_owned();
            let temporary = format!("intent-{PROBE_TOKEN}.tmp");
            let left = format!("probe-left-{PROBE_TOKEN}");
            let right = format!("probe-right-{PROBE_TOKEN}");
            let moved = format!("probe-moved-{PROBE_TOKEN}");
            let mut names = match self {
                Self::JournalBeforeIntent | Self::JournalAfterIntent => Vec::new(),
                Self::IntentTemporaryPrefix => vec![temporary],
                Self::IntentBeforeProbeMembers | Self::IntentAfterProbeMembers => vec![intent],
                Self::PreExclusiveLeftOnly => vec![intent, left],
                Self::PreExclusiveLeftRight => vec![intent, left, right],
                Self::PostExclusiveMovedIsLeftRightIsRight
                | Self::PostSwapMovedIsRightRightIsLeft => vec![intent, moved, right],
                Self::PostSwapMovedIsRight => vec![intent, moved],
            };
            names.sort();
            names
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[derive(Clone, Debug)]
    struct ProbeRecoverySeed {
        install_event_index: usize,
        stage: ProbeRecoverySeedStage,
        label: String,
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    struct ProbeRecoverySeedBoundaries {
        active_create: usize,
        intent_temporary_create: usize,
        intent_publish: usize,
        left_create: usize,
        right_create: usize,
        exclusive_rename: usize,
        swap_rename: usize,
        right_remove: usize,
        moved_remove: usize,
        intent_remove: usize,
        active_remove: usize,
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    impl ProbeRecoverySeedBoundaries {
        fn capture(trace: &[DurabilityEvent]) -> Self {
            let active = probe_active_path(PROBE_TOKEN);
            let intent_temporary = format!("{active}/intent-{PROBE_TOKEN}.tmp");
            let intent = format!("{active}/intent.json");
            Self {
                active_create: one_event_index(
                    trace,
                    DurabilityPhase::ProbeInstall,
                    DurabilityPrimitive::CreateDirectory,
                    &active,
                ),
                intent_temporary_create: one_event_index(
                    trace,
                    DurabilityPhase::ProbeInstall,
                    DurabilityPrimitive::CreateFile,
                    &intent_temporary,
                ),
                intent_publish: one_event_index(
                    trace,
                    DurabilityPhase::ProbeInstall,
                    DurabilityPrimitive::RenameExclusive,
                    &intent,
                ),
                left_create: one_event_index(
                    trace,
                    DurabilityPhase::ProbeInstall,
                    DurabilityPrimitive::CreateDirectory,
                    &probe_member_path("probe-left"),
                ),
                right_create: one_event_index(
                    trace,
                    DurabilityPhase::ProbeInstall,
                    DurabilityPrimitive::CreateDirectory,
                    &probe_member_path("probe-right"),
                ),
                exclusive_rename: one_event_index(
                    trace,
                    DurabilityPhase::ProbeInstall,
                    DurabilityPrimitive::RenameExclusive,
                    &probe_member_path("probe-moved"),
                ),
                swap_rename: one_event_index(
                    trace,
                    DurabilityPhase::ProbeInstall,
                    DurabilityPrimitive::RenameSwap,
                    &probe_member_path("probe-right"),
                ),
                right_remove: one_event_index(
                    trace,
                    DurabilityPhase::ProbeInstall,
                    DurabilityPrimitive::RemoveDirectory,
                    &probe_member_path("probe-right"),
                ),
                moved_remove: one_event_index(
                    trace,
                    DurabilityPhase::ProbeInstall,
                    DurabilityPrimitive::RemoveDirectory,
                    &probe_member_path("probe-moved"),
                ),
                intent_remove: one_event_index(
                    trace,
                    DurabilityPhase::ProbeInstall,
                    DurabilityPrimitive::RemoveFile,
                    &intent,
                ),
                active_remove: one_event_index(
                    trace,
                    DurabilityPhase::ProbeInstall,
                    DurabilityPrimitive::RemoveDirectory,
                    &active,
                ),
            }
        }

        fn stage(&self, event_index: usize) -> ProbeRecoverySeedStage {
            assert!(
                (self.active_create..self.active_remove).contains(&event_index),
                "probe recovery seed index is outside the active-journal lifetime"
            );
            if event_index < self.intent_temporary_create {
                ProbeRecoverySeedStage::JournalBeforeIntent
            } else if event_index < self.intent_publish {
                ProbeRecoverySeedStage::IntentTemporaryPrefix
            } else if event_index < self.left_create {
                ProbeRecoverySeedStage::IntentBeforeProbeMembers
            } else if event_index < self.right_create {
                ProbeRecoverySeedStage::PreExclusiveLeftOnly
            } else if event_index < self.exclusive_rename {
                ProbeRecoverySeedStage::PreExclusiveLeftRight
            } else if event_index < self.swap_rename {
                ProbeRecoverySeedStage::PostExclusiveMovedIsLeftRightIsRight
            } else if event_index < self.right_remove {
                ProbeRecoverySeedStage::PostSwapMovedIsRightRightIsLeft
            } else if event_index < self.moved_remove {
                ProbeRecoverySeedStage::PostSwapMovedIsRight
            } else if event_index < self.intent_remove {
                ProbeRecoverySeedStage::IntentAfterProbeMembers
            } else {
                ProbeRecoverySeedStage::JournalAfterIntent
            }
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn derive_probe_recovery_seeds(trace: &[DurabilityEvent]) -> Vec<ProbeRecoverySeed> {
        let boundaries = ProbeRecoverySeedBoundaries::capture(trace);
        let temporary = format!(
            "{}/intent-{PROBE_TOKEN}.tmp",
            probe_active_path(PROBE_TOKEN)
        );
        let seeds = (boundaries.active_create..boundaries.active_remove)
            .map(|install_event_index| {
                let stage = boundaries.stage(install_event_index);
                let event = &trace[install_event_index];
                let intent_prefix_bytes = trace[..=install_event_index]
                    .iter()
                    .filter(|event| {
                        event.path() == temporary
                            && matches!(
                                event.primitive(),
                                DurabilityPrimitive::WritePartial
                                    | DurabilityPrimitive::WriteFull
                            )
                    })
                    .count();
                let label = format!(
                    "{};install-event={install_event_index:04};primitive={:?};path={};ordinal={};intent-prefix-bytes={intent_prefix_bytes}",
                    stage.label(),
                    event.primitive(),
                    event.path(),
                    event.ordinal(),
                );
                ProbeRecoverySeed {
                    install_event_index,
                    stage,
                    label,
                }
            })
            .collect::<Vec<_>>();

        let expected_stages = BTreeSet::from([
            ProbeRecoverySeedStage::JournalBeforeIntent,
            ProbeRecoverySeedStage::IntentTemporaryPrefix,
            ProbeRecoverySeedStage::IntentBeforeProbeMembers,
            ProbeRecoverySeedStage::PreExclusiveLeftOnly,
            ProbeRecoverySeedStage::PreExclusiveLeftRight,
            ProbeRecoverySeedStage::PostExclusiveMovedIsLeftRightIsRight,
            ProbeRecoverySeedStage::PostSwapMovedIsRightRightIsLeft,
            ProbeRecoverySeedStage::PostSwapMovedIsRight,
            ProbeRecoverySeedStage::IntentAfterProbeMembers,
            ProbeRecoverySeedStage::JournalAfterIntent,
        ]);
        assert_eq!(
            seeds.iter().map(|seed| seed.stage).collect::<BTreeSet<_>>(),
            expected_stages,
            "rename-probe recovery seeds omit a production install shape"
        );
        assert_eq!(
            seeds
                .iter()
                .map(|seed| seed.label.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            seeds.len(),
            "rename-probe recovery seed labels are not unique"
        );
        seeds
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn seed_probe_recovery_shape(
        fixture: &ProbeFixture,
        install_trace: &[DurabilityEvent],
        seed: &ProbeRecoverySeed,
    ) {
        interrupt_probe_install(fixture, install_trace, seed.install_event_index, None);
        let names = fixture
            .rooted()
            .list_dir(&fixture.active_path())
            .unwrap_or_else(|error| panic!("inspect seeded probe shape {}: {error}", seed.label));
        assert_eq!(
            names,
            seed.stage.expected_names(),
            "seeded probe residue differs from its deterministic label: {}",
            seed.label
        );
        assert_valid_probe_recovery_residue(fixture, &seed.label);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn assert_valid_probe_recovery_residue(fixture: &ProbeFixture, label: &str) {
        let before = fixture.evidence_snapshot();
        let rooted = fixture.rooted();
        let parent = ".surgeist-generator/probes/layout";
        let names = rooted
            .list_dir(parent)
            .unwrap_or_else(|error| panic!("inspect probe recovery residue {label}: {error}"));
        match names.as_slice() {
            [] => {}
            [name] if name == &format!("active-{PROBE_TOKEN}") => {
                let active = fixture.active_path();
                let identity = rooted
                    .identity_at(&active)
                    .unwrap_or_else(|error| {
                        panic!("inspect active probe recovery residue {label}: {error}")
                    })
                    .unwrap_or_else(|| panic!("active probe recovery residue vanished: {label}"));
                ProbeRecoveryPlan::capture(
                    &rooted,
                    Domain::Layout,
                    PROBE_TOKEN,
                    &active,
                    &identity,
                )
                .unwrap_or_else(|error| {
                    panic!("probe recovery residue is not exactly valid ({label}): {error}")
                });
            }
            _ => panic!("unexpected probe recovery residue for {label}: {names:?}"),
        }
        assert_eq!(
            fixture.evidence_snapshot(),
            before,
            "read-only probe residue classification mutated state: {label}"
        );
        fixture.assert_protected();
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn seed_probe_recovery(fixture: &ProbeFixture, install_trace: &[DurabilityEvent]) {
        let swap = one_event_index(
            install_trace,
            DurabilityPhase::ProbeInstall,
            DurabilityPrimitive::RenameSwap,
            &probe_member_path("probe-right"),
        );
        interrupt_probe_install(fixture, install_trace, swap, None);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn record_probe_recovery_trace(
        install_trace: &[DurabilityEvent],
        seed: &ProbeRecoverySeed,
    ) -> Vec<DurabilityEvent> {
        let fixture = ProbeFixture::new();
        seed_probe_recovery_shape(&fixture, install_trace, seed);
        let observer = RootedObserver::recording();
        fixture
            .recover(Some(observer.clone()))
            .unwrap_or_else(|error| {
                panic!(
                    "record unhooked production rename-probe recovery for {}: {error}",
                    seed.label
                )
            });
        fixture.assert_clean();
        let trace = observer.events();
        assert!(
            trace.iter().any(|event| {
                event.phase() == DurabilityPhase::ProbeRecovery
                    && matches!(
                        event.primitive(),
                        DurabilityPrimitive::RemoveFile
                            | DurabilityPrimitive::RemoveDirectory
                            | DurabilityPrimitive::SyncDirectory
                    )
            }),
            "probe recovery trace contains no individual removal or sync: {}",
            seed.label
        );
        let recovered = fixture.evidence_snapshot();
        fixture
            .recover(None)
            .unwrap_or_else(|error| panic!("repeat clean recovery for {}: {error}", seed.label));
        assert_eq!(
            fixture.evidence_snapshot(),
            recovered,
            "unhooked probe recovery is not idempotent: {}",
            seed.label
        );
        trace
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn assert_probe_error_preserves_snapshot(fixture: &ProbeFixture, label: &str) {
        let before = snapshot(&fixture.owner.corpus);
        let error = fixture
            .recover(None)
            .expect_err("corrupt probe evidence must fail closed");
        assert_eq!(
            error.kind(),
            GeneratorErrorKind::ArtifactTransaction,
            "{label}"
        );
        assert_eq!(
            snapshot(&fixture.owner.corpus),
            before,
            "probe recovery mutated corrupt evidence: {label}"
        );
        fixture.assert_protected();
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn seed_probe_after_exclusive(fixture: &ProbeFixture, trace: &[DurabilityEvent]) {
        let rename = one_event_index(
            trace,
            DurabilityPhase::ProbeInstall,
            DurabilityPrimitive::RenameExclusive,
            &probe_member_path("probe-moved"),
        );
        interrupt_probe_install(fixture, trace, rename, None);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn seed_probe_before_exclusive(fixture: &ProbeFixture, trace: &[DurabilityEvent]) {
        let rename = one_event_index(
            trace,
            DurabilityPhase::ProbeInstall,
            DurabilityPrimitive::RenameExclusive,
            &probe_member_path("probe-moved"),
        );
        let preceding = rename
            .checked_sub(1)
            .expect("probe exclusive rename has a preceding durable prefix");
        interrupt_probe_install(fixture, trace, preceding, None);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn seed_nested_probe_child(fixture: &ProbeFixture, parent: &str) {
        let child = fixture.owner.corpus.join(parent).join("unknown-nested");
        fs::create_dir(&child).expect("seed nested unknown probe child");
        fs::set_permissions(&child, fs::Permissions::from_mode(PRIVATE_DIRECTORY_MODE))
            .expect("set nested unknown probe child mode");
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn probe_install_members(
        fixture: &ProbeFixture,
        names: &[&str],
    ) -> (ProbeIntent, Vec<u8>, Vec<(String, String, HeldIdentity)>) {
        let rooted = fixture.rooted();
        let intent_path = format!("{}/intent.json", fixture.active_path());
        let intent_bytes = rooted
            .read_file(&intent_path, PRIVATE_FILE_MODE)
            .expect("read seeded probe intent");
        let intent = serde_json::from_slice(&intent_bytes).expect("parse seeded probe intent");
        let members = names
            .iter()
            .map(|name| {
                let name = format!("{name}-{PROBE_TOKEN}");
                let path = format!("{}/{name}", fixture.active_path());
                let identity = rooted
                    .identity_at(&path)
                    .expect("inspect seeded capability member")
                    .expect("seeded capability member remains");
                (name, path, identity)
            })
            .collect();
        (intent, intent_bytes, members)
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn rename_probe_recovery_nested_later_directory_preserves_complete_evidence() {
        let trace = record_probe_install_trace();
        for (label, after_exclusive) in [
            ("pre-exclusive left/right", false),
            ("post-exclusive moved/right", true),
        ] {
            let fixture = ProbeFixture::new();
            if after_exclusive {
                seed_probe_after_exclusive(&fixture, &trace);
            } else {
                seed_probe_before_exclusive(&fixture, &trace);
            }
            seed_nested_probe_child(&fixture, &probe_member_path("probe-right"));
            let before = fixture.evidence_snapshot();

            let error = fixture
                .recover(None)
                .expect_err("nested probe evidence must stop recovery before cleanup");

            assert_eq!(
                error.kind(),
                GeneratorErrorKind::ArtifactTransaction,
                "{label}"
            );
            assert_eq!(
                fixture.evidence_snapshot(),
                before,
                "probe recovery removed earlier valid evidence: {label}"
            );
            fixture.assert_protected();
        }

        let raced = ProbeFixture::new();
        seed_probe_after_exclusive(&raced, &trace);
        let mut raced_snapshot = None;
        let result = {
            let mut before_mutation = |member: &str| {
                assert_eq!(member, format!("probe-moved-{PROBE_TOKEN}"));
                assert!(
                    raced_snapshot.is_none(),
                    "nested probe race ran more than once"
                );
                seed_nested_probe_child(&raced, &probe_member_path("probe-right"));
                raced_snapshot = Some(raced.evidence_snapshot());
                Ok(())
            };
            let mut control = ProbeRecoveryControl::new(&mut before_mutation);
            raced.recover_controlled(&mut control)
        };
        let error = result.expect_err("nested probe race must stop before cleanup mutation");
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_eq!(
            raced.evidence_snapshot(),
            raced_snapshot.expect("nested probe race captured its state"),
            "probe recovery removed evidence after a nested inventory race"
        );
        raced.assert_protected();
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn rename_probe_capability_cleanup_nested_later_directory_preserves_complete_evidence() {
        let trace = record_probe_install_trace();
        for (fault, rename_kind, names) in [
            (
                ProbeCapabilityFault::FailExclusiveRename,
                "exclusive",
                ["probe-left", "probe-right"],
            ),
            (
                ProbeCapabilityFault::FailSwapRename,
                "swap",
                ["probe-moved", "probe-right"],
            ),
        ] {
            let fixture = ProbeFixture::new();
            if fault == ProbeCapabilityFault::FailExclusiveRename {
                seed_probe_before_exclusive(&fixture, &trace);
            } else {
                seed_probe_after_exclusive(&fixture, &trace);
            }
            let (intent, intent_bytes, member_data) = probe_install_members(&fixture, &names);
            let rooted = fixture.rooted();
            let members = member_data
                .iter()
                .map(|(name, path, identity)| {
                    ProbeInstallMember::capture(&rooted, name, path, identity)
                        .expect("capture seeded capability member inventory")
                })
                .collect::<Vec<_>>();
            seed_nested_probe_child(&fixture, &probe_member_path("probe-right"));
            let before = fixture.evidence_snapshot();

            let error = finish_probe_capability_failure(
                &rooted,
                &intent,
                &intent_bytes,
                rename_kind,
                injected_probe_capability_error(rename_kind),
                &members,
                None,
            )
            .expect_err("nested probe evidence must stop capability cleanup before mutation");

            assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
            assert!(error.to_string().contains("capability"));
            assert_eq!(
                fixture.evidence_snapshot(),
                before,
                "capability cleanup removed earlier valid evidence: {rename_kind}"
            );
            fixture.assert_protected();
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn rename_probe_corruption_preserves_evidence() {
        let trace = record_probe_install_trace();

        let unknown = ProbeFixture::new();
        seed_probe_after_exclusive(&unknown, &trace);
        fs::write(
            unknown
                .owner
                .corpus
                .join(format!("{}/zz-unknown", unknown.active_path())),
            b"unknown probe evidence\n",
        )
        .expect("seed unknown probe member");
        assert_probe_error_preserves_snapshot(&unknown, "unknown member");

        let wrong_mode = ProbeFixture::new();
        seed_probe_after_exclusive(&wrong_mode, &trace);
        fs::set_permissions(
            wrong_mode
                .owner
                .corpus
                .join(probe_member_path("probe-moved")),
            fs::Permissions::from_mode(0o755),
        )
        .expect("change probe member mode");
        assert_probe_error_preserves_snapshot(&wrong_mode, "wrong directory mode");

        let wrong_type = ProbeFixture::new();
        seed_probe_after_exclusive(&wrong_type, &trace);
        let moved = wrong_type
            .owner
            .corpus
            .join(probe_member_path("probe-moved"));
        fs::remove_dir(&moved).expect("remove probe directory before type replacement");
        fs::write(&moved, b"not a probe directory\n").expect("replace probe with file");
        fs::set_permissions(&moved, fs::Permissions::from_mode(PRIVATE_FILE_MODE))
            .expect("set replacement probe mode");
        assert_probe_error_preserves_snapshot(&wrong_type, "wrong member type");

        let alias = ProbeFixture::new();
        seed_probe_after_exclusive(&alias, &trace);
        let moved = alias.owner.corpus.join(probe_member_path("probe-moved"));
        fs::remove_dir(&moved).expect("remove probe directory before alias replacement");
        std::os::unix::fs::symlink(probe_member_path("probe-right"), &moved)
            .expect("replace probe with alias");
        assert_probe_error_preserves_snapshot(&alias, "probe alias");

        let replaced_journal = ProbeFixture::new();
        seed_probe_after_exclusive(&replaced_journal, &trace);
        let active = replaced_journal
            .owner
            .corpus
            .join(replaced_journal.active_path());
        let displaced = replaced_journal
            .owner
            .corpus
            .join("displaced-probe-journal");
        fs::rename(&active, &displaced).expect("displace bound probe journal");
        fs::create_dir(&active).expect("replace probe journal directory");
        fs::set_permissions(&active, fs::Permissions::from_mode(PRIVATE_DIRECTORY_MODE))
            .expect("set replacement journal mode");
        for entry in fs::read_dir(&displaced).expect("read displaced probe journal") {
            let entry = entry.expect("read displaced member");
            fs::rename(entry.path(), active.join(entry.file_name()))
                .expect("move evidence into replacement journal");
        }
        fs::remove_dir(&displaced).expect("remove displaced journal shell");
        assert_probe_error_preserves_snapshot(&replaced_journal, "replaced journal identity");

        let raced_member = ProbeFixture::new();
        seed_probe_after_exclusive(&raced_member, &trace);
        let mut raced_snapshot = None;
        let result = {
            let mut before_mutation = |_: &str| {
                if raced_snapshot.is_none() {
                    let moved = raced_member
                        .owner
                        .corpus
                        .join(probe_member_path("probe-moved"));
                    fs::remove_dir(&moved).expect("remove validated probe before race");
                    fs::create_dir(&moved).expect("replace validated probe before unlink");
                    fs::set_permissions(&moved, fs::Permissions::from_mode(PRIVATE_DIRECTORY_MODE))
                        .expect("set raced probe mode");
                    raced_snapshot = Some(snapshot(&raced_member.owner.corpus));
                }
                Ok(())
            };
            let mut control = ProbeRecoveryControl::new(&mut before_mutation);
            raced_member.recover_controlled(&mut control)
        };
        let error = result.expect_err("replaced validated member must stop recovery");
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_eq!(
            snapshot(&raced_member.owner.corpus),
            raced_snapshot.expect("probe race captured its replacement state")
        );
        raced_member.assert_protected();
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn rename_probe_production_paths_recover_and_preserve_capability_errors() {
        let install_trace = record_probe_install_trace();
        let interrupted = ProbeFixture::new();
        seed_probe_recovery(&interrupted, &install_trace);
        interrupted.assert_fresh_recovery_and_reprobe();

        for fault in [
            ProbeCapabilityFault::FailExclusiveRename,
            ProbeCapabilityFault::FailSwapRename,
        ] {
            let fixture = ProbeFixture::new();
            let observer = RootedObserver::recording();
            let mut control = ProbeInstallControl::new(fault);
            let error = fixture
                .run_controlled(observer, &mut control)
                .expect_err("injected capability fault must reject the probe");
            assert_capability_error(&error);
            let expected_cleanup = match fault {
                ProbeCapabilityFault::FailExclusiveRename => vec![
                    probe_member_path("probe-left"),
                    probe_member_path("probe-right"),
                    "probe-directory-sync".to_owned(),
                    "intent.json".to_owned(),
                    "journal-directory".to_owned(),
                ],
                ProbeCapabilityFault::FailSwapRename => vec![
                    probe_member_path("probe-moved"),
                    probe_member_path("probe-right"),
                    "probe-directory-sync".to_owned(),
                    "intent.json".to_owned(),
                    "journal-directory".to_owned(),
                ],
            };
            assert_eq!(control.cleanup_trace(), expected_cleanup);
            fixture.assert_clean();
            fixture.recover(None).expect("repeat clean probe recovery");
            fixture.assert_clean();

            let interrupted_cleanup = ProbeFixture::new();
            let mut control = ProbeInstallControl::failing_cleanup(fault, 1);
            let error = interrupted_cleanup
                .run_controlled(RootedObserver::recording(), &mut control)
                .expect_err("partial capability cleanup must retain evidence");
            assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
            let diagnostic = error.to_string();
            assert!(diagnostic.contains("capability"), "{diagnostic}");
            assert!(diagnostic.contains("cleanup"), "{diagnostic}");
            interrupted_cleanup.assert_protected();
            interrupted_cleanup
                .recover(None)
                .expect("resume representative partial probe cleanup");
            interrupted_cleanup.assert_clean();
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    #[ignore = "exhaustive opt-in diagnostic"]
    fn rename_probe_install_every_prefix_recovers() {
        let trace = record_probe_install_trace();
        for event_index in 0..trace.len() {
            let fixture = ProbeFixture::new();
            interrupt_probe_install(&fixture, &trace, event_index, None);
            fixture.assert_fresh_recovery_and_reprobe();
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    #[ignore = "exhaustive opt-in diagnostic"]
    fn rename_probe_exclusive_unsupported_every_prefix_recovers() {
        exercise_probe_capability_prefixes(ProbeCapabilityFault::FailExclusiveRename);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    #[ignore = "exhaustive opt-in diagnostic"]
    fn rename_probe_swap_unsupported_every_prefix_recovers() {
        exercise_probe_capability_prefixes(ProbeCapabilityFault::FailSwapRename);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn exercise_probe_capability_prefixes(fault: ProbeCapabilityFault) {
        let (trace, _) = record_probe_capability_trace(fault);
        for event_index in 0..trace.len() {
            let fixture = ProbeFixture::new();
            interrupt_probe_install(&fixture, &trace, event_index, Some(fault));
            fixture
                .recover(None)
                .expect("recover capability-probe prefix");
            fixture.assert_clean();

            let observer = RootedObserver::recording();
            let mut control = ProbeInstallControl::new(fault);
            let error = fixture
                .run_controlled(observer, &mut control)
                .expect_err("capability limitation must recur after recovery");
            assert_capability_error(&error);
            fixture.assert_clean();
            fixture.assert_fresh_recovery_and_reprobe();
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    #[ignore = "exhaustive opt-in diagnostic"]
    fn rename_probe_unsupported_cleanup_failure_preserves_evidence() {
        for fault in [
            ProbeCapabilityFault::FailExclusiveRename,
            ProbeCapabilityFault::FailSwapRename,
        ] {
            let (_, cleanup_trace) = record_probe_capability_trace(fault);
            assert!(
                !cleanup_trace.is_empty(),
                "capability cleanup trace is empty"
            );
            for cleanup_index in 0..cleanup_trace.len() {
                let fixture = ProbeFixture::new();
                let observer = RootedObserver::recording();
                let mut control = ProbeInstallControl::failing_cleanup(fault, cleanup_index);
                let error = fixture
                    .run_controlled(observer, &mut control)
                    .expect_err("injected cleanup failure must reject the probe");
                assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
                let diagnostic = error.to_string();
                assert!(diagnostic.contains("capability"), "{diagnostic}");
                assert!(diagnostic.contains("cleanup"), "{diagnostic}");
                assert!(
                    fixture
                        .rooted()
                        .identity_at(&fixture.active_path())
                        .expect("inspect retained active probe journal")
                        .is_some(),
                    "cleanup failure did not retain an active probe journal"
                );
                fixture.assert_protected();
                fixture.recover(None).expect("resume failed probe cleanup");
                fixture.assert_clean();

                let mut repeated = ProbeInstallControl::new(fault);
                let repeated_error = fixture
                    .run_controlled(RootedObserver::recording(), &mut repeated)
                    .expect_err("capability limitation must recur after cleanup recovery");
                assert_capability_error(&repeated_error);
                fixture.assert_clean();
            }
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    #[ignore = "exhaustive opt-in diagnostic"]
    fn rename_probe_recovery_every_prefix_is_idempotent() {
        let install_trace = record_probe_install_trace();
        for seed in derive_probe_recovery_seeds(&install_trace) {
            let recovery_trace = record_probe_recovery_trace(&install_trace, &seed);
            for recovery_event_index in 0..recovery_trace.len() {
                let fixture = ProbeFixture::new();
                seed_probe_recovery_shape(&fixture, &install_trace, &seed);
                let observer = RootedObserver::interrupt_after(recovery_event_index);
                expect_interruption(|| fixture.recover(Some(observer.clone())));
                assert_event_prefix(
                    &observer,
                    &recovery_trace,
                    recovery_event_index,
                    &format!("rename probe recovery ({})", seed.label),
                );
                assert_valid_probe_recovery_residue(&fixture, &seed.label);
                fixture.assert_fresh_recovery_and_reprobe();
            }
        }
    }
}
