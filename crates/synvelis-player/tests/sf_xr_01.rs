//! SF-XR-01 (issue #109): the CTL-01/CTL-02 cross-gate oracle fixture.
//!
//! One persisted project is checked end-to-end across the gates the
//! SF-06 ledger keeps separate: the project model (CTL-01 strip/owner
//! cardinality and mute/solo audibility authority; CTL-02
//! `ParameterTargetId` as the single gain authority and the explicit
//! channel-map consent gate), serialization/undo, and the player
//! semantic lowering (`RoutingSourceMap` + typed `GraphSpec`).
//!
//! Headless only: no OS GUI, native plugin, Probe, or consumer lane is
//! exercised here — those stay `NotRun` in the evidence ledger.

use std::collections::BTreeSet;

use synvelis_player::lowering::EngineConfig;
use synvelis_player::routing_lower::{SemanticLowered, SemanticNodeRole, lower_semantic};
use synvelis_project::{
    AutomationTimeScope, BuildBudget, BuildMode, BusKind, BusRef, Command, ConnectionKind,
    ControlOwner, CounterIdGenerator, ExplicitChannelMap, ExplicitConversionPolicy, InMemoryStore,
    LOWERING_POLICY_VERSION, MappingResolution, NotLoweredReason, ParameterRole, PortDirection,
    PortOwner, PortRef, PortRole, Project, ProjectController, ProjectId, ProjectOp,
    ProjectRevision, RoutingBuildKey, RoutingEdit, RoutingEditIntent, RuntimeClockEpochRef,
    RuntimeInstanceId, SemanticNodeRef, SemanticPortId, SendGainSpec, SendId, SendTap, StripId,
    StripOwner, StripRef, TempBusId, TempConnectionId, TempSendId, TrackId,
    port_capability_generation, routing_fingerprint, routing_preflight, send_level_target_id,
};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0x109), "sfxr01"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply(c: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .unwrap()
}

fn add_track(c: &mut ProjectController, name: &str) -> TrackId {
    apply(c, ProjectOp::AddTrack { name: name.into() });
    *c.live().tracks.keys().next_back().unwrap()
}

fn add_track_strip(c: &mut ProjectController, track: TrackId) -> StripId {
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    c.live().routing.strip_of_track(track).unwrap().id
}

fn non_master_bus(p: &Project) -> synvelis_project::BusId {
    *p.routing
        .buses
        .keys()
        .find(|b| **b != p.routing.master)
        .expect("a non-master bus")
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

fn port_strip(p: &Project, port: SemanticPortId) -> StripId {
    match p.routing.ports[&port].owner {
        PortOwner::Strip(strip) => strip,
        PortOwner::Processor(_) => panic!("main ports are strip-owned"),
    }
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

fn intent(
    c: &ProjectController,
    edits: Vec<RoutingEdit>,
    approved_conversion: ExplicitConversionPolicy,
) -> RoutingEditIntent {
    RoutingEditIntent {
        operation_id: "sfxr01".into(),
        expected_port_capability_generation: port_capability_generation(c.live()),
        edits,
        approved_conversion,
    }
}

fn apply_intent(
    c: &mut ProjectController,
    intent: RoutingEditIntent,
) -> Result<(), synvelis_project::Error> {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op: ProjectOp::Routing { intent },
    })
    .map(|_| ())
}

fn engine() -> EngineConfig {
    EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: 256,
    }
}

fn canonical_format() -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(48_000).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
}

fn build_key(project: &Project, revision: ProjectRevision) -> RoutingBuildKey {
    RoutingBuildKey {
        project_id: project.project_id,
        project_revision: revision,
        routing_fingerprint: routing_fingerprint(project),
        capability_generation: port_capability_generation(project),
        engine_config_generation: 1,
        source_format_generation: 1,
        lowering_policy_version: LOWERING_POLICY_VERSION,
        build_mode: BuildMode::Realtime,
        device_clock_epoch: RuntimeClockEpochRef(1),
        runtime_instance: RuntimeInstanceId(1),
    }
}

fn test_budget() -> BuildBudget {
    BuildBudget {
        max_nodes: 4_096,
        max_connections: 16_384,
        max_frames: 512,
    }
}

fn lower_for_test(
    project: &Project,
    revision: ProjectRevision,
    generation: u64,
) -> SemanticLowered {
    lower_semantic(
        project,
        &|_| Some(canonical_format()),
        &engine(),
        &build_key(project, revision),
        generation,
        &test_budget(),
    )
    .unwrap()
}

