//! PC-C3-05 trace adapter: projects the dispatch ledger's real
//! events into per-target [`ControlTrace`]s (issue #100,
//! TRACE-C3-05).
//!
//! [`DispatchTraceAdapter`] **owns** a [`ParamDispatch`]: the report
//! channel has exactly one drain owner — [`Self::observe_cycle`] —
//! so no second reader can silently drop receipts or faults. The
//! adapter never re-implements ledger state; it maps
//! [`ParamValueObservation`]/[`DispatchStage`]/scheduler receipts
//! onto the project layer's stage slots and keeps a bounded event
//! history plus measured metrics.
//!
//! Honesty contract (inherited from `synvelis_project::trace`):
//!
//! - `Dispatched` evidence is a plan×span *projection*, never a
//!   plugin-side acknowledgment.
//! - `plugin_reported` carries an `Estimated` stamp — the report
//!   channel does not timestamp values, so the adapter stamps the
//!   drain-time transport position and the stamp can never classify
//!   as `Current`.
//! - Missing evidence is never synthesized: unresolvable plan
//!   generations surface as [`TraceSlot::Expired`], absent values
//!   leave the slot untouched.
//!
//! RT boundary: [`Self::record_configured`], [`Self::record_probe`],
//! [`Self::trace`], [`Self::metrics`] and the history queries are
//! allocation/lock/block-free once warm (verified by the `rt_`
//! test). [`Self::observe_cycle`], [`Self::dispatch`] and
//! [`Self::bind`] are non-RT orchestration paths and must never run
//! inside an RT callback.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use ruviform_audio_plugin_bridge::{
    DispatchStage, DropReason, ParamBinding, ParamValueObservation,
};
use ruviform_media_core::{
    AppliedReceipt, ApplyStatus, ClockEpochId, EngineFrame, LoopOccurrence, MediaDuration,
    MediaTime, MediaTimestamp, PlanGeneration, TimeBase, TimestampProvenance, TransportGeneration,
    TransportSpan,
};
use ruviform_plugin_core::{EventKind, ParamId, ParamInfo, ParamValue, TimedParamEvent};
use synvelis_project::trace::{
    AppliedStageReceipt, ConfiguredEvidence, ControlTrace, ControlTraceKey, DispatchDropReason,
    DispatchEvidence, PluginReportedEvidence, PreparedEvidence, ProbeObservation, ReceiptEvidence,
    ScheduledReceipt, TraceSlot,
};
use synvelis_project::{ParameterTargetId, ProjectId, ProjectRevision};

use crate::dispatch::{DispatchError, ParamDispatch, in_range};

/// Bounded-history and correlation-window configuration. Every cap
/// is explicit; defaults keep memory bounded for inspector-scale
/// use.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TraceAdapterConfig {
    /// Maximum retained [`TraceEvent`]s — oldest are evicted and
    /// counted as lost.
    pub history_capacity: usize,
    /// Maximum tracked targets — the least-recently-updated target
    /// is evicted and counted as lost beyond this.
    pub max_targets: usize,
    /// How many recent `dispatch` generations keep their
    /// ledger-revision → plan-generation mapping. Observations whose
    /// revision falls outside the window cannot be correlated and
    /// surface as [`TraceSlot::Expired`].
    pub plan_gen_window: usize,
}

impl Default for TraceAdapterConfig {
    fn default() -> Self {
        Self {
            history_capacity: 256,
            max_targets: 1024,
            plan_gen_window: 64,
        }
    }
}

/// Project-side context stamped onto trace keys — supplied by the
/// caller (project revision, graph generation) because the adapter
/// cannot observe project commits itself.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TraceContext {
    /// Owning project.
    pub project_id: ProjectId,
    /// Committed project revision evidence is produced under.
    pub project_revision: ProjectRevision,
    /// Active graph build generation.
    pub graph_generation: u64,
}

