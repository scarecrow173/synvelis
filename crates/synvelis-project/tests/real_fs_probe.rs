//! SV84-REAL-* — synvelis#84 / SYN-IO-02 Windows real-FS lane.
//!
//! `DirectoryBundleStore` の Snapshot `CreateNew` publish → HEAD
//! CAS/`Replace` publish を、実 Windows ファイルシステム上で
//! 実 OS process kill（`Child::kill` = `TerminateProcess` 相当）
//! を挟んで検証する opt-in lane。全 test が `#[ignore]` で既定
//! `cargo test` / CI には含まれない（`mise run
//! test-project-real-fs`）。観測値は `CASE=SV84-REAL-*` 行として
//! stdout に出し、`evidence/reports/
//! syn-io-02-windows-real-fs.md` へ実測転記する。
//!
//! Case ID（Issue #84「まず Windows 実機」行の残件）:
//!
//! - `SV84-REAL-01` — kill-mid-publish → reopen → 旧 HEAD 保持
//!   or 完結した新 HEAD + orphan 保存の不変条件。
//! - `SV84-REAL-02` — crash 後の `save_cas` retry 回復
//!   （stale `HEAD.lock` の実 FS break を含む）。
//! - `SV84-REAL-03` — Failed（publish 失敗・CAS `Conflict`）で
//!   HEAD 誤進行なし + retry 回復。
//!
//! Indeterminate（`PublishEffect::Indeterminate`）は実 NTFS 上で
//! 非破壊的に誘発不能（ambiguous OS error が必要）のため
//! **`NotRun`** — 証拠レポートに明記する。process kill は OS の
//! dirty page を失わせない（power-loss ではない）— 真の
//! power-loss は引き続き `NotRun`。
//!
//! bundle を置く volume は `SYNVELIS_REALFS_ROOT` で固定できる
//! （既定は `std::env::temp_dir()`）。NTFS 実測を主張する lane
//! なので、実行者は対象 volume が NTFS であることを別途確認し、
//! 証拠に OS/FS/toolchain を記録すること。

use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use synvelis_project::store::{DirectoryBundleStore, HeadCasError};
use synvelis_project::{ProjectRevision, ProjectStore, snapshot_name};

// ----- child re-exec protocol (head_cas.rs / import.rs 先例) -----

/// Env guard marking a re-executed child as a probe worker.
const CHILD_ENV: &str = "SYNVELIS_REALFS_CHILD";
const ENV_DIR: &str = "SYNVELIS_REALFS_DIR";
const ENV_OP: &str = "SYNVELIS_REALFS_OP";
/// Empty string asserts HEAD absent (`save` ignores this).
const ENV_EXPECTED: &str = "SYNVELIS_REALFS_EXPECTED";
const ENV_REVISION: &str = "SYNVELIS_REALFS_REVISION";
const ENV_SEED: &str = "SYNVELIS_REALFS_SEED";
const ENV_LEN: &str = "SYNVELIS_REALFS_LEN";
/// Go-file barrier: the child waits for this path before calling
/// into the store, so the parent controls exactly when the publish
/// starts relative to its own observation loop.
const ENV_GO: &str = "SYNVELIS_REALFS_GO";

/// Marker prefixes the child prints for the parent/evidence.
const MARK_ENTERED: &str = "REALFS_CHILD_ENTERED";
const MARK_RESULT: &str = "REALFS_CHILD_RESULT";

/// How long the child may wait for the go-file and how long the
/// parent waits for a trigger path before failing the round.
const CHILD_GO_TIMEOUT: Duration = Duration::from_secs(60);
const TRIGGER_TIMEOUT: Duration = Duration::from_secs(30);
/// Bound on waiting for a killed child to be reaped — a wedged
/// child fails the round instead of hanging the lane. Generously
/// above the store's own `HEAD_LOCK_BUSY_TIMEOUT` (30 s) so a
/// missed kill still resolves through the child's own error paths.
const CHILD_WAIT_TIMEOUT: Duration = Duration::from_secs(90);
/// Attempts per kill round when the child manages to finish before
/// the kill lands (a clean exit is a missed crash window, not a
/// contract result).
const KILL_ATTEMPTS: u32 = 3;

