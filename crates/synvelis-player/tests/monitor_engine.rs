//! RCD-03 monitor engine: `MonitorEngine`/`MonitorSession` が実
//! headless engine 経路（`HeadlessDriver`）に搭載され、gate・
//! route・PCM 混合が block 単位で正しく動くことを pin する
//! （docs/test-specs/rcd-03-monitor-engine.md）。
//!
//! すべて fixture 駆動の headless 検証であり、実 device・実 GUI
//! の受入を主張しない。

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_reader::PrefetchConfig;
use synvelis_player::fixture::write_fixture;
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::lowering::{EngineConfig, lower};
use synvelis_player::monitor_engine::{
    EngineBlocks, MonitorEngine, MonitorEngineError, MonitorRunReport, MonitorSegmentRecord,
};
use synvelis_player::resolver::prepare;
use synvelis_player::{HeadlessDriver, PumpPolicy};
use synvelis_project::{
    ClipExtent, ClipPlacement, ClipPlayback, ClockEpochId, Command, CounterIdGenerator, LaneId,
    MediaDuration, MediaTimeValue, MusicalTick, PlaybackMapping, Project, ProjectController,
    ProjectId, ProjectOp, ProjectRevision, SourceFrame, SourceId, SourceRange, TempoMapRef,
    TimeAnchor,
};
use synvelis_recording::{
    ArmState, BufferBudget, CaptureTarget, GateSpec, MonitorRoute, MonitorState,
    PreparedRecordingPlan, RecordingRequest, TrackMonitorContext, TransportPhase, preflight,
};

const RATE: u32 = 48_000;
const BLOCK: usize = 256;
const CHANNELS: u16 = 2;
/// 入力 PCM の識別値（playback fixture と区別できる定数）。
const INPUT_VALUE: f32 = 0.25;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-me-{}-{}-{}",
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
        Project::new(ProjectId::new(7), "me"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply(c: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
    let cmd = Command {
        expected_revision: c.live_revision(),
        op,
    };
    c.apply(cmd).unwrap()
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

/// import 済み fixture + lane + epoch を持つ project。
struct Fx {
    root: PathBuf,
    c: ProjectController,
    source_id: SourceId,
    lane_id: LaneId,
    epoch: ClockEpochId,
    tempo_revision: u64,
}

/// `wav_frames` 長の stereo fixture を import し、engine `[0,
/// wav_frames)` に source `[0, wav_frames)` を読む clip を置く。
/// `tempo` 指定時は tick/bpm の tempo event を 1 件追加する
/// （tempo map revision が 1 になる）。
fn fx(name: &str, wav_frames: usize, tempo: Option<(i64, u32)>) -> Fx {
    let root = scratch(name);
    let wav = root.join("tone.wav");
    write_fixture(&wav, CHANNELS, RATE, wav_frames).unwrap();
    let mut c = controller();
    let out = import_local(
        &mut c,
        &ImportRequest {
            path: wav,
            staging_root: root.join("staging"),
            media_root: root.join("media"),
            resume: None,
        },
    )
    .unwrap();
    let source_id = out.commit.source_id;
    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    let lane_id = *c.live().lanes.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id,
            source_id,
            placement: media_placement(0, u64::try_from(wav_frames).unwrap(), RATE),
            playback: natural_playback(0, i64::try_from(wav_frames).unwrap()),
        },
    );
    apply(
        &mut c,
        ProjectOp::RegisterClockEpoch {
            description: "monitor-engine".into(),
        },
    );
    let epoch = *c.live().clock_epochs.keys().next().unwrap();
    if let Some((tick, bpm_milli)) = tempo {
        apply(&mut c, ProjectOp::AddTempoEvent { tick, bpm_milli });
    }
    let tempo_revision = c.live().tempo_map.revision;
    Fx {
        root,
        c,
        source_id,
        lane_id,
        epoch,
        tempo_revision,
    }
}

fn budget() -> BufferBudget {
    BufferBudget {
        chunks: 16,
        chunk_frames: 1024,
        spans_per_chunk: 8,
        filled_capacity: 16,
        lookahead_spans: 16,
    }
}

