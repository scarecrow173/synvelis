//! ALIB-05 residual 2 acceptance tests (synvelis#646) — the
//! `pending_commit` restore/reinsert path: a job parked at
//! `commit-pending` when the process dies reinserts *live* on
//! `syn.project.open` (the durable manifest supplies the
//! `ImportIntent`), and the first `syn.library.*` op's
//! `reconcile_jobs` replays the owed `CommitVerifiedImport` on the
//! real controller — `committed-live` on a matching revision,
//! `stale-revision` when the live revision moved.
//!
//! - Reinserted entries project honestly (`running`/`commit-pending`/
//!   `pending_commit`, no runtime job) until the reconcile lands the
//!   terminal; they never fake a queued/running worker.
//! - A `commit-pending` entry whose manifest cannot prove the intent
//!   (missing, unreadable, foreign `import_id`, unknown format, or
//!   pre-`Published`) keeps the sv643 `recoverable` degrade — no
//!   receipt is fabricated.
//! - A recorded `import.cancel` restores on the reinserted job; the
//!   owed commit still replays, identical to a live parked commit.
//!
//! Restart is the sv643 headless stand-in: the persisted store/session
//! are rebuilt (`reopen_import_app`); the worker thread never is.
//! Providers/import ports are the shared in-memory fakes + temp-dir
//! stores (synvelis#91/#114 own the real-provider/real-GUI gates).

mod common;

use std::path::Path;
use std::sync::Arc;

use common::library::*;
use ruviform_ui::BackendReport;
use serde_json::{Value, json};

use synvelis_app::{LibraryImportServices, SynvelisApp};
use synvelis_library::PrincipalId;
use synvelis_workspace::LibraryBackend;

// ---------------------------------------------------------------------------
// helpers (mirror sv643's restart fixture over `dir`)
// ---------------------------------------------------------------------------

/// Fresh temp bundle dir for the *project* side.
fn project_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "sv646-proj-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

/// A second "process": fresh `SynvelisApp` with the same fake
/// provider registry and import services rebound over `dir`.
fn reopen_import_app(dir: &Path) -> SynvelisApp {
    let store = synvelis_library::ImportStore::new(dir.join("staging"), dir.join("media"));
    let blobs = synvelis_library::LocalBlobDir::new(store.blob_dir());
    let mut app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    app.set_library_backend(LibraryBackend {
        registry: registry_with(FakeProvider::new(1, true, true, true)),
        principal: PrincipalId::new("test").unwrap(),
    });
    app.set_library_import(LibraryImportServices {
        store,
        blobs: Arc::new(blobs),
        publisher: Arc::new(synvelis_library::StagedBlobPublisher),
    });
    app
}

/// Selects the fixture provider, then refreshes browse.
fn select_and_browse(app: &mut SynvelisApp, view: &str) {
    action(
        app,
        view,
        json!({ "action": "provider.select", "provider": PROVIDER_TAG }),
    );
    action(app, view, json!({ "action": "browse.refresh" }));
}

/// The durable manifest stage on disk — the restart-deterministic
/// observation point `syn.library.jobs` cannot show (reading the
/// ledger would reconcile the parked commit first).
fn manifest_stage(dir: &Path, import_id: u64) -> Option<String> {
    let path = dir
        .join("staging")
        .join(format!("import-{import_id:032x}"))
        .join("import-manifest.json");
    let bytes = std::fs::read(path).ok()?;
    let v: Value = serde_json::from_slice(&bytes).ok()?;
    v["stage"].as_str().map(str::to_string)
}

