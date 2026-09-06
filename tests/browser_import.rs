#![cfg(all(
    feature = "browser-corpus",
    target_os = "macos",
    target_arch = "aarch64"
))]

use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use surgeist_generator::browser::{
    SourceImportSpec, import_source, verify_import, verify_source_import,
};
use surgeist_generator::{
    CorpusLocation, GeneratorErrorKind, PinnedSource, RelativePath, SourceRevision,
};

const REPOSITORY: &str = "https://example.invalid/browser-fixtures.git";

struct Fixture {
    root: PathBuf,
    source: PathBuf,
    location: CorpusLocation,
    spec: SourceImportSpec,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "surgeist-browser-public-import-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let source = root.join("source");
        let corpus = root.join("corpus");
        fs::create_dir_all(source.join("fixtures")).unwrap();
        fs::create_dir_all(corpus.join("html/authored")).unwrap();
        fs::write(source.join("fixtures/imported.html"), b"imported\n").unwrap();
        fs::write(corpus.join("html/authored/local.html"), b"authored\n").unwrap();
        for args in [
            vec!["init", "--quiet"],
            vec!["config", "user.name", "Browser Import Test"],
            vec!["config", "user.email", "test@example.invalid"],
            vec!["remote", "add", "origin", REPOSITORY],
            vec!["add", "fixtures"],
            vec!["commit", "--quiet", "-m", "initial fixture"],
        ] {
            git(&source, &args);
        }
        let spec = specification(&source, 1);
        let location = CorpusLocation::new(&corpus, &corpus).unwrap();
        Self {
            root,
            source,
            location,
            spec,
        }
    }

    fn artifact(&self, name: &str) -> PathBuf {
        self.location.corpus_root().join("html").join(name)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

fn specification(source: &Path, count: usize) -> SourceImportSpec {
    SourceImportSpec::new(
        PinnedSource::new(
            "fixtures",
            REPOSITORY,
            SourceRevision::new(git(source, &["rev-parse", "HEAD"]).trim()).unwrap(),
            RelativePath::new("fixtures").unwrap(),
        )
        .unwrap(),
        RelativePath::new("html").unwrap(),
        RelativePath::new(".surgeist-source.json").unwrap(),
        "html".to_owned(),
        count,
        vec![RelativePath::new("authored").unwrap()],
        vec![RelativePath::new("authored/local.html").unwrap()],
    )
    .unwrap()
}

fn git(source: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(source)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn file_state(path: &Path) -> (u64, u64, Vec<u8>) {
    let metadata = fs::symlink_metadata(path).unwrap();
    (metadata.dev(), metadata.ino(), fs::read(path).unwrap())
}

#[test]
fn source_adoption_and_unchanged_reimport_preserve_published_file_identity() {
    let fixture = Fixture::new();
    fs::write(fixture.artifact("imported.html"), b"stale legacy bytes\n").unwrap();
    let first = import_source(&fixture.location, &fixture.spec, &fixture.source).unwrap();
    assert_eq!(
        fs::read(fixture.artifact("imported.html")).unwrap(),
        b"imported\n"
    );
    let paths = [
        "imported.html",
        ".surgeist-source.json",
        "authored/local.html",
    ];
    let before = paths.map(|path| file_state(&fixture.artifact(path)));

    let repeated = import_source(&fixture.location, &fixture.spec, &fixture.source).unwrap();
    assert_eq!(
        first.canonical_bytes().unwrap(),
        repeated.canonical_bytes().unwrap()
    );
    assert_eq!(
        paths.map(|path| file_state(&fixture.artifact(path))),
        before
    );
    assert_eq!(
        verify_import(&fixture.location, &fixture.spec).unwrap(),
        repeated
    );
    assert_eq!(
        verify_source_import(&fixture.location, &fixture.spec, &fixture.source).unwrap(),
        repeated
    );
}

#[test]
fn changed_source_pin_requires_reimport_even_when_selected_fixture_bytes_match() {
    let fixture = Fixture::new();
    let first = import_source(&fixture.location, &fixture.spec, &fixture.source).unwrap();
    fs::create_dir_all(fixture.source.join("fixtures/authored")).unwrap();
    fs::write(
        fixture.source.join("fixtures/authored/excluded.html"),
        b"excluded\n",
    )
    .unwrap();
    git(&fixture.source, &["add", "fixtures"]);
    git(
        &fixture.source,
        &["commit", "--quiet", "-m", "advance source pin"],
    );
    let advanced = specification(&fixture.source, 2);
    assert_ne!(advanced.pin().revision(), fixture.spec.pin().revision());
    let before = file_state(&fixture.artifact(".surgeist-source.json"));
    assert!(verify_import(&fixture.location, &advanced).is_err());
    assert!(verify_source_import(&fixture.location, &advanced, &fixture.source).is_err());
    assert_eq!(
        file_state(&fixture.artifact(".surgeist-source.json")),
        before
    );

    let refreshed = import_source(&fixture.location, &advanced, &fixture.source).unwrap();
    assert_eq!(refreshed.source(), advanced.pin());
    assert_ne!(
        refreshed.canonical_bytes().unwrap(),
        first.canonical_bytes().unwrap()
    );
    assert_eq!(refreshed.files().len(), 1);
    assert_eq!(
        fs::read(fixture.artifact("imported.html")).unwrap(),
        b"imported\n"
    );
    assert_eq!(
        fs::read(fixture.artifact("authored/local.html")).unwrap(),
        b"authored\n"
    );
    assert!(!fixture.artifact("authored/excluded.html").exists());
    assert_eq!(
        verify_import(&fixture.location, &advanced).unwrap(),
        refreshed
    );
    assert_eq!(
        verify_source_import(&fixture.location, &advanced, &fixture.source).unwrap(),
        refreshed
    );
    assert!(verify_import(&fixture.location, &fixture.spec).is_err());
}

#[test]
fn unknown_corpus_inventory_is_rejected_before_a_dirty_named_source() {
    let fixture = Fixture::new();
    import_source(&fixture.location, &fixture.spec, &fixture.source).unwrap();
    fs::write(fixture.artifact("unknown.html"), b"unknown inventory\n").unwrap();
    fs::write(
        fixture.source.join("fixtures/imported.html"),
        b"dirty checkout\n",
    )
    .unwrap();
    let paths = [
        "imported.html",
        ".surgeist-source.json",
        "authored/local.html",
        "unknown.html",
    ];
    let before = paths.map(|path| file_state(&fixture.artifact(path)));

    let error = import_source(&fixture.location, &fixture.spec, &fixture.source).unwrap_err();
    assert_eq!(
        error.kind(),
        GeneratorErrorKind::InvalidInventory,
        "{error}"
    );
    assert!(error.to_string().contains("unknown.html"), "{error}");
    assert_eq!(
        paths.map(|path| file_state(&fixture.artifact(path))),
        before
    );
}
