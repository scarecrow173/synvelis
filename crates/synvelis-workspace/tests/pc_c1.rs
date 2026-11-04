//! PC-C1 acceptance tests — `docs/test-specs/pc-c1-e2e-recording-transaction.md`.
//!
//! The recording-transaction safety contract (issue #98 / issue #6):
//! non-destructive `prepare`, CAS-bound apply against the fixed
//! revision, success-only `finish`, retained columns on every failure,
//! explicit rebase only, in-memory exactly-once, and gate closure on
//! every lifecycle change — verified headlessly with deterministic
//! `CounterIdGenerator` ids and `FakeWindowHost`; no GUI, no real
//! plugin (those live in PC-C6's oracle, issue #103).

use ruviform_media_core::ClockEpochId;
use synvelis_project::{
    AutomationLaneId, AutomationPoint, BeatPoint, ClipExtent, ClipId, ClipPlacement, ClipPlayback,
    ControlScope, CounterIdGenerator, MusicalDuration, MusicalTick, NoteId, ParamMapping,
    ParameterTargetId, PlaybackMapping, Project, ProjectController, ProjectId, ProjectOp,
    ProjectRevision, SourceFrame, SourceRange, TimeAnchor, TrackId, ValueKind,
};
use synvelis_workspace::{
    AutomationMode, FakeWindowHost, PreparedRecordingCommit, ProjectSession, RecordCommitError,
    RecordDisposition, RecordingTransactionId, RecordingTxnState, SealReason, WorkspaceManager,
    WriteArm,
};

// ---------------------------------------------------------------------------
// fixture (same conventions as pc_a.rs)
// ---------------------------------------------------------------------------

type Ws = WorkspaceManager<FakeWindowHost>;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0xCC1), "pc-c1-test");
    let controller = ProjectController::new(project, Box::new(CounterIdGenerator::new(1)));
    WorkspaceManager::new(FakeWindowHost::default(), ProjectSession::new(controller))
}

fn apply(ws: &mut Ws, op: ProjectOp) -> ProjectRevision {
    let rev = ws.session().revision();
    ws.session_mut().apply(op, rev).expect("fixture op")
}

fn bp(num: i64) -> BeatPoint {
    BeatPoint::new(num, 1).unwrap()
}

fn pt(pos: BeatPoint, value: f64) -> AutomationPoint {
    AutomationPoint { pos, value }
}

fn add_track_target(ws: &mut Ws, name: &str, base: f64) -> (TrackId, ParameterTargetId) {
    apply(ws, ProjectOp::AddTrack { name: "t".into() });
    let track = *ws.session().project().tracks.keys().last().unwrap();
    apply(
        ws,
        ProjectOp::AddParameterTarget {
            name: name.into(),
            owner: ControlScope::Track(track),
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base,
        },
    );
    let target = *ws
        .session()
        .project()
        .control
        .targets
        .keys()
        .last()
        .unwrap();
    (track, target)
}

fn add_automation_lane(ws: &mut Ws, target: ParameterTargetId) -> AutomationLaneId {
    apply(ws, ProjectOp::AddAutomationLane { target });
    *ws.session().project().control.lanes.keys().last().unwrap()
}

fn seeded_clip(ws: &mut Ws, track: TrackId, start_tick: i64, end_tick: i64) -> ClipId {
    apply(
        ws,
        ProjectOp::RegisterAsset {
            digest: "aa".into(),
        },
    );
    let asset = *ws.session().project().assets.keys().last().unwrap();
    apply(
        ws,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "aa".into(),
        },
    );
    let source = *ws.session().project().sources.keys().last().unwrap();
    apply(ws, ProjectOp::AddLane { track_id: track });
    let lane = *ws.session().project().lanes.keys().last().unwrap();
    apply(
        ws,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(start_tick)),
                extent: ClipExtent::Musical(MusicalDuration::new(end_tick - start_tick).unwrap()),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(1_000_000).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    *ws.session().project().clips.keys().last().unwrap()
}

