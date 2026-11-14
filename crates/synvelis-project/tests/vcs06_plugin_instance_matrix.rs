//! VCS-06 (issue #97): plugin-instance identity fixture rows —
//! `fork_project` (restore-as-copy) preserves
//! `Project::plugin_instances` (issued `PluginInstanceId`s + the
//! monotonic watermark) verbatim; only `project_id` is renumbered.
//! Plus the VCS-05 apply-surface rows for `ObjectKind::
//! PluginInstance`: byte-faithful `PluginInstanceIdentity`
//! transplant (`Added`/`Replaced`), monotone watermark reseed,
//! fail-closed `Removed`/presence/oracle rejections, and
//! ACK-gap replay.
//!
//! Same headless contract as the other `vcs06_*` matrices: real
//! GUI/plugin/consumer lanes remain `NotRun`
//! (see `evidence/reports/vcs-06-fixture-matrix-ledger.md`).

use std::collections::BTreeMap;
use std::path::PathBuf;

use synvelis_project::apply::{
    ApplyApproval, ApplyChangeIntent, ApplyError, ApplyJournal, ApplyJournalRecord,
    ApplyJournalState, ApplyOutcome, ApplyReceipt, ApplySources, ChangeSetSource,
    InMemoryApplyJournal, PluginAvailability, ResolvedChangeSet, apply_selected,
    target_fingerprint,
};
use synvelis_project::controller::{Command, ProjectController, ProjectRevision};
use synvelis_project::diff::{
    ChangeKind, ChangeSet, ComparisonCompleteness, ComparisonSpec, ObjectKind, SemanticChange,
    SemanticObjectRef, compare_snapshots,
};
use synvelis_project::id::{CounterIdGenerator, ProcessorId, ProjectId, StripId, TrackId};
use synvelis_project::ops::ProjectOp;
use synvelis_project::plugin_instance::{PluginInstanceId, PluginInstanceIdentity};
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
use synvelis_project::routing::{PluginRef, PortBinding, ProcessorBinding};
use synvelis_project::snapshot::{
    AssetAvailabilityManifest, BlobProbe, BlobRead, ContentDigest, FixedSnapshotLocator,
    ImmutableSnapshot, ReadOnlyProjectProjection, SnapshotCancel, SnapshotCompleteness,
    SnapshotIdentity, SnapshotOrigin, SnapshotResolver, SnapshotValidation,
};
use synvelis_project::store::{SCHEMA_VERSION, SnapshotEnvelope};

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

// ── VCS-APPLY: PluginInstance typed transplant ───────────────────
//
// `ObjectKind::PluginInstance` is on the VCS-05 apply surface
// (`Added`/`Replaced`; `Removed` stays fail-closed like every
// kind). The persisted `PluginInstanceIdentity` — issued
// `PluginInstanceId` + pinned `PluginRef` — is authored state and
// transplants byte-faithfully; the store `watermark` reseeds
// monotonically via `max(live, after)` so a live-retired id can
// never be reissued. Entry↔binding consistency, id range, and
// uniqueness remain `validate_references` → `validate_against`'s
// job after the write.

fn snap_manual(project: Project, tag: &str) -> ImmutableSnapshot {
    let bytes = serde_json::to_vec(&project).unwrap();
    let mut material = tag.as_bytes().to_vec();
    material.extend_from_slice(&SCHEMA_VERSION.to_le_bytes());
    material.extend_from_slice(&bytes);
    ImmutableSnapshot {
        identity: SnapshotIdentity {
            project_id: project.project_id,
            source: SnapshotOrigin::Live,
            locator: FixedSnapshotLocator::Live {
                project_id: project.project_id,
                live_revision: ProjectRevision(0),
            },
            digest: ContentDigest::sha256(&material),
            schema_version: SCHEMA_VERSION,
            projection_version: SCHEMA_VERSION,
            capture_generation: 0,
        },
        origin: SnapshotOrigin::Live,
        projection: ReadOnlyProjectProjection::new(SCHEMA_VERSION, project),
        validation: SnapshotValidation::Valid,
        assets: AssetAvailabilityManifest::default(),
        opaque_state: Vec::new(),
        completeness: SnapshotCompleteness::Complete,
    }
}

