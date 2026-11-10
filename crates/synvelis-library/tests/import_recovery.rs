//! ALIB-02 residual slice (synvelis#86): receipt→Save/HEAD recovery,
//! crash-at-stage convergence, stale-revision/target-loss honesty,
//! multi-window placement, and legacy CRC-blob coexistence.
//!
//! These fixtures cover the boundaries the stage machine alone does
//! not: the durable `applied_imports` receipt surviving save/load,
//! caller-owned `save` failures, undo-driven re-import, and the flat
//! legacy blob namespace next to `blobs/sha256/`.

mod common;

use synvelis_library::{
    AssetProvider, ImportError, ImportIntent, ImportStage, ImportStore, ImportTarget, LocalBlobDir,
    LocalFolderProvider, ObjectIdentity, ProjectBlobStore, StagedBlobPublisher, gc_orphans,
    import_verified, inspect_imports, rebase_import,
};
use synvelis_project::project::AssetDigest;
use synvelis_project::{
    ClipExtent, ClipPlacement, ClipPlayback, Command, CounterIdGenerator, ImportId, InMemoryStore,
    MusicalDuration, MusicalTick, PlaybackMapping, Project, ProjectController, ProjectId,
    ProjectOp, ProjectRevision, ProjectStore, SourceFrame, SourceRange, TimeAnchor, TrackId,
};

use common::{TempTree, aref, cx, pid};

fn store(tree: &TempTree) -> ImportStore {
    ImportStore::new(tree.join("staging"), tree.join("media"))
}

fn blobs(tree: &TempTree) -> LocalBlobDir {
    LocalBlobDir::new(store(tree).blob_dir())
}

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "p"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn cmd(c: &ProjectController, op: ProjectOp) -> Command<ProjectOp> {
    Command {
        expected_revision: c.live_revision(),
        op,
    }
}

fn intent(
    provider: &LocalFolderProvider,
    import_raw: u128,
    locator: &str,
    rev: u64,
) -> ImportIntent {
    let asset = aref(pid(1), locator);
    let pin = provider.pin(&asset, &cx()).expect("pin");
    ImportIntent {
        import_id: ImportId::new(import_raw),
        expected_project_revision: ProjectRevision(rev),
        source: pin,
        target: None,
    }
}

fn run(
    provider: &LocalFolderProvider,
    it: &ImportIntent,
    tree: &TempTree,
    c: &mut ProjectController,
) -> Result<synvelis_library::ImportOutcome, ImportError> {
    import_verified(
        provider,
        it,
        &store(tree),
        &blobs(tree),
        &StagedBlobPublisher,
        c,
        &cx(),
    )
}

fn musical_target(track: TrackId, lane: synvelis_project::LaneId) -> ImportTarget {
    ImportTarget {
        track_id: track,
        lane_id: lane,
        placement: ClipPlacement {
            anchor: TimeAnchor::Musical(MusicalTick::new(0)),
            extent: ClipExtent::Musical(MusicalDuration::new(960).unwrap()),
        },
        playback: ClipPlayback {
            source_region: SourceRange::try_new(
                SourceFrame::new(0).unwrap(),
                SourceFrame::new(960).unwrap(),
            )
            .unwrap(),
            mapping: PlaybackMapping::Natural,
        },
    }
}

/// A `ProjectStore` that fails its first `save` — the caller-owned
/// Save/HEAD fault boundary. Inner state stays untouched, so `head()`
/// honestly reports nothing was published.
struct FailOnceStore {
    inner: InMemoryStore,
    armed: bool,
}

impl ProjectStore for FailOnceStore {
    fn save(
        &mut self,
        project_id: &str,
        revision: ProjectRevision,
        bytes: &[u8],
    ) -> Result<String, synvelis_project::Error> {
        if self.armed {
            self.armed = false;
            return Err(synvelis_project::Error::Store(
                "injected save failure".into(),
            ));
        }
        self.inner.save(project_id, revision, bytes)
    }

    fn head(&self) -> Result<Option<String>, synvelis_project::Error> {
        self.inner.head()
    }

    fn read_snapshot(&self, name: &str) -> Result<Vec<u8>, synvelis_project::Error> {
        self.inner.read_snapshot(name)
    }
}

