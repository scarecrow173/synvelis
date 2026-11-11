//! VCS-02 semantic diff engine fixtures (synvelis#93).
//!
//! Headless verification of the `ChangeSet`: semantic change
//! kinds, typed identity/lineage ambiguity, and the `Unknown` tri-state.
//! Real OS / real Git remote / GUI / real plugin host are `NotRun`.

use std::collections::BTreeSet;
use std::sync::Arc;

use synvelis_project::control::{
    AutomationPoint, AutomationRegion, AutomationTimeScope, BeatPoint, BeatRange, ConflictPolicy,
    ControlOwner, Interpolation, ParamMapping, ParameterRole, ValueKind,
};
use synvelis_project::controller::{Command, ProjectController, ProjectRevision};
use synvelis_project::diff::{
    AmbiguityReason, ChangeKind, ChangeSet, ChangeSetCache, ComparisonBudget,
    ComparisonCompleteness, ComparisonDomain, ComparisonDomains, ComparisonEquality,
    ComparisonIssue, ComparisonSpec, DiffError, ObjectKind, SemanticChange, SemanticTimeRange,
    compare_snapshots,
};
use synvelis_project::id::{
    AssetId, ClipId, CounterIdGenerator, LaneId, NoteId, ProcessorId, ProjectId,
    RoutingConnectionId, SemanticPortId, SourceId, TrackId,
};
use synvelis_project::monitoring_data::{
    ArmState, MonitorChannelMap, MonitorState, PunchMarks, TrackMonitorDecl,
};
use synvelis_project::ops::ProjectOp;
use synvelis_project::placement::{
    ClipExtent, ClipPlacement, ClipPlayback, PlaybackMapping, TimeAnchor,
};
use synvelis_project::project::{Clip, Project, Track};
use synvelis_project::range::SourceRange;
use synvelis_project::routing::{ConnectionKind, ProcessorBinding, RoutingConnection};
use synvelis_project::snapshot::{
    AssetAvailabilityManifest, AssetResolution, BlobProbe, BlobRead, ContentDigest, ContentStatus,
    FixedSnapshotLocator, ImmutableSnapshot, OpaqueStateRef, OpaqueStateStatus,
    ReadOnlyProjectProjection, SnapshotCancel, SnapshotCompleteness, SnapshotIdentity,
    SnapshotOrigin, SnapshotResolver, SnapshotValidation,
};
use synvelis_project::store::SCHEMA_VERSION;
use synvelis_project::time::{MusicalDuration, MusicalTick, SourceFrame};

// ── helpers ────────────────────────────────────────────────────────

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "fixture"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply(c: &mut ProjectController, op: ProjectOp) {
    let rev = c.live_revision();
    c.apply(Command {
        expected_revision: rev,
        op,
    })
    .unwrap();
}

/// Reports every probed digest as present — keeps live captures free of
/// `UnresolvedSource` noise so `Equal` is reachable in fixtures.
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

fn changes_of(cs: &ChangeSet, kind: ObjectKind) -> Vec<&SemanticChange> {
    cs.changes
        .iter()
        .filter(|c| c.object.kind == kind)
        .collect()
}

fn has_kind(cs: &ChangeSet, kind: ObjectKind, ck: ChangeKind) -> bool {
    cs.changes
        .iter()
        .any(|c| c.object.kind == kind && c.kind == ck)
}

/// Valid-format fixture digest (`sha256:` + 64 lowercase hex) — the
/// VCS-01 manifest probe marks malformed digests `Unknown`, which
/// would (correctly) poison every comparison with `UnresolvedSource`.
fn sha(c: char) -> String {
    format!("sha256:{}", c.to_string().repeat(64))
}

fn fields_of(change: &SemanticChange) -> Vec<&str> {
    change
        .evidence
        .fields
        .iter()
        .map(|f| f.field.as_str())
        .collect()
}

fn clip_placement(anchor_tick: i64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Musical(MusicalTick::new(anchor_tick)),
        extent: ClipExtent::Musical(MusicalDuration::new(8 * 960).unwrap()),
    }
}

fn clip_playback() -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::ZERO,
            SourceFrame::new(1_000_000).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

