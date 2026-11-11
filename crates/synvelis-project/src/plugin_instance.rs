//! Persisted plugin instance identity (Issue #101, PC-C4-05, schema
//! v13).
//!
//! `Project::plugin_instances` is the single persisted authority for
//! the `StablePluginInstanceId` a semantic processor's plugin
//! instance carries at runtime. The store is keyed by [`ProcessorId`]
//! and pins the authored [`PluginRef`] the id was issued for, so a
//! save→reload or engine restart hydrates the *same* identity the
//! session was authored with — never a freshly minted runtime value.
//!
//! Invariants (enforced by [`PluginInstanceIdentities::validate_against`]
//! inside [`Project::validate_references`]):
//!
//! - `watermark` starts at 1 and only moves forward; every issued id
//!   satisfies `1 <= id < watermark` and is never reissued — removal
//!   frees the *entry*, never the id.
//! - Exactly one entry per `Resolved` processor; `Unresolved`
//!   processors carry none; every entry names a live processor whose
//!   `PluginRef` equals the entry's verbatim (descriptor, version and
//!   `capability_revision` — a capability bump is a different authored
//!   identity and mints a new id).
//!
//! This is a separate id domain from the entity `id_watermark` (u128)
//! and from runtime `NodeId`/`RuntimeInstanceId` — do not conflate
//! them.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{
    error::Error,
    id::ProcessorId,
    project::Project,
    routing::{PluginRef, Processor, ProcessorBinding},
};

/// The persisted plugin-instance identity issued for one authored
/// processor/plugin pair. Stable across save→reload and engine
/// restart; a changed [`PluginRef`] (including `capability_revision`)
/// is a different authored identity and receives a new id.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginInstanceId(u64);

impl PluginInstanceId {
    /// Wraps a raw issued value.
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    /// The raw issued value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

/// One persisted identity record: the issued id plus the authored
/// [`PluginRef`] it was minted for.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginInstanceIdentity {
    /// The issued instance id (`1 <= id < watermark`).
    pub id: PluginInstanceId,
    /// The authored plugin identity this id is pinned to.
    pub plugin: PluginRef,
}

/// The persisted plugin-instance identity store — schema v13's
/// `Project::plugin_instances` value.
///
/// `watermark` is the next id to issue (first allocation yields 1).
/// Issued ids are never reused: [`Self::remove`] drops the entry but
/// leaves the watermark untouched, so a later [`Self::allocate`] always
/// moves forward.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginInstanceIdentities {
    /// Next id to issue — always `>` every issued id.
    pub watermark: u64,
    /// Live identities keyed by owning [`ProcessorId`].
    pub instances: BTreeMap<ProcessorId, PluginInstanceIdentity>,
}

impl Default for PluginInstanceIdentities {
    /// Fresh state: watermark 1, no entries — the first allocation
    /// issues id 1.
    fn default() -> Self {
        Self {
            watermark: 1,
            instances: BTreeMap::new(),
        }
    }
}

impl PluginInstanceIdentities {
    /// Issues a fresh id for `processor` bound to `plugin` and records
    /// the entry, replacing any previous one. The watermark advances
    /// monotonically; the old id (if any) is retired, never reissued.
    ///
    /// # Errors
    /// [`Error::Validation`] when the watermark cannot advance (u64
    /// exhaustion — unreachable in practice).
    pub fn allocate(
        &mut self,
        processor: ProcessorId,
        plugin: &PluginRef,
    ) -> Result<PluginInstanceId, Error> {
        let next = self
            .watermark
            .checked_add(1)
            .ok_or_else(|| Error::Validation("plugin instance id watermark exhausted".into()))?;
        let id = PluginInstanceId::new(self.watermark);
        self.watermark = next;
        self.instances.insert(
            processor,
            PluginInstanceIdentity {
                id,
                plugin: plugin.clone(),
            },
        );
        Ok(id)
    }

    /// Ensures `processor` carries an identity for exactly `plugin`:
    /// an existing entry with an equal [`PluginRef`] keeps its id;
    /// anything else (missing entry or a different authored identity —
    /// descriptor, version or `capability_revision` drift) mints a new
    /// one. This is the apply/migration sync primitive: re-applying an
    /// unchanged processor is idempotent, a rebind is a new instance.
    ///
    /// # Errors
    /// Propagates [`Self::allocate`]'s watermark exhaustion error.
    pub fn ensure(&mut self, processor: ProcessorId, plugin: &PluginRef) -> Result<(), Error> {
        if self
            .instances
            .get(&processor)
            .is_some_and(|entry| entry.plugin == *plugin)
        {
            return Ok(());
        }
        self.allocate(processor, plugin).map(|_| ())
    }

