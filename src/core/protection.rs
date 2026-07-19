use std::path::{Path, PathBuf};

use crate::{CorpusLocation, GeneratorError, GeneratorErrorKind, Result};

use super::fs::{BoundPath, RootedFs};
use super::source::ProtectedSource;

#[derive(Clone, Debug, Eq, PartialEq)]
struct NamespaceSpec {
    label: String,
    path: PathBuf,
}

impl NamespaceSpec {
    fn new(label: &str, path: &Path) -> Result<Self> {
        if label.is_empty()
            || label
                .chars()
                .any(|character| matches!(character, '\n' | '\r' | '\0'))
        {
            return Err(invalid_path(
                "validate namespace label",
                "namespace label is empty or contains a control character",
            ));
        }
        Ok(Self {
            label: label.to_owned(),
            path: path.to_path_buf(),
        })
    }

    fn bind(&self) -> Result<NamespaceBinding> {
        BoundPath::bind(&self.path).map(|path| NamespaceBinding {
            label: self.label.clone(),
            path,
        })
    }
}

#[derive(Debug)]
struct NamespaceBinding {
    label: String,
    path: BoundPath,
}

/// Complete preflight proof for a mutation's writable and protected namespaces.
#[derive(Debug)]
pub(crate) struct NamespaceDisjointness {
    corpus_root: PathBuf,
    corpus_binding: BoundPath,
    writable: Vec<NamespaceSpec>,
    protected: Vec<NamespaceBinding>,
}

/// A namespace proof that cannot omit its verified source authorities.
#[derive(Debug)]
pub(crate) struct ProtectedSourceDisjointness<'source> {
    source: &'source ProtectedSource,
    namespaces: NamespaceDisjointness,
}

impl<'source> ProtectedSourceDisjointness<'source> {
    pub(crate) fn for_mutation(
        location: &CorpusLocation,
        writable: &[(&str, &Path)],
        protected: &[(&str, &Path)],
        source: &'source ProtectedSource,
    ) -> Result<Self> {
        let complete_protected = protected
            .iter()
            .map(|(label, path)| ((*label).to_owned(), (*path).to_path_buf()))
            .chain(
                source
                    .protection_namespaces()
                    .map(|(label, path)| (label.to_owned(), path.to_path_buf())),
            )
            .collect::<Vec<_>>();
        let complete_refs = complete_protected
            .iter()
            .map(|(label, path)| (label.as_str(), path.as_path()))
            .collect::<Vec<_>>();
        NamespaceDisjointness::for_mutation(location, writable, &complete_refs)
            .map(|namespaces| Self { source, namespaces })
    }

    pub(crate) fn revalidate(&self, rooted: &RootedFs) -> Result<()> {
        self.source.closing_revalidate()?;
        self.namespaces.revalidate(rooted)
    }
}

impl NamespaceDisjointness {
    /// Binds the complete matrix without creating an alias, probe, stage, or
    /// coordination entry. The coordination root is always a writable member.
    pub(crate) fn for_mutation(
        location: &CorpusLocation,
        writable: &[(&str, &Path)],
        protected: &[(&str, &Path)],
    ) -> Result<Self> {
        let corpus_binding = BoundPath::bind(location.corpus_root())?;
        corpus_binding.require_existing_directory("bind corpus namespace authority")?;

        let mut writable_specs = Vec::with_capacity(writable.len() + 1);
        writable_specs.push(NamespaceSpec::new(
            "generator coordination root",
            &location.coordination_root(),
        )?);
        writable_specs.extend(
            writable
                .iter()
                .map(|(label, path)| NamespaceSpec::new(label, path))
                .collect::<Result<Vec<_>>>()?,
        );
        let writable_bindings = writable_specs
            .iter()
            .map(NamespaceSpec::bind)
            .collect::<Result<Vec<_>>>()?;
        let protected_bindings = protected
            .iter()
            .map(|(label, path)| NamespaceSpec::new(label, path)?.bind())
            .collect::<Result<Vec<_>>>()?;
        prove_disjoint(&writable_bindings, &protected_bindings)?;

        Ok(Self {
            corpus_root: location.corpus_root().to_path_buf(),
            corpus_binding,
            writable: writable_specs,
            protected: protected_bindings,
        })
    }