fn spec_for(before: &ImmutableSnapshot, after: &ImmutableSnapshot) -> ComparisonSpec {
    ComparisonSpec::new(before.identity.clone(), after.identity.clone())
}

fn compare(before: &ImmutableSnapshot, after: &ImmutableSnapshot) -> ChangeSet {
    compare_snapshots(
        before,
        after,
        &spec_for(before, after),
        &SnapshotCancel::new(),
    )
    .unwrap()
}

struct Src {
    resolved: ResolvedChangeSet,
}

impl ChangeSetSource for Src {
    fn resolve(&self, id: &ContentDigest) -> Option<ResolvedChangeSet> {
        (id == &self.resolved.changeset.key.digest()).then(|| self.resolved.clone())
    }
}

fn sources_for(cs: &ChangeSet, before: &ImmutableSnapshot, after: &ImmutableSnapshot) -> Src {
    Src {
        resolved: ResolvedChangeSet {
            changeset: cs.clone(),
            before: before.clone(),
            after: after.clone(),
        },
    }
}

struct AllPlugins;

impl PluginAvailability for AllPlugins {
    fn plugin_available(&self, _plugin: &PluginRef) -> bool {
        true
    }
}

struct NoPlugins;

impl PluginAvailability for NoPlugins {
    fn plugin_available(&self, _plugin: &PluginRef) -> bool {
        false
    }
}

fn changes_of(cs: &ChangeSet, kind: ObjectKind) -> Vec<&SemanticChange> {
    cs.changes
        .iter()
        .filter(|c| c.object.kind == kind)
        .collect()
}

/// Track + strip fixture shared by before/after controllers —
/// deterministic `CounterIdGenerator` keeps entity ids aligned so
/// the changeset isolates the processor delta.
fn track_strip(c: &mut ProjectController) -> (TrackId, StripId) {
    apply_op(c, ProjectOp::AddTrack { name: "vox".into() });
    let track = *c.live().tracks.keys().next().unwrap();
    apply_op(c, ProjectOp::AddTrackStrip { track_id: track });
    let strip = c.live().routing.strip_of_track(track).unwrap().id;
    (track, strip)
}

fn intent_for(
    c: &ProjectController,
    cs: &ChangeSet,
    after: &ImmutableSnapshot,
    changes: &[&SemanticChange],
) -> ApplyChangeIntent {
    let mut objects: Vec<SemanticObjectRef> = changes.iter().map(|ch| ch.object.clone()).collect();
    objects.push(changes[0].object.clone());
    let fp = target_fingerprint(c.live(), &objects).unwrap();
    ApplyChangeIntent {
        change_set_id: cs.key.digest(),
        source_snapshot_id: after.identity.digest,
        target_object: changes[0].object.clone(),
        expected_project_revision: c.live_revision(),
        selected_change_ids: changes.iter().map(|ch| ch.id.clone()).collect(),
        expected_target_fingerprint: fp,
        approval: ApplyApproval {
            ticket: "user-approved".into(),
        },
    }
}

fn run_apply(
    c: &mut ProjectController,
    intent: &ApplyChangeIntent,
    src: &Src,
    journal: &mut dyn ApplyJournal,
) -> Result<ApplyReceipt, ApplyError> {
    let sources = ApplySources {
        changesets: src,
        assets: Some(&AllPresentProbe),
        plugins: Some(&AllPlugins),
    };
    apply_selected(c, intent, &sources, journal, &SnapshotCancel::new())
}

fn journal_key(intent: &ApplyChangeIntent) -> String {
    let digest = ContentDigest::sha256(&serde_json::to_vec(intent).unwrap());
    format!("apply-{}", digest.to_string().replace(':', ""))
}

/// Journal that persists `Pending` but drops terminal records —
/// simulates a crash/ACK gap after the mutation committed (mirrors
/// `vcs05_apply.rs`).
struct LossyJournal {
    inner: InMemoryApplyJournal,
}

