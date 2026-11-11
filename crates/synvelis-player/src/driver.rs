//! `HeadlessDriver` — the deterministic non-RT playback owner: graph
//! publish with last-good semantics, manually pumped prefetch
//! producers, per-block `RtRuntime` processing and master-output
//! capture. Seek/stop are explicit; underrun/EOF live in per-clip
//! [`PlaybackStats`], never folded into "successful audio".

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use synvelis_project::{ClipId, ParameterTargetId};

use ruviform_audio::typed::{
    ActivationError, CompileError, Compiler, CompilerConfig, FactoryError, GraphProcessor,
    GraphRuntime, InstalledInfo, PrepareOutcome, ProcessError, RtRuntime,
};
use ruviform_audio_plugin_bridge::{ParamValueObservation, PluginNode};
use ruviform_graph::{GraphProject, GraphRevision, NodeId, NodeSpec, PortId, PrepareError};
use ruviform_media_core::{
    AppliedReceipt, ClockEpochId, EngineFrame, LoopOccurrence, MediaDuration, MediaRange,
    MediaTime, PlanGeneration, SpanContext, TempoMapRevision, TimeBase, TransportCommand,
    TransportGeneration, TransportSpan, TransportStatus,
};
use ruviform_media_reader::{PrefetchHandle, PrefetchProducer, PumpOutcome, SourceFrame};
use ruviform_rt::RtContext;

use crate::binding::StablePluginInstanceId;
use crate::dispatch::ParamDispatch;
use crate::lowering::{EngineConfig, Lowered, NodeContent, PlaybackBinding};
use crate::process_boundary::{
    BoundarySessionError, BoundarySessionSpec, PrecisionProvenance, ProcessBoundarySession,
};
use crate::processor::{ClipPlayer, ClipSync, PlaybackStats};
use crate::resolver::PreparedClip;

/// Explicit transport state — `Stopped` is a state, not an absence.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DriverState {
    /// No blocks are being processed.
    #[default]
    Stopped,
    /// Blocks are being processed.
    Playing,
}

/// When prefetch producers pump relative to blocks.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PumpPolicy {
    /// Pump every producer once every `n`-th block (`0` = never —
    /// deterministic underrun).
    Every(u64),
}

/// Captured playback evidence from [`HeadlessDriver::run`].
#[derive(Clone, Debug)]
pub struct PlayReport {
    /// Master output, planar per channel.
    pub pcm: Vec<Vec<f32>>,
    /// Folded sample hash over all captured channels.
    pub crc: u64,
    /// Engine frames processed.
    pub frames: u64,
}

/// Failure of graph activation: validation, compile or publish —
/// the previously published plan (if any) stays live.
#[derive(Debug)]
pub enum ActivateError {
    /// `GraphProject::prepare` rejected the spec.
    Validation(PrepareError),
    /// The typed compiler rejected the validated graph.
    Compile(CompileError),
    /// `GraphRuntime::publish` failed to queue the plan.
    Publish(ActivationError),
    /// An [`AttachedNode`] named a node id with no `Processor` slot
    /// in the spec — every attached node must fill one
    /// `lowered.inserts` entry (dupes count as unmatched).
    UnmatchedAttached(NodeId),
    /// A boundary-declared attached node's session open or anchor
    /// staging refused — activation aborts before `prepare`, so no
    /// revision is recorded and nothing publishes. The typed
    /// [`BoundarySessionError`] is preserved verbatim.
    BoundarySession(BoundarySessionError),
}

impl fmt::Display for ActivateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(e) => write!(f, "graph validation failed: {e}"),
            Self::Compile(e) => write!(f, "graph compile failed: {e}"),
            Self::Publish(e) => write!(f, "graph publish failed: {e}"),
            Self::UnmatchedAttached(id) => {
                write!(f, "attached node {id} has no insert slot in the spec")
            }
            Self::BoundarySession(e) => write!(f, "attached boundary session refused: {e}"),
        }
    }
}

impl std::error::Error for ActivateError {}

/// A plugin node bound into the driver: the RT-side [`PluginNode`]
/// moves into its lowered `Processor` slot at activation while the
/// [`ParamDispatch`] orchestrator (owning the node's control handle)
/// stays on the non-RT side for binds, writes and observation.
///
/// The driver owns transport for attached nodes: `Play` is requested
/// at activation (position == engine frame from block zero) and
/// [`HeadlessDriver::seek`] re-anchors both the node's cursor and its
/// position, so plan events addressed by absolute engine frame land
/// exactly on it.
pub struct AttachedNode {
    /// The RT-side node — `ruviform_audio_plugin_bridge::Attached::node`.
    pub node: PluginNode,
    /// The dispatch orchestrator — owns `Attached::control`; its
    /// `node()` must be the `Processor` slot the node installs on.
    pub dispatch: ParamDispatch,
    /// TIM-03 attach-resolved boundary inputs — `Some` when the
    /// processor was attached under a boundary declaration (the
    /// [`crate::ProcessorControl::boundary`] spec vend path). The
    /// driver opens a [`ProcessBoundarySession`] per node at
    /// activation, pinning provenance to the driver-minted instance
    /// id, the node id, capability generation `0` (driver-attached
    /// nodes carry no project capability surface — the instance and
    /// graph-generation axes carry the pin) and the published plan
    /// generation. `None` keeps the legacy lenient transport path.
    ///
    /// Caller contract: the spec must be the same inputs the host
    /// projected onto `AttachConfig::{policy, budget}` at connect —
    /// [`ProcessorHost::boundary_attach`](crate::ProcessorHost::boundary_attach)
    /// vends exactly that. A mismatched spec fails closed at session
    /// open (`attach_delivery` re-evaluation).
    pub boundary: Option<BoundarySessionSpec>,
}

/// One attached node's non-RT orchestration state: the param/MIDI
/// dispatch half, the optional TIM-03 boundary session, the
/// transport mirror projecting the span context, and the last span
/// handed to `observe_cycle` (test/diagnostic evidence).
struct AttachedDispatch {
    /// Param/MIDI dispatch orchestrator — owns the node control.
    dispatch: ParamDispatch,
    /// TIM-03 per-activation [`ProcessBoundarySession`] — `Some`
    /// only for boundary-declared attaches.
    boundary: Option<ProcessBoundarySession>,
    /// Driver-side projection of the transport state the node
    /// scheduler was commanded into — supplies the honest
    /// [`SpanContext`] fields `observe_attached_block` used to
    /// fabricate.
    transport: TransportMirror,
    /// The span context last projected for this node.
    last_span: Option<TransportSpan>,
}

