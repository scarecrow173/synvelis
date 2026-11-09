//! Semantic probe runtime adapter (Issue #107, SF-04 slice 1).
//!
//! [`ProbeSession`] owns the non-RT halves of Ruviform's probe
//! machinery — [`ProbeBroker`] (control plane), [`ProbeCollector`]
//! (telemetry/capture receive) and the Synvelis-side binding/segment
//! ledgers — while the RT [`ProbeDeck`] is installed into
//! [`RtRuntime`] by the owning [`RoutingEngine`]. It translates stable
//! [`SemanticProbePoint`]s into runtime [`ProbePoint`]s through the
//! generation-scoped [`RoutingSourceMap`] + published [`GraphSpec`]
//! ([`resolve_probe_point`]), and keeps requested/queued state strictly
//! separate from RT-observed state.
//!
//! Nothing here re-implements Ruviform's generic probe, graph-swap
//! rebind (#308) or clock/alignment (#309) mechanisms: capture
//! continuation policy stays at the broker default
//! (`CapturePolicy::StopAtBoundary`), generation guards live in the
//! deck, and clock identity is carried by `RecordMeta`/`StatusRecord`.
//! The adapter's own bookkeeping is bounded: one ledger entry per
//! subscription key, one open segment per capturing probe.

use std::collections::BTreeMap;
use std::fmt;

use ruviform_audio::typed::GraphRuntime;
use ruviform_audio::typed::probe::{
    self, CaptureConfig, PointAvailability, ProbeBroker, ProbeCollector, ProbeConfig, ProbeDeck,
    ProbeError, ProbeEvent, ProbeId, ProbeMode as RtProbeMode, ProbePoint as RtProbePoint,
    RebindReceipt, RecordMeta, StatusKind, StatusRecord, StopReason,
};
use ruviform_graph::GraphSpec;

use synvelis_project::probe::{
    CaptureCompleteness, CaptureSegmentEnd, CaptureSessionId, ClockContextRef, ObservationGap,
    ObservationGapKind, ProbeCaptureSegment, ProbeFrameInterval, ProbeMode, ProbePointResolution,
    ProbeResolveState, ProbeStatusKind, ProbeStatusReceipt, ProbeStopReason, ProbeSubscriptionKey,
    ProbeUnobservedReason, RuntimeProbeId, SemanticProbeBinding, SemanticProbePoint,
    resolve_probe_point,
};
use synvelis_project::{Project, RoutingSourceMap};

/// Non-RT probe session: broker + collector + binding/segment ledgers.
pub struct ProbeSession {
    broker: ProbeBroker,
    collector: ProbeCollector,
    /// Subscription ledger — the contract-facing binding records.
    bindings: BTreeMap<ProbeSubscriptionKey, SemanticProbeBinding>,
    /// Subscription → broker probe id (runtime-only identity).
    probe_ids: BTreeMap<ProbeSubscriptionKey, ProbeId>,
    /// Broker probe id → subscription (telemetry routing).
    by_probe: BTreeMap<ProbeId, ProbeSubscriptionKey>,
    /// Closed capture segments.
    segments: Vec<ProbeCaptureSegment>,
    /// In-flight segment accumulators keyed by probe id.
    open: BTreeMap<ProbeId, OpenSegment>,
    /// Next capture session id (adapter-minted, session-scoped).
    next_capture_session: u64,
    /// Frames per RT block — sizes gap intervals recorded from
    /// per-block status positions.
    block_frames: u64,
}

/// Accumulator for one in-flight capture segment — flushed to
/// [`ProbeCaptureSegment`] when RT status closes it.
struct OpenSegment {
    capture_session_id: CaptureSessionId,
    segment_index: u64,
    point: SemanticProbePoint,
    generation: u64,
    clock: Option<ClockContextRef>,
    start: Option<u64>,
    end: u64,
    has_data: bool,
    gaps: Vec<ObservationGap>,
}

/// Failures of the probe control plane — defined, never a panic or a
/// silent no-op.
#[non_exhaustive]
#[derive(Debug)]
pub enum ProbeRequestError {
    /// The subscription key is not installed.
    UnknownKey,
    /// The binding has no runtime probe (unresolved/unavailable/
    /// unsupported resolution or failed install).
    NotBound,
    /// A capture segment is already open for this probe — an explicit
    /// `stop_capture` or generation cut is required first.
    AlreadyCapturing,
    /// The live project does not match the published plan's project.
    ProjectMismatch,
    /// No published plan/source map exists to resolve against.
    NoActivePlan,
    /// Broker/control-plane failure (channel full, capacity, budget).
    Broker(ProbeError),
}

