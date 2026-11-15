//! synvelis#176 (SV-GPUI-02 / PD-01〜07) — headless plugin-dock
//! contract slice.
//!
//! The shared `rf631.plugin-dock` vocabulary exported by
//! `ruviform-ui-test` (pin `ac10a2f7`) is exercised through the same
//! driver-neutral `SemanticGuiTest` port the product scenarios use:
//!
//! - `plugin_dock_view_spec()` declares a `Panel` holding one
//!   `NativeSlot` keyed `plugin.editor` — mounting it proves
//!   layout/semantics only, never dock/window/pixel evidence.
//! - `plugin_dock_spec()` requires `{NativePluginDock,
//!   VisualCapture}` — capabilities no semantic adapter claims, so a
//!   spec requiring them pre-flights to `Unsupported` before its body
//!   runs. (Spec-helper contract only: upstream `rf631.plugin-dock`
//!   probe cases declare `required = {}` and gate on artifact
//!   presence inside the body — they do not use this helper.)
//! - `PluginArtifactEvidence` carries the six plugin-identity dims
//!   (path/SHA-256/plugin_id/name/vendor/version) that PD evidence
//!   rows must record before a `Pass` is attributable.
//!
//! The Fake adapter's `view-mount` `Pass` is contract conformance
//! evidence only. It is not Native Dock, not a real plugin editor,
//! not pixel/input/focus evidence — PD-01〜07 Real OS / Real Plugin
//! lanes stay `NotRun` (`evidence/sv176/`).

use std::cell::Cell;
use std::collections::BTreeSet;

use ruviform_ui::WindowId;
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::view::{ComponentKey, ComponentSpec};
use ruviform_ui_test::{
    CaseId, FakeGuiTest, PLUGIN_DOCK_SCENARIO, PLUGIN_DOCK_SLOT_KEY, PluginArtifactEvidence, RunId,
    ScenarioCapability, ScenarioDriver, ScenarioError, ScenarioId, ScenarioOutcome, ScenarioSpec,
    ScenarioVerdict, SemanticGuiTest, TargetQuery, plugin_dock_spec, plugin_dock_view_spec,
    run_scenario,
};

/// `rf631.plugin-dock`/`view-mount` の scenario body — resolve +
/// observe the `plugin.editor` native slot on an already-mounted
/// window/view. Driver-neutral: `SemanticGuiTest` port ops only, so
/// the Fake and GPUI-simulated lanes run the identical body.
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

/// `plugin_dock_view_spec` は `plugin.dock` Panel 直下に
/// `plugin.editor` の `NativeSlot` を 1 つだけ宣言する — dock
/// binding・surface attach は spec の責務外であることを型で固定。
#[test]
fn plugin_dock_view_spec_declares_the_plugin_editor_native_slot() {
    let spec = plugin_dock_view_spec();
    let ComponentSpec::Panel(panel) = &spec.root else {
        panic!("plugin dock root must be a Panel");
    };
    assert_eq!(panel.key, ComponentKey("plugin.dock".to_string()));
    assert_eq!(panel.children.len(), 1);
    let ComponentSpec::NativeSlot(slot) = &panel.children[0] else {
        panic!("plugin dock child must be a NativeSlot");
    };
    assert_eq!(slot.key, ComponentKey(PLUGIN_DOCK_SLOT_KEY.to_string()));
    assert_eq!(PLUGIN_DOCK_SCENARIO, "rf631.plugin-dock");
    assert_eq!(PLUGIN_DOCK_SLOT_KEY, "plugin.editor");
}

/// `PluginArtifactEvidence` の environment JSON は PD 証拠行が
/// 必須とする 6 dims（`path`・`sha256`・`plugin_id`・`name`・
/// `vendor`・`version`）を全て含む — 再ビルド fixture を同一証拠と
/// 誤読しないための identity schema（test-spec PD-FRZ-06）。
#[test]
fn plugin_artifact_evidence_carries_the_six_identity_dims() {
    let artifact = PluginArtifactEvidence {
        path: "fixtures/test_gain.vst3".to_string(),
        sha256: "ab".repeat(32),
        plugin_id: "test.gain".to_string(),
        name: "Test Gain".to_string(),
        vendor: "Ruviform".to_string(),
        version: "0.1.0".to_string(),
    };
    let env = artifact.environment_value();
    let recorded = env["plugin_artifact"]
        .as_object()
        .expect("plugin_artifact must be a JSON object");
    for dim in ["path", "sha256", "plugin_id", "name", "vendor", "version"] {
        assert!(
            recorded.contains_key(dim),
            "plugin_artifact must record the {dim} identity dim"
        );
    }
    let evidence_ref = artifact.evidence_ref();
    assert!(
        evidence_ref.starts_with("plugin-artifact:fixtures/test_gain.vst3@"),
        "evidence ref must name the artifact path: {evidence_ref}"
    );
    assert!(
        evidence_ref.contains(&artifact.sha256[..12]),
        "evidence ref must carry the digest prefix: {evidence_ref}"
    );
}

