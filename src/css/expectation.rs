use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    CaseDisposition, GenerationCounts, GeneratorError, GeneratorErrorKind, RelativePath, Result,
    Sha256Digest, SourceRevision,
};

use super::fixture::ValidatedImport;
use super::manifest::CssManifest;

const GENERATOR: &str = "surgeist-css-generate";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct DerivedExpectations {
    pub(super) artifacts: Vec<ExpectationArtifact>,
    pub(super) counts: GenerationCounts,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct ExpectationArtifact {
    pub(super) path: RelativePath,
    pub(super) source_digest: Sha256Digest,
    pub(super) bytes: Vec<u8>,
    pub(super) case_count: usize,
}

#[derive(Serialize)]
struct ExpectationFile<'a> {
    schema_version: u8,
    generator: &'static str,
    source: RelativePath,
    source_sha256: &'a Sha256Digest,
    source_revision: &'a SourceRevision,
    import_provenance_sha256: &'a Sha256Digest,
    cases: Vec<ExpectationCase>,
}

#[derive(Serialize)]
struct ExpectationCase {
    id: String,
    context: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<CanonicalObject>,
    upstream_outcome: UpstreamOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    canonical_css: Option<String>,
    status: CaseDisposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum UpstreamOutcome {
    Parsed,
    Rejected,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PersistedExpectationFile {
    schema_version: u8,
    generator: String,
    source: RelativePath,
    source_sha256: Sha256Digest,
    source_revision: SourceRevision,
    import_provenance_sha256: Sha256Digest,
    cases: Vec<PersistedExpectationCase>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PersistedExpectationCase {
    id: String,
    context: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    input: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<CanonicalObject>,
    upstream_outcome: PersistedUpstreamOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    canonical_css: Option<String>,
    status: CaseDisposition,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
}

#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum PersistedUpstreamOutcome {
    Parsed,
    Rejected,
}

#[derive(Serialize)]
#[serde(transparent)]
struct CanonicalObject(BTreeMap<String, CanonicalValue>);

enum CanonicalValue {
    Null,
    Bool(bool),
    Number(serde_json::Number),
    String(String),
    Array(Vec<Self>),
    Object(BTreeMap<String, Self>),
}

impl Serialize for CanonicalValue {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Null => serializer.serialize_unit(),
            Self::Bool(value) => serializer.serialize_bool(*value),
            Self::Number(value) => value.serialize(serializer),
            Self::String(value) => serializer.serialize_str(value),
            Self::Array(value) => value.serialize(serializer),
            Self::Object(value) => value.serialize(serializer),
        }
    }
}

struct RawFixture {
    groups: Vec<FixtureGroup>,
}

struct FixtureGroup {
    label: String,
    members: FixtureMembers,
}

enum FixtureMembers {
    Singleton(CaseMember),
    Array(Vec<CaseMember>),
    LegacyErrorArray(Vec<RawErrorCase>),
}

struct CaseMember {
    source: String,
    error_truthy: bool,
    options: Option<CanonicalObject>,
    generate: Option<String>,
}

#[derive(Deserialize)]
struct RawErrorCase {
    source: String,
}

