use std::fs::{self, File, Metadata, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::{CorpusLocation, GeneratorError, GeneratorErrorKind, Result, RunScope, Sha256Digest};

/// An exclusive lease for one generator domain and canonical corpus root.
#[derive(Debug)]
pub struct GenerationLease {
    file: File,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObjectIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(not(unix))]
    length: u64,
    #[cfg(not(unix))]
    modified: Option<SystemTime>,
}

impl ObjectIdentity {
    fn from_metadata(metadata: &Metadata) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Self {
                device: metadata.dev(),
                inode: metadata.ino(),
            }
        }
        #[cfg(not(unix))]
        {
            Self {
                length: metadata.len(),
                modified: metadata.modified().ok(),
            }
        }
    }
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
        Self::acquire_inner(
            location,
            domain.as_ref(),
            generator.as_ref(),
            scope,
            command.as_ref(),
            None,
        )
    }

    fn acquire_inner(
        location: &CorpusLocation,
        domain: &str,
        generator: &str,
        scope: &RunScope,
        command: &str,
        coordination_hook: Option<&dyn Fn(&Path)>,
    ) -> Result<Self> {
        let domain = checked_metadata("domain", domain)?;
        let generator = checked_metadata("generator", generator)?;
        let command = checked_metadata("command", command)?;
        let owner_identity =
            real_directory_identity(location.owner_root(), "validate generation owner root")?;
        let coordination_root = location
            .owner_root()
            .join("target")
            .join("surgeist-generator");
        let coordination_identity =
            create_coordination_root(location.owner_root(), &owner_identity, &coordination_root)?;
        if let Some(hook) = coordination_hook {
            hook(&coordination_root);
        }
        validate_coordination_root(
            location.owner_root(),
            &owner_identity,
            &coordination_root,
            &coordination_identity,
        )?;

        let key = Sha256Digest::from_bytes(format!(
            "domain={domain}\ncorpus={}",
            location.corpus_root().display()
        ));
        let gate_path = coordination_root.join(format!("{key}.gate.lock"));
        let lease_path = coordination_root.join(format!("{key}.lease.lock"));
        let gate = open_lock_file(
            location.owner_root(),
            &owner_identity,
            &coordination_root,
            &coordination_identity,
            &gate_path,
        )?;
        validate_open_lock_file(
            &gate,
            location.owner_root(),
            &owner_identity,
            &coordination_root,
            &coordination_identity,
            &gate_path,
        )?;
        gate.lock().map_err(|source| {
            io_source(
                "lock generation acquisition gate",
                gate_path.display(),
                source,
            )
        })?;

        let mut lease = match open_lock_file(
            location.owner_root(),
            &owner_identity,
            &coordination_root,
            &coordination_identity,
            &lease_path,
        ) {
            Ok(file) => file,
            Err(error) => {
                let _ = gate.unlock();
                return Err(error);
            }
        };
        validate_open_lock_file(
            &lease,
            location.owner_root(),
            &owner_identity,
            &coordination_root,
            &coordination_identity,
            &lease_path,
        )?;
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
        validate_open_lock_file(
            &lease,
            location.owner_root(),
            &owner_identity,
            &coordination_root,
            &coordination_identity,
            &lease_path,
        )?;
        if let Err(source) = write_owner(&mut lease, metadata.as_bytes()) {
            let _ = lease.unlock();
            let _ = gate.unlock();
            return Err(io_source(
                "write generation lease owner",
                lease_path.display(),
                source,
            ));
        }
        if let Err(error) = validate_open_lock_file(
            &lease,
            location.owner_root(),
            &owner_identity,
            &coordination_root,
            &coordination_identity,
            &lease_path,
        ) {
            let _ = lease.unlock();
            let _ = gate.unlock();
            return Err(error);
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

    #[cfg(test)]
    fn acquire_with_coordination_hook(
        location: &CorpusLocation,
        domain: &str,
        generator: &str,
        scope: &RunScope,
        command: &str,
        hook: impl Fn(&Path),
    ) -> Result<Self> {
        Self::acquire_inner(location, domain, generator, scope, command, Some(&hook))
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

fn real_directory_identity(path: &Path, operation: &str) -> Result<ObjectIdentity> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|source| io_source(operation, path.display(), source))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            operation,
            format!("not a real directory: {}", path.display()),
        ));
    }
    Ok(ObjectIdentity::from_metadata(&metadata))
}

fn validate_real_directory(path: &Path, expected: &ObjectIdentity, operation: &str) -> Result<()> {
    let actual = real_directory_identity(path, operation)?;
    if actual != *expected {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            operation,
            format!("directory identity changed: {}", path.display()),
        ));
    }
    Ok(())
}

fn validate_coordination_root(
    owner_root: &Path,
    owner_identity: &ObjectIdentity,
    coordination_root: &Path,
    coordination_identity: &ObjectIdentity,
) -> Result<()> {
    validate_real_directory(
        owner_root,
        owner_identity,
        "revalidate generation owner root",
    )?;
    validate_real_directory(
        coordination_root,
        coordination_identity,
        "revalidate generation coordination root",
    )?;
    let canonical = fs::canonicalize(coordination_root).map_err(|source| {
        io_source(
            "canonicalize generation coordination root",
            coordination_root.display(),
            source,
        )
    })?;
    if !canonical.starts_with(owner_root) {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "revalidate generation coordination root",
            format!(
                "coordination root escaped owner root: {}",
                coordination_root.display()
            ),
        ));
    }
    Ok(())
}

