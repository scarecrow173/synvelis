//! AUTX-01 (synvelis#260) — read-only Action Catalog projection.
//!
//! [`ActionCatalog`] joins product [`ActionPresentation`] metadata with the
//! *current* [`CapabilityDescriptor`]s of the existing Gateway
//! `CapabilityRegistry`. It is a display/search projection only:
//!
//! - **No second registry.** Entries are derived from the supplied
//!   descriptor set on every [`ActionCatalog::project`] call; the catalog
//!   owns no capability truth and exposes no registration API.
//! - **No copied authority.** Schema, target, effects, permissions, data
//!   access, availability, and version live on the descriptor verbatim —
//!   [`ActionEntry::descriptor`] *is* the registered object. The
//!   presentation carries display name keys, category, description keys,
//!   parameter UI hints, and a visibility hint only.
//! - **Discovery is not authority.** `list`/`describe` never mint a
//!   caller, grant, approval, or manual authority; every invocation still
//!   goes through `Gateway::bind`/`invoke` and its policy evaluation.
//! - **Displayability ≠ invocability.** [`ActionStatus::Available`] means
//!   the descriptor reports `Availability::Available` — nothing more.
//!   Executor presence is not observable through the registry's public
//!   API, so [`ExecutorState`] reports `Unknown` and `Gateway::invoke`
//!   remains the sole authority that can return `GatewayError::NoExecutor`.
//! - **UI intents stay UI intents.** `ViewAction`-style view-local
//!   actions are a different type domain (`automation_ui`); only
//!   `CapabilityRef`-keyed entries exist here, and an id the registry
//!   does not know projects as [`ActionStatus::Unknown`] — never
//!   executable.
//!
//! Machine-readable display values (schema, target, effect,
//! availability, version) are the descriptor's own fields — the catalog
//! cannot drift from registration because it stores no copies.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use synvelis_gateway::{
    Availability, CapabilityDescriptor, CapabilityRef, CapabilityVersion, Gateway, SchemaRegistry,
};

pub mod fixture;

/// Product metadata bound to one exact [`CapabilityRef`] — the
/// presentation half of an Action (AUTX-01).
///
/// Display name key, category, description key, parameter UI hints, and
/// an optional visibility hint **only**. The type deliberately has no
/// schema, target, effect, permission, data-access, availability, or
/// version fields: those are Gateway-owned descriptor facts, and a copy
/// here could silently drift from registration.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ActionPresentation {
    /// The exact versioned capability this presentation describes.
    ///
    /// 1:1 with the registered [`CapabilityRef`]: renaming
    /// `display_key`/`description_key` never changes which capability or
    /// invocation the action means.
    pub capability: CapabilityRef,
    /// Key resolving the display name in the product's string table.
    pub display_key: String,
    /// Product category for browse grouping (palette tab / CLI section).
    pub category: String,
    /// Key resolving the description in the product's string table.
    pub description_key: String,
    /// Optional per-argument UI hints — argument name → hint key/text.
    /// Hints describe input widgets only; they grant no authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameter_hints: Option<BTreeMap<String, String>>,
    /// Optional display hint — controls enumeration visibility only,
    /// never authorization.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility_hint: Option<VisibilityHint>,
}

/// Display-only visibility hint on an [`ActionPresentation`].
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VisibilityHint {
    /// Default — eligible for browse listings.
    Listed,
    /// Excluded from `list` unless [`ActionFilter::include_hidden`];
    /// still describable by exact [`CapabilityRef`].
    Hidden,
}

/// Lifecycle/registration verdict for one catalog entry.
///
/// Mirrors `docs/automation/design-capability-action.md` §3: each
/// non-`Available` state is a *distinct* machine-readable verdict, and
/// none of them is an execution grant.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum ActionStatus {
    /// The registered descriptor reports `Availability::Available`.
    /// Display-only — invoke still passes Gateway policy and executor
    /// checks.
    Available,
    /// `Availability::Disabled` — the reason is carried verbatim.
    Disabled {
        /// The descriptor's disabled reason (safe to disclose).
        reason: String,
    },
    /// `Availability::Planned` — discoverable, never invocable.
    Planned,
    /// `Availability::Retired`.
    Retired,
    /// The capability id is registered but this exact version is not.
    UnsupportedVersion {
        /// Versions the registry does know for this id.
        supported: Vec<CapabilityVersion>,
    },
    /// No registered descriptor exists for this id — the entry exists
    /// only because a presentation referenced it.
    Unknown,
}

