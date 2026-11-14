//! Structural value schemas and static types.
//!
//! [`SchemaNode`] mirrors the gateway's `ValueSchema` contract as a
//! serializable snapshot form — the live Gateway Descriptor Read
//! Adapter (integration slice) projects `synvelis-gateway` schemas
//! into this shape; this crate never depends on the gateway.
//!
//! [`StaticType`] is the compile-time type of a referenced value
//! (`$input`, `$step_output`, `$item`): derived from a [`SchemaNode`]
//! or synthesized for step kinds that produce built-in outputs
//! (`async-submit`, `await-job`). [`SchemaNode::Job`] is the single
//! extension beyond the gateway mirror — it types workflow job handles
//! so `await-job` can only consume values that are provably job refs.

use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{AutomationError, CompileError};

/// Resource kinds — mirrors `synvelis-gateway`'s `ResourceKind` wire
/// tags. Kept as data (not project types) so this crate stays
/// headless and gateway-independent; the tagged-id *format*
/// (`<kind>-<32 hex>`) is validated identically.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ResourceKind {
    /// The project aggregate.
    Project,
    /// A track.
    Track,
    /// A lane.
    Lane,
    /// A clip.
    Clip,
    /// A media or MIDI source.
    Source,
    /// A stored asset.
    Asset,
    /// A note entity.
    Note,
    /// A take group.
    TakeGroup,
    /// A comp.
    Comp,
    /// A parameter target.
    ParameterTarget,
    /// An automation lane.
    AutomationLane,
    /// A channel strip.
    Strip,
    /// A send config.
    Send,
    /// A routing processor.
    Processor,
}

impl ResourceKind {
    /// The kebab-case wire tag.
    #[must_use]
    pub const fn tag(&self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Track => "track",
            Self::Lane => "lane",
            Self::Clip => "clip",
            Self::Source => "source",
            Self::Asset => "asset",
            Self::Note => "note",
            Self::TakeGroup => "take-group",
            Self::Comp => "comp",
            Self::ParameterTarget => "param",
            Self::AutomationLane => "automation-lane",
            Self::Strip => "strip",
            Self::Send => "send",
            Self::Processor => "processor",
        }
    }

    /// Parses a wire kind tag (accepts both `param` and
    /// `parameter-target`, mirroring the gateway).
    #[must_use]
    pub fn parse_tag(name: &str) -> Option<Self> {
        Some(match name {
            "project" => Self::Project,
            "track" => Self::Track,
            "lane" => Self::Lane,
            "clip" => Self::Clip,
            "source" => Self::Source,
            "asset" => Self::Asset,
            "note" => Self::Note,
            "take-group" => Self::TakeGroup,
            "comp" => Self::Comp,
            "param" | "parameter-target" => Self::ParameterTarget,
            "automation-lane" => Self::AutomationLane,
            "strip" => Self::Strip,
            "send" => Self::Send,
            "processor" => Self::Processor,
            _ => return None,
        })
    }
}

impl fmt::Display for ResourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.tag())
    }
}

/// Time coordinate domains — mirrors the gateway `TimeDomain` tags.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TimeDomain {
    /// Project timeline in PPQN ticks.
    ProjectMusical,
    /// Media time-base instants.
    Media,
    /// Source-native ticks.
    SourceNative,
}

/// A field inside [`SchemaNode::Object`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldNode {
    /// The field's schema.
    pub schema: SchemaNode,
    /// Whether the field must be present.
    pub required: bool,
}