impl fmt::Display for ProbeRequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownKey => f.write_str("probe subscription not installed"),
            Self::NotBound => f.write_str("probe binding has no runtime probe"),
            Self::AlreadyCapturing => f.write_str("probe already has an open capture segment"),
            Self::ProjectMismatch => f.write_str("project does not match the published plan"),
            Self::NoActivePlan => f.write_str("no published plan to resolve against"),
            Self::Broker(e) => write!(f, "probe broker error: {e}"),
        }
    }
}

impl std::error::Error for ProbeRequestError {}

impl From<ProbeError> for ProbeRequestError {
    fn from(e: ProbeError) -> Self {
        Self::Broker(e)
    }
}

/// Project-side [`ProbeMode`] → runtime mode (1:1 vocabulary). A mode
/// the contract cannot represent fails closed to `Off` — no
/// observation is ever guessed.
fn rt_mode(mode: ProbeMode) -> RtProbeMode {
    match mode {
        ProbeMode::Monitor => RtProbeMode::Monitor,
        ProbeMode::Capture => RtProbeMode::Capture,
        _ => RtProbeMode::Off,
    }
}

/// Runtime mode → project-side [`ProbeMode`] (`None` for variants the
/// contract cannot represent — never guessed).
fn project_mode(mode: RtProbeMode) -> Option<ProbeMode> {
    match mode {
        RtProbeMode::Off => Some(ProbeMode::Off),
        RtProbeMode::Monitor => Some(ProbeMode::Monitor),
        RtProbeMode::Capture => Some(ProbeMode::Capture),
        _ => None,
    }
}

/// `ProbeClockMark` → contract clock context (`None` while the series
/// is unwired/unknown — unknown never compares equal).
pub(crate) fn clock_ref(
    mark: ruviform_audio::typed::probe::ProbeClockMark,
) -> Option<ClockContextRef> {
    (!mark.is_unknown()).then_some(ClockContextRef {
        domain: mark.domain,
        epoch: mark.epoch,
    })
}

/// `ProbePoint` lacks `PartialEq` — compare the identity fields.
fn same_point(a: RtProbePoint, b: RtProbePoint) -> bool {
    a.node == b.node && a.port == b.port
}

/// `PointAvailability` → contract resolve state — the broker's
/// physical-slot verdict mirrored without invention.
fn availability_state(availability: PointAvailability) -> ProbeResolveState {
    match availability {
        PointAvailability::Unsupported => ProbeResolveState::Unsupported,
        PointAvailability::Unknown => ProbeResolveState::Unresolved,
        _ => ProbeResolveState::Unavailable,
    }
}

/// Runtime [`StopReason`] → contract [`ProbeStopReason`] — future
/// variants land on `Unknown`, never on a guessed cause.
fn stop_reason(reason: StopReason) -> ProbeStopReason {
    match reason {
        StopReason::Requested => ProbeStopReason::Requested,
        StopReason::BudgetExhausted => ProbeStopReason::BudgetExhausted,
        StopReason::GenerationBoundary => ProbeStopReason::GenerationBoundary,
        _ => ProbeStopReason::Unknown,
    }
}

/// Runtime [`UnobservedReason`](ruviform_audio::typed::probe::UnobservedReason)
/// → contract reason — future variants land on `Unknown`.
fn unobserved_reason(
    reason: ruviform_audio::typed::probe::UnobservedReason,
) -> ProbeUnobservedReason {
    match reason {
        ruviform_audio::typed::probe::UnobservedReason::Stale => ProbeUnobservedReason::Stale,
        ruviform_audio::typed::probe::UnobservedReason::BorrowConflict => {
            ProbeUnobservedReason::BorrowConflict
        }
        _ => ProbeUnobservedReason::Unknown,
    }
}

