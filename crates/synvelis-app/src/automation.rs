//! AUTX-07 (synvelis#266) — driver-neutral `ViewSpec` / state
//! projection / binding decode for the Action Palette, Macro Browser,
//! Workflow Editor v1, and Run Monitor surfaces.
//!
//! These builders project the [`synvelis_workspace::automation_ui`]
//! read models — themselves pure projections of Gateway/Automation
//! authority — into `ruviform_ui::view` specs. No GPUI/OS type appears
//! here, and no view state becomes project/grant/job truth: the only
//! mutable surface state is the per-view [`AutomationViewState`]
//! carried in `synvelis-workspace`'s `ViewState`.
//!
//! Interactive contract:
//!
//! - Driven `control_id == target` rules from `screens.rs` apply; the
//!   automation surfaces are command-driven (Buttons/Tables/Tabs), so
//!   every intent lands on one of the `AUTX_*_TARGET` logical targets
//!   and decodes through [`decode_automation_command`].
//! - Non-executable capabilities never get action buttons — the spec
//!   itself is fail-closed, not just the handler.
//! - [`decode_automation_command`] is the only place view intents
//!   become [`AutomationViewAction`]s; [`synvelis_workspace::
//!   automation_requests`] is the only place those become gateway wire
//!   requests.
//!
//! **Dependency direction:** `ruviform_ui::view` + `synvelis_workspace`
//! types only — no GPUI/OS/DOM, no `synvelis-gateway` types (gateway
//! vocabulary reaches the spec as display strings). **RT contract:**
//! non-RT only.

use std::cell::RefCell;
use std::rc::Rc;

use ruviform_ui::UiError;
use ruviform_ui::dock::ViewInstanceId;
use ruviform_ui::view::{
    A11ySpec, AttachEpochAllocator, ButtonSpec, CellValue, ComponentSpec, ComponentVisualState,
    LabelSpec, TableSelectionSpec, TableSpec, ViewHost, ViewSpec,
};
use serde_json::Value;
use synvelis_workspace::{
    ActionCatalogView, ActionRow, AutomationViewAction, AutomationViewState, ExplicitTarget,
    MacroBrowserView, MonitorStatus, RunMonitorView, WorkflowPlanView, effect_tag,
    target_contract_summary,
};

use crate::screens::{key, panel};

/// Logical target of palette view-local intents (row select, category
/// tab). The composition layer maps these onto `AutomationViewState`.
pub const AUTX_PALETTE_TARGET: &str = "autx.palette";
/// Logical target of macro-browser view-local intents.
pub const AUTX_MACRO_TARGET: &str = "autx.macro";
/// Logical target of workflow-editor view-local intents.
pub const AUTX_WORKFLOW_TARGET: &str = "autx.workflow";
/// Logical target of run-monitor view-local intents and the cancel
/// command.
pub const AUTX_RUN_TARGET: &str = "autx.run";
/// Logical target of the palette's domain commands (capture / preview
/// / submit) — these lower to gateway wire requests.
pub const AUTX_ACTION_TARGET: &str = "autx.action";

// ── screen states ─────────────────────────────────────────────────────

/// Action Palette screen state — the projected catalog plus the
/// view-local automation state (query/selection/frozen target/
/// outcome). `catalog: None` renders the empty-state spec.
#[derive(Clone, Debug, Default)]
pub struct ActionPaletteScreenState {
    /// Latest applied catalog projection.
    pub catalog: Option<ActionCatalogView>,
    /// Per-view automation state.
    pub view: AutomationViewState,
}

/// Macro Browser screen state.
#[derive(Clone, Debug, Default)]
pub struct MacroBrowserScreenState {
    /// Latest applied macro catalog projection.
    pub catalog: Option<MacroBrowserView>,
    /// Per-view automation state.
    pub view: AutomationViewState,
}

/// Workflow Editor v1 screen state — plan projection only; validation
/// and approval arrive as authority verdicts inside the view.
#[derive(Clone, Debug, Default)]
pub struct WorkflowEditorScreenState {
    /// Latest applied plan projection.
    pub plan: Option<WorkflowPlanView>,
    /// Per-view automation state.
    pub view: AutomationViewState,
}