pub(super) fn derive(
    imported: &ValidatedImport,
    manifest: &CssManifest,
) -> Result<DerivedExpectations> {
    let mut overrides = manifest
        .cases
        .iter()
        .map(|record| (record.id().to_owned(), record))
        .collect::<BTreeMap<_, _>>();
    let mut all_ids = BTreeSet::new();
    let mut artifacts = Vec::with_capacity(imported.fixtures().len());
    let mut disposition_totals = [0_usize; 4];
    let mut total_cases = 0_usize;

    for fixture in imported.fixtures() {
        let normalized = normalize_numeric_error_values(&fixture.bytes, &fixture.path)?;
        reject_duplicate_members_and_trailing(&normalized, &fixture.path)?;
        let raw: RawFixture = serde_json::from_slice(&normalized).map_err(|error| {
            invalid_inventory_with_source(
                "parse typed CSS fixture",
                format!("invalid fixture shape: {}", fixture.path.as_str()),
                error,
            )
        })?;
        let context = fixture
            .path
            .as_str()
            .split('/')
            .next()
            .ok_or_else(|| invalid_inventory("CSS fixture path has no context component"))?
            .to_owned();
        let mut cases = derive_fixture_cases(raw, &fixture.path, &context);
        if cases.is_empty() {
            return Err(invalid_inventory(format!(
                "CSS fixture derives no cases: {}",
                fixture.path.as_str()
            )));
        }
        cases.sort_by(|left, right| left.id.cmp(&right.id));
        for case in &mut cases {
            if !all_ids.insert(case.id.clone()) {
                return Err(invalid_inventory(format!(
                    "duplicate derived CSS case ID: {}",
                    case.id.as_str()
                )));
            }
            if let Some(record) = overrides.remove(case.id.as_str()) {
                let record = record
                    .bind(&fixture.path)
                    .map_err(|error| invalid_inventory(error.to_string()))?;
                case.id = record.case_id().as_str().to_owned();
                case.status = record.disposition();
                case.reason = record.reason().map(str::to_owned);
            }
            disposition_totals[disposition_index(case.status)] = disposition_totals
                [disposition_index(case.status)]
            .checked_add(1)
            .ok_or_else(|| invalid_inventory("CSS disposition count overflow"))?;
        }
        total_cases = total_cases
            .checked_add(cases.len())
            .ok_or_else(|| invalid_inventory("CSS case count overflow"))?;
        let source = prefixed(&manifest.import_root, &fixture.path)?;
        let expectation = ExpectationFile {
            schema_version: 1,
            generator: GENERATOR,
            source,
            source_sha256: &fixture.digest,
            source_revision: &manifest.revision,
            import_provenance_sha256: imported.sidecar_digest(),
            cases,
        };
        let mut bytes = serde_json::to_vec_pretty(&expectation).map_err(|error| {
            invalid_inventory_with_source("serialize CSS expectation", fixture.path.as_str(), error)
        })?;
        bytes.push(b'\n');
        artifacts.push(ExpectationArtifact {
            path: fixture.path.clone(),
            source_digest: fixture.digest.clone(),
            case_count: expectation.cases.len(),
            bytes,
        });
    }

    if let Some((id, _)) = overrides.into_iter().next() {
        return Err(invalid_inventory(format!(
            "CSS manifest override matches no derived case: {id}"
        )));
    }
    if total_cases != manifest.expected_cases {
        return Err(invalid_inventory(format!(
            "manifest expected {} CSS cases, fixtures derive {total_cases}",
            manifest.expected_cases
        )));
    }
    let counts = GenerationCounts::new(
        disposition_totals[0],
        disposition_totals[1],
        disposition_totals[2],
        disposition_totals[3],
        0,
    )?;
    Ok(DerivedExpectations { artifacts, counts })
}

fn derive_fixture_cases(
    raw: RawFixture,
    fixture_path: &RelativePath,
    context: &str,
) -> Vec<ExpectationCase> {
    let mut cases = Vec::new();
    for group in raw.groups {
        match group.members {
            FixtureMembers::Singleton(member) => {
                cases.push(non_legacy_case(
                    fixture_path,
                    context,
                    &group.label,
                    None,
                    member,
                ));
            }
            FixtureMembers::Array(members) => {
                for (index, member) in members.into_iter().enumerate() {
                    cases.push(non_legacy_case(
                        fixture_path,
                        context,
                        &group.label,
                        Some(index),
                        member,
                    ));
                }
            }
            FixtureMembers::LegacyErrorArray(errors) => {
                for (index, error) in errors.into_iter().enumerate() {
                    cases.push(ExpectationCase {
                        id: format!("{}#/error/{index}", fixture_path.as_str()),
                        context: context.to_owned(),
                        label: None,
                        input: error.source,
                        options: None,
                        upstream_outcome: UpstreamOutcome::Rejected,
                        canonical_css: None,
                        status: CaseDisposition::Active,
                        reason: None,
                    });
                }
            }
        }
    }
    cases
}

