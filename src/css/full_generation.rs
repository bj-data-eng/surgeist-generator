use std::collections::BTreeSet;

use crate::core::{
    ArtifactPlan, ArtifactReservation, Domain, GenerationLease, NamespaceDisjointness,
    PublicationInventory, PublicationPolicy, RootedFs,
};
use crate::{RelativePath, Result, RunScope};

use super::CssRequest;

const GENERATOR: &str = "surgeist-css-generate";
const COMMAND: &str = "generate";
const MANIFEST_FILE: &str = "corpus.toml";

pub(super) fn run(request: &CssRequest) -> Result<()> {
    #[cfg(test)]
    return run_impl(request, || {}, || {});
    #[cfg(not(test))]
    run_impl(request, || {})
}

#[cfg(test)]
pub(super) fn run_with_pre_lease_hook(request: &CssRequest, hook: impl FnOnce()) -> Result<()> {
    run_impl(request, hook, || {})
}

#[cfg(test)]
pub(super) fn run_with_inter_scan_hook(request: &CssRequest, hook: impl FnOnce()) -> Result<()> {
    run_impl(request, || {}, hook)
}

fn run_impl(
    request: &CssRequest,
    pre_lease_hook: impl FnOnce(),
    #[cfg(test)] inter_scan_hook: impl FnOnce(),
) -> Result<()> {
    let location = request.location();
    let manifest_path = location.corpus_root().join(MANIFEST_FILE);
    let manifest_bytes = super::importer::read_manifest_file(&manifest_path)?;
    let manifest = super::manifest::parse(&manifest_bytes, &manifest_path)?;

    let reservation = ArtifactReservation::new(Domain::Css)?;
    let expectation_root_path = manifest.expectation_root.join(location.corpus_root());
    let import_root_path = manifest.import_root.join(location.corpus_root());
    let external_stage_path = reservation.external_stage().join(location.corpus_root());
    let protection = NamespaceDisjointness::for_mutation(
        location,
        &[
            ("CSS expectation root", expectation_root_path.as_path()),
            ("CSS transaction stage", external_stage_path.as_path()),
        ],
        &[
            ("CSS corpus manifest", manifest_path.as_path()),
            ("CSS import root", import_root_path.as_path()),
        ],
    )?;
    let preflight_rooted = RootedFs::open_corpus(location)?;
    let historical = super::historical::inspect(&preflight_rooted, &manifest)?;
    drop(preflight_rooted);
    pre_lease_hook();
    let lease = GenerationLease::acquire_with_revalidation(
        location,
        Domain::Css,
        GENERATOR,
        &RunScope::Full,
        COMMAND,
        |rooted| protection.revalidate(rooted),
    )?;

    let binding = lease.bind(location, Domain::Css)?;
    let operation = binding.validate(location, Domain::Css)?;
    let rooted = operation.rooted();
    protection.revalidate(rooted)?;
    super::importer::revalidate_manifest(rooted, &manifest_bytes)?;
    let imported = super::fixture::inspect(rooted, &manifest)?;
    let report_relative = super::report::relative_path(&manifest)?;
    let desired = imported
        .fixtures()
        .iter()
        .map(|fixture| fixture.path.clone())
        .chain(std::iter::once(report_relative.clone()))
        .collect::<BTreeSet<_>>();
    let current_historical = super::historical::inspect(rooted, &manifest)?;
    current_historical.validate_union(&desired)?;
    if current_historical != historical {
        return Err(super::invalid_inventory(
            "CSS historical inventory changed before held validation",
        ));
    }
    let expectations = super::expectation::derive(&imported, &manifest)?;
    let report_bytes = super::report::build(
        &manifest,
        &manifest_bytes,
        imported.sidecar_digest(),
        &expectations,
    )?;
    if desired != desired_paths(&expectations, &report_relative) {
        return Err(super::invalid_inventory(
            "derived CSS expectation membership differs from the current import",
        ));
    }
    drop(operation);

    let mut artifacts = expectations
        .artifacts
        .into_iter()
        .map(|artifact| (artifact.path, artifact.bytes))
        .collect::<Vec<_>>();
    artifacts.push((report_relative.clone(), report_bytes));
    let classified = historical
        .classified_paths()
        .union(&desired)
        .cloned()
        .collect::<Vec<_>>();
    let inventory = PublicationInventory::new(
        classified,
        desired.iter().cloned().collect(),
        vec![report_relative],
    )?;
    let plan = ArtifactPlan::new(
        location,
        Domain::Css,
        &lease,
        manifest.expectation_root.clone(),
        PublicationPolicy::CleanFull,
        artifacts,
        inventory,
    )?
    .with_reservation(reservation)?;
    let revalidate = |rooted: &RootedFs| {
        protection.revalidate(rooted)?;
        super::importer::revalidate_manifest(rooted, &manifest_bytes)?;
        if super::fixture::inspect(rooted, &manifest)? != imported {
            return Err(super::invalid_inventory(
                "current CSS import changed after held validation",
            ));
        }
        let current_historical = super::historical::inspect(rooted, &manifest)?;
        current_historical.validate_union(&desired)?;
        if current_historical != historical {
            return Err(super::invalid_inventory(
                "CSS historical inventory changed after held validation",
            ));
        }
        Ok(())
    };
    #[cfg(test)]
    return plan.install_with_revalidation_and_inter_scan_hook(revalidate, inter_scan_hook);
    #[cfg(not(test))]
    plan.install_with_revalidation(revalidate)
}

fn desired_paths(
    expectations: &super::expectation::DerivedExpectations,
    report: &RelativePath,
) -> BTreeSet<RelativePath> {
    expectations
        .artifacts
        .iter()
        .map(|artifact| artifact.path.clone())
        .chain(std::iter::once(report.clone()))
        .collect()
}
