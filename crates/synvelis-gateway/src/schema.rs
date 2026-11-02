//! Input/output schema contract and validation.
//!
//! Schemas are versioned by name: [`SchemaRef`] = `name@major.minor`.
//! A wire request declares the schema version it speaks; the gateway
//! resolves it in the [`SchemaRegistry`] and checks compatibility with
//! the capability's declared input schema before validating args.
//!
//! Compatibility rule (documented, deterministic):
//! `requested.name == declared.name && requested.major == declared.major
//! && requested.minor <= declared.minor` — a client may speak any
//! registered same-major schema at or below the declared minor. Newer
//! minors and other majors are rejected, never coerced.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::outcome::GatewayError;
use crate::request::{ResourceKind, TimeDomain, TypedRange};

/// Semantic schema version — same ordering rules as capability versions.
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(deny_unknown_fields)]
pub struct SchemaVersion {
    /// Breaking-change epoch.
    pub major: u32,
    /// Backward-compatible revision within `major`.
    pub minor: u32,
}

impl SchemaVersion {
    /// Creates a version.
    #[must_use]
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }
}

impl fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// A versioned reference to a named schema.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaRef {
    /// Registered schema name.
    pub name: String,
    /// Exact version the client speaks.
    pub version: SchemaVersion,
}

impl SchemaRef {
    /// Creates a reference.
    #[must_use]
    pub fn new(name: impl Into<String>, major: u32, minor: u32) -> Self {
        Self {
            name: name.into(),
            version: SchemaVersion::new(major, minor),
        }
    }

    /// The compatibility rule: same name and major, requested minor at
    /// or below the declared minor.
    #[must_use]
    pub fn is_compatible_with(&self, declared: &Self) -> bool {
        self.name == declared.name
            && self.version.major == declared.version.major
            && self.version.minor <= declared.version.minor
    }
}

impl fmt::Display for SchemaRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.name, self.version)
    }
}

/// A field inside a [`ValueSchema::Object`].
#[derive(Clone, Debug)]
pub struct FieldSchema {
    /// The field's value schema.
    pub schema: ValueSchema,
    /// Whether the field must be present.
    pub required: bool,
}

impl FieldSchema {
    /// Required field.
    #[must_use]
    pub const fn required(schema: ValueSchema) -> Self {
        Self {
            schema,
            required: true,
        }
    }

    /// Optional field.
    #[must_use]
    pub const fn optional(schema: ValueSchema) -> Self {
        Self {
            schema,
            required: false,
        }
    }
}

/// The structural schema a wire argument is validated against.
///
/// Deliberately a compact, total model — not general JSON Schema. Two
/// nodes produce *typed* domain errors instead of generic violations:
/// [`Id`][ValueSchema::Id] rejects a tagged id of the wrong kind, and
/// [`TimeRange`][ValueSchema::TimeRange] rejects a range in the wrong
/// time domain.
#[derive(Clone, Debug)]
pub enum ValueSchema {
    /// A JSON boolean.
    Bool,
    /// A JSON integer within optional bounds.
    Integer {
        /// Inclusive minimum.
        min: Option<i64>,
        /// Inclusive maximum.
        max: Option<i64>,
    },
    /// Any JSON number.
    Number,
    /// A JSON string within an optional length bound.
    String {
        /// Maximum length in chars.
        max_len: Option<usize>,
    },
    /// A tagged stable id string of exactly `kind`.
    Id {
        /// The required resource kind.
        kind: ResourceKind,
    },
    /// One of a fixed string set.
    Enum {
        /// The accepted values.
        values: Vec<String>,
    },
    /// A JSON array of `item`.
    Array {
        /// Element schema.
        item: Box<ValueSchema>,
        /// Maximum element count.
        max_len: Option<usize>,
    },
    /// A JSON object with declared fields.
    Object {
        /// Field name → field schema.
        fields: BTreeMap<String, FieldSchema>,
        /// Whether undeclared fields are rejected.
        deny_unknown: bool,
    },
    /// `null` or a value of the inner schema.
    Optional(Box<ValueSchema>),
    /// An explicit `{domain, start, len}` range in the required
    /// [`TimeDomain`] — a wrong domain is a typed
    /// [`GatewayError::WrongTimeDomain`].
    TimeRange {
        /// The required time domain.
        domain: TimeDomain,
    },
}

/// A named, versioned schema store.
///
/// Registration is monotonic: a `(name, version)` pair is immutable once
/// registered so a schema's meaning cannot shift under existing
/// capabilities.
#[derive(Debug, Default)]
pub struct SchemaRegistry {
    schemas: BTreeMap<String, BTreeMap<SchemaVersion, ValueSchema>>,
}

impl SchemaRegistry {
    /// Empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `schema` as `name@version`.
    ///
    /// # Errors
    /// [`GatewayError::SchemaConflict`] — same name+version already
    /// registered (a schema version is immutable once published).
    pub fn register(
        &mut self,
        name: impl Into<String>,
        version: SchemaVersion,
        schema: ValueSchema,
    ) -> Result<(), GatewayError> {
        let name = name.into();
        let versions = self.schemas.entry(name.clone()).or_default();
        if versions.contains_key(&version) {
            return Err(GatewayError::SchemaConflict { name, version });
        }
        versions.insert(version, schema);
        Ok(())
    }

    /// Resolves `schema_ref` to its registered schema.
    ///
    /// # Errors
    /// - [`GatewayError::UnknownSchema`] — name not registered.
    /// - [`GatewayError::UnsupportedSchemaVersion`] — version not
    ///   registered for that name.
    pub fn resolve(&self, schema_ref: &SchemaRef) -> Result<&ValueSchema, GatewayError> {
        let versions =
            self.schemas
                .get(&schema_ref.name)
                .ok_or_else(|| GatewayError::UnknownSchema {
                    name: schema_ref.name.clone(),
                })?;
        versions
            .get(&schema_ref.version)
            .ok_or_else(|| GatewayError::UnsupportedSchemaVersion {
                name: schema_ref.name.clone(),
                requested: schema_ref.version,
                supported: versions.keys().copied().collect(),
            })
    }

