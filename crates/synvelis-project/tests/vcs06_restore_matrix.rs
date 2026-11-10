//! VCS-06 (issue #97): restore fixture rows not covered by
//! `vcs05_restore.rs` — interrupted-staging resume, tampered/foreign/
//! orphan staging conflicts, plugin-state required/copy paths,
//! traversal leaf rejection, and case-conflicting siblings.
//!
//! Real OS durability certification, real provider/plugin-host E2E,
//! and consumer lanes stay `NotRun`
//! (see `evidence/reports/vcs-06-fixture-matrix-ledger.md`).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use synvelis_project::controller::{Command, ProjectController};
use synvelis_project::id::{CounterIdGenerator, ProcessorId, ProjectId};
use synvelis_project::ops::ProjectOp;
use synvelis_project::plugin_state::{
    DirectoryPluginStateStore, PluginStateManifest, PluginStateOutcome, PluginStateRecord,
    PluginStateStore, StateCaptureQuality, plugin_state_manifest_name,
};
use synvelis_project::project::Project;
use synvelis_project::restore::{
    MissingAssetsPolicy, RestoreBlobSource, RestoreError, RestoreOutcome, RestorePluginStateSource,
    RestorePorts, RestoreRequest, restore_as_copy,
};
use synvelis_project::routing::{PluginRef, ProcessorBinding, StripOwner};
use synvelis_project::snapshot::{
    BlobProbe, BlobRead, ContentDigest, ImmutableSnapshot, SnapshotCancel, SnapshotResolver,
};

// ── fixtures (mirrors vcs05_restore.rs) ───────────────────────────

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

const PAYLOAD_A: &[u8] = b"fake-media-payload";
const PAYLOAD_B: &[u8] = b"fake-media-payload-2";

/// Project with two real-digest assets + sources + track/lane — two
/// blob reads give the interrupt fixture a deterministic mid-
/// materialize cancel point.
fn media_project() -> (ProjectController, Vec<String>) {
    let mut c = controller();
    apply_op(
        &mut c,
        ProjectOp::AddTrack {
            name: "drums".into(),
        },
    );
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddLane { track_id: track });
    let mut digests = Vec::new();
    for payload in [PAYLOAD_A, PAYLOAD_B] {
        let digest = ContentDigest::sha256(payload).to_string();
        apply_op(
            &mut c,
            ProjectOp::RegisterAsset {
                digest: digest.clone(),
            },
        );
        let asset = *c.live().assets.keys().next_back().unwrap();
        apply_op(
            &mut c,
            ProjectOp::RegisterSource {
                asset_id: asset,
                content_digest: digest.clone(),
            },
        );
        digests.push(digest);
    }
    (c, digests)
}

/// Project with a resolved plugin processor (plugin-state required).
fn plugin_project() -> (ProjectController, ProcessorId) {
    let mut c = controller();
    apply_op(
        &mut c,
        ProjectOp::AddTrack {
            name: "drums".into(),
        },
    );
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c
        .live()
        .routing
        .strips
        .values()
        .find(|s| s.owner == StripOwner::Track(track))
        .map(|s| s.id)
        .expect("track strip");
    apply_op(
        &mut c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index: 0,
            name: "fx".into(),
            binding: ProcessorBinding::Resolved(PluginRef {
                descriptor: "fixture-fx".into(),
                version: "1.0".into(),
                capability_revision: 1,
            }),
            sidechain_input: false,
        },
    );
    let pid = *c.live().routing.processors.keys().next().unwrap();
    (c, pid)
}

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

/// Cancels a cloned [`SnapshotCancel`] (the clone shares the flag)
/// during the first blob read — simulates an interrupt *after* the
/// staging manifest was written but before materialize completes.
struct InterruptOnceBlobs {
    inner: MapBlobs,
    cancel: SnapshotCancel,
    armed: AtomicBool,
}

impl RestoreBlobSource for InterruptOnceBlobs {
    fn read_blob(&self, digest: &str, cancel: &SnapshotCancel) -> Result<BlobRead, String> {
        if !self.armed.swap(true, Ordering::SeqCst) {
            self.cancel.cancel();
        }
        self.inner.read_blob(digest, cancel)
    }
    fn stat_blob(&self, digest: &str) -> Result<Option<u64>, String> {
        self.inner.stat_blob(digest)
    }
}

