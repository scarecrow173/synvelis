//! `synvelis-native` — the native desktop entry point (the `native`
//! feature, SV-GPUI-01 / Issue #175).
//!
//! This binary is the composition-root exception: the only place GPUI
//! enters the product tree. It assembles the RF386 standard driver
//! ([`GpuiDriver`]), the shared app (`SynvelisApp` → `AppService` →
//! `UiRuntime` inside [`NativeShell`]), mounts the product's minimal
//! screen spec (`Panel > Tabs > Seek`/`Fader`, RF387) through the
//! driver's `create_view_window` seam, and drives the shell through
//! the driver's blocking event loop. The view's `AttachEpoch` is
//! minted by the session-owned allocator on [`SynvelisApp`] before the
//! app is sealed; the logical `view_id` is the persisted identity,
//! never the epoch.
//!
//! Startup order mirrors the `synvelis` (webview) entry: install the
//! process-global logging subscriber first (SYN-OBS-01), then compose
//! the app, open the main window through the driver, and run the loop.
//! The driver's `WindowCapabilities` snapshot is logged verbatim —
//! `ime` stays `Unsupported` (Issue #389 owner) and
//! `native_surface_slot` reports the real backend availability; the
//! process never claims a native capability it does not have.
//!
//! Logging flags: `--log[=FILTER]`, `--log-file <DIR>`,
//! `--log-format text|jsonl` — same contract as `synvelis`.
//! Environment: `SYNVELIS_LOG`, `SYNVELIS_LOG_FILE`,
//! `SYNVELIS_LOG_FORMAT`, `SYNVELIS_PROJECT_DIR`,
//! `SYNVELIS_SECOND_WINDOW` (SV-GUI-04 slice 1 — `1`/`true`/`yes`/`on`
//! mounts a second window carrying the transport-deck view `view-2`
//! on the same app/session; unset keeps the single-window boot).
//!
//! Exit codes: `0` clean shutdown (last window destroyed /
//! `syn.app.quit`), `3` service-registration, window-creation, or
//! driver-loop failure, `4` logging configuration error.

use std::cell::RefCell;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::Duration;

use ruviform_ui::driver::{DriverEvent, WindowLifecycle};
use ruviform_ui::view::{AttachEpochAllocator, ViewHost, ViewSpec};
use ruviform_ui::{
    BackendReport, Capability, LogicalPosition, LogicalSize, WindowCapabilities, WindowDriver,
    WindowId, WindowSpec,
};
use ruviform_ui_gpui::{GpuiDriver, ViewMount};
use synvelis_theme::{ResolvedTheme, ThemeCatalog, ThemeSelection};

use synvelis_app::logging::{self, EnvOverrides};
use synvelis_app::{
    LogSettings, MainScreenState, NativeShell, SessionEngineConfig, SynvelisApp,
    compose_routing_engine, main_screen_spec, main_view_id, main_view_targets, mount_main_view,
    mount_secondary_view, secondary_screen_spec, secondary_view_id, secondary_view_targets,
};
use synvelis_project::{BuildBudget, BuildMode, RuntimeClockEpochRef, RuntimeInstanceId};

/// UI tick cadence — job drain, outbox flush, and clock advance rate.
const TICK: Duration = Duration::from_millis(16);

/// Opens the project bundle named by `SYNVELIS_PROJECT_DIR` — creating it
/// when the path does not exist yet — so Save/Reload act on a real bundle.
/// Without the variable the boot project stays unsaved (Save reports that).
fn open_boot_project(app: &mut SynvelisApp) {
    let Some(dir) = std::env::var_os("SYNVELIS_PROJECT_DIR") else {
        return;
    };
    let path = dir.to_string_lossy();
    let result = if std::path::Path::new(&dir).exists() {
        app.project_open(&path)
    } else {
        app.project_new(&path, None)
    };
    if let Err(e) = result {
        tracing::error!(error = %e, "boot project bundle failed");
    }
}

/// Product routing composition (PC-C4 drain slice, synvelis#101):
/// attaches the session routing engine — `PluginHostProcessors` over
/// a fresh `PluginHost`, catalog cell, report drain — through
/// [`compose_routing_engine`], so `syn.control.*` ops and the
/// `ui_pump` control/drain cadence run the real path. The product
/// currently ships no plugin factories (real VST3/OS backend is
/// ruviform#253, NotRun): the registry is empty and plugin-bound
/// processors fail typed `PluginUnavailable`, never fabricated.
/// `media_root` is the verified blob store beside the bundle —
/// `<SYNVELIS_PROJECT_DIR>/media`; without a boot project dir the
/// untitled session keeps the engine stub (no media root exists to
/// serve, and project recreate detaches anyway — per-project
/// re-attach is the deferred engine-wiring slice).
fn compose_product_routing(app: &mut SynvelisApp) {
    let Some(dir) = std::env::var_os("SYNVELIS_PROJECT_DIR") else {
        return;
    };
    compose_routing_engine(
        app,
        product_engine_config(PathBuf::from(dir).join("media")),
        Vec::new(),
    );
}

