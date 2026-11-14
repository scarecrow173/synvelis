//! Serialized capability/schema registry snapshot — the compiler's
//! only view of the outside world.
//!
//! These types mirror `synvelis-gateway`'s contract types
//! (`CapabilityDescriptor`, `ValueSchema`, `SchemaRegistry`,
//! `CapabilityRegistry`) as plain serializable data. The live Gateway
//! Descriptor Read Adapter is an integration-slice concern; this crate
//! compiles against snapshot fixtures only and never depends on the
//! gateway crate.
//!
//! Resolution semantics mirror the gateway exactly:
//!
//! - Capabilities resolve by **exact** `(id, version)` — no "latest".
//! - Schemas resolve by **exact** `(name, version)` — the
//!   `is_compatible_with` minor-range rule is an invoke-time concern,
//!   not a compile-time one.
//! - Discovery grants nothing: a descriptor's presence here is
//!   metadata for validation, never authority.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::error::AutomationError;
use crate::refs::Version;
use crate::schema::{ResourceKind, SchemaNode};

/// The only `schema_version` accepted for snapshots.
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

/// Operation domain — mirrors gateway `OperationDomain`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OperationDomain {
    /// Read-only context query / projection.
    Query,
    /// Non-destructive mutation preparation.
    Proposal,
    /// Semantic project mutation.
    ProjectMutation,
    /// Real-time adjacent control.
    RuntimeControl,
    /// Non-RT asynchronous job.
    AsyncOperation,
}

impl OperationDomain {
    /// The kebab-case domain name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::Proposal => "proposal",
            Self::ProjectMutation => "project-mutation",
            Self::RuntimeControl => "runtime-control",
            Self::AsyncOperation => "async-operation",
        }
    }
}

/// Target shape — mirrors gateway `TargetContract`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "kebab-case", deny_unknown_fields)]
pub enum TargetContract {
    /// No project resource target.
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
    /// Create under an explicit parent; placement lives in args.
    Create {
        /// Required parent kind.
        parent: ResourceKind,
        /// Kind of resource created.
        creates: ResourceKind,
    },
}

/// Effect kind — mirrors gateway `EffectKind`.
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
    /// External side effect (network, filesystem, …).
    ExternalAction,
}

impl EffectKind {
    /// The kebab-case name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Read => "read",
            Self::Write => "write",
            Self::Create => "create",
            Self::Delete => "delete",
            Self::Disclose => "disclose",
            Self::ExternalAction => "external-action",
        }
    }
}

/// Effect upper bound — mirrors gateway `EffectSet` (transparent set).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EffectSet(BTreeSet<EffectKind>);

impl EffectSet {
    /// Empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds from kinds.
    #[must_use]
    pub fn of(kinds: impl IntoIterator<Item = EffectKind>) -> Self {
        Self(kinds.into_iter().collect())
    }

    /// Iterates the bound.
    pub fn iter(&self) -> impl Iterator<Item = EffectKind> + '_ {
        self.0.iter().copied()
    }

    /// Unions `other` into `self` (bounds only grow — conservative).
    pub(crate) fn union_with(&mut self, other: &Self) {
        self.0.extend(other.0.iter().copied());
    }
}

impl FromIterator<EffectKind> for EffectSet {
    fn from_iter<I: IntoIterator<Item = EffectKind>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// Permission atom — mirrors gateway `PermissionKind`. Carried for
/// snapshot fidelity; the compiler never treats it as authority.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionKind {
    /// Read project-derived context.
    ReadContext,
    /// Disclose context externally.
    DiscloseContext,
    /// Mutate semantic project state.
    MutateProject,
    /// Drive transport / runtime control.
    ControlTransport,
    /// Submit or steer non-RT jobs.
    RunJob,
    /// External side effects.
    ExternalAction,
}

/// Permission set — mirrors gateway `PermissionSet`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PermissionSet(BTreeSet<PermissionKind>);

impl PermissionSet {
    /// Empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds from kinds.
    #[must_use]
    pub fn of(kinds: impl IntoIterator<Item = PermissionKind>) -> Self {
        Self(kinds.into_iter().collect())
    }
}

