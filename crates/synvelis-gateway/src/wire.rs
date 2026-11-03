//! Wire request contract — the only deserialization boundary.
//!
//! [`WireRequest`] is a pure data shape: it has **no caller, principal,
//! authority, or permission fields** — by construction there is nothing
//! on the wire that could mint trusted authority (GT-002). Two explicit
//! rejections are enforced before serde even runs:
//!
//! - authority-shaped top-level keys →
//!   [`GatewayError::SelfDeclaredAuthority`]
//! - implicit-targeting keys (UI selection / focus / active window) →
//!   [`GatewayError::ImplicitTargetRejected`] (GT-001)
//!
//! Everything else unknown is rejected by `deny_unknown_fields` as
//! [`GatewayError::MalformedRequest`]. Parsing produces only data;
//! turning it into an [`crate::OperationRequest`] is
//! [`crate::Gateway::bind`]'s separate responsibility.

use serde::Deserialize;
use serde_json::Value;
use synvelis_project::ProjectRevision;

use crate::capability::CapabilityRef;
use crate::id::RequestId;
use crate::outcome::GatewayError;
use crate::schema::SchemaRef;

/// Top-level keys that assert trusted authority — always rejected.
pub const FORBIDDEN_AUTHORITY_FIELDS: &[&str] = &[
    "principal",
    "caller",
    "authority",
    "auth",
    "authenticated",
    "token",
    "permissions",
    "permission",
    "approved",
    "approval",
    "impersonate",
    "as_user",
    "role",
];

/// Top-level keys that imply implicit targeting — always rejected.
pub const FORBIDDEN_IMPLICIT_TARGET_FIELDS: &[&str] = &[
    "selection",
    "focus",
    "active_window",
    "foreground_window",
    "current_selection",
];

/// The wire form of an operation request.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WireRequest {
    /// Caller-assigned correlation id.
    pub request_id: RequestId,
    /// The capability to invoke (exact versioned ref).
    pub capability: CapabilityRef,
    /// The input schema name+version the client speaks.
    pub schema: SchemaRef,
    /// Tagged project id (`"project-<32hex>"`).
    pub project: String,
    /// Explicit target — required iff the capability's
    /// [`crate::TargetContract`] requires one.
    #[serde(default)]
    pub target: Option<WireTarget>,
    /// Operation arguments (validated against the input schema).
    #[serde(default)]
    pub args: Value,
    /// Domain-appropriate consistency requirement.
    pub consistency: WireConsistency,
    /// Optional opaque task reference — not an authorization proof.
    #[serde(default)]
    pub task: Option<String>,
    /// Optional opaque grant reference — not an authorization proof.
    #[serde(default)]
    pub grant: Option<String>,
    /// Optional idempotency key.
    #[serde(default)]
    pub idempotency_key: Option<String>,
}

/// The wire form of an explicit target (`"shape"` discriminates).
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "shape", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WireTarget {
    /// One existing resource.
    Resource {
        /// Declared resource kind tag.
        kind: String,
        /// Tagged stable id.
        id: String,
    },
    /// A non-empty set of existing resources of one kind.
    ResourceSet {
        /// Declared resource kind tag.
        kind: String,
        /// Tagged stable ids.
        ids: Vec<String>,
    },
    /// Create a new resource under an explicit parent.
    Create {
        /// Declared kind of the parent.
        parent_kind: String,
        /// Tagged stable id of the parent.
        parent: String,
        /// Declared kind of the resource to create.
        creates: String,
    },
}

/// The wire form of a consistency requirement (tagged `"kind"`).
#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum WireConsistency {
    /// Read latest committed state (query only).
    Latest,
    /// Read pinned to an exact committed revision (query only).
    ReadAt {
        /// The committed revision to read.
        revision: ProjectRevision,
    },
    /// Read from a fixed immutable snapshot (query only).
    FixedSnapshot {
        /// The snapshot reference.
        snapshot: String,
    },
    /// Mutate only if live revision equals expectation (mutation only).
    ExpectedRevision {
        /// The expected live revision.
        revision: ProjectRevision,
    },
    /// No project-revision condition (runtime/async only).
    Unconditional,
}

