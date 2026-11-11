//! Semantic diff engine (VCS-02, synvelis#93).
//!
//! Pure, non-RT comparison of two VCS-01 [`ImmutableSnapshot`]s into one
//! canonical [`ChangeSet`]. The engine never mutates snapshots, the
//! controller, stores, VCS ports, or the runtime engine.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::Arc;

use serde::Serialize;

use crate::project::Project;
use crate::snapshot::ContentDigest;
use crate::snapshot::SnapshotIdentity;
use crate::snapshot::{ImmutableSnapshot, SnapshotCancel};

use super::adapters;
use super::types::{
    AmbiguityReason, ChangeEvidence, ChangeId, ChangeKind, ChangeSet, ChangeSetSummary,
    ComparisonCompleteness, ComparisonDomain, ComparisonEquality, ComparisonIssue, ComparisonKey,
    ComparisonSpec, DiffError, ObjectKind, SemanticChange, SemanticFieldDiff, SemanticObjectRef,
    SemanticTimeRange, SemanticValueRef, SnapshotSide,
};

/// A typed stable ID usable as an entity-map key.
pub(crate) trait EntityKey: Copy + Ord {
    /// Canonical `"<kind>-<32hex>"` form.
    fn tagged(&self) -> String;
    /// Raw `u128` value (generator-issued identity space).
    fn raw_u128(&self) -> u128;
}

macro_rules! impl_entity_key {
    ($($t:ty),+ $(,)?) => {
        $(
            impl EntityKey for $t {
                fn tagged(&self) -> String {
                    self.to_tagged()
                }
                fn raw_u128(&self) -> u128 {
                    self.raw()
                }
            }
        )+
    };
}

impl_entity_key!(
    crate::id::ProjectId,
    crate::id::TrackId,
    crate::id::LaneId,
    crate::id::ClipId,
    crate::id::NoteId,
    crate::id::AssetId,
    crate::id::SourceId,
    crate::id::ImportId,
    crate::id::ClockEpochId,
    crate::id::RecordingSessionId,
    crate::id::RecordingPassId,
    crate::id::CaptureStreamId,
    crate::id::CaptureSegmentId,
    crate::id::TakeId,
    crate::id::TakeGroupId,
    crate::id::CompId,
    crate::id::CompRegionId,
    crate::id::CompTransitionId,
    crate::id::ParameterTargetId,
    crate::id::AutomationLaneId,
    crate::id::ModulationSourceId,
    crate::id::ModulationAssignmentId,
    crate::id::ProcessorId,
    crate::id::ChainId,
    crate::id::SendId,
    crate::id::BusId,
    crate::id::StripId,
    crate::id::SemanticPortId,
    crate::id::RoutingConnectionId,
);

/// Internal field classification produced by the per-domain policy.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub(crate) enum FieldClass {
    /// Report as `Modified` (default).
    Modified,
    /// Report as `Moved`.
    Moved,
    /// Report as `Reordered`.
    Reordered,
    /// Report as `Reconnected`.
    Reconnected,
    /// Report as `Replaced`.
    Replaced,
    /// Owner/lineage reference changed → `AmbiguousIdentity`.
    Identity,
    /// Field handled elsewhere (child entities / custom pass) → no report.
    Skip,
    /// Migration-projected field → suppress; difference becomes
    /// `PartialProjection` instead of a fabricated change.
    Suppressed(&'static str),
}

impl FieldClass {
    pub(crate) fn change_kind(self) -> Option<ChangeKind> {
        match self {
            Self::Modified => Some(ChangeKind::Modified),
            Self::Moved => Some(ChangeKind::Moved),
            Self::Reordered => Some(ChangeKind::Reordered),
            Self::Reconnected => Some(ChangeKind::Reconnected),
            Self::Replaced => Some(ChangeKind::Replaced),
            Self::Identity | Self::Skip | Self::Suppressed(_) => None,
        }
    }
}

