use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{MapAccess, Visitor},
};

use crate::{
    GeneratorError, GeneratorErrorKind, ManifestVersion, RelativePath, Result, Sha256Digest,
    SourceRevision,
};

use super::{validate_identifier, validate_repository_url};

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
        if !validate_identifier(&generator) {
            return Err(invalid_report(
                "generator name is not a canonical identifier",
            ));
        }
        if domain_provenance
            .keys()
            .any(|key| !validate_identifier(key))
        {
            return Err(invalid_report(
                "domain provenance key is not a canonical identifier",
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
            domain_provenance: CheckedProvenanceMap,
        }

        let raw = RawProvenance::deserialize(deserializer)?;
        Self::new(
            raw.source_path,
            raw.source_digest,
            raw.generator,
            raw.schema_version,
            raw.domain_provenance.0,
        )
        .map_err(|error| serde::de::Error::custom(error.serde_message()))
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
    pub fn new(
        provenance: ArtifactProvenance,
        output_path: RelativePath,
        output_digest: Sha256Digest,
        case_count: usize,
    ) -> Result<Self> {
        if case_count == 0 || case_count > u32::MAX as usize {
            return Err(invalid_count(
                "report artifact case_count must be 1 through u32::MAX",
            ));
        }
        Ok(Self {
            provenance,
            output_path,
            output_digest,
            case_count,
        })
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
            case_count: u64,
        }

        let raw = RawArtifact::deserialize(deserializer)?;
        let case_count = usize::try_from(raw.case_count)
            .map_err(|_| serde::de::Error::custom("InvalidInventory: case_count overflow"))?;
        Self::new(
            raw.provenance,
            raw.output_path,
            raw.output_digest,
            case_count,
        )
        .map_err(|error| serde::de::Error::custom(error.serde_message()))
    }
}

/// Exact disposition and failure counts for a full generation report.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct GenerationCounts {
    active: usize,
    expected_fail: usize,
    unsupported: usize,
    quarantined: usize,
    failed_to_generate: usize,
}

impl GenerationCounts {
    pub fn new(
        active: usize,
        expected_fail: usize,
        unsupported: usize,
        quarantined: usize,
        failed_to_generate: usize,
    ) -> Result<Self> {
        let values = [
            active,
            expected_fail,
            unsupported,
            quarantined,
            failed_to_generate,
        ];
        if values.iter().any(|value| *value > u32::MAX as usize)
            || values
                .iter()
                .try_fold(0_u64, |total, value| total.checked_add(*value as u64))
                .is_none_or(|total| total > u64::from(u32::MAX))
        {
            return Err(invalid_count(
                "generation count exceeds the portable u32 bound",
            ));
        }
        Ok(Self {
            active,
            expected_fail,
            unsupported,
            quarantined,
            failed_to_generate,
        })
    }

    pub fn total(self) -> Result<usize> {
        let total = [
            self.active,
            self.expected_fail,
            self.unsupported,
            self.quarantined,
            self.failed_to_generate,
        ]
        .into_iter()
        .try_fold(0_u64, |total, value| total.checked_add(value as u64))
        .ok_or_else(|| invalid_count("generation count overflow"))?;
        if total > u64::from(u32::MAX) {
            return Err(invalid_count(
                "generation count exceeds the portable u32 bound",
            ));
        }
        usize::try_from(total).map_err(|_| invalid_count("generation count is not representable"))
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
        if !validate_repository_url(&source_repository) {
            return Err(GeneratorError::new(
                GeneratorErrorKind::SourceVerification,
                "validate generation report source",
                "source repository URL is not canonical HTTPS Git",
            ));
        }
        counts.total()?;
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
        let manifest_path = manifest_path.resolve_existing(corpus_root)?;
        verify_digest(&self.manifest_digest, &manifest_path, "manifest")?;
        for artifact in &self.artifacts {
            let source_path = artifact
                .provenance
                .source_path()
                .resolve_existing(corpus_root)?;
            verify_digest(artifact.provenance.source_digest(), &source_path, "source")?;
            let output_path = artifact.output_path.resolve_existing(corpus_root)?;
            verify_digest(artifact.output_digest(), &output_path, "artifact")?;
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
        .map_err(|error| serde::de::Error::custom(error.serde_message()))
    }
}

impl<'de> Deserialize<'de> for GenerationCounts {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawCounts {
            active: u64,
            expected_fail: u64,
            unsupported: u64,
            quarantined: u64,
            failed_to_generate: u64,
        }

        let raw = RawCounts::deserialize(deserializer)?;
        let convert = |value: u64| {
            usize::try_from(value)
                .map_err(|_| serde::de::Error::custom("InvalidInventory: count overflow"))
        };
        Self::new(
            convert(raw.active)?,
            convert(raw.expected_fail)?,
            convert(raw.unsupported)?,
            convert(raw.quarantined)?,
            convert(raw.failed_to_generate)?,
        )
        .map_err(|error| serde::de::Error::custom(error.serde_message()))
    }
}

