//! SF-02 (issue #105): the routing *transaction* boundary — intent
//! staging on a scratch candidate, typed rejection, one-CAS/one-undo
//! commit, temporary→stable ID receipts, explicit-conversion policy,
//! generic `ruviform-graph` validation, and the mute/solo audibility
//! authority. Commit stays exclusively on `ProjectController::apply`.

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

fn add_track_strip(c: &mut ProjectController, track: TrackId) -> StripId {
    c.apply(cmd(c, ProjectOp::AddTrackStrip { track_id: track }))
        .unwrap();
    c.live().routing.strip_of_track(track).unwrap().id
}

fn add_bus(c: &mut ProjectController, kind: BusKind, name: &str) -> BusId {
    c.apply(cmd(
        c,
        ProjectOp::AddBus {
            kind,
            name: name.into(),
        },
    ))
    .unwrap();
    // `next_back` cannot be used — the well-known master BusId is
    // always the maximum.
    c.live()
        .routing
        .buses
        .values()
        .find(|b| b.name == name)
        .unwrap()
        .id
}

fn master_strip(p: &Project) -> StripId {
    p.routing.buses[&p.routing.master].strip
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

fn intent(c: &ProjectController, edits: Vec<RoutingEdit>) -> RoutingEditIntent {
    RoutingEditIntent {
        operation_id: "op".into(),
        expected_port_capability_generation: port_capability_generation(c.live()),
        edits,
        approved_conversion: ExplicitConversionPolicy::None,
    }
}

fn apply_intent(c: &mut ProjectController, intent: RoutingEditIntent) -> Result<(), Error> {
    c.apply(cmd(c, ProjectOp::Routing { intent })).map(|_| ())
}

fn strip_main(strip: StripId, direction: PortDirection) -> PortRef {
    PortRef::StripMain {
        strip: StripRef::Existing(strip),
        direction,
    }
}

fn bus_main(bus: BusRef, direction: PortDirection) -> PortRef {
    PortRef::StripMain {
        strip: StripRef::OfBus(bus),
        direction,
    }
}

// ── SF02-ACC-01: Bass → Bus → Master as one transaction ─────────

#[test]
fn a_multi_edit_intent_commits_as_one_revision_and_one_undo() {
    let mut c = controller();
    let bass = add_track(&mut c, "bass");
    let bass_strip = add_track_strip(&mut c, bass);
    let master = master_strip(c.live());
    let (rev_before, conns_before, buses_before) = (
        c.live_revision(),
        c.live().routing.connections.len(),
        c.live().routing.buses.len(),
    );
    let before = serde_json::to_value(c.live()).unwrap();

    let temp_bus = TempBusId(1);
    let (t_c1, t_c2) = (TempConnectionId(1), TempConnectionId(2));
    let intent = intent(
        &c,
        vec![
            RoutingEdit::CreateBus {
                temp_id: temp_bus,
                kind: BusKind::Group,
                name: "drum bus".into(),
            },
            RoutingEdit::CreateConnection {
                temp_id: t_c1,
                source: strip_main(bass_strip, PortDirection::Output),
                destination: bus_main(BusRef::Temp(temp_bus), PortDirection::Input),
                kind: ConnectionKind::Main,
                channel_map: None,
            },
            RoutingEdit::CreateConnection {
                temp_id: t_c2,
                source: bus_main(BusRef::Temp(temp_bus), PortDirection::Output),
                destination: strip_main(master, PortDirection::Input),
                kind: ConnectionKind::Main,
                channel_map: None,
            },
        ],
    );
    apply_intent(&mut c, intent).unwrap();

    // Exactly one revision step for the whole gesture.
    assert_eq!(c.live_revision().0, rev_before.0 + 1);
    assert_eq!(c.live().routing.buses.len(), buses_before + 1);
    // +2 authored edges (+1 auto strip→master edge from AddTrackStrip).
    assert_eq!(c.live().routing.connections.len(), conns_before + 2);
    let bus = c
        .live()
        .routing
        .buses
        .values()
        .find(|b| b.kind == BusKind::Group)
        .unwrap();
    let bass_out = strip_port(c.live(), bass_strip, PortDirection::Output);
    let bus_in = strip_port(c.live(), bus.strip, PortDirection::Input);
    let bus_out = strip_port(c.live(), bus.strip, PortDirection::Output);
    let master_in = strip_port(c.live(), master, PortDirection::Input);
    assert!(
        c.live()
            .routing
            .connections
            .values()
            .any(|k| k.source == bass_out && k.destination == bus_in)
    );
    assert!(
        c.live()
            .routing
            .connections
            .values()
            .any(|k| k.source == bus_out && k.destination == master_in)
    );

    // One undo restores the exact pre-gesture state.
    c.undo().unwrap();
    assert_eq!(serde_json::to_value(c.live()).unwrap(), before);
}

// ── SF02-ACC-02: Pre/Post sends → Return bus ─────────────────────

#[test]
fn pre_and_post_sends_reach_a_return_bus_in_one_intent() {
    let mut c = controller();
    let track = add_track(&mut c, "vox");
    let strip = add_track_strip(&mut c, track);
    let ret = add_bus(&mut c, BusKind::Return, "verb");
    let ret_strip = c.live().routing.buses[&ret].strip;
    // The return must reach the master for the send graph to close.
    let intent = intent(
        &c,
        vec![
            RoutingEdit::CreateConnection {
                temp_id: TempConnectionId(10),
                source: strip_main(ret_strip, PortDirection::Output),
                destination: strip_main(master_strip(c.live()), PortDirection::Input),
                kind: ConnectionKind::Main,
                channel_map: None,
            },
            RoutingEdit::CreateSend {
                temp_id: TempSendId(1),
                temp_connection: TempConnectionId(1),
                source: strip_main(strip, PortDirection::Output),
                destination: strip_main(ret_strip, PortDirection::Input),
                tap: SendTap::PreFader,
                level: SendGainSpec::Create {
                    name: "pre verb".into(),
                    initial: 1.0,
                },
                channel_map: None,
            },
            RoutingEdit::CreateSend {
                temp_id: TempSendId(2),
                temp_connection: TempConnectionId(2),
                source: strip_main(strip, PortDirection::Output),
                destination: strip_main(ret_strip, PortDirection::Input),
                tap: SendTap::PostFader,
                level: SendGainSpec::Create {
                    name: "to verb".into(),
                    initial: 0.75,
                },
                channel_map: None,
            },
        ],
    );
    apply_intent(&mut c, intent).unwrap();

    let sends: Vec<&SendConfig> = c.live().routing.sends.values().collect();
    assert_eq!(sends.len(), 2);
    assert!(sends.iter().any(|s| s.tap == SendTap::PreFader));
    assert!(sends.iter().any(|s| s.tap == SendTap::PostFader));
    // Every send edge is a `Send`-kind connection pointing back at the
    // send — never a `Main` edge sharing the namespace.
    for send in &sends {
        let conn = &c.live().routing.connections[&send.connection];
        assert_eq!(conn.kind, ConnectionKind::Send(send.id));
        assert_eq!(
            conn.destination,
            strip_port(c.live(), ret_strip, PortDirection::Input)
        );
    }
    // `Create` produced a Send-owned SendLevel target — the single
    // persisted gain authority (CTL-02: every new send carries one).
    let post_send = sends.iter().find(|s| s.tap == SendTap::PostFader).unwrap();
    let pre_send = sends.iter().find(|s| s.tap == SendTap::PreFader).unwrap();
    assert!(pre_send.gain_target.is_some());
    let created = post_send.gain_target.unwrap();
    let target = &c.live().control.targets[&created];
    assert_eq!(target.owner, ControlOwner::Send(post_send.id));
    assert_eq!(target.role, ParameterRole::SendLevel);
    assert!((target.base - 0.75).abs() < f64::EPSILON);
}

// ── SF02-ACC-03: Kick → Compressor sidechain ──────────────────────

#[test]
fn a_sidechain_connection_joins_strip_output_to_processor_key() {
    let mut c = controller();
    let kick = add_track(&mut c, "kick");
    let kick_strip = add_track_strip(&mut c, kick);
    let bus = add_bus(&mut c, BusKind::Group, "drums");
    let bus_strip = c.live().routing.buses[&bus].strip;
    c.apply(cmd(
        &c,
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
    ))
    .unwrap();
    let processor = *c.live().routing.processors.keys().next_back().unwrap();

    let intent = intent(
        &c,
        vec![RoutingEdit::CreateConnection {
            temp_id: TempConnectionId(1),
            source: strip_main(kick_strip, PortDirection::Output),
            destination: PortRef::ProcessorPort {
                processor,
                role: PortRole::Sidechain,
                direction: PortDirection::Input,
            },
            kind: ConnectionKind::Sidechain,
            channel_map: None,
        }],
    );
    apply_intent(&mut c, intent).unwrap();
    let conn = c
        .live()
        .routing
        .connections
        .values()
        .find(|k| k.kind == ConnectionKind::Sidechain)
        .unwrap();
    assert_eq!(
        conn.source,
        strip_port(c.live(), kick_strip, PortDirection::Output)
    );
    let sc_port = c.live().routing.ports[&conn.destination].clone();
    assert_eq!(sc_port.role, PortRole::Sidechain);
    assert_eq!(sc_port.owner, PortOwner::Processor(processor));
}

// ── SF02-REJ-01: stale preconditions ──────────────────────────────

#[test]
fn a_stale_expected_revision_rejects_before_any_staging() {
    let mut c = controller();
    let i = intent(&c, vec![]);
    let err = c
        .apply(Command {
            expected_revision: ProjectRevision(99),
            op: ProjectOp::Routing { intent: i },
        })
        .unwrap_err();
    assert!(matches!(err, Error::StaleRevision { .. }));
}

#[test]
fn a_stale_port_capability_generation_rejects() {
    let mut c = controller();
    let mut i = intent(&c, vec![]);
    i.expected_port_capability_generation += 1;
    let err = apply_intent(&mut c, i).unwrap_err();
    assert!(
        err.to_string().contains("routing-stale-capability"),
        "{err}"
    );
}

#[test]
fn reconfiguring_capabilities_invalidates_an_authored_intent() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    c.apply(cmd(
        &c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "fx".into(),
            binding: ProcessorBinding::Resolved(PluginRef {
                descriptor: "vendor.fx".into(),
                version: "1.0".into(),
                capability_revision: 3,
            }),
            sidechain_input: false,
        },
    ))
    .unwrap();
    let authored = intent(&c, vec![]);
    // A rebind bumps the generation fingerprint — an intent authored
    // against the old view must reject, not silently rewire.
    c.apply(cmd(
        &c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "fx2".into(),
            binding: ProcessorBinding::Resolved(PluginRef {
                descriptor: "vendor.fx2".into(),
                version: "2.0".into(),
                capability_revision: 9,
            }),
            sidechain_input: false,
        },
    ))
    .unwrap();
    let err = apply_intent(&mut c, authored).unwrap_err();
    assert!(
        err.to_string().contains("routing-stale-capability"),
        "{err}"
    );
}

