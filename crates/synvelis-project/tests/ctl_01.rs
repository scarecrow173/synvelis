//! CTL-01 (issue #121): `ControlOwner` / `AutomationTimeScope` /
//! `ParameterRole` typed binding — owner existence, owner×scope matrix,
//! singleton role uniqueness, send gain ownership, v6→v7 migration and
//! legacy evaluation-equivalence coverage.

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

fn add_target(
    c: &mut ProjectController,
    owner: ControlOwner,
    role: ParameterRole,
    time_scope: AutomationTimeScope,
) -> Result<ParameterTargetId, Error> {
    let rev = c.live_revision();
    c.apply(cmd(
        c,
        ProjectOp::AddParameterTarget {
            name: "p".into(),
            owner,
            role,
            time_scope,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    ))?;
    // `apply` validates against `expected_revision`; a rejected apply
    // leaves revision and project untouched.
    assert_eq!(c.live_revision(), ProjectRevision(rev.0 + 1));
    // The bootstrap master-fader target is well-known (never issued) —
    // pick the newest *issued* target.
    Ok(*c
        .live()
        .control
        .targets
        .keys()
        .rfind(|id| is_issuable_raw(id.raw()))
        .unwrap())
}

fn master_in(p: &Project) -> SemanticPortId {
    let strip = &p.routing.strips[&p.routing.buses[&p.routing.master].strip];
    strip
        .port_ids
        .iter()
        .find(|id| p.routing.ports[id].direction == PortDirection::Input)
        .copied()
        .unwrap()
}

fn strip_out(p: &Project, strip: StripId) -> SemanticPortId {
    p.routing.strips[&strip]
        .port_ids
        .iter()
        .find(|id| p.routing.ports[id].direction == PortDirection::Output)
        .copied()
        .unwrap()
}

fn add_clip(c: &mut ProjectController, track: TrackId) -> ClipId {
    c.apply(cmd(c, ProjectOp::AddLane { track_id: track }))
        .unwrap();
    let lane = *c.live().lanes.keys().next_back().unwrap();
    c.apply(cmd(c, ProjectOp::RegisterAsset { digest: "d".into() }))
        .unwrap();
    let asset = *c.live().assets.keys().next_back().unwrap();
    c.apply(cmd(
        c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "d".into(),
        },
    ))
    .unwrap();
    let source = *c.live().sources.keys().next_back().unwrap();
    c.apply(cmd(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(0)),
                extent: ClipExtent::Musical(MusicalDuration::new(8 * 960).unwrap()),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(1_000_000).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    ))
    .unwrap();
    *c.live().clips.keys().next_back().unwrap()
}

// ── CTL-01-MODEL: owner existence + wire shape ──────────────────

#[test]
fn owner_serde_roundtrips_and_accepts_the_legacy_scope_tag() {
    for (owner, wire) in [
        (ControlOwner::Project, "Project"),
        (ControlOwner::Track(TrackId::new(1)), "Track"),
        (ControlOwner::Strip(StripId::new(2)), "Strip"),
        (ControlOwner::Send(SendId::new(3)), "Send"),
        (ControlOwner::Processor(ProcessorId::new(4)), "Processor"),
        (ControlOwner::Clip(ClipId::new(5)), "Clip"),
        (ControlOwner::Note(NoteId::new(6)), "Note"),
    ] {
        let v = serde_json::to_value(owner).unwrap();
        // New bodies write the `owner` tag only — the legacy `scope`
        // tag never reappears in fresh serialization.
        assert_eq!(v["owner"], wire);
        let back: ControlOwner = serde_json::from_value(v).unwrap();
        assert_eq!(back, owner);
    }
    // v6 bodies tag ownership `{"scope": "<Variant>", "id": ...}`; the
    // migration boundary reads that into `ControlOwner`.
    let legacy = serde_json::json!({"scope": "Track", "id": TrackId::new(9).to_tagged()});
    let parsed: ControlOwner = serde_json::from_value(legacy).unwrap();
    assert_eq!(parsed, ControlOwner::Track(TrackId::new(9)));
    let legacy_project = serde_json::json!({"scope": "Project"});
    let parsed: ControlOwner = serde_json::from_value(legacy_project).unwrap();
    assert_eq!(parsed, ControlOwner::Project);
}

