//! AC7/AC8/AC9/AC12: atomic commands, undo/redo, schema enforcement,
//! save/reload, and the verified-source reference port.

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

#[test]
fn stale_revision_and_invalid_target_are_rejected_atomically() {
    let mut c = controller();
    let rev0 = c.live_revision();
    // Stale expected revision (we are at 0, claim 5).
    let stale = Command {
        expected_revision: ProjectRevision(5),
        op: ProjectOp::AddTrack { name: "t".into() },
    };
    assert!(matches!(c.apply(stale), Err(Error::StaleRevision { .. })));
    assert_eq!(c.live_revision(), rev0);
    assert!(c.live().tracks.is_empty());
    // Invalid target: removing a track that does not exist.
    let bad = cmd(
        &c,
        ProjectOp::RemoveTrack {
            track_id: TrackId::new(999),
        },
    );
    assert!(matches!(c.apply(bad), Err(Error::NotFound(_))));
    assert_eq!(c.live_revision(), rev0); // no partial application
    assert!(c.live().tracks.is_empty());
}

#[test]
fn apply_advances_revision_and_is_atomic() {
    let mut c = controller();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "a".into() }))
        .unwrap();
    assert_eq!(c.live_revision(), ProjectRevision(1));
    assert_eq!(c.live().tracks.len(), 1);
    // Invalid op: AddLane on missing track — nothing changes.
    let bad = cmd(
        &c,
        ProjectOp::AddLane {
            track_id: TrackId::new(42),
        },
    );
    assert!(c.apply(bad).is_err());
    assert_eq!(c.live_revision(), ProjectRevision(1));
    assert!(c.live().lanes.is_empty());
}

#[test]
fn undo_redo_issue_new_revisions_and_keep_saved_intact() {
    let mut c = controller();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "a".into() }))
        .unwrap();
    let mut store = InMemoryStore::new();
    let saved = c.save(&mut store).unwrap();
    assert_eq!(saved.revision, ProjectRevision(1));

    c.apply(cmd(&c, ProjectOp::AddTrack { name: "b".into() }))
        .unwrap();
    assert_eq!(c.live().tracks.len(), 2);
    assert!(c.undo().unwrap());
    // Content reverted but revision advanced, saved untouched.
    assert_eq!(c.live().tracks.len(), 1);
    assert_eq!(c.live_revision(), ProjectRevision(3));
    assert_eq!(c.saved().unwrap().revision, ProjectRevision(1));

    assert!(c.redo().unwrap());
    assert_eq!(c.live().tracks.len(), 2);
    assert_eq!(c.live_revision(), ProjectRevision(4));

    // A fresh command clears redo.
    assert!(c.undo().unwrap());
    c.apply(cmd(
        &c,
        ProjectOp::SetProjectName {
            name: "renamed".into(),
        },
    ))
    .unwrap();
    assert!(!c.redo().unwrap()); // stack was cleared
    // Empty undo history drains to false.
    while c.undo().unwrap() {}
}

#[test]
fn save_reload_preserves_identity_ids_and_revision() {
    let mut c = controller();
    c.apply(cmd(
        &c,
        ProjectOp::RegisterAsset {
            digest: "d1".into(),
        },
    ))
    .unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::RegisterSource {
            asset_id: *c.live().assets.keys().next().unwrap(),
            content_digest: "d1".into(),
        },
    ))
    .unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::RegisterClockEpoch {
            description: "clk".into(),
        },
    ))
    .unwrap();
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();

    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c.live(), c2.live());
    assert_eq!(c.live_revision(), c2.live_revision());
    assert_eq!(c2.saved().unwrap().head, c.saved().unwrap().head);
    // All stable IDs identical.
    assert_eq!(
        c.live().sources.keys().next().unwrap(),
        c2.live().sources.keys().next().unwrap()
    );
    assert_eq!(
        c.live().clock_epochs.keys().next().unwrap(),
        c2.live().clock_epochs.keys().next().unwrap()
    );
    // Undo history cleared after load.
    assert!(!c2.undo().unwrap());
}

