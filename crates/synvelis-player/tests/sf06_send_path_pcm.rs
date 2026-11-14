//! SF-06 cross-gate headless fixtures (Issue #109): PCM-level oracles
//! for the send/bus path that the SF-01..05 slice tests verified only
//! at the lowering/source-map layer.
//!
//! - `sf06_send_pcm_pre_vs_post_tap` — a `PreFader` send carries the
//!   post-chain/pre-fader strip signal while a `PostFader` send
//!   carries the fader output; with the strip fader at 0.5 the two
//!   taps are measurably different at the master output, and the
//!   rendered PCM matches the independently derived sum exactly.
//! - `sf06_graph_edit_pcm_undo_save_reload` — a `RoutingEditIntent`
//!   commits bus+send+return wiring as one revision/one undo; master
//!   PCM matches the analytic golden before and after the edit, a
//!   save→load round-trip preserves project and PCM, and a single
//!   undo restores the pre-edit PCM bit-for-bit.
//! - `sf06_mute_solo_pcm_through_sends` — muting a strip silences its
//!   main *and* send contributions at the master (measured zeros,
//!   not unobserved), and soloing one strip removes every other
//!   strip's main+send contribution while the soloed strip still
//!   sounds through both paths.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use synvelis_player::driver::PumpPolicy;
use synvelis_player::fixture::{fixture_sample_f32, write_fixture};
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::lowering::EngineConfig;
use synvelis_player::resolver::prepare;
use synvelis_player::routing_engine::{RoutingDeps, RoutingEngine};
use synvelis_player::{ImportOutcome, PlayReport};
use synvelis_project::{
    BuildBudget, BuildMode, BusKind, BusRef, ClipExtent, ClipPlacement, ClipPlayback, Command,
    ConnectionKind, ControlOwner, CounterIdGenerator, ExplicitConversionPolicy, InMemoryStore,
    MediaDuration, MediaTimeValue, ParameterRole, PlaybackMapping, PortDirection, PortRef, Project,
    ProjectController, ProjectId, ProjectOp, ProjectRevision, RoutingBuildRequestId,
    RoutingCompileRequest, RoutingEdit, RoutingEditIntent, RuntimeClockEpochRef, RuntimeInstanceId,
    SendGainSpec, SendTap, SourceFrame, SourceId, SourceRange, StripId, StripRef, TempBusId,
    TempConnectionId, TempSendId, TimeAnchor, port_capability_generation,
};

use ruviform_audio_core::{AudioFormat, ChannelLayout, SampleFormat, SampleRate};
use ruviform_media_reader::PrefetchConfig;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

const CLIP_FRAMES: usize = 480;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sf06pcm-{}-{}-{}",
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
        Project::new(ProjectId::new(0x109), "sf06"),
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

fn engine() -> EngineConfig {
    EngineConfig {
        sample_rate: 48_000,
        channels: 2,
        block_frames: 256,
    }
}

