//! SYN-IO-02 HEAD CAS contract tests (test-spec `syn-io-02-head-cas`
//! Rules 1–8, `syn-io-02-head-cas-e2e` Rules 1–4):
//! `ProjectStore::save_cas` expected-head compare-and-swap on
//! `DirectoryBundleStore` (`HEAD.lock` create-new mutex + stale-lock
//! breaking) and `InMemoryStore` parity, `ProjectController::save`'s
//! expected-head supply, the unsupported-store fallback, and the
//! real 2-process `save_cas` race via test-binary self re-exec.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Child, Output, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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

// ----- Real 2-process coordination (test-spec syn-io-02-head-cas-e2e Rules 1–2) -----
//
// The thread-level race above proves the contract inside one
// process; this test re-executes the same test binary as two real
// OS processes (the `import.rs` self re-exec pattern) so the
// `HEAD.lock` create-new mutex, token fencing, and stale-break are
// exercised across a process boundary — the remaining NotRun gate
// from syn-io-02-head-cas. The code is portable (no OS-specific
// API), so CI runs it on every OS.

/// Env guard marking a re-executed child as a CAS worker.
const E2E_GUARD: &str = "SYNVELIS_HEAD_CAS_E2E";
const E2E_DIR: &str = "SYNVELIS_HEAD_CAS_E2E_DIR";
const E2E_EXPECTED: &str = "SYNVELIS_HEAD_CAS_E2E_EXPECTED";
const E2E_REVISION: &str = "SYNVELIS_HEAD_CAS_E2E_REVISION";
const E2E_TAG: &str = "SYNVELIS_HEAD_CAS_E2E_TAG";
const E2E_GO: &str = "SYNVELIS_HEAD_CAS_E2E_GO";

/// Marker prefix the worker prints for the parent to parse.
const E2E_MARKER: &str = "HEAD_CAS_CHILD_RESULT ";

/// How long a child may wait for the go-file barrier before
/// failing, and how long the parent waits for a child to exit.
const E2E_TIMEOUT: Duration = Duration::from_secs(60);

/// Payload bytes a worker publishes — the parent recomputes the
/// winner's expected content from its tag.
fn e2e_payload(tag: &str) -> Vec<u8> {
    format!("synvelis-head-cas-e2e child={tag}").into_bytes()
}

