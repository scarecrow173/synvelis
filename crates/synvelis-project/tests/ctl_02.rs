//! CTL-02 (issue #122): Host Parameter 正本 — strip channel param
//! registration, the send `gain_target` single authority, adoption /
//! removal guards, the stage-1 semantic plugin binding, and the
//! v9 → v10 snapshot migration.

use ruviform_plugin_core::{ParamFlags, ParamId};
use synvelis_project::*;

/// The seeded channel roles — mirrors `STRIP_SEED_ROLES` (crate-private);
/// `Pan` registers on first use, not at creation.
const SEED_ROLES: [ParameterRole; 3] = [
    ParameterRole::Fader,
    ParameterRole::Mute,
    ParameterRole::Solo,
];

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

fn strip_main(strip: StripId, direction: PortDirection) -> PortRef {
    PortRef::StripMain {
        strip: StripRef::Existing(strip),
        direction,
    }
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

fn strip_targets(p: &Project, strip: StripId) -> Vec<&ParameterTarget> {
    p.control
        .targets
        .values()
        .filter(|t| t.owner == ControlOwner::Strip(strip))
        .collect()
}

// ── CTL-02 registration: every strip seeds its channel params ────

#[test]
fn strip_creation_seeds_bit_exact_channel_params() {
    let mut c = controller();
    // The bootstrap master strip carries the seeded set under
    // well-known IDs from `Project::new`.
    let master = master_strip(c.live());
    let master_targets = strip_targets(c.live(), master);
    assert_eq!(master_targets.len(), 3, "{master_targets:?}");
    for (role, kind, base) in [
        (ParameterRole::Fader, ValueKind::Continuous, 1.0),
        (ParameterRole::Mute, ValueKind::Boolean, 0.0),
        (ParameterRole::Solo, ValueKind::Boolean, 0.0),
    ] {
        let t = master_targets
            .iter()
            .find(|t| t.role == role)
            .unwrap_or_else(|| panic!("master strip lacks {role:?}"));
        assert_eq!(t.kind, kind);
        assert!(!is_issuable_raw(t.id.raw()), "well-known id");
        assert_eq!(t.base, base);
        assert_eq!(t.time_scope, AutomationTimeScope::ProjectMusical);
        assert_eq!(t.capability, None);
    }
    // `Pan` is deliberately not seeded — its canonical center is not
    // bit-exact with the legacy bypass (see `STRIP_SEED_ROLES`).
    assert!(
        strip_channel_target_id(c.live(), master, ParameterRole::Pan).is_none(),
        "master strip must not carry a seeded Pan"
    );

    // A track strip seeds the same set under issued IDs.
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    let targets = strip_targets(c.live(), strip);
    assert_eq!(targets.len(), 3, "{targets:?}");
    for t in &targets {
        assert!(is_issuable_raw(t.id.raw()));
        assert_eq!(t.owner, ControlOwner::Strip(strip));
        assert_eq!(t.mapping, ParamMapping::Linear);
        assert_eq!(t.capability, None);
        let expected_base = match t.role {
            ParameterRole::Fader => 1.0,
            ParameterRole::Mute | ParameterRole::Solo => 0.0,
            other => panic!("unexpected seeded role {other:?}"),
        };
        assert_eq!(t.base, expected_base, "role {:?}", t.role);
    }
    // Singleton lookups resolve the registered authority.
    for role in SEED_ROLES {
        assert!(
            strip_channel_target_id(c.live(), strip, role).is_some(),
            "{role:?} lookup"
        );
    }
    assert!(strip_channel_target_id(c.live(), strip, ParameterRole::Pan).is_none());

    // A bus strip seeds identically.
    let bus = add_bus(&mut c, BusKind::Group, "bus");
    let bus_strip = c.live().routing.buses[&bus].strip;
    assert_eq!(strip_targets(c.live(), bus_strip).len(), 3);
}

#[test]
fn channel_params_are_the_single_base_authority() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    let fader = strip_channel_target_id(c.live(), strip, ParameterRole::Fader).unwrap();
    let mute = strip_channel_target_id(c.live(), strip, ParameterRole::Mute).unwrap();

    // `SetParameterBase` writes the persisted authority; lowering
    // reads the same `base` — no second stored gain.
    c.apply(cmd(
        &c,
        ProjectOp::SetParameterBase {
            target: fader,
            value: 0.5,
        },
    ))
    .unwrap();
    assert_eq!(c.live().control.targets[&fader].base, 0.5);

    // `SetStripMute` binds the same registered target.
    c.apply(cmd(
        &c,
        ProjectOp::SetStripMute {
            strip_id: strip,
            muted: true,
        },
    ))
    .unwrap();
    assert_eq!(c.live().control.targets[&mute].base, 1.0);
    assert_eq!(
        strip_channel_target_id(c.live(), strip, ParameterRole::Mute),
        Some(mute)
    );
}

