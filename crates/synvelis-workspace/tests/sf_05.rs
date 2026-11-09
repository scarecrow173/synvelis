//! SF-05 slice 1 (synvelis#108): the headless Signal Flow read model.
//!
//! Covers the workspace half of the slice's acceptance:
//!
//! - SF-UX-01 — Overview/Local/Expanded/Collapse/Role-Filter
//!   projections preserve routing meaning; every persisted
//!   `RoutingConnection` yields exactly one edge (a `Send` connection
//!   plus its `SendConfig` is one Send edge, never two).
//! - SF-UX-02 — `SignalFlowVisualState` mutations are pure value
//!   operations; read-model queries never touch the session.
//! - SF-UX-05 — absent engine evidence composes honest
//!   `engine_attached = false` / `Unobserved` status.
//!
//! Fixture mutations go through `ProjectSession::apply` +
//! `ProjectOp` only; the functions under test are
//! `signal_flow_graph`/`signal_flow_graph_with` and the `visual_*`
//! free functions.

use synvelis_project::{
    BusId, BusKind, ConnectionKind, CounterIdGenerator, PluginRef, PortDirection, PortRole,
    ProcessorBinding, ProcessorId, Project, ProjectController, ProjectId, ProjectOp,
    ProjectRevision, SemanticPortId, SendTap, StripId, TrackId,
};
use synvelis_workspace::{
    ProjectSession, SignalFlowCoverage, SignalFlowEdgeId, SignalFlowEdgeRole, SignalFlowEntityRef,
    SignalFlowLayer, SignalFlowNodeId, SignalFlowNodeRuntime, SignalFlowNodeState, SignalFlowScope,
    SignalFlowViewOptions, SignalFlowVisualGroup, SignalFlowVisualState, signal_flow_graph,
    visual_fold, visual_move_node, visual_set_group, visual_set_link_focus, visual_set_pan,
    visual_set_scope, visual_set_zoom,
};

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

fn fresh_session() -> ProjectSession {
    let project = Project::new(ProjectId::new(0x108), "sf05-test");
    ProjectSession::new(ProjectController::new(
        project,
        Box::new(CounterIdGenerator::new(1)),
    ))
}

fn apply(session: &mut ProjectSession, op: ProjectOp) -> ProjectRevision {
    let rev = session.revision();
    session.apply(op, rev).expect("fixture op")
}

fn add_track(session: &mut ProjectSession, name: &str) -> TrackId {
    apply(session, ProjectOp::AddTrack { name: name.into() });
    *session.project().tracks.keys().next_back().expect("track")
}

fn add_track_strip(session: &mut ProjectSession, track: TrackId) -> StripId {
    apply(session, ProjectOp::AddTrackStrip { track_id: track });
    session.project().routing.strip_of_track(track).unwrap().id
}

fn add_bus(session: &mut ProjectSession, kind: BusKind, name: &str) -> BusId {
    apply(
        session,
        ProjectOp::AddBus {
            kind,
            name: name.into(),
        },
    );
    session
        .project()
        .routing
        .buses
        .values()
        .find(|b| b.name == name)
        .unwrap()
        .id
}

fn bus_strip(session: &ProjectSession, bus: BusId) -> StripId {
    session.project().routing.buses[&bus].strip
}

/// A strip's main port in `direction`.
fn strip_main_port(
    session: &ProjectSession,
    strip: StripId,
    direction: PortDirection,
) -> SemanticPortId {
    session.project().routing.strips[&strip]
        .port_ids
        .iter()
        .find(|id| {
            let p = &session.project().routing.ports[id];
            p.direction == direction && p.role == PortRole::Main
        })
        .copied()
        .expect("strip main port")
}

/// A processor's sidechain input port.
fn processor_sidechain_port(session: &ProjectSession, processor: ProcessorId) -> SemanticPortId {
    session.project().routing.processors[&processor]
        .port_ids
        .iter()
        .find(|id| {
            let p = &session.project().routing.ports[id];
            p.direction == PortDirection::Input && p.role == PortRole::Sidechain
        })
        .copied()
        .expect("sidechain port")
}