    /// Reopens protected paths and repeats the complete matrix under the exact
    /// rooted corpus authority held by exclusive acquisition.
    pub(crate) fn revalidate(&self, rooted: &RootedFs) -> Result<()> {
        rooted.revalidate_root().map_err(|error| {
            GeneratorError::with_source(
                GeneratorErrorKind::InvalidPath,
                "revalidate namespace corpus authority",
                self.corpus_root.display().to_string(),
                error,
            )
        })?;
        if rooted.canonical_root() != self.corpus_root
            || !rooted
                .identity()
                .matches_recovery(self.corpus_binding.existing_identity())
        {
            return Err(invalid_path(
                "revalidate namespace corpus authority",
                "held mutation root differs from the preflight corpus authority",
            ));
        }
        self.corpus_binding.revalidate()?;
        for protected in &self.protected {
            protected.path.revalidate().map_err(|error| {
                GeneratorError::with_source(
                    GeneratorErrorKind::InvalidPath,
                    "revalidate protected namespace",
                    protected.label.clone(),
                    error,
                )
            })?;
        }
        let writable = self
            .writable
            .iter()
            .map(NamespaceSpec::bind)
            .collect::<Result<Vec<_>>>()?;
        prove_disjoint(&writable, &self.protected)
    }
}

fn prove_disjoint(writable: &[NamespaceBinding], protected: &[NamespaceBinding]) -> Result<()> {
    for writable_namespace in writable {
        for protected_namespace in protected {
            reject_overlap(writable_namespace, protected_namespace)?;
        }
    }
    for (index, writable_namespace) in writable.iter().enumerate() {
        for other_writable in &writable[index + 1..] {
            reject_overlap(writable_namespace, other_writable)?;
        }
    }
    Ok(())
}

fn reject_overlap(left: &NamespaceBinding, right: &NamespaceBinding) -> Result<()> {
    if left.path.overlaps(&right.path)? {
        return Err(invalid_path(
            "prove namespace disjointness",
            format!(
                "{} ({}) overlaps {} ({})",
                left.label,
                left.path.canonical_path().display(),
                right.label,
                right.path.canonical_path().display()
            ),
        ));
    }
    Ok(())
}

fn invalid_path(operation: &str, detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(GeneratorErrorKind::InvalidPath, operation, detail)
}

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
mod tests {
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use crate::{CorpusLocation, GeneratorErrorKind};

    use super::NamespaceDisjointness;

    static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "surgeist-generator-protection-{label}-{}-{sequence:016x}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create protection test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove protection test directory");
        }
    }

    #[test]
    fn namespace_disjointness_rejects_alias_ancestor_and_missing_suffix() {
        let temporary = TestDirectory::new("matrix");
        let corpus = temporary.path().join("corpus");
        let external = temporary.path().join("external");
        let protected = external.join("protected");
        fs::create_dir(&corpus).expect("create corpus");
        fs::create_dir_all(&protected).expect("create protected root");
        let location = CorpusLocation::new(temporary.path(), &corpus).expect("corpus location");

        let assert_invalid = |writable: &Path, protected_path: &Path, label: &str| {
            let error = NamespaceDisjointness::for_mutation(
                &location,
                &[("writable output", writable)],
                &[("protected input", protected_path)],
            )
            .expect_err(label);
            assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath, "{label}");
        };

        assert_invalid(&protected, &protected, "equal namespaces must fail");
        assert_invalid(
            &protected.join("missing/child"),
            &protected,
            "protected ancestor must fail",
        );
        assert_invalid(&external, &protected, "writable ancestor must fail");

        let alias = external.join("protected-alias");
        symlink(&protected, &alias).expect("create protected alias");
        assert_invalid(
            &alias.join("missing/child"),
            &protected,
            "descriptor alias ancestor must fail",
        );

        assert_invalid(
            &external.join("future/Output"),
            &external.join("future/output/child"),
            "case-ambiguous absent suffix must fail closed",
        );
        assert_invalid(
            &external.join("future/shared"),
            &external.join("future/shared/child"),
            "absent suffix ancestor must fail",
        );

        NamespaceDisjointness::for_mutation(
            &location,
            &[("writable output", external.join("future/import").as_path())],
            &[(
                "protected input",
                external.join("future/expectation").as_path(),
            )],
        )
        .expect("provably disjoint absent roots");
        assert!(
            !corpus.join(".surgeist-generator").exists(),
            "namespace preflight wrote coordination state"
        );
    }
}