/// Arms `target` in `Touch` and opens a recording transaction on a
/// rolling transport; returns the transaction id.
fn open_recording(
    ws: &mut Ws,
    target: ParameterTargetId,
    lane: AutomationLaneId,
) -> RecordingTransactionId {
    ws.session_mut()
        .control_with(|c, p| c.bind_write_lane(p, target, lane))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Touch))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.set_arm(p, target, WriteArm::Armed))
        .unwrap();
    ws.session_mut().control_mut().set_record_enabled(true);
    ws.session_mut().control_mut().transport_play();
    let rev = ws.session().revision();
    ws.session_mut().control_mut().record_start(rev).unwrap()
}

/// Total recorded points across the open transaction's segments.
fn recorded_points(ws: &Ws) -> usize {
    ws.session()
        .control()
        .recording()
        .map_or(0, |tx| tx.segments.values().map(Vec::len).sum())
}

// ---------------------------------------------------------------------------
// REC-C1-01 committed success
// ---------------------------------------------------------------------------

/// A successful commit lands one `Composite` = **one undo step** of
/// raw (pre-modulation) manual values on the fixed target/lane; the
/// commit never implies a save, and the transaction carries a
/// session-unique id.
#[test]
fn rec_c1_01_commit_success_one_undo_raw_manual_save_separate() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    let before = ws.session().revision();
    let id = open_recording(&mut ws, target, lane);
    assert_eq!(id, RecordingTransactionId(1));
    let tx = ws.session().control().recording().unwrap();
    assert_eq!(tx.fixed_revision, before);
    assert_eq!(tx.fixed_targets, vec![target]);
    assert_eq!(tx.fixed_lanes.get(&target), Some(&lane));
    assert_eq!(tx.state, RecordingTxnState::Pending);
    assert!(tx.sealed.is_none());

    // Manual (pre-modulation) points only — writes never touch the
    // project mid-accumulation (no revision drift, no project op).
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(1), 0.6))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(2), 0.7))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(3), 0.9))
        .unwrap();
    assert_eq!(recorded_points(&ws), 3);
    assert_eq!(ws.session().revision(), before);

    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert_eq!(outcome.id, id);
    assert!(outcome.applied && !outcome.was_sealed);
    let committed_rev = outcome.revision.unwrap();
    assert!(committed_rev > before);
    // The transaction is consumed; the receipt is registered.
    assert!(ws.session().control().recording().is_none());
    assert_eq!(
        ws.session().control().applied_record(id).unwrap().revision,
        committed_rev
    );
    assert_eq!(
        ws.session().control().last_disposition(),
        Some(RecordDisposition::Committed {
            id,
            revision: Some(committed_rev)
        })
    );

    // One region on the fixed lane, manual values verbatim.
    let regions = &ws.session().project().control.lanes[&lane].regions;
    assert_eq!(regions.len(), 1);
    assert_eq!(
        regions[0].points,
        vec![pt(bp(1), 0.6), pt(bp(2), 0.7), pt(bp(3), 0.9)]
    );

    // The commit never implies a save — the store still has nothing.
    assert!(ws.session().controller().saved().is_none());

    // One session = one undo: the whole interval leaves with a single
    // undo and returns with a single redo.
    assert!(ws.session_mut().undo().unwrap());
    assert_eq!(
        ws.session().project().control.lanes[&lane].regions,
        Vec::new()
    );
    assert!(ws.session_mut().redo().unwrap());
    assert_eq!(ws.session().project().control.lanes[&lane].regions.len(), 1);
}

/// An empty transaction commits nothing — no op, no undo entry.
#[test]
fn rec_c1_01_empty_session_applies_nothing() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    let id = open_recording(&mut ws, target, lane);
    let rev = ws.session().revision();
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert_eq!(outcome.id, id);
    assert!(!outcome.applied && outcome.revision.is_none());
    assert_eq!(ws.session().revision(), rev);
    // The empty commit created no undo entry: the stack top is still
    // the fixture's `AddAutomationLane`.
    assert!(ws.session_mut().undo().unwrap());
    assert!(!ws.session().project().control.lanes.contains_key(&lane));
}

