//! VCS-01 (issue #92): Immutable Snapshot Resolver headless fixtures.
//!
//! Covers test-spec `vcs-01-snapshot-resolver-fixtures` rules
//! SNAP-FIX-01..10: Live/Saved/Recovery/VCS-staged/VCS-committed
//! resolution, race detection, read-only projection, schema migration,
//! degraded completeness, cancellation and budgets. Real OS filesystem,
//! real Git providers, GUI, and plugin-host probing are out of scope
//! (NotRun/Unsupported — see the VCS-01 impl record).

use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

use synvelis_project::*;

// ---------------------------------------------------------------------------
// fixture helpers
// ---------------------------------------------------------------------------

fn controller() -> ProjectController {
    ProjectController::new(
        Project::new(ProjectId::new(0), "p"),
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
    c.apply(cmd(c, op)).expect("fixture op")
}

fn add_track(c: &mut ProjectController, name: &str) -> TrackId {
    apply(c, ProjectOp::AddTrack { name: name.into() });
    *c.live().tracks.keys().next_back().unwrap()
}

fn add_track_with_strip(c: &mut ProjectController, name: &str) -> TrackId {
    let track = add_track(c, name);
    apply(c, ProjectOp::AddTrackStrip { track_id: track });
    track
}

fn register_asset(c: &mut ProjectController, digest: &str) -> AssetId {
    apply(
        c,
        ProjectOp::RegisterAsset {
            digest: digest.into(),
        },
    );
    *c.live().assets.keys().next_back().unwrap()
}

fn no_cancel() -> SnapshotCancel {
    SnapshotCancel::new()
}

/// A `ProjectStore` fixture with scripted HEAD/read sequences and a
/// read-call journal for the read-only oracle.
#[derive(Default)]
struct FakeStore {
    snapshots: BTreeMap<String, Vec<u8>>,
    head: Option<String>,
    head_queue: RefCell<VecDeque<Option<String>>>,
    read_queue: RefCell<VecDeque<Vec<u8>>>,
    reads: RefCell<Vec<String>>,
    head_calls: Cell<usize>,
}

impl FakeStore {
    fn put(&mut self, name: &str, bytes: &[u8]) {
        self.snapshots.insert(name.to_string(), bytes.to_vec());
        self.head = Some(name.to_string());
    }

    fn script_heads(&self, heads: &[Option<&str>]) {
        *self.head_queue.borrow_mut() = heads.iter().map(|h| h.map(str::to_string)).collect();
    }

    fn script_reads(&self, reads: Vec<Vec<u8>>) {
        *self.read_queue.borrow_mut() = reads.into();
    }

    fn bytes_of(&self, name: &str) -> Option<Vec<u8>> {
        self.snapshots.get(name).cloned()
    }
}

impl ProjectStore for FakeStore {
    fn save(
        &mut self,
        _project_id: &str,
        revision: ProjectRevision,
        bytes: &[u8],
    ) -> Result<String, Error> {
        let name = snapshot_name(revision);
        self.snapshots
            .entry(name.clone())
            .or_insert_with(|| bytes.to_vec());
        self.head = Some(name.clone());
        Ok(name)
    }

    fn head(&self) -> Result<Option<String>, Error> {
        self.head_calls.set(self.head_calls.get() + 1);
        if let Some(scripted) = self.head_queue.borrow_mut().pop_front() {
            return Ok(scripted);
        }
        Ok(self.head.clone())
    }

    fn read_snapshot(&self, name: &str) -> Result<Vec<u8>, Error> {
        self.reads.borrow_mut().push(name.to_string());
        if let Some(scripted) = self.read_queue.borrow_mut().pop_front() {
            return Ok(scripted);
        }
        self.snapshots
            .get(name)
            .cloned()
            .ok_or_else(|| Error::Store(format!("missing snapshot {name}")))
    }
}

/// Serializes the live project exactly as `ProjectController::save`
/// would (same envelope construction; `id_watermark` already equals the
/// generator peek in steady state).
fn envelope_bytes(c: &ProjectController) -> Vec<u8> {
    let envelope = SnapshotEnvelope {
        schema_version: SCHEMA_VERSION,
        project_id: c.live().project_id,
        revision: c.live_revision(),
        project: c.live().clone(),
    };
    serde_json::to_vec(&envelope).expect("envelope serialize")
}

/// Serializes the live project as a schema-v5 envelope — the `routing`
/// field and the bootstrap master-fader target did not exist in v5
/// (same fixture shape as `routing_sf01.rs`).
fn as_v5_envelope(c: &ProjectController) -> serde_json::Value {
    let mut env = serde_json::json!({
        "schema_version": 5,
        "project_id": c.live().project_id,
        "revision": c.live_revision(),
        "project": serde_json::to_value(c.live()).unwrap(),
    });
    env["project"].as_object_mut().unwrap().remove("routing");
    env["project"]["control"]["targets"]
        .as_object_mut()
        .unwrap()
        .remove(&ParameterTargetId::new(MASTER_FADER_RAW).to_tagged());
    env
}

/// A `BlobProbe` fixture: digests in `present` report bytes, digests in
/// `absent` report missing, everything else reports unknown.
struct MapProbe {
    present: BTreeSet<String>,
    absent: BTreeSet<String>,
}

impl MapProbe {
    fn new() -> Self {
        Self {
            present: BTreeSet::new(),
            absent: BTreeSet::new(),
        }
    }
}

impl BlobProbe for MapProbe {
    fn probe(&self, digest: &str) -> BlobRead {
        if self.present.contains(digest) {
            BlobRead::Present(vec![0xde, 0xad])
        } else if self.absent.contains(digest) {
            BlobRead::Absent
        } else {
            BlobRead::Unknown
        }
    }
}

/// A `VcsBytesPort` fixture with per-locator file/blob tables, a
/// scripted generation sequence, and a call journal.
#[derive(Default)]
struct FakeVcs {
    /// `"staged:{path}"` or `"committed:{rev}:{path}"` → file read.
    files: BTreeMap<String, VcsFileRead>,
    /// `"staged:{digest}"` or `"committed:{rev}:{digest}"` → blob read.
    blobs: BTreeMap<String, BlobRead>,
    generation: IndexGeneration,
    gen_queue: RefCell<VecDeque<IndexGeneration>>,
    calls: RefCell<Vec<String>>,
}

impl FakeVcs {
    fn new(generation: &str) -> Self {
        Self {
            generation: IndexGeneration(generation.to_string()),
            ..Self::default()
        }
    }

    fn file_key(locator: &VcsLocator, path: &str) -> String {
        match locator {
            VcsLocator::Staged { .. } => format!("staged:{path}"),
            VcsLocator::Committed { fixed_revision, .. } => {
                format!("committed:{fixed_revision}:{path}")
            }
        }
    }

    fn blob_key(locator: &VcsLocator, digest: &str) -> String {
        match locator {
            VcsLocator::Staged { .. } => format!("staged:{digest}"),
            VcsLocator::Committed { fixed_revision, .. } => {
                format!("committed:{fixed_revision}:{digest}")
            }
        }
    }

    fn put_file(&mut self, locator: &VcsLocator, path: &str, read: VcsFileRead) {
        self.files.insert(Self::file_key(locator, path), read);
    }

    fn put_blob(&mut self, locator: &VcsLocator, digest: &str, read: BlobRead) {
        self.blobs.insert(Self::blob_key(locator, digest), read);
    }

    fn script_generations(&self, generations: &[&str]) {
        *self.gen_queue.borrow_mut() = generations
            .iter()
            .map(|g| IndexGeneration((*g).to_string()))
            .collect();
    }
}

impl VcsBytesPort for FakeVcs {
    fn index_generation(&self, repository_id: &str) -> Result<IndexGeneration, VcsPortError> {
        self.calls
            .borrow_mut()
            .push(format!("index_generation:{repository_id}"));
        if let Some(scripted) = self.gen_queue.borrow_mut().pop_front() {
            return Ok(scripted);
        }
        Ok(self.generation.clone())
    }

    fn read_file(&self, locator: &VcsLocator, path: &str) -> Result<VcsFileRead, VcsPortError> {
        let key = Self::file_key(locator, path);
        self.calls.borrow_mut().push(format!("read_file:{key}"));
        Ok(self.files.get(&key).cloned().unwrap_or(VcsFileRead::Absent))
    }

    fn read_blob(&self, locator: &VcsLocator, digest: &str) -> Result<BlobRead, VcsPortError> {
        let key = Self::blob_key(locator, digest);
        self.calls.borrow_mut().push(format!("read_blob:{key}"));
        Ok(self.blobs.get(&key).cloned().unwrap_or(BlobRead::Absent))
    }
}

fn staged_locator(repo: &str, path: &str) -> SnapshotLocator {
    SnapshotLocator::VcsStaged {
        repository_id: repo.into(),
        project_path: path.into(),
        index_generation: None,
    }
}

fn committed_locator(repo: &str, rev: &str, path: &str) -> SnapshotLocator {
    SnapshotLocator::VcsCommitted {
        repository_id: repo.into(),
        fixed_revision: rev.into(),
        project_path: path.into(),
    }
}

fn staged_vcs_locator(repo: &str, path: &str, generation: &str) -> VcsLocator {
    VcsLocator::Staged {
        repository_id: repo.into(),
        project_path: path.into(),
        index_generation: IndexGeneration(generation.into()),
    }
}

fn committed_vcs_locator(repo: &str, rev: &str, path: &str) -> VcsLocator {
    VcsLocator::Committed {
        repository_id: repo.into(),
        fixed_revision: rev.into(),
        project_path: path.into(),
    }
}

// ---------------------------------------------------------------------------
// SNAP-FIX-01/02/09: Live capture immutability + Live/Saved digest parity
// ---------------------------------------------------------------------------

#[test]
fn live_capture_is_immutable_across_edits_undo_and_save() {
    let mut c = controller();
    add_track(&mut c, "before");
    let rev = c.live_revision();
    let resolver = SnapshotResolver::new();
    let cancel = no_cancel();

    let snap = resolver
        .capture_live(&c, rev, &cancel, None)
        .expect("live capture");
    assert_eq!(snap.origin, SnapshotOrigin::Live);
    assert_eq!(snap.projection.project().tracks.len(), 1);
    let digest = snap.identity.digest;

    // Live edits, undo, and save must not move the captured snapshot.
    add_track(&mut c, "after");
    assert!(c.undo().unwrap());
    let mut store = InMemoryStore::new();
    c.save(&mut store).unwrap();

    assert_eq!(snap.identity.digest, digest);
    assert_eq!(snap.projection.project().tracks.len(), 1);
    assert_eq!(snap.projection.project().name, "p");
    // The controller kept its single-writer semantics throughout.
    assert!(c.live_revision().0 >= 3);
    assert_eq!(c.live().tracks.len(), 1); // undo reverted "after"
}

#[test]
fn stale_live_revision_is_explicit() {
    let mut c = controller();
    add_track(&mut c, "t");
    let resolver = SnapshotResolver::new();
    let cancel = no_cancel();
    let stale = ProjectRevision(c.live_revision().0 + 7);
    let err = resolver
        .capture_live(&c, stale, &cancel, None)
        .expect_err("stale revision");
    assert_eq!(
        err,
        SnapshotError::StaleLiveRevision {
            expected: stale,
            actual: c.live_revision(),
        }
    );
}

#[test]
fn live_and_saved_captures_share_digest_but_not_identity() {
    let mut c = controller();
    add_track(&mut c, "t");
    register_asset(&mut c, "d1");
    let rev = c.live_revision();
    let resolver = SnapshotResolver::new();
    let cancel = no_cancel();

    let live = resolver
        .capture_live(&c, rev, &cancel, None)
        .expect("live capture");
    let mut store = FakeStore::default();
    c.save(&mut store).unwrap();
    let saved = resolver
        .resolve_saved_head(&store, c.live().project_id, &cancel, None)
        .expect("saved resolve");

    // Same canonical bytes → same content digest; provenance differs.
    assert_eq!(live.identity.digest, saved.identity.digest);
    assert_eq!(live.identity.project_id, saved.identity.project_id);
    assert_eq!(live.identity.schema_version, saved.identity.schema_version);
    assert_ne!(live.identity.source, saved.identity.source);
    assert_ne!(live.identity.locator, saved.identity.locator);
    assert_ne!(
        live.identity.capture_generation,
        saved.identity.capture_generation
    );
    assert_eq!(live.projection.project(), saved.projection.project());
}

// ---------------------------------------------------------------------------
// SNAP-FIX-03: Saved HEAD window races
// ---------------------------------------------------------------------------

#[test]
fn saved_head_movement_inside_window_is_head_conflict() {
    let mut c = controller();
    add_track(&mut c, "t");
    let bytes = envelope_bytes(&c);

    let mut store = FakeStore::default();
    store.put("r1.json", &bytes);
    store.put("r2.json", &bytes);
    store.script_heads(&[Some("r1.json"), Some("r2.json")]);

    let resolver = SnapshotResolver::new();
    let err = resolver
        .resolve_saved_head(&store, c.live().project_id, &no_cancel(), None)
        .expect_err("HEAD moved mid-capture");
    assert_eq!(
        err,
        SnapshotError::HeadConflict {
            expected: Some("r1.json".into()),
            actual: Some("r2.json".into()),
        }
    );
}

#[test]
fn saved_head_ref_pin_mismatch_is_head_conflict() {
    let mut c = controller();
    add_track(&mut c, "t");
    let bytes = envelope_bytes(&c);

    let mut store = FakeStore::default();
    store.put("r1.json", &bytes);

    let resolver = SnapshotResolver::new();
    let locator = SnapshotLocator::Saved {
        project_id: c.live().project_id,
        head_ref: Some("r9.json".into()),
    };
    let err = resolver
        .resolve_saved(&store, &locator, &no_cancel(), None)
        .expect_err("head_ref pin mismatch");
    assert_eq!(
        err,
        SnapshotError::HeadConflict {
            expected: Some("r9.json".into()),
            actual: Some("r1.json".into()),
        }
    );
    // The pin check happens before any snapshot read.
    assert!(store.reads.borrow().is_empty());
}

#[test]
fn saved_head_ref_pin_match_resolves() {
    let mut c = controller();
    add_track(&mut c, "t");
    let mut store = FakeStore::default();
    c.save(&mut store).unwrap();
    let head = store.head().unwrap().unwrap();

    let resolver = SnapshotResolver::new();
    let locator = SnapshotLocator::Saved {
        project_id: c.live().project_id,
        head_ref: Some(head.clone()),
    };
    let snap = resolver
        .resolve_saved(&store, &locator, &no_cancel(), None)
        .expect("pinned head resolves");
    assert_eq!(
        snap.identity.locator,
        FixedSnapshotLocator::Saved {
            project_id: c.live().project_id,
            head,
        }
    );
}

#[test]
fn same_name_bytes_change_during_capture_is_source_changed() {
    let mut c = controller();
    add_track(&mut c, "t");
    let bytes_a = envelope_bytes(&c);
    add_track(&mut c, "u");
    let bytes_b = envelope_bytes(&c);

    let mut store = FakeStore::default();
    store.put("r1.json", &bytes_a);
    store.script_reads(vec![bytes_a.clone(), bytes_b]);

    let resolver = SnapshotResolver::new();
    let err = resolver
        .resolve_saved_head(&store, c.live().project_id, &no_cancel(), None)
        .expect_err("bytes changed mid-capture");
    assert!(matches!(err, SnapshotError::SourceChanged { .. }));
}

#[test]
fn saved_resolve_without_head_is_not_found() {
    let store = FakeStore::default();
    let resolver = SnapshotResolver::new();
    let err = resolver
        .resolve_saved_head(&store, ProjectId::new(0), &no_cancel(), None)
        .expect_err("empty store has no HEAD");
    assert_eq!(
        err,
        SnapshotError::NotFound {
            detail: "store has no HEAD".into(),
        }
    );
}

// ---------------------------------------------------------------------------
// SNAP-FIX-04/10: Recovery boundary + identity disambiguation
// ---------------------------------------------------------------------------

#[test]
fn recovery_checkpoint_is_isolated_from_saved_head() {
    let mut c = controller();
    add_track(&mut c, "saved-state");
    let mut saved_store = FakeStore::default();
    c.save(&mut saved_store).unwrap();

    // A separate recovery-rooted store holds a *different* checkpoint.
    let mut c2 = controller();
    add_track(&mut c2, "recovered-state");
    add_track(&mut c2, "extra");
    let recovery_bytes = envelope_bytes(&c2);
    let mut recovery_store = FakeStore::default();
    recovery_store.put("checkpoint-a", &recovery_bytes);

    let resolver = SnapshotResolver::new();
    let cancel = no_cancel();
    let saved = resolver
        .resolve_saved_head(&saved_store, c.live().project_id, &cancel, None)
        .expect("saved resolve");
    let recovery = resolver
        .resolve_recovery(
            &recovery_store,
            &SnapshotLocator::Recovery {
                project_id: c2.live().project_id,
                checkpoint: "checkpoint-a".into(),
            },
            &cancel,
            None,
        )
        .expect("recovery resolve");

    assert_eq!(saved.origin, SnapshotOrigin::Saved);
    assert_eq!(recovery.origin, SnapshotOrigin::Recovery);
    assert_ne!(saved.identity.digest, recovery.identity.digest);
    assert_ne!(saved.identity.locator, recovery.identity.locator);
    assert_eq!(recovery.projection.project().tracks.len(), 2);
    assert_eq!(saved.projection.project().tracks.len(), 1);

    // The recovery resolve never consulted the saved store, and the
    // saved resolve never consulted the recovery store.
    assert_eq!(saved_store.reads.borrow().len(), 2);
    assert_eq!(recovery_store.reads.borrow().len(), 2);
    assert!(
        recovery_store
            .reads
            .borrow()
            .iter()
            .all(|name| name == "checkpoint-a")
    );
}

#[test]
fn same_named_snapshots_are_distinguished_by_full_identity() {
    // Two different projects can both occupy the snapshot name `r1.json`
    // in different stores — the name alone proves nothing.
    let mut c_a = controller();
    add_track(&mut c_a, "a");
    let mut store_a = FakeStore::default();
    c_a.save(&mut store_a).unwrap();

    let mut c_b = ProjectController::new(
        Project::new(ProjectId::new(77), "other"),
        Box::new(CounterIdGenerator::new(1)),
    );
    add_track(&mut c_b, "b");
    let mut store_b = FakeStore::default();
    c_b.save(&mut store_b).unwrap();

    assert_eq!(store_a.head().unwrap(), store_b.head().unwrap()); // both r1.json? revisions may differ

    let resolver = SnapshotResolver::new();
    let cancel = no_cancel();
    let a = resolver
        .resolve_saved_head(&store_a, c_a.live().project_id, &cancel, None)
        .unwrap();
    let b = resolver
        .resolve_saved_head(&store_b, c_b.live().project_id, &cancel, None)
        .unwrap();

    // Same head name does not imply same snapshot: project id, digest,
    // and locator all participate in identity.
    assert_ne!(a.identity.project_id, b.identity.project_id);
    assert_ne!(a.identity.digest, b.identity.digest);
    assert_ne!(a.identity.locator, b.identity.locator);
}

// ---------------------------------------------------------------------------
// SNAP-FIX-05: schema migration + explicit rejection
// ---------------------------------------------------------------------------

#[test]
fn v5_snapshot_projects_read_only_into_current_schema() {
    let mut c = controller();
    add_track_with_strip(&mut c, "legacy");
    let v5_bytes = serde_json::to_vec(&as_v5_envelope(&c)).unwrap();

    let mut store = FakeStore::default();
    store.put("r7.json", &v5_bytes);
    let resolver = SnapshotResolver::new();
    let snap = resolver
        .resolve_saved_head(&store, c.live().project_id, &no_cancel(), None)
        .expect("v5 snapshot resolves");

    assert_eq!(snap.identity.schema_version, 5);
    assert_eq!(snap.identity.projection_version, SCHEMA_VERSION);
    assert_eq!(snap.projection.source_schema_version(), 5);
    assert_eq!(snap.projection.projection_version(), SCHEMA_VERSION);
    // The v5→v6 ladder projected the legacy clip→master routing.
    assert_eq!(snap.projection.project().routing.strips.len(), 2);
    // Source bytes were never rewritten.
    assert_eq!(store.bytes_of("r7.json").unwrap(), v5_bytes);
}

#[test]
fn unknown_and_too_old_schema_versions_are_rejected() {
    let mut c = controller();
    add_track(&mut c, "t");
    let mut env = serde_json::to_value(as_v5_envelope(&c)).unwrap();

    for version in [3u32, 10, 99] {
        env["schema_version"] = serde_json::json!(version);
        let bytes = serde_json::to_vec(&env).unwrap();
        let mut store = FakeStore::default();
        store.put("r1.json", &bytes);
        let resolver = SnapshotResolver::new();
        let err = resolver
            .resolve_saved_head(&store, c.live().project_id, &no_cancel(), None)
            .expect_err("unsupported schema");
        assert!(
            matches!(err, SnapshotError::UnsupportedSchema { .. }),
            "schema {version}: {err:?}"
        );
    }
}

#[test]
fn corrupted_and_invalid_snapshots_are_explicit_failures() {
    let mut c = controller();
    add_track(&mut c, "t");
    let project_id = c.live().project_id;
    let resolver = SnapshotResolver::new();
    let cancel = no_cancel();

    // Corrupt JSON.
    let mut store = FakeStore::default();
    store.put("r1.json", b"{not json");
    let err = resolver
        .resolve_saved_head(&store, project_id, &cancel, None)
        .expect_err("corrupt bytes");
    assert!(matches!(err, SnapshotError::InvalidSnapshot(_)));

    // Invalid internal reference: drop every asset while a source
    // still points at one.
    register_asset(&mut c, "d1");
    let asset_id = *c.live().assets.keys().next().unwrap();
    apply(
        &mut c,
        ProjectOp::RegisterSource {
            asset_id,
            content_digest: "d1".into(),
        },
    );
    let mut env = envelope_project_json(&c);
    env["project"]["assets"] = serde_json::json!({});
    let bytes = serde_json::to_vec(&env).unwrap();
    let mut store2 = FakeStore::default();
    store2.put("r1.json", &bytes);
    let err = resolver
        .resolve_saved_head(&store2, project_id, &cancel, None)
        .expect_err("dangling source reference");
    assert!(matches!(err, SnapshotError::InvalidSnapshot(_)));

    // Forged watermark below issued IDs.
    let mut env3 = envelope_project_json(&c);
    env3["project"]["id_watermark"] = serde_json::json!(0);
    let bytes3 = serde_json::to_vec(&env3).unwrap();
    let mut store3 = FakeStore::default();
    store3.put("r1.json", &bytes3);
    let err = resolver
        .resolve_saved_head(&store3, project_id, &cancel, None)
        .expect_err("forged watermark");
    assert!(matches!(err, SnapshotError::InvalidSnapshot(_)));

    // Envelope project_id disagreeing with the locator.
    let mut env4 = envelope_project_json(&c);
    env4["project_id"] = serde_json::json!("project-000000000000000000000000000000ff");
    let bytes4 = serde_json::to_vec(&env4).unwrap();
    let mut store4 = FakeStore::default();
    store4.put("r1.json", &bytes4);
    let err = resolver
        .resolve_saved_head(&store4, project_id, &cancel, None)
        .expect_err("project id mismatch");
    assert!(matches!(err, SnapshotError::InvalidSnapshot(_)));
}

/// The live project as a full v9 envelope JSON value.
fn envelope_project_json(c: &ProjectController) -> serde_json::Value {
    serde_json::json!({
        "schema_version": SCHEMA_VERSION,
        "project_id": c.live().project_id,
        "revision": c.live_revision(),
        "project": serde_json::to_value(c.live()).unwrap(),
    })
}

// ---------------------------------------------------------------------------
// SNAP-FIX-06: asset/plugin completeness is explicit, never silent Equal
// ---------------------------------------------------------------------------

#[test]
fn missing_unknown_and_unresolved_regions_are_degraded() {
    let mut c = controller();
    register_asset(&mut c, "present-d");
    register_asset(&mut c, "absent-d");
    register_asset(&mut c, "mystery-d");
    let track = add_track_with_strip(&mut c, "t");
    let strip_id = c.live().routing.strip_of_track(track).unwrap().id;
    apply(
        &mut c,
        ProjectOp::InsertProcessor {
            strip_id,
            index: 0,
            name: "eq".into(),
            binding: ProcessorBinding::Unresolved {
                descriptor: "vendor.missing-eq@1.0".into(),
            },
            sidechain_input: false,
        },
    );

    let mut probe = MapProbe::new();
    probe.present.insert("present-d".into());
    probe.absent.insert("absent-d".into());
    // "mystery-d" stays Unknown — no probe result.

    let mut store = FakeStore::default();
    c.save(&mut store).unwrap();
    let resolver = SnapshotResolver::new();
    let snap = resolver
        .resolve_saved_head(&store, c.live().project_id, &no_cancel(), Some(&probe))
        .expect("degraded snapshot still resolves");

    let statuses: BTreeMap<_, _> = snap
        .assets
        .entries
        .iter()
        .map(|e| (e.digest.as_str(), e.status))
        .collect();
    assert_eq!(statuses["present-d"], ContentStatus::Present);
    assert_eq!(statuses["absent-d"], ContentStatus::Missing);
    assert_eq!(statuses["mystery-d"], ContentStatus::Unknown);
    assert_eq!(snap.completeness, SnapshotCompleteness::Partial);
    assert_eq!(snap.validation, SnapshotValidation::Degraded);

    // The unresolved plugin is an explicit Missing opaque state.
    assert_eq!(snap.opaque_state.len(), 1);
    assert_eq!(snap.opaque_state[0].status, OpaqueStateStatus::Missing);
    assert_eq!(
        snap.opaque_state[0].descriptor.as_deref(),
        Some("vendor.missing-eq@1.0")
    );
}

#[test]
fn absent_probe_marks_every_asset_unknown_not_present() {
    let mut c = controller();
    register_asset(&mut c, "d1");
    let mut store = FakeStore::default();
    c.save(&mut store).unwrap();

    let resolver = SnapshotResolver::new();
    let snap = resolver
        .resolve_saved_head(&store, c.live().project_id, &no_cancel(), None)
        .expect("resolve without probe");
    assert_eq!(snap.assets.entries[0].status, ContentStatus::Unknown);
    assert_eq!(snap.completeness, SnapshotCompleteness::Partial);
    assert_eq!(snap.validation, SnapshotValidation::Degraded);
}

#[test]
fn fully_present_project_with_no_processors_is_complete() {
    let mut c = controller();
    register_asset(&mut c, "d1");
    let mut probe = MapProbe::new();
    probe.present.insert("d1".into());

    let mut store = FakeStore::default();
    c.save(&mut store).unwrap();
    let resolver = SnapshotResolver::new();
    let snap = resolver
        .resolve_saved_head(&store, c.live().project_id, &no_cancel(), Some(&probe))
        .expect("resolve");
    assert_eq!(snap.opaque_state.len(), 0);
    assert_eq!(snap.completeness, SnapshotCompleteness::Complete);
    assert_eq!(snap.validation, SnapshotValidation::Valid);
}

// ---------------------------------------------------------------------------
// SNAP-FIX-07: VCS staged/committed resolution
// ---------------------------------------------------------------------------

#[test]
fn vcs_staged_capture_pins_stable_index_generation() {
    let mut c = controller();
    register_asset(&mut c, "blob-d");
    let bytes = envelope_bytes(&c);

    let loc = staged_vcs_locator("repo-1", "project/main.json", "gen-1");
    let mut vcs = FakeVcs::new("gen-1");
    vcs.put_file(&loc, "project/main.json", VcsFileRead::Present(bytes));
    vcs.put_blob(&loc, "blob-d", BlobRead::Present(vec![1, 2, 3]));

    let resolver = SnapshotResolver::new();
    let snap = resolver
        .resolve_vcs(
            &vcs,
            &staged_locator("repo-1", "project/main.json"),
            &no_cancel(),
        )
        .expect("staged resolve");

    assert_eq!(snap.origin, SnapshotOrigin::VcsStaged);
    assert_eq!(
        snap.identity.locator,
        FixedSnapshotLocator::VcsStaged {
            repository_id: "repo-1".into(),
            project_path: "project/main.json".into(),
            index_generation: IndexGeneration("gen-1".into()),
        }
    );
    assert_eq!(snap.assets.entries[0].status, ContentStatus::Present);
    assert_eq!(snap.completeness, SnapshotCompleteness::Complete);
    // Two generation reads bracket the capture window.
    assert_eq!(
        vcs.calls
            .borrow()
            .iter()
            .filter(|c| c.starts_with("index_generation"))
            .count(),
        2
    );
}

#[test]
fn vcs_staged_index_mutation_is_index_changed() {
    let mut c = controller();
    add_track(&mut c, "t");
    let bytes = envelope_bytes(&c);

    let loc = staged_vcs_locator("repo-1", "p.json", "gen-1");
    let mut vcs = FakeVcs::new("gen-1");
    vcs.put_file(&loc, "p.json", VcsFileRead::Present(bytes));
    vcs.script_generations(&["gen-1", "gen-2"]);

    let resolver = SnapshotResolver::new();
    let err = resolver
        .resolve_vcs(&vcs, &staged_locator("repo-1", "p.json"), &no_cancel())
        .expect_err("index moved mid-capture");
    assert_eq!(
        err,
        SnapshotError::IndexChanged {
            before: IndexGeneration("gen-1".into()),
            after: IndexGeneration("gen-2".into()),
        }
    );
}

#[test]
fn vcs_staged_pinned_generation_mismatch_is_index_changed() {
    let mut c = controller();
    add_track(&mut c, "t");
    let bytes = envelope_bytes(&c);

    let loc = staged_vcs_locator("repo-1", "p.json", "gen-1");
    let mut vcs = FakeVcs::new("gen-9"); // port reports a different generation
    vcs.put_file(&loc, "p.json", VcsFileRead::Present(bytes));

    let resolver = SnapshotResolver::new();
    let locator = SnapshotLocator::VcsStaged {
        repository_id: "repo-1".into(),
        project_path: "p.json".into(),
        index_generation: Some(IndexGeneration("gen-1".into())),
    };
    let err = resolver
        .resolve_vcs(&vcs, &locator, &no_cancel())
        .expect_err("pinned generation mismatch");
    assert!(matches!(err, SnapshotError::IndexChanged { .. }));
}

#[test]
fn vcs_staged_partial_stage_is_explicit_failure() {
    let mut c = controller();
    register_asset(&mut c, "missing-blob");
    let bytes = envelope_bytes(&c);

    let loc = staged_vcs_locator("repo-1", "p.json", "gen-1");
    let mut vcs = FakeVcs::new("gen-1");
    vcs.put_file(&loc, "p.json", VcsFileRead::Present(bytes));
    // "missing-blob" has no staged blob → FakeVcs defaults to Absent.

    let resolver = SnapshotResolver::new();
    let err = resolver
        .resolve_vcs(&vcs, &staged_locator("repo-1", "p.json"), &no_cancel())
        .expect_err("partial stage");
    assert_eq!(
        err,
        SnapshotError::PartialStage {
            missing: vec!["missing-blob".into()],
        }
    );
}

#[test]
fn vcs_staged_absent_project_file_is_partial_stage() {
    let vcs = FakeVcs::new("gen-1"); // nothing staged at all
    let resolver = SnapshotResolver::new();
    let err = resolver
        .resolve_vcs(&vcs, &staged_locator("repo-1", "p.json"), &no_cancel())
        .expect_err("project file absent from index");
    assert_eq!(
        err,
        SnapshotError::PartialStage {
            missing: vec!["p.json".into()],
        }
    );
}

#[test]
fn vcs_committed_absent_blob_is_degraded_not_failure() {
    let mut c = controller();
    register_asset(&mut c, "gone-blob");
    let bytes = envelope_bytes(&c);

    let loc = committed_vcs_locator("repo-1", "rev-abc", "p.json");
    let mut vcs = FakeVcs::new("gen-1");
    vcs.put_file(&loc, "p.json", VcsFileRead::Present(bytes));
    // "gone-blob" absent at the committed revision → Missing record.

    let resolver = SnapshotResolver::new();
    let snap = resolver
        .resolve_vcs(
            &vcs,
            &committed_locator("repo-1", "rev-abc", "p.json"),
            &no_cancel(),
        )
        .expect("committed resolve tolerates absent blobs as degraded");

    assert_eq!(snap.origin, SnapshotOrigin::VcsCommitted);
    assert_eq!(snap.assets.entries[0].status, ContentStatus::Missing);
    assert_eq!(snap.completeness, SnapshotCompleteness::Partial);
    assert_eq!(snap.validation, SnapshotValidation::Degraded);
    // Committed reads never consult the index generation.
    assert!(
        vcs.calls
            .borrow()
            .iter()
            .all(|c| !c.starts_with("index_generation"))
    );
}

#[test]
fn vcs_committed_absent_project_file_is_not_found() {
    let vcs = FakeVcs::new("gen-1");
    let resolver = SnapshotResolver::new();
    let err = resolver
        .resolve_vcs(
            &vcs,
            &committed_locator("repo-1", "rev-abc", "p.json"),
            &no_cancel(),
        )
        .expect_err("project file absent at revision");
    assert!(matches!(err, SnapshotError::NotFound { .. }));
}

#[test]
fn wrong_locator_variant_is_invalid_locator() {
    let store = FakeStore::default();
    let resolver = SnapshotResolver::new();
    let err = resolver
        .resolve_recovery(
            &store,
            &SnapshotLocator::Saved {
                project_id: ProjectId::new(0),
                head_ref: None,
            },
            &no_cancel(),
            None,
        )
        .expect_err("saved locator into recovery entry point");
    assert!(matches!(err, SnapshotError::InvalidLocator(_)));
}

// ---------------------------------------------------------------------------
// SNAP-FIX-08: cancellation + budgets
// ---------------------------------------------------------------------------

#[test]
fn cancelled_token_fails_explicitly_at_phase_boundaries() {
    let mut c = controller();
    add_track(&mut c, "t");
    let mut store = FakeStore::default();
    c.save(&mut store).unwrap();

    let resolver = SnapshotResolver::new();
    let cancel = SnapshotCancel::new();
    cancel.cancel();

    let err = resolver
        .capture_live(&c, c.live_revision(), &cancel, None)
        .expect_err("cancelled live capture");
    assert_eq!(err, SnapshotError::Cancelled);
    let err = resolver
        .resolve_saved_head(&store, c.live().project_id, &cancel, None)
        .expect_err("cancelled saved resolve");
    assert_eq!(err, SnapshotError::Cancelled);
    let err = resolver
        .resolve_vcs(&FakeVcs::new("g"), &staged_locator("r", "p"), &cancel)
        .expect_err("cancelled vcs resolve");
    assert_eq!(err, SnapshotError::Cancelled);
}

#[test]
fn cancellation_during_asset_probe_is_explicit() {
    struct CancellingProbe {
        cancel: SnapshotCancel,
    }
    impl BlobProbe for CancellingProbe {
        fn probe(&self, _digest: &str) -> BlobRead {
            self.cancel.cancel();
            BlobRead::Unknown
        }
    }

    let mut c = controller();
    register_asset(&mut c, "d1");
    let mut store = FakeStore::default();
    c.save(&mut store).unwrap();

    let cancel = SnapshotCancel::new();
    let probe = CancellingProbe {
        cancel: cancel.clone(),
    };
    let resolver = SnapshotResolver::new();
    let err = resolver
        .resolve_saved_head(&store, c.live().project_id, &cancel, Some(&probe))
        .expect_err("cancel mid-pipeline");
    assert_eq!(err, SnapshotError::Cancelled);
}

#[test]
fn oversized_snapshot_is_budget_exceeded() {
    let mut c = controller();
    add_track(&mut c, "t");
    let mut store = FakeStore::default();
    c.save(&mut store).unwrap();

    let resolver = SnapshotResolver::with_limits(SnapshotLimits {
        max_snapshot_bytes: 8,
    });
    let err = resolver
        .resolve_saved_head(&store, c.live().project_id, &no_cancel(), None)
        .expect_err("tiny budget");
    assert!(matches!(err, SnapshotError::BudgetExceeded { .. }));
}

// ---------------------------------------------------------------------------
// SNAP-FIX-09: read-only oracle — resolver mutates nothing
// ---------------------------------------------------------------------------

#[test]
fn resolution_never_mutates_controller_store_or_vcs_port() {
    let mut c = controller();
    add_track(&mut c, "t");
    register_asset(&mut c, "d1");
    let rev_before = c.live_revision();
    let live_before = c.live().clone();

    let mut store = FakeStore::default();
    c.save(&mut store).unwrap();
    let saved_state = c.saved().cloned();
    let head = store.head().unwrap().unwrap();
    let snapshot_bytes_before = store.bytes_of(&head);

    let loc = staged_vcs_locator("repo-1", "p.json", "gen-1");
    let mut vcs = FakeVcs::new("gen-1");
    vcs.put_file(&loc, "p.json", VcsFileRead::Present(envelope_bytes(&c)));
    vcs.put_blob(&loc, "d1", BlobRead::Present(vec![9]));

    let resolver = SnapshotResolver::new();
    let cancel = no_cancel();
    resolver
        .capture_live(&c, rev_before, &cancel, None)
        .unwrap();
    resolver
        .resolve_saved_head(&store, c.live().project_id, &cancel, None)
        .unwrap();
    resolver
        .resolve_vcs(&vcs, &staged_locator("repo-1", "p.json"), &cancel)
        .unwrap();

    // Controller: revision, live content, saved metadata all untouched.
    assert_eq!(c.live_revision(), rev_before);
    assert_eq!(c.live(), &live_before);
    assert_eq!(c.saved(), saved_state.as_ref());

    // Store: HEAD and snapshot bytes untouched; only reads happened.
    assert_eq!(store.head().unwrap(), Some(head.clone()));
    assert_eq!(store.bytes_of(&head), snapshot_bytes_before);
    assert_eq!(store.reads.borrow().len(), 2); // bytes window: 2 reads of HEAD

    // VCS port: only read calls were recorded (no write surface exists).
    assert!(
        vcs.calls
            .borrow()
            .iter()
            .all(|call| call.starts_with("read_") || call.starts_with("index_generation"))
    );

    // Undo/redo still functions after resolution (single-writer intact).
    add_track(&mut c, "post-resolve");
    assert!(c.undo().unwrap());
    assert_eq!(c.live().tracks.len(), 1);
}