/// Runtime [`StatusKind`] → contract [`ProbeStatusKind`].
fn status_kind(kind: StatusKind) -> ProbeStatusKind {
    match kind {
        StatusKind::Installed => ProbeStatusKind::Installed,
        StatusKind::ModeChanged { to } => ProbeStatusKind::ModeChanged {
            to: project_mode(to),
        },
        StatusKind::CaptureStarted { budget_frames } => {
            ProbeStatusKind::CaptureStarted { budget_frames }
        }
        StatusKind::CaptureStopped { reason } => ProbeStatusKind::CaptureStopped {
            reason: stop_reason(reason),
        },
        StatusKind::Unbound => ProbeStatusKind::Unbound,
        StatusKind::Unobserved { reason } => ProbeStatusKind::Unobserved {
            reason: unobserved_reason(reason),
        },
        StatusKind::LostBlocks => ProbeStatusKind::LostBlocks,
        StatusKind::LostRecords => ProbeStatusKind::LostRecords,
        StatusKind::Removed => ProbeStatusKind::Removed,
        StatusKind::RebindDropped => ProbeStatusKind::RebindDropped,
        _ => ProbeStatusKind::Unknown,
    }
}

/// Runtime [`StopReason`] → segment end marker.
fn segment_end(reason: StopReason) -> CaptureSegmentEnd {
    match reason {
        StopReason::Requested => CaptureSegmentEnd::Requested,
        StopReason::BudgetExhausted => CaptureSegmentEnd::BudgetExhausted,
        StopReason::GenerationBoundary => CaptureSegmentEnd::GenerationBoundary,
        _ => CaptureSegmentEnd::Unknown,
    }
}

impl ProbeSession {
    /// Builds the broker/collector halves of a fresh probe `pair` and
    /// returns them with the RT [`ProbeDeck`] to install via
    /// `RtRuntime::set_probe_deck` before the runtime goes live.
    /// `block_frames` sizes recorded gap intervals; `sample_rate`
    /// declares the cursor series' frame unit for clock alignment.
    #[must_use]
    pub fn pair(config: &ProbeConfig, block_frames: u64, sample_rate: u32) -> (Self, ProbeDeck) {
        let parts = probe::pair(config);
        let mut session = Self {
            broker: parts.broker,
            collector: parts.collector,
            bindings: BTreeMap::new(),
            probe_ids: BTreeMap::new(),
            by_probe: BTreeMap::new(),
            segments: Vec::new(),
            open: BTreeMap::new(),
            next_capture_session: 1,
            block_frames,
        };
        session.broker.time_context_mut().sample_rate = sample_rate;
        // Best-effort: an unwired series stays UNKNOWN-marked, which
        // is already fail-closed for alignment.
        let _ = session.broker.wire_clock_mark();
        (session, parts.deck)
    }

    /// Requests observation of `key.point` in `mode`. Idempotent per
    /// key. Resolution is fail-closed: unresolvable/unsupported points
    /// are recorded on the binding (`Unresolved`/`Unavailable`/
    /// `Unsupported`) without touching the broker — no fake PCM.
    ///
    /// # Errors
    /// [`ProbeRequestError::Broker`] on install failure
    /// (capacity/channel-full).
    pub fn request(
        &mut self,
        runtime: &GraphRuntime,
        spec: &GraphSpec,
        project: &Project,
        map: &RoutingSourceMap,
        key: ProbeSubscriptionKey,
        mode: ProbeMode,
    ) -> Result<(), ProbeRequestError> {
        // Idempotent per live key — a `Stopped` record may be
        // re-requested (the stale entry is overwritten below).
        if self
            .bindings
            .get(&key)
            .is_some_and(|b| b.state != ProbeResolveState::Stopped)
        {
            return Ok(());
        }
        let mut binding = SemanticProbeBinding {
            point: key.point,
            project_revision: map.project_revision,
            runtime_instance_id: map.build_key.runtime_instance,
            active_plan_generation: map.generation,
            source_map_fingerprint: map.build_key.routing_fingerprint,
            runtime_probe_id: None,
            requested_mode: mode,
            confirmed_mode: None,
            state: ProbeResolveState::Requested,
            last_status: None,
        };
        match resolve_probe_point(project, map, spec, &key.point) {
            ProbePointResolution::Resolved(port_ref) => {
                let point = RtProbePoint {
                    node: port_ref.node,
                    port: port_ref.port,
                };
                match self.broker.install(runtime, spec, point, rt_mode(mode)) {
                    Ok(probe) => {
                        binding.runtime_probe_id = Some(RuntimeProbeId(probe.get()));
                        binding.state = ProbeResolveState::QueuedForBinding;
                        self.probe_ids.insert(key, probe);
                        self.by_probe.insert(probe, key);
                    }
                    Err(ProbeError::PointUnavailable { availability }) => {
                        binding.state = availability_state(availability);
                    }
                    Err(e) => return Err(e.into()),
                }
            }
            ProbePointResolution::Unresolved => {
                binding.state = ProbeResolveState::Unresolved;
            }
            ProbePointResolution::Unavailable | ProbePointResolution::NotLowered(_) => {
                binding.state = ProbeResolveState::Unavailable;
            }
            ProbePointResolution::Unsupported => {
                binding.state = ProbeResolveState::Unsupported;
            }
            // A resolution outcome the contract cannot represent —
            // fail closed, never bound.
            _ => binding.state = ProbeResolveState::Unavailable,
        }
        self.bindings.insert(key, binding);
        Ok(())
    }