// ---------------------------------------------------------------------------
// REC-C1-02 stale revision → Conflict, explicit rebase only
// ---------------------------------------------------------------------------

/// Any project drift — including an unrelated track edit — turns the
/// next commit into the default `Conflict` with the columns retained;
/// only an explicit `approve_rebase` lets the commit land.
#[test]
fn rec_c1_02_stale_revision_conflicts_then_explicit_rebase_commits() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    let id = open_recording(&mut ws, target, lane);
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(1), 0.5))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(2), 0.8))
        .unwrap();

    // An unrelated foreign edit drifts the revision and seals the
    // write gate — the columns stay.
    let fixed_rev = ws.session().control().recording().unwrap().fixed_revision;
    let foreign_rev = apply(&mut ws, ProjectOp::AddTrack { name: "x".into() });
    let tx = ws.session().control().recording().unwrap();
    assert_eq!(tx.sealed, Some(SealReason::ProjectChange));
    assert_eq!(tx.fixed_revision, fixed_rev);
    // The drifted revision alone blocks further writes.
    assert!(
        ws.session_mut()
            .control_with(|c, p| c.record_write(p, target, bp(3), 0.1))
            .is_err()
    );

    // Default outcome: Conflict naming fixed/current/targets.
    let err = ws.session_mut().control_record_commit().unwrap_err();
    match err {
        RecordCommitError::Conflict {
            fixed,
            current,
            affected_targets,
        } => {
            assert_eq!(fixed, fixed_rev);
            assert_eq!(current, foreign_rev);
            assert_eq!(affected_targets, vec![target]);
        }
        other => panic!("expected Conflict, got {other:?}"),
    }
    // Nothing applied, nothing lost: transaction Conflict, points
    // retained, no phantom undo entry.
    let tx = ws.session().control().recording().unwrap();
    assert_eq!(tx.state, RecordingTxnState::Conflict);
    assert_eq!(recorded_points(&ws), 2);
    assert_eq!(ws.session().revision(), foreign_rev);
    assert_eq!(ws.session().project().control.lanes[&lane].regions, vec![]);

    // Inspect: the preview reports what a rebase would write — target
    // and lane still exist, two recorded points, nothing overlapping.
    let preview = ws.session().control_record_rebase_preview().unwrap();
    assert_eq!(preview.id, id);
    assert_eq!(preview.fixed_revision, fixed_rev);
    assert_eq!(preview.current_revision, foreign_rev);
    assert_eq!(preview.sealed, Some(SealReason::ProjectChange));
    assert_eq!(preview.lanes.len(), 1);
    let report = &preview.lanes[0];
    assert_eq!(report.lane, lane);
    assert_eq!(report.fixed_target, target);
    assert!(report.target_exists && report.lane_exists);
    assert_eq!(report.recorded_points, 2);
    assert_eq!(report.recorded_range, Some((bp(1), bp(2))));
    assert_eq!(report.overlapping_regions, 0);

    // No automatic rebase: committing again still conflicts.
    assert!(matches!(
        ws.session_mut().control_record_commit(),
        Err(RecordCommitError::Conflict { .. })
    ));

    // Explicit Rebase — the only path forward.
    ws.session_mut().control_record_approve_rebase(id).unwrap();
    let tx = ws.session().control().recording().unwrap();
    assert_eq!(tx.fixed_revision, foreign_rev);
    assert_eq!(tx.state, RecordingTxnState::Pending);
    // The write gate stays sealed — a rebase never reopens recording.
    assert_eq!(tx.sealed, Some(SealReason::ProjectChange));

    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert!(outcome.applied);
    let regions = &ws.session().project().control.lanes[&lane].regions;
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].points, vec![pt(bp(1), 0.5), pt(bp(2), 0.8)]);
    // Still exactly one undo entry for the whole session.
    assert!(ws.session_mut().undo().unwrap());
    assert_eq!(ws.session().project().control.lanes[&lane].regions, vec![]);
}

