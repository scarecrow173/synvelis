//! Deterministic component-swap containment regressions (N1, Refs
//! Synvelis #85).
//!
//! Every `open`/`stat`/`read_dir` path in the baseline providers performs
//! `canonicalize` + containment check at T0 and then re-traverses the
//! same path at T1. Between T0 and T1 a leaf or intermediate
//! component can be swapped (rename + symlink plant), so the second
//! traversal resolves to an object outside the trusted root.
//!
//! The `pre_traverse` fault barrier fires *inside* that T0→T1 window
//! and performs the swap synchronously — no threads, no timing: the
//! race is injected deterministically.
//!
//! Contract under test: an object that resolves outside the trusted
//! root must never have its bytes, facts, or directory entries
//! served — the operation must fail. These tests currently FAIL on
//! the path-retraversal implementation (RED evidence) and must pass
//! once the open goes through a handle-relative rooted-open
//! capability (Ruviform-owned; pending). Until then they stay
//! `#[ignore]`d with the reason recorded — they are not deleted,
//! weakened, or silently tolerated.
//!
//! Run the evidence explicitly:
//! `cargo test -p synvelis-library --test containment_swap -- --ignored`

mod common;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use common::{TempTree, aref, browse_req, cx, loc, pid, try_symlink};

use synvelis_library::*;

/// Parks `victim` aside and plants a symlink to `outside` at its
/// name. Returns `false` when symlink creation is denied (callers
/// skip honestly — the swap is never half-done).
fn swap_in_link(victim: &Path, outside: &Path) -> bool {
    let parked = victim.with_file_name(format!(
        "{}.parked",
        victim.file_name().unwrap().to_string_lossy()
    ));
    fs::rename(victim, &parked).expect("park victim");
    if try_symlink(outside, victim) {
        true
    } else {
        let _ = fs::rename(&parked, victim);
        false
    }
}

/// A one-shot barrier: swaps `victim` for a link to `outside` the
/// first time a `want_op` traversal fires.
fn swap_once(
    victim: PathBuf,
    outside: PathBuf,
    want_op: &'static str,
) -> (PreTraverseHook, Arc<AtomicBool>) {
    let fired = Arc::new(AtomicBool::new(false));
    let flag = fired.clone();
    let hook = Arc::new(move |_path: &Path, op: &'static str| {
        if op == want_op && !flag.swap(true, Ordering::SeqCst) {
            assert!(
                swap_in_link(&victim, &outside),
                "test environment cannot plant links"
            );
        }
    });
    (hook, fired)
}

const OUTSIDE_BYTES: &[u8] = b"outside-secret-bytes";
const INSIDE_BYTES: &[u8] = b"inside-bytes";

// ── LocalBlobDir (single-component leaf) ─────────────────────────

/// `stat` resolves the leaf at T0, then `fs::metadata` re-traverses
/// at T1 — a leaf swap reports the outside file's length.
#[test]
#[ignore = "N1 RED evidence — blocked on Ruviform rooted-read capability; Refs #85"]
fn blob_stat_leaf_swap_reports_outside_facts() {
    let tree = TempTree::new("blob-swap-stat");
    let outside = tree.join("outside-secret");
    fs::write(&outside, OUTSIDE_BYTES).unwrap();
    let dir = tree.join("blobs");
    fs::create_dir_all(&dir).unwrap();
    let leaf = dir.join("deadbeef");
    fs::write(&leaf, INSIDE_BYTES).unwrap();

    let mut blobs = LocalBlobDir::new(&dir);
    let (hook, fired) = swap_once(leaf, outside, "stat");
    blobs.set_pre_traverse_hook(Some(hook));

    let obj = ObjectIdentity::new("deadbeef").unwrap();
    let result = blobs.stat(&obj);
    assert!(fired.load(Ordering::SeqCst), "barrier never fired");
    assert!(
        result.is_err(),
        "stat served an out-of-root object: len {:?}",
        result.ok()
    );
}

/// `open` resolves at T0 and `File::open` re-traverses at T1 — a leaf
/// swap streams the outside file's bytes.
#[test]
#[ignore = "N1 RED evidence — blocked on Ruviform rooted-read capability; Refs #85"]
fn blob_open_leaf_swap_serves_outside_bytes() {
    let tree = TempTree::new("blob-swap-open");
    let outside = tree.join("outside-secret");
    fs::write(&outside, OUTSIDE_BYTES).unwrap();
    let dir = tree.join("blobs");
    fs::create_dir_all(&dir).unwrap();
    let leaf = dir.join("deadbeef");
    fs::write(&leaf, INSIDE_BYTES).unwrap();

    let mut blobs = LocalBlobDir::new(&dir);
    let (hook, fired) = swap_once(leaf, outside, "open");
    blobs.set_pre_traverse_hook(Some(hook));

    let obj = ObjectIdentity::new("deadbeef").unwrap();
    if let Ok(mut blob) = blobs.open(&obj) {
        let mut leaked = Vec::new();
        blob.reader.read_to_end(&mut leaked).unwrap();
        panic!(
            "leaf swap served {} out-of-root bytes: {:?}",
            leaked.len(),
            String::from_utf8_lossy(&leaked)
        );
    }
    assert!(fired.load(Ordering::SeqCst), "barrier never fired");
}

