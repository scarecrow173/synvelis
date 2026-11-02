//! AC10: PPQN context, `TempoMapRef`, and tick↔media mapping.

use synvelis_project::*;

#[test]
fn project_defaults_to_ppqn_960_and_revision_zero() {
    let p = Project::new(ProjectId::new(0), "p");
    assert_eq!(p.ppqn, ProjectPpqn(960));
    assert_eq!(p.tempo_map.revision, 0);
    assert_eq!(p.tempo_map.ppqn, 960);
}

#[test]
fn tempo_map_mutation_bumps_revision_and_refs_point_at_it() {
    let mut c = ProjectController::new(
        Project::new(ProjectId::new(0), "p"),
        Box::new(CounterIdGenerator::new(1)),
    );
    c.apply(Command {
        expected_revision: c.live_revision(),
        op: ProjectOp::AddTempoEvent {
            tick: 7_680,
            bpm_milli: 60_000,
        },
    })
    .unwrap();
    assert_eq!(c.live().tempo_map.revision, 1);
    let before = TempoMapRef { revision: 0 };
    let after = TempoMapRef { revision: 1 };
    assert_ne!(before, after);
    // A duplicate tick is rejected atomically.
    let dup = Command {
        expected_revision: c.live_revision(),
        op: ProjectOp::AddTempoEvent {
            tick: 7_680,
            bpm_milli: 90_000,
        },
    };
    assert!(c.apply(dup).is_err());
    assert_eq!(c.live().tempo_map.revision, 1);
}

#[test]
fn tick_media_mapping_matches_design_example() {
    // 120 BPM, 4/4, PPQN 960, 48 kHz: 4 bars = 384,000 frames ↔ 15,360 ticks.
    let data = TempoMapData::default();
    let mapping = TickTimeMapping::new(&data).unwrap();
    assert_eq!(mapping.ppqn(), 960);

    let tick = MusicalTick::new(15_360);
    let media = mapping.tick_to_media(tick, 1, 48_000).unwrap();
    assert_eq!(media.value(), 384_000);

    let back = mapping.media_to_tick(media).unwrap();
    assert_eq!(back.value(), 15_360);
}

#[test]
fn media_ticks_persist_and_reload_through_snapshot() {
    let mut c = ProjectController::new(
        Project::new(ProjectId::new(0), "p"),
        Box::new(CounterIdGenerator::new(1)),
    );
    c.apply(Command {
        expected_revision: c.live_revision(),
        op: ProjectOp::AddTempoEvent {
            tick: 960,
            bpm_milli: 240_000,
        },
    })
    .unwrap();
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let mut c2 = ProjectController::new(
        Project::new(ProjectId::new(0), "x"),
        Box::new(CounterIdGenerator::new(1)),
    );
    c2.load(&store).unwrap();
    assert_eq!(c.live().tempo_map, c2.live().tempo_map);
}