/// Undo/redo are foreign project changes too: they seal the gate and
/// the commit conflicts until explicitly rebased.
#[test]
fn rec_c1_02_undo_during_recording_conflicts() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    let id = open_recording(&mut ws, target, lane);
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(1), 0.5))
        .unwrap();
    assert!(ws.session_mut().undo().unwrap());
    assert_eq!(
        ws.session().control().recording().unwrap().sealed,
        Some(SealReason::ProjectChange)
    );
    assert!(matches!(
        ws.session_mut().control_record_commit(),
        Err(RecordCommitError::Conflict { .. })
    ));
    // Abort is the honest end for a conflict the user declines — the
    // lane keeps no region from the aborted transaction (the undo
    // itself removed the fixture lane, which is fine).
    ws.session_mut().control_record_abort();
    assert!(ws.session().control().recording().is_none());
    assert_eq!(
        ws.session().control().last_disposition(),
        Some(RecordDisposition::Aborted { id })
    );
    assert!(
        ws.session()
            .project()
            .control
            .lanes
            .get(&lane)
            .is_none_or(|l| l.regions.is_empty())
    );
}

// ---------------------------------------------------------------------------
// REC-C1-03 structural rejection — project/revision/undo unchanged
// ---------------------------------------------------------------------------

/// A lane deleted mid-recording: the commit first reports `Conflict`
/// (the deletion itself drifted the revision), the preview reports
/// the dead lane, and after an explicit rebase the structural check
/// rejects — project, revision and undo stay exactly as the deletion
/// left them; points and fixed targets are retained.
#[test]
fn rec_c1_03_deleted_lane_rejects_with_columns_retained() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    let id = open_recording(&mut ws, target, lane);
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(1), 0.5))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(2), 0.8))
        .unwrap();

    let after_delete = apply(&mut ws, ProjectOp::RemoveAutomationLane { lane });
    assert!(matches!(
        ws.session_mut().control_record_commit(),
        Err(RecordCommitError::Conflict { .. })
    ));

    // Inspect: the lane is gone in the live project.
    let preview = ws.session().control_record_rebase_preview().unwrap();
    assert_eq!(preview.lanes.len(), 1);
    assert!(preview.lanes[0].target_exists);
    assert!(!preview.lanes[0].lane_exists);

    // Explicit rebase, then the structural check rejects.
    ws.session_mut().control_record_approve_rebase(id).unwrap();
    let err = ws.session_mut().control_record_commit().unwrap_err();
    assert!(matches!(err, RecordCommitError::Invalid(_)), "{err:?}");
    // Project, revision and undo are exactly what the deletion left:
    // no partial write, no phantom undo entry.
    assert_eq!(ws.session().revision(), after_delete);
    let tx = ws.session().control().recording().unwrap();
    assert_eq!(tx.state, RecordingTxnState::Rejected);
    assert_eq!(tx.fixed_targets, vec![target]);
    assert_eq!(recorded_points(&ws), 2);
    // The one live undo is the lane deletion itself — the failed
    // commit created none.
    assert!(ws.session_mut().undo().unwrap()); // undoes RemoveAutomationLane
    assert!(ws.session().project().control.lanes.contains_key(&lane));

    // Retained columns can still commit after the lane is back and
    // the revision is re-pinned.
    ws.session_mut().control_record_approve_rebase(id).unwrap();
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert!(outcome.applied);
    assert_eq!(
        ws.session().project().control.lanes[&lane].regions[0].points,
        vec![pt(bp(1), 0.5), pt(bp(2), 0.8)]
    );
}

/// A deleted fixed target rejects the same way.
#[test]
fn rec_c1_03_deleted_target_rejects() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    let id = open_recording(&mut ws, target, lane);
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(1), 0.5))
        .unwrap();
    apply(&mut ws, ProjectOp::RemoveAutomationLane { lane });
    apply(&mut ws, ProjectOp::RemoveParameterTarget { target });
    assert!(matches!(
        ws.session_mut().control_record_commit(),
        Err(RecordCommitError::Conflict { .. })
    ));
    ws.session_mut().control_record_approve_rebase(id).unwrap();
    let err = ws.session_mut().control_record_commit().unwrap_err();
    assert!(matches!(err, RecordCommitError::Invalid(_)), "{err:?}");
    assert_eq!(recorded_points(&ws), 1);
    assert_eq!(
        ws.session().control().recording().unwrap().fixed_targets,
        vec![target]
    );
}

