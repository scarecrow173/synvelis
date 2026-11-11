//! SF-03 slice 6 — SF-ACT-06 headless consumer: generation-scoped
//! automation→`TimedParamEvent` wiring plus the fast-control
//! active-generation gate with a bounded re-validating queue.
//!
//! [`crate::PluginBindingSession`] + [`crate::ParamDispatch`] already
//! own per-instance binding pins and plan staging, and
//! [`crate::routing_engine::RoutingEngine::take_processor_control`]
//! vends each published generation's non-RT control halves exactly
//! once. This module is the missing orchestration seam between them
//! and the semantic [`Project`]: it attaches one control surface per
//! routing plan generation, lowers evaluated automation into
//! generation-pinned `TimedParamEvent` batches, and gates point-in-time
//! fast-control writes on the **RT-observed** active generation —
//! never on the merely-published one.
//!
//! Contract highlights:
//!
//! - **Active-generation gate.** Automation ticks and fast-control
//!   deliveries run only while `active_generation == surface.generation`
//!   (the value [`RoutingEngine::rt_status`] reports after
//!   `RtRuntime::process` observes the swap). A published-but-not-yet-
//!   observed generation dispatches nothing; automation is
//!   position-driven so the next tick self-corrects, while fast
//!   controls wait in a bounded FIFO.
//! - **Bounded queue, re-validated replay.** Queued writes are never
//!   forwarded by stale `(node, param)` pairs: on drain each write is
//!   re-resolved through the *current* surface's verified binding
//!   record (`synvelis target → plugin ParamId`), so a generation swap
//!   delivers the same semantic intent to the new generation's plugin
//!   instance — or drops it typed when the target no longer binds.
//!   Overflow rejects the new write (`QueueFull`); nothing is silently
//!   evicted.
//! - **One plan per node per tick.** [`ParamDispatch::dispatch`]
//!   replaces the node's staged plan, so automation events and drained
//!   fast controls for the same node are merged into a single atomic
//!   [`PluginBindingSession::dispatch`] batch — a later batch can
//!   never silently drop an earlier write's pending events *inside one
//!   service call*. Across calls the upstream plan-replace semantics
//!   stand: a superseded plan's undelivered events are recorded
//!   `PlanReplaced` in the dispatch ledger (observable via
//!   [`ParamDispatch::observe`]), so consumers should stamp
//!   ≈current positions and service controls at a cadence where
//!   stamped events deliver before the next dispatch — automation
//!   self-corrects every tick, fast writes are point-in-time intents.
//! - **Generation-scoped evidence.** Every [`PluginParameterBinding`]
//!   pins `graph_generation` to the routing plan generation it was
//!   minted under; [`RoutingControlDispatch::sync`] mints a fresh
//!   surface (new sessions, new instance ids) per generation and the
//!   queue survives the swap for re-validation.
//! - **Reported-observation consumption.** [`Self::drain_output_reports`]
//!   is the single consumer of the plugin→host report channel
//!   (`PluginHost::drain_output_reports`): drained `ParamValue` events
//!   resolve `ParamId → target` through the same verified session
//!   bindings deliveries use, `Unavailable` answers and dead handles
//!   terminate pending ledger entries honestly, and non-terminal host
//!   errors stay typed evidence. [`Self::observe_cycle`] is the
//!   generation-scoped per-node projection half — the two never share
//!   a drain.
//!
//! Non-RT only: attach/sync/drain run on the orchestration thread.
//! The plugin parameter catalogs are caller-vended evidence — the same
//! `&[ParamInfo]` snapshots [`PluginBindingSession::bind`] validates
//! declared capabilities against; live re-enumeration is the caller's
//! [`RoutingControlDispatch::refresh_catalogs`] responsibility.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;

use ruviform_media_core::{EngineFrame, MediaTime, TimeBase, TransportCommand, TransportSpan};
use ruviform_plugin_core::{Event, EventKind, ParamInfo, ParamWriteMeans, TimedParamEvent};
use ruviform_plugin_host::{HostError, PluginHandle, ReportDrain};
use synvelis_project::{
    BeatPoint, ControlGraph, ControlOwner, EvalPlan, EvalScratch, ParameterTargetId, ProcessorId,
    Project, ProjectRevision, lower_event, port_capability_generation,
};

use crate::binding::{
    BindingError, PluginBindingSession, PluginParameterBinding, StablePluginInstanceId,
};
use crate::dispatch::{DispatchCycle, DispatchError, ParamDispatch};
use crate::plugin_state::PluginStateEndpoint;
use crate::process_boundary::{
    BoundaryPoint, BoundarySessionError, PrecisionProvenance, ProcessBoundaryPlan,
    ProcessBoundarySession,
};
use crate::routing_engine::RoutingEngine;

/// Caller-vended plugin parameter enumeration per semantic processor —
/// the evidence [`PluginBindingSession::bind`] checks declared target
/// capabilities against. Missing entries behave as an empty
/// enumeration (every capability bind fails `ParamRemoved`).
pub type ParamCatalog = BTreeMap<ProcessorId, Vec<ParamInfo>>;

/// A point-in-time fast-control write in the normalized host domain —
/// the same value domain [`PluginBindingSession::eval_event`] emits.
///
/// `position`/`means` are the delivery stamp; `project_revision` is
/// correlation evidence for trace keys — replay identity is decided
/// by binding re-resolution, never by the revision stamp.
#[derive(Clone, Debug)]
pub struct FastControlWrite {
    /// Semantic target the write addresses.
    pub target: ParameterTargetId,
    /// Normalized host value (`eval_rt`'s `host_normalized` domain).
    pub value: f64,
    /// Delivery precision the target's capability must accept.
    pub means: ParamWriteMeans,
    /// Absolute media position the event is stamped at.
    pub position: MediaTime,
    /// Project revision the intent was authored against (evidence).
    pub project_revision: ProjectRevision,
}