    /// Requests a mode change. `requested_mode` updates only after the
    /// command is accepted — RT confirmation arrives via `drain`.
    ///
    /// # Errors
    /// [`ProbeRequestError::UnknownKey`]/[`ProbeRequestError::NotBound`]/
    /// [`ProbeRequestError::Broker`].
    pub fn set_mode(
        &mut self,
        key: ProbeSubscriptionKey,
        mode: ProbeMode,
    ) -> Result<(), ProbeRequestError> {
        let probe = self.bound_probe(key)?;
        self.broker.set_mode(probe, rt_mode(mode))?;
        if let Some(binding) = self.bindings.get_mut(&key) {
            binding.requested_mode = mode;
        }
        Ok(())
    }

    /// Starts a bounded capture — an explicit new capture session.
    /// Refused while a segment is already open for the probe.
    ///
    /// # Errors
    /// [`ProbeRequestError::UnknownKey`]/[`ProbeRequestError::NotBound`]/
    /// [`ProbeRequestError::AlreadyCapturing`]/[`ProbeRequestError::Broker`].
    pub fn capture(
        &mut self,
        key: ProbeSubscriptionKey,
        max_frames: u64,
    ) -> Result<(), ProbeRequestError> {
        let probe = self.bound_probe(key)?;
        if self.open.contains_key(&probe) {
            return Err(ProbeRequestError::AlreadyCapturing);
        }
        self.broker.capture(probe, CaptureConfig { max_frames })?;
        let (point, generation) = self
            .bindings
            .get(&key)
            .map(|b| (b.point, b.active_plan_generation))
            .ok_or(ProbeRequestError::UnknownKey)?;
        let capture_session_id = CaptureSessionId(self.next_capture_session);
        self.next_capture_session += 1;
        self.open.insert(
            probe,
            OpenSegment {
                capture_session_id,
                segment_index: 0,
                point,
                generation,
                clock: None,
                start: None,
                end: 0,
                has_data: false,
                gaps: Vec::new(),
            },
        );
        if let Some(binding) = self.bindings.get_mut(&key) {
            binding.requested_mode = ProbeMode::Capture;
        }
        Ok(())
    }

    /// Stops an in-flight capture (returns to `Monitor` on the RT
    /// side). The open segment closes only on the RT
    /// `CaptureStopped` status — requested is never treated as done.
    ///
    /// # Errors
    /// [`ProbeRequestError::UnknownKey`]/[`ProbeRequestError::NotBound`]/
    /// [`ProbeRequestError::Broker`].
    pub fn stop_capture(&mut self, key: ProbeSubscriptionKey) -> Result<(), ProbeRequestError> {
        let probe = self.bound_probe(key)?;
        self.broker.stop_capture(probe)?;
        if let Some(binding) = self.bindings.get_mut(&key) {
            binding.requested_mode = ProbeMode::Monitor;
        }
        Ok(())
    }

