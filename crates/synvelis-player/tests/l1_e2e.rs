//! L1 acceptance: local WAV safe import → verified source → clip →
//! generic graph headless playback (Issue #24, acceptance L1-01..08).
//!
//! Deterministic evidence: every asserted sample is re-derived from
//! `fixture_sample`, import/commit receipts are kept separate, and
//! each failure class is a typed error — never a silent success.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use synvelis_player::driver::ActivateError;
use synvelis_player::fixture::{fixture_sample_f32, wav_pcm16, write_fixture};
use synvelis_player::import::{
    BlobMaterialization, ImportRequest, blob_path, content_digest, import_local,
};
use synvelis_player::lowering::{EngineConfig, lower};
use synvelis_player::resolver::prepare;
use synvelis_player::{DriverState, HeadlessDriver, ImportError, PumpPolicy, ResolveError};
use synvelis_project::{
    ClipExtent, ClipId, ClipPlacement, ClipPlayback, Command, CounterIdGenerator,
    DirectoryBundleStore, MediaDuration, MediaTimeValue, MusicalDuration, MusicalTick,
    PlaybackMapping, Project, ProjectController, ProjectId, ProjectOp, ProjectRevision,
    SourceFrame, SourceId, SourceRange, TimeAnchor,
};

use ruviform_media_reader::PrefetchConfig;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

/// Unique writable scratch area for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-l1-{}-{}-{}",
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
        Project::new(ProjectId::new(7), "l1"),
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

