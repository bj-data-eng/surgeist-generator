use std::cmp::Ordering;
use std::ffi::OsStr;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Component, Path, PathBuf};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{GeneratorError, GeneratorErrorKind, Result};

/// A normalized, UTF-8 path relative to a declared root.
#[derive(Clone, Debug, Eq)]
pub struct RelativePath(String);

impl RelativePath {
    /// Validates a forward-slash relative path.
    pub fn new(value: impl AsRef<str>) -> Result<Self> {
        let value = value.as_ref();
        if value.is_empty()
            || value.trim() != value
            || value.contains('\0')
            || value.contains('\\')
            || value.starts_with('/')
            || value.ends_with('/')
        {
            return Err(invalid_path("validate relative path", value));
        }
        if value.as_bytes().get(1) == Some(&b':') && value.as_bytes()[0].is_ascii_alphabetic() {
            return Err(invalid_path("validate relative path", value));
        }

        for segment in value.split('/') {
            if segment.is_empty() || segment == "." || segment == ".." {
                return Err(invalid_path("validate relative path", value));
            }
        }

        let path = Path::new(value);
        if path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
        {
            return Err(invalid_path("validate relative path", value));
        }

        Ok(Self(value.to_owned()))
    }

    /// Validates a path and its expected extension (without a leading dot).
    pub fn with_extension(value: impl AsRef<str>, expected: &str) -> Result<Self> {
        let path = Self::new(value)?;
        if expected.is_empty()
            || expected.contains('.')
            || Path::new(path.as_str()).extension() != Some(OsStr::new(expected))
        {
            return Err(invalid_path(
                "validate relative path extension",
                format!("{} does not have extension {expected}", path.as_str()),
            ));
        }
        Ok(path)
    }

    /// Returns the normalized forward-slash representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Joins this path to its declared root.
    #[must_use]
    pub fn join(&self, root: impl AsRef<Path>) -> PathBuf {
        self.0
            .split('/')
            .fold(root.as_ref().to_path_buf(), |mut path, segment| {
                path.push(segment);
                path
            })
    }

    /// Resolves an existing path and proves that symlinks do not escape `root`.
    pub fn resolve_existing(&self, root: impl AsRef<Path>) -> Result<PathBuf> {
        let root = canonical_directory(root.as_ref(), "canonicalize declared root")?;
        let candidate = self.join(&root);
        let resolved = fs::canonicalize(&candidate).map_err(|error| {
            GeneratorError::with_source(
                GeneratorErrorKind::Io,
                "canonicalize existing relative path",
                candidate.display().to_string(),
                error,
            )
        })?;
        require_contained(&root, &resolved, "resolve existing relative path")?;
        Ok(resolved)
    }

    /// Resolves the nearest existing output ancestor and proves containment.
    pub fn resolve_output(&self, root: impl AsRef<Path>) -> Result<PathBuf> {
        let root = canonical_directory(root.as_ref(), "canonicalize output root")?;
        let candidate = self.join(&root);
        let mut ancestor = candidate.as_path();
        while !ancestor.exists() {
            ancestor = ancestor
                .parent()
                .ok_or_else(|| invalid_path("resolve output path", candidate.display()))?;
        }
        let resolved_ancestor = fs::canonicalize(ancestor).map_err(|error| {
            GeneratorError::with_source(
                GeneratorErrorKind::Io,
                "canonicalize output ancestor",
                ancestor.display().to_string(),
                error,
            )
        })?;
        require_contained(&root, &resolved_ancestor, "resolve output path")?;
        Ok(candidate)
    }

    pub(crate) fn from_path(path: &Path) -> Result<Self> {
        let value = path
            .to_str()
            .ok_or_else(|| invalid_path("normalize filesystem path", path.display()))?;
        Self::new(value.replace(std::path::MAIN_SEPARATOR, "/"))
    }
}

impl PartialEq for RelativePath {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl PartialOrd for RelativePath {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for RelativePath {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl Hash for RelativePath {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl Serialize for RelativePath {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for RelativePath {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        Self::new(value).map_err(serde::de::Error::custom)
    }
}

/// Canonical owner and corpus roots for one invocation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CorpusLocation {
    owner_root: PathBuf,
    corpus_root: PathBuf,
}

impl CorpusLocation {
    pub fn new(owner_root: impl AsRef<Path>, corpus_root: impl AsRef<Path>) -> Result<Self> {
        reject_non_utf8(owner_root.as_ref(), "owner root")?;
        reject_non_utf8(corpus_root.as_ref(), "corpus root")?;
        let owner_root = canonical_directory(owner_root.as_ref(), "canonicalize owner root")?;
        let corpus_root = canonical_directory(corpus_root.as_ref(), "canonicalize corpus root")?;
        require_contained(&owner_root, &corpus_root, "validate corpus root")?;
        Ok(Self {
            owner_root,
            corpus_root,
        })
    }

