use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::core::{
    Domain, DurabilityEvent, DurabilityPhase, DurabilityPrimitive, GenerationLease,
    PRIVATE_FILE_MODE, RootedFs, RootedObserver,
};
use crate::{CorpusLocation, GeneratorError, GeneratorErrorKind, RelativePath, RunScope};

use super::browser_runtime::TrustedBrowser;
use super::measurement::{TestBrowserPlan, TestGenerationHost};
use super::model::EngineManifest;
use super::profile::{
    OwnedSupervisorChild, PROFILE_PARENT, ProfileAttempt, ProfileCreateContext, ProfileJournal,
    SUPERVISOR_EXIT_BOUND, SupervisorTermination, classify_pending, force_kill_group,
    resolve_terminalization, test_cleanup_path, test_group_is_dead, test_validate_journal_name,
};
use super::supervisor::TestBrowserMode;
use super::{
    BrowserCorpus, BrowserCorpusAdapter, BrowserLaunch, BrowserSettings, CaseOutcome, CaseSpec,
    FixtureInput, FixtureSpec, FixtureStatus, GenerationError, GenerationRequest, PreparedFixture,
    ReportScope, ResourceDependencies, engine, supervisor,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
const FIXTURE_BYTES: &[u8] = b"<div>fixture</div>\n";
const HELPER_BYTES: &[u8] = b"globalThis.surgeistLayoutHelper = true;\n";
const BASE_STYLE_BYTES: &[u8] = b"* { box-sizing: border-box; }\n";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(parent: &Path) -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = parent.join(format!(
            "surgeist-layout-generation-test-{}-{sequence:016x}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create generation test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove generation test directory");
    }
}

struct GenerationHarness {
    _temporary: TestDirectory,
    location: CorpusLocation,
    browser_path: RelativePath,
    executable: PathBuf,
    configuration: EngineManifest,
    corpus_spec: BrowserCorpus,
}
struct OpaqueAdapter;
impl BrowserCorpusAdapter for OpaqueAdapter {
    type Error = std::io::Error;
    fn prepare(
        &self,
        input: FixtureInput<'_>,
    ) -> std::result::Result<PreparedFixture, Self::Error> {
        PreparedFixture::new(
            String::from_utf8_lossy(input.source_bytes()).into_owned(),
            "true".to_owned(),
            String::new(),
            "{}".to_owned(),
            ResourceDependencies::Paths(Vec::new()),
        )
        .map_err(std::io::Error::other)
    }
    fn lower(
        &self,
        fixture: &FixtureSpec,
        _: serde_json::Value,
    ) -> std::result::Result<Vec<CaseOutcome>, Self::Error> {
        Ok(fixture
            .cases()
            .iter()
            .map(|case| CaseOutcome::Generated {
                case_id: case.id().to_owned(),
                bytes: b"<test/>\n".to_vec(),
            })
            .collect())
    }
}
impl GenerationHarness {
    fn new() -> Self {
        let executable = std::env::current_exe()
            .and_then(fs::canonicalize)
            .expect("current executable");
        let cache = executable.parent().expect("cache");
        let owner = cache.parent().expect("owner");
        let temporary = TestDirectory::new(owner);
        let corpus = temporary.path().join("corpus");
        fs::create_dir(&corpus).expect("corpus");
        let location = CorpusLocation::new(owner, &corpus).expect("location");
        let browser_path =
            RelativePath::new(path_relative_to(owner, &executable)).expect("browser");
        let settings = BrowserSettings::new(
            "synthetic".to_owned(),
            "1".to_owned(),
            "synthetic 1".to_owned(),
            RelativePath::new(path_relative_to(owner, cache)).expect("cache"),
            "browser {version} {repository_relative_executable}".to_owned(),
            BrowserLaunch::new(16, 1000, 10, Vec::new()).expect("launch"),
        )
        .expect("settings");
        let configuration =
            EngineManifest::new(settings.clone(), owner.to_path_buf()).expect("configuration");
        write(&corpus.join("corpus.toml"), b"synthetic=true\n");
        write(&corpus.join("scripts/gentest/test_helper.js"), HELPER_BYTES);
        write(
            &corpus.join("scripts/gentest/test_base_style.css"),
            BASE_STYLE_BYTES,
        );
        write(&corpus.join("html/grid/basic.html"), FIXTURE_BYTES);
        let cases = [
            "border_box_ltr",
            "border_box_rtl",
            "content_box_ltr",
            "content_box_rtl",
        ]
        .into_iter()
        .map(|variant| {
            CaseSpec::new(
                format!("basic__{variant}"),
                variant.to_owned(),
                RelativePath::new(format!("xml/grid/basic__{variant}.xml")).expect("output"),
            )
            .expect("case")
        })
        .collect();
        let fixture = FixtureSpec::new(
            "basic".to_owned(),
            RelativePath::new("html/grid/basic.html").expect("source"),
            cases,
            FixtureStatus::Active,
        )
        .expect("fixture");
        let corpus_spec = BrowserCorpus::new(
            location.clone(),
            RelativePath::new("corpus.toml").expect("manifest"),
            "consumer-host".to_owned(),
            RelativePath::new("xml").expect("output"),
            vec![
                ReportScope::new(
                    RelativePath::new("generation-reports/all.json").expect("report"),
                    None,
                )
                .expect("scope"),
            ],
            settings,
            vec![fixture],
            vec![
                RelativePath::new("scripts/gentest/test_helper.js").expect("helper"),
                RelativePath::new("scripts/gentest/test_base_style.css").expect("style"),
            ],
            Vec::new(),
        )
        .expect("corpus contract");
        Self {
            _temporary: temporary,
            location,
            browser_path,
            executable,
            configuration,
            corpus_spec,
        }
    }
    fn corpus(&self) -> &Path {
        self.location.corpus_root()
    }
    fn request(&self, filter: Option<&str>) -> GenerationRequest {
        GenerationRequest::new(
            self.corpus_spec.clone(),
            self.browser_path.clone(),
            filter.map(|value| RelativePath::new(format!("html/{value}")).expect("filter")),
            None,
        )
        .expect("request")
    }
    fn run(&self, plan: TestBrowserPlan) -> (crate::Result<()>, TestGenerationHost) {
        self.run_request(self.request(None), plan)
    }
    fn run_filtered(
        &self,
        filter: &str,
        plan: TestBrowserPlan,
    ) -> (crate::Result<()>, TestGenerationHost) {
        self.run_request(self.request(Some(filter)), plan)
    }
    fn run_request(
        &self,
        request: GenerationRequest,
        plan: TestBrowserPlan,
    ) -> (crate::Result<()>, TestGenerationHost) {
        let host = TestGenerationHost::new(plan);
        let result = engine::run_with_test_host(request, OpaqueAdapter, host.clone())
            .map(|_| ())
            .map_err(|error| match error {
                GenerationError::Generator(error) => error,
                GenerationError::Adapter { error, .. } => GeneratorError::with_source(
                    GeneratorErrorKind::Generation,
                    "synthetic adapter",
                    error.to_string(),
                    error,
                ),
            });
        (result, host)
    }
    fn check(&self) -> crate::Result<()> {
        engine::check_corpus(&self.corpus_spec, &OpaqueAdapter)
            .map(|_| ())
            .map_err(|error| match error {
                GenerationError::Generator(error) => error,
                GenerationError::Adapter { error, .. } => GeneratorError::with_source(
                    GeneratorErrorKind::Generation,
                    "synthetic adapter",
                    error.to_string(),
                    error,
                ),
            })
    }
    fn rooted(&self) -> RootedFs {
        RootedFs::open_corpus(&self.location).expect("open generation corpus")
    }