    /// Removes the probe entirely. The subscription record stays as
    /// `Stopped` lifecycle evidence; any open segment closes with
    /// `Removed` — the RT `Removed` status remains the confirmation.
    ///
    /// # Errors
    /// [`ProbeRequestError::UnknownKey`]/[`ProbeRequestError::Broker`].
    pub fn remove(&mut self, key: ProbeSubscriptionKey) -> Result<(), ProbeRequestError> {
        if !self.bindings.contains_key(&key) {
            return Err(ProbeRequestError::UnknownKey);
        }
        if let Some(probe) = self.probe_ids.remove(&key) {
            self.broker.remove(probe)?;
            self.by_probe.remove(&probe);
            if let Some(segment) = self.open.remove(&probe) {
                self.segments.push(close_segment(
                    segment,
                    Some(CaptureSegmentEnd::Removed),
                    None,
                ));
            }
        }
        if let Some(binding) = self.bindings.get_mut(&key) {
            binding.runtime_probe_id = None;
            binding.state = ProbeResolveState::Stopped;
        }
        Ok(())
    }

    /// Rebinds every installed probe against `runtime`'s newly
    /// published plan — call after every successful publish, with the
    /// *new* source map and spec. Each stable [`SemanticProbePoint`]
    /// is re-resolved through them first: the staged rebind alone
    /// would keep a probe's old runtime point, which can silently
    /// observe a different signal after a topology change. A moved
    /// point is replaced (remove + install — the broker cannot
    /// retarget); a vanished point lands in `receipt.unbound` →
    /// `StaleGeneration` + RT `RebindDropped`. Capture never restarts
    /// implicitly: `StopAtBoundary` (broker default) cuts in-flight
    /// segments at the generation boundary and a re-installed probe
    /// comes back `Monitor` even when `Capture` was requested — a new
    /// capture is always an explicit new session.
    ///
    /// # Errors
    /// [`ProbeError::CommandChannelFull`] — bindings keep their
    /// previous generation-guarded state (RT emits `Unbound` →
    /// `StaleGeneration`); safe to retry after the RT side drains.
    pub fn on_publish(
        &mut self,
        runtime: &GraphRuntime,
        spec: &GraphSpec,
        project: &Project,
        map: &RoutingSourceMap,
    ) -> Result<RebindReceipt, ProbeError> {
        let keys: Vec<ProbeSubscriptionKey> = self.bindings.keys().copied().collect();
        for key in keys {
            self.reresolve_on_publish(runtime, spec, project, map, key)?;
        }
        let receipt = self.broker.on_publish(runtime, spec)?;
        for &probe in &receipt.bound {
            if let Some(&key) = self.by_probe.get(&probe)
                && let Some(binding) = self.bindings.get_mut(&key)
            {
                binding.active_plan_generation = receipt.generation;
                binding.state = ProbeResolveState::PendingRtInstall;
            }
        }
        for &probe in &receipt.unbound {
            if let Some(&key) = self.by_probe.get(&probe)
                && let Some(binding) = self.bindings.get_mut(&key)
            {
                binding.state = ProbeResolveState::StaleGeneration;
            }
        }
        Ok(receipt)
    }