// ── CTL-02 send gain: one persisted authority, atomic creation ───

#[test]
fn send_create_commits_gain_target_in_one_undo() {
    let mut c = controller();
    let track = add_track(&mut c, "vox");
    let strip = add_track_strip(&mut c, track);
    let ret = add_bus(&mut c, BusKind::Return, "verb");
    let ret_strip = c.live().routing.buses[&ret].strip;
    let rev_before = c.live_revision();
    let before = serde_json::to_value(c.live()).unwrap();

    let it = intent(
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
                tap: SendTap::PostFader,
                level: SendGainSpec::Create {
                    name: "to verb".into(),
                    initial: 0.75,
                },
                channel_map: None,
            },
        ],
    );
    apply_intent(&mut c, it).unwrap();

    assert_eq!(c.live_revision().0, rev_before.0 + 1, "one CAS");
    let send_id = *c.live().routing.sends.keys().next().unwrap();
    let send = &c.live().routing.sends[&send_id];
    let target_id = send.gain_target.expect("new send carries a target");
    assert_eq!(send_level_target_id(c.live(), send_id), Some(target_id));
    let target = &c.live().control.targets[&target_id];
    assert_eq!(target.owner, ControlOwner::Send(send_id));
    assert_eq!(target.role, ParameterRole::SendLevel);
    assert_eq!(target.kind, ValueKind::Continuous);
    assert_eq!(target.capability, None, "internal gain — no ParamId");
    assert!((target.base - 0.75).abs() < f64::EPSILON);

    // The persisted authority is the target's `base` — `SendConfig`
    // stores no second gain value to diverge.
    c.apply(cmd(
        &c,
        ProjectOp::SetParameterBase {
            target: target_id,
            value: 0.25,
        },
    ))
    .unwrap();
    assert_eq!(c.live().control.targets[&target_id].base, 0.25);
    assert_eq!(
        c.live().routing.sends[&send_id].gain_target,
        Some(target_id),
        "reference unchanged — value lives only on the target"
    );

    // Two committed revisions (intent + base write) → two undos
    // restore the exact pre-gesture state — send and its target die
    // together, no orphan.
    c.undo().unwrap();
    c.undo().unwrap();
    assert_eq!(serde_json::to_value(c.live()).unwrap(), before);
    assert!(c.live().routing.sends.is_empty());
}

#[test]
fn failed_send_create_leaves_project_and_undo_untouched() {
    let mut c = controller();
    let track = add_track(&mut c, "vox");
    let strip = add_track_strip(&mut c, track);
    let ret = add_bus(&mut c, BusKind::Return, "verb");
    let ret_strip = c.live().routing.buses[&ret].strip;
    let before = serde_json::to_value(c.live()).unwrap();
    let sends_before = c.live().routing.sends.len();

    // Adopting a *registered strip channel target* is rejected — the
    // whole intent fails atomically: no send, no re-owned target, no
    // undo step.
    let fader = strip_channel_target_id(c.live(), strip, ParameterRole::Fader).unwrap();
    let it = intent(
        &c,
        vec![RoutingEdit::CreateSend {
            temp_id: TempSendId(1),
            temp_connection: TempConnectionId(1),
            source: strip_main(strip, PortDirection::Output),
            destination: strip_main(ret_strip, PortDirection::Input),
            tap: SendTap::PostFader,
            level: SendGainSpec::Adopt(fader),
            channel_map: None,
        }],
    );
    let result = apply_intent(&mut c, it);
    assert!(result.is_err(), "strip-owned target adoption rejected");
    assert_eq!(serde_json::to_value(c.live()).unwrap(), before);
    assert_eq!(c.live().routing.sends.len(), sends_before);
    assert_eq!(
        c.live().control.targets[&fader].owner,
        ControlOwner::Strip(strip),
        "fader target was never re-owned"
    );
    // Undo history untouched: the next undo restores the *previous*
    // committed state, not a phantom send.
    c.undo().unwrap();
    assert!(c.live().routing.sends.is_empty());
}

#[test]
fn low_level_add_send_keeps_unconfigured_option() {
    let mut c = controller();
    let track = add_track(&mut c, "vox");
    let strip = add_track_strip(&mut c, track);
    // `AddSend { gain_target: None }` remains the explicit
    // legacy/unconfigured representation — the transaction path is
    // what requires a target for *new enabled* sends.
    let main_port = |s: StripId, dir: PortDirection| {
        c.live().routing.strips[&s]
            .port_ids
            .iter()
            .find(|p| {
                c.live().routing.ports[p].direction == dir
                    && c.live().routing.ports[p].role == PortRole::Main
            })
            .copied()
            .unwrap()
    };
    let (src, dst) = (
        main_port(strip, PortDirection::Output),
        main_port(master_strip(c.live()), PortDirection::Input),
    );
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
    let send = c.live().routing.sends.values().next().unwrap();
    assert_eq!(send.gain_target, None);
    assert_eq!(send_level_target_id(c.live(), send.id), None);
}

