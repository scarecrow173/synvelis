//! TIM-04 (issue #128): offline plugin-state recipe — the render
//! preflight quality gate and receipt evidence.
//!
//! `Strict` + non-exact-eligible evidence refuses the export at the
//! preflight stage — before any staged destination exists.
//! `Standard` passes and records the evidence verbatim on the
//! receipt; degraded quality is never upgraded to an exact claim.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use synvelis_player::EngineConfig;
use synvelis_player::fixture::write_fixture;
use synvelis_player::import::{ImportOutcome, ImportRequest, import_local};
use synvelis_project::{
    ClipExtent, ClipPlacement, ClipPlayback, Command, CounterIdGenerator, LaneId, MediaDuration,
    MediaTimeValue, PlaybackMapping, PluginStateQualitySummary, Project, ProjectController,
    ProjectId, ProjectOp, ProjectRevision, SourceFrame, SourceId, SourceRange, StateQualityPolicy,
    TimeAnchor,
};
use synvelis_render::{
    ExportError, ExportJob, ExportOutcome, ExportRequest, ExportStage, OfflineRenderSupport,
    OutputHooks, PluginStateRecipe, PreflightError, RenderSnapshot, TailPolicy, export,
    preflight::preflight_static,
};

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-tim04-{name}-{}-{}",
        std::process::id(),
        TEST_SEQ.fetch_add(1, Ordering::Relaxed)
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(9), "tim04"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn cmd(c: &ProjectController, op: ProjectOp) -> Command<ProjectOp> {
    Command {
        expected_revision: c.live_revision(),
        op,
    }
}

fn apply(c: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
    c.apply(cmd(c, op)).unwrap()
}

fn media_placement(start: i64, end: i64, rate: u32) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Absolute(MediaTimeValue::new(start, 1, rate).unwrap()),
        extent: ClipExtent::Absolute(MediaDuration::new(end - start, 1, rate).unwrap()),
    }
}

fn natural_playback(offset: u64, frames: u64) -> ClipPlayback {
    ClipPlayback {
        source_region: SourceRange::try_new(
            SourceFrame::new(i64::try_from(offset).unwrap()).unwrap(),
            SourceFrame::new(i64::try_from(offset + frames).unwrap()).unwrap(),
        )
        .unwrap(),
        mapping: PlaybackMapping::Natural,
    }
}

fn import(c: &mut ProjectController, root: &Path, path: &Path) -> ImportOutcome {
    import_local(
        c,
        &ImportRequest {
            path: path.to_path_buf(),
            staging_root: root.join("staging"),
            media_root: root.join("media"),
            resume: None,
        },
    )
    .unwrap()
}

fn lane(c: &mut ProjectController) -> LaneId {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    // Semantic lowering requires every clip's track to own a strip.
    let owner = c.live().lanes[&lane].track_id;
    if c.live().routing.strip_of_track(owner).is_none() {
        apply(c, ProjectOp::AddTrackStrip { track_id: owner });
    }
    lane
}

fn add_clip(
    c: &mut ProjectController,
    lane: LaneId,
    source_id: SourceId,
    start: i64,
    end: i64,
    rate: u32,
    offset: u64,
) {
    apply(
        c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id,
            placement: media_placement(start, end, rate),
            playback: natural_playback(offset, u64::try_from(end - start).unwrap()),
        },
    );
}

fn engine(rate: u32, channels: u16, block: usize) -> EngineConfig {
    EngineConfig {
        sample_rate: rate,
        channels,
        block_frames: block,
    }
}

fn request(destination: PathBuf, plugin_state: Option<PluginStateRecipe>) -> ExportRequest {
    ExportRequest {
        engine: engine(48_000, 1, 256),
        range_start: 0,
        range_end: 512,
        tail: Some(TailPolicy::None),
        preroll: 0,
        destination,
        replace_existing: false,
        wire_format: ruviform_audio_core::SampleFormat::S16,
        inserts: Vec::new(),
        max_block: 256,
        clip_support: OfflineRenderSupport::Verified,
        plugin_state,
    }
}

fn recipe(policy: StateQualityPolicy, summary: PluginStateQualitySummary) -> PluginStateRecipe {
    PluginStateRecipe {
        policy,
        evidence: summary,
        manifest_digest: None,
    }
}

fn summary(exact: u64, degraded: u64, unsupported: u64, failed: u64) -> PluginStateQualitySummary {
    PluginStateQualitySummary {
        processors: exact + degraded + unsupported + failed,
        exact,
        degraded,
        unsupported,
        failed,
    }
}

fn failed(outcome: ExportOutcome) -> (ExportStage, ExportError) {
    match outcome {
        ExportOutcome::Failed { stage, error } => (stage, error),
        other => panic!("expected Failed, got {other:?}"),
    }
}

/// One verified clip — the minimal renderable project.
fn basic_fixture(root: &Path) -> (ProjectController, SourceId) {
    let wav = root.join("s.wav");
    write_fixture(&wav, 1, 48_000, 1_024).unwrap();
    let mut c = controller();
    let out = import(&mut c, root, &wav);
    let lane = lane(&mut c);
    add_clip(&mut c, lane, out.commit.source_id, 0, 512, 48_000, 0);
    (c, out.commit.source_id)
}

