use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::de::{DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize};

use crate::{
    CaseDisposition, CaseDispositionRecord, GenerationCounts, GeneratorError, GeneratorErrorKind,
    RelativePath, Result, Sha256Digest, SourceRevision,
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
    ordinary: Vec<(String, RawOrdinaryCase)>,
    errors: Vec<RawErrorCase>,
}

struct RawOrdinaryCase {
    source: String,
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
        .map(|record| (record.case_id().to_owned(), record))
        .collect::<BTreeMap<_, _>>();
    let mut all_ids = BTreeSet::new();
    let mut artifacts = Vec::with_capacity(imported.fixtures().len());
    let mut disposition_totals = [0_usize; 4];
    let mut total_cases = 0_usize;

    for fixture in imported.fixtures() {
        reject_duplicate_members_and_trailing(&fixture.bytes, &fixture.path)?;
        let raw: RawFixture = serde_json::from_slice(&fixture.bytes).map_err(|error| {
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
        let mut cases = Vec::with_capacity(raw.ordinary.len() + raw.errors.len());
        for (label, ordinary) in raw.ordinary {
            let id = format!(
                "{}#/{}",
                fixture.path.as_str(),
                escape_json_pointer_token(&label)
            );
            validate_derived_id(&id, &fixture.path)?;
            cases.push(ExpectationCase {
                id,
                context: context.clone(),
                label: Some(label),
                input: ordinary.source,
                options: ordinary.options,
                upstream_outcome: UpstreamOutcome::Parsed,
                canonical_css: ordinary.generate,
                status: CaseDisposition::Active,
                reason: None,
            });
        }
        for (index, error) in raw.errors.into_iter().enumerate() {
            let id = format!("{}#/error/{index}", fixture.path.as_str());
            validate_derived_id(&id, &fixture.path)?;
            cases.push(ExpectationCase {
                id,
                context: context.clone(),
                label: None,
                input: error.source,
                options: None,
                upstream_outcome: UpstreamOutcome::Rejected,
                canonical_css: None,
                status: CaseDisposition::Active,
                reason: None,
            });
        }
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
                    case.id
                )));
            }
            if let Some(record) = overrides.remove(&case.id) {
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

fn validate_derived_id(id: &str, source: &RelativePath) -> Result<()> {
    CaseDispositionRecord::new(id, source.clone(), CaseDisposition::Active, None::<String>)
        .map(|_| ())
        .map_err(|error| invalid_inventory(error.to_string()))
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
                let mut ordinary = Vec::new();
                let mut errors = None;
                while let Some(label) = map.next_key::<String>()? {
                    if label == "error" {
                        if errors.is_some() {
                            return Err(serde::de::Error::duplicate_field("error"));
                        }
                        errors = Some(map.next_value::<Vec<RawErrorCase>>()?);
                    } else {
                        ordinary.push((label, map.next_value::<RawOrdinaryCase>()?));
                    }
                }
                Ok(RawFixture {
                    ordinary,
                    errors: errors.unwrap_or_default(),
                })
            }
        }

        deserializer.deserialize_map(RawFixtureVisitor)
    }
}

impl<'de> Deserialize<'de> for RawOrdinaryCase {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct RawOrdinaryVisitor;

        impl<'de> Visitor<'de> for RawOrdinaryVisitor {
            type Value = RawOrdinaryCase;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a CSSTree ordinary case object")
            }

            fn visit_map<A>(self, mut map: A) -> std::result::Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut source = None;
                let mut ast_seen = false;
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
                if !ast_seen {
                    return Err(serde::de::Error::missing_field("ast"));
                }
                Ok(RawOrdinaryCase {
                    source: source.ok_or_else(|| serde::de::Error::missing_field("source"))?,
                    options,
                    generate,
                })
            }
        }

        deserializer.deserialize_map(RawOrdinaryVisitor)
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
