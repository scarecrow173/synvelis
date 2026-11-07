//! Deterministic component-swap containment regressions (N1, Refs
//! Synvelis #85).
//!
//! The baseline providers used to `canonicalize` + containment-check a
//! path at T0 and then re-traverse the same path at T1 — between the
//! two, a leaf or intermediate component could be swapped (rename +
//! symlink plant) so the second traversal resolved outside the
//! trusted root. The implementation now resolves everything
//! handle-relative through a Ruviform `RootedDir` capability: the
//! `pre_traverse` fault barrier fires immediately before each rooted
//! operation that touches the filesystem (`"stat"`/`"open"`/
//! `"read_dir"`) and performs the swap synchronously — no threads,
//! no timing: the race is injected deterministically.
//!
//! Contract under test: an object that resolves outside the trusted
//! root must never have its bytes, facts, or directory entries
//! served — the operation must fail. Under the rooted capability a
//! swapped component either stays inside the acquired handles or
//! fails `EscapesRoot`/`NotFound`; there is no post-check window.
//!
//! These were originally RED evidence ignored while the Ruviform
//! rooted-read capability was pending (ruviform#398); they are now
//! enforced GREEN. Scope acquisition consumes the ruviform#409
//! `open_scoped_root` contract — the resolved directory handle
//! itself becomes the capability floor, so there is no pathname
//! re-open window and no `NodeId` gate — and enumeration consumes
//! the bounded `read_dir_each` contract.

mod common;

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use common::{TempTree, aref, browse_req, cx, loc, pid, try_symlink};

use synvelis_library::*;

/// Parks `victim` aside and plants a symlink to `outside` at its
/// name. Returns `false` when symlink creation is denied — the swap
/// is never half-done. These regressions are enforced, so callers
/// assert rather than skip: a silently skipped containment test
/// would be a false-green, and an environment without symlink
/// privilege must report honestly.
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

/// `stat` — a leaf swapped for an outside link must fail the rooted
/// `metadata` call, never report the outside file's length.
#[test]
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

/// `open` — a leaf swapped for an outside link must fail
/// `open_regular`, never stream the outside file's bytes.
#[test]
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

/// `stat` on `sub/inner.wav`: the intermediate `sub` component
/// swapped for a link to an outside directory must fail the rooted
/// `metadata` call — the outside file's facts are never reported.
#[test]
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

/// `open_preview` on `sub/inner.wav`: the swapped component must
/// fail `open_regular` — outside bytes never reach the reader, even
/// with `Unverified` claims.
#[test]
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

/// `pin` — the swapped component must fail `open_regular` before any
/// byte is digested; the outside file's SHA-256 is never returned as
/// a `digest_hint` (a content-disclosure oracle).
#[test]
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

/// `open_pinned` — the swapped component must fail `open_regular`
/// before a single byte reaches the caller's buffer. EOF digest
/// verification is content integrity, not capability containment —
/// it cannot substitute for the rooted open.
#[test]
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

/// `browse` descend: the scope directory swapped for a link after
/// its capability was acquired changes nothing — enumeration runs on
/// the held handle, so the outside directory's entries are never
/// listed under root locators.
#[test]
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

/// `BrowseScope::Locator` — the scope directory itself is swapped
/// for a link to an outside directory at capability acquisition
/// (`open_scope`, tagged `"open"` — the single touch point before
/// `open_scoped_root` resolves the locator). The held-capability
/// resolution follows the link and lands outside the provider
/// root, failing `EscapesRoot` → `PermissionDenied` — outside
/// entries are never listed.
#[test]
fn local_scope_acquisition_swap_cannot_escape_root() {
    let tree = TempTree::new("local-swap-scope");
    let outside_dir = tree.join("outside");
    fs::create_dir_all(&outside_dir).unwrap();
    fs::write(outside_dir.join("out-secret.wav"), OUTSIDE_BYTES).unwrap();
    let root = tree.join("root");
    let sub = root.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("inner.wav"), INSIDE_BYTES).unwrap();

    let mut provider = LocalFolderProvider::new(pid(1), &root).unwrap();
    let (hook, fired) = swap_once(sub, outside_dir, "open");
    provider.set_pre_traverse_hook(Some(hook));

    let mut req = browse_req(16);
    req.scope = BrowseScope::Locator {
        locator: loc("sub"),
    };
    let result = provider.browse(&req, &cx());
    assert!(fired.load(Ordering::SeqCst), "barrier never fired");
    let err = result.expect_err("swapped scope must not serve a page");
    assert_eq!(
        err.kind(),
        ProviderErrorKind::PermissionDenied,
        "acquisition-time scope swap must fail PermissionDenied, got {err:?}"
    );
}