/// Identity triple of one compared entity: semantic kind, canonical
/// stable ID, and raw generator ID (for cross-kind collision checks).
pub(crate) struct EntityId {
    /// Semantic object kind.
    pub kind: ObjectKind,
    /// Canonical `"<kind>-<tagged>"` stable ID.
    pub stable_id: String,
    /// Raw `u128` identity-space value.
    pub raw: u128,
}

/// Shared diff context threaded through every domain adapter.
pub(crate) struct DiffContext<'a> {
    /// Before projection.
    pub before: &'a Project,
    /// After projection.
    pub after: &'a Project,
    /// Before snapshot identity (`project_id`, `schema_version`).
    pub before_id: &'a SnapshotIdentity,
    /// After snapshot identity.
    pub after_id: &'a SnapshotIdentity,
    /// Comparison spec (domains, budget).
    pub spec: &'a ComparisonSpec,
    /// Digest of the [`ComparisonKey`] — binds every emitted change ID.
    pub key_digest: ContentDigest,
    /// Cooperative cancellation token.
    pub cancel: &'a SnapshotCancel,
    /// raw u128 → claimed object kinds, per snapshot (entity-issued IDs).
    pub before_raw: BTreeMap<u128, BTreeSet<ObjectKind>>,
    /// See [`Self::before_raw`].
    pub after_raw: BTreeMap<u128, BTreeSet<ObjectKind>>,
    /// Raws claimed by different kinds on the two sides (or internally
    /// inconsistent) — every entity holding one is ambiguous.
    pub ambiguous_raws: BTreeSet<u128>,
    /// Confirmed changes (unsorted until finalize).
    pub changes: Vec<SemanticChange>,
    /// Unresolved issues (unsorted until finalize).
    pub issues: Vec<ComparisonIssue>,
    /// `(domain, detail)` pairs already reported as `PartialProjection`.
    pub suppressed: BTreeSet<(ComparisonDomain, &'static str)>,
    /// Entities visited (budget accounting).
    pub entities: usize,
}

