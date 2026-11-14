//! Versioned, digest-pinned registry snapshot.
//!
//! [`RegistrySnapshot`] freezes the Gateway *discovery* surface —
//! `CapabilityRegistry::list`/`describe`/`versions` plus the
//! `SchemaRegistry::resolve` results for every schema a listed capability
//! declares — into plain serializable data. It is the only input the model
//! builder accepts, so unit tests run against serialized fixtures instead
//! of live registries, and the snapshot digest anchors every downstream
//! artifact (model → bindings → stub).
//!
//! The snapshot is **read-only provenance**: it carries descriptors
//! verbatim (including effect/permission/data-access metadata) so the
//! model can make typed visibility decisions, but nothing in this crate
//! treats any of it as authority.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use synvelis_gateway::{
    CapabilityDescriptor, CapabilityId, CapabilityRegistry, CapabilityVersion, SchemaRef,
    SchemaRegistry, SchemaVersion,
};

use crate::canon::digest_of;
use crate::error::ScriptApiError;
use crate::schema::ScriptSchema;

/// Snapshot document format version.
pub const SNAPSHOT_SCHEMA_VERSION: u32 = 1;

/// One resolved schema: the declared `name@version` ref plus its
/// structural [`ScriptSchema`] mirror.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaEntry {
    /// Schema name.
    pub name: String,
    /// Exact schema version.
    pub version: SchemaVersion,
    /// Structural schema mirror.
    pub schema: ScriptSchema,
}

impl SchemaEntry {
    /// The `name@version` reference this entry resolves.
    #[must_use]
    pub fn schema_ref(&self) -> SchemaRef {
        SchemaRef {
            name: self.name.clone(),
            version: self.version,
        }
    }
}

/// A versioned capture of the Gateway discovery surface.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistrySnapshot {
    /// Document format version — must equal [`SNAPSHOT_SCHEMA_VERSION`].
    pub schema_version: u32,
    /// Every registered capability descriptor, sorted by `(id, version)`.
    pub capabilities: Vec<CapabilityDescriptor>,
    /// Every schema referenced by a listed capability, sorted by
    /// `(name, version)`.
    pub schemas: Vec<SchemaEntry>,
}

impl RegistrySnapshot {
    /// Captures the discovery surface of live registries.
    ///
    /// Enumerates `capabilities.list()`, clones each `describe`d
    /// descriptor, and resolves every declared input/output
    /// [`SchemaRef`] through `schemas.resolve`. Resolution failure is
    /// fail-closed — a registry whose descriptors name unresolvable
    /// schemas cannot produce a snapshot.
    ///
    /// # Errors
    /// [`ScriptApiError::Gateway`] — registry lookup failure;
    /// [`ScriptApiError::Json`] — canonicalization failure.
    pub fn capture(
        capabilities: &CapabilityRegistry,
        schemas: &SchemaRegistry,
    ) -> Result<Self, ScriptApiError> {
        let mut descriptors = Vec::new();
        for cap_ref in capabilities.list() {
            let descriptor = capabilities.describe(&cap_ref)?;
            descriptors.push(descriptor.clone());
        }
        descriptors.sort_by(|a, b| (a.id.as_str(), a.version).cmp(&(b.id.as_str(), b.version)));

        // Collect every declared schema ref, deduplicated, in sorted order.
        let mut wanted: BTreeMap<(String, SchemaVersion), SchemaRef> = BTreeMap::new();
        for descriptor in &descriptors {
            for schema_ref in [&descriptor.input_schema, &descriptor.output_schema] {
                wanted
                    .entry((schema_ref.name.clone(), schema_ref.version))
                    .or_insert_with(|| schema_ref.clone());
            }
        }

        let mut entries = Vec::with_capacity(wanted.len());
        for ((name, version), schema_ref) in wanted {
            let value_schema = schemas.resolve(&schema_ref)?;
            entries.push(SchemaEntry {
                name,
                version,
                schema: ScriptSchema::from_value_schema(value_schema),
            });
        }

        let snapshot = Self {
            schema_version: SNAPSHOT_SCHEMA_VERSION,
            capabilities: descriptors,
            schemas: entries,
        };
        snapshot.validate()?;
        Ok(snapshot)
    }

    /// Structural validation: supported `schema_version`, no duplicate
    /// `(id, version)` capabilities, no duplicate `(name, version)`
    /// schemas. Cross-reference resolution is *not* checked here — a
    /// dangling schema ref becomes a per-entry
    /// [`crate::model::ScriptVerdict::UnresolvedSchema`] at model build,
    /// which is more informative than failing the whole snapshot.
    ///
    /// # Errors
    /// [`ScriptApiError::Snapshot`] — unsupported version;
    /// [`ScriptApiError::DuplicateCapability`] /
    /// [`ScriptApiError::DuplicateSchema`] — duplicates.
    pub fn validate(&self) -> Result<(), ScriptApiError> {
        if self.schema_version != SNAPSHOT_SCHEMA_VERSION {
            return Err(ScriptApiError::Snapshot {
                reason: format!(
                    "unsupported schema_version {} (expected {SNAPSHOT_SCHEMA_VERSION})",
                    self.schema_version
                ),
            });
        }
        let mut seen_caps = BTreeSet::new();
        for descriptor in &self.capabilities {
            let key = (descriptor.id.as_str(), descriptor.version);
            if !seen_caps.insert(key) {
                return Err(ScriptApiError::DuplicateCapability {
                    id: descriptor.id.as_str().to_owned(),
                    version: descriptor.version,
                });
            }
        }
        let mut seen_schemas = BTreeSet::new();
        for entry in &self.schemas {
            let key = (entry.name.as_str(), entry.version);
            if !seen_schemas.insert(key) {
                return Err(ScriptApiError::DuplicateSchema {
                    name: entry.name.clone(),
                    version: entry.version,
                });
            }
        }
        Ok(())
    }

    /// SHA-256 digest of the canonical snapshot — the provenance anchor
    /// carried by every downstream artifact.
    ///
    /// # Errors
    /// [`ScriptApiError::Json`] — canonicalization failure.
    pub fn digest(&self) -> Result<String, ScriptApiError> {
        digest_of(self)
    }

    /// Exact `(id, version)` lookup — never "latest".
    #[must_use]
    pub fn describe(
        &self,
        id: &CapabilityId,
        version: CapabilityVersion,
    ) -> Option<&CapabilityDescriptor> {
        self.capabilities
            .iter()
            .find(|d| d.id == *id && d.version == version)
    }

    /// Whether `id` is registered at all (any version).
    #[must_use]
    pub fn has_id(&self, id: &CapabilityId) -> bool {
        self.capabilities.iter().any(|d| d.id == *id)
    }

    /// All versions registered for `id`, ascending.
    #[must_use]
    pub fn versions_for(&self, id: &CapabilityId) -> Vec<CapabilityVersion> {
        self.capabilities
            .iter()
            .filter(|d| d.id == *id)
            .map(|d| d.version)
            .collect()
    }

    /// Resolves a [`SchemaRef`] to its [`ScriptSchema`] — exact
    /// `(name, version)` match only.
    #[must_use]
    pub fn resolve_schema(&self, schema_ref: &SchemaRef) -> Option<&ScriptSchema> {
        self.schemas
            .iter()
            .find(|e| e.name == schema_ref.name && e.version == schema_ref.version)
            .map(|e| &e.schema)
    }
}
