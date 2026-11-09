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
    /// A segment end the contract cannot represent (future runtime
    /// variant) — recorded, never guessed.
    Unknown,
}

/// One capture segment — the probe-observation counterpart of a
/// recording segment, *not* a persisted recording-domain entity.
/// A segment never spans a plan-generation boundary: it is cut at the
/// boundary and a new capture requires an explicit new request.
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
                Some(MappingResolution::Resolved(_)) => {
                    return ProbePointResolution::Unavailable;
                }
                Some(MappingResolution::Unavailable(_) | MappingResolution::NotLowered(_)) => {
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
