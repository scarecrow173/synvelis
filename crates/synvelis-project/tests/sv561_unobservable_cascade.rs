//! synvelis#561 domain pin — the cascade firing conditions the
//! gateway `op_facts` mirrors when it reports `unsupported`.
//!
//! The gateway cannot name `ModulationSource`/`ModulationAssignment`/
//! `SmfDocument` as `ResourceKind`s, so wire prepare fails closed on
//! ops whose apply cascade deletes them. These tests pin the domain
//! side of that contract — the exact conditions under which the
//! unobservable deletes actually fire — so the gateway mirror cannot
//! silently drift from `ops.rs`:
//!
//! - `release_send_level_target` (via `SetSendGainTarget` rebind /
//!   clear) deletes lanes, the sealed target, and every assignment
//!   targeting it or using it as `depth_source`; send-owned sources
//!   survive.
//! - `remove_strip_owned` → `remove_control_owned(ControlOwner::Strip)`
//!   (via `RemoveTrack`) deletes strip-owned targets, sources,
//!   lanes, and assignments touching the closure.
//! - `RemoveMidiSource` drops the source's `smf_documents` entry
//!   unconditionally.

use synvelis_project::routing::{PortDirection, PortOwner, PortRole, SendTap};
use synvelis_project::{
    AutomationTimeScope, Command, ControlOwner, CounterIdGenerator, MidiTick, ParamMapping,
    ParameterRole, Polarity, Project, ProjectController, ProjectId, ProjectOp, SmfDocument,
    SmfEvent, SmfEventKind, SmfFormat, SmfTrack, SourceKind, TrackMonitorDecl, Transform,
    ValueKind,
};

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "p"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply(c: &mut ProjectController, op: ProjectOp) {
    let rev = c.live_revision();
    c.apply(Command {
        expected_revision: rev,
        op,
    })
    .unwrap();
}

fn port_of(
    project: &Project,
    owner: PortOwner,
    direction: PortDirection,
    role: PortRole,
) -> synvelis_project::SemanticPortId {
    project
        .routing
        .ports
        .values()
        .find(|p| p.owner == owner && p.direction == direction && p.role == role)
        .expect("port exists")
        .id
}

/// Track + strip + one send with `gain_target: None` — the shared
/// prefix of the sealed-send fixture. Returns the send id.
fn strip_send_prefix(c: &mut ProjectController) -> synvelis_project::SendId {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    let (source, destination) = {
        let p = c.live();
        let strip = p.routing.strip_of_track(track).unwrap().id;
        let master_strip = p.routing.buses[&p.routing.master].strip;
        (
            port_of(
                p,
                PortOwner::Strip(strip),
                PortDirection::Output,
                PortRole::Main,
            ),
            port_of(
                p,
                PortOwner::Strip(master_strip),
                PortDirection::Input,
                PortRole::Main,
            ),
        )
    };
    apply(
        c,
        ProjectOp::AddSend {
            source,
            destination,
            tap: SendTap::PostFader,
            gain_target: None,
        },
    );
    *c.live().routing.sends.keys().next().unwrap()
}

