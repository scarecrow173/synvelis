//! Capability descriptor contract (GTC-002).
//!
//! A [`CapabilityDescriptor`] is the registry-visible contract of one
//! invocable capability: its versioned identity, operation domain,
//! input/output schema, target contract, effect *upper bound*, required
//! permissions, execution contract, and availability. It is deliberately
//! **not** identified with an MCP tool name or a UI label — adapters map
//! those onto capability refs.

use std::collections::BTreeSet;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::request::ResourceKind;
use crate::schema::SchemaRef;

/// Stable capability identifier — a dotted, lowercase name such as
/// `synvelis.midi.add-note`. Independent of tool names and UI labels.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CapabilityId(String);

impl CapabilityId {
    /// Maximum accepted length.
    pub const MAX_LEN: usize = 128;

    /// Wraps a raw string, rejecting malformed identifiers.
    ///
    /// # Errors
    /// [`InvalidCapabilityId`][crate::GatewayError::MalformedRequest] when
    /// `text` is empty, too long, or contains characters outside
    /// `[a-z0-9._-]`.
    pub fn new(text: impl Into<String>) -> Result<Self, crate::GatewayError> {
        let text = text.into();
        let valid = !text.is_empty()
            && text.len() <= Self::MAX_LEN
            && text.chars().all(|c| {
                c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-')
            });
        if !valid {
            return Err(crate::GatewayError::MalformedRequest {
                reason: "capability id must be 1..=128 chars of [a-z0-9._-]".to_owned(),
            });
        }
        Ok(Self(text))
    }

    /// The raw identifier string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for CapabilityId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for CapabilityId {
    type Error = crate::GatewayError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::new(text)
    }
}

impl From<CapabilityId> for String {
    fn from(id: CapabilityId) -> Self {
        id.0
    }
}

/// Semantic capability version. Compatibility between a requested and a
/// declared version is defined on the *schema* level
/// ([`crate::SchemaRef::is_compatible_with`]); capability versions are
/// resolved exactly — there is no "latest" guessing.
#[derive(
    Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(deny_unknown_fields)]
pub struct CapabilityVersion {
    /// Breaking-change epoch.
    pub major: u32,
    /// Backward-compatible revision within `major`.
    pub minor: u32,
}

impl CapabilityVersion {
    /// Creates a version.
    #[must_use]
    pub const fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }
}

impl fmt::Display for CapabilityVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// A versioned reference to a capability — what the wire names.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityRef {
    /// Stable capability id.
    pub id: CapabilityId,
    /// Exact requested version.
    pub version: CapabilityVersion,
}

impl fmt::Display for CapabilityRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}@{}", self.id, self.version)
    }
}

/// The operation domain a capability belongs to (GTC-001/002).
///
/// Domains are disjoint: a `ProjectMutation` is never silently routed
/// through the query path, and not every operation flows to the project
/// writer.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationDomain {
    /// Read-only context query / projection.
    Query,
    /// Semantic project mutation (goes to the GW-06 commit boundary).
    ProjectMutation,
    /// Real-time adjacent control such as transport (owner: issue #20).
    RuntimeControl,
    /// Non-RT asynchronous job (owner: issue #21 job coordinator).
    AsyncOperation,
}

impl OperationDomain {
    /// The kebab-case domain name used in external messages.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::ProjectMutation => "project-mutation",
            Self::RuntimeControl => "runtime-control",
            Self::AsyncOperation => "async-operation",
        }
    }
}

