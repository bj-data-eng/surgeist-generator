use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::core::{Domain, GenerationLease, ObjectFormat, SnapshotEntry, VerifiedSourceSnapshot};
use crate::{
    CorpusLocation, GeneratorErrorKind, PinnedSource, RelativePath, RunScope, Sha256Digest,
    SourceRevision,
};

use super::tests::{SHA1_REVISION, manifest_text};
use super::{LayoutRequest, manifest, sidecar};

const SOURCE: &[u8] = b"<div>fixture</div>\n";
const HELPER: &[u8] = b"window.__surgeist = true;\n";
const BASE_STYLE: &[u8] = b"html, body { margin: 0; }\n";
const BROWSER_DIGEST: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const BROWSER_PROVENANCE: &str = "Chrome 123.0.1 browser-cache/chrome";
const VARIANTS: [&str; 4] = [
    "border_box_ltr",
    "border_box_rtl",
    "content_box_ltr",
    "content_box_rtl",
];

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    owner: PathBuf,
    corpus: PathBuf,
    location: CorpusLocation,
    source: Vec<u8>,
}

impl Fixture {
    fn current() -> Self {
        Self::with_source(SOURCE)
    }

    fn with_source(source: &[u8]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "surgeist-generator-layout-check-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        let owner = root.join("owner");
        let corpus = owner.join("corpus");
        fs::create_dir_all(&corpus).expect("create layout check corpus");
        let location = CorpusLocation::new(&owner, &corpus).expect("layout check location");
        let fixture = Self {
            root,
            owner,
            corpus,
            location,
            source: source.to_vec(),
        };
        fixture.write_manifest(1);
        write_file(
            &fixture.corpus.join("scripts/gentest/test_helper.js"),
            HELPER,
        );
        write_file(
            &fixture.corpus.join("scripts/gentest/test_base_style.css"),
            BASE_STYLE,
        );
        fixture.write_import(&[("grid/basic.html", source)]);
        fixture.write_current(BROWSER_PROVENANCE, BROWSER_DIGEST, false);
        fixture
    }

    fn with_dispositions() -> Self {
        let fixture = Self::current();
        let cases = r#"[[cases]]
id = "authored/expected"
source_root = "surgeist"
source = "authored/expected.html"
generator = "constrained-html"
status = "expected-fail"
reason = " known padded reason "

[[cases]]
id = "authored/unsupported"
source_root = "surgeist"
source = "authored/unsupported.html"
generator = "constrained-html"
status = "unsupported"

[[cases]]
id = "authored/quarantined"
source_root = "surgeist"
source = "authored/quarantined.html"
generator = "constrained-html"
status = "quarantined"
reason = ""
"#;
        let manifest = manifest_text(SHA1_REVISION, 1, cases)
            .replacen("generated = 4", "generated = 8", 1)
            .replacen("unsupported = 0", "unsupported = 1", 1)
            .replacen("expected_fail = 0", "expected_fail = 1", 1)
            .replacen("quarantined = 0", "quarantined = 1", 1);
        write_file(&fixture.corpus.join("corpus.toml"), manifest.as_bytes());
        write_file(
            &fixture.corpus.join("html/authored/expected.html"),
            b"<div>expected</div>\n",
        );
        write_file(
            &fixture.corpus.join("html/authored/unsupported.html"),
            b"<div>unsupported</div>\n",
        );
        write_file(
            &fixture.corpus.join("html/authored/quarantined.html"),
            b"<div>quarantined</div>\n",
        );
        fixture.write_disposition_state();
        fixture
    }

    fn with_failure_outside_scope() -> Self {
        let fixture = Self::current();
        let cases = r#"[[cases]]
id = "authored/outside"
source_root = "surgeist"
source = "authored/outside.html"
generator = "constrained-html"
status = "active"
"#;
        write_file(
            &fixture.corpus.join("corpus.toml"),
            manifest_text(SHA1_REVISION, 1, cases).as_bytes(),
        );
        write_file(
            &fixture.corpus.join("html/authored/outside.html"),
            b"<div>outside scope</div>\n",
        );
        fixture.write_diagnostic_outside_scope();
        fixture
    }

    fn request(&self) -> LayoutRequest {
        LayoutRequest::check_corpus(self.location.clone())
    }

    fn check(&self) -> crate::Result<()> {
        crate::layout::run(self.request())
    }

    fn write_manifest(&self, count: usize) {
        write_file(
            &self.corpus.join("corpus.toml"),
            manifest_text(SHA1_REVISION, count, "").as_bytes(),
        );
    }

