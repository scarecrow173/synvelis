//! SYN-IO-02 HEAD CAS contract tests (test-spec `syn-io-02-head-cas`):
//! `ProjectStore::save_cas` expected-head compare-and-swap on
//! `DirectoryBundleStore` (`HEAD.lock` create-new mutex + stale-lock
//! breaking) and `InMemoryStore` parity, `ProjectController::save`'s
//! expected-head supply, and the unsupported-store fallback.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use synvelis_project::store::{HEAD_LOCK_STALE_AFTER, HeadCasError};
use synvelis_project::*;

/// Unique temp bundle root; cleaned by the guard on drop.
fn workdir(tag: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "synvelis-head-cas-{tag}-{}-{:x}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
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

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

/// A `ProjectStore` with no `save_cas` override — the port default
/// must report `Unsupported` and `ProjectController::save` must keep
/// working through the unconditional fallback.
#[derive(Default)]
struct BareStore {
    snapshots: BTreeMap<String, Vec<u8>>,
    head: Option<String>,
}

impl ProjectStore for BareStore {
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
        Ok(self.head.clone())
    }

    fn read_snapshot(&self, name: &str) -> Result<Vec<u8>, Error> {
        self.snapshots
            .get(name)
            .cloned()
            .ok_or_else(|| Error::Store(format!("missing snapshot {name}")))
    }
}

// Rule 1: save_cas commits only while HEAD still names the expected
// snapshot — a matching expectation publishes snapshot + HEAD, a
// stale one is the typed Conflict and publishes nothing.
#[test]
fn save_cas_commits_only_when_head_matches() {
    let (_g, _dir, mut store) = bundle("cas-basic");

    // Empty store: asserting HEAD absent commits.
    let name = store
        .save_cas("p", None, ProjectRevision(1), b"one")
        .unwrap();
    assert_eq!(name, "r1.json");
    assert_eq!(store.head().unwrap().as_deref(), Some("r1.json"));

    // Matching expectation commits the next snapshot.
    let name = store
        .save_cas("p", Some("r1.json"), ProjectRevision(2), b"two")
        .unwrap();
    assert_eq!(name, "r2.json");
    assert_eq!(store.head().unwrap().as_deref(), Some("r2.json"));

    // Stale expectation: typed conflict carrying both sides; HEAD and
    // the would-be snapshot are untouched.
    let err = store
        .save_cas("p", Some("r1.json"), ProjectRevision(3), b"three")
        .unwrap_err();
    assert_eq!(
        err,
        HeadCasError::Conflict {
            expected: Some("r1.json".to_owned()),
            actual: Some("r2.json".to_owned()),
        }
    );
    assert_eq!(store.head().unwrap().as_deref(), Some("r2.json"));
    assert!(store.read_snapshot("r3.json").is_err());

    // Asserting absent against an existing HEAD conflicts too.
    let err = store
        .save_cas("p", None, ProjectRevision(4), b"four")
        .unwrap_err();
    assert_eq!(
        err,
        HeadCasError::Conflict {
            expected: None,
            actual: Some("r2.json".to_owned()),
        }
    );
    assert_eq!(store.head().unwrap().as_deref(), Some("r2.json"));
}

// Rule 2 (the lost-update reproduction): two store instances on the
// same root race `save_cas` with the same expected HEAD — exactly
// one commits; the loser gets the typed conflict and the winner's
// bytes are what HEAD/snapshot end up holding.
#[test]
fn concurrent_save_cas_admits_single_winner() {
    let (_g, dir, mut store_a) = bundle("cas-race");
    store_a
        .save_cas("p", None, ProjectRevision(1), b"seed")
        .unwrap();
    let mut store_b = DirectoryBundleStore::create(&dir).unwrap();

    let (ra, rb) = std::thread::scope(|s| {
        let a = s.spawn(|| store_a.save_cas("p", Some("r1.json"), ProjectRevision(2), b"from-a"));
        let b = s.spawn(|| store_b.save_cas("p", Some("r1.json"), ProjectRevision(2), b"from-b"));
        (a.join().unwrap(), b.join().unwrap())
    });
    let winner = match (ra, rb) {
        (Ok(name), Err(HeadCasError::Conflict { expected, actual }))
        | (Err(HeadCasError::Conflict { expected, actual }), Ok(name)) => {
            assert_eq!(name, "r2.json");
            assert_eq!(expected.as_deref(), Some("r1.json"));
            assert_eq!(actual.as_deref(), Some("r2.json"));
            name
        }
        other => panic!("concurrent save_cas must admit exactly one winner: {other:?}"),
    };
    assert_eq!(winner, "r2.json");
    let body = fs::read(dir.join("snapshots").join("r2.json")).unwrap();
    assert!(
        body == b"from-a" || body == b"from-b",
        "snapshot must hold a winner's bytes"
    );
    assert_eq!(fs::read_to_string(dir.join("HEAD")).unwrap(), "r2.json");
    assert!(
        !dir.join("HEAD.lock").exists(),
        "the lock is released when save_cas returns"
    );
}

