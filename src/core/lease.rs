use std::path::PathBuf;
use std::sync::{Arc, Weak};

use crate::{CorpusLocation, GeneratorError, GeneratorErrorKind, Result, RunScope};

use super::coordination::{
    CoordinationAccess, CoordinationGuard, CoordinationState, Domain, LeaseMetadata,
    acquire_exclusive, acquire_shared_check,
};
#[cfg(test)]
use super::coordination::{ExclusiveAcquisitionControl, acquire_exclusive_controlled};
use super::fs::{HeldIdentity, RootedFs};

/// A live, exclusive mutation authority for one fixed corpus and domain.
#[derive(Debug)]
pub(crate) struct GenerationLease {
    guard: CoordinationGuard,
}

impl GenerationLease {
    /// Acquires a corpus-local exclusive lease after all coordination recovery.
    pub(crate) fn acquire(
        location: &CorpusLocation,
        domain: Domain,
        generator: &str,
        scope: &RunScope,
        command: &str,
    ) -> Result<Self> {
        Self::acquire_with_revalidation(location, domain, generator, scope, command, |_| Ok(()))
    }

    /// Acquires a lease and performs a final protected-source check while locked.
    pub(crate) fn acquire_with_revalidation(
        location: &CorpusLocation,
        domain: Domain,
        generator: &str,
        scope: &RunScope,
        command: &str,
        protected_revalidation: impl FnOnce(&RootedFs) -> Result<()>,
    ) -> Result<Self> {
        let metadata = LeaseMetadata::new(generator, scope, command)?;
        acquire_exclusive(location, domain, metadata, protected_revalidation)
            .map(|guard| Self { guard })
    }

    /// Creates a non-owning binding to this exact acquisition.
    pub(crate) fn bind(&self, location: &CorpusLocation, domain: Domain) -> Result<LeaseBinding> {
        if self.guard.access() != CoordinationAccess::Exclusive {
            return Err(binding_error("lease is not exclusive"));
        }
        let state = self.guard.state();
        if !state.is_live() {
            return Err(binding_error("lease has been released"));
        }
        if state.domain() != domain {
            return Err(binding_error("lease domain does not match the plan"));
        }
        if state.canonical_corpus() != location.corpus_root() {
            return Err(binding_error("lease corpus does not match the plan"));
        }
        let token = state
            .token()
            .ok_or_else(|| binding_error("exclusive lease has no acquisition token"))?
            .to_owned();
        Ok(LeaseBinding {
            state: Arc::downgrade(state),
            original_token: token,
            canonical_corpus: location.corpus_root().to_path_buf(),
            corpus_identity: state.corpus_identity().clone(),
            domain,
        })
    }

    #[cfg(test)]
    fn state(&self) -> &Arc<CoordinationState> {
        self.guard.state()
    }

    #[cfg(test)]
    fn acquire_controlled(
        location: &CorpusLocation,
        domain: Domain,
        generator: &str,
        scope: &RunScope,
        command: &str,
        protected_revalidation: impl FnOnce(&RootedFs) -> Result<()>,
        control: &mut ExclusiveAcquisitionControl,
    ) -> Result<Self> {
        let metadata = LeaseMetadata::new(generator, scope, command)?;
        acquire_exclusive_controlled(location, domain, metadata, protected_revalidation, control)
            .map(|guard| Self { guard })
    }
}

/// A read-only shared coordination guard. Finishing repeats absence/residue checks.
#[derive(Debug)]
pub(crate) struct GenerationCheck {
    guard: Option<CoordinationGuard>,
}

impl GenerationCheck {
    pub(crate) fn acquire(location: &CorpusLocation, domain: Domain) -> Result<Self> {
        acquire_shared_check(location, domain).map(|guard| Self { guard: Some(guard) })
    }

    pub(crate) fn finish(mut self) -> Result<()> {
        self.guard
            .take()
            .ok_or_else(|| verification_error("check guard was already finished"))?
            .finish_check()
    }
}

/// A plan's proof that it was constructed from one original live acquisition.
#[derive(Clone, Debug)]
pub(crate) struct LeaseBinding {
    state: Weak<CoordinationState>,
    original_token: String,
    canonical_corpus: PathBuf,
    corpus_identity: HeldIdentity,
    domain: Domain,
}

