use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::io::Read;
use std::path::Path;

use crate::core::{
    ArtifactPlan, ArtifactReservation, CORPUS_FILE_MODE, Domain, GenerationCheck, GenerationLease,
    HeldIdentity, Inventory, InventoryPolicy, NodeKind, ProtectedSource,
    ProtectedSourceDisjointness, ProtectedSourceInventory, ProtectedTreeEntryKind,
    PublicationInventory, PublicationPolicy, RootedFs, SnapshotEntry, VerifiedSourceSnapshot,
    verify_protected_git_source_inventory,
};
use crate::{
    GeneratorError, GeneratorErrorKind, PinnedSource, RelativePath, Result, RunScope, Sha256Digest,
};

use super::LayoutRequest;
use super::manifest::{
    HTML_ROOT, LayoutManifest, MANIFEST_FILE, SIDECAR_FILE, TAFFY_REPOSITORY,
    TAFFY_SOURCE_DIRECTORY, paths_target_equal,
};
use super::sidecar::TaffyImportSidecar;

const GENERATOR: &str = "surgeist-layout-generate";
const COMMAND: &str = "import-taffy";
const HELPER_SCRIPT: &str = "scripts/gentest/test_helper.js";
const BASE_STYLE: &str = "scripts/gentest/test_base_style.css";

pub(super) fn run(request: &LayoutRequest) -> Result<()> {
    run_impl(request, || {}, || {})
}

