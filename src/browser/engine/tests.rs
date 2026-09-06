use std::fs;
use std::io;
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;
use crate::browser::measurement::{TestBrowserPlan, TestGenerationHost};
use crate::browser::{
    BrowserLaunch, BrowserLocation, BrowserReportSummary, BrowserSettings, CaseSpec, ReportScope,
};

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Harness {
    root: PathBuf,
    corpus: BrowserCorpus,
    browser_path: RelativePath,
    executable: PathBuf,
}
impl Harness {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "surgeist-browser-engine-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("create corpus");
        let executable = std::env::current_exe()
            .and_then(fs::canonicalize)
            .expect("test executable");
        let cache = executable.parent().expect("executable parent");
        let owner = cache.parent().expect("cache owner");
        let browser_path = RelativePath::new(
            executable
                .strip_prefix(owner)
                .expect("owner-relative executable")
                .to_str()
                .expect("UTF-8 test path"),
        )
        .expect("browser path");
        let browser = BrowserSettings::new(
            "synthetic-browser".to_owned(),
            "1".to_owned(),
            "synthetic 1".to_owned(),
            RelativePath::new(
                cache
                    .file_name()
                    .expect("cache name")
                    .to_str()
                    .expect("UTF-8 cache"),
            )
            .expect("cache path"),
            "browser {version} {repository_relative_executable}".to_owned(),
            BrowserLaunch::new(2, 5000, 10, Vec::new()).expect("launch"),
        )
        .expect("browser settings");
        let source = RelativePath::new("html/a.html").expect("source");
        let case = CaseSpec::new(
            "a__one".to_owned(),
            "one".to_owned(),
            RelativePath::new("xml/a.xml").expect("output"),
        )
        .expect("case");
        let fixture = FixtureSpec::new("a".to_owned(), source, vec![case], FixtureStatus::Active)
            .expect("fixture");
        fs::create_dir(root.join("html")).expect("source directory");
        fs::write(root.join("html/a.html"), b"<div>A</div>\n").expect("source bytes");
        fs::write(root.join("corpus.toml"), b"test = true\n").expect("manifest bytes");
        fs::write(root.join("helper.js"), b"true\n").expect("helper bytes");
        let corpus = BrowserCorpus::new(
            BrowserLocation::new(owner, &root).expect("independent roots"),
            RelativePath::new("corpus.toml").expect("manifest"),
            "consumer-host".to_owned(),
            RelativePath::new("xml").expect("output root"),
            vec![
                ReportScope::new(
                    RelativePath::new("reports/all.json").expect("report path"),
                    None,
                )
                .expect("scope"),
            ],
            browser,
            vec![fixture],
            vec![RelativePath::new("helper.js").expect("helper")],
            Vec::new(),
        )
        .expect("corpus");
        Self {
            root,
            corpus,
            browser_path,
            executable,
        }
    }
    fn request(&self) -> GenerationRequest {
        GenerationRequest::new(self.corpus.clone(), self.browser_path.clone(), None, None)
            .expect("request")
    }
    fn run(
        &self,
        request: GenerationRequest,
        adapter: Adapter,
        plan: TestBrowserPlan,
    ) -> std::result::Result<BrowserGenerationReport, GenerationError<io::Error>> {
        let host = GenerationHost {
            executable: self.executable.clone(),
            digest: Sha256Digest::from_bytes(fs::read(&self.executable).expect("host bytes")),
            execution: BrowserExecution::Test(TestGenerationHost::new(plan)),
        };
        run_with_host(request, adapter, host)
    }
    fn terminal(&self) {
        let profiles = self.root.join(super::super::profile::PROFILE_PARENT);
        assert!(
            !profiles.exists() || fs::read_dir(profiles).expect("profiles").next().is_none(),
            "browser profiles must be terminal before results return"
        );
    }
}
impl Drop for Harness {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[derive(Clone)]
struct Adapter {
    outcome: Mode,
    resources: ResourceDependencies,
    mutate: Option<PathBuf>,
}
#[derive(Clone, Copy)]
enum Mode {
    Generated,
    Changed,
    Unsupported,
    Missing,
    Duplicate,
    LowerError,
}
impl Adapter {
    fn generated() -> Self {
        Self {
            outcome: Mode::Generated,
            resources: ResourceDependencies::Paths(Vec::new()),
            mutate: None,
        }
    }
}
impl BrowserCorpusAdapter for Adapter {
    type Error = io::Error;
    fn prepare(&self, input: FixtureInput<'_>) -> std::result::Result<PreparedFixture, io::Error> {
        assert_eq!(
            input.input_bytes(&RelativePath::new("helper.js").expect("path")),
            Some(b"true\n".as_slice())
        );
        assert_eq!(input.source_bytes(), b"<div>A</div>\n");
        PreparedFixture::new(
            format!("<base href={:?}><div>A</div>", input.base_url()),
            "true".to_owned(),
            String::new(),
            "{}".to_owned(),
            self.resources.clone(),
        )
        .map_err(io::Error::other)
    }
    fn lower(
        &self,
        fixture: &FixtureSpec,
        measurement: serde_json::Value,
    ) -> std::result::Result<Vec<CaseOutcome>, io::Error> {
        assert_eq!(measurement, serde_json::json!({"synthetic":true}));
        if let Some(path) = &self.mutate {
            fs::write(path, b"changed\n")?;
        }
        let generated = || CaseOutcome::Generated {
            case_id: fixture.cases()[0].id().to_owned(),
            bytes: b"<test/>\n".to_vec(),
        };
        Ok(match self.outcome {
            Mode::Generated => vec![generated()],
            Mode::Changed => vec![CaseOutcome::Generated {
                case_id: fixture.cases()[0].id().to_owned(),
                bytes: b"<changed/>\n".to_vec(),
            }],
            Mode::Unsupported => vec![CaseOutcome::Unsupported {
                case_id: fixture.cases()[0].id().to_owned(),
                reason: "unsupported fixture".to_owned(),
            }],
            Mode::Missing => Vec::new(),
            Mode::Duplicate => vec![generated(), generated()],
            Mode::LowerError => return Err(io::Error::other("typed adapter rejection")),
        })
    }
}

#[test]
fn downstream_adapter_publishes_opaque_bytes_and_offline_report() {
    let harness = Harness::new();
    let report = harness
        .run(
            harness.request(),
            Adapter::generated(),
            TestBrowserPlan::Success,
        )
        .expect("generate");
    assert_eq!(report.summary().generated(), 1);
    assert_eq!(
        fs::read(harness.root.join("xml/a.xml")).expect("artifact"),
        b"<test/>\n"
    );
    assert_eq!(
        check_corpus(&harness.corpus, &Adapter::generated()).expect("offline integrity"),
        report
    );
    harness.terminal();
}

#[test]
fn unsupported_fixture_does_not_require_missing_resource_provenance() {
    let harness = Harness::new();
    let adapter = Adapter {
        outcome: Mode::Unsupported,
        resources: ResourceDependencies::Paths(vec![
            RelativePath::new("missing.css").expect("path"),
        ]),
        mutate: None,
    };
    let report = harness
        .run(harness.request(), adapter, TestBrowserPlan::Success)
        .expect("unsupported result");
    assert_eq!(report.summary().generated(), 0);
    assert_eq!(report.summary().unsupported(), 1);
    assert!(!harness.root.join("xml/a.xml").exists());
    check_corpus(&harness.corpus, &Adapter::generated()).expect("unsupported corpus integrity");
    harness.terminal();
}

#[test]
fn generated_fixture_rejects_missing_or_changed_resources_before_publication() {
    for changed in [false, true] {
        let harness = Harness::new();
        let resource = harness.root.join("resource.css");
        if changed {
            fs::write(&resource, b"original\n").expect("resource");
        }
        let adapter = Adapter {
            resources: ResourceDependencies::Paths(vec![
                RelativePath::new("resource.css").expect("path"),
            ]),
            mutate: changed.then_some(resource),
            ..Adapter::generated()
        };
        assert!(
            harness
                .run(harness.request(), adapter, TestBrowserPlan::Success)
                .is_err()
        );
        if changed {
            assert!(!harness.root.join("xml").exists());
        } else {
            assert!(!harness.root.join("xml/a.xml").exists());
            assert!(check_corpus(&harness.corpus, &Adapter::generated()).is_err());
        }
        harness.terminal();
    }
}

#[test]
fn adapter_missing_duplicate_and_typed_failure_leave_artifacts_unchanged() {
    for mode in [Mode::Missing, Mode::Duplicate, Mode::LowerError] {
        let harness = Harness::new();
        let adapter = Adapter {
            outcome: mode,
            ..Adapter::generated()
        };
        let error = harness
            .run(harness.request(), adapter, TestBrowserPlan::Success)
            .expect_err("invalid adapter result");
        if matches!(mode, Mode::LowerError) {
            assert!(matches!(
                error,
                GenerationError::Adapter { stage: "lower", .. }
            ));
        }
        if matches!(mode, Mode::LowerError) {
            assert!(!harness.root.join("xml/a.xml").exists());
        } else {
            assert!(!harness.root.join("xml").exists());
        }
        harness.terminal();
    }
}

#[test]
fn expected_accounting_mismatch_is_rejected_before_publication() {
    let harness = Harness::new();
    let corpus = harness
        .corpus
        .clone()
        .with_expected_counts(BrowserReportSummary::new(2, 0, 0, 0, 0).expect("counts"));
    let request =
        GenerationRequest::new(corpus, harness.browser_path.clone(), None, None).expect("request");
    assert!(
        harness
            .run(request, Adapter::generated(), TestBrowserPlan::Success)
            .is_err()
    );
    assert!(!harness.root.join("xml").exists());
    harness.terminal();
}

#[test]
fn retries_terminalize_each_attempt_and_exhaustion_publishes_diagnostics() {
    let harness = Harness::new();
    harness
        .run(
            harness.request(),
            Adapter::generated(),
            TestBrowserPlan::RetryOnce,
        )
        .expect("retry succeeds");
    harness.terminal();
    assert!(
        harness
            .run(
                harness.request(),
                Adapter::generated(),
                TestBrowserPlan::AlwaysFail
            )
            .is_err()
    );
    harness.terminal();
    assert!(
        check_corpus(&harness.corpus, &Adapter::generated()).is_err(),
        "diagnostic report cannot pass clean integrity"
    );
    harness
        .run(
            harness.request(),
            Adapter::generated(),
            TestBrowserPlan::Success,
        )
        .expect("full generation repairs diagnostic retained ownership");
    assert_eq!(
        fs::read(harness.root.join("xml/a.xml")).expect("prior artifact retained"),
        b"<test/>\n"
    );
}

#[test]
fn filtered_generation_preserves_reports_and_other_outputs() {
    let harness = Harness::new();
    harness
        .run(
            harness.request(),
            Adapter::generated(),
            TestBrowserPlan::Success,
        )
        .expect("initial full");
    let report = fs::read(harness.root.join("xml/reports/all.json")).expect("full report");
    let request = GenerationRequest::new(
        harness.corpus.clone(),
        harness.browser_path.clone(),
        Some(RelativePath::new("html/a.html").expect("filter")),
        None,
    )
    .expect("filtered request");
    harness
        .run(
            request,
            Adapter {
                outcome: Mode::Changed,
                ..Adapter::generated()
            },
            TestBrowserPlan::Success,
        )
        .expect("filtered generation");
    assert_eq!(
        fs::read(harness.root.join("xml/reports/all.json")).expect("retained report"),
        report
    );
    assert!(
        check_corpus(&harness.corpus, &Adapter::generated()).is_err(),
        "filtered changed artifacts make the historical full report stale"
    );
    harness
        .run(
            harness.request(),
            Adapter::generated(),
            TestBrowserPlan::Success,
        )
        .expect("full generation repairs filtered ownership");
    check_corpus(&harness.corpus, &Adapter::generated()).expect("repaired full integrity");
    harness.terminal();
}

#[path = "retry_contract_tests.rs"]
mod retry_contract_tests;

#[test]
fn declarations_reject_manifest_attestation_and_import_drift_before_generation() {
    for path in ["corpus.toml", "html/.source.json", "html/a.html"] {
        let mut harness = Harness::new();
        fs::write(
            harness.root.join("html/.source.json"),
            b"verified attestation\n",
        )
        .expect("attestation");
        harness
            .corpus
            .import_attestations
            .push(RelativePath::new("html/.source.json").expect("path"));
        let path = RelativePath::new(path).expect("path");
        let digest =
            Sha256Digest::from_bytes(fs::read(path.join(&harness.root)).expect("verified input"));
        harness.corpus = harness
            .corpus
            .clone()
            .with_expected_inputs(BTreeMap::from([(path.clone(), digest)]))
            .expect("verified input binding")
            .with_expected_inputs(BTreeMap::new())
            .expect("additional bindings preserve the earlier verification witness");
        fs::write(path.join(&harness.root), b"changed after verification\n").expect("mutate input");
        let rooted = RootedFs::open_corpus(harness.corpus.location()).expect("rooted corpus");
        assert!(
            capture_inputs(&rooted, &harness.corpus).is_err(),
            "verified {path:?} drift must be rejected"
        );
        assert!(!harness.root.join("xml").exists());
    }
}

#[path = "input_contract_tests.rs"]
mod input_contract_tests;

#[path = "adapter_error_tests.rs"]
mod adapter_error_tests;

#[path = "mixed_outcome_tests.rs"]
mod mixed_outcome_tests;
