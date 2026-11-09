//! Read-only Signal Flow projection + per-view visual state
//! (synvelis#108 SF-05 slice 1).
//!
//! [`signal_flow_graph`] projects `Project::routing` (the SF-01
//! semantic authority) into display DTOs — nodes/edges/status —
//! following the same "read the session, return DTOs" pattern as
//! [`crate::relation_ui`] and [`crate::arrangement_ui`]. It mutates
//! nothing: no project op, no undo entry, no engine call.
//!
//! [`SignalFlowVisualState`] is the Issue #108 `SignalFlowViewState`
//! counterpart — positions/groups/collapse/zoom/link-focus/scope,
//! a pure value type owned per `ViewInstanceId` by the app layer.
//! It deliberately stays out of `WorkspaceSnapshot`/`ViewState`
//! (schema v2, `deny_unknown_fields`): persistence is SF-UX-07.
//!
//! Runtime/probe truth arrives through [`SignalFlowRuntimeEvidence`]
//! — a snapshot the app composes from `SessionRoutingEngine`
//! (`receipt()`/`source_map()`/`probe_bindings()`). `Published`
//! (non-RT publish success) and `Active` (RT-observed generation)
//! stay distinct generation fields; probe `requested`/`queued`
//! states never masquerade as RT-confirmed observation.

use std::collections::{BTreeMap, BTreeSet};

use synvelis_project::{
    AvailabilityReason, BusId, BusKind, ConnectionKind, MappingResolution, NotLoweredReason,
    PortBinding, PortOwner, PortRole, ProbeMode, ProbeResolveState, ProcessorBinding, ProcessorId,
    ProjectId, ProjectRevision, RoutingActivationReceipt, RoutingActivationState,
    RoutingConnectionId, RoutingFault, RoutingModel, RoutingSourceMap, SemanticNodeRef,
    SemanticPortId, SemanticProbeBinding, SemanticProbePoint, SemanticSignalEntityRef, SendId,
    SendTap, StripId, StripOwner, TrackId, UnsupportedReason,
};

use crate::session::ProjectSession;

// -- typed entity refs -------------------------------------------------

/// Stable semantic entity a Signal Flow view can name — the Issue
/// #108 `SignalFlowEntityRef` vocabulary. Independent of the
/// arrangement [`crate::selection::SelectionDomain`]: naming a
/// routing entity here never retargets timeline selection or link
/// context.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SignalFlowEntityRef {
    /// A production track (resolved through its channel strip).
    Track(TrackId),
    /// A bus (group/return/master — resolved through its strip).
    Bus(BusId),
    /// A channel strip directly.
    Strip(StripId),
    /// A chain processor.
    Processor(ProcessorId),
    /// A semantic port.
    Port(SemanticPortId),
    /// A routing connection.
    Connection(RoutingConnectionId),
    /// A send.
    Send(SendId),
}

/// Identity of a projected graph node — a strip or (Expanded scope
/// only) one chain processor.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SignalFlowNodeId {
    /// A channel strip node.
    Strip(StripId),
    /// A chain processor node (Expanded scope).
    Processor(ProcessorId),
}

/// What a [`SignalFlowNode`] semantically is.
#[derive(Clone, Debug, PartialEq)]
pub enum SignalFlowNodeKind {
    /// A track's channel strip.
    TrackStrip {
        /// Owning track.
        track: TrackId,
        /// Track display name.
        name: String,
    },
    /// A bus's channel strip (group/return/master — the bus *is* its
    /// strip's DSP owner; no duplicate node is emitted).
    BusStrip {
        /// Owning bus.
        bus: BusId,
        /// Bus role (`Master`/`Group`/`Return`).
        kind: BusKind,
        /// Bus display name.
        name: String,
    },
    /// A chain processor inside a strip.
    Processor {
        /// Owning strip.
        strip: StripId,
        /// Position in `chain.processors` order.
        index: usize,
    },
}

/// Semantic resolution state of a node — carried verbatim from the
/// model (`ProcessorBinding::Unresolved` / `PortBinding::Unresolved`).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SignalFlowNodeState {
    /// All bindings resolve.
    Ready,
    /// A plugin/port binding is explicitly unresolved — kept visible,
    /// never silently rebound.
    Unresolved,
}

/// Runtime/probe state of a node composed from
/// [`SignalFlowRuntimeEvidence`]. `Unobserved` is the honest default:
/// no engine/map means nothing is claimed about the live graph.
#[derive(Clone, Debug, PartialEq)]
pub enum SignalFlowNodeRuntime {
    /// No runtime evidence (engine absent, or no mapping record).
    Unobserved,
    /// Mapped to a live runtime node under `generation`.
    Resolved {
        /// Plan generation the mapping belongs to.
        generation: u64,
    },
    /// The entity exists but its runtime object is unavailable.
    Unavailable(AvailabilityReason),
    /// The entity is outside the lowerable domain.
    Unsupported(UnsupportedReason),
    /// Lowering intentionally produced nothing (muted strip,
    /// disabled connection, out of scope).
    NotLowered(NotLoweredReason),
    /// The source map predates the live project revision — its
    /// resolutions describe an older snapshot.
    Stale,
}

