//! RCD-04 Take/Comp PCM fixture — RT(captured)-vs-Offline PCM 差記録
//! (synvelis#118 crate fixture slice)。
//!
//! 実 recording 経路（MockInput → gate → staging writer → finalize →
//! `commit_session`）で commit した take PCM を RT 側実測値とし、
//! `project_comp` の `EffectiveComp` を本 file 内の決定的 offline
//! renderer（player `lowering.rs` と同一の Natural 1:1・overlap
//! double-play・`FadeWindow::gain_at` curve を再現 — 依存方向の
//! 都合で player を import しない）で評価した PCM との差を
//! feature 別に記録する。
//!
//! - identity 域（Natural segment・overlap 外）: bit-exact を要求。
//! - transition overlap: 解析 gain 値どおりの crossfade を要求し、
//!   unity double-play との差を記録する。
//! - Loop / `SimpleStretch` / Silence: lowering の skip 規則どおり
//!   無音であることを記録する（RT PCM は存在するが鳴らない差）。
//!
//! fake/mock FS 上の契約検証であり、実 Device/OS/GUI/AudioLens の
//! Pass を意味しない。

use std::{
    fs,
    path::{Path, PathBuf},
};

use synvelis_project::*;
use synvelis_recording::*;

const RATE: u32 = 48_000;
const CHANNELS: usize = 2;
/// 120 BPM・PPQN 960 → 25 frames/tick @ 48 kHz。
const FPT: i64 = 25;

// ---------- recording helpers (vs_d.rs conventions) ----------

fn controller_with(lane_count: usize) -> (ProjectController, Vec<LaneId>, ClockEpochId) {
    let mut c = ProjectController::new(
        Project::new(ProjectId::new(0), "rcd-04-comp-pcm"),
        Box::new(CounterIdGenerator::new(1)),
    );
    c.apply(Command {
        expected_revision: c.live_revision(),
        op: ProjectOp::AddTrack { name: "t".into() },
    })
    .unwrap();
    let track = *c.live().tracks.keys().next().unwrap();
    let mut lanes = Vec::new();
    for _ in 0..lane_count {
        c.apply(Command {
            expected_revision: c.live_revision(),
            op: ProjectOp::AddLane { track_id: track },
        })
        .unwrap();
        lanes.push(*c.live().lanes.keys().last().unwrap());
    }
    c.apply(Command {
        expected_revision: c.live_revision(),
        op: ProjectOp::RegisterClockEpoch {
            description: "mock".into(),
        },
    })
    .unwrap();
    let epoch = *c.live().clock_epochs.keys().next().unwrap();
    (c, lanes, epoch)
}

fn apply<O: Operation>(c: &mut ProjectController, op: O) {
    c.apply(Command {
        expected_revision: c.live_revision(),
        op,
    })
    .unwrap();
}

fn rop(c: &mut ProjectController, op: RecordingOp) {
    apply(c, SynvelisOp::Recording(op));
}

fn open_session(
    c: &mut ProjectController,
    lanes: &[LaneId],
    epoch: ClockEpochId,
) -> (RecordingSessionId, Vec<StagingStreamConfig>) {
    rop(
        c,
        RecordingOp::CreateSession {
            name: "rcd-04-session".into(),
        },
    );
    let session_id = *c.live().recording.sessions.keys().next().unwrap();
    rop(c, RecordingOp::OpenSession { session_id });
    let mut cfgs = Vec::new();
    for (i, &lane) in lanes.iter().enumerate() {
        rop(
            c,
            RecordingOp::RegisterStream {
                session_id,
                lane_id: lane,
                clock_epoch_id: epoch,
            },
        );
        let stream_id = *c.live().recording.streams.keys().last().unwrap();
        cfgs.push(StagingStreamConfig {
            stream_index: u16::try_from(i).unwrap(),
            capture_stream_id: stream_id.to_tagged(),
            lane_id: lane.to_tagged(),
            channels: u32::try_from(CHANNELS).unwrap(),
        });
    }
    (session_id, cfgs)
}