#[test]
fn adoption_and_removal_guards_protect_registered_targets() {
    let mut c = controller();
    let track = add_track(&mut c, "vox");
    let strip = add_track_strip(&mut c, track);
    let ret = add_bus(&mut c, BusKind::Return, "verb");
    let ret_strip = c.live().routing.buses[&ret].strip;
    let fader = strip_channel_target_id(c.live(), strip, ParameterRole::Fader).unwrap();

    // Direct `AddSend` adoption of a strip channel param → rejected.
    let (src, dst) = {
        let sp = c.live().routing.strips[&strip]
            .port_ids
            .iter()
            .find(|p| {
                c.live().routing.ports[p].direction == PortDirection::Output
                    && c.live().routing.ports[p].role == PortRole::Main
            })
            .copied()
            .unwrap();
        let dp = c.live().routing.strips[&ret_strip]
            .port_ids
            .iter()
            .find(|p| {
                c.live().routing.ports[p].direction == PortDirection::Input
                    && c.live().routing.ports[p].role == PortRole::Main
            })
            .copied()
            .unwrap();
        (sp, dp)
    };
    assert!(matches!(
        c.apply(cmd(
            &c,
            ProjectOp::AddSend {
                source: src,
                destination: dst,
                tap: SendTap::PostFader,
                gain_target: Some(fader),
            },
        )),
        Err(Error::Validation(_))
    ));

    // `SetSendGainTarget` adoption is guarded identically.
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
    let send_id = *c.live().routing.sends.keys().next().unwrap();
    assert!(matches!(
        c.apply(cmd(
            &c,
            ProjectOp::SetSendGainTarget {
                send_id,
                gain_target: Some(fader),
            },
        )),
        Err(Error::Validation(_))
    ));

    // Registered channel targets cannot be removed; a free-form strip
    // param can.
    assert!(matches!(
        c.apply(cmd(&c, ProjectOp::RemoveParameterTarget { target: fader })),
        Err(Error::Validation(_))
    ));
    c.apply(cmd(
        &c,
        ProjectOp::AddParameterTarget {
            name: "aux".into(),
            owner: ControlOwner::Strip(strip),
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.0,
        },
    ))
    .unwrap();
    let aux = c
        .live()
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlOwner::Strip(strip) && t.role == ParameterRole::Generic)
        .unwrap()
        .id;
    c.apply(cmd(&c, ProjectOp::RemoveParameterTarget { target: aux }))
        .unwrap();
}

#[test]
fn pan_registers_on_first_use_with_canonical_spec() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    assert!(strip_channel_target_id(c.live(), strip, ParameterRole::Pan).is_none());

    c.apply(cmd(
        &c,
        ProjectOp::AddParameterTarget {
            name: "Pan".into(),
            owner: ControlOwner::Strip(strip),
            role: ParameterRole::Pan,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    ))
    .unwrap();
    let pan = strip_channel_target_id(c.live(), strip, ParameterRole::Pan).unwrap();
    let t = &c.live().control.targets[&pan];
    assert_eq!(t.kind, ValueKind::Continuous);
    assert_eq!(t.base, 0.5, "canonical center");
    assert_eq!(t.capability, None);

    // Singleton: a second Pan on the same strip is rejected.
    assert!(
        c.apply(cmd(
            &c,
            ProjectOp::AddParameterTarget {
                name: "Pan2".into(),
                owner: ControlOwner::Strip(strip),
                role: ParameterRole::Pan,
                time_scope: AutomationTimeScope::ProjectMusical,
                kind: ValueKind::Continuous,
                mapping: ParamMapping::Linear,
                capability: None,
                base: 0.5,
            },
        ))
        .is_err()
    );
}

// ── CTL-02 canonical internal spec enforcement ─────────────────

/// The `(kind, mapping, time_scope, capability)` fields of an
/// `AddParameterTarget` spec under test.
type SpecFields = (
    ValueKind,
    ParamMapping,
    AutomationTimeScope,
    Option<TargetCapability>,
);

/// Non-canonical specs for an internal channel/send target —
/// `SpecFields` tuples that deviate from the canonical contract in
/// one field each.
const NON_CANONICAL_SPECS: [SpecFields; 4] = [
    // `kind`: Boolean where the canonical channel/send spec is
    // Continuous (or Boolean for Mute/Solo — Pan is Continuous).
    (
        ValueKind::Boolean,
        ParamMapping::Linear,
        AutomationTimeScope::ProjectMusical,
        None,
    ),
    // `mapping`: Exponential where canonical is Linear.
    (
        ValueKind::Continuous,
        ParamMapping::Exponential,
        AutomationTimeScope::ProjectMusical,
        None,
    ),
    // `time_scope`: ProjectAbsolute where canonical is ProjectMusical.
    (
        ValueKind::Continuous,
        ParamMapping::Linear,
        AutomationTimeScope::ProjectAbsolute,
        None,
    ),
    // `capability`: `Some` — internal channel gains carry no ParamId.
    (
        ValueKind::Continuous,
        ParamMapping::Linear,
        AutomationTimeScope::ProjectMusical,
        Some(TargetCapability {
            param_id: 1,
            flags_bits: ParamFlags::AUTOMATABLE.bits(),
            min_value: 0.0,
            max_value: 1.0,
            default_value: 0.0,
        }),
    ),
];