/// One projected node.
#[derive(Clone, Debug, PartialEq)]
pub struct SignalFlowNode {
    /// Node identity.
    pub id: SignalFlowNodeId,
    /// Semantic kind + display label source.
    pub kind: SignalFlowNodeKind,
    /// Display label.
    pub label: String,
    /// Semantic binding state.
    pub state: SignalFlowNodeState,
    /// Runtime/probe composition.
    pub runtime: SignalFlowNodeRuntime,
    /// Latest probe lifecycle state for this node, when a binding
    /// targets it (`None` = no probe was ever requested — distinct
    /// from "observed").
    pub probe: Option<ProbeResolveState>,
    /// Strip mute flag (control model authority; `false` for
    /// processors).
    pub muted: bool,
    /// Strip solo flag (`false` for processors).
    pub soloed: bool,
    /// Chain length for strip nodes; `0` for processors.
    pub processor_count: usize,
}

// -- edges ------------------------------------------------------------

/// Identity of a projected edge. `Connection` edges are 1:1 with
/// persisted [`synvelis_project::RoutingConnection`]s; `Chain` edges
/// are synthesized from `chain.processors` order (Expanded scope)
/// and live in a disjoint id namespace — they can never collide with
/// or duplicate a real connection.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SignalFlowEdgeId {
    /// A persisted routing connection.
    Connection(RoutingConnectionId),
    /// A synthesized processor-order edge inside one strip's chain.
    Chain {
        /// Owning strip.
        strip: StripId,
        /// Index of the upstream processor in chain order.
        index: usize,
    },
}

/// Edge role — the Role Filter vocabulary. Roles stay distinct even
/// when endpoints collapse to the same strip (a Sidechain edge into a
/// folded processor never masquerades as a Main edge).
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SignalFlowEdgeRole {
    /// Strip/processor main signal path.
    Main,
    /// Key/sidechain input.
    Sidechain,
    /// Send tap route (Pre/Post carried by [`SignalFlowEdge::tap`]).
    Send,
    /// Synthesized processor-order edge (Expanded scope only).
    Chain,
}

/// One edge endpoint: the node plus the semantic port it touches.
#[derive(Clone, Debug, PartialEq)]
pub struct SignalFlowEndpoint {
    /// Node this end attaches to.
    pub node: SignalFlowNodeId,
    /// Semantic port id, when the edge is port-anchored (`None` for
    /// synthesized chain edges).
    pub port: Option<SemanticPortId>,
    /// Port role within its owner.
    pub port_role: PortRole,
}

/// One projected edge.
#[derive(Clone, Debug, PartialEq)]
pub struct SignalFlowEdge {
    /// Edge identity (connection 1:1 or synthesized chain).
    pub id: SignalFlowEdgeId,
    /// Display/filter role.
    pub role: SignalFlowEdgeRole,
    /// Upstream endpoint.
    pub source: SignalFlowEndpoint,
    /// Downstream endpoint.
    pub target: SignalFlowEndpoint,
    /// `RoutingConnection::enabled` verbatim — disabled edges carry
    /// no signal but remain visible as disabled.
    pub enabled: bool,
    /// Send tap point (`Some` only for `role = Send`).
    pub tap: Option<SendTap>,
}

// -- query / options ---------------------------------------------------

/// Display scope — the Issue #108 `SignalFlowScope` vocabulary minus
/// `History` snapshot identity (VCS infra not landed; `History`
/// queries return `coverage = Unsupported` honestly).
#[derive(Clone, Debug, PartialEq)]
pub enum SignalFlowScope {
    /// Strip-level graph only — no processor nodes.
    Overview,
    /// Root entity + its direct neighbors (1-hop induced subgraph).
    /// A `Processor` root expands only its owning strip's chain for
    /// that processor.
    Local {
        /// Root entity to resolve.
        root: SignalFlowEntityRef,
    },
    /// Strip-level graph plus every non-collapsed strip's processor
    /// chain expanded.
    Expanded,
    /// Fixed-snapshot history view — unsupported until VCS snapshot
    /// infrastructure lands (SF-UX-07+).
    History,
}

/// Signal layer — only the audio routing layer exists in
/// `RoutingModel` today (`SignalDomain::Audio` is the sole variant).
/// Control relations project through [`crate::relation_ui`] instead;
/// they are deliberately not duplicated here.
#[non_exhaustive]
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SignalFlowLayer {
    /// Audio routing (the only layer this slice projects).
    Audio,
}

/// How much of the graph a query may return.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum SignalFlowCoverage {
    /// Everything requested fit the bounds.
    Complete,
    /// Node/edge bounds cut the result — deterministic prefix kept.
    Truncated,
    /// `expected_revision` did not match the live revision.
    Stale,
    /// The requested scope/root/layer cannot be satisfied.
    Unavailable,
    /// The request needs infrastructure that does not exist yet
    /// (`History` scope, non-audio layers).
    Unsupported,
}

