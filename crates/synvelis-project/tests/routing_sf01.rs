//! SF-01 (issue #104): the persisted Semantic Routing Model — master
//! bootstrap, send↔connection 1:1, stable IDs, watermark coverage,
//! schema v5→v6 migration and one-CAS/one-undo routing commands.

use synvelis_project::*;

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "p"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn cmd(c: &ProjectController, op: ProjectOp) -> Command<ProjectOp> {
    Command {
        expected_revision: c.live_revision(),
        op,
    }
}

fn add_track(c: &mut ProjectController, name: &str) -> TrackId {
    c.apply(cmd(c, ProjectOp::AddTrack { name: name.into() }))
        .unwrap();
    *c.live().tracks.keys().next_back().unwrap()
}

/// The master strip's main input / output ports.
fn master_ports(p: &Project) -> (SemanticPortId, SemanticPortId) {
    let strip = &p.routing.strips[&p.routing.buses[&p.routing.master].strip];
    let mut input = None;
    let mut output = None;
    for id in &strip.port_ids {
        match p.routing.ports[id].direction {
            PortDirection::Input => input = Some(*id),
            PortDirection::Output => output = Some(*id),
        }
    }
    (input.unwrap(), output.unwrap())
}

/// A track's strip main output port (the send/connection source).
fn track_main_out(p: &Project, track: TrackId) -> SemanticPortId {
    let strip = p.routing.strip_of_track(track).unwrap();
    strip
        .port_ids
        .iter()
        .find(|id| p.routing.ports[id].direction == PortDirection::Output)
        .copied()
        .unwrap()
}

// ── SF-MODEL-01: master bootstrap ────────────────────────────────

#[test]
fn master_bus_exists_exactly_once_on_a_fresh_project() {
    let p = Project::new(ProjectId::new(0), "p");
    // The bootstrap master is the single logical project bus.
    assert_eq!(p.routing.buses.len(), 1);
    let master = &p.routing.buses[&p.routing.master];
    assert_eq!(master.kind, BusKind::Master);
    assert!(p.validate_references().is_ok());
    // The master is a semantic route, not a device endpoint — its
    // stable ID is a reserved well-known constant, never re-issued.
    assert!(!is_issuable_raw(p.routing.master.raw()));
}

#[test]
fn master_cannot_be_added_or_removed() {
    let mut c = controller();
    assert!(
        c.apply(cmd(
            &c,
            ProjectOp::AddBus {
                kind: BusKind::Master,
                name: "m2".into()
            }
        ))
        .is_err()
    );
    let master = c.live().routing.master;
    assert!(
        c.apply(cmd(&c, ProjectOp::RemoveBus { bus_id: master }))
            .is_err()
    );
    assert_eq!(c.live_revision(), ProjectRevision(0));
}

// ── SF-MODEL-02: send ↔ connection strict 1:1 ────────────────────

#[test]
fn add_send_creates_connection_and_config_atomically() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    c.apply(cmd(&c, ProjectOp::AddTrackStrip { track_id: track }))
        .unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::AddBus {
            kind: BusKind::Return,
            name: "rev".into(),
        },
    ))
    .unwrap();
    let rev0 = c.live_revision();

    let src = track_main_out(c.live(), track);
    let dst = master_ports(c.live()).0;
    c.apply(cmd(
        &c,
        ProjectOp::AddSend {
            source: src,
            destination: dst,
            tap: SendTap::PostFader,
            gain_target: None,
        },
    ))
    .unwrap();
    // One command → exactly one revision step (one CAS, one undo).
    assert_eq!(c.live_revision(), ProjectRevision(rev0.0 + 1));
    assert_eq!(c.live().routing.sends.len(), 1);
    let send = c.live().routing.sends.values().next().unwrap();
    let conn = &c.live().routing.connections[&send.connection];
    assert_eq!(conn.kind, ConnectionKind::Send(send.id));

    // Send connections cannot be minted through AddConnection — the
    // 1:1 relation holds by construction.
    assert!(
        c.apply(cmd(
            &c,
            ProjectOp::AddConnection {
                source: src,
                destination: dst,
                kind: ConnectionKind::Send(send.id),
                channel_map: None,
            }
        ))
        .is_err()
    );
}

