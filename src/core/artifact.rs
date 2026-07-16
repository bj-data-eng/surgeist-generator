use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::{GeneratorError, GeneratorErrorKind, RelativePath, Result, RunScope, Sha256Digest};

#[derive(Debug)]
struct PlannedArtifact {
    bytes: Vec<u8>,
    digest: Sha256Digest,
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
    modified: Option<std::time::SystemTime>,
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

#[derive(Debug)]
struct FileSnapshot {
    bytes: Vec<u8>,
    permissions: fs::Permissions,
}

#[derive(Debug, Default)]
struct TransactionSnapshot {
    files: BTreeMap<PathBuf, FileSnapshot>,
    directories: BTreeSet<PathBuf>,
}

impl TransactionSnapshot {
    fn capture<'a>(
        root: &Path,
        root_identity: &ObjectIdentity,
        kind: &PlanKind,
        paths: impl IntoIterator<Item = &'a RelativePath>,
    ) -> Result<Self> {
        let mut snapshot = Self::default();
        for path in paths {
            let target = path.join(root);
            validate_root_identity(root, root_identity)?;
            validate_components(root, &target, false)?;
            if target_exists_as_file(&target)? {
                let mut file = File::open(&target).map_err(|source| {
                    transaction_source("snapshot artifact", target.display(), source)
                })?;
                validate_opened_file(&file, &target)?;
                let metadata = file.metadata().map_err(|source| {
                    transaction_source("snapshot artifact", target.display(), source)
                })?;
                let mut bytes = Vec::new();
                file.read_to_end(&mut bytes).map_err(|source| {
                    transaction_source("snapshot artifact", target.display(), source)
                })?;
                validate_opened_file(&file, &target)?;
                snapshot.files.insert(
                    target,
                    FileSnapshot {
                        bytes,
                        permissions: metadata.permissions(),
                    },
                );
            }
        }
        if let PlanKind::Generated { generated_root, .. } = kind {
            let directory = generated_root.join(root);
            if directory.exists() {
                capture_directories(root, root_identity, &directory, &mut snapshot.directories)?;
            }
        }
        Ok(snapshot)
    }
}

fn capture_directories(
    root: &Path,
    root_identity: &ObjectIdentity,
    directory: &Path,
    directories: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    revalidate_mutation_parent(root, root_identity, &directory.join(".inventory"))?;
    let metadata = fs::symlink_metadata(directory).map_err(|source| {
        transaction_source("snapshot artifact directory", directory.display(), source)
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "snapshot artifact directory",
            format!("not a real directory: {}", directory.display()),
        ));
    }
    directories.insert(directory.to_path_buf());
    for entry in fs::read_dir(directory).map_err(|source| {
        transaction_source("snapshot artifact directory", directory.display(), source)
    })? {
        let entry = entry.map_err(|source| {
            transaction_source("snapshot artifact directory", directory.display(), source)
        })?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|source| {
            transaction_source(
                "snapshot artifact directory",
                entry.path().display(),
                source,
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(GeneratorError::new(
                GeneratorErrorKind::InvalidPath,
                "snapshot artifact directory",
                format!("symlink is not allowed: {}", entry.path().display()),
            ));
        }
        if metadata.is_dir() {
            capture_directories(root, root_identity, &entry.path(), directories)?;
        }
    }
    Ok(())
}

#[derive(Debug)]
enum PlanKind {
    Generated {
        generated_root: RelativePath,
        extension: String,
        retained: Option<BTreeSet<RelativePath>>,
    },
    Report,
}

/// A checked set of artifacts to install as one filesystem transaction.
#[derive(Debug)]
pub struct ArtifactPlan {
    output_root: PathBuf,
    output_root_identity: ObjectIdentity,
    artifacts: BTreeMap<RelativePath, PlannedArtifact>,
    kind: PlanKind,
}

impl ArtifactPlan {
    /// Checks one generated-artifact group without mutating disk.
    ///
    /// Full scope requires the complete retained inventory used for stale-file
    /// pruning. Filtered scope requires `None` and can only install its entries.
    pub fn new(
        output_root: impl AsRef<Path>,
        generated_root: RelativePath,
        generated_extension: impl Into<String>,
        scope: RunScope,
        artifacts: Vec<(RelativePath, Vec<u8>)>,
        retained_inventory: Option<Vec<RelativePath>>,
    ) -> Result<Self> {
        let (output_root, output_root_identity) = canonical_output_root(output_root.as_ref())?;
        let extension = generated_extension.into();
        validate_extension(&extension)?;
        validate_generated_root(&output_root, &generated_root)?;

        let artifacts = collect_artifacts(
            &output_root,
            artifacts,
            Some((&generated_root, extension.as_str())),
        )?;
        let retained = match scope {
            RunScope::Full => {
                let retained = retained_inventory.ok_or_else(|| {
                    transaction_error(
                        "construct artifact plan",
                        "full scope requires a complete retained inventory",
                    )
                })?;
                let retained = collect_retained(retained, &generated_root, &extension)?;
                if let Some(path) = artifacts.keys().find(|path| !retained.contains(*path)) {
                    return Err(transaction_error(
                        "construct artifact plan",
                        format!(
                            "generated output is absent from retained inventory: {}",
                            path.as_str()
                        ),
                    ));
                }
                let stale =
                    collect_stale(&output_root, None, &generated_root, &extension, &retained)?;
                validate_temporary_paths(&output_root, &artifacts, &stale)?;
                Some(retained)
            }
            RunScope::Filtered(_) => {
                if retained_inventory.is_some() {
                    return Err(transaction_error(
                        "construct artifact plan",
                        "filtered scope cannot provide or prune a retained inventory",
                    ));
                }
                None
            }
        };
        if matches!(scope, RunScope::Filtered(_)) {
            validate_temporary_paths(&output_root, &artifacts, &[])?;
        }

        Ok(Self {
            output_root,
            output_root_identity,
            artifacts,
            kind: PlanKind::Generated {
                generated_root,
                extension,
                retained,
            },
        })
    }

    /// Checks one canonical report replacement and rejects filtered scope.
    pub fn report(
        output_root: impl AsRef<Path>,
        report_path: RelativePath,
        scope: RunScope,
        bytes: Vec<u8>,
    ) -> Result<Self> {
        if !scope.may_write_report() {
            return Err(transaction_error(
                "construct report plan",
                "filtered scope cannot publish a report",
            ));
        }
        let (output_root, output_root_identity) = canonical_output_root(output_root.as_ref())?;
        let artifacts = collect_artifacts(&output_root, vec![(report_path, bytes)], None)?;
        validate_temporary_paths(&output_root, &artifacts, &[])?;
        Ok(Self {
            output_root,
            output_root_identity,
            artifacts,
            kind: PlanKind::Report,
        })
    }