/// Query options for [`signal_flow_graph`].
#[derive(Clone, Debug)]
pub struct SignalFlowViewOptions {
    /// Display scope.
    pub scope: SignalFlowScope,
    /// Layers to include — must contain [`SignalFlowLayer::Audio`].
    pub layers: BTreeSet<SignalFlowLayer>,
    /// Edge roles to include (Role Filter).
    pub roles: BTreeSet<SignalFlowEdgeRole>,
    /// Entities whose processor expansion stays folded (Collapse) —
    /// the strip/node itself and its edges are unaffected.
    pub collapsed: BTreeSet<SignalFlowEntityRef>,
    /// Node bound.
    pub max_nodes: usize,
    /// Edge bound.
    pub max_edges: usize,
    /// Revision the caller believes is live — `Some` mismatch yields
    /// `coverage = Stale` (reproduces stale-query behavior).
    pub expected_revision: Option<ProjectRevision>,
    /// Caller-side view generation echoed into the result for stale
    /// response matching (Issue #108 query contract).
    pub view_generation: u64,
}

impl Default for SignalFlowViewOptions {
    fn default() -> Self {
        Self {
            scope: SignalFlowScope::Overview,
            layers: BTreeSet::from([SignalFlowLayer::Audio]),
            roles: BTreeSet::from([
                SignalFlowEdgeRole::Main,
                SignalFlowEdgeRole::Sidechain,
                SignalFlowEdgeRole::Send,
                SignalFlowEdgeRole::Chain,
            ]),
            collapsed: BTreeSet::new(),
            max_nodes: 64,
            max_edges: 128,
            expected_revision: None,
            view_generation: 0,
        }
    }
}

// -- runtime evidence / status -----------------------------------------

/// Snapshot of engine-side truth the app composes for one query.
/// Every field is a `synvelis-project` type — the workspace never
/// sees player/runtime handles.
#[derive(Clone, Debug, Default)]
pub struct SignalFlowRuntimeEvidence {
    /// Latest activation receipt (`None` before the first prepare).
    pub receipt: Option<RoutingActivationReceipt>,
    /// Latest published plan's semantic→runtime map.
    pub source_map: Option<RoutingSourceMap>,
    /// Live probe bindings (requested *and* RT-confirmed states —
    /// the distinction is the point).
    pub probes: Vec<SemanticProbeBinding>,
}

/// Per-binding probe status carried verbatim — requested vs
/// RT-confirmed mode/state stay separate fields.
#[derive(Clone, Debug, PartialEq)]
pub struct SignalFlowProbeStatus {
    /// Semantic observation point.
    pub point: SemanticProbePoint,
    /// Lifecycle/resolution state.
    pub state: ProbeResolveState,
    /// Last requested mode.
    pub requested_mode: ProbeMode,
    /// RT-confirmed mode (`None` until status evidence arrives —
    /// "probe 未取得" is this `None`, not an empty list).
    pub confirmed_mode: Option<ProbeMode>,
    /// Plan generation the binding resolved against.
    pub generation: u64,
    /// Project revision the binding was requested against.
    pub project_revision: ProjectRevision,
}

/// Truthful status block — revision/generation/staleness/probe
/// evidence for the Signal Flow status display (SF-UX-05).
#[derive(Clone, Debug, PartialEq)]
pub struct SignalFlowStatus {
    /// Live project revision the projection describes.
    pub project_revision: ProjectRevision,
    /// Whether runtime evidence was supplied at all.
    pub engine_attached: bool,
    /// Activation evidence state — verbatim
    /// [`RoutingActivationState`]. `Published` and `Active` are
    /// distinct variants; publish success never implies activation.
    pub activation: Option<RoutingActivationState>,
    /// Generation the compile planned.
    pub planned_generation: Option<u64>,
    /// Generation `publish` accepted (non-RT evidence).
    pub published_generation: Option<u64>,
    /// Generation the RT side was observed running.
    pub observed_active_generation: Option<u64>,
    /// Last generation confirmed active before any failure.
    pub last_good_generation: Option<u64>,
    /// Failure detail when activation is `Blocked`/`Failed`.
    pub fault: Option<RoutingFault>,
    /// Revision the source map was built from.
    pub source_map_revision: Option<ProjectRevision>,
    /// Generation the source map's runtime IDs are scoped to.
    pub source_map_generation: Option<u64>,
    /// `true` when a source map exists but predates the live
    /// revision — its resolutions describe an older snapshot.
    pub stale: bool,
    /// Probe bindings in deterministic key order.
    pub probes: Vec<SignalFlowProbeStatus>,
}

/// The projection result.
#[derive(Clone, Debug, PartialEq)]
pub struct SignalFlowView {
    /// Owning project.
    pub project_id: ProjectId,
    /// Live revision the semantic snapshot came from.
    pub project_revision: ProjectRevision,
    /// Echo of `options.view_generation`.
    pub view_generation: u64,
    /// Query coverage.
    pub coverage: SignalFlowCoverage,
    /// Scope the result was computed under.
    pub scope: SignalFlowScope,
    /// Nodes in deterministic (model) order.
    pub nodes: Vec<SignalFlowNode>,
    /// Edges in deterministic (connection id / chain order) order.
    pub edges: Vec<SignalFlowEdge>,
    /// Runtime/probe status block.
    pub status: SignalFlowStatus,
}

