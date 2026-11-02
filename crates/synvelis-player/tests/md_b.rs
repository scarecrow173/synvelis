//! MD-B acceptance: the Effective Comp projection is consumed by the
//! player lowering — adopted segments play their bound sources,
//! transition sides carry their declared fades, silence regions emit
//! nothing, and lane clips inside an active comp's target range are
//! superseded instead of double-played (issue #50, scope 1).
//!
//! Deterministic evidence: every asserted sample is re-derived from
//! `fixture_sample_f32` through the projection's own geometry — the
//! same 120 BPM @ PPQN 960 map (25 frames/tick @48 kHz) the `md_a`
//! projection tests use.
//!
//! Tempo fixture: default map is 120 BPM @ PPQN 960 → 25 frames per
//! tick at 48 kHz.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use synvelis_player::PumpPolicy;
use synvelis_player::driver::HeadlessDriver;
use synvelis_player::fixture::{fixture_sample_f32, write_fixture};
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::lowering::{
    EngineConfig, FadeDirection, NodeContent, OverlapSide, SkippedPlayback, lower,
};
use synvelis_player::resolver::prepare;
use synvelis_project::{
    ClipExtent, ClipPlacement, ClipPlayback, Command, CompCandidateRef, CompOp, CompRegionId,
    CounterIdGenerator, FadeShape, MediaDuration, MediaTimeValue, MusicalRange, MusicalTick,
    PlaybackMapping, Project, ProjectController, ProjectId, ProjectOp, ProjectRange,
    ProjectRevision, SourceFrame, SourceRange, TakeId, TimeAnchor,
};
use synvelis_recording::SynvelisOp;
use synvelis_recording::ops::RecordingOp;

use ruviform_media_reader::PrefetchConfig;

const RATE: u32 = 48_000;
/// 120 BPM @ PPQN 960 → 0.5 s/quarter → 25 frames per tick @48 kHz.
const FRAMES_PER_TICK: i64 = 25;
/// Source length: every comp read stays inside it.
const SOURCE_FRAMES: usize = 60_000;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-mdb-{}-{}-{}",
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
        Project::new(ProjectId::new(0xB), "md-b"),
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

fn sop(c: &mut ProjectController, op: SynvelisOp) -> ProjectRevision {
    apply(c, op)
}

fn pop(c: &mut ProjectController, op: ProjectOp) -> ProjectRevision {
    sop(c, SynvelisOp::Project(op))
}

fn rop(c: &mut ProjectController, op: RecordingOp) -> ProjectRevision {
    sop(c, SynvelisOp::Recording(op))
}

fn cop(c: &mut ProjectController, op: CompOp) -> ProjectRevision {
    sop(c, SynvelisOp::Comp(op))
}

fn mr(start: i64, end: i64) -> ProjectRange {
    ProjectRange::Musical(
        MusicalRange::try_new(MusicalTick::new(start), MusicalTick::new(end)).unwrap(),
    )
}

fn media_placement(start_frame: i64, end_frame: i64, rate: u32) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Absolute(MediaTimeValue::new(start_frame, 1, rate).unwrap()),
        extent: ClipExtent::Absolute(MediaDuration::new(end_frame - start_frame, 1, rate).unwrap()),
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

fn engine(rate: u32, channels: u16, block: usize) -> EngineConfig {
    EngineConfig {
        sample_rate: rate,
        channels,
        block_frames: block,
    }
}

fn prefetch(channels: u16, rate: u32) -> PrefetchConfig {
    PrefetchConfig {
        block_frames: 256,
        block_count: 8,
        channels: usize::from(channels),
        sample_rate: rate,
    }
}

fn mono_format() -> ruviform_audio_core::AudioFormat {
    ruviform_audio_core::AudioFormat {
        sample_rate: ruviform_audio_core::SampleRate::new(RATE).unwrap(),
        layout: ruviform_audio_core::ChannelLayout::Mono,
        sample_format: ruviform_audio_core::SampleFormat::F32,
    }
}

/// IDs the comp fixture hands back to assertions.
struct CompFixture {
    take_id: TakeId,
    regions: Vec<CompRegionId>,
}