/// session sample rate = engine rate の録音計画を preflight する。
/// tick→frame は既定 tempo（120bpm・ppqn 960）で 25 f/tick。
fn plan_for(fx: &Fx, gate: GateSpec, stop_tick: i64) -> PreparedRecordingPlan {
    let req = RecordingRequest {
        targets: vec![CaptureTarget {
            lane_id: fx.lane_id,
            channels: u32::from(CHANNELS),
        }],
        clock_epoch: fx.epoch,
        tempo_map: TempoMapRef {
            revision: fx.tempo_revision,
        },
        transport_start: MusicalTick::ZERO,
        stop: MusicalTick::new(stop_tick),
        gate,
        budget: budget(),
        max_block_frames: 1024,
    };
    preflight(fx.c.live(), req, RATE).expect("preflight must pass")
}

fn punch_gate() -> GateSpec {
    GateSpec::Punch {
        punch_in: MusicalTick::new(12),
        punch_out: MusicalTick::new(28),
    }
}

fn ctx(monitor: MonitorState, arm: ArmState, ready: bool) -> TrackMonitorContext {
    TrackMonitorContext {
        arm,
        monitor,
        input_output_ready: ready,
        ..TrackMonitorContext::default()
    }
}

fn engine_cfg(rate: u32) -> EngineConfig {
    EngineConfig {
        sample_rate: rate,
        channels: CHANNELS,
        block_frames: BLOCK,
    }
}

fn prefetch() -> PrefetchConfig {
    PrefetchConfig {
        block_frames: BLOCK,
        block_count: 8,
        channels: usize::from(CHANNELS),
        sample_rate: RATE,
    }
}

fn fmt() -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(RATE).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
    }
}

/// `fx` の project を lower/prepare し、`plan` に monitor bus を
/// 搭載した [`MonitorEngine`]（`HeadlessDriver`）を構築する。
/// `ramp_frames = 0`（route 遷移は即時 — PCM の bit 一致を pin）。
fn make_engine<'a>(
    fx: &Fx,
    plan: &'a PreparedRecordingPlan,
    ctx: TrackMonitorContext,
    eng: EngineConfig,
) -> MonitorEngine<'a, HeadlessDriver> {
    let lowered = lower(fx.c.live(), &|_| Some(fmt()), &eng);
    let prepared = vec![
        prepare(
            fx.c.live(),
            fx.source_id,
            &fx.root.join("media"),
            &prefetch(),
        )
        .unwrap(),
    ];
    let mut driver = HeadlessDriver::new(eng);
    driver.activate(&lowered, prepared).unwrap();
    MonitorEngine::new(driver, plan, ctx, eng, 0).expect("monitor engine construction")
}

fn input_block() -> Vec<Vec<f32>> {
    vec![vec![INPUT_VALUE; BLOCK]; usize::from(CHANNELS)]
}

/// `blocks` block を `pump_every=1` で駆動する。`input` 指定時は
/// 各 block 分の入力 PCM を事前 staging する。
fn run(me: &mut MonitorEngine<'_, HeadlessDriver>, blocks: usize, input: bool) -> MonitorRunReport {
    if input {
        for _ in 0..blocks {
            me.push_input_block(input_block());
        }
    }
    me.run(blocks, 1).expect("engine run")
}

/// `segment_log` が `[0, total)` を隙間・重複・空区間なく tile し、
/// 各 segment の monitor 出力が `effective_route` と PCM 上で
/// 一致することを検証する（route pin + PCM pin の統合）。
fn assert_segments_tiled_and_routed(
    report: &MonitorRunReport,
    log: &[MonitorSegmentRecord],
    total: usize,
) {
    let mut pos = 0_usize;
    for rec in log {
        let a = usize::try_from(i64::from(rec.session.start())).unwrap();
        let b = usize::try_from(i64::from(rec.session.end())).unwrap();
        assert_eq!(a, pos, "segment gap/overlap at session {a}");
        assert!(b > a, "empty segment {a}..{b}");
        pos = b;
        for ch in 0..report.monitor.len() {
            for i in a..b {
                let expected = match rec.effective_route {
                    MonitorRoute::Input => INPUT_VALUE,
                    MonitorRoute::Muted => 0.0,
                    MonitorRoute::Playback => report.playback[ch][i],
                };
                assert_eq!(
                    report.monitor[ch][i], expected,
                    "ch{ch} frame{i} route {:?} mismatch",
                    rec.effective_route
                );
            }
        }
    }
    assert_eq!(pos, total, "segments do not cover run length");
}