pub(super) fn check(request: &LayoutRequest) -> Result<()> {
    let location = request.location();
    let manifest_path = location.corpus_root().join(MANIFEST_FILE);
    let manifest_bytes = super::manifest::read_file(&manifest_path)?;
    let manifest = super::manifest::parse(&manifest_bytes, &manifest_path)?;
    let inspection = scan_html(RootedFs::open_corpus(location)?)?;
    // A persisted sidecar owns current Taffy paths, so classify its inventory
    // before consulting the explicitly named checkout.
    let (expected, existing, authored) = if inspection.sidecar.is_some() {
        let (existing, authored) = classify_html(inspection, &manifest, &BTreeSet::new())?;
        let expected = source_expectation(request, &manifest)?;
        prove_partition_sets(&manifest.authored_files, &expected.desired_taffy)?;
        (expected, existing, authored)
    } else {
        let expected = source_expectation(request, &manifest)?;
        prove_partition_sets(&manifest.authored_files, &expected.desired_taffy)?;
        let (existing, authored) = classify_html(inspection, &manifest, &expected.desired_taffy)?;
        (expected, existing, authored)
    };
    validate_checked_import(&existing, &expected.sidecar)?;
    let authored_records = authored.records();

    let check =
        GenerationCheck::acquire(location, Domain::Layout).map_err(coordination_verification)?;
    let result = check_current(
        request,
        &manifest_bytes,
        &manifest,
        &expected,
        &existing,
        &authored,
        &authored_records,
    );
    let finish = check.finish().map_err(coordination_verification);
    match (result, finish) {
        (_, Err(error)) => Err(error),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn check_current(
    request: &LayoutRequest,
    manifest_bytes: &[u8],
    manifest: &LayoutManifest,
    expected: &SourceExpectation,
    existing: &ExistingHtml,
    authored: &AuthoredPartition,
    authored_records: &[(RelativePath, Vec<u8>, HeldIdentity)],
) -> Result<()> {
    let rooted = RootedFs::open_corpus(request.location())?;
    super::manifest::revalidate(&rooted, manifest_bytes)?;
    authored.revalidate(&rooted)?;
    let (current, current_authored) = inspect_html(rooted, manifest, &expected.desired_taffy)?;
    validate_checked_import(&current, &expected.sidecar)?;
    if &current != existing || current_authored.records() != authored_records {
        return Err(verification(
            "layout HTML identities or inventory changed during Taffy checking",
        ));
    }

    expected.source.closing_revalidate()?;
    let repeated = source_expectation(request, manifest)?;
    if repeated.sidecar != expected.sidecar || repeated.desired_taffy != expected.desired_taffy {
        return Err(source_verification(
            "immutable Taffy source snapshot changed during checking",
        ));
    }
    repeated.source.closing_revalidate()?;
    current_authored.revalidate(&current_authored.rooted)?;
    super::manifest::revalidate(&current_authored.rooted, manifest_bytes)?;
    Ok(())
}

#[derive(Debug)]
struct SourceExpectation {
    source: ProtectedSource,
    sidecar: TaffyImportSidecar,
    desired_taffy: BTreeSet<RelativePath>,
}

fn source_expectation(
    request: &LayoutRequest,
    manifest: &LayoutManifest,
) -> Result<SourceExpectation> {
    let pin = PinnedSource::new(
        "taffy",
        TAFFY_REPOSITORY,
        manifest.revision.clone(),
        RelativePath::new(TAFFY_SOURCE_DIRECTORY)?,
    )?;
    let source_inventory = verify_protected_git_source_inventory(
        request
            .source_root()
            .expect("LayoutRequest guarantees a Taffy source root"),
        &pin,
    )?;
    if source_inventory.verified().revision() != pin.revision() {
        return Err(source_verification(
            "verified revision differs from the manifest pin",
        ));
    }
    let (source, source_file_count) = validate_snapshot(manifest, source_inventory)?;
    let sidecar = TaffyImportSidecar::from_snapshot(&pin, source_file_count, source.snapshot())?;
    let desired_taffy = source
        .snapshot()
        .entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect();
    Ok(SourceExpectation {
        source,
        sidecar,
        desired_taffy,
    })
}

fn validate_checked_import(existing: &ExistingHtml, expected: &TaffyImportSidecar) -> Result<()> {
    let sidecar = existing.sidecar.as_ref().ok_or_else(|| {
        verification("Taffy import sidecar is absent; run import-taffy with the named source")
    })?;
    let inventory = existing.inventory.as_ref().ok_or_else(|| {
        verification("layout HTML root is absent; run import-taffy with the named source")
    })?;
    let sidecar_path = RelativePath::new(SIDECAR_FILE)?;
    let sidecar_entry = inventory.find(&sidecar_path).ok_or_else(|| {
        verification("Taffy import sidecar is absent; run import-taffy with the named source")
    })?;
    let canonical_sidecar = sidecar.canonical_bytes()?;
    if sidecar_entry.digest() != Some(&Sha256Digest::from_bytes(&canonical_sidecar)) {
        return Err(invalid_inventory(
            "Taffy import sidecar bytes changed during inventory inspection",
        ));
    }
    if sidecar != expected {
        return Err(verification(
            "Taffy import sidecar is stale against the manifest and named source",
        ));
    }
    for file in sidecar.files() {
        let Some(entry) = inventory.find(&file.path) else {
            return Err(verification(format!(
                "imported Taffy fixture is absent: {}",
                file.path.as_str()
            )));
        };
        if entry.digest() != Some(&file.sha256) {
            return Err(verification(format!(
                "imported Taffy fixture is stale: {}",
                file.path.as_str()
            )));
        }
    }
    Ok(())
}

fn coordination_verification(source: GeneratorError) -> GeneratorError {
    if source.kind() == GeneratorErrorKind::UnsupportedPlatform {
        return source;
    }
    GeneratorError::with_source(
        GeneratorErrorKind::Verification,
        "inspect layout generation coordination",
        source.to_string(),
        source,
    )
}

fn source_verification(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::SourceVerification,
        "verify Taffy check source",
        detail,
    )
}

fn verification(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::Verification,
        "check Taffy corpus",
        detail,
    )
}

#[cfg(test)]
pub(super) fn run_with_pre_lease_hook(request: &LayoutRequest, hook: impl FnOnce()) -> Result<()> {
    run_impl(request, hook, || {})
}

#[cfg(test)]
pub(super) fn run_with_inter_scan_hook(request: &LayoutRequest, hook: impl FnOnce()) -> Result<()> {
    run_impl(request, || {}, hook)
}