#[test]
fn missing_owner_entities_are_rejected_at_write_and_by_the_oracle() {
    let mut c = controller();
    for owner in [
        ControlOwner::Track(TrackId::new(777)),
        ControlOwner::Strip(StripId::new(777)),
        ControlOwner::Send(SendId::new(777)),
        ControlOwner::Processor(ProcessorId::new(777)),
        ControlOwner::Clip(ClipId::new(777)),
        ControlOwner::Note(NoteId::new(777)),
    ] {
        let before = c.live_revision();
        let result = c.apply(cmd(
            &c,
            ProjectOp::AddParameterTarget {
                name: "p".into(),
                owner,
                role: ParameterRole::Generic,
                time_scope: AutomationTimeScope::ProjectMusical,
                kind: ValueKind::Continuous,
                mapping: ParamMapping::Linear,
                capability: None,
                base: 0.5,
            },
        ));
        assert!(matches!(result, Err(Error::NotFound(_))), "{owner:?}");
        assert_eq!(c.live_revision(), before);
    }
    // The oracle catches the same violation on a hand-built project.
    let mut p = Project::new(ProjectId::new(0), "p");
    let id = ParameterTargetId::new(1);
    p.control.targets.insert(
        id,
        ParameterTarget {
            id,
            owner: ControlOwner::Strip(StripId::new(777)),
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            name: "ghost".into(),
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    assert!(matches!(p.validate_references(), Err(Error::NotFound(_))));
}

// ── CTL-01-MODEL: owner × time-scope matrix ─────────────────────

#[test]
fn the_owner_time_scope_matrix_is_enforced() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    let clip = add_clip(&mut c, track);
    c.apply(cmd(
        &c,
        ProjectOp::AddNote {
            clip,
            key: 60,
            start: BeatPoint::new(0, 1).unwrap(),
            len: BeatPoint::new(1, 1).unwrap(),
        },
    ))
    .unwrap();
    let note = *c.live().notes.keys().next().unwrap();

    // Legal combinations.
    for (owner, time_scope) in [
        (ControlOwner::Project, AutomationTimeScope::ProjectMusical),
        (ControlOwner::Project, AutomationTimeScope::ProjectAbsolute),
        (
            ControlOwner::Track(track),
            AutomationTimeScope::ProjectMusical,
        ),
        (
            ControlOwner::Strip(strip),
            AutomationTimeScope::ProjectMusical,
        ),
        (ControlOwner::Clip(clip), AutomationTimeScope::ClipLocal),
        (ControlOwner::Note(note), AutomationTimeScope::NoteLocal),
    ] {
        add_target(&mut c, owner, ParameterRole::Generic, time_scope)
            .unwrap_or_else(|e| panic!("{owner:?} + {time_scope:?}: {e}"));
    }
    // Illegal combinations: local scope on a non-matching owner, or
    // project-time scope on a local owner.
    for (owner, time_scope) in [
        (ControlOwner::Track(track), AutomationTimeScope::ClipLocal),
        (ControlOwner::Strip(strip), AutomationTimeScope::NoteLocal),
        (ControlOwner::Clip(clip), AutomationTimeScope::NoteLocal),
        (ControlOwner::Note(note), AutomationTimeScope::ClipLocal),
        (
            ControlOwner::Clip(clip),
            AutomationTimeScope::ProjectMusical,
        ),
        (
            ControlOwner::Note(note),
            AutomationTimeScope::ProjectAbsolute,
        ),
    ] {
        let before = c.live_revision();
        let result = c.apply(cmd(
            &c,
            ProjectOp::AddParameterTarget {
                name: "bad".into(),
                owner,
                role: ParameterRole::Generic,
                time_scope,
                kind: ValueKind::Continuous,
                mapping: ParamMapping::Linear,
                capability: None,
                base: 0.5,
            },
        ));
        assert!(
            matches!(result, Err(Error::Validation(_))),
            "{owner:?} + {time_scope:?} must be rejected"
        );
        assert_eq!(c.live_revision(), before);
    }
}

#[test]
fn absolute_scope_reports_unavailable_not_silent_beats() {
    let mut c = controller();
    let target = add_target(
        &mut c,
        ControlOwner::Project,
        ParameterRole::Generic,
        AutomationTimeScope::ProjectAbsolute,
    )
    .unwrap();
    let result = evaluate_control(c.live(), target, BeatPoint::new(1, 1).unwrap(), None);
    assert!(
        matches!(result, Err(Error::ScopeUnavailable { .. })),
        "{result:?}"
    );
}

// ── CTL-01-ROLE: singleton role uniqueness ──────────────────────

#[test]
fn singleton_roles_are_unique_per_owner() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    // CTL-02: strip creation already seeds the channel Fader — the
    // singleton is occupied before any user target exists.
    assert!(strip_channel_target_id(c.live(), strip, ParameterRole::Fader).is_some());
    // A second Fader on the same strip collides; on another strip it
    // does not.
    let result = c.apply(cmd(
        &c,
        ProjectOp::AddParameterTarget {
            name: "f2".into(),
            owner: ControlOwner::Strip(strip),
            role: ParameterRole::Fader,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    ));
    assert!(matches!(result, Err(Error::Validation(_))));
    // Generic and ProcessorParam targets may coexist on one owner.
    c.apply(cmd(
        &c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "fx".into(),
            binding: ProcessorBinding::Unresolved {
                descriptor: "vendor.fx".into(),
            },
            sidechain_input: false,
        },
    ))
    .unwrap();
    let proc_ = *c.live().routing.processors.keys().next().unwrap();
    for _ in 0..2 {
        add_target(
            &mut c,
            ControlOwner::Processor(proc_),
            ParameterRole::ProcessorParam,
            AutomationTimeScope::ProjectMusical,
        )
        .unwrap();
    }
}