/// Driver-side mirror of the node scheduler's transport state.
///
/// The scheduler's own [`TransportScheduler::apply`]/`do_wrap` rules
/// are projected for the state the *driver* commanded: status,
/// position anchor, loop region and occurrence. Because the driver
/// only ever commands `due = 0` transport messages, every staged
/// command applies at the next callback head — the staging-time
/// cursor is the apply frame the mirror anchors on. Staging success
/// updates the mirror optimistically; node-side `Rejected` receipts
/// correct only the generation axes (command kind is not carried by
/// [`AppliedReceipt`], so status/position/loop stay commanded-state
/// projections — documented limitation, receipts are the authority
/// for `transport_generation`/`plan_generation`).
///
/// Position/occurrence at an arbitrary frame are folded algebraically
/// rather than stepped per wrap: with anchor `(af, ap)` and loop
/// `(s, e)`, `e > s`, the playing position `p = ap + (f - af)` wraps
/// `w = (p - e) / (e - s) + 1` times when `p >= e`, landing at
/// `p - w·(e - s) ∈ [s, e)` — identical to the scheduler's sequential
/// `do_wrap` (`anchor_pos = ls + overflow`) including the immediate
/// cascade when the anchor already sits past the loop end. The
/// `Degraded` deferral path (wrap deferred to a legal boundary with
/// `overflow > 0`) is unreachable under the driver's activated
/// contract — every non-zero span is legal.
///
/// Known edge: a same-head command batch whose earlier `SetLoop`
/// leaves the raw position already past the *new* loop end, followed
/// by a preempting `Stop`/`Seek`/`ClearLoop`, can leave the mirror's
/// frozen `occurrence` ahead of the scheduler's (the scheduler
/// resets occurrence at `SetLoop` and counts the new position's
/// wraps only). `occurrence` reaches consumers as trace/diagnostic
/// metadata only — event gating reads status/position/generations —
/// so the divergence is evidence-level, never delivery-level.
#[derive(Debug)]
struct TransportMirror {
    /// Scheduler-equivalent transport status.
    status: TransportStatus,
    /// Position anchor frame (engine frames — the node's cursor
    /// domain after `set_clock`).
    anchor_frame: u64,
    /// Position at `anchor_frame`, in position ticks.
    anchor_pos: i64,
    /// Loop region `(start, end)` in position ticks — mirrors the
    /// scheduler's `loop_ticks`.
    loop_ticks: Option<(i64, i64)>,
    /// Loop occurrence at `anchor_frame`; wraps between the anchor
    /// and a queried frame are folded by [`Self::position_at`].
    occurrence: LoopOccurrence,
    /// Transport generation confirmed by the node's own receipts —
    /// the last drained [`AppliedReceipt`]'s value, or the
    /// scheduler's initial `TransportGeneration::new(0)` before the
    /// first receipt.
    transport_generation: TransportGeneration,
    /// Predicted transport generation while staged tg-bumping
    /// commands (Play/Pause/Stop/Seek) await their apply receipts —
    /// `effective` reports this over `transport_generation` until
    /// the first receipt of the batch arrives. Caller-side commands
    /// the mirror cannot see (e.g. `ParamDispatch::dispatch`'s
    /// internal `Play`) are corrected by the next receipt — param
    /// plans bind no transport generation, so the lag is inert for
    /// event projection.
    pending_generation: Option<TransportGeneration>,
}

impl TransportMirror {
    /// Scheduler-initial state: `Stopped`, zeroed anchor, no loop,
    /// generation/occurrence 0 — identical to
    /// `TransportScheduler::new`.
    fn new() -> Self {
        Self {
            status: TransportStatus::Stopped,
            anchor_frame: 0,
            anchor_pos: 0,
            loop_ticks: None,
            occurrence: LoopOccurrence::new(0),
            transport_generation: TransportGeneration::new(0),
            pending_generation: None,
        }
    }

    /// The transport generation the next span context reports:
    /// the pending prediction while staged bumps await receipts,
    /// else the last receipt-confirmed value.
    fn effective_generation(&self) -> TransportGeneration {
        self.pending_generation.unwrap_or(self.transport_generation)
    }

    /// The scheduler's raw `pos_at`: `anchor_pos + (frame -
    /// anchor_frame)` while `Playing`, frozen otherwise — *no* loop
    /// fold (the scheduler folds only by re-anchoring at wrap
    /// boundaries). Command applies read this value.
    fn raw_position_at(&self, frame: u64) -> i64 {
        if self.status != TransportStatus::Playing {
            return self.anchor_pos;
        }
        let delta = i64::try_from(frame.saturating_sub(self.anchor_frame)).unwrap_or(i64::MAX);
        self.anchor_pos.saturating_add(delta)
    }

    /// `(position_ticks, occurrence)` at engine `frame` — the
    /// algebraic fold described on [`TransportMirror`]. Equivalent
    /// to the scheduler's sequential `do_wrap` re-anchoring for every
    /// frame at or after `anchor_frame`. Saturating arithmetic
    /// mirrors `pos_at`'s `saturating_add`.
    fn position_at(&self, frame: u64) -> (i64, LoopOccurrence) {
        if self.status != TransportStatus::Playing {
            return (self.anchor_pos, self.occurrence);
        }
        let mut pos = self.raw_position_at(frame);
        let mut occurrence = self.occurrence.get();
        if let Some((start, end)) = self.loop_ticks
            && end > start
            && pos >= end
        {
            // i64 math: `pos - end >= 0`, `end - start > 0` — the
            // wrap count is exact, never negative.
            let wraps = (pos - end) / (end - start) + 1;
            pos -= wraps * (end - start);
            occurrence = occurrence.saturating_add(u64::try_from(wraps).unwrap_or(u64::MAX));
        }
        (pos, LoopOccurrence::new(occurrence))
    }

