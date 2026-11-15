//! SV266-NR-05 (synvelis#266) — AUTX-07 composition-root mount wiring
//! and `autx.*` action dispatch tests.
//!
//! Covers `docs/test-specs/autx-07-mount-wiring.md`:
//! - `AUTX-MOUNT-REGISTER` — `syn.autx.visual{action:"open"}` mints a
//!   detached view per surface, the shared automation screen cell
//!   publishes one state per registered view, and `automation_screen_spec`
//!   builds its spec.
//! - `AUTX-DECODE-PARAMS` / `AUTX-STRICT-REJECT` — params-bearing
//!   `autx.*` intents pass `decode_view_command`; unknown targets,
//!   actions, and malformed params reject typed.
//! - `AUTX-DISPATCH-OWN` — the shell routes `autx.*` to
//!   `syn.autx.action` and the op's `Result` settles on the originating
//!   window channel.
//! - `AUTX-LOCAL-ONLY` / `AUTX-NOCATALOG-REJECT` /
//!   `AUTX-CATALOG-LOWER` — view-local mutations touch only
//!   `ViewState.automation`; request actions reject typed without a
//!   pushed catalog and lower to wire request JSON with one.
//!
//! The mounted lane composes app → automation `ViewHost` →
//! `NativeShell` → `FakeDriver` exactly as `synvelis-native` does —
//! headless semantic evidence only (synvelis#114 lane boundary).

mod common;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use ruviform_ui::behavior::{CommandReceipt, ReceiptStatus, Revision};
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::ipc::Event;
use ruviform_ui::view::{ViewEvent, ViewHost};
use ruviform_ui::{
    BackendReport, DriverEvent, WindowDriver, WindowId, WindowLifecycle, WindowSpec,
};
use ruviform_ui_test::FakeDriver;
use serde_json::{Value, json};

use synvelis_app::automation::{
    AutomationScreenPublish, AutomationScreenState, automation_screen_spec, mount_automation_view,
};
use synvelis_app::binding::decode_view_command;
use synvelis_app::{NativeShell, SynvelisApp};

use common::mounted::{action, inject, key, results_for};

const SURFACES: [&str; 4] = [
    "action-palette",
    "macro-browser",
    "workflow-editor",
    "run-monitor",
];

/// Compact authority-pushed catalog: `set-position` (no target) +
/// `add-note` (resource target) + one two-step macro preset. The JSON
/// shape is the strict document `syn.autx.catalog` / the workspace
/// `AutomationCatalog` seam parse — `synvelis-app` cannot name
/// gateway types, so the document is literal here.
fn catalog_doc() -> Value {
    json!({
        "seq": 3,
        "capabilities": [
            {
                "descriptor": {
                    "availability": { "state": "available" },
                    "data_access": [],
                    "domain": "runtime-control",
                    "effect_bound": [],
                    "execution": "sync",
                    "id": "synvelis.transport.set-position",
                    "input_schema": {
                        "name": "synvelis.transport.set-position.input",
                        "version": { "major": 1, "minor": 0 }
                    },
                    "output_schema": {
                        "name": "synvelis.transport.set-position.output",
                        "version": { "major": 1, "minor": 0 }
                    },
                    "required_permissions": ["control-transport"],
                    "target": { "shape": "none" },
                    "version": { "major": 1, "minor": 0 }
                },
                "grant": { "state": "satisfied" }
            },
            {
                "descriptor": {
                    "availability": { "state": "available" },
                    "data_access": ["metadata-read"],
                    "domain": "project-mutation",
                    "effect_bound": ["read", "write"],
                    "execution": "sync",
                    "id": "synvelis.midi.add-note",
                    "input_schema": {
                        "name": "synvelis.midi.add-note.input",
                        "version": { "major": 1, "minor": 1 }
                    },
                    "output_schema": {
                        "name": "synvelis.midi.add-note.output",
                        "version": { "major": 1, "minor": 0 }
                    },
                    "required_permissions": ["mutate-project"],
                    "target": { "kind": "source", "shape": "resource" },
                    "version": { "major": 1, "minor": 1 }
                },
                "grant": { "state": "satisfied" }
            }
        ],
        "macros": [
            {
                "id": "preset.transport-home",
                "name": "Transport Home",
                "version": "1.0",
                "inputs": [],
                "steps": [
                    {
                        "capability": {
                            "id": "synvelis.transport.set-position",
                            "version": { "major": 1, "minor": 0 }
                        },
                        "args": { "position": 0 },
                        "target": null
                    },
                    {
                        "capability": {
                            "id": "synvelis.transport.set-position",
                            "version": { "major": 1, "minor": 0 }
                        },
                        "args": { "position": 960 },
                        "target": null
                    }
                ],
                "effects": [],
                "undo_scope": "none",
                "atomic": true,
                "availability": { "state": "available" },
                "grant": { "state": "satisfied" },
                "executable": true,
                "blocked": null
            }
        ]
    })
}