/// Measured counters and clock/latency stamps accumulated by the
/// adapter — a `Copy` snapshot returned by [`DispatchTraceAdapter::metrics`].
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct TraceMetrics {
    /// Total history events recorded (including evicted ones).
    pub events_recorded: u64,
    /// History events evicted by the ring cap.
    pub history_lost: u64,
    /// Targets evicted by the target cap.
    pub targets_lost: u64,
    /// Observations whose plan generation fell outside the
    /// correlation window.
    pub expired_correlations: u64,
    /// Ledger observations observed in a `Dropped` stage.
    pub dropped_observations: u64,
    /// Number of latency samples recorded.
    pub latency_samples: u64,
    /// Most recent apply latency in seconds.
    pub last_latency_seconds: Option<f64>,
    /// Largest observed apply latency in seconds.
    pub max_latency_seconds: f64,
    /// Normalized value of the newest observation (`f64` channel).
    pub last_value: Option<f64>,
    /// Newest measurement stamp recorded (`plugin_reported` /
    /// `observed` channels).
    pub last_clock: Option<MediaTimestamp>,
    /// Engine frame of the newest dispatched projection.
    pub last_call_start: Option<EngineFrame>,
    /// Plan generation of the newest dispatched plan.
    pub last_plan_generation: Option<PlanGeneration>,
    /// Transport generation of the newest attributed receipt.
    pub last_transport_generation: Option<TransportGeneration>,
    /// Clock epoch of the newest attributed receipt.
    pub last_clock_epoch: Option<ClockEpochId>,
}

/// What a [`TraceEvent`] records.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum TraceEventKind {
    /// A dispatch plan was staged for the target.
    DispatchPlanned,
    /// An observe cycle changed a stage slot.
    ObservationUpdated,
    /// A scheduler receipt was attributed to the target.
    ReceiptRecorded,
    /// Configured evidence was recorded.
    ConfiguredRecorded,
    /// Probe evidence was recorded.
    ProbeRecorded,
    /// Evidence could not be correlated (generation window miss).
    EvidenceExpired,
    /// The target was evicted by the target cap.
    TargetEvicted,
}

/// One bounded-history record. `Copy` — safe to retain.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TraceEvent {
    /// Monotonic sequence number.
    pub seq: u64,
    /// The target the event belongs to.
    pub target: ParameterTargetId,
    /// What happened.
    pub kind: TraceEventKind,
}

/// Per-target mutable trace state.
struct TargetTrace {
    trace: ControlTrace,
    /// Dedup key `(ledger revision, stage)` of the last applied
    /// observation — unchanged observations do not rewrite slots.
    last_obs: Option<(u64, DispatchStage)>,
    /// Last-update sequence for LRU eviction.
    last_seq: u64,
}

/// The PC-C3-05 RT trace adapter — owns [`ParamDispatch`] and is the
/// single drain owner of the node report channel.
pub struct DispatchTraceAdapter {
    dispatch: ParamDispatch,
    /// Frame base plans/positions are expressed in (== attach
    /// `frame_base`, forwarded for latency math).
    frame_base: TimeBase,
    config: TraceAdapterConfig,
    ctx: TraceContext,
    targets: BTreeMap<ParameterTargetId, TargetTrace>,
    /// `(ledger revision, plan generation)` for recent dispatches —
    /// bounded by `plan_gen_window`.
    plan_gens: VecDeque<(u64, PlanGeneration)>,
    /// Number of successful dispatches — equals the ledger revision
    /// the next `record_plan` stamps (the ledger starts at 0 and
    /// bumps once per `record_plan`; `ParamDispatch::dispatch` is
    /// the only caller and this adapter is its only driver).
    dispatches: u64,
    history: VecDeque<TraceEvent>,
    seq: u64,
    metrics: TraceMetrics,
    /// Live transport-side stamps from the newest observe cycle —
    /// fallback for trace-key gens before any receipt arrives.
    live_transport: Option<TransportGeneration>,
    live_clock_epoch: Option<ClockEpochId>,
    live_occurrence: LoopOccurrence,
    live_position: MediaTime,
}