fn non_legacy_case(
    fixture_path: &RelativePath,
    context: &str,
    label: &str,
    index: Option<usize>,
    member: CaseMember,
) -> ExpectationCase {
    let mut id = format!(
        "{}#/{}",
        fixture_path.as_str(),
        escape_json_pointer_token(label)
    );
    if let Some(index) = index {
        id.push('/');
        id.push_str(&index.to_string());
    }
    ExpectationCase {
        id,
        context: context.to_owned(),
        label: Some(label.to_owned()),
        input: member.source,
        options: member.options,
        upstream_outcome: if member.error_truthy {
            UpstreamOutcome::Rejected
        } else {
            UpstreamOutcome::Parsed
        },
        canonical_css: member.generate,
        status: CaseDisposition::Active,
        reason: None,
    }
}

pub(super) fn validate_persisted(
    bytes: &[u8],
    fixture_path: &RelativePath,
    manifest: &CssManifest,
) -> Result<()> {
    reject_duplicate_members_and_trailing(bytes, fixture_path)?;
    let persisted: PersistedExpectationFile = serde_json::from_slice(bytes).map_err(|error| {
        invalid_inventory_with_source(
            "parse persisted CSS expectation",
            format!("invalid expectation schema: {}", fixture_path.as_str()),
            error,
        )
    })?;
    let mut canonical = serde_json::to_vec_pretty(&persisted).map_err(|error| {
        invalid_inventory_with_source(
            "serialize persisted CSS expectation",
            fixture_path.as_str(),
            error,
        )
    })?;
    canonical.push(b'\n');
    if canonical != bytes {
        return Err(invalid_inventory(format!(
            "CSS expectation bytes are not canonical: {}",
            fixture_path.as_str()
        )));
    }
    if persisted.schema_version != 1 || persisted.generator != GENERATOR {
        return Err(invalid_inventory(format!(
            "CSS expectation header is noncanonical: {}",
            fixture_path.as_str()
        )));
    }
    if persisted.source != prefixed(&manifest.import_root, fixture_path)? {
        return Err(invalid_inventory(format!(
            "CSS expectation source mapping is invalid: {}",
            fixture_path.as_str()
        )));
    }
    if persisted.cases.is_empty() {
        return Err(invalid_inventory(format!(
            "CSS expectation has no cases: {}",
            fixture_path.as_str()
        )));
    }

    let context = fixture_path
        .as_str()
        .split('/')
        .next()
        .ok_or_else(|| invalid_inventory("CSS expectation path has no context component"))?;
    let mut prior_id = None::<&str>;
    let mut legacy_error_indices = BTreeSet::new();
    let mut ordinary_groups = BTreeMap::new();
    for case in &persisted.cases {
        if prior_id.is_some_and(|prior| prior >= case.id.as_str()) {
            return Err(invalid_inventory(format!(
                "CSS expectation case IDs are not strictly increasing: {}",
                fixture_path.as_str()
            )));
        }
        prior_id = Some(&case.id);
        if case.context != context
            || !crate::core::validate_disposition_reason(case.status, case.reason.as_deref())
        {
            return Err(invalid_inventory(format!(
                "CSS expectation case metadata is invalid: {}",
                case.id
            )));
        }
        validate_persisted_case_shape(
            case,
            fixture_path,
            &mut ordinary_groups,
            &mut legacy_error_indices,
        )?;
    }
    if legacy_error_indices
        .iter()
        .copied()
        .ne(0..legacy_error_indices.len())
    {
        return Err(invalid_inventory(format!(
            "rejected CSS expectation indices are not contiguous: {}",
            fixture_path.as_str()
        )));
    }
    for (label, group) in ordinary_groups {
        if let PersistedGroupShape::Array(indices) = group
            && indices.iter().copied().ne(0..indices.len())
        {
            return Err(invalid_inventory(format!(
                "CSS expectation group indices are not contiguous for label: {label}"
            )));
        }
    }
    Ok(())
}

enum PersistedGroupShape {
    Singleton,
    Array(BTreeSet<usize>),
}