// ── SF02-REJ-02: endpoint legality ────────────────────────────────

#[test]
fn unknown_dangling_and_unresolved_ports_reject_typed() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    let master = master_strip(c.live());
    let out = strip_port(c.live(), strip, PortDirection::Output);
    let dst = strip_main(master, PortDirection::Input);

    // Unknown port ID.
    let i = intent(
        &c,
        vec![RoutingEdit::CreateConnection {
            temp_id: TempConnectionId(1),
            source: PortRef::Existing(SemanticPortId::new(0xDEAD_BEEF)),
            destination: dst,
            kind: ConnectionKind::Main,
            channel_map: None,
        }],
    );
    let err = apply_intent(&mut c, i).unwrap_err();
    assert!(err.to_string().contains("routing-dangling"), "{err}");

    // Input as a source — direction violation.
    let i = intent(
        &c,
        vec![RoutingEdit::CreateConnection {
            temp_id: TempConnectionId(1),
            source: strip_main(strip, PortDirection::Input),
            destination: dst,
            kind: ConnectionKind::Main,
            channel_map: None,
        }],
    );
    let err = apply_intent(&mut c, i).unwrap_err();
    assert!(
        err.to_string().contains("routing-invalid-domain-direction"),
        "{err}"
    );

    // An unresolved plugin port can never carry a wire — missing
    // plugins are never auto-rebound.
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
    let ghost = *c.live().routing.processors.keys().next_back().unwrap();
    let i = intent(
        &c,
        vec![RoutingEdit::CreateConnection {
            temp_id: TempConnectionId(1),
            source: PortRef::Existing(out),
            destination: PortRef::ProcessorPort {
                processor: ghost,
                role: PortRole::Main,
                direction: PortDirection::Input,
            },
            kind: ConnectionKind::Main,
            channel_map: None,
        }],
    );
    let err = apply_intent(&mut c, i).unwrap_err();
    assert!(err.to_string().contains("routing-unresolved-port"), "{err}");

    // `Send(_)` is not a legal `CreateConnection` kind.
    let i = intent(
        &c,
        vec![RoutingEdit::CreateConnection {
            temp_id: TempConnectionId(1),
            source: PortRef::Existing(out),
            destination: dst,
            kind: ConnectionKind::Send(SendId::new(1)),
            channel_map: None,
        }],
    );
    let err = apply_intent(&mut c, i).unwrap_err();
    assert!(
        err.to_string().contains("routing-invalid-domain-direction"),
        "{err}"
    );
}