/// Root replacement between provider construction and scope
/// acquisition: the held `RootedDir` — never the root pathname —
/// decides which tree the scope binds to. The original root is
/// renamed aside (the open handle keeps authority over the moved
/// object) and a different tree is planted at the same pathname.
/// `open_scoped_root` resolves `sub` under the *held* handle, so
/// the replacement tree at the old pathname is a different object
/// the capability never reaches: the page serves the held tree's
/// `sub/inner.wav` under canonical-relative locators and never
/// lists `out-secret.wav`.
#[test]
fn local_scope_root_replacement_cannot_rebind_capability() {
    let tree = TempTree::new("local-swap-root");
    let root = tree.join("root");
    let sub = root.join("sub");
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("inner.wav"), INSIDE_BYTES).unwrap();

    let provider = LocalFolderProvider::new(pid(1), &root).unwrap();
    // No barrier needed: the swap is sequential — provider holds the
    // original root capability, then the pathname is repointed at a
    // different tree before `browse` acquires the scope.
    let held = tree.join("root_held");
    fs::rename(&root, &held).unwrap();
    fs::create_dir_all(&sub).unwrap();
    fs::write(sub.join("out-secret.wav"), OUTSIDE_BYTES).unwrap();

    let mut req = browse_req(16);
    req.scope = BrowseScope::Locator {
        locator: loc("sub"),
    };
    let page = provider
        .browse(&req, &cx())
        .expect("held capability still serves its own tree");
    let served: Vec<_> = page.entries.iter().map(|e| e.locator.as_str()).collect();
    assert_eq!(
        served,
        vec!["sub/inner.wav"],
        "scope must serve the held tree under canonical-relative locators, got {served:?}"
    );
}

/// After the scoped capability is acquired, swapping the scope
/// directory itself for an outside link changes nothing —
/// enumeration runs on the held handle (the `"read_dir"` barrier
/// fires before the scope dir's own listing), so the page serves
/// the acquired object's entries and the replacement's
/// `out-secret.wav` is never listed.
#[test]
fn local_scope_post_acquisition_swap_keeps_held_entries() {
    let tree = TempTree::new("local-swap-scope-read-dir");
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
    let page = provider
        .browse(&req, &cx())
        .expect("held scope handle still serves its own entries");
    assert!(fired.load(Ordering::SeqCst), "barrier never fired");
    let served: Vec<_> = page.entries.iter().map(|e| e.locator.as_str()).collect();
    assert_eq!(
        served,
        vec!["sub/inner.wav"],
        "post-acquisition swap must keep serving the held object, got {served:?}"
    );
}

// ── seam sanity (GREEN — proves the barrier is wired) ────────────

/// The barrier fires exactly at the tagged rooted-operation points
/// during a normal `stat`+`open` sequence; with no swap installed
/// the operations succeed as before.
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
    // Rooted `stat` is a single `metadata` call and `open` a single
    // `open_regular` — the old resolve→re-traverse double-stat seam
    // no longer exists.
    assert_eq!(*ops.lock().unwrap(), vec!["stat", "open"]);

    // Same wiring on the folder provider: a locator-scoped browse
    // must tag the scoped capability acquisition, the directory
    // enumeration, and the per-entry stat.
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
    assert_eq!(*ops.lock().unwrap(), vec!["open", "read_dir", "stat"]);
}
