//! `HeadlessDriver` — the deterministic non-RT playback owner: graph
//! publish with last-good semantics, manually pumped prefetch
//! producers, per-block `RtRuntime` processing and master-output
//! capture. Seek/stop are explicit; underrun/EOF live in per-clip
//! [`PlaybackStats`], never folded into "successful audio".

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use synvelis_project::ClipId;

use ruviform_audio::typed::{
    ActivationError, CompileError, Compiler, CompilerConfig, FactoryError, GraphRuntime,
    InstalledInfo, PrepareOutcome, ProcessError, RtRuntime,
};
use ruviform_graph::{GraphProject, GraphRevision, NodeSpec, PortId, PrepareError};
use ruviform_media_reader::{PrefetchHandle, PrefetchProducer, PumpOutcome, SourceFrame};
use ruviform_rt::RtContext;

use crate::lowering::{ClipBinding, EngineConfig, Lowered};
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
}

impl fmt::Display for ActivateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(e) => write!(f, "graph validation failed: {e}"),
            Self::Compile(e) => write!(f, "graph compile failed: {e}"),
            Self::Publish(e) => write!(f, "graph publish failed: {e}"),
        }
    }
}

impl std::error::Error for ActivateError {}

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
    bindings: Vec<ClipBinding>,
    stats: Vec<(ClipId, Arc<PlaybackStats>)>,
    master_in: Option<PortId>,
    generation: u64,
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
        }
    }

    /// Publishes `lowered` and installs one [`ClipPlayer`] per binding.
    ///
    /// `prepared` must carry one [`PreparedClip`] per `lowered.clips`
    /// entry, in the same order — each clip gets its own reader so a
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
        let mut pending: BTreeMap<_, ClipPlayer> = BTreeMap::new();
        let mut stats = Vec::with_capacity(lowered.clips.len());
        let mut producers = Vec::with_capacity(lowered.clips.len());
        let mut handles = Vec::with_capacity(lowered.clips.len());
        let mut nonces = Vec::with_capacity(lowered.clips.len());
        let mut drains = Vec::with_capacity(lowered.clips.len());
        for (binding, mut clip) in lowered.clips.iter().zip(prepared) {
            let stat = Arc::new(PlaybackStats::default());
            let nonce = Arc::new(AtomicU64::new(0));
            let drain = Arc::new(AtomicU64::new(0));
            pending.insert(
                binding.node_id,
                ClipPlayer::new(
                    clip.consumer,
                    binding.window,
                    ClipSync {
                        clock: Arc::clone(&self.clock),
                        pending_seek: Arc::clone(&nonce),
                        drain: Arc::clone(&drain),
                    },
                    Arc::clone(&stat),
                    self.engine.block_frames,
                    clip.canonical.layout.channels() as usize,
                ),
            );
            stats.push((binding.clip_id, stat));
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
                .map_or(Ok(PrepareOutcome::Boundary), |player| {
                    Ok(PrepareOutcome::New(Box::new(player)))
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
        self.bindings.clone_from(&lowered.clips);
        self.stats = stats;
        self.producers = producers;
        self.handles = handles;
        self.seek_nonces = nonces;
        self.drain_nonces = drains;
        self.generation += 1;
        self.master_in = Some(lowered.master_in);
        Ok(generation)
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

    /// Re-anchors the transport: engine cursor plus a per-clip source
    /// seek aligned to each window's mapping (clamped to the window —
    /// positions outside it still land at a defined frame).
    ///
    /// Every clip's in-flight ring is drained first (one quiesced
    /// process pass — output discarded) so the seek `Boundary` is the
    /// consumer's next item: deterministic re-anchor with no stale
    /// play-through and no position hole.
    pub fn seek(&mut self, engine_frame: u64) {
        if !self.drain_nonces.is_empty() {
            for nonce in &self.drain_nonces {
                nonce.fetch_add(1, Ordering::Relaxed);
            }
            self.clock.store(self.cursor, Ordering::Relaxed);
            let _ = self.rt.process(self.engine.block_frames, &self.ctx);
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
    }

    /// Explicit stop — transport state, not a missing plan.
    pub fn stop(&mut self) {
        self.state = DriverState::Stopped;
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

    /// Per-clip playback stats.
    #[must_use]
    pub fn stats_of(&self, clip_id: ClipId) -> Option<&Arc<PlaybackStats>> {
        self.stats
            .iter()
            .find(|(id, _)| *id == clip_id)
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
