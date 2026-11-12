//! Semantic probe contract types and fail-closed resolution
//! (Issue #107, SF-04 slice 1).
//!
//! This module owns the *semantic* side of observation: stable probe
//! points expressed in project vocabulary (`SignalBoundary` +
//! [`SemanticSignalEntityRef`] + optional [`SemanticPortId`]), the
//! subscription/binding lifecycle vocabulary, and capture-segment
//! metadata. It deliberately knows nothing about Ruviform's broker,
//! `ProbeId`, or RT scheduling — those are player-layer runtime
//! concerns. Nothing here fabricates PCM: a point that has no physical
//! observation slot resolves to `Unavailable`/`Unsupported`, never to
//! a zero-valued substitute.
//!
//! Resolution (`resolve_probe_point`) is a pure function over the
//! committed [`Project`], the generation-scoped [`RoutingSourceMap`]
//! and the published [`GraphSpec`]. `SignalBoundary` is a DAW semantic
//! concept and does *not* map 1:1 onto Ruviform `TapKind` — the
//! resolved [`PortRef`] is the honest runtime position.

use ruviform_graph::{Direction, GraphSpec, PortFormat, PortRef};
use ruviform_media_core::{ClockDomainId, ClockEpochId};
use serde::{Deserialize, Serialize};

use crate::controller::ProjectRevision;
use crate::id::{BusId, ProcessorId, SemanticPortId, SendId, StripId, TrackId};
use crate::project::Project;
use crate::routing::{PortDirection, PortOwner, PortRole, SignalDomain};
use crate::routing_build::{
    MappingResolution, NotLoweredReason, RoutingFingerprint, RoutingSourceMap, RuntimeInstanceId,
    SemanticNodeRef,
};

/// Where along a signal path a probe observes — a DAW semantic
/// position, not a runtime tap kind.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SignalBoundary {
    /// Before the first insert processor (strip/processor input
    /// summing point).
    PreInsert,
    /// After the last insert processor, before the strip fader
    /// (strip-only position; resolves to the fader node's input).
    PostInsert,
    /// Before the fader gain stage (strip-only; resolves to the
    /// fader's pre-out tap — the same physical point a pre-fader send
    /// taps).
    PreFader,
    /// After the fader gain stage (strip main output).
    PostFader,
    /// The entity's signal input.
    Input,
    /// The entity's signal output.
    Output,
    /// A sidechain/key input (processors only).
    Sidechain,
}

/// The stable semantic entity a probe point observes.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "entity", content = "id", deny_unknown_fields)]
pub enum SemanticSignalEntityRef {
    /// A track (resolved through its channel strip).
    Track(TrackId),
    /// A bus (resolved through its strip).
    Bus(BusId),
    /// A channel strip directly.
    Strip(StripId),
    /// A chain processor.
    Processor(ProcessorId),
    /// A send.
    Send(SendId),
}

/// One stable semantic observation point: which entity, at which
/// boundary, optionally naming one specific semantic port (used to
/// disambiguate e.g. processor sidechain inputs).
///
/// Persistable intent only — runtime `ProbeId`s and graph-local
/// `NodeId`/`PortId`s never appear here.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SemanticProbePoint {
    /// Observed entity.
    pub entity: SemanticSignalEntityRef,
    /// Position along the entity's signal path.
    pub boundary: SignalBoundary,
    /// Optional explicit semantic port — a disambiguator that must
    /// agree with the boundary-selected port, never an arbitrary
    /// override.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub port: Option<SemanticPortId>,
}

/// Session-scoped identity for probe subscriptions — minted by the
/// session/app layer, stable for one engine session, never persisted
/// into project routing data.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectSessionId(pub u64);

/// Identity of one explicit capture engagement — minted per capture
/// request by the player adapter. Segments of one session are cut at
/// generation boundaries; a new capture is always a new session.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CaptureSessionId(pub u64);

/// Mirror of a broker-issued runtime probe id. This is *runtime*
/// state: it is valid only while the owning broker/binding lives and
/// must never be persisted into the project.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RuntimeProbeId(pub u64);

/// Requested observation mode. `Off` is the default — a probe that was
/// never requested does no tap work at all.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProbeMode {
    /// No observation.
    Off,
    /// Per-block Peak/RMS metrics.
    Monitor,
    /// Bounded PCM capture.
    Capture,
}

/// What a subscription consumes — metrics telemetry or captured PCM.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SubscriptionKind {
    /// Peak/RMS metric stream.
    Metrics,
    /// Captured PCM stream.
    Pcm,
}

/// Identity of one probe subscription: session + semantic point +
/// consumption kind. Multiple subscriptions may share one semantic
/// point; aggregation policy is a session-layer concern.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeSubscriptionKey {
    /// Owning session.
    pub session_id: ProjectSessionId,
    /// Semantic observation point.
    pub point: SemanticProbePoint,
    /// Consumption kind.
    pub kind: SubscriptionKind,
}