#[test]
fn the_oracle_rejects_duplicate_singleton_roles_on_load() {
    let mut p = Project::new(ProjectId::new(0), "p");
    let track = TrackId::new(1);
    p.tracks.insert(
        track,
        Track {
            id: track,
            name: "t".into(),
            lane_ids: Vec::new(),
        },
    );
    let strip = StripId::new(2);
    let chain = ChainId::new(3);
    p.routing.chains.insert(
        chain,
        ProcessorChain {
            id: chain,
            processors: Vec::new(),
        },
    );
    let pin = SemanticPortId::new(4);
    let pout = SemanticPortId::new(5);
    for (id, direction) in [(pin, PortDirection::Input), (pout, PortDirection::Output)] {
        p.routing.ports.insert(
            id,
            SemanticPort {
                id,
                owner: PortOwner::Strip(strip),
                direction,
                role: PortRole::Main,
                domain: SignalDomain::Audio,
                binding: PortBinding::Internal,
                channels: synvelis_project::routing::DEFAULT_PORT_CHANNELS,
            },
        );
    }
    p.routing.strips.insert(
        strip,
        ChannelStrip {
            id: strip,
            owner: StripOwner::Track(track),
            chain,
            port_ids: vec![pin, pout],
        },
    );
    for i in 0..2u128 {
        let id = ParameterTargetId::new(10 + i);
        p.control.targets.insert(
            id,
            ParameterTarget {
                id,
                owner: ControlOwner::Strip(strip),
                role: ParameterRole::Pan,
                time_scope: AutomationTimeScope::ProjectMusical,
                name: format!("pan{i}"),
                kind: ValueKind::Continuous,
                mapping: ParamMapping::Linear,
                capability: None,
                base: 0.5,
            },
        );
    }
    assert!(matches!(p.validate_references(), Err(Error::Validation(_))));
}

// ── CTL-01-ROUTING: Track/Bus→Strip + send gain ownership ───────

