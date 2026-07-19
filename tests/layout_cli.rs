#![cfg(feature = "layout-browser")]

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const TAFFY_REPOSITORY: &str = "https://github.com/DioxusLabs/taffy.git";
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "surgeist-generator-layout-cli-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).expect("create CLI test root");
        Self(path)
    }
}

impl Drop for TestRoot {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove CLI test root");
    }
}

fn run_git(directory: &Path, arguments: &[&OsStr]) -> String {
    let output = Command::new("/usr/bin/git")
        .arg("-C")
        .arg(directory)
        .args(arguments)
        .output()
        .expect("run installed Git");
    assert!(
        output.status.success(),
        "Git failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("UTF-8 Git output")
        .trim_end()
        .to_owned()
}

#[test]
fn layout_cli_invalid_syntax_prints_exact_prefix_and_exits_64() {
    let output = Command::new(env!("CARGO_BIN_EXE_surgeist-layout-generate"))
        .output()
        .expect("run packaged layout binary");

    assert_eq!(output.status.code(), Some(64));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("UTF-8 diagnostic"),
        "surgeist-layout-generate: parse layout command line: missing --owner-root\n"
    );
}

#[test]
fn layout_cli_taffy_option_matrix_precedes_io() {
    for arguments in [
        vec![
            "--owner-root",
            "does-not-exist",
            "--corpus-root",
            "does-not-exist",
            "import-taffy",
        ],
        vec![
            "--owner-root",
            "does-not-exist",
            "--corpus-root",
            "does-not-exist",
            "import-taffy",
            "--source-root",
            "checkout",
            "--source-root",
            "other",
        ],
        vec![
            "--owner-root",
            "does-not-exist",
            "--corpus-root",
            "does-not-exist",
            "import-taffy",
            "--source-root",
            "checkout",
            "--filter",
            "grid",
        ],
        vec![
            "--owner-root",
            "does-not-exist",
            "--corpus-root",
            "does-not-exist",
            "check-taffy-corpus",
        ],
        vec![
            "--owner-root",
            "does-not-exist",
            "--corpus-root",
            "does-not-exist",
            "check-taffy-corpus",
            "--source-root",
            "checkout",
            "--filter",
            "grid",
        ],
        vec![
            "--owner-root",
            "does-not-exist",
            "--corpus-root",
            "does-not-exist",
            "check-corpus",
        ],
        vec![
            "--owner-root",
            "does-not-exist",
            "--corpus-root",
            "does-not-exist",
            "generate",
        ],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_surgeist-layout-generate"))
            .args(arguments)
            .output()
            .expect("run packaged layout binary");

        assert_eq!(output.status.code(), Some(64));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8(output.stderr)
                .expect("UTF-8 diagnostic")
                .starts_with("surgeist-layout-generate: parse layout command line:")
        );
    }

    for command in ["import-taffy", "check-taffy-corpus"] {
        let accepted = Command::new(env!("CARGO_BIN_EXE_surgeist-layout-generate"))
            .args([
                "--owner-root",
                "does-not-exist",
                "--corpus-root",
                "does-not-exist",
                command,
                "--source-root",
                "checkout",
            ])
            .output()
            .expect("run packaged layout binary");
        assert_eq!(accepted.status.code(), Some(1));
        assert!(accepted.stdout.is_empty());
        assert!(
            String::from_utf8(accepted.stderr)
                .expect("UTF-8 diagnostic")
                .starts_with("surgeist-layout-generate: canonicalize owner root:")
        );
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn layout_cli_import_and_check_taffy_execute_real_public_paths() {
    let root = TestRoot::new();
    let owner = root.0.join("owner");
    let corpus = owner.join("corpus");
    let checkout = root.0.join("checkout");
    fs::create_dir_all(&corpus).expect("create corpus");
    fs::create_dir(&checkout).expect("create checkout");
    run_git(&checkout, &[OsStr::new("init"), OsStr::new("--quiet")]);
    for (key, value) in [
        ("user.name", "Layout CLI Test"),
        ("user.email", "layout-cli@example.invalid"),
    ] {
        run_git(
            &checkout,
            &[OsStr::new("config"), OsStr::new(key), OsStr::new(value)],
        );
    }
    run_git(
        &checkout,
        &[
            OsStr::new("remote"),
            OsStr::new("add"),
            OsStr::new("origin"),
            OsStr::new(TAFFY_REPOSITORY),
        ],
    );
    let fixture = checkout.join("test_fixtures/grid/basic.html");
    fs::create_dir_all(fixture.parent().expect("fixture parent")).expect("create fixture parent");
    fs::write(&fixture, b"<div>CLI fixture</div>\n").expect("write fixture");
    run_git(&checkout, &[OsStr::new("add"), OsStr::new("test_fixtures")]);
    run_git(
        &checkout,
        &[
            OsStr::new("commit"),
            OsStr::new("--quiet"),
            OsStr::new("-m"),
            OsStr::new("fixture"),
        ],
    );
    let revision = run_git(&checkout, &[OsStr::new("rev-parse"), OsStr::new("HEAD")]);
    fs::write(corpus.join("corpus.toml"), manifest_text(&revision)).expect("write layout manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_surgeist-layout-generate"))
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("import-taffy")
        .arg("--source-root")
        .arg(&checkout)
        .output()
        .expect("run packaged layout import");
    assert!(
        output.status.success(),
        "layout binary failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read(corpus.join("html/grid/basic.html")).expect("read imported fixture"),
        b"<div>CLI fixture</div>\n"
    );
    let sidecar =
        fs::read(corpus.join("html/.surgeist-taffy-source.json")).expect("read imported sidecar");
    let value: serde_json::Value = serde_json::from_slice(&sidecar).expect("sidecar JSON");
    assert_eq!(value["source"]["revision"], revision);
    assert_eq!(value["source_file_count"], 1);
    assert_eq!(value["imported_file_count"], 1);

    let imported_path = corpus.join("html/grid/basic.html");
    let imported_identity = path_identity(&imported_path);
    let sidecar_identity = path_identity(&corpus.join("html/.surgeist-taffy-source.json"));
    let output = Command::new(env!("CARGO_BIN_EXE_surgeist-layout-generate"))
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("check-taffy-corpus")
        .arg("--source-root")
        .arg(&checkout)
        .output()
        .expect("run packaged layout Taffy check");
    assert!(
        output.status.success(),
        "layout check binary failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read(&imported_path).expect("read checked fixture"),
        b"<div>CLI fixture</div>\n"
    );
    assert_eq!(
        fs::read(corpus.join("html/.surgeist-taffy-source.json")).unwrap(),
        sidecar
    );
    assert_eq!(path_identity(&imported_path), imported_identity);
    assert_eq!(
        path_identity(&corpus.join("html/.surgeist-taffy-source.json")),
        sidecar_identity
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn path_identity(path: &Path) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::symlink_metadata(path).expect("path identity");
    (metadata.dev(), metadata.ino())
}

fn manifest_text(revision: &str) -> String {
    format!(
        r#"schema_version = 2

[browser]
source = "chrome-for-testing"
version = "123.0.1"
version_output = "Chrome for Testing 123.0.1"
cache_root = "browser-cache"
provenance_format = "Chrome {{version}} {{repository_relative_executable}}"

[browser.launch]
batch_size = 1
navigation_timeout_ms = 1
dom_poll_interval_ms = 1
retry_count = 1
job_order = "sorted-sequential"
retry_error_class = "open-load-reset-timeout"
profile_scope = "per-batch-and-retry"
page_scope = "per-job"
disable_default_args = true
disable_cache = true
arguments = [
  "use-mock-keychain",
  "headless",
  "no-sandbox",
  "disable-setuid-sandbox",
  "disable-gpu",
  "hide-scrollbars",
  "mute-audio",
  "no-first-run",
  "no-default-browser-check",
  "disable-background-networking",
  "disable-background-timer-throttling",
  "disable-client-side-phishing-detection",
  "disable-component-update",
  "disable-default-apps",
  "disable-dev-shm-usage",
  "disable-features=Translate",
  "disable-hang-monitor",
  "disable-popup-blocking",
  "disable-prompt-on-repost",
  "disable-renderer-backgrounding",
  "disable-sync",
  "metrics-recording-only",
  "password-store=basic",
  "safebrowsing-disable-auto-update",
  "enable-automation",
  "force-color-profile=srgb",
  "disable-blink-features=AutomationControlled",
  "window-size=1280,720",
]

[generation_reports.full]
file = "all.json"
generated = 4
unsupported = 0
expected_fail = 0
quarantined = 0
failed_to_generate = 0

[source_roots.taffy]
kind = "taffy"
path = "html"
upstream_commit = "{revision}"
description = "Pinned upstream Taffy fixtures"

[source_roots.surgeist]
kind = "surgeist"
path = "html"
description = "Surgeist-authored fixtures"

[imports.taffy]
repo = "{TAFFY_REPOSITORY}"
commit = "{revision}"
source_dir = "test_fixtures"
destination = "html"
expected_count = 1
excluded_destination_dirs = ["grid-lanes", "subgrid"]
"#
    )
}
