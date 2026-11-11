//! Issue #101 / PC-C4-05: persisted `StablePluginInstanceId` —
//! `Project::plugin_instances` (schema v13) allocation, monotonic
//! watermark, save→load round trip, v12→v13 migration backfill, the
//! load/apply oracle, undo/redo snapshot semantics, and VCS-05 apply
//! synchronization on the direct entity-write path.

use synvelis_project::apply::{
    ApplyApproval, ApplyChangeIntent, ApplyOutcome, ApplySources, ChangeSetSource,
    InMemoryApplyJournal, PluginAvailability, ResolvedChangeSet, apply_selected,
    target_fingerprint,
};
use synvelis_project::controller::{Command, ProjectController, ProjectRevision};
use synvelis_project::diff::{
    ChangeKind, ChangeSet, ComparisonSpec, ObjectKind, SemanticChange, SemanticObjectRef,
    compare_snapshots,
};
use synvelis_project::id::{CounterIdGenerator, ProcessorId, ProjectId, StripId};
use synvelis_project::ops::ProjectOp;
use synvelis_project::project::Project;
use synvelis_project::routing::{PluginRef, ProcessorBinding};
use synvelis_project::snapshot::{
    AssetAvailabilityManifest, BlobProbe, BlobRead, ContentDigest, FixedSnapshotLocator,
    ImmutableSnapshot, ReadOnlyProjectProjection, SnapshotCancel, SnapshotCompleteness,
    SnapshotIdentity, SnapshotOrigin, SnapshotResolver, SnapshotValidation,
};
use synvelis_project::store::{InMemoryStore, SCHEMA_VERSION};
use synvelis_project::*;

// ── controller/op fixtures (mirrors monitoring_persist.rs) ─────────

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "p"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply_op(c: &mut ProjectController, op: ProjectOp) {
    let rev = c.live_revision();
    c.apply(Command {
        expected_revision: rev,
        op,
    })
    .unwrap();
}

fn add_track(c: &mut ProjectController, name: &str) -> TrackId {
    apply_op(c, ProjectOp::AddTrack { name: name.into() });
    *c.live().tracks.keys().next_back().unwrap()
}

fn add_strip(c: &mut ProjectController, track: TrackId) -> StripId {
    apply_op(c, ProjectOp::AddTrackStrip { track_id: track });
    c.live().routing.strip_of_track(track).unwrap().id
}

fn plugin_ref() -> PluginRef {
    PluginRef {
        descriptor: "vendor.inst-persist".into(),
        version: "1.0.0".into(),
        capability_revision: 3,
    }
}

fn resolved_binding() -> ProcessorBinding {
    ProcessorBinding::Resolved(plugin_ref())
}

fn unresolved_binding() -> ProcessorBinding {
    ProcessorBinding::Unresolved {
        descriptor: "vendor.missing".into(),
    }
}

/// Project with one track strip ready for processor inserts.
fn strip_project() -> (ProjectController, StripId) {
    let mut c = controller();
    let track = add_track(&mut c, "vox");
    let strip = add_strip(&mut c, track);
    (c, strip)
}

fn insert_processor(
    c: &mut ProjectController,
    strip: StripId,
    binding: ProcessorBinding,
) -> ProcessorId {
    apply_op(
        c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "fx".into(),
            binding,
            sidechain_input: false,
        },
    );
    *c.live().routing.processors.keys().next_back().unwrap()
}

fn instance_id(c: &ProjectController, processor: ProcessorId) -> u64 {
    c.live().plugin_instances.instances[&processor].id.get()
}

// ── PC-C4-05-01: fresh state + round trip ───────────────────────────

#[test]
fn fresh_project_has_empty_identity_store() {
    let c = controller();
    assert_eq!(c.live().plugin_instances.watermark, 1);
    assert!(c.live().plugin_instances.instances.is_empty());
}

#[test]
fn resolved_insert_allocates_and_roundtrips() {
    let (mut c, strip) = strip_project();
    let first = insert_processor(&mut c, strip, resolved_binding());
    let second = insert_processor(&mut c, strip, resolved_binding());

    assert_eq!(instance_id(&c, first), 1);
    assert_eq!(instance_id(&c, second), 2);
    assert_eq!(c.live().plugin_instances.watermark, 3);
    assert_eq!(
        c.live().plugin_instances.instances[&first].plugin,
        plugin_ref()
    );

    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c.live(), c2.live());
    assert_eq!(instance_id(&c2, first), 1);
    assert_eq!(instance_id(&c2, second), 2);
    assert_eq!(c2.live().plugin_instances.watermark, 3);
}

