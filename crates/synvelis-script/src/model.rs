//! [`ScriptApiModel`] — the deterministic projection of a registry
//! snapshot + surface spec into the script-facing API model.
//!
//! The model is a **read-only view**: it resolves each surface entry's
//! exact capability ref against the snapshot, maps availability into a
//! typed [`ScriptVerdict`], binds resolved input/output
//! [`ScriptSchema`]s, and records digest provenance. It never carries
//! `Handler`, grants, approvals, principals, caller identity, or any
//! executable authority — those types are not representable here.
//!
//! Verdict semantics (fail-closed, discovery honesty preserved):
//!
//! | Condition | Verdict |
//! |---|---|
//! | name unusable as Luau ident | `InvalidName` |
//! | id not registered | `UnknownCapability` |
//! | id registered, version not | `UnsupportedVersion { supported }` |
//! | `Availability::Disabled` | `Disabled { reason }` |
//! | `Availability::Planned` | `Planned` |
//! | `Availability::Retired` | `Retired` |
//! | declared schema ref unresolvable | `UnresolvedSchema` |
//! | exact ref + `Available` + schemas resolved | `Invocable` |
//!
//! `visibility == Hidden` is tracked separately from the verdict: a
//! hidden entry may be `Invocable` by capability terms yet is still
//! never projected to stubs/bindings/listings.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use synvelis_gateway::{
    Availability, CapabilityDescriptor, CapabilityRef, CapabilityVersion, SchemaRef,
};

use crate::canon::digest_of;
use crate::error::ScriptApiError;
use crate::schema::ScriptSchema;
use crate::snapshot::RegistrySnapshot;
use crate::surface::{
    NameRejection, ScriptSurfaceEntry, ScriptSurfaceSpec, ScriptVisibility, luau_name_status,
};

/// Model document format version.
pub const MODEL_SCHEMA_VERSION: u32 = 1;

/// Per-entry typed verdict — the fail-closed classification of whether a
/// surface entry may produce invocable types.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ScriptVerdict {
    /// Exact ref registered, `Available`, schemas resolved.
    Invocable,
    /// Registered but administratively disabled.
    Disabled {
        /// Safe-to-disclose reason.
        reason: String,
    },
    /// Declared for a later milestone.
    Planned,
    /// Removed from service.
    Retired,
    /// Id registered but the requested version is not.
    UnsupportedVersion {
        /// Versions actually registered for the id.
        supported: Vec<CapabilityVersion>,
    },
    /// Id not registered at all.
    UnknownCapability,
    /// A declared schema ref did not resolve in the snapshot.
    UnresolvedSchema {
        /// The `name@major.minor` reference that failed.
        schema: String,
    },
    /// The script name is not a usable Luau identifier.
    InvalidName {
        /// Why the name was rejected.
        reason: NameRejection,
    },
}

/// Digest provenance tying a model entry back to its sources.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DescriptorProvenance {
    /// SHA-256 of the canonical [`RegistrySnapshot`] this model was
    /// built from.
    pub snapshot_digest: String,
    /// SHA-256 of the canonical [`CapabilityDescriptor`] — the exact
    /// registration the entry resolved to.
    pub capability_digest: String,
}

/// A resolved schema binding: the declared `SchemaRef` plus the
/// structural schema the snapshot carried for it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SchemaBinding {
    /// The capability-declared `name@version` reference.
    pub schema: SchemaRef,
    /// The resolved structural schema.
    pub resolved: ScriptSchema,
}

/// One script-facing API entry with its typed verdict and provenance.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptApiEntry {
    /// Script-visible API name (the surface spec's key).
    pub name: String,
    /// Public-surface visibility.
    pub visibility: ScriptVisibility,
    /// Optional human-readable description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Typed invocability verdict.
    pub verdict: ScriptVerdict,
    /// The exact capability ref requested by the surface spec.
    pub requested: CapabilityRef,
    /// The registered descriptor verbatim — present iff the exact
    /// `(id, version)` resolved in the snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub descriptor: Option<CapabilityDescriptor>,
    /// Resolved input schema binding — present when the declared ref
    /// resolved in the snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<SchemaBinding>,
    /// Resolved output schema binding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<SchemaBinding>,
    /// Digest provenance — present when a descriptor resolved.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provenance: Option<DescriptorProvenance>,
}

