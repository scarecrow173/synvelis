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
//! The S2 native-slot lane below mounts the product
//! `plugin_dock_screen_spec` (same `plugin.editor` key), declares a
//! zeroed Hidden `SlotSpec` on the driver's canonical shared surface
//! (`GpuiDriver::surface()` — never a fresh `GpuiSurfaceAdapter::new`,
//! which would own a disconnected ledger), binds it before the first
//! paint, and observes the renderer's geometry/visibility projection
//! plus the unbind suppression purely through the canonical public
//! ledger API (`spec`/`state`/`drain_events`).
//!
//! `GpuiSimulated` pass is toolkit-simulated semantic/layout evidence
//! only — never real-OS window/input/pixel or native-dock proof.
//! gpui-test deliberately captures no raw parent handle
//! (`capture_raw_parent` is `None` under `gpui-test`), so this lane
//! never leases/redeems: no native container, child-HWND, pixel, or
//! input evidence can exist here, and none is claimed.
//! PD-01〜07 Real OS / Real Plugin lanes stay `NotRun`
//! (`evidence/sv176/`).

#![cfg(feature = "gpui-test")]

use std::cell::Cell;
use std::collections::BTreeSet;

use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::native_slot::{
    LogicalRect, SlotCapabilities, SlotError, SlotGeometry, SlotSpec, SlotVisibility, Support,
    SurfaceSlotId,
};
// Windows-only positive lane — the container backend exists only on
// Windows upstream, so the projection/visibility observations compile
// there alone; the non-Windows lane asserts the defined refusal instead.
#[cfg(target_os = "windows")]
use ruviform_ui::native_slot::{SlotEvent, SlotState};
use ruviform_ui::view::{AttachEpochAllocator, ComponentKey, ViewHost};
use ruviform_ui::{UiError, WindowId, WindowSpec};
use ruviform_ui_gpui::{GpuiGuiTest, ViewMount};
use ruviform_ui_test::{
    CaseId, PLUGIN_DOCK_SCENARIO, PLUGIN_DOCK_SLOT_KEY, RunId, ScenarioCapability, ScenarioDriver,
    ScenarioError, ScenarioId, ScenarioOutcome, ScenarioSpec, ScenarioVerdict, SemanticGuiTest,
    TargetQuery, plugin_dock_spec, plugin_dock_view_spec, run_scenario,
};
use synvelis_app::{PluginDockStatus, plugin_dock_screen_spec};

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

// ---------------------------------------------------------------------
// S2 native-slot lane — canonical-surface binding/projection evidence.
//
// Everything below is semantic/layout ledger evidence only. gpui-test
// captures no raw parent handle, so no lease is ever minted here: the
// lane never exercises `lease_parent`/`redeem`, never observes a native
// container/child HWND, and claims no pixel or input evidence.
// ---------------------------------------------------------------------

/// An empty `PluginDockStatus` projection for `view` — no binding, no
/// session; the spec shape is what matters here.
fn empty_status(view: ViewInstanceId) -> PluginDockStatus {
    PluginDockStatus {
        view,
        window: None,
        slot: None,
        slot_state: None,
        instance: None,
        session: None,
        session_state: None,
        placement: None,
        size: None,
    }
}

/// Mounts the product `plugin_dock_screen_spec` through the production
/// composition shape (`ViewHost` + `create_view_window`) and returns the
/// reserved [`WindowId`] *without* settling — the slot lane must declare
/// and bind before the first paint.
fn create_product_plugin_dock_window(gui: &mut GpuiGuiTest, view: ViewInstanceId) -> WindowId {
    let mut host = ViewHost::new(view, AttachEpochAllocator::new().mint());
    host.mount(plugin_dock_screen_spec(&empty_status(view)))
        .expect("mount product plugin dock spec");
    host.drain_events();
    gui.observer_mut()
        .driver_mut()
        .create_view_window(&WindowSpec::default(), ViewMount { host, theme: None })
        .expect("create_view_window")
}

/// Declared capabilities for the lane's spec — mirrors the upstream
/// `rvf_view_native_slot` test declaration so the renderer's
/// geometry/clip/visibility projection is exercisable on the ledger.
/// This is the spec-side vocabulary for the simulated ledger only: no
/// OS container ever exists under gpui-test, so nothing here is a claim
/// about real-OS clip/hide-show capability.
fn lane_slot_capabilities() -> SlotCapabilities {
    SlotCapabilities {
        attach_parent: Support::Supported,
        hide_show: Support::Supported,
        clip: Support::Supported,
        ..SlotCapabilities::default()
    }
}

