//! SYN-IO-02 store publish contract tests (test-spec
//! `syn-io-02-store-publish-contract`): `DirectoryBundleStore` after
//! the `ruviform-io-file` staged-publish migration — snapshot
//! immutability, HEAD ordering, honest failure propagation, orphan /
//! leftover handling, external change and reopen behavior.

use std::fs;
use std::path::PathBuf;

use synvelis_project::*;

/// Unique temp bundle root; cleaned by the guard on drop.
fn workdir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "synvelis-store-publish-{tag}-{}-{:x}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos())
    ))
}

struct BundleGuard(PathBuf);

impl Drop for BundleGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

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

fn bundle(tag: &str) -> (BundleGuard, PathBuf, DirectoryBundleStore) {
    let dir = workdir(tag);
    let store = DirectoryBundleStore::create(&dir).unwrap();
    (BundleGuard(dir.clone()), dir, store)
}

// E1 (Rule 2, 7): two saves publish both immutable snapshots and
// repoint HEAD to the newest — the HEAD file holds the name verbatim.
#[test]
fn saves_publish_snapshots_then_repoint_head() {
    let (_g, dir, mut store) = bundle("basic");
    let mut c = controller();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "a".into() }))
        .unwrap();
    let saved = c.save(&mut store).unwrap();
    assert_eq!(saved.head, "r1.json");
    assert_eq!(fs::read_to_string(dir.join("HEAD")).unwrap(), "r1.json");
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "b".into() }))
        .unwrap();
    let saved2 = c.save(&mut store).unwrap();
    assert_eq!(saved2.head, "r2.json");
    assert_eq!(fs::read_to_string(dir.join("HEAD")).unwrap(), "r2.json");
    assert!(dir.join("snapshots").join("r1.json").exists());
    assert!(dir.join("snapshots").join("r2.json").exists());
}

// E2 (Rule 1): an immutable snapshot name refuses a byte-different
// republish — the saved bytes and HEAD stay untouched. A retry with
// byte-identical content is idempotent (crash-recovery restore path)
// and still repoints HEAD.
#[test]
fn same_revision_resave_never_overwrites_snapshot() {
    let (_g, dir, mut store) = bundle("immutable");
    let mut c = controller();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "a".into() }))
        .unwrap();
    c.save(&mut store).unwrap();
    let original = fs::read(dir.join("snapshots").join("r1.json")).unwrap();

    // Byte-different republish of the same immutable name is refused.
    let err = store
        .save("p", ProjectRevision(1), b"different-bytes")
        .unwrap_err();
    assert!(
        matches!(err, Error::Store(ref s) if s.contains("effect:")),
        "publish classification must propagate: {err}"
    );
    assert_eq!(
        fs::read(dir.join("snapshots").join("r1.json")).unwrap(),
        original
    );
    assert_eq!(fs::read_to_string(dir.join("HEAD")).unwrap(), "r1.json");

    // Byte-identical retry is idempotent: the already-published
    // snapshot satisfies CreateNew and HEAD is repointed again.
    store.save("p", ProjectRevision(1), &original).unwrap();
    assert_eq!(fs::read_to_string(dir.join("HEAD")).unwrap(), "r1.json");
    assert_eq!(
        fs::read(dir.join("snapshots").join("r1.json")).unwrap(),
        original
    );
}

// Rule 1 (watermark bookkeeping): `draw_raw_id` reservations consume
// ids without a revision bump, so a legitimate re-save of the same
// revision differs only in `id_watermark`. The store accepts it as a
// monotone refresh of the same logical snapshot — the file records
// the newer watermark and HEAD is repointed.
#[test]
fn same_revision_resave_advances_id_watermark() {
    let (_g, dir, mut store) = bundle("watermark");
    let mut c = controller();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "a".into() }))
        .unwrap();
    c.save(&mut store).unwrap();
    let original = fs::read(dir.join("snapshots").join("r1.json")).unwrap();
    let env0: SnapshotEnvelope = serde_json::from_slice(&original).unwrap();

    // Two external reservations (import/manifest draws) then a save.
    let _reserved = c.draw_raw_id();
    let _reserved2 = c.draw_raw_id();
    c.save(&mut store).unwrap();

    assert_eq!(fs::read_to_string(dir.join("HEAD")).unwrap(), "r1.json");
    let refreshed = fs::read(dir.join("snapshots").join("r1.json")).unwrap();
    let env1: SnapshotEnvelope = serde_json::from_slice(&refreshed).unwrap();
    assert_eq!(
        env1.project.id_watermark,
        env0.project.id_watermark + 2,
        "the monotone watermark record advances, content unchanged"
    );
    assert_eq!(env1.project.tracks.len(), 1);

    // A same-content snapshot carrying an *older* watermark is
    // satisfied without replacing — the stored record never regresses.
    let mut older = env1.clone();
    older.project.id_watermark = 0;
    let older_bytes = serde_json::to_vec(&older).unwrap();
    store.save("p", ProjectRevision(1), &older_bytes).unwrap();
    assert_eq!(
        fs::read(dir.join("snapshots").join("r1.json")).unwrap(),
        refreshed
    );
}