impl ScriptApiEntry {
    /// Whether this entry may produce invocable types *and* is on the
    /// public surface — the single predicate stubs, bindings, and
    /// listings all share.
    #[must_use]
    pub fn is_invocable(&self) -> bool {
        self.visibility == ScriptVisibility::Public && self.verdict == ScriptVerdict::Invocable
    }
}

/// The deterministic script API model: snapshot + surface → typed,
/// digest-pinned projection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScriptApiModel {
    /// Model document format version.
    pub schema_version: u32,
    /// Digest of the [`RegistrySnapshot`] input.
    pub snapshot_digest: String,
    /// Digest of the [`ScriptSurfaceSpec`] input (the naming policy).
    pub surface_digest: String,
    /// Digest of this model's canonical payload — the anchor every
    /// generated artifact carries and drift checks compare against.
    pub model_digest: String,
    /// Entries sorted by name.
    pub entries: Vec<ScriptApiEntry>,
}

/// The hashed payload of a model — everything except `model_digest`
/// itself, so the digest is well-defined.
#[derive(Serialize)]
struct ModelPayload<'a> {
    schema_version: u32,
    snapshot_digest: &'a str,
    surface_digest: &'a str,
    entries: &'a [ScriptApiEntry],
}

impl ScriptApiModel {
    /// Builds the model from a validated snapshot and surface spec.
    ///
    /// Fails closed on: unsupported document versions, duplicate script
    /// names (fatal — the public name space must be unambiguous), and
    /// canonicalization failures. Per-entry problems become typed
    /// [`ScriptVerdict`]s, never silent skips.
    ///
    /// # Errors
    /// [`ScriptApiError::Snapshot`] — bad document version;
    /// [`ScriptApiError::DuplicateApiName`] — name collision;
    /// [`ScriptApiError::DuplicateCapability`] /
    /// [`ScriptApiError::DuplicateSchema`] — malformed snapshot;
    /// [`ScriptApiError::Json`] — canonicalization failure.
    pub fn build(
        snapshot: &RegistrySnapshot,
        surface: &ScriptSurfaceSpec,
    ) -> Result<Self, ScriptApiError> {
        snapshot.validate()?;
        surface.validate()?;

        // Duplicate names are fatal: two entries sharing a script name
        // would make binding/stub lookup ambiguous.
        let mut seen = BTreeSet::new();
        for entry in &surface.entries {
            if !seen.insert(entry.name.as_str()) {
                return Err(ScriptApiError::DuplicateApiName {
                    name: entry.name.clone(),
                });
            }
        }

        let snapshot_digest = snapshot.digest()?;
        let surface_digest = digest_of(surface)?;

        let mut entries = Vec::with_capacity(surface.entries.len());
        for surface_entry in &surface.entries {
            entries.push(Self::build_entry(
                surface_entry,
                snapshot,
                &snapshot_digest,
            )?);
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));

        let payload = ModelPayload {
            schema_version: MODEL_SCHEMA_VERSION,
            snapshot_digest: &snapshot_digest,
            surface_digest: &surface_digest,
            entries: &entries,
        };
        let model_digest = digest_of(&payload)?;

