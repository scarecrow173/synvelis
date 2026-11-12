//! SV-GUI-03 slice 1+5 (synvelis#112) — `SemanticGuiTest`/
//! `FakeGuiTest` scenario coverage for the product screen specs.
//!
//! Runs the shared ruviform #464 contract against
//! [`main_screen_spec`]/[`secondary_screen_spec`] through the
//! driver-neutral scenario bodies in `tests/common/scenario.rs` —
//! the same functions `tests/scenario_gpui.rs` runs on
//! `GpuiGuiTest` (`gpui-test` feature). Fake evidence only —
//! `ScenarioDriver::Fake` claims `SemanticAction`/`MultiWindow`,
//! never OS/pointer/pixel proof; the app-side `ControlsUi` dispatch
//! is proven end-to-end in `view_mount.rs`.

mod common;

use std::collections::BTreeSet;

use common::scenario::{
    CASE_ARRANGEMENT_SELECT, CASE_FADER_PARITY, CASE_READ_ONLY_INSPECTOR, CASE_SECONDARY_SEEK,
    CASE_SEEK_PARITY, CASE_TAB_CYCLE, CASE_TRANSPORT_PLAY, KEY_ARRANGEMENT_TABLE,
    KEY_INSPECTOR_TABLE, KEY_SCREEN_TABS, KEY_SEEK_POSITION, KEY_TRANSPORT_PLAY,
    KEY_TRANSPORT_PLAY_SECONDARY, SCENARIO_MAIN_SCREEN, SCENARIO_SECONDARY_SCREEN,
    TAB_CYCLE_INDICES, assert_fader_parity, assert_honest_capabilities, assert_row_select,
    assert_seek_parity, assert_stale_generation_pin_rejected, assert_tab_cycle,
    assert_transport_play, assert_unclaimed_capabilities_rejected, assert_visual_honesty,
    command_actions, fader_parity_scenario, inspector_table_is_read_only, invoke_scenario,
    screen_scenario_report, screen_state, secondary_seek_parity_scenario, seek_parity_scenario,
    select_row_scenario, tab_cycle_scenario,
};
use ruviform_ui::WindowId;
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::view::{ComponentKey, ViewSpec};
use ruviform_ui_test::{
    FakeGuiTest, ScenarioCapability, ScenarioDriver, ScenarioError, ScenarioVerdict,
    SemanticGuiTest, TargetQuery,
};
use serde_json::json;

use synvelis_app::{
    MainScreenState, main_screen_spec, main_view_instance, secondary_screen_spec,
    secondary_view_instance,
};

/// Mounts `spec` as `view` on a fresh fake window.
fn mount_view(gui: &mut FakeGuiTest, view: ViewInstanceId, spec: ViewSpec) -> WindowId {
    let window = gui.open_window();
    gui.mount_view(window, view, spec)
        .expect("spec mounts on the fake adapter");
    window
}

/// The main screen spec on a fresh fake window.
fn mount_main(gui: &mut FakeGuiTest, state: &MainScreenState) -> WindowId {
    mount_view(gui, main_view_instance(), main_screen_spec(state))
}

/// The secondary transport-deck spec on a fresh fake window.
fn mount_secondary(gui: &mut FakeGuiTest, state: &MainScreenState) -> WindowId {
    mount_view(gui, secondary_view_instance(), secondary_screen_spec(state))
}