/// Base project: one track/lane, one asset+source, one clip, one note.
fn base_project() -> (ProjectController, TrackId, LaneId, SourceId, ClipId, NoteId) {
    let mut c = controller();
    apply(
        &mut c,
        ProjectOp::SetProjectName {
            name: "song".into(),
        },
    );
    apply(
        &mut c,
        ProjectOp::AddTrack {
            name: "drums".into(),
        },
    );
    let track = *c.live().tracks.keys().next().unwrap();
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply(&mut c, ProjectOp::RegisterAsset { digest: sha('a') });
    let asset = *c.live().assets.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: sha('1'),
        },
    );
    let source = *c.live().sources.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: clip_placement(0),
            playback: clip_playback(),
        },
    );
    let clip = *c.live().clips.keys().next_back().unwrap();
    apply(
        &mut c,
        ProjectOp::AddNote {
            clip,
            key: 60,
            start: BeatPoint::new(0, 1).unwrap(),
            len: BeatPoint::new(1, 1).unwrap(),
        },
    );
    let note = *c.live().notes.keys().next_back().unwrap();
    (c, track, lane, source, clip, note)
}

/// Builds an [`ImmutableSnapshot`] directly from a [`Project`] — used
/// for crafted states ops cannot produce (identity collisions, owner
/// conflicts, degraded manifests, old schema versions). The digest
/// material mixes `tag` + `schema_version` + serialized project so two
/// snapshots with identical project bytes but different schema (or a
/// crafted marker) never take the digest-equal fast path.
fn snap_manual(project: Project, schema_version: u32, tag: &str) -> ImmutableSnapshot {
    let bytes = serde_json::to_vec(&project).unwrap();
    let mut material = tag.as_bytes().to_vec();
    material.extend_from_slice(&schema_version.to_le_bytes());
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
            schema_version,
            projection_version: SCHEMA_VERSION,
            capture_generation: 0,
        },
        origin: SnapshotOrigin::Live,
        projection: ReadOnlyProjectProjection::new(schema_version, project),
        validation: SnapshotValidation::Valid,
        assets: AssetAvailabilityManifest::default(),
        opaque_state: Vec::new(),
        completeness: SnapshotCompleteness::Complete,
    }
}

fn manual_pair(before: Project, after: Project) -> (ImmutableSnapshot, ImmutableSnapshot) {
    (
        snap_manual(before, SCHEMA_VERSION, "before"),
        snap_manual(after, SCHEMA_VERSION, "after"),
    )
}

// ── VCS-DIFF-01: semantic classification ───────────────────────────

#[test]
fn aa_is_equal_and_comparison_is_deterministic() {
    let (c, ..) = base_project();
    let a = snap_live(&c);
    // Same snapshot object twice → strict AA.
    let cs = compare_snapshots(&a, &a, &spec_for(&a, &a), &SnapshotCancel::new()).unwrap();
    assert_eq!(cs.overall, ComparisonEquality::Equal);
    assert_eq!(cs.changes.len(), 0);
    assert_eq!(cs.unresolved.len(), 0);
    assert_eq!(cs.completeness, ComparisonCompleteness::Complete);

    // Two captures of identical content → also Equal, and repeated
    // comparisons are bit-identical.
    let b = snap_live(&c);
    let spec = spec_for(&a, &b);
    let cs1 = compare_snapshots(&a, &b, &spec, &SnapshotCancel::new()).unwrap();
    let cs2 = compare_snapshots(&a, &b, &spec, &SnapshotCancel::new()).unwrap();
    assert_eq!(cs1, cs2);
    assert_eq!(cs1.overall, ComparisonEquality::Equal);
}

#[test]
fn ab_ba_reverses_directional_kinds() {
    let mut c = controller();
    apply(&mut c, ProjectOp::AddTrack { name: "a".into() });
    apply(&mut c, ProjectOp::AddTrack { name: "b".into() });
    let before = snap_live(&c);
    let doomed = *c.live().tracks.keys().next().unwrap();
    apply(&mut c, ProjectOp::RemoveTrack { track_id: doomed });
    let after = snap_live(&c);

    let ab = compare(&before, &after);
    assert!(has_kind(&ab, ObjectKind::Track, ChangeKind::Removed));
    let ba = compare(&after, &before);
    assert!(has_kind(&ba, ObjectKind::Track, ChangeKind::Added));
    assert!(!has_kind(&ba, ObjectKind::Track, ChangeKind::Removed));
}

#[test]
fn track_name_change_is_modified() {
    let (c, track, ..) = base_project();
    let before = snap_live(&c);
    // No rename op exists yet — mutate the persisted field directly.
    let mut project = c.live().clone();
    project.tracks.get_mut(&track).unwrap().name = "renamed".into();
    let after = snap_manual(project, SCHEMA_VERSION, "after");

    let cs = compare(&before, &after);
    let hits = changes_of(&cs, ObjectKind::Track);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Modified);
    assert!(fields_of(hits[0]).contains(&"name"));
    assert_eq!(cs.overall, ComparisonEquality::Different);
}

