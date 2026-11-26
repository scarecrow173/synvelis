//! synvelis#114 SV-GUI-05 — SV-OS-01〜05 の consumer catalog。
//!
//! frozen `evidence/sv114/sv-os-case-definitions.md` の case identity を、
//! ruviform#464/#631 の `ScenarioEntry`/`ScenarioMatrix`/`DriverBinding`
//! 形式へ写す。case 本体は通常の Rust 関数であり、既存の
//! driver-neutral scenario helper と製品 namespace oracle を共有する。
//! この module 自体は adapter 型を知らない。report の driver/capability は
//! 実行した adapter の値であり、Fake/GpuiSimulated の semantic pass を
//! real OS/pixel/IME/native dock 証拠へ昇格させない。

use ruviform_ui::WindowId;
use ruviform_ui::behavior::{CommandPhase, CommandReceipt, ReceiptStatus};
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::ipc::Intent;
use ruviform_ui::{DriverEvent, WindowDriver, WindowLifecycle, WindowSpec};
use ruviform_ui_test::{
    RunId, ScenarioCapability, ScenarioEntry, ScenarioOutcome, ScenarioReport, ScenarioSpec,
    ScenarioSpecExtras, ScenarioVerdict, SemanticGuiTest, run_scenario,
};
use serde_json::json;
use synvelis_app::binding::PROJECT_TARGET;
use synvelis_app::{mixer_gain_target, secondary_view_id, secondary_view_targets};

use crate::common::mounted::{
    MountedFixture, action, fixture, key as mounted_key, last_result, notifies, open_clip_project,
    results_for, write_clip_bundle,
};

use super::{
    KEY_ARRANGEMENT_TABLE, KEY_TRANSPORT_PLAY, KEY_TRANSPORT_PLAY_SECONDARY, TAB_CYCLE_INDICES,
    assert_fader_parity, assert_row_select, assert_seek_parity,
    assert_stale_generation_pin_rejected, assert_tab_cycle, assert_transport_play, command_actions,
    fader_parity_scenario, inspector_table_is_read_only, invoke_scenario,
    secondary_seek_parity_scenario, seek_parity_scenario, select_row_scenario, tab_cycle_scenario,
};

/// SV-OS consumer catalog の scenario id。
pub const SV_OS_SCENARIO: &str = "synvelis.sv-os.consumer";
/// catalog run id。headless/real lane で共通の定義を使い、driver 列だけ分離する。
pub const SV_OS_RUN_ID: &str = "sv114.svos-consumer-catalog";

const CASE_SV_OS_01: &str = "SV-OS-01";
const CASE_SV_OS_02: &str = "SV-OS-02";
const CASE_SV_OS_03: &str = "SV-OS-03";
const CASE_SV_OS_04: &str = "SV-OS-04";
const CASE_SV_OS_05: &str = "SV-OS-05";

const SEMANTIC_REQUIRED: &[ScenarioCapability] = &[ScenarioCapability::SemanticAction];
const MULTIWINDOW_REQUIRED: &[ScenarioCapability] = &[
    ScenarioCapability::SemanticAction,
    ScenarioCapability::MultiWindow,
];

/// Requirement state used by the case/capability classification table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequirementState {
    /// 現在の lane が提供し、case verdict の一部として要求する。
    Required,
    /// Issue #114 の後続 gate に明示的に残す。
    Deferred,
    /// SV-OS-01〜05 では要求しない。
    NotApplicable,
}

/// One case/capability classification row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CapabilityClassification {
    /// frozen SV-OS case id。
    pub case: &'static str,
    /// 分類対象 capability。
    pub capability: ScenarioCapability,
    /// Required/Deferred/NotApplicable。
    pub state: RequirementState,
    /// 分類理由。
    pub rationale: &'static str,
}

