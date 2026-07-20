use std::collections::{BTreeMap, BTreeSet};

use crate::core::{
    CORPUS_FILE_MODE, Domain, GenerationCheck, HeldIdentity, Inventory, InventoryPolicy, NodeKind,
    RootedFs,
};
use crate::{GeneratorError, GeneratorErrorKind, RelativePath, Result, Sha256Digest};

use super::LayoutRequest;
use super::manifest::{HTML_ROOT, LayoutManifest, MANIFEST_FILE, SIDECAR_FILE};
use super::report::{
    self, CheckState, CurrentCorpus, DesiredDisposition, DesiredSource, desired_disposition,
};
use super::sidecar::TaffyImportSidecar;

const HELPER_SCRIPT: &str = "scripts/gentest/test_helper.js";
const BASE_STYLE: &str = "scripts/gentest/test_base_style.css";
const BASE_STYLE_LITERAL: &[u8] = b"test_base_style.css";
const XML_ROOT: &str = "xml";

pub(super) fn run(request: &LayoutRequest) -> Result<()> {
    let (initial, state) = inspect(request)?;
    if state == CheckState::Stale {
        return Err(verification(
            "layout corpus is absent, stale, diagnostic, or migration-only; run a clean full generation",
        ));
    }

    let check = GenerationCheck::acquire(request.location(), Domain::Layout)
        .map_err(coordination_verification)?;
    let repeated = inspect(request).map_err(changed_during_check);
    let finish = check.finish().map_err(coordination_verification);
    match (repeated, finish) {
        (_, Err(error)) => Err(error),
        (Err(error), Ok(())) => Err(error),
        (Ok((_repeated, CheckState::Stale)), Ok(())) => Err(verification(
            "layout corpus became stale during offline checking",
        )),
        (Ok((repeated, CheckState::Current)), Ok(())) if repeated == initial => Ok(()),
        (Ok(_), Ok(())) => Err(verification(
            "layout corpus bytes or identities changed during offline checking",
        )),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CheckSnapshot {
    manifest_bytes: Vec<u8>,
    manifest_identity: HeldIdentity,
    html_inventory: Inventory,
    helper_identity: HeldIdentity,
    helper_digest: Sha256Digest,
    base_style_identity: HeldIdentity,
    base_style_digest: Sha256Digest,
    xml_inventory: Option<Inventory>,
}

fn inspect(request: &LayoutRequest) -> Result<(CheckSnapshot, CheckState)> {
    let location = request.location();
    let manifest_path = location.corpus_root().join(MANIFEST_FILE);
    let manifest_bytes = super::manifest::read_file(&manifest_path)?;
    let manifest = super::manifest::parse(&manifest_bytes, &manifest_path)?;
    let rooted = RootedFs::open_corpus(location)?;
    let manifest_identity = required_regular_identity(&rooted, MANIFEST_FILE, "corpus manifest")?;
    let held_manifest = rooted
        .read_file(MANIFEST_FILE, CORPUS_FILE_MODE)
        .map_err(manifest_revalidation)?;
    if held_manifest != manifest_bytes {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidManifest,
            "revalidate layout corpus manifest",
            "manifest bytes changed during offline checking",
        ));
    }

    let html = inspect_html(&rooted, &manifest)?;
    let (helper_identity, helper_bytes) = required_input(&rooted, HELPER_SCRIPT, "helper script")?;
    let (base_style_identity, base_style_bytes) =
        required_input(&rooted, BASE_STYLE, "base style")?;
    let helper_digest = Sha256Digest::from_bytes(&helper_bytes);
    let base_style_digest = Sha256Digest::from_bytes(&base_style_bytes);
    let xml_inventory = Inventory::scan(&rooted, XML_ROOT, InventoryPolicy::FinalCorpus)?;
    let current = CurrentCorpus {
        manifest: &manifest,
        manifest_digest: Sha256Digest::from_bytes(&manifest_bytes),
        helper_digest: helper_digest.clone(),
        base_style_digest: base_style_digest.clone(),
        sidecar_digest: html.sidecar_digest,
        sources: html.sources,
    };
    let state = report::validate(&rooted, xml_inventory.as_ref(), &current)?;
    Ok((
        CheckSnapshot {
            manifest_bytes,
            manifest_identity,
            html_inventory: html.inventory,
            helper_identity,
            helper_digest,
            base_style_identity,
            base_style_digest,
            xml_inventory,
        },
        state,
    ))
}

struct HtmlState {
    inventory: Inventory,
    sidecar_digest: Sha256Digest,
    sources: BTreeMap<RelativePath, DesiredSource>,
}

