//! `synvelis-project`: the Synvelis project aggregate and its single
//! writer, [`ProjectController`].
//!
//! Owns every persisted value type (stable IDs, time types, ranges,
//! tempo map snapshot, project entities, recording registry POD data),
//! the [`Operation`]/[`Command`] contract, revision/undo semantics, and
//! the [`ProjectStore`] persistence port. Recording-domain *behavior*
//! (ops, transition rules, registration validation) lives in
//! `synvelis-recording`, which depends on this crate — never the reverse.

#![warn(missing_docs)]

pub mod comp;
pub mod control;
pub mod controller;
pub mod diff;
pub mod error;
pub mod id;
pub mod midi;
pub mod midi_comp;
pub(crate) mod migration;
pub mod ops;
pub mod placement;
pub mod plugin_state;
pub mod probe;
pub mod probe_trace;
pub mod project;
pub mod range;
pub mod recording_data;
pub mod relation;
pub mod routing;
pub mod routing_build;
pub mod routing_tx;
pub mod smf;
pub mod snapshot;
pub mod store;
pub mod tempo;
pub mod time;
pub mod trace;

pub use comp::{
    Comp, CompCandidateRef, CompOp, CompProvenance, CompRegion, CompRegistry, CompTransition,
    EditorDisplayTarget, EditorEditingTarget, EffectiveComp, EffectiveSegment, EffectiveTransition,
    FadeShape, SourceBinding, SourceVersionRef, TakeGroup, project_comp,
};
pub use control::{
    AutomationConflict, AutomationCurveView, AutomationLane, AutomationPoint, AutomationRegion,
    AutomationTimeScope, BeatPoint, BeatRange, ConflictPolicy, Contribution, ControlEvaluation,
    ControlGraph, ControlModel, ControlOwner, CurveFeature, CurveFeatureKind, CurveQuality,
    CurveQuery, EvalPlan, EvalScratch, Interpolation, LfoShape, MASTER_FADER_RAW,
    ModulationAssignment, ModulationSource, ParamMapping, ParameterRole, ParameterTarget, Polarity,
    ReferenceSource, SourceKind, TargetCapability, Transform, ValueKind, ViewBin, automation_view,
    cut_range_points, detect_conflicts, dispatch_check, evaluate_control, lane_value, lower_event,
    modulation_allowed, quantize_value, resolve_scope_pos, validate_control_binding,
    validate_control_owner,
};
pub use controller::{Command, Operation, ProjectController, ProjectRevision, SavedState};
pub use error::Error;
pub use id::{
    AssetId, AutomationLaneId, BusId, CaptureSegmentId, CaptureStreamId, ChainId, ClipId,
    ClockEpochId, CompId, CompRegionId, CompTransitionId, CounterIdGenerator, IdGenerator,
    ImportId, LaneId, MidiCompRegionId, ModulationAssignmentId, ModulationSourceId, NoteId,
    ParameterTargetId, ProcessorId, ProjectId, RecordingPassId, RecordingSessionId,
    RoutingConnectionId, SemanticPortId, SendId, SourceId, StripId, TakeGroupId, TakeId, TrackId,
};
pub use midi::{MidiDivision, MidiNote, MidiSource};
pub use midi_comp::{
    MidiCompSelection, MidiControllerEvent, MidiControllerKind, MidiControllerLane,
    MidiControllerLanes, MidiPhraseRegion, MidiPlanAction, MidiPlanEvent, MidiPlaybackPlan,
    MidiRegionReport, compile_midi_comp,
};
pub use ops::ProjectOp;
pub use placement::{
    ClipExtent, ClipPlacement, ClipPlayback, PlaybackMapping, ResolvedClip, StretchRatio,
    TimeAnchor,
};
pub use plugin_state::{
    CaptureMode, CaptureObservation, DirectoryPluginStateStore, InMemoryPluginStateStore,
    PLUGIN_STATE_SCHEMA_VERSION, PluginStateBinding, PluginStateCapture, PluginStateCaptureError,
    PluginStateError, PluginStateFailureKind, PluginStateManifest, PluginStateOutcome,
    PluginStatePort, PluginStateQualitySummary, PluginStateReceipt, PluginStateRecord,
    PluginStateRestoreOutcome, PluginStateRestoreReceipt, PluginStateRestoreRecord,
    PluginStateRestoreRequest, PluginStateSaveRequest, PluginStateSession, PluginStateStore,
    PluginStateStoreError, StateCaptureQuality, StateQualityPolicy, digest_hex, is_manifest_name,
    plugin_state_manifest_name,
};
pub use probe::{
    CaptureCompleteness, CaptureSegmentEnd, CaptureSessionId, ClockContextRef, ObservationGap,
    ObservationGapKind, ProbeCaptureSegment, ProbeFrameInterval, ProbeMode, ProbePointResolution,
    ProbeResolveState, ProbeStatusKind, ProbeStatusReceipt, ProbeStopReason, ProbeSubscriptionKey,
    ProbeUnobservedReason, ProjectSessionId, RuntimeProbeId, SemanticProbeBinding,
    SemanticProbePoint, SemanticSignalEntityRef, SignalBoundary, SubscriptionKind,
    resolve_probe_point,
};
pub use probe_trace::{ProbeObservationUpdate, ProbePortError, ProbeTracePort};
pub use project::{
    AppliedImport, AppliedRecordingCommit, Asset, Clip, Lane, Note, Project, ProjectDomain,
    ProjectRange, RecordingRegistry, Source, SourceState, Track,
};
pub use range::{MediaSpan, MidiRange, MusicalRange, Range, SessionRange, SourceRange};
pub use recording_data::{
    CaptureSegment, CaptureStream, ClockEpoch, RecordingPass, RecordingSession, SessionState, Take,
    TakeState,
};
pub use relation::{
    ControlEntityRef, ControlRelation, RelationCoverage, RelationDirection, RelationIndex,
    RelationKind, RelationQuery, RelationReply, RelationState,
};
pub use routing::{
    Bus, BusKind, ChannelStrip, ConnectionKind, ExplicitChannelMap, PluginRef, PortBinding,
    PortDirection, PortOwner, PortRole, Processor, ProcessorBinding, ProcessorChain,
    RoutingConnection, RoutingModel, SemanticPort, SendConfig, SendTap, SignalDomain, StripOwner,
    WELL_KNOWN_BASE, is_issuable_raw,
};
pub use routing_build::{
    AvailabilityReason, BuildBudget, BuildMode, ExecutionEquivalence, LOWERING_POLICY_VERSION,
    MappingResolution, NotLoweredReason, PlaybackContentRef, PlaybackOverlapSide,
    RoutingActivationReceipt, RoutingActivationState, RoutingBuildKey, RoutingBuildRequestId,
    RoutingCompileRequest, RoutingFault, RoutingFingerprint, RoutingPlaybackRef,
    RoutingReconcileMismatch, RoutingReconcileReport, RoutingSourceMap, RuntimeClockEpochRef,
    RuntimeInstanceId, SemanticNodeRef, UnsupportedReason, routing_fingerprint,
};
pub use routing_tx::{
    BusRef, ConnectionRef, ExplicitConversionPolicy, PortRef, RoutingEdit, RoutingEditError,
    RoutingEditIntent, RoutingPlan, RoutingReceipt, SendGainSpec, SendRef, StripRef, TempBusId,
    TempConnectionId, TempSendId, port_capability_generation, preflight as routing_preflight,
};
pub use smf::{
    SmfConversionReport, SmfDocument, SmfError, SmfEvent, SmfEventKind, SmfExportOptions, SmfFile,
    SmfFormat, SmfNoteSpec, SmfRational, SmfTrack, SmfUnsupportedEvent, export_smf, parse_smf,
};
pub use snapshot::{
    AssetAvailabilityManifest, AssetResolution, BlobProbe, BlobRead, ContentDigest, ContentStatus,
    FixedSnapshotLocator, ImmutableSnapshot, IndexGeneration, OpaqueStateRef, OpaqueStateStatus,
    ReadOnlyProjectProjection, SnapshotCancel, SnapshotCompleteness, SnapshotError,
    SnapshotIdentity, SnapshotLimits, SnapshotLocator, SnapshotOrigin, SnapshotResolver,
    SnapshotValidation, VcsBytesPort, VcsFileRead, VcsLocator, VcsPortError,
};
pub use store::{
    DirectoryBundleStore, InMemoryStore, ProjectStore, SCHEMA_VERSION, SnapshotEnvelope,
    snapshot_name,
};
pub use tempo::{
    ProjectPpqn, SignatureDto, TempoEventDto, TempoMapData, TempoMapRef, TickTimeMapping,
};
pub use time::{
    MediaDuration, MediaTimeValue, MidiTick, MusicalDuration, MusicalTick, ProjectTime,
    SessionFrame, SourceFrame,
};
pub use trace::{
    ActivePlan, AppliedStageReceipt, ConfiguredEvidence, ControlTrace, ControlTraceKey,
    DispatchDropReason, DispatchEvidence, FailedPlanAttempt, FreshnessContext, MeasuredValue,
    ObservationFreshness, PlanActivationState, PluginReportedEvidence, PreparedEvidence,
    ProbeObservation, ReceiptEvidence, ScheduledReceipt, StaleReason, TraceSlot, UnavailableReason,
};