/// The shared #464 gesture-parity scenario passes against the
/// product spec: `Begin → Preview → Commit` on `seek.position` mints
/// the `transport/position` command intents, the `Applied` receipt
/// and the authoritative position projection land through
/// `post_event` (the same `deliver_event` decode every driver uses),
/// and the controlled display settles on the seconds value — the
/// synvelis#175 seconds contract.
#[test]
fn seek_gesture_and_projection_follow_the_shared_parity_scenario() {
    let mut gui = FakeGuiTest::new();
    let window = mount_main(&mut gui, &screen_state());
    let view = main_view_instance();

    let outcome = seek_parity_scenario(&mut gui, window, view).expect("seek scenario runs");
    assert_seek_parity(&outcome);

    let mut report = screen_scenario_report(
        &gui,
        "run-fake-seek",
        SCENARIO_MAIN_SCREEN,
        CASE_SEEK_PARITY,
    );
    report.targets = vec![outcome.target];
    report.observed = Some(json!({
        "intents": outcome.intents.len(),
        "phase": format!("{:?}", outcome.snapshot.phase),
        "observed_value": outcome.snapshot.observed.as_ref().map(|o| o.value.clone()),
    }));
    report.verdict = ScenarioVerdict::Pass;

    assert_eq!(report.driver, ScenarioDriver::Fake);
    assert_eq!(report.verdict, ScenarioVerdict::Pass);
    assert_eq!(report.verdict.lane_status(), "pass");
    assert_honest_capabilities(&report);
}

/// The Mixer `Fader` rides the same `Begin → Preview → Commit` wire
/// path on its tagged param target — commit 0.4 lands an `Applied`
/// receipt and the authoritative projection settles the controlled
/// display (synvelis#185 domain contract).
#[test]
fn fader_gesture_and_projection_follow_the_shared_parity_scenario() {
    let mut gui = FakeGuiTest::new();
    let window = mount_main(&mut gui, &screen_state());
    let view = main_view_instance();

    let outcome = fader_parity_scenario(&mut gui, window, view).expect("fader scenario runs");
    assert_fader_parity(&outcome);

    let mut report = screen_scenario_report(
        &gui,
        "run-fake-fader",
        SCENARIO_MAIN_SCREEN,
        CASE_FADER_PARITY,
    );
    report.targets = vec![outcome.target];
    report.observed = Some(json!({
        "intents": outcome.intents.len(),
        "phase": format!("{:?}", outcome.snapshot.phase),
        "observed_value": outcome.snapshot.observed.as_ref().map(|o| o.value.clone()),
    }));
    report.verdict = ScenarioVerdict::Pass;

    assert_eq!(report.driver, ScenarioDriver::Fake);
    assert_eq!(report.verdict, ScenarioVerdict::Pass);
    assert_honest_capabilities(&report);
}

/// The Transport Play `Button` mints one discrete `Action` intent on
/// the `transport` target — the intent `ControlsUi::route_action`
/// resolves to `transport_play` on the app side (the end-to-end
/// dispatch is proven in `view_mount.rs`). The Applied receipt lands
/// on the button's command binding, and the Inspector `Table` stays
/// read-only — row selection is not a legal op on it.
#[test]
fn transport_play_invokes_and_inspector_table_is_read_only() {
    let mut gui = FakeGuiTest::new();
    let window = mount_main(&mut gui, &screen_state());
    let view = main_view_instance();

    // Play press → one Action intent on `transport`.
    let outcome = invoke_scenario(&mut gui, window, view, KEY_TRANSPORT_PLAY).expect("play invoke");
    assert_transport_play(&outcome);

    let mut report = screen_scenario_report(
        &gui,
        "run-fake-play",
        SCENARIO_MAIN_SCREEN,
        CASE_TRANSPORT_PLAY,
    );
    report.targets = vec![outcome.target];
    report.observed = Some(json!({
        "actions": command_actions(&outcome.intents),
        "phase": format!("{:?}", outcome.snapshot.phase),
    }));
    report.verdict = ScenarioVerdict::Pass;
    assert_honest_capabilities(&report);

    // The Inspector table mounts read-only: no selection, and a row
    // select is rejected rather than silently accepted.
    assert!(
        inspector_table_is_read_only(&mut gui, window, view).expect("read-only check"),
        "inspector table must reject row selection"
    );
    let read_only = screen_scenario_report(
        &gui,
        "run-fake-inspector-readonly",
        SCENARIO_MAIN_SCREEN,
        CASE_READ_ONLY_INSPECTOR,
    );
    assert_eq!(read_only.driver, ScenarioDriver::Fake);
    assert_eq!(read_only.verdict.lane_status(), "not_run");
}

