//! AC5/AC6: session state machine and full reference integrity.

use synvelis_project::*;
use synvelis_recording::*;

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "p"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn cmd(c: &ProjectController, op: SynvelisOp) -> Command<SynvelisOp> {
    Command {
        expected_revision: c.live_revision(),
        op,
    }
}

fn rec(op: RecordingOp) -> SynvelisOp {
    SynvelisOp::Recording(op)
}

fn proj(op: ProjectOp) -> SynvelisOp {
    SynvelisOp::Project(op)
}

/// Builds track+lane+asset+source+clock and returns their IDs.
fn scaffold(c: &mut ProjectController) -> (LaneId, SourceId, ClockEpochId) {
    c.apply(cmd(c, proj(ProjectOp::AddTrack { name: "t".into() })))
        .unwrap();
    let track_id = *c.live().tracks.keys().next().unwrap();
    c.apply(cmd(c, proj(ProjectOp::AddLane { track_id })))
        .unwrap();
    let lane_id = *c.live().lanes.keys().next().unwrap();
    c.apply(cmd(
        c,
        proj(ProjectOp::RegisterAsset { digest: "d".into() }),
    ))
    .unwrap();
    let asset_id = *c.live().assets.keys().next().unwrap();
    c.apply(cmd(
        c,
        proj(ProjectOp::RegisterSource {
            asset_id,
            content_digest: "d".into(),
        }),
    ))
    .unwrap();
    let source_id = *c.live().sources.keys().next().unwrap();
    c.apply(cmd(
        c,
        proj(ProjectOp::RegisterClockEpoch {
            description: "clk".into(),
        }),
    ))
    .unwrap();
    let clock_epoch_id = *c.live().clock_epochs.keys().next().unwrap();
    (lane_id, source_id, clock_epoch_id)
}

fn create_and_open(c: &mut ProjectController) -> RecordingSessionId {
    c.apply(cmd(c, rec(RecordingOp::CreateSession { name: "s".into() })))
        .unwrap();
    let session_id = *c.live().recording.sessions.keys().next().unwrap();
    c.apply(cmd(c, rec(RecordingOp::OpenSession { session_id })))
        .unwrap();
    session_id
}

#[test]
fn session_state_machine_legal_path_only() {
    let mut c = controller();
    scaffold(&mut c);
    c.apply(cmd(
        &c,
        rec(RecordingOp::CreateSession { name: "s".into() }),
    ))
    .unwrap();
    let session_id = *c.live().recording.sessions.keys().next().unwrap();
    assert_eq!(
        c.live().recording.sessions[&session_id].state,
        SessionState::Created
    );
    // Illegal: Close/Finalize from Created, and Open twice.
    assert!(
        c.apply(cmd(&c, rec(RecordingOp::CloseSession { session_id })))
            .is_err()
    );
    assert!(
        c.apply(cmd(&c, rec(RecordingOp::FinalizeSession { session_id })))
            .is_err()
    );
    c.apply(cmd(&c, rec(RecordingOp::OpenSession { session_id })))
        .unwrap();
    assert!(
        c.apply(cmd(&c, rec(RecordingOp::OpenSession { session_id })))
            .is_err()
    );
    c.apply(cmd(&c, rec(RecordingOp::CloseSession { session_id })))
        .unwrap();
    // Registration no longer accepted in Closed.
    assert!(
        c.apply(cmd(
            &c,
            rec(RecordingOp::RegisterPass {
                session_id,
                index: 1,
            })
        ))
        .is_err()
    );
    c.apply(cmd(&c, rec(RecordingOp::FinalizeSession { session_id })))
        .unwrap();
    // Nothing transitions out of Finalized.
    assert!(
        c.apply(cmd(&c, rec(RecordingOp::OpenSession { session_id })))
            .is_err()
    );
    assert!(
        c.apply(cmd(&c, rec(RecordingOp::CloseSession { session_id })))
            .is_err()
    );
}

#[test]
fn registration_rejected_before_open() {
    let mut c = controller();
    let (lane_id, _, clock_epoch_id) = scaffold(&mut c);
    c.apply(cmd(
        &c,
        rec(RecordingOp::CreateSession { name: "s".into() }),
    ))
    .unwrap();
    let session_id = *c.live().recording.sessions.keys().next().unwrap();
    // Created: no registrations.
    assert!(
        c.apply(cmd(
            &c,
            rec(RecordingOp::RegisterPass {
                session_id,
                index: 0,
            })
        ))
        .is_err()
    );
    assert!(
        c.apply(cmd(
            &c,
            rec(RecordingOp::RegisterStream {
                session_id,
                lane_id,
                clock_epoch_id,
            })
        ))
        .is_err()
    );
}