fn add_processor(
    session: &mut ProjectSession,
    strip: StripId,
    index: usize,
    name: &str,
    sidechain_input: bool,
) -> ProcessorId {
    apply(
        session,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index,
            name: name.into(),
            binding: ProcessorBinding::Resolved(PluginRef {
                descriptor: format!("vendor.{name}"),
                version: "1.0".into(),
                capability_revision: 1,
            }),
            sidechain_input,
        },
    );
    *session
        .project()
        .routing
        .processors
        .iter()
        .find(|(_, p)| p.name == name)
        .map(|(id, _)| id)
        .expect("processor")
}

fn add_connection(
    session: &mut ProjectSession,
    source: SemanticPortId,
    destination: SemanticPortId,
    kind: ConnectionKind,
) {
    apply(
        session,
        ProjectOp::AddConnection {
            source,
            destination,
            kind,
            channel_map: None,
        },
    );
}

fn add_send(
    session: &mut ProjectSession,
    source: SemanticPortId,
    destination: SemanticPortId,
    tap: SendTap,
) {
    apply(
        session,
        ProjectOp::AddSend {
            source,
            destination,
            tap,
            gain_target: None,
        },
    );
}

/// The Issue #108 fixture topology:
///
/// ```text
/// bass ──Main──▶ music(Group) ──Main──▶ master
///  │  └─Main──▶ master            (AddTrackStrip auto route)
///  ├─Send(Pre)─▶ verb(Return)
///  └─Send(Post)▶ verb(Return)
/// kick ──Main──▶ master
/// kick ──Sidechain──▶ comp.processor on music strip
/// vox  ──Main──▶ master
/// bass strip chain: [eq, comp2]  (Expanded order evidence)
/// ```
struct Fixture {
    bass: TrackId,
    kick: TrackId,
    bass_strip: StripId,
    kick_strip: StripId,
    vox_strip: StripId,
    music_strip: StripId,
    verb_strip: StripId,
    master_strip: StripId,
    eq: ProcessorId,
    comp2: ProcessorId,
    sidechain_proc: ProcessorId,
}

fn build_fixture() -> (ProjectSession, Fixture) {
    let mut session = fresh_session();

    let bass = add_track(&mut session, "bass");
    let kick = add_track(&mut session, "kick");
    let vox = add_track(&mut session, "vox");
    let bass_strip = add_track_strip(&mut session, bass);
    let kick_strip = add_track_strip(&mut session, kick);
    let vox_strip = add_track_strip(&mut session, vox);

    let music_bus = add_bus(&mut session, BusKind::Group, "music");
    let music_strip = bus_strip(&session, music_bus);
    let verb_bus = add_bus(&mut session, BusKind::Return, "verb");
    let verb_strip = bus_strip(&session, verb_bus);
    let master_strip = bus_strip(&session, session.project().routing.master);

    // Bass → Music Bus → Master main path (bass keeps its auto
    // strip→master route — parallel fan-out is legal routing).
    let bass_out = strip_main_port(&session, bass_strip, PortDirection::Output);
    let music_in = strip_main_port(&session, music_strip, PortDirection::Input);
    let music_out = strip_main_port(&session, music_strip, PortDirection::Output);
    let master_in = strip_main_port(&session, master_strip, PortDirection::Input);
    add_connection(&mut session, bass_out, music_in, ConnectionKind::Main);
    add_connection(&mut session, music_out, master_in, ConnectionKind::Main);

    // Kick → sidechain input of a processor on the music strip.
    let sidechain_proc = add_processor(&mut session, music_strip, 0, "comp", true);
    let kick_out = strip_main_port(&session, kick_strip, PortDirection::Output);
    let sc_in = processor_sidechain_port(&session, sidechain_proc);
    add_connection(&mut session, kick_out, sc_in, ConnectionKind::Sidechain);

    // Pre/Post sends → Return bus (one Send connection + one
    // SendConfig each — must project as exactly one Send edge apiece).
    let vox_out = strip_main_port(&session, vox_strip, PortDirection::Output);
    let verb_in = strip_main_port(&session, verb_strip, PortDirection::Input);
    add_send(&mut session, vox_out, verb_in, SendTap::PreFader);
    add_send(&mut session, bass_out, verb_in, SendTap::PostFader);

    // Two-processor chain on the bass strip — Expanded order evidence.
    let eq = add_processor(&mut session, bass_strip, 0, "eq", false);
    let comp2 = add_processor(&mut session, bass_strip, 1, "comp2", false);

    (
        session,
        Fixture {
            bass,
            kick,
            bass_strip,
            kick_strip,
            vox_strip,
            music_strip,
            verb_strip,
            master_strip,
            eq,
            comp2,
            sidechain_proc,
        },
    )
}