fn validate_persisted_case_shape(
    case: &PersistedExpectationCase,
    fixture_path: &RelativePath,
    ordinary_groups: &mut BTreeMap<String, PersistedGroupShape>,
    legacy_error_indices: &mut BTreeSet<usize>,
) -> Result<()> {
    if let Some(label) = case.label.as_deref() {
        return record_ordinary_group_case(case, fixture_path, label, ordinary_groups);
    }
    if matches!(case.upstream_outcome, PersistedUpstreamOutcome::Parsed) {
        return Err(invalid_inventory(format!(
            "parsed CSS expectation has no label: {}",
            case.id
        )));
    }
    if case.options.is_some() || case.canonical_css.is_some() {
        return Err(invalid_inventory(format!(
            "legacy rejected CSS expectation contains ordinary fields: {}",
            case.id
        )));
    }
    let prefix = format!("{}#/error/", fixture_path.as_str());
    let index = case
        .id
        .strip_prefix(&prefix)
        .and_then(parse_canonical_index)
        .ok_or_else(|| {
            invalid_inventory(format!(
                "legacy rejected CSS expectation ID is invalid: {}",
                case.id
            ))
        })?;
    if !legacy_error_indices.insert(index) {
        return Err(invalid_inventory(format!(
            "duplicate rejected CSS expectation index: {index}"
        )));
    }
    Ok(())
}

fn record_ordinary_group_case(
    case: &PersistedExpectationCase,
    fixture_path: &RelativePath,
    label: &str,
    groups: &mut BTreeMap<String, PersistedGroupShape>,
) -> Result<()> {
    use std::collections::btree_map::Entry;

    let base = format!(
        "{}#/{}",
        fixture_path.as_str(),
        escape_json_pointer_token(label)
    );
    let index = if case.id == base {
        None
    } else {
        case.id
            .strip_prefix(&format!("{base}/"))
            .and_then(parse_canonical_index)
            .map(Some)
            .ok_or_else(|| {
                invalid_inventory(format!(
                    "CSS expectation ID does not match its label: {}",
                    case.id
                ))
            })?
    };
    match (groups.entry(label.to_owned()), index) {
        (Entry::Vacant(entry), None) => {
            entry.insert(PersistedGroupShape::Singleton);
        }
        (Entry::Vacant(entry), Some(index)) => {
            entry.insert(PersistedGroupShape::Array(BTreeSet::from([index])));
        }
        (Entry::Occupied(_), None) => {
            return Err(invalid_inventory(format!(
                "CSS expectation mixes singleton and grouped IDs for label: {label}"
            )));
        }
        (Entry::Occupied(mut entry), Some(index)) => {
            let PersistedGroupShape::Array(indices) = entry.get_mut() else {
                return Err(invalid_inventory(format!(
                    "CSS expectation mixes singleton and grouped IDs for label: {label}"
                )));
            };
            if !indices.insert(index) {
                return Err(invalid_inventory(format!(
                    "duplicate CSS expectation group index: {label}/{index}"
                )));
            }
        }
    }
    Ok(())
}

fn parse_canonical_index(value: &str) -> Option<usize> {
    if value.is_empty() || (value.len() > 1 && value.starts_with('0')) {
        return None;
    }
    value.parse().ok()
}

fn disposition_index(disposition: CaseDisposition) -> usize {
    match disposition {
        CaseDisposition::Active => 0,
        CaseDisposition::ExpectedFail => 1,
        CaseDisposition::Unsupported => 2,
        CaseDisposition::Quarantined => 3,
    }
}

fn prefixed(root: &RelativePath, path: &RelativePath) -> Result<RelativePath> {
    RelativePath::new(format!("{}/{}", root.as_str(), path.as_str()))
        .map_err(|error| invalid_inventory(error.to_string()))
}

fn escape_json_pointer_token(value: &str) -> String {
    value.replace('~', "~0").replace('/', "~1")
}

fn normalize_numeric_error_values(bytes: &[u8], path: &RelativePath) -> Result<Vec<u8>> {
    NumericErrorNormalizer::new(bytes)
        .normalize()
        .map_err(|detail| {
            invalid_inventory(format!(
                "normalize numeric CSS error values in {}: {detail}",
                path.as_str()
            ))
        })
}

