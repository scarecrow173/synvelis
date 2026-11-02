//! MD-B acceptance: `CompOp` reaches the project controller through
//! the session dispatch surface — issue #50 scope 3.
//!
//! `ProjectSession::apply` is generic over `Operation`, so the comping
//! domain uses the same validated, revision-checked path as
//! `ProjectOp`. Takes/segments are seeded as POD before the session is
//! created (recording ops live in `synvelis-recording`, which is not a
//! workspace dependency).

use synvelis_project::{
    Asset, AssetId, CaptureSegmentId, CaptureStreamId, ClockEpoch, ClockEpochId, CompOp,
    CounterIdGenerator, Error, Lane, LaneId, MusicalRange, MusicalTick, Project, ProjectController,
    ProjectId, ProjectOp, ProjectRange, RecordingPassId, RecordingSessionId, SessionRange,
    SessionState, Source, SourceFrame, SourceId, SourceRange, SourceState, TakeId, Track, TrackId,
    recording_data::{
        CaptureSegment, CaptureStream, RecordingPass, RecordingSession, Take, TakeState,
    },
};
use synvelis_workspace::{FakeWindowHost, ProjectSession, WorkspaceManager};

type Ws = WorkspaceManager<FakeWindowHost>;

fn mr(start: i64, end: i64) -> ProjectRange {
    ProjectRange::Musical(
        MusicalRange::try_new(MusicalTick::new(start), MusicalTick::new(end)).unwrap(),
    )
}

/// A session whose project already carries one lane on one track, one
/// committed source, and one verified take covering `source [0,60_000)`
/// — the minimum a `RegisterTakeGroup` can act on.
#[allow(clippy::too_many_lines)] // sequential POD seed
fn seeded_ws() -> (Ws, LaneId, TakeId) {
    let mut p = Project::new(ProjectId::new(0xB), "md-b-ws");
    let track = TrackId::new(1);
    let lane = LaneId::new(2);
    let asset = AssetId::new(3);
    let source = SourceId::new(4);
    let session = RecordingSessionId::new(5);
    let pass = RecordingPassId::new(6);
    let stream = CaptureStreamId::new(7);
    let segment = CaptureSegmentId::new(8);
    let take = TakeId::new(9);
    let epoch = ClockEpochId::new(10);

    p.tracks.insert(
        track,
        Track {
            id: track,
            name: "t".into(),
            lane_ids: vec![lane],
        },
    );
    p.lanes.insert(
        lane,
        Lane {
            id: lane,
            track_id: track,
            clip_ids: Vec::new(),
        },
    );
    p.assets.insert(
        asset,
        Asset {
            id: asset,
            digest: "d".into(),
        },
    );
    p.sources.insert(
        source,
        Source {
            id: source,
            asset_id: asset,
            content_digest: "d".into(),
            state: SourceState::Committed,
        },
    );
    p.clock_epochs.insert(
        epoch,
        ClockEpoch {
            id: epoch,
            description: "clk".into(),
        },
    );
    p.recording.sessions.insert(
        session,
        RecordingSession {
            id: session,
            name: "s".into(),
            state: SessionState::Finalized,
        },
    );
    p.recording.passes.insert(
        pass,
        RecordingPass {
            id: pass,
            session_id: session,
            index: 0,
        },
    );
    p.recording.streams.insert(
        stream,
        CaptureStream {
            id: stream,
            session_id: session,
            lane_id: lane,
            clock_epoch_id: epoch,
        },
    );
    p.recording.segments.insert(
        segment,
        CaptureSegment {
            id: segment,
            stream_id: stream,
            pass_id: pass,
            session_range: SessionRange::try_new(
                synvelis_project::SessionFrame::new(0).unwrap(),
                synvelis_project::SessionFrame::new(60_000).unwrap(),
            )
            .unwrap(),
            source_range: SourceRange::try_new(
                SourceFrame::ZERO,
                SourceFrame::new(60_000).unwrap(),
            )
            .unwrap(),
            source_id: source,
        },
    );
    p.recording.takes.insert(
        take,
        Take {
            id: take,
            session_id: session,
            pass_id: pass,
            stream_id: stream,
            lane_id: lane,
            source_id: source,
            segment_ids: vec![segment],
            state: TakeState::Verified,
        },
    );

    let controller = ProjectController::new(p, Box::new(CounterIdGenerator::new(100)));
    (
        WorkspaceManager::new(FakeWindowHost::default(), ProjectSession::new(controller)),
        lane,
        take,
    )
}

/// `CompOp` applies through `ProjectSession::apply`: revision advances
/// and the take group lands in the project. A rejected comp op returns
/// the typed error without advancing the revision — the same contract
/// `ProjectOp` gets.
#[test]
fn comp_ops_apply_through_session_dispatch() {
    let (mut ws, lane, take) = seeded_ws();
    let rev0 = ws.session().revision();

    let rev = ws
        .session_mut()
        .apply(
            CompOp::RegisterTakeGroup {
                lane_id: lane,
                target_range: mr(0, 2_000),
                take_ids: vec![take],
                source_rate: 48_000,
            },
            rev0,
        )
        .expect("CompOp applies");
    assert_ne!(rev, rev0);
    assert_eq!(ws.session().revision(), rev);
    assert_eq!(ws.session().project().comp.take_groups.len(), 1);
    let group = *ws
        .session()
        .project()
        .comp
        .take_groups
        .keys()
        .next()
        .unwrap();

    // A second comp op through the same surface.
    ws.session_mut()
        .apply(
            CompOp::CreateComp {
                take_group_id: group,
            },
            rev,
        )
        .expect("CreateComp applies");
    assert_eq!(ws.session().project().comp.comps.len(), 1);

    // A stale expected revision is rejected without touching live state.
    let before = ws.session().revision();
    assert!(matches!(
        ws.session_mut().apply(
            CompOp::CreateComp {
                take_group_id: group
            },
            rev0,
        ),
        Err(Error::StaleRevision { .. })
    ));
    assert_eq!(ws.session().revision(), before);
    assert_eq!(ws.session().project().comp.comps.len(), 1);

    // A validation failure is likewise rejected honestly.
    assert!(
        ws.session_mut()
            .apply(
                CompOp::UnregisterTakeGroup {
                    take_group_id: group,
                },
                before,
            )
            .is_err(),
        "group with comps cannot unregister"
    );
    assert_eq!(ws.session().revision(), before);
}

/// `ProjectOp` and `CompOp` interleave on the same surface with the
/// revision chain intact.
#[test]
fn comp_and_project_ops_share_one_revision_chain() {
    let (mut ws, lane, take) = seeded_ws();
    let mut rev = ws.session().revision();
    rev = ws
        .session_mut()
        .apply(ProjectOp::SetProjectName { name: "n".into() }, rev)
        .unwrap();
    ws.session_mut()
        .apply(
            CompOp::RegisterTakeGroup {
                lane_id: lane,
                target_range: mr(0, 2_000),
                take_ids: vec![take],
                source_rate: 48_000,
            },
            rev,
        )
        .unwrap();
    assert_eq!(ws.session().project().comp.take_groups.len(), 1);
    assert_eq!(ws.session().project().name, "n");
}