/// Run Monitor screen state.
#[derive(Clone, Debug, Default)]
pub struct RunMonitorScreenState {
    /// Latest applied monitor projection.
    pub monitor: Option<RunMonitorView>,
    /// Per-view automation state.
    pub view: AutomationViewState,
}

// ── shared spec helpers ───────────────────────────────────────────────

fn label(key_name: &str, a11y_label: &str, text: String) -> ComponentSpec {
    ComponentSpec::Label(LabelSpec {
        key: key(key_name),
        text,
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some(a11y_label.to_string()),
            ..A11ySpec::default()
        },
    })
}

fn action_button(
    key_name: &str,
    label_text: &str,
    target: &str,
    action: &str,
    params: Option<Value>,
) -> ComponentSpec {
    ComponentSpec::Button(ButtonSpec {
        key: key(key_name),
        label: label_text.to_string(),
        target: target.to_string(),
        action: action.to_string(),
        params,
        visual: ComponentVisualState::default(),
        a11y: A11ySpec::default(),
    })
}

fn text_cell(s: impl Into<String>) -> CellValue {
    CellValue::Text(s.into())
}

// ── Action Palette ────────────────────────────────────────────────────

/// Distinct domain categories for the palette tab strip, in first-seen
/// order, always headed by `"all"`.
#[must_use]
pub fn palette_categories(catalog: &ActionCatalogView) -> Vec<String> {
    let mut cats = vec!["all".to_string()];
    for row in &catalog.rows {
        let domain = row.domain.name().to_string();
        if !cats.contains(&domain) {
            cats.push(domain);
        }
    }
    cats
}

fn palette_status_text(state: &ActionPaletteScreenState) -> String {
    match &state.catalog {
        None => "Action Palette — no catalog".to_string(),
        Some(catalog) => {
            let executable = catalog.rows.iter().filter(|r| r.executable).count();
            let blocked = catalog.rows.len() - executable;
            format!(
                "Action Palette — {} capabilities — {} executable — {} blocked — query {:?} — catalog seq {}",
                catalog.rows.len(),
                executable,
                blocked,
                catalog.query,
                catalog.seq,
            )
        }
    }
}

fn palette_detail_text(state: &ActionPaletteScreenState) -> String {
    let Some(catalog) = &state.catalog else {
        return "no catalog".to_string();
    };
    let Some(index) = state.view.palette_selected else {
        return "no selection".to_string();
    };
    let Some(row) = catalog.rows.get(index) else {
        return format!("selection {index} outside catalog");
    };
    let captured = state
        .view
        .palette_target
        .as_ref()
        .filter(|t| t.capability_id == row.id && t.capability_version == row.version)
        .map_or_else(
            || "none".to_string(),
            |t| {
                t.target.as_ref().map_or_else(
                    || format!("captured on {}", t.project),
                    |e| format!("{:?} on {}", e, t.project),
                )
            },
        );
    let outcome = state.view.palette_outcome.as_deref().unwrap_or("none");
    format!(
        "{} — domain {} — availability {} — contract {} — effects [{}] — in {} out {} — permissions [{}] — grant {} — {} — {} — captured {} — outcome {}",
        row.capability_label(),
        row.domain.name(),
        row.availability.name(),
        target_contract_summary(&row.target),
        row.effects
            .iter()
            .map(|e| effect_tag(*e))
            .collect::<Vec<_>>()
            .join(","),
        ActionRow::schema_label(&row.input_schema),
        ActionRow::schema_label(&row.output_schema),
        row.permissions.join(","),
        row.grant.label(),
        if row.executable { "ready" } else { "blocked" },
        row.blocked.as_deref().unwrap_or(""),
        captured,
        outcome,
    )
}