/// Payload size for the crashed publish — large enough that the
/// staged write/sync window is far wider than the parent's poll
/// loop, small enough to keep the opt-in lane quick.
const PAYLOAD_LEN: usize = 16 * 1024 * 1024;

/// Seed snapshot payload — small; only its name/head matter.
const SEED_LEN: usize = 4096;

/// Everything a worker child needs, passed through env vars so the
/// parent and child compute identical payloads and paths.
struct ChildSpec<'a> {
    /// This test binary re-executed with the env guard.
    exe: &'a Path,
    /// Test name filter (`--exact`) selecting the child entry point.
    test_name: &'a str,
    /// Bundle root the child opens.
    dir: &'a Path,
    /// `cas` (`save_cas`) or `save` (unconditional publish).
    op: &'a str,
    /// Expected HEAD name for `save_cas` (`None` = absent).
    expected: Option<&'a str>,
    /// Snapshot revision the child publishes.
    revision: u64,
    /// Payload seed — `probe_payload(seed, len)` is deterministic.
    seed: u64,
    /// Payload length in bytes.
    len: usize,
    /// Go-file barrier path.
    go: &'a Path,
}

fn unix_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

/// Unique temp bundle root; cleaned by the guard on drop. The base
/// volume is `SYNVELIS_REALFS_ROOT` when set (pin the FS under
/// test), else the platform temp dir.
fn workdir(tag: &str) -> PathBuf {
    let base =
        std::env::var_os("SYNVELIS_REALFS_ROOT").map_or_else(std::env::temp_dir, PathBuf::from);
    base.join(format!(
        "synvelis-realfs-{tag}-{}-{:x}",
        std::process::id(),
        unix_nanos()
    ))
}

struct BundleGuard(PathBuf);

impl Drop for BundleGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Deterministic payload both processes compute identically: a 4 KiB
/// xorshift64*-derived table tiled to `len`. Byte-identical retry is
/// what lets a post-crash `save_cas` adopt an orphan snapshot via
/// the `Satisfied` collision path.
fn probe_payload(seed: u64, len: usize) -> Vec<u8> {
    let mut table = [0u8; 4096];
    let mut state = seed ^ 0x9E37_79B9_7F4A_7C15;
    debug_assert_ne!(state, 0);
    for byte in &mut table {
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        let top = state.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 56;
        *byte = u8::try_from(top).expect("xorshift top byte fits u8");
    }
    let mut out = Vec::with_capacity(len);
    while out.len() < len {
        let n = (len - out.len()).min(table.len());
        out.extend_from_slice(&table[..n]);
    }
    out
}

/// Child half: waits for the go-file barrier, then performs exactly
/// one store call and reports the typed outcome. Only reached when
/// [`CHILD_ENV`] is set — a plain `cargo test` run takes the parent
/// path instead.
fn child_main() {
    let dir = PathBuf::from(std::env::var(ENV_DIR).expect("REALFS_DIR"));
    let op = std::env::var(ENV_OP).expect("REALFS_OP");
    let expected_raw = std::env::var(ENV_EXPECTED).expect("REALFS_EXPECTED");
    let expected = (!expected_raw.is_empty()).then_some(expected_raw.as_str());
    let revision = ProjectRevision(
        std::env::var(ENV_REVISION)
            .expect("REALFS_REVISION")
            .parse()
            .expect("REALFS_REVISION parses as u64"),
    );
    let seed = std::env::var(ENV_SEED)
        .expect("REALFS_SEED")
        .parse()
        .expect("REALFS_SEED parses as u64");
    let len = std::env::var(ENV_LEN)
        .expect("REALFS_LEN")
        .parse()
        .expect("REALFS_LEN parses as usize");
    let go = PathBuf::from(std::env::var(ENV_GO).expect("REALFS_GO"));
    let payload = probe_payload(seed, len);

    let deadline = Instant::now() + CHILD_GO_TIMEOUT;
    while !go.exists() {
        assert!(
            Instant::now() < deadline,
            "go-file barrier never appeared: {}",
            go.display()
        );
        std::thread::sleep(Duration::from_millis(5));
    }

    println!("{MARK_ENTERED} op={op} rev={}", revision.0);
    let _ = io::stdout().flush();

    let mut store = DirectoryBundleStore::create(&dir).expect("child opens bundle");
    match op.as_str() {
        "cas" => match store.save_cas("p", expected, revision, &payload) {
            Ok(name) => println!("{MARK_RESULT} ok name={name}"),
            Err(HeadCasError::Conflict { expected, actual }) => println!(
                "{MARK_RESULT} conflict expected={} actual={}",
                expected.as_deref().unwrap_or("none"),
                actual.as_deref().unwrap_or("none")
            ),
            Err(e) => println!("{MARK_RESULT} error {e:?}"),
        },
        "save" => match store.save("p", revision, &payload) {
            Ok(name) => println!("{MARK_RESULT} ok name={name}"),
            Err(e) => println!("{MARK_RESULT} error {e:?}"),
        },
        other => panic!("unknown REALFS_OP {other}"),
    }
    let _ = io::stdout().flush();
}