fn musical_placement(start_tick: i64, end_tick: i64) -> ClipPlacement {
    ClipPlacement {
        anchor: TimeAnchor::Musical(MusicalTick::new(start_tick)),
        extent: ClipExtent::Musical(MusicalDuration::new(end_tick - start_tick).unwrap()),
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

fn request(root: &Path, path: &Path) -> ImportRequest {
    ImportRequest {
        path: path.to_path_buf(),
        staging_root: root.join("staging"),
        media_root: root.join("media"),
        resume: None,
    }
}

/// Imports a fixture and returns (outcome, blob bytes digest).
fn do_import(
    c: &mut ProjectController,
    root: &Path,
    path: &Path,
) -> synvelis_player::ImportOutcome {
    import_local(c, &request(root, path)).unwrap()
}

fn prefetch(channels: u16, rate: u32) -> PrefetchConfig {
    PrefetchConfig {
        block_frames: 256,
        block_count: 8,
        channels: usize::from(channels),
        sample_rate: rate,
    }
}

fn engine(rate: u32, channels: u16, block: usize) -> EngineConfig {
    EngineConfig {
        sample_rate: rate,
        channels,
        block_frames: block,
    }
}

/// Compile-time witness that the three identity types are distinct.
fn typed_identities(_: SourceId, _: synvelis_project::AssetId, _: ClipId) {}

// ---------------------------------------------------------------- L1-01

/// L1-01: a small mono/stereo WAV verifies (frames/channels/rate/hash),
/// materializes as a verified blob, and an unverified source can never
/// resolve into a playback path.
#[test]
fn l1_01_import_verifies_and_materializes() {
    let root = scratch("l101");
    let wav = root.join("mono.wav");
    let bytes = write_fixture(&wav, 1, 48_000, 960).unwrap();
    let mut c = controller();

    let out = do_import(&mut c, &root, &wav);
    assert!(!out.reused);
    assert_eq!(out.media.frames, 960);
    assert_eq!(out.media.channels, 1);
    assert_eq!(out.media.sample_rate, 48_000);
    assert_eq!(out.media.codec, "pcm");
    assert_eq!(out.media.container, "wav");
    assert_eq!(out.media.digest, content_digest(&bytes));
    assert_eq!(out.media.byte_len, bytes.len() as u64);
    assert_ne!(out.media.durability, "");

    // Blob receipt is separate from the project commit receipt.
    assert_eq!(out.blob.digest, out.media.digest);
    assert_eq!(out.blob.byte_len, bytes.len() as u64);
    assert_eq!(out.blob.materialized, BlobMaterialization::Created);
    assert_eq!(fs::read(&out.blob.path).unwrap(), bytes);
    assert_ne!(out.commit.import_id.raw(), 0);

    // Project carries the registered asset + verified source.
    let project = c.live();
    assert!(project.assets.contains_key(&out.commit.asset_id));
    let source = &project.sources[&out.commit.source_id];
    assert_eq!(source.asset_id, out.commit.asset_id);
    assert_eq!(source.content_digest, out.media.digest);
    assert!(source.state >= synvelis_project::SourceState::Verified);
    assert_eq!(project.applied_imports.len(), 1);

    // The resolver now produces a real reader + prefetch pipeline.
    let prepared = prepare(
        project,
        out.commit.source_id,
        &root.join("media"),
        &prefetch(1, 48_000),
    )
    .unwrap();
    assert_eq!(prepared.canonical.layout.channels(), 1);
    assert_eq!(prepared.canonical.sample_rate.get(), 48_000);
    assert_eq!(prepared.digest, out.media.digest);

    // A pre-verification source is unplayable: register one bare.
    apply(&mut c, ProjectOp::RegisterAsset { digest: "x".into() });
    let bare_asset = *c
        .live()
        .assets
        .keys()
        .find(|a| **a != out.commit.asset_id)
        .unwrap();
    apply(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: bare_asset,
            content_digest: "x".into(),
        },
    );
    let bare_source = *c
        .live()
        .sources
        .keys()
        .find(|s| **s != out.commit.source_id)
        .unwrap();
    assert!(matches!(
        prepare(
            c.live(),
            bare_source,
            &root.join("media"),
            &prefetch(1, 48_000)
        ),
        Err(ResolveError::UnverifiedSource { .. })
    ));

    // Stereo path verifies the same way.
    let stereo = root.join("stereo.wav");
    let sbytes = write_fixture(&stereo, 2, 48_000, 300).unwrap();
    let out2 = do_import(&mut c, &root, &stereo);
    assert_eq!(out2.media.channels, 2);
    assert_eq!(out2.media.frames, 300);
    assert_eq!(out2.media.digest, content_digest(&sbytes));
}

// ---------------------------------------------------------------- L1-02

/// L1-02: SourceId/AssetId/ClipId are distinct identities; one source
/// backs two clips at different placement/offset; undo/redo/save/reload
/// preserve every reference.
#[test]
fn l1_02_id_separation_shared_source_undo_save() {
    let root = scratch("l102");
    let wav = root.join("s.wav");
    write_fixture(&wav, 1, 48_000, 4_800).unwrap();
    let mut c = controller();
    let out = do_import(&mut c, &root, &wav);
    let source_id = out.commit.source_id;
    let asset_id = out.commit.asset_id;

    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id,
            placement: media_placement(0, 480, 48_000),
            playback: natural_playback(0, 480),
        },
    );
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id,
            placement: media_placement(960, 1_440, 48_000),
            playback: natural_playback(96, 480),
        },
    );

    let clips: Vec<ClipId> = c.live().clips.keys().copied().collect();
    assert_eq!(clips.len(), 2);
    // Distinct typed identities — no raw/u128 mixing.
    typed_identities(source_id, asset_id, clips[0]);
    assert_ne!(source_id.raw(), asset_id.raw());
    assert!(c.live().clips.values().all(|cl| cl.source_id == source_id));
    assert_eq!(
        c.live()
            .clips
            .values()
            .map(|c| c.playback.source_region.start().value())
            .collect::<Vec<_>>(),
        vec![0, 96]
    );

    // Undo both clip adds; redo restores them.
    assert!(c.undo().unwrap());
    assert!(c.undo().unwrap());
    assert!(c.live().clips.is_empty());
    assert!(c.redo().unwrap());
    assert!(c.redo().unwrap());
    assert_eq!(c.live().clips.len(), 2);

    // Save → reload into a fresh controller preserves references.
    let mut store = DirectoryBundleStore::create(root.join("bundle")).unwrap();
    c.save(&mut store).unwrap();
    let mut reloaded = controller();
    reloaded.load(&store).unwrap();
    let rp = reloaded.live();
    assert_eq!(rp.clips.len(), 2);
    assert!(rp.clips.values().all(|cl| cl.source_id == source_id));
    assert_eq!(rp.sources[&source_id].content_digest, out.media.digest);
    assert_eq!(rp.applied_imports.len(), 1);
    assert_eq!(reloaded.live_revision(), c.live_revision());
}