/// The Arrangement `Table` is selectable (synvelis#112 slice 4): a
/// row press mints the view-local `select` Action on
/// `VIEW_STATE_TARGET` with `params {"row": n}`, and the `screen.tabs`
/// strip tours the four non-default tabs — each press mints a
/// `tab-select` Action with `params {"index": i}`.
#[test]
fn arrangement_select_row_and_tab_cycle_mint_view_state_actions() {
    let mut gui = FakeGuiTest::new();
    let window = mount_main(&mut gui, &screen_state());
    let view = main_view_instance();

    let selected = select_row_scenario(&mut gui, window, view, KEY_ARRANGEMENT_TABLE, 0)
        .expect("arrangement row select");
    assert_row_select(&selected, 0);
    let mut report = screen_scenario_report(
        &gui,
        "run-fake-arrangement-select",
        SCENARIO_MAIN_SCREEN,
        CASE_ARRANGEMENT_SELECT,
    );
    report.targets = vec![selected.target];
    report.observed = Some(json!({
        "actions": command_actions(&selected.intents),
        "selected": selected.snapshot.selected,
    }));
    report.verdict = ScenarioVerdict::Pass;
    assert_eq!(report.driver, ScenarioDriver::Fake);
    assert_honest_capabilities(&report);

    let cycled = tab_cycle_scenario(&mut gui, window, view, KEY_SCREEN_TABS, &TAB_CYCLE_INDICES)
        .expect("tab cycle");
    assert_tab_cycle(&cycled, &TAB_CYCLE_INDICES);
    let mut report = screen_scenario_report(
        &gui,
        "run-fake-tab-cycle",
        SCENARIO_MAIN_SCREEN,
        CASE_TAB_CYCLE,
    );
    report.targets = vec![cycled.target];
    report.observed = Some(json!({
        "actions": command_actions(&cycled.intents),
        "selected": cycled.selected,
    }));
    report.verdict = ScenarioVerdict::Pass;
    assert_eq!(report.driver, ScenarioDriver::Fake);
    assert_honest_capabilities(&report);
}

/// `MultiWindow` evidence on the Fake adapter: the secondary
/// transport-deck view (`view-2`, `secondary_screen_spec`) mounts on
/// a second window and runs the same gesture/invoke scenarios —
/// `seek2.position` commits on `transport/position` and
/// `transport2.play` invokes on `transport`.
#[test]
fn secondary_window_runs_the_same_scenarios() {
    let state = screen_state();
    let mut gui = FakeGuiTest::new();
    let main_window = mount_main(&mut gui, &state);
    let secondary_window = mount_secondary(&mut gui, &state);
    assert_ne!(main_window, secondary_window);

    let main_seek =
        seek_parity_scenario(&mut gui, main_window, main_view_instance()).expect("main seek");
    assert_seek_parity(&main_seek);
    let secondary_seek =
        secondary_seek_parity_scenario(&mut gui, secondary_window, secondary_view_instance())
            .expect("secondary seek");
    assert_seek_parity(&secondary_seek);
    let secondary_play = invoke_scenario(
        &mut gui,
        secondary_window,
        secondary_view_instance(),
        KEY_TRANSPORT_PLAY_SECONDARY,
    )
    .expect("secondary play invoke");
    assert_transport_play(&secondary_play);

    let mut report = screen_scenario_report(
        &gui,
        "run-fake-secondary",
        SCENARIO_SECONDARY_SCREEN,
        CASE_SECONDARY_SEEK,
    );
    report.targets = vec![secondary_seek.target, secondary_play.target];
    report.observed = Some(json!({
        "secondary_seek": command_actions(&secondary_seek.intents),
        "secondary_play": command_actions(&secondary_play.intents),
    }));
    report.verdict = ScenarioVerdict::Pass;
    assert_eq!(report.driver, ScenarioDriver::Fake);
    assert_honest_capabilities(&report);
}

