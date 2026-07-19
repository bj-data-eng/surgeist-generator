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
