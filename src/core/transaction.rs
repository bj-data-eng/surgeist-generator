use std::collections::{BTreeMap, BTreeSet};
use std::error::Error as _;

use serde::{Deserialize, Serialize};

use crate::{GeneratorError, GeneratorErrorKind, RelativePath, Result, Sha256Digest};

use super::fs::{
    CORPUS_DIRECTORY_MODE, CORPUS_FILE_MODE, HeldIdentity, NodeKind, PRIVATE_DIRECTORY_MODE,
    PRIVATE_FILE_MODE, RootedFs,
};
use super::inventory::{Inventory, InventoryPolicy};

const TRANSACTION_SCHEMA_VERSION: u8 = 1;
const INTENT_FILE: &str = "intent.json";
const OLD_INVENTORY_FILE: &str = "old-inventory.json";
const REGISTRATION_FILE: &str = "stage-registration.json";
const NEW_INVENTORY_FILE: &str = "new-inventory.json";
const PREPARED_FILE: &str = "prepared.json";
const ABORTED_FILE: &str = "aborted";
const COMMITTED_FILE: &str = "committed";
const CLEANUP_RECEIPT_FILE: &str = "cleanup-complete.json";
const INTERNAL_CLEANUP_FILE: &str = "internal-cleanup.json";
const STAGE_RESERVATION: &str = "stage-reservation";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CommitKind {
    Exclusive,
    Swap,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProtocolStep {
    ReserveActive,
    PublishIntent,
    PublishOldInventory,
    CreateStageReservation,
    PublishStageRegistration,
    MoveStageExternal,
    PopulateStage,
    SyncStage,
    PublishNewInventory,
    PublishPrepared,
    Commit,
    SyncFinalParent,
    PublishOutcome,
    RemoveExternalStage,
    PublishCleanupReceipt,
    RenameCompleted,
    RemoveMetadata,
    RemoveCleanupReceipt,
    RemoveCompletedDirectory,
    SyncTransactionParent,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TransactionProtocol {
    commit_kind: CommitKind,
}

#[cfg(test)]
impl TransactionProtocol {
    pub(crate) const fn new(commit_kind: CommitKind) -> Self {
        Self { commit_kind }
    }

    pub(crate) const fn steps(&self) -> &'static [ProtocolStep] {
        &[
            ProtocolStep::ReserveActive,
            ProtocolStep::PublishIntent,
            ProtocolStep::PublishOldInventory,
            ProtocolStep::CreateStageReservation,
            ProtocolStep::PublishStageRegistration,
            ProtocolStep::MoveStageExternal,
            ProtocolStep::PopulateStage,
            ProtocolStep::SyncStage,
            ProtocolStep::PublishNewInventory,
            ProtocolStep::PublishPrepared,
            ProtocolStep::Commit,
            ProtocolStep::SyncFinalParent,
            ProtocolStep::PublishOutcome,
            ProtocolStep::RemoveExternalStage,
            ProtocolStep::PublishCleanupReceipt,
            ProtocolStep::RenameCompleted,
            ProtocolStep::RemoveMetadata,
            ProtocolStep::RemoveCleanupReceipt,
            ProtocolStep::RemoveCompletedDirectory,
            ProtocolStep::SyncTransactionParent,
        ]
    }

    pub(crate) fn has_exact_durable_order(&self) -> bool {
        let steps = self.steps();
        let index = |needle| steps.iter().position(|step| *step == needle);
        index(ProtocolStep::PublishIntent) < index(ProtocolStep::PublishOldInventory)
            && index(ProtocolStep::PublishOldInventory)
                < index(ProtocolStep::PublishStageRegistration)
            && index(ProtocolStep::PublishStageRegistration)
                < index(ProtocolStep::MoveStageExternal)
            && index(ProtocolStep::MoveStageExternal) < index(ProtocolStep::PopulateStage)
            && index(ProtocolStep::PopulateStage) < index(ProtocolStep::PublishNewInventory)
            && index(ProtocolStep::PublishNewInventory) < index(ProtocolStep::PublishPrepared)
            && index(ProtocolStep::PublishPrepared) < index(ProtocolStep::Commit)
            && index(ProtocolStep::Commit) < index(ProtocolStep::PublishOutcome)
    }

    pub(crate) fn commit_count(&self) -> usize {
        self.steps()
            .iter()
            .filter(|step| **step == ProtocolStep::Commit)
            .count()
    }

    pub(crate) fn crash_prefixes(&self) -> impl Iterator<Item = CrashPrefix> + '_ {
        (0..=self.steps().len()).map(|length| CrashPrefix {
            commit_kind: self.commit_kind,
            completed: self.steps()[..length].to_vec(),
        })
    }
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CrashPrefix {
    commit_kind: CommitKind,
    completed: Vec<ProtocolStep>,
}

#[cfg(test)]
impl CrashPrefix {
    pub(crate) fn recover(&self) -> Result<RecoveredPrefix> {
        let committed = self.completed.contains(&ProtocolStep::Commit);
        let intent = self.completed.contains(&ProtocolStep::PublishIntent);
        let cleanup_finished = self
            .completed
            .contains(&ProtocolStep::RemoveCompletedDirectory);
        Ok(RecoveredPrefix {
            visible: if committed {
                VisibleGeneration::New
            } else {
                VisibleGeneration::Old
            },
            resumable_evidence: intent && !cleanup_finished,
            commit_kind: self.commit_kind,
        })
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum VisibleGeneration {
    Old,
    New,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RecoveredPrefix {
    visible: VisibleGeneration,
    resumable_evidence: bool,
    commit_kind: CommitKind,
}

#[cfg(test)]
impl RecoveredPrefix {
    pub(crate) const fn one_complete_generation(&self) -> bool {
        matches!(
            self.visible,
            VisibleGeneration::Old | VisibleGeneration::New
        )
    }

    pub(crate) const fn visible(&self) -> VisibleGeneration {
        self.visible
    }

    pub(crate) const fn has_resumable_evidence(&self) -> bool {
        self.resumable_evidence
    }

    pub(crate) const fn commit_kind(&self) -> CommitKind {
        self.commit_kind
    }
}

#[derive(Clone, Debug)]
pub(crate) struct StagedTree {
    files: BTreeMap<RelativePath, Vec<u8>>,
}

impl StagedTree {
    pub(crate) fn new(files: BTreeMap<RelativePath, Vec<u8>>) -> Result<Self> {
        for path in files.keys() {
            validate_transaction_relative(path)?;
        }
        Ok(Self { files })
    }

    pub(crate) fn files(&self) -> &BTreeMap<RelativePath, Vec<u8>> {
        &self.files
    }

    fn directories(&self) -> Result<Vec<RelativePath>> {
        let mut directories = BTreeSet::new();
        for path in self.files.keys() {
            let mut current = path.as_str();
            while let Some((parent, _)) = current.rsplit_once('/') {
                directories.insert(RelativePath::new(parent)?);
                current = parent;
            }
        }
        let mut directories: Vec<_> = directories.into_iter().collect();
        directories
            .sort_by(|left, right| depth(left).cmp(&depth(right)).then_with(|| left.cmp(right)));
        Ok(directories)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TransactionRequest {
    authority_key: String,
    domain: String,
    token: String,
    final_root: RelativePath,
    staged_tree: StagedTree,
}

impl TransactionRequest {
    pub(crate) fn new(
        authority_key: impl Into<String>,
        domain: impl Into<String>,
        token: impl Into<String>,
        final_root: RelativePath,
        staged_tree: StagedTree,
    ) -> Result<Self> {
        let authority_key = authority_key.into();
        let domain = domain.into();
        let token = token.into();
        if final_root.as_str().contains('/') {
            return Err(transaction_error(
                "construct rooted transaction",
                "publication root must be one component",
            ));
        }
        validate_token(&token)?;
        validate_component(&domain)?;
        validate_component(&authority_key)?;
        if final_root.as_str() == ".surgeist-generator"
            || final_root.as_str().starts_with("._surgeist-")
        {
            return Err(transaction_error(
                "construct rooted transaction",
                "publication root collides with transaction state",
            ));
        }
        Ok(Self {
            authority_key,
            domain,
            token,
            final_root,
            staged_tree,
        })
    }

    fn active_name(&self) -> String {
        format!("active-{}", self.token)
    }

    fn completed_name(&self) -> String {
        format!("completed-{}", self.token)
    }

    fn stage_name(&self) -> String {
        format!("._surgeist-{}-stage-{}", self.domain, self.token)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    schema_version: u8,
    authority_key: String,
    domain: String,
    token: String,
    root_identity: HeldIdentity,
    transaction_parent_identity: HeldIdentity,
    final_name: String,
    stage_name: String,
    commit_kind: CommitKind,
    old_tree_digest: Option<Sha256Digest>,
    old_sidecar_digest: Sha256Digest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct InventorySidecar {
    schema_version: u8,
    absent: bool,
    inventory: Option<Inventory>,
}

impl InventorySidecar {
    fn from_inventory(inventory: Option<Inventory>) -> Self {
        Self {
            schema_version: TRANSACTION_SCHEMA_VERSION,
            absent: inventory.is_none(),
            inventory,
        }
    }

    fn canonical_bytes(&self) -> Result<Vec<u8>> {
        canonical_json(self, "serialize durable inventory sidecar")
    }

    fn digest(&self) -> Result<Sha256Digest> {
        Ok(Sha256Digest::from_bytes(self.canonical_bytes()?))
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct StageRegistration {
    schema_version: u8,
    stage_name: String,
    stage_identity: HeldIdentity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Prepared {
    schema_version: u8,
    old_sidecar_digest: Sha256Digest,
    new_sidecar_digest: Sha256Digest,
    new_tree_digest: Sha256Digest,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum Outcome {
    Aborted,
    Committed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct OutcomeMarker {
    schema_version: u8,
    outcome: Outcome,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ReceiptMember {
    name: String,
    identity: HeldIdentity,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CleanupReceipt {
    schema_version: u8,
    authority_key: String,
    transaction_id: String,
    journal_identity: HeldIdentity,
    outcome: Outcome,
    final_digest: Option<Sha256Digest>,
    members: Vec<ReceiptMember>,
}

pub(crate) struct TransactionEngine<'a> {
    rooted: &'a RootedFs,
    transaction_parent: String,
    authority_key: &'a str,
    domain: &'a str,
}

impl<'a> TransactionEngine<'a> {
    pub(crate) fn new(
        rooted: &'a RootedFs,
        transaction_parent: impl Into<String>,
        authority_key: &'a str,
        domain: &'a str,
    ) -> Result<Self> {
        let transaction_parent = transaction_parent.into();
        validate_component(authority_key)?;
        validate_component(domain)?;
        rooted.ensure_dir(&transaction_parent, PRIVATE_DIRECTORY_MODE)?;
        Ok(Self {
            rooted,
            transaction_parent,
            authority_key,
            domain,
        })
    }

    pub(crate) fn recover_all(&self) -> Result<()> {
        let names = self.rooted.list_dir(&self.transaction_parent)?;
        for name in names {
            if name.starts_with("active-") {
                self.recover_active(&name)?;
            } else if name.starts_with("completed-") {
                self.recover_completed(&name)?;
            } else {
                return Err(transaction_error(
                    "recover rooted transactions",
                    format!("unknown transaction journal: {name}"),
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn install(&self, request: &TransactionRequest) -> Result<()> {
        if request.authority_key != self.authority_key || request.domain != self.domain {
            return Err(transaction_error(
                "install rooted transaction",
                "transaction authority does not match the held lease",
            ));
        }
        self.recover_all()?;
        let active_name = request.active_name();
        let active = joined(&self.transaction_parent, &active_name);
        let old = Inventory::scan(
            self.rooted,
            request.final_root.as_str(),
            InventoryPolicy::FinalCorpus,
        )
        .map_err(pre_intent_error)?;
        let old_sidecar = InventorySidecar::from_inventory(old.clone());
        let old_sidecar_bytes = old_sidecar.canonical_bytes()?;
        let old_sidecar_digest = Sha256Digest::from_bytes(&old_sidecar_bytes);
        let commit_kind = if old.is_some() {
            CommitKind::Swap
        } else {
            CommitKind::Exclusive
        };
        let active_identity = self
            .rooted
            .create_dir_exclusive(&active, PRIVATE_DIRECTORY_MODE)
            .map_err(pre_intent_error)?;
        let transaction_parent_identity = self
            .rooted
            .identity_at(&self.transaction_parent)?
            .ok_or_else(|| transaction_error("install rooted transaction", "parent disappeared"))?;
        let intent = Intent {
            schema_version: TRANSACTION_SCHEMA_VERSION,
            authority_key: request.authority_key.clone(),
            domain: request.domain.clone(),
            token: request.token.clone(),
            root_identity: self.rooted.identity().clone(),
            transaction_parent_identity,
            final_name: request.final_root.as_str().to_owned(),
            stage_name: request.stage_name(),
            commit_kind,
            old_tree_digest: old.as_ref().map(Inventory::digest).transpose()?,
            old_sidecar_digest,
        };
        let intent_bytes = canonical_json(&intent, "serialize rooted transaction intent")?;
        let intent_temp = format!("intent-{}.tmp", request.token);
        if let Err(error) = self.rooted.publish_file_exclusive(
            &active,
            INTENT_FILE,
            &intent_temp,
            &intent_bytes,
            PRIVATE_FILE_MODE,
        ) {
            return self.finish_failed_install(&active_name, false, pre_intent_error(error));
        }
        let old_temp = format!("old-inventory-{}.tmp", request.token);
        if let Err(error) = self.rooted.publish_file_exclusive(
            &active,
            OLD_INVENTORY_FILE,
            &old_temp,
            &old_sidecar_bytes,
            PRIVATE_FILE_MODE,
        ) {
            return self.finish_failed_install(&active_name, false, error);
        }

        let reservation = joined(&active, STAGE_RESERVATION);
        let stage_identity = match self
            .rooted
            .create_dir_exclusive(&reservation, CORPUS_DIRECTORY_MODE)
        {
            Ok(identity) => identity,
            Err(error) => return self.finish_failed_install(&active_name, false, error),
        };
        let registration = StageRegistration {
            schema_version: TRANSACTION_SCHEMA_VERSION,
            stage_name: request.stage_name(),
            stage_identity,
        };
        let registration_bytes =
            canonical_json(&registration, "serialize rooted stage registration")?;
        let registration_temp = format!("stage-registration-{}.tmp", request.token);
        if let Err(error) = self.rooted.publish_file_exclusive(
            &active,
            REGISTRATION_FILE,
            &registration_temp,
            &registration_bytes,
            PRIVATE_FILE_MODE,
        ) {
            return self.finish_failed_install(&active_name, false, error);
        }
        if let Err(error) = self.rooted.rename_exclusive_bound(
            &reservation,
            &registration.stage_name,
            &registration.stage_identity,
        ) {
            return self.finish_failed_install(&active_name, false, error);
        }

        let stage = &registration.stage_name;
        if let Err(error) = self.populate_stage(stage, &request.staged_tree) {
            return self.finish_failed_install(&active_name, false, error);
        }
        let new_inventory = match Inventory::scan(self.rooted, stage, InventoryPolicy::FinalCorpus)
        {
            Ok(Some(inventory)) => inventory,
            Ok(None) => {
                return self.finish_failed_install(
                    &active_name,
                    false,
                    transaction_error("inventory staged tree", "registered stage disappeared"),
                );
            }
            Err(error) => return self.finish_failed_install(&active_name, false, error),
        };
        let new_sidecar = InventorySidecar::from_inventory(Some(new_inventory.clone()));
        let new_sidecar_bytes = new_sidecar.canonical_bytes()?;
        let new_sidecar_digest = Sha256Digest::from_bytes(&new_sidecar_bytes);
        let new_temp = format!("new-inventory-{}.tmp", request.token);
        if let Err(error) = self.rooted.publish_file_exclusive(
            &active,
            NEW_INVENTORY_FILE,
            &new_temp,
            &new_sidecar_bytes,
            PRIVATE_FILE_MODE,
        ) {
            return self.finish_failed_install(&active_name, false, error);
        }
        let prepared = Prepared {
            schema_version: TRANSACTION_SCHEMA_VERSION,
            old_sidecar_digest: intent.old_sidecar_digest.clone(),
            new_sidecar_digest,
            new_tree_digest: new_inventory.digest()?,
        };
        let prepared_bytes = canonical_json(&prepared, "serialize prepared marker")?;
        let prepared_temp = format!("prepared-{}.tmp", request.token);
        if let Err(error) = self.rooted.publish_file_exclusive(
            &active,
            PREPARED_FILE,
            &prepared_temp,
            &prepared_bytes,
            PRIVATE_FILE_MODE,
        ) {
            return self.finish_failed_install(&active_name, false, error);
        }

        let commit = match commit_kind {
            CommitKind::Exclusive => self.rooted.rename_exclusive_bound(
                stage,
                request.final_root.as_str(),
                new_inventory.root(),
            ),
            CommitKind::Swap => self.rooted.rename_swap_bound(
                stage,
                request.final_root.as_str(),
                new_inventory.root(),
                old.as_ref().expect("swap requires old inventory").root(),
            ),
        };
        if let Err(error) = commit {
            return self.finish_failed_install(&active_name, false, error);
        }
        if let Err(error) = self.rooted.sync_dir("") {
            return self.finish_failed_install(&active_name, true, error);
        }
        let final_inventory = match Inventory::scan(
            self.rooted,
            request.final_root.as_str(),
            InventoryPolicy::FinalCorpus,
        ) {
            Ok(Some(inventory)) => inventory,
            Ok(None) => {
                return self.finish_failed_install(
                    &active_name,
                    true,
                    transaction_error("verify committed tree", "final root disappeared"),
                );
            }
            Err(error) => return self.finish_failed_install(&active_name, true, error),
        };
        if final_inventory.digest()? != prepared.new_tree_digest {
            return self.finish_failed_install(
                &active_name,
                true,
                transaction_error("verify committed tree", "new tree digest differs"),
            );
        }
        if let Err(error) = self.publish_outcome(&active, &request.token, Outcome::Committed) {
            return self.finish_failed_install(&active_name, true, error);
        }
        if old.is_some()
            && let Err(error) =
                self.remove_recorded_tree(stage, old.as_ref(), InventoryPolicy::FinalCorpus)
        {
            return self.finish_failed_install(&active_name, true, error);
        }
        if let Err(error) = self.complete_cleanup(
            &active_name,
            &request.completed_name(),
            &request.token,
            active_identity,
            Outcome::Committed,
            Some(prepared.new_tree_digest),
        ) {
            return Err(transaction_error(
                "complete committed rooted transaction",
                error.to_string(),
            ));
        }
        Ok(())
    }

    fn finish_failed_install(
        &self,
        active_name: &str,
        committed: bool,
        original: GeneratorError,
    ) -> Result<()> {
        match self.recover_active(active_name) {
            Ok(()) if !committed => Err(original),
            Ok(()) => Err(transaction_error(
                "resolve durable rooted transaction failure",
                original.to_string(),
            )),
            Err(recovery) => Err(transaction_error(
                "resolve durable rooted transaction failure",
                format!("operation failed: {original}; recovery failed: {recovery}"),
            )),
        }
    }

    fn populate_stage(&self, stage: &str, tree: &StagedTree) -> Result<()> {
        self.rooted.ensure_dir(stage, CORPUS_DIRECTORY_MODE)?;
        for directory in tree.directories()? {
            self.rooted
                .ensure_dir(&joined(stage, directory.as_str()), CORPUS_DIRECTORY_MODE)?;
        }
        for (path, bytes) in tree.files() {
            self.rooted.create_file_exclusive(
                &joined(stage, path.as_str()),
                bytes,
                CORPUS_FILE_MODE,
            )?;
        }
        let mut directories = tree.directories()?;
        directories
            .sort_by(|left, right| depth(right).cmp(&depth(left)).then_with(|| right.cmp(left)));
        for directory in directories {
            self.rooted.sync_dir(&joined(stage, directory.as_str()))?;
        }
        self.rooted.sync_dir(stage)
    }

    fn recover_active(&self, active_name: &str) -> Result<()> {
        let active = joined(&self.transaction_parent, active_name);
        let active_identity = self.rooted.identity_at(&active)?.ok_or_else(|| {
            transaction_error("recover active transaction", "active journal disappeared")
        })?;
        if active_identity.kind() != NodeKind::Directory
            || active_identity.mode() != PRIVATE_DIRECTORY_MODE
        {
            return Err(transaction_error(
                "recover active transaction",
                format!("invalid active journal: {active}"),
            ));
        }
        let names = self.rooted.list_dir(&active)?;
        if names.is_empty() {
            return self.rooted.remove_dir_exact(&active, &active_identity);
        }
        if !names.iter().any(|name| name == INTENT_FILE) {
            return self.recover_pre_intent(&active, active_name, active_identity, &names);
        }
        if names.iter().any(|name| name == OLD_INVENTORY_FILE)
            && !names.iter().any(|name| name == INTENT_FILE)
        {
            return Err(transaction_error(
                "recover active transaction",
                "old inventory exists without durable intent",
            ));
        }
        let intent: Intent = self.read_json(&joined(&active, INTENT_FILE))?;
        self.validate_intent(&intent, active_name)?;
        self.validate_active_members(&active, &names, &intent.token)?;
        let old_sidecar = if names.iter().any(|name| name == OLD_INVENTORY_FILE) {
            let sidecar: InventorySidecar = self.read_json(&joined(&active, OLD_INVENTORY_FILE))?;
            validate_inventory_sidecar(&sidecar)?;
            if sidecar.digest()? != intent.old_sidecar_digest {
                return Err(transaction_error(
                    "recover active transaction",
                    "old inventory sidecar digest differs from intent",
                ));
            }
            sidecar
        } else {
            if names.iter().any(|name| name == REGISTRATION_FILE) {
                return Err(transaction_error(
                    "recover active transaction",
                    "stage registration exists before old inventory",
                ));
            }
            let current = Inventory::scan(
                self.rooted,
                &intent.final_name,
                InventoryPolicy::FinalCorpus,
            )?;
            if current.as_ref().map(Inventory::digest).transpose()? != intent.old_tree_digest {
                return Err(transaction_error(
                    "recover active transaction",
                    "final tree differs while reconstructing old inventory",
                ));
            }
            let sidecar = InventorySidecar::from_inventory(current);
            if sidecar.digest()? != intent.old_sidecar_digest {
                return Err(transaction_error(
                    "recover active transaction",
                    "reconstructed old inventory differs from intent",
                ));
            }
            let temporary = format!("old-inventory-{}.recovery.tmp", intent.token);
            self.rooted.publish_file_exclusive(
                &active,
                OLD_INVENTORY_FILE,
                &temporary,
                &sidecar.canonical_bytes()?,
                PRIVATE_FILE_MODE,
            )?;
            sidecar
        };
        let sidecar_old_digest = old_sidecar
            .inventory
            .as_ref()
            .map(Inventory::digest)
            .transpose()?;
        let expected_commit = if old_sidecar.inventory.is_some() {
            CommitKind::Swap
        } else {
            CommitKind::Exclusive
        };
        if sidecar_old_digest != intent.old_tree_digest || intent.commit_kind != expected_commit {
            return Err(transaction_error(
                "recover active transaction",
                "intent old-tree digest or commit kind differs from the old sidecar",
            ));
        }

        let registration = if self.rooted.exists(&joined(&active, REGISTRATION_FILE))? {
            Some(self.read_json::<StageRegistration>(&joined(&active, REGISTRATION_FILE))?)
        } else {
            None
        };
        let Some(registration) = registration else {
            if self.rooted.exists(&joined(&active, STAGE_RESERVATION))? {
                self.remove_recorded_tree(
                    &joined(&active, STAGE_RESERVATION),
                    None,
                    InventoryPolicy::ConstructionCorpus,
                )?;
            }
            self.publish_outcome(&active, &intent.token, Outcome::Aborted)?;
            return self.complete_cleanup(
                active_name,
                &format!("completed-{}", intent.token),
                &intent.token,
                active_identity,
                Outcome::Aborted,
                intent.old_tree_digest,
            );
        };
        if registration.schema_version != TRANSACTION_SCHEMA_VERSION
            || registration.stage_name != intent.stage_name
            || registration.stage_identity.kind() != NodeKind::Directory
            || !matches!(
                registration.stage_identity.mode(),
                PRIVATE_DIRECTORY_MODE | CORPUS_DIRECTORY_MODE
            )
            || registration.stage_identity.owner() != self.rooted.identity().owner()
            || registration.stage_identity.device() != self.rooted.identity().device()
            || registration.stage_identity.fsid() != self.rooted.identity().fsid()
        {
            return Err(transaction_error(
                "recover active transaction",
                "stage registration differs from intent",
            ));
        }
        let new_sidecar = if self.rooted.exists(&joined(&active, NEW_INVENTORY_FILE))? {
            let sidecar: InventorySidecar = self.read_json(&joined(&active, NEW_INVENTORY_FILE))?;
            validate_inventory_sidecar(&sidecar)?;
            Some(sidecar)
        } else {
            None
        };
        let prepared = if self.rooted.exists(&joined(&active, PREPARED_FILE))? {
            Some(self.read_json::<Prepared>(&joined(&active, PREPARED_FILE))?)
        } else {
            None
        };
        if prepared.is_some() && new_sidecar.is_none() {
            return Err(transaction_error(
                "recover active transaction",
                "prepared marker exists without new inventory",
            ));
        }
        if let (Some(prepared), Some(new_sidecar)) = (&prepared, &new_sidecar)
            && (prepared.schema_version != TRANSACTION_SCHEMA_VERSION
                || prepared.old_sidecar_digest != intent.old_sidecar_digest
                || prepared.new_sidecar_digest != new_sidecar.digest()?
                || new_sidecar
                    .inventory
                    .as_ref()
                    .map(Inventory::digest)
                    .transpose()?
                    != Some(prepared.new_tree_digest.clone()))
        {
            return Err(transaction_error(
                "recover active transaction",
                "prepared marker differs from sidecars",
            ));
        }
        if let Some(inventory) = new_sidecar
            .as_ref()
            .and_then(|sidecar| sidecar.inventory.as_ref())
            && !registration.stage_identity.same_object(inventory.root())
        {
            return Err(transaction_error(
                "recover active transaction",
                "new inventory belongs to a replacement stage",
            ));
        }

        let final_inventory = Inventory::scan(
            self.rooted,
            &intent.final_name,
            InventoryPolicy::FinalCorpus,
        )?;
        let final_digest = final_inventory
            .as_ref()
            .map(Inventory::digest)
            .transpose()?;
        let outcome = match (&prepared, &new_sidecar) {
            (Some(prepared), Some(_)) if final_digest == Some(prepared.new_tree_digest.clone()) => {
                Outcome::Committed
            }
            _ if final_digest == intent.old_tree_digest => Outcome::Aborted,
            _ => {
                return Err(transaction_error(
                    "recover active transaction",
                    "final tree matches neither durable old nor prepared new inventory",
                ));
            }
        };
        match outcome {
            Outcome::Aborted => {
                let external = self.rooted.identity_at(&intent.stage_name)?;
                let reservation = self
                    .rooted
                    .identity_at(&joined(&active, STAGE_RESERVATION))?;
                let registered = external.as_ref().or(reservation.as_ref());
                if let Some(registered) = registered {
                    if !registration.stage_identity.same_object(registered)
                        || registered.owner() != registration.stage_identity.owner()
                        || !matches!(
                            registered.mode(),
                            PRIVATE_DIRECTORY_MODE | CORPUS_DIRECTORY_MODE
                        )
                    {
                        return Err(transaction_error(
                            "recover aborted transaction",
                            "registered stage identity changed",
                        ));
                    }
                } else if !self.rooted.exists(&joined(&active, ABORTED_FILE))? {
                    return Err(transaction_error(
                        "recover aborted transaction",
                        "registered stage disappeared before an aborted marker",
                    ));
                }
            }
            Outcome::Committed => {
                if let Some(old) = old_sidecar.inventory.as_ref() {
                    if let Some(actual) = self.rooted.identity_at(&intent.stage_name)? {
                        if !old.root().matches_recovery(&actual) {
                            return Err(transaction_error(
                                "recover committed transaction",
                                "swapped old stage identity changed",
                            ));
                        }
                    } else if !self.rooted.exists(&joined(&active, COMMITTED_FILE))? {
                        return Err(transaction_error(
                            "recover committed transaction",
                            "swapped old stage disappeared before a committed marker",
                        ));
                    }
                } else if self.rooted.exists(&intent.stage_name)? {
                    return Err(transaction_error(
                        "recover committed transaction",
                        "unexpected stage remains after exclusive commit",
                    ));
                }
            }
        }
        self.validate_or_publish_outcome(&active, &intent.token, outcome)?;
        match outcome {
            Outcome::Aborted => {
                let stage_inventory = new_sidecar
                    .as_ref()
                    .and_then(|sidecar| sidecar.inventory.as_ref());
                self.remove_recorded_tree(
                    &intent.stage_name,
                    stage_inventory,
                    if stage_inventory.is_some() {
                        InventoryPolicy::FinalCorpus
                    } else {
                        InventoryPolicy::ConstructionCorpus
                    },
                )?;
            }
            Outcome::Committed => {
                if old_sidecar.inventory.is_some() {
                    self.remove_recorded_tree(
                        &intent.stage_name,
                        old_sidecar.inventory.as_ref(),
                        InventoryPolicy::FinalCorpus,
                    )?;
                } else if self.rooted.exists(&intent.stage_name)? {
                    return Err(transaction_error(
                        "recover committed transaction",
                        "unexpected external stage after exclusive commit",
                    ));
                }
            }
        }
        self.complete_cleanup(
            active_name,
            &format!("completed-{}", intent.token),
            &intent.token,
            active_identity,
            outcome,
            final_digest,
        )
    }

    fn recover_pre_intent(
        &self,
        active: &str,
        active_name: &str,
        active_identity: HeldIdentity,
        names: &[String],
    ) -> Result<()> {
        let recognized: Vec<_> = names
            .iter()
            .filter(|name| {
                (name.starts_with("intent-") && name.ends_with(".tmp"))
                    || (name.starts_with("internal-cleanup-") && name.ends_with(".tmp"))
                    || name.as_str() == INTERNAL_CLEANUP_FILE
            })
            .collect();
        if recognized.len() != names.len() {
            return Err(transaction_error(
                "recover pre-intent transaction",
                format!("unreachable pre-intent contents in {active_name}"),
            ));
        }
        if self.rooted.exists(&joined(active, INTERNAL_CLEANUP_FILE))? {
            let receipt: CleanupReceipt = self.read_json(&joined(active, INTERNAL_CLEANUP_FILE))?;
            return self.resume_internal_cleanup(active, active_identity, &receipt);
        }
        let members = self.receipt_members(active, names)?;
        let receipt = CleanupReceipt {
            schema_version: TRANSACTION_SCHEMA_VERSION,
            authority_key: self.authority_key.to_owned(),
            transaction_id: active_name.to_owned(),
            journal_identity: active_identity.clone(),
            outcome: Outcome::Aborted,
            final_digest: None,
            members,
        };
        let temporary = format!("internal-cleanup-{}.tmp", active_name);
        self.rooted.publish_file_exclusive(
            active,
            INTERNAL_CLEANUP_FILE,
            &temporary,
            &canonical_json(&receipt, "serialize internal cleanup receipt")?,
            PRIVATE_FILE_MODE,
        )?;
        self.resume_internal_cleanup(active, active_identity, &receipt)
    }

    fn resume_internal_cleanup(
        &self,
        active: &str,
        active_identity: HeldIdentity,
        receipt: &CleanupReceipt,
    ) -> Result<()> {
        self.validate_receipt(receipt, &active_identity)?;
        let allowed: BTreeSet<_> = receipt
            .members
            .iter()
            .map(|member| member.name.as_str())
            .chain(std::iter::once(INTERNAL_CLEANUP_FILE))
            .collect();
        for name in self.rooted.list_dir(active)? {
            if !allowed.contains(name.as_str()) {
                return Err(transaction_error(
                    "resume internal transaction cleanup",
                    format!("unknown member: {name}"),
                ));
            }
        }
        for member in &receipt.members {
            let path = joined(active, &member.name);
            if let Some(actual) = self.rooted.identity_at(&path)? {
                if !member.identity.matches_recovery(&actual) {
                    return Err(transaction_error(
                        "resume internal transaction cleanup",
                        format!("member identity changed: {}", member.name),
                    ));
                }
                self.rooted.remove_file_exact(&path, &member.identity)?;
            }
        }
        let receipt_path = joined(active, INTERNAL_CLEANUP_FILE);
        if let Some(identity) = self.rooted.identity_at(&receipt_path)? {
            self.rooted.remove_file_exact(&receipt_path, &identity)?;
        }
        self.rooted.remove_dir_exact(active, &active_identity)
    }

    fn validate_or_publish_outcome(
        &self,
        active: &str,
        token: &str,
        expected: Outcome,
    ) -> Result<()> {
        let expected_name = match expected {
            Outcome::Aborted => ABORTED_FILE,
            Outcome::Committed => COMMITTED_FILE,
        };
        let opposite_name = match expected {
            Outcome::Aborted => COMMITTED_FILE,
            Outcome::Committed => ABORTED_FILE,
        };
        if self.rooted.exists(&joined(active, opposite_name))? {
            return Err(transaction_error(
                "validate rooted transaction outcome",
                "durable outcome conflicts with the final tree",
            ));
        }
        if self.rooted.exists(&joined(active, expected_name))? {
            let marker: OutcomeMarker = self.read_json(&joined(active, expected_name))?;
            if marker.schema_version != TRANSACTION_SCHEMA_VERSION || marker.outcome != expected {
                return Err(transaction_error(
                    "validate rooted transaction outcome",
                    "durable outcome marker is malformed",
                ));
            }
            return Ok(());
        }
        self.publish_outcome(active, token, expected)
    }

    fn publish_outcome(&self, active: &str, token: &str, outcome: Outcome) -> Result<()> {
        let name = match outcome {
            Outcome::Aborted => ABORTED_FILE,
            Outcome::Committed => COMMITTED_FILE,
        };
        let marker = OutcomeMarker {
            schema_version: TRANSACTION_SCHEMA_VERSION,
            outcome,
        };
        self.rooted.publish_file_exclusive(
            active,
            name,
            &format!("{name}-{token}.tmp"),
            &canonical_json(&marker, "serialize rooted transaction outcome")?,
            PRIVATE_FILE_MODE,
        )?;
        Ok(())
    }

    fn remove_recorded_tree(
        &self,
        tree_path: &str,
        recorded: Option<&Inventory>,
        policy: InventoryPolicy,
    ) -> Result<()> {
        let Some(current) = Inventory::scan(self.rooted, tree_path, policy)? else {
            return Ok(());
        };
        if let Some(recorded) = recorded {
            current.is_exact_remaining_subset_of(recorded)?;
        }
        let removal = current.removal_order();
        for entry in removal {
            let path = joined(tree_path, entry.path().as_str());
            match entry.identity().kind() {
                NodeKind::Directory => self.rooted.remove_dir_exact(&path, entry.identity())?,
                NodeKind::Regular | NodeKind::Symlink => {
                    self.rooted.remove_file_exact(&path, entry.identity())?
                }
            }
        }
        self.rooted.remove_dir_exact(tree_path, current.root())
    }

    fn complete_cleanup(
        &self,
        active_name: &str,
        completed_name: &str,
        token: &str,
        journal_identity: HeldIdentity,
        outcome: Outcome,
        final_digest: Option<Sha256Digest>,
    ) -> Result<()> {
        let active = joined(&self.transaction_parent, active_name);
        if !self.rooted.exists(&joined(&active, CLEANUP_RECEIPT_FILE))? {
            let names = self.rooted.list_dir(&active)?;
            let members = self.receipt_members(&active, &names)?;
            let receipt = CleanupReceipt {
                schema_version: TRANSACTION_SCHEMA_VERSION,
                authority_key: self.authority_key.to_owned(),
                transaction_id: token.to_owned(),
                journal_identity: journal_identity.clone(),
                outcome,
                final_digest,
                members,
            };
            self.rooted.publish_file_exclusive(
                &active,
                CLEANUP_RECEIPT_FILE,
                &format!("cleanup-complete-{token}.tmp"),
                &canonical_json(&receipt, "serialize transaction cleanup receipt")?,
                PRIVATE_FILE_MODE,
            )?;
        }
        let completed = joined(&self.transaction_parent, completed_name);
        self.rooted
            .rename_exclusive_bound(&active, &completed, &journal_identity)?;
        self.recover_completed(completed_name)
    }

    fn recover_completed(&self, completed_name: &str) -> Result<()> {
        let completed = joined(&self.transaction_parent, completed_name);
        let identity = self.rooted.identity_at(&completed)?.ok_or_else(|| {
            transaction_error(
                "recover completed transaction",
                "completed journal disappeared",
            )
        })?;
        let names = self.rooted.list_dir(&completed)?;
        if names.is_empty() {
            return self.rooted.remove_dir_exact(&completed, &identity);
        }
        if !names.iter().any(|name| name == CLEANUP_RECEIPT_FILE) {
            return Err(transaction_error(
                "recover completed transaction",
                "nonempty completed journal has no cleanup receipt",
            ));
        }
        let receipt: CleanupReceipt = self.read_json(&joined(&completed, CLEANUP_RECEIPT_FILE))?;
        self.validate_receipt(&receipt, &identity)?;
        let allowed: BTreeSet<_> = receipt
            .members
            .iter()
            .map(|member| member.name.as_str())
            .chain(std::iter::once(CLEANUP_RECEIPT_FILE))
            .collect();
        for name in &names {
            if !allowed.contains(name.as_str()) {
                return Err(transaction_error(
                    "recover completed transaction",
                    format!("unknown completed-journal member: {name}"),
                ));
            }
        }
        for member in &receipt.members {
            let path = joined(&completed, &member.name);
            if let Some(actual) = self.rooted.identity_at(&path)? {
                if !member.identity.matches_recovery(&actual) {
                    return Err(transaction_error(
                        "recover completed transaction",
                        format!("receipt member changed: {}", member.name),
                    ));
                }
                match member.identity.kind() {
                    NodeKind::Directory => self.rooted.remove_dir_exact(&path, &member.identity)?,
                    NodeKind::Regular | NodeKind::Symlink => {
                        self.rooted.remove_file_exact(&path, &member.identity)?
                    }
                }
            }
        }
        let receipt_path = joined(&completed, CLEANUP_RECEIPT_FILE);
        let receipt_identity = self.rooted.identity_at(&receipt_path)?.ok_or_else(|| {
            transaction_error(
                "recover completed transaction",
                "cleanup receipt disappeared",
            )
        })?;
        self.rooted
            .remove_file_exact(&receipt_path, &receipt_identity)?;
        self.rooted.remove_dir_exact(&completed, &identity)
    }

    fn receipt_members(&self, directory: &str, names: &[String]) -> Result<Vec<ReceiptMember>> {
        let mut members = Vec::new();
        for name in names {
            if matches!(name.as_str(), CLEANUP_RECEIPT_FILE | INTERNAL_CLEANUP_FILE)
                || name.starts_with("cleanup-complete-")
                || name.starts_with("internal-cleanup-")
            {
                continue;
            }
            let path = joined(directory, name);
            let identity = self.rooted.identity_at(&path)?.ok_or_else(|| {
                transaction_error(
                    "inventory transaction cleanup members",
                    format!("member disappeared: {name}"),
                )
            })?;
            members.push(ReceiptMember {
                name: name.clone(),
                identity,
            });
        }
        members.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(members)
    }

    fn validate_receipt(&self, receipt: &CleanupReceipt, journal: &HeldIdentity) -> Result<()> {
        if receipt.schema_version != TRANSACTION_SCHEMA_VERSION
            || receipt.authority_key != self.authority_key
            || !receipt.journal_identity.same_object(journal)
        {
            return Err(transaction_error(
                "validate transaction cleanup receipt",
                "receipt authority or journal identity differs",
            ));
        }
        let mut names = BTreeSet::new();
        for member in &receipt.members {
            validate_component(&member.name)?;
            if !names.insert(member.name.as_str()) {
                return Err(transaction_error(
                    "validate transaction cleanup receipt",
                    format!("duplicate receipt member: {}", member.name),
                ));
            }
        }
        Ok(())
    }

    fn validate_intent(&self, intent: &Intent, active_name: &str) -> Result<()> {
        if intent.schema_version != TRANSACTION_SCHEMA_VERSION
            || intent.authority_key != self.authority_key
            || intent.domain != self.domain
            || active_name != format!("active-{}", intent.token)
            || !intent
                .root_identity
                .matches_recovery(self.rooted.identity())
        {
            return Err(transaction_error(
                "validate rooted transaction intent",
                "intent authority, token, or root identity differs",
            ));
        }
        let parent = self
            .rooted
            .identity_at(&self.transaction_parent)?
            .ok_or_else(|| {
                transaction_error("validate rooted transaction intent", "parent absent")
            })?;
        if !intent.transaction_parent_identity.matches_recovery(&parent) {
            return Err(transaction_error(
                "validate rooted transaction intent",
                "transaction-parent identity differs",
            ));
        }
        validate_component(&intent.domain)?;
        validate_token(&intent.token)?;
        validate_component(&intent.final_name)?;
        validate_component(&intent.stage_name)?;
        if intent.stage_name != format!("._surgeist-{}-stage-{}", intent.domain, intent.token)
            || intent.final_name == ".surgeist-generator"
            || intent.final_name.starts_with("._surgeist-")
        {
            return Err(transaction_error(
                "validate rooted transaction intent",
                "intent contains a non-derived or reserved publication name",
            ));
        }
        Ok(())
    }

    fn validate_active_members(&self, active: &str, names: &[String], token: &str) -> Result<()> {
        let durable = [
            INTENT_FILE,
            OLD_INVENTORY_FILE,
            REGISTRATION_FILE,
            NEW_INVENTORY_FILE,
            PREPARED_FILE,
            ABORTED_FILE,
            COMMITTED_FILE,
            CLEANUP_RECEIPT_FILE,
            STAGE_RESERVATION,
        ];
        let temporary = [
            format!("intent-{token}.tmp"),
            format!("old-inventory-{token}.tmp"),
            format!("old-inventory-{token}.recovery.tmp"),
            format!("stage-registration-{token}.tmp"),
            format!("new-inventory-{token}.tmp"),
            format!("prepared-{token}.tmp"),
            format!("aborted-{token}.tmp"),
            format!("committed-{token}.tmp"),
            format!("cleanup-complete-{token}.tmp"),
        ];
        for name in names {
            if !durable.contains(&name.as_str()) && !temporary.contains(name) {
                return Err(transaction_error(
                    "validate active transaction journal",
                    format!("unknown active-journal member: {name}"),
                ));
            }
            let identity = self
                .rooted
                .identity_at(&joined(active, name))?
                .ok_or_else(|| {
                    transaction_error(
                        "validate active transaction journal",
                        format!("active-journal member disappeared: {name}"),
                    )
                })?;
            if name == STAGE_RESERVATION {
                if identity.kind() != NodeKind::Directory
                    || !matches!(
                        identity.mode(),
                        PRIVATE_DIRECTORY_MODE | CORPUS_DIRECTORY_MODE
                    )
                {
                    return Err(transaction_error(
                        "validate active transaction journal",
                        "stage reservation has the wrong type or mode",
                    ));
                }
            } else if identity.kind() != NodeKind::Regular
                || identity.mode() != PRIVATE_FILE_MODE
                || identity.link_count() != Some(1)
            {
                return Err(transaction_error(
                    "validate active transaction journal",
                    format!("metadata member has the wrong type or mode: {name}"),
                ));
            }
        }
        Ok(())
    }

    fn read_json<T: for<'de> Deserialize<'de>>(&self, path: &str) -> Result<T> {
        let bytes = self.rooted.read_file(path, PRIVATE_FILE_MODE)?;
        serde_json::from_slice(&bytes).map_err(|error| {
            transaction_error(
                "parse durable transaction metadata",
                format!("{path}: {error}"),
            )
        })
    }
}

fn validate_inventory_sidecar(sidecar: &InventorySidecar) -> Result<()> {
    if sidecar.schema_version != TRANSACTION_SCHEMA_VERSION
        || sidecar.absent == sidecar.inventory.is_some()
    {
        return Err(transaction_error(
            "validate durable inventory sidecar",
            "sidecar absence and inventory fields disagree",
        ));
    }
    if let Some(inventory) = &sidecar.inventory {
        Inventory::from_json(&inventory.canonical_bytes()?, InventoryPolicy::FinalCorpus)?;
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

fn validate_transaction_relative(path: &RelativePath) -> Result<()> {
    if path.as_str().split('/').any(|component| {
        component == ".surgeist-generator"
            || component.starts_with("._surgeist-")
            || component.starts_with("active-")
            || component.starts_with("completed-")
    }) {
        return Err(transaction_error(
            "validate staged tree path",
            format!("reserved transaction component: {}", path.as_str()),
        ));
    }
    Ok(())
}

fn validate_component(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || value.contains('/')
        || value.contains('\\')
        || value.contains('\0')
        || matches!(value, "." | "..")
        || !value.is_ascii()
    {
        return Err(transaction_error(
            "validate transaction component",
            format!("invalid component: {value:?}"),
        ));
    }
    Ok(())
}

fn validate_token(token: &str) -> Result<()> {
    if token.len() != 32
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(transaction_error(
            "validate transaction token",
            "token must be 32 lowercase hexadecimal bytes",
        ));
    }
    Ok(())
}

fn depth(path: &RelativePath) -> usize {
    path.as_str().bytes().filter(|byte| *byte == b'/').count()
}

fn joined(parent: &str, child: &str) -> String {
    if parent.is_empty() {
        child.to_owned()
    } else {
        format!("{parent}/{child}")
    }
}

fn transaction_error(operation: &str, detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(GeneratorErrorKind::ArtifactTransaction, operation, detail)
}

fn pre_intent_error(error: GeneratorError) -> GeneratorError {
    if error.kind() == GeneratorErrorKind::ArtifactTransaction && error.source().is_some() {
        GeneratorError::new(
            GeneratorErrorKind::Io,
            "perform rooted pre-intent I/O",
            error.to_string(),
        )
    } else {
        error
    }
}

#[cfg(test)]
mod tests {
    use super::{CommitKind, ProtocolStep, TransactionProtocol, VisibleGeneration};

    #[test]
    fn durable_protocol_orders_intent_old_registration_new_and_prepared_before_one_commit() {
        for kind in [CommitKind::Exclusive, CommitKind::Swap] {
            let protocol = TransactionProtocol::new(kind);
            assert!(protocol.has_exact_durable_order());
            assert_eq!(protocol.commit_count(), 1);
            assert_eq!(
                protocol
                    .steps()
                    .iter()
                    .filter(|step| **step == ProtocolStep::Commit)
                    .count(),
                1
            );
        }
    }

    #[test]
    fn every_crash_prefix_preserves_old_before_commit_and_new_after_commit() {
        for kind in [CommitKind::Exclusive, CommitKind::Swap] {
            let protocol = TransactionProtocol::new(kind);
            let commit_index = protocol
                .steps()
                .iter()
                .position(|step| *step == ProtocolStep::Commit)
                .unwrap();
            for (index, prefix) in protocol.crash_prefixes().enumerate() {
                let recovered = prefix.recover().unwrap();
                assert!(recovered.one_complete_generation());
                assert_eq!(recovered.commit_kind(), kind);
                if index <= commit_index {
                    assert_eq!(recovered.visible(), VisibleGeneration::Old);
                } else {
                    assert_eq!(recovered.visible(), VisibleGeneration::New);
                }
                let cleanup_finished = index
                    > protocol
                        .steps()
                        .iter()
                        .position(|step| *step == ProtocolStep::RemoveCompletedDirectory)
                        .unwrap();
                let intent_durable = index
                    > protocol
                        .steps()
                        .iter()
                        .position(|step| *step == ProtocolStep::PublishIntent)
                        .unwrap();
                assert_eq!(
                    recovered.has_resumable_evidence(),
                    intent_durable && !cleanup_finished
                );
            }
        }
    }
}
