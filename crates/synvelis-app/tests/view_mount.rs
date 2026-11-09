//! `ViewHost` mounted through the real `syn.*` runtime on `FakeDriver`
//! (SV-GPUI-01 view-mount slice, RF387×RF390).
//!
//! This is the headless half of the Fake↔GPUI semantic parity claim:
//! the same [`main_screen_spec`] + theme + fixture drives the mounted
//! `ViewHost` here exactly as `RvfView` drives it on the GPUI lane —
//! `dispatch_input` → `ViewEvent::Intent` → `rvf.ui.command.v1`
//! `Action` → shell → `syn.control.dispatch`; outbox receipts/
//! projections → translated `Result`/`Notify` → `apply_receipt`/
//! `apply_projection`. `deliver_to_host` mirrors `RvfView::drain_inbox`
//! decode verbatim so the two lanes exercise the *same wire*, not two
//! interpretations of it.
//!
//! Nothing here asserts OS/GPU/IME behavior — the real-window evidence
//! is a separate gate recorded in the task's PR (the binary
//! `synvelis-native` mounts this same spec through
//! `GpuiDriver::create_view_window`).

use std::time::Duration;

use serde_json::{Value, json};

use ruviform_ui::behavior::{
    CommandReceipt, ControlInput, InputSource, ReceiptStatus, Revision, StateProjection, ValueInput,
};
use ruviform_ui::ipc::{Event, Intent};
use ruviform_ui::view::{
    CellValue, ComponentKey, ComponentSpec, ProjectionOp, TableSpec, ViewEvent, ViewHost, ViewSpec,
};
use ruviform_ui::{
    BackendReport, DriverEvent, WindowDriver, WindowId, WindowLifecycle, WindowSpec,
};
use ruviform_ui_test::FakeDriver;

use synvelis_app::binding::project_action_op;
use synvelis_app::{
    MainScreenState, NativeShell, SynvelisApp, main_screen_spec, main_view_id, main_view_targets,
    mixer_gain_target, mount_main_view, seek_range_seconds,
};
use synvelis_project::{
    ClipExtent, ClipPlacement, ClipPlayback, CounterIdGenerator, DirectoryBundleStore,
    MusicalDuration, MusicalTick, PlaybackMapping, Project, ProjectController, ProjectId,
    ProjectOp, ProjectRevision, SourceFrame, SourceRange, TimeAnchor,
};
use synvelis_workspace::ProjectSession;

fn key(name: &str) -> ComponentKey {
    ComponentKey(name.to_string())
}

fn pump(shell: &mut NativeShell, driver: &mut FakeDriver) {
    driver.pump(&mut |ev, d| shell.handle(ev, d));
}

fn inject(shell: &mut NativeShell, driver: &mut FakeDriver, ev: DriverEvent) {
    driver.inject(ev);
    pump(shell, driver);
}

fn action(shell: &mut NativeShell, driver: &mut FakeDriver, window: WindowId, intent: Intent) {
    inject(
        shell,
        driver,
        DriverEvent::Windowed {
            window,
            event: WindowLifecycle::Action(intent),
        },
    );
}

/// Composes app → mounted view host → shell → fake window at `Ready`,
/// then `attach_view` + subscriptions — the exact composition-root
/// order `synvelis-native` uses (host minted before the app seals).
struct MountedFixture {
    shell: NativeShell,
    driver: FakeDriver,
    host: ViewHost,
    window: WindowId,
    view_id: String,
    /// The app's shared main-screen state cell — taken before the app
    /// sealed into the shell, exactly like `synvelis-native` does. The
    /// UI pump keeps it fresh; tests push it into the host through
    /// `update` the same way the driver's `update_view` does.
    screen: std::rc::Rc<std::cell::RefCell<MainScreenState>>,
    /// Cursor into `driver.events_for(window)` — events are an
    /// append-only log on the fake, so deliveries are consumed by
    /// index, mirroring `RvfView`'s drainable inbox.
    inbox_cursor: usize,
}

fn fixture() -> MountedFixture {
    let app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let screen = app.main_screen_cell();
    let host =
        mount_main_view(&app.attach_epoch_allocator(), &screen.borrow()).expect("main view mounts");
    let view_id = main_view_id();
    let mut shell = NativeShell::new(app, Duration::from_millis(16)).expect("compose shell");
    let mut driver = FakeDriver::new();
    let window = driver
        .create_window(&WindowSpec {
            title: "Synvelis".to_string(),
            ..WindowSpec::default()
        })
        .expect("view window");
    inject(
        &mut shell,
        &mut driver,
        DriverEvent::Windowed {
            window,
            event: WindowLifecycle::Ready,
        },
    );
    let attach = shell
        .attach_view(&mut driver, window, &view_id, &main_view_targets())
        .expect("attach_view ok");
    assert!(attach["generation"].as_u64().is_some_and(|g| g >= 1));
    MountedFixture {
        shell,
        driver,
        host,
        window,
        view_id,
        screen,
        inbox_cursor: 0,
    }
}