        Ok(Self {
            schema_version: MODEL_SCHEMA_VERSION,
            snapshot_digest,
            surface_digest,
            model_digest,
            entries,
        })
    }

    fn build_entry(
        surface_entry: &ScriptSurfaceEntry,
        snapshot: &RegistrySnapshot,
        snapshot_digest: &str,
    ) -> Result<ScriptApiEntry, ScriptApiError> {
        let requested = surface_entry.capability.clone();

        // 1. Name usability — recorded but never invocable.
        if let Err(reason) = luau_name_status(&surface_entry.name) {
            return Ok(Self::entry_shell(
                surface_entry,
                requested,
                ScriptVerdict::InvalidName { reason },
            ));
        }

        // 2. Exact capability resolution — never "latest".
        let Some(descriptor) = snapshot.describe(&requested.id, requested.version) else {
            let verdict = if snapshot.has_id(&requested.id) {
                ScriptVerdict::UnsupportedVersion {
                    supported: snapshot.versions_for(&requested.id),
                }
            } else {
                ScriptVerdict::UnknownCapability
            };
            return Ok(Self::entry_shell(surface_entry, requested, verdict));
        };

        // 3. Resolve declared schemas (independent of availability — the
        //    model records what *would* bind so display stays honest).
        let input = Self::bind_schema(snapshot, &descriptor.input_schema);
        let output = Self::bind_schema(snapshot, &descriptor.output_schema);

        // 4. Availability → verdict. `Available` still requires both
        //    schemas resolved to be invocable.
        let verdict = match &descriptor.availability {
            Availability::Available => {
                if input.is_none() {
                    ScriptVerdict::UnresolvedSchema {
                        schema: Self::schema_label(&descriptor.input_schema),
                    }
                } else if output.is_none() {
                    ScriptVerdict::UnresolvedSchema {
                        schema: Self::schema_label(&descriptor.output_schema),
                    }
                } else {
                    ScriptVerdict::Invocable
                }
            }
            Availability::Disabled { reason } => ScriptVerdict::Disabled {
                reason: reason.clone(),
            },
            Availability::Planned => ScriptVerdict::Planned,
            Availability::Retired => ScriptVerdict::Retired,
        };

        let provenance = DescriptorProvenance {
            snapshot_digest: snapshot_digest.to_owned(),
            capability_digest: digest_of(descriptor)?,
        };

        Ok(ScriptApiEntry {
            name: surface_entry.name.clone(),
            visibility: surface_entry.visibility,
            description: surface_entry.description.clone(),
            verdict,
            requested,
            descriptor: Some(descriptor.clone()),
            input,
            output,
            provenance: Some(provenance),
        })
    }

    fn entry_shell(
        surface_entry: &ScriptSurfaceEntry,
        requested: CapabilityRef,
        verdict: ScriptVerdict,
    ) -> ScriptApiEntry {
        ScriptApiEntry {
            name: surface_entry.name.clone(),
            visibility: surface_entry.visibility,
            description: surface_entry.description.clone(),
            verdict,
            requested,
            descriptor: None,
            input: None,
            output: None,
            provenance: None,
        }
    }

    fn bind_schema(snapshot: &RegistrySnapshot, schema_ref: &SchemaRef) -> Option<SchemaBinding> {
        snapshot
            .resolve_schema(schema_ref)
            .map(|resolved| SchemaBinding {
                schema: schema_ref.clone(),
                resolved: resolved.clone(),
            })
    }

    fn schema_label(schema_ref: &SchemaRef) -> String {
        format!(
            "{}@{}.{}",
            schema_ref.name, schema_ref.version.major, schema_ref.version.minor
        )
    }

    /// All entries, including hidden — for audit/introspection.
    #[must_use]
    pub fn entries(&self) -> &[ScriptApiEntry] {
        &self.entries
    }

    /// Looks up an entry by script name (hidden included — callers
    /// decide what to disclose).
    #[must_use]
    pub fn describe(&self, name: &str) -> Option<&ScriptApiEntry> {
        self.entries.iter().find(|e| e.name == name)
    }

    /// Public-surface listing: `Public` visibility entries only.
    /// Verdicts (including non-invocable ones) remain visible — display
    /// status is metadata, not permission.
    #[must_use]
    pub fn list_public(&self) -> Vec<&ScriptApiEntry> {
        self.entries
            .iter()
            .filter(|e| e.visibility == ScriptVisibility::Public)
            .collect()
    }

    /// Entries that may produce invocable bindings/stubs.
    #[must_use]
    pub fn invocable_entries(&self) -> Vec<&ScriptApiEntry> {
        self.entries.iter().filter(|e| e.is_invocable()).collect()
    }

    /// Whether `name` resolves to an invocable public entry.
    #[must_use]
    pub fn is_invocable(&self, name: &str) -> bool {
        self.describe(name)
            .is_some_and(ScriptApiEntry::is_invocable)
    }

    /// Verifies that a generated artifact was produced from *this*
    /// model — the digest-drift check.
    ///
    /// # Errors
    /// [`ScriptApiError::StaleArtifact`] — digest mismatch.
    pub fn verify_artifact(
        &self,
        artifact: &'static str,
        generated_from: &str,
    ) -> Result<(), ScriptApiError> {
        if generated_from == self.model_digest {
            Ok(())
        } else {
            Err(ScriptApiError::StaleArtifact {
                artifact,
                generated_from: generated_from.to_owned(),
                expected: self.model_digest.clone(),
            })
        }
    }
}
