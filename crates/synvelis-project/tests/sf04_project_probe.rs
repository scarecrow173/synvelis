//! SF-04 slice 1 (Issue #107): semantic probe point resolution —
//! `resolve_probe_point` maps stable `SemanticProbePoint`s through the
//! generation-scoped `RoutingSourceMap` + published `GraphSpec`,
//! fail-closed: missing references → `Unresolved`, absent physical
//! slots → `Unavailable`, out-of-domain positions → `Unsupported`,
//! `NotLowered`/`Unavailable` map entries → `Unavailable`. Nothing is
//! substituted or fabricated.

use std::collections::BTreeMap;

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_graph::{
    Activation, Availability, Direction, GraphBuilder, GraphSpec, Multiplicity, NodeDef, NodeId,
    NodeKind, PortDef, PortFormat, PortId, PortRef, PortRole as GraphPortRole,
};
use synvelis_project::probe::{
    ProbePointResolution, SemanticProbePoint, SemanticSignalEntityRef, SignalBoundary,
    resolve_probe_point,
};
use synvelis_project::*;

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0x107), "sf04"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply(c: &mut ProjectController, op: ProjectOp) {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .unwrap();
}

fn add_track_strip(c: &mut ProjectController, name: &str) -> (TrackId, StripId) {
    apply(c, ProjectOp::AddTrack { name: name.into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    (track, strip)
}

fn add_bus(c: &mut ProjectController, name: &str) -> (BusId, StripId) {
    apply(
        c,
        ProjectOp::AddBus {
            kind: BusKind::Group,
            name: name.into(),
        },
    );
    let bus = *c
        .live()
        .routing
        .buses
        .keys()
        .find(|b| **b != c.live().routing.master)
        .unwrap();
    (bus, c.live().routing.buses[&bus].strip)
}

fn strip_port(p: &Project, strip: StripId, direction: PortDirection) -> SemanticPortId {
    p.routing.strips[&strip]
        .port_ids
        .iter()
        .find(|id| {
            p.routing.ports[id].direction == direction && p.routing.ports[id].role == PortRole::Main
        })
        .copied()
        .unwrap()
}

fn processor_ports(p: &Project, processor: ProcessorId) -> (SemanticPortId, SemanticPortId, SemanticPortId) {
    let mut input = None;
    let mut output = None;
    let mut sidechain = None;
    for id in &p.routing.processors[&processor].port_ids {
        let port = &p.routing.ports[id];
        match (port.direction, port.role) {
            (PortDirection::Input, PortRole::Main) => input = Some(*id),
            (PortDirection::Output, PortRole::Main) => output = Some(*id),
            (PortDirection::Input, PortRole::Sidechain) => sidechain = Some(*id),
            _ => {}
        }
    }
    (input.unwrap(), output.unwrap(), sidechain.unwrap())
}

/// The fixture project: track strip → group bus strip (send,
/// pre-fader) with a sidechain-capable processor on the bus chain.
struct Fixture {
    controller: ProjectController,
    track: TrackId,
    strip: StripId,
    bus: BusId,
    bus_strip: StripId,
    send: SendId,
    processor: ProcessorId,
}

fn fixture() -> Fixture {
    let mut c = controller();
    let (track, strip) = add_track_strip(&mut c, "t");
    let (bus, bus_strip) = add_bus(&mut c, "fx");
    let src_out = strip_port(c.live(), strip, PortDirection::Output);
    let dst_in = strip_port(c.live(), bus_strip, PortDirection::Input);
    apply(
        &mut c,
        ProjectOp::AddSend {
            source: src_out,
            destination: dst_in,
            tap: SendTap::PreFader,
            gain_target: None,
        },
    );
    let send = *c.live().routing.sends.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::InsertProcessor {
            strip_id: bus_strip,
            index: 0,
            name: "comp".into(),
            binding: ProcessorBinding::Resolved(PluginRef {
                descriptor: "vendor.comp".into(),
                version: "1.0".into(),
                capability_revision: 7,
            }),
            sidechain_input: true,
        },
    );
    let processor = *c.live().routing.processors.keys().next_back().unwrap();
    Fixture {
        controller: c,
        track,
        strip,
        bus,
        bus_strip,
        send,
        processor,
    }
}

fn fmt() -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(48_000).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
}

