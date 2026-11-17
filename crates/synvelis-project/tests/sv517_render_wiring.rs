//! synvelis#517 — production render/branch pipeline wiring.
//!
//! Pins the three `synvelis-project` contracts this slice ships:
//!
//! - `DirectoryBranchContentStore` — the durable, content-addressed
//!   `BranchContentStore` behind `VariantPorts::standard(Some(root))`:
//!   payloads survive a fresh store instance (process restart), writes
//!   are digest-verified and idempotent, and a corrupted payload file
//!   fails closed instead of being trusted.
//! - `JournaledRuntimePublish` — the production `RuntimePublish`: every
//!   request is durably journaled and reported as typed `Pending`
//!   (never `Published` — no runtime confirmed activation), `Failed`
//!   when even the journal write fails, and the pending record is
//!   visible to a fresh port instance (restart-visible last-good
//!   ledger).
//! - VCS-RESTORE-10 — `restore_as_copy` fork-copies every `Captured`
//!   branch payload referenced by the snapshot into the restored
//!   project's `branch-content/` store, digest-verified; payloads that
//!   cannot be sourced are receipt-reported `unavailable` (the honest
//!   `CandidateUnavailable` verdict) instead of failing or fabricating
//!   the restore.

use std::path::PathBuf;

use synvelis_project::apply::ApplyApproval;
use synvelis_project::controller::{Command, ProjectController};
use synvelis_project::id::{BranchId, CounterIdGenerator, ProjectId, TrackId, VariantSetId};
use synvelis_project::ops::ProjectOp;
use synvelis_project::project::Project;
use synvelis_project::range::MusicalRange;
use synvelis_project::restore::{
    MissingAssetsPolicy, RestoreBlobSource, RestorePorts, RestoreRequest, restore_as_copy,
};
use synvelis_project::snapshot::{
    BlobProbe, BlobRead, ContentDigest, ImmutableSnapshot, SnapshotCancel, SnapshotResolver,
};
use synvelis_project::time::MusicalTick;
use synvelis_project::variant::{
    AdoptIntent, AdoptSources, ArrangementDomainMask, BranchContentRef, BranchContentStore,
    DirectoryBranchContentStore, FakeDomainAdapter, ForkReceipt, InMemoryBranchContentStore,
    JournaledRuntimePublish, NoopRuntimePublish, RuntimePublish, RuntimePublishRequest,
    RuntimePublishState, VariantScope, adopt_branch, decode_candidate, fork_variant_set,
    scope_digest,
};

// ── shared fixtures ──────────────────────────────────────────────

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-sv517-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "fixture"),
        Box::new(CounterIdGenerator::new(1)),
    )
}

fn apply_op(c: &mut ProjectController, op: ProjectOp) {
    let rev = c.live_revision();
    c.apply(Command {
        expected_revision: rev,
        op,
    })
    .unwrap();
}

struct AllPresentProbe;

impl BlobProbe for AllPresentProbe {
    fn probe(&self, _digest: &str) -> BlobRead {
        BlobRead::Present(vec![0])
    }
}

fn snap_live(c: &ProjectController) -> ImmutableSnapshot {
    SnapshotResolver::new()
        .capture_live(
            c,
            c.live_revision(),
            &SnapshotCancel::new(),
            Some(&AllPresentProbe),
        )
        .unwrap()
}

struct EmptyBlobs;

impl RestoreBlobSource for EmptyBlobs {
    fn read_blob(&self, _digest: &str, _cancel: &SnapshotCancel) -> Result<BlobRead, String> {
        Ok(BlobRead::Absent)
    }
    fn stat_blob(&self, _digest: &str) -> Result<Option<u64>, String> {
        Ok(None)
    }
}

// ── variant fixture (mirrors vcs06_variant_matrix.rs) ────────────

fn arrangement_scope(track: TrackId) -> VariantScope {
    VariantScope::Arrangement {
        tracks: vec![track],
        time: MusicalRange::try_new(MusicalTick::new(0), MusicalTick::new(100_000)).unwrap(),
        fields: ArrangementDomainMask::ALL,
    }
}

