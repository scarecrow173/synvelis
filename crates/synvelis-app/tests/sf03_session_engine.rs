//! SF-03 slice 5 acceptance: the session→routing engine connection.
//!
//! `ProjectSession` notifies its [`EnginePort`] on every committed
//! project revision; [`SessionRoutingEngine`] turns each notification
//! into `RoutingDeps` + `RoutingCompileRequest` and drives
//! `RoutingEngine::prepare` → `publish`. These tests pin the session
//! path of the slice-1–4 contract:
//!
//! - publish → `run` → `Active` receipt + last-good generation,
//! - a failed build keeps last-good and the old plan keeps running,
//! - a semantic no-op revision is `Equivalent` (no rebuild),
//! - stale revisions are rejected before reaching the engine, and a
//!   controller-bypassed commit surfaces as reconcile drift.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use synvelis_app::{SessionEngineConfig, SessionRoutingEngine};
use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::write_fixture;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_project::{
    ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, CounterIdGenerator, MediaDuration,
    MediaTimeValue, PlaybackMapping, Project, ProjectController, ProjectId, ProjectOp,
    ProjectRevision, RoutingActivationState, RoutingFault, RoutingReconcileMismatch, SourceFrame,
    SourceId, SourceRange, TimeAnchor,
};
use synvelis_workspace::{EnginePort, ProjectSession};

use ruviform_media_reader::PrefetchConfig;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sf03s5-{}-{}-{}",
        name,
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0x106), "sf03s5"),
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

fn media_placement(start_frame: u64, end_frame: u64, rate: u32) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Absolute(
            MediaTimeValue::new(i64::try_from(start_frame).unwrap(), 1, rate).unwrap(),
        ),
        extent: ClipExtent::Absolute(
            MediaDuration::new(i64::try_from(end_frame - start_frame).unwrap(), 1, rate).unwrap(),
        ),
    }
}

fn natural_playback(offset: i64, frames: i64) -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::new(offset).unwrap(),
            SourceFrame::new(offset + frames).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

fn engine_config() -> synvelis_player::lowering::EngineConfig {
    synvelis_player::lowering::EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: 256,
    }
}

fn prefetch() -> PrefetchConfig {
    PrefetchConfig {
        block_frames: 256,
        block_count: 8,
        channels: 2,
        sample_rate: 48_000,
    }
}

fn session_config(media_root: PathBuf) -> SessionEngineConfig {
    SessionEngineConfig {
        media_root,
        engine: engine_config(),
        prefetch: prefetch(),
        budget: synvelis_project::BuildBudget {
            max_nodes: 4_096,
            max_connections: 16_384,
            max_frames: 512,
        },
        mode: synvelis_project::BuildMode::Realtime,
        runtime_instance: synvelis_project::RuntimeInstanceId(1),
        device_clock_epoch: synvelis_project::RuntimeClockEpochRef(1),
        engine_config_generation: 1,
    }
}

fn import_stereo(
    c: &mut ProjectController,
    root: &Path,
    name: &str,
    frames: usize,
) -> synvelis_player::ImportOutcome {
    let wav = root.join(name);
    write_fixture(&wav, 2, 48_000, frames).unwrap();
    import_local(
        c,
        &ImportRequest {
            path: wav,
            staging_root: root.join("staging"),
            media_root: root.join("media"),
            resume: None,
        },
    )
    .unwrap()
}

fn add_track_strip(c: &mut ProjectController) -> synvelis_project::LaneId {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    apply(c, ProjectOp::AddLane { track_id: track });
    *c.live().tracks[&track].lane_ids.last().unwrap()
}

fn add_clip(
    c: &mut ProjectController,
    lane: synvelis_project::LaneId,
    source: SourceId,
    start: u64,
    end: u64,
    offset: i64,
) -> ClipId {
    apply(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: media_placement(start, end, 48_000),
            playback: natural_playback(offset, i64::try_from(end - start).unwrap()),
        },
    );
    *c.live().clips.keys().next_back().unwrap()
}

fn strip_of_track(
    project: &Project,
    track: synvelis_project::TrackId,
) -> synvelis_project::StripId {
    project.routing.strip_of_track(track).map(|s| s.id).unwrap()
}

/// A session whose project already holds one imported stereo source,
/// one track strip and one clip — plus a `SessionRoutingEngine`
/// attached through the real `EnginePort` path. Setup mutations go
/// through the controller directly (pre-session), so the engine's
/// first observation is the first `session.apply`.
fn session_with_engine(root: &Path) -> (ProjectSession, SessionRoutingEngine) {
    let mut c = controller();
    let out = import_stereo(&mut c, root, "a.wav", 4_800);
    let lane = add_track_strip(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 480, 0);

    let mut session = ProjectSession::new(c);
    let engine = SessionRoutingEngine::new(session_config(root.join("media")));
    session.set_engine(Box::new(engine.clone()));
    (session, engine)
}