// ── receipt → save/load → replay ─────────────────────────────────

#[test]
fn receipt_survives_save_load_and_replays() {
    let tree = TempTree::new("rec-reload");
    let payload: Vec<u8> = (0..8_192u32).map(|i| (i % 251) as u8).collect();
    tree.write("a.bin", &payload);
    let provider = LocalFolderProvider::new(pid(1), tree.path()).unwrap();
    let mut c = controller();
    let it = intent(&provider, 101, "a.bin", 0);
    let first = run(&provider, &it, &tree, &mut c).expect("import");

    // Save/HEAD axis: the caller-owned save publishes the committed
    // live state; CommittedLive alone never implied SavedRevision.
    let mut store = InMemoryStore::new();
    let saved = c.save(&mut store).expect("save");
    assert_eq!(saved.revision, first.commit.revision);

    // Reload into a fresh controller — the durable receipt travels
    // with the snapshot.
    let mut c2 = controller();
    c2.load(&store).expect("load");
    assert_eq!(
        c2.live().applied_imports[&it.import_id].asset_id,
        first.commit.asset_id
    );

    // Replaying the same intent converges through the registry — no
    // re-staging, no duplicate registration, same identities.
    let replay = run(&provider, &it, &tree, &mut c2).expect("replay");
    assert!(replay.reused);
    assert_eq!(replay.commit.asset_id, first.commit.asset_id);
    assert_eq!(replay.commit.source_id, first.commit.source_id);
    assert_eq!(replay.content_digest, first.content_digest);
    assert_eq!(c2.live().applied_imports.len(), 1);
    assert_eq!(c2.live().assets.len(), 1);
    assert_eq!(c2.live().sources.len(), 1);
}

#[test]
fn save_failure_preserves_commit_and_recovers() {
    let tree = TempTree::new("rec-savefail");
    tree.write("a.bin", b"save-fault-bytes");
    let provider = LocalFolderProvider::new(pid(1), tree.path()).unwrap();
    let mut c = controller();
    let it = intent(&provider, 102, "a.bin", 0);
    let out = run(&provider, &it, &tree, &mut c).expect("import");
    assert!(c.saved().is_none());

    // The Save/HEAD fault: the store rejects the publish — the live
    // commit and its receipt are untouched, HEAD honestly stays empty.
    let mut failing = FailOnceStore {
        inner: InMemoryStore::new(),
        armed: true,
    };
    assert!(c.save(&mut failing).is_err());
    assert!(c.saved().is_none());
    assert!(failing.head().unwrap().is_none());
    assert!(c.live().applied_imports.contains_key(&it.import_id));

    // Recovery: a healthy store publishes; reload + replay converges.
    let mut good = InMemoryStore::new();
    c.save(&mut good).expect("save retry");
    let mut c2 = controller();
    c2.load(&good).expect("load");
    let replay = run(&provider, &it, &tree, &mut c2).expect("replay");
    assert!(replay.reused);
    assert_eq!(replay.commit.asset_id, out.commit.asset_id);
    assert_eq!(
        synvelis_project::head_revision(&good).unwrap(),
        Some(out.commit.revision)
    );
}

#[test]
fn undo_after_commit_reimport_converges() {
    let tree = TempTree::new("rec-undo");
    tree.write("a.bin", b"undo-bytes");
    let provider = LocalFolderProvider::new(pid(1), tree.path()).unwrap();
    let mut c = controller();
    let it = intent(&provider, 103, "a.bin", 0);
    let out = run(&provider, &it, &tree, &mut c).expect("import");

    // Undo removes the live commit (asset/source/receipt) — the
    // manifest stays `Committed` and the published blob is retained
    // for exactly this replay.
    assert!(c.undo().expect("undo"));
    assert!(c.live().applied_imports.is_empty());
    assert!(c.live().assets.is_empty());

    // The persisted revision expectation is now stale — resolve only
    // through the explicit rebase, then the same import re-commits
    // with its reserved identities.
    let live_rev = c.live_revision();
    let err = run(&provider, &it, &tree, &mut c).unwrap_err();
    assert!(matches!(err, ImportError::StaleRevision { .. }), "{err:?}");
    rebase_import(&store(&tree), it.import_id, live_rev, c.live()).expect("rebase");
    let again = run(&provider, &it, &tree, &mut c).expect("re-import");
    assert!(!again.reused);
    assert_eq!(again.commit.asset_id, out.commit.asset_id);
    assert_eq!(again.commit.source_id, out.commit.source_id);
    assert_eq!(c.live().applied_imports.len(), 1);
    assert_eq!(c.live().assets.len(), 1);

    // GC now sees a confirmed committed+applied job — the staging
    // directory is a legitimate delete candidate.
    let report = gc_orphans(&store(&tree), c.live()).expect("gc");
    assert!(report.deleted_staging.contains(&it.import_id));
}

