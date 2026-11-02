//! PC-B acceptance tests — `docs/test-specs/pc-b-e2e-parameter-dispatch.md`.
//!
//! PC-B-02: note-local expression scope — `ControlScope::Note` resolves
//! to a note-relative beat domain anchored inside its owning clip, and
//! recorded automation points are stored in that coordinate.

use synvelis_project::{
    AutomationLaneId, AutomationPoint, AutomationRegion, BeatPoint, BeatRange, ClipExtent, ClipId,
    ClipPlacement, ClipPlayback, ConflictPolicy, ControlScope, CounterIdGenerator, Error,
    Interpolation, MusicalDuration, MusicalTick, NoteId, ParamMapping, ParameterTargetId,
    PlaybackMapping, Project, ProjectController, ProjectId, ProjectOp, ProjectRevision,
    SourceFrame, SourceRange, TimeAnchor, TrackId, ValueKind, evaluate_control,
};
use synvelis_workspace::{
    AutomationMode, FakeWindowHost, ProjectSession, WorkspaceManager, WriteArm,
};

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

type Ws = WorkspaceManager<FakeWindowHost>;

fn fresh_ws() -> Ws {
    let project = Project::new(ProjectId::new(0xFCB), "pc-b-test");
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

fn span(start: BeatPoint, end: BeatPoint) -> BeatRange {
    BeatRange::try_new(start, end).unwrap()
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

/// clip at project beats 8..16 (ppqn 960 → ticks 7680..15360),
/// note at clip-relative beats 2..6 (project beats 10..14),
/// note-scoped target + automation lane.
fn note_fixture() -> (Ws, ClipId, NoteId, ParameterTargetId, AutomationLaneId) {
    let mut ws = fresh_ws();
    apply(&mut ws, ProjectOp::AddTrack { name: "t".into() });
    let track = *ws.session().project().tracks.keys().last().unwrap();
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
    let note = *ws.session().project().notes.keys().last().unwrap();
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
    let target = *ws
        .session()
        .project()
        .control
        .targets
        .keys()
        .last()
        .unwrap();
    apply(&mut ws, ProjectOp::AddAutomationLane { target });
    let lane = *ws.session().project().control.lanes.keys().last().unwrap();
    (ws, clip, note, target, lane)
}

// ---------------------------------------------------------------------------
// PC-B-02 note-local scope
// ---------------------------------------------------------------------------

/// Note scope resolves project → clip-relative → note-relative beats:
/// project pos 10 = note-local 0, pos 12 = note-local 2.
#[test]
fn pc_b_02_note_scope_evaluates_note_relative() {
    let (ws, _clip, _note, target, lane) = note_fixture();
    let mut ws = ws;
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: AutomationRegion {
                range: span(bp(0), bp(4)),
                interpolation: Interpolation::Step,
                points: vec![pt(bp(0), 0.2), pt(bp(2), 0.8)],
            },
            policy: ConflictPolicy::Reject,
        },
    );
    let project = ws.session().project();
    // project 10 → clip-rel 2 → note-rel 0 → value 0.2 (step holds)
    let eval = evaluate_control(project, target, bp(10), None).unwrap();
    assert!((eval.host_normalized - 0.2).abs() < 1e-9);
    assert_eq!(eval.position.numerator(), 0);
    // project 12 → note-rel 2 → value 0.8
    let eval = evaluate_control(project, target, bp(12), None).unwrap();
    assert!((eval.host_normalized - 0.8).abs() < 1e-9);
    // project 9 → clip-rel 1 < note.start → outside domain
    assert!(matches!(
        evaluate_control(project, target, bp(9), None),
        Err(Error::ScopeUnavailable { .. })
    ));
    // project 14 → note-rel 4 == start+len → outside domain
    assert!(matches!(
        evaluate_control(project, target, bp(14), None),
        Err(Error::ScopeUnavailable { .. })
    ));
    // project 20 → outside the clip entirely
    assert!(matches!(
        evaluate_control(project, target, bp(20), None),
        Err(Error::ScopeUnavailable { .. })
    ));
    // project 10.5 → note-rel 0.5: step chase holds the point at rel 0
    let eval = evaluate_control(project, target, BeatPoint::new(21, 2).unwrap(), None).unwrap();
    assert!((eval.host_normalized - 0.2).abs() < 1e-9);
}

/// Recorded points land in note-relative coordinates: a write at
/// project pos 11 stores pos=1; a write outside the note domain
/// records nothing.
#[test]
fn pc_b_02_note_scope_records_note_relative() {
    let (mut ws, _clip, _note, target, lane) = note_fixture();
    ws.session_mut()
        .control_with(|c, p| c.set_mode(p, target, AutomationMode::Write))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.set_arm(p, target, WriteArm::Armed))
        .unwrap();
    ws.session_mut()
        .control_with(|c, p| c.bind_write_lane(p, target, lane))
        .unwrap();
    ws.session_mut().control_mut().set_record_enabled(true);
    ws.session_mut().control_mut().transport_play();
    let rev = ws.session().revision();
    ws.session_mut().control_mut().record_start(rev).unwrap();
    // project 11 → clip-rel 3 → note-rel 1
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(11), 0.9))
        .unwrap();
    // outside the note domain → skipped, no transaction point
    ws.session_mut()
        .control_with(|c, p| c.record_write(p, target, bp(15), 0.5))
        .unwrap();
    let outcome = ws.session_mut().control_record_commit().unwrap();
    assert!(outcome.applied);
    let points = &ws.session().project().control.lanes[&lane].regions[0].points;
    assert_eq!(points.len(), 1, "only the in-domain write records");
    assert_eq!(points[0].pos, bp(1));
    assert!((points[0].value - 0.9).abs() < 1e-9);
}