    fn write_import(&self, files: &[(&str, &[u8])]) {
        for (path, bytes) in files {
            write_file(&self.corpus.join("html").join(path), bytes);
        }
        let snapshot = VerifiedSourceSnapshot {
            object_format: ObjectFormat::Sha1,
            entries: files
                .iter()
                .enumerate()
                .map(|(index, (path, bytes))| SnapshotEntry {
                    path: RelativePath::new(path).expect("Taffy fixture path"),
                    git_mode: "100644".to_owned(),
                    blob_object_id: format!("{index:040x}"),
                    digest: Sha256Digest::from_bytes(bytes),
                    bytes: bytes.to_vec(),
                })
                .collect(),
        };
        let pin = PinnedSource::new(
            "taffy",
            manifest::TAFFY_REPOSITORY,
            SourceRevision::new(SHA1_REVISION).expect("revision"),
            RelativePath::new(manifest::TAFFY_SOURCE_DIRECTORY).expect("source directory"),
        )
        .expect("Taffy pin");
        let bytes = sidecar::canonical_bytes(&pin, files.len(), &snapshot).expect("sidecar");
        write_file(
            &self.corpus.join("html/.surgeist-taffy-source.json"),
            &bytes,
        );
    }

    fn write_current(&self, browser: &str, browser_digest: &str, legacy: bool) {
        let metadata = self.metadata(browser, browser_digest);
        let mut records = Vec::new();
        for variant in VARIANTS {
            let output = format!("xml/grid/basic__{variant}.xml");
            let bytes = self.xml_bytes(variant, browser, browser_digest, None);
            write_file(&self.corpus.join(&output), &bytes);
            records.push(Record {
                name: format!("basic__{variant}"),
                source: "html/grid/basic.html".to_owned(),
                output,
                digest: Sha256Digest::from_bytes(&bytes).to_string(),
                variant: variant.to_owned(),
            });
        }
        self.write_reports(&metadata, &records, legacy);
    }

    fn write_reports(&self, metadata: &Metadata, records: &[Record], legacy: bool) {
        write_file(
            &self.corpus.join("xml/generation-reports/all.json"),
            report_json(metadata, None, records, legacy).as_bytes(),
        );
        write_file(
            &self.corpus.join("xml/generation-reports/grid.json"),
            report_json(metadata, Some("grid"), records, legacy).as_bytes(),
        );
    }

    fn refresh_report_digests(&self, browser: &str, browser_digest: &str) {
        let metadata = self.metadata(browser, browser_digest);
        let records = VARIANTS
            .into_iter()
            .map(|variant| {
                let output = format!("xml/grid/basic__{variant}.xml");
                let bytes = fs::read(self.corpus.join(&output)).expect("read rewritten XML");
                Record {
                    name: format!("basic__{variant}"),
                    source: "html/grid/basic.html".to_owned(),
                    output,
                    digest: Sha256Digest::from_bytes(bytes).to_string(),
                    variant: variant.to_owned(),
                }
            })
            .collect::<Vec<_>>();
        self.write_reports(&metadata, &records, false);
    }

    fn write_diagnostic(&self) {
        if self.corpus.join("xml").exists() {
            fs::remove_dir_all(self.corpus.join("xml")).expect("remove clean XML state");
        }
        let metadata = self.metadata(BROWSER_PROVENANCE, BROWSER_DIGEST);
        for (file, filter) in [("all.json", "null"), ("grid.json", "\"grid\"")] {
            let bytes = format!(
                "{{\n  \"metadata\": {metadata},\n  \"filter\": {filter},\n  \"summary\": {{\n    \"generated\": 0,\n    \"unsupported\": 0,\n    \"expected_fail\": 0,\n    \"quarantined\": 0,\n    \"failed_to_generate\": 1\n  }},\n  \"generated\": [],\n  \"unsupported\": [],\n  \"expected_fail\": [],\n  \"quarantined\": [],\n  \"failed_to_generate\": [\n    {{\n      \"name\": \"basic\",\n      \"source\": \"html/grid/basic.html\",\n      \"reason\": \"browser job failed\"\n    }}\n  ]\n}}\n",
                metadata = metadata.pretty_object(),
            );
            write_file(
                &self.corpus.join("xml/generation-reports").join(file),
                bytes.as_bytes(),
            );
        }
    }

