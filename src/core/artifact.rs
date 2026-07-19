use std::collections::{BTreeMap, BTreeSet};
use std::error::Error as _;

use crate::{
    CorpusLocation, GeneratorError, GeneratorErrorKind, RelativePath, Result, Sha256Digest,
};

use super::coordination::{Domain, new_token};
use super::fs::{CORPUS_FILE_MODE, NodeKind};
use super::inventory::{Inventory, InventoryPolicy};
use super::lease::{GenerationLease, LeaseBinding};
#[cfg(feature = "css-corpus")]
use super::transaction::external_stage_name;
use super::transaction::{StagedTree, TransactionEngine, TransactionRequest};

/// The three publication behaviors admitted by the shared transaction layer.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum PublicationPolicy {
    CleanFull,
    DiagnosticFull,
    Filtered,
}

/// Domain-provided classification of every admissible file in a publication unit.
#[derive(Clone, Debug)]
pub(crate) struct PublicationInventory {
    classified: BTreeSet<RelativePath>,
    retained: BTreeSet<RelativePath>,
    reports: BTreeSet<RelativePath>,
}

impl PublicationInventory {
    pub(crate) fn new(
        classified: Vec<RelativePath>,
        retained: Vec<RelativePath>,
        reports: Vec<RelativePath>,
    ) -> Result<Self> {
        let classified = exact_path_set("classified", classified)?;
        let retained = exact_path_set("retained", retained)?;
        let reports = exact_path_set("report", reports)?;
        if retained.iter().any(|path| !classified.contains(path)) {
            return Err(plan_error(
                "retained output is absent from the classified inventory",
            ));
        }
        if reports.iter().any(|path| !classified.contains(path)) {
            return Err(plan_error(
                "report output is absent from the classified inventory",
            ));
        }
        Ok(Self {
            classified,
            retained,
            reports,
        })
    }

    fn admits_directory(&self, path: &RelativePath) -> bool {
        let prefix = format!("{}/", path.as_str());
        self.classified
            .iter()
            .any(|candidate| candidate.as_str().starts_with(&prefix))
    }
}

#[derive(Clone, Debug)]
struct PlannedArtifact {
    bytes: Vec<u8>,
    digest: Sha256Digest,
}

/// One exact-root publication plan bound to its original exclusive lease.
#[derive(Debug)]
pub(crate) struct ArtifactPlan {
    location: CorpusLocation,
    domain: Domain,
    binding: LeaseBinding,
    final_root: RelativePath,
    policy: PublicationPolicy,
    inventory: PublicationInventory,
    artifacts: BTreeMap<RelativePath, PlannedArtifact>,
    #[cfg(feature = "css-corpus")]
    transaction_token: Option<String>,
}

/// In-memory proof of the exact external stage namespace chosen before lease.
#[cfg(feature = "css-corpus")]
#[derive(Debug)]
pub(crate) struct ArtifactReservation {
    domain: Domain,
    token: String,
    external_stage: RelativePath,
}

#[cfg(feature = "css-corpus")]
impl ArtifactReservation {
    pub(crate) fn new(domain: Domain) -> Result<Self> {
        let token = new_token()?;
        let external_stage = external_stage_name(domain.as_str(), &token)?;
        Ok(Self {
            domain,
            token,
            external_stage,
        })
    }

    pub(crate) const fn external_stage(&self) -> &RelativePath {
        &self.external_stage
    }
}

impl ArtifactPlan {
    /// Constructs a plan without probing or mutating the filesystem.
    pub(crate) fn new(
        location: &CorpusLocation,
        domain: Domain,
        lease: &GenerationLease,
        final_root: RelativePath,
        policy: PublicationPolicy,
        artifacts: Vec<(RelativePath, Vec<u8>)>,
        inventory: PublicationInventory,
    ) -> Result<Self> {
        if final_root.as_str().contains('/')
            || final_root.as_str() == ".surgeist-generator"
            || final_root.as_str().starts_with("._surgeist-")
        {
            return Err(plan_error(
                "publication root must be one non-reserved component",
            ));
        }
        let binding = lease.bind(location, domain)?;
        let mut planned = BTreeMap::new();
        for (path, bytes) in artifacts {
            if uses_reserved_component(&path) {
                return Err(plan_error("artifact path uses a reserved component"));
            }
            if !inventory.classified.contains(&path) {
                return Err(plan_error(format!(
                    "artifact is absent from classified inventory: {}",
                    path.as_str()
                )));
            }
            if policy == PublicationPolicy::Filtered && inventory.reports.contains(&path) {
                return Err(plan_error("filtered publication cannot write a report"));
            }
            let artifact = PlannedArtifact {
                digest: Sha256Digest::from_bytes(&bytes),
                bytes,
            };
            if planned.insert(path.clone(), artifact).is_some() {
                return Err(plan_error(format!(
                    "duplicate artifact path: {}",
                    path.as_str()
                )));
            }
        }
        if policy == PublicationPolicy::CleanFull
            && planned
                .keys()
                .any(|path| !inventory.retained.contains(path))
        {
            return Err(plan_error(
                "clean-full artifact is absent from retained inventory",
            ));
        }
        Ok(Self {
            location: location.clone(),
            domain,
            binding,
            final_root,
            policy,
            inventory,
            artifacts: planned,
            #[cfg(feature = "css-corpus")]
            transaction_token: None,
        })
    }

