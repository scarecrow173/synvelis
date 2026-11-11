//! RCD-04 slice A (synvelis#118): an adopted `EffectiveComp` plays
//! through the REAL RT engine paths — legacy `lower()` +
//! `HeadlessDriver::run` and semantic `lower_semantic` +
//! `RoutingEngine::run` — and its PCM is compared against the real
//! offline render path (`export()` → `ClipRenderer` → reopened F32
//! WAV captured via `expect_pcm`). All three consume the shared
//! `plan_playbacks`/`FadeWindow` authority, so this test detects
//! drift the crate fixture's test-local renderer copy cannot
//! (ClipPlayer↔ClipRenderer differences, prefetch/resync vs bounded
//! decode, semantic wiring differences).
//!
//! Evidence classification: this is headless + deterministic-fixture
//! evidence only. Real device/OS GUI/AudioLens f64 lanes are
//! NotRun/Unsupported for this slice — passing here does not claim
//! real-machine playback equivalence.
//!
//! Tempo fixture: default map is 120 BPM @ PPQN 960 → 25 frames/tick
//! @48 kHz; S1 adds `AddTempoEvent{tick:2880, bpm_milli:90_000}` so
//! regions/transitions after tick 2880 resolve through a non-uniform
//! (33⅓ frames/tick) tempo boundary on every path.
//!
//! Tempo geometry (S1): `[0,2880)t` → 25 f/t → `[0,72000)f`;
//! `[2880,3840)t` → 100/3 f/t → `+32_000f`; comp target resolves to
//! `[0,104_000)f`. Silence region `[2400,2880)t` → `[60000,72000)f`;
//! T3 fade-in overhang `[2820,2880)t` → `[70500,72000)f` intrudes
//! into it — probed below.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_reader::PrefetchConfig;

use synvelis_player::driver::HeadlessDriver;
use synvelis_player::fixture::{fixture_sample_f32, write_fixture};
use synvelis_player::import::{ImportRequest, blob_path, import_local};
use synvelis_player::lowering::{
    CompSkipReason, EngineConfig, NodeContent, PlaybackPlan, SkippedPlayback, lower, plan_playbacks,
};
use synvelis_player::resolver::{PreparedClip, prepare};
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::{PlayReport, PumpPolicy};
use synvelis_project::{
    BuildBudget, BuildMode, ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command,
    CompCandidateRef, CompOp, CompRegionId, CounterIdGenerator, FadeShape, MediaDuration,
    MediaTimeValue, MusicalRange, MusicalTick, PlaybackMapping, Project, ProjectController,
    ProjectId, ProjectOp, ProjectRange, ProjectRevision, RoutingBuildRequestId,
    RoutingCompileRequest, RoutingFault, RuntimeClockEpochRef, RuntimeInstanceId, SessionFrame,
    SessionRange, SourceFrame, SourceId, SourceRange, StretchRatio, TakeId, TimeAnchor,
};
use synvelis_recording::SynvelisOp;
use synvelis_recording::ops::RecordingOp;
use synvelis_render::{
    ExportError, ExportJob, ExportOutcome, ExportRequest, ExportStage, OfflineRenderSupport,
    OutputHooks, PreflightError, RenderSnapshot, TailPolicy, export,
};

