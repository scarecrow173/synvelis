//! `synvelis-app` — the Synvelis product application composition root
//! and GUI host boundary (Issue #110 / SV-GUI-01, Issue #175 /
//! SV-GPUI-01).
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
//! webview/GPUI type. Everything platform-dependent (wry/tao host,
//! embedded assets, the desktop entry points) sits behind the
//! non-default `webview`/`native` features so `cargo check
//! --workspace` stays a headless verification path. GUI, CLI, and
//! headless callers all reach the same [`ruviform_ui::UiRuntime`] +
//! [`Service`] contract, so the single-project-source-of-truth
//! invariant holds on every surface.
//!
//! SV-GPUI-01 adds the driver-neutral native lane: [`NativeShell`]
//! routes the RF386 [`ruviform_ui::WindowDriver`] contract onto the
//! same sealed runtime, [`ViewBinding`] is the typed adapter native
//! views speak, and the `synvelis-native` binary (feature `native`)
//! assembles the GPUI standard driver — the only place `gpui` types
//! enter the tree.
//!
//! ## Ops
//!
//! `syn.app.info`, `syn.app.capabilities`, `syn.app.quit`,
//! `syn.project.new`, `syn.project.open`, `syn.project.save`,
//! `syn.project.reload`, `syn.project.info`, `syn.transport.set`, `syn.workspace.mount` —
//! see [`service::OPS`].

pub mod app;
#[cfg(feature = "webview")]
pub mod assets;
pub mod binding;
pub mod engine;
pub mod error;
pub mod host;
pub mod logging;
pub mod monitoring;
pub mod recording;
pub mod revision;
pub mod screens;
pub mod service;
pub mod shell;
pub mod ui_ops;

pub use app::{AudioRuntimeSlot, ObservabilityOwner, RecorderSlot, SynvelisApp};
pub use binding::{BindingNotice, ProjectionRow, ViewBinding};
pub use engine::{SessionEngineConfig, SessionEngineError, SessionRoutingEngine};
pub use error::{AppError, AssetError, AssetErrorKind};
pub use host::{HeadlessWindowHost, ShellWindowHost};
pub use logging::{
    CliOverrides, EnvOverrides, HostLog, InstallState, LogInstallError, LogSettings, SettingsError,
    install, parse_args, to_logging_config,
};
pub use revision::{APP_ASSET_SCHEMA, AssetManifest, HOST_REVISION, MANIFEST_NAME, check_bundle};
pub use screens::{
    InspectorRow, InspectorScreenState, MainScreenState, MeterScreenState, MixerScreenState,
    TransportScreenState, VcsScreenState, VizScreenState, WaveformScreenState, history_screen_spec,
    main_screen_spec, main_view_id, main_view_instance, main_view_targets, mixer_gain_target,
    mixer_strip_targets, mount_main_view, mount_secondary_view, secondary_screen_spec,
    secondary_view_id, secondary_view_instance, secondary_view_targets, seek_range_seconds,
};
pub use service::{AppService, OPS};
pub use shell::NativeShell;