/// Whether the descriptor's declared input/output schemas resolve in
/// the supplied [`SchemaRegistry`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "schema", rename_all = "kebab-case")]
pub enum SchemaConsistency {
    /// Every declared `SchemaRef` resolved exactly.
    Consistent,
    /// A declared `SchemaRef` did not resolve — an incoherent
    /// registration (e.g. inserted via `registry_mut()` past
    /// `Gateway::register_capability`'s coherence check).
    Mismatch {
        /// Which declaration failed and why.
        detail: String,
    },
    /// No schema store was supplied to the projection — consistency was
    /// not evaluated. Never guessed `Consistent`.
    Unknown,
}

/// Executor installation state.
///
/// `CapabilityRegistry`'s public API does not expose whether a
/// registration carries a handler (`handler`/`resolve` are
/// crate-private), so a read-only projection cannot distinguish
/// "registered without executor" (`GatewayError::NoExecutor` at invoke)
/// from "executor installed". Every projection reports [`Self::Unknown`]
/// until a read-only `has_executor` predicate lands on the registry —
/// `Gateway::invoke` remains the only authority either way.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutorState {
    /// Presence cannot be determined by this projection.
    Unknown,
}

/// One catalog row: a [`CapabilityRef`] joined with its registered
/// descriptor (when any) and its product presentation (when any).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ActionEntry {
    /// The exact versioned identity — stable across display-name changes.
    pub capability: CapabilityRef,
    /// The registered descriptor verbatim — the machine-readable source
    /// for schema/target/effect/permission/availability display values.
    /// `None` for orphan presentations and synthesized lookups.
    pub descriptor: Option<CapabilityDescriptor>,
    /// Product presentation metadata — `None` for registered
    /// capabilities that have no display metadata yet.
    pub presentation: Option<ActionPresentation>,
    /// Lifecycle/registration verdict.
    pub status: ActionStatus,
    /// Declared-schema coherence verdict.
    pub schema: SchemaConsistency,
    /// Executor presence — always [`ExecutorState::Unknown`] today.
    pub executor: ExecutorState,
}

/// Browse filter for [`ActionCatalog::list`].
#[derive(Clone, Copy, Debug)]
pub struct ActionFilter<'a> {
    /// Case-insensitive substring matched against capability id,
    /// version, domain name, display key, description key, and category.
    pub query: Option<&'a str>,
    /// Case-insensitive exact match on the presentation category.
    /// Entries without a presentation never match a category filter.
    pub category: Option<&'a str>,
    /// Include entries whose presentation visibility hint is
    /// [`VisibilityHint::Hidden`]. Default: excluded from browse
    /// listings (still describable).
    pub include_hidden: bool,
    /// Include registered capabilities that have no
    /// [`ActionPresentation`]. Default: included — the registry's full
    /// capability set stays enumerable; GUI consumers may opt out.
    pub include_unpresented: bool,
}

impl Default for ActionFilter<'_> {
    fn default() -> Self {
        Self {
            query: None,
            category: None,
            include_hidden: false,
            include_unpresented: true,
        }
    }
}

/// Fail-closed catalog construction errors.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "error", rename_all = "kebab-case")]
pub enum ActionCatalogError {
    /// The supplied descriptor set contains the same
    /// `(id, version)` twice — ambiguous registry truth is rejected,
    /// not guessed.
    DuplicateCapability {
        /// The duplicated capability ref.
        capability: CapabilityRef,
    },
    /// Two presentations claim the same exact [`CapabilityRef`].
    DuplicatePresentation {
        /// The duplicated capability ref.
        capability: CapabilityRef,
    },
}