fn tmp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "synvelis-vcs06-{tag}-{}-{}",
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

fn media_ports(blobs: &MapBlobs) -> RestorePorts<'_> {
    RestorePorts {
        blobs,
        plugin_state: None,
        storage: None,
    }
}

fn media_blobs() -> MapBlobs {
    MapBlobs {
        map: [
            (
                ContentDigest::sha256(PAYLOAD_A).to_string(),
                PAYLOAD_A.to_vec(),
            ),
            (
                ContentDigest::sha256(PAYLOAD_B).to_string(),
                PAYLOAD_B.to_vec(),
            ),
        ]
        .into_iter()
        .collect(),
    }
}

fn staging_dir(parent: &Path, leaf: &str, restore_id: &str) -> PathBuf {
    parent.join(format!(".{leaf}.restore-{restore_id}"))
}

/// Runs a restore expected to be interrupted mid-materialize and
/// returns the error; asserts the staging tree remains and no
/// destination was published.
fn interrupt_restore(
    snap: &ImmutableSnapshot,
    dest: &Path,
    staging: &Path,
    rid: &str,
) -> RestoreError {
    let cancel = SnapshotCancel::new();
    let flaky = InterruptOnceBlobs {
        inner: media_blobs(),
        cancel: cancel.clone(),
        armed: AtomicBool::new(false),
    };
    let ports = RestorePorts {
        blobs: &flaky,
        plugin_state: None,
        storage: None,
    };
    let err = restore_as_copy(
        &request(snap.clone(), dest.to_path_buf(), rid),
        &ports,
        &cancel,
    )
    .expect_err("interrupted restore must fail");
    assert!(staging.exists(), "staging must remain for resume");
    assert!(!dest.exists());
    err
}

// ── VCS-RESTORE-03: crash / resume / readback ─────────────────────

