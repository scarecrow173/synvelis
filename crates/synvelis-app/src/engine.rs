//! Session-layer routing engine — the [`EnginePort`] implementation
//! that connects [`ProjectSession`](synvelis_workspace::ProjectSession)
//! to [`RoutingEngine`] (SF-03 slice 5).
//!
//! [`SessionRoutingEngine`] is a cloneable handle over shared inner
//! state so both the session (as `Box<dyn EnginePort>`) and headless
//! callers/tests can observe the same engine. On every committed
//! project revision it:
//!
//! 1. counts the notification (`graph_builds`/`last_built_revision`),
//! 2. plans playbacks via [`plan_playbacks`] with a per-build
//!    `canonical_of` probe cache backed by
//!    [`resolver::canonical_format`],
//! 3. prepares one [`PreparedClip`] per playback in plan order,
//! 4. drives `RoutingEngine::prepare` → `publish` with a fresh
//!    [`RoutingCompileRequest`].
//!
//! All activation/last-good/stale/equivalence semantics stay inside
//! `RoutingEngine` — this adapter only supplies honest inputs and
//! records adapter-side clip resolution failures in
//! [`SessionRoutingEngine::last_prepare_error`] without touching the
//! engine's live plan.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use ruviform_audio::typed::{ProcessError, SharedStatusSnapshot};
use ruviform_audio_core::AudioFormat;
use ruviform_media_reader::PrefetchConfig;

use synvelis_player::ProcessorControl;
use synvelis_player::driver::{PlayReport, PumpPolicy};
use synvelis_player::host::ProcessorHost;
use synvelis_player::lowering::{EngineConfig, plan_playbacks};
use synvelis_player::probe_trace::ProbeTraceBridge;
use synvelis_player::resolver::{self, PreparedClip};
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine, RoutingPrepare};
use synvelis_project::{
    BuildBudget, BuildMode, ProcessorId, Project, ProjectRevision, RoutingActivationReceipt,
    RoutingBuildRequestId, RoutingCompileRequest, RoutingReconcileReport, RoutingSourceMap,
    RuntimeClockEpochRef, RuntimeInstanceId, SemanticProbeBinding, SourceId,
};
use synvelis_workspace::EnginePort;

/// Static configuration for a [`SessionRoutingEngine`]. One adapter
/// instance is bound to one media root and one engine geometry; a
/// project switch or geometry change means a new adapter (and a new
/// `RoutingEngine`, so generations never leak across projects).
#[derive(Clone, Debug)]
pub struct SessionEngineConfig {
    /// Root of the verified media blob store (`blob_path` layout).
    pub media_root: PathBuf,
    /// Engine geometry — must match the `RoutingEngine`'s
    /// `EngineConfig` and the `PrefetchConfig` channel/rate layout.
    pub engine: EngineConfig,
    /// Prefetch tuning for every prepared playback.
    pub prefetch: PrefetchConfig,
    /// Compile resource ceiling carried on every request.
    pub budget: BuildBudget,
    /// Build mode stamped on every request.
    pub mode: BuildMode,
    /// Runtime instance this engine serves — requests under any other
    /// instance are rejected by the engine.
    pub runtime_instance: RuntimeInstanceId,
    /// Device clock epoch this engine serves.
    pub device_clock_epoch: RuntimeClockEpochRef,
    /// Engine geometry generation reported in the build key.
    /// Bump (via a new adapter) whenever geometry changes.
    pub engine_config_generation: u64,
}

/// Adapter-side preparation failure — the engine was *not* called,
/// so its receipt/live plan are untouched (last-good preserved).
/// Engine-level faults are reported through the engine's own
/// [`RoutingActivationReceipt`], not here.
#[derive(Clone, Debug)]
pub enum SessionEngineError {
    /// A playback's source could not be resolved into a
    /// [`PreparedClip`] after playback planning succeeded (e.g. the
    /// verified blob vanished between probe and prepare).
    ClipResolve {
        /// The playback's source.
        source_id: SourceId,
        /// The resolver's typed failure, rendered.
        error: String,
    },
}

impl std::fmt::Display for SessionEngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ClipResolve { source_id, error } => {
                write!(f, "clip source {source_id} failed to resolve: {error}")
            }
        }
    }
}

impl std::error::Error for SessionEngineError {}

/// Shared engine state behind the cloneable [`SessionRoutingEngine`]
/// handle.
struct Inner {
    /// The [`ProbeTraceBridge`] owns the [`RoutingEngine`] plus the
    /// node-keyed trace adapters — the probe/trace port the session
    /// layer drains through (SF-04 slice 4). Engine lifecycle calls
    /// go through `bridge.engine()`/`engine_mut()`; probe lifecycle
    /// and drain go through the port impl so session correlation and
    /// drain ownership stay single-sourced.
    bridge: ProbeTraceBridge,
    config: SessionEngineConfig,
    /// Monotonic request id — one per revision notification that
    /// reaches `prepare`.
    next_request_id: u64,
    graph_builds: u64,
    last_built_revision: ProjectRevision,
    /// Canonical formats consulted by the last build, keyed by source.
    source_formats: BTreeMap<SourceId, AudioFormat>,
    /// Generation counter bumped whenever the consulted canonical
    /// format set changes.
    source_format_generation: u64,
    last_prepare_error: Option<SessionEngineError>,
}