impl DiffContext<'_> {
    /// `true` when `raw` is claimed ambiguously across the two snapshots.
    pub(crate) fn raw_is_ambiguous(&self, raw: u128) -> bool {
        self.ambiguous_raws.contains(&raw)
    }

    /// Cooperative cancellation + budget checks.
    pub(crate) fn tick(&self, entities: usize) -> Result<(), DiffError> {
        if self.cancel.is_cancelled() {
            return Err(DiffError::Cancelled);
        }
        if self.entities + entities > self.spec.budget.max_entities {
            return Err(DiffError::BudgetExceeded {
                limit: "max_entities",
                actual: self.entities + entities,
            });
        }
        Ok(())
    }

    /// Builds the canonical object reference for an entity.
    pub(crate) fn object_ref(&self, kind: ObjectKind, stable_id: String) -> SemanticObjectRef {
        SemanticObjectRef {
            project_id: self.before_id.project_id,
            kind,
            stable_id,
            lineage: None,
        }
    }

    /// Digest of an entity's canonical JSON serialization.
    pub(crate) fn value_ref<T: Serialize>(entity: &T) -> Result<SemanticValueRef, DiffError> {
        let bytes = serde_json::to_vec(entity)
            .map_err(|e| DiffError::Internal(format!("entity serialize: {e}")))?;
        Ok(SemanticValueRef {
            digest: ContentDigest::sha256(&bytes),
        })
    }

    /// Deterministic change ID bound to the comparison key.
    pub(crate) fn change_id(
        &self,
        object: &SemanticObjectRef,
        kind: ChangeKind,
        fields: &[SemanticFieldDiff],
    ) -> ChangeId {
        let field_key = fields
            .iter()
            .map(|f| f.field.as_str())
            .collect::<Vec<_>>()
            .join(";");
        let material = format!(
            "{}|{}|{}|{}|{}|{}",
            self.key_digest,
            object.kind as u8,
            object.stable_id,
            kind as u8,
            field_key,
            object.project_id.to_tagged(),
        );
        let digest = ContentDigest::sha256(material.as_bytes()).to_string();
        // "sha256:" + 64 hex → keep 24 hex chars of content hash.
        ChangeId(format!("chg-{}", &digest[7..31]))
    }

    /// Emits one confirmed change (budget-checked).
    pub(crate) fn emit_change(&mut self, change: SemanticChange) -> Result<(), DiffError> {
        if self.changes.len() >= self.spec.budget.max_changes {
            return Err(DiffError::BudgetExceeded {
                limit: "max_changes",
                actual: self.changes.len() + 1,
            });
        }
        self.changes.push(change);
        Ok(())
    }

    /// Emits (or dedup-records) one unresolved issue.
    pub(crate) fn emit_issue(&mut self, issue: ComparisonIssue) -> Result<(), DiffError> {
        if self.issues.len() >= self.spec.budget.max_unresolved {
            return Err(DiffError::BudgetExceeded {
                limit: "max_unresolved",
                actual: self.issues.len() + 1,
            });
        }
        self.issues.push(issue);
        Ok(())
    }

    /// Emits an `AmbiguousIdentity` issue for `object`.
    pub(crate) fn emit_ambiguous(
        &mut self,
        object: SemanticObjectRef,
        reason: AmbiguityReason,
    ) -> Result<(), DiffError> {
        self.emit_issue(ComparisonIssue::AmbiguousIdentity { object, reason })
    }

    /// Emits a `PartialProjection` issue once per `(domain, detail)`.
    pub(crate) fn emit_suppressed(
        &mut self,
        domain: ComparisonDomain,
        detail: &'static str,
    ) -> Result<(), DiffError> {
        if self.suppressed.insert((domain, detail)) {
            self.emit_issue(ComparisonIssue::PartialProjection {
                domain,
                detail: detail.to_string(),
            })?;
        }
        Ok(())
    }

    /// Presence diff for one entity: `None` on a side means absent.
    /// `entity.raw` is the raw ID for collision detection.
    pub(crate) fn diff_presence<V: Serialize>(
        &mut self,
        entity: EntityId,
        before: Option<&V>,
        after: Option<&V>,
        affected_range: impl Fn(Option<&V>, Option<&V>) -> Option<SemanticTimeRange>,
    ) -> Result<(), DiffError> {
        if self.raw_is_ambiguous(entity.raw) {
            return self.emit_ambiguous(
                self.object_ref(entity.kind, entity.stable_id),
                AmbiguityReason::CrossKindId,
            );
        }
        let (change_kind, before_ref, after_ref) = match (before, after) {
            (None, Some(a)) => (ChangeKind::Added, None, Some(Self::value_ref(a)?)),
            (Some(b), None) => (ChangeKind::Removed, Some(Self::value_ref(b)?), None),
            _ => return Ok(()),
        };
        let object = self.object_ref(entity.kind, entity.stable_id);
        let id = self.change_id(&object, change_kind, &[]);
        self.emit_change(SemanticChange {
            id,
            object,
            kind: change_kind,
            before: before_ref,
            after: after_ref,
            affected_range: affected_range(before, after),
            evidence: ChangeEvidence::default(),
            dependencies: Vec::new(),
        })
    }

    /// Matched-pair diff: field-level canonical JSON comparison classified
    /// by the per-kind policy. `digest_only` kinds skip field extraction.
    pub(crate) fn diff_matched<V>(
        &mut self,
        entity: EntityId,
        before: &V,
        after: &V,
        affected_range: impl Fn(Option<&V>, Option<&V>) -> Option<SemanticTimeRange>,
        digest_only: bool,
    ) -> Result<(), DiffError>
    where
        V: PartialEq + Serialize,
    {
        if self.raw_is_ambiguous(entity.raw) {
            return self.emit_ambiguous(
                self.object_ref(entity.kind, entity.stable_id.clone()),
                AmbiguityReason::CrossKindId,
            );
        }
        if before == after {
            return Ok(());
        }
        let object = self.object_ref(entity.kind, entity.stable_id);
        let before_ref = Self::value_ref(before)?;
        let after_ref = Self::value_ref(after)?;
        let affected = affected_range(Some(before), Some(after));

        if digest_only {
            let id = self.change_id(&object, ChangeKind::Modified, &[]);
            return self.emit_change(SemanticChange {
                id,
                object,
                kind: ChangeKind::Modified,
                before: Some(before_ref),
                after: Some(after_ref),
                affected_range: affected,
                evidence: ChangeEvidence::default(),
                dependencies: Vec::new(),
            });
        }

        let before_json = serde_json::to_value(before)
            .map_err(|e| DiffError::Internal(format!("entity to_value: {e}")))?;
        let after_json = serde_json::to_value(after)
            .map_err(|e| DiffError::Internal(format!("entity to_value: {e}")))?;
        let mut raw_fields = Vec::new();
        json_diff(&before_json, &after_json, "", &mut raw_fields);

        // Classify each field diff; identity hits poison the whole object.
        let mut identity_hit = false;
        let mut by_kind: BTreeMap<ChangeKind, Vec<SemanticFieldDiff>> = BTreeMap::new();
        for f in raw_fields {
            match adapters::classify_field(self, entity.kind, &f.field, &f.before, &f.after) {
                FieldClass::Identity => identity_hit = true,
                FieldClass::Suppressed(detail) => {
                    self.emit_suppressed(entity.kind.domain(), detail)?;
                }
                FieldClass::Skip => {}
                class => {
                    if let Some(ck) = class.change_kind() {
                        by_kind.entry(ck).or_default().push(f);
                    }
                }
            }
        }
        if identity_hit {
            return self.emit_ambiguous(object, AmbiguityReason::OwnerConflict);
        }
        for (ck, fields) in by_kind {
            let id = self.change_id(&object, ck, &fields);
            self.emit_change(SemanticChange {
                id,
                object: object.clone(),
                kind: ck,
                before: Some(before_ref),
                after: Some(after_ref),
                affected_range: affected,
                evidence: ChangeEvidence { fields },
                dependencies: Vec::new(),
            })?;
        }
        Ok(())
    }
}