// Rule 1 (crash recovery): an orphan snapshot — published but HEAD
// never repointed — is re-adopted by a `Satisfied` re-save of the
// same bytes: HEAD is repointed and the snapshot becomes loadable.
#[test]
fn satisfied_resave_repoints_head_onto_orphan_snapshot() {
    let (_g, dir, mut store) = bundle("orphan-adopt");
    let mut c = controller();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "a".into() }))
        .unwrap();
    c.save(&mut store).unwrap();
    let bytes = fs::read(dir.join("snapshots").join("r1.json")).unwrap();
    // Simulate the crash window: snapshot published, HEAD removed.
    fs::remove_file(dir.join("HEAD")).unwrap();
    assert_eq!(store.head().unwrap(), None);

    // The journaled/committed restore re-saves the same bytes.
    store.save("p", ProjectRevision(1), &bytes).unwrap();
    assert_eq!(store.head().unwrap().as_deref(), Some("r1.json"));
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c2.live_revision(), ProjectRevision(1));
    assert_eq!(c2.live().tracks.len(), 1);
}

// E3 (Rule 2, 4): a snapshot publish failure leaves HEAD untouched
// and does not modify the conflicting destination bytes.
#[test]
fn failed_snapshot_publish_keeps_head_and_destination() {
    let (_g, dir, mut store) = bundle("snap-fail");
    let mut c = controller();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "a".into() }))
        .unwrap();
    c.save(&mut store).unwrap();

    // An external actor occupies the next snapshot name.
    fs::write(dir.join("snapshots").join("r2.json"), b"foreign").unwrap();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "b".into() }))
        .unwrap();
    let err = c.save(&mut store).unwrap_err();
    assert!(
        matches!(err, Error::Store(ref s) if s.contains("effect:")),
        "publish classification must propagate: {err}"
    );
    // HEAD keeps pointing at r1; the foreign file is byte-identical.
    assert_eq!(fs::read_to_string(dir.join("HEAD")).unwrap(), "r1.json");
    assert_eq!(
        fs::read(dir.join("snapshots").join("r2.json")).unwrap(),
        b"foreign"
    );
    assert_eq!(store.head().unwrap().as_deref(), Some("r1.json"));
    assert!(c.saved().is_some(), "failed save keeps prior SavedState");
}

// E4 (Rule 3, 4): when the HEAD publish fails, the published snapshot
// stays as an orphan and the previous HEAD content is preserved.
#[test]
fn failed_head_publish_keeps_snapshot_orphan_and_old_head() {
    let (_g, dir, mut store) = bundle("head-fail");
    let mut c = controller();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "a".into() }))
        .unwrap();
    c.save(&mut store).unwrap();
    assert_eq!(store.head().unwrap().as_deref(), Some("r1.json"));

    // Replace HEAD with a directory: the file-over-directory rename
    // definitively fails on unix and Windows.
    fs::remove_file(dir.join("HEAD")).unwrap();
    fs::create_dir(dir.join("HEAD")).unwrap();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "b".into() }))
        .unwrap();
    let err = c.save(&mut store).unwrap_err();
    assert!(
        matches!(err, Error::Store(ref s) if s.contains("effect:")),
        "publish classification must propagate: {err}"
    );

    // The snapshot was published (orphan); HEAD stayed the directory.
    assert!(dir.join("snapshots").join("r2.json").exists());
    assert!(dir.join("HEAD").is_dir());
    assert!(store.head().is_err(), "HEAD as directory cannot be read");

    // Recovery path: restoring a real HEAD file makes the orphan
    // snapshot the loadable truth.
    fs::remove_dir(dir.join("HEAD")).unwrap();
    fs::write(dir.join("HEAD"), "r2.json").unwrap();
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c2.live_revision(), ProjectRevision(2));
    assert_eq!(c2.live().tracks.len(), 2);
}

