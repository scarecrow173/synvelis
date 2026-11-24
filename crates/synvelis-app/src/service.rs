//! [`AppService`] — the `syn.*` op surface.
//!
//! Registered on a [`ruviform_ui::UiRuntime`] and reached identically
//! from the GUI host (`HostEvent::Message`), headless drivers
//! (`UiRuntime::dispatch_op`), and any future CLI adapter — so every
//! surface exercises the same service contract and the same single
//! project/session source of truth.
//!
//! Op names live under `syn.*` — `sys.*` is reserved to the runtime.
//! Every op returns [`CallOutcome::Result`]; long-running work (none in
//! SV-GUI-01) would use [`CallOutcome::Job`].

use serde_json::{Value, json};

use ruviform_ui::ipc::{ErrorBody, codes};
use ruviform_ui::{CallOutcome, OpInfo, Service, ServiceContext};

use crate::app::SynvelisApp;
use crate::error::AppError;

/// The `syn.*` ops this service registers.
pub const OPS: &[OpInfo] = &[
    OpInfo {
        name: "syn.app.info",
        summary: "application revision, host/IPC revisions, backend probe, counters",
    },
    OpInfo {
        name: "syn.app.capabilities",
        summary: "backend + feature availability report (unavailable areas are listed)",
    },
    OpInfo {
        name: "syn.app.quit",
        summary: "request application shutdown — the host closes the window",
    },
    OpInfo {
        name: "syn.project.new",
        summary: "create a new project bundle at params.path (params.name/ppqn optional)",
    },
    OpInfo {
        name: "syn.project.open",
        summary: "open an existing project bundle at params.path",
    },
    OpInfo {
        name: "syn.project.save",
        summary: "save the live project to the open bundle",
    },
    OpInfo {
        name: "syn.project.reload",
        summary: "reopen the bundle this session was created or opened from (discards unsaved edits)",
    },
    OpInfo {
        name: "syn.project.info",
        summary: "project name, id, revision, saved state",
    },
    OpInfo {
        name: "syn.project.set_ppqn",
        summary: "change project PPQN — params.old_ppqn/new_ppqn/policy(/rounding)/expected_revision?",
    },
    OpInfo {
        name: "syn.project.preview_ppqn",
        summary: "dry-run project PPQN change — params.new_ppqn/policy(/rounding) → site counts, inexact/overflow, per-policy feasibility (read-only)",
    },
    OpInfo {
        name: "syn.project.import_smf",
        summary: "import an SMF Format 0/1 file as a new MIDI source — params.path, expected_revision? → source_id + import summary",
    },
    OpInfo {
        name: "syn.project.export_smf",
        summary: "export a MIDI source to an SMF file — params.source_id/path, format?, division? → conversion report (read-only)",
    },
    OpInfo {
        name: "syn.recording.info",
        summary: "recording state projection — sessions/passes/streams/segments/takes + recorder status",
    },
    OpInfo {
        name: "syn.project.undo",
        summary: "undo the last project command — {\"undone\": bool}",
    },
    OpInfo {
        name: "syn.project.redo",
        summary: "redo the last undone project command — {\"redone\": bool}",
    },
    OpInfo {
        name: "syn.signal_flow.view",
        summary: "Signal Flow projection — coverage/nodes/edges/status (view_id?, scope?, roles?, …)",
    },
    OpInfo {
        name: "syn.signal_flow.visual",
        summary: "visual-only Signal Flow op — move_node|set_group|fold|unfold|zoom|pan|link_focus|set_scope",
    },
    OpInfo {
        name: "syn.signal_flow.inspection",
        summary: "Signal Flow inspection focus — inspect|set_mode|set_follow_source|back|forward (Follow/Explore/Pinned)",
    },
    OpInfo {
        name: "syn.signal_flow.routing",
        summary: "Signal Flow port-drag routing gesture — begin|hover|drop|confirm|cancel|state",
    },
    OpInfo {
        name: "syn.vcs.view",
        summary: "VCS History/A-B Diff projection — status, rows, selection, freshness, diff (view_id)",
    },
    OpInfo {
        name: "syn.vcs.visual",
        summary: "VCS history op — open|close|refresh|load_more|select_node|pin|compare|view_mode|clear (read-only)",
    },
    OpInfo {
        name: "syn.library.providers",
        summary: "asset library provider catalog — registered providers with capability tags (ALIB-UX-H)",
    },
    OpInfo {
        name: "syn.library.visual",
        summary: "asset library view lifecycle — open|close (detached AssetLibrary view)",
    },
    OpInfo {
        name: "syn.library.view",
        summary: "asset library projection — providers + typed view state + deterministic ViewSpec",
    },
    OpInfo {
        name: "syn.library.action",
        summary: "one LibraryAction — provider.select|browse.*|search.*|entry.*|preview.*|import.*|drag.*",
    },
    OpInfo {
        name: "syn.autx.visual",
        summary: "AUTX surface lifecycle — open {surface} | close {view_id} (action-palette|macro-browser|workflow-editor|run-monitor)",
    },
    OpInfo {
        name: "syn.autx.catalog",
        summary: "push the authority automation catalog document {seq,capabilities,macros} — projections + request lowering read it",
    },
    OpInfo {
        name: "syn.autx.action",
        summary: "dispatch one autx.* surface command {view_id,target,action,params?} — view-local mutate or lowered wire requests",
    },
    OpInfo {
        name: "syn.variant-diff.visual",
        summary: "Difference Explorer lifecycle — open {target?} | close {view_id} (CB-09, synvelis#286)",
    },
    OpInfo {
        name: "syn.variant-diff.view",
        summary: "Difference Explorer projection — persisted view state + deterministic ViewSpec (view_id)",
    },
    OpInfo {
        name: "syn.variant-diff.action",
        summary: "one CB-09 action {view_id,action,...} — select|audition|set-target|compare|adopt-preview|adopt (explicit pins + approval ticket)",
    },
    OpInfo {
        name: "syn.transport.set",
        summary: "set transport rolling — params.playing (bool)",
    },
    OpInfo {
        name: "syn.workspace.mount",
        summary: "workspace mount-point summary (shell window, views, dock)",
    },
    OpInfo {
        name: "syn.window.close_guard",
        summary: "window-close guard reasons — unsaved_changes/recording_active/recorder_active",
    },
    OpInfo {
        name: "syn.view.attach",
        summary: "attach a logical view to a window (view_id, window_id)",
    },
    OpInfo {
        name: "syn.view.detach",
        summary: "detach a view — contractual gesture cancel + generation bump",
    },
    OpInfo {
        name: "syn.view.close",
        summary: "close a view — drops UI state, never project/audio/render",
    },
    OpInfo {
        name: "syn.view.list",
        summary: "view registry (view_id, window_id, attached, generation)",
    },
    OpInfo {
        name: "syn.view.select",
        summary: "select an arrangement row on a view — ObjectSelections + inspector focus",
    },
    OpInfo {
        name: "syn.window.adopt",
        summary: "idempotently register a driver-created window (window_id, title?, geometry?)",
    },
    OpInfo {
        name: "syn.window.list",
        summary: "workspace windows + docked/detached views + pending dock moves",
    },
    OpInfo {
        name: "syn.window.dock_prepare",
        summary: "begin two-phase move of view_id to existing window_id",
    },
    OpInfo {
        name: "syn.window.dock_prepare_new_window",
        summary: "begin dock move into a new driver window (title?) — async destination",
    },
    OpInfo {
        name: "syn.window.dock_commit",
        summary: "commit the pending dock move for view_id",
    },
    OpInfo {
        name: "syn.window.dock_abort",
        summary: "abort the pending dock move for view_id (failed?, detail?)",
    },
    OpInfo {
        name: "syn.window.dock_detach",
        summary: "move view_id into the workspace detached pool",
    },
    OpInfo {
        name: "syn.window.dock_reattach",
        summary: "move view_id from pool/window into window_id",
    },
    OpInfo {
        name: "syn.window.lifecycle",
        summary: "internal: NativeShell → workspace window lifecycle forwarding",
    },
    OpInfo {
        name: "syn.plugin_dock.open",
        summary: "adopt PluginDock view + declare slot + open editor session (view_id/window_id/instance)",
    },
    OpInfo {
        name: "syn.plugin_dock.hide",
        summary: "Attached → Hidden (editor tab inactive)",
    },
    OpInfo {
        name: "syn.plugin_dock.show",
        summary: "Hidden → Attached (editor tab active)",
    },
    OpInfo {
        name: "syn.plugin_dock.focus",
        summary: "keyboard focus on the editor (focused bool, non-fatal)",
    },
    OpInfo {
        name: "syn.plugin_dock.resize",
        summary: "request editor logical width×height",
    },
    OpInfo {
        name: "syn.plugin_dock.close",
        summary: "close the editor session, free lease + instance binding",
    },
    OpInfo {
        name: "syn.plugin_dock.save",
        summary: "PD-07 logical snapshot (instance/placement/size/visibility)",
    },
    OpInfo {
        name: "syn.plugin_dock.restore",
        summary: "adopt + slot + restore a saved editor snapshot",
    },
    OpInfo {
        name: "syn.plugin_dock.state",
        summary: "panel read model — view/window/slot/instance/session states",
    },
    OpInfo {
        name: "syn.plugin_dock.list",
        summary: "every live PluginDock panel's read model",
    },
    OpInfo {
        name: "syn.control.dispatch",
        summary: "dispatch CommandIntent + DispatchContext — ack only, receipts notify",
    },
    OpInfo {
        name: "syn.control.subscribe",
        summary: "subscribe (view_id, target) — initial projection pushed",
    },
    OpInfo {
        name: "syn.control.unsubscribe",
        summary: "release a (view_id, target) subscription",
    },
    OpInfo {
        name: "syn.control.targets",
        summary: "bounded bindable-target listing",
    },
    OpInfo {
        name: "syn.control.fast_control",
        summary: "point-in-time plugin param write — target/value/means/position_frames/project_revision → Delivered|Queued|Blocked",
    },
    OpInfo {
        name: "syn.control.status",
        summary: "control surface evidence — SurfaceReport/TickReport/ReportDrainReport/generation/active_generation/frame_base",
    },
    OpInfo {
        name: "syn.control.observe",
        summary: "observe one call window (processor/generation/call/span) → DispatchStage observations + Reported-only feed count",
    },
    OpInfo {
        name: "syn.control.relation",
        summary: "UI-C5 control relation inspection — view|inspect|set_mode|set_follow_source|back|forward → focus + RelationView",
    },
    OpInfo {
        name: "syn.control.edge",
        summary: "UI-C5 control layer edge gesture — begin|preview|commit|cancel|state (mod-source → param, fixed revision/window CAS)",
    },
    OpInfo {
        name: "syn.recording.status",
        summary: "UI-C5 recording status read model — conflict|prepare|pending|applied|unknown + provenance + legal actions",
    },
    OpInfo {
        name: "syn.recording.action",
        summary: "UI-C5 recording explicit actions — inspect|retry|rebase|abort",
    },
    OpInfo {
        name: "syn.probe.control",
        summary: "semantic probe lifecycle — request|set_mode|capture|stop_capture|remove (SF-04)",
    },
    OpInfo {
        name: "syn.probe.state",
        summary: "probe projection — bindings/segments/drain/traces",
    },
    OpInfo {
        name: "syn.probe.compare",
        summary: "multi-probe alignment comparison — params.request (ProbeComparisonRequest) → AcousticObservation (SF-04)",
    },
    OpInfo {
        name: "syn.monitoring.set",
        summary: "session-scope track monitor declaration — arm/monitor/binding/hw-direct patch or clear (RCD-03)",
    },
    OpInfo {
        name: "syn.monitoring.punch",
        summary: "punch in/out marks — set|in|out|clear (project ticks, half-open)",
    },
    OpInfo {
        name: "syn.monitoring.transport",
        summary: "monitor session transport — play|pause|stop|seek (project ticks)",
    },
    OpInfo {
        name: "syn.monitoring.session",
        summary: "monitor session lifecycle — start (preflight; loop/count-in/pre-roll declarations) | stop",
    },
    OpInfo {
        name: "syn.monitoring.latency",
        summary: "latency report/readiness/tone-difference injection (headless)",
    },
    OpInfo {
        name: "syn.monitoring.state",
        summary: "monitor projection — session/tracks/routes/notices/segments/latency",
    },
    OpInfo {
        name: "syn.recorder.start",
        summary: "recorder session start — preflight→input open/add→staging/writer→stream start (targets/streams/staging_root/gate params)",
    },
    OpInfo {
        name: "syn.recorder.pump",
        summary: "recorder non-RT drain — input work queues→engines→writers",
    },
    OpInfo {
        name: "syn.recorder.stop",
        summary: "recorder stop — stop streams then checkpoint+finalize writers (phase→stopped)",
    },
    OpInfo {
        name: "syn.recorder.commit",
        summary: "recorder commit — CloseSession+commit_session→Finalized takes/clips/sources",
    },
    OpInfo {
        name: "syn.recorder.discard",
        summary: "recorder discard — drop the active session without commit (best-effort CloseSession)",
    },
    OpInfo {
        name: "syn.recorder.status",
        summary: "recorder owner projection — phase/session/staging/fault/devices",
    },
    OpInfo {
        name: "syn.recorder.mark_take_verified",
        summary: "mark a take verified — params.take_id (session must be Open|Closed)",
    },
    OpInfo {
        name: "syn.viz.bind",
        summary: "bind surface to source (view_id, surface_id, source, kind, plot_px?)",
    },
    OpInfo {
        name: "syn.viz.meta",
        summary: "bounded surface row — status/meta/span/cache",
    },
    OpInfo {
        name: "syn.viz.query",
        summary: "bounded waveform window query (start, end, buckets, max_bytes?)",
    },
    OpInfo {
        name: "syn.viz.viewport",
        summary: "viewport intent forward — d | factor+at | start+end",
    },
    OpInfo {
        name: "syn.viz.select",
        summary: "selection range forward (start, end)",
    },
    OpInfo {
        name: "syn.viz.snapshot",
        summary: "latest-complete meter/spectrum frame row",
    },
    OpInfo {
        name: "syn.viz.register",
        summary: "register a manual viz source port (kind, source, …)",
    },
    OpInfo {
        name: "syn.viz.push_frame",
        summary: "push a meter/spectrum frame into a manual port",
    },
    OpInfo {
        name: "syn.viz.source_changed",
        summary: "mark a source revision-changed (stale contract)",
    },
];