/// Target resolution covers window/view/component pins on the main
/// view: unknown ids are explicit errors, a wrong generation pin is
/// `StaleGeneration`, and `detach_view` stales previously resolved
/// targets — the same contract `scenario_gpui.rs` verifies on
/// `GpuiGuiTest`.
#[test]
fn fake_target_resolution_and_stale_pins_cover_main_view() {
    let mut gui = FakeGuiTest::new();
    let window = mount_main(&mut gui, &screen_state());
    let view = main_view_instance();

    let (target, seek_key) =
        common::scenario::resolve_component(&mut gui, window, view, KEY_SEEK_POSITION)
            .expect("resolve seek target");
    assert_eq!(target.window, window);
    assert_eq!(target.view, view);
    assert_eq!(target.component, Some(seek_key.clone()));

    // 未知 component / view / window は明示 error。
    let err = gui
        .resolve_target(
            &TargetQuery::new(window, view).component(ComponentKey::from("no-such-key")),
        )
        .expect_err("unknown component must fail");
    assert!(matches!(err, ScenarioError::UnknownComponent { .. }));

    let err = gui
        .resolve_target(&TargetQuery::new(
            window,
            ViewInstanceId::new(999).expect("nonzero"),
        ))
        .expect_err("unknown view must fail");
    assert!(matches!(err, ScenarioError::NoSuchView { .. }));

    let err = gui
        .resolve_target(&TargetQuery::new(
            WindowId::new(999).expect("nonzero"),
            view,
        ))
        .expect_err("unknown window must fail");
    assert!(matches!(err, ScenarioError::NoSuchWindow { .. }));

    // generation pin の stale 検証は両 adapter 共通 helper。
    assert_stale_generation_pin_rejected(&mut gui, window, view, KEY_SEEK_POSITION)
        .expect("stale generation pin rejected");

    // detach で generation が進み、pin 済み target は stale。
    gui.detach_view(window, view, "test-detach")
        .expect("detach");
    let err = gui
        .observe(&target, &seek_key)
        .expect_err("detached view must stale the target");
    assert!(
        matches!(err, ScenarioError::StaleGeneration { .. }),
        "expected StaleGeneration, got {err:?}"
    );
}

/// 実行不能項目は Unsupported/NotRun のまま記録する — fake が申告
/// しない capability は pre-flight で `Unsupported`、pixel capture と
/// golden step も `Unsupported` verdict を返し、semantic pass を
/// 実機・pixel pass へ昇格させない。
#[test]
fn fake_unrunnable_items_report_unsupported_not_run_not_pass() {
    let mut gui = FakeGuiTest::new();
    let window = mount_main(&mut gui, &screen_state());
    let view = main_view_instance();
    let (target, _) =
        common::scenario::resolve_component(&mut gui, window, view, KEY_INSPECTOR_TABLE)
            .expect("resolve");

    assert_unclaimed_capabilities_rejected(&mut gui);

    let baseline =
        std::env::temp_dir().join(format!("sv112-scenario-golden-fake-{}", std::process::id()));
    assert_visual_honesty(&mut gui, &target, "fake", &baseline);

    // ScenarioReport: driver=Fake・verdict=Unsupported のまま記録し、
    // semantic pass を実機・pixel pass へ昇格しない。
    let report = common::scenario::gated_scenario_report(
        &gui,
        "run-fake-unsupported",
        SCENARIO_MAIN_SCREEN,
        CASE_READ_ONLY_INSPECTOR,
        BTreeSet::from([
            ScenarioCapability::RealOsPointerOrKeyboard,
            ScenarioCapability::VisualCapture,
        ]),
        ScenarioVerdict::Unsupported,
        "real OS input and pixel capture are not provided by the fake adapter",
    );
    assert_eq!(report.driver, ScenarioDriver::Fake);
    assert_eq!(report.verdict, ScenarioVerdict::Unsupported);
    assert_eq!(report.verdict.lane_status(), "unsupported");
    assert!(
        !report
            .capabilities
            .contains(&ScenarioCapability::VisualCapture)
    );
    assert!(
        !report
            .capabilities
            .contains(&ScenarioCapability::RealOsPointerOrKeyboard)
    );
}
