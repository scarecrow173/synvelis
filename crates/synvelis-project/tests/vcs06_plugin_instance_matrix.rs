//! VCS-06 (issue #97): plugin-instance identity fixture rows —
//! `fork_project` (restore-as-copy) preserves
//! `Project::plugin_instances` (issued `PluginInstanceId`s + the
//! monotonic watermark) verbatim; only `project_id` is renumbered.
//!
//! Same headless contract as the other `vcs06_*` matrices: real
//! GUI/plugin/consumer lanes remain `NotRun`
//! (see `evidence/reports/vcs-06-fixture-matrix-ledger.md`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use synvelis_project::controller::{Command, ProjectController};
use synvelis_project::id::{CounterIdGenerator, ProcessorId, ProjectId};
use synvelis_project::ops::ProjectOp;
use synvelis_project::plugin_instance::PluginInstanceId;
use synvelis_project::plugin_state::{
    DirectoryPluginStateStore, PLUGIN_STATE_SCHEMA_VERSION, PluginStateManifest,
    PluginStateOutcome, PluginStateRecord, PluginStateStore, StateCaptureQuality,
    plugin_state_manifest_name,
};
use synvelis_project::project::Project;
use synvelis_project::restore::{
    MissingAssetsPolicy, RestoreBlobSource, RestoreOutcome, RestorePluginStateSource, RestorePorts,
    RestoreRequest, restore_as_copy,
};
use synvelis_project::routing::{PluginRef, ProcessorBinding};
use synvelis_project::snapshot::{
    BlobProbe, BlobRead, ContentDigest, ImmutableSnapshot, SnapshotCancel, SnapshotResolver,
};
use synvelis_project::store::SnapshotEnvelope;

// ── helpers (mirrors vcs06_* matrix files) ───────────────────────

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

fn plugin_ref(descriptor: &str, version: &str, capability_revision: u64) -> PluginRef {
    PluginRef {
        descriptor: descriptor.into(),
        version: version.into(),
        capability_revision,
    }
}

fn insert_processor(
    c: &mut ProjectController,
    strip: synvelis_project::id::StripId,
    index: usize,
    name: &str,
    binding: ProcessorBinding,
) -> ProcessorId {
    apply_op(
        c,
        ProjectOp::InsertProcessor {
            strip_id: strip,
            index,
            name: name.into(),
            binding,
            sidechain_input: false,
        },
    );
    *c.live().routing.processors.keys().next_back().unwrap()
}