    fn write_disposition_state(&self) {
        fs::remove_dir_all(self.corpus.join("xml")).expect("remove prior XML state");
        let metadata = self.metadata(BROWSER_PROVENANCE, BROWSER_DIGEST);
        let sources = [
            (
                "authored/expected.html",
                b"<div>expected</div>\n".as_slice(),
            ),
            ("grid/basic.html", SOURCE),
        ];
        let mut records = Vec::new();
        for (source, bytes) in sources {
            let stem = Path::new(source)
                .file_stem()
                .expect("source stem")
                .to_str()
                .expect("UTF-8 stem");
            let parent = Path::new(source)
                .parent()
                .expect("source parent")
                .to_str()
                .expect("UTF-8 parent");
            for variant in VARIANTS {
                let output = format!("xml/{parent}/{stem}__{variant}.xml");
                let xml = xml_bytes_for(
                    source,
                    bytes,
                    variant,
                    &metadata,
                    BROWSER_PROVENANCE,
                    BROWSER_DIGEST,
                );
                write_file(&self.corpus.join(&output), &xml);
                records.push(Record {
                    name: format!("{stem}__{variant}"),
                    source: format!("html/{source}"),
                    output,
                    digest: Sha256Digest::from_bytes(&xml).to_string(),
                    variant: variant.to_owned(),
                });
            }
        }
        records.sort_by(|left, right| {
            (
                &left.source,
                &left.name,
                &left.variant,
                &left.output,
                &left.digest,
            )
                .cmp(&(
                    &right.source,
                    &right.name,
                    &right.variant,
                    &right.output,
                    &right.digest,
                ))
        });
        let mut full = report_json(&metadata, None, &records, false)
            .replacen("\"unsupported\": 0", "\"unsupported\": 1", 1)
            .replacen("\"expected_fail\": 0", "\"expected_fail\": 1", 1)
            .replacen("\"quarantined\": 0", "\"quarantined\": 1", 1);
        full = full.replace(
            "  \"unsupported\": []",
            "  \"unsupported\": [\n    {\n      \"name\": \"authored/unsupported\",\n      \"source\": \"html/authored/unsupported.html\",\n      \"variant\": \"manifest\",\n      \"reason\": \"manifest marks case unsupported\"\n    }\n  ]",
        );
        full = full.replace(
            "  \"expected_fail\": []",
            "  \"expected_fail\": [\n    {\n      \"name\": \"authored/expected\",\n      \"source\": \"html/authored/expected.html\",\n      \"reason\": \" known padded reason \"\n    }\n  ]",
        );
        full = full.replace(
            "  \"quarantined\": []",
            "  \"quarantined\": [\n    {\n      \"name\": \"authored/quarantined\",\n      \"source\": \"html/authored/quarantined.html\",\n      \"reason\": \"\"\n    }\n  ]",
        );
        write_file(
            &self.corpus.join("xml/generation-reports/all.json"),
            full.as_bytes(),
        );
        let grid = records
            .iter()
            .filter(|record| record.source == "html/grid/basic.html")
            .map(|record| Record {
                name: record.name.clone(),
                source: record.source.clone(),
                output: record.output.clone(),
                digest: record.digest.clone(),
                variant: record.variant.clone(),
            })
            .collect::<Vec<_>>();
        write_file(
            &self.corpus.join("xml/generation-reports/grid.json"),
            report_json(&metadata, Some("grid"), &grid, false).as_bytes(),
        );
    }

    fn write_diagnostic_outside_scope(&self) {
        fs::remove_dir_all(self.corpus.join("xml")).expect("remove prior XML state");
        let metadata = self.metadata(BROWSER_PROVENANCE, BROWSER_DIGEST);
        let mut records = Vec::new();
        for variant in VARIANTS {
            let output = format!("xml/grid/basic__{variant}.xml");
            let xml = xml_bytes_for(
                "grid/basic.html",
                SOURCE,
                variant,
                &metadata,
                BROWSER_PROVENANCE,
                BROWSER_DIGEST,
            );
            write_file(&self.corpus.join(&output), &xml);
            records.push(Record {
                name: format!("basic__{variant}"),
                source: "html/grid/basic.html".to_owned(),
                output,
                digest: Sha256Digest::from_bytes(&xml).to_string(),
                variant: variant.to_owned(),
            });
        }
        let mut full = report_json(&metadata, None, &records, false).replacen(
            "\"failed_to_generate\": 0",
            "\"failed_to_generate\": 1",
            1,
        );
        full = full.replace(
            "  \"failed_to_generate\": []",
            "  \"failed_to_generate\": [\n    {\n      \"name\": \"outside\",\n      \"source\": \"html/authored/outside.html\",\n      \"reason\": \"browser job failed\"\n    }\n  ]",
        );
        write_file(
            &self.corpus.join("xml/generation-reports/all.json"),
            full.as_bytes(),
        );
        write_file(
            &self.corpus.join("xml/generation-reports/grid.json"),
            report_json(&metadata, Some("grid"), &records, false).as_bytes(),
        );
    }

