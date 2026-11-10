//! `syn.recording.info` projection fixture（synvelis#115 RCD-01
//! headless slice）。
//!
//! `Project.recording` の persisted entity（session/pass/stream/
//! segment/take）と recorder slot 状態が read-only projection として
//! 返ることを検証する。録音 command・arm/punch UI・実 Device 配線は
//! 本 slice の対象外（#114 / SV-GUI-05、ruviform#251）。

use serde_json::{Value, json};

use ruviform_ui::{BackendReport, DataRegistry, UiRuntime};

use synvelis_app::recording::recording_info;
use synvelis_app::{AppService, RecorderSlot, SynvelisApp};
use synvelis_project::{
    CaptureSegment, CaptureSegmentId, CaptureStream, CaptureStreamId, ClockEpochId,
    CounterIdGenerator, LaneId, Project, ProjectController, ProjectId, RecordingPass,
    RecordingPassId, RecordingSession, RecordingSessionId, SessionFrame, SessionRange,
    SessionState, SourceFrame, SourceId, SourceRange, Take, TakeId, TakeState,
};
use synvelis_workspace::ProjectSession;

fn session_with(project: Project) -> ProjectSession {
    ProjectSession::new(ProjectController::new(
        project,
        Box::new(CounterIdGenerator::new(1)),
    ))
}

fn empty_project() -> Project {
    Project::new(ProjectId::new(1), "recording-fixture")
}

fn frames(start: i64, end: i64) -> SessionRange {
    SessionRange::try_new(
        SessionFrame::new(start).expect("session frame"),
        SessionFrame::new(end).expect("session frame"),
    )
    .expect("session range")
}

fn source_frames(start: i64, end: i64) -> SourceRange {
    SourceRange::try_new(
        SourceFrame::new(start).expect("source frame"),
        SourceFrame::new(end).expect("source frame"),
    )
    .expect("source range")
}

/// session 1 件・pass 1・stream 1・segment 2・take 1（Verified）を
/// 持つ project を組み立てる。
fn populated_project() -> Project {
    let mut p = empty_project();
    let session_id = RecordingSessionId::new(0xa1);
    let pass_id = RecordingPassId::new(0xb1);
    let stream_id = CaptureStreamId::new(0xc1);
    let lane_id = LaneId::new(7);
    let epoch_id = ClockEpochId::new(3);
    let source_id = SourceId::new(0xe1);
    let seg_a = CaptureSegmentId::new(0xd1);
    let seg_b = CaptureSegmentId::new(0xd2);

    p.recording.sessions.insert(
        session_id,
        RecordingSession {
            id: session_id,
            name: "take-1".to_string(),
            state: SessionState::Finalized,
        },
    );
    p.recording.passes.insert(
        pass_id,
        RecordingPass {
            id: pass_id,
            session_id,
            index: 0,
        },
    );
    p.recording.streams.insert(
        stream_id,
        CaptureStream {
            id: stream_id,
            session_id,
            lane_id,
            clock_epoch_id: epoch_id,
        },
    );
    for (id, s, e) in [(seg_a, 0, 24_000), (seg_b, 24_000, 48_000)] {
        p.recording.segments.insert(
            id,
            CaptureSegment {
                id,
                stream_id,
                pass_id,
                session_range: frames(s, e),
                source_range: source_frames(s, e),
                source_id,
            },
        );
    }
    p.recording.takes.insert(
        TakeId::new(0xf1),
        Take {
            id: TakeId::new(0xf1),
            session_id,
            pass_id,
            stream_id,
            lane_id,
            source_id,
            segment_ids: vec![seg_a, seg_b],
            state: TakeState::Verified,
        },
    );
    p
}

#[test]
fn empty_project_projects_empty_registry_and_idle_recorder() {
    let session = session_with(empty_project());
    let info = recording_info(&session, &RecorderSlot::default());
    assert_eq!(
        info["counts"],
        json!({
            "sessions": 0,
            "passes": 0,
            "streams": 0,
            "segments": 0,
            "takes": 0,
        })
    );
    for key in ["sessions", "passes", "streams", "segments", "takes"] {
        assert_eq!(info[key], json!([]), "{key} must be empty");
    }
    assert_eq!(
        info["recorder"],
        json!({ "status": "idle", "active": false, "transaction_open": false })
    );
}

#[test]
fn populated_registry_projects_all_entity_rows() {
    let session = session_with(populated_project());
    let info = recording_info(&session, &RecorderSlot::default());

    assert_eq!(info["counts"]["sessions"], 1);
    assert_eq!(info["counts"]["passes"], 1);
    assert_eq!(info["counts"]["streams"], 1);
    assert_eq!(info["counts"]["segments"], 2);
    assert_eq!(info["counts"]["takes"], 1);

    let s = &info["sessions"][0];
    assert_eq!(s["id"], RecordingSessionId::new(0xa1).to_tagged());
    assert_eq!(s["name"], "take-1");
    assert_eq!(s["state"], "finalized");

    let pass = &info["passes"][0];
    assert_eq!(
        pass["session_id"],
        RecordingSessionId::new(0xa1).to_tagged()
    );
    assert_eq!(pass["index"], 0);

    let stream = &info["streams"][0];
    assert_eq!(stream["lane_id"], LaneId::new(7).to_tagged());
    assert_eq!(stream["clock_epoch_id"], ClockEpochId::new(3).to_tagged());

    let segs = info["segments"].as_array().expect("segments array");
    assert_eq!(segs.len(), 2);
    let first = segs
        .iter()
        .find(|g| g["id"] == CaptureSegmentId::new(0xd1).to_tagged())
        .expect("segment d1");
    assert_eq!(first["session_start"], 0);
    assert_eq!(first["session_end"], 24_000);
    assert_eq!(first["source_start"], 0);
    assert_eq!(first["source_end"], 24_000);
    assert_eq!(first["pass_id"], RecordingPassId::new(0xb1).to_tagged());
    assert_eq!(first["stream_id"], CaptureStreamId::new(0xc1).to_tagged());

    let take = &info["takes"][0];
    assert_eq!(take["state"], "verified");
    assert_eq!(take["segment_count"], 2);
    assert_eq!(take["lane_id"], LaneId::new(7).to_tagged());
}

/// `syn.recording.info` op が service surface に登録され、fresh
/// project で空 projection を返すこと（op wiring の検証）。
#[test]
fn service_op_returns_recording_projection() {
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(AppService::new(SynvelisApp::new(BackendReport::available(
        "webview2",
        "test-1.0".to_string(),
    ))))
    .expect("register AppService");
    let events = rt.dispatch_op("test-1", "syn.recording.info", json!({}));
    let result = events
        .iter()
        .find_map(|e| match e {
            ruviform_ui::ipc::Event::Result {
                ok, data, error, ..
            } => Some((*ok, data.clone(), error.clone())),
            _ => None,
        })
        .expect("result event");
    assert!(result.0, "op failed: {:?}", result.2);
    let info: Value = result.1.expect("result data");
    assert_eq!(info["counts"]["sessions"], 0);
    assert_eq!(info["recorder"]["status"], "idle");
    // op は OPS 一覧にも載る。
    assert!(
        synvelis_app::OPS
            .iter()
            .any(|op| op.name == "syn.recording.info")
    );
}