fn run_impl(
    request: &LayoutRequest,
    pre_lease_hook: impl FnOnce(),
    inter_scan_hook: impl FnOnce(),
) -> Result<()> {
    let location = request.location();
    let manifest_path = location.corpus_root().join(MANIFEST_FILE);
    let manifest_bytes = super::manifest::read_file(&manifest_path)?;
    let manifest = super::manifest::parse(&manifest_bytes, &manifest_path)?;
    let pin = PinnedSource::new(
        "taffy",
        TAFFY_REPOSITORY,
        manifest.revision.clone(),
        RelativePath::new(TAFFY_SOURCE_DIRECTORY)?,
    )?;
    let source_inventory = verify_protected_git_source_inventory(
        request
            .source_root()
            .expect("LayoutRequest guarantees an import source root"),
        &pin,
    )?;
    if source_inventory.verified().revision() != pin.revision() {
        return Err(GeneratorError::new(
            GeneratorErrorKind::SourceVerification,
            "verify Taffy import source",
            "verified revision differs from the manifest pin",
        ));
    }
    let (source, source_file_count) = validate_snapshot(&manifest, source_inventory)?;
    let desired_sidecar =
        TaffyImportSidecar::from_snapshot(&pin, source_file_count, source.snapshot())?;
    let desired_sidecar_bytes = desired_sidecar.canonical_bytes()?;
    let desired_taffy = source
        .snapshot()
        .entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
    prove_partition_sets(&manifest.authored_files, &desired_taffy)?;

    let rooted = RootedFs::open_corpus(location)?;
    let (existing, authored) = inspect_html(rooted, &manifest, &desired_taffy)?;
    let reservation = ArtifactReservation::new(Domain::Layout)?;
    let html_root = location.corpus_root().join(HTML_ROOT);
    let external_stage = reservation.external_stage().join(location.corpus_root());
    let helper_script = location.corpus_root().join(HELPER_SCRIPT);
    let base_style = location.corpus_root().join(BASE_STYLE);
    let protected = [
        ("layout corpus manifest", manifest_path.as_path()),
        ("layout helper script", helper_script.as_path()),
        ("layout base style", base_style.as_path()),
    ];
    let protection = if authored.files.is_empty() {
        ProtectedSourceDisjointness::for_mutation(
            location,
            &[
                ("layout HTML publication root", html_root.as_path()),
                ("layout transaction stage", external_stage.as_path()),
            ],
            &protected,
            &source,
        )?
    } else {
        let retained_owned = authored
            .files
            .iter()
            .map(|file| {
                (
                    format!("retained authored HTML {}", file.path.as_str()),
                    file.path.join(&html_root),
                )
            })
            .collect::<Vec<_>>();
        let retained = retained_owned
            .iter()
            .map(|(label, path)| (label.as_str(), path.as_path()))
            .collect::<Vec<_>>();
        ProtectedSourceDisjointness::for_partitioned_mutation(
            location,
            ("layout HTML publication root", &html_root),
            &[("layout transaction stage", external_stage.as_path())],
            &protected,
            &retained,
            &source,
        )?
    };
    pre_lease_hook();
    let lease = GenerationLease::acquire_with_protected_source(
        location,
        Domain::Layout,
        GENERATOR,
        &RunScope::Full,
        COMMAND,
        &protection,
    )?;

    let binding = lease.bind(location, Domain::Layout)?;
    let operation = binding.validate(location, Domain::Layout)?;
    let held_rooted = operation.rooted();
    protection.revalidate(held_rooted)?;
    super::manifest::revalidate(held_rooted, &manifest_bytes)?;
    authored.revalidate(held_rooted)?;
    let (held_existing, held_authored) =
        inspect_html(RootedFs::open_corpus(location)?, &manifest, &desired_taffy)?;
    held_authored.revalidate(held_rooted)?;
    if held_existing != existing || held_authored.records() != authored.records() {
        return Err(invalid_inventory(
            "layout HTML partition changed after held validation",
        ));
    }
    drop(operation);

    let sidecar_path = RelativePath::new(SIDECAR_FILE)?;
    let mut classified = existing.regular_paths();
    let mut retained = BTreeSet::from([sidecar_path.clone()]);
    let mut artifacts = vec![(sidecar_path, desired_sidecar_bytes)];
    for entry in &source.snapshot().entries {
        classified.insert(entry.path.clone());
        retained.insert(entry.path.clone());
        artifacts.push((entry.path.clone(), entry.bytes.clone()));
    }
    for file in &authored.files {
        classified.insert(file.path.clone());
        retained.insert(file.path.clone());
        artifacts.push((file.path.clone(), file.bytes.clone()));
    }
    classified.extend(retained.iter().cloned());
    let inventory = PublicationInventory::new(
        classified.into_iter().collect(),
        retained.into_iter().collect(),
        Vec::new(),
    )?;
    let plan = ArtifactPlan::new(
        location,
        Domain::Layout,
        &lease,
        RelativePath::new(HTML_ROOT)?,
        PublicationPolicy::CleanFull,
        artifacts,
        inventory,
    )?
    .with_reservation(reservation)?;
    let revalidate = |rooted: &RootedFs| {
        protection.revalidate(rooted)?;
        super::manifest::revalidate(rooted, &manifest_bytes)?;
        authored.revalidate(rooted)?;
        let (current, current_authored) =
            inspect_html(RootedFs::open_corpus(location)?, &manifest, &desired_taffy)?;
        current_authored.revalidate(rooted)?;
        if current != existing || current_authored.records() != authored.records() {
            return Err(invalid_inventory(
                "layout HTML partition changed before import intent",
            ));
        }
        Ok(())
    };
    #[cfg(test)]
    return plan.install_with_revalidation_and_inter_scan_hook(revalidate, inter_scan_hook);
    #[cfg(not(test))]
    {
        let _ = inter_scan_hook;
        plan.install_with_revalidation(revalidate)
    }
}