// E5 (Rule 5): an external HEAD rewrite is reported as-is; a dangling
// name surfaces through `read_snapshot`.
#[test]
fn external_head_rewrite_is_reported_honestly() {
    let (_g, dir, mut store) = bundle("ext-head");
    let mut c = controller();
    for name in ["a", "b"] {
        c.apply(cmd(&c, ProjectOp::AddTrack { name: name.into() }))
            .unwrap();
        c.save(&mut store).unwrap();
    }
    assert_eq!(store.head().unwrap().as_deref(), Some("r2.json"));

    // External rewind to r1 is what `head()` now reports.
    fs::write(dir.join("HEAD"), "r1.json").unwrap();
    assert_eq!(store.head().unwrap().as_deref(), Some("r1.json"));
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c2.live_revision(), ProjectRevision(1));

    // A dangling name propagates as a store error.
    fs::write(dir.join("HEAD"), "ghost.json").unwrap();
    assert_eq!(store.head().unwrap().as_deref(), Some("ghost.json"));
    assert!(store.read_snapshot("ghost.json").is_err());
    let mut c3 = controller();
    assert!(matches!(c3.load(&store), Err(Error::Store(_))));
}

// E6 (Rule 5): leftover `*.rvf-tmp-*` siblings and unrelated files do
// not disturb save/head/read — and the store does not delete them.
#[test]
fn leftover_temp_siblings_do_not_disturb_the_store() {
    let (_g, dir, mut store) = bundle("leftovers");
    let mut c = controller();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "a".into() }))
        .unwrap();
    c.save(&mut store).unwrap();

    let snap_tmp = dir
        .join("snapshots")
        .join("r9.json.rvf-tmp-0123456789abcdef");
    let head_tmp = dir.join("HEAD.rvf-tmp-fedcba9876543210");
    fs::write(&snap_tmp, b"partial").unwrap();
    fs::write(&head_tmp, b"partial-head").unwrap();
    fs::write(dir.join("snapshots").join("garbage.txt"), b"junk").unwrap();

    c.apply(cmd(&c, ProjectOp::AddTrack { name: "b".into() }))
        .unwrap();
    let saved = c.save(&mut store).unwrap();
    assert_eq!(saved.head, "r2.json");
    assert_eq!(store.head().unwrap().as_deref(), Some("r2.json"));
    assert_eq!(
        store.read_snapshot("r1.json").unwrap(),
        fs::read(dir.join("snapshots").join("r1.json")).unwrap()
    );
    // Leftovers are the caller's recovery data — never auto-deleted.
    assert!(snap_tmp.exists());
    assert!(head_tmp.exists());
}

// E7 (Rule 6): reopening the same root reproduces HEAD and snapshots;
// the loaded controller resumes at the saved revision.
#[test]
fn reopen_reproduces_head_and_snapshots() {
    let (_g, dir, mut store) = bundle("reopen");
    let mut c = controller();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "a".into() }))
        .unwrap();
    c.save(&mut store).unwrap();
    drop(store);

    let store = DirectoryBundleStore::create(&dir).unwrap();
    assert_eq!(store.head().unwrap().as_deref(), Some("r1.json"));
    let mut c2 = controller();
    c2.load(&store).unwrap();
    assert_eq!(c2.live_revision(), ProjectRevision(1));
    assert_eq!(c2.live(), c.live());
}

// E8 (Rule 5, 6): an empty bundle has no HEAD; controller load
// reports a store error rather than inventing state.
#[test]
fn empty_bundle_has_no_head() {
    let (_g, _dir, store) = bundle("empty");
    assert_eq!(store.head().unwrap(), None);
    let mut c = controller();
    assert!(matches!(c.load(&store), Err(Error::Store(_))));
    assert_eq!(c.live_revision(), ProjectRevision(0));
}

