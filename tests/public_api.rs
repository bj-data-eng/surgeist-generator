use std::collections::BTreeMap;
use std::fmt::{Debug, Display};
use std::hash::Hash;

use serde::{Serialize, de::DeserializeOwned};
use surgeist_generator::{
    ArtifactProvenance, CaseDisposition, CaseDispositionRecord, CorpusLocation, GenerationCounts,
    GenerationReport, GeneratorError, GeneratorErrorKind, ManifestVersion, PinnedSource,
    RelativePath, ReportArtifact, RunScope, Sha256Digest, SourceRevision, VerifiedSource,
    collect_regular_files, parse_manifest, validate_disposition_records, verify_git_source,
};

#[cfg(feature = "css-corpus")]
use surgeist_generator::css::{CssCommand, CssRequest};

const REVISION: &str = "0123456789abcdef0123456789abcdef01234567";
const ZERO_DIGEST: &str = "0000000000000000000000000000000000000000000000000000000000000000";
const ONE_DIGEST: &str = "1111111111111111111111111111111111111111111111111111111111111111";

fn assert_clone_debug_eq<T: Clone + Debug + Eq + PartialEq>() {}
fn assert_copy_debug_eq<T: Clone + Copy + Debug + Eq + PartialEq>() {}
fn assert_ordered_hash_serde<T>()
where
    T: Clone
        + Debug
        + Display
        + Eq
        + Hash
        + Ord
        + PartialEq
        + PartialOrd
        + Serialize
        + DeserializeOwned,
{
}
fn assert_path_traits<T>()
where
    T: Clone + Debug + Eq + Hash + Ord + PartialEq + PartialOrd + Serialize + DeserializeOwned,
{
}
fn assert_serde<T: Serialize + DeserializeOwned>() {}
fn assert_error<T: Debug + Display + std::error::Error>() {}

#[test]
fn shared_public_surface_has_the_exact_constructible_contracts() {
    assert_copy_debug_eq::<GeneratorErrorKind>();
    let _ = GeneratorErrorKind::UnsupportedPlatform;
    assert_error::<GeneratorError>();
    assert_path_traits::<RelativePath>();
    assert_clone_debug_eq::<CorpusLocation>();
    assert_clone_debug_eq::<RunScope>();
    assert_serde::<ManifestVersion>();
    assert_ordered_hash_serde::<SourceRevision>();
    assert_clone_debug_eq::<PinnedSource>();
    assert_serde::<PinnedSource>();
    assert_clone_debug_eq::<VerifiedSource>();
    assert_copy_debug_eq::<CaseDisposition>();
    assert_serde::<CaseDisposition>();
    assert_clone_debug_eq::<CaseDispositionRecord>();
    assert_serde::<CaseDispositionRecord>();
    assert_ordered_hash_serde::<Sha256Digest>();
    assert_clone_debug_eq::<ArtifactProvenance>();
    assert_serde::<ArtifactProvenance>();
    assert_clone_debug_eq::<ReportArtifact>();
    assert_serde::<ReportArtifact>();
    assert_copy_debug_eq::<GenerationCounts>();
    assert_serde::<GenerationCounts>();
    assert_clone_debug_eq::<GenerationReport>();
    assert_serde::<GenerationReport>();

    let _: fn(&str, &str) -> surgeist_generator::Result<RelativePath> =
        |value, extension| RelativePath::with_extension(value, extension);
    let _: fn(&str, &str) -> surgeist_generator::Result<CorpusLocation> =
        |owner, corpus| CorpusLocation::new(owner, corpus);
    let _: fn(&str, &str) -> surgeist_generator::Result<serde_json::Value> =
        |text, path| parse_manifest(text, path);
    let _: fn(&str, &PinnedSource) -> surgeist_generator::Result<VerifiedSource> =
        |checkout, pin| verify_git_source(checkout, pin);
    let _: fn(&str, &str) -> surgeist_generator::Result<Vec<RelativePath>> =
        |root, extension| collect_regular_files(root, extension);
    let _: fn(
        Vec<CaseDispositionRecord>,
    ) -> surgeist_generator::Result<Vec<CaseDispositionRecord>> = validate_disposition_records;
}