/// Recursive canonical-JSON diff. Objects recurse per key; arrays and
/// scalars diff wholesale at their path (order semantics are decided by
/// the field policy, which inspects the values).
pub(crate) fn json_diff(
    before: &serde_json::Value,
    after: &serde_json::Value,
    path: &str,
    out: &mut Vec<SemanticFieldDiff>,
) {
    use serde_json::Value;
    if let (Value::Object(b), Value::Object(a)) = (before, after) {
        let mut keys: Vec<&String> = b.keys().chain(a.keys()).collect();
        keys.sort();
        keys.dedup();
        for key in keys {
            let bv = b.get(key).cloned().unwrap_or(Value::Null);
            let av = a.get(key).cloned().unwrap_or(Value::Null);
            let child = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}.{key}")
            };
            json_diff(&bv, &av, &child, out);
        }
        return;
    }
    if before != after {
        out.push(SemanticFieldDiff {
            field: path.to_string(),
            before: before.clone(),
            after: after.clone(),
        });
    }
}

/// Diffs one entity map by stable ID: matched pairs go through
/// [`DiffContext::diff_matched`], single-sided entries through
/// [`DiffContext::diff_presence`]. `stable_id`/`raw` come from the key.
pub(crate) fn diff_map<K, V>(
    ctx: &mut DiffContext<'_>,
    kind: ObjectKind,
    before: &BTreeMap<K, V>,
    after: &BTreeMap<K, V>,
    affected_range: impl Fn(Option<&V>, Option<&V>) -> Option<SemanticTimeRange> + Copy,
    digest_only: bool,
) -> Result<(), DiffError>
where
    K: EntityKey,
    V: PartialEq + Serialize,
{
    ctx.tick(before.len() + after.len())?;
    ctx.entities += before.len() + after.len();
    let mut keys: BTreeSet<K> = before.keys().copied().collect();
    keys.extend(after.keys().copied());
    for key in keys {
        if ctx.cancel.is_cancelled() {
            return Err(DiffError::Cancelled);
        }
        let entity = EntityId {
            kind,
            stable_id: key.tagged(),
            raw: key.raw_u128(),
        };
        match (before.get(&key), after.get(&key)) {
            (Some(b), Some(a)) => {
                ctx.diff_matched(entity, b, a, affected_range, digest_only)?;
            }
            (Some(b), None) => {
                ctx.diff_presence(entity, Some(b), None, affected_range)?;
            }
            (None, Some(a)) => {
                ctx.diff_presence(entity, None, Some(a), affected_range)?;
            }
            (None, None) => {}
        }
    }
    Ok(())
}