    /// The [`SpanContext`] the node scheduler would stamp on a span
    /// starting at engine `frame`, given the mirror's commanded
    /// state and the caller-supplied plan generation. `tempo_revision`
    /// stays `TempoMapRevision::new(0)` — every plan this driver
    /// installs is built through `plan_builder`, which never attaches
    /// a tempo map (revision 0 by construction; real tempo maps are
    /// the Tempo slice's scope).
    fn span_context(
        &self,
        frame: u64,
        position_base: TimeBase,
        clock_epoch: ClockEpochId,
        plan_generation: PlanGeneration,
    ) -> SpanContext {
        let (position_ticks, occurrence) = self.position_at(frame);
        SpanContext {
            status: self.status,
            position_ticks,
            position_base,
            loop_ticks: self.loop_ticks,
            occurrence,
            clock_epoch,
            transport_generation: self.effective_generation(),
            plan_generation,
            tempo_revision: TempoMapRevision::new(0),
        }
    }

    /// Folds drained scheduler receipts into the mirror: the last
    /// receipt's `transport_generation` is the newest scheduler
    /// truth and clears the pending prediction. Receipts of every
    /// lifecycle stage (`Requested`/`Pending`/`Applied`/`Rejected`)
    /// carry the generation live at production time, so the last one
    /// in the drain is always the freshest known value.
    fn note_receipts(&mut self, receipts: &[AppliedReceipt]) {
        if let Some(last) = receipts.last() {
            self.transport_generation = last.transport_generation;
            self.pending_generation = None;
        }
    }

    /// Re-anchors `(anchor_frame, anchor_pos, occurrence)` — the
    /// mirror's equivalent of the scheduler moving its anchor.
    /// `occurrence` must be the value *at* `frame` (from
    /// [`Self::position_at`]) so wraps between the old anchor and
    /// `frame` are not lost or double-counted.
    fn anchor_at(&mut self, frame: u64, position: i64, occurrence: LoopOccurrence) {
        self.anchor_frame = frame;
        self.anchor_pos = position;
        self.occurrence = occurrence;
    }

    /// Occurrence counting loop wraps with boundary strictly before
    /// `frame` — the value a command applying at `frame` freezes.
    /// Upstream applies due commands before evaluating a wrap at the
    /// same head, so `Stop`/`Seek`/`ClearLoop` see only earlier
    /// wraps; `position_at` counts the head wrap inclusively, so it
    /// is subtracted back out here. A head wrap exists iff the raw
    /// position lands exactly on the loop end at `frame` (positions
    /// advance one tick per frame, so boundaries are `loop_len`
    /// apart and at most one coincides with a frame).
    fn occurrence_before(&self, frame: u64) -> LoopOccurrence {
        let (_, inclusive) = self.position_at(frame);
        let head_wrap = self.status == TransportStatus::Playing
            && self.loop_ticks.is_some_and(|(start, end)| {
                end > start && {
                    let raw = self.raw_position_at(frame);
                    raw >= end && (raw - end) % (end - start) == 0
                }
            });
        LoopOccurrence::new(inclusive.get().saturating_sub(u64::from(head_wrap)))
    }

    /// `Play` applied at `frame`: status flips and the anchor
    /// captures the running position — folded `position_at` already
    /// accounts a wrap due at the same head (the scheduler applies
    /// the command first, then wraps; `Play` does not move the
    /// position, so the head-wrap count carries over unchanged).
    /// Transport generation bumps.
    fn note_play(&mut self, frame: u64) {
        let (pos, occurrence) = self.position_at(frame);
        self.status = TransportStatus::Playing;
        self.anchor_at(frame, pos, occurrence);
        self.note_tg_bump();
    }

    /// `Stop` applied at `frame`: frozen at position 0 (a later bare
    /// `Play` restarts at 0 — `resume_attached_transport` exists for
    /// that reason), generation bumps. Occurrence freezes at its
    /// pre-stop value — a `Stopped` scheduler processes no wraps, and
    /// the apply preempts a wrap due at the same head.
    fn note_stop(&mut self, frame: u64) {
        let occurrence = self.occurrence_before(frame);
        self.status = TransportStatus::Stopped;
        self.anchor_at(frame, 0, occurrence);
        self.note_tg_bump();
    }

    /// `Seek(t)` applied at `frame`: position re-anchors to the raw
    /// `t`, status carries over, generation bumps. The occurrence
    /// base excludes a wrap due at the apply head under the *old*
    /// position; wraps the new position coincides with — including
    /// a full cascade when `t` lands past the loop end — are folded
    /// lazily by `position_at`, matching the scheduler's same-head
    /// wrap cascade.
    fn note_seek(&mut self, frame: u64, position: i64) {
        let occurrence = self.occurrence_before(frame);
        self.anchor_at(frame, position, occurrence);
        self.note_tg_bump();
    }

    /// `SetLoop(s, e)` applied at `frame`: the region installs and
    /// occurrence restarts at 0. The anchor captures the *raw*
    /// position — the scheduler keeps its anchor untouched and the
    /// new loop (not the old one) folds whatever the position had
    /// already passed, including wraps before `frame` that collapse
    /// onto the apply head upstream; the lazy fold counts them
    /// identically.
    fn note_set_loop(&mut self, frame: u64, region: (i64, i64)) {
        let pos = self.raw_position_at(frame);
        self.loop_ticks = Some(region);
        self.anchor_at(frame, pos, LoopOccurrence::new(0));
    }

    /// `ClearLoop` applied at `frame`: the region clears and the raw
    /// position line continues unfolded; occurrence keeps its
    /// pre-command value (a `loop_ticks: None` scheduler processes no
    /// wraps, and the apply preempts a wrap due at the same head).
    fn note_clear_loop(&mut self, frame: u64) {
        let pos = self.raw_position_at(frame);
        let occurrence = self.occurrence_before(frame);
        self.loop_ticks = None;
        self.anchor_at(frame, pos, occurrence);
    }

    /// One unapplied tg-bumping command staged successfully — the
    /// predicted generation advances one past the current effective
    /// value (each of Play/Pause/Stop/Seek bumps exactly once at
    /// apply).
    fn note_tg_bump(&mut self) {
        let next = TransportGeneration::new(self.effective_generation().get().saturating_add(1));
        self.pending_generation = Some(next);
    }
}

