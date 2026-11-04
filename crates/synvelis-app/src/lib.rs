//! `synvelis-app` — the Synvelis product application composition root
//! and GUI host boundary (Issue #110 / SV-GUI-01).
//!
//! This crate owns the single place where the product's runtime
//! services are assembled: the project session, workspace manager,
//! audio-runtime slot, recorder slot, and observability owner are
//! composed into [`SynvelisApp`], and [`AppService`] exposes them to
//! the frontend through the `ruviform-ipc` `syn.*` op surface.
//!
//! ## Layer boundary
//!
//! The library half is headless — it never imports a windowing or
//! webview type. Everything platform-dependent (wry/tao host, embedded
//! assets, the desktop entry point) sits behind the non-default
//! `webview` feature so `cargo check --workspace` stays a headless
//! verification path. GUI, CLI, and headless callers all reach the
//! same [`ruviform_ui::UiRuntime`] + [`Service`] contract, so the
//! single-project-source-of-truth invariant holds on every surface.
//!
//! ## Ops
//!
//! `syn.app.info`, `syn.app.capabilities`, `syn.app.quit`,
//! `syn.project.new`, `syn.project.open`, `syn.project.save`,
//! `syn.project.info`, `syn.transport.set`, `syn.workspace.mount` —
//! see [`service::OPS`].

pub mod app;
#[cfg(feature = "webview")]
pub mod assets;
pub mod error;
pub mod host;
pub mod logging;
pub mod revision;
pub mod service;

pub use app::{AudioRuntimeSlot, ObservabilityOwner, RecorderSlot, SynvelisApp};
pub use error::{AppError, AssetError, AssetErrorKind};
pub use host::ShellWindowHost;
pub use logging::{
    CliOverrides, EnvOverrides, HostLog, InstallState, LogInstallError, LogSettings, SettingsError,
    install, parse_args, to_logging_config,
};
pub use revision::{APP_ASSET_SCHEMA, AssetManifest, HOST_REVISION, MANIFEST_NAME, check_bundle};
pub use service::{AppService, OPS};
