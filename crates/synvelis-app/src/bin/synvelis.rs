//! `synvelis` — the desktop application entry point (the `webview`
//! feature).
//!
//! Startup order is deliberate: probe the `WebView` backend *before* a
//! window exists (missing `WebView2`/`WebKit` gets an install hint on
//! stderr, not a crash), compose the application, validate the
//! frontend bundle, then run the host loop. A rejected bundle does
//! not abort the process — it swaps in a generated explanation page so
//! the user sees the failure in-app, and the mismatched files are
//! never served.
//!
//! Exit codes: `0` clean shutdown (window closed / `syn.app.quit`),
//! `2` backend unavailable, `3` service-registration or host failure.

use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

use ruviform_ui::{BackendState, DataRegistry, HostEvent, UiRuntime, dispatch_script};
use ruviform_ui_webview::{HostConfig, detect_backend, run};

use synvelis_app::assets::{app_asset_source, error_asset_source};
use synvelis_app::revision::check_bundle;
use synvelis_app::{AppService, SynvelisApp};

fn main() -> ExitCode {
    let report = detect_backend();
    if report.state != BackendState::Available {
        eprintln!("synvelis: webview backend `{}` unavailable", report.name);
        for note in &report.notes {
            eprintln!("  - {note}");
        }
        eprintln!("install the runtime (WebView2 Evergreen on Windows) and retry.");
        return ExitCode::from(2);
    }

    let mut app = SynvelisApp::new(report);
    let quit = app.quit_flag();
    let initial_notify = app.app_state_notify();

    let real_assets = app_asset_source();
    let assets: Arc<dyn ruviform_ui::AssetSource> = match check_bundle(real_assets.as_ref()) {
        Ok(manifest) => {
            eprintln!("synvelis: asset bundle ok (frontend {})", manifest.frontend);
            real_assets
        }
        Err(e) => {
            eprintln!("synvelis: frontend asset check failed: {e}");
            eprintln!("serving the startup-error shell — fix the bundle and relaunch.");
            error_asset_source(&e)
        }
    };

    let data = DataRegistry::new();
    let mut rt = UiRuntime::new(data.clone());
    if let Err(e) = rt.register_service(AppService::new(app)) {
        eprintln!("synvelis: failed to register app service: {e}");
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
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("synvelis: host failure: {e}");
            ExitCode::from(3)
        }
    }
}