// ── SF02-REJ-03: explicit conversion policy ───────────────────────

#[test]
fn channel_layout_mismatch_needs_an_explicit_approved_map() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    let master = master_strip(c.live());
    let out = strip_port(c.live(), strip, PortDirection::Output);
    let bus = add_bus(&mut c, BusKind::Group, "g");
    let bus_in = c.live().routing.buses[&bus].strip;
    // Mutate the staging base: both the strip output and the master
    // input read mono, so the pre-existing strip→master edge stays a
    // consistent 1→1 — only the new edge to the still-stereo bus input
    // mismatches.
    let mut p = c.live().clone();
    p.routing.ports.get_mut(&out).unwrap().channels = 1;
    let master_in = strip_port(&p, master, PortDirection::Input);
    p.routing.ports.get_mut(&master_in).unwrap().channels = 1;
    let mut ids = CounterIdGenerator::new(p.id_watermark);
    let mk = |approved_conversion, channel_map: Option<ExplicitChannelMap>| RoutingEditIntent {
        operation_id: "op".into(),
        expected_port_capability_generation: port_capability_generation(&p),
        edits: vec![RoutingEdit::CreateConnection {
            temp_id: TempConnectionId(1),
            source: strip_main(strip, PortDirection::Output),
            destination: strip_main(bus_in, PortDirection::Input),
            kind: ConnectionKind::Main,
            channel_map,
        }],
        approved_conversion,
    };

    // No map at all on a 1→2 mismatch → decision required.
    let err =
        routing_preflight(&p, &mk(ExplicitConversionPolicy::Approved, None), &mut ids).unwrap_err();
    assert!(
        err.to_string().contains("routing-requires-decision"),
        "{err}"
    );

    // A map without the approved policy → still a decision, not a
    // silent upmix.
    let map = || {
        Some(ExplicitChannelMap {
            entries: vec![0, 0],
        })
    };
    let err =
        routing_preflight(&p, &mk(ExplicitConversionPolicy::None, map()), &mut ids).unwrap_err();
    assert!(
        err.to_string().contains("routing-requires-decision"),
        "{err}"
    );

    // Approved + well-formed map → staged.
    routing_preflight(&p, &mk(ExplicitConversionPolicy::Approved, map()), &mut ids).unwrap();

    // A map that reads a channel the source doesn't have → malformed.
    let err = routing_preflight(
        &p,
        &mk(
            ExplicitConversionPolicy::Approved,
            Some(ExplicitChannelMap {
                entries: vec![0, 3],
            }),
        ),
        &mut ids,
    )
    .unwrap_err();
    assert!(err.to_string().contains("routing-unsupported-map"), "{err}");
}