// ---------------------------------------------------------------------------
// REC-C1-04 idempotency — in-memory exactly-once
// ---------------------------------------------------------------------------

/// A duplicate commit of the same prepared payload resolves through
/// the applied registry to `AlreadyApplied` — never a second write.
/// The same id with a different fingerprint is `IntegrityConflict`,
/// an unknown id `UnknownTransaction`. The registry is in-memory and
/// session-scoped only (no crash recovery — issue #12).
#[test]
fn rec_c1_04_duplicate_ack_is_exactly_once() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    let id = open_recording(&mut ws, target, lane);
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(1), 0.5))
        .unwrap();
    let rev = ws.session().revision();
    let prepared = ws
        .session_mut()
        .control_with(|c, p| c.prepare_record_commit(p, rev))
        .unwrap();
    assert_eq!(prepared.id, id);

    // First finish applies once.
    let outcome = ws.session_mut().control_record_finish(&prepared).unwrap();
    assert!(outcome.applied);
    let committed_rev = outcome.revision.unwrap();

    // Duplicate ACK / double click: same id + same fingerprint.
    let err = ws
        .session_mut()
        .control_record_finish(&prepared)
        .unwrap_err();
    assert_eq!(
        err,
        RecordCommitError::AlreadyApplied {
            id,
            revision: committed_rev
        }
    );
    // Still exactly one region — nothing was written twice.
    assert_eq!(ws.session().project().control.lanes[&lane].regions.len(), 1);

    // Same id, different payload → IntegrityConflict.
    let mut tampered = prepared.clone();
    tampered.fingerprint ^= 0xA5;
    assert_eq!(
        ws.session_mut()
            .control_record_finish(&tampered)
            .unwrap_err(),
        RecordCommitError::IntegrityConflict { id }
    );

    // Unknown id → UnknownTransaction.
    let unknown = PreparedRecordingCommit {
        id: RecordingTransactionId(999),
        fingerprint: 0,
        expected_revision: committed_rev,
        op: None,
        was_sealed: false,
    };
    assert_eq!(
        ws.session_mut()
            .control_record_finish(&unknown)
            .unwrap_err(),
        RecordCommitError::UnknownTransaction {
            id: RecordingTransactionId(999)
        }
    );

    // No open transaction → NoTransaction.
    assert_eq!(
        ws.session_mut().control_record_commit().unwrap_err(),
        RecordCommitError::NoTransaction
    );
}

/// An old prepared payload cannot finish a *different* open
/// transaction — the open one keeps its own identity.
#[test]
fn rec_c1_04_stale_prepared_cannot_hijack_open_transaction() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    let id1 = open_recording(&mut ws, target, lane);
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(1), 0.5))
        .unwrap();
    let rev = ws.session().revision();
    let first = ws
        .session_mut()
        .control_with(|c, p| c.prepare_record_commit(p, rev))
        .unwrap();
    ws.session_mut().control_record_abort();

    // A second transaction gets a fresh, monotonically higher id.
    let id2 = open_recording(&mut ws, target, lane);
    assert!(id2 > id1);
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(4), 0.9))
        .unwrap();
    // Finishing the aborted transaction's prepared payload must not
    // consume the open one.
    assert_eq!(
        ws.session_mut().control_record_finish(&first).unwrap_err(),
        RecordCommitError::UnknownTransaction { id: id1 }
    );
    assert_eq!(ws.session().control().recording().unwrap().id, id2);
    assert_eq!(recorded_points(&ws), 1);
    // The open transaction still commits on its own prepared payload.
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert_eq!(outcome.id, id2);
    assert!(outcome.applied);
}

// ---------------------------------------------------------------------------
// REC-C1-05 gates close on lifecycle changes
// ---------------------------------------------------------------------------