/// Why a fast-control write was refused instead of delivered or
/// queued. Every variant is typed — a rejection is never a silent
/// success or a reroute to another parameter.
#[derive(Debug)]
pub enum FastControlBlock {
    /// No generation surface is attached yet.
    NoSurface,
    /// The attached surface's capability/graph pins no longer match
    /// the project — `sync` to a fresh generation is required.
    StaleSurface,
    /// The write's queue is full; the new write was rejected (older
    /// queued writes are never evicted).
    QueueFull,
    /// The target has no verified binding in the active surface.
    UnboundTarget,
    /// [`synvelis_project::lower_event`]'s capability/write gate
    /// rejected the write (missing capability, unsupported means,
    /// non-finite value).
    Lowering(synvelis_project::Error),
    /// The pinned-binding batch dispatch rejected the write
    /// ([`BindingError`] — defunct/foreign pins, node `QueueFull`,
    /// unrepresentable position).
    Dispatch(BindingError),
}

impl fmt::Display for FastControlBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSurface => write!(f, "no control surface attached"),
            Self::StaleSurface => write!(f, "control surface pins are stale"),
            Self::QueueFull => write!(f, "fast-control queue is full"),
            Self::UnboundTarget => write!(f, "target has no verified binding"),
            Self::Lowering(e) => write!(f, "lowering rejected: {e}"),
            Self::Dispatch(e) => write!(f, "dispatch rejected: {e}"),
        }
    }
}

impl std::error::Error for FastControlBlock {}

/// `Copy` projection of [`FastControlBlock`] for outcome reporting —
/// the full typed reason is available through
/// [`RoutingControlDispatch::last_block`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FastControlBlockKind {
    /// See [`FastControlBlock::NoSurface`].
    NoSurface,
    /// See [`FastControlBlock::StaleSurface`].
    StaleSurface,
    /// See [`FastControlBlock::QueueFull`].
    QueueFull,
    /// See [`FastControlBlock::UnboundTarget`].
    UnboundTarget,
    /// See [`FastControlBlock::Lowering`].
    Lowering,
    /// See [`FastControlBlock::Dispatch`].
    Dispatch,
}

impl FastControlBlock {
    /// The `Copy` kind of this block.
    #[must_use]
    pub const fn kind(&self) -> FastControlBlockKind {
        match self {
            Self::NoSurface => FastControlBlockKind::NoSurface,
            Self::StaleSurface => FastControlBlockKind::StaleSurface,
            Self::QueueFull => FastControlBlockKind::QueueFull,
            Self::UnboundTarget => FastControlBlockKind::UnboundTarget,
            Self::Lowering(_) => FastControlBlockKind::Lowering,
            Self::Dispatch(_) => FastControlBlockKind::Dispatch,
        }
    }
}

/// Why a [`RoutingControlDispatch::boundary_plan_window`] /
/// [`RoutingControlDispatch::boundary_stage_window`] call refused —
/// typed, never a silent skip. The session's own
/// [`BoundarySessionError`] is preserved verbatim under
/// [`Self::Session`].
#[derive(Debug)]
pub enum BoundaryStageError {
    /// No generation surface is attached yet.
    NoSurface,
    /// The attached surface's capability/graph pins no longer match
    /// the project — `sync` to a fresh generation is required.
    StaleSurface,
    /// The RT-observed `active_generation` does not match the
    /// surface's generation — staging is gated on activation
    /// evidence, never on the merely-published generation.
    InactiveGeneration {
        /// The generation the attached surface serves.
        surface: u64,
        /// The caller's RT-observed activation (`None` = not active).
        active: Option<u64>,
    },
    /// The processor has no boundary session on this surface —
    /// undeclared attach, no vended control, or a boundary-open
    /// refusal already recorded in `SurfaceReport::without_control`.
    NoSession(ProcessorId),
    /// The [`ProcessBoundarySession`] refused — stale provenance,
    /// oracle refusal, or node-side `QueueFull`; the typed
    /// [`BoundarySessionError`] is preserved verbatim.
    Session(BoundarySessionError),
}

impl fmt::Display for BoundaryStageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSurface => write!(f, "no control surface attached"),
            Self::StaleSurface => write!(f, "control surface pins are stale"),
            Self::InactiveGeneration { surface, active } => {
                write!(
                    f,
                    "generation {surface} is not RT-active (observed {active:?})"
                )
            }
            Self::NoSession(processor) => {
                write!(f, "processor {processor} has no boundary session")
            }
            Self::Session(e) => write!(f, "boundary session refused: {e}"),
        }
    }
}

impl std::error::Error for BoundaryStageError {}

/// Result of [`RoutingControlDispatch::fast_control`].
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FastControlOutcome {
    /// Staged on the RT-active generation's plugin node.
    Delivered,
    /// Held in the bounded queue awaiting activation or re-validation.
    Queued {
        /// Queue depth after this write was accepted.
        depth: usize,
    },
    /// Refused — see [`FastControlBlock`].
    Blocked(FastControlBlockKind),
}

/// Per-surface attach/sync evidence: which processors obtained
/// plugin control halves, which targets bound, which failed typed.
#[derive(Clone, Debug, Default)]
pub struct SurfaceReport {
    /// Routing plan generation this surface serves.
    pub generation: u64,
    /// Plugin-backed chain processors with a live control surface.
    pub controlled_processors: Vec<ProcessorId>,
    /// Semantic processors that yielded no control half
    /// ([`crate::StaticProcessorHost`] connections, already-vended
    /// handles, or processors absent from the published generation).
    pub without_control: Vec<ProcessorId>,
    /// Targets with a verified [`PluginParameterBinding`].
    pub bound_targets: usize,
    /// Targets whose bind failed — `target → rejection` evidence.
    pub unbound: BTreeMap<ParameterTargetId, String>,
}

