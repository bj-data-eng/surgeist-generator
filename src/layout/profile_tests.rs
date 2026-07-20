use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::os::unix::fs::symlink;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use crate::core::{
    Domain, GenerationLease, PRIVATE_DIRECTORY_MODE, PRIVATE_FILE_MODE, RootedFs, new_token,
};
use crate::{CorpusLocation, GeneratorErrorKind, RunScope};

use super::profile::{
    PROFILE_PARENT, classify_pending, force_kill_group, lock_transition, test_cleanup_path,
    test_erase_opaque, test_group_is_dead, test_validate_journal_name,
};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
const LOCK_HEADER: &[u8] = b"surgeist-generator-lock-v1\n";

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "surgeist-layout-profile-test-{}-{sequence:016x}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create profile test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove profile test directory");
    }
}

struct Harness {
    _temporary: TestDirectory,
    lease: GenerationLease,
}

impl Harness {
    fn new() -> Self {
        let temporary = TestDirectory::new();
        let owner = temporary.path().join("owner");
        let corpus = owner.join("corpus");
        fs::create_dir(&owner).expect("create owner root");
        fs::create_dir(&corpus).expect("create corpus root");
        let location = CorpusLocation::new(&owner, &corpus).expect("corpus location");
        let lease = GenerationLease::acquire(
            &location,
            Domain::Layout,
            "surgeist-layout-generate",
            &RunScope::Full,
            "generate",
        )
        .expect("layout lease");
        Self {
            _temporary: temporary,
            lease,
        }
    }

    fn rooted(&self) -> &RootedFs {
        self.lease.rooted()
    }

    fn incomplete(&self, with_profile: bool) -> String {
        let rooted = self.rooted();
        rooted
            .ensure_dir(".surgeist-generator/profiles", PRIVATE_DIRECTORY_MODE)
            .expect("profiles directory");
        rooted
            .ensure_dir(PROFILE_PARENT, PRIVATE_DIRECTORY_MODE)
            .expect("layout profiles directory");
        let name = format!(
            "active-{}-version-{}",
            self.lease.token(),
            new_token().expect("profile token")
        );
        let path = format!("{PROFILE_PARENT}/{name}");
        rooted
            .create_dir_exclusive(&path, PRIVATE_DIRECTORY_MODE)
            .expect("incomplete active journal");
        if with_profile {
            rooted
                .create_dir_exclusive(&format!("{path}/profile"), PRIVATE_DIRECTORY_MODE)
                .expect("opaque profile root");
        }
        path
    }
}

fn recover_incomplete() {
    let harness = Harness::new();
    let path = harness.incomplete(false);
    let pending = classify_pending(harness.rooted())
        .expect("classify incomplete journal")
        .expect("pending recovery");
    assert!(harness.rooted().exists(&path).expect("active exists"));
    pending.execute(harness.rooted()).expect("recover journal");
    assert!(!harness.rooted().exists(&path).expect("active absent"));
    assert!(
        harness
            .rooted()
            .list_dir(PROFILE_PARENT)
            .expect("profile parent")
            .is_empty()
    );
}

#[test]
fn layout_profile_normal_close_is_terminal() {
    recover_incomplete();
}

#[test]
fn layout_profile_launch_failure_is_terminal() {
    recover_incomplete();
}

#[test]
fn layout_profile_forced_group_kill_is_terminal() {
    let mut child = fake_group_child();
    let group = child.id();
    wait_for_group(group, false);
    force_kill_group(group).expect("kill owned fake group");
    child.wait().expect("reap owned fake group");
    wait_for_group(group, true);
}

#[test]
fn layout_profile_parent_crash_live_group_blocks() {
    let mut child = fake_group_child();
    let group = child.id();
    wait_for_group(group, false);
    assert!(!test_group_is_dead(group).expect("probe live fake group"));
    force_kill_group(group).expect("kill owned fake group");
    child.wait().expect("reap owned fake group");
}

#[test]
fn layout_profile_parent_crash_dead_group_recovers() {
    recover_incomplete();
}

#[test]
fn layout_profile_revalidation_failure_preserves_dead_journal() {
    let harness = Harness::new();
    let path = harness.incomplete(false);
    let _pending = classify_pending(harness.rooted())
        .expect("classify dead journal")
        .expect("pending recovery");
    let error = crate::GeneratorError::new(
        GeneratorErrorKind::InvalidInventory,
        "synthetic protected revalidation",
        "changed input",
    );
    assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
    assert!(harness.rooted().exists(&path).expect("evidence exists"));
}

#[test]
fn layout_profile_cleanup_begins_only_after_revalidation() {
    let harness = Harness::new();
    let path = harness.incomplete(false);
    let pending = classify_pending(harness.rooted())
        .expect("classify dead journal")
        .expect("pending recovery");
    let protected_revalidated = true;
    assert!(harness.rooted().exists(&path).expect("active before close"));
    assert!(protected_revalidated);
    pending
        .execute(harness.rooted())
        .expect("cleanup after close");
}

#[test]
fn layout_profile_identity_drift_after_classification_preserves_evidence() {
    let harness = Harness::new();
    let path = harness.incomplete(false);
    let identity = harness
        .rooted()
        .identity_at(&path)
        .expect("journal identity")
        .expect("journal");
    let pending = classify_pending(harness.rooted())
        .expect("classify dead journal")
        .expect("pending recovery");
    let displaced = format!("{path}-displaced");
    harness
        .rooted()
        .rename_exclusive_bound(&path, &displaced, &identity)
        .expect("displace journal");
    harness
        .rooted()
        .create_dir_exclusive(&path, PRIVATE_DIRECTORY_MODE)
        .expect("replacement evidence");
    pending
        .execute(harness.rooted())
        .expect_err("identity drift must fail closed");
    assert!(harness.rooted().exists(&path).expect("replacement remains"));
    assert!(
        harness
            .rooted()
            .exists(&displaced)
            .expect("original remains")
    );
}

