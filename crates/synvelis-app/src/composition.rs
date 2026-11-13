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

use ruviform_plugin_core::{ParamInfo, PluginFactory};

use synvelis_player::host::PluginHostProcessors;

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