    /// Re-resolves one subscription's semantic point through the new
    /// generation's map/spec and replaces the runtime probe when the
    /// resolved point moved. Unresolvable points are left for the
    /// staged rebind to report as `unbound`.
    fn reresolve_on_publish(
        &mut self,
        runtime: &GraphRuntime,
        spec: &GraphSpec,
        project: &Project,
        map: &RoutingSourceMap,
        key: ProbeSubscriptionKey,
    ) -> Result<(), ProbeError> {
        let Some(&probe) = self.probe_ids.get(&key) else {
            return Ok(());
        };
        let (point, requested) = {
            let Some(binding) = self.bindings.get(&key) else {
                return Ok(());
            };
            (binding.point, binding.requested_mode)
        };
        let resolution = resolve_probe_point(project, map, spec, &point);
        let desired = if let ProbePointResolution::Resolved(port_ref) = &resolution {
            RtProbePoint {
                node: port_ref.node,
                port: port_ref.port,
            }
        } else {
            // The semantic point no longer resolves — the probe
            // must not keep observing a stale runtime slot under
            // the new generation. Remove it; the staged rebind
            // never sees it.
            self.broker.remove(probe)?;
            self.probe_ids.remove(&key);
            self.by_probe.remove(&probe);
            if let Some(segment) = self.open.remove(&probe) {
                self.segments.push(close_segment(
                    segment,
                    Some(CaptureSegmentEnd::GenerationBoundary),
                    None,
                ));
            }
            if let Some(binding) = self.bindings.get_mut(&key) {
                binding.runtime_probe_id = None;
                binding.state = match &resolution {
                    ProbePointResolution::Unresolved => ProbeResolveState::Unresolved,
                    ProbePointResolution::Unsupported => ProbeResolveState::Unsupported,
                    // Unavailable / NotLowered / any outcome the
                    // contract cannot represent — fail closed.
                    _ => ProbeResolveState::Unavailable,
                };
            }
            return Ok(());
        };
        if self
            .broker
            .point(probe)
            .is_some_and(|current| same_point(current, desired))
        {
            // Point unchanged — the staged rebind handles it.
            return Ok(());
        }
        // The semantic point moved (or the broker lost the probe) —
        // replace it. An in-flight segment closes at the generation
        // boundary (the remove preempts the RT `CaptureStopped`, so
        // the adapter records the cut itself).
        if self.broker.point(probe).is_some() {
            self.broker.remove(probe)?;
        }
        self.probe_ids.remove(&key);
        self.by_probe.remove(&probe);
        if let Some(segment) = self.open.remove(&probe) {
            self.segments.push(close_segment(
                segment,
                Some(CaptureSegmentEnd::GenerationBoundary),
                None,
            ));
        }
        // Capture never restarts implicitly across a rebind — the
        // requested mode is preserved on the binding while the new
        // probe installs `Monitor`.
        let mode = match requested {
            ProbeMode::Capture => ProbeMode::Monitor,
            mode => mode,
        };
        match self.broker.install(runtime, spec, desired, rt_mode(mode)) {
            Ok(new_probe) => {
                self.probe_ids.insert(key, new_probe);
                self.by_probe.insert(new_probe, key);
                if let Some(binding) = self.bindings.get_mut(&key) {
                    binding.runtime_probe_id = Some(RuntimeProbeId(new_probe.get()));
                    binding.active_plan_generation = map.generation;
                    binding.state = ProbeResolveState::QueuedForBinding;
                }
            }
            Err(ProbeError::PointUnavailable { availability }) => {
                if let Some(binding) = self.bindings.get_mut(&key) {
                    binding.runtime_probe_id = None;
                    binding.state = availability_state(availability);
                }
            }
            Err(e) => return Err(e),
        }
        Ok(())
    }