impl ApplyJournal for LossyJournal {
    fn load(
        &mut self,
        key: &str,
    ) -> Result<Option<ApplyJournalRecord>, synvelis_project::apply::ApplyJournalError> {
        self.inner.load(key)
    }
    fn store(
        &mut self,
        record: ApplyJournalRecord,
    ) -> Result<(), synvelis_project::apply::ApplyJournalError> {
        match record.state {
            ApplyJournalState::Pending => self.inner.store(record),
            _ => Ok(()), // drop Applied/Failed — receipt "lost"
        }
    }
}

#[test]
fn plugin_instance_added_selection_transplants_identity_verbatim() {
    let plugin_a = plugin_ref("vendor.dist", "1.0.0", 3);
    // before: track+strip, no processors — a Complete live capture
    // (no opaque plugin state to degrade the changeset).
    let mut before_c = controller();
    track_strip(&mut before_c);
    let before = snap_live(&before_c);

    // after: same project plus one resolved processor whose persisted
    // identity is hand-set to id=7/watermark=8 — divergent from the
    // id=1/watermark=2 a live `sync_processor` mint would produce —
    // proving the transplant adopts the snapshot's authored values
    // verbatim rather than re-issuing them.
    let mut after_c = controller();
    let (_track, strip) = track_strip(&mut after_c);
    let proc = insert_processor(
        &mut after_c,
        strip,
        0,
        "dist",
        ProcessorBinding::Resolved(plugin_a.clone()),
    );
    let mut after_project = after_c.live().clone();
    {
        let store = &mut after_project.plugin_instances;
        store.instances.get_mut(&proc).unwrap().id = PluginInstanceId::new(7);
        store.watermark = 8;
    }
    let after = snap_manual(after_project, "after");
    let cs = compare(&before, &after);
    assert_eq!(cs.completeness, ComparisonCompleteness::Complete);
    let pi = changes_of(&cs, ObjectKind::PluginInstance);
    assert_eq!(pi.len(), 1);
    assert_eq!(pi[0].kind, ChangeKind::Added);
    assert_eq!(pi[0].object.stable_id, proc.to_tagged());

    let changes: Vec<&SemanticChange> = cs.changes.iter().collect();
    let intent = intent_for(&before_c, &cs, &after, &changes);
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();

    let mut live = before_c;
    let receipt = run_apply(&mut live, &intent, &src, &mut journal).unwrap();
    assert_eq!(receipt.outcome, ApplyOutcome::Applied);
    assert!(receipt.effects.single_undo_step);

    // Byte-faithful: the snapshot's id=7/watermark=8 landed verbatim.
    let store = &live.live().plugin_instances;
    assert_eq!(store.instances[&proc].id, PluginInstanceId::new(7));
    assert_eq!(store.instances[&proc].plugin, plugin_a);
    assert_eq!(store.watermark, 8);
    assert_eq!(
        live.live().routing.processors[&proc].binding,
        ProcessorBinding::Resolved(plugin_a)
    );
    live.live().validate_references().unwrap();

    // One undo step removes processor + identity together; redo
    // restores the transplanted values.
    assert!(live.undo().unwrap());
    assert!(!live.live().plugin_instances.instances.contains_key(&proc));
    assert_eq!(live.live().plugin_instances.watermark, 1);
    assert!(live.redo().unwrap());
    assert_eq!(
        live.live().plugin_instances.instances[&proc].id,
        PluginInstanceId::new(7)
    );
    assert_eq!(live.live().plugin_instances.watermark, 8);
}

