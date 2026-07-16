use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{GeneratorError, GeneratorErrorKind, RelativePath, Result};

/// Exact lowercase hexadecimal Git object revision.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceRevision(String);

impl SourceRevision {
    pub fn new(value: impl AsRef<str>) -> Result<Self> {
        let value = value.as_ref();
        if !matches!(value.len(), 40 | 64)
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(GeneratorError::new(
                GeneratorErrorKind::InvalidManifest,
                "validate source revision",
                "revision must be a full 40- or 64-character lowercase hexadecimal object ID",
            ));
        }
        Ok(Self(value.to_owned()))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceRevision {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for SourceRevision {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for SourceRevision {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Manifest-declared identity and subdirectory of an external source checkout.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct PinnedSource {
    label: String,
    repository_url: String,
    revision: SourceRevision,
    source_subdirectory: RelativePath,
}

impl PinnedSource {
    pub fn new(
        label: impl Into<String>,
        repository_url: impl Into<String>,
        revision: SourceRevision,
        source_subdirectory: RelativePath,
    ) -> Result<Self> {
        let label = label.into();
        let repository_url = repository_url.into();
        if label.is_empty() || label.trim() != label || label.contains('\0') {
            return Err(invalid_source("source label must be nonempty and trimmed"));
        }
        if repository_url.is_empty()
            || repository_url.trim() != repository_url
            || repository_url.contains('\0')
        {
            return Err(invalid_source(
                "source repository URL must be nonempty and trimmed",
            ));
        }
        Ok(Self {
            label,
            repository_url,
            revision,
            source_subdirectory,
        })
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn repository_url(&self) -> &str {
        &self.repository_url
    }

    #[must_use]
    pub const fn revision(&self) -> &SourceRevision {
        &self.revision
    }

    #[must_use]
    pub const fn source_subdirectory(&self) -> &RelativePath {
        &self.source_subdirectory
    }
}

impl<'de> Deserialize<'de> for PinnedSource {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawPinnedSource {
            label: String,
            repository_url: String,
            revision: SourceRevision,
            source_subdirectory: RelativePath,
        }

        let raw = RawPinnedSource::deserialize(deserializer)?;
        Self::new(
            raw.label,
            raw.repository_url,
            raw.revision,
            raw.source_subdirectory,
        )
        .map_err(serde::de::Error::custom)
    }
}

/// A source checkout proven to match a complete clean pin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedSource {
    canonical_root: PathBuf,
    canonical_source_root: PathBuf,
    revision: SourceRevision,
}

impl VerifiedSource {
    #[must_use]
    pub fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }

    #[must_use]
    pub fn canonical_source_root(&self) -> &Path {
        &self.canonical_source_root
    }

    #[must_use]
    pub const fn revision(&self) -> &SourceRevision {
        &self.revision
    }
}

/// Verifies an existing Git checkout without fetching or mutating Git state.
pub fn verify_git_source(checkout: impl AsRef<Path>, pin: &PinnedSource) -> Result<VerifiedSource> {
    let checkout = checkout.as_ref();
    let inside = git(checkout, &["rev-parse", "--is-inside-work-tree"])?;
    require_stdout(&inside, "true", "checkout is not a Git worktree")?;

    let root_output = git(checkout, &["rev-parse", "--show-toplevel"])?;
    let root_text = stdout_line(&root_output, "resolve Git worktree root")?;
    let canonical_root = fs::canonicalize(root_text).map_err(|error| {
        GeneratorError::with_source(
            GeneratorErrorKind::Io,
            "canonicalize Git worktree root",
            root_text.to_owned(),
            error,
        )
    })?;

    let head = git(&canonical_root, &["rev-parse", "HEAD"])?;
    require_stdout(
        &head,
        pin.revision.as_str(),
        format!(
            "HEAD does not equal pinned revision {}",
            pin.revision.as_str()
        ),
    )?;

    let status = git(
        &canonical_root,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    if !status.stdout.is_empty() {
        return Err(invalid_source(format!(
            "source checkout is dirty: {}",
            String::from_utf8_lossy(&status.stdout).trim_end()
        )));
    }

    let origin = git(&canonical_root, &["remote", "get-url", "origin"])?;
    require_stdout(
        &origin,
        pin.repository_url(),
        format!("origin does not equal {}", pin.repository_url()),
    )?;

    let source_candidate = pin.source_subdirectory.join(&canonical_root);
    let canonical_source_root = fs::canonicalize(&source_candidate).map_err(|error| {
        GeneratorError::with_source(
            GeneratorErrorKind::SourceVerification,
            "canonicalize pinned source subdirectory",
            source_candidate.display().to_string(),
            error,
        )
    })?;
    if !canonical_source_root.is_dir() || !canonical_source_root.starts_with(&canonical_root) {
        return Err(invalid_source(format!(
            "source subdirectory escapes checkout: {}",
            pin.source_subdirectory.as_str()
        )));
    }

    Ok(VerifiedSource {
        canonical_root,
        canonical_source_root,
        revision: pin.revision.clone(),
    })
}

fn git(directory: &Path, arguments: &[&str]) -> Result<Output> {
    let output = Command::new("git")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .arg("-C")
        .arg(directory)
        .args(arguments)
        .output()
        .map_err(|error| {
            GeneratorError::with_source(
                GeneratorErrorKind::Process,
                "run installed git",
                format!("git -C {} {}", directory.display(), arguments.join(" ")),
                error,
            )
        })?;
    if !output.status.success() {
        return Err(GeneratorError::new(
            GeneratorErrorKind::SourceVerification,
            "verify Git source",
            format!(
                "git -C {} {} exited with {}; stdout={:?}; stderr={:?}",
                directory.display(),
                arguments.join(" "),
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        ));
    }
    Ok(output)
}

fn stdout_line<'a>(output: &'a Output, operation: &str) -> Result<&'a str> {
    std::str::from_utf8(&output.stdout)
        .map(|value| {
            let value = value.strip_suffix('\n').unwrap_or(value);
            value.strip_suffix('\r').unwrap_or(value)
        })
        .map_err(|_| invalid_source(format!("{operation} returned non-UTF-8 output")))
}

fn require_stdout(output: &Output, expected: &str, detail: impl Into<String>) -> Result<()> {
    if stdout_line(output, "read Git output")? != expected {
        return Err(invalid_source(detail));
    }
    Ok(())
}

fn invalid_source(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::SourceVerification,
        "verify pinned source",
        detail,
    )
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{PinnedSource, SourceRevision, verify_git_source};
    use crate::{GeneratorErrorKind, RelativePath};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "surgeist-generator-{label}-{}-{nonce}",
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

    fn run_git(directory: &Path, arguments: &[&OsStr]) -> String {
        let output = Command::new("git")
            .arg("-C")
            .arg(directory)
            .args(arguments)
            .output()
            .expect("run installed git");
        assert!(
            output.status.success(),
            "git failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("UTF-8 Git output")
            .trim_end()
            .to_owned()
    }

    fn repository() -> (TestDirectory, String, SourceRevision) {
        let directory = TestDirectory::new("git-source");
        run_git(
            directory.path(),
            &[OsStr::new("init"), OsStr::new("--quiet")],
        );
        run_git(
            directory.path(),
            &[
                OsStr::new("config"),
                OsStr::new("user.name"),
                OsStr::new("Test"),
            ],
        );
        run_git(
            directory.path(),
            &[
                OsStr::new("config"),
                OsStr::new("user.email"),
                OsStr::new("test@example.invalid"),
            ],
        );
        let origin = "https://example.invalid/source.git".to_owned();
        run_git(
            directory.path(),
            &[
                OsStr::new("remote"),
                OsStr::new("add"),
                OsStr::new("origin"),
                OsStr::new(&origin),
            ],
        );
        fs::create_dir(directory.path().join("fixtures")).expect("create fixture directory");
        fs::write(directory.path().join("fixtures/case.json"), b"{}\n").expect("write fixture");
        run_git(
            directory.path(),
            &[OsStr::new("add"), OsStr::new("fixtures/case.json")],
        );
        run_git(
            directory.path(),
            &[
                OsStr::new("commit"),
                OsStr::new("--quiet"),
                OsStr::new("-m"),
                OsStr::new("fixture"),
            ],
        );
        let revision = SourceRevision::new(run_git(
            directory.path(),
            &[OsStr::new("rev-parse"), OsStr::new("HEAD")],
        ))
        .expect("full revision");
        (directory, origin, revision)
    }

    fn pin(origin: &str, revision: SourceRevision, source: &str) -> PinnedSource {
        PinnedSource::new(
            "fixtures",
            origin,
            revision,
            RelativePath::new(source).expect("strict path"),
        )
        .expect("valid pin")
    }

    #[test]
    fn source_verification_does_not_refresh_git_index() {
        let (directory, origin, revision) = repository();
        let object = run_git(
            directory.path(),
            &[
                OsStr::new("rev-parse"),
                OsStr::new("HEAD:fixtures/case.json"),
            ],
        );
        let cache_entry = format!("100644,{object},fixtures/case.json");
        run_git(
            directory.path(),
            &[
                OsStr::new("update-index"),
                OsStr::new("--cacheinfo"),
                OsStr::new(&cache_entry),
            ],
        );
        let index_text = run_git(
            directory.path(),
            &[
                OsStr::new("rev-parse"),
                OsStr::new("--git-path"),
                OsStr::new("index"),
            ],
        );
        let index_path = Path::new(&index_text);
        let index_path = if index_path.is_absolute() {
            index_path.to_path_buf()
        } else {
            directory.path().join(index_path)
        };
        let before = fs::read(&index_path).expect("read deliberately stale Git index");

        verify_git_source(directory.path(), &pin(&origin, revision, "fixtures"))
            .expect("verify clean source");

        let after = fs::read(index_path).expect("read Git index after verification");
        assert_eq!(after, before, "source verification refreshed the Git index");
    }

    #[test]
    fn pinned_source_requires_exact_clean_revision() {
        let (directory, origin, revision) = repository();
        assert!(SourceRevision::new(&revision.as_str()[..12]).is_err());
        let verified = verify_git_source(
            directory.path(),
            &pin(&origin, revision.clone(), "fixtures"),
        )
        .expect("exact clean source");
        assert_eq!(verified.revision(), &revision);

        let prefix = SourceRevision::new(format!("{}0", &revision.as_str()[..39]))
            .expect("full but incorrect revision");
        assert_eq!(
            verify_git_source(directory.path(), &pin(&origin, prefix, "fixtures"))
                .expect_err("wrong revision")
                .kind(),
            GeneratorErrorKind::SourceVerification
        );

        fs::write(directory.path().join("untracked.json"), b"{}\n").expect("dirty checkout");
        assert!(verify_git_source(directory.path(), &pin(&origin, revision, "fixtures")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn pinned_source_rejects_wrong_origin_and_escaped_source_root() {
        use std::os::unix::fs::symlink;

        let (directory, origin, revision) = repository();
        assert!(
            verify_git_source(
                directory.path(),
                &pin(
                    "https://example.invalid/wrong.git",
                    revision.clone(),
                    "fixtures"
                )
            )
            .is_err()
        );

        symlink(std::env::temp_dir(), directory.path().join("escape")).expect("create symlink");
        run_git(directory.path(), &[OsStr::new("add"), OsStr::new("escape")]);
        run_git(
            directory.path(),
            &[
                OsStr::new("commit"),
                OsStr::new("--quiet"),
                OsStr::new("-m"),
                OsStr::new("escape"),
            ],
        );
        let escaped_revision = SourceRevision::new(run_git(
            directory.path(),
            &[OsStr::new("rev-parse"), OsStr::new("HEAD")],
        ))
        .expect("full revision");
        assert!(
            verify_git_source(directory.path(), &pin(&origin, escaped_revision, "escape")).is_err()
        );
    }
}
