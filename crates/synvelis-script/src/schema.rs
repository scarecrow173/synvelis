//! Serializable mirror of the Gateway [`ValueSchema`] contract.
//!
//! `synvelis-gateway`'s `ValueSchema`/`FieldSchema` are deliberately not
//! serde types — they are the runtime validation contract. This module
//! carries the *same structure* as plain serializable data so registry
//! snapshots, the script API model, bindings, and stubs can be canonicalized
//! and digest-pinned. [`ScriptSchema::to_value_schema`] reconstructs the
//! authoritative runtime schema for validation; nothing here re-implements
//! validation rules.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use synvelis_gateway::{FieldSchema, ResourceKind, TimeDomain, ValueSchema};

/// A field inside a [`ScriptSchema::Object`].
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptField {
    /// The field's value schema.
    pub schema: ScriptSchema,
    /// Whether the field must be present.
    pub required: bool,
}

/// Serializable mirror of [`ValueSchema`] — the snapshot/model/binding
/// representation of a Gateway argument schema.
///
/// Externally tagged (`{"integer": {"min": …}}`, `"bool"`, …) so the
/// canonical digest form is stable and self-describing. The variant set is
/// total over `ValueSchema`: every Gateway schema node maps to exactly one
/// `ScriptSchema` node and back.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScriptSchema {
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
        item: Box<ScriptSchema>,
        /// Maximum element count.
        max_len: Option<usize>,
    },
    /// A JSON object with declared fields.
    Object {
        /// Field name → field schema.
        fields: BTreeMap<String, ScriptField>,
        /// Whether undeclared fields are rejected.
        deny_unknown: bool,
    },
    /// `null` or a value of the inner schema.
    Optional(Box<ScriptSchema>),
    /// An explicit `{domain, start, len}` range in the required
    /// [`TimeDomain`].
    TimeRange {
        /// The required time domain.
        domain: TimeDomain,
    },
}

impl ScriptSchema {
    /// Projects a Gateway [`ValueSchema`] into its serializable mirror.
    /// Total: every variant is covered.
    #[must_use]
    pub fn from_value_schema(schema: &ValueSchema) -> Self {
        match schema {
            ValueSchema::Bool => Self::Bool,
            ValueSchema::Integer { min, max } => Self::Integer {
                min: *min,
                max: *max,
            },
            ValueSchema::Number => Self::Number,
            ValueSchema::String { max_len } => Self::String { max_len: *max_len },
            ValueSchema::Id { kind } => Self::Id { kind: *kind },
            ValueSchema::Enum { values } => Self::Enum {
                values: values.clone(),
            },
            ValueSchema::Array { item, max_len } => Self::Array {
                item: Box::new(Self::from_value_schema(item)),
                max_len: *max_len,
            },
            ValueSchema::Object {
                fields,
                deny_unknown,
            } => Self::Object {
                fields: fields
                    .iter()
                    .map(|(name, field)| {
                        (
                            name.clone(),
                            ScriptField {
                                schema: Self::from_value_schema(&field.schema),
                                required: field.required,
                            },
                        )
                    })
                    .collect(),
                deny_unknown: *deny_unknown,
            },
            ValueSchema::Optional(inner) => {
                Self::Optional(Box::new(Self::from_value_schema(inner)))
            }
            ValueSchema::TimeRange { domain } => Self::TimeRange { domain: *domain },
        }
    }

    /// Reconstructs the authoritative Gateway [`ValueSchema`].
    ///
    /// This is the only place `ScriptSchema` re-enters the runtime
    /// contract; argument validation always runs against the returned
    /// `ValueSchema`, never against `ScriptSchema` itself.
    #[must_use]
    pub fn to_value_schema(&self) -> ValueSchema {
        match self {
            Self::Bool => ValueSchema::Bool,
            Self::Integer { min, max } => ValueSchema::Integer {
                min: *min,
                max: *max,
            },
            Self::Number => ValueSchema::Number,
            Self::String { max_len } => ValueSchema::String { max_len: *max_len },
            Self::Id { kind } => ValueSchema::Id { kind: *kind },
            Self::Enum { values } => ValueSchema::Enum {
                values: values.clone(),
            },
            Self::Array { item, max_len } => ValueSchema::Array {
                item: Box::new(item.to_value_schema()),
                max_len: *max_len,
            },
            Self::Object {
                fields,
                deny_unknown,
            } => ValueSchema::Object {
                fields: fields
                    .iter()
                    .map(|(name, field)| {
                        (
                            name.clone(),
                            FieldSchema {
                                schema: field.schema.to_value_schema(),
                                required: field.required,
                            },
                        )
                    })
                    .collect(),
                deny_unknown: *deny_unknown,
            },
            Self::Optional(inner) => ValueSchema::Optional(Box::new(inner.to_value_schema())),
            Self::TimeRange { domain } => ValueSchema::TimeRange { domain: *domain },
        }
    }
}