    #[cfg(feature = "css-corpus")]
    pub(crate) fn with_reservation(mut self, reservation: ArtifactReservation) -> Result<Self> {
        if reservation.domain != self.domain {
            return Err(plan_error(
                "artifact reservation domain differs from the publication plan",
            ));
        }
        if reservation.external_stage == self.final_root {
            return Err(plan_error(
                "artifact reservation collides with the publication root",
            ));
        }
        self.transaction_token = Some(reservation.token);
        Ok(self)
    }

    /// Installs the complete new unit through one EXCL or SWAP root transition.
    pub(crate) fn install(&self) -> Result<()> {
        self.install_impl(|_| Ok(()), None)
    }

    /// Revalidates domain-owned read authorities at the last pre-intent boundary.
    #[cfg(feature = "css-corpus")]
    pub(crate) fn install_with_revalidation(
        self,
        pre_intent_revalidation: impl FnOnce(&super::fs::RootedFs) -> Result<()>,
    ) -> Result<()> {
        let transaction_token = self.transaction_token.clone();
        self.install_impl(pre_intent_revalidation, transaction_token)
    }

    fn install_impl(
        &self,
        pre_intent_revalidation: impl FnOnce(&super::fs::RootedFs) -> Result<()>,
        transaction_token: Option<String>,
    ) -> Result<()> {
        // This validation intentionally precedes every descriptor recheck, probe, and write.
        let state = self.binding.validate(&self.location, self.domain)?;
        let rooted = state.rooted();
        rooted.revalidate_root()?;
        let current = Inventory::scan(
            rooted,
            self.final_root.as_str(),
            InventoryPolicy::FinalCorpus,
        )
        .map_err(pre_intent_error)?;
        self.validate_current(current.as_ref())?;
        pre_intent_revalidation(rooted)?;

        let mut desired = BTreeMap::new();
        if let Some(current) = current.as_ref() {
            for entry in current.entries() {
                if entry.identity().kind() != NodeKind::Regular {
                    continue;
                }
                let preserve = match self.policy {
                    PublicationPolicy::CleanFull => self.inventory.retained.contains(entry.path()),
                    PublicationPolicy::DiagnosticFull | PublicationPolicy::Filtered => true,
                };
                if preserve {
                    desired.insert(
                        entry.path().clone(),
                        rooted
                            .read_file(
                                &joined(self.final_root.as_str(), entry.path().as_str()),
                                CORPUS_FILE_MODE,
                            )
                            .map_err(pre_intent_error)?,
                    );
                }
            }
        }
        for (path, artifact) in &self.artifacts {
            desired.insert(path.clone(), artifact.bytes.clone());
        }
        if self.policy == PublicationPolicy::CleanFull {
            for path in &self.inventory.retained {
                if !desired.contains_key(path) {
                    return Err(GeneratorError::new(
                        GeneratorErrorKind::InvalidInventory,
                        "validate clean-full publication",
                        format!("retained artifact is unavailable: {}", path.as_str()),
                    ));
                }
            }
        }

        if state.token().is_none() {
            return Err(plan_error("exclusive lease token disappeared"));
        }
        let transaction_token = transaction_token.map_or_else(new_token, Ok)?;
        let request = TransactionRequest::new(
            state.authority_key(),
            self.domain.as_str(),
            transaction_token,
            self.final_root.clone(),
            StagedTree::new(desired)?,
        )?;
        TransactionEngine::new(
            rooted,
            state.transaction_parent(),
            state.authority_key(),
            self.domain.as_str(),
        )?
        .install(&request)
    }

    pub(crate) fn artifact_digest(&self, path: &RelativePath) -> Option<&Sha256Digest> {
        self.artifacts.get(path).map(|artifact| &artifact.digest)
    }

    fn validate_current(&self, current: Option<&Inventory>) -> Result<()> {
        let Some(current) = current else {
            return Ok(());
        };
        for entry in current.entries() {
            let admitted = match entry.identity().kind() {
                NodeKind::Directory => self.inventory.admits_directory(entry.path()),
                NodeKind::Regular => self.inventory.classified.contains(entry.path()),
                NodeKind::Symlink => false,
            };
            if !admitted {
                return Err(GeneratorError::new(
                    GeneratorErrorKind::InvalidInventory,
                    "classify current publication tree",
                    format!("unknown current entry: {}", entry.path().as_str()),
                ));
            }
        }
        Ok(())
    }
}

