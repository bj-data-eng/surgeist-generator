use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::core::{
    ArtifactPlan, ArtifactReservation, CORPUS_FILE_MODE, Domain, GenerationCheck, GenerationLease,
    Inventory, InventoryPolicy, NodeKind, ProtectedSource, ProtectedSourceDisjointness,
    ProtectedTreeEntryKind, PublicationInventory, PublicationPolicy, RootedFs, SnapshotEntry,
    VerifiedSourceSnapshot,
};
use crate::{
    CorpusLocation, GeneratorError, GeneratorErrorKind, PinnedSource, RelativePath, Result,
    RunScope, Sha256Digest,
};

/// Caller-owned source selection and its disjoint imported/authored partition.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawSpec")]
pub struct SourceImportSpec {
    pin: PinnedSource,
    destination: RelativePath,
    sidecar: RelativePath,
    extension: String,
    expected_source_files: usize,
    excluded_directories: Vec<RelativePath>,
    authored_files: Vec<RelativePath>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSpec {
    pin: PinnedSource,
    destination: RelativePath,
    sidecar: RelativePath,
    extension: String,
    expected_source_files: usize,
    excluded_directories: Vec<RelativePath>,
    authored_files: Vec<RelativePath>,
}
impl TryFrom<RawSpec> for SourceImportSpec {
    type Error = GeneratorError;
    fn try_from(raw: RawSpec) -> Result<Self> {
        Self::new(
            raw.pin,
            raw.destination,
            raw.sidecar,
            raw.extension,
            raw.expected_source_files,
            raw.excluded_directories,
            raw.authored_files,
        )
    }
}
impl SourceImportSpec {
    /// Exclusions and authored files are relative to `destination`; excluded
    /// directories remain available for the caller's explicitly authored files.
    pub fn new(
        pin: PinnedSource,
        destination: RelativePath,
        sidecar: RelativePath,
        extension: String,
        expected_source_files: usize,
        mut excluded_directories: Vec<RelativePath>,
        mut authored_files: Vec<RelativePath>,
    ) -> Result<Self> {
        if !crate::core::validate_generated_extension(&extension) || expected_source_files == 0 {
            return Err(invalid(
                "source import requires a canonical extension and positive source count",
            ));
        }
        for path in [&destination, &sidecar]
            .into_iter()
            .chain(&excluded_directories)
            .chain(&authored_files)
        {
            validate_path(path)?;
        }
        excluded_directories.sort();
        authored_files.sort();
        validate_distinct(&excluded_directories)?;
        validate_distinct(&authored_files)?;
        let declared_files = authored_files
            .iter()
            .cloned()
            .chain([sidecar.clone()])
            .collect::<Vec<_>>();
        validate_distinct(&declared_files)?;
        for path in &authored_files {
            RelativePath::with_extension(path.as_str(), &extension)?;
            if aliases(path, &sidecar) {
                return Err(invalid("authored path aliases the import attestation"));
            }
        }
        Ok(Self {
            pin,
            destination,
            sidecar,
            extension,
            expected_source_files,
            excluded_directories,
            authored_files,
        })
    }
    #[must_use]
    pub const fn pin(&self) -> &PinnedSource {
        &self.pin
    }
    #[must_use]
    pub const fn destination(&self) -> &RelativePath {
        &self.destination
    }
    #[must_use]
    pub const fn sidecar(&self) -> &RelativePath {
        &self.sidecar
    }
    #[must_use]
    pub fn extension(&self) -> &str {
        &self.extension
    }
    #[must_use]
    pub const fn expected_source_files(&self) -> usize {
        self.expected_source_files
    }
    #[must_use]
    pub fn excluded_directories(&self) -> &[RelativePath] {
        &self.excluded_directories
    }
    #[must_use]
    pub fn authored_files(&self) -> &[RelativePath] {
        &self.authored_files
    }
    fn excluded(&self, path: &RelativePath) -> bool {
        self.excluded_directories.iter().any(|excluded| {
            aliases(path, excluded)
                || path
                    .as_str()
                    .to_ascii_lowercase()
                    .starts_with(&format!("{}/", excluded.as_str().to_ascii_lowercase()))
        })
    }
}

/// One immutable imported file, including the upstream object and local bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportedSourceFile {
    path: RelativePath,
    git_mode: String,
    blob_object_id: String,
    sha256: Sha256Digest,
}
impl ImportedSourceFile {
    #[must_use]
    pub const fn path(&self) -> &RelativePath {
        &self.path
    }
    #[must_use]
    pub fn git_mode(&self) -> &str {
        &self.git_mode
    }
    #[must_use]
    pub fn blob_object_id(&self) -> &str {
        &self.blob_object_id
    }
    #[must_use]
    pub const fn sha256(&self) -> &Sha256Digest {
        &self.sha256
    }
}

