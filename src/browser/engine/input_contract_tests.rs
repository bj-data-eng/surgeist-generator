use super::*;

#[test]
fn exact_helper_roots_reject_extra_files_and_closing_inventory_drift() {
    let mut harness = Harness::new();
    fs::create_dir(harness.root.join("helpers")).expect("helper directory");
    fs::rename(
        harness.root.join("helper.js"),
        harness.root.join("helpers/helper.js"),
    )
    .expect("move helper");
    harness.corpus.global_inputs = vec![RelativePath::new("helpers/helper.js").expect("path")];
    harness.corpus = harness
        .corpus
        .clone()
        .with_exact_input_roots(vec![RelativePath::new("helpers").expect("root")])
        .expect("exact input root");
    let rooted = RootedFs::open_corpus(harness.corpus.location()).expect("rooted corpus");
    let captured =
        capture_inputs(&rooted, &harness.corpus).expect("capture exact helper inventory");
    fs::write(
        harness.root.join("helpers/extra.js"),
        b"unexpected helper\n",
    )
    .expect("extra helper");
    assert!(capture_inputs(&rooted, &harness.corpus).is_err());
    assert!(captured.revalidate(&rooted).is_err());
    for roots in [
        vec!["helpers", "helpers"],
        vec!["helpers", "helpers/nested"],
        vec!["xml"],
        vec![".surgeist-generator"],
    ] {
        let roots = roots
            .into_iter()
            .map(|path| RelativePath::new(path).expect("path"))
            .collect();
        assert!(
            harness
                .corpus
                .clone()
                .with_exact_input_roots(roots)
                .is_err()
        );
    }
}

#[test]
fn expected_fail_is_measured_and_annotated() {
    let mut harness = Harness::new();
    let old = &harness.corpus.fixtures[0];
    harness.corpus.fixtures = vec![
        FixtureSpec::new(
            old.name().to_owned(),
            old.source().clone(),
            old.cases().to_vec(),
            FixtureStatus::ExpectedFail {
                reason: "known parity difference".to_owned(),
            },
        )
        .expect("expected fail fixture"),
    ];
    let report = harness
        .run(
            harness.request(),
            Adapter::generated(),
            TestBrowserPlan::Success,
        )
        .expect("expected fail generation");
    assert_eq!(report.summary().generated(), 1);
    assert_eq!(report.summary().expected_fail(), 1);
    assert!(harness.root.join("xml/a.xml").is_file());
    check_corpus(&harness.corpus, &Adapter::generated())
        .expect("annotated generated case integrity");
}

fn rewrite_report_ownership(harness: &Harness, report: &BrowserGenerationReport) {
    let report_path = harness.root.join("xml/reports/all.json");
    let bytes = report.to_bytes().expect("canonical report");
    fs::write(&report_path, &bytes).expect("rewrite report");
    let ownership_path = harness.root.join("xml/generation-ownership.json");
    let mut ownership = Ownership::parse(&fs::read(&ownership_path).expect("ownership"))
        .expect("canonical ownership");
    ownership.artifacts.insert(
        RelativePath::new("xml/reports/all.json").expect("report path"),
        Sha256Digest::from_bytes(bytes),
    );
    fs::write(
        ownership_path,
        ownership.bytes().expect("canonical ownership"),
    )
    .expect("rewrite ownership");
}

#[test]
fn offline_check_recomputes_resources_even_when_reports_and_ownership_are_rehashed() {
    let harness = Harness::new();
    fs::write(harness.root.join("resource.css"), b"old resource\n").expect("resource");
    let adapter = Adapter {
        resources: ResourceDependencies::Paths(vec![
            RelativePath::new("resource.css").expect("path"),
        ]),
        ..Adapter::generated()
    };
    let mut report = harness
        .run(harness.request(), adapter.clone(), TestBrowserPlan::Success)
        .expect("generation");
    report.generated[0].linked_resources.clear();
    rewrite_report_ownership(&harness, &report);
    fs::write(harness.root.join("resource.css"), b"changed resource\n").expect("resource drift");
    assert!(
        check_corpus(&harness.corpus, &adapter).is_err(),
        "reported resource omission must not weaken checking"
    );
}

#[test]
fn offline_check_accepts_a_relocated_corpus_without_the_historical_host_or_browser() {
    let mut harness = Harness::new();
    harness
        .run(
            harness.request(),
            Adapter::generated(),
            TestBrowserPlan::Success,
        )
        .expect("generation");
    let relocated = harness.root.with_extension("relocated");
    fs::rename(&harness.root, &relocated).expect("relocate corpus");
    harness.root = relocated;
    // A cold checkout carries committed corpus artifacts, not the ignored
    // coordination records whose ownership is intentionally bound to one root.
    fs::remove_dir_all(harness.root.join(".surgeist-generator"))
        .expect("omit local coordination from cold clone");
    harness.corpus.location =
        crate::CorpusLocation::new(&harness.root, &harness.root).expect("relocated root");
    harness.corpus.browser_owner = harness.root.clone();
    check_corpus(&harness.corpus, &Adapter::generated())
        .expect("portable historical job and browser provenance");
}