impl DispatchTraceAdapter {
    /// Takes ownership of `dispatch` — after this call the only way
    /// to drain node reports is [`Self::observe_cycle`]. `ctx` seeds
    /// the project-side key fields; [`Self::observe_cycle`] refreshes
    /// them each cycle.
    #[must_use]
    pub fn new(
        dispatch: ParamDispatch,
        frame_base: TimeBase,
        config: TraceAdapterConfig,
        ctx: TraceContext,
    ) -> Self {
        Self {
            dispatch,
            frame_base,
            config,
            ctx,
            targets: BTreeMap::new(),
            plan_gens: VecDeque::new(),
            dispatches: 0,
            history: VecDeque::with_capacity(config.history_capacity.max(1)),
            seq: 0,
            metrics: TraceMetrics::default(),
            live_transport: None,
            live_clock_epoch: None,
            live_occurrence: LoopOccurrence::new(0),
            live_position: MediaTime::new(0, frame_base),
        }
    }

    /// Binds `target` to the plugin-local `param` — delegates to
    /// [`ParamDispatch::bind`]. Non-RT.
    ///
    /// # Errors
    /// Same as [`ParamDispatch::bind`].
    pub fn bind(
        &mut self,
        target: ParameterTargetId,
        param: ParamId,
        params: &[ParamInfo],
    ) -> Result<ParamBinding, DispatchError> {
        self.dispatch.bind(target, param, params)
    }

    /// Revalidates `target`'s binding — delegates to
    /// [`ParamDispatch::revalidate`]. Non-RT.
    pub fn revalidate(&mut self, target: ParameterTargetId, params: &[ParamInfo]) -> bool {
        self.dispatch.revalidate(target, params)
    }

    /// Drops `target`'s binding — delegates to [`ParamDispatch::unbind`].
    /// Non-RT.
    pub fn unbind(&mut self, target: ParameterTargetId) -> bool {
        self.dispatch.unbind(target)
    }

    /// Stages `writes` as a new plan — delegates to
    /// [`ParamDispatch::dispatch`] and, on success, records
    /// `PreparedEvidence` plus the revision→generation mapping the
    /// ledger stamps. Non-RT.
    ///
    /// # Errors
    /// Same as [`ParamDispatch::dispatch`].
    pub fn dispatch(
        &mut self,
        writes: &[(ParameterTargetId, TimedParamEvent)],
    ) -> Result<std::sync::Arc<ruviform_media_core::TransportPlan<EventKind>>, DispatchError> {
        let plan = self.dispatch.dispatch(writes)?;
        self.dispatches = self.dispatches.wrapping_add(1);
        let revision = self.dispatches;
        let generation = plan.generation;
        self.plan_gens.push_back((revision, generation));
        while self.plan_gens.len() > self.config.plan_gen_window.max(1) {
            self.plan_gens.pop_front();
        }
        self.metrics.last_plan_generation = Some(generation);
        let ctx = self.ctx;
        for (target, event) in writes {
            let seq = self.next_seq();
            let t = self.target_slot(*target, seq);
            t.trace.prepared = TraceSlot::Present(PreparedEvidence {
                plan_generation: generation,
            });
            t.trace.key.control_plan_generation = generation;
            t.trace.key.evaluation_position = event.position;
            Self::refresh_key_ctx(&ctx, t);
            t.last_obs = None;
            self.push_event(*target, TraceEventKind::DispatchPlanned);
        }
        Ok(plan)
    }

    /// Advances the trace projection one cycle: drains node reports
    /// and projects the ledger exactly once (inside
    /// [`ParamDispatch::observe_cycle`]), then maps every observation
    /// and receipt onto the owning target's [`ControlTrace`]. This is
    /// the **only** report-channel drain path — single drain owner.
    ///
    /// `span`/`call_start`/`call_frames` describe the physical call
    /// window exactly as [`ParamDispatch::observe`] requires.
    /// `ctx` refreshes the project-side key fields for evidence
    /// recorded this cycle. Non-RT — allocates; never call from an
    /// RT callback.
    pub fn observe_cycle(
        &mut self,
        span: &TransportSpan,
        call_start: EngineFrame,
        call_frames: u32,
        ctx: &TraceContext,
    ) {
        self.ctx = *ctx;
        self.live_transport = Some(span.ctx.transport_generation);
        self.live_clock_epoch = Some(span.ctx.clock_epoch);
        self.live_occurrence = span.ctx.occurrence;
        self.live_position = span.ctx.position();

        let cycle = self.dispatch.observe_cycle(span, call_start, call_frames);

        // Receipts first: they carry the scheduler's post-apply
        // generations, which become the trace-key transport/clock
        // context for attributed targets.
        let mut attributed = BTreeSet::new();
        for receipt in &cycle.receipts {
            self.apply_receipt(receipt, &mut attributed);
        }
        for (target, obs) in &cycle.observations {
            self.apply_observation(*target, obs, span, &attributed);
        }
    }