// ── stale revision → target loss → honest rejection ──────────────

#[test]
fn stale_then_target_gone_rejects_and_orphan_protected() {
    let tree = TempTree::new("rec-targetgone");
    tree.write("a.bin", b"target-gone-bytes");
    let provider = LocalFolderProvider::new(pid(1), tree.path()).unwrap();
    let mut c = controller();

    // Two windows' worth of structure: the target lane lives on
    // track B while track A stays untouched throughout.
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "a".into() }))
        .unwrap();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "b".into() }))
        .unwrap();
    let tracks: Vec<TrackId> = c.live().tracks.keys().copied().collect();
    let (track_a, track_b) = (tracks[0], tracks[1]);
    c.apply(cmd(&c, ProjectOp::AddLane { track_id: track_a }))
        .unwrap();
    c.apply(cmd(&c, ProjectOp::AddLane { track_id: track_b }))
        .unwrap();
    let lanes: Vec<synvelis_project::LaneId> = c.live().lanes.keys().copied().collect();
    let (lane_a, lane_b) = (lanes[0], lanes[1]);

    let mut it = intent(&provider, 104, "a.bin", c.live_revision().0);
    it.target = Some(musical_target(track_b, lane_b));

    // An unrelated commit lands between intent and import — the
    // manifest's revision expectation goes stale only at commit time,
    // after the blob is already published (the crash-adjacent window).
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "c".into() }))
        .unwrap();
    let err = run(&provider, &it, &tree, &mut c).unwrap_err();
    assert!(matches!(err, ImportError::StaleRevision { .. }), "{err:?}");
    // Published-but-uncommitted: the blob is a protected orphan and
    // the manifest honestly records the Published stage.
    let jobs = inspect_imports(&store(&tree), c.live()).unwrap();
    assert_eq!(jobs[0].stage, ImportStage::Published);

    // The target lane disappears before the explicit rebase — the
    // re-commit must reject honestly rather than misplace the clip.
    c.apply(cmd(&c, ProjectOp::RemoveLane { lane_id: lane_b }))
        .unwrap();
    let live_rev = c.live_revision();
    rebase_import(&store(&tree), it.import_id, live_rev, c.live()).expect("rebase");
    let err = run(&provider, &it, &tree, &mut c).unwrap_err();
    match err {
        ImportError::CommitRejected { source, .. } => {
            assert!(
                matches!(source, synvelis_project::Error::NotFound(_)),
                "{source}"
            );
        }
        other => panic!("expected CommitRejected, got {other:?}"),
    }
    assert!(c.live().applied_imports.is_empty());
    assert!(c.live().clips.is_empty());
    assert_eq!(c.live().lanes[&lane_a].clip_ids.len(), 0);

    // The published blob stays protected by its manifest — GC keeps
    // it as an active-import orphan, never silently deletes.
    let report = gc_orphans(&store(&tree), c.live()).expect("gc");
    assert_eq!(report.deleted_blobs.len(), 0);
    assert!(
        report
            .retained_blobs
            .iter()
            .any(|(_, r)| matches!(r, synvelis_library::BlobRetainReason::ActiveImport))
    );
}

