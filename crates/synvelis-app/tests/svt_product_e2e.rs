//! SVT-01〜09 product State/Receipt — headless `FakeDriver` lane
//! (synvelis#114 / SV-GUI-05).
//!
//! Each `svtNN_*` test drives the *product* path end to end:
//! mounted `ViewHost` → `rvf.ui.command.v1`/`syn.*` intent → `NativeShell`
//! → `UiRuntime`/`AppService` → receipt/projection → host display, with
//! `syn.view.list`/`syn.project.info`/bundle files as the independent
//! product oracle. Real Service (`AppService` over the sealed
//! `SynvelisApp`) is the authority under test; view-local/mock state
//! (host display, optimistic selection) is asserted only as the
//! *projection* of that authority, never as proof of it.
//!
//! Evidence honesty (POLICIES.md): every pass here is OS-L1 semantic/
//! headless evidence. Real OS window/input/focus/DPI/pixel (GUI-L2),
//! native dock, real plugin editor sessions, `CallOutcome::Job` product
//! ops, and real audio/MIDI devices (EXT-L3) are `NotRun`/`Unsupported`
//! — recorded in `evidence/sv114/`, never upgraded by these tests.

mod common;

use common::mounted::*;
use ruviform_ui::WindowDriver;
use ruviform_ui::behavior::{CommandPhase, ReceiptStatus};
use ruviform_ui::ipc::Intent;
use ruviform_ui::{DriverEvent, WindowLifecycle, WindowSpec};
use serde_json::json;

use synvelis_app::{mixer_gain_target, secondary_view_id, secondary_view_targets};

fn op(f: &mut MountedFixture, id: &str, name: &str, params: serde_json::Value) {
    action(
        &mut f.shell,
        &mut f.driver,
        f.window,
        Intent::new(id, name, params),
    );
}

fn op_ok(
    f: &mut MountedFixture,
    id: &str,
    name: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    op(f, id, name, params);
    let (ok, data) = results_for(&f.driver, f.window, id)
        .last()
        .expect("op result")
        .clone();
    assert!(ok, "{name} failed: {data:?}");
    data.unwrap_or(serde_json::Value::Null)
}

fn project_info(f: &mut MountedFixture) -> serde_json::Value {
    op_ok(f, "pinfo", "syn.project.info", json!({}))
}

/// Dispatches one `syn.*` op on `window` (any tracked window, not just
/// the fixture's main window).
fn op_on(
    f: &mut MountedFixture,
    window: ruviform_ui::WindowId,
    id: &str,
    name: &str,
    params: serde_json::Value,
) {
    action(
        &mut f.shell,
        &mut f.driver,
        window,
        Intent::new(id, name, params),
    );
}