/// Deterministic headless playback driver for the L1 slice.
pub struct HeadlessDriver {
    engine: EngineConfig,
    project: GraphProject,
    compiler: Compiler,
    graph: GraphRuntime,
    rt: RtRuntime,
    ctx: RtContext,
    clock: Arc<AtomicU64>,
    cursor: u64,
    state: DriverState,
    producers: Vec<PrefetchProducer>,
    handles: Vec<PrefetchHandle>,
    seek_nonces: Vec<Arc<AtomicU64>>,
    drain_nonces: Vec<Arc<AtomicU64>>,
    bindings: Vec<PlaybackBinding>,
    stats: Vec<(NodeContent, Arc<PlaybackStats>)>,
    master_in: Option<PortId>,
    generation: u64,
    /// Attached plugin nodes' non-RT orchestration halves — the RT
    /// halves live inside the published plan. Transport ownership:
    /// the driver requests `Play` at activation and re-anchors on
    /// `seek`, staging every transport command through the node's
    /// [`ProcessBoundarySession`] when one is open.
    attached: Vec<AttachedDispatch>,
    /// Monotonic instance-id source for boundary-session provenance
    /// — one fresh [`StablePluginInstanceId`] per attached node per
    /// activation (a re-attach mints a new identity, matching the
    /// provenance contract).
    next_instance: u64,
    /// Latest per-target observation merged across attached nodes —
    /// refreshed per block inside [`HeadlessDriver::run`].
    param_observations: BTreeMap<ParameterTargetId, ParamValueObservation>,
    /// Clock epoch of the driver's frame series — the scheduler
    /// epoch attached nodes start in. `device_restart` is never sent,
    /// so this stays the node's live epoch.
    epoch: ClockEpochId,
}

impl HeadlessDriver {
    /// Creates an empty driver over `engine` geometry.
    #[must_use]
    pub fn new(engine: EngineConfig) -> Self {
        let config = CompilerConfig {
            max_frames: engine.block_frames,
            ..CompilerConfig::default()
        };
        let (graph, rt) = GraphRuntime::new(&config, 64, 64);
        Self {
            engine,
            project: GraphProject::new(),
            compiler: Compiler::new(config),
            graph,
            rt,
            ctx: RtContext::mint(),
            clock: Arc::new(AtomicU64::new(0)),
            cursor: 0,
            state: DriverState::Stopped,
            producers: Vec::new(),
            handles: Vec::new(),
            seek_nonces: Vec::new(),
            drain_nonces: Vec::new(),
            bindings: Vec::new(),
            stats: Vec::new(),
            master_in: None,
            generation: 1,
            attached: Vec::new(),
            next_instance: 1,
            param_observations: BTreeMap::new(),
            epoch: ClockEpochId::new(0),
        }
    }

    /// Publishes `lowered` and installs one [`ClipPlayer`] per binding.
    ///
    /// `prepared` must carry one [`PreparedClip`] per `lowered.playbacks`
    /// entry, in the same order — each node gets its own reader so a
    /// shared source plays through independent decode paths. Each
    /// consumer is initially aligned to its window's `source_start`.
    ///
    /// On validation/compile/publish failure the previously published
    /// plan keeps running (last-good); with none, the driver stays
    /// [`DriverState::Stopped`].
    ///
    /// # Errors
    /// [`ActivateError`] — the attempted revision is recorded on the
    /// project either way.
    ///
    /// # Panics
    /// When the graph project fails to expose the plan it just
    /// published (internal invariant violation).
    pub fn activate(
        &mut self,
        lowered: &Lowered,
        prepared: Vec<PreparedClip>,
    ) -> Result<u64, ActivateError> {
        self.activate_with_attached(lowered, prepared, Vec::new())
    }

    /// [`activate`] plus [`AttachedNode`]s installed into the spec's
    /// `Processor` slots (the `lowered.inserts` chain). Every attached
    /// `dispatch.node()` must name an insert slot — anything else is
    /// [`ActivateError::UnmatchedAttached`] before the spec is even
    /// validated. An insert slot left without an attached node
    /// surfaces as the compiler's `MissingProcessor`, keeping the
    /// previous plan live.
    ///
    /// On success each attached node's transport is re-anchored to
    /// the driver's cursor (`set_clock` + `Seek`) and `Play` is
    /// requested — plan positions are engine frames, so the node's
    /// position tracks the driver's frame series 1:1 even when
    /// activation happens mid-run.
    ///
    /// # Errors
    /// [`ActivateError`] — the attempted revision is recorded on the
    /// project either way, except [`ActivateError::UnmatchedAttached`]
    /// which returns before `prepare` and records nothing.
    ///
    /// # Panics
    /// When the graph project fails to expose the plan it just
    /// published (internal invariant violation).
    pub fn activate_with_attached(
        &mut self,
        lowered: &Lowered,
        prepared: Vec<PreparedClip>,
        attached: Vec<AttachedNode>,
    ) -> Result<u64, ActivateError> {
        let mut slots: std::collections::BTreeSet<NodeId> =
            lowered.inserts.iter().map(|i| i.node_id).collect();
        for node in &attached {
            if !slots.remove(&node.dispatch.node()) {
                return Err(ActivateError::UnmatchedAttached(node.dispatch.node()));
            }
        }
        let mut pending: BTreeMap<NodeId, Box<dyn GraphProcessor>> = BTreeMap::new();
        let mut attached_dispatch = Vec::with_capacity(attached.len());
        for node in attached {
            let node_id = node.dispatch.node();
            // `slots.remove` above already proved id uniqueness.
            pending.insert(node_id, Box::new(node.node));
            // TIM-03: open the boundary session (when declared) and
            // stage the transport anchor *before* `prepare` — a
            // sessioned node's refusal aborts activation with
            // nothing published, matching `UnmatchedAttached`.
            attached_dispatch.push(self.prepare_attached(node.dispatch, node.boundary)?);
        }
        let mut stats = Vec::with_capacity(lowered.playbacks.len());
        let mut producers = Vec::with_capacity(lowered.playbacks.len());
        let mut handles = Vec::with_capacity(lowered.playbacks.len());
        let mut nonces = Vec::with_capacity(lowered.playbacks.len());
        let mut drains = Vec::with_capacity(lowered.playbacks.len());
        for (binding, mut clip) in lowered.playbacks.iter().zip(prepared) {
            let stat = Arc::new(PlaybackStats::default());
            let nonce = Arc::new(AtomicU64::new(0));
            let drain = Arc::new(AtomicU64::new(0));
            pending.insert(
                binding.node_id,
                Box::new(ClipPlayer::new(
                    clip.consumer,
                    binding.window,
                    binding.fade,
                    ClipSync {
                        clock: Arc::clone(&self.clock),
                        pending_seek: Arc::clone(&nonce),
                        drain: Arc::clone(&drain),
                    },
                    Arc::clone(&stat),
                    self.engine.block_frames,
                    clip.canonical.layout.channels() as usize,
                )),
            );
            stats.push((binding.content, stat));
            if clip
                .handle
                .seek(SourceFrame(binding.window.source_start))
                .is_ok()
            {
                nonce.fetch_add(1, Ordering::Relaxed);
            }
            producers.push(clip.producer);
            handles.push(clip.handle);
            nonces.push(nonce);
            drains.push(drain);
        }

        let mut factory = |node: &NodeSpec,
                           _existing: Option<&InstalledInfo>|
         -> Result<PrepareOutcome, FactoryError> {
            pending
                .remove(&node.id())
                .map_or(Ok(PrepareOutcome::Boundary), |proc| {
                    Ok(PrepareOutcome::New(proc))
                })
        };
        self.project
            .prepare(lowered.spec.clone())
            .map_err(ActivateError::Validation)?;
        let validated = self
            .project
            .published()
            .expect("prepare just published")
            .graph();
        let generation = self.generation;
        let compiled = self
            .compiler
            .compile(
                validated,
                &mut factory,
                self.graph.registry(),
                self.graph.allocator(),
                generation,
            )
            .map_err(ActivateError::Compile)?;
        self.graph
            .publish(compiled)
            .map_err(ActivateError::Publish)?;
        self.bindings.clone_from(&lowered.playbacks);
        self.stats = stats;
        self.producers = producers;
        self.handles = handles;
        self.seek_nonces = nonces;
        self.drain_nonces = drains;
        self.generation += 1;
        self.master_in = Some(lowered.master_in);
        self.param_observations.clear();
        self.attached = attached_dispatch;
        Ok(generation)
    }

