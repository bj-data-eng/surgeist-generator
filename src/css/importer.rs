use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::core::{
    ArtifactPlan, ArtifactReservation, CORPUS_FILE_MODE, Domain, GenerationLease, Inventory,
    InventoryPolicy, NodeKind, ProtectedSource, ProtectedSourceDisjointness,
    ProtectedSourceInventory, ProtectedTreeEntryKind, PublicationInventory, PublicationPolicy,
    RootedFs, SnapshotEntry, VerifiedSourceSnapshot, verify_protected_git_source_inventory,
};
use crate::{
    GeneratorError, GeneratorErrorKind, PinnedSource, RelativePath, Result, RunScope, Sha256Digest,
};

use super::CssRequest;
use super::manifest::{CssManifest, SIDECAR_FILE};
use super::sidecar::{self, CssImportSidecar};

const MANIFEST_FILE: &str = "corpus.toml";
const GENERATOR: &str = "surgeist-css-generate";
const COMMAND: &str = "import-csstree";

pub(super) fn run(request: &CssRequest) -> Result<()> {
    run_impl(request, || {})
}

#[cfg(test)]
pub(super) fn run_with_pre_lease_hook(request: &CssRequest, hook: impl FnOnce()) -> Result<()> {
    run_impl(request, hook)
}

#[cfg(test)]
pub(super) fn run_with_inter_scan_hook(request: &CssRequest, hook: impl FnOnce()) -> Result<()> {
    run_impl_with_inter_scan_hook(request, || {}, hook)
}

fn run_impl(request: &CssRequest, pre_lease_hook: impl FnOnce()) -> Result<()> {
    run_impl_with_inter_scan_hook(request, pre_lease_hook, || {})
}

