use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use surgeist_generator::{
    CaseDisposition, CaseDispositionRecord, CorpusLocation, GeneratorErrorKind, RelativePath,
    RunScope, collect_regular_files, validate_disposition_records,
};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new(label: &str) -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "surgeist-generator-t02-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).expect("create test directory");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove test directory");
    }
}

#[test]
fn disposition_validation_accepts_repeated_sources_and_sorts_only_by_case_id() {
    let source = RelativePath::new("fixture.json").unwrap();
    let later = CaseDispositionRecord::new(
        "fixture.json#/z",
        source.clone(),
        CaseDisposition::Unsupported,
        Some("unsupported shape"),
    )
    .unwrap();
    let earlier = CaseDispositionRecord::new(
        "fixture.json#/a",
        source,
        CaseDisposition::Active,
        None::<String>,
    )
    .unwrap();
    let records = validate_disposition_records(vec![later, earlier]).unwrap();
    assert_eq!(
        records
            .iter()
            .map(CaseDispositionRecord::case_id)
            .collect::<Vec<_>>(),
        ["fixture.json#/a", "fixture.json#/z"]
    );
}

#[test]
fn disposition_records_preserve_source_independent_case_id_compatibility() {
    let cases = [
        ("other.json#/x", "source.json"),
        ("non-json.fixture#/case", "metadata.toml"),
        ("generic/case-id", "inputs/data.bin"),
        ("legacy/case.txt#", "source/input.css"),
    ];

    for (case_id, source_path) in cases {
        let record = CaseDispositionRecord::new(
            case_id,
            RelativePath::new(source_path).expect("strict source path"),
            CaseDisposition::Active,
            None::<String>,
        )
        .unwrap_or_else(|error| {
            panic!("baseline-compatible case ID {case_id:?} was rejected: {error}")
        });
        assert_eq!(record.case_id(), case_id);
        assert_eq!(record.source_path().as_str(), source_path);

        let expected = format!(
            "{{\"case_id\":\"{case_id}\",\"source_path\":\"{source_path}\",\"disposition\":\"active\"}}"
        );
        assert_eq!(
            serde_json::to_string(&record).expect("serialize disposition record"),
            expected
        );
        let decoded: CaseDispositionRecord =
            serde_json::from_str(&expected).expect("deserialize baseline-compatible record");
        assert_eq!(decoded, record);
    }

    let source = RelativePath::new("source.json").expect("strict source path");
    for invalid in [
        "other.json#not-a-pointer",
        "other.json#/bad~2escape",
        "nested//case#/x",
        "/absolute#/x",
        " other.json#/x",
    ] {
        assert!(
            CaseDispositionRecord::new(
                invalid,
                source.clone(),
                CaseDisposition::Active,
                None::<String>,
            )
            .is_err(),
            "accepted intrinsically malformed case ID {invalid:?}"
        );
        let encoded = format!(
            "{{\"case_id\":\"{invalid}\",\"source_path\":\"source.json\",\"disposition\":\"active\"}}"
        );
        assert!(
            serde_json::from_str::<CaseDispositionRecord>(&encoded).is_err(),
            "deserialized intrinsically malformed case ID {invalid:?}"
        );
    }
}

#[test]
fn disposition_record_constructor_rejects_multiple_hash_delimiters() {
    let error = CaseDispositionRecord::new(
        "a.json##/x",
        RelativePath::new("source.json").expect("strict source path"),
        CaseDisposition::Active,
        None::<String>,
    )
    .expect_err("generic case IDs have at most one hash delimiter");
    assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
}

#[test]
fn disposition_record_deserializer_rejects_multiple_hash_delimiters() {
    let encoded = r#"{"case_id":"a.json##/x","source_path":"source.json","disposition":"active"}"#;
    let error = serde_json::from_str::<CaseDispositionRecord>(encoded)
        .expect_err("public serde preserves the generic case-ID grammar");
    assert!(
        error.to_string().contains("InvalidInventory"),
        "unexpected serde diagnostic: {error}"
    );
}

#[test]
fn disposition_case_ids_keep_literal_hash_support_outside_the_public_generic_grammar() {
    let source = RelativePath::new("nested#source/Fixture#.json").unwrap();
    for private_css_id in [
        "nested#source/Fixture#.json#/before#~1middle~1#after",
        "nested#source/Fixture#.json#pointer",
        "nested#source/Fixture#.json#/bad~2escape",
    ] {
        assert!(
            CaseDispositionRecord::new(
                private_css_id,
                source.clone(),
                CaseDisposition::Active,
                None::<String>,
            )
            .is_err(),
            "public generic constructor accepted private CSS ID {private_css_id:?}"
        );
    }
}

#[test]
fn corpus_locations_reject_the_exact_reserved_coordination_component() {
    let owner = TestDirectory::new("reserved-owner");
    let reserved = owner.path().join(".surgeist-generator");
    let corpus = reserved.join("corpus");
    fs::create_dir(&reserved).unwrap();
    fs::create_dir(&corpus).unwrap();
    let error = CorpusLocation::new(owner.path(), &corpus).unwrap_err();
    assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);
}

#[test]
fn collection_rejects_wrong_extensions_and_classifies_contract_failures() {
    let root = TestDirectory::new("collection");
    fs::write(root.path().join("case.json"), b"{}\n").unwrap();
    fs::write(root.path().join("unexpected.txt"), b"text\n").unwrap();
    let error = collect_regular_files(root.path(), "json").unwrap_err();
    assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
    let error = collect_regular_files(root.path(), ".json").unwrap_err();
    assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
}

#[cfg(unix)]
#[test]
fn collection_classifies_symlinks_as_path_failures() {
    use std::os::unix::fs::symlink;

    let root = TestDirectory::new("collection-symlink");
    fs::write(root.path().join("case.json"), b"{}\n").unwrap();
    symlink("case.json", root.path().join("alias.json")).unwrap();
    let error = collect_regular_files(root.path(), "json").unwrap_err();
    assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);
}

#[cfg(unix)]
#[test]
fn output_resolution_rejects_a_dangling_symlink_ancestor() {
    use std::os::unix::fs::symlink;

    let root = TestDirectory::new("dangling-output");
    symlink("missing-target", root.path().join("dangling")).unwrap();
    let output = RelativePath::new("dangling/generated.json").unwrap();
    let error = output
        .resolve_output(root.path())
        .expect_err("a dangling symlink must not be treated as an absent output component");
    assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);
}

#[test]
fn filtered_scope_never_authorizes_report_or_stale_output_mutation() {
    let filter = RelativePath::new("nested").unwrap();
    let scope = RunScope::Filtered(filter);
    assert!(!scope.may_write_report());
    assert!(!scope.may_remove_stale());
    let error = scope
        .require_match(&[RelativePath::new("other/case.json").unwrap()])
        .unwrap_err();
    assert_eq!(error.kind(), GeneratorErrorKind::InvalidInventory);
}