/// Counters from one [`RoutingControlDispatch::tick`] call.
#[derive(Clone, Debug, Default)]
pub struct TickReport {
    /// `false` when no surface is attached — nothing ran.
    pub attached: bool,
    /// The surface's routing plan generation.
    pub generation: u64,
    /// `true` when `active_generation` did not match the surface —
    /// the tick dispatched nothing (automation self-corrects next
    /// tick; queued fast controls stay queued).
    pub gated: bool,
    /// `true` when capability/graph pins drifted — the surface is
    /// stale and requires [`RoutingControlDispatch::sync`].
    pub stale: bool,
    /// Queue writes accepted into this tick's dispatch batches.
    pub queue_delivered: usize,
    /// Queue writes dropped during drain (defunct binding, lowering
    /// rejection, owner/capability drift).
    pub queue_dropped: usize,
    /// Queue writes retained because the node's bounded channel was
    /// full — retried on the next active tick.
    pub queue_retained: usize,
    /// Automation evaluations attempted (bound, project-owned targets).
    pub evaluated: usize,
    /// Bound targets skipped because evaluation or lowering failed.
    pub eval_skipped: usize,
    /// `TimedParamEvent`s staged across all nodes.
    pub events: usize,
    /// Nodes that accepted a dispatch batch.
    pub nodes_dispatched: usize,
    /// Nodes whose batch dispatch failed.
    pub dispatch_errors: usize,
}

/// Counters from one [`RoutingControlDispatch::drain_output_reports`]
/// call — the generation-scoped plugin→host report consumption.
#[derive(Clone, Debug, Default)]
pub struct ReportDrainReport {
    /// `false` when no surface is attached — nothing drained.
    pub attached: bool,
    /// The surface's routing plan generation.
    pub generation: u64,
    /// `true` when `generation` did not name the surface — every
    /// endpoint was refused before any drain ran, so a report vended
    /// for another generation can never be attributed here.
    pub generation_gated: bool,
    /// `true` when `active_generation` did not match the surface —
    /// reports stay queued plugin-side under the same gate deliveries
    /// use; the next active drain still sees them.
    pub inactive_gated: bool,
    /// Endpoints answered [`ReportDrain::Drained`].
    pub drained: usize,
    /// Endpoints with no usable report channel — `Unavailable`, a
    /// stale handle or a converged-failed instance — whose pending
    /// ledger entries moved to `Unavailable`.
    pub unavailable: usize,
    /// Endpoints whose drain failed non-terminally or answered an
    /// unknown [`ReportDrain`] variant — the ledger stays untouched.
    pub failed: usize,
    /// `Event`s appended across all drained endpoints (every kind).
    pub events: usize,
    /// `EventKind::ParamValue` reports consumed.
    pub param_reports: usize,
    /// Reports that advanced a ledger entry to `Reported`.
    pub reported: usize,
    /// Param reports naming a `ParamId` no session binding covers —
    /// counted, never attributed to a target.
    pub unbound_param: usize,
    /// Bound-param reports [`ParamDispatch::record_reported`]
    /// declined — no eligible `Scheduled`/`Dispatched` entry exists
    /// (plugin-initiated reports or already-terminal writes).
    pub no_pending: usize,
    /// Endpoints skipped: the processor has no node surface on this
    /// generation (static host connection or unvend control).
    pub unrouted: usize,
    /// Every `Err(HostError)` drain answer as `(processor, error)`;
    /// `unavailable`/`failed` record how each was classified.
    pub errors: Vec<(ProcessorId, HostError)>,
}

/// Result of [`RoutingControlDispatch::sync`].
#[derive(Clone, Debug)]
pub enum SyncOutcome {
    /// Same generation — surface refreshed in place (plan recompile /
    /// rebind as needed); no controls were re-vended.
    Current(SurfaceReport),
    /// A different generation was attached — fresh sessions, fresh
    /// instance ids; the fast-control queue was retained for
    /// re-validated replay.
    Swapped(SurfaceReport),
}

/// Failure to attach or sync a control surface.
#[derive(Debug)]
pub enum ControlDispatchError {
    /// The control model does not compile against `project`
    /// ([`ControlGraph::compile`]).
    Project(synvelis_project::Error),
    /// [`ParamDispatch::new`] rejected the vended control's frame
    /// base — unreachable through [`RoutingEngine`] (the engine mints
    /// `frame_base` from its own sample rate) but typed anyway.
    Dispatch(DispatchError),
}

impl fmt::Display for ControlDispatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Project(e) => write!(f, "control plan compile failed: {e}"),
            Self::Dispatch(e) => write!(f, "param dispatch attach failed: {e}"),
        }
    }
}

impl std::error::Error for ControlDispatchError {}

/// One published generation's control surface: per-processor
/// [`PluginBindingSession`] + [`ParamDispatch`] pairs, the compiled
/// [`EvalPlan`], the caller-vended param catalogs and the last
/// attach/sync [`SurfaceReport`].
struct GenerationSurface {
    generation: u64,
    /// Capability generation pinned at attach — sessions carry the
    /// same pin; freshness is always checked against the *live*
    /// project value, never this snapshot.
    capability_generation: u64,
    nodes: BTreeMap<ProcessorId, NodeSurface>,
    catalogs: ParamCatalog,
    plan: EvalPlan,
    scratch: EvalScratch,
    report: SurfaceReport,
}

/// One chain processor's verified control pair.
struct NodeSurface {
    session: PluginBindingSession,
    dispatch: ParamDispatch,
    /// TIM-03 per-generation [`ProcessBoundarySession`] — `Some` only
    /// when the vended [`crate::ProcessorControl`] carried a
    /// [`crate::BoundarySessionSpec`] (the processor was attached
    /// under a boundary declaration). Pinned to this surface's
    /// instance/node/capability/graph generations; a surface swap
    /// mints a fresh session with fresh pins.
    boundary: Option<ProcessBoundarySession>,
}

/// A batch entry staged for one node — automation evaluations and
/// drained fast controls merged atomically; `requeue` carries the
/// original write so a node-side `QueueFull` can retain it instead of
/// losing the user's intent.
struct BatchEntry {
    binding: PluginParameterBinding,
    event: TimedParamEvent,
    requeue: Option<FastControlWrite>,
}

/// Why [`RoutingControlDispatch::lower_for_node`] refused a write —
/// permanent failures only (node `QueueFull` surfaces later at
/// dispatch time, never here).
enum LowerFail {
    /// No project target / processor owner / bound session record.
    Unbound,
    /// [`lower_event`]'s capability/write gate rejected the write.
    Lowering(synvelis_project::Error),
}