/// Target shape a capability requires on the wire.
///
/// GTC-002: explicit typed stable ids or explicit parent+kind — never
/// implicit resolution from UI selection/focus/active window.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TargetContract {
    /// No project resource target (e.g. transport, project-wide job).
    None,
    /// Exactly one existing resource of `kind`.
    Resource {
        /// The only accepted resource kind.
        kind: ResourceKind,
    },
    /// A non-empty set of existing resources, all of `kind`.
    ResourceSet {
        /// The only accepted resource kind.
        kind: ResourceKind,
    },
    /// Create a new resource under an explicit parent of `parent` kind.
    ///
    /// A create never names a nonexistent target id — it names the
    /// explicit parent, the created kind, and carries placement in args.
    Create {
        /// Required kind of the parent reference.
        parent: ResourceKind,
        /// Kind of resource the operation creates.
        creates: ResourceKind,
    },
}

/// Kind of side effect an operation may have (GTC-003 `PreparedEffects`).
///
/// The descriptor's [`EffectSet`] is the *upper bound* — the actual set
/// for a concrete operation is derived by GW-04, never taken from an
/// agent's self-declared effects.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EffectKind {
    /// Reads project data.
    Read,
    /// Writes existing project state.
    Write,
    /// Creates new project state.
    Create,
    /// Deletes project state.
    Delete,
    /// Discloses data to an external principal.
    Disclose,
    /// Performs an external side effect (network, filesystem, …).
    ExternalAction,
}

/// Set of [`EffectKind`] — the declared effect upper bound.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EffectSet(BTreeSet<EffectKind>);

impl EffectSet {
    /// Empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a set from kinds.
    #[must_use]
    pub fn of(kinds: impl IntoIterator<Item = EffectKind>) -> Self {
        Self(kinds.into_iter().collect())
    }

    /// Whether `kind` is inside the bound.
    #[must_use]
    pub fn contains(&self, kind: EffectKind) -> bool {
        self.0.contains(&kind)
    }

    /// Iterate the bound.
    pub fn iter(&self) -> impl Iterator<Item = EffectKind> + '_ {
        self.0.iter().copied()
    }
}

impl FromIterator<EffectKind> for EffectSet {
    fn from_iter<I: IntoIterator<Item = EffectKind>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// Coarse permission atom the capability requires the caller to hold.
///
/// GW-01 types the requirement only — grant semantics, scopes, and
/// evaluation belong to GW-02 (GTC-003).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionKind {
    /// Read project-derived context.
    ReadContext,
    /// Disclose context to an external principal.
    DiscloseContext,
    /// Mutate semantic project state.
    MutateProject,
    /// Drive transport / runtime control.
    ControlTransport,
    /// Submit or steer non-RT jobs.
    RunJob,
    /// Perform external side effects.
    ExternalAction,
}

/// Set of [`PermissionKind`] required by a capability.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PermissionSet(BTreeSet<PermissionKind>);

impl PermissionSet {
    /// Empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a set from kinds.
    #[must_use]
    pub fn of(kinds: impl IntoIterator<Item = PermissionKind>) -> Self {
        Self(kinds.into_iter().collect())
    }

    /// Whether every member of `self` is present in `other`.
    #[must_use]
    pub fn is_subset_of(&self, other: &Self) -> bool {
        self.0.is_subset(&other.0)
    }

    /// Iterate the set.
    pub fn iter(&self) -> impl Iterator<Item = PermissionKind> + '_ {
        self.0.iter().copied()
    }
}

impl FromIterator<PermissionKind> for PermissionSet {
    fn from_iter<I: IntoIterator<Item = PermissionKind>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// Data-access class a capability may touch (AHG-007, GW-02).
///
/// [`PermissionKind`] is the coarse capability-level verb; this is the
/// finer data-dimension split a grant restricts separately — metadata,
/// media bytes, and derived data are different read classes, and
/// disclosing data to an external principal is different again from
/// transferring it to an external provider.
///
/// Like [`CapabilityDescriptor::effect_bound`], the descriptor's
/// [`data_access`][CapabilityDescriptor::data_access] is an *upper
/// bound* declared at registration by Synvelis — not an agent's
/// self-declared access.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DataAccessClass {
    /// Reads project metadata (structure, names, placements).
    MetadataRead,
    /// Reads media payload bytes (PCM, waveform, encoded content).
    MediaBytesRead,
    /// Reads structured content payloads (MIDI note data, clip-local
    /// notes) — the typed-content analogue of [`Self::MediaBytesRead`]
    /// for non-media sources (GW-03).
    StructuredContentRead,
    /// Reads derived data (analysis features, computed fingerprints).
    DerivedDataRead,
    /// Reads workspace selection/focus state through an explicitly
    /// named view (GW-03 — the view is a permitted context window, not
    /// an implicit UI state feed).
    WorkspaceSelectionRead,
    /// Discloses data to an external principal.
    ExternalDisclosure,
    /// Transfers data to an external provider (model, service).
    ProviderTransfer,
}