#[test]
fn unknown_schema_and_corrupt_snapshots_are_rejected() {
    let mut c = controller();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "x".into() }))
        .unwrap();
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();

    // Craft an unsupported schema version through the store bytes.
    let head = store.head().unwrap().unwrap();
    let bytes = store.read_snapshot(&head).unwrap();
    let mut env: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    env["schema_version"] = serde_json::json!(99);
    store
        .save("p", ProjectRevision(9), &serde_json::to_vec(&env).unwrap())
        .unwrap();
    let mut c2 = controller();
    assert!(matches!(
        c2.load(&store),
        Err(Error::UnsupportedSchema { found: 99 })
    ));
    // Live state untouched by the failed load.
    assert_eq!(c2.live_revision(), ProjectRevision(0));

    // Unknown top-level field is rejected (deny_unknown_fields).
    let mut env2: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    env2["mystery"] = serde_json::json!(1);
    store
        .save(
            "p",
            ProjectRevision(10),
            &serde_json::to_vec(&env2).unwrap(),
        )
        .unwrap();
    assert!(c2.load(&store).is_err());
    assert_eq!(c2.live_revision(), ProjectRevision(0));

    // Truncated JSON also fails without touching live state.
    store
        .save("p", ProjectRevision(11), &bytes[..bytes.len() / 2])
        .unwrap();
    assert!(c2.load(&store).is_err());
    assert_eq!(c2.live_revision(), ProjectRevision(0));
}

#[test]
fn source_state_is_forward_only_and_carries_no_media() {
    let mut c = controller();
    c.apply(cmd(&c, ProjectOp::RegisterAsset { digest: "d".into() }))
        .unwrap();
    let asset_id = *c.live().assets.keys().next().unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::RegisterSource {
            asset_id,
            content_digest: "d".into(),
        },
    ))
    .unwrap();
    let source_id = *c.live().sources.keys().next().unwrap();
    assert_eq!(c.live().sources[&source_id].state, SourceState::Imported);

    // Skip ahead or backward: rejected.
    assert!(
        c.apply(cmd(
            &c,
            ProjectOp::SetSourceState {
                source_id,
                state: SourceState::Saved,
            }
        ))
        .is_err()
    );
    c.apply(cmd(
        &c,
        ProjectOp::SetSourceState {
            source_id,
            state: SourceState::Verified,
        },
    ))
    .unwrap();
    // States are distinct: Imported != Verified != Committed != Saved.
    assert_ne!(SourceState::Imported, SourceState::Verified);
    assert_ne!(SourceState::Verified, SourceState::Committed);
    assert_ne!(SourceState::Committed, SourceState::Saved);
    // Regression rejected.
    assert!(
        c.apply(cmd(
            &c,
            ProjectOp::SetSourceState {
                source_id,
                state: SourceState::Imported,
            }
        ))
        .is_err()
    );

    // The serialized project contains no media bytes — only the
    // reference fields (ids, digest string, state tag).
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let bytes = store
        .read_snapshot(&store.head().unwrap().unwrap())
        .unwrap();
    let json = String::from_utf8(bytes).unwrap();
    assert!(json.contains("verified"));
    assert!(json.contains("source-"));
    // No byte-array payload is present anywhere in the schema.
    assert!(!json.contains("\"data\""));
}

// ---------------------------------------------------------------------
// MD-B: load-time revalidation (issue #50 scope 2)
// ---------------------------------------------------------------------

/// Builds a valid snapshot with one clip on one lane, then lets the
/// caller mutate the serialized project before reloading.
fn snapshot_with_clip(store: &mut InMemoryStore) -> serde_json::Value {
    let mut c = controller();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "t".into() }))
        .unwrap();
    let track = *c.live().tracks.keys().next().unwrap();
    c.apply(cmd(&c, ProjectOp::AddLane { track_id: track }))
        .unwrap();
    let lane = *c.live().lanes.keys().next().unwrap();
    c.apply(cmd(&c, ProjectOp::RegisterAsset { digest: "d".into() }))
        .unwrap();
    let asset = *c.live().assets.keys().next().unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "d".into(),
        },
    ))
    .unwrap();
    let source = *c.live().sources.keys().next().unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: ClipPlacement {
                anchor: TimeAnchor::Absolute(MediaTimeValue::new(0, 1, 48_000).unwrap()),
                extent: ClipExtent::Absolute(MediaDuration::new(48_000, 1, 48_000).unwrap()),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(48_000).unwrap(),
                )
                .unwrap(),
                mapping: PlaybackMapping::Natural,
            },
        },
    ))
    .unwrap();
    c.save(store).unwrap();
    let head = store.head().unwrap().unwrap();
    serde_json::from_slice(&store.read_snapshot(&head).unwrap()).unwrap()
}

