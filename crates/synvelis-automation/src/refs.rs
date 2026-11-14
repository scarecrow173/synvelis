//! Versioned references shared by definitions and registry snapshots.
//!
//! [`VersionedRef`] is the DSL-facing `name@major.minor` reference used
//! for capabilities, macros, and schema refs. It serializes as a single
//! canonical string so digests are stable regardless of JSON key order.
//!
//! [`Version`] is the `{major, minor}` pair used inside registry
//! snapshots where name and version are separate fields (mirroring the
//! gateway's `CapabilityVersion`/`SchemaVersion` wire shape).

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::AutomationError;

/// A `major.minor` version pair.
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(deny_unknown_fields)]
pub struct Version {
    /// Breaking-change epoch.
    pub major: u32,
    /// Backward-compatible revision within `major`.
    pub minor: u32,
}

impl Version {
    /// Creates a version.
    #[must_use]
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// A `name@major.minor` reference.
///
/// `id` is the referenced name: a capability id, a macro id, or a
/// schema name depending on position. The wire form is a single
/// canonical string; parsing rejects non-canonical spellings (leading
/// zeros, extra components, whitespace) so one meaning has exactly one
/// serialization.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct VersionedRef {
    /// The referenced name (capability id / macro id / schema name).
    pub id: String,
    /// Exact requested version.
    pub version: Version,
}

impl VersionedRef {
    /// Creates a reference.
    #[must_use]
    pub fn new(id: impl Into<String>, major: u32, minor: u32) -> Self {
        Self {
            id: id.into(),
            version: Version::new(major, minor),
        }
    }

    /// Parses `name@major.minor`.
    ///
    /// # Errors
    /// `Err(reason)` on malformed input.
    pub fn parse(text: &str) -> Result<Self, String> {
        let (id, ver) = text
            .rsplit_once('@')
            .ok_or_else(|| format!("reference '{text}' is not name@major.minor"))?;
        if id.is_empty() {
            return Err(format!("reference '{text}' has an empty name"));
        }
        if id
            .chars()
            .any(|c| c.is_whitespace() || c == '@' || c == '#')
        {
            return Err(format!("reference '{text}' has an invalid name"));
        }
        let (major, minor) = ver
            .split_once('.')
            .ok_or_else(|| format!("reference '{text}' has a malformed version"))?;
        Ok(Self {
            id: id.to_owned(),
            version: Version {
                major: parse_version_part(major, text)?,
                minor: parse_version_part(minor, text)?,
            },
        })
    }
}

fn parse_version_part(part: &str, whole: &str) -> Result<u32, String> {
    // Canonical decimal only: no sign, no leading zeros (except "0").
    if part.is_empty()
        || (part.len() > 1 && part.starts_with('0'))
        || !part.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(format!(
            "reference '{whole}' has a non-canonical version part"
        ));
    }
    part.parse::<u32>()
        .map_err(|_| format!("reference '{whole}' version out of range"))
}

impl fmt::Display for VersionedRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.id, self.version)
    }
}

impl Serialize for VersionedRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for VersionedRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// Converts a [`VersionedRef::parse`] failure into the crate error type.
#[must_use]
pub fn ref_parse_error(reason: String) -> AutomationError {
    AutomationError::Definition { reason }
}