/// Builds `raw u128 → claimed object kinds` for one project, covering
/// every entity-issued ID collection. Keyed-synthetic collections
/// (`smf_documents`, `applied_commits`, `monitoring.tracks`,
/// `plugin_instances.instances`) reuse another entity's ID space and
/// are excluded so they cannot fabricate collisions.
fn raw_kind_map(project: &Project) -> BTreeMap<u128, BTreeSet<ObjectKind>> {
    let mut map: BTreeMap<u128, BTreeSet<ObjectKind>> = BTreeMap::new();
    let mut add = |kind: ObjectKind, raw: u128| {
        map.entry(raw).or_default().insert(kind);
    };
    for id in project.tracks.keys() {
        add(ObjectKind::Track, id.raw());
    }
    for id in project.lanes.keys() {
        add(ObjectKind::Lane, id.raw());
    }
    for id in project.clips.keys() {
        add(ObjectKind::Clip, id.raw());
    }
    for id in project.notes.keys() {
        add(ObjectKind::Note, id.raw());
    }
    for id in project.assets.keys() {
        add(ObjectKind::Asset, id.raw());
    }
    for id in project.sources.keys() {
        add(ObjectKind::MediaSource, id.raw());
    }
    for (id, src) in &project.midi_sources {
        add(ObjectKind::MidiSource, id.raw());
        for nid in src.notes.keys() {
            add(ObjectKind::MidiNote, nid.raw());
        }
    }
    for id in project.clock_epochs.keys() {
        add(ObjectKind::ClockEpoch, id.raw());
    }
    let rec = &project.recording;
    for id in rec.sessions.keys() {
        add(ObjectKind::RecordingSession, id.raw());
    }
    for id in rec.passes.keys() {
        add(ObjectKind::RecordingPass, id.raw());
    }
    for id in rec.streams.keys() {
        add(ObjectKind::CaptureStream, id.raw());
    }
    for id in rec.segments.keys() {
        add(ObjectKind::CaptureSegment, id.raw());
    }
    for id in rec.takes.keys() {
        add(ObjectKind::Take, id.raw());
    }
    for id in project.applied_imports.keys() {
        add(ObjectKind::AppliedImport, id.raw());
    }
    let comp = &project.comp;
    for id in comp.take_groups.keys() {
        add(ObjectKind::TakeGroup, id.raw());
    }
    for id in comp.comps.keys() {
        add(ObjectKind::Comp, id.raw());
    }
    for id in comp.regions.keys() {
        add(ObjectKind::CompRegion, id.raw());
    }
    for id in comp.transitions.keys() {
        add(ObjectKind::CompTransition, id.raw());
    }
    let control = &project.control;
    for id in control.targets.keys() {
        add(ObjectKind::ParameterTarget, id.raw());
    }
    for id in control.lanes.keys() {
        add(ObjectKind::AutomationLane, id.raw());
    }
    for id in control.sources.keys() {
        add(ObjectKind::ModulationSource, id.raw());
    }
    for id in control.assignments.keys() {
        add(ObjectKind::ModulationAssignment, id.raw());
    }
    let routing = &project.routing;
    for id in routing.buses.keys() {
        add(ObjectKind::Bus, id.raw());
    }
    for id in routing.strips.keys() {
        add(ObjectKind::Strip, id.raw());
    }
    for id in routing.processors.keys() {
        add(ObjectKind::Processor, id.raw());
    }
    for id in routing.chains.keys() {
        add(ObjectKind::ProcessorChain, id.raw());
    }
    for id in routing.ports.keys() {
        add(ObjectKind::SemanticPort, id.raw());
    }
    for id in routing.connections.keys() {
        add(ObjectKind::RoutingConnection, id.raw());
    }
    for id in routing.sends.keys() {
        add(ObjectKind::SendConfig, id.raw());
    }
    map
}