#[test]
fn one_track_maps_to_at_most_one_strip_and_bus_pairs_stay_exact() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    add_track_strip(&mut c, track);
    assert!(
        c.apply(cmd(&c, ProjectOp::AddTrackStrip { track_id: track }))
            .is_err()
    );
    // Every bus references exactly one strip that points back at it.
    c.apply(cmd(
        &c,
        ProjectOp::AddBus {
            kind: BusKind::Return,
            name: "r".into(),
        },
    ))
    .unwrap();
    for bus in c.live().routing.buses.values() {
        let strip = &c.live().routing.strips[&bus.strip];
        assert_eq!(strip.owner, StripOwner::Bus(bus.id));
    }
    assert!(c.live().validate_references().is_ok());
}

#[test]
fn a_send_owns_its_gain_target_for_life() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    let target = add_target(
        &mut c,
        ControlOwner::Track(track),
        ParameterRole::Generic,
        AutomationTimeScope::ProjectMusical,
    )
    .unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::AddSend {
            source: strip_out(c.live(), strip),
            destination: master_in(c.live()),
            tap: SendTap::PostFader,
            gain_target: Some(target),
        },
    ))
    .unwrap();
    // `AddSend` seals the binding: the send owns its level target.
    let send = *c.live().routing.sends.keys().next().unwrap();
    let bound = &c.live().control.targets[&target];
    assert_eq!(bound.owner, ControlOwner::Send(send));
    assert_eq!(bound.role, ParameterRole::SendLevel);

    // A clip-local target can never carry a send owner — the op
    // rejects the binding before the send is created.
    let clip = add_clip(&mut c, track);
    let local = add_target(
        &mut c,
        ControlOwner::Clip(clip),
        ParameterRole::Generic,
        AutomationTimeScope::ClipLocal,
    )
    .unwrap();
    assert!(
        c.apply(cmd(
            &c,
            ProjectOp::AddSend {
                source: strip_out(c.live(), strip),
                destination: master_in(c.live()),
                tap: SendTap::PreFader,
                gain_target: Some(local),
            }
        ))
        .is_err()
    );
}