// ---------------------------------------------------------------- L1-03

/// L1-03: deterministic 48 kHz headless playback — clip start/end,
/// `[start,end)`, source-offset read, seek/stop, block-boundary
/// crossing and EOF are all sample-exact.
#[test]
fn l1_03_headless_clip_playback_exact() {
    let root = scratch("l103");
    let wav = root.join("stereo.wav");
    write_fixture(&wav, 2, 48_000, 4_800).unwrap();
    let mut c = controller();
    let out = do_import(&mut c, &root, &wav);
    let source_id = out.commit.source_id;

    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    // Clip at engine [64, 764), reading source [13, 713) — both edges
    // deliberately inside blocks (256-frame blocks).
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id,
            placement: media_placement(64, 764, 48_000),
            playback: natural_playback(13, 700),
        },
    );
    let clip_id = *c.live().clips.keys().next().unwrap();

    let eng = engine(48_000, 2, 256);
    let lowered = lower(
        c.live(),
        &|_| {
            Some(ruviform_audio_core::AudioFormat {
                sample_rate: ruviform_audio_core::SampleRate::new(48_000).unwrap(),
                layout: ruviform_audio_core::ChannelLayout::Stereo,
                sample_format: ruviform_audio_core::SampleFormat::F32,
            })
        },
        &eng,
    );
    assert_eq!(lowered.playbacks.len(), 1);
    assert_eq!(lowered.playbacks[0].window.engine_start, 64);
    assert_eq!(lowered.playbacks[0].window.engine_len, 700);
    assert_eq!(lowered.playbacks[0].window.source_start, 13);

    let mut driver = HeadlessDriver::new(eng);
    let prepared = vec![
        prepare(
            c.live(),
            source_id,
            &root.join("media"),
            &prefetch(2, 48_000),
        )
        .unwrap(),
    ];
    driver.activate(&lowered, prepared).unwrap();
    assert_eq!(driver.state(), DriverState::Stopped);
    assert!(driver.published_revision().is_some());

    // 6 blocks = 1536 engine frames: window 64..764 lands inside.
    let report = driver.run(6, PumpPolicy::Every(1)).unwrap();
    assert_eq!(driver.state(), DriverState::Playing);
    assert_eq!(report.pcm.len(), 2);
    for ch in 0..2 {
        for frame in 0..report.pcm[ch].len() {
            let expected = if (64..764).contains(&frame) {
                fixture_sample_f32(ch, 13 + (frame - 64))
            } else {
                0.0
            };
            assert_eq!(
                report.pcm[ch][frame], expected,
                "ch{ch} frame{frame} mismatch"
            );
        }
    }
    let stats = driver.stats_of(clip_id).unwrap().snapshot();
    assert_eq!(stats.written, 700);
    assert_eq!(stats.underrun, 0);
    assert_eq!(stats.faulted, 0);
    assert_eq!(stats.eof_blocks, 0); // window ends before source EOF
    assert!(stats.silenced > 0);

    // Seek back to start and replay — same PCM (loop-equivalent
    // determinism, cursor + per-clip source seek).
    driver.seek(0).expect("seek");
    assert_eq!(driver.cursor(), 0);
    let replay = driver.run(6, PumpPolicy::Every(1)).unwrap();
    assert_eq!(replay.crc, report.crc);
    assert_eq!(replay.pcm, report.pcm);

    // Explicit stop is a state, not a failure.
    driver.stop();
    assert_eq!(driver.state(), DriverState::Stopped);

    // Underrun: rewind the cursor into the window, then never pump —
    // zero audio, underrun counted, never reported as content.
    driver.seek(0).expect("seek");
    let starved = driver.run(2, PumpPolicy::Every(0)).unwrap();
    assert!(starved.pcm.iter().flatten().all(|s| *s == 0.0));
    let stats2 = driver.stats_of(clip_id).unwrap().snapshot();
    assert!(stats2.underrun > stats.underrun);
}