fn palette_table(catalog: &ActionCatalogView) -> ComponentSpec {
    ComponentSpec::Table(TableSpec {
        key: key("autx.palette.table"),
        columns: vec![
            "Capability".to_string(),
            "Domain".to_string(),
            "Availability".to_string(),
            "Target".to_string(),
            "Effects".to_string(),
            "Grant".to_string(),
            "Status".to_string(),
        ],
        rows: catalog
            .rows
            .iter()
            .map(|row| {
                vec![
                    text_cell(format!("{}@{}", row.id, row.version)),
                    text_cell(row.domain.name().to_string()),
                    text_cell(row.availability.name().to_string()),
                    text_cell(target_contract_summary(&row.target)),
                    text_cell(
                        row.effects
                            .iter()
                            .map(|e| effect_tag(*e))
                            .collect::<Vec<_>>()
                            .join(","),
                    ),
                    text_cell(row.grant.label()),
                    text_cell(row.blocked.clone().unwrap_or_else(|| "ready".to_string())),
                ]
                .into_iter()
                .map(Some)
                .collect()
            })
            .collect(),
        selection: Some(TableSelectionSpec {
            target: AUTX_PALETTE_TARGET.to_string(),
        }),
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Action palette capabilities".to_string()),
            ..A11ySpec::default()
        },
    })
}

/// Category switcher for the palette — one `Button` per domain plus
/// `"all"`. Buttons (not `Tabs`) keep a single shared table under one
/// key namespace: each category button emits `category` with
/// `{"index": n}` on [`AUTX_PALETTE_TARGET`], and the composition
/// layer re-projects the catalog filtered by that category.
fn palette_category_buttons(state: &ActionPaletteScreenState) -> ComponentSpec {
    let categories = state
        .catalog
        .as_ref()
        .map_or_else(|| vec!["all".to_string()], palette_categories);
    let active = state.view.palette_category.unwrap_or(0);
    let mut children: Vec<ComponentSpec> = categories
        .iter()
        .enumerate()
        .map(|(index, c)| {
            let marker = if index == active { "● " } else { "" };
            action_button(
                &format!("autx.palette.cat.{index}"),
                &format!("{marker}{c}"),
                AUTX_PALETTE_TARGET,
                "category",
                Some(serde_json::json!({ "index": index })),
            )
        })
        .collect();
    // The table is a sibling, not a tab child — one key, one selection.
    children.push(palette_table(
        state.catalog.as_ref().expect("categories imply catalog"),
    ));
    panel("autx.palette.browser", None, children)
}

fn palette_action_buttons(state: &ActionPaletteScreenState) -> Vec<ComponentSpec> {
    let mut buttons = Vec::new();
    let selected_label = state.view.palette_selected.and_then(|i| {
        state
            .catalog
            .as_ref()
            .and_then(|c| c.rows.get(i))
            .map(|r| format!("{}@{}", r.id, r.version))
    });
    if let Some(label_text) = selected_label {
        let capability = serde_json::json!({ "capability": label_text });
        let params = || Some(capability.clone());
        buttons.push(action_button(
            "autx.action.capture",
            "Capture Target",
            AUTX_ACTION_TARGET,
            "capture",
            params(),
        ));
        // Fail-closed: Preview/Submit exist only for executable rows.
        let executable = state
            .view
            .palette_selected
            .and_then(|i| state.catalog.as_ref()?.rows.get(i))
            .is_some_and(|r| r.executable);
        if executable {
            buttons.push(action_button(
                "autx.action.preview",
                "Preview",
                AUTX_ACTION_TARGET,
                "preview",
                params(),
            ));
            buttons.push(action_button(
                "autx.action.submit",
                "Submit",
                AUTX_ACTION_TARGET,
                "submit",
                params(),
            ));
        }
    }
    buttons
}

/// The Action Palette `ViewSpec` — status readout, domain category
/// tabs, the capability table (selection → view-local), the selected
/// capability detail readout, and fail-closed action buttons.
#[must_use]
pub fn action_palette_screen_spec(state: &ActionPaletteScreenState) -> ViewSpec {
    let mut children = vec![
        label(
            "autx.palette.status",
            "Action palette status",
            palette_status_text(state),
        ),
        label(
            "autx.palette.detail",
            "Selected capability",
            palette_detail_text(state),
        ),
    ];
    if state.catalog.is_some() {
        children.push(palette_category_buttons(state));
        let buttons = palette_action_buttons(state);
        if !buttons.is_empty() {
            children.push(panel("autx.action.buttons", None, buttons));
        }
    } else {
        children.push(label(
            "autx.palette.empty",
            "Palette empty",
            "no capability catalog — the automation adapter has not projected one".to_string(),
        ));
    }
    ViewSpec {
        root: panel("autx.palette.root", Some("Action Palette"), children),
    }
}