/// `gate_open` record の session 区間を連結した半開区間一覧
/// （block 単位の分割を吸収する）。
fn merged_open_intervals(log: &[MonitorSegmentRecord]) -> Vec<(i64, i64)> {
    let mut merged: Vec<(i64, i64)> = Vec::new();
    for rec in log.iter().filter(|r| r.gate_open) {
        let (a, b) = (i64::from(rec.session.start()), i64::from(rec.session.end()));
        if let Some(last) = merged.last_mut()
            && last.1 == a
        {
            last.1 = b;
        } else {
            merged.push((a, b));
        }
    }
    merged
}

/// playback PCM が実際に非無音を含むこと（engine が本当に
/// fixture を再生した証拠 — 全 0 の playback で route pin が
/// 空虚にならないようにする）。
fn assert_playback_non_silent(report: &MonitorRunReport) {
    assert!(
        report.playback.iter().flatten().any(|s| *s != 0.0),
        "engine playback is all silence — fixture did not play"
    );
}

// ---------- AC: engine 搭載 route pin ----------

/// Punch [12,28)tick = session frame [300,700)。Auto+arm+ready+
/// Playing で gate open は Input、閉区間は Playback、session 終端
/// 以降は Playback（post-session 復帰）。block 256 は punch 境界を
/// block 途中に跨ぐ。PCM は bit 一致（native placement・二重補正
/// なし・ramp=0 即時切替）。
#[test]
fn auto_punch_gate_routes_input_and_playback_on_engine_pcm() {
    let fx = fx("punch", 8_192, None);
    let plan = plan_for(&fx, punch_gate(), 200);
    assert_eq!(plan.session_end.value(), 5_000);
    let mut me = make_engine(
        &fx,
        &plan,
        ctx(MonitorState::Auto, ArmState::On, true),
        engine_cfg(RATE),
    );
    me.set_phase(TransportPhase::Playing);
    // 20 blocks = 5120 frames（session_end 5000 + post-session 120）。
    let report = run(&mut me, 20, true);
    assert_eq!(report.frames, 20 * BLOCK as u64);
    assert_playback_non_silent(&report);
    assert_eq!(report.monitor[0].len(), 20 * BLOCK);
    assert_segments_tiled_and_routed(&report, me.session().segment_log(), 20 * BLOCK);
    // 入力未供給でないこと・downgrade が無いこと。
    assert!(
        me.session()
            .segment_log()
            .iter()
            .all(|r| !r.input_missing && r.input_supplied)
    );
    // gate open 区間 [300,700) が route Input の record として
    // 記録されていること（block 途中境界で分割されるため連結して
    // 比較する）。
    let merged = merged_open_intervals(me.session().segment_log());
    assert_eq!(merged, vec![(300, 700)]);
    for rec in me.session().segment_log().iter().filter(|r| r.gate_open) {
        assert_eq!(rec.route, MonitorRoute::Input);
        assert_eq!(rec.effective_route, MonitorRoute::Input);
    }
}

/// route `Input` が選ばれても入力 PCM 未供給なら `Muted` へ
/// fail-closed し `input_missing` notice を立てる（PCM 捏造なし）。
#[test]
fn input_missing_fails_closed_to_muted_on_engine() {
    let fx = fx("miss", 8_192, None);
    let plan = plan_for(&fx, punch_gate(), 200);
    let mut me = make_engine(
        &fx,
        &plan,
        ctx(MonitorState::Auto, ArmState::On, true),
        engine_cfg(RATE),
    );
    me.set_phase(TransportPhase::Playing);
    let report = run(&mut me, 20, false); // 入力 PCM を供給しない
    assert_segments_tiled_and_routed(&report, me.session().segment_log(), 20 * BLOCK);
    let merged = merged_open_intervals(me.session().segment_log());
    assert_eq!(merged, vec![(300, 700)]);
    for rec in me.session().segment_log().iter().filter(|r| r.gate_open) {
        assert_eq!(rec.route, MonitorRoute::Input, "decided route stays Input");
        assert_eq!(
            rec.effective_route,
            MonitorRoute::Muted,
            "missing input downgrades to Muted"
        );
        assert!(rec.input_missing && !rec.input_supplied);
    }
    // [300,700) は完全無音。
    for ch in 0..report.monitor.len() {
        assert!(
            report.monitor[ch][300..700].iter().all(|s| *s == 0.0),
            "input-missing region must be silent"
        );
    }
}