impl LeaseBinding {
    /// Validates only in-memory authority first, before any capability probe or write.
    pub(crate) fn validate(
        &self,
        location: &CorpusLocation,
        domain: Domain,
    ) -> Result<LeaseOperation> {
        if self.domain != domain {
            return Err(binding_error("plan domain differs from its original lease"));
        }
        if self.canonical_corpus != location.corpus_root() {
            return Err(binding_error("plan corpus differs from its original lease"));
        }
        let state = self
            .state
            .upgrade()
            .ok_or_else(|| binding_error("original lease no longer exists"))?;
        if !state.is_live() {
            return Err(binding_error("original lease has been released"));
        }
        if !state.try_begin_operation() {
            return Err(binding_error(
                "another transaction is active under the original lease",
            ));
        }
        if state.domain() != self.domain
            || state.canonical_corpus() != self.canonical_corpus
            || state.corpus_identity() != &self.corpus_identity
            || state.token() != Some(self.original_token.as_str())
        {
            state.finish_operation();
            return Err(binding_error("original lease identity changed"));
        }
        Ok(LeaseOperation { state })
    }
}

#[derive(Debug)]
pub(crate) struct LeaseOperation {
    state: Arc<CoordinationState>,
}

impl std::ops::Deref for LeaseOperation {
    type Target = CoordinationState;

    fn deref(&self) -> &Self::Target {
        &self.state
    }
}

impl Drop for LeaseOperation {
    fn drop(&mut self) {
        self.state.finish_operation();
    }
}

fn binding_error(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::ArtifactTransaction,
        "validate artifact mutation lease",
        detail,
    )
}

fn verification_error(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::Verification,
        "finish generation check",
        detail,
    )
}

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
mod tests {
    use std::fs;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::core::coordination::{ExclusiveAcquisitionControl, ProbeCapabilityFault};
    use crate::core::fs::{DurabilityEvent, DurabilityPhase, DurabilityPrimitive, RootedObserver};
    use crate::{CorpusLocation, GeneratorErrorKind, RunScope};