/// Pause mutes the write gate but does **not** seal — resume lands
/// writes into the same transaction; stop/seek/disarm/mode-change do
/// seal. An old receipt and a non-open commit are typed errors.
#[test]
#[allow(clippy::too_many_lines)] // sequential acceptance scenario
fn rec_c1_05_lifecycle_gates() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    open_recording(&mut ws, target, lane);

    // Pause: writes are rejected, the transaction stays unsealed.
    ws.session_mut().control_mut().transport_pause();
    assert!(
        ws.session_mut()
            .control_with(|c, p| c.record_write(p, target, bp(1), 0.5))
            .is_err()
    );
    assert!(ws.session().control().recording().unwrap().sealed.is_none());
    // A Touch release during the pause writes nothing.
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(1)))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(1), 0.9)
        .unwrap();
    let end = ws.session_mut().control_gesture_end(target).unwrap();
    assert_eq!(end, synvelis_workspace::GestureEnd::Returned);
    assert_eq!(recorded_points(&ws), 0);

    // Resume: the same transaction keeps recording.
    ws.session_mut().control_mut().transport_resume();
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(1), 0.5))
        .unwrap();
    assert_eq!(recorded_points(&ws), 1);

    // Stop: seals the write gate; commit still works (seal ≠ abort).
    ws.session_mut().control_mut().transport_stop();
    assert_eq!(
        ws.session().control().recording().unwrap().sealed,
        Some(SealReason::Stopped)
    );
    assert!(
        ws.session_mut()
            .control_with(|c, p| c.record_write(p, target, bp(2), 0.6))
            .is_err()
    );
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert!(outcome.applied && outcome.was_sealed);
    assert!(ws.session().control().recording().is_none());
    // Old receipt for a consumed transaction → NoTransaction, not a
    // silent second apply.
    assert_eq!(
        ws.session_mut().control_record_commit().unwrap_err(),
        RecordCommitError::NoTransaction
    );

    // Seek seals.
    let id = open_recording(&mut ws, target, lane);
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(1), 0.4))
        .unwrap();
    ws.session_mut().control_mut().transport_seek(bp(9));
    assert_eq!(
        ws.session().control().recording().unwrap().sealed,
        Some(SealReason::Seek)
    );
    ws.session_mut().control_record_abort();
    assert_eq!(
        ws.session().control().last_disposition(),
        Some(RecordDisposition::Aborted { id })
    );

    // Disarm seals.
    open_recording(&mut ws, target, lane);
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(1), 0.4))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.set_arm(p, target, WriteArm::Disarmed))
        .unwrap();
    assert_eq!(
        ws.session().control().recording().unwrap().sealed,
        Some(SealReason::ArmChanged)
    );
    ws.session_mut().control_record_abort();

    // Mode change seals.
    open_recording(&mut ws, target, lane);
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Read))
        .unwrap();
    assert_eq!(
        ws.session().control().recording().unwrap().sealed,
        Some(SealReason::ModeChanged)
    );
    ws.session_mut().control_record_abort();

    // Rebinding a fixed target's lane mid-recording is refused.
    open_recording(&mut ws, target, lane);
    let other_lane = add_automation_lane(&mut ws, target);
    // Adding the lane is itself a foreign change → sealed.
    assert_eq!(
        ws.session().control().recording().unwrap().sealed,
        Some(SealReason::ProjectChange)
    );
    assert!(
        ws.session_mut()
            .control_with(|c, p| c.bind_write_lane(p, target, other_lane))
            .is_err()
    );
    // Recorded lanes are still the fixed ones, not the rebound lane.
    assert_eq!(
        ws.session().control().recording().unwrap().fixed_lanes[&target],
        lane
    );

    // A cancelled gesture writes nothing.
    ws.session_mut().control_record_abort();
    let _id = open_recording(&mut ws, target, lane);
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(1)))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(1), 0.9)
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_cancel(target)
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.gesture_end(p, target))
        .unwrap_err();
    assert_eq!(recorded_points(&ws), 0);
    // The transaction itself is untouched by the cancel and commits.
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert!(!outcome.applied);
}