    /// Installs this checked plan with staged replacement and rollback.
    pub fn install(&self) -> Result<()> {
        self.install_inner(None, None, None)
    }

    /// Returns the digest computed during checked plan construction.
    #[must_use]
    pub fn artifact_digest(&self, path: &RelativePath) -> Option<&Sha256Digest> {
        self.artifacts.get(path).map(|artifact| &artifact.digest)
    }

    #[cfg(test)]
    fn install_with_failure(&self, install_index: Option<usize>) -> Result<()> {
        self.install_inner(None, install_index, None)
    }

    #[cfg(test)]
    fn install_with_stage_failure(&self, stage_index: usize) -> Result<()> {
        self.install_inner(Some(stage_index), None, None)
    }

    #[cfg(test)]
    fn install_with_cleanup_failure(&self, cleanup_index: usize) -> Result<()> {
        self.install_inner(None, None, Some(cleanup_index))
    }

    fn install_inner(
        &self,
        fail_stage_at: Option<usize>,
        fail_install_at: Option<usize>,
        fail_cleanup_at: Option<usize>,
    ) -> Result<()> {
        validate_root_identity(&self.output_root, &self.output_root_identity)?;
        let stale = match &self.kind {
            PlanKind::Generated {
                generated_root,
                extension,
                retained,
            } => match retained {
                Some(retained) => collect_stale(
                    &self.output_root,
                    Some(&self.output_root_identity),
                    generated_root,
                    extension,
                    retained,
                )?,
                None => Vec::new(),
            },
            PlanKind::Report => Vec::new(),
        };
        validate_temporary_paths(&self.output_root, &self.artifacts, &stale)?;
        let snapshot = TransactionSnapshot::capture(
            &self.output_root,
            &self.output_root_identity,
            &self.kind,
            self.artifacts.keys().chain(stale.iter()),
        )?;

        let mut created_directories = Vec::new();
        let mut stages = BTreeMap::new();
        for (index, (path, artifact)) in self.artifacts.iter().enumerate() {
            let target = path.join(&self.output_root);
            stages.insert(path.clone(), temporary_sibling(&target, "stage")?);
            let staged = ensure_parent_directories(
                &self.output_root,
                &self.output_root_identity,
                &target,
                &mut created_directories,
            )
            .and_then(|()| {
                if fail_stage_at == Some(index) {
                    Err(transaction_error(
                        "stage artifact",
                        format!("injected staging failure for {}", path.as_str()),
                    ))
                } else {
                    stage_artifact(
                        &self.output_root,
                        &self.output_root_identity,
                        &target,
                        &artifact.bytes,
                    )
                }
            });
            if let Err(error) = staged {
                return self.rollback_transaction(
                    error,
                    &[],
                    &[],
                    stages.values(),
                    &created_directories,
                    &snapshot,
                );
            }
        }

        let mut backup_paths = BTreeSet::new();
        for path in self.artifacts.keys().chain(stale.iter()) {
            let target = path.join(&self.output_root);
            if let Err(error) =
                revalidate_mutation_parent(&self.output_root, &self.output_root_identity, &target)
            {
                return self.rollback_transaction(
                    error,
                    &[],
                    &[],
                    stages.values(),
                    &created_directories,
                    &snapshot,
                );
            }
            match target_exists_as_file(&target) {
                Ok(true) => {
                    backup_paths.insert(path.clone());
                }
                Ok(false) => {}
                Err(error) => {
                    return self.rollback_transaction(
                        error,
                        &[],
                        &[],
                        stages.values(),
                        &created_directories,
                        &snapshot,
                    );
                }
            }
        }

        let mut backups = Vec::new();
        for path in backup_paths {
            let target = path.join(&self.output_root);
            let backup = temporary_sibling(&target, "backup")?;
            if let Err(error) =
                revalidate_mutation_parent(&self.output_root, &self.output_root_identity, &target)
            {
                return self.rollback_transaction(
                    error,
                    &[],
                    &backups,
                    stages.values(),
                    &created_directories,
                    &snapshot,
                );
            }
            if let Err(error) = require_absent(&backup, "validate artifact backup") {
                return self.rollback_transaction(
                    error,
                    &[],
                    &backups,
                    stages.values(),
                    &created_directories,
                    &snapshot,
                );
            }
            if let Err(error) = fs::rename(&target, &backup)
                .map_err(|source| transaction_source("backup artifact", target.display(), source))
            {
                return self.rollback_transaction(
                    error,
                    &[],
                    &backups,
                    stages.values(),
                    &created_directories,
                    &snapshot,
                );
            }
            backups.push((target, backup));
            if let Err(error) = revalidate_existing_file(
                &self.output_root,
                &self.output_root_identity,
                &backups.last().expect("backup was just recorded").1,
            ) {
                return self.rollback_transaction(
                    error,
                    &[],
                    &backups,
                    stages.values(),
                    &created_directories,
                    &snapshot,
                );
            }
        }

        let mut installed = Vec::new();
        for (index, (path, stage)) in stages.iter().enumerate() {
            if fail_install_at == Some(index) {
                let error = transaction_error(
                    "install artifact",
                    format!("injected installation failure for {}", path.as_str()),
                );
                return self.rollback_transaction(
                    error,
                    &installed,
                    &backups,
                    stages.values(),
                    &created_directories,
                    &snapshot,
                );
            }
            let target = path.join(&self.output_root);
            if let Err(error) =
                revalidate_existing_file(&self.output_root, &self.output_root_identity, stage)
            {
                return self.rollback_transaction(
                    error,
                    &installed,
                    &backups,
                    stages.values(),
                    &created_directories,
                    &snapshot,
                );
            }
            if let Err(error) = require_absent(&target, "validate artifact install target") {
                return self.rollback_transaction(
                    error,
                    &installed,
                    &backups,
                    stages.values(),
                    &created_directories,
                    &snapshot,
                );
            }
            if let Err(source) = fs::rename(stage, &target) {
                let error = transaction_source("install artifact", target.display(), source);
                return self.rollback_transaction(
                    error,
                    &installed,
                    &backups,
                    stages.values(),
                    &created_directories,
                    &snapshot,
                );
            }
            installed.push(target.clone());
            if let Err(error) =
                revalidate_existing_file(&self.output_root, &self.output_root_identity, &target)
            {
                return self.rollback_transaction(
                    error,
                    &installed,
                    &backups,
                    stages.values(),
                    &created_directories,
                    &snapshot,
                );
            }
        }

        for (cleanup_index, (_, backup)) in backups.iter().enumerate() {
            if fail_cleanup_at == Some(cleanup_index) {
                let error = transaction_error(
                    "remove artifact backup",
                    format!("injected cleanup failure for {}", backup.display()),
                );
                return self.rollback_transaction(
                    error,
                    &installed,
                    &backups,
                    stages.values(),
                    &created_directories,
                    &snapshot,
                );
            }
            if let Err(error) =
                revalidate_mutation_parent(&self.output_root, &self.output_root_identity, backup)
            {
                return self.rollback_transaction(
                    error,
                    &installed,
                    &backups,
                    stages.values(),
                    &created_directories,
                    &snapshot,
                );
            }
            fs::remove_file(backup)
                .map_err(|source| {
                    transaction_source("remove artifact backup", backup.display(), source)
                })
                .or_else(|error| {
                    self.rollback_transaction(
                        error,
                        &installed,
                        &backups,
                        stages.values(),
                        &created_directories,
                        &snapshot,
                    )
                })?;
        }
        if let PlanKind::Generated {
            generated_root,
            extension,
            ..
        } = &self.kind
        {
            let _ = extension;
            if let Err(error) = remove_empty_directories(
                &generated_root.join(&self.output_root),
                &self.output_root,
                &self.output_root_identity,
            ) {
                return self.rollback_transaction(
                    error,
                    &installed,
                    &backups,
                    stages.values(),
                    &created_directories,
                    &snapshot,
                );
            }
        }
        Ok(())
    }

