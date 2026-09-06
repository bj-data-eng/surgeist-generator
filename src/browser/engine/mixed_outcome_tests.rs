use super::*;

const RESOURCE: &str = "resource.css";
const ARTIFACT: &[u8] = b"<mixed/>\n";

struct MixedOutcomes;

impl BrowserCorpusAdapter for MixedOutcomes {
    type Error = io::Error;

    fn prepare(&self, input: FixtureInput<'_>) -> std::result::Result<PreparedFixture, io::Error> {
        Adapter {
            resources: ResourceDependencies::Paths(vec![
                RelativePath::new(RESOURCE).expect("resource path"),
            ]),
            ..Adapter::generated()
        }
        .prepare(input)
    }

    fn lower(
        &self,
        fixture: &FixtureSpec,
        measurement: serde_json::Value,
    ) -> std::result::Result<Vec<CaseOutcome>, io::Error> {
        assert_eq!(measurement, serde_json::json!({"synthetic": true}));
        let [unsupported, generated] = fixture.cases() else {
            panic!("mixed fixture must declare two cases");
        };
        Ok(vec![
            CaseOutcome::Unsupported {
                case_id: unsupported.id().to_owned(),
                reason: "first variant is unsupported".to_owned(),
            },
            CaseOutcome::Generated {
                case_id: generated.id().to_owned(),
                bytes: ARTIFACT.to_vec(),
            },
        ])
    }
}

fn mixed_harness() -> Harness {
    let mut harness = Harness::new();
    let fixture = &harness.corpus.fixtures[0];
    let second = CaseSpec::new(
        "a__two".to_owned(),
        "two".to_owned(),
        RelativePath::new("xml/b.xml").expect("second output"),
    )
    .expect("second case");
    harness.corpus.fixtures = vec![
        FixtureSpec::new(
            fixture.name().to_owned(),
            fixture.source().clone(),
            vec![fixture.cases()[0].clone(), second],
            FixtureStatus::Active,
        )
        .expect("mixed fixture"),
    ];
    harness.corpus = harness
        .corpus
        .clone()
        .with_expected_counts(BrowserReportSummary::new(1, 1, 0, 0, 0).expect("mixed counts"));
    harness
}

#[test]
fn mixed_outcomes_publish_generated_case_with_complete_resource_provenance() {
    let harness = mixed_harness();
    let resource_bytes = b"body { color: green; }\n";
    fs::write(harness.root.join(RESOURCE), resource_bytes).expect("resource");
    let report = run_with_test_host(
        harness.request(),
        MixedOutcomes,
        TestGenerationHost::new(TestBrowserPlan::Success),
    )
    .expect("mixed generation");

    assert_eq!(
        report.summary(),
        &BrowserReportSummary::new(1, 1, 0, 0, 0).expect("mixed counts")
    );
    assert!(!harness.root.join("xml/a.xml").exists());
    assert_eq!(
        fs::read(harness.root.join("xml/b.xml")).expect("generated second case"),
        ARTIFACT
    );
    let generated = &report.generated()[0];
    assert_eq!(generated.name(), "a__two");
    assert_eq!(generated.variant(), "two");
    assert_eq!(generated.source().as_str(), "html/a.html");
    assert_eq!(
        generated.output_sha256(),
        &Sha256Digest::from_bytes(ARTIFACT)
    );
    assert_eq!(
        generated.linked_resources,
        BTreeMap::from([(
            RelativePath::new(RESOURCE).expect("resource path"),
            Sha256Digest::from_bytes(resource_bytes),
        )])
    );
    assert_eq!(report.unsupported[0].name, "a__one");
    assert_eq!(report.unsupported[0].variant.as_deref(), Some("one"));
    assert_eq!(report.unsupported[0].source.as_str(), "html/a.html");
    assert_eq!(
        check_corpus(&harness.corpus, &MixedOutcomes).expect("mixed corpus integrity"),
        report
    );
    fs::write(harness.root.join(RESOURCE), b"changed resource\n").expect("resource drift");
    assert!(check_corpus(&harness.corpus, &MixedOutcomes).is_err());
    harness.terminal();
}

#[test]
fn mixed_outcomes_require_resource_provenance_when_one_case_is_generated() {
    let harness = mixed_harness();
    let error = run_with_test_host(
        harness.request(),
        MixedOutcomes,
        TestGenerationHost::new(TestBrowserPlan::Success),
    )
    .expect_err("generated second case requires its declared resource");
    assert!(matches!(
        &error,
        GenerationError::Generator(error) if error.kind() == GeneratorErrorKind::InvalidInventory
    ));
    assert!(error.to_string().contains(RESOURCE));
    assert!(!harness.root.join("xml/a.xml").exists());
    assert!(!harness.root.join("xml/b.xml").exists());
    let report = BrowserGenerationReport::parse(
        &fs::read(harness.root.join("xml/reports/all.json")).expect("diagnostic report"),
    )
    .expect("canonical diagnostic report");
    assert_eq!(report.summary().generated(), 0);
    assert_eq!(report.summary().failed_to_generate(), 1);
    assert_eq!(report.failed_to_generate[0].source.as_str(), "html/a.html");
    assert!(check_corpus(&harness.corpus, &MixedOutcomes).is_err());
    harness.terminal();
}