/// Raws whose claimed kind sets differ between the two snapshots, or
/// that are internally inconsistent within one snapshot.
fn ambiguous_raws(
    before: &BTreeMap<u128, BTreeSet<ObjectKind>>,
    after: &BTreeMap<u128, BTreeSet<ObjectKind>>,
) -> BTreeSet<u128> {
    let mut out = BTreeSet::new();
    for (raw, kinds) in before {
        if kinds.len() > 1 {
            out.insert(*raw);
        }
        if let Some(other) = after.get(raw)
            && other != kinds
        {
            out.insert(*raw);
        }
    }
    for (raw, kinds) in after {
        if kinds.len() > 1 {
            out.insert(*raw);
        }
    }
    out
}

/// Collects snapshot-manifest issues (asset availability, opaque state)
/// for one side. Gated by the included domains they belong to.
fn collect_snapshot_issues(
    ctx: &mut DiffContext<'_>,
    snap: &ImmutableSnapshot,
    side: SnapshotSide,
) -> Result<(), DiffError> {
    if ctx.spec.domains.contains(ComparisonDomain::Media) {
        for entry in &snap.assets.entries {
            match entry.status {
                crate::snapshot::ContentStatus::Present => {}
                crate::snapshot::ContentStatus::Missing => {
                    ctx.emit_issue(ComparisonIssue::MissingAsset {
                        asset_id: entry.asset_id,
                        side,
                        status: entry.status,
                    })?;
                }
                crate::snapshot::ContentStatus::Unknown => {
                    ctx.emit_issue(ComparisonIssue::UnresolvedSource {
                        asset_id: entry.asset_id,
                        side,
                    })?;
                }
            }
        }
    }
    if ctx.spec.domains.contains(ComparisonDomain::Routing) {
        for opaque in &snap.opaque_state {
            if opaque.status != crate::snapshot::OpaqueStateStatus::Known {
                ctx.emit_issue(ComparisonIssue::MissingOpaqueState {
                    processor_id: opaque.processor_id,
                    side,
                    status: opaque.status,
                })?;
            }
        }
    }
    Ok(())
}

/// Merges `Before`/`After` duplicates of the same observation into `Both`.
fn merge_issue_sides(issues: &mut [ComparisonIssue]) {
    let mut sides: BTreeMap<String, SnapshotSide> = BTreeMap::new();
    for issue in issues.iter() {
        let (key, side) = issue_side_key(issue);
        sides
            .entry(key)
            .and_modify(|s| {
                if *s != side {
                    *s = SnapshotSide::Both;
                }
            })
            .or_insert(side);
    }
    for issue in issues.iter_mut() {
        let (key, _) = issue_side_key(issue);
        if let Some(SnapshotSide::Both) = sides.get(&key) {
            set_issue_side(issue, SnapshotSide::Both);
        }
    }
}

/// Canonical dedup key for an issue ignoring its `side` field.
fn issue_side_key(issue: &ComparisonIssue) -> (String, SnapshotSide) {
    let side = match issue {
        ComparisonIssue::UnsupportedSchema { side, .. }
        | ComparisonIssue::MissingAsset { side, .. }
        | ComparisonIssue::MissingOpaqueState { side, .. }
        | ComparisonIssue::UnresolvedSource { side, .. } => *side,
        _ => SnapshotSide::Both,
    };
    // Serialize with side blanked for a stable merge key.
    let key = match issue {
        ComparisonIssue::UnsupportedSchema { found, .. } => format!("schema:{found}"),
        ComparisonIssue::MissingAsset {
            asset_id, status, ..
        } => {
            format!("asset:{asset_id}:{status:?}")
        }
        ComparisonIssue::MissingOpaqueState {
            processor_id,
            status,
            ..
        } => format!("opaque:{processor_id}:{status:?}"),
        ComparisonIssue::AmbiguousIdentity { object, reason } => {
            format!(
                "ambig:{}:{}:{reason:?}",
                object.kind as u8, object.stable_id
            )
        }
        ComparisonIssue::PartialProjection { domain, detail } => {
            format!("partial:{domain:?}:{detail}")
        }
        ComparisonIssue::UnresolvedSource { asset_id, .. } => format!("unresolved:{asset_id}"),
        ComparisonIssue::InvalidReference { detail } => format!("invalid:{detail}"),
    };
    (key, side)
}

