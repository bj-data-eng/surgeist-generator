#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
use std::path::Component;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{CorpusLocation, GeneratorError, GeneratorErrorKind, Result};

pub(crate) const PRIVATE_DIRECTORY_MODE: u32 = 0o700;
pub(crate) const PRIVATE_FILE_MODE: u32 = 0o600;
pub(crate) const CORPUS_DIRECTORY_MODE: u32 = 0o755;
pub(crate) const CORPUS_FILE_MODE: u32 = 0o644;

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

pub(crate) struct RootedFs {
    canonical_root: PathBuf,
    identity: HeldIdentity,
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    root: rustix::fd::OwnedFd,
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
                            Ok(()) => true,
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
                    fsync(&child).map_err(|source| {
                        io_error(
                            "sync rooted directory",
                            &self.canonical_root.join(&current_path),
                            source,
                        )
                    })?;
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
            let directory = open_directory_at(&parent, name, "open exclusive rooted directory")?;
            fchmod(&directory, checked_mode(final_mode)?).map_err(|source| {
                io_error(
                    "set exclusive rooted directory mode",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
            fsync(&directory).map_err(|source| {
                io_error(
                    "sync exclusive rooted directory",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
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
            fsync(&parent).map_err(|source| {
                io_error(
                    "sync exclusive rooted directory parent",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
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
        self.revalidate_root()?;
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            use rustix::fs::{OFlags, fchmod, fsync, openat};
            use std::fs::File;
            use std::io::Write;

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
            let mut file = File::from(opened);
            file.write_all(bytes).map_err(|source| {
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
            file.flush().map_err(|source| {
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
            let identity = identity_from_fd(&file, "inspect created rooted regular file")?;
            require_regular_policy(
                &identity,
                final_mode,
                "validate created rooted regular file",
            )?;
            require_same_mount(&self.identity, &identity, "validate created file mount")?;
            fsync(&parent).map_err(|source| {
                io_error(
                    "sync rooted regular file parent",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
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
        self.revalidate_root()?;
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            use rustix::fs::{OFlags, fchmod, fsync, openat};
            use std::io::Write;

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
            let mut file = std::fs::File::from(opened);
            file.write_all(bytes).map_err(|source| {
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
            file.flush().map_err(|source| {
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
            let identity = identity_from_fd(&file, "inspect created rooted file handle")?;
            require_regular_policy(&identity, final_mode, "validate created rooted file handle")?;
            require_same_mount(
                &self.identity,
                &identity,
                "validate rooted file-handle mount",
            )?;
            fsync(&parent).map_err(|source| {
                io_error(
                    "sync rooted file-handle parent",
                    &self.canonical_root.join(relative),
                    source,
                )
            })?;
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

    pub(crate) fn rename_exclusive(&self, from: &str, to: &str) -> Result<()> {
        self.rename_with(from, to, RenameMode::Exclusive, None, None)
    }

    pub(crate) fn rename_exclusive_bound(
        &self,
        from: &str,
        to: &str,
        expected_from: &HeldIdentity,
    ) -> Result<()> {
        self.rename_with(from, to, RenameMode::Exclusive, Some(expected_from), None)
    }

    pub(crate) fn rename_swap(&self, from: &str, to: &str) -> Result<()> {
        self.rename_with(from, to, RenameMode::Swap, None, None)
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
            fsync(&to_parent).map_err(|source| {
                io_error(
                    "sync rooted rename destination parent",
                    &self.canonical_root.join(to),
                    source,
                )
            })
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

    fn remove_exact(&self, relative: &str, expected: &HeldIdentity, directory: bool) -> Result<()> {
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
            fsync(&parent).map_err(|source| {
                io_error(
                    "sync rooted unlink parent",
                    &self.canonical_root.join(relative),
                    source,
                )
            })
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = (relative, expected, directory);
            MutationTarget::Unsupported.require_supported("remove rooted object")
        }
    }

    pub(crate) fn sync_dir(&self, relative: &str) -> Result<()> {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            let directory = self.open_dir(relative)?;
            rustix::fs::fsync(&directory).map_err(|source| {
                io_error(
                    "sync rooted directory",
                    &self.canonical_root.join(relative),
                    source,
                )
            })
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = relative;
            MutationTarget::Unsupported.require_supported("sync rooted directory")
        }
    }

    pub(crate) fn probe_rename_flags(&self, journal_dir: &str, token: &str) -> Result<()> {
        let left = joined(journal_dir, &format!("probe-left-{token}"));
        let right = joined(journal_dir, &format!("probe-right-{token}"));
        let moved = joined(journal_dir, &format!("probe-moved-{token}"));
        let left_identity = self.ensure_dir(&left, PRIVATE_DIRECTORY_MODE)?;
        let right_identity = self.ensure_dir(&right, PRIVATE_DIRECTORY_MODE)?;
        if let Err(error) = self.rename_exclusive(&left, &moved) {
            let cleanup = self
                .remove_dir_exact(&left, &left_identity)
                .and_then(|_| self.remove_dir_exact(&right, &right_identity));
            return match cleanup {
                Ok(()) => Err(GeneratorError::new(
                    GeneratorErrorKind::UnsupportedPlatform,
                    "probe rooted exclusive rename",
                    error.to_string(),
                )),
                Err(cleanup_error) => Err(transaction_error(
                    "clean rooted exclusive-rename probe",
                    cleanup_error.to_string(),
                )),
            };
        }
        let moved_identity = self
            .identity_at(&moved)?
            .ok_or_else(|| transaction_error("probe rooted rename", "moved probe disappeared"))?;
        if let Err(error) = self.rename_swap(&moved, &right) {
            let cleanup = self
                .remove_dir_exact(&moved, &moved_identity)
                .and_then(|_| self.remove_dir_exact(&right, &right_identity));
            return match cleanup {
                Ok(()) => Err(GeneratorError::new(
                    GeneratorErrorKind::UnsupportedPlatform,
                    "probe rooted swap rename",
                    error.to_string(),
                )),
                Err(cleanup_error) => Err(transaction_error(
                    "clean rooted swap-rename probe",
                    cleanup_error.to_string(),
                )),
            };
        }
        let right_after = self.identity_at(&right)?.ok_or_else(|| {
            transaction_error("probe rooted swap rename", "right probe disappeared")
        })?;
        let moved_after = self.identity_at(&moved)?.ok_or_else(|| {
            transaction_error("probe rooted swap rename", "moved probe disappeared")
        })?;
        self.remove_dir_exact(&right, &right_after)?;
        self.remove_dir_exact(&moved, &moved_after)?;
        self.sync_dir(journal_dir)
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
    use std::fs;
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    use std::os::unix::fs::PermissionsExt;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{CORPUS_DIRECTORY_MODE, CORPUS_FILE_MODE, MutationTarget, NodeKind, RootedFs};
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
