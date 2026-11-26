//! synvelis#114 SV-GUI-05 — SV-OS consumer catalog の headless lane。
//!
//! 同じ `sv_os_catalog` entries を Fake lane(default test)と、feature 有効時は
//! `GpuiGuiTest` lane に投影・実行する。いずれの report も semantic lane の
//! 証拠であり、real OS/pixel/IME/native plugin dock pass へ昇格させない。

mod common;

use common::scenario::screen_state;
use common::scenario::sv_os_catalog::{
    CAPABILITY_CLASSIFICATIONS, RequirementState, SV_OS_SCENARIO, SvOsLane, sv_os_entries,
    sv_os_run_id,
};
use ruviform_ui::WindowId;
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::view::ViewSpec;
use ruviform_ui_test::{
    DriverBinding, FakeGuiTest, ScenarioCapability, ScenarioDriver, ScenarioMatrix,
    ScenarioVerdict, run_entries,
};
use synvelis_app::{
    main_screen_spec, main_view_instance, secondary_screen_spec, secondary_view_instance,
};

fn mount_fake(gui: &mut FakeGuiTest, view: ViewInstanceId, spec: ViewSpec) -> WindowId {
    let window = gui.open_window();
    gui.mount_view(window, view, spec)
        .expect("spec mounts on the fake adapter");
    window
}

fn fake_lane() -> SvOsLane<FakeGuiTest> {
    let mut gui = FakeGuiTest::new();
    let state = screen_state();
    let main_window = mount_fake(&mut gui, main_view_instance(), main_screen_spec(&state));
    let secondary_window = mount_fake(
        &mut gui,
        secondary_view_instance(),
        secondary_screen_spec(&state),
    );
    SvOsLane::new(
        gui,
        main_window,
        main_view_instance(),
        secondary_window,
        secondary_view_instance(),
    )
}

fn assert_catalog_reports(reports: &[ruviform_ui_test::ScenarioReport], driver: ScenarioDriver) {
    assert_eq!(reports.len(), 5);
    assert_eq!(
        reports
            .iter()
            .map(|report| report.case.as_ref().map(ruviform_ui_test::CaseId::as_str))
            .collect::<Vec<_>>(),
        vec![
            Some("SV-OS-01"),
            Some("SV-OS-02"),
            Some("SV-OS-03"),
            Some("SV-OS-04"),
            Some("SV-OS-05"),
        ]
    );
    for report in reports {
        assert_eq!(report.scenario.as_str(), SV_OS_SCENARIO);
        assert_eq!(report.driver, driver);
        assert_eq!(
            report.verdict,
            ScenarioVerdict::Pass,
            "{} unexpectedly failed: {:?} / {:?}",
            report.case.as_ref().map_or("<none>", |case| case.as_str()),
            report.detail,
            report.observed,
        );
        assert!(
            report
                .capabilities
                .contains(&ScenarioCapability::SemanticAction)
        );
        assert!(
            !report
                .capabilities
                .contains(&ScenarioCapability::VisualCapture)
        );
        assert!(!report.capabilities.contains(&ScenarioCapability::Ime));
        assert!(
            !report
                .capabilities
                .contains(&ScenarioCapability::NativePluginDock)
        );
        assert!(report.expected.is_some());
        assert!(report.observed.is_some());
        assert_ne!(report.targets.len(), 0);
    }
}

/// Fake driver column executes the same catalog declaration. This is
/// OS-L1/semantic evidence only: product command intents, Applied receipts,
/// and view-state oracles are checked, while real OS input and pixels stay
/// outside the lane verdict.
#[test]
fn fake_lane_executes_sv_os_catalog_cases() {
    let mut lane = fake_lane();
    let entries = sv_os_entries::<FakeGuiTest>();
    let reports = run_entries(&entries, &sv_os_run_id(), &mut lane);
    assert_catalog_reports(&reports, ScenarioDriver::Fake);
}