    /// Records a plugin-reported value into the ledger — the stage
    /// becomes visible on the next [`Self::observe_cycle`]. Non-RT.
    /// Returns `false` when no eligible ledger entry exists.
    pub fn record_reported(&mut self, target: ParameterTargetId, value: ParamValue) -> bool {
        if !in_range(target) {
            return false;
        }
        self.dispatch.record_reported(target, value)
    }

    /// Declares no plugin→host reporting channel: pending ledger
    /// entries move to `Unavailable` on the next observe cycle.
    /// Non-RT.
    pub fn mark_reporting_unavailable(&mut self) {
        self.dispatch.mark_reporting_unavailable();
    }

    /// Records host-computed configured evidence for `target`.
    /// RT-safe after warm-up (bounded ring push only).
    pub fn record_configured(&mut self, target: ParameterTargetId, evidence: ConfiguredEvidence) {
        let ctx = self.ctx;
        let seq = self.next_seq();
        let t = self.target_slot(target, seq);
        t.trace.configured = TraceSlot::Present(evidence);
        Self::refresh_key_ctx(&ctx, t);
        self.push_event(target, TraceEventKind::ConfiguredRecorded);
    }

    /// Records probe-channel evidence for `target` — `Missing`,
    /// `Disabled`, `Lost` and `Measuring` are preserved verbatim.
    /// RT-safe after warm-up.
    pub fn record_probe(&mut self, target: ParameterTargetId, observation: ProbeObservation) {
        if let ProbeObservation::Measuring { stamp, .. } = observation {
            self.metrics.last_clock = Some(stamp);
        }
        let position = match observation {
            ProbeObservation::Measuring { stamp, .. } => Some(stamp.time),
            _ => None,
        };
        let ctx = self.ctx;
        let seq = self.next_seq();
        let t = self.target_slot(target, seq);
        t.trace.observed = TraceSlot::Present(observation);
        Self::refresh_key_ctx(&ctx, t);
        if let Some(position) = position {
            t.trace.key.evaluation_position = position;
        }
        self.push_event(target, TraceEventKind::ProbeRecorded);
    }

    /// The trace for `target`, if the adapter tracks it.
    #[must_use]
    pub fn trace(&self, target: ParameterTargetId) -> Option<&ControlTrace> {
        self.targets.get(&target).map(|t| &t.trace)
    }

    /// All tracked traces — non-RT (allocates).
    #[must_use]
    pub fn traces(&self) -> Vec<(ParameterTargetId, ControlTrace)> {
        self.targets
            .iter()
            .map(|(target, t)| (*target, t.trace.clone()))
            .collect()
    }

    /// Accumulated metrics snapshot.
    #[must_use]
    pub const fn metrics(&self) -> TraceMetrics {
        self.metrics
    }

    /// Retained history length.
    #[must_use]
    pub fn history_len(&self) -> usize {
        self.history.len()
    }

    /// Retained history, oldest first — non-RT (allocates).
    #[must_use]
    pub fn history(&self) -> Vec<TraceEvent> {
        self.history.iter().copied().collect()
    }

    /// Whether any evidence was lost to a cap (history ring, target
    /// cap, or generation-correlation window).
    #[must_use]
    pub fn is_truncated(&self) -> bool {
        self.metrics.history_lost > 0
            || self.metrics.targets_lost > 0
            || self.metrics.expired_correlations > 0
    }

    /// The number of tracked targets.
    #[must_use]
    pub fn target_count(&self) -> usize {
        self.targets.len()
    }

    // -- internals ----------------------------------------------------

    fn next_seq(&mut self) -> u64 {
        self.seq = self.seq.wrapping_add(1);
        self.seq
    }