    fn observed_rooted(&self, observer: RootedObserver) -> RootedFs {
        RootedFs::open_corpus_observed(&self.location, observer)
            .expect("open observed generation corpus")
    }

    fn lease(&self) -> GenerationLease {
        GenerationLease::acquire_for_test(
            &self.location,
            Domain::Browser,
            "consumer-host",
            &RunScope::Full,
            "generate",
        )
        .expect("layout generation lease")
    }

    fn parsed_manifest(&self) -> EngineManifest {
        self.configuration.clone()
    }

    fn trusted_browser(&self, manifest: &EngineManifest) -> TrustedBrowser {
        TrustedBrowser::validate(&self.location, manifest, &self.browser_path)
            .expect("trusted current test executable")
    }

    fn version_journal(&self, lease: &GenerationLease) -> ProfileJournal {
        let manifest = self.parsed_manifest();
        let browser = self.trusted_browser(&manifest);
        ProfileJournal::create(
            ProfileCreateContext {
                location: &self.location,
                lease,
                browser: &browser,
                manifest: &manifest,
            },
            ProfileAttempt::Version {
                launch_strings: vec!["version".to_owned()],
            },
        )
        .expect("create real version profile journal")
    }

    fn observed_version_journal(
        &self,
        lease: &GenerationLease,
        rooted: &RootedFs,
    ) -> ProfileJournal {
        let manifest = self.parsed_manifest();
        let browser = self.trusted_browser(&manifest);
        ProfileJournal::create_observed(
            ProfileCreateContext {
                location: &self.location,
                lease,
                browser: &browser,
                manifest: &manifest,
            },
            ProfileAttempt::Version {
                launch_strings: vec!["version".to_owned()],
            },
            rooted,
        )
        .expect("create observed version profile journal")
    }

    fn spawn_supervisor(&self, journal: &ProfileJournal, mode: TestBrowserMode) -> Child {
        let capsule = journal.capsule_json().expect("test launch capsule");
        journal
            .validates_prefix(&self.rooted())
            .expect("complete journal prefix");
        let mut command = supervisor::test_process_command(&self.executable, &capsule, mode);
        command.stderr(Stdio::inherit());
        command.spawn().expect("spawn crate-owned test supervisor")
    }

    fn abandon_dead_journal(&self) -> String {
        let lease = self.lease();
        let journal = self.version_journal(&lease);
        let path = journal.journal_path().to_owned();
        let mut child = self.spawn_supervisor(&journal, TestBrowserMode::Success);
        wait_for_running(lease.rooted(), &path);
        assert!(child.wait().expect("wait test supervisor").success());
        drop(journal);
        drop(lease);
        path
    }

    fn assert_terminal(&self) {
        let rooted = self.rooted();
        if rooted.exists(PROFILE_PARENT).expect("profile parent state") {
            assert!(
                rooted
                    .list_dir(PROFILE_PARENT)
                    .expect("profile parent inventory")
                    .is_empty(),
                "profile parent must be empty"
            );
        }
    }

    fn report_bytes(&self) -> Vec<u8> {
        fs::read(self.corpus().join("xml/generation-reports/all.json"))
            .expect("full generation report")
    }

    fn assert_four_xml(&self) {
        for variant in [
            "border_box_ltr",
            "border_box_rtl",
            "content_box_ltr",
            "content_box_rtl",
        ] {
            assert!(
                self.corpus()
                    .join(format!("xml/grid/basic__{variant}.xml"))
                    .is_file(),
                "missing generated variant {variant}"
            );
        }
    }
}
#[test]
fn layout_profile_normal_close_is_terminal() {
    let harness = GenerationHarness::new();
    harness
        .run(TestBrowserPlan::Success)
        .0
        .expect("complete generation");
    harness.assert_four_xml();
    harness.check().expect("published corpus is current");
    harness.assert_terminal();
}

#[test]
fn layout_profile_launch_failure_is_terminal() {
    let harness = GenerationHarness::new();
    harness
        .run(TestBrowserPlan::Success)
        .0
        .expect("seed prior generation");
    let before = snapshot_xml(harness.corpus());
    let error = harness
        .run(TestBrowserPlan::BrowserFailure)
        .0
        .expect_err("fake browser failure must propagate");
    assert_eq!(error.kind(), GeneratorErrorKind::Process);
    assert_eq!(snapshot_xml(harness.corpus()), before);
    harness.assert_terminal();
}