fn resolved_send(project: &Project, tap: SendTap) -> (SendId, &synvelis_project::SendConfig) {
    project
        .routing
        .sends
        .iter()
        .find(|(_, send)| send.tap == tap)
        .map_or_else(|| panic!("no {tap:?} send"), |(id, send)| (*id, send))
}

// ── SF-XR-01 / SF-XR-01a: persisted authority → serialization/undo →
//    semantic lowering, all on one committed project ────────────────

/// CTL-01 strip cardinality + CTL-02 send-level authority + mute
/// audibility, cross-checked against the generation-scoped
/// `RoutingSourceMap` and the typed `GraphSpec` it produced.
#[expect(clippy::too_many_lines)] // sequential cross-gate assertions
#[test]
fn sfxr01_01_strip_send_authority_survives_serialization_undo_and_lowering() {
    let mut c = controller();
    let track_a = add_track(&mut c, "a");
    let track_b = add_track(&mut c, "b");
    let strip_a = add_track_strip(&mut c, track_a);
    let strip_b = add_track_strip(&mut c, track_b);
    let master = master_strip(c.live());

    // Pre-gesture snapshot for the undo half of the serialization gate.
    let before = c.live().clone();
    let before_rev = c.live_revision();

    // One atomic intent: group bus + bus feed + bus→master route + two
    // sends off track B (pre- and post-fader). The pre-fader send
    // carries an approved explicit channel map — a declared swap on an
    // equal layout, still consent-gated.
    let swap = ExplicitChannelMap {
        entries: vec![1, 0],
    };
    let i = intent(
        &c,
        vec![
            RoutingEdit::CreateBus {
                temp_id: TempBusId(1),
                kind: BusKind::Group,
                name: "fx".into(),
            },
            RoutingEdit::CreateConnection {
                temp_id: TempConnectionId(1),
                source: strip_main(strip_a, PortDirection::Output),
                destination: bus_main(BusRef::Temp(TempBusId(1)), PortDirection::Input),
                kind: ConnectionKind::Main,
                channel_map: None,
            },
            RoutingEdit::CreateConnection {
                temp_id: TempConnectionId(2),
                source: bus_main(BusRef::Temp(TempBusId(1)), PortDirection::Output),
                destination: strip_main(master, PortDirection::Input),
                kind: ConnectionKind::Main,
                channel_map: None,
            },
            RoutingEdit::CreateSend {
                temp_id: TempSendId(1),
                temp_connection: TempConnectionId(3),
                source: strip_main(strip_b, PortDirection::Output),
                destination: bus_main(BusRef::Temp(TempBusId(1)), PortDirection::Input),
                tap: SendTap::PreFader,
                level: SendGainSpec::Create {
                    name: "pre send".into(),
                    initial: 0.25,
                },
                channel_map: Some(swap.clone()),
            },
            RoutingEdit::CreateSend {
                temp_id: TempSendId(2),
                temp_connection: TempConnectionId(4),
                source: strip_main(strip_b, PortDirection::Output),
                destination: bus_main(BusRef::Temp(TempBusId(1)), PortDirection::Input),
                tap: SendTap::PostFader,
                level: SendGainSpec::Create {
                    name: "post send".into(),
                    initial: 0.75,
                },
                channel_map: None,
            },
        ],
        ExplicitConversionPolicy::Approved,
    );
    apply_intent(&mut c, i).unwrap();
    // The whole gesture is one CAS: exactly one revision for five edits.
    assert_eq!(c.live_revision(), ProjectRevision(before_rev.0 + 1));

    // ── persisted CTL-01/CTL-02 invariants ─────────────────────────
    let bus = non_master_bus(c.live());
    let bus_strip = c.live().routing.buses[&bus].strip;
    {
        let p = c.live();
        // 1 Track/1 Bus → 1 Strip; master exactly one.
        assert_eq!(p.routing.strips.len(), 4);
        for (track, strip) in [(track_a, strip_a), (track_b, strip_b)] {
            assert_eq!(p.routing.strips[&strip].owner, StripOwner::Track(track));
            assert_eq!(p.routing.strip_of_track(track).unwrap().id, strip);
        }
        assert_eq!(p.routing.strips[&bus_strip].owner, StripOwner::Bus(bus));
        assert_eq!(p.routing.buses[&bus].strip, bus_strip);
        assert_eq!(
            p.routing
                .buses
                .values()
                .filter(|b| b.kind == BusKind::Master)
                .count(),
            1
        );
        assert_eq!(
            p.routing.strips[&master].owner,
            StripOwner::Bus(p.routing.master)
        );

        // Send gain lives only in `control.targets` — `SendConfig`
        // carries a reference, never a second value.
        let (pre_send, pre_cfg) = resolved_send(p, SendTap::PreFader);
        let (post_send, post_cfg) = resolved_send(p, SendTap::PostFader);
        for (send, cfg, base) in [(pre_send, pre_cfg, 0.25), (post_send, post_cfg, 0.75)] {
            let conn = &p.routing.connections[&cfg.connection];
            assert_eq!(conn.kind, ConnectionKind::Send(send));
            assert!(conn.enabled);
            let target_id = send_level_target_id(p, send).expect("send level target");
            assert_eq!(cfg.gain_target, Some(target_id));
            let target = &p.control.targets[&target_id];
            assert_eq!(target.owner, ControlOwner::Send(send));
            assert_eq!(target.role, ParameterRole::SendLevel);
            assert_eq!(target.time_scope, AutomationTimeScope::ProjectMusical);
            assert!((target.base - base).abs() < 1e-9);
            // Serialized SendConfig has no gain field at all.
            let v = serde_json::to_value(cfg).unwrap();
            let keys: BTreeSet<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
            assert_eq!(
                keys,
                ["connection", "gain_target", "id", "tap"]
                    .into_iter()
                    .collect::<BTreeSet<_>>()
            );
        }
        // The approved map persists on the pre-fader send's edge only.
        assert_eq!(
            p.routing.connections[&pre_cfg.connection].channel_map,
            Some(swap)
        );
        assert_eq!(
            p.routing.connections[&post_cfg.connection].channel_map,
            None
        );
        assert_eq!(pre_cfg.tap, SendTap::PreFader);
        assert_eq!(post_cfg.tap, SendTap::PostFader);
    }

    // ── serialization round-trip ──────────────────────────────────
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let mut reloaded = controller();
    reloaded.load(&store).unwrap();
    assert_eq!(reloaded.live_revision(), c.live_revision());
    assert_eq!(
        serde_json::to_value(reloaded.live()).unwrap(),
        serde_json::to_value(c.live()).unwrap()
    );

    // ── undo/redo restores the pre-gesture project exactly ────────
    assert!(c.undo().unwrap());
    assert_eq!(
        serde_json::to_value(c.live()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert!(c.redo().unwrap());
    assert_eq!(
        serde_json::to_value(c.live()).unwrap(),
        serde_json::to_value(reloaded.live()).unwrap()
    );

    // ── semantic lowering cross-check (graph source-map gate) ─────
    let (pre_send, _) = resolved_send(c.live(), SendTap::PreFader);
    let (post_send, _) = resolved_send(c.live(), SendTap::PostFader);
    let lowered = lower_for_test(c.live(), c.live_revision(), 7);
    let sm = &lowered.source_map;

    // Every strip resolves to exactly one fader node; a bus resolves
    // through its strip to that same node — 1:1, never duplicated.
    let node_of = |strip: StripId| match &sm.semantic_nodes[&SemanticNodeRef::Strip(strip)] {
        MappingResolution::Resolved(node) => *node,
        other => panic!("strip {strip:?} must resolve, got {other:?}"),
    };
    let node_a = node_of(strip_a);
    let node_b = node_of(strip_b);
    let node_bus = node_of(bus_strip);
    let node_master = node_of(master);
    let bus_node = match &sm.semantic_nodes[&SemanticNodeRef::Bus(bus)] {
        MappingResolution::Resolved(node) => *node,
        other => panic!("bus {bus:?} must resolve, got {other:?}"),
    };
    assert_eq!(bus_node, node_bus);
    let distinct: BTreeSet<_> = [node_a, node_b, node_bus, node_master]
        .into_iter()
        .collect();
    assert_eq!(distinct.len(), 4, "each strip owns a distinct fader node");

    // Pre/post taps: same fader node, different ports; PostFader is
    // the strip main-out resolution itself.
    let out_b = strip_port(c.live(), strip_b, PortDirection::Output);
    let MappingResolution::Resolved(post_ref) = sm.ports[&out_b] else {
        panic!("strip B main out must resolve");
    };
    let pre_tap = sm.send_taps[&pre_send];
    let post_tap = sm.send_taps[&post_send];
    assert_eq!(post_tap.node, post_ref.node);
    assert_eq!(post_tap.port, post_ref.port);
    assert_eq!(pre_tap.node, post_ref.node);
    assert_ne!(pre_tap.port, post_ref.port);

    // The send connections resolve to their gain→destination edges;
    // the persisted channel map reaches the typed spec edge verbatim
    // (destination→source indices become (src, dst) pairs).
    let send_gain = |send: SendId| {
        lowered
            .node_roles
            .values()
            .find_map(|role| match role {
                SemanticNodeRole::SendGain { send: s, gain } if *s == send => Some(*gain),
                _ => None,
            })
            .unwrap_or_else(|| panic!("no SendGain node for {send:?}"))
    };
    assert!((send_gain(pre_send) - 0.25).abs() < 1e-6);
    assert!((send_gain(post_send) - 0.75).abs() < 1e-6);
    for (send, expect_map) in [(pre_send, true), (post_send, false)] {
        let conn = &c.live().routing.connections[&c.live().routing.sends[&send].connection];
        let MappingResolution::Resolved(conn_id) = sm.connections[&conn.id] else {
            panic!("send connection must resolve");
        };
        let edge = lowered
            .spec
            .connections()
            .iter()
            .find(|e| e.id() == conn_id)
            .expect("resolved connection id must exist in the spec");
        if expect_map {
            let map = edge
                .channel_map()
                .expect("approved map reaches the graph edge");
            let mut entries = map.entries.clone();
            entries.sort_unstable();
            assert_eq!(entries, vec![(0, 1), (1, 0)]);
        } else {
            assert!(edge.channel_map().is_none());
        }
    }

    // ── mute: the same audibility authority prunes every edge out of
    //    the muted strip — sends included, pre- and post-fader ──────
    apply(
        &mut c,
        ProjectOp::SetStripMute {
            strip_id: strip_b,
            muted: true,
        },
    );
    // CTL-01: mute is a Strip-owned Boolean target, not a side flag.
    assert!(
        c.live()
            .control
            .targets
            .values()
            .any(|t| { t.owner == ControlOwner::Strip(strip_b) && t.role == ParameterRole::Mute })
    );
    let muted = lower_for_test(c.live(), c.live_revision(), 8);
    let msm = &muted.source_map;
    assert!(!msm.send_taps.contains_key(&pre_send));
    assert!(!msm.send_taps.contains_key(&post_send));
    for conn in c.live().routing.connections.values() {
        let touches_b = port_strip(c.live(), conn.source) == strip_b
            || port_strip(c.live(), conn.destination) == strip_b;
        if touches_b {
            assert!(
                matches!(
                    msm.connections[&conn.id],
                    MappingResolution::NotLowered(NotLoweredReason::MutedStrip)
                ),
                "connection {:?} touching muted strip must not lower",
                conn.id
            );
        } else {
            assert!(
                matches!(msm.connections[&conn.id], MappingResolution::Resolved(_)),
                "unrelated connection {:?} must still resolve",
                conn.id
            );
        }
    }
}

// ── SF-XR-01 / SF-XR-01b: explicit channel-map consent on the
//    CreateSend path + commit-path atomicity ────────────────────────

/// A `CreateSend` intent against `strip`→`bus_strip` carrying the
/// given consent policy and channel map.
fn send_intent(
    c: &ProjectController,
    strip: StripId,
    bus_strip: StripId,
    approved_conversion: ExplicitConversionPolicy,
    channel_map: Option<ExplicitChannelMap>,
) -> RoutingEditIntent {
    RoutingEditIntent {
        operation_id: "sfxr01-02".into(),
        expected_port_capability_generation: port_capability_generation(c.live()),
        edits: vec![RoutingEdit::CreateSend {
            temp_id: TempSendId(7),
            temp_connection: TempConnectionId(7),
            source: strip_main(strip, PortDirection::Output),
            destination: strip_main(bus_strip, PortDirection::Input),
            tap: SendTap::PostFader,
            level: SendGainSpec::Create {
                name: "send".into(),
                initial: 0.5,
            },
            channel_map,
        }],
        approved_conversion,
    }
}

#[expect(clippy::too_many_lines)] // sequential consent/atomicity matrix
#[test]
fn sfxr01_02_create_send_channel_map_consent_and_atomic_rejection() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    let master = master_strip(c.live());
    apply(
        &mut c,
        ProjectOp::AddBus {
            kind: BusKind::Group,
            name: "fx".into(),
        },
    );
    let bus = non_master_bus(c.live());
    let bus_strip = c.live().routing.buses[&bus].strip;

    // Mutated staging base: strip output and master input read mono so
    // the pre-existing strip→master edge stays a consistent 1→1; the
    // bus input stays stereo → the new send edge is a 1→2 mismatch.
    let mut p = c.live().clone();
    let out_port = strip_port(&p, strip, PortDirection::Output);
    p.routing.ports.get_mut(&out_port).unwrap().channels = 1;
    let master_in = strip_port(&p, master, PortDirection::Input);
    p.routing.ports.get_mut(&master_in).unwrap().channels = 1;
    let fold = || {
        Some(ExplicitChannelMap {
            entries: vec![0, 0],
        })
    };
    let mk = |approved_conversion, channel_map| RoutingEditIntent {
        operation_id: "sfxr01-02-staging".into(),
        expected_port_capability_generation: port_capability_generation(&p),
        edits: vec![RoutingEdit::CreateSend {
            temp_id: TempSendId(1),
            temp_connection: TempConnectionId(1),
            source: strip_main(strip, PortDirection::Output),
            destination: strip_main(bus_strip, PortDirection::Input),
            tap: SendTap::PreFader,
            level: SendGainSpec::Create {
                name: "send".into(),
                initial: 0.5,
            },
            channel_map,
        }],
        approved_conversion,
    };

    // Layout mismatch without a map → explicit decision required,
    // even when the policy says Approved.
    let mut ids = CounterIdGenerator::new(p.id_watermark);
    let err =
        routing_preflight(&p, &mk(ExplicitConversionPolicy::Approved, None), &mut ids).unwrap_err();
    assert!(
        err.to_string().contains("routing-requires-decision"),
        "{err}"
    );

    // A well-formed map without the approved policy → still a
    // decision, never a silent upmix.
    let err =
        routing_preflight(&p, &mk(ExplicitConversionPolicy::None, fold()), &mut ids).unwrap_err();
    assert!(
        err.to_string().contains("routing-requires-decision"),
        "{err}"
    );

    // Approved + valid map → staged atomically: send, its Send-kind
    // connection carrying the map, and its Send-owned SendLevel
    // target all appear in the candidate.
    let plan = routing_preflight(
        &p,
        &mk(ExplicitConversionPolicy::Approved, fold()),
        &mut ids,
    )
    .unwrap();
    let staged = plan.project.routing.sends.values().next().unwrap();
    let gain_target = staged
        .gain_target
        .expect("created send owns a level target");
    assert_eq!(plan.receipt.gain_targets[&TempSendId(1)], gain_target);
    let target = &plan.project.control.targets[&gain_target];
    assert_eq!(target.owner, ControlOwner::Send(staged.id));
    assert_eq!(target.role, ParameterRole::SendLevel);
    let staged_conn = &plan.project.routing.connections[&staged.connection];
    assert_eq!(staged_conn.kind, ConnectionKind::Send(staged.id));
    assert_eq!(staged_conn.channel_map, fold());

    // A map reading a source channel the mono input does not have →
    // malformed, typed rejection.
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

    // ── commit path: same-layout map still needs consent, and a
    //    rejected intent must not consume revision, state, or undo ──
    let swap = || {
        Some(ExplicitChannelMap {
            entries: vec![1, 0],
        })
    };
    let rev = c.live_revision();
    let snapshot = serde_json::to_value(c.live()).unwrap();
    let unapproved = send_intent(&c, strip, bus_strip, ExplicitConversionPolicy::None, swap());
    let err = apply_intent(&mut c, unapproved).unwrap_err();
    assert!(
        err.to_string().contains("routing-requires-decision"),
        "{err}"
    );
    assert_eq!(c.live_revision(), rev);
    assert_eq!(serde_json::to_value(c.live()).unwrap(), snapshot);
    assert!(c.live().routing.sends.is_empty());

    // Undo-stack proof: a successful op, then a rejected intent —
    // undo must restore the pre-op state, i.e. the rejection pushed
    // nothing.
    let before_track = serde_json::to_value(c.live()).unwrap();
    apply(&mut c, ProjectOp::AddTrack { name: "u".into() });
    let after_track = serde_json::to_value(c.live()).unwrap();
    let rejected_again = send_intent(&c, strip, bus_strip, ExplicitConversionPolicy::None, swap());
    apply_intent(&mut c, rejected_again).unwrap_err();
    assert_eq!(serde_json::to_value(c.live()).unwrap(), after_track);
    assert!(c.undo().unwrap());
    assert_eq!(serde_json::to_value(c.live()).unwrap(), before_track);
    assert!(c.redo().unwrap());
    assert_eq!(serde_json::to_value(c.live()).unwrap(), after_track);

    // Approved → commits; the map persists on the send's connection
    // and the gain target is Send-owned.
    let approved = send_intent(
        &c,
        strip,
        bus_strip,
        ExplicitConversionPolicy::Approved,
        swap(),
    );
    apply_intent(&mut c, approved).unwrap();
    let send = *c.live().routing.sends.keys().next().unwrap();
    let cfg = &c.live().routing.sends[&send];
    assert_eq!(cfg.gain_target, send_level_target_id(c.live(), send));
    assert_eq!(
        c.live().routing.connections[&cfg.connection].channel_map,
        swap()
    );
}