/// A verified import receipt and the exact input bytes observed by a successful
/// import or check. The ephemeral input witness is not serialized into the receipt.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct SourceAttestation {
    schema_version: u8,
    specification: SourceImportSpec,
    files: Vec<ImportedSourceFile>,
    #[serde(skip)]
    verified_inputs: BTreeMap<RelativePath, Sha256Digest>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawAttestation {
    schema_version: u8,
    specification: SourceImportSpec,
    files: Vec<ImportedSourceFile>,
}

/// The source pin, import policy, and immutable files recorded by a canonical
/// receipt. The corpus-relative sidecar and digest bind this projection to bytes
/// that a generation request must independently capture and authenticate.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "RawProvenance")]
pub struct SourceImportProvenance {
    schema_version: u8,
    specification: SourceImportSpec,
    files: Vec<ImportedSourceFile>,
    sidecar: RelativePath,
    receipt_sha256: Sha256Digest,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawProvenance {
    schema_version: u8,
    specification: SourceImportSpec,
    files: Vec<ImportedSourceFile>,
    sidecar: RelativePath,
    receipt_sha256: Sha256Digest,
}

impl TryFrom<RawProvenance> for SourceImportProvenance {
    type Error = GeneratorError;

    fn try_from(raw: RawProvenance) -> Result<Self> {
        let receipt = SourceAttestation {
            schema_version: raw.schema_version,
            specification: raw.specification,
            files: raw.files,
            verified_inputs: BTreeMap::new(),
        };
        receipt.validate()?;
        let value = receipt.provenance()?;
        if value.sidecar != raw.sidecar || value.receipt_sha256 != raw.receipt_sha256 {
            return Err(invalid(
                "source provenance does not match its canonical receipt",
            ));
        }
        Ok(value)
    }
}

impl SourceImportProvenance {
    #[must_use]
    pub const fn sidecar(&self) -> &RelativePath {
        &self.sidecar
    }
    #[must_use]
    pub const fn receipt_sha256(&self) -> &Sha256Digest {
        &self.receipt_sha256
    }
    #[must_use]
    pub const fn specification(&self) -> &SourceImportSpec {
        &self.specification
    }
    #[must_use]
    pub fn files(&self) -> &[ImportedSourceFile] {
        &self.files
    }
}

impl SourceAttestation {
    #[must_use]
    pub const fn source(&self) -> &PinnedSource {
        &self.specification.pin
    }
    #[must_use]
    pub const fn specification(&self) -> &SourceImportSpec {
        &self.specification
    }
    #[must_use]
    pub fn files(&self) -> &[ImportedSourceFile] {
        &self.files
    }
    /// Returns the complete verified fixture partition, relative to the destination.
    #[must_use]
    pub fn fixture_paths(&self) -> Vec<RelativePath> {
        self.files
            .iter()
            .map(|file| file.path.clone())
            .chain(self.specification.authored_files.iter().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = serde_json::to_vec(self).map_err(|error| invalid(error.to_string()))?;
        bytes.push(b'\n');
        Ok(bytes)
    }
    /// Returns the verified source receipt and fixture digests, relative to the corpus.
    #[must_use]
    pub fn verified_inputs(&self) -> BTreeMap<RelativePath, Sha256Digest> {
        self.verified_inputs.clone()
    }
    pub fn digest(&self) -> Result<Sha256Digest> {
        self.canonical_bytes().map(Sha256Digest::from_bytes)
    }
    pub fn provenance(&self) -> Result<SourceImportProvenance> {
        Ok(SourceImportProvenance {
            schema_version: self.schema_version,
            specification: self.specification.clone(),
            files: self.files.clone(),
            sidecar: RelativePath::new(joined(
                &self.specification.destination,
                &self.specification.sidecar,
            ))?,
            receipt_sha256: self.digest()?,
        })
    }
    fn parse(bytes: &[u8]) -> Result<Self> {
        let raw: RawAttestation =
            serde_json::from_slice(bytes).map_err(|error| invalid(error.to_string()))?;
        let value = Self {
            schema_version: raw.schema_version,
            specification: raw.specification,
            files: raw.files,
            verified_inputs: BTreeMap::new(),
        };
        value.validate()?;
        if value.canonical_bytes()? != bytes {
            return Err(invalid("source attestation is not canonical"));
        }
        Ok(value)
    }
    fn bind_verified_inputs(&mut self, inspection: &Inspection) -> Result<()> {
        let destination = &self.specification.destination;
        let mut inputs = BTreeMap::from([(
            RelativePath::new(joined(destination, &self.specification.sidecar))?,
            self.digest()?,
        )]);
        for file in &self.files {
            inputs.insert(
                RelativePath::new(joined(destination, &file.path))?,
                file.sha256.clone(),
            );
        }
        for (path, file) in &inspection.authored {
            inputs.insert(
                RelativePath::new(joined(destination, path))?,
                Sha256Digest::from_bytes(&file.bytes),
            );
        }
        self.verified_inputs = inputs;
        Ok(())
    }
    fn validate(&self) -> Result<()> {
        if self.schema_version != 1 || self.files.len() > self.specification.expected_source_files {
            return Err(invalid("invalid source attestation schema or count"));
        }
        let paths = self
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect::<Vec<_>>();
        validate_distinct(&paths)?;
        if self
            .files
            .windows(2)
            .any(|pair| pair[0].path >= pair[1].path)
        {
            return Err(invalid("imported paths must be ordered"));
        }
        for file in &self.files {
            validate_imported_path(&self.specification, &file.path)?;
            if file.git_mode != "100644"
                || file.blob_object_id.len() != self.source().revision().as_str().len()
                || !file
                    .blob_object_id
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err(invalid("invalid imported blob mode or object ID"));
            }
        }
        Ok(())
    }
}

/// Atomically imports the selected source files while preserving authored files.
pub fn import_source(
    location: &CorpusLocation,
    spec: &SourceImportSpec,
    checkout: &Path,
) -> Result<SourceAttestation> {
    import_source_with_expected_inputs(location, spec, checkout, &BTreeMap::new())
}

/// Imports only while the caller's corpus-relative input witnesses remain valid.
/// Inputs must be disjoint from the imported destination and generator state.
pub fn import_source_with_expected_inputs(
    location: &CorpusLocation,
    spec: &SourceImportSpec,
    checkout: &Path,
    expected_inputs: &BTreeMap<RelativePath, Sha256Digest>,
) -> Result<SourceAttestation> {
    import_source_impl(
        location,
        spec,
        checkout,
        expected_inputs,
        #[cfg(test)]
        || {},
        #[cfg(test)]
        || {},
    )
}

fn import_source_impl(
    location: &CorpusLocation,
    spec: &SourceImportSpec,
    checkout: &Path,
    expected_inputs: &BTreeMap<RelativePath, Sha256Digest>,
    #[cfg(test)] after_source_verification: impl FnOnce(),
    #[cfg(test)] inter_scan_hook: impl FnOnce(),
) -> Result<SourceAttestation> {
    let rooted = RootedFs::open_corpus(location)?;
    rooted.revalidate_root()?;
    let inputs = ImportInputs::capture(&rooted, spec, expected_inputs)?;
    // Existing ownership is sufficient to reject an unknown destination before
    // inspecting the named source. Initial adoption still needs its source tree
    // to establish which legacy fixture paths are imported.
    if rooted
        .identity_at(&joined(&spec.destination, &spec.sidecar))?
        .is_some()
    {
        inspect(location, spec, None)?;
    }
    let (source, mut desired) = source_expectation(spec, checkout)?;
    #[cfg(test)]
    after_source_verification();
    let existing = inspect(location, spec, Some(&desired))?;
    let reservation = ArtifactReservation::new(Domain::Browser)?;
    let destination = spec.destination.join(location.corpus_root());
    let stage = reservation.external_stage().join(location.corpus_root());
    let retained_owned = spec
        .authored_files
        .iter()
        .map(|path| {
            (
                format!("retained authored source {}", path.as_str()),
                path.join(&destination),
            )
        })
        .collect::<Vec<_>>();
    let retained = retained_owned
        .iter()
        .map(|(label, path)| (label.as_str(), path.as_path()))
        .collect::<Vec<_>>();
    let input_paths = expected_inputs
        .keys()
        .map(|path| path.join(location.corpus_root()))
        .collect::<Vec<_>>();
    let protected_inputs = input_paths
        .iter()
        .map(|path| ("source import input witness", path.as_path()))
        .collect::<Vec<_>>();
    let protection = if retained.is_empty() {
        ProtectedSourceDisjointness::for_mutation(
            location,
            &[
                ("source import destination", destination.as_path()),
                ("source import stage", stage.as_path()),
            ],
            &protected_inputs,
            &source,
        )?
    } else {
        ProtectedSourceDisjointness::for_partitioned_mutation(
            location,
            ("source import destination", destination.as_path()),
            &[("source import stage", stage.as_path())],
            &protected_inputs,
            &retained,
            &source,
        )?
    };
    let revalidate = |rooted: &RootedFs| {
        inputs.revalidate(rooted)?;
        protection.revalidate(rooted)?;
        source.closing_revalidate()?;
        existing.revalidate(rooted, spec)?;
        if inspect(location, spec, Some(&desired))?.inventory != existing.inventory {
            return Err(invalid(
                "source import inventory changed during publication",
            ));
        }
        Ok(())
    };
    let lease = GenerationLease::acquire_with_revalidation(
        location,
        Domain::Browser,
        "surgeist-browser-source",
        &RunScope::Full,
        "import-source",
        |rooted| {
            let pending = super::profile::classify_pending(rooted)?;
            revalidate(rooted)?;
            if let Some(pending) = pending {
                pending.execute(rooted)?;
            }
            Ok(())
        },
    )?;
    let mut artifacts = vec![(spec.sidecar.clone(), desired.canonical_bytes()?)];
    artifacts.extend(
        source
            .snapshot()
            .entries
            .iter()
            .map(|entry| (entry.path.clone(), entry.bytes.clone())),
    );
    artifacts.extend(
        existing
            .authored
            .iter()
            .map(|(path, file)| (path.clone(), file.bytes.clone())),
    );
    if existing.matches_artifacts(&artifacts) {
        #[cfg(test)]
        inter_scan_hook();
        revalidate(lease.rooted())?;
        desired.bind_verified_inputs(&existing)?;
        return Ok(desired);
    }
    let retained = artifacts
        .iter()
        .map(|(path, _)| path.clone())
        .collect::<BTreeSet<_>>();
    let mut classified = existing.regular_paths();
    classified.extend(retained.iter().cloned());
    let inventory = PublicationInventory::new(
        classified.into_iter().collect(),
        retained.into_iter().collect(),
        vec![],
    )?;
    let plan = ArtifactPlan::new(
        location,
        Domain::Browser,
        &lease,
        spec.destination.clone(),
        PublicationPolicy::CleanFull,
        artifacts,
        inventory,
    )?
    .with_reservation(reservation)?;
    #[cfg(test)]
    plan.install_with_revalidation_and_inter_scan_hook(revalidate, inter_scan_hook)?;
    #[cfg(not(test))]
    plan.install_with_revalidation(revalidate)?;
    desired.bind_verified_inputs(&existing)?;
    Ok(desired)
}

/// Checks imported bytes and their pin offline, without consulting a checkout.
pub fn verify_import(
    location: &CorpusLocation,
    spec: &SourceImportSpec,
) -> Result<SourceAttestation> {
    let check = GenerationCheck::acquire(location, Domain::Browser)?;
    let result = (|| {
        let inspection = inspect(location, spec, None)?;
        let sidecar = inspection
            .sidecar
            .as_ref()
            .ok_or_else(|| invalid("source attestation is absent"))?;
        if &sidecar.specification != spec {
            return Err(invalid(
                "source attestation differs from the declared source pin or selection policy",
            ));
        }
        let inventory = inspection
            .inventory
            .as_ref()
            .ok_or_else(|| invalid("source destination is absent"))?;
        for file in &sidecar.files {
            if inventory.find(&file.path).and_then(|entry| entry.digest()) != Some(&file.sha256) {
                return Err(invalid(format!(
                    "imported source is absent or changed: {}",
                    file.path.as_str()
                )));
            }
        }
        let rooted = RootedFs::open_corpus(location)?;
        inspection.revalidate(&rooted, spec)?;
        if Inventory::scan(
            &rooted,
            spec.destination.as_str(),
            InventoryPolicy::FinalCorpus,
        )? != inspection.inventory
        {
            return Err(invalid("source inventory changed during checking"));
        }
        let mut verified = sidecar.clone();
        verified.bind_verified_inputs(&inspection)?;
        Ok(verified)
    })();
    let finish = check.finish();
    match (result, finish) {
        (_, Err(error)) | (Err(error), Ok(())) => Err(error),
        (Ok(value), Ok(())) => Ok(value),
    }
}

/// Additionally checks the explicitly supplied pinned source checkout.
pub fn verify_source_import(
    location: &CorpusLocation,
    spec: &SourceImportSpec,
    checkout: &Path,
) -> Result<SourceAttestation> {
    let actual = verify_import(location, spec)?;
    let (source, expected) = source_expectation(spec, checkout)?;
    if actual.canonical_bytes()? != expected.canonical_bytes()? {
        return Err(invalid(
            "import attestation differs from the exact source checkout",
        ));
    }
    source.closing_revalidate()?;
    if verify_import(location, spec)? != actual {
        return Err(invalid("import changed during source verification"));
    }
    source.closing_revalidate()?;
    Ok(actual)
}

fn source_expectation(
    spec: &SourceImportSpec,
    checkout: &Path,
) -> Result<(ProtectedSource, SourceAttestation)> {
    let inventory = super::acquisition::verify_source_inventory(checkout, &spec.pin)?;
    let snapshot = inventory.snapshot();
    let mut count = 0_usize;
    let mut imported = Vec::new();
    for entry in &snapshot.entries {
        let path = entry
            .path
            .to_relative_path()
            .ok_or_else(|| invalid("source tree path is not normalized UTF-8"))?;
        if entry.kind != ProtectedTreeEntryKind::Blob
            || !matches!(entry.git_mode.as_str(), "100644" | "100755")
        {
            return Err(invalid(format!(
                "source tree contains an unsupported link or object: {}",
                path.as_str()
            )));
        }
        if Path::new(path.as_str()).extension() != Some(std::ffi::OsStr::new(&spec.extension)) {
            continue;
        }
        count = count
            .checked_add(1)
            .ok_or_else(|| invalid("source count overflow"))?;
        if entry.git_mode != "100644" {
            return Err(invalid("selected source fixture must use Git mode 100644"));
        }
        if spec.excluded(&path) {
            continue;
        }
        validate_imported_path(spec, &path)?;
        let bytes = entry
            .bytes
            .clone()
            .ok_or_else(|| invalid("selected source bytes are absent"))?;
        let digest = Sha256Digest::from_bytes(&bytes);
        if entry.digest.as_ref() != Some(&digest) {
            return Err(invalid("immutable source snapshot digest mismatch"));
        }
        imported.push(SnapshotEntry {
            path,
            git_mode: entry.git_mode.clone(),
            blob_object_id: entry.object_id.clone(),
            bytes,
            digest,
        });
    }
    if count != spec.expected_source_files {
        return Err(invalid(format!(
            "expected {} pre-exclusion source files, found {count}",
            spec.expected_source_files
        )));
    }
    imported.sort_by(|left, right| left.path.cmp(&right.path));
    let verified = VerifiedSourceSnapshot {
        object_format: snapshot.object_format,
        entries: imported,
    };
    let source = inventory.into_protected_source(verified)?;
    let files = source
        .snapshot()
        .entries
        .iter()
        .map(|entry| ImportedSourceFile {
            path: entry.path.clone(),
            git_mode: entry.git_mode.clone(),
            blob_object_id: entry.blob_object_id.clone(),
            sha256: entry.digest.clone(),
        })
        .collect();
    let attestation = SourceAttestation {
        schema_version: 1,
        specification: spec.clone(),
        files,
        verified_inputs: BTreeMap::new(),
    };
    attestation.validate()?;
    Ok((source, attestation))
}

struct CapturedFile {
    bytes: Vec<u8>,
    identity: crate::core::HeldIdentity,
    handle: std::fs::File,
}

impl CapturedFile {
    fn capture(rooted: &RootedFs, path: &str) -> Result<Self> {
        use std::io::Read;
        let mut handle = rooted.open_file_handle(path, CORPUS_FILE_MODE, false)?;
        let identity = rooted.validate_handle_at(path, &handle, CORPUS_FILE_MODE)?;
        let mut bytes = Vec::new();
        handle
            .read_to_end(&mut bytes)
            .map_err(|error| invalid(error.to_string()))?;
        rooted.validate_handle_at(path, &handle, CORPUS_FILE_MODE)?;
        Ok(Self {
            bytes,
            identity,
            handle,
        })
    }