struct ReapObservingChild<'a> {
    child: &'a mut Child,
    rooted: &'a RootedFs,
    journal: &'a str,
    reaped: bool,
    journal_present_when_reaped: bool,
}

impl<'a> ReapObservingChild<'a> {
    fn new(child: &'a mut Child, rooted: &'a RootedFs, journal: &'a str) -> Self {
        Self {
            child,
            rooted,
            journal,
            reaped: false,
            journal_present_when_reaped: false,
        }
    }
}

impl OwnedSupervisorChild for ReapObservingChild<'_> {
    fn id(&self) -> Option<u32> {
        Some(self.child.id())
    }

    fn try_wait(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        let status = self.child.try_wait()?;
        if status.is_some() {
            self.reaped = true;
            self.journal_present_when_reaped = self
                .rooted
                .exists(self.journal)
                .expect("observe journal state at child reap");
        }
        Ok(status)
    }
}

#[test]
fn layout_profile_graceful_delayed_exit_is_reaped_without_signal_before_cleanup() {
    let harness = GenerationHarness::new();
    let lease = harness.lease();
    let journal = harness.version_journal(&lease);
    let path = journal.journal_path().to_owned();
    let mut child = harness.spawn_supervisor(&journal, TestBrowserMode::DelayedSupervisorExit);
    let group = child.id();
    wait_for_running(lease.rooted(), &path);
    wait_for_group(group, false);

    let (termination, reaped, journal_present_when_reaped) = {
        let mut observed = ReapObservingChild::new(&mut child, lease.rooted(), &path);
        let termination = journal
            .terminalize_owned_supervisor(lease.rooted(), Some(&mut observed))
            .expect("graceful delayed terminalization");
        (
            termination,
            observed.reaped,
            observed.journal_present_when_reaped,
        )
    };
    assert_eq!(termination, SupervisorTermination::Graceful);
    assert!(reaped, "terminalization must reap the supervisor");
    assert!(
        journal_present_when_reaped,
        "profile cleanup must begin only after the child is reaped"
    );
    assert!(
        child
            .try_wait()
            .expect("probe gracefully reaped supervisor")
            .expect("graceful terminalization reaps its owned supervisor")
            .success(),
        "a supervisor that exits within the graceful bound must not be killed"
    );
    wait_for_group(group, true);
    drop(lease);
    harness.assert_terminal();
}

#[test]
fn layout_profile_forced_group_kill_is_terminal() {
    let harness = GenerationHarness::new();
    let lease = harness.lease();
    let journal = harness.version_journal(&lease);
    let path = journal.journal_path().to_owned();
    let mut child = harness.spawn_supervisor(&journal, TestBrowserMode::Hang);
    let group = child.id();
    wait_for_running(lease.rooted(), &path);
    wait_for_group(group, false);
    let started = Instant::now();
    let (termination, reaped, journal_present_when_reaped) = {
        let mut observed = ReapObservingChild::new(&mut child, lease.rooted(), &path);
        let termination = journal
            .terminalize_owned_supervisor(lease.rooted(), Some(&mut observed))
            .expect("forced terminalization");
        (
            termination,
            observed.reaped,
            observed.journal_present_when_reaped,
        )
    };
    assert_eq!(termination, SupervisorTermination::Forced);
    assert!(
        started.elapsed() >= SUPERVISOR_EXIT_BOUND,
        "forced signaling must follow the five-second graceful exit bound"
    );
    assert!(reaped, "forced terminalization must reap the supervisor");
    assert!(
        journal_present_when_reaped,
        "profile cleanup must begin only after forced reaping"
    );
    assert!(
        !child
            .try_wait()
            .expect("probe reaped test supervisor")
            .expect("forced terminalization reaps its owned supervisor")
            .success()
    );
    wait_for_group(group, true);
    drop(lease);
    harness.assert_terminal();
}

#[test]
fn layout_profile_dead_group_without_retained_reap_proof_preserves_evidence() {
    let harness = GenerationHarness::new();
    let lease = harness.lease();
    let journal = harness.version_journal(&lease);
    let path = journal.journal_path().to_owned();
    let mut child = harness.spawn_supervisor(&journal, TestBrowserMode::Success);
    let group = child.id();
    wait_for_running(lease.rooted(), &path);
    assert!(child.wait().expect("reap successful supervisor").success());
    wait_for_group(group, true);

    let error = journal
        .terminalize_owned_supervisor(lease.rooted(), None)
        .expect_err("dead group without retained child proof must preserve evidence");
    assert_eq!(error.kind(), GeneratorErrorKind::Process);
    assert!(
        error
            .to_string()
            .contains("no retained owned supervisor child"),
        "{error}"
    );
    assert!(
        lease
            .rooted()
            .exists(&path)
            .expect("profile evidence state"),
        "active profile evidence must remain"
    );

    classify_pending(lease.rooted())
        .expect("classify retained dead profile")
        .expect("retained dead profile is recoverable")
        .execute(lease.rooted())
        .expect("recover retained dead profile");
    drop(lease);
    harness.assert_terminal();
}

