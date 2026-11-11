//! Issue #86 DoD(2) chain — verified consumer import:
//! root-bounded pin → `import_verified` → published-blob decode
//! verification → `SetSourceState`→`Verified` → resolver +
//! `HeadlessDriver` sample-exact playback.
//!
//! Also covered: undo→re-import manifest reservation convergence
//! (CAS/Undo), Saved HEAD→load→playback, a library-committed
//! `Imported` source replayed through the entry, interrupted-job
//! `inspect`→resume→commit→playback, sha256 missing/mismatch
//! negatives, opaque digest fail-closed, and the legacy `Crc32Fold`
//! flat-blob non-regression.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use synvelis_library::{
    AssetProvider, BlobPublishError, BlobPublishReceipt, BlobPublisher, ContentDigest,
    ImportError as LibImportError, ImportIntent, ImportStore, ImportTarget, LocalBlobDir,
    LocalFolderProvider, OpaqueLocator, PrincipalId, ProviderContext, ProviderErrorKind,
    PublishDurability, RemoteAssetRef, StagedBlobPublisher, inspect_imports, rebase_import,
};
use synvelis_player::blob::open_blob_verified;
use synvelis_player::driver::HeadlessDriver;
use synvelis_player::error::VerifiedImportError;
use synvelis_player::fixture::{fixture_sample_f32, write_fixture};
use synvelis_player::import::{ImportRequest, import_local};
use synvelis_player::lowering::{EngineConfig, lower};
use synvelis_player::resolver::prepare;
use synvelis_player::verified_import::{
    VERIFIED_IMPORT_PROVIDER, VerifiedImportRequest, import_verified_local,
};
use synvelis_player::{DriverState, PumpPolicy, ResolveError};
use synvelis_project::{
    ClipExtent, ClipPlacement, ClipPlayback, Command, CounterIdGenerator, DirectoryBundleStore,
    ImportId, MediaDuration, MediaTimeValue, PlaybackMapping, Project, ProjectController,
    ProjectId, ProjectOp, ProjectRevision, SourceFrame, SourceId, SourceRange, SourceState,
    TimeAnchor,
};

use ruviform_media_reader::PrefetchConfig;

static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

/// Unique writable scratch area for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-vimp-{}-{}-{}",
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
        Project::new(ProjectId::new(86), "vimp"),
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