/// Product engine geometry — 48 kHz/stereo/256-frame Realtime build,
/// single runtime instance/epoch, generation 1. Matches the headless
/// fixture geometry so composed evidence is comparable.
fn product_engine_config(media_root: PathBuf) -> SessionEngineConfig {
    SessionEngineConfig {
        media_root,
        engine: synvelis_player::lowering::EngineConfig {
            sample_rate: 48_000,
            channels: 2,
            block_frames: 256,
        },
        prefetch: ruviform_media_reader::PrefetchConfig {
            block_frames: 256,
            block_count: 8,
            channels: 2,
            sample_rate: 48_000,
        },
        budget: BuildBudget {
            max_nodes: 4_096,
            max_connections: 16_384,
            max_frames: 512,
        },
        mode: BuildMode::Realtime,
        runtime_instance: RuntimeInstanceId(1),
        device_clock_epoch: RuntimeClockEpochRef(1),
        engine_config_generation: 1,
    }
}

/// Reads `SYNVELIS_LOG*` at the process boundary — the library never
/// touches `std::env` itself.
fn env_overrides() -> EnvOverrides {
    EnvOverrides {
        log: std::env::var("SYNVELIS_LOG").ok(),
        log_file: std::env::var("SYNVELIS_LOG_FILE").ok().map(Into::into),
        log_format: std::env::var("SYNVELIS_LOG_FORMAT").ok(),
    }
}

/// SYN-OBS-01: the process entry point resolves product settings and
/// installs the global subscriber at most once.
fn install_logging() -> Result<logging::HostLog, ExitCode> {
    let cli = match logging::parse_args(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(cli) => cli,
        Err(e) => {
            eprintln!("synvelis-native: {e}");
            return Err(ExitCode::from(4));
        }
    };
    let settings = match LogSettings::resolve(&cli, &env_overrides()) {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("synvelis-native: {e}");
            return Err(ExitCode::from(4));
        }
    };
    let host_log = match logging::install(&settings) {
        Ok(host_log) => host_log,
        Err(e) => {
            eprintln!("synvelis-native: {e}");
            return Err(ExitCode::from(4));
        }
    };
    tracing::info!(
        state = host_log.state().as_str(),
        "synvelis-native logging install"
    );
    for entry in host_log.degraded() {
        tracing::warn!(sink = entry.sink_index, kind = %entry.kind, "logging sink degraded");
    }
    Ok(host_log)
}

fn capability_str(c: Capability) -> &'static str {
    match c {
        Capability::Supported => "supported",
        _ => "unsupported",
    }
}

fn log_capabilities(caps: &WindowCapabilities) {
    tracing::info!(
        resize = capability_str(caps.resize),
        focus = capability_str(caps.focus),
        scale_factor = capability_str(caps.scale_factor),
        multi_window = capability_str(caps.multi_window),
        ime = capability_str(caps.ime),
        native_surface_slot = capability_str(caps.native_surface_slot),
        "gpui driver capabilities"
    );
}

/// Resolves the shipped product theme (`synvelis/dark/comfortable`)
/// through the `synvelis-theme` catalog — RF385 `ResolvedTheme`
/// interpretation stays delegated upstream; this function only selects
/// the layered documents. `None` yields the driver's literal fallback
/// palette — a missing theme is degraded, never fatal.
fn product_theme() -> Option<synvelis_theme::ResolvedTheme> {
    let mut catalog = ThemeCatalog::new();
    for json in [
        include_str!("../../../../assets/themes/synvelis-base.dtcg.json"),
        include_str!("../../../../assets/themes/synvelis-dark.dtcg.json"),
        include_str!("../../../../assets/themes/synvelis-density-comfortable.dtcg.json"),
        include_str!("../../../../assets/themes/synvelis-density-compact.dtcg.json"),
        include_str!("../../../../assets/themes/synvelis-light.dtcg.json"),
    ] {
        match synvelis_theme::load_document(json) {
            Ok(doc) => {
                catalog.insert(doc);
            }
            Err(e) => tracing::warn!(error = %e, "theme document rejected"),
        }
    }
    let sel = ThemeSelection {
        theme: "synvelis".to_string(),
        mode: "dark".to_string(),
        density: "comfortable".to_string(),
    };
    match catalog.resolve(&sel) {
        Ok(theme) => Some(theme),
        Err(e) => {
            tracing::warn!(error = %e, "theme resolution failed — fallback palette");
            None
        }
    }
}