/// Asserts `AddParameterTarget` rejects `owner`/`role` carrying the
/// given spec fields.
fn expect_target_reject(
    c: &mut ProjectController,
    owner: ControlOwner,
    role: ParameterRole,
    (kind, mapping, time_scope, capability): SpecFields,
) {
    assert!(
        matches!(
            c.apply(cmd(
                c,
                ProjectOp::AddParameterTarget {
                    name: "x".into(),
                    owner,
                    role,
                    time_scope,
                    kind,
                    mapping,
                    capability,
                    base: 0.5,
                },
            )),
            Err(Error::Validation(_))
        ),
        "{owner:?} {role:?} target spec must reject"
    );
}

/// `AddParameterTarget` enforces the canonical persisted spec for
/// strip channel roles (`Fader`/`Pan`/`Mute`/`Solo`): the same
/// `kind`/`mapping`/`time_scope`/`capability: None` contract the
/// canonical factory seeds — `name`/`base` stay authored. Anything
/// else would persist a shape no authority lookup or lowering path
/// defines (e.g. a `Pan` carrying a plugin `ParamId`).
#[test]
fn strip_channel_targets_require_the_canonical_spec() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);

    // `Pan` is the only channel role reachable through this op — the
    // seeded roles are already singleton-occupied at strip creation.
    // Every non-canonical deviation rejects before anything persists.
    for spec in NON_CANONICAL_SPECS {
        expect_target_reject(&mut c, ControlOwner::Strip(strip), ParameterRole::Pan, spec);
    }
    // Out-of-range `base` still rejects through the shared check.
    assert!(matches!(
        c.apply(cmd(
            &c,
            ProjectOp::AddParameterTarget {
                name: "pan".into(),
                owner: ControlOwner::Strip(strip),
                role: ParameterRole::Pan,
                time_scope: AutomationTimeScope::ProjectMusical,
                kind: ValueKind::Continuous,
                mapping: ParamMapping::Linear,
                capability: None,
                base: 1.5,
            },
        )),
        Err(Error::Validation(_))
    ));
    assert!(strip_channel_target_id(c.live(), strip, ParameterRole::Pan).is_none());

    // The canonical spec is accepted — `base` is authored (0.25 is a
    // legal persisted value; only the spec fields are fixed).
    c.apply(cmd(
        &c,
        ProjectOp::AddParameterTarget {
            name: "Pan".into(),
            owner: ControlOwner::Strip(strip),
            role: ParameterRole::Pan,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.25,
        },
    ))
    .unwrap();
    let pan = strip_channel_target_id(c.live(), strip, ParameterRole::Pan).unwrap();
    assert_eq!(c.live().control.targets[&pan].base, 0.25);
}

/// `Send`-owned `SendLevel` targets carry the same canonical
/// contract — the `SendGainSpec::Create` staging path writes exactly
/// this shape — and the `SendLevel` role is rejected on any non-Send
/// owner as an orphan no send's `gain_target` can reference.
#[test]
fn send_level_targets_require_the_canonical_spec() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    let ret = add_bus(&mut c, BusKind::Return, "verb");
    let ret_strip = c.live().routing.buses[&ret].strip;
    let main_port = |s: StripId, dir: PortDirection| {
        c.live().routing.strips[&s]
            .port_ids
            .iter()
            .find(|p| {
                c.live().routing.ports[p].direction == dir
                    && c.live().routing.ports[p].role == PortRole::Main
            })
            .copied()
            .unwrap()
    };
    c.apply(cmd(
        &c,
        ProjectOp::AddSend {
            source: main_port(strip, PortDirection::Output),
            destination: main_port(ret_strip, PortDirection::Input),
            tap: SendTap::PostFader,
            gain_target: None,
        },
    ))
    .unwrap();
    let send = *c.live().routing.sends.keys().next().unwrap();

    for spec in NON_CANONICAL_SPECS {
        expect_target_reject(
            &mut c,
            ControlOwner::Send(send),
            ParameterRole::SendLevel,
            spec,
        );
    }

    // Canonical `SendLevel` is accepted and binds through the
    // existing low-level `SetSendGainTarget` path.
    c.apply(cmd(
        &c,
        ProjectOp::AddParameterTarget {
            name: "lvl".into(),
            owner: ControlOwner::Send(send),
            role: ParameterRole::SendLevel,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    ))
    .unwrap();
    let lvl = c
        .live()
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlOwner::Send(send) && t.role == ParameterRole::SendLevel)
        .unwrap()
        .id;
    c.apply(cmd(
        &c,
        ProjectOp::SetSendGainTarget {
            send_id: send,
            gain_target: Some(lvl),
        },
    ))
    .unwrap();
    assert_eq!(send_level_target_id(c.live(), send), Some(lvl));

    // `SendLevel` on a non-Send owner is an orphan — `send_level_target_id`
    // resolves `Send`-owned `SendLevel` targets only.
    let canonical = (
        ValueKind::Continuous,
        ParamMapping::Linear,
        AutomationTimeScope::ProjectMusical,
        None,
    );
    for owner in [ControlOwner::Track(track), ControlOwner::Project] {
        expect_target_reject(&mut c, owner, ParameterRole::SendLevel, canonical);
    }
}