#[test]
fn clip_move_is_moved_and_trim_is_modified() {
    let (mut c, _, _, _, clip, note) = base_project();
    let before = snap_live(&c);

    // Move: anchor change only → Moved, never Modified/Added/Removed.
    apply(
        &mut c,
        ProjectOp::MoveClip {
            clip_id: clip,
            anchor: TimeAnchor::Musical(MusicalTick::new(960)),
        },
    );
    let moved = snap_live(&c);
    let cs = compare(&before, &moved);
    let clip_changes = changes_of(&cs, ObjectKind::Clip);
    assert_eq!(clip_changes.len(), 1);
    assert_eq!(clip_changes[0].kind, ChangeKind::Moved);
    assert!(
        clip_changes[0]
            .evidence
            .fields
            .iter()
            .any(|f| f.field.starts_with("placement.anchor"))
    );
    // Clip-relative notes must not appear as delete/add.
    assert_eq!(changes_of(&cs, ObjectKind::Note).len(), 0);
    assert!(
        cs.changes
            .iter()
            .all(|ch| !ch.object.stable_id.contains(&note.to_tagged()))
    );

    // Trim: extent change → Modified, never Moved.
    apply(
        &mut c,
        ProjectOp::SetClipExtent {
            clip_id: clip,
            extent: ClipExtent::Musical(MusicalDuration::new(4 * 960).unwrap()),
        },
    );
    let trimmed = snap_live(&c);
    let cs2 = compare(&moved, &trimmed);
    let hits = changes_of(&cs2, ObjectKind::Clip);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Modified);
    assert!(
        hits[0]
            .evidence
            .fields
            .iter()
            .any(|f| f.field.starts_with("placement.extent"))
    );
}

#[test]
fn clip_move_plus_trim_is_two_changes() {
    let (mut c, _, _, _, clip, _) = base_project();
    let before = snap_live(&c);
    apply(
        &mut c,
        ProjectOp::MoveClip {
            clip_id: clip,
            anchor: TimeAnchor::Musical(MusicalTick::new(480)),
        },
    );
    apply(
        &mut c,
        ProjectOp::SetClipExtent {
            clip_id: clip,
            extent: ClipExtent::Musical(MusicalDuration::new(4 * 960).unwrap()),
        },
    );
    let after = snap_live(&c);
    let cs = compare(&before, &after);
    let kinds: Vec<ChangeKind> = changes_of(&cs, ObjectKind::Clip)
        .iter()
        .map(|c| c.kind)
        .collect();
    assert!(kinds.contains(&ChangeKind::Moved));
    assert!(kinds.contains(&ChangeKind::Modified));
}

#[test]
fn clip_source_swap_is_replaced() {
    let (mut c, _, _, _, clip, _) = base_project();
    let before = snap_live(&c);
    apply(&mut c, ProjectOp::RegisterAsset { digest: sha('b') });
    let asset2 = *c.live().assets.keys().next_back().unwrap();
    apply(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset2,
            content_digest: sha('2'),
        },
    );
    let source2 = *c.live().sources.keys().next_back().unwrap();
    let mut project = c.live().clone();
    project.clips.get_mut(&clip).unwrap().source_id = source2;
    let after = snap_manual(project, SCHEMA_VERSION, "after");

    let cs = compare(&before, &after);
    let hits = changes_of(&cs, ObjectKind::Clip);
    assert!(
        hits.iter()
            .any(|ch| ch.kind == ChangeKind::Replaced && fields_of(ch).contains(&"source_id"))
    );
    // The newly registered source appears as Added media content.
    assert!(has_kind(&cs, ObjectKind::MediaSource, ChangeKind::Added));
}

#[test]
fn clip_duplication_is_added_without_lane_noise() {
    let (mut c, _, lane, source, _, _) = base_project();
    let before = snap_live(&c);
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: clip_placement(960),
            playback: clip_playback(),
        },
    );
    let after = snap_live(&c);
    let cs = compare(&before, &after);
    assert_eq!(changes_of(&cs, ObjectKind::Clip).len(), 1);
    assert_eq!(changes_of(&cs, ObjectKind::Clip)[0].kind, ChangeKind::Added);
    // Lane clip_ids membership changed but must not double-report.
    assert_eq!(changes_of(&cs, ObjectKind::Lane).len(), 0);
}