/// A read-only projection of registered capabilities plus product
/// presentation metadata.
///
/// Build with [`ActionCatalog::project`] (explicit descriptor slice +
/// optional schema store) or [`ActionCatalog::project_gateway`] (snapshot
/// a live [`Gateway`]). The catalog is a snapshot: registry registrations
/// are append-only and immutable, so rebuilding on registration changes
/// is the refresh path.
#[derive(Clone, Debug)]
pub struct ActionCatalog {
    /// Sorted by [`CapabilityRef`] — deterministic listing order.
    entries: Vec<ActionEntry>,
}

impl ActionCatalog {
    /// Projects `capabilities` (the registry's registered descriptors)
    /// joined with `presentations`.
    ///
    /// `schemas` is the [`SchemaRegistry`] the descriptors were
    /// registered against; pass `None` when no schema store is
    /// reachable — every entry then reports
    /// [`SchemaConsistency::Unknown`] rather than guessing.
    ///
    /// # Errors
    /// [`ActionCatalogError::DuplicateCapability`] /
    /// [`ActionCatalogError::DuplicatePresentation`] — ambiguous input
    /// is rejected fail-closed.
    pub fn project(
        capabilities: &[CapabilityDescriptor],
        schemas: Option<&SchemaRegistry>,
        presentations: &[ActionPresentation],
    ) -> Result<Self, ActionCatalogError> {
        let mut registered: BTreeMap<CapabilityRef, CapabilityDescriptor> = BTreeMap::new();
        for descriptor in capabilities {
            let capability = descriptor.capability_ref();
            if registered
                .insert(capability.clone(), descriptor.clone())
                .is_some()
            {
                return Err(ActionCatalogError::DuplicateCapability { capability });
            }
        }

        let mut presented: BTreeMap<CapabilityRef, ActionPresentation> = BTreeMap::new();
        for presentation in presentations {
            if presented
                .insert(presentation.capability.clone(), presentation.clone())
                .is_some()
            {
                return Err(ActionCatalogError::DuplicatePresentation {
                    capability: presentation.capability.clone(),
                });
            }
        }

        let mut entries: BTreeMap<CapabilityRef, ActionEntry> = BTreeMap::new();
        for (capability, descriptor) in &registered {
            let status = match &descriptor.availability {
                Availability::Available => ActionStatus::Available,
                Availability::Disabled { reason } => ActionStatus::Disabled {
                    reason: reason.clone(),
                },
                Availability::Planned => ActionStatus::Planned,
                Availability::Retired => ActionStatus::Retired,
            };
            let schema = schemas.map_or(SchemaConsistency::Unknown, |store| {
                schema_consistency(store, descriptor)
            });
            entries.insert(
                capability.clone(),
                ActionEntry {
                    capability: capability.clone(),
                    descriptor: Some(descriptor.clone()),
                    presentation: presented.remove(capability),
                    status,
                    schema,
                    executor: ExecutorState::Unknown,
                },
            );
        }

        // Leftover presentations reference capabilities the registry
        // does not know at that exact version — surfaced, never hidden.
        for (capability, presentation) in presented {
            let supported: Vec<CapabilityVersion> = registered
                .values()
                .filter(|d| d.id == capability.id)
                .map(|d| d.version)
                .collect();
            let status = if supported.is_empty() {
                ActionStatus::Unknown
            } else {
                ActionStatus::UnsupportedVersion { supported }
            };
            entries.insert(
                capability.clone(),
                ActionEntry {
                    capability,
                    descriptor: None,
                    presentation: Some(presentation),
                    status,
                    schema: SchemaConsistency::Unknown,
                    executor: ExecutorState::Unknown,
                },
            );
        }

        Ok(Self {
            entries: entries.into_values().collect(),
        })
    }

    /// Snapshots the live [`Gateway`]'s registry and schema store into a
    /// catalog.
    ///
    /// Takes `&mut Gateway` only because `Gateway::schemas_mut()` is the
    /// sole schema accessor; the projection itself is read-only and
    /// grants nothing. Internally the registry is snapshotted before the
    /// schema borrow so no authority state is held across the boundary.
    ///
    /// # Errors
    /// Same as [`ActionCatalog::project`].
    pub fn project_gateway(
        gateway: &mut Gateway,
        presentations: &[ActionPresentation],
    ) -> Result<Self, ActionCatalogError> {
        let capabilities: Vec<CapabilityDescriptor> = {
            let registry = gateway.registry();
            registry
                .list()
                .into_iter()
                .filter_map(|capability| registry.describe(&capability).ok().cloned())
                .collect()
        };
        let schemas: &SchemaRegistry = gateway.schemas_mut();
        Self::project(&capabilities, Some(schemas), presentations)
    }

