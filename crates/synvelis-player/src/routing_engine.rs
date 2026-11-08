//! SF-03 slice 1 — `RoutingEngine`: the semantic-routing activation
//! owner. It composes the same Ruviform primitives [`HeadlessDriver`]
//! uses — `GraphProject` validation, `Compiler`, `GraphRuntime`
//! publish and `RtRuntime` processing — but keyed by
//! [`RoutingBuildKey`] so prepare/publish/activate evidence stays
//! honest:
//!
//! - `Configured → Validated → Prepared → Published → Active` are
//!   distinct receipt states; `Published` is non-RT evidence only —
//!   `Active` requires the RT side to have observed the generation.
//! - Any prepare/compile/publish failure leaves the previously
//!   published plan live (last-good) and records a `Failed` receipt.
//! - `publish` re-validates the live project against the prepared
//!   key: a moved revision is `StaleProjectRevision`, semantic drift
//!   is `StaleCapability` — a stale plan can never displace a newer
//!   committed snapshot.
//! - Generation-tagged stale events/commands are rejected by
//!   `RtRuntime` itself and counted on `SharedStatus`; the engine
//!   surfaces that counter, it does not reinterpret it.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ruviform_audio::typed::{
    CompileError, CompiledPlan, Compiler, CompilerConfig, FactoryError, GraphProcessor,
    GraphRuntime, InstalledInfo, PrepareOutcome, ProcessError, RtRuntime, SharedStatusSnapshot,
};
use ruviform_audio_core::AudioFormat;
use ruviform_graph::{GraphProject, GraphSpec, NodeId, NodeSpec, PortId};
use ruviform_media_reader::{PrefetchHandle, PrefetchProducer, PumpOutcome, SourceFrame};
use ruviform_rt::RtContext;

use synvelis_project::{
    LOWERING_POLICY_VERSION, ProcessorId, Project, ProjectRevision, RoutingActivationReceipt,
    RoutingActivationState, RoutingBuildKey, RoutingBuildRequestId, RoutingCompileRequest,
    RoutingFault, RoutingSourceMap, RuntimeClockEpochRef, RuntimeInstanceId,
    port_capability_generation, routing_fingerprint,
};

use crate::driver::{DriverState, PlayReport, PumpPolicy};
use crate::lowering::{EngineConfig, NodeContent, PlaybackBinding};
use crate::processor::{ClipPlayer, ClipSync, PlaybackStats};
use crate::resolver::PreparedClip;
use crate::routing_lower::{
    GainProcessor, SemanticLowered, SemanticNodeRole, StripProcessor, lower_semantic,
};

/// External inputs one build consumes — prepared clips in
/// `SemanticLowered::playbacks` order, processor instances keyed by
/// semantic `ProcessorId`, and the generation counters the build key
/// pins (engine geometry / canonical source format revisions managed
/// by the caller).
pub struct RoutingDeps<'a> {
    /// Canonical-format resolver shared with the legacy lowering.
    pub canonical_of: &'a dyn Fn(synvelis_project::SourceId) -> Option<AudioFormat>,
    /// One [`PreparedClip`] per lowered playback, same order.
    pub clips: Vec<PreparedClip>,
    /// Chain-processor instances by semantic ID. A chain member with
    /// no entry fails the compile closed (`PluginUnavailable`).
    pub processors: BTreeMap<ProcessorId, Box<dyn GraphProcessor>>,
    /// Engine geometry generation (rate/channels/block revision).
    pub engine_config_generation: u64,
    /// Canonical source format generation.
    pub source_format_generation: u64,
}

/// A fully prepared routing build: compiled plan plus the staged
/// playback runtime state committed only on successful `publish`.
pub struct PreparedRouting {
    /// Build identity this plan was compiled under.
    pub key: RoutingBuildKey,
    /// Caller correlation id.
    pub request_id: RoutingBuildRequestId,
    /// The lowered spec (evidence/debug; the compiled plan is
    /// authoritative).
    pub spec: GraphSpec,
    /// Generation-scoped semantic→runtime map.
    pub source_map: RoutingSourceMap,
    /// Compiled plan awaiting publish.
    pub compiled: CompiledPlan,
    /// Device sink input port — the graph output tap.
    pub device_in: PortId,
    /// Playback bindings in plan order.
    pub playbacks: Vec<PlaybackBinding>,
    /// Generation assigned at compile time.
    pub generation: u64,
    /// Staged prefetch producers (commit on publish).
    producers: Vec<PrefetchProducer>,
    /// Staged prefetch seek handles.
    handles: Vec<PrefetchHandle>,
    /// Staged per-clip seek nonces.
    seek_nonces: Vec<Arc<AtomicU64>>,
    /// Staged per-clip drain nonces.
    drain_nonces: Vec<Arc<AtomicU64>>,
    /// Staged per-content playback stats.
    stats: Vec<(NodeContent, Arc<PlaybackStats>)>,
}