#[test]
fn processor_reorder_is_reordered() {
    let mut c = controller();
    let before = snap_live(&c);
    let strip = *c.live().routing.strips.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "p1".into(),
            binding: ProcessorBinding::Unresolved {
                descriptor: "fixture".into(),
            },
            sidechain_input: false,
        },
    );
    apply(
        &mut c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 1,
            name: "p2".into(),
            binding: ProcessorBinding::Unresolved {
                descriptor: "fixture".into(),
            },
            sidechain_input: false,
        },
    );
    // `mid` already contains both processors — the reorder diff must
    // not re-report them as Added.
    let mid = snap_live(&c);
    let p1 = *c.live().routing.processors.keys().next().unwrap();
    let chain = c.live().routing.strips[&strip].chain;
    apply(
        &mut c,
        ProjectOp::ReorderProcessor {
            chain_id: chain,
            processor_id: p1,
            new_index: 1,
        },
    );
    let after = snap_live(&c);
    let cs = compare(&mid, &after);
    assert!(has_kind(
        &cs,
        ObjectKind::ProcessorChain,
        ChangeKind::Reordered
    ));
    // Same processor set on both sides → no Added/Removed/Moved noise.
    assert_eq!(changes_of(&cs, ObjectKind::Processor).len(), 0);

    // Versus the pre-insert snapshot the processors are Added content.
    let cs_added = compare(&before, &mid);
    assert!(has_kind(
        &cs_added,
        ObjectKind::Processor,
        ChangeKind::Added
    ));
}

#[test]
fn existing_connection_rewire_is_reconnected() {
    let mut c = controller();
    // Seed an identical connection on both sides, then rewire `after`.
    let ports: Vec<SemanticPortId> = c.live().routing.ports.keys().copied().collect();
    assert!(ports.len() >= 2);
    let conn_id = RoutingConnectionId::new(0xbeef);
    let new_destination = ports
        .iter()
        .copied()
        .find(|p| *p != ports[1])
        .unwrap_or(ports[0]);
    let mut before = c.live().clone();
    before.routing.connections.insert(
        conn_id,
        RoutingConnection {
            id: conn_id,
            source: ports[0],
            destination: ports[1],
            kind: ConnectionKind::Main,
            enabled: true,
            channel_map: None,
        },
    );
    let mut after = before.clone();
    after
        .routing
        .connections
        .get_mut(&conn_id)
        .unwrap()
        .destination = new_destination;
    let _ = &mut c;
    let (b, a) = manual_pair(before, after);
    let cs = compare(&b, &a);
    let hits = changes_of(&cs, ObjectKind::RoutingConnection);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Reconnected);
    assert!(fields_of(hits[0]).contains(&"destination"));
}

#[test]
fn connection_add_remove_is_added_removed() {
    let mut c = controller();
    let ports: Vec<SemanticPortId> = c.live().routing.ports.keys().copied().collect();
    let conn_id = RoutingConnectionId::new(0xf00d);
    let mut after = c.live().clone();
    after.routing.connections.insert(
        conn_id,
        RoutingConnection {
            id: conn_id,
            source: ports[0],
            destination: ports[1],
            kind: ConnectionKind::Main,
            enabled: true,
            channel_map: None,
        },
    );
    let _ = &mut c;
    let (b, a) = manual_pair(c.live().clone(), after);
    let ab = compare(&b, &a);
    assert!(has_kind(
        &ab,
        ObjectKind::RoutingConnection,
        ChangeKind::Added
    ));
    let ba = compare(&a, &b);
    assert!(has_kind(
        &ba,
        ObjectKind::RoutingConnection,
        ChangeKind::Removed
    ));
}

#[test]
fn automation_region_edit_is_modified() {
    let mut c = controller();
    apply(
        &mut c,
        ProjectOp::AddParameterTarget {
            name: "gain".into(),
            owner: ControlOwner::Project,
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    let target = *c.live().control.targets.keys().next_back().unwrap();
    apply(&mut c, ProjectOp::AddAutomationLane { target });
    let lane = *c.live().control.lanes.keys().next_back().unwrap();
    apply(
        &mut c,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: AutomationRegion {
                range: BeatRange::try_new(
                    BeatPoint::new(0, 1).unwrap(),
                    BeatPoint::new(4, 1).unwrap(),
                )
                .unwrap(),
                interpolation: Interpolation::Step,
                points: vec![AutomationPoint {
                    pos: BeatPoint::new(0, 1).unwrap(),
                    value: 0.5,
                }],
            },
            policy: ConflictPolicy::Reject,
        },
    );
    let before = snap_live(&c);
    // Edit the stored point value directly.
    let mut project = c.live().clone();
    project.control.lanes.get_mut(&lane).unwrap().regions[0].points[0].value = 0.9;
    let after = snap_manual(project, SCHEMA_VERSION, "after");

    let cs = compare(&before, &after);
    let hits = changes_of(&cs, ObjectKind::AutomationLane);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Modified);
    assert!(fields_of(hits[0]).contains(&"regions"));
}