/// SF-ACT-06 headless control dispatcher: attaches one verified
/// control surface per routing plan generation and gates every
/// delivery on the RT-observed active generation.
///
/// Construct with [`Self::attach`] for the first generation and
/// [`Self::sync`] whenever the published generation changes; drive
/// [`Self::tick`] at the consumer's control cadence and
/// [`Self::fast_control`] for point-in-time writes.
pub struct RoutingControlDispatch {
    queue_capacity: usize,
    next_instance: u64,
    surface: Option<GenerationSurface>,
    queue: VecDeque<FastControlWrite>,
    last_block: Option<FastControlBlock>,
}

impl RoutingControlDispatch {
    /// Attaches the control surface for `generation` of `engine`:
    /// vends each semantic processor's one-shot
    /// [`ProcessorControl`](crate::ProcessorControl), opens a
    /// [`PluginBindingSession`] + [`ParamDispatch`] pair pinned to
    /// `generation`/`port_capability_generation(project)`, compiles
    /// the [`EvalPlan`], and binds every `ControlOwner::Processor`
    /// target that declares a plugin capability.
    ///
    /// `catalogs` is the caller's current plugin parameter evidence —
    /// stored for later revalidation/rebind; refresh it through
    /// [`Self::refresh_catalogs`] when a plugin re-enumerates.
    ///
    /// # Errors
    /// [`ControlDispatchError::Project`] when the control model does
    /// not compile; [`ControlDispatchError::Dispatch`] on a frame-base
    /// mismatch (unreachable via [`RoutingEngine`]).
    pub fn attach(
        engine: &mut RoutingEngine,
        generation: u64,
        project: &Project,
        catalogs: &ParamCatalog,
    ) -> Result<Self, ControlDispatchError> {
        let mut dispatcher = Self {
            queue_capacity: 64,
            next_instance: 1,
            surface: None,
            queue: VecDeque::new(),
            last_block: None,
        };
        dispatcher.sync(engine, generation, project, catalogs)?;
        Ok(dispatcher)
    }

    /// Overrides the fast-control queue capacity (default 64).
    /// Queued writes beyond capacity are rejected `QueueFull` — older
    /// entries are never evicted.
    #[must_use]
    pub fn with_queue_capacity(mut self, capacity: usize) -> Self {
        self.queue_capacity = capacity;
        self
    }

    /// The attached surface's routing plan generation, if any.
    #[must_use]
    pub fn generation(&self) -> Option<u64> {
        self.surface.as_ref().map(|s| s.generation)
    }

    /// Current fast-control queue depth.
    #[must_use]
    pub fn queue_depth(&self) -> usize {
        self.queue.len()
    }

    /// The most recent [`FastControlBlock`] — typed rejection evidence
    /// for [`FastControlOutcome::Blocked`].
    #[must_use]
    pub fn last_block(&self) -> Option<&FastControlBlock> {
        self.last_block.as_ref()
    }

    /// The attached surface's evidence report.
    #[must_use]
    pub fn report(&self) -> Option<&SurfaceReport> {
        self.surface.as_ref().map(|s| &s.report)
    }

    /// The verified attach-time frame base shared by the surface's
    /// nodes (`None` before attach or when no processor is
    /// plugin-backed). Positions for [`Self::fast_control`] and
    /// automation ticks are stamped in this base.
    #[must_use]
    pub fn frame_base(&self) -> Option<TimeBase> {
        self.surface
            .as_ref()
            .and_then(|s| s.nodes.values().next().map(|n| n.dispatch.frame_base()))
    }

    /// The verified [`PluginParameterBinding`] for `target` on the
    /// attached surface — generation/capability/instance pins included.
    /// `None` before attach, for unbound targets, or after the binding
    /// went defunct. This is the cross-consumer evidence seam: the
    /// record's `graph_generation` must equal the routing plan
    /// generation the engine's receipt/source map reports.
    #[must_use]
    pub fn binding(&self, target: ParameterTargetId) -> Option<PluginParameterBinding> {
        let surface = self.surface.as_ref()?;
        surface
            .nodes
            .values()
            .find_map(|node| node.session.binding(target).cloned())
    }

    /// Swaps the control surface to `generation`: same generation
    /// refreshes bindings in place (one-shot controls are *not*
    /// re-vended — sessions and dispatches persist); a different
    /// generation vends fresh controls, mints fresh sessions and
    /// retains the fast-control queue for re-validated replay.
    ///
    /// # Errors
    /// Same as [`Self::attach`].
    pub fn sync(
        &mut self,
        engine: &mut RoutingEngine,
        generation: u64,
        project: &Project,
        catalogs: &ParamCatalog,
    ) -> Result<SyncOutcome, ControlDispatchError> {
        if let Some(surface) = self.surface.as_mut()
            && surface.generation == generation
        {
            Self::refresh_surface(surface, project, catalogs);
            return Ok(SyncOutcome::Current(surface.report.clone()));
        }
        let plan = ControlGraph::compile(project).map_err(ControlDispatchError::Project)?;
        let mut surface = GenerationSurface {
            generation,
            capability_generation: port_capability_generation(project),
            nodes: BTreeMap::new(),
            catalogs: catalogs.clone(),
            scratch: plan.scratch(),
            plan,
            report: SurfaceReport {
                generation,
                ..SurfaceReport::default()
            },
        };
        self.vend_nodes(engine, &mut surface, project);
        let report = surface.report.clone();
        self.surface = Some(surface);
        Ok(SyncOutcome::Swapped(report))
    }

    /// Replaces the stored parameter catalogs (plugin re-enumeration
    /// evidence) and re-runs every bind/revalidate against them on the
    /// current surface. Targets whose live enumeration drifted flip
    /// defunct; new or changed capabilities bind fresh.
    pub fn refresh_catalogs(&mut self, project: &Project, catalogs: &ParamCatalog) {
        let Some(surface) = self.surface.as_mut() else {
            return;
        };
        Self::refresh_surface(surface, project, catalogs);
    }