/// Playback/processor state staged by `prepare` — committed to the
/// engine's live fields only on successful `publish`.
struct StagedBuild {
    /// node → processor for the compile factory.
    pending: BTreeMap<NodeId, Box<dyn GraphProcessor>>,
    producers: Vec<PrefetchProducer>,
    handles: Vec<PrefetchHandle>,
    seek_nonces: Vec<Arc<AtomicU64>>,
    drain_nonces: Vec<Arc<AtomicU64>>,
    stats: Vec<(NodeContent, Arc<PlaybackStats>)>,
}

/// A published plan awaiting RT-observed activation.
struct PendingActivation {
    key: RoutingBuildKey,
    request_id: RoutingBuildRequestId,
    generation: u64,
    project_revision: ProjectRevision,
    /// Frames processed since publish — activation requires > 0.
    frames: u64,
}

/// Activation evidence ledger — the receipt is the honest answer to
/// "how far did the latest build get?".
#[derive(Default)]
struct Ledger {
    receipt: Option<RoutingActivationReceipt>,
    pending: Option<PendingActivation>,
    last_good_generation: Option<u64>,
    planned_generation: Option<u64>,
    published_generation: Option<u64>,
}

/// Deterministic semantic-routing playback/activation engine.
pub struct RoutingEngine {
    engine: EngineConfig,
    project_rt: GraphProject,
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
    device_in: Option<PortId>,
    /// Latest published plan's source map (replaced wholesale per
    /// publish — runtime IDs are generation-scoped, never persisted).
    source_map: Option<RoutingSourceMap>,
    /// Next generation to assign — starts at 1 like the driver, and
    /// is *reserved* at `prepare` so two outstanding prepares can
    /// never share a generation.
    generation: u64,
    instance: RuntimeInstanceId,
    epoch: RuntimeClockEpochRef,
    ledger: Ledger,
}

impl RoutingEngine {
    /// Creates an engine over `engine` geometry bound to one runtime
    /// instance and clock epoch. Requests carrying a different
    /// `runtime_instance`/`device_clock_epoch` are rejected at
    /// `prepare` — a plan can never target the wrong runtime.
    #[must_use]
    pub fn new(
        engine: EngineConfig,
        instance: RuntimeInstanceId,
        epoch: RuntimeClockEpochRef,
    ) -> Self {
        let config = CompilerConfig {
            max_frames: engine.block_frames,
            ..CompilerConfig::default()
        };
        let (graph, rt) = GraphRuntime::new(&config, 64, 64);
        Self {
            engine,
            project_rt: GraphProject::new(),
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
            device_in: None,
            source_map: None,
            generation: 1,
            instance,
            epoch,
            ledger: Ledger::default(),
        }
    }