#[test]
fn asset_content_swap_is_replaced() {
    let (c, ..) = base_project();
    let before = snap_live(&c);
    let asset = *c.live().assets.keys().next().unwrap();
    let mut project = c.live().clone();
    project.assets.get_mut(&asset).unwrap().digest = sha('f');
    let after = snap_manual(project, SCHEMA_VERSION, "after");

    let cs = compare(&before, &after);
    let hits = changes_of(&cs, ObjectKind::Asset);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Replaced);
}

// ── VCS-DIFF-06..07: Unknown tri-state ─────────────────────────────

#[test]
fn missing_asset_blocks_equal_promotion() {
    let (c, ..) = base_project();
    let complete = snap_live(&c);
    let mut degraded = snap_manual(c.live().clone(), SCHEMA_VERSION, "degraded");
    degraded.assets.entries.push(AssetResolution {
        asset_id: AssetId::new(0xdead),
        digest: sha('e'),
        status: ContentStatus::Missing,
    });
    degraded.completeness = SnapshotCompleteness::Partial;
    degraded.validation = SnapshotValidation::Degraded;

    // Identical project content, but the manifest reports a missing
    // asset → Unknown, never Equal.
    let cs = compare(&complete, &degraded);
    assert_eq!(cs.changes.len(), 0);
    assert!(matches!(cs.overall, ComparisonEquality::Unknown(_)));
    assert_eq!(cs.completeness, ComparisonCompleteness::Partial);
    assert!(cs.unresolved.iter().any(|i| matches!(
        i,
        ComparisonIssue::MissingAsset {
            status: ContentStatus::Missing,
            ..
        }
    )));

    // AA on the degraded snapshot is also not Equal.
    let aa = compare_snapshots(
        &degraded,
        &degraded,
        &spec_for(&degraded, &degraded),
        &SnapshotCancel::new(),
    )
    .unwrap();
    assert!(matches!(aa.overall, ComparisonEquality::Unknown(_)));
}

#[test]
fn unknown_asset_status_is_unresolved_source() {
    let (c, ..) = base_project();
    let mut snap = snap_manual(c.live().clone(), SCHEMA_VERSION, "s");
    snap.assets.entries.push(AssetResolution {
        asset_id: AssetId::new(0xbeef),
        digest: sha('d'),
        status: ContentStatus::Unknown,
    });
    snap.completeness = SnapshotCompleteness::Partial;
    let cs = compare(&snap, &snap);
    assert!(
        cs.unresolved
            .iter()
            .any(|i| matches!(i, ComparisonIssue::UnresolvedSource { .. }))
    );
    assert!(matches!(cs.overall, ComparisonEquality::Unknown(_)));
}

#[test]
fn unknown_opaque_state_blocks_equal() {
    let (c, ..) = base_project();
    let mut snap = snap_manual(c.live().clone(), SCHEMA_VERSION, "s");
    snap.opaque_state.push(OpaqueStateRef {
        processor_id: ProcessorId::new(7),
        descriptor: None,
        status: OpaqueStateStatus::Unknown,
    });
    snap.completeness = SnapshotCompleteness::Partial;
    let cs = compare(&snap, &snap);
    assert!(cs.unresolved.iter().any(|i| matches!(
        i,
        ComparisonIssue::MissingOpaqueState {
            status: OpaqueStateStatus::Unknown,
            ..
        }
    )));
    assert!(matches!(cs.overall, ComparisonEquality::Unknown(_)));
}

#[test]
fn pre_v6_snapshot_marks_routing_partial_projection() {
    let (c, ..) = base_project();
    let current = snap_manual(c.live().clone(), SCHEMA_VERSION, "cur");
    // A v5 snapshot: routing is a migration projection, not authored.
    let old = snap_manual(c.live().clone(), 5, "old");
    let cs = compare(&old, &current);
    assert!(cs.unresolved.iter().any(|i| matches!(
        i,
        ComparisonIssue::PartialProjection {
            domain: ComparisonDomain::Routing,
            ..
        }
    )));
    // No fabricated routing entity diffs.
    assert_eq!(changes_of(&cs, ObjectKind::Bus).len(), 0);
    assert_eq!(changes_of(&cs, ObjectKind::RoutingConnection).len(), 0);
    assert_ne!(cs.overall, ComparisonEquality::Equal);
}

// ── VCS-DIFF-07: identity ambiguity ────────────────────────────────