#[test]
fn oracle_rejects_orphaned_and_doubly_referenced_sends() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    c.apply(cmd(&c, ProjectOp::AddTrackStrip { track_id: track }))
        .unwrap();
    let src = track_main_out(c.live(), track);
    let dst = master_ports(c.live()).0;
    c.apply(cmd(
        &c,
        ProjectOp::AddSend {
            source: src,
            destination: dst,
            tap: SendTap::PreFader,
            gain_target: None,
        },
    ))
    .unwrap();

    // Orphaned config: the send points at a removed connection.
    let mut orphan = c.live().clone();
    orphan.routing.connections.clear();
    assert!(orphan.validate_references().is_err());

    // Orphaned send edge: the connection names a send with no config.
    let mut edge = c.live().clone();
    edge.routing.sends.clear();
    assert!(edge.validate_references().is_err());

    // Broken back-reference: the config points at the connection, but
    // the connection names a different send id.
    let mut broken = c.live().clone();
    let send_id = *broken.routing.sends.keys().next().unwrap();
    let conn_id = broken.routing.sends[&send_id].connection;
    broken.routing.connections.get_mut(&conn_id).unwrap().kind =
        ConnectionKind::Send(SendId::new(999_999));
    assert!(broken.validate_references().is_err());
}

// ── SF-MODEL-03: stable ID typing ────────────────────────────────

#[test]
fn semantic_ids_are_typed_and_stable() {
    let bus = BusId::new(1);
    // Tagged roundtrip keeps kind + value.
    let tagged = bus.to_tagged();
    assert!(tagged.starts_with("bus-"));
    assert_eq!(BusId::from_tagged(&tagged).unwrap(), bus);
    // A different kind with the same raw value is a different type —
    // no numeric-equality rebinding between semantic domains.
    let strip = StripId::new(1);
    assert!(strip.to_tagged().starts_with("strip-"));
    assert_ne!(
        std::mem::discriminant(&PortOwner::Strip(strip)),
        std::mem::discriminant(&PortOwner::Processor(ProcessorId::new(1)))
    );
}

// ── SF-MODEL-04: watermark covers routing IDs ────────────────────

#[test]
fn watermark_covers_every_routing_id_and_survives_reload() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    c.apply(cmd(&c, ProjectOp::AddTrackStrip { track_id: track }))
        .unwrap();
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();

    // Reload: the generator reseeds past every persisted routing ID —
    // the next draw never collides.
    let mut c2 = controller();
    c2.load(&store).unwrap();
    // Issued strips only — the reserved bootstrap strip is a constant,
    // not an issued ID, so it never bounds the generator.
    let pre = c2
        .live()
        .routing
        .strips
        .keys()
        .map(|s| s.raw())
        .filter(|raw| is_issuable_raw(*raw))
        .max()
        .unwrap();
    c2.apply(cmd(
        &c2,
        ProjectOp::AddBus {
            kind: BusKind::Group,
            name: "g".into(),
        },
    ))
    .unwrap();
    let new_bus = c2
        .live()
        .routing
        .buses
        .values()
        .find(|b| b.kind == BusKind::Group)
        .unwrap();
    assert!(new_bus.id.raw() > pre);
    // The reserved bootstrap entities never raise the watermark: an
    // empty project's watermark stays in the generator domain.
    assert!(c2.live().id_watermark < WELL_KNOWN_BASE);
}

// ── SF-MODEL-05: serialize / reload ──────────────────────────────