fn canonical_format() -> AudioFormat {
    AudioFormat {
        sample_rate: SampleRate::new(48_000).unwrap(),
        layout: ChannelLayout::Stereo,
        sample_format: SampleFormat::F32,
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

fn new_engine() -> RoutingEngine {
    RoutingEngine::new(engine(), RuntimeInstanceId(1), RuntimeClockEpochRef(1))
}

/// Imports a stereo fixture and returns the commit outcome.
fn import_stereo(
    c: &mut ProjectController,
    root: &Path,
    name: &str,
    frames: usize,
) -> ImportOutcome {
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

/// Adds a track, gives it a strip (auto-wired to master), adds a lane
/// and returns (track, lane).
fn add_track_strip(
    c: &mut ProjectController,
) -> (synvelis_project::TrackId, synvelis_project::LaneId) {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next_back().unwrap();
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    apply(c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().tracks[&track].lane_ids.last().unwrap();
    (track, lane)
}

fn add_clip(
    c: &mut ProjectController,
    lane: synvelis_project::LaneId,
    source: SourceId,
    start: u64,
    end: u64,
    offset: i64,
) -> synvelis_project::ClipId {
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

fn strip_of_track(project: &Project, track: synvelis_project::TrackId) -> StripId {
    project.routing.strip_of_track(track).map(|s| s.id).unwrap()
}

fn master_strip(project: &Project) -> StripId {
    project.routing.buses[&project.routing.master].strip
}

/// The strip's seeded `Fader` parameter target — the persisted base
/// the lowering bakes into the strip's post-fader gain.
fn strip_fader_target(
    c: &ProjectController,
    strip: StripId,
) -> synvelis_project::ParameterTargetId {
    *c.live()
        .control
        .targets
        .keys()
        .find(|id| {
            let t = &c.live().control.targets[*id];
            t.owner == ControlOwner::Strip(strip) && t.role == ParameterRole::Fader
        })
        .unwrap()
}

// ------------------------------------------------------------- intents

fn intent(c: &ProjectController, edits: Vec<RoutingEdit>) -> RoutingEditIntent {
    RoutingEditIntent {
        operation_id: "sf06".into(),
        expected_port_capability_generation: port_capability_generation(c.live()),
        edits,
        approved_conversion: ExplicitConversionPolicy::None,
    }
}

fn apply_intent(c: &mut ProjectController, edits: Vec<RoutingEdit>) -> ProjectRevision {
    apply(
        c,
        ProjectOp::Routing {
            intent: intent(c, edits),
        },
    )
}

fn strip_main(strip: StripId, direction: PortDirection) -> PortRef {
    PortRef::StripMain {
        strip: StripRef::Existing(strip),
        direction,
    }
}

fn bus_main(bus: BusRef, direction: PortDirection) -> PortRef {
    PortRef::StripMain {
        strip: StripRef::OfBus(bus),
        direction,
    }
}

/// `CreateBus` + bus→master `Main` connection + one `CreateSend`
/// per `(strip, tap, gain)` — the whole send/return wiring commits
/// as a single revision inside one intent.
fn wire_sends(
    c: &mut ProjectController,
    master: StripId,
    sends: &[(StripId, SendTap, f64)],
) -> ProjectRevision {
    let temp_bus = TempBusId(1);
    let mut edits = vec![
        RoutingEdit::CreateBus {
            temp_id: temp_bus,
            kind: BusKind::Return,
            name: "verb".into(),
        },
        RoutingEdit::CreateConnection {
            temp_id: TempConnectionId(1),
            source: bus_main(BusRef::Temp(temp_bus), PortDirection::Output),
            destination: strip_main(master, PortDirection::Input),
            kind: ConnectionKind::Main,
            channel_map: None,
        },
    ];
    for (i, (strip, tap, gain)) in sends.iter().enumerate() {
        edits.push(RoutingEdit::CreateSend {
            temp_id: TempSendId(u32::try_from(i + 1).unwrap()),
            temp_connection: TempConnectionId(u32::try_from(i + 2).unwrap()),
            source: strip_main(*strip, PortDirection::Output),
            destination: bus_main(BusRef::Temp(temp_bus), PortDirection::Input),
            tap: *tap,
            level: SendGainSpec::Create {
                name: format!("send{i}"),
                initial: *gain,
            },
            channel_map: None,
        });
    }
    apply_intent(c, edits)
}

// ------------------------------------------------------------- render

/// Fresh-engine prepare→publish→run — every render is an independent
/// `Configured→Prepared→Published→Active` cycle so PCM comparisons
/// never carry engine state across revisions.
fn render(c: &mut ProjectController, media: &Path, request_id: u64, blocks: usize) -> PlayReport {
    let mut eng = new_engine();
    let revision = c.live_revision();
    let canonical = |_: SourceId| Some(canonical_format());
    let deps = RoutingDeps {
        canonical_of: &canonical,
        clips: c
            .live()
            .clips
            .values()
            .map(|clip| prepare(c.live(), clip.source_id, media, &prefetch()).unwrap())
            .collect(),
        engine_config_generation: 1,
        source_format_generation: 1,
    };
    let prepared = eng
        .prepare(&request(request_id, revision), c.live(), revision, deps)
        .unwrap()
        .expect_built();
    eng.publish(prepared, c.live(), revision).unwrap();
    eng.run(blocks, PumpPolicy::Every(1)).unwrap()
}

/// The clip used by every fixture: engine frames `[0, CLIP_FRAMES)`
/// carry `fixture_sample_f32(ch, f)`; everything after is silence.
fn clip_source_frame(engine_frame: usize) -> Option<usize> {
    (engine_frame < CLIP_FRAMES).then_some(engine_frame)
}

/// Asserts every rendered sample equals `gain ·` the fixture sample
/// inside the clip window and exact `0.0` outside it. All gains used
/// here are dyadic, so the expectation is bit-exact.
fn assert_golden(pcm: &[Vec<f32>], gain: f32, label: &str) {
    for (ch, plane) in pcm.iter().enumerate() {
        for (f, s) in plane.iter().enumerate() {
            let expected =
                clip_source_frame(f).map_or(0.0, |src| fixture_sample_f32(ch, src) * gain);
            assert_eq!(*s, expected, "{label}: ch{ch} frame{f}");
        }
    }
}

/// Two-clip variant: clip A covers engine `[0, CLIP_FRAMES)` reading
/// source `[0, CLIP_FRAMES)`; clip B covers `[CLIP_FRAMES, 2·CLIP)`
/// reading the same source range — `src` is the source frame.
fn assert_two_clip_golden(pcm: &[Vec<f32>], gain_a: f32, gain_b: f32, label: &str) {
    for (ch, plane) in pcm.iter().enumerate() {
        for (f, s) in plane.iter().enumerate() {
            let expected = if f < CLIP_FRAMES {
                fixture_sample_f32(ch, f) * gain_a
            } else if f < 2 * CLIP_FRAMES {
                fixture_sample_f32(ch, f - CLIP_FRAMES) * gain_b
            } else {
                0.0
            };
            assert_eq!(*s, expected, "{label}: ch{ch} frame{f}");
        }
    }
}

// ---------------------------------------------------------------- tests

/// SF-ACT-02 / SF-VALID-03/04: pre- vs post-fader send taps carry
/// different signals once the fader is not unity, the send gain
/// stages apply their persisted levels, and the bus→master `Main`
/// edge sums everything at the master — all bit-exact against the
/// analytic oracle.
///
/// Topology: track clip → strip fader 0.5 → master (0.5·x);
/// pre-fader send 0.5 → return bus (1.0·x·0.5 = 0.5·x);
/// post-fader send 0.25 → return bus (0.5·x·0.25 = 0.125·x);
/// bus → master. Master = 0.5x + 0.5x + 0.125x = 1.125·x.
/// If the pre tap wrongly carried the post-fader signal the total
/// would be 0.875·x — the oracle discriminates the tap points.
#[test]
fn sf06_send_pcm_pre_vs_post_tap() {
    let root = scratch("tap");
    let mut c = controller();
    let out = import_stereo(&mut c, &root, "a.wav", 4_800);
    let (track, lane) = add_track_strip(&mut c);
    add_clip(
        &mut c,
        lane,
        out.commit.source_id,
        0,
        u64::try_from(CLIP_FRAMES).unwrap(),
        0,
    );
    let strip = strip_of_track(c.live(), track);
    let fader = strip_fader_target(&c, strip);
    apply(
        &mut c,
        ProjectOp::SetParameterBase {
            target: fader,
            value: 0.5,
        },
    );

    let master = master_strip(c.live());
    wire_sends(
        &mut c,
        master,
        &[
            (strip, SendTap::PreFader, 0.5),
            (strip, SendTap::PostFader, 0.25),
        ],
    );

    // 3 blocks = 768 frames: the 480-frame clip window plus tail
    // silence.
    let report = render(&mut c, &root.join("media"), 1, 3);
    assert_golden(&report.pcm, 1.125, "pre/post send taps");
}

/// SF-MODEL / SF-ACT-02: a routing edit's PCM is the authoritative
/// projection of the committed project — identical before/after a
/// save→load round-trip, and a single undo restores the pre-edit
/// PCM bit-for-bit.
///
/// Topology before the edit: track clip → master at fader 1.0
/// (PCM = x). The intent adds a return bus wired to master plus a
/// pre-fader send at 0.5 and a post-fader send at 0.25 — with unity
/// fader both taps carry x, so master = x + 0.5x + 0.25x = 1.75·x.
#[test]
fn sf06_graph_edit_pcm_undo_save_reload() {
    let root = scratch("edit");
    let mut c = controller();
    let out = import_stereo(&mut c, &root, "a.wav", 4_800);
    let (track, lane) = add_track_strip(&mut c);
    add_clip(
        &mut c,
        lane,
        out.commit.source_id,
        0,
        u64::try_from(CLIP_FRAMES).unwrap(),
        0,
    );
    let strip = strip_of_track(c.live(), track);
    let media = root.join("media");

    let before = render(&mut c, &media, 1, 3);
    assert_golden(&before.pcm, 1.0, "pre-edit");

    let rev_before = c.live_revision();
    let master = master_strip(c.live());
    wire_sends(
        &mut c,
        master,
        &[
            (strip, SendTap::PreFader, 0.5),
            (strip, SendTap::PostFader, 0.25),
        ],
    );
    // The whole gesture is exactly one revision.
    assert_eq!(c.live_revision().0, rev_before.0 + 1);

    let after = render(&mut c, &media, 2, 3);
    assert_golden(&after.pcm, 1.75, "post-edit");

    // Save → load: the reloaded project is identical and renders the
    // same PCM — the persisted state is the PCM authority.
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c.live(), c2.live());
    let reloaded = render(&mut c2, &media, 3, 3);
    assert_eq!(reloaded.pcm, after.pcm);
    assert_eq!(reloaded.crc, after.crc);

    // One undo restores the pre-edit project *and* its PCM.
    c.undo().unwrap();
    let undone = render(&mut c, &media, 4, 3);
    assert_eq!(undone.pcm, before.pcm);
    assert_eq!(undone.crc, before.crc);
}

/// SF-VALID-03/04: mute and solo semantics hold through the send
/// path at PCM level — a muted strip's main *and* send contributions
/// are measured silence at the master, and a soloed strip is the
/// only one audible through both paths.
///
/// Topology: tracks A and B on disjoint clip windows
/// (`[0,480)` / `[480,960)`), each with a post-fader send at 0.5 to
/// a shared return bus wired to master; all faders unity.
/// Both audible: each window = 1.0·x (main) + 0.5·x (send) = 1.5·x.
#[test]
fn sf06_mute_solo_pcm_through_sends() {
    let root = scratch("ms");
    let mut c = controller();
    let out_a = import_stereo(&mut c, &root, "a.wav", 4_800);
    let out_b = import_stereo(&mut c, &root, "b.wav", 4_800);
    let (ta, la) = add_track_strip(&mut c);
    add_clip(
        &mut c,
        la,
        out_a.commit.source_id,
        0,
        u64::try_from(CLIP_FRAMES).unwrap(),
        0,
    );
    let (tb, lb) = add_track_strip(&mut c);
    // Clip B covers engine [480, 960) reading source [0, 480).
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lb,
            source_id: out_b.commit.source_id,
            placement: media_placement(
                u64::try_from(CLIP_FRAMES).unwrap(),
                2 * u64::try_from(CLIP_FRAMES).unwrap(),
                48_000,
            ),
            playback: natural_playback(0, i64::try_from(CLIP_FRAMES).unwrap()),
        },
    );
    let strip_a = strip_of_track(c.live(), ta);
    let strip_b = strip_of_track(c.live(), tb);
    let media = root.join("media");

    let master = master_strip(c.live());
    wire_sends(
        &mut c,
        master,
        &[
            (strip_a, SendTap::PostFader, 0.5),
            (strip_b, SendTap::PostFader, 0.5),
        ],
    );

    // 4 blocks = 1024 frames: both clip windows plus tail silence.
    let both = render(&mut c, &media, 1, 4);
    assert_two_clip_golden(&both.pcm, 1.5, 1.5, "both audible");

    // Mute A: A's main edge AND its send edge are pruned — A's
    // window is measured silence while B still sounds alone.
    apply(
        &mut c,
        ProjectOp::SetStripMute {
            strip_id: strip_a,
            muted: true,
        },
    );
    let muted = render(&mut c, &media, 2, 4);
    for (ch, plane) in muted.pcm.iter().enumerate() {
        for (f, s) in plane.iter().enumerate() {
            let expected = if (CLIP_FRAMES..2 * CLIP_FRAMES).contains(&f) {
                fixture_sample_f32(ch, f - CLIP_FRAMES) * 1.5
            } else {
                0.0
            };
            assert_eq!(*s, expected, "mute A: ch{ch} frame{f}");
        }
    }

    // Unmute A, solo A: B's main+send contributions drop to measured
    // silence; A sounds through both paths exactly as before.
    apply(
        &mut c,
        ProjectOp::SetStripMute {
            strip_id: strip_a,
            muted: false,
        },
    );
    apply(
        &mut c,
        ProjectOp::SetStripSolo {
            strip_id: strip_a,
            soloed: true,
        },
    );
    let soloed = render(&mut c, &media, 3, 4);
    for (ch, plane) in soloed.pcm.iter().enumerate() {
        for (f, s) in plane.iter().enumerate() {
            let expected = if f < CLIP_FRAMES {
                fixture_sample_f32(ch, f) * 1.5
            } else {
                0.0
            };
            assert_eq!(*s, expected, "solo A: ch{ch} frame{f}");
        }
    }
}