/// [`Service`] over [`SynvelisApp`] — the GUI/headless op boundary.
pub struct AppService {
    app: SynvelisApp,
}

impl AppService {
    /// Wraps `app`.
    #[must_use]
    pub const fn new(app: SynvelisApp) -> Self {
        Self { app }
    }

    /// The composed application.
    #[must_use]
    pub const fn app(&self) -> &SynvelisApp {
        &self.app
    }

    /// Mutable application access — used by the host loop to drain
    /// the quit flag set by `syn.app.quit` and to push the initial
    /// app-state notify on `Ready`.
    #[must_use]
    pub const fn app_mut(&mut self) -> &mut SynvelisApp {
        &mut self.app
    }

    fn string_param<'a>(params: &'a Value, key: &str) -> Result<&'a str, ErrorBody> {
        params.get(key).and_then(Value::as_str).ok_or_else(|| {
            ErrorBody::new(
                codes::INVALID_PARAMS,
                format!("params.{key} (string) required"),
            )
        })
    }

    /// Optional `u64 → u32` param — absent or `null` means `None`;
    /// domain-range validation stays with the owning layer.
    fn optional_u32_param(params: &Value, key: &str) -> Result<Option<u32>, ErrorBody> {
        match params.get(key) {
            None | Some(Value::Null) => Ok(None),
            Some(v) => v
                .as_u64()
                .and_then(|n| u32::try_from(n).ok())
                .map(Some)
                .ok_or_else(|| {
                    ErrorBody::new(
                        codes::INVALID_PARAMS,
                        format!("params.{key} must be a u64 fitting u32"),
                    )
                }),
        }
    }

    /// Per-surface screen ops — signal-flow, VCS history, and the
    /// AUTX-07 automation surfaces (synvelis#266 SV266-NR-05). Returns
    /// `None` for ops outside the surface family so [`Self::dispatch`]
    /// keeps its flat table for everything else.
    fn dispatch_surface_ops(
        &mut self,
        op: &str,
        params: &Value,
    ) -> Option<Result<Value, ErrorBody>> {
        Some(match op {
            "syn.signal_flow.view" => self.app.op_signal_flow_view(params),
            "syn.signal_flow.visual" => self.app.op_signal_flow_visual(params),
            "syn.signal_flow.inspection" => self.app.op_signal_flow_inspection(params),
            "syn.signal_flow.routing" => self.app.op_signal_flow_routing(params),
            "syn.vcs.view" => self.app.op_vcs_view(params),
            "syn.vcs.visual" => self.app.op_vcs_visual(params),
            "syn.library.providers" => self.app.op_library_providers(params),
            "syn.library.visual" => self.app.op_library_visual(params),
            "syn.library.view" => self.app.op_library_view(params),
            "syn.library.action" => self.app.op_library_action(params),
            "syn.autx.visual" | "syn.autx.catalog" | "syn.autx.action" => {
                self.dispatch_autx(op, params)
            }
            "syn.variant-diff.visual" => self.app.op_variant_diff_visual(params),
            "syn.variant-diff.view" => self.app.op_variant_diff_view(params),
            "syn.variant-diff.action" => self.app.op_variant_diff_action(params),
            _ => return None,
        })
    }

    /// The three `syn.autx.*` ops share one dispatch arm — visual
    /// lifecycle, catalog push, and surface action all live on
    /// [`SynvelisApp`].
    fn dispatch_autx(&mut self, op: &str, params: &Value) -> Result<Value, ErrorBody> {
        match op {
            "syn.autx.visual" => self.app.op_autx_visual(params),
            "syn.autx.catalog" => self.app.op_autx_catalog(params),
            "syn.autx.action" => self.app.op_autx_action(params),
            _ => Err(ErrorBody::new(
                codes::INVALID_PARAMS,
                format!("unknown autx op {op}"),
            )),
        }
    }

    fn dispatch(&mut self, op: &str, params: &Value) -> Result<Value, ErrorBody> {
        if let Some(result) = self.dispatch_surface_ops(op, params) {
            return result;
        }
        match op {
            "syn.app.info" => Ok(self.app.app_info()),
            "syn.app.capabilities" => Ok(self.app.capabilities()),
            "syn.app.quit" => {
                self.app.request_quit();
                Ok(json!({ "quit": true }))
            }
            "syn.project.new" => {
                let path = Self::string_param(params, "path")?;
                let name = params.get("name").and_then(Value::as_str);
                let ppqn = Self::optional_u32_param(params, "ppqn")?;
                self.app
                    .project_new_with_ppqn(path, name, ppqn)
                    .map_err(AppError::into_body)
            }
            "syn.project.open" => {
                let path = Self::string_param(params, "path")?;
                self.app.project_open(path).map_err(AppError::into_body)
            }
            "syn.project.save" => self.app.project_save().map_err(AppError::into_body),
            "syn.project.reload" => self.app.project_reload().map_err(AppError::into_body),
            "syn.project.info" => Ok(self.app.project_info()),
            "syn.project.set_ppqn" => self.app.project_set_ppqn(params),
            "syn.project.preview_ppqn" => self.app.project_preview_ppqn(params),
            "syn.project.import_smf" => self.app.project_import_smf(params),
            "syn.project.export_smf" => self.app.project_export_smf(params),
            "syn.recording.info" => Ok(self.app.recording_info()),
            "syn.project.undo" => self.app.op_project_undo(),
            "syn.project.redo" => self.app.op_project_redo(),
            "syn.transport.set" => {
                let playing = params
                    .get("playing")
                    .and_then(Value::as_bool)
                    .ok_or_else(|| {
                        ErrorBody::new(
                            codes::INVALID_PARAMS,
                            "params.playing (bool) required".to_string(),
                        )
                    })?;
                self.app.transport_set(playing);
                Ok(json!({ "playing": playing }))
            }
            "syn.workspace.mount" => Ok(self.app.workspace_info()),
            "syn.window.close_guard" => Ok(json!({ "reasons": self.app.close_veto_reasons() })),
            "syn.view.attach" => self.app.op_view_attach(params),
            "syn.view.detach" => self.app.op_view_detach(params),
            "syn.view.close" => self.app.op_view_close(params),
            "syn.view.list" => Ok(self.app.op_view_list()),
            "syn.view.select" => self.app.op_view_select(params),
            "syn.window.adopt" => self.app.op_window_adopt(params),
            "syn.window.list" => Ok(self.app.op_window_list()),
            "syn.window.dock_prepare" => self.app.op_window_dock_prepare(params),
            "syn.window.dock_prepare_new_window" => {
                self.app.op_window_dock_prepare_new_window(params)
            }
            "syn.window.dock_commit" => self.app.op_window_dock_commit(params),
            "syn.window.dock_abort" => self.app.op_window_dock_abort(params),
            "syn.window.dock_detach" => self.app.op_window_dock_detach(params),
            "syn.window.dock_reattach" => self.app.op_window_dock_reattach(params),
            "syn.window.lifecycle" => self.app.op_window_lifecycle(params),
            name if name.starts_with("syn.plugin_dock.") => self.dispatch_plugin_dock(name, params),
            "syn.control.dispatch" => self.app.op_control_dispatch(params),
            "syn.control.subscribe" => self.app.op_control_subscribe(params),
            "syn.control.unsubscribe" => self.app.op_control_unsubscribe(params),
            "syn.control.targets" => Ok(self.app.op_control_targets()),
            "syn.control.fast_control" => self.app.op_control_fast_control(params),
            "syn.control.status" => Ok(self.app.op_control_status()),
            "syn.control.observe" => self.app.op_control_observe(params),
            "syn.control.relation" => self.app.op_control_relation(params),
            "syn.control.edge" => self.app.op_control_edge(params),
            "syn.recording.status" => self.app.op_recording_status(params),
            "syn.recording.action" => self.app.op_recording_action(params),
            "syn.probe.control" => self.app.op_probe_control(params),
            "syn.probe.state" => Ok(self.app.op_probe_state()),
            "syn.probe.compare" => self.app.op_probe_compare(params),
            "syn.monitoring.set" => self.app.op_monitoring_set(params),
            "syn.monitoring.punch" => self.app.op_monitoring_punch(params),
            "syn.monitoring.transport" => self.app.op_monitoring_transport(params),
            "syn.monitoring.session" => self.app.op_monitoring_session(params),
            "syn.monitoring.latency" => self.app.op_monitoring_latency(params),
            "syn.monitoring.state" => Ok(self.app.op_monitoring_state()),
            name if name.starts_with("syn.recorder.") => self.dispatch_recorder(name, params),
            "syn.viz.bind" => self.app.op_viz_bind(params),
            "syn.viz.meta" => self.app.op_viz_meta(params),
            "syn.viz.query" => self.app.op_viz_query(params),
            "syn.viz.viewport" => self.app.op_viz_viewport(params),
            "syn.viz.select" => self.app.op_viz_select(params),
            "syn.viz.snapshot" => self.app.op_viz_snapshot(params),
            "syn.viz.register" => self.app.op_viz_register(params),
            "syn.viz.push_frame" => self.app.op_viz_push_frame(params),
            "syn.viz.source_changed" => self.app.op_viz_source_changed(params),
            _ => Err(ErrorBody::new(
                codes::UNKNOWN_OP,
                format!("unhandled op: {op}"),
            )),
        }
    }

    /// `syn.recorder.*` family dispatch — recorder lifecycle ops
    /// (synvelis#114 SV-GUI-05 Option B) grouped out of the flat
    /// `dispatch` match so the table stays bounded as ops grow.
    fn dispatch_recorder(&mut self, op: &str, params: &Value) -> Result<Value, ErrorBody> {
        match op {
            "syn.recorder.start" => self.app.op_recorder_start(params),
            "syn.recorder.pump" => self.app.op_recorder_pump(params),
            "syn.recorder.stop" => self.app.op_recorder_stop(params),
            "syn.recorder.commit" => self.app.op_recorder_commit(params),
            "syn.recorder.discard" => self.app.op_recorder_discard(params),
            "syn.recorder.status" => Ok(self.app.op_recorder_status()),
            "syn.recorder.mark_take_verified" => self.app.op_recorder_mark_take_verified(params),
            _ => Err(ErrorBody::new(
                codes::UNKNOWN_OP,
                format!("unhandled recorder op: {op}"),
            )),
        }
    }

    /// `syn.plugin_dock.*` family dispatch — the SV-GPUI-02 headless
    /// slice's editor-dock surface (synvelis#176), grouped like
    /// `dispatch_recorder` so the flat table stays bounded.
    fn dispatch_plugin_dock(&mut self, op: &str, params: &Value) -> Result<Value, ErrorBody> {
        match op {
            "syn.plugin_dock.open" => self.app.op_plugin_dock_open(params),
            "syn.plugin_dock.hide" => self.app.op_plugin_dock_hide(params),
            "syn.plugin_dock.show" => self.app.op_plugin_dock_show(params),
            "syn.plugin_dock.focus" => self.app.op_plugin_dock_focus(params),
            "syn.plugin_dock.resize" => self.app.op_plugin_dock_resize(params),
            "syn.plugin_dock.close" => self.app.op_plugin_dock_close(params),
            "syn.plugin_dock.save" => self.app.op_plugin_dock_save(params),
            "syn.plugin_dock.restore" => self.app.op_plugin_dock_restore(params),
            "syn.plugin_dock.state" => self.app.op_plugin_dock_state(params),
            "syn.plugin_dock.list" => Ok(self.app.op_plugin_dock_list()),
            _ => Err(ErrorBody::new(
                codes::UNKNOWN_OP,
                format!("unhandled plugin dock op: {op}"),
            )),
        }
    }
}

impl Service for AppService {
    fn ops(&self) -> &'static [OpInfo] {
        OPS
    }

    fn call(
        &mut self,
        op: &str,
        params: &Value,
        _ctx: &mut ServiceContext<'_>,
    ) -> Result<CallOutcome, ErrorBody> {
        // Non-RT boundary: tracing is allowed here (service ops run on
        // the UI/host thread, never on an audio callback). Event fields
        // carry the op name only — params/bodies may contain paths.
        tracing::debug!(op, "syn op received");
        self.app.observability().record_intent();
        let result = self.dispatch(op, params);
        if let Err(error) = &result {
            self.app.observability().record_intent_error();
            tracing::warn!(op, code = %error.code, "syn op failed");
        }
        self.app.publish_project_length();
        result.map(CallOutcome::Result)
    }
}
