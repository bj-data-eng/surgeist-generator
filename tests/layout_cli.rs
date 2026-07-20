#![cfg(feature = "layout-browser")]

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use surgeist_generator::Sha256Digest;

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

fn layout_binary() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_surgeist-layout-generate"));
    command
        .env("SURGEIST_LAYOUT_OWNER_ROOT", "ignored/owner")
        .env("SURGEIST_LAYOUT_CORPUS_ROOT", "ignored/corpus")
        .env("SURGEIST_LAYOUT_SOURCE_ROOT", "ignored/source")
        .env("SURGEIST_LAYOUT_BROWSER_PATH", "ignored/browser")
        .env("SURGEIST_LAYOUT_FILTER", "ignored/filter");
    command
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
fn layout_cli_ignores_operator_environment_and_invalid_syntax_exits_64() {
    let output = layout_binary()
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
    for (arguments, diagnostic) in [
        (
            vec![
                "--owner-root",
                "does-not-exist",
                "--corpus-root",
                "does-not-exist",
                "import-taffy",
            ],
            "missing --source-root",
        ),
        (
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
            "duplicate --source-root",
        ),
        (
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
            "import-taffy forbids --filter",
        ),
        (
            vec![
                "--owner-root",
                "does-not-exist",
                "--corpus-root",
                "does-not-exist",
                "check-corpus",
                "--source-root",
                "checkout",
            ],
            "check-corpus forbids --source-root",
        ),
        (
            vec![
                "--owner-root",
                "does-not-exist",
                "--corpus-root",
                "does-not-exist",
                "check-taffy-corpus",
            ],
            "missing --source-root",
        ),
        (
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
            "check-taffy-corpus forbids --filter",
        ),
        (
            vec![
                "--owner-root",
                "does-not-exist",
                "--corpus-root",
                "does-not-exist",
                "generate",
            ],
            "missing --browser-path",
        ),
        (
            vec![
                "--owner-root",
                "does-not-exist",
                "--corpus-root",
                "does-not-exist",
                "check-corpus",
                "--browser-path",
                "cache/chrome",
            ],
            "check-corpus forbids --browser-path",
        ),
        (
            vec![
                "--owner-root",
                "does-not-exist",
                "--corpus-root",
                "does-not-exist",
                "check-corpus",
                "check-corpus",
            ],
            "duplicate layout command",
        ),
    ] {
        let output = layout_binary()
            .args(arguments)
            .output()
            .expect("run packaged layout binary");

        assert_eq!(output.status.code(), Some(64));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).expect("UTF-8 diagnostic"),
            format!("surgeist-layout-generate: parse layout command line: {diagnostic}\n")
        );
    }

    for command in ["import-taffy", "check-taffy-corpus"] {
        let accepted = layout_binary()
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
        assert_eq!(
            String::from_utf8(accepted.stderr).expect("UTF-8 diagnostic"),
            "surgeist-layout-generate: canonicalize owner root: unresolvable path: does-not-exist\n"
        );
    }

    let accepted = layout_binary()
        .args([
            "--owner-root",
            "does-not-exist",
            "--corpus-root",
            "does-not-exist",
            "check-corpus",
        ])
        .output()
        .expect("run packaged layout corpus check");
    assert_eq!(accepted.status.code(), Some(1));
    assert!(accepted.stdout.is_empty());
    assert_eq!(
        String::from_utf8(accepted.stderr).expect("UTF-8 diagnostic"),
        "surgeist-layout-generate: canonicalize owner root: unresolvable path: does-not-exist\n"
    );

    let accepted = layout_binary()
        .args([
            "--owner-root",
            "does-not-exist",
            "--corpus-root",
            "does-not-exist",
            "generate",
            "--browser-path",
            "cache/chrome",
            "--filter",
            "grid/case.html",
        ])
        .output()
        .expect("run packaged layout generation");
    assert_eq!(accepted.status.code(), Some(1));
    assert!(accepted.stdout.is_empty());
    assert_eq!(
        String::from_utf8(accepted.stderr).expect("UTF-8 diagnostic"),
        "surgeist-layout-generate: canonicalize owner root: unresolvable path: does-not-exist\n"
    );

    for (arguments, diagnostic) in [
        (
            vec![
                "--owner-root",
                "does-not-exist",
                "--corpus-root",
                "does-not-exist",
                "generate",
                "--browser-path",
                "cache/chrome",
                "--source-root",
                "checkout",
            ],
            "generate forbids --source-root",
        ),
        (
            vec![
                "--owner-root",
                "does-not-exist",
                "--corpus-root",
                "does-not-exist",
                "generate",
                "--browser-path",
                "cache/chrome",
                "--filter",
                ".surgeist-taffy-source.json",
            ],
            "generation filter uses the reserved Taffy sidecar path",
        ),
    ] {
        let output = layout_binary()
            .args(arguments)
            .output()
            .expect("run rejected packaged generation");
        assert_eq!(output.status.code(), Some(64));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).expect("UTF-8 diagnostic"),
            format!("surgeist-layout-generate: parse layout command line: {diagnostic}\n")
        );
    }
}