fn validate_snapshot(
    manifest: &LayoutManifest,
    source: ProtectedSourceInventory,
) -> Result<(ProtectedSource, usize)> {
    let snapshot = source.snapshot();
    let mut source_file_count = 0_usize;
    let mut imported = Vec::new();
    for entry in &snapshot.entries {
        let display = entry.path.display();
        if entry.kind != ProtectedTreeEntryKind::Blob
            || !matches!(entry.git_mode.as_str(), "100644" | "100755")
        {
            return Err(invalid_inventory(format!(
                "Taffy source tree contains an alias, link, or unsupported mode: {display}"
            )));
        }
        let path = entry.path.to_relative_path().ok_or_else(|| {
            invalid_inventory(format!(
                "Taffy fixture path is not canonical UTF-8: {display}"
            ))
        })?;
        if Path::new(path.as_str()).extension() != Some(OsStr::new("html")) {
            continue;
        }
        source_file_count = source_file_count
            .checked_add(1)
            .ok_or_else(|| invalid_inventory("Taffy HTML count overflows usize"))?;
        if entry.git_mode != "100644" {
            return Err(invalid_inventory(format!(
                "Taffy HTML fixture must be a Git mode 100644 blob: {display}"
            )));
        }
        if is_excluded(&path) {
            continue;
        }
        super::sidecar::validate_fixture_path(&path)?;
        let bytes = entry.bytes.as_ref().ok_or_else(|| {
            invalid_inventory(format!("Taffy fixture blob bytes are absent: {display}"))
        })?;
        let digest = entry.digest.as_ref().ok_or_else(|| {
            invalid_inventory(format!("Taffy fixture blob digest is absent: {display}"))
        })?;
        if digest != &Sha256Digest::from_bytes(bytes) {
            return Err(invalid_inventory(format!(
                "immutable Taffy snapshot digest mismatch: {display}"
            )));
        }
        imported.push(SnapshotEntry {
            path,
            git_mode: entry.git_mode.clone(),
            blob_object_id: entry.object_id.clone(),
            bytes: bytes.clone(),
            digest: digest.clone(),
        });
    }
    if source_file_count != manifest.expected_source_files {
        return Err(invalid_inventory(format!(
            "manifest expected {} pre-exclusion Taffy HTML files, verified snapshot contains {source_file_count}",
            manifest.expected_source_files
        )));
    }
    if imported.windows(2).any(|pair| pair[0].path >= pair[1].path) {
        return Err(invalid_inventory(
            "Taffy fixture paths are not strictly increasing",
        ));
    }
    let verified = VerifiedSourceSnapshot {
        object_format: snapshot.object_format,
        entries: imported,
    };
    source
        .into_protected_source(verified)
        .map(|source| (source, source_file_count))
}

fn is_excluded(path: &RelativePath) -> bool {
    path.as_str()
        .split('/')
        .next()
        .is_some_and(|component| matches!(component, "grid-lanes" | "subgrid"))
}

