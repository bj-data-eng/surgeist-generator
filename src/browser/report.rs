use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::model::{BrowserCorpus, invalid, selected};
use crate::{GeneratorError, RelativePath, Result, Sha256Digest};

/// Deterministic counts in a full or scoped browser generation report.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserReportSummary {
    generated: usize,
    unsupported: usize,
    expected_fail: usize,
    quarantined: usize,
    failed_to_generate: usize,
}
impl BrowserReportSummary {
    pub fn new(
        generated: usize,
        unsupported: usize,
        expected_fail: usize,
        quarantined: usize,
        failed_to_generate: usize,
    ) -> Result<Self> {
        if [
            generated,
            unsupported,
            expected_fail,
            quarantined,
            failed_to_generate,
        ]
        .into_iter()
        .try_fold(0_usize, usize::checked_add)
        .is_none()
        {
            return Err(invalid(
                "declare browser report counts",
                "aggregate count overflows usize",
            ));
        }
        Ok(Self {
            generated,
            unsupported,
            expected_fail,
            quarantined,
            failed_to_generate,
        })
    }
    #[must_use]
    pub const fn generated(&self) -> usize {
        self.generated
    }
    #[must_use]
    pub const fn unsupported(&self) -> usize {
        self.unsupported
    }
    #[must_use]
    pub const fn expected_fail(&self) -> usize {
        self.expected_fail
    }
    #[must_use]
    pub const fn quarantined(&self) -> usize {
        self.quarantined
    }
    #[must_use]
    pub const fn failed_to_generate(&self) -> usize {
        self.failed_to_generate
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Metadata {
    pub(super) schema_version: u8,
    pub(super) generator: String,
    pub(super) engine_version: String,
    pub(super) host_executable_sha256: Sha256Digest,
    pub(super) browser_executable: RelativePath,
    pub(super) browser_executable_sha256: Sha256Digest,
    pub(super) browser_provenance: String,
    pub(super) contract_sha256: Sha256Digest,
    pub(super) inputs: BTreeMap<RelativePath, Sha256Digest>,
    pub(super) source_imports: Vec<super::source_import::SourceImportProvenance>,
}

/// A generated artifact and its complete captured local input lineage.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BrowserGeneratedArtifact {
    pub(super) name: String,
    pub(super) source: RelativePath,
    pub(super) output: RelativePath,
    pub(super) variant: String,
    pub(super) source_sha256: Sha256Digest,
    pub(super) linked_resources: BTreeMap<RelativePath, Sha256Digest>,
    pub(super) job_sha256: Sha256Digest,
    pub(super) output_sha256: Sha256Digest,
}
impl BrowserGeneratedArtifact {
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
    #[must_use]
    pub const fn source(&self) -> &RelativePath {
        &self.source
    }
    #[must_use]
    pub const fn output(&self) -> &RelativePath {
        &self.output
    }
    #[must_use]
    pub fn variant(&self) -> &str {
        &self.variant
    }
    #[must_use]
    pub const fn output_sha256(&self) -> &Sha256Digest {
        &self.output_sha256
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Disposition {
    pub(super) name: String,
    pub(super) source: RelativePath,
    pub(super) variant: Option<String>,
    pub(super) reason: String,
}

/// Version-four browser report. This schema is independent of the CSS report.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct BrowserGenerationReport {
    pub(super) metadata: Metadata,
    pub(super) filter: Option<RelativePath>,
    summary: BrowserReportSummary,
    pub(super) generated: Vec<BrowserGeneratedArtifact>,
    pub(super) unsupported: Vec<Disposition>,
    pub(super) expected_fail: Vec<Disposition>,
    pub(super) quarantined: Vec<Disposition>,
    pub(super) failed_to_generate: Vec<Disposition>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawReport {
    metadata: Metadata,
    filter: Option<RelativePath>,
    summary: BrowserReportSummary,
    generated: Vec<BrowserGeneratedArtifact>,
    unsupported: Vec<Disposition>,
    expected_fail: Vec<Disposition>,
    quarantined: Vec<Disposition>,
    failed_to_generate: Vec<Disposition>,
}

impl BrowserGenerationReport {
    pub(super) fn new(metadata: Metadata, filter: Option<RelativePath>) -> Self {
        Self {
            metadata,
            filter,
            summary: BrowserReportSummary::default(),
            generated: Vec::new(),
            unsupported: Vec::new(),
            expected_fail: Vec::new(),
            quarantined: Vec::new(),
            failed_to_generate: Vec::new(),
        }
    }
    #[must_use]
    pub const fn summary(&self) -> &BrowserReportSummary {
        &self.summary
    }
    #[must_use]
    pub fn generated(&self) -> &[BrowserGeneratedArtifact] {
        &self.generated
    }
    #[must_use]
    pub const fn filter(&self) -> Option<&RelativePath> {
        self.filter.as_ref()
    }
    /// Returns canonical two-space JSON with exactly one terminating newline.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        self.validate()?;
        let mut bytes = serde_json::to_vec_pretty(self).map_err(serialization)?;
        bytes.push(b'\n');
        Ok(bytes)
    }
    pub(super) fn parse(bytes: &[u8]) -> Result<Self> {
        let raw: RawReport = serde_json::from_slice(bytes).map_err(serialization)?;
        let report = Self {
            metadata: raw.metadata,
            filter: raw.filter,
            summary: raw.summary,
            generated: raw.generated,
            unsupported: raw.unsupported,
            expected_fail: raw.expected_fail,
            quarantined: raw.quarantined,
            failed_to_generate: raw.failed_to_generate,
        };
        report.validate()?;
        if report.to_bytes()? != bytes {
            return Err(invalid(
                "read browser generation report",
                "report is not canonical version-four JSON",
            ));
        }
        Ok(report)
    }
    pub(super) fn canonicalize(&mut self) {
        self.generated
            .sort_by(|a, b| (&a.source, &a.name, &a.output).cmp(&(&b.source, &b.name, &b.output)));
        for bucket in [
            &mut self.unsupported,
            &mut self.expected_fail,
            &mut self.quarantined,
            &mut self.failed_to_generate,
        ] {
            bucket.sort_by(|a, b| {
                (&a.source, &a.name, &a.variant).cmp(&(&b.source, &b.name, &b.variant))
            });
        }
        self.summary = self.counts();
    }
    fn counts(&self) -> BrowserReportSummary {
        BrowserReportSummary {
            generated: self.generated.len(),
            unsupported: self.unsupported.len(),
            expected_fail: self.expected_fail.len(),
            quarantined: self.quarantined.len(),
            failed_to_generate: self.failed_to_generate.len(),
        }
    }
    pub(super) fn scoped(&self, filter: &RelativePath) -> Self {
        let mut report = self.clone();
        report.filter = Some(filter.clone());
        report
            .generated
            .retain(|entry| selected(&entry.source, filter));
        for bucket in [
            &mut report.unsupported,
            &mut report.expected_fail,
            &mut report.quarantined,
            &mut report.failed_to_generate,
        ] {
            bucket.retain(|entry| selected(&entry.source, filter));
        }
        report.canonicalize();
        report
    }
    pub(super) fn validate(&self) -> Result<()> {
        if self.metadata.schema_version != 4
            || self.summary != self.counts()
            || !crate::core::validate_identifier(&self.metadata.generator)
        {
            return Err(invalid(
                "validate browser generation report",
                "schema, generator, or accounting is invalid",
            ));
        }
        let mut paths = BTreeSet::new();
        let mut cases = BTreeSet::new();
        for entry in &self.generated {
            if !paths.insert(&entry.output) || !cases.insert((&entry.source, &entry.name)) {
                return Err(invalid(
                    "validate browser generation report",
                    "duplicate generated path or source/case relation",
                ));
            }
        }
        for entry in &self.unsupported {
            if entry.variant.is_some() && !cases.insert((&entry.source, &entry.name)) {
                return Err(invalid(
                    "validate browser generation report",
                    "duplicate runtime case outcome",
                ));
            }
        }
        for bucket in [
            &self.unsupported,
            &self.expected_fail,
            &self.quarantined,
            &self.failed_to_generate,
        ] {
            let mut identities = BTreeSet::new();
            for entry in bucket {
                if entry.name.trim().is_empty()
                    || entry.reason.trim().is_empty()
                    || entry.reason.trim() != entry.reason
                    || !identities.insert((&entry.source, &entry.name, &entry.variant))
                {
                    return Err(invalid(
                        "validate browser generation report",
                        "invalid or duplicate disposition",
                    ));
                }
            }
        }
        let mut sorted = self.clone();
        sorted.canonicalize();
        if sorted != *self {
            return Err(invalid(
                "validate browser generation report",
                "report entries must be sorted",
            ));
        }
        Ok(())
    }
}

pub(super) fn contract_digest(corpus: &BrowserCorpus) -> Result<Sha256Digest> {
    serde_json::to_vec(&(
        &corpus.generator,
        &corpus.manifest_path,
        &corpus.output_root,
        &corpus.report_paths,
        &corpus.browser,
        &corpus.fixtures,
        &corpus.global_inputs,
        &corpus.import_attestations,
        &corpus.expected_counts,
        &corpus.input_contract,
    ))
    .map(Sha256Digest::from_bytes)
    .map_err(serialization)
}
fn serialization(error: serde_json::Error) -> GeneratorError {
    invalid("serialize browser generation report", error.to_string())
}