#[test]
fn unresolved_insert_has_no_entry() {
    let (mut c, strip) = strip_project();
    let proc = insert_processor(&mut c, strip, unresolved_binding());
    assert!(!c.live().plugin_instances.instances.contains_key(&proc));
    assert_eq!(c.live().plugin_instances.watermark, 1);
    assert!(c.live().validate_references().is_ok());
}

// ── PC-C4-05-02: monotonic watermark + undo/redo ────────────────────

#[test]
fn remove_then_insert_never_reuses_id() {
    let (mut c, strip) = strip_project();
    let a = insert_processor(&mut c, strip, resolved_binding());
    assert_eq!(instance_id(&c, a), 1);

    apply_op(&mut c, ProjectOp::RemoveProcessor { processor_id: a });
    assert!(c.live().plugin_instances.instances.is_empty());
    // The watermark does not move backward — id 1 stays retired.
    assert_eq!(c.live().plugin_instances.watermark, 2);

    let b = insert_processor(&mut c, strip, resolved_binding());
    assert_eq!(instance_id(&c, b), 2);
    assert_eq!(c.live().plugin_instances.watermark, 3);
    assert!(c.live().validate_references().is_ok());
}

#[test]
fn undo_redo_restores_identity_snapshot() {
    let (mut c, strip) = strip_project();
    let a = insert_processor(&mut c, strip, resolved_binding());
    let b = insert_processor(&mut c, strip, resolved_binding());
    assert_eq!(instance_id(&c, a), 1);
    assert_eq!(instance_id(&c, b), 2);

    // Undo the second insert: the snapshot restores A's entry and the
    // watermark that existed at that revision — B's id was never
    // issued on this timeline.
    assert!(c.undo().unwrap());
    assert!(c.live().plugin_instances.instances.contains_key(&a));
    assert!(!c.live().plugin_instances.instances.contains_key(&b));
    assert_eq!(c.live().plugin_instances.watermark, 2);
    assert_eq!(instance_id(&c, a), 1);

    // Redo restores B's minted identity verbatim — snapshot restore,
    // not re-allocation.
    assert!(c.redo().unwrap());
    assert_eq!(instance_id(&c, a), 1);
    assert_eq!(instance_id(&c, b), 2);
    assert_eq!(c.live().plugin_instances.watermark, 3);
    assert!(c.live().validate_references().is_ok());
}

// ── PC-C4-05-04: v12 → v13 migration ────────────────────────────────

#[test]
fn v12_snapshot_migrates_with_deterministic_backfill() {
    let (mut c, strip) = strip_project();
    let first = insert_processor(&mut c, strip, resolved_binding());
    let unresolved = insert_processor(&mut c, strip, unresolved_binding());
    let second = insert_processor(&mut c, strip, resolved_binding());
    let _ = (first, unresolved, second);

    // Rewrite the saved envelope as a v12 body: schema_version 12 and
    // no `plugin_instances` member (v12 carried no identity store).
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let head = store.head().unwrap().unwrap();
    let bytes = store.read_snapshot(&head).unwrap();
    let mut env: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    env["schema_version"] = serde_json::json!(12);
    env["project"]
        .as_object_mut()
        .unwrap()
        .remove("plugin_instances");
    let v12_bytes = serde_json::to_vec(&env).unwrap();

    let mut store12 = InMemoryStore::new();
    let saved_rev = c.saved().unwrap().revision;
    store12.save("p", saved_rev, &v12_bytes).unwrap();

    let mut c2 = controller();
    c2.load(&store12).unwrap();

    // Resolved processors backfill in ProcessorId order: ids 1 and 2,
    // watermark 3. The unresolved processor gets no entry.
    let ids: Vec<ProcessorId> = c2
        .live()
        .routing
        .processors
        .iter()
        .filter(|(_, p)| matches!(p.binding, ProcessorBinding::Resolved(_)))
        .map(|(id, _)| *id)
        .collect();
    assert_eq!(ids.len(), 2);
    assert_eq!(instance_id(&c2, ids[0]), 1);
    assert_eq!(instance_id(&c2, ids[1]), 2);
    assert_eq!(c2.live().plugin_instances.watermark, 3);
    assert!(
        !c2.live()
            .plugin_instances
            .instances
            .contains_key(&unresolved)
    );
    // Migration counts as one implicit revision step.
    assert_eq!(c2.live_revision(), ProjectRevision(saved_rev.0 + 1));
    assert!(c2.live().validate_references().is_ok());
}

// ── PC-C4-05-03: oracle rejects ─────────────────────────────────────