fn tmp_dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("synvelis-rcd04-{}-{name}", std::process::id()));
    if d.exists() {
        fs::remove_dir_all(&d).unwrap();
    }
    fs::create_dir_all(&d).unwrap();
    d
}

fn request(
    lanes: &[LaneId],
    epoch: ClockEpochId,
    gate: GateSpec,
    stop_tick: i64,
) -> RecordingRequest {
    RecordingRequest {
        targets: lanes
            .iter()
            .map(|&lane_id| CaptureTarget {
                lane_id,
                channels: u32::try_from(CHANNELS).unwrap(),
            })
            .collect(),
        clock_epoch: epoch,
        tempo_map: TempoMapRef { revision: 0 },
        transport_start: MusicalTick::ZERO,
        stop: MusicalTick::new(stop_tick),
        gate,
        budget: BufferBudget {
            chunks: 16,
            chunk_frames: 40_000,
            spans_per_chunk: 8,
            filled_capacity: 16,
            lookahead_spans: 32,
        },
        max_block_frames: 40_000,
    }
}

/// mock 録音 → staging → checkpoint → finalize → `CloseSession` → commit。
fn record_close_commit(
    c: &mut ProjectController,
    root: &Path,
    session_id: RecordingSessionId,
    cfgs: &[StagingStreamConfig],
    req: RecordingRequest,
    epoch: ClockEpochId,
) -> (StagingArea, CommitReport) {
    let area = StagingArea::create(
        root,
        &StagingSessionConfig {
            session_id,
            sample_rate: RATE,
            streams: cfgs.to_vec(),
        },
    )
    .unwrap();
    let plan = preflight(c.live(), req, RATE).expect("preflight");
    let a = area.clone();
    let mut sc: Scenario<StreamWriter> = Scenario::build_with(plan, move |i, ends, fault| {
        StreamWriter::create(&a, cfgs[i as usize].clone(), ends, fault).unwrap()
    });
    let mut input = MockInput::new(epoch, 0);
    sc.run_to_end(&mut input, 40_000);
    for rig in &mut sc.rigs {
        rig.writer.checkpoint().unwrap();
    }
    let tag = session_id.to_tagged();
    let writers: Vec<&mut StreamWriter> = sc.rigs.iter_mut().map(|r| &mut r.writer).collect();
    let report = finish_all(&tag, RATE, writers);
    for (_, r) in &report.refs {
        assert!(r.is_ok(), "finalize failed: {r:?}");
    }
    rop(c, RecordingOp::CloseSession { session_id });
    let report = commit_session(c, &area).expect("commit_session");
    (area, report)
}

// ---------- PCM / comp helpers ----------

/// staging media (`f32le-interleaved`) を frame×channel の f32 vec へ。
fn read_pcm(path: &Path) -> Vec<f32> {
    let bytes = fs::read(path).unwrap();
    assert_eq!(bytes.len() % 4, 0);
    let (chunks, _) = bytes.as_chunks::<4>();
    chunks.iter().map(|b| f32::from_le_bytes(*b)).collect()
}

/// interleaved buffer の frame 先頭 index（frame は常に非負の fixture 値）。
fn frame_slot(frame: i64) -> usize {
    usize::try_from(frame).unwrap() * CHANNELS
}

fn pcm_at(pcm: &[f32], frame: i64, ch: usize) -> f32 {
    pcm[frame_slot(frame) + ch]
}

/// channel 0 の出力 sample（assertion 専用の読み出し）。
fn out_at(out: &[f32], frame: i64) -> f32 {
    out[frame_slot(frame)]
}

fn mr(start: i64, end: i64) -> ProjectRange {
    ProjectRange::Musical(
        MusicalRange::try_new(MusicalTick::new(start), MusicalTick::new(end)).unwrap(),
    )
}

fn mt(t: i64) -> MusicalTick {
    MusicalTick::new(t)
}

/// `project_range` / overlap / boundary を engine frame へ
/// （lowering.rs `range_span`/`point_frame` と同一の写像）。
fn span_frames(range: &ProjectRange, map: &TickTimeMapping) -> (i64, i64) {
    let ProjectRange::Musical(r) = range else {
        panic!("fixture uses musical ranges only")
    };
    (
        map.tick_to_media(r.start(), 1, RATE).unwrap().value(),
        map.tick_to_media(r.end(), 1, RATE).unwrap().value(),
    )
}