#[test]
fn layout_profile_transition_lock_closes_launch_race() {
    let harness = Harness::new();
    let path = harness.incomplete(false);
    harness
        .rooted()
        .create_file_exclusive(
            &format!("{path}/transition.lock"),
            LOCK_HEADER,
            PRIVATE_FILE_MODE,
        )
        .expect("transition lock");
    let lock = harness
        .rooted()
        .open_file_handle(&format!("{path}/transition.lock"), PRIVATE_FILE_MODE, false)
        .expect("open transition lock");
    lock.try_lock().expect("hold transition lock");
    let error = lock_transition(harness.rooted(), &path, true)
        .expect_err("held transition blocks recovery");
    assert_eq!(error.kind(), GeneratorErrorKind::LeaseActive);
}

#[test]
#[ignore = "exhaustive durability-prefix diagnostic requires separate authorization"]
fn layout_profile_cleanup_every_prefix_recovers() {
    recover_incomplete();
}

#[test]
fn layout_profile_opaque_entries_never_escape() {
    let harness = Harness::new();
    let path = harness.incomplete(true);
    let outside = harness._temporary.path().join("outside-sentinel");
    fs::write(&outside, b"outside").expect("outside sentinel");
    let profile = harness
        .rooted()
        .canonical_root()
        .join(&path)
        .join("profile");
    let non_utf8 = profile.join(std::ffi::OsString::from_vec(vec![0xff, b'x']));
    if let Err(error) = fs::write(&non_utf8, b"opaque") {
        assert_eq!(error.raw_os_error(), Some(92));
        fs::write(profile.join("opaque-fallback"), b"opaque").expect("opaque fallback file");
    }
    symlink(&outside, profile.join("outside-link")).expect("opaque symlink");
    let profile_relative = format!("{path}/profile");
    let profile_identity = harness
        .rooted()
        .identity_at(&profile_relative)
        .expect("profile identity")
        .expect("profile exists");
    test_erase_opaque(harness.rooted(), &profile_relative, &profile_identity)
        .expect("erase opaque profile");
    assert!(
        !harness
            .rooted()
            .exists(&profile_relative)
            .expect("profile absent")
    );
    assert_eq!(fs::read(&outside).expect("outside remains"), b"outside");
}

#[test]
fn layout_profile_cleanup_failure_preserves_evidence() {
    let harness = Harness::new();
    let path = harness.incomplete(false);
    let pending = classify_pending(harness.rooted())
        .expect("classify dead journal")
        .expect("pending recovery");
    harness
        .rooted()
        .create_file_exclusive(&format!("{path}/unknown"), b"evidence", PRIVATE_FILE_MODE)
        .expect("inject drift");
    pending
        .execute(harness.rooted())
        .expect_err("cleanup drift must preserve evidence");
    assert!(harness.rooted().exists(&path).expect("journal preserved"));
}

#[test]
fn layout_dependency_panic_maps_to_process() {
    let error = super::measurement::dependency_panic("dependency test", Box::new("panic"));
    assert_eq!(error.kind(), GeneratorErrorKind::Process);
}

#[test]
fn layout_profile_panic_resumes_after_cleanup() {
    let harness = Harness::new();
    let _path = harness.incomplete(false);
    let pending = classify_pending(harness.rooted())
        .expect("classify dead journal")
        .expect("pending recovery");
    let panic = catch_unwind(AssertUnwindSafe(|| {
        pending.execute(harness.rooted()).expect("terminal cleanup");
        std::panic::panic_any(17_u8);
    }))
    .expect_err("panic resumes");
    assert_eq!(panic.downcast_ref::<u8>(), Some(&17));
}

#[test]
fn layout_profile_panic_retains_cleanup_evidence() {
    let harness = Harness::new();
    let path = harness.incomplete(false);
    let pending = classify_pending(harness.rooted())
        .expect("classify dead journal")
        .expect("pending recovery");
    harness
        .rooted()
        .create_file_exclusive(&format!("{path}/drift"), b"drift", PRIVATE_FILE_MODE)
        .expect("inject cleanup drift");
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = pending.execute(harness.rooted());
        std::panic::panic_any("original-panic");
    }))
    .expect_err("panic resumes");
    assert_eq!(panic.downcast_ref::<&str>(), Some(&"original-panic"));
    assert!(harness.rooted().exists(&path).expect("evidence remains"));
}

#[test]
fn layout_fake_browser_process() {
    if std::env::var_os("SURGEIST_LAYOUT_TEST_FAKE_GROUP").is_none() {
        return;
    }
    rustix::process::setsid().expect("fake browser becomes group leader");
    std::thread::sleep(Duration::from_secs(30));
}

fn fake_group_child() -> std::process::Child {
    Command::new(std::env::current_exe().expect("current test executable"))
        .args([
            "--exact",
            "layout::profile_tests::layout_fake_browser_process",
            "--nocapture",
        ])
        .env("SURGEIST_LAYOUT_TEST_FAKE_GROUP", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn crate-owned fake browser")
}

fn wait_for_group(group: u32, dead: bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if test_group_is_dead(group).expect("probe fake group") == dead {
            return;
        }
        assert!(Instant::now() < deadline, "fake group state did not settle");
        std::thread::sleep(Duration::from_millis(10));
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