// ── Macro Browser ─────────────────────────────────────────────────────

fn macro_status_text(state: &MacroBrowserScreenState) -> String {
    match &state.catalog {
        None => "Macro Browser — no catalog".to_string(),
        Some(catalog) => {
            let executable = catalog.presets.iter().filter(|p| p.executable).count();
            format!(
                "Macro Browser — {} presets — {} applicable — catalog seq {}",
                catalog.presets.len(),
                executable,
                catalog.seq,
            )
        }
    }
}

fn macro_detail_text(state: &MacroBrowserScreenState) -> String {
    let Some(catalog) = &state.catalog else {
        return "no catalog".to_string();
    };
    let Some(preset) = state
        .view
        .macro_selected
        .as_deref()
        .and_then(|id| catalog.presets.iter().find(|p| p.id == id))
    else {
        return "no selection".to_string();
    };
    let steps = preset
        .steps
        .iter()
        .map(|s| s.capability.to_string())
        .collect::<Vec<_>>()
        .join(" → ");
    format!(
        "{}@{} — {} steps [{}] — effects [{}] — undo {} — atomic {} — availability {} — grant {} — inputs [{}] — {}",
        preset.name,
        preset.version,
        preset.steps.len(),
        steps,
        preset
            .effects
            .iter()
            .map(|e| effect_tag(*e))
            .collect::<Vec<_>>()
            .join(","),
        preset.undo_scope,
        preset.atomic,
        preset.availability.name(),
        preset.grant.label(),
        preset
            .inputs
            .iter()
            .map(|i| format!("{}{}", i.name, if i.required { "*" } else { "" }))
            .collect::<Vec<_>>()
            .join(","),
        preset.blocked.as_deref().unwrap_or("ready"),
    )
}

fn macro_table(catalog: &MacroBrowserView) -> ComponentSpec {
    ComponentSpec::Table(TableSpec {
        key: key("autx.macro.table"),
        columns: vec![
            "Preset".to_string(),
            "Version".to_string(),
            "Steps".to_string(),
            "Effects".to_string(),
            "Undo".to_string(),
            "Availability".to_string(),
            "Status".to_string(),
        ],
        rows: catalog
            .presets
            .iter()
            .map(|p| {
                vec![
                    text_cell(p.name.clone()),
                    text_cell(p.version.clone()),
                    text_cell(p.steps.len().to_string()),
                    text_cell(
                        p.effects
                            .iter()
                            .map(|e| effect_tag(*e))
                            .collect::<Vec<_>>()
                            .join(","),
                    ),
                    text_cell(p.undo_scope.clone()),
                    text_cell(p.availability.name().to_string()),
                    text_cell(p.blocked.clone().unwrap_or_else(|| "ready".to_string())),
                ]
                .into_iter()
                .map(Some)
                .collect()
            })
            .collect(),
        selection: Some(TableSelectionSpec {
            target: AUTX_MACRO_TARGET.to_string(),
        }),
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Macro presets".to_string()),
            ..A11ySpec::default()
        },
    })
}

/// The Macro Browser `ViewSpec`.
#[must_use]
pub fn macro_browser_screen_spec(state: &MacroBrowserScreenState) -> ViewSpec {
    let mut children = vec![
        label(
            "autx.macro.status",
            "Macro browser status",
            macro_status_text(state),
        ),
        label(
            "autx.macro.detail",
            "Selected macro",
            macro_detail_text(state),
        ),
    ];
    if let Some(catalog) = &state.catalog {
        children.push(macro_table(catalog));
        let mut buttons = Vec::new();
        if let Some(preset) = state
            .view
            .macro_selected
            .as_deref()
            .and_then(|id| catalog.presets.iter().find(|p| p.id == id))
        {
            let preset_params = serde_json::json!({ "preset": preset.id });
            let params = || Some(preset_params.clone());
            buttons.push(action_button(
                "autx.macro.preview",
                "Preview",
                AUTX_MACRO_TARGET,
                "preview",
                params(),
            ));
            if preset.executable {
                buttons.push(action_button(
                    "autx.macro.apply",
                    "Apply",
                    AUTX_MACRO_TARGET,
                    "apply",
                    params(),
                ));
            }
        }
        if !buttons.is_empty() {
            children.push(panel("autx.macro.buttons", None, buttons));
        }
    } else {
        children.push(label(
            "autx.macro.empty",
            "Macro browser empty",
            "no macro catalog — the automation adapter has not projected one".to_string(),
        ));
    }
    ViewSpec {
        root: panel("autx.macro.root", Some("Macro Browser"), children),
    }
}

