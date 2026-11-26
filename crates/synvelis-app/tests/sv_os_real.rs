//! synvelis#114 SV-GUI-05 — SV-OS consumer catalog の real-OS lane。
//!
//! `ruviform-ui-gpui/real-scenario` の `RealOsGuiTest` で製品の Main/Secondary
//! screen spec を実 OS window へ mount し、headless lane と同じ catalog entries
//! を実行する。interactive desktop を利用できない場合は fail-closed に全件
//! `NotRun` を記録して終了する。semantic dispatch が実 window へ届いたことは
//! 記録するが、physical OS input/pixel/native dock pass へは昇格させない。

#![cfg(all(any(windows, target_os = "linux"), feature = "real-os-scenario"))]

mod common;

use std::collections::BTreeSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;

use common::scenario::screen_state;
use common::scenario::sv_os_catalog::{SvOsLane, sv_os_entries, sv_os_run_id};
use ruviform_ui::behavior::ControlInput;
use ruviform_ui::ipc::Event;
use ruviform_ui::view::{
    AttachEpochAllocator, ComponentKey, ComponentSnapshot, ViewHost, ViewSpec,
};
use ruviform_ui::{LogicalPosition, WindowId, WindowSpec};
use ruviform_ui_gpui::{RealOsGuiTest, RealWindowMount, RealWindowOptions, ViewMount};
use ruviform_ui_test::{
    CaseId, MintedIntent, ScenarioDriver, ScenarioError, ScenarioId, ScenarioReport,
    ScenarioTarget, ScenarioVerdict, SemanticGuiTest, TargetQuery, run_entries, settle_until,
};
use serde_json::{Value, json};
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

fn write_evidence(reports: &[ScenarioReport], session: Value) {
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
                "environment_blocked": true,
            }));
            report
        })
        .collect()
}

/// Real OS composition root. Window creation is itself the interactive
/// preflight: when it fails no case body executes and every case is recorded
/// as NotRun. When it succeeds the catalog runs unchanged, then focus and
/// geometry facts are stamped into the real-lane rows. SV-OS-05 additionally
/// requires observed focus transitions on both real windows; absent focus is
/// a NotRun environment limitation, never a pass.
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
        |gui| {
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

            let main_focus_requested = gui.request_focus(main_window, true).is_ok();
            let main_focused = main_focus_requested
                && settle_until(gui, |gui| gui.window_focused(main_window), 30).unwrap_or(false);
            let secondary_focus_requested = gui.request_focus(secondary_window, true).is_ok();
            let secondary_focused = secondary_focus_requested
                && settle_until(gui, |gui| gui.window_focused(secondary_window), 30)
                    .unwrap_or(false);
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
        },
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
                json!({
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
                json!({
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
