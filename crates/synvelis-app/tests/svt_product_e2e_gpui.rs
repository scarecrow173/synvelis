//! SVT-01〜09 GPUI Test seam lane (synvelis#114 / SV-GUI-05).
//!
//! The product State/Receipt chain (view → `syn.*` op → receipt →
//! projection → display) is proven on the headless `FakeDriver` lane in
//! `tests/svt_product_e2e.rs` — `GpuiGuiTest` has no product service,
//! so this file covers the *view-layer* half of the SVT contracts on
//! the GPUI-simulated adapter: the same driver-neutral scenarios
//! `tests/common/scenario.rs` defines, mapped to their SVT cases.
//!
//! Evidence honesty: `GpuiSimulated` + `{SemanticAction, MultiWindow}`
//! only — never real OS window/input/pixel (GUI-L2) or native dock /
//! plugin session (EXT-L3); those stay `NotRun`/`Unsupported`.

#![cfg(feature = "gpui-test")]

mod common;

use common::scenario::{
    CASE_ARRANGEMENT_SELECT, CASE_FADER_PARITY, CASE_SEEK_PARITY, CASE_TRANSPORT_PLAY,
    KEY_ARRANGEMENT_TABLE, KEY_TRANSPORT_PLAY, assert_fader_parity, assert_honest_capabilities,
    assert_row_select, assert_seek_parity, assert_transport_play, fader_parity_scenario,
    invoke_scenario, screen_scenario_report, screen_state, secondary_seek_parity_scenario,
    seek_parity_scenario, select_row_scenario,
};
use ruviform_ui::view::{AttachEpochAllocator, ViewHost};
use ruviform_ui::{WindowId, WindowSpec};
use ruviform_ui_gpui::{GpuiGuiTest, ViewMount};
use ruviform_ui_test::{ScenarioDriver, ScenarioVerdict, SemanticGuiTest};
use synvelis_app::{
    main_screen_spec, main_view_instance, secondary_screen_spec, secondary_view_instance,
};

/// `spec` を `view` の `ViewHost` に mount し GPUI test window として
/// 立てる — `scenario_gpui.rs` と同じ production composition 形。
fn mount_gpui(
    gui: &mut GpuiGuiTest,
    view: ruviform_ui::dock::ViewInstanceId,
    spec: ruviform_ui::view::ViewSpec,
) -> WindowId {
    let mut host = ViewHost::new(view, AttachEpochAllocator::new().mint());
    host.mount(spec).expect("mount");
    host.drain_events();
    let window = gui
        .observer_mut()
        .driver_mut()
        .create_view_window(&WindowSpec::default(), ViewMount { host, theme: None })
        .expect("create_view_window");
    gui.settle().expect("settle mount");
    window
}

/// SVT-02/03/05 の view-layer 契約を GPUI-simulated adapter で実行:
/// selection intent、fader/seek gesture parity、transport play invoke。
/// `ScenarioReport` は adapter 実測の `GpuiSimulated` を記録する。
#[test]
fn svt_gpui_view_layer_scenarios_pass_with_declared_capabilities() {
    let state = screen_state();
    let mut gui = GpuiGuiTest::new();
    let window = mount_gpui(&mut gui, main_view_instance(), main_screen_spec(&state));
    let view = main_view_instance();

    // SVT-05 — seek gesture parity (Requested→Observed display settle
    // is asserted inside the shared oracle).
    let seek = seek_parity_scenario(&mut gui, window, view).expect("gpui seek");
    assert_seek_parity(&seek);
    let mut report = screen_scenario_report(
        &gui,
        "svt-gpui-seek",
        "synvelis.main.screen",
        CASE_SEEK_PARITY,
    );
    report.targets = vec![seek.target.clone()];
    report.verdict = ScenarioVerdict::Pass;
    assert_eq!(report.driver, ScenarioDriver::GpuiSimulated);
    assert_honest_capabilities(&report);

    // SVT-03 — fader gesture parity.
    let fader = fader_parity_scenario(&mut gui, window, view).expect("gpui fader");
    assert_fader_parity(&fader);
    let mut report = screen_scenario_report(
        &gui,
        "svt-gpui-fader",
        "synvelis.main.screen",
        CASE_FADER_PARITY,
    );
    report.targets = vec![fader.target.clone()];
    report.verdict = ScenarioVerdict::Pass;
    assert_honest_capabilities(&report);

    // SVT-05 — transport play Action intent.
    let play = invoke_scenario(&mut gui, window, view, KEY_TRANSPORT_PLAY).expect("gpui play");
    assert_transport_play(&play);
    let mut report = screen_scenario_report(
        &gui,
        "svt-gpui-play",
        "synvelis.main.screen",
        CASE_TRANSPORT_PLAY,
    );
    report.targets = vec![play.target.clone()];
    report.verdict = ScenarioVerdict::Pass;
    assert_honest_capabilities(&report);

    // SVT-02 — arrangement row select intent (view-local `select`).
    let select =
        select_row_scenario(&mut gui, window, view, KEY_ARRANGEMENT_TABLE, 0).expect("gpui select");
    assert_row_select(&select, 0);
    let mut report = screen_scenario_report(
        &gui,
        "svt-gpui-select",
        "synvelis.main.screen",
        CASE_ARRANGEMENT_SELECT,
    );
    report.targets = vec![select.target.clone()];
    report.verdict = ScenarioVerdict::Pass;
    assert_honest_capabilities(&report);

    // Secondary window view-layer parity (SVT-07 two-window aspect at
    // semantic level only — real OS two-window is GUI-L2 NotRun).
    let window2 = mount_gpui(
        &mut gui,
        secondary_view_instance(),
        secondary_screen_spec(&state),
    );
    let seek2 = secondary_seek_parity_scenario(&mut gui, window2, secondary_view_instance())
        .expect("gpui secondary seek");
    assert_seek_parity(&seek2);
}
