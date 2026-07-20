use std::collections::{BTreeMap, BTreeSet};

use serde::de::{Deserialize, Deserializer, Error as _, MapAccess, Visitor};

use crate::core::{CORPUS_FILE_MODE, Inventory, InventoryEntry, NodeKind, RootedFs};
use crate::{
    GeneratorError, GeneratorErrorKind, RelativePath, Result, Sha256Digest, SourceRevision,
};

use super::case::LayoutCaseStatus;
use super::manifest::LayoutManifest;

const GENERATOR: &str = "surgeist-layout-generate";
const REPORT_DIRECTORY: &str = "generation-reports";
const FULL_REPORT: &str = "generation-reports/all.json";
const HTML_PREFIX: &str = "html/";
const VARIANTS: [&str; 4] = [
    "border_box_ltr",
    "border_box_rtl",
    "content_box_ltr",
    "content_box_rtl",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum DesiredDisposition {
    Active,
    ExpectedFail { name: String, reason: String },
    Unsupported { name: String, reason: String },
    Quarantined { name: String, reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DesiredSource {
    pub(super) digest: Sha256Digest,
    pub(super) uses_base_style: bool,
    pub(super) disposition: DesiredDisposition,
}

#[derive(Debug)]
pub(super) struct CurrentCorpus<'a> {
    pub(super) manifest: &'a LayoutManifest,
    pub(super) manifest_digest: Sha256Digest,
    pub(super) helper_digest: Sha256Digest,
    pub(super) base_style_digest: Sha256Digest,
    pub(super) sidecar_digest: Sha256Digest,
    pub(super) sources: BTreeMap<RelativePath, DesiredSource>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum CheckState {
    Current,
    Stale,
}

pub(super) fn validate(
    rooted: &RootedFs,
    inventory: Option<&Inventory>,
    current: &CurrentCorpus<'_>,
) -> Result<CheckState> {
    let Some(inventory) = inventory else {
        return Ok(CheckState::Stale);
    };
    if inventory.entries().is_empty() {
        return Ok(CheckState::Stale);
    }

    let report_entries = report_entries(inventory)?;
    if !report_entries.contains_key(FULL_REPORT) {
        return Err(invalid_inventory(
            "nonempty layout XML root has no generation-reports/all.json authority",
        ));
    }
    let mut reports = BTreeMap::new();
    for (path, entry) in &report_entries {
        if entry.identity().kind() != NodeKind::Regular {
            return Err(invalid_inventory(format!(
                "layout report is not a regular file: {path}"
            )));
        }
        let bytes = rooted
            .read_file(&format!("xml/{path}"), CORPUS_FILE_MODE)
            .map_err(report_io)?;
        reports.insert(path.clone(), LayoutReport::parse(&bytes)?);
    }
    let full = reports
        .get(FULL_REPORT)
        .expect("full report entry was parsed");
    if full.filter.is_some() {
        return Err(invalid_inventory("full layout report filter must be null"));
    }

    let mode = validate_report_set(full, &reports)?;
    parse_browser_provenance(&full.metadata, current.manifest)?;
    let historical_outputs = full
        .generated
        .iter()
        .map(|entry| entry.output.clone())
        .collect::<BTreeSet<_>>();
    let desired_outputs = desired_outputs(&current.sources)?;
    validate_visible_inventory(
        inventory,
        reports.keys(),
        &historical_outputs,
        &desired_outputs,
    )?;

    let mut stale = mode == ReportMode::Legacy;
    let diagnostic = !full.failed_to_generate.is_empty();
    stale |= !report_paths_are_current(reports.keys(), current.manifest);
    stale |= !full_ledger_is_current(full, current)?;
    stale |= !metadata_is_current(&full.metadata, current);
    stale |= !report_expectations_are_current(full, &reports, current.manifest, diagnostic)?;

    let visible_outputs = visible_output_entries(inventory);
    let generated_by_output = full
        .generated
        .iter()
        .map(|entry| (entry.output.clone(), entry))
        .collect::<BTreeMap<_, _>>();
    for (output, entry) in &visible_outputs {
        let bytes = rooted
            .read_file(output.as_str(), CORPUS_FILE_MODE)
            .map_err(report_io)?;
        let attestation = XmlAttestation::parse(&bytes)?;
        let expected_mapping = generated_by_output
            .get(output)
            .map(|generated| (&generated.source, generated.variant.as_str()))
            .or_else(|| desired_mapping(output, &current.sources));
        let Some((expected_source, expected_variant)) = expected_mapping else {
            return Err(invalid_inventory(format!(
                "XML output has no historical or desired mapping: {}",
                output.as_str()
            )));
        };
        attestation.validate_mapping(expected_source, output, expected_variant)?;
        attestation.validate_browser(&full.metadata)?;

        if let Some(generated) = generated_by_output.get(output) {
            stale |= !attestation.matches_report_metadata(&full.metadata);
            match &generated.output_sha256 {
                Some(expected) => stale |= expected != &Sha256Digest::from_bytes(&bytes),
                None => stale = true,
            }
        } else {
            stale = true;
        }

        if let Some(source) = current.sources.get(expected_source) {
            attestation.validate_current_optional_fields(source)?;
            stale |= !attestation.matches_current_source(expected_source, source, current);
        }

        if entry.identity().kind() != NodeKind::Regular {
            return Err(invalid_inventory(format!(
                "XML output is not a regular file: {}",
                output.as_str()
            )));
        }
    }

    let visible_set = visible_outputs.keys().cloned().collect::<BTreeSet<_>>();
    stale |= visible_set != historical_outputs;
    stale |= diagnostic;
    Ok(if stale {
        CheckState::Stale
    } else {
        CheckState::Current
    })
}

fn report_entries(inventory: &Inventory) -> Result<BTreeMap<String, &InventoryEntry>> {
    let mut reports = BTreeMap::new();
    for entry in inventory.entries() {
        let path = entry.path().as_str();
        if path == REPORT_DIRECTORY {
            if entry.identity().kind() != NodeKind::Directory {
                return Err(invalid_inventory(
                    "layout generation-reports authority is not a directory",
                ));
            }
            continue;
        }
        let Some(file) = path.strip_prefix("generation-reports/") else {
            continue;
        };
        if file.contains('/')
            || file.is_empty()
            || !file.ends_with(".json")
            || file.len() == ".json".len()
        {
            return Err(invalid_inventory(format!(
                "historical layout report path is noncanonical: {path}"
            )));
        }
        reports.insert(path.to_owned(), entry);
    }
    Ok(reports)
}

fn validate_visible_inventory<'a>(
    inventory: &Inventory,
    report_paths: impl Iterator<Item = &'a String>,
    historical_outputs: &BTreeSet<RelativePath>,
    desired_outputs: &BTreeSet<RelativePath>,
) -> Result<()> {
    let mut admitted = historical_outputs
        .union(desired_outputs)
        .cloned()
        .collect::<BTreeSet<_>>();
    admitted.extend(report_paths.map(|path| {
        RelativePath::new(format!("xml/{path}"))
            .expect("validated report path remains strict with xml prefix")
    }));
    for entry in inventory.entries() {
        let full_path = RelativePath::new(format!("xml/{}", entry.path().as_str()))?;
        let admitted_entry = match entry.identity().kind() {
            NodeKind::Regular => admitted.contains(&full_path),
            NodeKind::Directory => {
                let prefix = format!("{}/", full_path.as_str());
                admitted
                    .iter()
                    .any(|path| path.as_str().starts_with(&prefix))
            }
            NodeKind::Symlink => false,
        };
        if !admitted_entry {
            return Err(invalid_inventory(format!(
                "unknown entry in layout XML root: {}",
                entry.path().as_str()
            )));
        }
    }
    Ok(())
}

fn visible_output_entries(inventory: &Inventory) -> BTreeMap<RelativePath, &InventoryEntry> {
    inventory
        .entries()
        .iter()
        .filter(|entry| {
            entry.identity().kind() == NodeKind::Regular
                && !entry.path().as_str().starts_with("generation-reports/")
        })
        .map(|entry| {
            (
                RelativePath::new(format!("xml/{}", entry.path().as_str()))
                    .expect("inventory path remains strict with xml prefix"),
                entry,
            )
        })
        .collect()
}

fn desired_outputs(
    sources: &BTreeMap<RelativePath, DesiredSource>,
) -> Result<BTreeSet<RelativePath>> {
    let mut outputs = BTreeSet::new();
    for (source, record) in sources {
        if matches!(
            record.disposition,
            DesiredDisposition::Unsupported { .. } | DesiredDisposition::Quarantined { .. }
        ) {
            continue;
        }
        for variant in VARIANTS {
            outputs.insert(output_for(source, variant)?);
        }
    }
    Ok(outputs)
}

fn desired_mapping<'a>(
    output: &RelativePath,
    sources: &'a BTreeMap<RelativePath, DesiredSource>,
) -> Option<(&'a RelativePath, &'static str)> {
    sources.iter().find_map(|(source, record)| {
        if matches!(
            record.disposition,
            DesiredDisposition::Unsupported { .. } | DesiredDisposition::Quarantined { .. }
        ) {
            return None;
        }
        VARIANTS.into_iter().find_map(|variant| {
            (output_for(source, variant).ok().as_ref() == Some(output)).then_some((source, variant))
        })
    })
}

fn report_paths_are_current<'a>(
    paths: impl Iterator<Item = &'a String>,
    manifest: &LayoutManifest,
) -> bool {
    let actual = paths.cloned().collect::<BTreeSet<_>>();
    let mut expected = BTreeSet::from([FULL_REPORT.to_owned()]);
    expected.extend(
        manifest
            .reports
            .scoped
            .iter()
            .map(|report| format!("generation-reports/{}", report.file.as_str())),
    );
    actual == expected
}