    fn revalidate(&self, rooted: &RootedFs, path: &str, subject: &str) -> Result<()> {
        let identity = rooted.validate_handle_at(path, &self.handle, CORPUS_FILE_MODE)?;
        if !identity.matches_recovery(&self.identity)
            || rooted.read_file(path, CORPUS_FILE_MODE)? != self.bytes
        {
            return Err(invalid(format!("{subject} changed during import: {path}")));
        }
        rooted.validate_handle_at(path, &self.handle, CORPUS_FILE_MODE)?;
        Ok(())
    }
}

struct ImportInputs {
    files: BTreeMap<RelativePath, CapturedFile>,
}

impl ImportInputs {
    fn capture(
        rooted: &RootedFs,
        spec: &SourceImportSpec,
        expected: &BTreeMap<RelativePath, Sha256Digest>,
    ) -> Result<Self> {
        let mut files = BTreeMap::new();
        for (path, digest) in expected {
            validate_path(path)?;
            if aliases(path, &spec.destination) {
                return Err(invalid(
                    "source import input witness overlaps its destination",
                ));
            }
            let file = CapturedFile::capture(rooted, path.as_str())?;
            if Sha256Digest::from_bytes(&file.bytes) != *digest {
                return Err(invalid(format!(
                    "source import input witness is stale: {}",
                    path.as_str()
                )));
            }
            files.insert(path.clone(), file);
        }
        Ok(Self { files })
    }