// ── Workflow Editor v1 ────────────────────────────────────────────────

fn workflow_status_text(state: &WorkflowEditorScreenState) -> String {
    match &state.plan {
        None => "Workflow Editor — no plan".to_string(),
        Some(plan) => {
            let validation = match &plan.validation {
                synvelis_workspace::PlanValidation::Valid => "valid".to_string(),
                synvelis_workspace::PlanValidation::Invalid { reasons } => {
                    format!("invalid ({} reasons)", reasons.len())
                }
                synvelis_workspace::PlanValidation::Unknown => "unknown".to_string(),
            };
            format!(
                "Workflow — {}@{} — {} steps — validation {} — approval {} — plan seq {}",
                plan.name,
                plan.version,
                plan.steps.len(),
                validation,
                plan.approval,
                plan.seq,
            )
        }
    }
}

fn workflow_detail_text(state: &WorkflowEditorScreenState) -> String {
    let Some(plan) = &state.plan else {
        return "no plan".to_string();
    };
    let validation = match &plan.validation {
        synvelis_workspace::PlanValidation::Valid => "valid".to_string(),
        synvelis_workspace::PlanValidation::Invalid { reasons } => {
            format!("invalid: {}", reasons.join("; "))
        }
        synvelis_workspace::PlanValidation::Unknown => "validation unknown".to_string(),
    };
    let step = state
        .view
        .workflow_step
        .and_then(|i| plan.steps.get(i))
        .map_or_else(
            || "no step selected".to_string(),
            |s| {
                format!(
                    "step {} — {} — {} — capability {} — bound {}",
                    s.index,
                    s.kind.name(),
                    s.label,
                    s.capability
                        .as_ref()
                        .map_or_else(|| "—".to_string(), ToString::to_string),
                    s.bound.as_deref().unwrap_or("—"),
                )
            },
        );
    format!("{validation} — approval: {} — {step}", plan.approval)
}

fn workflow_steps_table(plan: &WorkflowPlanView) -> ComponentSpec {
    ComponentSpec::Table(TableSpec {
        key: key("autx.workflow.steps"),
        columns: vec![
            "#".to_string(),
            "Kind".to_string(),
            "Label".to_string(),
            "Capability".to_string(),
            "Bound".to_string(),
        ],
        rows: plan
            .steps
            .iter()
            .map(|s| {
                vec![
                    text_cell(s.index.to_string()),
                    text_cell(s.kind.name()),
                    text_cell(s.label.clone()),
                    text_cell(
                        s.capability
                            .as_ref()
                            .map_or_else(|| "—".to_string(), ToString::to_string),
                    ),
                    text_cell(s.bound.clone().unwrap_or_else(|| "—".to_string())),
                ]
                .into_iter()
                .map(Some)
                .collect()
            })
            .collect(),
        selection: Some(TableSelectionSpec {
            target: AUTX_WORKFLOW_TARGET.to_string(),
        }),
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Workflow steps".to_string()),
            ..A11ySpec::default()
        },
    })
}

/// The Workflow Editor v1 `ViewSpec` — bounded step visualization,
/// validation verdict, and approval *display*. No action buttons: the
/// v1 slice renders authority state only and never mints approvals or
/// launches runs (GW-09 display contract).
#[must_use]
pub fn workflow_editor_screen_spec(state: &WorkflowEditorScreenState) -> ViewSpec {
    let mut children = vec![
        label(
            "autx.workflow.status",
            "Workflow status",
            workflow_status_text(state),
        ),
        label(
            "autx.workflow.detail",
            "Workflow detail",
            workflow_detail_text(state),
        ),
    ];
    if let Some(plan) = &state.plan {
        children.push(workflow_steps_table(plan));
    } else {
        children.push(label(
            "autx.workflow.empty",
            "Workflow editor empty",
            "no workflow plan — the automation adapter has not projected one".to_string(),
        ));
    }
    ViewSpec {
        root: panel("autx.workflow.root", Some("Workflow Editor"), children),
    }
}