/// The session-facing routing engine: an [`EnginePort`] that drives a
/// [`RoutingEngine`] through the `ProcessorHost` boundary on every
/// committed project revision. Clone the handle freely — all clones
/// share one engine.
#[derive(Clone)]
pub struct SessionRoutingEngine {
    inner: Rc<RefCell<Inner>>,
}

impl SessionRoutingEngine {
    /// Creates an engine bound to `config`'s media root, geometry,
    /// instance and epoch. The processor host defaults to
    /// `StaticProcessorHost`; swap it with
    /// [`SessionRoutingEngine::set_processor_host`] before the first
    /// build that needs real plugins.
    #[must_use]
    pub fn new(config: SessionEngineConfig) -> Self {
        let engine = RoutingEngine::new(
            config.engine,
            config.runtime_instance,
            config.device_clock_epoch,
        );
        Self {
            inner: Rc::new(RefCell::new(Inner {
                bridge: ProbeTraceBridge::new(engine, BTreeMap::new()),
                config,
                next_request_id: 1,
                graph_builds: 0,
                last_built_revision: ProjectRevision::default(),
                source_formats: BTreeMap::new(),
                source_format_generation: 0,
                last_prepare_error: None,
            })),
        }
    }

    /// Replaces the processor host used by *future* builds — same
    /// semantics as [`RoutingEngine::set_processor_host`].
    pub fn set_processor_host(&self, host: Box<dyn ProcessorHost>) {
        self.inner
            .borrow_mut()
            .bridge
            .engine_mut()
            .set_processor_host(host);
    }

    /// Runs `blocks` blocks of engine geometry — RT pumping plus
    /// activation observation. `Active` receipt evidence appears only
    /// after the published generation is observed here; a successful
    /// publish alone never reports activation.
    ///
    /// # Errors
    /// [`ProcessError`] from the runtime.
    pub fn run(&self, blocks: usize, policy: PumpPolicy) -> Result<PlayReport, ProcessError> {
        self.inner
            .borrow_mut()
            .bridge
            .engine_mut()
            .run(blocks, policy)
    }

    /// Scoped mutable access to the probe/trace port — the
    /// [`ProbeTraceBridge`] that owns this engine (SF-04 slice 4).
    /// `f` runs with `&mut` access; the port's contract reads return
    /// references into shared state, so they cannot escape the scope
    /// — gather owned data inside `f`. This is the composition
    /// root's supply path for [`ControlSession`](synvelis_workspace::ControlSession)'s
    /// per-call `&mut dyn ProbeTracePort` parameters and for
    /// adapter/route registration ([`ProbeTraceBridge::insert_adapter`],
    /// [`ProbeTraceBridge::route_target`]). Do not call engine
    /// lifecycle methods (`run`, `on_project_revision`, …) inside
    /// `f` — they re-borrow the same `RefCell` and would panic.
    pub fn with_probe_bridge<R>(&self, f: impl FnOnce(&mut ProbeTraceBridge) -> R) -> R {
        f(&mut self.inner.borrow_mut().bridge)
    }

    /// The latest activation receipt — `None` before the first
    /// `prepare`. Cloned out of the shared state.
    #[must_use]
    pub fn receipt(&self) -> Option<RoutingActivationReceipt> {
        self.inner.borrow().bridge.engine().receipt().cloned()
    }

    /// Point-in-time RT status snapshot.
    #[must_use]
    pub fn rt_status(&self) -> SharedStatusSnapshot {
        self.inner.borrow().bridge.engine().rt_status()
    }

    /// The latest published plan's source map — `None` before the
    /// first successful publish.
    #[must_use]
    pub fn source_map(&self) -> Option<RoutingSourceMap> {
        self.inner.borrow().bridge.engine().source_map().cloned()
    }

    /// Reconciles the published plan's build key against the live
    /// committed snapshot and the adapter's tracked generations.
    #[must_use]
    pub fn reconcile(
        &self,
        project: &Project,
        revision: ProjectRevision,
    ) -> RoutingReconcileReport {
        let mut inner = self.inner.borrow_mut();
        let engine_config_generation = inner.config.engine_config_generation;
        let source_format_generation = inner.source_format_generation;
        inner.bridge.engine_mut().reconcile(
            project,
            revision,
            engine_config_generation,
            source_format_generation,
        )
    }