#[test]
fn the_oracle_rejects_a_foreign_owned_gain_target() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    add_track_strip(&mut c, track);
    // Hand-built state: a send whose gain_target is owned by a track —
    // what the `AddSend` adoption makes impossible through ops.
    let mut p = c.live().clone();
    let send_id = SendId::new(900);
    let conn_id = RoutingConnectionId::new(901);
    let strip = p.routing.strip_of_track(track).unwrap().id;
    let out = strip_out(&p, strip);
    let dst = master_in(&p);
    p.routing.connections.insert(
        conn_id,
        RoutingConnection {
            id: conn_id,
            source: out,
            destination: dst,
            kind: ConnectionKind::Send(send_id),
            enabled: true,
            channel_map: None,
        },
    );
    let target = ParameterTargetId::new(902);
    p.control.targets.insert(
        target,
        ParameterTarget {
            id: target,
            owner: ControlOwner::Track(track),
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            name: "foreign".into(),
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    p.routing.sends.insert(
        send_id,
        SendConfig {
            id: send_id,
            connection: conn_id,
            tap: SendTap::PostFader,
            gain_target: Some(target),
        },
    );
    assert!(matches!(p.validate_references(), Err(Error::Validation(_))));
}

#[test]
fn removing_a_bus_cannot_orphan_strip_owned_targets() {
    let mut c = controller();
    c.apply(cmd(
        &c,
        ProjectOp::AddBus {
            kind: BusKind::Return,
            name: "r".into(),
        },
    ))
    .unwrap();
    let bus = *c.live().routing.buses.keys().next_back().unwrap();
    let strip = c.live().routing.buses[&bus].strip;
    add_target(
        &mut c,
        ControlOwner::Strip(strip),
        ParameterRole::Generic,
        AutomationTimeScope::ProjectMusical,
    )
    .unwrap();
    // The strip still owns a target — `RemoveBus` must not produce a
    // dangling owner; the command is rejected whole by the oracle.
    assert!(
        c.apply(cmd(&c, ProjectOp::RemoveBus { bus_id: bus }))
            .is_err()
    );
    assert!(c.live().routing.buses.contains_key(&bus));
    assert!(c.live().validate_references().is_ok());
}

// ── CTL-01-SCHEMA: v6 → v7 migration ────────────────────────────

/// Serializes `project` as a schema-v6 envelope: targets carry the
/// legacy `{"scope": ...}` owner tag and no `role`/`time_scope`.
/// The bootstrap master-fader target (synvelis#185) and the CTL-02
/// strip channel targets did not exist in v6 — a real v6 body cannot
/// carry their `Strip` owners (the v6→v7 step rejects non-legacy
/// owners), so they are omitted here and re-seeded by the later
/// migration steps on load.
fn as_v6_envelope(c: &ProjectController) -> serde_json::Value {
    let mut env = serde_json::json!({
        "schema_version": 6,
        "project_id": c.live().project_id,
        "revision": c.live_revision(),
        "project": serde_json::to_value(c.live()).unwrap(),
    });
    let targets = env["project"]["control"]["targets"]
        .as_object_mut()
        .unwrap();
    targets.remove(&ParameterTargetId::new(MASTER_FADER_RAW).to_tagged());
    // Strip-owned channel targets (CTL-02) are post-v6 state — drop
    // them plus anything bound to them so the body is a faithful v6
    // snapshot.
    let strip_owned: Vec<String> = targets
        .iter()
        .filter(|(_, t)| t["owner"]["owner"] == "Strip")
        .map(|(k, _)| k.clone())
        .collect();
    for key in &strip_owned {
        targets.remove(key);
    }
    if !strip_owned.is_empty() {
        let lanes = env["project"]["control"]["lanes"].as_object_mut().unwrap();
        lanes.retain(|_, l| {
            l["target"]
                .as_str()
                .is_none_or(|t| !strip_owned.iter().any(|k| k == t))
        });
        let assignments = env["project"]["control"]["assignments"]
            .as_object_mut()
            .unwrap();
        assignments.retain(|_, a| {
            [a["target"].as_str(), a["depth_source"].as_str()]
                .into_iter()
                .flatten()
                .all(|t| !strip_owned.iter().any(|k| k == t))
        });
    }
    let scope_of = |owner: &serde_json::Value| {
        let mut scope = serde_json::json!({"scope": owner["owner"]});
        if let Some(id) = owner.get("id") {
            scope["id"] = id.clone();
        }
        scope
    };
    for t in env["project"]["control"]["targets"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        t["owner"] = scope_of(&t["owner"]);
        t.as_object_mut().unwrap().remove("role");
        t.as_object_mut().unwrap().remove("time_scope");
    }
    for s in env["project"]["control"]["sources"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        s["owner"] = scope_of(&s["owner"]);
    }
    env
}

#[test]
fn v6_snapshot_migrates_owner_role_and_time_scope_deterministically() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let clip = add_clip(&mut c, track);
    c.apply(cmd(
        &c,
        ProjectOp::AddNote {
            clip,
            key: 60,
            start: BeatPoint::new(0, 1).unwrap(),
            len: BeatPoint::new(1, 1).unwrap(),
        },
    ))
    .unwrap();
    let note = *c.live().notes.keys().next().unwrap();
    for (owner, scope) in [
        (
            ControlOwner::Track(track),
            AutomationTimeScope::ProjectMusical,
        ),
        (ControlOwner::Clip(clip), AutomationTimeScope::ClipLocal),
        (ControlOwner::Note(note), AutomationTimeScope::NoteLocal),
        (ControlOwner::Project, AutomationTimeScope::ProjectMusical),
    ] {
        add_target(&mut c, owner, ParameterRole::Generic, scope).unwrap();
    }
    let watermark_before = c.live().id_watermark;
    let mut store = InMemoryStore::new();
    let saved_rev = c.live_revision();
    store
        .save(
            "p",
            saved_rev,
            &serde_json::to_vec(&as_v6_envelope(&c)).unwrap(),
        )
        .unwrap();

    let mut c2 = controller();
    c2.load(&store).unwrap();
    // Every *migrated* binding is `Generic` and derives its coordinate
    // strictly from the legacy scope. The bootstrap master-fader target
    // is not a migrated binding — the v7→v8 step re-seeds it canonically.
    for t in c2
        .live()
        .control
        .targets
        .values()
        .filter(|t| is_issuable_raw(t.id.raw()))
    {
        assert_eq!(t.role, ParameterRole::Generic);
        let expected = match t.owner {
            ControlOwner::Clip(_) => AutomationTimeScope::ClipLocal,
            ControlOwner::Note(_) => AutomationTimeScope::NoteLocal,
            _ => AutomationTimeScope::ProjectMusical,
        };
        assert_eq!(t.time_scope, expected, "owner {:?}", t.owner);
    }
    // The projection allocates no IDs and never touches the watermark.
    assert_eq!(c2.live().id_watermark, watermark_before);
    assert!(c2.live().validate_references().is_ok());

    // Determinism: the same v6 body projects identically every load.
    let mut c3 = controller();
    c3.load(&store).unwrap();
    assert_eq!(c2.live(), c3.live());

    // Saving publishes a new v7 snapshot; the v6 body stays immutable.
    let head_before = store.head().unwrap().unwrap();
    let saved = c2.save(&mut store).unwrap();
    assert_ne!(saved.head, head_before);
    let bytes = store.read_snapshot(&saved.head).unwrap();
    let env: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(env["schema_version"], serde_json::json!(SCHEMA_VERSION));
    let target = env["project"]["control"]["targets"]
        .as_object()
        .unwrap()
        .values()
        .next()
        .unwrap();
    // Fresh serialization emits `owner`/`role`/`time_scope` only.
    assert!(target["owner"].get("owner").is_some());
    assert!(target["owner"].get("scope").is_none());
    assert_eq!(target["role"], "generic");
    assert!(target.get("time_scope").is_some());
}

