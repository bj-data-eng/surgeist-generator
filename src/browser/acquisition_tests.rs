use super::*;
use crate::{RelativePath, SourceRevision};
use std::fs;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Fixture {
    root: PathBuf,
    owner: PathBuf,
    source: PathBuf,
    pin: PinnedSource,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "surgeist-cache-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let owner = root.join("owner");
        let source = root.join("source");
        fs::create_dir_all(owner.join("src")).unwrap();
        fs::create_dir_all(source.join("fixtures")).unwrap();
        fs::write(
            owner.join("Cargo.toml"),
            "[package]\nname=\"cache-owner-test\"\nversion=\"0.1.0\"\nedition=\"2024\"\n",
        )
        .unwrap();
        fs::write(owner.join("src/lib.rs"), "pub fn value() -> u32 { 1 }\n").unwrap();
        fs::write(owner.join(".gitignore"), "tmp/\ntarget/\n").unwrap();
        git(&owner, &["init", "--quiet"]);
        fs::write(source.join("fixtures/a.html"), "<div>a</div>\n").unwrap();
        git(&source, &["init", "--quiet"]);
        git(&source, &["config", "user.name", "Cache Fixture"]);
        git(
            &source,
            &["config", "user.email", "fixture@example.invalid"],
        );
        git(
            &source,
            &[
                "remote",
                "add",
                "origin",
                "https://example.invalid/source.git",
            ],
        );
        git(&source, &["add", "."]);
        git(&source, &["commit", "--quiet", "-m", "fixture"]);
        let revision = git(&source, &["rev-parse", "HEAD"]);
        let pin = PinnedSource::new(
            "source",
            "https://example.invalid/source.git",
            SourceRevision::new(revision.trim()).unwrap(),
            RelativePath::new("fixtures").unwrap(),
        )
        .unwrap();
        Self {
            root,
            owner,
            source,
            pin,
        }
    }
    fn fetch(&self, stage: &Path, pin: &PinnedSource) -> Result<()> {
        let output = Command::new("git")
            .args(["clone", "--quiet", "--no-hardlinks"])
            .arg(&self.source)
            .arg(stage)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        git(
            stage,
            &["remote", "set-url", "origin", pin.repository_url()],
        );
        Ok(())
    }
    fn cache(&self) -> PathBuf {
        self.owner
            .join("tmp/surgeist-sources/source")
            .join(self.pin.revision().as_str())
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}
fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(path)
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn managed_source_acquisition_survives_cargo_clean_and_reuses_without_fetch() {
    let fixture = Fixture::new();
    let acquired = acquire_source_with(
        &fixture.owner,
        &fixture.pin,
        AcquisitionMode::Managed,
        |stage, pin| fixture.fetch(stage, pin),
    )
    .unwrap();
    assert_eq!(
        acquired.canonical_root(),
        fixture.cache().canonicalize().unwrap()
    );
    let browser_pin = browser_settings();
    let browser_executable = browser_cache::acquire_browser_with(
        &fixture.owner,
        &browser_pin,
        AcquisitionMode::Managed,
        fetched_browser,
    )
    .unwrap();
    let output = Command::new("cargo")
        .args(["build", "--offline"])
        .env("CARGO_TARGET_DIR", fixture.owner.join("target"))
        .current_dir(&fixture.owner)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let output = Command::new("cargo")
        .args(["clean", "--offline"])
        .env("CARGO_TARGET_DIR", fixture.owner.join("target"))
        .current_dir(&fixture.owner)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        fs::read(fixture.cache().join("fixtures/a.html")).unwrap(),
        b"<div>a</div>\n"
    );
    acquire_source_with(
        &fixture.owner,
        &fixture.pin,
        AcquisitionMode::Managed,
        |_, _| panic!("complete cache must not fetch again"),
    )
    .unwrap();
    acquire_source_with(
        &fixture.owner,
        &fixture.pin,
        AcquisitionMode::ExistingOnly,
        |_, _| panic!("existing-only must never fetch"),
    )
    .unwrap();
    assert_eq!(
        fs::read(browser_executable.join(&fixture.owner)).unwrap(),
        b"pinned browser bytes"
    );
    browser_cache::acquire_browser_with(
        &fixture.owner,
        &browser_pin,
        AcquisitionMode::Managed,
        |_| panic!("cargo clean must not evict the browser cache"),
    )
    .unwrap();
    assert_eq!(
        git(&fixture.owner, &["check-ignore", "tmp/surgeist-browser"]).trim(),
        "tmp/surgeist-browser"
    );
    let ignored = git(
        &fixture.owner,
        &["check-ignore", "tmp/surgeist-sources/source"],
    );
    assert_eq!(ignored.trim(), "tmp/surgeist-sources/source");
}