// ------------------------------------------------------------------
// Preflight gate matrix — `preflight_static` is the first stage and
// touches no destination; the gate ordering is what the e2e below
// proves end-to-end.
// ------------------------------------------------------------------

#[test]
fn strict_rejects_every_non_exact_class() {
    let root = scratch("gate");
    let c = controller();
    let snapshot = RenderSnapshot::freeze(&c, root.join("media"));

    for evidence in [
        summary(1, 1, 0, 0), // one BestEffort
        summary(1, 0, 1, 0), // one Unsupported
        summary(1, 0, 0, 1), // one failed
        summary(0, 0, 0, 0), // empty project is exact-eligible — control
    ] {
        let empty = evidence.processors == 0;
        let req = request(
            root.join("out.wav"),
            Some(recipe(StateQualityPolicy::Strict, evidence)),
        );
        let result = preflight_static(&snapshot, &req);
        if empty {
            assert!(result.is_ok(), "empty evidence is exact-eligible");
        } else {
            assert!(
                matches!(result, Err(PreflightError::PluginStateQuality { .. })),
                "expected PluginStateQuality, got {result:?}"
            );
        }
    }
}

#[test]
fn strict_accepts_exact_and_standard_passes_degraded() {
    let root = scratch("gate2");
    let c = controller();
    let snapshot = RenderSnapshot::freeze(&c, root.join("media"));

    let exact = request(
        root.join("a.wav"),
        Some(recipe(StateQualityPolicy::Strict, summary(2, 0, 0, 0))),
    );
    assert!(preflight_static(&snapshot, &exact).is_ok());

    // Standard never gates — degraded evidence reaches the receipt.
    let degraded = request(
        root.join("b.wav"),
        Some(recipe(StateQualityPolicy::Standard, summary(1, 1, 0, 0))),
    );
    assert!(preflight_static(&snapshot, &degraded).is_ok());

    // No recipe — unchanged behavior.
    let plain = request(root.join("c.wav"), None);
    assert!(preflight_static(&snapshot, &plain).is_ok());
}

// ------------------------------------------------------------------
// End-to-end: the gate fires before any output byte exists; Standard
// records the recipe verbatim on the terminal receipt.
// ------------------------------------------------------------------

#[test]
fn strict_export_fails_at_preflight_before_destination() {
    let root = scratch("e2e-strict");
    let (c, _src) = basic_fixture(&root);
    let snapshot = RenderSnapshot::freeze(&c, root.join("media"));

    let dest = root.join("strict.wav");
    let req = request(
        dest.clone(),
        Some(recipe(StateQualityPolicy::Strict, summary(1, 1, 0, 0))),
    );
    let job = ExportJob::new();
    let (stage, err) = failed(export(&snapshot, &req, OutputHooks::default(), &job));
    assert_eq!(stage, ExportStage::Preflight);
    assert!(matches!(
        err,
        ExportError::Preflight(PreflightError::PluginStateQuality { .. })
    ));
    // Nothing was staged or published — the destination never existed.
    assert!(!dest.exists());
}

#[test]
fn standard_export_completes_and_records_evidence_verbatim() {
    let root = scratch("e2e-standard");
    let (c, _src) = basic_fixture(&root);
    let snapshot = RenderSnapshot::freeze(&c, root.join("media"));

    let dest = root.join("standard.wav");
    let req = request(
        dest.clone(),
        Some(recipe(StateQualityPolicy::Standard, summary(1, 1, 0, 0))),
    );
    let job = ExportJob::new();
    let outcome = export(&snapshot, &req, OutputHooks::default(), &job);
    let ExportOutcome::Completed { receipt } = outcome else {
        panic!("expected Completed, got {outcome:?}");
    };
    let recorded = receipt.plugin_state.expect("recipe evidence on receipt");
    assert_eq!(recorded.policy, StateQualityPolicy::Standard);
    assert_eq!(recorded.evidence.processors, 2);
    assert_eq!(recorded.evidence.exact, 1);
    assert_eq!(recorded.evidence.degraded, 1);
    // The degraded record stays degraded — never an exact claim.
    assert!(!recorded.evidence.exact_eligible());
    assert!(dest.exists());
}

#[test]
fn strict_export_with_exact_evidence_completes() {
    let root = scratch("e2e-exact");
    let (c, _src) = basic_fixture(&root);
    let snapshot = RenderSnapshot::freeze(&c, root.join("media"));

    let dest = root.join("exact.wav");
    let req = request(
        dest.clone(),
        Some(recipe(StateQualityPolicy::Strict, summary(2, 0, 0, 0))),
    );
    let job = ExportJob::new();
    let outcome = export(&snapshot, &req, OutputHooks::default(), &job);
    assert!(outcome.is_completed(), "got {outcome:?}");
    assert!(dest.exists());
}