    /// Live semantic probe bindings — requested *and* RT-confirmed
    /// resolve states together; keeping the distinction is the
    /// caller's job (SF-05 status display, synvelis#108).
    #[must_use]
    pub fn probe_bindings(&self) -> Vec<SemanticProbeBinding> {
        self.inner
            .borrow()
            .bridge
            .engine()
            .probe_bindings()
            .values()
            .cloned()
            .collect()
    }

    /// Vends the published generation's processor control handle —
    /// see [`RoutingEngine::take_processor_control`].
    #[must_use]
    pub fn take_processor_control(
        &self,
        generation: u64,
        processor: ProcessorId,
    ) -> Option<ProcessorControl> {
        self.inner
            .borrow_mut()
            .bridge
            .engine_mut()
            .take_processor_control(generation, processor)
    }

    /// The most recent adapter-side clip resolution failure —
    /// `Some` only when a build could not assemble `deps.clips` and
    /// the engine was left untouched. Cleared at the start of every
    /// revision notification.
    #[must_use]
    pub fn last_prepare_error(&self) -> Option<SessionEngineError> {
        self.inner.borrow().last_prepare_error.clone()
    }
}

impl EnginePort for SessionRoutingEngine {
    fn on_project_revision(&mut self, project: &Project, revision: ProjectRevision) {
        rebuild(&mut self.inner.borrow_mut(), project, revision);
    }

    fn graph_builds(&self) -> u64 {
        self.inner.borrow().graph_builds
    }

    fn last_built_revision(&self) -> ProjectRevision {
        self.inner.borrow().last_built_revision
    }
}

/// One revision notification: plan → prepare clips → engine
/// prepare → publish. Every failure path is evidence, never a panic
/// and never a session error — the project commit already succeeded.
fn rebuild(inner: &mut Inner, project: &Project, revision: ProjectRevision) {
    inner.graph_builds += 1;
    inner.last_built_revision = revision;
    inner.last_prepare_error = None;

    // Per-build canonical probe cache: `canonical_of` may be consulted
    // repeatedly for the same source within one build (multiple clips
    // or comp bindings), and the cache is fresh per notification so a
    // rebuilt project always sees current media state.
    let cache: RefCell<BTreeMap<SourceId, Option<AudioFormat>>> = RefCell::new(BTreeMap::new());
    let media_root = inner.config.media_root.clone();
    let canonical_of = |source_id: SourceId| -> Option<AudioFormat> {
        let mut cache = cache.borrow_mut();
        *cache
            .entry(source_id)
            .or_insert_with(|| resolver::canonical_format(project, source_id, &media_root).ok())
    };
    let canonical_of: &dyn Fn(SourceId) -> Option<AudioFormat> = &canonical_of;

    let (plans, _skipped) = plan_playbacks(project, canonical_of, &inner.config.engine);

    // Track the consulted canonical format set: the build key's
    // `source_format_generation` pins "the format generation this
    // build consumed" — a change in the consulted map bumps it.
    let consulted: BTreeMap<SourceId, AudioFormat> = cache
        .borrow()
        .iter()
        .filter_map(|(source_id, format)| format.map(|format| (*source_id, format)))
        .collect();
    if consulted != inner.source_formats {
        inner.source_formats = consulted;
        inner.source_format_generation += 1;
    }

    // One PreparedClip per playback plan, in plan order — the same
    // order `lower_semantic` produces inside `prepare`.
    let mut clips: Vec<PreparedClip> = Vec::with_capacity(plans.len());
    for plan in &plans {
        match resolver::prepare(project, plan.source_id, &media_root, &inner.config.prefetch) {
            Ok(prepared) => clips.push(prepared),
            Err(error) => {
                inner.last_prepare_error = Some(SessionEngineError::ClipResolve {
                    source_id: plan.source_id,
                    error: error.to_string(),
                });
                return;
            }
        }
    }

    let request = RoutingCompileRequest {
        request_id: RoutingBuildRequestId(inner.next_request_id),
        expected_project_revision: revision,
        mode: inner.config.mode,
        resource_budget: inner.config.budget,
        runtime_instance: inner.config.runtime_instance,
        device_clock_epoch: inner.config.device_clock_epoch,
    };
    inner.next_request_id += 1;

    let deps = RoutingDeps {
        canonical_of,
        clips,
        engine_config_generation: inner.config.engine_config_generation,
        source_format_generation: inner.source_format_generation,
    };

    match inner
        .bridge
        .engine_mut()
        .prepare(&request, project, revision, deps)
    {
        Ok(RoutingPrepare::Built(prepared)) => {
            // Publish failures are already recorded on the engine's
            // activation receipt — last-good stays live either way.
            let _ = inner
                .bridge
                .engine_mut()
                .publish(*prepared, project, revision);
        }
        // NoRebuild equivalence leaves the live plan untouched, and
        // prepare faults are already recorded on the engine's
        // activation receipt — neither publishes nor mutates state.
        Ok(RoutingPrepare::Equivalent(_)) | Err(_) => {}
    }
}
