use std::path::Path;

use crate::core::{ObjectFormat, SnapshotEntry, VerifiedSourceSnapshot};
use crate::{GeneratorErrorKind, PinnedSource, RelativePath, Sha256Digest, SourceRevision};

use super::{manifest, sidecar};

const SHA1_REVISION: &str = "1111111111111111111111111111111111111111";
const SHA256_REVISION: &str = "1111111111111111111111111111111111111111111111111111111111111111";

fn manifest_text(revision: &str, expected_count: usize, cases: &str) -> String {
    format!(
        r#"schema_version = 2

[browser]
source = "chrome-for-testing"
version = "123.0.1"
version_output = "Chrome for Testing 123.0.1"
cache_root = "browser-cache"
provenance_format = "Chrome {{version}} {{repository_relative_executable}}"

[browser.launch]
batch_size = 4
navigation_timeout_ms = 20000
dom_poll_interval_ms = 25
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

[[generation_reports.scoped]]
filter = "grid"
file = "grid.json"
generated = 4

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
repo = "https://github.com/DioxusLabs/taffy.git"
commit = "{revision}"
source_dir = "test_fixtures"
destination = "html"
expected_count = {expected_count}
excluded_destination_dirs = ["grid-lanes", "subgrid"]

{cases}"#
    )
}

fn authored_cases() -> &'static str {
    r#"[[cases]]
id = "authored/first"
source_root = "surgeist"
source = "authored/first.html"
generator = "constrained-html"
status = "active"

[[cases]]
id = "authored/second"
source_root = "surgeist"
source = "authored/second.html"
generator = "constrained-html"
status = "expected-fail"
reason = " known padded reason "

[[cases]]
id = ""
source_root = "taffy"
source = "compatibility/record.any"
generator = "constrained-html"
status = "quarantined"
reason = ""
"#
}

#[test]
fn layout_schema2_full_field_golden_is_accepted() {
    let (parsed, digest) = manifest::parse_with_launch_digest(
        manifest_text(SHA1_REVISION, 3, authored_cases()).as_bytes(),
        Path::new("corpus.toml"),
    )
    .expect("full schema-2 manifest");
    assert_eq!(parsed.revision.as_str(), SHA1_REVISION);
    assert_eq!(parsed.expected_source_files, 3);
    assert_eq!(
        parsed
            .authored_files
            .iter()
            .map(RelativePath::as_str)
            .collect::<Vec<_>>(),
        ["authored/first.html", "authored/second.html"]
    );
    assert_eq!(
        digest.as_str(),
        "1732a55f43798a0e9d8659ce35631291b5bf2bdb8a83faaee195de7ac45d374b"
    );
}

#[test]
fn layout_schema2_accepts_manifest_owned_taffy_pin_and_count() {
    for (revision, count) in [(SHA1_REVISION, 1), (SHA256_REVISION, 17)] {
        let parsed = manifest::parse(
            manifest_text(revision, count, "").as_bytes(),
            Path::new("corpus.toml"),
        )
        .expect("manifest-owned pin and count");
        assert_eq!(parsed.revision.as_str(), revision);
        assert_eq!(parsed.expected_source_files, count);
    }
}

#[test]
fn layout_schema2_preserves_taffy_compatibility_records_and_raw_reasons() {
    let parsed = manifest::parse(
        manifest_text(SHA1_REVISION, 1, authored_cases()).as_bytes(),
        Path::new("corpus.toml"),
    )
    .expect("compatibility case records");
    assert_eq!(parsed.authored_files.len(), 2);
}

#[test]
fn layout_schema2_rejects_mismatched_taffy_revision_fields() {
    let text = manifest_text(SHA1_REVISION, 1, "").replacen(
        &format!("commit = \"{SHA1_REVISION}\""),
        &format!("commit = \"{}\"", "2".repeat(40)),
        1,
    );
    let error = manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
        .expect_err("mismatched revisions");
    assert_eq!(error.kind(), GeneratorErrorKind::InvalidManifest);
}

#[test]
fn layout_schema2_rejects_noncanonical_retry_count() {
    let text = manifest_text(SHA1_REVISION, 1, "").replace("retry_count = 1", "retry_count = 2");
    let error = manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
        .expect_err("noncanonical retry count");
    assert_eq!(error.kind(), GeneratorErrorKind::InvalidManifest);
}