    fn rollback_transaction<'a>(
        &self,
        cause: GeneratorError,
        installed: &[PathBuf],
        backups: &[(PathBuf, PathBuf)],
        stages: impl IntoIterator<Item = &'a PathBuf>,
        created_directories: &[PathBuf],
        snapshot: &TransactionSnapshot,
    ) -> Result<()> {
        rollback_snapshot(
            cause,
            &self.output_root,
            &self.output_root_identity,
            installed,
            backups,
            stages,
            created_directories,
            snapshot,
        )
    }
}

fn canonical_output_root(path: &Path) -> Result<(PathBuf, ObjectIdentity)> {
    if path.to_str().is_none() {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "validate artifact output root",
            "non-UTF-8 output root",
        ));
    }
    let supplied_metadata = fs::symlink_metadata(path).map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::InvalidPath,
            "inspect artifact output root",
            path.display().to_string(),
            source,
        )
    })?;
    if supplied_metadata.file_type().is_symlink() || !supplied_metadata.is_dir() {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "validate artifact output root",
            format!("not a real directory: {}", path.display()),
        ));
    }
    let canonical = fs::canonicalize(path).map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            GeneratorError::new(
                GeneratorErrorKind::InvalidPath,
                "canonicalize artifact output root",
                format!("missing path: {}", path.display()),
            )
        } else {
            GeneratorError::with_source(
                GeneratorErrorKind::Io,
                "canonicalize artifact output root",
                path.display().to_string(),
                source,
            )
        }
    })?;
    if !canonical.is_dir() {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "validate artifact output root",
            format!("not a directory: {}", path.display()),
        ));
    }
    let metadata = fs::symlink_metadata(&canonical).map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::Io,
            "inspect canonical artifact output root",
            canonical.display().to_string(),
            source,
        )
    })?;
    Ok((canonical, ObjectIdentity::from_metadata(&metadata)))
}

fn validate_extension(extension: &str) -> Result<()> {
    if extension.is_empty()
        || extension.contains('.')
        || extension.contains('/')
        || extension.contains('\\')
        || extension.trim() != extension
    {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "validate generated extension",
            extension,
        ));
    }
    Ok(())
}

fn validate_generated_root(root: &Path, generated: &RelativePath) -> Result<()> {
    let target = generated.join(root);
    validate_components(root, &target, true)
}

fn collect_artifacts(
    root: &Path,
    artifacts: Vec<(RelativePath, Vec<u8>)>,
    boundary: Option<(&RelativePath, &str)>,
) -> Result<BTreeMap<RelativePath, PlannedArtifact>> {
    let mut collected = BTreeMap::new();
    for (path, bytes) in artifacts {
        if let Some((generated_root, extension)) = boundary {
            validate_generated_path(&path, generated_root, extension)?;
        }
        validate_components(root, &path.join(root), false)?;
        let digest = Sha256Digest::from_bytes(&bytes);
        if collected
            .insert(path.clone(), PlannedArtifact { bytes, digest })
            .is_some()
        {
            return Err(transaction_error(
                "construct artifact plan",
                format!("duplicate output path: {}", path.as_str()),
            ));
        }
    }
    Ok(collected)
}

fn collect_retained(
    retained: Vec<RelativePath>,
    generated_root: &RelativePath,
    extension: &str,
) -> Result<BTreeSet<RelativePath>> {
    let mut collected = BTreeSet::new();
    for path in retained {
        validate_generated_path(&path, generated_root, extension)?;
        if !collected.insert(path.clone()) {
            return Err(transaction_error(
                "construct artifact plan",
                format!("duplicate retained path: {}", path.as_str()),
            ));
        }
    }
    Ok(collected)
}

fn validate_generated_path(
    path: &RelativePath,
    generated_root: &RelativePath,
    extension: &str,
) -> Result<()> {
    let suffix = path
        .as_str()
        .strip_prefix(generated_root.as_str())
        .filter(|suffix| suffix.starts_with('/'))
        .ok_or_else(|| {
            GeneratorError::new(
                GeneratorErrorKind::InvalidPath,
                "validate generated artifact boundary",
                path.as_str(),
            )
        })?;
    if suffix.len() == 1
        || Path::new(path.as_str()).extension() != Some(std::ffi::OsStr::new(extension))
    {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "validate generated artifact extension",
            path.as_str(),
        ));
    }
    Ok(())
}

fn validate_components(root: &Path, target: &Path, final_directory: bool) -> Result<()> {
    let relative = target.strip_prefix(root).map_err(|_| {
        GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "validate artifact target",
            target.display().to_string(),
        )
    })?;
    let mut current = root.to_path_buf();
    let component_count = relative.components().count();
    for (index, component) in relative.components().enumerate() {
        current.push(component.as_os_str());
        let metadata = match fs::symlink_metadata(&current) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(source) => {
                return Err(GeneratorError::with_source(
                    GeneratorErrorKind::Io,
                    "inspect artifact target",
                    current.display().to_string(),
                    source,
                ));
            }
        };
        if metadata.file_type().is_symlink() {
            return Err(GeneratorError::new(
                GeneratorErrorKind::InvalidPath,
                "validate artifact target",
                format!("symlink is not allowed: {}", current.display()),
            ));
        }
        let is_final = index + 1 == component_count;
        if (!is_final || final_directory) && !metadata.is_dir() {
            return Err(transaction_error(
                "validate artifact target",
                format!("directory collision: {}", current.display()),
            ));
        }
        if is_final && !final_directory && !metadata.is_file() {
            return Err(transaction_error(
                "validate artifact target",
                format!("non-file collision: {}", current.display()),
            ));
        }
    }
    Ok(())
}

