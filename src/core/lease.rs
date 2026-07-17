use std::path::PathBuf;
use std::sync::{Arc, Weak};

use crate::{CorpusLocation, GeneratorError, GeneratorErrorKind, Result, RunScope};

use super::coordination::{
    CoordinationAccess, CoordinationGuard, CoordinationState, Domain, LeaseMetadata,
    acquire_exclusive, acquire_shared_check,
};
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
    pub(crate) fn bind(
        &self,
        location: &CorpusLocation,
        domain: Domain,
    ) -> Result<LeaseBinding> {
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
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::{CorpusLocation, GeneratorErrorKind, RunScope};

    use super::{Domain, GenerationCheck, GenerationLease};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "surgeist-generator-lease-{}-{sequence}",
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
        assert_eq!(binding.validate(&outer, Domain::Layout).unwrap_err().kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_ne!(second.state().token(), Some(binding.original_token.as_str()));
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
        let binding = first
            .bind(&outer, Domain::Layout)
            .expect("lease binding");
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
        let first_binding = lease
            .bind(&outer, Domain::Layout)
            .expect("first binding");
        let second_binding = lease
            .bind(&outer, Domain::Layout)
            .expect("second binding");
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

    fn directory_snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        fn visit(root: &Path, current: &Path, output: &mut Vec<(PathBuf, Vec<u8>)>) {
            let mut entries: Vec<_> = fs::read_dir(current)
                .expect("read snapshot directory")
                .map(|entry| entry.expect("read snapshot entry"))
                .collect();
            entries.sort_by_key(std::fs::DirEntry::file_name);
            for entry in entries {
                let path = entry.path();
                let relative = path.strip_prefix(root).expect("relative snapshot").to_path_buf();
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