// -- visual state ------------------------------------------------------

/// A user-defined visual grouping — display bundling only. It never
/// creates audio routing, ownership, or semantic diff (Issue #108:
/// Visual Group ≠ Semantic Group/Bus/Folder).
#[derive(Clone, Debug, PartialEq)]
pub struct SignalFlowVisualGroup {
    /// Caller-assigned group id.
    pub id: u64,
    /// Display label.
    pub label: String,
    /// Member entities.
    pub members: BTreeSet<SignalFlowEntityRef>,
    /// Whether the group renders folded.
    pub collapsed: bool,
}

/// Per-view visual-only state — the Issue #108
/// `SignalFlowViewState` counterpart. Pure value type: it holds no
/// session/project/engine reference, so no mutation path can reach
/// DSP, project revision, undo, or PCM (SF-UX-02).
#[derive(Clone, Debug, PartialEq)]
pub struct SignalFlowVisualState {
    /// Node positions (visual coordinates only).
    pub positions: BTreeMap<SignalFlowEntityRef, [f64; 2]>,
    /// Visual groups.
    pub groups: Vec<SignalFlowVisualGroup>,
    /// Folded entities (processor expansion suppression).
    pub collapsed: BTreeSet<SignalFlowEntityRef>,
    /// Zoom factor (> 0).
    pub zoom: f64,
    /// Pan offset.
    pub pan: [f64; 2],
    /// Linked inspection focus — display highlight only, never an
    /// editing target.
    pub link_focus: Option<SignalFlowEntityRef>,
    /// Current display scope.
    pub scope: SignalFlowScope,
    /// Edge roles currently filtered in.
    pub roles: BTreeSet<SignalFlowEdgeRole>,
    /// Layers currently filtered in.
    pub layers: BTreeSet<SignalFlowLayer>,
    /// Bumped on every mutation — the stale-response guard.
    pub visual_revision: u64,
}

impl Default for SignalFlowVisualState {
    fn default() -> Self {
        Self {
            positions: BTreeMap::new(),
            groups: Vec::new(),
            collapsed: BTreeSet::new(),
            zoom: 1.0,
            pan: [0.0, 0.0],
            link_focus: None,
            scope: SignalFlowScope::Overview,
            roles: SignalFlowViewOptions::default().roles,
            layers: BTreeSet::from([SignalFlowLayer::Audio]),
            visual_revision: 0,
        }
    }
}

impl SignalFlowVisualState {
    /// Folds the visual state into query options — the bridge between
    /// "what the user arranged" and "what the projection computes".
    /// `expected_revision` stays `None` (live) and bounds are the
    /// caller's.
    #[must_use]
    pub fn view_options(&self, max_nodes: usize, max_edges: usize) -> SignalFlowViewOptions {
        SignalFlowViewOptions {
            scope: self.scope.clone(),
            layers: self.layers.clone(),
            roles: self.roles.clone(),
            collapsed: self.collapsed.clone(),
            max_nodes,
            max_edges,
            expected_revision: None,
            view_generation: self.visual_revision,
        }
    }
}

fn bump(state: &mut SignalFlowVisualState) {
    state.visual_revision += 1;
}

/// Moves a node — visual coordinates only.
pub fn visual_move_node(
    state: &mut SignalFlowVisualState,
    entity: SignalFlowEntityRef,
    position: [f64; 2],
) {
    state.positions.insert(entity, position);
    bump(state);
}

/// Creates or replaces a visual group. Display bundling only — no
/// semantic effect.
pub fn visual_set_group(state: &mut SignalFlowVisualState, group: SignalFlowVisualGroup) {
    match state.groups.iter_mut().find(|g| g.id == group.id) {
        Some(existing) => *existing = group,
        None => state.groups.push(group),
    }
    bump(state);
}

/// Folds or unfolds an entity's expansion detail.
pub fn visual_fold(state: &mut SignalFlowVisualState, entity: SignalFlowEntityRef, folded: bool) {
    if folded {
        state.collapsed.insert(entity);
    } else {
        state.collapsed.remove(&entity);
    }
    bump(state);
}

/// Sets zoom — non-positive values are ignored (a zero/negative zoom
/// is not a meaningful display state).
pub fn visual_set_zoom(state: &mut SignalFlowVisualState, zoom: f64) {
    if zoom > 0.0 && zoom.is_finite() {
        state.zoom = zoom;
        bump(state);
    }
}

/// Sets the pan offset.
pub fn visual_set_pan(state: &mut SignalFlowVisualState, pan: [f64; 2]) {
    state.pan = pan;
    bump(state);
}

/// Sets or clears the linked inspection focus — display highlight
/// only; it never retargets editing selection.
pub fn visual_set_link_focus(
    state: &mut SignalFlowVisualState,
    focus: Option<SignalFlowEntityRef>,
) {
    state.link_focus = focus;
    bump(state);
}

/// Sets the display scope.
pub fn visual_set_scope(state: &mut SignalFlowVisualState, scope: SignalFlowScope) {
    state.scope = scope;
    bump(state);
}

