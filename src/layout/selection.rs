use std::collections::{BTreeMap, BTreeSet};

use crate::core::{CORPUS_FILE_MODE, Inventory, InventoryPolicy, NodeKind, RootedFs};
use crate::{GeneratorError, GeneratorErrorKind, RelativePath, Result, RunScope, Sha256Digest};

use super::case::{LayoutCase, LayoutCaseStatus};
use super::manifest::{HTML_ROOT, LayoutManifest, SIDECAR_FILE};
use super::sidecar::TaffyImportSidecar;

pub(super) const HELPER_SCRIPT: &str = "scripts/gentest/test_helper.js";
pub(super) const BASE_STYLE: &str = "scripts/gentest/test_base_style.css";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum FixtureDisposition {
    Active,
    ExpectedFail { name: String, reason: String },
    Unsupported { name: String, reason: String },
    Quarantined { name: String, reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Fixture {
    relative: RelativePath,
    source: RelativePath,
    bytes: Vec<u8>,
    digest: Sha256Digest,
    uses_base_style: bool,
    disposition: FixtureDisposition,
}

impl Fixture {
    pub(super) const fn relative(&self) -> &RelativePath {
        &self.relative
    }

    pub(super) const fn source(&self) -> &RelativePath {
        &self.source
    }

    pub(super) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(super) const fn digest(&self) -> &Sha256Digest {
        &self.digest
    }

    pub(super) const fn uses_base_style(&self) -> bool {
        self.uses_base_style
    }

    pub(super) const fn disposition(&self) -> &FixtureDisposition {
        &self.disposition
    }

    pub(super) fn schedules_browser(&self) -> bool {
        matches!(
            self.disposition,
            FixtureDisposition::Active | FixtureDisposition::ExpectedFail { .. }
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CurrentInputs {
    html_inventory: Inventory,
    fixtures: Vec<Fixture>,
    helper: Vec<u8>,
    helper_digest: Sha256Digest,
    base_style: Vec<u8>,
    sidecar_digest: Sha256Digest,
}

impl CurrentInputs {
    pub(super) fn helper(&self) -> &[u8] {
        &self.helper
    }

    pub(super) fn base_style(&self) -> &[u8] {
        &self.base_style
    }

    pub(super) const fn helper_digest(&self) -> &Sha256Digest {
        &self.helper_digest
    }

    pub(super) fn base_style_digest(&self) -> Sha256Digest {
        Sha256Digest::from_bytes(&self.base_style)
    }

    pub(super) const fn sidecar_digest(&self) -> &Sha256Digest {
        &self.sidecar_digest
    }

    pub(super) fn all_output_paths(&self) -> Result<BTreeSet<RelativePath>> {
        let mut paths = BTreeSet::new();
        for fixture in &self.fixtures {
            for variant in super::xml::Variant::ALL {
                paths.insert(variant.output_path(fixture.source())?);
            }
        }
        Ok(paths)
    }

    pub(super) fn revalidate(&self, rooted: &RootedFs, manifest: &LayoutManifest) -> Result<()> {
        if &inspect(rooted, manifest)? != self {
            return Err(invalid_inventory(
                "layout HTML/helper input changed after preflight",
            ));
        }
        Ok(())
    }
}

pub(super) fn inspect(rooted: &RootedFs, manifest: &LayoutManifest) -> Result<CurrentInputs> {
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
    let sidecar_bytes =
        rooted.read_file(&format!("{HTML_ROOT}/{SIDECAR_FILE}"), CORPUS_FILE_MODE)?;
    let sidecar = TaffyImportSidecar::parse_canonical(&sidecar_bytes)?;
    if sidecar.revision() != &manifest.revision
        || sidecar.source_file_count() != manifest.expected_source_files
    {
        return Err(verification(
            "Taffy import sidecar is stale against the corpus manifest",
        ));
    }

    let authored = manifest
        .authored_cases
        .iter()
        .map(|case| (case.source.clone(), case))
        .collect::<BTreeMap<_, _>>();
    let taffy = sidecar
        .files()
        .iter()
        .map(|file| (file.path.clone(), file))
        .collect::<BTreeMap<_, _>>();
    if let Some(collision) = authored.keys().find(|path| taffy.contains_key(*path)) {
        return Err(invalid_inventory(format!(
            "Surgeist-authored and Taffy fixtures collide: {}",
            collision.as_str()
        )));
    }

    let expected_files = authored
        .keys()
        .chain(taffy.keys())
        .cloned()
        .chain(std::iter::once(sidecar_path.clone()))
        .collect::<BTreeSet<_>>();
    let actual_files = inventory
        .entries()
        .iter()
        .filter(|entry| entry.identity().kind() == NodeKind::Regular)
        .map(|entry| entry.path().clone())
        .collect::<BTreeSet<_>>();
    if actual_files != expected_files
        || inventory
            .entries()
            .iter()
            .any(|entry| match entry.identity().kind() {
                NodeKind::Symlink => true,
                NodeKind::Directory => {
                    let prefix = format!("{}/", entry.path().as_str());
                    !expected_files
                        .iter()
                        .any(|path| path.as_str().starts_with(&prefix))
                }
                NodeKind::Regular => false,
            })
    {
        return Err(invalid_inventory(
            "layout HTML inventory differs from manifest plus Taffy sidecar authority",
        ));
    }

    let mut fixtures = Vec::with_capacity(authored.len() + taffy.len());
    for (relative, file) in taffy {
        let bytes = rooted.read_file(
            &format!("{HTML_ROOT}/{}", relative.as_str()),
            CORPUS_FILE_MODE,
        )?;
        if Sha256Digest::from_bytes(&bytes) != file.sha256 {
            return Err(verification(format!(
                "imported Taffy fixture is stale: {}",
                relative.as_str()
            )));
        }
        fixtures.push(fixture(relative, bytes, FixtureDisposition::Active)?);
    }
    for (relative, case) in authored {
        let bytes = rooted.read_file(
            &format!("{HTML_ROOT}/{}", relative.as_str()),
            CORPUS_FILE_MODE,
        )?;
        if inventory.find(&relative).and_then(|entry| entry.digest())
            != Some(&Sha256Digest::from_bytes(&bytes))
        {
            return Err(invalid_inventory(format!(
                "Surgeist-authored fixture changed during inspection: {}",
                relative.as_str()
            )));
        }
        fixtures.push(fixture(relative, bytes, disposition(case))?);
    }
    fixtures.sort_by(|left, right| left.relative.cmp(&right.relative));

    let helper = rooted
        .read_file(HELPER_SCRIPT, CORPUS_FILE_MODE)
        .map_err(|source| verification_source("read layout helper script", source))?;
    let base_style = rooted
        .read_file(BASE_STYLE, CORPUS_FILE_MODE)
        .map_err(|source| verification_source("read layout base style", source))?;
    let helper_digest = Sha256Digest::from_bytes(&helper);
    Ok(CurrentInputs {
        html_inventory: inventory,
        fixtures,
        helper,
        helper_digest,
        base_style,
        sidecar_digest: Sha256Digest::from_bytes(&sidecar_bytes),
    })
}

fn fixture(
    relative: RelativePath,
    bytes: Vec<u8>,
    disposition: FixtureDisposition,
) -> Result<Fixture> {
    relative
        .as_str()
        .rsplit('/')
        .next()
        .and_then(|file| file.strip_suffix(".html"))
        .ok_or_else(|| invalid_inventory("layout fixture is not a strict .html path"))?;
    let source = RelativePath::new(format!("{HTML_ROOT}/{}", relative.as_str()))?;
    let uses_base_style = bytes
        .windows(b"test_base_style.css".len())
        .any(|window| window == b"test_base_style.css");
    Ok(Fixture {
        relative,
        source,
        digest: Sha256Digest::from_bytes(&bytes),
        bytes,
        uses_base_style,
        disposition,
    })
}

fn disposition(case: &LayoutCase) -> FixtureDisposition {
    match case.status {
        LayoutCaseStatus::Active => FixtureDisposition::Active,
        LayoutCaseStatus::ExpectedFail => FixtureDisposition::ExpectedFail {
            name: case.id.clone(),
            reason: case.reason.clone(),
        },
        LayoutCaseStatus::Unsupported => FixtureDisposition::Unsupported {
            name: case.id.clone(),
            reason: case.reason.clone(),
        },
        LayoutCaseStatus::Quarantined => FixtureDisposition::Quarantined {
            name: case.id.clone(),
            reason: case.reason.clone(),
        },
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SelectionLedger {
    scope: RunScope,
    selected: BTreeSet<RelativePath>,
    scheduled_outputs: BTreeSet<RelativePath>,
}

impl SelectionLedger {
    pub(super) fn new(inputs: &CurrentInputs, filter: Option<&RelativePath>) -> Result<Self> {
        Self::from_fixtures(&inputs.fixtures, filter)
    }

    fn from_fixtures(fixtures: &[Fixture], filter: Option<&RelativePath>) -> Result<Self> {
        let (scope, selected) = match filter {
            None => (
                RunScope::Full,
                fixtures
                    .iter()
                    .map(|fixture| fixture.relative.clone())
                    .collect(),
            ),
            Some(filter) => {
                let selected = fixtures
                    .iter()
                    .filter(|fixture| matches_filter(filter, fixture.relative()))
                    .map(|fixture| fixture.relative.clone())
                    .collect::<BTreeSet<_>>();
                if selected.is_empty() {
                    return Err(verification(format!(
                        "layout filter matches no current fixture: {}",
                        filter.as_str()
                    )));
                }
                (RunScope::Filtered(filter.clone()), selected)
            }
        };
        let mut scheduled_outputs = BTreeSet::new();
        for fixture in fixtures
            .iter()
            .filter(|fixture| selected.contains(fixture.relative()) && fixture.schedules_browser())
        {
            for variant in super::xml::Variant::ALL {
                scheduled_outputs.insert(variant.output_path(fixture.source())?);
            }
        }
        Ok(Self {
            scope,
            selected,
            scheduled_outputs,
        })
    }

    pub(super) const fn scope(&self) -> &RunScope {
        &self.scope
    }

    pub(super) const fn is_filtered(&self) -> bool {
        matches!(self.scope, RunScope::Filtered(_))
    }

    pub(super) fn is_disposition_only(&self) -> bool {
        self.scheduled_outputs.is_empty()
    }

    pub(super) const fn scheduled_outputs(&self) -> &BTreeSet<RelativePath> {
        &self.scheduled_outputs
    }

    pub(super) fn fixtures<'a>(&self, inputs: &'a CurrentInputs) -> Vec<&'a Fixture> {
        inputs
            .fixtures
            .iter()
            .filter(|fixture| self.selected.contains(fixture.relative()))
            .collect()
    }
}

pub(super) fn validate_request_filter(filter: &RelativePath) -> Result<()> {
    if super::manifest::paths_target_equal(filter.as_str(), SIDECAR_FILE) {
        return Err(GeneratorError::new(
            GeneratorErrorKind::Cli,
            "construct layout request",
            "generation filter uses the reserved Taffy sidecar path",
        ));
    }
    Ok(())
}

pub(super) fn matches_filter(filter: &RelativePath, fixture: &RelativePath) -> bool {
    if filter
        .as_str()
        .rsplit('/')
        .next()
        .is_some_and(|component| component.ends_with(".html"))
    {
        fixture == filter
    } else {
        fixture == filter
            || fixture
                .as_str()
                .strip_prefix(filter.as_str())
                .is_some_and(|suffix| suffix.starts_with('/'))
    }
}

fn invalid_inventory(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidInventory,
        "inspect layout generation inputs",
        detail,
    )
}

fn verification(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::Verification,
        "select layout generation fixtures",
        detail,
    )
}

fn verification_source(operation: &str, source: GeneratorError) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::Verification,
        operation,
        source.to_string(),
        source,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        Fixture, FixtureDisposition, SelectionLedger, matches_filter, validate_request_filter,
    };
    use crate::{GeneratorErrorKind, RelativePath, Sha256Digest};

    fn path(value: &str) -> RelativePath {
        RelativePath::new(value).expect("strict path")
    }

    #[test]
    fn layout_filter_exact_file() {
        assert!(matches_filter(&path("grid/a.html"), &path("grid/a.html")));
        assert!(!matches_filter(
            &path("grid/a.html"),
            &path("grid/a/b.html")
        ));
    }

    #[test]
    fn layout_filter_component_prefix() {
        assert!(matches_filter(&path("grid/a"), &path("grid/a/case.html")));
    }

    #[test]
    fn layout_filter_rejects_partial_component() {
        assert!(!matches_filter(&path("grid/a"), &path("grid/ab/case.html")));
    }

    fn fixture(path_value: &str, disposition: FixtureDisposition) -> Fixture {
        let relative = path(path_value);
        let source = path(&format!("html/{path_value}"));
        Fixture {
            relative,
            source,
            bytes: b"<div></div>".to_vec(),
            digest: Sha256Digest::from_bytes(b"<div></div>"),
            uses_base_style: false,
            disposition,
        }
    }

    #[test]
    fn layout_filter_rejects_reserved() {
        let error = validate_request_filter(&path(".surgeist-taffy-source.json"))
            .expect_err("reserved sidecar filter");
        assert_eq!(error.kind(), GeneratorErrorKind::Cli);
    }

    #[test]
    fn layout_filter_absent_is_verification() {
        let fixtures = [fixture("grid/a.html", FixtureDisposition::Active)];
        let error = SelectionLedger::from_fixtures(&fixtures, Some(&path("absent")))
            .expect_err("absent filter");
        assert_eq!(error.kind(), GeneratorErrorKind::Verification);
    }

    #[test]
    fn layout_filter_disposition_only_is_noop() {
        let fixtures = [fixture(
            "grid/unsupported.html",
            FixtureDisposition::Unsupported {
                name: "unsupported".to_owned(),
                reason: "manifest".to_owned(),
            },
        )];
        let ledger = SelectionLedger::from_fixtures(&fixtures, Some(&path("grid")))
            .expect("disposition-only selection");
        assert!(ledger.is_disposition_only());
    }

    #[test]
    fn layout_filter_expected_fail_schedules_and_accounts() {
        let fixtures = [fixture(
            "grid/expected.html",
            FixtureDisposition::ExpectedFail {
                name: "expected".to_owned(),
                reason: "known".to_owned(),
            },
        )];
        let ledger = SelectionLedger::from_fixtures(&fixtures, Some(&path("grid")))
            .expect("expected-fail selection");
        assert_eq!(ledger.scheduled_outputs().len(), 4);
    }

    #[test]
    fn layout_filter_manifest_unsupported_has_no_job() {
        let fixtures = [fixture(
            "grid/unsupported.html",
            FixtureDisposition::Unsupported {
                name: "unsupported".to_owned(),
                reason: "known".to_owned(),
            },
        )];
        let ledger = SelectionLedger::from_fixtures(&fixtures, None).expect("full selection");
        assert!(ledger.scheduled_outputs().is_empty());
    }

    #[test]
    fn layout_filter_matches_taffy_sidecar_fixture() {
        let fixtures = [fixture("grid/taffy.html", FixtureDisposition::Active)];
        let ledger = SelectionLedger::from_fixtures(&fixtures, Some(&path("grid/taffy.html")))
            .expect("Taffy selection");
        assert_eq!(ledger.scheduled_outputs().len(), 4);
    }

    #[test]
    fn layout_scoped_report_uses_filter_matcher() {
        assert!(matches_filter(
            &path("grid"),
            &path("grid/nested/case.html")
        ));
        assert!(!matches_filter(&path("grid"), &path("grid-copy/case.html")));
    }
}
