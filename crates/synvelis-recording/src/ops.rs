//! `RecordingOp`: recording-domain operations, and the `SynvelisOp`
//! sum type that a [`ProjectController`] consumes.
//!
//! All registrations run inside the single `Project` aggregate through
//! the controller — there is no second writer and no second store.

use synvelis_project::{
    CaptureSegmentId, CaptureStreamId, ClockEpochId, Error, IdGenerator, LaneId, Operation,
    Project, ProjectOp, RecordingPassId, RecordingSessionId, SessionRange, SessionState, SourceId,
    SourceRange, TakeId, TakeState,
    recording_data::{CaptureSegment, CaptureStream, RecordingPass, RecordingSession, Take},
};

use crate::{
    session,
    validate::{
        require_lane, require_pass, require_registerable, require_segment, require_session,
        require_source,
    },
};

/// Recording-domain operations.
#[derive(Clone, Debug)]
pub enum RecordingOp {
    /// Create a session in `Created` state (the only `Created` entry).
    CreateSession {
        /// Display name (immutable in F0).
        name: String,
    },
    /// `Created → Open`.
    OpenSession {
        /// Target session.
        session_id: RecordingSessionId,
    },
    /// `Open → Closed`.
    CloseSession {
        /// Target session.
        session_id: RecordingSessionId,
    },
    /// `Closed → Finalized`.
    FinalizeSession {
        /// Target session.
        session_id: RecordingSessionId,
    },
    /// Register a pass (`Open` only, unique index within the session).
    RegisterPass {
        /// Owning session.
        session_id: RecordingSessionId,
        /// Ordinal within the session.
        index: u32,
    },
    /// Register a capture stream (`Open` only).
    RegisterStream {
        /// Owning session.
        session_id: RecordingSessionId,
        /// Lane the stream feeds.
        lane_id: LaneId,
        /// Clock epoch the session frames are measured in.
        clock_epoch_id: ClockEpochId,
    },
    /// Register a capture segment (`Open` only; both ranges non-empty).
    RegisterSegment {
        /// Producing stream (determines the session).
        stream_id: CaptureStreamId,
        /// Pass during which it was captured.
        pass_id: RecordingPassId,
        /// Session-time `[start, end)` (non-empty).
        session_range: SessionRange,
        /// Source-time `[start, end)` (non-empty).
        source_range: SourceRange,
        /// Media source holding the captured material.
        source_id: SourceId,
    },
    /// Register a take (`Open` only; `segment_ids` non-empty and all
    /// within the session).
    RegisterTake {
        /// Owning session.
        session_id: RecordingSessionId,
        /// Pass in which it was taken.
        pass_id: RecordingPassId,
        /// Producing stream.
        stream_id: CaptureStreamId,
        /// Lane it occupies (must equal the stream's lane).
        lane_id: LaneId,
        /// Media source it reads.
        source_id: SourceId,
        /// Composing segments.
        segment_ids: Vec<CaptureSegmentId>,
    },
    /// `TakeState::Raw → Verified` (`Open` or `Closed` session).
    MarkTakeVerified {
        /// Target take.
        take_id: TakeId,
    },
    /// Apply a persisted commit manifest as one Project mutation
    /// (VS-D: `Closed → Finalized` session + assets/sources/passes/
    /// segments/takes/clips + applied-commit registry entry). All IDs
    /// come from the manifest reserved before commit — nothing is
    /// re-generated here.
    CommitRecording {
        /// The commit manifest (identity + reserved stable IDs +
        /// content hash + full payload). PCM is never carried; the
        /// payload holds references, digests and ranges only.
        manifest: Box<crate::commit::RecordingCommitManifest>,
    },
}