#[test]
fn layout_profile_unverified_group_is_not_signaled_and_preserves_evidence() {
    let harness = GenerationHarness::new();
    let lease = harness.lease();
    let journal = harness.version_journal(&lease);
    let path = journal.journal_path().to_owned();
    let mut child = harness.spawn_supervisor(&journal, TestBrowserMode::Hang);
    let group = child.id();
    wait_for_running(lease.rooted(), &path);
    wait_for_group(group, false);

    let result = journal.terminalize_owned_supervisor(lease.rooted(), None);
    let group_was_signaled = child
        .try_wait()
        .expect("probe unverified supervisor")
        .is_some();
    let evidence_remains = lease
        .rooted()
        .exists(&path)
        .expect("profile evidence state");
    if !group_was_signaled {
        force_kill_group(group).expect("stop crate-owned unverified group");
        child.wait().expect("reap unverified test supervisor");
    }
    wait_for_group(group, true);

    let error = resolve_terminalization::<()>(
        Err(GeneratorError::new(
            GeneratorErrorKind::Generation,
            "synthetic primary generation failure",
            "primary context",
        )),
        result.map(|_| ()),
    )
    .expect_err("unverified cleanup failure must override the primary failure");
    assert_eq!(error.kind(), GeneratorErrorKind::Process);
    assert!(error.to_string().contains("primary failure"), "{error}");
    assert!(error.to_string().contains("cleanup failure"), "{error}");
    assert!(!group_was_signaled, "unverified group must not be signaled");
    assert!(evidence_remains, "active profile evidence must be retained");
}

#[test]
fn layout_profile_reused_group_identity_is_not_signaled_and_preserves_evidence() {
    let harness = GenerationHarness::new();
    let lease = harness.lease();
    let journal = harness.version_journal(&lease);
    let path = journal.journal_path().to_owned();
    let mut recorded_child = harness.spawn_supervisor(&journal, TestBrowserMode::Hang);
    let recorded_group = recorded_child.id();
    wait_for_running(lease.rooted(), &path);
    wait_for_group(recorded_group, false);

    let unrelated = GenerationHarness::new();
    let unrelated_lease = unrelated.lease();
    let unrelated_journal = unrelated.version_journal(&unrelated_lease);
    let unrelated_path = unrelated_journal.journal_path().to_owned();
    let mut unrelated_child = unrelated.spawn_supervisor(&unrelated_journal, TestBrowserMode::Hang);
    let unrelated_group = unrelated_child.id();
    wait_for_running(unrelated_lease.rooted(), &unrelated_path);
    wait_for_group(unrelated_group, false);

    let result = journal.terminalize_owned_supervisor(lease.rooted(), Some(&mut unrelated_child));
    let recorded_group_remained_live =
        !test_group_is_dead(recorded_group).expect("probe reused recorded group");
    let unrelated_group_remained_live =
        !test_group_is_dead(unrelated_group).expect("probe unrelated owned group");
    let evidence_remains = lease
        .rooted()
        .exists(&path)
        .expect("profile evidence state");

    force_kill_group(recorded_group).expect("stop crate-owned recorded group");
    recorded_child
        .wait()
        .expect("reap recorded test supervisor");
    wait_for_group(recorded_group, true);
    force_kill_group(unrelated_group).expect("stop crate-owned unrelated group");
    unrelated_child
        .wait()
        .expect("reap unrelated test supervisor");
    wait_for_group(unrelated_group, true);

    let error = result.expect_err("reused process identity must block forced signaling");
    assert_eq!(error.kind(), GeneratorErrorKind::Process);
    assert!(
        error
            .to_string()
            .contains("running record differs from the retained owned supervisor child"),
        "{error}"
    );
    assert!(recorded_group_remained_live);
    assert!(unrelated_group_remained_live);
    assert!(evidence_remains, "active profile evidence must be retained");
}

#[test]
fn layout_profile_parent_crash_live_group_blocks() {
    let harness = GenerationHarness::new();
    let lease = harness.lease();
    let journal = harness.version_journal(&lease);
    let path = journal.journal_path().to_owned();
    let mut child = harness.spawn_supervisor(&journal, TestBrowserMode::Hang);
    let group = child.id();
    wait_for_running(lease.rooted(), &path);
    drop(journal);
    drop(lease);

    let error = harness
        .run(TestBrowserPlan::Success)
        .0
        .expect_err("live abandoned group must block generation");
    assert_eq!(error.kind(), GeneratorErrorKind::LeaseActive);
    force_kill_group(group).expect("kill crate-owned abandoned group");
    child.wait().expect("reap abandoned supervisor");
    wait_for_group(group, true);
    harness
        .run(TestBrowserPlan::Success)
        .0
        .expect("dead journal recovers on retry");
    harness.assert_terminal();
}

#[test]
fn layout_profile_parent_crash_dead_group_recovers() {
    let harness = GenerationHarness::new();
    let path = harness.abandon_dead_journal();
    assert!(
        harness
            .rooted()
            .exists(&path)
            .expect("dead evidence exists")
    );
    harness
        .run(TestBrowserPlan::Success)
        .0
        .expect("generation recovers dead journal");
    harness.assert_four_xml();
    harness.assert_terminal();
}

#[test]
fn layout_profile_revalidation_failure_preserves_dead_journal() {
    let harness = GenerationHarness::new();
    let path = harness.abandon_dead_journal();
    let error = harness
        .run(TestBrowserPlan::ClosingRevalidationFailure)
        .0
        .expect_err("protected input drift must fail closing validation");
    assert!(matches!(
        error.kind(),
        GeneratorErrorKind::InvalidInventory | GeneratorErrorKind::SourceVerification
    ));
    assert!(
        harness
            .rooted()
            .exists(&path)
            .expect("dead evidence remains")
    );
}

#[test]
fn layout_profile_cleanup_begins_only_after_revalidation() {
    let harness = GenerationHarness::new();
    let path = harness.abandon_dead_journal();
    harness
        .run(TestBrowserPlan::ClosingRevalidationFailure)
        .0
        .expect_err("closing validation fails before recovery");
    assert!(
        harness
            .rooted()
            .exists(&path)
            .expect("evidence before close")
    );
    write(
        &harness.corpus().join("scripts/gentest/test_helper.js"),
        HELPER_BYTES,
    );
    harness
        .run(TestBrowserPlan::Success)
        .0
        .expect("successful revalidation permits cleanup");
    assert!(!harness.rooted().exists(&path).expect("old journal removed"));
    harness.assert_terminal();
}

