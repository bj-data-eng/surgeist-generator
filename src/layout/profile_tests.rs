use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::symlink;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::core::{
    Domain, GenerationLease, ObjectFormat, PRIVATE_FILE_MODE, RootedFs, SnapshotEntry,
    VerifiedSourceSnapshot,
};
use crate::{
    CorpusLocation, GeneratorError, GeneratorErrorKind, PinnedSource, RelativePath, RunScope,
    Sha256Digest, SourceRevision,
};

use super::browser::TrustedBrowser;
use super::measurement::{TestBrowserPlan, TestGenerationHost};
use super::profile::{
    PROFILE_PARENT, ProfileAttempt, ProfileCreateContext, ProfileCreateStep, ProfileJournal,
    force_kill_group, test_cleanup_path, test_group_is_dead, test_validate_journal_name,
};
use super::supervisor::TestBrowserMode;
use super::{LayoutRequest, generation, manifest, sidecar, supervisor, tests};

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
}

impl GenerationHarness {
    fn new() -> Self {
        let executable =
            fs::canonicalize(std::env::current_exe().expect("current test executable"))
                .expect("canonical test executable");
        let cache = executable
            .parent()
            .expect("test executable cache directory");
        let owner = cache
            .parent()
            .expect("test executable cache has an owner directory");
        let temporary = TestDirectory::new(owner);
        let corpus = temporary.path().join("corpus");
        fs::create_dir(&corpus).expect("create generation corpus");
        let location = CorpusLocation::new(owner, &corpus).expect("generation corpus location");
        let cache_root = path_relative_to(owner, cache);
        let browser_path = RelativePath::new(path_relative_to(owner, &executable))
            .expect("owner-relative current test executable");
        let manifest_text = tests::manifest_text(tests::SHA1_REVISION, 1, "").replace(
            "cache_root = \"browser-cache\"",
            &format!("cache_root = \"{cache_root}\""),
        );
        write(
            &corpus.join(manifest::MANIFEST_FILE),
            manifest_text.as_bytes(),
        );
        write(&corpus.join("scripts/gentest/test_helper.js"), HELPER_BYTES);
        write(
            &corpus.join("scripts/gentest/test_base_style.css"),
            BASE_STYLE_BYTES,
        );
        write(&corpus.join("html/grid/basic.html"), FIXTURE_BYTES);
        write(
            &corpus.join("html").join(manifest::SIDECAR_FILE),
            &sidecar_bytes(),
        );
        Self {
            _temporary: temporary,
            location,
            browser_path,
            executable,
        }
    }

    fn corpus(&self) -> &Path {
        self.location.corpus_root()
    }

    fn request(&self, filter: Option<&str>) -> LayoutRequest {
        LayoutRequest::generate(
            self.location.clone(),
            self.browser_path.clone(),
            filter
                .map(RelativePath::new)
                .transpose()
                .expect("generation filter"),
        )
        .expect("generation request")
    }

    fn run(&self, plan: TestBrowserPlan) -> (crate::Result<()>, TestGenerationHost) {
        let host = TestGenerationHost::new(plan);
        let result = generation::run_with_test_host(self.request(None), host.clone());
        (result, host)
    }

    fn run_filtered(
        &self,
        filter: &str,
        plan: TestBrowserPlan,
    ) -> (crate::Result<()>, TestGenerationHost) {
        let host = TestGenerationHost::new(plan);
        let result = generation::run_with_test_host(self.request(Some(filter)), host.clone());
        (result, host)
    }

    fn check(&self) -> crate::Result<()> {
        super::run(LayoutRequest::check_corpus(self.location.clone()))
    }

    fn rooted(&self) -> RootedFs {
        RootedFs::open_corpus(&self.location).expect("open generation corpus")
    }

    fn lease(&self) -> GenerationLease {
        GenerationLease::acquire(
            &self.location,
            Domain::Layout,
            "surgeist-layout-generate",
            &RunScope::Full,
            "generate",
        )
        .expect("layout generation lease")
    }

    fn parsed_manifest(&self) -> super::manifest::LayoutManifest {
        let path = self.corpus().join(manifest::MANIFEST_FILE);
        let bytes = fs::read(&path).expect("read generation manifest");
        manifest::parse(&bytes, &path).expect("parse generation manifest")
    }

    fn trusted_browser(&self, manifest: &super::manifest::LayoutManifest) -> TrustedBrowser {
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

#[test]
fn layout_profile_forced_group_kill_is_terminal() {
    let harness = GenerationHarness::new();
    let lease = harness.lease();
    let journal = harness.version_journal(&lease);
    let path = journal.journal_path().to_owned();
    let child = harness.spawn_supervisor(&journal, TestBrowserMode::Hang);
    let group = child.id();
    wait_for_running(lease.rooted(), &path);
    wait_for_group(group, false);
    let waiter = std::thread::spawn(move || {
        let mut child = child;
        child.wait().expect("reap killed test supervisor")
    });
    journal
        .terminalize_with_forced_group_kill(lease.rooted())
        .expect("forced terminalization");
    assert!(!waiter.join().expect("join supervisor waiter").success());
    wait_for_group(group, true);
    drop(lease);
    harness.assert_terminal();
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
    for stop_after in ProfileCreateStep::ALL {
        let harness = GenerationHarness::new();
        let lease = harness.lease();
        let manifest = harness.parsed_manifest();
        let browser = harness.trusted_browser(&manifest);
        let error = ProfileJournal::create_observed(
            ProfileCreateContext {
                location: &harness.location,
                lease: &lease,
                browser: &browser,
                manifest: &manifest,
            },
            ProfileAttempt::Version {
                launch_strings: vec!["version".to_owned()],
            },
            |step| {
                if step == stop_after {
                    Err(GeneratorError::new(
                        GeneratorErrorKind::ArtifactTransaction,
                        "interrupt layout profile creation prefix",
                        format!("interrupted after {step:?}"),
                    ))
                } else {
                    Ok(())
                }
            },
        )
        .expect_err("selected prefix must interrupt journal creation");
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        drop(lease);
        harness
            .run(TestBrowserPlan::Success)
            .0
            .expect("fresh generation recovers interrupted prefix");
        harness.assert_terminal();
    }
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

fn sidecar_bytes() -> Vec<u8> {
    let snapshot = VerifiedSourceSnapshot {
        object_format: ObjectFormat::Sha1,
        entries: vec![SnapshotEntry {
            path: RelativePath::new("grid/basic.html").expect("fixture path"),
            git_mode: "100644".to_owned(),
            blob_object_id: "2".repeat(40),
            digest: Sha256Digest::from_bytes(FIXTURE_BYTES),
            bytes: FIXTURE_BYTES.to_vec(),
        }],
    };
    let pin = PinnedSource::new(
        "taffy",
        manifest::TAFFY_REPOSITORY,
        SourceRevision::new(tests::SHA1_REVISION).expect("source revision"),
        RelativePath::new(manifest::TAFFY_SOURCE_DIRECTORY).expect("source directory"),
    )
    .expect("Taffy pin");
    sidecar::canonical_bytes(&pin, 1, &snapshot).expect("canonical Taffy sidecar")
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
    let running = format!("{journal}/running.json");
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        if rooted.exists(&running).expect("running record state") {
            return;
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