/// Lifecycle/resolution state of one [`SemanticProbeBinding`].
///
/// `Requested`/`QueuedForBinding`/`PendingRtInstall` are *requested or
/// enqueued* states — never proof of RT installation. Only RT-observed
/// status evidence moves a binding to `BoundActive`.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProbeResolveState {
    /// Accepted by the adapter; resolution or enqueue pending.
    Requested,
    /// The entity/port reference does not exist in the project or map.
    Unresolved,
    /// The position has no physical observation slot in this build.
    Unavailable,
    /// The boundary/entity combination is outside the observable domain.
    Unsupported,
    /// Install command enqueued; awaiting RT application.
    QueuedForBinding,
    /// Rebind staged for a new generation; awaiting RT confirmation.
    PendingRtInstall,
    /// RT-observed installed under the active generation.
    BoundActive,
    /// Last observed state belongs to a superseded generation.
    StaleGeneration,
    /// A generation rebind left this binding in a partial state.
    PartiallyRebound,
    /// The binding was dropped by a committed rebind.
    Lost,
    /// The probe was removed.
    Stopped,
}

/// Why a capture stopped — semantic mirror of the runtime stop reason.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProbeStopReason {
    /// Explicit stop request.
    Requested,
    /// Frame budget exhausted.
    BudgetExhausted,
    /// A generation boundary cut the capture.
    GenerationBoundary,
    /// A stop cause the contract cannot represent (future runtime
    /// variant) — recorded, never guessed.
    Unknown,
}

/// Why a point produced no data for a block.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProbeUnobservedReason {
    /// Slot stamp predates the current callback.
    Stale,
    /// The slot was mutably borrowed at tap time.
    BorrowConflict,
    /// An unobserved cause the contract cannot represent (future
    /// runtime variant) — recorded, never guessed.
    Unknown,
}

/// Semantic mirror of a runtime probe status event — what happened,
/// under which generation, at which cursor position, on which clock.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ProbeStatusKind {
    /// Binding applied on the RT side.
    Installed,
    /// Mode change applied (`None` = runtime mode the contract cannot
    /// represent — never guessed).
    ModeChanged {
        /// New mode.
        to: Option<ProbeMode>,
    },
    /// Capture began.
    CaptureStarted {
        /// Remaining frame budget.
        budget_frames: u64,
    },
    /// Capture ended.
    CaptureStopped {
        /// Why.
        reason: ProbeStopReason,
    },
    /// Generation guard skipped the point this block.
    Unbound,
    /// Freshness/borrow audit rejected the slot.
    Unobserved {
        /// Why.
        reason: ProbeUnobservedReason,
    },
    /// Capture PCM dropped (chunk pool exhausted).
    LostBlocks,
    /// A telemetry record was dropped.
    LostRecords,
    /// Probe removed on the RT side.
    Removed,
    /// Binding dropped by a committed generation rebind.
    RebindDropped,
    /// A status kind the contract cannot represent (future runtime
    /// variant) — recorded, never guessed.
    Unknown,
}

/// Clock identity carried by probe records — `(domain, epoch)` of the
/// frame-cursor series. `ClockDomainId::UNKNOWN` is fail-closed: an
/// unwired series never compares equal to anything.
///
/// Runtime evidence only — media-core clock ids carry no serde, so
/// this type is not persisted.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct ClockContextRef {
    /// Clock domain of the cursor series.
    pub domain: ClockDomainId,
    /// Epoch of the cursor series.
    pub epoch: ClockEpochId,
}

/// RT-observed status evidence for one binding — the *only* proof of
/// installation/observation state. Runtime evidence, not persisted.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeStatusReceipt {
    /// What happened.
    pub kind: ProbeStatusKind,
    /// Plan generation at emit time.
    pub generation: u64,
    /// Absolute cursor position at emit time.
    pub sample_position: u64,
    /// Clock identity of the cursor series (`None` = unwired/unknown).
    pub clock: Option<ClockContextRef>,
}

/// Runtime binding state for one [`ProbeSubscriptionKey`] — the
/// authoritative record of what was requested, what resolved, and
/// what RT confirmed. `requested_mode` and `confirmed_mode` are
/// deliberately distinct: queued is never installed.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticProbeBinding {
    /// Semantic observation point.
    pub point: SemanticProbePoint,
    /// Project revision the binding was requested against.
    pub project_revision: ProjectRevision,
    /// Runtime instance the binding targets.
    pub runtime_instance_id: RuntimeInstanceId,
    /// Active plan generation the binding resolved against.
    pub active_plan_generation: u64,
    /// Fingerprint of the source map the point resolved through.
    pub source_map_fingerprint: RoutingFingerprint,
    /// Broker-issued probe id (`None` until install succeeds).
    pub runtime_probe_id: Option<RuntimeProbeId>,
    /// Last requested mode.
    pub requested_mode: ProbeMode,
    /// RT-confirmed mode (`None` until status evidence arrives).
    pub confirmed_mode: Option<ProbeMode>,
    /// Lifecycle/resolution state.
    pub state: ProbeResolveState,
    /// Latest RT status evidence.
    pub last_status: Option<ProbeStatusReceipt>,
}