    /// All entries in deterministic [`CapabilityRef`] order.
    #[must_use]
    pub fn entries(&self) -> &[ActionEntry] {
        &self.entries
    }

    /// Browse listing under `filter` — a filtered view of
    /// [`entries`][Self::entries], never a different truth.
    #[must_use]
    pub fn list(&self, filter: &ActionFilter<'_>) -> Vec<&ActionEntry> {
        self.entries
            .iter()
            .filter(|entry| {
                if !filter.include_hidden
                    && matches!(
                        entry.presentation.as_ref().and_then(|p| p.visibility_hint),
                        Some(VisibilityHint::Hidden)
                    )
                {
                    return false;
                }
                if !filter.include_unpresented && entry.presentation.is_none() {
                    return false;
                }
                if let Some(category) = filter.category {
                    let matches = entry
                        .presentation
                        .as_ref()
                        .is_some_and(|p| p.category.eq_ignore_ascii_case(category));
                    if !matches {
                        return false;
                    }
                }
                if let Some(query) = filter.query {
                    let needle = query.to_lowercase();
                    if !search_text(entry).contains(&needle) {
                        return false;
                    }
                }
                true
            })
            .collect()
    }

    /// Exact lookup by versioned [`CapabilityRef`].
    ///
    /// Returns the stored entry when the catalog knows the ref (registered
    /// or presentation-referenced). When the id is registered but the
    /// requested version is not, synthesizes an
    /// [`ActionStatus::UnsupportedVersion`] verdict on the fly — the
    /// catalog reports the mismatch without inventing a descriptor.
    /// Returns `None` only when the catalog has no information at all.
    #[must_use]
    pub fn describe(&self, capability: &CapabilityRef) -> Option<ActionEntry> {
        if let Some(entry) = self.entries.iter().find(|e| e.capability == *capability) {
            return Some(entry.clone());
        }
        let supported: Vec<CapabilityVersion> = self
            .entries
            .iter()
            .filter(|e| e.capability.id == capability.id && e.descriptor.is_some())
            .map(|e| e.capability.version)
            .collect();
        if supported.is_empty() {
            return None;
        }
        Some(ActionEntry {
            capability: capability.clone(),
            descriptor: None,
            presentation: None,
            status: ActionStatus::UnsupportedVersion { supported },
            schema: SchemaConsistency::Unknown,
            executor: ExecutorState::Unknown,
        })
    }

    /// The registered descriptors — the exact slice a consumer such as
    /// `automation_ui::project_action_catalog` needs. Cloned because the
    /// catalog stores its own snapshot; authority stays on the registry.
    #[must_use]
    pub fn descriptors(&self) -> Vec<CapabilityDescriptor> {
        self.entries
            .iter()
            .filter_map(|e| e.descriptor.clone())
            .collect()
    }
}

/// Resolves both declared schemas; the first failure is reported.
fn schema_consistency(
    store: &SchemaRegistry,
    descriptor: &CapabilityDescriptor,
) -> SchemaConsistency {
    for schema_ref in [&descriptor.input_schema, &descriptor.output_schema] {
        if let Err(err) = store.resolve(schema_ref) {
            return SchemaConsistency::Mismatch {
                detail: format!("{schema_ref:?} unresolved: {err:?}"),
            };
        }
    }
    SchemaConsistency::Consistent
}

/// Lowercased haystack for [`ActionFilter::query`] matching.
fn search_text(entry: &ActionEntry) -> String {
    let mut text = format!(
        "{} {} {}",
        entry.capability.id,
        entry.capability.version,
        entry.descriptor.as_ref().map_or("", |d| d.domain.name()),
    );
    if let Some(presentation) = &entry.presentation {
        text.push(' ');
        text.push_str(&presentation.display_key);
        text.push(' ');
        text.push_str(&presentation.description_key);
        text.push(' ');
        text.push_str(&presentation.category);
    }
    text.to_lowercase()
}
