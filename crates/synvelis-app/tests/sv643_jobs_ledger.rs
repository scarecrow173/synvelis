//! ALIB-05 residual acceptance tests (synvelis#643) — the
//! `syn.library.jobs` ledger sidecar persistence/recovery, the
//! `syn.library.imports` `inspect_imports` op, and the generic
//! runtime `job.cancel` wire reaching library jobs.
//!
//! - `syn.project.save`/`new` write `library-jobs.json` beside `HEAD`;
//!   `syn.project.open` restores `done` receipts verbatim and lands
//!   in-flight entries as typed terminals (`recoverable`/`cancelled`/
//!   `failed`) — a headless stand-in for a real process restart (the
//!   persisted store/session are rebuilt; the worker thread never is).
//! - `syn.library.imports` exposes `inspect_imports` on the same
//!   wire/ledger path as the job ops (reconcile → durable manifest
//!   rows → ledger correlation per `ImportId`).
//! - `job.cancel` (ruviform built-in) reaches the cooperative cancel
//!   token a `CallOutcome::Job` worker observes — measured through
//!   `UiRuntime::dispatch_op` + `poll`, never GUI.
//!
//! Providers/import ports are the shared in-memory fakes + temp-dir
//! stores (synvelis#91/#114 own the real-provider/real-GUI gates).

mod common;

use std::path::Path;
use std::sync::Arc;

use common::library::*;
use ruviform_ui::ipc::{Event, JobState};
use ruviform_ui::{BackendReport, DataRegistry, UiRuntime};
use serde_json::{Value, json};

