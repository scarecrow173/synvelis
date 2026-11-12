//! SV-GUI-03 slice 5 (synvelis#112 AC5, ruviform#464) — GPUI-simulated
//! adapter(`GpuiGuiTest`/`GpuiObserver`)の scenario lane。
//!
//! `tests/common/scenario.rs` の driver-neutral scenario を
//! `GpuiGuiTest`(`gpui-test` feature)で実行し、Fake adapter と同一
//! 契約が動くこと、能力宣言が `GpuiSimulated` として正直であること、
//! 実行不能項目が Unsupported/NotRun のままであることを検証する。
//! `gpui::TestAppContext` 等の GPUI 型は adapter 内部に閉じ、
//! scenario は contract 型だけを使う。実 Window/実 OS Driver
//! (ruviform#313)・pixel golden(ruviform#314)・製品 Native E2E
//! (synvelis#114)は本 lane の証拠範囲外 — `NotRun`/`Unsupported`
//! のまま記録する。

#![cfg(feature = "gpui-test")]

mod common;

use std::collections::BTreeSet;

use common::scenario::{
    ActionOutcome, CASE_ARRANGEMENT_SELECT, CASE_FADER_PARITY, CASE_READ_ONLY_INSPECTOR,
    CASE_SECONDARY_SEEK, CASE_SEEK_PARITY, CASE_TAB_CYCLE, CASE_TRANSPORT_PLAY, GestureOutcome,
    KEY_ARRANGEMENT_TABLE, KEY_INSPECTOR_TABLE, KEY_SCREEN_TABS, KEY_SEEK_POSITION,
    KEY_TRANSPORT_PLAY, KEY_TRANSPORT_PLAY_SECONDARY, SCENARIO_MAIN_SCREEN,
    SCENARIO_SECONDARY_SCREEN, TAB_CYCLE_INDICES, TabCycleOutcome, assert_fader_parity,
    assert_honest_capabilities, assert_row_select, assert_seek_parity,
    assert_stale_generation_pin_rejected, assert_tab_cycle, assert_transport_play,
    assert_unclaimed_capabilities_rejected, assert_visual_honesty, command_actions,
    fader_parity_scenario, inspector_table_is_read_only, invoke_scenario, screen_scenario_report,
    screen_state, secondary_seek_parity_scenario, seek_parity_scenario, select_row_scenario,
    tab_cycle_scenario,
};
use ruviform_ui::behavior::CommandIntent;
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::view::{AttachEpochAllocator, ComponentKey, ViewHost, ViewSpec};
use ruviform_ui::{WindowId, WindowSpec};
use ruviform_ui_gpui::{GpuiGuiTest, ViewMount};
use ruviform_ui_test::{
    FakeGuiTest, ScenarioCapability, ScenarioDriver, ScenarioError, ScenarioTarget,
    ScenarioVerdict, SemanticGuiTest, TargetQuery,
};
use serde_json::json;

use synvelis_app::{
    MainScreenState, main_screen_spec, main_view_instance, secondary_screen_spec,
    secondary_view_instance,
};