/// Latch/Write held values only land while the gates hold; a pause
/// neither writes nor seals.
#[test]
fn rec_c1_05_latch_pause_resume_no_seal() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    ws.session_mut()
        .control_with(|c, p| c.bind_write_lane(p, target, lane))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Latch))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.set_arm(p, target, WriteArm::Armed))
        .unwrap();
    ws.session_mut().control_mut().set_record_enabled(true);
    ws.session_mut().control_mut().transport_play();
    let rev = ws.session().revision();
    ws.session_mut().control_mut().record_start(rev).unwrap();

    // Gesture end in Latch holds the value; transport_advance lands it.
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0)))
        .unwrap();
    ws.session_mut()
        .control_mut()
        .gesture_update(target, bp(0), 0.7)
        .unwrap();
    let end = ws.session_mut().control_gesture_end(target).unwrap();
    assert_eq!(end, synvelis_workspace::GestureEnd::Latched { value: 0.7 });
    ws.session_mut()
        .control_with(|c, p| c.transport_advance(p, bp(1)))
        .unwrap();
    assert_eq!(recorded_points(&ws), 2); // gesture point + held write

    // Pause: advance lands nothing and seals nothing.
    ws.session_mut().control_mut().transport_pause();
    ws.session_mut()
        .control_with(|c, p| c.transport_advance(p, bp(2)))
        .unwrap();
    assert_eq!(recorded_points(&ws), 2);
    assert!(ws.session().control().recording().unwrap().sealed.is_none());

    // Resume: held writes land again in the same transaction.
    ws.session_mut().control_mut().transport_resume();
    ws.session_mut()
        .control_with(|c, p| c.transport_advance(p, bp(3)))
        .unwrap();
    assert_eq!(recorded_points(&ws), 3);
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert!(outcome.applied && !outcome.was_sealed);
}

// ---------------------------------------------------------------------------
// REC-C1-06 scope/time/epoch context + non-RT bounds
// ---------------------------------------------------------------------------