impl MountedFixture {
    /// Drains minted `ViewEvent::Intent`s into `rvf.ui.command.v1`
    /// `Action`s on the window channel — the same envelope `RvfView`
    /// mints for `IntentSink::send` (RF387).
    fn pump_view_intents(&mut self) {
        for event in self.host.drain_events() {
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
                action(
                    &mut self.shell,
                    &mut self.driver,
                    self.window,
                    Intent::new(command_id, "rvf.ui.command.v1", params),
                );
            }
        }
    }

    /// Applies every newly delivered window event to the host —
    /// `Result` → `CommandReceipt` (`data.status` decoded against the
    /// canonical vocabulary; bare `ok` is `Accepted`, never fake
    /// `Applied`), `Notify` → `StateProjection` — mirroring
    /// `RvfView::drain_inbox`'s decode.
    fn deliver_to_host(&mut self) {
        let events: Vec<Event> = self
            .driver
            .events_for(self.window)
            .into_iter()
            .skip(self.inbox_cursor)
            .cloned()
            .collect();
        self.inbox_cursor += events.len();
        for event in events {
            match event {
                Event::Result {
                    id,
                    ok,
                    data,
                    error,
                } => {
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
                    self.host.apply_receipt(&CommandReceipt {
                        command_id: id,
                        status,
                        revision,
                        detail,
                    });
                }
                Event::Notify { data, .. } => {
                    let Some(control_id) = data.get("control_id").and_then(Value::as_str) else {
                        continue;
                    };
                    let Some(revision) = data
                        .get("revision")
                        .and_then(|v| serde_json::from_value::<Revision>(v.clone()).ok())
                    else {
                        continue;
                    };
                    let provenance = data
                        .get("provenance")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                    self.host.apply_projection(
                        control_id,
                        &StateProjection {
                            revision,
                            value: data.get("value").cloned().unwrap_or(Value::Null),
                            provenance,
                        },
                    );
                }
                _ => {}
            }
        }
    }

    /// One control input end-to-end: dispatch into the host, mint
    /// intents onto the wire, then apply everything delivered back.
    fn input(&mut self, k: &str, input: ControlInput) {
        let generation = self.host.generation();
        self.host
            .dispatch_input(generation, &key(k), input)
            .expect("dispatch_input");
        self.pump_view_intents();
        self.deliver_to_host();
    }

    /// Presses the Button `k` — the same mint/deliver path as `input`.
    fn press(&mut self, k: &str) {
        let generation = self.host.generation();
        self.host
            .invoke(generation, &key(k))
            .expect("button invoke");
        self.pump_view_intents();
        self.deliver_to_host();
    }

    /// Pushes the app's current [`MainScreenState`] into the host —
    /// the headless mirror of `GpuiDriver::update_view` on the native
    /// lane (`view_driver.update_view(window, main_screen_spec(&state))`).
    fn push_screen_state(&mut self) -> Vec<ProjectionOp> {
        let state = self.screen.borrow().clone();
        let generation = self.host.generation();
        self.host
            .update(main_screen_spec(&state), generation)
            .expect("screen state update applies")
    }

    fn begin(id: &str) -> ControlInput {
        ControlInput::Begin {
            gesture_id: id.to_string(),
            source: InputSource::from("pointer"),
            origin_view: None,
            target: None,
            at: None,
            modifiers: None,
            seq: None,
        }
    }

    fn preview(id: &str, plain: f64) -> ControlInput {
        ControlInput::Preview {
            gesture_id: id.to_string(),
            value: ValueInput::Plain(plain),
            modifiers: None,
            seq: None,
        }
    }

    fn commit(id: &str) -> ControlInput {
        ControlInput::Commit {
            gesture_id: id.to_string(),
            value: None,
            modifiers: None,
            seq: None,
        }
    }

    fn cancel(id: &str) -> ControlInput {
        ControlInput::Cancel {
            gesture_id: id.to_string(),
            reason: None,
            seq: None,
        }
    }
}

/// `data` payloads of `notify` events delivered to `window` whose
/// `channel` matches — same shape as `tests/shell.rs`'s helper.
fn notifies(driver: &FakeDriver, window: WindowId, channel: &str) -> Vec<Value> {
    driver
        .events_for(window)
        .iter()
        .filter_map(|e| match e {
            Event::Notify { data, .. } if data["channel"] == channel => Some(data["data"].clone()),
            _ => None,
        })
        .collect()
}

fn results_for(driver: &FakeDriver, window: WindowId, id: &str) -> Vec<(bool, Option<Value>)> {
    driver
        .events_for(window)
        .iter()
        .filter_map(|e| match e {
            Event::Result {
                id: rid, ok, data, ..
            } if rid == id => Some((*ok, data.clone())),
            _ => None,
        })
        .collect()
}

#[test]
fn mount_attach_subscribe_and_projection_reaches_host() {
    let mut f = fixture();
    assert_eq!(f.shell.view_window(&f.view_id), Some(f.window));
    assert!(f.shell.is_ready(f.window));

    // The subscribe result is on the wire (minted by attach_view).
    let sub_results = results_for(&f.driver, f.window, "shell-sub-view-1-transport/position");
    assert_eq!(
        sub_results.len(),
        1,
        "one subscribe result: {sub_results:?}"
    );
    assert!(sub_results[0].0, "subscribe ok: {sub_results:?}");

    // The initial projection row is pushed by the subscribe pump —
    // the translated `Notify{control_id, revision, value}` must reach
    // the host's binding (`last_observed`); the controlled display is
    // asserted in `seek_display_follows_the_authoritative_projection_in_seconds`.
    let projections = notifies(&f.driver, f.window, "syn.control.projection");
    assert!(
        projections
            .iter()
            .any(|p| p["target"] == "transport/position"),
        "initial projection row pushed: {projections:?}"
    );
    f.deliver_to_host();
    let snap = f
        .host
        .observe(&key("seek.position"))
        .expect("seek snapshot");
    assert!(
        snap.observed.is_some(),
        "transport/position projection landed on the host binding: {snap:?}"
    );
    assert!(snap.mounted);
}