    /// Builds one [`AttachedDispatch`] at activation: opens the
    /// [`ProcessBoundarySession`] when `boundary` declares one
    /// (provenance pinned to a fresh driver-minted instance id, the
    /// node id, capability generation `0` — driver-attached nodes
    /// carry no project capability surface — and this activation's
    /// plan `generation`), then stages the transport anchor through
    /// [`Self::anchor_link`]. Session open/stage refusals surface as
    /// [`ActivateError::BoundarySession`] before anything publishes.
    fn prepare_attached(
        &mut self,
        mut dispatch: ParamDispatch,
        boundary: Option<BoundarySessionSpec>,
    ) -> Result<AttachedDispatch, ActivateError> {
        let node_id = dispatch.node();
        let boundary = match boundary {
            Some(spec) => {
                let provenance = PrecisionProvenance::new(
                    StablePluginInstanceId::new(self.next_instance),
                    node_id,
                    0,
                    self.generation,
                );
                self.next_instance = self.next_instance.wrapping_add(1);
                Some(
                    ProcessBoundarySession::open(
                        dispatch.control(),
                        spec.capability,
                        spec.mode,
                        spec.consent,
                        spec.budget,
                        provenance,
                    )
                    .map_err(ActivateError::BoundarySession)?,
                )
            }
            None => None,
        };
        let mut transport = TransportMirror::new();
        Self::anchor_link(
            &mut dispatch,
            boundary.as_ref(),
            &mut transport,
            self.epoch,
            self.cursor,
            self.window_frames(),
        )
        .map_err(ActivateError::BoundarySession)?;
        Ok(AttachedDispatch {
            dispatch,
            boundary,
            transport,
            last_span: None,
        })
    }

    /// The upcoming callback window every staged command is
    /// legality-checked against — the driver processes exactly one
    /// window shape, `engine.block_frames`.
    fn window_frames(&self) -> u32 {
        u32::try_from(self.engine.block_frames).unwrap_or(u32::MAX)
    }

    /// Stages the activation/resume anchor on one attached node:
    /// `set_clock(epoch, cursor)` then `Seek(position = cursor)` +
    /// `Play` — position == engine frame is the driver's invariant,
    /// so plan events addressed by absolute engine frame land on it.
    ///
    /// A boundary-declared node stages through its
    /// [`ProcessBoundarySession`] (`stage_window` evaluates the
    /// oracle before anything pushes; the session's provenance pin
    /// is the honest `current` — the driver binds session and
    /// dispatch for the node's whole attached lifetime, so no drift
    /// channel exists). `set_clock` is a clock rebase, not a
    /// transport command — it maps its `QueueFull` onto the same
    /// bounded-channel refusal. Non-declared nodes keep the legacy
    /// lenient path; the mirror records only commands that actually
    /// queued.
    ///
    /// # Errors
    /// [`BoundarySessionError`] — oracle/provenance refusal or a
    /// bounded-queue drop on a sessioned node. On `QueueFull` an
    /// earlier command of the batch may remain staged (bounded-queue
    /// reality); the mirror keeps the last fully-successful state.
    fn anchor_link(
        dispatch: &mut ParamDispatch,
        boundary: Option<&ProcessBoundarySession>,
        transport: &mut TransportMirror,
        epoch: ClockEpochId,
        cursor: u64,
        window_frames: u32,
    ) -> Result<(), BoundarySessionError> {
        let position = i64::try_from(cursor).unwrap_or(i64::MAX);
        let seek = TransportCommand::Seek(MediaTime::new(position, dispatch.frame_base()));
        if let Some(session) = boundary {
            dispatch
                .control()
                .set_clock(epoch, cursor)
                .map_err(|_| BoundarySessionError::QueueFull)?;
            session.stage_window(
                dispatch.control(),
                &session.provenance(),
                window_frames,
                Vec::new(),
                &[(seek, 0), (TransportCommand::Play, 0)],
            )?;
            transport.note_seek(cursor, position);
            transport.note_play(cursor);
        } else {
            let _ = dispatch.control().set_clock(epoch, cursor);
            if dispatch.control().request(seek, 0).is_ok() {
                transport.note_seek(cursor, position);
            }
            if dispatch
                .control()
                .request(TransportCommand::Play, 0)
                .is_ok()
            {
                transport.note_play(cursor);
            }
        }
        Ok(())
    }