/// A zeroed Hidden [`SlotSpec`] owned by `window`/`view` — the
/// pre-paint declaration the renderer projection overwrites with real
/// laid-out geometry once bound.
fn zero_hidden_slot_spec(window: WindowId, view: ViewInstanceId) -> SlotSpec {
    SlotSpec {
        window,
        view,
        geometry: SlotGeometry {
            rect: LogicalRect {
                x: 0.0,
                y: 0.0,
                width: 0.0,
                height: 0.0,
            },
            scale_factor: 1.0,
        },
        visibility: SlotVisibility::Hidden,
        capabilities: lane_slot_capabilities(),
    }
}

/// `bind_native_slot` refusal vocabulary is stringly typed upstream
/// (`UiError::Host` carrying a reason string) — assert the refusal and
/// its variant honestly without pinning exact wording.
fn assert_host_refusal(result: Result<(), UiError>, case: &str) {
    match result {
        Err(UiError::Host { .. }) => {}
        other => panic!("{case} must be refused as UiError::Host: {other:?}"),
    }
}

/// Windows positive lane: declare a zeroed Hidden slot on the canonical
/// shared surface for the reserved window, bind `plugin.editor` before
/// the first paint, and observe through the canonical public ledger API
/// that the renderer projection writes non-empty geometry + Visible.
/// Unbinding then suppresses the slot back to Hidden on the next painted
/// frame — the same suppression an inactive dock tab would produce.
///
/// Semantic/layout evidence only: no lease, no container, no pixel/input
/// evidence (gpui-test captures no raw parent — see module header).
#[cfg(target_os = "windows")]
#[test]
fn gpui_native_slot_projection_visible_then_unbind_suppresses_hidden() {
    let mut gui = GpuiGuiTest::new();
    let view = ViewInstanceId::new(1).expect("view id 1 is non-zero");
    // The product slot key — identical to upstream `PLUGIN_DOCK_SLOT_KEY`.
    let key = ComponentKey(synvelis_app::PLUGIN_DOCK_SLOT_KEY.to_string());
    assert_eq!(synvelis_app::PLUGIN_DOCK_SLOT_KEY, PLUGIN_DOCK_SLOT_KEY);

    // Reserve the window through the production composition shape — no
    // settle yet: declare + bind must land before the first paint so the
    // initial frame already projects the slot.
    let window = create_product_plugin_dock_window(&mut gui, view);

    // The canonical shared surface — the one ledger the mounted view
    // projects onto (never a fresh `GpuiSurfaceAdapter::new`).
    let surface = gui.observer().driver().surface();
    let slot = surface
        .declare_slot(zero_hidden_slot_spec(window, view))
        .expect("declare zeroed Hidden slot on canonical surface");
    gui.observer_mut()
        .driver_mut()
        .bind_native_slot(window, key.clone(), slot)
        .expect("bind plugin.editor before first paint");

    gui.settle().expect("settle first paint");

    // Painted + bound → real geometry and Visible in the canonical ledger.
    let projected = surface
        .spec(slot)
        .expect("surface borrow")
        .expect("slot spec live on canonical surface");
    assert!(
        projected.geometry.rect.width > 0.0 && projected.geometry.rect.height > 0.0,
        "painted bounds projected into the canonical ledger: {:?}",
        projected.geometry.rect
    );
    assert_eq!(
        projected.visibility,
        SlotVisibility::Visible,
        "painted bound slot projects Visible"
    );
    assert_eq!(
        surface.state(slot).expect("surface borrow"),
        Some(SlotState::Declared),
        "no lease is minted in this lane — the slot stays Declared"
    );
    let events = surface.drain_events().expect("drain canonical events");
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SlotEvent::GeometryChanged { slot: s } if *s == slot)),
        "geometry projection recorded on the canonical ledger: {events:?}"
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SlotEvent::VisibilityChanged { slot: s } if *s == slot)),
        "visibility projection recorded on the canonical ledger: {events:?}"
    );

    // Unbind → the slot leaves the painted set, so the next painted
    // frame suppresses it Hidden. GPUI repaints only on invalidation;
    // the public `update_view` seam re-applies the (unchanged) spec,
    // which notifies the view and forces that frame.
    gui.observer_mut()
        .driver_mut()
        .unbind_native_slot(window, &key)
        .expect("unbind plugin.editor");
    gui.observer_mut()
        .driver_mut()
        .update_view(window, plugin_dock_screen_spec(&empty_status(view)))
        .expect("update_view repaint after unbind");
    gui.settle().expect("settle suppression frame");

    let suppressed = surface
        .spec(slot)
        .expect("surface borrow")
        .expect("slot spec live on canonical surface");
    assert_eq!(
        suppressed.visibility,
        SlotVisibility::Hidden,
        "unbound slot suppressed Hidden on the canonical ledger"
    );
}