// ── Run Monitor ───────────────────────────────────────────────────────

fn run_status_text(state: &RunMonitorScreenState) -> String {
    match &state.monitor {
        None => "Run Monitor — no data".to_string(),
        Some(monitor) => {
            let running = monitor
                .rows
                .iter()
                .filter(|r| {
                    matches!(
                        r.status,
                        MonitorStatus::Pending
                            | MonitorStatus::Running
                            | MonitorStatus::CancelRequested
                    )
                })
                .count();
            let indeterminate = monitor
                .rows
                .iter()
                .filter(|r| r.status == MonitorStatus::Indeterminate || r.recovery_required)
                .count();
            format!(
                "Run Monitor — {} runs — {} active — {} indeterminate/recovery — monitor seq {}",
                monitor.rows.len(),
                running,
                indeterminate,
                monitor.seq,
            )
        }
    }
}

fn run_detail_text(state: &RunMonitorScreenState) -> String {
    let Some(monitor) = &state.monitor else {
        return "no monitor data".to_string();
    };
    let Some(row) = state
        .view
        .run_selected
        .as_deref()
        .and_then(|id| monitor.rows.iter().find(|r| r.run == id))
    else {
        return "no selection".to_string();
    };
    let job = row.job.as_ref().map_or_else(
        || "no job".to_string(),
        |j| {
            format!(
                "job {} — phase {:?} — cancel {:?} — progress {}",
                j.job,
                j.phase,
                j.cancel,
                j.progress
                    .map_or_else(|| "—".to_string(), |p| format!("{p}%")),
            )
        },
    );
    format!(
        "{} — state {} — status {} — op {} — step {} — rev {} — recovery {} — cancel-requested {} — {}",
        row.run,
        row.run_state,
        row.status.label(),
        row.operation.as_deref().unwrap_or("—"),
        row.step.map_or_else(|| "—".to_string(), |s| s.to_string()),
        row.project_revision
            .map_or_else(|| "—".to_string(), |r| r.0.to_string()),
        row.recovery_required,
        row.cancel_requested,
        job,
    )
}

fn run_table(monitor: &RunMonitorView) -> ComponentSpec {
    ComponentSpec::Table(TableSpec {
        key: key("autx.run.table"),
        columns: vec![
            "Run".to_string(),
            "Operation".to_string(),
            "State".to_string(),
            "Status".to_string(),
            "Cancel".to_string(),
            "Revision".to_string(),
        ],
        rows: monitor
            .rows
            .iter()
            .map(|r| {
                vec![
                    text_cell(r.run.clone()),
                    text_cell(r.operation.clone().unwrap_or_else(|| "—".to_string())),
                    text_cell(r.run_state.clone()),
                    text_cell(r.status.label()),
                    text_cell(if r.cancel_requested {
                        "requested".to_string()
                    } else {
                        "—".to_string()
                    }),
                    text_cell(
                        r.project_revision
                            .map_or_else(|| "—".to_string(), |rev| rev.0.to_string()),
                    ),
                ]
                .into_iter()
                .map(Some)
                .collect()
            })
            .collect(),
        selection: Some(TableSelectionSpec {
            target: AUTX_RUN_TARGET.to_string(),
        }),
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Run monitor".to_string()),
            ..A11ySpec::default()
        },
    })
}