#[test]
fn layout_profile_identity_drift_after_classification_preserves_evidence() {
    let harness = GenerationHarness::new();
    let path = harness.abandon_dead_journal();
    let error = harness
        .run(TestBrowserPlan::ProfileIdentityDrift)
        .0
        .expect_err("classified profile identity drift must fail");
    assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
    assert!(harness.rooted().exists(&path).expect("replacement remains"));
    assert!(
        harness
            .rooted()
            .exists(&format!("{path}-displaced"))
            .expect("original evidence remains")
    );
}

#[test]
fn layout_profile_transition_lock_closes_launch_race() {
    let harness = GenerationHarness::new();
    let lease = harness.lease();
    let journal = harness.version_journal(&lease);
    let path = journal.journal_path().to_owned();
    let mut child = harness.spawn_supervisor(&journal, TestBrowserMode::HoldTransition);
    let group = child.id();
    wait_for_running(lease.rooted(), &path);
    drop(journal);
    drop(lease);

    let error = harness
        .run(TestBrowserPlan::Success)
        .0
        .expect_err("held transition must block recovery");
    assert_eq!(error.kind(), GeneratorErrorKind::LeaseActive);
    force_kill_group(group).expect("kill transition-race supervisor");
    child.wait().expect("reap transition-race supervisor");
    wait_for_group(group, true);
    harness
        .run(TestBrowserPlan::Success)
        .0
        .expect("dead transition journal recovers");
    harness.assert_terminal();
}

#[test]
#[ignore = "exhaustive durability-prefix diagnostic requires separate authorization"]
fn layout_profile_cleanup_every_prefix_recovers() {
    let trace_harness = GenerationHarness::new();
    let lifecycle_observer = RootedObserver::recording();
    run_observed_profile_lifecycle(&trace_harness, lifecycle_observer.clone())
        .expect("record complete production profile lifecycle");
    let lifecycle_trace = normalized_events(&lifecycle_observer.events());
    assert!(!lifecycle_trace.is_empty());

    for event_index in 0..lifecycle_trace.len() {
        let harness = GenerationHarness::new();
        let observer = RootedObserver::interrupt_after(event_index);
        let interrupted = catch_unwind(AssertUnwindSafe(|| {
            run_observed_profile_lifecycle(&harness, observer.clone())
        }))
        .expect_err("selected production lifecycle event must interrupt");
        assert!(RootedObserver::is_interruption(interrupted.as_ref()));
        assert_eq!(
            normalized_events(&observer.events()),
            lifecycle_trace[..=event_index]
        );
        harness
            .run(TestBrowserPlan::Success)
            .0
            .expect("fresh generation recovers interrupted lifecycle prefix");
        harness.assert_terminal();
    }

    let recovery_trace_harness = GenerationHarness::new();
    recovery_trace_harness.abandon_dead_journal();
    let recovery_observer = RootedObserver::recording();
    run_observed_profile_recovery(&recovery_trace_harness, recovery_observer.clone())
        .expect("record complete production profile recovery");
    let recovery_trace = normalized_events(&recovery_observer.events());
    assert!(!recovery_trace.is_empty());

    for event_index in 0..recovery_trace.len() {
        let harness = GenerationHarness::new();
        harness.abandon_dead_journal();
        let observer = RootedObserver::interrupt_after(event_index);
        let interrupted = catch_unwind(AssertUnwindSafe(|| {
            run_observed_profile_recovery(&harness, observer.clone())
        }))
        .expect_err("selected production recovery event must interrupt");
        assert!(RootedObserver::is_interruption(interrupted.as_ref()));
        assert_eq!(
            normalized_events(&observer.events()),
            recovery_trace[..=event_index]
        );
        harness
            .run(TestBrowserPlan::Success)
            .0
            .expect("fresh generation recovers interrupted recovery prefix");
        harness.assert_terminal();
    }
}

#[test]
fn layout_profile_durability_trace_registers_complete_production_lifecycle() {
    let harness = GenerationHarness::new();
    let lifecycle_observer = RootedObserver::recording();
    run_observed_profile_lifecycle(&harness, lifecycle_observer.clone())
        .expect("record production profile lifecycle");
    let lifecycle = lifecycle_observer.events();

    for phase in [
        DurabilityPhase::ProfileCreate,
        DurabilityPhase::ProfileRunningPublication,
        DurabilityPhase::ProfileTerminalization,
    ] {
        assert!(
            lifecycle.iter().any(|event| event.phase() == phase),
            "missing profile durability phase {phase:?}"
        );
    }
    assert_profile_event(
        &lifecycle,
        DurabilityPhase::ProfileCreate,
        DurabilityPrimitive::WritePartial,
        "intent.json.temporary",
    );
    assert_profile_event(
        &lifecycle,
        DurabilityPhase::ProfileCreate,
        DurabilityPrimitive::RenameExclusive,
        "intent.json",
    );
    assert_profile_event(
        &lifecycle,
        DurabilityPhase::ProfileRunningPublication,
        DurabilityPrimitive::RenameExclusive,
        "running.json",
    );
    assert_profile_event(
        &lifecycle,
        DurabilityPhase::ProfileTerminalization,
        DurabilityPrimitive::RenameExclusive,
        "cleanup-",
    );
    assert_profile_event(
        &lifecycle,
        DurabilityPhase::ProfileTerminalization,
        DurabilityPrimitive::RemoveFile,
        "opaque-file",
    );
    assert_profile_event(
        &lifecycle,
        DurabilityPhase::ProfileTerminalization,
        DurabilityPrimitive::RemoveDirectory,
        "/profile",
    );

    let recovery_harness = GenerationHarness::new();
    recovery_harness.abandon_dead_journal();
    let recovery_observer = RootedObserver::recording();
    run_observed_profile_recovery(&recovery_harness, recovery_observer.clone())
        .expect("record production profile recovery");
    let recovery = recovery_observer.events();
    assert_profile_event(
        &recovery,
        DurabilityPhase::ProfileRecovery,
        DurabilityPrimitive::RenameExclusive,
        "cleanup-",
    );
    assert_profile_event(
        &recovery,
        DurabilityPhase::ProfileRecovery,
        DurabilityPrimitive::RemoveDirectory,
        "cleanup-",
    );
    assert!(
        recovery.iter().any(|event| {
            event.phase() == DurabilityPhase::ProfileRecovery
                && event.primitive() == DurabilityPrimitive::SyncDirectory
        }),
        "profile recovery must register synchronization"
    );
}