    /// Stages one transport `command` (`due = 0` — the next callback
    /// head) on every attached node and folds the mirror forward on
    /// success. Sessioned nodes evaluate through
    /// [`ProcessBoundarySession::stage_window`] first — an oracle or
    /// provenance refusal stages nothing; non-declared nodes use the
    /// raw control request. Returns the first typed refusal —
    /// earlier nodes may already hold the command (same partial
    /// staging contract as before).
    fn stage_transport(&mut self, command: TransportCommand) -> Result<(), BoundarySessionError> {
        let cursor = self.cursor;
        let window = self.window_frames();
        for attached in &mut self.attached {
            Self::stage_node_transport(attached, cursor, window, command)?;
        }
        Ok(())
    }

    /// [`Self::stage_transport`] for a single node — the sessioned
    /// path goes through the oracle, the legacy path pushes raw.
    /// On success the mirror records the command's apply at `cursor`
    /// (the next callback head for `due = 0`).
    fn stage_node_transport(
        attached: &mut AttachedDispatch,
        cursor: u64,
        window_frames: u32,
        command: TransportCommand,
    ) -> Result<(), BoundarySessionError> {
        match attached.boundary.as_ref() {
            Some(session) => {
                session.stage_window(
                    attached.dispatch.control(),
                    &session.provenance(),
                    window_frames,
                    Vec::new(),
                    &[(command, 0)],
                )?;
            }
            None => {
                attached
                    .dispatch
                    .control()
                    .request(command, 0)
                    .map_err(|_| BoundarySessionError::QueueFull)?;
            }
        }
        match command {
            TransportCommand::Play => attached.transport.note_play(cursor),
            TransportCommand::Stop => attached.transport.note_stop(cursor),
            TransportCommand::SetLoop(range) => attached
                .transport
                .note_set_loop(cursor, (range.start().value(), range.end().value())),
            TransportCommand::ClearLoop => attached.transport.note_clear_loop(cursor),
            // `Seek` carries its position — staged through
            // `anchor_link`/the seek path, never this helper.
            // `Pause` is never commanded by the driver.
            _ => {}
        }
        Ok(())
    }

    /// Runs `blocks` blocks of `engine.block_frames`, collecting the
    /// master output. Pumping is policy-driven — `Every(0)` produces
    /// a deterministic underrun without touching the plan.
    ///
    /// # Errors
    /// [`ProcessError`] from the runtime (e.g. over-budget frames).
    pub fn run(&mut self, blocks: usize, policy: PumpPolicy) -> Result<PlayReport, ProcessError> {
        self.state = DriverState::Playing;
        let channels = usize::from(self.engine.channels);
        let block = self.engine.block_frames;
        let mut pcm: Vec<Vec<f32>> = vec![Vec::with_capacity(blocks * block); channels];
        for index in 0..blocks as u64 {
            let PumpPolicy::Every(n) = policy;
            if n > 0 && index % n == 0 {
                for producer in &mut self.producers {
                    // One `pump` emits at most one item — drive until the
                    // producer idles so markers and decoded blocks both
                    // reach the ring (bounded for determinism).
                    for _ in 0..64 {
                        if !matches!(producer.pump(), Ok(PumpOutcome::Pushed)) {
                            break;
                        }
                    }
                }
            }
            self.clock.store(self.cursor, Ordering::Relaxed);
            self.rt.process(block, &self.ctx)?;
            self.observe_attached_block();
            if let Some(port) = self.master_in
                && let Some(slot) = self.rt.output_reader(port)
            {
                for (c, channel_pcm) in pcm.iter_mut().enumerate() {
                    if let Some(channel) = slot.data.channel(c) {
                        channel_pcm.extend_from_slice(&channel[..block]);
                    }
                }
            }
            self.cursor += block as u64;
        }
        let mut crc = 0_u64;
        for channel in &pcm {
            for sample in channel {
                crc = crc
                    .wrapping_mul(31)
                    .wrapping_add(u64::from(sample.to_bits()));
            }
        }
        Ok(PlayReport {
            pcm,
            crc,
            frames: blocks as u64 * block as u64,
        })
    }

    /// Projects the block that just processed onto every attached
    /// node's ledger. Call after each `rt.process` while
    /// `self.cursor` still holds the block's start frame.
    ///
    /// The span context is no longer fabricated: each node's
    /// [`TransportMirror`] projects the transport state the driver
    /// commanded (status/position/loop/occurrence — the same
    /// `apply`/`do_wrap` rules the scheduler runs), generation axes
    /// come from the node's own drained [`AppliedReceipt`]s
    /// (`transport_generation` receipt-confirmed with the pending
    /// prediction while staged bumps await applies;
    /// `plan_generation` from the dispatch's staged plan — the same
    /// value the projection gate checks), `clock_epoch` is the
    /// driver's owned epoch and `tempo_revision` is `0` by
    /// construction (no plan this driver installs carries a tempo
    /// map). `position == engine frame` remains the driver's
    /// invariant; the mirror makes the *wrapped* position and loop
    /// metadata honest where a `SetLoop` is active.
    fn observe_attached_block(&mut self) {
        let call_start = EngineFrame::new(self.epoch, self.cursor);
        let block = self.engine.block_frames;
        let call_frames = u32::try_from(block).unwrap_or(u32::MAX);
        for attached in &mut self.attached {
            let span = TransportSpan {
                start: call_start,
                end: call_start.advance(block as u64),
                ctx: attached.transport.span_context(
                    self.cursor,
                    attached.dispatch.frame_base(),
                    self.epoch,
                    attached
                        .dispatch
                        .last_plan_generation()
                        .unwrap_or_else(|| PlanGeneration::new(0)),
                ),
                deliverable: true,
            };
            let cycle = attached
                .dispatch
                .observe_cycle(&span, call_start, call_frames);
            attached.transport.note_receipts(&cycle.receipts);
            attached.last_span = Some(span);
            for (target, obs) in cycle.observations {
                self.param_observations.insert(target, obs);
            }
        }
    }

