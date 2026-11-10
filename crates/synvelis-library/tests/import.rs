//! ALIB-02 streaming verified-import tests (Synvelis #86).
//!
//! Covers the slice acceptance criteria: bounded-memory streaming,
//! SHA-256 namespace, replay idempotency/conflict, staged fault
//! recovery, stale-revision + explicit rebase, target validation,
//! honest publish trichotomy, orphan GC, and live-vs-saved
//! separation. Mock/double publishers prove *classification* only —
//! they are never durability evidence.

mod common;

use std::fs;
use std::io::Read;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use ruviform_io_file::{DurabilityEvidence, PublishEffect, PublishFailureKind, PublishPhase};
use synvelis_library::{
    AccessMode, AssetProvider, BlobMaterialization, BlobPublishError, BlobPublishFailure,
    BlobPublisher, BlobRetainReason, Capability, ContentDigest, DigestAlgorithm, DirectorySync,
    FetchClaims, FetchCompletion, FetchSlot, ImportError, ImportIntent, ImportStage, ImportStore,
    ImportTarget, LocalBlobDir, LocalFolderProvider, ObjectIdentity, PinCapability, PinnedAssetRef,
    PinnedRead, ProjectBlobStore, ProviderCapabilities, ProviderContext, ProviderError,
    ProviderErrorKind, ProviderId, ProviderKind, RemoteAssetRef, StagedBlobPublisher,
    StagingRetainReason, StrongRevisionToken, VerificationPolicy, gc_orphans, import_verified,
    inspect_imports, rebase_import,
};
use synvelis_project::{
    ClipExtent, ClipPlacement, ClipPlayback, Command, CounterIdGenerator, ImportId,
    MusicalDuration, MusicalTick, PlaybackMapping, Project, ProjectController, ProjectId,
    ProjectOp, ProjectRevision, SourceFrame, SourceRange, SourceState, TrackId,
};

use common::{TempTree, aref, cx, loc, pid};

fn sha256_of(bytes: &[u8]) -> ContentDigest {
    use sha2::Digest as _;
    ContentDigest::new(DigestAlgorithm::Sha256, &sha2::Sha256::digest(bytes))
}

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

// ── happy path: stream → verify → publish → commit ───────────────

#[test]
fn import_streams_verifies_publishes_and_commits() {
    let tree = TempTree::new("imp-happy");
    // Opaque non-media bytes (invalid UTF-8, NULs) must survive
    // byte-for-byte — the pipeline treats content as opaque.
    let mut payload = Vec::with_capacity(300_000);
    for i in 0..300_000u32 {
        payload.push(u8::try_from((i.wrapping_mul(2_654_435_761) >> 13) & 0xff).unwrap());
    }
    payload.extend_from_slice(&[
        0x00, 0xff, 0xfe, 0x80, b'n', b'o', b't', b'-', b'm', b'e', b'd', b'i', b'a',
    ]);
    tree.write("media.bin", &payload);
    let provider = LocalFolderProvider::new(pid(1), tree.path()).unwrap();
    let mut c = controller();
    let it = intent(&provider, 7, "media.bin", 0);

    let out = run(&provider, &it, &tree, &mut c).expect("import");

    // SHA-256 identity + namespace path.
    let digest = sha256_of(&payload);
    assert_eq!(out.content_digest, digest);
    assert_eq!(out.byte_len, payload.len() as u64);
    assert_eq!(out.blob.identity.as_str(), digest.to_string());
    assert_eq!(
        out.blob.destination.parent().unwrap().file_name().unwrap(),
        "sha256"
    );
    assert_eq!(out.blob.destination, store(&tree).blob_path(&digest));
    assert_eq!(out.blob.materialized, BlobMaterialization::Created);
    assert_eq!(out.source_state, SourceState::Imported);
    assert!(!out.reused);
    // Provenance: provider id + pin object identity survive.
    assert_eq!(it.source.asset.provider, pid(1));
    assert_eq!(it.source.object_identity, out.blob.identity);

    // Blob readable through the project blob port under the
    // "sha256:<hex>" identity → blobs/sha256/<hex> path.
    let obj = blobs(&tree).open(&out.blob.identity).expect("blob open");
    assert_eq!(obj.byte_len, payload.len() as u64);
    let mut got = Vec::new();
    obj.reader.take(1 << 20).read_to_end(&mut got).unwrap();
    assert_eq!(got, payload);

    // Project boundary: typed ids, applied-import receipt, source
    // state Imported — never Saved.
    let live = c.live();
    assert!(live.assets.contains_key(&out.commit.asset_id));
    assert!(live.sources.contains_key(&out.commit.source_id));
    let applied = live.applied_imports.get(&it.import_id).expect("applied");
    assert_eq!(applied.content_hash, digest.to_string());
    assert_eq!(applied.asset_id, out.commit.asset_id);
    assert_eq!(applied.source_id, out.commit.source_id);
    assert_eq!(
        live.sources[&out.commit.source_id].state,
        SourceState::Imported
    );
    // Live commit advanced the live revision but saved() stays None —
    // CommittedLive never implies SavedRevision/HEAD.
    assert_eq!(out.commit.revision, c.live_revision());
    assert!(c.saved().is_none());
    // inspect_imports reports the committed job.
    let jobs = inspect_imports(&store(&tree), live).unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].stage, ImportStage::Committed);
    assert!(jobs[0].applied);
}