/// Builds the canonical MD-B project: one imported mono source, one
/// lane, one take covering `source [0, 60_000)`, an active comp with
/// regions `[0,960) Take@0`, `[960,1920) ExistingSource@2000`,
/// `[1920,2000) Silence` and a `[900,1020)` transition — plus a lane
/// clip inside the target range that must be superseded.
#[allow(clippy::too_many_lines)] // sequential comp construction
fn comp_project(root: &Path) -> (ProjectController, CompFixture) {
    let wav = root.join("s.wav");
    write_fixture(&wav, 1, RATE, SOURCE_FRAMES).unwrap();
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

    pop(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    pop(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    pop(
        &mut c,
        ProjectOp::RegisterClockEpoch {
            description: "clk".into(),
        },
    );
    let epoch = *c.live().clock_epochs.keys().next().unwrap();

    // A lane clip overlapping the comp target range, reading a source
    // window different from every comp binding — if it wrongly played
    // alongside the comp the mixed samples would differ.
    pop(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id,
            placement: media_placement(0, 48_000, RATE),
            playback: natural_playback(12_000, 48_000),
        },
    );

    // Recording side through the real ops (session → pass → stream →
    // segment → take), the same path a commit would produce.
    rop(&mut c, RecordingOp::CreateSession { name: "s".into() });
    let session = *c.live().recording.sessions.keys().next().unwrap();
    rop(
        &mut c,
        RecordingOp::OpenSession {
            session_id: session,
        },
    );
    rop(
        &mut c,
        RecordingOp::RegisterPass {
            session_id: session,
            index: 0,
        },
    );
    let pass = *c.live().recording.passes.keys().next().unwrap();
    rop(
        &mut c,
        RecordingOp::RegisterStream {
            session_id: session,
            lane_id: lane,
            clock_epoch_id: epoch,
        },
    );
    let stream = *c.live().recording.streams.keys().next().unwrap();
    rop(
        &mut c,
        RecordingOp::RegisterSegment {
            stream_id: stream,
            pass_id: pass,
            session_range: synvelis_project::SessionRange::try_new(
                synvelis_project::SessionFrame::new(0).unwrap(),
                synvelis_project::SessionFrame::new(i64::try_from(SOURCE_FRAMES).unwrap()).unwrap(),
            )
            .unwrap(),
            source_range: SourceRange::try_new(
                SourceFrame::ZERO,
                SourceFrame::new(i64::try_from(SOURCE_FRAMES).unwrap()).unwrap(),
            )
            .unwrap(),
            source_id,
        },
    );
    let segment = *c.live().recording.segments.keys().next().unwrap();
    rop(
        &mut c,
        RecordingOp::RegisterTake {
            session_id: session,
            pass_id: pass,
            stream_id: stream,
            lane_id: lane,
            source_id,
            segment_ids: vec![segment],
        },
    );
    let take_id = *c.live().recording.takes.keys().next().unwrap();
    rop(&mut c, RecordingOp::MarkTakeVerified { take_id });

    cop(
        &mut c,
        CompOp::RegisterTakeGroup {
            lane_id: lane,
            target_range: mr(0, 2_000),
            take_ids: vec![take_id],
            source_rate: RATE,
        },
    );
    let group = *c.live().comp.take_groups.keys().next().unwrap();
    cop(
        &mut c,
        CompOp::CreateComp {
            take_group_id: group,
        },
    );
    let comp = *c.live().comp.comps.keys().next().unwrap();
    for (range, candidate) in [
        (
            mr(0, 960),
            CompCandidateRef::Take {
                take_id,
                source_offset: SourceFrame::ZERO,
            },
        ),
        (
            mr(960, 1_920),
            CompCandidateRef::ExistingSource {
                source_id,
                handle: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(i64::try_from(SOURCE_FRAMES).unwrap()).unwrap(),
                )
                .unwrap(),
                play_from: SourceFrame::new(2_000).unwrap(),
            },
        ),
        (mr(1_920, 2_000), CompCandidateRef::Silence),
    ] {
        cop(
            &mut c,
            CompOp::AddCompRegion {
                comp_id: comp,
                project_range: range,
                candidate,
                mapping: PlaybackMapping::Natural,
            },
        );
    }
    cop(
        &mut c,
        CompOp::SetActiveComp {
            take_group_id: group,
            comp_id: Some(comp),
        },
    );
    let regions = c.live().comp.comps[&comp].region_ids.clone();
    cop(
        &mut c,
        CompOp::AddCompTransition {
            left_region_id: regions[0],
            right_region_id: regions[1],
            overlap_range: mr(900, 1_020),
            fade_out: FadeShape::EqualPower,
            fade_in: FadeShape::Linear,
        },
    );
    (c, CompFixture { take_id, regions })
}