fn receipt_state(engine: &SessionRoutingEngine) -> RoutingActivationState {
    engine.receipt().expect("activation receipt").state.clone()
}

/// `session.apply` → prepare+publish → `run` → `Active` receipt with
/// last-good recorded — the publish/activation distinction is real:
/// `Published` precedes RT observation, never conflated.
#[test]
fn session_apply_publishes_and_activates() {
    let root = scratch("activate");
    let (mut session, engine) = session_with_engine(&root);

    session
        .apply(
            ProjectOp::SetProjectName {
                name: "renamed".into(),
            },
            session.revision(),
        )
        .unwrap();

    // First notification: full build → publish (non-RT evidence).
    assert_eq!(engine.graph_builds(), 1);
    assert_eq!(engine.last_built_revision(), session.revision());
    assert!(engine.last_prepare_error().is_none());
    let receipt = engine.receipt().expect("receipt after first build");
    let published = match receipt.state {
        RoutingActivationState::Published { generation, .. } => generation,
        ref other => panic!("expected Published, got {other:?}"),
    };
    assert_eq!(receipt.published_generation, Some(published));
    // Publish is non-RT evidence: the observed generation is still the
    // pre-activation sentinel, never the just-published one.
    assert_ne!(receipt.observed_active_generation, Some(published));

    // RT observation → Active + last-good.
    engine.run(1, PumpPolicy::Every(1)).unwrap();
    let receipt = engine.receipt().unwrap();
    assert!(matches!(
        receipt.state,
        RoutingActivationState::Active { generation, .. } if generation == published
    ));
    assert_eq!(receipt.last_good_generation, Some(published));
    assert_eq!(receipt.observed_active_generation, Some(published));
}

/// A failing build (unresolvable processor binding) commits the
/// project change but fails prepare — the receipt records `Failed`
/// with `last_good_generation`, and the previous plan keeps running.
#[test]
fn failed_build_keeps_last_good() {
    let root = scratch("lastgood");
    let (mut session, engine) = session_with_engine(&root);

    session
        .apply(
            ProjectOp::SetProjectName { name: "v1".into() },
            session.revision(),
        )
        .unwrap();
    engine.run(1, PumpPolicy::Every(1)).unwrap();
    let good = match receipt_state(&engine) {
        RoutingActivationState::Active { generation, .. } => generation,
        ref other => panic!("expected Active, got {other:?}"),
    };

    // Semantic change that cannot build: unresolved chain binding.
    let track = *session.project().tracks.keys().next_back().unwrap();
    let strip = strip_of_track(session.project(), track);
    session
        .apply(
            ProjectOp::InsertProcessor {
                strip_id: strip,
                index: 0,
                name: "gain".into(),
                binding: synvelis_project::ProcessorBinding::Unresolved {
                    descriptor: "test.gain".into(),
                },
                sidechain_input: false,
            },
            session.revision(),
        )
        .unwrap();

    // The project committed (revision advanced) but the build failed —
    // evidence lives on the engine receipt, not the session.
    assert_eq!(engine.graph_builds(), 2);
    let receipt = engine.receipt().unwrap();
    assert!(matches!(
        receipt.state,
        RoutingActivationState::Failed {
            fault: RoutingFault::PluginUnavailable,
            last_good_generation: Some(g),
        } if g == good
    ));
    assert_eq!(receipt.last_good_generation, Some(good));

    // The last-good plan is still the live one — pumping continues
    // the old generation, bit-identical evidence aside.
    engine.run(1, PumpPolicy::Every(1)).unwrap();
    assert_eq!(engine.rt_status().active_generation, good);
}

/// A semantic no-op revision (project rename) reaches the engine as
/// a notification but resolves to `RoutingPrepare::Equivalent`: no
/// generation is consumed, the receipt is untouched.
#[test]
fn semantic_noop_is_equivalent() {
    let root = scratch("equiv");
    let (mut session, engine) = session_with_engine(&root);

    session
        .apply(
            ProjectOp::SetProjectName { name: "v1".into() },
            session.revision(),
        )
        .unwrap();
    engine.run(1, PumpPolicy::Every(1)).unwrap();
    let active = match receipt_state(&engine) {
        RoutingActivationState::Active { generation, .. } => generation,
        ref other => panic!("expected Active, got {other:?}"),
    };

    session
        .apply(
            ProjectOp::SetProjectName { name: "v2".into() },
            session.revision(),
        )
        .unwrap();

    // Notification delivered (graph_builds advanced) but no rebuild:
    // the receipt still reports the same active generation.
    assert_eq!(engine.graph_builds(), 2);
    let receipt = engine.receipt().unwrap();
    assert!(matches!(
        receipt.state,
        RoutingActivationState::Active { generation, .. } if generation == active
    ));
    assert_eq!(receipt.published_generation, Some(active));
    assert_eq!(receipt.last_good_generation, Some(active));
}