fn identity(raw: u64, plugin: PluginRef) -> PluginInstanceIdentity {
    PluginInstanceIdentity {
        id: PluginInstanceId::new(raw),
        plugin,
    }
}

#[test]
fn oracle_rejects_dangling_entry() {
    let (mut c, strip) = strip_project();
    insert_processor(&mut c, strip, resolved_binding());

    let mut broken = c.live().clone();
    broken
        .plugin_instances
        .instances
        .insert(ProcessorId::new(0xdead), identity(2, plugin_ref()));
    broken.plugin_instances.watermark = 3;
    assert!(matches!(
        broken.validate_references(),
        Err(Error::NotFound(_))
    ));
}

#[test]
fn oracle_rejects_plugin_mismatch() {
    let (mut c, strip) = strip_project();
    let proc = insert_processor(&mut c, strip, resolved_binding());

    // Descriptor drift.
    let mut broken = c.live().clone();
    broken
        .plugin_instances
        .instances
        .get_mut(&proc)
        .unwrap()
        .plugin
        .descriptor = "vendor.other".into();
    assert!(broken.validate_references().is_err());

    // capability_revision drift — a different authored identity.
    let mut broken = c.live().clone();
    broken
        .plugin_instances
        .instances
        .get_mut(&proc)
        .unwrap()
        .plugin
        .capability_revision += 1;
    assert!(broken.validate_references().is_err());

    // Version drift.
    let mut broken = c.live().clone();
    broken
        .plugin_instances
        .instances
        .get_mut(&proc)
        .unwrap()
        .plugin
        .version = "9.9.9".into();
    assert!(broken.validate_references().is_err());
}

#[test]
fn oracle_rejects_duplicate_and_out_of_range_ids() {
    let (mut c, strip) = strip_project();
    let a = insert_processor(&mut c, strip, resolved_binding());
    let b = insert_processor(&mut c, strip, resolved_binding());

    // Two entries sharing one issued id.
    let mut broken = c.live().clone();
    broken.plugin_instances.instances.get_mut(&b).unwrap().id = PluginInstanceId::new(1);
    assert!(broken.validate_references().is_err());

    // Id at/past the watermark (never issued).
    let mut broken = c.live().clone();
    broken.plugin_instances.instances.get_mut(&a).unwrap().id = PluginInstanceId::new(3);
    assert!(broken.validate_references().is_err());

    // Id 0 is outside the issued range (ids start at 1).
    let mut broken = c.live().clone();
    broken.plugin_instances.instances.get_mut(&a).unwrap().id = PluginInstanceId::new(0);
    assert!(broken.validate_references().is_err());

    // Watermark below 1.
    let mut broken = c.live().clone();
    broken.plugin_instances.watermark = 0;
    assert!(broken.validate_references().is_err());
}

#[test]
fn oracle_rejects_missing_and_foreign_entries() {
    let (mut c, strip) = strip_project();
    let resolved = insert_processor(&mut c, strip, resolved_binding());
    let unresolved = insert_processor(&mut c, strip, unresolved_binding());

    // A Resolved processor without an entry.
    let mut broken = c.live().clone();
    broken.plugin_instances.instances.remove(&resolved);
    assert!(broken.validate_references().is_err());

    // An Unresolved processor carrying an entry.
    let mut broken = c.live().clone();
    broken
        .plugin_instances
        .instances
        .insert(unresolved, identity(5, plugin_ref()));
    broken.plugin_instances.watermark = 6;
    assert!(broken.validate_references().is_err());
}

// ── PC-C4-05-05: VCS apply synchronization ──────────────────────────
//
// Fixtures mirror vcs06_apply_matrix.rs: before/after snapshots of the
// same logical project, a ChangeSet, and `apply_selected` against the
// live controller. `snap_manual` marks the endpoint Complete so the
// apply gate reaches the entity writes (a live capture would record
// plugin state Unknown → NotApplicable first).

struct AllPresentProbe;

impl BlobProbe for AllPresentProbe {
    fn probe(&self, _digest: &str) -> BlobRead {
        BlobRead::Present(vec![0])
    }
}

struct AllPlugins;

impl PluginAvailability for AllPlugins {
    fn plugin_available(&self, _plugin: &PluginRef) -> bool {
        true
    }
}

fn snap_live(c: &ProjectController) -> ImmutableSnapshot {
    SnapshotResolver::new()
        .capture_live(
            c,
            c.live_revision(),
            &SnapshotCancel::new(),
            Some(&AllPresentProbe),
        )
        .unwrap()
}