/// Clip-scoped and note-scoped targets keep their scope-relative
/// coordinates through the whole transaction; tempo/PPQN edits and a
/// transport-epoch change are distinct contexts — never conflated —
/// and project edits seal/conflict like every foreign change.
#[test]
#[allow(clippy::too_many_lines)] // sequential acceptance scenario
fn rec_c1_06_scope_epoch_and_tempo_contexts() {
    let mut ws = fresh_ws();
    let (track, _t) = add_track_target(&mut ws, "trk", 0.3);
    // clip at project beats [8, 16), note at clip-relative [2, 6) —
    // project beats [10, 14).
    let clip = seeded_clip(&mut ws, track, 7680, 15360);
    apply(
        &mut ws,
        ProjectOp::AddNote {
            clip,
            key: 60,
            start: bp(2),
            len: bp(4),
        },
    );
    let note: NoteId = *ws.session().project().notes.keys().last().unwrap();
    apply(
        &mut ws,
        ProjectOp::AddParameterTarget {
            name: "clip-env".into(),
            owner: ControlScope::Clip(clip),
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.1,
        },
    );
    let clip_target = *ws
        .session()
        .project()
        .control
        .targets
        .keys()
        .last()
        .unwrap();
    let clip_lane = add_automation_lane(&mut ws, clip_target);
    apply(
        &mut ws,
        ProjectOp::AddParameterTarget {
            name: "note-expr".into(),
            owner: ControlScope::Note(note),
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.1,
        },
    );
    let note_target = *ws
        .session()
        .project()
        .control
        .targets
        .keys()
        .last()
        .unwrap();
    let note_lane = add_automation_lane(&mut ws, note_target);

    let clip_id = open_recording(&mut ws, clip_target, clip_lane);
    // Project position 10 → clip-relative 2.
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, clip_target, bp(10), 0.5))
        .unwrap();
    // Project position 9 → clip-relative 1 — still inside the clip.
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, clip_target, bp(9), 0.3))
        .unwrap();
    // Project position 20 → outside the clip: records nothing.
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, clip_target, bp(20), 0.9))
        .unwrap();
    let tx = ws.session().control().recording().unwrap();
    assert_eq!(tx.epoch, ws.session().control().playback().epoch);
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert_eq!(outcome.id, clip_id);
    // Clip-relative coordinates — the project positions did not leak.
    let regions = &ws.session().project().control.lanes[&clip_lane].regions;
    assert_eq!(regions[0].points, vec![pt(bp(1), 0.3), pt(bp(2), 0.5)]);

    let note_id = open_recording(&mut ws, note_target, note_lane);
    // Project 10 → note-relative 0; project 12 → note-relative 2.
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, note_target, bp(10), 0.4))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, note_target, bp(12), 0.9))
        .unwrap();
    // Outside the note domain → nothing recorded.
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, note_target, bp(9), 0.1))
        .unwrap();
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert_eq!(outcome.id, note_id);
    let regions = &ws.session().project().control.lanes[&note_lane].regions;
    assert_eq!(regions[0].points, vec![pt(bp(0), 0.4), pt(bp(2), 0.9)]);

    // Transport generation is captured per transaction: a new epoch
    // does not rewrite the recorded context.
    let id = open_recording(&mut ws, clip_target, clip_lane);
    let epoch_at_start = ws.session().control().recording().unwrap().epoch;
    // A tempo-map edit is a foreign project change — seal + conflict,
    // never a silent reinterpretation of the recorded beats.
    apply(
        &mut ws,
        ProjectOp::AddTempoEvent {
            tick: 7680,
            bpm_milli: 140_000,
        },
    );
    assert_eq!(
        ws.session().control().recording().unwrap().sealed,
        Some(SealReason::ProjectChange)
    );
    assert!(matches!(
        ws.session_mut().control_record_commit(),
        Err(RecordCommitError::Conflict { .. })
    ));
    assert_eq!(
        ws.session().control().recording().unwrap().epoch,
        epoch_at_start
    );
    ws.session_mut().control_record_abort();
    assert_eq!(
        ws.session().control().last_disposition(),
        Some(RecordDisposition::Aborted { id })
    );
    // Transaction ids never repeat within the session.
    let next = open_recording(&mut ws, clip_target, clip_lane);
    assert!(next > id);
    ws.session_mut().control_record_abort();
    // The epoch a transaction captures is the playback epoch at
    // `record_start` — distinct ids and contexts are never conflated.
    assert_eq!(
        ws.session().control().playback().epoch,
        ClockEpochId::new(1)
    );
}

/// The recorded-columns budget is non-RT bounded: a batch that would
/// exceed `RECORD_POINT_BUDGET` is rejected whole — already-recorded
/// points are never dropped — and the transaction still commits.
#[test]
fn rec_c1_06_point_budget_rejects_whole_batch() {
    let mut ws = fresh_ws();
    let (_track, target) = add_track_target(&mut ws, "vol", 0.4);
    let lane = add_automation_lane(&mut ws, target);
    open_recording(&mut ws, target, lane);
    // Two prior points stay through the over-budget rejection.
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(1), 0.5))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(2), 0.6))
        .unwrap();

    // One gesture carrying more than the budget of distinct
    // positions: `gesture_end` hands the whole batch to
    // `record_points`, which must reject it atomically.
    ws.session_mut()
        .control_with(|c, p| c.gesture_begin(p, target, bp(0)))
        .unwrap();
    let budget = i64::try_from(synvelis_workspace::RECORD_POINT_BUDGET).unwrap();
    for i in 0..=budget {
        ws.session_mut()
            .control_mut()
            .gesture_update(target, bp(3 + i), 0.5)
            .unwrap();
    }
    assert!(
        ws.session_mut()
            .control_with(|c, p| c.gesture_end(p, target))
            .is_err()
    );
    // Prior points retained; the rejected batch recorded none.
    assert_eq!(recorded_points(&ws), 2);
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert!(outcome.applied);
    assert_eq!(
        ws.session().project().control.lanes[&lane].regions[0].points,
        vec![pt(bp(1), 0.5), pt(bp(2), 0.6)]
    );
}