fn overview() -> SignalFlowViewOptions {
    SignalFlowViewOptions::default()
}

fn expanded() -> SignalFlowViewOptions {
    SignalFlowViewOptions {
        scope: SignalFlowScope::Expanded,
        ..SignalFlowViewOptions::default()
    }
}

fn node_strips(view: &synvelis_workspace::SignalFlowView) -> Vec<StripId> {
    view.nodes
        .iter()
        .filter_map(|n| match n.id {
            SignalFlowNodeId::Strip(strip) => Some(strip),
            SignalFlowNodeId::Processor(_) => None,
        })
        .collect()
}

fn has_node(view: &synvelis_workspace::SignalFlowView, id: SignalFlowNodeId) -> bool {
    view.nodes.iter().any(|n| n.id == id)
}

// ---------------------------------------------------------------------------
// SF-UX-01: hierarchy display preserves routing meaning
// ---------------------------------------------------------------------------

#[test]
fn overview_projects_every_strip_and_every_connection_exactly_once() {
    let (session, f) = build_fixture();
    let view = signal_flow_graph(&session, &overview(), None);

    assert_eq!(view.coverage, SignalFlowCoverage::Complete);
    assert_eq!(view.status.project_revision, session.revision());

    // Every strip is a node; no processor nodes in Overview.
    let routing = &session.project().routing;
    assert_eq!(node_strips(&view).len(), routing.strips.len());
    for strip in [
        f.bass_strip,
        f.kick_strip,
        f.vox_strip,
        f.music_strip,
        f.verb_strip,
        f.master_strip,
    ] {
        assert!(has_node(&view, SignalFlowNodeId::Strip(strip)), "{strip:?}");
    }
    assert!(
        view.nodes
            .iter()
            .all(|n| matches!(n.id, SignalFlowNodeId::Strip(_)))
    );

    // Edge dedup contract: one edge per persisted RoutingConnection —
    // a Send connection + its SendConfig is exactly one Send edge.
    let connection_edges: Vec<_> = view
        .edges
        .iter()
        .filter(|e| matches!(e.id, SignalFlowEdgeId::Connection(_)))
        .collect();
    assert_eq!(connection_edges.len(), routing.connections.len());
    let mut seen = std::collections::BTreeSet::new();
    for edge in &connection_edges {
        let SignalFlowEdgeId::Connection(id) = edge.id else {
            unreachable!()
        };
        assert!(seen.insert(id), "duplicate edge for {id:?}");
    }
    assert_eq!(seen.len(), routing.connections.len());
    // SendConfig never mints a second edge: #sends == #Send edges.
    assert_eq!(
        view.edges
            .iter()
            .filter(|e| e.role == SignalFlowEdgeRole::Send)
            .count(),
        routing.sends.len()
    );
    // No synthesized chain edges outside Expanded.
    assert!(
        view.edges
            .iter()
            .all(|e| e.role != SignalFlowEdgeRole::Chain)
    );

    // Required topology is present with the right roles.
    let has_edge = |role: SignalFlowEdgeRole,
                    src: Option<StripId>,
                    dst: Option<StripId>,
                    tap: Option<SendTap>| {
        view.edges.iter().any(|e| {
            e.role == role
                && e.tap == tap
                && src.is_none_or(|s| e.source.node == SignalFlowNodeId::Strip(s))
                && dst.is_none_or(|d| e.target.node == SignalFlowNodeId::Strip(d))
        })
    };
    assert!(has_edge(
        SignalFlowEdgeRole::Main,
        Some(f.bass_strip),
        Some(f.music_strip),
        None
    ));
    assert!(has_edge(
        SignalFlowEdgeRole::Main,
        Some(f.music_strip),
        Some(f.master_strip),
        None
    ));
    assert!(has_edge(
        SignalFlowEdgeRole::Send,
        Some(f.vox_strip),
        Some(f.verb_strip),
        Some(SendTap::PreFader)
    ));
    assert!(has_edge(
        SignalFlowEdgeRole::Send,
        Some(f.bass_strip),
        Some(f.verb_strip),
        Some(SendTap::PostFader)
    ));
    // Kick → sidechain: the edge targets the processor's strip in
    // Overview (processor nodes are not projected), role stays
    // Sidechain — never masquerades as Main.
    let sidechain = view
        .edges
        .iter()
        .find(|e| e.role == SignalFlowEdgeRole::Sidechain)
        .expect("sidechain edge");
    assert_eq!(sidechain.source.node, SignalFlowNodeId::Strip(f.kick_strip));
    assert_eq!(
        sidechain.target.node,
        SignalFlowNodeId::Strip(f.music_strip)
    );
}