#[test]
fn plugin_instance_replaced_selection_transplants_new_identity() {
    let plugin_a = plugin_ref("vendor.dist", "1.0.0", 3);
    // A capability bump is a different authored `PluginRef` — the
    // rebind mints a new persisted identity (PC-C4-05).
    let plugin_b = plugin_ref("vendor.dist", "1.0.0", 4);
    let mut before_c = controller();
    let (_track, strip) = track_strip(&mut before_c);
    let proc = insert_processor(
        &mut before_c,
        strip,
        0,
        "dist",
        ProcessorBinding::Resolved(plugin_a.clone()),
    );
    assert_eq!(
        before_c.live().plugin_instances.instances[&proc].id,
        PluginInstanceId::new(1)
    );
    assert_eq!(before_c.live().plugin_instances.watermark, 2);
    // Processor-bearing endpoints snapshot via `snap_manual` — a
    // live capture records plugin state Unknown → Partial → the
    // apply gate would reject NotApplicable before planning.
    let before = snap_manual(before_c.live().clone(), "before");

    // after: the processor is rebound to plugin_b — binding, its
    // `PluginDeclared` port capability stamps, and the persisted
    // identity {id=9, plugin_b}/watermark=10 all move together, the
    // shape a real rebind op would produce.
    let mut after_project = before_c.live().clone();
    after_project
        .routing
        .processors
        .get_mut(&proc)
        .unwrap()
        .binding = ProcessorBinding::Resolved(plugin_b.clone());
    let port_ids = after_project.routing.processors[&proc].port_ids.clone();
    for port_id in port_ids {
        let port = after_project.routing.ports.get_mut(&port_id).unwrap();
        if let PortBinding::PluginDeclared {
            capability_revision,
            ..
        } = &mut port.binding
        {
            *capability_revision = plugin_b.capability_revision;
        }
    }
    after_project.plugin_instances.instances.insert(
        proc,
        PluginInstanceIdentity {
            id: PluginInstanceId::new(9),
            plugin: plugin_b.clone(),
        },
    );
    after_project.plugin_instances.watermark = 10;
    let after = snap_manual(after_project, "after");
    let cs = compare(&before, &after);
    assert_eq!(cs.completeness, ComparisonCompleteness::Complete);

    let pi = changes_of(&cs, ObjectKind::PluginInstance);
    assert_eq!(pi.len(), 1);
    assert_eq!(pi[0].kind, ChangeKind::Replaced);
    // The rebind drags the processor binding and its declared ports
    // along in the same changeset.
    assert!(
        changes_of(&cs, ObjectKind::Processor)
            .iter()
            .any(|c| c.kind == ChangeKind::Modified)
    );
    assert!(
        changes_of(&cs, ObjectKind::SemanticPort)
            .iter()
            .any(|c| c.kind == ChangeKind::Modified)
    );

    let changes: Vec<&SemanticChange> = cs.changes.iter().collect();
    let intent = intent_for(&before_c, &cs, &after, &changes);
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();

    let mut live = before_c;
    let receipt = run_apply(&mut live, &intent, &src, &mut journal).unwrap();
    assert_eq!(receipt.outcome, ApplyOutcome::Applied);
    assert!(receipt.effects.single_undo_step);

    // Byte-faithful: id=9/plugin_b landed verbatim — neither the
    // id=2 `sync_processor` would have minted nor a remapped value.
    let store = &live.live().plugin_instances;
    assert_eq!(
        store.instances[&proc],
        PluginInstanceIdentity {
            id: PluginInstanceId::new(9),
            plugin: plugin_b.clone(),
        }
    );
    assert_eq!(store.watermark, 10);
    assert_eq!(
        live.live().routing.processors[&proc].binding,
        ProcessorBinding::Resolved(plugin_b.clone())
    );
    live.live().validate_references().unwrap();

    // One undo restores the pre-rebind identity, binding, and ports.
    assert!(live.undo().unwrap());
    assert_eq!(
        live.live().plugin_instances.instances[&proc].id,
        PluginInstanceId::new(1)
    );
    assert_eq!(
        live.live().plugin_instances.instances[&proc].plugin,
        plugin_a
    );
    assert_eq!(live.live().plugin_instances.watermark, 2);
    assert!(live.redo().unwrap());
    assert_eq!(
        live.live().plugin_instances.instances[&proc].id,
        PluginInstanceId::new(9)
    );
    assert_eq!(live.live().plugin_instances.watermark, 10);
}