struct MapBlobs {
    map: BTreeMap<String, Vec<u8>>,
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

/// Serves a real `DirectoryPluginStateStore` layout back as the
/// restore read port (mirrors `vcs06_restore_matrix.rs`).
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

/// Builds an authentic plugin-state store — a real
/// `DirectoryPluginStateStore` on disk holding one `Captured`
/// record per `(processor, blob)` pair — and returns it as the
/// restore read port plus the store dir for cleanup (mirrors
/// `plugin_state_manifest_and_blobs_are_copied` in
/// `vcs06_restore_matrix.rs`).
fn plugin_state_source(
    c: &ProjectController,
    pairs: &[(ProcessorId, &[u8])],
) -> (StoreBackedPluginState, PathBuf) {
    let store_dir = tmp_dir("pistore");
    let mut store = DirectoryPluginStateStore::create(&store_dir).unwrap();
    let mut entries = Vec::new();
    for (i, (proc, blob)) in pairs.iter().enumerate() {
        let blob = blob.to_vec();
        let blob_digest = ContentDigest::sha256(&blob);
        store.put_blob(&blob_digest, &blob).unwrap();
        entries.push(PluginStateRecord {
            processor_id: *proc,
            binding: None,
            project_revision: c.live_revision().0,
            stored_at: format!("fixture-{i}"),
            outcome: PluginStateOutcome::Captured {
                quality: StateCaptureQuality::Quiescent,
                blob_digest,
                blob_len: blob.len() as u64,
            },
            detail: None,
        });
    }
    let manifest = PluginStateManifest {
        schema_version: PLUGIN_STATE_SCHEMA_VERSION,
        project_id: c.live().project_id,
        project_revision: c.live_revision().0,
        project_head: None,
        entries,
    };
    let manifest_bytes = serde_json::to_vec(&manifest).unwrap();
    let manifest_name = store.save_manifest(&manifest_bytes).unwrap();
    assert_eq!(manifest_name, plugin_state_manifest_name(&manifest_bytes));
    let source = StoreBackedPluginState {
        root: store_dir.clone(),
        manifest_name,
        manifest_bytes,
    };
    (source, store_dir)
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

// ── VCS-RESTORE-08: plugin_instances verbatim restore-as-copy ────

#[test]
fn restore_as_copy_preserves_plugin_instances_verbatim() {
    // Two resolved processors pin distinct authored PluginRefs
    // (ids 1 and 2, watermark 3); the unresolved processor carries
    // no identity entry — the store shape `fork_project` must carry
    // verbatim.
    let plugin_a = plugin_ref("vendor.dist", "1.0.0", 3);
    let plugin_b = plugin_ref("vendor.eq", "2.1.0", 7);
    let mut c = controller();
    apply_op(&mut c, ProjectOp::AddTrack { name: "vox".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(&mut c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;

    let proc_a = insert_processor(
        &mut c,
        strip,
        0,
        "dist",
        ProcessorBinding::Resolved(plugin_a.clone()),
    );
    let proc_b = insert_processor(
        &mut c,
        strip,
        1,
        "eq",
        ProcessorBinding::Resolved(plugin_b.clone()),
    );
    let proc_unresolved = insert_processor(
        &mut c,
        strip,
        2,
        "missing",
        ProcessorBinding::Unresolved {
            descriptor: "vendor.missing".into(),
        },
    );

    // Pre-conditions: the issued identities the fork must preserve.
    let identities = &c.live().plugin_instances;
    assert_eq!(identities.watermark, 3);
    assert_eq!(identities.instances.len(), 2);
    assert_eq!(identities.instances[&proc_a].id, PluginInstanceId::new(1));
    assert_eq!(identities.instances[&proc_a].plugin, plugin_a);
    assert_eq!(identities.instances[&proc_b].id, PluginInstanceId::new(2));
    assert_eq!(identities.instances[&proc_b].plugin, plugin_b);
    assert!(!identities.instances.contains_key(&proc_unresolved));

    let snap = snap_live(&c);

    // Processor-bearing projects require a plugin-state source.
    let (source, store_dir) =
        plugin_state_source(&c, &[(proc_a, b"state-a".as_slice()), (proc_b, b"state-b")]);
    let blobs = MapBlobs {
        map: BTreeMap::default(),
    };
    let parent = tmp_dir("pirestore");
    let dest = parent.join("copy-pi");
    let ports = RestorePorts {
        blobs: &blobs,
        plugin_state: Some(&source),
        storage: None,
    };
    let receipt = restore_as_copy(
        &request(snap, dest.clone(), "r-pi"),
        &ports,
        &SnapshotCancel::new(),
    )
    .unwrap();
    assert_eq!(receipt.outcome, RestoreOutcome::Published);
    assert_eq!(receipt.source_project_id, ProjectId::new(1));
    assert_eq!(receipt.new_project_id, ProjectId::new(777));
    assert_ne!(receipt.new_project_id, receipt.source_project_id);
    assert!(receipt.effects.source.project_unchanged);
    assert!(receipt.effects.source.saved_head_unchanged);
    assert!(receipt.effects.git.index_untouched);
    assert!(receipt.effects.git.worktree_untouched);

    // Readback: the published snapshot envelope's fork carries
    // `plugin_instances` verbatim — every issued id, its pinned
    // PluginRef, and the watermark — with `project_id` the only
    // renumbered field.
    let head = std::fs::read_to_string(dest.join("HEAD")).unwrap();
    let snap_bytes = std::fs::read(dest.join("snapshots").join(head.trim())).unwrap();
    let envelope: SnapshotEnvelope = serde_json::from_slice(&snap_bytes).unwrap();
    assert_eq!(envelope.project_id, ProjectId::new(777));
    assert_eq!(envelope.project.project_id, ProjectId::new(777));
    assert_ne!(envelope.project.project_id, c.live().project_id);

    let restored = &envelope.project.plugin_instances;
    assert_eq!(restored, &c.live().plugin_instances);
    assert_eq!(restored.watermark, 3);
    assert_eq!(restored.instances.len(), 2);
    assert_eq!(restored.instances[&proc_a].id, PluginInstanceId::new(1));
    assert_eq!(restored.instances[&proc_a].plugin, plugin_a);
    assert_eq!(restored.instances[&proc_b].id, PluginInstanceId::new(2));
    assert_eq!(restored.instances[&proc_b].plugin, plugin_b);
    assert!(!restored.instances.contains_key(&proc_unresolved));

    // The preserved identities still resolve against the fork's
    // authored bindings — verbatim is functional, not just equal.
    assert_eq!(
        restored.resolve(proc_a, &plugin_a),
        Some(PluginInstanceId::new(1))
    );
    assert_eq!(
        restored.resolve(proc_b, &plugin_b),
        Some(PluginInstanceId::new(2))
    );
    // ...and the fork's reference oracle accepts the carried store.
    envelope.project.validate_references().unwrap();

    let _ = std::fs::remove_dir_all(&parent);
    let _ = std::fs::remove_dir_all(&store_dir);
}