#[test]
fn migrated_clip_and_note_targets_evaluate_in_their_local_domains() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let clip = add_clip(&mut c, track);
    let clip_target = add_target(
        &mut c,
        ControlOwner::Clip(clip),
        ParameterRole::Generic,
        AutomationTimeScope::ClipLocal,
    )
    .unwrap();
    let lane = {
        c.apply(cmd(
            &c,
            ProjectOp::AddAutomationLane {
                target: clip_target,
            },
        ))
        .unwrap();
        *c.live().control.lanes.keys().next_back().unwrap()
    };
    c.apply(cmd(
        &c,
        ProjectOp::WriteAutomationRegion {
            lane,
            region: AutomationRegion {
                range: BeatRange::try_new(
                    BeatPoint::new(0, 1).unwrap(),
                    BeatPoint::new(4, 1).unwrap(),
                )
                .unwrap(),
                interpolation: Interpolation::Step,
                points: vec![AutomationPoint {
                    pos: BeatPoint::new(0, 1).unwrap(),
                    value: 0.9,
                }],
            },
            policy: ConflictPolicy::Reject,
        },
    ))
    .unwrap();
    let mut store = InMemoryStore::new();
    store
        .save(
            "p",
            c.live_revision(),
            &serde_json::to_vec(&as_v6_envelope(&c)).unwrap(),
        )
        .unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    // Clip-local points evaluate at clip.start + p — clip anchored at
    // tick 0 so project pos 1 == clip-local 1.
    let ev = evaluate_control(c2.live(), clip_target, BeatPoint::new(1, 1).unwrap(), None).unwrap();
    assert_eq!(ev.automation, Some(0.9));
    // Outside the clip extent the domain reports unavailable honestly.
    assert!(matches!(
        evaluate_control(c2.live(), clip_target, BeatPoint::new(40, 1).unwrap(), None),
        Err(Error::ScopeUnavailable { .. })
    ));
}

#[test]
fn eval_plan_staleness_tracks_time_scope_changes() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let target = add_target(
        &mut c,
        ControlOwner::Track(track),
        ParameterRole::Generic,
        AutomationTimeScope::ProjectMusical,
    )
    .unwrap();
    let plan = ControlGraph::compile(c.live()).unwrap();
    assert!(!plan.is_stale(c.live()));
    // A binding mutation (time_scope) is exactly what `is_stale`
    // freezes — recompile after changing it must diverge.
    let mut p = c.live().clone();
    p.control.targets.get_mut(&target).unwrap().time_scope = AutomationTimeScope::ProjectAbsolute;
    assert!(plan.is_stale(&p));
}