fn collect_stale(
    root: &Path,
    root_identity: Option<&ObjectIdentity>,
    generated_root: &RelativePath,
    extension: &str,
    retained: &BTreeSet<RelativePath>,
) -> Result<Vec<RelativePath>> {
    let directory = generated_root.join(root);
    if let Some(identity) = root_identity {
        validate_root_identity(root, identity)?;
        validate_components(root, &directory, true)?;
    }
    match fs::symlink_metadata(&directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            return Err(GeneratorError::new(
                GeneratorErrorKind::InvalidPath,
                "inventory generated artifacts",
                format!("not a real directory: {}", directory.display()),
            ));
        }
        Err(source) => {
            return Err(GeneratorError::with_source(
                GeneratorErrorKind::Io,
                "inventory generated artifacts",
                directory.display().to_string(),
                source,
            ));
        }
    }
    let mut stale = Vec::new();
    collect_stale_directory(
        root,
        root_identity,
        &directory,
        extension,
        retained,
        &mut stale,
    )?;
    stale.sort();
    Ok(stale)
}

fn collect_stale_directory(
    root: &Path,
    root_identity: Option<&ObjectIdentity>,
    directory: &Path,
    extension: &str,
    retained: &BTreeSet<RelativePath>,
    stale: &mut Vec<RelativePath>,
) -> Result<()> {
    if let Some(identity) = root_identity {
        revalidate_mutation_parent(root, identity, &directory.join(".inventory"))?;
    }
    let entries = fs::read_dir(directory).map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::Io,
            "read generated directory",
            directory.display().to_string(),
            source,
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|source| {
            GeneratorError::with_source(
                GeneratorErrorKind::Io,
                "read generated entry",
                directory.display().to_string(),
                source,
            )
        })?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|source| {
            GeneratorError::with_source(
                GeneratorErrorKind::Io,
                "inspect generated entry",
                entry.path().display().to_string(),
                source,
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(GeneratorError::new(
                GeneratorErrorKind::InvalidPath,
                "inventory generated artifacts",
                format!("symlink is not allowed: {}", entry.path().display()),
            ));
        }
        if metadata.is_dir() {
            collect_stale_directory(
                root,
                root_identity,
                &entry.path(),
                extension,
                retained,
                stale,
            )?;
        } else if metadata.is_file() {
            if entry.path().extension() == Some(std::ffi::OsStr::new(extension)) {
                let entry_path = entry.path();
                let relative = entry_path.strip_prefix(root).map_err(|_| {
                    GeneratorError::new(
                        GeneratorErrorKind::InvalidPath,
                        "inventory generated artifacts",
                        entry_path.display().to_string(),
                    )
                })?;
                let relative = RelativePath::from_path(relative)?;
                if !retained.contains(&relative) {
                    stale.push(relative);
                }
            }
        } else {
            return Err(transaction_error(
                "inventory generated artifacts",
                format!("special entry is not allowed: {}", entry.path().display()),
            ));
        }
    }
    Ok(())
}

fn validate_temporary_paths(
    root: &Path,
    artifacts: &BTreeMap<RelativePath, PlannedArtifact>,
    stale: &[RelativePath],
) -> Result<()> {
    let mut temporary = BTreeSet::new();
    for path in artifacts.keys() {
        let target = path.join(root);
        let stage = temporary_sibling(&target, "stage")?;
        if !temporary.insert(stage.clone()) {
            return Err(transaction_error(
                "validate artifact staging",
                format!("temporary path collision: {}", stage.display()),
            ));
        }
        require_absent(&stage, "validate artifact staging")?;
    }
    for path in artifacts.keys().chain(stale.iter()) {
        let target = path.join(root);
        if target.exists() {
            let backup = temporary_sibling(&target, "backup")?;
            if !temporary.insert(backup.clone()) {
                return Err(transaction_error(
                    "validate artifact backup",
                    format!("temporary path collision: {}", backup.display()),
                ));
            }
            require_absent(&backup, "validate artifact backup")?;
        }
    }
    Ok(())
}

fn require_absent(path: &Path, operation: &str) -> Result<()> {
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Ok(_) => Err(transaction_error(
            operation,
            format!("residual collision: {}", path.display()),
        )),
        Err(source) => Err(GeneratorError::with_source(
            GeneratorErrorKind::Io,
            operation,
            path.display().to_string(),
            source,
        )),
    }
}

fn temporary_sibling(target: &Path, suffix: &str) -> Result<PathBuf> {
    let name = target.file_name().ok_or_else(|| {
        transaction_error(
            "construct artifact temporary path",
            target.display().to_string(),
        )
    })?;
    let mut temporary_name = OsString::from(".");
    temporary_name.push(name);
    temporary_name.push(format!(".surgeist-{suffix}"));
    Ok(target.with_file_name(temporary_name))
}

fn validate_root_identity(root: &Path, expected: &ObjectIdentity) -> Result<()> {
    let metadata = fs::symlink_metadata(root).map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::InvalidPath,
            "revalidate artifact output root",
            root.display().to_string(),
            source,
        )
    })?;
    if metadata.file_type().is_symlink()
        || !metadata.is_dir()
        || ObjectIdentity::from_metadata(&metadata) != *expected
    {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "revalidate artifact output root",
            format!("output root identity changed: {}", root.display()),
        ));
    }
    let canonical = fs::canonicalize(root).map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::InvalidPath,
            "revalidate artifact output root",
            root.display().to_string(),
            source,
        )
    })?;
    if canonical != root {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "revalidate artifact output root",
            format!("output root path changed: {}", root.display()),
        ));
    }
    Ok(())
}

fn revalidate_mutation_parent(
    root: &Path,
    root_identity: &ObjectIdentity,
    target: &Path,
) -> Result<()> {
    validate_root_identity(root, root_identity)?;
    let parent = target.parent().ok_or_else(|| {
        transaction_error("revalidate artifact mutation", target.display().to_string())
    })?;
    validate_components(root, parent, true)?;
    let canonical = fs::canonicalize(parent).map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::InvalidPath,
            "revalidate artifact mutation parent",
            parent.display().to_string(),
            source,
        )
    })?;
    if !canonical.starts_with(root) {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "revalidate artifact mutation parent",
            format!("parent escaped output root: {}", parent.display()),
        ));
    }
    Ok(())
}

