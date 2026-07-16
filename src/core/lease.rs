use std::fs::{self, File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{CorpusLocation, GeneratorError, GeneratorErrorKind, Result, RunScope, Sha256Digest};

/// An exclusive lease for one generator domain and canonical corpus root.
#[derive(Debug)]
pub struct GenerationLease {
    file: File,
}

impl GenerationLease {
    /// Acquires the lease for `domain` and the location's canonical corpus root.
    ///
    /// The supplied location and scope are already-validated lifecycle inputs.
    /// Contention returns [`GeneratorErrorKind::LeaseActive`] with coherent owner
    /// metadata; dropping the returned value releases the advisory lock.
    pub fn acquire(
        location: &CorpusLocation,
        domain: impl AsRef<str>,
        generator: impl AsRef<str>,
        scope: &RunScope,
        command: impl AsRef<str>,
    ) -> Result<Self> {
        let domain = checked_metadata("domain", domain.as_ref())?;
        let generator = checked_metadata("generator", generator.as_ref())?;
        let command = checked_metadata("command", command.as_ref())?;
        let coordination_root = location
            .owner_root()
            .join("target")
            .join("surgeist-generator");
        create_coordination_root(location.owner_root(), &coordination_root)?;

        let key = Sha256Digest::from_bytes(format!(
            "domain={domain}\ncorpus={}",
            location.corpus_root().display()
        ));
        let gate_path = coordination_root.join(format!("{key}.gate.lock"));
        let lease_path = coordination_root.join(format!("{key}.lease.lock"));
        let gate = open_lock_file(&gate_path)?;
        gate.lock().map_err(|source| {
            io_source(
                "lock generation acquisition gate",
                gate_path.display(),
                source,
            )
        })?;

        let mut lease = match open_lock_file(&lease_path) {
            Ok(file) => file,
            Err(error) => {
                let _ = gate.unlock();
                return Err(error);
            }
        };
        match lease.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                let owner = read_owner(&mut lease, &lease_path)?;
                gate.unlock().map_err(|source| {
                    io_source(
                        "unlock generation acquisition gate",
                        gate_path.display(),
                        source,
                    )
                })?;
                return Err(GeneratorError::new(
                    GeneratorErrorKind::LeaseActive,
                    "acquire generation lease",
                    format!("active owner: {owner}"),
                ));
            }
            Err(TryLockError::Error(source)) => {
                let _ = gate.unlock();
                return Err(io_source(
                    "lock generation lease",
                    lease_path.display(),
                    source,
                ));
            }
        }

        let metadata = owner_metadata(location, domain, generator, scope, command)?;
        if let Err(source) = write_owner(&mut lease, metadata.as_bytes()) {
            let _ = lease.unlock();
            let _ = gate.unlock();
            return Err(io_source(
                "write generation lease owner",
                lease_path.display(),
                source,
            ));
        }
        gate.unlock().map_err(|source| {
            let _ = lease.unlock();
            io_source(
                "unlock generation acquisition gate",
                gate_path.display(),
                source,
            )
        })?;
        Ok(Self { file: lease })
    }
}

impl Drop for GenerationLease {
    fn drop(&mut self) {
        let _ = self.file.unlock();
    }
}

fn checked_metadata<'a>(label: &str, value: &'a str) -> Result<&'a str> {
    if value.is_empty()
        || value.trim() != value
        || value.contains('\0')
        || value.contains('\n')
        || value.contains('\r')
    {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "validate generation lease metadata",
            format!("invalid {label}"),
        ));
    }
    Ok(value)
}

fn create_coordination_root(owner_root: &Path, target: &Path) -> Result<()> {
    let relative = target.strip_prefix(owner_root).map_err(|_| {
        GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "validate generation coordination root",
            target.display().to_string(),
        )
    })?;
    let mut current = owner_root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        match fs::create_dir(&current) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                let metadata = fs::symlink_metadata(&current).map_err(|source| {
                    io_source(
                        "inspect generation coordination root",
                        current.display(),
                        source,
                    )
                })?;
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    return Err(GeneratorError::new(
                        GeneratorErrorKind::InvalidPath,
                        "validate generation coordination root",
                        format!("not a real directory: {}", current.display()),
                    ));
                }
            }
            Err(source) => {
                return Err(io_source(
                    "create generation coordination root",
                    current.display(),
                    source,
                ));
            }
        }
    }
    Ok(())
}

fn open_lock_file(path: &Path) -> Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|source| io_source("open generation lock file", path.display(), source))
}

fn read_owner(file: &mut File, path: &Path) -> Result<String> {
    file.seek(SeekFrom::Start(0))
        .and_then(|_| {
            let mut owner = String::new();
            file.read_to_string(&mut owner).map(|_| owner)
        })
        .map_err(|source| io_source("read generation lease owner", path.display(), source))
        .map(|owner| owner.trim_end().to_owned())
}