fn set_issue_side(issue: &mut ComparisonIssue, side: SnapshotSide) {
    match issue {
        ComparisonIssue::UnsupportedSchema { side: s, .. }
        | ComparisonIssue::MissingAsset { side: s, .. }
        | ComparisonIssue::MissingOpaqueState { side: s, .. }
        | ComparisonIssue::UnresolvedSource { side: s, .. } => *s = side,
        _ => {}
    }
}

/// Sorts issues deterministically and removes exact duplicates.
fn finalize_issues(issues: &mut Vec<ComparisonIssue>) {
    merge_issue_sides(issues);
    issues.sort_by(|a, b| {
        serde_json::to_string(a)
            .unwrap_or_default()
            .cmp(&serde_json::to_string(b).unwrap_or_default())
    });
    issues.dedup();
}

/// Deterministic change ordering: domain → object kind → stable ID →
/// change kind → primary field → change ID.
fn sort_changes(changes: &mut [SemanticChange]) {
    changes.sort_by(|a, b| {
        (
            a.object.kind.domain(),
            a.object.kind,
            a.object.stable_id.as_str(),
            a.kind,
            a.evidence.fields.first().map_or("", |f| f.field.as_str()),
            a.id.0.as_str(),
        )
            .cmp(&(
                b.object.kind.domain(),
                b.object.kind,
                b.object.stable_id.as_str(),
                b.kind,
                b.evidence.fields.first().map_or("", |f| f.field.as_str()),
                b.id.0.as_str(),
            ))
    });
}