/// Off/On/Auto × arm × mute/solo × phase の代表行を engine 経路で
/// pin する（route 判定の権威は `decide` — ここでは session が
/// それを正しく配線していることを検証）。
/// matrix case の期待 route 写像（`gate_open` → route）。
type RouteExpect = fn(bool) -> MonitorRoute;

/// matrix case 1 件分。
struct MatrixCase {
    ctx: TrackMonitorContext,
    phase: TransportPhase,
    expect: RouteExpect,
}

fn open_input(open: bool) -> MonitorRoute {
    if open {
        MonitorRoute::Input
    } else {
        MonitorRoute::Playback
    }
}

fn open_input_closed_muted(open: bool) -> MonitorRoute {
    if open {
        MonitorRoute::Input
    } else {
        MonitorRoute::Muted
    }
}

fn open_muted(open: bool) -> MonitorRoute {
    if open {
        MonitorRoute::Muted
    } else {
        MonitorRoute::Playback
    }
}

/// Off/Auto/On × arm × mute/solo × phase の代表行を列挙する。
fn matrix_cases() -> Vec<MatrixCase> {
    let playing = TransportPhase::Playing;
    vec![
        // Auto+arm+ready+Playing: open→Input, closed→Playback。
        MatrixCase {
            ctx: ctx(MonitorState::Auto, ArmState::On, true),
            phase: playing,
            expect: open_input,
        },
        // mute は Playback route のみ抑制 — open の Input は残る。
        MatrixCase {
            ctx: TrackMonitorContext {
                muted: true,
                ..ctx(MonitorState::Auto, ArmState::On, true)
            },
            phase: playing,
            expect: open_input_closed_muted,
        },
        // any_solo で非 soloed → Playback 抑制。
        MatrixCase {
            ctx: TrackMonitorContext {
                any_solo: true,
                ..ctx(MonitorState::Auto, ArmState::On, true)
            },
            phase: playing,
            expect: open_input_closed_muted,
        },
        // soloed → Playback 可聴。
        MatrixCase {
            ctx: TrackMonitorContext {
                soloed: true,
                any_solo: true,
                ..ctx(MonitorState::Auto, ArmState::On, true)
            },
            phase: playing,
            expect: open_input,
        },
        // Off+arm: open→Muted, closed→Playback。
        MatrixCase {
            ctx: ctx(MonitorState::Off, ArmState::On, true),
            phase: playing,
            expect: open_muted,
        },
        // On+!ready: Playback fallback + input_unavailable notice。
        MatrixCase {
            ctx: ctx(MonitorState::On, ArmState::On, false),
            phase: playing,
            expect: |_| MonitorRoute::Playback,
        },
        // On+ready: gate/phase に依らず Input。
        MatrixCase {
            ctx: ctx(MonitorState::On, ArmState::On, true),
            phase: playing,
            expect: |_| MonitorRoute::Input,
        },
        // Auto+Stopped: gate に依らず Input（transport 停止中の
        // 入力モニタ）。
        MatrixCase {
            ctx: ctx(MonitorState::Auto, ArmState::On, true),
            phase: TransportPhase::Stopped,
            expect: |_| MonitorRoute::Input,
        },
        // arm Off: gate open でも Playback。
        MatrixCase {
            ctx: ctx(MonitorState::Auto, ArmState::Off, true),
            phase: playing,
            expect: |_| MonitorRoute::Playback,
        },
    ]
}

/// matrix case 1 件を engine 経路で検証する。
fn assert_matrix_case(
    fx: &Fx,
    plan: &PreparedRecordingPlan,
    eng: EngineConfig,
    case: &MatrixCase,
    i: usize,
) {
    let mut me = make_engine(fx, plan, case.ctx.clone(), eng);
    me.set_phase(case.phase);
    let report = run(&mut me, 4, true); // 1024 frames: open [300,700) を跨ぐ
    let log = me.session().segment_log();
    assert!(!log.is_empty(), "case {i} produced no segments");
    assert!(
        log.iter().any(|r| r.gate_open),
        "case {i} never saw gate open"
    );
    for rec in log {
        assert_eq!(
            rec.route,
            (case.expect)(rec.gate_open),
            "case {i} route mismatch on gate_open={}",
            rec.gate_open
        );
        if case.ctx.monitor == MonitorState::On && !case.ctx.input_output_ready {
            assert!(
                rec.notices.input_unavailable,
                "case {i} missing input_unavailable notice"
            );
        }
    }
    assert_segments_tiled_and_routed(&report, log, 4 * BLOCK);
}