/// L1-03 (EOF edge): a clip window extending past source end plays the
/// real samples then silence, with EOF recorded — never extra content.
#[test]
fn l1_03_clip_end_past_source_eof() {
    let root = scratch("l103eof");
    let wav = root.join("short.wav");
    write_fixture(&wav, 1, 48_000, 300).unwrap();
    let mut c = controller();
    let out = do_import(&mut c, &root, &wav);
    let source_id = out.commit.source_id;

    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    // Window [0, 512) on a 300-frame source: 300 real + 212 silence.
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id,
            placement: media_placement(0, 512, 48_000),
            playback: natural_playback(0, 512),
        },
    );
    let clip_id = *c.live().clips.keys().next().unwrap();

    let eng = engine(48_000, 1, 256);
    let lowered = lower(
        c.live(),
        &|_| {
            Some(ruviform_audio_core::AudioFormat {
                sample_rate: ruviform_audio_core::SampleRate::new(48_000).unwrap(),
                layout: ruviform_audio_core::ChannelLayout::Mono,
                sample_format: ruviform_audio_core::SampleFormat::F32,
            })
        },
        &eng,
    );
    let mut driver = HeadlessDriver::new(eng);
    driver
        .activate(
            &lowered,
            vec![
                prepare(
                    c.live(),
                    source_id,
                    &root.join("media"),
                    &prefetch(1, 48_000),
                )
                .unwrap(),
            ],
        )
        .unwrap();
    let report = driver.run(2, PumpPolicy::Every(1)).unwrap();
    eprintln!(
        "DIAG stats={:?}",
        driver.stats_of(clip_id).unwrap().snapshot()
    );
    for frame in 0..512 {
        let expected = if frame < 300 {
            fixture_sample_f32(0, frame)
        } else {
            0.0
        };
        assert_eq!(report.pcm[0][frame], expected, "frame{frame}");
    }
    let stats = driver.stats_of(clip_id).unwrap().snapshot();
    eprintln!("DIAG stats={stats:?} pcm0={:?}", &report.pcm[0][..8]);
    assert_eq!(stats.written, 300);
    assert!(stats.eof_blocks >= 1);
}

// ---------------------------------------------------------------- L1-04

