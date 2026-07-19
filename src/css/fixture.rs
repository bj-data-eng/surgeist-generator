use std::collections::BTreeSet;

use crate::core::{CORPUS_FILE_MODE, Inventory, InventoryPolicy, NodeKind, RootedFs};
use crate::{GeneratorError, GeneratorErrorKind, RelativePath, Result, Sha256Digest};

use super::manifest::{CssManifest, SIDECAR_FILE};
use super::sidecar::CssImportSidecar;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ValidatedImport {
    inventory: Inventory,
    sidecar_digest: Sha256Digest,
    fixtures: Vec<ValidatedFixture>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ValidatedFixture {
    pub(super) path: RelativePath,
    pub(super) bytes: Vec<u8>,
    pub(super) digest: Sha256Digest,
}

impl ValidatedImport {
    pub(super) fn sidecar_digest(&self) -> &Sha256Digest {
        &self.sidecar_digest
    }

    pub(super) fn fixtures(&self) -> &[ValidatedFixture] {
        &self.fixtures
    }
}

pub(super) fn inspect(rooted: &RootedFs, manifest: &CssManifest) -> Result<ValidatedImport> {
    let inventory = Inventory::scan(
        rooted,
        manifest.import_root.as_str(),
        InventoryPolicy::FinalCorpus,
    )?
    .ok_or_else(|| verification("CSS import root is absent"))?;
    if inventory.entries().is_empty() {
        return Err(verification("CSS import root is empty"));
    }

    let sidecar_path = RelativePath::new(SIDECAR_FILE)?;
    let sidecar_entry = inventory
        .find(&sidecar_path)
        .ok_or_else(|| invalid_inventory("nonempty CSS import root has no canonical sidecar"))?;
    if sidecar_entry.identity().kind() != NodeKind::Regular {
        return Err(invalid_inventory(
            "CSS import sidecar is not a regular file",
        ));
    }
    let sidecar_bytes = rooted
        .read_file(
            &joined(manifest.import_root.as_str(), SIDECAR_FILE),
            CORPUS_FILE_MODE,
        )
        .map_err(|error| invalid_inventory_with_source("read CSS import sidecar", error))?;
    let sidecar = CssImportSidecar::parse_canonical(&sidecar_bytes)?;
    if sidecar.source().repository_url() != manifest.repository
        || sidecar.source().revision() != &manifest.revision
        || sidecar.source().source_subdirectory() != &manifest.fixture_root
        || sidecar.files().len() != manifest.expected_files
    {
        return Err(verification(
            "CSS import sidecar does not match the current manifest",
        ));
    }

    let mut admitted = BTreeSet::from([sidecar_path]);
    admitted.extend(sidecar.files().iter().map(|file| file.path.clone()));
    validate_visible_inventory(&inventory, &admitted, "CSS import root")?;

    let mut fixtures = Vec::with_capacity(sidecar.files().len());
    for file in sidecar.files() {
        let Some(entry) = inventory.find(&file.path) else {
            return Err(verification(format!(
                "CSS imported fixture is absent: {}",
                file.path.as_str()
            )));
        };
        if entry.identity().kind() != NodeKind::Regular {
            return Err(invalid_inventory(format!(
                "CSS imported fixture is not regular: {}",
                file.path.as_str()
            )));
        }
        let bytes = rooted
            .read_file(
                &joined(manifest.import_root.as_str(), file.path.as_str()),
                CORPUS_FILE_MODE,
            )
            .map_err(|error| invalid_inventory_with_source("read CSS imported fixture", error))?;
        let digest = Sha256Digest::from_bytes(&bytes);
        if digest != file.sha256 || entry.digest() != Some(&file.sha256) {
            return Err(verification(format!(
                "CSS imported fixture digest is stale: {}",
                file.path.as_str()
            )));
        }
        fixtures.push(ValidatedFixture {
            path: file.path.clone(),
            bytes,
            digest,
        });
    }

    Ok(ValidatedImport {
        inventory,
        sidecar_digest: Sha256Digest::from_bytes(&sidecar_bytes),
        fixtures,
    })
}

pub(super) fn validate_visible_inventory(
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

fn joined(parent: &str, child: &str) -> String {
    format!("{parent}/{child}")
}

fn invalid_inventory(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidInventory,
        "validate current CSS import",
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

fn verification(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::Verification,
        "validate current CSS import",
        detail,
    )
}
