//! synvelis#114 SV-GUI-05 — SV-OS consumer catalog の real-OS lane。
//!
//! `ruviform-ui-gpui/real-scenario` の `RealOsGuiTest` で製品の Main/Secondary
//! screen spec を実 OS window へ mount し、headless lane と同じ catalog entries
//! を実行する。interactive desktop を利用できない場合は fail-closed に全件
//! `NotRun` を記録して終了する。SV-OS-01 では `project.save` に実 OS pointer
//! injection と pixel capture の最小縦断を追加するが、OS automation injection を
//! physical manual input と呼ばず、IME/native dock pass へは昇格させない。

#![cfg(all(any(windows, target_os = "linux"), feature = "real-os-scenario"))]

mod common;

use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use common::scenario::sv_os_catalog::{SvOsLane, sv_os_entries, sv_os_run_id};
use common::scenario::{command_actions, screen_state};
use ruviform_ui::behavior::{CommandPhase, CommandReceipt, ControlInput, ReceiptStatus};
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::ipc::Event;
use ruviform_ui::view::{
    AttachEpochAllocator, ComponentKey, ComponentSnapshot, ViewHost, ViewSpec,
};
use ruviform_ui::{LogicalPosition, WindowId, WindowSpec};
use ruviform_ui_gpui::os_input::{OsInputOutcome, PreflightScope};
use ruviform_ui_gpui::{RealOsGuiTest, RealWindowMount, RealWindowOptions, ViewMount};
use ruviform_ui_test::{
    CaptureOutcome, CaseId, MintedIntent, ScenarioDriver, ScenarioError, ScenarioId,
    ScenarioReport, ScenarioTarget, ScenarioVerdict, SemanticGuiTest, TargetQuery, VisualGuiTest,
    run_entries, settle_until,
};
use serde_json::{Value, json};
use synvelis_app::binding::PROJECT_TARGET;
use synvelis_app::{
    main_screen_spec, main_view_instance, secondary_screen_spec, secondary_view_instance,
};

const REAL_EVIDENCE_ENV: &str = "SYNVELIS_SV114_REAL_EVIDENCE";
const EVIDENCE_SCHEMA: &str = "sv114-svos-real/v1";

#[cfg(windows)]
const STARTUP_SETTLE_TICKS: u32 = 30;
#[cfg(target_os = "linux")]
const STARTUP_SETTLE_TICKS: u32 = 600;

/// Session-scoped adapter handle. `RealOsGuiTest` exists only as `&mut`
/// inside `RealOsGuiTest::run`, while the catalog context is generic over an
/// adapter value. This local wrapper delegates every port call unchanged, so
/// catalog entries still execute the real adapter and no second runner or
/// evidence authority is introduced.
struct RealGui<'a>(&'a mut RealOsGuiTest);

impl SemanticGuiTest for RealGui<'_> {
    fn driver(&self) -> ScenarioDriver {
        self.0.driver()
    }

    fn capabilities(&self) -> BTreeSet<ruviform_ui_test::ScenarioCapability> {
        self.0.capabilities()
    }

    fn windows(&self) -> Vec<WindowId> {
        self.0.windows()
    }

    fn resolve_target(&mut self, query: &TargetQuery) -> Result<ScenarioTarget, ScenarioError> {
        self.0.resolve_target(query)
    }

    fn dispatch_input(
        &mut self,
        target: &ScenarioTarget,
        key: &ComponentKey,
        input: ControlInput,
    ) -> Result<(), ScenarioError> {
        self.0.dispatch_input(target, key, input)
    }

    fn select_row(
        &mut self,
        target: &ScenarioTarget,
        key: &ComponentKey,
        row: usize,
    ) -> Result<(), ScenarioError> {
        self.0.select_row(target, key, row)
    }

    fn select_tab(
        &mut self,
        target: &ScenarioTarget,
        key: &ComponentKey,
        index: usize,
    ) -> Result<(), ScenarioError> {
        self.0.select_tab(target, key, index)
    }

    fn invoke(&mut self, target: &ScenarioTarget, key: &ComponentKey) -> Result<(), ScenarioError> {
        self.0.invoke(target, key)
    }

    fn observe(
        &self,
        target: &ScenarioTarget,
        key: &ComponentKey,
    ) -> Result<ComponentSnapshot, ScenarioError> {
        self.0.observe(target, key)
    }

    fn settle(&mut self) -> Result<(), ScenarioError> {
        self.0.settle()
    }

    fn drain_intents(&mut self) -> Vec<MintedIntent> {
        self.0.drain_intents()
    }

    fn post_event(&mut self, target: &ScenarioTarget, event: &Event) -> Result<(), ScenarioError> {
        self.0.post_event(target, event)
    }
}

