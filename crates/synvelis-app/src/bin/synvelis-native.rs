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
//! `SYNVELIS_LOG_FORMAT`.
//!
//! Exit codes: `0` clean shutdown (last window destroyed /
//! `syn.app.quit`), `3` service-registration, window-creation, or
//! driver-loop failure, `4` logging configuration error.

use std::process::ExitCode;
use std::time::Duration;

use ruviform_ui::driver::{DriverEvent, WindowLifecycle};
use ruviform_ui::{BackendReport, Capability, WindowCapabilities, WindowDriver, WindowSpec};
use ruviform_ui_gpui::{GpuiDriver, ViewMount};
use synvelis_theme::{ThemeCatalog, ThemeSelection};

use synvelis_app::logging::{self, EnvOverrides};
use synvelis_app::{
    LogSettings, MainScreenState, NativeShell, SynvelisApp, main_screen_spec, main_view_id,
    main_view_targets, mount_main_view,
};

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

    // The view host is minted while the app is still composition-owned
    // — its `AttachEpoch` comes from the session allocator handle,
    // never a second allocator. The main-screen state cell is taken
    // here too: the app publishes into it after every op and UI pump,
    // and the loop below pushes changes into the mounted spec.
    let screen_state = app.main_screen_cell();
    let view_host = match mount_main_view(&app.attach_epoch_allocator(), &screen_state.borrow()) {
        Ok(host) => host,
        Err(e) => {
            eprintln!("synvelis-native: view mount failed: {e}");
            tracing::error!(error = %e, "view mount failed");
            return ExitCode::from(3);
        }
    };
    let view_id = main_view_id();
    let view_targets = main_view_targets();

    let mut shell = match NativeShell::new(app, TICK) {
        Ok(shell) => shell,
        Err(e) => {
            eprintln!("synvelis-native: failed to compose shell: {e}");
            tracing::error!(error = %e, "service registration failed");
            return ExitCode::from(3);
        }
    };
    let theme = product_theme();
    let window = match driver.create_view_window(
        &WindowSpec {
            title: "Synvelis".to_string(),
            ..WindowSpec::default()
        },
        ViewMount {
            host: view_host,
            theme,
        },
    ) {
        Ok(window) => window,
        Err(e) => {
            eprintln!("synvelis-native: window creation failed: {e}");
            tracing::error!(error = %e, "window creation failed");
            return ExitCode::from(3);
        }
    };

    // Attach + subscribe on the main window's first `Ready` — after
    // `handle` so the shell has registered the window (owner resolution
    // requires it in `windows`). Any screen-state change (project
    // length → seek range, transport readout, inspector rows) is
    // pushed as an updated spec, which the driver applies to the
    // mounted view in place (no remount).
    let view_driver = driver.clone();
    let mut applied_state = screen_state.borrow().clone();
    let mut view_attached = false;
    let result = driver.run(Some(TICK), move |event, driver| {
        let main_ready = matches!(
            &event,
            DriverEvent::Windowed {
                window: w,
                event: WindowLifecycle::Ready,
            } if *w == window
        );
        shell.handle(event, driver);
        if main_ready && !view_attached {
            view_attached = true;
            if let Err(e) = shell.attach_view(driver, window, &view_id, &view_targets) {
                eprintln!("synvelis-native: view attach failed: {e}");
                tracing::error!(error = %e, "view attach failed");
            } else {
                tracing::info!(view_id = %view_id, "main view attached");
            }
        }
        let state = screen_state.borrow().clone();
        if state != applied_state {
            applied_state = state.clone();
            if let Err(e) = view_driver.update_view(window, main_screen_spec(&state)) {
                tracing::error!(error = %e, "screen state update failed");
            } else {
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
    // `host_log` drops here — sinks drain, flush, and close in order.
}