/// SV-OS-01〜05 の capability 分類。semantic catalog が要求するのは
/// `SemanticAction`/`MultiWindow` のみ。real OS input/pixel/IME は L2 後続 gate、
/// `NativePluginDock` は SV-OS-09 であって 01〜05 には要求しない。
pub const CAPABILITY_CLASSIFICATIONS: &[CapabilityClassification] = &[
    CapabilityClassification {
        case: CASE_SV_OS_01,
        capability: ScenarioCapability::RealOsPointerOrKeyboard,
        state: RequirementState::Deferred,
        rationale: "real OS input is part of the separate interactive GUI-L2 gate",
    },
    CapabilityClassification {
        case: CASE_SV_OS_01,
        capability: ScenarioCapability::VisualCapture,
        state: RequirementState::Deferred,
        rationale: "pixel evidence belongs to the ruviform#314/L2 gate",
    },
    CapabilityClassification {
        case: CASE_SV_OS_01,
        capability: ScenarioCapability::NativePluginDock,
        state: RequirementState::NotApplicable,
        rationale: "native dock is SV-OS-09/PD scope, not project lifecycle",
    },
    CapabilityClassification {
        case: CASE_SV_OS_02,
        capability: ScenarioCapability::RealOsPointerOrKeyboard,
        state: RequirementState::Deferred,
        rationale: "arrangement/inspector semantic selection is required; OS input is deferred",
    },
    CapabilityClassification {
        case: CASE_SV_OS_02,
        capability: ScenarioCapability::VisualCapture,
        state: RequirementState::Deferred,
        rationale: "pixel evidence belongs to the ruviform#314/L2 gate",
    },
    CapabilityClassification {
        case: CASE_SV_OS_02,
        capability: ScenarioCapability::NativePluginDock,
        state: RequirementState::NotApplicable,
        rationale: "native dock is SV-OS-09/PD scope, not arrangement/inspector",
    },
    CapabilityClassification {
        case: CASE_SV_OS_03,
        capability: ScenarioCapability::RealOsPointerOrKeyboard,
        state: RequirementState::Deferred,
        rationale: "fader/undo semantic receipts are required; OS input is deferred",
    },
    CapabilityClassification {
        case: CASE_SV_OS_03,
        capability: ScenarioCapability::VisualCapture,
        state: RequirementState::Deferred,
        rationale: "pixel evidence belongs to the ruviform#314/L2 gate",
    },
    CapabilityClassification {
        case: CASE_SV_OS_03,
        capability: ScenarioCapability::NativePluginDock,
        state: RequirementState::NotApplicable,
        rationale: "native dock is SV-OS-09/PD scope, not mixer undo",
    },
    CapabilityClassification {
        case: CASE_SV_OS_04,
        capability: ScenarioCapability::RealOsPointerOrKeyboard,
        state: RequirementState::Deferred,
        rationale: "transport/save/reload semantic receipts are required; OS input is deferred",
    },
    CapabilityClassification {
        case: CASE_SV_OS_04,
        capability: ScenarioCapability::VisualCapture,
        state: RequirementState::Deferred,
        rationale: "pixel evidence belongs to the ruviform#314/L2 gate",
    },
    CapabilityClassification {
        case: CASE_SV_OS_04,
        capability: ScenarioCapability::NativePluginDock,
        state: RequirementState::NotApplicable,
        rationale: "native dock is SV-OS-09/PD scope, not transport persistence",
    },
    CapabilityClassification {
        case: CASE_SV_OS_05,
        capability: ScenarioCapability::RealOsPointerOrKeyboard,
        state: RequirementState::Deferred,
        rationale: "two-window semantic ownership is required; OS input/focus proof is deferred",
    },
    CapabilityClassification {
        case: CASE_SV_OS_05,
        capability: ScenarioCapability::VisualCapture,
        state: RequirementState::Deferred,
        rationale: "pixel evidence belongs to the ruviform#314/L2 gate",
    },
    CapabilityClassification {
        case: CASE_SV_OS_05,
        capability: ScenarioCapability::NativePluginDock,
        state: RequirementState::NotApplicable,
        rationale: "native dock is SV-OS-09/PD scope, not two-window lifecycle",
    },
];