fn prove_partition_sets(
    authored: &BTreeSet<RelativePath>,
    desired_taffy: &BTreeSet<RelativePath>,
) -> Result<()> {
    let desired = desired_taffy.iter().collect::<Vec<_>>();
    for (index, left) in desired.iter().enumerate() {
        for right in &desired[index + 1..] {
            if paths_target_equal(left.as_str(), right.as_str()) {
                return Err(invalid_inventory(
                    "desired Taffy paths alias on the mutation target",
                ));
            }
        }
        if authored
            .iter()
            .any(|path| paths_target_equal(path.as_str(), left.as_str()))
        {
            return Err(invalid_inventory(format!(
                "Taffy destination collides with a Surgeist-authored file: {}",
                left.as_str()
            )));
        }
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExistingHtml {
    inventory: Option<Inventory>,
    sidecar: Option<TaffyImportSidecar>,
}

impl ExistingHtml {
    fn regular_paths(&self) -> BTreeSet<RelativePath> {
        self.inventory
            .iter()
            .flat_map(|inventory| inventory.entries())
            .filter(|entry| entry.identity().kind() == NodeKind::Regular)
            .map(|entry| entry.path().clone())
            .collect()
    }
}

#[derive(Debug)]
struct AuthoredPartition {
    rooted: RootedFs,
    files: Vec<AuthoredFile>,
}

#[derive(Debug)]
struct AuthoredFile {
    path: RelativePath,
    bytes: Vec<u8>,
    identity: HeldIdentity,
    handle: std::fs::File,
}

impl AuthoredPartition {
    fn records(&self) -> Vec<(RelativePath, Vec<u8>, HeldIdentity)> {
        self.files
            .iter()
            .map(|file| (file.path.clone(), file.bytes.clone(), file.identity.clone()))
            .collect()
    }

    fn revalidate(&self, rooted: &RootedFs) -> Result<()> {
        self.rooted.revalidate_root().map_err(authored_error)?;
        rooted.revalidate_root().map_err(authored_error)?;
        if self.rooted.canonical_root() != rooted.canonical_root()
            || !self.rooted.identity().same_object(rooted.identity())
        {
            return Err(invalid_inventory(
                "held authored partition uses a different corpus authority",
            ));
        }
        for file in &self.files {
            let rooted_path = joined(HTML_ROOT, file.path.as_str());
            let identity = self
                .rooted
                .validate_handle_at(&rooted_path, &file.handle, CORPUS_FILE_MODE)
                .map_err(authored_error)?;
            if !identity.matches_recovery(&file.identity) {
                return Err(invalid_inventory(format!(
                    "authored HTML identity changed: {}",
                    file.path.as_str()
                )));
            }
            let bytes = self
                .rooted
                .read_file(&rooted_path, CORPUS_FILE_MODE)
                .map_err(authored_error)?;
            if bytes != file.bytes {
                return Err(invalid_inventory(format!(
                    "authored HTML bytes changed: {}",
                    file.path.as_str()
                )));
            }
            self.rooted
                .validate_handle_at(&rooted_path, &file.handle, CORPUS_FILE_MODE)
                .map_err(authored_error)?;
        }
        Ok(())
    }
}

fn inspect_html(
    rooted: RootedFs,
    manifest: &LayoutManifest,
    desired_taffy: &BTreeSet<RelativePath>,
) -> Result<(ExistingHtml, AuthoredPartition)> {
    classify_html(scan_html(rooted)?, manifest, desired_taffy)
}

struct HtmlInspection {
    rooted: RootedFs,
    inventory: Option<Inventory>,
    sidecar: Option<TaffyImportSidecar>,
}

fn scan_html(rooted: RootedFs) -> Result<HtmlInspection> {
    let inventory = Inventory::scan(&rooted, HTML_ROOT, InventoryPolicy::FinalCorpus)?;
    let sidecar_path = RelativePath::new(SIDECAR_FILE)?;
    let sidecar_entry = inventory
        .as_ref()
        .and_then(|inventory| inventory.find(&sidecar_path));
    let sidecar = if let Some(entry) = sidecar_entry {
        if entry.identity().kind() != NodeKind::Regular {
            return Err(invalid_inventory(
                "Taffy import sidecar is not a regular file",
            ));
        }
        let bytes = rooted
            .read_file(&joined(HTML_ROOT, SIDECAR_FILE), CORPUS_FILE_MODE)
            .map_err(authored_error)?;
        Some(TaffyImportSidecar::parse_canonical(&bytes)?)
    } else {
        None
    };
    Ok(HtmlInspection {
        rooted,
        inventory,
        sidecar,
    })
}

fn classify_html(
    inspection: HtmlInspection,
    manifest: &LayoutManifest,
    desired_taffy: &BTreeSet<RelativePath>,
) -> Result<(ExistingHtml, AuthoredPartition)> {
    let HtmlInspection {
        rooted,
        inventory,
        sidecar,
    } = inspection;
    let sidecar_path = RelativePath::new(SIDECAR_FILE)?;

    let current_taffy = sidecar.as_ref().map_or_else(
        || desired_taffy.clone(),
        |sidecar| {
            sidecar
                .files()
                .iter()
                .map(|file| file.path.clone())
                .collect()
        },
    );
    prove_partition_sets(&manifest.authored_files, &current_taffy)?;
    let mut admitted = manifest.authored_files.clone();
    admitted.extend(current_taffy);
    if sidecar.is_some() {
        admitted.insert(sidecar_path);
    }
    if let Some(inventory) = &inventory {
        validate_visible_inventory(inventory, &admitted)?;
    }
    let authored = snapshot_authored(rooted, inventory.as_ref(), &manifest.authored_files)?;
    Ok((ExistingHtml { inventory, sidecar }, authored))
}

fn validate_visible_inventory(
    inventory: &Inventory,
    admitted_files: &BTreeSet<RelativePath>,
) -> Result<()> {
    for entry in inventory.entries() {
        let admitted = match entry.identity().kind() {
            NodeKind::Regular => admitted_files.contains(entry.path()),
            NodeKind::Directory => {
                let prefix = format!("{}/", entry.path().as_str());
                admitted_files
                    .iter()
                    .any(|path| path.as_str().starts_with(&prefix))
            }
            NodeKind::Symlink => false,
        };
        if !admitted {
            let alias = admitted_files
                .iter()
                .any(|path| paths_target_equal(path.as_str(), entry.path().as_str()));
            return Err(invalid_inventory(format!(
                "unknown{} entry in layout HTML root: {}",
                if alias { " or aliased" } else { "" },
                entry.path().as_str()
            )));
        }
    }
    Ok(())
}

fn snapshot_authored(
    rooted: RootedFs,
    inventory: Option<&Inventory>,
    authored: &BTreeSet<RelativePath>,
) -> Result<AuthoredPartition> {
    let mut files = Vec::with_capacity(authored.len());
    for path in authored {
        let entry = inventory
            .and_then(|inventory| inventory.find(path))
            .ok_or_else(|| {
                invalid_inventory(format!(
                    "manifest-authored HTML is missing: {}",
                    path.as_str()
                ))
            })?;
        if entry.identity().kind() != NodeKind::Regular {
            return Err(invalid_inventory(format!(
                "manifest-authored HTML is not regular: {}",
                path.as_str()
            )));
        }
        let rooted_path = joined(HTML_ROOT, path.as_str());
        let mut handle = rooted
            .open_file_handle(&rooted_path, CORPUS_FILE_MODE, false)
            .map_err(authored_error)?;
        let identity = rooted
            .validate_handle_at(&rooted_path, &handle, CORPUS_FILE_MODE)
            .map_err(authored_error)?;
        if !identity.matches_recovery(entry.identity()) {
            return Err(invalid_inventory(format!(
                "manifest-authored HTML identity differs from inventory: {}",
                path.as_str()
            )));
        }
        let mut bytes = Vec::new();
        handle.read_to_end(&mut bytes).map_err(|error| {
            GeneratorError::with_source(
                GeneratorErrorKind::InvalidInventory,
                "snapshot authored layout HTML",
                path.as_str(),
                error,
            )
        })?;
        rooted
            .validate_handle_at(&rooted_path, &handle, CORPUS_FILE_MODE)
            .map_err(authored_error)?;
        if entry.digest() != Some(&Sha256Digest::from_bytes(&bytes)) {
            return Err(invalid_inventory(format!(
                "manifest-authored HTML bytes differ from inventory: {}",
                path.as_str()
            )));
        }
        files.push(AuthoredFile {
            path: path.clone(),
            bytes,
            identity,
            handle,
        });
    }
    Ok(AuthoredPartition { rooted, files })
}

fn joined(parent: &str, child: &str) -> String {
    format!("{parent}/{child}")
}

fn invalid_inventory(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidInventory,
        "validate layout Taffy import inventory",
        detail,
    )
}

fn authored_error(source: GeneratorError) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::InvalidInventory,
        "revalidate authored layout HTML",
        source.to_string(),
        source,
    )
}
