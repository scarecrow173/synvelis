//! Script-facing surface specification: script name → exact capability.
//!
//! The surface spec is the *naming policy* input to the model — it maps a
//! script-visible API name onto an exact [`synvelis_gateway::CapabilityRef`]
//! (id + version, never "latest"). It carries no schema, target, effect,
//! permission, or availability data of its own: those come from the
//! [`crate::snapshot::RegistrySnapshot`] so the capability registry stays
//! the single source of truth.
//!
//! Names must be usable Luau identifiers because the same name keys the
//! generated stub's `declare function` and the runtime binding lookup. A
//! name that is not a valid, non-reserved Luau identifier produces a
//! per-entry [`crate::model::ScriptVerdict::InvalidName`] — the entry is
//! still recorded (discovery honesty) but is never invocable.

use serde::{Deserialize, Serialize};

use synvelis_gateway::CapabilityRef;

/// Surface document format version.
pub const SURFACE_SCHEMA_VERSION: u32 = 1;

/// Whether a surface entry is projected onto the public script surface.
///
/// `Hidden` is a *model-level* visibility decision (the merged Gateway
/// `Availability` has no `Hidden` variant): hidden entries stay in the
/// model for audit but are excluded from stubs, bindings, listings, and
/// completion candidates.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScriptVisibility {
    /// Projected to the public script surface.
    #[default]
    Public,
    /// Never projected — discovery metadata only.
    Hidden,
}

/// One script-facing API name bound to an exact capability ref.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptSurfaceEntry {
    /// Script-visible API name — must be a non-reserved Luau identifier.
    pub name: String,
    /// Exact capability identity (id + version).
    pub capability: CapabilityRef,
    /// Public-surface visibility. Defaults to `public`.
    #[serde(default)]
    pub visibility: ScriptVisibility,
    /// Optional human-readable description (safe to disclose).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A versioned script surface: the naming policy over a snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptSurfaceSpec {
    /// Document format version — must equal [`SURFACE_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Surface entries. Order is preserved in the document; the model
    /// sorts by name for canonical output.
    pub entries: Vec<ScriptSurfaceEntry>,
}

impl ScriptSurfaceSpec {
    /// Validates the document envelope. Per-entry name/capability
    /// problems are *not* fatal here — they become typed per-entry
    /// verdicts in the model so one bad entry cannot hide the rest.
    ///
    /// # Errors
    /// [`crate::ScriptApiError::Snapshot`] — unsupported
    /// `schema_version`.
    pub fn validate(&self) -> Result<(), crate::ScriptApiError> {
        if self.schema_version != SURFACE_SCHEMA_VERSION {
            return Err(crate::ScriptApiError::Snapshot {
                reason: format!(
                    "unsupported surface schema_version {} (expected {SURFACE_SCHEMA_VERSION})",
                    self.schema_version
                ),
            });
        }
        Ok(())
    }
}

/// Why a script name cannot be used as a Luau-facing API identifier.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NameRejection {
    /// The name is empty.
    Empty,
    /// The name exceeds [`MAX_API_NAME_LEN`] chars.
    TooLong,
    /// The name is not `[A-Za-z_][A-Za-z0-9_]*`.
    InvalidIdentifier,
    /// The name collides with a Luau keyword.
    ReservedWord,
}

/// Maximum accepted script API name length.
pub const MAX_API_NAME_LEN: usize = 64;

/// Luau reserved words plus contextual keywords that must not be used as
/// API names (`type`, `export`, `continue`). Conservative on purpose:
/// rejecting a borderline name is cheap, a confusing stub is not.
const LUAU_RESERVED: &[&str] = &[
    "and", "break", "continue", "do", "else", "elseif", "end", "export", "false", "for",
    "function", "if", "in", "local", "nil", "not", "or", "repeat", "return", "then", "true",
    "type", "until", "while",
];

/// Checks whether `name` is a usable Luau identifier for an API entry.
///
/// # Errors
/// [`NameRejection`] — why the name is unusable.
pub fn luau_name_status(name: &str) -> Result<(), NameRejection> {
    if name.is_empty() {
        return Err(NameRejection::Empty);
    }
    if name.chars().count() > MAX_API_NAME_LEN {
        return Err(NameRejection::TooLong);
    }
    let mut chars = name.chars();
    let first_ok = chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_');
    if !first_ok || !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        return Err(NameRejection::InvalidIdentifier);
    }
    if LUAU_RESERVED.contains(&name) {
        return Err(NameRejection::ReservedWord);
    }
    Ok(())
}
