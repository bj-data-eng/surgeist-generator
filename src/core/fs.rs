#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use std::path::Component;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{CorpusLocation, GeneratorError, GeneratorErrorKind, Result};

pub(crate) const PRIVATE_DIRECTORY_MODE: u32 = 0o700;
pub(crate) const PRIVATE_FILE_MODE: u32 = 0o600;
pub(crate) const CORPUS_DIRECTORY_MODE: u32 = 0o755;
pub(crate) const CORPUS_FILE_MODE: u32 = 0o644;

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum DurabilityPhase {
    Rooted,
    FilePublication,
    TransactionInstall,
    TransactionStage,
    TransactionRecovery,
    TransactionCleanup,
    BootstrapInstall,
    BootstrapRecovery,
    BootstrapCleanup,
    OwnerInstall,
    OwnerRecovery,
    OwnerCleanup,
    ProbeInstall,
    ProbeRecovery,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum DurabilityPrimitive {
    CreateDirectory,
    CreateFile,
    WritePartial,
    WriteFull,
    SetPermissions,
    FlushFile,
    SyncFile,
    ValidateIdentity,
    DropHandle,
    RenameExclusive,
    RenameSwap,
    RemoveFile,
    RemoveDirectory,
    SyncDirectory,
}

#[cfg(test)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DurabilityEvent {
    phase: DurabilityPhase,
    primitive: DurabilityPrimitive,
    path: String,
    ordinal: usize,
}

#[cfg(test)]
impl DurabilityEvent {
    pub(crate) const fn phase(&self) -> DurabilityPhase {
        self.phase
    }

    pub(crate) const fn primitive(&self) -> DurabilityPrimitive {
        self.primitive
    }

    pub(crate) fn path(&self) -> &str {
        &self.path
    }

    pub(crate) const fn ordinal(&self) -> usize {
        self.ordinal
    }
}

#[cfg(test)]
#[derive(Debug)]
struct RootedObserverState {
    events: Vec<DurabilityEvent>,
    next_ordinals: std::collections::BTreeMap<DurabilityPhase, usize>,
    phase_stacks: std::collections::HashMap<std::thread::ThreadId, Vec<DurabilityPhase>>,
    interrupt_after: Option<usize>,
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub(crate) struct RootedObserver {
    state: std::sync::Arc<std::sync::Mutex<RootedObserverState>>,
}

#[cfg(test)]
impl RootedObserver {
    pub(crate) fn recording() -> Self {
        Self::with_interruption(None)
    }

    /// Arms the observer for the zero-based global event index.
    pub(crate) fn interrupt_after(event_index: usize) -> Self {
        Self::with_interruption(Some(event_index))
    }

    fn with_interruption(interrupt_after: Option<usize>) -> Self {
        Self {
            state: std::sync::Arc::new(std::sync::Mutex::new(RootedObserverState {
                events: Vec::new(),
                next_ordinals: std::collections::BTreeMap::new(),
                phase_stacks: std::collections::HashMap::new(),
                interrupt_after,
            })),
        }
    }

    pub(crate) fn events(&self) -> Vec<DurabilityEvent> {
        self.lock_state().events.clone()
    }

    pub(crate) fn is_interruption(payload: &(dyn std::any::Any + Send)) -> bool {
        payload.is::<RootedInterruption>()
    }

    fn lock_state(&self) -> std::sync::MutexGuard<'_, RootedObserverState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn enter_phase(&self, phase: DurabilityPhase, only_if_unset: bool) -> ObservationPhaseGuard {
        let thread = std::thread::current().id();
        let pushed = {
            let mut state = self.lock_state();
            let stack = state.phase_stacks.entry(thread).or_default();
            if only_if_unset && !stack.is_empty() {
                false
            } else {
                stack.push(phase);
                true
            }
        };
        ObservationPhaseGuard {
            observer: Some(self.clone()),
            thread,
            phase,
            pushed,
        }
    }

    fn record(&self, primitive: DurabilityPrimitive, path: &str) -> bool {
        let thread = std::thread::current().id();
        let mut state = self.lock_state();
        let phase = state
            .phase_stacks
            .get(&thread)
            .and_then(|stack| stack.last())
            .copied()
            .unwrap_or(DurabilityPhase::Rooted);
        let ordinal = {
            let next = state.next_ordinals.entry(phase).or_default();
            let ordinal = *next;
            *next += 1;
            ordinal
        };
        let event_index = state.events.len();
        state.events.push(DurabilityEvent {
            phase,
            primitive,
            path: path.to_owned(),
            ordinal,
        });
        state.interrupt_after == Some(event_index)
    }
}

#[cfg(test)]
#[derive(Debug)]
pub(crate) struct ObservationPhaseGuard {
    observer: Option<RootedObserver>,
    thread: std::thread::ThreadId,
    phase: DurabilityPhase,
    pushed: bool,
}

#[cfg(test)]
impl ObservationPhaseGuard {
    fn inactive(phase: DurabilityPhase) -> Self {
        Self {
            observer: None,
            thread: std::thread::current().id(),
            phase,
            pushed: false,
        }
    }
}

#[cfg(test)]
impl Drop for ObservationPhaseGuard {
    fn drop(&mut self) {
        if !self.pushed {
            return;
        }
        let observer = self
            .observer
            .as_ref()
            .expect("an active phase guard has an observer");
        let mut state = observer.lock_state();
        let stack = state
            .phase_stacks
            .get_mut(&self.thread)
            .expect("an active phase guard has a thread stack");
        assert_eq!(
            stack.pop(),
            Some(self.phase),
            "rooted observation phases must unwind in stack order"
        );
        if stack.is_empty() {
            state.phase_stacks.remove(&self.thread);
        }
    }
}

#[cfg(test)]
#[derive(Debug)]
struct RootedInterruption;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum MutationTarget {
    AppleSiliconMacOs,
    Unsupported,
}