/// Moving the clip shifts the note domain without rewriting points.
#[test]
fn pc_b_02_note_domain_follows_clip_anchor() {
    let (mut ws, clip, _note, target, lane) = note_fixture();
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: AutomationRegion {
                range: span(bp(0), bp(4)),
                interpolation: Interpolation::Step,
                points: vec![pt(bp(0), 0.7)],
            },
            policy: ConflictPolicy::Reject,
        },
    );
    // Shift the clip +4 beats: note domain is now project 14..18.
    apply(
        &mut ws,
        ProjectOp::MoveClip {
            clip_id: clip,
            anchor: TimeAnchor::Musical(MusicalTick::new(7680 + 4 * 960)),
        },
    );
    let project = ws.session().project();
    assert!(matches!(
        evaluate_control(project, target, bp(10), None),
        Err(Error::ScopeUnavailable { .. })
    ));
    let eval = evaluate_control(project, target, bp(14), None).unwrap();
    assert!((eval.host_normalized - 0.7).abs() < 1e-9);
}

/// A note-scoped target pins its note's owning clip: `RemoveClip` is
/// rejected while the reference stands (the post-apply reference
/// oracle never commits a dangling owner). Once lane and target are
/// removed, `RemoveClip` succeeds and cascades the note.
#[test]
fn pc_b_02_clip_removal_cascades_notes() {
    let (mut ws, clip, note, target, lane) = note_fixture();
    let rev = ws.session().revision();
    assert!(matches!(
        ws.session_mut()
            .apply(ProjectOp::RemoveClip { clip_id: clip }, rev),
        Err(Error::NotFound(_))
    ));
    // Nothing was committed — clip and note are still there.
    assert!(ws.session().project().clips.contains_key(&clip));
    assert!(ws.session().project().notes.contains_key(&note));

    apply(&mut ws, ProjectOp::RemoveAutomationLane { lane });
    apply(&mut ws, ProjectOp::RemoveParameterTarget { target });
    apply(&mut ws, ProjectOp::RemoveClip { clip_id: clip });
    assert!(!ws.session().project().notes.contains_key(&note));
}

/// `AddNote` rejects malformed input: key > 127, start+len past the
/// owning clip's musical extent.
#[test]
fn pc_b_02_add_note_validates_domain() {
    let mut ws = fresh_ws();
    apply(&mut ws, ProjectOp::AddTrack { name: "t".into() });
    let track = *ws.session().project().tracks.keys().last().unwrap();
    let clip = seeded_clip(&mut ws, track, 7680, 15360);
    let rev = ws.session().revision();
    assert!(matches!(
        ws.session_mut().apply(
            ProjectOp::AddNote {
                clip,
                key: 200,
                start: bp(0),
                len: bp(1),
            },
            rev
        ),
        Err(Error::Validation(_))
    ));
    // clip extent is 8 beats — 6+4 exceeds it.
    assert!(matches!(
        ws.session_mut().apply(
            ProjectOp::AddNote {
                clip,
                key: 60,
                start: bp(6),
                len: bp(4),
            },
            rev
        ),
        Err(Error::Validation(_))
    ));
}

/// `SplitClip` refuses to cut a straddling note rather than orphaning
/// its tail domain.
#[test]
fn pc_b_02_split_clip_rejects_straddling_note() {
    let (mut ws, clip, _note, _target, _lane) = note_fixture();
    // note spans clip-relative 2..6 — split at 4 cuts it.
    let rev = ws.session().revision();
    assert!(matches!(
        ws.session_mut().apply(
            ProjectOp::SplitClip {
                clip_id: clip,
                at: MusicalTick::new(7680 + 4 * 960),
                clip_b_source_offset: SourceFrame::new(500_000).unwrap(),
            },
            rev
        ),
        Err(Error::Validation(_))
    ));
}

/// A note fully past the split point follows clip B with a re-based
/// start — its evaluation domain is preserved.
#[test]
fn pc_b_02_split_clip_reparents_notes() {
    let (mut ws, clip, note, target, lane) = note_fixture();
    apply(
        &mut ws,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: AutomationRegion {
                range: span(bp(0), bp(4)),
                interpolation: Interpolation::Step,
                points: vec![pt(bp(2), 0.8)],
            },
            policy: ConflictPolicy::Reject,
        },
    );
    // Split at clip-relative beat 1: A = project 8..9, B = 9..16.
    // The note (clip-rel 2..6) moves to B as rel 1..5 — still
    // covering project beats 10..14.
    apply(
        &mut ws,
        ProjectOp::SplitClip {
            clip_id: clip,
            at: MusicalTick::new(7680 + 960),
            clip_b_source_offset: SourceFrame::new(500_000).unwrap(),
        },
    );
    let project = ws.session().project();
    let note_ref = &project.notes[&note];
    assert_ne!(note_ref.clip, clip, "note moved to clip B");
    assert_eq!(note_ref.start, bp(1));
    assert_eq!(note_ref.len, bp(4));
    // Evaluation still resolves the same absolute domain.
    let eval = evaluate_control(project, target, bp(12), None).unwrap();
    assert!((eval.host_normalized - 0.8).abs() < 1e-9);
}