fn app() -> SynvelisApp {
    SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()))
}

fn open_surface(app: &mut SynvelisApp, surface: &str) -> String {
    app.op_autx_visual(&json!({ "action": "open", "surface": surface }))
        .expect("syn.autx.visual open")["view_id"]
        .as_str()
        .expect("view_id")
        .to_string()
}

fn parse_view(view_id: &str) -> ViewInstanceId {
    let n: u64 = view_id
        .strip_prefix("view-")
        .and_then(|s| s.parse().ok())
        .expect("view-{n} id");
    ViewInstanceId::new(n).expect("nonzero view id")
}

/// Index of the surface by its `syn.autx.visual` name.
fn surface_index(surface: &str) -> usize {
    SURFACES
        .iter()
        .position(|s| *s == surface)
        .expect("known surface")
}

/// One mounted automation surface: its wire `view_id`, adopted
/// instance, `ViewHost`, owning `WindowId`, and the event cursor into
/// that window's append-only log.
struct AutxView {
    view_id: String,
    inst: ViewInstanceId,
    host: ViewHost,
    window: WindowId,
    cursor: usize,
}

/// app → four automation `ViewHost`s → `NativeShell` → `FakeDriver`,
/// one `Ready` window per surface — the same composition order
/// `synvelis-native` uses under `SYNVELIS_AUTX_VIEWS`.
struct AutxMounted {
    shell: NativeShell,
    driver: FakeDriver,
    views: Vec<AutxView>,
    cell: Rc<RefCell<BTreeMap<ViewInstanceId, AutomationScreenPublish>>>,
}

fn mounted_fixture(with_catalog: bool) -> AutxMounted {
    let mut app = app();
    let epochs = app.attach_epoch_allocator();
    let mut view_ids = Vec::new();
    for surface in SURFACES {
        view_ids.push(open_surface(&mut app, surface));
    }
    if with_catalog {
        app.op_autx_catalog(&catalog_doc())
            .expect("syn.autx.catalog");
    }
    let cell = app.automation_screen_cell();

    let mut views = Vec::new();
    for view_id in &view_ids {
        let inst = parse_view(view_id);
        let publish = cell
            .borrow()
            .get(&inst)
            .unwrap_or_else(|| panic!("{view_id} publishes a screen state"))
            .clone();
        let host = mount_automation_view(&epochs, inst, &publish.state)
            .unwrap_or_else(|e| panic!("{view_id} mounts: {e}"));
        views.push((view_id.clone(), inst, host));
    }

    let mut shell = NativeShell::new(app, Duration::from_millis(16)).expect("compose shell");
    let mut driver = FakeDriver::new();
    let mut mounted = Vec::new();
    for (view_id, inst, host) in views {
        let window = driver
            .create_window(&WindowSpec {
                title: view_id.clone(),
                ..WindowSpec::default()
            })
            .expect("surface window");
        inject(
            &mut shell,
            &mut driver,
            DriverEvent::Windowed {
                window,
                event: WindowLifecycle::Ready,
            },
        );
        shell
            .attach_view(&mut driver, window, &view_id, &[])
            .unwrap_or_else(|e| panic!("attach {view_id}: {e}"));
        mounted.push(AutxView {
            view_id,
            inst,
            host,
            window,
            cursor: 0,
        });
    }
    AutxMounted {
        shell,
        driver,
        views: mounted,
        cell,
    }
}