/// Half-open frame interval `[start, end)` in probe cursor coordinates.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProbeFrameInterval {
    /// First frame (inclusive).
    pub start: u64,
    /// One past the last frame (exclusive).
    pub end: u64,
}

/// Classification of a discontinuity inside a capture segment.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ObservationGapKind {
    /// The freshness/borrow audit rejected the block.
    Unobserved,
    /// Capture PCM dropped — chunk pool exhausted.
    LostBlocks,
    /// A telemetry record was dropped.
    LostRecords,
    /// A plan-generation boundary fell inside the interval.
    GenerationBoundary,
    /// The binding was dropped by a committed rebind.
    RebindDropped,
    /// The probe was `Off`/removed — no observation by design.
    Off,
    /// The point's capability is unsupported.
    Unsupported,
    /// The point has no backing slot.
    Unavailable,
    /// State cannot be determined — unexplained gaps land here.
    Unknown,
}

/// One classified gap inside a capture segment.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ObservationGap {
    /// Frame interval the gap covers.
    pub range: ProbeFrameInterval,
    /// Gap classification.
    pub kind: ObservationGapKind,
}

/// Whether a closed capture segment's coverage is proven.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptureCompleteness {
    /// Every expected block was observed (measured silence counts).
    Complete,
    /// Known gaps or losses — see `discontinuities`.
    Incomplete,
    /// Coverage cannot be proven (no data or telemetry loss).
    Unknown,
}

/// What closed a capture segment.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CaptureSegmentEnd {
    /// Explicit stop request.
    Requested,
    /// Frame budget exhausted.
    BudgetExhausted,
    /// Generation boundary cut.
    GenerationBoundary,
    /// Binding dropped by rebind.
    RebindDropped,
    /// Probe removed.
    Removed,
    /// The cursor-series clock epoch changed (transport
    /// discontinuity such as a seek) — the segment never spans it.
    ClockEpoch,
    /// A segment end the contract cannot represent (future runtime
    /// variant) — recorded, never guessed.
    Unknown,
}

/// One capture segment — the probe-observation counterpart of a
/// recording segment, *not* a persisted recording-domain entity.
/// A segment never spans a plan-generation boundary or a cursor
/// clock-epoch change: it is cut at the boundary and a new capture
/// requires an explicit new request.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeCaptureSegment {
    /// Owning capture session.
    pub capture_session_id: CaptureSessionId,
    /// Segment index within the session.
    pub segment_index: u64,
    /// Observed semantic point.
    pub point: SemanticProbePoint,
    /// Plan generation this segment belongs to.
    pub active_plan_generation: u64,
    /// Clock identity of the cursor series (`None` = unknown).
    pub clock_context: Option<ClockContextRef>,
    /// Bounding frame interval actually observed (`None` = no data).
    pub frame_range: Option<ProbeFrameInterval>,
    /// Classified gaps inside the segment.
    pub discontinuities: Vec<ObservationGap>,
    /// Proven coverage state.
    pub completeness: CaptureCompleteness,
    /// What closed the segment (`None` while still open — open
    /// segments are not exposed through `probe_segments`).
    pub ended_by: Option<CaptureSegmentEnd>,
}

/// Outcome of [`resolve_probe_point`].
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ProbePointResolution {
    /// Resolved to a runtime `(node, port)` — a candidate for broker
    /// install; physical slot existence is still verified by the
    /// broker against the published plan.
    Resolved(PortRef),
    /// The entity or port reference does not exist.
    Unresolved,
    /// The position has no physical observation slot in this build.
    Unavailable,
    /// The boundary/entity combination is outside the observable
    /// domain (non-audio, unsupported topology).
    Unsupported,
    /// Lowering intentionally produced no runtime object (muted
    /// strip, disabled connection, out of scope) — no physical slot.
    NotLowered(NotLoweredReason),
}

/// The subject a [`SemanticSignalEntityRef`] resolves to.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum ProbeSubject {
    /// A channel strip (track/bus entities resolve to their strip).
    Strip(StripId),
    /// A chain processor.
    Processor(ProcessorId),
    /// A send.
    Send(SendId),
}

/// Which strip-internal graph position a boundary selects — these
/// positions have no semantic port and are resolved through the
/// published spec's fader node.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum StripInternal {
    /// Fader node input — post-chain/pre-fader signal.
    FaderInput,
    /// Fader node's non-main-out output — the pre-fader tap.
    FaderPreOut,
}

/// Boundary-driven port selection before explicit-port validation.
enum PortSelection {
    /// A semantic port resolved through `map.ports`.
    Semantic(SemanticPortId),
    /// A strip-internal graph position resolved through `spec`.
    StripInternal(StripInternal),
    /// A send's configured tap point (`map.send_taps`).
    SendTap(SendId),
    /// Processor sidechain inputs — explicit port required when more
    /// than one exists.
    SidechainInputs(Vec<SemanticPortId>),
}