#[test]
fn routing_state_survives_save_reload_unchanged() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    c.apply(cmd(&c, ProjectOp::AddTrackStrip { track_id: track }))
        .unwrap();
    let src = track_main_out(c.live(), track);
    let dst = master_ports(c.live()).0;
    c.apply(cmd(
        &c,
        ProjectOp::AddSend {
            source: src,
            destination: dst,
            tap: SendTap::PreFader,
            gain_target: None,
        },
    ))
    .unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::InsertProcessor {
            strip_id: c.live().routing.strip_of_track(track).unwrap().id,
            index: 0,
            name: "eq".into(),
            binding: ProcessorBinding::Unresolved {
                descriptor: "vendor.missing-eq@1.2.3".into(),
            },
            sidechain_input: true,
        },
    ))
    .unwrap();
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();

    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c.live(), c2.live());
    // Processor order persists through the round trip.
    let strip = c2.live().routing.strip_of_track(track).unwrap();
    let chain = &c2.live().routing.chains[&strip.chain];
    assert_eq!(chain.processors.len(), 1);
}

// ── SF-MODEL-06: v5 → v6 migration ──────────────────────────────

/// Serializes `project` as a schema-v5 envelope (the `routing` field
/// did not exist in v5). The bootstrap master-fader target
/// (synvelis#185) did not exist in v5 either — its `Strip` owner is
/// not a legacy control owner — so it is omitted here and re-seeded by
/// the v7→v8 step on load.
fn as_v5_envelope(c: &ProjectController) -> serde_json::Value {
    let mut env = serde_json::json!({
        "schema_version": 5,
        "project_id": c.live().project_id,
        "revision": c.live_revision(),
        "project": serde_json::to_value(c.live()).unwrap(),
    });
    env["project"].as_object_mut().unwrap().remove("routing");
    env["project"]["control"]["targets"]
        .as_object_mut()
        .unwrap()
        .remove(&ParameterTargetId::new(MASTER_FADER_RAW).to_tagged());
    env
}

#[test]
fn v5_snapshot_migrates_to_legacy_clip_to_master_projection() {
    let mut c = controller();
    let t1 = add_track(&mut c, "a");
    let t2 = add_track(&mut c, "b");
    let mut store = InMemoryStore::new();
    let saved_rev = c.live_revision();
    store
        .save(
            "p",
            saved_rev,
            &serde_json::to_vec(&as_v5_envelope(&c)).unwrap(),
        )
        .unwrap();

    let mut c2 = controller();
    c2.load(&store).unwrap();
    let r = &c2.live().routing;
    // Legacy projection: one strip per track, each routed to master.
    assert_eq!(r.strips.len(), 3); // 2 track strips + master strip
    assert_eq!(r.connections.len(), 2);
    for track in [t1, t2] {
        let strip = r.strip_of_track(track).unwrap();
        let out = strip
            .port_ids
            .iter()
            .find(|id| r.ports[id].direction == PortDirection::Output)
            .unwrap();
        assert!(
            r.connections
                .values()
                .any(|conn| conn.source == *out && conn.kind == ConnectionKind::Main)
        );
    }
    // No external `insert_count` FX were ever persisted — nothing is
    // fabricated as processors during migration.
    assert!(r.processors.is_empty());
    // The migrated load is one implicit revision step so a save can
    // never overwrite the v5 body.
    assert_eq!(c2.live_revision(), ProjectRevision(saved_rev.0 + 1));

    // Determinism: the same v5 body always projects identically.
    let mut c3 = controller();
    c3.load(&store).unwrap();
    assert_eq!(c2.live(), c3.live());

    // Saving the migrated project publishes a NEW snapshot name; the
    // v5 body stays immutable in the store.
    let head_before = store.head().unwrap().unwrap();
    let saved = c2.save(&mut store).unwrap();
    assert_ne!(saved.head, head_before);
}

#[test]
fn a_v5_body_carrying_routing_state_is_rejected() {
    let c = controller();
    let mut store = InMemoryStore::new();
    let mut env = as_v5_envelope(&c);
    // A "v5" body that already carries routing cannot own that state —
    // the projection would have to guess its provenance. A bootstrap
    // body alone is indistinguishable from the serde default, so the
    // probe carries a foreign strip no v5 project could produce.
    let mut routing = serde_json::to_value(&c.live().routing).unwrap();
    routing["strips"]
        .as_object_mut()
        .unwrap()
        .values_mut()
        .next()
        .unwrap()["id"] = serde_json::json!("strip-00000000000000000000000000ffffff");
    env["project"]["routing"] = routing;
    store
        .save("p", ProjectRevision(0), &serde_json::to_vec(&env).unwrap())
        .unwrap();
    let mut c2 = controller();
    assert!(c2.load(&store).is_err());
}

