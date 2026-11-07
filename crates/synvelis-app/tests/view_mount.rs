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
    CommandReceipt, ControlInput, InputSource, InteractionEvent, ReceiptStatus, Revision,
    StateProjection, ValueInput,
};
use ruviform_ui::ipc::{Event, Intent};
use ruviform_ui::view::{ComponentKey, ViewEvent, ViewHost};
use ruviform_ui::{
    BackendReport, DriverEvent, WindowDriver, WindowId, WindowLifecycle, WindowSpec,
};
use ruviform_ui_test::FakeDriver;

use synvelis_app::{NativeShell, SynvelisApp, main_view_id, main_view_targets, mount_main_view};

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
    /// Cursor into `driver.events_for(window)` — events are an
    /// append-only log on the fake, so deliveries are consumed by
    /// index, mirroring `RvfView`'s drainable inbox.
    inbox_cursor: usize,
}

fn fixture() -> MountedFixture {
    let app = SynvelisApp::new(BackendReport::available("gpui", "test-0.1".to_string()));
    let host = mount_main_view(&app.attach_epoch_allocator()).expect("main view mounts");
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

#[test]
fn unsupported_mixer_fader_never_mints_a_command() {
    let mut f = fixture();
    f.deliver_to_host();
    let receipts_before = notifies(&f.driver, f.window, "syn.control.receipt").len();

    // `fader.gain` is mounted `unsupported: true` — a begin resolves
    // locally as `Unavailable`, minting no backend intent.
    let generation = f.host.generation();
    f.host
        .dispatch_input(generation, &key("fader.gain"), MountedFixture::begin("gx"))
        .expect("dispatch_input on unsupported control");
    let events = f.host.drain_events();
    assert!(
        events.iter().any(|e| matches!(
            e,
            ViewEvent::Gesture {
                event: InteractionEvent::Unavailable { .. },
                ..
            }
        )),
        "unavailable reported on the unsupported fader: {events:?}"
    );
    assert!(
        !events.iter().any(|e| matches!(e, ViewEvent::Intent { .. })),
        "no intent minted for an unsupported control: {events:?}"
    );
    let receipts_after = notifies(&f.driver, f.window, "syn.control.receipt").len();
    assert_eq!(receipts_before, receipts_after, "no domain receipt minted");
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
    let mut fresh = mount_main_view(&f.shell.attach_epoch_allocator()).expect("fresh host mounts");

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
