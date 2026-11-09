//! `ProbeTraceBridge` — the player-side [`ProbeTracePort`] reference
//! implementation (Issue #107, SF-04 slice 2).
//!
//! The bridge owns a [`RoutingEngine`] (semantic probe lifecycle +
//! drain) and a set of node-keyed [`DispatchTraceAdapter`]s (the
//! `ControlTrace.observed` slot writers), and translates runtime
//! [`ProbeEvent`]s into contract-level [`ProbeObservationUpdate`]s.
//! Runtime identities — `ProbeId`, ruviform record types, graph-local
//! `NodeId` — never cross the port boundary.
//!
//! Fail-closed translation rules:
//!
//! - A record whose probe id is not in the binding snapshot (unknown
//!   or already removed) produces no update.
//! - A `Metric` record whose `generation` differs from the binding's
//!   `active_plan_generation` is stale-generation evidence and is
//!   **rejected** — never recorded as a current observation.
//! - A record whose stamp cannot be honestly constructed (position
//!   outside `i64`, `sample_rate == 0`) produces no update.
//! - `ProbeClockMark` unknown (clock unwired) maps the stamp
//!   provenance to [`TimestampProvenance::Estimated`] — it can never
//!   reach `Current`; a wired clock maps to `Derived` (the position
//!   is derived from the frame cursor, not container-carried).
//! - Status kinds that carry no slot-level observation state
//!   (`Installed`, `Unbound`, `Unobserved`, `CaptureStarted`,
//!   `CaptureStopped`, `ModeChanged` to a live mode, unknown future
//!   variants) produce no update — the binding ledger and segment
//!   gaps already carry that evidence, and the slot honestly ages
//!   out via freshness instead of claiming a state it cannot prove.
//! - `Lost*` counts are the collector's **cumulative** channel-level
//!   counters — broker-wide loss evidence, not per-subscription.
//!
//! [`TimestampProvenance::Estimated`]: ruviform_media_core::TimestampProvenance::Estimated

use std::collections::BTreeMap;

use ruviform_audio::typed::probe::{
    self, MetricRecord, ProbeEvent, ProbeMode as RtProbeMode, StatusKind, StatusRecord,
};
use ruviform_graph::NodeId;
use ruviform_media_core::{MediaTime, MediaTimestamp, TimeBase};
use ruviform_plugin_core::ParamValue;

use synvelis_project::probe::{ProbeCaptureSegment, SemanticProbeBinding};
use synvelis_project::trace::{ControlTrace, MeasuredValue, ProbeObservation};
use synvelis_project::{
    ParameterTargetId, ProbeMode, ProbeObservationUpdate, ProbePortError, ProbeSubscriptionKey,
    ProbeTracePort, Project,
};

use crate::probe::{ProbeRequestError, clock_ref};
use crate::routing_engine::RoutingEngine;
use crate::trace::DispatchTraceAdapter;

/// `ProbeRequestError` → [`ProbePortError`] — the contract layer
/// cannot name ruviform broker errors, so they collapse into
/// `Failed` with the provider's own message preserved.
fn port_error(error: ProbeRequestError) -> ProbePortError {
    match error {
        ProbeRequestError::UnknownKey => ProbePortError::UnknownKey,
        ProbeRequestError::NotBound => ProbePortError::NotBound,
        ProbeRequestError::AlreadyCapturing => ProbePortError::AlreadyCapturing,
        ProbeRequestError::ProjectMismatch => ProbePortError::ProjectMismatch,
        ProbeRequestError::NoActivePlan => ProbePortError::NoActivePlan,
        ProbeRequestError::Broker(e) => ProbePortError::Failed(e.to_string()),
    }
}

/// Probe-id → `(subscription key, active plan generation)` snapshot
/// entry. The raw `u64` is the broker-issued id mirrored on the
/// contract binding as [`RuntimeProbeId`](synvelis_project::RuntimeProbeId)
/// — compared by value only; `ProbeId` itself never leaves the
/// runtime layer.
type ProbeIndex = BTreeMap<u64, (ProbeSubscriptionKey, u64)>;

/// Builds the drain-time probe index from the binding ledger.
/// Bindings without a runtime probe (unresolved/unavailable/stopped)
/// index nothing — their events cannot be honestly attributed.
fn probe_index(engine: &RoutingEngine) -> ProbeIndex {
    engine
        .probe_bindings()
        .iter()
        .filter_map(|(key, binding)| {
            binding
                .runtime_probe_id
                .map(|id| (id.0, (*key, binding.active_plan_generation)))
        })
        .collect()
}

