//! synvelis#117 RCD-03: authored Arm/Monitor/channel-map/lane
//! declarations and punch marks persist through `Project::monitoring`
//! (schema v12) — ops, undo/redo, save/load round trips, v11→v12
//! migration, and track/lane cascade rules.

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

fn add_lane(c: &mut ProjectController, track: TrackId) -> LaneId {
    c.apply(cmd(c, ProjectOp::AddLane { track_id: track }))
        .unwrap();
    *c.live().tracks[&track].lane_ids.last().unwrap()
}

fn decl(lane: Option<LaneId>) -> TrackMonitorDecl {
    TrackMonitorDecl {
        lane,
        channel_map: MonitorChannelMap::identity(),
        arm: ArmState::On,
        monitor: MonitorState::Auto,
    }
}

#[test]
fn fresh_project_has_empty_monitoring() {
    let c = controller();
    assert!(c.live().monitoring.tracks.is_empty());
    assert_eq!(c.live().monitoring.punch, PunchMarks::default());
}

#[test]
fn set_track_monitor_upserts_and_clear_removes() {
    let mut c = controller();
    let track = add_track(&mut c, "vox");
    let lane = add_lane(&mut c, track);

    c.apply(cmd(
        &c,
        ProjectOp::SetTrackMonitor {
            track,
            decl: decl(Some(lane)),
        },
    ))
    .unwrap();
    let stored = &c.live().monitoring.tracks[&track];
    assert_eq!(stored.lane, Some(lane));
    assert_eq!(stored.arm, ArmState::On);
    assert_eq!(stored.monitor, MonitorState::Auto);
    assert_eq!(stored.channel_map.channels(), &[0, 1]);

    // Upsert replaces the whole declaration.
    let mut next = decl(None);
    next.arm = ArmState::Off;
    next.monitor = MonitorState::On;
    next.channel_map = MonitorChannelMap {
        channels: vec![0, 1, 2, 3],
    };
    c.apply(cmd(
        &c,
        ProjectOp::SetTrackMonitor {
            track,
            decl: next.clone(),
        },
    ))
    .unwrap();
    assert_eq!(&c.live().monitoring.tracks[&track], &next);

    c.apply(cmd(&c, ProjectOp::ClearTrackMonitor { track }))
        .unwrap();
    assert!(c.live().monitoring.tracks.is_empty());
}

#[test]
fn set_track_monitor_rejects_invalid_references_atomically() {
    let mut c = controller();
    let track = add_track(&mut c, "vox");
    let other = add_track(&mut c, "drums");
    let foreign_lane = add_lane(&mut c, other);
    let rev = c.live_revision();

    // Unknown track.
    assert!(matches!(
        c.apply(cmd(
            &c,
            ProjectOp::SetTrackMonitor {
                track: TrackId::new(999),
                decl: decl(None),
            },
        )),
        Err(Error::NotFound(_))
    ));
    // Lane belonging to a different track.
    assert!(
        c.apply(cmd(
            &c,
            ProjectOp::SetTrackMonitor {
                track,
                decl: decl(Some(foreign_lane)),
            },
        ))
        .is_err()
    );
    // Empty channel map.
    let mut empty = decl(None);
    empty.channel_map = MonitorChannelMap { channels: vec![] };
    assert!(
        c.apply(cmd(&c, ProjectOp::SetTrackMonitor { track, decl: empty },))
            .is_err()
    );
    // ClearTrackMonitor on an unknown track.
    assert!(matches!(
        c.apply(cmd(
            &c,
            ProjectOp::ClearTrackMonitor {
                track: TrackId::new(999),
            },
        )),
        Err(Error::NotFound(_))
    ));

    // Nothing was applied — revision and state untouched.
    assert_eq!(c.live_revision(), rev);
    assert!(c.live().monitoring.tracks.is_empty());
}