/// Result of the SV-OS-01 real-input/pixel vertical probe. The probe is
/// deliberately outside the shared catalog body: Fake and GPUI-simulated lanes
/// keep their semantic-only verdicts, while this real-only record is merged
/// into the same SV-OS-01 run/case row.
struct InputPixelAugmentation {
    evidence: Value,
    verdict: ScenarioVerdict,
    detail: String,
    receipts: Vec<CommandReceipt>,
    target: Option<ScenarioTarget>,
}

fn verdict_rank(verdict: ScenarioVerdict) -> u8 {
    match verdict {
        ScenarioVerdict::Pass => 0,
        ScenarioVerdict::Blocked => 2,
        ScenarioVerdict::Unsupported => 3,
        ScenarioVerdict::Fail => 4,
        _ => 1,
    }
}

fn worse_verdict(left: ScenarioVerdict, right: ScenarioVerdict) -> ScenarioVerdict {
    if verdict_rank(left) >= verdict_rank(right) {
        left
    } else {
        right
    }
}

fn os_op_json(outcome: &OsInputOutcome) -> Value {
    match outcome {
        OsInputOutcome::Injected {
            action,
            commanded,
            cursor,
            detail,
        } => json!({
            "tag": "injected",
            "action": action,
            "commanded": commanded,
            "cursor": cursor,
            "detail": detail,
        }),
        OsInputOutcome::Unsupported { reason } => json!({
            "tag": "unsupported",
            "detail": reason,
        }),
        OsInputOutcome::EnvironmentBlocked { detail } => json!({
            "tag": "environment_blocked",
            "detail": detail,
        }),
        OsInputOutcome::NotRun { reason } => json!({
            "tag": "not_run",
            "detail": reason,
        }),
        _ => json!({ "tag": "unknown" }),
    }
}