#[test]
fn shared_serde_emits_the_exact_compact_canonical_goldens() {
    let source_path = RelativePath::new("fixtures/case.json").expect("source path");
    let output_path = RelativePath::new("expectations/case.json").expect("output path");
    let revision = SourceRevision::new(REVISION).expect("revision");
    let digest_zero = Sha256Digest::from_text(ZERO_DIGEST).expect("zero digest");
    let digest_one = Sha256Digest::from_text(ONE_DIGEST).expect("one digest");
    let version = ManifestVersion::new(1).expect("manifest version");
    let pin = PinnedSource::new(
        "csstree",
        "https://example.invalid/csstree.git",
        revision.clone(),
        RelativePath::new("fixtures/ast").expect("source subdirectory"),
    )
    .expect("source pin");

    assert_eq!(
        serde_json::to_string(&source_path).unwrap(),
        "\"fixtures/case.json\""
    );
    assert_eq!(serde_json::to_string(&version).unwrap(), "1");
    assert_eq!(
        serde_json::to_string(&revision).unwrap(),
        format!("\"{REVISION}\"")
    );
    assert_eq!(
        serde_json::to_string(&pin).unwrap(),
        format!(
            "{{\"label\":\"csstree\",\"repository_url\":\"https://example.invalid/csstree.git\",\"revision\":\"{REVISION}\",\"source_subdirectory\":\"fixtures/ast\"}}"
        )
    );
    assert_eq!(
        serde_json::to_string(&CaseDisposition::Active).unwrap(),
        "\"active\""
    );
    assert_eq!(
        serde_json::to_string(&CaseDisposition::ExpectedFail).unwrap(),
        "\"expected-fail\""
    );

    let active = CaseDispositionRecord::new(
        "fixtures/case.json#/ordinary",
        source_path.clone(),
        CaseDisposition::Active,
        None::<String>,
    )
    .expect("active disposition");
    let expected = CaseDispositionRecord::new(
        "fixtures/case.json#/error/0",
        source_path.clone(),
        CaseDisposition::ExpectedFail,
        Some("known mismatch"),
    )
    .expect("expected-fail disposition");
    assert_eq!(
        serde_json::to_string(&active).unwrap(),
        "{\"case_id\":\"fixtures/case.json#/ordinary\",\"source_path\":\"fixtures/case.json\",\"disposition\":\"active\"}"
    );
    assert_eq!(
        serde_json::to_string(&expected).unwrap(),
        "{\"case_id\":\"fixtures/case.json#/error/0\",\"source_path\":\"fixtures/case.json\",\"disposition\":\"expected-fail\",\"reason\":\"known mismatch\"}"
    );

    let mut domain = BTreeMap::new();
    domain.insert("z-last".to_owned(), digest_zero.clone());
    domain.insert("a-first".to_owned(), digest_one.clone());
    let provenance = ArtifactProvenance::new(
        source_path,
        digest_zero.clone(),
        "surgeist-css-generate",
        version,
        domain,
    )
    .expect("artifact provenance");
    assert_eq!(
        serde_json::to_string(&provenance).unwrap(),
        format!(
            "{{\"source_path\":\"fixtures/case.json\",\"source_digest\":\"{ZERO_DIGEST}\",\"generator\":\"surgeist-css-generate\",\"schema_version\":1,\"domain_provenance\":{{\"a-first\":\"{ONE_DIGEST}\",\"z-last\":\"{ZERO_DIGEST}\"}}}}"
        )
    );

    let artifact =
        ReportArtifact::new(provenance, output_path, digest_one, 1).expect("report artifact");
    let counts = GenerationCounts::new(1, 1, 0, 0, 0).expect("generation counts");
    let report = GenerationReport::new(
        digest_zero,
        "https://example.invalid/csstree.git",
        revision,
        counts,
        vec![artifact],
    )
    .expect("structurally valid shared report");
    let json = serde_json::to_string(&report).expect("canonical report JSON");
    assert_eq!(
        json,
        format!(
            "{{\"manifest_digest\":\"{ZERO_DIGEST}\",\"source_repository\":\"https://example.invalid/csstree.git\",\"source_revision\":\"{REVISION}\",\"counts\":{{\"active\":1,\"expected_fail\":1,\"unsupported\":0,\"quarantined\":0,\"failed_to_generate\":0}},\"artifacts\":[{{\"provenance\":{{\"source_path\":\"fixtures/case.json\",\"source_digest\":\"{ZERO_DIGEST}\",\"generator\":\"surgeist-css-generate\",\"schema_version\":1,\"domain_provenance\":{{\"a-first\":\"{ONE_DIGEST}\",\"z-last\":\"{ZERO_DIGEST}\"}}}},\"output_path\":\"expectations/case.json\",\"output_digest\":\"{ONE_DIGEST}\",\"case_count\":1}}]}}"
        )
    );
}