/// A snapshot whose persisted tempo map fails to compile is rejected
/// at load — the live project is untouched.
#[test]
fn load_rejects_malformed_tempo_map() {
    let mut store = InMemoryStore::new();
    let mut env = snapshot_with_clip(&mut store);
    // An empty tempo event list cannot compile — this is the honest
    // detection the load path owes a broken snapshot.
    env["project"]["tempo_map"]["tempo_events"] = serde_json::json!([]);
    store
        .save("p", ProjectRevision(20), &serde_json::to_vec(&env).unwrap())
        .unwrap();

    let mut c2 = controller();
    assert!(c2.load(&store).is_err(), "malformed tempo map must fail");
    assert_eq!(c2.live_revision(), ProjectRevision(0));
    assert!(c2.live().clips.is_empty());
}

/// A clip whose persisted contract is unsatisfiable — a source window
/// shorter than the absolute extent demands — is rejected at load even
/// though every individual field deserializes.
#[test]
fn load_rejects_invalid_clip_contract() {
    let mut store = InMemoryStore::new();
    let mut env = snapshot_with_clip(&mut store);
    // Shrink the source window to 100 frames under a 48_000-frame
    // absolute extent: `Natural` playback then consumes more than the
    // window — the contract fails at revalidation.
    let clips = env["project"]["clips"].as_object_mut().unwrap();
    let clip = clips.values_mut().next().unwrap();
    clip["playback"]["source_region"]["end"] = serde_json::json!(100);
    store
        .save("p", ProjectRevision(21), &serde_json::to_vec(&env).unwrap())
        .unwrap();

    let mut c2 = controller();
    assert!(c2.load(&store).is_err(), "broken clip contract must fail");
    assert_eq!(c2.live_revision(), ProjectRevision(0));
    assert!(c2.live().clips.is_empty());
}

/// The same revalidation leaves a conforming snapshot loadable — the
/// new checks tighten the oracle, not the contract.
#[test]
fn load_still_accepts_valid_snapshot() {
    let mut store = InMemoryStore::new();
    let env = snapshot_with_clip(&mut store);
    store
        .save("p", ProjectRevision(22), &serde_json::to_vec(&env).unwrap())
        .unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c2.live().clips.len(), 1);
    // The store-level revision names the snapshot file; the envelope's
    // own `revision` field (5 ops applied above) is what loads.
    assert_eq!(c2.live_revision(), ProjectRevision(5));
}

