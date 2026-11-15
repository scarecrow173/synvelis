//! Production composition root for the session routing engine and
//! its control surface (PC-C4 drain slice, synvelis#101).
//!
//! [`compose_routing_engine`] is the single place the product turns
//! "plugin registry + engine geometry" into a live
//! [`SessionRoutingEngine`] attached to [`SynvelisApp`]: it builds
//! [`PluginHostProcessors`] over a fresh `PluginHost`
//! ([`PluginHostProcessors::product`]), vends the static `ParamInfo`
//! catalog each plugin declares, hands the shared catalog cell and
//! the [`PluginReportDrain`] handle to the engine/app pair, and
//! attaches the engine as the session's
//! [`EnginePort`](synvelis_workspace::EnginePort). The app and
//! the entry binaries never name `PluginHost` — the shared
//! `Rc<RefCell>` cell stays inside the player adapter.
//!
//! Project recreate (`project.new`/`project.open`) detaches the
//! engine and clears the drain registration; re-composition for the
//! new session's media root is the deferred project-switch engine
//! wiring slice (SF-03 slice 5), not this root's job.

use std::path::PathBuf;

use ruviform_plugin_core::{ParamInfo, PluginFactory};

use synvelis_player::host::PluginHostProcessors;
use synvelis_project::{BuildBudget, BuildMode, RuntimeClockEpochRef, RuntimeInstanceId};

use crate::SynvelisApp;
use crate::engine::{SessionEngineConfig, SessionRoutingEngine};

/// One product plugin registration for [`compose_routing_engine`]:
/// the factory the host instantiates for `Scan`/`Instantiate` plus
/// the static [`ParamInfo`] table the composition root registers as
/// the connect-time catalog evidence (PC-C4-04 catalog vend — the
/// pinned host exposes no parameter enumeration for attached
/// instances, so the product declares it). An empty `params` table
/// vends no binds — typed rejections, never fabricated metadata.
pub struct ProductPlugin {
    /// Factory registered on the host.
    pub factory: Box<dyn PluginFactory>,
    /// Static parameter table vendored under the factory's
    /// descriptor id.
    pub params: Vec<ParamInfo>,
}

/// Composes and attaches the production routing engine: fresh
/// `PluginHost` + registered factories + per-plugin `ParamInfo`
/// tables → `PluginHostProcessors` → `SessionRoutingEngine` with
/// processor host, catalog cell, and report drain wired — then
/// attaches the engine to `app`'s session and registers the drain
/// so `ui_pump`/`drain_control_reports` drive the periodic
/// `drain_output_reports` pass. Returns the engine handle for the
/// caller-driven RT pump (`run`) and evidence reads.
///
/// Non-RT only — host construction and registration may allocate.
/// Re-composition over an already-attached engine replaces the app
/// handles; the previous engine's session port is superseded by the
/// new attach.
pub fn compose_routing_engine(
    app: &mut SynvelisApp,
    config: SessionEngineConfig,
    plugins: Vec<ProductPlugin>,
) -> SessionRoutingEngine {
    let mut param_tables = Vec::with_capacity(plugins.len());
    let mut factories = Vec::with_capacity(plugins.len());
    for plugin in plugins {
        param_tables.push((plugin.factory.descriptor().id, plugin.params));
        factories.push(plugin.factory);
    }
    let mut processors = PluginHostProcessors::product(factories);
    for (plugin_id, params) in param_tables {
        processors.register_params(plugin_id, params);
    }
    let cell = processors.param_catalog_cell();
    let drain = processors.report_drain();
    let engine = SessionRoutingEngine::new(config);
    engine.set_processor_host(Box::new(processors));
    engine.set_param_catalog(cell);
    app.attach_routing_engine(engine.clone());
    app.set_control_report_drain(drain);
    engine
}

/// Product routing composition for the desktop bins (PC-C4 webview
/// composition, synvelis#101): attaches the session routing engine
/// for a boot project directory — `media_root` is the verified blob
/// store beside the bundle at `<dir>/media`, the plugin registry is
/// empty (real VST3/OS factories are ruviform#253, `NotRun` —
/// plugin-bound processors fail typed `PluginUnavailable`, never
/// fabricated), and the engine geometry is the product Realtime
/// build ([`product_engine_config`]).
///
/// `project_dir` is the `SYNVELIS_PROJECT_DIR` value resolved by the
/// caller at the process boundary — the library never reads
/// `std::env` itself. `None` (unset) keeps the engine stub: an
/// untitled boot session has no media root to serve. Project
/// recreate (`project.new`/`project.open`) detaches the engine;
/// re-attach for the new session's media root is the deferred
/// project-switch engine wiring slice, not this root's job.
pub fn compose_product_routing(app: &mut SynvelisApp, project_dir: Option<PathBuf>) {
    let Some(dir) = project_dir else {
        return;
    };
    compose_routing_engine(app, product_engine_config(dir.join("media")), Vec::new());
}

/// Product engine geometry — 48 kHz/stereo/256-frame Realtime build,
/// single runtime instance/epoch, generation 1. Matches the native
/// bin's composition and the headless fixture geometry so composed
/// evidence is comparable.
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