/// Resolves a [`SemanticProbePoint`] against the committed project,
/// the generation-scoped [`RoutingSourceMap`] and the published
/// [`GraphSpec`]. Pure and fail-closed: unknown references, missing
/// physical slots and unsupported combinations are reported, never
/// substituted.
///
/// Boundary rules (see the SF-04 slice1 design doc):
///
/// - Strip (incl. track/bus): `Input`/`PreInsert` → main-in semantic
///   port (entry: chain head input or fader input — the lowering has
///   no separate pre-strip stage, so both coincide); `PostInsert` →
///   fader node input; `PreFader` → fader pre-out; `Output`/`PostFader`
///   → main-out semantic port (fader post-out); `Sidechain` →
///   `Unsupported`.
/// - Processor: `Input`/`PreInsert` → main-in; `Output`/`PostInsert` →
///   main-out; `Sidechain` → the named sidechain input port (or the
///   single sidechain port when unambiguous); `PreFader`/`PostFader` →
///   `Unsupported`.
/// - Send: `Input` → the configured tap point; `Output` → the send
///   connection's destination semantic port (the shared destination
///   summing point — send-isolated observation is not lowered);
///   other boundaries → `Unsupported`.
#[must_use]
pub fn resolve_probe_point(
    project: &Project,
    map: &RoutingSourceMap,
    spec: &GraphSpec,
    point: &SemanticProbePoint,
) -> ProbePointResolution {
    let Some(subject) = probe_subject(project, point.entity) else {
        return ProbePointResolution::Unresolved;
    };
    let selection = match select_port(project, subject, point.boundary) {
        Ok(selection) => selection,
        Err(resolution) => return resolution,
    };

    // An explicit `point.port` must agree with the boundary-selected
    // semantic port — it disambiguates, never overrides.
    let selection = match (point.port, selection) {
        (Some(port_id), PortSelection::Semantic(expected)) if port_id != expected => {
            return ProbePointResolution::Unresolved;
        }
        (Some(port_id), PortSelection::SidechainInputs(ref ports)) if !ports.contains(&port_id) => {
            return ProbePointResolution::Unresolved;
        }
        (Some(_), PortSelection::StripInternal(_) | PortSelection::SendTap(_)) => {
            return ProbePointResolution::Unresolved;
        }
        (Some(port_id), PortSelection::SidechainInputs(_)) => PortSelection::Semantic(port_id),
        (_, selection) => selection,
    };

    match selection {
        PortSelection::Semantic(port_id) => resolve_semantic_port(map, port_id),
        PortSelection::StripInternal(pos) => {
            resolve_strip_internal(project, map, spec, subject, pos)
        }
        PortSelection::SendTap(send_id) => match map.send_taps.get(&send_id) {
            Some(port_ref) => ProbePointResolution::Resolved(*port_ref),
            // The send exists but produced no tap this build (e.g.
            // muted source) — no physical slot, reported honestly.
            None => ProbePointResolution::Unavailable,
        },
        PortSelection::SidechainInputs(ports) => match ports.as_slice() {
            [only] => resolve_semantic_port(map, *only),
            _ => ProbePointResolution::Unresolved,
        },
    }
}

/// Maps an entity reference to its probe subject — track/bus entities
/// resolve through their owning strip.
fn probe_subject(project: &Project, entity: SemanticSignalEntityRef) -> Option<ProbeSubject> {
    let routing = &project.routing;
    match entity {
        SemanticSignalEntityRef::Track(track) => routing
            .strip_of_track(track)
            .map(|strip| ProbeSubject::Strip(strip.id)),
        SemanticSignalEntityRef::Bus(bus) => routing
            .buses
            .get(&bus)
            .map(|b| ProbeSubject::Strip(b.strip)),
        SemanticSignalEntityRef::Strip(strip) => routing
            .strips
            .contains_key(&strip)
            .then_some(ProbeSubject::Strip(strip)),
        SemanticSignalEntityRef::Processor(processor) => routing
            .processors
            .contains_key(&processor)
            .then_some(ProbeSubject::Processor(processor)),
        SemanticSignalEntityRef::Send(send) => routing
            .sends
            .contains_key(&send)
            .then_some(ProbeSubject::Send(send)),
    }
}