// ── CTL-01-SCHEMA: save/reload preserves the binding ────────────

#[test]
fn save_reload_preserves_owner_role_and_time_scope() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    // CTL-02: the seeded strip channel Fader is itself a Strip-owned
    // target — save/reload must preserve its binding untouched.
    assert!(strip_channel_target_id(c.live(), strip, ParameterRole::Fader).is_some());
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c.live(), c2.live());
    let bound = c2
        .live()
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlOwner::Strip(strip) && t.role == ParameterRole::Fader)
        .unwrap();
    assert_eq!(bound.time_scope, AutomationTimeScope::ProjectMusical);
}

// ── SV185: bootstrap master-strip fader target ──────────────────

/// The well-known fader target id every fresh project carries.
fn fader_target() -> ParameterTargetId {
    ParameterTargetId::new(MASTER_FADER_RAW)
}

#[test]
fn new_project_carries_the_canonical_master_strip_fader() {
    let c = controller();
    let p = c.live();
    let master_strip = p.routing.buses[&p.routing.master].strip;
    let t = p.control.targets.get(&fader_target()).unwrap();
    assert_eq!(t.owner, ControlOwner::Strip(master_strip));
    assert_eq!(t.role, ParameterRole::Fader);
    assert_eq!(t.time_scope, AutomationTimeScope::ProjectMusical);
    assert_eq!(t.kind, ValueKind::Continuous);
    assert_eq!(t.mapping, ParamMapping::Linear);
    assert!(t.capability.is_none());
    assert_eq!(t.base, 1.0);
    assert!(p.validate_references().is_ok());
}

#[test]
fn master_fader_evaluates_and_commits_through_set_parameter_base() {
    let mut c = controller();
    let id = fader_target();
    let ev = evaluate_control(c.live(), id, BeatPoint::new(0, 1).unwrap(), None).unwrap();
    assert_eq!(ev.host_normalized, 1.0);
    assert_eq!(ev.host_plain, 1.0);

    c.apply(cmd(
        &c,
        ProjectOp::SetParameterBase {
            target: id,
            value: 0.25,
        },
    ))
    .unwrap();
    assert_eq!(c.live().control.targets[&id].base, 0.25);
    let ev = evaluate_control(c.live(), id, BeatPoint::new(0, 1).unwrap(), None).unwrap();
    assert_eq!(ev.host_normalized, 0.25);
}

#[test]
fn master_fader_target_cannot_be_removed_or_shadowed() {
    let mut c = controller();
    let id = fader_target();
    let rev = c.live_revision();
    // Removal is refused — the bootstrap target is structural.
    assert!(matches!(
        c.apply(cmd(&c, ProjectOp::RemoveParameterTarget { target: id })),
        Err(Error::Validation(_))
    ));
    assert_eq!(c.live_revision(), rev);
    assert!(c.live().control.targets.contains_key(&id));

    // A second Fader-role target on the same owner violates the
    // singleton role rule — the bootstrap target already holds it.
    let master_strip = c.live().routing.buses[&c.live().routing.master].strip;
    assert!(
        add_target(
            &mut c,
            ControlOwner::Strip(master_strip),
            ParameterRole::Fader,
            AutomationTimeScope::ProjectMusical,
        )
        .is_err()
    );
    // A Generic target on the same owner is still legal.
    add_target(
        &mut c,
        ControlOwner::Strip(master_strip),
        ParameterRole::Generic,
        AutomationTimeScope::ProjectMusical,
    )
    .unwrap();
}

/// Serializes `project` as a schema-v7 envelope — the current wire
/// shape minus the bootstrap fader target, which v7 bodies predate.
fn as_v7_envelope_without_fader(c: &ProjectController) -> serde_json::Value {
    let mut env = serde_json::json!({
        "schema_version": 7,
        "project_id": c.live().project_id,
        "revision": c.live_revision(),
        "project": serde_json::to_value(c.live()).unwrap(),
    });
    env["project"]["control"]["targets"]
        .as_object_mut()
        .unwrap()
        .remove(&ParameterTargetId::new(MASTER_FADER_RAW).to_tagged());
    env
}