    /// Evaluates every bound automation target at `pos` (beat domain)
    /// / `position` (media domain — the same instant) and stages one
    /// merged batch per node, but only while `active_generation`
    /// equals the surface's generation — the RT-observed activation
    /// evidence, not the published one.
    ///
    /// `pos` and `position` must denote the same transport instant in
    /// the beat and media domains. When the surface is gated or stale
    /// the tick dispatches nothing and reports why; queued fast
    /// controls drain only on an active, current surface.
    pub fn tick(
        &mut self,
        project: &Project,
        pos: BeatPoint,
        position: MediaTime,
        active_generation: Option<u64>,
    ) -> TickReport {
        let mut report = TickReport::default();
        let Some(mut surface) = self.surface.take() else {
            return report;
        };
        self.tick_inner(
            &mut surface,
            project,
            pos,
            position,
            active_generation,
            &mut report,
        );
        self.surface = Some(surface);
        report
    }

    fn tick_inner(
        &mut self,
        surface: &mut GenerationSurface,
        project: &Project,
        pos: BeatPoint,
        position: MediaTime,
        active_generation: Option<u64>,
        report: &mut TickReport,
    ) {
        report.attached = true;
        report.generation = surface.generation;
        if !Self::surface_is_current(surface, project) {
            report.stale = true;
            return;
        }
        if surface.plan.is_stale(project) {
            let catalogs = surface.catalogs.clone();
            Self::refresh_surface(surface, project, &catalogs);
        }
        if active_generation != Some(surface.generation) {
            report.gated = true;
            return;
        }
        let mut batches: BTreeMap<ProcessorId, Vec<BatchEntry>> = BTreeMap::new();
        self.drain_queue(surface, project, &mut batches, report);
        Self::collect_automation(surface, project, pos, position, &mut batches, report);
        self.flush_batches(surface, &mut batches, report);
    }

    /// Submits a point-in-time fast-control write.
    ///
    /// While the surface's generation is RT-active the write lowers
    /// through [`lower_event`] and dispatches immediately against the
    /// verified binding; otherwise it queues (bounded — overflow is
    /// `Blocked(QueueFull)`). Queue drain re-resolves every write
    /// through the *current* surface's bindings, so a generation swap
    /// forwards the same semantic intent to the new instance's
    /// parameter — never to a stale `(node, param)` pair.
    ///
    /// `active_generation` is the caller's RT-observed value
    /// ([`RoutingEngine::rt_status`]); `None` (stopped engine, no
    /// observed activation) always queues or blocks — never delivers.
    pub fn fast_control(
        &mut self,
        project: &Project,
        write: FastControlWrite,
        active_generation: Option<u64>,
    ) -> FastControlOutcome {
        self.last_block = None;
        let Some(mut surface) = self.surface.take() else {
            return self.block(FastControlBlock::NoSurface);
        };
        let outcome = self.fast_control_inner(&mut surface, project, write, active_generation);
        self.surface = Some(surface);
        outcome
    }

    fn fast_control_inner(
        &mut self,
        surface: &mut GenerationSurface,
        project: &Project,
        write: FastControlWrite,
        active_generation: Option<u64>,
    ) -> FastControlOutcome {
        if !Self::surface_is_current(surface, project) {
            return self.block(FastControlBlock::StaleSurface);
        }
        if surface.plan.is_stale(project) {
            let catalogs = surface.catalogs.clone();
            Self::refresh_surface(surface, project, &catalogs);
        }
        if active_generation != Some(surface.generation) {
            return self.enqueue(write);
        }
        let (processor, entry) = match Self::lower_for_node(surface, project, &write) {
            Ok(pair) => pair,
            Err(LowerFail::Unbound) => return self.block(FastControlBlock::UnboundTarget),
            Err(LowerFail::Lowering(e)) => {
                return self.block(FastControlBlock::Lowering(e));
            }
        };
        let Some(node) = surface.nodes.get_mut(&processor) else {
            return self.block(FastControlBlock::UnboundTarget);
        };
        let writes = [(entry.binding, entry.event)];
        match node.session.dispatch(&mut node.dispatch, &writes) {
            Ok(_) => FastControlOutcome::Delivered,
            Err(BindingError::Dispatch(DispatchError::QueueFull)) => {
                // Node channel full — retain the write for the next
                // active drain rather than dropping the intent.
                self.queue.push_front(write);
                FastControlOutcome::Queued {
                    depth: self.queue.len(),
                }
            }
            Err(e) => self.block(FastControlBlock::Dispatch(e)),
        }
    }

    /// Drains plugin→host output reports for `endpoints` — the
    /// `PluginHost::drain_output_reports` consumer half — into the
    /// attached surface's per-node dispatch ledgers.
    ///
    /// `endpoints` is the caller-vended `(processor, PluginHandle)`
    /// set for `generation` ([`RoutingEngine::plugin_state_handles`]).
    /// `drain` is the caller's host borrow — e.g.
    /// `|handle, out| host.borrow_mut().drain_output_reports(handle, out)`
    /// — injected as a seam so this surface stays host-type thin and
    /// the drain answer stays testable without an L1 worker.
    /// `generation` must name the attached surface and
    /// `active_generation` the RT-observed one ([`RoutingEngine::rt_status`]);
    /// either mismatch gates the whole call before any endpoint drains,
    /// so reports stay queued plugin-side and a foreign generation's
    /// reports can never be attributed to this surface's bindings.
    ///
    /// A `Drained` answer's events map through the node's verified
    /// session bindings — the same `ParamId → synvelis target` table
    /// deliveries resolve through, so a report can only land on a
    /// target this generation actually bound. `EventKind::ParamValue`
    /// advances the ledger via [`ParamDispatch::record_reported`];
    /// `Unavailable` and stale/failed handles terminate pending
    /// entries honestly via
    /// [`ParamDispatch::mark_reporting_unavailable`]; non-terminal
    /// [`HostError`]s are typed `errors` evidence with the ledger
    /// untouched. `EventKind::Midi` and future kinds count toward
    /// `events` but never reach the parameter ledger.
    ///
    /// Non-RT only. These are the plugin's *output* reports — the
    /// node scheduler report channel stays owned by
    /// [`Self::observe_cycle`]; the two never share a drain.
    pub fn drain_output_reports(
        &mut self,
        endpoints: &[PluginStateEndpoint],
        generation: u64,
        active_generation: Option<u64>,
        mut drain: impl FnMut(PluginHandle, &mut Vec<Event>) -> Result<ReportDrain, HostError>,
    ) -> ReportDrainReport {
        let mut report = ReportDrainReport::default();
        let Some(mut surface) = self.surface.take() else {
            return report;
        };
        report.attached = true;
        report.generation = surface.generation;
        if generation != surface.generation {
            report.generation_gated = true;
        } else if active_generation != Some(surface.generation) {
            report.inactive_gated = true;
        } else {
            let mut out = Vec::new();
            for endpoint in endpoints {
                let Some(node) = surface.nodes.get_mut(&endpoint.processor) else {
                    report.unrouted += 1;
                    continue;
                };
                out.clear();
                match drain(endpoint.handle, &mut out) {
                    Ok(ReportDrain::Drained) => {
                        report.drained += 1;
                        report.events += out.len();
                        for event in &out {
                            let EventKind::ParamValue(param) = event.kind() else {
                                continue;
                            };
                            report.param_reports += 1;
                            let target = node
                                .session
                                .bindings()
                                .find(|b| b.plugin_param == param.param_id)
                                .map(|b| b.synvelis_target);
                            if let Some(target) = target {
                                if node.dispatch.record_reported(target, param.value) {
                                    report.reported += 1;
                                } else {
                                    report.no_pending += 1;
                                }
                            } else {
                                report.unbound_param += 1;
                            }
                        }
                    }
                    Ok(ReportDrain::Unavailable) => {
                        report.unavailable += 1;
                        node.dispatch.mark_reporting_unavailable();
                    }
                    Ok(_) => report.failed += 1,
                    Err(error) => {
                        report.errors.push((endpoint.processor, error));
                        if matches!(error, HostError::InvalidHandle | HostError::Failed) {
                            report.unavailable += 1;
                            node.dispatch.mark_reporting_unavailable();
                        } else {
                            report.failed += 1;
                        }
                    }
                }
            }
        }
        self.surface = Some(surface);
        report
    }