#[test]
fn gesture_begin_preview_commit_lands_applied_receipt_on_host() {
    let mut f = fixture();
    f.deliver_to_host();

    f.input("seek.position", MountedFixture::begin("g1"));
    f.input("seek.position", MountedFixture::preview("g1", 30.0));
    f.input("seek.position", MountedFixture::commit("g1"));

    // Host-side binding state proves the domain receipt landed:
    // commit minted under the host's epoch namespace must report
    // `applied` — the authoritative receipt, not the transport ack.
    let binding = f
        .host
        .binding("transport/position")
        .expect("binding for transport/position");
    let receipt = binding
        .state()
        .last_receipt
        .clone()
        .expect("a receipt landed");
    assert_eq!(
        receipt.status,
        ReceiptStatus::Applied,
        "commit receipt applied: {receipt:?}"
    );

    // The wire carried the same receipt as a translated Result whose
    // id is the binding-minted command_id.
    let cmd = receipt.command_id.clone();
    let results = results_for(&f.driver, f.window, &cmd);
    assert_eq!(results.len(), 1, "one translated Result: {results:?}");
    assert!(results[0].0);
    assert_eq!(results[0].1.as_ref().unwrap()["status"], "applied");

    // Raw receipt notify still broadcasts for ViewBinding consumers.
    let receipts = notifies(&f.driver, f.window, "syn.control.receipt");
    assert!(
        receipts.iter().any(|r| r["receipt"]["commandId"] == cmd),
        "raw receipt broadcast: {receipts:?}"
    );
}

/// The Seek's controlled display — the value the renderer reads — is
/// the authoritative transport position in seconds once the commit's
/// projection lands (8 s under the default 120 BPM map is beat 16).
#[test]
fn seek_display_follows_the_authoritative_projection_in_seconds() {
    let mut f = fixture();
    f.deliver_to_host();
    f.input("seek.position", MountedFixture::begin("gd"));
    f.input("seek.position", MountedFixture::preview("gd", 3.0));
    f.input("seek.position", MountedFixture::commit("gd"));
    f.deliver_to_host();

    let snap = f
        .host
        .observe(&key("seek.position"))
        .expect("seek snapshot");
    assert!(snap.observed.is_some(), "projection landed: {snap:?}");
    let display = snap.display.expect("controlled display");
    assert!((display.plain - 3.0).abs() < 1e-9, "display: {display:?}");
}

#[test]
fn cancel_gesture_reports_receipt_without_commit() {
    let mut f = fixture();
    f.deliver_to_host();
    f.input("seek.position", MountedFixture::begin("g2"));
    f.input("seek.position", MountedFixture::preview("g2", 10.0));
    f.input("seek.position", MountedFixture::cancel("g2"));

    let receipts = notifies(&f.driver, f.window, "syn.control.receipt");
    // The cancel reports its own receipt; the committed value stays
    // untouched (no commit receipt minted for g2).
    let cancel_cmds: Vec<&str> = receipts
        .iter()
        .filter_map(|r| r["receipt"]["commandId"].as_str())
        .collect();
    assert!(
        !cancel_cmds.is_empty(),
        "cancel produced a receipt: {receipts:?}"
    );
}

/// The Mixer `Fader` is a driven control (synvelis#185): its binding
/// target is the bootstrap master-strip `ParameterRole::Fader` id, the
/// subscribe resolves, the controlled display is the authoritative
/// `host_normalized` projection, and a drag rides the same
/// `Begin → Preview → Commit` wire path as the Seek.
///
/// Automation mode note: the session default is `Read`, under which a
/// commit returns to the recorded series without writing — the
/// `Off`-mode `SetParameterBase` write is covered headless in
/// `synvelis-workspace/tests/sv185_mixer_gain.rs`. Here the wire-level
/// contract is what matters: preview moves the controlled display and
/// commit lands an `Applied` receipt.
#[test]
fn mixer_fader_display_and_gesture_follow_the_domain_projection() {
    let mut f = fixture();
    let target = mixer_gain_target();

    // attach_view subscribed both driven targets — the fader's tagged
    // param id resolves against the bootstrap control model.
    let sub = results_for(&f.driver, f.window, &format!("shell-sub-view-1-{target}"));
    assert_eq!(sub.len(), 1, "one fader subscribe result: {sub:?}");
    assert!(sub[0].0, "fader subscribe ok: {sub:?}");

    // The initial projection row reaches the host binding; the
    // controlled display is the authoritative base (unity = 1.0).
    let projections = notifies(&f.driver, f.window, "syn.control.projection");
    assert!(
        projections.iter().any(|p| p["target"] == target),
        "fader projection row pushed: {projections:?}"
    );
    f.deliver_to_host();
    let snap = f.host.observe(&key("fader.gain")).expect("fader snapshot");
    assert!(snap.observed.is_some(), "fader projection landed: {snap:?}");
    let display = snap.display.expect("controlled display");
    assert!(
        (display.plain - 1.0).abs() < 1e-9,
        "authoritative unity display: {display:?}"
    );

    // Drag: begin → preview moves the controlled display (preview wins
    // over the staged external value), commit lands `Applied` on the
    // binding and the display returns to the authoritative base under
    // the default Read mode.
    f.input("fader.gain", MountedFixture::begin("gf"));
    f.input("fader.gain", MountedFixture::preview("gf", 0.4));
    let snap = f
        .host
        .observe(&key("fader.gain"))
        .expect("fader snapshot during preview");
    let display = snap.display.expect("controlled display during preview");
    assert!(
        (display.plain - 0.4).abs() < 1e-9,
        "preview drives the display: {display:?}"
    );

    f.input("fader.gain", MountedFixture::commit("gf"));
    let binding = f
        .host
        .binding(&target)
        .expect("binding for the fader target");
    let receipt = binding
        .state()
        .last_receipt
        .clone()
        .expect("a receipt landed");
    assert_eq!(
        receipt.status,
        ReceiptStatus::Applied,
        "fader commit receipt applied: {receipt:?}"
    );
    f.deliver_to_host();
    let snap = f
        .host
        .observe(&key("fader.gain"))
        .expect("fader snapshot after commit");
    let display = snap.display.expect("controlled display after commit");
    assert!(
        (display.plain - 1.0).abs() < 1e-9,
        "Read-mode commit returns to the authoritative base: {display:?}"
    );
}