/// Keys each shape of the `target` object may carry. Unknown `shape`
/// values are left for serde to reject — the scan only guards known
/// variants, since `deny_unknown_fields` is inert on internally-tagged
/// enums (serde#2123).
pub(crate) fn allowed_target_keys(shape: &str) -> Option<&'static [&'static str]> {
    Some(match shape {
        "resource" => &["shape", "kind", "id"],
        "resource-set" => &["shape", "kind", "ids"],
        "create" => &["shape", "parent_kind", "parent", "creates"],
        _ => return None,
    })
}

/// Keys each `kind` of the `consistency` object may carry.
fn allowed_consistency_keys(kind: &str) -> Option<&'static [&'static str]> {
    Some(match kind {
        "latest" | "unconditional" => &["kind"],
        "read-at" | "expected-revision" => &["kind", "revision"],
        "fixed-snapshot" => &["kind", "snapshot"],
        _ => return None,
    })
}

/// Rejects keys outside `allowed` inside the object at `path`.
pub(crate) fn deny_extra_keys(
    value: &Value,
    path: &str,
    allowed: &[&str],
) -> Result<(), GatewayError> {
    if let Some(obj) = value.as_object() {
        for key in obj.keys() {
            if !allowed.contains(&key.as_str()) {
                return Err(GatewayError::MalformedRequest {
                    reason: format!("unknown field {path}.{key}"),
                });
            }
        }
    }
    Ok(())
}

impl WireRequest {
    /// Parses a wire request from raw JSON, rejecting authority-shaped
    /// and implicit-targeting top-level keys before serde validation.
    ///
    /// Nested objects the wire schema owns (`capability`, `schema`,
    /// `target`, `consistency`) are also deny-by-default: unknown keys
    /// inside them are `MalformedRequest`, never silently dropped —
    /// serde's `deny_unknown_fields` cannot enforce that on internally
    /// tagged enums, so the explicit scan does.
    ///
    /// # Errors
    /// - [`GatewayError::SelfDeclaredAuthority`] — an authority-shaped
    ///   key is present.
    /// - [`GatewayError::ImplicitTargetRejected`] — an implicit-target
    ///   key is present.
    /// - [`GatewayError::MalformedRequest`] — anything else invalid.
    pub fn parse(value: &Value) -> Result<Self, GatewayError> {
        if let Some(obj) = value.as_object() {
            for field in FORBIDDEN_AUTHORITY_FIELDS {
                if obj.contains_key(*field) {
                    return Err(GatewayError::SelfDeclaredAuthority {
                        field: (*field).to_owned(),
                    });
                }
            }
            for field in FORBIDDEN_IMPLICIT_TARGET_FIELDS {
                if obj.contains_key(*field) {
                    return Err(GatewayError::ImplicitTargetRejected {
                        field: (*field).to_owned(),
                    });
                }
            }
            if let Some(capability) = obj.get("capability") {
                deny_extra_keys(capability, "capability", &["id", "version"])?;
                if let Some(version) = capability.get("version") {
                    deny_extra_keys(version, "capability.version", &["major", "minor"])?;
                }
            }
            if let Some(schema) = obj.get("schema") {
                deny_extra_keys(schema, "schema", &["name", "version"])?;
                if let Some(version) = schema.get("version") {
                    deny_extra_keys(version, "schema.version", &["major", "minor"])?;
                }
            }
            if let Some(target) = obj.get("target")
                && let Some(shape) = target.get("shape").and_then(Value::as_str)
                && let Some(allowed) = allowed_target_keys(shape)
            {
                deny_extra_keys(target, "target", allowed)?;
            }
            if let Some(consistency) = obj.get("consistency")
                && let Some(kind) = consistency.get("kind").and_then(Value::as_str)
                && let Some(allowed) = allowed_consistency_keys(kind)
            {
                deny_extra_keys(consistency, "consistency", allowed)?;
            }
        }
        serde_json::from_value(value.clone()).map_err(|e| GatewayError::MalformedRequest {
            reason: e.to_string(),
        })
    }
}