#[test]
fn interrupted_source_acquisition_does_not_publish_and_managed_retry_recovers() {
    let fixture = Fixture::new();
    let error = acquire_source_with(
        &fixture.owner,
        &fixture.pin,
        AcquisitionMode::Managed,
        |stage, _| {
            fs::create_dir_all(stage).unwrap();
            fs::write(stage.join("partial"), "interrupted").unwrap();
            Err(GeneratorError::new(
                GeneratorErrorKind::Process,
                "fetch fixture",
                "interrupted",
            ))
        },
    )
    .unwrap_err();
    assert_eq!(error.kind(), GeneratorErrorKind::Process);
    assert!(!fixture.cache().exists());
    acquire_source_with(
        &fixture.owner,
        &fixture.pin,
        AcquisitionMode::Managed,
        |stage, pin| fixture.fetch(stage, pin),
    )
    .unwrap();
    assert!(!fixture.cache().join("partial").exists());
}

#[test]
fn existing_only_missing_and_modified_source_cache_never_fetch_or_repair() {
    let fixture = Fixture::new();
    assert!(
        acquire_source_with(
            &fixture.owner,
            &fixture.pin,
            AcquisitionMode::ExistingOnly,
            |_, _| panic!("must not fetch")
        )
        .is_err()
    );
    assert!(!fixture.owner.join("tmp").exists());
    acquire_source_with(
        &fixture.owner,
        &fixture.pin,
        AcquisitionMode::Managed,
        |stage, pin| fixture.fetch(stage, pin),
    )
    .unwrap();
    fs::write(fixture.cache().join("fixtures/a.html"), "modified").unwrap();
    assert!(
        acquire_source_with(
            &fixture.owner,
            &fixture.pin,
            AcquisitionMode::Managed,
            |_, _| panic!("invalid complete cache must not be repaired")
        )
        .is_err()
    );
    assert_eq!(
        fs::read(fixture.cache().join("fixtures/a.html")).unwrap(),
        b"modified"
    );
}

#[cfg(feature = "browser-corpus")]
fn browser_settings() -> super::super::BrowserSettings {
    super::super::BrowserSettings::new(
        "chrome-for-testing".into(),
        "149.0.7827.115".into(),
        "Google Chrome for Testing 149.0.7827.115".into(),
        RelativePath::new("tmp/surgeist-browser").unwrap(),
        "{version}:{repository_relative_executable}".into(),
        super::super::BrowserLaunch::new(1, 1000, 10, vec![]).unwrap(),
    )
    .unwrap()
}
#[cfg(feature = "browser-corpus")]
fn fetched_browser(stage: &Path) -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    fs::create_dir_all(stage.join("app")).unwrap();
    let executable = stage.join("app/browser");
    fs::write(&executable, b"pinned browser bytes").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    Ok(executable)
}
#[cfg(feature = "browser-corpus")]
#[test]
fn browser_acquisition_reuses_complete_cache_and_rejects_changed_executable() {
    let fixture = Fixture::new();
    let settings = browser_settings();
    let executable = browser_cache::acquire_browser_with(
        &fixture.owner,
        &settings,
        AcquisitionMode::Managed,
        fetched_browser,
    )
    .unwrap();
    assert_eq!(
        executable.as_str(),
        "tmp/surgeist-browser/mac_arm-149.0.7827.115/app/browser"
    );
    browser_cache::acquire_browser_with(
        &fixture.owner,
        &settings,
        AcquisitionMode::Managed,
        |_| panic!("must reuse pinned cache"),
    )
    .unwrap();
    browser_cache::acquire_browser_with(
        &fixture.owner,
        &settings,
        AcquisitionMode::ExistingOnly,
        |_| panic!("must not fetch"),
    )
    .unwrap();
    fs::write(executable.join(&fixture.owner), b"tampered").unwrap();
    assert!(
        browser_cache::acquire_browser_with(
            &fixture.owner,
            &settings,
            AcquisitionMode::Managed,
            |_| panic!("must preserve invalid complete cache")
        )
        .is_err()
    );
}
#[cfg(feature = "browser-corpus")]
#[test]
fn browser_acquisition_recovers_interrupted_stage_without_publishing_partial_bytes() {
    let fixture = Fixture::new();
    let settings = browser_settings();
    assert!(
        browser_cache::acquire_browser_with(
            &fixture.owner,
            &settings,
            AcquisitionMode::ExistingOnly,
            |_| panic!("must never fetch")
        )
        .is_err()
    );
    assert!(!fixture.owner.join("tmp").exists());
    assert!(
        browser_cache::acquire_browser_with(
            &fixture.owner,
            &settings,
            AcquisitionMode::Managed,
            |stage| {
                fs::create_dir_all(stage).unwrap();
                fs::write(stage.join("incomplete"), b"partial").unwrap();
                Err(invalid("interrupted download"))
            }
        )
        .is_err()
    );
    assert!(
        !fixture
            .owner
            .join("tmp/surgeist-browser/mac_arm-149.0.7827.115")
            .exists()
    );
    browser_cache::acquire_browser_with(
        &fixture.owner,
        &settings,
        AcquisitionMode::Managed,
        fetched_browser,
    )
    .unwrap();
    assert!(
        !fixture
            .owner
            .join("tmp/surgeist-browser/mac_arm-149.0.7827.115/incomplete")
            .exists()
    );
}