/// Set of [`DataAccessClass`] a capability may require or a grant may
/// permit.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DataAccessSet(BTreeSet<DataAccessClass>);

impl DataAccessSet {
    /// Empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a set from classes.
    #[must_use]
    pub fn of(classes: impl IntoIterator<Item = DataAccessClass>) -> Self {
        Self(classes.into_iter().collect())
    }

    /// Whether `class` is in the set.
    #[must_use]
    pub fn contains(&self, class: DataAccessClass) -> bool {
        self.0.contains(&class)
    }

    /// Whether every member of `self` is present in `other`.
    #[must_use]
    pub fn is_subset_of(&self, other: &Self) -> bool {
        self.0.is_subset(&other.0)
    }

    /// Iterate the set.
    pub fn iter(&self) -> impl Iterator<Item = DataAccessClass> + '_ {
        self.0.iter().copied()
    }
}

impl FromIterator<DataAccessClass> for DataAccessSet {
    fn from_iter<I: IntoIterator<Item = DataAccessClass>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// How an invocation completes.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionContract {
    /// Handler runs to completion within the invocation.
    Sync,
    /// Handler accepts and returns an operation ref; completion is
    /// reported separately through the job owner (GTC-007).
    AsyncAccept,
}

/// Registration-visible availability.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Availability {
    /// Invocable now.
    Available,
    /// Registered but administratively disabled.
    Disabled {
        /// Human-readable reason (safe to disclose).
        reason: String,
    },
    /// Declared for a later milestone — never invocable by guessing.
    Planned,
    /// Removed from service; kept only for schema/audit continuity.
    Retired,
}

impl Availability {
    /// The kebab-case availability name used in external messages.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Available => "available",
            Self::Disabled { .. } => "disabled",
            Self::Planned => "planned",
            Self::Retired => "retired",
        }
    }
}

/// The registered contract of one capability version (GTC-002).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDescriptor {
    /// Stable capability id.
    pub id: CapabilityId,
    /// This registration's version.
    pub version: CapabilityVersion,
    /// Operation domain — decides consistency rules and the
    /// authorization stage evaluated at invoke.
    pub domain: OperationDomain,
    /// Declared input schema (name + current version).
    pub input_schema: SchemaRef,
    /// Declared output schema.
    pub output_schema: SchemaRef,
    /// Required target shape.
    pub target: TargetContract,
    /// Upper bound of possible effects.
    pub effect_bound: EffectSet,
    /// Permissions the caller must hold — existence of this descriptor
    /// in the registry never grants them.
    pub required_permissions: PermissionSet,
    /// Upper bound of data-access classes the capability may touch
    /// (GW-02). Registration-declared, like [`Self::effect_bound`] — a
    /// grant must cover every class listed here.
    #[serde(default)]
    pub data_access: DataAccessSet,
    /// Sync / async-accept execution contract.
    pub execution: ExecutionContract,
    /// Current availability.
    pub availability: Availability,
}

impl CapabilityDescriptor {
    /// The versioned identity of this descriptor.
    #[must_use]
    pub fn capability_ref(&self) -> CapabilityRef {
        CapabilityRef {
            id: self.id.clone(),
            version: self.version,
        }
    }
}
