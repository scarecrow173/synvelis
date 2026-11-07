//! `synvelis-native` — the native desktop entry point (the `native`
//! feature, SV-GPUI-01 / Issue #175).
//!
//! This binary is the composition-root exception: the only place GPUI
//! enters the product tree. It assembles the RF386 standard driver
//! ([`GpuiDriver`]), the shared app (`SynvelisApp` → `AppService` →
//! `UiRuntime` inside [`NativeShell`]), and drives the shell through
//! the driver's blocking event loop. View rendering stays inside the
//! driver's minimal fixture until the RF387 renderer contract lands —
//! this binary wires the shell/lifecycle/binding seam, not screens.
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

use ruviform_ui::{BackendReport, Capability, WindowCapabilities, WindowDriver};
use ruviform_ui_gpui::GpuiDriver;

use synvelis_app::logging::{self, EnvOverrides};
use synvelis_app::{LogSettings, NativeShell, SynvelisApp};

/// UI tick cadence — job drain, outbox flush, and clock advance rate.
const TICK: Duration = Duration::from_millis(16);

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

    let mut shell = match NativeShell::new(app, TICK) {
        Ok(shell) => shell,
        Err(e) => {
            eprintln!("synvelis-native: failed to compose shell: {e}");
            tracing::error!(error = %e, "service registration failed");
            return ExitCode::from(3);
        }
    };
    if let Err(e) = shell.open_main(&mut driver) {
        eprintln!("synvelis-native: window creation failed: {e}");
        tracing::error!(error = %e, "window creation failed");
        return ExitCode::from(3);
    }

    let result = driver.run(Some(TICK), move |event, driver| {
        shell.handle(event, driver);
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
