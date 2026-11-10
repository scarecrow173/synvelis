//! VCS-05 Restore As New Copy — headless fixtures (synvelis#96).
//!
//! Covers staged publish/readback/replay, source invariants, and the
//! fail-closed destination/asset/disk/cancel paths. Real OS
//! filesystem durability certification, real provider E2E, and plugin
//! host restore are `NotRun` (synvelis#97).

use std::path::{Path, PathBuf};

use synvelis_project::controller::{Command, ProjectController};
use synvelis_project::id::{CounterIdGenerator, ProjectId};
use synvelis_project::ops::ProjectOp;
use synvelis_project::project::Project;
use synvelis_project::restore::{
    MissingAssetsPolicy, RestoreBlobSource, RestoreError, RestoreOutcome, RestorePorts,
    RestoreRequest, RestoreStorageProbe, restore_as_copy,
};
use synvelis_project::snapshot::{
    BlobProbe, BlobRead, ContentDigest, ImmutableSnapshot, SnapshotCancel, SnapshotResolver,
};

// ── fixtures ───────────────────────────────────────────────────────

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(1), "song"),
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

struct AbsentProbe;

impl BlobProbe for AbsentProbe {
    fn probe(&self, _digest: &str) -> BlobRead {
        BlobRead::Absent
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

/// Project with one real-digest asset + source + track/lane.
fn media_project() -> (ProjectController, String) {
    let mut c = controller();
    apply_op(
        &mut c,
        ProjectOp::AddTrack {
            name: "drums".into(),
        },
    );
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddLane { track_id: track });
    let bytes = b"fake-media-payload".to_vec();
    let digest = ContentDigest::sha256(&bytes).to_string();
    apply_op(
        &mut c,
        ProjectOp::RegisterAsset {
            digest: digest.clone(),
        },
    );
    let asset = *c.live().assets.keys().next().unwrap();
    apply_op(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id: asset,
            content_digest: digest.clone(),
        },
    );
    (c, digest)
}

/// Blob source serving `digest → bytes` (real sha256-verified).
struct MapBlobs {
    map: std::collections::BTreeMap<String, Vec<u8>>,
}

impl RestoreBlobSource for MapBlobs {
    fn read_blob(&self, digest: &str, _cancel: &SnapshotCancel) -> Result<BlobRead, String> {
        Ok(match self.map.get(digest) {
            Some(b) => BlobRead::Present(b.clone()),
            None => BlobRead::Absent,
        })
    }
    fn stat_blob(&self, digest: &str) -> Result<Option<u64>, String> {
        Ok(self.map.get(digest).map(|b| b.len() as u64))
    }
}

struct LowDisk;