    /// Advances one node's dispatch ledger through one physical call
    /// window — the generation-scoped
    /// [`ParamDispatch::observe_cycle`]: drains the node's scheduler
    /// report channel, applies receipts/faults, projects `Scheduled`
    /// entries onto `[call_start, call_start + call_frames)` of
    /// `span`, and returns observations plus scheduler receipts.
    ///
    /// `generation` must name the attached surface and `processor` a
    /// plugin-backed chain node on it — either mismatch returns
    /// `None`, fail-closed: a foreign generation's window can never
    /// mint `Dispatched` entries here. The caller supplies the
    /// span/call window because the physical call partition lives
    /// inside the RT runtime; this method is the non-RT side
    /// observing what the node reported. Non-RT only.
    #[must_use]
    pub fn observe_cycle(
        &mut self,
        processor: ProcessorId,
        generation: u64,
        span: &TransportSpan,
        call_start: EngineFrame,
        call_frames: u32,
    ) -> Option<DispatchCycle> {
        let surface = self.surface.as_mut()?;
        if surface.generation != generation {
            return None;
        }
        let node = surface.nodes.get_mut(&processor)?;
        Some(node.dispatch.observe_cycle(span, call_start, call_frames))
    }

    /// The processor's [`ProcessBoundarySession`] on the attached
    /// surface — `None` before attach, for processors attached
    /// without a boundary declaration, or when no control was
    /// vended. Read-only evidence accessor; request evaluation goes
    /// through the scoped `boundary_*` methods which hold the
    /// surface's dispatch half.
    #[must_use]
    pub fn boundary_session(&self, processor: ProcessorId) -> Option<&ProcessBoundarySession> {
        self.surface
            .as_ref()?
            .nodes
            .get(&processor)?
            .boundary
            .as_ref()
    }

    /// The session's pinned [`PrecisionProvenance`] — the
    /// instance/node/capability/graph-generation evidence this
    /// surface opened with.
    #[must_use]
    pub fn boundary_provenance(&self, processor: ProcessorId) -> Option<PrecisionProvenance> {
        self.boundary_session(processor)
            .map(ProcessBoundarySession::provenance)
    }

    /// Pure evaluation of one upcoming window through the
    /// processor's [`ProcessBoundarySession`] — provenance check
    /// plus the non-RT oracle with the session's fixed attach
    /// inputs. Nothing is staged.
    ///
    /// # Errors
    /// [`BoundaryStageError::NoSurface`] / [`StaleSurface`] /
    /// [`NoSession`] on surface/session gating; [`Session`] on
    /// stale provenance or oracle refusal.
    pub fn boundary_plan_window(
        &self,
        processor: ProcessorId,
        project: &Project,
        window_frames: u32,
        boundaries: Vec<BoundaryPoint>,
    ) -> Result<ProcessBoundaryPlan, BoundaryStageError> {
        let surface = self.surface.as_ref().ok_or(BoundaryStageError::NoSurface)?;
        if !Self::surface_is_current(surface, project) {
            return Err(BoundaryStageError::StaleSurface);
        }
        let node = surface
            .nodes
            .get(&processor)
            .ok_or(BoundaryStageError::NoSession(processor))?;
        let session = node
            .boundary
            .as_ref()
            .ok_or(BoundaryStageError::NoSession(processor))?;
        let pin = session.provenance();
        let current = PrecisionProvenance::new(
            pin.instance(),
            pin.node(),
            port_capability_generation(project),
            surface.generation,
        );
        session
            .plan_window(&current, window_frames, boundaries)
            .map_err(BoundaryStageError::Session)
    }