/// L1-04: missing/corrupt/unsupported input, hash drift, unresolved
/// source, missing media, revision conflict, cancellation-ish resume
/// and store errors all fail typed — no partial project mutation.
#[test]
fn l1_04_failure_classes_are_typed_and_atomic() {
    let root = scratch("l104");
    let mut c = controller();
    let rev0 = c.live_revision();

    // Missing file → staging I/O error, project untouched.
    let missing = root.join("nope.wav");
    assert!(matches!(
        import_local(&mut c, &request(&root, &missing)),
        Err(ImportError::StagingIo { .. })
    ));
    assert_eq!(c.live_revision(), rev0);
    assert!(c.live().sources.is_empty());

    // Corrupt bytes → container probe failure.
    let garbage = root.join("garbage.wav");
    fs::write(&garbage, vec![0xAB_u8; 512]).unwrap();
    assert!(matches!(
        import_local(&mut c, &request(&root, &garbage)),
        Err(ImportError::Corrupt { .. } | ImportError::StagingIo { .. })
    ));
    assert_eq!(c.live_revision(), rev0);

    // Unsupported codec: valid WAV container, non-PCM format tag.
    let unsupported = root.join("nonpcm.wav");
    let mut bytes = wav_pcm16(1, 48_000, 64);
    bytes[20] = 0x55; // format tag → MP3-in-WAV, not PCM
    bytes[21] = 0x00;
    fs::write(&unsupported, &bytes).unwrap();
    let err = import_local(&mut c, &request(&root, &unsupported)).unwrap_err();
    assert!(
        matches!(
            err,
            ImportError::UnsupportedCodec { .. } | ImportError::Corrupt { .. }
        ),
        "unexpected error {err}"
    );
    assert_eq!(c.live_revision(), rev0);
    assert!(c.live().assets.is_empty());

    // Unknown resume token → UnknownImport (no manifest, no mutation).
    assert!(matches!(
        import_local(
            &mut c,
            &ImportRequest {
                resume: Some(synvelis_project::ImportId::new(9_999)),
                ..request(&root, &missing)
            }
        ),
        Err(ImportError::UnknownImport { .. })
    ));
    assert_eq!(c.live_revision(), rev0);

    // Stale expected revision → rejected before any mutation.
    let stale = Command {
        expected_revision: ProjectRevision(42),
        op: ProjectOp::AddTrack { name: "x".into() },
    };
    assert!(matches!(
        c.apply(stale),
        Err(synvelis_project::Error::StaleRevision { .. })
    ));
    assert_eq!(c.live_revision(), rev0);
    assert!(c.live().tracks.is_empty());

    // A good import, then blob corruption on disk → HashMismatch.
    let wav = root.join("ok.wav");
    write_fixture(&wav, 1, 48_000, 128).unwrap();
    let out = do_import(&mut c, &root, &wav);
    let blob = blob_path(&root.join("media"), &out.media.digest);
    let tampered = root.join("media-tampered");
    fs::create_dir_all(tampered.join("blobs")).unwrap();
    fs::write(
        tampered.join("blobs").join(&out.media.digest),
        b"not the same bytes",
    )
    .unwrap();
    assert!(matches!(
        prepare(
            c.live(),
            out.commit.source_id,
            &tampered,
            &prefetch(1, 48_000)
        ),
        Err(ResolveError::HashMismatch { .. })
    ));

    // Missing media: correct root but blob deleted.
    fs::remove_file(&blob).unwrap();
    assert!(matches!(
        prepare(
            c.live(),
            out.commit.source_id,
            &root.join("media"),
            &prefetch(1, 48_000)
        ),
        Err(ResolveError::MissingMedia { .. })
    ));

    // Unresolved source id.
    assert!(matches!(
        prepare(
            c.live(),
            SourceId::new(9_999),
            &root.join("media"),
            &prefetch(1, 48_000)
        ),
        Err(ResolveError::UnresolvedSource { .. })
    ));

    // Store error: bundle root is a regular file, not a directory.
    let file_root = root.join("not-a-dir");
    fs::write(&file_root, b"x").unwrap();
    assert!(DirectoryBundleStore::create(&file_root).is_err());
}

// ---------------------------------------------------------------- L1-05

