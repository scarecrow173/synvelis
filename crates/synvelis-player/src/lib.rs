//! `synvelis-player` — the L1 player slice: safe local WAV import,
//! verified media resolution, timeline lowering onto the generic
//! Ruviform graph, and a deterministic headless playback driver.
//!
//! Layering: this crate owns the player-domain pipeline only. Project
//! mutations go through [`synvelis_project::ProjectController`] as
//! validated [`synvelis_project::Operation`]s; media verification,
//! decoding, prefetching and the audio runtime are Ruviform services.
//! No Project/Track/Clip type crosses the graph boundary — lowering
//! emits only stable node/port IDs plus plain audio formats.
//!
//! Pipeline (Issue #24 contract):
//!
//! ```text
//! local path → non-RT staging → size/hash/container/codec/frame/channel
//!   verify → measured OS durability → verified blob materialization →
//!   expected-revision project mutation → save/reload
//! ```

pub mod binding;
pub mod blob;
pub mod dispatch;
pub mod driver;
pub mod error;
pub mod fixture;
pub mod host;
pub mod host_param;
pub mod import;
pub mod lowering;
pub mod midi;
pub mod monitor;
pub mod monitor_engine;
pub mod plugin_state;
pub mod probe;
pub mod probe_compare;
pub mod probe_trace;
pub mod process_boundary;
pub mod processor;
pub mod publish;
pub mod rcp;
pub mod resolver;
pub mod routing_control;
pub mod routing_engine;
pub mod routing_lower;
pub mod trace;
pub mod verified_import;

pub use binding::{
    AttachClockContext, BindingError, PluginBindingSession, PluginParameterBinding,
    StablePluginInstanceId,
};
pub use blob::{VerifiedBlob, open_blob_verified};
pub use dispatch::{DispatchCycle, DispatchError, ParamDispatch};
pub use driver::{AttachedNode, DriverState, HeadlessDriver, PlayReport, PumpPolicy};
pub use error::{ImportError, ResolveError, VerifiedImportError};
pub use host::{
    BoundaryDeclaration, PluginHostProcessors, PluginReportDrain, ProcessorConnectError,
    ProcessorConnectRequest, ProcessorConnection, ProcessorControl, ProcessorHost, ProcessorLease,
    StaticProcessorHost,
};
pub use host_param::{HostParamDelivery, HostParamEvidence, binding_evidence};
pub use import::{BlobReceipt, ImportOutcome, ImportRequest, ProjectReceipt, VerifiedMedia};
pub use lowering::{
    ClipWindow, CompSkipReason, EngineConfig, FadeDirection, FadeWindow, Lowered, NodeContent,
    OverlapSide, PlaybackBinding, SkippedPlayback,
};
pub use midi::{MidiCompileError, compile_midi_events};
pub use monitor::{
    LatencyBasis, LatencyComponent, LatencyQuality, LatencyReport, LatencyResolution,
    LatencyResolutionBasis, LowLatencyMonitor, LowLatencyReadiness, LowLatencyReason,
    LowLatencyState, MonitorAdapter, MonitorBlock, MonitorOutput, MonitorOutputNotices,
    MonitorRoute, prepare_low_latency,
};
pub use monitor_engine::{
    EngineBlocks, MonitorEngine, MonitorEngineError, MonitorRunReport, MonitorSegmentRecord,
    MonitorSession,
};
pub use plugin_state::{PluginStateEndpoint, PluginStateHost};
pub use probe::{ProbeRequestError, ProbeSession};
pub use probe_compare::{PROBE_COVERAGE_MAX_EVENTS, ProbeCoverage, probe_compare};
pub use probe_trace::ProbeTraceBridge;
pub use process_boundary::{
    AttachDelivery, BoundaryDegradation, BoundaryKind, BoundaryPoint, BoundarySessionError,
    DegradedConsent, ExecutionMode, PrecisionProvenance, ProcessBoundaryError, ProcessBoundaryPlan,
    ProcessBoundaryRequest, ProcessBoundarySession, ProcessCapability, SegmentPlan,
    TimingPrecision,
};
pub use processor::{ClipPlayer, PlaybackStats};
pub use publish::{
    PublishError, PublishReport, PublishedStream, SkippedStream, publish_recorded,
    publish_recorded_with,
};
pub use resolver::PreparedClip;
pub use routing_control::{
    ControlDispatchError, FastControlBlock, FastControlBlockKind, FastControlOutcome,
    FastControlWrite, ParamCatalog, ReportDrainReport, RoutingControlDispatch, SurfaceReport,
    SyncOutcome, TickReport,
};
pub use trace::{
    DispatchTraceAdapter, TraceAdapterConfig, TraceContext, TraceEvent, TraceEventKind,
    TraceMetrics,
};
pub use verified_import::{
    VERIFIED_IMPORT_PROVIDER, VerifiedImportOutcome, VerifiedImportRequest, import_verified_local,
};