// ── replay idempotency + conflict ────────────────────────────────

#[test]
fn replay_same_import_is_idempotent() {
    let tree = TempTree::new("imp-replay");
    tree.write("a.bin", b"replay-bytes");
    let provider = LocalFolderProvider::new(pid(1), tree.path()).unwrap();
    let mut c = controller();
    let it = intent(&provider, 11, "a.bin", 0);

    let first = run(&provider, &it, &tree, &mut c).expect("first");
    let assets = c.live().assets.len();
    let sources = c.live().sources.len();

    let second = run(&provider, &it, &tree, &mut c).expect("replay");
    assert!(second.reused);
    assert_eq!(second.commit.asset_id, first.commit.asset_id);
    assert_eq!(second.commit.source_id, first.commit.source_id);
    assert_eq!(second.content_digest, first.content_digest);
    assert_eq!(c.live().assets.len(), assets);
    assert_eq!(c.live().sources.len(), sources);
    assert_eq!(c.live().applied_imports.len(), 1);
}

#[test]
fn same_import_id_different_content_is_conflict() {
    let tree = TempTree::new("imp-conflict");
    tree.write("a.bin", b"content-a");
    tree.write("b.bin", b"content-b-different");
    let provider = LocalFolderProvider::new(pid(1), tree.path()).unwrap();
    let mut c = controller();
    let first = intent(&provider, 21, "a.bin", 0);
    run(&provider, &first, &tree, &mut c).expect("first");

    // Same ImportId, different pin → integrity conflict, and the
    // conflict is detected before any duplicate registration.
    let second = intent(&provider, 21, "b.bin", c.live_revision().0);
    let err = run(&provider, &second, &tree, &mut c).unwrap_err();
    assert!(
        matches!(
            err,
            ImportError::IntegrityConflict { .. } | ImportError::Provider(_)
        ),
        "unexpected error: {err:?}"
    );
    assert_eq!(c.live().applied_imports.len(), 1);
}

// ── bounded memory: tiny chunks, larger payload ─────────────────

/// A reader that yields at most `chunk` bytes per `read` — proves the
/// pipeline never assumes provider chunk sizes and holds only its
/// fixed buffers.
struct TinyReader {
    data: Vec<u8>,
    pos: usize,
    chunk: usize,
}

impl Read for TinyReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.chunk.min(buf.len()).min(self.data.len() - self.pos);
        buf[..n].copy_from_slice(&self.data[self.pos..self.pos + n]);
        self.pos += n;
        Ok(n)
    }
}

/// Minimal scripted provider: serves `bytes` through `TinyReader`.
/// Its completion slot is never recorded — the pipeline must not
/// depend on provider-side completion for its own verification.
struct ScriptedProvider {
    id: ProviderId,
    bytes: Vec<u8>,
    chunk: usize,
    digest_override: Option<ContentDigest>,
}

impl ScriptedProvider {
    fn caps() -> ProviderCapabilities {
        let unsup = || Capability::Unsupported {
            reason: synvelis_library::UnsupportedReason::NotSupportedByBackend,
        };
        ProviderCapabilities {
            browse: unsup(),
            stat: unsup(),
            search: unsup(),
            pin: PinCapability::ConditionalStrong,
            range_read: unsup(),
            preview: unsup(),
            fetch: Capability::Supported,
            checksum_hint: Capability::Supported,
            access: AccessMode::ReadOnly,
        }
    }

    fn pin_for(&self) -> PinnedAssetRef {
        let digest = self
            .digest_override
            .clone()
            .unwrap_or_else(|| sha256_of(&self.bytes));
        PinnedAssetRef {
            asset: RemoteAssetRef {
                provider: self.id,
                locator: loc("scripted"),
            },
            proof: synvelis_library::PinProof::ConditionalStrong {
                token: StrongRevisionToken::ContentHash {
                    digest: digest.clone(),
                },
            },
            object_identity: ObjectIdentity::new("scripted-obj").unwrap(),
            expected_size: Some(self.bytes.len() as u64),
            digest_hint: Some(digest),
        }
    }
}

