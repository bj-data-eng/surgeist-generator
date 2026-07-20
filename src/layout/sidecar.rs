use serde::{Deserialize, Serialize};

use crate::core::{ObjectFormat, VerifiedSourceSnapshot};
use crate::{GeneratorError, GeneratorErrorKind, PinnedSource, RelativePath, Result, Sha256Digest};

use super::manifest::{
    EXCLUDED_DIRECTORIES, SIDECAR_FILE, TAFFY_REPOSITORY, TAFFY_SOURCE_DIRECTORY,
    paths_target_equal,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TaffyImportSidecar {
    schema_version: u8,
    source: PinnedSource,
    object_format: SidecarObjectFormat,
    source_file_count: usize,
    excluded_destination_dirs: Vec<String>,
    imported_file_count: usize,
    files: Vec<TaffyImportFile>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum SidecarObjectFormat {
    Sha1,
    Sha256,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TaffyImportFile {
    pub(super) path: RelativePath,
    pub(super) git_mode: String,
    pub(super) blob_object_id: String,
    pub(super) sha256: Sha256Digest,
}

impl TaffyImportSidecar {
    pub(super) fn from_snapshot(
        pin: &PinnedSource,
        source_file_count: usize,
        snapshot: &VerifiedSourceSnapshot,
    ) -> Result<Self> {
        let object_format = SidecarObjectFormat::from_core(snapshot.object_format);
        if object_format.object_id_len() != pin.revision().as_str().len() {
            return Err(invalid_inventory(
                "snapshot object format differs from the pinned revision",
            ));
        }
        let files = snapshot
            .entries
            .iter()
            .map(|entry| {
                validate_fixture_path(&entry.path)?;
                if entry.git_mode != "100644" {
                    return Err(invalid_inventory(format!(
                        "Taffy fixture is not Git mode 100644: {}",
                        entry.path.as_str()
                    )));
                }
                validate_object_id(
                    &entry.blob_object_id,
                    object_format,
                    "snapshot blob object ID",
                )?;
                let digest = Sha256Digest::from_bytes(&entry.bytes);
                if digest != entry.digest {
                    return Err(invalid_inventory(format!(
                        "snapshot digest differs from immutable bytes: {}",
                        entry.path.as_str()
                    )));
                }
                Ok(TaffyImportFile {
                    path: entry.path.clone(),
                    git_mode: entry.git_mode.clone(),
                    blob_object_id: entry.blob_object_id.clone(),
                    sha256: digest,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        let sidecar = Self {
            schema_version: 1,
            source: pin.clone(),
            object_format,
            source_file_count,
            excluded_destination_dirs: EXCLUDED_DIRECTORIES
                .into_iter()
                .map(str::to_owned)
                .collect(),
            imported_file_count: files.len(),
            files,
        };
        sidecar.validate()?;
        Ok(sidecar)
    }

    pub(super) fn parse_canonical(bytes: &[u8]) -> Result<Self> {
        let sidecar: Self = serde_json::from_slice(bytes).map_err(|error| {
            GeneratorError::with_source(
                GeneratorErrorKind::InvalidInventory,
                "parse Taffy import sidecar",
                "invalid sidecar JSON",
                error,
            )
        })?;
        sidecar.validate()?;
        if sidecar.canonical_bytes()? != bytes {
            return Err(invalid_inventory(
                "Taffy import sidecar bytes are not canonical",
            ));
        }
        Ok(sidecar)
    }

    pub(super) fn canonical_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = serde_json::to_vec(self).map_err(|error| {
            GeneratorError::with_source(
                GeneratorErrorKind::InvalidInventory,
                "serialize Taffy import sidecar",
                "sidecar serialization failed",
                error,
            )
        })?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    pub(super) fn files(&self) -> &[TaffyImportFile] {
        &self.files
    }

    pub(super) fn revision(&self) -> &crate::SourceRevision {
        self.source.revision()
    }

    pub(super) const fn source_file_count(&self) -> usize {
        self.source_file_count
    }

    fn validate(&self) -> Result<()> {
        if self.schema_version != 1 {
            return Err(invalid_inventory(
                "Taffy import sidecar schema_version must be 1",
            ));
        }
        if self.source.label() != "taffy"
            || self.source.repository_url() != TAFFY_REPOSITORY
            || self.source.source_subdirectory().as_str() != TAFFY_SOURCE_DIRECTORY
        {
            return Err(invalid_inventory(
                "Taffy import sidecar source pin is noncanonical",
            ));
        }
        if self.object_format.object_id_len() != self.source.revision().as_str().len() {
            return Err(invalid_inventory(
                "Taffy import sidecar object format differs from revision width",
            ));
        }
        if self.source_file_count == 0
            || self.imported_file_count != self.files.len()
            || self.source_file_count < self.imported_file_count
        {
            return Err(invalid_inventory(
                "Taffy import sidecar source/imported counts are inconsistent",
            ));
        }
        if self.excluded_destination_dirs
            != EXCLUDED_DIRECTORIES
                .into_iter()
                .map(str::to_owned)
                .collect::<Vec<_>>()
        {
            return Err(invalid_inventory(
                "Taffy import sidecar exclusions are noncanonical",
            ));
        }
        for (index, file) in self.files.iter().enumerate() {
            validate_fixture_path(&file.path)?;
            if file.git_mode != "100644" {
                return Err(invalid_inventory(format!(
                    "Taffy import sidecar records non-100644 mode: {}",
                    file.path.as_str()
                )));
            }
            validate_object_id(
                &file.blob_object_id,
                self.object_format,
                "sidecar blob object ID",
            )?;
            if index > 0 && self.files[index - 1].path >= file.path {
                return Err(invalid_inventory(
                    "Taffy import sidecar paths must be strictly increasing",
                ));
            }
        }
        Ok(())
    }
}

impl SidecarObjectFormat {
    const fn from_core(value: ObjectFormat) -> Self {
        match value {
            ObjectFormat::Sha1 => Self::Sha1,
            ObjectFormat::Sha256 => Self::Sha256,
        }
    }

    const fn object_id_len(self) -> usize {
        match self {
            Self::Sha1 => 40,
            Self::Sha256 => 64,
        }
    }
}

#[cfg(test)]
pub(super) fn canonical_bytes(
    pin: &PinnedSource,
    source_file_count: usize,
    snapshot: &VerifiedSourceSnapshot,
) -> Result<Vec<u8>> {
    TaffyImportSidecar::from_snapshot(pin, source_file_count, snapshot)?.canonical_bytes()
}

pub(super) fn validate_fixture_path(path: &RelativePath) -> Result<()> {
    RelativePath::with_extension(path.as_str(), "html")
        .map_err(|error| invalid_inventory(error.to_string()))?;
    let first = path
        .as_str()
        .split('/')
        .next()
        .expect("RelativePath always has one component");
    if EXCLUDED_DIRECTORIES
        .iter()
        .any(|excluded| paths_target_equal(first, excluded))
        || paths_target_equal(path.as_str(), SIDECAR_FILE)
        || path.as_str().split('/').any(|component| {
            component == ".surgeist-generator" || component.starts_with("._surgeist-")
        })
    {
        return Err(invalid_inventory(format!(
            "reserved or excluded Taffy fixture path: {}",
            path.as_str()
        )));
    }
    Ok(())
}

fn validate_object_id(value: &str, object_format: SidecarObjectFormat, label: &str) -> Result<()> {
    if value.len() != object_format.object_id_len()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(invalid_inventory(format!(
            "{label} is not a full lowercase object ID"
        )));
    }
    Ok(())
}

fn invalid_inventory(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidInventory,
        "validate Taffy import sidecar",
        detail,
    )
}