#[test]
fn plugin_instance_removed_selection_stays_fail_closed() {
    let plugin_a = plugin_ref("vendor.dist", "1.0.0", 3);
    // Live at S1: processor + persisted identity present.
    let mut live_c = controller();
    let (_track, strip) = track_strip(&mut live_c);
    insert_processor(
        &mut live_c,
        strip,
        0,
        "dist",
        ProcessorBinding::Resolved(plugin_a),
    );
    // `snap_manual` keeps the endpoint Complete — plugin state is
    // opaque to live capture (Unknown → Partial → NotApplicable).
    let s1 = snap_manual(live_c.live().clone(), "s1");

    // S0: same track+strip, no processor — the removal changeset.
    let mut empty_c = controller();
    track_strip(&mut empty_c);
    let s0 = snap_live(&empty_c);

    let cs = compare(&s1, &s0);
    let pi = changes_of(&cs, ObjectKind::PluginInstance);
    assert_eq!(pi.len(), 1);
    assert_eq!(pi[0].kind, ChangeKind::Removed);

    let intent = intent_for(&live_c, &cs, &s0, &[pi[0]]);
    let src = sources_for(&cs, &s1, &s0);
    let mut journal = InMemoryApplyJournal::new();
    let err = run_apply(&mut live_c, &intent, &src, &mut journal).unwrap_err();
    assert_eq!(
        err,
        ApplyError::Unsupported {
            kind: ObjectKind::PluginInstance,
            change: ChangeKind::Removed,
            detail: "removal changes are deferred (cascade design)".into(),
        }
    );
    // Live identity store and journal Failed record stay honest.
    assert_eq!(live_c.live().plugin_instances.instances.len(), 1);
    assert_eq!(live_c.live().plugin_instances.watermark, 2);
    let record = journal.load(&journal_key(&intent)).unwrap().unwrap();
    assert!(
        matches!(record.state, ApplyJournalState::Failed { .. }),
        "{record:?}"
    );
}

#[test]
fn plugin_instance_rebind_without_processor_change_fails_closed() {
    let plugin_a = plugin_ref("vendor.dist", "1.0.0", 3);
    let plugin_b = plugin_ref("vendor.dist", "1.0.0", 4);
    let mut before_c = controller();
    let (_track, strip) = track_strip(&mut before_c);
    let proc = insert_processor(
        &mut before_c,
        strip,
        0,
        "dist",
        ProcessorBinding::Resolved(plugin_a.clone()),
    );
    let before = snap_manual(before_c.live().clone(), "before");

    // after: ONLY the persisted entry's pinned PluginRef drifts —
    // the processor binding (and ports) still say plugin_a. The
    // transplant writes, then `validate_against` rejects the
    // entry↔binding mismatch inside the single-writer apply.
    let mut after_project = before_c.live().clone();
    after_project
        .plugin_instances
        .instances
        .get_mut(&proc)
        .unwrap()
        .plugin = plugin_b;
    let after = snap_manual(after_project, "after");
    let cs = compare(&before, &after);
    assert_eq!(cs.completeness, ComparisonCompleteness::Complete);
    let pi = changes_of(&cs, ObjectKind::PluginInstance);
    assert_eq!(pi.len(), 1);
    assert_eq!(pi[0].kind, ChangeKind::Replaced);
    // No processor/port churn — the identity change stands alone.
    assert!(changes_of(&cs, ObjectKind::Processor).is_empty());
    assert!(changes_of(&cs, ObjectKind::SemanticPort).is_empty());

    let intent = intent_for(&before_c, &cs, &after, &[pi[0]]);
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();
    let err = run_apply(&mut before_c, &intent, &src, &mut journal).unwrap_err();
    assert!(
        matches!(err, ApplyError::Rejected { .. }),
        "expected oracle rejection, got {err:?}"
    );

    // Live store, binding, revision, and undo history unchanged.
    let store = &before_c.live().plugin_instances;
    assert_eq!(store.instances[&proc].id, PluginInstanceId::new(1));
    assert_eq!(store.instances[&proc].plugin, plugin_a);
    assert_eq!(store.watermark, 2);
    let record = journal.load(&journal_key(&intent)).unwrap().unwrap();
    assert!(
        matches!(record.state, ApplyJournalState::Failed { .. }),
        "{record:?}"
    );
}

