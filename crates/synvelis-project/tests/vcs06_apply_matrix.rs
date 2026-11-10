//! VCS-06 (issue #97): apply fixture rows not covered by
//! `vcs05_apply.rs` — multi-change atomic apply with a single undo,
//! plugin-availability rejection, cooperative cancel, unsupported
//! singleton fields, stale-target `TargetMissing`/`TargetConflict`,
//! and on-disk `DirectoryApplyJournal` crash/replay persistence.
//!
//! Headless only: real engine/GUI/provider lanes stay `NotRun`
//! (see `evidence/reports/vcs-06-fixture-matrix-ledger.md`).

use std::path::PathBuf;

use synvelis_project::apply::{
    ApplyApproval, ApplyChangeIntent, ApplyError, ApplyJournal, ApplyOutcome, ApplyReceipt,
    ApplySources, ChangeSetSource, DirectoryApplyJournal, InMemoryApplyJournal, PluginAvailability,
    ResolvedChangeSet, apply_selected, target_fingerprint,
};
use synvelis_project::controller::{Command, ProjectController, ProjectRevision};
use synvelis_project::diff::{
    ChangeKind, ChangeSet, ComparisonSpec, ObjectKind, SemanticChange, SemanticObjectRef,
    compare_snapshots,
};
use synvelis_project::id::{CounterIdGenerator, ProjectId, TrackId};
use synvelis_project::ops::ProjectOp;
use synvelis_project::project::{Project, Track};
use synvelis_project::routing::{PluginRef, ProcessorBinding, StripOwner};
use synvelis_project::snapshot::{
    AssetAvailabilityManifest, BlobProbe, BlobRead, ContentDigest, FixedSnapshotLocator,
    ImmutableSnapshot, ReadOnlyProjectProjection, SnapshotCancel, SnapshotCompleteness,
    SnapshotIdentity, SnapshotOrigin, SnapshotResolver, SnapshotValidation,
};
use synvelis_project::store::SCHEMA_VERSION;

// ── fixtures (mirrors vcs05_apply.rs) ─────────────────────────────

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "fixture"),
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

struct AllPresentProbe;

