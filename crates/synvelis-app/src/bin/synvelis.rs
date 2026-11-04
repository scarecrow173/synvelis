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
//! Logging flags: `--log[=FILTER]` (EnvFilter directives, bare means
//! `info`), `--log-file <DIR>` (daily rolling JSONL sink), `--log-format
//! text|jsonl` (stderr format). Environment: `SYNVELIS_LOG`,
//! `SYNVELIS_LOG_FILE`, `SYNVELIS_LOG_FORMAT`. Logging is OFF unless one
//! of these enables it; the installed global subscriber cannot be
//! uninstalled, and `HostLog` is dropped after `run` returns so every
//! sink flushes and closes in order.
//!
//! Exit codes: `0` clean shutdown (window closed / `syn.app.quit`),
//! `2` backend unavailable, `3` service-registration or host failure,
//! `4` logging configuration error.

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use ruviform_ui::{BackendState, DataRegistry, HostEvent, UiRuntime, dispatch_script};
use ruviform_ui_webview::{HostConfig, detect_backend, run};

use synvelis_app::assets::{app_asset_source, error_asset_source};
use synvelis_app::logging::{self, EnvOverrides};
use synvelis_app::revision::check_bundle;
use synvelis_app::{AppService, LogSettings, SynvelisApp};

/// Reads `SYNVELIS_LOG`/`SYNVELIS_LOG_FILE`/`SYNVELIS_LOG_FORMAT` at the
/// process boundary — the library never touches `std::env` itself.
fn env_overrides() -> EnvOverrides {
    EnvOverrides {
        log: std::env::var("SYNVELIS_LOG").ok(),
        log_file: std::env::var("SYNVELIS_LOG_FILE").ok().map(Into::into),
        log_format: std::env::var("SYNVELIS_LOG_FORMAT").ok(),
    }
}

fn main() -> ExitCode {
    // SYN-OBS-01: the process entry point resolves product settings and
    // installs the global subscriber at most once. A settings error is a
    // startup failure — never silently disable a user-requested log.
    let cli = match logging::parse_args(&std::env::args().skip(1).collect::<Vec<_>>()) {
        Ok(cli) => cli,
        Err(e) => {
            eprintln!("synvelis: {e}");
            return ExitCode::from(4);
        }
    };
    let settings = match LogSettings::resolve(&cli, &env_overrides()) {
        Ok(settings) => settings,
        Err(e) => {
            eprintln!("synvelis: {e}");
            return ExitCode::from(4);
        }
    };
    let mut host_log = match logging::install(&settings) {
        Ok(host_log) => host_log,
        Err(e) => {
            eprintln!("synvelis: {e}");
            return ExitCode::from(4);
        }
    };
    tracing::info!(
        state = host_log.state().as_str(),
        "synvelis logging install"
    );
    for entry in host_log.degraded() {
        tracing::warn!(sink = entry.sink_index, kind = %entry.kind, "logging sink degraded");
    }

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
    let quit = app.quit_flag();
    let initial_notify = app.app_state_notify();

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

    let data = DataRegistry::new();
    let mut rt = UiRuntime::new(data.clone());
    if let Err(e) = rt.register_service(AppService::new(app)) {
        eprintln!("synvelis: failed to register app service: {e}");
        tracing::error!("failed to register app service");
        return ExitCode::from(3);
    }

    let config = HostConfig {
        title: "Synvelis".to_string(),
        width: 1440,
        height: 900,
        url: "rvf://app/index.html".to_string(),
        data_dir: None,
        tick: Some(Duration::from_millis(16)),
        devtools: cfg!(debug_assertions),
    };

    let mut sent_notify = false;
    let result = run(&config, assets, data, move |ev, host| {
        let ready = matches!(ev, HostEvent::Ready { .. });
        for out in rt.handle(ev) {
            host.eval(&dispatch_script(&out));
        }
        if ready && !sent_notify {
            sent_notify = true;
            host.eval(&dispatch_script(&initial_notify));
        }
        if quit.swap(false, Ordering::AcqRel) {
            host.request_close();
        }
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