    /// Drains collector telemetry/chunks, updating binding states and
    /// open segments from RT evidence *before* invoking `f` for each
    /// event. Returns the number of events drained.
    pub fn drain(&mut self, mut f: impl FnMut(ProbeEvent<'_>)) -> usize {
        let Self {
            collector,
            bindings,
            by_probe,
            open,
            segments,
            block_frames,
            ..
        } = self;
        collector.drain(|event| {
            observe_event(&event, bindings, by_probe, open, segments, *block_frames);
            f(event);
        })
    }

    /// All subscription bindings (contract view).
    #[must_use]
    pub fn bindings(&self) -> &BTreeMap<ProbeSubscriptionKey, SemanticProbeBinding> {
        &self.bindings
    }

    /// Closed capture segments, in close order.
    #[must_use]
    pub fn segments(&self) -> &[ProbeCaptureSegment] {
        &self.segments
    }

    /// Broker/collector loss counters.
    #[must_use]
    pub fn counts(&self) -> probe::ProbeCounts {
        self.collector.counts()
    }

    /// The broker's non-RT time-series context (clock domain/epoch,
    /// runtime instance, generation window).
    #[must_use]
    pub fn time_context(&self) -> &probe::ProbeTimeContext {
        self.broker.time_context()
    }

    /// Declares a cursor-series discontinuity (transport seek) —
    /// bumps the clock epoch so records after the boundary classify
    /// as `EpochMismatch`/`DiscontinuityInWindow` instead of silently
    /// aligning with pre-discontinuity spans (Ruviform #309). Open
    /// capture segments are *not* closed here: they cut lazily when
    /// the first new-epoch evidence drains, so every pre-discontinuity
    /// chunk still lands in its own segment. The RT probe keeps its
    /// mode — post-cut chunks carry the new epoch, match no open
    /// segment and are discarded (never re-attributed, never
    /// auto-recaptured).
    ///
    /// # Errors
    /// [`ProbeRequestError::Broker`] on `CommandChannelFull` — the
    /// broker epoch still advances (upstream bumps before the push),
    /// so `time_context().epoch` and the deck's mark disagree until a
    /// later `wire_clock_mark`/`bump_epoch` lands; observable, never
    /// silently aligned.
    pub fn bump_clock_epoch(&mut self) -> Result<(), ProbeRequestError> {
        self.broker.bump_epoch()?;
        Ok(())
    }

    /// Runtime probe id for an installed binding.
    fn bound_probe(&self, key: ProbeSubscriptionKey) -> Result<ProbeId, ProbeRequestError> {
        self.probe_ids
            .get(&key)
            .copied()
            .ok_or(ProbeRequestError::NotBound)
    }
}

/// Applies one drained event to the binding ledger and open segments.
/// Runs inside `collector.drain` — non-RT side only.
fn observe_event(
    event: &ProbeEvent<'_>,
    bindings: &mut BTreeMap<ProbeSubscriptionKey, SemanticProbeBinding>,
    by_probe: &BTreeMap<ProbeId, ProbeSubscriptionKey>,
    open: &mut BTreeMap<ProbeId, OpenSegment>,
    segments: &mut Vec<ProbeCaptureSegment>,
    block_frames: u64,
) {
    match event {
        ProbeEvent::Status(status) => {
            observe_status(*status, bindings, by_probe, open, segments, block_frames);
        }
        ProbeEvent::Captured(chunk) => {
            if let Some(meta) = chunk.meta {
                observe_meta(meta, open, segments);
            }
        }
        _ => {}
    }
}

/// Status evidence → binding state + segment bookkeeping.
fn observe_status(
    status: StatusRecord,
    bindings: &mut BTreeMap<ProbeSubscriptionKey, SemanticProbeBinding>,
    by_probe: &BTreeMap<ProbeId, ProbeSubscriptionKey>,
    open: &mut BTreeMap<ProbeId, OpenSegment>,
    segments: &mut Vec<ProbeCaptureSegment>,
    block_frames: u64,
) {
    let Some(&key) = by_probe.get(&status.probe) else {
        return;
    };
    let Some(binding) = bindings.get_mut(&key) else {
        return;
    };
    binding.last_status = Some(ProbeStatusReceipt {
        kind: status_kind(status.kind),
        generation: status.generation,
        sample_position: status.sample_position,
        clock: clock_ref(status.clock),
    });
    // Clock-epoch discontinuity: evidence stamped under a new cursor
    // epoch cuts the open segment before any gap bookkeeping below —
    // post-epoch loss/unobserved reports must not land inside the old
    // segment. Closing kinds keep their own end reason.
    if !matches!(
        status.kind,
        StatusKind::CaptureStopped { .. } | StatusKind::Removed | StatusKind::RebindDropped
    ) && let Some(clock) = clock_ref(status.clock)
    {
        cut_segment_on_epoch_change(open, segments, status.probe, clock);
    }
    match status.kind {
        StatusKind::Installed => {
            // Generation-guarded: only the active generation's install
            // confirms `BoundActive`; anything else is stale evidence.
            binding.state = if status.generation == binding.active_plan_generation {
                ProbeResolveState::BoundActive
            } else {
                ProbeResolveState::StaleGeneration
            };
        }
        StatusKind::ModeChanged { to } => {
            binding.confirmed_mode = project_mode(to);
        }
        StatusKind::CaptureStarted { .. } => {
            binding.confirmed_mode = Some(ProbeMode::Capture);
        }
        StatusKind::CaptureStopped { reason } => {
            binding.confirmed_mode = Some(ProbeMode::Monitor);
            if let Some(segment) = open.remove(&status.probe) {
                segments.push(close_segment(
                    segment,
                    Some(segment_end(reason)),
                    clock_ref(status.clock),
                ));
            }
        }
        StatusKind::Unbound => {
            binding.state = ProbeResolveState::StaleGeneration;
        }
        StatusKind::Unobserved { .. } => {
            record_gap(
                open,
                status.probe,
                status.sample_position,
                block_frames,
                ObservationGapKind::Unobserved,
            );
        }
        StatusKind::LostBlocks => {
            record_gap(
                open,
                status.probe,
                status.sample_position,
                block_frames,
                ObservationGapKind::LostBlocks,
            );
        }
        StatusKind::LostRecords => {
            record_gap(
                open,
                status.probe,
                status.sample_position,
                block_frames,
                ObservationGapKind::LostRecords,
            );
        }
        StatusKind::Removed => {
            binding.state = ProbeResolveState::Stopped;
            if let Some(segment) = open.remove(&status.probe) {
                segments.push(close_segment(
                    segment,
                    Some(CaptureSegmentEnd::Removed),
                    clock_ref(status.clock),
                ));
            }
        }
        StatusKind::RebindDropped => {
            binding.state = ProbeResolveState::Lost;
            if let Some(segment) = open.remove(&status.probe) {
                segments.push(close_segment(
                    segment,
                    Some(CaptureSegmentEnd::RebindDropped),
                    clock_ref(status.clock),
                ));
            }
        }
        _ => {}
    }
}

/// Captured PCM chunk metadata → open-segment frame range. Only
/// records whose `meta.generation` matches the segment's generation
/// accumulate — stale-generation records are never claimed as current
/// data. A record whose clock epoch differs from the segment's cuts
/// the segment at the discontinuity (`ClockEpoch`) and is itself
/// discarded: post-epoch data never joins the old segment and no new
/// segment starts implicitly.
fn observe_meta(
    meta: RecordMeta,
    open: &mut BTreeMap<ProbeId, OpenSegment>,
    segments: &mut Vec<ProbeCaptureSegment>,
) {
    let Some(segment) = open.get(&meta.probe) else {
        return;
    };
    if meta.generation != segment.generation {
        return;
    }
    if let Some(clock) = clock_ref(meta.clock) {
        cut_segment_on_epoch_change(open, segments, meta.probe, clock);
        if !open.contains_key(&meta.probe) {
            return;
        }
    }
    let Some(segment) = open.get_mut(&meta.probe) else {
        return;
    };
    let end = meta.sample_position.saturating_add(u64::from(meta.frames));
    segment.start = Some(
        segment
            .start
            .map_or(meta.sample_position, |s| s.min(meta.sample_position)),
    );
    segment.end = segment.end.max(end);
    segment.has_data = true;
    if segment.clock.is_none() {
        segment.clock = clock_ref(meta.clock);
    }
}

/// Closes `probe`'s open segment at a cursor clock-epoch change —
/// the segment keeps its old-epoch `clock_context` and the triggering
/// record is the caller's to discard. No-op while the segment has no
/// known clock yet (unwired series stays honestly unknown) or the
/// epoch already matches.
fn cut_segment_on_epoch_change(
    open: &mut BTreeMap<ProbeId, OpenSegment>,
    segments: &mut Vec<ProbeCaptureSegment>,
    probe: ProbeId,
    clock: ClockContextRef,
) {
    let Some(segment) = open.get(&probe) else {
        return;
    };
    let Some(old) = segment.clock else {
        return;
    };
    if old == clock {
        return;
    }
    let segment = open.remove(&probe).expect("segment presence checked above");
    segments.push(close_segment(
        segment,
        Some(CaptureSegmentEnd::ClockEpoch),
        Some(clock),
    ));
}

/// Records a per-block gap at `position` inside the open segment.
fn record_gap(
    open: &mut BTreeMap<ProbeId, OpenSegment>,
    probe: ProbeId,
    position: u64,
    block_frames: u64,
    kind: ObservationGapKind,
) {
    let Some(segment) = open.get_mut(&probe) else {
        return;
    };
    segment.gaps.push(ObservationGap {
        range: ProbeFrameInterval {
            start: position,
            end: position.saturating_add(block_frames),
        },
        kind,
    });
}

/// Flushes an open segment into a closed [`ProbeCaptureSegment`].
fn close_segment(
    segment: OpenSegment,
    ended_by: Option<CaptureSegmentEnd>,
    status_clock: Option<ClockContextRef>,
) -> ProbeCaptureSegment {
    let completeness = if !segment.gaps.is_empty() {
        CaptureCompleteness::Incomplete
    } else if segment.has_data {
        CaptureCompleteness::Complete
    } else {
        CaptureCompleteness::Unknown
    };
    ProbeCaptureSegment {
        capture_session_id: segment.capture_session_id,
        segment_index: segment.segment_index,
        point: segment.point,
        active_plan_generation: segment.generation,
        clock_context: segment.clock.or(status_clock),
        frame_range: segment.has_data.then(|| ProbeFrameInterval {
            start: segment.start.unwrap_or(0),
            end: segment.end,
        }),
        discontinuities: segment.gaps,
        completeness,
        ended_by,
    }
}