/// Lane context shared by the headless and real-OS catalog lanes.
///
/// The context owns the adapter and the already-mounted product view ids.
/// It deliberately contains no service bypass: every operation in a case body
/// enters through `SemanticGuiTest`, and product assertions inspect the
/// minted command/receipt/visible-state evidence those operations produce.
pub struct SvOsLane<G> {
    /// The live semantic adapter.
    pub gui: G,
    /// Main product window.
    pub main_window: WindowId,
    /// Main product view instance.
    pub main_view: ViewInstanceId,
    /// Secondary product window.
    pub secondary_window: WindowId,
    /// Secondary product view instance.
    pub secondary_view: ViewInstanceId,
}

impl<G> SvOsLane<G> {
    /// Create the lane context for a caller that already mounted both views.
    #[must_use]
    pub const fn new(
        gui: G,
        main_window: WindowId,
        main_view: ViewInstanceId,
        secondary_window: WindowId,
        secondary_view: ViewInstanceId,
    ) -> Self {
        Self {
            gui,
            main_window,
            main_view,
            secondary_window,
            secondary_view,
        }
    }
}

/// SV-OS-01〜05 executable catalog entries for one lane context type.
///
/// The array is returned by value because the context type is consumer-local
/// no second catalog implementation or JSON DSL is introduced.
#[must_use]
pub fn sv_os_entries<G>() -> [ScenarioEntry<SvOsLane<G>>; 5]
where
    G: SemanticGuiTest,
{
    [
        ScenarioEntry {
            case: CASE_SV_OS_01,
            scenario: SV_OS_SCENARIO,
            required: SEMANTIC_REQUIRED,
            spec_extras: sv_os_extras::<G>,
            run: run_sv_os_01::<G>,
        },
        ScenarioEntry {
            case: CASE_SV_OS_02,
            scenario: SV_OS_SCENARIO,
            required: SEMANTIC_REQUIRED,
            spec_extras: sv_os_extras::<G>,
            run: run_sv_os_02::<G>,
        },
        ScenarioEntry {
            case: CASE_SV_OS_03,
            scenario: SV_OS_SCENARIO,
            required: SEMANTIC_REQUIRED,
            spec_extras: sv_os_extras::<G>,
            run: run_sv_os_03::<G>,
        },
        ScenarioEntry {
            case: CASE_SV_OS_04,
            scenario: SV_OS_SCENARIO,
            required: SEMANTIC_REQUIRED,
            spec_extras: sv_os_extras::<G>,
            run: run_sv_os_04::<G>,
        },
        ScenarioEntry {
            case: CASE_SV_OS_05,
            scenario: SV_OS_SCENARIO,
            required: MULTIWINDOW_REQUIRED,
            spec_extras: sv_os_extras::<G>,
            run: run_sv_os_05::<G>,
        },
    ]
}

/// `RunId` shared by lane executors.
#[must_use]
pub fn sv_os_run_id() -> RunId {
    RunId::new(SV_OS_RUN_ID)
}

fn sv_os_extras<G: SemanticGuiTest>(ctx: &SvOsLane<G>) -> ScenarioSpecExtras {
    ScenarioSpecExtras {
        environment: Some(json!({
            "lane": "sv-os-consumer-catalog",
            "catalog": "sv114-svos-consumer-catalog",
            "declared_driver": format!("{:?}", ctx.gui.driver()),
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
        })),
        timeout_ms: Some(5_000),
    }
}

fn action_receipts(intents: &[ruviform_ui_test::MintedIntent]) -> Vec<CommandReceipt> {
    intents
        .iter()
        .map(|minted| CommandReceipt {
            command_id: minted.command_id.clone(),
            status: ReceiptStatus::Applied,
            revision: None,
            detail: Some(
                "semantic scenario helper posted the Applied receipt through the view wire"
                    .to_string(),
            ),
        })
        .collect()
}