/// `MetricRecord` → [`ProbeObservation::Measuring`], fail-closed.
/// Returns `None` for stale-generation records, unattributable
/// probes, unrepresentable stamps and non-finite peaks.
fn metric_update(record: &MetricRecord, index: &ProbeIndex) -> Option<ProbeObservationUpdate> {
    let meta = &record.meta;
    let &(key, active_generation) = index.get(&meta.probe.get())?;
    // Stale-generation reject: a record minted under a superseded
    // plan generation is never current observation evidence.
    if meta.generation != active_generation {
        return None;
    }
    let position = i64::try_from(meta.sample_position).ok()?;
    let base = TimeBase::sample_rate(meta.sample_rate).ok()?;
    let time = MediaTime::new(position, base);
    // Wired clock → Derived (position derived from the frame cursor);
    // unwired/unknown → Estimated, which can never classify Current.
    let stamp = if clock_ref(meta.clock).is_some() {
        MediaTimestamp::derived(time)
    } else {
        MediaTimestamp::estimated(time)
    };
    let channels = usize::from(record.n).min(record.levels.len());
    if channels == 0 {
        return None;
    }
    let levels = &record.levels[..channels];
    // Fail-closed: a non-finite or negative peak is corrupt evidence.
    // `f32::max` would launder NaN to `0.0` and fabricate
    // `Signal(0.0)` — skip the record instead.
    if levels
        .iter()
        .any(|level| !level.peak.is_finite() || level.peak < 0.0)
    {
        return None;
    }
    let value = if levels
        .iter()
        .all(|level| level.peak == 0.0 && level.rms == 0.0)
    {
        // Every channel measured true silence — a real measurement,
        // distinct from missing/lost/disabled.
        MeasuredValue::Silence
    } else {
        // Block peak normalized to [0,1]; >0 dBFS saturates honestly
        // at the clamp, non-finite input fails closed.
        let peak = levels
            .iter()
            .map(|level| level.peak)
            .fold(0.0_f32, f32::max);
        MeasuredValue::Signal(ParamValue::new(f64::from(peak)).ok()?)
    };
    Some(ProbeObservationUpdate {
        key,
        observation: ProbeObservation::Measuring { stamp, value },
    })
}

/// `StatusRecord` → slot-level observation state, fail-closed.
/// Only kinds that describe the observation channel itself produce
/// an update; lifecycle/transient kinds stay in the binding ledger.
/// No generation guard here: status evidence is channel state, not a
/// measurement claim — e.g. `RebindDropped` legitimately arrives
/// under the *new* generation while the binding still records the
/// old one.
fn status_update(
    record: &StatusRecord,
    index: &ProbeIndex,
    counts: &probe::ProbeCounts,
) -> Option<ProbeObservationUpdate> {
    let &(key, _) = index.get(&record.probe.get())?;
    let observation = match record.kind {
        StatusKind::LostBlocks => ProbeObservation::Lost {
            count: counts.lost_blocks,
        },
        StatusKind::LostRecords => ProbeObservation::Lost {
            count: counts.lost_records,
        },
        // The probe was removed RT-side — the channel has no binding.
        StatusKind::Removed => ProbeObservation::Missing,
        // Mode off or rebind-dropped: the subscription may still
        // exist, but observation is stopped until re-install —
        // `Disabled`, never `Missing` (a contract binding remains).
        StatusKind::ModeChanged {
            to: RtProbeMode::Off,
        }
        | StatusKind::RebindDropped => ProbeObservation::Disabled,
        _ => return None,
    };
    Some(ProbeObservationUpdate { key, observation })
}

/// The [`ProbeTracePort`] reference implementation: [`RoutingEngine`]
/// probe lifecycle/drain plus node-keyed [`DispatchTraceAdapter`]s
/// for the `observed` slot. Composition roots own publish/run cycles
/// through [`Self::engine_mut`]; this type never calls `publish`
/// itself, so `RebindReceipt`/segment-cut semantics stay exactly on
/// the slice-1 `RoutingEngine::publish → ProbeSession::on_publish`
/// path.
pub struct ProbeTraceBridge {
    engine: RoutingEngine,
    /// Runtime node → the adapter owning that node's `ParamDispatch`
    /// and target traces.
    adapters: BTreeMap<NodeId, DispatchTraceAdapter>,
    /// `ParameterTargetId` → owning adapter node. Registered
    /// explicitly by the composition root at bind time — never
    /// guessed from project topology.
    routes: BTreeMap<ParameterTargetId, NodeId>,
}

impl ProbeTraceBridge {
    /// Wraps `engine` with `adapters` keyed by their runtime node.
    #[must_use]
    pub fn new(engine: RoutingEngine, adapters: BTreeMap<NodeId, DispatchTraceAdapter>) -> Self {
        Self {
            engine,
            adapters,
            routes: BTreeMap::new(),
        }
    }

