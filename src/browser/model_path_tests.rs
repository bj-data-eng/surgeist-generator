use super::*;

fn corpus(outputs: &[&str], reports: &[&str], inputs: &[&str]) -> Result<BrowserCorpus> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let location = CorpusLocation::new(root, root).unwrap();
    let fixture = FixtureSpec::new(
        "fixture".to_owned(),
        RelativePath::new("html/fixture.html").unwrap(),
        outputs
            .iter()
            .enumerate()
            .map(|(index, output)| {
                CaseSpec::new(
                    format!("case-{index}"),
                    "default".to_owned(),
                    RelativePath::new(output).unwrap(),
                )
                .unwrap()
            })
            .collect(),
        FixtureStatus::Active,
    )
    .unwrap();
    BrowserCorpus::new(
        location,
        RelativePath::new("corpus.toml").unwrap(),
        "example-adapter".to_owned(),
        RelativePath::new("xml").unwrap(),
        reports
            .iter()
            .enumerate()
            .map(|(index, path)| {
                ReportScope::new(
                    RelativePath::new(path).unwrap(),
                    (index != 0).then(|| RelativePath::new("html").unwrap()),
                )
                .unwrap()
            })
            .collect(),
        BrowserSettings::new(
            "chrome-for-testing".to_owned(),
            "123.0.1".to_owned(),
            "Chrome for Testing 123.0.1".to_owned(),
            RelativePath::new("tmp/surgeist-browser").unwrap(),
            "Chrome {version} {repository_relative_executable}".to_owned(),
            BrowserLaunch::new(1, 1, 1, Vec::new()).unwrap(),
        )
        .unwrap(),
        vec![fixture],
        inputs
            .iter()
            .map(|path| RelativePath::new(path).unwrap())
            .collect(),
        Vec::new(),
    )
}

#[test]
fn case_report_and_ownership_files_cannot_be_declared_as_ancestors() {
    for (outputs, reports) in [
        (vec!["xml/a", "xml/a/b"], vec!["reports/all.json"]),
        (vec!["xml/a/b", "xml/a"], vec!["reports/all.json"]),
        (vec!["xml/a"], vec!["a/b"]),
        (vec!["xml/a/b"], vec!["a"]),
        (vec!["xml/case"], vec!["reports", "reports/all.json"]),
        (
            vec!["xml/generation-ownership.json/child"],
            vec!["reports/all.json"],
        ),
    ] {
        let error = corpus(&outputs, &reports, &[]).unwrap_err();
        assert_eq!(
            error.kind(),
            GeneratorErrorKind::InvalidInventory,
            "{outputs:?}, {reports:?}: {error}"
        );
    }
}

#[test]
fn declaration_paths_reject_file_and_directory_case_aliases() {
    for (outputs, reports, inputs) in [
        (
            vec!["xml/A.json", "xml/a.json"],
            vec!["reports/all.json"],
            vec![],
        ),
        (
            vec!["xml/A/one", "xml/a/two"],
            vec!["reports/all.json"],
            vec![],
        ),
        (vec!["xml/REPORTS/item"], vec!["reports/all.json"], vec![]),
        (
            vec!["xml/case"],
            vec!["reports/all.json", "Reports/grid.json"],
            vec![],
        ),
        (
            vec!["xml/case"],
            vec!["reports/all.json"],
            vec!["XML/helper.js"],
        ),
        (
            vec!["xml/case"],
            vec!["reports/all.json"],
            vec!["HTML/helper.js"],
        ),
        (
            vec!["xml/case"],
            vec!["reports/all.json"],
            vec!["CORPUS.toml"],
        ),
        (vec!["xml/case"], vec!["reports/all.json"], vec!["html"]),
    ] {
        let error = corpus(&outputs, &reports, &inputs).unwrap_err();
        assert_eq!(
            error.kind(),
            GeneratorErrorKind::InvalidInventory,
            "{outputs:?}, {reports:?}, {inputs:?}: {error}"
        );
    }
}

#[test]
fn reserved_components_are_rejected_in_all_declared_file_roles() {
    for component in [
        ".surgeist-generator",
        ".SURGEIST-GENERATOR",
        "._surgeist-stage",
        "._SURGEIST-stage",
    ] {
        let output = format!("xml/{component}/case");
        assert!(corpus(&[&output], &["reports/all.json"], &[]).is_err());
        let report = format!("reports/{component}/all.json");
        assert!(corpus(&["xml/case"], &[&report], &[]).is_err());
        let input = format!("helpers/{component}/helper.js");
        assert!(corpus(&["xml/case"], &["reports/all.json"], &[&input]).is_err());
    }
}

#[test]
fn exact_shared_input_and_consistently_spelled_sibling_directories_are_valid() {
    let valid = corpus(
        &["xml/a/b", "xml/a/c", "xml/a-b", "xml/Ångström/result"],
        &["reports/all.json", "reports/scoped.json"],
        &["helpers/one.js", "helpers/two.js", "corpus.toml"],
    )
    .unwrap();
    assert_eq!(valid.fixtures()[0].cases().len(), 4);
}