fn audio_port(direction: Direction, role: GraphPortRole) -> PortDef {
    PortDef {
        direction,
        role,
        format: PortFormat::Audio(fmt()),
        multiplicity: Multiplicity::unbounded(0),
        capabilities: Vec::new(),
        availability: Availability::Optional,
        activation: Activation::Enabled,
    }
}

/// Runtime ids of one lowered fader node: `(node, in, post_out,
/// pre_out)` — the shape `routing_lower` emits (1 audio in, 2 audio
/// outs where index 1 is post-fader and index 2 the pre-fader tap).
struct FaderIds {
    node: NodeId,
    input: PortId,
    post_out: PortId,
    pre_out: PortId,
}

fn add_fader(builder: &mut GraphBuilder) -> FaderIds {
    let (node, ports) = builder.add_node(&NodeDef {
        kind: NodeKind::Processor,
        ports: vec![
            audio_port(Direction::Input, GraphPortRole::Main),
            audio_port(Direction::Output, GraphPortRole::Main),
            audio_port(Direction::Output, GraphPortRole::Main),
        ],
    });
    FaderIds {
        node,
        input: ports[0],
        post_out: ports[1],
        pre_out: ports[2],
    }
}

struct ProcessorIds {
    node: NodeId,
    input: PortId,
    output: PortId,
    sidechain: PortId,
}

fn add_processor_node(builder: &mut GraphBuilder) -> ProcessorIds {
    let (node, ports) = builder.add_node(&NodeDef {
        kind: NodeKind::Processor,
        ports: vec![
            audio_port(Direction::Input, GraphPortRole::Main),
            audio_port(Direction::Output, GraphPortRole::Main),
            audio_port(Direction::Input, GraphPortRole::Sidechain),
        ],
    });
    ProcessorIds {
        node,
        input: ports[0],
        output: ports[1],
        sidechain: ports[2],
    }
}

/// Spec + source map mirroring what `routing_lower` emits for the
/// fixture: strip main-in → fader input (empty chain), main-out →
/// fader post-out, send tap → fader pre-out, processor ports → its
/// node ports.
struct Lowered {
    spec: GraphSpec,
    map: RoutingSourceMap,
    fader_t: FaderIds,
    fader_bus: FaderIds,
    proc: ProcessorIds,
}

fn lowered(f: &Fixture) -> Lowered {
    let p = f.controller.live();
    let revision = f.controller.live_revision();
    let mut builder = GraphBuilder::new();
    let fader_t = add_fader(&mut builder);
    let fader_bus = add_fader(&mut builder);
    let proc = add_processor_node(&mut builder);
    let spec = builder.build();

    let mut semantic_nodes = BTreeMap::new();
    semantic_nodes.insert(
        SemanticNodeRef::Strip(f.strip),
        MappingResolution::Resolved(fader_t.node),
    );
    semantic_nodes.insert(
        SemanticNodeRef::Bus(f.bus),
        MappingResolution::Resolved(fader_bus.node),
    );
    semantic_nodes.insert(
        SemanticNodeRef::Processor(f.processor),
        MappingResolution::Resolved(proc.node),
    );

    let mut ports = BTreeMap::new();
    let strip_in = strip_port(p, f.strip, PortDirection::Input);
    let strip_out = strip_port(p, f.strip, PortDirection::Output);
    let bus_in = strip_port(p, f.bus_strip, PortDirection::Input);
    let bus_out = strip_port(p, f.bus_strip, PortDirection::Output);
    let (proc_in, proc_out, proc_sc) = processor_ports(p, f.processor);
    for (sem, node, port) in [
        (strip_in, fader_t.node, fader_t.input),
        (strip_out, fader_t.node, fader_t.post_out),
        (bus_in, fader_bus.node, fader_bus.input),
        (bus_out, fader_bus.node, fader_bus.post_out),
        (proc_in, proc.node, proc.input),
        (proc_out, proc.node, proc.output),
        (proc_sc, proc.node, proc.sidechain),
    ] {
        ports.insert(
            sem,
            MappingResolution::Resolved(PortRef { node, port }),
        );
    }

    let mut send_taps = BTreeMap::new();
    send_taps.insert(
        f.send,
        PortRef {
            node: fader_t.node,
            port: fader_t.pre_out,
        },
    );

    let map = RoutingSourceMap {
        project_id: p.project_id,
        project_revision: revision,
        build_key: RoutingBuildKey {
            project_id: p.project_id,
            project_revision: revision,
            routing_fingerprint: routing_fingerprint(p),
            capability_generation: port_capability_generation(p),
            engine_config_generation: 1,
            source_format_generation: 1,
            lowering_policy_version: LOWERING_POLICY_VERSION,
            build_mode: BuildMode::Realtime,
            device_clock_epoch: RuntimeClockEpochRef(1),
            runtime_instance: RuntimeInstanceId(1),
        },
        generation: 1,
        semantic_nodes,
        ports,
        connections: BTreeMap::new(),
        send_taps,
        playback_bindings: Vec::new(),
    };
    Lowered {
        spec,
        map,
        fader_t,
        fader_bus,
        proc,
    }
}

