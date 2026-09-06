use super::*;
use std::error::Error;
use std::fmt;

#[derive(Debug)]
struct AdapterRejection {
    display: &'static str,
    cause: io::Error,
}
impl fmt::Display for AdapterRejection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.display)
    }
}
impl Error for AdapterRejection {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.cause)
    }
}
struct RejectingAdapter {
    stage: &'static str,
    display: &'static str,
}
impl RejectingAdapter {
    fn rejection(&self) -> AdapterRejection {
        AdapterRejection {
            display: self.display,
            cause: io::Error::other("adapter root cause"),
        }
    }
}
impl BrowserCorpusAdapter for RejectingAdapter {
    type Error = AdapterRejection;
    fn prepare(
        &self,
        input: FixtureInput<'_>,
    ) -> std::result::Result<PreparedFixture, Self::Error> {
        if self.stage == "prepare" {
            return Err(self.rejection());
        }
        Ok(Adapter::generated()
            .prepare(input)
            .expect("prepared fixture"))
    }
    fn lower(
        &self,
        _fixture: &FixtureSpec,
        _measurement: serde_json::Value,
    ) -> std::result::Result<Vec<CaseOutcome>, Self::Error> {
        Err(self.rejection())
    }
}

#[test]
fn arbitrary_adapter_error_display_preserves_typed_source_and_diagnostic_accounting() {
    for stage in ["prepare", "lower"] {
        for display in ["adapter failure\n", " \n"] {
            let harness = Harness::new();
            let error = run_with_test_host(
                harness.request(),
                RejectingAdapter { stage, display },
                TestGenerationHost::new(TestBrowserPlan::Success),
            )
            .expect_err("adapter rejection");
            match error {
                GenerationError::Adapter {
                    source,
                    stage: actual_stage,
                    error,
                } => {
                    assert_eq!(source.as_str(), "html/a.html");
                    assert_eq!(actual_stage, stage);
                    assert_eq!(error.to_string(), display);
                    assert_eq!(
                        error.source().expect("original cause").to_string(),
                        "adapter root cause"
                    );
                }
                error => {
                    panic!("adapter identity must survive diagnostic serialization: {error:?}")
                }
            }
            let report: serde_json::Value = serde_json::from_slice(
                &fs::read(harness.root.join("xml/reports/all.json")).expect("diagnostic report"),
            )
            .expect("report JSON");
            assert_eq!(report["summary"]["failed_to_generate"], 1);
            assert_eq!(report["summary"]["generated"], 0);
            let reason = report["failed_to_generate"][0]["reason"]
                .as_str()
                .expect("failure reason");
            assert!(!reason.is_empty());
            assert_eq!(reason, reason.trim());
            harness.terminal();
        }
    }
}