// ── SF02-REJ-04: cycles ───────────────────────────────────────────

#[test]
fn a_feedback_cycle_rejects_as_a_typed_cycle() {
    let mut c = controller();
    let (a, b) = (add_track(&mut c, "a"), add_track(&mut c, "b"));
    let (gtr_strip, vox_strip) = (add_track_strip(&mut c, a), add_track_strip(&mut c, b));
    // Both strips already feed the master; crossing them closes a loop.
    let i = intent(
        &c,
        vec![
            RoutingEdit::CreateConnection {
                temp_id: TempConnectionId(1),
                source: strip_main(gtr_strip, PortDirection::Output),
                destination: strip_main(vox_strip, PortDirection::Input),
                kind: ConnectionKind::Main,
                channel_map: None,
            },
            RoutingEdit::CreateConnection {
                temp_id: TempConnectionId(2),
                source: strip_main(vox_strip, PortDirection::Output),
                destination: strip_main(gtr_strip, PortDirection::Input),
                kind: ConnectionKind::Main,
                channel_map: None,
            },
        ],
    );
    let err = apply_intent(&mut c, i).unwrap_err();
    assert!(err.to_string().contains("routing-cycle"), "{err}");
}

// ── SF02-REJ-05: whole-intent atomicity ───────────────────────────