// ── LocalFolderProvider (intermediate component) ─────────────────

/// `stat` on `sub/inner.wav`: the intermediate `sub` component is
/// swapped for a link to an outside directory after resolve — the
/// reported facts describe the outside file.
#[test]
#[ignore = "N1 RED evidence — blocked on Ruviform rooted-read capability; Refs #85"]
fn local_stat_component_swap_reports_outside_facts() {
    let tree = TempTree::new("local-swap-stat");
    let outside_dir = tree.join("outside");
    fs::create_dir_all(&outside_dir).unwrap();
    fs::write(outside_dir.join("inner.wav"), OUTSIDE_BYTES).unwrap();
    let root = tree.join("root");
    let sub = root.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("inner.wav"), INSIDE_BYTES).unwrap();

    let mut provider = LocalFolderProvider::new(pid(1), &root).unwrap();
    let (hook, fired) = swap_once(sub, outside_dir, "stat");
    provider.set_pre_traverse_hook(Some(hook));

    let result = provider.stat(&aref(pid(1), "sub/inner.wav"), &cx());
    assert!(fired.load(Ordering::SeqCst), "barrier never fired");
    assert!(
        result.is_err(),
        "stat served an out-of-root object: {:?}",
        result.ok()
    );
}

/// `open_preview` resolves `sub/inner.wav` then `File::open`
/// re-traverses — the swapped component streams outside bytes with
/// `Unverified` claims.
#[test]
#[ignore = "N1 RED evidence — blocked on Ruviform rooted-read capability; Refs #85"]
fn local_preview_component_swap_serves_outside_bytes() {
    let tree = TempTree::new("local-swap-preview");
    let outside_dir = tree.join("outside");
    fs::create_dir_all(&outside_dir).unwrap();
    fs::write(outside_dir.join("inner.wav"), OUTSIDE_BYTES).unwrap();
    let root = tree.join("root");
    let sub = root.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("inner.wav"), INSIDE_BYTES).unwrap();

    let mut provider = LocalFolderProvider::new(pid(1), &root).unwrap();
    let (hook, fired) = swap_once(sub, outside_dir, "open");
    provider.set_pre_traverse_hook(Some(hook));

    if let Ok(mut read) = provider.open_preview(&aref(pid(1), "sub/inner.wav"), None, &cx()) {
        let mut leaked = Vec::new();
        read.reader.read_to_end(&mut leaked).unwrap();
        panic!(
            "component swap served {} out-of-root bytes: {:?}",
            leaked.len(),
            String::from_utf8_lossy(&leaked)
        );
    }
    assert!(fired.load(Ordering::SeqCst), "barrier never fired");
}

/// `pin` re-traverses at `File::open` — the swapped component is
/// digested as pinned content, so the returned `digest_hint` is the
/// outside file's SHA-256 (a content-disclosure oracle), not an
/// error.
#[test]
#[ignore = "N1 RED evidence — blocked on Ruviform rooted-read capability; Refs #85"]
fn local_pin_component_swap_digests_outside_file() {
    let tree = TempTree::new("local-swap-pin");
    let outside_dir = tree.join("outside");
    fs::create_dir_all(&outside_dir).unwrap();
    fs::write(outside_dir.join("inner.wav"), OUTSIDE_BYTES).unwrap();
    let root = tree.join("root");
    let sub = root.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("inner.wav"), INSIDE_BYTES).unwrap();

    let mut provider = LocalFolderProvider::new(pid(1), &root).unwrap();
    let (hook, fired) = swap_once(sub, outside_dir, "open");
    provider.set_pre_traverse_hook(Some(hook));

    let result = provider.pin(&aref(pid(1), "sub/inner.wav"), &cx());
    assert!(fired.load(Ordering::SeqCst), "barrier never fired");
    assert!(
        result.is_err(),
        "pin fixed an out-of-root object: {:?}",
        result.ok().map(|p| p.digest_hint)
    );
}

