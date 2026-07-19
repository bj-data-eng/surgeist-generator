use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use crate::core::{
    ArtifactPlan, ArtifactReservation, CORPUS_FILE_MODE, Domain, GenerationLease, Inventory,
    InventoryPolicy, NodeKind, ProtectedSource, ProtectedSourceDisjointness, PublicationInventory,
    PublicationPolicy, RootedFs, VerifiedSourceSnapshot, verify_protected_git_source,
};
use crate::{
    GenerationReport, GeneratorError, GeneratorErrorKind, PinnedSource, RelativePath, Result,
    RunScope, Sha256Digest,
};

use super::CssRequest;
use super::manifest::{CSSTREE_REPOSITORY, CssManifest, REPORT_RELATIVE, SIDECAR_FILE};
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
    let source = verify_protected_git_source(
        request
            .source_root()
            .expect("CssRequest guarantees an import source root"),
        &pin,
    )?;
    if source.verified().revision() != pin.revision() {
        return Err(GeneratorError::new(
            GeneratorErrorKind::SourceVerification,
            "verify CSS import source",
            "verified revision differs from the manifest pin",
        ));
    }
    validate_snapshot(&manifest, &source)?;
    let desired_sidecar = CssImportSidecar::from_snapshot(&pin, source.snapshot())?;
    let desired_sidecar_bytes = desired_sidecar.canonical_bytes()?;
    let desired_sidecar_digest = Sha256Digest::from_bytes(&desired_sidecar_bytes);
    let manifest_digest = Sha256Digest::from_bytes(&manifest_bytes);

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
    let existing_downstream = inspect_downstream(
        rooted,
        &manifest,
        &manifest_digest,
        &desired_sidecar_digest,
        source.snapshot(),
    )?;
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
        if inspect_downstream(
            rooted,
            &manifest,
            &manifest_digest,
            &desired_sidecar_digest,
            source.snapshot(),
        )? != existing_downstream
        {
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

fn validate_snapshot(manifest: &CssManifest, source: &ProtectedSource) -> Result<()> {
    let snapshot = source.snapshot();
    if snapshot.entries.len() != manifest.expected_files {
        return Err(invalid_inventory(format!(
            "manifest expected {} CSSTree files, verified snapshot contains {}",
            manifest.expected_files,
            snapshot.entries.len()
        )));
    }
    for entry in &snapshot.entries {
        if entry.git_mode != "100644" {
            return Err(invalid_inventory(format!(
                "CSSTree fixture must use Git mode 100644: {}",
                entry.path.as_str()
            )));
        }
        sidecar::validate_fixture_path(&entry.path)?;
        if entry.digest != Sha256Digest::from_bytes(&entry.bytes) {
            return Err(invalid_inventory(format!(
                "immutable snapshot digest mismatch: {}",
                entry.path.as_str()
            )));
        }
    }
    Ok(())
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct ExistingDownstream {
    inventory: Option<Inventory>,
    report: Option<GenerationReport>,
    state: DownstreamPublicationState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DownstreamPublicationState {
    Absent,
    Current,
    Stale,
}

fn inspect_downstream(
    rooted: &RootedFs,
    manifest: &CssManifest,
    manifest_digest: &Sha256Digest,
    desired_sidecar_digest: &Sha256Digest,
    desired_snapshot: &VerifiedSourceSnapshot,
) -> Result<ExistingDownstream> {
    let inventory = Inventory::scan(
        rooted,
        manifest.expectation_root.as_str(),
        InventoryPolicy::FinalCorpus,
    )?;
    let Some(current) = inventory.as_ref() else {
        return Ok(ExistingDownstream {
            inventory,
            report: None,
            state: DownstreamPublicationState::Absent,
        });
    };
    if current.entries().is_empty() {
        return Ok(ExistingDownstream {
            inventory,
            report: None,
            state: DownstreamPublicationState::Absent,
        });
    }
    let report_relative = report_relative(manifest)?;
    let Some(entry) = current.find(&report_relative) else {
        return Err(invalid_inventory(
            "nonempty CSS expectation root has no historical full report",
        ));
    };
    if entry.identity().kind() != NodeKind::Regular {
        return Err(invalid_inventory(
            "CSS historical full report is not a regular file",
        ));
    }
    let report_bytes = rooted
        .read_file(manifest.report_file.as_str(), CORPUS_FILE_MODE)
        .map_err(|error| invalid_inventory_with_source("read CSS historical report", error))?;
    let report: GenerationReport = serde_json::from_slice(&report_bytes).map_err(|error| {
        GeneratorError::with_source(
            GeneratorErrorKind::InvalidInventory,
            "parse CSS historical report",
            "invalid report JSON",
            error,
        )
    })?;
    let mut canonical = serde_json::to_vec_pretty(&report).map_err(|error| {
        GeneratorError::with_source(
            GeneratorErrorKind::InvalidInventory,
            "serialize CSS historical report",
            "report serialization failed",
            error,
        )
    })?;
    canonical.push(b'\n');
    if canonical != report_bytes {
        return Err(invalid_inventory(
            "CSS historical report bytes are not canonical",
        ));
    }
    let admitted = validate_report(manifest, &report, report_relative)?;
    validate_visible_inventory(current, &admitted, "CSS expectation root")?;
    let state = if downstream_is_current(
        current,
        manifest,
        manifest_digest,
        desired_sidecar_digest,
        desired_snapshot,
        &report,
    )? {
        DownstreamPublicationState::Current
    } else {
        DownstreamPublicationState::Stale
    };
    Ok(ExistingDownstream {
        inventory,
        report: Some(report),
        state,
    })
}

fn downstream_is_current(
    inventory: &Inventory,
    manifest: &CssManifest,
    manifest_digest: &Sha256Digest,
    desired_sidecar_digest: &Sha256Digest,
    desired_snapshot: &VerifiedSourceSnapshot,
    report: &GenerationReport,
) -> Result<bool> {
    if report.manifest_digest() != manifest_digest
        || report.source_revision() != &manifest.revision
        || report.artifacts().len() != desired_snapshot.entries.len()
        || report.counts().total()? != manifest.expected_cases
    {
        return Ok(false);
    }
    for source in &desired_snapshot.entries {
        let Some(artifact) = report.artifacts().iter().find(|artifact| {
            strip_root(artifact.output_path(), &manifest.expectation_root).as_ref()
                == Some(&source.path)
        }) else {
            return Ok(false);
        };
        let provenance = artifact.provenance();
        if strip_root(provenance.source_path(), &manifest.import_root).as_ref()
            != Some(&source.path)
            || provenance.source_digest() != &source.digest
            || provenance.domain_provenance().get("csstree-import") != Some(desired_sidecar_digest)
        {
            return Ok(false);
        }
        let Some(output) = inventory.find(&source.path) else {
            return Ok(false);
        };
        if output.identity().kind() != NodeKind::Regular
            || output.digest() != Some(artifact.output_digest())
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn validate_report(
    manifest: &CssManifest,
    report: &GenerationReport,
    report_relative: RelativePath,
) -> Result<BTreeSet<RelativePath>> {
    if report.source_repository() != CSSTREE_REPOSITORY {
        return Err(invalid_inventory(
            "CSS historical report has a noncanonical repository",
        ));
    }
    if report.artifacts().is_empty() || report.counts().failed_to_generate() != 0 {
        return Err(invalid_inventory(
            "CSS historical report must contain artifacts and no generation failures",
        ));
    }
    let mut case_count = 0_usize;
    let mut admitted = BTreeSet::from([report_relative]);
    for artifact in report.artifacts() {
        case_count = case_count
            .checked_add(artifact.case_count())
            .ok_or_else(|| invalid_inventory("CSS historical report case count overflow"))?;
        let provenance = artifact.provenance();
        if provenance.generator() != GENERATOR
            || provenance.schema_version().get() != 1
            || provenance.domain_provenance().len() != 1
            || !provenance
                .domain_provenance()
                .contains_key("csstree-import")
        {
            return Err(invalid_inventory(
                "CSS historical artifact provenance is noncanonical",
            ));
        }
        let source_relative = strip_root(provenance.source_path(), &manifest.import_root)
            .ok_or_else(|| {
                invalid_inventory("CSS historical source path is outside import_root")
            })?;
        let output_relative = strip_root(artifact.output_path(), &manifest.expectation_root)
            .ok_or_else(|| {
                invalid_inventory("CSS historical output path is outside expectation_root")
            })?;
        if source_relative != output_relative || output_relative.as_str() == REPORT_RELATIVE {
            return Err(invalid_inventory(
                "CSS historical source/output mapping or report reservation is invalid",
            ));
        }
        sidecar::validate_fixture_path(&source_relative)?;
        if !admitted.insert(output_relative) {
            return Err(invalid_inventory(
                "CSS historical report repeats an output path",
            ));
        }
    }
    if report.counts().total()? != case_count {
        return Err(invalid_inventory(
            "CSS historical report counts do not match artifact case counts",
        ));
    }
    Ok(admitted)
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

fn report_relative(manifest: &CssManifest) -> Result<RelativePath> {
    strip_root(&manifest.report_file, &manifest.expectation_root)
        .ok_or_else(|| invalid_inventory("CSS report path is outside the expectation root"))
}

fn strip_root(path: &RelativePath, root: &RelativePath) -> Option<RelativePath> {
    path.as_str()
        .strip_prefix(root.as_str())
        .and_then(|suffix| suffix.strip_prefix('/'))
        .and_then(|relative| RelativePath::new(relative).ok())
}

fn read_manifest_file(path: &Path) -> Result<Vec<u8>> {
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

fn revalidate_manifest(rooted: &RootedFs, expected: &[u8]) -> Result<()> {
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