#[test]
#[allow(clippy::too_many_lines)] // sequential assertion scenario
fn full_reference_integrity_is_validated() {
    let mut c = controller();
    let (lane_id, source_id, clock_epoch_id) = scaffold(&mut c);
    let session_id = create_and_open(&mut c);

    // Pass registration requires existing session + unique index.
    c.apply(cmd(
        &c,
        rec(RecordingOp::RegisterPass {
            session_id,
            index: 0,
        }),
    ))
    .unwrap();
    let pass_id = *c.live().recording.passes.keys().next().unwrap();
    assert!(
        c.apply(cmd(
            &c,
            rec(RecordingOp::RegisterPass {
                session_id,
                index: 0,
            })
        ))
        .is_err()
    ); // duplicate index
    assert!(
        c.apply(cmd(
            &c,
            rec(RecordingOp::RegisterPass {
                session_id: RecordingSessionId::new(999),
                index: 1,
            })
        ))
        .is_err()
    ); // missing session

    // Stream requires existing lane + clock epoch + session.
    assert!(
        c.apply(cmd(
            &c,
            rec(RecordingOp::RegisterStream {
                session_id,
                lane_id: LaneId::new(999),
                clock_epoch_id,
            })
        ))
        .is_err()
    );
    assert!(
        c.apply(cmd(
            &c,
            rec(RecordingOp::RegisterStream {
                session_id,
                lane_id,
                clock_epoch_id: ClockEpochId::new(999),
            })
        ))
        .is_err()
    );
    c.apply(cmd(
        &c,
        rec(RecordingOp::RegisterStream {
            session_id,
            lane_id,
            clock_epoch_id,
        }),
    ))
    .unwrap();
    let stream_id = *c.live().recording.streams.keys().next().unwrap();

    // Segment requires pass+stream in same session, existing source,
    // non-empty ranges.
    let range = |a, b| {
        SessionRange::try_new(SessionFrame::new(a).unwrap(), SessionFrame::new(b).unwrap()).unwrap()
    };
    let srange = |a, b| {
        SourceRange::try_new(SourceFrame::new(a).unwrap(), SourceFrame::new(b).unwrap()).unwrap()
    };
    assert!(
        c.apply(cmd(
            &c,
            rec(RecordingOp::RegisterSegment {
                stream_id,
                pass_id,
                session_range: range(0, 0), // empty rejected
                source_range: srange(0, 10),
                source_id,
            })
        ))
        .is_err()
    );
    assert!(
        c.apply(cmd(
            &c,
            rec(RecordingOp::RegisterSegment {
                stream_id: CaptureStreamId::new(999),
                pass_id,
                session_range: range(0, 10),
                source_range: srange(0, 10),
                source_id,
            })
        ))
        .is_err()
    );
    c.apply(cmd(
        &c,
        rec(RecordingOp::RegisterSegment {
            stream_id,
            pass_id,
            session_range: range(0, 96_000),
            source_range: srange(0, 96_000),
            source_id,
        }),
    ))
    .unwrap();
    let segment_id = *c.live().recording.segments.keys().next().unwrap();

    // Take: wrong-owner stream, missing pass, empty segments rejected.
    assert!(
        c.apply(cmd(
            &c,
            rec(RecordingOp::RegisterTake {
                session_id,
                pass_id,
                stream_id,
                lane_id,
                source_id,
                segment_ids: vec![],
            })
        ))
        .is_err()
    );
    assert!(
        c.apply(cmd(
            &c,
            rec(RecordingOp::RegisterTake {
                session_id,
                pass_id: RecordingPassId::new(999),
                stream_id,
                lane_id,
                source_id,
                segment_ids: vec![segment_id],
            })
        ))
        .is_err()
    );
    c.apply(cmd(
        &c,
        rec(RecordingOp::RegisterTake {
            session_id,
            pass_id,
            stream_id,
            lane_id,
            source_id,
            segment_ids: vec![segment_id],
        }),
    ))
    .unwrap();
    let take_id = *c.live().recording.takes.keys().next().unwrap();

    // Cross-session references: build a second open session and try to
    // attach session A's segment to a take in session B.
    c.apply(cmd(
        &c,
        rec(RecordingOp::CreateSession { name: "s2".into() }),
    ))
    .unwrap();
    let session2 = *c
        .live()
        .recording
        .sessions
        .keys()
        .find(|id| **id != session_id)
        .unwrap();
    c.apply(cmd(
        &c,
        rec(RecordingOp::OpenSession {
            session_id: session2,
        }),
    ))
    .unwrap();
    c.apply(cmd(
        &c,
        rec(RecordingOp::RegisterPass {
            session_id: session2,
            index: 0,
        }),
    ))
    .unwrap();
    let pass2 = *c
        .live()
        .recording
        .passes
        .keys()
        .find(|id| **id != pass_id)
        .unwrap();
    c.apply(cmd(
        &c,
        rec(RecordingOp::RegisterStream {
            session_id: session2,
            lane_id,
            clock_epoch_id,
        }),
    ))
    .unwrap();
    let stream2 = *c
        .live()
        .recording
        .streams
        .keys()
        .find(|id| **id != stream_id)
        .unwrap();
    assert!(
        c.apply(cmd(
            &c,
            rec(RecordingOp::RegisterTake {
                session_id: session2,
                pass_id: pass2,
                stream_id: stream2,
                lane_id,
                source_id,
                segment_ids: vec![segment_id], // belongs to session A
            })
        ))
        .is_err()
    );

    // MarkTakeVerified works in Open and Closed, not Finalized.
    c.apply(cmd(&c, rec(RecordingOp::MarkTakeVerified { take_id })))
        .unwrap();
    assert_eq!(
        c.live().recording.takes[&take_id].state,
        TakeState::Verified
    );
    c.apply(cmd(&c, rec(RecordingOp::CloseSession { session_id })))
        .unwrap();
    // Closed still allows verification transitions.
    c.apply(cmd(&c, rec(RecordingOp::MarkTakeVerified { take_id })))
        .unwrap();
    c.apply(cmd(&c, rec(RecordingOp::FinalizeSession { session_id })))
        .unwrap();
    assert!(
        c.apply(cmd(&c, rec(RecordingOp::MarkTakeVerified { take_id })))
            .is_err()
    );
}