// -- projection --------------------------------------------------------

/// Resolves a [`SignalFlowEntityRef`] to the strip node it denotes,
/// when the project has one. `Processor` resolves to its owning
/// strip; `Port`/`Connection`/`Send` resolve through their port or
/// connection endpoints. `None` = the entity has no strip-level
/// presence (unknown id, or a send/connection that cannot be
/// resolved).
fn entity_strip(routing: &RoutingModel, entity: SignalFlowEntityRef) -> Option<StripId> {
    let strip = match entity {
        SignalFlowEntityRef::Track(track) => routing.strip_of_track(track).map(|s| s.id),
        SignalFlowEntityRef::Bus(bus) => routing.buses.get(&bus).map(|b| b.strip),
        SignalFlowEntityRef::Strip(strip) => Some(strip),
        SignalFlowEntityRef::Processor(processor) => routing.strip_of_processor(processor),
        SignalFlowEntityRef::Port(port) => routing.ports.get(&port).and_then(|p| match p.owner {
            PortOwner::Strip(strip) => Some(strip),
            PortOwner::Processor(processor) => routing.strip_of_processor(processor),
        }),
        SignalFlowEntityRef::Connection(connection) => routing
            .connections
            .get(&connection)
            .and_then(|c| routing.ports.get(&c.source))
            .and_then(|p| match p.owner {
                PortOwner::Strip(strip) => Some(strip),
                PortOwner::Processor(processor) => routing.strip_of_processor(processor),
            }),
        SignalFlowEntityRef::Send(send) => routing
            .sends
            .get(&send)
            .and_then(|s| routing.connections.get(&s.connection))
            .and_then(|c| routing.ports.get(&c.source))
            .and_then(|p| match p.owner {
                PortOwner::Strip(strip) => Some(strip),
                PortOwner::Processor(processor) => routing.strip_of_processor(processor),
            }),
    }?;
    routing.strips.contains_key(&strip).then_some(strip)
}

/// Whether a strip's owned ports or (for processor nodes) its
/// binding carry an explicit `Unresolved` marker.
fn strip_unresolved(routing: &RoutingModel, strip: StripId) -> bool {
    let Some(channel) = routing.strips.get(&strip) else {
        return false;
    };
    channel.port_ids.iter().any(|port| {
        matches!(
            routing.ports.get(port).map(|p| &p.binding),
            Some(PortBinding::Unresolved { .. })
        )
    })
}

fn processor_unresolved(routing: &RoutingModel, processor: ProcessorId) -> bool {
    let Some(proc_entity) = routing.processors.get(&processor) else {
        return false;
    };
    matches!(proc_entity.binding, ProcessorBinding::Unresolved { .. })
        || proc_entity.port_ids.iter().any(|port| {
            matches!(
                routing.ports.get(port).map(|p| &p.binding),
                Some(PortBinding::Unresolved { .. })
            )
        })
}

/// Projects the live routing model into a [`SignalFlowView`].
///
/// Read-only: the session/project are borrowed, never mutated.
/// `evidence` supplies runtime/probe truth; `None` yields honest
/// `Unobserved`/`engine_attached = false` status.
#[must_use]
pub fn signal_flow_graph(
    session: &ProjectSession,
    options: &SignalFlowViewOptions,
    evidence: Option<&SignalFlowRuntimeEvidence>,
) -> SignalFlowView {
    signal_flow_graph_with(session, options, evidence)
}

