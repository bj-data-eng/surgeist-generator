use std::collections::BTreeMap;

use crate::{
    ArtifactProvenance, GenerationReport, GeneratorError, GeneratorErrorKind, ManifestVersion,
    RelativePath, ReportArtifact, Result, Sha256Digest,
};

use super::expectation::DerivedExpectations;
use super::manifest::{CSSTREE_REPOSITORY, CssManifest, REPORT_RELATIVE};

const GENERATOR: &str = "surgeist-css-generate";

pub(super) fn build(
    manifest: &CssManifest,
    manifest_bytes: &[u8],
    sidecar_digest: &Sha256Digest,
    expectations: &DerivedExpectations,
) -> Result<Vec<u8>> {
    let mut artifacts = Vec::with_capacity(expectations.artifacts.len());
    let mut artifact_case_count = 0_usize;
    for expectation in &expectations.artifacts {
        artifact_case_count = artifact_case_count
            .checked_add(expectation.case_count)
            .ok_or_else(|| invalid_inventory("CSS report case count overflow"))?;
        let mut domain_provenance = BTreeMap::new();
        domain_provenance.insert("csstree-import".to_owned(), sidecar_digest.clone());
        let provenance = ArtifactProvenance::new(
            prefixed(&manifest.import_root, &expectation.path)?,
            expectation.source_digest.clone(),
            GENERATOR,
            ManifestVersion::new(1)?,
            domain_provenance,
        )?;
        artifacts.push(ReportArtifact::new(
            provenance,
            prefixed(&manifest.expectation_root, &expectation.path)?,
            Sha256Digest::from_bytes(&expectation.bytes),
            expectation.case_count,
        )?);
    }
    if artifact_case_count != expectations.counts.total()?
        || artifact_case_count != manifest.expected_cases
    {
        return Err(invalid_inventory(
            "CSS report artifact counts do not match derived disposition counts",
        ));
    }
    let report = GenerationReport::new(
        Sha256Digest::from_bytes(manifest_bytes),
        CSSTREE_REPOSITORY,
        manifest.revision.clone(),
        expectations.counts,
        artifacts,
    )?;
    let mut bytes = serde_json::to_vec_pretty(&report).map_err(|error| {
        GeneratorError::with_source(
            GeneratorErrorKind::InvalidInventory,
            "serialize CSS generation report",
            "report serialization failed",
            error,
        )
    })?;
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn relative_path(manifest: &CssManifest) -> Result<RelativePath> {
    let path = manifest
        .report_file
        .as_str()
        .strip_prefix(manifest.expectation_root.as_str())
        .and_then(|suffix| suffix.strip_prefix('/'))
        .ok_or_else(|| invalid_inventory("CSS report path is outside expectation_root"))?;
    let relative = RelativePath::new(path)?;
    if relative.as_str() != REPORT_RELATIVE {
        return Err(invalid_inventory("CSS report path is not canonical"));
    }
    Ok(relative)
}

fn prefixed(root: &RelativePath, path: &RelativePath) -> Result<RelativePath> {
    RelativePath::new(format!("{}/{}", root.as_str(), path.as_str()))
        .map_err(|error| invalid_inventory(error.to_string()))
}

fn invalid_inventory(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidInventory,
        "construct CSS generation report",
        detail,
    )
}