fn point_frame(time: &ProjectTime, map: &TickTimeMapping) -> i64 {
    match time {
        ProjectTime::Musical(t) => map.tick_to_media(*t, 1, RATE).unwrap().value(),
        ProjectTime::Media(v) => v.frames_at(RATE),
    }
}

/// `FadeWindow::gain_at` と同一の gain（curve は f64 で評価し f32 へ）。
#[allow(clippy::cast_precision_loss)] // fixture の tick/frame 値は i64 精度内
#[allow(clippy::cast_possible_truncation)] // PCM は f32 — f64→f32 は意図的
fn fade_gain(shape: FadeShape, fade_out: bool, frame: i64, start: i64, end: i64) -> f32 {
    let span = end - start;
    if span <= 0 {
        return 1.0;
    }
    let t = ((frame - start) as f64 / span as f64).clamp(0.0, 1.0);
    let curve = |x: f64| match shape {
        FadeShape::Linear => x,
        FadeShape::EqualPower => (x * std::f64::consts::FRAC_PI_2).sin(),
    };
    (if fade_out { curve(1.0 - t) } else { curve(t) }) as f32
}

/// `EffectiveComp` の決定的 offline render（interleaved f32、
/// `frames` 出力 frame 数）。player lowering と同一規則:
/// Natural segment は 1:1 unity、transition は base unity +
/// 片側 overhang に fade gain、Loop/SimpleStretch/Silence は skip。
fn render_comp(
    project: &Project,
    eff: &EffectiveComp,
    pcm: &[f32],
    pcm_frames: i64,
    out_frames: usize,
) -> (Vec<f32>, Vec<String>) {
    let map = TickTimeMapping::new(&project.tempo_map).unwrap();
    let mut out = vec![0.0f32; out_frames * CHANNELS];
    let mut skips = Vec::new();
    let add = |out: &mut Vec<f32>, frame: i64, gain: f32, src: i64| {
        if src < 0 || src >= pcm_frames {
            return;
        }
        for ch in 0..CHANNELS {
            let v = pcm_at(pcm, src, ch) * gain;
            out[frame_slot(frame) + ch] += v;
        }
    };
    for seg in &eff.segments {
        let Some(binding) = &seg.binding else {
            continue; // silence
        };
        let (ms, me) = span_frames(&seg.project_range, &map);
        match binding.mapping {
            PlaybackMapping::Natural => {
                for f in ms..me {
                    add(&mut out, f, 1.0, binding.window.start().value() + (f - ms));
                }
            }
            other => skips.push(format!(
                "segment {:?} mapping {other:?} -> UnsupportedMapping (silence)",
                seg.region_id
            )),
        }
    }
    for tr in &eff.transitions {
        let (o_start, o_end) = span_frames(&tr.overlap_range, &map);
        let boundary = point_frame(&tr.boundary, &map);
        if boundary <= o_start || boundary >= o_end {
            skips.push(format!(
                "transition {:?} degenerate overlap",
                tr.transition_id
            ));
            continue;
        }
        // Left side: engine [boundary, o_end), fade_out over that span.
        if let Some(left) = &tr.left {
            if matches!(left.mapping, PlaybackMapping::Natural) {
                let read_start = left.window.end().value().saturating_sub(o_end - boundary);
                for f in boundary..o_end {
                    let g = fade_gain(tr.fade_out, true, f, boundary, o_end);
                    add(&mut out, f, g, read_start + (f - boundary));
                }
            } else {
                skips.push(format!(
                    "transition {:?} left non-Natural",
                    tr.transition_id
                ));
            }
        }
        // Right side: engine [o_start, boundary), fade_in over that span.
        if let Some(right) = &tr.right {
            if matches!(right.mapping, PlaybackMapping::Natural) {
                let read_start = right.window.start().value();
                for f in o_start..boundary {
                    let g = fade_gain(tr.fade_in, false, f, o_start, boundary);
                    add(&mut out, f, g, read_start + (f - o_start));
                }
            } else {
                skips.push(format!(
                    "transition {:?} right non-Natural",
                    tr.transition_id
                ));
            }
        }
    }
    (out, skips)
}