/// Track + strip + one send whose `SendLevel` target is sealed, plus
/// a `Macro`→sealed-target assignment. With `depth_source`, the
/// assignment instead targets a second free target while naming the
/// sealed one as `depth_source` — the release predicate's second
/// leg. Returns the controller with the send and sealed target ids
/// recorded for assertions.
fn sealed_send_fixture(
    depth_source: bool,
) -> (
    ProjectController,
    synvelis_project::SendId,
    synvelis_project::ParameterTargetId,
    synvelis_project::ModulationSourceId,
) {
    let mut c = controller();
    let send = strip_send_prefix(&mut c);
    apply(
        &mut c,
        ProjectOp::AddParameterTarget {
            name: "send-lvl".into(),
            owner: ControlOwner::Send(send),
            role: ParameterRole::SendLevel,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    let sealed = c
        .live()
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlOwner::Send(send) && t.role == ParameterRole::SendLevel)
        .unwrap()
        .id;
    apply(
        &mut c,
        ProjectOp::SetSendGainTarget {
            send_id: send,
            gain_target: Some(sealed),
        },
    );
    apply(
        &mut c,
        ProjectOp::AddModulationSource {
            owner: ControlOwner::Project,
            kind: SourceKind::Macro { value: 0.5 },
        },
    );
    let mod_source = *c.live().control.sources.keys().next().unwrap();
    let mut assignment_target = sealed;
    let mut assignment_depth_source = None;
    if depth_source {
        apply(
            &mut c,
            ProjectOp::AddParameterTarget {
                name: "other".into(),
                owner: ControlOwner::Project,
                role: ParameterRole::Generic,
                time_scope: AutomationTimeScope::ProjectMusical,
                kind: ValueKind::Continuous,
                mapping: ParamMapping::Linear,
                capability: None,
                base: 0.5,
            },
        );
        assignment_target = c
            .live()
            .control
            .targets
            .values()
            .find(|t| t.name == "other")
            .unwrap()
            .id;
        assignment_depth_source = Some(sealed);
    }
    apply(
        &mut c,
        ProjectOp::AddModulationAssignment {
            source: mod_source,
            target: assignment_target,
            depth: 0.5,
            depth_source: assignment_depth_source,
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );
    (c, send, sealed, mod_source)
}

#[test]
fn set_gain_target_clear_releases_seal_and_drops_assignment() {
    // `release_send_level_target` fires on clear: the sealed target,
    // its lanes and the touching assignment die; the independent
    // modulation source survives (it may feed other targets).
    let (mut c, send, sealed, mod_source) = sealed_send_fixture(false);
    assert_eq!(c.live().control.assignments.len(), 1);

    apply(
        &mut c,
        ProjectOp::SetSendGainTarget {
            send_id: send,
            gain_target: None,
        },
    );

    let p = c.live();
    assert!(p.routing.sends.get(&send).unwrap().gain_target.is_none());
    assert!(!p.control.targets.contains_key(&sealed));
    assert!(p.control.assignments.is_empty());
    assert!(p.control.sources.contains_key(&mod_source));
}

#[test]
fn remove_track_drops_strip_owned_modulation() {
    // `RemoveTrack` → `remove_strip_owned` → `remove_control_owned
    // (Strip)`: the strip-owned `Macro` source and the seeded channel
    // targets die with the strip.
    let mut c = controller();
    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(&mut c, ProjectOp::AddTrackStrip { track_id: track });
    let conn = *c.live().routing.connections.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::RemoveConnection {
            connection_id: conn,
        },
    );
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    apply(
        &mut c,
        ProjectOp::AddModulationSource {
            owner: ControlOwner::Strip(strip),
            kind: SourceKind::Macro { value: 0.5 },
        },
    );
    assert_eq!(c.live().control.sources.len(), 1);

    apply(&mut c, ProjectOp::RemoveTrack { track_id: track });

    let p = c.live();
    assert!(!p.tracks.contains_key(&track));
    assert!(p.routing.strip_of_track(track).is_none());
    assert!(p.control.sources.is_empty());
    assert!(
        p.control
            .targets
            .values()
            .all(|t| t.owner != ControlOwner::Strip(strip))
    );
}

#[test]
fn remove_midi_source_drops_smf_document() {
    // `RemoveMidiSource` drops the `smf_documents` provenance entry
    // together with the source — unconditionally, even though the
    // document has no `ResourceKind`.
    let mut c = controller();
    apply(
        &mut c,
        ProjectOp::ImportMidiSmf {
            document: SmfDocument {
                format: SmfFormat::Format0,
                division: 480,
                tracks: vec![SmfTrack {
                    events: vec![
                        SmfEvent {
                            tick: MidiTick::new(0).unwrap(),
                            kind: SmfEventKind::NoteOn {
                                channel: 0,
                                key: 60,
                                velocity: 100,
                            },
                        },
                        SmfEvent {
                            tick: MidiTick::new(240).unwrap(),
                            kind: SmfEventKind::NoteOff {
                                channel: 0,
                                key: 60,
                                velocity: 0,
                            },
                        },
                        SmfEvent {
                            tick: MidiTick::new(480).unwrap(),
                            kind: SmfEventKind::EndOfTrack,
                        },
                    ],
                }],
            },
        },
    );
    let source = *c.live().smf_documents.keys().next().unwrap();
    assert!(c.live().smf_documents.contains_key(&source));

    apply(&mut c, ProjectOp::RemoveMidiSource { source_id: source });

    let p = c.live();
    assert!(!p.midi_sources.contains_key(&source));
    assert!(!p.smf_documents.contains_key(&source));
}

#[test]
fn set_gain_target_same_target_does_not_release() {
    // Re-setting the same target is not a release: the assignment
    // touching the still-sealed target survives (the mirror's
    // `Some(old) != *gain_target` guard).
    let (mut c, send, sealed, _) = sealed_send_fixture(false);

    apply(
        &mut c,
        ProjectOp::SetSendGainTarget {
            send_id: send,
            gain_target: Some(sealed),
        },
    );

    let p = c.live();
    assert_eq!(p.control.assignments.len(), 1);
    assert!(p.control.targets.contains_key(&sealed));
    assert_eq!(
        p.routing.sends.get(&send).unwrap().gain_target,
        Some(sealed)
    );
}

#[test]
fn set_gain_target_clear_releases_assignment_via_depth_source() {
    // The release predicate's second leg: an assignment whose
    // `depth_source` names the sealed target — without targeting it —
    // dies in the same cascade.
    let (mut c, send, sealed, mod_source) = sealed_send_fixture(true);
    assert_eq!(c.live().control.assignments.len(), 1);

    apply(
        &mut c,
        ProjectOp::SetSendGainTarget {
            send_id: send,
            gain_target: None,
        },
    );

    let p = c.live();
    assert!(p.control.assignments.is_empty());
    assert!(!p.control.targets.contains_key(&sealed));
    assert!(p.control.sources.contains_key(&mod_source));
}

#[test]
fn remove_send_drops_assignment_on_owned_target() {
    // `RemoveSend` → `remove_control_owned(Send)`: with no
    // send-owned sources, an assignment touching a send-owned target
    // still dies with the send — the closure's `target` leg.
    let (mut c, send, sealed, mod_source) = sealed_send_fixture(false);
    assert_eq!(c.live().control.assignments.len(), 1);

    apply(&mut c, ProjectOp::RemoveSend { send_id: send });

    let p = c.live();
    assert!(!p.routing.sends.contains_key(&send));
    assert!(!p.control.targets.contains_key(&sealed));
    assert!(p.control.assignments.is_empty());
    assert!(p.control.sources.contains_key(&mod_source));
}

#[test]
fn remove_track_drops_monitor_decl() {
    // `RemoveTrack` drops the track's `monitoring.tracks`
    // declaration unconditionally — authored state with no
    // `ResourceKind`, deleted even when the track owns no strip.
    let mut c = controller();
    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::SetTrackMonitor {
            track,
            decl: TrackMonitorDecl::default(),
        },
    );
    assert!(c.live().monitoring.tracks.contains_key(&track));

    apply(&mut c, ProjectOp::RemoveTrack { track_id: track });

    assert!(!c.live().monitoring.tracks.contains_key(&track));
}
