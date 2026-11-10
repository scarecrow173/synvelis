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
use ruviform_audio_plugin_bridge::{ParamValueObservation, PluginNode, QueueFull};
use ruviform_graph::{GraphProject, GraphRevision, NodeId, NodeSpec, PortId, PrepareError};
use ruviform_media_core::{
    ClockEpochId, EngineFrame, LoopOccurrence, MediaDuration, MediaRange, MediaTime,
    PlanGeneration, SpanContext, TempoMapRevision, TransportCommand, TransportGeneration,
    TransportSpan, TransportStatus,
};
use ruviform_media_reader::{PrefetchHandle, PrefetchProducer, PumpOutcome, SourceFrame};
use ruviform_rt::RtContext;

use crate::dispatch::ParamDispatch;
use crate::lowering::{EngineConfig, Lowered, NodeContent, PlaybackBinding};
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
    /// Attached plugin nodes' non-RT dispatch halves — the RT halves
    /// live inside the published plan. Transport ownership: the
    /// driver requests `Play` at activation and re-anchors on `seek`.
    attached: Vec<ParamDispatch>,
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
            // `slots.remove` above already proved id uniqueness.
            pending.insert(node.dispatch.node(), Box::new(node.node));
            attached_dispatch.push(node.dispatch);
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
        self.anchor_attached_transport();
        Ok(generation)
    }

    /// Re-anchors every attached node's cursor and position to the
    /// driver cursor, then requests `Play`. A fresh node's cursor
    /// starts at frame 0, so without this its frame/position series
    /// would lag the driver's by `self.cursor`. `set_clock` applies
    /// first in drain order, so the `Seek`'s due frame is already
    /// the new cursor and position == frame holds from the node's
    /// first callback; `Play` last so its anchor reads the
    /// re-anchored position. Best-effort: the bounded control queue
    /// drains at every callback head, so a drop here means the
    /// consumer flooded the channel — `param_observations`/reports
    /// still reflect what the node actually did.
    fn anchor_attached_transport(&mut self) {
        for dispatch in &mut self.attached {
            let base = dispatch.frame_base();
            let _ = dispatch.control().set_clock(self.epoch, self.cursor);
            let _ = dispatch.control().request(
                TransportCommand::Seek(MediaTime::new(
                    i64::try_from(self.cursor).unwrap_or(i64::MAX),
                    base,
                )),
                0,
            );
            let _ = dispatch.control().request(TransportCommand::Play, 0);
        }
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
    /// dispatcher's ledger. Call after each `rt.process` while
    /// `self.cursor` still holds the block's start frame — fabricates
    /// the span the node's scheduler ran: the driver keeps attached
    /// nodes always-`Playing` with position == engine frame
    /// (anchored at activation, re-anchored on seek), and due=0
    /// applies land at callback heads — so the one honest span is
    /// exactly the physical call window.
    fn observe_attached_block(&mut self) {
        let call_start = EngineFrame::new(self.epoch, self.cursor);
        let block = self.engine.block_frames;
        let call_frames = u32::try_from(block).unwrap_or(u32::MAX);
        for dispatch in &mut self.attached {
            let span = TransportSpan {
                start: call_start,
                end: call_start.advance(block as u64),
                ctx: SpanContext {
                    status: TransportStatus::Playing,
                    position_ticks: i64::try_from(self.cursor).unwrap_or(i64::MAX),
                    position_base: dispatch.frame_base(),
                    loop_ticks: None,
                    occurrence: LoopOccurrence::new(0),
                    clock_epoch: self.epoch,
                    transport_generation: TransportGeneration::new(0),
                    plan_generation: dispatch
                        .last_plan_generation()
                        .unwrap_or_else(|| PlanGeneration::new(0)),
                    tempo_revision: TempoMapRevision::new(0),
                },
                deliverable: true,
            };
            for (target, obs) in dispatch.observe(&span, call_start, call_frames) {
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
    pub fn seek(&mut self, engine_frame: u64) {
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
        for dispatch in &mut self.attached {
            // Re-anchor cursor, then position — `set_clock` is applied
            // first in the drain order, so the `Seek`'s due frame is
            // already the new cursor; `Seek` keeps position == frame
            // even if a prior `Play` had anchored elsewhere. Best-effort
            // queueing as at activation.
            let base = dispatch.frame_base();
            let _ = dispatch.control().set_clock(self.epoch, engine_frame);
            let _ = dispatch.control().request(
                TransportCommand::Seek(MediaTime::new(
                    i64::try_from(engine_frame).unwrap_or(i64::MAX),
                    base,
                )),
                0,
            );
        }
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
    /// [`QueueFull`] when a node's bounded control queue drops the
    /// command — nodes earlier in `attached` may already have it.
    pub fn set_loop(&mut self, start_frame: u64, end_frame: u64) -> Result<(), QueueFull> {
        for dispatch in &mut self.attached {
            let base = dispatch.frame_base();
            let start = MediaTime::new(i64::try_from(start_frame).unwrap_or(i64::MAX), base);
            let duration = MediaDuration::new(
                i64::try_from(end_frame.saturating_sub(start_frame)).unwrap_or(i64::MAX),
                base,
            );
            dispatch.control().request(
                TransportCommand::SetLoop(MediaRange::new(start, duration)),
                0,
            )?;
        }
        Ok(())
    }

    /// Clears the loop region on every attached plugin node
    /// (`TransportCommand::ClearLoop`) — playback then continues past
    /// the former loop end without wrapping.
    ///
    /// # Errors
    /// [`QueueFull`] — see [`HeadlessDriver::set_loop`].
    pub fn clear_loop(&mut self) -> Result<(), QueueFull> {
        for dispatch in &mut self.attached {
            dispatch.control().request(TransportCommand::ClearLoop, 0)?;
        }
        Ok(())
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
    /// [`QueueFull`] — see [`HeadlessDriver::set_loop`].
    pub fn panic(&mut self) -> Result<(), QueueFull> {
        for dispatch in &mut self.attached {
            dispatch.control().request(TransportCommand::Stop, 0)?;
        }
        Ok(())
    }

    /// Re-anchors every attached node's cursor and position to the
    /// driver cursor and requests `Play` — identical to the
    /// activation-time anchor, so plan events addressed by absolute
    /// engine frame land on the driver's frame series again. This is
    /// the resume path after [`HeadlessDriver::panic`]; the transport
    /// generation bump makes each node release-then-chase once at the
    /// re-anchor point, reconstructing sounding notes from the plan
    /// prefix at the cursor.
    pub fn resume_attached_transport(&mut self) {
        self.anchor_attached_transport();
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
        self.attached.iter_mut().find(|d| d.node() == node_id)
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