/// A cancelled fader gesture drops the preview and restores the
/// authoritative display — nothing is written to the domain.
#[test]
fn mixer_fader_cancel_restores_the_authoritative_display() {
    let mut f = fixture();
    f.deliver_to_host();

    f.input("fader.gain", MountedFixture::begin("gc"));
    f.input("fader.gain", MountedFixture::preview("gc", 0.1));
    f.input("fader.gain", MountedFixture::cancel("gc"));
    f.deliver_to_host();

    let snap = f
        .host
        .observe(&key("fader.gain"))
        .expect("fader snapshot after cancel");
    let display = snap.display.expect("controlled display after cancel");
    assert!(
        (display.plain - 1.0).abs() < 1e-9,
        "cancel restores the authoritative base: {display:?}"
    );
}

#[test]
fn reattach_bumps_generation_and_fresh_host_absorbs_stale_namespace() {
    let mut f = fixture();

    // Reattach: the domain bumps the attach generation (the same
    // dock-transfer contract shell tests assert across windows).
    let first = f
        .shell
        .attach_view(&mut f.driver, f.window, &f.view_id, &main_view_targets())
        .expect("reattach ok");
    // NOTE: generation is bumped by the domain on each attach.

    // Host recreation: the same logical view, fresh epoch minted from
    // the *same* session allocator through the post-seal shell handle —
    // the mid-session recreation path the composition root must own.
    // Command-id namespaces embed `a{attach_epoch}:m{mount_epoch}` —
    // distinct across hosts, so stale receipts addressed to the dead
    // host cannot resolve on the live one.
    let mut dead = f.host;
    let mut fresh = mount_main_view(
        &f.shell.attach_epoch_allocator(),
        &MainScreenState::default(),
    )
    .expect("fresh host mounts");

    let g_dead = dead.generation();
    dead.dispatch_input(g_dead, &key("seek.position"), MountedFixture::begin("d1"))
        .expect("dead begin");
    let dead_ids: Vec<String> = dead
        .drain_events()
        .into_iter()
        .filter_map(|e| match e {
            ViewEvent::Intent { command_id, .. } => Some(command_id),
            _ => None,
        })
        .collect();

    let g_fresh = fresh.generation();
    fresh
        .dispatch_input(g_fresh, &key("seek.position"), MountedFixture::begin("f1"))
        .expect("fresh begin");
    let fresh_ids: Vec<String> = fresh
        .drain_events()
        .into_iter()
        .filter_map(|e| match e {
            ViewEvent::Intent { command_id, .. } => Some(command_id),
            _ => None,
        })
        .collect();

    assert!(!dead_ids.is_empty() && !fresh_ids.is_empty());
    for d in &dead_ids {
        assert!(
            !fresh_ids.contains(d),
            "fresh host never reuses a dead namespace id: {d}"
        );
    }
    let _ = first;
}

