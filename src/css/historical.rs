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

    pub(super) const fn has_report(&self) -> bool {
        self.report.is_some()
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

    pub(super) fn require_filtered_ownership(
        &self,
        selected: &BTreeSet<RelativePath>,
    ) -> Result<()> {
        if self.inventory.is_none() {
            return Err(verification(
                "CSS expectation root is absent; run full generation first",
            ));
        }
        if self.report.is_none() {
            return Err(verification(
                "CSS expectation root has no historical full-report ownership",
            ));
        }
        if let Some(unowned) = selected
            .iter()
            .find(|path| !self.classified_paths.contains(*path))
        {
            return Err(verification(format!(
                "CSS expectation is not historically owned: {}",
                unowned.as_str()
            )));
        }
        Ok(())
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
    let mut paths = Vec::with_capacity(report.artifacts().len());
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
        sidecar::validate_fixture_path(&source_relative)?;
        sidecar::validate_fixture_path(&output_relative)?;
        paths.push((source_relative, output_relative));
    }
    validate_nonoverlapping_paths(
        paths.iter().map(|(source, _)| source),
        "historical source path",
    )?;
    validate_nonoverlapping_paths(
        paths.iter().map(|(_, output)| output),
        "historical output path",
    )?;

    let mut classified = BTreeSet::from([report_relative]);
    for (source_relative, output_relative) in paths {
        if source_relative != output_relative || output_relative.as_str() == REPORT_RELATIVE {
            return Err(invalid_inventory(
                "CSS historical source/output mapping or report reservation is invalid",
            ));
        }
        classified.insert(output_relative);
    }
    if report.counts().total()? != case_count {
        return Err(invalid_inventory(
            "CSS historical report counts do not match artifact case counts",
        ));
    }
    Ok(classified)
}

fn validate_nonoverlapping_paths<'a>(
    paths: impl IntoIterator<Item = &'a RelativePath>,
    label: &str,
) -> Result<()> {
    let mut prior = Vec::<&RelativePath>::new();
    for path in paths {
        for collision in &prior {
            if paths_overlap(collision, path, label)? {
                return Err(invalid_inventory(format!(
                    "CSS {label}s collide: {} and {}",
                    collision.as_str(),
                    path.as_str()
                )));
            }
        }
        prior.push(path);
    }
    Ok(())
}

fn paths_overlap(left: &RelativePath, right: &RelativePath, label: &str) -> Result<bool> {
    Ok(is_same_or_descendant(left, right, label)? || is_same_or_descendant(right, left, label)?)
}

fn is_same_or_descendant(
    path: &RelativePath,
    ancestor: &RelativePath,
    label: &str,
) -> Result<bool> {
    let mut path_components = path.as_str().split('/');
    for ancestor_component in ancestor.as_str().split('/') {
        let Some(path_component) = path_components.next() else {
            return Ok(false);
        };
        if !target_components_equal(path_component, ancestor_component, label)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn target_components_equal(left: &str, right: &str, label: &str) -> Result<bool> {
    if left == right {
        return Ok(true);
    }
    if left.is_ascii() && right.is_ascii() {
        #[cfg(target_os = "macos")]
        return Ok(left.eq_ignore_ascii_case(right));
        #[cfg(not(target_os = "macos"))]
        return Ok(false);
    }
    Err(invalid_inventory(format!(
        "cannot prove distinct {label} components: {left:?} and {right:?}"
    )))
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

fn verification(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::Verification,
        "validate CSS filtered ownership",
        detail,
    )
}

#[cfg(test)]
mod tests {
    use super::validate_nonoverlapping_paths;
    use crate::RelativePath;

    fn paths(values: &[&str]) -> Vec<RelativePath> {
        values
            .iter()
            .map(|value| RelativePath::new(value).expect("strict path"))
            .collect()
    }

    #[test]
    fn css_historical_inventory_path_collision_matrix_uses_strict_target_components() {
        for colliding in [
            ["a.json", "a.json"],
            ["a.json", "a.json/b.json"],
            ["a.json/b.json", "a.json"],
        ] {
            let paths = paths(&colliding);
            validate_nonoverlapping_paths(paths.iter(), "test path")
                .expect_err("exact or ancestor collision");
        }

        let distinct = paths(&["a.json", "a.json-copy/b.json"]);
        validate_nonoverlapping_paths(distinct.iter(), "test path")
            .expect("partial component is not a collision");

        let target_aliases = paths(&["Case.json", "case.json"]);
        #[cfg(target_os = "macos")]
        validate_nonoverlapping_paths(target_aliases.iter(), "test path")
            .expect_err("macOS target aliases collide");
        #[cfg(not(target_os = "macos"))]
        validate_nonoverlapping_paths(target_aliases.iter(), "test path")
            .expect("case-distinct paths do not alias on this target");
    }
}