const RATE: u32 = 48_000;
const CHANNELS: u16 = 2;
const BLOCK: usize = 256;
/// Source length: every comp read (incl. transition overhang) stays
/// inside it.
const SOURCE_FRAMES: usize = 400_000;
/// Take coverage windows — three passes over the same source at
/// disjoint source windows stand in for correlated material.
const TAKE_SPAN: i64 = 120_000;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-rcd04-rtpcm-{}-{}-{}",
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
        Project::new(ProjectId::new(0xC), "rcd04-rtpcm"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply<O: synvelis_project::Operation>(c: &mut ProjectController, op: O) -> ProjectRevision {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .expect("op applies")
}

fn pop(c: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
    apply(c, SynvelisOp::Project(op))
}

fn rop(c: &mut ProjectController, op: RecordingOp) -> ProjectRevision {
    apply(c, SynvelisOp::Recording(op))
}

fn cop(c: &mut ProjectController, op: CompOp) -> ProjectRevision {
    apply(c, SynvelisOp::Comp(op))
}

fn mr(start: i64, end: i64) -> ProjectRange {
    ProjectRange::Musical(
        MusicalRange::try_new(MusicalTick::new(start), MusicalTick::new(end)).unwrap(),
    )
}

fn media_placement(start_frame: i64, end_frame: i64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Absolute(MediaTimeValue::new(start_frame, 1, RATE).unwrap()),
        extent: ClipExtent::Absolute(MediaDuration::new(end_frame - start_frame, 1, RATE).unwrap()),
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

fn engine() -> EngineConfig {
    EngineConfig {
        sample_rate: RATE,
        channels: CHANNELS,
        block_frames: BLOCK,
    }
}

fn prefetch() -> PrefetchConfig {
    PrefetchConfig {
        block_frames: 256,
        block_count: 8,
        channels: usize::from(CHANNELS),
        sample_rate: RATE,
    }
}

fn canonical() -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(RATE).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
}

fn request(id: u64, revision: ProjectRevision) -> RoutingCompileRequest {
    RoutingCompileRequest {
        request_id: RoutingBuildRequestId(id),
        expected_project_revision: revision,
        mode: BuildMode::Realtime,
        resource_budget: BuildBudget {
            max_nodes: 4_096,
            max_connections: 16_384,
            max_frames: 512,
        },
        runtime_instance: RuntimeInstanceId(1),
        device_clock_epoch: RuntimeClockEpochRef(1),
    }
}

fn blocks_for(frames: u64) -> usize {
    usize::try_from(frames.div_ceil(BLOCK as u64)).unwrap()
}

fn trim(report: &PlayReport, frames: u64) -> Vec<Vec<f32>> {
    let n = usize::try_from(frames).unwrap();
    report.pcm.iter().map(|ch| ch[..n].to_vec()).collect()
}

/// IDs the comp fixture hands back to assertions.
struct CompFixture {
    lane: synvelis_project::LaneId,
    source: SourceId,
    takes: [TakeId; 3],
    regions: Vec<CompRegionId>,
    /// The lane clip superseded by the active comp (S1 only).
    clip: Option<ClipId>,
}

/// Imports the stereo fixture and registers one session/stream plus
/// `pass_count` passes, each producing one verified take covering a
/// disjoint `TAKE_SPAN` source window — the correlated-material
/// stand-in. Returns `(source_id, lane, take_ids)`.
fn substrate(
    c: &mut ProjectController,
    root: &Path,
    pass_count: usize,
) -> (SourceId, synvelis_project::LaneId, Vec<TakeId>) {
    let wav = root.join("s.wav");
    write_fixture(&wav, CHANNELS, RATE, SOURCE_FRAMES).unwrap();
    let out = import_local(
        c,
        &ImportRequest {
            path: wav,
            staging_root: root.join("staging"),
            media_root: root.join("media"),
            resume: None,
        },
    )
    .unwrap();
    let source_id = out.commit.source_id;

    pop(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    pop(c, ProjectOp::AddTrackStrip { track_id: track });
    pop(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    pop(
        c,
        ProjectOp::RegisterClockEpoch {
            description: "clk".into(),
        },
    );
    let epoch = *c.live().clock_epochs.keys().next().unwrap();

    rop(c, RecordingOp::CreateSession { name: "s".into() });
    let session = *c.live().recording.sessions.keys().next().unwrap();
    rop(
        c,
        RecordingOp::OpenSession {
            session_id: session,
        },
    );
    rop(
        c,
        RecordingOp::RegisterStream {
            session_id: session,
            lane_id: lane,
            clock_epoch_id: epoch,
        },
    );
    let stream = *c.live().recording.streams.keys().next().unwrap();

    let mut takes = Vec::new();
    for index in 0..pass_count {
        let lo = i64::try_from(index).unwrap() * TAKE_SPAN;
        rop(
            c,
            RecordingOp::RegisterPass {
                session_id: session,
                index: u32::try_from(index).unwrap(),
            },
        );
        let pass = *c.live().recording.passes.keys().next_back().unwrap();
        rop(
            c,
            RecordingOp::RegisterSegment {
                stream_id: stream,
                pass_id: pass,
                session_range: SessionRange::try_new(
                    SessionFrame::new(lo).unwrap(),
                    SessionFrame::new(lo + TAKE_SPAN).unwrap(),
                )
                .unwrap(),
                source_range: SourceRange::try_new(
                    SourceFrame::new(lo).unwrap(),
                    SourceFrame::new(lo + TAKE_SPAN).unwrap(),
                )
                .unwrap(),
                source_id,
            },
        );
        let segment = *c.live().recording.segments.keys().next_back().unwrap();
        rop(
            c,
            RecordingOp::RegisterTake {
                session_id: session,
                pass_id: pass,
                stream_id: stream,
                lane_id: lane,
                source_id,
                segment_ids: vec![segment],
            },
        );
        let take = *c.live().recording.takes.keys().next_back().unwrap();
        rop(c, RecordingOp::MarkTakeVerified { take_id: take });
        takes.push(take);
    }
    (source_id, lane, takes)
}

/// Registers the take group + comp + regions/transitions given as
/// `(project_range, candidate, mapping)` and `transition specs`,
/// activates the comp and returns the region ids in insertion order.
fn build_comp(
    c: &mut ProjectController,
    lane: synvelis_project::LaneId,
    target: ProjectRange,
    take_ids: Vec<TakeId>,
    regions: &[(ProjectRange, CompCandidateRef, PlaybackMapping)],
    transitions: &[(usize, usize, ProjectRange, FadeShape, FadeShape)],
) -> Vec<CompRegionId> {
    cop(
        c,
        CompOp::RegisterTakeGroup {
            lane_id: lane,
            target_range: target,
            take_ids,
            source_rate: RATE,
        },
    );
    let group = *c.live().comp.take_groups.keys().next().unwrap();
    cop(
        c,
        CompOp::CreateComp {
            take_group_id: group,
        },
    );
    let comp = *c.live().comp.comps.keys().next().unwrap();
    for (range, candidate, mapping) in regions {
        cop(
            c,
            CompOp::AddCompRegion {
                comp_id: comp,
                project_range: *range,
                candidate: candidate.clone(),
                mapping: *mapping,
            },
        );
    }
    cop(
        c,
        CompOp::SetActiveComp {
            take_group_id: group,
            comp_id: Some(comp),
        },
    );
    let region_ids = c.live().comp.comps[&comp].region_ids.clone();
    for &(left, right, ref overlap, fade_out, fade_in) in transitions {
        cop(
            c,
            CompOp::AddCompTransition {
                left_region_id: region_ids[left],
                right_region_id: region_ids[right],
                overlap_range: *overlap,
                fade_out,
                fade_in,
            },
        );
    }
    region_ids
}

fn take_cand(take_id: TakeId, offset: i64) -> CompCandidateRef {
    CompCandidateRef::Take {
        take_id,
        source_offset: SourceFrame::new(offset).unwrap(),
    }
}

fn existing_source_cand(source_id: SourceId, play_from: i64) -> CompCandidateRef {
    CompCandidateRef::ExistingSource {
        source_id,
        handle: SourceRange::try_new(
            SourceFrame::ZERO,
            SourceFrame::new(i64::try_from(SOURCE_FRAMES).unwrap()).unwrap(),
        )
        .unwrap(),
        play_from: SourceFrame::new(play_from).unwrap(),
    }
}

/// S1 fixture: 6 regions (Take×4 across correlated windows /
/// `ExistingSource` / Silence) + 3 transitions (EP-out/Lin-in,
/// Lin-out/EP-in, silence-side Lin-in) + tempo change at tick 2880 +
/// a lane clip inside the target that must be superseded.
fn comp_project_s1(root: &Path) -> (ProjectController, CompFixture) {
    let mut c = controller();
    let (source, lane, takes) = substrate(&mut c, root, 3);

    // Lane clip inside the comp target span — superseded, never
    // double-played (its source window differs from every binding).
    pop(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id: source,
            placement: media_placement(0, 48_000),
            playback: natural_playback(12_000, 48_000),
        },
    );
    let clip = *c.live().clips.keys().next().unwrap();

    let regions = [
        (
            mr(0, 960),
            take_cand(takes[0], 8_000),
            PlaybackMapping::Natural,
        ),
        (
            mr(960, 1_920),
            take_cand(takes[1], TAKE_SPAN + 4_000),
            PlaybackMapping::Natural,
        ),
        (
            mr(1_920, 2_400),
            existing_source_cand(source, 30_000),
            PlaybackMapping::Natural,
        ),
        (
            mr(2_400, 2_880),
            CompCandidateRef::Silence,
            PlaybackMapping::Natural,
        ),
        (
            mr(2_880, 3_360),
            take_cand(takes[2], 2 * TAKE_SPAN + 12_000),
            PlaybackMapping::Natural,
        ),
        (
            mr(3_360, 3_840),
            take_cand(takes[0], 40_000),
            PlaybackMapping::Natural,
        ),
    ];
    let transitions = [
        (
            0,
            1,
            mr(900, 1_020),
            FadeShape::EqualPower,
            FadeShape::Linear,
        ),
        (
            1,
            2,
            mr(1_860, 1_980),
            FadeShape::Linear,
            FadeShape::EqualPower,
        ),
        (3, 4, mr(2_820, 2_940), FadeShape::Linear, FadeShape::Linear),
    ];
    let region_ids = build_comp(
        &mut c,
        lane,
        mr(0, 3_840),
        takes.clone(),
        &regions,
        &transitions,
    );

    // 120→90 BPM at tick 2880: post-2880 placements resolve through a
    // non-uniform tempo boundary on every path.
    pop(
        &mut c,
        ProjectOp::AddTempoEvent {
            tick: 2_880,
            bpm_milli: 90_000,
        },
    );

    (
        c,
        CompFixture {
            lane,
            source,
            takes: takes.try_into().unwrap(),
            regions: region_ids,
            clip: Some(clip),
        },
    )
}

/// S2 fixture: Natural + Loop + `SimpleStretch` + Natural regions —
/// unsupported mappings must fail closed on every path.
fn comp_project_s2(root: &Path) -> (ProjectController, CompFixture) {
    let mut c = controller();
    let (source, lane, takes) = substrate(&mut c, root, 3);
    let regions = [
        (
            mr(0, 960),
            take_cand(takes[0], 8_000),
            PlaybackMapping::Natural,
        ),
        (
            mr(960, 1_920),
            take_cand(takes[1], TAKE_SPAN + 4_000),
            PlaybackMapping::Loop,
        ),
        (
            mr(1_920, 2_880),
            take_cand(takes[1], TAKE_SPAN + 40_000),
            PlaybackMapping::SimpleStretch(StretchRatio::new(2, 1).unwrap()),
        ),
        (
            mr(2_880, 3_360),
            take_cand(takes[2], 2 * TAKE_SPAN + 12_000),
            PlaybackMapping::Natural,
        ),
    ];
    let region_ids = build_comp(&mut c, lane, mr(0, 3_360), takes.clone(), &regions, &[]);
    (
        c,
        CompFixture {
            lane,
            source,
            takes: takes.try_into().unwrap(),
            regions: region_ids,
            clip: None,
        },
    )
}

// ---------- drivers: the three real implementation paths ----------

/// Legacy RT path: `lower()` → `ClipPlayer`s → `HeadlessDriver::run`.
fn run_driver(c: &ProjectController, media_root: &Path, frames: u64) -> PlayReport {
    let eng = engine();
    let lowered = lower(c.live(), &|_| Some(canonical()), &eng);
    let prepared: Vec<PreparedClip> = lowered
        .playbacks
        .iter()
        .map(|b| prepare(c.live(), b.source_id, media_root, &prefetch()).unwrap())
        .collect();
    let mut driver = HeadlessDriver::new(eng);
    driver.activate(&lowered, prepared).unwrap();
    driver
        .run(blocks_for(frames), PumpPolicy::Every(1))
        .unwrap()
}

/// Semantic RT path: `RoutingEngine::prepare` (→ `lower_semantic`) →
/// `publish` → `run`. `deps.clips` follows `plan_playbacks` order —
/// the same order `lower_semantic` emits its playbacks.
fn run_engine(
    c: &ProjectController,
    media_root: &Path,
    frames: u64,
) -> Result<PlayReport, RoutingFault> {
    let eng = engine();
    let (plans, _skipped) = plan_playbacks(c.live(), &|_| Some(canonical()), &eng);
    let clips: Vec<PreparedClip> = plans
        .iter()
        .map(|p| prepare(c.live(), p.source_id, media_root, &prefetch()).unwrap())
        .collect();
    let mut engine_rt = RoutingEngine::new(eng, RuntimeInstanceId(1), RuntimeClockEpochRef(1));
    let rev = c.live_revision();
    let prepared = engine_rt.prepare(
        &request(1, rev),
        c.live(),
        rev,
        RoutingDeps {
            canonical_of: &|_| Some(canonical()),
            clips,
            engine_config_generation: 1,
            source_format_generation: 1,
        },
    )?;
    engine_rt
        .publish(prepared.expect_built(), c.live(), rev)
        .unwrap();
    Ok(engine_rt
        .run(blocks_for(frames), PumpPolicy::Every(1))
        .expect("RT run"))
}

/// Offline path: `export()` → `ClipRenderer`s → F32 WAV → reopen →
/// `expect_pcm` captures the decoded planes the file actually holds.
fn run_export(c: &ProjectController, media_root: &Path, dest: &Path, frames: u64) -> Vec<Vec<f32>> {
    let snapshot = RenderSnapshot::freeze(c, media_root);
    let req = ExportRequest {
        engine: engine(),
        range_start: 0,
        range_end: frames,
        tail: Some(TailPolicy::None),
        preroll: 0,
        destination: dest.to_path_buf(),
        replace_existing: false,
        wire_format: SampleFormat::F32,
        inserts: Vec::new(),
        max_block: 1_024,
        clip_support: OfflineRenderSupport::Verified,
        plugin_state: None,
    };
    let captured: Arc<Mutex<Option<Vec<Vec<f32>>>>> = Arc::new(Mutex::new(None));
    let cap = captured.clone();
    let hooks = OutputHooks {
        expect_pcm: Some(Box::new(move |planes: &[Vec<f32>]| {
            *cap.lock().unwrap() = Some(planes.to_vec());
            true
        })),
        ..OutputHooks::default()
    };
    let outcome = export(&snapshot, &req, hooks, &ExportJob::default());
    let ExportOutcome::Completed { receipt } = outcome else {
        panic!("export must complete: {outcome:?}");
    };
    assert!(receipt.verified.reopened, "independent reopen ran");
    captured.lock().unwrap().clone().unwrap()
}

// ---------- plan-derived anchors (shared authority, not a renderer) ----------

/// The expected master sample at `engine_frame` derived from
/// `plan_playbacks` — each covering plan contributes its window-mapped
/// `fixture_sample_f32` scaled by its `FadeWindow::gain_at`. Two-term
/// sums are the only mixes this fixture produces, so f32 addition is
/// commutative-exact here.
fn expected_at(plans: &[PlaybackPlan], channel: usize, engine_frame: i64) -> f32 {
    let mut acc = 0.0_f32;
    for plan in plans {
        let start = i64::try_from(plan.window.engine_start).unwrap();
        let end = start + i64::try_from(plan.window.engine_len).unwrap();
        if engine_frame < start || engine_frame >= end {
            continue;
        }
        let src = usize::try_from(plan.window.source_start).unwrap()
            + usize::try_from(engine_frame - start).unwrap();
        let gain = plan.fade.map_or(1.0, |fade| fade.gain_at(engine_frame));
        acc += fixture_sample_f32(channel, src) * gain;
    }
    acc
}

fn assert_planes(got: &[Vec<f32>], want: &[Vec<f32>], label: &str) {
    assert_eq!(got.len(), want.len(), "{label}: channels");
    for (c, (g, w)) in got.iter().zip(want.iter()).enumerate() {
        assert_eq!(g.len(), w.len(), "{label}: ch{c} len");
        for (i, (a, b)) in g.iter().zip(w.iter()).enumerate() {
            assert_eq!(a, b, "{label}: ch{c} frame{i}: got {a}, want {b}");
        }
    }
}

/// Tempo-boundary proof: post-2880 placements resolve at 100/3
/// frames/tick, not the default 25 — R5 `[2880,3360)t` →
/// `[72000,88000)f`, R6 `[3360,3840)t` → `[88000,104000)f` (pure
/// 120 BPM would put them at `[72000,84000)`/`[84000,96000)`).
/// Without these asserts the tempo coverage could pass vacuously.
fn assert_tempo_windows(plans: &[PlaybackPlan], fx: &CompFixture) {
    let seg_window = |region_id: CompRegionId| {
        plans
            .iter()
            .find(|p| {
                matches!(p.content, NodeContent::CompSegment { region_id: r, .. } if r == region_id)
            })
            .map(|p| (p.window.engine_start, p.window.engine_len))
            .expect("region lowered")
    };
    assert_eq!(seg_window(fx.regions[0]), (0, 24_000));
    assert_eq!(seg_window(fx.regions[4]), (72_000, 16_000));
    assert_eq!(seg_window(fx.regions[5]), (88_000, 16_000));
    // T3's silence-side fade-in overhang resolves to [70500,72000)f
    // around boundary 72000 — inside the silence region's own span.
    let t3_right = plans
        .iter()
        .find(|p| {
            matches!(
                p.content,
                NodeContent::CompOverlap {
                    side: synvelis_player::lowering::OverlapSide::Right,
                    ..
                }
            ) && p.window.engine_start == 70_500
        })
        .expect("T3 right side at 70500");
    assert_eq!(t3_right.window.engine_len, 1_500);
}

/// Project+media fingerprint captured before playback/render and
/// compared afterward — neither path may mutate its inputs.
struct Fingerprint {
    revision: ProjectRevision,
    digest: String,
    blob_bytes: Vec<u8>,
    takes: usize,
    assets: usize,
}

fn fingerprint(c: &ProjectController, media_root: &Path, source: SourceId) -> Fingerprint {
    let digest = c.live().sources[&source].content_digest.clone();
    Fingerprint {
        revision: c.live_revision(),
        blob_bytes: fs::read(blob_path(media_root, &digest)).unwrap(),
        digest,
        takes: c.live().recording.takes.len(),
        assets: c.live().assets.len(),
    }
}

impl Fingerprint {
    fn assert_unchanged(&self, c: &ProjectController, media_root: &Path, source: SourceId) {
        assert_eq!(c.live_revision(), self.revision, "project revision");
        assert_eq!(
            c.live().sources[&source].content_digest,
            self.digest,
            "source digest"
        );
        assert_eq!(
            fs::read(blob_path(media_root, &self.digest)).unwrap(),
            self.blob_bytes,
            "media bytes"
        );
        assert_eq!(c.live().recording.takes.len(), self.takes);
        assert_eq!(c.live().assets.len(), self.assets);
    }
}

// ------------------------------------------------------------------ S1

/// S1: comp playback through BOTH real RT engine paths matches the
/// real offline render path sample-for-sample — the Issue #118
/// RT/offline PCM equivalence evidence for Comp conditions (source
/// handle ranges, fade spans, correlated material, transient-carrying
/// deterministic samples, beat anchors under a tempo boundary, and
/// the supersession of a lane clip).
#[test]
fn rcd_04_rt_engine_comp_pcm_matches_offline_export() {
    const FRAMES: u64 = 104_000; // tick 3840 under the two-segment tempo map
    let root = scratch("s1");
    let media_root = root.join("media");
    let (c, fx) = comp_project_s1(&root);

    // Skip/plan inventory: 5 segments + 5 transition sides; the lane
    // clip superseded and the silence region explicit — nothing else.
    let (plans, skipped) = plan_playbacks(c.live(), &|_| Some(canonical()), &engine());
    assert_eq!(plans.len(), 10, "plans: {plans:?}");
    let comp = *c.live().comp.comps.keys().next().unwrap();
    let seg = plans
        .iter()
        .filter(|p| matches!(p.content, NodeContent::CompSegment { .. }))
        .count();
    let side = plans
        .iter()
        .filter(|p| matches!(p.content, NodeContent::CompOverlap { .. }))
        .count();
    assert_eq!((seg, side), (5, 5));
    assert_eq!(
        skipped,
        vec![
            SkippedPlayback::SupersededByComp {
                clip_id: fx.clip.unwrap(),
                take_group_id: *c.live().comp.take_groups.keys().next().unwrap(),
            },
            SkippedPlayback::CompSilence {
                comp_id: comp,
                region_id: fx.regions[3],
            },
        ],
        "skip inventory"
    );

    assert_tempo_windows(&plans, &fx);

    // Media identity before running anything.
    let fp = fingerprint(&c, &media_root, fx.source);

    // Path 1 — legacy RT driver (lower → ClipPlayer → RT graph).
    let driver_report = run_driver(&c, &media_root, FRAMES);
    // Path 2 — semantic RT engine (lower_semantic → ClipPlayer → RT graph).
    let engine_report = run_engine(&c, &media_root, FRAMES).expect("semantic build succeeds");
    // Path 3 — offline render (lower_semantic_offline → ClipRenderer → WAV).
    let export_planes = run_export(&c, &media_root, &root.join("export.wav"), FRAMES);

    let driver_pcm = trim(&driver_report, FRAMES);
    let engine_pcm = trim(&engine_report, FRAMES);

    // The core deliverable: three implementation paths agree on every
    // sample, and the two RT reports share the deterministic CRC.
    assert_planes(&engine_pcm, &driver_pcm, "engine vs driver");
    assert_planes(&export_planes, &driver_pcm, "export vs driver");
    assert_eq!(engine_report.crc, driver_report.crc, "RT CRCs agree");

    // Determinism: a fresh lower+prepare+activate+run reproduces the
    // identical report.
    let again = run_driver(&c, &media_root, FRAMES);
    assert_eq!(again.crc, driver_report.crc, "repeat run deterministic");
    assert_eq!(again.pcm, driver_report.pcm);

    // Plan-derived anchors — identity, silence, the T3 fade-in
    // overhang inside the silence region's span, and both fade shapes
    // on both sides of each transition boundary. These reference
    // values come from the shared `plan_playbacks`/`FadeWindow`
    // authority both paths consume — not from a copied renderer.
    let probes: &[(&str, i64)] = &[
        ("identity r1", 1_000),
        ("identity r2", 26_000),
        ("identity r3", 50_000),
        ("t1 fade-in half", 22_501),
        ("t1 boundary-1", 23_999),
        ("t1 boundary+1", 24_001),
        ("t1 fade-out tail", 25_499),
        ("t2 fade-in", 46_501),
        ("t2 fade-out", 49_499),
        ("silence inside", 65_000),
        ("t3 fade-in overhang", 71_000),
        ("post-tempo r5", 80_000),
        ("post-tempo r6", 100_000),
    ];
    for (ch, pcm) in driver_pcm.iter().enumerate() {
        for &(label, f) in probes {
            let want = expected_at(&plans, ch, f);
            let got = pcm[usize::try_from(f).unwrap()];
            assert_eq!(got, want, "{label} ch{ch} f{f}: got {got}, want {want}");
        }
    }
    // Real-content check: identity spans are not a silent false-pass.
    assert!(driver_pcm.iter().flatten().any(|s| *s != 0.0));
    // Explicit silence: inside R4 before the T3 fade-in overhang.
    assert!(
        driver_pcm
            .iter()
            .all(|ch| ch[60_000..70_500].iter().all(|s| *s == 0.0)),
        "silence region [60000,70500)"
    );

    // Immutability: playback and render mutate neither project state
    // nor media bytes.
    fp.assert_unchanged(&c, &media_root, fx.source);
    let _ = fx.lane;
    let _ = fx.takes;
}

// ------------------------------------------------------------------ S2

/// S2: `Loop`/`SimpleStretch` comp regions fail closed on every path —
/// the honest difference is recorded: the legacy lowering emits
/// explicit `UnsupportedMapping` skips and the driver plays the
/// remaining regions with silence over the unlowerable spans, while
/// the semantic engine and the offline export refuse the whole build
/// (`RoutingFault::UnsupportedFormatOrTimeMap` /
/// `PreflightError::UnrenderableComp`). That divergence is the
/// declared skip contract, not a defect.
#[test]
fn rcd_04_rt_engine_unsupported_mapping_fails_closed() {
    const FRAMES: u64 = 84_000; // tick 3360 @ 25 f/t, default map
    let root = scratch("s2");
    let media_root = root.join("media");
    let (c, fx) = comp_project_s2(&root);

    let comp = *c.live().comp.comps.keys().next().unwrap();
    let (plans, skipped) = plan_playbacks(c.live(), &|_| Some(canonical()), &engine());
    assert_eq!(plans.len(), 2, "only the two Natural regions lower");
    assert_eq!(
        skipped,
        vec![
            SkippedPlayback::CompSegment {
                comp_id: comp,
                region_id: fx.regions[1],
                reason: CompSkipReason::UnsupportedMapping,
            },
            SkippedPlayback::CompSegment {
                comp_id: comp,
                region_id: fx.regions[2],
                reason: CompSkipReason::UnsupportedMapping,
            },
        ],
        "explicit unsupported-mapping skips"
    );

    // Legacy path: explicit skips → silence over [24000,72000); the
    // Natural segments still play their real samples.
    let report = run_driver(&c, &media_root, FRAMES);
    let pcm = trim(&report, FRAMES);
    for ch in &pcm {
        assert!(ch[..24_000].iter().any(|s| *s != 0.0), "R_a plays");
        assert!(
            ch[24_000..72_000].iter().all(|s| *s == 0.0),
            "skipped regions stay silent"
        );
        assert!(ch[72_000..].iter().any(|s| *s != 0.0), "R_d plays");
    }

    // Semantic RT: the whole build is refused — never partial audio.
    let result = run_engine(&c, &media_root, FRAMES);
    assert_eq!(
        result.unwrap_err(),
        RoutingFault::UnsupportedFormatOrTimeMap,
        "semantic build fails closed"
    );

    // Offline: export refuses at preflight with a typed comp error.
    let snapshot = RenderSnapshot::freeze(&c, &media_root);
    let req = ExportRequest {
        engine: engine(),
        range_start: 0,
        range_end: FRAMES,
        tail: Some(TailPolicy::None),
        preroll: 0,
        destination: root.join("s2.wav"),
        replace_existing: false,
        wire_format: SampleFormat::F32,
        inserts: Vec::new(),
        max_block: 1_024,
        clip_support: OfflineRenderSupport::Verified,
        plugin_state: None,
    };
    let outcome = export(
        &snapshot,
        &req,
        OutputHooks::default(),
        &ExportJob::default(),
    );
    let ExportOutcome::Failed { stage, error } = outcome else {
        panic!("export must fail closed: {outcome:?}");
    };
    assert_eq!(stage, ExportStage::Preflight);
    assert!(
        matches!(
            error,
            ExportError::Preflight(PreflightError::UnrenderableComp { .. })
        ),
        "expected UnrenderableComp, got {error:?}"
    );
}