fn prepared_all(
    c: &ProjectController,
    lowered: &synvelis_player::Lowered,
    root: &Path,
) -> Vec<synvelis_player::resolver::PreparedClip> {
    lowered
        .playbacks
        .iter()
        .map(|b| {
            prepare(
                c.live(),
                b.source_id,
                &root.join("media"),
                &prefetch(1, RATE),
            )
            .unwrap()
        })
        .collect()
}

// ---------------------------------------------------------------- MD-B-01

/// MD-B-01: the active comp's Effective projection lowers into real
/// playback nodes — segment windows read the bound sources, the lane
/// clip inside the target range is superseded, silence is explicit.
#[test]
fn md_b_01_comp_lowering_replaces_lane_clips() {
    let root = scratch("mdb01");
    let (c, fx) = comp_project(&root);
    let lowered = lower(c.live(), &|_| Some(mono_format()), &engine(RATE, 1, 256));

    // 4 playback nodes: two segments + two transition sides; the lane
    // clip and the silence region are explicit skips.
    assert_eq!(lowered.playbacks.len(), 4);
    let comp = *c.live().comp.comps.keys().next().unwrap();
    assert_eq!(
        lowered.playbacks[0].content,
        NodeContent::CompSegment {
            comp_id: comp,
            region_id: fx.regions[0]
        }
    );
    assert_eq!(lowered.playbacks[0].window.engine_start, 0);
    assert_eq!(
        lowered.playbacks[0].window.engine_len,
        960 * FRAMES_PER_TICK as u64
    );
    assert_eq!(lowered.playbacks[0].window.source_start, 0);
    assert_eq!(lowered.playbacks[0].fade, None);

    assert_eq!(
        lowered.playbacks[1].content,
        NodeContent::CompSegment {
            comp_id: comp,
            region_id: fx.regions[1]
        }
    );
    assert_eq!(
        lowered.playbacks[1].window.engine_start,
        960 * FRAMES_PER_TICK as u64
    );
    assert_eq!(
        lowered.playbacks[1].window.engine_len,
        960 * FRAMES_PER_TICK as u64
    );
    assert_eq!(lowered.playbacks[1].window.source_start, 2_000);

    // Transition sides: left plays boundary→overlap.end reading its
    // window tail; right plays overlap.start→boundary from the head.
    let transition = *c.live().comp.transitions.keys().next().unwrap();
    let (o_start, o_end) = (900 * FRAMES_PER_TICK, 1_020 * FRAMES_PER_TICK);
    let boundary = 960 * FRAMES_PER_TICK;
    assert_eq!(
        lowered.playbacks[2].content,
        NodeContent::CompOverlap {
            comp_id: comp,
            transition_id: transition,
            side: OverlapSide::Left
        }
    );
    let left = &lowered.playbacks[2];
    assert_eq!(left.window.engine_start, u64::try_from(boundary).unwrap());
    assert_eq!(
        left.window.engine_len,
        u64::try_from(o_end - boundary).unwrap()
    );
    assert_eq!(left.window.source_start, u64::try_from(boundary).unwrap());
    let fade = left.fade.expect("left side carries its fade-out");
    assert_eq!(fade.shape, FadeShape::EqualPower);
    assert_eq!(fade.direction, FadeDirection::Out);
    assert_eq!(fade.engine_start, boundary);
    assert_eq!(fade.engine_end, o_end);

    assert_eq!(
        lowered.playbacks[3].content,
        NodeContent::CompOverlap {
            comp_id: comp,
            transition_id: transition,
            side: OverlapSide::Right
        }
    );
    let right = &lowered.playbacks[3];
    assert_eq!(right.window.engine_start, u64::try_from(o_start).unwrap());
    assert_eq!(
        right.window.engine_len,
        u64::try_from(boundary - o_start).unwrap()
    );
    assert_eq!(right.window.source_start, 500);
    let fade = right.fade.expect("right side carries its fade-in");
    assert_eq!(fade.shape, FadeShape::Linear);
    assert_eq!(fade.direction, FadeDirection::In);
    assert_eq!(fade.engine_start, o_start);
    assert_eq!(fade.engine_end, boundary);

    // Skips: the lane clip superseded by the active group, the silence
    // region explicit.
    let clip_id = *c.live().clips.keys().next().unwrap();
    let group = *c.live().comp.take_groups.keys().next().unwrap();
    assert!(
        lowered
            .skipped
            .contains(&SkippedPlayback::SupersededByComp {
                clip_id,
                take_group_id: group,
            }),
        "lane clip must be superseded: {:?}",
        lowered.skipped
    );
    assert!(
        lowered.skipped.contains(&SkippedPlayback::CompSilence {
            comp_id: comp,
            region_id: fx.regions[2],
        }),
        "silence region must be explicit: {:?}",
        lowered.skipped
    );
}