// ── SF-MODEL-07: unresolved bindings stay explicit ──────────────

#[test]
fn unresolved_processor_ports_stay_explicitly_unresolved() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    c.apply(cmd(&c, ProjectOp::AddTrackStrip { track_id: track }))
        .unwrap();
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    c.apply(cmd(
        &c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "ghost".into(),
            binding: ProcessorBinding::Unresolved {
                descriptor: "vendor.ghost".into(),
            },
            sidechain_input: false,
        },
    ))
    .unwrap();
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    // Missing plugin → every port keeps the explicit Unresolved
    // marker; nothing rebinds on a numeric match.
    let proc = c2.live().routing.processors.values().next().unwrap();
    assert!(matches!(proc.binding, ProcessorBinding::Unresolved { .. }));
    for port_id in &proc.port_ids {
        assert!(matches!(
            c2.live().routing.ports[port_id].binding,
            PortBinding::Unresolved { .. }
        ));
    }
}

// ── SF-MODEL-08: channel maps ───────────────────────────────────

#[test]
fn empty_channel_map_is_rejected() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    c.apply(cmd(&c, ProjectOp::AddTrackStrip { track_id: track }))
        .unwrap();
    let src = track_main_out(c.live(), track);
    let dst = master_ports(c.live()).0;
    // The strip→master main route already exists (AddTrackStrip) —
    // add a bus and route the track into it too, with an empty map.
    c.apply(cmd(
        &c,
        ProjectOp::AddBus {
            kind: BusKind::Group,
            name: "g".into(),
        },
    ))
    .unwrap();
    let group = c
        .live()
        .routing
        .buses
        .values()
        .find(|b| b.kind == BusKind::Group)
        .unwrap();
    let gdst = c.live().routing.strips[&group.strip]
        .port_ids
        .iter()
        .find(|id| c.live().routing.ports[id].direction == PortDirection::Input)
        .copied()
        .unwrap();
    assert!(
        c.apply(cmd(
            &c,
            ProjectOp::AddConnection {
                source: src,
                destination: gdst,
                kind: ConnectionKind::Main,
                channel_map: Some(ExplicitChannelMap { entries: vec![] }),
            }
        ))
        .is_err()
    );
    let _ = dst;
}

// ── SF-MODEL-09: undo covers the whole routing command ──────────

#[test]
fn undo_removes_send_connection_and_config_together() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    c.apply(cmd(&c, ProjectOp::AddTrackStrip { track_id: track }))
        .unwrap();
    let src = track_main_out(c.live(), track);
    let dst = master_ports(c.live()).0;
    c.apply(cmd(
        &c,
        ProjectOp::AddSend {
            source: src,
            destination: dst,
            tap: SendTap::PostFader,
            gain_target: None,
        },
    ))
    .unwrap();
    assert!(c.undo().unwrap());
    // One undo step reverts the whole atomic unit.
    assert!(c.live().routing.sends.is_empty());
    assert!(
        c.live()
            .routing
            .connections
            .values()
            .all(|conn| !matches!(conn.kind, ConnectionKind::Send(_)))
    );
    assert!(c.redo().unwrap());
    assert_eq!(c.live().routing.sends.len(), 1);
}

// ── SF-MODEL-10: referenced deletion is rejected ────────────────