    #[must_use]
    pub fn owner_root(&self) -> &Path {
        &self.owner_root
    }

    #[must_use]
    pub fn corpus_root(&self) -> &Path {
        &self.corpus_root
    }
}

/// Scope and mutation authority for a generation run.
#[derive(Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum RunScope {
    Full,
    Filtered(RelativePath),
}

impl RunScope {
    #[must_use]
    pub const fn may_write_report(&self) -> bool {
        matches!(self, Self::Full)
    }

    #[must_use]
    pub const fn may_remove_stale(&self) -> bool {
        matches!(self, Self::Full)
    }

    #[must_use]
    pub fn includes(&self, source: &RelativePath) -> bool {
        match self {
            Self::Full => true,
            Self::Filtered(filter) => {
                source == filter
                    || source
                        .as_str()
                        .strip_prefix(filter.as_str())
                        .is_some_and(|suffix| suffix.starts_with('/'))
            }
        }
    }

    /// Ensures a filtered scope names at least one available source.
    pub fn require_match(&self, sources: &[RelativePath]) -> Result<()> {
        if matches!(self, Self::Filtered(_)) && !sources.iter().any(|source| self.includes(source))
        {
            return Err(GeneratorError::new(
                GeneratorErrorKind::InvalidInventory,
                "validate run filter",
                "filter matches no source",
            ));
        }
        Ok(())
    }
}

/// Recursively inventories regular files with `extension`, without following symlinks.
pub fn collect_regular_files(root: impl AsRef<Path>, extension: &str) -> Result<Vec<RelativePath>> {
    if extension.is_empty() || extension.contains('.') {
        return Err(invalid_path("validate inventory extension", extension));
    }
    let root = canonical_directory(root.as_ref(), "canonicalize inventory root")?;
    let mut files = Vec::new();
    collect_directory(&root, &root, extension, &mut files)?;
    files.sort();
    if files.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidInventory,
            "collect regular files",
            "duplicate normalized path",
        ));
    }
    Ok(files)
}

fn collect_directory(
    root: &Path,
    directory: &Path,
    extension: &str,
    files: &mut Vec<RelativePath>,
) -> Result<()> {
    let entries = fs::read_dir(directory).map_err(|error| {
        GeneratorError::with_source(
            GeneratorErrorKind::Io,
            "read inventory directory",
            directory.display().to_string(),
            error,
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            GeneratorError::with_source(
                GeneratorErrorKind::Io,
                "read inventory entry",
                directory.display().to_string(),
                error,
            )
        })?;
        if entry.file_name().to_str().is_none() {
            return Err(invalid_path(
                "collect regular files",
                entry.path().display(),
            ));
        }
        let file_type = entry.file_type().map_err(|error| {
            GeneratorError::with_source(
                GeneratorErrorKind::Io,
                "inspect inventory entry",
                entry.path().display().to_string(),
                error,
            )
        })?;
        if file_type.is_symlink() {
            return Err(GeneratorError::new(
                GeneratorErrorKind::InvalidInventory,
                "collect regular files",
                format!("symlink is not allowed: {}", entry.path().display()),
            ));
        }
        if file_type.is_dir() {
            collect_directory(root, &entry.path(), extension, files)?;
        } else if file_type.is_file() {
            let entry_path = entry.path();
            if entry_path.extension() == Some(OsStr::new(extension)) {
                let relative = entry_path
                    .strip_prefix(root)
                    .map_err(|_| invalid_path("collect regular files", entry_path.display()))?;
                files.push(RelativePath::from_path(relative)?);
            }
        } else {
            return Err(GeneratorError::new(
                GeneratorErrorKind::InvalidInventory,
                "collect regular files",
                format!("special entry is not allowed: {}", entry.path().display()),
            ));
        }
    }
    Ok(())
}

fn reject_non_utf8(path: &Path, label: &str) -> Result<()> {
    if path.to_str().is_none() {
        return Err(invalid_path(
            "validate corpus location",
            format!("non-UTF-8 {label}"),
        ));
    }
    Ok(())
}

