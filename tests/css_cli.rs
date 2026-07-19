#![cfg(feature = "css-corpus")]

use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

const CSSTREE_REPOSITORY: &str = "https://github.com/csstree/csstree.git";
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestRoot(PathBuf);

impl TestRoot {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "surgeist-generator-css-cli-{}-{}",
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
fn css_cli_invalid_syntax_prints_exact_prefix_and_exits_64() {
    let output = Command::new(env!("CARGO_BIN_EXE_surgeist-css-generate"))
        .output()
        .expect("run packaged CSS binary");

    assert_eq!(output.status.code(), Some(64));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).expect("UTF-8 diagnostic"),
        "surgeist-css-generate: parse CSS command line: missing --owner-root\n"
    );
}

#[test]
fn css_cli_filter_syntax_and_option_matrix_precede_io() {
    for (command, extra, expected) in [
        (
            "generate",
            ["--filter", "."],
            "surgeist-css-generate: parse CSS command line: invalid --filter: .\n",
        ),
        (
            "generate",
            ["--filter", "generation-reports/all.json"],
            "surgeist-css-generate: construct CSS request: generate reserves generation-reports/all.json for the full report\n",
        ),
        (
            "generate",
            ["--source-root", "checkout"],
            "surgeist-css-generate: parse CSS command line: generate forbids --source-root\n",
        ),
        (
            "import-csstree",
            ["--filter", "declaration"],
            "surgeist-css-generate: parse CSS command line: import-csstree forbids --filter\n",
        ),
        (
            "check-corpus",
            ["--source-root", "checkout"],
            "surgeist-css-generate: parse CSS command line: check-corpus forbids --source-root\n",
        ),
        (
            "check-corpus",
            ["--filter", "declaration"],
            "surgeist-css-generate: parse CSS command line: check-corpus forbids --filter\n",
        ),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_surgeist-css-generate"))
            .args([
                "--owner-root",
                "does-not-exist",
                "--corpus-root",
                "does-not-exist",
                command,
            ])
            .args(extra)
            .output()
            .expect("run packaged CSS binary");

        assert_eq!(output.status.code(), Some(64));
        assert!(output.stdout.is_empty());
        assert_eq!(
            String::from_utf8(output.stderr).expect("UTF-8 diagnostic"),
            expected
        );
    }

    let accepted = Command::new(env!("CARGO_BIN_EXE_surgeist-css-generate"))
        .args([
            "--owner-root",
            "does-not-exist",
            "--corpus-root",
            "does-not-exist",
            "generate",
            "--filter",
            "declaration",
        ])
        .output()
        .expect("run packaged CSS binary");
    assert_eq!(accepted.status.code(), Some(1));
    assert!(accepted.stdout.is_empty());
    let diagnostic = String::from_utf8(accepted.stderr).expect("UTF-8 diagnostic");
    assert!(diagnostic.starts_with("surgeist-css-generate: canonicalize owner root:"));

    let accepted = Command::new(env!("CARGO_BIN_EXE_surgeist-css-generate"))
        .args([
            "--owner-root",
            "does-not-exist",
            "--corpus-root",
            "does-not-exist",
            "check-corpus",
        ])
        .output()
        .expect("run packaged CSS binary");
    assert_eq!(accepted.status.code(), Some(1));
    assert!(accepted.stdout.is_empty());
    let diagnostic = String::from_utf8(accepted.stderr).expect("UTF-8 diagnostic");
    assert!(diagnostic.starts_with("surgeist-css-generate: canonicalize owner root:"));
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn css_cli_import_csstree_executes_real_public_path() {
    let root = TestRoot::new();
    let owner = root.0.join("owner");
    let corpus = owner.join("corpus");
    let checkout = root.0.join("checkout");
    fs::create_dir_all(&corpus).expect("create corpus");
    fs::create_dir(&checkout).expect("create checkout");
    run_git(&checkout, &[OsStr::new("init"), OsStr::new("--quiet")]);
    for (key, value) in [
        ("user.name", "CSS CLI Test"),
        ("user.email", "css-cli@example.invalid"),
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
            OsStr::new(CSSTREE_REPOSITORY),
        ],
    );
    let fixture = checkout.join("fixtures/ast/declaration/Declaration.json");
    fs::create_dir_all(fixture.parent().expect("fixture parent")).expect("create fixture parent");
    fs::write(&fixture, b"{\"case\":{}}\n").expect("write fixture");
    run_git(&checkout, &[OsStr::new("add"), OsStr::new("fixtures/ast")]);
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
    fs::write(
        corpus.join("corpus.toml"),
        format!(
            "schema_version = 1\n\n[source]\nkind = \"csstree\"\nrepository = \"{CSSTREE_REPOSITORY}\"\nrevision = \"{revision}\"\nfixture_root = \"fixtures/ast\"\nimport_root = \"source\"\nexpected_files = 1\nexpected_cases = 1\n\n[artifacts]\nexpectation_root = \"expectations\"\nreport_file = \"expectations/generation-reports/all.json\"\n\n[[cases]]\nid = \"declaration/Declaration.json#/case\"\nstatus = \"active\"\n"
        ),
    )
    .expect("write manifest");

    let output = Command::new(env!("CARGO_BIN_EXE_surgeist-css-generate"))
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("import-csstree")
        .arg("--source-root")
        .arg(&checkout)
        .output()
        .expect("run packaged CSS import");
    assert!(
        output.status.success(),
        "CSS binary failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
    assert_eq!(
        fs::read(corpus.join("source/declaration/Declaration.json"))
            .expect("read imported fixture"),
        b"{\"case\":{}}\n"
    );
    assert!(corpus.join("source/.surgeist-source.json").is_file());
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
#[test]
fn css_cli_filtered_update_leaves_stale_check_until_full_generation() {
    let root = TestRoot::new();
    let owner = root.0.join("owner");
    let corpus = owner.join("corpus");
    let checkout = root.0.join("checkout");
    fs::create_dir_all(&corpus).expect("create corpus");
    fs::create_dir(&checkout).expect("create checkout");
    run_git(&checkout, &[OsStr::new("init"), OsStr::new("--quiet")]);
    for (key, value) in [
        ("user.name", "CSS CLI Generate Test"),
        ("user.email", "css-cli-generate@example.invalid"),
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
            OsStr::new(CSSTREE_REPOSITORY),
        ],
    );
    let fixture = checkout.join("fixtures/ast/declaration/Declaration.json");
    fs::create_dir_all(fixture.parent().expect("fixture parent")).expect("create fixture parent");
    fs::write(
        &fixture,
        b"{\"case\":{\"source\":\"a { color: red }\",\"ast\":{},\"generate\":\"a{color:red}\"}}\n",
    )
    .expect("write fixture");
    let other_fixture = checkout.join("fixtures/ast/value/Value.json");
    fs::create_dir_all(other_fixture.parent().expect("other fixture parent"))
        .expect("create other fixture parent");
    fs::write(
        &other_fixture,
        b"{\"case\":{\"source\":\"b {}\",\"ast\":{},\"generate\":\"b{}\"}}\n",
    )
    .expect("write other fixture");
    run_git(&checkout, &[OsStr::new("add"), OsStr::new("fixtures/ast")]);
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
    fs::write(
        corpus.join("corpus.toml"),
        format!(
            "schema_version = 1\n\n[source]\nkind = \"csstree\"\nrepository = \"{CSSTREE_REPOSITORY}\"\nrevision = \"{revision}\"\nfixture_root = \"fixtures/ast\"\nimport_root = \"source\"\nexpected_files = 2\nexpected_cases = 2\n\n[artifacts]\nexpectation_root = \"expectations\"\nreport_file = \"expectations/generation-reports/all.json\"\n"
        ),
    )
    .expect("write manifest");

    let import = Command::new(env!("CARGO_BIN_EXE_surgeist-css-generate"))
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("import-csstree")
        .arg("--source-root")
        .arg(&checkout)
        .output()
        .expect("run packaged CSS import");
    assert!(
        import.status.success(),
        "CSS import failed: {}",
        String::from_utf8_lossy(&import.stderr)
    );

    let generate = Command::new(env!("CARGO_BIN_EXE_surgeist-css-generate"))
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("generate")
        .output()
        .expect("run packaged CSS generation");
    assert!(
        generate.status.success(),
        "CSS generation failed: {}",
        String::from_utf8_lossy(&generate.stderr)
    );
    assert!(generate.stdout.is_empty());
    assert!(generate.stderr.is_empty());
    assert!(
        corpus
            .join("expectations/declaration/Declaration.json")
            .is_file()
    );
    assert!(
        corpus
            .join("expectations/generation-reports/all.json")
            .is_file()
    );

    let selected = corpus.join("expectations/declaration/Declaration.json");
    let unselected = corpus.join("expectations/value/Value.json");
    let report = corpus.join("expectations/generation-reports/all.json");
    let selected_bytes = fs::read(&selected).expect("read selected expectation");
    let unselected_bytes =
        String::from_utf8(fs::read(&unselected).expect("read unselected output"))
            .expect("UTF-8 unselected expectation");
    let report_bytes = fs::read(&report).expect("read full report");
    fs::write(&selected, b"stale selected expectation\n").expect("stale selected expectation");
    let stale_revision = if revision.starts_with('0') {
        "1".repeat(revision.len())
    } else {
        "0".repeat(revision.len())
    };
    let stale_unselected = unselected_bytes.replace(&revision, &stale_revision);
    assert_ne!(stale_unselected, unselected_bytes);
    fs::write(&unselected, stale_unselected.as_bytes()).expect("stale unselected expectation");

    let filtered = Command::new(env!("CARGO_BIN_EXE_surgeist-css-generate"))
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("generate")
        .arg("--filter")
        .arg("declaration/Declaration.json")
        .output()
        .expect("run packaged filtered CSS generation");
    assert!(
        filtered.status.success(),
        "filtered CSS generation failed: {}",
        String::from_utf8_lossy(&filtered.stderr)
    );
    assert!(filtered.stdout.is_empty());
    assert!(filtered.stderr.is_empty());
    assert_eq!(
        fs::read(selected).expect("read selected output"),
        selected_bytes
    );
    assert_eq!(
        fs::read(unselected).expect("read unselected output"),
        stale_unselected.as_bytes()
    );
    assert_eq!(
        fs::read(report).expect("read preserved report"),
        report_bytes
    );

    let stale_check = Command::new(env!("CARGO_BIN_EXE_surgeist-css-generate"))
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("check-corpus")
        .output()
        .expect("check filtered CSS generation");
    assert_eq!(stale_check.status.code(), Some(1));
    assert!(stale_check.stdout.is_empty());
    assert_eq!(
        String::from_utf8(stale_check.stderr).expect("UTF-8 stale-check diagnostic"),
        "surgeist-css-generate: check current CSS corpus: CSS expectation is stale: value/Value.json\n"
    );

    let repair = Command::new(env!("CARGO_BIN_EXE_surgeist-css-generate"))
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("generate")
        .output()
        .expect("repair the intentionally stale unselected expectation");
    assert!(
        repair.status.success(),
        "CSS repair generation failed: {}",
        String::from_utf8_lossy(&repair.stderr)
    );
    fs::remove_dir_all(&checkout).expect("remove source checkout before read-only check");
    let before = snapshot_tree(&corpus);

    let check = Command::new(env!("CARGO_BIN_EXE_surgeist-css-generate"))
        .arg("--owner-root")
        .arg(&owner)
        .arg("--corpus-root")
        .arg(&corpus)
        .arg("check-corpus")
        .output()
        .expect("run packaged CSS corpus check");
    assert!(
        check.status.success(),
        "CSS corpus check failed: {}",
        String::from_utf8_lossy(&check.stderr)
    );
    assert!(check.stdout.is_empty());
    assert!(check.stderr.is_empty());
    assert_eq!(snapshot_tree(&corpus), before);
}

fn snapshot_tree(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    fn visit(base: &Path, current: &Path, output: &mut Vec<(PathBuf, Vec<u8>)>) {
        let mut entries = fs::read_dir(current)
            .expect("read CLI snapshot directory")
            .collect::<std::result::Result<Vec<_>, _>>()
            .expect("read CLI snapshot entries");
        entries.sort_by_key(fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            if entry.file_type().expect("CLI snapshot entry type").is_dir() {
                visit(base, &path, output);
            } else {
                output.push((
                    path.strip_prefix(base)
                        .expect("CLI snapshot relative path")
                        .to_path_buf(),
                    fs::read(path).expect("read CLI snapshot file"),
                ));
            }
        }
    }

    let mut output = Vec::new();
    visit(root, root, &mut output);
    output
}