fn validate_open_lock_file(
    file: &File,
    owner_root: &Path,
    owner_identity: &ObjectIdentity,
    coordination_root: &Path,
    coordination_identity: &ObjectIdentity,
    path: &Path,
) -> Result<()> {
    validate_coordination_root(
        owner_root,
        owner_identity,
        coordination_root,
        coordination_identity,
    )?;
    let path_metadata = fs::symlink_metadata(path)
        .map_err(|source| io_source("inspect generation lock path", path.display(), source))?;
    let handle_metadata = file
        .metadata()
        .map_err(|source| io_source("inspect generation lock handle", path.display(), source))?;
    if path_metadata.file_type().is_symlink()
        || !path_metadata.is_file()
        || ObjectIdentity::from_metadata(&path_metadata)
            != ObjectIdentity::from_metadata(&handle_metadata)
    {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "validate generation lock identity",
            format!("lock file identity changed: {}", path.display()),
        ));
    }
    Ok(())
}

fn create_coordination_root(
    owner_root: &Path,
    owner_identity: &ObjectIdentity,
    target: &Path,
) -> Result<ObjectIdentity> {
    let relative = target.strip_prefix(owner_root).map_err(|_| {
        GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "validate generation coordination root",
            target.display().to_string(),
        )
    })?;
    let mut current = owner_root.to_path_buf();
    let mut component_identities: Vec<(std::path::PathBuf, ObjectIdentity)> = Vec::new();
    for component in relative.components() {
        validate_real_directory(
            owner_root,
            owner_identity,
            "revalidate generation owner root",
        )?;
        for (path, identity) in &component_identities {
            validate_real_directory(
                path,
                identity,
                "revalidate generation coordination component",
            )?;
        }
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
        component_identities.push((current.clone(), ObjectIdentity::from_metadata(&metadata)));
    }
    validate_real_directory(
        owner_root,
        owner_identity,
        "revalidate generation owner root",
    )?;
    for (path, identity) in &component_identities {
        validate_real_directory(
            path,
            identity,
            "revalidate generation coordination component",
        )?;
    }
    real_directory_identity(target, "validate generation coordination root")
}

fn open_lock_file(
    owner_root: &Path,
    owner_identity: &ObjectIdentity,
    coordination_root: &Path,
    coordination_identity: &ObjectIdentity,
    path: &Path,
) -> Result<File> {
    validate_coordination_root(
        owner_root,
        owner_identity,
        coordination_root,
        coordination_identity,
    )?;
    let file = match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                return Err(GeneratorError::new(
                    GeneratorErrorKind::InvalidPath,
                    "validate generation lock file",
                    format!("not a real file: {}", path.display()),
                ));
            }
            OpenOptions::new().read(true).write(true).open(path)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .open(path),
        Err(error) => Err(error),
    }
    .map_err(|source| io_source("open generation lock file", path.display(), source))?;
    validate_open_lock_file(
        &file,
        owner_root,
        owner_identity,
        coordination_root,
        coordination_identity,
        path,
    )?;
    Ok(file)
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

    use super::{GenerationLease, open_lock_file, real_directory_identity};
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

    #[cfg(unix)]
    #[test]
    fn generation_lease_rejects_coordination_path_swap() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new("lease-coordination-swap");
        let outside = TestDirectory::new("lease-coordination-outside");
        let corpus = directory.path().join("corpus");
        fs::create_dir(&corpus).unwrap();
        let location = CorpusLocation::new(directory.path(), &corpus).unwrap();

        symlink(outside.path(), directory.path().join("target")).unwrap();
        let construction_error = GenerationLease::acquire(
            &location,
            "layout",
            "surgeist-layout-generate",
            &RunScope::Full,
            "generate",
        )
        .expect_err("a symlinked target directory must be rejected");
        assert_eq!(construction_error.kind(), GeneratorErrorKind::InvalidPath);
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);
        fs::remove_file(directory.path().join("target")).unwrap();

        let error = GenerationLease::acquire_with_coordination_hook(
            &location,
            "layout",
            "surgeist-layout-generate",
            &RunScope::Full,
            "generate",
            |coordination| {
                fs::rename(coordination, coordination.with_extension("original")).unwrap();
                symlink(outside.path(), coordination).unwrap();
            },
        )
        .expect_err("a coordination-directory swap must be rejected");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);

        fs::remove_file(directory.path().join("target/surgeist-generator")).unwrap();
        fs::rename(
            directory.path().join("target/surgeist-generator.original"),
            directory.path().join("target/surgeist-generator"),
        )
        .unwrap();
        let lock_link = directory
            .path()
            .join("target/surgeist-generator/attacker.gate.lock");
        let outside_lock = outside.path().join("outside.lock");
        fs::write(&outside_lock, b"outside").unwrap();
        symlink(&outside_lock, &lock_link).unwrap();
        let lock_error = open_lock_file(
            location.owner_root(),
            &real_directory_identity(location.owner_root(), "test owner").unwrap(),
            &directory.path().join("target/surgeist-generator"),
            &real_directory_identity(
                &directory.path().join("target/surgeist-generator"),
                "test coordination",
            )
            .unwrap(),
            &lock_link,
        )
        .expect_err("a symlinked lock file must be rejected");
        assert_eq!(lock_error.kind(), GeneratorErrorKind::InvalidPath);
        assert_eq!(fs::read(&outside_lock).unwrap(), b"outside");
    }
}
