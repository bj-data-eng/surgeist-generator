use std::path::Path;

use crate::core::{ObjectFormat, SnapshotEntry, VerifiedSourceSnapshot};
use crate::{GeneratorErrorKind, PinnedSource, RelativePath, Sha256Digest, SourceRevision};

use super::{manifest, sidecar};

const CSSTREE_REPOSITORY: &str = "https://github.com/csstree/csstree.git";
const SHA1_REVISION: &str = "1111111111111111111111111111111111111111";
const SHA256_REVISION: &str = "1111111111111111111111111111111111111111111111111111111111111111";

fn manifest_text(revision: &str, expected_files: usize) -> String {
    format!(
        "schema_version = 1\n\
         \n\
         [source]\n\
         kind = \"csstree\"\n\
         repository = \"{CSSTREE_REPOSITORY}\"\n\
         revision = \"{revision}\"\n\
         fixture_root = \"fixtures/ast\"\n\
         import_root = \"source\"\n\
         expected_files = {expected_files}\n\
         expected_cases = 1\n\
         \n\
         [artifacts]\n\
         expectation_root = \"expectations\"\n\
         report_file = \"expectations/generation-reports/all.json\"\n\
         \n\
         [[cases]]\n\
         id = \"declaration/Declaration.json#/case\"\n\
         status = \"active\"\n"
    )
}

#[test]
fn css_manifest_valid_schema_1_matrix_is_accepted() {
    let mut valid = vec![manifest_text(SHA1_REVISION, 1)];
    valid.push(
        manifest_text(SHA1_REVISION, 1)
            .split("[[cases]]")
            .next()
            .expect("manifest prefix")
            .to_owned(),
    );
    valid.push(manifest_text(SHA256_REVISION, 1).replace(
        "status = \"active\"",
        "status = \"expected-fail\"\nreason = \"known upstream mismatch\"",
    ));

    for text in valid {
        manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
            .expect("valid schema-1 CSS manifest");
    }
}

#[test]
fn css_manifest_invalid_schema_1_matrix_is_rejected() {
    let valid = manifest_text(SHA1_REVISION, 1);
    let invalid = [
        valid.replace("schema_version = 1", "schema_version = 2"),
        format!("schema_version = 1\nschema_version = 1\n{}", &valid[19..]),
        valid.replace("kind = \"csstree\"", "kind = \"other\""),
        valid.replace(CSSTREE_REPOSITORY, "http://github.com/csstree/csstree.git"),
        valid.replace(SHA1_REVISION, "ABCDEF"),
        valid.replace(
            "fixture_root = \"fixtures/ast\"",
            "fixture_root = \"fixtures/other\"",
        ),
        valid.replace(
            "import_root = \"source\"",
            "import_root = \"nested/source\"",
        ),
        valid.replace(
            "expectation_root = \"expectations\"",
            "expectation_root = \"source\"",
        ),
        valid.replace("expected_files = 1", "expected_files = 0"),
        valid.replace("expected_cases = 1", "expected_cases = 0"),
        valid.replace(
            "report_file = \"expectations/generation-reports/all.json\"",
            "report_file = \"expectations/report.json\"",
        ),
        valid.replace(
            "status = \"active\"",
            "status = \"active\"\nreason = \"not allowed\"",
        ),
        valid.replace("status = \"active\"", "status = \"unsupported\""),
        valid.replace(
            "id = \"declaration/Declaration.json#/case\"",
            "id = \"declaration/Declaration.json#/case\"\nid = \"duplicate\"",
        ),
        format!(
            "{valid}\n[[cases]]\nid = \"declaration/Declaration.json#/case\"\nstatus = \"active\"\n"
        ),
        format!("{valid}unknown = true\n"),
    ];

    for text in invalid {
        let error = manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
            .expect_err("invalid schema-1 CSS manifest");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidManifest, "{text}");
    }
}