#[test]
fn expanded_exposes_processor_nodes_and_ordered_chain_edges() {
    let (session, f) = build_fixture();
    let view = signal_flow_graph(&session, &expanded(), None);

    assert_eq!(view.coverage, SignalFlowCoverage::Complete);

    // Processor nodes exist with their chain position.
    for (processor, index) in [(f.eq, 0usize), (f.comp2, 1), (f.sidechain_proc, 0)] {
        let node = view
            .nodes
            .iter()
            .find(|n| n.id == SignalFlowNodeId::Processor(processor))
            .unwrap_or_else(|| panic!("processor node {processor:?}"));
        assert!(matches!(
            &node.kind,
            synvelis_workspace::SignalFlowNodeKind::Processor { index: i, .. } if *i == index
        ));
        assert_eq!(node.state, SignalFlowNodeState::Ready);
    }

    // Chain order: bass strip [eq → comp2] yields exactly one Chain
    // edge in that direction; single-processor strips yield none.
    let chain_edges: Vec<_> = view
        .edges
        .iter()
        .filter(|e| e.role == SignalFlowEdgeRole::Chain)
        .collect();
    assert_eq!(chain_edges.len(), 1);
    let chain = chain_edges[0];
    assert_eq!(chain.source.node, SignalFlowNodeId::Processor(f.eq));
    assert_eq!(chain.target.node, SignalFlowNodeId::Processor(f.comp2));
    assert_eq!(
        chain.id,
        SignalFlowEdgeId::Chain {
            strip: f.bass_strip,
            index: 0
        }
    );

    // The sidechain edge now anchors to the processor node itself.
    let sidechain = view
        .edges
        .iter()
        .find(|e| e.role == SignalFlowEdgeRole::Sidechain)
        .expect("sidechain edge");
    assert_eq!(
        sidechain.target.node,
        SignalFlowNodeId::Processor(f.sidechain_proc)
    );
}

#[test]
fn collapse_folds_processor_expansion_without_losing_the_strip() {
    let (session, f) = build_fixture();
    let options = SignalFlowViewOptions {
        scope: SignalFlowScope::Expanded,
        collapsed: [SignalFlowEntityRef::Strip(f.bass_strip)]
            .into_iter()
            .collect(),
        ..SignalFlowViewOptions::default()
    };
    let view = signal_flow_graph(&session, &options, None);

    // Bass strip node and its routing edges survive; only its
    // processor expansion is folded.
    assert!(has_node(&view, SignalFlowNodeId::Strip(f.bass_strip)));
    assert!(!view.nodes.iter().any(|n| matches!(
        &n.kind,
        synvelis_workspace::SignalFlowNodeKind::Processor { strip, .. } if *strip == f.bass_strip
    )));
    assert!(
        view.edges.iter().all(
            |e| !matches!(e.id, SignalFlowEdgeId::Chain { strip, .. } if strip == f.bass_strip)
        )
    );
    // Other strips keep their expansion (music strip's processor).
    assert!(has_node(
        &view,
        SignalFlowNodeId::Processor(f.sidechain_proc)
    ));
    // Routing edges touching the folded strip are unaffected.
    assert!(view.edges.iter().any(|e| {
        e.role == SignalFlowEdgeRole::Main && e.source.node == SignalFlowNodeId::Strip(f.bass_strip)
    }));
}