#[test]
fn directory_bundle_save_publishes_head_last() {
    let dir = std::env::temp_dir().join(format!("synvelis-f0-{}-{}", std::process::id(), "bundle"));
    let _ = std::fs::remove_dir_all(&dir);
    let mut store = DirectoryBundleStore::create(&dir).unwrap();
    let mut c = controller();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "a".into() }))
        .unwrap();
    let saved = c.save(&mut store).unwrap();
    assert!(dir.join("snapshots").join(&saved.head).exists());
    assert_eq!(
        std::fs::read_to_string(dir.join("HEAD")).unwrap(),
        saved.head
    );
    // Two saves keep both immutable snapshots.
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "b".into() }))
        .unwrap();
    let saved2 = c.save(&mut store).unwrap();
    assert!(dir.join("snapshots").join(&saved.head).exists());
    assert!(dir.join("snapshots").join(&saved2.head).exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// Issue #64: `EvalPlan::is_stale` detects control-model mutations the
/// plan froze at compile — and stays fresh over placement changes the
/// snapshot contract resolves live.
#[test]
#[allow(clippy::too_many_lines)] // sequential acceptance scenario
fn eval_plan_is_stale_tracks_frozen_inputs() {
    let mut c = controller();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "t".into() }))
        .unwrap();
    let track = *c.live().tracks.keys().next().unwrap();
    c.apply(cmd(&c, ProjectOp::AddLane { track_id: track }))
        .unwrap();
    let lane_id = *c.live().lanes.keys().next().unwrap();
    c.apply(cmd(&c, ProjectOp::RegisterAsset { digest: "d".into() }))
        .unwrap();
    let asset = *c.live().assets.keys().next().unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: "d".into(),
        },
    ))
    .unwrap();
    let source = *c.live().sources.keys().next().unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::AddClip {
            lane_id,
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
    let clip = *c.live().clips.keys().next().unwrap();
    for (owner, base) in [
        (ControlScope::Clip(clip), 0.5),
        (ControlScope::Track(track), 0.4),
    ] {
        c.apply(cmd(
            &c,
            ProjectOp::AddParameterTarget {
                name: "p".into(),
                owner,
                kind: ValueKind::Continuous,
                mapping: ParamMapping::Linear,
                capability: None,
                base,
            },
        ))
        .unwrap();
    }
    let clip_target = c
        .live()
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlScope::Clip(clip))
        .unwrap()
        .id;
    let target = c
        .live()
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlScope::Track(track))
        .unwrap()
        .id;
    c.apply(cmd(&c, ProjectOp::AddAutomationLane { target }))
        .unwrap();
    let auto_lane = *c.live().control.lanes.keys().next().unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::WriteAutomationRegion {
            lane: auto_lane,
            region: AutomationRegion {
                range: BeatRange::try_new(
                    BeatPoint::new(0, 1).unwrap(),
                    BeatPoint::new(8, 1).unwrap(),
                )
                .unwrap(),
                interpolation: Interpolation::Step,
                points: vec![AutomationPoint {
                    pos: BeatPoint::new(0, 1).unwrap(),
                    value: 0.5,
                }],
            },
            policy: ConflictPolicy::Reject,
        },
    ))
    .unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::AddModulationSource {
            owner: ControlScope::Track(track),
            kind: SourceKind::Macro { value: 0.25 },
        },
    ))
    .unwrap();
    let src = *c.live().control.sources.keys().next().unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::AddModulationAssignment {
            source: src,
            target,
            depth: 0.1,
            depth_source: Some(clip_target),
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    ))
    .unwrap();
    let assignment = *c.live().control.assignments.keys().next().unwrap();

    let plan = ControlGraph::compile(c.live()).unwrap();
    assert!(!plan.is_stale(c.live()));

    // Placement moves are resolved live — the frozen inputs are
    // untouched, so the plan stays correct without a recompile.
    c.apply(cmd(
        &c,
        ProjectOp::MoveClip {
            clip_id: clip,
            anchor: TimeAnchor::Musical(MusicalTick::new(4 * 960)),
        },
    ))
    .unwrap();
    assert!(!plan.is_stale(c.live()));
    // The same plan still resolves the moved clip's scope against the
    // new anchor — placement is live, not frozen.
    let mut scratch = plan.scratch();
    assert!(
        plan.eval_rt(
            c.live(),
            clip_target,
            BeatPoint::new(5, 1).unwrap(),
            None,
            &mut scratch
        )
        .is_ok()
    );

    // Frozen inputs, one at a time (recompiled baseline each step).
    c.apply(cmd(&c, ProjectOp::SetParameterBase { target, value: 0.9 }))
        .unwrap();
    assert!(plan.is_stale(c.live()), "base is frozen input");

    let plan = ControlGraph::compile(c.live()).unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::WriteAutomationRegion {
            lane: auto_lane,
            region: AutomationRegion {
                range: BeatRange::try_new(
                    BeatPoint::new(8, 1).unwrap(),
                    BeatPoint::new(12, 1).unwrap(),
                )
                .unwrap(),
                interpolation: Interpolation::Step,
                points: vec![AutomationPoint {
                    pos: BeatPoint::new(8, 1).unwrap(),
                    value: 0.1,
                }],
            },
            policy: ConflictPolicy::Reject,
        },
    ))
    .unwrap();
    assert!(plan.is_stale(c.live()), "merged series is frozen input");

    let plan = ControlGraph::compile(c.live()).unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::SetModulationAssignment {
            id: assignment,
            depth: None,
            enabled: Some(false),
        },
    ))
    .unwrap();
    assert!(plan.is_stale(c.live()), "assignment enable is frozen input");

    let plan = ControlGraph::compile(c.live()).unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::AddParameterTarget {
            name: "q".into(),
            owner: ControlScope::Project,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.0,
        },
    ))
    .unwrap();
    assert!(plan.is_stale(c.live()), "target membership is frozen input");

    // A model that no longer compiles is definitionally stale. A
    // genuine depth cycle needs the reverse edge — re-enabling the
    // disabled assignment plus the inserted counter-assignment forms a
    // `target ↔ clip_target` cycle (the op oracle does not check
    // acyclicity, so this state is reachable; here it is constructed
    // by direct mutation to keep the fixture compact).
    let plan = ControlGraph::compile(c.live()).unwrap();
    let mut bad = c.live().clone();
    bad.control
        .assignments
        .get_mut(&assignment)
        .unwrap()
        .enabled = true;
    bad.control.assignments.insert(
        ModulationAssignmentId::new(99),
        ModulationAssignment {
            id: ModulationAssignmentId::new(99),
            source: src,
            target: clip_target,
            depth: 0.1,
            depth_source: Some(target),
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );
    assert!(
        ControlGraph::compile(&bad).is_err(),
        "the fixture must fail compilation to exercise the Err arm"
    );
    assert!(
        plan.is_stale(&bad),
        "a rejected recompile means the plan is out of date"
    );
}
