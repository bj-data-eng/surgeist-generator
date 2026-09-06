//! Browser retry behavior exercised through the generation path and owned supervisor.

use super::*;

fn run_traced(
    harness: &Harness,
    plan: TestBrowserPlan,
) -> (
    std::result::Result<BrowserGenerationReport, GenerationError<io::Error>>,
    TestGenerationHost,
) {
    let trace = TestGenerationHost::new(plan);
    let host = GenerationHost {
        executable: harness.executable.clone(),
        digest: Sha256Digest::from_bytes(fs::read(&harness.executable).expect("host bytes")),
        execution: BrowserExecution::Test(trace.clone()),
    };
    (
        run_with_host(harness.request(), Adapter::generated(), host),
        trace,
    )
}

#[test]
fn helper_measurement_and_close_failures_are_not_retried() {
    for plan in [
        TestBrowserPlan::HelperFailure,
        TestBrowserPlan::MeasurementFailure,
        TestBrowserPlan::CloseFailure,
    ] {
        let harness = Harness::new();
        let (result, trace) = run_traced(&harness, plan);
        assert!(
            result.is_err(),
            "a terminal page failure must fail generation"
        );
        assert_eq!(
            trace.attempts(),
            [(0, 0)],
            "{plan:?} must not rerun the fixture"
        );
        harness.terminal();
    }
}

#[test]
fn each_timed_out_fixture_uses_its_own_fresh_retry_browser() {
    let mut harness = Harness::new();
    fs::write(harness.root.join("html/b.html"), b"<div>A</div>\n").expect("second fixture source");
    harness.corpus.fixtures.push(
        FixtureSpec::new(
            "b".to_owned(),
            RelativePath::new("html/b.html").expect("source"),
            vec![
                CaseSpec::new(
                    "b__one".to_owned(),
                    "one".to_owned(),
                    RelativePath::new("xml/b.xml").expect("output"),
                )
                .expect("case"),
            ],
            FixtureStatus::Active,
        )
        .expect("fixture"),
    );
    let (result, trace) = run_traced(&harness, TestBrowserPlan::RetryOnce);
    assert_eq!(
        result
            .expect("both timeouts should recover")
            .summary()
            .generated(),
        2
    );
    let attempts = trace
        .fixture_attempts()
        .iter()
        .map(|sources| {
            sources
                .iter()
                .map(|source| source.as_str().to_owned())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        attempts,
        [
            vec!["html/a.html", "html/b.html"],
            vec!["html/a.html"],
            vec!["html/b.html"]
        ]
    );
    harness.terminal();
}