/// Boundary-driven selection of the semantic port (or graph-internal
/// position) a point observes.
fn select_port(
    project: &Project,
    subject: ProbeSubject,
    boundary: SignalBoundary,
) -> Result<PortSelection, ProbePointResolution> {
    let routing = &project.routing;
    match subject {
        ProbeSubject::Strip(strip_id) => {
            if !routing.strips.contains_key(&strip_id) {
                return Err(ProbePointResolution::Unresolved);
            }
            match boundary {
                SignalBoundary::Input | SignalBoundary::PreInsert => Ok(PortSelection::Semantic(
                    strip_main_port(project, strip_id, PortDirection::Input)?,
                )),
                SignalBoundary::Output | SignalBoundary::PostFader => Ok(PortSelection::Semantic(
                    strip_main_port(project, strip_id, PortDirection::Output)?,
                )),
                SignalBoundary::PostInsert => {
                    Ok(PortSelection::StripInternal(StripInternal::FaderInput))
                }
                SignalBoundary::PreFader => {
                    Ok(PortSelection::StripInternal(StripInternal::FaderPreOut))
                }
                SignalBoundary::Sidechain => Err(ProbePointResolution::Unsupported),
            }
        }
        ProbeSubject::Processor(processor_id) => {
            let Some(processor) = routing.processors.get(&processor_id) else {
                return Err(ProbePointResolution::Unresolved);
            };
            match boundary {
                SignalBoundary::Input | SignalBoundary::PreInsert => {
                    Ok(PortSelection::Semantic(owner_main_port(
                        project,
                        PortOwner::Processor(processor_id),
                        PortDirection::Input,
                    )?))
                }
                SignalBoundary::Output | SignalBoundary::PostInsert => {
                    Ok(PortSelection::Semantic(owner_main_port(
                        project,
                        PortOwner::Processor(processor_id),
                        PortDirection::Output,
                    )?))
                }
                SignalBoundary::Sidechain => {
                    let ports: Vec<SemanticPortId> = processor
                        .port_ids
                        .iter()
                        .copied()
                        .filter(|id| {
                            routing.ports.get(id).is_some_and(|port| {
                                port.owner == PortOwner::Processor(processor_id)
                                    && port.direction == PortDirection::Input
                                    && port.role == PortRole::Sidechain
                            })
                        })
                        .collect();
                    Ok(PortSelection::SidechainInputs(ports))
                }
                SignalBoundary::PreFader | SignalBoundary::PostFader => {
                    Err(ProbePointResolution::Unsupported)
                }
            }
        }
        ProbeSubject::Send(send_id) => {
            let Some(send) = routing.sends.get(&send_id) else {
                return Err(ProbePointResolution::Unresolved);
            };
            match boundary {
                SignalBoundary::Input => Ok(PortSelection::SendTap(send_id)),
                SignalBoundary::Output => {
                    let destination = routing
                        .connections
                        .get(&send.connection)
                        .map(|conn| conn.destination);
                    match destination {
                        Some(port_id) => Ok(PortSelection::Semantic(port_id)),
                        None => Err(ProbePointResolution::Unresolved),
                    }
                }
                _ => Err(ProbePointResolution::Unsupported),
            }
        }
    }
}

/// The strip's single main port in `direction` (`Main` role).
fn strip_main_port(
    project: &Project,
    strip: StripId,
    direction: PortDirection,
) -> Result<SemanticPortId, ProbePointResolution> {
    owner_main_port(project, PortOwner::Strip(strip), direction)
}

/// The owner's single `Main`-role port in `direction`.
fn owner_main_port(
    project: &Project,
    owner: PortOwner,
    direction: PortDirection,
) -> Result<SemanticPortId, ProbePointResolution> {
    let mut found = None;
    for (id, port) in &project.routing.ports {
        if port.owner == owner
            && port.direction == direction
            && port.role == PortRole::Main
            && port.domain == SignalDomain::Audio
        {
            if found.is_some() {
                return Err(ProbePointResolution::Unresolved);
            }
            found = Some(*id);
        }
    }
    found.ok_or(ProbePointResolution::Unresolved)
}

/// Maps a semantic port through the source map — the `MappingResolution`
/// vocabulary is preserved so muted/disabled/out-of-scope lowering
/// stays explicit.
fn resolve_semantic_port(map: &RoutingSourceMap, port_id: SemanticPortId) -> ProbePointResolution {
    match map.ports.get(&port_id) {
        Some(MappingResolution::Resolved(port_ref)) => ProbePointResolution::Resolved(*port_ref),
        Some(MappingResolution::Unavailable(_) | MappingResolution::NotLowered(_)) => {
            ProbePointResolution::Unavailable
        }
        Some(MappingResolution::Unsupported(_)) => ProbePointResolution::Unsupported,
        None => ProbePointResolution::Unresolved,
    }
}