fn point(entity: SemanticSignalEntityRef, boundary: SignalBoundary) -> SemanticProbePoint {
    SemanticProbePoint {
        entity,
        boundary,
        port: None,
    }
}

fn resolve(
    f: &Fixture,
    l: &Lowered,
    point: &SemanticProbePoint,
) -> ProbePointResolution {
    resolve_probe_point(f.controller.live(), &l.map, &l.spec, point)
}

fn resolved_port(resolution: &ProbePointResolution) -> PortRef {
    match resolution {
        ProbePointResolution::Resolved(port) => *port,
        other => panic!("expected Resolved, got {other:?}"),
    }
}

// ── strip / track / bus boundaries ──────────────────────────────

#[test]
fn strip_boundaries_resolve_through_map_and_spec() {
    let f = fixture();
    let l = lowered(&f);

    let input = resolved_port(&resolve(
        &f,
        &l,
        &point(
            SemanticSignalEntityRef::Strip(f.strip),
            SignalBoundary::Input,
        ),
    ));
    assert_eq!(
        input,
        PortRef {
            node: l.fader_t.node,
            port: l.fader_t.input
        }
    );

    let output = resolved_port(&resolve(
        &f,
        &l,
        &point(
            SemanticSignalEntityRef::Strip(f.strip),
            SignalBoundary::Output,
        ),
    ));
    assert_eq!(
        output,
        PortRef {
            node: l.fader_t.node,
            port: l.fader_t.post_out
        }
    );

    // PostInsert → the fader node's single audio input.
    let post_insert = resolved_port(&resolve(
        &f,
        &l,
        &point(
            SemanticSignalEntityRef::Strip(f.strip),
            SignalBoundary::PostInsert,
        ),
    ));
    assert_eq!(post_insert, input);

    // PreFader → the fader audio output *other* than post-fader.
    let pre_fader = resolved_port(&resolve(
        &f,
        &l,
        &point(
            SemanticSignalEntityRef::Strip(f.strip),
            SignalBoundary::PreFader,
        ),
    ));
    assert_eq!(
        pre_fader,
        PortRef {
            node: l.fader_t.node,
            port: l.fader_t.pre_out
        }
    );

    // Track and bus entities resolve through their owning strip.
    let track_in = resolved_port(&resolve(
        &f,
        &l,
        &point(SemanticSignalEntityRef::Track(f.track), SignalBoundary::Input),
    ));
    assert_eq!(track_in, input);
    let bus_out = resolved_port(&resolve(
        &f,
        &l,
        &point(SemanticSignalEntityRef::Bus(f.bus), SignalBoundary::Output),
    ));
    assert_eq!(
        bus_out,
        PortRef {
            node: l.fader_bus.node,
            port: l.fader_bus.post_out
        }
    );
}

// ── processor boundaries ────────────────────────────────────────

