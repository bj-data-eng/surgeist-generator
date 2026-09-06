use super::*;
use crate::SourceRevision;
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    root: std::path::PathBuf,
    source: std::path::PathBuf,
    location: CorpusLocation,
    spec: SourceImportSpec,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "surgeist-import-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let source = root.join("source");
        let corpus = root.join("corpus");
        fs::create_dir_all(source.join("fixtures/excluded")).unwrap();
        fs::create_dir_all(corpus.join("html/excluded")).unwrap();
        fs::write(source.join("fixtures/a.html"), b"imported\n").unwrap();
        fs::write(source.join("fixtures/excluded/skip.html"), b"excluded\n").unwrap();
        fs::write(corpus.join("html/excluded/authored.html"), b"authored\n").unwrap();
        for args in [
            vec!["init", "--quiet"],
            vec!["config", "user.name", "Import Test"],
            vec!["config", "user.email", "test@example.invalid"],
            vec![
                "remote",
                "add",
                "origin",
                "https://example.invalid/source.git",
            ],
            vec!["add", "."],
            vec!["commit", "--quiet", "-m", "fixture"],
        ] {
            git(&source, &args);
        }
        let revision = git(&source, &["rev-parse", "HEAD"]);
        let pin = PinnedSource::new(
            "upstream",
            "https://example.invalid/source.git",
            SourceRevision::new(revision.trim()).unwrap(),
            RelativePath::new("fixtures").unwrap(),
        )
        .unwrap();
        let spec = SourceImportSpec::new(
            pin,
            RelativePath::new("html").unwrap(),
            RelativePath::new(".source.json").unwrap(),
            "html".into(),
            2,
            vec![RelativePath::new("excluded").unwrap()],
            vec![RelativePath::new("excluded/authored.html").unwrap()],
        )
        .unwrap();
        let location = CorpusLocation::new(&corpus, &corpus).unwrap();
        Self {
            root,
            source,
            location,
            spec,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn git(path: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .current_dir(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}
#[test]
fn import_attests_exact_selection_and_checks_offline_after_source_removal() {
    let fixture = Fixture::new();
    let attestation = import_source(&fixture.location, &fixture.spec, &fixture.source).unwrap();
    assert_eq!(attestation.files().len(), 1);
    assert_eq!(attestation.files()[0].path().as_str(), "a.html");
    assert_eq!(
        fs::read(
            fixture
                .location
                .corpus_root()
                .join("html/excluded/authored.html")
        )
        .unwrap(),
        b"authored\n"
    );
    assert!(
        !fixture
            .location
            .corpus_root()
            .join("html/excluded/skip.html")
            .exists()
    );
    assert_eq!(
        verify_source_import(&fixture.location, &fixture.spec, &fixture.source).unwrap(),
        attestation
    );
    fs::remove_dir_all(&fixture.source).unwrap();
    assert_eq!(
        verify_import(&fixture.location, &fixture.spec).unwrap(),
        attestation
    );
}
#[test]
fn offline_import_check_rejects_changed_bytes_and_policy() {
    let fixture = Fixture::new();
    import_source(&fixture.location, &fixture.spec, &fixture.source).unwrap();
    fs::write(
        fixture.location.corpus_root().join("html/a.html"),
        b"changed",
    )
    .unwrap();
    assert!(verify_import(&fixture.location, &fixture.spec).is_err());
    fs::write(
        fixture.location.corpus_root().join("html/a.html"),
        b"imported\n",
    )
    .unwrap();
    let mut changed = fixture.spec.clone();
    changed.expected_source_files = 3;
    assert!(verify_import(&fixture.location, &changed).is_err());
}
#[test]
fn import_rejects_unknown_inventory_without_modifying_authored_files() {
    let fixture = Fixture::new();
    fs::write(
        fixture.location.corpus_root().join("html/unknown.html"),
        b"unknown",
    )
    .unwrap();
    assert!(import_source(&fixture.location, &fixture.spec, &fixture.source).is_err());
    assert_eq!(
        fs::read(
            fixture
                .location
                .corpus_root()
                .join("html/excluded/authored.html")
        )
        .unwrap(),
        b"authored\n"
    );
    assert!(
        !fixture
            .location
            .corpus_root()
            .join("html/.source.json")
            .exists()
    );
}

#[test]
fn explicit_source_check_rejects_modified_excluded_source() {
    let fixture = Fixture::new();
    import_source(&fixture.location, &fixture.spec, &fixture.source).unwrap();
    fs::write(
        fixture.source.join("fixtures/excluded/skip.html"),
        b"modified excluded source",
    )
    .unwrap();
    assert!(verify_source_import(&fixture.location, &fixture.spec, &fixture.source).is_err());
    verify_import(&fixture.location, &fixture.spec).unwrap();
}

#[test]
fn repeated_import_preserves_authored_changes_and_repairs_imported_bytes() {
    let fixture = Fixture::new();
    let original = import_source(&fixture.location, &fixture.spec, &fixture.source).unwrap();
    fs::write(
        fixture.location.corpus_root().join("html/a.html"),
        b"stale imported bytes",
    )
    .unwrap();
    fs::write(
        fixture
            .location
            .corpus_root()
            .join("html/excluded/authored.html"),
        b"updated authored bytes",
    )
    .unwrap();
    assert_eq!(
        import_source(&fixture.location, &fixture.spec, &fixture.source)
            .unwrap()
            .canonical_bytes()
            .unwrap(),
        original.canonical_bytes().unwrap()
    );
    assert_eq!(
        fs::read(fixture.location.corpus_root().join("html/a.html")).unwrap(),
        b"imported\n"
    );
    assert_eq!(
        fs::read(
            fixture
                .location
                .corpus_root()
                .join("html/excluded/authored.html")
        )
        .unwrap(),
        b"updated authored bytes"
    );
}

#[test]
fn source_spec_rejects_ancestor_collisions_before_io() {
    let fixture = Fixture::new();
    assert!(
        SourceImportSpec::new(
            fixture.spec.pin.clone(),
            RelativePath::new("html").unwrap(),
            RelativePath::new("owned.html").unwrap(),
            "html".into(),
            2,
            vec![],
            vec![RelativePath::new("owned.html/fixture.html").unwrap()]
        )
        .is_err()
    );
}

#[test]
fn import_witness_binds_exact_receipt_imported_and_authored_bytes() {
    let fixture = Fixture::new();
    import_source(&fixture.location, &fixture.spec, &fixture.source).unwrap();
    let checked = verify_import(&fixture.location, &fixture.spec).unwrap();
    let inputs = checked.verified_inputs();
    assert_eq!(inputs.len(), 3);
    for path in [
        "html/.source.json",
        "html/a.html",
        "html/excluded/authored.html",
    ] {
        let bytes = fs::read(fixture.location.corpus_root().join(path)).unwrap();
        assert_eq!(
            inputs.get(&RelativePath::new(path).unwrap()),
            Some(&Sha256Digest::from_bytes(bytes))
        );
    }
    fs::write(
        fixture
            .location
            .corpus_root()
            .join("html/excluded/authored.html"),
        b"later authored edit",
    )
    .unwrap();
    let later = verify_import(&fixture.location, &fixture.spec).unwrap();
    assert_ne!(later.verified_inputs(), checked.verified_inputs());
    assert_eq!(
        later.canonical_bytes().unwrap(),
        checked.canonical_bytes().unwrap()
    );
}

#[test]
fn import_rejects_a_stale_caller_input_before_source_verification() {
    let fixture = Fixture::new();
    fs::write(
        fixture.location.corpus_root().join("corpus.toml"),
        b"new pin",
    )
    .unwrap();
    let expected = BTreeMap::from([(
        RelativePath::new("corpus.toml").unwrap(),
        Sha256Digest::from_bytes(b"old pin"),
    )]);
    fs::remove_dir_all(&fixture.source).unwrap();
    let error = import_source_with_expected_inputs(
        &fixture.location,
        &fixture.spec,
        &fixture.source,
        &expected,
    )
    .unwrap_err();
    assert!(error.to_string().contains("input witness"), "{error}");
    assert_unpublished(&fixture);
}

#[test]
fn import_revalidates_caller_inputs_under_lease_and_before_intent() {
    for (before_lease, reimport) in [(true, false), (false, false), (true, true), (false, true)] {
        let fixture = Fixture::new();
        let previous = reimport
            .then(|| import_source(&fixture.location, &fixture.spec, &fixture.source).unwrap());
        let manifest = fixture.location.corpus_root().join("corpus.toml");
        fs::write(&manifest, b"original pin and policy").unwrap();
        let expected = BTreeMap::from([(
            RelativePath::new("corpus.toml").unwrap(),
            Sha256Digest::from_bytes(b"original pin and policy"),
        )]);
        let error = import_source_impl(
            &fixture.location,
            &fixture.spec,
            &fixture.source,
            &expected,
            || {
                if before_lease {
                    fs::write(&manifest, b"changed pin and policy").unwrap();
                }
            },
            || {
                if !before_lease {
                    fs::write(&manifest, b"changed pin and policy").unwrap();
                }
            },
        )
        .unwrap_err();
        assert!(error.to_string().contains("input witness"), "{error}");
        if let Some(previous) = previous {
            assert_eq!(
                verify_import(&fixture.location, &fixture.spec).unwrap(),
                previous
            );
        } else {
            assert_unpublished(&fixture);
        }
    }
}

#[test]
fn import_accepts_current_input_witness_but_rejects_replaced_identity() {
    let fixture = Fixture::new();
    let manifest = fixture.location.corpus_root().join("corpus.toml");
    let bytes = b"current pin and policy";
    fs::write(&manifest, bytes).unwrap();
    let expected = BTreeMap::from([(
        RelativePath::new("corpus.toml").unwrap(),
        Sha256Digest::from_bytes(bytes),
    )]);
    assert!(
        import_source_impl(
            &fixture.location,
            &fixture.spec,
            &fixture.source,
            &expected,
            || {
                fs::remove_file(&manifest).unwrap();
                fs::write(&manifest, bytes).unwrap();
            },
            || {},
        )
        .is_err()
    );
    assert_unpublished(&fixture);
    let receipt = import_source_with_expected_inputs(
        &fixture.location,
        &fixture.spec,
        &fixture.source,
        &expected,
    )
    .unwrap();
    assert_eq!(
        receipt,
        verify_import(&fixture.location, &fixture.spec).unwrap()
    );
}

#[test]
fn import_input_witness_must_be_outside_the_published_partition() {
    let fixture = Fixture::new();
    let expected = BTreeMap::from([(
        RelativePath::new("html/excluded/authored.html").unwrap(),
        Sha256Digest::from_bytes(b"authored\n"),
    )]);
    let error = import_source_with_expected_inputs(
        &fixture.location,
        &fixture.spec,
        &fixture.source,
        &expected,
    )
    .unwrap_err();
    assert!(
        error.to_string().contains("overlaps its destination"),
        "{error}"
    );
    assert_unpublished(&fixture);
}

#[test]
fn import_provenance_rejects_a_projection_not_bound_to_the_receipt() {
    let fixture = Fixture::new();
    let receipt = import_source(&fixture.location, &fixture.spec, &fixture.source).unwrap();
    let provenance = receipt.provenance().unwrap();
    assert_eq!(provenance.sidecar().as_str(), "html/.source.json");
    assert_eq!(provenance.receipt_sha256(), &receipt.digest().unwrap());
    assert_eq!(provenance.specification(), &fixture.spec);
    assert_eq!(provenance.files(), receipt.files());
    let value = serde_json::to_value(&provenance).unwrap();
    assert_eq!(
        serde_json::from_value::<SourceImportProvenance>(value.clone()).unwrap(),
        provenance
    );
    for (field, replacement) in [
        ("sidecar", serde_json::json!("elsewhere/receipt.json")),
        (
            "receipt_sha256",
            serde_json::to_value(Sha256Digest::from_bytes(b"other receipt")).unwrap(),
        ),
        ("schema_version", serde_json::json!(2)),
    ] {
        let mut changed = value.clone();
        changed[field] = replacement;
        assert!(
            serde_json::from_value::<SourceImportProvenance>(changed).is_err(),
            "{field}"
        );
    }
    let mut changed = value;
    changed["specification"]["expected_source_files"] = serde_json::json!(3);
    assert!(serde_json::from_value::<SourceImportProvenance>(changed).is_err());
}

fn assert_unpublished(fixture: &Fixture) {
    let corpus = fixture.location.corpus_root();
    assert!(!corpus.join("html/a.html").exists());
    assert!(!corpus.join("html/.source.json").exists());
    assert_eq!(
        fs::read(corpus.join("html/excluded/authored.html")).unwrap(),
        b"authored\n"
    );
}