/// Resolves a strip-internal position (fader input / pre-fader tap)
/// through the fader node in the published spec. These positions are
/// graph-internal — no semantic port names them — so resolution goes
/// through `semantic_nodes` + `spec`, never through `ports`.
fn resolve_strip_internal(
    project: &Project,
    map: &RoutingSourceMap,
    spec: &GraphSpec,
    subject: ProbeSubject,
    pos: StripInternal,
) -> ProbePointResolution {
    let ProbeSubject::Strip(strip_id) = subject else {
        return ProbePointResolution::Unresolved;
    };
    let node_id = match map.semantic_nodes.get(&SemanticNodeRef::Strip(strip_id)) {
        Some(MappingResolution::Resolved(node)) => *node,
        Some(MappingResolution::Unavailable(_) | MappingResolution::NotLowered(_)) => {
            return ProbePointResolution::Unavailable;
        }
        Some(MappingResolution::Unsupported(_)) => return ProbePointResolution::Unsupported,
        None => return ProbePointResolution::Unresolved,
    };
    let Some(node) = spec.node(node_id) else {
        return ProbePointResolution::Unavailable;
    };
    match pos {
        StripInternal::FaderInput => {
            // Exactly one audio input port on the fader node.
            let inputs: Vec<_> = node
                .ports()
                .iter()
                .filter(|p| p.direction() == Direction::Input && audio_port(p))
                .collect();
            match inputs.as_slice() {
                [port] => ProbePointResolution::Resolved(PortRef {
                    node: node_id,
                    port: port.id(),
                }),
                _ => ProbePointResolution::Unavailable,
            }
        }
        StripInternal::FaderPreOut => {
            // The pre-fader tap is the fader's audio output port
            // *other* than the resolved main-out (post-fader) port.
            let Ok(main_out) = strip_main_port(project, strip_id, PortDirection::Output) else {
                return ProbePointResolution::Unresolved;
            };
            let post_port = match map.ports.get(&main_out) {
                Some(MappingResolution::Resolved(port_ref)) if port_ref.node == node_id => {
                    port_ref.port
                }
                // A main-out resolved to a different node cannot
                // delimit this fader's pre-out — fail closed.
                Some(
                    MappingResolution::Resolved(_)
                    | MappingResolution::Unavailable(_)
                    | MappingResolution::NotLowered(_),
                ) => {
                    return ProbePointResolution::Unavailable;
                }
                Some(MappingResolution::Unsupported(_)) => {
                    return ProbePointResolution::Unsupported;
                }
                None => return ProbePointResolution::Unresolved,
            };
            let pre_ports: Vec<_> = node
                .ports()
                .iter()
                .filter(|p| {
                    p.direction() == Direction::Output && p.id() != post_port && audio_port(p)
                })
                .collect();
            match pre_ports.as_slice() {
                [port] => ProbePointResolution::Resolved(PortRef {
                    node: node_id,
                    port: port.id(),
                }),
                _ => ProbePointResolution::Unavailable,
            }
        }
    }
}

fn audio_port(port: &ruviform_graph::PortSpec) -> bool {
    matches!(port.format(), PortFormat::Audio(_))
}

// ── SF-04 slice 6: multi-probe alignment comparison ─────────────
//
// `ProbeComparisonRequest` → `AcousticObservation` is the Issue #107
// §4 contract for comparing two semantic observation points. The
// request is persistable intent; the observation is runtime evidence
// (clock ids carry no serde — the same rule as `ClockContextRef`).
// Alignment itself is Ruviform #309's `align_observation_points`;
// these types are its semantic mirror plus the consumer-side
// resolution failures the runtime vocabulary cannot name.

/// Which side of a [`ProbeComparisonRequest`] an evidence failure or
/// gap belongs to.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProbeComparisonSide {
    /// The `before` (reference) point.
    Before,
    /// The `after` point.
    After,
}

/// Metric families a comparison may cover — declared intent carried
/// on the request for downstream consumers. Slice 6 compares the
/// alignment envelope only (clock/epoch/latency/format/generation/
/// coverage); SF-PROBE-08 diagnostics and SF-PROBE-09 measurement own
/// the payload content.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProbeMetricKind {
    /// Per-channel Peak/RMS block metrics.
    PeakRms,
    /// Bounded captured-PCM difference evidence.
    PcmDifference,
}

/// Explicit consumer choices for probe alignment — the semantic
/// mirror of Ruviform #309 `AlignPolicy`. Defaults match the
/// fail-closed runtime policy: same generation required, known
/// latency required, no implicit rate conversion, zero error budget,
/// conditional results allowed, shared cursor preferred over anchors.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
// flat policy bag mirroring ruviform AlignPolicy — each flag is an
// independent consumer choice, not a state machine
#[allow(clippy::struct_excessive_bools)]
pub struct ProbeAlignPolicy {
    /// Both spans must share one plan generation — no crossing and
    /// equal ranges.
    pub require_same_generation: bool,
    /// Both spans must declare known latency.
    pub require_known_latency: bool,
    /// Permit rational sample-rate conversion (residue enters the
    /// error budget).
    pub allow_rate_conversion: bool,
    /// Frame bound permitting a bounded-unknown latency condition;
    /// `None` refuses unknown latency outright.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unknown_latency_bound_frames: Option<u64>,
    /// Maximum tolerated alignment error in frames.
    pub max_error_frames: u64,
    /// Whether `Conditional` results may be returned; `false` turns
    /// every condition into the matching incomparable reason.
    pub allow_conditional: bool,
    /// Try anchor-mapped engine frames before the shared cursor.
    pub prefer_engine_frame: bool,
}

