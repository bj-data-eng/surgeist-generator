use std::collections::BTreeSet;

use crate::core::{CORPUS_FILE_MODE, Domain, GenerationCheck, RootedFs};
use crate::{GeneratorError, GeneratorErrorKind, Result};

use super::CssRequest;

const MANIFEST_FILE: &str = "corpus.toml";

pub(super) fn run(request: &CssRequest) -> Result<()> {
    let check = GenerationCheck::acquire(request.location(), Domain::Css)
        .map_err(coordination_verification)?;
    let result = inspect_current(request);
    let finish = check.finish().map_err(coordination_verification);
    match (result, finish) {
        (_, Err(error)) => Err(error),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn coordination_verification(source: GeneratorError) -> GeneratorError {
    if source.kind() == GeneratorErrorKind::UnsupportedPlatform {
        return source;
    }
    GeneratorError::with_source(
        GeneratorErrorKind::Verification,
        "inspect CSS generation coordination",
        source.to_string(),
        source,
    )
}

fn inspect_current(request: &CssRequest) -> Result<()> {
    let location = request.location();
    let rooted = RootedFs::open_corpus(location)?;
    let manifest_path = location.corpus_root().join(MANIFEST_FILE);
    let manifest_bytes = super::importer::read_manifest_file(&manifest_path)?;
    super::importer::revalidate_manifest(&rooted, &manifest_bytes)?;
    let manifest = super::manifest::parse(&manifest_bytes, &manifest_path)?;
    let imported = super::fixture::inspect(&rooted, &manifest)?;
    let expectations = super::expectation::derive(&imported, &manifest)?;
    let report_relative = super::report::relative_path(&manifest)?;
    let desired = expectations
        .artifacts
        .iter()
        .map(|artifact| artifact.path.clone())
        .chain(std::iter::once(report_relative))
        .collect::<BTreeSet<_>>();
    let historical = super::historical::inspect(&rooted, &manifest)?;
    historical.validate_union(&desired)?;
    if !historical.has_report() {
        return Err(verification(
            "CSS expectation root and full report are absent",
        ));
    }

    for expectation in &expectations.artifacts {
        let relative = expectation.path.as_str();
        let corpus_path = format!("{}/{relative}", manifest.expectation_root.as_str());
        if !rooted
            .exists(&corpus_path)
            .map_err(invalid_inventory_from)?
        {
            return Err(verification(format!(
                "CSS expectation is absent: {relative}"
            )));
        }
        let bytes = rooted
            .read_file(&corpus_path, CORPUS_FILE_MODE)
            .map_err(invalid_inventory_from)?;
        super::expectation::validate_persisted(&bytes, &expectation.path, &manifest)?;
        if bytes != expectation.bytes {
            return Err(verification(format!(
                "CSS expectation is stale: {relative}"
            )));
        }
    }

    let report_bytes = rooted
        .read_file(manifest.report_file.as_str(), CORPUS_FILE_MODE)
        .map_err(invalid_inventory_from)?;
    let expected_report = super::report::build(
        &manifest,
        &manifest_bytes,
        imported.sidecar_digest(),
        &expectations,
    )?;
    if report_bytes != expected_report {
        return Err(verification("CSS generation report is stale"));
    }

    super::importer::revalidate_manifest(&rooted, &manifest_bytes)?;
    if super::fixture::inspect(&rooted, &manifest)? != imported {
        return Err(verification("CSS import changed during corpus checking"));
    }
    let final_historical = super::historical::inspect(&rooted, &manifest)?;
    final_historical.validate_union(&desired)?;
    if final_historical != historical {
        return Err(verification(
            "CSS expectation inventory changed during corpus checking",
        ));
    }
    Ok(())
}

fn invalid_inventory_from(source: GeneratorError) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::InvalidInventory,
        "validate current CSS corpus",
        source.to_string(),
        source,
    )
}

fn verification(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::Verification,
        "check current CSS corpus",
        detail,
    )
}