fn inspect_html(rooted: &RootedFs, manifest: &LayoutManifest) -> Result<HtmlState> {
    let inventory = Inventory::scan(rooted, HTML_ROOT, InventoryPolicy::FinalCorpus)?
        .ok_or_else(|| verification("layout HTML root is absent; run import-taffy"))?;
    let sidecar_path = RelativePath::new(SIDECAR_FILE)?;
    let sidecar_entry = inventory
        .find(&sidecar_path)
        .ok_or_else(|| verification("Taffy import sidecar is absent; run import-taffy"))?;
    if sidecar_entry.identity().kind() != NodeKind::Regular {
        return Err(invalid_inventory(
            "Taffy import sidecar is not a regular file",
        ));
    }
    let sidecar_bytes = rooted
        .read_file(&format!("{HTML_ROOT}/{SIDECAR_FILE}"), CORPUS_FILE_MODE)
        .map_err(html_io)?;
    let sidecar = TaffyImportSidecar::parse_canonical(&sidecar_bytes)?;
    let sidecar_is_stale = sidecar.revision() != &manifest.revision
        || sidecar.source_file_count() != manifest.expected_source_files;

    let taffy_paths = sidecar
        .files()
        .iter()
        .map(|file| file.path.clone())
        .collect::<BTreeSet<_>>();
    if manifest
        .authored_files
        .iter()
        .any(|path| taffy_paths.contains(path))
    {
        return Err(invalid_inventory(
            "manifest-authored and Taffy-owned HTML paths collide",
        ));
    }
    let mut admitted = manifest.authored_files.clone();
    admitted.extend(taffy_paths);
    admitted.insert(sidecar_path);
    validate_html_inventory(&inventory, &admitted)?;

    let mut sources = BTreeMap::new();
    for file in sidecar.files() {
        let entry = inventory.find(&file.path).ok_or_else(|| {
            verification(format!(
                "imported Taffy fixture is absent: {}",
                file.path.as_str()
            ))
        })?;
        if entry.digest() != Some(&file.sha256) {
            return Err(verification(format!(
                "imported Taffy fixture is stale: {}",
                file.path.as_str()
            )));
        }
        let bytes = rooted
            .read_file(
                &format!("{HTML_ROOT}/{}", file.path.as_str()),
                CORPUS_FILE_MODE,
            )
            .map_err(html_io)?;
        let source_path = RelativePath::new(format!("html/{}", file.path.as_str()))?;
        sources.insert(
            source_path,
            DesiredSource {
                digest: Sha256Digest::from_bytes(&bytes),
                uses_base_style: contains(&bytes, BASE_STYLE_LITERAL),
                disposition: DesiredDisposition::Active,
            },
        );
    }
    for case in &manifest.authored_cases {
        let entry = inventory.find(&case.source).ok_or_else(|| {
            verification(format!(
                "manifest-authored layout fixture is absent: {}",
                case.source.as_str()
            ))
        })?;
        if entry.identity().kind() != NodeKind::Regular {
            return Err(invalid_inventory(format!(
                "manifest-authored layout fixture is not regular: {}",
                case.source.as_str()
            )));
        }
        let bytes = rooted
            .read_file(
                &format!("{HTML_ROOT}/{}", case.source.as_str()),
                CORPUS_FILE_MODE,
            )
            .map_err(html_io)?;
        if entry.digest() != Some(&Sha256Digest::from_bytes(&bytes)) {
            return Err(invalid_inventory(format!(
                "manifest-authored layout fixture changed during inventory: {}",
                case.source.as_str()
            )));
        }
        let source_path = RelativePath::new(format!("html/{}", case.source.as_str()))?;
        sources.insert(
            source_path,
            DesiredSource {
                digest: Sha256Digest::from_bytes(&bytes),
                uses_base_style: contains(&bytes, BASE_STYLE_LITERAL),
                disposition: desired_disposition(case.status, case.id.clone(), case.reason.clone()),
            },
        );
    }
    if sidecar_is_stale {
        return Err(verification(
            "Taffy import sidecar is stale against the corpus manifest",
        ));
    }
    Ok(HtmlState {
        inventory,
        sidecar_digest: Sha256Digest::from_bytes(&sidecar_bytes),
        sources,
    })
}

fn validate_html_inventory(
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
            return Err(invalid_inventory(format!(
                "unknown entry in layout HTML root: {}",
                entry.path().as_str()
            )));
        }
    }
    Ok(())
}

fn required_input(rooted: &RootedFs, path: &str, label: &str) -> Result<(HeldIdentity, Vec<u8>)> {
    let identity = required_regular_identity(rooted, path, label)?;
    let bytes = rooted
        .read_file(path, CORPUS_FILE_MODE)
        .map_err(|source| input_io(label, source))?;
    Ok((identity, bytes))
}

fn required_regular_identity(rooted: &RootedFs, path: &str, label: &str) -> Result<HeldIdentity> {
    if !rooted
        .exists(path)
        .map_err(|source| input_io(label, source))?
    {
        return Err(verification(format!("layout {label} is absent")));
    }
    let identity = rooted
        .identity_at(path)
        .map_err(|source| input_io(label, source))?
        .ok_or_else(|| verification(format!("layout {label} disappeared")))?;
    if identity.kind() != NodeKind::Regular
        || identity.mode() != CORPUS_FILE_MODE
        || identity.link_count() != Some(1)
    {
        return Err(invalid_inventory(format!(
            "layout {label} must be a single-link mode-0644 regular file"
        )));
    }
    Ok(identity)
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
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

fn changed_during_check(source: GeneratorError) -> GeneratorError {
    if source.kind() == GeneratorErrorKind::UnsupportedPlatform {
        return source;
    }
    GeneratorError::with_source(
        GeneratorErrorKind::Verification,
        "revalidate offline layout corpus",
        source.to_string(),
        source,
    )
}

fn manifest_revalidation(source: GeneratorError) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::InvalidManifest,
        "revalidate layout corpus manifest",
        source.to_string(),
        source,
    )
}

fn input_io(label: &str, source: GeneratorError) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::InvalidInventory,
        "read layout corpus helper",
        format!("{label}: {source}"),
        source,
    )
}

fn html_io(source: GeneratorError) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::InvalidInventory,
        "read layout HTML inventory",
        source.to_string(),
        source,
    )
}

fn invalid_inventory(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidInventory,
        "validate offline layout corpus",
        detail,
    )
}

fn verification(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::Verification,
        "check layout corpus",
        detail,
    )
}