/// takes を pass index 昇順に並べ、(TakeId, `source_range`) を返す。
fn takes_by_pass(c: &ProjectController) -> Vec<(TakeId, SourceId, (i64, i64))> {
    let p = c.live();
    let mut takes: Vec<_> = p.recording.takes.values().collect();
    takes.sort_by_key(|t| p.recording.passes[&t.pass_id].index);
    takes
        .iter()
        .map(|t| {
            let seg = &p.recording.segments[&t.segment_ids[0]];
            (
                t.id,
                t.source_id,
                (
                    seg.source_range.start().value(),
                    seg.source_range.end().value(),
                ),
            )
        })
        .collect()
}

// ---------------------------------------------------------------------
// F3: RT↔Offline PCM 差記録
// ---------------------------------------------------------------------

#[test]
#[allow(clippy::too_many_lines)] // sequential fixture scenario
#[allow(clippy::cast_precision_loss)] // fixture の tick/frame 値は i64 精度内
#[allow(clippy::cast_possible_truncation)] // PCM は f32 — f64→f32 は意図的
fn rcd04_comp_pcm_rt_vs_offline_fixture() {
    let root = tmp_dir("comp-pcm");
    let (mut c, lanes, epoch) = controller_with(1);
    let (session_id, cfgs) = open_session(&mut c, &lanes, epoch);
    // 3-pass loop [0,3840) ticks → pass ごとに異なる input frame を
    // 捕捉した 3 take（相関素材: 同一 lane・別演奏）。
    let req = request(
        &lanes,
        epoch,
        GateSpec::Loop {
            loop_start: MusicalTick::ZERO,
            loop_end: mt(3_840),
        },
        11_520,
    );
    let (area, report) = record_close_commit(&mut c, &root, session_id, &cfgs, req, epoch);
    assert!(matches!(report.outcome, CommitOutcome::Applied { .. }));
    assert_eq!(report.takes_committed, 3);

    let pcm_path = stream_paths(&area, 0).media;
    let pcm_bytes_before = fs::read(&pcm_path).unwrap();
    let pcm = read_pcm(&pcm_path);
    let pcm_frames = i64::try_from(pcm.len() / CHANNELS).unwrap();

    let takes = takes_by_pass(&c);
    assert_eq!(takes.len(), 3);
    let (t0, src0, r0) = takes[0];
    let (t1, src1, r1) = takes[1];
    let (t2, src2, r2) = takes[2];
    // 相関素材の確認: 3 take は同一 source を異なる source_range で
    // 読み、同じ相対位置の PCM は pass ごとに異なる。
    assert_eq!(src0, src1);
    assert_eq!(src1, src2);
    assert_ne!(r0, r1);
    assert_ne!(r1, r2);
    let rel = 1_000i64;
    assert_ne!(
        pcm_at(&pcm, r0.0 + rel, 0).to_bits(),
        pcm_at(&pcm, r1.0 + rel, 0).to_bits(),
        "pass-0/pass-1 PCM must differ (correlated but distinct takes)"
    );

    // ---------- comp 構成 ----------
    let lane = lanes[0];
    apply(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: lane,
            target_range: mr(0, 7_680),
            take_ids: vec![t0, t1, t2],
            source_rate: RATE,
        },
    );
    let group = *c.live().comp.take_groups.keys().next().unwrap();
    apply(
        &mut c,
        CompOp::CreateComp {
            take_group_id: group,
        },
    );
    let comp = *c.live().comp.comps.keys().next().unwrap();

    let take_cand = |take: TakeId, off: i64| CompCandidateRef::Take {
        take_id: take,
        source_offset: SourceFrame::new(off).unwrap(),
    };
    // R1 [0,960)   Take t0  offset+8000  Natural   → frames [0,24000)
    // R2 [960,1920) Take t1  offset+8000  Natural   → [24000,48000)  T1
    //   (T1 right side needs 2400f pre-roll before play_from)
    // R3 [1920,2880) Take t2 offset+4000 Natural   → [48000,72000)  T2
    // R4 [2880,3840) ExistingSource tail Natural    → [72000,96000)
    // R5 [3840,4800) Take t0  Loop                   → skip (差記録)
    // R6 [4800,5760) Take t1  SimpleStretch(2/1)     → skip (差記録)
    // R7 [5760,6720) Silence                          → silence
    // R8 [6720,7680) Take t2 offset+16000 Natural    → [168000,192000) T3
    let regions = [
        (
            mr(0, 960),
            take_cand(t0, r0.0 + 8_000),
            PlaybackMapping::Natural,
        ),
        (
            mr(960, 1_920),
            take_cand(t1, r1.0 + 8_000),
            PlaybackMapping::Natural,
        ),
        (
            mr(1_920, 2_880),
            take_cand(t2, r2.0 + 4_000),
            PlaybackMapping::Natural,
        ),
        (
            mr(2_880, 3_840),
            CompCandidateRef::ExistingSource {
                source_id: src0,
                handle: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(pcm_frames).unwrap(),
                )
                .unwrap(),
                play_from: SourceFrame::new(pcm_frames - 30_000).unwrap(),
            },
            PlaybackMapping::Natural,
        ),
        (mr(3_840, 4_800), take_cand(t0, r0.0), PlaybackMapping::Loop),
        (
            mr(4_800, 5_760),
            take_cand(t1, r1.0),
            PlaybackMapping::SimpleStretch(StretchRatio::new(2, 1).unwrap()),
        ),
        (
            mr(5_760, 6_720),
            CompCandidateRef::Silence,
            PlaybackMapping::Natural,
        ),
        (
            mr(6_720, 7_680),
            take_cand(t2, r2.0 + 16_000),
            PlaybackMapping::Natural,
        ),
    ];
    let mut region_ids = Vec::new();
    for (range, candidate, mapping) in regions {
        apply(
            &mut c,
            CompOp::AddCompRegion {
                comp_id: comp,
                project_range: range,
                candidate,
                mapping,
            },
        );
        region_ids.push(*c.live().comp.comps[&comp].region_ids.last().unwrap());
    }
    apply(
        &mut c,
        CompOp::SetActiveComp {
            take_group_id: group,
            comp_id: Some(comp),
        },
    );
    // T1: R1/R2 boundary 960t=24000f, overlap [864,1056)t →
    //      [21600,26400)f, EqualPower out / Linear in.
    // T2: R2/R3 boundary 1920t=48000f, overlap [1800,2040)t →
    //      [45000,51000)f, Linear out / EqualPower in.
    // T3: R7(silence)/R8 boundary 6720t=168000f, overlap
    //      [6600,6840)t → [165000,171000)f, Linear/Linear,
    //      left side is silence → only right fade-in plays.
    for (l, r, ov, fo, fi) in [
        (
            region_ids[0],
            region_ids[1],
            mr(864, 1_056),
            FadeShape::EqualPower,
            FadeShape::Linear,
        ),
        (
            region_ids[1],
            region_ids[2],
            mr(1_800, 2_040),
            FadeShape::Linear,
            FadeShape::EqualPower,
        ),
        (
            region_ids[6],
            region_ids[7],
            mr(6_600, 6_840),
            FadeShape::Linear,
            FadeShape::Linear,
        ),
    ] {
        apply(
            &mut c,
            CompOp::AddCompTransition {
                left_region_id: l,
                right_region_id: r,
                overlap_range: ov,
                fade_out: fo,
                fade_in: fi,
            },
        );
    }

    // ---------- offline render + RT 比較 ----------
    let eff = project_comp(c.live(), comp).unwrap();
    assert_eq!(eff.segments.len(), 8);
    assert_eq!(eff.transitions.len(), 3);
    let out_frames = usize::try_from(7_680 * FPT).unwrap();
    let (out, skips) = render_comp(c.live(), &eff, &pcm, pcm_frames, out_frames);
    // Loop / SimpleStretch は lowering の skip 規則どおり無音。
    assert_eq!(skips.len(), 2, "{skips:?}");
    assert!(skips.iter().all(|s| s.contains("UnsupportedMapping")));

    // --- identity 域 bit-exact ---
    // R1 [0,24000): take0 window = r0.start+8000, offset f-m_start.
    let w1 = eff.segments[0].binding.as_ref().unwrap().window;
    assert_eq!(w1.start().value(), r0.0 + 8_000);
    assert_eq!(w1.end().value(), r0.0 + 8_000 + 24_000);
    // identity probes must stay outside T1's overlap [21600,26400)。
    for f in [1_000i64, 12_000, 20_000] {
        let src = w1.start().value() + f;
        assert_eq!(
            out_at(&out, f).to_bits(),
            pcm_at(&pcm, src, 0).to_bits(),
            "R1 frame {f} must equal captured PCM bit-exact"
        );
    }
    // R4 ExistingSource: window = [pcm_frames-30000, +24000)。
    let w4 = eff.segments[3].binding.as_ref().unwrap().window;
    assert_eq!(w4.start().value(), pcm_frames - 30_000);
    assert_eq!(w4.end().value(), pcm_frames - 6_000);
    for f in [73_000i64, 84_000, 95_999] {
        let src = w4.start().value() + (f - 72_000);
        assert_eq!(
            out_at(&out, f).to_bits(),
            pcm_at(&pcm, src, 0).to_bits(),
            "R4 frame {f} must equal captured PCM bit-exact"
        );
    }
    // R8 [168000,192000): take2 offset+16000。
    let w8 = eff.segments[7].binding.as_ref().unwrap().window;
    assert_eq!(w8.start().value(), r2.0 + 16_000);
    // identity probes must stay outside T3's overlap [165000,171000)。
    for f in [172_000i64, 180_000, 191_999] {
        let src = w8.start().value() + (f - 168_000);
        assert_eq!(
            out_at(&out, f).to_bits(),
            pcm_at(&pcm, src, 0).to_bits(),
            "R8 frame {f} must equal captured PCM bit-exact"
        );
    }

    // --- skipped / silence 域は無音（RT PCM との差を記録） ---
    for f in [100_000i64, 110_000, 130_000, 140_000, 150_000, 160_000] {
        assert_eq!(out_at(&out, f), 0.0, "frame {f} must be silent");
    }

    // --- transition overlap: 解析 gain との一致 + unity との差記録 ---
    // T1 right-head domain [21600,24000): left base unity + right
    // fade-in Linear。probe f=23000 → t=(23000-21600)/2400。
    let tr1 = &eff.transitions[0];
    let (o1s, o1e) = span_frames(
        &tr1.overlap_range,
        &TickTimeMapping::new(&c.live().tempo_map).unwrap(),
    );
    let b1 = point_frame(
        &tr1.boundary,
        &TickTimeMapping::new(&c.live().tempo_map).unwrap(),
    );
    assert_eq!((o1s, o1e, b1), (21_600, 26_400, 24_000));
    let f = 23_000i64;
    let t = (f - o1s) as f64 / (b1 - o1s) as f64;
    let g_in = t as f32; // Linear fade-in
    // left base: R1 window at f (segment covers up to boundary)
    let left_src = w1.start().value() + f;
    // right head: extended window start + (f - o_start)
    let rw = tr1.right.as_ref().unwrap().window;
    let right_src = rw.start().value() + (f - o1s);
    let expected = pcm_at(&pcm, left_src, 0) + pcm_at(&pcm, right_src, 0) * g_in;
    let got = out_at(&out, f);
    assert!(
        (got - expected).abs() <= f32::EPSILON * 8.0,
        "T1 right-head frame {f}: got {got} expected {expected}"
    );
    // unity double-play との差を記録（crossfade は実際に gain を掛ける）。
    let unity = pcm_at(&pcm, left_src, 0) + pcm_at(&pcm, right_src, 0);
    assert_ne!(got.to_bits(), unity.to_bits(), "fade must modify the mix");

    // T1 left-tail domain [24000,26400): right base unity + left
    // fade-out EqualPower。probe f=25200 → t=0.5 → sin(π/4)。
    let f = 25_200i64;
    let t = (f - b1) as f64 / (o1e - b1) as f64;
    let g_out = (std::f64::consts::FRAC_PI_2 * (1.0 - t)).sin() as f32;
    assert!((g_out - std::f32::consts::FRAC_1_SQRT_2).abs() < 1e-6);
    let right_base_src = tr1
        .right
        .as_ref()
        .map(|_| {
            eff.segments[1]
                .binding
                .as_ref()
                .unwrap()
                .window
                .start()
                .value()
                + (f - b1)
        })
        .unwrap();
    let lw = tr1.left.as_ref().unwrap().window;
    let left_tail_src = lw.end().value() - (o1e - f);
    let expected = pcm_at(&pcm, right_base_src, 0) + pcm_at(&pcm, left_tail_src, 0) * g_out;
    let got = out_at(&out, f);
    assert!(
        (got - expected).abs() <= f32::EPSILON * 8.0,
        "T1 left-tail frame {f}: got {got} expected {expected}"
    );

    // boundary frame: 両側 unity の bounded double-play 点を記録。
    // left tail read_start = window.end - (o_end - boundary)、
    // right base は window.start + (boundary - boundary)。
    let lw = tr1.left.as_ref().unwrap().window;
    let left_at_boundary = lw.end().value() - (o1e - b1);
    let right_at_boundary = eff.segments[1]
        .binding
        .as_ref()
        .unwrap()
        .window
        .start()
        .value();
    let boundary_mix = out_at(&out, b1);
    let expected_boundary = pcm_at(&pcm, left_at_boundary, 0) + pcm_at(&pcm, right_at_boundary, 0);
    assert!(
        (boundary_mix - expected_boundary).abs() <= f32::EPSILON * 8.0,
        "boundary frame must be the documented double-play point"
    );

    // T3: left=silence → overlap では right fade-in のみ。
    let tr3 = &eff.transitions[2];
    assert!(tr3.left.is_none(), "silence side has no binding");
    let f = 167_000i64; // inside [165000,168000) right-head domain
    let rw3 = tr3.right.as_ref().unwrap().window;
    let src3 = rw3.start().value() + (f - 165_000);
    let g3 = fade_gain(tr3.fade_in, false, f, 165_000, 168_000);
    let expected3 = pcm_at(&pcm, src3, 0) * g3;
    let got3 = out_at(&out, f);
    assert!(
        (got3 - expected3).abs() <= f32::EPSILON * 8.0,
        "T3 silence-side frame {f}: got {got3} expected {expected3}"
    );

    // --- Beat Anchor/Tempo 記録 ---
    // 120 BPM・PPQN 960・25 f/tick: region 960t → 24000f、
    // overlap 192t → 4800f。tempo_map revision 0・ppqn 960。
    let tm = &c.live().tempo_map;
    assert_eq!(tm.revision, 0);
    assert_eq!(tm.ppqn, 960);
    assert_eq!(FPT * 960, 24_000);
    let group = &c.live().comp.take_groups[&group_id(&c)];
    assert_eq!(group.source_rate, RATE);

    // --- raw PCM / Take / Source / Asset 不変 ---
    assert_eq!(
        fs::read(&pcm_path).unwrap(),
        pcm_bytes_before,
        "comp ops + render must not touch captured PCM"
    );
    let p = c.live();
    for t in p.recording.takes.values() {
        assert_eq!(t.state, TakeState::Verified);
    }
    assert_eq!(p.sources.len(), 1);
    assert_eq!(p.assets.len(), 1);

    // --- determinism ---
    let eff2 = project_comp(c.live(), comp).unwrap();
    assert_eq!(eff, eff2);
    let (out2, skips2) = render_comp(c.live(), &eff2, &pcm, pcm_frames, out_frames);
    assert_eq!(out, out2);
    assert_eq!(skips, skips2);

    fs::remove_dir_all(&root).ok();
}

fn group_id(c: &ProjectController) -> TakeGroupId {
    *c.live().comp.take_groups.keys().next().unwrap()
}