/// The structural schema a value is validated against — the
/// serializable mirror of the gateway `ValueSchema` plus the
/// workflow-local [`Self::Job`] extension.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SchemaNode {
    /// A JSON boolean.
    Bool,
    /// A JSON integer within optional inclusive bounds.
    Integer {
        /// Inclusive minimum.
        min: Option<i64>,
        /// Inclusive maximum.
        max: Option<i64>,
    },
    /// Any JSON number.
    Number,
    /// A JSON string within an optional length bound (chars).
    String {
        /// Maximum length in chars.
        max_len: Option<usize>,
    },
    /// A tagged stable id string of exactly `kind`
    /// (`<kind>-<32 hex>`).
    Id {
        /// The required resource kind.
        kind: ResourceKind,
    },
    /// One of a fixed string set.
    Enum {
        /// Accepted values.
        values: Vec<String>,
    },
    /// A JSON array of `item`.
    Array {
        /// Element schema.
        item: Box<SchemaNode>,
        /// Maximum element count.
        max_len: Option<usize>,
    },
    /// A JSON object with declared fields.
    Object {
        /// Field name → field schema.
        #[serde(default)]
        fields: BTreeMap<String, FieldNode>,
        /// Whether undeclared fields are rejected.
        #[serde(default)]
        deny_unknown: bool,
    },
    /// `null` or a value of the inner schema.
    Optional {
        /// Inner schema.
        inner: Box<SchemaNode>,
    },
    /// An explicit `{domain, start, len}` range in `domain`.
    TimeRange {
        /// The required time domain.
        domain: TimeDomain,
    },
    /// A workflow job handle — produced only by `async-submit` step
    /// outputs or declared on workflow inputs. Not a gateway
    /// `ValueSchema` node; capability schemas never contain it.
    Job,
}

/// The compile-time static type of a value expression.
#[derive(Clone, Debug, PartialEq)]
pub enum StaticType {
    /// Always null.
    Null,
    /// Boolean.
    Bool,
    /// Integer.
    Int,
    /// Any number (integer or non-integer).
    Number,
    /// String (including enum-typed strings).
    String,
    /// Tagged stable id of `kind`.
    Id(ResourceKind),
    /// Array of `item`.
    Array(Box<StaticType>),
    /// Object with declared fields.
    Object(BTreeMap<String, FieldType>),
    /// `null` or `inner`.
    Optional(Box<StaticType>),
    /// `{domain, start, len}` time range.
    TimeRange,
    /// A workflow job handle.
    Job,
    /// Opaque JSON — produced only by `await-job` results whose
    /// `output_schema` was not declared. Assignable anywhere; the
    /// runtime Gateway re-validates concretely (documented v1
    /// relaxation — static checking cannot see inside job payloads).
    Json,
}

/// A field inside [`StaticType::Object`].
#[derive(Clone, Debug, PartialEq)]
pub struct FieldType {
    /// The field's static type.
    pub ty: StaticType,
    /// Whether the field must be present.
    pub required: bool,
}

impl StaticType {
    /// Derives the static type of a [`SchemaNode`].
    #[must_use]
    pub fn from_schema(node: &SchemaNode) -> Self {
        match node {
            SchemaNode::Bool => Self::Bool,
            SchemaNode::Integer { .. } => Self::Int,
            SchemaNode::Number => Self::Number,
            SchemaNode::String { .. } | SchemaNode::Enum { .. } => Self::String,
            SchemaNode::Id { kind } => Self::Id(*kind),
            SchemaNode::Array { item, .. } => Self::Array(Box::new(Self::from_schema(item))),
            SchemaNode::Object { fields, .. } => Self::Object(
                fields
                    .iter()
                    .map(|(name, f)| {
                        (
                            name.clone(),
                            FieldType {
                                ty: Self::from_schema(&f.schema),
                                required: f.required,
                            },
                        )
                    })
                    .collect(),
            ),
            SchemaNode::Optional { inner } => Self::Optional(Box::new(Self::from_schema(inner))),
            SchemaNode::TimeRange { .. } => Self::TimeRange,
            SchemaNode::Job => Self::Job,
        }
    }

    /// Human-readable type name for diagnostics.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Self::Null => "null".to_owned(),
            Self::Bool => "bool".to_owned(),
            Self::Int => "int".to_owned(),
            Self::Number => "number".to_owned(),
            Self::String => "string".to_owned(),
            Self::Id(kind) => format!("id:{kind}"),
            Self::Array(item) => format!("array[{}]", item.describe()),
            Self::Object(fields) => {
                let inner: Vec<String> = fields
                    .iter()
                    .map(|(name, f)| {
                        format!(
                            "{}{}:{}",
                            name,
                            if f.required { "" } else { "?" },
                            f.ty.describe()
                        )
                    })
                    .collect();
                format!("object{{{}}}", inner.join(","))
            }
            Self::Optional(inner) => format!("optional[{}]", inner.describe()),
            Self::TimeRange => "time-range".to_owned(),
            Self::Job => "job".to_owned(),
            Self::Json => "json".to_owned(),
        }
    }
}

