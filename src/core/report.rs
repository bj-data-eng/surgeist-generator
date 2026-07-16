use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    GeneratorError, GeneratorErrorKind, ManifestVersion, RelativePath, Result, Sha256Digest,
    SourceRevision,
};

/// Shared provenance attached to one generated artifact.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ArtifactProvenance {
    source_path: RelativePath,
    source_digest: Sha256Digest,
    generator: String,
    schema_version: ManifestVersion,
    domain_provenance: BTreeMap<String, Sha256Digest>,
}

impl ArtifactProvenance {
    pub fn new(
        source_path: RelativePath,
        source_digest: Sha256Digest,
        generator: impl Into<String>,
        schema_version: ManifestVersion,
        domain_provenance: BTreeMap<String, Sha256Digest>,
    ) -> Result<Self> {
        let generator = generator.into();
        if generator.is_empty() || generator.trim() != generator || generator.contains('\0') {
            return Err(invalid_report(
                "generator name must be nonempty and trimmed",
            ));
        }
        if domain_provenance
            .keys()
            .any(|key| key.is_empty() || key.trim() != key || key.contains('\0'))
        {
            return Err(invalid_report(
                "domain provenance keys must be nonempty and trimmed",
            ));
        }
        Ok(Self {
            source_path,
            source_digest,
            generator,
            schema_version,
            domain_provenance,
        })
    }

    #[must_use]
    pub const fn source_path(&self) -> &RelativePath {
        &self.source_path
    }

    #[must_use]
    pub const fn source_digest(&self) -> &Sha256Digest {
        &self.source_digest
    }

    #[must_use]
    pub fn generator(&self) -> &str {
        &self.generator
    }

    #[must_use]
    pub const fn schema_version(&self) -> ManifestVersion {
        self.schema_version
    }

    #[must_use]
    pub const fn domain_provenance(&self) -> &BTreeMap<String, Sha256Digest> {
        &self.domain_provenance
    }
}

impl<'de> Deserialize<'de> for ArtifactProvenance {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawProvenance {
            source_path: RelativePath,
            source_digest: Sha256Digest,
            generator: String,
            schema_version: ManifestVersion,
            domain_provenance: BTreeMap<String, Sha256Digest>,
        }

        let raw = RawProvenance::deserialize(deserializer)?;
        Self::new(
            raw.source_path,
            raw.source_digest,
            raw.generator,
            raw.schema_version,
            raw.domain_provenance,
        )
        .map_err(serde::de::Error::custom)
    }
}

/// One deterministic artifact entry in a generation report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ReportArtifact {
    provenance: ArtifactProvenance,
    output_path: RelativePath,
    output_digest: Sha256Digest,
    case_count: usize,
}

impl ReportArtifact {
    #[must_use]
    pub const fn new(
        provenance: ArtifactProvenance,
        output_path: RelativePath,
        output_digest: Sha256Digest,
        case_count: usize,
    ) -> Self {
        Self {
            provenance,
            output_path,
            output_digest,
            case_count,
        }
    }

    #[must_use]
    pub const fn provenance(&self) -> &ArtifactProvenance {
        &self.provenance
    }

    #[must_use]
    pub const fn output_path(&self) -> &RelativePath {
        &self.output_path
    }

    #[must_use]
    pub const fn output_digest(&self) -> &Sha256Digest {
        &self.output_digest
    }

    #[must_use]
    pub const fn case_count(&self) -> usize {
        self.case_count
    }
}

impl<'de> Deserialize<'de> for ReportArtifact {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawArtifact {
            provenance: ArtifactProvenance,
            output_path: RelativePath,
            output_digest: Sha256Digest,
            case_count: usize,
        }

        let raw = RawArtifact::deserialize(deserializer)?;
        Ok(Self::new(
            raw.provenance,
            raw.output_path,
            raw.output_digest,
            raw.case_count,
        ))
    }
}

/// Exact disposition and failure counts for a full generation report.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationCounts {
    active: usize,
    expected_fail: usize,
    unsupported: usize,
    quarantined: usize,
    failed_to_generate: usize,
}

impl GenerationCounts {
    #[must_use]
    pub const fn new(
        active: usize,
        expected_fail: usize,
        unsupported: usize,
        quarantined: usize,
        failed_to_generate: usize,
    ) -> Self {
        Self {
            active,
            expected_fail,
            unsupported,
            quarantined,
            failed_to_generate,
        }
    }

    pub fn total(self) -> Result<usize> {
        self.active
            .checked_add(self.expected_fail)
            .and_then(|value| value.checked_add(self.unsupported))
            .and_then(|value| value.checked_add(self.quarantined))
            .and_then(|value| value.checked_add(self.failed_to_generate))
            .ok_or_else(|| invalid_report("generation count overflow"))
    }

    #[must_use]
    pub const fn active(self) -> usize {
        self.active
    }

    #[must_use]
    pub const fn expected_fail(self) -> usize {
        self.expected_fail
    }

    #[must_use]
    pub const fn unsupported(self) -> usize {
        self.unsupported
    }

    #[must_use]
    pub const fn quarantined(self) -> usize {
        self.quarantined
    }

    #[must_use]
    pub const fn failed_to_generate(self) -> usize {
        self.failed_to_generate
    }
}

/// Checked shared report header, source pin, counts, and sorted artifact inventory.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct GenerationReport {
    manifest_digest: Sha256Digest,
    source_repository: String,
    source_revision: SourceRevision,
    counts: GenerationCounts,
    artifacts: Vec<ReportArtifact>,
}