#[test]
fn a_rejected_intent_leaves_project_revision_and_ids_untouched() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    add_track_strip(&mut c, track);
    let (project, revision) = (serde_json::to_value(c.live()).unwrap(), c.live_revision());
    // A legal first edit then a dangling second — the legal half must
    // not leak into the live project.
    let i = intent(
        &c,
        vec![
            RoutingEdit::CreateBus {
                temp_id: TempBusId(1),
                kind: BusKind::Group,
                name: "leak?".into(),
            },
            RoutingEdit::CreateConnection {
                temp_id: TempConnectionId(1),
                source: bus_main(BusRef::Temp(TempBusId(1)), PortDirection::Output),
                destination: PortRef::Existing(SemanticPortId::new(0xBAD)),
                kind: ConnectionKind::Main,
                channel_map: None,
            },
        ],
    );
    apply_intent(&mut c, i).unwrap_err();
    assert_eq!(c.live_revision(), revision);
    assert_eq!(serde_json::to_value(c.live()).unwrap(), project);
    assert_eq!(c.live().routing.buses.len(), 1); // master only
}

// ── SF02-API-01: preview + receipts ────────────────────────────────

#[test]
fn preview_routing_mutates_nothing_and_receipts_are_deterministic() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let _strip = add_track_strip(&mut c, track);
    let master = master_strip(c.live());
    let temp = TempBusId(1);
    let i = intent(
        &c,
        vec![
            RoutingEdit::CreateBus {
                temp_id: temp,
                kind: BusKind::Group,
                name: "g".into(),
            },
            RoutingEdit::CreateConnection {
                temp_id: TempConnectionId(1),
                source: bus_main(BusRef::Temp(temp), PortDirection::Output),
                destination: strip_main(master, PortDirection::Input),
                kind: ConnectionKind::Main,
                channel_map: None,
            },
        ],
    );
    let live_before = serde_json::to_value(c.live()).unwrap();
    let plan_a = c.preview_routing(&i).unwrap();
    let plan_b = c.preview_routing(&i).unwrap();
    assert_eq!(serde_json::to_value(c.live()).unwrap(), live_before);
    // Dry-run staging is deterministic: the same intent over the same
    // base produces the same stable IDs.
    assert_eq!(plan_a.receipt, plan_b.receipt);
    let issued_bus = plan_a.receipt.bus_ids[&temp];
    assert!(plan_a.project.routing.buses.contains_key(&issued_bus));
    let conn_id = plan_a.receipt.connection_ids[&TempConnectionId(1)];
    let conn = &plan_a.project.routing.connections[&conn_id];
    let bus_out = strip_port(
        &plan_a.project,
        plan_a.project.routing.buses[&issued_bus].strip,
        PortDirection::Output,
    );
    assert_eq!(conn.source, bus_out);

    // Committing resolves the same stable IDs — preview is a faithful
    // read of what commit would do.
    apply_intent(&mut c, i).unwrap();
    assert!(c.live().routing.buses.contains_key(&issued_bus));
    assert!(c.live().routing.connections.contains_key(&conn_id));
}

#[test]
fn duplicate_temporary_ids_reject() {
    let mut c = controller();
    let i = intent(
        &c,
        vec![
            RoutingEdit::CreateBus {
                temp_id: TempBusId(1),
                kind: BusKind::Group,
                name: "a".into(),
            },
            RoutingEdit::CreateBus {
                temp_id: TempBusId(1),
                kind: BusKind::Group,
                name: "b".into(),
            },
        ],
    );
    let err = apply_intent(&mut c, i).unwrap_err();
    assert!(err.to_string().contains("routing-duplicate"), "{err}");
}

// ── SF02-API-02: send level adoption stays single-authority ───────