#[test]
fn plugin_instance_target_missing_and_conflict_fail_closed() {
    let plugin_a = plugin_ref("vendor.dist", "1.0.0", 3);
    // Two entry-bearing snapshots from one lineage: v1 {id=1} and
    // v2 {id=9} — a Replaced change on the PluginInstance object.
    let mut v1_c = controller();
    let (_t1, strip1) = track_strip(&mut v1_c);
    let proc = insert_processor(
        &mut v1_c,
        strip1,
        0,
        "dist",
        ProcessorBinding::Resolved(plugin_a.clone()),
    );
    let v1 = snap_manual(v1_c.live().clone(), "v1");

    let mut v2_project = v1_c.live().clone();
    v2_project
        .plugin_instances
        .instances
        .get_mut(&proc)
        .unwrap()
        .id = PluginInstanceId::new(9);
    v2_project.plugin_instances.watermark = 10;
    let v2 = snap_manual(v2_project, "v2");
    let cs = compare(&v1, &v2);
    let pi = changes_of(&cs, ObjectKind::PluginInstance);
    assert_eq!(pi.len(), 1);
    assert_eq!(pi[0].kind, ChangeKind::Replaced);

    // TargetMissing: live has no entry for the object (a
    // processor-less project) — a non-Added change on an absent
    // target fails closed.
    let mut missing_c = controller();
    track_strip(&mut missing_c);
    let intent = intent_for(&missing_c, &cs, &v2, &[pi[0]]);
    let src = sources_for(&cs, &v1, &v2);
    let mut journal = InMemoryApplyJournal::new();
    let err = run_apply(&mut missing_c, &intent, &src, &mut journal).unwrap_err();
    assert!(
        matches!(err, ApplyError::TargetMissing { .. }),
        "expected TargetMissing, got {err:?}"
    );
    assert!(missing_c.live().plugin_instances.instances.is_empty());

    // TargetConflict: the Added direction — live already carries
    // the entry the change wants to create.
    let mut no_proc_c = controller();
    track_strip(&mut no_proc_c);
    let s0 = snap_live(&no_proc_c);
    let mut with_proc_c = controller();
    let (_t2, strip2) = track_strip(&mut with_proc_c);
    insert_processor(
        &mut with_proc_c,
        strip2,
        0,
        "dist",
        ProcessorBinding::Resolved(plugin_a.clone()),
    );
    let s1 = snap_manual(with_proc_c.live().clone(), "s1");
    let cs_add = compare(&s0, &s1);
    let pi_add = changes_of(&cs_add, ObjectKind::PluginInstance);
    assert_eq!(pi_add.len(), 1);
    assert_eq!(pi_add[0].kind, ChangeKind::Added);

    // Live is `with_proc_c` itself — the entry already exists.
    let intent = intent_for(&with_proc_c, &cs_add, &s1, &[pi_add[0]]);
    let src = sources_for(&cs_add, &s0, &s1);
    let mut journal = InMemoryApplyJournal::new();
    let err = run_apply(&mut with_proc_c, &intent, &src, &mut journal).unwrap_err();
    assert!(
        matches!(err, ApplyError::TargetConflict { .. }),
        "expected TargetConflict, got {err:?}"
    );
    assert_eq!(with_proc_c.live().plugin_instances.instances.len(), 1);
    assert_eq!(with_proc_c.live().plugin_instances.watermark, 2);
}

