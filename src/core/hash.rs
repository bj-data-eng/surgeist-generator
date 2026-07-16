use std::fmt;
use std::fs::File;
use std::io::Read;
use std::path::Path;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de::Visitor};
use sha2::{Digest, Sha256};

use crate::{GeneratorError, GeneratorErrorKind, Result};

/// A canonical lowercase SHA-256 digest.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct Sha256Digest(String);

impl Sha256Digest {
    #[must_use]
    pub fn from_bytes(bytes: impl AsRef<[u8]>) -> Self {
        Self(format!("{:x}", Sha256::digest(bytes.as_ref())))
    }

    pub fn from_text(value: impl AsRef<str>) -> Result<Self> {
        let value = value.as_ref();
        if value.len() != 64
            || !value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(GeneratorError::new(
                GeneratorErrorKind::Verification,
                "validate SHA-256 digest",
                "digest must contain exactly 64 lowercase hexadecimal characters",
            ));
        }
        Ok(Self(value.to_owned()))
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let mut file = File::open(path).map_err(|error| {
            GeneratorError::with_source(
                GeneratorErrorKind::Io,
                "open file for SHA-256",
                path.display().to_string(),
                error,
            )
        })?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer).map_err(|error| {
                GeneratorError::with_source(
                    GeneratorErrorKind::Io,
                    "read file for SHA-256",
                    path.display().to_string(),
                    error,
                )
            })?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        Ok(Self(format!("{:x}", hasher.finalize())))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Sha256Digest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

impl Serialize for Sha256Digest {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Sha256Digest {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct DigestVisitor;

        impl Visitor<'_> for DigestVisitor {
            type Value = Sha256Digest;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a canonical lowercase SHA-256 string")
            }

            fn visit_str<E>(self, value: &str) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Sha256Digest::from_text(value).map_err(|error| E::custom(error.serde_message()))
            }

            fn visit_string<E>(self, value: String) -> std::result::Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Sha256Digest::from_text(value).map_err(|error| E::custom(error.serde_message()))
            }
        }

        deserializer.deserialize_str(DigestVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::Sha256Digest;

    #[test]
    fn hash_text_validation_rejects_noncanonical_text() {
        assert!(Sha256Digest::from_text("0".repeat(63)).is_err());
        assert!(Sha256Digest::from_text("A".repeat(64)).is_err());
        assert!(Sha256Digest::from_text("g".repeat(64)).is_err());
        assert_eq!(
            Sha256Digest::from_bytes(b"abc").as_str(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
