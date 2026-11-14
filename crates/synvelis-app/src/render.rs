//! RCP-06 (synvelis#295) — driver-neutral `ViewSpec` / state
//! projection / binding decode for the render surfaces: Render Status
//! (five-axis node presentation + policy + job monitor), Branch
//! Difference Explorer, and Render Request preview.
//!
//! These builders project the [`synvelis_workspace::render_ui`] read
//! models — themselves pure projections of the RCP-01..RCP-04
//! authority types — into `ruviform_ui::view` specs. No GPUI/OS type
//! appears here, and no view state becomes project/render/policy/job
//! truth: the only mutable surface state is the per-view
//! [`RenderViewState`] carried in `synvelis-workspace`'s `ViewState`.
//!
//! Interactive contract (AUTX-07 pattern):
//!
//! - Every intent lands on one of the `RCP_*_TARGET` logical targets
//!   and decodes through [`decode_render_command`] — the only place
//!   view intents become [`RenderCommand`]s.
//! - Non-executable action candidates never get buttons — the spec
//!   itself is fail-closed, not just the handler. Disabled reasons
//!   stay visible as text (shared [`RenderReason::code`] vocabulary).
//! - Domain requests lower to authority only through
//!   [`synvelis_workspace::render_request_report`] — the composition
//!   layer supplies the pinned selection/grant; this module never
//!   fabricates them.
//!
//! **Dependency direction:** `ruviform_ui::view` +
//! `synvelis_workspace` types only — no GPUI/OS/DOM, no
//! `synvelis-render`/`synvelis-gateway` types (authority vocabulary
//! reaches the spec as display strings). **RT contract:** non-RT
//! only.

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
    BoundaryCandidateView, BoundaryDiffRow, BoundarySideView, BranchDiffView, RenderCommand,
    RenderJobView, RenderNodePresentation, RenderPolicyView, RenderReason, RenderRequestPreview,
    RenderSelectionView, RenderViewAction, RenderViewState, SemanticChange, SemanticDiffRow,
    SnapshotPinView, apply_render_view_action,
};

use crate::screens::{key, panel};

/// Logical target of the render-status surface (node table is
/// read-only; job selection + job-control + policy draft intents).
pub const RCP_RENDER_TARGET: &str = "rcp.render.status";
/// Logical target of the Branch Difference Explorer.
pub const RCP_BRANCH_TARGET: &str = "rcp.branch.diff";
/// Logical target of the Render Request preview (action candidates).
pub const RCP_REQUEST_TARGET: &str = "rcp.render.request";

// ── screen states ────────────────────────────────────────────────────

/// Render Status surface state — five-axis node rows, selection
/// verdict, policy projection, and job monitor rows. All fields are
/// workspace read models; the app holds no authority.
#[derive(Clone, Debug, Default)]
pub struct RenderStatusScreenState {
    /// Per-view render state (selections + policy draft).
    pub view: RenderViewState,
    /// Execution-selection projection, when a selection ran.
    pub selection: Option<RenderSelectionView>,
    /// Render policy projection, when policy resolved.
    pub policy: Option<RenderPolicyView>,
    /// Per-node five-axis rows (Signal Flow/Inspector/Mixer overlay
    /// input projected for display).
    pub nodes: Vec<RenderNodePresentation>,
    /// Render job monitor rows.
    pub jobs: Vec<RenderJobView>,
}

/// Branch Difference Explorer surface state.
#[derive(Clone, Debug, Default)]
pub struct BranchDiffScreenState {
    /// Per-view render state (boundary selection).
    pub view: RenderViewState,
    /// The diff projection, when both sides pinned.
    pub diff: Option<BranchDiffView>,
}

