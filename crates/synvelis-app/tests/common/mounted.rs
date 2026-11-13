//! Mounted-view fixture shared by the headless product scenario lanes
//! (`view_mount.rs`, `svt_product_e2e.rs`).
//!
//! Composes app → mounted `ViewHost` → `NativeShell` → `FakeDriver`
//! exactly as `synvelis-native` does on the GPUI lane: `dispatch_input` →
//! `ViewEvent::Intent` → `rvf.ui.command.v1` `Action` → shell →
//! `syn.control.dispatch`; outbox receipts/projections → translated
//! `Result`/`Notify` → `apply_receipt`/`apply_projection`.
//! `deliver_to_host` mirrors `RvfView::drain_inbox` decode verbatim so
//! the Fake and GPUI lanes exercise the *same wire*.
//!
//! Nothing here asserts OS/GPU/IME behavior — semantic/headless
//! evidence only (synvelis#114 SV-GUI-05).

use std::time::Duration;

use serde_json::{Value, json};

use ruviform_ui::behavior::{
    CommandReceipt, ControlInput, InputSource, ReceiptStatus, Revision, StateProjection, ValueInput,
};
use ruviform_ui::ipc::{Event, Intent};
use ruviform_ui::view::{ComponentKey, ProjectionOp, ViewEvent, ViewHost};
use ruviform_ui::{
    BackendReport, DriverEvent, WindowDriver, WindowId, WindowLifecycle, WindowSpec,
};
use ruviform_ui_test::FakeDriver;

use synvelis_app::{
    MainScreenState, NativeShell, SynvelisApp, main_screen_spec, main_view_id, main_view_targets,
    mount_main_view,
};
use synvelis_project::{
    ClipExtent, ClipPlacement, ClipPlayback, CounterIdGenerator, DirectoryBundleStore,
    MusicalDuration, MusicalTick, PlaybackMapping, Project, ProjectController, ProjectId,
    ProjectOp, ProjectRevision, SourceFrame, SourceRange, TimeAnchor,
};
use synvelis_workspace::ProjectSession;

/// Composes app → mounted view host → shell → fake window at `Ready`,
/// then `attach_view` + subscriptions — the exact composition-root
/// order `synvelis-native` uses (host minted before the app seals).
pub struct MountedFixture {
    pub shell: NativeShell,
    pub driver: FakeDriver,
    pub host: ViewHost,
    pub window: WindowId,
    pub view_id: String,
    /// The app's shared main-screen state cell — taken before the app
    /// sealed into the shell. The UI pump keeps it fresh; tests push it
    /// into the host through `update` the same way the driver's
    /// `update_view` does.
    pub screen: std::rc::Rc<std::cell::RefCell<MainScreenState>>,
    /// Cursor into `driver.events_for(window)` — events are an
    /// append-only log on the fake, so deliveries are consumed by
    /// index, mirroring `RvfView`'s drainable inbox.
    inbox_cursor: usize,
}

pub fn key(name: &str) -> ComponentKey {
    ComponentKey(name.to_string())
}

pub fn pump(shell: &mut NativeShell, driver: &mut FakeDriver) {
    driver.pump(&mut |ev, d| shell.handle(ev, d));
}

pub fn inject(shell: &mut NativeShell, driver: &mut FakeDriver, ev: DriverEvent) {
    driver.inject(ev);
    pump(shell, driver);
}

pub fn action(shell: &mut NativeShell, driver: &mut FakeDriver, window: WindowId, intent: Intent) {
    inject(
        shell,
        driver,
        DriverEvent::Windowed {
            window,
            event: WindowLifecycle::Action(intent),
        },
    );
}

/// `data` payloads of `notify` events delivered to `window` whose
/// `channel` matches.
pub fn notifies(driver: &FakeDriver, window: WindowId, channel: &str) -> Vec<Value> {
    driver
        .events_for(window)
        .iter()
        .filter_map(|e| match e {
            Event::Notify { data, .. } if data["channel"] == channel => Some(data["data"].clone()),
            _ => None,
        })
        .collect()
}