fn snapshot(object_format: ObjectFormat, blob_object_id: &str) -> VerifiedSourceSnapshot {
    let bytes = b"{\"case\":{}}\n".to_vec();
    VerifiedSourceSnapshot {
        object_format,
        entries: vec![SnapshotEntry {
            path: RelativePath::new("declaration/Declaration.json").expect("fixture path"),
            git_mode: "100644".to_owned(),
            blob_object_id: blob_object_id.to_owned(),
            digest: Sha256Digest::from_bytes(&bytes),
            bytes,
        }],
    }
}

fn pin(revision: &str) -> PinnedSource {
    PinnedSource::new(
        "csstree",
        CSSTREE_REPOSITORY,
        SourceRevision::new(revision).expect("source revision"),
        RelativePath::new("fixtures/ast").expect("fixture root"),
    )
    .expect("source pin")
}

#[test]
fn css_import_sidecar_sha1_golden() {
    let snapshot = snapshot(
        ObjectFormat::Sha1,
        "2222222222222222222222222222222222222222",
    );
    let digest = snapshot.entries[0].digest.as_str();
    let expected = format!(
        "{{\"schema_version\":1,\"source\":{{\"label\":\"csstree\",\"repository_url\":\"{CSSTREE_REPOSITORY}\",\"revision\":\"{SHA1_REVISION}\",\"source_subdirectory\":\"fixtures/ast\"}},\"object_format\":\"sha1\",\"file_count\":1,\"files\":[{{\"path\":\"declaration/Declaration.json\",\"git_mode\":\"100644\",\"blob_object_id\":\"2222222222222222222222222222222222222222\",\"sha256\":\"{digest}\"}}]}}\n"
    );
    assert_eq!(
        sidecar::canonical_bytes(&pin(SHA1_REVISION), &snapshot).expect("SHA-1 sidecar"),
        expected.as_bytes()
    );
}