/// Whether `actual` is assignable to `expected`.
///
/// `expected` is the static type derived from the target [`SchemaNode`].
/// `Json` is assignable to anything — it is the documented opaque
/// escape for awaited-job payloads whose shape the compiler cannot see.
/// An `Optional` *actual* never satisfies a required `expected` (the
/// value may be absent at runtime); an `Optional` *expected* accepts a
/// compatible concrete type, `null`, or a compatible optional.
#[must_use]
pub fn type_compatible(actual: &StaticType, expected: &StaticType) -> bool {
    use StaticType as T;
    if matches!(actual, T::Json) {
        return true;
    }
    if let T::Optional(inner) = expected {
        return matches!(actual, T::Null)
            || type_compatible(actual, inner)
            || matches!(actual, T::Optional(a) if type_compatible(a, inner));
    }
    match (actual, expected) {
        (T::Int, T::Int | T::Number)
        | (T::Number, T::Number)
        | (T::Bool, T::Bool)
        | (T::String, T::String)
        | (T::TimeRange, T::TimeRange)
        | (T::Job, T::Job) => true,
        (T::Id(a), T::Id(e)) => a == e,
        (T::Array(a), T::Array(e)) => type_compatible(a, e),
        (T::Object(a), T::Object(e)) => {
            for (name, ef) in e {
                match a.get(name) {
                    Some(af) => {
                        if !type_compatible(&af.ty, &ef.ty) {
                            return false;
                        }
                        // A required expected field needs a guaranteed
                        // present actual field.
                        if ef.required && !af.required {
                            return false;
                        }
                    }
                    None if ef.required => return false,
                    None => {}
                }
            }
            // Extra actual fields are permitted statically; a schema's
            // `deny_unknown` is enforced concretely for literal args
            // and re-validated by the Gateway at execution for refs.
            true
        }
        _ => false,
    }
}

/// Validates a concrete JSON `value` against `schema` at `path`.
///
/// Used for provided workflow inputs. Mirrors the gateway
/// `ValueSchema::validate` rules, including typed id and time-range
/// checks.
///
/// # Errors
/// [`CompileError::SchemaViolation`] on the first mismatch.
#[allow(clippy::too_many_lines)] // flat match per schema variant
pub fn validate_value(schema: &SchemaNode, path: &str, value: &Value) -> Result<(), CompileError> {
    let violation = |reason: &str| CompileError::SchemaViolation {
        path: path.to_owned(),
        reason: reason.to_owned(),
    };
    match schema {
        SchemaNode::Bool => {
            if !value.is_boolean() {
                return Err(violation("expected a boolean"));
            }
        }
        SchemaNode::Integer { min, max } => {
            let n = value
                .as_i64()
                .ok_or_else(|| violation("expected an integer"))?;
            if let Some(min) = min
                && n < *min
            {
                return Err(violation(&format!("below minimum {min}")));
            }
            if let Some(max) = max
                && n > *max
            {
                return Err(violation(&format!("above maximum {max}")));
            }
        }
        SchemaNode::Number => {
            if !value.is_number() {
                return Err(violation("expected a number"));
            }
        }
        SchemaNode::String { max_len } => {
            let s = value
                .as_str()
                .ok_or_else(|| violation("expected a string"))?;
            if let Some(max) = max_len
                && s.chars().count() > *max
            {
                return Err(violation(&format!("longer than {max} chars")));
            }
        }
        SchemaNode::Id { kind } => {
            let s = value
                .as_str()
                .ok_or_else(|| violation("expected a tagged id string"))?;
            validate_tagged_id(*kind, s).map_err(|reason| violation(&reason))?;
        }
        SchemaNode::Enum { values } => {
            let s = value
                .as_str()
                .ok_or_else(|| violation("expected an enum string"))?;
            if !values.iter().any(|v| v == s) {
                return Err(violation("value not in enum"));
            }
        }
        SchemaNode::Array { item, max_len } => {
            let arr = value
                .as_array()
                .ok_or_else(|| violation("expected an array"))?;
            if let Some(max) = max_len
                && arr.len() > *max
            {
                return Err(violation(&format!("more than {max} elements")));
            }
            for (i, v) in arr.iter().enumerate() {
                validate_value(item, &format!("{path}[{i}]"), v)?;
            }
        }
        SchemaNode::Object {
            fields,
            deny_unknown,
        } => {
            let obj = value
                .as_object()
                .ok_or_else(|| violation("expected an object"))?;
            for (name, field) in fields {
                let child = format!("{path}.{name}");
                match obj.get(name) {
                    Some(v) => validate_value(&field.schema, &child, v)?,
                    None if field.required => {
                        return Err(violation(&format!("missing required field {name}")));
                    }
                    None => {}
                }
            }
            if *deny_unknown {
                for name in obj.keys() {
                    if !fields.contains_key(name) {
                        return Err(violation(&format!("unknown field {name}")));
                    }
                }
            }
        }
        SchemaNode::Optional { inner } => {
            if !value.is_null() {
                validate_value(inner, path, value)?;
            }
        }
        SchemaNode::TimeRange { domain } => {
            validate_time_range(*domain, path, value)?;
        }
        SchemaNode::Job => {
            // Job handles are opaque tokens; any non-empty string is a
            // syntactically valid handle. Semantic job existence is a
            // runtime concern (Gateway re-validates).
            match value {
                Value::String(s) if !s.is_empty() => {}
                _ => return Err(violation("expected a non-empty job handle string")),
            }
        }
    }
    Ok(())
}