#[test]
fn recording_ids_survive_save_reload() {
    let mut c = controller();
    let (lane_id, source_id, clock_epoch_id) = scaffold(&mut c);
    let session_id = create_and_open(&mut c);
    c.apply(cmd(
        &c,
        rec(RecordingOp::RegisterPass {
            session_id,
            index: 0,
        }),
    ))
    .unwrap();
    let pass_id = *c.live().recording.passes.keys().next().unwrap();
    c.apply(cmd(
        &c,
        rec(RecordingOp::RegisterStream {
            session_id,
            lane_id,
            clock_epoch_id,
        }),
    ))
    .unwrap();
    let stream_id = *c.live().recording.streams.keys().next().unwrap();
    c.apply(cmd(
        &c,
        rec(RecordingOp::RegisterSegment {
            stream_id,
            pass_id,
            session_range: SessionRange::try_new(
                SessionFrame::new(0).unwrap(),
                SessionFrame::new(100).unwrap(),
            )
            .unwrap(),
            source_range: SourceRange::try_new(
                SourceFrame::new(0).unwrap(),
                SourceFrame::new(100).unwrap(),
            )
            .unwrap(),
            source_id,
        }),
    ))
    .unwrap();
    let segment_id = *c.live().recording.segments.keys().next().unwrap();
    c.apply(cmd(
        &c,
        rec(RecordingOp::RegisterTake {
            session_id,
            pass_id,
            stream_id,
            lane_id,
            source_id,
            segment_ids: vec![segment_id],
        }),
    ))
    .unwrap();
    let take_id = *c.live().recording.takes.keys().next().unwrap();

    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();

    assert!(c2.live().recording.sessions.contains_key(&session_id));
    assert!(c2.live().recording.passes.contains_key(&pass_id));
    assert!(c2.live().recording.streams.contains_key(&stream_id));
    assert!(c2.live().recording.segments.contains_key(&segment_id));
    assert!(c2.live().recording.takes.contains_key(&take_id));
    assert_eq!(c.live(), c2.live());
    // Reloaded snapshot passes the reference validator.
    c2.live().validate_references().unwrap();
}