    use super::{Domain, GenerationCheck, GenerationLease};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "surgeist-generator-lease-{}-{sequence:016x}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create test root");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove test root");
        }
    }

    fn locations() -> (TestDirectory, CorpusLocation, CorpusLocation) {
        let temporary = TestDirectory::new();
        let owner = temporary.path().join("owner");
        let nested_owner = owner.join("nested");
        let corpus = nested_owner.join("corpus");
        fs::create_dir(&owner).expect("create owner");
        fs::create_dir(&nested_owner).expect("create nested owner");
        fs::create_dir(&corpus).expect("create corpus");
        let outer = CorpusLocation::new(&owner, &corpus).expect("outer location");
        let inner = CorpusLocation::new(&nested_owner, &corpus).expect("inner location");
        (temporary, outer, inner)
    }

    #[test]
    fn distinct_owner_ancestors_for_one_corpus_contend_on_the_corpus_mutex() {
        let (_temporary, outer, inner) = locations();
        let held = GenerationLease::acquire(
            &outer,
            Domain::Layout,
            "layout-generator",
            &RunScope::Full,
            "generate",
        )
        .expect("first lease");
        let error = GenerationLease::acquire(
            &inner,
            Domain::Layout,
            "layout-generator",
            &RunScope::Full,
            "generate",
        )
        .expect_err("same corpus/domain must contend");
        assert_eq!(error.kind(), GeneratorErrorKind::LeaseActive);
        drop(held);
    }

    #[test]
    fn two_shared_checks_are_read_only_and_can_coexist() {
        let (_temporary, outer, _) = locations();
        drop(
            GenerationLease::acquire(
                &outer,
                Domain::Layout,
                "layout-generator",
                &RunScope::Full,
                "generate",
            )
            .expect("seed coordination"),
        );
        let before = directory_snapshot(outer.corpus_root());
        let first = GenerationCheck::acquire(&outer, Domain::Layout).expect("first check");
        let second = GenerationCheck::acquire(&outer, Domain::Layout).expect("second check");
        second.finish().expect("finish second");
        first.finish().expect("finish first");
        assert_eq!(directory_snapshot(outer.corpus_root()), before);
    }

    #[test]
    fn binding_rejects_release_and_a_later_reacquisition() {
        let (_temporary, outer, _) = locations();
        let first = GenerationLease::acquire(
            &outer,
            Domain::Layout,
            "layout-generator",
            &RunScope::Full,
            "generate",
        )
        .expect("first lease");
        let binding = first
            .bind(&outer, Domain::Layout)
            .expect("bind first lease");
        assert_eq!(binding.domain, Domain::Layout);
        assert_eq!(binding.canonical_corpus, outer.corpus_root());
        assert!(binding.validate(&outer, Domain::Layout).is_ok());
        drop(first);
        let second = GenerationLease::acquire(
            &outer,
            Domain::Layout,
            "layout-generator",
            &RunScope::Full,
            "generate",
        )
        .expect("second lease");
        assert_eq!(
            binding.validate(&outer, Domain::Layout).unwrap_err().kind(),
            GeneratorErrorKind::ArtifactTransaction
        );
        assert_ne!(
            second.state().token(),
            Some(binding.original_token.as_str())
        );
    }

    #[test]
    fn validated_binding_pins_the_os_mutex_until_the_operation_finishes() {
        let (_temporary, outer, _) = locations();
        let first = GenerationLease::acquire(
            &outer,
            Domain::Layout,
            "layout-generator",
            &RunScope::Full,
            "generate",
        )
        .expect("first lease");
        let binding = first.bind(&outer, Domain::Layout).expect("lease binding");
        let operation = binding
            .validate(&outer, Domain::Layout)
            .expect("validated operation");
        drop(first);
        let error = GenerationLease::acquire(
            &outer,
            Domain::Layout,
            "layout-generator",
            &RunScope::Full,
            "generate",
        )
        .expect_err("operation pin must retain the lock");
        assert_eq!(error.kind(), GeneratorErrorKind::LeaseActive);
        drop(operation);
        drop(
            GenerationLease::acquire(
                &outer,
                Domain::Layout,
                "layout-generator",
                &RunScope::Full,
                "generate",
            )
            .expect("lease after operation completion"),
        );
    }

    #[test]
    fn one_acquisition_serializes_in_process_transactions() {
        let (_temporary, outer, _) = locations();
        let lease = GenerationLease::acquire(
            &outer,
            Domain::Layout,
            "layout-generator",
            &RunScope::Full,
            "generate",
        )
        .expect("lease");
        let first_binding = lease.bind(&outer, Domain::Layout).expect("first binding");
        let second_binding = lease.bind(&outer, Domain::Layout).expect("second binding");
        let first = first_binding
            .validate(&outer, Domain::Layout)
            .expect("first transaction");
        let error = second_binding
            .validate(&outer, Domain::Layout)
            .expect_err("concurrent transaction");
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        drop(first);
        drop(
            second_binding
                .validate(&outer, Domain::Layout)
                .expect("serialized transaction"),
        );
    }

    const ACQUISITION_TOKEN_A: &str = "77777777777777777777777777777777";
    const ACQUISITION_TOKEN_B: &str = "88888888888888888888888888888888";

    fn historical_owner_path(location: &CorpusLocation) -> PathBuf {
        location
            .corpus_root()
            .join(".surgeist-generator/leases/layout/owner.json")
    }

    fn owner_transaction_path(location: &CorpusLocation) -> PathBuf {
        location
            .corpus_root()
            .join(".surgeist-generator/leases/layout/owner-transactions")
    }

    fn probe_path(location: &CorpusLocation) -> PathBuf {
        location
            .corpus_root()
            .join(".surgeist-generator/probes/layout")
    }

    fn seed_acquisition_fixture(location: &CorpusLocation) -> Vec<u8> {
        fs::write(
            location.corpus_root().join("domain-artifact.bin"),
            b"lease acquisition must preserve domain bytes\n",
        )
        .expect("seed acquisition domain artifact");
        drop(
            GenerationLease::acquire(
                location,
                Domain::Layout,
                "layout-generator",
                &RunScope::Full,
                "historical-generate",
            )
            .expect("seed historical acquisition"),
        );
        fs::read(historical_owner_path(location)).expect("read historical owner")
    }

    fn controlled_acquire(
        location: &CorpusLocation,
        observer: RootedObserver,
        token: &str,
        fault: Option<ProbeCapabilityFault>,
        protected_revalidation: impl FnOnce(&super::RootedFs) -> crate::Result<()>,
    ) -> crate::Result<GenerationLease> {
        let mut control = ExclusiveAcquisitionControl::new(token, observer, fault);
        GenerationLease::acquire_controlled(
            location,
            Domain::Layout,
            "layout-generator",
            &RunScope::Full,
            "replacement-generate",
            protected_revalidation,
            &mut control,
        )
    }

    fn controlled_acquire_failing_probe_cleanup(
        location: &CorpusLocation,
        observer: RootedObserver,
        token: &str,
        fault: ProbeCapabilityFault,
        cleanup_index: usize,
        protected_revalidation: impl FnOnce(&super::RootedFs) -> crate::Result<()>,
    ) -> crate::Result<GenerationLease> {
        let mut control = ExclusiveAcquisitionControl::failing_probe_cleanup(
            token,
            observer,
            fault,
            cleanup_index,
        );
        GenerationLease::acquire_controlled(
            location,
            Domain::Layout,
            "layout-generator",
            &RunScope::Full,
            "replacement-generate",
            protected_revalidation,
            &mut control,
        )
    }

    fn expect_acquisition_interruption(
        location: &CorpusLocation,
        trace: &[DurabilityEvent],
        event_index: usize,
    ) {
        let observer = RootedObserver::interrupt_after(event_index);
        let interrupted = catch_unwind(AssertUnwindSafe(|| {
            controlled_acquire(
                location,
                observer.clone(),
                ACQUISITION_TOKEN_A,
                None,
                |_| Ok(()),
            )
        }))
        .expect_err("controlled acquisition must interrupt");
        assert!(RootedObserver::is_interruption(interrupted.as_ref()));
        assert_eq!(
            observer.events(),
            trace[..=event_index],
            "acquisition interruption trace differs"
        );
    }

    fn assert_recovery_before_probe_before_owner(trace: &[DurabilityEvent]) {
        let probe_install = trace
            .iter()
            .position(|event| event.phase() == DurabilityPhase::ProbeInstall)
            .expect("fresh acquisition reran the rename probe");
        let owner_install = trace
            .iter()
            .position(|event| event.phase() == DurabilityPhase::OwnerInstall)
            .expect("fresh acquisition installed its owner record");
        assert!(probe_install < owner_install);
        for (index, event) in trace.iter().enumerate() {
            if matches!(
                event.phase(),
                DurabilityPhase::OwnerRecovery | DurabilityPhase::ProbeRecovery
            ) {
                assert!(
                    index < probe_install,
                    "recovery event occurred after the repeated capability probe"
                );
            }
        }
    }

    #[test]
    fn lease_probe_failure_releases_mutex_without_owner_install() {
        let (_temporary, outer, _) = locations();
        let historical_owner = seed_acquisition_fixture(&outer);
        let domain_before = fs::read(outer.corpus_root().join("domain-artifact.bin"))
            .expect("read protected domain artifact");
        let observer = RootedObserver::recording();
        let error = controlled_acquire(
            &outer,
            observer,
            ACQUISITION_TOKEN_A,
            Some(ProbeCapabilityFault::FailExclusiveRename),
            |_| panic!("revalidation ran after capability rejection"),
        )
        .expect_err("capability rejection must return no guard");
        assert_eq!(error.kind(), GeneratorErrorKind::UnsupportedPlatform);
        assert!(std::error::Error::source(&error).is_some());
        assert_eq!(
            fs::read(historical_owner_path(&outer)).expect("reread historical owner"),
            historical_owner
        );
        assert_eq!(
            fs::read(outer.corpus_root().join("domain-artifact.bin"))
                .expect("reread protected domain artifact"),
            domain_before
        );
        assert!(
            fs::read_dir(owner_transaction_path(&outer))
                .expect("inspect owner transactions")
                .next()
                .is_none()
        );
        assert!(
            fs::read_dir(probe_path(&outer))
                .expect("inspect probe residue")
                .next()
                .is_none()
        );
        drop(
            GenerationLease::acquire(
                &outer,
                Domain::Layout,
                "layout-generator",
                &RunScope::Full,
                "after-capability-failure",
            )
            .expect("capability failure released the corpus mutex"),
        );

        let (_cleanup_temporary, cleanup_location, _) = locations();
        let historical_owner = seed_acquisition_fixture(&cleanup_location);
        let domain_before = fs::read(cleanup_location.corpus_root().join("domain-artifact.bin"))
            .expect("read domain before partial probe cleanup");
        let error = controlled_acquire_failing_probe_cleanup(
            &cleanup_location,
            RootedObserver::recording(),
            ACQUISITION_TOKEN_A,
            ProbeCapabilityFault::FailExclusiveRename,
            1,
            |_| panic!("revalidation ran after failed probe cleanup"),
        )
        .expect_err("partial probe cleanup must reject acquisition");
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        let diagnostic = error.to_string();
        assert!(diagnostic.contains("capability"), "{diagnostic}");
        assert!(diagnostic.contains("cleanup"), "{diagnostic}");
        assert!(
            fs::read_dir(probe_path(&cleanup_location))
                .expect("inspect retained probe journal")
                .next()
                .is_some()
        );

        let recovery_observer = RootedObserver::recording();
        let error = controlled_acquire(
            &cleanup_location,
            recovery_observer.clone(),
            ACQUISITION_TOKEN_B,
            Some(ProbeCapabilityFault::FailExclusiveRename),
            |_| panic!("revalidation ran after repeated capability rejection"),
        )
        .expect_err("next acquisition must recover and repeat the capability rejection");
        assert_eq!(error.kind(), GeneratorErrorKind::UnsupportedPlatform);
        let trace = recovery_observer.events();
        let recovery = trace
            .iter()
            .position(|event| event.phase() == DurabilityPhase::ProbeRecovery)
            .expect("next acquisition recovered the retained probe journal");
        let probe = trace
            .iter()
            .position(|event| event.phase() == DurabilityPhase::ProbeInstall)
            .expect("next acquisition reran the capability probe");
        assert!(recovery < probe);
        assert!(
            trace
                .iter()
                .all(|event| event.phase() != DurabilityPhase::OwnerInstall)
        );
        assert_eq!(
            fs::read(historical_owner_path(&cleanup_location))
                .expect("owner after probe cleanup recovery"),
            historical_owner
        );
        assert_eq!(
            fs::read(cleanup_location.corpus_root().join("domain-artifact.bin"))
                .expect("domain after probe cleanup recovery"),
            domain_before
        );
        assert!(
            fs::read_dir(owner_transaction_path(&cleanup_location))
                .expect("inspect owner transactions after cleanup recovery")
                .next()
                .is_none()
        );
        assert!(
            fs::read_dir(probe_path(&cleanup_location))
                .expect("inspect probe residue after cleanup recovery")
                .next()
                .is_none()
        );
        drop(
            GenerationLease::acquire(
                &cleanup_location,
                Domain::Layout,
                "layout-generator",
                &RunScope::Full,
                "after-cleanup-recovery",
            )
            .expect("cleanup failure and repeated capability rejection released the mutex"),
        );
    }

    #[test]
    fn lease_acquisition_recovers_probe_before_revalidation() {
        let (_trace_temporary, trace_location, _) = locations();
        seed_acquisition_fixture(&trace_location);
        let trace_observer = RootedObserver::recording();
        drop(
            controlled_acquire(
                &trace_location,
                trace_observer.clone(),
                ACQUISITION_TOKEN_A,
                None,
                |_| Ok(()),
            )
            .expect("record acquisition probe trace"),
        );
        let trace = trace_observer.events();
        let swap = trace
            .iter()
            .position(|event| {
                event.phase() == DurabilityPhase::ProbeInstall
                    && event.primitive() == DurabilityPrimitive::RenameSwap
            })
            .expect("acquisition trace contains the probe swap");

        let (_temporary, location, _) = locations();
        let historical_owner = seed_acquisition_fixture(&location);
        expect_acquisition_interruption(&location, &trace, swap);
        assert_eq!(
            fs::read(historical_owner_path(&location))
                .expect("owner after interrupted acquisition probe"),
            historical_owner
        );
        let recovery_observer = RootedObserver::recording();
        let lease = controlled_acquire(
            &location,
            recovery_observer.clone(),
            ACQUISITION_TOKEN_B,
            None,
            |rooted| {
                assert!(
                    rooted
                        .list_dir(".surgeist-generator/probes/layout")?
                        .is_empty()
                );
                assert_eq!(
                    rooted.read_file(".surgeist-generator/leases/layout/owner.json", 0o600,)?,
                    historical_owner
                );
                Ok(())
            },
        )
        .expect("fresh acquisition recovered and reran the probe");
        assert_recovery_before_probe_before_owner(&recovery_observer.events());
        drop(lease);
    }

    #[test]
    #[ignore = "exhaustive opt-in diagnostic"]
    fn lease_acquisition_recovers_owner_and_probe_prefixes() {
        let (_trace_temporary, trace_location, _) = locations();
        seed_acquisition_fixture(&trace_location);
        let trace_observer = RootedObserver::recording();
        drop(
            controlled_acquire(
                &trace_location,
                trace_observer.clone(),
                ACQUISITION_TOKEN_A,
                None,
                |_| Ok(()),
            )
            .expect("record complete production acquisition"),
        );
        let acquisition_trace = trace_observer.events();
        let owner_prefixes = acquisition_trace
            .iter()
            .enumerate()
            .filter_map(|(index, event)| {
                (event.phase() == DurabilityPhase::OwnerInstall).then_some(index)
            })
            .collect::<Vec<_>>();
        let probe_prefixes = acquisition_trace
            .iter()
            .enumerate()
            .filter_map(|(index, event)| {
                (event.phase() == DurabilityPhase::ProbeInstall).then_some(index)
            })
            .collect::<Vec<_>>();
        assert!(!owner_prefixes.is_empty());
        assert!(!probe_prefixes.is_empty());

        for event_index in owner_prefixes {
            let (_temporary, location, _) = locations();
            let domain_before = seed_acquisition_fixture(&location);
            expect_acquisition_interruption(&location, &acquisition_trace, event_index);
            let recovery_observer = RootedObserver::recording();
            let lease = controlled_acquire(
                &location,
                recovery_observer.clone(),
                ACQUISITION_TOKEN_B,
                None,
                |rooted| {
                    assert!(
                        rooted
                            .list_dir(".surgeist-generator/leases/layout/owner-transactions")?
                            .is_empty()
                    );
                    assert!(
                        rooted
                            .list_dir(".surgeist-generator/probes/layout")?
                            .is_empty()
                    );
                    Ok(())
                },
            )
            .expect("acquisition recovered owner prefix");
            assert_eq!(
                fs::read(location.corpus_root().join("domain-artifact.bin"))
                    .expect("read domain after owner recovery"),
                b"lease acquisition must preserve domain bytes\n"
            );
            assert!(!domain_before.is_empty());
            assert_recovery_before_probe_before_owner(&recovery_observer.events());
            drop(lease);
        }

        for event_index in probe_prefixes {
            let (_temporary, location, _) = locations();
            let historical_owner = seed_acquisition_fixture(&location);
            let domain_before = fs::read(location.corpus_root().join("domain-artifact.bin"))
                .expect("read domain before probe interruption");
            expect_acquisition_interruption(&location, &acquisition_trace, event_index);
            assert_eq!(
                fs::read(historical_owner_path(&location))
                    .expect("owner after interrupted prerequisite probe"),
                historical_owner
            );
            let recovery_observer = RootedObserver::recording();
            let lease = controlled_acquire(
                &location,
                recovery_observer.clone(),
                ACQUISITION_TOKEN_B,
                None,
                |rooted| {
                    assert_eq!(
                        rooted.read_file(".surgeist-generator/leases/layout/owner.json", 0o600,)?,
                        historical_owner
                    );
                    assert!(
                        rooted
                            .list_dir(".surgeist-generator/probes/layout")?
                            .is_empty()
                    );
                    Ok(())
                },
            )
            .expect("acquisition recovered probe prefix and reprobed");
            assert_eq!(
                fs::read(location.corpus_root().join("domain-artifact.bin"))
                    .expect("read domain after probe recovery"),
                domain_before
            );
            assert_recovery_before_probe_before_owner(&recovery_observer.events());
            drop(lease);
        }
    }

    fn directory_snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        fn visit(root: &Path, current: &Path, output: &mut Vec<(PathBuf, Vec<u8>)>) {
            let mut entries: Vec<_> = fs::read_dir(current)
                .expect("read snapshot directory")
                .map(|entry| entry.expect("read snapshot entry"))
                .collect();
            entries.sort_by_key(std::fs::DirEntry::file_name);
            for entry in entries {
                let path = entry.path();
                let relative = path
                    .strip_prefix(root)
                    .expect("relative snapshot")
                    .to_path_buf();
                let metadata = fs::symlink_metadata(&path).expect("snapshot metadata");
                if metadata.is_dir() {
                    output.push((relative, Vec::new()));
                    visit(root, &path, output);
                } else {
                    output.push((relative, fs::read(path).expect("snapshot bytes")));
                }
            }
        }
        let mut output = Vec::new();
        visit(root, root, &mut output);
        output
    }
}