fn revalidate_existing_file(
    root: &Path,
    root_identity: &ObjectIdentity,
    path: &Path,
) -> Result<()> {
    revalidate_mutation_parent(root, root_identity, path)?;
    let metadata = fs::symlink_metadata(path).map_err(|source| {
        GeneratorError::with_source(
            GeneratorErrorKind::InvalidPath,
            "revalidate artifact file",
            path.display().to_string(),
            source,
        )
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "revalidate artifact file",
            format!("not a real file: {}", path.display()),
        ));
    }
    Ok(())
}

fn validate_opened_file(file: &File, path: &Path) -> Result<()> {
    let handle = file.metadata().map_err(|source| {
        transaction_source("inspect opened artifact file", path.display(), source)
    })?;
    let path_metadata = fs::symlink_metadata(path).map_err(|source| {
        transaction_source("inspect opened artifact path", path.display(), source)
    })?;
    if path_metadata.file_type().is_symlink()
        || !path_metadata.is_file()
        || ObjectIdentity::from_metadata(&handle) != ObjectIdentity::from_metadata(&path_metadata)
    {
        return Err(GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "validate opened artifact path",
            format!("opened file identity changed: {}", path.display()),
        ));
    }
    Ok(())
}

fn ensure_parent_directories(
    root: &Path,
    root_identity: &ObjectIdentity,
    target: &Path,
    created: &mut Vec<PathBuf>,
) -> Result<()> {
    let parent = target.parent().ok_or_else(|| {
        transaction_error("create artifact parents", target.display().to_string())
    })?;
    let relative = parent.strip_prefix(root).map_err(|_| {
        GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "create artifact parents",
            parent.display().to_string(),
        )
    })?;
    let mut current = root.to_path_buf();
    for component in relative.components() {
        current.push(component.as_os_str());
        validate_root_identity(root, root_identity)?;
        match fs::create_dir(&current) {
            Ok(()) => {
                validate_components(root, &current, true)?;
                created.push(current.clone());
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                validate_components(root, &current, true)?;
            }
            Err(source) => {
                return Err(transaction_source(
                    "create artifact parent",
                    current.display(),
                    source,
                ));
            }
        }
    }
    Ok(())
}

fn stage_artifact(
    root: &Path,
    root_identity: &ObjectIdentity,
    target: &Path,
    bytes: &[u8],
) -> Result<()> {
    let stage = temporary_sibling(target, "stage")?;
    revalidate_mutation_parent(root, root_identity, &stage)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&stage)
        .map_err(|source| transaction_source("create artifact stage", stage.display(), source))?;
    if let Err(error) = validate_opened_file(&file, &stage) {
        drop(file);
        if revalidate_mutation_parent(root, root_identity, &stage).is_ok() {
            let _ = fs::remove_file(&stage);
        }
        return Err(error);
    }
    if let Err(source) = file
        .write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
    {
        drop(file);
        if revalidate_mutation_parent(root, root_identity, &stage).is_ok() {
            let _ = fs::remove_file(&stage);
        }
        return Err(transaction_source(
            "write artifact stage",
            stage.display(),
            source,
        ));
    }
    validate_opened_file(&file, &stage)?;
    Ok(())
}

fn target_exists_as_file(target: &Path) -> Result<bool> {
    match fs::symlink_metadata(target) {
        Ok(metadata) if metadata.is_file() => Ok(true),
        Ok(metadata) if metadata.file_type().is_symlink() => Err(GeneratorError::new(
            GeneratorErrorKind::InvalidPath,
            "inspect artifact target",
            format!("symlink is not allowed: {}", target.display()),
        )),
        Ok(_) => Err(transaction_error(
            "inspect artifact target",
            format!("non-file collision: {}", target.display()),
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(GeneratorError::with_source(
            GeneratorErrorKind::Io,
            "inspect artifact target",
            target.display().to_string(),
            source,
        )),
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "rollback must receive every live transaction collection explicitly"
)]
fn rollback_snapshot<'a>(
    cause: GeneratorError,
    root: &Path,
    root_identity: &ObjectIdentity,
    installed: &[PathBuf],
    backups: &[(PathBuf, PathBuf)],
    stages: impl IntoIterator<Item = &'a PathBuf>,
    created_directories: &[PathBuf],
    snapshot: &TransactionSnapshot,
) -> Result<()> {
    let mut failures = Vec::new();
    for target in installed.iter().rev() {
        if let Err(error) = revalidate_mutation_parent(root, root_identity, target) {
            failures.push(format!("revalidate {}: {error}", target.display()));
        } else if let Err(error) = fs::remove_file(target)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            failures.push(format!("remove {}: {error}", target.display()));
        }
    }
    for (_, backup) in backups.iter().rev() {
        if let Err(error) = revalidate_mutation_parent(root, root_identity, backup) {
            failures.push(format!("revalidate {}: {error}", backup.display()));
        } else if let Err(error) = fs::remove_file(backup)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            failures.push(format!("remove {}: {error}", backup.display()));
        }
    }
    for stage in stages {
        if let Err(error) = revalidate_mutation_parent(root, root_identity, stage) {
            failures.push(format!("revalidate {}: {error}", stage.display()));
        } else if let Err(error) = fs::remove_file(stage)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            failures.push(format!("remove {}: {error}", stage.display()));
        }
    }
    let mut directories = snapshot.directories.iter().collect::<Vec<_>>();
    directories.sort_by_key(|path| path.components().count());
    for directory in directories {
        if directory.exists() {
            continue;
        }
        if let Some(parent) = directory.parent()
            && let Err(error) =
                revalidate_mutation_parent(root, root_identity, &parent.join(".restore"))
        {
            failures.push(format!("revalidate {}: {error}", directory.display()));
            continue;
        }
        if let Err(error) = fs::create_dir(directory) {
            failures.push(format!(
                "restore directory {}: {error}",
                directory.display()
            ));
        }
    }
    for (target, original) in &snapshot.files {
        if let Err(error) = revalidate_mutation_parent(root, root_identity, target) {
            failures.push(format!("revalidate {}: {error}", target.display()));
            continue;
        }
        match OpenOptions::new().write(true).create_new(true).open(target) {
            Ok(mut file) => {
                let restored = validate_opened_file(&file, target)
                    .and_then(|()| {
                        file.write_all(&original.bytes)
                            .and_then(|()| file.flush())
                            .and_then(|()| file.sync_all())
                            .map_err(|source| {
                                transaction_source("restore artifact", target.display(), source)
                            })
                    })
                    .and_then(|()| {
                        revalidate_mutation_parent(root, root_identity, target)?;
                        fs::set_permissions(target, original.permissions.clone()).map_err(
                            |source| {
                                transaction_source(
                                    "restore artifact permissions",
                                    target.display(),
                                    source,
                                )
                            },
                        )
                    });
                if let Err(error) = restored {
                    failures.push(format!("restore {}: {error}", target.display()));
                }
            }
            Err(error) => failures.push(format!("restore {}: {error}", target.display())),
        }
    }
    for directory in created_directories.iter().rev() {
        if let Err(error) =
            revalidate_mutation_parent(root, root_identity, &directory.join(".remove"))
        {
            failures.push(format!("revalidate {}: {error}", directory.display()));
        } else if let Err(error) = fs::remove_dir(directory)
            && error.kind() != std::io::ErrorKind::NotFound
        {
            failures.push(format!("remove directory {}: {error}", directory.display()));
        }
    }
    if failures.is_empty() {
        Err(cause)
    } else {
        Err(transaction_error(
            "rollback artifact transaction",
            format!("{cause}; rollback failures: {}", failures.join("; ")),
        ))
    }
}