/// `SYNVELIS_SECOND_WINDOW` — the SV-GUI-04 slice-1 gate for the
/// second native window (synvelis#113). Truthy values `1`, `true`,
/// `yes`, `on` (case-insensitive, surrounding whitespace ignored);
/// unset or anything else keeps the single-window boot. Read at the
/// process boundary only — the library never touches `std::env`.
fn second_window_enabled() -> bool {
    std::env::var("SYNVELIS_SECOND_WINDOW").is_ok_and(|v| {
        matches!(
            v.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

/// One mounted view↔window pair the run loop drives: attach the view
/// on the window's first `Ready`, then push the matching screen spec
/// whenever the shared [`MainScreenState`] changes.
struct PendingMount {
    window: WindowId,
    view_id: String,
    targets: Vec<String>,
    /// `true` for the transport-deck view — its state pushes use
    /// [`secondary_screen_spec`] instead of [`main_screen_spec`].
    secondary: bool,
    attached: bool,
}

impl PendingMount {
    /// The screen spec this mount currently displays — rebuilt from
    /// the shared [`MainScreenState`] on every push.
    fn spec(&self, state: &MainScreenState) -> ViewSpec {
        if self.secondary {
            secondary_screen_spec(state)
        } else {
            main_screen_spec(state)
        }
    }
}

/// Structured trace for one pushed screen-state update.
fn log_screen_state_update(state: &MainScreenState) {
    tracing::info!(
        project_length_seconds = state.project_length_seconds,
        arrangement_rows = state.arrangement_rows.len(),
        transport_status = ?state.transport.status,
        inspector_target = ?state.inspector.target,
        "screen state updated"
    );
}

/// Mounts the product view hosts while the app is still
/// composition-owned — each `AttachEpoch` comes from the session
/// allocator handle, never a second allocator, and the shared
/// [`MainScreenState`] cell is what the loop below pushes into the
/// mounted specs. Returns the main host plus the env-gated secondary
/// host (`SYNVELIS_SECOND_WINDOW`, SV-GUI-04 slice 1 — a mount failure
/// under the explicit gate is a composition defect, never a silent
/// degrade).
fn mount_views(
    attach_epochs: &Rc<RefCell<AttachEpochAllocator>>,
    screen_state: &Rc<RefCell<MainScreenState>>,
) -> Result<(ViewHost, Option<ViewHost>), ExitCode> {
    let main = mount_main_view(attach_epochs, &screen_state.borrow()).map_err(|e| {
        eprintln!("synvelis-native: view mount failed: {e}");
        tracing::error!(error = %e, "view mount failed");
        ExitCode::from(3)
    })?;
    let secondary = if second_window_enabled() {
        Some(
            mount_secondary_view(attach_epochs, &screen_state.borrow()).map_err(|e| {
                eprintln!("synvelis-native: secondary view mount failed: {e}");
                tracing::error!(error = %e, "secondary view mount failed");
                ExitCode::from(3)
            })?,
        )
    } else {
        None
    };
    Ok((main, secondary))
}

/// Reserves one view window; `what` names the window in the failure
/// diagnostics. Window creation happens before `run`, so a failure
/// here is a composition defect (exit 3).
fn create_view_window(
    driver: &mut GpuiDriver,
    spec: &WindowSpec,
    host: ViewHost,
    theme: Option<&ResolvedTheme>,
    what: &str,
) -> Result<WindowId, ExitCode> {
    driver
        .create_view_window(
            spec,
            ViewMount {
                host,
                theme: theme.cloned(),
            },
        )
        .map_err(|e| {
            eprintln!("synvelis-native: {what} failed: {e}");
            tracing::error!(error = %e, "{what} failed");
            ExitCode::from(3)
        })
}

/// Creates the OS windows for the mounted views — the main window
/// always, the transport-deck window only when the env gate mounted a
/// secondary view — and returns the run-loop mount records in
/// creation order.
fn open_windows(
    driver: &mut GpuiDriver,
    theme: Option<&ResolvedTheme>,
    main_host: ViewHost,
    secondary_host: Option<ViewHost>,
) -> Result<Vec<PendingMount>, ExitCode> {
    let window = create_view_window(
        driver,
        &WindowSpec {
            title: "Synvelis".to_string(),
            ..WindowSpec::default()
        },
        main_host,
        theme,
        "window creation",
    )?;
    let mut mounts = vec![PendingMount {
        window,
        view_id: main_view_id(),
        targets: main_view_targets(),
        secondary: false,
        attached: false,
    }];

    if let Some(host) = secondary_host {
        let spec = WindowSpec {
            title: "Synvelis — Secondary".to_string(),
            logical_size: LogicalSize::new(960, 600),
            position: Some(LogicalPosition::new(80, 80)),
            resizable: true,
        };
        let window = create_view_window(driver, &spec, host, theme, "secondary window creation")?;
        tracing::info!(window = %window, "secondary window reserved");
        mounts.push(PendingMount {
            window,
            view_id: secondary_view_id(),
            targets: secondary_view_targets(),
            secondary: true,
            attached: false,
        });
    }
    Ok(mounts)
}

/// Drives the shell through the driver's blocking event loop. Each
/// mount attaches its view on its window's first `Ready` — after
/// `handle` so the shell has registered the window (owner resolution
/// requires it in `windows`) — then every screen-state change
/// (project length → seek range, transport readout, inspector rows)
/// is pushed as an updated spec to every attached mount, which the
/// driver applies to the mounted view in place (no remount).
/// `view_driver` is the `Rc`-shared clone the `'static` event closure
/// uses for `update_view`.
fn run_loop(
    driver: &mut GpuiDriver,
    mut shell: NativeShell,
    mut mounts: Vec<PendingMount>,
    screen_state: Rc<RefCell<MainScreenState>>,
) -> ExitCode {
    let view_driver = driver.clone();
    let mut applied_state = screen_state.borrow().clone();
    let result = driver.run(Some(TICK), move |event, driver| {
        // First `Ready` per mount is detected before `handle` consumes
        // the event — `attach_view` needs the window registered in the
        // shell's `windows` map, which `handle` performs.
        let ready: Vec<usize> = mounts
            .iter()
            .enumerate()
            .filter(|(_, m)| {
                !m.attached
                    && matches!(
                        &event,
                        DriverEvent::Windowed {
                            window: w,
                            event: WindowLifecycle::Ready,
                        } if *w == m.window
                    )
            })
            .map(|(i, _)| i)
            .collect();
        shell.handle(event, driver);
        for i in ready {
            let m = &mut mounts[i];
            m.attached = true;
            if let Err(e) = shell.attach_view(driver, m.window, &m.view_id, &m.targets) {
                eprintln!("synvelis-native: view attach failed: {e}");
                tracing::error!(error = %e, view_id = %m.view_id, "view attach failed");
            } else {
                tracing::info!(view_id = %m.view_id, window = %m.window, "view attached");
            }
            // A window that attached after the state cell moved must
            // not keep its mount-time spec — push the current one.
            let state = screen_state.borrow().clone();
            if let Err(e) = view_driver.update_view(m.window, m.spec(&state)) {
                tracing::error!(error = %e, "screen state update failed");
            }
        }
        let state = screen_state.borrow().clone();
        if state != applied_state {
            applied_state = state.clone();
            let mut pushed = false;
            for m in mounts.iter().filter(|m| m.attached) {
                match view_driver.update_view(m.window, m.spec(&state)) {
                    Ok(()) => pushed = true,
                    Err(e) => tracing::error!(error = %e, "screen state update failed"),
                }
            }
            if pushed {
                log_screen_state_update(&state);
            }
        }
    });
    match result {
        Ok(()) => {
            tracing::info!("synvelis-native shutdown");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("synvelis-native: driver failure: {e}");
            tracing::error!(error = %e, "driver failure");
            ExitCode::from(3)
        }
    }
}

fn main() -> ExitCode {
    let mut host_log = match install_logging() {
        Ok(host_log) => host_log,
        Err(code) => return code,
    };

    let mut driver = GpuiDriver::new();
    log_capabilities(&driver.capabilities());

    let mut app = SynvelisApp::new(BackendReport::available(
        "gpui",
        ruviform_ui_gpui::version().to_string(),
    ));
    app.attach_logging(
        host_log.state(),
        host_log.health().cloned(),
        host_log.take_degraded(),
    );

    open_boot_project(&mut app);
    compose_product_routing(&mut app);

    // The view hosts are minted while the app is still
    // composition-owned — the `AttachEpoch` comes from the session
    // allocator handle, never a second allocator. The main-screen
    // state cell is taken here too: the app publishes into it after
    // every op and UI pump, and the loop below pushes changes into
    // the mounted specs.
    let screen_state = app.main_screen_cell();
    let attach_epochs = app.attach_epoch_allocator();
    let (main_host, secondary_host) = match mount_views(&attach_epochs, &screen_state) {
        Ok(mounts) => mounts,
        Err(code) => return code,
    };

    let shell = match NativeShell::new(app, TICK) {
        Ok(shell) => shell,
        Err(e) => {
            eprintln!("synvelis-native: failed to compose shell: {e}");
            tracing::error!(error = %e, "service registration failed");
            return ExitCode::from(3);
        }
    };
    let theme = product_theme();
    let mounts = match open_windows(&mut driver, theme.as_ref(), main_host, secondary_host) {
        Ok(mounts) => mounts,
        Err(code) => return code,
    };

    run_loop(&mut driver, shell, mounts, screen_state)
    // `host_log` drops here — sinks drain, flush, and close in order.
}