fn cx() -> ProviderContext {
    ProviderContext::new(PrincipalId::new("vimp-test").unwrap())
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

fn request(
    root: &Path,
    path: &Path,
    target: Option<ImportTarget>,
    import_id: Option<ImportId>,
) -> VerifiedImportRequest {
    VerifiedImportRequest {
        path: path.to_path_buf(),
        staging_root: root.join("staging"),
        media_root: root.join("media"),
        target,
        import_id,
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

fn engine(rate: u32, channels: u16, block: usize) -> EngineConfig {
    EngineConfig {
        sample_rate: rate,
        channels,
        block_frames: block,
    }
}

/// Playback window used across tests: engine [64,764), source [13,713).
fn clip_target(c: &mut ProjectController) -> ImportTarget {
    apply(c, ProjectOp::AddTrack { name: "t".into() });
    let track_id = *c.live().tracks.keys().next().unwrap();
    apply(c, ProjectOp::AddLane { track_id });
    let lane_id = *c.live().lanes.keys().next().unwrap();
    ImportTarget {
        track_id,
        lane_id,
        placement: media_placement(64, 764, 48_000),
        playback: natural_playback(13, 700),
    }
}

/// Lowers the project, prepares `source_id`, and runs 6 engine blocks
/// through the headless driver — the offline playback evidence.
fn play(project: &Project, media_root: &Path, source_id: SourceId) -> Vec<Vec<f32>> {
    let eng = engine(48_000, 2, 256);
    let lowered = lower(
        project,
        &|_| {
            Some(ruviform_audio_core::AudioFormat {
                sample_rate: ruviform_audio_core::SampleRate::new(48_000).unwrap(),
                layout: ruviform_audio_core::ChannelLayout::Stereo,
                sample_format: ruviform_audio_core::SampleFormat::F32,
            })
        },
        &eng,
    );
    let prepared = vec![prepare(project, source_id, media_root, &prefetch(2, 48_000)).unwrap()];
    let mut driver = HeadlessDriver::new(eng);
    driver.activate(&lowered, prepared).unwrap();
    assert_eq!(driver.state(), DriverState::Stopped);
    let report = driver.run(6, PumpPolicy::Every(1)).unwrap();
    assert_eq!(driver.state(), DriverState::Playing);
    report.pcm
}

/// Every sample of the 6-block report must equal the re-derived
/// fixture: clip [64,764) plays source frames [13,713).
fn assert_window_pcm(pcm: &[Vec<f32>]) {
    assert_eq!(pcm.len(), 2);
    for (ch, plane) in pcm.iter().enumerate() {
        for (frame, &sample) in plane.iter().enumerate() {
            let expected = if (64..764).contains(&frame) {
                fixture_sample_f32(ch, 13 + (frame - 64))
            } else {
                0.0
            };
            assert_eq!(sample, expected, "ch{ch} frame{frame} mismatch");
        }
    }
}

/// A `BlobPublisher` that publishes normally, then trips the shared
/// cancel — parking the job at `Published` exactly where a crash
/// between publish and commit leaves it.
struct CancelAfterPublish;

impl BlobPublisher for CancelAfterPublish {
    fn publish(
        &self,
        staged_source: &Path,
        expected: &ContentDigest,
        expected_len: u64,
        destination: &Path,
        durability: PublishDurability,
        cx: &ProviderContext,
    ) -> Result<BlobPublishReceipt, BlobPublishError> {
        let receipt = StagedBlobPublisher.publish(
            staged_source,
            expected,
            expected_len,
            destination,
            durability,
            cx,
        )?;
        cx.cancel.cancel();
        Ok(receipt)
    }
}

/// Drives `synvelis_library::import_verified` directly on `wav` with
/// the same provider identity the entry uses — the fixture-level way
/// to produce a committed-at-`Imported` source or a parked job that
/// the entry then resumes.
fn lib_import(
    c: &mut ProjectController,
    wav: &Path,
    staging_root: &Path,
    media_root: &Path,
    target: Option<ImportTarget>,
    publisher: &dyn BlobPublisher,
    cx: &ProviderContext,
) -> Result<(ImportId, synvelis_library::ImportOutcome), LibImportError> {
    let provider =
        LocalFolderProvider::new(VERIFIED_IMPORT_PROVIDER, wav.parent().unwrap()).unwrap();
    let asset = RemoteAssetRef {
        provider: VERIFIED_IMPORT_PROVIDER,
        locator: OpaqueLocator::new(wav.file_name().unwrap().to_str().unwrap()).unwrap(),
    };
    let pin = provider.pin(&asset, cx).expect("pin");
    let store = ImportStore::new(staging_root, media_root);
    let blobs = LocalBlobDir::new(store.blob_dir());
    let intent = ImportIntent {
        import_id: ImportId::new(c.draw_raw_id()),
        expected_project_revision: c.live_revision(),
        source: pin,
        target,
    };
    let id = intent.import_id;
    synvelis_library::import_verified(&provider, &intent, &store, &blobs, publisher, c, cx)
        .map(|outcome| (id, outcome))
}

// ---------------------------------------------------------------- chain

/// The acceptance chain: a `"sha256:<hex>"` source created by
/// `import_verified` resolves through the digest-kind blob layer, the
/// entry advances it to `Verified`, and playback is sample-exact.
#[test]
fn sha256_import_resolves_and_plays() {
    let root = scratch("sha256");
    let wav = root.join("a.wav");
    let bytes = write_fixture(&wav, 2, 48_000, 4_800).unwrap();
    let mut c = controller();
    let target = clip_target(&mut c);

    let out =
        import_verified_local(&mut c, &request(&root, &wav, Some(target), None), &cx()).unwrap();

    // Receipt: library commit + media facts + state-advance revision.
    assert!(!out.import.reused);
    assert!(out.verified_revision.is_some());
    assert_eq!(out.import.source_state, SourceState::Imported);
    let digest = out.import.content_digest.to_string();
    assert!(digest.starts_with("sha256:"), "digest {digest}");
    assert_eq!(out.media.digest, digest);
    assert_eq!(out.media.byte_len, bytes.len() as u64);
    assert_eq!(out.media.frames, 4_800);
    assert_eq!(out.media.channels, 2);
    assert_eq!(out.media.sample_rate, 48_000);
    assert_eq!(out.media.codec, "pcm");
    assert_eq!(out.media.container, "wav");
    // Durability evidence is recorded, never inferred — directory
    // sync unsupported on this platform is honest evidence.
    assert!(out.media.durability.contains("dirsync="));

    // The blob lives under the algorithm namespace — never the legacy
    // flat path.
    let hex = digest.strip_prefix("sha256:").unwrap();
    let blob_path = root.join("media").join("blobs").join("sha256").join(hex);
    assert_eq!(out.import.blob.destination, blob_path);
    assert_eq!(fs::read(&blob_path).unwrap(), bytes);
    assert!(!root.join("media").join("blobs").join(&digest).exists());

    // Project: asset + source at Verified + applied-import receipt +
    // the atomically placed clip.
    let live = c.live();
    let source_id = out.import.commit.source_id;
    assert!(live.assets.contains_key(&out.import.commit.asset_id));
    assert_eq!(live.sources[&source_id].state, SourceState::Verified);
    assert_eq!(live.sources[&source_id].content_digest, digest);
    assert_eq!(live.applied_imports.len(), 1);
    let clip_id = out.import.commit.clip_id.expect("clip committed");
    assert_eq!(live.clips[&clip_id].source_id, source_id);
    assert_eq!(live.clips[&clip_id].lane_id, target.lane_id);

    // Resolver + headless driver: sample-exact offline playback.
    let pcm = play(live, &root.join("media"), source_id);
    assert_window_pcm(&pcm);
}

/// `import_verified` commits at `Imported` — media verification is
/// deliberately NOT its job. A source committed through the library
/// alone stays behind the playback gate; replaying the same import id
/// through the consumer entry reuses the commit, verifies the media,
/// and advances to `Verified`.
#[test]
fn imported_source_is_gated_until_entry_verifies() {
    let root = scratch("gate");
    let wav = root.join("b.wav");
    write_fixture(&wav, 2, 48_000, 4_800).unwrap();
    let mut c = controller();
    let target = clip_target(&mut c);

    // Library pipeline alone: commits at Imported — never playable.
    let (import_id, out) = lib_import(
        &mut c,
        &wav,
        &root.join("staging"),
        &root.join("media"),
        Some(target),
        &StagedBlobPublisher,
        &cx(),
    )
    .unwrap();
    let source_id = out.commit.source_id;
    let clip_id = out.commit.clip_id.unwrap();
    assert_eq!(out.source_state, SourceState::Imported);
    assert_eq!(c.live().sources[&source_id].state, SourceState::Imported);
    assert!(matches!(
        prepare(
            c.live(),
            source_id,
            &root.join("media"),
            &prefetch(2, 48_000)
        ),
        Err(ResolveError::UnverifiedSource { .. })
    ));

    // Replaying the import id through the entry: registry replay
    // (reused — nothing committed twice), media verification, then
    // the state advance.
    let again =
        import_verified_local(&mut c, &request(&root, &wav, None, Some(import_id)), &cx()).unwrap();
    assert!(again.import.reused);
    assert_eq!(again.import.commit.source_id, source_id);
    assert_eq!(again.import.commit.clip_id, Some(clip_id));
    assert!(again.verified_revision.is_some());
    assert_eq!(c.live().sources[&source_id].state, SourceState::Verified);

    let pcm = play(c.live(), &root.join("media"), source_id);
    assert_window_pcm(&pcm);
}

/// Undo → re-import converges on the manifest's reserved IDs: the
/// stale commit is a typed `StaleRevision`, an explicit `rebase_import`
/// re-arms the job, and the retry re-registers the same identities.
#[test]
fn undo_then_reimport_converges_reserved_ids() {
    let root = scratch("undo");
    let wav = root.join("c.wav");
    write_fixture(&wav, 2, 48_000, 4_800).unwrap();
    let mut c = controller();
    let target = clip_target(&mut c);

    let out =
        import_verified_local(&mut c, &request(&root, &wav, Some(target), None), &cx()).unwrap();
    let import_id = out.import.import_id;
    let asset_id = out.import.commit.asset_id;
    let source_id = out.import.commit.source_id;
    let clip_id = out.import.commit.clip_id.unwrap();

    // Two undos: SetSourceState, then the import commit itself.
    assert!(c.undo().unwrap());
    assert!(c.undo().unwrap());
    assert!(c.live().applied_imports.is_empty());
    assert!(c.live().sources.is_empty());
    assert!(c.live().clips.is_empty());

    // The manifest still pins the original expected revision — a blind
    // retry is a typed stale revision, never a silent rebase.
    let err = import_verified_local(
        &mut c,
        &request(&root, &wav, Some(target), Some(import_id)),
        &cx(),
    )
    .unwrap_err();
    assert!(
        matches!(
            &err,
            VerifiedImportError::Import(LibImportError::StaleRevision { .. })
        ),
        "expected StaleRevision, got {err:?}"
    );

    // Explicit rebase re-arms the manifest; the retry converges onto
    // the reserved asset/source/clip identities.
    rebase_import(
        &ImportStore::new(root.join("staging"), root.join("media")),
        import_id,
        c.live_revision(),
        c.live(),
    )
    .unwrap();
    let again = import_verified_local(
        &mut c,
        &request(&root, &wav, Some(target), Some(import_id)),
        &cx(),
    )
    .unwrap();
    assert!(!again.import.reused);
    assert_eq!(again.import.import_id, import_id);
    assert_eq!(again.import.commit.asset_id, asset_id);
    assert_eq!(again.import.commit.source_id, source_id);
    assert_eq!(again.import.commit.clip_id, Some(clip_id));
    assert_eq!(c.live().sources[&source_id].state, SourceState::Verified);
    assert_eq!(c.live().applied_imports.len(), 1);

    let pcm = play(c.live(), &root.join("media"), source_id);
    assert_window_pcm(&pcm);
}

/// Saved HEAD → load → playback: the persisted `Verified` state and
/// the sha256 blob survive a full reload boundary.
#[test]
fn saved_head_load_replays() {
    let root = scratch("save");
    let wav = root.join("d.wav");
    write_fixture(&wav, 2, 48_000, 4_800).unwrap();
    let mut c = controller();
    let target = clip_target(&mut c);

    let out =
        import_verified_local(&mut c, &request(&root, &wav, Some(target), None), &cx()).unwrap();
    let source_id = out.import.commit.source_id;

    let mut store = DirectoryBundleStore::create(root.join("bundle")).unwrap();
    c.save(&mut store).unwrap();
    let mut reloaded = controller();
    reloaded.load(&store).unwrap();
    assert_eq!(reloaded.live_revision(), c.live_revision());
    let live = reloaded.live();
    assert_eq!(live.sources[&source_id].state, SourceState::Verified);
    assert_eq!(live.applied_imports.len(), 1);

    let pcm = play(live, &root.join("media"), source_id);
    assert_window_pcm(&pcm);
}

/// An interrupted job is inspectable, resumable through the entry,
/// and lands the commit + state advance — the manifest's reserved
/// identities carry through.
#[test]
fn interrupted_job_inspects_resumes_and_plays() {
    let root = scratch("interrupt");
    let wav = root.join("e.wav");
    write_fixture(&wav, 2, 48_000, 4_800).unwrap();
    let mut c = controller();

    // Publish succeeds, then the shared cancel trips before commit —
    // the job parks at Published with the blob manifest-protected.
    let target = clip_target(&mut c);
    let err = lib_import(
        &mut c,
        &wav,
        &root.join("staging"),
        &root.join("media"),
        Some(target),
        &CancelAfterPublish,
        &cx(),
    )
    .unwrap_err();
    assert!(
        matches!(
            &err,
            LibImportError::Provider(e) if e.kind() == ProviderErrorKind::Cancelled
        ),
        "expected cancelled, got {err:?}"
    );

    // inspect_imports surfaces the parked job; nothing was applied.
    let store = ImportStore::new(root.join("staging"), root.join("media"));
    let jobs = inspect_imports(&store, c.live()).unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(
        jobs[0].stage,
        synvelis_library::ImportStage::Published,
        "job parked at {:?}",
        jobs[0].stage
    );
    assert!(!jobs[0].applied);
    assert!(c.live().applied_imports.is_empty());
    assert!(c.live().sources.is_empty());

    // Resume through the entry — same file, same import id: the job
    // commits from Published without re-publishing, then advances.
    let out = import_verified_local(
        &mut c,
        &request(&root, &wav, None, Some(jobs[0].import_id)),
        &cx(),
    )
    .unwrap();
    assert!(!out.import.reused);
    assert!(out.verified_revision.is_some());
    assert!(out.import.commit.clip_id.is_some());
    let source_id = out.import.commit.source_id;
    assert_eq!(c.live().sources[&source_id].state, SourceState::Verified);

    let jobs = inspect_imports(&store, c.live()).unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].stage, synvelis_library::ImportStage::Committed);
    assert!(jobs[0].applied);

    let pcm = play(c.live(), &root.join("media"), source_id);
    assert_window_pcm(&pcm);
}

// ---------------------------------------------------------------- negatives

/// Missing blob and drifted bytes are distinct typed failures on the
/// sha256 path — never conflated with an unverified source.
#[test]
fn sha256_missing_and_mismatch_fail_typed() {
    let root = scratch("neg");
    let wav = root.join("f.wav");
    let bytes = write_fixture(&wav, 2, 48_000, 960).unwrap();
    let mut c = controller();

    let out = import_verified_local(&mut c, &request(&root, &wav, None, None), &cx()).unwrap();
    let source_id = out.import.commit.source_id;
    let digest = out.import.content_digest.to_string();
    let blob_path = out.import.blob.destination.clone();

    // Missing blob → MissingMedia on both resolve paths.
    fs::remove_file(&blob_path).unwrap();
    assert!(matches!(
        open_blob_verified(&root.join("media"), &digest),
        Err(ResolveError::MissingMedia { .. })
    ));
    assert!(matches!(
        prepare(
            c.live(),
            source_id,
            &root.join("media"),
            &prefetch(2, 48_000)
        ),
        Err(ResolveError::MissingMedia { .. })
    ));

    // Drifted bytes → HashMismatch (sha256 measured != recorded).
    fs::write(&blob_path, b"different bytes").unwrap();
    assert!(matches!(
        open_blob_verified(&root.join("media"), &digest),
        Err(ResolveError::HashMismatch { .. })
    ));
    assert!(matches!(
        prepare(
            c.live(),
            source_id,
            &root.join("media"),
            &prefetch(2, 48_000)
        ),
        Err(ResolveError::HashMismatch { .. })
    ));

    // Restore the true bytes → resolves again.
    fs::write(&blob_path, &bytes).unwrap();
    prepare(
        c.live(),
        source_id,
        &root.join("media"),
        &prefetch(2, 48_000),
    )
    .unwrap();
}

/// Opaque digest strings are never treated as content addresses —
/// resolution fails closed even when a decoy file sits at the
/// would-be flat path.
#[test]
fn opaque_digest_fails_closed() {
    let root = scratch("opaque");
    let media_root = root.join("media");
    let mut c = controller();

    // A decoy under the flat path must not be consulted.
    let blobs = media_root.join("blobs");
    fs::create_dir_all(&blobs).unwrap();
    fs::write(blobs.join("opaque-not-hex"), b"decoy").unwrap();

    assert!(matches!(
        open_blob_verified(&media_root, "opaque-not-hex"),
        Err(ResolveError::UnsupportedDigest { .. })
    ));
    // Malformed sha256 tags are opaque too — never namespaced.
    let malformed = format!("sha256:{}", "z".repeat(64));
    assert!(matches!(
        open_blob_verified(&media_root, &malformed),
        Err(ResolveError::UnsupportedDigest { .. })
    ));

    // The same fail-closed through the resolver's source gate.
    apply(
        &mut c,
        ProjectOp::RegisterAsset {
            digest: "opaque-not-hex".into(),
        },
    );
    let asset_id = *c.live().assets.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id,
            content_digest: "opaque-not-hex".into(),
        },
    );
    let source_id = *c.live().sources.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::SetSourceState {
            source_id,
            state: SourceState::Verified,
        },
    );
    assert!(matches!(
        prepare(c.live(), source_id, &media_root, &prefetch(2, 48_000)),
        Err(ResolveError::UnsupportedDigest { .. })
    ));
}

/// Non-regression: a `Crc32Fold` legacy digest still resolves the flat
/// `blobs/<digest>` namespace — the `import_local` path is untouched.
#[test]
fn crc32_fold_source_still_resolves() {
    let root = scratch("crc");
    let wav = root.join("g.wav");
    write_fixture(&wav, 1, 48_000, 960).unwrap();
    let mut c = controller();

    let out = import_local(
        &mut c,
        &ImportRequest {
            path: wav.clone(),
            staging_root: root.join("staging"),
            media_root: root.join("media"),
            resume: None,
        },
    )
    .unwrap();
    let digest = out.media.digest.clone();
    // 16 lowercase hex — the legacy fold digest namespace.
    assert_eq!(digest.len(), 16);
    assert!(
        digest
            .chars()
            .all(|ch| ch.is_ascii_hexdigit() && !ch.is_ascii_uppercase())
    );

    let blob = open_blob_verified(&root.join("media"), &digest).unwrap();
    assert_eq!(blob.path, root.join("media").join("blobs").join(&digest));

    prepare(
        c.live(),
        out.commit.source_id,
        &root.join("media"),
        &prefetch(1, 48_000),
    )
    .unwrap();
}