fn snap_manual(project: Project, tag: &str) -> ImmutableSnapshot {
    let bytes = serde_json::to_vec(&project).unwrap();
    let mut material = tag.as_bytes().to_vec();
    material.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
    material.extend_from_slice(&bytes);
    ImmutableSnapshot {
        identity: SnapshotIdentity {
            project_id: project.project_id,
            source: SnapshotOrigin::Live,
            locator: FixedSnapshotLocator::Live {
                project_id: project.project_id,
                live_revision: ProjectRevision(0),
            },
            digest: ContentDigest::sha256(&material),
            schema_version: SCHEMA_VERSION,
            projection_version: SCHEMA_VERSION,
            capture_generation: 0,
        },
        origin: SnapshotOrigin::Live,
        projection: ReadOnlyProjectProjection::new(SCHEMA_VERSION, project),
        validation: SnapshotValidation::Valid,
        assets: AssetAvailabilityManifest::default(),
        opaque_state: Vec::new(),
        completeness: SnapshotCompleteness::Complete,
    }
}

fn spec_for(before: &ImmutableSnapshot, after: &ImmutableSnapshot) -> ComparisonSpec {
    ComparisonSpec::new(before.identity.clone(), after.identity.clone())
}

fn compare(before: &ImmutableSnapshot, after: &ImmutableSnapshot) -> ChangeSet {
    compare_snapshots(
        before,
        after,
        &spec_for(before, after),
        &SnapshotCancel::new(),
    )
    .unwrap()
}

struct Src {
    resolved: ResolvedChangeSet,
}

impl ChangeSetSource for Src {
    fn resolve(&self, id: &ContentDigest) -> Option<ResolvedChangeSet> {
        (id == &self.resolved.changeset.key.digest()).then(|| self.resolved.clone())
    }
}

fn sources_for(cs: &ChangeSet, before: &ImmutableSnapshot, after: &ImmutableSnapshot) -> Src {
    Src {
        resolved: ResolvedChangeSet {
            changeset: cs.clone(),
            before: before.clone(),
            after: after.clone(),
        },
    }
}

fn intent_for(
    c: &ProjectController,
    cs: &ChangeSet,
    after: &ImmutableSnapshot,
    changes: &[&SemanticChange],
) -> ApplyChangeIntent {
    let mut objects: Vec<SemanticObjectRef> = changes.iter().map(|ch| ch.object.clone()).collect();
    objects.push(changes[0].object.clone());
    let fp = target_fingerprint(c.live(), &objects).unwrap();
    ApplyChangeIntent {
        change_set_id: cs.key.digest(),
        source_snapshot_id: after.identity.digest,
        target_object: changes[0].object.clone(),
        expected_project_revision: c.live_revision(),
        selected_change_ids: changes.iter().map(|ch| ch.id.clone()).collect(),
        expected_target_fingerprint: fp,
        approval: ApplyApproval {
            ticket: "user-approved".into(),
        },
    }
}

/// Applies `changes` (typically the whole changeset) to `live` with an
/// all-plugins-available probe. Returns the receipt.
fn run_apply(
    c: &mut ProjectController,
    cs: &ChangeSet,
    before: &ImmutableSnapshot,
    after: &ImmutableSnapshot,
    changes: &[&SemanticChange],
) -> synvelis_project::apply::ApplyReceipt {
    let intent = intent_for(c, cs, after, changes);
    let src = sources_for(cs, before, after);
    let mut journal = InMemoryApplyJournal::new();
    let sources = ApplySources {
        changesets: &src,
        assets: Some(&AllPresentProbe),
        plugins: Some(&AllPlugins),
    };
    apply_selected(c, &intent, &sources, &mut journal, &SnapshotCancel::new())
        .expect("apply_selected")
}

/// Deterministic before/after pair: identical op sequences, the
/// after side adds one resolved processor. Returns both controllers
/// plus snapshots and the changeset.
fn deterministic_pair() -> (
    ProjectController,
    ProjectController,
    ImmutableSnapshot,
    ImmutableSnapshot,
    ChangeSet,
) {
    let mut before_c = controller();
    let track = add_track(&mut before_c, "vox");
    let strip = add_strip(&mut before_c, track);
    let before = snap_live(&before_c);

    let mut after_c = controller();
    let track2 = add_track(&mut after_c, "vox");
    assert_eq!(track, track2);
    let strip2 = add_strip(&mut after_c, track2);
    assert_eq!(strip, strip2);
    insert_processor(&mut after_c, strip2, resolved_binding());
    let after = snap_manual(after_c.live().clone(), "after");
    let cs = compare(&before, &after);
    (before_c, after_c, before, after, cs)
}