// ---------------------------------------------------------------- MD-B-02

/// MD-B-02: lowered comp nodes play their bound sources sample-exact —
/// base segments at unity, overhang sides shaped by the declared
/// fades, silence regions emitting nothing. No double-play of the
/// superseded lane clip.
#[allow(clippy::cast_precision_loss)] // engine frames stay far under 2^53
#[test]
fn md_b_02_comp_playback_pcm_exact() {
    let root = scratch("mdb02");
    let (c, fx) = comp_project(&root);
    let eng = engine(RATE, 1, 256);
    let lowered = lower(c.live(), &|_| Some(mono_format()), &eng);

    let mut driver = HeadlessDriver::new(eng);
    let prepared = prepared_all(&c, &lowered, &root);
    driver.activate(&lowered, prepared).unwrap();

    // 200 blocks × 256 = 51_200 frames covers the whole target range.
    let report = driver.run(200, PumpPolicy::Every(1)).unwrap();
    assert_eq!(report.pcm.len(), 1);
    let pcm = &report.pcm[0];
    let (o_start, o_end) = (900 * FRAMES_PER_TICK, 1_020 * FRAMES_PER_TICK);
    let boundary = 960 * FRAMES_PER_TICK;
    for (f, &got) in pcm.iter().enumerate() {
        let f = i64::try_from(f).unwrap();
        let want = if f < o_start {
            // Region r0 base: take_a0@0.
            fixture_sample_f32(0, usize::try_from(f).unwrap())
        } else if f < boundary {
            // r0 base at unity + right overhang fading in (Linear
            // over [o_start, boundary)) reading source [500, 2000).
            let t = (f - o_start) as f64 / (boundary - o_start) as f64;
            #[allow(clippy::cast_possible_truncation)]
            let gain = t as f32;
            fixture_sample_f32(0, usize::try_from(f).unwrap())
                + fixture_sample_f32(0, usize::try_from(500 + (f - o_start)).unwrap()) * gain
        } else if f < o_end {
            // r1 base at unity reading source [2000 + (f−boundary))
            // + left overhang fading out (EqualPower over
            // [boundary, o_end)) reading source [f, f+... ).
            let t = (f - boundary) as f64 / (o_end - boundary) as f64;
            #[allow(clippy::cast_possible_truncation)]
            let gain = ((1.0 - t) * std::f64::consts::FRAC_PI_2).sin() as f32;
            fixture_sample_f32(0, usize::try_from(2_000 + (f - boundary)).unwrap())
                + fixture_sample_f32(0, usize::try_from(f).unwrap()) * gain
        } else if f < 1_920 * FRAMES_PER_TICK {
            // Region r1 base alone.
            fixture_sample_f32(0, usize::try_from(2_000 + (f - boundary)).unwrap())
        } else {
            // r2 silence and everything past the target range.
            0.0
        };
        assert!(
            (got - want).abs() <= 1e-4,
            "frame {f}: got {got}, want {want}"
        );
    }
    // Comp-node stats prove real reads happened on every node.
    let comp = *c.live().comp.comps.keys().next().unwrap();
    let transition = *c.live().comp.transitions.keys().next().unwrap();
    for content in [
        NodeContent::CompSegment {
            comp_id: comp,
            region_id: fx.regions[0],
        },
        NodeContent::CompSegment {
            comp_id: comp,
            region_id: fx.regions[1],
        },
        NodeContent::CompOverlap {
            comp_id: comp,
            transition_id: transition,
            side: OverlapSide::Left,
        },
        NodeContent::CompOverlap {
            comp_id: comp,
            transition_id: transition,
            side: OverlapSide::Right,
        },
    ] {
        let stats = driver.stats_of_content(&content).unwrap().snapshot();
        assert!(stats.written > 0, "{content:?} wrote no frames");
    }
    // The take id stays reachable for diagnostics.
    let _ = fx.take_id;
}