struct NumericErrorNormalizer<'a> {
    input: &'a [u8],
    cursor: usize,
    output: Vec<u8>,
}

impl<'a> NumericErrorNormalizer<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            cursor: 0,
            output: Vec::with_capacity(input.len()),
        }
    }

    fn normalize(mut self) -> std::result::Result<Vec<u8>, String> {
        self.copy_whitespace();
        if self.peek() != Some(b'{') {
            return Ok(self.input.to_vec());
        }
        self.normalize_fixture_object()?;
        self.output.extend_from_slice(&self.input[self.cursor..]);
        Ok(self.output)
    }

    fn normalize_fixture_object(&mut self) -> std::result::Result<(), String> {
        self.copy_expected(b'{')?;
        self.copy_whitespace();
        if self.copy_if(b'}') {
            return Ok(());
        }
        loop {
            let label = self.copy_object_key()?;
            self.copy_member_separator()?;
            match self.peek() {
                Some(b'{') => self.normalize_case_member_object()?,
                Some(b'[') if label != "error" => self.normalize_group_array()?,
                _ => self.normalize_value(false)?,
            }
            if self.copy_collection_end(b'}')? {
                return Ok(());
            }
        }
    }

    fn normalize_group_array(&mut self) -> std::result::Result<(), String> {
        self.copy_expected(b'[')?;
        self.copy_whitespace();
        if self.copy_if(b']') {
            return Ok(());
        }
        loop {
            if self.peek() == Some(b'{') {
                self.normalize_case_member_object()?;
            } else {
                self.normalize_value(false)?;
            }
            if self.copy_collection_end(b']')? {
                return Ok(());
            }
        }
    }

    fn normalize_case_member_object(&mut self) -> std::result::Result<(), String> {
        self.copy_expected(b'{')?;
        self.copy_whitespace();
        if self.copy_if(b'}') {
            return Ok(());
        }
        loop {
            let field = self.copy_object_key()?;
            self.copy_member_separator()?;
            self.normalize_value(field == "error")?;
            if self.copy_collection_end(b'}')? {
                return Ok(());
            }
        }
    }

    fn normalize_value(&mut self, normalize_numbers: bool) -> std::result::Result<(), String> {
        self.copy_whitespace();
        match self.peek() {
            Some(b'{') => self.normalize_object(normalize_numbers),
            Some(b'[') => self.normalize_array(normalize_numbers),
            Some(b'"') => self.copy_string().map(|_| ()),
            Some(b't') => self.copy_literal(b"true"),
            Some(b'f') => self.copy_literal(b"false"),
            Some(b'n') => self.copy_literal(b"null"),
            Some(b'-' | b'0'..=b'9') => self.copy_number(normalize_numbers),
            Some(_) => Err(self.error("expected a JSON value")),
            None => Err(self.error("unexpected end of JSON value")),
        }
    }

    fn normalize_object(&mut self, normalize_numbers: bool) -> std::result::Result<(), String> {
        self.copy_expected(b'{')?;
        self.copy_whitespace();
        if self.copy_if(b'}') {
            return Ok(());
        }
        loop {
            self.copy_object_key()?;
            self.copy_member_separator()?;
            self.normalize_value(normalize_numbers)?;
            if self.copy_collection_end(b'}')? {
                return Ok(());
            }
        }
    }

    fn normalize_array(&mut self, normalize_numbers: bool) -> std::result::Result<(), String> {
        self.copy_expected(b'[')?;
        self.copy_whitespace();
        if self.copy_if(b']') {
            return Ok(());
        }
        loop {
            self.normalize_value(normalize_numbers)?;
            if self.copy_collection_end(b']')? {
                return Ok(());
            }
        }
    }

    fn copy_object_key(&mut self) -> std::result::Result<String, String> {
        self.copy_whitespace();
        let start = self.cursor;
        let end = self.copy_string()?;
        serde_json::from_slice(&self.input[start..end])
            .map_err(|error| self.error(&format!("invalid JSON object member: {error}")))
    }

    fn copy_string(&mut self) -> std::result::Result<usize, String> {
        self.copy_expected(b'"')?;
        while let Some(byte) = self.peek() {
            self.copy_byte();
            match byte {
                b'"' => return Ok(self.cursor),
                b'\\' => {
                    if self.peek().is_none() {
                        return Err(self.error("unterminated JSON string escape"));
                    }
                    self.copy_byte();
                }
                _ => {}
            }
        }
        Err(self.error("unterminated JSON string"))
    }

    fn copy_number(&mut self, normalize: bool) -> std::result::Result<(), String> {
        let start = self.cursor;
        if self.peek() == Some(b'-') {
            self.cursor += 1;
        }
        match self.peek() {
            Some(b'0') => self.cursor += 1,
            Some(b'1'..=b'9') => self.advance_digits(),
            _ => return Err(self.error("invalid JSON number integer")),
        }
        if self.peek() == Some(b'.') {
            self.cursor += 1;
            self.require_digits("invalid JSON number fraction")?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.cursor += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.cursor += 1;
            }
            self.require_digits("invalid JSON number exponent")?;
        }
        let token = &self.input[start..self.cursor];
        let overflow = normalize
            && std::str::from_utf8(token)
                .ok()
                .and_then(|value| value.parse::<f64>().ok())
                .is_some_and(f64::is_infinite);
        if overflow {
            self.output.push(b'1');
            self.output
                .resize(self.output.len() + token.len() - 1, b' ');
        } else {
            self.output.extend_from_slice(token);
        }
        Ok(())
    }

    fn require_digits(&mut self, detail: &str) -> std::result::Result<(), String> {
        if !matches!(self.peek(), Some(b'0'..=b'9')) {
            return Err(self.error(detail));
        }
        self.advance_digits();
        Ok(())
    }

    fn advance_digits(&mut self) {
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.cursor += 1;
        }
    }

    fn copy_member_separator(&mut self) -> std::result::Result<(), String> {
        self.copy_whitespace();
        self.copy_expected(b':')?;
        self.copy_whitespace();
        Ok(())
    }

    fn copy_collection_end(&mut self, end: u8) -> std::result::Result<bool, String> {
        self.copy_whitespace();
        if self.copy_if(end) {
            return Ok(true);
        }
        self.copy_expected(b',')?;
        self.copy_whitespace();
        Ok(false)
    }

    fn copy_literal(&mut self, literal: &[u8]) -> std::result::Result<(), String> {
        if !self.input[self.cursor..].starts_with(literal) {
            return Err(self.error("invalid JSON literal"));
        }
        self.output.extend_from_slice(literal);
        self.cursor += literal.len();
        Ok(())
    }

    fn copy_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.copy_byte();
        }
    }

    fn copy_expected(&mut self, expected: u8) -> std::result::Result<(), String> {
        if !self.copy_if(expected) {
            return Err(self.error(&format!("expected JSON byte {:?}", char::from(expected))));
        }
        Ok(())
    }

    fn copy_if(&mut self, expected: u8) -> bool {
        if self.peek() == Some(expected) {
            self.copy_byte();
            true
        } else {
            false
        }
    }

    fn copy_byte(&mut self) {
        self.output.push(self.input[self.cursor]);
        self.cursor += 1;
    }

    fn peek(&self) -> Option<u8> {
        self.input.get(self.cursor).copied()
    }

    fn error(&self, detail: &str) -> String {
        format!("{detail} at byte {}", self.cursor)
    }
}