#[test]
fn v7_snapshot_regains_the_master_fader_target_on_load() {
    let c = controller();
    let mut store = InMemoryStore::new();
    let saved_rev = c.live_revision();
    store
        .save(
            "p",
            saved_rev,
            &serde_json::to_vec(&as_v7_envelope_without_fader(&c)).unwrap(),
        )
        .unwrap();

    let mut c2 = controller();
    c2.load(&store).unwrap();
    // The v7→v8 step re-seeds the canonical target — identical to a
    // fresh project's, and the load counts as one migrated revision.
    assert_eq!(
        c2.live().control.targets[&fader_target()],
        c.live().control.targets[&fader_target()]
    );
    assert_eq!(c2.live_revision(), ProjectRevision(saved_rev.0 + 1));
    assert!(c2.live().validate_references().is_ok());
}

#[test]
fn v9_snapshot_without_the_fader_target_is_rejected() {
    let c = controller();
    // A current-schema body — stripping the well-known
    // target makes it foreign state `validate_references` must refuse.
    let mut env = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "project_id": c.live().project_id,
        "revision": c.live_revision(),
        "project": serde_json::to_value(c.live()).unwrap(),
    });
    env["project"]["control"]["targets"]
        .as_object_mut()
        .unwrap()
        .remove(&ParameterTargetId::new(MASTER_FADER_RAW).to_tagged());

    let mut store = InMemoryStore::new();
    store
        .save("p", c.live_revision(), &serde_json::to_vec(&env).unwrap())
        .unwrap();
    let mut c2 = controller();
    assert!(matches!(c2.load(&store), Err(Error::Validation(_))));
}

#[test]
fn v7_snapshot_with_a_user_fader_rekeys_it_onto_the_well_known_id() {
    let mut c = controller();
    let master_strip = c.live().routing.buses[&c.live().routing.master].strip;
    // A v7-legal shape: an *issued* Fader-role target on the master
    // strip — legal then because nothing seeded the bootstrap target.
    // `add_target` writes Generic; the serialized v7 body below flips
    // the role, which is exactly what a v7 `AddParameterTarget`
    // caller could have persisted.
    let issued = add_target(
        &mut c,
        ControlOwner::Strip(master_strip),
        ParameterRole::Generic,
        AutomationTimeScope::ProjectMusical,
    )
    .unwrap();
    c.apply(cmd(&c, ProjectOp::AddAutomationLane { target: issued }))
        .unwrap();

    let mut env = as_v7_envelope_without_fader(&c);
    env["project"]["control"]["targets"][issued.to_tagged()]["role"] = serde_json::json!("fader");

    let mut store = InMemoryStore::new();
    store
        .save("p", c.live_revision(), &serde_json::to_vec(&env).unwrap())
        .unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();

    let p = c2.live();
    assert!(p.validate_references().is_ok());
    // The user target *became* the canonical fader — same entity,
    // well-known ID — not a second Fader beside it.
    let fader = &p.control.targets[&fader_target()];
    assert_eq!(fader.role, ParameterRole::Fader);
    assert_eq!(fader.owner, ControlOwner::Strip(master_strip));
    assert!(!p.control.targets.contains_key(&issued));
    // The automation lane that fed the issued ID now feeds the
    // well-known one.
    assert!(p.control.lanes.values().all(|l| l.target == fader_target()));
}

#[test]
fn v9_snapshot_with_a_foreign_entry_at_the_fader_id_is_rejected() {
    let c = controller();
    // The well-known ID is the app's lookup key for "the master
    // strip's fader" — an entry that squats on it while carrying a
    // foreign role is as invalid as a missing one.
    let mut env = serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "project_id": c.live().project_id,
        "revision": c.live_revision(),
        "project": serde_json::to_value(c.live()).unwrap(),
    });
    env["project"]["control"]["targets"][fader_target().to_tagged()]["role"] =
        serde_json::json!("generic");

    let mut store = InMemoryStore::new();
    store
        .save("p", c.live_revision(), &serde_json::to_vec(&env).unwrap())
        .unwrap();
    let mut c2 = controller();
    assert!(matches!(c2.load(&store), Err(Error::Validation(_))));
}