/// [`signal_flow_graph`] — named `_with` for symmetry with
/// [`crate::relation_ui::relation_rows_with`].
#[must_use]
pub fn signal_flow_graph_with(
    session: &ProjectSession,
    options: &SignalFlowViewOptions,
    evidence: Option<&SignalFlowRuntimeEvidence>,
) -> SignalFlowView {
    let project = session.project();
    let routing = &project.routing;
    let live = session.revision();
    let status = build_status(live, evidence);

    let mut view = SignalFlowView {
        project_id: project.project_id,
        project_revision: live,
        view_generation: options.view_generation,
        coverage: SignalFlowCoverage::Complete,
        scope: options.scope.clone(),
        nodes: Vec::new(),
        edges: Vec::new(),
        status,
    };

    // Layer gate: only Audio exists; anything else is Unsupported.
    if !options.layers.contains(&SignalFlowLayer::Audio) {
        view.coverage = SignalFlowCoverage::Unsupported;
        return view;
    }
    if matches!(options.scope, SignalFlowScope::History) {
        view.coverage = SignalFlowCoverage::Unsupported;
        return view;
    }
    if let Some(expected) = options.expected_revision
        && expected != live
    {
        view.coverage = SignalFlowCoverage::Stale;
        return view;
    }

    // -- strip nodes ---------------------------------------------------
    let mut strip_nodes: BTreeMap<StripId, SignalFlowNode> = BTreeMap::new();
    for (strip_id, channel) in &routing.strips {
        let (kind, label) = match channel.owner {
            StripOwner::Track(track) => {
                let name = project
                    .tracks
                    .get(&track)
                    .map_or_else(|| track.to_string(), |t| t.name.clone());
                (
                    SignalFlowNodeKind::TrackStrip {
                        track,
                        name: name.clone(),
                    },
                    name,
                )
            }
            StripOwner::Bus(bus) => {
                let bus_entity = routing.buses.get(&bus);
                let name = bus_entity.map_or_else(|| bus.to_string(), |b| b.name.clone());
                (
                    SignalFlowNodeKind::BusStrip {
                        bus,
                        kind: bus_entity.map_or(BusKind::Group, |b| b.kind),
                        name: name.clone(),
                    },
                    name,
                )
            }
        };
        let processor_count = routing
            .chains
            .get(&channel.chain)
            .map_or(0, |chain| chain.processors.len());
        strip_nodes.insert(
            *strip_id,
            SignalFlowNode {
                id: SignalFlowNodeId::Strip(*strip_id),
                kind,
                label,
                state: if strip_unresolved(routing, *strip_id) {
                    SignalFlowNodeState::Unresolved
                } else {
                    SignalFlowNodeState::Ready
                },
                runtime: SignalFlowNodeRuntime::Unobserved,
                probe: None,
                muted: routing.strip_muted(&project.control, *strip_id),
                soloed: routing.strip_soloed(&project.control, *strip_id),
                processor_count,
            },
        );
    }

    // -- expansion decision ---------------------------------------------
    // Expanded: every strip not collapsed. Local{Processor}: only the
    // root processor inside its strip. Overview/Local{other}: none.
    let collapsed_strips: BTreeSet<StripId> = options
        .collapsed
        .iter()
        .filter_map(|entity| entity_strip(routing, *entity))
        .collect();
    let mut expanded: BTreeSet<StripId> = BTreeSet::new();
    let mut local_only_processor: Option<ProcessorId> = None;
    match &options.scope {
        SignalFlowScope::Expanded => {
            expanded = strip_nodes.keys().copied().collect();
            expanded.retain(|strip| !collapsed_strips.contains(strip));
        }
        SignalFlowScope::Local {
            root: SignalFlowEntityRef::Processor(processor),
        } => {
            if let Some(strip) = routing.strip_of_processor(*processor)
                && !collapsed_strips.contains(&strip)
            {
                expanded.insert(strip);
                local_only_processor = Some(*processor);
            }
        }
        _ => {}
    }

    // -- processor nodes -------------------------------------------------
    let mut proc_nodes: BTreeMap<ProcessorId, SignalFlowNode> = BTreeMap::new();
    for strip_id in &expanded {
        let Some(channel) = routing.strips.get(strip_id) else {
            continue;
        };
        let Some(chain) = routing.chains.get(&channel.chain) else {
            continue;
        };
        for (index, processor_id) in chain.processors.iter().enumerate() {
            if let Some(only) = local_only_processor
                && *processor_id != only
            {
                continue;
            }
            let Some(entity) = routing.processors.get(processor_id) else {
                continue;
            };
            proc_nodes.insert(
                *processor_id,
                SignalFlowNode {
                    id: SignalFlowNodeId::Processor(*processor_id),
                    kind: SignalFlowNodeKind::Processor {
                        strip: *strip_id,
                        index,
                    },
                    label: entity.name.clone(),
                    state: if processor_unresolved(routing, *processor_id) {
                        SignalFlowNodeState::Unresolved
                    } else {
                        SignalFlowNodeState::Ready
                    },
                    runtime: SignalFlowNodeRuntime::Unobserved,
                    probe: None,
                    muted: false,
                    soloed: false,
                    processor_count: 0,
                },
            );
        }
    }

    // -- edges -----------------------------------------------------------
    let port_node = |port_id: SemanticPortId,
                     proc_nodes: &BTreeMap<ProcessorId, SignalFlowNode>|
     -> Option<SignalFlowNodeId> {
        let port = routing.ports.get(&port_id)?;
        match port.owner {
            PortOwner::Strip(strip) => Some(SignalFlowNodeId::Strip(strip)),
            PortOwner::Processor(processor) => proc_nodes
                .contains_key(&processor)
                .then_some(SignalFlowNodeId::Processor(processor))
                .or_else(|| {
                    routing
                        .strip_of_processor(processor)
                        .map(SignalFlowNodeId::Strip)
                }),
        }
    };

    let mut edges: Vec<SignalFlowEdge> = Vec::new();
    for (connection_id, connection) in &routing.connections {
        let (Some(source_port), Some(target_port)) = (
            routing.ports.get(&connection.source),
            routing.ports.get(&connection.destination),
        ) else {
            continue;
        };
        let (Some(source_node), Some(target_node)) = (
            port_node(connection.source, &proc_nodes),
            port_node(connection.destination, &proc_nodes),
        ) else {
            continue;
        };
        let (role, tap) = match connection.kind {
            ConnectionKind::Main => (SignalFlowEdgeRole::Main, None),
            ConnectionKind::Sidechain => (SignalFlowEdgeRole::Sidechain, None),
            ConnectionKind::Send(send_id) => (
                SignalFlowEdgeRole::Send,
                routing.sends.get(&send_id).map(|s| s.tap),
            ),
        };
        edges.push(SignalFlowEdge {
            id: SignalFlowEdgeId::Connection(*connection_id),
            role,
            source: SignalFlowEndpoint {
                node: source_node,
                port: Some(connection.source),
                port_role: source_port.role,
            },
            target: SignalFlowEndpoint {
                node: target_node,
                port: Some(connection.destination),
                port_role: target_port.role,
            },
            enabled: connection.enabled,
            tap,
        });
    }

    // Synthesized chain-order edges (Expanded scope only).
    if !expanded.is_empty() {
        for strip_id in &expanded {
            let Some(channel) = routing.strips.get(strip_id) else {
                continue;
            };
            let Some(chain) = routing.chains.get(&channel.chain) else {
                continue;
            };
            // Only emit chain edges when the whole chain is expanded
            // (Local{Processor} shows the node without implying order).
            if local_only_processor.is_some() {
                continue;
            }
            for (index, window) in chain.processors.windows(2).enumerate() {
                let (up, down) = (window[0], window[1]);
                if !proc_nodes.contains_key(&up) || !proc_nodes.contains_key(&down) {
                    continue;
                }
                edges.push(SignalFlowEdge {
                    id: SignalFlowEdgeId::Chain {
                        strip: *strip_id,
                        index,
                    },
                    role: SignalFlowEdgeRole::Chain,
                    source: SignalFlowEndpoint {
                        node: SignalFlowNodeId::Processor(up),
                        port: None,
                        port_role: PortRole::Main,
                    },
                    target: SignalFlowEndpoint {
                        node: SignalFlowNodeId::Processor(down),
                        port: None,
                        port_role: PortRole::Main,
                    },
                    enabled: true,
                    tap: None,
                });
            }
        }
    }

    // -- Local scope: induced 1-hop subgraph -----------------------------
    if let SignalFlowScope::Local { root } = &options.scope {
        let focus_strip = entity_strip(routing, *root);
        let focus_proc = match root {
            SignalFlowEntityRef::Processor(p) => Some(*p),
            _ => None,
        };
        let mut keep_nodes: BTreeSet<SignalFlowNodeId> = BTreeSet::new();
        if let Some(strip) = focus_strip {
            keep_nodes.insert(SignalFlowNodeId::Strip(strip));
        }
        if let Some(proc_id) = focus_proc
            && proc_nodes.contains_key(&proc_id)
        {
            keep_nodes.insert(SignalFlowNodeId::Processor(proc_id));
        }
        if keep_nodes.is_empty() {
            view.coverage = SignalFlowCoverage::Unavailable;
            return view;
        }
        // Neighbors: nodes sharing an edge with a focus node.
        let mut neighbor_nodes: BTreeSet<SignalFlowNodeId> = BTreeSet::new();
        for edge in &edges {
            let touches =
                keep_nodes.contains(&edge.source.node) || keep_nodes.contains(&edge.target.node);
            if touches {
                neighbor_nodes.insert(edge.source.node);
                neighbor_nodes.insert(edge.target.node);
            }
        }
        keep_nodes.extend(neighbor_nodes);
        strip_nodes.retain(|strip, _| keep_nodes.contains(&SignalFlowNodeId::Strip(*strip)));
        proc_nodes.retain(|proc, _| keep_nodes.contains(&SignalFlowNodeId::Processor(*proc)));
        edges.retain(|edge| {
            keep_nodes.contains(&edge.source.node) && keep_nodes.contains(&edge.target.node)
        });
    }

    // -- Role filter ------------------------------------------------------
    edges.retain(|edge| options.roles.contains(&edge.role));

    // -- Assemble nodes ----------------------------------------------------
    let mut nodes: Vec<SignalFlowNode> = strip_nodes
        .values()
        .cloned()
        .chain(proc_nodes.values().cloned())
        .collect();
    nodes.sort_by_key(|node| match node.id {
        SignalFlowNodeId::Strip(strip) => (0, strip.raw()),
        SignalFlowNodeId::Processor(processor) => (1, processor.raw()),
    });

    // -- Runtime/probe enrichment ------------------------------------------
    enrich_runtime(&mut nodes, &mut view.status, routing, evidence);

    // -- Bounds ------------------------------------------------------------
    let mut truncated = false;
    if nodes.len() > options.max_nodes {
        nodes.truncate(options.max_nodes);
        truncated = true;
    }
    let kept: BTreeSet<SignalFlowNodeId> = nodes.iter().map(|n| n.id).collect();
    edges.retain(|edge| kept.contains(&edge.source.node) && kept.contains(&edge.target.node));
    if edges.len() > options.max_edges {
        edges.truncate(options.max_edges);
        truncated = true;
    }
    if truncated {
        view.coverage = SignalFlowCoverage::Truncated;
    }

    view.nodes = nodes;
    view.edges = edges;
    view
}