struct CheckedProvenanceMap(BTreeMap<String, Sha256Digest>);

impl<'de> Deserialize<'de> for CheckedProvenanceMap {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ProvenanceMapVisitor;

        impl<'de> Visitor<'de> for ProvenanceMapVisitor {
            type Value = CheckedProvenanceMap;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a domain-provenance object with unique identifier keys")
            }

            fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = BTreeMap::new();
                while let Some((key, value)) = map.next_entry::<String, Sha256Digest>()? {
                    if !validate_identifier(&key) {
                        return Err(serde::de::Error::custom(
                            "Verification: invalid domain-provenance identifier",
                        ));
                    }
                    if values.insert(key, value).is_some() {
                        return Err(serde::de::Error::custom(
                            "Verification: duplicate domain-provenance key",
                        ));
                    }
                }
                Ok(CheckedProvenanceMap(values))
            }
        }

        deserializer.deserialize_map(ProvenanceMapVisitor)
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
        GeneratorErrorKind::Verification,
        "validate generation report",
        detail,
    )
}

fn invalid_count(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidInventory,
        "validate generation counts",
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

    #[cfg(unix)]
    #[test]
    fn report_verification_rejects_symlink_escapes() {
        use std::os::unix::fs::symlink;

        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let parent = std::env::temp_dir().join(format!(
            "surgeist-generator-report-escape-{}-{nonce}",
            std::process::id()
        ));
        let root = parent.join("corpus");
        let outside = parent.join("outside");
        fs::create_dir_all(&root).expect("create corpus");
        fs::create_dir(&outside).expect("create outside directory");

        for directory in [&root, &outside] {
            fs::write(directory.join("manifest.toml"), b"schema_version = 1\n")
                .expect("write manifest");
            fs::write(directory.join("source.json"), b"{}\n").expect("write source");
            fs::write(directory.join("output.json"), b"{\"cases\":[]}\n").expect("write output");
        }
        symlink(&outside, root.join("escape")).expect("create escaped report path");

        let make_report = |source_path: &str, output_path: &str| {
            let provenance = ArtifactProvenance::new(
                RelativePath::new(source_path).expect("source path"),
                Sha256Digest::from_file(if source_path.starts_with("escape/") {
                    outside.join("source.json")
                } else {
                    root.join("source.json")
                })
                .expect("source digest"),
                "surgeist-test",
                ManifestVersion::new(1).expect("version"),
                BTreeMap::new(),
            )
            .expect("provenance");
            GenerationReport::new(
                Sha256Digest::from_file(root.join("manifest.toml")).expect("manifest digest"),
                "https://example.invalid/source.git",
                revision(),
                GenerationCounts::new(1, 0, 0, 0, 0).expect("counts"),
                vec![
                    ReportArtifact::new(
                        provenance,
                        RelativePath::new(output_path).expect("output path"),
                        Sha256Digest::from_file(if output_path.starts_with("escape/") {
                            outside.join("output.json")
                        } else {
                            root.join("output.json")
                        })
                        .expect("output digest"),
                        1,
                    )
                    .expect("artifact"),
                ],
            )
            .expect("valid report")
        };

        let safe_manifest = RelativePath::new("manifest.toml").expect("manifest path");
        let escaped_manifest = RelativePath::new("escape/manifest.toml").expect("escaped path");
        let error = make_report("source.json", "output.json")
            .verify_files(&root, &escaped_manifest)
            .expect_err("escaped manifest must fail");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);

        let error = make_report("escape/source.json", "output.json")
            .verify_files(&root, &safe_manifest)
            .expect_err("escaped source must fail");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);

        let error = make_report("source.json", "escape/output.json")
            .verify_files(&root, &safe_manifest)
            .expect_err("escaped output must fail");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);

        fs::remove_dir_all(parent).expect("remove test directories");
    }

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
        )
        .expect("artifact");
        let structurally_distinct_counts = GenerationReport::new(
            Sha256Digest::from_file(root.join("manifest.toml")).expect("manifest digest"),
            "https://example.invalid/source.git",
            revision(),
            GenerationCounts::new(2, 0, 0, 0, 0).expect("counts"),
            vec![artifact.clone()],
        )
        .expect("artifact cases need not equal structural report counts");
        assert_eq!(structurally_distinct_counts.counts().total().unwrap(), 2);
        let report = GenerationReport::new(
            Sha256Digest::from_file(root.join("manifest.toml")).expect("manifest digest"),
            "https://example.invalid/source.git",
            revision(),
            GenerationCounts::new(1, 0, 0, 0, 0).expect("counts"),
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