/// `open_pinned` — the pinned path leaks too: EOF verification
/// detects the digest mismatch only *after* the outside bytes were
/// already delivered to the caller's buffer. Content integrity is
/// not capability containment.
#[test]
#[ignore = "N1 RED evidence — blocked on Ruviform rooted-read capability; Refs #85"]
fn local_open_pinned_component_swap_leaks_before_eof() {
    let tree = TempTree::new("local-swap-pinned");
    let outside_dir = tree.join("outside");
    fs::create_dir_all(&outside_dir).unwrap();
    // Same length as INSIDE_BYTES so the expected_size precondition
    // does not pre-empt the demonstration — only digest mismatches,
    // and only at EOF.
    fs::write(outside_dir.join("inner.wav"), b"outside-scrt").unwrap();
    let root = tree.join("root");
    let sub = root.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("inner.wav"), INSIDE_BYTES).unwrap();

    let mut provider = LocalFolderProvider::new(pid(1), &root).unwrap();
    let pin = provider
        .pin(&aref(pid(1), "sub/inner.wav"), &cx())
        .expect("pin real file");
    let (hook, fired) = swap_once(sub, outside_dir, "open");
    provider.set_pre_traverse_hook(Some(hook));

    let mut delivered = Vec::new();
    if let Ok(mut read) = provider.open_pinned(&pin, &cx()) {
        // The EOF integrity check may fire — the leak is that bytes
        // reached the buffer before it did.
        let _ = read.reader.read_to_end(&mut delivered);
    }
    assert!(fired.load(Ordering::SeqCst), "barrier never fired");
    assert!(
        delivered.is_empty(),
        "open_pinned delivered {} out-of-root bytes before verification: {:?}",
        delivered.len(),
        String::from_utf8_lossy(&delivered)
    );
}

/// `browse` descend: the verified scope directory is swapped for a
/// link to an outside directory between resolve and `read_dir` — the
/// outside directory's entries are enumerated under root locators.
#[test]
#[ignore = "N1 RED evidence — blocked on Ruviform rooted-read capability; Refs #85"]
fn local_browse_descend_component_swap_lists_outside_entries() {
    let tree = TempTree::new("local-swap-browse");
    let outside_dir = tree.join("outside");
    fs::create_dir_all(&outside_dir).unwrap();
    fs::write(outside_dir.join("out-secret.wav"), OUTSIDE_BYTES).unwrap();
    let root = tree.join("root");
    let sub = root.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("inner.wav"), INSIDE_BYTES).unwrap();

    let mut provider = LocalFolderProvider::new(pid(1), &root).unwrap();
    let (hook, fired) = swap_once(sub, outside_dir, "read_dir");
    provider.set_pre_traverse_hook(Some(hook));

    let mut req = browse_req(16);
    req.scope = BrowseScope::Locator {
        locator: loc("sub"),
    };
    if let Ok(page) = provider.browse(&req, &cx()) {
        let served: Vec<_> = page
            .entries
            .iter()
            .map(|e| e.display_name.as_str())
            .collect();
        assert!(
            !served.contains(&"out-secret.wav"),
            "descend listed out-of-root entries: {served:?}"
        );
    }
    assert!(fired.load(Ordering::SeqCst), "barrier never fired");
}

// ── seam sanity (GREEN — proves the barrier is wired) ────────────

/// The barrier fires exactly at the tagged re-traversal points during
/// a normal `stat`+`open` sequence; with no swap installed the
/// operations succeed as before.
#[test]
fn pre_traverse_hook_fires_at_tagged_points() {
    let tree = TempTree::new("seam-sanity");
    let dir = tree.join("blobs");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("deadbeef"), INSIDE_BYTES).unwrap();

    let ops = Arc::new(std::sync::Mutex::new(Vec::<&'static str>::new()));
    let log = ops.clone();
    let mut blobs = LocalBlobDir::new(&dir);
    blobs.set_pre_traverse_hook(Some(Arc::new(move |_p, op| {
        log.lock().unwrap().push(op);
    })));

    let obj = ObjectIdentity::new("deadbeef").unwrap();
    blobs.stat(&obj).unwrap();
    let mut blob = blobs.open(&obj).unwrap();
    let mut buf = Vec::new();
    blob.reader.read_to_end(&mut buf).unwrap();
    assert_eq!(buf, INSIDE_BYTES);
    assert_eq!(*ops.lock().unwrap(), vec!["stat", "stat", "open"]);

    // Same wiring on the folder provider: a locator-scoped browse
    // must tag the scope check, the directory enumeration, and the
    // per-entry stat.
    let root = tree.join("root");
    let sub = root.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("inner.wav"), INSIDE_BYTES).unwrap();
    let ops = Arc::new(std::sync::Mutex::new(Vec::<&'static str>::new()));
    let log = ops.clone();
    let mut provider = LocalFolderProvider::new(pid(1), &root).unwrap();
    provider.set_pre_traverse_hook(Some(Arc::new(move |_p, op| {
        log.lock().unwrap().push(op);
    })));
    let mut req = browse_req(16);
    req.scope = BrowseScope::Locator {
        locator: loc("sub"),
    };
    let page = provider.browse(&req, &cx()).unwrap();
    assert_eq!(page.entries.len(), 1);
    assert_eq!(*ops.lock().unwrap(), vec!["stat", "read_dir", "stat"]);
}