#[test]
fn source_cache_rejects_symlinked_cache_ancestors_and_stage_without_deleting_target() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let outside = fixture.root.join("unrelated");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("keep"), b"keep").unwrap();
    symlink(&outside, fixture.owner.join("tmp")).unwrap();
    assert!(
        acquire_source_with(
            &fixture.owner,
            &fixture.pin,
            AcquisitionMode::Managed,
            |_, _| panic!("must not fetch through alias")
        )
        .is_err()
    );
    assert_eq!(fs::read(outside.join("keep")).unwrap(), b"keep");
}

#[test]
fn source_acquisition_lock_excludes_a_second_writer() {
    let fixture = Fixture::new();
    acquire_source_with(
        &fixture.owner,
        &fixture.pin,
        AcquisitionMode::Managed,
        |stage, pin| {
            let error = acquire_source_with(
                &fixture.owner,
                &fixture.pin,
                AcquisitionMode::Managed,
                |_, _| panic!("second writer must not fetch"),
            )
            .unwrap_err();
            assert_eq!(error.kind(), GeneratorErrorKind::LeaseActive);
            fixture.fetch(stage, pin)
        },
    )
    .unwrap();
}

#[test]
fn interrupted_source_stage_alias_is_rejected_without_deleting_its_target() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let outside = fixture.root.join("stage-target");
    fs::create_dir(&outside).unwrap();
    fs::write(outside.join("keep"), b"keep").unwrap();
    fs::create_dir_all(fixture.cache().parent().unwrap()).unwrap();
    let stage = fixture
        .cache()
        .with_file_name(format!("{}.partial", fixture.pin.revision().as_str()));
    symlink(&outside, &stage).unwrap();
    assert!(
        acquire_source_with(
            &fixture.owner,
            &fixture.pin,
            AcquisitionMode::Managed,
            |_, _| panic!("must not fetch through stage alias")
        )
        .is_err()
    );
    assert_eq!(fs::read(outside.join("keep")).unwrap(), b"keep");
}

#[test]
fn legacy_fetcher_cache_is_reused_without_redownload_or_payload_mutation() {
    use std::os::unix::fs::PermissionsExt;
    let fixture = Fixture::new();
    let settings = browser_settings();
    let relative = "tmp/surgeist-browser/mac_arm-149.0.7827.115/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing";
    let executable = fixture.owner.join(relative);
    fs::create_dir_all(executable.parent().unwrap()).unwrap();
    fs::write(&executable, b"existing legacy payload").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    let selected = browser_cache::acquire_browser_with(
        &fixture.owner,
        &settings,
        AcquisitionMode::Managed,
        |_| panic!("existing fetcher cache must not redownload"),
    )
    .unwrap();
    assert_eq!(selected.as_str(), relative);
    assert_eq!(fs::read(&executable).unwrap(), b"existing legacy payload");
    assert!(
        !fixture
            .owner
            .join("tmp/surgeist-browser/mac_arm-149.0.7827.115/.surgeist-browser.json")
            .exists()
    );
}
