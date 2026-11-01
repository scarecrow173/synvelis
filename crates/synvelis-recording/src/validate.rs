//! Shared validation helpers for recording ops. All checks are
//! read-only and total — a failed check means the op is rejected
//! before any mutation.

use synvelis_project::{
    CaptureSegmentId, CaptureStreamId, Error, LaneId, Project, RecordingPassId, RecordingSessionId,
    SessionState, SourceId,
};

/// The session exists and is in the expected state.
///
/// # Errors
/// [`Error::NotFound`], [`Error::IllegalTransition`].
pub fn require_session(
    project: &Project,
    session_id: RecordingSessionId,
    expected: SessionState,
) -> Result<(), Error> {
    let session = project
        .recording
        .sessions
        .get(&session_id)
        .ok_or_else(|| Error::NotFound(format!("session {session_id}")))?;
    if session.state != expected {
        return Err(Error::IllegalTransition(format!(
            "session {session_id} is {:?}, expected {expected:?}",
            session.state
        )));
    }
    Ok(())
}

/// The session exists and accepts registrations (state `Open`).
///
/// # Errors
/// [`Error::NotFound`], [`Error::IllegalTransition`].
pub fn require_registerable(
    project: &Project,
    session_id: RecordingSessionId,
) -> Result<(), Error> {
    let session = project
        .recording
        .sessions
        .get(&session_id)
        .ok_or_else(|| Error::NotFound(format!("session {session_id}")))?;
    if !crate::session::accepts_registration(session.state) {
        return Err(Error::IllegalTransition(format!(
            "session {session_id} is {:?}; registrations require Open",
            session.state
        )));
    }
    Ok(())
}

/// The pass exists and belongs to `session_id`.
///
/// # Errors
/// [`Error::NotFound`], [`Error::WrongOwner`].
pub fn require_pass(
    project: &Project,
    pass_id: RecordingPassId,
    session_id: RecordingSessionId,
) -> Result<(), Error> {
    let pass = project
        .recording
        .passes
        .get(&pass_id)
        .ok_or_else(|| Error::NotFound(format!("pass {pass_id}")))?;
    if pass.session_id != session_id {
        return Err(Error::WrongOwner(format!("pass {pass_id}")));
    }
    Ok(())
}

/// The stream exists and belongs to `session_id`.
///
/// # Errors
/// [`Error::NotFound`], [`Error::WrongOwner`].
pub fn require_stream(
    project: &Project,
    stream_id: CaptureStreamId,
    session_id: RecordingSessionId,
) -> Result<(), Error> {
    let stream = project
        .recording
        .streams
        .get(&stream_id)
        .ok_or_else(|| Error::NotFound(format!("stream {stream_id}")))?;
    if stream.session_id != session_id {
        return Err(Error::WrongOwner(format!("stream {stream_id}")));
    }
    Ok(())
}

/// The segment exists and its stream belongs to `session_id`.
///
/// # Errors
/// [`Error::NotFound`], [`Error::WrongOwner`].
pub fn require_segment(
    project: &Project,
    segment_id: CaptureSegmentId,
    session_id: RecordingSessionId,
) -> Result<(), Error> {
    let segment = project
        .recording
        .segments
        .get(&segment_id)
        .ok_or_else(|| Error::NotFound(format!("segment {segment_id}")))?;
    let stream = project
        .recording
        .streams
        .get(&segment.stream_id)
        .ok_or_else(|| Error::NotFound(format!("stream {}", segment.stream_id)))?;
    if stream.session_id != session_id {
        return Err(Error::WrongOwner(format!("segment {segment_id}")));
    }
    Ok(())
}

/// The lane exists.
///
/// # Errors
/// [`Error::NotFound`].
pub fn require_lane(project: &Project, lane_id: LaneId) -> Result<(), Error> {
    project
        .lanes
        .contains_key(&lane_id)
        .then_some(())
        .ok_or_else(|| Error::NotFound(format!("lane {lane_id}")))
}

/// The source exists.
///
/// # Errors
/// [`Error::NotFound`].
pub fn require_source(project: &Project, source_id: SourceId) -> Result<(), Error> {
    project
        .sources
        .contains_key(&source_id)
        .then_some(())
        .ok_or_else(|| Error::NotFound(format!("source {source_id}")))
}