/// Stale revisions never reach the engine: `session.apply` with a
/// wrong expected revision is rejected by the controller and
/// `graph_builds` does not move. A commit that bypasses the session
/// (direct controller apply) advances the revision without
/// notification — the engine's reconcile reports the drift.
#[test]
fn stale_revision_rejected_and_bypass_surfaces_as_drift() {
    let root = scratch("stale");
    let (mut session, engine) = session_with_engine(&root);

    session
        .apply(
            ProjectOp::SetProjectName { name: "v1".into() },
            session.revision(),
        )
        .unwrap();
    engine.run(1, PumpPolicy::Every(1)).unwrap();
    assert_eq!(engine.graph_builds(), 1);

    // Stale expected revision → controller rejects before the engine.
    let stale = ProjectRevision::default();
    let builds_before = engine.graph_builds();
    assert!(
        session
            .apply(
                ProjectOp::SetProjectName {
                    name: "never".into(),
                },
                stale,
            )
            .is_err()
    );
    assert_eq!(engine.graph_builds(), builds_before);

    // Controller bypass: commit a semantic change without notifying
    // the engine — reconcile must surface the drift.
    let lane = {
        let project = session.project();
        let track = *project.tracks.keys().next_back().unwrap();
        *project.tracks[&track].lane_ids.last().unwrap()
    };
    let source = session.project().clips.values().next().unwrap().source_id;
    let live_revision = session.revision();
    session
        .controller_mut()
        .apply(Command {
            expected_revision: live_revision,
            op: ProjectOp::AddClip {
                lane_id: lane,
                source_id: source,
                placement: media_placement(480, 960, 48_000),
                playback: natural_playback(0, 480),
            },
        })
        .unwrap();
    assert_eq!(engine.graph_builds(), builds_before);

    let report = engine.reconcile(session.project(), session.revision());
    assert!(
        report
            .mismatches
            .contains(&RoutingReconcileMismatch::ProjectRevision)
    );
    assert!(report.fault.is_some());

    // The next session.apply rebuilds against the live revision and
    // republishes — the engine converges back to current.
    session
        .apply(
            ProjectOp::SetProjectName { name: "v2".into() },
            session.revision(),
        )
        .unwrap();
    engine.run(1, PumpPolicy::Every(1)).unwrap();
    let receipt = engine.receipt().unwrap();
    assert!(matches!(
        receipt.state,
        RoutingActivationState::Active { .. }
    ));
    assert!(
        receipt
            .build_key
            .as_ref()
            .is_some_and(|key| key.project_revision == session.revision())
    );
}

/// Removing the verified media blob makes the canonical probe fail —
/// the clip becomes an explicit `MissingSource` skip and the engine
/// records `Failed` while last-good stays live.
#[test]
fn missing_media_fails_build_and_keeps_last_good() {
    let root = scratch("missing");
    let (mut session, engine) = session_with_engine(&root);

    session
        .apply(
            ProjectOp::SetProjectName { name: "v1".into() },
            session.revision(),
        )
        .unwrap();
    engine.run(1, PumpPolicy::Every(1)).unwrap();
    let good = match receipt_state(&engine) {
        RoutingActivationState::Active { generation, .. } => generation,
        ref other => panic!("expected Active, got {other:?}"),
    };

    // Delete the verified blob — the next build cannot resolve the
    // clip's canonical format. Blobs live under `media/blobs/`.
    let blob = fs::read_dir(root.join("media").join("blobs"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    fs::remove_file(&blob).unwrap();

    session
        .apply(
            ProjectOp::SetProjectName { name: "v2".into() },
            session.revision(),
        )
        .unwrap();

    let receipt = engine.receipt().unwrap();
    assert!(matches!(
        receipt.state,
        RoutingActivationState::Failed {
            fault: RoutingFault::MissingSource,
            last_good_generation: Some(g),
        } if g == good
    ));
    assert_eq!(receipt.last_good_generation, Some(good));
}