    /// Lowers, validates and compiles `project`'s semantic routing
    /// into a [`PreparedRouting`] — nothing is published and no RT
    /// state changes. The returned plan must go through
    /// [`Self::publish`] against the same committed revision.
    ///
    /// `revision` is the caller-observed committed revision; it must
    /// equal `request.expected_project_revision` — a stale candidate
    /// (e.g. a gesture preview built before commit) is rejected
    /// before any graph work.
    ///
    /// # Errors
    /// [`RoutingFault`] — recorded on the activation receipt; the
    /// previously published plan is untouched.
    pub fn prepare(
        &mut self,
        request: &RoutingCompileRequest,
        project: &Project,
        revision: ProjectRevision,
        deps: RoutingDeps<'_>,
    ) -> Result<PreparedRouting, RoutingFault> {
        let key = self.checked_key(request, project, revision, &deps)?;

        // Reserve the generation up front — a failed prepare leaves a
        // gap, never a duplicate.
        let generation = self.generation;
        self.generation += 1;

        let lowered = lower_semantic(
            project,
            deps.canonical_of,
            &self.engine,
            &key,
            generation,
            &request.resource_budget,
        )
        .map_err(|e| self.fail(request, &key, e.fault()))?;

        if deps.clips.len() != lowered.playbacks.len() {
            return Err(self.fail(request, &key, RoutingFault::ValidationFailed));
        }

        let staged = self.stage_build(request, &key, &lowered, deps)?;
        let StagedBuild {
            mut pending,
            producers,
            handles,
            seek_nonces,
            drain_nonces,
            stats,
        } = staged;

        self.project_rt
            .prepare(lowered.spec.clone())
            .map_err(|_| self.fail(request, &key, RoutingFault::ValidationFailed))?;
        let Some(published) = self.project_rt.published() else {
            return Err(self.fail(request, &key, RoutingFault::CompileFailed));
        };
        let validated = published.graph();
        let mut factory = |node: &NodeSpec,
                           _existing: Option<&InstalledInfo>|
         -> Result<PrepareOutcome, FactoryError> {
            pending
                .remove(&node.id())
                .map_or(Ok(PrepareOutcome::Boundary), |proc| {
                    Ok(PrepareOutcome::New(proc))
                })
        };
        let compiled = self
            .compiler
            .compile(
                validated,
                &mut factory,
                self.graph.registry(),
                self.graph.allocator(),
                generation,
            )
            .map_err(|e| {
                self.fail(
                    request,
                    &key,
                    match e {
                        CompileError::MissingProcessor { .. } => RoutingFault::PluginUnavailable,
                        _ => RoutingFault::CompileFailed,
                    },
                )
            })?;

        let SemanticLowered {
            source_map,
            playbacks,
            device_in,
            skipped: _,
            spec,
            node_roles: _,
        } = lowered;
        self.ledger.planned_generation = Some(generation);
        self.record_state(
            Some(request.request_id),
            Some(key.clone()),
            RoutingActivationState::Prepared {
                project_revision: revision,
                planned_generation: generation,
            },
        );
        Ok(PreparedRouting {
            key,
            request_id: request.request_id,
            spec,
            source_map,
            compiled,
            device_in,
            playbacks,
            generation,
            producers,
            handles,
            seek_nonces,
            drain_nonces,
            stats,
        })
    }

    /// Builds the [`RoutingBuildKey`] for `project@revision` and
    /// rejects stale/misdirected requests before any graph work:
    /// expected-revision drift is `StaleProjectRevision`, a foreign
    /// runtime instance/epoch or an under-budgeted request is
    /// `ValidationFailed`.
    fn checked_key(
        &mut self,
        request: &RoutingCompileRequest,
        project: &Project,
        revision: ProjectRevision,
        deps: &RoutingDeps<'_>,
    ) -> Result<RoutingBuildKey, RoutingFault> {
        let key = RoutingBuildKey {
            project_id: project.project_id,
            project_revision: revision,
            routing_fingerprint: routing_fingerprint(project),
            capability_generation: port_capability_generation(project),
            engine_config_generation: deps.engine_config_generation,
            source_format_generation: deps.source_format_generation,
            lowering_policy_version: LOWERING_POLICY_VERSION,
            build_mode: request.mode,
            device_clock_epoch: request.device_clock_epoch,
            runtime_instance: request.runtime_instance,
        };
        if request.expected_project_revision != revision {
            return Err(self.fail(request, &key, RoutingFault::StaleProjectRevision));
        }
        if request.runtime_instance != self.instance || request.device_clock_epoch != self.epoch {
            return Err(self.fail(request, &key, RoutingFault::ValidationFailed));
        }
        if self.engine.block_frames > request.resource_budget.max_frames {
            return Err(self.fail(request, &key, RoutingFault::ValidationFailed));
        }
        Ok(key)
    }

    /// Stages clip players, role-driven stages and supplied processor
    /// instances for the compile factory. Chain members without an
    /// instance stay absent so the factory reports them honestly
    /// (`PluginUnavailable`) — never a silent bypass.
    fn stage_build(
        &mut self,
        request: &RoutingCompileRequest,
        key: &RoutingBuildKey,
        lowered: &SemanticLowered,
        deps: RoutingDeps<'_>,
    ) -> Result<StagedBuild, RoutingFault> {
        let mut pending: BTreeMap<NodeId, Box<dyn GraphProcessor>> = BTreeMap::new();
        let mut producers = Vec::with_capacity(lowered.playbacks.len());
        let mut handles = Vec::with_capacity(lowered.playbacks.len());
        let mut seek_nonces = Vec::with_capacity(lowered.playbacks.len());
        let mut drain_nonces = Vec::with_capacity(lowered.playbacks.len());
        let mut stats = Vec::with_capacity(lowered.playbacks.len());
        let mut clips = deps.clips.into_iter();
        for binding in &lowered.playbacks {
            let Some(mut clip) = clips.next() else {
                return Err(self.fail(request, key, RoutingFault::ValidationFailed));
            };
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
            seek_nonces.push(nonce);
            drain_nonces.push(drain);
        }

        let mut processors = deps.processors;
        for (node, role) in &lowered.node_roles {
            match role {
                SemanticNodeRole::Playback(_) => {}
                SemanticNodeRole::StripFader { gains, .. } => {
                    pending.insert(*node, Box::new(StripProcessor::new(*gains)));
                }
                SemanticNodeRole::SendGain { gain, .. } => {
                    pending.insert(*node, Box::new(GainProcessor::new(*gain)));
                }
                SemanticNodeRole::ChainProcessor { processor } => {
                    if let Some(instance) = processors.remove(processor) {
                        pending.insert(*node, instance);
                    }
                }
            }
        }
        Ok(StagedBuild {
            pending,
            producers,
            handles,
            seek_nonces,
            drain_nonces,
            stats,
        })
    }

