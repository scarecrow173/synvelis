//! SF-03 slice 8 acceptance (synvelis#106): the workspace-side
//! same-Revision fixture. `synvelis-workspace` never sees player or
//! app handles — runtime truth arrives only as the
//! [`SignalFlowRuntimeEvidence`] snapshot the app composes from
//! `SessionRoutingEngine`. These tests pin the projection contract:
//!
//! - `session.revision()`, the `EnginePort` notification revision,
//!   `signal_flow_graph`'s `project_revision`, and every revision
//!   field inside synthesized evidence (receipt build key, source
//!   map, probe binding) name one committed revision.
//! - Generation-scoped source-map evidence is consulted per semantic
//!   node — a `MappingResolution` for `SemanticNodeRef::Strip`/`Bus`
//!   lands on exactly that entity's node, nothing is fabricated.
//! - A revision transition (apply/undo/redo) makes prior-generation
//!   evidence read `stale` with `SignalFlowNodeRuntime::Stale`
//!   nodes — old runtime identity is never presented as current.
//! - `save` publishes a snapshot HEAD; it is persistence evidence on
//!   the same revision axis, not engine/activation evidence.

use std::collections::BTreeMap;

use synvelis_project::{
    AvailabilityReason, BusId, BusKind, CounterIdGenerator, InMemoryStore, MappingResolution,
    PortDirection, PortRole, ProbeMode, ProbeResolveState, Project, ProjectController, ProjectId,
    ProjectOp, ProjectRevision, RoutingActivationReceipt, RoutingActivationState, RoutingBuildKey,
    RoutingSourceMap, RuntimeClockEpochRef, RuntimeInstanceId, SemanticNodeRef, SemanticPortId,
    SemanticProbeBinding, SemanticProbePoint, SemanticSignalEntityRef, SignalBoundary, StripId,
    TrackId, routing_fingerprint,
};
use synvelis_workspace::{
    ProjectSession, SignalFlowNodeId, SignalFlowNodeRuntime, SignalFlowRuntimeEvidence,
    SignalFlowViewOptions, signal_flow_graph,
};

// ---------------------------------------------------------------------------
// fixture
// ---------------------------------------------------------------------------