    fn metadata(&self, browser: &str, browser_digest: &str) -> Metadata {
        let manifest_bytes = fs::read(self.corpus.join("corpus.toml")).expect("read manifest");
        let (_, launch_digest) =
            manifest::parse_with_launch_digest(&manifest_bytes, &self.corpus.join("corpus.toml"))
                .expect("parse manifest for launch digest");
        Metadata {
            browser: browser.to_owned(),
            browser_digest: browser_digest.to_owned(),
            launch_digest: launch_digest.to_string(),
            helper_digest: Sha256Digest::from_bytes(HELPER).to_string(),
            base_style_digest: Sha256Digest::from_bytes(BASE_STYLE).to_string(),
            manifest_digest: Sha256Digest::from_bytes(&manifest_bytes).to_string(),
            sidecar_digest: Sha256Digest::from_bytes(
                fs::read(self.corpus.join("html/.surgeist-taffy-source.json"))
                    .expect("read sidecar"),
            )
            .to_string(),
        }
    }

    fn xml_bytes(
        &self,
        variant: &str,
        browser: &str,
        browser_digest: &str,
        body: Option<&str>,
    ) -> Vec<u8> {
        let metadata = self.metadata(browser, browser_digest);
        let source_digest = Sha256Digest::from_bytes(&self.source);
        let optional_base = if contains(&self.source, b"test_base_style.css") {
            format!(" base-style-sha256=\"{}\"", metadata.base_style_digest)
        } else {
            String::new()
        };
        format!(
            "<!-- generated-by: surgeist-layout-generate schema=2 source=\"html/grid/basic.html\" source-sha256=\"{source_digest}\" helper-sha256=\"{}\"{optional_base} browser=\"{}\" browser-executable-sha256=\"{browser_digest}\" launch-profile-sha256=\"{}\" corpus-manifest-sha256=\"{}\" taffy-revision=\"{SHA1_REVISION}\" taffy-sidecar-sha256=\"{}\" -->\n{}\n",
            metadata.helper_digest,
            escape_xml_attribute(browser),
            metadata.launch_digest,
            metadata.manifest_digest,
            metadata.sidecar_digest,
            body.map_or_else(
                || format!("<test name=\"basic__{variant}\"/>") ,
                str::to_owned,
            ),
        )
        .into_bytes()
    }

    fn assert_preserved(&self, expected: Option<GeneratorErrorKind>) {
        let before = snapshot(&self.root);
        let result = self.check();
        match expected {
            None => result.expect("current layout corpus"),
            Some(kind) => {
                let error = result.expect_err("layout check rejection");
                assert_eq!(error.kind(), kind, "unexpected diagnostic: {error}");
            }
        }
        assert_eq!(snapshot(&self.root), before);
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).expect("remove layout check fixture");
    }
}

#[derive(Clone)]
struct Metadata {
    browser: String,
    browser_digest: String,
    launch_digest: String,
    helper_digest: String,
    base_style_digest: String,
    manifest_digest: String,
    sidecar_digest: String,
}

impl Metadata {
    fn pretty_object(&self) -> String {
        format!(
            "{{\n    \"schema_version\": 2,\n    \"generator\": \"surgeist-layout-generate\",\n    \"browser_source\": \"chrome-for-testing\",\n    \"browser_version\": \"123.0.1\",\n    \"browser_provenance\": \"{}\",\n    \"browser_executable_sha256\": \"{}\",\n    \"launch_profile_sha256\": \"{}\",\n    \"helper_sha256\": \"{}\",\n    \"base_style_sha256\": \"{}\",\n    \"corpus_manifest_sha256\": \"{}\",\n    \"taffy_revision\": \"{SHA1_REVISION}\",\n    \"taffy_sidecar_sha256\": \"{}\"\n  }}",
            escape_json(&self.browser),
            self.browser_digest,
            self.launch_digest,
            self.helper_digest,
            self.base_style_digest,
            self.manifest_digest,
            self.sidecar_digest,
        )
    }
}

struct Record {
    name: String,
    source: String,
    output: String,
    digest: String,
    variant: String,
}

fn report_json(
    metadata: &Metadata,
    filter: Option<&str>,
    records: &[Record],
    legacy: bool,
) -> String {
    let generated = if records.is_empty() {
        "[]".to_owned()
    } else {
        format!(
            "[\n{}\n  ]",
            records
                .iter()
                .map(|record| {
                    let digest = if legacy {
                        String::new()
                    } else {
                        format!("\n      \"output_sha256\": \"{}\",", record.digest)
                    };
                    format!(
                        "    {{\n      \"name\": \"{}\",\n      \"source\": \"{}\",\n      \"output\": \"{}\",{digest}\n      \"variant\": \"{}\"\n    }}",
                        record.name, record.source, record.output, record.variant
                    )
                })
                .collect::<Vec<_>>()
                .join(",\n")
        )
    };
    let filter = filter.map_or_else(|| "null".to_owned(), |value| format!("\"{value}\""));
    format!(
        "{{\n  \"metadata\": {},\n  \"filter\": {filter},\n  \"summary\": {{\n    \"generated\": {},\n    \"unsupported\": 0,\n    \"expected_fail\": 0,\n    \"quarantined\": 0,\n    \"failed_to_generate\": 0\n  }},\n  \"generated\": {generated},\n  \"unsupported\": [],\n  \"expected_fail\": [],\n  \"quarantined\": [],\n  \"failed_to_generate\": []\n}}\n",
        metadata.pretty_object(),
        records.len(),
    )
}

