//! sv#571: `SetProcessorBinding` rebind must re-derive each owned
//! `SemanticPort.binding` at the same boundary that updates
//! `proc.binding`. The pre-fix arm refreshed only the persisted
//! plugin-instance identity — ports kept the previous plugin's stamp,
//! so a `Resolved→Unresolved` rebind left `PluginDeclared` ports the
//! routing `UnresolvedPort` rejection cannot see (the same logical
//! state `InsertProcessor`+`Unresolved` produces `Unresolved` ports
//! for), and a `Resolved(A)→Resolved(B)` rebind reported plugin A's
//! stamp against persisted fact B. These pins lock both rebind
//! directions to the insert-fresh shape and pin the
//! `resolved_endpoints` rejection parity across the two paths.

use synvelis_project::*;

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "p"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply_op(c: &mut ProjectController, op: ProjectOp) {
    let rev = c.live_revision();
    c.apply(Command {
        expected_revision: rev,
        op,
    })
    .unwrap();
}

/// A project with one track strip ready for a sidechain-capable
/// processor insert (three stamped slots: in / out / sidechain).
fn strip_project() -> (ProjectController, StripId) {
    let mut c = controller();
    apply_op(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply_op(&mut c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    (c, strip)
}

fn plugin_ref(descriptor: &str, version: &str, capability_revision: u64) -> PluginRef {
    PluginRef {
        descriptor: descriptor.into(),
        version: version.into(),
        capability_revision,
    }
}

fn insert_processor(
    c: &mut ProjectController,
    strip: StripId,
    binding: ProcessorBinding,
) -> ProcessorId {
    apply_op(
        c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "fx".into(),
            binding,
            sidechain_input: true,
        },
    );
    *c.live().routing.processors.keys().next_back().unwrap()
}

fn rebind(c: &mut ProjectController, proc: ProcessorId, binding: ProcessorBinding) {
    apply_op(
        c,
        ProjectOp::SetProcessorBinding {
            processor_id: proc,
            binding,
        },
    );
}

/// The port bindings `InsertProcessor`+`binding` produces for a fresh
/// processor — the insert-fresh reference shape a rebind must equal.
fn fresh_port_bindings(binding: ProcessorBinding) -> Vec<PortBinding> {
    let (mut c, strip) = strip_project();
    let proc = insert_processor(&mut c, strip, binding);
    c.live().routing.processors[&proc]
        .port_ids
        .iter()
        .map(|id| c.live().routing.ports[id].binding.clone())
        .collect()
}

fn port_bindings(c: &ProjectController, proc: ProcessorId) -> Vec<PortBinding> {
    c.live().routing.processors[&proc]
        .port_ids
        .iter()
        .map(|id| c.live().routing.ports[id].binding.clone())
        .collect()
}

fn strip_main_out(c: &ProjectController, strip: StripId) -> SemanticPortId {
    c.live().routing.strips[&strip]
        .port_ids
        .iter()
        .find(|id| {
            c.live().routing.ports[id].direction == PortDirection::Output
                && c.live().routing.ports[id].role == PortRole::Main
        })
        .copied()
        .unwrap()
}

// ── sv#571 AC1: Resolved→Unresolved rebind reaches UnresolvedPort ──

#[test]
fn rebind_to_unresolved_marks_every_port_unresolved() {
    let plugin_a = plugin_ref("vendor.a", "1.0.0", 3);
    let unresolved = ProcessorBinding::Unresolved {
        descriptor: "vendor.gone".into(),
    };
    let (mut c, strip) = strip_project();
    let proc = insert_processor(&mut c, strip, ProcessorBinding::Resolved(plugin_a));
    rebind(&mut c, proc, unresolved.clone());

    // Every owned port is re-stamped `Unresolved` — identical to the
    // `InsertProcessor`+`Unresolved` path's shape, slot names included.
    assert_eq!(
        port_bindings(&c, proc),
        fresh_port_bindings(unresolved),
        "rebind must produce the insert-fresh Unresolved shape"
    );
    assert!(c.live().validate_references().is_ok());
}

#[test]
fn rebind_to_unresolved_rejects_endpoints_like_insert_path() {
    let plugin_a = plugin_ref("vendor.a", "1.0.0", 3);
    let unresolved = ProcessorBinding::Unresolved {
        descriptor: "vendor.gone".into(),
    };

    // Rebind path: Resolved(A) insert, then SetProcessorBinding.
    let (mut rebind_c, strip) = strip_project();
    let proc = insert_processor(&mut rebind_c, strip, ProcessorBinding::Resolved(plugin_a));
    rebind(&mut rebind_c, proc, unresolved.clone());
    // Insert path: Unresolved at insert — the designed rejection.
    let (mut insert_c, strip_i) = strip_project();
    let proc_i = insert_processor(&mut insert_c, strip_i, unresolved);
    // Identical op sequences draw identical ids.
    assert_eq!(proc, proc_i);
    assert_eq!(strip, strip_i);

    // The capability fingerprint also agrees — no stale stamp leaks
    // into the generation an intent is authored against.
    assert_eq!(
        port_capability_generation(rebind_c.live()),
        port_capability_generation(insert_c.live())
    );

    // Parity on the typed rejection: wiring the track strip's output
    // into the processor's sidechain input — the one connection shape
    // a processor port may legally take — hits `UnresolvedPort` on
    // both paths, naming the same port.
    let out = strip_main_out(&rebind_c, strip);
    let intent = |c: &ProjectController| RoutingEditIntent {
        operation_id: "op".into(),
        expected_port_capability_generation: port_capability_generation(c.live()),
        edits: vec![RoutingEdit::CreateConnection {
            temp_id: TempConnectionId(1),
            source: PortRef::Existing(out),
            destination: PortRef::ProcessorPort {
                processor: proc,
                role: PortRole::Sidechain,
                direction: PortDirection::Input,
            },
            kind: ConnectionKind::Sidechain,
            channel_map: None,
        }],
        approved_conversion: ExplicitConversionPolicy::None,
    };
    let mut ids = CounterIdGenerator::new(rebind_c.live().id_watermark);
    let err_rebind = routing_preflight(rebind_c.live(), &intent(&rebind_c), &mut ids).unwrap_err();
    let mut ids = CounterIdGenerator::new(insert_c.live().id_watermark);
    let err_insert = routing_preflight(insert_c.live(), &intent(&insert_c), &mut ids).unwrap_err();
    assert_eq!(err_rebind, err_insert);
    assert!(
        matches!(err_rebind, RoutingEditError::UnresolvedPort { .. }),
        "{err_rebind}"
    );

    // The commit path collapses to the same typed code.
    let err = rebind_c
        .apply(Command {
            expected_revision: rebind_c.live_revision(),
            op: ProjectOp::Routing {
                intent: intent(&rebind_c),
            },
        })
        .unwrap_err();
    assert!(err.to_string().contains("routing-unresolved-port"), "{err}");
}

// ── sv#571 AC2: Resolved(A)→Resolved(B) stamps B's identity ────────

#[test]
fn rebind_resolved_to_resolved_restamps_every_port() {
    let plugin_a = plugin_ref("vendor.a", "1.0.0", 3);
    // Distinct descriptor — a same-descriptor pair cannot detect
    // descriptor staleness (the pre-fix vcs06 fixture used one).
    let plugin_b = plugin_ref("vendor.b", "2.0.0", 4);
    let (mut c, strip) = strip_project();
    let proc = insert_processor(&mut c, strip, ProcessorBinding::Resolved(plugin_a));
    rebind(&mut c, proc, ProcessorBinding::Resolved(plugin_b.clone()));

    // Every owned port declares B's identity — identical to the
    // `InsertProcessor`+`Resolved(B)` path's shape.
    assert_eq!(
        port_bindings(&c, proc),
        fresh_port_bindings(ProcessorBinding::Resolved(plugin_b.clone())),
        "rebind must produce the insert-fresh Resolved(B) shape"
    );
    for binding in port_bindings(&c, proc) {
        let PortBinding::PluginDeclared {
            plugin,
            capability_revision,
            ..
        } = &binding
        else {
            panic!("{binding:?}");
        };
        assert_eq!(plugin, &plugin_b.descriptor);
        assert_eq!(*capability_revision, plugin_b.capability_revision);
    }
    // The slot's `advertised` name is the port's own — preserved
    // across the rebind.
    let ports = &c.live().routing.ports;
    for port_id in &c.live().routing.processors[&proc].port_ids {
        let port = &ports[port_id];
        let expected = match (port.role, port.direction) {
            (PortRole::Main, PortDirection::Input) => "in",
            (PortRole::Main, PortDirection::Output) => "out",
            (PortRole::Sidechain, PortDirection::Input) => "sidechain",
            (PortRole::Sidechain, PortDirection::Output) => unreachable!("oracle-rejected shape"),
        };
        let PortBinding::PluginDeclared { advertised, .. } = &port.binding else {
            panic!("{port_id}: {:?}", port.binding);
        };
        assert_eq!(advertised, expected, "{port_id}");
    }
    assert_eq!(port_capability_generation(c.live()), {
        let (mut c2, strip2) = strip_project();
        insert_processor(&mut c2, strip2, ProcessorBinding::Resolved(plugin_b));
        port_capability_generation(c2.live())
    });
    assert!(c.live().validate_references().is_ok());
}

// ── third matrix edge: Unresolved→Resolved re-stamps too ───────────

#[test]
fn rebind_unresolved_to_resolved_stamps_and_reopens_endpoints() {
    let plugin_b = plugin_ref("vendor.b", "2.0.0", 4);
    let (mut c, strip) = strip_project();
    let proc = insert_processor(
        &mut c,
        strip,
        ProcessorBinding::Unresolved {
            descriptor: "vendor.gone".into(),
        },
    );
    rebind(&mut c, proc, ProcessorBinding::Resolved(plugin_b.clone()));

    assert_eq!(
        port_bindings(&c, proc),
        fresh_port_bindings(ProcessorBinding::Resolved(plugin_b)),
        "a recovered binding must produce the insert-fresh shape"
    );
    // The endpoints are resolvable again — a sidechain wire into the
    // processor's key input stages cleanly.
    let out = strip_main_out(&c, strip);
    let intent = RoutingEditIntent {
        operation_id: "op".into(),
        expected_port_capability_generation: port_capability_generation(c.live()),
        edits: vec![RoutingEdit::CreateConnection {
            temp_id: TempConnectionId(1),
            source: PortRef::Existing(out),
            destination: PortRef::ProcessorPort {
                processor: proc,
                role: PortRole::Sidechain,
                direction: PortDirection::Input,
            },
            kind: ConnectionKind::Sidechain,
            channel_map: None,
        }],
        approved_conversion: ExplicitConversionPolicy::None,
    };
    let mut ids = CounterIdGenerator::new(c.live().id_watermark);
    routing_preflight(c.live(), &intent, &mut ids).unwrap();
    assert!(c.live().validate_references().is_ok());
}