/// `spec` を `view` の `ViewHost` に mount し、GPUI test window として
/// 立てる — production の `synvelis-native` composition
/// (`ViewHost` + `create_view_window`)と同じ形。
fn mount_gpui(gui: &mut GpuiGuiTest, view: ViewInstanceId, spec: ViewSpec) -> WindowId {
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

/// The main screen spec on a fresh GPUI test window.
fn mount_main_gpui(gui: &mut GpuiGuiTest, state: &MainScreenState) -> WindowId {
    mount_gpui(gui, main_view_instance(), main_screen_spec(state))
}

/// The secondary transport-deck spec on a fresh GPUI test window.
fn mount_secondary_gpui(gui: &mut GpuiGuiTest, state: &MainScreenState) -> WindowId {
    mount_gpui(gui, secondary_view_instance(), secondary_screen_spec(state))
}

/// GPUI lane の pass 記録 — `ScenarioReport` は adapter 実測の
/// `GpuiSimulated` + `{SemanticAction, MultiWindow}` を記録し、
/// semantic pass を実機・pixel pass へ昇格させない。
fn assert_gpui_pass_report(
    gui: &GpuiGuiTest,
    run_id: &str,
    case_id: &str,
    target: &ScenarioTarget,
    observed: serde_json::Value,
) {
    let mut report = screen_scenario_report(gui, run_id, SCENARIO_MAIN_SCREEN, case_id);
    report.targets = vec![target.clone()];
    report.observed = Some(observed);
    report.verdict = ScenarioVerdict::Pass;
    assert_eq!(report.driver, ScenarioDriver::GpuiSimulated);
    assert_eq!(report.verdict.lane_status(), "pass");
    assert_honest_capabilities(&report);
}

/// gesture outcome の observed payload(intents/phase/表示値)。
fn gesture_observed(outcome: &GestureOutcome) -> serde_json::Value {
    json!({
        "intents": outcome.intents.len(),
        "phase": format!("{:?}", outcome.snapshot.phase),
        "observed_value": outcome.snapshot.observed.as_ref().map(|o| o.value.clone()),
    })
}

/// action outcome の observed payload(action intent 列/phase)。
fn action_observed(outcome: &ActionOutcome) -> serde_json::Value {
    json!({
        "actions": command_actions(&outcome.intents),
        "phase": format!("{:?}", outcome.snapshot.phase),
    })
}

/// `main`+`secondary` の全 scenario を 1 adapter で実行した結果 —
/// Fake/GPUI で呼び出し列が完全に同一であることが parity の前提。
struct ParityOutcomes {
    seek: GestureOutcome,
    fader: GestureOutcome,
    play: ActionOutcome,
    select: ActionOutcome,
    tabs: TabCycleOutcome,
    seek2: GestureOutcome,
    play2: ActionOutcome,
}

/// 両 window に mount 済みの adapter へ同一の scenario 呼び出し列を
/// 実行する driver-neutral suite。
fn run_parity_suite<G: SemanticGuiTest>(
    gui: &mut G,
    main_window: WindowId,
    secondary_window: WindowId,
) -> ParityOutcomes {
    let main_view = main_view_instance();
    let secondary_view = secondary_view_instance();
    ParityOutcomes {
        seek: seek_parity_scenario(gui, main_window, main_view).expect("seek parity"),
        fader: fader_parity_scenario(gui, main_window, main_view).expect("fader parity"),
        play: invoke_scenario(gui, main_window, main_view, KEY_TRANSPORT_PLAY).expect("play"),
        select: select_row_scenario(gui, main_window, main_view, KEY_ARRANGEMENT_TABLE, 0)
            .expect("select"),
        tabs: tab_cycle_scenario(
            gui,
            main_window,
            main_view,
            KEY_SCREEN_TABS,
            &TAB_CYCLE_INDICES,
        )
        .expect("tabs"),
        seek2: secondary_seek_parity_scenario(gui, secondary_window, secondary_view)
            .expect("secondary seek"),
        play2: invoke_scenario(
            gui,
            secondary_window,
            secondary_view,
            KEY_TRANSPORT_PLAY_SECONDARY,
        )
        .expect("secondary play"),
    }
}

/// `main_screen_spec` の全 driver-neutral scenario が GPUI-simulated
/// adapter で pass する — seek/fader gesture parity、transport play
/// invoke、Arrangement row select、4-tab 巡回、Inspector read-only。
/// `ScenarioReport` は adapter 実測の `GpuiSimulated` +
/// `{SemanticAction, MultiWindow}` を記録する。
#[test]
fn gpui_main_screen_scenarios_pass_with_declared_capabilities() {
    let state = screen_state();
    let mut gui = GpuiGuiTest::new();
    let window = mount_main_gpui(&mut gui, &state);
    let view = main_view_instance();

    let seek = seek_parity_scenario(&mut gui, window, view).expect("gpui seek");
    assert_seek_parity(&seek);
    assert_gpui_pass_report(
        &gui,
        "run-gpui-seek",
        CASE_SEEK_PARITY,
        &seek.target,
        gesture_observed(&seek),
    );

    let fader = fader_parity_scenario(&mut gui, window, view).expect("gpui fader");
    assert_fader_parity(&fader);
    assert_gpui_pass_report(
        &gui,
        "run-gpui-fader",
        CASE_FADER_PARITY,
        &fader.target,
        gesture_observed(&fader),
    );

    let play = invoke_scenario(&mut gui, window, view, KEY_TRANSPORT_PLAY).expect("gpui play");
    assert_transport_play(&play);
    assert_gpui_pass_report(
        &gui,
        "run-gpui-play",
        CASE_TRANSPORT_PLAY,
        &play.target,
        action_observed(&play),
    );

    assert!(
        inspector_table_is_read_only(&mut gui, window, view).expect("read-only check"),
        "inspector table must reject row selection"
    );
    let read_only = screen_scenario_report(
        &gui,
        "run-gpui-inspector-readonly",
        SCENARIO_MAIN_SCREEN,
        CASE_READ_ONLY_INSPECTOR,
    );
    assert_eq!(read_only.driver, ScenarioDriver::GpuiSimulated);
    assert_eq!(read_only.verdict.lane_status(), "not_run");

    let selected = select_row_scenario(&mut gui, window, view, KEY_ARRANGEMENT_TABLE, 0)
        .expect("gpui arrangement select");
    assert_row_select(&selected, 0);
    assert_gpui_pass_report(
        &gui,
        "run-gpui-arrangement-select",
        CASE_ARRANGEMENT_SELECT,
        &selected.target,
        json!({
            "actions": command_actions(&selected.intents),
            "selected": selected.snapshot.selected,
        }),
    );

    let cycled = tab_cycle_scenario(&mut gui, window, view, KEY_SCREEN_TABS, &TAB_CYCLE_INDICES)
        .expect("gpui tab cycle");
    assert_tab_cycle(&cycled, &TAB_CYCLE_INDICES);
    assert_gpui_pass_report(
        &gui,
        "run-gpui-tab-cycle",
        CASE_TAB_CYCLE,
        &cycled.target,
        json!({
            "actions": command_actions(&cycled.intents),
            "selected": cycled.selected,
        }),
    );
}

/// `MultiWindow` 証拠: main + secondary の 2 window に spec を mount
/// し、両 view で同一 scenario を実行する — `seek2.position` の
/// gesture parity と `transport2.play` の invoke が別 window・別
/// view instance で成立する。
#[test]
fn gpui_two_window_main_and_secondary_scenarios() {
    let state = screen_state();
    let mut gui = GpuiGuiTest::new();
    let main_window = mount_main_gpui(&mut gui, &state);
    let secondary_window = mount_secondary_gpui(&mut gui, &state);
    assert_ne!(main_window, secondary_window);

    let main_seek =
        seek_parity_scenario(&mut gui, main_window, main_view_instance()).expect("gpui main seek");
    assert_seek_parity(&main_seek);
    let secondary_seek =
        secondary_seek_parity_scenario(&mut gui, secondary_window, secondary_view_instance())
            .expect("gpui secondary seek");
    assert_seek_parity(&secondary_seek);
    let secondary_play = invoke_scenario(
        &mut gui,
        secondary_window,
        secondary_view_instance(),
        KEY_TRANSPORT_PLAY_SECONDARY,
    )
    .expect("gpui secondary play");
    assert_transport_play(&secondary_play);

    let mut report = screen_scenario_report(
        &gui,
        "run-gpui-secondary",
        SCENARIO_SECONDARY_SCREEN,
        CASE_SECONDARY_SEEK,
    );
    report.targets = vec![secondary_seek.target, secondary_play.target];
    report.observed = Some(json!({
        "secondary_seek": command_actions(&secondary_seek.intents),
        "secondary_play": command_actions(&secondary_play.intents),
    }));
    report.verdict = ScenarioVerdict::Pass;
    assert_eq!(report.driver, ScenarioDriver::GpuiSimulated);
    assert_honest_capabilities(&report);
}

/// AC5 の parity 証拠: 同一 scenario 呼び出しを Fake / GPUI 両
/// adapter で実行し、発行 intent が一致することを検証する。
/// `driver`/`capabilities` は各 adapter の正直な申告のまま —
/// `GpuiSimulated` の pass は `Fake` の pass と同じ semantic
/// 証拠水準であり、実機・pixel 証拠へ昇格しない。
#[test]
fn gpui_and_fake_run_the_same_scenarios_with_equal_evidence() {
    let state = screen_state();

    // Fake adapter: main + secondary を 2 window に mount。
    let mut fake = FakeGuiTest::new();
    let f_main = fake.open_window();
    fake.mount_view(f_main, main_view_instance(), main_screen_spec(&state))
        .expect("fake main mount");
    let f_secondary = fake.open_window();
    fake.mount_view(
        f_secondary,
        secondary_view_instance(),
        secondary_screen_spec(&state),
    )
    .expect("fake secondary mount");

    // GPUI adapter: 同じ spec・同じ view instance を 2 window に mount。
    let mut gpui = GpuiGuiTest::new();
    let g_main = mount_main_gpui(&mut gpui, &state);
    let g_secondary = mount_secondary_gpui(&mut gpui, &state);

    // 同一の scenario 呼び出し列を両 adapter で実行する。
    let f = run_parity_suite(&mut fake, f_main, f_secondary);
    let g = run_parity_suite(&mut gpui, g_main, g_secondary);

    // Action intent は決定的 — target/action/params が両 adapter で一致。
    for (name, f, g) in [
        ("play", &f.play, &g.play),
        ("select", &f.select, &g.select),
        ("play2", &f.play2, &g.play2),
    ] {
        assert_eq!(
            command_actions(&f.intents),
            command_actions(&g.intents),
            "{name} action intents must match across adapters"
        );
    }
    assert_eq!(
        command_actions(&f.tabs.intents),
        command_actions(&g.tabs.intents),
        "tabs action intents must match across adapters"
    );
    assert_eq!(f.select.snapshot.selected, g.select.snapshot.selected);
    assert_eq!(f.tabs.selected, g.tabs.selected);

    // Gesture の Begin/Preview は dispatch 由来の field を持ち得る
    // ため、parity oracle は Commit intent と settle 後の表示値。
    for (name, f, g) in [
        ("seek", &f.seek, &g.seek),
        ("fader", &f.fader, &g.fader),
        ("seek2", &f.seek2, &g.seek2),
    ] {
        let f_commits: Vec<&CommandIntent> = f
            .intents
            .iter()
            .filter(|m| matches!(m.intent, CommandIntent::Commit { .. }))
            .map(|m| &m.intent)
            .collect();
        let g_commits: Vec<&CommandIntent> = g
            .intents
            .iter()
            .filter(|m| matches!(m.intent, CommandIntent::Commit { .. }))
            .map(|m| &m.intent)
            .collect();
        assert_eq!(f_commits, g_commits, "{name} commit intents must match");
        let f_plain = f.snapshot.committed.as_ref().expect("fake committed").plain;
        let g_plain = g.snapshot.committed.as_ref().expect("gpui committed").plain;
        assert!(
            (f_plain - g_plain).abs() < 1e-12,
            "{name} committed plain values diverge: {f_plain} vs {g_plain}"
        );
    }
}

/// Target 解決は Fake lane と同一契約 — window/view/component pin、
/// 未知 id の明示 error、generation pin の `StaleGeneration`、
/// detach 後の stale target。
#[test]
fn gpui_target_resolution_and_stale_pins_cover_main_view() {
    let mut gui = GpuiGuiTest::new();
    let window = mount_main_gpui(&mut gui, &screen_state());
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

    // detach で generation が進み、pin 済み target は stale
    // (attach epoch も不一致になり得るが、generation が先に fail する)。
    gui.detach_view(window, "test-detach").expect("detach");
    let err = gui
        .observe(&target, &seek_key)
        .expect_err("detached view must stale the target");
    assert!(
        matches!(
            err,
            ScenarioError::StaleGeneration { .. } | ScenarioError::StaleEpoch { .. }
        ),
        "expected stale pin error, got {err:?}"
    );
}

/// 実行不能項目は Unsupported/NotRun のまま記録する — gpui-test が
/// 申告しない capability は pre-flight で `Unsupported`、pixel
/// capture と golden step も `Unsupported` verdict を返し、semantic
/// pass を実機・pixel pass へ昇格させない。
#[test]
fn gpui_unrunnable_items_report_unsupported_not_run_not_pass() {
    let mut gui = GpuiGuiTest::new();
    let window = mount_main_gpui(&mut gui, &screen_state());
    let (target, _) = common::scenario::resolve_component(
        &mut gui,
        window,
        main_view_instance(),
        KEY_INSPECTOR_TABLE,
    )
    .expect("resolve");

    assert_unclaimed_capabilities_rejected(&mut gui);

    let baseline =
        std::env::temp_dir().join(format!("sv112-scenario-golden-gpui-{}", std::process::id()));
    assert_visual_honesty(&mut gui, &target, "gpui-test", &baseline);

    // ScenarioReport: driver=GpuiSimulated・verdict=Unsupported の
    // まま記録し、semantic pass を実機・pixel pass へ昇格しない。
    let report = common::scenario::gated_scenario_report(
        &gui,
        "run-gpui-unsupported",
        SCENARIO_MAIN_SCREEN,
        CASE_READ_ONLY_INSPECTOR,
        BTreeSet::from([
            ScenarioCapability::RealOsPointerOrKeyboard,
            ScenarioCapability::VisualCapture,
        ]),
        ScenarioVerdict::Unsupported,
        "real OS input and pixel capture are not provided by the gpui-test adapter",
    );
    assert_eq!(report.driver, ScenarioDriver::GpuiSimulated);
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