// E9 (Rule 7): `save` is byte-faithful — the bytes it was given are
// exactly what `read_snapshot` returns, and the HEAD file contains
// the bare snapshot name.
#[test]
fn save_round_trips_bytes_verbatim() {
    let (_g, dir, mut store) = bundle("bytes");
    let bytes = br#"{"schema_version":7,"payload":[1,2,3]}"#;
    let name = store.save("p", ProjectRevision(7), bytes).unwrap();
    assert_eq!(name, "r7.json");
    assert_eq!(store.read_snapshot(&name).unwrap(), bytes);
    assert_eq!(fs::read(dir.join("HEAD")).unwrap(), b"r7.json");
}

// Rule 4 (staging): a `StagedFile::create` failure before any bytes
// are written is surfaced as `Error::Store` and leaves no HEAD.
#[test]
fn failed_stage_create_leaves_no_head() {
    let (_g, dir, mut store) = bundle("stage-fail");
    // Occupy the snapshots path with a regular file — staging a temp
    // sibling inside it must fail at the create step.
    fs::remove_dir_all(dir.join("snapshots")).unwrap();
    fs::write(dir.join("snapshots"), b"not-a-dir").unwrap();
    let err = store.save("p", ProjectRevision(1), b"bytes").unwrap_err();
    assert!(
        matches!(err, Error::Store(ref s) if s.contains("stage create")),
        "stage failure must be labeled: {err}"
    );
    assert!(!dir.join("HEAD").exists());
    assert!(dir.join("snapshots").is_file());
}

// Rule 1 port coherence: `InMemoryStore` honors the same immutable
// name contract — byte-identical republish is idempotent, the
// watermark-only refresh stores the newer bytes, and different
// bytes under the same name are rejected with head and bytes kept.
#[test]
fn in_memory_store_honors_immutable_snapshot_names() {
    let mut store = InMemoryStore::new();
    store.save("p", ProjectRevision(1), b"bytes").unwrap();
    store.save("p", ProjectRevision(1), b"bytes").unwrap();
    assert_eq!(store.head().unwrap().as_deref(), Some("r1.json"));
    let err = store.save("p", ProjectRevision(1), b"other").unwrap_err();
    assert!(matches!(err, Error::Store(_)));
    assert_eq!(store.read_snapshot("r1.json").unwrap(), b"bytes");
    assert_eq!(store.head().unwrap().as_deref(), Some("r1.json"));

    // Refresh branch: same envelope, newer id_watermark → the stored
    // bytes advance (mirroring the file store's Replace republish).
    let envelope = |w: u128| SnapshotEnvelope {
        schema_version: SCHEMA_VERSION,
        project_id: ProjectId::new(0),
        revision: ProjectRevision(2),
        project: {
            let mut p = Project::new(ProjectId::new(0), "p");
            p.id_watermark = w;
            p
        },
    };
    let old = serde_json::to_vec(&envelope(4)).unwrap();
    let new = serde_json::to_vec(&envelope(6)).unwrap();
    store.save("p", ProjectRevision(2), &old).unwrap();
    store.save("p", ProjectRevision(2), &new).unwrap();
    assert_eq!(store.read_snapshot("r2.json").unwrap(), new);
    assert_eq!(store.head().unwrap().as_deref(), Some("r2.json"));
}

// Rule 1/concurrency: two store instances racing the same immutable
// snapshot name with different bytes — the atomic no-replace publish
// admits exactly one winner and the loser's error carries the
// DestinationExists classification; the winner's bytes survive.
#[test]
fn concurrent_same_name_publish_admits_one_winner() {
    let (_g, dir, mut store_a) = bundle("race");
    // A second handle on the same root — `create` is idempotent.
    let mut store_b = DirectoryBundleStore::create(&dir).unwrap();

    let (ra, rb) = std::thread::scope(|s| {
        let a = s.spawn(|| store_a.save("p", ProjectRevision(1), b"bytes-from-a"));
        let b = s.spawn(|| store_b.save("p", ProjectRevision(1), b"bytes-from-b"));
        (a.join().unwrap(), b.join().unwrap())
    });
    match (ra, rb) {
        (Ok(_), Err(Error::Store(_))) | (Err(Error::Store(_)), Ok(_)) => {}
        other => panic!("concurrent CreateNew must admit exactly one winner: {other:?}"),
    }
    // The surviving snapshot holds the winner's bytes; HEAD names it.
    let body = fs::read(dir.join("snapshots").join("r1.json")).unwrap();
    assert!(body == b"bytes-from-a" || body == b"bytes-from-b");
    assert_eq!(fs::read_to_string(dir.join("HEAD")).unwrap(), "r1.json");
}