/// Parks an `import.submit` job at `commit-pending`: the
/// `GatePublisher` holds the worker mid-publish until `gate.open()`.
/// The worker then either commits the scratch controller or the job
/// watcher parks it at the durable publish boundary; both paths end in
/// `park_commit` arming the owed live commit. Returns once the raw ledger
/// witnesses that owed commit — a reconciling ledger read would consume
/// it first, so the raw snapshot is the park observation point.
fn submit_and_park(app: &mut SynvelisApp, dir: &Path, gate: &Gate, import_id: u64) {
    let view = open_view(app);
    select_and_browse(app, &view);
    action_json(
        app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": import_id,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    wait_stage(app, &import_key(import_id), "materializing");
    gate.open();
    let parked = wait_raw_commit_boundary(app, &import_key(import_id));
    assert!(
        parked["pending_commit"].as_bool() == Some(true),
        "import {import_id} reached the commit boundary without parking pending_commit: {parked}"
    );
    let stage = manifest_stage(dir, import_id);
    assert!(
        matches!(stage.as_deref(), Some("published" | "committed")),
        "the durable manifest proves the publish boundary behind the parked ledger entry: stage={stage:?}"
    );
}

// ---------------------------------------------------------------------------
// AC2 — pending_commit intent persistence + commit replay
// ---------------------------------------------------------------------------

#[test]
fn pending_commit_replays_to_committed_live_after_reopen() {
    let gate = Gate::new();
    let (mut app, dir) = app_with_import_gate(
        "sv646-replay",
        gate.clone(),
        vec![FakeProvider::new(1, true, true, true)],
    );
    let proj = project_dir("replay");
    app.project_new(&proj.to_string_lossy(), Some("p"))
        .expect("project_new");
    let live_before = app.project_info()["revision"].as_u64().unwrap_or(0);
    submit_and_park(&mut app, &dir, &gate, 21);
    // The parked commit is saved owed — the sidecar captures
    // `pending_commit` (the ledger read stays unreconciled: save
    // snapshots without reconciling).
    app.project_save().expect("project_save");
    drop(app);

    let mut app2 = reopen_import_app(&dir);
    app2.project_open(&proj.to_string_lossy())
        .expect("project_open");
    // The first `syn.library.*` read reconciles — the reinserted
    // entry's intent replays `import_verified` on the real controller.
    let restored = job(&mut app2, &import_key(21)).expect("restored ledger entry");
    assert_eq!(restored["state"], "done");
    assert_eq!(restored["pending_commit"], false);
    assert_eq!(restored["terminal"]["kind"], "committed-live");
    assert_eq!(
        restored["terminal"]["import_id"],
        "import-00000000000000000000000000000015"
    );
    assert_eq!(restored["terminal"]["reused"], false);
    // The replayed commit advanced the live revision and registered
    // in the project — the receipt is project truth, not a scratch.
    let live_after = app2.project_info()["revision"].as_u64().unwrap();
    assert!(
        live_after > live_before,
        "replayed commit advances the live revision"
    );
    let imports = app2
        .op_library_imports(&json!({}))
        .expect("syn.library.imports");
    let row = &imports["imports"].as_array().unwrap()[0];
    assert_eq!(row["stage"], "committed");
    assert_eq!(row["applied"], true);

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn pending_commit_stale_revision_lands_typed_terminal() {
    // The manifest's `expected_project_revision` is the only revision
    // the commit may use — a live revision that moved under the saved
    // intent lands the typed `stale-revision` terminal on replay,
    // never a silent rebase. `syn.project.set_ppqn` moves the live
    // revision without touching `reconcile_jobs`, so the park and the
    // staleness coexist in one saved sidecar.
    let gate = Gate::new();
    let (mut app, dir) = app_with_import_gate(
        "sv646-stale",
        gate.clone(),
        vec![FakeProvider::new(1, true, true, true)],
    );
    let proj = project_dir("stale");
    app.project_new(&proj.to_string_lossy(), Some("p"))
        .expect("project_new");
    submit_and_park(&mut app, &dir, &gate, 22);
    // The parked intent pinned revision 0 — move the live project.
    app.project_set_ppqn(&json!({
        "old_ppqn": 960,
        "new_ppqn": 480,
        "policy": "exact",
    }))
    .expect("project_set_ppqn bumps the live revision");
    assert_eq!(app.project_info()["revision"], 1);
    app.project_save().expect("project_save");
    drop(app);

    let mut app2 = reopen_import_app(&dir);
    app2.project_open(&proj.to_string_lossy())
        .expect("project_open");
    let restored = job(&mut app2, &import_key(22)).expect("restored ledger entry");
    assert_eq!(restored["state"], "done");
    assert_eq!(restored["pending_commit"], false);
    assert_eq!(restored["terminal"]["kind"], "stale-revision");
    assert_eq!(restored["terminal"]["expected"], 0);
    assert_eq!(restored["terminal"]["actual"], 1);
    // The project stayed at the PPQN revision — the stale commit
    // never landed, and `applied` stays false.
    assert_eq!(app2.project_info()["revision"], 1);
    let imports = app2
        .op_library_imports(&json!({}))
        .expect("syn.library.imports");
    let row = &imports["imports"].as_array().unwrap()[0];
    assert_eq!(row["applied"], false);

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn pending_commit_without_manifest_degrades_recoverable() {
    // The sidecar claims `commit-pending` but the durable manifest is
    // gone — the intent can no longer be proven, so the entry keeps
    // the sv643 `recoverable` degrade rather than inventing a replay.
    let gate = Gate::new();
    let (mut app, dir) = app_with_import_gate(
        "sv646-missing",
        gate.clone(),
        vec![FakeProvider::new(1, true, true, true)],
    );
    let proj = project_dir("missing");
    app.project_new(&proj.to_string_lossy(), Some("p"))
        .expect("project_new");
    submit_and_park(&mut app, &dir, &gate, 23);
    app.project_save().expect("project_save");
    drop(app);
    // The manifest disappears between save and reopen.
    std::fs::remove_dir_all(dir.join("staging").join(format!("import-{:032x}", 23)))
        .expect("remove manifest dir");

    let mut app2 = reopen_import_app(&dir);
    app2.project_open(&proj.to_string_lossy())
        .expect("project_open");
    let restored = job(&mut app2, &import_key(23)).expect("restored ledger entry");
    assert_eq!(restored["state"], "done");
    assert_eq!(restored["pending_commit"], false);
    assert_eq!(restored["terminal"]["kind"], "recoverable");
    assert_eq!(restored["terminal"]["reason"], "interrupted");
    assert_eq!(restored["terminal"]["stage"], "commit-pending");
    // Nothing was applied — no fabricated receipt.
    let imports = app2
        .op_library_imports(&json!({}))
        .expect("syn.library.imports");
    assert!(
        imports["imports"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["import_id"].as_str() != Some("import-00000000000000000000000000000017"))
    );

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn pending_commit_unreadable_manifest_degrades_recoverable() {
    // A torn/corrupt manifest cannot prove the intent — same degrade
    // as the missing manifest (fail-closed, never a misread replay).
    let gate = Gate::new();
    let (mut app, dir) = app_with_import_gate(
        "sv646-corrupt",
        gate.clone(),
        vec![FakeProvider::new(1, true, true, true)],
    );
    let proj = project_dir("corrupt");
    app.project_new(&proj.to_string_lossy(), Some("p"))
        .expect("project_new");
    submit_and_park(&mut app, &dir, &gate, 24);
    app.project_save().expect("project_save");
    drop(app);
    std::fs::write(
        dir.join("staging")
            .join(format!("import-{:032x}", 24))
            .join("import-manifest.json"),
        "{not-json",
    )
    .expect("corrupt manifest");

    let mut app2 = reopen_import_app(&dir);
    app2.project_open(&proj.to_string_lossy())
        .expect("project_open");
    let restored = job(&mut app2, &import_key(24)).expect("restored ledger entry");
    assert_eq!(restored["terminal"]["kind"], "recoverable");
    assert_eq!(restored["terminal"]["stage"], "commit-pending");

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn pending_commit_cancel_requested_still_replays() {
    // A sidecar that recorded `cancel_requested` on a `commit-pending`
    // entry restores the flag through `request_cancel` — and the owed
    // commit still replays, identical to a live parked commit (the
    // flag marks it; `reconcile_jobs` does not filter on it).
    // Production reaches this wire state through cancel paths that do
    // not reconcile first (e.g. window-scoped job cancels) — the test
    // plants the flag on the real saved sidecar.
    let gate = Gate::new();
    let (mut app, dir) = app_with_import_gate(
        "sv646-cancel",
        gate.clone(),
        vec![FakeProvider::new(1, true, true, true)],
    );
    let proj = project_dir("cancel");
    app.project_new(&proj.to_string_lossy(), Some("p"))
        .expect("project_new");
    submit_and_park(&mut app, &dir, &gate, 25);
    app.project_save().expect("project_save");
    drop(app);

    // Record the cancel on the parked entry inside the saved sidecar.
    let sidecar_path = proj.join("library-jobs.json");
    let mut sidecar: Value =
        serde_json::from_slice(&std::fs::read(&sidecar_path).expect("read sidecar"))
            .expect("parse sidecar");
    for job in sidecar["jobs"].as_array_mut().unwrap() {
        if job["job_key"].as_str() == Some(import_key(25).as_str()) {
            job["cancel_requested"] = json!(true);
            job["state"] = json!("cancel-requested");
        }
    }
    std::fs::write(&sidecar_path, serde_json::to_vec_pretty(&sidecar).unwrap())
        .expect("write sidecar");

    let mut app2 = reopen_import_app(&dir);
    app2.project_open(&proj.to_string_lossy())
        .expect("project_open");
    let restored = job(&mut app2, &import_key(25)).expect("restored ledger entry");
    assert_eq!(restored["state"], "done");
    assert_eq!(restored["cancel_requested"], true);
    assert_eq!(restored["terminal"]["kind"], "committed-live");

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&proj);
}