fn fresh_session() -> ProjectSession {
    let project = Project::new(ProjectId::new(0x106), "sf03s8-test");
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

fn connect_strips(session: &mut ProjectSession, source: StripId, destination: StripId) {
    apply(
        session,
        ProjectOp::AddConnection {
            source: strip_main_port(session, source, PortDirection::Output),
            destination: strip_main_port(session, destination, PortDirection::Input),
            kind: synvelis_project::ConnectionKind::Main,
            channel_map: None,
        },
    );
}

/// Track strip → group bus → master, plus a lane so the fixture
/// mirrors a minimal real session. Returns
/// `(session, track_strip, group_bus)`.
fn build_fixture() -> (ProjectSession, StripId, BusId) {
    let mut session = fresh_session();
    let track = add_track(&mut session, "vox");
    let track_strip = add_track_strip(&mut session, track);
    let bus = add_bus(&mut session, BusKind::Group, "music");
    let group_strip = bus_strip(&session, bus);
    let master_strip = bus_strip(&session, session.project().routing.master);
    connect_strips(&mut session, track_strip, group_strip);
    connect_strips(&mut session, group_strip, master_strip);
    apply(&mut session, ProjectOp::AddLane { track_id: track });
    (session, track_strip, bus)
}

// ---------------------------------------------------------------------------
// synthesized runtime evidence — the same shape the app composes from
// `SessionRoutingEngine` (`receipt` + `source_map` + `probe_bindings`)
// ---------------------------------------------------------------------------

/// Builds the evidence snapshot a real engine would vend for a plan
/// built from `revision` at `generation`. `unavailable` names the
/// semantic nodes whose runtime mapping is typed-unavailable — every
/// other node honestly projects `Unobserved` (no fabricated
/// `Resolved`).
fn evidence_at(
    session: &ProjectSession,
    revision: ProjectRevision,
    generation: u64,
    probe_strip: StripId,
    unavailable: &[SemanticNodeRef],
) -> SignalFlowRuntimeEvidence {
    let project = session.project();
    let fingerprint = routing_fingerprint(project);
    let key = RoutingBuildKey {
        project_id: project.project_id,
        project_revision: revision,
        routing_fingerprint: fingerprint,
        capability_generation: 1,
        engine_config_generation: 1,
        source_format_generation: 1,
        lowering_policy_version: synvelis_project::LOWERING_POLICY_VERSION,
        build_mode: synvelis_project::BuildMode::Realtime,
        device_clock_epoch: RuntimeClockEpochRef(1),
        runtime_instance: RuntimeInstanceId(1),
    };
    let semantic_nodes = unavailable
        .iter()
        .map(|node| {
            (
                *node,
                MappingResolution::Unavailable(AvailabilityReason::PluginUnavailable),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let source_map = RoutingSourceMap {
        project_id: project.project_id,
        project_revision: revision,
        build_key: key.clone(),
        generation,
        semantic_nodes,
        ports: BTreeMap::new(),
        connections: BTreeMap::new(),
        send_taps: BTreeMap::new(),
        playback_bindings: Vec::new(),
    };
    let receipt = RoutingActivationReceipt {
        request_id: None,
        build_key: Some(key),
        state: RoutingActivationState::Active {
            project_revision: revision,
            generation,
            epoch: RuntimeClockEpochRef(1),
        },
        planned_generation: Some(generation),
        published_generation: Some(generation),
        observed_active_generation: Some(generation),
        last_good_generation: Some(generation),
        frames_observed: 0,
        fault: None,
    };
    let probe = SemanticProbeBinding {
        point: SemanticProbePoint {
            entity: SemanticSignalEntityRef::Strip(probe_strip),
            boundary: SignalBoundary::PostFader,
            port: None,
        },
        project_revision: revision,
        runtime_instance_id: RuntimeInstanceId(1),
        active_plan_generation: generation,
        source_map_fingerprint: fingerprint,
        runtime_probe_id: None,
        requested_mode: ProbeMode::Monitor,
        confirmed_mode: None,
        state: ProbeResolveState::BoundActive,
        last_status: None,
    };
    SignalFlowRuntimeEvidence {
        receipt: Some(receipt),
        source_map: Some(source_map),
        probes: vec![probe],
    }
}

fn overview() -> SignalFlowViewOptions {
    SignalFlowViewOptions::default()
}

// ---------------------------------------------------------------------------
// same-Revision fixture
// ---------------------------------------------------------------------------

/// `session.revision()`, the `EnginePort` notification revision, the
/// view's `project_revision`, and every revision field inside the
/// evidence snapshot name the same committed revision — and the
/// generation-scoped map resolves exactly the semantic nodes it
/// records.
#[test]
fn session_engine_projection_and_evidence_share_one_revision() {
    let (session, track_strip, group_bus) = build_fixture();
    let rev = session.revision();

    // EnginePort observed every committed revision; the last
    // notification names the live revision.
    assert_eq!(session.engine().graph_builds(), 6);
    assert_eq!(session.engine().last_built_revision(), rev);

    let group_strip = bus_strip(&session, group_bus);
    let evidence = evidence_at(
        &session,
        rev,
        7,
        track_strip,
        &[
            SemanticNodeRef::Strip(track_strip),
            SemanticNodeRef::Bus(group_bus),
        ],
    );
    let view = signal_flow_graph(&session, &overview(), Some(&evidence));

    // One revision axis everywhere.
    assert_eq!(view.project_revision, rev);
    assert_eq!(view.status.project_revision, rev);
    assert!(view.status.engine_attached);
    assert!(!view.status.stale);
    assert_eq!(view.status.source_map_revision, Some(rev));
    assert_eq!(view.status.source_map_generation, Some(7));
    assert!(matches!(
        view.status.activation,
        Some(RoutingActivationState::Active {
            project_revision,
            generation: 7,
            ..
        }) if project_revision == rev
    ));
    assert_eq!(view.status.probes.len(), 1);
    assert_eq!(view.status.probes[0].project_revision, rev);
    assert_eq!(view.status.probes[0].generation, 7);

    // The generation-scoped map is consulted per semantic node:
    // recorded entities project their typed resolution, unrecorded
    // entities stay honestly `Unobserved`.
    let track_node = view
        .nodes
        .iter()
        .find(|n| n.id == SignalFlowNodeId::Strip(track_strip))
        .expect("track strip node");
    assert_eq!(
        track_node.runtime,
        SignalFlowNodeRuntime::Unavailable(AvailabilityReason::PluginUnavailable)
    );
    // A bus strip node resolves through `SemanticNodeRef::Bus` first.
    let bus_node = view
        .nodes
        .iter()
        .find(|n| n.id == SignalFlowNodeId::Strip(group_strip))
        .expect("bus strip node");
    assert_eq!(
        bus_node.runtime,
        SignalFlowNodeRuntime::Unavailable(AvailabilityReason::PluginUnavailable)
    );
    let master_strip = bus_strip(&session, session.project().routing.master);
    let master_node = view
        .nodes
        .iter()
        .find(|n| n.id == SignalFlowNodeId::Strip(master_strip))
        .expect("master strip node");
    assert_eq!(master_node.runtime, SignalFlowNodeRuntime::Unobserved);
    // The probe binding's point lands on the track strip node.
    assert_eq!(track_node.probe, Some(ProbeResolveState::BoundActive));
}

/// A revision transition makes prior-generation evidence stale:
/// apply, undo and redo each advance `session.revision()` and notify
/// the engine port, while the old source map keeps naming the
/// revision it was built from — the projection marks it `stale`
/// instead of silently reusing it.
#[test]
fn revision_transitions_stale_prior_generation_evidence() {
    let (mut session, track_strip, _group_bus) = build_fixture();
    let rev1 = session.revision();
    let evidence_v1 = evidence_at(&session, rev1, 3, track_strip, &[]);

    // Baseline: evidence built at the live revision is current.
    let view = signal_flow_graph(&session, &overview(), Some(&evidence_v1));
    assert!(!view.status.stale);
    assert_eq!(view.status.source_map_revision, Some(rev1));

    // apply → rev2: engine port notified, old evidence now stale.
    let rev2 = apply(
        &mut session,
        ProjectOp::SetProjectName { name: "v2".into() },
    );
    assert_eq!(session.engine().last_built_revision(), rev2);
    let view = signal_flow_graph(&session, &overview(), Some(&evidence_v1));
    assert_eq!(view.project_revision, rev2);
    assert_eq!(view.status.project_revision, rev2);
    assert_eq!(view.status.source_map_revision, Some(rev1));
    assert!(view.status.stale);
    assert!(
        view.nodes
            .iter()
            .all(|n| n.runtime == SignalFlowNodeRuntime::Stale)
    );
    // The probe binding still names the revision it was requested
    // against — honest old-revision evidence, not rewritten.
    assert_eq!(view.status.probes[0].project_revision, rev1);

    // undo → rev3 (a new committed revision, engine notified again).
    assert!(session.undo().expect("undo"));
    let rev3 = session.revision();
    assert_ne!(rev3, rev2);
    assert_eq!(session.engine().last_built_revision(), rev3);
    let view = signal_flow_graph(&session, &overview(), Some(&evidence_v1));
    assert_eq!(view.status.project_revision, rev3);
    assert!(view.status.stale);

    // redo → rev4; fresh evidence at the live revision is current
    // again — replacement, not accumulation of stale identity.
    assert!(session.redo().expect("redo"));
    let rev4 = session.revision();
    assert_eq!(session.engine().last_built_revision(), rev4);
    let evidence_v4 = evidence_at(&session, rev4, 4, track_strip, &[]);
    let view = signal_flow_graph(&session, &overview(), Some(&evidence_v4));
    assert_eq!(view.status.project_revision, rev4);
    assert_eq!(view.status.source_map_revision, Some(rev4));
    assert_eq!(view.status.source_map_generation, Some(4));
    assert!(!view.status.stale);
    assert_eq!(view.status.probes[0].project_revision, rev4);
}

/// `save` publishes an immutable snapshot HEAD on the same revision
/// axis — persistence evidence only. It never notifies the engine
/// port, never advances the revision, and never implies activation:
/// the saved revision is exactly the committed revision the
/// evidence names.
#[test]
fn save_is_persistence_evidence_on_the_same_revision_axis() {
    let (mut session, track_strip, _group_bus) = build_fixture();
    let rev = session.revision();
    let builds_before = session.engine().graph_builds();
    let evidence = evidence_at(&session, rev, 9, track_strip, &[]);

    // No save yet — honest absence.
    assert!(session.controller().saved().is_none());

    let mut store = InMemoryStore::new();
    session
        .controller_mut()
        .save(&mut store)
        .expect("save fixture");
    let saved = session.controller().saved().expect("saved state").clone();
    assert_eq!(saved.revision, rev);

    // Save touched nothing on the engine/projection axes.
    assert_eq!(session.revision(), rev);
    assert_eq!(session.engine().graph_builds(), builds_before);
    assert_eq!(session.engine().last_built_revision(), rev);
    let view = signal_flow_graph(&session, &overview(), Some(&evidence));
    assert_eq!(view.status.project_revision, rev);
    assert_eq!(view.status.source_map_revision, Some(rev));
    assert!(!view.status.stale);
    // The snapshot HEAD names the same committed revision — save is
    // not a second revision axis and not activation evidence.
    assert_eq!(saved.revision, view.status.source_map_revision.unwrap());
}