/// Fills `status` from the live revision + optional evidence.
fn build_status(
    live: ProjectRevision,
    evidence: Option<&SignalFlowRuntimeEvidence>,
) -> SignalFlowStatus {
    let mut status = SignalFlowStatus {
        project_revision: live,
        engine_attached: evidence.is_some(),
        activation: None,
        planned_generation: None,
        published_generation: None,
        observed_active_generation: None,
        last_good_generation: None,
        fault: None,
        source_map_revision: None,
        source_map_generation: None,
        stale: false,
        probes: Vec::new(),
    };
    let Some(evidence) = evidence else {
        return status;
    };
    if let Some(receipt) = &evidence.receipt {
        status.activation = Some(receipt.state.clone());
        status.planned_generation = receipt.planned_generation;
        status.published_generation = receipt.published_generation;
        status.observed_active_generation = receipt.observed_active_generation;
        status.last_good_generation = receipt.last_good_generation;
        status.fault = receipt.fault;
    }
    if let Some(map) = &evidence.source_map {
        status.source_map_revision = Some(map.project_revision);
        status.source_map_generation = Some(map.generation);
        status.stale = map.project_revision != live;
    }
    status.probes = evidence
        .probes
        .iter()
        .map(|binding| SignalFlowProbeStatus {
            point: binding.point,
            state: binding.state,
            requested_mode: binding.requested_mode,
            confirmed_mode: binding.confirmed_mode,
            generation: binding.active_plan_generation,
            project_revision: binding.project_revision,
        })
        .collect();
    status
}