#[test]
fn an_adopted_gain_target_is_sealed_to_the_send() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    let ret = add_bus(&mut c, BusKind::Return, "r");
    let ret_strip = c.live().routing.buses[&ret].strip;
    c.apply(cmd(
        &c,
        ProjectOp::AddParameterTarget {
            name: "shared".into(),
            owner: ControlOwner::Track(track),
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    ))
    .unwrap();
    // The bootstrap master-fader target is well-known (never issued) —
    // pick the newest *issued* target.
    let target = *c
        .live()
        .control
        .targets
        .keys()
        .rfind(|id| is_issuable_raw(id.raw()))
        .unwrap();

    let i = intent(
        &c,
        vec![RoutingEdit::CreateSend {
            temp_id: TempSendId(1),
            temp_connection: TempConnectionId(1),
            source: strip_main(strip, PortDirection::Output),
            destination: strip_main(ret_strip, PortDirection::Input),
            tap: SendTap::PreFader,
            level: SendGainSpec::Adopt(target),
            channel_map: None,
        }],
    );
    apply_intent(&mut c, i).unwrap();
    let send = c.live().routing.sends.values().next().unwrap();
    assert_eq!(send.gain_target, Some(target));
    let t = &c.live().control.targets[&target];
    assert_eq!(t.owner, ControlOwner::Send(send.id));
    assert_eq!(t.role, ParameterRole::SendLevel);
    // No second gain value ever lives on the send itself.
    assert!(
        c.live()
            .routing
            .sends
            .values()
            .all(|s| s.gain_target == Some(target))
    );
}

// ── SF02-API-03: composite edits across the gesture ───────────────

#[test]
fn mixed_edits_compose_inside_one_intent() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    c.apply(cmd(
        &c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "one".into(),
            binding: ProcessorBinding::Unresolved {
                descriptor: "vendor.one".into(),
            },
            sidechain_input: false,
        },
    ))
    .unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "two".into(),
            binding: ProcessorBinding::Unresolved {
                descriptor: "vendor.two".into(),
            },
            sidechain_input: false,
        },
    ))
    .unwrap();
    let chain = c.live().routing.strips[&strip].chain;
    let order_before = c.live().routing.chains[&chain].processors.clone();
    let (first, second) = (order_before[0], order_before[1]);
    let temp = TempBusId(7);

    let i = intent(
        &c,
        vec![
            RoutingEdit::CreateBus {
                temp_id: temp,
                kind: BusKind::Group,
                name: "g".into(),
            },
            RoutingEdit::ChangeBusConfiguration {
                bus: BusRef::Temp(temp),
                name: Some("renamed".into()),
                kind: None,
            },
            RoutingEdit::ReorderProcessor {
                chain,
                processor: first,
                new_index: 1,
            },
            RoutingEdit::CreateConnection {
                temp_id: TempConnectionId(1),
                source: bus_main(BusRef::Temp(temp), PortDirection::Output),
                destination: strip_main(master_strip(c.live()), PortDirection::Input),
                kind: ConnectionKind::Main,
                channel_map: None,
            },
            RoutingEdit::SetConnectionEnabled {
                connection: ConnectionRef::Temp(TempConnectionId(1)),
                enabled: false,
            },
            RoutingEdit::SetMute {
                strip: StripRef::Existing(strip),
                muted: true,
            },
        ],
    );
    apply_intent(&mut c, i).unwrap();

    let bus = c
        .live()
        .routing
        .buses
        .values()
        .find(|b| b.name == "renamed")
        .unwrap();
    assert_eq!(bus.kind, BusKind::Group);
    assert_eq!(
        c.live().routing.chains[&chain].processors,
        vec![second, first]
    );
    let edge = c
        .live()
        .routing
        .connections
        .values()
        .find(|k| k.source == strip_port(c.live(), bus.strip, PortDirection::Output))
        .unwrap();
    assert!(!edge.enabled);
    assert!(c.live().routing.strip_muted(&c.live().control, strip));
    // Muting is a persisted mixer fact on the control model — the
    // audibility oracle reports the strip silent.
    let audible = c.live().routing.audible_strips(&c.live().control);
    assert!(!audible.contains(&strip));
}

// ── SF02-MIX-01: mute/solo audibility is persisted-state driven ────