impl FromIterator<PermissionKind> for PermissionSet {
    fn from_iter<I: IntoIterator<Item = PermissionKind>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// Data-access class — mirrors gateway `DataAccessClass`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DataAccessClass {
    /// Project metadata reads.
    MetadataRead,
    /// Media payload bytes.
    MediaBytesRead,
    /// Structured content payloads (MIDI notes, …).
    StructuredContentRead,
    /// Derived data.
    DerivedDataRead,
    /// Workspace selection/focus via named view.
    WorkspaceSelectionRead,
    /// Disclosure to external principal.
    ExternalDisclosure,
    /// Transfer to external provider.
    ProviderTransfer,
}

impl DataAccessClass {
    /// The kebab-case name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::MetadataRead => "metadata-read",
            Self::MediaBytesRead => "media-bytes-read",
            Self::StructuredContentRead => "structured-content-read",
            Self::DerivedDataRead => "derived-data-read",
            Self::WorkspaceSelectionRead => "workspace-selection-read",
            Self::ExternalDisclosure => "external-disclosure",
            Self::ProviderTransfer => "provider-transfer",
        }
    }
}

/// Data-access set — mirrors gateway `DataAccessSet`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DataAccessSet(BTreeSet<DataAccessClass>);

impl DataAccessSet {
    /// Empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds from classes.
    #[must_use]
    pub fn of(classes: impl IntoIterator<Item = DataAccessClass>) -> Self {
        Self(classes.into_iter().collect())
    }

    /// Iterates the set.
    pub fn iter(&self) -> impl Iterator<Item = DataAccessClass> + '_ {
        self.0.iter().copied()
    }

    /// Unions `other` into `self` (bounds only grow — conservative).
    pub(crate) fn union_with(&mut self, other: &Self) {
        self.0.extend(other.0.iter().copied());
    }
}

impl FromIterator<DataAccessClass> for DataAccessSet {
    fn from_iter<I: IntoIterator<Item = DataAccessClass>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

/// Execution contract — mirrors gateway `ExecutionContract`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionContract {
    /// Runs to completion within the invocation.
    Sync,
    /// Accepts and returns an operation ref; completion reported via
    /// the job owner.
    AsyncAccept,
}

impl ExecutionContract {
    /// The kebab-case name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Sync => "sync",
            Self::AsyncAccept => "async-accept",
        }
    }
}

/// Availability — mirrors gateway `Availability`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Availability {
    /// Invocable now.
    Available,
    /// Administratively disabled.
    Disabled {
        /// Human-readable reason.
        reason: String,
    },
    /// Declared for a later milestone — never invocable.
    Planned,
    /// Removed from service.
    Retired,
}

impl Availability {
    /// The kebab-case availability name.
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

/// One registered capability version — mirrors gateway
/// `CapabilityDescriptor` (minus the executor, which snapshots never
/// carry).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityDescriptor {
    /// Stable capability id (`[a-z0-9._-]`, ≤128).
    pub id: String,
    /// This registration's version.
    pub version: Version,
    /// Operation domain.
    pub domain: OperationDomain,
    /// Declared input schema ref (`name@major.minor`).
    pub input_schema: crate::refs::VersionedRef,
    /// Declared output schema ref.
    pub output_schema: crate::refs::VersionedRef,
    /// Required target shape.
    pub target: TargetContract,
    /// Effect upper bound.
    pub effect_bound: EffectSet,
    /// Required permissions (metadata only — grants nothing).
    #[serde(default)]
    pub required_permissions: PermissionSet,
    /// Data-access upper bound.
    #[serde(default)]
    pub data_access: DataAccessSet,
    /// Sync / async-accept contract.
    pub execution: ExecutionContract,
    /// Current availability.
    pub availability: Availability,
}

/// One registered schema version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaEntry {
    /// Registered schema name.
    pub name: String,
    /// This entry's version.
    pub version: Version,
    /// The structural schema (`SchemaNode` — the snapshot mirror of
    /// gateway `ValueSchema` plus the workflow-local `job` node).
    pub schema: SchemaNode,
}