impl AutxMounted {
    /// Re-publishes the view's current screen state into its host —
    /// the headless mirror of the native `update_view` push.
    fn push_state(&mut self, index: usize) {
        let publish = {
            let view = &self.views[index];
            self.cell
                .borrow()
                .get(&view.inst)
                .unwrap_or_else(|| panic!("{} still publishes", view.view_id))
                .clone()
        };
        let spec = automation_screen_spec(&publish.state);
        let view = &mut self.views[index];
        view.host
            .update(spec, view.host.generation())
            .expect("screen state update applies");
    }

    /// Drains the host's minted intents onto its window as
    /// `rvf.ui.command.v1` Actions, then applies every newly delivered
    /// window event back to the host — the same mint/deliver path as
    /// `MountedFixture::input`.
    fn pump(&mut self, index: usize) {
        let events: Vec<ViewEvent> = self.views[index].host.drain_events();
        for event in events {
            if let ViewEvent::Intent {
                intent,
                command_id,
                expect_receipt,
            } = event
            {
                let params = json!({
                    "command_id": command_id,
                    "expect_receipt": expect_receipt,
                    "intent": intent,
                });
                let view = &self.views[index];
                action(
                    &mut self.shell,
                    &mut self.driver,
                    view.window,
                    ruviform_ui::ipc::Intent::new(command_id, "rvf.ui.command.v1", params),
                );
            }
        }
        self.deliver(index);
    }

    /// Applies newly delivered `Result` events on the view's window to
    /// its host as command receipts (same decode as
    /// `MountedFixture::deliver_to_host`).
    fn deliver(&mut self, index: usize) {
        let window = self.views[index].window;
        let events: Vec<Event> = self
            .driver
            .events_for(window)
            .into_iter()
            .skip(self.views[index].cursor)
            .cloned()
            .collect();
        self.views[index].cursor += events.len();
        for event in events {
            if let Event::Result {
                id,
                ok,
                data,
                error,
            } = event
            {
                let revision = data
                    .as_ref()
                    .and_then(|d| d.get("revision"))
                    .and_then(|v| serde_json::from_value::<Revision>(v.clone()).ok());
                let mut detail = error.map(|e| e.message).or_else(|| {
                    data.as_ref()
                        .and_then(|d| d.get("detail"))
                        .and_then(Value::as_str)
                        .map(str::to_string)
                });
                let status = if ok {
                    match data.as_ref().and_then(|d| d.get("status")) {
                        None => ReceiptStatus::Accepted,
                        Some(raw) => serde_json::from_value::<ReceiptStatus>(raw.clone())
                            .unwrap_or_else(|_| {
                                detail = Some(format!("unrecognized receipt status: {raw}"));
                                ReceiptStatus::Rejected
                            }),
                    }
                } else {
                    ReceiptStatus::Rejected
                };
                self.views[index].host.apply_receipt(&CommandReceipt {
                    command_id: id,
                    status,
                    revision,
                    detail,
                });
            }
        }
    }

    /// `ViewHost::select_row` + intent pump/deliver — a table row
    /// selection end-to-end.
    fn select_row(&mut self, index: usize, component: &str, row: usize) {
        let generation = self.views[index].host.generation();
        self.views[index]
            .host
            .select_row(generation, &key(component), row)
            .expect("row selectable");
        self.pump(index);
    }

    /// `ViewHost::invoke` + intent pump/deliver — a Button press
    /// end-to-end.
    fn press(&mut self, index: usize, component: &str) {
        let generation = self.views[index].host.generation();
        self.views[index]
            .host
            .invoke(generation, &key(component))
            .expect("button invocable");
        self.pump(index);
    }