// Rule 3: `ProjectController::save` supplies `saved.head` as the
// expected HEAD — a controller baselined on an older HEAD cannot
// repoint over a newer one (the multi-writer lost update `save`
// alone could not prevent), and its divergent snapshot is never
// even published.
#[test]
fn controller_save_cas_rejects_moved_head() {
    let (_g, dir, mut store_a) = bundle("cas-controller");
    let mut store_b = DirectoryBundleStore::create(&dir).unwrap();
    let mut c_a = controller();
    let mut c_b = controller();

    c_a.apply(cmd(&c_a, ProjectOp::AddTrack { name: "a".into() }))
        .unwrap();
    let saved = c_a.save(&mut store_a).unwrap();
    assert_eq!(saved.head, "r1.json");

    // c_b adopts the same HEAD, then falls behind.
    c_b.load(&store_b).unwrap();
    assert_eq!(c_b.live_revision(), ProjectRevision(1));
    for name in ["a2", "a3"] {
        c_a.apply(cmd(&c_a, ProjectOp::AddTrack { name: name.into() }))
            .unwrap();
        c_a.save(&mut store_a).unwrap();
    }
    assert_eq!(store_b.head().unwrap().as_deref(), Some("r3.json"));

    // c_b advances three revisions past its r1 baseline — without
    // CAS this would publish r4.json and repoint HEAD, discarding
    // c_a's committed line.
    for name in ["b2", "b3", "b4"] {
        c_b.apply(cmd(&c_b, ProjectOp::AddTrack { name: name.into() }))
            .unwrap();
    }
    let err = c_b.save(&mut store_b).unwrap_err();
    assert!(
        matches!(err, Error::Store(ref s) if s.contains("head CAS conflict")),
        "controller save must surface the typed conflict: {err}"
    );
    assert_eq!(c_b.saved().unwrap().head, "r1.json");
    assert_eq!(store_b.head().unwrap().as_deref(), Some("r3.json"));
    assert!(
        !dir.join("snapshots").join("r4.json").exists(),
        "a conflicted CAS publishes no snapshot"
    );

    // Recovery: reload adopts the winner's HEAD and the next save
    // chains from it.
    c_b.load(&store_b).unwrap();
    assert_eq!(c_b.saved().unwrap().head, "r3.json");
    c_b.apply(cmd(&c_b, ProjectOp::AddTrack { name: "b5".into() }))
        .unwrap();
    let saved = c_b.save(&mut store_b).unwrap();
    assert_eq!(saved.head, "r4.json");
    assert_eq!(store_b.head().unwrap().as_deref(), Some("r4.json"));
}

// Rule 4: `InMemoryStore` honors the same expected-head contract —
// the port's CAS semantics are reproducible without a filesystem.
#[test]
fn in_memory_store_save_cas_parity() {
    let mut store = InMemoryStore::new();
    store
        .save_cas("p", None, ProjectRevision(1), b"one")
        .unwrap();
    store
        .save_cas("p", Some("r1.json"), ProjectRevision(2), b"two")
        .unwrap();
    let err = store
        .save_cas("p", Some("r1.json"), ProjectRevision(3), b"three")
        .unwrap_err();
    assert_eq!(
        err,
        HeadCasError::Conflict {
            expected: Some("r1.json".to_owned()),
            actual: Some("r2.json".to_owned()),
        }
    );
    assert_eq!(store.head().unwrap().as_deref(), Some("r2.json"));
    assert!(store.read_snapshot("r3.json").is_err());

    // A fresh controller asserts HEAD-absent: saving onto a store it
    // never loaded conflicts rather than silently repointing.
    let mut c = controller();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "a".into() }))
        .unwrap();
    let err = c.save(&mut store).unwrap_err();
    assert!(
        matches!(err, Error::Store(ref s) if s.contains("head CAS conflict")),
        "fresh controller onto a non-empty store must conflict: {err}"
    );
}