fn metadata_is_current(metadata: &ReportMetadata, current: &CurrentCorpus<'_>) -> bool {
    metadata.browser_source == current.manifest.browser.source
        && metadata.browser_version == current.manifest.browser.version
        && metadata.launch_profile_sha256 == current.manifest.launch_digest
        && metadata.helper_sha256 == current.helper_digest
        && metadata.base_style_sha256 == current.base_style_digest
        && metadata.corpus_manifest_sha256 == current.manifest_digest
        && metadata.taffy_revision == current.manifest.revision
        && metadata.taffy_sidecar_sha256 == current.sidecar_digest
}

fn report_expectations_are_current(
    full: &LayoutReport,
    reports: &BTreeMap<String, LayoutReport>,
    manifest: &LayoutManifest,
    diagnostic: bool,
) -> Result<bool> {
    let expected = manifest.reports.full;
    let mut current = if diagnostic {
        full.summary.expected_fail == expected.expected_fail
            && full.summary.quarantined == expected.quarantined
    } else {
        full.summary.generated == expected.generated
            && full.summary.unsupported == expected.unsupported
            && full.summary.expected_fail == expected.expected_fail
            && full.summary.quarantined == expected.quarantined
            && full.summary.failed_to_generate == expected.failed_to_generate
    };
    for scoped in &manifest.reports.scoped {
        let path = format!("generation-reports/{}", scoped.file.as_str());
        let Some(report) = reports.get(&path) else {
            current = false;
            continue;
        };
        if report.filter.as_ref() != Some(&scoped.filter) {
            current = false;
            continue;
        }
        let has_failed_fixture = report
            .failed_to_generate
            .iter()
            .any(|entry| source_matches(&entry.source, &scoped.filter));
        if !diagnostic || !has_failed_fixture {
            current &= report.summary.generated == scoped.generated;
        }
    }
    Ok(current)
}

fn full_ledger_is_current(full: &LayoutReport, current: &CurrentCorpus<'_>) -> Result<bool> {
    let actual_sources = full.fixture_sources();
    if actual_sources != current.sources.keys().cloned().collect() {
        return Ok(false);
    }
    for (source, desired) in &current.sources {
        let expected = match &desired.disposition {
            DesiredDisposition::Active => DesiredBucket::Active,
            DesiredDisposition::ExpectedFail { name, reason } => {
                DesiredBucket::ExpectedFail(name, reason)
            }
            DesiredDisposition::Unsupported { name, reason } => {
                DesiredBucket::Unsupported(name, reason)
            }
            DesiredDisposition::Quarantined { name, reason } => {
                DesiredBucket::Quarantined(name, reason)
            }
        };
        if !full.source_matches_desired(source, expected) {
            return Ok(false);
        }
    }
    Ok(true)
}

