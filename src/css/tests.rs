use std::path::Path;

use crate::core::{ObjectFormat, SnapshotEntry, VerifiedSourceSnapshot};
use crate::{GeneratorErrorKind, PinnedSource, RelativePath, Sha256Digest, SourceRevision};

use super::{case, manifest, sidecar};

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
    valid.push(manifest_text(SHA1_REVISION, 1).replace(
        "declaration/Declaration.json#/case",
        "nested#context/Fixture#.json#/before#~1middle~1#after",
    ));

    for text in valid {
        manifest::parse(text.as_bytes(), Path::new("corpus.toml"))
            .expect("valid schema-1 CSS manifest");
    }
}

#[test]
fn css_manifest_override_binding_uses_the_exact_json_fixture_source() {
    let id = "nested#context/Fixture#.json#/before#~1middle~1#after";
    let text = manifest_text(SHA1_REVISION, 1).replace("declaration/Declaration.json#/case", id);
    let parsed =
        manifest::parse(text.as_bytes(), Path::new("corpus.toml")).expect("valid override syntax");
    let record = parsed.cases[0]
        .bind(&RelativePath::new("nested#context/Fixture#.json").expect("matching source"))
        .expect("exact source-bound override");
    assert_eq!(record.case_id().as_str(), id);

    let error = parsed.cases[0]
        .bind(&RelativePath::new("other/Fixture.json").expect("mismatched source"))
        .expect_err("override must remain owned by its exact fixture source");
    assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
}