// Rule 5: a store without a `save_cas` override reports the typed
// `Unsupported`, and `ProjectController::save` keeps the
// unconditional publish for it — backward compatibility for stores
// written before the CAS contract existed.
#[test]
fn unsupported_store_keeps_unconditional_save() {
    let mut bare = BareStore::default();
    let err = bare
        .save_cas("p", Some("r9.json"), ProjectRevision(1), b"b")
        .unwrap_err();
    assert_eq!(err, HeadCasError::Unsupported);

    let mut c = controller();
    c.apply(cmd(&c, ProjectOp::AddTrack { name: "a".into() }))
        .unwrap();
    let saved = c.save(&mut bare).unwrap();
    assert_eq!(saved.head, "r1.json");
    assert_eq!(bare.head().unwrap().as_deref(), Some("r1.json"));
}

// Rule 6: a `HEAD.lock` older than `HEAD_LOCK_STALE_AFTER` is
// treated as a crashed writer's leftover — the contender breaks it
// (rename-aside) and the CAS proceeds normally.
#[test]
fn stale_head_lock_is_broken() {
    let (_g, dir, mut store) = bundle("cas-stale");
    store.save("p", ProjectRevision(1), b"seed").unwrap();

    let stale_ns = unix_nanos() - HEAD_LOCK_STALE_AFTER.as_nanos() - 1_000_000_000;
    fs::write(
        dir.join("HEAD.lock"),
        format!("synvelis-head-lock/1\ncreated_ns={stale_ns}\ntoken=deadbeef\n"),
    )
    .unwrap();

    let name = store
        .save_cas("p", Some("r1.json"), ProjectRevision(2), b"two")
        .unwrap();
    assert_eq!(name, "r2.json");
    assert_eq!(store.head().unwrap().as_deref(), Some("r2.json"));
    assert!(
        !dir.join("HEAD.lock").exists(),
        "the acquired lock is released on return"
    );
    let leftovers: Vec<_> = fs::read_dir(&dir)
        .unwrap()
        .filter_map(std::result::Result::ok)
        .filter(|e| e.file_name().to_string_lossy().contains("HEAD.lock"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "no lock or stale-aside leftovers: {leftovers:?}"
    );
}

// Rule 7: a *fresh* foreign `HEAD.lock` is respected — `save_cas`
// waits inside the critical-section protocol instead of bypassing
// the holder, and proceeds once the lock is released.
#[test]
fn fresh_head_lock_blocks_save_until_released() {
    let (_g, dir, mut store_a) = bundle("cas-lockwait");
    store_a.save("p", ProjectRevision(1), b"seed").unwrap();
    let mut store_b = DirectoryBundleStore::create(&dir).unwrap();

    fs::write(
        dir.join("HEAD.lock"),
        format!(
            "synvelis-head-lock/1\ncreated_ns={}\ntoken=foreign\n",
            unix_nanos()
        ),
    )
    .unwrap();

    std::thread::scope(|s| {
        let handle = s.spawn(|| store_b.save_cas("p", Some("r1.json"), ProjectRevision(2), b"two"));
        std::thread::sleep(Duration::from_millis(300));
        assert!(
            !handle.is_finished(),
            "save_cas must wait on a fresh lock, not bypass it"
        );
        fs::remove_file(dir.join("HEAD.lock")).unwrap();
        let name = handle.join().unwrap().unwrap();
        assert_eq!(name, "r2.json");
    });
    assert_eq!(fs::read_to_string(dir.join("HEAD")).unwrap(), "r2.json");
}

// Rule 8: a failure inside the CAS critical section still releases
// the lock — the next save_cas is not wedged behind a leftover.
#[test]
fn cas_failure_releases_the_lock() {
    let (_g, dir, mut store) = bundle("cas-fail");
    store.save("p", ProjectRevision(1), b"seed").unwrap();

    // A foreign occupant makes the snapshot publish fail inside CAS.
    fs::write(dir.join("snapshots").join("r2.json"), b"foreign").unwrap();
    let err = store
        .save_cas("p", Some("r1.json"), ProjectRevision(2), b"two")
        .unwrap_err();
    assert!(
        matches!(err, HeadCasError::Store(_)),
        "publish failure surfaces as Store: {err}"
    );
    assert_eq!(store.head().unwrap().as_deref(), Some("r1.json"));
    assert!(
        !dir.join("HEAD.lock").exists(),
        "the lock is released even when the save fails"
    );

    // Clearing the obstruction lets the retried CAS commit.
    fs::remove_file(dir.join("snapshots").join("r2.json")).unwrap();
    let name = store
        .save_cas("p", Some("r1.json"), ProjectRevision(2), b"two")
        .unwrap();
    assert_eq!(name, "r2.json");
}