#[test]
fn local_scope_projects_the_root_and_its_direct_neighborhood() {
    let (session, f) = build_fixture();
    let options = SignalFlowViewOptions {
        scope: SignalFlowScope::Local {
            root: SignalFlowEntityRef::Track(f.bass),
        },
        ..SignalFlowViewOptions::default()
    };
    let view = signal_flow_graph(&session, &options, None);

    assert_eq!(view.coverage, SignalFlowCoverage::Complete);
    assert!(has_node(&view, SignalFlowNodeId::Strip(f.bass_strip)));
    // Direct neighbors: music bus (Main edge), master (auto Main +
    // music path is 2-hop so only direct edges count), verb return
    // (Send edge). Kick is two hops away — excluded.
    assert!(has_node(&view, SignalFlowNodeId::Strip(f.music_strip)));
    assert!(has_node(&view, SignalFlowNodeId::Strip(f.master_strip)));
    assert!(has_node(&view, SignalFlowNodeId::Strip(f.verb_strip)));
    assert!(!has_node(&view, SignalFlowNodeId::Strip(f.kick_strip)));
    assert!(!has_node(&view, SignalFlowNodeId::Strip(f.vox_strip)));
    // No retained edge reaches outside the projected neighborhood.
    assert!(view.edges.iter().all(|e| {
        e.source.node != SignalFlowNodeId::Strip(f.kick_strip)
            && e.target.node != SignalFlowNodeId::Strip(f.kick_strip)
    }));
}

#[test]
fn local_processor_root_expands_only_that_processor() {
    let (session, f) = build_fixture();
    let options = SignalFlowViewOptions {
        scope: SignalFlowScope::Local {
            root: SignalFlowEntityRef::Processor(f.eq),
        },
        ..SignalFlowViewOptions::default()
    };
    let view = signal_flow_graph(&session, &options, None);

    assert_eq!(view.coverage, SignalFlowCoverage::Complete);
    assert!(has_node(&view, SignalFlowNodeId::Strip(f.bass_strip)));
    assert!(has_node(&view, SignalFlowNodeId::Processor(f.eq)));
    // Sibling processor stays folded — Local shows the root only.
    assert!(!has_node(&view, SignalFlowNodeId::Processor(f.comp2)));
    // No chain-order implication in Local.
    assert!(
        view.edges
            .iter()
            .all(|e| e.role != SignalFlowEdgeRole::Chain)
    );
}

#[test]
fn local_unknown_root_is_unavailable() {
    let (session, _) = build_fixture();
    for root in [
        SignalFlowEntityRef::Strip(StripId::new(999_999)),
        SignalFlowEntityRef::Track(TrackId::new(999_999)),
        SignalFlowEntityRef::Bus(BusId::new(999_999)),
        SignalFlowEntityRef::Processor(ProcessorId::new(999_999)),
    ] {
        let options = SignalFlowViewOptions {
            scope: SignalFlowScope::Local { root },
            ..SignalFlowViewOptions::default()
        };
        let view = signal_flow_graph(&session, &options, None);
        assert_eq!(view.coverage, SignalFlowCoverage::Unavailable, "{root:?}");
        assert!(view.nodes.is_empty());
        assert!(view.edges.is_empty());
    }
}

#[test]
fn role_filter_narrows_edges_without_dropping_nodes() {
    let (session, f) = build_fixture();
    let options = SignalFlowViewOptions {
        scope: SignalFlowScope::Expanded,
        roles: [SignalFlowEdgeRole::Send].into_iter().collect(),
        ..SignalFlowViewOptions::default()
    };
    let view = signal_flow_graph(&session, &options, None);

    assert_eq!(view.coverage, SignalFlowCoverage::Complete);
    assert!(!view.edges.is_empty());
    assert!(
        view.edges
            .iter()
            .all(|e| e.role == SignalFlowEdgeRole::Send)
    );
    // Nodes are not filtered — the graph stays navigable.
    assert_eq!(
        node_strips(&view).len(),
        session.project().routing.strips.len()
    );
    assert!(has_node(&view, SignalFlowNodeId::Processor(f.eq)));
    // Chain edges filtered out with the Chain role absent.
    assert!(
        view.edges
            .iter()
            .all(|e| e.role != SignalFlowEdgeRole::Chain)
    );

    // Excluding Chain in Expanded keeps processor nodes but drops
    // order edges — filter is on edges, not expansion.
    let options = SignalFlowViewOptions {
        scope: SignalFlowScope::Expanded,
        roles: [
            SignalFlowEdgeRole::Main,
            SignalFlowEdgeRole::Sidechain,
            SignalFlowEdgeRole::Send,
        ]
        .into_iter()
        .collect(),
        ..SignalFlowViewOptions::default()
    };
    let view = signal_flow_graph(&session, &options, None);
    assert!(has_node(&view, SignalFlowNodeId::Processor(f.eq)));
    assert!(
        view.edges
            .iter()
            .all(|e| e.role != SignalFlowEdgeRole::Chain)
    );
}