#[test]
fn css_import_sidecar_sha256_golden() {
    let snapshot = snapshot(
        ObjectFormat::Sha256,
        "2222222222222222222222222222222222222222222222222222222222222222",
    );
    let digest = snapshot.entries[0].digest.as_str();
    let expected = format!(
        "{{\"schema_version\":1,\"source\":{{\"label\":\"csstree\",\"repository_url\":\"{CSSTREE_REPOSITORY}\",\"revision\":\"{SHA256_REVISION}\",\"source_subdirectory\":\"fixtures/ast\"}},\"object_format\":\"sha256\",\"file_count\":1,\"files\":[{{\"path\":\"declaration/Declaration.json\",\"git_mode\":\"100644\",\"blob_object_id\":\"2222222222222222222222222222222222222222222222222222222222222222\",\"sha256\":\"{digest}\"}}]}}\n"
    );
    assert_eq!(
        sidecar::canonical_bytes(&pin(SHA256_REVISION), &snapshot).expect("SHA-256 sidecar"),
        expected.as_bytes()
    );
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod imports {
    use std::collections::{BTreeMap, BTreeSet};
    use std::ffi::OsStr;
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::css::{CssCommand, CssRequest};
    use crate::{
        ArtifactProvenance, CorpusLocation, GenerationCounts, GenerationReport, GeneratorErrorKind,
        ManifestVersion, RelativePath, ReportArtifact, Sha256Digest, SourceRevision,
    };

    use super::{CSSTREE_REPOSITORY, manifest_text};
    use crate::css::importer;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
        corpus: PathBuf,
        source: PathBuf,
        location: CorpusLocation,
        revision: String,
    }

    impl Fixture {
        fn new(files: &[(&str, &[u8], bool)]) -> Self {
            Self::new_with_object_format(files, None)
        }

        fn new_sha256(files: &[(&str, &[u8], bool)]) -> Self {
            Self::new_with_object_format(files, Some("sha256"))
        }

        fn new_with_object_format(
            files: &[(&str, &[u8], bool)],
            object_format: Option<&str>,
        ) -> Self {
            let root = std::env::temp_dir().join(format!(
                "surgeist-generator-css-import-{}-{}",
                std::process::id(),
                NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
            ));
            let owner = root.join("owner");
            let corpus = owner.join("corpus");
            let source = root.join("checkout");
            fs::create_dir_all(&corpus).expect("create corpus");
            fs::create_dir(&source).expect("create source root");
            let mut init = vec![OsStr::new("init"), OsStr::new("--quiet")];
            let format_argument;
            if let Some(object_format) = object_format {
                format_argument = format!("--object-format={object_format}");
                init.push(OsStr::new(&format_argument));
            }
            run_git(&source, &init);
            run_git(
                &source,
                &[
                    OsStr::new("config"),
                    OsStr::new("user.name"),
                    OsStr::new("CSS Test"),
                ],
            );
            run_git(
                &source,
                &[
                    OsStr::new("config"),
                    OsStr::new("user.email"),
                    OsStr::new("css@example.invalid"),
                ],
            );
            run_git(
                &source,
                &[
                    OsStr::new("remote"),
                    OsStr::new("add"),
                    OsStr::new("origin"),
                    OsStr::new(CSSTREE_REPOSITORY),
                ],
            );
            write_source_files(&source, files);
            run_git(&source, &[OsStr::new("add"), OsStr::new("fixtures/ast")]);
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
            fs::write(
                corpus.join("corpus.toml"),
                manifest_text(&revision, files.len()),
            )
            .expect("write manifest");
            let location = CorpusLocation::new(&owner, &corpus).expect("corpus location");
            Self {
                root,
                corpus,
                source,
                location,
                revision,
            }
        }

        fn request(&self) -> CssRequest {
            CssRequest::new(
                self.location.clone(),
                CssCommand::ImportCsstree,
                Some(self.source.clone()),
                None,
            )
            .expect("import request")
        }

        fn import(&self) -> crate::Result<()> {
            crate::css::run(self.request())
        }

        fn replace_commit(&mut self, files: &[(&str, &[u8], bool)]) {
            fs::remove_dir_all(self.source.join("fixtures/ast")).expect("remove old fixtures");
            write_source_files(&self.source, files);
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
            fs::write(
                self.corpus.join("corpus.toml"),
                manifest_text(&self.revision, files.len()),
            )
            .expect("update manifest");
        }

        fn replace_fixture_with_symlink(&mut self, relative: &str, target: &str) {
            let path = self.source.join("fixtures/ast").join(relative);
            fs::remove_file(&path).expect("remove regular fixture");
            symlink(target, &path).expect("create fixture symlink");
            run_git(&self.source, &[OsStr::new("add"), OsStr::new("--all")]);
            run_git(
                &self.source,
                &[
                    OsStr::new("commit"),
                    OsStr::new("--quiet"),
                    OsStr::new("-m"),
                    OsStr::new("replace fixture with symlink"),
                ],
            );
            self.revision = run_git(&self.source, &[OsStr::new("rev-parse"), OsStr::new("HEAD")]);
            fs::write(
                self.corpus.join("corpus.toml"),
                manifest_text(&self.revision, 1),
            )
            .expect("update manifest");
        }

        fn imported(&self, relative: &str) -> PathBuf {
            self.corpus.join("source").join(relative)
        }

        fn sidecar(&self) -> Vec<u8> {
            fs::read(self.imported(".surgeist-source.json")).expect("read sidecar")
        }

        fn seed_downstream(&self, fixture: &str) -> DownstreamProof {
            let sidecar_digest = Sha256Digest::from_bytes(self.sidecar());
            let output_bytes = b"{\"sentinel\":\"preserve downstream bytes\"}\n".to_vec();
            let output = self.corpus.join("expectations").join(fixture);
            fs::create_dir_all(output.parent().expect("output parent"))
                .expect("create expectation parent");
            fs::write(&output, &output_bytes).expect("write expectation");

            let mut domain = BTreeMap::new();
            domain.insert("csstree-import".to_owned(), sidecar_digest.clone());
            let source_path =
                RelativePath::new(format!("source/{fixture}")).expect("report source path");
            let provenance = ArtifactProvenance::new(
                source_path,
                Sha256Digest::from_file(self.imported(fixture)).expect("source digest"),
                "surgeist-css-generate",
                ManifestVersion::new(1).expect("schema"),
                domain,
            )
            .expect("provenance");
            let artifact = ReportArtifact::new(
                provenance,
                RelativePath::new(format!("expectations/{fixture}")).expect("report output path"),
                Sha256Digest::from_bytes(&output_bytes),
                1,
            )
            .expect("report artifact");
            let report = GenerationReport::new(
                Sha256Digest::from_file(self.corpus.join("corpus.toml")).expect("manifest digest"),
                CSSTREE_REPOSITORY,
                SourceRevision::new(&self.revision).expect("report revision"),
                GenerationCounts::new(1, 0, 0, 0, 0).expect("counts"),
                vec![artifact],
            )
            .expect("generation report");
            let mut report_bytes = serde_json::to_vec_pretty(&report).expect("serialize report");
            report_bytes.push(b'\n');
            let report_path = self.corpus.join("expectations/generation-reports/all.json");
            fs::create_dir_all(report_path.parent().expect("report parent"))
                .expect("create report parent");
            fs::write(report_path, &report_bytes).expect("write report");

            DownstreamProof {
                sidecar_digest,
                bytes: snapshot_tree(&self.corpus.join("expectations")),
                report_bytes,
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).expect("remove CSS fixture");
        }
    }

    struct DownstreamProof {
        sidecar_digest: Sha256Digest,
        bytes: BTreeMap<PathBuf, Vec<u8>>,
        report_bytes: Vec<u8>,
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
            let path = root.join("fixtures/ast").join(relative);
            fs::create_dir_all(path.parent().expect("fixture parent"))
                .expect("create fixture parent");
            fs::write(&path, bytes).expect("write fixture");
            if *executable {
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755))
                    .expect("make fixture executable");
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

    fn fixture_paths(sidecar: &[u8]) -> BTreeSet<String> {
        let value: serde_json::Value = serde_json::from_slice(sidecar).expect("sidecar JSON");
        value["files"]
            .as_array()
            .expect("sidecar files")
            .iter()
            .map(|record| record["path"].as_str().expect("sidecar path").to_owned())
            .collect()
    }

    #[test]
    fn css_import_publishes_exact_sidecar_and_snapshot_atomically() {
        let fixture = Fixture::new(&[
            ("declaration/Declaration.json", b"{\"case\":{}}\n", false),
            ("selector/Selector.json", b"{\"selector\":{}}\n", false),
        ]);
        fixture.import().expect("import fixtures");

        let tree = snapshot_tree(&fixture.corpus.join("source"));
        assert_eq!(
            tree.keys().collect::<Vec<_>>(),
            [
                Path::new(".surgeist-source.json"),
                Path::new("declaration/Declaration.json"),
                Path::new("selector/Selector.json"),
            ]
        );
        assert_eq!(
            fixture_paths(&fixture.sidecar()),
            BTreeSet::from([
                "declaration/Declaration.json".to_owned(),
                "selector/Selector.json".to_owned(),
            ])
        );
    }

    #[test]
    fn css_import_sha256_checkout_publishes_full_object_ids() {
        let fixture =
            Fixture::new_sha256(&[("declaration/Declaration.json", b"{\"case\":{}}\n", false)]);
        fixture.import().expect("import SHA-256 repository");

        let sidecar: serde_json::Value =
            serde_json::from_slice(&fixture.sidecar()).expect("sidecar JSON");
        assert_eq!(sidecar["object_format"], "sha256");
        assert_eq!(
            sidecar["source"]["revision"]
                .as_str()
                .expect("revision")
                .len(),
            64
        );
        assert_eq!(
            sidecar["files"][0]["blob_object_id"]
                .as_str()
                .expect("blob object ID")
                .len(),
            64
        );
    }

    #[test]
    fn css_import_rejects_manifest_file_count_mismatch_before_publication() {
        let fixture = Fixture::new(&[("declaration/Declaration.json", b"{\"case\":{}}\n", false)]);
        fs::write(
            fixture.corpus.join("corpus.toml"),
            manifest_text(&fixture.revision, 2),
        )
        .expect("write mismatched manifest");
        let error = fixture.import().expect_err("file count mismatch");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert!(!fixture.corpus.join("source").exists());
    }

    #[test]
    fn css_import_rejects_non_100644_json() {
        let fixture = Fixture::new(&[("declaration/Declaration.json", b"{\"case\":{}}\n", true)]);
        let error = fixture.import().expect_err("executable JSON fixture");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert!(!fixture.corpus.join("source").exists());
    }

    #[test]
    fn css_import_rejects_symlink_fixture_as_invalid_inventory() {
        let mut fixture =
            Fixture::new(&[("declaration/Declaration.json", b"{\"case\":{}}\n", false)]);
        fixture.replace_fixture_with_symlink("declaration/Declaration.json", "missing.json");

        let error = fixture.import().expect_err("symlink fixture");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert!(!fixture.corpus.join("source").exists());
    }

    #[test]
    fn css_import_rejects_non_json_fixture_as_invalid_inventory() {
        let fixture = Fixture::new(&[("declaration/Declaration.txt", b"not JSON\n", false)]);

        let error = fixture.import().expect_err("non-JSON fixture");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert!(!fixture.corpus.join("source").exists());
    }

    #[test]
    fn css_import_rejects_sidecar_path_collision() {
        let fixture = Fixture::new(&[(".surgeist-source.json", b"{}\n", false)]);
        let error = fixture.import().expect_err("reserved sidecar collision");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert!(!fixture.corpus.join("source").exists());
    }

    #[test]
    fn css_import_rejects_report_path_collision() {
        let fixture = Fixture::new(&[("generation-reports/all.json", b"{}\n", false)]);
        let error = fixture.import().expect_err("reserved report collision");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert!(!fixture.corpus.join("source").exists());
    }

    #[test]
    fn css_import_source_pin_mismatch_is_source_verification() {
        let fixture = Fixture::new(&[("declaration/Declaration.json", b"{\"case\":{}}\n", false)]);
        fs::write(
            fixture.corpus.join("corpus.toml"),
            manifest_text("0000000000000000000000000000000000000000", 1),
        )
        .expect("write mismatched pin");
        let error = fixture.import().expect_err("source pin mismatch");
        assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
        assert!(!fixture.corpus.join("source").exists());
    }

    #[test]
    fn css_import_uses_immutable_git_snapshot_bytes() {
        let fixture = Fixture::new(&[(
            "declaration/Declaration.json",
            b"{\"original\":true}\n",
            false,
        )]);
        let request = fixture.request();
        let source_file = fixture
            .source
            .join("fixtures/ast/declaration/Declaration.json");
        importer::run_with_pre_lease_hook(&request, || {
            fs::write(source_file, b"{\"replacement\":true}\n")
                .expect("replace materialized source bytes");
        })
        .expect("snapshot-backed import");
        assert_eq!(
            fs::read(fixture.imported("declaration/Declaration.json"))
                .expect("read imported snapshot"),
            b"{\"original\":true}\n"
        );
    }

    #[test]
    fn css_import_source_root_replacement_fails_before_intent() {
        let fixture = Fixture::new(&[(
            "declaration/Declaration.json",
            b"{\"original\":true}\n",
            false,
        )]);
        let request = fixture.request();
        let fixture_root = fixture.source.join("fixtures/ast");
        let displaced = fixture.source.join("fixtures/ast-displaced");
        let error = importer::run_with_pre_lease_hook(&request, || {
            fs::rename(&fixture_root, &displaced).expect("displace verified source root");
            fs::create_dir(&fixture_root).expect("replace source root");
            fs::write(fixture_root.join("replacement.json"), b"{}\n")
                .expect("write replacement source");
        })
        .expect_err("source replacement");
        assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
        assert!(!fixture.corpus.join("source").exists());
    }

    #[test]
    fn css_import_clean_full_removes_only_classified_stale_fixtures() {
        let mut fixture = Fixture::new(&[
            ("declaration/Declaration.json", b"{\"old\":1}\n", false),
            ("selector/Removed.json", b"{\"old\":2}\n", false),
        ]);
        fixture.import().expect("initial import");
        fixture.replace_commit(&[("declaration/Declaration.json", b"{\"new\":1}\n", false)]);
        fixture.import().expect("replacement import");

        assert_eq!(
            fs::read(fixture.imported("declaration/Declaration.json")).expect("updated fixture"),
            b"{\"new\":1}\n"
        );
        assert!(!fixture.imported("selector/Removed.json").exists());
        assert_eq!(
            fixture_paths(&fixture.sidecar()),
            BTreeSet::from(["declaration/Declaration.json".to_owned()])
        );
    }

    #[test]
    fn css_import_unknown_old_entry_is_invalid_and_unchanged() {
        let fixture = Fixture::new(&[("declaration/Declaration.json", b"{\"case\":{}}\n", false)]);
        fixture.import().expect("initial import");
        fs::write(fixture.imported("unknown.json"), b"{}\n").expect("write unknown entry");
        let before = snapshot_tree(&fixture.corpus.join("source"));
        let error = fixture.import().expect_err("unknown old import entry");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert_eq!(snapshot_tree(&fixture.corpus.join("source")), before);
    }

    #[test]
    fn css_import_inter_scan_unknown_is_invalid_unchanged_and_has_no_intent() {
        let mut fixture =
            Fixture::new(&[("declaration/Declaration.json", b"{\"old\":true}\n", false)]);
        fixture.import().expect("initial import");
        fixture.replace_commit(&[("declaration/Declaration.json", b"{\"new\":true}\n", false)]);
        let request = fixture.request();
        let unknown_path = fixture.imported("late-unknown.json");
        let unknown_bytes = b"{\"unknown\":true}\n";
        let mut expected = snapshot_tree(&fixture.corpus.join("source"));
        expected.insert(PathBuf::from("late-unknown.json"), unknown_bytes.to_vec());

        let error = importer::run_with_inter_scan_hook(&request, move || {
            fs::write(unknown_path, unknown_bytes).expect("insert inter-scan unknown entry");
        })
        .expect_err("inter-scan unknown entry");

        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert_eq!(snapshot_tree(&fixture.corpus.join("source")), expected);
        let transactions = fixture.corpus.join(".surgeist-generator/transactions/css");
        assert_eq!(
            fs::read_dir(transactions)
                .expect("inspect CSS transactions")
                .count(),
            0,
            "inter-scan rejection created transaction intent or residue"
        );
        assert!(
            fs::read_dir(&fixture.corpus)
                .expect("inspect corpus root")
                .all(|entry| {
                    !entry
                        .expect("corpus entry")
                        .file_name()
                        .to_string_lossy()
                        .starts_with("._surgeist-")
                }),
            "inter-scan rejection created an external stage"
        );
    }

    #[test]
    fn css_import_malformed_old_sidecar_is_invalid_and_unchanged() {
        let fixture = Fixture::new(&[("declaration/Declaration.json", b"{\"case\":{}}\n", false)]);
        fixture.import().expect("initial import");
        fs::write(fixture.imported(".surgeist-source.json"), b"{}\n").expect("corrupt old sidecar");
        let before = snapshot_tree(&fixture.corpus.join("source"));
        let error = fixture.import().expect_err("malformed old sidecar");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert_eq!(snapshot_tree(&fixture.corpus.join("source")), before);
    }

    #[test]
    fn css_import_replaces_known_stale_fixture_bytes() {
        let fixture =
            Fixture::new(&[("declaration/Declaration.json", b"{\"clean\":true}\n", false)]);
        fixture.import().expect("initial import");
        fs::write(
            fixture.imported("declaration/Declaration.json"),
            b"{\"stale\":true}\n",
        )
        .expect("stale imported fixture");

        fixture.import().expect("replace known stale fixture");
        assert_eq!(
            fs::read(fixture.imported("declaration/Declaration.json"))
                .expect("restored imported fixture"),
            b"{\"clean\":true}\n"
        );
    }

    #[test]
    fn css_import_malformed_downstream_authority_is_invalid_and_unchanged() {
        let fixture = Fixture::new(&[("declaration/Declaration.json", b"{\"case\":{}}\n", false)]);
        fixture.import().expect("initial import");
        let report = fixture
            .corpus
            .join("expectations/generation-reports/all.json");
        fs::create_dir_all(report.parent().expect("report parent")).expect("create report parent");
        fs::write(report, b"{}\n").expect("write malformed authority");
        let import_before = snapshot_tree(&fixture.corpus.join("source"));
        let error = fixture
            .import()
            .expect_err("malformed downstream authority");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert_eq!(snapshot_tree(&fixture.corpus.join("source")), import_before);
    }

    #[test]
    fn css_import_unknown_downstream_entry_is_invalid_and_unchanged() {
        let fixture = Fixture::new(&[("declaration/Declaration.json", b"{\"case\":{}}\n", false)]);
        fixture.import().expect("initial import");
        fixture.seed_downstream("declaration/Declaration.json");
        fs::write(fixture.corpus.join("expectations/rogue.json"), b"{}\n")
            .expect("write unknown downstream entry");
        let import_before = snapshot_tree(&fixture.corpus.join("source"));
        let downstream_before = snapshot_tree(&fixture.corpus.join("expectations"));
        let error = fixture.import().expect_err("unknown downstream entry");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert_eq!(snapshot_tree(&fixture.corpus.join("source")), import_before);
        assert_eq!(
            snapshot_tree(&fixture.corpus.join("expectations")),
            downstream_before
        );
    }

    #[test]
    fn css_import_unchanged_sidecar_preserves_downstream_freshness() {
        let fixture = Fixture::new(&[("declaration/Declaration.json", b"{\"case\":{}}\n", false)]);
        fixture.import().expect("initial import");
        let downstream = fixture.seed_downstream("declaration/Declaration.json");
        fixture.import().expect("unchanged import");

        assert_eq!(
            Sha256Digest::from_bytes(fixture.sidecar()),
            downstream.sidecar_digest
        );
        assert_eq!(
            snapshot_tree(&fixture.corpus.join("expectations")),
            downstream.bytes
        );
        assert_eq!(
            fs::read(
                fixture
                    .corpus
                    .join("expectations/generation-reports/all.json")
            )
            .expect("preserved report"),
            downstream.report_bytes
        );
    }

    #[test]
    fn css_import_preserves_classifiable_stale_downstream_bytes() {
        let fixture = Fixture::new(&[("declaration/Declaration.json", b"{\"case\":{}}\n", false)]);
        fixture.import().expect("initial import");
        fixture.seed_downstream("declaration/Declaration.json");
        fs::write(
            fixture
                .corpus
                .join("expectations/declaration/Declaration.json"),
            b"{\"stale\":\"but historically owned\"}\n",
        )
        .expect("make downstream stale");
        let downstream_before = snapshot_tree(&fixture.corpus.join("expectations"));

        fixture.import().expect("import with stale downstream");
        assert_eq!(
            snapshot_tree(&fixture.corpus.join("expectations")),
            downstream_before
        );
    }

    #[test]
    fn css_import_changed_sidecar_preserves_downstream_and_makes_it_stale() {
        let mut fixture =
            Fixture::new(&[("declaration/Declaration.json", b"{\"old\":true}\n", false)]);
        fixture.import().expect("initial import");
        let downstream = fixture.seed_downstream("declaration/Declaration.json");
        fixture.replace_commit(&[("declaration/Declaration.json", b"{\"new\":true}\n", false)]);
        fixture.import().expect("changed import");

        let new_digest = Sha256Digest::from_bytes(fixture.sidecar());
        assert_ne!(new_digest, downstream.sidecar_digest);
        assert_eq!(
            snapshot_tree(&fixture.corpus.join("expectations")),
            downstream.bytes
        );
        let report = fs::read(
            fixture
                .corpus
                .join("expectations/generation-reports/all.json"),
        )
        .expect("preserved stale report");
        assert_eq!(report, downstream.report_bytes);
        assert!(
            String::from_utf8(report)
                .expect("UTF-8 report")
                .contains(downstream.sidecar_digest.as_str())
        );
    }
}