impl Default for ProbeAlignPolicy {
    fn default() -> Self {
        Self {
            require_same_generation: true,
            require_known_latency: true,
            allow_rate_conversion: false,
            unknown_latency_bound_frames: None,
            max_error_frames: 0,
            allow_conditional: true,
            prefer_engine_frame: false,
        }
    }
}

/// One multi-probe comparison request — non-RT and bounded by the
/// `max_bytes` evidence budget. Semantic points only: runtime probe
/// ids and graph-local node/port ids never appear here.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProbeComparisonRequest {
    /// The "before" (reference) semantic point.
    pub before: SemanticProbePoint,
    /// The "after" semantic point.
    pub after: SemanticProbePoint,
    /// Optional cursor-frame window restricting the comparison —
    /// `None` compares each side's retained evidence extent. An empty
    /// or inverted range fails closed as `CoverageEmpty`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub observed_range: Option<ProbeFrameInterval>,
    /// Alignment policy (defaults to the fail-closed policy).
    #[serde(default)]
    pub alignment_policy: ProbeAlignPolicy,
    /// Metric families the consumer intends to compare.
    #[serde(default)]
    pub metric_set: Vec<ProbeMetricKind>,
    /// Evidence budget in bytes — exceeding it fails closed with
    /// `EvidenceBudgetExceeded` before alignment runs.
    pub max_bytes: u64,
}

/// Inclusive plan-generation range an observation span covered — the
/// contract mirror of the runtime `GenerationRange`.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProbeGenerationRange {
    /// First (oldest) generation observed.
    pub first: u64,
    /// Last (newest) generation observed.
    pub last: u64,
}

/// Coordinate basis an alignment was computed in — semantic mirror
/// of the runtime `AlignmentBasis`.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ProbeComparisonBasis {
    /// Same cursor series and epoch — offsets are cursor-frame
    /// differences (latency-compensated).
    SharedCursor {
        /// Shared clock identity.
        clock: ClockContextRef,
    },
    /// Both spans mapped through anchors into one engine epoch —
    /// offsets are engine-frame differences.
    EngineFrame {
        /// Shared engine epoch the frames live in.
        epoch: ClockEpochId,
    },
    /// A basis the contract cannot represent — never guessed.
    Unknown,
}

/// A condition keeping an alignment `Conditional` rather than
/// `Aligned` — semantic mirror of the runtime `AlignmentCondition`.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum ProbeAlignCondition {
    /// One side's latency is unknown; the declared bound frames the
    /// worst-case misalignment.
    LatencyBounded {
        /// Worst-case latency uncertainty in frames.
        bound_frames: u64,
    },
    /// Anchor calibration or stamp provenance is estimated.
    ProvenanceEstimated,
    /// Part of the compared window lacks valid coverage on at least
    /// one side — `excluded_intervals` names it.
    PartialCoverage,
    /// The spans' generation ranges differ or cross a rebind.
    CrossGeneration,
    /// The spans declare different precisions.
    MixedPrecision,
    /// A condition the contract cannot represent (future runtime
    /// variant) — recorded, never guessed.
    Unknown,
}

/// Why two observations cannot be aligned — consumer-side resolution
/// failures the runtime vocabulary cannot name, plus the semantic
/// mirror of the runtime `IncomparableReason`. `Unknown` covers
/// future runtime variants.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProbeIncomparableReason {
    /// No subscription binding exists for the semantic point.
    PointUnresolved {
        /// Which side failed.
        side: ProbeComparisonSide,
    },
    /// Bindings exist for the point but none has an installed
    /// runtime probe.
    ProbeNotBound {
        /// Which side failed.
        side: ProbeComparisonSide,
    },
    /// The compared window contains no observed data records on this
    /// side (nothing retained, or the window covers only
    /// status/gap evidence).
    NoObservationData {
        /// Which side failed.
        side: ProbeComparisonSide,
    },
    /// In-window records disagree on channel count or sample rate —
    /// the side cannot declare one honest format.
    InconsistentFormat {
        /// Which side failed.
        side: ProbeComparisonSide,
    },
    /// The observed channel count has no declarable layout — the
    /// runtime record carries no speaker mask, so non-canonical
    /// counts fail closed rather than inventing one.
    UnsupportedChannelLayout {
        /// Which side failed.
        side: ProbeComparisonSide,
        /// Observed channel count.
        channels: u16,
    },
    /// Retained evidence exceeds the request's byte budget.
    EvidenceBudgetExceeded,
    /// Either span's clock mark is unknown — unknown domains never
    /// align, not even with each other.
    UnknownClock,
    /// Different clock domains and no shared anchor evidence.
    DomainMismatch,
    /// Same domain but different epochs and no anchor evidence —
    /// the Seek/Loop/Restart discontinuity case.
    EpochMismatch,
    /// Anchor mapping placed the spans in different engine epochs.
    SeriesMismatch,
    /// Declared sample rates differ and no conversion was permitted
    /// (or the rate is unusable).
    RateMismatch {
        /// First span's declared rate.
        a: u32,
        /// Second span's declared rate.
        b: u32,
    },
    /// Channel count or layout differ — not the same signal shape.
    FormatMismatch,
    /// Latency is unknown and the policy allows no bound.
    LatencyUnknown,
    /// No jointly valid coverage inside the compared window.
    CoverageEmpty,
    /// A generation/epoch discontinuity or mixed clock marks fall
    /// inside the compared window.
    DiscontinuityInWindow,
    /// The spans' generation ranges cross a plan boundary while the
    /// policy requires a single generation.
    GenerationSpan,
    /// No anchor exists for a span's domain (or its context/frame
    /// base is undeclared).
    AnchorMissing,
    /// Anchor mapping was attempted and rejected — the runtime
    /// rejection detail stays on the Ruviform result.
    AnchorRejected,
    /// Frame arithmetic overflowed.
    Overflow,
    /// Computed error budget exceeds the policy's frame bound.
    ErrorBudgetExceeded,
    /// Provenance is estimated and conditional results are
    /// disallowed.
    EstimatedProvenance,
    /// A reason the contract cannot represent (future runtime
    /// variant) — recorded, never guessed.
    Unknown,
}