    /// Newest `Result` on the view's window.
    fn last_result(&self, index: usize) -> Option<(bool, Option<Value>)> {
        let window = self.views[index].window;
        self.driver
            .events_for(window)
            .iter()
            .rev()
            .find_map(|e| match e {
                Event::Result { ok, data, .. } => Some((*ok, data.clone())),
                _ => None,
            })
    }
}

// ── AUTX-MOUNT-REGISTER ──────────────────────────────────────────────

#[test]
fn autx_visual_opens_four_surfaces_with_published_states() {
    let mut app = app();
    let mut view_ids = Vec::new();
    for surface in SURFACES {
        view_ids.push(open_surface(&mut app, surface));
    }
    // Distinct minted ids, all `view-{n}`.
    let unique: std::collections::BTreeSet<_> = view_ids.iter().collect();
    assert_eq!(unique.len(), 4);
    for view_id in &view_ids {
        assert!(view_id.starts_with("view-"));
    }

    let cell = app.automation_screen_cell();
    assert_eq!(cell.borrow().len(), 4);
    for (index, view_id) in view_ids.iter().enumerate() {
        let inst = parse_view(view_id);
        let publish = {
            let screens = cell.borrow();
            screens.get(&inst).expect("published state").clone()
        };
        let expected = matches!(
            (SURFACES[index], &publish.state),
            ("action-palette", AutomationScreenState::Palette(_))
                | ("macro-browser", AutomationScreenState::Macro(_))
                | ("workflow-editor", AutomationScreenState::Workflow(_))
                | ("run-monitor", AutomationScreenState::Run(_))
        );
        assert!(expected, "{view_id} publishes its own surface state");
        // Every published state builds a spec — no fabricated mounts.
        let _spec = automation_screen_spec(&publish.state);
    }
}

#[test]
fn autx_visual_close_drops_published_state() {
    let mut app = app();
    let view_id = open_surface(&mut app, "action-palette");
    let cell = app.automation_screen_cell();
    assert_eq!(cell.borrow().len(), 1);
    app.op_autx_visual(&json!({ "action": "close", "view_id": view_id }))
        .expect("close ok");
    // Direct op calls bypass the service tail's publish pass — taking
    // the cell republishes, and the closed view's entry is gone rather
    // than a stale spec.
    let cell = app.automation_screen_cell();
    assert!(cell.borrow().is_empty());
}

#[test]
fn autx_visual_rejects_unknown_surface_and_foreign_view() {
    let mut app = app();
    assert!(
        app.op_autx_visual(&json!({ "action": "open", "surface": "mixer" }))
            .is_err()
    );
    // `close` on a non-automation view refuses — composition-root
    // views stay unreachable through the autx lane.
    let err = app
        .op_autx_visual(&json!({ "action": "close", "view_id": "view-1" }))
        .expect_err("main view is not an autx surface");
    assert_eq!(err.code, "invalid_params");
}

// ── AUTX-LOCAL-ONLY ──────────────────────────────────────────────────

#[test]
fn autx_local_select_mutates_only_view_state() {
    let mut app = app();
    let view_id = open_surface(&mut app, "action-palette");
    let revision_before = app.workspace().session().revision();

    let result = app
        .op_autx_action(&json!({
            "view_id": view_id,
            "target": "autx.palette",
            "action": "select",
            "params": { "row": 0 }
        }))
        .expect("select ok");
    assert_eq!(result["kind"], json!("view-local"));
    assert_eq!(result["state"]["palette_selected"], json!(0));
    // Project/authority truth untouched — no revision bump, no fake
    // catalog fabricated.
    assert_eq!(app.workspace().session().revision(), revision_before);
    assert_eq!(result["requests"], json!([]));

    // Category select re-bases the row selection.
    app.op_autx_action(&json!({
        "view_id": view_id,
        "target": "autx.palette",
        "action": "category",
        "params": { "index": 1 }
    }))
    .expect("category ok");
    let state = app
        .workspace()
        .state(parse_view(&view_id))
        .expect("view state")
        .automation
        .clone()
        .expect("automation state");
    assert_eq!(state.palette_category, Some(1));
    assert_eq!(state.palette_selected, None);
}