#[test]
fn solo_unions_suppliers_and_paths_to_master_but_never_muted() {
    let mut c = controller();
    let (gtr, vox, keys) = (
        add_track(&mut c, "a"),
        add_track(&mut c, "b"),
        add_track(&mut c, "x"),
    );
    let (gtr_strip, vox_strip, keys_strip) = (
        add_track_strip(&mut c, gtr),
        add_track_strip(&mut c, vox),
        add_track_strip(&mut c, keys),
    );
    let master = master_strip(c.live());
    // Route a→group→master; b→master (auto); x muted.
    let i = intent(
        &c,
        vec![
            RoutingEdit::CreateBus {
                temp_id: TempBusId(1),
                kind: BusKind::Group,
                name: "g".into(),
            },
            RoutingEdit::CreateConnection {
                temp_id: TempConnectionId(1),
                source: bus_main(BusRef::Temp(TempBusId(1)), PortDirection::Output),
                destination: strip_main(master, PortDirection::Input),
                kind: ConnectionKind::Main,
                channel_map: None,
            },
            RoutingEdit::CreateConnection {
                temp_id: TempConnectionId(2),
                source: strip_main(gtr_strip, PortDirection::Output),
                destination: bus_main(BusRef::Temp(TempBusId(1)), PortDirection::Input),
                kind: ConnectionKind::Main,
                channel_map: None,
            },
            RoutingEdit::SetMute {
                strip: StripRef::Existing(keys_strip),
                muted: true,
            },
            RoutingEdit::SetSolo {
                strip: StripRef::Existing(vox_strip),
                soloed: true,
            },
        ],
    );
    apply_intent(&mut c, i).unwrap();

    let audible = c.live().routing.audible_strips(&c.live().control);
    // Soloed b plus the master stay audible; a's non-solo supply
    // through the group and muted x do not.
    assert!(audible.contains(&vox_strip));
    assert!(audible.contains(&master));
    let group_strip = c
        .live()
        .routing
        .buses
        .values()
        .find(|b| b.kind == BusKind::Group)
        .unwrap()
        .strip;
    assert!(!audible.contains(&gtr_strip));
    assert!(!audible.contains(&keys_strip));
    assert!(!audible.contains(&group_strip));

    // Clearing the solo restores non-muted audibility.
    let clear = intent(
        &c,
        vec![RoutingEdit::SetSolo {
            strip: StripRef::Existing(vox_strip),
            soloed: false,
        }],
    );
    apply_intent(&mut c, clear).unwrap();
    let audible = c.live().routing.audible_strips(&c.live().control);
    assert!(audible.contains(&gtr_strip) && audible.contains(&vox_strip));
    assert!(!audible.contains(&keys_strip));
}

// ── SV185: the bootstrap master fader is never send-adoptable ─────

#[test]
fn the_bootstrap_fader_cannot_be_adopted_as_a_send_gain() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    let ret = add_bus(&mut c, BusKind::Return, "r");
    let ret_strip = c.live().routing.buses[&ret].strip;
    let fader = ParameterTargetId::new(MASTER_FADER_RAW);

    // `CreateSend` adoption re-owns the target to the send — the
    // well-known bootstrap fader must be refused before that.
    let i = intent(
        &c,
        vec![RoutingEdit::CreateSend {
            temp_id: TempSendId(1),
            temp_connection: TempConnectionId(1),
            source: strip_main(strip, PortDirection::Output),
            destination: strip_main(ret_strip, PortDirection::Input),
            tap: SendTap::PreFader,
            level: SendGainSpec::Adopt(fader),
            channel_map: None,
        }],
    );
    let err = apply_intent(&mut c, i).unwrap_err();
    assert!(
        err.to_string().contains("well-known bootstrap target"),
        "{err}"
    );

    // `SetSendGainTarget` reaches the same re-owning apply path.
    let i = intent(
        &c,
        vec![RoutingEdit::CreateSend {
            temp_id: TempSendId(1),
            temp_connection: TempConnectionId(1),
            source: strip_main(strip, PortDirection::Output),
            destination: strip_main(ret_strip, PortDirection::Input),
            tap: SendTap::PreFader,
            level: SendGainSpec::Create {
                name: "send level".into(),
                initial: 1.0,
            },
            channel_map: None,
        }],
    );
    apply_intent(&mut c, i).unwrap();
    let send = *c.live().routing.sends.keys().next().unwrap();
    let i = intent(
        &c,
        vec![RoutingEdit::SetSendGainTarget {
            send: SendRef::Existing(send),
            target: Some(fader),
        }],
    );
    let err = apply_intent(&mut c, i).unwrap_err();
    assert!(
        err.to_string().contains("well-known bootstrap target"),
        "{err}"
    );

    // The fader survives untouched — still the master strip's Fader.
    let t = &c.live().control.targets[&fader];
    assert_eq!(t.role, ParameterRole::Fader);
    assert_eq!(t.owner, ControlOwner::Strip(master_strip(c.live())));
}