impl MutationTarget {
    pub(crate) const fn current() -> Self {
        if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
            Self::AppleSiliconMacOs
        } else {
            Self::Unsupported
        }
    }

    pub(crate) fn require_supported(self, operation: &str) -> Result<()> {
        match self {
            Self::AppleSiliconMacOs => Ok(()),
            Self::Unsupported => Err(GeneratorError::new(
                GeneratorErrorKind::UnsupportedPlatform,
                operation,
                "filesystem mutation requires aarch64-apple-darwin",
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum NodeKind {
    Directory,
    Regular,
    Symlink,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct HeldIdentity {
    kind: NodeKind,
    device: i64,
    inode: u64,
    fsid: FilesystemId,
    mode: u32,
    owner: u32,
    link_count: Option<u64>,
}

impl HeldIdentity {
    pub(crate) const fn kind(&self) -> NodeKind {
        self.kind
    }

    pub(crate) const fn device(&self) -> i64 {
        self.device
    }

    pub(crate) const fn inode(&self) -> u64 {
        self.inode
    }

    pub(crate) const fn fsid(&self) -> &FilesystemId {
        &self.fsid
    }

    pub(crate) const fn mode(&self) -> u32 {
        self.mode
    }

    pub(crate) const fn owner(&self) -> u32 {
        self.owner
    }

    pub(crate) const fn link_count(&self) -> Option<u64> {
        self.link_count
    }

    pub(crate) fn same_object(&self, other: &Self) -> bool {
        self.kind == other.kind
            && self.device == other.device
            && self.inode == other.inode
            && self.fsid == other.fsid
    }

    pub(crate) fn matches_recovery(&self, other: &Self) -> bool {
        self.same_object(other)
            && self.mode == other.mode
            && self.owner == other.owner
            && (self.kind == NodeKind::Directory || self.link_count == other.link_count)
    }

    #[cfg(test)]
    pub(crate) const fn synthetic(
        kind: NodeKind,
        inode: u64,
        mode: u32,
        link_count: Option<u64>,
    ) -> Self {
        Self {
            kind,
            device: 1,
            inode,
            fsid: FilesystemId { words: [0, 2] },
            mode,
            owner: 3,
            link_count,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FilesystemId {
    words: [i32; 2],
}

impl std::fmt::Display for FilesystemId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}:{}", self.words[0], self.words[1])
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BoundComponent {
    name: Option<String>,
    path: PathBuf,
    identity: HeldIdentity,
}

/// A read-only binding of an absolute namespace to its nearest existing object.
///
/// Supported mutation hosts retain descriptors for every existing ancestor. An
/// absent suffix remains symbolic as exact UTF-8 components until closing
/// revalidation proves that the namespace is still separated from protected
/// authorities.
pub(crate) struct BoundPath {
    requested: PathBuf,
    canonical_path: PathBuf,
    remaining: Vec<String>,
    components: Vec<BoundComponent>,
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    descriptors: Vec<rustix::fd::OwnedFd>,
}

impl std::fmt::Debug for BoundPath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("BoundPath")
            .field("requested", &self.requested)
            .field("canonical_path", &self.canonical_path)
            .field("remaining", &self.remaining)
            .field("components", &self.components)
            .finish_non_exhaustive()
    }
}

impl BoundPath {
    pub(crate) fn bind(path: &Path) -> Result<Self> {
        MutationTarget::current().require_supported("bind protected namespace path")?;
        bind_path(path)
    }

    pub(crate) fn require_existing_directory(&self, operation: &str) -> Result<()> {
        if !self.remaining.is_empty()
            || self
                .components
                .last()
                .is_none_or(|component| component.identity.kind() != NodeKind::Directory)
        {
            return Err(invalid_path(
                operation,
                format!(
                    "namespace is not an existing directory: {}",
                    self.requested.display()
                ),
            ));
        }
        Ok(())
    }

    pub(crate) fn canonical_path(&self) -> &Path {
        &self.canonical_path
    }

    pub(crate) fn existing_identity(&self) -> &HeldIdentity {
        &self
            .components
            .last()
            .expect("a bound absolute path always contains the filesystem root")
            .identity
    }

    pub(crate) fn revalidate(&self) -> Result<()> {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        for (component, descriptor) in self.components.iter().zip(&self.descriptors) {
            let current = identity_from_fd(descriptor, "revalidate held namespace descriptor")
                .map_err(|error| {
                    GeneratorError::with_source(
                        GeneratorErrorKind::InvalidPath,
                        "revalidate protected namespace",
                        component.path.display().to_string(),
                        error,
                    )
                })?;
            if !component.identity.matches_recovery(&current) {
                return Err(invalid_path(
                    "revalidate protected namespace",
                    format!("held identity changed: {}", component.path.display()),
                ));
            }
        }

        let current = Self::bind(&self.requested)?;
        if !self.same_binding(&current) {
            return Err(invalid_path(
                "revalidate protected namespace",
                format!(
                    "path identity or absent suffix changed: {}",
                    self.requested.display()
                ),
            ));
        }
        Ok(())
    }

    /// Duplicates the exact held regular-file descriptor without resolving its path again.
    #[cfg(feature = "layout-browser")]
    pub(crate) fn held_regular_file(&self) -> Result<std::fs::File> {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            let identity = self.existing_identity();
            if !self.remaining.is_empty()
                || identity.kind() != NodeKind::Regular
                || identity.link_count() != Some(1)
            {
                return Err(invalid_path(
                    "open held protected regular file",
                    format!(
                        "path is not a single-link regular file: {}",
                        self.requested.display()
                    ),
                ));
            }
            self.revalidate()?;
            let descriptor = self.descriptors.last().ok_or_else(|| {
                invalid_path(
                    "open held protected regular file",
                    format!(
                        "path has no retained descriptor: {}",
                        self.requested.display()
                    ),
                )
            })?;
            let duplicate = rustix::io::dup(descriptor).map_err(|source| {
                namespace_io_error(
                    "duplicate held protected regular file",
                    &self.requested,
                    source,
                )
            })?;
            Ok(std::fs::File::from(duplicate))
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            MutationTarget::Unsupported.require_supported("open held protected regular file")?;
            unreachable!("unsupported mutation target returned success")
        }
    }

    pub(crate) fn overlaps(&self, other: &Self) -> Result<bool> {
        if self.canonical_path == other.canonical_path
            || self.canonical_path.starts_with(&other.canonical_path)
            || other.canonical_path.starts_with(&self.canonical_path)
        {
            return Ok(true);
        }
        Ok(self.descriptor_ancestor_of(other)? || other.descriptor_ancestor_of(self)?)
    }

    fn same_binding(&self, other: &Self) -> bool {
        self.canonical_path == other.canonical_path
            && self.remaining == other.remaining
            && self.components == other.components
    }

    fn descriptor_ancestor_of(&self, other: &Self) -> Result<bool> {
        let existing = self.existing_identity();
        if self.remaining.is_empty()
            && other
                .components
                .iter()
                .any(|component| existing.same_object(&component.identity))
        {
            return Ok(true);
        }

        let Some(anchor_index) = other
            .components
            .iter()
            .rposition(|component| existing.same_object(&component.identity))
        else {
            return Ok(false);
        };
        if self.remaining.is_empty() {
            return Ok(true);
        }

        let mut other_tail = other.components[anchor_index + 1..]
            .iter()
            .map(|component| {
                component.name.as_deref().ok_or_else(|| {
                    invalid_path(
                        "compare namespace ancestry",
                        "filesystem root unexpectedly appeared inside an ancestry tail",
                    )
                })
            })
            .collect::<Result<Vec<_>>>()?;
        other_tail.extend(other.remaining.iter().map(String::as_str));
        component_prefix(&self.remaining, &other_tail)
    }
}

fn component_prefix(prefix: &[String], value: &[&str]) -> Result<bool> {
    for (left, right) in prefix.iter().map(String::as_str).zip(value.iter().copied()) {
        if left == right {
            continue;
        }
        if left.is_ascii() && right.is_ascii() && !left.eq_ignore_ascii_case(right) {
            return Ok(false);
        }
        return Err(invalid_path(
            "compare absent namespace suffixes",
            format!("cannot prove distinct path components: {left:?} and {right:?}"),
        ));
    }
    Ok(prefix.len() <= value.len())
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn bind_path(path: &Path) -> Result<BoundPath> {
    use rustix::fs::{AtFlags, FileType, Mode, OFlags, open, openat, statat};

    validate_absolute_namespace_path(path)?;
    let mut nearest = path.to_path_buf();
    let mut remaining = Vec::new();
    loop {
        match std::fs::symlink_metadata(&nearest) {
            Ok(_) => break,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) =>
            {
                let name = nearest
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| {
                        invalid_path(
                            "bind protected namespace path",
                            format!("cannot represent absent suffix: {}", path.display()),
                        )
                    })?;
                remaining.insert(0, name.to_owned());
                if !nearest.pop() {
                    return Err(invalid_path(
                        "bind protected namespace path",
                        format!("cannot resolve an existing ancestor: {}", path.display()),
                    ));
                }
            }
            Err(error) => {
                return Err(namespace_io_error(
                    "inspect protected namespace ancestor",
                    &nearest,
                    error,
                ));
            }
        }
    }

    let canonical_existing = std::fs::canonicalize(&nearest).map_err(|error| {
        namespace_io_error("canonicalize protected namespace ancestor", &nearest, error)
    })?;
    validate_absolute_namespace_path(&canonical_existing)?;
    if !remaining.is_empty() && !canonical_existing.is_dir() {
        return Err(invalid_path(
            "bind protected namespace path",
            format!(
                "absent suffix has a nondirectory ancestor: {}",
                canonical_existing.display()
            ),
        ));
    }

    let root = open(
        "/",
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|source| {
        namespace_io_error("open namespace filesystem root", Path::new("/"), source)
    })?;
    let root_identity = identity_from_fd(&root, "inspect namespace filesystem root")
        .map_err(|error| invalid_path("bind protected namespace path", error))?;
    let mut descriptors = vec![root];
    let mut components = vec![BoundComponent {
        name: None,
        path: PathBuf::from("/"),
        identity: root_identity,
    }];
    let names = canonical_existing
        .components()
        .filter_map(|component| match component {
            Component::Normal(name) => Some(name.to_str().map(str::to_owned).ok_or_else(|| {
                invalid_path(
                    "bind protected namespace path",
                    format!(
                        "non-UTF-8 canonical component: {}",
                        canonical_existing.display()
                    ),
                )
            })),
            Component::RootDir => None,
            _ => Some(Err(invalid_path(
                "bind protected namespace path",
                format!(
                    "nonnormal canonical component: {}",
                    canonical_existing.display()
                ),
            ))),
        })
        .collect::<Result<Vec<_>>>()?;
    let mut current_path = PathBuf::from("/");
    for (index, name) in names.iter().enumerate() {
        let parent = descriptors
            .last()
            .expect("the namespace filesystem root descriptor is retained");
        require_exact_entry_name(parent, name)
            .map_err(|error| invalid_path("bind protected namespace path", error))?;
        let stat = statat(parent, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|source| {
            namespace_io_error(
                "inspect protected namespace component",
                &current_path.join(name),
                source,
            )
        })?;
        let kind = FileType::from_raw_mode(stat.st_mode);
        let directory_required = index + 1 != names.len();
        let flags = if kind == FileType::Directory {
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW
        } else if kind == FileType::RegularFile && !directory_required {
            OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW
        } else {
            return Err(invalid_path(
                "bind protected namespace path",
                format!(
                    "symlink, special object, or nondirectory ancestor is not allowed: {}",
                    current_path.join(name).display()
                ),
            ));
        };
        let opened = openat(parent, name, flags, Mode::empty()).map_err(|source| {
            namespace_io_error(
                "open protected namespace component",
                &current_path.join(name),
                source,
            )
        })?;
        let identity = identity_from_fd(&opened, "inspect held namespace component")
            .map_err(|error| invalid_path("bind protected namespace path", error))?;
        current_path.push(name);
        components.push(BoundComponent {
            name: Some(name.clone()),
            path: current_path.clone(),
            identity,
        });
        descriptors.push(opened);
    }

    let canonical_path = remaining
        .iter()
        .fold(canonical_existing, |mut current, component| {
            current.push(component);
            current
        });
    Ok(BoundPath {
        requested: path.to_path_buf(),
        canonical_path,
        remaining,
        components,
        descriptors,
    })
}

#[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
fn bind_path(_path: &Path) -> Result<BoundPath> {
    MutationTarget::Unsupported.require_supported("bind protected namespace path")?;
    unreachable!("unsupported mutation target returned success")
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn validate_absolute_namespace_path(path: &Path) -> Result<()> {
    let rendered = path.to_str().ok_or_else(|| {
        invalid_path(
            "validate protected namespace path",
            "namespace contains a non-UTF-8 component",
        )
    })?;
    if !path.is_absolute()
        || rendered.contains('\0')
        || rendered.contains('\\')
        || (rendered != "/" && rendered.ends_with('/'))
        || rendered.contains("//")
        || rendered
            .split('/')
            .skip(1)
            .any(|component| matches!(component, "" | "." | ".."))
        || path
            .components()
            .any(|component| !matches!(component, Component::RootDir | Component::Normal(_)))
    {
        return Err(invalid_path(
            "validate protected namespace path",
            format!("namespace is not a normalized absolute path: {rendered}"),
        ));
    }
    Ok(())
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn namespace_io_error<E>(operation: &str, path: &Path, source: E) -> GeneratorError
where
    E: std::error::Error + Send + Sync + 'static,
{
    GeneratorError::with_source(
        GeneratorErrorKind::InvalidPath,
        operation,
        path.display().to_string(),
        source,
    )
}

pub(crate) struct RootedFs {
    canonical_root: PathBuf,
    identity: HeldIdentity,
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    root: rustix::fd::OwnedFd,
    #[cfg(test)]
    observer: Option<RootedObserver>,
}

impl std::fmt::Debug for RootedFs {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RootedFs")
            .field("canonical_root", &self.canonical_root)
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}

impl RootedFs {
    pub(crate) fn open_corpus(location: &CorpusLocation) -> Result<Self> {
        MutationTarget::current().require_supported("open rooted corpus authority")?;
        Self::open_canonical(location.corpus_root())
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn open_canonical(path: &Path) -> Result<Self> {
        use rustix::fs::{Mode, OFlags, open};

        if !path.is_absolute() {
            return Err(invalid_path(
                "open rooted filesystem authority",
                format!("root is not absolute: {}", path.display()),
            ));
        }
        let mut current = open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(|source| io_error("open filesystem root", Path::new("/"), source))?;
        let mut canonical = PathBuf::from("/");
        for component in path.components() {
            match component {
                Component::RootDir => {}
                Component::Normal(name) => {
                    let name = name.to_str().ok_or_else(|| {
                        invalid_path(
                            "open rooted filesystem authority",
                            format!("non-UTF-8 component in {}", path.display()),
                        )
                    })?;
                    require_exact_entry_name(&current, name)?;
                    current = open_directory_at(&current, name, "open rooted component")?;
                    canonical.push(name);
                }
                _ => {
                    return Err(invalid_path(
                        "open rooted filesystem authority",
                        format!("noncanonical root: {}", path.display()),
                    ));
                }
            }
        }
        let identity = identity_from_fd(&current, "inspect rooted filesystem authority")?;
        require_directory_policy(&identity, None, "validate rooted filesystem authority")?;
        Ok(Self {
            canonical_root: canonical,
            identity,
            root: current,
            #[cfg(test)]
            observer: None,
        })
    }

    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    fn open_canonical(_path: &Path) -> Result<Self> {
        MutationTarget::Unsupported.require_supported("open rooted filesystem authority")?;
        unreachable!("unsupported mutation target returned success")
    }

    pub(crate) fn canonical_root(&self) -> &Path {
        &self.canonical_root
    }

    #[cfg(test)]
    pub(crate) fn open_corpus_observed(
        location: &CorpusLocation,
        observer: RootedObserver,
    ) -> Result<Self> {
        let mut rooted = Self::open_corpus(location)?;
        rooted.observer = Some(observer);
        Ok(rooted)
    }

    #[cfg(test)]
    pub(crate) fn begin_observation_phase(&self, phase: DurabilityPhase) -> ObservationPhaseGuard {
        self.observer.as_ref().map_or_else(
            || ObservationPhaseGuard::inactive(phase),
            |observer| observer.enter_phase(phase, false),
        )
    }

    #[cfg(test)]
    fn begin_default_observation_phase(&self, phase: DurabilityPhase) -> ObservationPhaseGuard {
        self.observer.as_ref().map_or_else(
            || ObservationPhaseGuard::inactive(phase),
            |observer| observer.enter_phase(phase, true),
        )
    }

    #[cfg(test)]
    fn record_durability(&self, primitive: DurabilityPrimitive, relative: &str) {
        let Some(observer) = &self.observer else {
            return;
        };
        assert!(
            strict_observation_path(relative),
            "durability events require a strict rooted path: {relative}"
        );
        if observer.record(primitive, relative) {
            std::panic::panic_any(RootedInterruption);
        }
    }

    pub(crate) fn identity(&self) -> &HeldIdentity {
        &self.identity
    }

    pub(crate) fn revalidate_root(&self) -> Result<()> {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            let actual = identity_from_fd(&self.root, "revalidate rooted authority")?;
            if !self.identity.matches_recovery(&actual) {
                return Err(transaction_error(
                    "revalidate rooted authority",
                    format!("root identity changed: {}", self.canonical_root.display()),
                ));
            }
            let named = Self::open_canonical(&self.canonical_root)?;
            if !self.identity.matches_recovery(&named.identity) {
                return Err(transaction_error(
                    "revalidate rooted authority",
                    format!(
                        "canonical root name now resolves to a different object: {}",
                        self.canonical_root.display()
                    ),
                ));
            }
            Ok(())
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            MutationTarget::Unsupported.require_supported("revalidate rooted authority")
        }
    }

    pub(crate) fn ensure_dir(&self, relative: &str, mode: u32) -> Result<HeldIdentity> {
        #[cfg(test)]
        let _phase = self.begin_default_observation_phase(DurabilityPhase::Rooted);
        self.revalidate_root()?;
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            use rustix::fs::{AtFlags, fchmod, fsync, mkdirat, statat};
            use rustix::io::Errno;

            let components = checked_components(relative)?;
            let mut current = rustix::io::dup(&self.root).map_err(|source| {
                io_error("duplicate rooted authority", &self.canonical_root, source)
            })?;
            let mut current_path = String::new();
            for component in components {
                if !current_path.is_empty() {
                    current_path.push('/');
                }
                current_path.push_str(component);
                let created = match statat(&current, component, AtFlags::SYMLINK_NOFOLLOW) {
                    Ok(_) => {
                        require_exact_entry_name(&current, component)?;
                        false
                    }
                    Err(Errno::NOENT) => {
                        match mkdirat(&current, component, checked_mode(PRIVATE_DIRECTORY_MODE)?) {
                            Ok(()) => {
                                #[cfg(test)]
                                self.record_durability(
                                    DurabilityPrimitive::CreateDirectory,
                                    &current_path,
                                );
                                true
                            }
                            Err(Errno::EXIST) => {
                                require_exact_entry_name(&current, component)?;
                                false
                            }
                            Err(source) => {
                                return Err(io_error(
                                    "create rooted directory",
                                    &self.canonical_root.join(&current_path),
                                    source,
                                ));
                            }
                        }
                    }
                    Err(source) => {
                        return Err(io_error(
                            "inspect rooted directory",
                            &self.canonical_root.join(&current_path),
                            source,
                        ));
                    }
                };
                if created {
                    fsync(&current).map_err(|source| {
                        io_error(
                            "sync rooted directory parent",
                            &self.canonical_root.join(&current_path),
                            source,
                        )
                    })?;
                    #[cfg(test)]
                    self.record_durability(
                        DurabilityPrimitive::SyncDirectory,
                        observation_parent(&current_path),
                    );
                }
                let child = open_directory_at(&current, component, "open rooted directory")?;
                let mut identity = identity_from_fd(&child, "inspect rooted directory")?;
                require_same_mount(&self.identity, &identity, "validate rooted directory mount")?;
                require_directory_policy(&identity, None, "validate rooted directory")?;
                if created && identity.mode != mode {
                    fchmod(&child, checked_mode(mode)?).map_err(|source| {
                        io_error(
                            "set rooted directory mode",
                            &self.canonical_root.join(&current_path),
                            source,
                        )
                    })?;
                    #[cfg(test)]
                    self.record_durability(DurabilityPrimitive::SetPermissions, &current_path);
                    fsync(&child).map_err(|source| {
                        io_error(
                            "sync rooted directory",
                            &self.canonical_root.join(&current_path),
                            source,
                        )
                    })?;
                    #[cfg(test)]
                    self.record_durability(DurabilityPrimitive::SyncDirectory, &current_path);
                    identity = identity_from_fd(&child, "reinspect rooted directory")?;
                }
                if identity.mode != mode {
                    return Err(transaction_error(
                        "validate rooted directory mode",
                        format!(
                            "expected {mode:#o}, got {:#o}: {}",
                            identity.mode,
                            self.canonical_root.join(&current_path).display()
                        ),
                    ));
                }
                current = child;
            }
            identity_from_fd(&current, "inspect final rooted directory")
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = (relative, mode);
            MutationTarget::Unsupported.require_supported("create rooted directory")?;
            unreachable!("unsupported mutation target returned success")
        }
    }

    pub(crate) fn create_dir_exclusive(
        &self,
        relative: &str,
        final_mode: u32,
    ) -> Result<HeldIdentity> {
        #[cfg(test)]
        let _phase = self.begin_default_observation_phase(DurabilityPhase::Rooted);
        self.revalidate_root()?;
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            use rustix::fs::{fchmod, fsync, mkdirat};

            let (parent, name) = self.open_parent(relative)?;
            mkdirat(&parent, name, checked_mode(PRIVATE_DIRECTORY_MODE)?).map_err(|source| {
                io_error(
                    "create exclusive rooted directory",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            #[cfg(test)]
            self.record_durability(DurabilityPrimitive::CreateDirectory, relative);
            let directory = open_directory_at(&parent, name, "open exclusive rooted directory")?;
            fchmod(&directory, checked_mode(final_mode)?).map_err(|source| {
                io_error(
                    "set exclusive rooted directory mode",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            #[cfg(test)]
            self.record_durability(DurabilityPrimitive::SetPermissions, relative);
            fsync(&directory).map_err(|source| {
                io_error(
                    "sync exclusive rooted directory",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            #[cfg(test)]
            self.record_durability(DurabilityPrimitive::SyncDirectory, relative);
            let identity = identity_from_fd(&directory, "inspect exclusive rooted directory")?;
            require_same_mount(
                &self.identity,
                &identity,
                "validate exclusive directory mount",
            )?;
            require_directory_policy(
                &identity,
                Some(final_mode),
                "validate exclusive rooted directory",
            )?;
            #[cfg(test)]
            self.record_durability(DurabilityPrimitive::ValidateIdentity, relative);
            fsync(&parent).map_err(|source| {
                io_error(
                    "sync exclusive rooted directory parent",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            #[cfg(test)]
            self.record_durability(
                DurabilityPrimitive::SyncDirectory,
                observation_parent(relative),
            );
            Ok(identity)
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = (relative, final_mode);
            MutationTarget::Unsupported.require_supported("create exclusive rooted directory")?;
            unreachable!("unsupported mutation target returned success")
        }
    }

    pub(crate) fn list_dir(&self, relative: &str) -> Result<Vec<String>> {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            let directory = self.open_dir(relative)?;
            list_names(&directory, &self.canonical_root.join(relative))
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = relative;
            MutationTarget::Unsupported.require_supported("list rooted directory")?;
            unreachable!("unsupported mutation target returned success")
        }
    }

    pub(crate) fn exists(&self, relative: &str) -> Result<bool> {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            let components = checked_components(relative)?;
            let mut current = rustix::io::dup(&self.root).map_err(|source| {
                io_error("duplicate rooted authority", &self.canonical_root, source)
            })?;
            let mut current_path = PathBuf::new();
            for (index, component) in components.iter().enumerate() {
                current_path.push(component);
                let Some(child) = open_existing_component(
                    &current,
                    component,
                    &self.identity,
                    &self.canonical_root.join(&current_path),
                    index + 1 != components.len(),
                )?
                else {
                    return Ok(false);
                };
                current = child;
            }
            Ok(true)
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = relative;
            MutationTarget::Unsupported.require_supported("inspect rooted path")?;
            unreachable!("unsupported mutation target returned success")
        }
    }

    pub(crate) fn identity_at(&self, relative: &str) -> Result<Option<HeldIdentity>> {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            let (parent, name) = self.open_parent(relative)?;
            identity_at_held_parent(
                &parent,
                name,
                &self.identity,
                &self.canonical_root.join(relative),
            )
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = relative;
            MutationTarget::Unsupported.require_supported("inspect rooted object")?;
            unreachable!("unsupported mutation target returned success")
        }
    }

    pub(crate) fn read_file(&self, relative: &str, expected_mode: u32) -> Result<Vec<u8>> {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            use rustix::fs::{Mode, OFlags, openat};
            use std::fs::File;
            use std::io::Read;

            let (parent, name) = self.open_parent(relative)?;
            require_exact_entry_name(&parent, name)?;
            let opened = openat(
                &parent,
                name,
                OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
            )
            .map_err(|source| {
                io_error(
                    "open rooted regular file",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            let before = identity_from_fd(&opened, "inspect rooted regular file")?;
            require_regular_policy(&before, expected_mode, "validate rooted regular file")?;
            require_same_mount(
                &self.identity,
                &before,
                "validate rooted regular file mount",
            )?;
            let mut file = File::from(opened);
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).map_err(|source| {
                io_error(
                    "read rooted regular file",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            let after = identity_from_fd(&file, "reinspect rooted regular file")?;
            if !before.matches_recovery(&after) {
                return Err(transaction_error(
                    "reinspect rooted regular file",
                    format!(
                        "identity changed: {}",
                        self.canonical_root.join(relative).display()
                    ),
                ));
            }
            Ok(bytes)
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = (relative, expected_mode);
            MutationTarget::Unsupported.require_supported("read rooted regular file")?;
            unreachable!("unsupported mutation target returned success")
        }
    }

    pub(crate) fn create_file_exclusive(
        &self,
        relative: &str,
        bytes: &[u8],
        final_mode: u32,
    ) -> Result<HeldIdentity> {
        #[cfg(test)]
        let _phase = self.begin_default_observation_phase(DurabilityPhase::Rooted);
        self.revalidate_root()?;
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            use rustix::fs::{OFlags, fchmod, fsync, openat};
            use std::fs::File;

            let (parent, name) = self.open_parent(relative)?;
            let opened = openat(
                &parent,
                name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                checked_mode(PRIVATE_FILE_MODE)?,
            )
            .map_err(|source| {
                io_error(
                    "create rooted regular file",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            #[cfg(test)]
            self.record_durability(DurabilityPrimitive::CreateFile, relative);
            let mut file = File::from(opened);
            self.write_file_handle_all(relative, &mut file, bytes)
                .map_err(|source| {
                    io_error(
                        "write rooted regular file",
                        &self.canonical_root.join(relative),
                        source,
                    )
                })?;
            fchmod(&file, checked_mode(final_mode)?).map_err(|source| {
                io_error(
                    "set rooted regular file mode",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            #[cfg(test)]
            self.record_durability(DurabilityPrimitive::SetPermissions, relative);
            self.flush_file_handle(relative, &mut file)
                .map_err(|source| {
                    io_error(
                        "flush rooted regular file",
                        &self.canonical_root.join(relative),
                        source,
                    )
                })?;
            fsync(&file).map_err(|source| {
                io_error(
                    "sync rooted regular file",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            #[cfg(test)]
            self.record_durability(DurabilityPrimitive::SyncFile, relative);
            let identity = identity_from_fd(&file, "inspect created rooted regular file")?;
            require_regular_policy(
                &identity,
                final_mode,
                "validate created rooted regular file",
            )?;
            require_same_mount(&self.identity, &identity, "validate created file mount")?;
            #[cfg(test)]
            self.record_durability(DurabilityPrimitive::ValidateIdentity, relative);
            fsync(&parent).map_err(|source| {
                io_error(
                    "sync rooted regular file parent",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            #[cfg(test)]
            self.record_durability(
                DurabilityPrimitive::SyncDirectory,
                observation_parent(relative),
            );
            Ok(identity)
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = (relative, bytes, final_mode);
            MutationTarget::Unsupported.require_supported("create rooted regular file")?;
            unreachable!("unsupported mutation target returned success")
        }
    }

    pub(crate) fn create_file_handle_exclusive(
        &self,
        relative: &str,
        bytes: &[u8],
        final_mode: u32,
    ) -> Result<std::fs::File> {
        #[cfg(test)]
        let _phase = self.begin_default_observation_phase(DurabilityPhase::Rooted);
        self.revalidate_root()?;
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            use rustix::fs::{OFlags, fchmod, fsync, openat};

            let (parent, name) = self.open_parent(relative)?;
            let opened = openat(
                &parent,
                name,
                OFlags::RDWR | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                checked_mode(PRIVATE_FILE_MODE)?,
            )
            .map_err(|source| {
                io_error(
                    "create rooted file handle",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            #[cfg(test)]
            self.record_durability(DurabilityPrimitive::CreateFile, relative);
            let mut file = std::fs::File::from(opened);
            self.write_file_handle_all(relative, &mut file, bytes)
                .map_err(|source| {
                    io_error(
                        "write rooted file handle",
                        &self.canonical_root.join(relative),
                        source,
                    )
                })?;
            fchmod(&file, checked_mode(final_mode)?).map_err(|source| {
                io_error(
                    "set rooted file-handle mode",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            #[cfg(test)]
            self.record_durability(DurabilityPrimitive::SetPermissions, relative);
            self.flush_file_handle(relative, &mut file)
                .map_err(|source| {
                    io_error(
                        "flush rooted file handle",
                        &self.canonical_root.join(relative),
                        source,
                    )
                })?;
            fsync(&file).map_err(|source| {
                io_error(
                    "sync rooted file handle",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            #[cfg(test)]
            self.record_durability(DurabilityPrimitive::SyncFile, relative);
            let identity = identity_from_fd(&file, "inspect created rooted file handle")?;
            require_regular_policy(&identity, final_mode, "validate created rooted file handle")?;
            require_same_mount(
                &self.identity,
                &identity,
                "validate rooted file-handle mount",
            )?;
            #[cfg(test)]
            self.record_durability(DurabilityPrimitive::ValidateIdentity, relative);
            fsync(&parent).map_err(|source| {
                io_error(
                    "sync rooted file-handle parent",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            #[cfg(test)]
            self.record_durability(
                DurabilityPrimitive::SyncDirectory,
                observation_parent(relative),
            );
            Ok(file)
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = (relative, bytes, final_mode);
            MutationTarget::Unsupported.require_supported("create rooted file handle")?;
            unreachable!("unsupported mutation target returned success")
        }
    }

    pub(crate) fn open_file_handle(
        &self,
        relative: &str,
        expected_mode: u32,
        writable: bool,
    ) -> Result<std::fs::File> {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            use rustix::fs::{Mode, OFlags, openat};

            let (parent, name) = self.open_parent(relative)?;
            require_exact_entry_name(&parent, name)?;
            let access = if writable {
                OFlags::RDWR
            } else {
                OFlags::RDONLY
            };
            let opened = openat(
                &parent,
                name,
                access | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
            )
            .map_err(|source| {
                io_error(
                    "open rooted file handle",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            let file = std::fs::File::from(opened);
            let identity = identity_from_fd(&file, "inspect opened rooted file handle")?;
            require_regular_policy(&identity, expected_mode, "validate rooted file handle")?;
            require_same_mount(
                &self.identity,
                &identity,
                "validate rooted file-handle mount",
            )?;
            Ok(file)
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = (relative, expected_mode, writable);
            MutationTarget::Unsupported.require_supported("open rooted file handle")?;
            unreachable!("unsupported mutation target returned success")
        }
    }

    pub(crate) fn write_file_handle_all(
        &self,
        relative: &str,
        file: &mut std::fs::File,
        bytes: &[u8],
    ) -> std::io::Result<()> {
        use std::io::Write;

        #[cfg(not(test))]
        let _ = relative;
        #[cfg(test)]
        if self.observer.is_some() {
            for (index, byte) in bytes.iter().enumerate() {
                file.write_all(std::slice::from_ref(byte))?;
                let primitive = if index + 1 == bytes.len() {
                    DurabilityPrimitive::WriteFull
                } else {
                    DurabilityPrimitive::WritePartial
                };
                self.record_durability(primitive, relative);
            }
            return Ok(());
        }

        file.write_all(bytes)
    }

    pub(crate) fn flush_file_handle(
        &self,
        relative: &str,
        file: &mut std::fs::File,
    ) -> std::io::Result<()> {
        use std::io::Write;

        #[cfg(not(test))]
        let _ = relative;
        file.flush()?;
        #[cfg(test)]
        self.record_durability(DurabilityPrimitive::FlushFile, relative);
        Ok(())
    }

    pub(crate) fn sync_file_handle(
        &self,
        relative: &str,
        file: &std::fs::File,
    ) -> std::io::Result<()> {
        #[cfg(not(test))]
        let _ = relative;
        file.sync_all()?;
        #[cfg(test)]
        self.record_durability(DurabilityPrimitive::SyncFile, relative);
        Ok(())
    }

    pub(crate) fn drop_file_handle(&self, relative: &str, file: std::fs::File) {
        #[cfg(not(test))]
        let _ = relative;
        drop(file);
        #[cfg(test)]
        self.record_durability(DurabilityPrimitive::DropHandle, relative);
    }

    #[cfg(test)]
    pub(crate) fn observe_handle_identity(&self, relative: &str) {
        self.record_durability(DurabilityPrimitive::ValidateIdentity, relative);
    }

    pub(crate) fn identity_of_handle(&self, file: &std::fs::File) -> Result<HeldIdentity> {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            let identity = identity_from_fd(file, "inspect rooted file handle")?;
            require_same_mount(
                &self.identity,
                &identity,
                "validate rooted file-handle mount",
            )?;
            Ok(identity)
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = file;
            MutationTarget::Unsupported.require_supported("inspect rooted file handle")?;
            unreachable!("unsupported mutation target returned success")
        }
    }

    pub(crate) fn validate_handle_at(
        &self,
        relative: &str,
        file: &std::fs::File,
        expected_mode: u32,
    ) -> Result<HeldIdentity> {
        let handle = self.identity_of_handle(file)?;
        let path = self.identity_at(relative)?.ok_or_else(|| {
            transaction_error(
                "validate rooted file handle identity",
                format!("path disappeared: {relative}"),
            )
        })?;
        require_regular_policy(&handle, expected_mode, "validate rooted file handle")?;
        if !handle.matches_recovery(&path) {
            return Err(transaction_error(
                "validate rooted file handle identity",
                format!("path and handle differ: {relative}"),
            ));
        }
        #[cfg(test)]
        self.record_durability(DurabilityPrimitive::ValidateIdentity, relative);
        Ok(handle)
    }

    pub(crate) fn publish_file_exclusive(
        &self,
        parent: &str,
        final_name: &str,
        temporary_name: &str,
        bytes: &[u8],
        mode: u32,
    ) -> Result<HeldIdentity> {
        #[cfg(test)]
        let _phase = self.begin_default_observation_phase(DurabilityPhase::FilePublication);
        checked_name(final_name)?;
        checked_name(temporary_name)?;
        let temporary = joined(parent, temporary_name);
        let final_path = joined(parent, final_name);
        if self.exists(&temporary)? {
            self.discard_partial_temporary(&temporary, bytes, mode)?;
        }
        let temporary_identity = match self.create_file_exclusive(&temporary, bytes, mode) {
            Ok(identity) => identity,
            Err(original) => {
                return match self.exists(&temporary) {
                    Ok(true) => match self.discard_partial_temporary(&temporary, bytes, mode) {
                        Ok(()) => Err(original),
                        Err(cleanup) => Err(transaction_error(
                            "clean failed rooted publication temporary",
                            cleanup.to_string(),
                        )),
                    },
                    Ok(false) => Err(original),
                    Err(cleanup) => Err(transaction_error(
                        "inspect failed rooted publication temporary",
                        cleanup.to_string(),
                    )),
                };
            }
        };
        if let Err(error) =
            self.rename_exclusive_bound(&temporary, &final_path, &temporary_identity)
        {
            return match self.remove_file_exact(&temporary, &temporary_identity) {
                Ok(()) => Err(error),
                Err(cleanup) => Err(transaction_error(
                    "clean rooted publication temporary",
                    cleanup.to_string(),
                )),
            };
        }
        self.identity_at(&final_path)?.ok_or_else(|| {
            transaction_error(
                "publish rooted regular file",
                format!("published name disappeared: {final_path}"),
            )
        })
    }

    fn discard_partial_temporary(&self, temporary: &str, expected: &[u8], mode: u32) -> Result<()> {
        let identity = self.identity_at(temporary)?.ok_or_else(|| {
            transaction_error(
                "recover rooted publication temporary",
                format!("temporary disappeared: {temporary}"),
            )
        })?;
        let bytes = self.read_file(temporary, mode)?;
        if !expected.starts_with(&bytes) {
            return Err(transaction_error(
                "recover rooted publication temporary",
                format!("temporary bytes are not a publication prefix: {temporary}"),
            ));
        }
        self.remove_file_exact(temporary, &identity)
    }

    pub(crate) fn rename_exclusive_bound(
        &self,
        from: &str,
        to: &str,
        expected_from: &HeldIdentity,
    ) -> Result<()> {
        self.rename_with(from, to, RenameMode::Exclusive, Some(expected_from), None)
    }

    pub(crate) fn rename_swap_bound(
        &self,
        from: &str,
        to: &str,
        expected_from: &HeldIdentity,
        expected_to: &HeldIdentity,
    ) -> Result<()> {
        self.rename_with(
            from,
            to,
            RenameMode::Swap,
            Some(expected_from),
            Some(expected_to),
        )
    }

    fn rename_with(
        &self,
        from: &str,
        to: &str,
        mode: RenameMode,
        expected_from: Option<&HeldIdentity>,
        expected_to: Option<&HeldIdentity>,
    ) -> Result<()> {
        #[cfg(test)]
        let _phase = self.begin_default_observation_phase(DurabilityPhase::Rooted);
        self.revalidate_root()?;
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            use rustix::fs::{RenameFlags, fsync, renameat_with};

            let (from_parent, from_name) = self.open_parent(from)?;
            let (to_parent, to_name) = self.open_parent(to)?;
            let from_before = identity_at_held_parent(
                &from_parent,
                from_name,
                &self.identity,
                &self.canonical_root.join(from),
            )?
            .ok_or_else(|| {
                transaction_error("rename rooted object", format!("source is absent: {from}"))
            })?;
            if expected_from.is_some_and(|expected| !expected.matches_recovery(&from_before)) {
                return Err(transaction_error(
                    "rename rooted object",
                    format!("source identity changed: {from}"),
                ));
            }
            let to_before = identity_at_held_parent(
                &to_parent,
                to_name,
                &self.identity,
                &self.canonical_root.join(to),
            )?;
            match mode {
                RenameMode::Exclusive if to_before.is_some() => {
                    return Err(transaction_error(
                        "rename rooted object exclusively",
                        format!("destination already exists: {to}"),
                    ));
                }
                RenameMode::Swap => {
                    let actual = to_before.as_ref().ok_or_else(|| {
                        transaction_error(
                            "swap rooted objects",
                            format!("destination is absent: {to}"),
                        )
                    })?;
                    if expected_to.is_some_and(|expected| !expected.matches_recovery(actual)) {
                        return Err(transaction_error(
                            "swap rooted objects",
                            format!("destination identity changed: {to}"),
                        ));
                    }
                }
                RenameMode::Exclusive => {}
            }
            let flags = match mode {
                RenameMode::Exclusive => RenameFlags::NOREPLACE,
                RenameMode::Swap => RenameFlags::EXCHANGE,
            };
            renameat_with(&from_parent, from_name, &to_parent, to_name, flags).map_err(
                |source| {
                    let operation = match mode {
                        RenameMode::Exclusive => "rename rooted object exclusively",
                        RenameMode::Swap => "swap rooted objects",
                    };
                    io_error(operation, &self.canonical_root.join(to), source)
                },
            )?;
            #[cfg(test)]
            self.record_durability(
                match mode {
                    RenameMode::Exclusive => DurabilityPrimitive::RenameExclusive,
                    RenameMode::Swap => DurabilityPrimitive::RenameSwap,
                },
                to,
            );
            let from_after = identity_at_held_parent(
                &from_parent,
                from_name,
                &self.identity,
                &self.canonical_root.join(from),
            )?;
            let to_after = identity_at_held_parent(
                &to_parent,
                to_name,
                &self.identity,
                &self.canonical_root.join(to),
            )?;
            let placement_is_exact = match mode {
                RenameMode::Exclusive => {
                    from_after.is_none()
                        && to_after
                            .as_ref()
                            .is_some_and(|actual| from_before.matches_recovery(actual))
                }
                RenameMode::Swap => {
                    let to_before = to_before.as_ref().expect("swap destination checked");
                    from_after
                        .as_ref()
                        .is_some_and(|actual| to_before.matches_recovery(actual))
                        && to_after
                            .as_ref()
                            .is_some_and(|actual| from_before.matches_recovery(actual))
                }
            };
            if !placement_is_exact {
                return Err(transaction_error(
                    "validate rooted rename placement",
                    format!("post-rename identities differ: {from} -> {to}"),
                ));
            }
            fsync(&from_parent).map_err(|source| {
                io_error(
                    "sync rooted rename source parent",
                    &self.canonical_root.join(from),
                    source,
                )
            })?;
            #[cfg(test)]
            self.record_durability(DurabilityPrimitive::SyncDirectory, observation_parent(from));
            fsync(&to_parent).map_err(|source| {
                io_error(
                    "sync rooted rename destination parent",
                    &self.canonical_root.join(to),
                    source,
                )
            })?;
            #[cfg(test)]
            self.record_durability(DurabilityPrimitive::SyncDirectory, observation_parent(to));
            Ok(())
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = (from, to, mode, expected_from, expected_to);
            MutationTarget::Unsupported.require_supported("rename rooted object")
        }
    }

    pub(crate) fn remove_file_exact(&self, relative: &str, expected: &HeldIdentity) -> Result<()> {
        self.remove_exact(relative, expected, false)
    }

    pub(crate) fn remove_dir_exact(&self, relative: &str, expected: &HeldIdentity) -> Result<()> {
        self.remove_exact(relative, expected, true)
    }

    /// Erases one browser-owned opaque directory without resolving a child through a path.
    #[cfg(feature = "layout-browser")]
    pub(crate) fn erase_opaque_directory(
        &self,
        relative: &str,
        expected: &HeldIdentity,
    ) -> Result<()> {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            let directory = self.open_dir(relative)?;
            let actual = identity_from_fd(&directory, "inspect opaque directory root")?;
            require_same_mount(&self.identity, &actual, "validate opaque directory mount")?;
            if actual.kind != NodeKind::Directory || !expected.matches_recovery(&actual) {
                return Err(transaction_error(
                    "erase opaque directory",
                    format!("opaque root identity changed: {relative}"),
                ));
            }
            erase_opaque_children(self, &directory, relative)?;
            let after = identity_from_fd(&directory, "reinspect opaque directory root")?;
            if !expected.matches_recovery(&after) {
                return Err(transaction_error(
                    "erase opaque directory",
                    format!("opaque root identity drifted: {relative}"),
                ));
            }
            drop(directory);
            self.remove_dir_exact(relative, expected)
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = (relative, expected);
            MutationTarget::Unsupported.require_supported("erase opaque browser directory")
        }
    }

    fn remove_exact(&self, relative: &str, expected: &HeldIdentity, directory: bool) -> Result<()> {
        #[cfg(test)]
        let _phase = self.begin_default_observation_phase(DurabilityPhase::Rooted);
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            use rustix::fs::{AtFlags, fsync, unlinkat};

            let (parent, name) = self.open_parent(relative)?;
            let actual = identity_at_held_parent(
                &parent,
                name,
                &self.identity,
                &self.canonical_root.join(relative),
            )?
            .ok_or_else(|| {
                transaction_error(
                    "remove rooted object",
                    format!("recorded object is absent: {relative}"),
                )
            })?;
            if !expected.matches_recovery(&actual) {
                return Err(transaction_error(
                    "remove rooted object",
                    format!("recorded identity changed: {relative}"),
                ));
            }
            if directory != (actual.kind == NodeKind::Directory) {
                return Err(transaction_error(
                    "remove rooted object",
                    format!("recorded type changed: {relative}"),
                ));
            }
            let flags = if directory {
                AtFlags::REMOVEDIR
            } else {
                AtFlags::empty()
            };
            unlinkat(&parent, name, flags).map_err(|source| {
                io_error(
                    "unlink rooted object",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            #[cfg(test)]
            self.record_durability(
                if directory {
                    DurabilityPrimitive::RemoveDirectory
                } else {
                    DurabilityPrimitive::RemoveFile
                },
                relative,
            );
            fsync(&parent).map_err(|source| {
                io_error(
                    "sync rooted unlink parent",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            #[cfg(test)]
            self.record_durability(
                DurabilityPrimitive::SyncDirectory,
                observation_parent(relative),
            );
            Ok(())
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = (relative, expected, directory);
            MutationTarget::Unsupported.require_supported("remove rooted object")
        }
    }

    pub(crate) fn sync_dir(&self, relative: &str) -> Result<()> {
        #[cfg(test)]
        let _phase = self.begin_default_observation_phase(DurabilityPhase::Rooted);
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            let directory = self.open_dir(relative)?;
            rustix::fs::fsync(&directory).map_err(|source| {
                io_error(
                    "sync rooted directory",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            #[cfg(test)]
            self.record_durability(DurabilityPrimitive::SyncDirectory, relative);
            Ok(())
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = relative;
            MutationTarget::Unsupported.require_supported("sync rooted directory")
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn open_dir(&self, relative: &str) -> Result<rustix::fd::OwnedFd> {
        let components = checked_components_allow_root(relative)?;
        let mut current = rustix::io::dup(&self.root).map_err(|source| {
            io_error("duplicate rooted authority", &self.canonical_root, source)
        })?;
        for component in components {
            require_exact_entry_name(&current, component)?;
            let child = open_directory_at(&current, component, "open rooted directory")?;
            let identity = identity_from_fd(&child, "inspect rooted directory")?;
            require_same_mount(&self.identity, &identity, "validate rooted directory mount")?;
            current = child;
        }
        Ok(current)
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn open_parent<'a>(&self, relative: &'a str) -> Result<(rustix::fd::OwnedFd, &'a str)> {
        let components = checked_components(relative)?;
        let (name, parents) = components
            .split_last()
            .ok_or_else(|| invalid_path("open rooted parent", relative))?;
        let mut current = rustix::io::dup(&self.root).map_err(|source| {
            io_error("duplicate rooted authority", &self.canonical_root, source)
        })?;
        for component in parents {
            require_exact_entry_name(&current, component)?;
            let child = open_directory_at(&current, component, "open rooted parent component")?;
            let identity = identity_from_fd(&child, "inspect rooted parent component")?;
            require_same_mount(&self.identity, &identity, "validate rooted parent mount")?;
            current = child;
        }
        Ok((current, name))
    }
}

#[cfg(all(
    feature = "layout-browser",
    target_os = "macos",
    target_arch = "aarch64"
))]
fn erase_opaque_children(
    rooted: &RootedFs,
    parent: &rustix::fd::OwnedFd,
    relative: &str,
) -> Result<()> {
    use std::ffi::CString;

    use rustix::fs::{AtFlags, FileType, Mode, OFlags, fsync, openat, statat, unlinkat};

    let parent_identity = identity_from_fd(parent, "inspect opaque directory")?;
    require_same_mount(
        &rooted.identity,
        &parent_identity,
        "validate opaque directory mount",
    )?;
    let mut directory = rustix::fs::Dir::read_from(parent).map_err(|source| {
        io_error(
            "open opaque directory stream",
            &rooted.canonical_root.join(relative),
            source,
        )
    })?;
    let mut names = Vec::new();
    for entry in &mut directory {
        let entry = entry.map_err(|source| {
            io_error(
                "read opaque directory",
                &rooted.canonical_root.join(relative),
                source,
            )
        })?;
        let bytes = entry.file_name().to_bytes();
        if bytes != b"." && bytes != b".." {
            names.push(bytes.to_vec());
        }
    }
    names.sort();
    drop(directory);

    for bytes in names {
        let name = CString::new(bytes.clone()).map_err(|_| {
            transaction_error(
                "erase opaque directory",
                "directory entry unexpectedly contains NUL",
            )
        })?;
        let display_name = opaque_observation_name(&bytes);
        let child_relative = format!("{relative}/{display_name}");
        let before =
            statat(parent, name.as_c_str(), AtFlags::SYMLINK_NOFOLLOW).map_err(|source| {
                io_error(
                    "inspect opaque directory entry",
                    &rooted.canonical_root.join(&child_relative),
                    source,
                )
            })?;
        let file_type = FileType::from_raw_mode(before.st_mode);
        let before_identity = if file_type == FileType::Directory {
            let child = openat(
                parent,
                name.as_c_str(),
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::empty(),
            )
            .map_err(|source| {
                io_error(
                    "open opaque child directory",
                    &rooted.canonical_root.join(&child_relative),
                    source,
                )
            })?;
            let identity = identity_from_fd(&child, "inspect opaque child directory")?;
            require_same_mount(
                &rooted.identity,
                &identity,
                "validate opaque child directory mount",
            )?;
            erase_opaque_children(rooted, &child, &child_relative)?;
            let after = identity_from_fd(&child, "reinspect opaque child directory")?;
            if !identity.matches_recovery(&after) {
                return Err(transaction_error(
                    "erase opaque directory",
                    format!("opaque child identity drifted: {child_relative}"),
                ));
            }
            drop(child);
            identity
        } else {
            let identity = identity_from_stat(&before, parent_identity.fsid)?;
            require_same_mount(&rooted.identity, &identity, "validate opaque entry mount")?;
            identity
        };
        let current =
            statat(parent, name.as_c_str(), AtFlags::SYMLINK_NOFOLLOW).map_err(|source| {
                io_error(
                    "reinspect opaque directory entry",
                    &rooted.canonical_root.join(&child_relative),
                    source,
                )
            })?;
        let current_identity = identity_from_stat(&current, parent_identity.fsid)?;
        if !before_identity.matches_recovery(&current_identity)
            || (file_type == FileType::Directory)
                != (FileType::from_raw_mode(current.st_mode) == FileType::Directory)
        {
            return Err(transaction_error(
                "erase opaque directory",
                format!("opaque entry identity changed: {child_relative}"),
            ));
        }
        let flags = if file_type == FileType::Directory {
            AtFlags::REMOVEDIR
        } else {
            AtFlags::empty()
        };
        unlinkat(parent, name.as_c_str(), flags).map_err(|source| {
            io_error(
                "remove opaque directory entry",
                &rooted.canonical_root.join(&child_relative),
                source,
            )
        })?;
        #[cfg(test)]
        rooted.record_durability(
            if file_type == FileType::Directory {
                DurabilityPrimitive::RemoveDirectory
            } else {
                DurabilityPrimitive::RemoveFile
            },
            &child_relative,
        );
        fsync(parent).map_err(|source| {
            io_error(
                "sync opaque directory",
                &rooted.canonical_root.join(relative),
                source,
            )
        })?;
        #[cfg(test)]
        rooted.record_durability(DurabilityPrimitive::SyncDirectory, relative);
    }
    Ok(())
}

#[cfg(all(
    feature = "layout-browser",
    target_os = "macos",
    target_arch = "aarch64"
))]
fn opaque_observation_name(bytes: &[u8]) -> String {
    std::str::from_utf8(bytes).map_or_else(
        |_| {
            let mut encoded = String::new();
            for byte in bytes {
                use std::fmt::Write as _;
                write!(&mut encoded, "%{byte:02x}").expect("writing to a String cannot fail");
            }
            encoded
        },
        str::to_owned,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RenameMode {
    Exclusive,
    Swap,
}

fn checked_components(relative: &str) -> Result<Vec<&str>> {
    if relative.is_empty() || relative.starts_with('/') || relative.ends_with('/') {
        return Err(invalid_path("validate rooted relative path", relative));
    }
    let components: Vec<_> = relative.split('/').collect();
    if components.iter().any(|component| {
        component.is_empty()
            || *component == "."
            || *component == ".."
            || component.contains('\0')
            || component.contains('\\')
    }) {
        return Err(invalid_path("validate rooted relative path", relative));
    }
    Ok(components)
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn checked_components_allow_root(relative: &str) -> Result<Vec<&str>> {
    if relative.is_empty() {
        Ok(Vec::new())
    } else {
        checked_components(relative)
    }
}

fn checked_name(name: &str) -> Result<()> {
    let components = checked_components(name)?;
    if components.len() != 1 {
        return Err(invalid_path("validate rooted entry name", name));
    }
    Ok(())
}

fn joined(parent: &str, name: &str) -> String {
    if parent.is_empty() {
        name.to_owned()
    } else {
        format!("{parent}/{name}")
    }
}

#[cfg(test)]
fn observation_parent(relative: &str) -> &str {
    relative
        .rsplit_once('/')
        .map_or("", |(parent, _name)| parent)
}

#[cfg(test)]
fn strict_observation_path(relative: &str) -> bool {
    relative.is_empty()
        || (!relative.starts_with('/')
            && !relative.ends_with('/')
            && relative
                .split('/')
                .all(|component| !matches!(component, "" | "." | "..")))
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn checked_mode(mode: u32) -> Result<rustix::fs::Mode> {
    let raw = u16::try_from(mode).map_err(|_| {
        transaction_error(
            "validate rooted filesystem mode",
            format!("mode is outside the supported range: {mode:#o}"),
        )
    })?;
    Ok(rustix::fs::Mode::from_raw_mode(raw))
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn open_directory_at(
    parent: &rustix::fd::OwnedFd,
    name: &str,
    operation: &str,
) -> Result<rustix::fd::OwnedFd> {
    use rustix::fs::{Mode, OFlags, openat};

    openat(
        parent,
        name,
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
        Mode::empty(),
    )
    .map_err(|source| io_error(operation, Path::new(name), source))
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn open_existing_component(
    parent: &rustix::fd::OwnedFd,
    name: &str,
    root: &HeldIdentity,
    display: &Path,
    directory_required: bool,
) -> Result<Option<rustix::fd::OwnedFd>> {
    use rustix::fs::{AtFlags, Mode, OFlags, openat, statat};
    use rustix::io::Errno;

    let stat = match statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(Errno::NOENT) => {
            let names = list_names(parent, display)?;
            if names.iter().any(|entry| entry == name) {
                return Err(transaction_error(
                    "inspect rooted path",
                    format!("entry appeared during absence check: {}", display.display()),
                ));
            }
            return Ok(None);
        }
        Err(source) => return Err(io_error("inspect rooted path", display, source)),
    };
    require_exact_entry_name(parent, name)?;

    let stat_identity = identity_from_stat(&stat, root.fsid)?;
    let flags = match stat_identity.kind {
        NodeKind::Directory => {
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW
        }
        NodeKind::Regular => {
            if directory_required {
                OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW
            } else {
                OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW
            }
        }
        NodeKind::Symlink => OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
    };
    let opened = openat(parent, name, flags, Mode::empty())
        .map_err(|source| io_error("open rooted existence component", display, source))?;
    let identity = identity_from_fd(&opened, "inspect rooted existence component")?;
    require_same_mount(root, &identity, "validate rooted existence mount")?;
    if !stat_identity.same_object(&identity) {
        return Err(transaction_error(
            "inspect rooted path",
            format!(
                "identity changed during existence check: {}",
                display.display()
            ),
        ));
    }
    require_existing_component_policy(root, &identity, directory_required, display)?;
    Ok(Some(opened))
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn require_existing_component_policy(
    root: &HeldIdentity,
    identity: &HeldIdentity,
    directory_required: bool,
    display: &Path,
) -> Result<()> {
    let valid = match identity.kind {
        NodeKind::Directory => matches!(
            identity.mode,
            PRIVATE_DIRECTORY_MODE | CORPUS_DIRECTORY_MODE
        ),
        NodeKind::Regular => {
            !directory_required
                && matches!(identity.mode, PRIVATE_FILE_MODE | CORPUS_FILE_MODE)
                && identity.link_count == Some(1)
        }
        NodeKind::Symlink => false,
    };
    if !valid || identity.owner != root.owner {
        return Err(transaction_error(
            "validate rooted existence component",
            format!(
                "type, ownership, or mode policy mismatch at {}: {:?} {:#o} {} {:?}",
                display.display(),
                identity.kind,
                identity.mode,
                identity.owner,
                identity.link_count
            ),
        ));
    }
    Ok(())
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn identity_at_held_parent(
    parent: &rustix::fd::OwnedFd,
    name: &str,
    root: &HeldIdentity,
    display: &Path,
) -> Result<Option<HeldIdentity>> {
    use rustix::fs::{AtFlags, FileType, Mode, OFlags, openat, statat};
    use rustix::io::Errno;

    let stat = match statat(parent, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(stat) => stat,
        Err(Errno::NOENT) => return Ok(None),
        Err(source) => return Err(io_error("inspect rooted object", display, source)),
    };
    require_exact_entry_name(parent, name)?;
    let kind = FileType::from_raw_mode(stat.st_mode);
    if kind == FileType::Symlink {
        let parent_identity = identity_from_fd(parent, "inspect symlink parent")?;
        let identity = identity_from_stat(&stat, parent_identity.fsid)?;
        require_same_mount(root, &identity, "validate symlink mount")?;
        return Ok(Some(identity));
    }
    let flags = if kind == FileType::Directory {
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW
    } else {
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW
    };
    let opened = openat(parent, name, flags, Mode::empty())
        .map_err(|source| io_error("open rooted object", display, source))?;
    let identity = identity_from_fd(&opened, "inspect opened rooted object")?;
    require_same_mount(root, &identity, "validate rooted object mount")?;
    Ok(Some(identity))
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn require_exact_entry_name(parent: &rustix::fd::OwnedFd, expected: &str) -> Result<()> {
    let names = list_names(parent, Path::new(expected))?;
    if names.iter().any(|name| name == expected) {
        return Ok(());
    }
    Err(invalid_path(
        "validate exact rooted entry name",
        format!("entry is absent or aliased: {expected}"),
    ))
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn list_names(parent: &rustix::fd::OwnedFd, display: &Path) -> Result<Vec<String>> {
    let mut directory = rustix::fs::Dir::read_from(parent)
        .map_err(|source| io_error("open rooted directory stream", display, source))?;
    let mut names = Vec::new();
    for entry in &mut directory {
        let entry = entry.map_err(|source| io_error("read rooted directory", display, source))?;
        let bytes = entry.file_name().to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        let name = std::str::from_utf8(bytes).map_err(|_| {
            transaction_error(
                "read rooted directory",
                format!("non-UTF-8 entry beneath {}", display.display()),
            )
        })?;
        names.push(name.to_owned());
    }
    names.sort();
    Ok(names)
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn identity_from_fd<Fd: std::os::fd::AsFd>(fd: Fd, operation: &str) -> Result<HeldIdentity> {
    let stat = rustix::fs::fstat(&fd)
        .map_err(|source| io_error(operation, Path::new("<held descriptor>"), source))?;
    let statfs = rustix::fs::fstatfs(&fd).map_err(|source| {
        GeneratorError::new(
            GeneratorErrorKind::UnsupportedPlatform,
            operation,
            format!("filesystem identity is unavailable: {source}"),
        )
    })?;
    let rendered = format!("{:?}", statfs.f_fsid);
    let words = rendered
        .strip_prefix("fsid_t { __fsid_val: [")
        .and_then(|value| value.strip_suffix("] }"))
        .and_then(|value| value.split_once(','))
        .and_then(|(left, right)| {
            Some((
                left.trim().parse::<i32>().ok()?,
                right.trim().parse::<i32>().ok()?,
            ))
        })
        .map(|(left, right)| [left, right])
        .ok_or_else(|| {
            GeneratorError::new(
                GeneratorErrorKind::UnsupportedPlatform,
                operation,
                format!("filesystem identity has an unknown representation: {rendered}"),
            )
        })?;
    identity_from_stat(&stat, FilesystemId { words })
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn identity_from_stat(stat: &rustix::fs::Stat, fsid: FilesystemId) -> Result<HeldIdentity> {
    use rustix::fs::FileType;

    let kind = match FileType::from_raw_mode(stat.st_mode) {
        FileType::Directory => NodeKind::Directory,
        FileType::RegularFile => NodeKind::Regular,
        FileType::Symlink => NodeKind::Symlink,
        other => {
            return Err(transaction_error(
                "inspect rooted object type",
                format!("unsupported object type: {other:?}"),
            ));
        }
    };
    Ok(HeldIdentity {
        kind,
        device: i64::from(stat.st_dev),
        inode: stat.st_ino,
        fsid,
        mode: u32::from(stat.st_mode) & 0o7777,
        owner: stat.st_uid,
        link_count: if kind == NodeKind::Directory {
            None
        } else {
            Some(u64::from(stat.st_nlink))
        },
    })
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn require_same_mount(root: &HeldIdentity, child: &HeldIdentity, operation: &str) -> Result<()> {
    if root.device != child.device || root.fsid != child.fsid {
        return Err(invalid_path(
            operation,
            "rooted traversal crossed a device or filesystem identity",
        ));
    }
    Ok(())
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn require_directory_policy(
    identity: &HeldIdentity,
    expected_mode: Option<u32>,
    operation: &str,
) -> Result<()> {
    if identity.kind != NodeKind::Directory {
        return Err(transaction_error(operation, "object is not a directory"));
    }
    if let Some(expected_mode) = expected_mode
        && identity.mode != expected_mode
    {
        return Err(transaction_error(
            operation,
            format!("expected mode {expected_mode:#o}, got {:#o}", identity.mode),
        ));
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    if identity.owner != rustix::process::geteuid().as_raw() {
        return Err(transaction_error(
            operation,
            "directory has a foreign owner",
        ));
    }
    Ok(())
}

fn require_regular_policy(
    identity: &HeldIdentity,
    expected_mode: u32,
    operation: &str,
) -> Result<()> {
    if identity.kind != NodeKind::Regular
        || identity.mode != expected_mode
        || identity.link_count != Some(1)
    {
        return Err(transaction_error(
            operation,
            format!(
                "regular-file policy mismatch: kind={:?}, mode={:#o}, links={:?}",
                identity.kind, identity.mode, identity.link_count
            ),
        ));
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    if identity.owner != rustix::process::geteuid().as_raw() {
        return Err(transaction_error(
            operation,
            "regular file has a foreign owner",
        ));
    }
    Ok(())
}

fn invalid_path(operation: &str, detail: impl std::fmt::Display) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidPath,
        operation,
        detail.to_string(),
    )
}

fn transaction_error(operation: &str, detail: impl std::fmt::Display) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::ArtifactTransaction,
        operation,
        detail.to_string(),
    )
}

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
fn io_error<E>(operation: &str, path: &Path, source: E) -> GeneratorError
where
    E: std::error::Error + Send + Sync + 'static,
{
    GeneratorError::with_source(
        GeneratorErrorKind::ArtifactTransaction,
        operation,
        path.display().to_string(),
        source,
    )
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        CORPUS_DIRECTORY_MODE, CORPUS_FILE_MODE, DurabilityPhase, DurabilityPrimitive,
        MutationTarget, NodeKind, PRIVATE_DIRECTORY_MODE, PRIVATE_FILE_MODE, RootedFs,
        RootedObserver,
    };
    use crate::CorpusLocation;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "surgeist-generator-rooted-{label}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn rooted_fixture(label: &str) -> (TestDirectory, PathBuf, RootedFs) {
        let directory = TestDirectory::new(label);
        let corpus = directory.path().join("corpus");
        fs::create_dir(&corpus).unwrap();
        let location = CorpusLocation::new(directory.path(), &corpus).unwrap();
        let rooted = RootedFs::open_corpus(&location).unwrap();
        (directory, corpus, rooted)
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn observed_fixture(
        label: &str,
        observer: RootedObserver,
    ) -> (TestDirectory, PathBuf, RootedFs) {
        let directory = TestDirectory::new(label);
        let corpus = directory.path().join("corpus");
        fs::create_dir(&corpus).unwrap();
        let location = CorpusLocation::new(directory.path(), &corpus).unwrap();
        let rooted = RootedFs::open_corpus_observed(&location, observer).unwrap();
        (directory, corpus, rooted)
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn record_recovery_distinct_trace(rooted: &RootedFs) {
        rooted
            .publish_file_exclusive(
                "",
                "published.json",
                "published.tmp",
                b"abc",
                PRIVATE_FILE_MODE,
            )
            .unwrap();

        {
            let _phase = rooted.begin_observation_phase(DurabilityPhase::TransactionStage);
            rooted
                .ensure_dir("stage/deep", CORPUS_DIRECTORY_MODE)
                .unwrap();
            rooted
                .create_file_exclusive("stage/deep/value.txt", b"xy", CORPUS_FILE_MODE)
                .unwrap();
            rooted.sync_dir("stage/deep").unwrap();
            rooted.sync_dir("stage").unwrap();
        }

        {
            let _phase = rooted.begin_observation_phase(DurabilityPhase::OwnerInstall);
            let mut stage = rooted
                .create_file_handle_exclusive("owner.stage", b"", PRIVATE_FILE_MODE)
                .unwrap();
            rooted.identity_of_handle(&stage).unwrap();
            rooted.observe_handle_identity("owner.stage");
            rooted
                .write_file_handle_all("owner.stage", &mut stage, b"owner")
                .unwrap();
            rooted.flush_file_handle("owner.stage", &mut stage).unwrap();
            rooted.sync_file_handle("owner.stage", &stage).unwrap();
            rooted
                .validate_handle_at("owner.stage", &stage, PRIVATE_FILE_MODE)
                .unwrap();
            rooted.drop_file_handle("owner.stage", stage);
        }

        let exclusive = rooted
            .create_dir_exclusive("exclusive-source", PRIVATE_DIRECTORY_MODE)
            .unwrap();
        {
            let _phase = rooted.begin_observation_phase(DurabilityPhase::TransactionInstall);
            rooted
                .rename_exclusive_bound("exclusive-source", "exclusive-final", &exclusive)
                .unwrap();
        }

        let swap_source = rooted
            .create_dir_exclusive("swap-source", PRIVATE_DIRECTORY_MODE)
            .unwrap();
        let swap_destination = rooted
            .create_dir_exclusive("swap-destination", PRIVATE_DIRECTORY_MODE)
            .unwrap();
        {
            let _phase = rooted.begin_observation_phase(DurabilityPhase::TransactionInstall);
            rooted
                .rename_swap_bound(
                    "swap-source",
                    "swap-destination",
                    &swap_source,
                    &swap_destination,
                )
                .unwrap();
        }

        let claim = rooted
            .create_dir_exclusive("claim-source", PRIVATE_DIRECTORY_MODE)
            .unwrap();
        {
            let _phase = rooted.begin_observation_phase(DurabilityPhase::BootstrapRecovery);
            rooted
                .rename_exclusive_bound("claim-source", "recovering-claim", &claim)
                .unwrap();
        }

        let active = rooted
            .create_dir_exclusive("active-journal", PRIVATE_DIRECTORY_MODE)
            .unwrap();
        {
            let _phase = rooted.begin_observation_phase(DurabilityPhase::TransactionCleanup);
            rooted
                .rename_exclusive_bound("active-journal", "completed-journal", &active)
                .unwrap();
        }

        let receipt_journal = rooted
            .create_dir_exclusive("receipt-journal", PRIVATE_DIRECTORY_MODE)
            .unwrap();
        let receipt_directory = rooted
            .create_dir_exclusive("receipt-journal/receipt-dir", PRIVATE_DIRECTORY_MODE)
            .unwrap();
        let receipt_file = rooted
            .create_file_exclusive("receipt-journal/receipt-file", b"member", PRIVATE_FILE_MODE)
            .unwrap();
        let receipt = rooted
            .create_file_exclusive(
                "receipt-journal/cleanup-receipt.json",
                b"receipt",
                PRIVATE_FILE_MODE,
            )
            .unwrap();
        {
            let _phase = rooted.begin_observation_phase(DurabilityPhase::TransactionCleanup);
            rooted
                .remove_file_exact("receipt-journal/receipt-file", &receipt_file)
                .unwrap();
            rooted
                .remove_dir_exact("receipt-journal/receipt-dir", &receipt_directory)
                .unwrap();
            rooted
                .remove_file_exact("receipt-journal/cleanup-receipt.json", &receipt)
                .unwrap();
            rooted
                .remove_dir_exact("receipt-journal", &receipt_journal)
                .unwrap();
        }

        let probe_journal = rooted
            .create_dir_exclusive("probe-journal", PRIVATE_DIRECTORY_MODE)
            .unwrap();
        let probe_member = rooted
            .create_dir_exclusive("probe-journal/probe-member", PRIVATE_DIRECTORY_MODE)
            .unwrap();
        {
            let _phase = rooted.begin_observation_phase(DurabilityPhase::ProbeRecovery);
            rooted
                .remove_dir_exact("probe-journal/probe-member", &probe_member)
                .unwrap();
            rooted
                .remove_dir_exact("probe-journal", &probe_journal)
                .unwrap();
        }

        let owner_journal = rooted
            .create_dir_exclusive("owner-journal", PRIVATE_DIRECTORY_MODE)
            .unwrap();
        let owner_member = rooted
            .create_file_exclusive("owner-journal/intent.json", b"intent", PRIVATE_FILE_MODE)
            .unwrap();
        {
            let _phase = rooted.begin_observation_phase(DurabilityPhase::OwnerCleanup);
            rooted
                .remove_file_exact("owner-journal/intent.json", &owner_member)
                .unwrap();
            rooted
                .remove_dir_exact("owner-journal", &owner_journal)
                .unwrap();
        }
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn rooted_observer_records_recovery_distinct_primitives() {
        let first_observer = RootedObserver::recording();
        let (_first_directory, _first_corpus, first_rooted) =
            observed_fixture("observer-records-first", first_observer.clone());
        record_recovery_distinct_trace(&first_rooted);
        let first = first_observer.events();

        let second_observer = RootedObserver::recording();
        let (_second_directory, _second_corpus, second_rooted) =
            observed_fixture("observer-records-second", second_observer.clone());
        record_recovery_distinct_trace(&second_rooted);
        let second = second_observer.events();

        assert_eq!(first_observer.events(), first);
        assert_eq!(second, first, "the same rooted fixture must have one trace");

        let mut primitive_counts = BTreeMap::new();
        for event in &first {
            *primitive_counts.entry(event.primitive()).or_insert(0usize) += 1;
        }
        assert_eq!(first.len(), 161);
        assert_eq!(
            primitive_counts,
            BTreeMap::from([
                (DurabilityPrimitive::CreateDirectory, 12),
                (DurabilityPrimitive::CreateFile, 6),
                (DurabilityPrimitive::WritePartial, 23),
                (DurabilityPrimitive::WriteFull, 6),
                (DurabilityPrimitive::SetPermissions, 18),
                (DurabilityPrimitive::FlushFile, 7),
                (DurabilityPrimitive::SyncFile, 7),
                (DurabilityPrimitive::ValidateIdentity, 18),
                (DurabilityPrimitive::DropHandle, 1),
                (DurabilityPrimitive::RenameExclusive, 4),
                (DurabilityPrimitive::RenameSwap, 1),
                (DurabilityPrimitive::RemoveFile, 3),
                (DurabilityPrimitive::RemoveDirectory, 5),
                (DurabilityPrimitive::SyncDirectory, 50),
            ])
        );

        let primitives: BTreeSet<_> = first.iter().map(|event| event.primitive()).collect();
        assert_eq!(
            primitives,
            BTreeSet::from([
                DurabilityPrimitive::CreateDirectory,
                DurabilityPrimitive::CreateFile,
                DurabilityPrimitive::WritePartial,
                DurabilityPrimitive::WriteFull,
                DurabilityPrimitive::SetPermissions,
                DurabilityPrimitive::FlushFile,
                DurabilityPrimitive::SyncFile,
                DurabilityPrimitive::ValidateIdentity,
                DurabilityPrimitive::DropHandle,
                DurabilityPrimitive::RenameExclusive,
                DurabilityPrimitive::RenameSwap,
                DurabilityPrimitive::RemoveFile,
                DurabilityPrimitive::RemoveDirectory,
                DurabilityPrimitive::SyncDirectory,
            ])
        );

        let mut next_ordinal = BTreeMap::new();
        for event in &first {
            let expected = next_ordinal.entry(event.phase()).or_insert(0);
            assert_eq!(event.ordinal(), *expected);
            *expected += 1;
            assert!(
                event.path().is_empty()
                    || (!event.path().starts_with('/')
                        && !event.path().ends_with('/')
                        && event
                            .path()
                            .split('/')
                            .all(|component| !matches!(component, "" | "." | ".."))),
                "observer path is not strict and rooted: {}",
                event.path()
            );
        }

        let contains = |phase, primitive, path| {
            first.iter().any(|event| {
                event.phase() == phase && event.primitive() == primitive && event.path() == path
            })
        };
        assert!(contains(
            DurabilityPhase::FilePublication,
            DurabilityPrimitive::CreateFile,
            "published.tmp"
        ));
        assert!(contains(
            DurabilityPhase::FilePublication,
            DurabilityPrimitive::WritePartial,
            "published.tmp"
        ));
        assert!(contains(
            DurabilityPhase::FilePublication,
            DurabilityPrimitive::WriteFull,
            "published.tmp"
        ));
        assert!(contains(
            DurabilityPhase::FilePublication,
            DurabilityPrimitive::SyncFile,
            "published.tmp"
        ));
        assert!(contains(
            DurabilityPhase::FilePublication,
            DurabilityPrimitive::RenameExclusive,
            "published.json"
        ));
        assert!(contains(
            DurabilityPhase::FilePublication,
            DurabilityPrimitive::SyncDirectory,
            ""
        ));
        assert!(contains(
            DurabilityPhase::TransactionStage,
            DurabilityPrimitive::CreateDirectory,
            "stage/deep"
        ));
        assert!(contains(
            DurabilityPhase::TransactionStage,
            DurabilityPrimitive::CreateFile,
            "stage/deep/value.txt"
        ));
        assert!(contains(
            DurabilityPhase::TransactionStage,
            DurabilityPrimitive::SyncDirectory,
            "stage/deep"
        ));
        for primitive in [
            DurabilityPrimitive::WritePartial,
            DurabilityPrimitive::WriteFull,
            DurabilityPrimitive::FlushFile,
            DurabilityPrimitive::SyncFile,
            DurabilityPrimitive::ValidateIdentity,
            DurabilityPrimitive::DropHandle,
        ] {
            assert!(contains(
                DurabilityPhase::OwnerInstall,
                primitive,
                "owner.stage"
            ));
        }
        assert!(contains(
            DurabilityPhase::TransactionInstall,
            DurabilityPrimitive::RenameExclusive,
            "exclusive-final"
        ));
        assert!(contains(
            DurabilityPhase::TransactionInstall,
            DurabilityPrimitive::RenameSwap,
            "swap-destination"
        ));
        assert!(contains(
            DurabilityPhase::BootstrapRecovery,
            DurabilityPrimitive::RenameExclusive,
            "recovering-claim"
        ));
        assert!(contains(
            DurabilityPhase::TransactionCleanup,
            DurabilityPrimitive::RenameExclusive,
            "completed-journal"
        ));
        assert!(contains(
            DurabilityPhase::TransactionCleanup,
            DurabilityPrimitive::RemoveFile,
            "receipt-journal/cleanup-receipt.json"
        ));
        assert!(contains(
            DurabilityPhase::ProbeRecovery,
            DurabilityPrimitive::RemoveDirectory,
            "probe-journal/probe-member"
        ));
        assert!(contains(
            DurabilityPhase::OwnerCleanup,
            DurabilityPrimitive::RemoveDirectory,
            "owner-journal"
        ));
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn rooted_observer_interrupts_without_generator_error() {
        let observer = RootedObserver::interrupt_after(1);
        let (directory, corpus, rooted) = observed_fixture("observer-interrupts", observer.clone());

        let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            rooted.publish_file_exclusive(
                "",
                "interrupted.json",
                "interrupted.tmp",
                b"abc",
                PRIVATE_FILE_MODE,
            )
        }))
        .expect_err("the observer must unwind instead of returning GeneratorError");
        assert!(RootedObserver::is_interruption(interrupted.as_ref()));
        assert!(
            interrupted
                .downcast_ref::<crate::GeneratorError>()
                .is_none()
        );
        let events = observer.events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].phase(), DurabilityPhase::FilePublication);
        assert_eq!(events[0].primitive(), DurabilityPrimitive::CreateFile);
        assert_eq!(events[0].path(), "interrupted.tmp");
        assert_eq!(events[0].ordinal(), 0);
        assert_eq!(events[1].phase(), DurabilityPhase::FilePublication);
        assert_eq!(events[1].primitive(), DurabilityPrimitive::WritePartial);
        assert_eq!(events[1].path(), "interrupted.tmp");
        assert_eq!(events[1].ordinal(), 1);

        drop(rooted);
        let location = CorpusLocation::new(directory.path(), &corpus).unwrap();
        let fresh = RootedFs::open_corpus(&location).unwrap();
        assert!(!fresh.exists("interrupted.json").unwrap());
        assert_eq!(
            fresh
                .read_file("interrupted.tmp", PRIVATE_FILE_MODE)
                .unwrap(),
            b"a"
        );

        fresh
            .publish_file_exclusive(
                "",
                "production.json",
                "production.tmp",
                b"ok",
                PRIVATE_FILE_MODE,
            )
            .unwrap();
        assert_eq!(
            observer.events().len(),
            2,
            "ordinary RootedFs construction must not share an observer"
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    fn fixture_snapshot(root: &Path) -> Vec<(PathBuf, &'static str, u32, Vec<u8>)> {
        fn visit(
            root: &Path,
            directory: &Path,
            snapshot: &mut Vec<(PathBuf, &'static str, u32, Vec<u8>)>,
        ) {
            let mut entries: Vec<_> = fs::read_dir(directory)
                .unwrap()
                .map(|entry| entry.unwrap())
                .collect();
            entries.sort_by_key(|entry| entry.file_name());
            for entry in entries {
                let path = entry.path();
                let relative = path.strip_prefix(root).unwrap().to_owned();
                let metadata = fs::symlink_metadata(&path).unwrap();
                let mode = metadata.permissions().mode() & 0o7777;
                let file_type = metadata.file_type();
                if file_type.is_dir() {
                    snapshot.push((relative, "directory", mode, Vec::new()));
                    visit(root, &path, snapshot);
                } else if file_type.is_file() {
                    snapshot.push((relative, "regular", mode, fs::read(&path).unwrap()));
                } else if file_type.is_symlink() {
                    snapshot.push((
                        relative,
                        "symlink",
                        mode,
                        fs::read_link(&path)
                            .unwrap()
                            .as_os_str()
                            .as_bytes()
                            .to_vec(),
                    ));
                } else {
                    snapshot.push((relative, "other", mode, Vec::new()));
                }
            }
        }

        let mut snapshot = Vec::new();
        visit(root, root, &mut snapshot);
        snapshot
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn rooted_exists_missing_intermediate_is_false() {
        let (_directory, corpus, rooted) = rooted_fixture("exists-missing-intermediate");
        fs::write(corpus.join("sentinel"), b"unchanged\n").unwrap();
        let before = fixture_snapshot(&corpus);

        assert!(!rooted.exists("missing/leaf").unwrap());

        assert_eq!(fixture_snapshot(&corpus), before);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn rooted_exists_missing_leaf_is_false() {
        let (_directory, corpus, rooted) = rooted_fixture("exists-missing-leaf");
        fs::create_dir(corpus.join("present")).unwrap();
        fs::write(corpus.join("present/sentinel"), b"unchanged\n").unwrap();
        let before = fixture_snapshot(&corpus);

        assert!(!rooted.exists("present/missing").unwrap());

        assert_eq!(fixture_snapshot(&corpus), before);
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn rooted_exists_preserves_strict_alias_symlink_and_non_directory_errors() {
        use std::os::unix::fs::symlink;

        let (_directory, corpus, rooted) = rooted_fixture("exists-strict-components");
        fs::create_dir(corpus.join("ExactName")).unwrap();
        fs::create_dir(corpus.join("target")).unwrap();
        symlink("target", corpus.join("directory-link")).unwrap();
        symlink("target", corpus.join("leaf-link")).unwrap();
        fs::write(corpus.join("regular"), b"not a directory\n").unwrap();

        if fs::symlink_metadata(corpus.join("exactname")).is_ok() {
            assert!(rooted.exists("exactname/leaf").is_err());
        }
        assert!(rooted.exists("directory-link/leaf").is_err());
        assert!(rooted.exists("leaf-link").is_err());
        assert!(rooted.exists("regular/leaf").is_err());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn rooted_exists_preserves_strict_non_utf8_sibling_error() {
        let (_directory, corpus, rooted) = rooted_fixture("exists-non-utf8");
        fs::write(corpus.join("present"), b"present\n").unwrap();
        let sibling = corpus.join(std::ffi::OsString::from_vec(vec![
            b'n', b'a', b'm', b'e', 0xff,
        ]));
        if let Err(error) = fs::write(&sibling, b"non-UTF-8 sibling\n") {
            assert_eq!(
                error.raw_os_error(),
                Some(92),
                "unexpected failure creating non-UTF-8 fixture: {error}"
            );
            return;
        }

        assert!(rooted.exists("present").is_err());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn rooted_exists_preserves_strict_permission_and_mode_errors() {
        let (_directory, corpus, rooted) = rooted_fixture("exists-permission-mode");
        let denied = corpus.join("denied");
        fs::create_dir(&denied).unwrap();
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o000)).unwrap();
        let permission_result = rooted.exists("denied/leaf");
        fs::set_permissions(&denied, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(permission_result.is_err());

        let wrong_mode = corpus.join("wrong-mode");
        fs::create_dir(&wrong_mode).unwrap();
        fs::set_permissions(&wrong_mode, fs::Permissions::from_mode(0o777)).unwrap();
        assert!(rooted.exists("wrong-mode/leaf").is_err());
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn supported_target_uses_held_root_and_fixed_modes() {
        assert_eq!(MutationTarget::current(), MutationTarget::AppleSiliconMacOs);
        let directory = TestDirectory::new("fixed-modes");
        let corpus = directory.path().join("corpus");
        fs::create_dir(&corpus).unwrap();
        let location = CorpusLocation::new(directory.path(), &corpus).unwrap();
        let rooted = RootedFs::open_corpus(&location).unwrap();

        let nested = rooted
            .ensure_dir("nested/child", CORPUS_DIRECTORY_MODE)
            .unwrap();
        assert_eq!(nested.kind(), NodeKind::Directory);
        assert_eq!(nested.mode(), CORPUS_DIRECTORY_MODE);
        let file = rooted
            .create_file_exclusive("nested/child/value.json", b"{}\n", CORPUS_FILE_MODE)
            .unwrap();
        assert_eq!(file.kind(), NodeKind::Regular);
        assert_eq!(file.mode(), CORPUS_FILE_MODE);
        assert_eq!(file.link_count(), Some(1));
        assert_eq!(
            rooted
                .read_file("nested/child/value.json", CORPUS_FILE_MODE)
                .unwrap(),
            b"{}\n"
        );
    }

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    #[test]
    fn rooted_authority_rejects_symlink_and_hard_link_mutation_targets() {
        use std::os::unix::fs::symlink;

        let directory = TestDirectory::new("links");
        let corpus = directory.path().join("corpus");
        let outside = directory.path().join("outside");
        fs::create_dir(&corpus).unwrap();
        fs::create_dir(&outside).unwrap();
        let location = CorpusLocation::new(directory.path(), &corpus).unwrap();
        let rooted = RootedFs::open_corpus(&location).unwrap();

        symlink(&outside, corpus.join("alias")).unwrap();
        assert!(
            rooted
                .ensure_dir("alias/child", CORPUS_DIRECTORY_MODE)
                .is_err()
        );
        fs::write(corpus.join("one"), b"one").unwrap();
        fs::hard_link(corpus.join("one"), corpus.join("two")).unwrap();
        assert!(rooted.read_file("one", CORPUS_FILE_MODE).is_err());
    }

    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    #[test]
    fn unsupported_target_stops_before_root_mutation() {
        assert_eq!(MutationTarget::current(), MutationTarget::Unsupported);
        let error = MutationTarget::current()
            .require_supported("test unsupported mutation")
            .unwrap_err();
        assert_eq!(error.kind(), crate::GeneratorErrorKind::UnsupportedPlatform);
        let _ = std::mem::size_of::<RootedFs>();
    }
}