/// Render Request preview surface state.
#[derive(Clone, Debug, Default)]
pub struct RenderRequestScreenState {
    /// Per-view render state (boundary selection).
    pub view: RenderViewState,
    /// The request preview projection.
    pub preview: Option<RenderRequestPreview>,
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

fn reason_text(reasons: &[RenderReason]) -> String {
    if reasons.is_empty() {
        return "—".to_string();
    }
    reasons
        .iter()
        .map(|r| format!("{} ({})", r.code(), r.label()))
        .collect::<Vec<_>>()
        .join("; ")
}

fn axis_row(node: &RenderNodePresentation) -> Vec<Option<CellValue>> {
    vec![
        text_cell(node.node.0.to_string()),
        text_cell(node.editability.label()),
        text_cell(node.executability.label()),
        text_cell(node.render_validity.label()),
        text_cell(node.playback_quality.label()),
        text_cell(node.runtime_state.label()),
        text_cell(node.substitution.as_ref().map_or_else(
            || "—".to_string(),
            |s| {
                format!(
                    "{}{}",
                    s.artifact_id,
                    if s.preview_class { " (preview)" } else { "" }
                )
            },
        )),
        text_cell(reason_text(&node.reasons)),
    ]
    .into_iter()
    .map(Some)
    .collect()
}

const NODE_COLUMNS: [&str; 8] = [
    "Node",
    "Editability",
    "Executability",
    "Render validity",
    "Playback quality",
    "Runtime",
    "Substitution",
    "Reasons",
];

fn nodes_table(nodes: &[RenderNodePresentation]) -> ComponentSpec {
    ComponentSpec::Table(TableSpec {
        key: key("rcp.render.nodes"),
        columns: NODE_COLUMNS.iter().map(ToString::to_string).collect(),
        rows: nodes.iter().map(axis_row).collect(),
        selection: None,
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Render node axes".to_string()),
            ..A11ySpec::default()
        },
    })
}

fn jobs_table(jobs: &[RenderJobView]) -> ComponentSpec {
    ComponentSpec::Table(TableSpec {
        key: key("rcp.render.jobs"),
        columns: [
            "Job",
            "State",
            "Attempt",
            "Pending",
            "Cancel",
            "Completeness",
            "Failure",
        ]
        .iter()
        .map(ToString::to_string)
        .collect(),
        rows: jobs
            .iter()
            .enumerate()
            .map(|(i, j)| {
                vec![
                    text_cell(format!("{i} — {}", j.key_summary)),
                    text_cell(j.state_label),
                    text_cell(j.attempt.to_string()),
                    text_cell(
                        j.pending_reason
                            .map_or_else(|| "—".to_string(), |r| r.code().to_string()),
                    ),
                    text_cell(if j.cancel_requested {
                        "requested".to_string()
                    } else {
                        "—".to_string()
                    }),
                    text_cell(j.completeness.as_ref().map_or_else(
                        || "—".to_string(),
                        |c| {
                            format!(
                                "{}{}",
                                c.verdict,
                                if c.gaps.is_empty() {
                                    String::new()
                                } else {
                                    format!(" ({} gaps)", c.gaps.len())
                                }
                            )
                        },
                    )),
                    text_cell(j.failure.as_ref().map_or_else(
                        || "—".to_string(),
                        |f| format!("{}: {}", f.reason.code(), f.detail),
                    )),
                ]
                .into_iter()
                .map(Some)
                .collect()
            })
            .collect(),
        selection: Some(TableSelectionSpec {
            target: RCP_RENDER_TARGET.to_string(),
        }),
        visual: ComponentVisualState::default(),
        a11y: A11ySpec {
            label: Some("Render jobs".to_string()),
            ..A11ySpec::default()
        },
    })
}

fn selection_text(selection: &RenderSelectionView) -> String {
    format!(
        "intent {} — verdict {} — non-equivalent {} — live {} / subst {} / skipped {} / unresolved {} / degraded {} — pins rev {} struct {:x} cap {} render-idx {} sel {:x}",
        selection.intent,
        selection.verdict,
        selection.non_equivalent,
        selection.live_regions,
        selection.substitutions,
        selection.skipped,
        selection.unresolved,
        selection.degraded,
        selection.pins.project_revision.0,
        selection.pins.structure_digest,
        selection.pins.capability_generation,
        selection.pins.render_index_generation,
        selection.pins.selection_digest,
    )
}