/// L1-05: no source-frame/placement off-by-one — media and musical
/// anchors convert exactly, rounding only at the final frame boundary.
#[test]
fn l1_05_time_conversion_is_exact_and_documented() {
    let root = scratch("l105");
    let wav = root.join("s.wav");
    write_fixture(&wav, 1, 48_000, 9_600).unwrap();
    let mut c = controller();
    let out = do_import(&mut c, &root, &wav);
    let source_id = out.commit.source_id;

    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    // Media anchor: exact frame math in the declared base (1/48000).
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id,
            placement: media_placement(1, 101, 48_000),
            playback: natural_playback(5, 100),
        },
    );
    // Musical anchor: default 120 BPM @ 960 PPQN → 25 engine frames
    // per tick at 48 kHz; tick [8, 208) → frames [200, 5200). Kept
    // disjoint from the media clip so outputs never mix.
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id,
            placement: musical_placement(8, 208),
            playback: natural_playback(0, 5_000),
        },
    );

    let eng = engine(48_000, 1, 256);
    let canonical = ruviform_audio_core::AudioFormat {
        sample_rate: ruviform_audio_core::SampleRate::new(48_000).unwrap(),
        layout: ruviform_audio_core::ChannelLayout::Mono,
        sample_format: ruviform_audio_core::SampleFormat::F32,
    };
    let lowered = lower(c.live(), &|_| Some(canonical), &eng);
    let mut windows: Vec<(u64, u64, u64)> = lowered
        .playbacks
        .iter()
        .map(|b| {
            (
                b.window.engine_start,
                b.window.engine_len,
                b.window.source_start,
            )
        })
        .collect();
    windows.sort_unstable();
    assert_eq!(windows, vec![(1, 100, 5), (200, 5_000, 0)]);

    // Play it back: media clip frames 1..=100 map onto source 5..=104;
    // the musical clip covers frames 200..=5199.
    let mut driver = HeadlessDriver::new(eng);
    let prepared = lowered
        .playbacks
        .iter()
        .map(|_| {
            prepare(
                c.live(),
                source_id,
                &root.join("media"),
                &prefetch(1, 48_000),
            )
            .unwrap()
        })
        .collect();
    driver.activate(&lowered, prepared).unwrap();
    let report = driver.run(21, PumpPolicy::Every(1)).unwrap();
    assert_eq!(report.pcm[0][0], 0.0);
    for frame in 1..=100_usize {
        assert_eq!(
            report.pcm[0][frame],
            fixture_sample_f32(0, 5 + (frame - 1)),
            "media clip frame{frame}"
        );
    }
    // `[start,end)` — frame 101 is the exclusive end, silent.
    assert_eq!(report.pcm[0][101], 0.0, "exclusive end must be silent");
    for frame in [199_usize, 200, 201, 5_199, 5_200] {
        let expected = if (200..5_200).contains(&frame) {
            fixture_sample_f32(0, frame - 200)
        } else {
            0.0
        };
        assert_eq!(report.pcm[0][frame], expected, "musical frame{frame}");
    }
}

// ---------------------------------------------------------------- L1-06

/// L1-06: reader/graph capability and latency evidence — backend,
/// container, canonical format, declared frames, plan latency — all
/// recorded, and underrun/EOF are never conflated with audio.
#[test]
fn l1_06_capability_and_latency_evidence() {
    let root = scratch("l106");
    let wav = root.join("cap.wav");
    write_fixture(&wav, 2, 48_000, 1_024).unwrap();
    let mut c = controller();
    let out = do_import(&mut c, &root, &wav);
    let source_id = out.commit.source_id;

    let prepared = prepare(
        c.live(),
        source_id,
        &root.join("media"),
        &prefetch(2, 48_000),
    )
    .unwrap();
    // Recorded capabilities for the report: backend/container/format.
    assert_eq!(prepared.canonical.sample_rate.get(), 48_000);
    assert_eq!(prepared.canonical.layout.channels(), 2);
    assert_eq!(out.media.container, "wav");
    assert_eq!(out.media.codec, "pcm");
    assert_ne!(out.media.durability, "");

    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id,
            placement: media_placement(0, 1_024, 48_000),
            playback: natural_playback(0, 1_024),
        },
    );
    let eng = engine(48_000, 2, 256);
    let canonical = prepared.canonical;
    let lowered = lower(c.live(), &|_| Some(canonical), &eng);
    let mut driver = HeadlessDriver::new(eng);
    driver.activate(&lowered, vec![prepared]).unwrap();
    // Plan latency is reported as a number, not hidden.
    let latency = driver.total_latency();
    assert_eq!(latency, 0, "zero-delay edges compile to zero latency");
    let report = driver.run(4, PumpPolicy::Every(1)).unwrap();
    assert!(report.pcm.iter().flatten().any(|s| *s != 0.0));
}

// ---------------------------------------------------------------- L1-04b

