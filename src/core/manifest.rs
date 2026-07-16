use std::path::Path;

use serde::{
    Deserialize, Deserializer, Serialize, Serializer,
    de::{DeserializeOwned, Visitor},
};

use crate::{GeneratorError, GeneratorErrorKind, Result};

/// Positive manifest schema version.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ManifestVersion(u64);

impl ManifestVersion {
    pub fn new(value: u64) -> Result<Self> {
        if value == 0 {
            return Err(GeneratorError::new(
                GeneratorErrorKind::InvalidManifest,
                "validate manifest version",
                "schema_version must be a positive integer",
            ));
        }
        Ok(Self(value))
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    pub fn require(self, expected: Self, manifest_path: impl AsRef<Path>) -> Result<()> {
        if self != expected {
            return Err(GeneratorError::new(
                GeneratorErrorKind::InvalidManifest,
                "validate manifest version",
                format!(
                    "{} has schema_version {}, expected {}",
                    manifest_path.as_ref().display(),
                    self.get(),
                    expected.get()
                ),
            ));
        }
        Ok(())
    }
}

impl Serialize for ManifestVersion {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(self.0)
    }
}

impl<'de> Deserialize<'de> for ManifestVersion {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ManifestVersionVisitor;

        impl Visitor<'_> for ManifestVersionVisitor {
            type Value = ManifestVersion;

            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("a positive unsigned manifest version")
            }

            fn visit_u64<E>(self, value: u64) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                ManifestVersion::new(value).map_err(|error| E::custom(error.serde_message()))
            }

            fn visit_i64<E>(self, value: i64) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                let value = u64::try_from(value)
                    .map_err(|_| E::custom("InvalidManifest: schema_version must be positive"))?;
                ManifestVersion::new(value).map_err(|error| E::custom(error.serde_message()))
            }
        }

        deserializer.deserialize_u64(ManifestVersionVisitor)
    }
}

/// Parses TOML without conflating syntax errors with later semantic validation.
pub fn parse_manifest<T>(text: &str, manifest_path: impl AsRef<Path>) -> Result<T>
where
    T: DeserializeOwned,
{
    toml::from_str(text).map_err(|error| {
        GeneratorError::with_source(
            GeneratorErrorKind::InvalidManifest,
            "parse manifest TOML",
            manifest_path.as_ref().display().to_string(),
            error,
        )
    })
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::{ManifestVersion, parse_manifest};
    use crate::GeneratorErrorKind;

    #[derive(Debug, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct TestManifest {
        schema_version: ManifestVersion,
    }

    #[test]
    fn parsing_and_semantic_manifest_validation_are_distinct() {
        let parse_error = parse_manifest::<TestManifest>("schema_version =", "manifest.toml")
            .expect_err("malformed TOML must fail parsing");
        assert_eq!(parse_error.kind(), GeneratorErrorKind::InvalidManifest);

        let manifest = parse_manifest::<TestManifest>("schema_version = 2", "manifest.toml")
            .expect("well-formed manifest");
        let semantic_error = manifest
            .schema_version
            .require(ManifestVersion::new(1).expect("positive"), "manifest.toml")
            .expect_err("unsupported version must fail semantic validation");
        assert_eq!(semantic_error.kind(), GeneratorErrorKind::InvalidManifest);

        assert!(
            parse_manifest::<TestManifest>("schema_version = 1\nextra = true", "manifest.toml")
                .is_err()
        );
        assert!(
            parse_manifest::<TestManifest>(
                "schema_version = 1\nschema_version = 1",
                "manifest.toml"
            )
            .is_err()
        );
    }
}