    /// Publishes a [`PreparedRouting`] after re-validating it against
    /// the live project: the committed revision must still equal the
    /// prepared key's, and the semantic fingerprint/capability
    /// generation must not have drifted. On success the staged
    /// playback state commits and the receipt moves to `Published` —
    /// non-RT evidence only; `Active` still requires RT observation.
    ///
    /// # Errors
    /// [`RoutingFault::StaleProjectRevision`] / [`RoutingFault::StaleCapability`]
    /// when the live snapshot moved, [`RoutingFault::PublishChannelFull`]
    /// when the runtime command channel cannot hold the plan delta.
    /// Every failure leaves the last-good plan live.
    pub fn publish(
        &mut self,
        prepared: PreparedRouting,
        project: &Project,
        revision: ProjectRevision,
    ) -> Result<RoutingActivationReceipt, RoutingFault> {
        let request_id = prepared.request_id;
        let key = prepared.key.clone();
        if key.project_revision != revision || project.project_id != key.project_id {
            return Err(self.fail_receipt(
                Some(request_id),
                &key,
                RoutingActivationState::Blocked {
                    fault: RoutingFault::StaleProjectRevision,
                    last_good_generation: self.ledger.last_good_generation,
                },
                RoutingFault::StaleProjectRevision,
            ));
        }
        if routing_fingerprint(project) != key.routing_fingerprint
            || port_capability_generation(project) != key.capability_generation
        {
            return Err(self.fail_receipt(
                Some(request_id),
                &key,
                RoutingActivationState::Blocked {
                    fault: RoutingFault::StaleCapability,
                    last_good_generation: self.ledger.last_good_generation,
                },
                RoutingFault::StaleCapability,
            ));
        }

        let generation = prepared.compiled.plan.generation;
        match self.graph.publish(prepared.compiled) {
            Ok(()) => {
                self.bindings = prepared.playbacks;
                self.stats = prepared.stats;
                self.producers = prepared.producers;
                self.handles = prepared.handles;
                self.seek_nonces = prepared.seek_nonces;
                self.drain_nonces = prepared.drain_nonces;
                self.device_in = Some(prepared.device_in);
                self.source_map = Some(prepared.source_map);
                self.ledger.published_generation = Some(generation);
                self.ledger.pending = Some(PendingActivation {
                    key: key.clone(),
                    request_id,
                    generation,
                    project_revision: revision,
                    frames: 0,
                });
                Ok(self.record_state(
                    Some(request_id),
                    Some(key),
                    RoutingActivationState::Published {
                        project_revision: revision,
                        generation,
                    },
                ))
            }
            Err(_) => Err(self.fail_receipt(
                Some(request_id),
                &key,
                RoutingActivationState::Failed {
                    fault: RoutingFault::PublishChannelFull,
                    last_good_generation: self.ledger.last_good_generation,
                },
                RoutingFault::PublishChannelFull,
            )),
        }
    }

    /// The latest activation receipt — `None` before the first
    /// `prepare`.
    #[must_use]
    pub fn receipt(&self) -> Option<&RoutingActivationReceipt> {
        self.ledger.receipt.as_ref()
    }

    /// Point-in-time RT status counters (stale events/commands,
    /// uninit reads, fault flags, observed active generation).
    #[must_use]
    pub fn rt_status(&self) -> SharedStatusSnapshot {
        self.graph.status().snapshot()
    }

    /// The latest published plan's source map — `None` before the
    /// first successful publish. Runtime IDs here are scoped to the
    /// published generation; never persist them.
    #[must_use]
    pub fn source_map(&self) -> Option<&RoutingSourceMap> {
        self.source_map.as_ref()
    }