#[test]
fn interrupted_staging_resumes_to_published() {
    let (c, _digests) = media_project();
    let snap = snap_live(&c);
    let parent = tmp_dir("resume");
    let dest = parent.join("copy-r");
    let staging = staging_dir(&parent, "copy-r", "r-r");

    let err = interrupt_restore(&snap, &dest, &staging, "r-r");
    assert!(
        matches!(err, RestoreError::Cancelled | RestoreError::Io { .. }),
        "unexpected interrupt error: {err:?}"
    );

    // Resume with a healthy source: same request → Published.
    let blobs = media_blobs();
    let ports = media_ports(&blobs);
    let receipt = restore_as_copy(
        &request(snap, dest.clone(), "r-r"),
        &ports,
        &SnapshotCancel::new(),
    )
    .unwrap();
    assert_eq!(receipt.outcome, RestoreOutcome::Published);
    assert!(dest.join("HEAD").exists());
    assert!(dest.join("restore-receipt.json").exists());
    assert!(!staging.exists(), "staging consumed by publish");
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn tampered_staging_fails_resume_and_keeps_destination_absent() {
    let (c, _digests) = media_project();
    let snap = snap_live(&c);
    let parent = tmp_dir("tamper");
    let dest = parent.join("copy-t");
    let staging = staging_dir(&parent, "copy-t", "r-t");

    interrupt_restore(&snap, &dest, &staging, "r-t");

    // Corrupt the staged snapshot envelope — resume re-materializes
    // idempotently, but `publish_file` refuses an existing file whose
    // bytes differ, so readback verification must reject the resume
    // before anything is published. (HEAD uses `publish_replace` and
    // would be healed; the envelope is `CreateNew`-verified.)
    let head = std::fs::read_to_string(staging.join("HEAD")).unwrap();
    let envelope = staging.join("snapshots").join(head.trim());
    assert!(
        envelope.exists(),
        "staged envelope written during materialize"
    );
    std::fs::write(&envelope, b"tampered-envelope-bytes").unwrap();

    let blobs = media_blobs();
    let ports = media_ports(&blobs);
    let err = restore_as_copy(
        &request(snap, dest.clone(), "r-t"),
        &ports,
        &SnapshotCancel::new(),
    )
    .unwrap_err();
    assert!(
        matches!(
            err,
            RestoreError::Validation { .. }
                | RestoreError::Io { .. }
                | RestoreError::MissingAsset { .. }
        ),
        "unexpected error: {err:?}"
    );
    assert!(!dest.exists());
    assert!(
        staging.exists(),
        "failed resume keeps staging for inspection"
    );
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn foreign_staging_manifest_conflicts() {
    let (c_a, _digests) = media_project();
    let snap_a = snap_live(&c_a);
    let parent = tmp_dir("foreign");
    let dest = parent.join("copy-f");
    let staging = staging_dir(&parent, "copy-f", "r-f");

    // Interrupt restore A mid-materialize.
    interrupt_restore(&snap_a, &dest, &staging, "r-f");

    // A *different* snapshot under the same restore_id must not
    // silently resume A's staging tree.
    let mut c_b = controller();
    apply_op(
        &mut c_b,
        ProjectOp::SetProjectName {
            name: "other".into(),
        },
    );
    let snap_b = snap_live(&c_b);
    let blobs = media_blobs();
    let ports = media_ports(&blobs);
    let err = restore_as_copy(
        &request(snap_b, dest.clone(), "r-f"),
        &ports,
        &SnapshotCancel::new(),
    )
    .unwrap_err();
    assert!(
        matches!(err, RestoreError::StagingConflict { .. }),
        "{err:?}"
    );
    assert!(!dest.exists());
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn orphan_staging_dir_conflicts() {
    let (c, _digests) = media_project();
    let snap = snap_live(&c);
    let parent = tmp_dir("orphan");
    let dest = parent.join("copy-o");
    // Staging dir exists but holds no manifest — never deleted blindly.
    std::fs::create_dir_all(staging_dir(&parent, "copy-o", "r-o")).unwrap();
    let blobs = MapBlobs {
        map: std::collections::BTreeMap::default(),
    };
    let ports = media_ports(&blobs);
    let err = restore_as_copy(
        &request(snap, dest.clone(), "r-o"),
        &ports,
        &SnapshotCancel::new(),
    )
    .unwrap_err();
    assert!(
        matches!(err, RestoreError::StagingConflict { .. }),
        "{err:?}"
    );
    assert!(!dest.exists());
    let _ = std::fs::remove_dir_all(&parent);
}

// ── VCS-RESTORE-04: plugin state ──────────────────────────────────

#[test]
fn processor_project_requires_plugin_state_source() {
    let (c, _pid) = plugin_project();
    let snap = snap_live(&c);
    let parent = tmp_dir("psreq");
    let dest = parent.join("copy-p");
    let blobs = MapBlobs {
        map: std::collections::BTreeMap::default(),
    };
    let ports = RestorePorts {
        blobs: &blobs,
        plugin_state: None,
        storage: None,
    };
    let err = restore_as_copy(
        &request(snap, dest.clone(), "r-p"),
        &ports,
        &SnapshotCancel::new(),
    )
    .unwrap_err();
    assert!(
        matches!(err, RestoreError::PluginStateRequired { .. }),
        "{err:?}"
    );
    assert!(!dest.exists());
    let _ = std::fs::remove_dir_all(&parent);
}

/// Serves a real `DirectoryPluginStateStore` layout back as the
/// restore read port.
struct StoreBackedPluginState {
    root: PathBuf,
    manifest_name: String,
    manifest_bytes: Vec<u8>,
}

impl RestorePluginStateSource for StoreBackedPluginState {
    fn head_manifest(&self) -> Result<Option<(String, Vec<u8>)>, String> {
        Ok(Some((
            self.manifest_name.clone(),
            self.manifest_bytes.clone(),
        )))
    }
    fn read_blob(&self, digest: &ContentDigest) -> Result<Vec<u8>, String> {
        let store = DirectoryPluginStateStore::create(&self.root).map_err(|e| e.to_string())?;
        store.read_blob(digest).map_err(|e| e.to_string())
    }
}

#[test]
fn plugin_state_manifest_and_blobs_are_copied() {
    let (c, pid) = plugin_project();
    let snap = snap_live(&c);

    // Build an authentic plugin-state store: one Captured record
    // pointing at a content-addressed blob.
    let blob = b"plugin-state-payload".to_vec();
    let blob_digest = ContentDigest::sha256(&blob);
    let store_dir = tmp_dir("psstore");
    let mut store = DirectoryPluginStateStore::create(&store_dir).unwrap();
    store.put_blob(&blob_digest, &blob).unwrap();
    let manifest = PluginStateManifest {
        schema_version: synvelis_project::plugin_state::PLUGIN_STATE_SCHEMA_VERSION,
        project_id: c.live().project_id,
        project_revision: c.live_revision().0,
        project_head: None,
        entries: vec![PluginStateRecord {
            processor_id: pid,
            binding: None,
            project_revision: c.live_revision().0,
            stored_at: "fixture".into(),
            outcome: PluginStateOutcome::Captured {
                quality: StateCaptureQuality::Quiescent,
                blob_digest,
                blob_len: blob.len() as u64,
            },
            detail: None,
        }],
    };
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    let manifest_name = store.save_manifest(&manifest_bytes).unwrap();
    assert_eq!(manifest_name, plugin_state_manifest_name(&manifest_bytes));

    let source = StoreBackedPluginState {
        root: store_dir.clone(),
        manifest_name,
        manifest_bytes,
    };
    let blobs = MapBlobs {
        map: std::collections::BTreeMap::default(),
    };
    let parent = tmp_dir("pscopy");
    let dest = parent.join("copy-ps");
    let ports = RestorePorts {
        blobs: &blobs,
        plugin_state: Some(&source),
        storage: None,
    };
    let receipt = restore_as_copy(
        &request(snap, dest.clone(), "r-ps"),
        &ports,
        &SnapshotCancel::new(),
    )
    .unwrap();
    assert_eq!(receipt.outcome, RestoreOutcome::Published);
    let report = receipt.plugin_state.expect("plugin-state report");
    assert_ne!(report.manifest_name, "");

    // The published bundle carries the plugin-state tree.
    let ps_dir = dest.join("plugin-state");
    assert!(
        ps_dir
            .join("manifests")
            .read_dir()
            .unwrap()
            .next()
            .is_some()
    );
    assert!(ps_dir.join("blobs").read_dir().unwrap().next().is_some());
    let _ = std::fs::remove_dir_all(&parent);
    let _ = std::fs::remove_dir_all(&store_dir);
}

// ── VCS-RESTORE-05: destination traversal / case conflicts ─────────

#[test]
fn dotdot_leaf_is_rejected_before_any_write() {
    let (c, _digests) = media_project();
    let snap = snap_live(&c);
    let parent = tmp_dir("dotdot");
    let blobs = MapBlobs {
        map: std::collections::BTreeMap::default(),
    };
    let ports = media_ports(&blobs);
    let err = restore_as_copy(
        &request(snap, parent.join(".."), "r-dd"),
        &ports,
        &SnapshotCancel::new(),
    )
    .unwrap_err();
    assert!(
        matches!(err, RestoreError::InvalidRequest { .. }),
        "{err:?}"
    );
    let _ = std::fs::remove_dir_all(&parent);
}

#[test]
fn case_conflicting_sibling_is_rejected() {
    let (c, _digests) = media_project();
    let snap = snap_live(&c);
    let parent = tmp_dir("case");
    // Sibling differing only by ASCII case — rejected on every FS
    // because the conflict scan is lexical, not FS-dependent.
    std::fs::create_dir_all(parent.join("Copy-Case")).unwrap();
    let blobs = MapBlobs {
        map: std::collections::BTreeMap::default(),
    };
    let ports = media_ports(&blobs);
    let err = restore_as_copy(
        &request(snap, parent.join("copy-case"), "r-cc"),
        &ports,
        &SnapshotCancel::new(),
    )
    .unwrap_err();
    assert!(
        matches!(err, RestoreError::DestinationExists { .. }),
        "{err:?}"
    );
    assert!(parent.join("Copy-Case").exists());
    let _ = std::fs::remove_dir_all(&parent);
}