fn write_owner(file: &mut File, bytes: &[u8]) -> std::io::Result<()> {
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(bytes)?;
    file.flush()?;
    file.sync_all()
}

fn owner_metadata(
    location: &CorpusLocation,
    domain: &str,
    generator: &str,
    scope: &RunScope,
    command: &str,
) -> Result<String> {
    let start = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| {
            GeneratorError::new(
                GeneratorErrorKind::Io,
                "read generation lease start time",
                error.to_string(),
            )
        })?
        .as_secs();
    let scope = match scope {
        RunScope::Full => "full".to_owned(),
        RunScope::Filtered(filter) => format!("filtered:{}", filter.as_str()),
    };
    Ok(format!(
        "generator={generator}\npid={}\ncorpus_root={}\ndomain={domain}\nscope={scope}\ncommand={command}\nunix_start={start}\n",
        std::process::id(),
        escape_metadata_path(location.corpus_root())
    ))
}

fn escape_metadata_path(path: &Path) -> String {
    path.display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn io_source(
    operation: &str,
    detail: impl std::fmt::Display,
    source: std::io::Error,
) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::Io,
        operation,
        detail.to_string(),
        source,
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::GenerationLease;
    use crate::{CorpusLocation, GeneratorErrorKind, RelativePath, RunScope};

    static NEXT_TEMP: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let sequence = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "surgeist-generator-{label}-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated test directory");
        }
    }

    #[test]
    fn generation_lease_contends_by_corpus() {
        let directory = TestDirectory::new("lease-contention");
        let corpus = directory.path().join("corpus");
        fs::create_dir(&corpus).expect("create corpus");
        let location = CorpusLocation::new(directory.path(), &corpus).expect("valid location");

        let held = GenerationLease::acquire(
            &location,
            "layout",
            "surgeist-layout-generate",
            &RunScope::Full,
            "generate",
        )
        .expect("acquire first lease");
        let unrelated_domain = GenerationLease::acquire(
            &location,
            "css",
            "surgeist-css-generate",
            &RunScope::Full,
            "generate",
        )
        .expect("another domain has an unrelated lease key");
        let other_corpus = directory.path().join("other-corpus");
        fs::create_dir(&other_corpus).expect("create second corpus");
        let other_location =
            CorpusLocation::new(directory.path(), &other_corpus).expect("valid second location");
        let unrelated_corpus = GenerationLease::acquire(
            &other_location,
            "layout",
            "surgeist-layout-generate",
            &RunScope::Full,
            "generate",
        )
        .expect("another corpus has an unrelated lease key");
        let contender = GenerationLease::acquire(
            &location,
            "layout",
            "surgeist-layout-generate",
            &RunScope::Filtered(RelativePath::new("one.html").unwrap()),
            "generate-existing",
        )
        .expect_err("full and filtered work on one corpus must contend");
        assert_eq!(contender.kind(), GeneratorErrorKind::LeaseActive);
        let diagnostic = contender.to_string();
        assert!(diagnostic.contains("surgeist-layout-generate"));
        assert!(diagnostic.contains("scope=full"));
        assert!(diagnostic.contains("command=generate"));
        assert!(diagnostic.contains(&format!("pid={}", std::process::id())));
        assert!(diagnostic.contains("unix_start="));

        let coordination = directory.path().join("target/surgeist-generator");
        let owner_files = fs::read_dir(&coordination)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().ends_with(".lease.lock"))
            .collect::<Vec<_>>();
        assert_eq!(owner_files.len(), 3);
        assert!(owner_files.iter().any(|entry| {
            fs::read_to_string(entry.path())
                .is_ok_and(|owner| owner.contains("generator=surgeist-layout-generate"))
        }));

        drop(held);
        GenerationLease::acquire(
            &location,
            "layout",
            "surgeist-layout-generate",
            &RunScope::Full,
            "check-corpus",
        )
        .expect("dropping the lease permits reacquisition");
        drop(unrelated_domain);
        drop(unrelated_corpus);
    }

    #[test]
    fn generation_lease_validates_metadata_before_coordination_writes() {
        let directory = TestDirectory::new("lease-validation");
        let corpus = directory.path().join("corpus");
        fs::create_dir(&corpus).unwrap();
        let location = CorpusLocation::new(directory.path(), &corpus).unwrap();

        let error = GenerationLease::acquire(
            &location,
            "layout\nother",
            "surgeist-layout-generate",
            &RunScope::Full,
            "generate",
        )
        .unwrap_err();
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);
        assert!(!directory.path().join("target").exists());
    }
}