fn remove_empty_directories(
    directory: &Path,
    output_root: &Path,
    root_identity: &ObjectIdentity,
) -> Result<bool> {
    revalidate_mutation_parent(output_root, root_identity, &directory.join(".inspect"))?;
    match fs::symlink_metadata(directory) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(true),
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => {
            return Err(GeneratorError::new(
                GeneratorErrorKind::InvalidPath,
                "remove empty generated directory",
                format!("not a real directory: {}", directory.display()),
            ));
        }
        Err(source) => {
            return Err(GeneratorError::with_source(
                GeneratorErrorKind::Io,
                "inspect generated directory",
                directory.display().to_string(),
                source,
            ));
        }
    }
    for entry in fs::read_dir(directory).map_err(|source| {
        transaction_source("inspect generated directory", directory.display(), source)
    })? {
        let entry = entry.map_err(|source| {
            transaction_source("inspect generated directory", directory.display(), source)
        })?;
        let metadata = fs::symlink_metadata(entry.path()).map_err(|source| {
            transaction_source(
                "inspect generated directory",
                entry.path().display(),
                source,
            )
        })?;
        if metadata.file_type().is_symlink() {
            return Err(GeneratorError::new(
                GeneratorErrorKind::InvalidPath,
                "remove empty generated directory",
                format!("symlink is not allowed: {}", entry.path().display()),
            ));
        }
        if metadata.is_dir() {
            remove_empty_directories(&entry.path(), output_root, root_identity)?;
        }
    }
    if directory != output_root {
        revalidate_mutation_parent(output_root, root_identity, &directory.join(".remove"))?;
        match fs::remove_dir(directory) {
            Ok(()) => return Ok(true),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::DirectoryNotEmpty | std::io::ErrorKind::NotFound
                ) => {}
            Err(source) => {
                return Err(transaction_source(
                    "remove empty generated directory",
                    directory.display(),
                    source,
                ));
            }
        }
    }
    Ok(false)
}

fn transaction_error(operation: &str, detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(GeneratorErrorKind::ArtifactTransaction, operation, detail)
}