    /// [`Self::boundary_plan_window`] plus staging `commands` on the
    /// node's control — the per-request wiring of transport
    /// discontinuities through the generation surface. The session
    /// adds command-derived boundary points, evaluates the plan
    /// first, and only then pushes; an oracle refusal stages
    /// nothing, and a node-side `QueueFull` preserves earlier
    /// commands as staged (bounded-queue reality, typed under
    /// [`BoundaryStageError::Session`]).
    ///
    /// Gated on the RT-observed `active_generation` matching the
    /// surface — the same activation evidence `tick`/`fast_control`
    /// require; a merely-published generation never stages.
    ///
    /// # Errors
    /// As [`Self::boundary_plan_window`], plus
    /// [`BoundaryStageError::InactiveGeneration`] when the surface's
    /// generation is not the RT-active one.
    pub fn boundary_stage_window(
        &mut self,
        processor: ProcessorId,
        project: &Project,
        window_frames: u32,
        boundaries: Vec<BoundaryPoint>,
        commands: &[(TransportCommand, u64)],
        active_generation: Option<u64>,
    ) -> Result<ProcessBoundaryPlan, BoundaryStageError> {
        let surface = self.surface.as_mut().ok_or(BoundaryStageError::NoSurface)?;
        if !Self::surface_is_current(surface, project) {
            return Err(BoundaryStageError::StaleSurface);
        }
        if active_generation != Some(surface.generation) {
            return Err(BoundaryStageError::InactiveGeneration {
                surface: surface.generation,
                active: active_generation,
            });
        }
        let generation = surface.generation;
        let capability = port_capability_generation(project);
        let node = surface
            .nodes
            .get_mut(&processor)
            .ok_or(BoundaryStageError::NoSession(processor))?;
        let Some(session) = node.boundary.as_ref() else {
            return Err(BoundaryStageError::NoSession(processor));
        };
        let pin = session.provenance();
        let current = PrecisionProvenance::new(pin.instance(), pin.node(), capability, generation);
        session
            .stage_window(
                node.dispatch.control(),
                &current,
                window_frames,
                boundaries,
                commands,
            )
            .map_err(BoundaryStageError::Session)
    }

    fn block(&mut self, block: FastControlBlock) -> FastControlOutcome {
        let kind = block.kind();
        self.last_block = Some(block);
        FastControlOutcome::Blocked(kind)
    }

    fn enqueue(&mut self, write: FastControlWrite) -> FastControlOutcome {
        if self.queue.len() >= self.queue_capacity {
            return self.block(FastControlBlock::QueueFull);
        }
        self.queue.push_back(write);
        FastControlOutcome::Queued {
            depth: self.queue.len(),
        }
    }

    /// Builds the generation surface's per-processor control pairs.
    /// One-shot vends that return `None` (static hosts, foreign
    /// generations, already-taken handles) land in
    /// `report.without_control`; bind failures land in
    /// `report.unbound` — both typed, never silent.
    fn vend_nodes(
        &mut self,
        engine: &mut RoutingEngine,
        surface: &mut GenerationSurface,
        project: &Project,
    ) {
        for processor in project.routing.processors.keys() {
            let Some(control) = engine.take_processor_control(surface.generation, *processor)
            else {
                surface.report.without_control.push(*processor);
                continue;
            };
            // PC-C4-02: a control that cannot vouch for the instance's
            // authored plugin identity never backs a binding session —
            // typed `without_control` evidence, not a silent skip.
            let Some(plugin) = control.plugin else {
                surface.report.without_control.push(*processor);
                continue;
            };
            let instance = StablePluginInstanceId::new(self.next_instance);
            self.next_instance = self.next_instance.wrapping_add(1);
            // TIM-03: a vended boundary spec opens the node's
            // per-generation session pinned to this surface's
            // capability/graph generations and the same instance id
            // the binding session mints — one identity per surface.
            // `connect` already projected the identical
            // `attach_delivery`, so an open refusal is unreachable in
            // practice; it still fails closed into typed
            // `without_control` evidence rather than a silent skip.
            let boundary = match control.boundary {
                Some(spec) => {
                    let provenance = PrecisionProvenance::new(
                        instance,
                        control.node,
                        surface.capability_generation,
                        surface.generation,
                    );
                    if let Ok(session) = ProcessBoundarySession::open(
                        &control.control,
                        spec.capability,
                        spec.mode,
                        spec.consent,
                        spec.budget,
                        provenance,
                    ) {
                        Some(session)
                    } else {
                        surface.report.without_control.push(*processor);
                        continue;
                    }
                }
                None => None,
            };
            let session = PluginBindingSession::attach(
                &control.control,
                control.node,
                *processor,
                plugin,
                surface.generation,
                surface.capability_generation,
                instance,
            );
            let Ok(dispatch) =
                ParamDispatch::new(control.control, control.node, control.frame_base)
            else {
                surface.report.without_control.push(*processor);
                continue;
            };
            let mut node = NodeSurface {
                session,
                dispatch,
                boundary,
            };
            Self::bind_node(
                &mut node,
                &surface.catalogs,
                &mut surface.report,
                project,
                *processor,
            );
            surface.report.controlled_processors.push(*processor);
            surface.report.bound_targets += node.session.bindings().count();
            surface.nodes.insert(*processor, node);
        }
    }

    /// Rebinds every eligible target on `node` against the stored
    /// catalogs — used at attach, plan-stale refresh and catalog
    /// refresh. Failures replace the `unbound` evidence.
    fn bind_node(
        node: &mut NodeSurface,
        catalogs: &ParamCatalog,
        report: &mut SurfaceReport,
        project: &Project,
        processor: ProcessorId,
    ) {
        let params: &[ParamInfo] = catalogs.get(&processor).map_or(&[], Vec::as_slice);
        for target in project.control.targets.values() {
            if target.owner != ControlOwner::Processor(processor) || target.capability.is_none() {
                continue;
            }
            match node
                .session
                .bind(&mut node.dispatch, project, target.id, params)
            {
                Ok(_) => {
                    report.unbound.remove(&target.id);
                }
                Err(e) => {
                    report.unbound.insert(target.id, e.to_string());
                }
            }
        }
    }