#[cfg(unix)]
#[test]
fn layout_cli_forwards_os_native_root_arguments_to_domain_validation() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let owner = OsString::from_vec(b"missing-owner-\x80".to_vec());
    let corpus = OsString::from_vec(b"missing-corpus-\x81".to_vec());
    let output = layout_binary()
        .arg("--owner-root")
        .arg(owner)
        .arg("--corpus-root")
        .arg(corpus)
        .arg("check-corpus")
        .output()
        .expect("run packaged layout binary with OS-native roots");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).starts_with(
            "surgeist-layout-generate: canonicalize owner root: unresolvable path: missing-owner-",
        ),
        "unexpected diagnostic: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn layout_cli_taffy_adoption_and_offline_checks_execute_real_public_paths() {
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
    fs::write(corpus.join("corpus.toml"), manifest_text(&revision, 1))
        .expect("write layout manifest");
    let legacy_fixture = corpus.join("html/grid/basic.html");
    fs::create_dir_all(legacy_fixture.parent().expect("legacy fixture parent"))
        .expect("create sidecar-free legacy partition");
    fs::write(&legacy_fixture, b"<div>stale legacy fixture</div>\n")
        .expect("write sidecar-free legacy fixture");

    let output = layout_binary()
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
    let output = layout_binary()
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

    write_current_layout_state(&corpus, &sidecar);
    let before = snapshot_tree(&root.0);
    let output = layout_binary()
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("check-corpus")
        .output()
        .expect("run packaged offline layout corpus check");
    assert!(
        output.status.success(),
        "offline layout check failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert_eq!(snapshot_tree(&root.0), before);

    let unchanged_import = layout_binary()
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("import-taffy")
        .arg("--source-root")
        .arg(&checkout)
        .output()
        .expect("rerun unchanged packaged layout import");
    assert!(
        unchanged_import.status.success(),
        "unchanged import failed: {}",
        String::from_utf8_lossy(&unchanged_import.stderr)
    );
    assert!(unchanged_import.stdout.is_empty());
    assert!(unchanged_import.stderr.is_empty());
    assert_eq!(
        fs::read(corpus.join("html/.surgeist-taffy-source.json")).unwrap(),
        sidecar
    );
    let unchanged_check = layout_binary()
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("check-corpus")
        .output()
        .expect("check downstream freshness after unchanged import");
    assert!(
        unchanged_check.status.success(),
        "unchanged import manufactured staleness: {}",
        String::from_utf8_lossy(&unchanged_check.stderr)
    );

    let excluded = checkout.join("test_fixtures/grid-lanes/excluded.html");
    fs::create_dir_all(excluded.parent().expect("excluded fixture parent"))
        .expect("create excluded fixture parent");
    fs::write(&excluded, b"<div>excluded from import</div>\n").expect("write excluded fixture");
    run_git(&checkout, &[OsStr::new("add"), OsStr::new("test_fixtures")]);
    run_git(
        &checkout,
        &[
            OsStr::new("commit"),
            OsStr::new("--quiet"),
            OsStr::new("-m"),
            OsStr::new("advance pin and pre-exclusion count"),
        ],
    );
    let updated_revision = run_git(&checkout, &[OsStr::new("rev-parse"), OsStr::new("HEAD")]);
    fs::write(
        corpus.join("corpus.toml"),
        manifest_text(&updated_revision, 2),
    )
    .expect("update manifest-owned pin and count");

    let stale_source_check = layout_binary()
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("check-taffy-corpus")
        .arg("--source-root")
        .arg(&checkout)
        .output()
        .expect("check stale import after manifest pin update");
    assert_eq!(stale_source_check.status.code(), Some(1));
    assert!(stale_source_check.stdout.is_empty());
    assert_eq!(
        String::from_utf8(stale_source_check.stderr).expect("UTF-8 stale-source diagnostic"),
        "surgeist-layout-generate: check Taffy corpus: Taffy import sidecar is stale against the manifest and named source\n"
    );

    let updated_import = layout_binary()
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("import-taffy")
        .arg("--source-root")
        .arg(&checkout)
        .output()
        .expect("reimport updated manifest-owned pin and count");
    assert!(
        updated_import.status.success(),
        "updated import failed: {}",
        String::from_utf8_lossy(&updated_import.stderr)
    );
    let updated_sidecar =
        fs::read(corpus.join("html/.surgeist-taffy-source.json")).expect("updated sidecar");
    let updated_value: serde_json::Value =
        serde_json::from_slice(&updated_sidecar).expect("updated sidecar JSON");
    assert_ne!(updated_sidecar, sidecar);
    assert_eq!(updated_value["source"]["revision"], updated_revision);
    assert_eq!(updated_value["source_file_count"], 2);
    assert_eq!(updated_value["imported_file_count"], 1);

    let updated_source_check = layout_binary()
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("check-taffy-corpus")
        .arg("--source-root")
        .arg(&checkout)
        .output()
        .expect("check updated Taffy import");
    assert!(
        updated_source_check.status.success(),
        "updated source check failed: {}",
        String::from_utf8_lossy(&updated_source_check.stderr)
    );

    let stale_corpus_check = layout_binary()
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("check-corpus")
        .output()
        .expect("check downstream freshness after changed import");
    assert_eq!(stale_corpus_check.status.code(), Some(1));
    assert!(stale_corpus_check.stdout.is_empty());
    assert_eq!(
        String::from_utf8(stale_corpus_check.stderr).expect("UTF-8 stale-corpus diagnostic"),
        "surgeist-layout-generate: check layout corpus: layout corpus is absent, stale, diagnostic, or migration-only; run a clean full generation\n"
    );

    write_current_layout_state(&corpus, &updated_sidecar);
    let refreshed_corpus_check = layout_binary()
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("check-corpus")
        .output()
        .expect("check synthetically refreshed corpus");
    assert!(
        refreshed_corpus_check.status.success(),
        "refreshed corpus check failed: {}",
        String::from_utf8_lossy(&refreshed_corpus_check.stderr)
    );

    fs::write(corpus.join("html/unknown.html"), b"unknown inventory\n")
        .expect("write unknown current inventory");
    fs::write(&fixture, b"<div>dirty checkout</div>\n").expect("dirty explicit checkout");
    let precedence = layout_binary()
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("import-taffy")
        .arg("--source-root")
        .arg(&checkout)
        .output()
        .expect("run import precedence sequence");
    assert_eq!(precedence.status.code(), Some(1));
    assert!(precedence.stdout.is_empty());
    assert_eq!(
        String::from_utf8(precedence.stderr).expect("UTF-8 precedence diagnostic"),
        "surgeist-layout-generate: validate layout Taffy import inventory: unknown entry in layout HTML root: unknown.html\n"
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn write_current_layout_state(corpus: &Path, sidecar: &[u8]) {
    const HELPER: &[u8] = b"window.__surgeist = true;\n";
    const BASE_STYLE: &[u8] = b"html, body { margin: 0; }\n";
    const BROWSER_DIGEST: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    let helper = corpus.join("scripts/gentest/test_helper.js");
    fs::create_dir_all(helper.parent().expect("helper parent")).expect("create helper parent");
    fs::write(&helper, HELPER).expect("write helper");
    fs::write(
        corpus.join("scripts/gentest/test_base_style.css"),
        BASE_STYLE,
    )
    .expect("write base style");

    let source = fs::read(corpus.join("html/grid/basic.html")).expect("read HTML source");
    let manifest = fs::read(corpus.join("corpus.toml")).expect("read manifest");
    let launch_arguments = vec![
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
    ];
    let launch = serde_json::to_vec(&(
        1_u8,
        1_u64,
        1_u64,
        1_u64,
        1_u64,
        "sorted-sequential",
        "open-load-reset-timeout",
        "per-batch-and-retry",
        "per-job",
        true,
        true,
        launch_arguments,
    ))
    .expect("serialize synthetic launch tuple");
    let metadata = (
        Sha256Digest::from_bytes(&launch),
        Sha256Digest::from_bytes(HELPER),
        Sha256Digest::from_bytes(BASE_STYLE),
        Sha256Digest::from_bytes(&manifest),
        Sha256Digest::from_bytes(sidecar),
        Sha256Digest::from_bytes(&source),
    );
    let mut generated = Vec::new();
    for variant in [
        "border_box_ltr",
        "border_box_rtl",
        "content_box_ltr",
        "content_box_rtl",
    ] {
        let output = format!("xml/grid/basic__{variant}.xml");
        let xml = format!(
            "<!-- generated-by: surgeist-layout-generate schema=2 source=\"html/grid/basic.html\" source-sha256=\"{}\" helper-sha256=\"{}\" browser=\"Chrome 123.0.1 browser-cache/chrome\" browser-executable-sha256=\"{BROWSER_DIGEST}\" launch-profile-sha256=\"{}\" corpus-manifest-sha256=\"{}\" taffy-revision=\"{}\" taffy-sidecar-sha256=\"{}\" -->\n<test name=\"basic__{variant}\"/>\n",
            metadata.5,
            metadata.1,
            metadata.0,
            metadata.3,
            serde_json::from_slice::<serde_json::Value>(sidecar)
                .expect("sidecar JSON")["source"]["revision"]
                .as_str()
                .expect("sidecar revision"),
            metadata.4,
        );
        let path = corpus.join(&output);
        fs::create_dir_all(path.parent().expect("XML parent")).expect("create XML parent");
        fs::write(&path, xml.as_bytes()).expect("write XML");
        generated.push(format!(
            "    {{\n      \"name\": \"basic__{variant}\",\n      \"source\": \"html/grid/basic.html\",\n      \"output\": \"{output}\",\n      \"output_sha256\": \"{}\",\n      \"variant\": \"{variant}\"\n    }}",
            Sha256Digest::from_bytes(xml.as_bytes())
        ));
    }
    let report = format!(
        "{{\n  \"metadata\": {{\n    \"schema_version\": 2,\n    \"generator\": \"surgeist-layout-generate\",\n    \"browser_source\": \"chrome-for-testing\",\n    \"browser_version\": \"123.0.1\",\n    \"browser_provenance\": \"Chrome 123.0.1 browser-cache/chrome\",\n    \"browser_executable_sha256\": \"{BROWSER_DIGEST}\",\n    \"launch_profile_sha256\": \"{}\",\n    \"helper_sha256\": \"{}\",\n    \"base_style_sha256\": \"{}\",\n    \"corpus_manifest_sha256\": \"{}\",\n    \"taffy_revision\": \"{}\",\n    \"taffy_sidecar_sha256\": \"{}\"\n  }},\n  \"filter\": null,\n  \"summary\": {{\n    \"generated\": 4,\n    \"unsupported\": 0,\n    \"expected_fail\": 0,\n    \"quarantined\": 0,\n    \"failed_to_generate\": 0\n  }},\n  \"generated\": [\n{}\n  ],\n  \"unsupported\": [],\n  \"expected_fail\": [],\n  \"quarantined\": [],\n  \"failed_to_generate\": []\n}}\n",
        metadata.0,
        metadata.1,
        metadata.2,
        metadata.3,
        serde_json::from_slice::<serde_json::Value>(sidecar)
            .expect("sidecar JSON")["source"]["revision"]
            .as_str()
            .expect("sidecar revision"),
        metadata.4,
        generated.join(",\n"),
    );
    let report_path = corpus.join("xml/generation-reports/all.json");
    fs::create_dir_all(report_path.parent().expect("report parent")).expect("create report parent");
    fs::write(report_path, report).expect("write report");
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn snapshot_tree(root: &Path) -> std::collections::BTreeMap<PathBuf, (u64, u64, Vec<u8>)> {
    fn visit(
        base: &Path,
        current: &Path,
        output: &mut std::collections::BTreeMap<PathBuf, (u64, u64, Vec<u8>)>,
    ) {
        use std::os::unix::fs::MetadataExt;

        let metadata = fs::symlink_metadata(current).expect("snapshot metadata");
        output.insert(
            current
                .strip_prefix(base)
                .expect("relative path")
                .to_path_buf(),
            (
                metadata.dev(),
                metadata.ino(),
                if metadata.is_file() {
                    fs::read(current).expect("snapshot bytes")
                } else {
                    Vec::new()
                },
            ),
        );
        if metadata.is_dir() {
            let mut entries = fs::read_dir(current)
                .expect("snapshot directory")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("snapshot entries");
            entries.sort_by_key(fs::DirEntry::file_name);
            for entry in entries {
                visit(base, &entry.path(), output);
            }
        }
    }
    let mut output = std::collections::BTreeMap::new();
    visit(root, root, &mut output);
    output
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn path_identity(path: &Path) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::symlink_metadata(path).expect("path identity");
    (metadata.dev(), metadata.ino())
}

fn manifest_text(revision: &str, expected_count: usize) -> String {
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
expected_count = {expected_count}
excluded_destination_dirs = ["grid-lanes", "subgrid"]
"#
    )
}