fn transaction_source(
    operation: &str,
    detail: impl std::fmt::Display,
    source: std::io::Error,
) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::ArtifactTransaction,
        operation,
        detail.to_string(),
        source,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::ArtifactPlan;
    use crate::{GeneratorErrorKind, RelativePath, RunScope};

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

    fn relative(value: &str) -> RelativePath {
        RelativePath::new(value).expect("valid test path")
    }

    fn snapshot_tree(root: &Path) -> BTreeMap<String, Option<Vec<u8>>> {
        fn visit(root: &Path, directory: &Path, snapshot: &mut BTreeMap<String, Option<Vec<u8>>>) {
            let mut entries = fs::read_dir(directory)
                .unwrap()
                .map(|entry| entry.unwrap())
                .collect::<Vec<_>>();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                let relative = path
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                if entry.file_type().unwrap().is_dir() {
                    snapshot.insert(format!("{relative}/"), None);
                    visit(root, &path, snapshot);
                } else {
                    snapshot.insert(relative, Some(fs::read(path).unwrap()));
                }
            }
        }

        let mut snapshot = BTreeMap::new();
        visit(root, root, &mut snapshot);
        snapshot
    }

    #[test]
    fn artifact_transaction_restores_prior_tree() {
        let directory = TestDirectory::new("artifact-rollback");
        fs::create_dir(directory.path().join("generated")).expect("create generated directory");
        fs::write(directory.path().join("generated/a.xml"), b"old-a").expect("write prior file");
        fs::write(directory.path().join("generated/stale.xml"), b"stale")
            .expect("write stale file");

        let plan = ArtifactPlan::new(
            directory.path(),
            relative("generated"),
            "xml",
            RunScope::Full,
            vec![
                (relative("generated/a.xml"), b"new-a".to_vec()),
                (relative("generated/b.xml"), b"new-b".to_vec()),
            ],
            Some(vec![
                relative("generated/a.xml"),
                relative("generated/b.xml"),
            ]),
        )
        .expect("construct checked full plan");

        let error = plan
            .install_with_failure(Some(1))
            .expect_err("injected install failure");
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_eq!(
            fs::read(directory.path().join("generated/a.xml")).unwrap(),
            b"old-a"
        );
        assert_eq!(
            fs::read(directory.path().join("generated/stale.xml")).unwrap(),
            b"stale"
        );
        assert!(!directory.path().join("generated/b.xml").exists());
        assert!(
            fs::read_dir(directory.path().join("generated"))
                .unwrap()
                .all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains(".surgeist-"))
        );
    }

    #[test]
    fn filtered_scope_cannot_publish_or_prune() {
        let directory = TestDirectory::new("filtered-authority");
        let filtered = RunScope::Filtered(relative("source/one.html"));

        let prune_error = ArtifactPlan::new(
            directory.path(),
            relative("generated"),
            "xml",
            filtered.clone(),
            vec![(relative("generated/one.xml"), b"one".to_vec())],
            Some(vec![relative("generated/one.xml")]),
        )
        .expect_err("filtered scope must not accept retained inventory");
        assert_eq!(prune_error.kind(), GeneratorErrorKind::ArtifactTransaction);

        let report_error = ArtifactPlan::report(
            directory.path(),
            relative("report.json"),
            filtered,
            b"{}".to_vec(),
        )
        .expect_err("filtered scope must not publish reports");
        assert_eq!(report_error.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert!(!directory.path().join("report.json").exists());
    }

    #[test]
    fn full_scope_replaces_deterministically_and_prunes_only_generated_files() {
        let directory = TestDirectory::new("artifact-full");
        fs::create_dir_all(directory.path().join("generated/nested"))
            .expect("create generated directories");
        fs::write(directory.path().join("generated/a.xml"), b"old-a").unwrap();
        fs::write(directory.path().join("generated/stale.xml"), b"stale").unwrap();
        fs::write(
            directory.path().join("generated/nested/stale.xml"),
            b"nested-stale",
        )
        .unwrap();
        fs::write(directory.path().join("generated/notes.txt"), b"keep").unwrap();

        let plan = ArtifactPlan::new(
            directory.path(),
            relative("generated"),
            "xml",
            RunScope::Full,
            vec![
                (relative("generated/z.xml"), b"new-z".to_vec()),
                (relative("generated/a.xml"), b"new-a".to_vec()),
            ],
            Some(vec![
                relative("generated/z.xml"),
                relative("generated/a.xml"),
            ]),
        )
        .unwrap();
        assert_eq!(
            plan.artifact_digest(&relative("generated/a.xml")),
            Some(&crate::Sha256Digest::from_bytes(b"new-a"))
        );

        plan.install().expect("install full artifact transaction");
        assert_eq!(
            fs::read(directory.path().join("generated/a.xml")).unwrap(),
            b"new-a"
        );
        assert_eq!(
            fs::read(directory.path().join("generated/z.xml")).unwrap(),
            b"new-z"
        );
        assert!(!directory.path().join("generated/stale.xml").exists());
        assert!(!directory.path().join("generated/nested").exists());
        assert_eq!(
            fs::read(directory.path().join("generated/notes.txt")).unwrap(),
            b"keep"
        );
    }

    #[test]
    fn filtered_scope_installs_without_touching_nonmatching_state() {
        let directory = TestDirectory::new("artifact-filtered-install");
        fs::create_dir(directory.path().join("generated")).unwrap();
        fs::write(directory.path().join("generated/one.xml"), b"old-one").unwrap();
        fs::write(directory.path().join("generated/two.xml"), b"old-two").unwrap();
        fs::write(directory.path().join("report.json"), b"old-report").unwrap();

        ArtifactPlan::new(
            directory.path(),
            relative("generated"),
            "xml",
            RunScope::Filtered(relative("source/one.html")),
            vec![(relative("generated/one.xml"), b"new-one".to_vec())],
            None,
        )
        .unwrap()
        .install()
        .unwrap();

        assert_eq!(
            fs::read(directory.path().join("generated/one.xml")).unwrap(),
            b"new-one"
        );
        assert_eq!(
            fs::read(directory.path().join("generated/two.xml")).unwrap(),
            b"old-two"
        );
        assert_eq!(
            fs::read(directory.path().join("report.json")).unwrap(),
            b"old-report"
        );
    }

    #[test]
    fn full_scope_report_uses_transactional_replace() {
        let directory = TestDirectory::new("artifact-report");
        fs::write(directory.path().join("report.json"), b"old-report").unwrap();
        let report_path = relative("report.json");
        let plan = ArtifactPlan::report(
            directory.path(),
            report_path.clone(),
            RunScope::Full,
            b"new-report".to_vec(),
        )
        .unwrap();
        assert_eq!(
            plan.artifact_digest(&report_path),
            Some(&crate::Sha256Digest::from_bytes(b"new-report"))
        );
        plan.install().unwrap();
        assert_eq!(
            fs::read(directory.path().join("report.json")).unwrap(),
            b"new-report"
        );
    }

    #[test]
    fn staging_failure_removes_stages_and_new_directories() {
        let directory = TestDirectory::new("artifact-stage-failure");
        fs::create_dir(directory.path().join("generated")).unwrap();
        fs::write(directory.path().join("generated/a.xml"), b"old-a").unwrap();
        let plan = ArtifactPlan::new(
            directory.path(),
            relative("generated"),
            "xml",
            RunScope::Full,
            vec![
                (relative("generated/a.xml"), b"a".to_vec()),
                (relative("generated/nested/b.xml"), b"b".to_vec()),
            ],
            Some(vec![
                relative("generated/a.xml"),
                relative("generated/nested/b.xml"),
            ]),
        )
        .unwrap();

        let error = plan.install_with_stage_failure(1).unwrap_err();
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_eq!(
            fs::read(directory.path().join("generated/a.xml")).unwrap(),
            b"old-a"
        );
        assert!(!directory.path().join("generated/nested").exists());
        assert!(
            !directory
                .path()
                .join("generated/.a.xml.surgeist-stage")
                .exists()
        );
    }

    #[test]
    fn construction_rejects_duplicates_scope_escapes_and_residual_stages() {
        let directory = TestDirectory::new("artifact-collisions");
        fs::create_dir(directory.path().join("generated")).unwrap();
        let path = relative("generated/a.xml");
        let duplicate = ArtifactPlan::new(
            directory.path(),
            relative("generated"),
            "xml",
            RunScope::Full,
            vec![(path.clone(), b"a".to_vec()), (path.clone(), b"b".to_vec())],
            Some(vec![path.clone()]),
        )
        .unwrap_err();
        assert_eq!(duplicate.kind(), GeneratorErrorKind::ArtifactTransaction);

        let outside = ArtifactPlan::new(
            directory.path(),
            relative("generated"),
            "xml",
            RunScope::Full,
            vec![(relative("other/a.xml"), b"a".to_vec())],
            Some(vec![relative("other/a.xml")]),
        )
        .unwrap_err();
        assert_eq!(outside.kind(), GeneratorErrorKind::InvalidPath);

        fs::write(
            directory.path().join("generated/.a.xml.surgeist-stage"),
            b"residual",
        )
        .unwrap();
        let residual = ArtifactPlan::new(
            directory.path(),
            relative("generated"),
            "xml",
            RunScope::Filtered(relative("source/a.html")),
            vec![(path, b"a".to_vec())],
            None,
        )
        .unwrap_err();
        assert_eq!(residual.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_eq!(
            fs::read(directory.path().join("generated/.a.xml.surgeist-stage")).unwrap(),
            b"residual"
        );
        fs::remove_file(directory.path().join("generated/.a.xml.surgeist-stage")).unwrap();
        fs::write(directory.path().join("generated/a.xml"), b"old").unwrap();
        fs::write(
            directory.path().join("generated/.a.xml.surgeist-backup"),
            b"residual-backup",
        )
        .unwrap();
        let backup = ArtifactPlan::new(
            directory.path(),
            relative("generated"),
            "xml",
            RunScope::Full,
            vec![(relative("generated/a.xml"), b"new".to_vec())],
            Some(vec![relative("generated/a.xml")]),
        )
        .unwrap_err();
        assert_eq!(backup.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_eq!(
            fs::read(directory.path().join("generated/.a.xml.surgeist-backup")).unwrap(),
            b"residual-backup"
        );
    }

    #[cfg(unix)]
    #[test]
    fn construction_rejects_symlinks_and_special_generated_entries() {
        use std::os::unix::fs::symlink;
        use std::os::unix::net::UnixListener;

        let directory = TestDirectory::new("artifact-entry-kinds");
        let outside = TestDirectory::new("artifact-outside");
        symlink(outside.path(), directory.path().join("generated")).unwrap();
        let symlink_error = ArtifactPlan::new(
            directory.path(),
            relative("generated"),
            "xml",
            RunScope::Filtered(relative("source/a.html")),
            vec![(relative("generated/a.xml"), b"a".to_vec())],
            None,
        )
        .unwrap_err();
        assert_eq!(symlink_error.kind(), GeneratorErrorKind::InvalidPath);
        fs::remove_file(directory.path().join("generated")).unwrap();

        fs::create_dir(directory.path().join("generated")).unwrap();
        let short_socket = std::env::temp_dir().join(format!(
            "sg-{}-{}.sock",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        let _listener = UnixListener::bind(&short_socket).unwrap();
        let socket_path = directory.path().join("generated/special.socket");
        fs::rename(short_socket, &socket_path).unwrap();
        let special_error = ArtifactPlan::new(
            directory.path(),
            relative("generated"),
            "xml",
            RunScope::Full,
            Vec::new(),
            Some(Vec::new()),
        )
        .unwrap_err();
        assert_eq!(
            special_error.kind(),
            GeneratorErrorKind::ArtifactTransaction
        );
    }

    #[cfg(unix)]
    #[test]
    fn artifact_install_rejects_root_and_component_swaps() {
        use std::os::unix::fs::symlink;

        let supplied = TestDirectory::new("artifact-root-supplied");
        let outside = TestDirectory::new("artifact-root-outside");
        let root_link = supplied.path().join("root-link");
        symlink(outside.path(), &root_link).unwrap();
        let root_error = ArtifactPlan::new(
            &root_link,
            relative("generated"),
            "xml",
            RunScope::Filtered(relative("source/a.html")),
            vec![(relative("generated/a.xml"), b"new".to_vec())],
            None,
        )
        .expect_err("a supplied output-root symlink must be rejected");
        assert_eq!(root_error.kind(), GeneratorErrorKind::InvalidPath);

        let replace_parent = TestDirectory::new("artifact-root-replacement");
        let replace_root = replace_parent.path().join("output");
        fs::create_dir(&replace_root).unwrap();
        let replace_plan = ArtifactPlan::new(
            &replace_root,
            relative("generated"),
            "xml",
            RunScope::Filtered(relative("source/a.html")),
            vec![(relative("generated/a.xml"), b"new".to_vec())],
            None,
        )
        .unwrap();
        fs::rename(&replace_root, replace_parent.path().join("output-original")).unwrap();
        symlink(outside.path(), &replace_root).unwrap();
        let replace_error = replace_plan
            .install()
            .expect_err("a replaced output root must be rejected at install time");
        assert_eq!(replace_error.kind(), GeneratorErrorKind::InvalidPath);
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 0);

        let directory = TestDirectory::new("artifact-component-swap");
        let outside = TestDirectory::new("artifact-component-swap-outside");
        fs::create_dir(directory.path().join("generated")).unwrap();
        let plan = ArtifactPlan::new(
            directory.path(),
            relative("generated"),
            "xml",
            RunScope::Filtered(relative("source/a.html")),
            vec![(relative("generated/a.xml"), b"new".to_vec())],
            None,
        )
        .unwrap();
        fs::rename(
            directory.path().join("generated"),
            directory.path().join("generated-original"),
        )
        .unwrap();
        symlink(outside.path(), directory.path().join("generated")).unwrap();

        let error = plan
            .install()
            .expect_err("an install-time component swap must be rejected");
        assert_eq!(error.kind(), GeneratorErrorKind::InvalidPath);
        assert!(!outside.path().join("a.xml").exists());

        let late_stale_root = TestDirectory::new("artifact-late-stale");
        fs::create_dir(late_stale_root.path().join("generated")).unwrap();
        let late_stale_plan = ArtifactPlan::new(
            late_stale_root.path(),
            relative("generated"),
            "xml",
            RunScope::Full,
            vec![(relative("generated/a.xml"), b"new".to_vec())],
            Some(vec![relative("generated/a.xml")]),
        )
        .unwrap();
        fs::write(
            late_stale_root
                .path()
                .join("generated/created-after-plan.xml"),
            b"late stale",
        )
        .unwrap();
        late_stale_plan.install().unwrap();
        assert!(
            !late_stale_root
                .path()
                .join("generated/created-after-plan.xml")
                .exists()
        );
    }

    #[test]
    fn artifact_cleanup_failure_restores_prior_tree() {
        let directory = TestDirectory::new("artifact-cleanup-rollback");
        fs::create_dir_all(directory.path().join("generated/empty")).unwrap();
        fs::write(directory.path().join("generated/a.xml"), b"old-a").unwrap();
        fs::write(directory.path().join("generated/stale.xml"), b"stale").unwrap();
        let prior_tree = snapshot_tree(directory.path());
        let plan = ArtifactPlan::new(
            directory.path(),
            relative("generated"),
            "xml",
            RunScope::Full,
            vec![(relative("generated/a.xml"), b"new-a".to_vec())],
            Some(vec![relative("generated/a.xml")]),
        )
        .unwrap();

        let error = plan
            .install_with_cleanup_failure(1)
            .expect_err("injected cleanup failure must report rollback");
        assert_eq!(error.kind(), GeneratorErrorKind::ArtifactTransaction);
        assert_eq!(
            fs::read(directory.path().join("generated/a.xml")).unwrap(),
            b"old-a"
        );
        assert_eq!(
            fs::read(directory.path().join("generated/stale.xml")).unwrap(),
            b"stale"
        );
        assert!(directory.path().join("generated/empty").is_dir());
        assert!(
            fs::read_dir(directory.path().join("generated"))
                .unwrap()
                .all(|entry| !entry
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .contains(".surgeist-"))
        );
        assert_eq!(snapshot_tree(directory.path()), prior_tree);
    }
}