    /// Drops `processor`'s identity entry, if any. The issued id stays
    /// retired — the watermark never moves backward, so the id can
    /// never be reissued to a later processor.
    pub fn remove(&mut self, processor: ProcessorId) {
        self.instances.remove(&processor);
    }

    /// Synchronizes the store with one processor's authored binding:
    /// `Resolved` → [`Self::ensure`], `Unresolved` → [`Self::remove`].
    /// The single dispatch every mutation path (ops, VCS apply entity
    /// writes, migration backfill) shares.
    ///
    /// # Errors
    /// Propagates [`Self::ensure`]'s watermark exhaustion error.
    pub fn sync_processor(&mut self, processor: &Processor) -> Result<(), Error> {
        match &processor.binding {
            ProcessorBinding::Resolved(plugin) => self.ensure(processor.id, plugin),
            ProcessorBinding::Unresolved { .. } => {
                self.remove(processor.id);
                Ok(())
            }
        }
    }

    /// Hydrates the persisted identity for `processor` when `plugin`
    /// (the instance's connect-time authored identity) matches the
    /// stored entry *and* the id lies inside the issued range.
    /// `None` covers every fail-closed case — no entry, plugin drift
    /// (including `capability_revision`), or an out-of-range id — so
    /// callers never distinguish "missing" from "stale".
    #[must_use]
    pub fn resolve(&self, processor: ProcessorId, plugin: &PluginRef) -> Option<PluginInstanceId> {
        let entry = self.instances.get(&processor)?;
        if entry.plugin != *plugin {
            return None;
        }
        let raw = entry.id.get();
        if raw == 0 || raw >= self.watermark {
            return None;
        }
        Some(entry.id)
    }

    /// The load/apply oracle: the store must be a total, consistent
    /// projection of `project.routing.processors`.
    ///
    /// Checks, in order: `watermark >= 1`; every entry id inside
    /// `1..watermark` and unique across entries; every entry names a
    /// live processor whose binding is `Resolved` with a `PluginRef`
    /// equal to the entry's; and every `Resolved` processor carries
    /// exactly one entry (`Unresolved` processors carry none — already
    /// rejected above when an entry exists for them).
    ///
    /// # Errors
    /// [`Error::Validation`] on any invariant breach;
    /// [`Error::NotFound`] when an entry names an absent processor.
    pub fn validate_against(&self, project: &Project) -> Result<(), Error> {
        if self.watermark < 1 {
            return Err(Error::Validation(
                "plugin instance watermark below 1".into(),
            ));
        }
        let mut issued = std::collections::BTreeSet::new();
        for (processor_id, entry) in &self.instances {
            let raw = entry.id.get();
            if raw == 0 || raw >= self.watermark {
                return Err(Error::Validation(format!(
                    "plugin instance id {raw} for processor {processor_id} outside issued range"
                )));
            }
            if !issued.insert(raw) {
                return Err(Error::Validation(format!(
                    "plugin instance id {raw} issued more than once"
                )));
            }
            let processor = project
                .routing
                .processors
                .get(processor_id)
                .ok_or_else(|| {
                    Error::NotFound(format!("processor {processor_id} (plugin instance)"))
                })?;
            match &processor.binding {
                ProcessorBinding::Resolved(plugin) => {
                    if entry.plugin != *plugin {
                        return Err(Error::Validation(format!(
                            "plugin instance for processor {processor_id} pins a different plugin"
                        )));
                    }
                }
                ProcessorBinding::Unresolved { .. } => {
                    return Err(Error::Validation(format!(
                        "unresolved processor {processor_id} carries a plugin instance"
                    )));
                }
            }
        }
        for (processor_id, processor) in &project.routing.processors {
            if matches!(processor.binding, ProcessorBinding::Resolved(_))
                && !self.instances.contains_key(processor_id)
            {
                return Err(Error::Validation(format!(
                    "resolved processor {processor_id} lacks a plugin instance"
                )));
            }
        }
        Ok(())
    }
}