    /// Re-anchors the transport: engine cursor plus a per-clip source
    /// seek aligned to each window's mapping (clamped to the window —
    /// positions outside it still land at a defined frame).
    ///
    /// Every clip's in-flight ring is drained first (one quiesced
    /// process pass — output discarded) so the seek `Boundary` is the
    /// consumer's next item: deterministic re-anchor with no stale
    /// play-through and no position hole.
    ///
    /// The same quiesce pass runs when attached nodes exist: node
    /// controls compute their due frame against the cursor **at pop
    /// time**, so a dispatch staged before `seek` must drain at the
    /// old cursor or it would install mid-rewind under the old plan.
    /// Each pass is a real processed block for attached nodes —
    /// in-window plan events deliver for real — so it is observed
    /// like any other block. Two passes cover the node's 64-entry
    /// control channel at its 32-message per-callback drain cap;
    /// deeper floods surface as `QueueFull` at push time.
    ///
    /// The per-node re-anchor is typed: boundary-declared nodes stage
    /// through their [`ProcessBoundarySession`] and a refusal returns
    /// [`BoundarySessionError`] (the clip seeks above already
    /// committed — same partial-application contract as before);
    /// undeclared nodes keep the lenient best-effort queueing.
    ///
    /// # Errors
    /// [`BoundarySessionError`] — oracle/provenance refusal or bounded
    /// queue drop on a boundary-declared node.
    pub fn seek(&mut self, engine_frame: u64) -> Result<(), BoundarySessionError> {
        if !self.drain_nonces.is_empty() || !self.attached.is_empty() {
            let passes = usize::from(!self.attached.is_empty()) + 1;
            for _ in 0..passes {
                for nonce in &self.drain_nonces {
                    nonce.fetch_add(1, Ordering::Relaxed);
                }
                self.clock.store(self.cursor, Ordering::Relaxed);
                // A failed pass produced no spans — skip the
                // projection rather than fabricate one (the only
                // current `ProcessError` is unreachable under
                // `max_frames == block_frames`, kept honest anyway).
                if self.rt.process(self.engine.block_frames, &self.ctx).is_ok() {
                    self.observe_attached_block();
                }
            }
        }
        self.cursor = engine_frame;
        for (index, binding) in self.bindings.iter().enumerate() {
            let w = binding.window;
            let target = if engine_frame < w.engine_start {
                w.source_start
            } else if engine_frame >= w.engine_start + w.engine_len {
                w.source_start + w.source_len
            } else {
                w.source_start + (engine_frame - w.engine_start)
            };
            if self.handles[index].seek(SourceFrame(target)).is_ok() {
                self.seek_nonces[index].fetch_add(1, Ordering::Relaxed);
            }
        }
        let window = self.window_frames();
        for attached in &mut self.attached {
            // Re-anchor cursor, then position — `set_clock` is applied
            // first in the drain order, so the `Seek`'s due frame is
            // already the new cursor; `Seek` keeps position == frame
            // even if a prior `Play` had anchored elsewhere.
            Self::stage_seek(attached, self.epoch, engine_frame, window)?;
        }
        Ok(())
    }

    /// Stages the seek re-anchor on one node: `set_clock(epoch,
    /// frame)` then `Seek(position = frame)` — position == engine
    /// frame is the driver's invariant. Sessioned nodes evaluate
    /// through the oracle (`stage_window`); the mirror folds the
    /// seek only after the command actually queued.
    fn stage_seek(
        attached: &mut AttachedDispatch,
        epoch: ClockEpochId,
        frame: u64,
        window_frames: u32,
    ) -> Result<(), BoundarySessionError> {
        let position = i64::try_from(frame).unwrap_or(i64::MAX);
        let seek = TransportCommand::Seek(MediaTime::new(position, attached.dispatch.frame_base()));
        if let Some(session) = attached.boundary.as_ref() {
            attached
                .dispatch
                .control()
                .set_clock(epoch, frame)
                .map_err(|_| BoundarySessionError::QueueFull)?;
            session.stage_window(
                attached.dispatch.control(),
                &session.provenance(),
                window_frames,
                Vec::new(),
                &[(seek, 0)],
            )?;
        } else {
            let _ = attached.dispatch.control().set_clock(epoch, frame);
            // Legacy lenient path: a dropped command stays
            // best-effort (pre-TIM-03 behavior) — the mirror
            // only records what actually queued.
            if attached.dispatch.control().request(seek, 0).is_err() {
                return Ok(());
            }
        }
        attached.transport.note_seek(frame, position);
        Ok(())
    }

    /// Explicit stop — transport state, not a missing plan. Attached
    /// plugin nodes are *not* notified: their schedulers keep their
    /// last commanded state. The MIDI panic path is
    /// [`HeadlessDriver::panic`]; resume after it with
    /// [`HeadlessDriver::resume_attached_transport`].
    pub fn stop(&mut self) {
        self.state = DriverState::Stopped;
    }

    /// Forwards a loop region `[start_frame, end_frame)` — engine
    /// frames — to every attached plugin node as
    /// `TransportCommand::SetLoop` in that node's frame base
    /// (`due = 0`, applied at the next callback head; TIM-02, issue
    /// #126).
    ///
    /// A loop wrap bumps the node's `LoopOccurrence`, which the node
    /// treats as a transport discontinuity: sounding notes are
    /// released and the plan prefix is chased at the wrap position,
    /// so scheduled events inside the loop re-fire once per
    /// occurrence while events outside it fire exactly once.
    /// `start_frame < end_frame` is required — an empty or inverted
    /// range is rejected upstream as an `InvalidCommand` receipt
    /// (observable via node reports), never silently looped.
    ///
    /// # Errors
    /// [`BoundarySessionError`] when a node's bounded control queue
    /// drops the command or a boundary-declared node's oracle refuses
    /// it — nodes earlier in `attached` may already have it.
    pub fn set_loop(
        &mut self,
        start_frame: u64,
        end_frame: u64,
    ) -> Result<(), BoundarySessionError> {
        let cursor = self.cursor;
        let window = self.window_frames();
        for attached in &mut self.attached {
            // The command embeds a `MediaTime` in *this* node's frame
            // base — nodes may disagree, so build per node.
            let base = attached.dispatch.frame_base();
            let start = MediaTime::new(i64::try_from(start_frame).unwrap_or(i64::MAX), base);
            let duration = MediaDuration::new(
                i64::try_from(end_frame.saturating_sub(start_frame)).unwrap_or(i64::MAX),
                base,
            );
            Self::stage_node_transport(
                attached,
                cursor,
                window,
                TransportCommand::SetLoop(MediaRange::new(start, duration)),
            )?;
        }
        Ok(())
    }