fn xml_bytes_for(
    source: &str,
    source_bytes: &[u8],
    variant: &str,
    metadata: &Metadata,
    browser: &str,
    browser_digest: &str,
) -> Vec<u8> {
    let stem = Path::new(source)
        .file_stem()
        .expect("source stem")
        .to_str()
        .expect("UTF-8 stem");
    format!(
        "<!-- generated-by: surgeist-layout-generate schema=2 source=\"html/{source}\" source-sha256=\"{}\" helper-sha256=\"{}\" browser=\"{}\" browser-executable-sha256=\"{browser_digest}\" launch-profile-sha256=\"{}\" corpus-manifest-sha256=\"{}\" taffy-revision=\"{SHA1_REVISION}\" taffy-sidecar-sha256=\"{}\" -->\n<test name=\"{stem}__{variant}\"/>\n",
        Sha256Digest::from_bytes(source_bytes),
        metadata.helper_digest,
        escape_xml_attribute(browser),
        metadata.launch_digest,
        metadata.manifest_digest,
        metadata.sidecar_digest,
    )
    .into_bytes()
}

fn write_file(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().expect("fixture parent")).expect("create fixture parent");
    fs::write(path, bytes).expect("write fixture file");
    fs::set_permissions(path, fs::Permissions::from_mode(0o644)).expect("set fixture file mode");
    fs::set_permissions(
        path.parent().expect("fixture parent"),
        fs::Permissions::from_mode(0o755),
    )
    .expect("set fixture parent mode");
}