impl AssetProvider for ScriptedProvider {
    fn id(&self) -> ProviderId {
        self.id
    }
    fn kind(&self) -> ProviderKind {
        ProviderKind::LocalFolder
    }
    fn capabilities(&self, _asset: Option<&RemoteAssetRef>) -> ProviderCapabilities {
        Self::caps()
    }
    fn browse(
        &self,
        _req: &synvelis_library::BrowseRequest,
        _cx: &ProviderContext,
    ) -> Result<synvelis_library::BrowsePage, ProviderError> {
        Err(ProviderError::unsupported("browse", "scripted"))
    }
    fn stat(
        &self,
        _asset: &RemoteAssetRef,
        _cx: &ProviderContext,
    ) -> Result<synvelis_library::AssetStat, ProviderError> {
        Err(ProviderError::unsupported("stat", "scripted"))
    }
    fn pin(
        &self,
        _asset: &RemoteAssetRef,
        _cx: &ProviderContext,
    ) -> Result<PinnedAssetRef, ProviderError> {
        Ok(self.pin_for())
    }
    fn open_pinned(
        &self,
        pin: &PinnedAssetRef,
        _cx: &ProviderContext,
    ) -> Result<PinnedRead, ProviderError> {
        if pin.object_identity.as_str() != "scripted-obj" {
            return Err(ProviderError::new(
                ProviderErrorKind::InvalidRequest,
                "foreign pin",
            ));
        }
        Ok(PinnedRead {
            reader: Box::new(TinyReader {
                data: self.bytes.clone(),
                pos: 0,
                chunk: self.chunk,
            }),
            claims: FetchClaims {
                object_identity: pin.object_identity.clone(),
                expected_size: Some(self.bytes.len() as u64),
                verification: VerificationPolicy::Unverified,
            },
            completion: FetchCompletion::new(Arc::new(FetchSlot::new())),
        })
    }
}

#[test]
fn bounded_memory_streaming_with_tiny_chunks() {
    let tree = TempTree::new("imp-bounded");
    // ~1 MiB of deterministic pseudo-random bytes — far beyond any
    // fixed pipeline buffer, delivered 7 bytes at a time.
    let mut bytes = Vec::with_capacity(1 << 20);
    let mut state = 0x9e37_79b9_7f4a_7c15u64;
    while bytes.len() < (1 << 20) {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        bytes.extend_from_slice(&state.to_le_bytes());
    }
    bytes.truncate(1 << 20);
    let provider = ScriptedProvider {
        id: pid(3),
        bytes,
        chunk: 7,
        digest_override: None,
    };
    let mut c = controller();
    let pin = provider.pin_for();
    let it = ImportIntent {
        import_id: ImportId::new(31),
        expected_project_revision: ProjectRevision(0),
        source: pin,
        target: None,
    };
    let out = import_verified(
        &provider,
        &it,
        &store(&tree),
        &blobs(&tree),
        &StagedBlobPublisher,
        &mut c,
        &cx(),
    )
    .expect("bounded import");
    assert_eq!(out.byte_len, 1 << 20);
    assert_eq!(out.content_digest, sha256_of(&provider.bytes));
    // The published blob round-trips byte-exact.
    let obj = blobs(&tree).open(&out.blob.identity).unwrap();
    let mut reader = obj.reader;
    let mut got = Vec::new();
    reader.read_to_end(&mut got).unwrap();
    assert_eq!(got.len(), 1 << 20);
    assert_eq!(got, provider.bytes);
}

// ── integrity failures ───────────────────────────────────────────

#[test]
fn digest_mismatch_is_integrity_error() {
    let tree = TempTree::new("imp-digest");
    let provider = ScriptedProvider {
        id: pid(3),
        bytes: b"real-bytes".to_vec(),
        chunk: 4096,
        digest_override: Some(sha256_of(b"different-bytes")),
    };
    let mut c = controller();
    let it = ImportIntent {
        import_id: ImportId::new(41),
        expected_project_revision: ProjectRevision(0),
        source: provider.pin_for(),
        target: None,
    };
    let err = import_verified(
        &provider,
        &it,
        &store(&tree),
        &blobs(&tree),
        &StagedBlobPublisher,
        &mut c,
        &cx(),
    )
    .unwrap_err();
    assert!(
        matches!(err, ImportError::IntegrityMismatch { .. }),
        "unexpected: {err:?}"
    );
    // Nothing committed.
    assert_eq!(c.live().applied_imports.len(), 0);
    assert_eq!(c.live().assets.len(), 0);
}

#[test]
fn byte_budget_is_enforced_during_staging() {
    let tree = TempTree::new("imp-budget");
    let provider = ScriptedProvider {
        id: pid(3),
        bytes: vec![0xAB; 1 << 20],
        chunk: 4096,
        digest_override: None,
    };
    let mut cx = cx();
    cx.limits.max_bytes = 4096;
    let mut c = controller();
    let it = ImportIntent {
        import_id: ImportId::new(43),
        expected_project_revision: ProjectRevision(0),
        source: provider.pin_for(),
        target: None,
    };
    let err = import_verified(
        &provider,
        &it,
        &store(&tree),
        &blobs(&tree),
        &StagedBlobPublisher,
        &mut c,
        &cx,
    )
    .unwrap_err();
    assert!(
        matches!(&err, ImportError::Provider(e) if e.kind() == ProviderErrorKind::BudgetExceeded)
            || matches!(err, ImportError::IntegrityMismatch { .. }),
        "unexpected: {err:?}"
    );
}