#[test]
fn save_and_reload_preserve_the_mounted_view() {
    let mut f = fixture();
    let dir = std::env::temp_dir().join(format!("sv175-mount-{}-p1", std::process::id()));
    let path = dir.to_string_lossy().to_string();
    let _ = std::fs::remove_dir_all(&dir);

    // Gesture → project.new → save → open — all through the wire,
    // while the view stays attached.
    f.input("seek.position", MountedFixture::begin("gs"));
    f.input("seek.position", MountedFixture::commit("gs"));

    action(
        &mut f.shell,
        &mut f.driver,
        f.window,
        Intent::new(
            "pn",
            "syn.project.new",
            json!({ "path": path, "name": "slice" }),
        ),
    );
    assert!(results_for(&f.driver, f.window, "pn").last().unwrap().0);

    action(
        &mut f.shell,
        &mut f.driver,
        f.window,
        Intent::new("ps", "syn.project.save", json!({})),
    );
    assert!(results_for(&f.driver, f.window, "ps").last().unwrap().0);

    action(
        &mut f.shell,
        &mut f.driver,
        f.window,
        Intent::new("po", "syn.project.open", json!({ "path": path })),
    );
    assert!(results_for(&f.driver, f.window, "po").last().unwrap().0);

    // The view attach/subscription registry is app-session state —
    // it survives the project reload, and a post-reload gesture still
    // lands applied through the same generation.
    assert_eq!(f.shell.view_window(&f.view_id), Some(f.window));
    f.input("seek.position", MountedFixture::begin("gr"));
    f.input("seek.position", MountedFixture::commit("gr"));
    let binding = f
        .host
        .binding("transport/position")
        .expect("binding survives reload");
    let receipt = binding.state().last_receipt.expect("post-reload receipt");
    assert_eq!(receipt.status, ReceiptStatus::Applied);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn destroying_the_view_window_runs_view_teardown() {
    let mut f = fixture();

    // A second live window keeps the loop up so post-teardown state
    // is still observable.
    let other = {
        let w = f.driver.create_window(&WindowSpec::default()).unwrap();
        inject(
            &mut f.shell,
            &mut f.driver,
            DriverEvent::Windowed {
                window: w,
                event: WindowLifecycle::Ready,
            },
        );
        w
    };

    inject(
        &mut f.shell,
        &mut f.driver,
        DriverEvent::Windowed {
            window: f.window,
            event: WindowLifecycle::CloseRequested,
        },
    );
    assert!(f.driver.destroyed().contains(&f.window));
    assert!(
        f.shell.view_window(&f.view_id).is_none(),
        "view bookkeeping cleared on destroy"
    );

    action(
        &mut f.shell,
        &mut f.driver,
        other,
        Intent::new("vl", "syn.view.list", json!({})),
    );
    let (ok, data) = results_for(&f.driver, other, "vl").last().cloned().unwrap();
    assert!(ok);
    assert_eq!(
        data.unwrap()["views"].as_array().unwrap().len(),
        0,
        "backend view registry torn down — subscription/gesture cancel replayed"
    );
}

/// `ok`/`data` of the newest `Result` the window received — the
/// answer to the most recent intent.
fn last_result(driver: &FakeDriver, window: WindowId) -> Option<(bool, Option<Value>)> {
    driver
        .events_for(window)
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Result { ok, data, .. } => Some((*ok, data.clone())),
            _ => None,
        })
}

/// `ok` of the newest `Result` the window received — the answer to the
/// most recent Button press.
fn last_result_ok(driver: &FakeDriver, window: WindowId) -> Option<bool> {
    driver
        .events_for(window)
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Result { ok, .. } => Some(*ok),
            _ => None,
        })
}

#[test]
fn seek_range_follows_project_length_in_place() {
    let mut f = fixture();
    let generation = f.host.generation();
    let longer = MainScreenState {
        project_length_seconds: 120.0,
        ..MainScreenState::default()
    };

    // The seek's descriptor changes in place: nothing mounts or unmounts,
    // the seek key is among the updated nodes, the host keeps its
    // generation (no remount), and the tree is the new spec.
    let ops = f
        .host
        .update(main_screen_spec(&longer), generation)
        .expect("length update applies");
    assert!(
        ops.iter()
            .all(|op| matches!(op, ProjectionOp::Updated { .. }))
    );
    assert!(ops.contains(&ProjectionOp::Updated {
        key: key("seek.position")
    }));
    assert_eq!(f.host.generation(), generation);
    assert_eq!(f.host.spec(), Some(&main_screen_spec(&longer)));

    // The control keeps working on the updated range.
    f.input("seek.position", MountedFixture::begin("gs"));
    f.input("seek.position", MountedFixture::commit("gs"));
    let receipt = f
        .host
        .binding("transport/position")
        .expect("binding survives the range update")
        .state()
        .last_receipt
        .expect("commit receipt");
    assert_eq!(receipt.status, ReceiptStatus::Applied);
}

#[test]
fn empty_project_keeps_the_default_seek_range() {
    assert_eq!(seek_range_seconds(0.0), 240.0);
    assert_eq!(seek_range_seconds(f64::INFINITY), 240.0);
    assert_eq!(seek_range_seconds(90.5), 90.5);
}