#[test]
fn processor_boundaries_resolve_main_and_sidechain() {
    let f = fixture();
    let l = lowered(&f);
    let p = f.controller.live();
    let (proc_in, _, proc_sc) = processor_ports(p, f.processor);

    let input = resolved_port(&resolve(
        &f,
        &l,
        &point(
            SemanticSignalEntityRef::Processor(f.processor),
            SignalBoundary::Input,
        ),
    ));
    assert_eq!(
        input,
        PortRef {
            node: l.proc.node,
            port: l.proc.input
        }
    );
    let output = resolved_port(&resolve(
        &f,
        &l,
        &point(
            SemanticSignalEntityRef::Processor(f.processor),
            SignalBoundary::Output,
        ),
    ));
    assert_eq!(
        output,
        PortRef {
            node: l.proc.node,
            port: l.proc.output
        }
    );

    // Sidechain without an explicit port: the single sidechain input
    // is unambiguous.
    let sc = resolved_port(&resolve(
        &f,
        &l,
        &point(
            SemanticSignalEntityRef::Processor(f.processor),
            SignalBoundary::Sidechain,
        ),
    ));
    assert_eq!(
        sc,
        PortRef {
            node: l.proc.node,
            port: l.proc.sidechain
        }
    );

    // Explicit sidechain port agrees.
    let sc_explicit = SemanticProbePoint {
        entity: SemanticSignalEntityRef::Processor(f.processor),
        boundary: SignalBoundary::Sidechain,
        port: Some(proc_sc),
    };
    assert_eq!(resolved_port(&resolve(&f, &l, &sc_explicit)), sc);

    // An explicit port outside the sidechain set never overrides.
    let wrong = SemanticProbePoint {
        entity: SemanticSignalEntityRef::Processor(f.processor),
        boundary: SignalBoundary::Sidechain,
        port: Some(proc_in),
    };
    assert_eq!(
        resolve(&f, &l, &wrong),
        ProbePointResolution::Unresolved
    );
}

// ── send boundaries ─────────────────────────────────────────────

#[test]
fn send_boundaries_resolve_tap_and_destination() {
    let f = fixture();
    let l = lowered(&f);

    // Input → the configured (pre-fader) tap recorded in send_taps.
    let tap = resolved_port(&resolve(
        &f,
        &l,
        &point(SemanticSignalEntityRef::Send(f.send), SignalBoundary::Input),
    ));
    assert_eq!(
        tap,
        PortRef {
            node: l.fader_t.node,
            port: l.fader_t.pre_out
        }
    );

    // Output → the send connection's destination semantic port.
    let dst = resolved_port(&resolve(
        &f,
        &l,
        &point(SemanticSignalEntityRef::Send(f.send), SignalBoundary::Output),
    ));
    assert_eq!(
        dst,
        PortRef {
            node: l.fader_bus.node,
            port: l.fader_bus.input
        }
    );

    // Other boundaries on a send are outside the observable domain.
    assert_eq!(
        resolve(
            &f,
            &l,
            &point(SemanticSignalEntityRef::Send(f.send), SignalBoundary::PreFader),
        ),
        ProbePointResolution::Unsupported
    );
}

// ── explicit port validation ────────────────────────────────────

#[test]
fn explicit_port_must_agree_with_the_boundary_selection() {
    let f = fixture();
    let l = lowered(&f);
    let p = f.controller.live();
    let strip_in = strip_port(p, f.strip, PortDirection::Input);
    let strip_out = strip_port(p, f.strip, PortDirection::Output);

    let agreeing = SemanticProbePoint {
        entity: SemanticSignalEntityRef::Strip(f.strip),
        boundary: SignalBoundary::Input,
        port: Some(strip_in),
    };
    assert!(matches!(
        resolve(&f, &l, &agreeing),
        ProbePointResolution::Resolved(_)
    ));

    let disagreeing = SemanticProbePoint {
        entity: SemanticSignalEntityRef::Strip(f.strip),
        boundary: SignalBoundary::Input,
        port: Some(strip_out),
    };
    assert_eq!(
        resolve(&f, &l, &disagreeing),
        ProbePointResolution::Unresolved
    );

    // An explicit port cannot name a graph-internal position.
    let internal = SemanticProbePoint {
        entity: SemanticSignalEntityRef::Strip(f.strip),
        boundary: SignalBoundary::PostInsert,
        port: Some(strip_in),
    };
    assert_eq!(
        resolve(&f, &l, &internal),
        ProbePointResolution::Unresolved
    );
}