fn snapshot(root: &Path) -> BTreeMap<PathBuf, (u32, u64, u64, Vec<u8>)> {
    fn visit(
        base: &Path,
        current: &Path,
        output: &mut BTreeMap<PathBuf, (u32, u64, u64, Vec<u8>)>,
    ) {
        let metadata = fs::symlink_metadata(current).expect("snapshot metadata");
        let kind = if metadata.is_dir() { 1 } else { 2 };
        output.insert(
            current
                .strip_prefix(base)
                .expect("relative snapshot")
                .to_path_buf(),
            (
                kind,
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
            let mut children = fs::read_dir(current)
                .expect("snapshot directory")
                .collect::<std::result::Result<Vec<_>, _>>()
                .expect("snapshot entries");
            children.sort_by_key(fs::DirEntry::file_name);
            for child in children {
                visit(base, &child.path(), output);
            }
        }
    }
    let mut output = BTreeMap::new();
    visit(root, root, &mut output);
    output
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

fn escape_xml_attribute(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('\"', "&quot;")
        .replace('<', "&lt;")
}

fn escape_json(value: &str) -> String {
    value.replace('\\', "\\\\").replace('\"', "\\\"")
}

#[test]
fn layout_check_corpus_current_state_is_read_only() {
    Fixture::current().assert_preserved(None);
}

#[test]
fn layout_check_corpus_absent_stale_and_diagnostic_states_are_verification() {
    let absent = Fixture::current();
    fs::remove_dir_all(absent.corpus.join("xml")).expect("remove XML state");
    absent.assert_preserved(Some(GeneratorErrorKind::Verification));

    let stale = Fixture::current();
    write_file(
        &stale.corpus.join("scripts/gentest/test_helper.js"),
        b"changed helper\n",
    );
    stale.assert_preserved(Some(GeneratorErrorKind::Verification));

    let diagnostic = Fixture::current();
    diagnostic.write_diagnostic();
    diagnostic.assert_preserved(Some(GeneratorErrorKind::Verification));
}

#[test]
fn layout_check_corpus_import_and_helper_states_keep_exact_error_kinds() {
    let missing_sidecar = Fixture::current();
    fs::remove_file(
        missing_sidecar
            .corpus
            .join("html/.surgeist-taffy-source.json"),
    )
    .expect("remove sidecar");
    missing_sidecar.assert_preserved(Some(GeneratorErrorKind::Verification));

    let stale_import = Fixture::current();
    write_file(
        &stale_import.corpus.join("html/grid/basic.html"),
        b"stale imported bytes\n",
    );
    stale_import.assert_preserved(Some(GeneratorErrorKind::Verification));

    let malformed_sidecar = Fixture::current();
    write_file(
        &malformed_sidecar
            .corpus
            .join("html/.surgeist-taffy-source.json"),
        b"{}\n",
    );
    malformed_sidecar.assert_preserved(Some(GeneratorErrorKind::InvalidInventory));

    let unknown_html = Fixture::current();
    write_file(&unknown_html.corpus.join("html/unknown.html"), b"unknown\n");
    unknown_html.assert_preserved(Some(GeneratorErrorKind::InvalidInventory));

    let missing_helper = Fixture::current();
    fs::remove_file(missing_helper.corpus.join("scripts/gentest/test_helper.js"))
        .expect("remove helper");
    missing_helper.assert_preserved(Some(GeneratorErrorKind::Verification));

    let malformed_helper = Fixture::current();
    let helper = malformed_helper
        .corpus
        .join("scripts/gentest/test_helper.js");
    fs::set_permissions(&helper, fs::Permissions::from_mode(0o600)).expect("malform helper mode");
    malformed_helper.assert_preserved(Some(GeneratorErrorKind::InvalidInventory));
}

#[test]
fn layout_historical_inventory_rejects_malformed_authority_and_unknown_entries() {
    let malformed = Fixture::current();
    fs::remove_file(malformed.corpus.join("xml/generation-reports/all.json"))
        .expect("remove full authority");
    malformed.assert_preserved(Some(GeneratorErrorKind::InvalidInventory));

    let unknown = Fixture::current();
    write_file(&unknown.corpus.join("xml/unknown.xml"), b"unknown\n");
    unknown.assert_preserved(Some(GeneratorErrorKind::InvalidInventory));
}

#[test]
fn layout_historical_inventory_membership_delta_is_classifiable_stale() {
    let fixture = Fixture::current();
    fixture.write_manifest(2);
    fixture.write_import(&[
        ("grid/basic.html", SOURCE),
        ("grid/new.html", b"<div>new</div>\n"),
    ]);
    fixture.assert_preserved(Some(GeneratorErrorKind::Verification));
}

#[test]
fn layout_legacy_report_requires_complete_report_migration() {
    let fixture = Fixture::current();
    fixture.write_current(BROWSER_PROVENANCE, BROWSER_DIGEST, true);
    fixture.assert_preserved(Some(GeneratorErrorKind::Verification));
}

#[test]
fn layout_legacy_report_rejects_mixed_current_and_legacy_authority() {
    let fixture = Fixture::current();
    let legacy = fixture.metadata(BROWSER_PROVENANCE, BROWSER_DIGEST);
    let records = VARIANTS
        .into_iter()
        .map(|variant| {
            let output = format!("xml/grid/basic__{variant}.xml");
            Record {
                name: format!("basic__{variant}"),
                source: "html/grid/basic.html".to_owned(),
                digest: Sha256Digest::from_bytes(
                    fs::read(fixture.corpus.join(&output)).expect("XML bytes"),
                )
                .to_string(),
                output,
                variant: variant.to_owned(),
            }
        })
        .collect::<Vec<_>>();
    write_file(
        &fixture.corpus.join("xml/generation-reports/grid.json"),
        report_json(&legacy, Some("grid"), &records, true).as_bytes(),
    );
    fixture.assert_preserved(Some(GeneratorErrorKind::InvalidInventory));
}

#[test]
fn layout_report_rejects_tampered_xml_body() {
    let fixture = Fixture::current();
    let path = fixture.corpus.join("xml/grid/basic__border_box_ltr.xml");
    let bytes = fs::read(&path).expect("read XML");
    let text = String::from_utf8(bytes).expect("UTF-8 XML");
    write_file(
        &path,
        text.replace("basic__border_box_ltr", "tampered").as_bytes(),
    );
    fixture.assert_preserved(Some(GeneratorErrorKind::Verification));
}

#[test]
fn layout_report_rejects_invalid_counts_variants_coverage_and_scoped_subsets() {
    let counts = Fixture::current();
    let path = counts.corpus.join("xml/generation-reports/all.json");
    let text = fs::read_to_string(&path).expect("full report");
    write_file(
        &path,
        text.replacen("\"generated\": 4", "\"generated\": 3", 1)
            .as_bytes(),
    );
    counts.assert_preserved(Some(GeneratorErrorKind::InvalidInventory));

    let variant = Fixture::current();
    let path = variant.corpus.join("xml/generation-reports/all.json");
    let text = fs::read_to_string(&path).expect("full report");
    write_file(
        &path,
        text.replacen(
            "\"variant\": \"border_box_ltr\"",
            "\"variant\": \"bogus\"",
            1,
        )
        .as_bytes(),
    );
    variant.assert_preserved(Some(GeneratorErrorKind::InvalidInventory));

    let coverage = Fixture::current();
    let metadata = coverage.metadata(BROWSER_PROVENANCE, BROWSER_DIGEST);
    let mut records = VARIANTS
        .into_iter()
        .map(|variant| {
            let output = format!("xml/grid/basic__{variant}.xml");
            Record {
                name: format!("basic__{variant}"),
                source: "html/grid/basic.html".to_owned(),
                digest: Sha256Digest::from_bytes(
                    fs::read(coverage.corpus.join(&output)).expect("XML bytes"),
                )
                .to_string(),
                output,
                variant: variant.to_owned(),
            }
        })
        .collect::<Vec<_>>();
    records.pop();
    coverage.write_reports(&metadata, &records, false);
    coverage.assert_preserved(Some(GeneratorErrorKind::InvalidInventory));

    let scoped = Fixture::current();
    let path = scoped.corpus.join("xml/generation-reports/grid.json");
    let text = fs::read_to_string(&path).expect("scoped report");
    write_file(
        &path,
        text.replacen("\"filter\": \"grid\"", "\"filter\": \"other\"", 1)
            .as_bytes(),
    );
    scoped.assert_preserved(Some(GeneratorErrorKind::InvalidInventory));
}

#[test]
fn layout_report_validates_four_variant_disposition_count_and_coverage() {
    Fixture::with_dispositions().assert_preserved(None);
}

#[test]
fn layout_provenance_rejects_duplicate_unknown_or_misordered_fields() {
    let rewrites: [fn(String) -> String; 3] = [
        |line: String| {
            line.replacen(
                " helper-sha256=",
                " source-sha256=\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\" helper-sha256=",
                1,
            )
        },
        |line: String| line.replacen(" helper-sha256=", " unknown=\"value\" helper-sha256=", 1),
        |line: String| {
            line.replacen(
                " source-sha256=",
                " helper-sha256=\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\" source-sha256=",
                1,
            )
        },
    ];
    for rewrite in rewrites {
        let fixture = Fixture::current();
        let path = fixture.corpus.join("xml/grid/basic__border_box_ltr.xml");
        let text = fs::read_to_string(&path).expect("XML text");
        write_file(&path, rewrite(text).as_bytes());
        fixture.refresh_report_digests(BROWSER_PROVENANCE, BROWSER_DIGEST);
        fixture.assert_preserved(Some(GeneratorErrorKind::InvalidInventory));
    }

    let optional = Fixture::current();
    let path = optional.corpus.join("xml/grid/basic__border_box_ltr.xml");
    let text = fs::read_to_string(&path).expect("XML text");
    let digest = Sha256Digest::from_bytes(BASE_STYLE);
    write_file(
        &path,
        text.replacen(
            " browser=",
            &format!(" base-style-sha256=\"{digest}\" browser="),
            1,
        )
        .as_bytes(),
    );
    optional.refresh_report_digests(BROWSER_PROVENANCE, BROWSER_DIGEST);
    optional.assert_preserved(Some(GeneratorErrorKind::InvalidInventory));
}

#[test]
fn layout_xml_optional_provenance_complete_golden() {
    Fixture::with_source(b"<link href=\"test_base_style.css\">\n").assert_preserved(None);
}

#[test]
fn layout_xml_provenance_complete_golden() {
    let fixture = Fixture::current();
    let bytes =
        fs::read(fixture.corpus.join("xml/grid/basic__border_box_ltr.xml")).expect("XML golden");
    assert!(bytes.starts_with(b"<!-- generated-by: surgeist-layout-generate schema=2 "));
    assert!(bytes.ends_with(b"\n"));
    fixture.assert_preserved(None);
}

#[test]
fn layout_xml_preserved_escape_complete_golden() {
    let fixture = Fixture::current();
    let manifest_path = fixture.corpus.join("corpus.toml");
    let manifest = fs::read_to_string(&manifest_path)
        .expect("manifest text")
        .replace(
            "Chrome {version} {repository_relative_executable}",
            "Chrome & \\\" < > {version} {repository_relative_executable}",
        );
    write_file(&manifest_path, manifest.as_bytes());
    let browser = "Chrome & \" < > 123.0.1 browser-cache/chrome";
    fixture.write_current(browser, BROWSER_DIGEST, false);
    let xml = fs::read_to_string(fixture.corpus.join("xml/grid/basic__border_box_ltr.xml"))
        .expect("escaped XML");
    assert!(xml.contains("browser=\"Chrome &amp; &quot; &lt; > 123.0.1"));
    fixture.assert_preserved(None);
}

#[test]
fn layout_report_generated_digest_complete_golden() {
    let fixture = Fixture::current();
    let report: serde_json::Value = serde_json::from_slice(
        &fs::read(fixture.corpus.join("xml/generation-reports/all.json")).expect("report golden"),
    )
    .expect("report JSON");
    for generated in report["generated"].as_array().expect("generated entries") {
        let output = generated["output"].as_str().expect("output path");
        assert_eq!(
            generated["output_sha256"].as_str().expect("output digest"),
            Sha256Digest::from_bytes(fs::read(fixture.corpus.join(output)).expect("generated XML"))
                .as_str()
        );
    }
    fixture.assert_preserved(None);
}

#[test]
fn layout_diagnostic_full_failure_inside_scope_golden() {
    let fixture = Fixture::current();
    fixture.write_diagnostic();
    fixture.assert_preserved(Some(GeneratorErrorKind::Verification));
}

#[test]
fn layout_diagnostic_full_failure_outside_scope_golden() {
    Fixture::with_failure_outside_scope().assert_preserved(Some(GeneratorErrorKind::Verification));
}

#[test]
fn layout_diagnostic_summary_is_actual_bucket_lengths() {
    let fixture = Fixture::with_failure_outside_scope();
    let report: serde_json::Value = serde_json::from_slice(
        &fs::read(fixture.corpus.join("xml/generation-reports/all.json"))
            .expect("diagnostic report"),
    )
    .expect("diagnostic JSON");
    assert_eq!(report["summary"]["generated"], 4);
    assert_eq!(report["summary"]["failed_to_generate"], 1);
    fixture.assert_preserved(Some(GeneratorErrorKind::Verification));
}

#[test]
fn layout_diagnostic_rejects_unexplained_count_divergence() {
    let fixture = Fixture::with_failure_outside_scope();
    let report_path = fixture.corpus.join("xml/generation-reports/grid.json");
    let report = fs::read_to_string(&report_path).expect("scoped diagnostic");
    write_file(
        &report_path,
        report
            .replacen("\"generated\": 4", "\"generated\": 3", 1)
            .as_bytes(),
    );
    fixture.assert_preserved(Some(GeneratorErrorKind::InvalidInventory));
}

#[test]
fn layout_check_ignores_absent_browser_cache() {
    let fixture = Fixture::current();
    assert!(!fixture.owner.join("browser-cache").exists());
    fixture.assert_preserved(None);
}

#[test]
fn layout_check_ignores_replaced_browser_identity() {
    let fixture = Fixture::current();
    let path = fixture.owner.join("browser-cache/chrome");
    write_file(&path, b"first browser bytes\n");
    let before = fs::symlink_metadata(&path).expect("browser identity").ino();
    fs::remove_file(&path).expect("replace browser");
    write_file(&path, b"replacement browser bytes\n");
    assert_ne!(
        fs::symlink_metadata(&path).expect("new browser").ino(),
        before
    );
    fixture.assert_preserved(None);
}

#[test]
fn layout_check_ignores_browser_byte_drift() {
    let fixture = Fixture::current();
    let path = fixture.owner.join("browser-cache/chrome");
    write_file(&path, b"first browser bytes\n");
    fixture.assert_preserved(None);
    write_file(&path, b"drifted browser bytes\n");
    fixture.assert_preserved(None);
}

#[test]
fn layout_check_rejects_cross_artifact_browser_digest_mismatch() {
    let fixture = Fixture::current();
    let path = fixture.corpus.join("xml/grid/basic__border_box_ltr.xml");
    let text = fs::read_to_string(&path).expect("XML text");
    write_file(
        &path,
        text.replacen(BROWSER_DIGEST, &"c".repeat(64), 1).as_bytes(),
    );
    fixture.refresh_report_digests(BROWSER_PROVENANCE, BROWSER_DIGEST);
    fixture.assert_preserved(Some(GeneratorErrorKind::InvalidInventory));
}

#[test]
fn layout_check_accepts_self_consistent_historical_browser_attestation_rewrite() {
    let fixture = Fixture::current();
    fixture.write_current(
        "Chrome 123.0.1 browser-cache/replaced/chrome",
        &"c".repeat(64),
        false,
    );
    fixture.assert_preserved(None);
}

#[test]
fn layout_read_only_corpus_coordination_states_are_verification_and_byte_identical() {
    let active = Fixture::current();
    let lease = GenerationLease::acquire(
        &active.location,
        Domain::Layout,
        "surgeist-layout-generate",
        &RunScope::Full,
        "generate",
    )
    .expect("hold exclusive lease");
    active.assert_preserved(Some(GeneratorErrorKind::Verification));
    drop(lease);

    let resumable = Fixture::current();
    let transaction = resumable
        .corpus
        .join(".surgeist-generator/transactions/layout/active-read-only-corpus");
    fs::create_dir_all(&transaction).expect("create resumable transaction");
    fs::set_permissions(&transaction, fs::Permissions::from_mode(0o700))
        .expect("private transaction mode");
    resumable.assert_preserved(Some(GeneratorErrorKind::Verification));

    let malformed = Fixture::current();
    write_file(
        &malformed
            .corpus
            .join(".surgeist-generator/acquisition.lock"),
        b"malformed coordination\n",
    );
    malformed.assert_preserved(Some(GeneratorErrorKind::Verification));
}