#[test]
fn shared_deserialization_rechecks_every_constructor_invariant() {
    for invalid in ["null", "1", "\"/absolute\"", "\"a/../b\"", "\"a\\\\b\""] {
        assert!(
            serde_json::from_str::<RelativePath>(invalid).is_err(),
            "accepted {invalid}"
        );
    }
    for invalid in ["0", "-1", "1.0", "1e0", "\"1\""] {
        assert!(
            serde_json::from_str::<ManifestVersion>(invalid).is_err(),
            "accepted {invalid}"
        );
    }
    for invalid in ["\"ACTIVE\"", "\"expected_fail\"", "\"failed-to-generate\""] {
        assert!(
            serde_json::from_str::<CaseDisposition>(invalid).is_err(),
            "accepted {invalid}"
        );
    }

    let active_null = "{\"case_id\":\"a.json\",\"source_path\":\"a.json\",\"disposition\":\"active\",\"reason\":null}";
    let active: CaseDispositionRecord = serde_json::from_str(active_null).expect("active null");
    assert_eq!(active.reason(), None);
    let missing_reason =
        "{\"case_id\":\"a.json\",\"source_path\":\"a.json\",\"disposition\":\"unsupported\"}";
    assert!(serde_json::from_str::<CaseDispositionRecord>(missing_reason).is_err());
    let repeated = "{\"case_id\":\"a.json\",\"case_id\":\"b.json\",\"source_path\":\"a.json\",\"disposition\":\"active\"}";
    assert!(serde_json::from_str::<CaseDispositionRecord>(repeated).is_err());
    let unknown = "{\"case_id\":\"a.json\",\"source_path\":\"a.json\",\"disposition\":\"active\",\"extra\":true}";
    assert!(serde_json::from_str::<CaseDispositionRecord>(unknown).is_err());

    let duplicate_domain_key = format!(
        "{{\"source_path\":\"a.json\",\"source_digest\":\"{ZERO_DIGEST}\",\"generator\":\"surgeist-test\",\"schema_version\":1,\"domain_provenance\":{{\"same\":\"{ZERO_DIGEST}\",\"same\":\"{ONE_DIGEST}\"}}}}"
    );
    assert!(serde_json::from_str::<ArtifactProvenance>(&duplicate_domain_key).is_err());

    let count_overflow = format!(
        "{{\"active\":{},\"expected_fail\":1,\"unsupported\":0,\"quarantined\":0,\"failed_to_generate\":0}}",
        u32::MAX
    );
    assert!(serde_json::from_str::<GenerationCounts>(&count_overflow).is_err());
    let scalar_overflow = format!(
        "{{\"active\":{},\"expected_fail\":0,\"unsupported\":0,\"quarantined\":0,\"failed_to_generate\":0}}",
        u64::from(u32::MAX) + 1
    );
    assert!(serde_json::from_str::<GenerationCounts>(&scalar_overflow).is_err());
    let fraction = "{\"active\":1.0,\"expected_fail\":0,\"unsupported\":0,\"quarantined\":0,\"failed_to_generate\":0}";
    assert!(serde_json::from_str::<GenerationCounts>(fraction).is_err());
}