impl Operation for RecordingOp {
    #[allow(clippy::too_many_lines)] // flat match per op variant
    fn validate(&self, project: &Project) -> Result<(), Error> {
        match self {
            Self::CreateSession { .. } => Ok(()),
            Self::OpenSession { session_id } => {
                require_session(project, *session_id, SessionState::Created)
            }
            Self::CloseSession { session_id } => {
                require_session(project, *session_id, SessionState::Open)
            }
            Self::FinalizeSession { session_id } => {
                require_session(project, *session_id, SessionState::Closed)
            }
            Self::RegisterPass { session_id, index } => {
                require_registerable(project, *session_id)?;
                if project
                    .recording
                    .passes
                    .values()
                    .any(|p| p.session_id == *session_id && p.index == *index)
                {
                    return Err(Error::Validation(format!(
                        "duplicate pass index {index} in session {session_id}"
                    )));
                }
                Ok(())
            }
            Self::RegisterStream {
                session_id,
                lane_id,
                clock_epoch_id,
            } => {
                require_registerable(project, *session_id)?;
                require_lane(project, *lane_id)?;
                project
                    .clock_epochs
                    .contains_key(clock_epoch_id)
                    .then_some(())
                    .ok_or_else(|| Error::NotFound(format!("clock epoch {clock_epoch_id}")))
            }
            Self::RegisterSegment {
                stream_id,
                pass_id,
                session_range,
                source_range,
                source_id,
            } => {
                let stream = project
                    .recording
                    .streams
                    .get(stream_id)
                    .ok_or_else(|| Error::NotFound(format!("stream {stream_id}")))?;
                require_registerable(project, stream.session_id)?;
                require_pass(project, *pass_id, stream.session_id)?;
                require_source(project, *source_id)?;
                if session_range.is_empty() || source_range.is_empty() {
                    return Err(Error::Validation(
                        "capture segment ranges must be non-empty".into(),
                    ));
                }
                Ok(())
            }
            Self::RegisterTake {
                session_id,
                pass_id,
                stream_id,
                lane_id,
                source_id,
                segment_ids,
            } => {
                require_registerable(project, *session_id)?;
                require_pass(project, *pass_id, *session_id)?;
                let stream = project
                    .recording
                    .streams
                    .get(stream_id)
                    .ok_or_else(|| Error::NotFound(format!("stream {stream_id}")))?;
                if stream.session_id != *session_id {
                    return Err(Error::WrongOwner(format!("stream {stream_id}")));
                }
                if stream.lane_id != *lane_id {
                    return Err(Error::WrongOwner(format!(
                        "take lane {lane_id} != stream lane {}",
                        stream.lane_id
                    )));
                }
                require_source(project, *source_id)?;
                if segment_ids.is_empty() {
                    return Err(Error::Validation("take requires segments".into()));
                }
                for seg_id in segment_ids {
                    require_segment(project, *seg_id, *session_id)?;
                }
                Ok(())
            }
            Self::MarkTakeVerified { take_id } => {
                let take = project
                    .recording
                    .takes
                    .get(take_id)
                    .ok_or_else(|| Error::NotFound(format!("take {take_id}")))?;
                let session_state = project
                    .recording
                    .sessions
                    .get(&take.session_id)
                    .ok_or_else(|| Error::NotFound(format!("session {}", take.session_id)))?
                    .state;
                if !session::accepts_take_verification(session_state) {
                    return Err(Error::IllegalTransition(format!(
                        "session {} is {session_state:?}; verification requires Open|Closed",
                        take.session_id
                    )));
                }
                Ok(())
            }
            Self::CommitRecording { manifest } => crate::commit::validate_commit(project, manifest),
        }
    }

    #[allow(clippy::too_many_lines)] // flat match per op variant
    fn apply(&self, project: &mut Project, ids: &mut dyn IdGenerator) -> Result<(), Error> {
        match self {
            Self::CreateSession { name } => {
                let id = RecordingSessionId::new(ids.next());
                project.recording.sessions.insert(
                    id,
                    RecordingSession {
                        id,
                        name: name.clone(),
                        state: SessionState::Created,
                    },
                );
            }
            Self::OpenSession { session_id }
            | Self::CloseSession { session_id }
            | Self::FinalizeSession { session_id } => {
                if let Some(session) = project.recording.sessions.get_mut(session_id) {
                    session.state = match session.state {
                        SessionState::Created => SessionState::Open,
                        SessionState::Open => SessionState::Closed,
                        SessionState::Closed | SessionState::Finalized => SessionState::Finalized,
                    };
                }
            }
            Self::RegisterPass { session_id, index } => {
                let id = RecordingPassId::new(ids.next());
                project.recording.passes.insert(
                    id,
                    RecordingPass {
                        id,
                        session_id: *session_id,
                        index: *index,
                    },
                );
            }
            Self::RegisterStream {
                session_id,
                lane_id,
                clock_epoch_id,
            } => {
                let id = CaptureStreamId::new(ids.next());
                project.recording.streams.insert(
                    id,
                    CaptureStream {
                        id,
                        session_id: *session_id,
                        lane_id: *lane_id,
                        clock_epoch_id: *clock_epoch_id,
                    },
                );
            }
            Self::RegisterSegment {
                stream_id,
                pass_id,
                session_range,
                source_range,
                source_id,
            } => {
                let id = CaptureSegmentId::new(ids.next());
                project.recording.segments.insert(
                    id,
                    CaptureSegment {
                        id,
                        stream_id: *stream_id,
                        pass_id: *pass_id,
                        session_range: *session_range,
                        source_range: *source_range,
                        source_id: *source_id,
                    },
                );
            }
            Self::RegisterTake {
                session_id,
                pass_id,
                stream_id,
                lane_id,
                source_id,
                segment_ids,
            } => {
                let id = TakeId::new(ids.next());
                project.recording.takes.insert(
                    id,
                    Take {
                        id,
                        session_id: *session_id,
                        pass_id: *pass_id,
                        stream_id: *stream_id,
                        lane_id: *lane_id,
                        source_id: *source_id,
                        segment_ids: segment_ids.clone(),
                        state: TakeState::Raw,
                    },
                );
            }
            Self::MarkTakeVerified { take_id } => {
                if let Some(take) = project.recording.takes.get_mut(take_id) {
                    take.state = TakeState::Verified;
                }
            }
            Self::CommitRecording { manifest } => {
                crate::commit::apply_commit(project, manifest, ids)?;
            }
        }
        Ok(())
    }
}

/// The operation sum type used by the `PoC` and tests: project ops plus
/// recording ops under one controller.
#[derive(Clone, Debug)]
pub enum SynvelisOp {
    /// A project-domain operation.
    Project(ProjectOp),
    /// A recording-domain operation.
    Recording(RecordingOp),
}

impl Operation for SynvelisOp {
    fn validate(&self, project: &Project) -> Result<(), Error> {
        match self {
            Self::Project(op) => op.validate(project),
            Self::Recording(op) => op.validate(project),
        }
    }

    fn apply(&self, project: &mut Project, ids: &mut dyn IdGenerator) -> Result<(), Error> {
        match self {
            Self::Project(op) => op.apply(project, ids),
            Self::Recording(op) => op.apply(project, ids),
        }
    }
}