#[test]
fn layout_profile_durability_observer_interrupts_real_creation_and_recovers() {
    let harness = GenerationHarness::new();
    let observer = RootedObserver::interrupt_after(0);
    let interrupted = catch_unwind(AssertUnwindSafe(|| {
        run_observed_profile_lifecycle(&harness, observer.clone())
    }))
    .expect_err("first production profile event must interrupt");
    assert!(RootedObserver::is_interruption(interrupted.as_ref()));
    assert_eq!(observer.events().len(), 1);
    assert_eq!(observer.events()[0].phase(), DurabilityPhase::ProfileCreate);
    harness
        .run(TestBrowserPlan::Success)
        .0
        .expect("fresh generation recovers first production event prefix");
    harness.assert_terminal();
}

#[test]
fn layout_profile_partial_metadata_publication_prefix_recovers() {
    let trace_harness = GenerationHarness::new();
    let trace_observer = RootedObserver::recording();
    let (trace_lease, trace_journal, trace_rooted) =
        create_observed_profile_only(&trace_harness, trace_observer.clone());
    let trace = trace_observer.events();
    let partial_index = trace
        .iter()
        .position(|event| {
            event.phase() == DurabilityPhase::ProfileCreate
                && event.primitive() == DurabilityPrimitive::WritePartial
                && event.path().ends_with("intent.json.temporary")
        })
        .expect("production trace contains an intent byte prefix");
    trace_journal
        .terminalize(&trace_rooted)
        .expect("clean trace journal");
    drop(trace_lease);

    let harness = GenerationHarness::new();
    let observer = RootedObserver::interrupt_after(partial_index);
    let interrupted = catch_unwind(AssertUnwindSafe(|| {
        let _ = create_observed_profile_only(&harness, observer.clone());
    }))
    .expect_err("intent byte prefix must interrupt");
    assert!(RootedObserver::is_interruption(interrupted.as_ref()));
    let rooted = harness.rooted();
    classify_pending(&rooted)
        .expect("classify intent publication prefix")
        .expect("intent publication is pending")
        .execute(&rooted)
        .expect("recover intent publication prefix");
    harness.assert_terminal();
}

#[test]
fn layout_profile_opaque_entries_never_escape() {
    let harness = GenerationHarness::new();
    let lease = harness.lease();
    let journal = harness.version_journal(&lease);
    let outside = harness._temporary.path().join("outside-sentinel");
    fs::write(&outside, b"outside").expect("outside sentinel");
    let profile = journal.profile_path();
    let non_utf8 = profile.join(std::ffi::OsString::from_vec(vec![0xff, b'x']));
    if let Err(error) = fs::write(&non_utf8, b"opaque") {
        assert_eq!(error.raw_os_error(), Some(92));
        fs::write(profile.join("opaque-fallback"), b"opaque").expect("opaque fallback");
    }
    symlink(&outside, profile.join("outside-link")).expect("opaque symlink");
    journal
        .terminalize(lease.rooted())
        .expect("terminalize opaque profile");
    assert_eq!(fs::read(&outside).expect("outside remains"), b"outside");
    drop(lease);
    harness.assert_terminal();
}

#[test]
fn layout_profile_cleanup_failure_preserves_evidence() {
    let harness = GenerationHarness::new();
    let lease = harness.lease();
    let journal = harness.version_journal(&lease);
    lease
        .rooted()
        .create_file_exclusive(
            &format!("{}/unexpected", journal.journal_path()),
            b"evidence",
            PRIVATE_FILE_MODE,
        )
        .expect("inject cleanup evidence");
    let error = journal
        .terminalize(lease.rooted())
        .expect_err("unknown journal member blocks cleanup");
    assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
    assert_eq!(
        lease
            .rooted()
            .list_dir(PROFILE_PARENT)
            .expect("retained journal inventory")
            .len(),
        1
    );
}

#[test]
fn layout_profile_terminalization_rejects_journal_root_symlink_before_snapshot() {
    let harness = GenerationHarness::new();
    let lease = harness.lease();
    let journal = harness.version_journal(&lease);
    let active = harness.corpus().join(journal.journal_path());
    let displaced = harness
        .corpus()
        .join(format!("{}-displaced", journal.journal_path()));
    let outside = harness._temporary.path().join("outside-journal");
    fs::create_dir(&outside).expect("create outside journal");
    let secret = outside.join("secret");
    fs::write(&secret, b"outside evidence\n").expect("write outside evidence");
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o000))
        .expect("make outside evidence unreadable");
    fs::rename(&active, &displaced).expect("displace held journal");
    symlink(&outside, &active).expect("replace journal root with outside symlink");

    let error = journal
        .terminalize(lease.rooted())
        .expect_err("terminalization must reject a replaced journal root");

    assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
    assert!(
        error
            .to_string()
            .contains("profile journal root identity changed before snapshot"),
        "{error}"
    );
    fs::set_permissions(&secret, fs::Permissions::from_mode(0o600))
        .expect("restore outside evidence permissions");
    assert_eq!(
        fs::read(&secret).expect("read outside evidence"),
        b"outside evidence\n"
    );
    assert!(displaced.is_dir(), "original journal evidence is retained");
}