#[test]
fn cross_kind_raw_collision_is_ambiguous_identity() {
    // Same raw u128 claimed by Track on one side and Clip on the other —
    // undo watermark reseed / fork reissue evidence.
    let mut before = Project::new(ProjectId::new(1), "p");
    before.tracks.insert(
        TrackId::new(77),
        Track {
            id: TrackId::new(77),
            name: "ghost".into(),
            lane_ids: vec![],
        },
    );
    let mut after = before.clone();
    after.tracks.remove(&TrackId::new(77));
    after.clips.insert(
        ClipId::new(77),
        Clip {
            id: ClipId::new(77),
            lane_id: LaneId::new(1),
            source_id: SourceId::new(1),
            placement: clip_placement(0),
            playback: clip_playback(),
            muted: false,
        },
    );
    let (b, a) = manual_pair(before, after);
    let cs = compare(&b, &a);
    // Neither side may report Added/Removed/Modified for the collided IDs.
    assert_eq!(changes_of(&cs, ObjectKind::Track).len(), 0);
    assert_eq!(changes_of(&cs, ObjectKind::Clip).len(), 0);
    let ambig = cs
        .unresolved
        .iter()
        .filter(|i| {
            matches!(
                i,
                ComparisonIssue::AmbiguousIdentity {
                    reason: AmbiguityReason::CrossKindId,
                    ..
                }
            )
        })
        .count();
    assert_eq!(ambig, 2);
    assert!(matches!(cs.overall, ComparisonEquality::Unknown(_)));
}

#[test]
fn owner_reference_conflict_is_ambiguous_identity() {
    // A note whose owning clip differs across snapshots cannot be
    // confirmed as the same entity.
    let (c, _, _, _, _, note) = base_project();
    let mut after = c.live().clone();
    after.notes.get_mut(&note).unwrap().clip = ClipId::new(0x9999);
    let (b, a) = manual_pair(c.live().clone(), after);
    let cs = compare(&b, &a);
    assert!(cs.unresolved.iter().any(|i| matches!(
        i,
        ComparisonIssue::AmbiguousIdentity {
            reason: AmbiguityReason::OwnerConflict,
            ..
        }
    )));
    assert_eq!(changes_of(&cs, ObjectKind::Note).len(), 0);
}

#[test]
fn different_project_ids_are_unknown_not_diffed() {
    let (c, ..) = base_project();
    let before = snap_live(&c);
    let mut other = c.live().clone();
    other.project_id = ProjectId::new(2);
    let after = snap_manual(other, SCHEMA_VERSION, "other");
    let cs = compare(&before, &after);
    assert_eq!(cs.changes.len(), 0);
    assert!(matches!(cs.overall, ComparisonEquality::Unknown(_)));
    assert!(cs.unresolved.iter().any(|i| matches!(
        i,
        ComparisonIssue::AmbiguousIdentity {
            reason: AmbiguityReason::CrossProject,
            ..
        }
    )));
}

// ── VCS-DIFF-08: determinism / cache / budget ──────────────────────

#[test]
fn change_set_cache_returns_identical_result() {
    let (mut c, _, _, _, clip, _) = base_project();
    let before = snap_live(&c);
    apply(
        &mut c,
        ProjectOp::MoveClip {
            clip_id: clip,
            anchor: TimeAnchor::Musical(MusicalTick::new(480)),
        },
    );
    let after = snap_live(&c);
    let spec = spec_for(&before, &after);
    let cancel = SnapshotCancel::new();
    let mut cache = ChangeSetCache::new(8);
    let first: Arc<ChangeSet> = cache
        .get_or_compare(&before, &after, &spec, &cancel)
        .unwrap();
    let second = cache
        .get_or_compare(&before, &after, &spec, &cancel)
        .unwrap();
    assert!(Arc::ptr_eq(&first, &second));
}

#[test]
fn budget_exceeded_and_cancel_are_explicit_errors() {
    let (mut c, _, _, _, clip, _) = base_project();
    let before = snap_live(&c);
    apply(
        &mut c,
        ProjectOp::MoveClip {
            clip_id: clip,
            anchor: TimeAnchor::Musical(MusicalTick::new(480)),
        },
    );
    let after = snap_live(&c);
    let mut spec = spec_for(&before, &after);
    spec.budget = ComparisonBudget {
        max_entities: 1,
        max_changes: 10,
        max_unresolved: 10,
    };
    let err = compare_snapshots(&before, &after, &spec, &SnapshotCancel::new()).unwrap_err();
    assert!(matches!(err, DiffError::BudgetExceeded { .. }));

    let cancel = SnapshotCancel::new();
    cancel.cancel();
    let err2 = compare_snapshots(&before, &after, &spec_for(&before, &after), &cancel).unwrap_err();
    assert_eq!(err2, DiffError::Cancelled);
}