// ---------------------------------------------------------------------------
// coverage edge cases
// ---------------------------------------------------------------------------

#[test]
fn history_scope_and_empty_layers_are_unsupported() {
    let (session, _) = build_fixture();
    let history = SignalFlowViewOptions {
        scope: SignalFlowScope::History,
        ..SignalFlowViewOptions::default()
    };
    let view = signal_flow_graph(&session, &history, None);
    assert_eq!(view.coverage, SignalFlowCoverage::Unsupported);
    assert!(view.nodes.is_empty());

    let no_layers = SignalFlowViewOptions {
        layers: Default::default(),
        ..SignalFlowViewOptions::default()
    };
    let view = signal_flow_graph(&session, &no_layers, None);
    assert_eq!(view.coverage, SignalFlowCoverage::Unsupported);
}

#[test]
fn expected_revision_mismatch_is_stale_with_empty_graph() {
    let (session, _) = build_fixture();
    let live = session.revision();
    let options = SignalFlowViewOptions {
        expected_revision: Some(ProjectRevision(live.0 + 1)),
        ..SignalFlowViewOptions::default()
    };
    let view = signal_flow_graph(&session, &options, None);

    assert_eq!(view.coverage, SignalFlowCoverage::Stale);
    assert!(view.nodes.is_empty());
    assert!(view.edges.is_empty());
    // Status still reports the live revision — Stale describes the
    // query, not a fabricated project state.
    assert_eq!(view.status.project_revision, live);
    assert_eq!(view.project_revision, live);

    // Matching expected_revision proceeds normally.
    let options = SignalFlowViewOptions {
        expected_revision: Some(live),
        ..SignalFlowViewOptions::default()
    };
    let view = signal_flow_graph(&session, &options, None);
    assert_eq!(view.coverage, SignalFlowCoverage::Complete);
}

#[test]
fn node_and_edge_bounds_truncate_deterministically() {
    let (session, _) = build_fixture();
    let routing = &session.project().routing;

    // max_nodes = 1 → deterministic prefix (lowest strip id wins the
    // sort), coverage Truncated.
    let options = SignalFlowViewOptions {
        max_nodes: 1,
        ..SignalFlowViewOptions::default()
    };
    let view = signal_flow_graph(&session, &options, None);
    assert_eq!(view.coverage, SignalFlowCoverage::Truncated);
    assert_eq!(view.nodes.len(), 1);
    // Edges touching the dropped nodes are filtered with them.
    assert!(
        view.edges
            .iter()
            .all(|e| view.nodes.iter().any(|n| n.id == e.source.node)
                && view.nodes.iter().any(|n| n.id == e.target.node))
    );

    // Generous node bound + tight edge bound → Truncated via edges.
    let options = SignalFlowViewOptions {
        max_nodes: routing.strips.len() + 8,
        max_edges: 1,
        ..SignalFlowViewOptions::default()
    };
    let view = signal_flow_graph(&session, &options, None);
    assert_eq!(view.coverage, SignalFlowCoverage::Truncated);
    assert_eq!(view.edges.len(), 1);

    // Same query twice → identical prefix (deterministic).
    let again = signal_flow_graph(&session, &options, None);
    assert_eq!(view.nodes, again.nodes);
    assert_eq!(view.edges, again.edges);
}

// ---------------------------------------------------------------------------
// SF-UX-02 (workspace half): reads and visual ops never mutate
// ---------------------------------------------------------------------------