use synvelis_app::{AppService, LibraryImportServices, SynvelisApp};
use synvelis_library::PrincipalId;
use synvelis_workspace::LibraryBackend;

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Fresh temp bundle dir for the *project* side (the fixture's `dir`
/// is the import staging/media root — the project bundle is separate).
fn project_dir(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "sv643-proj-{tag}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ))
}

/// A second "process": fresh `SynvelisApp` with the same fake
/// provider registry and import services rebound over `dir` — the
/// restart half of every recovery test.
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

// ---------------------------------------------------------------------------
// AC1 — ledger persistence + restart recovery
// ---------------------------------------------------------------------------

#[test]
fn jobs_ledger_survives_project_save_and_reopen() {
    // A `done` job's receipt is the durable record: save → "restart"
    // → the ledger projects the same terminal verbatim.
    let (mut app, dir) = app_with_import("sv643-done");
    let proj = project_dir("done");
    app.project_new(&proj.to_string_lossy(), Some("p"))
        .expect("project_new");
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    action_json(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 9,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    let done = wait_job(&mut app, &import_key(9));
    assert_eq!(done["terminal"]["kind"], "committed-live");
    app.project_save().expect("project_save");
    assert!(
        proj.join("library-jobs.json").exists(),
        "save writes the ledger sidecar beside HEAD"
    );
    drop(app);

    let mut app2 = reopen_import_app(&dir);
    app2.project_open(&proj.to_string_lossy())
        .expect("project_open");
    let restored = job(&mut app2, &import_key(9)).expect("restored ledger entry");
    assert_eq!(restored["state"], "done");
    assert_eq!(restored["terminal"]["kind"], "committed-live");
    assert_eq!(
        restored["terminal"]["import_id"], done["terminal"]["import_id"],
        "the committed-live receipt survives verbatim"
    );
    assert_eq!(
        restored["terminal"]["asset_id"],
        done["terminal"]["asset_id"]
    );
    assert_eq!(restored["kind"], "import-submit");

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn restart_restores_inflight_import_as_recoverable() {
    // Parked inside `publish`: the durable manifest sits at `verified`
    // — the restart receipt is `recoverable` at `materializing` (the
    // manifest stage), never a fabricated in-flight state.
    let gate = Gate::new();
    let (mut app, dir) = app_with_import_gate(
        "sv643-inflight",
        gate,
        vec![FakeProvider::new(1, true, true, true)],
    );
    let proj = project_dir("inflight");
    app.project_new(&proj.to_string_lossy(), Some("p"))
        .expect("project_new");
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    action_json(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 10,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    wait_stage(&mut app, &import_key(10), "materializing");
    app.project_save().expect("project_save");
    drop(app); // the parked worker dies with the process — the gate stays closed

    let mut app2 = reopen_import_app(&dir);
    app2.project_open(&proj.to_string_lossy())
        .expect("project_open");
    let restored = job(&mut app2, &import_key(10)).expect("restored ledger entry");
    assert_eq!(restored["state"], "done");
    assert_eq!(restored["terminal"]["kind"], "recoverable");
    assert_eq!(restored["terminal"]["reason"], "interrupted");
    assert_eq!(restored["terminal"]["stage"], "materializing");
    assert!(
        restored["terminal"]["detail"]
            .as_str()
            .unwrap()
            .contains("interrupted")
    );

    // `syn.library.imports` sees the same durable row the terminal
    // was derived from — `verified` manifest, not applied, and the
    // restored ledger entry as `job`.
    let imports = app2
        .op_library_imports(&json!({}))
        .expect("syn.library.imports");
    let row = imports["imports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["import_id"].as_str() == Some("import-0000000000000000000000000000000a"))
        .expect("manifest row");
    assert_eq!(row["stage"], "verified");
    assert_eq!(row["applied"], false);
    assert_eq!(row["job"]["terminal"]["kind"], "recoverable");

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn restart_restores_pending_commit_as_recoverable() {
    // The restart lands between `park_commit` and the service-thread
    // reconcile: the ledger saved `pending_commit` while the durable
    // manifest proves `Published` — the receipt is `recoverable` at
    // `commit-pending`, and no commit is replayed.
    let gate = Gate::new();
    let (mut app, dir) = app_with_import_gate(
        "sv643-park",
        gate.clone(),
        vec![FakeProvider::new(1, true, true, true)],
    );
    let proj = project_dir("park");
    app.project_new(&proj.to_string_lossy(), Some("p"))
        .expect("project_new");
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    action_json(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 13,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    wait_stage(&mut app, &import_key(13), "materializing");
    gate.open();
    // Reading the ledger would reconcile the parked commit — the
    // durable manifest is the deterministic park witness instead.
    // The worker's dry-run `import_verified` commits on a scratch
    // controller, so `committed` on the manifest is what parks the
    // job; `applied` stays false until the service-thread reconcile
    // replays the commit on the live controller.
    for _ in 0..2000 {
        if manifest_stage(&dir, 13).as_deref() == Some("committed") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert_eq!(manifest_stage(&dir, 13).as_deref(), Some("committed"));
    // `park_commit` runs µs after the worker returns; give the
    // post-join ledger write headroom before the save snapshot.
    std::thread::sleep(std::time::Duration::from_millis(200));
    app.project_save().expect("project_save");
    drop(app);

    let mut app2 = reopen_import_app(&dir);
    app2.project_open(&proj.to_string_lossy())
        .expect("project_open");
    let restored = job(&mut app2, &import_key(13)).expect("restored ledger entry");
    assert_eq!(restored["state"], "done");
    assert_eq!(restored["pending_commit"], false);
    assert_eq!(restored["terminal"]["kind"], "recoverable");
    assert_eq!(restored["terminal"]["reason"], "interrupted");
    assert_eq!(restored["terminal"]["stage"], "commit-pending");
    // The owed commit was never replayed — the manifest says
    // `committed` but the live registry never applied it.
    let imports = app2
        .op_library_imports(&json!({}))
        .expect("syn.library.imports");
    let row = &imports["imports"].as_array().unwrap()[0];
    assert_eq!(row["stage"], "committed");
    assert_eq!(row["applied"], false);

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn restart_restores_cancel_requested_as_cancelled() {
    // An `import.cancel` recorded before the restart restores as the
    // `cancelled` terminal — `resumable` because the durable manifest
    // exists.
    let gate = Gate::new();
    let (mut app, dir) = app_with_import_gate(
        "sv643-cancelreq",
        gate,
        vec![FakeProvider::new(1, true, true, true)],
    );
    let proj = project_dir("cancelreq");
    app.project_new(&proj.to_string_lossy(), Some("p"))
        .expect("project_new");
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    action_json(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 14,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    wait_stage(&mut app, &import_key(14), "materializing");
    action(
        &mut app,
        &view,
        json!({ "action": "import.cancel", "import_id": 14 }),
    );
    assert_eq!(
        job(&mut app, &import_key(14)).unwrap()["cancel_requested"],
        true
    );
    app.project_save().expect("project_save");
    drop(app);

    let mut app2 = reopen_import_app(&dir);
    app2.project_open(&proj.to_string_lossy())
        .expect("project_open");
    let restored = job(&mut app2, &import_key(14)).expect("restored ledger entry");
    assert_eq!(restored["state"], "done");
    assert_eq!(restored["cancel_requested"], true);
    assert_eq!(restored["terminal"]["kind"], "cancelled");
    assert_eq!(restored["terminal"]["resumable"], true);
    assert_eq!(restored["terminal"]["stage"], "materializing");

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn restart_restores_queued_preview_as_failed() {
    // A preview job holds no durable state — the restart receipt is
    // `failed`/`interrupted`, honest about nothing to resume.
    let gate = Gate::new();
    let (mut app, dir) = app_with_import_gate(
        "sv643-preview",
        gate.clone(),
        vec![FakeProvider::new(1, true, true, true).with_preview_gate(gate)],
    );
    let proj = project_dir("preview");
    app.project_new(&proj.to_string_lossy(), Some("p"))
        .expect("project_new");
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    action_json(
        &mut app,
        &view,
        json!({
            "action": "preview.fetch",
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    assert!(
        job(&mut app, &preview_key(&view)).is_some(),
        "the fetch registers synchronously"
    );
    app.project_save().expect("project_save");
    drop(app);

    let mut app2 = reopen_import_app(&dir);
    app2.project_open(&proj.to_string_lossy())
        .expect("project_open");
    let restored = job(&mut app2, &preview_key(&view)).expect("restored ledger entry");
    assert_eq!(restored["state"], "done");
    assert_eq!(restored["kind"], "preview-fetch");
    assert_eq!(restored["terminal"]["kind"], "failed");
    assert_eq!(restored["terminal"]["reason"], "interrupted");

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn project_new_overwrites_a_stale_sidecar() {
    // A bundle dir may already carry a `library-jobs.json` (foreign
    // residue): `project_new` rewrites it for the new project id, so a
    // stale receipt can never surface under the new project.
    let (mut app, dir) = app_with_import("sv643-new");
    let proj = project_dir("new");
    std::fs::create_dir_all(&proj).expect("project dir");
    std::fs::write(
        proj.join("library-jobs.json"),
        json!({
            "format": "synvelis-library-jobs/1",
            "project_id": "project-00000000000000000000000000000009",
            "jobs": [{
                "job_key": "import:import-00000000000000000000000000000009",
                "kind": "import-submit",
                "view": "v3",
                "import_id": "import-00000000000000000000000000000009",
                "state": "done",
                "cancel_requested": false,
                "pending_commit": false,
                "epoch": 1,
                "terminal": { "kind": "failed", "reason": "interrupted", "detail": "stale" },
            }],
        })
        .to_string(),
    )
    .expect("plant stale sidecar");
    app.project_new(&proj.to_string_lossy(), Some("p"))
        .expect("project_new");

    // The sidecar now names this project's id with an empty ledger.
    let sidecar: Value =
        serde_json::from_str(&std::fs::read_to_string(proj.join("library-jobs.json")).unwrap())
            .unwrap();
    assert_eq!(sidecar["jobs"], json!([]));
    assert_eq!(
        sidecar["project_id"].as_str(),
        app.project_info()["project_id"].as_str()
    );

    let mut app2 = reopen_import_app(&dir);
    app2.project_open(&proj.to_string_lossy())
        .expect("project_open");
    assert!(
        jobs(&mut app2).is_empty(),
        "no stale job rows survive the rewrite"
    );

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&proj);
}

#[test]
fn foreign_project_sidecar_is_ignored_on_open() {
    // A sidecar naming a different `project_id` is residue, not data —
    // open skips it without error and without inventing job rows.
    let (mut app, dir) = app_with_import("sv643-foreign");
    let proj = project_dir("foreign");
    app.project_new(&proj.to_string_lossy(), Some("p"))
        .expect("project_new");
    let sidecar = proj.join("library-jobs.json");
    let mut planted: Value =
        serde_json::from_str(&std::fs::read_to_string(&sidecar).unwrap()).unwrap();
    planted["project_id"] = json!("project-000000000000000000000000000000ff");
    std::fs::write(&sidecar, planted.to_string()).unwrap();
    drop(app);

    let mut app2 = reopen_import_app(&dir);
    app2.project_open(&proj.to_string_lossy())
        .expect("project_open");
    assert!(jobs(&mut app2).is_empty(), "foreign sidecar not restored");

    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::remove_dir_all(&proj);
}

// ---------------------------------------------------------------------------
// AC2 — `syn.library.imports` (inspect_imports on the ledger wire path)
// ---------------------------------------------------------------------------

#[test]
fn library_imports_reports_manifest_and_ledger_job() {
    let (mut app, dir) = app_with_import("sv643-imports");
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    action_json(
        &mut app,
        &view,
        json!({
            "action": "import.submit",
            "import_id": 11,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    );
    let done = wait_job(&mut app, &import_key(11));
    assert_eq!(done["terminal"]["kind"], "committed-live");

    let imports = app
        .op_library_imports(&json!({}))
        .expect("syn.library.imports");
    let row = imports["imports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["import_id"].as_str() == Some("import-0000000000000000000000000000000b"))
        .expect("manifest row");
    assert_eq!(row["stage"], "committed");
    assert_eq!(row["applied"], true);
    assert_eq!(row["job_key"], import_key(11));
    assert_eq!(row["job"]["state"], "done");
    assert_eq!(row["job"]["terminal"]["kind"], "committed-live");
    assert_eq!(row["byte_len"], ASSET_BYTES.len() as u64);

    // Unbound import services answer `unavailable`, never an empty
    // inspection masquerading as "no imports".
    let mut bare = common::library::app();
    let err = bare
        .op_library_imports(&json!({}))
        .expect_err("unbound services");
    assert_eq!(err.code, "unavailable");

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// AC3 — generic `job.cancel` wire reaches library jobs (headless)
// ---------------------------------------------------------------------------

/// Extracts the single `result` event payload from a dispatch.
fn result_of(events: Vec<Event>) -> (bool, Value) {
    events
        .into_iter()
        .find_map(|e| match e {
            Event::Result { ok, data, .. } => Some((ok, data.unwrap_or(Value::Null))),
            _ => None,
        })
        .expect("dispatch yields a result event")
}

/// The `syn.library.jobs` entry for `key` read through the runtime.
fn rt_job(rt: &mut UiRuntime, key: &str) -> Option<Value> {
    let (_, data) = result_of(rt.dispatch_op("t", "syn.library.jobs", json!({})));
    data["jobs"]
        .as_array()?
        .iter()
        .find(|j| j["job_key"].as_str() == Some(key))
        .cloned()
}

/// Polls `rt` until job `id` lands a `JobState::Cancelled` event.
fn wait_job_cancelled(rt: &mut UiRuntime, id: u64) {
    for _ in 0..400 {
        for e in rt.poll(Some(std::time::Duration::from_millis(20))) {
            if let Event::Job {
                job,
                state: JobState::Cancelled,
                ..
            } = e
                && job == id
            {
                return;
            }
        }
    }
    panic!("job {id} never produced a Cancelled event");
}

#[test]
fn generic_job_cancel_reaches_import_job() {
    // `job.cancel` (ruviform built-in) → the `JobContext` token →
    // `RuntimeJobDrive::is_cancelled` → the job's cooperative
    // `ProviderContext` token → `BlobPublishError::Interrupted` → the
    // ledger `cancelled` terminal + a `job_cancelled` runtime event.
    let gate = Gate::new();
    let (mut app, dir) = app_with_import_gate(
        "sv643-jcimport",
        gate,
        vec![FakeProvider::new(1, true, true, true)],
    );
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    let service = AppService::new(app);
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(service).expect("register AppService");

    let (ok, ack) = result_of(rt.dispatch_op(
        "t",
        "syn.library.action",
        json!({
            "view_id": view,
            "action": "import.submit",
            "import_id": 12,
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    ));
    assert!(ok);
    assert_eq!(ack["label"], "syn.library.import.submit");
    let job_id = ack["job"].as_u64().expect("runtime job id");

    // Park inside publish (manifest `verified`) — the wire entry
    // carries the runtime job correlation a cancel caller needs.
    let mut entry = None;
    for _ in 0..2000 {
        entry = rt_job(&mut rt, &import_key(12));
        if entry.as_ref().and_then(|e| e["stage"].as_str()) == Some("materializing") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let entry = entry.expect("ledger entry");
    assert_eq!(entry["stage"], "materializing");
    assert_eq!(entry["runtime_job"].as_u64(), Some(job_id));

    let (ok, res) = result_of(rt.dispatch_op("t", "job.cancel", json!({ "job": job_id })));
    assert_eq!(res, json!({ "job": job_id, "cancelling": true }), "ok={ok}");
    wait_job_cancelled(&mut rt, job_id);

    let done = rt_job(&mut rt, &import_key(12)).expect("ledger entry");
    assert_eq!(done["state"], "done");
    assert_eq!(done["terminal"]["kind"], "cancelled");
    // The durable manifest (`verified`) makes the cancel resumable.
    assert_eq!(done["terminal"]["resumable"], true);
    assert_eq!(done["terminal"]["stage"], "materializing");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn generic_job_cancel_reaches_preview_job() {
    // Same wire on the preview lane: `job.cancel` raises the
    // `JobContext` token the drive observes at the next read
    // boundary — the ledger lands `cancelled` and the runtime emits
    // `job_cancelled`.
    let gate = Gate::new();
    let (mut app, dir) = app_with_publisher(
        "sv643-jcpreview",
        Arc::new(synvelis_library::StagedBlobPublisher),
        vec![FakeProvider::new(1, true, true, true).with_preview_gate(gate.clone())],
    );
    let view = open_view(&mut app);
    select_and_browse(&mut app, &view);
    let service = AppService::new(app);
    let mut rt = UiRuntime::new(DataRegistry::new());
    rt.register_service(service).expect("register AppService");

    let (ok, ack) = result_of(rt.dispatch_op(
        "t",
        "syn.library.action",
        json!({
            "view_id": view,
            "action": "preview.fetch",
            "locator": "loc-a",
            "allow_full_object": true,
        }),
    ));
    assert!(ok);
    assert_eq!(ack["label"], "syn.library.preview.fetch");
    let job_id = ack["job"].as_u64().expect("runtime job id");

    // The entry registers synchronously at dispatch; `runtime_job`
    // lands inside `run_plan_job` on the job thread — poll until the
    // correlation is visible on the wire.
    let key = preview_key(&view);
    let mut entry = None;
    for _ in 0..2000 {
        entry = rt_job(&mut rt, &key);
        if entry
            .as_ref()
            .and_then(|e| e["runtime_job"].as_u64())
            .is_some()
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    let entry = entry.expect("ledger entry");
    assert_eq!(entry["runtime_job"].as_u64(), Some(job_id));

    let (ok, res) = result_of(rt.dispatch_op("t", "job.cancel", json!({ "job": job_id })));
    assert_eq!(res, json!({ "job": job_id, "cancelling": true }), "ok={ok}");
    // The gated reader observes the ledger/ctx boundary only after
    // the gate releases — the cancel is already latched on the
    // JobContext token the drive checks at the next read.
    gate.open();
    wait_job_cancelled(&mut rt, job_id);

    let done = rt_job(&mut rt, &key).expect("ledger entry");
    assert_eq!(done["state"], "done");
    assert_eq!(
        done["terminal"],
        json!({ "kind": "cancelled", "resumable": false, "stage": null })
    );

    let _ = std::fs::remove_dir_all(&dir);
}