    /// Returns the target's trace state with `last_seq` already
    /// stamped, creating it (all-Missing trace keyed to the current
    /// context) when absent. Evicts the least-recently-updated target
    /// beyond `max_targets`.
    fn target_slot(&mut self, target: ParameterTargetId, seq: u64) -> &mut TargetTrace {
        if !self.targets.contains_key(&target)
            && self.targets.len() >= self.config.max_targets.max(1)
        {
            self.evict_oldest_target();
        }
        if !self.targets.contains_key(&target) {
            let trace = self.fresh_trace(target);
            self.targets.insert(
                target,
                TargetTrace {
                    trace,
                    last_obs: None,
                    last_seq: seq,
                },
            );
        }
        let t = self
            .targets
            .get_mut(&target)
            .expect("target inserted above");
        t.last_seq = seq;
        t
    }

    fn fresh_trace(&self, target: ParameterTargetId) -> ControlTrace {
        ControlTrace {
            key: ControlTraceKey {
                project_id: self.ctx.project_id,
                parameter_target_id: target,
                project_revision: self.ctx.project_revision,
                control_plan_generation: PlanGeneration::new(0),
                graph_generation: self.ctx.graph_generation,
                transport_generation: self
                    .live_transport
                    .unwrap_or_else(|| TransportGeneration::new(0)),
                clock_epoch: self
                    .live_clock_epoch
                    .unwrap_or_else(|| ClockEpochId::new(0)),
                loop_occurrence: self.live_occurrence,
                evaluation_position: self.live_position,
            },
            configured: TraceSlot::Missing,
            prepared: TraceSlot::Missing,
            scheduled: TraceSlot::Missing,
            applied: TraceSlot::Missing,
            dispatched: TraceSlot::Missing,
            plugin_reported: TraceSlot::Missing,
            observed: TraceSlot::Missing,
        }
    }

    /// Refreshes the caller-supplied key fields (project identity,
    /// revision, graph generation) from the stored context — called
    /// whenever new evidence is recorded for the target. Static so
    /// callers can hold a `&mut TargetTrace` into `self.targets`
    /// while reading `self.ctx`.
    fn refresh_key_ctx(ctx: &TraceContext, t: &mut TargetTrace) {
        t.trace.key.project_id = ctx.project_id;
        t.trace.key.project_revision = ctx.project_revision;
        t.trace.key.graph_generation = ctx.graph_generation;
    }

    fn evict_oldest_target(&mut self) {
        if let Some((&oldest, _)) = self.targets.iter().min_by_key(|(_, t)| t.last_seq) {
            self.targets.remove(&oldest);
            self.metrics.targets_lost += 1;
            self.push_event(oldest, TraceEventKind::TargetEvicted);
        }
    }

    fn push_event(&mut self, target: ParameterTargetId, kind: TraceEventKind) {
        self.seq = self.seq.wrapping_add(1);
        self.metrics.events_recorded += 1;
        let event = TraceEvent {
            seq: self.seq,
            target,
            kind,
        };
        if self.history.len() >= self.config.history_capacity.max(1) {
            self.history.pop_front();
            self.metrics.history_lost += 1;
        }
        self.history.push_back(event);
    }

    /// Maps a ledger revision to its plan generation through the
    /// bounded window. `None` = outside the window.
    fn generation_for_revision(&self, revision: u64) -> Option<PlanGeneration> {
        self.plan_gens
            .iter()
            .find(|(rev, _)| *rev == revision)
            .map(|(_, generation)| *generation)
    }

