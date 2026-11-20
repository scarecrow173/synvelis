//! `uitest` — the opt-in `rvf.uitest.v1` control seam for
//! `synvelis-native` (Issue #600 stage 1 slice 1; Ruviform's
//! RF-GT-03/RF-GT-04 port, ruviform#313/#314).
//!
//! External fixtures drive the product's real `syn.*` op surface over
//! the same protocol Ruviform's windowed apps-under-test speak:
//! `test.hello` → `test.fixture.prepare{ipc_emit}` injects raw
//! `rvf.ipc.v1` bodies at the inbound boundary → `test.observe.*`
//! returns journaled evidence → `test.shutdown` tears the session
//! down. The seam exists only under the `uitest` feature, and even
//! there stays inert until `RVF_TEST_CONTROL` selects a transport —
//! [`opted_in`] is the process-boundary gate `synvelis-native::main`
//! checks before composing; `serve_from_env` re-resolves the same
//! `RVF_TEST_*` environment for the actual serve path.
//!
//! [`SynvelisTestApp`] is the app-under-test half: the native run
//! loop's wiring expressed as [`TestApp`] — first-`Ready` view
//! attach, shared screen-state pushes, [`NativeShell`] event routing —
//! so externally injected events take the identical
//! `DriverEvent`/`WindowLifecycle::Action` → `AppService` path the
//! normal loop applies. `GpuiDriver::update_view` reaches the adapter
//! as an injected closure ([`SpecPush`]), keeping this module
//! driver-neutral and GPUI-free.
//!
//! **RT contract:** non-RT only — the control port is test plumbing,
//! never a live audio path.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::view::ViewSpec;
use ruviform_ui::{
    DriverEvent, ErrorBody, UiError, WindowDriver, WindowId, WindowLifecycle, codes,
};
use ruviform_ui_test::transport::AgentTransport;
use ruviform_ui_test::{
    FixtureController, FixtureReceipt, FixtureSpec, ShutdownMode, TestApp, TestIo,
};
use serde_json::{Value, json};

use crate::automation::AutomationScreenPublish;
use crate::screens::{MainScreenState, PendingMount, VcsScreenState, log_screen_state_update};
use crate::shell::NativeShell;

/// Whether the `rvf.uitest.v1` port should serve — checked once at
/// the process boundary before any composition work. A truthy
/// `RVF_TEST_DISABLE` beats every transport selection; an unset or
/// `off` `RVF_TEST_CONTROL` leaves the binary on its normal path.
/// Transport/auth details (loopback peer, run secret) stay with
/// `serve_from_env` — this gate only decides "serve at all".
#[must_use]
pub fn opted_in(disabled: bool, transport: &AgentTransport) -> bool {
    !disabled && !matches!(transport, AgentTransport::Off)
}

/// How a mounted view's rebuilt spec is pushed after a screen-state
/// change — the composition root injects `GpuiDriver::update_view`
/// (or a recorder in tests) so the adapter stays driver-neutral.
pub type SpecPush = Box<dyn FnMut(WindowId, ViewSpec) -> Result<(), UiError>>;

/// The app-under-test half of the seam — `synvelis-native`'s run loop
/// as a [`TestApp`]: [`NativeShell::handle`] consumes every event
/// (the same `syn.*` dispatch/`post_event` path as the normal loop),
/// each mount attaches its view on its window's first `Ready`
/// (after `handle` registers the window — owner resolution requires
/// it), and every shared screen-state change pushes a rebuilt spec
/// to every attached mount.
pub struct SynvelisTestApp {
    shell: NativeShell,
    mounts: Vec<PendingMount>,
    screen_state: Rc<RefCell<MainScreenState>>,
    vcs_screens: Rc<RefCell<BTreeMap<ViewInstanceId, VcsScreenState>>>,
    autx_screens: Rc<RefCell<BTreeMap<ViewInstanceId, AutomationScreenPublish>>>,
    applied_state: MainScreenState,
    applied_vcs: BTreeMap<ViewInstanceId, VcsScreenState>,
    applied_autx: BTreeMap<ViewInstanceId, u64>,
    push_spec: SpecPush,
}