// ── CTL-02 adoption: canonical SendLevel spec enforcement ──────

/// A free-form (non-internal) target carrying the given spec fields —
/// Track-owned `Generic` targets keep authored specs, so the
/// adoption guard is what must reject the non-canonical ones.
fn add_free_target(
    c: &mut ProjectController,
    owner: ControlOwner,
    (kind, mapping, time_scope, capability): SpecFields,
) -> ParameterTargetId {
    c.apply(cmd(
        c,
        ProjectOp::AddParameterTarget {
            name: "adopt-me".into(),
            owner,
            role: ParameterRole::Generic,
            time_scope,
            kind,
            mapping,
            capability,
            base: 0.5,
        },
    ))
    .unwrap();
    *c.live()
        .control
        .targets
        .keys()
        .rfind(|id| is_issuable_raw(id.raw()))
        .unwrap()
}

/// Strip main port by direction — `ProjectOp::AddSend` wants the
/// resolved `SemanticPortId`, not the `PortRef` used by intents.
fn main_port_id(c: &ProjectController, strip: StripId, dir: PortDirection) -> SemanticPortId {
    c.live().routing.strips[&strip]
        .port_ids
        .iter()
        .find(|p| {
            c.live().routing.ports[p].direction == dir
                && c.live().routing.ports[p].role == PortRole::Main
        })
        .copied()
        .unwrap()
}

/// Fixture for adoption tests: a track strip sending to a return
/// bus, plus one unconfigured send for `SetSendGainTarget` rejects.
struct AdoptFixture {
    c: ProjectController,
    track: TrackId,
    strip: StripId,
    ret_strip: StripId,
    send: SendId,
    src: SemanticPortId,
    dst: SemanticPortId,
}

fn adopt_fixture() -> AdoptFixture {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    let ret = add_bus(&mut c, BusKind::Return, "verb");
    let ret_strip = c.live().routing.buses[&ret].strip;
    let src = main_port_id(&c, strip, PortDirection::Output);
    let dst = main_port_id(&c, ret_strip, PortDirection::Input);
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
    let send = *c.live().routing.sends.keys().next().unwrap();
    AdoptFixture {
        c,
        track,
        strip,
        ret_strip,
        send,
        src,
        dst,
    }
}

/// Issue #122 residual: adoption re-owns the target as
/// `Send`+`SendLevel` at apply without rewriting its spec — a target
/// whose authored spec deviates from the canonical send-level
/// contract must be rejected before anything persists, on both the
/// low-level ops and the `RoutingEditIntent` transaction path.
/// `ProjectAbsolute` passes the legacy "project-time" check but is
/// not the canonical `ProjectMusical` scope, so it rejects too.
#[test]
fn adopted_send_gain_targets_reject_non_canonical_specs() {
    let mut f = adopt_fixture();
    for spec in NON_CANONICAL_SPECS {
        let target = add_free_target(&mut f.c, ControlOwner::Track(f.track), spec);
        let before = serde_json::to_value(f.c.live()).unwrap();

        // Low-level `AddSend` adoption → reject, nothing persists.
        assert!(
            matches!(
                f.c.apply(cmd(
                    &f.c,
                    ProjectOp::AddSend {
                        source: f.src,
                        destination: f.dst,
                        tap: SendTap::PostFader,
                        gain_target: Some(target),
                    },
                )),
                Err(Error::Validation(_))
            ),
            "AddSend must reject a non-canonical gain target"
        );
        assert_eq!(serde_json::to_value(f.c.live()).unwrap(), before);

        // Low-level `SetSendGainTarget` adoption → same reject.
        assert!(
            matches!(
                f.c.apply(cmd(
                    &f.c,
                    ProjectOp::SetSendGainTarget {
                        send_id: f.send,
                        gain_target: Some(target),
                    },
                )),
                Err(Error::Validation(_))
            ),
            "SetSendGainTarget must reject a non-canonical gain target"
        );
        assert_eq!(serde_json::to_value(f.c.live()).unwrap(), before);

        // `RoutingEditIntent` paths reject atomically — no partial
        // send/target state survives a failed intent.
        let it = intent(
            &f.c,
            vec![RoutingEdit::CreateSend {
                temp_id: TempSendId(1),
                temp_connection: TempConnectionId(1),
                source: strip_main(f.strip, PortDirection::Output),
                destination: strip_main(f.ret_strip, PortDirection::Input),
                tap: SendTap::PostFader,
                level: SendGainSpec::Adopt(target),
                channel_map: None,
            }],
        );
        assert!(apply_intent(&mut f.c, it).is_err());
        assert_eq!(serde_json::to_value(f.c.live()).unwrap(), before);

        let it = intent(
            &f.c,
            vec![RoutingEdit::SetSendGainTarget {
                send: SendRef::Existing(f.send),
                target: Some(target),
            }],
        );
        assert!(apply_intent(&mut f.c, it).is_err());
        assert_eq!(serde_json::to_value(f.c.live()).unwrap(), before);
    }
}