#[test]
fn output_ownership_drift_blocks_repair_without_rewriting_artifacts() {
    let harness = Harness::new();
    harness
        .run(
            harness.request(),
            Adapter::generated(),
            TestBrowserPlan::Success,
        )
        .expect("generation");
    fs::write(harness.root.join("xml/a.xml"), b"tampered\n").expect("artifact drift");
    assert!(check_corpus(&harness.corpus, &Adapter::generated()).is_err());
    assert!(
        harness
            .run(
                harness.request(),
                Adapter::generated(),
                TestBrowserPlan::Success
            )
            .is_err()
    );
    assert_eq!(
        fs::read(harness.root.join("xml/a.xml")).expect("artifact"),
        b"tampered\n"
    );
}

#[test]
fn legacy_adoption_protects_evidence_outside_output_root() {
    let harness = Harness::new();
    fs::create_dir(harness.root.join("xml")).expect("output root");
    fs::write(harness.root.join("xml/legacy.json"), b"legacy report\n").expect("legacy report");
    fs::write(harness.root.join("xml/a.xml"), b"legacy artifact\n").expect("legacy artifact");
    let digest = |path: &str| {
        Sha256Digest::from_bytes(fs::read(harness.root.join(path)).expect("legacy evidence"))
    };
    let prior = super::super::super::PriorOwnership::new(
        vec![
            (
                RelativePath::new("corpus.toml").expect("manifest"),
                digest("corpus.toml"),
            ),
            (
                RelativePath::new("xml/legacy.json").expect("report"),
                digest("xml/legacy.json"),
            ),
        ],
        vec![(
            RelativePath::new("xml/a.xml").expect("artifact"),
            digest("xml/a.xml"),
        )],
    )
    .expect("prior ownership");
    let request = GenerationRequest::new(
        harness.corpus.clone(),
        harness.browser_path.clone(),
        None,
        Some(prior),
    )
    .expect("adoption request");
    harness
        .run(request, Adapter::generated(), TestBrowserPlan::Success)
        .expect("adopt legacy corpus");
    assert!(harness.root.join("corpus.toml").is_file());
    assert!(!harness.root.join("xml/legacy.json").exists());
    check_corpus(&harness.corpus, &Adapter::generated()).expect("adopted integrity");
}

#[test]
fn clean_check_rejects_filtered_outputs_outside_the_restored_full_contract() {
    let mut harness = Harness::new();
    harness
        .run(
            harness.request(),
            Adapter::generated(),
            TestBrowserPlan::Success,
        )
        .expect("initial full generation");
    let original = harness.corpus.clone();
    let manifest = fs::read(harness.root.join("corpus.toml")).expect("original manifest");
    let full_report = fs::read(harness.root.join("xml/reports/all.json")).expect("full report");
    let fixture = &harness.corpus.fixtures[0];
    harness.corpus.fixtures = vec![
        FixtureSpec::new(
            fixture.name().to_owned(),
            fixture.source().clone(),
            vec![
                CaseSpec::new(
                    fixture.cases()[0].id().to_owned(),
                    fixture.cases()[0].variant().to_owned(),
                    RelativePath::new("xml/new.xml").expect("temporary output"),
                )
                .expect("temporary case"),
            ],
            fixture.status().clone(),
        )
        .expect("temporary fixture"),
    ];
    fs::write(
        harness.root.join("corpus.toml"),
        b"temporary_output = true\n",
    )
    .expect("temporary manifest contract");
    let request = GenerationRequest::new(
        harness.corpus.clone(),
        harness.browser_path.clone(),
        Some(RelativePath::new("html/a.html").expect("filter")),
        None,
    )
    .expect("filtered request");
    harness
        .run(request, Adapter::generated(), TestBrowserPlan::Success)
        .expect("filtered generation with new output");
    assert!(harness.root.join("xml/a.xml").is_file());
    assert!(harness.root.join("xml/new.xml").is_file());
    assert_eq!(
        fs::read(harness.root.join("xml/reports/all.json")).unwrap(),
        full_report
    );

    harness.corpus = original;
    fs::write(harness.root.join("corpus.toml"), manifest).expect("restore original contract");
    let error = check_corpus(&harness.corpus, &Adapter::generated())
        .expect_err("repair ownership must not admit an unreported output in a clean check");
    assert!(error.to_string().contains("new.xml"), "{error}");
    assert_eq!(
        fs::read(harness.root.join("xml/new.xml")).unwrap(),
        b"<test/>\n"
    );

    harness
        .run(
            harness.request(),
            Adapter::generated(),
            TestBrowserPlan::Success,
        )
        .expect("full generation repairs broader ledger ownership");
    assert!(!harness.root.join("xml/new.xml").exists());
    check_corpus(&harness.corpus, &Adapter::generated()).expect("clean repaired corpus");
    harness.terminal();
}