#[test]
fn public_constructor_grammars_and_exit_codes_are_exact() {
    for invalid in ["Upper", "-leading", "contains..dots", "", &"a".repeat(65)] {
        let result = PinnedSource::new(
            invalid,
            "https://example.invalid/source.git",
            SourceRevision::new(REVISION).unwrap(),
            RelativePath::new("fixtures").unwrap(),
        );
        assert_eq!(
            result.unwrap_err().kind(),
            GeneratorErrorKind::SourceVerification
        );
    }
    for invalid in [
        "http://example.invalid/source.git",
        "https://localhost/source.git",
        "https://Example.invalid/source.git",
        "https://user@example.invalid/source.git",
        "https://example.invalid:443/source.git",
        "https://example.invalid/a/../source.git",
        "https://example.invalid/source",
    ] {
        assert!(
            PinnedSource::new(
                "source",
                invalid,
                SourceRevision::new(REVISION).unwrap(),
                RelativePath::new("fixtures").unwrap(),
            )
            .is_err(),
            "accepted {invalid}"
        );
    }

    for invalid in [
        "a.json#not-a-pointer",
        "a.json#/bad~2escape",
        "a.json#/control\u{7f}",
        " a.json",
        "a.json ",
    ] {
        assert!(
            CaseDispositionRecord::new(
                invalid,
                RelativePath::new("a.json").unwrap(),
                CaseDisposition::Active,
                None::<String>,
            )
            .is_err(),
            "accepted {invalid:?}"
        );
    }
    assert!(
        CaseDispositionRecord::new(
            "a.json#/ok~0tilde~1slash",
            RelativePath::new("a.json").unwrap(),
            CaseDisposition::ExpectedFail,
            Some("line\nbreak"),
        )
        .is_err()
    );

    assert_eq!(
        RelativePath::new("../escape").unwrap_err().kind(),
        GeneratorErrorKind::InvalidPath
    );
    assert_eq!(
        SourceRevision::new("f").unwrap_err().kind(),
        GeneratorErrorKind::SourceVerification
    );
    assert_eq!(
        Sha256Digest::from_text("f").unwrap_err().kind(),
        GeneratorErrorKind::Verification
    );
    assert_eq!(
        ManifestVersion::new(0).unwrap_err().kind(),
        GeneratorErrorKind::InvalidManifest
    );
    let report_error = GenerationReport::new(
        Sha256Digest::from_text(ZERO_DIGEST).unwrap(),
        "http://example.invalid/source.git",
        SourceRevision::new(REVISION).unwrap(),
        GenerationCounts::new(0, 0, 0, 0, 0).unwrap(),
        Vec::new(),
    )
    .unwrap_err();
    assert_eq!(report_error.kind(), GeneratorErrorKind::SourceVerification);

    let cli = parse_manifest::<ManifestVersion>("not = [valid", "manifest.toml").unwrap_err();
    assert_eq!(cli.exit_code(), 1);
    assert_eq!(GeneratorErrorKind::Cli as u8, GeneratorErrorKind::Cli as u8);
}

#[test]
fn portable_count_bounds_and_structural_report_counts_are_enforced() {
    assert!(GenerationCounts::new(u32::MAX as usize, 0, 0, 0, 0).is_ok());
    assert!(GenerationCounts::new(u32::MAX as usize, 1, 0, 0, 0).is_err());
    assert!(GenerationCounts::new((u32::MAX as usize) + 1, 0, 0, 0, 0).is_err());

    let provenance = ArtifactProvenance::new(
        RelativePath::new("a.json").unwrap(),
        Sha256Digest::from_text(ZERO_DIGEST).unwrap(),
        "surgeist-test",
        ManifestVersion::new(1).unwrap(),
        BTreeMap::new(),
    )
    .unwrap();
    assert!(
        ReportArtifact::new(
            provenance.clone(),
            RelativePath::new("out.json").unwrap(),
            Sha256Digest::from_text(ONE_DIGEST).unwrap(),
            0,
        )
        .is_err()
    );
    assert!(
        ReportArtifact::new(
            provenance.clone(),
            RelativePath::new("out.json").unwrap(),
            Sha256Digest::from_text(ONE_DIGEST).unwrap(),
            (u32::MAX as usize) + 1,
        )
        .is_err()
    );

    let artifact = ReportArtifact::new(
        provenance,
        RelativePath::new("out.json").unwrap(),
        Sha256Digest::from_text(ONE_DIGEST).unwrap(),
        1,
    )
    .unwrap();
    let counts = GenerationCounts::new(1, 0, 1, 0, 1).unwrap();
    GenerationReport::new(
        Sha256Digest::from_text(ZERO_DIGEST).unwrap(),
        "https://example.invalid/source.git",
        SourceRevision::new(REVISION).unwrap(),
        counts,
        vec![artifact],
    )
    .expect("shared reports do not invent artifacts for unsupported/failed cases");
}