#[test]
fn project_buttons_save_and_reload_through_the_mounted_view() {
    let mut f = fixture();
    let dir = std::env::temp_dir().join(format!("sv175-buttons-{}", std::process::id()));
    let path = dir.to_string_lossy().to_string();
    let _ = std::fs::remove_dir_all(&dir);

    // No bundle yet: Save is answered as a failure, never applied.
    f.press("project.save");
    assert_eq!(last_result_ok(&f.driver, f.window), Some(false));

    action(
        &mut f.shell,
        &mut f.driver,
        f.window,
        Intent::new(
            "pn",
            "syn.project.new",
            json!({ "path": path, "name": "buttons" }),
        ),
    );
    assert!(results_for(&f.driver, f.window, "pn").last().unwrap().0);

    f.press("project.save");
    assert_eq!(last_result_ok(&f.driver, f.window), Some(true));

    // Reload reopens the bundle the session was created from.
    f.press("project.reload");
    assert_eq!(last_result_ok(&f.driver, f.window), Some(true));
    action(
        &mut f.shell,
        &mut f.driver,
        f.window,
        Intent::new("pi", "syn.project.info", json!({})),
    );
    let info = results_for(&f.driver, f.window, "pi")
        .last()
        .and_then(|(_, data)| data.clone())
        .expect("info after reload");
    assert_eq!(info["name"], "buttons");
    assert_eq!(info["saved"], true);

    assert_eq!(project_action_op("save"), Some("syn.project.save"));
    assert_eq!(project_action_op("reload"), Some("syn.project.reload"));
    assert_eq!(project_action_op("explode"), None);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn tab_press_switches_the_displayed_tab_through_the_shell() {
    let mut f = fixture();
    let generation = f.host.generation();
    let tabs = key("screen.tabs");

    // A pointer press on the Mixer header: the display moves at once and
    // the shell's view-state acknowledgement settles it as accepted.
    f.host.select_tab(generation, &tabs, 1).expect("tab press");
    f.pump_view_intents();
    f.deliver_to_host();

    assert_eq!(f.host.observe(&tabs).and_then(|s| s.selected), Some(1));
    assert_eq!(last_result_ok(&f.driver, f.window), Some(true));
}

// ---------------------------------------------------------------------
// SV-GUI-03 slice 1 (synvelis#112): Transport actions + Inspector display
// ---------------------------------------------------------------------

/// Finds a component by key anywhere in the spec tree — Panels recurse
/// through `children`, Tabs through each tab's `child`.
fn find_component<'a>(spec: &'a ComponentSpec, name: &str) -> Option<&'a ComponentSpec> {
    if spec.key().0 == name {
        return Some(spec);
    }
    match spec {
        ComponentSpec::Panel(p) => p.children.iter().find_map(|c| find_component(c, name)),
        ComponentSpec::Tabs(t) => t
            .tabs
            .iter()
            .find_map(|tab| find_component(&tab.child, name)),
        _ => None,
    }
}

/// The resolved `text` of the `Label` `name` in `spec`.
fn spec_label_text(spec: &ViewSpec, name: &str) -> String {
    match find_component(&spec.root, name) {
        Some(ComponentSpec::Label(l)) => l.text.clone(),
        other => panic!("label `{name}` missing or wrong kind: {other:?}"),
    }
}

/// The `Table` `name` in `spec`.
fn spec_table<'a>(spec: &'a ViewSpec, name: &str) -> &'a TableSpec {
    match find_component(&spec.root, name) {
        Some(ComponentSpec::Table(t)) => t,
        other => panic!("table `{name}` missing or wrong kind: {other:?}"),
    }
}

/// The mounted spec carries the Transport Play/Stop `Button`s on the
/// `transport` action target, the status `Label`, and the read-only
/// Inspector `Table` — stable component keys so `ViewHost::update`
/// preserves mounted identity (synvelis#112 slice 1).
#[test]
fn main_spec_carries_transport_buttons_status_and_inspector_table() {
    let spec = main_screen_spec(&MainScreenState::default());
    for (name, action) in [("transport.play", "play"), ("transport.stop", "stop")] {
        match find_component(&spec.root, name) {
            Some(ComponentSpec::Button(b)) => {
                assert_eq!(b.target, "transport");
                assert_eq!(b.action, action);
            }
            other => panic!("button `{name}` missing or wrong kind: {other:?}"),
        }
    }
    assert!(matches!(
        find_component(&spec.root, "transport.status"),
        Some(ComponentSpec::Label(_))
    ));
    let table = spec_table(&spec, "inspector.table");
    assert_eq!(table.columns, ["Field", "Value"]);
    assert!(
        table.rows.is_empty(),
        "no breakdown before first projection"
    );
    assert!(table.selection.is_none(), "inspector table is read-only");
}

/// The Transport Play/Stop `Button`s ride the existing
/// `syn.control.*` command path end to end: the press mints an
/// `Action` on the `transport` target, `ControlsUi::route_action`
/// applies it, the `Applied` receipt and the re-broadcast
/// `TransportProjection` land, the UI pump's screen-state capture
/// writes the shared cell, and the `ViewHost::update` spec push paints
/// the status `Label` — generation and mounted identity preserved
/// (synvelis#112 slice 1).
#[test]
fn transport_buttons_play_and_stop_through_the_command_path() {
    let mut f = fixture();

    // attach_view subscribed `transport` as an action surface — its
    // projection feeds the status readout even before any press.
    let sub = results_for(&f.driver, f.window, "shell-sub-view-1-transport");
    assert_eq!(sub.len(), 1, "one transport subscribe result: {sub:?}");
    assert!(sub[0].0, "transport subscribe ok: {sub:?}");
    assert_eq!(
        f.screen.borrow().transport.status.as_deref(),
        Some("Stopped"),
        "initial capture reflects the stopped truth"
    );

    f.press("transport.play");

    // The wire carried an `Applied` receipt for the transport action —
    // route_action's authoritative outcome, not a transport ack.
    let receipts = notifies(&f.driver, f.window, "syn.control.receipt");
    assert!(
        receipts
            .iter()
            .any(|r| { r["target"] == "transport" && r["receipt"]["status"] == "applied" }),
        "play receipt applied: {receipts:?}"
    );

    // The broadcast re-projected transport truth; the capture wrote it
    // into the shared cell, and pushing the spec paints the Label.
    {
        let screen = f.screen.borrow();
        assert_eq!(screen.transport.status.as_deref(), Some("Playing"));
        assert_eq!(screen.transport.position_seconds, Some(0.0));
    }
    let generation = f.host.generation();
    let ops = f.push_screen_state();
    assert_eq!(
        f.host.generation(),
        generation,
        "state push keeps generation"
    );
    assert!(
        ops.iter()
            .all(|op| matches!(op, ProjectionOp::Updated { .. })),
        "spec push updates in place, never remounts: {ops:?}"
    );
    let spec = f.host.spec().expect("spec mounted");
    let status = spec_label_text(spec, "transport.status");
    assert!(status.contains("Playing"), "status label: {status}");
    assert!(status.contains("0.00s"), "position in seconds: {status}");

    f.press("transport.stop");
    assert_eq!(
        f.screen.borrow().transport.status.as_deref(),
        Some("Stopped")
    );
    f.push_screen_state();
    let spec = f.host.spec().expect("spec mounted");
    let status = spec_label_text(spec, "transport.status");
    assert!(status.contains("Stopped"), "status label: {status}");
}