fn reject_duplicate_members_and_trailing(bytes: &[u8], path: &RelativePath) -> Result<()> {
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    DuplicateFreeValue
        .deserialize(&mut deserializer)
        .and_then(|()| deserializer.end())
        .map_err(|error| {
            invalid_inventory_with_source(
                "prepass CSS fixture",
                format!("invalid JSON member stream: {}", path.as_str()),
                error,
            )
        })
}

struct DuplicateFreeValue;

impl<'de> DeserializeSeed<'de> for DuplicateFreeValue {
    type Value = ();

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(DuplicateFreeVisitor)
    }
}

struct DuplicateFreeVisitor;

impl<'de> Visitor<'de> for DuplicateFreeVisitor {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("one JSON value with unique decoded object members")
    }

    fn visit_bool<E>(self, _value: bool) -> std::result::Result<Self::Value, E> {
        Ok(())
    }

    fn visit_i64<E>(self, _value: i64) -> std::result::Result<Self::Value, E> {
        Ok(())
    }

    fn visit_u64<E>(self, _value: u64) -> std::result::Result<Self::Value, E> {
        Ok(())
    }

    fn visit_f64<E>(self, _value: f64) -> std::result::Result<Self::Value, E> {
        Ok(())
    }

    fn visit_str<E>(self, _value: &str) -> std::result::Result<Self::Value, E> {
        Ok(())
    }

    fn visit_string<E>(self, _value: String) -> std::result::Result<Self::Value, E> {
        Ok(())
    }

    fn visit_none<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(())
    }

    fn visit_unit<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(())
    }

    fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        while sequence.next_element_seed(DuplicateFreeValue)?.is_some() {}
        Ok(())
    }

    fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut keys = BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) {
                return Err(serde::de::Error::custom(format!(
                    "duplicate decoded object member: {key}"
                )));
            }
            map.next_value_seed(DuplicateFreeValue)?;
        }
        Ok(())
    }
}

