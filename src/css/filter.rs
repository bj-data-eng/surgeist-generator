use std::collections::BTreeSet;

use crate::{GeneratorError, GeneratorErrorKind, RelativePath, Result, RunScope};

use super::expectation::DerivedExpectations;
use super::fixture::ValidatedImport;
use super::manifest::REPORT_RELATIVE;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct SelectionLedger {
    scope: RunScope,
    desired_expectations: BTreeSet<RelativePath>,
    selected_expectations: BTreeSet<RelativePath>,
}

impl SelectionLedger {
    pub(super) fn new(imported: &ValidatedImport, filter: Option<&RelativePath>) -> Result<Self> {
        let desired_expectations = imported
            .fixtures()
            .iter()
            .map(|fixture| fixture.path.clone())
            .collect::<BTreeSet<_>>();
        let (scope, selected_expectations) = match filter {
            None => (RunScope::Full, desired_expectations.clone()),
            Some(filter) => {
                let selected = desired_expectations
                    .iter()
                    .filter(|path| matches(filter, path))
                    .cloned()
                    .collect::<BTreeSet<_>>();
                if selected.is_empty() {
                    return Err(verification(format!(
                        "CSS filter matches no current fixture: {}",
                        filter.as_str()
                    )));
                }
                (RunScope::Filtered(filter.clone()), selected)
            }
        };
        Ok(Self {
            scope,
            desired_expectations,
            selected_expectations,
        })
    }

    pub(super) const fn scope(&self) -> &RunScope {
        &self.scope
    }

    pub(super) const fn is_filtered(&self) -> bool {
        matches!(self.scope, RunScope::Filtered(_))
    }

    pub(super) const fn selected_expectations(&self) -> &BTreeSet<RelativePath> {
        &self.selected_expectations
    }

    pub(super) fn validate_derived(&self, expectations: &DerivedExpectations) -> Result<()> {
        let derived = expectations
            .artifacts
            .iter()
            .map(|artifact| artifact.path.clone())
            .collect::<BTreeSet<_>>();
        if derived != self.desired_expectations {
            return Err(super::invalid_inventory(
                "derived CSS expectation membership differs from the current import",
            ));
        }
        if !self.selected_expectations.is_subset(&derived) {
            return Err(super::invalid_inventory(
                "CSS selection ledger is incomplete for the derived expectations",
            ));
        }
        Ok(())
    }

    pub(super) fn includes(&self, path: &RelativePath) -> bool {
        self.selected_expectations.contains(path)
    }
}

pub(super) fn validate_request_filter(filter: &RelativePath) -> Result<()> {
    if filter.as_str() == REPORT_RELATIVE {
        return Err(super::cli_error(
            "construct CSS request",
            "generate reserves generation-reports/all.json for the full report",
        ));
    }
    Ok(())
}

fn matches(filter: &RelativePath, fixture: &RelativePath) -> bool {
    let exact_file = filter
        .as_str()
        .rsplit('/')
        .next()
        .is_some_and(|component| component.ends_with(".json"));
    if exact_file {
        fixture == filter
    } else {
        fixture == filter
            || fixture
                .as_str()
                .strip_prefix(filter.as_str())
                .is_some_and(|suffix| suffix.starts_with('/'))
    }
}

fn verification(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::Verification,
        "select CSS generation fixtures",
        detail,
    )
}