#[test]
fn set_punch_marks_replaces_both_fields_verbatim() {
    let mut c = controller();
    c.apply(cmd(
        &c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(960)),
            punch_out: Some(MusicalTick::new(1920)),
        },
    ))
    .unwrap();
    assert_eq!(
        c.live().monitoring.punch,
        PunchMarks {
            punch_in: Some(MusicalTick::new(960)),
            punch_out: Some(MusicalTick::new(1920)),
        }
    );

    // The op replaces both fields — a one-sided update clears the
    // other side verbatim.
    c.apply(cmd(
        &c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(-480)),
            punch_out: None,
        },
    ))
    .unwrap();
    assert_eq!(
        c.live().monitoring.punch,
        PunchMarks {
            punch_in: Some(MusicalTick::new(-480)),
            punch_out: None,
        }
    );

    // `None`/`None` clears.
    c.apply(cmd(
        &c,
        ProjectOp::SetPunchMarks {
            punch_in: None,
            punch_out: None,
        },
    ))
    .unwrap();
    assert_eq!(c.live().monitoring.punch, PunchMarks::default());
}

#[test]
fn undo_redo_restores_declarations_and_punch() {
    let mut c = controller();
    let track = add_track(&mut c, "vox");
    let lane = add_lane(&mut c, track);

    c.apply(cmd(
        &c,
        ProjectOp::SetTrackMonitor {
            track,
            decl: decl(Some(lane)),
        },
    ))
    .unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(0)),
            punch_out: Some(MusicalTick::new(960)),
        },
    ))
    .unwrap();
    assert!(c.live().monitoring.tracks.contains_key(&track));
    assert_eq!(
        c.live().monitoring.punch.punch_in,
        Some(MusicalTick::new(0))
    );

    // Undo punch, then the declaration — each restores prior state.
    assert!(c.undo().unwrap());
    assert_eq!(c.live().monitoring.punch, PunchMarks::default());
    assert!(c.live().monitoring.tracks.contains_key(&track));
    assert!(c.undo().unwrap());
    assert!(c.live().monitoring.tracks.is_empty());

    // Redo restores the declaration first, then the punch marks.
    assert!(c.redo().unwrap());
    assert!(c.live().monitoring.tracks.contains_key(&track));
    assert_eq!(c.live().monitoring.punch, PunchMarks::default());
    assert!(c.redo().unwrap());
    assert_eq!(
        c.live().monitoring.punch.punch_out,
        Some(MusicalTick::new(960))
    );
}

#[test]
fn save_load_round_trip_preserves_monitoring() {
    let mut c = controller();
    let track = add_track(&mut c, "vox");
    let lane = add_lane(&mut c, track);
    c.apply(cmd(
        &c,
        ProjectOp::SetTrackMonitor {
            track,
            decl: decl(Some(lane)),
        },
    ))
    .unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(480)),
            punch_out: Some(MusicalTick::new(1440)),
        },
    ))
    .unwrap();

    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c.live(), c2.live());
    assert_eq!(c.live_revision(), c2.live_revision());
    assert_eq!(c2.live().monitoring.tracks[&track].lane, Some(lane));
    assert_eq!(
        c2.live().monitoring.punch.punch_in,
        Some(MusicalTick::new(480))
    );
}