/// The Inspector tab shows the subscribed parameter target's
/// `InspectorBreakdown`: the UI pump's capture flattens the
/// `ParamProjection` into `MainScreenState.inspector` and the spec
/// push renders it as `inspector.target` + the read-only
/// `inspector.table` Field/Value rows. Slice 1 displays the view's
/// subscribed param target — the mixer gain bootstrap; target
/// selection UI is a later slice (synvelis#112 slice 1).
#[test]
fn inspector_table_shows_the_subscribed_targets_breakdown() {
    let mut f = fixture();
    f.deliver_to_host();

    {
        let screen = f.screen.borrow();
        assert_eq!(screen.inspector.target, Some(mixer_gain_target()));
        let fields: Vec<&str> = screen
            .inspector
            .rows
            .iter()
            .map(|r| r.field.as_str())
            .collect();
        for expected in [
            "plain",
            "normalized",
            "mode",
            "arm",
            "observation",
            "writer",
            "read_only",
            "capability",
        ] {
            assert!(
                fields.contains(&expected),
                "missing `{expected}`: {fields:?}"
            );
        }
        assert_eq!(screen.inspector.rows.len(), 8);
    }

    f.push_screen_state();
    let spec = f.host.spec().expect("spec mounted");
    assert_eq!(
        spec_label_text(spec, "inspector.target"),
        mixer_gain_target()
    );
    let table = spec_table(spec, "inspector.table");
    assert_eq!(table.columns, ["Field", "Value"]);
    assert_eq!(table.rows.len(), 8);
    assert!(table.selection.is_none(), "inspector table is read-only");
    for (row, field) in table.rows.iter().zip([
        "plain",
        "normalized",
        "mode",
        "arm",
        "observation",
        "writer",
        "read_only",
        "capability",
    ]) {
        assert_eq!(row.len(), 2, "row `{field}` has two cells");
        assert_eq!(row[0], Some(CellValue::Text(field.to_string())));
        assert!(
            matches!(row[1], Some(CellValue::Text(_))),
            "row `{field}` value is resolved text"
        );
    }
}

// ---------------------------------------------------------------------
// SV-GUI-03 slice 2 (synvelis#112): Arrangement table + Undo/Redo
// ---------------------------------------------------------------------

fn apply_op(session: &mut ProjectSession, op: ProjectOp) {
    let rev: ProjectRevision = session.revision();
    session.apply(op, rev).expect("fixture op");
}

/// Writes a clip-bearing project bundle into `dir` through the same
/// `DirectoryBundleStore` + `ProjectController` save path the app
/// uses — `syn.project.open` then loads real track/lane/clip content
/// for the Arrangement table (synvelis#112 slice 2). The clip is
/// muted so the Muted column is exercised.
fn write_clip_bundle(dir: &std::path::Path) {
    let mut store = DirectoryBundleStore::create(dir).expect("bundle store");
    let mut session = ProjectSession::new(ProjectController::new(
        Project::new(ProjectId::new(0x5EE), "arrangement"),
        Box::new(CounterIdGenerator::new(1)),
    ));
    apply_op(
        &mut session,
        ProjectOp::RegisterAsset {
            digest: "deadbeef".to_string(),
        },
    );
    let asset_id = *session.project().assets.keys().next().expect("asset");
    apply_op(
        &mut session,
        ProjectOp::RegisterSource {
            asset_id,
            content_digest: "deadbeef".to_string(),
        },
    );
    apply_op(
        &mut session,
        ProjectOp::AddTrack {
            name: "t1".to_string(),
        },
    );
    let track_id = *session.project().tracks.keys().next().expect("track");
    apply_op(&mut session, ProjectOp::AddLane { track_id });
    let lane_id = *session.project().lanes.keys().next().expect("lane");
    let source_id = *session.project().sources.keys().next().expect("source");
    apply_op(
        &mut session,
        ProjectOp::AddClip {
            lane_id,
            source_id,
            placement: ClipPlacement {
                anchor: TimeAnchor::Musical(MusicalTick::new(0)),
                extent: ClipExtent::Musical(MusicalDuration::new(960).expect("extent")),
            },
            playback: ClipPlayback {
                source_region: SourceRange::try_new(
                    SourceFrame::ZERO,
                    SourceFrame::new(1_000_000).expect("frame"),
                )
                .expect("region"),
                mapping: PlaybackMapping::Natural,
            },
        },
    );
    let clip_id = *session.project().clips.keys().next().expect("clip");
    apply_op(
        &mut session,
        ProjectOp::SetClipMuted {
            clip_id,
            muted: true,
        },
    );
    session
        .controller_mut()
        .save(&mut store)
        .expect("save bundle");
}