/// The canonical spec is accepted on the low-level ops — apply seals
/// `Send`+`SendLevel` only; `name`/`base` stay authored.
#[test]
fn canonical_send_gain_targets_adopt_and_seal() {
    let mut f = adopt_fixture();
    let canonical = (
        ValueKind::Continuous,
        ParamMapping::Linear,
        AutomationTimeScope::ProjectMusical,
        None,
    );

    let target = add_free_target(&mut f.c, ControlOwner::Track(f.track), canonical);
    f.c.apply(cmd(
        &f.c,
        ProjectOp::SetSendGainTarget {
            send_id: f.send,
            gain_target: Some(target),
        },
    ))
    .unwrap();
    let t = &f.c.live().control.targets[&target];
    assert_eq!(t.owner, ControlOwner::Send(f.send));
    assert_eq!(t.role, ParameterRole::SendLevel);
    assert_eq!(t.name, "adopt-me");
    assert_eq!(t.base, 0.5);
    assert_eq!(t.kind, ValueKind::Continuous);
    assert_eq!(t.mapping, ParamMapping::Linear);
    assert_eq!(t.time_scope, AutomationTimeScope::ProjectMusical);
    assert_eq!(t.capability, None);
    assert_eq!(send_level_target_id(f.c.live(), f.send), Some(target));

    // `AddSend` adoption of a canonical free target seals the same
    // way — the persisted `SendLevel` is canonical by construction.
    let target2 = add_free_target(&mut f.c, ControlOwner::Track(f.track), canonical);
    f.c.apply(cmd(
        &f.c,
        ProjectOp::AddSend {
            source: f.src,
            destination: f.dst,
            tap: SendTap::PreFader,
            gain_target: Some(target2),
        },
    ))
    .unwrap();
    let send2 =
        f.c.live()
            .routing
            .sends
            .values()
            .find(|s| s.gain_target == Some(target2))
            .unwrap()
            .id;
    assert_eq!(
        f.c.live().control.targets[&target2].owner,
        ControlOwner::Send(send2)
    );
    assert_eq!(send_level_target_id(f.c.live(), send2), Some(target2));
}

/// `SendGainSpec::Adopt` through the intent path binds a canonical
/// target identically — the persisted `SendLevel` is canonical.
#[test]
fn canonical_send_gain_targets_adopt_via_routing_intent() {
    let mut f = adopt_fixture();
    let canonical = (
        ValueKind::Continuous,
        ParamMapping::Linear,
        AutomationTimeScope::ProjectMusical,
        None,
    );

    let target = add_free_target(&mut f.c, ControlOwner::Track(f.track), canonical);
    let it = intent(
        &f.c,
        vec![RoutingEdit::CreateSend {
            temp_id: TempSendId(1),
            temp_connection: TempConnectionId(1),
            source: strip_main(f.strip, PortDirection::Output),
            destination: strip_main(f.ret_strip, PortDirection::Input),
            tap: SendTap::PostFader,
            level: SendGainSpec::Adopt(target),
            channel_map: None,
        }],
    );
    apply_intent(&mut f.c, it).unwrap();
    let send =
        f.c.live()
            .routing
            .sends
            .values()
            .find(|s| s.gain_target == Some(target))
            .unwrap()
            .id;
    let t = &f.c.live().control.targets[&target];
    assert_eq!(t.owner, ControlOwner::Send(send));
    assert_eq!(t.role, ParameterRole::SendLevel);
    assert_eq!(t.name, "adopt-me");
    assert_eq!(send_level_target_id(f.c.live(), send), Some(target));

    // `RoutingEdit::SetSendGainTarget` rebinds a canonical target the
    // same way.
    let target2 = add_free_target(&mut f.c, ControlOwner::Track(f.track), canonical);
    let it = intent(
        &f.c,
        vec![RoutingEdit::SetSendGainTarget {
            send: SendRef::Existing(f.send),
            target: Some(target2),
        }],
    );
    apply_intent(&mut f.c, it).unwrap();
    assert_eq!(
        f.c.live().control.targets[&target2].owner,
        ControlOwner::Send(f.send)
    );
    assert_eq!(send_level_target_id(f.c.live(), f.send), Some(target2));
}