#[test]
fn v11_snapshot_migrates_to_v12_with_empty_monitoring() {
    let mut c = controller();
    let track = add_track(&mut c, "vox");
    c.apply(cmd(
        &c,
        ProjectOp::SetTrackMonitor {
            track,
            decl: decl(None),
        },
    ))
    .unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::SetPunchMarks {
            punch_in: Some(MusicalTick::new(1)),
            punch_out: None,
        },
    ))
    .unwrap();

    // Rewrite the saved envelope as a v11 body: schema_version 11 and
    // no `monitoring` member (v11 carried no monitor declarations).
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let head = store.head().unwrap().unwrap();
    let bytes = store.read_snapshot(&head).unwrap();
    let mut env: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    env["schema_version"] = serde_json::json!(11);
    env["project"].as_object_mut().unwrap().remove("monitoring");
    let v11_bytes = serde_json::to_vec(&env).unwrap();

    let mut store11 = InMemoryStore::new();
    let saved_rev = c.saved().unwrap().revision;
    store11.save("p", saved_rev, &v11_bytes).unwrap();

    let mut c2 = controller();
    c2.load(&store11).unwrap();
    // Migration is explicit but identity: v11 had no declarations, so
    // the loaded project carries empty monitoring state and the live
    // revision advances by the one implicit migration step.
    assert!(c2.live().monitoring.tracks.is_empty());
    assert_eq!(c2.live().monitoring.punch, PunchMarks::default());
    assert_eq!(c2.live_revision(), ProjectRevision(saved_rev.0 + 1));
    assert_eq!(c2.saved().unwrap().revision, saved_rev);
    // The rest of the project survived untouched.
    assert!(c2.live().tracks.contains_key(&track));
}

#[test]
fn remove_track_cascades_monitor_decl() {
    let mut c = controller();
    let track = add_track(&mut c, "vox");
    c.apply(cmd(
        &c,
        ProjectOp::SetTrackMonitor {
            track,
            decl: decl(None),
        },
    ))
    .unwrap();
    c.apply(cmd(&c, ProjectOp::RemoveTrack { track_id: track }))
        .unwrap();
    assert!(c.live().monitoring.tracks.is_empty());
    // Undo restores both the track and its declaration.
    assert!(c.undo().unwrap());
    assert!(c.live().monitoring.tracks.contains_key(&track));
}

#[test]
fn remove_lane_rejects_monitor_reference() {
    let mut c = controller();
    let track = add_track(&mut c, "vox");
    let lane = add_lane(&mut c, track);
    c.apply(cmd(
        &c,
        ProjectOp::SetTrackMonitor {
            track,
            decl: decl(Some(lane)),
        },
    ))
    .unwrap();

    // A lane bound by a monitor declaration cannot be removed.
    assert!(
        c.apply(cmd(&c, ProjectOp::RemoveLane { lane_id: lane }))
            .is_err()
    );
    assert!(c.live().lanes.contains_key(&lane));

    // Clearing the declaration unblocks removal.
    c.apply(cmd(&c, ProjectOp::ClearTrackMonitor { track }))
        .unwrap();
    c.apply(cmd(&c, ProjectOp::RemoveLane { lane_id: lane }))
        .unwrap();
    assert!(!c.live().lanes.contains_key(&lane));
}

#[test]
fn validate_references_rejects_dangling_monitor_state() {
    // The load-path oracle mirrors the op validation: a project whose
    // serialized state references a missing track/lane or an empty
    // channel map is rejected on load, not silently degraded.
    let mut c = controller();
    let track = add_track(&mut c, "vox");
    let lane = add_lane(&mut c, track);
    c.apply(cmd(
        &c,
        ProjectOp::SetTrackMonitor {
            track,
            decl: decl(Some(lane)),
        },
    ))
    .unwrap();

    // Corrupt the live project in memory (bypassing ops) and run the
    // same oracle `load` uses.
    let mut broken = c.live().clone();
    broken
        .monitoring
        .tracks
        .insert(TrackId::new(999), decl(None));
    assert!(broken.validate_references().is_err());

    let mut broken = c.live().clone();
    broken.monitoring.tracks.get_mut(&track).unwrap().lane = Some(LaneId::new(999));
    assert!(broken.validate_references().is_err());

    let mut broken = c.live().clone();
    broken
        .monitoring
        .tracks
        .get_mut(&track)
        .unwrap()
        .channel_map = MonitorChannelMap { channels: vec![] };
    assert!(broken.validate_references().is_err());

    // The unmodified project passes.
    assert!(c.live().validate_references().is_ok());
}
