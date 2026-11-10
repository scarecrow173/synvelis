//! Read-only projection of the persisted `Project.recording` registry
//! plus recorder-slot status — the `syn.recording.info` data surface
//! (synvelis#115 RCD-01 headless slice).
//!
//! This is a state projection only: it exposes what the project
//! aggregate already persists (sessions/passes/streams/segments/
//! takes), the declared recorder slot, and whether a recording
//! transaction is open. Recording commands, arm/punch UI, and
//! real-device wiring are out of scope here (#114 / SV-GUI-05,
//! ruviform#251).

use serde_json::{Value, json};

use synvelis_project::{SessionState, TakeState};
use synvelis_workspace::ProjectSession;

use crate::app::RecorderSlot;

fn session_state(state: SessionState) -> &'static str {
    match state {
        SessionState::Created => "created",
        SessionState::Open => "open",
        SessionState::Closed => "closed",
        SessionState::Finalized => "finalized",
    }
}

fn take_state(state: TakeState) -> &'static str {
    match state {
        TakeState::Raw => "raw",
        TakeState::Verified => "verified",
    }
}

/// `syn.recording.info` payload — persisted recording entities plus
/// recorder status. Read-only; never mutates the session.
#[must_use]
pub fn recording_info(session: &ProjectSession, recorder: &RecorderSlot) -> Value {
    let project = session.project();
    let reg = &project.recording;
    let sessions: Vec<Value> = reg
        .sessions
        .values()
        .map(|s| {
            json!({
                "id": s.id.to_tagged(),
                "name": s.name,
                "state": session_state(s.state),
            })
        })
        .collect();
    let passes: Vec<Value> = reg
        .passes
        .values()
        .map(|p| {
            json!({
                "id": p.id.to_tagged(),
                "session_id": p.session_id.to_tagged(),
                "index": p.index,
            })
        })
        .collect();
    let streams: Vec<Value> = reg
        .streams
        .values()
        .map(|s| {
            json!({
                "id": s.id.to_tagged(),
                "session_id": s.session_id.to_tagged(),
                "lane_id": s.lane_id.to_tagged(),
                "clock_epoch_id": s.clock_epoch_id.to_tagged(),
            })
        })
        .collect();
    let segments: Vec<Value> = reg
        .segments
        .values()
        .map(|g| {
            json!({
                "id": g.id.to_tagged(),
                "stream_id": g.stream_id.to_tagged(),
                "pass_id": g.pass_id.to_tagged(),
                "source_id": g.source_id.to_tagged(),
                "session_start": g.session_range.start().value(),
                "session_end": g.session_range.end().value(),
                "source_start": g.source_range.start().value(),
                "source_end": g.source_range.end().value(),
            })
        })
        .collect();
    let takes: Vec<Value> = reg
        .takes
        .values()
        .map(|t| {
            json!({
                "id": t.id.to_tagged(),
                "session_id": t.session_id.to_tagged(),
                "pass_id": t.pass_id.to_tagged(),
                "stream_id": t.stream_id.to_tagged(),
                "lane_id": t.lane_id.to_tagged(),
                "source_id": t.source_id.to_tagged(),
                "state": take_state(t.state),
                "segment_count": t.segment_ids.len(),
            })
        })
        .collect();
    json!({
        "sessions": sessions,
        "passes": passes,
        "streams": streams,
        "segments": segments,
        "takes": takes,
        "counts": {
            "sessions": reg.sessions.len(),
            "passes": reg.passes.len(),
            "streams": reg.streams.len(),
            "segments": reg.segments.len(),
            "takes": reg.takes.len(),
        },
        "recorder": {
            "status": recorder.status(),
            "active": recorder.active(),
            "transaction_open": session.control().recording().is_some(),
        },
    })
}