// ── AUTX-NOCATALOG-REJECT ────────────────────────────────────────────

#[test]
fn autx_request_without_catalog_rejects_typed() {
    let mut app = app();
    let view_id = open_surface(&mut app, "action-palette");
    let err = app
        .op_autx_action(&json!({
            "view_id": view_id,
            "target": "autx.action",
            "action": "submit",
            "params": {
                "capability": "synvelis.transport.set-position@1.0",
                "args": { "position": 0 }
            }
        }))
        .expect_err("no catalog → typed rejection");
    assert_eq!(err.code, "unavailable");
    // A view-local action still mutates state without a catalog.
    app.op_autx_action(&json!({
        "view_id": view_id,
        "target": "autx.palette",
        "action": "select",
        "params": { "row": 0 }
    }))
    .expect("view-local unaffected");
}

// ── AUTX-STRICT-REJECT ───────────────────────────────────────────────

#[test]
fn decode_view_command_admits_autx_params_and_rejects_domain_params() {
    // Params-bearing autx intents decode — admission is not acceptance;
    // `decode_automation_command` re-validates at dispatch.
    let cmd = decode_view_command(&json!({
        "command_id": "c1",
        "expect_receipt": true,
        "intent": {
            "kind": "action",
            "target": "autx.palette",
            "control_id": "autx.palette.table",
            "sequence": 0,
            "action": "select",
            "params": { "row": 2 }
        }
    }))
    .expect("autx params admitted");
    assert_eq!(cmd.action_params, Some(json!({ "row": 2 })));

    // A domain action carrying params is not representable.
    assert!(
        decode_view_command(&json!({
            "command_id": "c2",
            "expect_receipt": true,
            "intent": {
                "kind": "action",
                "target": "syn.control.gain",
                "control_id": "gain",
                "sequence": 0,
                "action": "set",
                "params": { "row": 2 }
            }
        }))
        .is_err()
    );
}

#[test]
fn autx_action_rejects_unknown_target_action_and_malformed_params() {
    let mut app = app();
    let view_id = open_surface(&mut app, "action-palette");
    let mut dispatch = |target: &str, action: &str, params: Value| {
        app.op_autx_action(&json!({
            "view_id": view_id,
            "target": target,
            "action": action,
            "params": params
        }))
        .expect_err("must reject")
        .code
    };
    assert_eq!(
        dispatch("autx.palette", "explode", json!({})),
        "invalid_params"
    );
    assert_eq!(
        dispatch("autx.palette", "select", json!({})),
        "invalid_params"
    );
    assert_eq!(
        dispatch("autx.palette", "select", json!({ "row": "x" })),
        "invalid_params"
    );
    // Malformed explicit target rejects, never drops to "no target".
    assert_eq!(
        dispatch(
            "autx.action",
            "submit",
            json!({
                "capability": "synvelis.transport.set-position@1.0",
                "target": { "shape": "bogus" }
            })
        ),
        "invalid_params"
    );
    // A non-automation view_id refuses before decode.
    let err = app
        .op_autx_action(&json!({
            "view_id": "view-1",
            "target": "autx.palette",
            "action": "select",
            "params": { "row": 0 }
        }))
        .expect_err("main view is not an autx surface");
    assert_eq!(err.code, "invalid_params");
}

// ── AUTX-CATALOG-LOWER ───────────────────────────────────────────────