enum DesiredBucket<'a> {
    Active,
    ExpectedFail(&'a str, &'a str),
    Unsupported(&'a str, &'a str),
    Quarantined(&'a str, &'a str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReportMode {
    Current,
    Legacy,
}

fn validate_report_set(
    full: &LayoutReport,
    reports: &BTreeMap<String, LayoutReport>,
) -> Result<ReportMode> {
    full.validate_shape()?;
    let mut saw_current = full
        .generated
        .iter()
        .any(|entry| entry.output_sha256.is_some());
    let mut saw_legacy = full
        .generated
        .iter()
        .any(|entry| entry.output_sha256.is_none());
    let mut filters = BTreeSet::new();
    for (path, report) in reports {
        report.validate_shape()?;
        saw_current |= report
            .generated
            .iter()
            .any(|entry| entry.output_sha256.is_some());
        saw_legacy |= report
            .generated
            .iter()
            .any(|entry| entry.output_sha256.is_none());
        if path == FULL_REPORT {
            continue;
        }
        let filter = report.filter.as_ref().ok_or_else(|| {
            invalid_inventory(format!(
                "historical scoped report has a null filter: {path}"
            ))
        })?;
        if !filters.insert(filter.clone()) {
            return Err(invalid_inventory(
                "historical scoped report filters must be unique",
            ));
        }
        if report.metadata != full.metadata || !report.is_exact_subset(full, filter) {
            return Err(invalid_inventory(format!(
                "historical scoped report is not an exact full-report subset: {path}"
            )));
        }
    }
    if saw_current && saw_legacy {
        return Err(invalid_inventory(
            "layout report authority mixes current and migration-only generated entries",
        ));
    }
    Ok(if saw_legacy {
        ReportMode::Legacy
    } else {
        ReportMode::Current
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LayoutReport {
    metadata: ReportMetadata,
    filter: Option<RelativePath>,
    summary: ReportSummary,
    generated: Vec<GeneratedEntry>,
    unsupported: Vec<UnsupportedEntry>,
    expected_fail: Vec<DispositionEntry>,
    quarantined: Vec<DispositionEntry>,
    failed_to_generate: Vec<DispositionEntry>,
}

impl LayoutReport {
    fn parse(bytes: &[u8]) -> Result<Self> {
        if !CanonicalJson::validate(bytes) {
            return Err(invalid_inventory(
                "layout report bytes are not canonical two-space JSON with one final LF",
            ));
        }
        serde_json::from_slice(bytes).map_err(|error| {
            GeneratorError::with_source(
                GeneratorErrorKind::InvalidInventory,
                "parse layout generation report",
                "invalid report schema or member order",
                error,
            )
        })
    }

    fn validate_shape(&self) -> Result<()> {
        self.metadata.validate()?;
        if self.summary.generated != self.generated.len()
            || self.summary.unsupported != self.unsupported.len()
            || self.summary.expected_fail != self.expected_fail.len()
            || self.summary.quarantined != self.quarantined.len()
            || self.summary.failed_to_generate != self.failed_to_generate.len()
        {
            return Err(invalid_inventory(
                "layout report summary differs from its bucket lengths",
            ));
        }
        require_strictly_sorted(
            &self.generated,
            |left, right| left.sort_key().cmp(&right.sort_key()),
            "generated",
        )?;
        require_strictly_sorted(
            &self.unsupported,
            |left, right| left.sort_key().cmp(&right.sort_key()),
            "unsupported",
        )?;
        require_strictly_sorted(
            &self.expected_fail,
            |left, right| left.sort_key().cmp(&right.sort_key()),
            "expected_fail",
        )?;
        require_strictly_sorted(
            &self.quarantined,
            |left, right| left.sort_key().cmp(&right.sort_key()),
            "quarantined",
        )?;
        require_strictly_sorted(
            &self.failed_to_generate,
            |left, right| left.sort_key().cmp(&right.sort_key()),
            "failed_to_generate",
        )?;
        self.validate_coverage()
    }

    fn validate_coverage(&self) -> Result<()> {
        let mut states: BTreeMap<RelativePath, FixtureState<'_>> = BTreeMap::new();
        let mut outputs = BTreeSet::new();
        for entry in &self.generated {
            entry.validate_mapping()?;
            if !outputs.insert(entry.output.clone()) {
                return Err(invalid_inventory(
                    "layout report repeats a generated output",
                ));
            }
            if !states
                .entry(entry.source.clone())
                .or_default()
                .variants
                .insert(entry.variant.as_str())
            {
                return Err(invalid_inventory(
                    "layout report repeats a fixture variant outcome",
                ));
            }
        }
        for entry in &self.unsupported {
            entry.validate()?;
            let state = states.entry(entry.source.clone()).or_default();
            if entry.variant == "manifest" {
                if state.manifest_unsupported.replace(entry).is_some() {
                    return Err(invalid_inventory(
                        "layout report repeats a manifest-unsupported fixture",
                    ));
                }
            } else {
                if !state.variants.insert(entry.variant.as_str()) {
                    return Err(invalid_inventory(
                        "layout report repeats a fixture variant outcome",
                    ));
                }
            }
        }
        for entry in &self.expected_fail {
            entry.validate()?;
            if states
                .entry(entry.source.clone())
                .or_default()
                .expected_fail
                .replace(entry)
                .is_some()
            {
                return Err(invalid_inventory(
                    "layout report repeats an expected-fail fixture",
                ));
            }
        }
        for entry in &self.quarantined {
            entry.validate()?;
            if states
                .entry(entry.source.clone())
                .or_default()
                .quarantined
                .replace(entry)
                .is_some()
            {
                return Err(invalid_inventory(
                    "layout report repeats a quarantined fixture",
                ));
            }
        }
        for entry in &self.failed_to_generate {
            entry.validate()?;
            if entry.name != fixture_stem(&entry.source)?
                || states
                    .entry(entry.source.clone())
                    .or_default()
                    .failed
                    .replace(entry)
                    .is_some()
            {
                return Err(invalid_inventory(
                    "layout report has an invalid or repeated failed fixture",
                ));
            }
        }
        for state in states.values() {
            let disposition_only =
                state.manifest_unsupported.is_some() || state.quarantined.is_some();
            if state.manifest_unsupported.is_some() && state.quarantined.is_some()
                || disposition_only
                    && (state.expected_fail.is_some()
                        || state.failed.is_some()
                        || !state.variants.is_empty())
            {
                return Err(invalid_inventory(
                    "layout report disposition-only fixture also has a browser outcome",
                ));
            }
            if disposition_only {
                continue;
            }
            let failed = state.failed.is_some();
            if (failed && !state.variants.is_empty())
                || (!failed && state.variants != VARIANTS.into_iter().collect::<BTreeSet<_>>())
            {
                return Err(invalid_inventory(
                    "layout report fixture must have one failure or all four variant outcomes",
                ));
            }
        }
        Ok(())
    }

    fn fixture_sources(&self) -> BTreeSet<RelativePath> {
        self.generated
            .iter()
            .map(|entry| entry.source.clone())
            .chain(self.unsupported.iter().map(|entry| entry.source.clone()))
            .chain(self.expected_fail.iter().map(|entry| entry.source.clone()))
            .chain(self.quarantined.iter().map(|entry| entry.source.clone()))
            .chain(
                self.failed_to_generate
                    .iter()
                    .map(|entry| entry.source.clone()),
            )
            .collect()
    }

    fn source_matches_desired(&self, source: &RelativePath, desired: DesiredBucket<'_>) -> bool {
        let expected = self
            .expected_fail
            .iter()
            .find(|entry| &entry.source == source);
        let unsupported = self
            .unsupported
            .iter()
            .find(|entry| &entry.source == source && entry.variant == "manifest");
        let quarantined = self
            .quarantined
            .iter()
            .find(|entry| &entry.source == source);
        match desired {
            DesiredBucket::Active => {
                expected.is_none() && unsupported.is_none() && quarantined.is_none()
            }
            DesiredBucket::ExpectedFail(name, reason) => {
                expected.is_some_and(|entry| entry.name == name && entry.reason == reason)
                    && unsupported.is_none()
                    && quarantined.is_none()
            }
            DesiredBucket::Unsupported(name, reason) => {
                expected.is_none()
                    && unsupported.is_some_and(|entry| entry.name == name && entry.reason == reason)
                    && quarantined.is_none()
            }
            DesiredBucket::Quarantined(name, reason) => {
                expected.is_none()
                    && unsupported.is_none()
                    && quarantined.is_some_and(|entry| entry.name == name && entry.reason == reason)
            }
        }
    }

    fn is_exact_subset(&self, full: &Self, filter: &RelativePath) -> bool {
        self.generated
            == full
                .generated
                .iter()
                .filter(|entry| source_matches(&entry.source, filter))
                .cloned()
                .collect::<Vec<_>>()
            && self.unsupported
                == full
                    .unsupported
                    .iter()
                    .filter(|entry| source_matches(&entry.source, filter))
                    .cloned()
                    .collect::<Vec<_>>()
            && self.expected_fail
                == full
                    .expected_fail
                    .iter()
                    .filter(|entry| source_matches(&entry.source, filter))
                    .cloned()
                    .collect::<Vec<_>>()
            && self.quarantined
                == full
                    .quarantined
                    .iter()
                    .filter(|entry| source_matches(&entry.source, filter))
                    .cloned()
                    .collect::<Vec<_>>()
            && self.failed_to_generate
                == full
                    .failed_to_generate
                    .iter()
                    .filter(|entry| source_matches(&entry.source, filter))
                    .cloned()
                    .collect::<Vec<_>>()
    }
}

#[derive(Default)]
struct FixtureState<'a> {
    variants: BTreeSet<&'a str>,
    manifest_unsupported: Option<&'a UnsupportedEntry>,
    expected_fail: Option<&'a DispositionEntry>,
    quarantined: Option<&'a DispositionEntry>,
    failed: Option<&'a DispositionEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ReportMetadata {
    schema_version: u8,
    generator: String,
    browser_source: String,
    browser_version: String,
    browser_provenance: String,
    browser_executable_sha256: Sha256Digest,
    launch_profile_sha256: Sha256Digest,
    helper_sha256: Sha256Digest,
    base_style_sha256: Sha256Digest,
    corpus_manifest_sha256: Sha256Digest,
    taffy_revision: SourceRevision,
    taffy_sidecar_sha256: Sha256Digest,
}

impl ReportMetadata {
    fn validate(&self) -> Result<()> {
        if self.schema_version != 2 || self.generator != GENERATOR {
            return Err(invalid_inventory(
                "layout report metadata schema or generator is noncanonical",
            ));
        }
        if self.browser_source.is_empty()
            || self.browser_version.is_empty()
            || self.browser_provenance.is_empty()
        {
            return Err(invalid_inventory(
                "layout report browser metadata must be nonempty",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReportSummary {
    generated: usize,
    unsupported: usize,
    expected_fail: usize,
    quarantined: usize,
    failed_to_generate: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GeneratedEntry {
    name: String,
    source: RelativePath,
    output: RelativePath,
    output_sha256: Option<Sha256Digest>,
    variant: String,
}

impl GeneratedEntry {
    fn validate_mapping(&self) -> Result<()> {
        validate_source(&self.source)?;
        validate_variant(&self.variant)?;
        let expected = output_for(&self.source, &self.variant)?;
        if self.output != expected || self.name != variant_name(&self.source, &self.variant)? {
            return Err(invalid_inventory(
                "layout generated entry has an impossible source/name/output mapping",
            ));
        }
        Ok(())
    }

    fn sort_key(&self) -> (&RelativePath, &str, &str, &RelativePath, &str, &str) {
        (
            &self.source,
            &self.name,
            &self.variant,
            &self.output,
            self.output_sha256.as_ref().map_or("", Sha256Digest::as_str),
            "",
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct UnsupportedEntry {
    name: String,
    source: RelativePath,
    variant: String,
    reason: String,
}

impl UnsupportedEntry {
    fn validate(&self) -> Result<()> {
        validate_source(&self.source)?;
        if self.variant == "manifest" {
            validate_disposition_entry(&self.name, &self.source)?;
        } else {
            validate_variant(&self.variant)?;
            if self.name != variant_name(&self.source, &self.variant)? {
                return Err(invalid_inventory(
                    "layout unsupported entry has an impossible variant name",
                ));
            }
        }
        Ok(())
    }

    fn sort_key(&self) -> (&RelativePath, &str, &str, &str, &str, &str) {
        (
            &self.source,
            &self.name,
            &self.variant,
            "",
            "",
            &self.reason,
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DispositionEntry {
    name: String,
    source: RelativePath,
    reason: String,
}

impl DispositionEntry {
    fn validate(&self) -> Result<()> {
        validate_disposition_entry(&self.name, &self.source)
    }

    fn sort_key(&self) -> (&RelativePath, &str, &str, &str, &str, &str) {
        (&self.source, &self.name, "", "", "", &self.reason)
    }
}

fn validate_disposition_entry(name: &str, source: &RelativePath) -> Result<()> {
    validate_source(source)?;
    if name.is_empty() || name.trim() != name || name.chars().any(char::is_control) {
        return Err(invalid_inventory(
            "layout disposition name must be nonempty trimmed text",
        ));
    }
    Ok(())
}

fn validate_source(source: &RelativePath) -> Result<()> {
    let Some(relative) = source.as_str().strip_prefix(HTML_PREFIX) else {
        return Err(invalid_inventory(
            "layout report source must be beneath html",
        ));
    };
    RelativePath::with_extension(relative, "html")
        .map_err(|_| invalid_inventory("layout report source must be a strict .html path"))?;
    Ok(())
}

fn validate_variant(variant: &str) -> Result<()> {
    if !VARIANTS.contains(&variant) {
        return Err(invalid_inventory("layout report variant is noncanonical"));
    }
    Ok(())
}

fn output_for(source: &RelativePath, variant: &str) -> Result<RelativePath> {
    validate_source(source)?;
    validate_variant(variant)?;
    let relative = source
        .as_str()
        .strip_prefix(HTML_PREFIX)
        .expect("validated HTML prefix");
    let stemmed = relative
        .strip_suffix(".html")
        .expect("validated HTML extension");
    RelativePath::new(format!("xml/{stemmed}__{variant}.xml"))
}

fn fixture_stem(source: &RelativePath) -> Result<String> {
    validate_source(source)?;
    let relative = source
        .as_str()
        .strip_prefix(HTML_PREFIX)
        .expect("validated HTML prefix");
    let file = relative
        .rsplit('/')
        .next()
        .expect("strict path has a component");
    Ok(file
        .strip_suffix(".html")
        .expect("validated HTML extension")
        .to_owned())
}

fn variant_name(source: &RelativePath, variant: &str) -> Result<String> {
    Ok(format!("{}__{variant}", fixture_stem(source)?))
}

fn source_matches(source: &RelativePath, filter: &RelativePath) -> bool {
    let Some(relative) = source.as_str().strip_prefix(HTML_PREFIX) else {
        return false;
    };
    if filter.as_str().ends_with(".html") {
        return relative == filter.as_str();
    }
    relative == filter.as_str()
        || relative
            .strip_prefix(filter.as_str())
            .is_some_and(|suffix| suffix.starts_with('/'))
}

fn require_strictly_sorted<T>(
    values: &[T],
    compare: impl Fn(&T, &T) -> std::cmp::Ordering,
    bucket: &str,
) -> Result<()> {
    if values
        .windows(2)
        .any(|window| compare(&window[0], &window[1]) != std::cmp::Ordering::Less)
    {
        return Err(invalid_inventory(format!(
            "layout report {bucket} bucket is not strictly sorted"
        )));
    }
    Ok(())
}

#[derive(Debug)]
struct XmlAttestation {
    source: RelativePath,
    source_sha256: Sha256Digest,
    linked_resource_sha256: Option<String>,
    helper_sha256: Sha256Digest,
    base_style_sha256: Option<Sha256Digest>,
    browser: String,
    browser_executable_sha256: Sha256Digest,
    launch_profile_sha256: Sha256Digest,
    corpus_manifest_sha256: Sha256Digest,
    taffy_revision: SourceRevision,
    taffy_sidecar_sha256: Sha256Digest,
}

impl XmlAttestation {
    fn parse(bytes: &[u8]) -> Result<Self> {
        if !bytes.ends_with(b"\n") || bytes.ends_with(b"\n\n") {
            return Err(invalid_inventory("layout XML must have one final LF"));
        }
        let line_end = bytes
            .iter()
            .position(|byte| *byte == b'\n')
            .ok_or_else(|| invalid_inventory("layout XML has no generated-by first line"))?;
        if line_end + 1 == bytes.len() || bytes[line_end + 1] != b'<' {
            return Err(invalid_inventory("layout XML body is absent or malformed"));
        }
        std::str::from_utf8(&bytes[line_end + 1..])
            .map_err(|_| invalid_inventory("layout XML body is not UTF-8"))?;
        let line = std::str::from_utf8(&bytes[..line_end])
            .map_err(|_| invalid_inventory("layout XML first line is not UTF-8"))?;
        let prefix = "<!-- generated-by: surgeist-layout-generate ";
        let suffix = " -->";
        let fields = line
            .strip_prefix(prefix)
            .and_then(|line| line.strip_suffix(suffix))
            .ok_or_else(|| {
                invalid_inventory("layout XML first line is not the canonical comment")
            })?;
        let attributes = parse_comment_attributes(fields)?;
        validate_comment_order(&attributes)?;
        let value = |name: &str| {
            attributes
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
                .ok_or_else(|| invalid_inventory(format!("layout XML comment omits {name}")))
        };
        if value("schema")? != "2" {
            return Err(invalid_inventory("layout XML comment schema must be 2"));
        }
        let linked_resource_sha256 = attributes
            .iter()
            .find(|(key, _)| key == "linked-resource-sha256")
            .map(|(_, value)| {
                validate_linked_resources(value)?;
                Ok(value.clone())
            })
            .transpose()?;
        Ok(Self {
            source: RelativePath::new(value("source")?).map_err(|_| {
                invalid_inventory("layout XML comment source is not a strict relative path")
            })?,
            source_sha256: digest(value("source-sha256")?, "source-sha256")?,
            linked_resource_sha256,
            helper_sha256: digest(value("helper-sha256")?, "helper-sha256")?,
            base_style_sha256: attributes
                .iter()
                .find(|(key, _)| key == "base-style-sha256")
                .map(|(_, value)| digest(value, "base-style-sha256"))
                .transpose()?,
            browser: value("browser")?.to_owned(),
            browser_executable_sha256: digest(
                value("browser-executable-sha256")?,
                "browser-executable-sha256",
            )?,
            launch_profile_sha256: digest(
                value("launch-profile-sha256")?,
                "launch-profile-sha256",
            )?,
            corpus_manifest_sha256: digest(
                value("corpus-manifest-sha256")?,
                "corpus-manifest-sha256",
            )?,
            taffy_revision: SourceRevision::new(value("taffy-revision")?).map_err(|_| {
                invalid_inventory("layout XML comment has a noncanonical Taffy revision")
            })?,
            taffy_sidecar_sha256: digest(value("taffy-sidecar-sha256")?, "taffy-sidecar-sha256")?,
        })
    }

    fn validate_mapping(
        &self,
        source: &RelativePath,
        output: &RelativePath,
        variant: &str,
    ) -> Result<()> {
        if &self.source != source || output_for(source, variant)? != *output {
            return Err(invalid_inventory(
                "layout XML comment source does not match its report mapping",
            ));
        }
        Ok(())
    }

    fn validate_browser(&self, metadata: &ReportMetadata) -> Result<()> {
        if self.browser != metadata.browser_provenance
            || self.browser_executable_sha256 != metadata.browser_executable_sha256
        {
            return Err(invalid_inventory(
                "layout XML and report browser attestations differ",
            ));
        }
        Ok(())
    }

    fn matches_report_metadata(&self, metadata: &ReportMetadata) -> bool {
        self.helper_sha256 == metadata.helper_sha256
            && self
                .base_style_sha256
                .as_ref()
                .is_none_or(|digest| digest == &metadata.base_style_sha256)
            && self.launch_profile_sha256 == metadata.launch_profile_sha256
            && self.corpus_manifest_sha256 == metadata.corpus_manifest_sha256
            && self.taffy_revision == metadata.taffy_revision
            && self.taffy_sidecar_sha256 == metadata.taffy_sidecar_sha256
    }

    fn matches_current_source(
        &self,
        source_path: &RelativePath,
        source: &DesiredSource,
        current: &CurrentCorpus<'_>,
    ) -> bool {
        &self.source == source_path
            && self.source_sha256 == source.digest
            && self.helper_sha256 == current.helper_digest
            && self.launch_profile_sha256 == current.manifest.launch_digest
            && self.corpus_manifest_sha256 == current.manifest_digest
            && self.taffy_revision == current.manifest.revision
            && self.taffy_sidecar_sha256 == current.sidecar_digest
    }

    fn validate_current_optional_fields(&self, source: &DesiredSource) -> Result<()> {
        if self.linked_resource_sha256.is_some()
            || self.base_style_sha256.is_some() != source.uses_base_style
        {
            return Err(invalid_inventory(
                "layout XML comment optional provenance fields do not match the source",
            ));
        }
        Ok(())
    }
}

fn parse_comment_attributes(value: &str) -> Result<Vec<(String, String)>> {
    let bytes = value.as_bytes();
    let mut offset = 0;
    let mut output = Vec::new();
    while offset < bytes.len() {
        let key_start = offset;
        while bytes
            .get(offset)
            .is_some_and(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        {
            offset += 1;
        }
        if offset == key_start || bytes.get(offset) != Some(&b'=') {
            return Err(invalid_inventory(
                "layout XML comment attribute is malformed",
            ));
        }
        let key = &value[key_start..offset];
        offset += 1;
        let decoded = if key == "schema" {
            let start = offset;
            while bytes.get(offset).is_some_and(u8::is_ascii_digit) {
                offset += 1;
            }
            value[start..offset].to_owned()
        } else {
            if bytes.get(offset) != Some(&b'\"') {
                return Err(invalid_inventory(
                    "layout XML comment value must be double quoted",
                ));
            }
            offset += 1;
            let start = offset;
            while bytes.get(offset).is_some_and(|byte| *byte != b'\"') {
                offset += 1;
            }
            if bytes.get(offset) != Some(&b'\"') {
                return Err(invalid_inventory(
                    "layout XML comment quote is unterminated",
                ));
            }
            let decoded = decode_attribute(&value[start..offset])?;
            offset += 1;
            decoded
        };
        output.push((key.to_owned(), decoded));
        if offset == bytes.len() {
            break;
        }
        if bytes.get(offset) != Some(&b' ') || bytes.get(offset + 1) == Some(&b' ') {
            return Err(invalid_inventory(
                "layout XML comment attributes require one separating space",
            ));
        }
        offset += 1;
    }
    Ok(output)
}

fn validate_comment_order(attributes: &[(String, String)]) -> Result<()> {
    let mut expected = vec!["schema", "source", "source-sha256"];
    if attributes
        .get(expected.len())
        .is_some_and(|(key, _)| key == "linked-resource-sha256")
    {
        expected.push("linked-resource-sha256");
    }
    expected.push("helper-sha256");
    if attributes
        .get(expected.len())
        .is_some_and(|(key, _)| key == "base-style-sha256")
    {
        expected.push("base-style-sha256");
    }
    expected.extend([
        "browser",
        "browser-executable-sha256",
        "launch-profile-sha256",
        "corpus-manifest-sha256",
        "taffy-revision",
        "taffy-sidecar-sha256",
    ]);
    if attributes.iter().map(|(key, _)| key.as_str()).ne(expected) {
        return Err(invalid_inventory(
            "layout XML comment has duplicate, unknown, missing, or misordered attributes",
        ));
    }
    Ok(())
}

fn decode_attribute(value: &str) -> Result<String> {
    let mut decoded = String::new();
    let mut remaining = value;
    while let Some(index) = remaining.find(['&', '<', '\"']) {
        decoded.push_str(&remaining[..index]);
        remaining = &remaining[index..];
        if remaining.starts_with("&amp;") {
            decoded.push('&');
            remaining = &remaining[5..];
        } else if remaining.starts_with("&quot;") {
            decoded.push('\"');
            remaining = &remaining[6..];
        } else if remaining.starts_with("&lt;") {
            decoded.push('<');
            remaining = &remaining[4..];
        } else {
            return Err(invalid_inventory(
                "layout XML comment uses noncanonical attribute escaping",
            ));
        }
    }
    decoded.push_str(remaining);
    Ok(decoded)
}

fn validate_linked_resources(value: &str) -> Result<()> {
    if value.is_empty() {
        return Err(invalid_inventory(
            "linked-resource-sha256 must be omitted when empty",
        ));
    }
    let mut previous = None;
    for record in value.split(',') {
        let (path, digest_text) = record
            .rsplit_once('=')
            .ok_or_else(|| invalid_inventory("linked-resource-sha256 record is malformed"))?;
        RelativePath::new(path).map_err(|_| {
            invalid_inventory("linked-resource-sha256 path is not a strict relative path")
        })?;
        digest(digest_text, "linked-resource-sha256")?;
        if previous.is_some_and(|previous: &str| previous >= path) {
            return Err(invalid_inventory(
                "linked-resource-sha256 records are not strictly path sorted",
            ));
        }
        previous = Some(path);
    }
    Ok(())
}

fn digest(value: &str, label: &str) -> Result<Sha256Digest> {
    Sha256Digest::from_text(value).map_err(|_| {
        invalid_inventory(format!(
            "layout XML comment {label} is not lowercase SHA-256"
        ))
    })
}

fn parse_browser_provenance(metadata: &ReportMetadata, manifest: &LayoutManifest) -> Result<()> {
    let format = manifest
        .browser
        .provenance_format
        .replace("{version}", &metadata.browser_version);
    let (prefix, suffix) = format
        .split_once("{repository_relative_executable}")
        .expect("validated manifest provenance placeholder");
    let path = metadata
        .browser_provenance
        .strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(suffix))
        .ok_or_else(|| {
            invalid_inventory("report browser provenance does not match the manifest format")
        })?;
    let path = RelativePath::new(path).map_err(|_| {
        invalid_inventory("report browser provenance path is not a strict relative path")
    })?;
    let prefix = format!("{}/", manifest.browser.cache_root.as_str());
    if !path.as_str().starts_with(&prefix) {
        return Err(invalid_inventory(
            "report browser provenance is not beneath the manifest cache root",
        ));
    }
    Ok(())
}

struct CanonicalJson<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl CanonicalJson<'_> {
    fn validate(bytes: &[u8]) -> bool {
        let Some(body) = bytes.strip_suffix(b"\n") else {
            return false;
        };
        if body.is_empty()
            || body.ends_with(b"\n")
            || body.contains(&b'\r')
            || body.contains(&b'\t')
        {
            return false;
        }
        let mut parser = CanonicalJson {
            bytes: body,
            offset: 0,
        };
        parser.value(0) && parser.offset == parser.bytes.len()
    }

    fn value(&mut self, indent: usize) -> bool {
        match self.peek() {
            Some(b'{') => self.object(indent),
            Some(b'[') => self.array(indent),
            Some(b'\"') => self.string(),
            Some(b'0'..=b'9') => self.number(),
            Some(b'n') => self.take(b"null"),
            _ => false,
        }
    }

    fn object(&mut self, indent: usize) -> bool {
        if !self.byte(b'{') {
            return false;
        }
        if self.byte(b'}') {
            return true;
        }
        if !self.byte(b'\n') {
            return false;
        }
        loop {
            if !self.spaces(indent + 2)
                || !self.string()
                || !self.take(b": ")
                || !self.value(indent + 2)
            {
                return false;
            }
            if self.byte(b',') {
                if !self.byte(b'\n') {
                    return false;
                }
                continue;
            }
            return self.byte(b'\n') && self.spaces(indent) && self.byte(b'}');
        }
    }

    fn array(&mut self, indent: usize) -> bool {
        if !self.byte(b'[') {
            return false;
        }
        if self.byte(b']') {
            return true;
        }
        if !self.byte(b'\n') {
            return false;
        }
        loop {
            if !self.spaces(indent + 2) || !self.value(indent + 2) {
                return false;
            }
            if self.byte(b',') {
                if !self.byte(b'\n') {
                    return false;
                }
                continue;
            }
            return self.byte(b'\n') && self.spaces(indent) && self.byte(b']');
        }
    }

    fn string(&mut self) -> bool {
        if !self.byte(b'\"') {
            return false;
        }
        loop {
            match self.peek() {
                None | Some(b'\n' | b'\r' | b'\"') => return self.byte(b'\"'),
                Some(0x00..=0x1f) => return false,
                Some(b'\\') => {
                    self.offset += 1;
                    match self.peek() {
                        Some(b'\"' | b'\\' | b'b' | b'f' | b'n' | b'r' | b't') => {
                            self.offset += 1;
                        }
                        Some(b'u') => {
                            self.offset += 1;
                            let start = self.offset;
                            if self.offset + 4 > self.bytes.len()
                                || !self.bytes[self.offset..self.offset + 4].iter().all(|byte| {
                                    byte.is_ascii_digit() || (b'a'..=b'f').contains(byte)
                                })
                            {
                                return false;
                            }
                            self.offset += 4;
                            let Ok(code) = u8::from_str_radix(
                                std::str::from_utf8(&self.bytes[start..self.offset])
                                    .unwrap_or("ffff"),
                                16,
                            ) else {
                                return false;
                            };
                            if code > 0x1f || matches!(code, 8 | 9 | 10 | 12 | 13) {
                                return false;
                            }
                        }
                        _ => return false,
                    }
                }
                Some(_) => self.offset += 1,
            }
        }
    }

    fn number(&mut self) -> bool {
        if self.byte(b'0') {
            return !self.peek().is_some_and(|byte| byte.is_ascii_digit());
        }
        let start = self.offset;
        while self.peek().is_some_and(|byte| byte.is_ascii_digit()) {
            self.offset += 1;
        }
        self.offset > start
    }

    fn spaces(&mut self, count: usize) -> bool {
        (0..count).all(|_| self.byte(b' '))
    }

    fn take(&mut self, expected: &[u8]) -> bool {
        if self.bytes.get(self.offset..self.offset + expected.len()) == Some(expected) {
            self.offset += expected.len();
            true
        } else {
            false
        }
    }

    fn byte(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.offset += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.offset).copied()
    }
}

fn next_value<'de, A, T>(map: &mut A, expected: &'static str) -> std::result::Result<T, A::Error>
where
    A: MapAccess<'de>,
    T: Deserialize<'de>,
{
    let key = map
        .next_key::<String>()?
        .ok_or_else(|| A::Error::missing_field(expected))?;
    if key != expected {
        return Err(A::Error::custom(format!(
            "expected member {expected}, found {key}"
        )));
    }
    map.next_value()
}

fn finish_map<'de, A>(map: &mut A) -> std::result::Result<(), A::Error>
where
    A: MapAccess<'de>,
{
    if let Some(key) = map.next_key::<String>()? {
        return Err(A::Error::custom(format!("unexpected member {key}")));
    }
    Ok(())
}

macro_rules! ordered_map_deserialize {
    ($type:ident, $visitor:ident, {$($field:ident : $field_type:ty => $name:literal),+ $(,)?}) => {
        impl<'de> Deserialize<'de> for $type {
            fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct $visitor;
                impl<'de> Visitor<'de> for $visitor {
                    type Value = $type;
                    fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                        formatter.write_str("a canonical ordered layout report object")
                    }
                    fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
                    where
                        A: MapAccess<'de>,
                    {
                        $(let $field: $field_type = next_value(&mut map, $name)?;)+
                        finish_map(&mut map)?;
                        Ok($type { $($field),+ })
                    }
                }
                deserializer.deserialize_map($visitor)
            }
        }
    };
}

ordered_map_deserialize!(ReportMetadata, MetadataVisitor, {
    schema_version: u8 => "schema_version",
    generator: String => "generator",
    browser_source: String => "browser_source",
    browser_version: String => "browser_version",
    browser_provenance: String => "browser_provenance",
    browser_executable_sha256: Sha256Digest => "browser_executable_sha256",
    launch_profile_sha256: Sha256Digest => "launch_profile_sha256",
    helper_sha256: Sha256Digest => "helper_sha256",
    base_style_sha256: Sha256Digest => "base_style_sha256",
    corpus_manifest_sha256: Sha256Digest => "corpus_manifest_sha256",
    taffy_revision: SourceRevision => "taffy_revision",
    taffy_sidecar_sha256: Sha256Digest => "taffy_sidecar_sha256",
});

impl<'de> Deserialize<'de> for ReportSummary {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct SummaryVisitor;
        impl<'de> Visitor<'de> for SummaryVisitor {
            type Value = ReportSummary;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a canonical ordered layout report summary")
            }
            fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let convert = |value: u64| {
                    usize::try_from(value)
                        .map_err(|_| A::Error::custom("summary count overflows usize"))
                };
                let generated = convert(next_value(&mut map, "generated")?)?;
                let unsupported = convert(next_value(&mut map, "unsupported")?)?;
                let expected_fail = convert(next_value(&mut map, "expected_fail")?)?;
                let quarantined = convert(next_value(&mut map, "quarantined")?)?;
                let failed_to_generate = convert(next_value(&mut map, "failed_to_generate")?)?;
                finish_map(&mut map)?;
                Ok(ReportSummary {
                    generated,
                    unsupported,
                    expected_fail,
                    quarantined,
                    failed_to_generate,
                })
            }
        }
        deserializer.deserialize_map(SummaryVisitor)
    }
}

impl<'de> Deserialize<'de> for GeneratedEntry {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct GeneratedVisitor;
        impl<'de> Visitor<'de> for GeneratedVisitor {
            type Value = GeneratedEntry;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a current or migration-only generated entry")
            }
            fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let name = next_value(&mut map, "name")?;
                let source = next_value(&mut map, "source")?;
                let output = next_value(&mut map, "output")?;
                let key = map
                    .next_key::<String>()?
                    .ok_or_else(|| A::Error::missing_field("variant"))?;
                let (output_sha256, variant) = if key == "output_sha256" {
                    let digest = Some(map.next_value()?);
                    let variant = next_value(&mut map, "variant")?;
                    (digest, variant)
                } else if key == "variant" {
                    (None, map.next_value()?)
                } else {
                    return Err(A::Error::custom(format!("unexpected member {key}")));
                };
                finish_map(&mut map)?;
                Ok(GeneratedEntry {
                    name,
                    source,
                    output,
                    output_sha256,
                    variant,
                })
            }
        }
        deserializer.deserialize_map(GeneratedVisitor)
    }
}

ordered_map_deserialize!(UnsupportedEntry, UnsupportedVisitor, {
    name: String => "name",
    source: RelativePath => "source",
    variant: String => "variant",
    reason: String => "reason",
});

ordered_map_deserialize!(DispositionEntry, DispositionVisitor, {
    name: String => "name",
    source: RelativePath => "source",
    reason: String => "reason",
});

ordered_map_deserialize!(LayoutReport, ReportVisitor, {
    metadata: ReportMetadata => "metadata",
    filter: Option<RelativePath> => "filter",
    summary: ReportSummary => "summary",
    generated: Vec<GeneratedEntry> => "generated",
    unsupported: Vec<UnsupportedEntry> => "unsupported",
    expected_fail: Vec<DispositionEntry> => "expected_fail",
    quarantined: Vec<DispositionEntry> => "quarantined",
    failed_to_generate: Vec<DispositionEntry> => "failed_to_generate",
});

pub(super) fn desired_disposition(
    status: LayoutCaseStatus,
    name: String,
    reason: String,
) -> DesiredDisposition {
    match status {
        LayoutCaseStatus::Active => DesiredDisposition::Active,
        LayoutCaseStatus::ExpectedFail => DesiredDisposition::ExpectedFail { name, reason },
        LayoutCaseStatus::Unsupported => DesiredDisposition::Unsupported { name, reason },
        LayoutCaseStatus::Quarantined => DesiredDisposition::Quarantined { name, reason },
    }
}

fn report_io(source: GeneratorError) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::InvalidInventory,
        "read layout XML/report inventory",
        source.to_string(),
        source,
    )
}

fn invalid_inventory(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidInventory,
        "validate layout XML/report inventory",
        detail,
    )
}