/// The three-valued alignment result — semantic mirror of the
/// runtime `Alignment`, with the compared window and excluded
/// evidence spelled out in contract vocabulary.
#[non_exhaustive]
#[derive(Clone, Debug, PartialEq)]
pub enum ProbeComparisonQuality {
    /// Unconditionally aligned — basis, offset, error budget and the
    /// jointly observed overlap are all explicit.
    Aligned {
        /// Coordinate basis used.
        basis: ProbeComparisonBasis,
        /// A→B offset in basis frame units for the same signal
        /// instant (latency-compensated on the shared-cursor basis).
        offset_frames: i64,
        /// Worst-case alignment error in frames.
        error_budget_frames: u64,
    },
    /// Alignable only under the stated conditions — never silently
    /// promoted.
    Conditional {
        /// Coordinate basis used.
        basis: ProbeComparisonBasis,
        /// A→B offset (same convention as `Aligned`).
        offset_frames: i64,
        /// Worst-case alignment error in frames.
        error_budget_frames: u64,
        /// Conditions the consumer must weigh.
        conditions: Vec<ProbeAlignCondition>,
        /// Sub-intervals of the overlap excluded from valid
        /// comparison, in after-side basis coordinates.
        excluded_intervals: Vec<ProbeFrameInterval>,
    },
    /// Cannot be aligned — explicit reason.
    Incomparable {
        /// Why.
        reason: ProbeIncomparableReason,
    },
}

/// A classified non-valid coverage interval inside a compared
/// window. `range` is in that side's own cursor coordinates.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeComparisonGap {
    /// Which side the gap belongs to.
    pub side: ProbeComparisonSide,
    /// Frame interval the gap covers.
    pub range: ProbeFrameInterval,
    /// Gap classification (`Unknown` covers unexplained regions,
    /// including window areas outside all retained coverage).
    pub kind: ObservationGapKind,
}

/// Clock/generation provenance of the compared evidence.
#[derive(Clone, Debug, PartialEq)]
pub struct ProbeComparisonProvenance {
    /// Clock identity of each side's span (`None` = side unresolved
    /// or mark unknown).
    pub before_clock: Option<ClockContextRef>,
    /// Clock identity of the after side's span.
    pub after_clock: Option<ClockContextRef>,
    /// Generation range each side's span covered.
    pub before_generations: Option<ProbeGenerationRange>,
    /// Generation range of the after side's span.
    pub after_generations: Option<ProbeGenerationRange>,
    /// Cursor-series runtime instance.
    pub runtime_instance: u64,
    /// Declared sample rate of the cursor series (`0` = undeclared).
    pub sample_rate: u32,
}

/// The `ProbeComparisonRequest → AcousticObservation` result —
/// runtime evidence, never persisted (clock ids carry no serde).
/// Signal-value payloads (peak/rms deltas, bounded PCM difference)
/// are SF-PROBE-08/09 scope; this slice reports the alignment
/// envelope honestly and fabricates no signal content.
#[derive(Clone, Debug, PartialEq)]
pub struct AcousticObservation {
    /// Semantic points compared (echo of the request).
    pub before: SemanticProbePoint,
    /// Semantic points compared (echo of the request).
    pub after: SemanticProbePoint,
    /// Three-valued alignment quality.
    pub quality: ProbeComparisonQuality,
    /// After-side cursor-frame window actually compared — present
    /// iff both sides resolved to evidence spans (alignment was
    /// attempted). Units are the after probe's cursor frames
    /// regardless of the alignment basis.
    pub interval: Option<ProbeFrameInterval>,
    /// Per-side classified non-valid coverage inside each side's
    /// window.
    pub gap_mask: Vec<ProbeComparisonGap>,
    /// Evidence provenance.
    pub provenance: ProbeComparisonProvenance,
}