// ── RCD-03: monitor declaration classification (synvelis#117) ─────

fn monitor_decl(lane: Option<LaneId>) -> TrackMonitorDecl {
    TrackMonitorDecl {
        lane,
        channel_map: MonitorChannelMap::identity(),
        arm: ArmState::On,
        monitor: MonitorState::Auto,
    }
}

#[test]
fn monitor_decl_add_remove_is_added_removed() {
    let mut c = controller();
    apply(&mut c, ProjectOp::AddTrack { name: "vox".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    let before = snap_live(&c);
    apply(
        &mut c,
        ProjectOp::SetTrackMonitor {
            track,
            decl: monitor_decl(None),
        },
    );
    let after = snap_live(&c);

    let ab = compare(&before, &after);
    let hits = changes_of(&ab, ObjectKind::TrackMonitorDecl);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Added);
    assert!(hits[0].before.is_none());
    assert_eq!(hits[0].object.stable_id, track.to_tagged());
    // The declaration is not double-reported on its owner track.
    assert_eq!(changes_of(&ab, ObjectKind::Track).len(), 0);

    apply(&mut c, ProjectOp::ClearTrackMonitor { track });
    let cleared = snap_live(&c);
    let bc = compare(&after, &cleared);
    let hits = changes_of(&bc, ObjectKind::TrackMonitorDecl);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Removed);
    assert!(hits[0].after.is_none());
}

#[test]
fn monitor_decl_field_edit_is_modified() {
    let mut c = controller();
    apply(&mut c, ProjectOp::AddTrack { name: "vox".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    apply(
        &mut c,
        ProjectOp::SetTrackMonitor {
            track,
            decl: monitor_decl(None),
        },
    );
    let before = snap_live(&c);

    let mut next = monitor_decl(Some(lane));
    next.arm = ArmState::Off;
    next.monitor = MonitorState::On;
    next.channel_map = MonitorChannelMap {
        channels: vec![0, 1, 2, 3],
    };
    apply(&mut c, ProjectOp::SetTrackMonitor { track, decl: next });
    let after = snap_live(&c);

    let cs = compare(&before, &after);
    let hits = changes_of(&cs, ObjectKind::TrackMonitorDecl);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Modified);
    let fields = fields_of(hits[0]);
    assert!(fields.contains(&"lane"));
    assert!(fields.contains(&"arm"));
    assert!(fields.contains(&"monitor"));
    assert!(fields.contains(&"channel_map.channels"));
    assert_eq!(cs.overall, ComparisonEquality::Different);
}

#[test]
fn punch_marks_add_modify_remove() {
    let mut c = controller();
    let before = snap_live(&c);
    apply(
        &mut c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(960)),
            punch_out: Some(MusicalTick::new(1920)),
        },
    );
    let punched = snap_live(&c);

    // Undeclared → declared = Added, carrying the declared range.
    let ab = compare(&before, &punched);
    let hits = changes_of(&ab, ObjectKind::PunchMarks);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Added);
    assert!(hits[0].before.is_none());
    assert!(matches!(
        hits[0].affected_range,
        Some(SemanticTimeRange::Musical(_))
    ));

    // Declared → declared (mark edit) = Modified with field evidence.
    apply(
        &mut c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(480)),
            punch_out: Some(MusicalTick::new(1920)),
        },
    );
    let moved = snap_live(&c);
    let bc = compare(&punched, &moved);
    let hits = changes_of(&bc, ObjectKind::PunchMarks);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Modified);
    let fields = fields_of(hits[0]);
    assert!(fields.contains(&"punch_in"));
    assert!(!fields.contains(&"punch_out"));

    // Declared → undeclared = Removed, still reporting the span the
    // removed declaration covered (in=480, out=1920 at `moved`).
    apply(
        &mut c,
        ProjectOp::SetPunchMarks {
            punch_in: None,
            punch_out: None,
        },
    );
    let cleared = snap_live(&c);
    let cd = compare(&moved, &cleared);
    let hits = changes_of(&cd, ObjectKind::PunchMarks);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Removed);
    assert!(hits[0].after.is_none());
    let Some(SemanticTimeRange::Musical(range)) = hits[0].affected_range else {
        panic!("removed punch declaration must report its covered range");
    };
    assert_eq!(range.start(), MusicalTick::new(480));
    assert_eq!(range.end(), MusicalTick::new(1920));
}