fn policy_text(policy: &RenderPolicyView) -> String {
    let triggers = policy
        .triggers
        .iter()
        .map(|t| {
            format!(
                "{}:{}{}",
                t.label,
                if t.enabled { "on" } else { "off" },
                t.blocked_reason
                    .map(|r| format!(" ({})", r.code()))
                    .unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "scope {} — completion {} — quality {} — budget [{}] — retention [{}] — resolution {} — commit {} — recipe-invariant {}",
        policy.scope,
        policy.completion,
        policy.quality_profile,
        policy.resource_budget,
        policy.retention,
        policy.resolution_digest,
        match &policy.commit {
            synvelis_workspace::CommitView::Supported => "supported".to_string(),
            synvelis_workspace::CommitView::Unsupported { reason, detail } => {
                format!("{} ({})", reason.code(), detail)
            }
            synvelis_workspace::CommitView::Unknown => "unknown".to_string(),
        },
        policy.recipe_invariant,
    ) + &format!(" — triggers: {triggers}")
}

fn provenance_text(policy: &RenderPolicyView) -> String {
    policy
        .provenance
        .iter()
        .map(|p| format!("{}={}", p.field, p.layer))
        .collect::<Vec<_>>()
        .join(", ")
}

fn job_action_buttons(job: &RenderJobView) -> Vec<ComponentSpec> {
    job.actions
        .iter()
        .filter(|a| a.executable)
        .map(|a| {
            let op = match a.kind {
                synvelis_workspace::RenderActionKind::Cancel => "cancel",
                synvelis_workspace::RenderActionKind::Retry => "retry",
                synvelis_workspace::RenderActionKind::Attach => "attach",
                _ => "verify",
            };
            action_button(
                &format!("rcp.render.job.{op}"),
                a.label,
                RCP_RENDER_TARGET,
                "job-control",
                Some(serde_json::json!({ "op": op })),
            )
        })
        .collect()
}

fn job_disabled_reasons(job: &RenderJobView) -> String {
    job.actions
        .iter()
        .filter(|a| !a.executable)
        .filter_map(|a| a.disabled_reason.map(|r| format!("{}={}", a.id, r.code())))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The Render Status `ViewSpec` — five-axis node table (non-color
/// semantics via stable labels + reason codes), selection verdict +
/// pins, policy projection with trigger/provider/provenance rows,
/// and the job monitor with idempotent cancel/retry/attach controls
/// gated on the projection's own executability.
#[must_use]
pub fn render_status_screen_spec(state: &RenderStatusScreenState) -> ViewSpec {
    let mut children = vec![label(
        "rcp.render.legend",
        "Axis legend",
        "axes: editability=edit-possible|edit-blocked · executability=live|blocked · \
             validity=render-valid|render-preview|render-stale|render-missing|render-corrupt|\
             render-unverified|render-unsupported|render-denied|render-unknown · \
             quality=quality-exact-artifact|quality-verified-tolerance|quality-preview|\
             quality-unavailable · runtime=runtime-configured|runtime-prepared|\
             runtime-published|runtime-active|runtime-last-good-mismatch"
            .to_string(),
    )];
    if let Some(selection) = &state.selection {
        children.push(label(
            "rcp.render.selection",
            "Selection verdict",
            selection_text(selection),
        ));
        if !selection.unresolved_reasons.is_empty() {
            children.push(label(
                "rcp.render.unresolved",
                "Unresolved reasons",
                reason_text(&selection.unresolved_reasons),
            ));
        }
    }
    if !state.nodes.is_empty() {
        children.push(nodes_table(&state.nodes));
    }
    if let Some(policy) = &state.policy {
        children.push(label(
            "rcp.render.policy",
            "Render policy",
            policy_text(policy),
        ));
        children.push(label(
            "rcp.render.provenance",
            "Policy provenance",
            provenance_text(policy),
        ));
        if state.view.draft_triggers.is_some()
            || state.view.draft_completion.is_some()
            || state.view.draft_scope.is_some()
        {
            children.push(label(
                "rcp.render.draft",
                "Pending policy intent",
                "draft policy edits pending — trigger-only edits never invalidate recipe keys"
                    .to_string(),
            ));
        }
    }
    if !state.jobs.is_empty() {
        children.push(jobs_table(&state.jobs));
        // Job-control buttons exist only for the *selected* job and
        // only for executable candidates — fail-closed by spec.
        if let Some(job) = state.view.selected_job.and_then(|i| state.jobs.get(i)) {
            let buttons = job_action_buttons(job);
            if !buttons.is_empty() {
                children.push(panel("rcp.render.job.buttons", None, buttons));
            }
            let disabled = job_disabled_reasons(job);
            if !disabled.is_empty() {
                children.push(label(
                    "rcp.render.job.disabled",
                    "Disabled job actions",
                    disabled,
                ));
            }
        }
    }
    if state.selection.is_none() && state.nodes.is_empty() && state.policy.is_none() {
        children.push(label(
            "rcp.render.empty",
            "Render status empty",
            "no render data — the authority layer has not projected any".to_string(),
        ));
    }
    ViewSpec {
        root: panel("rcp.render.root", Some("Render Status"), children),
    }
}

// ── Branch Difference Explorer ────────────────────────────────────────

fn boundary_row(row: &BoundaryDiffRow) -> Vec<Option<CellValue>> {
    let resolution = |s: &BoundarySideView| {
        format!(
            "{:?}{}",
            s.resolution,
            s.recipe_key
                .as_ref()
                .map(|k| format!(" {k}"))
                .unwrap_or_default(),
        )
    };
    vec![
        text_cell(format!("{} / {}", row.left.boundary, row.right.boundary)),
        text_cell(resolution(&row.left)),
        text_cell(resolution(&row.right)),
        text_cell(row.coverage.clone()),
        text_cell(
            row.changed_parameters
                .iter()
                .map(|c| format!("{}: {} -> {}", c.path, c.left, c.right))
                .collect::<Vec<_>>()
                .join("; "),
        ),
        text_cell(row.missing_plugins.join(", ")),
        text_cell(row.preview_markers.join("; ")),
        text_cell(reason_text(
            &row.left
                .reasons
                .iter()
                .chain(row.right.reasons.iter())
                .copied()
                .collect::<Vec<_>>(),
        )),
    ]
    .into_iter()
    .map(Some)
    .collect()
}

fn semantic_row(row: &SemanticDiffRow) -> Vec<Option<CellValue>> {
    vec![
        text_cell(row.path.clone()),
        text_cell(row.left.clone()),
        text_cell(row.right.clone()),
        text_cell(match row.kind {
            SemanticChange::Added => "added",
            SemanticChange::Removed => "removed",
            SemanticChange::Modified => "modified",
            SemanticChange::Unknown => "unknown",
        }),
    ]
    .into_iter()
    .map(Some)
    .collect()
}

fn pin_text(pin: &SnapshotPinView) -> String {
    pin.pin_text()
}

/// The Branch Difference Explorer `ViewSpec` — pinned left/right
/// snapshot pins, per-boundary resolution/recipe-key/coverage rows,
/// changed parameters, missing-plugin lists, semantic diffs, and
/// non-equivalent preview markers. No waveform is ever presented as
/// the current edit result.
#[must_use]
pub fn branch_diff_screen_spec(state: &BranchDiffScreenState) -> ViewSpec {
    let mut children = Vec::new();
    match &state.diff {
        None => children.push(label(
            "rcp.branch.empty",
            "Branch diff empty",
            "no branch diff — both sides must pin snapshots first".to_string(),
        )),
        Some(diff) => {
            children.push(label(
                "rcp.branch.pins",
                "Pinned revisions",
                format!(
                    "left {} — right {} — context {}",
                    pin_text(&diff.left_pin),
                    pin_text(&diff.right_pin),
                    diff.context,
                ),
            ));
            if !diff.warnings.is_empty() {
                children.push(label(
                    "rcp.branch.warnings",
                    "Diff warnings",
                    diff.warnings.join("; "),
                ));
            }
            children.push(ComponentSpec::Table(TableSpec {
                key: key("rcp.branch.boundaries"),
                columns: [
                    "Boundary",
                    "Left",
                    "Right",
                    "Coverage",
                    "Changed",
                    "Missing plugins",
                    "Preview",
                    "Reasons",
                ]
                .iter()
                .map(ToString::to_string)
                .collect(),
                rows: diff.rows.iter().map(boundary_row).collect(),
                selection: Some(TableSelectionSpec {
                    target: RCP_BRANCH_TARGET.to_string(),
                }),
                visual: ComponentVisualState::default(),
                a11y: A11ySpec {
                    label: Some("Branch boundary diff".to_string()),
                    ..A11ySpec::default()
                },
            }));
            children.push(label(
                "rcp.branch.missing",
                "Missing plugins",
                if diff.missing_plugins.is_empty() {
                    "none".to_string()
                } else {
                    diff.missing_plugins.join(", ")
                },
            ));
            if !diff.semantic_diffs.is_empty() {
                children.push(ComponentSpec::Table(TableSpec {
                    key: key("rcp.branch.semantic"),
                    columns: ["Path", "Left", "Right", "Kind"]
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                    rows: diff.semantic_diffs.iter().map(semantic_row).collect(),
                    selection: None,
                    visual: ComponentVisualState::default(),
                    a11y: A11ySpec {
                        label: Some("Semantic diff".to_string()),
                        ..A11ySpec::default()
                    },
                }));
            }
            if !diff.preview_markers.is_empty() {
                children.push(label(
                    "rcp.branch.preview",
                    "Non-equivalent preview",
                    diff.preview_markers.join("; "),
                ));
            }
        }
    }
    ViewSpec {
        root: panel("rcp.branch.root", Some("Branch Difference"), children),
    }
}

// ── Render Request preview ────────────────────────────────────────────

fn candidate_row(view: &BoundaryCandidateView) -> Vec<Option<CellValue>> {
    vec![
        text_cell(view.index.to_string()),
        text_cell(view.boundary.clone()),
        text_cell(view.interval.clone()),
        text_cell(view.resolution.label()),
        text_cell(view.recipe_key.clone().unwrap_or_else(|| "—".to_string())),
        text_cell(reason_text(&view.reasons)),
    ]
    .into_iter()
    .map(Some)
    .collect()
}

fn request_action_buttons(preview: &RenderRequestPreview) -> Vec<ComponentSpec> {
    preview
        .actions
        .iter()
        .filter(|a| a.executable)
        .map(|a| {
            action_button(
                &format!(
                    "rcp.request.{}",
                    a.id.rsplit('.').next().unwrap_or("action")
                ),
                a.label,
                RCP_REQUEST_TARGET,
                "request",
                Some(serde_json::json!({ "action": a.id })),
            )
        })
        .collect()
}

fn request_disabled_actions(preview: &RenderRequestPreview) -> String {
    preview
        .actions
        .iter()
        .filter(|a| !a.executable)
        .filter_map(|a| a.disabled_reason.map(|r| format!("{}={}", a.id, r.code())))
        .collect::<Vec<_>>()
        .join(", ")
}

/// The Render Request preview `ViewSpec` — pinned snapshot, boundary
/// candidates with resolution/recipe-key/reason rows, coverage,
/// expected size, missing plugins, owner-machine indicator,
/// invalidation reason, and executable-gated action candidates.
/// Previewing never submits work.
#[must_use]
pub fn render_request_screen_spec(state: &RenderRequestScreenState) -> ViewSpec {
    let mut children = Vec::new();
    match &state.preview {
        None => children.push(label(
            "rcp.request.empty",
            "Render request empty",
            "no render request preview — pin a snapshot and resolve demands first".to_string(),
        )),
        Some(preview) => {
            children.push(label(
                "rcp.request.summary",
                "Request summary",
                format!(
                    "pin {} — purpose {} — coverage {} — expected {} — owner-machine {} — invalidation {}",
                    preview.pin.pin_text(),
                    preview.purpose,
                    preview.coverage,
                    preview
                        .expected_bytes
                        .map_or_else(|| "unknown".to_string(), |b| format!("{b}B")),
                    preview.requires_owner_machine,
                    preview
                        .invalidation
                        .map_or_else(|| "none".to_string(), |r| r.code().to_string()),
                ),
            ));
            if !preview.missing_plugins.is_empty() {
                children.push(label(
                    "rcp.request.missing",
                    "Missing plugins",
                    preview.missing_plugins.join(", "),
                ));
            }
            children.push(ComponentSpec::Table(TableSpec {
                key: key("rcp.request.boundaries"),
                columns: [
                    "#",
                    "Boundary",
                    "Interval",
                    "Resolution",
                    "Recipe key",
                    "Reasons",
                ]
                .iter()
                .map(ToString::to_string)
                .collect(),
                rows: preview.boundaries.iter().map(candidate_row).collect(),
                selection: Some(TableSelectionSpec {
                    target: RCP_REQUEST_TARGET.to_string(),
                }),
                visual: ComponentVisualState::default(),
                a11y: A11ySpec {
                    label: Some("Render request boundaries".to_string()),
                    ..A11ySpec::default()
                },
            }));
            let buttons = request_action_buttons(preview);
            if !buttons.is_empty() {
                children.push(panel("rcp.request.buttons", None, buttons));
            }
            let disabled = request_disabled_actions(preview);
            if !disabled.is_empty() {
                children.push(label("rcp.request.disabled", "Disabled actions", disabled));
            }
        }
    }
    ViewSpec {
        root: panel("rcp.request.root", Some("Render Request"), children),
    }
}

// ── mount ─────────────────────────────────────────────────────────────

fn mount_render_view(
    attach_epochs: &Rc<RefCell<AttachEpochAllocator>>,
    instance: ViewInstanceId,
    spec: ViewSpec,
    label_name: &str,
) -> Result<ViewHost, UiError> {
    let mut host = ViewHost::new(instance, attach_epochs.borrow_mut().mint());
    host.mount(spec).map_err(|e| UiError::Internal {
        message: format!("{label_name} spec invalid: {e}"),
    })?;
    Ok(host)
}

/// Mounts the Render Status surface — same attach-epoch contract as
/// `mount_action_palette_view`.
///
/// # Errors
/// [`UiError::Internal`] when the spec fails validation.
pub fn mount_render_status_view(
    attach_epochs: &Rc<RefCell<AttachEpochAllocator>>,
    instance: ViewInstanceId,
    state: &RenderStatusScreenState,
) -> Result<ViewHost, UiError> {
    mount_render_view(
        attach_epochs,
        instance,
        render_status_screen_spec(state),
        "render status",
    )
}

/// Mounts the Branch Difference Explorer — same contract.
///
/// # Errors
/// [`UiError::Internal`] when the spec fails validation.
pub fn mount_branch_diff_view(
    attach_epochs: &Rc<RefCell<AttachEpochAllocator>>,
    instance: ViewInstanceId,
    state: &BranchDiffScreenState,
) -> Result<ViewHost, UiError> {
    mount_render_view(
        attach_epochs,
        instance,
        branch_diff_screen_spec(state),
        "branch diff",
    )
}

/// Mounts the Render Request preview — same contract.
///
/// # Errors
/// [`UiError::Internal`] when the spec fails validation.
pub fn mount_render_request_view(
    attach_epochs: &Rc<RefCell<AttachEpochAllocator>>,
    instance: ViewInstanceId,
    state: &RenderRequestScreenState,
) -> Result<ViewHost, UiError> {
    mount_render_view(
        attach_epochs,
        instance,
        render_request_screen_spec(state),
        "render request",
    )
}

// ── intent decode ─────────────────────────────────────────────────────

fn params_usize(params: &Value, key_name: &str) -> Option<usize> {
    params
        .get(key_name)
        .and_then(Value::as_u64)
        .and_then(|v| usize::try_from(v).ok())
}

fn params_str(params: &Value, key_name: &str) -> Option<String> {
    params
        .get(key_name)
        .and_then(Value::as_str)
        .map(str::to_string)
}

fn params_bool(params: &Value, key_name: &str) -> Option<bool> {
    params.get(key_name).and_then(Value::as_bool)
}

fn params_trigger(params: &Value) -> Option<synvelis_workspace::RenderTrigger> {
    params
        .get("trigger")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

fn params_completion(params: &Value) -> Option<synvelis_workspace::CompletionRequirement> {
    params
        .get("completion")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

fn params_scope(params: &Value) -> Option<synvelis_workspace::RenderScope> {
    params
        .get("scope")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
}

fn params_job_op(params: &Value) -> Option<synvelis_workspace::JobControlOp> {
    match params_str(params, "op")?.as_str() {
        "cancel" => Some(synvelis_workspace::JobControlOp::Cancel),
        "retry" => Some(synvelis_workspace::JobControlOp::Retry),
        "attach" => Some(synvelis_workspace::JobControlOp::Attach),
        _ => None,
    }
}

/// Decodes one render-surface intent `(target, action, params)` into a
/// [`RenderCommand`]. Returns `None` for unknown targets/actions and
/// malformed params — the caller answers those with a rejected
/// command rather than guessing.
///
/// Table row selection arrives as `select`/`{"row": n}` (the
/// `ruviform-ui` contract): on `RCP_RENDER_TARGET` it selects a job
/// row; on `RCP_BRANCH_TARGET`/`RCP_REQUEST_TARGET` it selects a
/// boundary row.
#[must_use]
pub fn decode_render_command(target: &str, action: &str, params: &Value) -> Option<RenderCommand> {
    match (target, action) {
        (RCP_RENDER_TARGET, "select") => {
            Some(RenderCommand::ViewLocal(RenderViewAction::SelectJob {
                index: params_usize(params, "row")?,
            }))
        }
        (RCP_RENDER_TARGET, "job-control") => Some(RenderCommand::JobControl {
            op: params_job_op(params)?,
        }),
        (RCP_RENDER_TARGET, "toggle-trigger") => {
            Some(RenderCommand::ViewLocal(RenderViewAction::ToggleTrigger {
                trigger: params_trigger(params)?,
                enabled: params_bool(params, "enabled")?,
            }))
        }
        (RCP_RENDER_TARGET, "set-completion") => {
            Some(RenderCommand::ViewLocal(RenderViewAction::SetCompletion {
                completion: params_completion(params)?,
            }))
        }
        (RCP_RENDER_TARGET, "set-scope") => {
            Some(RenderCommand::ViewLocal(RenderViewAction::SetScope {
                scope: params_scope(params)?,
            }))
        }
        (RCP_RENDER_TARGET, "clear-draft") => {
            Some(RenderCommand::ViewLocal(RenderViewAction::ClearDraft))
        }
        (RCP_BRANCH_TARGET | RCP_REQUEST_TARGET, "select") => {
            Some(RenderCommand::ViewLocal(RenderViewAction::SelectBoundary {
                index: params_usize(params, "row")?,
            }))
        }
        (RCP_REQUEST_TARGET, "request") => Some(RenderCommand::Request {
            action: params_str(params, "action")?,
        }),
        _ => None,
    }
}

/// Applies a decoded view-local command to `state` — the composition
/// layer's view-state update path. Domain requests/job controls are
/// *not* applied here; they route to authority ports.
pub fn apply_render_view_command(state: &mut RenderViewState, command: &RenderCommand) {
    if let RenderCommand::ViewLocal(action) = command {
        apply_render_view_action(state, *action);
    }
}
