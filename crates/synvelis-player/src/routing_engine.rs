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
//!   key: a moved revision is `StaleProjectSnapshot`, semantic drift
//!   is `SpeculativeMismatch`, capability drift is
//!   `PluginCapabilityChanged` — a stale plan can never displace a
//!   newer committed snapshot, and an out-of-order generation is
//!   `Superseded`.
//! - `reconcile` re-checks the published plan's build key and source
//!   map against the live committed snapshot and RT counters,
//!   recording every drift aspect as a typed [`RoutingFault`] while
//!   the last-good plan stays live.
//! - Generation-tagged stale events/commands are rejected by
//!   `RtRuntime` itself and counted on `SharedStatus`; the engine
//!   surfaces that counter through `reconcile`, it does not
//!   reinterpret it.
//!
//! [`RoutingFault`] covers the 18 Issue #106 §5 categories — the
//! mint-path table lives in
//! `docs/designs/sf-03-slice2-fault-reconcile-design.md`.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use ruviform_audio::typed::{
    CompileError, CompiledPlan, Compiler, CompilerConfig, FactoryError, GraphProcessor,
    GraphRuntime, InstalledInfo, OfflineRenderSupport, PrepareOutcome, ProcessError, RtRuntime,
    SharedStatusSnapshot,
};
use ruviform_audio_core::AudioFormat;
use ruviform_graph::{
    GraphProject, GraphSpec, NodeId, NodeSpec, PortId, PrepareError, ValidationError,
};
use ruviform_media_core::TimeBase;
use ruviform_media_reader::{PrefetchHandle, PrefetchProducer, PumpOutcome, SourceFrame};
use ruviform_rt::RtContext;

use synvelis_project::BuildMode;
use synvelis_project::{
    ExecutionEquivalence, LOWERING_POLICY_VERSION, ProcessorId, Project, ProjectRevision,
    RoutingActivationReceipt, RoutingActivationState, RoutingBuildKey, RoutingBuildRequestId,
    RoutingCompileRequest, RoutingFault, RoutingReconcileMismatch, RoutingReconcileReport,
    RoutingSourceMap, RuntimeClockEpochRef, RuntimeInstanceId, port_capability_generation,
    routing_fingerprint,
};

use crate::driver::{DriverState, PlayReport, PumpPolicy};
use crate::host::{
    ProcessorConnectRequest, ProcessorConnection, ProcessorControl, ProcessorHost, ProcessorLease,
    StaticProcessorHost,
};
use crate::lowering::{
    CompSkipReason, EngineConfig, NodeContent, PlaybackBinding, SkippedPlayback,
};
use crate::processor::{ClipPlayer, ClipSync, PlaybackStats};
use crate::resolver::PreparedClip;
use crate::routing_lower::{
    GainProcessor, SemanticLowered, SemanticNodeRole, StripProcessor, engine_format, lower_semantic,
};

/// External inputs one build consumes — prepared clips in
/// `SemanticLowered::playbacks` order and the generation counters
/// the build key pins (engine geometry / canonical source format
/// revisions managed by the caller). Chain processors are *not*
/// supplied here: they are provisioned through the engine's
/// [`ProcessorHost`] during `stage_build`.
pub struct RoutingDeps<'a> {
    /// Canonical-format resolver shared with the legacy lowering.
    pub canonical_of: &'a dyn Fn(synvelis_project::SourceId) -> Option<AudioFormat>,
    /// One [`PreparedClip`] per lowered playback, same order.
    pub clips: Vec<PreparedClip>,
    /// Engine geometry generation (rate/channels/block revision).
    pub engine_config_generation: u64,
    /// Canonical source format generation.
    pub source_format_generation: u64,
}

/// One host connection's non-RT halves retained for its generation:
/// the teardown lease plus — for real plugin hosts — the node's
/// [`PluginNodeControl`](ruviform_audio_plugin_bridge::PluginNodeControl)
/// that [`RoutingEngine::take_processor_control`] may vend once.
/// Field order is load-bearing: `_lease` drops before `control`, so
/// host teardown (StopProcessing→Deactivate→Destroy) always runs
/// before the control channel's producer end closes — the same
/// order the lease closure used to enforce internally.
struct RetainedConnection {
    /// Semantic processor this connection serves.
    processor: ProcessorId,
    /// Generation-local graph node the processor fills.
    node: NodeId,
    /// Host teardown token — drop runs the lifecycle teardown once.
    _lease: ProcessorLease,
    /// Non-RT plugin control half, `Some` only for real plugin
    /// hosts; `None` for [`StaticProcessorHost`] connections.
    control: Option<ruviform_audio_plugin_bridge::PluginNodeControl>,
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
    /// Host connections for this build's chain processors —
    /// committed to the engine's generation ledger on `publish`.
    connections: Vec<RetainedConnection>,
}