/// Compares two immutable snapshots into one canonical [`ChangeSet`].
///
/// Pure and non-RT: reads only the snapshots' projections and manifests,
/// writes nothing. Fails only on cancellation or budget exhaustion —
/// uncertainty is reported inside the result as unresolved issues.
///
/// # Errors
/// [`DiffError::Cancelled`] on cooperative cancellation,
/// [`DiffError::BudgetExceeded`] when a budget bound is hit,
/// [`DiffError::Internal`] on canonical-serialization failure.
pub fn compare_snapshots(
    before: &ImmutableSnapshot,
    after: &ImmutableSnapshot,
    spec: &ComparisonSpec,
    cancel: &SnapshotCancel,
) -> Result<ChangeSet, DiffError> {
    let key = ComparisonKey::new(&before.identity, &after.identity, spec);
    let key_digest = key.digest();
    let before_project = before.projection.project();
    let after_project = after.projection.project();

    let mut ctx = DiffContext {
        before: before_project,
        after: after_project,
        before_id: &before.identity,
        after_id: &after.identity,
        spec,
        key_digest,
        cancel,
        before_raw: BTreeMap::new(),
        after_raw: BTreeMap::new(),
        ambiguous_raws: BTreeSet::new(),
        changes: Vec::new(),
        issues: Vec::new(),
        suppressed: BTreeSet::new(),
        entities: 0,
    };

    if before.identity.project_id != after.identity.project_id {
        // Cross-project comparison: no auto-matching; explicit identity
        // mapping is out of scope → Unknown.
        ctx.emit_ambiguous(
            ctx.object_ref(ObjectKind::Project, before.identity.project_id.to_tagged()),
            AmbiguityReason::CrossProject,
        )?;
    } else if before.identity.digest != after.identity.digest {
        if spec.domains.is_empty() {
            ctx.emit_suppressed(ComparisonDomain::Project, "no-domains-included")?;
        } else {
            ctx.before_raw = raw_kind_map(before_project);
            ctx.after_raw = raw_kind_map(after_project);
            ctx.ambiguous_raws = ambiguous_raws(&ctx.before_raw, &ctx.after_raw);
            for adapter in adapters::default_adapters() {
                if spec.domains.contains(adapter.domain()) {
                    if cancel.is_cancelled() {
                        return Err(DiffError::Cancelled);
                    }
                    adapter.diff(&mut ctx)?;
                }
            }
        }
        // Snapshot-manifest issues apply to both equal and differing
        // content: a partial snapshot can never promote to Equal.
        if before.identity.project_id == after.identity.project_id {
            collect_snapshot_issues(&mut ctx, before, SnapshotSide::Before)?;
            collect_snapshot_issues(&mut ctx, after, SnapshotSide::After)?;
        }
    } else {
        // AA fast path: identical bytes → no entity changes possible,
        // but manifest issues still gate the Equal promotion.
        collect_snapshot_issues(&mut ctx, before, SnapshotSide::Before)?;
        collect_snapshot_issues(&mut ctx, after, SnapshotSide::After)?;
    }

    finalize_issues(&mut ctx.issues);
    sort_changes(&mut ctx.changes);

    let mut summary = ChangeSetSummary {
        unresolved: ctx.issues.len(),
        ..ChangeSetSummary::default()
    };
    for change in &ctx.changes {
        summary.total += 1;
        match change.kind {
            ChangeKind::Added => summary.added += 1,
            ChangeKind::Removed => summary.removed += 1,
            ChangeKind::Modified => summary.modified += 1,
            ChangeKind::Moved => summary.moved += 1,
            ChangeKind::Reordered => summary.reordered += 1,
            ChangeKind::Reconnected => summary.reconnected += 1,
            ChangeKind::Replaced => summary.replaced += 1,
        }
    }

    let overall = if !ctx.changes.is_empty() {
        ComparisonEquality::Different
    } else if let Some(first) = ctx.issues.first() {
        ComparisonEquality::Unknown(first.clone())
    } else {
        ComparisonEquality::Equal
    };
    let completeness = if ctx.issues.is_empty() {
        ComparisonCompleteness::Complete
    } else {
        ComparisonCompleteness::Partial
    };

    Ok(ChangeSet {
        comparison: spec.clone(),
        key,
        changes: ctx.changes,
        unresolved: ctx.issues,
        overall,
        completeness,
        summary,
    })
}

/// Bounded derived cache of [`ChangeSet`]s keyed by comparison-key
/// digest. A cache hit returns the identical result without recomputing;
/// it is never a source of truth for project or media content.
pub struct ChangeSetCache {
    capacity: usize,
    /// LRU order: front = oldest.
    order: VecDeque<ContentDigest>,
    entries: BTreeMap<ContentDigest, Arc<ChangeSet>>,
}

impl ChangeSetCache {
    /// A cache holding at most `capacity` results (0 disables caching).
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            order: VecDeque::new(),
            entries: BTreeMap::new(),
        }
    }

    /// Returns the cached [`ChangeSet`] for this comparison or computes,
    /// stores, and returns it. `Arc`-shared so views reuse one result.
    ///
    /// # Errors
    /// Propagates [`compare_snapshots`] errors.
    pub fn get_or_compare(
        &mut self,
        before: &ImmutableSnapshot,
        after: &ImmutableSnapshot,
        spec: &ComparisonSpec,
        cancel: &SnapshotCancel,
    ) -> Result<Arc<ChangeSet>, DiffError> {
        let digest = ComparisonKey::new(&before.identity, &after.identity, spec).digest();
        if let Some(hit) = self.entries.get(&digest) {
            if let Some(pos) = self.order.iter().position(|d| *d == digest) {
                self.order.remove(pos);
            }
            self.order.push_back(digest);
            return Ok(Arc::clone(hit));
        }
        let change_set = Arc::new(compare_snapshots(before, after, spec, cancel)?);
        if self.capacity > 0 {
            self.entries.insert(digest, Arc::clone(&change_set));
            self.order.push_back(digest);
            while self.order.len() > self.capacity {
                if let Some(oldest) = self.order.pop_front() {
                    self.entries.remove(&oldest);
                }
            }
        }
        Ok(change_set)
    }
}