/// A serialized registry snapshot — capabilities plus the schema
/// store they reference.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrySnapshot {
    /// Must equal [`SNAPSHOT_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Registered capability descriptors.
    pub capabilities: Vec<CapabilityDescriptor>,
    /// Registered schemas.
    pub schemas: Vec<SchemaEntry>,
}

impl RegistrySnapshot {
    /// Structural validation: version, unique `(id, version)`
    /// capability pairs, unique `(name, version)` schema entries, and
    /// capability-id shape.
    ///
    /// # Errors
    /// [`AutomationError::Registry`] on the first violation.
    pub fn validate(&self) -> Result<(), AutomationError> {
        if self.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(AutomationError::Registry {
                reason: format!(
                    "unsupported snapshot schema_version: {}",
                    self.schema_version
                ),
            });
        }
        let mut caps = BTreeSet::new();
        for cap in &self.capabilities {
            validate_capability_id(&cap.id)?;
            if !caps.insert((cap.id.clone(), cap.version)) {
                return Err(reg_err(format!(
                    "duplicate capability registration: {}@{}",
                    cap.id, cap.version
                )));
            }
        }
        let mut schemas = BTreeSet::new();
        for entry in &self.schemas {
            if entry.name.is_empty() {
                return Err(reg_err("schema entry with empty name".to_owned()));
            }
            if !schemas.insert((entry.name.clone(), entry.version)) {
                return Err(reg_err(format!(
                    "duplicate schema registration: {}@{}",
                    entry.name, entry.version
                )));
            }
        }
        Ok(())
    }

    /// Exact capability resolution — mirrors
    /// `CapabilityRegistry::resolve`: unknown id and unsupported
    /// version are distinct failures.
    ///
    /// # Errors
    /// [`crate::error::CompileError::UnknownCapability`] /
    /// [`crate::error::CompileError::UnsupportedCapabilityVersion`].
    pub fn resolve_capability(
        &self,
        cap: &crate::refs::VersionedRef,
    ) -> Result<&CapabilityDescriptor, crate::error::CompileError> {
        use crate::error::CompileError as E;
        if !self.capabilities.iter().any(|c| c.id == cap.id) {
            return Err(E::UnknownCapability { id: cap.id.clone() });
        }
        self.capabilities
            .iter()
            .find(|c| c.id == cap.id && c.version == cap.version)
            .ok_or_else(|| E::UnsupportedCapabilityVersion {
                id: cap.id.clone(),
                requested: cap.version.to_string(),
            })
    }

    /// Exact schema resolution — mirrors `SchemaRegistry::resolve`.
    ///
    /// # Errors
    /// [`crate::error::CompileError::UnknownSchema`] /
    /// [`crate::error::CompileError::UnsupportedSchemaVersion`].
    pub fn resolve_schema(
        &self,
        schema_ref: &crate::refs::VersionedRef,
    ) -> Result<&SchemaNode, crate::error::CompileError> {
        use crate::error::CompileError as E;
        if !self.schemas.iter().any(|s| s.name == schema_ref.id) {
            return Err(E::UnknownSchema {
                name: schema_ref.id.clone(),
            });
        }
        self.schemas
            .iter()
            .find(|s| s.name == schema_ref.id && s.version == schema_ref.version)
            .map(|s| &s.schema)
            .ok_or_else(|| E::UnsupportedSchemaVersion {
                name: schema_ref.id.clone(),
                requested: schema_ref.version.to_string(),
            })
    }
}

/// `[a-z0-9._-]`, 1..=128 — mirrors gateway `CapabilityId::new`.
fn validate_capability_id(id: &str) -> Result<(), AutomationError> {
    let valid = !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-'));
    if !valid {
        return Err(reg_err(format!(
            "invalid capability id '{id}' (want [a-z0-9._-], ≤128)"
        )));
    }
    Ok(())
}

fn reg_err(reason: String) -> AutomationError {
    AutomationError::Registry { reason }
}