/// Worker half of the 2-process test: waits for the go-file, then
/// performs exactly one `save_cas` and reports the typed outcome
/// as a marker line. Only reached when [`E2E_GUARD`] is set —
/// a plain `cargo test` run takes the parent path instead.
fn e2e_worker() {
    let dir = std::env::var(E2E_DIR).expect("E2E_DIR");
    let expected_raw = std::env::var(E2E_EXPECTED).expect("E2E_EXPECTED");
    let expected = if expected_raw.is_empty() {
        None
    } else {
        Some(expected_raw.as_str())
    };
    let revision = ProjectRevision(
        std::env::var(E2E_REVISION)
            .expect("E2E_REVISION")
            .parse()
            .expect("E2E_REVISION parses as u64"),
    );
    let tag = std::env::var(E2E_TAG).expect("E2E_TAG");
    let go = PathBuf::from(std::env::var(E2E_GO).expect("E2E_GO"));

    let deadline = Instant::now() + E2E_TIMEOUT;
    while !go.exists() {
        assert!(
            Instant::now() < deadline,
            "go-file barrier never appeared: {}",
            go.display()
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    let mut store = DirectoryBundleStore::create(&dir).expect("worker opens bundle");
    match store.save_cas("p", expected, revision, &e2e_payload(&tag)) {
        Ok(name) => println!("{E2E_MARKER}ok name={name}"),
        Err(HeadCasError::Conflict { expected, actual }) => println!(
            "{E2E_MARKER}conflict expected={} actual={}",
            expected.as_deref().unwrap_or("none"),
            actual.as_deref().unwrap_or("none")
        ),
        Err(e) => println!("{E2E_MARKER}error {e:?}"),
    }
}

/// Spawns one worker child: this test binary re-executed with the
/// env guard, filtered to exactly this test.
fn e2e_spawn(
    exe: &Path,
    test_name: &str,
    go: &Path,
    dir: &Path,
    expected: Option<&str>,
    revision: u64,
    tag: &str,
) -> Child {
    std::process::Command::new(exe)
        .arg(test_name)
        .arg("--exact")
        .arg("--nocapture")
        .env(E2E_GUARD, "1")
        .env(E2E_DIR, dir)
        .env(E2E_EXPECTED, expected.unwrap_or(""))
        .env(E2E_REVISION, revision.to_string())
        .env(E2E_TAG, tag)
        .env(E2E_GO, go)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn e2e worker")
}

/// Waits for a child with a bounded deadline — a wedged worker is
/// a test failure, never an unbounded hang.
fn e2e_wait(tag: &str, mut child: Child) -> Output {
    let deadline = Instant::now() + E2E_TIMEOUT;
    loop {
        if let Some(_status) = child.try_wait().expect("try_wait") {
            return child.wait_with_output().expect("wait_with_output");
        }
        assert!(
            Instant::now() < deadline,
            "e2e worker {tag} did not exit within {E2E_TIMEOUT:?}"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Typed outcome a worker reported through the marker protocol.
/// Anything else (`error`, missing marker, non-zero exit) already
/// panicked inside [`e2e_parse`].
#[derive(Debug)]
enum E2eOutcome {
    /// `save_cas` committed; carries the published snapshot name.
    Ok(String),
    /// `save_cas` conflicted; carries the observed expected/actual.
    Conflict {
        /// HEAD name asserted by the worker (`None` = absent).
        expected: Option<String>,
        /// HEAD name observed inside the critical section.
        actual: Option<String>,
    },
}

/// Parses the worker's marker line out of its captured stdout.
/// A missing marker or non-zero exit means the worker never
/// reached a typed outcome — reported with stderr context.
fn e2e_parse(tag: &str, out: &Output) -> E2eOutcome {
    assert!(
        out.status.success(),
        "e2e worker {tag} exited {:?}: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout
        .lines()
        .rev()
        .find(|l| l.starts_with(E2E_MARKER))
        .unwrap_or_else(|| {
            panic!(
                "e2e worker {tag} printed no {E2E_MARKER} marker; stdout={stdout} stderr={}",
                String::from_utf8_lossy(&out.stderr)
            )
        });
    let rest = line.trim_start_matches(E2E_MARKER);
    if let Some(name) = rest.strip_prefix("ok name=") {
        E2eOutcome::Ok(name.to_owned())
    } else if let Some(fields) = rest.strip_prefix("conflict ") {
        let mut expected = None;
        let mut actual = None;
        for field in fields.split(' ') {
            if let Some(v) = field.strip_prefix("expected=") {
                expected = (v != "none").then(|| v.to_owned());
            } else if let Some(v) = field.strip_prefix("actual=") {
                actual = (v != "none").then(|| v.to_owned());
            }
        }
        E2eOutcome::Conflict { expected, actual }
    } else {
        panic!("e2e worker {tag} reported a non-typed outcome: {rest}");
    }
}

/// Test-spec `syn-io-02-head-cas-e2e` Rules 1–2: two real OS
/// processes race `save_cas` on the same bundle root. Exactly
/// one commits; the loser gets the typed `Conflict` carrying the
/// observed HEAD — no lost update, no false success, no orphan
/// snapshot, no leftover lock. Repeated over several rounds so
/// the moved-head assert is exercised on a non-empty store too.
#[test]
fn two_process_save_cas_single_winner_typed_conflict() {
    if std::env::var(E2E_GUARD).is_ok() {
        e2e_worker();
        return;
    }

    let test_name = "two_process_save_cas_single_winner_typed_conflict";
    let (_g, dir, probe) = bundle("cas-2proc");
    let exe = std::env::current_exe().expect("test exe");
    let mut next_rev = 1_u64;

    for round in 0..3_u64 {
        let expected = probe.head().unwrap();
        let go = dir.with_file_name(format!(
            "{}.go-{round}",
            dir.file_name().unwrap().to_string_lossy()
        ));
        let rev_a = next_rev;
        let rev_b = next_rev + 1;
        next_rev += 2;

        let child_a = e2e_spawn(&exe, test_name, &go, &dir, expected.as_deref(), rev_a, "a");
        let child_b = e2e_spawn(&exe, test_name, &go, &dir, expected.as_deref(), rev_b, "b");
        fs::write(&go, b"go").expect("write go-file barrier");

        let out_a = e2e_wait("a", child_a);
        let out_b = e2e_wait("b", child_b);
        let _ = fs::remove_file(&go);

        let outcome_a = e2e_parse("a", &out_a);
        let outcome_b = e2e_parse("b", &out_b);

        // Exactly one Ok + one Conflict; any other combination —
        // two winners (lost update), two conflicts, an error, or
        // an Unsupported — is a contract violation.
        let (winner_tag, winner_rev) = match (outcome_a, outcome_b) {
            (
                E2eOutcome::Ok(name_a),
                E2eOutcome::Conflict {
                    expected: exp_b,
                    actual: act_b,
                },
            ) => {
                assert_eq!(name_a, snapshot_name(ProjectRevision(rev_a)));
                assert_eq!(exp_b.as_deref(), expected.as_deref());
                assert_eq!(act_b.as_deref(), Some(name_a.as_str()));
                ("a", rev_a)
            }
            (
                E2eOutcome::Conflict {
                    expected: exp_a,
                    actual: act_a,
                },
                E2eOutcome::Ok(name_b),
            ) => {
                assert_eq!(name_b, snapshot_name(ProjectRevision(rev_b)));
                assert_eq!(exp_a.as_deref(), expected.as_deref());
                assert_eq!(act_a.as_deref(), Some(name_b.as_str()));
                ("b", rev_b)
            }
            (a, b) => panic!(
                "round {round}: concurrent save_cas must admit exactly one winner: {a:?} / {b:?}"
            ),
        };
        let winner_name = snapshot_name(ProjectRevision(winner_rev));
        let loser_rev = if winner_tag == "a" { rev_b } else { rev_a };

        // Final state reflects only the winner's commit.
        assert_eq!(
            probe.head().unwrap().as_deref(),
            Some(winner_name.as_str()),
            "round {round}: HEAD must name the winner's snapshot"
        );
        assert_eq!(
            probe
                .read_snapshot(&winner_name)
                .expect("winner snapshot readable"),
            e2e_payload(winner_tag),
            "round {round}: snapshot must hold the winner's bytes"
        );
        let loser_name = snapshot_name(ProjectRevision(loser_rev));
        assert!(
            !dir.join("snapshots").join(&loser_name).exists(),
            "round {round}: the loser's snapshot must never be published ({loser_name})"
        );
        assert!(
            !dir.join("HEAD.lock").exists(),
            "round {round}: HEAD.lock must be released"
        );
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(std::result::Result::ok)
            .filter(|e| e.file_name().to_string_lossy().starts_with("HEAD.lock"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "round {round}: no HEAD.lock or stale-aside leftovers: {leftovers:?}"
        );
    }
}