    /// Applies one scheduler receipt to every target whose staged
    /// plan generation it names. Receipts are node-scoped; the
    /// `plan_generation` match is the only honest attribution.
    /// `attributed` collects the targets this receipt landed on so
    /// observation mapping does not overwrite receipt-freshened key
    /// generations with span-context values.
    fn apply_receipt(
        &mut self,
        receipt: &AppliedReceipt,
        attributed: &mut BTreeSet<ParameterTargetId>,
    ) {
        let evidence = ReceiptEvidence::from_applied_receipt(receipt);
        let matched: Vec<ParameterTargetId> = self
            .targets
            .iter()
            .filter(|(_, t)| t.trace.key.control_plan_generation == evidence.plan_generation)
            .map(|(target, _)| *target)
            .collect();
        for target in matched {
            let Some(t) = self.targets.get_mut(&target) else {
                continue;
            };
            match receipt.status {
                ApplyStatus::Requested | ApplyStatus::Pending => {
                    if let Some(wrapped) = ScheduledReceipt::new(evidence) {
                        t.trace.scheduled = TraceSlot::Present(wrapped);
                    }
                }
                ApplyStatus::Applied => {
                    if let Some(wrapped) = AppliedStageReceipt::new(evidence) {
                        t.trace.applied = TraceSlot::Present(wrapped);
                    }
                }
                // Rejected (and any future status): fail-closed —
                // the slot keeps its previous honest state.
                _ => {}
            }
            // The receipt's post-apply generations are the freshest
            // transport/clock truth for this target's context.
            t.trace.key.transport_generation = receipt.transport_generation;
            t.trace.key.clock_epoch = receipt.clock_epoch;
            attributed.insert(target);
            self.metrics.last_transport_generation = Some(receipt.transport_generation);
            self.metrics.last_clock_epoch = Some(receipt.clock_epoch);
            self.record_latency(receipt);
            self.push_event(target, TraceEventKind::ReceiptRecorded);
        }
    }

    /// Records apply latency (`applied_frame - requested_frame`) in
    /// seconds for receipts that reached `Applied`.
    fn record_latency(&mut self, receipt: &AppliedReceipt) {
        let Some(applied) = receipt.applied_frame else {
            return;
        };
        let Some(frames) = applied.checked_offset(receipt.requested_frame) else {
            return;
        };
        let Ok(frames_i64) = i64::try_from(frames) else {
            return;
        };
        let seconds = MediaDuration::new(frames_i64, self.frame_base).to_seconds_f64();
        self.metrics.latency_samples += 1;
        self.metrics.last_latency_seconds = Some(seconds);
        if seconds > self.metrics.max_latency_seconds {
            self.metrics.max_latency_seconds = seconds;
        }
    }

    /// Maps one ledger observation onto its target's `dispatched`
    /// slot (+ `plugin_reported` for `Reported`).
    fn apply_observation(
        &mut self,
        target: ParameterTargetId,
        obs: &ParamValueObservation,
        span: &TransportSpan,
        attributed: &BTreeSet<ParameterTargetId>,
    ) {
        // Dedup: unchanged (revision, stage) does not rewrite.
        if let Some(t) = self.targets.get(&target)
            && t.last_obs == Some((obs.revision, obs.stage))
        {
            return;
        }

        let mapped = map_observation(
            obs,
            self.generation_for_revision(obs.revision),
            span.ctx.clock_epoch,
        );
        let Some(t) = self.targets.get_mut(&target) else {
            return;
        };
        t.last_obs = Some((obs.revision, obs.stage));

        match mapped {
            MappedObservation::Present(evidence) => {
                t.trace.dispatched = TraceSlot::Present(evidence);
                self.apply_dispatched_side_effects(target, evidence, span, attributed);
            }
            MappedObservation::Skip => {
                // Stage matched but the value was absent — evidence
                // cannot be synthesized; leave the slot untouched.
            }
            MappedObservation::Expired => {
                // Generation outside the correlation window — honest
                // expiry, never a fabricated generation.
                t.trace.dispatched = TraceSlot::Expired;
                t.trace.key.control_plan_generation = PlanGeneration::new(0);
                self.metrics.expired_correlations += 1;
                self.push_event(target, TraceEventKind::EvidenceExpired);
                return;
            }
        }
        if matches!(obs.stage, DispatchStage::Dropped { .. }) {
            self.metrics.dropped_observations += 1;
        }
        if let Some(value) = obs.value {
            self.metrics.last_value = Some(value.get());
        }
        if let DispatchStage::Dispatched { call_start, .. } = obs.stage {
            self.metrics.last_call_start = Some(EngineFrame::new(span.ctx.clock_epoch, call_start));
        }
        self.push_event(target, TraceEventKind::ObservationUpdated);
    }