fn canonical_directory(path: &Path, operation: &str) -> Result<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            invalid_path(operation, format!("missing path: {}", path.display()))
        } else {
            GeneratorError::with_source(
                GeneratorErrorKind::Io,
                operation,
                path.display().to_string(),
                error,
            )
        }
    })?;
    if !canonical.is_dir() {
        return Err(invalid_path(
            operation,
            format!("not a directory: {}", path.display()),
        ));
    }
    Ok(canonical)
}

fn require_contained(root: &Path, candidate: &Path, operation: &str) -> Result<()> {
    if !candidate.starts_with(root) {
        return Err(invalid_path(
            operation,
            format!("{} escapes {}", candidate.display(), root.display()),
        ));
    }
    Ok(())
}

fn invalid_path(operation: impl Into<String>, detail: impl std::fmt::Display) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidPath,
        operation,
        detail.to_string(),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::{CorpusLocation, RelativePath, RunScope, collect_regular_files};
    use crate::GeneratorErrorKind;

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

        #[cfg(unix)]
        fn new_short() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos();
            let path = PathBuf::from(format!("/tmp/sgg-{}-{nonce}", std::process::id()));
            fs::create_dir(&path).expect("create short test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove test directory");
        }
    }

    #[test]
    fn strict_paths_reject_invalid_components() {
        for invalid in [
            "",
            " fixture.json",
            "fixture.json ",
            "/fixture.json",
            "C:/fixture.json",
            "fixtures\\fixture.json",
            "fixtures//fixture.json",
            "fixtures/./fixture.json",
            "fixtures/../fixture.json",
        ] {
            assert!(RelativePath::new(invalid).is_err(), "accepted {invalid:?}");
        }
        assert!(RelativePath::with_extension("fixture.toml", "json").is_err());
    }

    #[test]
    fn corpus_locations_reject_roots_outside_owner() {
        let owner = TestDirectory::new("owner");
        let outside = TestDirectory::new("outside");
        let error = CorpusLocation::new(owner.path(), outside.path())
            .expect_err("outside corpus must fail");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);
    }

    #[test]
    fn collection_is_sorted_and_filtered_scope_is_iteration_only() {
        let root = TestDirectory::new("inventory");
        fs::create_dir(root.path().join("nested")).expect("nested directory");
        fs::write(root.path().join("z.json"), b"{}\n").expect("z fixture");
        fs::write(root.path().join("a.json"), b"{}\n").expect("a fixture");
        fs::write(root.path().join("nested/b.json"), b"{}\n").expect("b fixture");
        fs::write(root.path().join("ignored.txt"), b"ignored\n").expect("ignored file");

        let files = collect_regular_files(root.path(), "json").expect("inventory");
        let names: Vec<_> = files.iter().map(RelativePath::as_str).collect();
        assert_eq!(names, ["a.json", "nested/b.json", "z.json"]);

        let scope = RunScope::Filtered(RelativePath::new("nested").expect("filter"));
        assert!(!scope.may_write_report());
        assert!(!scope.may_remove_stale());
        scope.require_match(&files).expect("matching prefix");
        assert!(scope.includes(&files[1]));
        assert!(!scope.includes(&files[0]));
    }

    #[cfg(unix)]
    #[test]
    fn strict_paths_and_collection_reject_symlink_escapes() {
        use std::os::unix::fs::symlink;

        let root = TestDirectory::new("symlink-root");
        let outside = TestDirectory::new("symlink-outside");
        fs::write(outside.path().join("case.json"), b"{}\n").expect("outside fixture");
        symlink(outside.path(), root.path().join("escape")).expect("directory symlink");

        let relative = RelativePath::new("escape/case.json").expect("strict lexical path");
        assert!(relative.resolve_existing(root.path()).is_err());
        let output = RelativePath::new("escape/new.json").expect("strict output path");
        assert!(output.resolve_output(root.path()).is_err());
        assert!(collect_regular_files(root.path(), "json").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn corpus_location_rejects_non_utf8_cli_input() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let owner = TestDirectory::new("non-utf-owner");
        let invalid = PathBuf::from(OsString::from_vec(vec![b'b', 0xff]));
        assert!(CorpusLocation::new(owner.path(), invalid).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn collection_rejects_special_entries() {
        use std::os::unix::net::UnixListener;

        let root = TestDirectory::new_short();
        let _socket = UnixListener::bind(root.path().join("case.json")).expect("create socket");
        assert!(collect_regular_files(root.path(), "json").is_err());
    }
}
