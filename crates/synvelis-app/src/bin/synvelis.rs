//! `synvelis` — the desktop application entry point (the `webview`
//! feature).
//!
//! Startup order is deliberate: resolve product logging settings and
//! install the process-global subscriber first (SYN-OBS-01 — the entry
//! point is the only place a global default is installed), then probe
//! the `WebView` backend *before* a window exists (missing
//! `WebView2`/`WebKit` gets an install hint on stderr, not a crash),
//! compose the application, validate the frontend bundle, then run the
//! host loop. A rejected bundle does not abort the process — it swaps in
//! a generated explanation page so the user sees the failure in-app, and
//! the mismatched files are never served.
//!
//! The event loop is the driver-neutral [`NativeShell`] composition —
//! the same `DriverEvent`/`WindowDriver` contract the GPUI lane drives
//! (RF386): typed `Action` intents dispatch through `UiRuntime`,
//! `Tick`s drain jobs and advance the UI clock, and the last destroyed
//! window exits the loop. `rvf://data` pulls resolve against the same
//! [`DataRegistry`] the runtime's services write to.
//!
//! Logging flags: `--log[=FILTER]` (`EnvFilter` directives, bare means
//! `info`), `--log-file <DIR>` (daily rolling JSONL sink), `--log-format
//! text|jsonl` (stderr format). Environment: `SYNVELIS_LOG`,
//! `SYNVELIS_LOG_FILE`, `SYNVELIS_LOG_FORMAT`. Logging is OFF unless one
//! of these enables it; the installed global subscriber cannot be
//! uninstalled, and `HostLog` is dropped after `run_multi` returns so
//! every sink flushes and closes in order.
//!
//! Exit codes: `0` clean shutdown (window closed / `syn.app.quit`),
//! `2` backend unavailable, `3` service-registration or host failure,
//! `4` logging configuration error.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use ruviform_ui::{BackendState, DataRegistry, LogicalSize, WindowSpec};
use ruviform_ui_webview::{SessionConfig, WebViewWindowConfig, detect_backend, run_multi};

use synvelis_app::assets::{app_asset_source, error_asset_source};
use synvelis_app::logging::{self, EnvOverrides};
use synvelis_app::revision::check_bundle;
use synvelis_app::{LogSettings, NativeShell, SynvelisApp};

/// UI tick cadence — job drain, outbox flush, and clock advance rate.
const TICK: Duration = Duration::from_millis(16);

/// Reads `SYNVELIS_LOG`/`SYNVELIS_LOG_FILE`/`SYNVELIS_LOG_FORMAT` at the
/// process boundary — the library never touches `std::env` itself.
fn env_overrides() -> EnvOverrides {
    EnvOverrides {
        log: std::env::var("SYNVELIS_LOG").ok(),
        log_file: std::env::var("SYNVELIS_LOG_FILE").ok().map(Into::into),
        log_format: std::env::var("SYNVELIS_LOG_FORMAT").ok(),
    }
}

/// SYN-OBS-01: the process entry point resolves product settings and
/// installs the global subscriber at most once. A settings error is a
/// startup failure — never silently disable a user-requested log.
fn install_logging() -> Result<logging::HostLog, ExitCode> {
    let cli = match logging::parse_args(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(cli) => cli,
        Err(e) => {
            eprintln!("synvelis: {e}");
            return Err(ExitCode::from(4));
        }
    };
    let settings = match LogSettings::resolve(&cli, &env_overrides()) {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("synvelis: {e}");
            return Err(ExitCode::from(4));
        }
    };
    let host_log = match logging::install(&settings) {
        Ok(host_log) => host_log,
        Err(e) => {
            eprintln!("synvelis: {e}");
            return Err(ExitCode::from(4));
        }
    };
    tracing::info!(
        state = host_log.state().as_str(),
        "synvelis logging install"
    );
    for entry in host_log.degraded() {
        tracing::warn!(sink = entry.sink_index, kind = %entry.kind, "logging sink degraded");
    }
    Ok(host_log)
}

fn main() -> ExitCode {
    let mut host_log = match install_logging() {
        Ok(host_log) => host_log,
        Err(code) => return code,
    };

    let report = detect_backend();
    if report.state != BackendState::Available {
        eprintln!("synvelis: webview backend `{}` unavailable", report.name);
        for note in &report.notes {
            eprintln!("  - {note}");
        }
        eprintln!("install the runtime (WebView2 Evergreen on Windows) and retry.");
        tracing::warn!(backend = report.name, "webview backend unavailable");
        return ExitCode::from(2);
    }
    tracing::info!(backend = report.name, "webview backend available");

    let mut app = SynvelisApp::new(report);
    app.attach_logging(
        host_log.state(),
        host_log.health().cloned(),
        host_log.take_degraded(),
    );

    let real_assets = app_asset_source();
    let assets: Arc<dyn ruviform_ui::AssetSource> = match check_bundle(real_assets.as_ref()) {
        Ok(manifest) => {
            eprintln!("synvelis: asset bundle ok (frontend {})", manifest.frontend);
            tracing::info!(frontend = %manifest.frontend, "asset bundle ok");
            real_assets
        }
        Err(e) => {
            eprintln!("synvelis: frontend asset check failed: {e}");
            eprintln!("serving the startup-error shell — fix the bundle and relaunch.");
            tracing::warn!(error = %e, "frontend asset check failed");
            error_asset_source(&e)
        }
    };

    // One registry shared by the runtime (service data sources) and
    // the `rvf://data` scheme handler — clones of a `DataRegistry`
    // alias the same store.
    let data = DataRegistry::new();
    let mut shell = match NativeShell::new_with_data(app, TICK, data.clone()) {
        Ok(shell) => shell,
        Err(e) => {
            eprintln!("synvelis: failed to compose shell: {e}");
            tracing::error!("failed to register app service");
            return ExitCode::from(3);
        }
    };

    let config = SessionConfig {
        data_dir: None,
        tick: Some(TICK),
        windows: vec![WebViewWindowConfig {
            spec: WindowSpec {
                title: "Synvelis".to_string(),
                logical_size: LogicalSize::new(1440, 900),
                ..WindowSpec::default()
            },
            url: "rvf://app/index.html".to_string(),
            devtools: cfg!(debug_assertions),
        }],
        default_url: "rvf://app/index.html".to_string(),
        devtools: cfg!(debug_assertions),
    };

    let result = run_multi(&config, assets, data, |event, driver| {
        shell.handle(event, driver);
    });

    match result {
        Ok(()) => {
            tracing::info!("synvelis shutdown");
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("synvelis: host failure: {e}");
            tracing::error!(error = %e, "host failure");
            ExitCode::from(3)
        }
    }
    // `host_log` drops here — sinks drain, flush, and close in order.
}