    /// Side effects of a `Present` dispatched evidence: key context
    /// refresh and the `plugin_reported` slot for `Reported`.
    fn apply_dispatched_side_effects(
        &mut self,
        target: ParameterTargetId,
        evidence: DispatchEvidence,
        span: &TransportSpan,
        attributed: &BTreeSet<ParameterTargetId>,
    ) {
        let ctx = self.ctx;
        let Some(t) = self.targets.get_mut(&target) else {
            return;
        };
        Self::refresh_key_ctx(&ctx, t);
        // Live transport-side context for this observation — unless a
        // receipt already supplied fresher post-apply generations.
        if !attributed.contains(&target) {
            t.trace.key.transport_generation = span.ctx.transport_generation;
            t.trace.key.clock_epoch = span.ctx.clock_epoch;
        }
        t.trace.key.loop_occurrence = span.ctx.occurrence;
        t.trace.key.evaluation_position = span.ctx.position();

        if let DispatchEvidence::Reported { value, .. } = evidence {
            // The report channel carries no timestamp — stamp the
            // drain-time transport position as `Estimated` so the
            // freshness classifier can never call it `Current`.
            let stamp = MediaTimestamp {
                time: span.ctx.position(),
                provenance: TimestampProvenance::Estimated,
            };
            t.trace.plugin_reported = TraceSlot::Present(PluginReportedEvidence { value, stamp });
            self.metrics.last_clock = Some(stamp);
        }
    }
}

/// Result of mapping one ledger observation onto dispatch evidence.
enum MappedObservation {
    /// Presentable evidence for the `dispatched` slot.
    Present(DispatchEvidence),
    /// Nothing may be recorded — the stage carried no value or is
    /// unknown; the slot keeps its previous honest state.
    Skip,
    /// The stage needs a plan generation the bounded window no
    /// longer resolves — surface `Expired`, never fabricate.
    Expired,
}

/// Maps one ledger observation onto dispatch evidence.
fn map_observation(
    obs: &ParamValueObservation,
    generation: Option<PlanGeneration>,
    clock_epoch: ClockEpochId,
) -> MappedObservation {
    match obs.stage {
        DispatchStage::Scheduled
        | DispatchStage::Dispatched { .. }
        | DispatchStage::Dropped { .. } => {
            let Some(generation) = generation else {
                return MappedObservation::Expired;
            };
            match obs.stage {
                DispatchStage::Scheduled => obs.value.map_or(MappedObservation::Skip, |value| {
                    MappedObservation::Present(DispatchEvidence::Scheduled {
                        value,
                        position: obs.position,
                        plan_generation: generation,
                    })
                }),
                DispatchStage::Dispatched {
                    call_start,
                    frame_offset,
                } => obs.value.map_or(MappedObservation::Skip, |value| {
                    MappedObservation::Present(DispatchEvidence::Dispatched {
                        value,
                        position: obs.position,
                        plan_generation: generation,
                        call_start: EngineFrame::new(clock_epoch, call_start),
                        frame_offset,
                    })
                }),
                DispatchStage::Dropped { reason } => {
                    MappedObservation::Present(DispatchEvidence::Dropped {
                        position: obs.position,
                        plan_generation: generation,
                        reason: map_drop_reason(reason),
                    })
                }
                _ => MappedObservation::Skip,
            }
        }
        DispatchStage::Reported => obs.value.map_or(MappedObservation::Skip, |value| {
            MappedObservation::Present(DispatchEvidence::Reported {
                value,
                position: obs.position,
            })
        }),
        DispatchStage::Unavailable => MappedObservation::Present(DispatchEvidence::Unavailable {
            position: obs.position,
        }),
        _ => MappedObservation::Skip,
    }
}

/// Bridge `DropReason` → project `DispatchDropReason` — the semantic
/// mirror, kept in exactly one place. Unknown future variants map to
/// `ScheduleFault` rather than silently succeeding.
fn map_drop_reason(reason: DropReason) -> DispatchDropReason {
    match reason {
        DropReason::EventScratchOverflow => DispatchDropReason::EventScratchOverflow,
        DropReason::EndpointError { kind } => DispatchDropReason::EndpointError { kind },
        DropReason::UndeliverableSpan => DispatchDropReason::UndeliverableSpan,
        DropReason::PlanReplaced => DispatchDropReason::PlanReplaced,
        DropReason::VaultExceeded => DispatchDropReason::VaultExceeded,
        // `ScheduleFault` and any future variant: fault-class drop.
        _ => DispatchDropReason::ScheduleFault,
    }
}