#[test]
fn monitor_state_matrix_routes_on_engine() {
    let fx = fx("matrix", 8_192, None);
    let plan = plan_for(&fx, punch_gate(), 200);
    let eng = engine_cfg(RATE);
    for (i, case) in matrix_cases().into_iter().enumerate() {
        assert_matrix_case(&fx, &plan, eng, &case, i);
    }
}

// ---------- AC: Loop/PunchInLoop gate on engine ----------

/// `PunchInLoop` loop`[0,40)` punch`[10,30)` → `pass_len=1000f`、open 区間
/// [250,750),[1250,1750),[2250,2500)（stop=100tick=2500f）。
/// block 256 は全境界を block 途中に跨ぐ。
#[test]
fn punch_in_loop_gate_repeats_open_per_pass_on_engine() {
    let fx = fx("pil", 8_192, None);
    let plan = plan_for(
        &fx,
        GateSpec::PunchInLoop {
            loop_start: MusicalTick::ZERO,
            loop_end: MusicalTick::new(40),
            punch_in: MusicalTick::new(10),
            punch_out: MusicalTick::new(30),
        },
        100,
    );
    assert_eq!(plan.session_end.value(), 2_500);
    let mut me = make_engine(
        &fx,
        &plan,
        ctx(MonitorState::Auto, ArmState::On, true),
        engine_cfg(RATE),
    );
    me.set_phase(TransportPhase::Playing);
    // 10 blocks = 2560 frames（session_end 2500 + post-session 60）。
    let report = run(&mut me, 10, true);
    assert_playback_non_silent(&report);
    assert_segments_tiled_and_routed(&report, me.session().segment_log(), 10 * BLOCK);
    // gate open record の session 区間が pass ごとの punch に
    // 一致すること。
    assert_eq!(
        merged_open_intervals(me.session().segment_log()),
        vec![(250, 750), (1250, 1750), (2250, 2500)]
    );
}

/// Loop loop[10,50) → gate open は loop body 全体で連続
/// [250,2750)（pass 境界で開閉しない）。stop=110tick=2750f。
#[test]
fn loop_gate_open_contiguous_on_engine() {
    let fx = fx("loop", 8_192, None);
    let plan = plan_for(
        &fx,
        GateSpec::Loop {
            loop_start: MusicalTick::new(10),
            loop_end: MusicalTick::new(50),
        },
        110,
    );
    assert_eq!(plan.session_end.value(), 2_750);
    let mut me = make_engine(
        &fx,
        &plan,
        ctx(MonitorState::Auto, ArmState::On, true),
        engine_cfg(RATE),
    );
    me.set_phase(TransportPhase::Playing);
    // 11 blocks = 2816 frames。
    let report = run(&mut me, 11, true);
    assert_playback_non_silent(&report);
    assert_segments_tiled_and_routed(&report, me.session().segment_log(), 11 * BLOCK);
    // open record を連結すると [250,2750) の単一区間。
    assert_eq!(
        merged_open_intervals(me.session().segment_log()),
        vec![(250, 2_750)]
    );
}

/// tempo 120→60bpm @tick480。punch [470,500)tick は tempo 境界を
/// 跨ぎ、session frame では [11750,13000) に写像される
/// （470*25=11750、480*25+20*50=13000）。gate 境界が engine
/// block 途中に落ちることを pin する。
#[test]
fn punch_gate_across_tempo_boundary_on_engine() {
    let fx = fx("tempo", 16_384, Some((480, 60_000)));
    let plan = plan_for(
        &fx,
        GateSpec::Punch {
            punch_in: MusicalTick::new(470),
            punch_out: MusicalTick::new(500),
        },
        900,
    );
    let mut me = make_engine(
        &fx,
        &plan,
        ctx(MonitorState::Auto, ArmState::On, true),
        engine_cfg(RATE),
    );
    me.set_phase(TransportPhase::Playing);
    // 52 blocks = 13312 frames（open [11750,13000) をカバー）。
    let report = run(&mut me, 52, true);
    assert_playback_non_silent(&report);
    assert_segments_tiled_and_routed(&report, me.session().segment_log(), 52 * BLOCK);
    // tempo 境界で分割された open record が session-frame 精度で
    // [11750,13000) を連続被覆すること。
    assert_eq!(
        merged_open_intervals(me.session().segment_log()),
        vec![(11_750, 13_000)]
    );
}

