use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{GeneratorError, GeneratorErrorKind, RelativePath, Result, Sha256Digest};

use super::fs::{
    CORPUS_DIRECTORY_MODE, CORPUS_FILE_MODE, HeldIdentity, NodeKind, PRIVATE_DIRECTORY_MODE,
    PRIVATE_FILE_MODE, RootedFs,
};

const INVENTORY_SCHEMA_VERSION: u8 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum InventoryPolicy {
    FinalCorpus,
    ConstructionCorpus,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct InventoryEntry {
    path: RelativePath,
    identity: HeldIdentity,
    length: Option<u64>,
    digest: Option<Sha256Digest>,
    link_target: Option<Vec<u8>>,
}

impl InventoryEntry {
    pub(crate) fn directory(path: RelativePath, identity: HeldIdentity) -> Result<Self> {
        if identity.kind() != NodeKind::Directory || identity.link_count().is_some() {
            return Err(inventory_error(
                "construct inventory directory",
                format!("directory identity is invalid: {}", path.as_str()),
            ));
        }
        Ok(Self {
            path,
            identity,
            length: None,
            digest: None,
            link_target: None,
        })
    }

    pub(crate) fn regular(
        path: RelativePath,
        identity: HeldIdentity,
        bytes: &[u8],
    ) -> Result<Self> {
        if identity.kind() != NodeKind::Regular || identity.link_count() != Some(1) {
            return Err(inventory_error(
                "construct inventory regular file",
                format!("regular-file identity is invalid: {}", path.as_str()),
            ));
        }
        Ok(Self {
            path,
            identity,
            length: Some(u64::try_from(bytes.len()).map_err(|_| {
                inventory_error(
                    "construct inventory regular file",
                    "regular-file length exceeds u64",
                )
            })?),
            digest: Some(Sha256Digest::from_bytes(bytes)),
            link_target: None,
        })
    }

    pub(crate) const fn path(&self) -> &RelativePath {
        &self.path
    }

    pub(crate) const fn identity(&self) -> &HeldIdentity {
        &self.identity
    }

    #[cfg(any(feature = "css-corpus", feature = "layout-browser"))]
    pub(crate) const fn digest(&self) -> Option<&Sha256Digest> {
        self.digest.as_ref()
    }

    fn validate_shape(&self) -> Result<()> {
        match self.identity.kind() {
            NodeKind::Directory => {
                if self.identity.link_count().is_some()
                    || self.length.is_some()
                    || self.digest.is_some()
                    || self.link_target.is_some()
                {
                    return Err(inventory_error(
                        "validate inventory entry",
                        format!("directory carries file metadata: {}", self.path.as_str()),
                    ));
                }
            }
            NodeKind::Regular => {
                if self.identity.link_count() != Some(1)
                    || self.length.is_none()
                    || self.digest.is_none()
                    || self.link_target.is_some()
                {
                    return Err(inventory_error(
                        "validate inventory entry",
                        format!(
                            "regular-file metadata is incomplete: {}",
                            self.path.as_str()
                        ),
                    ));
                }
            }
            NodeKind::Symlink => {
                let Some(target) = &self.link_target else {
                    return Err(inventory_error(
                        "validate inventory entry",
                        format!("symbolic-link target is absent: {}", self.path.as_str()),
                    ));
                };
                if self.identity.link_count() != Some(1)
                    || self.length != u64::try_from(target.len()).ok()
                    || self.digest.as_ref() != Some(&Sha256Digest::from_bytes(target))
                {
                    return Err(inventory_error(
                        "validate inventory entry",
                        format!("symbolic-link metadata differs: {}", self.path.as_str()),
                    ));
                }
            }
        }
        Ok(())
    }

    #[cfg(test)]
    fn test_directory(path: &str, inode: u64) -> Self {
        Self::directory(
            RelativePath::new(path).unwrap(),
            HeldIdentity::synthetic(NodeKind::Directory, inode, CORPUS_DIRECTORY_MODE, None),
        )
        .unwrap()
    }

    #[cfg(test)]
    fn test_regular(path: &str, inode: u64, bytes: &[u8]) -> Self {
        Self::regular(
            RelativePath::new(path).unwrap(),
            HeldIdentity::synthetic(NodeKind::Regular, inode, CORPUS_FILE_MODE, Some(1)),
            bytes,
        )
        .unwrap()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Inventory {
    schema_version: u8,
    root: HeldIdentity,
    entries: Vec<InventoryEntry>,
}

impl Inventory {
    pub(crate) fn new(
        root: HeldIdentity,
        mut entries: Vec<InventoryEntry>,
        policy: InventoryPolicy,
    ) -> Result<Self> {
        if root.kind() != NodeKind::Directory || root.link_count().is_some() {
            return Err(inventory_error(
                "construct tree inventory",
                "inventory root is not a directory identity",
            ));
        }
        entries.sort_by(|left, right| left.path.cmp(&right.path));
        for window in entries.windows(2) {
            if window[0].path == window[1].path {
                return Err(inventory_error(
                    "construct tree inventory",
                    format!("duplicate path: {}", window[0].path.as_str()),
                ));
            }
        }
        let entry_paths: BTreeMap<_, _> = entries
            .iter()
            .map(|entry| (entry.path.as_str(), entry.identity.kind()))
            .collect();
        for entry in &entries {
            entry.validate_shape()?;
            validate_policy_entry(entry, policy)?;
            if let Some((parent, _)) = entry.path.as_str().rsplit_once('/')
                && entry_paths.get(parent) != Some(&NodeKind::Directory)
            {
                return Err(inventory_error(
                    "construct tree inventory",
                    format!("missing parent directory inventory: {parent}"),
                ));
            }
        }
        validate_policy_identity(&root, policy, true, "inventory root")?;
        Ok(Self {
            schema_version: INVENTORY_SCHEMA_VERSION,
            root,
            entries,
        })
    }

    pub(crate) fn from_json(bytes: &[u8], policy: InventoryPolicy) -> Result<Self> {
        let inventory: Self = serde_json::from_slice(bytes).map_err(|error| {
            transaction_error(
                "parse durable tree inventory",
                format!("invalid inventory JSON: {error}"),
            )
        })?;
        if inventory.schema_version != INVENTORY_SCHEMA_VERSION {
            return Err(transaction_error(
                "parse durable tree inventory",
                format!("unsupported schema version: {}", inventory.schema_version),
            ));
        }
        Self::new(inventory.root, inventory.entries, policy)
            .map_err(|error| transaction_error("parse durable tree inventory", error.to_string()))
    }

    pub(crate) fn scan(
        rooted: &RootedFs,
        root_path: &str,
        policy: InventoryPolicy,
    ) -> Result<Option<Self>> {
        let Some(root) = rooted.identity_at(root_path)? else {
            return Ok(None);
        };
        validate_policy_identity(&root, policy, true, root_path)?;
        let mut entries = Vec::new();
        scan_directory(rooted, root_path, "", policy, &mut entries)?;
        Self::new(root, entries, policy).map(Some)
    }

    pub(crate) const fn root(&self) -> &HeldIdentity {
        &self.root
    }

    pub(crate) fn entries(&self) -> &[InventoryEntry] {
        &self.entries
    }

    #[cfg(any(feature = "css-corpus", feature = "layout-browser"))]
    pub(crate) fn find(&self, path: &RelativePath) -> Option<&InventoryEntry> {
        self.entries
            .binary_search_by(|entry| entry.path.cmp(path))
            .ok()
            .map(|index| &self.entries[index])
    }

    pub(crate) fn canonical_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = serde_json::to_vec(self).map_err(|error| {
            transaction_error(
                "serialize durable tree inventory",
                format!("inventory serialization failed: {error}"),
            )
        })?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    pub(crate) fn digest(&self) -> Result<Sha256Digest> {
        Ok(Sha256Digest::from_bytes(self.canonical_bytes()?))
    }

    pub(crate) fn is_exact_remaining_subset_of(&self, original: &Self) -> Result<()> {
        if !self.root.same_object(&original.root)
            || self.root.mode() != original.root.mode()
            || self.root.owner() != original.root.owner()
        {
            return Err(transaction_error(
                "validate recovery inventory subset",
                "tree root identity or policy changed",
            ));
        }
        let original_entries: BTreeMap<_, _> = original
            .entries
            .iter()
            .map(|entry| (entry.path.as_str(), entry))
            .collect();
        for entry in &self.entries {
            let Some(expected) = original_entries.get(entry.path.as_str()) else {
                return Err(transaction_error(
                    "validate recovery inventory subset",
                    format!("unknown remaining path: {}", entry.path.as_str()),
                ));
            };
            if !entries_match_for_recovery(entry, expected) {
                return Err(transaction_error(
                    "validate recovery inventory subset",
                    format!("remaining identity changed: {}", entry.path.as_str()),
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn removal_order(&self) -> Vec<&InventoryEntry> {
        let mut entries: Vec<_> = self.entries.iter().collect();
        entries.sort_by(|left, right| {
            let left_depth = left
                .path
                .as_str()
                .bytes()
                .filter(|byte| *byte == b'/')
                .count();
            let right_depth = right
                .path
                .as_str()
                .bytes()
                .filter(|byte| *byte == b'/')
                .count();
            right_depth
                .cmp(&left_depth)
                .then_with(|| right.path.cmp(&left.path))
        });
        entries
    }
}

fn scan_directory(
    rooted: &RootedFs,
    root_path: &str,
    relative: &str,
    policy: InventoryPolicy,
    entries: &mut Vec<InventoryEntry>,
) -> Result<()> {
    let directory = if relative.is_empty() {
        root_path.to_owned()
    } else {
        format!("{root_path}/{relative}")
    };
    for name in rooted.list_dir(&directory)? {
        let child_relative = if relative.is_empty() {
            name
        } else {
            format!("{relative}/{name}")
        };
        let child_rooted = format!("{root_path}/{child_relative}");
        let path = RelativePath::new(&child_relative)?;
        let identity = rooted.identity_at(&child_rooted)?.ok_or_else(|| {
            transaction_error(
                "scan rooted tree inventory",
                format!("entry disappeared: {child_rooted}"),
            )
        })?;
        validate_policy_identity(&identity, policy, false, &child_rooted)?;
        match identity.kind() {
            NodeKind::Directory => {
                entries.push(InventoryEntry::directory(path, identity)?);
                scan_directory(rooted, root_path, &child_relative, policy, entries)?;
            }
            NodeKind::Regular => {
                let bytes = rooted.read_file(&child_rooted, identity.mode())?;
                entries.push(InventoryEntry::regular(path, identity, &bytes)?);
            }
            NodeKind::Symlink => {
                return Err(inventory_error(
                    "scan rooted tree inventory",
                    format!("symbolic link is not allowed: {child_rooted}"),
                ));
            }
        }
    }
    Ok(())
}

fn validate_policy_entry(entry: &InventoryEntry, policy: InventoryPolicy) -> Result<()> {
    validate_policy_identity(
        &entry.identity,
        policy,
        entry.identity.kind() == NodeKind::Directory,
        entry.path.as_str(),
    )
}

fn validate_policy_identity(
    identity: &HeldIdentity,
    policy: InventoryPolicy,
    directory_position: bool,
    path: &str,
) -> Result<()> {
    let valid = match (policy, identity.kind()) {
        (InventoryPolicy::FinalCorpus, NodeKind::Directory) => {
            identity.mode() == CORPUS_DIRECTORY_MODE
        }
        (InventoryPolicy::FinalCorpus, NodeKind::Regular) => {
            identity.mode() == CORPUS_FILE_MODE && identity.link_count() == Some(1)
        }
        (InventoryPolicy::ConstructionCorpus, NodeKind::Directory) => {
            matches!(
                identity.mode(),
                PRIVATE_DIRECTORY_MODE | CORPUS_DIRECTORY_MODE
            )
        }
        (InventoryPolicy::ConstructionCorpus, NodeKind::Regular) => {
            matches!(identity.mode(), PRIVATE_FILE_MODE | CORPUS_FILE_MODE)
                && identity.link_count() == Some(1)
        }
        (_, NodeKind::Symlink) => false,
    };
    if !valid || (directory_position && identity.kind() != NodeKind::Directory) {
        return Err(inventory_error(
            "validate tree inventory policy",
            format!(
                "wrong type, mode, or link count at {path}: {:?} {:#o} {:?}",
                identity.kind(),
                identity.mode(),
                identity.link_count()
            ),
        ));
    }
    Ok(())
}

fn entries_match_for_recovery(left: &InventoryEntry, right: &InventoryEntry) -> bool {
    left.path == right.path
        && left.identity.matches_recovery(&right.identity)
        && left.length == right.length
        && left.digest == right.digest
        && left.link_target == right.link_target
}

fn inventory_error(operation: &str, detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(GeneratorErrorKind::InvalidInventory, operation, detail)
}

fn transaction_error(operation: &str, detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(GeneratorErrorKind::ArtifactTransaction, operation, detail)
}

#[cfg(test)]
mod tests {
    use super::{Inventory, InventoryEntry, InventoryPolicy};
    use crate::core::fs::{CORPUS_DIRECTORY_MODE, HeldIdentity, NodeKind};

    fn root(inode: u64) -> HeldIdentity {
        HeldIdentity::synthetic(NodeKind::Directory, inode, CORPUS_DIRECTORY_MODE, None)
    }

    #[test]
    fn canonical_inventory_uses_nullable_directory_links_and_exact_subsets() {
        let inventory = Inventory::new(
            root(1),
            vec![
                InventoryEntry::test_regular("nested/value.json", 3, b"value"),
                InventoryEntry::test_directory("nested", 2),
            ],
            InventoryPolicy::FinalCorpus,
        )
        .unwrap();
        assert_eq!(inventory.entries()[0].path().as_str(), "nested");
        assert_eq!(inventory.entries()[0].identity().link_count(), None);
        assert!(inventory.canonical_bytes().unwrap().ends_with(b"\n"));
        assert!(inventory.is_exact_remaining_subset_of(&inventory).is_ok());

        let remaining = Inventory::new(
            root(1),
            vec![InventoryEntry::test_directory("nested", 2)],
            InventoryPolicy::FinalCorpus,
        )
        .unwrap();
        remaining.is_exact_remaining_subset_of(&inventory).unwrap();
    }

    #[test]
    fn recovery_subset_rejects_replacements_and_unknown_children() {
        let original = Inventory::new(
            root(1),
            vec![InventoryEntry::test_regular("value.json", 2, b"old")],
            InventoryPolicy::FinalCorpus,
        )
        .unwrap();
        let replacement = Inventory::new(
            root(1),
            vec![InventoryEntry::test_regular("value.json", 3, b"old")],
            InventoryPolicy::FinalCorpus,
        )
        .unwrap();
        assert!(replacement.is_exact_remaining_subset_of(&original).is_err());

        let unknown = Inventory::new(
            root(1),
            vec![InventoryEntry::test_regular("unknown.json", 4, b"new")],
            InventoryPolicy::FinalCorpus,
        )
        .unwrap();
        assert!(unknown.is_exact_remaining_subset_of(&original).is_err());
    }

    #[test]
    fn final_policy_rejects_wrong_modes_before_intent() {
        let wrong = InventoryEntry::regular(
            crate::RelativePath::new("value.json").unwrap(),
            HeldIdentity::synthetic(NodeKind::Regular, 2, 0o755, Some(1)),
            b"value",
        )
        .unwrap();
        let error = Inventory::new(root(1), vec![wrong], InventoryPolicy::FinalCorpus).unwrap_err();
        assert_eq!(error.kind(), crate::GeneratorErrorKind::InvalidInventory);
    }
}