#[test]
fn layout_profile_terminalization_rejects_journal_root_replacement_before_snapshot() {
    let harness = GenerationHarness::new();
    let lease = harness.lease();
    let journal = harness.version_journal(&lease);
    let active = harness.corpus().join(journal.journal_path());
    let displaced = harness
        .corpus()
        .join(format!("{}-displaced", journal.journal_path()));
    fs::rename(&active, &displaced).expect("displace held journal");
    fs::create_dir(&active).expect("replace journal directory");
    let sentinel = active.join("sentinel");
    fs::write(&sentinel, b"replacement evidence\n").expect("write replacement evidence");

    let error = journal
        .terminalize(lease.rooted())
        .expect_err("terminalization must reject a replacement directory");

    assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
    assert!(
        error
            .to_string()
            .contains("profile journal root identity changed before snapshot"),
        "{error}"
    );
    assert_eq!(
        fs::read(&sentinel).expect("read replacement evidence"),
        b"replacement evidence\n"
    );
    assert!(displaced.is_dir(), "original journal evidence is retained");
}

#[test]
fn layout_dependency_panic_maps_to_process() {
    let harness = GenerationHarness::new();
    let error = harness
        .run(TestBrowserPlan::DependencyPanic)
        .0
        .expect_err("dependency panic must become Process");
    assert_eq!(error.kind(), GeneratorErrorKind::Process);
    harness.assert_terminal();
    assert!(!harness.corpus().join("xml").exists());
}

#[test]
fn layout_profile_panic_resumes_after_cleanup() {
    let harness = GenerationHarness::new();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = harness.run(TestBrowserPlan::OwnedPanic);
    }))
    .expect_err("owned panic resumes through generation front door");
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"synthetic owned generation panic")
    );
    harness.assert_terminal();
}

#[test]
fn layout_profile_panic_retains_cleanup_evidence() {
    let harness = GenerationHarness::new();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = harness.run(TestBrowserPlan::OwnedPanicWithCleanupFailure);
    }))
    .expect_err("owned panic resumes despite cleanup failure");
    assert_eq!(
        panic.downcast_ref::<&str>(),
        Some(&"synthetic owned generation panic")
    );
    assert_eq!(
        harness
            .rooted()
            .list_dir(PROFILE_PARENT)
            .expect("retained cleanup evidence")
            .len(),
        1
    );
}

#[test]
fn layout_generate_retry_then_publishes_clean() {
    let harness = GenerationHarness::new();
    let (result, host) = harness.run(TestBrowserPlan::RetryOnce);
    result.expect("retry generation succeeds");
    assert_eq!(host.attempts(), vec![(0, 0), (0, 1)]);
    harness.assert_four_xml();
    let report: serde_json::Value =
        serde_json::from_slice(&harness.report_bytes()).expect("report JSON");
    assert_eq!(report["summary"]["generated"], 4);
    assert_eq!(report["summary"]["failed_to_generate"], 0);
    harness.check().expect("retry publication is current");
    harness.assert_terminal();
}

#[test]
fn layout_generate_diagnostic_failure_publishes_report_without_xml() {
    let harness = GenerationHarness::new();
    let (result, host) = harness.run(TestBrowserPlan::AlwaysFail);
    let error = result.expect_err("exhausted retry publishes diagnostic error");
    assert_eq!(error.kind(), GeneratorErrorKind::Generation);
    assert_eq!(host.attempts(), vec![(0, 0), (0, 1)]);
    let report: serde_json::Value =
        serde_json::from_slice(&harness.report_bytes()).expect("diagnostic report JSON");
    assert_eq!(report["summary"]["generated"], 0);
    assert_eq!(report["summary"]["failed_to_generate"], 1);
    assert!(
        !harness
            .corpus()
            .join("xml/grid/basic__border_box_ltr.xml")
            .exists()
    );
    assert_eq!(
        harness
            .check()
            .expect_err("diagnostic corpus is stale")
            .kind(),
        GeneratorErrorKind::Verification
    );
    harness.assert_terminal();
}

#[test]
fn layout_generate_filtered_success_preserves_full_report() {
    let harness = GenerationHarness::new();
    harness
        .run(TestBrowserPlan::Success)
        .0
        .expect("seed full generation");
    let report = harness.report_bytes();
    let (result, host) = harness.run_filtered("grid/basic.html", TestBrowserPlan::Success);
    result.expect("filtered generation succeeds");
    assert_eq!(host.attempts(), vec![(0, 0)]);
    assert_eq!(harness.report_bytes(), report);
    harness.assert_four_xml();
    harness.assert_terminal();
}

#[test]
fn layout_fake_supervisor_process() {
    if std::env::var_os(supervisor::CAPSULE_ENV).is_none() {
        return;
    }
    supervisor::test_run_from_env().expect("run crate-owned test supervisor");
}

#[test]
fn layout_fake_browser_success_process() {
    if !is_fake_browser_invocation() {
        return;
    }
    assert_eq!(std::env::var("PATH").as_deref(), Ok("/usr/bin:/bin"));
    assert_eq!(std::env::var("NO_PROXY").as_deref(), Ok("*"));
    assert_eq!(std::env::var("HTTP_PROXY").as_deref(), Ok(""));
}

#[test]
fn layout_fake_browser_failure_process() {
    if is_fake_browser_invocation() {
        panic!("crate-owned fake browser launch failure");
    }
}

#[test]
fn layout_fake_browser_hang_process() {
    if is_fake_browser_invocation() {
        std::thread::sleep(Duration::from_secs(30));
    }
}