    /// Clears the loop region on every attached plugin node
    /// (`TransportCommand::ClearLoop`) — playback then continues past
    /// the former loop end without wrapping.
    ///
    /// # Errors
    /// [`BoundarySessionError`] — see [`HeadlessDriver::set_loop`].
    pub fn clear_loop(&mut self) -> Result<(), BoundarySessionError> {
        self.stage_transport(TransportCommand::ClearLoop)
    }

    /// Panics every attached plugin node: `TransportCommand::Stop` is
    /// the upstream primitive that releases all ledger-tracked notes
    /// at the next callback head *without* retriggering them — the
    /// node's transport stops, so `Stopped` suppresses both chase and
    /// plan events and sounding voices end with zero remainder. The
    /// release lands on the next processed block (`run`, or the
    /// `seek` quiesce pass), not inside this call.
    ///
    /// Resume with [`HeadlessDriver::resume_attached_transport`]: a
    /// bare `Play` would restart the node's position at 0 and diverge
    /// from the engine cursor this driver owns.
    ///
    /// # Errors
    /// [`BoundarySessionError`] — see [`HeadlessDriver::set_loop`].
    pub fn panic(&mut self) -> Result<(), BoundarySessionError> {
        self.stage_transport(TransportCommand::Stop)
    }

    /// Re-anchors every attached node's cursor and position to the
    /// driver cursor and requests `Play` — identical to the
    /// activation-time anchor, so plan events addressed by absolute
    /// engine frame land on the driver's frame series again. This is
    /// the resume path after [`HeadlessDriver::panic`]; the transport
    /// generation bump makes each node release-then-chase once at the
    /// re-anchor point, reconstructing sounding notes from the plan
    /// prefix at the cursor.
    ///
    /// # Errors
    /// [`BoundarySessionError`] — oracle/provenance refusal or bounded
    /// queue drop on a boundary-declared node; undeclared nodes keep
    /// the lenient best-effort queueing.
    pub fn resume_attached_transport(&mut self) -> Result<(), BoundarySessionError> {
        let epoch = self.epoch;
        let cursor = self.cursor;
        let window = self.window_frames();
        for attached in &mut self.attached {
            Self::anchor_link(
                &mut attached.dispatch,
                attached.boundary.as_ref(),
                &mut attached.transport,
                epoch,
                cursor,
                window,
            )?;
        }
        Ok(())
    }

    /// Current transport state.
    #[must_use]
    pub const fn state(&self) -> DriverState {
        self.state
    }

    /// Engine frame the next block starts at.
    #[must_use]
    pub const fn cursor(&self) -> u64 {
        self.cursor
    }

    /// The [`ParamDispatch`] orchestrator of the attached node at
    /// `node_id` — binds, writes, revalidation and per-target stage
    /// queries. `None` when the active plan installed no node there.
    pub fn attached_dispatch(&mut self, node_id: NodeId) -> Option<&mut ParamDispatch> {
        self.attached
            .iter_mut()
            .find(|d| d.dispatch.node() == node_id)
            .map(|d| &mut d.dispatch)
    }

    /// The [`TransportSpan`] last projected onto the attached node at
    /// `node_id` — diagnostic evidence for the mirror-fed span
    /// context (status/position/loop/occurrence/generations as the
    /// node's ledger saw them). `None` before the first observed
    /// block or when no node is attached there.
    #[must_use]
    pub fn attached_span(&self, node_id: NodeId) -> Option<&TransportSpan> {
        self.attached
            .iter()
            .find(|d| d.dispatch.node() == node_id)
            .and_then(|d| d.last_span.as_ref())
    }

    /// The attached node's TIM-03 [`ProcessBoundarySession`] paired
    /// with its [`ParamDispatch`] — `Some` only for boundary-declared
    /// attaches. The pair lets consumers run the session's gated
    /// `dispatch_params`/`dispatch_midi`/`stage_window` through the
    /// same dispatch half the driver stages transport on; transport
    /// commands should keep going through the driver's typed
    /// methods, not raw `control()` pushes.
    ///
    /// `current` provenance for session calls is the session's own
    /// pin — [`ProcessBoundarySession::provenance`] — the driver
    /// binds session and dispatch for the node's whole attached
    /// lifetime, so no drift channel exists.
    pub fn attached_boundary(
        &mut self,
        node_id: NodeId,
    ) -> Option<(&ProcessBoundarySession, &mut ParamDispatch)> {
        let attached = self
            .attached
            .iter_mut()
            .find(|d| d.dispatch.node() == node_id)?;
        Some((attached.boundary.as_ref()?, &mut attached.dispatch))
    }

    /// Latest observation per parameter target, merged across all
    /// attached nodes — refreshed per processed block inside
    /// [`HeadlessDriver::run`] and on the `seek` quiesce pass
    /// (per-call ledger projection plus drained node reports).
    /// Cleared on activation. Per-target rows are last-writer-wins:
    /// a `ParameterTargetId` bound on two attached nodes reports
    /// only the later dispatcher's stage — use
    /// [`HeadlessDriver::attached_dispatch`]'s per-node `stage` for
    /// attribution-critical queries.
    #[must_use]
    pub fn param_observations(&self) -> &BTreeMap<ParameterTargetId, ParamValueObservation> {
        &self.param_observations
    }

    /// Per-clip playback stats — the node's clip identity only; comp
    /// segments and transition sides report through
    /// [`HeadlessDriver::stats_of_content`].
    #[must_use]
    pub fn stats_of(&self, clip_id: ClipId) -> Option<&Arc<PlaybackStats>> {
        self.stats_of_content(&NodeContent::Clip(clip_id))
    }

    /// Per-node playback stats, keyed by the node's content.
    #[must_use]
    pub fn stats_of_content(&self, content: &NodeContent) -> Option<&Arc<PlaybackStats>> {
        self.stats
            .iter()
            .find(|(id, _)| id == content)
            .map(|(_, stats)| stats)
    }

    /// Revision of the last published spec, if any.
    #[must_use]
    pub fn published_revision(&self) -> Option<GraphRevision> {
        self.project.published_revision()
    }

    /// Total latency of the active plan in frames.
    #[must_use]
    pub fn total_latency(&self) -> u32 {
        self.graph.published_plan().total_latency
    }

    /// Runs pending non-RT retire work.
    pub fn collect(&mut self) {
        self.graph.collect();
    }
}