impl RestoreStorageProbe for LowDisk {
    fn available_bytes(&self, _dir: &Path) -> Result<Option<u64>, String> {
        Ok(Some(0))
    }
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-vcs05-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn request(snapshot: ImmutableSnapshot, output_dir: PathBuf, restore_id: &str) -> RestoreRequest {
    RestoreRequest {
        restore_id: restore_id.into(),
        snapshot,
        output_dir,
        new_project_id: ProjectId::new(777),
        missing_assets: MissingAssetsPolicy::Fail,
    }
}

// ── VCS-RESTORE-01: publish + readback + replay ────────────────────

#[test]
fn restore_publishes_verified_new_copy() {
    let (c, digest) = media_project();
    let snap = SnapshotResolver::new()
        .capture_live(
            &c,
            c.live_revision(),
            &SnapshotCancel::new(),
            Some(&AllPresentProbe),
        )
        .unwrap();
    let parent = tmp_dir("pub");
    let dest = parent.join("copy-a");
    let blobs = MapBlobs {
        map: [(digest.clone(), b"fake-media-payload".to_vec())]
            .into_iter()
            .collect(),
    };
    let ports = RestorePorts {
        blobs: &blobs,
        plugin_state: None,
        storage: None,
    };
    let rev_before = c.live_revision();
    let pid_before = c.live().project_id;

    let receipt = restore_as_copy(
        &request(snap.clone(), dest.clone(), "r-1"),
        &ports,
        &SnapshotCancel::new(),
    )
    .unwrap();

    assert_eq!(receipt.outcome, RestoreOutcome::Published);
    assert_eq!(receipt.new_project_id, ProjectId::new(777));
    assert_ne!(receipt.source_project_id, receipt.new_project_id);
    assert!(receipt.effects.source.project_unchanged);
    assert!(receipt.effects.source.saved_head_unchanged);
    assert!(receipt.effects.git.index_untouched);
    assert!(receipt.effects.git.worktree_untouched);
    assert!(!receipt.effects.copy.in_place_restore);
    assert!(!receipt.effects.copy.degraded_copy);
    assert_eq!(receipt.assets.copied, 1);
    assert!(
        receipt
            .stable_ids
            .counts
            .get("track")
            .is_some_and(|n| *n >= 1)
    );

    // Bundle layout: HEAD → snapshot, blob at content-addressed path,
    // receipt + manifest inside the published tree.
    let head = std::fs::read_to_string(dest.join("HEAD")).unwrap();
    let snap_bytes = std::fs::read(dest.join("snapshots").join(head.trim())).unwrap();
    let envelope: synvelis_project::store::SnapshotEnvelope =
        serde_json::from_slice(&snap_bytes).unwrap();
    assert_eq!(envelope.project_id, ProjectId::new(777));
    assert_eq!(envelope.project.project_id, ProjectId::new(777));
    assert_eq!(envelope.project.name, "song");
    assert_eq!(envelope.project.tracks.len(), 1);
    let hex = digest.trim_start_matches("sha256:");
    assert!(dest.join("blobs").join("sha256").join(hex).exists());
    assert!(dest.join("restore-receipt.json").exists());
    assert!(dest.join("restore.json").exists());

    // Source untouched.
    assert_eq!(c.live_revision(), rev_before);
    assert_eq!(c.live().project_id, pid_before);

    // Replay: same request → Replayed receipt, no error on existing dest.
    let replay = restore_as_copy(
        &request(snap, dest.clone(), "r-1"),
        &ports,
        &SnapshotCancel::new(),
    )
    .unwrap();
    assert_eq!(replay.outcome, RestoreOutcome::Replayed);
    assert_eq!(replay.new_project_id, receipt.new_project_id);

    let _ = std::fs::remove_dir_all(&parent);
}

// ── VCS-RESTORE-02: fail-closed paths ──────────────────────────────

#[test]
fn missing_asset_fails_and_leaves_no_destination() {
    let (c, _digest) = media_project();
    // Snapshot whose asset probe reports Absent → status Missing.
    let snap = SnapshotResolver::new()
        .capture_live(
            &c,
            c.live_revision(),
            &SnapshotCancel::new(),
            Some(&AbsentProbe),
        )
        .unwrap();
    let parent = tmp_dir("miss");
    let dest = parent.join("copy-b");
    let blobs = MapBlobs {
        map: std::collections::BTreeMap::default(),
    };
    let ports = RestorePorts {
        blobs: &blobs,
        plugin_state: None,
        storage: None,
    };
    let err = restore_as_copy(
        &request(snap, dest.clone(), "r-2"),
        &ports,
        &SnapshotCancel::new(),
    )
    .unwrap_err();
    assert!(matches!(err, RestoreError::MissingAsset { .. }));
    assert!(!dest.exists());
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn degraded_policy_publishes_marked_copy() {
    let (c, _digest) = media_project();
    let snap = SnapshotResolver::new()
        .capture_live(
            &c,
            c.live_revision(),
            &SnapshotCancel::new(),
            Some(&AbsentProbe),
        )
        .unwrap();
    let parent = tmp_dir("deg");
    let dest = parent.join("copy-c");
    let blobs = MapBlobs {
        map: std::collections::BTreeMap::default(),
    };
    let ports = RestorePorts {
        blobs: &blobs,
        plugin_state: None,
        storage: None,
    };
    let mut req = request(snap, dest.clone(), "r-3");
    req.missing_assets = MissingAssetsPolicy::AllowDegraded;
    let receipt = restore_as_copy(&req, &ports, &SnapshotCancel::new()).unwrap();
    assert!(receipt.effects.copy.degraded_copy);
    assert_eq!(receipt.assets.copied, 0);
    assert_eq!(receipt.assets.degraded.len(), 1);
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn existing_destination_fails_closed() {
    let (c, digest) = media_project();
    let snap = snap_live(&c);
    let parent = tmp_dir("exists");
    let dest = parent.join("copy-d");
    std::fs::create_dir_all(&dest).unwrap();
    let blobs = MapBlobs {
        map: [(digest, b"fake-media-payload".to_vec())]
            .into_iter()
            .collect(),
    };
    let ports = RestorePorts {
        blobs: &blobs,
        plugin_state: None,
        storage: None,
    };
    let err =
        restore_as_copy(&request(snap, dest, "r-4"), &ports, &SnapshotCancel::new()).unwrap_err();
    assert!(matches!(err, RestoreError::DestinationExists { .. }));
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn unsafe_destinations_fail_before_any_write() {
    let (c, _d) = media_project();
    let snap = snap_live(&c);
    let blobs = MapBlobs {
        map: std::collections::BTreeMap::default(),
    };
    let ports = RestorePorts {
        blobs: &blobs,
        plugin_state: None,
        storage: None,
    };
    // Relative path.
    let rel = request(snap.clone(), PathBuf::from("rel-copy"), "r-5");
    assert!(matches!(
        restore_as_copy(&rel, &ports, &SnapshotCancel::new()),
        Err(RestoreError::InvalidRequest { .. })
    ));
    // Reserved device name leaf.
    let parent = tmp_dir("trav");
    let bad = request(snap.clone(), parent.join("CON"), "r-7");
    assert!(matches!(
        restore_as_copy(&bad, &ports, &SnapshotCancel::new()),
        Err(RestoreError::InvalidRequest { .. })
    ));
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn low_disk_fails_closed() {
    let (c, digest) = media_project();
    let snap = snap_live(&c);
    let parent = tmp_dir("disk");
    let dest = parent.join("copy-e");
    let blobs = MapBlobs {
        map: [(digest, b"fake-media-payload".to_vec())]
            .into_iter()
            .collect(),
    };
    let ports = RestorePorts {
        blobs: &blobs,
        plugin_state: None,
        storage: Some(&LowDisk),
    };
    let err = restore_as_copy(
        &request(snap, dest.clone(), "r-8"),
        &ports,
        &SnapshotCancel::new(),
    )
    .unwrap_err();
    assert!(matches!(err, RestoreError::LowDisk { .. }));
    assert!(!dest.exists());
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn cancelled_restore_writes_nothing() {
    let (c, _d) = media_project();
    let snap = snap_live(&c);
    let parent = tmp_dir("cancel");
    let dest = parent.join("copy-f");
    let blobs = MapBlobs {
        map: std::collections::BTreeMap::default(),
    };
    let ports = RestorePorts {
        blobs: &blobs,
        plugin_state: None,
        storage: None,
    };
    let cancel = SnapshotCancel::new();
    cancel.cancel();
    let err = restore_as_copy(&request(snap, dest.clone(), "r-9"), &ports, &cancel).unwrap_err();
    assert!(matches!(err, RestoreError::Cancelled));
    assert!(!dest.exists());
    // No staging residue either.
    assert!(!parent.join(".copy-f.restore-r-9").exists());
    let _ = std::fs::remove_dir_all(&parent);
}