/// `bind_native_slot` refusals that need no live slot — valid on every
/// OS: an unknown window refuses, an unknown slot on a live window
/// refuses, and the canonical ledger itself answers a phantom id with
/// the typed `SlotError::UnknownSlot` (never a fake success).
#[test]
fn gpui_bind_native_slot_refuses_unknown_window_and_unknown_slot() {
    let mut gui = GpuiGuiTest::new();
    let view = ViewInstanceId::new(1).expect("view id 1 is non-zero");
    let window = create_product_plugin_dock_window(&mut gui, view);
    let key = ComponentKey(synvelis_app::PLUGIN_DOCK_SLOT_KEY.to_string());
    // A phantom id nothing ever declared — construction is
    // `#[doc(hidden)]` upstream because minting is registry-internal;
    // here it only ever plays the *invalid* input.
    let phantom = SurfaceSlotId::new(0, 0);

    let unknown_window = WindowId::new(u64::MAX).expect("nonzero");
    assert_host_refusal(
        gui.observer_mut()
            .driver_mut()
            .bind_native_slot(unknown_window, key.clone(), phantom),
        "bind on an unknown window",
    );

    assert_host_refusal(
        gui.observer_mut()
            .driver_mut()
            .bind_native_slot(window, key.clone(), phantom),
        "bind of an undeclared slot on a live window",
    );

    let surface = gui.observer().driver().surface();
    assert!(
        matches!(surface.release_slot(phantom), Err(SlotError::UnknownSlot)),
        "the canonical ledger answers a phantom id with the typed refusal"
    );
}

/// `bind_native_slot` refusal matrix needing live declared slots —
/// Windows-only because `declare_slot` is a defined refusal
/// (`SlotError::Unsupported`) on OSes without the container backend:
/// duplicate key, slot already bound to another key, cross-window slot,
/// and released (stale) slot all refuse as `UiError::Host`.
#[cfg(target_os = "windows")]
#[test]
fn gpui_bind_native_slot_refuses_duplicate_cross_window_and_released() {
    let mut gui = GpuiGuiTest::new();
    let view = ViewInstanceId::new(1).expect("view id 1 is non-zero");
    let view2 = ViewInstanceId::new(2).expect("view id 2 is non-zero");
    let window = create_product_plugin_dock_window(&mut gui, view);
    let window2 = create_product_plugin_dock_window(&mut gui, view2);
    let key = ComponentKey(synvelis_app::PLUGIN_DOCK_SLOT_KEY.to_string());
    let surface = gui.observer().driver().surface();

    let slot = surface
        .declare_slot(zero_hidden_slot_spec(window, view))
        .expect("declare slot");
    gui.observer_mut()
        .driver_mut()
        .bind_native_slot(window, key.clone(), slot)
        .expect("first bind is legitimate");

    // Already-bound key — a second slot for the same key refuses.
    let slot2 = surface
        .declare_slot(zero_hidden_slot_spec(window, view))
        .expect("declare second slot");
    assert_host_refusal(
        gui.observer_mut()
            .driver_mut()
            .bind_native_slot(window, key.clone(), slot2),
        "re-bind of an already-bound key",
    );

    // Slot already bound — the same slot under a second key refuses
    // (one slot paints into one region).
    let other_key = ComponentKey("plugin.editor.other".to_string());
    assert_host_refusal(
        gui.observer_mut()
            .driver_mut()
            .bind_native_slot(window, other_key.clone(), slot),
        "bind of an already-bound slot under a second key",
    );

    // Cross-window — a slot declared on `window2` refuses to bind in
    // `window` (the slot's owner window is part of its declaration).
    let foreign = surface
        .declare_slot(zero_hidden_slot_spec(window2, view2))
        .expect("declare foreign-window slot");
    assert_host_refusal(
        gui.observer_mut()
            .driver_mut()
            .bind_native_slot(window, other_key.clone(), foreign),
        "bind of a slot owned by a different window",
    );

    // Released/stale — a released slot refuses to bind.
    let released = surface
        .declare_slot(zero_hidden_slot_spec(window, view))
        .expect("declare slot to release");
    surface.release_slot(released).expect("release slot");
    assert_host_refusal(
        gui.observer_mut()
            .driver_mut()
            .bind_native_slot(window, other_key.clone(), released),
        "bind of a released slot",
    );
}

/// Non-Windows defined refusal: the upstream container backend is
/// Windows-only, so the canonical `declare_slot` refuses with
/// `SlotError::Unsupported` — an explicit, typed refusal, never a fake
/// success and never a silently skipped positive lane.
#[cfg(not(target_os = "windows"))]
#[test]
fn gpui_declare_native_slot_refuses_unsupported_without_container_backend() {
    let mut gui = GpuiGuiTest::new();
    let view = ViewInstanceId::new(1).expect("view id 1 is non-zero");
    let window = create_product_plugin_dock_window(&mut gui, view);
    gui.settle().expect("settle mount");

    let surface = gui.observer().driver().surface();
    let result = surface.declare_slot(zero_hidden_slot_spec(window, view));
    assert!(
        matches!(result, Err(SlotError::Unsupported)),
        "non-Windows canonical declare_slot must be a defined refusal: {result:?}"
    );
}