#[test]
fn layout_schema2_rejects_html_source_root_and_duplicate_ids_or_sources() {
    let base = authored_cases();
    for cases in [
        base.replacen("source_root = \"surgeist\"", "source_root = \"html\"", 1),
        format!(
            "{base}\n[[cases]]\nid = \"authored/first\"\nsource_root = \"taffy\"\nsource = \"other.html\"\ngenerator = \"constrained-html\"\nstatus = \"active\"\n"
        ),
        format!(
            "{base}\n[[cases]]\nid = \"other\"\nsource_root = \"taffy\"\nsource = \"authored/first.html\"\ngenerator = \"constrained-html\"\nstatus = \"active\"\n"
        ),
    ] {
        let error = manifest::parse(
            manifest_text(SHA1_REVISION, 1, &cases).as_bytes(),
            Path::new("corpus.toml"),
        )
        .expect_err("invalid source root or duplicate case identity");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidManifest);
    }
}

#[test]
fn layout_schema2_rejects_only_declared_tightening_shapes() {
    let valid = manifest_text(SHA1_REVISION, 1, authored_cases());
    let invalid = [
        valid.replace(
            "source = \"authored/first.html\"",
            "source = \"authored/./first.html\"",
        ),
        valid.replace(
            "excluded_destination_dirs = [\"grid-lanes\", \"subgrid\"]",
            "excluded_destination_dirs = [\"grid-lanes\", \"grid-lanes\"]",
        ),
        valid.replace("  \"headless\",", "  \"--use-mock-keychain\","),
        valid.replace("  \"use-mock-keychain\",", "  \"use-mock-keychain=value\","),
        valid.replace("  \"headless\",", "  \"path/bearing\","),
        valid.replace(
            "Chrome {version} {repository_relative_executable}",
            "Chrome {version} {version} {repository_relative_executable}",
        ),
        valid.replace(
            "Chrome {version} {repository_relative_executable}",
            "Chrome {version} {repository_relative_executable} {unknown}",
        ),
    ];
    for text in invalid {
        let error = manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
            .expect_err("declared tightening");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidManifest, "{text}");
    }
}

#[test]
fn layout_schema2_rejects_three_exclusions_with_duplicate_member() {
    let text = manifest_text(SHA1_REVISION, 1, "").replace(
        "excluded_destination_dirs = [\"grid-lanes\", \"subgrid\"]",
        "excluded_destination_dirs = [\"grid-lanes\", \"grid-lanes\", \"subgrid\"]",
    );
    let error = manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
        .expect_err("three-entry exclusion list with duplicate member");
    assert_eq!(error.kind(), GeneratorErrorKind::InvalidManifest);
}

#[test]
fn layout_schema2_rejects_noncanonical_scoped_report_files() {
    for replacement in [r#"file = "dir\\name.json""#, r#"file = " scoped.json""#] {
        let text =
            manifest_text(SHA1_REVISION, 1, "").replacen(r#"file = "grid.json""#, replacement, 1);
        let error = manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
            .expect_err("noncanonical scoped report file");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidManifest);
    }
}

#[test]
fn layout_schema2_launch_digest_preserves_manifest_order() {
    let original = manifest_text(SHA1_REVISION, 1, "");
    let reordered = original.replace(
        "  \"headless\",\n  \"no-sandbox\",",
        "  \"no-sandbox\",\n  \"headless\",",
    );
    let (_, original_digest) =
        manifest::parse_with_launch_digest(original.as_bytes(), Path::new("corpus.toml"))
            .expect("original launch");
    let (_, reordered_digest) =
        manifest::parse_with_launch_digest(reordered.as_bytes(), Path::new("corpus.toml"))
            .expect("reordered launch");
    assert_ne!(original_digest, reordered_digest);
}

fn snapshot(object_format: ObjectFormat, object_id: &str) -> VerifiedSourceSnapshot {
    let bytes = b"<div>fixture</div>\n".to_vec();
    VerifiedSourceSnapshot {
        object_format,
        entries: vec![SnapshotEntry {
            path: RelativePath::new("grid/basic.html").expect("fixture path"),
            git_mode: "100644".to_owned(),
            blob_object_id: object_id.to_owned(),
            digest: Sha256Digest::from_bytes(&bytes),
            bytes,
        }],
    }
}

fn pin(revision: &str) -> PinnedSource {
    PinnedSource::new(
        "taffy",
        manifest::TAFFY_REPOSITORY,
        SourceRevision::new(revision).expect("source revision"),
        RelativePath::new(manifest::TAFFY_SOURCE_DIRECTORY).expect("fixture root"),
    )
    .expect("Taffy pin")
}