// ── map-state failures are explicit ─────────────────────────────

#[test]
fn map_resolution_states_surface_honestly() {
    let f = fixture();
    let mut l = lowered(&f);
    let p = f.controller.live();
    let strip_in = strip_port(p, f.strip, PortDirection::Input);
    let input_point = point(
        SemanticSignalEntityRef::Strip(f.strip),
        SignalBoundary::Input,
    );

    l.map
        .ports
        .insert(strip_in, MappingResolution::Unavailable(AvailabilityReason::PluginUnavailable));
    assert_eq!(
        resolve(&f, &l, &input_point),
        ProbePointResolution::Unavailable
    );

    l.map
        .ports
        .insert(strip_in, MappingResolution::NotLowered(NotLoweredReason::MutedStrip));
    assert_eq!(
        resolve(&f, &l, &input_point),
        ProbePointResolution::Unavailable
    );

    l.map.ports.insert(
        strip_in,
        MappingResolution::Unsupported(UnsupportedReason::TopologyUnsupported),
    );
    assert_eq!(
        resolve(&f, &l, &input_point),
        ProbePointResolution::Unsupported
    );

    // A send with no recorded tap has no physical slot.
    l.map.send_taps.clear();
    assert_eq!(
        resolve(
            &f,
            &l,
            &point(SemanticSignalEntityRef::Send(f.send), SignalBoundary::Input),
        ),
        ProbePointResolution::Unavailable
    );
}

// ── unresolved references ───────────────────────────────────────

#[test]
fn unknown_entities_and_unmapped_ports_are_unresolved() {
    let f = fixture();
    let mut l = lowered(&f);
    let p = f.controller.live();

    assert_eq!(
        resolve(
            &f,
            &l,
            &point(SemanticSignalEntityRef::Strip(StripId::new(999)), SignalBoundary::Input),
        ),
        ProbePointResolution::Unresolved
    );
    assert_eq!(
        resolve(
            &f,
            &l,
            &point(
                SemanticSignalEntityRef::Processor(ProcessorId::new(999)),
                SignalBoundary::Input,
            ),
        ),
        ProbePointResolution::Unresolved
    );

    // A semantic port absent from the map has no runtime identity.
    let strip_out = strip_port(p, f.strip, PortDirection::Output);
    l.map.ports.remove(&strip_out);
    assert_eq!(
        resolve(
            &f,
            &l,
            &point(
                SemanticSignalEntityRef::Strip(f.strip),
                SignalBoundary::Output,
            ),
        ),
        ProbePointResolution::Unresolved
    );
}

// ── strip-internal positions need a physical slot ───────────────