/// `(ok, data)` of every `Result` on `window` carrying `id`.
pub fn results_for(driver: &FakeDriver, window: WindowId, id: &str) -> Vec<(bool, Option<Value>)> {
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

/// The newest `Result` the window received — the answer to the most
/// recent dispatched intent/button press.
pub fn last_result(driver: &FakeDriver, window: WindowId) -> Option<(bool, Option<Value>)> {
    driver
        .events_for(window)
        .iter()
        .rev()
        .find_map(|e| match e {
            Event::Result { ok, data, .. } => Some((*ok, data.clone())),
            _ => None,
        })
}

/// `ok` of the newest `Result` — convenience over [`last_result`].
pub fn last_result_ok(driver: &FakeDriver, window: WindowId) -> Option<bool> {
    last_result(driver, window).map(|(ok, _)| ok)
}

/// Fresh mounted fixture at `Ready` + attached main view.
pub fn fixture() -> MountedFixture {
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
    pub fn pump_view_intents(&mut self) {
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
    pub fn deliver_to_host(&mut self) {
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
    pub fn input(&mut self, k: &str, input: ControlInput) {
        let generation = self.host.generation();
        self.host
            .dispatch_input(generation, &key(k), input)
            .expect("dispatch_input");
        self.pump_view_intents();
        self.deliver_to_host();
    }

    /// Presses the Button `k` — the same mint/deliver path as `input`.
    pub fn press(&mut self, k: &str) {
        let generation = self.host.generation();
        self.host
            .invoke(generation, &key(k))
            .expect("button invoke");
        self.pump_view_intents();
        self.deliver_to_host();
    }

    /// Pushes the app's current [`MainScreenState`] into the host —
    /// the headless mirror of `GpuiDriver::update_view` on the native
    /// lane.
    pub fn push_screen_state(&mut self) -> Vec<ProjectionOp> {
        let state = self.screen.borrow().clone();
        let generation = self.host.generation();
        self.host
            .update(main_screen_spec(&state), generation)
            .expect("screen state update applies")
    }

    pub fn begin(id: &str) -> ControlInput {
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

    pub fn preview(id: &str, plain: f64) -> ControlInput {
        ControlInput::Preview {
            gesture_id: id.to_string(),
            value: ValueInput::Plain(plain),
            modifiers: None,
            seq: None,
        }
    }

    pub fn commit(id: &str) -> ControlInput {
        ControlInput::Commit {
            gesture_id: id.to_string(),
            value: None,
            modifiers: None,
            seq: None,
        }
    }

    pub fn cancel(id: &str) -> ControlInput {
        ControlInput::Cancel {
            gesture_id: id.to_string(),
            reason: None,
            seq: None,
        }
    }
}

/// `session.apply` for fixture bundle construction.
pub fn apply_op(session: &mut ProjectSession, op: ProjectOp) {
    let rev: ProjectRevision = session.revision();
    session.apply(op, rev).expect("fixture op");
}

/// Writes a clip-bearing project bundle into `dir` through the same
/// `DirectoryBundleStore` + `ProjectController` save path the app
/// uses — `syn.project.open` then loads real track/lane/clip content
/// for the Arrangement table. The clip is muted so readout columns
/// are exercised.
pub fn write_clip_bundle(dir: &std::path::Path) {
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

/// Opens the clip-bearing fixture bundle and returns the published
/// clip tagged id. `tag` keeps each test's bundle dir distinct under
/// parallel runs.
pub fn open_clip_project(f: &mut MountedFixture, tag: &str) -> String {
    let dir = std::env::temp_dir().join(format!("sv114-sel-{tag}-{}", std::process::id()));
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
    f.deliver_to_host();
    let clip = f.screen.borrow().arrangement_rows[0].clip.clone();
    let _ = std::fs::remove_dir_all(&dir);
    clip
}