#[test]
fn apply_added_resolved_processor_allocates_identity() {
    let (mut live, _after_c, before, after, cs) = deterministic_pair();
    let changes: Vec<&SemanticChange> = cs.changes.iter().collect();
    let receipt = run_apply(&mut live, &cs, &before, &after, &changes);
    assert_eq!(receipt.outcome, ApplyOutcome::Applied);

    // The applied processor carries a freshly issued persisted
    // identity — the direct entity-write path synchronized the store.
    let proc = *live.live().routing.processors.keys().next().unwrap();
    assert_eq!(instance_id(&live, proc), 1);
    assert_eq!(live.live().plugin_instances.watermark, 2);
    assert!(live.live().validate_references().is_ok());

    // One undo reverts the whole apply — processor and identity entry
    // together (snapshot semantics).
    assert!(live.undo().unwrap());
    assert!(live.live().routing.processors.is_empty());
    assert!(live.live().plugin_instances.instances.is_empty());
}

#[test]
fn apply_modified_processor_same_plugin_keeps_identity() {
    // Before: one resolved processor (id 1 issued by the op).
    let mut before_c = controller();
    let track = add_track(&mut before_c, "vox");
    let strip = add_strip(&mut before_c, track);
    let proc = insert_processor(&mut before_c, strip, resolved_binding());
    assert_eq!(instance_id(&before_c, proc), 1);

    // After: identical state except the processor's display name —
    // a `Modified` change whose binding is untouched.
    let mut after_project = before_c.live().clone();
    after_project
        .routing
        .processors
        .get_mut(&proc)
        .unwrap()
        .name = "renamed".into();
    let before = snap_manual(before_c.live().clone(), "before");
    let after = snap_manual(after_project, "after");
    let cs = compare(&before, &after);
    let change = cs
        .changes
        .iter()
        .find(|ch| ch.object.kind == ObjectKind::Processor && ch.kind == ChangeKind::Modified)
        .expect("processor rename change");

    let mut live = before_c;
    let changes = vec![change];
    let receipt = run_apply(&mut live, &cs, &before, &after, &changes);
    assert_eq!(receipt.outcome, ApplyOutcome::Applied);
    assert_eq!(instance_id(&live, proc), 1);
    assert_eq!(live.live().plugin_instances.watermark, 2);
    assert!(live.live().validate_references().is_ok());
}

#[test]
fn apply_modified_processor_changed_plugin_allocates_new_identity() {
    let mut before_c = controller();
    let track = add_track(&mut before_c, "vox");
    let strip = add_strip(&mut before_c, track);
    let proc = insert_processor(&mut before_c, strip, resolved_binding());
    assert_eq!(instance_id(&before_c, proc), 1);

    // After: the binding's PluginRef drifts (capability_revision) —
    // a different authored identity, so apply must mint a new id.
    let mut after_project = before_c.live().clone();
    let after_binding = ProcessorBinding::Resolved(PluginRef {
        capability_revision: 4,
        ..plugin_ref()
    });
    after_project
        .routing
        .processors
        .get_mut(&proc)
        .unwrap()
        .binding = after_binding.clone();
    // Keep the after snapshot's own identity store consistent — the
    // after side was produced by ops in reality; here we simulate the
    // post-rebind state directly.
    after_project.plugin_instances.instances.insert(
        proc,
        identity(
            2,
            match &after_binding {
                ProcessorBinding::Resolved(p) => p.clone(),
                ProcessorBinding::Unresolved { .. } => unreachable!(),
            },
        ),
    );
    after_project.plugin_instances.watermark = 3;

    let before = snap_manual(before_c.live().clone(), "before");
    let after = snap_manual(after_project, "after");
    let cs = compare(&before, &after);
    let change = cs
        .changes
        .iter()
        .find(|ch| ch.object.kind == ObjectKind::Processor && ch.kind == ChangeKind::Modified)
        .expect("processor rebind change");

    let mut live = before_c;
    let changes = vec![change];
    let receipt = run_apply(&mut live, &cs, &before, &after, &changes);
    assert_eq!(receipt.outcome, ApplyOutcome::Applied);
    // New authored identity → new persisted id; the old id stays
    // retired (watermark only moves forward).
    assert_eq!(instance_id(&live, proc), 2);
    assert_eq!(live.live().plugin_instances.watermark, 3);
    assert_eq!(
        live.live().plugin_instances.instances[&proc].plugin,
        match &after_binding {
            ProcessorBinding::Resolved(p) => p.clone(),
            ProcessorBinding::Unresolved { .. } => unreachable!(),
        }
    );
    assert!(live.live().validate_references().is_ok());
}