/// The Run Monitor `ViewSpec` — run/step/job status rows with the
/// pending/cancel/indeterminate distinctions preserved verbatim, plus
/// a Cancel button that exists only while a cancellable run is
/// selected (request ≠ confirmed — the linkage axis stays visible).
#[must_use]
pub fn run_monitor_screen_spec(state: &RunMonitorScreenState) -> ViewSpec {
    let mut children = vec![
        label(
            "autx.run.status",
            "Run monitor status",
            run_status_text(state),
        ),
        label("autx.run.detail", "Selected run", run_detail_text(state)),
    ];
    if let Some(monitor) = &state.monitor {
        children.push(run_table(monitor));
        let cancellable = state
            .view
            .run_selected
            .as_deref()
            .and_then(|id| monitor.rows.iter().find(|r| r.run == id))
            .is_some_and(|r| {
                r.operation.is_some()
                    && !matches!(
                        r.status,
                        MonitorStatus::Completed
                            | MonitorStatus::Failed
                            | MonitorStatus::Cancelled
                            | MonitorStatus::Indeterminate
                    )
            });
        if cancellable {
            children.push(panel(
                "autx.run.buttons",
                None,
                vec![action_button(
                    "autx.run.cancel",
                    "Cancel",
                    AUTX_RUN_TARGET,
                    "cancel",
                    None,
                )],
            ));
        }
    } else {
        children.push(label(
            "autx.run.empty",
            "Run monitor empty",
            "no run data — the automation adapter has not projected any".to_string(),
        ));
    }
    ViewSpec {
        root: panel("autx.run.root", Some("Run Monitor"), children),
    }
}

// ── mount ─────────────────────────────────────────────────────────────

fn mount_automation_view(
    attach_epochs: &Rc<RefCell<AttachEpochAllocator>>,
    instance: ViewInstanceId,
    spec: ViewSpec,
    label: &str,
) -> Result<ViewHost, UiError> {
    let mut host = ViewHost::new(instance, attach_epochs.borrow_mut().mint());
    host.mount(spec).map_err(|e| UiError::Internal {
        message: format!("{label} spec invalid: {e}"),
    })?;
    Ok(host)
}

/// Mounts the Action Palette onto a fresh [`ViewHost`] under the
/// adopted workspace view id — same attach-epoch contract as
/// [`crate::screens::mount_vcs_history_view`].
///
/// # Errors
/// [`UiError::Internal`] when the spec fails validation.
pub fn mount_action_palette_view(
    attach_epochs: &Rc<RefCell<AttachEpochAllocator>>,
    instance: ViewInstanceId,
    state: &ActionPaletteScreenState,
) -> Result<ViewHost, UiError> {
    mount_automation_view(
        attach_epochs,
        instance,
        action_palette_screen_spec(state),
        "action palette",
    )
}

/// Mounts the Macro Browser — same contract as
/// [`mount_action_palette_view`].
///
/// # Errors
/// [`UiError::Internal`] when the spec fails validation.
pub fn mount_macro_browser_view(
    attach_epochs: &Rc<RefCell<AttachEpochAllocator>>,
    instance: ViewInstanceId,
    state: &MacroBrowserScreenState,
) -> Result<ViewHost, UiError> {
    mount_automation_view(
        attach_epochs,
        instance,
        macro_browser_screen_spec(state),
        "macro browser",
    )
}

/// Mounts the Workflow Editor v1 — same contract as
/// [`mount_action_palette_view`].
///
/// # Errors
/// [`UiError::Internal`] when the spec fails validation.
pub fn mount_workflow_editor_view(
    attach_epochs: &Rc<RefCell<AttachEpochAllocator>>,
    instance: ViewInstanceId,
    state: &WorkflowEditorScreenState,
) -> Result<ViewHost, UiError> {
    mount_automation_view(
        attach_epochs,
        instance,
        workflow_editor_screen_spec(state),
        "workflow editor",
    )
}

/// Mounts the Run Monitor — same contract as
/// [`mount_action_palette_view`].
///
/// # Errors
/// [`UiError::Internal`] when the spec fails validation.
pub fn mount_run_monitor_view(
    attach_epochs: &Rc<RefCell<AttachEpochAllocator>>,
    instance: ViewInstanceId,
    state: &RunMonitorScreenState,
) -> Result<ViewHost, UiError> {
    mount_automation_view(
        attach_epochs,
        instance,
        run_monitor_screen_spec(state),
        "run monitor",
    )
}

// ── intent decode ─────────────────────────────────────────────────────