#[test]
fn graph_queries_and_visual_ops_leave_the_session_untouched() {
    let (session, f) = build_fixture();
    let before_rev = session.revision();
    let before_project = serde_json::to_value(session.project()).unwrap();

    // A spread of read-model queries.
    for options in [
        overview(),
        expanded(),
        SignalFlowViewOptions {
            scope: SignalFlowScope::Local {
                root: SignalFlowEntityRef::Track(f.bass),
            },
            ..SignalFlowViewOptions::default()
        },
        SignalFlowViewOptions {
            scope: SignalFlowScope::History,
            ..SignalFlowViewOptions::default()
        },
    ] {
        let _ = signal_flow_graph(&session, &options, None);
    }

    // Every visual-only mutation on a visual state.
    let mut visual = SignalFlowVisualState::default();
    visual_move_node(
        &mut visual,
        SignalFlowEntityRef::Strip(f.bass_strip),
        [12.5, -3.0],
    );
    visual_set_group(
        &mut visual,
        SignalFlowVisualGroup {
            id: 1,
            label: "drums".into(),
            members: [SignalFlowEntityRef::Track(f.kick)].into_iter().collect(),
            collapsed: false,
        },
    );
    visual_fold(&mut visual, SignalFlowEntityRef::Strip(f.bass_strip), true);
    visual_set_zoom(&mut visual, 2.0);
    visual_set_pan(&mut visual, [4.0, 5.0]);
    visual_set_link_focus(&mut visual, Some(SignalFlowEntityRef::Track(f.bass)));
    visual_set_scope(&mut visual, SignalFlowScope::Expanded);

    // The view options derived from visual state still only *read*
    // the project.
    let options = visual.view_options(64, 128);
    let _ = signal_flow_graph(&session, &options, None);

    assert_eq!(session.revision(), before_rev);
    assert_eq!(
        serde_json::to_value(session.project()).unwrap(),
        before_project,
        "read model / visual ops must not mutate the project"
    );
}

#[test]
fn visual_state_is_a_pure_value_with_monotone_revision() {
    let mut visual = SignalFlowVisualState::default();
    assert_eq!(visual.visual_revision, 0);
    assert_eq!(visual.zoom, 1.0);

    visual_move_node(
        &mut visual,
        SignalFlowEntityRef::Strip(StripId::new(1)),
        [1.0, 2.0],
    );
    assert_eq!(visual.visual_revision, 1);
    visual_set_zoom(&mut visual, 0.0); // rejected — no bump
    assert_eq!(visual.visual_revision, 1);
    visual_set_zoom(&mut visual, -2.0); // rejected
    assert_eq!(visual.visual_revision, 1);
    visual_set_zoom(&mut visual, f64::NAN); // rejected
    assert_eq!(visual.visual_revision, 1);
    visual_set_zoom(&mut visual, 1.5);
    assert_eq!(visual.visual_revision, 2);
    assert_eq!(visual.zoom, 1.5);

    visual_fold(
        &mut visual,
        SignalFlowEntityRef::Strip(StripId::new(2)),
        true,
    );
    visual_fold(
        &mut visual,
        SignalFlowEntityRef::Strip(StripId::new(2)),
        false,
    );
    assert!(
        !visual
            .collapsed
            .contains(&SignalFlowEntityRef::Strip(StripId::new(2)))
    );
    visual_set_scope(&mut visual, SignalFlowScope::Expanded);

    let options = visual.view_options(10, 20);
    assert_eq!(options.scope, SignalFlowScope::Expanded);
    assert_eq!(options.view_generation, visual.visual_revision);
    assert_eq!(options.max_nodes, 10);
    assert_eq!(options.max_edges, 20);
    // Layers default keeps Audio — never an empty filter.
    assert!(options.layers.contains(&SignalFlowLayer::Audio));
}

// ---------------------------------------------------------------------------
// SF-UX-05 (workspace half): absent engine → honest unobserved status
// ---------------------------------------------------------------------------

#[test]
fn absent_engine_evidence_reports_unobserved_not_success() {
    let (session, _) = build_fixture();
    let view = signal_flow_graph(&session, &overview(), None);

    assert!(!view.status.engine_attached);
    assert_eq!(view.status.activation, None);
    assert_eq!(view.status.published_generation, None);
    assert_eq!(view.status.observed_active_generation, None);
    assert!(!view.status.stale);
    assert!(view.status.probes.is_empty());
    assert!(
        view.nodes
            .iter()
            .all(|n| n.runtime == SignalFlowNodeRuntime::Unobserved && n.probe.is_none())
    );
}