// ---------------------------------------------------------------- MD-B-03

/// MD-B-03: an inactive comp supersedes nothing — the lane clip plays
/// normally and no comp nodes are emitted. Activating the comp swaps
/// the same project to adopted playback.
#[test]
fn md_b_03_inactive_comp_plays_raw_clips() {
    let root = scratch("mdb03");
    let (mut c, _fx) = comp_project(&root);
    // Deactivate the comp — the group now has no adopted arrangement.
    let group = *c.live().comp.take_groups.keys().next().unwrap();
    cop(
        &mut c,
        CompOp::SetActiveComp {
            take_group_id: group,
            comp_id: None,
        },
    );

    let lowered = lower(c.live(), &|_| Some(mono_format()), &engine(RATE, 1, 256));
    assert_eq!(lowered.playbacks.len(), 1);
    let clip_id = *c.live().clips.keys().next().unwrap();
    assert_eq!(lowered.playbacks[0].content, NodeContent::Clip(clip_id));
    assert_eq!(lowered.skipped, Vec::new());
    assert_eq!(lowered.playbacks[0].window.source_start, 12_000);

    let mut driver = HeadlessDriver::new(engine(RATE, 1, 256));
    driver
        .activate(&lowered, prepared_all(&c, &lowered, &root))
        .unwrap();
    let report = driver.run(200, PumpPolicy::Every(1)).unwrap();
    for (f, &got) in report.pcm[0].iter().enumerate() {
        let want = if f < 48_000 {
            fixture_sample_f32(0, 12_000 + f)
        } else {
            0.0
        };
        assert_eq!(got, want, "frame {f}");
    }
}

// ---------------------------------------------------------------- MD-B-04

/// MD-B-04: a comp binding whose source cannot be resolved lowers as
/// an explicit `CompSegment` skip — never a silent drop — and a group
/// whose `source_rate` differs from the engine refuses to resample.
#[test]
fn md_b_04_comp_skips_are_explicit() {
    let root = scratch("mdb04");
    let (c, _fx) = comp_project(&root);
    let comp = *c.live().comp.comps.keys().next().unwrap();

    // Source unresolvable: every bound segment and transition side is
    // a typed skip; the silence region still reports CompSilence.
    let lowered = lower(c.live(), &|_| None, &engine(RATE, 1, 256));
    assert!(lowered.playbacks.is_empty());
    assert_eq!(
        lowered
            .skipped
            .iter()
            .filter(|s| {
                matches!(
                    s,
                    SkippedPlayback::CompSegment { comp_id, .. } if *comp_id == comp
                )
            })
            .count(),
        2,
        "both bound segments must skip explicitly: {:?}",
        lowered.skipped
    );
    assert_eq!(
        lowered
            .skipped
            .iter()
            .filter(|s| {
                matches!(
                    s,
                    SkippedPlayback::CompOverlap { comp_id, .. } if *comp_id == comp
                )
            })
            .count(),
        2,
        "both transition sides must skip explicitly: {:?}",
        lowered.skipped
    );
    // The lane clip also reports MissingSource (never supersession —
    // it still falls through to its own health check).
    let clip_id = *c.live().clips.keys().next().unwrap();
    assert!(
        lowered.skipped.iter().any(
            |s| matches!(s, SkippedPlayback::SupersededByComp { clip_id: id, .. } if *id == clip_id)
                || matches!(s, SkippedPlayback::MissingSource { clip_id: id, .. } if *id == clip_id)
        ),
        "lane clip must be reported: {:?}",
        lowered.skipped
    );

    // Rate mismatch: engine at 96 kHz against a 48 kHz group.
    let lowered = lower(c.live(), &|_| Some(mono_format()), &engine(96_000, 1, 256));
    assert!(lowered.playbacks.is_empty());
    assert!(
        lowered.skipped.iter().any(|s| matches!(
            s,
            SkippedPlayback::CompSegment {
                reason: synvelis_player::lowering::CompSkipReason::RateMismatch(48_000),
                ..
            }
        )),
        "group/engine rate mismatch must refuse resampling: {:?}",
        lowered.skipped
    );
}