#[test]
fn plugin_instance_watermark_never_moves_backward() {
    let plugin_a = plugin_ref("vendor.dist", "1.0.0", 3);
    // Phase A: apply an Added selection whose after-store carries
    // id=9/watermark=10 — live adopts both verbatim.
    let mut live_c = controller();
    track_strip(&mut live_c);
    let before_a = snap_live(&live_c);

    let mut after_a_c = controller();
    let (_t, strip) = track_strip(&mut after_a_c);
    let proc = insert_processor(
        &mut after_a_c,
        strip,
        0,
        "dist",
        ProcessorBinding::Resolved(plugin_a.clone()),
    );
    let mut after_a_project = after_a_c.live().clone();
    after_a_project
        .plugin_instances
        .instances
        .get_mut(&proc)
        .unwrap()
        .id = PluginInstanceId::new(9);
    after_a_project.plugin_instances.watermark = 10;
    let after_a = snap_manual(after_a_project, "after-a");
    let cs_a = compare(&before_a, &after_a);
    let changes_a: Vec<&SemanticChange> = cs_a.changes.iter().collect();
    let intent_a = intent_for(&live_c, &cs_a, &after_a, &changes_a);
    let src_a = sources_for(&cs_a, &before_a, &after_a);
    let mut journal = InMemoryApplyJournal::new();
    let receipt = run_apply(&mut live_c, &intent_a, &src_a, &mut journal).unwrap();
    assert_eq!(receipt.outcome, ApplyOutcome::Applied);
    assert_eq!(live_c.live().plugin_instances.watermark, 10);
    assert_eq!(
        live_c.live().plugin_instances.instances[&proc].id,
        PluginInstanceId::new(9)
    );

    // Phase B: a later snapshot whose store regressed to
    // id=5/watermark=6 — an id-only Replaced on the entry. The
    // transplanted id adopts the snapshot's 5, but the watermark
    // must stay at live's 10: `max(live, after)`, never backward,
    // so ids 6..=9 remain retired and can never be reissued.
    // (`snap_manual` — live now carries a processor, so a live
    // capture would be Partial and the apply gate would refuse.)
    let before_b = snap_manual(live_c.live().clone(), "before-b");
    let mut after_b_project = live_c.live().clone();
    after_b_project
        .plugin_instances
        .instances
        .get_mut(&proc)
        .unwrap()
        .id = PluginInstanceId::new(5);
    after_b_project.plugin_instances.watermark = 6;
    let after_b = snap_manual(after_b_project, "after-b");
    let cs_b = compare(&before_b, &after_b);
    let pi_b = changes_of(&cs_b, ObjectKind::PluginInstance);
    assert_eq!(pi_b.len(), 1);
    assert_eq!(pi_b[0].kind, ChangeKind::Replaced);
    // id is the only semantically diffed field — watermark is
    // allocator bookkeeping, excluded from the classifier.
    assert_eq!(cs_b.changes.len(), 1);

    let intent_b = intent_for(&live_c, &cs_b, &after_b, &[pi_b[0]]);
    let src_b = sources_for(&cs_b, &before_b, &after_b);
    let mut journal = InMemoryApplyJournal::new();
    let receipt = run_apply(&mut live_c, &intent_b, &src_b, &mut journal).unwrap();
    assert_eq!(receipt.outcome, ApplyOutcome::Applied);

    let store = &live_c.live().plugin_instances;
    assert_eq!(store.instances[&proc].id, PluginInstanceId::new(5));
    assert_eq!(store.watermark, 10, "watermark must never move backward");
    live_c.live().validate_references().unwrap();

    // Undo restores {id=9, watermark=10}; the retired range stays
    // retired through the whole round trip.
    assert!(live_c.undo().unwrap());
    assert_eq!(
        live_c.live().plugin_instances.instances[&proc].id,
        PluginInstanceId::new(9)
    );
    assert_eq!(live_c.live().plugin_instances.watermark, 10);
}