/// Matrix projection is data-only and shows the same five cases as runnable
/// on both honest headless driver columns. It executes nothing and therefore
/// cannot upgrade a projected case into a pass.
#[test]
fn headless_matrix_projects_fake_and_gpui_simulated_columns() {
    const HEADLESS_CAPABILITIES: &[ScenarioCapability] = &[
        ScenarioCapability::SemanticAction,
        ScenarioCapability::MultiWindow,
    ];
    let entries = sv_os_entries::<FakeGuiTest>();
    let bindings = [
        DriverBinding {
            driver: ScenarioDriver::Fake,
            capabilities: HEADLESS_CAPABILITIES,
            cases: &entries,
        },
        DriverBinding {
            driver: ScenarioDriver::GpuiSimulated,
            capabilities: HEADLESS_CAPABILITIES,
            cases: &entries,
        },
    ];
    let matrix = ScenarioMatrix {
        bindings: &bindings,
    };
    let projection = matrix.project();
    assert_eq!(projection.len(), 10);
    assert!(projection.iter().all(|cell| cell.runnable));
    assert!(projection.iter().all(|cell| cell.missing.is_none()));
    assert_eq!(
        projection
            .iter()
            .filter(|cell| cell.driver == ScenarioDriver::GpuiSimulated)
            .count(),
        5
    );
}

/// Capability classification rows keep the frozen case ids explicit. They do
/// not silently require unavailable L2/L3 capabilities for SV-OS-02〜05, while
/// SV-OS-01 records the real-lane-only input/pixel vertical slice explicitly.
#[test]
fn capability_classification_keeps_l2_l3_out_of_headless_requirements() {
    assert_eq!(CAPABILITY_CLASSIFICATIONS.len(), 15);
    assert!(CAPABILITY_CLASSIFICATIONS.iter().all(|row| matches!(
        row.case,
        "SV-OS-01" | "SV-OS-02" | "SV-OS-03" | "SV-OS-04" | "SV-OS-05"
    )));
    assert!(
        CAPABILITY_CLASSIFICATIONS
            .iter()
            .filter(|row| row.capability == ScenarioCapability::NativePluginDock)
            .all(|row| row.state == RequirementState::NotApplicable)
    );
    assert!(
        CAPABILITY_CLASSIFICATIONS
            .iter()
            .filter(|row| row.capability == ScenarioCapability::RealOsPointerOrKeyboard)
            .all(|row| {
                if row.case == "SV-OS-01" {
                    row.state == RequirementState::Required
                } else {
                    row.state == RequirementState::Deferred
                }
            })
    );
    assert!(
        CAPABILITY_CLASSIFICATIONS
            .iter()
            .filter(|row| row.capability == ScenarioCapability::VisualCapture)
            .all(|row| {
                if row.case == "SV-OS-01" {
                    row.state == RequirementState::Required
                } else {
                    row.state == RequirementState::Deferred
                }
            })
    );
}

#[cfg(feature = "gpui-test")]
mod gpui {
    use super::*;
    use ruviform_ui::WindowSpec;
    use ruviform_ui::view::{AttachEpochAllocator, ViewHost};
    use ruviform_ui_gpui::{GpuiGuiTest, ViewMount};
    use ruviform_ui_test::SemanticGuiTest;

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

    fn gpui_lane() -> SvOsLane<GpuiGuiTest> {
        let mut gui = GpuiGuiTest::new();
        let state = screen_state();
        let main_window = mount_gpui(&mut gui, main_view_instance(), main_screen_spec(&state));
        let secondary_window = mount_gpui(
            &mut gui,
            secondary_view_instance(),
            secondary_screen_spec(&state),
        );
        SvOsLane::new(
            gui,
            main_window,
            main_view_instance(),
            secondary_window,
            secondary_view_instance(),
        )
    }

    /// The same catalog executes through the production-like GPUI test
    /// composition (`ViewHost` + `create_view_window`). Reports honestly use
    /// `GpuiSimulated`; no pixel capture is claimed because `gpui-test` has
    /// no framebuffer readback.
    #[test]
    fn gpui_simulated_lane_executes_the_same_sv_os_catalog_cases() {
        let mut lane = gpui_lane();
        let entries = sv_os_entries::<GpuiGuiTest>();
        let reports = run_entries(&entries, &sv_os_run_id(), &mut lane);
        assert_catalog_reports(&reports, ScenarioDriver::GpuiSimulated);
    }
}