#[test]
fn strip_internal_positions_require_the_fader_slot() {
    let f = fixture();
    let mut l = lowered(&f);
    let post_insert = point(
        SemanticSignalEntityRef::Strip(f.strip),
        SignalBoundary::PostInsert,
    );
    let pre_fader = point(
        SemanticSignalEntityRef::Strip(f.strip),
        SignalBoundary::PreFader,
    );

    // Fader node absent from the spec → no physical slot (an empty
    // spec issues no node IDs, so the mapped node cannot exist).
    let empty_spec = GraphBuilder::new().build();
    let no_node = resolve_probe_point(
        f.controller.live(),
        &l.map,
        &empty_spec,
        &post_insert,
    );
    assert_eq!(no_node, ProbePointResolution::Unavailable);

    // Semantic node mapping unavailable / not lowered / unsupported.
    l.map.semantic_nodes.insert(
        SemanticNodeRef::Strip(f.strip),
        MappingResolution::Unavailable(AvailabilityReason::InstanceMissing),
    );
    assert_eq!(
        resolve(&f, &l, &post_insert),
        ProbePointResolution::Unavailable
    );
    l.map.semantic_nodes.insert(
        SemanticNodeRef::Strip(f.strip),
        MappingResolution::NotLowered(NotLoweredReason::MutedStrip),
    );
    assert_eq!(
        resolve(&f, &l, &post_insert),
        ProbePointResolution::Unavailable
    );
    l.map.semantic_nodes.insert(
        SemanticNodeRef::Strip(f.strip),
        MappingResolution::Unsupported(UnsupportedReason::TopologyUnsupported),
    );
    assert_eq!(
        resolve(&f, &l, &post_insert),
        ProbePointResolution::Unsupported
    );
    l.map.semantic_nodes.remove(&SemanticNodeRef::Strip(f.strip));
    assert_eq!(
        resolve(&f, &l, &post_insert),
        ProbePointResolution::Unresolved
    );

    // Restore and give the fader node two audio inputs — ambiguous.
    l.map.semantic_nodes.insert(
        SemanticNodeRef::Strip(f.strip),
        MappingResolution::Resolved(l.fader_t.node),
    );
    let mut builder = GraphBuilder::new();
    let (wide, wide_ports) = builder.add_node(&NodeDef {
        kind: NodeKind::Processor,
        ports: vec![
            audio_port(Direction::Input, GraphPortRole::Main),
            audio_port(Direction::Input, GraphPortRole::Aux),
            audio_port(Direction::Output, GraphPortRole::Main),
            audio_port(Direction::Output, GraphPortRole::Main),
        ],
    });
    // Point the map at the ambiguous node.
    l.map.semantic_nodes.insert(
        SemanticNodeRef::Strip(f.strip),
        MappingResolution::Resolved(wide),
    );
    // The semantic main-out port must resolve to one of the node's
    // outputs for PreFader to find "the other" output — keep the
    // original post_out id, which does not exist on `wide`, so the
    // map entry is stale: update it to wide's first output.
    let p = f.controller.live();
    let strip_out = strip_port(p, f.strip, PortDirection::Output);
    l.map.ports.insert(
        strip_out,
        MappingResolution::Resolved(PortRef {
            node: wide,
            port: wide_ports[2],
        }),
    );
    let wide_spec = builder.build();
    assert_eq!(
        resolve_probe_point(f.controller.live(), &l.map, &wide_spec, &post_insert),
        ProbePointResolution::Unavailable
    );

    // A single non-audio input is not an audio tap.
    let mut builder = GraphBuilder::new();
    let (control_node, _) = builder.add_node(&NodeDef {
        kind: NodeKind::Processor,
        ports: vec![PortDef {
            direction: Direction::Input,
            role: GraphPortRole::Main,
            format: PortFormat::Control,
            multiplicity: Multiplicity::unbounded(0),
            capabilities: Vec::new(),
            availability: Availability::Optional,
            activation: Activation::Enabled,
        }],
    });
    l.map.semantic_nodes.insert(
        SemanticNodeRef::Strip(f.strip),
        MappingResolution::Resolved(control_node),
    );
    let control_spec = builder.build();
    assert_eq!(
        resolve_probe_point(f.controller.live(), &l.map, &control_spec, &post_insert),
        ProbePointResolution::Unavailable
    );

    // PreFader with no distinct second audio output → Unavailable.
    let mut builder = GraphBuilder::new();
    let (single_out, ports) = builder.add_node(&NodeDef {
        kind: NodeKind::Processor,
        ports: vec![
            audio_port(Direction::Input, GraphPortRole::Main),
            audio_port(Direction::Output, GraphPortRole::Main),
        ],
    });
    l.map.semantic_nodes.insert(
        SemanticNodeRef::Strip(f.strip),
        MappingResolution::Resolved(single_out),
    );
    l.map.ports.insert(
        strip_out,
        MappingResolution::Resolved(PortRef {
            node: single_out,
            port: ports[1],
        }),
    );
    let single_spec = builder.build();
    assert_eq!(
        resolve_probe_point(f.controller.live(), &l.map, &single_spec, &pre_fader),
        ProbePointResolution::Unavailable
    );
}