    /// Runs `blocks` blocks of `engine.block_frames`, collecting the
    /// master output — the same pump/process/capture loop as
    /// [`HeadlessDriver::run`], plus activation observation: after
    /// each processed block the RT-reported `active_generation` is
    /// compared against the pending publish.
    ///
    /// # Errors
    /// [`ProcessError`] from the runtime.
    pub fn run(&mut self, blocks: usize, policy: PumpPolicy) -> Result<PlayReport, ProcessError> {
        self.state = DriverState::Playing;
        let channels = usize::from(self.engine.channels);
        let block = self.engine.block_frames;
        let mut pcm: Vec<Vec<f32>> = vec![Vec::with_capacity(blocks * block); channels];
        for index in 0..blocks as u64 {
            let PumpPolicy::Every(n) = policy;
            if n > 0 && index % n == 0 {
                for producer in &mut self.producers {
                    for _ in 0..64 {
                        if !matches!(producer.pump(), Ok(PumpOutcome::Pushed)) {
                            break;
                        }
                    }
                }
            }
            self.clock.store(self.cursor, Ordering::Relaxed);
            self.rt.process(block, &self.ctx)?;
            self.observe_activation(block as u64);
            if let Some(port) = self.device_in
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

    /// Re-anchors transport after a seek — one quiesced process pass
    /// drains in-flight rings, then every clip handle seeks to the
    /// frame its window maps `engine_frame` onto (clamped to the
    /// window, same convention as the driver).
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

    /// Per-content playback stats of the active plan.
    #[must_use]
    pub fn stats(&self) -> &[(NodeContent, Arc<PlaybackStats>)] {
        &self.stats
    }

    /// Compares the RT-observed active generation against the pending
    /// publish and, on agreement after real frames, transitions the
    /// receipt to `Active` and records last-good.
    fn observe_activation(&mut self, frames: u64) {
        let Some(pending) = self.ledger.pending.as_mut() else {
            return;
        };
        pending.frames += frames;
        let observed = self.graph.status().snapshot().active_generation;
        if observed != pending.generation || pending.frames == 0 {
            return;
        }
        let (key, request_id, generation, revision) = (
            pending.key.clone(),
            pending.request_id,
            pending.generation,
            pending.project_revision,
        );
        self.ledger.pending = None;
        self.ledger.last_good_generation = Some(generation);
        self.record_state(
            Some(request_id),
            Some(key),
            RoutingActivationState::Active {
                project_revision: revision,
                generation,
                epoch: self.epoch,
            },
        );
    }

    /// Records a `Failed` receipt for a `prepare`-stage fault and
    /// returns the fault.
    fn fail(
        &mut self,
        request: &RoutingCompileRequest,
        key: &RoutingBuildKey,
        fault: RoutingFault,
    ) -> RoutingFault {
        self.record_state(
            Some(request.request_id),
            Some(key.clone()),
            RoutingActivationState::Failed {
                fault,
                last_good_generation: self.ledger.last_good_generation,
            },
        );
        fault
    }

    /// Records a `Blocked`/`Failed` receipt for a `publish`-stage
    /// fault and returns the fault.
    fn fail_receipt(
        &mut self,
        request_id: Option<RoutingBuildRequestId>,
        key: &RoutingBuildKey,
        state: RoutingActivationState,
        fault: RoutingFault,
    ) -> RoutingFault {
        self.record_state(request_id, Some(key.clone()), state);
        fault
    }

    /// Writes the current receipt, keeping shared fields (planned/
    /// published/observed generations, frames) coherent.
    fn record_state(
        &mut self,
        request_id: Option<RoutingBuildRequestId>,
        build_key: Option<RoutingBuildKey>,
        state: RoutingActivationState,
    ) -> RoutingActivationReceipt {
        let fault = match &state {
            RoutingActivationState::Blocked { fault, .. }
            | RoutingActivationState::Failed { fault, .. } => Some(*fault),
            _ => None,
        };
        let receipt = RoutingActivationReceipt {
            request_id,
            build_key,
            state,
            planned_generation: self.ledger.planned_generation,
            published_generation: self.ledger.published_generation,
            observed_active_generation: Some(self.graph.status().snapshot().active_generation),
            last_good_generation: self.ledger.last_good_generation,
            frames_observed: self.ledger.pending.as_ref().map_or(0, |p| p.frames),
            fault,
        };
        self.ledger.receipt = Some(receipt.clone());
        receipt
    }
}
