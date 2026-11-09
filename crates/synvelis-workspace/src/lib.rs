//! `synvelis-workspace` — headless Workspace foundation (Synvelis WS-A,
//! issue #36) plus the agreed editing model (ED-A, issues #11/#38).
//!
//! Owns the multi-window/multi-view workspace model on top of
//! `ruviform-ui`'s dock/window contracts and delegates all project
//! mutations to the single `synvelis-project` `ProjectController`.
//! The editing model — per-domain object selection, time selection,
//! lane scope, tool resolution, gesture fixation/preview, snap and
//! linked-context separation — lives in the `selection`, `tool`,
//! `snap`, `gesture` and `edit` modules.
//!
//! Everything here is non-RT and GUI-agnostic: windows are driven
//! through the [`ruviform_ui::window::WindowHandle`] port (a fake host
//! is provided for tests), so no OS window, webview, GPU or IME is
//! touched.

pub mod arrangement_ui;
pub mod command;
pub mod control;
pub mod controls_ui;
pub mod edit;
pub mod error;
pub mod focus;
pub mod gateway_ui;
pub mod gesture;
pub mod host;
pub mod link;
pub mod persist;
pub mod preset;
pub mod probe_trace;
pub mod relation_ui;
pub mod selection;
pub mod session;
pub mod snap;
pub mod state;
pub mod tool;
pub mod view;
pub mod viz_ui;
pub mod workspace;

pub use arrangement_ui::{ArrangementRow, arrangement_rows};
pub use command::{CommandContext, CommandKind, CommandOutcome};
pub use control::{
    APPLIED_REGISTRY_CAP, AppliedRecord, AssignmentView, AutomationLaneView, AutomationMode,
    AutomationRecordEvent, ControlGesture, ControlGestureFixation, ControlObservation,
    ControlSession, DEFAULT_TOUCH_RETURN_MS, GestureEnd, InspectorBreakdown, LoopPassId, Lowered,
    ObservationData, ObservationState, PlaybackState, PlaybackStatus, PreparedRecordingCommit,
    RECORD_DELIVERY_CAP, RECORD_DRAIN_BUDGET, RECORD_EVENT_QUEUE_CAP, RECORD_GAP_CAP,
    RECORD_HEALTH_CAP, RECORD_NOTIFY_CAP, RECORD_POINT_BUDGET, RECORD_REJECT_CAP, RebaseLaneReport,
    RebasePreview, RecordBudgets, RecordCommitError, RecordDisposition, RecordDrainReport,
    RecordGap, RecordHealthSummary, RecordNotification, RecordPushError, RecordRejectKind,
    RecordRejection, RecordTargetHealth, RecordingOutcome, RecordingTransaction,
    RecordingTransactionId, RecordingTxnState, ResolvedRecordGap, SealReason, TouchReturn,
    TransportEffect, WRITER_CLOSE_CAP, WriteArm, WriterCloseReason, WriterClosure, WriterState,
};
pub use controls_ui::{
    BindingSnapshot, CommandIntent, CommandPhase, CommandReceipt, ControlValue, ControlsUi,
    DispatchContext, GestureCancelReason, IntentKind, ParamProjection, ProjectionValue,
    ReceiptStatus, StateProjection, StripRow, TRANSPORT_POSITION_TARGET, TRANSPORT_TARGET,
    TransportProjection, TransportRow, UiDiagnostics, WireReceipt,
};
pub use edit::CommitEffect;
pub use error::WorkspaceError;
pub use focus::{FocusState, InputContext};
pub use gateway_ui::{
    AiFeature, AiPanelView, AiSwitches, ApplyMode, ApplyReport, ApprovalSurface, BudgetExplanation,
    CommandRow, CommitVerdict, ConnectionRow, EffectRow, EngineApplyState, ExpiryExplanation,
    GatewayUi, GatewayUiDeps, GatewayUiError, GrantExplanation, GrantRow, JobRow, LinkedEffect,
    MidiNoteRow, OperationRow, OwnerExplanation, Revocation, RevocationReport, ScopeExplanation,
    SurfaceDecision, SwitchRow, TaskRow, capture_scope, explain_grant,
};
pub use gesture::{CancelReason, Gesture, GestureFixation, GestureOutcome, GesturePreview};
pub use host::FakeWindowHost;
pub use link::{LinkEvent, LinkGroupId, LinkGroupManager};
pub use persist::{WORKSPACE_SCHEMA, WorkspaceSnapshot};
pub use preset::{
    PRESET_SCHEMA, PresetDock, PresetFocus, PresetGeometry, PresetLink, PresetReport, PresetView,
    PresetViewState, PresetWindow, WorkspacePreset,
};
pub use probe_trace::{ProbeDrainReport, ProbeTraceError};
pub use relation_ui::{
    RELATION_VIEW_INCLUDE_DISABLED, RELATION_VIEW_MAX_DEPTH, RELATION_VIEW_MAX_EDGES, RelationNav,
    RelationRow, RelationView, RelationViewOptions, relation_rows, relation_rows_with,
};
pub use selection::{
    EditBinding, EditTimeDomain, EditTimePoint, LaneScope, ObjectSelections, SelectionDomain,
    TimeSelection,
};
pub use session::{EnginePort, EngineStub, ProjectSession, TransportState};
pub use snap::{
    GridConfig, SnapCandidate, SnapConfig, SnapEnvironment, SnapSource, SnapUnit, SnapWinner,
    resolve_snap,
};
pub use state::{
    EditingSelection, InspectionFocus, InspectionMode, LinkedContext, SemanticTarget, ViewState,
    ViewStateRegistry, Viewport,
};
pub use tool::{
    EditOperation, HandleKind, HitCandidate, HitProbe, HitZone, Tool, ToolResolution, resolve_tool,
};
pub use view::{HeadlessView, ViewDescriptor, ViewFactory, ViewRegistry, ViewType};
pub use viz_ui::{
    DEFAULT_CACHE_BUDGET, DEFAULT_MAX_QUERY_BYTES, DEFAULT_STALE_AFTER_MS, FreqScale, MAX_BUCKETS,
    MAX_EVENTS, ManualMeter, ManualSpectrum, ManualWaveform, MemoryWaveform, MeterChannel,
    MeterFrame, MeterMeta, MeterPort, MeterRow, MeterScale, OverlayMarker, OverlayState,
    QueryErrorKind, QueryId, QueryOutcome, QueryPayload, QueryReply, SourceRevision,
    SpectrumBinsAxis, SpectrumFrame, SpectrumMeta, SpectrumPort, SpectrumRow, SpectrumScale,
    SubrangeWaveform, SurfaceId, TimeRange, TimelineAxis, TimelineItem, TimelinePayload,
    TimelineRow, ViewportQuery, ViewportTrigger, VizDiagnostics, VizError, VizQueryError,
    VizSourceId, VizStatus, VizStyleInput, VizUi, VizViewEvent, WaveformAxis, WaveformBucket,
    WaveformMeta, WaveformPort, WaveformQuery, WaveformRow, WaveformView, meter_master_target,
    meter_track_target, spectrum_probe_target, waveform_clip_target, waveform_source_target,
};
pub use workspace::{PointerOutcome, WindowRecord, WorkspaceManager};