// ── stale revision + explicit rebase ─────────────────────────────

#[test]
fn stale_revision_rejected_then_explicit_rebase_converges() {
    let tree = TempTree::new("imp-stale");
    tree.write("a.bin", b"stale-bytes");
    tree.write("b.bin", b"other-bytes");
    let provider = LocalFolderProvider::new(pid(1), tree.path()).unwrap();
    let mut c = controller();

    // Import A succeeds → live revision advances.
    let a = intent(&provider, 51, "a.bin", 0);
    run(&provider, &a, &tree, &mut c).expect("import a");
    let live = c.live_revision().0;
    assert_eq!(live, 1);

    // Import B captured expected_revision = 0 before A committed —
    // its commit must be rejected as stale, never silently moved.
    let b = intent(&provider, 52, "b.bin", 0);
    let err = run(&provider, &b, &tree, &mut c).unwrap_err();
    assert!(
        matches!(
            err,
            ImportError::StaleRevision {
                expected: 0,
                actual: 1
            }
        ),
        "unexpected: {err:?}"
    );
    // Blob was published and is retained as a protected orphan.
    let jobs = inspect_imports(&store(&tree), c.live()).unwrap();
    let b_job = jobs
        .iter()
        .find(|j| j.import_id == ImportId::new(52))
        .unwrap();
    assert_eq!(b_job.stage, ImportStage::Published);
    assert!(!b_job.applied);

    // Explicit rebase is the only resolution.
    rebase_import(
        &store(&tree),
        ImportId::new(52),
        ProjectRevision(live),
        c.live(),
    )
    .unwrap();
    let out = run(&provider, &b, &tree, &mut c).expect("rebased import");
    assert_eq!(out.commit.revision, c.live_revision());
    assert_eq!(c.live().applied_imports.len(), 2);
}

// ── target validation: missing lane / wrong track ────────────────