// ── CTL-02 stage-1 semantic plugin binding ──────────────────────

fn add_processor(c: &mut ProjectController, strip: StripId, resolved: bool) -> ProcessorId {
    c.apply(cmd(
        c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "fx".into(),
            binding: if resolved {
                ProcessorBinding::Resolved(PluginRef {
                    descriptor: "org.example.gain".into(),
                    version: "1.0.0".into(),
                    capability_revision: 3,
                })
            } else {
                ProcessorBinding::Unresolved {
                    descriptor: "org.example.gain".into(),
                }
            },
            sidechain_input: false,
        },
    ))
    .unwrap();
    *c.live().routing.processors.keys().next_back().unwrap()
}

fn add_plugin_param(
    c: &mut ProjectController,
    processor: ProcessorId,
    param_id: u32,
) -> ParameterTargetId {
    c.apply(cmd(
        c,
        ProjectOp::AddParameterTarget {
            name: "gain".into(),
            owner: ControlOwner::Processor(processor),
            role: ParameterRole::ProcessorParam,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: Some(TargetCapability {
                param_id,
                flags_bits: ParamFlags::AUTOMATABLE.bits(),
                min_value: 0.0,
                max_value: 1.0,
                default_value: 0.0,
            }),
            base: 0.0,
        },
    ))
    .unwrap();
    c.live()
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlOwner::Processor(processor))
        .unwrap()
        .id
}

#[test]
fn semantic_plugin_binding_resolves_stage_one() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    let processor = add_processor(&mut c, strip, true);
    let target = add_plugin_param(&mut c, processor, 7);

    let binding = semantic_plugin_binding(c.live(), target).unwrap();
    assert_eq!(binding.target, target);
    assert_eq!(binding.processor, processor);
    assert_eq!(binding.plugin.descriptor, "org.example.gain");
    assert_eq!(binding.plugin.version, "1.0.0");
    assert_eq!(binding.plugin.capability_revision, 3);
    assert_eq!(binding.param_id, ParamId(7));

    // Same descriptor on a *different* processor is a different
    // binding — plugin ID reuse never aliases a target.
    let processor2 = add_processor(&mut c, strip, true);
    let target2 = add_plugin_param(&mut c, processor2, 7);
    let binding2 = semantic_plugin_binding(c.live(), target2).unwrap();
    assert_ne!(binding.processor, binding2.processor);
    assert_ne!(binding.target, binding2.target);

    // Fail-closed paths.
    let fader = strip_channel_target_id(c.live(), strip, ParameterRole::Fader).unwrap();
    assert!(matches!(
        semantic_plugin_binding(c.live(), fader),
        Err(Error::Validation(_))
    ));
    assert!(matches!(
        semantic_plugin_binding(c.live(), ParameterTargetId::new(0xDEAD)),
        Err(Error::NotFound(_))
    ));

    // Unresolved plugin binding → typed rejection, never a guess.
    let unresolved = add_processor(&mut c, strip, false);
    let unresolved_target = add_plugin_param(&mut c, unresolved, 9);
    assert!(matches!(
        semantic_plugin_binding(c.live(), unresolved_target),
        Err(Error::Validation(_))
    ));

    // A processor param without a capability contract cannot bind.
    c.apply(cmd(
        &c,
        ProjectOp::AddParameterTarget {
            name: "plain".into(),
            owner: ControlOwner::Processor(processor),
            role: ParameterRole::ProcessorParam,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.0,
        },
    ))
    .unwrap();
    let no_cap = c
        .live()
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlOwner::Processor(processor) && t.capability.is_none())
        .unwrap()
        .id;
    assert!(matches!(
        semantic_plugin_binding(c.live(), no_cap),
        Err(Error::Validation(_))
    ));
}

// ── PC-C4 plugin-binding: stage-1.5 canonical instance mapping ────