fn run_impl_with_inter_scan_hook(
    request: &CssRequest,
    pre_lease_hook: impl FnOnce(),
    inter_scan_hook: impl FnOnce(),
) -> Result<()> {
    let location = request.location();
    let manifest_path = location.corpus_root().join(MANIFEST_FILE);
    let manifest_bytes = read_manifest_file(&manifest_path)?;
    let manifest = super::manifest::parse(&manifest_bytes, &manifest_path)?;
    let pin = PinnedSource::new(
        "csstree",
        manifest.repository.clone(),
        manifest.revision.clone(),
        manifest.fixture_root.clone(),
    )?;
    let source_inventory = verify_protected_git_source_inventory(
        request
            .source_root()
            .expect("CssRequest guarantees an import source root"),
        &pin,
    )?;
    if source_inventory.verified().revision() != pin.revision() {
        return Err(GeneratorError::new(
            GeneratorErrorKind::SourceVerification,
            "verify CSS import source",
            "verified revision differs from the manifest pin",
        ));
    }
    let source = validate_snapshot(&manifest, source_inventory)?;
    let desired_sidecar = CssImportSidecar::from_snapshot(&pin, source.snapshot())?;
    let desired_sidecar_bytes = desired_sidecar.canonical_bytes()?;
    pre_lease_hook();

    let reservation = ArtifactReservation::new(Domain::Css)?;
    let import_root_path = manifest.import_root.join(location.corpus_root());
    let expectation_root_path = manifest.expectation_root.join(location.corpus_root());
    let external_stage_path = reservation.external_stage().join(location.corpus_root());
    let protection = ProtectedSourceDisjointness::for_mutation(
        location,
        &[
            ("CSS import root", import_root_path.as_path()),
            ("CSS transaction stage", external_stage_path.as_path()),
        ],
        &[
            ("CSS corpus manifest", manifest_path.as_path()),
            ("CSS expectation root", expectation_root_path.as_path()),
        ],
        &source,
    )?;
    let lease = GenerationLease::acquire_with_protected_source(
        location,
        Domain::Css,
        GENERATOR,
        &RunScope::Full,
        COMMAND,
        &protection,
    )?;

    let binding = lease.bind(location, Domain::Css)?;
    let operation = binding.validate(location, Domain::Css)?;
    let rooted = operation.rooted();
    protection.revalidate(rooted)?;
    revalidate_manifest(rooted, &manifest_bytes)?;
    let existing_import = inspect_import(rooted, &manifest)?;
    let existing_downstream = inspect_downstream(rooted, &manifest, source.snapshot())?;
    drop(operation);

    let sidecar_path = RelativePath::new(SIDECAR_FILE)?;
    let mut classified = existing_import.classified_paths();
    let mut retained = BTreeSet::from([sidecar_path.clone()]);
    let mut artifacts = vec![(sidecar_path, desired_sidecar_bytes)];
    for entry in &source.snapshot().entries {
        classified.insert(entry.path.clone());
        retained.insert(entry.path.clone());
        artifacts.push((entry.path.clone(), entry.bytes.clone()));
    }
    classified.extend(retained.iter().cloned());

    let inventory = PublicationInventory::new(
        classified.into_iter().collect(),
        retained.into_iter().collect(),
        Vec::new(),
    )?;
    let plan = ArtifactPlan::new(
        location,
        Domain::Css,
        &lease,
        manifest.import_root.clone(),
        PublicationPolicy::CleanFull,
        artifacts,
        inventory,
    )?
    .with_reservation(reservation)?;
    let revalidate = |rooted: &RootedFs| {
        protection.revalidate(rooted)?;
        revalidate_manifest(rooted, &manifest_bytes)?;
        if inspect_import(rooted, &manifest)? != existing_import {
            return Err(invalid_inventory(
                "current CSS import tree changed after held validation",
            ));
        }
        if inspect_downstream(rooted, &manifest, source.snapshot())? != existing_downstream {
            return Err(invalid_inventory(
                "CSS expectation tree changed after held validation",
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
    manifest: &CssManifest,
    source: ProtectedSourceInventory,
) -> Result<ProtectedSource> {
    let snapshot = source.snapshot();
    if snapshot.entries.len() != manifest.expected_files {
        return Err(invalid_inventory(format!(
            "manifest expected {} CSSTree files, verified snapshot contains {}",
            manifest.expected_files,
            snapshot.entries.len()
        )));
    }
    let mut verified_entries = Vec::with_capacity(snapshot.entries.len());
    for entry in &snapshot.entries {
        let display = entry.path.display();
        if entry.kind != ProtectedTreeEntryKind::Blob || entry.git_mode != "100644" {
            return Err(invalid_inventory(format!(
                "CSSTree fixture must be a Git mode 100644 blob: {display}"
            )));
        }
        let path = entry.path.to_relative_path().ok_or_else(|| {
            invalid_inventory(format!("CSSTree fixture path is not canonical: {display}"))
        })?;
        sidecar::validate_fixture_path(&path)?;
        let bytes = entry.bytes.as_ref().ok_or_else(|| {
            invalid_inventory(format!("CSSTree fixture blob bytes are absent: {display}"))
        })?;
        let digest = entry.digest.as_ref().ok_or_else(|| {
            invalid_inventory(format!("CSSTree fixture blob digest is absent: {display}"))
        })?;
        if digest != &Sha256Digest::from_bytes(bytes) {
            return Err(invalid_inventory(format!(
                "immutable snapshot digest mismatch: {display}"
            )));
        }
        verified_entries.push(SnapshotEntry {
            path,
            git_mode: entry.git_mode.clone(),
            blob_object_id: entry.object_id.clone(),
            bytes: bytes.clone(),
            digest: digest.clone(),
        });
    }
    if verified_entries
        .windows(2)
        .any(|pair| pair[0].path >= pair[1].path)
    {
        return Err(invalid_inventory(
            "CSSTree fixture paths are not strictly increasing",
        ));
    }
    let verified_snapshot = VerifiedSourceSnapshot {
        object_format: snapshot.object_format,
        entries: verified_entries,
    };
    source.into_protected_source(verified_snapshot)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExistingImport {
    inventory: Option<Inventory>,
    sidecar: Option<CssImportSidecar>,
}

impl ExistingImport {
    fn classified_paths(&self) -> BTreeSet<RelativePath> {
        let mut paths = BTreeSet::new();
        if let Some(sidecar) = &self.sidecar {
            paths.insert(
                RelativePath::new(SIDECAR_FILE).expect("the fixed sidecar path is canonical"),
            );
            paths.extend(sidecar.files().iter().map(|file| file.path.clone()));
        }
        paths
    }
}

fn inspect_import(rooted: &RootedFs, manifest: &CssManifest) -> Result<ExistingImport> {
    let inventory = Inventory::scan(
        rooted,
        manifest.import_root.as_str(),
        InventoryPolicy::FinalCorpus,
    )?;
    let Some(current) = inventory.as_ref() else {
        return Ok(ExistingImport {
            inventory,
            sidecar: None,
        });
    };
    if current.entries().is_empty() {
        return Ok(ExistingImport {
            inventory,
            sidecar: None,
        });
    }
    let sidecar_path = RelativePath::new(SIDECAR_FILE)?;
    let Some(entry) = current.find(&sidecar_path) else {
        return Err(invalid_inventory(
            "nonempty CSS import root has no canonical sidecar",
        ));
    };
    if entry.identity().kind() != NodeKind::Regular {
        return Err(invalid_inventory(
            "CSS import sidecar is not a regular file",
        ));
    }
    let bytes = rooted
        .read_file(
            &joined(manifest.import_root.as_str(), SIDECAR_FILE),
            CORPUS_FILE_MODE,
        )
        .map_err(|error| invalid_inventory_with_source("read CSS import sidecar", error))?;
    let sidecar = CssImportSidecar::parse_canonical(&bytes)?;
    let mut admitted = BTreeSet::from([sidecar_path]);
    admitted.extend(sidecar.files().iter().map(|file| file.path.clone()));
    validate_visible_inventory(current, &admitted, "CSS import root")?;
    Ok(ExistingImport {
        inventory,
        sidecar: Some(sidecar),
    })
}

type ExistingDownstream = super::historical::HistoricalInventory;

fn inspect_downstream(
    rooted: &RootedFs,
    manifest: &CssManifest,
    desired_snapshot: &VerifiedSourceSnapshot,
) -> Result<ExistingDownstream> {
    let historical = super::historical::inspect(rooted, manifest)?;
    let mut desired = desired_snapshot
        .entries
        .iter()
        .map(|entry| entry.path.clone())
        .collect::<BTreeSet<_>>();
    desired.insert(super::report::relative_path(manifest)?);
    historical.validate_union(&desired)?;
    Ok(historical)
}

fn validate_visible_inventory(
    inventory: &Inventory,
    admitted_files: &BTreeSet<RelativePath>,
    label: &str,
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
            return Err(invalid_inventory(format!(
                "unknown entry in {label}: {}",
                entry.path().as_str()
            )));
        }
    }
    Ok(())
}

pub(super) fn read_manifest_file(path: &Path) -> Result<Vec<u8>> {
    let before = fs::symlink_metadata(path).map_err(|error| {
        GeneratorError::with_source(
            GeneratorErrorKind::InvalidManifest,
            "read CSS corpus manifest",
            path.display().to_string(),
            error,
        )
    })?;
    require_manifest_metadata(path, &before)?;
    let bytes = fs::read(path).map_err(|error| {
        GeneratorError::with_source(
            GeneratorErrorKind::InvalidManifest,
            "read CSS corpus manifest",
            path.display().to_string(),
            error,
        )
    })?;
    let after = fs::symlink_metadata(path).map_err(|error| {
        GeneratorError::with_source(
            GeneratorErrorKind::InvalidManifest,
            "revalidate CSS corpus manifest",
            path.display().to_string(),
            error,
        )
    })?;
    require_manifest_metadata(path, &after)?;
    if !same_file_identity(&before, &after) || before.len() != after.len() {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidManifest,
            "read CSS corpus manifest",
            "manifest identity changed while it was read",
        ));
    }
    Ok(bytes)
}

fn require_manifest_metadata(path: &Path, metadata: &fs::Metadata) -> Result<()> {
    if !metadata.is_file() {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidManifest,
            "read CSS corpus manifest",
            format!("manifest is not a regular file: {}", path.display()),
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        if metadata.nlink() != 1 || metadata.permissions().mode() & 0o7777 != CORPUS_FILE_MODE {
            return Err(GeneratorError::new(
                GeneratorErrorKind::InvalidManifest,
                "read CSS corpus manifest",
                "manifest must be a single-link mode-0644 regular file",
            ));
        }
    }
    Ok(())
}

fn same_file_identity(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;

        left.dev() == right.dev() && left.ino() == right.ino()
    }
    #[cfg(not(unix))]
    {
        left.len() == right.len() && left.modified().ok() == right.modified().ok()
    }
}

pub(super) fn revalidate_manifest(rooted: &RootedFs, expected: &[u8]) -> Result<()> {
    let bytes = rooted
        .read_file(MANIFEST_FILE, CORPUS_FILE_MODE)
        .map_err(|error| {
            GeneratorError::with_source(
                GeneratorErrorKind::InvalidManifest,
                "revalidate CSS corpus manifest",
                "held manifest is unavailable",
                error,
            )
        })?;
    if bytes != expected {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidManifest,
            "revalidate CSS corpus manifest",
            "manifest bytes changed after preflight",
        ));
    }
    Ok(())
}

fn joined(parent: &str, child: &str) -> String {
    format!("{parent}/{child}")
}

fn invalid_inventory(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidInventory,
        "validate CSS import inventory",
        detail,
    )
}

fn invalid_inventory_with_source(operation: &str, source: GeneratorError) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::InvalidInventory,
        operation,
        source.to_string(),
        source,
    )
}