#[cfg(feature = "css-corpus")]
#[test]
fn css_public_request_matrix_is_io_free_and_accessors_are_exact() {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TestRoot(PathBuf);

    impl Drop for TestRoot {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove public API roots");
        }
    }

    static NEXT: AtomicU64 = AtomicU64::new(0);
    let root = TestRoot(std::env::temp_dir().join(format!(
        "surgeist-generator-css-public-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )));
    let owner = root.0.join("owner");
    let corpus = owner.join("corpus");
    fs::create_dir_all(&corpus).expect("create public API roots");
    let location = CorpusLocation::new(&owner, &corpus).expect("location");
    fs::rename(&owner, root.0.join("detached-owner"))
        .expect("detach roots before request construction");
    let source = PathBuf::from("a-source-that-need-not-exist");
    let filter = RelativePath::new("declaration").expect("filter");

    assert_copy_debug_eq::<CssCommand>();
    assert_clone_debug_eq::<CssRequest>();
    let _: fn(CssRequest) -> surgeist_generator::Result<()> = surgeist_generator::css::run;
    let _: fn() -> surgeist_generator::Result<()> = surgeist_generator::css::run_from_env;
    let request = CssRequest::new(
        location.clone(),
        CssCommand::ImportCsstree,
        Some(source.clone()),
        None,
    )
    .expect("I/O-free import request");
    assert_eq!(request.location(), &location);
    assert_eq!(request.command(), CssCommand::ImportCsstree);
    assert_eq!(request.source_root(), Some(source.as_path()));
    assert_eq!(request.filter(), None);

    let generate = CssRequest::new(location.clone(), CssCommand::Generate, None, None)
        .expect("I/O-free unfiltered generation request");
    assert_eq!(generate.location(), &location);
    assert_eq!(generate.command(), CssCommand::Generate);
    assert_eq!(generate.source_root(), None);
    assert_eq!(generate.filter(), None);

    let filtered = CssRequest::new(
        location.clone(),
        CssCommand::Generate,
        None,
        Some(filter.clone()),
    )
    .expect("I/O-free filtered generation request");
    assert_eq!(filtered.location(), &location);
    assert_eq!(filtered.command(), CssCommand::Generate);
    assert_eq!(filtered.source_root(), None);
    assert_eq!(filtered.filter(), Some(&filter));

    let check = CssRequest::new(location.clone(), CssCommand::CheckCorpus, None, None)
        .expect("I/O-free corpus check request");
    assert_eq!(check.location(), &location);
    assert_eq!(check.command(), CssCommand::CheckCorpus);
    assert_eq!(check.source_root(), None);
    assert_eq!(check.filter(), None);

    for (source_root, filter) in [
        (None, None),
        (None, Some(filter.clone())),
        (Some(PathBuf::new()), None),
        (Some(PathBuf::new()), Some(filter.clone())),
        (Some(source.clone()), Some(filter.clone())),
    ] {
        let error = CssRequest::new(
            location.clone(),
            CssCommand::ImportCsstree,
            source_root,
            filter,
        )
        .expect_err("invalid import payload");
        assert_eq!(error.kind(), GeneratorErrorKind::Cli);
    }

    for (source_root, filter) in [
        (Some(source.clone()), None),
        (Some(source.clone()), Some(filter.clone())),
        (
            None,
            Some(RelativePath::new("generation-reports/all.json").expect("reserved filter")),
        ),
    ] {
        let error = CssRequest::new(location.clone(), CssCommand::Generate, source_root, filter)
            .expect_err("invalid generation payload");
        assert_eq!(error.kind(), GeneratorErrorKind::Cli);
    }

    for (source_root, filter) in [
        (Some(source.clone()), None),
        (None, Some(filter.clone())),
        (Some(source), Some(filter)),
    ] {
        let error = CssRequest::new(
            location.clone(),
            CssCommand::CheckCorpus,
            source_root,
            filter,
        )
        .expect_err("invalid corpus check payload");
        assert_eq!(error.kind(), GeneratorErrorKind::Cli);
    }
}