impl BlobProbe for AllPresentProbe {
    fn probe(&self, _digest: &str) -> BlobRead {
        BlobRead::Present(vec![0])
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

fn base_project() -> ProjectController {
    let mut c = controller();
    apply_op(
        &mut c,
        ProjectOp::SetProjectName {
            name: "song".into(),
        },
    );
    apply_op(
        &mut c,
        ProjectOp::AddTrack {
            name: "drums".into(),
        },
    );
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddLane { track_id: track });
    c
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

fn run_apply(
    c: &mut ProjectController,
    intent: &ApplyChangeIntent,
    src: &Src,
    journal: &mut dyn ApplyJournal,
) -> Result<ApplyReceipt, ApplyError> {
    let sources = ApplySources {
        changesets: src,
        assets: Some(&AllPresentProbe),
        plugins: None,
    };
    apply_selected(c, intent, &sources, journal, &SnapshotCancel::new())
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-vcs06-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ── VCS-APPLY-01: multi-change apply → one undo step ──────────────

#[test]
fn multi_change_apply_commits_as_single_undo_step() {
    let before_c = base_project();
    let before = snap_live(&before_c);
    let mut after_c = base_project();
    apply_op(&mut after_c, ProjectOp::AddTrack { name: "vox".into() });
    apply_op(
        &mut after_c,
        ProjectOp::AddTrack {
            name: "keys".into(),
        },
    );
    let after = snap_live(&after_c);
    let cs = compare(&before, &after);

    let added: Vec<&SemanticChange> = cs
        .changes
        .iter()
        .filter(|ch| ch.object.kind == ObjectKind::Track && ch.kind == ChangeKind::Added)
        .collect();
    assert_eq!(added.len(), 2);

    let intent = intent_for(&before_c, &cs, &after, &added);
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();

    let mut live = before_c;
    let receipt = run_apply(&mut live, &intent, &src, &mut journal).unwrap();

    assert_eq!(receipt.outcome, ApplyOutcome::Applied);
    assert_eq!(receipt.applied_change_ids.len(), 2);
    assert!(receipt.effects.single_undo_step);
    assert_eq!(live.live().tracks.len(), 3);

    // Exactly one undo reverts BOTH added tracks.
    assert!(live.undo().unwrap());
    assert_eq!(live.live().tracks.len(), 1);
    assert!(live.redo().unwrap());
    assert_eq!(live.live().tracks.len(), 3);
}

// ── VCS-APPLY-02: cancel + unsupported field + stale targets ──────

#[test]
fn cancelled_intent_fails_closed_before_mutation() {
    let mut c = base_project();
    let before = snap_live(&c);
    let mut after_c = base_project();
    apply_op(&mut after_c, ProjectOp::AddTrack { name: "vox".into() });
    let after = snap_live(&after_c);
    let cs = compare(&before, &after);
    let change = cs
        .changes
        .iter()
        .find(|ch| ch.object.kind == ObjectKind::Track && ch.kind == ChangeKind::Added)
        .unwrap();
    let intent = intent_for(&c, &cs, &after, &[change]);
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();

    let sources = ApplySources {
        changesets: &src,
        assets: Some(&AllPresentProbe),
        plugins: None,
    };
    let cancel = SnapshotCancel::new();
    cancel.cancel();
    let err = apply_selected(&mut c, &intent, &sources, &mut journal, &cancel).unwrap_err();
    assert!(matches!(err, ApplyError::Cancelled));
    assert_eq!(c.live().tracks.len(), 1);
}

#[test]
fn unsupported_singleton_field_fails_closed() {
    // Project singleton allows only `name`; a ppqn drift must reject.
    let c = base_project();
    let before = snap_live(&c);
    let mut project = c.live().clone();
    project.ppqn.0 = 480;
    let after = snap_manual(project, "after");
    let cs = compare(&before, &after);
    let change = cs
        .changes
        .iter()
        .find(|ch| ch.object.kind == ObjectKind::Project && ch.kind == ChangeKind::Modified)
        .expect("project ppqn change");
    assert!(change.evidence.fields.iter().any(|f| f.field == "ppqn"));

    let intent = intent_for(&c, &cs, &after, &[change]);
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();
    let mut live = c;
    let err = run_apply(&mut live, &intent, &src, &mut journal).unwrap_err();
    assert!(matches!(err, ApplyError::UnsupportedField { .. }));
    assert_eq!(live.live().ppqn.0, 960);
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

#[test]
fn modified_change_on_absent_target_is_target_missing() {
    // before/after snapshots describe a track rename; the live project
    // never had that track — CAS fingerprints both encode `null`, so
    // planning must fail closed with TargetMissing.
    let ghost = TrackId::new(0x4242);
    let mut p_before = Project::new(ProjectId::new(1), "fixture");
    p_before.tracks.insert(
        ghost,
        Track {
            id: ghost,
            name: "old".into(),
            lane_ids: vec![],
        },
    );
    let mut p_after = p_before.clone();
    p_after.tracks.get_mut(&ghost).unwrap().name = "new".into();
    let before = snap_manual(p_before, "before");
    let after = snap_manual(p_after, "after");
    let cs = compare(&before, &after);
    let change = cs
        .changes
        .iter()
        .find(|ch| ch.object.kind == ObjectKind::Track && ch.kind == ChangeKind::Modified)
        .expect("track rename change");

    // Live controller: same project id, no tracks at all.
    let mut live = controller();
    apply_op(
        &mut live,
        ProjectOp::SetProjectName {
            name: "fixture".into(),
        },
    );
    let intent = intent_for(&live, &cs, &after, &[change]);
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();
    let err = run_apply(&mut live, &intent, &src, &mut journal).unwrap_err();
    assert!(matches!(err, ApplyError::TargetMissing { .. }));
    assert!(live.live().tracks.is_empty());
}

#[test]
fn added_change_with_live_id_collision_is_target_conflict() {
    // Live already owns the stable ID the changeset wants to add —
    // fingerprint matches (computed over the live entity), planning
    // must still refuse the identity collision.
    let mut live = controller();
    apply_op(
        &mut live,
        ProjectOp::SetProjectName {
            name: "fixture".into(),
        },
    );
    apply_op(
        &mut live,
        ProjectOp::AddTrack {
            name: "existing".into(),
        },
    );
    let tid = *live.live().tracks.keys().next().unwrap();

    let p_before = Project::new(ProjectId::new(1), "fixture");
    let mut p_after = p_before.clone();
    p_after.tracks.insert(
        tid,
        Track {
            id: tid,
            name: "intruder".into(),
            lane_ids: vec![],
        },
    );
    let before = snap_manual(p_before, "before");
    let after = snap_manual(p_after, "after");
    let cs = compare(&before, &after);
    let change = cs
        .changes
        .iter()
        .find(|ch| ch.object.kind == ObjectKind::Track && ch.kind == ChangeKind::Added)
        .expect("track add change");

    let intent = intent_for(&live, &cs, &after, &[change]);
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();
    let err = run_apply(&mut live, &intent, &src, &mut journal).unwrap_err();
    assert!(matches!(err, ApplyError::TargetConflict { .. }));
    assert_eq!(live.live().tracks.len(), 1);
    assert_eq!(live.live().tracks[&tid].name, "existing");
}

// ── VCS-APPLY-03: plugin availability ─────────────────────────────

struct NoPlugins;

impl PluginAvailability for NoPlugins {
    fn plugin_available(&self, _plugin: &PluginRef) -> bool {
        false
    }
}

#[test]
fn unavailable_plugin_binding_fails_closed() {
    let before_c = base_project();
    let before = snap_live(&before_c);
    let mut after_c = base_project();
    let track = *after_c.live().tracks.keys().next().unwrap();
    apply_op(&mut after_c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = after_c
        .live()
        .routing
        .strips
        .values()
        .find(|s| s.owner == StripOwner::Track(track))
        .map(|s| s.id)
        .expect("track strip");
    apply_op(
        &mut after_c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "fx".into(),
            binding: ProcessorBinding::Resolved(PluginRef {
                descriptor: "fixture-fx".into(),
                version: "1.0".into(),
                capability_revision: 1,
            }),
            sidechain_input: false,
        },
    );
    // `snap_manual` marks the endpoint Complete with no opaque state —
    // a live capture would (correctly) record the new processor's
    // plugin state as `Unknown` → `Partial` → `NotApplicable` before
    // the plugin gate is ever reached. This fixture isolates the
    // plugin-availability check itself.
    let after = snap_manual(after_c.live().clone(), "after");
    let cs = compare(&before, &after);
    // The processor delta spans Processor + SemanticPort (+chain)
    // changes — select the whole addition set so port wiring stays
    // consistent; plugin preflight must still reject the binding.
    let changes: Vec<&SemanticChange> = cs.changes.iter().collect();
    assert!(
        changes
            .iter()
            .any(|ch| ch.object.kind == ObjectKind::Processor && ch.kind == ChangeKind::Added),
        "fixture needs a Processor Added change: {cs:?}"
    );

    let intent = intent_for(&before_c, &cs, &after, &changes);
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();
    let sources = ApplySources {
        changesets: &src,
        assets: Some(&AllPresentProbe),
        plugins: Some(&NoPlugins),
    };
    let mut live = before_c;
    let err = apply_selected(
        &mut live,
        &intent,
        &sources,
        &mut journal,
        &SnapshotCancel::new(),
    )
    .unwrap_err();
    assert!(matches!(err, ApplyError::MissingPlugin { .. }), "{err:?}");
    assert!(live.live().routing.processors.is_empty());
}

#[test]
fn processor_changeset_with_unknown_opaque_state_is_not_applicable() {
    // The honest counterpart: a *live* capture of the same
    // processor-bearing after-state records plugin state as Unknown
    // → Partial completeness → the apply gate refuses before any
    // plugin check. Unknown is never promoted to applicable.
    let before_c = base_project();
    let before = snap_live(&before_c);
    let mut after_c = base_project();
    let track = *after_c.live().tracks.keys().next().unwrap();
    apply_op(&mut after_c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = after_c
        .live()
        .routing
        .strips
        .values()
        .find(|s| s.owner == StripOwner::Track(track))
        .map(|s| s.id)
        .expect("track strip");
    apply_op(
        &mut after_c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "fx".into(),
            binding: ProcessorBinding::Resolved(PluginRef {
                descriptor: "fixture-fx".into(),
                version: "1.0".into(),
                capability_revision: 1,
            }),
            sidechain_input: false,
        },
    );
    let after = snap_live(&after_c);
    let cs = compare(&before, &after);
    assert_eq!(
        cs.completeness,
        synvelis_project::diff::ComparisonCompleteness::Partial
    );
    let changes: Vec<&SemanticChange> = cs.changes.iter().collect();
    let intent = intent_for(&before_c, &cs, &after, &changes);
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();
    let mut live = before_c;
    let err = run_apply(&mut live, &intent, &src, &mut journal).unwrap_err();
    assert!(matches!(err, ApplyError::NotApplicable { .. }), "{err:?}");
    assert!(live.live().routing.processors.is_empty());
}

// ── VCS-APPLY-04: on-disk journal crash/replay ────────────────────

#[test]
fn directory_journal_survives_restart_and_replays() {
    let mut c = base_project();
    let before = snap_live(&c);
    let mut after_c = base_project();
    apply_op(&mut after_c, ProjectOp::AddTrack { name: "vox".into() });
    let after = snap_live(&after_c);
    let cs = compare(&before, &after);
    let change = cs
        .changes
        .iter()
        .find(|ch| ch.object.kind == ObjectKind::Track && ch.kind == ChangeKind::Added)
        .unwrap();
    let intent = intent_for(&c, &cs, &after, &[change]);
    let src = sources_for(&cs, &before, &after);

    let journal_dir = tmp_dir("journal");
    {
        let mut journal = DirectoryApplyJournal::new(&journal_dir);
        let receipt = run_apply(&mut c, &intent, &src, &mut journal).unwrap();
        assert_eq!(receipt.outcome, ApplyOutcome::Applied);
        assert_eq!(c.live().tracks.len(), 2);
    }
    // "Restart": a fresh journal instance over the same directory must
    // find the persisted Applied record and replay — never re-apply.
    {
        let mut journal = DirectoryApplyJournal::new(&journal_dir);
        let receipt = run_apply(&mut c, &intent, &src, &mut journal).unwrap();
        assert_eq!(receipt.outcome, ApplyOutcome::Replayed);
        assert_eq!(c.live().tracks.len(), 2);
    }
    let _ = std::fs::remove_dir_all(&journal_dir);
}