/// `plugin_dock_spec` は `rf631.plugin-dock` scenario の
/// `{NativePluginDock, VisualCapture}` を required として宣言する
/// — semantic adapter がこの spec を名乗っても pre-flight で
/// `Unsupported` になる構造そのものを固定する。
#[test]
fn plugin_dock_spec_requires_native_dock_and_visual_capture() {
    let spec = plugin_dock_spec(
        RunId::new("sv176-pd-contract"),
        CaseId::new("dock-pixels"),
        None,
    );
    assert_eq!(spec.scenario, ScenarioId::new(PLUGIN_DOCK_SCENARIO));
    assert_eq!(spec.case, Some(CaseId::new("dock-pixels")));
    assert_eq!(
        spec.required,
        BTreeSet::from([
            ScenarioCapability::NativePluginDock,
            ScenarioCapability::VisualCapture,
        ])
    );
}

/// `view-mount` case: `plugin_dock_view_spec` を Fake adapter に
/// mount し、`plugin.editor` slot の resolve/observe が `Pass` する。
/// `ScenarioReport` は adapter 実測の `Fake` +
/// `{SemanticAction, MultiWindow}` のみを記録し、semantic pass を
/// dock/window/pixel/plugin 証拠へ昇格させない。
#[test]
fn view_mount_case_passes_on_the_fake_adapter() {
    let mut gui = FakeGuiTest::new();
    let window = gui.open_window();
    let view = ViewInstanceId::new(1).expect("view id 1 is non-zero");
    gui.mount_view(window, view, plugin_dock_view_spec())
        .expect("mount plugin dock view spec");

    let spec = ScenarioSpec::new(
        RunId::new("sv176-pd-view-mount-fake"),
        ScenarioId::new(PLUGIN_DOCK_SCENARIO),
    )
    .case(CaseId::new("view-mount"))
    .required(BTreeSet::from([ScenarioCapability::SemanticAction]));

    let report = run_scenario(&mut gui, &spec, |gui| view_mount_body(gui, window, view));

    assert_eq!(report.verdict, ScenarioVerdict::Pass);
    assert_eq!(report.verdict.lane_status(), "pass");
    assert_eq!(report.driver, ScenarioDriver::Fake);
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
    assert_eq!(report.targets.len(), 1);
    assert_eq!(
        report.targets[0].component,
        Some(ComponentKey(PLUGIN_DOCK_SLOT_KEY.to_string()))
    );
    assert!(
        report
            .evidence
            .iter()
            .any(|e| e.contains("semantic-mount:native_slot")),
        "report must carry the semantic-mount evidence ref: {:?}",
        report.evidence
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

/// spec-helper gate: `plugin_dock_spec` の required
/// `{NativePluginDock, VisualCapture}` は Fake adapter の
/// capabilities に含まれないため、`run_scenario` は body を
/// 実行せず pre-flight `Unsupported` を返す — native-dock/pixel
/// 証拠を要求する spec が semantic adapter で `Pass` を出せない
/// ことの検証であり、PD case の `NotRun` 記録を補強する
/// （Pass ではない）。upstream probe case 自体は `required={}`
/// の `probe_spec` で artifact presence を body 内 gate するため
/// 本 test の対象ではない。
#[test]
fn plugin_dock_spec_gate_is_unsupported_before_the_body_runs() {
    let mut gui = FakeGuiTest::new();
    let ran = Cell::new(false);
    let spec = plugin_dock_spec(
        RunId::new("sv176-pd-dock-gate-fake"),
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
    assert_eq!(report.driver, ScenarioDriver::Fake);
    assert_eq!(
        report.required,
        BTreeSet::from([
            ScenarioCapability::NativePluginDock,
            ScenarioCapability::VisualCapture,
        ])
    );
    assert!(
        report
            .detail
            .as_deref()
            .is_some_and(|d| d.contains("unsupported capability")),
        "report must name the missing capability: {:?}",
        report.detail
    );
}
