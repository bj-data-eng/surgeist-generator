use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::{
    CorpusLocation, GeneratorError, GeneratorErrorKind, Result, RunScope, Sha256Digest,
};

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
            && self.steps().iter().position(|step| {
                *step == BootstrapStep::ReleaseStageBeforeLostMarker
            }) < self
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
        self.state.rooted.revalidate_root().map_err(verification_from)?;
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

pub(crate) fn acquire_exclusive(
    location: &CorpusLocation,
    domain: Domain,
    metadata: LeaseMetadata,
    protected_revalidation: impl FnOnce(&RootedFs) -> Result<()>,
) -> Result<CoordinationGuard> {
    MutationTarget::current().require_supported("acquire generation mutation lease")?;
    let rooted = RootedFs::open_corpus(location)?;
    rooted.ensure_dir(COORDINATION_ROOT, PRIVATE_DIRECTORY_MODE)?;
    rooted.ensure_dir(
        ".surgeist-generator/bootstrap",
        PRIVATE_DIRECTORY_MODE,
    )?;
    rooted.ensure_dir(BOOTSTRAP_LOCKS, PRIVATE_DIRECTORY_MODE)?;
    validate_coordination_tree(&rooted, domain, false)?;
    recover_bootstrap(&rooted)?;
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
    let transaction_parent = format!(
        ".surgeist-generator/transactions/{}",
        domain.as_str()
    );
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

pub(crate) fn acquire_shared_check(
    location: &CorpusLocation,
    domain: Domain,
) -> Result<CoordinationGuard> {
    MutationTarget::current().require_supported("acquire generation check guard")?;
    let rooted = RootedFs::open_corpus(location)?;
    let authority_key = corpus_authority_key(&rooted, domain);
    let transaction_parent = format!(
        ".surgeist-generator/transactions/{}",
        domain.as_str()
    );
    if !rooted.exists(COORDINATION_ROOT).map_err(verification_from)? {
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
    let gate = open_existing_lock(
        &rooted,
        ACQUISITION_LOCK,
        CoordinationAccess::Shared,
        true,
    )?;
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
    let mutex = match open_existing_lock(
        &rooted,
        &mutex_path,
        CoordinationAccess::Shared,
        true,
    ) {
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

fn open_or_bootstrap_lock(
    rooted: &RootedFs,
    final_path: &str,
    label: &str,
    token: &str,
    access: CoordinationAccess,
) -> Result<File> {
    if rooted.exists(final_path)? {
        return open_existing_lock(rooted, final_path, access, false);
    }
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
    stage.write_all(LOCK_HEADER).map_err(|source| {
        transaction_source(
            "write immutable generation lock header",
            final_path,
            source,
        )
    })?;
    stage.flush().map_err(|source| {
        transaction_source(
            "flush immutable generation lock header",
            final_path,
            source,
        )
    })?;
    stage.sync_all().map_err(|source| {
        transaction_source(
            "sync immutable generation lock header",
            final_path,
            source,
        )
    })?;
    rooted.validate_handle_at(&stage_path, &stage, PRIVATE_FILE_MODE)?;
    lock_file(&stage, access, final_path)?;
    match rooted.rename_exclusive_bound(&stage_path, final_path, &stage_record.identity) {
        Ok(()) => {
            rooted.validate_handle_at(final_path, &stage, PRIVATE_FILE_MODE)?;
            cleanup_bootstrap_directory(rooted, &active, &active_name, Some("lock.stage"))?;
            Ok(stage)
        }
        Err(_rename_error) if rooted.exists(final_path)? => {
            drop(stage);
            let final_file = match open_existing_lock(rooted, final_path, access, false) {
                Ok(file) => file,
                Err(error) if error.kind() == GeneratorErrorKind::LeaseActive => {
                    let final_handle = rooted.open_file_handle(final_path, PRIVATE_FILE_MODE, false)?;
                    validate_lock_header(rooted, final_path, &final_handle, false)?;
                    let final_identity = rooted.identity_of_handle(&final_handle)?;
                    drop(final_handle);
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
        .open_file_handle(path, PRIVATE_FILE_MODE, access == CoordinationAccess::Exclusive)
        .map_err(|error| if verification { verification_from(error) } else { error })?;
    validate_lock_header(rooted, path, &file, verification)?;
    lock_file(&file, access, path)?;
    rooted
        .validate_handle_at(path, &file, PRIVATE_FILE_MODE)
        .map_err(|error| if verification { verification_from(error) } else { error })?;
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
        .map_err(|error| if verification { verification_from(error) } else { error })?;
    let mut copy = file.try_clone().map_err(|source| {
        transaction_source("clone immutable generation lock", path, source)
    })?;
    copy.seek(SeekFrom::Start(0)).map_err(|source| {
        transaction_source("seek immutable generation lock", path, source)
    })?;
    let mut bytes = Vec::new();
    copy.read_to_end(&mut bytes).map_err(|source| {
        transaction_source("read immutable generation lock", path, source)
    })?;
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
        .map_err(|error| if verification { verification_from(error) } else { error })?;
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
            if process_is_live(parsed.owner_pid)? && !relinquished {
                return Err(GeneratorError::new(
                    GeneratorErrorKind::LeaseActive,
                    "recover generation lock bootstrap",
                    "a live bootstrap owner is active",
                ));
            }
            let claim_token = new_token()?;
            let claimant_pid = std::process::id();
            let claim_name = format!(
                "recovering-{}-{}-by-{claimant_pid}-{claim_token}",
                parsed.origin_pid, parsed.origin_token
            );
            let claim_path = format!("{BOOTSTRAP_LOCKS}/{claim_name}");
            let journal_identity = rooted.identity_at(&path)?.ok_or_else(|| {
                transaction_error("claim bootstrap recovery", "bootstrap journal disappeared")
            })?;
            if let Err(error) =
                rooted.rename_exclusive_bound(&path, &claim_path, &journal_identity)
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
        transaction_error("validate bootstrap journal", "bootstrap journal disappeared")
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
        transaction_error("validate lost-contended marker", "bound final lock is absent")
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
    let intent: BootstrapIntent = serde_json::from_slice(
        &rooted.read_file(&intent_path, PRIVATE_FILE_MODE)?,
    )
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
    let stage_record: BootstrapStage = serde_json::from_slice(
        &rooted.read_file(&stage_record_path, PRIVATE_FILE_MODE)?,
    )
    .map_err(|error| transaction_error("recover bootstrap stage", error.to_string()))?;
    rooted.rename_exclusive_bound(
        &stage_path,
        &intent.final_path,
        &stage_record.identity,
    )?;
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
    let identity = rooted.identity_at(path)?.ok_or_else(|| {
        transaction_error("clean bootstrap directory", "bootstrap directory disappeared")
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
    if receipt.schema_version != 1
        || !receipt.journal_identity.matches_recovery(&identity)
    {
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
    let allowed = ["acquisition.lock", "bootstrap", "leases", "transactions", "probes"];
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
                validate_private_directory(
                    rooted,
                    &format!("{owner_transactions}/{name}"),
                )?;
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
        if !rooted.exists(parent).map_err(|error| {
            error
        })? {
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
    let identity = rooted.identity_at(path)?.ok_or_else(|| {
        transaction_error("validate private coordination file", path.to_owned())
    })?;
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
            && !rooted.list_dir(&parent).map_err(verification_from)?.is_empty()
        {
            return Err(verification_error(
                "inspect generation coordination",
                format!("unresolved durable state: {parent}"),
            ));
        }
    }
    let owner = owner_path(domain);
    if rooted.exists(&owner).map_err(verification_from)? {
        let record: OwnerRecord = serde_json::from_slice(
            &rooted
                .read_file(&owner, PRIVATE_FILE_MODE)
                .map_err(verification_from)?,
        )
        .map_err(|error| {
            verification_error(
                "inspect historical generation owner",
                format!("owner record is invalid: {error}"),
            )
        })?;
        validate_owner_record(rooted, &record).map_err(verification_from)?;
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

fn run_rename_probe(rooted: &RootedFs, domain: Domain, token: &str) -> Result<()> {
    let parent = format!(".surgeist-generator/probes/{}", domain.as_str());
    let active = format!("{parent}/active-{token}");
    let active_identity = rooted.create_dir_exclusive(&active, PRIVATE_DIRECTORY_MODE)?;
    let intent = canonical_json(&domain, "serialize rename probe intent")?;
    rooted.publish_file_exclusive(
        &active,
        "intent.json",
        &format!("intent-{token}.tmp"),
        &intent,
        PRIVATE_FILE_MODE,
    )?;
    let result = rooted.probe_rename_flags(&active, token);
    if result.is_ok()
        || result
            .as_ref()
            .is_err_and(|error| error.kind() == GeneratorErrorKind::UnsupportedPlatform)
    {
        let intent_path = format!("{active}/intent.json");
        if let Some(identity) = rooted.identity_at(&intent_path)? {
            rooted.remove_file_exact(&intent_path, &identity)?;
        }
        rooted.remove_dir_exact(&active, &active_identity)?;
    }
    result
}

fn recover_probe_journals(rooted: &RootedFs, domain: Domain) -> Result<()> {
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
            transaction_error("recover rename capability probe", "probe journal disappeared")
        })?;
        for member in rooted.list_dir(&active)? {
            let member_path = format!("{active}/{member}");
            let identity = rooted.identity_at(&member_path)?.ok_or_else(|| {
                transaction_error("recover rename capability probe", "probe member disappeared")
            })?;
            if member == "intent.json" || member.ends_with(".tmp") {
                rooted.remove_file_exact(&member_path, &identity)?;
            } else if member.starts_with("probe-") && identity.kind() == NodeKind::Directory {
                rooted.remove_dir_exact(&member_path, &identity)?;
            } else {
                return Err(transaction_error(
                    "recover rename capability probe",
                    format!("unknown or replaced probe member: {member}"),
                ));
            }
        }
        rooted.remove_dir_exact(&active, &active_identity)?;
    }
    Ok(())
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

fn install_owner_record(
    rooted: &RootedFs,
    location: &CorpusLocation,
    domain: Domain,
    metadata: &LeaseMetadata,
    token: &str,
    authority_key: &str,
) -> Result<()> {
    let parent = format!(
        ".surgeist-generator/leases/{}/{}",
        domain.as_str(),
        OWNER_TRANSACTIONS
    );
    let active = format!("{parent}/active-{token}");
    let active_identity = rooted.create_dir_exclusive(&active, PRIVATE_DIRECTORY_MODE)?;
    let owner = OwnerRecord {
        schema_version: 1,
        generator: metadata.generator.clone(),
        pid: std::process::id(),
        owner_root: location.owner_root().display().to_string(),
        corpus_root: location.corpus_root().display().to_string(),
        scope: metadata.scope.clone(),
        command: metadata.command.clone(),
        unix_start_time: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|error| {
                transaction_error(
                    "construct historical generation owner",
                    format!("system clock precedes Unix epoch: {error}"),
                )
            })?
            .as_secs(),
    };
    let owner_bytes = canonical_json(&owner, "serialize historical generation owner")?;
    let owner_path = owner_path(domain);
    let (old_digest, old_identity) = if rooted.exists(&owner_path)? {
        (
            Some(Sha256Digest::from_bytes(
                rooted.read_file(&owner_path, PRIVATE_FILE_MODE)?,
            )),
            rooted.identity_at(&owner_path)?,
        )
    } else {
        (None, None)
    };
    let stage_path = format!("{active}/owner.stage");
    let intent = OwnerIntent {
        schema_version: 1,
        authority_key: authority_key.to_owned(),
        token: token.to_owned(),
        owner_path: owner_path.clone(),
        stage_path: stage_path.clone(),
        old_digest,
        old_identity,
        new_digest: Sha256Digest::from_bytes(&owner_bytes),
    };
    rooted.publish_file_exclusive(
        &active,
        "intent.json",
        &format!("intent-{token}.tmp"),
        &canonical_json(&intent, "serialize owner-record intent")?,
        PRIVATE_FILE_MODE,
    )?;
    let mut stage = rooted.create_file_handle_exclusive(&stage_path, b"", PRIVATE_FILE_MODE)?;
    let stage_identity = rooted.identity_of_handle(&stage)?;
    rooted.publish_file_exclusive(
        &active,
        "stage-registration.json",
        &format!("stage-registration-{token}.tmp"),
        &canonical_json(&stage_identity, "serialize owner-stage registration")?,
        PRIVATE_FILE_MODE,
    )?;
    stage.write_all(&owner_bytes).map_err(|source| {
        transaction_source("write historical generation owner stage", &stage_path, source)
    })?;
    stage.flush().map_err(|source| {
        transaction_source("flush historical generation owner stage", &stage_path, source)
    })?;
    stage.sync_all().map_err(|source| {
        transaction_source("sync historical generation owner stage", &stage_path, source)
    })?;
    rooted.validate_handle_at(&stage_path, &stage, PRIVATE_FILE_MODE)?;
    drop(stage);
    rooted.publish_file_exclusive(
        &active,
        "prepared.json",
        &format!("prepared-{token}.tmp"),
        &canonical_json(&intent.new_digest, "serialize owner prepared marker")?,
        PRIVATE_FILE_MODE,
    )?;
    if let Some(old_identity) = &intent.old_identity {
        rooted.rename_swap_bound(
            &stage_path,
            &owner_path,
            &stage_identity,
            old_identity,
        )?;
    } else {
        rooted.rename_exclusive_bound(&stage_path, &owner_path, &stage_identity)?;
    }
    rooted.sync_dir(&format!(".surgeist-generator/leases/{}", domain.as_str()))?;
    rooted.publish_file_exclusive(
        &active,
        "committed",
        &format!("committed-{token}.tmp"),
        &canonical_json(&intent.new_digest, "serialize owner committed marker")?,
        PRIVATE_FILE_MODE,
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
    cleanup_simple_journal(rooted, &active, active_identity)
}

fn recover_owner_transactions(
    rooted: &RootedFs,
    domain: Domain,
    authority_key: &str,
) -> Result<()> {
    let parent = format!(
        ".surgeist-generator/leases/{}/{}",
        domain.as_str(),
        OWNER_TRANSACTIONS
    );
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
        if active_identity.kind() != NodeKind::Directory
            || active_identity.mode() != PRIVATE_DIRECTORY_MODE
        {
            return Err(transaction_error(
                "recover owner transaction",
                "owner journal has the wrong type or mode",
            ));
        }
        let names = rooted.list_dir(&active)?;
        if names.is_empty() {
            rooted.remove_dir_exact(&active, &active_identity)?;
            continue;
        }
        if !rooted.exists(&format!("{active}/intent.json"))? {
            let expected = format!("intent-{token}.tmp");
            if names.iter().any(|member| member != &expected) {
                return Err(transaction_error(
                    "recover owner transaction",
                    "owner journal has unknown pre-intent state",
                ));
            }
            for member in names {
                let path = format!("{active}/{member}");
                let identity = rooted.identity_at(&path)?.ok_or_else(|| {
                    transaction_error("recover owner transaction", "intent temp disappeared")
                })?;
                if identity.kind() != NodeKind::Regular
                    || identity.mode() != PRIVATE_FILE_MODE
                    || identity.link_count() != Some(1)
                {
                    return Err(transaction_error(
                        "recover owner transaction",
                        "owner intent temp has the wrong type or mode",
                    ));
                }
                rooted.remove_file_exact(&path, &identity)?;
            }
            rooted.remove_dir_exact(&active, &active_identity)?;
            continue;
        }
        let intent: OwnerIntent = serde_json::from_slice(
            &rooted.read_file(&format!("{active}/intent.json"), PRIVATE_FILE_MODE)?,
        )
        .map_err(|error| {
            transaction_error(
                "recover owner transaction",
                format!("invalid owner intent: {error}"),
            )
        })?;
        let expected_owner = owner_path(domain);
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
        for member in &names {
            let identity = rooted
                .identity_at(&format!("{active}/{member}"))?
                .ok_or_else(|| {
                    transaction_error("recover owner transaction", "journal member disappeared")
                })?;
            if identity.kind() != NodeKind::Regular
                || identity.mode() != PRIVATE_FILE_MODE
                || identity.link_count() != Some(1)
            {
                return Err(transaction_error(
                    "recover owner transaction",
                    format!("owner journal member has the wrong policy: {member}"),
                ));
            }
        }
        let registration = if rooted.exists(&format!("{active}/stage-registration.json"))? {
            Some(
                serde_json::from_slice::<HeldIdentity>(
                    &rooted.read_file(
                        &format!("{active}/stage-registration.json"),
                        PRIVATE_FILE_MODE,
                    )?,
                )
                .map_err(|error| {
                    transaction_error(
                        "recover owner transaction",
                        format!("invalid owner stage registration: {error}"),
                    )
                })?,
            )
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
        let prepared = if rooted.exists(&format!("{active}/prepared.json"))? {
            Some(
                serde_json::from_slice::<Sha256Digest>(
                    &rooted.read_file(&format!("{active}/prepared.json"), PRIVATE_FILE_MODE)?,
                )
                .map_err(|error| {
                    transaction_error(
                        "recover owner transaction",
                        format!("invalid owner prepared marker: {error}"),
                    )
                })?,
            )
        } else {
            None
        };
        if prepared.as_ref().is_some_and(|digest| digest != &intent.new_digest)
            || (prepared.is_some() && registration.is_none())
        {
            return Err(transaction_error(
                "recover owner transaction",
                "owner prepared marker differs from its registration",
            ));
        }
        let owner_digest = if rooted.exists(&intent.owner_path)? {
            Some(Sha256Digest::from_bytes(
                rooted.read_file(&intent.owner_path, PRIVATE_FILE_MODE)?,
            ))
        } else {
            None
        };
        let stage_digest = if rooted.exists(&intent.stage_path)? {
            Some(Sha256Digest::from_bytes(
                rooted.read_file(&intent.stage_path, PRIVATE_FILE_MODE)?,
            ))
        } else {
            None
        };
        if owner_digest == Some(intent.new_digest.clone()) {
            if prepared.is_none() || registration.is_none() {
                return Err(transaction_error(
                    "recover owner transaction",
                    "new owner is visible without prepared registration",
                ));
            }
            validate_owner_outcome(rooted, &active, token, &intent.new_digest, true)?;
            match (&intent.old_digest, &intent.old_identity, stage_digest) {
                (Some(old_digest), Some(old_identity), Some(stage_digest))
                    if &stage_digest == old_digest =>
                {
                    let actual = rooted.identity_at(&intent.stage_path)?.ok_or_else(|| {
                        transaction_error("recover owner transaction", "old owner stage vanished")
                    })?;
                    if !old_identity.matches_recovery(&actual) {
                        return Err(transaction_error(
                            "recover owner transaction",
                            "old owner stage identity changed",
                        ));
                    }
                    rooted.remove_file_exact(&intent.stage_path, old_identity)?;
                }
                (None, None, None) => {}
                _ => {
                    return Err(transaction_error(
                        "recover owner transaction",
                        "post-commit owner stage differs from the durable old owner",
                    ));
                }
            }
        } else if owner_digest == intent.old_digest {
            validate_owner_outcome(rooted, &active, token, &intent.new_digest, false)?;
            if let Some(stage_digest) = stage_digest {
                let actual = rooted.identity_at(&intent.stage_path)?.ok_or_else(|| {
                    transaction_error("recover owner transaction", "owner stage disappeared")
                })?;
                if let Some(registration) = registration.as_ref() {
                    if !registration.matches_recovery(&actual) {
                        return Err(transaction_error(
                            "recover owner transaction",
                            "pre-commit owner stage identity changed",
                        ));
                    }
                    if prepared.is_some() && stage_digest != intent.new_digest {
                        return Err(transaction_error(
                            "recover owner transaction",
                            "prepared owner stage bytes differ",
                        ));
                    }
                } else {
                    if prepared.is_some()
                        || !rooted
                            .read_file(&intent.stage_path, PRIVATE_FILE_MODE)?
                            .is_empty()
                    {
                        return Err(transaction_error(
                            "recover owner transaction",
                            "nonempty owner stage exists without registration",
                        ));
                    }
                }
                rooted.remove_file_exact(&intent.stage_path, &actual)?;
            } else if registration.is_some()
                && !rooted.exists(&format!("{active}/aborted"))?
            {
                return Err(transaction_error(
                    "recover owner transaction",
                    "registered owner stage disappeared before abort",
                ));
            }
        } else {
            return Err(transaction_error(
                "recover owner transaction",
                "owner/stage contents match neither durable outcome",
            ));
        }
        cleanup_simple_journal(rooted, &active, active_identity)?;
    }
    Ok(())
}

fn validate_owner_outcome(
    rooted: &RootedFs,
    active: &str,
    token: &str,
    digest: &Sha256Digest,
    committed: bool,
) -> Result<()> {
    let (expected, opposite) = if committed {
        ("committed", "aborted")
    } else {
        ("aborted", "committed")
    };
    if rooted.exists(&format!("{active}/{opposite}"))? {
        return Err(transaction_error(
            "recover owner transaction",
            "owner outcome marker conflicts with visible state",
        ));
    }
    if rooted.exists(&format!("{active}/{expected}"))? {
        let recorded: Sha256Digest = serde_json::from_slice(
            &rooted.read_file(&format!("{active}/{expected}"), PRIVATE_FILE_MODE)?,
        )
        .map_err(|error| {
            transaction_error(
                "recover owner transaction",
                format!("invalid owner outcome marker: {error}"),
            )
        })?;
        if &recorded != digest {
            return Err(transaction_error(
                "recover owner transaction",
                "owner outcome digest differs",
            ));
        }
        return Ok(());
    }
    rooted.publish_file_exclusive(
        active,
        expected,
        &format!("{expected}-{token}.tmp"),
        &canonical_json(digest, "serialize recovered owner outcome")?,
        PRIVATE_FILE_MODE,
    )?;
    Ok(())
}

fn cleanup_simple_journal(
    rooted: &RootedFs,
    journal: &str,
    journal_identity: HeldIdentity,
) -> Result<()> {
    for name in rooted.list_dir(journal)? {
        let path = format!("{journal}/{name}");
        let identity = rooted.identity_at(&path)?.ok_or_else(|| {
            transaction_error("clean private journal", format!("member disappeared: {name}"))
        })?;
        if identity.kind() != NodeKind::Regular || identity.mode() != PRIVATE_FILE_MODE {
            return Err(transaction_error(
                "clean private journal",
                format!("invalid journal member: {name}"),
            ));
        }
        rooted.remove_file_exact(&path, &identity)?;
    }
    rooted.remove_dir_exact(journal, &journal_identity)
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
    format!(
        ".surgeist-generator/leases/{}/mutex.lock",
        domain.as_str()
    )
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
        write!(&mut token, "{byte:02x}").map_err(|error| {
            transaction_error("format transaction token", error.to_string())
        })?;
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
    let pid = value.parse::<u32>().map_err(|_| {
        transaction_error("parse bootstrap PID", format!("invalid PID: {value}"))
    })?;
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
}
