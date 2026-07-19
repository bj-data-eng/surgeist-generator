use serde::{Deserialize, Serialize};

use crate::core::{ObjectFormat, VerifiedSourceSnapshot};
use crate::{GeneratorError, GeneratorErrorKind, PinnedSource, RelativePath, Result, Sha256Digest};

use super::manifest::{CSSTREE_REPOSITORY, FIXTURE_ROOT, REPORT_RELATIVE, SIDECAR_FILE};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CssImportSidecar {
    schema_version: u8,
    source: PinnedSource,
    object_format: SidecarObjectFormat,
    file_count: usize,
    files: Vec<CssImportFile>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
enum SidecarObjectFormat {
    Sha1,
    Sha256,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CssImportFile {
    pub(super) path: RelativePath,
    pub(super) git_mode: String,
    pub(super) blob_object_id: String,
    pub(super) sha256: Sha256Digest,
}

impl CssImportSidecar {
    pub(super) fn from_snapshot(
        pin: &PinnedSource,
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
                if entry.git_mode != "100644" {
                    return Err(invalid_inventory(format!(
                        "CSSTree fixture is not Git mode 100644: {}",
                        entry.path.as_str()
                    )));
                }
                validate_fixture_path(&entry.path)?;
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
                Ok(CssImportFile {
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
            file_count: files.len(),
            files,
        };
        sidecar.validate()?;
        Ok(sidecar)
    }

    pub(super) fn parse_canonical(bytes: &[u8]) -> Result<Self> {
        let sidecar: Self = serde_json::from_slice(bytes).map_err(|error| {
            GeneratorError::with_source(
                GeneratorErrorKind::InvalidInventory,
                "parse CSS import sidecar",
                "invalid sidecar JSON",
                error,
            )
        })?;
        sidecar.validate()?;
        if sidecar.canonical_bytes()? != bytes {
            return Err(invalid_inventory(
                "CSS import sidecar bytes are not canonical",
            ));
        }
        Ok(sidecar)
    }

    pub(super) fn canonical_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = serde_json::to_vec(self).map_err(|error| {
            GeneratorError::with_source(
                GeneratorErrorKind::InvalidInventory,
                "serialize CSS import sidecar",
                "sidecar serialization failed",
                error,
            )
        })?;
        bytes.push(b'\n');
        Ok(bytes)
    }

    pub(super) fn files(&self) -> &[CssImportFile] {
        &self.files
    }

    fn validate(&self) -> Result<()> {
        if self.schema_version != 1 {
            return Err(invalid_inventory(
                "CSS import sidecar schema_version must be 1",
            ));
        }
        if self.source.label() != "csstree"
            || self.source.repository_url() != CSSTREE_REPOSITORY
            || self.source.source_subdirectory().as_str() != FIXTURE_ROOT
        {
            return Err(invalid_inventory(
                "CSS import sidecar source pin is noncanonical",
            ));
        }
        if self.object_format.object_id_len() != self.source.revision().as_str().len() {
            return Err(invalid_inventory(
                "CSS import sidecar object format differs from revision width",
            ));
        }
        if self.file_count == 0 || self.file_count != self.files.len() {
            return Err(invalid_inventory(
                "CSS import sidecar file_count must be positive and exact",
            ));
        }
        for (index, file) in self.files.iter().enumerate() {
            validate_fixture_path(&file.path)?;
            if file.git_mode != "100644" {
                return Err(invalid_inventory(format!(
                    "CSS import sidecar records non-100644 mode: {}",
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
                    "CSS import sidecar paths must be strictly increasing",
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
    snapshot: &VerifiedSourceSnapshot,
) -> Result<Vec<u8>> {
    CssImportSidecar::from_snapshot(pin, snapshot)?.canonical_bytes()
}

pub(super) fn validate_fixture_path(path: &RelativePath) -> Result<()> {
    RelativePath::with_extension(path.as_str(), "json")
        .map_err(|error| invalid_inventory(error.to_string()))?;
    if [SIDECAR_FILE, REPORT_RELATIVE]
        .iter()
        .any(|reserved| paths_overlap(path.as_str(), reserved))
        || path.as_str().split('/').any(|component| {
            component == ".surgeist-generator" || component.starts_with("._surgeist-")
        })
    {
        return Err(invalid_inventory(format!(
            "reserved CSSTree fixture path: {}",
            path.as_str()
        )));
    }
    Ok(())
}

fn paths_overlap(left: &str, right: &str) -> bool {
    is_same_or_descendant(left, right) || is_same_or_descendant(right, left)
}

fn is_same_or_descendant(path: &str, ancestor: &str) -> bool {
    let path = path.split('/').collect::<Vec<_>>();
    let ancestor = ancestor.split('/').collect::<Vec<_>>();
    path.len() >= ancestor.len()
        && path
            .iter()
            .zip(ancestor)
            .all(|(component, ancestor)| target_components_equal(component, ancestor))
}

fn target_components_equal(left: &str, right: &str) -> bool {
    #[cfg(target_os = "macos")]
    {
        left.eq_ignore_ascii_case(right)
    }
    #[cfg(not(target_os = "macos"))]
    {
        left == right
    }
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
        "validate CSS import sidecar",
        detail,
    )
}