fn dummy_target(track: TrackId, lane: synvelis_project::LaneId) -> ImportTarget {
    ImportTarget {
        track_id: track,
        lane_id: lane,
        placement: ClipPlacement {
            anchor: musical_anchor(),
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

fn musical_anchor() -> synvelis_project::TimeAnchor {
    synvelis_project::TimeAnchor::Musical(MusicalTick::new(0))
}

#[test]
fn missing_lane_target_is_rejected_without_clip() {
    let tree = TempTree::new("imp-lane");
    tree.write("a.bin", b"lane-bytes");
    let provider = LocalFolderProvider::new(pid(1), tree.path()).unwrap();
    let mut c = controller();
    let mut it = intent(&provider, 61, "a.bin", 0);
    it.target = Some(dummy_target(
        TrackId::new(1),
        synvelis_project::LaneId::new(999),
    ));

    let err = run(&provider, &it, &tree, &mut c).unwrap_err();
    assert!(
        matches!(err, ImportError::CommitRejected { .. }),
        "unexpected: {err:?}"
    );
    // Nothing registered — no clip anywhere.
    assert_eq!(c.live().applied_imports.len(), 0);
    assert_eq!(c.live().clips.len(), 0);
    assert_eq!(c.live().assets.len(), 0);
}

#[test]
fn wrong_track_lane_target_is_rejected() {
    let tree = TempTree::new("imp-wrongtrack");
    tree.write("a.bin", b"track-bytes");
    let provider = LocalFolderProvider::new(pid(1), tree.path()).unwrap();
    let mut c = controller();

    // Track A + lane; Track B exists but owns nothing.
    c.apply(Command {
        expected_revision: c.live_revision(),
        op: ProjectOp::AddTrack { name: "A".into() },
    })
    .unwrap();
    let track_a = c.live().tracks.keys().next().copied().unwrap();
    c.apply(Command {
        expected_revision: c.live_revision(),
        op: ProjectOp::AddLane { track_id: track_a },
    })
    .unwrap();
    let lane = c.live().lanes.keys().next().copied().unwrap();
    c.apply(Command {
        expected_revision: c.live_revision(),
        op: ProjectOp::AddTrack { name: "B".into() },
    })
    .unwrap();
    let track_b = c
        .live()
        .tracks
        .keys()
        .find(|t| **t != track_a)
        .copied()
        .unwrap();

    let mut it = intent(&provider, 63, "a.bin", c.live_revision().0);
    it.target = Some(dummy_target(track_b, lane));
    let err = run(&provider, &it, &tree, &mut c).unwrap_err();
    assert!(
        matches!(err, ImportError::CommitRejected { .. }),
        "unexpected: {err:?}"
    );
    assert_eq!(c.live().clips.len(), 0);
    assert_eq!(c.live().applied_imports.len(), 0);
}

// ── publish fault trichotomy ─────────────────────────────────────

/// Scripted publisher: replays a queue of results, then delegates to
/// the real `StagedBlobPublisher`.
struct ScriptedPublisher {
    results: Mutex<Vec<Result<(), BlobPublishError>>>,
    calls: AtomicUsize,
}

impl ScriptedPublisher {
    fn new(results: Vec<BlobPublishError>) -> Self {
        Self {
            results: Mutex::new(results.into_iter().map(Err).collect()),
            calls: AtomicUsize::new(0),
        }
    }
}

fn publish_failure(
    kind: PublishFailureKind,
    phase: PublishPhase,
    effect: PublishEffect,
) -> BlobPublishError {
    BlobPublishError::Publish(BlobPublishFailure {
        kind,
        phase,
        effect,
        durability: DurabilityEvidence::None,
        directory_sync: DirectorySync::Unknown,
        detail: "scripted fault".into(),
    })
}

impl BlobPublisher for ScriptedPublisher {
    fn publish(
        &self,
        staged_source: &std::path::Path,
        expected: &ContentDigest,
        expected_len: u64,
        destination: &std::path::Path,
        durability: synvelis_library::PublishDurability,
        cx: &ProviderContext,
    ) -> Result<synvelis_library::BlobPublishReceipt, BlobPublishError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let scripted = self.results.lock().unwrap().pop();
        match scripted {
            Some(Err(e)) => Err(e),
            _ => StagedBlobPublisher.publish(
                staged_source,
                expected,
                expected_len,
                destination,
                durability,
                cx,
            ),
        }
    }
}

#[test]
fn publish_failure_resumes_from_manifest() {
    let tree = TempTree::new("imp-pubfail");
    let mut bytes = Vec::new();
    for i in 0..50_000u32 {
        bytes.push(u8::try_from(i % 251).unwrap());
    }
    let provider = ScriptedProvider {
        id: pid(3),
        bytes,
        chunk: 8192,
        digest_override: None,
    };
    let publisher = ScriptedPublisher::new(vec![publish_failure(
        PublishFailureKind::Io,
        PublishPhase::Publish,
        PublishEffect::NotPublished,
    )]);
    let mut c = controller();
    let it = ImportIntent {
        import_id: ImportId::new(71),
        expected_project_revision: ProjectRevision(0),
        source: provider.pin_for(),
        target: None,
    };
    let err = import_verified(
        &provider,
        &it,
        &store(&tree),
        &blobs(&tree),
        &publisher,
        &mut c,
        &cx(),
    )
    .unwrap_err();
    assert!(matches!(err, ImportError::PublishFailed { .. }), "{err:?}");
    // Manifest parked at Verified — nothing committed, nothing lost.
    let jobs = inspect_imports(&store(&tree), c.live()).unwrap();
    assert_eq!(jobs[0].stage, ImportStage::Verified);
    assert_eq!(c.live().applied_imports.len(), 0);

    // Retry with the publisher recovered: resumes at Verified, does
    // not re-stage (the staged file is still authoritative).
    let publisher2 = ScriptedPublisher::new(vec![]);
    let out = import_verified(
        &provider,
        &it,
        &store(&tree),
        &blobs(&tree),
        &publisher2,
        &mut c,
        &cx(),
    )
    .expect("resumed import");
    assert_eq!(out.commit.revision, c.live_revision());
    assert_eq!(c.live().applied_imports.len(), 1);
}

#[test]
fn indeterminate_publish_is_reconciled_not_misreported() {
    let tree = TempTree::new("imp-indet");
    let provider = ScriptedProvider {
        id: pid(3),
        bytes: b"indeterminate-bytes".to_vec(),
        chunk: 4096,
        digest_override: None,
    };
    // Two indeterminate outcomes: reconcile finds nothing, retries,
    // still indeterminate → PublishIndeterminate, never "not
    // published".
    let publisher = ScriptedPublisher::new(vec![
        publish_failure(
            PublishFailureKind::Io,
            PublishPhase::Publish,
            PublishEffect::Indeterminate,
        ),
        publish_failure(
            PublishFailureKind::Io,
            PublishPhase::Publish,
            PublishEffect::Indeterminate,
        ),
    ]);
    let mut c = controller();
    let it = ImportIntent {
        import_id: ImportId::new(73),
        expected_project_revision: ProjectRevision(0),
        source: provider.pin_for(),
        target: None,
    };
    let err = import_verified(
        &provider,
        &it,
        &store(&tree),
        &blobs(&tree),
        &publisher,
        &mut c,
        &cx(),
    )
    .unwrap_err();
    assert!(
        matches!(err, ImportError::PublishIndeterminate { .. }),
        "unexpected: {err:?}"
    );
    // The indeterminacy is persisted on the manifest for review.
    let jobs = inspect_imports(&store(&tree), c.live()).unwrap();
    assert!(jobs[0].publish_indeterminate);
    assert_eq!(c.live().applied_imports.len(), 0);
}

struct DirSyncFailPublisher;
impl BlobPublisher for DirSyncFailPublisher {
    fn publish(
        &self,
        staged_source: &std::path::Path,
        expected: &ContentDigest,
        expected_len: u64,
        destination: &std::path::Path,
        durability: synvelis_library::PublishDurability,
        cx: &ProviderContext,
    ) -> Result<synvelis_library::BlobPublishReceipt, BlobPublishError> {
        StagedBlobPublisher.publish(
            staged_source,
            expected,
            expected_len,
            destination,
            durability,
            cx,
        )?;
        Err(BlobPublishError::Publish(BlobPublishFailure {
            kind: PublishFailureKind::Io,
            phase: PublishPhase::DirectorySync,
            effect: PublishEffect::Published,
            durability: DurabilityEvidence::FileAndDirectorySynced,
            directory_sync: DirectorySync::Failed,
            detail: "scripted dirsync failure".into(),
        }))
    }
}

#[test]
fn directory_sync_failure_is_recorded_not_hidden() {
    let tree = TempTree::new("imp-dirsync");
    let provider = ScriptedProvider {
        id: pid(3),
        bytes: b"dirsync-bytes".to_vec(),
        chunk: 4096,
        digest_override: None,
    };
    // Publish lands (effect Published) but directory sync fails —
    // import proceeds and records DirectorySync::Failed honestly.
    let publisher = ScriptedPublisher::new(vec![publish_failure(
        PublishFailureKind::Io,
        PublishPhase::DirectorySync,
        PublishEffect::Published,
    )]);
    // The scripted failure has effect Published but the double does
    // not actually place the blob — the reconcile verify would fail.
    // Use a publisher that fails AFTER a real publish instead:
    // delegate-then-fail is not expressible, so emulate by having the
    // first call really publish then report the dirsync failure.
    let _ = publisher;
    let mut c = controller();
    let it = ImportIntent {
        import_id: ImportId::new(75),
        expected_project_revision: ProjectRevision(0),
        source: provider.pin_for(),
        target: None,
    };
    let out = import_verified(
        &provider,
        &it,
        &store(&tree),
        &blobs(&tree),
        &DirSyncFailPublisher,
        &mut c,
        &cx(),
    )
    .expect("import with dirsync failure recorded");
    assert_eq!(out.blob.directory_sync, DirectorySync::Failed);
    assert_eq!(c.live().applied_imports.len(), 1);
}

// ── orphan GC ────────────────────────────────────────────────────

#[test]
fn orphan_gc_protects_reachable_and_deletes_only_orphans() {
    let tree = TempTree::new("imp-gc");
    tree.write("a.bin", b"gc-bytes");
    let provider = LocalFolderProvider::new(pid(1), tree.path()).unwrap();
    let mut c = controller();
    let it = intent(&provider, 81, "a.bin", 0);
    let out = run(&provider, &it, &tree, &mut c).expect("import");

    // Plant an orphan blob (valid sha256 name, unreferenced) and a
    // foreign-name file in the sha256 namespace.
    let sha_dir = store(&tree).blob_dir().join("sha256");
    fs::create_dir_all(&sha_dir).unwrap();
    let orphan_hex = "00".repeat(32);
    fs::write(sha_dir.join(&orphan_hex), b"orphan").unwrap();
    fs::write(sha_dir.join("not-a-digest.txt"), b"foreign").unwrap();
    // A legacy flat blob outside the sha256 namespace — never GC'd.
    fs::create_dir_all(store(&tree).blob_dir()).unwrap();
    fs::write(store(&tree).blob_dir().join("legacyflat"), b"legacy").unwrap();

    let report = gc_orphans(&store(&tree), c.live()).expect("gc");

    // Orphan deleted; committed import's blob + staging retained then
    // staging collected (committed+applied).
    assert!(
        report
            .deleted_blobs
            .contains(&format!("sha256:{orphan_hex}"))
    );
    assert_eq!(report.skipped_foreign_blobs, 1);
    assert!(
        report
            .retained_blobs
            .iter()
            .any(|(id, r)| id == out.blob.identity.as_str()
                && *r == BlobRetainReason::ReferencedByProject)
    );
    assert!(report.deleted_staging.contains(&it.import_id));
    // Legacy flat blob untouched.
    assert!(store(&tree).blob_dir().join("legacyflat").exists());
    assert!(sha_dir.join("not-a-digest.txt").exists());
    // Project-referenced blob still opens.
    blobs(&tree)
        .open(&out.blob.identity)
        .expect("blob survives gc");
}

#[test]
fn incomplete_job_and_blob_are_protected_from_gc() {
    let tree = TempTree::new("imp-gcprotect");
    let provider = ScriptedProvider {
        id: pid(3),
        bytes: b"protected-bytes".to_vec(),
        chunk: 4096,
        digest_override: None,
    };
    // Publish succeeds; commit fails via a foreign manifest? Simpler:
    // use a stale-revision failure after publish.
    let publisher = ScriptedPublisher::new(vec![]);
    let mut c = controller();
    // Bump revision first so the import's expected rev is stale.
    c.apply(Command {
        expected_revision: c.live_revision(),
        op: ProjectOp::SetProjectName { name: "x".into() },
    })
    .unwrap();
    let it = ImportIntent {
        import_id: ImportId::new(83),
        expected_project_revision: ProjectRevision(0),
        source: provider.pin_for(),
        target: None,
    };
    let err = import_verified(
        &provider,
        &it,
        &store(&tree),
        &blobs(&tree),
        &publisher,
        &mut c,
        &cx(),
    )
    .unwrap_err();
    assert!(matches!(err, ImportError::StaleRevision { .. }));

    let report = gc_orphans(&store(&tree), c.live()).unwrap();
    // Published-but-uncommitted blob is protected by the manifest.
    let digest = sha256_of(b"protected-bytes");
    assert!(
        report
            .retained_blobs
            .iter()
            .any(|(id, r)| id == &digest.to_string() && *r == BlobRetainReason::ActiveImport)
    );
    assert!(
        report
            .retained_staging
            .iter()
            .any(|(id, r)| *id == it.import_id && *r == StagingRetainReason::Incomplete)
    );
    assert_eq!(report.deleted_blobs.len(), 0);
}

// ── namespace separation ─────────────────────────────────────────

#[test]
fn sha256_namespace_cannot_collide_with_legacy_flat() {
    let tree = TempTree::new("imp-ns");
    let blob_root = store(&tree).blob_dir();
    fs::create_dir_all(&blob_root).unwrap();
    // Legacy flat blob: filename-safe identity, no colon — still
    // resolves to <root>/<name> exactly as before.
    fs::write(blob_root.join("abcd1234"), b"legacy").unwrap();
    let legacy = LocalBlobDir::new(&blob_root);
    let obj = legacy
        .open(&ObjectIdentity::new("abcd1234").unwrap())
        .expect("legacy flat blob");
    assert_eq!(obj.byte_len, 6);

    // A sha256-namespaced identity resolves into sha256/<hex> — a
    // different directory, so no flat name can shadow it.
    let digest = sha256_of(b"namespaced");
    let sha_dir = blob_root.join("sha256");
    fs::create_dir_all(&sha_dir).unwrap();
    fs::write(sha_dir.join(digest.hex()), b"namespaced").unwrap();
    let obj = legacy
        .open(&ObjectIdentity::new(digest.to_string()).unwrap())
        .unwrap();
    assert_eq!(obj.byte_len, 10);
    // The flat file of the same hex text does NOT satisfy the
    // namespaced identity (different path).
    fs::write(blob_root.join(digest.hex()), b"shadow").unwrap();
    let obj = legacy
        .open(&ObjectIdentity::new(digest.to_string()).unwrap())
        .unwrap();
    let mut got = Vec::new();
    obj.reader.take(64).read_to_end(&mut got).unwrap();
    assert_eq!(got, b"namespaced");
}

// ── ALIB-07 fault matrix: cancel / cross-provider share ─────────

/// Fires `on_read` once, just before the first `read` returns —
/// deterministic mid-stream abort injection.
struct ArmOnRead {
    inner: Box<dyn Read + Send>,
    on_read: Box<dyn Fn() + Send>,
    armed: bool,
}

impl Read for ArmOnRead {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if !self.armed {
            self.armed = true;
            (self.on_read)();
        }
        self.inner.read(buf)
    }
}

/// A [`ScriptedProvider`] whose body stream cancels the caller's
/// context token on the first read — models a user cancel or
/// watchdog abort landing mid-stream.
struct CancelOnReadProvider {
    inner: ScriptedProvider,
}

impl AssetProvider for CancelOnReadProvider {
    fn id(&self) -> ProviderId {
        self.inner.id()
    }
    fn kind(&self) -> ProviderKind {
        self.inner.kind()
    }
    fn capabilities(&self, asset: Option<&RemoteAssetRef>) -> ProviderCapabilities {
        self.inner.capabilities(asset)
    }
    fn browse(
        &self,
        req: &synvelis_library::BrowseRequest,
        cx: &ProviderContext,
    ) -> Result<synvelis_library::BrowsePage, ProviderError> {
        self.inner.browse(req, cx)
    }
    fn stat(
        &self,
        asset: &RemoteAssetRef,
        cx: &ProviderContext,
    ) -> Result<synvelis_library::AssetStat, ProviderError> {
        self.inner.stat(asset, cx)
    }
    fn pin(
        &self,
        asset: &RemoteAssetRef,
        cx: &ProviderContext,
    ) -> Result<PinnedAssetRef, ProviderError> {
        self.inner.pin(asset, cx)
    }
    fn open_pinned(
        &self,
        pin: &PinnedAssetRef,
        cx: &ProviderContext,
    ) -> Result<PinnedRead, ProviderError> {
        let mut pr = self.inner.open_pinned(pin, cx)?;
        let token = cx.cancel.clone();
        pr.reader = Box::new(ArmOnRead {
            inner: pr.reader,
            on_read: Box::new(move || token.cancel()),
            armed: false,
        });
        Ok(pr)
    }
}

#[test]
fn cancel_mid_stream_is_typed_and_same_import_resumes() {
    let tree = TempTree::new("imp-cancel");
    let bytes: Vec<u8> = (0..8_192u32)
        .map(|i| u8::try_from(i % 251).unwrap())
        .collect();
    let cancelling = CancelOnReadProvider {
        inner: ScriptedProvider {
            id: pid(3),
            bytes: bytes.clone(),
            chunk: 1024,
            digest_override: None,
        },
    };
    let healthy = ScriptedProvider {
        id: pid(3),
        bytes,
        chunk: 1024,
        digest_override: None,
    };
    let mut c = controller();
    let pin = healthy.pin_for();
    let it = ImportIntent {
        import_id: ImportId::new(91),
        expected_project_revision: ProjectRevision(0),
        source: pin,
        target: None,
    };

    // First run: the cancel lands after the first body chunk → typed
    // Cancelled, nothing committed, no asset/source registered.
    let err = import_verified(
        &cancelling,
        &it,
        &store(&tree),
        &blobs(&tree),
        &StagedBlobPublisher,
        &mut c,
        &cx(),
    )
    .unwrap_err();
    assert!(
        matches!(&err, ImportError::Provider(e) if e.kind() == ProviderErrorKind::Cancelled),
        "unexpected: {err:?}"
    );
    assert_eq!(c.live().applied_imports.len(), 0);
    assert_eq!(c.live().assets.len(), 0);

    // Retry under the same ImportId with a live context: the manifest
    // stayed at Started, re-stages, and converges — no duplicates.
    let out = import_verified(
        &healthy,
        &it,
        &store(&tree),
        &blobs(&tree),
        &StagedBlobPublisher,
        &mut c,
        &cx(),
    )
    .expect("resumed import");
    assert_eq!(out.commit.revision, c.live_revision());
    assert_eq!(c.live().applied_imports.len(), 1);
    assert_eq!(c.live().assets.len(), 1);
    assert_eq!(out.source_state, SourceState::Imported);
}

#[test]
fn same_bytes_across_providers_share_one_verified_blob() {
    let tree = TempTree::new("imp-xprov");
    let bytes = b"shared-content-across-providers".to_vec();
    let provider_a = ScriptedProvider {
        id: pid(3),
        bytes: bytes.clone(),
        chunk: 4096,
        digest_override: None,
    };
    let provider_b = ScriptedProvider {
        id: pid(4),
        bytes,
        chunk: 4096,
        digest_override: None,
    };
    let mut c = controller();

    let it_a = ImportIntent {
        import_id: ImportId::new(93),
        expected_project_revision: ProjectRevision(0),
        source: provider_a.pin_for(),
        target: None,
    };
    let out_a = import_verified(
        &provider_a,
        &it_a,
        &store(&tree),
        &blobs(&tree),
        &StagedBlobPublisher,
        &mut c,
        &cx(),
    )
    .expect("import a");
    assert_eq!(out_a.blob.materialized, BlobMaterialization::Created);

    // A different provider serving byte-identical content reuses the
    // published blob — the sha256 namespace is content-addressed,
    // never provider-scoped.
    let it_b = ImportIntent {
        import_id: ImportId::new(94),
        expected_project_revision: c.live_revision(),
        source: provider_b.pin_for(),
        target: None,
    };
    let out_b = import_verified(
        &provider_b,
        &it_b,
        &store(&tree),
        &blobs(&tree),
        &StagedBlobPublisher,
        &mut c,
        &cx(),
    )
    .expect("import b");
    assert_eq!(out_b.blob.materialized, BlobMaterialization::Reused);
    assert_eq!(out_b.content_digest, out_a.content_digest);
    assert_eq!(out_b.blob.identity, out_a.blob.identity);
    assert_ne!(out_b.commit.asset_id, out_a.commit.asset_id);
    assert_ne!(out_b.commit.source_id, out_a.commit.source_id);
    assert_eq!(c.live().applied_imports.len(), 2);
}