/// The outcome of [`RoutingEngine::prepare`]: either a freshly
/// compiled [`PreparedRouting`] or proof that the live published
/// plan already covers the configured revision
/// ([`ExecutionEquivalence`]) — the `NoRebuild` path. The two are
/// mutually exclusive by construction: `Equivalent` carries nothing
/// publishable and updates no receipt, generation counter or host
/// connection.
pub enum RoutingPrepare {
    /// A new plan was lowered, host-connected and compiled —
    /// publish it against the same committed revision. Boxed: the
    /// prepared bundle is far larger than the equivalence evidence.
    Built(Box<PreparedRouting>),
    /// The last published plan's build key matches this request on
    /// every field except `project_revision` — no rebuild was
    /// needed. Carries the equivalence evidence; the live plan's
    /// receipt state is unchanged.
    Equivalent(ExecutionEquivalence),
}

impl std::fmt::Debug for RoutingPrepare {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Built(_) => f.write_str("RoutingPrepare::Built(..)"),
            Self::Equivalent(equivalence) => f
                .debug_tuple("RoutingPrepare::Equivalent")
                .field(equivalence)
                .finish(),
        }
    }
}

impl RoutingPrepare {
    /// The prepared plan, or a panic describing the equivalence —
    /// for callers/tests that require a fresh build. Prefer
    /// matching on the enum when `Equivalent` is meaningful.
    ///
    /// # Panics
    /// Panics when the result is [`Self::Equivalent`].
    #[must_use]
    #[track_caller]
    pub fn expect_built(self) -> PreparedRouting {
        match self {
            Self::Built(prepared) => *prepared,
            Self::Equivalent(equivalence) => panic!(
                "prepare returned ExecutionEquivalence (plan generation {} covers revision {})",
                equivalence.plan_generation, equivalence.configured_project_revision.0
            ),
        }
    }
}