    /// Versions registered for `name` (empty when unknown).
    #[must_use]
    pub fn versions(&self, name: &str) -> Vec<SchemaVersion> {
        self.schemas
            .get(name)
            .map(|v| v.keys().copied().collect())
            .unwrap_or_default()
    }
}

fn violation(path: &str, reason: impl Into<String>) -> GatewayError {
    GatewayError::SchemaViolation {
        path: path.to_owned(),
        reason: reason.into(),
    }
}

impl ValueSchema {
    /// Validates `value` against the schema at JSON `path`.
    ///
    /// # Errors
    /// - [`GatewayError::SchemaViolation`] — structural mismatch.
    /// - [`GatewayError::WrongResourceKind`] — [`Id`][ValueSchema::Id]
    ///   tag mismatch.
    /// - [`GatewayError::WrongTimeDomain`] —
    ///   [`TimeRange`][ValueSchema::TimeRange] domain mismatch.
    /// - [`GatewayError::InvalidId`] /
    ///   [`GatewayError::UnknownResourceKind`] — unparseable values.
    #[allow(clippy::too_many_lines)] // flat match per schema variant
    pub fn validate(&self, path: &str, value: &Value) -> Result<(), GatewayError> {
        match self {
            Self::Bool => {
                if !value.is_boolean() {
                    return Err(violation(path, "expected a boolean"));
                }
            }
            Self::Integer { min, max } => {
                let n = value
                    .as_i64()
                    .ok_or_else(|| violation(path, "expected an integer"))?;
                if let Some(min) = min
                    && n < *min
                {
                    return Err(violation(path, format!("below minimum {min}")));
                }
                if let Some(max) = max
                    && n > *max
                {
                    return Err(violation(path, format!("above maximum {max}")));
                }
            }
            Self::Number => {
                if !value.is_number() {
                    return Err(violation(path, "expected a number"));
                }
            }
            Self::String { max_len } => {
                let s = value
                    .as_str()
                    .ok_or_else(|| violation(path, "expected a string"))?;
                if let Some(max) = max_len
                    && s.chars().count() > *max
                {
                    return Err(violation(path, format!("longer than {max} chars")));
                }
            }
            Self::Id { kind } => {
                let s = value
                    .as_str()
                    .ok_or_else(|| violation(path, "expected a tagged id string"))?;
                crate::request::ResourceRef::expect(*kind, s)?;
            }
            Self::Enum { values } => {
                let s = value
                    .as_str()
                    .ok_or_else(|| violation(path, "expected an enum string"))?;
                if !values.iter().any(|v| v == s) {
                    return Err(violation(
                        path,
                        format!("expected one of {}", values.join(", ")),
                    ));
                }
            }
            Self::Array { item, max_len } => {
                let arr = value
                    .as_array()
                    .ok_or_else(|| violation(path, "expected an array"))?;
                if let Some(max) = max_len
                    && arr.len() > *max
                {
                    return Err(violation(path, format!("more than {max} elements")));
                }
                for (i, v) in arr.iter().enumerate() {
                    item.validate(&format!("{path}[{i}]"), v)?;
                }
            }
            Self::Object {
                fields,
                deny_unknown,
            } => {
                let obj = value
                    .as_object()
                    .ok_or_else(|| violation(path, "expected an object"))?;
                for (name, field) in fields {
                    let child = format!("{path}.{name}");
                    match obj.get(name) {
                        Some(v) => field.schema.validate(&child, v)?,
                        None if field.required => {
                            return Err(violation(&child, "missing required field"));
                        }
                        None => {}
                    }
                }
                if *deny_unknown {
                    for name in obj.keys() {
                        if !fields.contains_key(name) {
                            return Err(violation(&format!("{path}.{name}"), "unknown field"));
                        }
                    }
                }
            }
            Self::Optional(inner) => {
                if !value.is_null() {
                    inner.validate(path, value)?;
                }
            }
            Self::TimeRange { domain } => {
                Self::validate_time_range(*domain, path, value)?;
            }
        }
        Ok(())
    }

    fn validate_time_range(
        expected: TimeDomain,
        path: &str,
        value: &Value,
    ) -> Result<(), GatewayError> {
        let obj = value
            .as_object()
            .ok_or_else(|| violation(path, "expected a {domain,start,len} range object"))?;
        let domain_str = obj
            .get("domain")
            .and_then(Value::as_str)
            .ok_or_else(|| violation(&format!("{path}.domain"), "missing domain tag"))?;
        let actual = match domain_str {
            "project-musical" => TimeDomain::ProjectMusical,
            "media" => TimeDomain::Media,
            "source-native" => TimeDomain::SourceNative,
            _ => {
                return Err(violation(
                    &format!("{path}.domain"),
                    format!("unknown time domain: {domain_str}"),
                ));
            }
        };
        if actual != expected {
            return Err(GatewayError::WrongTimeDomain { expected, actual });
        }
        let start = obj
            .get("start")
            .and_then(Value::as_i64)
            .ok_or_else(|| violation(&format!("{path}.start"), "expected an integer"))?;
        let len = obj
            .get("len")
            .and_then(Value::as_i64)
            .ok_or_else(|| violation(&format!("{path}.len"), "expected an integer"))?;
        if len <= 0 {
            return Err(violation(&format!("{path}.len"), "must be positive"));
        }
        let _ = TypedRange {
            domain: actual,
            start,
            len,
        };
        Ok(())
    }
}