#[test]
fn punch_marks_partial_declaration_is_modified_without_range() {
    // A declaration with only one mark set is still declared (Modified
    // against a full declaration) but carries no expressible range.
    let mut before = Project::new(ProjectId::new(1), "p");
    before.monitoring.punch = PunchMarks {
        punch_in: Some(MusicalTick::new(0)),
        punch_out: Some(MusicalTick::new(960)),
    };
    let mut after = before.clone();
    after.monitoring.punch = PunchMarks {
        punch_in: Some(MusicalTick::new(480)),
        punch_out: None,
    };
    let (b, a) = manual_pair(before, after);
    let cs = compare(&b, &a);
    let hits = changes_of(&cs, ObjectKind::PunchMarks);
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].kind, ChangeKind::Modified);
    assert!(hits[0].affected_range.is_none());
}

#[test]
fn monitor_changes_follow_recording_domain_filter() {
    let mut c = controller();
    apply(&mut c, ProjectOp::AddTrack { name: "vox".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    let before = snap_live(&c);
    apply(
        &mut c,
        ProjectOp::SetTrackMonitor {
            track,
            decl: monitor_decl(None),
        },
    );
    apply(
        &mut c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(0)),
            punch_out: Some(MusicalTick::new(960)),
        },
    );
    let after = snap_live(&c);

    // All domains (incl. Recording) → both monitor kinds appear and
    // a monitor-only difference is `Different`, never `Equal`.
    let all = compare(&before, &after);
    assert!(has_kind(
        &all,
        ObjectKind::TrackMonitorDecl,
        ChangeKind::Added
    ));
    assert!(has_kind(&all, ObjectKind::PunchMarks, ChangeKind::Added));
    assert_eq!(all.overall, ComparisonEquality::Different);

    // Recording excluded → no monitor changes (nor any others).
    let mut spec = spec_for(&before, &after);
    spec.domains = ComparisonDomains(BTreeSet::from([
        ComparisonDomain::Project,
        ComparisonDomain::Arrangement,
    ]));
    let cs = compare_snapshots(&before, &after, &spec, &SnapshotCancel::new()).unwrap();
    assert_eq!(changes_of(&cs, ObjectKind::TrackMonitorDecl).len(), 0);
    assert_eq!(changes_of(&cs, ObjectKind::PunchMarks).len(), 0);
    assert_eq!(cs.overall, ComparisonEquality::Equal);
}

#[test]
fn monitor_state_does_not_poison_aa_equality() {
    let mut c = controller();
    apply(&mut c, ProjectOp::AddTrack { name: "vox".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::SetTrackMonitor {
            track,
            decl: monitor_decl(None),
        },
    );
    apply(
        &mut c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(0)),
            punch_out: Some(MusicalTick::new(960)),
        },
    );
    let a = snap_live(&c);
    let b = snap_live(&c);
    let cs = compare(&a, &b);
    assert_eq!(cs.overall, ComparisonEquality::Equal);
    assert_eq!(cs.changes.len(), 0);
}

#[test]
fn monitor_decl_on_ambiguous_track_raw_is_ambiguous() {
    // before: track 77 + monitor decl keyed by it; after: raw 77
    // reissued as a clip — the decl key's identity is ambiguous, so
    // neither the decl nor the collided entities may diff.
    let mut before = Project::new(ProjectId::new(1), "p");
    before.tracks.insert(
        TrackId::new(77),
        Track {
            id: TrackId::new(77),
            name: "ghost".into(),
            lane_ids: vec![],
        },
    );
    before
        .monitoring
        .tracks
        .insert(TrackId::new(77), monitor_decl(None));
    let mut after = before.clone();
    after.tracks.remove(&TrackId::new(77));
    after.monitoring.tracks.remove(&TrackId::new(77));
    after.clips.insert(
        ClipId::new(77),
        Clip {
            id: ClipId::new(77),
            lane_id: LaneId::new(1),
            source_id: SourceId::new(1),
            placement: clip_placement(0),
            playback: clip_playback(),
            muted: false,
        },
    );
    let (b, a) = manual_pair(before, after);
    let cs = compare(&b, &a);
    assert_eq!(changes_of(&cs, ObjectKind::TrackMonitorDecl).len(), 0);
    assert_eq!(changes_of(&cs, ObjectKind::Track).len(), 0);
    assert_eq!(changes_of(&cs, ObjectKind::Clip).len(), 0);
    assert!(cs.unresolved.iter().any(|i| matches!(
        i,
        ComparisonIssue::AmbiguousIdentity {
            object,
            reason: AmbiguityReason::CrossKindId,
        } if object.kind == ObjectKind::TrackMonitorDecl
    )));
    assert!(matches!(cs.overall, ComparisonEquality::Unknown(_)));
}