#[test]
fn multi_track_placement_lands_on_intended_lane_only() {
    let tree = TempTree::new("rec-placetrack");
    tree.write("a.bin", b"placement-bytes");
    let provider = LocalFolderProvider::new(pid(1), tree.path()).unwrap();
    let mut c = controller();

    // Two windows/tracks — the clip must land on track B's lane and
    // nowhere else.
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "a".into() }))
        .unwrap();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "b".into() }))
        .unwrap();
    let tracks: Vec<TrackId> = c.live().tracks.keys().copied().collect();
    let (track_a, track_b) = (tracks[0], tracks[1]);
    c.apply(cmd(&c, ProjectOp::AddLane { track_id: track_a }))
        .unwrap();
    c.apply(cmd(&c, ProjectOp::AddLane { track_id: track_b }))
        .unwrap();
    let lanes: Vec<synvelis_project::LaneId> = c.live().lanes.keys().copied().collect();
    let (lane_a, lane_b) = (lanes[0], lanes[1]);
    assert_eq!(c.live().lanes[&lane_a].track_id, track_a);
    assert_eq!(c.live().lanes[&lane_b].track_id, track_b);

    let mut it = intent(&provider, 105, "a.bin", c.live_revision().0);
    it.target = Some(musical_target(track_b, lane_b));
    let out = run(&provider, &it, &tree, &mut c).expect("import");
    let clip_id = out.commit.clip_id.expect("clip committed");

    let clip = &c.live().clips[&clip_id];
    assert_eq!(clip.lane_id, lane_b);
    assert_eq!(clip.source_id, out.commit.source_id);
    assert_eq!(c.live().lanes[&lane_b].clip_ids, vec![clip_id]);
    assert_eq!(c.live().lanes[&lane_a].clip_ids.len(), 0);
    // Track A's lane list and clip set are untouched.
    assert!(c.live().tracks[&track_a].lane_ids.contains(&lane_a));
}

// ── legacy CRC blob namespace coexistence ────────────────────────

#[test]
fn legacy_crc_blob_coexists_with_sha256_namespace() {
    let tree = TempTree::new("rec-legacy");
    // A v5-era flat blob: `blobs/<crc16>` holding opaque legacy bytes.
    let legacy_bytes = b"legacy-crc-payload".to_vec();
    let crc_digest = "0123456789abcdef";
    let flat = store(&tree).blob_dir().join(crc_digest);
    std::fs::create_dir_all(flat.parent().unwrap()).unwrap();
    std::fs::write(&flat, &legacy_bytes).unwrap();

    // The project references it through the same opaque digest string
    // a v5 snapshot would carry.
    let mut c = controller();
    c.apply(cmd(
        &c,
        ProjectOp::RegisterAsset {
            digest: crc_digest.into(),
        },
    ))
    .unwrap();
    let asset = *c.live().assets.keys().next_back().unwrap();
    c.apply(cmd(
        &c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: crc_digest.into(),
        },
    ))
    .unwrap();

    // Typed classification: the flat CRC digest is weak, the new
    // import's digest is the strong content address.
    assert_eq!(
        AssetDigest::parse(crc_digest).kind(),
        synvelis_project::project::DigestKind::Crc32Fold
    );

    // The flat blob still resolves through the read port — legacy
    // assets remain readable (lossless), classified weak.
    let obj = blobs(&tree)
        .open(&ObjectIdentity::new(crc_digest).unwrap())
        .expect("legacy blob open");
    assert_eq!(obj.byte_len, legacy_bytes.len() as u64);

    // A new verified import lands in the sha256 namespace without
    // touching the flat file.
    tree.write("a.bin", b"new-sha256-bytes");
    let provider = LocalFolderProvider::new(pid(1), tree.path()).unwrap();
    let it = intent(&provider, 106, "a.bin", c.live_revision().0);
    let out = run(&provider, &it, &tree, &mut c).expect("import");
    assert_eq!(
        AssetDigest::parse(&out.content_digest.to_string()).kind(),
        synvelis_project::project::DigestKind::Sha256
    );
    assert!(flat.exists());

    // GC sweeps only `blobs/sha256/` — the flat legacy blob is never
    // a deletion candidate even when unreferenced by name.
    let report = gc_orphans(&store(&tree), c.live()).expect("gc");
    assert!(flat.exists());
    assert!(
        report
            .retained_blobs
            .iter()
            .any(|(id, r)| id == &out.content_digest.to_string()
                && matches!(r, synvelis_library::BlobRetainReason::ReferencedByProject))
    );
}