impl<'de> Deserialize<'de> for CanonicalValue {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(CanonicalValueVisitor)
    }
}

impl CanonicalValue {
    fn is_javascript_truthy(&self) -> bool {
        match self {
            Self::Null => false,
            Self::Bool(value) => *value,
            Self::Number(value) => value.as_f64().is_some_and(|number| number != 0.0),
            Self::String(value) => !value.is_empty(),
            Self::Array(_) | Self::Object(_) => true,
        }
    }
}

struct CanonicalValueVisitor;

impl<'de> Visitor<'de> for CanonicalValueVisitor {
    type Value = CanonicalValue;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> std::result::Result<Self::Value, E> {
        Ok(CanonicalValue::Bool(value))
    }

    fn visit_i64<E>(self, value: i64) -> std::result::Result<Self::Value, E> {
        Ok(CanonicalValue::Number(value.into()))
    }

    fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E> {
        Ok(CanonicalValue::Number(value.into()))
    }

    fn visit_f64<E>(self, value: f64) -> std::result::Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        serde_json::Number::from_f64(value)
            .map(CanonicalValue::Number)
            .ok_or_else(|| E::custom("non-finite JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E> {
        Ok(CanonicalValue::String(value.to_owned()))
    }

    fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E> {
        Ok(CanonicalValue::String(value))
    }

    fn visit_none<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(CanonicalValue::Null)
    }

    fn visit_unit<E>(self) -> std::result::Result<Self::Value, E> {
        Ok(CanonicalValue::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let mut values = Vec::new();
        while let Some(value) = sequence.next_element()? {
            values.push(value);
        }
        Ok(CanonicalValue::Array(values))
    }

    fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut values = BTreeMap::new();
        while let Some((key, value)) = map.next_entry()? {
            if values.insert(key, value).is_some() {
                return Err(serde::de::Error::custom(
                    "duplicate canonical JSON object member",
                ));
            }
        }
        Ok(CanonicalValue::Object(values))
    }
}

impl<'de> Deserialize<'de> for CanonicalObject {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CanonicalObjectVisitor;

        impl<'de> Visitor<'de> for CanonicalObjectVisitor {
            type Value = CanonicalObject;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a JSON object")
            }

            fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = BTreeMap::new();
                while let Some((key, value)) = map.next_entry()? {
                    if values.insert(key, value).is_some() {
                        return Err(serde::de::Error::custom(
                            "duplicate canonical options member",
                        ));
                    }
                }
                Ok(CanonicalObject(values))
            }
        }

        deserializer.deserialize_map(CanonicalObjectVisitor)
    }
}

impl<'de> Deserialize<'de> for RawFixture {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RawFixtureVisitor;

        impl<'de> Visitor<'de> for RawFixtureVisitor {
            type Value = RawFixture;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a CSSTree fixture object")
            }

            fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut groups = Vec::new();
                while let Some(label) = map.next_key::<String>()? {
                    groups.push(map.next_value_seed(FixtureGroupSeed { label })?);
                }
                Ok(RawFixture { groups })
            }
        }

        deserializer.deserialize_map(RawFixtureVisitor)
    }
}

struct FixtureGroupSeed {
    label: String,
}

impl<'de> DeserializeSeed<'de> for FixtureGroupSeed {
    type Value = FixtureGroup;

    fn deserialize<D>(self, deserializer: D) -> std::result::Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(FixtureGroupVisitor { label: self.label })
    }
}

struct FixtureGroupVisitor {
    label: String,
}

impl<'de> Visitor<'de> for FixtureGroupVisitor {
    type Value = FixtureGroup;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a CSSTree case object or array")
    }

    fn visit_map<A>(self, map: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        Ok(FixtureGroup {
            label: self.label,
            members: FixtureMembers::Singleton(CaseMember::from_map(map)?),
        })
    }

    fn visit_seq<A>(self, mut sequence: A) -> std::result::Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let members = if self.label == "error" {
            let mut members = Vec::new();
            while let Some(member) = sequence.next_element::<RawErrorCase>()? {
                members.push(member);
            }
            FixtureMembers::LegacyErrorArray(members)
        } else {
            let mut members = Vec::new();
            while let Some(member) = sequence.next_element::<CaseMember>()? {
                members.push(member);
            }
            FixtureMembers::Array(members)
        };
        Ok(FixtureGroup {
            label: self.label,
            members,
        })
    }
}

impl<'de> Deserialize<'de> for CaseMember {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct CaseMemberVisitor;

        impl<'de> Visitor<'de> for CaseMemberVisitor {
            type Value = CaseMember;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a CSSTree case object")
            }

            fn visit_map<A>(self, map: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                CaseMember::from_map(map)
            }
        }

        deserializer.deserialize_map(CaseMemberVisitor)
    }
}

impl CaseMember {
    fn from_map<'de, A>(mut map: A) -> std::result::Result<Self, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut source = None;
        let mut ast_seen = false;
        let mut error_truthy = None;
        let mut options = None;
        let mut options_seen = false;
        let mut generate = None;
        let mut generate_seen = false;
        while let Some(field) = map.next_key::<String>()? {
            match field.as_str() {
                "source" => {
                    if source.is_some() {
                        return Err(serde::de::Error::duplicate_field("source"));
                    }
                    source = Some(map.next_value::<String>()?);
                }
                "ast" => {
                    if ast_seen {
                        return Err(serde::de::Error::duplicate_field("ast"));
                    }
                    ast_seen = true;
                    map.next_value::<IgnoredAny>()?;
                }
                "error" => {
                    if error_truthy.is_some() {
                        return Err(serde::de::Error::duplicate_field("error"));
                    }
                    error_truthy = Some(map.next_value::<CanonicalValue>()?.is_javascript_truthy());
                }
                "options" => {
                    if options_seen {
                        return Err(serde::de::Error::duplicate_field("options"));
                    }
                    options_seen = true;
                    options = Some(map.next_value::<CanonicalObject>()?);
                }
                "generate" => {
                    if generate_seen {
                        return Err(serde::de::Error::duplicate_field("generate"));
                    }
                    generate_seen = true;
                    generate = Some(map.next_value::<String>()?);
                }
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        let error_truthy = error_truthy.unwrap_or(false);
        if !error_truthy && !ast_seen {
            return Err(serde::de::Error::missing_field("ast"));
        }
        Ok(Self {
            source: source.ok_or_else(|| serde::de::Error::missing_field("source"))?,
            error_truthy,
            options,
            generate,
        })
    }
}

fn invalid_inventory(detail: impl Into<String>) -> GeneratorError {
    GeneratorError::new(
        GeneratorErrorKind::InvalidInventory,
        "derive CSS expectations",
        detail,
    )
}

fn invalid_inventory_with_source(
    operation: &str,
    detail: impl Into<String>,
    source: impl std::error::Error + Send + Sync + 'static,
) -> GeneratorError {
    GeneratorError::with_source(
        GeneratorErrorKind::InvalidInventory,
        operation,
        detail,
        source,
    )
}