/// Which on-disk event the parent waits for before killing the
/// child — each lands the process kill in a different publish
/// window of the same save.
#[derive(Clone, Copy, Debug)]
enum Trigger {
    /// `HEAD.lock` exists: inside the `save_cas` critical section,
    /// before the snapshot publish has had time to finish.
    HeadLock,
    /// `snapshots/r<N>.json.rvf-tmp-*` exists: mid snapshot staging.
    SnapshotTemp,
    /// `snapshots/r<N>.json` exists: snapshot rename applied,
    /// HEAD repoint still pending or in flight.
    SnapshotDone,
}

impl Trigger {
    fn label(self) -> &'static str {
        match self {
            Self::HeadLock => "head-lock",
            Self::SnapshotTemp => "snapshot-temp",
            Self::SnapshotDone => "snapshot-done",
        }
    }
}

/// Spawns one worker child: this test binary re-executed with the
/// env guard, filtered to exactly `spec.test_name`. `--ignored` is
/// required — every probe test is `#[ignore]`d so the default
/// `cargo test` / CI run skips it, and the child must opt back in.
fn spawn_child(spec: &ChildSpec<'_>) -> Child {
    Command::new(spec.exe)
        .arg(spec.test_name)
        .arg("--exact")
        .arg("--nocapture")
        .arg("--ignored")
        .env(CHILD_ENV, "1")
        .env(ENV_DIR, spec.dir)
        .env(ENV_OP, spec.op)
        .env(ENV_EXPECTED, spec.expected.unwrap_or(""))
        .env(ENV_REVISION, spec.revision.to_string())
        .env(ENV_SEED, spec.seed.to_string())
        .env(ENV_LEN, spec.len.to_string())
        .env(ENV_GO, spec.go)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn realfs child")
}

/// Whether any direct child of `dir` has a file name starting with
/// `prefix` — used for `*.rvf-tmp-*` temp-residue observation.
fn any_child_named(dir: &Path, prefix: &str) -> bool {
    fs::read_dir(dir).is_ok_and(|entries| {
        entries
            .filter_map(std::result::Result::ok)
            .any(|e| e.file_name().to_string_lossy().starts_with(prefix))
    })
}

/// The trigger predicate for `dir`/`snap_name`.
fn trigger_hit(dir: &Path, snap_name: &str, trigger: Trigger) -> bool {
    match trigger {
        Trigger::HeadLock => dir.join("HEAD.lock").exists(),
        Trigger::SnapshotTemp => {
            any_child_named(&dir.join("snapshots"), &format!("{snap_name}.rvf-tmp-"))
        }
        Trigger::SnapshotDone => dir.join("snapshots").join(snap_name).exists(),
    }
}

