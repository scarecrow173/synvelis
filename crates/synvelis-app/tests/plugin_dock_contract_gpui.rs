//! synvelis#176 (SV-GPUI-02 / PD-01〜07) — GPUI-simulated lane of the
//! headless plugin-dock contract (`gpui-test` feature only).
//!
//! Runs the identical `rf631.plugin-dock`/`view-mount` body and the
//! `plugin_dock_spec` capability gate on `GpuiGuiTest`
//! (`GpuiSimulated`) that `plugin_dock_contract.rs` runs on
//! `FakeGuiTest` — the production composition shape (`ViewHost` +
//! `create_view_window`) mounts the shared `plugin_dock_view_spec`
//! and the helper's `{NativePluginDock, VisualCapture}` requirement
//! still pre-flights to `Unsupported`.
//!
//! `GpuiSimulated` pass is toolkit-simulated semantic evidence
//! only — never real-OS window/input/pixel or native-dock proof.
//! PD-01〜07 Real OS / Real Plugin lanes stay `NotRun`
//! (`evidence/sv176/`).

#![cfg(feature = "gpui-test")]

use std::cell::Cell;
use std::collections::BTreeSet;

use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::view::{AttachEpochAllocator, ComponentKey, ViewHost};
use ruviform_ui::{WindowId, WindowSpec};
use ruviform_ui_gpui::{GpuiGuiTest, ViewMount};
use ruviform_ui_test::{
    CaseId, PLUGIN_DOCK_SCENARIO, PLUGIN_DOCK_SLOT_KEY, RunId, ScenarioCapability, ScenarioDriver,
    ScenarioError, ScenarioId, ScenarioOutcome, ScenarioSpec, ScenarioVerdict, SemanticGuiTest,
    TargetQuery, plugin_dock_spec, plugin_dock_view_spec, run_scenario,
};

/// `plugin_dock_view_spec` を production composition と同じ形
/// (`ViewHost` + `create_view_window`)で GPUI test window に mount
/// する — `scenario_gpui.rs` の `mount_gpui` と同じ手順。
fn mount_plugin_dock_gpui(gui: &mut GpuiGuiTest, view: ViewInstanceId) -> WindowId {
    let mut host = ViewHost::new(view, AttachEpochAllocator::new().mint());
    host.mount(plugin_dock_view_spec()).expect("mount");
    host.drain_events();
    let window = gui
        .observer_mut()
        .driver_mut()
        .create_view_window(&WindowSpec::default(), ViewMount { host, theme: None })
        .expect("create_view_window");
    gui.settle().expect("settle mount");
    window
}

/// `rf631.plugin-dock`/`view-mount` の scenario body — Fake lane と
/// 同一の driver-neutral 実装。
fn view_mount_body(
    gui: &mut impl SemanticGuiTest,
    window: WindowId,
    view: ViewInstanceId,
) -> Result<ScenarioOutcome, ScenarioError> {
    let slot_key = ComponentKey(PLUGIN_DOCK_SLOT_KEY.to_string());
    let mut query = TargetQuery::new(window, view);
    query.component = Some(slot_key.clone());
    let target = gui.resolve_target(&query)?;
    gui.observe(&target, &slot_key)?;

    let mut outcome = ScenarioOutcome::new(ScenarioVerdict::Pass);
    outcome.targets = vec![target];
    outcome
        .evidence
        .push(format!("semantic-mount:native_slot:{PLUGIN_DOCK_SLOT_KEY}"));
    outcome.detail = Some(
        "semantic view mount only — contract conformance, not dock/window/pixel evidence"
            .to_string(),
    );
    Ok(outcome)
}

/// `view-mount` case on `GpuiSimulated`: the shared spec mounts
/// through the GPUI production composition shape, the `plugin.editor`
/// slot resolves and observes, and the report records the adapter's
/// honest `GpuiSimulated` + `{SemanticAction, MultiWindow}` — never
/// promoted to real-OS/pixel/native-dock evidence.
#[test]
fn gpui_view_mount_case_passes_with_declared_capabilities() {
    let mut gui = GpuiGuiTest::new();
    let view = ViewInstanceId::new(1).expect("view id 1 is non-zero");
    let window = mount_plugin_dock_gpui(&mut gui, view);

    let spec = ScenarioSpec::new(
        RunId::new("sv176-pd-view-mount-gpui"),
        ScenarioId::new(PLUGIN_DOCK_SCENARIO),
    )
    .case(CaseId::new("view-mount"))
    .required(BTreeSet::from([ScenarioCapability::SemanticAction]));

    let report = run_scenario(&mut gui, &spec, |gui| view_mount_body(gui, window, view));

    assert_eq!(report.verdict, ScenarioVerdict::Pass);
    assert_eq!(report.verdict.lane_status(), "pass");
    assert_eq!(report.driver, ScenarioDriver::GpuiSimulated);
    assert_eq!(
        report.capabilities,
        BTreeSet::from([
            ScenarioCapability::SemanticAction,
            ScenarioCapability::MultiWindow,
        ])
    );
    assert!(
        !report
            .capabilities
            .contains(&ScenarioCapability::NativePluginDock)
    );
    assert!(
        !report
            .capabilities
            .contains(&ScenarioCapability::VisualCapture)
    );
    assert!(
        report
            .detail
            .as_deref()
            .is_some_and(|d| d.contains("not dock/window/pixel evidence")),
        "report must keep the honesty detail: {:?}",
        report.detail
    );
}

/// spec-helper gate on `GpuiSimulated`: `plugin_dock_spec`'s
/// `{NativePluginDock, VisualCapture}` requirement is not claimed by
/// the adapter, so the body never runs and the report is
/// `Unsupported` — a spec requiring native-dock/pixel evidence
/// cannot emit `Pass` on a semantic-simulated adapter.
/// (Upstream `rf631.plugin-dock` probe cases use `required = {}`
/// `probe_spec` and gate on artifact presence inside the body —
/// this test exercises the `plugin_dock_spec` helper only.)
#[test]
fn gpui_plugin_dock_spec_gate_is_unsupported_before_the_body_runs() {
    let mut gui = GpuiGuiTest::new();
    let ran = Cell::new(false);
    let spec = plugin_dock_spec(
        RunId::new("sv176-pd-dock-gate-gpui"),
        CaseId::new("editor-attach"),
        None,
    );

    let report = run_scenario(&mut gui, &spec, |_| {
        ran.set(true);
        Ok(ScenarioOutcome::new(ScenarioVerdict::Pass))
    });

    assert!(!ran.get(), "body must not run on capability failure");
    assert_eq!(report.verdict, ScenarioVerdict::Unsupported);
    assert_eq!(report.verdict.lane_status(), "unsupported");
    assert_eq!(report.driver, ScenarioDriver::GpuiSimulated);
    assert_eq!(
        report.required,
        BTreeSet::from([
            ScenarioCapability::NativePluginDock,
            ScenarioCapability::VisualCapture,
        ])
    );
}