#[test]
fn layout_taffy_sidecar_sha1_golden() {
    let snapshot = snapshot(
        ObjectFormat::Sha1,
        "2222222222222222222222222222222222222222",
    );
    let digest = snapshot.entries[0].digest.as_str();
    let expected = format!(
        "{{\"schema_version\":1,\"source\":{{\"label\":\"taffy\",\"repository_url\":\"{}\",\"revision\":\"{SHA1_REVISION}\",\"source_subdirectory\":\"test_fixtures\"}},\"object_format\":\"sha1\",\"source_file_count\":3,\"excluded_destination_dirs\":[\"grid-lanes\",\"subgrid\"],\"imported_file_count\":1,\"files\":[{{\"path\":\"grid/basic.html\",\"git_mode\":\"100644\",\"blob_object_id\":\"2222222222222222222222222222222222222222\",\"sha256\":\"{digest}\"}}]}}\n",
        manifest::TAFFY_REPOSITORY
    );
    assert_eq!(
        sidecar::canonical_bytes(&pin(SHA1_REVISION), 3, &snapshot).expect("SHA-1 sidecar"),
        expected.as_bytes()
    );
}

#[test]
fn layout_taffy_sidecar_sha256_golden() {
    let snapshot = snapshot(
        ObjectFormat::Sha256,
        "2222222222222222222222222222222222222222222222222222222222222222",
    );
    let bytes =
        sidecar::canonical_bytes(&pin(SHA256_REVISION), 1, &snapshot).expect("SHA-256 sidecar");
    let value: serde_json::Value = serde_json::from_slice(&bytes).expect("sidecar JSON");
    assert_eq!(value["object_format"], "sha256");
    assert_eq!(value["source"]["revision"].as_str().unwrap().len(), 64);
    assert_eq!(
        value["files"][0]["blob_object_id"].as_str().unwrap().len(),
        64
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod imports {
    use std::collections::{BTreeMap, BTreeSet};
    use std::ffi::OsStr;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::core::{Domain, GenerationLease};
    use crate::layout::LayoutRequest;
    use crate::{CorpusLocation, GeneratorErrorKind, RunScope};

    use super::{manifest, manifest_text};
    use crate::layout::importer;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        corpus: PathBuf,
        source: PathBuf,
        location: CorpusLocation,
        revision: String,
        authored: Vec<(&'static str, &'static [u8])>,
        source_files: Vec<(&'static str, &'static [u8], bool)>,
    }

    impl Fixture {
        fn new(source_files: Vec<(&'static str, &'static [u8], bool)>) -> Self {
            Self::new_with_object_format(source_files, false)
        }

        fn new_sha256(source_files: Vec<(&'static str, &'static [u8], bool)>) -> Self {
            Self::new_with_object_format(source_files, true)
        }

        fn new_with_object_format(
            source_files: Vec<(&'static str, &'static [u8], bool)>,
            sha256: bool,
        ) -> Self {
            let root = std::env::temp_dir().join(format!(
                "surgeist-generator-layout-import-{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            let owner = root.join("owner");
            let corpus = owner.join("corpus");
            let source = root.join("checkout");
            fs::create_dir_all(&corpus).expect("create corpus");
            fs::create_dir(&source).expect("create source root");
            if sha256 {
                run_git(
                    &source,
                    &[
                        OsStr::new("init"),
                        OsStr::new("--quiet"),
                        OsStr::new("--object-format=sha256"),
                    ],
                );
            } else {
                run_git(&source, &[OsStr::new("init"), OsStr::new("--quiet")]);
            }
            for (key, value) in [
                ("user.name", "Layout Test"),
                ("user.email", "layout@example.invalid"),
            ] {
                run_git(
                    &source,
                    &[OsStr::new("config"), OsStr::new(key), OsStr::new(value)],
                );
            }
            run_git(
                &source,
                &[
                    OsStr::new("remote"),
                    OsStr::new("add"),
                    OsStr::new("origin"),
                    OsStr::new(manifest::TAFFY_REPOSITORY),
                ],
            );
            write_source_files(&source, &source_files);
            run_git(
                &source,
                &[
                    OsStr::new("add"),
                    OsStr::new(manifest::TAFFY_SOURCE_DIRECTORY),
                ],
            );
            run_git(
                &source,
                &[
                    OsStr::new("commit"),
                    OsStr::new("--quiet"),
                    OsStr::new("-m"),
                    OsStr::new("fixtures"),
                ],
            );
            let revision = run_git(&source, &[OsStr::new("rev-parse"), OsStr::new("HEAD")]);
            let authored = vec![
                ("authored/first.html", b"<p>authored first</p>\n".as_slice()),
                (
                    "authored/second.html",
                    b"<p>authored second</p>\n".as_slice(),
                ),
            ];
            let fixture = Self {
                root,
                corpus,
                source,
                location: CorpusLocation::new(&owner, owner.join("corpus"))
                    .expect("corpus location"),
                revision,
                authored,
                source_files,
            };
            fixture.seed_authored();
            fixture.write_manifest();
            fixture
        }

        fn request(&self) -> LayoutRequest {
            LayoutRequest::import_taffy(self.location.clone(), self.source.clone())
                .expect("import request")
        }

        fn import(&self) -> crate::Result<()> {
            crate::layout::run(self.request())
        }

        fn check_request(&self) -> LayoutRequest {
            LayoutRequest::check_taffy_corpus(self.location.clone(), self.source.clone())
                .expect("Taffy check request")
        }

        fn check(&self) -> crate::Result<()> {
            crate::layout::run(self.check_request())
        }

        fn seed_authored(&self) {
            for (relative, bytes) in &self.authored {
                write_corpus_file(&self.corpus.join("html").join(relative), bytes);
            }
        }

        fn expected_count(&self) -> usize {
            self.source_files
                .iter()
                .filter(|(path, _, _)| Path::new(path).extension() == Some(OsStr::new("html")))
                .count()
        }

        fn write_manifest(&self) {
            write_corpus_file(
                &self.corpus.join("corpus.toml"),
                manifest_text(
                    &self.revision,
                    self.expected_count(),
                    super::authored_cases(),
                )
                .as_bytes(),
            );
        }

        fn replace_source(&mut self, files: Vec<(&'static str, &'static [u8], bool)>) {
            fs::remove_dir_all(self.source.join(manifest::TAFFY_SOURCE_DIRECTORY))
                .expect("remove old source fixtures");
            write_source_files(&self.source, &files);
            run_git(&self.source, &[OsStr::new("add"), OsStr::new("--all")]);
            run_git(
                &self.source,
                &[
                    OsStr::new("commit"),
                    OsStr::new("--quiet"),
                    OsStr::new("-m"),
                    OsStr::new("replace fixtures"),
                ],
            );
            self.revision = run_git(&self.source, &[OsStr::new("rev-parse"), OsStr::new("HEAD")]);
            self.source_files = files;
            self.write_manifest();
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).expect("remove layout fixture");
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

    fn write_source_files(root: &Path, files: &[(&str, &[u8], bool)]) {
        for (relative, bytes, executable) in files {
            let path = root.join(manifest::TAFFY_SOURCE_DIRECTORY).join(relative);
            fs::create_dir_all(path.parent().expect("source parent"))
                .expect("create source parent");
            fs::write(&path, bytes).expect("write source fixture");
            if *executable {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
                    .expect("set executable mode");
            }
        }
    }

    fn write_corpus_file(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().expect("corpus parent")).expect("create corpus parent");
        fs::write(path, bytes).expect("write corpus file");
        fs::set_permissions(path, fs::Permissions::from_mode(0o644)).expect("set corpus file mode");
        let mut parent = path.parent().expect("corpus parent");
        while parent.file_name().is_some() {
            fs::set_permissions(parent, fs::Permissions::from_mode(0o755))
                .expect("set corpus directory mode");
            parent = parent.parent().expect("parent");
            if parent.ends_with("owner") {
                break;
            }
        }
    }

    fn snapshot_tree(root: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
        fn visit(base: &Path, current: &Path, output: &mut BTreeMap<PathBuf, Vec<u8>>) {
            let mut entries = fs::read_dir(current)
                .expect("read snapshot directory")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("snapshot entries");
            entries.sort_by_key(fs::DirEntry::file_name);
            for entry in entries {
                let path = entry.path();
                let relative = path.strip_prefix(base).expect("relative snapshot path");
                if entry.file_type().expect("file type").is_dir() {
                    visit(base, &path, output);
                } else {
                    output.insert(
                        relative.to_path_buf(),
                        fs::read(path).expect("snapshot file"),
                    );
                }
            }
        }
        let mut output = BTreeMap::new();
        if root.exists() {
            visit(root, root, &mut output);
        }
        output
    }

    fn path_identity(path: &Path) -> (u64, u64) {
        let metadata = fs::symlink_metadata(path).expect("path identity");
        (metadata.dev(), metadata.ino())
    }

    fn snapshot_path_identities(root: &Path) -> BTreeMap<PathBuf, (u64, u64)> {
        fn visit(base: &Path, current: &Path, output: &mut BTreeMap<PathBuf, (u64, u64)>) {
            let metadata = fs::symlink_metadata(current).expect("snapshot path identity");
            output.insert(
                current
                    .strip_prefix(base)
                    .expect("relative snapshot identity path")
                    .to_path_buf(),
                (metadata.dev(), metadata.ino()),
            );
            if metadata.is_dir() {
                let mut entries = fs::read_dir(current)
                    .expect("read identity snapshot directory")
                    .collect::<std::result::Result<Vec<_>, _>>()
                    .expect("identity snapshot entries");
                entries.sort_by_key(fs::DirEntry::file_name);
                for entry in entries {
                    visit(base, &entry.path(), output);
                }
            }
        }

        let mut output = BTreeMap::new();
        visit(root, root, &mut output);
        output
    }

    fn assert_check_preserves(fixture: &Fixture, expected: Option<GeneratorErrorKind>) {
        let before_bytes = snapshot_tree(&fixture.root);
        let before_identities = snapshot_path_identities(&fixture.root);
        let before_root_identity = path_identity(&fixture.root);

        let result = fixture.check();
        match expected {
            Some(kind) => {
                let error = result.expect_err("Taffy corpus check must reject state");
                assert_eq!(error.kind(), kind, "unexpected check diagnostic: {error}");
            }
            None => result.expect("Taffy corpus check must accept current state"),
        }

        assert_eq!(snapshot_tree(&fixture.root), before_bytes);
        assert_eq!(snapshot_path_identities(&fixture.root), before_identities);
        assert_eq!(path_identity(&fixture.root), before_root_identity);
    }

    fn sidecar_paths(fixture: &Fixture) -> BTreeSet<String> {
        let bytes = fs::read(fixture.corpus.join("html/.surgeist-taffy-source.json"))
            .expect("read Taffy sidecar");
        let value: serde_json::Value = serde_json::from_slice(&bytes).expect("sidecar JSON");
        value["files"]
            .as_array()
            .expect("sidecar files")
            .iter()
            .map(|file| file["path"].as_str().expect("sidecar path").to_owned())
            .collect()
    }

    #[test]
    fn layout_taffy_legacy_nonempty_migration_adds_sidecar_and_preserves_authored_and_downstream() {
        let fixture = Fixture::new(vec![
            ("grid/current.html", b"<div>current</div>\n", false),
            ("nested/missing.html", b"<div>missing</div>\n", false),
            ("grid-lanes/excluded.html", b"excluded\n", false),
            ("notes.txt", b"not imported\n", false),
        ]);
        write_corpus_file(
            &fixture.corpus.join("html/grid/current.html"),
            b"<div>stale</div>\n",
        );
        write_corpus_file(&fixture.corpus.join("xml/sentinel.xml"), b"xml sentinel\n");
        let downstream_before = snapshot_tree(&fixture.corpus.join("xml"));
        let downstream_identity = path_identity(&fixture.corpus.join("xml"));
        let authored_before = fixture
            .authored
            .iter()
            .map(|(path, _)| {
                (
                    *path,
                    fs::read(fixture.corpus.join("html").join(path)).expect("authored bytes"),
                )
            })
            .collect::<Vec<_>>();

        fixture.import().expect("partition-safe import");

        assert_eq!(
            fs::read(fixture.corpus.join("html/grid/current.html")).unwrap(),
            b"<div>current</div>\n"
        );
        assert_eq!(
            fs::read(fixture.corpus.join("html/nested/missing.html")).unwrap(),
            b"<div>missing</div>\n"
        );
        assert!(
            !fixture
                .corpus
                .join("html/grid-lanes/excluded.html")
                .exists()
        );
        assert_eq!(
            sidecar_paths(&fixture),
            BTreeSet::from([
                "grid/current.html".to_owned(),
                "nested/missing.html".to_owned()
            ])
        );
        for (path, bytes) in authored_before {
            assert_eq!(
                fs::read(fixture.corpus.join("html").join(path)).unwrap(),
                bytes
            );
        }
        assert_eq!(
            snapshot_tree(&fixture.corpus.join("xml")),
            downstream_before
        );
        assert_eq!(
            path_identity(&fixture.corpus.join("xml")),
            downstream_identity
        );
    }

    #[test]
    fn layout_taffy_sidecar_mode_removes_only_classified_stale_taffy_files() {
        let mut fixture = Fixture::new(vec![
            ("old/removed.html", b"old\n", false),
            ("same/changed.html", b"before\n", false),
        ]);
        fixture.import().expect("initial import");
        fixture.replace_source(vec![
            ("new/added.html", b"new\n", false),
            ("same/changed.html", b"after\n", false),
        ]);
        fixture.import().expect("updated import");

        assert!(!fixture.corpus.join("html/old/removed.html").exists());
        assert_eq!(
            fs::read(fixture.corpus.join("html/new/added.html")).unwrap(),
            b"new\n"
        );
        assert_eq!(
            fs::read(fixture.corpus.join("html/same/changed.html")).unwrap(),
            b"after\n"
        );
        assert_eq!(
            sidecar_paths(&fixture),
            BTreeSet::from(["new/added.html".to_owned(), "same/changed.html".to_owned()])
        );
    }

    #[test]
    fn layout_taffy_legacy_unknown_file_is_not_guessed() {
        let fixture = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        write_corpus_file(&fixture.corpus.join("html/unknown.html"), b"unknown\n");
        let before = snapshot_tree(&fixture.corpus.join("html"));
        let error = fixture.import().expect_err("unknown legacy file");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert_eq!(snapshot_tree(&fixture.corpus.join("html")), before);
    }

    #[test]
    fn layout_taffy_authored_destination_collision_is_rejected() {
        let fixture = Fixture::new(vec![(
            "authored/first.html",
            b"upstream collision\n",
            false,
        )]);
        let before = snapshot_tree(&fixture.corpus.join("html"));
        let error = fixture.import().expect_err("authored/Taffy collision");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert_eq!(snapshot_tree(&fixture.corpus.join("html")), before);
    }

    #[test]
    fn layout_taffy_malformed_sidecar_never_falls_back() {
        let fixture = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        write_corpus_file(
            &fixture.corpus.join("html/.surgeist-taffy-source.json"),
            b"{}\n",
        );
        write_corpus_file(
            &fixture.corpus.join("html/grid/current.html"),
            b"legacy bytes\n",
        );
        let before = snapshot_tree(&fixture.corpus.join("html"));
        let error = fixture.import().expect_err("malformed sidecar");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert_eq!(snapshot_tree(&fixture.corpus.join("html")), before);
    }

    #[test]
    fn layout_taffy_source_pin_and_snapshot_drift_are_source_verification() {
        let fixture = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        fs::write(
            fixture.source.join("test_fixtures/grid/current.html"),
            b"dirty\n",
        )
        .expect("dirty source fixture");
        let error = fixture.import().expect_err("dirty source snapshot");
        assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
        assert!(
            !fixture
                .corpus
                .join("html/.surgeist-taffy-source.json")
                .exists()
        );
    }

    #[test]
    fn layout_taffy_source_replacement_after_preflight_fails_before_intent() {
        let fixture = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        let original = fixture.source.clone();
        let moved = fixture.root.join("moved-checkout");
        let request = fixture.request();
        let error = importer::run_with_pre_lease_hook(&request, || {
            fs::rename(&original, &moved).expect("move verified source");
            fs::create_dir(&original).expect("replace source name");
        })
        .expect_err("source replacement");
        assert!(matches!(
            error.kind(),
            GeneratorErrorKind::SourceVerification | GeneratorErrorKind::InvalidPath
        ));
        assert!(
            !fixture
                .corpus
                .join("html/.surgeist-taffy-source.json")
                .exists()
        );
    }

    #[test]
    fn layout_taffy_authored_revalidation_precedes_import_intent() {
        let fixture = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        let authored = fixture.corpus.join("html/authored/first.html");
        let request = fixture.request();
        let error = importer::run_with_pre_lease_hook(&request, || {
            fs::remove_file(&authored).expect("remove authored path");
            write_corpus_file(&authored, b"replacement\n");
        })
        .expect_err("authored replacement");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert!(
            !fixture
                .corpus
                .join("html/.surgeist-taffy-source.json")
                .exists()
        );
    }

    #[test]
    fn layout_taffy_wrong_mode_link_and_count_fail_before_publication() {
        let executable = Fixture::new(vec![("grid/current.html", b"current\n", true)]);
        assert_eq!(
            executable.import().unwrap_err().kind(),
            GeneratorErrorKind::InvalidInventory
        );

        let linked = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        let link = linked.source.join("test_fixtures/grid/link.html");
        symlink("current.html", &link).expect("create source symlink");
        run_git(&linked.source, &[OsStr::new("add"), OsStr::new("--all")]);
        run_git(
            &linked.source,
            &[
                OsStr::new("commit"),
                OsStr::new("--quiet"),
                OsStr::new("-m"),
                OsStr::new("link"),
            ],
        );
        let revision = run_git(
            &linked.source,
            &[OsStr::new("rev-parse"), OsStr::new("HEAD")],
        );
        write_corpus_file(
            &linked.corpus.join("corpus.toml"),
            manifest_text(&revision, 2, super::authored_cases()).as_bytes(),
        );
        assert_eq!(
            linked.import().unwrap_err().kind(),
            GeneratorErrorKind::InvalidInventory
        );

        let count = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        write_corpus_file(
            &count.corpus.join("corpus.toml"),
            manifest_text(&count.revision, 2, super::authored_cases()).as_bytes(),
        );
        assert_eq!(
            count.import().unwrap_err().kind(),
            GeneratorErrorKind::InvalidInventory
        );
    }

    #[test]
    fn layout_taffy_exclusion_alias_is_rejected_before_publication() {
        let aliased = Fixture::new(vec![("Grid-Lanes/aliased.html", b"alias\n", false)]);
        let error = aliased.import().expect_err("target-aliased exclusion");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert!(
            !aliased
                .corpus
                .join("html/.surgeist-taffy-source.json")
                .exists()
        );
    }

    #[test]
    fn layout_taffy_closing_inventory_revalidation_rejects_unknown_entry() {
        let fixture = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        let unknown = fixture.corpus.join("html/unknown.html");
        let request = fixture.request();
        let error = importer::run_with_inter_scan_hook(&request, || {
            write_corpus_file(&unknown, b"raced unknown\n");
        })
        .expect_err("inventory race");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert!(
            !fixture
                .corpus
                .join("html/.surgeist-taffy-source.json")
                .exists()
        );
    }

    #[test]
    fn layout_check_taffy_matching_sha1_and_sha256_imports_are_read_only() {
        for fixture in [
            Fixture::new(vec![("grid/sha1.html", b"sha1\n", false)]),
            Fixture::new_sha256(vec![("grid/sha256.html", b"sha256\n", false)]),
        ] {
            fixture.import().expect("seed current Taffy import");
            fs::remove_dir_all(fixture.corpus.join(".surgeist-generator"))
                .expect("remove completed coordination before read-only check");
            fs::write(
                fixture.root.join("outside-sentinel"),
                b"outside bytes remain unchanged\n",
            )
            .expect("write outside sentinel");

            assert_check_preserves(&fixture, None);
            assert!(!fixture.corpus.join(".surgeist-generator").exists());
        }
    }

    #[test]
    fn layout_check_taffy_checkout_pin_object_and_snapshot_drift_are_source_verification() {
        let dirty = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        dirty.import().expect("seed current import");
        fs::write(
            dirty.source.join("test_fixtures/grid/current.html"),
            b"dirty source bytes\n",
        )
        .expect("drift source snapshot");
        assert_check_preserves(&dirty, Some(GeneratorErrorKind::SourceVerification));

        let pin_drift = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        pin_drift.import().expect("seed current import");
        write_source_files(
            &pin_drift.source,
            &[("grid/added.html", b"new commit\n", false)],
        );
        run_git(&pin_drift.source, &[OsStr::new("add"), OsStr::new("--all")]);
        run_git(
            &pin_drift.source,
            &[
                OsStr::new("commit"),
                OsStr::new("--quiet"),
                OsStr::new("-m"),
                OsStr::new("advance checkout without manifest"),
            ],
        );
        assert_check_preserves(&pin_drift, Some(GeneratorErrorKind::SourceVerification));

        let sha1 = Fixture::new(vec![("grid/sha1.html", b"sha1\n", false)]);
        sha1.import().expect("seed SHA-1 import");
        let sha256 = Fixture::new_sha256(vec![("grid/sha256.html", b"sha256\n", false)]);
        let before_bytes = snapshot_tree(&sha1.root);
        let before_identities = snapshot_path_identities(&sha1.root);
        let request =
            LayoutRequest::check_taffy_corpus(sha1.location.clone(), sha256.source.clone())
                .expect("cross-format check request");
        let error = crate::layout::run(request).expect_err("object-format/pin drift");
        assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
        assert_eq!(snapshot_tree(&sha1.root), before_bytes);
        assert_eq!(snapshot_path_identities(&sha1.root), before_identities);
    }

    #[test]
    fn layout_check_taffy_absent_and_stale_known_imports_are_verification() {
        let absent = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        assert_check_preserves(&absent, Some(GeneratorErrorKind::Verification));

        let missing = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        missing.import().expect("seed current import");
        fs::remove_file(missing.corpus.join("html/grid/current.html"))
            .expect("remove known imported fixture");
        assert_check_preserves(&missing, Some(GeneratorErrorKind::Verification));

        let stale = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        stale.import().expect("seed current import");
        fs::write(
            stale.corpus.join("html/grid/current.html"),
            b"stale imported bytes\n",
        )
        .expect("drift imported fixture bytes");
        assert_check_preserves(&stale, Some(GeneratorErrorKind::Verification));
    }

    #[test]
    fn layout_check_taffy_malformed_and_unknown_inventory_are_invalid_inventory() {
        let malformed = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        malformed.import().expect("seed current import");
        fs::write(
            malformed.corpus.join("html/.surgeist-taffy-source.json"),
            b"{}\n",
        )
        .expect("malform Taffy sidecar");
        assert_check_preserves(&malformed, Some(GeneratorErrorKind::InvalidInventory));

        let unknown = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        unknown.import().expect("seed current import");
        write_corpus_file(&unknown.corpus.join("html/unknown.html"), b"unknown\n");
        assert_check_preserves(&unknown, Some(GeneratorErrorKind::InvalidInventory));
    }

    #[test]
    fn layout_check_taffy_malformed_sidecar_precedes_dirty_source_verification() {
        let malformed = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        malformed.import().expect("seed current import");
        fs::write(
            malformed.corpus.join("html/.surgeist-taffy-source.json"),
            b"{}\n",
        )
        .expect("malform Taffy sidecar");
        fs::write(
            malformed.source.join("test_fixtures/grid/current.html"),
            b"dirty source bytes\n",
        )
        .expect("drift source snapshot");

        assert_check_preserves(&malformed, Some(GeneratorErrorKind::InvalidInventory));
    }

    #[test]
    fn layout_check_taffy_unknown_inventory_precedes_wrong_checkout_verification() {
        let unknown = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        unknown.import().expect("seed current import");
        write_corpus_file(&unknown.corpus.join("html/unknown.html"), b"unknown\n");
        let wrong = Fixture::new(vec![("other/wrong.html", b"wrong checkout\n", false)]);
        let before_unknown_bytes = snapshot_tree(&unknown.root);
        let before_unknown_identities = snapshot_path_identities(&unknown.root);
        let before_wrong_bytes = snapshot_tree(&wrong.root);
        let before_wrong_identities = snapshot_path_identities(&wrong.root);
        let request =
            LayoutRequest::check_taffy_corpus(unknown.location.clone(), wrong.source.clone())
                .expect("wrong-checkout request");

        let error = crate::layout::run(request).expect_err("unknown import inventory");
        assert_eq!(
            error.kind(),
            GeneratorErrorKind::InvalidInventory,
            "unexpected check diagnostic: {error}"
        );
        assert_eq!(snapshot_tree(&unknown.root), before_unknown_bytes);
        assert_eq!(
            snapshot_path_identities(&unknown.root),
            before_unknown_identities
        );
        assert_eq!(snapshot_tree(&wrong.root), before_wrong_bytes);
        assert_eq!(
            snapshot_path_identities(&wrong.root),
            before_wrong_identities
        );
    }

    #[test]
    fn layout_read_only_taffy_coordination_states_are_verification_and_byte_identical() {
        let active = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        active.import().expect("seed current import");
        let lease = GenerationLease::acquire(
            &active.location,
            Domain::Layout,
            "surgeist-layout-generate",
            &RunScope::Full,
            "import-taffy",
        )
        .expect("hold active exclusive layout lease");
        assert_check_preserves(&active, Some(GeneratorErrorKind::Verification));
        drop(lease);

        let resumable = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        resumable.import().expect("seed current import");
        let active_transaction = resumable
            .corpus
            .join(".surgeist-generator/transactions/layout/active-read-only-test");
        fs::create_dir(&active_transaction).expect("create resumable transaction residue");
        fs::set_permissions(&active_transaction, fs::Permissions::from_mode(0o700))
            .expect("set private residue mode");
        assert_check_preserves(&resumable, Some(GeneratorErrorKind::Verification));

        let malformed = Fixture::new(vec![("grid/current.html", b"current\n", false)]);
        malformed.import().expect("seed current import");
        fs::write(
            malformed
                .corpus
                .join(".surgeist-generator/acquisition.lock"),
            b"malformed lock header\n",
        )
        .expect("malform immutable acquisition lock");
        assert_check_preserves(&malformed, Some(GeneratorErrorKind::Verification));
    }

    #[test]
    fn layout_taffy_pin_and_count_update_requires_reimport_not_generator_change() {
        let mut fixture = Fixture::new(vec![("grid/first.html", b"first\n", false)]);
        fixture.import().expect("import first manifest-owned pair");
        fixture.check().expect("check first manifest-owned pair");
        let first_sidecar = fs::read(fixture.corpus.join("html/.surgeist-taffy-source.json"))
            .expect("read first sidecar");

        fixture.replace_source(vec![
            ("grid/second.html", b"second\n", false),
            ("nested/third.html", b"third\n", false),
        ]);
        assert_check_preserves(&fixture, Some(GeneratorErrorKind::Verification));
        assert_eq!(
            fs::read(fixture.corpus.join("html/.surgeist-taffy-source.json"))
                .expect("old sidecar remains"),
            first_sidecar
        );

        fixture
            .import()
            .expect("reimport second manifest-owned pair");
        assert_check_preserves(&fixture, None);
        let second_sidecar = fs::read(fixture.corpus.join("html/.surgeist-taffy-source.json"))
            .expect("read second sidecar");
        let value: serde_json::Value =
            serde_json::from_slice(&second_sidecar).expect("second sidecar JSON");
        assert_ne!(second_sidecar, first_sidecar);
        assert_eq!(value["source"]["revision"], fixture.revision);
        assert_eq!(value["source_file_count"], 2);
        assert_eq!(value["imported_file_count"], 2);
    }
}