#[test]
fn layout_profile_name_and_cleanup_grammar_is_exact() {
    let lease = "1".repeat(32);
    let profile = "2".repeat(32);
    let active = format!("active-{lease}-version-{profile}");
    test_validate_journal_name(&active).expect("version journal grammar");
    let cleanup = test_cleanup_path(&format!("{PROFILE_PARENT}/{active}")).expect("cleanup path");
    assert_eq!(
        cleanup,
        format!("{PROFILE_PARENT}/cleanup-{lease}-version-{profile}")
    );
}
fn run_observed_profile_lifecycle(
    harness: &GenerationHarness,
    observer: RootedObserver,
) -> crate::Result<()> {
    let dead_group = dead_crate_owned_process_id();
    let lease = harness.lease();
    let rooted = harness.observed_rooted(observer);
    let journal = harness.observed_version_journal(&lease, &rooted);
    fs::write(journal.profile_path().join("opaque-file"), b"opaque\n")
        .expect("seed opaque browser file");
    let outside = harness._temporary.path().join("opaque-outside");
    fs::write(&outside, b"outside\n").expect("seed opaque outside sentinel");
    symlink(&outside, journal.profile_path().join("opaque-link"))
        .expect("seed opaque browser symlink");
    journal.test_publish_running(&rooted, dead_group)?;
    journal.terminalize(&rooted)?;
    assert_eq!(fs::read(outside).expect("outside sentinel"), b"outside\n");
    drop(lease);
    harness.assert_terminal();
    Ok(())
}

fn create_observed_profile_only(
    harness: &GenerationHarness,
    observer: RootedObserver,
) -> (GenerationLease, ProfileJournal, RootedFs) {
    let lease = harness.lease();
    let rooted = harness.observed_rooted(observer);
    let journal = harness.observed_version_journal(&lease, &rooted);
    (lease, journal, rooted)
}

fn run_observed_profile_recovery(
    harness: &GenerationHarness,
    observer: RootedObserver,
) -> crate::Result<()> {
    let lease = harness.lease();
    let rooted = harness.observed_rooted(observer);
    classify_pending(&rooted)?
        .expect("abandoned profile is pending")
        .execute(&rooted)?;
    drop(lease);
    harness.assert_terminal();
    Ok(())
}

fn dead_crate_owned_process_id() -> u32 {
    let executable = std::env::current_exe().expect("current test executable");
    let mut child = std::process::Command::new(executable)
        .args([
            "--exact",
            "browser::profile_tests::layout_fake_browser_success_process",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn crate-owned dead-group probe");
    let id = child.id();
    assert!(child.wait().expect("reap dead-group probe").success());
    id
}

fn assert_profile_event(
    events: &[DurabilityEvent],
    phase: DurabilityPhase,
    primitive: DurabilityPrimitive,
    path_fragment: &str,
) {
    assert!(
        events.iter().any(|event| {
            event.phase() == phase
                && event.primitive() == primitive
                && event.path().contains(path_fragment)
        }),
        "missing {phase:?}/{primitive:?} event containing {path_fragment:?}"
    );
}

fn normalized_events(
    events: &[DurabilityEvent],
) -> Vec<(DurabilityPhase, DurabilityPrimitive, String)> {
    events
        .iter()
        .map(|event| {
            let path = event
                .path()
                .split('/')
                .map(|component| {
                    if component.starts_with("active-") || component.starts_with("cleanup-") {
                        "<profile-journal>"
                    } else {
                        component
                    }
                })
                .collect::<Vec<_>>()
                .join("/");
            (event.phase(), event.primitive(), path)
        })
        .collect()
}

fn path_relative_to(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .expect("path below owner root")
        .components()
        .map(|component| component.as_os_str().to_str().expect("UTF-8 test path"))
        .collect::<Vec<_>>()
        .join("/")
}

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().expect("file parent")).expect("create file parent");
    fs::write(path, bytes).expect("write generation fixture");
}

fn snapshot_xml(corpus: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let root = corpus.join("xml");
    let mut files = Vec::new();
    fn visit(root: &Path, current: &Path, files: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut children = fs::read_dir(current)
            .expect("read XML directory")
            .collect::<std::io::Result<Vec<_>>>()
            .expect("read XML entries");
        children.sort_by_key(fs::DirEntry::file_name);
        for child in children {
            let path = child.path();
            if path.is_dir() {
                visit(root, &path, files);
            } else {
                files.push((
                    path.strip_prefix(root).expect("XML path").to_path_buf(),
                    fs::read(path).expect("read XML file"),
                ));
            }
        }
    }
    visit(&root, &root, &mut files);
    files
}

fn wait_for_running(rooted: &RootedFs, journal: &str) {
    let running = rooted.canonical_root().join(journal).join("running.json");
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        match fs::symlink_metadata(&running) {
            Ok(metadata) => {
                assert!(
                    metadata.file_type().is_file(),
                    "running record is not a file"
                );
                return;
            }
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => panic!("inspect running record state: {source}"),
        }
        assert!(
            Instant::now() < deadline,
            "supervisor did not publish running record"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn wait_for_group(group: u32, dead: bool) {
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if test_group_is_dead(group).expect("probe fake group") == dead {
            return;
        }
        assert!(Instant::now() < deadline, "fake group state did not settle");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn is_fake_browser_invocation() -> bool {
    let home = std::env::var_os("HOME");
    let xdg = std::env::var_os("XDG_CONFIG_HOME");
    home.as_deref()
        .is_some_and(|path| Path::new(path).ends_with("home"))
        && xdg
            .as_deref()
            .is_some_and(|path| Path::new(path).ends_with("xdg-config"))
        && std::env::var("NO_PROXY").as_deref() == Ok("*")
}

#[test]
fn browser_profile_waits_for_registration_before_owned_cleanup() {
    let harness = GenerationHarness::new();
    let lease = harness.lease();
    let journal = harness.version_journal(&lease);
    let mut child = harness.spawn_supervisor(&journal, TestBrowserMode::DelayedRegistration);
    let terminal = journal.terminalize_owned_supervisor(lease.rooted(), Some(&mut child));
    let status = child.wait().expect("reap delayed supervisor");
    assert_eq!(
        terminal.expect("registered supervisor terminalization"),
        SupervisorTermination::Graceful
    );
    assert!(
        status.success(),
        "cleanup must preserve the journal until its delayed supervisor registers and exits"
    );
    drop(lease);
    harness.assert_terminal();
}
