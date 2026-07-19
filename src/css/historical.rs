use std::collections::BTreeSet;

use crate::core::{CORPUS_FILE_MODE, Inventory, InventoryPolicy, NodeKind, RootedFs};
use crate::{GenerationReport, GeneratorError, GeneratorErrorKind, RelativePath, Result};

use super::fixture::validate_visible_inventory;
use super::manifest::{CSSTREE_REPOSITORY, CssManifest, REPORT_RELATIVE};
use super::sidecar;

const GENERATOR: &str = "surgeist-css-generate";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct HistoricalInventory {
    inventory: Option<Inventory>,
    report: Option<GenerationReport>,
    classified_paths: BTreeSet<RelativePath>,
    fresh_outputs: BTreeSet<RelativePath>,
}

impl HistoricalInventory {
    pub(super) fn classified_paths(&self) -> &BTreeSet<RelativePath> {
        &self.classified_paths
    }

    pub(super) fn validate_union(&self, desired: &BTreeSet<RelativePath>) -> Result<()> {
        let Some(inventory) = &self.inventory else {
            return Ok(());
        };
        let admitted = self
            .classified_paths
            .union(desired)
            .cloned()
            .collect::<BTreeSet<_>>();
        validate_visible_inventory(inventory, &admitted, "CSS expectation root")
    }
}

pub(super) fn inspect(rooted: &RootedFs, manifest: &CssManifest) -> Result<HistoricalInventory> {
    let inventory = Inventory::scan(
        rooted,
        manifest.expectation_root.as_str(),
        InventoryPolicy::FinalCorpus,
    )?;
    let Some(current) = inventory.as_ref() else {
        return Ok(HistoricalInventory {
            inventory,
            report: None,
            classified_paths: BTreeSet::new(),
            fresh_outputs: BTreeSet::new(),
        });
    };
    if current.entries().is_empty() {
        return Ok(HistoricalInventory {
            inventory,
            report: None,
            classified_paths: BTreeSet::new(),
            fresh_outputs: BTreeSet::new(),
        });
    }

    let report_relative = super::report::relative_path(manifest)?;
    let entry = current.find(&report_relative).ok_or_else(|| {
        invalid_inventory("nonempty CSS expectation root has no historical full report")
    })?;
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
    let classified_paths = validate_report(manifest, &report, report_relative)?;
    let fresh_outputs = report
        .artifacts()
        .iter()
        .filter_map(|artifact| {
            let relative = strip_root(artifact.output_path(), &manifest.expectation_root)?;
            current.find(&relative).and_then(|entry| {
                (entry.identity().kind() == NodeKind::Regular
                    && entry.digest() == Some(artifact.output_digest()))
                .then_some(relative)
            })
        })
        .collect();
    Ok(HistoricalInventory {
        inventory,
        report: Some(report),
        classified_paths,
        fresh_outputs,
    })
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
    let mut source_paths = BTreeSet::new();
    let mut classified = BTreeSet::from([report_relative]);
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
        if !source_paths.insert(source_relative) {
            return Err(invalid_inventory(
                "CSS historical report repeats a source path",
            ));
        }
        if !classified.insert(output_relative) {
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
    Ok(classified)
}

fn strip_root(path: &RelativePath, root: &RelativePath) -> Option<RelativePath> {
    path.as_str()
        .strip_prefix(root.as_str())
        .and_then(|suffix| suffix.strip_prefix('/'))
        .and_then(|relative| RelativePath::new(relative).ok())
}

fn invalid_inventory(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidInventory,
        "validate CSS historical inventory",
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