/// Validates a tagged stable id `"<kind>-<32 hex>"` against `kind`.
///
/// Mirrors the gateway `ResourceRef::expect` format check without
/// depending on `synvelis-project` id types.
///
/// # Errors
/// `Err(reason)` on malformed ids or kind-tag mismatches.
pub fn validate_tagged_id(kind: ResourceKind, id: &str) -> Result<(), String> {
    let (tag, hex) = id
        .rsplit_once('-')
        .ok_or_else(|| format!("malformed tagged id: {id}"))?;
    let expected_tag = kind.tag();
    // `param` ids serialize under the `param` tag.
    let tag_ok =
        tag == expected_tag || (kind == ResourceKind::ParameterTarget && tag == "parameter-target");
    if !tag_ok {
        return Err(format!(
            "id tag '{tag}' does not match kind '{expected_tag}'"
        ));
    }
    if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("malformed tagged id payload: {id}"));
    }
    Ok(())
}

fn validate_time_range(
    expected: TimeDomain,
    path: &str,
    value: &Value,
) -> Result<(), CompileError> {
    let violation = |reason: &str| CompileError::SchemaViolation {
        path: path.to_owned(),
        reason: reason.to_owned(),
    };
    let obj = value
        .as_object()
        .ok_or_else(|| violation("expected a {domain,start,len} range object"))?;
    let domain_str = obj
        .get("domain")
        .and_then(Value::as_str)
        .ok_or_else(|| violation("missing domain tag"))?;
    let actual = match domain_str {
        "project-musical" => TimeDomain::ProjectMusical,
        "media" => TimeDomain::Media,
        "source-native" => TimeDomain::SourceNative,
        _ => return Err(violation(&format!("unknown time domain: {domain_str}"))),
    };
    if actual != expected {
        return Err(violation(&format!(
            "time domain mismatch: expected {expected:?}, got {actual:?}"
        )));
    }
    let start = obj
        .get("start")
        .and_then(Value::as_i64)
        .ok_or_else(|| violation("expected an integer start"))?;
    let len = obj
        .get("len")
        .and_then(Value::as_i64)
        .ok_or_else(|| violation("expected an integer len"))?;
    if len <= 0 {
        return Err(violation("len must be positive"));
    }
    let _ = start;
    Ok(())
}

/// Wraps a [`CompileError`] into [`AutomationError`].
#[must_use]
pub fn as_automation(err: CompileError) -> AutomationError {
    AutomationError::Compile(err)
}