impl SynvelisTestApp {
    /// Composes the AUT from the same parts `synvelis-native` hands
    /// its run loop — the sealed shell plus one mount record per
    /// reserved window.
    #[must_use]
    pub fn new(
        shell: NativeShell,
        mounts: Vec<PendingMount>,
        screen_state: Rc<RefCell<MainScreenState>>,
        vcs_screens: Rc<RefCell<BTreeMap<ViewInstanceId, VcsScreenState>>>,
        autx_screens: Rc<RefCell<BTreeMap<ViewInstanceId, AutomationScreenPublish>>>,
        push_spec: SpecPush,
    ) -> Self {
        // Automation publishes carry no `PartialEq` — the rev stamp
        // is the change signal; a stamp move means "republished after
        // an op", and the pushed spec is rebuilt either way.
        let applied_state = screen_state.borrow().clone();
        let applied_vcs = vcs_screens.borrow().clone();
        Self {
            applied_state,
            applied_vcs,
            applied_autx: BTreeMap::new(),
            shell,
            mounts,
            screen_state,
            vcs_screens,
            autx_screens,
            push_spec,
        }
    }
}

impl TestApp for SynvelisTestApp {
    fn on_event(&mut self, dev: DriverEvent, drv: &mut dyn WindowDriver) {
        // First `Ready` per mount is detected before `handle` consumes
        // the event — `attach_view` needs the window registered in the
        // shell's `windows` map, which `handle` performs.
        let ready: Vec<usize> = self
            .mounts
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                !m.attached
                    && matches!(
                        &dev,
                        DriverEvent::Windowed {
                            window: w,
                            event: WindowLifecycle::Ready,
                        } if *w == m.window
                    )
            })
            .map(|(i, _)| i)
            .collect();
        self.shell.handle(dev, drv);
        for i in ready {
            let window = {
                let m = &mut self.mounts[i];
                m.attached = true;
                if let Err(e) = self
                    .shell
                    .attach_view(drv, m.window, &m.view_id, &m.targets)
                {
                    eprintln!("synvelis-native: view attach failed: {e}");
                    tracing::error!(error = %e, view_id = %m.view_id, "view attach failed");
                } else {
                    tracing::info!(view_id = %m.view_id, window = %m.window, "view attached");
                }
                m.window
            };
            // A window that attached after the state cell moved must
            // not keep its mount-time spec — push the current one.
            let state = self.screen_state.borrow().clone();
            let vcs_state = self.vcs_screens.borrow().clone();
            let autx_state = self.autx_screens.borrow().clone();
            if let Some(spec) = self.mounts[i].spec(&state, &vcs_state, &autx_state)
                && let Err(e) = (self.push_spec)(window, spec)
            {
                tracing::error!(error = %e, "screen state update failed");
            }
        }
        let state = self.screen_state.borrow().clone();
        let vcs_state = self.vcs_screens.borrow().clone();
        let autx_state = self.autx_screens.borrow().clone();
        let autx_revs: BTreeMap<ViewInstanceId, u64> = autx_state
            .iter()
            .map(|(inst, publish)| (*inst, publish.rev))
            .collect();
        if state != self.applied_state
            || vcs_state != self.applied_vcs
            || autx_revs != self.applied_autx
        {
            self.applied_state = state.clone();
            self.applied_vcs = vcs_state.clone();
            self.applied_autx = autx_revs;
            let mut pushed = false;
            for m in self.mounts.iter().filter(|m| m.attached) {
                if let Some(spec) = m.spec(&state, &vcs_state, &autx_state) {
                    match (self.push_spec)(m.window, spec) {
                        Ok(()) => pushed = true,
                        Err(e) => tracing::error!(error = %e, "screen state update failed"),
                    }
                }
            }
            if pushed {
                log_screen_state_update(&state);
            }
        }
    }

    /// Cleanup accounting reported verbatim as `CleanupReport::app`.
    fn shutdown(&mut self, _mode: ShutdownMode, drv: &mut dyn WindowDriver) -> Value {
        json!({
            "mounts": self.mounts.len(),
            "mounts_attached": self.mounts.iter().filter(|m| m.attached).count(),
            "windows_open": drv.windows().len(),
        })
    }
}

/// The `ipc_emit` fixture — the `test.fixture.prepare` target that
/// injects one raw `rvf.ipc.v1` body into `params.window`'s inbound
/// channel. Decode happens at the backend's inbound boundary
/// ([`TestIo::emit_ipc_body`]): a valid body becomes a
/// `WindowLifecycle::Action` on the normal event path — how external
/// fixtures reach `syn.*` ops — an armed `ipc_disconnect` fault
/// suppresses it pre-decode, and a malformed body is answered
/// driver-side with `Event::error`, never reaching the app. Params
/// shape follows Ruviform's `windowed_test_app` precedent:
/// `{ "window": <id>, "body": "<json>" }`.
pub struct IpcEmitFixture;