    fn revalidate(&self, rooted: &RootedFs) -> Result<()> {
        rooted.revalidate_root()?;
        for (path, file) in &self.files {
            file.revalidate(rooted, path.as_str(), "source import input witness")?;
        }
        Ok(())
    }
}

struct Inspection {
    inventory: Option<Inventory>,
    sidecar: Option<SourceAttestation>,
    authored: BTreeMap<RelativePath, CapturedFile>,
}
impl Inspection {
    fn matches_artifacts(&self, artifacts: &[(RelativePath, Vec<u8>)]) -> bool {
        let Some(inventory) = &self.inventory else {
            return false;
        };
        self.regular_paths().len() == artifacts.len()
            && artifacts.iter().all(|(path, bytes)| {
                inventory.find(path).and_then(|entry| entry.digest())
                    == Some(&Sha256Digest::from_bytes(bytes))
            })
    }

    fn regular_paths(&self) -> BTreeSet<RelativePath> {
        self.inventory
            .iter()
            .flat_map(|inventory| inventory.entries())
            .filter(|entry| entry.identity().kind() == NodeKind::Regular)
            .map(|entry| entry.path().clone())
            .collect()
    }
    fn revalidate(&self, rooted: &RootedFs, spec: &SourceImportSpec) -> Result<()> {
        rooted.revalidate_root()?;
        for (path, file) in &self.authored {
            let relative = joined(&spec.destination, path);
            file.revalidate(rooted, &relative, "authored source")?;
        }
        Ok(())
    }
}
fn inspect(
    location: &CorpusLocation,
    spec: &SourceImportSpec,
    desired: Option<&SourceAttestation>,
) -> Result<Inspection> {
    let rooted = RootedFs::open_corpus(location)?;
    let inventory = Inventory::scan(
        &rooted,
        spec.destination.as_str(),
        InventoryPolicy::FinalCorpus,
    )?;
    let sidecar = if let Some(entry) = inventory
        .as_ref()
        .and_then(|inventory| inventory.find(&spec.sidecar))
    {
        if entry.identity().kind() != NodeKind::Regular {
            return Err(invalid("source attestation is not a regular file"));
        }
        let bytes =
            rooted.read_file(&joined(&spec.destination, &spec.sidecar), CORPUS_FILE_MODE)?;
        if entry.digest() != Some(&Sha256Digest::from_bytes(&bytes)) {
            return Err(invalid("source attestation changed during inspection"));
        }
        Some(SourceAttestation::parse(&bytes)?)
    } else {
        None
    };
    let imported = sidecar
        .as_ref()
        .or(desired)
        .map(|attestation| {
            attestation
                .files
                .iter()
                .map(|file| file.path.clone())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    for path in &imported {
        if spec
            .authored_files
            .iter()
            .any(|authored| aliases(path, authored))
            || aliases(path, &spec.sidecar)
        {
            return Err(invalid(
                "prior import ownership overlaps current authored ownership",
            ));
        }
    }
    let mut admitted = spec
        .authored_files
        .iter()
        .cloned()
        .chain(imported)
        .collect::<BTreeSet<_>>();
    if sidecar.is_some() {
        admitted.insert(spec.sidecar.clone());
    }
    if let Some(inventory) = &inventory {
        for entry in inventory.entries() {
            let allowed = match entry.identity().kind() {
                NodeKind::Regular => admitted.contains(entry.path()),
                NodeKind::Directory => admitted.iter().any(|path| {
                    path.as_str()
                        .starts_with(&format!("{}/", entry.path().as_str()))
                }),
                NodeKind::Symlink => false,
            };
            if !allowed {
                return Err(invalid(format!(
                    "unclassified source destination entry: {}",
                    entry.path().as_str()
                )));
            }
        }
    }
    let mut authored = BTreeMap::new();
    for path in &spec.authored_files {
        let entry = inventory
            .as_ref()
            .and_then(|inventory| inventory.find(path))
            .ok_or_else(|| invalid(format!("authored source is absent: {}", path.as_str())))?;
        let relative = joined(&spec.destination, path);
        let file = CapturedFile::capture(&rooted, &relative)?;
        if !file.identity.matches_recovery(entry.identity()) {
            return Err(invalid("authored source identity changed during snapshot"));
        }
        if entry.digest() != Some(&Sha256Digest::from_bytes(&file.bytes)) {
            return Err(invalid("authored source bytes changed during snapshot"));
        }
        authored.insert(path.clone(), file);
    }
    Ok(Inspection {
        inventory,
        sidecar,
        authored,
    })
}
fn joined(parent: &RelativePath, child: &RelativePath) -> String {
    format!("{}/{}", parent.as_str(), child.as_str())
}

fn aliases(left: &RelativePath, right: &RelativePath) -> bool {
    let left = left.as_str().to_ascii_lowercase();
    let right = right.as_str().to_ascii_lowercase();
    left == right
        || left.starts_with(&format!("{right}/"))
        || right.starts_with(&format!("{left}/"))
}
fn validate_distinct(paths: &[RelativePath]) -> Result<()> {
    let mut normalized = paths
        .iter()
        .map(|path| path.as_str().to_ascii_lowercase())
        .collect::<Vec<_>>();
    normalized.sort();
    if normalized
        .windows(2)
        .any(|pair| pair[0] == pair[1] || pair[1].starts_with(&format!("{}/", pair[0])))
    {
        return Err(invalid("source paths collide on the mutation target"));
    }
    Ok(())
}

fn validate_path(path: &RelativePath) -> Result<()> {
    if path.as_str().split('/').any(|p| {
        p.eq_ignore_ascii_case(".surgeist-generator")
            || p.to_ascii_lowercase().starts_with("._surgeist-")
    }) {
        return Err(invalid(
            "source import path uses a generator-reserved component",
        ));
    }
    Ok(())
}
fn validate_imported_path(spec: &SourceImportSpec, path: &RelativePath) -> Result<()> {
    validate_path(path)?;
    RelativePath::with_extension(path.as_str(), &spec.extension)?;
    if spec.excluded(path)
        || aliases(path, &spec.sidecar)
        || spec
            .authored_files
            .iter()
            .any(|authored| aliases(path, authored))
    {
        return Err(invalid(format!(
            "imported path overlaps the excluded/authored partition: {}",
            path.as_str()
        )));
    }
    Ok(())
}
fn invalid(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidInventory,
        "validate source import",
        detail,
    )
}

#[cfg(all(test, target_os = "macos", target_arch = "aarch64"))]
#[path = "source_import_tests.rs"]
mod tests;