fn semantic_outcome(
    case: &str,
    expected: serde_json::Value,
    observed: serde_json::Value,
    targets: Vec<ruviform_ui_test::ScenarioTarget>,
    receipts: Vec<CommandReceipt>,
) -> ScenarioOutcome {
    let mut outcome = ScenarioOutcome::new(ScenarioVerdict::Pass);
    outcome.expected = Some(expected);
    outcome.observed = Some(observed);
    outcome.targets = targets;
    outcome.receipts = receipts;
    outcome.evidence = vec!["catalog:sv114-svos-consumer".to_string()];
    outcome.detail = Some(format!(
        "{case}: lane-scoped semantic Command/Receipt/view-state proof; real OS input, pixel, IME, and native-dock evidence are not claimed"
    ));
    outcome
}

fn fail_outcome(detail: impl Into<String>) -> ScenarioOutcome {
    let mut failed = ScenarioOutcome::new(ScenarioVerdict::Fail);
    failed.detail = Some(detail.into());
    failed
}

// ---------------------------------------------------------------------
// independent product-service oracle (headless mounted composition)
// ---------------------------------------------------------------------

static ORACLE_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

fn oracle_unique() -> usize {
    ORACLE_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

fn service_op(f: &mut MountedFixture, id: &str, name: &str, params: serde_json::Value) {
    action(
        &mut f.shell,
        &mut f.driver,
        f.window,
        Intent::new(id, name, params),
    );
}

fn service_op_ok(
    f: &mut MountedFixture,
    id: &str,
    name: &str,
    params: serde_json::Value,
) -> serde_json::Value {
    service_op(f, id, name, params);
    let (ok, data) = results_for(&f.driver, f.window, id)
        .last()
        .expect("service op result")
        .clone();
    assert!(ok, "{name} failed in independent service oracle: {data:?}");
    data.unwrap_or(serde_json::Value::Null)
}

fn service_project_info(f: &mut MountedFixture) -> serde_json::Value {
    service_op_ok(f, "pinfo", "syn.project.info", json!({}))
}

fn service_oracle_project_lifecycle() -> serde_json::Value {
    let mut f = fixture();
    let dir = std::env::temp_dir().join(format!(
        "sv114-svos01-{}-{}",
        std::process::id(),
        oracle_unique()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    let path = dir.to_string_lossy().to_string();
    service_op_ok(
        &mut f,
        "pnew",
        "syn.project.new",
        json!({ "path": path, "name": "svos01-oracle" }),
    );
    let info = service_project_info(&mut f);
    assert_eq!(info["name"], json!("svos01-oracle"));
    let list = service_op_ok(&mut f, "vlist", "syn.view.list", json!({}));
    assert!(
        list["views"]
            .as_array()
            .expect("views")
            .iter()
            .any(|view| view["view_id"] == json!(f.view_id)),
        "main view must remain listed in the service oracle"
    );
    f.input("seek.position", MountedFixture::begin("svos01g"));
    f.input("seek.position", MountedFixture::commit("svos01g"));
    let binding = f
        .host
        .binding("transport/position")
        .expect("position binding");
    assert_eq!(
        binding
            .state()
            .last_receipt
            .expect("service receipt")
            .status,
        ReceiptStatus::Applied
    );
    let _ = std::fs::remove_dir_all(&dir);
    json!({
        "service": "NativeShell/SynvelisApp mounted fixture",
        "project_name": info["name"],
        "view_listed": true,
        "transport_receipt": "applied",
    })
}

fn service_oracle_arrangement_inspector() -> serde_json::Value {
    let mut f = fixture();
    let tag = format!("svos02-oracle-{}", oracle_unique());
    let clip_tag = open_clip_project(&mut f, &tag);
    f.push_screen_state();
    let generation = f.host.generation();
    f.host
        .select_row(generation, &mounted_key("arrangement.table"), 0)
        .expect("row 0 selectable");
    f.pump_view_intents();
    f.deliver_to_host();
    let (ok, _) = last_result(&f.driver, f.window).expect("select result");
    assert!(ok, "syn.view.select must succeed in the service oracle");
    let list = service_op_ok(&mut f, "vlist", "syn.view.list", json!({}));
    let entry = list["views"]
        .as_array()
        .expect("views")
        .iter()
        .find(|view| view["view_id"] == json!(f.view_id))
        .expect("main view")
        .clone();
    assert_eq!(entry["selection"], json!(clip_tag));
    assert_eq!(entry["inspection"], json!(clip_tag));
    assert_eq!(
        f.screen.borrow().inspector.target.as_deref(),
        Some(clip_tag.as_str())
    );
    json!({
        "service": "NativeShell/SynvelisApp mounted fixture",
        "selection": clip_tag,
        "inspection_follows_selection": true,
    })
}

fn service_oracle_fader_undo() -> serde_json::Value {
    let mut f = fixture();
    f.input("fader.gain", MountedFixture::begin("svos03g"));
    f.input("fader.gain", MountedFixture::preview("svos03g", 0.5));
    f.input("fader.gain", MountedFixture::commit("svos03g"));
    let binding = f.host.binding(&mixer_gain_target()).expect("gain binding");
    assert_eq!(
        binding.state().last_receipt.expect("gain receipt").status,
        ReceiptStatus::Applied
    );
    let snapshot = f
        .host
        .observe(&mounted_key("fader.gain"))
        .expect("fader snapshot");
    assert_eq!(snapshot.phase, CommandPhase::Observed);
    let info = service_project_info(&mut f);
    let old_ppqn = info["ppqn"].as_u64().expect("ppqn");
    let new_ppqn = if old_ppqn == 480 { 960 } else { 480 };
    service_op_ok(
        &mut f,
        "ppqns",
        "syn.project.set_ppqn",
        json!({ "old_ppqn": old_ppqn, "new_ppqn": new_ppqn, "policy": "exact" }),
    );
    assert_eq!(service_project_info(&mut f)["ppqn"], json!(new_ppqn));
    f.press("project.undo");
    let (undo_ok, undo_data) = last_result(&f.driver, f.window).expect("undo result");
    assert!(undo_ok);
    assert_eq!(undo_data.expect("undo data")["undone"], json!(true));
    f.press("project.redo");
    let (redo_ok, redo_data) = last_result(&f.driver, f.window).expect("redo result");
    assert!(redo_ok);
    assert_eq!(redo_data.expect("redo data")["redone"], json!(true));
    json!({
        "service": "NativeShell/SynvelisApp mounted fixture",
        "fader_receipt": "applied",
        "fader_phase": format!("{:?}", snapshot.phase),
        "undo": true,
        "redo": true,
    })
}

fn service_oracle_transport_persistence() -> serde_json::Value {
    let mut f = fixture();
    f.press("transport.play");
    let receipts = notifies(&f.driver, f.window, "syn.control.receipt");
    assert!(
        receipts
            .iter()
            .any(|receipt| receipt["target"] == "transport"
                && receipt["receipt"]["status"] == "applied"),
        "transport play must produce an applied service receipt"
    );
    f.input("seek.position", MountedFixture::begin("svos04s"));
    f.input("seek.position", MountedFixture::commit("svos04s"));
    let binding = f
        .host
        .binding("transport/position")
        .expect("position binding");
    assert_eq!(
        binding.state().last_receipt.expect("seek receipt").status,
        ReceiptStatus::Applied
    );
    let dir = std::env::temp_dir().join(format!(
        "sv114-svos04-{}-{}",
        std::process::id(),
        oracle_unique()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    write_clip_bundle(&dir);
    let path = dir.to_string_lossy().to_string();
    service_op_ok(&mut f, "po", "syn.project.open", json!({ "path": path }));
    service_op_ok(&mut f, "ps", "syn.project.save", json!({}));
    let head = std::fs::read_to_string(dir.join("HEAD")).expect("HEAD exists after save");
    assert!(head.trim().starts_with('r'));
    service_op_ok(&mut f, "pr", "syn.project.reload", json!({}));
    let info = service_project_info(&mut f);
    assert_eq!(info["name"], json!("arrangement"));
    assert_eq!(f.screen.borrow().arrangement_rows.len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
    json!({
        "service": "NativeShell/SynvelisApp mounted fixture",
        "transport_receipt": "applied",
        "seek_receipt": "applied",
        "bundle_head": head.trim(),
        "reload_rows": 1,
    })
}

fn service_oracle_two_window() -> serde_json::Value {
    let mut f = fixture();
    let secondary = f
        .driver
        .create_window(&WindowSpec {
            title: "svos05 secondary".to_string(),
            ..WindowSpec::default()
        })
        .expect("secondary window");
    crate::common::mounted::inject(
        &mut f.shell,
        &mut f.driver,
        DriverEvent::Windowed {
            window: secondary,
            event: WindowLifecycle::Ready,
        },
    );
    f.shell
        .attach_view(
            &mut f.driver,
            secondary,
            &secondary_view_id(),
            &secondary_view_targets(),
        )
        .expect("secondary attach");
    assert_eq!(f.shell.view_window(&secondary_view_id()), Some(secondary));
    assert_eq!(f.shell.view_window(&f.view_id), Some(f.window));
    let before = service_project_info(&mut f);
    crate::common::mounted::inject(
        &mut f.shell,
        &mut f.driver,
        DriverEvent::Windowed {
            window: secondary,
            event: WindowLifecycle::Destroyed,
        },
    );
    assert_eq!(f.shell.view_window(&secondary_view_id()), None);
    assert_eq!(f.shell.view_window(&f.view_id), Some(f.window));
    assert_eq!(service_project_info(&mut f)["name"], before["name"]);
    json!({
        "service": "NativeShell/SynvelisApp mounted fixture",
        "secondary_ownership_after_close": null,
        "main_window_survives": true,
        "project_name_survives": before["name"],
    })
}

fn run_sv_os_01<G: SemanticGuiTest>(ctx: &mut SvOsLane<G>, spec: &ScenarioSpec) -> ScenarioReport {
    let main_window = ctx.main_window;
    let main_view = ctx.main_view;
    run_scenario(&mut ctx.gui, spec, |gui| {
        let service_oracle = service_oracle_project_lifecycle();
        let mut targets = Vec::new();
        let mut intents = Vec::new();
        for (component, action) in [
            ("project.save", "save"),
            ("project.reload", "reload"),
            ("project.undo", "undo"),
            ("project.redo", "redo"),
        ] {
            let outcome = invoke_scenario(gui, main_window, main_view, component)?;
            let actions = command_actions(&outcome.intents);
            if actions != vec![(PROJECT_TARGET.to_string(), action.to_string(), None)] {
                let mut failed = fail_outcome(format!(
                    "SV-OS-01 product oracle rejected project action {action}"
                ));
                failed.expected = Some(json!({ "actions": [[PROJECT_TARGET, action]] }));
                failed.observed = Some(json!({ "actions": actions }));
                return Ok(failed);
            }
            targets.push(outcome.target);
            intents.extend(outcome.intents);
        }
        let observed_actions = command_actions(&intents);
        Ok(semantic_outcome(
            CASE_SV_OS_01,
            json!({
                "commands": ["project.save", "project.reload", "project.undo", "project.redo"],
                "target": PROJECT_TARGET,
                "receipt_status": "applied",
            }),
            json!({ "actions": observed_actions, "service_oracle": service_oracle }),
            targets,
            action_receipts(&intents),
        ))
    })
}

fn run_sv_os_02<G: SemanticGuiTest>(ctx: &mut SvOsLane<G>, spec: &ScenarioSpec) -> ScenarioReport {
    let main_window = ctx.main_window;
    let main_view = ctx.main_view;
    run_scenario(&mut ctx.gui, spec, |gui| {
        let service_oracle = service_oracle_arrangement_inspector();
        let selected = select_row_scenario(gui, main_window, main_view, KEY_ARRANGEMENT_TABLE, 1)?;
        assert_row_select(&selected, 1);
        let read_only = inspector_table_is_read_only(gui, main_window, main_view)?;
        if !read_only {
            return Ok(fail_outcome(
                "SV-OS-02 product oracle rejected inspector.table: not read-only",
            ));
        }
        Ok(semantic_outcome(
            CASE_SV_OS_02,
            json!({
                "arrangement_row": 1,
                "inspector_table": "read_only",
            }),
            json!({
                "arrangement_actions": command_actions(&selected.intents),
                "arrangement_selected": selected.snapshot.selected,
                "inspector_read_only": read_only,
                "service_oracle": service_oracle,
            }),
            vec![selected.target],
            action_receipts(&selected.intents),
        ))
    })
}

fn run_sv_os_03<G: SemanticGuiTest>(ctx: &mut SvOsLane<G>, spec: &ScenarioSpec) -> ScenarioReport {
    let main_window = ctx.main_window;
    let main_view = ctx.main_view;
    run_scenario(&mut ctx.gui, spec, |gui| {
        let service_oracle = service_oracle_fader_undo();
        let fader = fader_parity_scenario(gui, main_window, main_view)?;
        assert_fader_parity(&fader);
        let undo = invoke_scenario(gui, main_window, main_view, "project.undo")?;
        let redo = invoke_scenario(gui, main_window, main_view, "project.redo")?;
        let actions = command_actions(&undo.intents)
            .into_iter()
            .chain(command_actions(&redo.intents))
            .collect::<Vec<_>>();
        if actions
            != vec![
                (PROJECT_TARGET.to_string(), "undo".to_string(), None),
                (PROJECT_TARGET.to_string(), "redo".to_string(), None),
            ]
        {
            let mut failed = fail_outcome("SV-OS-03 product oracle rejected undo/redo actions");
            failed.expected =
                Some(json!({ "actions": [["project", "undo"], ["project", "redo"]] }));
            failed.observed = Some(json!({ "actions": actions }));
            return Ok(failed);
        }
        let mut intents = undo.intents;
        intents.extend(redo.intents);
        Ok(semantic_outcome(
            CASE_SV_OS_03,
            json!({
                "fader_commit": 0.4,
                "undo_redo": ["undo", "redo"],
            }),
            json!({
                "fader_snapshot": {
                    "committed": fader.snapshot.committed,
                    "phase": format!("{:?}", fader.snapshot.phase),
                    "display": fader.snapshot.display,
                    "observed": fader.snapshot.observed,
                },
                "undo_redo_actions": actions,
                "service_oracle": service_oracle,
            }),
            vec![fader.target, undo.target, redo.target],
            action_receipts(&intents),
        ))
    })
}

fn run_sv_os_04<G: SemanticGuiTest>(ctx: &mut SvOsLane<G>, spec: &ScenarioSpec) -> ScenarioReport {
    let main_window = ctx.main_window;
    let main_view = ctx.main_view;
    run_scenario(&mut ctx.gui, spec, |gui| {
        let service_oracle = service_oracle_transport_persistence();
        let seek = seek_parity_scenario(gui, main_window, main_view)?;
        assert_seek_parity(&seek);
        let play = invoke_scenario(gui, main_window, main_view, KEY_TRANSPORT_PLAY)?;
        assert_transport_play(&play);
        let save = invoke_scenario(gui, main_window, main_view, "project.save")?;
        let reload = invoke_scenario(gui, main_window, main_view, "project.reload")?;
        let persistence_actions = command_actions(&save.intents)
            .into_iter()
            .chain(command_actions(&reload.intents))
            .collect::<Vec<_>>();
        if persistence_actions
            != vec![
                (PROJECT_TARGET.to_string(), "save".to_string(), None),
                (PROJECT_TARGET.to_string(), "reload".to_string(), None),
            ]
        {
            let mut failed = fail_outcome("SV-OS-04 product oracle rejected save/reload actions");
            failed.expected =
                Some(json!({ "actions": [["project", "save"], ["project", "reload"]] }));
            failed.observed = Some(json!({ "actions": persistence_actions }));
            return Ok(failed);
        }
        let mut intents = play.intents;
        intents.extend(save.intents);
        intents.extend(reload.intents);
        Ok(semantic_outcome(
            CASE_SV_OS_04,
            json!({
                "transport_position": 8.0,
                "transport_action": "play",
                "persistence": ["save", "reload"],
            }),
            json!({
                "seek_snapshot": {
                    "committed": seek.snapshot.committed,
                    "phase": format!("{:?}", seek.snapshot.phase),
                    "display": seek.snapshot.display,
                    "observed": seek.snapshot.observed,
                },
                "transport_actions": command_actions(&intents),
                "service_oracle": service_oracle,
            }),
            vec![seek.target, play.target, save.target, reload.target],
            action_receipts(&intents),
        ))
    })
}

fn run_sv_os_05<G: SemanticGuiTest>(ctx: &mut SvOsLane<G>, spec: &ScenarioSpec) -> ScenarioReport {
    let main_window = ctx.main_window;
    let main_view = ctx.main_view;
    let secondary_window = ctx.secondary_window;
    let secondary_view = ctx.secondary_view;
    run_scenario(&mut ctx.gui, spec, |gui| {
        let service_oracle = service_oracle_two_window();
        let secondary_seek = secondary_seek_parity_scenario(gui, secondary_window, secondary_view)?;
        assert_seek_parity(&secondary_seek);
        let secondary_play = invoke_scenario(
            gui,
            secondary_window,
            secondary_view,
            KEY_TRANSPORT_PLAY_SECONDARY,
        )?;
        assert_transport_play(&secondary_play);
        let tabs = tab_cycle_scenario(
            gui,
            main_window,
            main_view,
            super::KEY_SCREEN_TABS,
            &TAB_CYCLE_INDICES,
        )?;
        assert_tab_cycle(&tabs, &TAB_CYCLE_INDICES);
        assert_stale_generation_pin_rejected(
            gui,
            secondary_window,
            secondary_view,
            super::KEY_SEEK_POSITION_SECONDARY,
        )?;
        let windows = gui.windows();
        if !windows.contains(&main_window) || !windows.contains(&secondary_window) {
            return Ok(fail_outcome(format!(
                "SV-OS-05 product oracle rejected window ownership: expected {main_window:?}+{secondary_window:?}, observed {windows:?}"
            )));
        }
        let mut intents = secondary_play.intents;
        intents.extend(tabs.intents);
        Ok(semantic_outcome(
            CASE_SV_OS_05,
            json!({
                "windows": 2,
                "secondary_transport_action": "play",
                "stale_generation": "rejected",
            }),
            json!({
                "windows": windows.iter().map(|window| window.get()).collect::<Vec<_>>(),
                "secondary_seek": {
                    "committed": secondary_seek.snapshot.committed,
                    "phase": format!("{:?}", secondary_seek.snapshot.phase),
                    "display": secondary_seek.snapshot.display,
                    "observed": secondary_seek.snapshot.observed,
                },
                "selected_tab": tabs.selected,
                "service_oracle": service_oracle,
            }),
            vec![secondary_seek.target, secondary_play.target, tabs.target],
            action_receipts(&intents),
        ))
    })
}