fn fork_set(
    c: &mut ProjectController,
    store: &mut InMemoryBranchContentStore,
    scope: VariantScope,
) -> ForkReceipt {
    fork_variant_set(
        c,
        &FakeDomainAdapter::new(),
        store,
        &SnapshotResolver::new(),
        scope,
        &SnapshotCancel::new(),
    )
    .unwrap()
}

/// Track + fork + adopt — leaves a frozen `Captured` branch ref whose
/// payload bytes live only in `store`.
fn variant_project() -> (
    ProjectController,
    InMemoryBranchContentStore,
    VariantSetId,
    BranchId,
) {
    let mut c = controller();
    apply_op(&mut c, ProjectOp::AddTrack { name: "vox".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    let scope = arrangement_scope(track);
    let mut store = InMemoryBranchContentStore::default();
    let fork = fork_set(&mut c, &mut store, scope.clone());
    let set_state = &c.live().variant_sets[&fork.variant_set];
    let intent = AdoptIntent {
        variant_set: fork.variant_set,
        branch: fork.candidate,
        expected_project_revision: c.live_revision(),
        expected_branch_revision: set_state.branches[&fork.candidate].content.revision(),
        expected_scope_digest: scope_digest(c.live(), &scope).unwrap(),
        approval: ApplyApproval {
            ticket: "approve".into(),
        },
    };
    let sources = AdoptSources {
        assets: None,
        plugins: None,
        runtime: &NoopRuntimePublish,
    };
    adopt_branch(&mut c, &mut store, &sources, &intent).unwrap();
    (c, store, fork.variant_set, fork.candidate)
}

fn restore_request(snap: ImmutableSnapshot, dest: PathBuf) -> RestoreRequest {
    RestoreRequest {
        restore_id: "sv517-r1".into(),
        snapshot: snap,
        output_dir: dest,
        new_project_id: ProjectId::new(777),
        missing_assets: MissingAssetsPolicy::Fail,
    }
}

// ── (a) DirectoryBranchContentStore ──────────────────────────────

#[test]
fn directory_store_persists_payloads_across_instances() {
    let root = tmp_dir("store").join("branch-content");
    let payload = b"candidate-project-bytes".to_vec();
    let digest = ContentDigest::sha256(&payload);

    let mut first = DirectoryBranchContentStore::new(&root);
    first.put(&digest, &payload).unwrap();
    // Idempotent re-put of the same digest is accepted.
    first.put(&digest, &payload).unwrap();

    // A fresh instance — the process-restart shape — reads the same
    // bytes back by digest alone.
    let second = DirectoryBranchContentStore::new(&root);
    assert_eq!(second.get(&digest).unwrap(), Some(payload));
    assert_eq!(second.get(&ContentDigest::sha256(b"other")).unwrap(), None);

    // put rejects bytes that do not match the declared digest.
    let mut third = DirectoryBranchContentStore::new(&root);
    assert!(third.put(&digest, b"forged").is_err());
}

#[test]
fn directory_store_fails_closed_on_corrupted_payload() {
    let root = tmp_dir("store-corrupt").join("branch-content");
    let payload = b"honest-bytes".to_vec();
    let digest = ContentDigest::sha256(&payload);
    let mut store = DirectoryBranchContentStore::new(&root);
    store.put(&digest, &payload).unwrap();

    // Corrupt the published file in place.
    let hex = synvelis_project::plugin_state::digest_hex(&digest);
    std::fs::write(root.join(&hex), b"tampered").unwrap();

    // An idempotent re-put must not trust the digest-named file.
    assert!(store.put(&digest, &payload).is_err());
}

// ── (b) JournaledRuntimePublish ──────────────────────────────────

fn publish_request() -> RuntimePublishRequest {
    RuntimePublishRequest {
        project_id: ProjectId::new(1),
        revision: synvelis_project::controller::ProjectRevision(7),
        variant_set: VariantSetId::new(3),
        branch: BranchId::new(4),
        scope_digest: ContentDigest::sha256(b"scope"),
    }
}

#[test]
fn journaled_publish_reports_typed_pending_and_survives_restart() {
    let dir = tmp_dir("runtime-publish");
    let port = JournaledRuntimePublish::new(&dir);
    let evidence = port.publish(&publish_request());
    // Never Published — no runtime confirmed activation; the honest
    // state is Pending with the last-good semantics recorded.
    assert!(
        matches!(evidence.state, RuntimePublishState::Pending { .. }),
        "got {:?}",
        evidence.state
    );
    // The journal record is durable: a fresh port instance (restart)
    // sees the same pending ledger entry.
    let records: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.path().extension().is_some_and(|x| x == "json"))
        .collect();
    assert_eq!(records.len(), 1);
    let body = std::fs::read_to_string(records[0].path()).unwrap();
    assert!(body.contains("\"state\":\"pending\""), "got {body}");

    // Duplicate request → same digest-named record (idempotent).
    let again = port.publish(&publish_request());
    assert!(matches!(again.state, RuntimePublishState::Pending { .. }));
    assert_eq!(
        std::fs::read_dir(&dir)
            .unwrap()
            .filter(|e| e
                .as_ref()
                .unwrap()
                .path()
                .extension()
                .is_some_and(|x| x == "json"))
            .count(),
        1
    );
}

#[test]
fn journaled_publish_reports_failed_when_journal_unwritable() {
    // A journal path that is an existing *file* can never become a
    // directory — every publish must report typed Failed, never panic
    // or fabricate Pending/Published.
    let parent = tmp_dir("runtime-fail");
    let blocker = parent.join("not-a-dir");
    std::fs::write(&blocker, b"occupied").unwrap();
    let port = JournaledRuntimePublish::new(blocker.join("journal"));
    let evidence = port.publish(&publish_request());
    assert!(
        matches!(evidence.state, RuntimePublishState::Failed { .. }),
        "got {:?}",
        evidence.state
    );
}

// ── (d) VCS-RESTORE-10 fork-copy ─────────────────────────────────

#[test]
fn restore_as_copy_fork_copies_captured_branch_payloads() {
    let (c, store, _set, _branch) = variant_project();
    let snap = snap_live(&c);
    // Sanity: the snapshot really references at least one Captured
    // payload — otherwise this test proves nothing.
    // The destination must not pre-exist (restore publishes it).
    let dest = tmp_dir("restore-copy").join("dest");
    let blobs = EmptyBlobs;
    let ports = RestorePorts {
        blobs: &blobs,
        plugin_state: None,
        storage: None,
        branch_content: Some(&store),
    };
    let receipt = restore_as_copy(
        &restore_request(snap.clone(), dest.clone()),
        &ports,
        &SnapshotCancel::new(),
    )
    .expect("restore must succeed");

    let report = &receipt.branch_content;
    assert!(
        report.referenced > 0,
        "fixture must reference Captured payloads"
    );
    assert_eq!(report.copied, report.referenced);
    assert_eq!(report.unavailable, Vec::<String>::new());

    // The payload is materialized under the restored project's own
    // content-addressed store and resolves through decode_candidate.
    let dest_store = DirectoryBranchContentStore::new(dest.join("branch-content"));
    let set = &snap
        .projection
        .project()
        .variant_sets
        .values()
        .next()
        .unwrap();
    let branch = set.branches.values().next().unwrap();
    let BranchContentRef::Captured { .. } = &branch.content else {
        panic!("expected a Captured branch ref");
    };
    decode_candidate(&dest_store, &branch.content)
        .expect("fork-copied payload must decode in the restored project");
}

#[test]
fn restore_as_copy_reports_unavailable_candidates_without_source() {
    let (c, _store, _set, _branch) = variant_project();
    let snap = snap_live(&c);
    let dest = tmp_dir("restore-nocopy").join("dest");
    let blobs = EmptyBlobs;
    // No branch_content port — every referenced digest is the honest
    // CandidateUnavailable verdict on the receipt.
    let ports = RestorePorts {
        blobs: &blobs,
        plugin_state: None,
        storage: None,
        branch_content: None,
    };
    let receipt = restore_as_copy(
        &restore_request(snap, dest.clone()),
        &ports,
        &SnapshotCancel::new(),
    )
    .expect("restore must still succeed — absence is per-reference");
    let report = &receipt.branch_content;
    assert!(report.referenced > 0);
    assert_eq!(report.copied, 0);
    assert_eq!(report.unavailable.len() as u64, report.referenced);
    assert!(!dest.join("branch-content").exists());
}