#[test]
fn css_case_private_parser_accepts_historical_source_and_pointer_token_hashes() {
    let source = RelativePath::new("nested#context/Fixture#.json").expect("strict CSS source");
    let id = "nested#context/Fixture#.json#/before#~1middle~1#after";
    let parsed = case::CssCaseId::new(id, &source).expect("private source-bound CSS case ID");
    assert_eq!(parsed.as_str(), id);
    let record = case::CssCaseDispositionRecord::new(
        id,
        &source,
        crate::CaseDisposition::ExpectedFail,
        Some("historical hash-label mismatch"),
    )
    .expect("private source-bound CSS record");
    assert_eq!(record.disposition(), crate::CaseDisposition::ExpectedFail);
    assert_eq!(record.reason(), Some("historical hash-label mismatch"));
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
        valid.replace(
            "declaration/Declaration.json#/case",
            "declaration/Declaration.json#",
        ),
        valid.replace(
            "declaration/Declaration.json#/case",
            "declaration/Declaration.json#not-a-pointer",
        ),
        valid.replace(
            "declaration/Declaration.json#/case",
            "declaration/Declaration.css#/case",
        ),
        valid.replace(
            "declaration/Declaration.json#/case",
            "declaration/Declaration.json##/extra-delimiter",
        ),
        valid.replace(
            "declaration/Declaration.json#/case",
            "declaration/Declaration.json#/bad~2escape",
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

#[test]
fn css_import_reserved_fixture_path_relation_matrix() {
    for path in [
        ".surgeist-source.json",
        ".surgeist-source.json/child.json",
        "generation-reports/all.json",
        "generation-reports/all.json/child.json",
    ] {
        let path = RelativePath::new(path).expect("canonical fixture path");
        let error = sidecar::validate_fixture_path(&path)
            .expect_err("reserved equality, ancestor, or descendant");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert_eq!(
            error.to_string(),
            format!(
                "validate CSS import sidecar: reserved CSSTree fixture path: {}",
                path.as_str()
            )
        );
    }

    for path in ["generation-reports", "generation-reports/all"] {
        let path = RelativePath::new(path).expect("canonical fixture path");
        let error = sidecar::validate_fixture_path(&path).expect_err("reserved ancestor");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert_eq!(
            error.to_string(),
            format!(
                "validate CSS import sidecar: validate relative path extension: {} does not have extension json",
                path.as_str()
            )
        );
    }

    for path in [
        ".surgeist-source.json-copy/child.json",
        "nested/.surgeist-source.json",
        "generation-reports/all-copy.json",
        "generation-reports/all.json-copy/child.json",
    ] {
        let path = RelativePath::new(path).expect("canonical fixture path");
        sidecar::validate_fixture_path(&path).expect("noncolliding fixture path");
    }
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod imports {
    use std::cell::Cell;
    use std::collections::{BTreeMap, BTreeSet};
    use std::ffi::OsStr;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::css::{CssCommand, CssRequest};
    use crate::{
        ArtifactProvenance, CorpusLocation, GenerationCounts, GenerationReport, GeneratorErrorKind,
        ManifestVersion, PinnedSource, RelativePath, ReportArtifact, Sha256Digest, SourceRevision,
    };

    use super::{CSSTREE_REPOSITORY, manifest_text};
    use crate::css::{full_generation, importer};

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

        fn generate_request(&self) -> CssRequest {
            CssRequest::new(self.location.clone(), CssCommand::Generate, None, None)
                .expect("full generation request")
        }

        fn pin(&self) -> PinnedSource {
            PinnedSource::new(
                "csstree",
                CSSTREE_REPOSITORY,
                SourceRevision::new(&self.revision).expect("source revision"),
                RelativePath::new("fixtures/ast").expect("fixture root"),
            )
            .expect("CSSTree pin")
        }

        fn import(&self) -> crate::Result<()> {
            crate::css::run(self.request())
        }

        fn generate(&self) -> crate::Result<()> {
            crate::css::run(self.generate_request())
        }

        fn set_manifest(
            &self,
            expected_files: usize,
            expected_cases: usize,
            overrides: &[(&str, &str, Option<&str>)],
        ) {
            let mut text = format!(
                "schema_version = 1\n\n[source]\nkind = \"csstree\"\nrepository = \"{CSSTREE_REPOSITORY}\"\nrevision = \"{}\"\nfixture_root = \"fixtures/ast\"\nimport_root = \"source\"\nexpected_files = {expected_files}\nexpected_cases = {expected_cases}\n\n[artifacts]\nexpectation_root = \"expectations\"\nreport_file = \"expectations/generation-reports/all.json\"\n",
                self.revision
            );
            for (id, status, reason) in overrides {
                text.push_str(&format!(
                    "\n[[cases]]\nid = \"{id}\"\nstatus = \"{status}\"\n"
                ));
                if let Some(reason) = reason {
                    text.push_str(&format!("reason = \"{reason}\"\n"));
                }
            }
            fs::write(self.corpus.join("corpus.toml"), text).expect("write generation manifest");
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

        fn replace_commit_with_gitlink(&mut self, relative: &str) {
            fs::remove_dir_all(self.source.join("fixtures/ast")).expect("remove old fixtures");
            let gitlink = self.source.join("fixtures/ast").join(relative);
            fs::create_dir_all(&gitlink).expect("create nested Git repository");
            run_git(&gitlink, &[OsStr::new("init"), OsStr::new("--quiet")]);
            run_git(
                &gitlink,
                &[
                    OsStr::new("config"),
                    OsStr::new("user.name"),
                    OsStr::new("CSS Gitlink Test"),
                ],
            );
            run_git(
                &gitlink,
                &[
                    OsStr::new("config"),
                    OsStr::new("user.email"),
                    OsStr::new("css-gitlink@example.invalid"),
                ],
            );
            fs::write(gitlink.join("fixture.json"), b"{}\n").expect("write nested Git fixture");
            run_git(&gitlink, &[OsStr::new("add"), OsStr::new("fixture.json")]);
            run_git(
                &gitlink,
                &[
                    OsStr::new("commit"),
                    OsStr::new("--quiet"),
                    OsStr::new("-m"),
                    OsStr::new("nested fixture"),
                ],
            );
            run_git(&self.source, &[OsStr::new("add"), OsStr::new("--all")]);
            run_git(
                &self.source,
                &[
                    OsStr::new("commit"),
                    OsStr::new("--quiet"),
                    OsStr::new("-m"),
                    OsStr::new("replace fixture with gitlink"),
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

        fn expectation(&self, relative: &str) -> Vec<u8> {
            fs::read(self.corpus.join("expectations").join(relative)).expect("read CSS expectation")
        }

        fn report(&self) -> Vec<u8> {
            fs::read(self.corpus.join("expectations/generation-reports/all.json"))
                .expect("read CSS generation report")
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

        fn replace_historical_report(&self, paths: &[(&str, &str)]) {
            let sidecar_digest = Sha256Digest::from_bytes(self.sidecar());
            let artifacts = paths
                .iter()
                .enumerate()
                .map(|(index, (source, output))| {
                    let mut domain = BTreeMap::new();
                    domain.insert("csstree-import".to_owned(), sidecar_digest.clone());
                    let provenance = ArtifactProvenance::new(
                        RelativePath::new(format!("source/{source}"))
                            .expect("historical source path"),
                        Sha256Digest::from_bytes(format!("source-{index}").as_bytes()),
                        "surgeist-css-generate",
                        ManifestVersion::new(1).expect("historical schema"),
                        domain,
                    )
                    .expect("historical provenance");
                    ReportArtifact::new(
                        provenance,
                        RelativePath::new(format!("expectations/{output}"))
                            .expect("historical output path"),
                        Sha256Digest::from_bytes(format!("output-{index}").as_bytes()),
                        1,
                    )
                    .expect("historical artifact")
                })
                .collect::<Vec<_>>();
            let report = GenerationReport::new(
                Sha256Digest::from_file(self.corpus.join("corpus.toml"))
                    .expect("historical manifest digest"),
                CSSTREE_REPOSITORY,
                SourceRevision::new(&self.revision).expect("historical revision"),
                GenerationCounts::new(paths.len(), 0, 0, 0, 0).expect("historical counts"),
                artifacts,
            )
            .expect("structurally canonical historical report");
            let mut bytes =
                serde_json::to_vec_pretty(&report).expect("serialize historical report");
            bytes.push(b'\n');
            fs::write(
                self.corpus.join("expectations/generation-reports/all.json"),
                bytes,
            )
            .expect("replace historical report");
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

    fn path_identity(path: &Path) -> (u64, u64) {
        let metadata = fs::symlink_metadata(path).expect("read path identity");
        (metadata.dev(), metadata.ino())
    }

    fn assert_no_import_intent_journal_or_stage(fixture: &Fixture) {
        let transactions = fixture.corpus.join(".surgeist-generator/transactions/css");
        if transactions.exists() {
            assert_eq!(
                fs::read_dir(transactions)
                    .expect("inspect CSS transactions")
                    .count(),
                0,
                "rejection created transaction intent or residue"
            );
        }
        fn assert_no_active_journal(path: &Path) {
            for entry in fs::read_dir(path)
                .expect("inspect coordination state")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("read coordination entries")
            {
                let name = entry.file_name();
                let name = name.to_string_lossy();
                assert!(
                    !name.starts_with("active-")
                        && !name.starts_with("recovering-")
                        && name != "intent.json",
                    "rejection retained an active journal or intent: {}",
                    entry.path().display()
                );
                if entry.file_type().expect("coordination entry type").is_dir() {
                    assert_no_active_journal(&entry.path());
                }
            }
        }
        let coordination = fixture.corpus.join(".surgeist-generator");
        if coordination.exists() {
            assert_no_active_journal(&coordination);
        }
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
            "reserved-path rejection created an external stage"
        );
    }

    fn assert_fixture_path_rejected_before_transaction(path: &str) {
        let mut fixture =
            Fixture::new(&[("declaration/Declaration.json", b"{\"old\":true}\n", false)]);
        fixture.import().expect("initial import");
        let import_root = fixture.corpus.join("source");
        let before_bytes = snapshot_tree(&import_root);
        let before_identity = path_identity(&import_root);

        fixture.replace_commit(&[(path, b"{}\n", false)]);
        let pre_lease_reached = Cell::new(false);
        let request = fixture.request();
        let error = importer::run_with_pre_lease_hook(&request, || pre_lease_reached.set(true))
            .expect_err("reserved fixture descendant");

        assert_eq!(snapshot_tree(&import_root), before_bytes);
        assert_eq!(path_identity(&import_root), before_identity);
        assert_no_import_intent_journal_or_stage(&fixture);
        assert!(
            !pre_lease_reached.get(),
            "invalid fixture path reached lease preflight: {path}"
        );
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
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
    fn css_import_reserved_path_relations_and_case_aliases_preserve_existing_import() {
        let paths = [
            ".surgeist-source.json",
            ".surgeist-source.json/child.json",
            "generation-reports",
            "generation-reports/all",
            "generation-reports/all.json",
            "generation-reports/all.json/child.json",
        ];
        #[cfg(target_os = "macos")]
        let paths = paths
            .into_iter()
            .chain([
                ".Surgeist-Source.json",
                ".Surgeist-Source.json/child.json",
                "Generation-Reports",
                "Generation-Reports/All",
                "Generation-Reports/all.json",
                "generation-reports/All.json",
                "Generation-Reports/All.json",
                "Generation-Reports/All.json/child.json",
            ])
            .collect::<Vec<_>>();
        for path in paths {
            assert_fixture_path_rejected_before_transaction(path);
        }
    }

    #[test]
    fn css_import_gitlink_fixture_is_invalid_before_lease_or_intent() {
        let mut fixture =
            Fixture::new(&[("declaration/Declaration.json", b"{\"old\":true}\n", false)]);
        fixture.import().expect("initial import");
        let import_root = fixture.corpus.join("source");
        let before_bytes = snapshot_tree(&import_root);
        let before_identity = path_identity(&import_root);
        fixture.replace_commit_with_gitlink("declaration/Gitlink.json");
        assert_eq!(
            crate::verify_git_source(&fixture.source, &fixture.pin())
                .expect_err("generic source proof must reject a gitlink")
                .kind(),
            GeneratorErrorKind::SourceVerification
        );

        let pre_lease_reached = Cell::new(false);
        let request = fixture.request();
        let error = importer::run_with_pre_lease_hook(&request, || pre_lease_reached.set(true))
            .expect_err("gitlink fixture");

        assert!(!pre_lease_reached.get(), "gitlink reached lease preflight");
        assert_eq!(snapshot_tree(&import_root), before_bytes);
        assert_eq!(path_identity(&import_root), before_identity);
        assert_no_import_intent_journal_or_stage(&fixture);
        assert_eq!(
            error.kind(),
            GeneratorErrorKind::InvalidInventory,
            "unexpected gitlink classification: {error}"
        );
    }

    #[test]
    fn css_import_noncanonical_fixture_path_is_invalid_before_lease_or_intent() {
        let mut fixture =
            Fixture::new(&[("declaration/Declaration.json", b"{\"old\":true}\n", false)]);
        fixture.import().expect("initial import");
        let import_root = fixture.corpus.join("source");
        let before_bytes = snapshot_tree(&import_root);
        let before_identity = path_identity(&import_root);
        fixture.replace_commit(&[("declaration\\Rejected.json", b"{}\n", false)]);
        assert_eq!(
            crate::verify_git_source(&fixture.source, &fixture.pin())
                .expect_err("generic source proof must reject a noncanonical path")
                .kind(),
            GeneratorErrorKind::SourceVerification
        );

        let pre_lease_reached = Cell::new(false);
        let request = fixture.request();
        let error = importer::run_with_pre_lease_hook(&request, || pre_lease_reached.set(true))
            .expect_err("noncanonical fixture path");

        assert!(
            !pre_lease_reached.get(),
            "noncanonical path reached lease preflight"
        );
        assert_eq!(snapshot_tree(&import_root), before_bytes);
        assert_eq!(path_identity(&import_root), before_identity);
        assert_no_import_intent_journal_or_stage(&fixture);
        assert_eq!(
            error.kind(),
            GeneratorErrorKind::InvalidInventory,
            "unexpected noncanonical-path classification: {error}"
        );
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
    fn css_import_materialization_drift_remains_source_verification() {
        let fixture = Fixture::new(&[(
            "declaration/Declaration.json",
            b"{\"original\":true}\n",
            false,
        )]);
        fs::write(
            fixture
                .source
                .join("fixtures/ast/declaration/Declaration.json"),
            b"{\"materialized-drift\":true}\n",
        )
        .expect("drift materialized source bytes");
        let pre_lease_reached = Cell::new(false);

        let error = importer::run_with_pre_lease_hook(&fixture.request(), || {
            pre_lease_reached.set(true);
        })
        .expect_err("materialized source drift");

        assert_eq!(error.kind(), GeneratorErrorKind::SourceVerification);
        assert!(
            !pre_lease_reached.get(),
            "source drift reached lease preflight"
        );
        assert!(!fixture.corpus.join("source").exists());
        assert_no_import_intent_journal_or_stage(&fixture);
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
        assert_no_import_intent_journal_or_stage(&fixture);
    }

    #[test]
    fn css_import_inter_scan_known_entry_changes_are_invalid_without_intent() {
        for replace_identity in [false, true] {
            let mut fixture =
                Fixture::new(&[("declaration/Declaration.json", b"{\"old\":true}\n", false)]);
            fixture.import().expect("initial import");
            fixture.replace_commit(&[("declaration/Declaration.json", b"{\"new\":true}\n", false)]);
            let request = fixture.request();
            let import_root = fixture.corpus.join("source");
            let imported = fixture.imported("declaration/Declaration.json");
            let displaced = fixture.root.join("displaced-imported.json");
            let root_identity = path_identity(&import_root);
            let original_file_identity = path_identity(&imported);
            let replacement_identity = Cell::new(None);
            let replacement_bytes = if replace_identity {
                b"{\"old\":true}\n".as_slice()
            } else {
                b"{\"changed-between-scans\":true}\n".as_slice()
            };
            let mut expected_tree = snapshot_tree(&import_root);
            expected_tree.insert(
                PathBuf::from("declaration/Declaration.json"),
                replacement_bytes.to_vec(),
            );

            let error = importer::run_with_inter_scan_hook(&request, || {
                if replace_identity {
                    fs::rename(&imported, &displaced).expect("displace known imported fixture");
                }
                fs::write(&imported, replacement_bytes).expect("replace known imported fixture");
                replacement_identity.set(Some(path_identity(&imported)));
            })
            .expect_err("known import entry changed between scans");

            assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
            assert_eq!(
                error.to_string(),
                "revalidate current publication tree: publication inventory changed before transaction intent"
            );
            assert_eq!(path_identity(&import_root), root_identity);
            assert_eq!(snapshot_tree(&import_root), expected_tree);
            let retained_identity = path_identity(&imported);
            assert_eq!(Some(retained_identity), replacement_identity.get());
            if replace_identity {
                assert_ne!(
                    retained_identity, original_file_identity,
                    "same-byte replacement retained the original identity"
                );
            } else {
                assert_eq!(
                    retained_identity, original_file_identity,
                    "changed-byte rewrite unexpectedly replaced identity"
                );
            }
            assert_no_import_intent_journal_or_stage(&fixture);
        }
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

    fn imported_generation_fixture(
        relative: &str,
        bytes: &'static [u8],
        expected_cases: usize,
        overrides: &[(&str, &str, Option<&str>)],
    ) -> Fixture {
        let fixture = Fixture::new(&[(relative, bytes, false)]);
        fixture.set_manifest(1, expected_cases, overrides);
        fixture.import().expect("import generation fixture");
        fixture
    }

    #[test]
    fn css_expectation_case_order_golden() {
        let source = b"{\n  \"slash/~label\": {\n    \"generate\": \"a{color:red}\",\n    \"options\": {\"z\": {\"\\u03b2\": 2, \"a\": 1}, \"a\": [{\"z\": true, \"a\": false}, 3]},\n    \"ast\": {\"secret\": \"must not persist\"},\n    \"source\": \"a { color: red }\",\n    \"diagnostic\": \"must not persist\",\n    \"comments\": [\"must not persist\"],\n    \"recovery\": {\"must\": \"not persist\"}\n  },\n  \"plain\": {\"source\": \"b {}\", \"ast\": null},\n  \"error\": [{\"source\": \"broken\", \"message\": \"must not persist\", \"offset\": 4}]\n}\n";
        let fixture = imported_generation_fixture("declaration/Declaration.json", source, 3, &[]);
        fixture.generate().expect("generate neutral expectations");

        let source_digest = Sha256Digest::from_bytes(source);
        let sidecar_digest = Sha256Digest::from_bytes(fixture.sidecar());
        let expected = format!(
            "{{\n  \"schema_version\": 1,\n  \"generator\": \"surgeist-css-generate\",\n  \"source\": \"source/declaration/Declaration.json\",\n  \"source_sha256\": \"{source_digest}\",\n  \"source_revision\": \"{}\",\n  \"import_provenance_sha256\": \"{sidecar_digest}\",\n  \"cases\": [\n    {{\n      \"id\": \"declaration/Declaration.json#/error/0\",\n      \"context\": \"declaration\",\n      \"input\": \"broken\",\n      \"upstream_outcome\": \"rejected\",\n      \"status\": \"active\"\n    }},\n    {{\n      \"id\": \"declaration/Declaration.json#/plain\",\n      \"context\": \"declaration\",\n      \"label\": \"plain\",\n      \"input\": \"b {{}}\",\n      \"upstream_outcome\": \"parsed\",\n      \"status\": \"active\"\n    }},\n    {{\n      \"id\": \"declaration/Declaration.json#/slash~1~0label\",\n      \"context\": \"declaration\",\n      \"label\": \"slash/~label\",\n      \"input\": \"a {{ color: red }}\",\n      \"options\": {{\n        \"a\": [\n          {{\n            \"a\": false,\n            \"z\": true\n          }},\n          3\n        ],\n        \"z\": {{\n          \"a\": 1,\n          \"β\": 2\n        }}\n      }},\n      \"upstream_outcome\": \"parsed\",\n      \"canonical_css\": \"a{{color:red}}\",\n      \"status\": \"active\"\n    }}\n  ]\n}}\n",
            fixture.revision
        );
        let actual = fixture.expectation("declaration/Declaration.json");
        assert_eq!(actual, expected.as_bytes());
        assert!(!actual.windows(3).any(|window| window == b"ast"));
        assert!(!actual.windows(10).any(|window| window == b"diagnostic"));
        assert!(!actual.windows(7).any(|window| window == b"message"));
        assert!(!actual.windows(6).any(|window| window == b"offset"));
        assert!(!actual.windows(8).any(|window| window == b"comments"));
        assert!(!actual.windows(8).any(|window| window == b"recovery"));
    }

    #[test]
    fn css_expectation_hash_label_and_strict_hash_source_path_golden() {
        let source =
            b"{\"before#/middle/#after\":{\"source\":\"a {}\",\"ast\":{},\"generate\":\"a{}\"}}\n";
        let fixture = imported_generation_fixture("hash#context/Fixture#.json", source, 1, &[]);
        fixture.generate().expect("generate hash-label expectation");

        let source_digest = Sha256Digest::from_bytes(source);
        let sidecar_digest = Sha256Digest::from_bytes(fixture.sidecar());
        let expected = format!(
            "{{\n  \"schema_version\": 1,\n  \"generator\": \"surgeist-css-generate\",\n  \"source\": \"source/hash#context/Fixture#.json\",\n  \"source_sha256\": \"{source_digest}\",\n  \"source_revision\": \"{}\",\n  \"import_provenance_sha256\": \"{sidecar_digest}\",\n  \"cases\": [\n    {{\n      \"id\": \"hash#context/Fixture#.json#/before#~1middle~1#after\",\n      \"context\": \"hash#context\",\n      \"label\": \"before#/middle/#after\",\n      \"input\": \"a {{}}\",\n      \"upstream_outcome\": \"parsed\",\n      \"canonical_css\": \"a{{}}\",\n      \"status\": \"active\"\n    }}\n  ]\n}}\n",
            fixture.revision
        );
        assert_eq!(
            fixture.expectation("hash#context/Fixture#.json"),
            expected.as_bytes()
        );
    }

    #[test]
    fn css_expectation_hash_case_id_matches_override_disposition() {
        let id = "hash#context/Fixture#.json#/before#~1middle~1#after";
        let fixture = imported_generation_fixture(
            "hash#context/Fixture#.json",
            b"{\"before#/middle/#after\":{\"source\":\"a {}\",\"ast\":{}}}\n",
            1,
            &[(id, "unsupported", Some("hash-label override"))],
        );
        fixture
            .generate()
            .expect("generate overridden hash-label case");

        let expectation: serde_json::Value =
            serde_json::from_slice(&fixture.expectation("hash#context/Fixture#.json"))
                .expect("hash-label expectation JSON");
        assert_eq!(expectation["cases"][0]["id"], id);
        assert_eq!(expectation["cases"][0]["status"], "unsupported");
        assert_eq!(expectation["cases"][0]["reason"], "hash-label override");
    }

    #[test]
    fn css_expectation_duplicate_decoded_members_at_every_depth_are_rejected() {
        let fixtures: &[&[u8]] = &[
            b"{\"\\u0063ase\":{\"source\":\"a\",\"ast\":{}},\"case\":{\"source\":\"b\",\"ast\":{}}}\n",
            b"{\"case\":{\"source\":\"a\",\"source\":\"b\",\"ast\":{}}}\n",
            b"{\"case\":{\"source\":\"a\",\"ast\":{\"x\":1,\"\\u0078\":2}}}\n",
            b"{\"case\":{\"source\":\"a\",\"ast\":{},\"options\":{\"nested\":{\"x\":1,\"\\u0078\":2}}}}\n",
            b"{\"error\":[{\"source\":\"a\",\"source\":\"b\"}]}\n",
        ];
        for bytes in fixtures {
            let fixture =
                imported_generation_fixture("declaration/Declaration.json", bytes, 1, &[]);
            let error = fixture
                .generate()
                .expect_err("duplicate decoded JSON member");
            assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
            assert!(!fixture.corpus.join("expectations").exists());
            assert_no_import_intent_journal_or_stage(&fixture);
        }
    }

    #[test]
    fn css_expectation_empty_malformed_and_trailing_fixtures_publish_nothing() {
        let fixtures: &[&[u8]] = &[
            b"not JSON\n",
            b"{}\n",
            b"{\"error\":[]}\n",
            b"[]\n",
            b"{\"case\":{\"source\":\"a\",\"ast\":{}}} {}\n",
            b"{\"case\":{\"source\":1,\"ast\":{}}}\n",
            b"{\"case\":{\"source\":\"a\"}}\n",
            b"{\"case\":{\"source\":\"a\",\"ast\":{},\"options\":[]}}\n",
            b"{\"case\":{\"source\":\"a\",\"ast\":{},\"options\":null}}\n",
            b"{\"case\":{\"source\":\"a\",\"ast\":{},\"generate\":null}}\n",
            b"{\"error\":{\"source\":\"a\"}}\n",
            b"{\"error\":[{\"source\":null}]}\n",
        ];
        for bytes in fixtures {
            let fixture =
                imported_generation_fixture("declaration/Declaration.json", bytes, 1, &[]);
            let error = fixture.generate().expect_err("invalid fixture shape");
            assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
            assert!(!fixture.corpus.join("expectations").exists());
        }
    }

    #[test]
    fn css_expectation_options_recursively_sort_decoded_keys_and_preserve_arrays() {
        let fixture = imported_generation_fixture(
            "value/Options.json",
            b"{\"case\":{\"source\":\"value\",\"ast\":{},\"options\":{\"\\u03b2\":{\"z\":0,\"a\":1},\"a\":[3,2,1]}}}\n",
            1,
            &[],
        );
        fixture.generate().expect("generate canonical options");
        let expectation: serde_json::Value =
            serde_json::from_slice(&fixture.expectation("value/Options.json"))
                .expect("expectation JSON");
        let options = &expectation["cases"][0]["options"];
        assert_eq!(
            serde_json::to_string(options).expect("serialize options"),
            "{\"a\":[3,2,1],\"β\":{\"a\":1,\"z\":0}}"
        );
    }

    #[test]
    fn css_expectation_default_override_repeated_source_and_reason_accounting() {
        let fixture = imported_generation_fixture(
            "declaration/Disposition.json",
            b"{\"second\":{\"source\":\"same source\",\"ast\":{}},\"quarantine\":{\"source\":\"same source\",\"ast\":{}},\"first\":{\"source\":\"same source\",\"ast\":{}},\"error\":[{\"source\":\"same source\"}]}\n",
            4,
            &[
                (
                    "declaration/Disposition.json#/error/0",
                    "expected-fail",
                    Some("known rejection"),
                ),
                (
                    "declaration/Disposition.json#/first",
                    "unsupported",
                    Some("unsupported grammar"),
                ),
                (
                    "declaration/Disposition.json#/quarantine",
                    "quarantined",
                    Some("isolated fixture"),
                ),
            ],
        );
        fixture.generate().expect("generate dispositions");

        let expectation: serde_json::Value =
            serde_json::from_slice(&fixture.expectation("declaration/Disposition.json"))
                .expect("expectation JSON");
        assert_eq!(expectation["cases"][0]["status"], "expected-fail");
        assert_eq!(expectation["cases"][0]["reason"], "known rejection");
        assert_eq!(expectation["cases"][1]["status"], "unsupported");
        assert_eq!(expectation["cases"][1]["reason"], "unsupported grammar");
        assert_eq!(expectation["cases"][2]["status"], "quarantined");
        assert_eq!(expectation["cases"][2]["reason"], "isolated fixture");
        assert_eq!(expectation["cases"][3]["status"], "active");
        assert!(expectation["cases"][3].get("reason").is_none());
        assert_eq!(
            expectation["cases"]
                .as_array()
                .expect("cases")
                .iter()
                .map(|case| case["input"].as_str().expect("input"))
                .collect::<Vec<_>>(),
            ["same source", "same source", "same source", "same source"]
        );

        let report: GenerationReport =
            serde_json::from_slice(&fixture.report()).expect("generation report");
        assert_eq!(report.counts().active(), 1);
        assert_eq!(report.counts().expected_fail(), 1);
        assert_eq!(report.counts().unsupported(), 1);
        assert_eq!(report.counts().quarantined(), 1);
        assert_eq!(report.counts().failed_to_generate(), 0);
    }

    #[test]
    fn css_expectation_unmatched_override_and_full_count_mismatch_publish_nothing() {
        let unmatched = imported_generation_fixture(
            "declaration/Case.json",
            b"{\"case\":{\"source\":\"a\",\"ast\":{}}}\n",
            1,
            &[(
                "declaration/Case.json#/missing",
                "quarantined",
                Some("not derived"),
            )],
        );
        let error = unmatched.generate().expect_err("unmatched override");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert!(!unmatched.corpus.join("expectations").exists());

        let mismatch = imported_generation_fixture(
            "declaration/Case.json",
            b"{\"case\":{\"source\":\"a\",\"ast\":{}}}\n",
            2,
            &[],
        );
        let error = mismatch.generate().expect_err("full count mismatch");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert!(!mismatch.corpus.join("expectations").exists());
    }

    #[test]
    fn css_full_generate_report_binds_provenance_counts_digests_and_artifacts() {
        let source = b"{\"case\":{\"source\":\"a{}\",\"ast\":{},\"generate\":\"a{}\"}}\n";
        let fixture = imported_generation_fixture("declaration/Case.json", source, 1, &[]);
        fixture.generate().expect("full generation");

        let report_bytes = fixture.report();
        assert_eq!(report_bytes.last(), Some(&b'\n'));
        let manifest_digest =
            Sha256Digest::from_file(fixture.corpus.join("corpus.toml")).expect("manifest digest");
        let source_digest = Sha256Digest::from_bytes(source);
        let sidecar_digest = Sha256Digest::from_bytes(fixture.sidecar());
        let output_digest = Sha256Digest::from_bytes(fixture.expectation("declaration/Case.json"));
        let expected_report = format!(
            "{{\n  \"manifest_digest\": \"{manifest_digest}\",\n  \"source_repository\": \"{CSSTREE_REPOSITORY}\",\n  \"source_revision\": \"{}\",\n  \"counts\": {{\n    \"active\": 1,\n    \"expected_fail\": 0,\n    \"unsupported\": 0,\n    \"quarantined\": 0,\n    \"failed_to_generate\": 0\n  }},\n  \"artifacts\": [\n    {{\n      \"provenance\": {{\n        \"source_path\": \"source/declaration/Case.json\",\n        \"source_digest\": \"{source_digest}\",\n        \"generator\": \"surgeist-css-generate\",\n        \"schema_version\": 1,\n        \"domain_provenance\": {{\n          \"csstree-import\": \"{sidecar_digest}\"\n        }}\n      }},\n      \"output_path\": \"expectations/declaration/Case.json\",\n      \"output_digest\": \"{output_digest}\",\n      \"case_count\": 1\n    }}\n  ]\n}}\n",
            fixture.revision
        );
        assert_eq!(report_bytes, expected_report.as_bytes());
        let report: GenerationReport =
            serde_json::from_slice(&report_bytes).expect("generation report");
        assert_eq!(report.manifest_digest(), &manifest_digest);
        assert_eq!(report.source_repository(), CSSTREE_REPOSITORY);
        assert_eq!(report.source_revision().as_str(), fixture.revision);
        assert_eq!(report.counts().total().expect("count total"), 1);
        assert_eq!(report.artifacts().len(), 1);
        let artifact = &report.artifacts()[0];
        assert_eq!(artifact.case_count(), 1);
        assert_eq!(
            artifact.provenance().source_path().as_str(),
            "source/declaration/Case.json"
        );
        assert_eq!(artifact.provenance().source_digest(), &source_digest);
        assert_eq!(artifact.provenance().generator(), "surgeist-css-generate");
        assert_eq!(artifact.provenance().schema_version().get(), 1);
        assert_eq!(
            artifact
                .provenance()
                .domain_provenance()
                .get("csstree-import"),
            Some(&sidecar_digest)
        );
        assert_eq!(
            artifact.output_path().as_str(),
            "expectations/declaration/Case.json"
        );
        assert_eq!(artifact.output_digest(), &output_digest);
    }

    #[test]
    fn css_historical_inventory_removal_rename_addition_regenerates() {
        let mut fixture = Fixture::new(&[(
            "old/Old.json",
            b"{\"case\":{\"source\":\"old\",\"ast\":{}}}\n",
            false,
        )]);
        fixture.set_manifest(1, 1, &[]);
        fixture.import().expect("initial import");
        fixture.generate().expect("initial generation");
        assert!(fixture.corpus.join("expectations/old/Old.json").is_file());

        fixture.replace_commit(&[
            (
                "renamed/New.json",
                b"{\"case\":{\"source\":\"renamed\",\"ast\":{}}}\n",
                false,
            ),
            (
                "added/Added.json",
                b"{\"case\":{\"source\":\"added\",\"ast\":{}}}\n",
                false,
            ),
        ]);
        fixture.set_manifest(2, 2, &[]);
        fixture.import().expect("replacement import");
        fixture.generate().expect("replacement generation");

        assert!(!fixture.corpus.join("expectations/old/Old.json").exists());
        assert!(
            fixture
                .corpus
                .join("expectations/renamed/New.json")
                .is_file()
        );
        assert!(
            fixture
                .corpus
                .join("expectations/added/Added.json")
                .is_file()
        );
        let report: GenerationReport =
            serde_json::from_slice(&fixture.report()).expect("replacement report");
        assert_eq!(
            report
                .artifacts()
                .iter()
                .map(|artifact| artifact.output_path().as_str())
                .collect::<Vec<_>>(),
            [
                "expectations/added/Added.json",
                "expectations/renamed/New.json"
            ]
        );
    }

    #[test]
    fn css_historical_inventory_rejects_missing_or_malformed_authority() {
        let missing = imported_generation_fixture(
            "declaration/Case.json",
            b"{\"case\":{\"source\":\"a\",\"ast\":{}}}\n",
            1,
            &[],
        );
        missing.generate().expect("initial generation");
        fs::remove_file(
            missing
                .corpus
                .join("expectations/generation-reports/all.json"),
        )
        .expect("remove historical authority");
        let before = snapshot_tree(&missing.corpus.join("expectations"));
        let error = missing.generate().expect_err("missing authority");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert_eq!(snapshot_tree(&missing.corpus.join("expectations")), before);

        let malformed = imported_generation_fixture(
            "declaration/Case.json",
            b"{\"case\":{\"source\":\"a\",\"ast\":{}}}\n",
            1,
            &[],
        );
        malformed.generate().expect("initial generation");
        let report_path = malformed
            .corpus
            .join("expectations/generation-reports/all.json");
        let mut report: serde_json::Value =
            serde_json::from_slice(&fs::read(&report_path).expect("report")).expect("report JSON");
        report["counts"]["active"] = serde_json::json!(2);
        let mut bytes = serde_json::to_vec_pretty(&report).expect("serialize malformed report");
        bytes.push(b'\n');
        fs::write(&report_path, bytes).expect("write malformed authority");
        let before = snapshot_tree(&malformed.corpus.join("expectations"));
        let error = malformed.generate().expect_err("malformed authority");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert_eq!(
            snapshot_tree(&malformed.corpus.join("expectations")),
            before
        );
    }

    fn assert_historical_path_collision_is_pre_lease(
        source_paths: (&str, &str),
        output_paths: (&str, &str),
        expected_detail: &str,
    ) {
        let fixture = imported_generation_fixture(
            "seed/Case.json",
            b"{\"case\":{\"source\":\"seed\",\"ast\":{}}}\n",
            1,
            &[],
        );
        fixture.generate().expect("seed existing publication");
        fixture.replace_historical_report(&[
            (source_paths.0, output_paths.0),
            (source_paths.1, output_paths.1),
        ]);
        let expectation_root = fixture.corpus.join("expectations");
        let report_path = expectation_root.join("generation-reports/all.json");
        let before_bytes = snapshot_tree(&expectation_root);
        let before_root_identity = path_identity(&expectation_root);
        let before_report_identity = path_identity(&report_path);
        let pre_lease_reached = Cell::new(false);

        let error = full_generation::run_with_pre_lease_hook(&fixture.generate_request(), || {
            pre_lease_reached.set(true)
        })
        .expect_err("colliding historical paths must be rejected");

        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert!(
            error.to_string().contains(expected_detail),
            "unexpected collision diagnostic: {error}"
        );
        assert!(
            !pre_lease_reached.get(),
            "historical path collision reached lease acquisition"
        );
        assert_eq!(snapshot_tree(&expectation_root), before_bytes);
        assert_eq!(path_identity(&expectation_root), before_root_identity);
        assert_eq!(path_identity(&report_path), before_report_identity);
        assert_no_import_intent_journal_or_stage(&fixture);
    }

    #[test]
    fn css_historical_inventory_rejects_source_ancestor_collision_before_lease_or_intent() {
        assert_historical_path_collision_is_pre_lease(
            ("a.json", "a.json/b.json"),
            ("one.json", "two.json"),
            "historical source paths collide",
        );
    }

    #[test]
    fn css_historical_inventory_rejects_output_ancestor_collision_before_lease_or_intent() {
        assert_historical_path_collision_is_pre_lease(
            ("one.json", "two.json"),
            ("a.json", "a.json/b.json"),
            "historical output paths collide",
        );
    }

    #[test]
    fn css_historical_inventory_rejects_unicode_case_ambiguous_sources_before_lease_or_intent() {
        assert_historical_path_collision_is_pre_lease(
            ("\u{00c9}.json", "\u{00e9}.json"),
            ("one.json", "two.json"),
            "cannot prove distinct historical source path components",
        );
    }

    #[test]
    fn css_historical_inventory_rejects_unicode_normalization_ambiguous_outputs_before_lease_or_intent()
     {
        assert_historical_path_collision_is_pre_lease(
            ("one.json", "two.json"),
            ("\u{00e9}.json", "e\u{0301}.json"),
            "cannot prove distinct historical output path components",
        );
    }

    #[test]
    fn css_historical_inventory_rejects_aligned_ancestor_collision_without_publication() {
        assert_historical_path_collision_is_pre_lease(
            ("a.json", "a.json/b.json"),
            ("a.json", "a.json/b.json"),
            "historical source paths collide",
        );
    }

    #[test]
    fn css_full_generate_replaces_stale_owned_output_and_rejects_unknown_entry() {
        let fixture = imported_generation_fixture(
            "declaration/Case.json",
            b"{\"case\":{\"source\":\"a\",\"ast\":{}}}\n",
            1,
            &[],
        );
        fixture.generate().expect("initial generation");
        let output = fixture.corpus.join("expectations/declaration/Case.json");
        let expected = fs::read(&output).expect("current expectation");
        fs::write(&output, b"stale but historically owned\n").expect("stale output");
        fixture.generate().expect("replace stale output");
        assert_eq!(fs::read(&output).expect("replaced expectation"), expected);

        fs::remove_file(&output).expect("remove historically owned output");
        fixture.generate().expect("recreate absent owned output");
        assert_eq!(fs::read(&output).expect("recreated expectation"), expected);

        fs::write(fixture.corpus.join("expectations/unknown.json"), b"{}\n")
            .expect("unknown expectation entry");
        let before = snapshot_tree(&fixture.corpus.join("expectations"));
        let error = fixture.generate().expect_err("unknown output entry");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert_eq!(snapshot_tree(&fixture.corpus.join("expectations")), before);
    }

    struct PublishedGenerationProof {
        tree: BTreeMap<PathBuf, Vec<u8>>,
        root_identity: (u64, u64),
        output_bytes: Vec<u8>,
        output_identity: (u64, u64),
        report_bytes: Vec<u8>,
        report_identity: (u64, u64),
    }

    impl PublishedGenerationProof {
        fn capture(fixture: &Fixture) -> Self {
            let root = fixture.corpus.join("expectations");
            let output = root.join("declaration/Case.json");
            let report = root.join("generation-reports/all.json");
            Self {
                tree: snapshot_tree(&root),
                root_identity: path_identity(&root),
                output_bytes: fs::read(&output).expect("read published expectation"),
                output_identity: path_identity(&output),
                report_bytes: fs::read(&report).expect("read published report"),
                report_identity: path_identity(&report),
            }
        }

        fn assert_unchanged(&self, fixture: &Fixture) {
            let root = fixture.corpus.join("expectations");
            let output = root.join("declaration/Case.json");
            let report = root.join("generation-reports/all.json");
            assert_eq!(snapshot_tree(&root), self.tree);
            assert_eq!(path_identity(&root), self.root_identity);
            assert_eq!(
                fs::read(&output).expect("read retained expectation"),
                self.output_bytes
            );
            assert_eq!(path_identity(&output), self.output_identity);
            assert_eq!(
                fs::read(&report).expect("read retained report"),
                self.report_bytes
            );
            assert_eq!(path_identity(&report), self.report_identity);
        }
    }

    fn canonical_sidecar_replacement(bytes: &[u8], old: &str, new: &str, field: &str) -> Vec<u8> {
        assert_eq!(
            old.len(),
            new.len(),
            "replacement must preserve field width"
        );
        let text = String::from_utf8(bytes.to_vec()).expect("canonical sidecar is UTF-8");
        assert_eq!(
            text.matches(old).count(),
            1,
            "sidecar must contain one {field} value"
        );
        text.replacen(old, new, 1).into_bytes()
    }

    fn remove_completed_coordination(fixture: &Fixture) {
        let coordination = fixture.corpus.join(".surgeist-generator");
        fs::remove_dir_all(&coordination).expect("remove completed test coordination");
        assert!(!coordination.exists());
    }

    fn assert_initial_import_rejected_before_lease(
        mutate: impl FnOnce(&Fixture),
        expected_kind: GeneratorErrorKind,
        expected_error: &str,
    ) {
        let fixture = imported_generation_fixture(
            "declaration/Case.json",
            b"{\"case\":{\"source\":\"a\",\"ast\":{}}}\n",
            1,
            &[],
        );
        fixture.generate().expect("seed current expectations");
        mutate(&fixture);
        remove_completed_coordination(&fixture);
        let published = PublishedGenerationProof::capture(&fixture);
        let hook_reached = Cell::new(false);

        let error = full_generation::run_with_pre_lease_hook(&fixture.generate_request(), || {
            hook_reached.set(true)
        })
        .expect_err("invalid initial import must reject full generation");

        published.assert_unchanged(&fixture);
        assert_no_import_intent_journal_or_stage(&fixture);
        let coordination = fixture.corpus.join(".surgeist-generator");
        let owner = coordination.join("leases/css/owner.json");
        assert_eq!(
            (
                error.kind(),
                hook_reached.get(),
                coordination.exists(),
                owner.exists(),
            ),
            (expected_kind, false, false, false),
            "initial import validation must precede the hook and every coordination mutation"
        );
        assert_eq!(error.to_string(), expected_error);
    }

    #[test]
    fn css_full_generate_rejects_initially_absent_import_sidecar_before_lease() {
        assert_initial_import_rejected_before_lease(
            |fixture| {
                fs::remove_file(fixture.imported(".surgeist-source.json"))
                    .expect("remove canonical import sidecar");
            },
            GeneratorErrorKind::Verification,
            "validate current CSS import: nonempty CSS import root has no canonical sidecar",
        );
    }

    #[test]
    fn css_full_generate_rejects_initially_absent_import_fixture_before_lease() {
        assert_initial_import_rejected_before_lease(
            |fixture| {
                fs::remove_file(fixture.imported("declaration/Case.json"))
                    .expect("remove listed import fixture");
            },
            GeneratorErrorKind::Verification,
            "validate current CSS import: CSS imported fixture is absent: declaration/Case.json",
        );
    }

    #[test]
    fn css_full_generate_rejects_initially_stale_import_sidecar_before_lease() {
        assert_initial_import_rejected_before_lease(
            |fixture| {
                let sidecar_path = fixture.imported(".surgeist-source.json");
                let sidecar = fs::read(&sidecar_path).expect("read canonical import sidecar");
                let stale_revision = if fixture.revision.starts_with('0') {
                    "1".repeat(fixture.revision.len())
                } else {
                    "0".repeat(fixture.revision.len())
                };
                let stale = canonical_sidecar_replacement(
                    &sidecar,
                    &fixture.revision,
                    &stale_revision,
                    "source revision",
                );
                fs::write(sidecar_path, stale).expect("write stale canonical sidecar");
            },
            GeneratorErrorKind::Verification,
            "validate current CSS import: CSS import sidecar does not match the current manifest",
        );
    }

    #[test]
    fn css_full_generate_rejects_initially_stale_import_fixture_before_lease() {
        assert_initial_import_rejected_before_lease(
            |fixture| {
                fs::write(
                    fixture.imported("declaration/Case.json"),
                    b"{\"case\":{\"source\":\"stale\",\"ast\":{}}}\n",
                )
                .expect("write stale import fixture");
            },
            GeneratorErrorKind::Verification,
            "validate current CSS import: CSS imported fixture digest is stale: declaration/Case.json",
        );
    }

    #[test]
    fn css_full_generate_rejects_initially_unknown_import_entry_before_lease() {
        assert_initial_import_rejected_before_lease(
            |fixture| {
                fs::write(fixture.imported("unknown.json"), b"{}\n")
                    .expect("write unknown import entry");
            },
            GeneratorErrorKind::InvalidInventory,
            "validate current CSS import: unknown entry in CSS import root: unknown.json",
        );
    }

    #[derive(Clone, Copy, Debug)]
    enum PreLeaseImportMutation {
        FixtureByteDrift,
        FixtureSameBytesIdentityReplacement,
        SidecarByteDrift,
        SidecarSameBytesIdentityReplacement,
    }

    fn assert_pre_lease_import_revalidation(mutation: PreLeaseImportMutation) {
        const SOURCE: &[u8] = b"{\"case\":{\"source\":\"a\",\"ast\":{}}}\n";

        let fixture = imported_generation_fixture("declaration/Case.json", SOURCE, 1, &[]);
        fixture.generate().expect("seed current expectations");
        remove_completed_coordination(&fixture);
        let published = PublishedGenerationProof::capture(&fixture);
        let target = match mutation {
            PreLeaseImportMutation::FixtureByteDrift
            | PreLeaseImportMutation::FixtureSameBytesIdentityReplacement => {
                fixture.imported("declaration/Case.json")
            }
            PreLeaseImportMutation::SidecarByteDrift
            | PreLeaseImportMutation::SidecarSameBytesIdentityReplacement => {
                fixture.imported(".surgeist-source.json")
            }
        };
        let displaced = fixture.root.join("displaced-pre-lease-import");
        let original_bytes = fs::read(&target).expect("read retained import target");
        let original_identity = path_identity(&target);
        let replace_identity = matches!(
            mutation,
            PreLeaseImportMutation::FixtureSameBytesIdentityReplacement
                | PreLeaseImportMutation::SidecarSameBytesIdentityReplacement
        );
        let replacement_bytes = match mutation {
            PreLeaseImportMutation::FixtureByteDrift => {
                b"{\"case\":{\"source\":\"pre-lease drift\",\"ast\":{}}}\n".to_vec()
            }
            PreLeaseImportMutation::SidecarByteDrift => canonical_sidecar_replacement(
                &original_bytes,
                Sha256Digest::from_bytes(SOURCE).as_str(),
                Sha256Digest::from_bytes(b"pre-lease sidecar byte drift").as_str(),
                "fixture digest",
            ),
            PreLeaseImportMutation::FixtureSameBytesIdentityReplacement
            | PreLeaseImportMutation::SidecarSameBytesIdentityReplacement => original_bytes.clone(),
        };
        let hook_reached = Cell::new(false);
        let replacement_identity = Cell::new(None);

        let result = full_generation::run_with_pre_lease_hook(&fixture.generate_request(), || {
            assert!(
                !hook_reached.replace(true),
                "pre-lease hook ran more than once"
            );
            assert!(
                !fixture.corpus.join(".surgeist-generator").exists(),
                "lease acquisition began before the pre-lease hook"
            );
            assert_eq!(
                fs::read(&target).expect("read hook import target"),
                original_bytes
            );
            assert_eq!(path_identity(&target), original_identity);
            published.assert_unchanged(&fixture);
            if replace_identity {
                fs::rename(&target, &displaced).expect("displace preflight import target");
            }
            fs::write(&target, &replacement_bytes).expect("write raced import target");
            replacement_identity.set(Some(path_identity(&target)));
        });
        let error = result.expect_err("pre-lease import replacement must reject generation");

        published.assert_unchanged(&fixture);
        assert_no_import_intent_journal_or_stage(&fixture);
        let owner = fixture
            .corpus
            .join(".surgeist-generator/leases/css/owner.json");
        let expected_kind = if replace_identity {
            GeneratorErrorKind::InvalidInventory
        } else {
            GeneratorErrorKind::Verification
        };
        assert_eq!(
            (error.kind(), hook_reached.get(), owner.exists()),
            (expected_kind, true, false),
            "pre-lease import proof must close under the mutex before owner installation"
        );
        if replace_identity {
            assert_eq!(
                error.to_string(),
                "revalidate CSS generation inputs: current CSS import changed before lease owner installation"
            );
            assert_ne!(path_identity(&target), original_identity);
            assert_eq!(path_identity(&displaced), original_identity);
            assert_eq!(
                fs::read(&displaced).expect("read displaced import"),
                original_bytes
            );
        } else {
            assert_eq!(
                error.to_string(),
                "validate current CSS import: CSS imported fixture digest is stale: declaration/Case.json"
            );
            assert_eq!(path_identity(&target), original_identity);
            assert!(!displaced.exists());
        }
        assert_eq!(Some(path_identity(&target)), replacement_identity.get());
        assert_eq!(
            fs::read(&target).expect("read raced import target"),
            replacement_bytes
        );
    }

    #[test]
    fn css_full_generate_pre_lease_revalidation_rejects_fixture_byte_drift_before_owner_install() {
        assert_pre_lease_import_revalidation(PreLeaseImportMutation::FixtureByteDrift);
    }

    #[test]
    fn css_full_generate_pre_lease_revalidation_rejects_fixture_same_byte_identity_replacement_before_owner_install()
     {
        assert_pre_lease_import_revalidation(
            PreLeaseImportMutation::FixtureSameBytesIdentityReplacement,
        );
    }

    #[test]
    fn css_full_generate_pre_lease_revalidation_rejects_sidecar_byte_drift_before_owner_install() {
        assert_pre_lease_import_revalidation(PreLeaseImportMutation::SidecarByteDrift);
    }

    #[test]
    fn css_full_generate_pre_lease_revalidation_rejects_sidecar_same_byte_identity_replacement_before_owner_install()
     {
        assert_pre_lease_import_revalidation(
            PreLeaseImportMutation::SidecarSameBytesIdentityReplacement,
        );
    }

    #[test]
    fn css_full_generate_rejects_current_import_digest_drift_without_publication() {
        let fixture = imported_generation_fixture(
            "declaration/Case.json",
            b"{\"case\":{\"source\":\"a\",\"ast\":{}}}\n",
            1,
            &[],
        );
        fs::write(
            fixture.imported("declaration/Case.json"),
            b"{\"case\":{\"source\":\"drift\",\"ast\":{}}}\n",
        )
        .expect("drift imported bytes");
        let error = fixture.generate().expect_err("current import digest drift");
        assert_eq!(error.kind(), GeneratorErrorKind::Verification);
        assert!(!fixture.corpus.join("expectations").exists());
    }

    #[derive(Clone, Copy)]
    enum HeldSidecarMutation {
        ByteDrift,
        SameBytesIdentityReplacement,
    }

    fn assert_full_generation_held_sidecar_revalidation(mutation: HeldSidecarMutation) {
        const SOURCE: &[u8] = b"{\"case\":{\"source\":\"a\",\"ast\":{}}}\n";

        let fixture = imported_generation_fixture("declaration/Case.json", SOURCE, 1, &[]);
        fixture.generate().expect("seed current expectations");
        let expectation_root = fixture.corpus.join("expectations");
        let expectation = expectation_root.join("declaration/Case.json");
        let report = expectation_root.join("generation-reports/all.json");
        let sidecar = fixture.imported(".surgeist-source.json");
        let displaced = fixture.root.join("displaced-import-sidecar.json");
        let before_tree = snapshot_tree(&expectation_root);
        let before_expectation_bytes = fixture.expectation("declaration/Case.json");
        let before_report_bytes = fixture.report();
        let before_root_identity = path_identity(&expectation_root);
        let before_expectation_identity = path_identity(&expectation);
        let before_report_identity = path_identity(&report);
        let original_sidecar_bytes = fixture.sidecar();
        let original_sidecar_identity = path_identity(&sidecar);
        let raced_sidecar_bytes = match mutation {
            HeldSidecarMutation::ByteDrift => {
                let original_digest = Sha256Digest::from_bytes(SOURCE);
                let drifted_digest = Sha256Digest::from_bytes(b"held sidecar byte drift");
                let sidecar_text = String::from_utf8(original_sidecar_bytes.clone())
                    .expect("canonical sidecar is UTF-8");
                assert_eq!(
                    sidecar_text.matches(original_digest.as_str()).count(),
                    1,
                    "fixture digest must identify one canonical sidecar field"
                );
                let drifted = sidecar_text
                    .replacen(original_digest.as_str(), drifted_digest.as_str(), 1)
                    .into_bytes();
                assert_eq!(drifted.len(), original_sidecar_bytes.len());
                assert_ne!(drifted, original_sidecar_bytes);
                drifted
            }
            HeldSidecarMutation::SameBytesIdentityReplacement => original_sidecar_bytes.clone(),
        };
        let hook_reached = Cell::new(false);
        let raced_sidecar_identity = Cell::new(None);

        let error = full_generation::run_with_inter_scan_hook(&fixture.generate_request(), || {
            assert!(
                !hook_reached.replace(true),
                "inter-scan hook ran more than once"
            );
            assert_eq!(fixture.sidecar(), original_sidecar_bytes);
            assert_eq!(path_identity(&sidecar), original_sidecar_identity);
            assert_eq!(snapshot_tree(&expectation_root), before_tree);
            assert_no_import_intent_journal_or_stage(&fixture);
            if matches!(mutation, HeldSidecarMutation::SameBytesIdentityReplacement) {
                fs::rename(&sidecar, &displaced).expect("displace validated import sidecar");
            }
            fs::write(&sidecar, &raced_sidecar_bytes)
                .expect("write raced canonical import sidecar");
            raced_sidecar_identity.set(Some(path_identity(&sidecar)));
        })
        .expect_err("held import sidecar change must reject generation");

        assert!(hook_reached.get(), "inter-scan hook was not reached");
        match mutation {
            HeldSidecarMutation::ByteDrift => {
                assert_eq!(error.kind(), GeneratorErrorKind::Verification);
                assert_eq!(
                    error.to_string(),
                    "validate current CSS import: CSS imported fixture digest is stale: declaration/Case.json"
                );
                assert_eq!(path_identity(&sidecar), original_sidecar_identity);
                assert!(!displaced.exists());
            }
            HeldSidecarMutation::SameBytesIdentityReplacement => {
                assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
                assert_eq!(
                    error.to_string(),
                    "revalidate CSS generation inputs: current CSS import changed after held validation"
                );
                assert_ne!(path_identity(&sidecar), original_sidecar_identity);
                assert_eq!(path_identity(&displaced), original_sidecar_identity);
                assert_eq!(
                    fs::read(&displaced).expect("read displaced sidecar"),
                    original_sidecar_bytes
                );
            }
        }
        assert_eq!(
            Some(path_identity(&sidecar)),
            raced_sidecar_identity.get(),
            "rejection replaced the racer's canonical sidecar"
        );
        assert_eq!(
            fs::read(&sidecar).expect("read raced canonical sidecar"),
            raced_sidecar_bytes,
            "rejection changed the racer's canonical sidecar bytes"
        );
        assert_eq!(snapshot_tree(&expectation_root), before_tree);
        assert_eq!(
            fixture.expectation("declaration/Case.json"),
            before_expectation_bytes
        );
        assert_eq!(fixture.report(), before_report_bytes);
        assert_eq!(path_identity(&expectation_root), before_root_identity);
        assert_eq!(path_identity(&expectation), before_expectation_identity);
        assert_eq!(path_identity(&report), before_report_identity);
        assert_no_import_intent_journal_or_stage(&fixture);
    }

    #[test]
    fn css_full_generate_held_revalidation_rejects_sidecar_byte_drift_before_intent() {
        assert_full_generation_held_sidecar_revalidation(HeldSidecarMutation::ByteDrift);
    }

    #[test]
    fn css_full_generate_held_revalidation_rejects_sidecar_same_byte_identity_replacement_before_intent()
     {
        assert_full_generation_held_sidecar_revalidation(
            HeldSidecarMutation::SameBytesIdentityReplacement,
        );
    }

    #[test]
    fn css_full_generate_held_revalidation_rejects_current_import_byte_or_identity_change_before_intent()
     {
        for replace_identity in [false, true] {
            let fixture = imported_generation_fixture(
                "declaration/Case.json",
                b"{\"case\":{\"source\":\"a\",\"ast\":{}}}\n",
                1,
                &[],
            );
            fixture.generate().expect("seed current expectations");
            let expectation_root = fixture.corpus.join("expectations");
            let expectation = expectation_root.join("declaration/Case.json");
            let report = expectation_root.join("generation-reports/all.json");
            let before_tree = snapshot_tree(&expectation_root);
            let before_root_identity = path_identity(&expectation_root);
            let before_expectation_identity = path_identity(&expectation);
            let before_report_identity = path_identity(&report);
            let before_report_bytes = fixture.report();
            let imported = fixture.imported("declaration/Case.json");
            let displaced = fixture.root.join("displaced-imported-fixture.json");
            let original_import_identity = path_identity(&imported);
            let replacement_identity = Cell::new(None);
            let replacement_bytes = if replace_identity {
                fs::read(&imported).expect("read current imported fixture")
            } else {
                b"{\"case\":{\"source\":\"changed after validation\",\"ast\":{}}}\n".to_vec()
            };

            let error =
                full_generation::run_with_inter_scan_hook(&fixture.generate_request(), || {
                    if replace_identity {
                        fs::rename(&imported, &displaced)
                            .expect("displace validated imported fixture");
                    }
                    fs::write(&imported, &replacement_bytes)
                        .expect("replace validated imported fixture");
                    replacement_identity.set(Some(path_identity(&imported)));
                })
                .expect_err("held import proof change must reject generation");

            if replace_identity {
                assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
                assert_eq!(
                    error.to_string(),
                    "revalidate CSS generation inputs: current CSS import changed after held validation"
                );
            } else {
                assert_eq!(error.kind(), GeneratorErrorKind::Verification);
                assert_eq!(
                    error.to_string(),
                    "validate current CSS import: CSS imported fixture digest is stale: declaration/Case.json"
                );
            }
            let retained_import_identity = path_identity(&imported);
            assert_eq!(Some(retained_import_identity), replacement_identity.get());
            if replace_identity {
                assert_ne!(retained_import_identity, original_import_identity);
            } else {
                assert_eq!(retained_import_identity, original_import_identity);
            }
            assert_eq!(
                fs::read(&imported).expect("read raced import"),
                replacement_bytes
            );
            assert_eq!(snapshot_tree(&expectation_root), before_tree);
            assert_eq!(path_identity(&expectation_root), before_root_identity);
            assert_eq!(path_identity(&expectation), before_expectation_identity);
            assert_eq!(fixture.report(), before_report_bytes);
            assert_eq!(path_identity(&report), before_report_identity);
            assert_no_import_intent_journal_or_stage(&fixture);
        }
    }

    #[test]
    fn css_full_generate_held_revalidation_rejects_historical_report_byte_or_identity_change_before_intent()
     {
        for replace_identity in [false, true] {
            let fixture = imported_generation_fixture(
                "declaration/Case.json",
                b"{\"case\":{\"source\":\"a\",\"ast\":{}}}\n",
                1,
                &[],
            );
            fixture.generate().expect("seed historical authority");
            let expectation_root = fixture.corpus.join("expectations");
            let expectation = expectation_root.join("declaration/Case.json");
            let report = expectation_root.join("generation-reports/all.json");
            let displaced = fixture.root.join("displaced-historical-report.json");
            let mut expected_tree = snapshot_tree(&expectation_root);
            let before_root_identity = path_identity(&expectation_root);
            let before_expectation_identity = path_identity(&expectation);
            let original_report_identity = path_identity(&report);
            let mut replacement_bytes = fixture.report();
            if !replace_identity {
                replacement_bytes.extend_from_slice(b"late historical change\n");
                expected_tree.insert(
                    PathBuf::from("generation-reports/all.json"),
                    replacement_bytes.clone(),
                );
            }
            let replacement_identity = Cell::new(None);

            let error =
                full_generation::run_with_inter_scan_hook(&fixture.generate_request(), || {
                    if replace_identity {
                        fs::rename(&report, &displaced)
                            .expect("displace validated historical report");
                    }
                    fs::write(&report, &replacement_bytes)
                        .expect("replace validated historical report");
                    replacement_identity.set(Some(path_identity(&report)));
                })
                .expect_err("held historical authority change must reject generation");

            assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
            assert_eq!(
                error.to_string(),
                "revalidate current publication tree: publication inventory changed before transaction intent"
            );
            let retained_report_identity = path_identity(&report);
            assert_eq!(Some(retained_report_identity), replacement_identity.get());
            if replace_identity {
                assert_ne!(retained_report_identity, original_report_identity);
            } else {
                assert_eq!(retained_report_identity, original_report_identity);
            }
            assert_eq!(snapshot_tree(&expectation_root), expected_tree);
            assert_eq!(path_identity(&expectation_root), before_root_identity);
            assert_eq!(path_identity(&expectation), before_expectation_identity);
            assert_eq!(fixture.report(), replacement_bytes);
            assert_no_import_intent_journal_or_stage(&fixture);
        }
    }

    #[test]
    fn css_full_generate_rejects_persisted_report_path_collision() {
        let fixture = imported_generation_fixture(
            "declaration/Case.json",
            b"{\"case\":{\"source\":\"a\",\"ast\":{}}}\n",
            1,
            &[],
        );
        let sidecar_path = fixture.imported(".surgeist-source.json");
        let sidecar = String::from_utf8(fs::read(&sidecar_path).expect("sidecar"))
            .expect("UTF-8 sidecar")
            .replace("declaration/Case.json", "generation-reports/all.json");
        fs::write(&sidecar_path, sidecar).expect("persist colliding sidecar");
        fs::remove_dir_all(fixture.corpus.join("source/declaration"))
            .expect("remove original source fixture");
        let colliding = fixture.imported("generation-reports/all.json");
        fs::create_dir_all(colliding.parent().expect("collision parent"))
            .expect("create collision parent");
        fs::write(colliding, b"{\"case\":{\"source\":\"a\",\"ast\":{}}}\n")
            .expect("write colliding fixture");

        let error = fixture
            .generate()
            .expect_err("persisted report path collision");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
        assert!(!fixture.corpus.join("expectations").exists());
    }
}