#[test]
fn removing_a_track_with_live_connections_is_rejected() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    c.apply(cmd(&c, ProjectOp::AddTrackStrip { track_id: track }))
        .unwrap();
    // The strip→master main connection references the strip's ports —
    // a bare RemoveTrack would orphan it, so the whole command fails.
    let rev = c.live_revision();
    assert!(
        c.apply(cmd(&c, ProjectOp::RemoveTrack { track_id: track }))
            .is_err()
    );
    assert_eq!(c.live_revision(), rev);
    assert!(c.live().tracks.contains_key(&track));

    // The sanctioned path: one composite that first removes the
    // referencing connection — still a single CAS and undo step.
    let conn = c
        .live()
        .routing
        .connections
        .values()
        .find(|conn| {
            c.live()
                .routing
                .ports
                .get(&conn.source)
                .is_some_and(|p| matches!(p.owner, PortOwner::Strip(_)))
        })
        .unwrap()
        .id;
    c.apply(cmd(
        &c,
        ProjectOp::Composite {
            ops: vec![
                ProjectOp::RemoveConnection {
                    connection_id: conn,
                },
                ProjectOp::RemoveTrack { track_id: track },
            ],
        },
    ))
    .unwrap();
    assert!(!c.live().tracks.contains_key(&track));
    assert!(c.live().routing.strip_of_track(track).is_none());
}

// ── SF-MODEL-11: control target is the single gain authority ────

#[test]
fn send_gain_target_references_the_control_model() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    c.apply(cmd(&c, ProjectOp::AddTrackStrip { track_id: track }))
        .unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::AddParameterTarget {
            name: "send-lvl".into(),
            owner: ControlOwner::Track(track),
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.75,
        },
    ))
    .unwrap();
    let target = *c.live().control.targets.keys().next().unwrap();
    let src = track_main_out(c.live(), track);
    let dst = master_ports(c.live()).0;
    c.apply(cmd(
        &c,
        ProjectOp::AddSend {
            source: src,
            destination: dst,
            tap: SendTap::PostFader,
            gain_target: Some(target),
        },
    ))
    .unwrap();

    // The send adopts the target: `AddSend` seals the binding so the
    // gain target is owned by the send itself.
    let send = *c.live().routing.sends.keys().next().unwrap();
    assert_eq!(
        c.live().control.targets[&target].owner,
        ControlOwner::Send(send)
    );
    assert_eq!(
        c.live().control.targets[&target].role,
        ParameterRole::SendLevel
    );

    // The control model stays the single authority: a target still
    // referenced by a send cannot be removed.
    assert!(
        c.apply(cmd(&c, ProjectOp::RemoveParameterTarget { target }))
            .is_err()
    );
    // The send owns its level target for life: removing the send
    // cascades to the owned target, so a later removal is a NotFound.
    c.apply(cmd(&c, ProjectOp::RemoveSend { send_id: send }))
        .unwrap();
    assert!(!c.live().control.targets.contains_key(&target));
    assert!(
        c.apply(cmd(&c, ProjectOp::RemoveParameterTarget { target }))
            .is_err()
    );
}

// ── SF-MODEL-12: cycles are rejected ────────────────────────────

#[test]
fn the_strip_graph_cannot_cycle() {
    let mut c = controller();
    let t1 = add_track(&mut c, "a");
    let t2 = add_track(&mut c, "b");
    c.apply(cmd(&c, ProjectOp::AddTrackStrip { track_id: t1 }))
        .unwrap();
    c.apply(cmd(&c, ProjectOp::AddTrackStrip { track_id: t2 }))
        .unwrap();
    // Route t1 → t2's strip input, then try t2 → t1's strip input —
    // the back edge would close a zero-delay cycle.
    let t1_in = c.live().routing.strip_of_track(t1).unwrap().port_ids[0];
    let t2_in = c.live().routing.strip_of_track(t2).unwrap().port_ids[0];
    c.apply(cmd(
        &c,
        ProjectOp::AddConnection {
            source: track_main_out(c.live(), t1),
            destination: t2_in,
            kind: ConnectionKind::Main,
            channel_map: None,
        },
    ))
    .unwrap();
    assert!(
        c.apply(cmd(
            &c,
            ProjectOp::AddConnection {
                source: track_main_out(c.live(), t2),
                destination: t1_in,
                kind: ConnectionKind::Main,
                channel_map: None,
            }
        ))
        .is_err()
    );
}