/// Spin-polls the trigger predicate until `deadline`. No `sleep` —
/// Windows timer granularity (~15 ms) is coarser than the publish
/// windows this lane measures, so the poll burns a few hundred ms of
/// one core instead of sleeping through the window.
fn wait_for_trigger(dir: &Path, snap_name: &str, trigger: Trigger, deadline: Instant) -> bool {
    let mut spins = 0u32;
    loop {
        if trigger_hit(dir, snap_name, trigger) {
            return true;
        }
        spins += 1;
        if spins.is_multiple_of(256) && Instant::now() >= deadline {
            return false;
        }
        std::hint::spin_loop();
    }
}

/// Waits for a child with a bounded deadline — a wedged worker is
/// a test failure, never an unbounded hang (`head_cas.rs`'s
/// `e2e_wait` precedent).
fn wait_bounded(mut child: Child, timeout: Duration) -> Output {
    let deadline = Instant::now() + timeout;
    loop {
        if child.try_wait().expect("try_wait").is_some() {
            return child.wait_with_output().expect("wait_with_output");
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("realfs child did not exit within {timeout:?}");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// One kill round on a fresh seeded bundle: spawn → go barrier →
/// wait for `trigger` → `Child::kill` → assert the process died
/// mid-flight. Each attempt gets its own bundle so a missed window
/// (child commits before the kill lands) never contaminates the
/// retry — a clean exit is retried, never counted as a pass.
/// Returns the live bundle guard, its dir, and the child's output.
struct KillRound<'a> {
    /// This test binary re-executed with the env guard.
    exe: &'a Path,
    /// Test name filter (`--exact`) selecting the child entry point.
    test_name: &'a str,
    /// `cas` (`save_cas`) or `save` (unconditional publish).
    op: &'a str,
    /// On-disk event the parent waits for before killing.
    trigger: Trigger,
    /// Revision of the seeded HEAD the child publishes over.
    seed_rev: u64,
    /// Snapshot revision the child publishes.
    revision: u64,
    /// Seed for the seed snapshot's payload (`SEED_LEN` bytes).
    bundle_seed: u64,
    /// Payload seed — `probe_payload(seed, len)` is deterministic.
    payload_seed: u64,
    /// Payload length in bytes.
    len: usize,
    /// `workdir` tag base; the attempt index is appended.
    tag: &'a str,
}

fn kill_round(spec: &KillRound<'_>) -> (BundleGuard, PathBuf, Output) {
    let snap_name = snapshot_name(ProjectRevision(spec.revision));
    for attempt in 1..=KILL_ATTEMPTS {
        let dir = workdir(&format!("{}-{attempt}", spec.tag));
        let guard = BundleGuard(dir.clone());
        let mut store = DirectoryBundleStore::create(&dir).expect("seed bundle");
        store
            .save(
                "p",
                ProjectRevision(spec.seed_rev),
                &probe_payload(spec.bundle_seed, SEED_LEN),
            )
            .expect("seed save");
        drop(store);
        let expected_name = snapshot_name(ProjectRevision(spec.seed_rev));
        let expected = (spec.op == "cas").then_some(expected_name.as_str());
        let go = dir.with_file_name(format!(
            "{}.go-{}-{attempt}",
            dir.file_name().unwrap().to_string_lossy(),
            spec.op
        ));
        let child_spec = ChildSpec {
            exe: spec.exe,
            test_name: spec.test_name,
            dir: &dir,
            op: spec.op,
            expected,
            revision: spec.revision,
            seed: spec.payload_seed,
            len: spec.len,
            go: &go,
        };
        let mut child = spawn_child(&child_spec);
        fs::write(&go, b"go").expect("write go-file barrier");
        let deadline = Instant::now() + TRIGGER_TIMEOUT;
        assert!(
            wait_for_trigger(&dir, &snap_name, spec.trigger, deadline),
            "trigger {} never observed for {snap_name}",
            spec.trigger.label()
        );
        // `kill` may race the child's own exit — either way the wait
        // below classifies the outcome; only a clean exit is a miss.
        let _ = child.kill();
        let out = wait_bounded(child, CHILD_WAIT_TIMEOUT);
        let _ = fs::remove_file(&go);
        if !out.status.success() {
            return (guard, dir, out);
        }
        eprintln!(
            "kill round attempt {attempt}: child exited before kill landed; retrying on a fresh bundle"
        );
    }
    panic!("child finished cleanly on all {KILL_ATTEMPTS} kill attempts");
}

/// The post-crash on-disk state, observed through a reopened store
/// plus raw FS reads.
struct CrashState {
    /// `store.head()` result.
    head: Option<String>,
    /// Whether `snapshots/<snap>` exists and, if so, its length.
    snap_exists: bool,
    /// `snapshots/<snap>` byte length when present.
    snap_len: Option<u64>,
    /// Raw `HEAD` file content.
    head_raw: Option<String>,
    /// Whether `HEAD.lock` remains (dead holder's leftover).
    lock_leftover: bool,
    /// `HEAD.lock.stale-*` asides present (must stay zero until a
    /// contender breaks one).
    stale_asides: usize,
    /// `*.rvf-tmp-*` residue count across root + snapshots.
    temp_residue: usize,
}

fn observe_crash_state(dir: &Path, snap_name: &str) -> CrashState {
    let store = DirectoryBundleStore::create(dir).expect("reopen bundle");
    let head = store.head().expect("head() after crash");
    let snap_path = dir.join("snapshots").join(snap_name);
    let snap_len = fs::metadata(&snap_path).ok().map(|m| m.len());
    let head_raw = fs::read_to_string(dir.join("HEAD")).ok();
    let mut stale_asides = 0usize;
    let mut temp_residue = 0usize;
    for sub in [dir.to_path_buf(), dir.join("snapshots")] {
        if let Ok(entries) = fs::read_dir(&sub) {
            for e in entries.filter_map(std::result::Result::ok) {
                let name = e.file_name().to_string_lossy().into_owned();
                if name.starts_with("HEAD.lock.stale-") {
                    stale_asides += 1;
                }
                if name.contains(".rvf-tmp-") {
                    temp_residue += 1;
                }
            }
        }
    }
    CrashState {
        head,
        snap_exists: snap_len.is_some(),
        snap_len,
        head_raw,
        lock_leftover: dir.join("HEAD.lock").exists(),
        stale_asides,
        temp_residue,
    }
}

fn print_crash_state(case: &str, round: &str, op: &str, trigger: Trigger, state: &CrashState) {
    println!(
        "CASE={case} round={round} op={op} trigger={} head={} snap_exists={} snap_len={} lock_leftover={} stale_asides={} temp_residue={} head_raw={}",
        trigger.label(),
        state.head.as_deref().unwrap_or("none"),
        state.snap_exists,
        state
            .snap_len
            .map_or_else(|| "none".to_owned(), |l| l.to_string()),
        state.lock_leftover,
        state.stale_asides,
        state.temp_residue,
        state.head_raw.as_deref().unwrap_or("none"),
    );
}

/// The SV84-REAL-01 invariant: after a real process kill during a
/// publish, reopening the bundle must show either the old HEAD or a
/// completed new HEAD — never a torn HEAD, never HEAD pointing at a
/// partial snapshot, and any orphan snapshot/temp residue preserved
/// (the store never auto-deletes recovery data).
fn assert_crash_invariant(
    dir: &Path,
    seed_rev: u64,
    crash_rev: u64,
    payload: &[u8],
    op: &str,
) -> CrashState {
    let crash_name = snapshot_name(ProjectRevision(crash_rev));
    let state = observe_crash_state(dir, &crash_name);
    let seed_name = snapshot_name(ProjectRevision(seed_rev));

    // HEAD names exactly one of the two published snapshots — the
    // atomic Replace publish cannot leave a torn or foreign name.
    assert!(
        state.head.as_deref() == Some(seed_name.as_str())
            || state.head.as_deref() == Some(crash_name.as_str()),
        "HEAD must name the seed or crashed snapshot, got {:?}",
        state.head
    );
    // Raw HEAD bytes agree with the store read (no torn content).
    assert_eq!(
        state.head_raw.as_deref().map(str::trim),
        state.head.as_deref(),
        "raw HEAD content must match head()"
    );

    let store = DirectoryBundleStore::create(dir).unwrap();
    if state.head.as_deref() == Some(crash_name.as_str()) {
        // Completed commit: the snapshot HEAD names must be whole.
        assert_eq!(
            store.read_snapshot(&crash_name).unwrap(),
            payload,
            "HEAD-committed snapshot must hold the crashed writer's bytes"
        );
    } else {
        // Old HEAD kept: the crashed snapshot may exist only as a
        // complete orphan (atomic rename) — never partial bytes.
        let orphan = dir.join("snapshots").join(&crash_name);
        if orphan.exists() {
            assert_eq!(
                fs::read(&orphan).unwrap(),
                payload,
                "orphan snapshot must be byte-complete (atomic rename)"
            );
        }
    }

    if op == "save" {
        assert!(
            !dir.join("HEAD.lock").exists(),
            "unconditional save never takes HEAD.lock"
        );
    }
    assert_eq!(
        state.stale_asides, 0,
        "no contender ran yet — no stale-aside break may exist"
    );
    state
}

/// Creates a bundle seeded at `seed_rev` via the unconditional
/// `save` path and returns the guard/dir/store.
fn seeded_bundle(
    tag: &str,
    seed_rev: u64,
    seed: u64,
) -> (BundleGuard, PathBuf, DirectoryBundleStore) {
    let dir = workdir(tag);
    let mut store = DirectoryBundleStore::create(&dir).unwrap();
    store
        .save(
            "p",
            ProjectRevision(seed_rev),
            &probe_payload(seed, SEED_LEN),
        )
        .unwrap();
    let seed_name = snapshot_name(ProjectRevision(seed_rev));
    assert_eq!(store.head().unwrap().as_deref(), Some(seed_name.as_str()));
    (BundleGuard(dir.clone()), dir, store)
}

/// Prints the child's protocol markers plus output tails for
/// evidence — the libtest summary line would otherwise hide the
/// `REALFS_CHILD_*` result inside `stdout`.
fn print_child_out(case: &str, stage: &str, out: &Output) {
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    let markers: Vec<&str> = stdout
        .lines()
        .filter(|line| line.contains("REALFS_CHILD"))
        .collect();
    println!(
        "CASE={case} stage={stage} status={:?} markers={markers:?} stderr_tail={:?}",
        out.status.code(),
        stderr.lines().last().unwrap_or(""),
    );
}

// ----- SV84-REAL-01: kill-mid-publish → reopen invariant -----

/// Case `SV84-REAL-01`: a real OS process is killed while inside
/// `save`/`save_cas` on a real FS. Reopening must yield the old HEAD
/// or a completed new HEAD (+ preserved orphan) — across five
/// trigger windows covering the lock, snapshot staging, snapshot
/// rename, and HEAD repoint.
#[test]
#[ignore = "real-FS lane: requires a real local filesystem (run `mise run test-project-real-fs`)"]
fn sv84_real_01_kill_mid_publish_reopen_invariant() {
    if std::env::var(CHILD_ENV).is_ok() {
        child_main();
        return;
    }

    let test_name = "sv84_real_01_kill_mid_publish_reopen_invariant";
    let exe = std::env::current_exe().expect("test exe");
    let crash_seed = 0x5EED_0001u64;
    let payload = probe_payload(crash_seed, PAYLOAD_LEN);

    // (op, trigger, round label) — each on a fresh bundle.
    let rounds: [(&str, Trigger, &str); 5] = [
        ("cas", Trigger::HeadLock, "cas-lock"),
        ("cas", Trigger::SnapshotTemp, "cas-snaptemp"),
        ("cas", Trigger::SnapshotDone, "cas-snapdone"),
        ("save", Trigger::SnapshotTemp, "save-snaptemp"),
        ("save", Trigger::SnapshotDone, "save-snapdone"),
    ];

    for (i, (op, trigger, label)) in rounds.iter().enumerate() {
        let (_guard, dir, out) = kill_round(&KillRound {
            exe: &exe,
            test_name,
            op,
            trigger: *trigger,
            seed_rev: 1,
            revision: 2,
            bundle_seed: 0x5EED,
            payload_seed: crash_seed,
            len: PAYLOAD_LEN,
            tag: &format!("real01-{i}"),
        });
        print_child_out("SV84-REAL-01", label, &out);
        let state = assert_crash_invariant(&dir, 1, 2, &payload, op);
        print_crash_state("SV84-REAL-01", label, op, *trigger, &state);
    }
}

// ----- SV84-REAL-02: post-crash save_cas retry recovery -----

/// Case `SV84-REAL-02`: after a real process kill mid-`save_cas`,
/// a retry `save_cas` from a fresh store instance reconciles to a
/// consistent HEAD — adopting a same-bytes orphan via the
/// `Satisfied` collision path or chaining past an already-committed
/// HEAD. A leftover `HEAD.lock` is broken through the real stale
/// path (`HEAD_LOCK_STALE_AFTER`), not removed by hand.
#[test]
#[ignore = "real-FS lane: requires a real local filesystem (run `mise run test-project-real-fs`)"]
fn sv84_real_02_save_cas_retry_recovers_after_process_kill() {
    if std::env::var(CHILD_ENV).is_ok() {
        child_main();
        return;
    }

    let test_name = "sv84_real_02_save_cas_retry_recovers_after_process_kill";
    let exe = std::env::current_exe().expect("test exe");
    let crash_seed = 0x5EED_0002u64;
    let payload = probe_payload(crash_seed, PAYLOAD_LEN);
    let retry_seed = 0x5EED_0003u64;
    let retry_payload = probe_payload(retry_seed, PAYLOAD_LEN);

    let (_guard, dir, out) = kill_round(&KillRound {
        exe: &exe,
        test_name,
        op: "cas",
        trigger: Trigger::SnapshotDone,
        seed_rev: 1,
        revision: 2,
        bundle_seed: 0x5EED,
        payload_seed: crash_seed,
        len: PAYLOAD_LEN,
        tag: "real02",
    });
    print_child_out("SV84-REAL-02", "kill", &out);

    // Reopen and branch on the honest observed HEAD.
    let mut store = DirectoryBundleStore::create(&dir).unwrap();
    let head = store.head().unwrap();
    println!(
        "CASE=SV84-REAL-02 observed_head={} lock_leftover_before_retry={}",
        head.as_deref().unwrap_or("none"),
        dir.join("HEAD.lock").exists()
    );

    match head.as_deref() {
        Some("r1.json") => {
            // Old HEAD kept: retry the identical CAS — a surviving
            // r2.json orphan satisfies the immutable-name collision
            // and HEAD is repointed onto it (crash-window recovery).
            let name = store
                .save_cas("p", Some("r1.json"), ProjectRevision(2), &payload)
                .expect("post-crash retry save_cas must commit");
            assert_eq!(name, "r2.json");
            assert_eq!(store.head().unwrap().as_deref(), Some("r2.json"));
            assert_eq!(
                store.read_snapshot("r2.json").unwrap(),
                payload,
                "recovered snapshot must hold the crashed writer's bytes"
            );
        }
        Some("r2.json") => {
            // The kill landed after the HEAD repoint: the commit is
            // already whole — verify it, then chain the next CAS.
            assert_eq!(store.read_snapshot("r2.json").unwrap(), payload);
            let name = store
                .save_cas("p", Some("r2.json"), ProjectRevision(3), &retry_payload)
                .expect("post-commit save_cas must commit");
            assert_eq!(name, "r3.json");
            assert_eq!(store.head().unwrap().as_deref(), Some("r3.json"));
            assert_eq!(store.read_snapshot("r3.json").unwrap(), retry_payload);
            // The committed r2 snapshot is untouched.
            assert_eq!(store.read_snapshot("r2.json").unwrap(), payload);
        }
        other => panic!("unexpected HEAD after crash: {other:?}"),
    }

    // Recovery leaves no lock or stale-aside residue.
    assert!(
        !dir.join("HEAD.lock").exists(),
        "retry must release HEAD.lock"
    );
    let mut leftovers = Vec::new();
    for e in fs::read_dir(&dir)
        .unwrap()
        .filter_map(std::result::Result::ok)
    {
        let name = e.file_name().to_string_lossy().into_owned();
        if name.starts_with("HEAD.lock") {
            leftovers.push(name);
        }
    }
    assert!(
        leftovers.is_empty(),
        "no HEAD.lock or stale-aside leftovers after recovery: {leftovers:?}"
    );
    println!(
        "CASE=SV84-REAL-02 recovered head={} lock_cleared=true",
        store.head().unwrap().as_deref().unwrap_or("none")
    );
}

// ----- SV84-REAL-03: Failed/Conflict never advances HEAD -----

/// Case `SV84-REAL-03`: a real publish failure (foreign occupant at
/// the immutable snapshot name) and a stale-expected CAS `Conflict`
/// must neither advance HEAD nor publish a divergent snapshot —
/// and a cleaned-up retry commits normally. `Indeterminate` cannot
/// be induced non-destructively on real NTFS (it needs an ambiguous
/// OS error) and stays `NotRun` in the evidence report.
#[test]
#[ignore = "real-FS lane: requires a real local filesystem (run `mise run test-project-real-fs`)"]
fn sv84_real_03_failed_publish_and_conflict_never_advance_head() {
    let (_g, dir, mut store) = seeded_bundle("real03", 1, 0x5EED);
    let payload = probe_payload(0x5EED_0004, PAYLOAD_LEN);
    let foreign = b"foreign-occupant-not-a-snapshot".to_vec();

    // Foreign occupant at the immutable name: the snapshot publish
    // fails for real on the FS (CreateNew collision → conflict
    // classification → Store error), inside the CAS critical
    // section.
    fs::write(dir.join("snapshots").join("r2.json"), &foreign).unwrap();
    let err = store
        .save_cas("p", Some("r1.json"), ProjectRevision(2), &payload)
        .expect_err("foreign occupant must fail the publish");
    assert!(
        matches!(err, HeadCasError::Store(_)),
        "expected Store failure, got {err:?}"
    );
    println!("CASE=SV84-REAL-03 foreign_publish_error={err:?}");

    // HEAD did not advance; the lock was released; the foreign
    // bytes were not overwritten (immutable-name conflict).
    assert_eq!(store.head().unwrap().as_deref(), Some("r1.json"));
    assert!(
        !dir.join("HEAD.lock").exists(),
        "failed CAS releases HEAD.lock"
    );
    assert_eq!(
        fs::read(dir.join("snapshots").join("r2.json")).unwrap(),
        foreign,
        "conflicted snapshot keeps the existing bytes"
    );

    // Stale-expected CAS: typed Conflict before any publish — no
    // r3.json orphan is generated and HEAD stays put.
    let err = store
        .save_cas("p", Some("r9.json"), ProjectRevision(3), &payload)
        .expect_err("stale expected must conflict");
    assert_eq!(
        err,
        HeadCasError::Conflict {
            expected: Some("r9.json".to_owned()),
            actual: Some("r1.json".to_owned()),
        }
    );
    assert!(!dir.join("snapshots").join("r3.json").exists());
    assert_eq!(store.head().unwrap().as_deref(), Some("r1.json"));
    println!(
        "CASE=SV84-REAL-03 stale_conflict expected=r9.json actual=r1.json head_unchanged=true"
    );

    // Recovery: clear the obstruction, retry the identical CAS —
    // it commits and HEAD advances exactly once.
    fs::remove_file(dir.join("snapshots").join("r2.json")).unwrap();
    let name = store
        .save_cas("p", Some("r1.json"), ProjectRevision(2), &payload)
        .expect("retry after failure must commit");
    assert_eq!(name, "r2.json");
    assert_eq!(store.head().unwrap().as_deref(), Some("r2.json"));
    assert_eq!(store.read_snapshot("r2.json").unwrap(), payload);
    println!("CASE=SV84-REAL-03 retry_committed head=r2.json");
}