/// The mounted spec carries the Arrangement tab (first, read-only
/// `Table` with the track/lane/clip/source/start/end/muted columns)
/// and the toolbar Undo/Redo `Button`s on the `project` action target
/// (synvelis#112 slice 2).
#[test]
fn main_spec_carries_arrangement_table_and_undo_redo_buttons() {
    let spec = main_screen_spec(&MainScreenState::default());
    for (name, action) in [("project.undo", "undo"), ("project.redo", "redo")] {
        match find_component(&spec.root, name) {
            Some(ComponentSpec::Button(b)) => {
                assert_eq!(b.target, "project");
                assert_eq!(b.action, action);
            }
            other => panic!("button `{name}` missing or wrong kind: {other:?}"),
        }
    }
    let tabs = match find_component(&spec.root, "screen.tabs") {
        Some(ComponentSpec::Tabs(t)) => t,
        other => panic!("tabs missing or wrong kind: {other:?}"),
    };
    assert_eq!(tabs.tabs[0].title, "Arrangement");
    assert!(
        matches!(
            find_component(&spec.root, "tab.arrangement.panel"),
            Some(ComponentSpec::Panel(_))
        ),
        "arrangement tab child mounted"
    );
    let table = spec_table(&spec, "arrangement.table");
    assert_eq!(
        table.columns,
        ["Track", "Lane", "Clip", "Source", "Start", "End", "Muted"]
    );
    assert!(table.rows.is_empty(), "no clips before project open");
    assert!(table.selection.is_none(), "arrangement table is read-only");
}

/// Arrangement rows are project truth: `syn.project.open` loads a
/// clip-bearing bundle, the service tail's `publish_project_length`
/// fills `MainScreenState.arrangement_rows`, and the `ViewHost::update`
/// spec push renders them into `arrangement.table` — generation and
/// mounted identity preserved (synvelis#112 slice 2).
#[test]
fn arrangement_rows_follow_project_open_and_state_push() {
    let mut f = fixture();
    let dir = std::env::temp_dir().join(format!("sv112-arr-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    write_clip_bundle(&dir);
    let path = dir.to_string_lossy().to_string();

    action(
        &mut f.shell,
        &mut f.driver,
        f.window,
        Intent::new("po", "syn.project.open", json!({ "path": path })),
    );
    assert!(results_for(&f.driver, f.window, "po").last().unwrap().0);

    {
        let screen = f.screen.borrow();
        assert_eq!(screen.arrangement_rows.len(), 1, "clip row published");
        let row = &screen.arrangement_rows[0];
        assert_eq!(row.track, "t1");
        assert!(row.muted, "muted flag carried");
        assert!(row.start_seconds.is_some());
        assert!(row.end_seconds.is_some());
    }

    let generation = f.host.generation();
    let ops = f.push_screen_state();
    assert_eq!(f.host.generation(), generation, "push keeps generation");
    assert!(
        ops.iter()
            .all(|op| matches!(op, ProjectionOp::Updated { .. })),
        "spec push updates in place, never remounts: {ops:?}"
    );
    let spec = f.host.spec().expect("spec mounted");
    let table = spec_table(spec, "arrangement.table");
    assert_eq!(table.rows.len(), 1);
    let cells = &table.rows[0];
    assert_eq!(cells[0], Some(CellValue::Text("t1".to_string())));
    assert_eq!(cells[1], Some(CellValue::Text(row_lane_tag(&f))));
    // Default tempo map: 960 ticks = 1 beat = 0.5 s at 120 BPM.
    assert!(
        matches!(cells[4], Some(CellValue::Text(ref s)) if s == "0.00"),
        "start seconds: {cells:?}"
    );
    assert!(
        matches!(cells[5], Some(CellValue::Text(ref s)) if s == "0.50"),
        "end seconds: {cells:?}"
    );
    assert_eq!(cells[6], Some(CellValue::Text("yes".to_string())));

    let _ = std::fs::remove_dir_all(&dir);
}

/// The lane tagged id of the fixture's single clip — read back from
/// the published row so the assertion does not hard-code the id.
fn row_lane_tag(f: &MountedFixture) -> String {
    f.screen.borrow().arrangement_rows[0].lane.clone()
}

/// The Undo/Redo `Button`s ride the `project` action path end to end:
/// the press mints an `Action` on `project`, `project_action_op`
/// resolves `syn.project.undo`/`syn.project.redo`, the shell dispatches
/// the service op, and the op's `Result` answers the command — an
/// empty history reports `false`, never a fake success
/// (synvelis#112 slice 2).
#[test]
fn undo_redo_buttons_dispatch_project_ops_through_the_mounted_view() {
    let mut f = fixture();

    f.press("project.undo");
    let (ok, data) = last_result(&f.driver, f.window).expect("undo result");
    assert!(ok, "undo op ok");
    assert_eq!(data.expect("undo data")["undone"], false);

    f.press("project.redo");
    let (ok, data) = last_result(&f.driver, f.window).expect("redo result");
    assert!(ok, "redo op ok");
    assert_eq!(data.expect("redo data")["redone"], false);

    assert_eq!(project_action_op("undo"), Some("syn.project.undo"));
    assert_eq!(project_action_op("redo"), Some("syn.project.redo"));
}