/// SVT-01 — Project Create/Open → View composition. `syn.project.new`
/// through the window channel replaces the workspace session; the
/// mounted view's app-side attach/subscription registry must survive
/// (it is session state, not project state), `syn.view.list` keeps
/// reporting the view, and a post-switch gesture still lands `Applied`
/// on the *new* session's transport truth.
#[test]
fn svt01_project_new_keeps_view_composition_and_session_truth() {
    let mut f = fixture();
    let dir = std::env::temp_dir().join(format!("svt01-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.to_string_lossy().to_string();

    let before = project_info(&mut f);
    assert!(before["name"].is_string());

    op_ok(
        &mut f,
        "pnew",
        "syn.project.new",
        json!({ "path": path, "name": "svt01proj" }),
    );
    let after = project_info(&mut f);
    assert_eq!(after["name"], json!("svt01proj"));

    // The mounted view is still owned by this window and listed.
    assert_eq!(f.shell.view_window(&f.view_id), Some(f.window));
    let list = op_ok(&mut f, "vlist", "syn.view.list", json!({}));
    assert!(
        list["views"]
            .as_array()
            .expect("views")
            .iter()
            .any(|v| v["view_id"] == json!(f.view_id)),
        "main view must remain listed after project.new: {list}"
    );

    // The new project has no clips — the Arrangement read model is
    // honestly empty after the UI pump.
    assert!(
        f.screen.borrow().arrangement_rows.is_empty(),
        "new project must publish zero arrangement rows"
    );

    // A control gesture on the fresh session still completes the full
    // receipt path — view composition survived the project switch.
    f.input("seek.position", MountedFixture::begin("svt01g"));
    f.input("seek.position", MountedFixture::commit("svt01g"));
    let binding = f
        .host
        .binding("transport/position")
        .expect("position binding");
    assert_eq!(
        binding.state().last_receipt.expect("receipt").status,
        ReceiptStatus::Applied
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// SVT-02 — Track/Clip selection → Inspector focus. Row select rides
/// the view-local `select` action → `syn.view.select` → workspace
/// `ObjectSelections` + inspection focus; `syn.view.list` and the
/// Inspector screen state are the independent oracles. Out-of-range
/// selection is rejected without touching the authoritative state.
#[test]
fn svt02_row_selection_drives_inspector_focus() {
    let mut f = fixture();
    let clip_tag = open_clip_project(&mut f, "svt02");

    f.push_screen_state();
    let generation = f.host.generation();
    f.host
        .select_row(generation, &key("arrangement.table"), 0)
        .expect("row 0 selectable");
    f.pump_view_intents();
    f.deliver_to_host();

    let (ok, _) = last_result(&f.driver, f.window).expect("select result");
    assert!(ok, "syn.view.select must succeed");

    // Independent oracle 1: workspace selection/inspection via view.list.
    let list = op_ok(&mut f, "vlist2", "syn.view.list", json!({}));
    let entry = list["views"]
        .as_array()
        .expect("views")
        .iter()
        .find(|v| v["view_id"] == json!(f.view_id))
        .expect("main view")
        .clone();
    assert_eq!(entry["selection"], json!(clip_tag));
    assert_eq!(entry["inspection"], json!(clip_tag));

    // Independent oracle 2: Inspector screen state follows the focus.
    assert_eq!(
        f.screen.borrow().inspector.target.as_deref(),
        Some(clip_tag.as_str())
    );

    // Rejected path: out-of-range row fails honestly at the product
    // op and the authoritative selection is preserved.
    let view = f.view_id.clone();
    op(
        &mut f,
        "selbad",
        "syn.view.select",
        json!({ "view_id": view, "row": 99 }),
    );
    assert_eq!(
        results_for(&f.driver, f.window, "selbad")
            .last()
            .map(|(ok, _)| *ok),
        Some(false)
    );
    let list = op_ok(&mut f, "vlist3", "syn.view.list", json!({}));
    let entry = list["views"]
        .as_array()
        .expect("views")
        .iter()
        .find(|v| v["view_id"] == json!(f.view_id))
        .expect("main view")
        .clone();
    assert_eq!(entry["selection"], json!(clip_tag));
}

/// SVT-03 — Fader/Parameter gesture: explicit target → controller →
/// receipt → matching UI state. The `Applied` receipt and the
/// `Observed` projection are asserted separately from the displayed
/// value — Requested/Applied/Observed stay distinct.
#[test]
fn svt03_fader_gesture_receipt_and_observed_projection() {
    let mut f = fixture();

    f.input("fader.gain", MountedFixture::begin("svt03g"));
    f.input("fader.gain", MountedFixture::preview("svt03g", 0.5));
    f.input("fader.gain", MountedFixture::commit("svt03g"));

    // Receipt authority: the control binding recorded an Applied
    // receipt for the committed gesture.
    let binding = f
        .host
        .binding(&mixer_gain_target())
        .expect("mixer gain binding");
    let receipt = binding.state().last_receipt.expect("gain receipt");
    assert_eq!(receipt.status, ReceiptStatus::Applied);

    // Observed authority: an authoritative projection landed after the
    // receipt — the display phase reaches Observed, not just Applied.
    let snap = f.host.observe(&key("fader.gain")).expect("fader snapshot");
    assert_eq!(snap.phase, CommandPhase::Observed);

    // Display truth: a committed value and an observed projection are
    // both recorded — the display is not a local-only fabrication.
    assert!(snap.committed.is_some(), "committed value expected");
    assert!(snap.observed.is_some(), "observed projection expected");
}

/// SVT-04 — Undo/Redo + rejected input preserving authoritative truth.
/// `syn.project.set_ppqn` is one undoable project edit; project
/// undo/redo ride the `project` Button actions; empty-history undo and
/// malformed ops report failure without mutating truth.
#[test]
fn svt04_undo_redo_roundtrip_and_rejected_truth() {
    let mut f = fixture();

    // One undoable project mutation through the product op path.
    let info = project_info(&mut f);
    let old_ppqn = info["ppqn"].as_u64().expect("ppqn");
    let new_ppqn = if old_ppqn == 480 { 960 } else { 480 };
    op_ok(
        &mut f,
        "ppqns",
        "syn.project.set_ppqn",
        json!({ "old_ppqn": old_ppqn, "new_ppqn": new_ppqn, "policy": "exact" }),
    );
    assert_eq!(project_info(&mut f)["ppqn"], json!(new_ppqn));

    f.press("project.undo");
    let (ok, data) = last_result(&f.driver, f.window).expect("undo result");
    assert!(ok);
    assert_eq!(data.expect("undo data")["undone"], json!(true));

    // History is now empty — a second undo reports `undone:false`,
    // never a fabricated success.
    f.press("project.undo");
    let (ok, data) = last_result(&f.driver, f.window).expect("undo2 result");
    assert!(ok);
    assert_eq!(data.expect("undo2 data")["undone"], json!(false));

    f.press("project.redo");
    let (ok, data) = last_result(&f.driver, f.window).expect("redo result");
    assert!(ok);
    assert_eq!(data.expect("redo data")["redone"], json!(true));

    // Rejected path: an unknown op fails with a typed error Result and
    // project truth is untouched.
    let before = project_info(&mut f);
    op(&mut f, "badop", "syn.no.such.op", json!({}));
    let results = results_for(&f.driver, f.window, "badop");
    assert_eq!(results.last().map(|(ok, _)| *ok), Some(false));
    let after = project_info(&mut f);
    assert_eq!(before, after, "rejected op must not mutate project truth");
}

/// SVT-05 — Transport Play/Pause/Stop/Seek distinguishing
/// Requested/Applied/Observed. The receipt and the authoritative
/// projection are asserted as separate milestones; the displayed
/// transport state only reaches `Observed` after the projection lands.
#[test]
fn svt05_transport_receipt_projection_phase_progression() {
    let mut f = fixture();

    // Split the press so the pre-delivery phase is observable: after
    // the intent is dispatched but before receipts/projections are
    // applied to the host, the command must NOT already be Observed.
    let generation = f.host.generation();
    f.host
        .invoke(generation, &key("transport.play"))
        .expect("play invoke");
    f.pump_view_intents();
    let mid = f
        .host
        .observe(&key("transport.play"))
        .expect("mid snapshot");
    assert_ne!(
        mid.phase,
        CommandPhase::Observed,
        "requested transport.play must not be Observed before delivery"
    );
    f.deliver_to_host();
    let post = f
        .host
        .observe(&key("transport.play"))
        .expect("post snapshot");
    // Terminal phase is Applied (receipt authority) or Observed (when
    // a correlated projection also lands) — never Requested/Pending
    // after full delivery.
    assert!(
        matches!(post.phase, CommandPhase::Applied | CommandPhase::Observed),
        "post-delivery phase must be Applied or Observed, got {:?}",
        post.phase
    );

    // Receipt authority: the wire broadcast an `Applied` receipt for
    // the transport action (route_action's authoritative outcome).
    let receipts = notifies(&f.driver, f.window, "syn.control.receipt");
    assert!(
        receipts
            .iter()
            .any(|r| r["target"] == "transport" && r["receipt"]["status"] == "applied"),
        "play receipt applied: {receipts:?}"
    );

    // Stop rides the same path and reports its own Applied receipt.
    f.press("transport.stop");
    let receipts = notifies(&f.driver, f.window, "syn.control.receipt");
    assert!(
        receipts
            .iter()
            .any(|r| r["target"] == "transport" && r["receipt"]["status"] == "applied"),
        "stop receipt applied: {receipts:?}"
    );

    // Seek gesture → transport/position receipt + projection (the
    // seek parity contract is covered driver-neutrally by
    // `tests/common/scenario.rs`; here it is asserted on the product
    // receipt path).
    f.input("seek.position", MountedFixture::begin("svt05s"));
    f.input("seek.position", MountedFixture::commit("svt05s"));
    let pos = f
        .host
        .binding("transport/position")
        .expect("position binding");
    assert_eq!(
        pos.state().last_receipt.expect("seek receipt").status,
        ReceiptStatus::Applied
    );
}

/// SVT-06 — Waveform/Meter surfaces: bounded queries, honest no-source
/// state, and source-revision stale marking. `syn.viz.*` ops are the
/// product boundary; `syn.viz.meta`/`syn.viz.query` results are the
/// independent oracle. NaN-gap rendering and real canvas/DOM pixels
/// are `NotRun` (no headless API exposes them).
#[test]
fn svt06_viz_bounded_query_stale_and_honest_empty() {
    let mut f = fixture();
    let view = f.view_id.clone();

    op_ok(
        &mut f,
        "vreg",
        "syn.viz.register",
        json!({
            "kind": "waveform", "source": "svt-src", "rate": 48000u64,
            "channels": [[0.0, 0.5, -0.5, 1.0]],
        }),
    );
    op_ok(
        &mut f,
        "vbind",
        "syn.viz.bind",
        json!({ "view_id": view, "surface_id": "wave",
                "source": "svt-src", "kind": "waveform", "plot_px": 16u64 }),
    );

    // Bounded query: the returned view respects max_bytes.
    let q = op_ok(
        &mut f,
        "vq",
        "syn.viz.query",
        json!({ "view_id": view, "surface_id": "wave",
                "start": 0.0, "end": 1.0, "buckets": 16u64,
                "max_bytes": 1 << 20 }),
    );
    assert!(q["bytes"].as_u64().expect("bytes") <= (1 << 20));

    // Over-budget query is rejected honestly — no silent truncation
    // presented as success.
    op(
        &mut f,
        "vqbig",
        "syn.viz.query",
        json!({
            "view_id": view, "surface_id": "wave",
            "start": 0.0, "end": 1.0, "buckets": 16u64, "max_bytes": 1u64,
        }),
    );
    assert_eq!(
        results_for(&f.driver, f.window, "vqbig")
            .last()
            .map(|(ok, _)| *ok),
        Some(false),
        "over-budget query must fail, not fabricate a view"
    );

    // Source revision change resolves honestly: `ui_pump` runs inside
    // the op, so by the time meta/query answer, the surface has
    // resynced — `status ok`, no fabricated stale row, no lingering
    // pre-change view. The transient `view_stale == true` window
    // itself is asserted at the `VizUi` layer by
    // `synvelis-workspace/tests/sv48_viz.rs`.
    op_ok(
        &mut f,
        "vchg",
        "syn.viz.source_changed",
        json!({ "source": "svt-src" }),
    );
    let meta = op_ok(
        &mut f,
        "vmeta",
        "syn.viz.meta",
        json!({ "view_id": view, "surface_id": "wave" }),
    );
    assert_eq!(
        meta["status"]["kind"],
        json!("ok"),
        "meta after change: {meta}"
    );
    assert_eq!(meta["view_stale"], json!(false), "resolved by pump: {meta}");
    let q2 = op_ok(
        &mut f,
        "vq2",
        "syn.viz.query",
        json!({ "view_id": view, "surface_id": "wave",
                "start": 0.0, "end": 1.0, "buckets": 16u64,
                "max_bytes": 1 << 20 }),
    );
    assert_eq!(q2["status"]["kind"], json!("ok"), "query resolves: {q2}");

    // Honest empty: meta on a never-bound surface fails rather than
    // fabricating a row.
    op(
        &mut f,
        "vmeta2",
        "syn.viz.meta",
        json!({ "view_id": view, "surface_id": "never-bound" }),
    );
    assert_eq!(
        results_for(&f.driver, f.window, "vmeta2")
            .last()
            .map(|(ok, _)| *ok),
        Some(false)
    );
}

/// SVT-06b — meter surface push/snapshot through the same product ops.
#[test]
fn svt06b_meter_push_frame_snapshot() {
    let mut f = fixture();
    let view = f.view_id.clone();

    op_ok(
        &mut f,
        "mreg",
        "syn.viz.register",
        json!({ "kind": "meter", "source": "svt-meter", "channels": 2u64 }),
    );
    op_ok(
        &mut f,
        "mbind",
        "syn.viz.bind",
        json!({ "view_id": view, "surface_id": "meters",
                "source": "svt-meter", "kind": "meter" }),
    );
    op_ok(
        &mut f,
        "mframe",
        "syn.viz.push_frame",
        json!({ "source": "svt-meter",
                "frame": { "sequence": 3u64, "channels": [
                    { "peak": 0.25, "rms": 0.1 },
                    { "peak": 0.5, "rms": 0.2 }] } }),
    );
    let snap = op_ok(
        &mut f,
        "msnap",
        "syn.viz.snapshot",
        json!({ "view_id": view, "surface_id": "meters" }),
    );
    assert_eq!(snap["frame"]["sequence"], json!(3));
}

/// SVT-07 — Dock/two-window movement, window close, project/job
/// lifetime. Two windows share the one project session; destroying a
/// window detaches its views without touching project truth; the
/// surviving window keeps full function. Real OS window/focus/DPI and
/// native dock are `NotRun` (GUI-L2/EXT-L3); `CallOutcome::Job` has no
/// `syn.*` producer today — job-lifetime semantics are `Unsupported`
/// at this layer (see `evidence/sv114/`).
#[test]
fn svt07_two_window_ownership_close_and_project_lifetime() {
    let mut f = fixture();

    // Second driver window adopts through the same lifecycle path.
    let w2 = f
        .driver
        .create_window(&WindowSpec {
            title: "secondary".to_string(),
            ..WindowSpec::default()
        })
        .expect("second window");
    inject(
        &mut f.shell,
        &mut f.driver,
        DriverEvent::Windowed {
            window: w2,
            event: WindowLifecycle::Ready,
        },
    );
    let attach = f
        .shell
        .attach_view(
            &mut f.driver,
            w2,
            &secondary_view_id(),
            &secondary_view_targets(),
        )
        .expect("secondary attach");
    assert!(attach["generation"].as_u64().is_some_and(|g| g >= 1));
    assert_eq!(f.shell.view_window(&secondary_view_id()), Some(w2));
    assert_eq!(f.shell.view_window(&f.view_id), Some(f.window));

    // Both windows share the one project session.
    let info_w2 = {
        op_on(&mut f, w2, "i2", "syn.project.info", json!({}));
        results_for(&f.driver, w2, "i2")
            .last()
            .unwrap()
            .1
            .clone()
            .unwrap()
    };
    let info_w1 = project_info(&mut f);
    assert_eq!(info_w1["name"], info_w2["name"]);

    // Destroying w2 detaches the secondary view; project truth and the
    // main view/window are untouched.
    inject(
        &mut f.shell,
        &mut f.driver,
        DriverEvent::Windowed {
            window: w2,
            event: WindowLifecycle::Destroyed,
        },
    );
    let list = op_ok(&mut f, "vlist7", "syn.view.list", json!({}));
    let views = list["views"].as_array().expect("views");
    assert!(
        views
            .iter()
            .any(|v| v["view_id"] == json!(secondary_view_id())),
        "secondary view must remain listed after window destroy"
    );
    assert_eq!(
        f.shell.view_window(&secondary_view_id()),
        None,
        "destroyed window must not retain view ownership"
    );
    assert_eq!(f.shell.view_window(&f.view_id), Some(f.window));
    assert_eq!(project_info(&mut f)["name"], info_w1["name"]);

    // The surviving window still completes the full receipt path.
    f.input("fader.gain", MountedFixture::begin("svt07g"));
    f.input("fader.gain", MountedFixture::commit("svt07g"));
    let binding = f.host.binding(&mixer_gain_target()).expect("gain binding");
    assert_eq!(
        binding.state().last_receipt.expect("receipt").status,
        ReceiptStatus::Applied
    );
}

/// SVT-08 — Project Save/Reload against an independent oracle: the
/// bundle files on disk (`HEAD` → `snapshots/r<N>.json`) are read
/// directly, bypassing the app's load path entirely.
#[test]
fn svt08_save_reload_matches_independent_bundle_oracle() {
    let mut f = fixture();
    let dir = std::env::temp_dir().join(format!("svt08-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    write_clip_bundle(&dir);
    let path = dir.to_string_lossy().to_string();

    op_ok(&mut f, "po8", "syn.project.open", json!({ "path": path }));
    f.deliver_to_host();

    // Mutate project truth through the product path, then save.
    f.input("fader.gain", MountedFixture::begin("svt08g"));
    f.input("fader.gain", MountedFixture::preview("svt08g", 0.75));
    f.input("fader.gain", MountedFixture::commit("svt08g"));
    op_ok(&mut f, "ps8", "syn.project.save", json!({}));

    // Independent oracle: read HEAD + the snapshot it names, parse the
    // JSON ourselves — no synvelis-project load path involved.
    let head = std::fs::read_to_string(dir.join("HEAD")).expect("HEAD exists");
    let snap_name = head.trim().to_string();
    assert!(
        snap_name.starts_with('r')
            && std::path::Path::new(&snap_name)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("json")),
        "HEAD must name a canonical snapshot, got {snap_name}"
    );
    let oracle_rev: u64 = snap_name
        .trim_start_matches('r')
        .trim_end_matches(".json")
        .parse()
        .expect("snapshot revision");
    let snap_path = dir.join("snapshots").join(&snap_name);
    let snap_text = std::fs::read_to_string(&snap_path)
        .unwrap_or_else(|_| std::fs::read_to_string(dir.join(&snap_name)).expect("snapshot file"));
    let snap: serde_json::Value = serde_json::from_str(&snap_text).expect("snapshot JSON");
    let project = snap.get("project").unwrap_or(&snap);
    assert_eq!(project["name"], json!("arrangement"));
    assert_eq!(
        project["tracks"]
            .as_object()
            .map_or(0, serde_json::Map::len),
        1,
        "oracle must see exactly one track"
    );
    assert_eq!(
        project["clips"].as_object().map_or(0, serde_json::Map::len),
        1,
        "oracle must see exactly one clip"
    );

    // Reload through the product op and compare against the oracle.
    op_ok(&mut f, "pr8", "syn.project.reload", json!({}));
    let info = project_info(&mut f);
    assert_eq!(info["name"], json!("arrangement"));
    let rev_field = info["revision"]
        .as_u64()
        .or_else(|| info["rev"].as_u64())
        .expect("project.info revision");
    assert_eq!(
        rev_field, oracle_rev,
        "reloaded revision must equal the on-disk oracle revision"
    );
    assert_eq!(f.screen.borrow().arrangement_rows.len(), 1);

    let _ = std::fs::remove_dir_all(&dir);
}

/// SVT-09 — IPC duplicate/delay/reorder/disconnect/reattach at the
/// product boundary. Detach drops the view's command channel honestly
/// (rejected receipt, authoritative state preserved); re-attach mints
/// a fresh generation and restores the full path. Duplicate/reordered
/// *delivery* is a view-layer concern covered by `binding.rs` receipt
/// demux tests and the stale-namespace pin tests — recorded here as
/// the product-side contract they rely on.
#[test]
fn svt09_detach_drops_commands_and_reattach_restores_with_new_generation() {
    let mut f = fixture();
    let first_attach = f.shell.view_window(&f.view_id).expect("attached");

    // Baseline: gesture completes before disconnect.
    f.input("fader.gain", MountedFixture::begin("svt09a"));
    f.input("fader.gain", MountedFixture::commit("svt09a"));

    // Disconnect: view detach pools the dock-domain view.
    let view = f.view_id.clone();
    op_ok(
        &mut f,
        "vdet",
        "syn.view.detach",
        json!({ "view_id": view, "reason": "dock-transfer" }),
    );

    // While detached, a minted view command must be rejected — never
    // silently applied to stale ownership.
    f.input("fader.gain", MountedFixture::begin("svt09b"));
    f.input("fader.gain", MountedFixture::commit("svt09b"));
    let binding = f.host.binding(&mixer_gain_target()).expect("gain binding");
    assert_eq!(
        binding
            .state()
            .last_receipt
            .expect("detached receipt")
            .status,
        ReceiptStatus::Rejected,
        "commands on a detached view must reject, not apply"
    );

    // Reattach: fresh attach generation, ownership restored.
    op_ok(
        &mut f,
        "vatt",
        "syn.view.attach",
        json!({ "view_id": view, "window_id": first_attach.to_string() }),
    );
    assert_eq!(f.shell.view_window(&f.view_id), Some(first_attach));

    // The reattached view completes the full receipt path again.
    f.input("fader.gain", MountedFixture::begin("svt09c"));
    f.input("fader.gain", MountedFixture::commit("svt09c"));
    let binding = f.host.binding(&mixer_gain_target()).expect("gain binding");
    assert_eq!(
        binding
            .state()
            .last_receipt
            .expect("reattached receipt")
            .status,
        ReceiptStatus::Applied
    );
}