#[test]
fn plugin_parameter_instance_binding_resolves_persisted_instance() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let strip = add_track_strip(&mut c, track);
    let processor = add_processor(&mut c, strip, true);
    let target = add_plugin_param(&mut c, processor, 7);

    // The op path syncs `plugin_instances` — the store vouches for
    // the semantic binding's `(processor, plugin)` pair.
    let resolved = plugin_parameter_instance_binding(c.live(), target).unwrap();
    assert_eq!(resolved.semantic.target, target);
    assert_eq!(resolved.semantic.processor, processor);
    assert_eq!(resolved.semantic.param_id, ParamId(7));
    let instance = resolved.instance.expect("store must vouch");
    assert_eq!(
        instance,
        c.live()
            .plugin_instances
            .resolve(processor, &resolved.semantic.plugin)
            .unwrap()
    );

    // A second processor on the same plugin descriptor resolves to a
    // *different* persisted instance — no aliasing.
    let processor2 = add_processor(&mut c, strip, true);
    let target2 = add_plugin_param(&mut c, processor2, 7);
    let resolved2 = plugin_parameter_instance_binding(c.live(), target2).unwrap();
    let inst1 = c
        .live()
        .plugin_instances
        .resolve(processor, &resolved.semantic.plugin);
    let inst2 = c
        .live()
        .plugin_instances
        .resolve(processor2, &resolved2.semantic.plugin);
    assert!(inst1.is_some() && inst2.is_some());
    assert_ne!(inst1, inst2);
    assert_eq!(resolved2.instance, inst2);

    // Store entry missing → `instance: None` (fail-closed data, not
    // an error) while the semantic half still resolves.
    let mut stripped = c.live().clone();
    stripped.plugin_instances.instances.clear();
    let resolved = plugin_parameter_instance_binding(&stripped, target).unwrap();
    assert!(resolved.instance.is_none());
    assert_eq!(resolved.semantic.param_id, ParamId(7));

    // Store entry drifted to a different PluginRef → `None`.
    let mut drifted = c.live().clone();
    drifted
        .plugin_instances
        .instances
        .get_mut(&processor)
        .unwrap()
        .plugin = PluginRef {
        descriptor: "org.example.other".into(),
        ..drifted.plugin_instances.instances[&processor]
            .plugin
            .clone()
    };
    let resolved = plugin_parameter_instance_binding(&drifted, target).unwrap();
    assert!(resolved.instance.is_none());

    // Semantic failures still surface as `Err` — same contract as
    // `semantic_plugin_binding`.
    let fader = strip_channel_target_id(c.live(), strip, ParameterRole::Fader).unwrap();
    assert!(matches!(
        plugin_parameter_instance_binding(c.live(), fader),
        Err(Error::Validation(_))
    ));
    assert!(matches!(
        plugin_parameter_instance_binding(c.live(), ParameterTargetId::new(0xDEAD)),
        Err(Error::NotFound(_))
    ));
}

// ── CTL-02 migration: v9 bodies gain the seeded channel set ──────

/// Serializes `c`'s project as a schema-v9 envelope with every
/// strip-owned channel-role target removed — the shape a pre-#122
/// snapshot has.
fn as_v9_envelope(c: &ProjectController) -> serde_json::Value {
    let mut env = serde_json::json!({
        "schema_version": 9,
        "project_id": c.live().project_id,
        "revision": c.live_revision(),
        "project": serde_json::to_value(c.live()).unwrap(),
    });
    let targets = env["project"]["control"]["targets"]
        .as_object_mut()
        .unwrap();
    let strip_channel = |t: &serde_json::Value| {
        t["owner"]["owner"] == "strip"
            && matches!(t["role"].as_str(), Some("fader" | "mute" | "solo"))
    };
    let doomed: Vec<String> = targets
        .iter()
        .filter(|(_, t)| strip_channel(t))
        .map(|(k, _)| k.clone())
        .collect();
    for k in doomed {
        targets.remove(&k);
    }
    env
}

#[test]
fn v9_snapshot_seeds_channel_params_deterministically() {
    let mut c = controller();
    let track = add_track(&mut c, "t");
    let _strip = add_track_strip(&mut c, track);
    let mut store = InMemoryStore::new();
    store
        .save(
            "p",
            c.live_revision(),
            &serde_json::to_vec(&as_v9_envelope(&c)).unwrap(),
        )
        .unwrap();

    let mut c2 = controller();
    c2.load(&store).unwrap();
    // Every strip — master included — carries the seeded set with
    // canonical bases; the master strip's are the well-known IDs.
    let master = master_strip(c2.live());
    for role in SEED_ROLES {
        let id = strip_channel_target_id(c2.live(), master, role).unwrap();
        assert!(!is_issuable_raw(id.raw()), "master {role:?} well-known");
    }
    for strip in c2.live().routing.strips.keys() {
        for role in SEED_ROLES {
            assert!(
                strip_channel_target_id(c2.live(), *strip, role).is_some(),
                "strip {strip} lacks seeded {role:?}"
            );
        }
        assert!(strip_channel_target_id(c2.live(), *strip, ParameterRole::Pan).is_none());
    }
    // Issued seeded targets sit above the watermark.
    for t in c2
        .live()
        .control
        .targets
        .values()
        .filter(|t| is_issuable_raw(t.id.raw()))
    {
        assert!(t.id.raw() < c2.live().id_watermark);
    }
    assert!(c2.live().validate_references().is_ok());

    // Determinism: the same v9 body projects identically every load.
    let mut c3 = controller();
    c3.load(&store).unwrap();
    assert_eq!(c2.live(), c3.live());
}