/// A decoded automation-surface command.
#[derive(Clone, Debug, PartialEq)]
pub enum AutomationCommand {
    /// View-local mutation — updates `AutomationViewState` only; never
    /// touches project/grant/job truth.
    ViewLocal(AutomationViewAction),
    /// Domain request — lowers to gateway wire request(s) via
    /// [`synvelis_workspace::automation_requests`].
    Request(AutomationViewAction),
    /// Cancel the run currently selected in the monitor's view state —
    /// the composition layer resolves the operation id.
    CancelSelectedRun,
}

fn params_usize(params: &Value, key: &str) -> Option<usize> {
    params
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|v| usize::try_from(v).ok())
}

fn params_str(params: &Value, key: &str) -> Option<String> {
    params.get(key).and_then(Value::as_str).map(str::to_string)
}

/// Strict `params.target` decode: absent/null → `Ok(None)`; a
/// present-but-malformed target → `Err(())` (the whole command rejects
/// — a bad target is never silently dropped into "no target").
fn params_target(params: &Value) -> Result<Option<ExplicitTarget>, ()> {
    match params.get("target") {
        None | Some(Value::Null) => Ok(None),
        Some(v) => serde_json::from_value(v.clone()).map(Some).map_err(|_| ()),
    }
}

fn params_args(params: &Value) -> Value {
    params
        .get("args")
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}))
}

/// Decodes one view intent `(target, action, params)` into an
/// [`AutomationCommand`]. Returns `None` for unknown targets/actions
/// and for malformed params — the caller answers those with a
/// rejected command rather than guessing.
///
/// Table selection arrives as `select`/`{"row": n}` and tab selection
/// as `tab-select`/`{"index": n}` (the `ruviform-ui` contract); the
/// palette table's row index maps to `PaletteSelect` while category
/// tabs map to `PaletteCategory`.
#[must_use]
pub fn decode_automation_command(
    target: &str,
    action: &str,
    params: &Value,
) -> Option<AutomationCommand> {
    match (target, action) {
        (AUTX_PALETTE_TARGET, "select") => Some(AutomationCommand::ViewLocal(
            AutomationViewAction::PaletteSelect {
                row: params_usize(params, "row")?,
            },
        )),
        (AUTX_PALETTE_TARGET, "category") => Some(AutomationCommand::ViewLocal(
            AutomationViewAction::PaletteCategory {
                index: params_usize(params, "index")?,
            },
        )),
        (AUTX_MACRO_TARGET, "select") => Some(AutomationCommand::ViewLocal(
            AutomationViewAction::MacroSelect {
                index: params_usize(params, "row")?,
            },
        )),
        (AUTX_MACRO_TARGET, "preview") => Some(AutomationCommand::Request(
            AutomationViewAction::MacroPreview {
                preset: params_str(params, "preset")?,
            },
        )),
        (AUTX_MACRO_TARGET, "apply") => Some(AutomationCommand::Request(
            AutomationViewAction::MacroApply {
                preset: params_str(params, "preset")?,
                idempotency_key: params_str(params, "idempotency_key"),
            },
        )),
        (AUTX_WORKFLOW_TARGET, "select") => Some(AutomationCommand::ViewLocal(
            AutomationViewAction::WorkflowSelect {
                index: params_usize(params, "row")?,
            },
        )),
        (AUTX_RUN_TARGET, "select") => Some(AutomationCommand::ViewLocal(
            AutomationViewAction::RunSelect {
                index: params_usize(params, "row")?,
            },
        )),
        (AUTX_RUN_TARGET, "cancel") => Some(AutomationCommand::CancelSelectedRun),
        (AUTX_ACTION_TARGET, "capture") => Some(AutomationCommand::ViewLocal(
            AutomationViewAction::CaptureTarget {
                capability: params_str(params, "capability")?,
                target: params_target(params).ok()?,
            },
        )),
        (AUTX_ACTION_TARGET, "preview") => {
            Some(AutomationCommand::Request(AutomationViewAction::Preview {
                capability: params_str(params, "capability")?,
                target: params_target(params).ok()?,
                args: params_args(params),
            }))
        }
        (AUTX_ACTION_TARGET, "submit") => {
            Some(AutomationCommand::Request(AutomationViewAction::Submit {
                capability: params_str(params, "capability")?,
                target: params_target(params).ok()?,
                args: params_args(params),
                idempotency_key: params_str(params, "idempotency_key"),
            }))
        }
        _ => None,
    }
}