    /// Refresh path: recompile the plan when the control model drifted
    /// and rebind every node — sessions and dispatches persist
    /// (controls are one-shot), so Equivalent prepares that keep the
    /// generation still pick up new automation lanes and capability
    /// changes. Nodes whose session pins no longer match the live
    /// capability generation are skipped — `surface_is_current`
    /// reports the surface stale for `sync` instead.
    fn refresh_surface(
        surface: &mut GenerationSurface,
        project: &Project,
        catalogs: &ParamCatalog,
    ) {
        surface.catalogs = catalogs.clone();
        if surface.plan.is_stale(project)
            && let Ok(plan) = ControlGraph::compile(project)
        {
            surface.scratch = plan.scratch();
            surface.plan = plan;
        }
        let capability = port_capability_generation(project);
        surface.report.unbound.clear();
        surface.report.bound_targets = 0;
        for (processor, node) in &mut surface.nodes {
            if !node.session.is_current(capability, surface.generation) {
                continue;
            }
            // Enumeration drift flips bindings defunct before rebind —
            // a param the plugin dropped can never keep delivering.
            let params: &[ParamInfo] = catalogs.get(processor).map_or(&[], Vec::as_slice);
            let bound: Vec<ParameterTargetId> =
                node.session.bindings().map(|b| b.synvelis_target).collect();
            for target in bound {
                node.session.revalidate(&mut node.dispatch, target, params);
            }
            Self::bind_node(node, catalogs, &mut surface.report, project, *processor);
            surface.report.bound_targets += node.session.bindings().count();
        }
        surface.report.controlled_processors = surface.nodes.keys().copied().collect();
        surface.report.without_control = project
            .routing
            .processors
            .keys()
            .filter(|id| !surface.nodes.contains_key(*id))
            .copied()
            .collect();
    }

    /// `false` when any node's session pins no longer match the live
    /// project's capability generation — the surface needs a new
    /// generation (`sync`), not a refresh.
    fn surface_is_current(surface: &GenerationSurface, project: &Project) -> bool {
        let capability = port_capability_generation(project);
        surface
            .nodes
            .values()
            .all(|node| node.session.is_current(capability, surface.generation))
    }

    /// Drains the bounded fast-control queue into per-node batches —
    /// each write re-resolved through the *current* surface's verified
    /// binding (never a stale `(node, param)` pair). Writes that can
    /// never deliver on this surface drop typed.
    fn drain_queue(
        &mut self,
        surface: &GenerationSurface,
        project: &Project,
        batches: &mut BTreeMap<ProcessorId, Vec<BatchEntry>>,
        report: &mut TickReport,
    ) {
        let drained: Vec<FastControlWrite> = self.queue.drain(..).collect();
        for write in drained {
            match Self::lower_for_node(surface, project, &write) {
                Ok((processor, entry)) => {
                    report.queue_delivered += 1;
                    batches.entry(processor).or_default().push(entry);
                }
                Err(_) => report.queue_dropped += 1,
            }
        }
    }

    /// Collects automation evaluations into per-node batches.
    fn collect_automation(
        surface: &mut GenerationSurface,
        project: &Project,
        pos: BeatPoint,
        position: MediaTime,
        batches: &mut BTreeMap<ProcessorId, Vec<BatchEntry>>,
        report: &mut TickReport,
    ) {
        for (processor, node) in &mut surface.nodes {
            let targets: Vec<ParameterTargetId> = node
                .session
                .bindings()
                .map(|b| b.synvelis_target)
                .filter(|target| {
                    project
                        .control
                        .targets
                        .get(target)
                        .is_some_and(|t| t.owner == ControlOwner::Processor(*processor))
                })
                .collect();
            for target in targets {
                report.evaluated += 1;
                match node.session.eval_event(
                    project,
                    &surface.plan,
                    &mut surface.scratch,
                    target,
                    pos,
                    None,
                    position,
                    ParamWriteMeans::TimedEvent,
                ) {
                    Ok(event) => {
                        if let Some(binding) = node.session.binding(target).cloned() {
                            batches.entry(*processor).or_default().push(BatchEntry {
                                binding,
                                event,
                                requeue: None,
                            });
                        } else {
                            report.eval_skipped += 1;
                        }
                    }
                    Err(_) => report.eval_skipped += 1,
                }
            }
        }
    }

    /// Stages every node's merged batch through its pinned session —
    /// one atomic [`PluginBindingSession::dispatch`] per node so queued
    /// fast controls and automation events land in the same plan.
    /// `QueueFull` re-queues the batch's fast-control writes; other
    /// failures drop them typed (automation re-evaluates next tick).
    fn flush_batches(
        &mut self,
        surface: &mut GenerationSurface,
        batches: &mut BTreeMap<ProcessorId, Vec<BatchEntry>>,
        report: &mut TickReport,
    ) {
        for (processor, entries) in std::mem::take(batches) {
            if entries.is_empty() {
                continue;
            }
            let Some(node) = surface.nodes.get_mut(&processor) else {
                continue;
            };
            let writes: Vec<(PluginParameterBinding, TimedParamEvent)> = entries
                .iter()
                .map(|e| (e.binding.clone(), e.event))
                .collect();
            match node.session.dispatch(&mut node.dispatch, &writes) {
                Ok(_) => {
                    report.nodes_dispatched += 1;
                    report.events += entries.len();
                }
                Err(e) => {
                    report.dispatch_errors += 1;
                    if matches!(e, BindingError::Dispatch(DispatchError::QueueFull)) {
                        for entry in entries.iter().rev() {
                            if let Some(write) = &entry.requeue {
                                self.queue.push_front(write.clone());
                                report.queue_retained += 1;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Resolves `write.target` to its owning node surface and lowers it
    /// to a pinned batch entry. `Err` means the write can never deliver
    /// on this surface (unbound target, owner drift, lowering
    /// rejection) — queue drain drops it, `fast_control` blocks it.
    fn lower_for_node(
        surface: &GenerationSurface,
        project: &Project,
        write: &FastControlWrite,
    ) -> Result<(ProcessorId, BatchEntry), LowerFail> {
        let project_target = project
            .control
            .targets
            .get(&write.target)
            .ok_or(LowerFail::Unbound)?;
        let ControlOwner::Processor(processor) = project_target.owner else {
            return Err(LowerFail::Unbound);
        };
        let node = surface.nodes.get(&processor).ok_or(LowerFail::Unbound)?;
        let Some(binding) = node.session.binding(write.target).cloned() else {
            return Err(LowerFail::Unbound);
        };
        let event = lower_event(project_target, write.value, write.position, write.means)
            .map_err(LowerFail::Lowering)?;
        Ok((
            processor,
            BatchEntry {
                binding,
                event,
                requeue: Some(write.clone()),
            },
        ))
    }
}