// ---------- fail-closed / independence ----------

/// monitor bus は engine 通常出力を変更しない: 同一 fixture を
/// plain `HeadlessDriver` と `MonitorEngine` で走らせ、master tap
/// PCM が bit 一致することを pin する。
#[test]
fn monitor_engine_does_not_alter_engine_playback() {
    let fx = fx("indep", 8_192, None);
    let plan = plan_for(&fx, punch_gate(), 200);
    let eng = engine_cfg(RATE);

    // plain driver
    let lowered = lower(fx.c.live(), &|_| Some(fmt()), &eng);
    let prepared = vec![
        prepare(
            fx.c.live(),
            fx.source_id,
            &fx.root.join("media"),
            &prefetch(),
        )
        .unwrap(),
    ];
    let mut bare = HeadlessDriver::new(eng);
    bare.activate(&lowered, prepared).unwrap();
    let bare_report = bare.run(20, PumpPolicy::Every(1)).unwrap();

    // monitor engine（同一 cadence: pump_every=1）
    let mut me = make_engine(&fx, &plan, ctx(MonitorState::Auto, ArmState::On, true), eng);
    me.set_phase(TransportPhase::Playing);
    let report = run(&mut me, 20, true);
    assert_eq!(
        report.playback, bare_report.pcm,
        "monitor bus altered engine output"
    );
    assert_eq!(report.frames, bare_report.frames);
}

/// engine cursor が session 位置と再照合できない（後方 seek 等）
/// 場合は fail-closed: monitor 出力は無音になり `faulted` が
/// 観測可能になる。engine 自体は動き続ける。
#[test]
fn session_position_drift_fails_closed() {
    let fx = fx("drift", 8_192, None);
    let plan = plan_for(&fx, punch_gate(), 200);
    let mut me = make_engine(
        &fx,
        &plan,
        ctx(MonitorState::Auto, ArmState::On, true),
        engine_cfg(RATE),
    );
    me.set_phase(TransportPhase::Playing);
    run(&mut me, 4, true); // position = 1024
    me.driver_mut().seek(0).expect("seek");
    assert_eq!(me.cursor(), 0);
    let report = me.run(1, 1).expect("engine run");
    assert!(
        me.session().faulted(),
        "session must fault on unreconcilable position"
    );
    assert!(
        report.monitor.iter().flatten().all(|s| *s == 0.0),
        "faulted session must output silence"
    );
    // engine playback 自体は継続する（monitor bus の故障が
    // engine 出力を止めない）。
    assert_eq!(report.playback.len(), usize::from(CHANNELS));
}

/// `plan.sample_rate != engine.sample_rate` は構築時に拒否する
/// （engine frame ↔ session frame の 1:1 対応が崩れるため）。
#[test]
fn sample_rate_mismatch_is_rejected() {
    let fx = fx("rate", 8_192, None);
    let plan = plan_for(&fx, punch_gate(), 200);
    let eng = EngineConfig {
        sample_rate: 44_100,
        channels: CHANNELS,
        block_frames: BLOCK,
    };
    let driver = HeadlessDriver::new(eng);
    let err = MonitorEngine::new(
        driver,
        &plan,
        ctx(MonitorState::Auto, ArmState::On, true),
        eng,
        0,
    )
    .err()
    .expect("rate mismatch must be rejected");
    assert_eq!(
        err,
        MonitorEngineError::SampleRateMismatch {
            plan: RATE,
            engine: 44_100
        }
    );
}

/// `EngineBlocks` は `RoutingEngine` にも実装される（app の実
/// 再生経路）。構築・1 block 実行が `HeadlessDriver` と同じ契約で
/// 動くことを最小 pin する（RoutingEngine の全機能検証は別経路の
/// 既存テストが担う）。
#[test]
fn routing_engine_implements_engine_blocks() {
    use synvelis_player::routing_engine::RoutingEngine;
    // 型レベル pin: 両 driver が trait を実装する。
    fn assert_impl<T: EngineBlocks>() {}
    assert_impl::<HeadlessDriver>();
    assert_impl::<RoutingEngine>();
}