fn ops_json(ops: &[(&'static str, OsInputOutcome)]) -> Value {
    let mut object = serde_json::Map::new();
    for (name, outcome) in ops {
        object.insert((*name).to_string(), os_op_json(outcome));
    }
    Value::Object(object)
}

fn first_non_injected<'a>(ops: &'a [(&'static str, OsInputOutcome)]) -> Option<&'a OsInputOutcome> {
    ops.iter()
        .map(|(_, outcome)| outcome)
        .find(|outcome| !matches!(outcome, OsInputOutcome::Injected { .. }))
}

fn os_input_verdict(outcome: &OsInputOutcome) -> ScenarioVerdict {
    match outcome {
        OsInputOutcome::Unsupported { .. } => ScenarioVerdict::Unsupported,
        OsInputOutcome::EnvironmentBlocked { .. } | OsInputOutcome::NotRun { .. } => {
            ScenarioVerdict::NotRun
        }
        _ => ScenarioVerdict::NotRun,
    }
}

fn snapshot_json(snapshot: &ComponentSnapshot) -> Value {
    json!({
        "mounted": snapshot.mounted,
        "phase": snapshot.phase,
        "display": snapshot.display,
        "committed": snapshot.committed,
        "observed": snapshot.observed,
        "selected": snapshot.selected,
    })
}

fn post_real_applied_receipts(
    gui: &mut RealOsGuiTest,
    target: &ScenarioTarget,
    intents: &[MintedIntent],
) -> Result<Vec<CommandReceipt>, ScenarioError> {
    for minted in intents {
        gui.post_event(
            target,
            &Event::Result {
                id: minted.command_id.clone(),
                ok: true,
                data: Some(json!({ "status": "applied" })),
                error: None,
            },
        )?;
    }
    Ok(intents
        .iter()
        .map(|minted| CommandReceipt {
            command_id: minted.command_id.clone(),
            status: ReceiptStatus::Applied,
            revision: None,
            detail: Some(
                "real-input harness posted the Applied receipt through the view wire; this is not a production service receipt"
                    .to_string(),
            ),
        })
        .collect())
}

fn capture_project_save(
    gui: &mut RealOsGuiTest,
    target: &ScenarioTarget,
) -> Result<(Value, ScenarioVerdict), ScenarioError> {
    match gui.capture_frame(target)? {
        CaptureOutcome::Captured(image) => {
            let pixel_count = image.pixel_count();
            let meta = gui.last_capture_meta();
            let capture = json!({
                "outcome": "captured",
                "backend": meta.map(|m| m.backend),
                "width": meta.map(|m| m.width),
                "height": meta.map(|m| m.height),
                "scale_factor": meta.map(|m| m.scale_factor),
                "pixel_count": pixel_count,
            });
            let verdict = if pixel_count > 0 {
                ScenarioVerdict::Pass
            } else {
                ScenarioVerdict::Fail
            };
            Ok((capture, verdict))
        }
        CaptureOutcome::Unsupported { reason } => Ok((
            json!({
                "outcome": "unsupported",
                "reason": reason,
            }),
            ScenarioVerdict::Unsupported,
        )),
        CaptureOutcome::NotRun { reason } => Ok((
            json!({
                "outcome": "not_run",
                "reason": reason,
            }),
            ScenarioVerdict::NotRun,
        )),
        _ => Ok((json!({ "outcome": "unknown" }), ScenarioVerdict::NotRun)),
    }
}

type CommandActionRow = (String, String, Option<Value>);

fn preflight_json(gui: &mut RealOsGuiTest) -> Value {
    let preflight = gui.os_input_preflight(PreflightScope::Pointer);
    json!({
        "effective": preflight.effective,
        "checks": preflight.checks.iter().map(|check| json!({
            "family": check.family,
            "effective": check.effective,
            "detail": check.detail,
        })).collect::<Vec<_>>(),
    })
}

fn project_save_focus_json(gui: &mut RealOsGuiTest, window: WindowId) -> Value {
    let focus_request = gui.request_focus(window, true);
    let focus_requested = focus_request.is_ok();
    let focus_request_error = focus_request.err().map(|error| error.to_string());
    let focus_wait = if focus_requested {
        Some(settle_until(gui, |gui| gui.window_focused(window), 30))
    } else {
        None
    };
    let focus_observed = focus_wait
        .as_ref()
        .and_then(|result| result.as_ref().ok())
        .copied()
        .unwrap_or(false);
    let focus_wait_error =
        focus_wait.and_then(|result| result.err().map(|error| error.to_string()));
    json!({
        "requested": focus_requested,
        "request_error": focus_request_error,
        "observed": focus_observed,
        "wait_error": focus_wait_error,
    })
}

fn inject_project_save_pointer(
    gui: &mut RealOsGuiTest,
    target: &ScenarioTarget,
    key: &ComponentKey,
) -> Result<Vec<(&'static str, OsInputOutcome)>, ScenarioError> {
    let mut ops = Vec::new();
    let move_outcome = gui.os_pointer_move_to(target, key, (0.5, 0.5))?;
    let move_injected = matches!(move_outcome, OsInputOutcome::Injected { .. });
    ops.push(("pointer-move", move_outcome));
    if move_injected {
        let down_outcome = gui.os_mouse_down(target)?;
        let down_injected = matches!(down_outcome, OsInputOutcome::Injected { .. });
        ops.push(("mouse-down", down_outcome));
        if down_injected {
            let up_outcome = gui.os_mouse_up(target)?;
            ops.push(("mouse-up", up_outcome));
        }
    }
    Ok(ops)
}

struct InputPixelEvidenceInput<'a> {
    window: WindowId,
    preflight: Value,
    focus: Value,
    before: &'a ComponentSnapshot,
    ops: &'a [(&'static str, OsInputOutcome)],
    expected_actions: &'a [CommandActionRow],
    actions: &'a [CommandActionRow],
    intent_count: usize,
    receipts: &'a [CommandReceipt],
    receipts_applied: bool,
    after: &'a ComponentSnapshot,
    product_state_oracle: Option<&'a Value>,
    capture: &'a Value,
    input_verdict: ScenarioVerdict,
    capture_verdict: ScenarioVerdict,
    verdict: ScenarioVerdict,
}

fn input_pixel_evidence(input: &InputPixelEvidenceInput<'_>) -> Value {
    json!({
        "component": "project.save",
        "window": input.window.get(),
        "input_method": "OS automation injection through RealOsGuiTest; not physical manual input",
        "preflight": input.preflight,
        "focus": input.focus,
        "before": snapshot_json(input.before),
        "ops": ops_json(input.ops),
        "command": {
            "expected_actions": input.expected_actions,
            "actions": input.actions,
            "intent_count": input.intent_count,
            "matched": input.actions == input.expected_actions,
        },
        "receipts": {
            "origin": "test harness posted Applied receipts through the consumer-to-view wire after OS input; not a production service receipt",
            "count": input.receipts.len(),
            "applied": input.receipts_applied,
            "items": input.receipts,
        },
        "settled_observation": snapshot_json(input.after),
        "product_state_oracle": input.product_state_oracle,
        "capture": input.capture,
        "verdicts": {
            "input": input.input_verdict,
            "capture": input.capture_verdict,
            "combined": input.verdict,
        },
    })
}

fn collect_sv_os_01_input_pixel(
    gui: &mut RealOsGuiTest,
    window: WindowId,
    view: ViewInstanceId,
    product_state_oracle: Option<&Value>,
) -> Result<InputPixelAugmentation, ScenarioError> {
    let key = ComponentKey::from("project.save");
    let target = gui.resolve_target(&TargetQuery::new(window, view).component(key.clone()))?;
    let preflight = preflight_json(gui);
    let focus = project_save_focus_json(gui, window);
    let before = gui.observe(&target, &key)?;
    let _ = gui.drain_intents();
    let ops = inject_project_save_pointer(gui, &target, &key)?;

    gui.settle()?;
    let intents = gui.drain_intents();
    let receipts = post_real_applied_receipts(gui, &target, &intents)?;
    gui.settle()?;
    let after = gui.observe(&target, &key)?;
    let (capture, capture_verdict) = capture_project_save(gui, &target)?;

    let actions = command_actions(&intents);
    let expected_actions = vec![(PROJECT_TARGET.to_string(), "save".to_string(), None)];
    let command_match = actions == expected_actions;
    let receipts_applied = !intents.is_empty()
        && receipts.len() == intents.len()
        && receipts
            .iter()
            .all(|receipt| receipt.status == ReceiptStatus::Applied);
    let input_pass = first_non_injected(&ops).is_none()
        && ops.len() == 3
        && command_match
        && receipts_applied
        && after.phase == CommandPhase::Applied;
    let input_verdict = if let Some(outcome) = first_non_injected(&ops) {
        os_input_verdict(outcome)
    } else if input_pass {
        ScenarioVerdict::Pass
    } else {
        ScenarioVerdict::Fail
    };
    let verdict = worse_verdict(input_verdict, capture_verdict);
    let detail = if verdict == ScenarioVerdict::Pass {
        "SV-OS-01 real input/pixel augmentation passed for project.save".to_string()
    } else {
        format!(
            "SV-OS-01 real input/pixel augmentation did not pass (input={input_verdict:?}, capture={capture_verdict:?})"
        )
    };
    let evidence = input_pixel_evidence(&InputPixelEvidenceInput {
        window,
        preflight,
        focus,
        before: &before,
        ops: &ops,
        expected_actions: &expected_actions,
        actions: &actions,
        intent_count: intents.len(),
        receipts: &receipts,
        receipts_applied,
        after: &after,
        product_state_oracle,
        capture: &capture,
        input_verdict,
        capture_verdict,
        verdict,
    });

    Ok(InputPixelAugmentation {
        evidence,
        verdict,
        detail,
        receipts,
        target: Some(target),
    })
}

fn apply_sv_os_01_input_pixel(report: &mut ScenarioReport, augmentation: InputPixelAugmentation) {
    let semantic_verdict = report.verdict;
    let augmentation_verdict = augmentation.verdict;
    report.verdict = worse_verdict(report.verdict, augmentation.verdict);
    report.receipts.extend(augmentation.receipts);
    if let Some(target) = augmentation.target
        && !report.targets.contains(&target)
    {
        report.targets.push(target);
    }

    let expected = report.expected.get_or_insert_with(|| json!({}));
    expected
        .as_object_mut()
        .expect("SV-OS-01 semantic expected evidence is an object")
        .insert(
            "real_input_pixel".to_string(),
            json!({
                "component": "project.save",
                "input": "os pointer move/down/up injected",
                "command": "project save Action",
                "receipt": "Applied receipt accepted through the view wire",
                "capture": "Captured pixels with pixel_count > 0",
                "product_state_oracle": "observed.service_oracle",
            }),
        );
    let observed = report.observed.get_or_insert_with(|| json!({}));
    observed
        .as_object_mut()
        .expect("SV-OS-01 semantic observed evidence is an object")
        .insert("real_input_pixel".to_string(), augmentation.evidence);

    report
        .evidence
        .push("real-input-pixel:sv114-svos01-project-save".to_string());
    report.detail = Some(
        if semantic_verdict == ScenarioVerdict::Pass
            && augmentation_verdict == ScenarioVerdict::Pass
        {
            "SV-OS-01: real OS pointer move/down/up on project.save produced the expected Command and harness-posted Applied receipt, and capture_frame produced pixels. The product state oracle is the independent headless service oracle in observed.service_oracle; IME, DPI/monitor matrix, native dock, and production wiring are not claimed".to_string()
        } else {
            format!(
                "SV-OS-01 semantic catalog verdict {semantic_verdict:?}; real input/pixel augmentation verdict {augmentation_verdict:?}: {}",
                augmentation.detail
            )
        },
    );
}

fn augment_sv_os_01_input_pixel(
    gui: &mut RealOsGuiTest,
    window: WindowId,
    view: ViewInstanceId,
    report: &mut ScenarioReport,
) {
    let product_state_oracle = report
        .observed
        .as_ref()
        .and_then(|observed| observed.get("service_oracle"))
        .cloned();
    match collect_sv_os_01_input_pixel(gui, window, view, product_state_oracle.as_ref()) {
        Ok(augmentation) => apply_sv_os_01_input_pixel(report, augmentation),
        Err(error) => {
            let augmentation = InputPixelAugmentation {
                evidence: json!({
                    "component": "project.save",
                    "window": window.get(),
                    "error": error.to_string(),
                    "environment_blocked": false,
                    "product_state_oracle": product_state_oracle,
                }),
                verdict: ScenarioVerdict::Fail,
                detail: format!(
                    "real input/pixel augmentation hit a scenario contract error: {error}"
                ),
                receipts: Vec::new(),
                target: None,
            };
            apply_sv_os_01_input_pixel(report, augmentation);
        }
    }
}

fn product_mount(
    view: ruviform_ui::dock::ViewInstanceId,
    spec: ViewSpec,
    epochs: &mut AttachEpochAllocator,
    title: &str,
    position: Option<LogicalPosition>,
) -> RealWindowMount {
    let mut host = ViewHost::new(view, epochs.mint());
    host.mount(spec).expect("product screen spec mounts");
    host.drain_events();
    RealWindowMount {
        window: WindowSpec {
            title: title.to_string(),
            position,
            ..WindowSpec::default()
        },
        view: ViewMount { host, theme: None },
    }
}

fn real_mounts() -> Vec<RealWindowMount> {
    let state = screen_state();
    let mut epochs = AttachEpochAllocator::new();
    vec![
        product_mount(
            main_view_instance(),
            main_screen_spec(&state),
            &mut epochs,
            "Synvelis SV-OS real main",
            Some(LogicalPosition::new(32, 32)),
        ),
        product_mount(
            secondary_view_instance(),
            secondary_screen_spec(&state),
            &mut epochs,
            "Synvelis SV-OS real secondary",
            Some(LogicalPosition::new(480, 96)),
        ),
    ]
}

fn report_json(report: &ScenarioReport) -> Value {
    json!({
        "type": "case",
        "schema": EVIDENCE_SCHEMA,
        "run_id": report.run_id.as_str(),
        "scenario": report.scenario.as_str(),
        "case": report.case.as_ref().map(CaseId::as_str),
        "driver": report.driver,
        "verdict": report.verdict,
        "lane_status": report.verdict.lane_status(),
        "required": report.required,
        "capabilities": report.capabilities,
        "expected": report.expected,
        "observed": report.observed,
        "receipts": report.receipts,
        "targets": report.targets.len(),
        "environment": report.environment,
        "detail": report.detail,
        "evidence": report.evidence,
    })
}

fn write_evidence(reports: &[ScenarioReport], session: &Value) {
    let Ok(path) = std::env::var(REAL_EVIDENCE_ENV) else {
        return;
    };
    let path = PathBuf::from(path);
    let path = if path.is_absolute() {
        path
    } else {
        // Cargo runs integration tests with the package root as cwd. Anchor
        // consumer-relative evidence paths at the repository root instead.
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(path)
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create evidence directory");
    }
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .expect("open real evidence jsonl");
    writeln!(
        file,
        "{}",
        json!({
            "type": "run",
            "schema": EVIDENCE_SCHEMA,
            "lane": "real-os",
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "session": session,
        })
    )
    .expect("write evidence run row");
    for report in reports {
        writeln!(file, "{}", report_json(report)).expect("write evidence case row");
    }
}

fn blocked_reports(error: &str) -> Vec<ScenarioReport> {
    sv_os_entries::<RealGui<'static>>()
        .iter()
        .map(|entry| {
            let mut report = ScenarioReport::new(
                sv_os_run_id(),
                ScenarioId::new(entry.scenario),
                Some(CaseId::new(entry.case)),
                ScenarioDriver::RealOs,
            );
            report.required = entry.required.iter().copied().collect::<BTreeSet<_>>();
            report.verdict = ScenarioVerdict::NotRun;
            report.detail = Some(format!(
                "environment_blocked: real OS interactive session preflight failed before case execution: {error}"
            ));
            report.environment = Some(json!({
                "lane": "real-os",
                "os": std::env::consts::OS,
                "arch": std::env::consts::ARCH,
                "environment_kind": "deterministic-backend",
                "environment_blocked": true,
            }));
            report
        })
        .collect()
}

fn run_real_catalog_session(
    gui: &mut RealOsGuiTest,
    main_window: WindowId,
    secondary_window: WindowId,
) -> Vec<ScenarioReport> {
    let mut lane = SvOsLane::new(
        RealGui(gui),
        main_window,
        main_view_instance(),
        secondary_window,
        secondary_view_instance(),
    );
    let entries = sv_os_entries::<RealGui<'_>>();
    let mut reports = run_entries(&entries, &sv_os_run_id(), &mut lane);
    let gui = lane.gui.0;
    if let Some(report) = reports.iter_mut().find(|report| {
        report
            .case
            .as_ref()
            .is_some_and(|case| case.as_str() == "SV-OS-01")
    }) {
        augment_sv_os_01_input_pixel(gui, main_window, main_view_instance(), report);
    }

    let main_focus_requested = gui.request_focus(main_window, true).is_ok();
    let main_focused = main_focus_requested
        && settle_until(gui, |gui| gui.window_focused(main_window), 30).unwrap_or(false);
    let secondary_focus_requested = gui.request_focus(secondary_window, true).is_ok();
    let secondary_focused = secondary_focus_requested
        && settle_until(gui, |gui| gui.window_focused(secondary_window), 30).unwrap_or(false);
    let geometry = [main_window, secondary_window]
        .iter()
        .map(|window| {
            json!({
                "window": window.get(),
                "focused": gui.window_focused(*window),
                "geometry": gui.window_geometry(*window).ok().map(|geometry| json!({
                    "x": geometry.origin_x,
                    "y": geometry.origin_y,
                    "width": geometry.width,
                    "height": geometry.height,
                    "scale_factor": geometry.scale_factor,
                })),
                "events": gui.window_events(*window).len(),
            })
        })
        .collect::<Vec<_>>();
    for report in &mut reports {
        let is_two_window_case = report
            .case
            .as_ref()
            .is_some_and(|case| case.as_str() == "SV-OS-05");
        if is_two_window_case && !(main_focused && secondary_focused) {
            report.verdict = ScenarioVerdict::NotRun;
            report.detail = Some(format!(
                "environment_blocked: real windows opened, but OS focus transitions were not observed (main={main_focused}, secondary={secondary_focused}); semantic case evidence remains unclaimed for GUI-L2 focus"
            ));
        }
        report.environment = Some(json!({
            "lane": "real-os",
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "composition": "RealOsGuiTest + product main/secondary ViewHost",
            "backend": "gpui/real-os",
            "environment_kind": "real-window",
            "gpu": "not_observed",
            "focus": {
                "main_requested": main_focus_requested,
                "main_observed": main_focused,
                "secondary_requested": secondary_focus_requested,
                "secondary_observed": secondary_focused,
            },
            "windows": geometry,
        }));
        report.evidence.push(format!("env:{REAL_EVIDENCE_ENV}"));
    }
    reports
}

/// Real OS composition root. Window creation is itself the interactive
/// preflight: when it fails no case body executes and every case is recorded
/// as `NotRun`. When it succeeds the catalog runs unchanged, then focus and
/// geometry facts are stamped into the real-lane rows. SV-OS-05 additionally
/// requires observed focus transitions on both real windows; absent focus is
/// a `NotRun` environment limitation, never a pass.
#[test]
fn real_os_lane_executes_sv_os_catalog_or_records_not_run() {
    let main_window = WindowId::new(1).expect("window id 1");
    let secondary_window = WindowId::new(2).expect("window id 2");
    let outcome = RealOsGuiTest::run(
        real_mounts(),
        RealWindowOptions {
            startup_settle_ticks: STARTUP_SETTLE_TICKS,
            ..RealWindowOptions::default()
        },
        |gui| run_real_catalog_session(gui, main_window, secondary_window),
    );

    match outcome {
        Ok(reports) => {
            assert_eq!(reports.len(), 5);
            for report in &reports {
                assert_ne!(
                    report.verdict,
                    ScenarioVerdict::Fail,
                    "real lane case failed: {:?} / {:?}",
                    report.detail,
                    report.observed,
                );
                assert_eq!(report.driver, ScenarioDriver::RealOs);
            }
            write_evidence(
                &reports,
                &json!({
                    "preflight": "pass",
                    "verdicts": reports.iter().map(|report| report.verdict).collect::<Vec<_>>(),
                }),
            );
        }
        Err(error) => {
            let reports = blocked_reports(&error.to_string());
            assert_eq!(reports.len(), 5);
            assert!(
                reports
                    .iter()
                    .all(|report| report.verdict == ScenarioVerdict::NotRun)
            );
            write_evidence(
                &reports,
                &json!({
                    "preflight": "not_run",
                    "environment_blocked": true,
                    "error": error.to_string(),
                }),
            );
            eprintln!(
                "SV-OS real lane NotRun: environment_blocked: real OS interactive session preflight failed: {error}"
            );
        }
    }
}