/// Playback/processor state staged by `prepare` — committed to the
/// engine's live fields only on successful `publish`.
struct StagedBuild {
    /// node → processor for the compile factory.
    pending: BTreeMap<NodeId, Box<dyn GraphProcessor>>,
    /// Host connections for every host-connected chain processor.
    connections: Vec<RetainedConnection>,
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
    /// Request id of the last successful publish — reused for
    /// reconcile receipts which have no request of their own.
    published_request_id: Option<RoutingBuildRequestId>,
    /// Frames observed at the last activation — kept so the receipt's
    /// `frames_observed` survives the pending slot being cleared.
    observed_frames: u64,
    /// `SharedStatus` counter baselines for `reconcile` drift checks.
    last_stale_events: u64,
    last_stale_commands: u64,
    last_retire_overflows: u64,
    last_plan_retire_overflows: u64,
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
    /// Processor provisioning boundary — connects semantic chain
    /// processors to runtime `GraphProcessor`s plus teardown leases.
    /// Replaced wholesale by [`Self::set_processor_host`]; leases
    /// already minted keep their own host handle.
    host: Box<dyn ProcessorHost>,
    /// Generation → retained host connections for every published
    /// plan. A generation's connections drop only when its plan can
    /// no longer be RT-active: displaced pending generations release
    /// at the next publish, and all non-active generations release
    /// once a newer generation is RT-observed.
    connections: BTreeMap<u64, Vec<RetainedConnection>>,
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
            host: Box::new(StaticProcessorHost::new()),
            connections: BTreeMap::new(),
        }
    }

    /// Replaces the processor host used by *future* `prepare` calls.
    /// Leases already minted keep their own host reference, so
    /// in-flight generations still tear down correctly; only new
    /// connects observe the swap.
    pub fn set_processor_host(&mut self, host: Box<dyn ProcessorHost>) {
        self.host = host;
    }

    /// Lowers, validates and compiles `project`'s semantic routing
    /// into a [`RoutingPrepare::Built`] — nothing is published and no
    /// RT state changes. The returned plan must go through
    /// [`Self::publish`] against the same committed revision.
    ///
    /// **`NoRebuild`:** when the last published plan's
    /// [`RoutingBuildKey`] already matches this request on every
    /// field except `project_revision` (a rename, view-only change
    /// or undo/redo that restored identical graph content), the
    /// compile is skipped entirely — no generation is reserved, no
    /// lowering/host-connect/compile runs and no receipt is
    /// recorded. The result is [`RoutingPrepare::Equivalent`]
    /// carrying [`ExecutionEquivalence`] evidence; the live plan's
    /// `Active`/`Published` state is untouched, so a new revision is
    /// never falsely reported as RT-active. A re-prepare at the
    /// *same* revision always builds — it is the explicit rebuild
    /// path.
    ///
    /// `revision` is the caller-observed committed revision; it must
    /// equal `request.expected_project_revision` — a stale candidate
    /// (e.g. a gesture preview built before commit) is rejected
    /// before any graph work, equivalence included.
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
    ) -> Result<RoutingPrepare, RoutingFault> {
        let key = self.checked_key(request, project, revision, &deps)?;

        // NoRebuild equivalence: runs after `checked_key` so stale
        // candidates, wrong epochs and under-budgeted requests are
        // still rejected, and before generation reservation so an
        // equivalent prepare consumes nothing.
        if let Some(equivalence) = self.equivalence(&key) {
            return Ok(RoutingPrepare::Equivalent(equivalence));
        }

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

        // Skipped playbacks that are not intentional silence fail the
        // build closed — a clip that should play but cannot is never
        // lowered to fake silence. This runs before the clip-count
        // check so the skip's own fault is reported, not masked as a
        // count mismatch.
        if let Some(fault) = skipped_fault(&lowered.skipped) {
            return Err(self.fail(request, &key, fault));
        }

        if deps.clips.len() != lowered.playbacks.len() {
            return Err(self.fail(request, &key, RoutingFault::InvalidRouting));
        }

        let staged = self.stage_build(request, &key, &lowered, project, deps, generation)?;
        if key.build_mode == BuildMode::Offline {
            self.check_offline_capability(request, &key, &lowered, &staged.pending)?;
        }
        let StagedBuild {
            mut pending,
            connections,
            producers,
            handles,
            seek_nonces,
            drain_nonces,
            stats,
        } = staged;

        self.project_rt
            .prepare(lowered.spec.clone())
            .map_err(|e| self.fail(request, &key, graph_prepare_fault(&e)))?;
        let Some(published) = self.project_rt.published() else {
            return Err(self.fail(request, &key, RoutingFault::InvalidRouting));
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
            .map_err(|e| self.fail(request, &key, compile_fault(&e)))?;

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
        Ok(RoutingPrepare::Built(Box::new(PreparedRouting {
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
            connections,
        })))
    }

    /// The `NoRebuild` probe: `Some(ExecutionEquivalence)` when the
    /// last published plan's build key matches `key` on every field
    /// except `project_revision` — and the revisions actually
    /// differ. A same-revision re-prepare is *not* equivalent: it is
    /// the explicit rebuild path. Pending published-but-not-yet-
    /// active plans count as live evidence — their key is what RT
    /// will run once observed.
    fn equivalence(&self, key: &RoutingBuildKey) -> Option<ExecutionEquivalence> {
        let source_map = self.source_map.as_ref()?;
        let live = &source_map.build_key;
        (live.project_revision != key.project_revision && live.semantic_eq(key)).then(|| {
            ExecutionEquivalence {
                configured_project_revision: key.project_revision,
                active_built_from_revision: live.project_revision,
                matching_semantic_build_key: live.clone(),
                plan_generation: source_map.generation,
            }
        })
    }

    /// Builds the [`RoutingBuildKey`] for `project@revision` and
    /// rejects stale/misdirected requests before any graph work:
    /// expected-revision drift is `StaleProjectSnapshot`, a foreign
    /// runtime instance/epoch is `DeviceEpochChanged`, and an
    /// under-budgeted request is `CompileResourceExceeded`.
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
            return Err(self.fail(request, &key, RoutingFault::StaleProjectSnapshot));
        }
        if request.runtime_instance != self.instance || request.device_clock_epoch != self.epoch {
            return Err(self.fail(request, &key, RoutingFault::DeviceEpochChanged));
        }
        if self.engine.block_frames > request.resource_budget.max_frames {
            return Err(self.fail(request, &key, RoutingFault::CompileResourceExceeded));
        }
        Ok(key)
    }

    /// Offline builds require every host-connected chain processor
    /// that actually enters the graph to report
    /// [`OfflineRenderSupport::Verified`] — `Unknown`/`Declared` are
    /// never treated as verified exact offline support. Instances
    /// absent from `pending` are left to the compile factory, which
    /// reports `PluginUnavailable`.
    fn check_offline_capability(
        &mut self,
        request: &RoutingCompileRequest,
        key: &RoutingBuildKey,
        lowered: &SemanticLowered,
        pending: &BTreeMap<NodeId, Box<dyn GraphProcessor>>,
    ) -> Result<(), RoutingFault> {
        for (node, role) in &lowered.node_roles {
            if let SemanticNodeRole::ChainProcessor { .. } = role
                && let Some(instance) = pending.get(node)
                && instance.offline_support() != OfflineRenderSupport::Verified
            {
                return Err(self.fail(request, key, RoutingFault::OfflineCapabilityMismatch));
            }
        }
        Ok(())
    }

    /// Stages clip players, role-driven stages and host-connected
    /// chain processors for the compile factory. Each
    /// [`SemanticNodeRole::ChainProcessor`] goes through
    /// [`ProcessorHost::connect`]; a connect failure fails the whole
    /// build closed (`fault()` mapping) and every lease minted so far
    /// drops — running its teardown — before the error propagates.
    fn stage_build(
        &mut self,
        request: &RoutingCompileRequest,
        key: &RoutingBuildKey,
        lowered: &SemanticLowered,
        project: &Project,
        deps: RoutingDeps<'_>,
        generation: u64,
    ) -> Result<StagedBuild, RoutingFault> {
        let mut pending: BTreeMap<NodeId, Box<dyn GraphProcessor>> = BTreeMap::new();
        let mut connections: Vec<RetainedConnection> = Vec::new();
        let mut producers = Vec::with_capacity(lowered.playbacks.len());
        let mut handles = Vec::with_capacity(lowered.playbacks.len());
        let mut seek_nonces = Vec::with_capacity(lowered.playbacks.len());
        let mut drain_nonces = Vec::with_capacity(lowered.playbacks.len());
        let mut stats = Vec::with_capacity(lowered.playbacks.len());
        let mut clips = deps.clips.into_iter();
        for binding in &lowered.playbacks {
            let Some(mut clip) = clips.next() else {
                return Err(self.fail(request, key, RoutingFault::InvalidRouting));
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

        let port_format = engine_format(&self.engine);
        let max_frames = u32::try_from(self.engine.block_frames)
            .map_err(|_| self.fail(request, key, RoutingFault::InvalidRouting))?;
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
                    let Some(semantic) = project.routing.processors.get(processor) else {
                        return Err(self.fail(request, key, RoutingFault::InvalidRouting));
                    };
                    let mut ports = Vec::with_capacity(semantic.port_ids.len());
                    for port_id in &semantic.port_ids {
                        let Some(port) = project.routing.ports.get(port_id) else {
                            return Err(self.fail(request, key, RoutingFault::InvalidRouting));
                        };
                        ports.push(port.clone());
                    }
                    match self.host.connect(ProcessorConnectRequest {
                        processor: semantic.clone(),
                        ports,
                        mode: request.mode,
                        generation,
                        node: *node,
                        port_format,
                        max_frames,
                    }) {
                        Ok(connection) => Self::retain_connection(
                            &mut pending,
                            &mut connections,
                            semantic.id,
                            *node,
                            connection,
                        ),
                        Err(error) => {
                            return Err(self.fail(request, key, error.fault()));
                        }
                    }
                }
            }
        }
        Ok(StagedBuild {
            pending,
            connections,
            producers,
            handles,
            seek_nonces,
            drain_nonces,
            stats,
        })
    }

    /// Splits a successful host connect into its RT half (the graph
    /// processor) and its generation-retained non-RT halves (lease +
    /// optional plugin control).
    fn retain_connection(
        pending: &mut BTreeMap<NodeId, Box<dyn GraphProcessor>>,
        connections: &mut Vec<RetainedConnection>,
        processor: ProcessorId,
        node: NodeId,
        connection: ProcessorConnection,
    ) {
        pending.insert(node, connection.processor);
        connections.push(RetainedConnection {
            processor,
            node,
            _lease: connection.lease,
            control: connection.control,
        });
    }

    /// Publishes a [`PreparedRouting`] after re-validating it against
    /// the live project: the committed revision must still equal the
    /// prepared key's, the semantic fingerprint must not have drifted
    /// (`SpeculativeMismatch`), the capability generation must not
    /// have changed (`PluginCapabilityChanged`), the prepared bundle
    /// must be internally coherent (`BadSourceMap`), and its
    /// generation must be newer than the last published plan
    /// (`Superseded`). On success the staged playback state commits
    /// and the receipt moves to `Published` — non-RT evidence only;
    /// `Active` still requires RT observation.
    ///
    /// # Errors
    /// [`RoutingFault::StaleProjectSnapshot`] /
    /// [`RoutingFault::SpeculativeMismatch`] /
    /// [`RoutingFault::PluginCapabilityChanged`] /
    /// [`RoutingFault::BadSourceMap`] /
    /// [`RoutingFault::Superseded`] when the live snapshot moved or
    /// the plan is stale, [`RoutingFault::PublishQueueFull`] when the
    /// runtime command channel cannot hold the plan delta. Every
    /// failure leaves the last-good plan live.
    pub fn publish(
        &mut self,
        prepared: PreparedRouting,
        project: &Project,
        revision: ProjectRevision,
    ) -> Result<RoutingActivationReceipt, RoutingFault> {
        let request_id = prepared.request_id;
        let key = prepared.key.clone();
        if key.project_revision != revision || project.project_id != key.project_id {
            return Err(self.blocked(request_id, &key, RoutingFault::StaleProjectSnapshot));
        }
        if routing_fingerprint(project) != key.routing_fingerprint {
            return Err(self.blocked(request_id, &key, RoutingFault::SpeculativeMismatch));
        }
        if port_capability_generation(project) != key.capability_generation {
            return Err(self.blocked(request_id, &key, RoutingFault::PluginCapabilityChanged));
        }

        let generation = prepared.compiled.plan.generation;
        // The prepared bundle is caller-constructed — its source map,
        // build key and generation counters must agree before the
        // plan is allowed near the runtime.
        if prepared.generation != generation
            || prepared.source_map.generation != generation
            || prepared.source_map.build_key != key
            || prepared.source_map.project_id != key.project_id
            || prepared.source_map.project_revision != key.project_revision
        {
            return Err(self.blocked(request_id, &key, RoutingFault::BadSourceMap));
        }
        // Generations are monotonic: a plan that is not newer than the
        // last published one is superseded — it must never displace
        // the newer plan or rewind the generation counter.
        if generation <= self.graph.published_plan().generation {
            return Err(self.blocked(request_id, &key, RoutingFault::Superseded));
        }

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
                self.connections.insert(generation, prepared.connections);
                // The new plan displaces every generation that is
                // neither RT-active nor this one — a displaced
                // pending generation can never activate, so its
                // plugin instances tear down now. The still-active
                // generation's leases stay until RT observes the
                // swap (or a later publish displaces it too).
                let active = self.graph.status().snapshot().active_generation;
                self.connections
                    .retain(|g, _| *g == generation || *g == active);
                self.ledger.published_generation = Some(generation);
                self.ledger.published_request_id = Some(request_id);
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
                    fault: RoutingFault::PublishQueueFull,
                    last_good_generation: self.ledger.last_good_generation,
                },
                RoutingFault::PublishQueueFull,
            )),
        }
    }

    /// Reconciles the last published plan against the live committed
    /// snapshot and the RT status counters. Every drift aspect is
    /// reported in [`RoutingReconcileReport::mismatches`]; when any
    /// exists, the highest-precedence one is recorded on the
    /// activation receipt as `Blocked` while the last-good plan stays
    /// live. A clean reconcile leaves the receipt untouched.
    ///
    /// This is the pre-publish/pre-retry gate: it detects stale CAS,
    /// semantic/capability/format/epoch drift, source-map identity
    /// corruption, RT counter regressions and overdue activations —
    /// so a speculative plan with zero publishes can never slip past
    /// a changed live state.
    ///
    /// Fault precedence (first match wins): `BadSourceMap` →
    /// `StaleProjectSnapshot` → `SpeculativeMismatch` →
    /// `PluginCapabilityChanged` → `UnsupportedFormatOrTimeMap` →
    /// `DeviceEpochChanged` → `RetireOverflow` → `EventStale` →
    /// `ActivationNotObserved`.
    #[must_use]
    pub fn reconcile(
        &mut self,
        project: &Project,
        revision: ProjectRevision,
        engine_config_generation: u64,
        source_format_generation: u64,
    ) -> RoutingReconcileReport {
        let Some(source_map) = self.source_map.clone() else {
            return RoutingReconcileReport {
                plan_key: None,
                mismatches: Vec::new(),
                fault: None,
            };
        };
        let key = source_map.build_key.clone();
        let mut mismatches = Vec::new();

        // Source-map / plan identity coherence.
        if project.project_id != key.project_id
            || source_map.project_id != key.project_id
            || source_map.project_revision != key.project_revision
            || self.ledger.published_generation != Some(source_map.generation)
        {
            mismatches.push(RoutingReconcileMismatch::SourceMapIdentity);
        }
        // Committed-snapshot CAS and semantic drift. A revision-only
        // advance is *not* drift when the live semantic content still
        // matches the published key — the `NoRebuild` path leaves the
        // plan covering the newer revision without a rebuild, and a
        // rename must not surface as `StaleProjectSnapshot`.
        if key.project_revision != revision
            && !self.revision_covered(
                &key,
                project,
                revision,
                engine_config_generation,
                source_format_generation,
            )
        {
            mismatches.push(RoutingReconcileMismatch::ProjectRevision);
        }
        if routing_fingerprint(project) != key.routing_fingerprint {
            mismatches.push(RoutingReconcileMismatch::RoutingFingerprint);
        }
        if port_capability_generation(project) != key.capability_generation {
            mismatches.push(RoutingReconcileMismatch::CapabilityGeneration);
        }
        if engine_config_generation != key.engine_config_generation {
            mismatches.push(RoutingReconcileMismatch::EngineConfigGeneration);
        }
        if source_format_generation != key.source_format_generation {
            mismatches.push(RoutingReconcileMismatch::SourceFormatGeneration);
        }
        if self.epoch != key.device_clock_epoch || self.instance != key.runtime_instance {
            mismatches.push(RoutingReconcileMismatch::DeviceEpoch);
        }

        // RT counters — baselines advance on every reconcile so only
        // new discards/overflows are reported.
        let snap = self.rt_status();
        let retire_advanced = snap.retire_overflows > self.ledger.last_retire_overflows
            || snap.plan_retire_overflows > self.ledger.last_plan_retire_overflows;
        let stale_advanced = snap.stale_events > self.ledger.last_stale_events
            || snap.stale_commands > self.ledger.last_stale_commands;
        if retire_advanced || stale_advanced {
            mismatches.push(RoutingReconcileMismatch::RuntimeCounters);
        }
        self.ledger.last_stale_events = snap.stale_events;
        self.ledger.last_stale_commands = snap.stale_commands;
        self.ledger.last_retire_overflows = snap.retire_overflows;
        self.ledger.last_plan_retire_overflows = snap.plan_retire_overflows;

        // Pending activation: frames were processed but the published
        // generation was never observed — activation is overdue.
        // `frames == 0` is transient (published, not yet run) and is
        // never a fault.
        let pending_overdue = self
            .ledger
            .pending
            .as_ref()
            .is_some_and(|p| p.frames > 0 && snap.active_generation != p.generation);
        if pending_overdue {
            mismatches.push(RoutingReconcileMismatch::PendingActivation);
        }

        // Mismatches are pushed in precedence order — the first one
        // determines the receipt fault.
        let fault = mismatches
            .iter()
            .map(|mismatch| Self::fault_for(*mismatch, retire_advanced))
            .next();
        if let Some(fault) = fault {
            self.record_state(
                self.ledger.published_request_id,
                Some(key),
                RoutingActivationState::Blocked {
                    fault,
                    last_good_generation: self.ledger.last_good_generation,
                },
            );
        }
        RoutingReconcileReport {
            plan_key: Some(source_map.build_key),
            mismatches,
            fault,
        }
    }

    /// The receipt fault a reconcile mismatch maps to — precedence
    /// order is the order mismatches are pushed in `reconcile`.
    fn fault_for(mismatch: RoutingReconcileMismatch, retire_advanced: bool) -> RoutingFault {
        match mismatch {
            RoutingReconcileMismatch::SourceMapIdentity => RoutingFault::BadSourceMap,
            RoutingReconcileMismatch::ProjectRevision => RoutingFault::StaleProjectSnapshot,
            RoutingReconcileMismatch::RoutingFingerprint => RoutingFault::SpeculativeMismatch,
            RoutingReconcileMismatch::CapabilityGeneration => RoutingFault::PluginCapabilityChanged,
            RoutingReconcileMismatch::EngineConfigGeneration
            | RoutingReconcileMismatch::SourceFormatGeneration => {
                RoutingFault::UnsupportedFormatOrTimeMap
            }
            RoutingReconcileMismatch::DeviceEpoch => RoutingFault::DeviceEpochChanged,
            RoutingReconcileMismatch::RuntimeCounters => {
                if retire_advanced {
                    RoutingFault::RetireOverflow
                } else {
                    RoutingFault::EventStale
                }
            }
            RoutingReconcileMismatch::PendingActivation => RoutingFault::ActivationNotObserved,
        }
    }

    /// Whether the committed `project@revision` still describes the
    /// same executable graph as the published `key` — the
    /// reconcile-side `NoRebuild` probe. The live key is rebuilt from
    /// the same inputs `prepare` would use; `build_mode` comes from
    /// `key` because reconcile inputs carry no mode (a mode change
    /// surfaces at the next `prepare` as a key difference, not here).
    fn revision_covered(
        &self,
        key: &RoutingBuildKey,
        project: &Project,
        revision: ProjectRevision,
        engine_config_generation: u64,
        source_format_generation: u64,
    ) -> bool {
        let live_key = RoutingBuildKey {
            project_id: project.project_id,
            project_revision: revision,
            routing_fingerprint: routing_fingerprint(project),
            capability_generation: port_capability_generation(project),
            engine_config_generation,
            source_format_generation,
            lowering_policy_version: LOWERING_POLICY_VERSION,
            build_mode: key.build_mode,
            device_clock_epoch: self.epoch,
            runtime_instance: self.instance,
        };
        key.semantic_eq(&live_key)
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

    /// Takes the non-RT control handle for `processor` of a
    /// **published** `generation` — one-shot: the engine retains the
    /// connection's [`PluginNodeControl`](ruviform_audio_plugin_bridge::PluginNodeControl)
    /// only until it is vended here or the generation retires.
    ///
    /// Returns `None` for an unknown generation or processor, a
    /// connection without a plugin control half
    /// ([`StaticProcessorHost`]), or an already-taken handle. The
    /// vended [`ProcessorControl`] is caller-owned: it keeps working
    /// after the generation retires, but its channel is dedicated to
    /// that generation's plugin instance — staged commands can never
    /// reach another generation's plugin (wrong-plugin 0 by
    /// construction), and pushes against a dead node fail as
    /// `QueueFull` or sit in the bounded buffer undelivered.
    ///
    /// `frame_base` is the attach-time `AttachConfig::frame_base`
    /// (`TimeBase::sample_rate(engine.sample_rate)`) — the exact
    /// value [`crate::ParamDispatch::new`] requires.
    pub fn take_processor_control(
        &mut self,
        generation: u64,
        processor: ProcessorId,
    ) -> Option<ProcessorControl> {
        let frame_base = TimeBase::sample_rate(self.engine.sample_rate).ok()?;
        let connections = self.connections.get_mut(&generation)?;
        let retained = connections
            .iter_mut()
            .find(|connection| connection.processor == processor)?;
        let control = retained.control.take()?;
        Some(ProcessorControl {
            node: retained.node,
            control,
            frame_base,
        })
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
            // The drain pass can activate a pending plan — observe it
            // so activation evidence and lease teardown do not wait
            // for the next `run`.
            self.observe_activation(self.engine.block_frames as u64);
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

    /// Drains deferred processor/plan retirement on the non-RT side —
    /// drops retired `GraphProcessor`s (including detached plugin
    /// endpoints, whose `SendCore` share is what finally releases a
    /// destroyed plugin instance) and retired plan `Arc`s. Call after
    /// generations have been superseded; the channel is bounded, so
    /// skipping this leaks on overflow.
    pub fn collect(&mut self) -> usize {
        self.graph.collect()
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
        let (key, request_id, generation, revision, frames) = (
            pending.key.clone(),
            pending.request_id,
            pending.generation,
            pending.project_revision,
            pending.frames,
        );
        self.ledger.pending = None;
        self.ledger.last_good_generation = Some(generation);
        self.ledger.observed_frames = frames;
        // RT observed this generation active — it is the latest
        // published plan (activation only records against `pending`),
        // so every other generation's plan is retired and its host
        // instances tear down.
        self.connections.retain(|g, _| *g == generation);
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

    /// Records a `Blocked` receipt for a `publish`-stage rejection
    /// and returns the fault.
    fn blocked(
        &mut self,
        request_id: RoutingBuildRequestId,
        key: &RoutingBuildKey,
        fault: RoutingFault,
    ) -> RoutingFault {
        self.fail_receipt(
            Some(request_id),
            key,
            RoutingActivationState::Blocked {
                fault,
                last_good_generation: self.ledger.last_good_generation,
            },
            fault,
        )
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
            frames_observed: self
                .ledger
                .pending
                .as_ref()
                .map_or(self.ledger.observed_frames, |p| p.frames),
            fault,
        };
        self.ledger.receipt = Some(receipt.clone());
        receipt
    }
}

/// Maps a non-intentional [`SkippedPlayback`] to the [`RoutingFault`]
/// the build must fail with. Intentional silence — `Muted`,
/// `SupersededByComp`, `CompSilence` — returns `None`; everything
/// else means content that should play could not be lowered and is
/// never silently replaced by silence.
fn skipped_fault(skipped: &[SkippedPlayback]) -> Option<RoutingFault> {
    skipped.iter().find_map(|skip| match skip {
        SkippedPlayback::Muted { .. }
        | SkippedPlayback::SupersededByComp { .. }
        | SkippedPlayback::CompSilence { .. } => None,
        SkippedPlayback::MissingSource { .. } => Some(RoutingFault::MissingSource),
        SkippedPlayback::RateMismatch { .. }
        | SkippedPlayback::UnresolvableAnchor { .. }
        | SkippedPlayback::UnsupportedMapping { .. } => {
            Some(RoutingFault::UnsupportedFormatOrTimeMap)
        }
        SkippedPlayback::CompProjection { .. } => Some(RoutingFault::InvalidRouting),
        SkippedPlayback::CompSegment { reason, .. }
        | SkippedPlayback::CompOverlap { reason, .. } => match reason {
            CompSkipReason::MissingSource(_) => Some(RoutingFault::MissingSource),
            CompSkipReason::RateMismatch(_)
            | CompSkipReason::Unresolvable
            | CompSkipReason::UnsupportedMapping => Some(RoutingFault::UnsupportedFormatOrTimeMap),
        },
    })
}

/// Classifies a `GraphProject::prepare` rejection: validator cycle
/// errors are `Cycle`, every other validation failure is
/// `InvalidRouting`.
fn graph_prepare_fault(error: &PrepareError) -> RoutingFault {
    match error {
        PrepareError::Validation { errors, .. }
            if errors.errors().iter().any(|e| {
                matches!(
                    e,
                    ValidationError::ZeroDelayCycle { .. }
                        | ValidationError::DelayedCycleUnsupported { .. }
                )
            }) =>
        {
            RoutingFault::Cycle
        }
        _ => RoutingFault::InvalidRouting,
    }
}

/// Classifies a typed-compiler rejection: a missing processor
/// instance is `PluginUnavailable`, resource overflows are
/// `CompileResourceExceeded`, and every other rejection means the
/// lowered spec itself is invalid.
fn compile_fault(error: &CompileError) -> RoutingFault {
    match error {
        CompileError::MissingProcessor { .. } => RoutingFault::PluginUnavailable,
        CompileError::SlotOverflow { .. }
        | CompileError::DelayLineOverflow { .. }
        | CompileError::DelayOverflow { .. }
        | CompileError::ProcessorOverflow { .. } => RoutingFault::CompileResourceExceeded,
        _ => RoutingFault::InvalidRouting,
    }
}