/// Composes per-node runtime/probe state from the evidence snapshot.
fn enrich_runtime(
    nodes: &mut [SignalFlowNode],
    status: &mut SignalFlowStatus,
    routing: &RoutingModel,
    evidence: Option<&SignalFlowRuntimeEvidence>,
) {
    let Some(evidence) = evidence else {
        return;
    };
    let map = evidence.source_map.as_ref();
    let generation = map.map(|m| m.generation);
    let stale = status.stale;

    // Node → SemanticNodeRef candidates (bus strips may resolve
    // under either Bus or Strip depending on lowering).
    let semantic_ref = |node: &SignalFlowNode| -> Vec<SemanticNodeRef> {
        match node.id {
            SignalFlowNodeId::Strip(strip) => match &node.kind {
                SignalFlowNodeKind::BusStrip { bus, .. } => {
                    vec![SemanticNodeRef::Bus(*bus), SemanticNodeRef::Strip(strip)]
                }
                _ => vec![SemanticNodeRef::Strip(strip)],
            },
            SignalFlowNodeId::Processor(processor) => {
                vec![SemanticNodeRef::Processor(processor)]
            }
        }
    };

    for node in nodes.iter_mut() {
        node.runtime = if stale {
            SignalFlowNodeRuntime::Stale
        } else if let Some(map) = map {
            let resolution = semantic_ref(node)
                .iter()
                .find_map(|key| map.semantic_nodes.get(key));
            match resolution {
                Some(MappingResolution::Resolved(_)) => SignalFlowNodeRuntime::Resolved {
                    generation: generation.unwrap_or(0),
                },
                Some(MappingResolution::Unavailable(reason)) => {
                    SignalFlowNodeRuntime::Unavailable(*reason)
                }
                Some(MappingResolution::Unsupported(reason)) => {
                    SignalFlowNodeRuntime::Unsupported(*reason)
                }
                Some(MappingResolution::NotLowered(reason)) => {
                    SignalFlowNodeRuntime::NotLowered(*reason)
                }
                None => SignalFlowNodeRuntime::Unobserved,
            }
        } else {
            SignalFlowNodeRuntime::Unobserved
        };
    }

    // Probe states: first binding (evidence order = subscription key
    // order from the app) whose point resolves to the node wins —
    // the full list stays in `status.probes`.
    for node in nodes.iter_mut() {
        node.probe = evidence
            .probes
            .iter()
            .find(|binding| probe_targets_node(&binding.point, node, routing))
            .map(|binding| binding.state);
    }
}

/// Whether a probe point's entity denotes `node`. Track/Bus/Strip
/// entities resolve through their strip; a `Processor` entity matches
/// its processor node (or — when the processor node is not projected
/// — nothing, since the probe observes the processor specifically,
/// not the whole strip). `Send` points stay list-only in
/// `status.probes`.
fn probe_targets_node(
    point: &SemanticProbePoint,
    node: &SignalFlowNode,
    routing: &RoutingModel,
) -> bool {
    let entity_strip = |entity: SemanticSignalEntityRef| -> Option<StripId> {
        match entity {
            SemanticSignalEntityRef::Track(track) => routing.strip_of_track(track).map(|s| s.id),
            SemanticSignalEntityRef::Bus(bus) => routing.buses.get(&bus).map(|b| b.strip),
            SemanticSignalEntityRef::Strip(strip) => Some(strip),
            SemanticSignalEntityRef::Processor(_) | SemanticSignalEntityRef::Send(_) => None,
            _ => None,
        }
    };
    match point.entity {
        SemanticSignalEntityRef::Processor(processor) => {
            node.id == SignalFlowNodeId::Processor(processor)
        }
        entity => {
            let Some(strip) = entity_strip(entity) else {
                return false;
            };
            node.id == SignalFlowNodeId::Strip(strip)
        }
    }
}