fn exact_path_set(label: &str, paths: Vec<RelativePath>) -> Result<BTreeSet<RelativePath>> {
    let mut exact = BTreeSet::new();
    for path in paths {
        if uses_reserved_component(&path) {
            return Err(plan_error(format!(
                "{label} inventory path uses a reserved component: {}",
                path.as_str()
            )));
        }
        if !exact.insert(path.clone()) {
            return Err(plan_error(format!(
                "duplicate {label} inventory path: {}",
                path.as_str()
            )));
        }
    }
    Ok(exact)
}

fn uses_reserved_component(path: &RelativePath) -> bool {
    path.as_str()
        .split('/')
        .any(|component| component == ".surgeist-generator" || component.starts_with("._surgeist-"))
}

fn joined(parent: &str, child: &str) -> String {
    format!("{parent}/{child}")
}

fn plan_error(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::ArtifactTransaction,
        "construct rooted artifact plan",
        detail,
    )
}

fn pre_intent_error(error: GeneratorError) -> GeneratorError {
    if error.kind() == GeneratorErrorKind::ArtifactTransaction && error.source().is_some() {
        GeneratorError::new(
            GeneratorErrorKind::Io,
            "perform artifact pre-intent I/O",
            error.to_string(),
        )
    } else {
        error
    }
}

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::{CorpusLocation, GeneratorErrorKind, RelativePath, RunScope};

    use super::{ArtifactPlan, Domain, GenerationLease, PublicationInventory, PublicationPolicy};

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        location: CorpusLocation,
    }

    impl Fixture {
        fn new() -> Self {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "surgeist-generator-artifact-{}-{sequence}",
                std::process::id()
            ));
            let owner = root.join("owner");
            let corpus = owner.join("corpus");
            fs::create_dir(&root).expect("create test root");
            fs::create_dir(&owner).expect("create owner");
            fs::create_dir(&corpus).expect("create corpus");
            let location = CorpusLocation::new(&owner, &corpus).expect("corpus location");
            Self { root, location }
        }

        fn seed(&self, files: &[(&str, &[u8])]) {
            let unit = self.location.corpus_root().join("xml");
            fs::create_dir(&unit).expect("create publication root");
            fs::set_permissions(&unit, fs::Permissions::from_mode(0o755)).expect("root mode");
            for (relative, bytes) in files {
                let path = unit.join(relative);
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent).expect("create artifact parents");
                    let mut cursor = parent;
                    while cursor.starts_with(&unit) && cursor != unit {
                        fs::set_permissions(cursor, fs::Permissions::from_mode(0o755))
                            .expect("directory mode");
                        cursor = cursor.parent().expect("parent");
                    }
                }
                fs::write(&path, bytes).expect("write artifact");
                fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("file mode");
            }
        }

        fn lease(&self) -> GenerationLease {
            GenerationLease::acquire(
                &self.location,
                Domain::Layout,
                "layout-generator",
                &RunScope::Full,
                "generate",
            )
            .expect("generation lease")
        }

        fn read(&self, path: &str) -> Vec<u8> {
            fs::read(self.location.corpus_root().join("xml").join(path)).expect("read artifact")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).expect("remove fixture");
        }
    }

    fn path(value: &str) -> RelativePath {
        RelativePath::new(value).expect("relative path")
    }

    fn inventory(classified: &[&str], retained: &[&str], reports: &[&str]) -> PublicationInventory {
        PublicationInventory::new(
            classified.iter().map(|value| path(value)).collect(),
            retained.iter().map(|value| path(value)).collect(),
            reports.iter().map(|value| path(value)).collect(),
        )
        .expect("publication inventory")
    }

    #[test]
    fn plan_install_without_its_original_live_lease_stops_before_writes() {
        let fixture = Fixture::new();
        fixture.seed(&[("old.xml", b"old")]);
        let lease = fixture.lease();
        let plan = ArtifactPlan::new(
            &fixture.location,
            Domain::Layout,
            &lease,
            path("xml"),
            PublicationPolicy::CleanFull,
            vec![(path("new.xml"), b"new".to_vec())],
            inventory(&["old.xml", "new.xml"], &["new.xml"], &[]),
        )
        .expect("plan");
        drop(lease);
        let before = snapshot(fixture.location.corpus_root());
        let error = plan.install().expect_err("released lease");
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_eq!(snapshot(fixture.location.corpus_root()), before);
    }

    #[test]
    fn clean_full_prunes_only_classified_stale_output() {
        let fixture = Fixture::new();
        fixture.seed(&[("keep.xml", b"keep"), ("stale.xml", b"stale")]);
        let lease = fixture.lease();
        let plan = ArtifactPlan::new(
            &fixture.location,
            Domain::Layout,
            &lease,
            path("xml"),
            PublicationPolicy::CleanFull,
            vec![(path("new.xml"), b"new".to_vec())],
            inventory(
                &["keep.xml", "stale.xml", "new.xml"],
                &["keep.xml", "new.xml"],
                &[],
            ),
        )
        .expect("clean plan");
        assert_eq!(
            plan.artifact_digest(&path("new.xml")),
            Some(&crate::Sha256Digest::from_bytes(b"new"))
        );
        plan.install().expect("install clean plan");
        assert_eq!(fixture.read("keep.xml"), b"keep");
        assert_eq!(fixture.read("new.xml"), b"new");
        assert!(
            !fixture
                .location
                .corpus_root()
                .join("xml/stale.xml")
                .exists()
        );
    }

    #[test]
    fn diagnostic_full_preserves_stale_output() {
        let fixture = Fixture::new();
        fixture.seed(&[("stale.xml", b"stale")]);
        let lease = fixture.lease();
        ArtifactPlan::new(
            &fixture.location,
            Domain::Layout,
            &lease,
            path("xml"),
            PublicationPolicy::DiagnosticFull,
            vec![(
                path("generation-reports/diagnostic.json"),
                b"diagnostic".to_vec(),
            )],
            inventory(
                &["stale.xml", "generation-reports/diagnostic.json"],
                &[],
                &["generation-reports/diagnostic.json"],
            ),
        )
        .expect("diagnostic plan")
        .install()
        .expect("install diagnostic plan");
        assert_eq!(fixture.read("stale.xml"), b"stale");
        assert_eq!(
            fixture.read("generation-reports/diagnostic.json"),
            b"diagnostic"
        );
    }

    #[test]
    fn filtered_preserves_stale_and_cannot_write_reports() {
        let fixture = Fixture::new();
        fixture.seed(&[
            ("matched.xml", b"old"),
            ("stale.xml", b"stale"),
            ("generation-reports/full.json", b"report"),
        ]);
        let lease = fixture.lease();
        let classification = || {
            inventory(
                &["matched.xml", "stale.xml", "generation-reports/full.json"],
                &[],
                &["generation-reports/full.json"],
            )
        };
        let report_error = ArtifactPlan::new(
            &fixture.location,
            Domain::Layout,
            &lease,
            path("xml"),
            PublicationPolicy::Filtered,
            vec![(path("generation-reports/full.json"), b"changed".to_vec())],
            classification(),
        )
        .expect_err("filtered report");
        assert_eq!(report_error.kind(), GeneratorErrorKind::ArtifactTransaction);
        ArtifactPlan::new(
            &fixture.location,
            Domain::Layout,
            &lease,
            path("xml"),
            PublicationPolicy::Filtered,
            vec![(path("matched.xml"), b"new".to_vec())],
            classification(),
        )
        .expect("filtered plan")
        .install()
        .expect("install filtered plan");
        assert_eq!(fixture.read("matched.xml"), b"new");
        assert_eq!(fixture.read("stale.xml"), b"stale");
        assert_eq!(fixture.read("generation-reports/full.json"), b"report");
    }

    #[test]
    fn unknown_inventory_fails_before_transaction_intent() {
        let fixture = Fixture::new();
        fixture.seed(&[("unknown.xml", b"unknown")]);
        let lease = fixture.lease();
        let plan = ArtifactPlan::new(
            &fixture.location,
            Domain::Layout,
            &lease,
            path("xml"),
            PublicationPolicy::CleanFull,
            vec![(path("new.xml"), b"new".to_vec())],
            inventory(&["new.xml"], &["new.xml"], &[]),
        )
        .expect("plan");
        let error = plan.install().expect_err("unknown inventory");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        let transactions = fixture
            .location
            .corpus_root()
            .join(".surgeist-generator/transactions/layout");
        assert_eq!(fs::read_dir(transactions).expect("transactions").count(), 0);
    }

    fn snapshot(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        fn visit(root: &Path, current: &Path, output: &mut BTreeMap<PathBuf, Vec<u8>>) {
            for entry in fs::read_dir(current).expect("snapshot directory") {
                let entry = entry.expect("snapshot entry");
                let path = entry.path();
                let relative = path
                    .strip_prefix(root)
                    .expect("relative path")
                    .to_path_buf();
                let metadata = fs::symlink_metadata(&path).expect("snapshot metadata");
                if metadata.is_dir() {
                    output.insert(relative, Vec::new());
                    visit(root, &path, output);
                } else {
                    output.insert(relative, fs::read(path).expect("snapshot file"));
                }
            }
        }
        let mut output = BTreeMap::new();
        visit(root, root, &mut output);
        output
    }
}