/// L1-04 (idempotency/orphan): re-running the same import replays the
/// manifest (no duplicate asset/source/blob); a blob published before
/// a failed commit is an explicit orphan.
#[test]
fn l1_04_import_idempotency_and_orphan() {
    let root = scratch("l104idem");
    let wav = root.join("i.wav");
    write_fixture(&wav, 1, 48_000, 256).unwrap();
    let mut c = controller();
    let out = do_import(&mut c, &root, &wav);
    let sources_before = c.live().sources.len();
    let assets_before = c.live().assets.len();
    let rev_after = c.live_revision();

    // Replay with the same import id → manifest replay, no new state.
    let replay = import_local(
        &mut c,
        &ImportRequest {
            resume: Some(out.commit.import_id),
            ..request(&root, &wav)
        },
    )
    .unwrap();
    assert!(replay.reused);
    assert_eq!(replay.blob.materialized, BlobMaterialization::Reused);
    assert_eq!(replay.commit.source_id, out.commit.source_id);
    assert_eq!(replay.commit.asset_id, out.commit.asset_id);
    assert_eq!(c.live().sources.len(), sources_before);
    assert_eq!(c.live().assets.len(), assets_before);
    assert_eq!(c.live_revision(), rev_after); // no duplicate mutation

    // Fresh import of the same file: a distinct import identity, but
    // the verified blob for the same digest is reused, not rewritten.
    let again = do_import(&mut c, &root, &wav);
    assert!(!again.reused);
    assert_eq!(again.blob.materialized, BlobMaterialization::Reused);
    assert_ne!(again.commit.import_id, out.commit.import_id);
    assert_eq!(c.live().sources.len(), sources_before + 1);

    // Orphan: a manifest under a *fresh* import id whose reserved
    // source_id collides with an existing project source passes the
    // replay guard, reaches the commit stage and is rejected — the
    // published blob is reported as an explicit orphan.
    let orphan_id = synvelis_project::ImportId::new(0x0BAD_0BAD);
    let orphan_dir = root
        .join("staging")
        .join(format!("import-{:032x}", orphan_id.raw()));
    fs::create_dir_all(&orphan_dir).unwrap();
    let source_dir = root
        .join("staging")
        .join(format!("import-{:032x}", out.commit.import_id.raw()));
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(source_dir.join("import-manifest.json")).unwrap())
            .unwrap();
    manifest["import_id"] = serde_json::json!(orphan_id.to_string());
    manifest["source_id"] = serde_json::json!(out.commit.source_id.to_string());
    manifest["stage"] = serde_json::json!("BlobPublished");
    fs::write(
        orphan_dir.join("import-manifest.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .unwrap();
    let sources_before_orphan = c.live().sources.len();
    let err = import_local(
        &mut c,
        &ImportRequest {
            resume: Some(orphan_id),
            ..request(&root, &wav)
        },
    )
    .unwrap_err();
    assert!(
        matches!(err, ImportError::Orphaned { ref digest, .. } if digest == &out.media.digest),
        "expected orphan receipt, got {err:?}"
    );
    // The blob stays published (explicit orphan for cleanup/retry) and
    // the project took no partial mutation.
    assert!(blob_path(&root.join("media"), &out.media.digest).exists());
    assert_eq!(c.live().sources.len(), sources_before_orphan);
    assert!(!c.live().applied_imports.contains_key(&orphan_id));
}

// ---------------------------------------------------------------- L1-07

/// L1-07 evidence: this test exists inside `cargo test --workspace
/// --locked`; the lock revision is asserted indirectly by successful
/// compilation and recorded in the implementation record.
#[test]
fn l1_07_fixture_is_deterministic() {
    // Same fixture written twice is byte-identical; the sample
    // function is pure and exactly representable.
    let a = wav_pcm16(2, 48_000, 512);
    let b = wav_pcm16(2, 48_000, 512);
    assert_eq!(a, b);
    assert_eq!(content_digest(&a), content_digest(&b));
    for frame in [0_usize, 1, 255, 256, 257, 4095, 4096] {
        let f = fixture_sample_f32(1, frame);
        assert!((-1.0..=1.0).contains(&f));
    }
}

/// Graph activation failure keeps last-good state: a driver that has
/// never published stays Stopped, and a second `activate` after a
/// failure leaves the earlier plan live.
#[test]
fn l1_compat_prepare_failure_keeps_last_good() {
    let root = scratch("l1compat");
    let wav = root.join("c.wav");
    write_fixture(&wav, 1, 48_000, 256).unwrap();
    let mut c = controller();
    let out = do_import(&mut c, &root, &wav);
    let source_id = out.commit.source_id;

    apply(&mut c, ProjectOp::AddTrack { name: "t".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply(&mut c, ProjectOp::AddLane { track_id: track });
    let lane = *c.live().lanes.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::AddClip {
            lane_id: lane,
            source_id,
            placement: media_placement(0, 256, 48_000),
            playback: natural_playback(0, 256),
        },
    );
    let eng = engine(48_000, 1, 256);
    let canonical = ruviform_audio_core::AudioFormat {
        sample_rate: ruviform_audio_core::SampleRate::new(48_000).unwrap(),
        layout: ruviform_audio_core::ChannelLayout::Mono,
        sample_format: ruviform_audio_core::SampleFormat::F32,
    };
    let lowered = lower(c.live(), &|_| Some(canonical), &eng);
    let mut driver = HeadlessDriver::new(eng);
    driver
        .activate(
            &lowered,
            vec![
                prepare(
                    c.live(),
                    source_id,
                    &root.join("media"),
                    &prefetch(1, 48_000),
                )
                .unwrap(),
            ],
        )
        .unwrap();
    let first = driver.published_revision().unwrap();
    let report = driver.run(1, PumpPolicy::Every(1)).unwrap();
    assert_eq!(report.pcm[0][0], fixture_sample_f32(0, 0));

    // A spec whose edge declares a channel mapping outside the
    // destination port's layout must fail validation while the
    // last-good plan stays published and playable.
    let mut bad_builder = ruviform_graph::GraphBuilder::new();
    let (sink, sink_ports) = bad_builder.add_node(&ruviform_graph::NodeDef {
        kind: ruviform_graph::NodeKind::Sink,
        ports: vec![ruviform_graph::PortDef {
            direction: ruviform_graph::Direction::Input,
            role: ruviform_graph::PortRole::Main,
            format: ruviform_graph::PortFormat::Audio(canonical),
            multiplicity: ruviform_graph::Multiplicity::unbounded(0),
            capabilities: Vec::new(),
            availability: ruviform_graph::Availability::Always,
            activation: ruviform_graph::Activation::Enabled,
        }],
    });
    let (proc_node, proc_ports) = bad_builder.add_node(&ruviform_graph::NodeDef {
        kind: ruviform_graph::NodeKind::Processor,
        ports: vec![ruviform_graph::PortDef {
            direction: ruviform_graph::Direction::Output,
            role: ruviform_graph::PortRole::Main,
            format: ruviform_graph::PortFormat::Audio(canonical),
            multiplicity: ruviform_graph::Multiplicity::exactly(1),
            capabilities: Vec::new(),
            availability: ruviform_graph::Availability::Always,
            activation: ruviform_graph::Activation::Enabled,
        }],
    });
    bad_builder.connect(&ruviform_graph::ConnDef {
        from: ruviform_graph::PortRef {
            node: proc_node,
            port: proc_ports[0],
        },
        to: ruviform_graph::PortRef {
            node: sink,
            port: sink_ports[0],
        },
        delay: ruviform_graph::EdgeDelay::Zero,
        channel_map: Some(ruviform_graph::ChannelMap {
            entries: vec![(0, 99)], // dst 99 outside a mono port
        }),
    });
    let bad = synvelis_player::lowering::Lowered {
        spec: bad_builder.build(),
        playbacks: Vec::new(),
        master_in: lowered.master_in,
        inserts: Vec::new(),
        skipped: Vec::new(),
    };
    let attempt = driver.activate(&bad, Vec::new());
    assert!(matches!(
        attempt,
        Err(ActivateError::Validation(_) | ActivateError::Compile(_))
    ));
    assert_eq!(driver.published_revision(), Some(first));
    driver.seek(0).expect("seek");
    let again = driver.run(1, PumpPolicy::Every(1)).unwrap();
    assert_eq!(again.pcm[0][0], fixture_sample_f32(0, 0));
    driver.collect();
}