#[test]
fn autx_catalog_push_enables_request_lowering() {
    let mut app = app();
    let view_id = open_surface(&mut app, "action-palette");
    let pushed = app
        .op_autx_catalog(&catalog_doc())
        .expect("catalog push ok");
    assert_eq!(pushed["seq"], json!(3));
    assert_eq!(pushed["capabilities"], json!(2));
    assert_eq!(pushed["macros"], json!(1));

    // Palette row for `set-position` — resolve its display index from
    // the published state rather than assuming push order.
    let cell = app.automation_screen_cell();
    let inst = parse_view(&view_id);
    let row = {
        let screens = cell.borrow();
        let publish = screens.get(&inst).expect("palette publish");
        match &publish.state {
            AutomationScreenState::Palette(s) => s
                .catalog
                .as_ref()
                .expect("catalog projected")
                .rows
                .iter()
                .position(|r| r.id == "synvelis.transport.set-position")
                .expect("set-position row"),
            _ => panic!("palette surface"),
        }
    };
    app.op_autx_action(&json!({
        "view_id": view_id,
        "target": "autx.palette",
        "action": "select",
        "params": { "row": row }
    }))
    .expect("select ok");

    // Submit lowers to a wire request document — consistency pinned at
    // the live revision for mutations.
    let submitted = app
        .op_autx_action(&json!({
            "view_id": view_id,
            "target": "autx.action",
            "action": "submit",
            "params": {
                "capability": "synvelis.transport.set-position@1.0",
                "args": { "position": 120 }
            }
        }))
        .expect("submit lowers");
    assert_eq!(submitted["kind"], json!("request"));
    let requests = submitted["requests"].as_array().expect("requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0]["capability"]["id"],
        json!("synvelis.transport.set-position")
    );
    assert_eq!(
        requests[0]["consistency"]["kind"],
        json!("expected-revision")
    );
    assert!(
        requests[0]["request_id"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
    );

    // Preview reads `latest` — dry-run never pins a revision.
    let previewed = app
        .op_autx_action(&json!({
            "view_id": view_id,
            "target": "autx.action",
            "action": "preview",
            "params": {
                "capability": "synvelis.transport.set-position@1.0",
                "args": { "position": 120 }
            }
        }))
        .expect("preview lowers");
    assert_eq!(
        previewed["requests"][0]["consistency"]["kind"],
        json!("latest")
    );
}

#[test]
fn autx_macro_select_apply_lowers_per_step_requests() {
    let mut app = app();
    app.op_autx_catalog(&catalog_doc()).expect("catalog");
    // Macro select resolves the catalog row to the stable preset id;
    // apply lowers one request per step with `<key>/<index>` suffixes.
    let macro_view = open_surface(&mut app, "macro-browser");
    app.op_autx_action(&json!({
        "view_id": macro_view,
        "target": "autx.macro",
        "action": "select",
        "params": { "row": 0 }
    }))
    .expect("macro select ok");
    let state = app
        .workspace()
        .state(parse_view(&macro_view))
        .expect("view state")
        .automation
        .clone()
        .expect("automation state");
    assert_eq!(
        state.macro_selected.as_deref(),
        Some("preset.transport-home")
    );

    let applied = app
        .op_autx_action(&json!({
            "view_id": macro_view,
            "target": "autx.macro",
            "action": "apply",
            "params": {
                "preset": "preset.transport-home",
                "idempotency_key": "apply-9"
            }
        }))
        .expect("macro apply lowers");
    let requests = applied["requests"].as_array().expect("requests");
    assert_eq!(requests.len(), 2);
    assert_eq!(requests[0]["idempotency_key"], json!("apply-9/0"));
    assert_eq!(requests[1]["idempotency_key"], json!("apply-9/1"));
    assert!(
        requests[0]["request_id"]
            .as_str()
            .is_some_and(|id| id.ends_with("/0"))
    );

    let view_id = open_surface(&mut app, "action-palette");
    // A malformed catalog push rejects whole and leaves the previous
    // catalog serving requests.
    assert!(
        app.op_autx_catalog(&json!({ "seq": 4, "bogus": true }))
            .is_err()
    );
    app.op_autx_action(&json!({
        "view_id": view_id,
        "target": "autx.action",
        "action": "submit",
        "params": {
            "capability": "synvelis.transport.set-position@1.0",
            "args": {}
        }
    }))
    .expect("previous catalog still serves");
}

#[test]
fn autx_run_select_and_cancel_reject_without_monitor_data() {
    let mut app = app();
    app.op_autx_catalog(&catalog_doc()).expect("catalog");
    let view_id = open_surface(&mut app, "run-monitor");
    // No run-monitor producer exists yet (#264) — selection cannot
    // resolve a run id, so it rejects typed rather than storing a
    // dangling index; cancel has nothing to resolve.
    let err = app
        .op_autx_action(&json!({
            "view_id": view_id,
            "target": "autx.run",
            "action": "select",
            "params": { "row": 0 }
        }))
        .expect_err("no monitor projection");
    assert_eq!(err.code, "unavailable");
    let err = app
        .op_autx_action(&json!({
            "view_id": view_id,
            "target": "autx.run",
            "action": "cancel",
            "params": {}
        }))
        .expect_err("no selection to cancel");
    assert_eq!(err.code, "invalid_params");
}

// ── AUTX-DISPATCH-OWN (mounted lane) ─────────────────────────────────

#[test]
fn mounted_palette_select_and_submit_dispatch_through_shell() {
    let mut f = mounted_fixture(true);
    let palette = surface_index("action-palette");

    // The mounted palette projects the pushed catalog — find the
    // `set-position` row in the live spec state.
    let row = {
        let publish = f.cell.borrow();
        let view = &f.views[palette];
        match &publish.get(&view.inst).expect("publish").state {
            AutomationScreenState::Palette(s) => s
                .catalog
                .as_ref()
                .expect("catalog")
                .rows
                .iter()
                .position(|r| r.id == "synvelis.transport.set-position")
                .expect("row"),
            _ => panic!("palette"),
        }
    };

    // Table select → `autx.palette/select` → `syn.autx.action` →
    // view-local mutation; the op's Result settles on this window.
    f.select_row(palette, "autx.palette.table", row);
    let (ok, data) = f.last_result(palette).expect("select result");
    assert!(ok, "palette select accepted: {data:?}");
    assert_eq!(
        data.as_ref().map(|d| &d["kind"]),
        Some(&json!("view-local"))
    );
    assert_eq!(
        data.as_ref()
            .and_then(|d| d["state"]["palette_selected"].as_u64()),
        Some(row as u64)
    );

    // Re-publish so the spec gains the executable row's action buttons.
    f.push_state(palette);
    // Button press → `autx.action/submit` → lowered request documents
    // ride the Result back to the originating window.
    f.press(palette, "autx.action.submit");
    let (ok, data) = f.last_result(palette).expect("submit result");
    assert!(ok, "submit lowers: {data:?}");
    let data = data.expect("result data");
    assert_eq!(data["kind"], json!("request"));
    let requests = data["requests"].as_array().expect("requests");
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0]["capability"]["id"],
        json!("synvelis.transport.set-position")
    );
}

#[test]
fn mounted_unknown_autx_action_rejects_on_originating_channel() {
    let mut f = mounted_fixture(true);
    let palette = surface_index("action-palette");
    // A command the strict decode table does not know rejects typed —
    // the rejection lands on the same window the intent arrived on.
    let window = f.views[palette].window;
    action(
        &mut f.shell,
        &mut f.driver,
        window,
        ruviform_ui::ipc::Intent::new(
            "bad-1",
            "rvf.ui.command.v1",
            json!({
                "command_id": "bad-1",
                "expect_receipt": true,
                "intent": {
                    "kind": "action",
                    "target": "autx.palette",
                    "control_id": "autx.palette.table",
                    "sequence": 0,
                    "action": "explode",
                    "params": {}
                }
            }),
        ),
    );
    let results = results_for(&f.driver, window, "bad-1");
    assert_eq!(results.len(), 1);
    let (ok, _) = &results[0];
    assert!(!ok, "unknown autx action must reject");
    f.deliver(palette);
}