#[test]
fn plugin_instance_added_selection_rejects_unavailable_plugin() {
    let plugin_a = plugin_ref("vendor.dist", "1.0.0", 3);
    let mut before_c = controller();
    track_strip(&mut before_c);
    let before = snap_live(&before_c);

    let mut after_c = controller();
    let (_t, strip) = track_strip(&mut after_c);
    insert_processor(
        &mut after_c,
        strip,
        0,
        "dist",
        ProcessorBinding::Resolved(plugin_a),
    );
    let mut after_project = after_c.live().clone();
    // Divergent authored id again — the selection must still fail at
    // plugin preflight before any write is attempted.
    let proc = *after_project.routing.processors.keys().next_back().unwrap();
    after_project
        .plugin_instances
        .instances
        .get_mut(&proc)
        .unwrap()
        .id = PluginInstanceId::new(7);
    after_project.plugin_instances.watermark = 8;
    let after = snap_manual(after_project, "after");
    let cs = compare(&before, &after);
    let changes: Vec<&SemanticChange> = cs.changes.iter().collect();
    let intent = intent_for(&before_c, &cs, &after, &changes);
    let src = sources_for(&cs, &before, &after);
    let mut journal = InMemoryApplyJournal::new();

    let sources = ApplySources {
        changesets: &src,
        assets: Some(&AllPresentProbe),
        plugins: Some(&NoPlugins),
    };
    let mut live = before_c;
    let err = apply_selected(
        &mut live,
        &intent,
        &sources,
        &mut journal,
        &SnapshotCancel::new(),
    )
    .unwrap_err();
    assert!(
        matches!(err, ApplyError::MissingPlugin { .. }),
        "expected MissingPlugin, got {err:?}"
    );
    // Fail-closed: no processor, no identity entry, no undo step.
    assert!(live.live().routing.processors.is_empty());
    assert!(live.live().plugin_instances.instances.is_empty());
    assert_eq!(live.live().plugin_instances.watermark, 1);
    let record = journal.load(&journal_key(&intent)).unwrap().unwrap();
    assert!(
        matches!(record.state, ApplyJournalState::Failed { .. }),
        "{record:?}"
    );
}

#[test]
fn plugin_instance_ack_gap_replays_without_double_mutation() {
    let plugin_a = plugin_ref("vendor.dist", "1.0.0", 3);
    let mut before_c = controller();
    track_strip(&mut before_c);
    let before = snap_live(&before_c);

    let mut after_c = controller();
    let (_t, strip) = track_strip(&mut after_c);
    let proc = insert_processor(
        &mut after_c,
        strip,
        0,
        "dist",
        ProcessorBinding::Resolved(plugin_a.clone()),
    );
    let mut after_project = after_c.live().clone();
    after_project
        .plugin_instances
        .instances
        .get_mut(&proc)
        .unwrap()
        .id = PluginInstanceId::new(7);
    after_project.plugin_instances.watermark = 8;
    let after = snap_manual(after_project, "after");
    let cs = compare(&before, &after);
    let changes: Vec<&SemanticChange> = cs.changes.iter().collect();
    let intent = intent_for(&before_c, &cs, &after, &changes);
    let src = sources_for(&cs, &before, &after);

    let mut lossy = LossyJournal {
        inner: InMemoryApplyJournal::new(),
    };
    let sources = ApplySources {
        changesets: &src,
        assets: Some(&AllPresentProbe),
        plugins: Some(&AllPlugins),
    };
    // First attempt commits but the Applied receipt is dropped —
    // the journal still holds only Pending.
    let first = apply_selected(
        &mut before_c,
        &intent,
        &sources,
        &mut lossy,
        &SnapshotCancel::new(),
    )
    .unwrap();
    assert_eq!(first.outcome, ApplyOutcome::Applied);
    assert_eq!(
        before_c.live().plugin_instances.instances[&proc].id,
        PluginInstanceId::new(7)
    );

    // Retry: live already equals the after-state for every selected
    // object (the byte-faithful id=7 is what makes the fingerprint
    // reconcile) → Replayed, never a second mutation or re-mint.
    let second = apply_selected(
        &mut before_c,
        &intent,
        &sources,
        &mut lossy,
        &SnapshotCancel::new(),
    )
    .unwrap();
    assert_eq!(second.outcome, ApplyOutcome::Replayed);
    let store = &before_c.live().plugin_instances;
    assert_eq!(store.instances[&proc].id, PluginInstanceId::new(7));
    assert_eq!(store.instances[&proc].plugin, plugin_a);
    assert_eq!(store.watermark, 8);
    assert_eq!(before_c.live().routing.processors.len(), 1);
}