impl FixtureController for IpcEmitFixture {
    fn name(&self) -> &'static str {
        "ipc_emit"
    }

    fn prepare(
        &mut self,
        spec: &FixtureSpec,
        io: &mut dyn TestIo,
    ) -> Result<FixtureReceipt, ErrorBody> {
        let Some(raw) = spec.params.get("window").and_then(Value::as_u64) else {
            return Err(ErrorBody::new(
                codes::INVALID_PARAMS,
                "ipc_emit requires a \"window\" param",
            ));
        };
        let Some(window) = WindowId::new(raw) else {
            return Err(ErrorBody::new(
                codes::INVALID_PARAMS,
                "window must be nonzero",
            ));
        };
        let body = spec
            .params
            .get("body")
            .and_then(Value::as_str)
            .unwrap_or("{}")
            .to_string();
        io.emit_ipc_body(window, &body);
        Ok(FixtureReceipt {
            name: spec.name.clone(),
            applied: json!({ "window": raw }),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;
    use std::time::Duration;

    use ruviform_ui::ipc::Event;
    use ruviform_ui::view::ViewHost;
    use ruviform_ui::{BackendReport, WindowSpec};
    use ruviform_ui_test::transport::{FrameRead, read_frame, write_frame};
    use ruviform_ui_test::{
        AgentConfig, AppMeta, FakeDriver, HeadlessIo, ServeOutcome, TestAgent, serve, test_codes,
    };

    use super::*;
    use crate::{MountSpec, SynvelisApp, main_view_id, main_view_targets, mount_main_view};

    /// A `SynvelisTestApp` over the same composition `synvelis-native`
    /// builds — sealed [`NativeShell`], mounted main view, shared
    /// cells — with `pushes` recording every spec the adapter pushes.
    /// The returned host keeps the mounted view alive for the test's
    /// scope; the real composition hands it to the driver through
    /// `create_view_window`.
    fn test_app(
        mounts: Vec<PendingMount>,
        pushes: &Rc<RefCell<Vec<(WindowId, ViewSpec)>>>,
    ) -> (SynvelisTestApp, ViewHost) {
        let app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
        let screen_state = app.main_screen_cell();
        let vcs_screens = app.vcs_screen_cell();
        let autx_screens = app.automation_screen_cell();
        let host = mount_main_view(&app.attach_epoch_allocator(), &screen_state.borrow())
            .expect("main view mounts");
        let shell = NativeShell::new(app, Duration::from_millis(16)).expect("compose shell");
        let recorder = Rc::clone(pushes);
        (
            SynvelisTestApp::new(
                shell,
                mounts,
                screen_state,
                vcs_screens,
                autx_screens,
                Box::new(move |window, spec| {
                    recorder.borrow_mut().push((window, spec));
                    Ok(())
                }),
            ),
            host,
        )
    }

    fn intent_body(id: &str, op: &str, params: &Value) -> String {
        json!({ "v": 1, "kind": "intent", "id": id, "op": op, "params": params }).to_string()
    }

    fn read_all_frames(mut cursor: Cursor<Vec<u8>>) -> Vec<Value> {
        let mut out = Vec::new();
        loop {
            match read_frame(&mut cursor).expect("frame read") {
                FrameRead::Frame(body) => {
                    out.push(serde_json::from_str::<Value>(&body).expect("event json"));
                }
                FrameRead::Eof => return out,
                FrameRead::TooLarge { declared } => {
                    panic!("control reply frame {declared} exceeded the cap")
                }
                other => panic!("unexpected frame outcome: {other:?}"),
            }
        }
    }

    /// `RVF_TEST_DISABLE` beats every transport selection; an unset or
    /// `off` control leaves the normal path — fail-closed, never a
    /// fallback.
    #[test]
    fn opt_in_gate_truth_table() {
        let tcp = AgentTransport::Tcp {
            addr: "127.0.0.1:9".to_string(),
        };
        for transport in [AgentTransport::Off, AgentTransport::Stdio, tcp.clone()] {
            assert!(
                !opted_in(true, &transport),
                "disabled must win: {transport:?}"
            );
        }
        assert!(!opted_in(false, &AgentTransport::Off));
        assert!(opted_in(false, &AgentTransport::Stdio));
        assert!(opted_in(false, &tcp));
    }

    /// Ops before `test.hello` are refused `unauthenticated` — the
    /// protocol's auth contract, exercised at the `TestAgent` level.
    #[test]
    fn ops_before_hello_are_unauthenticated() {
        let pushes = Rc::new(RefCell::new(Vec::new()));
        let (app, _host) = test_app(Vec::new(), &pushes);
        let io: &mut dyn TestIo = &mut HeadlessIo::new(app);
        let mut agent = TestAgent::new(
            AgentConfig::new(AppMeta {
                name: "synvelis".to_string(),
                version: "test".to_string(),
                revision: None,
            }),
            None,
        );
        let events = agent.handle(io, &intent_body("x", "test.observe.windows", &json!({})));
        let [
            Event::Result {
                ok: false, error, ..
            },
        ] = events.as_slice()
        else {
            panic!("expected a single unauthenticated result, got {events:?}");
        };
        assert_eq!(
            error.as_ref().map(|e| e.code.as_str()),
            Some(test_codes::UNAUTHENTICATED)
        );
    }

    /// A disabled agent refuses `serve` outright — the same
    /// "disabled beats opt-in" semantic `serve_from_env` applies when
    /// `RVF_TEST_DISABLE` is set (the env read itself is upstream's).
    #[test]
    fn serve_refuses_when_agent_disabled() {
        let pushes = Rc::new(RefCell::new(Vec::new()));
        let (app, _host) = test_app(Vec::new(), &pushes);
        let mut io = HeadlessIo::new(app);
        let mut agent = TestAgent::new(
            AgentConfig::new(AppMeta {
                name: "synvelis".to_string(),
                version: "test".to_string(),
                revision: None,
            }),
            None,
        );
        agent.set_enabled(false);
        let mut out = Vec::new();
        let outcome = serve(&mut agent, &mut io, Cursor::new(Vec::new()), &mut out);
        assert!(matches!(outcome, ServeOutcome::Disabled { .. }));
        assert_eq!(out, [] as [u8; 0]);
    }

    /// The end-to-end seam over the real composition: framed
    /// `test.hello` → `ipc_emit` injecting `syn.app.info` → receipts +
    /// the app's own result event → `test.shutdown`. Headless `serve`
    /// drives the identical `TestIo`/`TestApp` wiring `WindowedIo`
    /// uses on the GPUI lane.
    #[test]
    fn ipc_emit_drives_syn_op_end_to_end() {
        let pushes = Rc::new(RefCell::new(Vec::new()));
        let (app, _host) = test_app(Vec::new(), &pushes);
        let mut io = HeadlessIo::new(app);
        io.add_fixture(Box::new(IpcEmitFixture));
        let window = io
            .create_window(&WindowSpec {
                title: "Synvelis".to_string(),
                ..WindowSpec::default()
            })
            .expect("window");
        // `FakeDriver` buffers `post_event` until `Ready` — deliver it
        // so the op's result reaches `delivered` like a real driver.
        io.emit(DriverEvent::Windowed {
            window,
            event: WindowLifecycle::Ready,
        });

        let syn_intent = intent_body("i1", "syn.app.info", &json!({}));
        let frames_in = [
            intent_body("h", "test.hello", &json!({ "run_id": "run-uitest" })),
            intent_body(
                "f",
                "test.fixture.prepare",
                &json!({
                    "name": "ipc_emit",
                    "params": { "window": window.get(), "body": syn_intent },
                }),
            ),
            intent_body("r", "test.observe.receipts", &json!({})),
            intent_body("s", "test.shutdown", &json!({})),
        ];
        let mut input = Vec::new();
        for body in &frames_in {
            write_frame(&mut input, body).expect("frame write");
        }

        let mut agent = TestAgent::new(
            AgentConfig::new(AppMeta {
                name: "synvelis".to_string(),
                version: "test".to_string(),
                revision: None,
            }),
            None,
        );
        let mut out = Vec::new();
        let outcome = serve(&mut agent, &mut io, Cursor::new(input), &mut out);
        assert!(
            matches!(outcome, ServeOutcome::Shutdown { .. }),
            "expected clean shutdown, got {outcome:?}"
        );

        let replies = read_all_frames(Cursor::new(out));
        let results: Vec<&Value> = replies.iter().filter(|v| v["kind"] == "result").collect();
        assert_eq!(results.len(), 4, "one result per control op: {replies:?}");

        // hello: the real identity + capability claims.
        let welcome = &results[0];
        assert_eq!(welcome["id"], "h");
        assert_eq!(welcome["ok"], true);
        assert_eq!(welcome["data"]["app"]["name"], "synvelis");
        assert_eq!(welcome["data"]["v"], 1);
        assert_eq!(welcome["data"]["capabilities"]["fixture"], true);
        assert_eq!(welcome["data"]["backend"]["name"], "headless-fake");

        // fixture.prepare: the emit was applied on the named window.
        let receipt = &results[1];
        assert_eq!(receipt["id"], "f");
        assert_eq!(receipt["ok"], true);
        assert_eq!(receipt["data"]["applied"]["window"], window.get());

        // receipts: the raw body crossed `In` as a message and the
        // app's reply left `Out` as post_event — journaled, never
        // replayed.
        let page = &results[2]["data"];
        let entries = page["entries"].as_array().expect("receipt entries");
        assert!(
            entries.iter().any(|e| e["direction"] == "in"
                && e["kind"] == "message"
                && e["window"] == window.get()),
            "missing inbound message receipt: {entries:?}"
        );
        assert!(
            entries.iter().any(|e| e["direction"] == "out"
                && e["kind"] == "post_event"
                && e["window"] == window.get()),
            "missing outbound post_event receipt: {entries:?}"
        );

        // shutdown: graceful cleanup + the app's own report.
        let shutdown = &results[3];
        assert_eq!(shutdown["id"], "s");
        assert_eq!(shutdown["ok"], true);
        assert_eq!(shutdown["data"]["bye"], true);
        assert_eq!(shutdown["data"]["cleanup"]["windows_closed"], 1);
        assert_eq!(shutdown["data"]["cleanup"]["app"]["windows_open"], 1);
        assert_eq!(shutdown["data"]["cleanup"]["app"]["mounts"], 0);

        // The `syn.app.info` op really ran on the app's service path:
        // its typed Result was posted back to the window.
        let result = io
            .host()
            .events_for(window)
            .iter()
            .find_map(|e| match e {
                Event::Result {
                    id, ok: true, data, ..
                } if id == "i1" => data.as_ref(),
                _ => None,
            })
            .expect("syn.app.info result on the window");
        assert_eq!(result["name"], "synvelis");
    }

    /// A malformed raw body is answered driver-side with an error
    /// event — journaled, posted to the window, and never delivered
    /// to the app's consumer path.
    #[test]
    fn malformed_ipc_body_is_answered_driver_side() {
        let pushes = Rc::new(RefCell::new(Vec::new()));
        let (app, _host) = test_app(Vec::new(), &pushes);
        let mut io = HeadlessIo::new(app);
        let window = io
            .create_window(&WindowSpec {
                title: "Synvelis".to_string(),
                ..WindowSpec::default()
            })
            .expect("window");
        io.emit(DriverEvent::Windowed {
            window,
            event: WindowLifecycle::Ready,
        });
        let seen_before = io.host().events_for(window).len();

        io.emit_ipc_body(window, "this is not json");

        let new_events = &io.host().events_for(window)[seen_before..];
        let [Event::Error { error }] = new_events else {
            panic!("expected a single driver-side error event, got {new_events:?}");
        };
        assert_ne!(error.code, "");
        // The consumer path saw nothing — no dispatch ran, so no
        // screen-state push was provoked either.
        assert!(pushes.borrow().is_empty());
    }

    /// First `Ready` on a mount's window attaches the view and pushes
    /// the current screen spec — the run loop's mount semantics,
    /// unchanged under the seam.
    #[test]
    fn ready_attaches_view_and_pushes_spec() {
        let pushes = Rc::new(RefCell::new(Vec::new()));
        let mut driver = FakeDriver::new();
        let window = driver
            .create_window(&WindowSpec {
                title: "Synvelis".to_string(),
                ..WindowSpec::default()
            })
            .expect("window");
        let (mut app, _host) = test_app(
            vec![PendingMount {
                window,
                view_id: main_view_id(),
                targets: main_view_targets(),
                spec_kind: MountSpec::Main,
                attached: false,
            }],
            &pushes,
        );

        // Deliver through `pump` — not a bare `on_event` call — so the
        // fake's `Ready` bookkeeping (ready flag, backlog flush) runs
        // before delivery like a real driver's event loop.
        driver.inject(DriverEvent::Windowed {
            window,
            event: WindowLifecycle::Ready,
        });
        driver.pump(&mut |dev, drv| app.on_event(dev, drv));

        // The first push is the post-attach spec; `Ready`+attach
        // publishing may legitimately add a state-diff push on top —
        // the run loop's exact semantics, not a double-attach.
        assert!(!pushes.borrow().is_empty(), "post-attach spec push missing");
        assert_eq!(pushes.borrow()[0].0, window);
        // `syn.view.attach`/`syn.control.subscribe` results were
        // posted back to the window — attach ran the real op path.
        let attach_result = driver.events_for(window).iter().any(|e| {
            matches!(
                e,
                Event::Result { id, ok: true, .. }
                    if id == &format!("shell-attach-{}", main_view_id())
            )
        });
        assert!(attach_result, "view attach result missing");
    }
}