impl GenerationReport {
    pub fn new(
        manifest_digest: Sha256Digest,
        source_repository: impl Into<String>,
        source_revision: SourceRevision,
        counts: GenerationCounts,
        mut artifacts: Vec<ReportArtifact>,
    ) -> Result<Self> {
        let source_repository = source_repository.into();
        if source_repository.is_empty()
            || source_repository.trim() != source_repository
            || source_repository.contains('\0')
        {
            return Err(invalid_report(
                "source repository must be nonempty and trimmed",
            ));
        }
        artifacts.sort_by(|left, right| left.output_path.cmp(&right.output_path));
        let mut outputs = BTreeSet::new();
        for artifact in &artifacts {
            if !outputs.insert(artifact.output_path.clone()) {
                return Err(invalid_report(format!(
                    "duplicate report output: {}",
                    artifact.output_path.as_str()
                )));
            }
        }
        let artifact_cases = artifacts.iter().try_fold(0_usize, |total, artifact| {
            total
                .checked_add(artifact.case_count)
                .ok_or_else(|| invalid_report("artifact case count overflow"))
        })?;
        if artifact_cases != counts.total()? {
            return Err(invalid_report(format!(
                "artifact case count {artifact_cases} does not equal report count {}",
                counts.total()?
            )));
        }
        Ok(Self {
            manifest_digest,
            source_repository,
            source_revision,
            counts,
            artifacts,
        })
    }

    #[must_use]
    pub const fn manifest_digest(&self) -> &Sha256Digest {
        &self.manifest_digest
    }

    #[must_use]
    pub fn source_repository(&self) -> &str {
        &self.source_repository
    }

    #[must_use]
    pub const fn source_revision(&self) -> &SourceRevision {
        &self.source_revision
    }

    #[must_use]
    pub const fn counts(&self) -> GenerationCounts {
        self.counts
    }

    #[must_use]
    pub fn artifacts(&self) -> &[ReportArtifact] {
        &self.artifacts
    }

    /// Recomputes the manifest, source, and output digests beneath `corpus_root`.
    pub fn verify_files(
        &self,
        corpus_root: impl AsRef<Path>,
        manifest_path: &RelativePath,
    ) -> Result<()> {
        let corpus_root = corpus_root.as_ref();
        verify_digest(
            &self.manifest_digest,
            &manifest_path.join(corpus_root),
            "manifest",
        )?;
        for artifact in &self.artifacts {
            verify_digest(
                artifact.provenance.source_digest(),
                &artifact.provenance.source_path().join(corpus_root),
                "source",
            )?;
            verify_digest(
                artifact.output_digest(),
                &artifact.output_path.join(corpus_root),
                "artifact",
            )?;
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for GenerationReport {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawReport {
            manifest_digest: Sha256Digest,
            source_repository: String,
            source_revision: SourceRevision,
            counts: GenerationCounts,
            artifacts: Vec<ReportArtifact>,
        }

        let raw = RawReport::deserialize(deserializer)?;
        Self::new(
            raw.manifest_digest,
            raw.source_repository,
            raw.source_revision,
            raw.counts,
            raw.artifacts,
        )
        .map_err(serde::de::Error::custom)
    }
}

fn verify_digest(expected: &Sha256Digest, path: &Path, label: &str) -> Result<()> {
    let actual = Sha256Digest::from_file(path)?;
    if &actual != expected {
        return Err(GeneratorError::new(
            GeneratorErrorKind::Verification,
            "verify generation report",
            format!(
                "{label} digest mismatch for {}: expected {expected}, got {actual}",
                path.display()
            ),
        ));
    }
    Ok(())
}

fn invalid_report(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidInventory,
        "validate generation report",
        detail,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{ArtifactProvenance, GenerationCounts, GenerationReport, ReportArtifact};
    use crate::{GeneratorErrorKind, ManifestVersion, RelativePath, Sha256Digest, SourceRevision};

    #[test]
    fn report_counts_and_provenance_detect_drift() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "surgeist-generator-report-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).expect("create corpus");
        fs::write(root.join("manifest.toml"), b"schema_version = 1\n").expect("manifest");
        fs::write(root.join("source.json"), b"{}\n").expect("source");
        fs::write(root.join("output.json"), b"{\"cases\":[]}\n").expect("output");

        let provenance = ArtifactProvenance::new(
            RelativePath::new("source.json").expect("source path"),
            Sha256Digest::from_file(root.join("source.json")).expect("source digest"),
            "surgeist-test",
            ManifestVersion::new(1).expect("version"),
            BTreeMap::new(),
        )
        .expect("provenance");
        let artifact = ReportArtifact::new(
            provenance,
            RelativePath::new("output.json").expect("output path"),
            Sha256Digest::from_file(root.join("output.json")).expect("output digest"),
            1,
        );
        assert!(
            GenerationReport::new(
                Sha256Digest::from_file(root.join("manifest.toml")).expect("manifest digest"),
                "https://example.invalid/source.git",
                revision(),
                GenerationCounts::new(2, 0, 0, 0, 0),
                vec![artifact.clone()],
            )
            .is_err()
        );
        let report = GenerationReport::new(
            Sha256Digest::from_file(root.join("manifest.toml")).expect("manifest digest"),
            "https://example.invalid/source.git",
            revision(),
            GenerationCounts::new(1, 0, 0, 0, 0),
            vec![artifact],
        )
        .expect("valid report");
        let manifest = RelativePath::new("manifest.toml").expect("manifest path");
        report
            .verify_files(&root, &manifest)
            .expect("matching report");

        fs::write(root.join("output.json"), b"drift\n").expect("drift output");
        let error = report
            .verify_files(&root, &manifest)
            .expect_err("output drift must fail");
        assert_eq!(error.kind(), GeneratorErrorKind::Verification);
        fs::remove_dir_all(root).expect("remove corpus");
    }

    fn revision() -> SourceRevision {
        SourceRevision::new("0123456789abcdef0123456789abcdef01234567").expect("full revision")
    }
}