    /// Shared engine access — read projections (`source_map`,
    /// `probe_bindings`, …).
    #[must_use]
    pub fn engine(&self) -> &RoutingEngine {
        &self.engine
    }

    /// Mutable engine access — the composition root drives
    /// `prepare`/`publish`/`run`/`reconcile` here. Probe lifecycle
    /// must go through the [`ProbeTracePort`] impl (or
    /// [`Self::probe_request`] & friends) so session correlation and
    /// drain ownership stay single-sourced.
    #[must_use]
    pub fn engine_mut(&mut self) -> &mut RoutingEngine {
        &mut self.engine
    }

    /// Routes `target`'s probe observations to the adapter owned by
    /// `node`. Replaces any previous route for `target`.
    pub fn route_target(&mut self, target: ParameterTargetId, node: NodeId) {
        self.routes.insert(target, node);
    }

    /// Drops `target`'s trace route — later `record_probe` calls for
    /// it return `false` (visible unrouted drop).
    pub fn unroute_target(&mut self, target: ParameterTargetId) -> bool {
        self.routes.remove(&target).is_some()
    }

    /// The adapter registered for `node`, if any.
    #[must_use]
    pub fn adapter(&self, node: NodeId) -> Option<&DispatchTraceAdapter> {
        self.adapters.get(&node)
    }

    /// Mutable adapter access for `observe_cycle`/dispatch driving —
    /// the composition root's non-probe trace path.
    #[must_use]
    pub fn adapter_mut(&mut self, node: NodeId) -> Option<&mut DispatchTraceAdapter> {
        self.adapters.get_mut(&node)
    }
}

impl ProbeTracePort for ProbeTraceBridge {
    fn probe_request(
        &mut self,
        project: &Project,
        key: ProbeSubscriptionKey,
        mode: ProbeMode,
    ) -> Result<(), ProbePortError> {
        self.engine
            .probe_request(project, key, mode)
            .map_err(port_error)
    }

    fn probe_set_mode(
        &mut self,
        key: ProbeSubscriptionKey,
        mode: ProbeMode,
    ) -> Result<(), ProbePortError> {
        self.engine.probe_set_mode(key, mode).map_err(port_error)
    }

    fn probe_capture(
        &mut self,
        key: ProbeSubscriptionKey,
        max_frames: u64,
    ) -> Result<(), ProbePortError> {
        self.engine
            .probe_capture(key, max_frames)
            .map_err(port_error)
    }

    fn probe_stop_capture(&mut self, key: ProbeSubscriptionKey) -> Result<(), ProbePortError> {
        self.engine.probe_stop_capture(key).map_err(port_error)
    }

    fn probe_remove(&mut self, key: ProbeSubscriptionKey) -> Result<(), ProbePortError> {
        self.engine.probe_remove(key).map_err(port_error)
    }

    fn probe_drain(&mut self) -> Vec<ProbeObservationUpdate> {
        // Snapshot before draining: the ledger is updated *inside*
        // `probe_drain` per event, but `active_plan_generation` only
        // moves on publish — stable across this call.
        let index = probe_index(&self.engine);
        let counts = self.engine.probe_counts();
        let mut updates = Vec::new();
        self.engine.probe_drain(|event| match event {
            ProbeEvent::Metric(record) => {
                if let Some(update) = metric_update(&record, &index) {
                    updates.push(update);
                }
            }
            ProbeEvent::Status(record) => {
                if let Some(update) = status_update(&record, &index, &counts) {
                    updates.push(update);
                }
            }
            // Captured PCM and unknown future variants carry no
            // slot-level observation — segment ledger owns PCM.
            _ => {}
        });
        updates
    }

    fn probe_bindings(&self) -> &BTreeMap<ProbeSubscriptionKey, SemanticProbeBinding> {
        self.engine.probe_bindings()
    }

    fn probe_segments(&self) -> &[ProbeCaptureSegment] {
        self.engine.probe_segments()
    }

    fn record_probe(&mut self, target: ParameterTargetId, observation: ProbeObservation) -> bool {
        let Some(node) = self.routes.get(&target) else {
            return false;
        };
        let Some(adapter) = self.adapters.get_mut(node) else {
            return false;
        };
        adapter.record_probe(target, observation);
        true
    }

    fn trace(&self, target: ParameterTargetId) -> Option<&ControlTrace> {
        let node = self.routes.get(&target)?;
        self.adapters.get(node)?.trace(target)
    }

    fn traces(&self) -> Vec<(ParameterTargetId, ControlTrace)> {
        let mut out = Vec::new();
        for adapter in self.adapters.values() {
            out.extend(adapter.traces());
        }
        out
    }
}
