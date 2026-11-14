//! AUTX-07 (synvelis#266) headless slice — driver-neutral read models
//! and the view-action → capability binding for the Action Palette,
//! Macro Browser, Workflow Editor v1, and Run Monitor surfaces.
//!
//! Everything here is a *projection* of Gateway / Automation authority:
//! the capability catalog, grant verdicts, macro presets, workflow
//! plans, and job/run observations all arrive from their owners. This
//! layer never mints a second capability registry, grant store, job
//! book, workflow engine, or project truth, and it never resolves an
//! implicit target — explicit targets are captured verbatim and frozen
//! at confirm time. GPUI/OS handles never appear in these types.
//!
//! **RT contract:** non-RT only — driven on the UI thread.

use std::fmt;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use synvelis_gateway::{
    Availability, CancelLink, CapabilityDescriptor, CapabilityRef, EffectKind, JobPhase,
    OperationDomain, PermissionKind, RequestId, ResourceKind, SchemaRef, TargetContract,
    WireConsistency, WireRequest, WireTarget,
};
use synvelis_project::ProjectRevision;

// ── errors ────────────────────────────────────────────────────────────

/// Failure of an automation-surface projection or binding step.
#[derive(Debug, Clone, PartialEq)]
pub enum AutomationUiError {
    /// The request could not be formed (bad shape, missing field).
    InvalidRequest {
        /// Human-readable reason.
        detail: String,
    },
    /// The referenced capability is not in the supplied catalog.
    UnknownCapability {
        /// The `id@version` text that failed to resolve.
        capability: String,
    },
    /// The capability's target contract rejected the explicit target.
    TargetMismatch {
        /// Human-readable reason.
        detail: String,
    },
    /// The capability is not executable (availability/grant).
    NotExecutable {
        /// Human-readable reason.
        detail: String,
    },
    /// The incoming projection is older than the applied one — stale.
    Stale {
        /// The applied sequence.
        applied: u64,
        /// The incoming sequence.
        incoming: u64,
    },
}

impl fmt::Display for AutomationUiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequest { detail } => write!(f, "invalid automation request: {detail}"),
            Self::UnknownCapability { capability } => {
                write!(f, "unknown capability {capability}")
            }
            Self::TargetMismatch { detail } => write!(f, "target mismatch: {detail}"),
            Self::NotExecutable { detail } => write!(f, "not executable: {detail}"),
            Self::Stale { applied, incoming } => {
                write!(
                    f,
                    "stale projection: applied seq {applied}, incoming {incoming}"
                )
            }
        }
    }
}

impl std::error::Error for AutomationUiError {}

fn invalid(detail: impl Into<String>) -> AutomationUiError {
    AutomationUiError::InvalidRequest {
        detail: detail.into(),
    }
}

// ── shared display vocabulary ─────────────────────────────────────────

/// The authority-reported grant verdict for one capability.
///
/// The GUI never decides authorization — it displays the verdict the
/// Gateway/policy authority produced. `Unknown` is a first-class state:
/// absent information is never rendered as granted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum GrantState {
    /// The caller holds every permission the capability requires.
    Satisfied,
    /// The authority reports specific required permissions missing.
    Missing {
        /// Wire names of the missing permissions.
        permissions: Vec<String>,
    },
    /// No authority verdict is available for this capability.
    Unknown,
}

impl GrantState {
    /// Whether the verdict permits display as executable.
    #[must_use]
    pub const fn satisfied(&self) -> bool {
        matches!(self, Self::Satisfied)
    }

    /// Short display label for the grant column.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::Satisfied => "granted".to_string(),
            Self::Missing { permissions } => format!("missing {}", permissions.join(",")),
            Self::Unknown => "unknown".to_string(),
        }
    }
}

/// An explicit target the user confirmed — the only target form the
/// wire accepts (GTC-002: never implicit focus/selection resolution).
///
/// This is the serde-able mirror of [`WireTarget`]; conversion happens
/// in [`ExplicitTarget::to_wire`].
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "shape", rename_all = "kebab-case")]
pub enum ExplicitTarget {
    /// One existing resource of `kind`.
    Resource {
        /// Wire kind tag (e.g. `"source"`).
        kind: String,
        /// Tagged stable id.
        id: String,
    },
    /// A non-empty set of existing resources of one kind.
    ResourceSet {
        /// Wire kind tag.
        kind: String,
        /// Tagged stable ids.
        ids: Vec<String>,
    },
    /// Create a new resource under an explicit parent.
    Create {
        /// Wire kind tag of the parent.
        parent_kind: String,
        /// Tagged stable id of the parent.
        parent: String,
        /// Wire kind tag of the created resource.
        creates: String,
    },
}

impl ExplicitTarget {
    /// Converts to the wire form. `ResourceSet` with an empty id list
    /// is rejected — the wire requires non-empty.
    ///
    /// # Errors
    /// [`AutomationUiError::InvalidRequest`] on an empty id set.
    pub fn to_wire(&self) -> Result<WireTarget, AutomationUiError> {
        Ok(match self {
            Self::Resource { kind, id } => WireTarget::Resource {
                kind: kind.clone(),
                id: id.clone(),
            },
            Self::ResourceSet { kind, ids } => {
                if ids.is_empty() {
                    return Err(invalid("resource-set target requires at least one id"));
                }
                WireTarget::ResourceSet {
                    kind: kind.clone(),
                    ids: ids.clone(),
                }
            }
            Self::Create {
                parent_kind,
                parent,
                creates,
            } => WireTarget::Create {
                parent_kind: parent_kind.clone(),
                parent: parent.clone(),
                creates: creates.clone(),
            },
        })
    }
}

/// A target captured at action-confirm time. Once captured it is
/// frozen: later focus/selection changes in any view must not retarget
/// it (AUTX-07 stable-target contract).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapturedTarget {
    /// Capability id the target was captured for.
    pub capability_id: String,
    /// Capability version (`major.minor`).
    pub capability_version: String,
    /// Tagged project id the target belongs to.
    pub project: String,
    /// The confirmed explicit target, if the contract requires one.
    pub target: Option<ExplicitTarget>,
}

/// The wire kind tag of a [`ResourceKind`] (`"source"`, `"track"`, …).
#[must_use]
pub fn resource_kind_tag(kind: ResourceKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Validates an explicit target against a capability's
/// [`TargetContract`] and freezes it into a [`CapturedTarget`].
///
/// Fail-closed rules:
/// - `TargetContract::None` requires *no* explicit target.
/// - `Resource`/`ResourceSet` require an explicit target of the same
///   shape and declared kind.
/// - `Create` requires the `Create` shape with matching parent/creates
///   kinds.
///
/// Any mismatch is an error — the GUI never guesses a target.
///
/// # Errors
/// [`AutomationUiError::TargetMismatch`] / [`AutomationUiError::InvalidRequest`].
pub fn capture_target(
    descriptor: &CapabilityDescriptor,
    project: &str,
    explicit: Option<ExplicitTarget>,
) -> Result<CapturedTarget, AutomationUiError> {
    let capability = descriptor.capability_ref();
    match &descriptor.target {
        TargetContract::None => {
            if explicit.is_some() {
                return Err(AutomationUiError::TargetMismatch {
                    detail: format!("{capability} takes no target"),
                });
            }
        }
        TargetContract::Resource { kind } => {
            let want = resource_kind_tag(*kind);
            match &explicit {
                Some(ExplicitTarget::Resource { kind: k, .. }) if *k == want => {}
                _ => {
                    return Err(AutomationUiError::TargetMismatch {
                        detail: format!("{capability} requires one {want} resource"),
                    });
                }
            }
        }
        TargetContract::ResourceSet { kind } => {
            let want = resource_kind_tag(*kind);
            match &explicit {
                Some(ExplicitTarget::ResourceSet { kind: k, .. }) if *k == want => {}
                _ => {
                    return Err(AutomationUiError::TargetMismatch {
                        detail: format!("{capability} requires a {want} resource set"),
                    });
                }
            }
        }
        TargetContract::Create { parent, creates } => {
            let want_parent = resource_kind_tag(*parent);
            let want_creates = resource_kind_tag(*creates);
            match &explicit {
                Some(ExplicitTarget::Create {
                    parent_kind,
                    creates: c,
                    ..
                }) if *parent_kind == want_parent && *c == want_creates => {}
                _ => {
                    return Err(AutomationUiError::TargetMismatch {
                        detail: format!(
                            "{capability} requires create under {want_parent} of {want_creates}"
                        ),
                    });
                }
            }
        }
    }
    // Payload sanity at the freeze boundary: the wire rejects empty
    // id sets/strings later, so a target that cannot lower must never
    // be captured into view state.
    match &explicit {
        Some(ExplicitTarget::Resource { id, .. }) if id.is_empty() => {
            return Err(invalid("resource target id must not be empty"));
        }
        Some(ExplicitTarget::ResourceSet { ids, .. })
            if ids.is_empty() || ids.iter().any(String::is_empty) =>
        {
            return Err(invalid("resource-set target requires non-empty ids"));
        }
        Some(ExplicitTarget::Create { parent, .. }) if parent.is_empty() => {
            return Err(invalid("create target parent must not be empty"));
        }
        _ => {}
    }
    Ok(CapturedTarget {
        capability_id: capability.id.to_string(),
        capability_version: capability.version.to_string(),
        project: project.to_string(),
        target: explicit,
    })
}

// ── Action Palette ────────────────────────────────────────────────────

/// One catalog row — a capability projected for display. Every field
/// is authority truth verbatim; `executable`/`blocked` are the
/// fail-closed display verdict, never an authorization decision.
#[derive(Clone, Debug)]
pub struct ActionRow {
    /// Capability id.
    pub id: String,
    /// Capability version (`major.minor`).
    pub version: String,
    /// Operation domain.
    pub domain: OperationDomain,
    /// Registration-visible availability (verbatim).
    pub availability: Availability,
    /// Required target shape (verbatim).
    pub target: TargetContract,
    /// Declared effect upper bound.
    pub effects: Vec<EffectKind>,
    /// Input schema reference (name + version).
    pub input_schema: SchemaRef,
    /// Output schema reference.
    pub output_schema: SchemaRef,
    /// Permissions the caller must hold (wire names).
    pub permissions: Vec<String>,
    /// Authority-reported grant verdict.
    pub grant: GrantState,
    /// Fail-closed display verdict: `true` only when the capability is
    /// `Available` *and* the grant verdict is `Satisfied`.
    pub executable: bool,
    /// Why the row is not executable (`None` when executable).
    pub blocked: Option<String>,
}

impl ActionRow {
    /// `"id@version"` — the palette's stable row identity.
    #[must_use]
    pub fn capability_label(&self) -> String {
        format!("{}@{}", self.id, self.version)
    }

    /// `"name@version"` schema label.
    #[must_use]
    pub fn schema_label(schema: &SchemaRef) -> String {
        format!("{}@{}", schema.name, schema.version)
    }
}

/// Projects one descriptor into a display row.
///
/// `grant` is the authority verdict for this capability — the
/// projection does not evaluate grants itself. `executable` stays
/// `false` for `Disabled`/`Planned`/`Retired`, for `Missing`/`Unknown`
/// grants, and for any combination — unavailable or unauthorized
/// actions never look runnable.
#[must_use]
pub fn project_action_row(descriptor: &CapabilityDescriptor, grant: GrantState) -> ActionRow {
    let capability = descriptor.capability_ref();
    let executable =
        matches!(descriptor.availability, Availability::Available) && grant.satisfied();
    let blocked = if executable {
        None
    } else {
        Some(blocked_reason(&descriptor.availability, &grant))
    };
    ActionRow {
        id: capability.id.to_string(),
        version: capability.version.to_string(),
        domain: descriptor.domain,
        availability: descriptor.availability.clone(),
        target: descriptor.target,
        effects: descriptor.effect_bound.iter().collect(),
        input_schema: descriptor.input_schema.clone(),
        output_schema: descriptor.output_schema.clone(),
        permissions: descriptor
            .required_permissions
            .iter()
            .map(permission_tag)
            .collect(),
        grant,
        executable,
        blocked,
    }
}

fn blocked_reason(availability: &Availability, grant: &GrantState) -> String {
    match availability {
        Availability::Available => match grant {
            GrantState::Satisfied => String::new(),
            GrantState::Missing { permissions } => {
                format!("grant missing: {}", permissions.join(","))
            }
            GrantState::Unknown => "grant unknown".to_string(),
        },
        other => format!("unavailable: {}", other.name()),
    }
}

/// Wire name of a [`PermissionKind`] (kebab-case, matching the gateway
/// serde vocabulary).
#[must_use]
pub fn permission_tag(kind: PermissionKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Wire name of an [`EffectKind`] (kebab-case).
#[must_use]
pub fn effect_tag(kind: EffectKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// One-line display summary of a [`TargetContract`].
#[must_use]
pub fn target_contract_summary(contract: &TargetContract) -> String {
    match contract {
        TargetContract::None => "none".to_string(),
        TargetContract::Resource { kind } => format!("resource:{}", resource_kind_tag(*kind)),
        TargetContract::ResourceSet { kind } => {
            format!("resource-set:{}", resource_kind_tag(*kind))
        }
        TargetContract::Create { parent, creates } => format!(
            "create:{}→{}",
            resource_kind_tag(*parent),
            resource_kind_tag(*creates)
        ),
    }
}

/// The Action Palette catalog projection: filtered rows plus the
/// catalog epoch used for stale-update rejection.
#[derive(Clone, Debug, Default)]
pub struct ActionCatalogView {
    /// Matching rows in catalog order (filtered by `query`).
    pub rows: Vec<ActionRow>,
    /// The applied query text.
    pub query: String,
    /// Catalog epoch — bumped by the producer on every catalog change;
    /// consumers reject older epochs.
    pub seq: u64,
}

/// Projects the capability catalog for the palette.
///
/// `grant_of` supplies the authority's grant verdict per descriptor —
/// the projection calls it once per row and never caches authority
/// decisions. `query` filters on capability id / domain name
/// (case-insensitive substring); non-matching rows are omitted while
/// every *matching* row — including blocked ones — stays visible so
/// unsupported/unauthorized/unknown states remain inspectable.
#[must_use]
pub fn project_action_catalog(
    descriptors: &[CapabilityDescriptor],
    grant_of: impl Fn(&CapabilityDescriptor) -> GrantState,
    query: &str,
    seq: u64,
) -> ActionCatalogView {
    let needle = query.trim().to_lowercase();
    let rows = descriptors
        .iter()
        .filter(|d| {
            needle.is_empty()
                || d.id.to_string().to_lowercase().contains(&needle)
                || d.domain.name().contains(&needle)
        })
        .map(|d| project_action_row(d, grant_of(d)))
        .collect();
    ActionCatalogView {
        rows,
        query: query.to_string(),
        seq,
    }
}

// ── Macro Browser ─────────────────────────────────────────────────────

/// One step of a macro preset — a capability call the atomic apply
/// lowers to. `args` is the step's declared argument object.
#[derive(Clone, Debug, PartialEq)]
pub struct MacroStepView {
    /// The capability this step invokes.
    pub capability: CapabilityRef,
    /// Declared step arguments (validated by the capability's input
    /// schema at bind time — the GUI does not pre-validate).
    pub args: Value,
    /// The explicit target for this step, if the contract requires one.
    pub target: Option<ExplicitTarget>,
}

/// One declared macro input (name + display hint only — values are
/// supplied at apply time by the caller, never stored in the view).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacroInputView {
    /// Input name.
    pub name: String,
    /// Whether the input is required.
    pub required: bool,
}

/// A macro preset row — version, inputs, targets, batch effects, undo
/// scope, and availability, all projected from the preset authority.
#[derive(Clone, Debug)]
pub struct MacroPresetView {
    /// Stable preset id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Preset version.
    pub version: String,
    /// Declared inputs.
    pub inputs: Vec<MacroInputView>,
    /// Steps executed atomically on one project.
    pub steps: Vec<MacroStepView>,
    /// Union of step effect bounds (display upper bound).
    pub effects: Vec<EffectKind>,
    /// Undo scope label (`"project-undo"`, `"atomic-rollback"`, `"none"`).
    pub undo_scope: String,
    /// Whether apply is atomic across the whole step list on one project.
    pub atomic: bool,
    /// Preset availability (same vocabulary as capabilities).
    pub availability: Availability,
    /// Authority grant verdict for apply.
    pub grant: GrantState,
    /// Fail-closed display verdict.
    pub executable: bool,
    /// Why apply is not offered.
    pub blocked: Option<String>,
}

/// The Macro Browser projection.
#[derive(Clone, Debug, Default)]
pub struct MacroBrowserView {
    /// Preset rows in catalog order.
    pub presets: Vec<MacroPresetView>,
    /// Catalog epoch for stale rejection.
    pub seq: u64,
}

/// Projects a preset into a display row. `executable` requires
/// `Available` + `Satisfied` grant — same fail-closed rule as actions.
#[must_use]
pub fn project_macro_preset(preset: &MacroPresetView, grant: GrantState) -> MacroPresetView {
    let mut row = preset.clone();
    row.grant = grant;
    row.executable = matches!(row.availability, Availability::Available) && row.grant.satisfied();
    row.blocked = if row.executable {
        None
    } else {
        Some(blocked_reason(&row.availability, &row.grant))
    };
    row
}

/// Builds the [`WireRequest`] sequence a macro apply lowers to — one
/// request per step, in order, all against the same project and the
/// caller-supplied `consistency`.
///
/// This is the *only* place the GUI turns a macro selection into
/// capability calls; the requests carry no authority of their own —
/// `Gateway::bind`/`invoke` re-checks availability, schema, target,
/// and authorization per step. `idempotency_key`, when supplied, is
/// suffixed per step (`<key>/<index>`) so a retried apply replays
/// identically instead of double-applying.
///
/// # Errors
/// [`AutomationUiError::UnknownCapability`] when a step's capability is
/// absent from `descriptors`; [`AutomationUiError::InvalidRequest`] /
/// [`AutomationUiError::TargetMismatch`] / [`NotExecutable`] from the
/// per-step checks.
pub fn macro_step_requests(
    preset: &MacroPresetView,
    descriptors: &[CapabilityDescriptor],
    project: &str,
    request_id: &str,
    consistency: &WireConsistency,
    idempotency_key: Option<&str>,
) -> Result<Vec<WireRequest>, AutomationUiError> {
    // Re-derive the executable verdict from the authority fields rather
    // than trusting the projected flag — `MacroPresetView` fields are
    // public, so a hand-built preset must fail closed identically.
    if !(matches!(preset.availability, Availability::Available) && preset.grant.satisfied()) {
        return Err(AutomationUiError::NotExecutable {
            detail: preset
                .blocked
                .clone()
                .unwrap_or_else(|| format!("macro {} not executable", preset.id)),
        });
    }
    let mut out = Vec::with_capacity(preset.steps.len());
    for (index, step) in preset.steps.iter().enumerate() {
        let descriptor = descriptors
            .iter()
            .find(|d| d.capability_ref() == step.capability)
            .ok_or_else(|| AutomationUiError::UnknownCapability {
                capability: step.capability.to_string(),
            })?;
        // Same fail-closed rule as the palette path: a step whose
        // capability is not `Available` never lowers to a request.
        if !matches!(descriptor.availability, Availability::Available) {
            return Err(AutomationUiError::NotExecutable {
                detail: format!("{} is {}", step.capability, descriptor.availability.name()),
            });
        }
        let target = match &step.target {
            Some(t) => Some(t.to_wire()?),
            None => None,
        };
        // The step's explicit target must satisfy the capability's
        // contract — same fail-closed check as the palette capture.
        let explicit = target.clone().map(|w| match w {
            WireTarget::Resource { kind, id } => ExplicitTarget::Resource { kind, id },
            WireTarget::ResourceSet { kind, ids } => ExplicitTarget::ResourceSet { kind, ids },
            WireTarget::Create {
                parent_kind,
                parent,
                creates,
            } => ExplicitTarget::Create {
                parent_kind,
                parent,
                creates,
            },
        });
        capture_target(descriptor, project, explicit)?;
        out.push(WireRequest {
            request_id: RequestId::new(format!("{request_id}/{index}"))
                .map_err(|e| invalid(format!("request id: {e}")))?,
            capability: step.capability.clone(),
            schema: descriptor.input_schema.clone(),
            project: project.to_string(),
            target,
            args: step.args.clone(),
            consistency: consistency.clone(),
            task: None,
            grant: None,
            idempotency_key: idempotency_key.map(|k| format!("{k}/{index}")),
        });
    }
    Ok(out)
}

// ── Workflow Editor v1 ────────────────────────────────────────────────

/// Bounded workflow step kinds (AUTX-05 contract vocabulary). The v1
/// editor visualizes this closed set only — no free-form graph or code.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum WorkflowStepKind {
    /// Read-only context query.
    Query,
    /// Atomic capability batch.
    AtomicBatch,
    /// Asynchronous job submission.
    JobSubmit,
    /// Wait on a job/run.
    Await,
    /// Collect a result.
    Result,
    /// Conditional branch.
    If,
    /// Bounded iteration.
    BoundedLoop,
}

impl WorkflowStepKind {
    /// Stable display/serialization name.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Query => "query",
            Self::AtomicBatch => "atomic-batch",
            Self::JobSubmit => "job-submit",
            Self::Await => "await",
            Self::Result => "result",
            Self::If => "if",
            Self::BoundedLoop => "bounded-loop",
        }
    }
}

/// Plan validation verdict — the editor displays it verbatim; it never
/// re-validates the plan itself.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanValidation {
    /// The plan passed validation.
    Valid,
    /// The plan failed validation; `reasons` are the validator's own
    /// messages, displayed verbatim.
    Invalid {
        /// Validator-reported reasons.
        reasons: Vec<String>,
    },
    /// No validation result is available — never shown as valid.
    Unknown,
}

/// One workflow step row for the editor.
#[derive(Clone, Debug)]
pub struct WorkflowStepView {
    /// Step index in plan order.
    pub index: usize,
    /// Step kind.
    pub kind: WorkflowStepKind,
    /// Display label.
    pub label: String,
    /// Capability the step invokes, when it invokes one.
    pub capability: Option<CapabilityRef>,
    /// Declared bound (loop iteration cap / await timeout label).
    pub bound: Option<String>,
}

/// The Workflow Editor projection: plan steps + validation + the
/// approval *display* state (GW-09 contract — the GUI shows approval
/// status, it never mints or evaluates approvals).
#[derive(Clone, Debug)]
pub struct WorkflowPlanView {
    /// Plan name.
    pub name: String,
    /// Plan version.
    pub version: String,
    /// Steps in plan order.
    pub steps: Vec<WorkflowStepView>,
    /// Validator verdict.
    pub validation: PlanValidation,
    /// Approval status label from the authority (`"none"`, `"required"`,
    /// `"approved"`, `"declined"`, …) — display only.
    pub approval: String,
    /// Catalog epoch for stale rejection.
    pub seq: u64,
}

/// Projects a workflow plan for the editor. Pure projection — the
/// caller supplies the plan's already-lowered step list and the
/// validator's verdict.
#[must_use]
pub fn project_workflow_plan(
    name: &str,
    version: &str,
    steps: Vec<WorkflowStepView>,
    validation: PlanValidation,
    approval: &str,
    seq: u64,
) -> WorkflowPlanView {
    WorkflowPlanView {
        name: name.to_string(),
        version: version.to_string(),
        steps,
        validation,
        approval: approval.to_string(),
        seq,
    }
}

// ── Run Monitor ───────────────────────────────────────────────────────

/// The monitor's display status — the *only* vocabulary the run
/// surface renders. Every authority state maps to exactly one label;
/// pending is never collapsed into completed, and cancellation
/// requested/confirmed/unconfirmed stay distinct.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MonitorStatus {
    /// Admitted, not yet started.
    Pending,
    /// Executing.
    Running,
    /// Provider-confirmed completion.
    Completed,
    /// Provider-side failure.
    Failed,
    /// Cancel requested, not yet confirmed.
    CancelRequested,
    /// Cancel requested but the provider never confirmed — the job's
    /// fate is unknown (indeterminate), never shown as cancelled.
    CancelUnconfirmed,
    /// Provider-confirmed cancellation.
    Cancelled,
    /// The run/step outcome cannot be determined — reconciliation
    /// (recovery) is required before any success/failure claim.
    Indeterminate,
}

impl MonitorStatus {
    /// Stable display label.
    #[must_use]
    pub const fn label(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::CancelRequested => "cancel-requested",
            Self::CancelUnconfirmed => "cancel-unconfirmed",
            Self::Cancelled => "cancelled",
            Self::Indeterminate => "indeterminate",
        }
    }
}

/// One observed job behind a run — the authority's own phase and cancel
/// linkage, carried verbatim for projection.
#[derive(Clone, Debug)]
pub struct JobObservation {
    /// Gateway operation id the job runs under.
    pub operation: String,
    /// Coordinator job reference.
    pub job: String,
    /// Last provider-observed phase (verbatim).
    pub phase: JobPhase,
    /// Cancel linkage axis (verbatim).
    pub cancel: CancelLink,
    /// Provider-reported progress (0..=100), when reported.
    pub progress: Option<u64>,
}

/// One run-monitor row: the run/operation identity plus its job
/// observation when the run is job-backed.
#[derive(Clone, Debug)]
pub struct RunRow {
    /// Run identifier (authority-issued).
    pub run: String,
    /// Gateway operation id, when known.
    pub operation: Option<String>,
    /// Workflow step index this row covers, when step-scoped.
    pub step: Option<usize>,
    /// Project revision the run is bound to.
    pub project_revision: Option<ProjectRevision>,
    /// Job observation, when the row is job-backed.
    pub job: Option<JobObservation>,
    /// Authority run state label (verbatim, e.g. `"running"`).
    pub run_state: String,
    /// Whether the run requires reconciliation before any terminal
    /// claim (authority `Indeterminate` → recovery required).
    pub recovery_required: bool,
    /// Projected display status.
    pub status: MonitorStatus,
    /// Whether a cancel has been requested (requested ≠ confirmed).
    pub cancel_requested: bool,
}

/// Maps one job observation to its display status — the fail-closed
/// rules the monitor must never violate:
///
/// - `Pending`/`Running` stay pending/running — never completed.
/// - `Completed` is only `Completed` when the provider said so.
/// - `Cancelled` phase is `Cancelled` only with `CancelLink::Confirmed`;
///   `Requested`/`Unconfirmed` linkage surfaces as
///   `CancelRequested`/`CancelUnconfirmed` instead.
/// - `CancelLink::Unconfirmed` on any non-terminal phase is
///   `Indeterminate` — the provider's fate is unknown. `Pending` honors
///   the same cancel axis: a requested cancel is `CancelRequested`,
///   and a `Confirmed` linkage on a still-pending phase is
///   `Indeterminate` (contradictory authority signals).
#[must_use]
pub fn project_job_status(obs: &JobObservation) -> MonitorStatus {
    match (&obs.phase, obs.cancel) {
        (JobPhase::Pending, CancelLink::Requested) => MonitorStatus::CancelRequested,
        (JobPhase::Pending, CancelLink::Unconfirmed | CancelLink::Confirmed) => {
            MonitorStatus::Indeterminate
        }
        (JobPhase::Pending, _) => MonitorStatus::Pending,
        (
            JobPhase::Running | JobPhase::Completed | JobPhase::Failed { .. },
            CancelLink::Unconfirmed,
        ) => MonitorStatus::Indeterminate,
        (JobPhase::Running | JobPhase::Cancelled, CancelLink::Requested) => {
            MonitorStatus::CancelRequested
        }
        (JobPhase::Running, _) => MonitorStatus::Running,
        (JobPhase::Completed, _) => MonitorStatus::Completed,
        (JobPhase::Failed { .. }, _) => MonitorStatus::Failed,
        (JobPhase::Cancelled, CancelLink::Confirmed) => MonitorStatus::Cancelled,
        (JobPhase::Cancelled, _) => MonitorStatus::CancelUnconfirmed,
    }
}

/// One run-monitor input row — the authority's run/job observation
/// carried verbatim into the projection.
#[derive(Clone, Debug)]
pub struct RunObservation {
    /// Run identifier (authority-issued).
    pub run: String,
    /// Gateway operation id, when known.
    pub operation: Option<String>,
    /// Workflow step index this row covers, when step-scoped.
    pub step: Option<usize>,
    /// Project revision the run is bound to.
    pub project_revision: Option<ProjectRevision>,
    /// Authority run state label (verbatim, e.g. `"running"`).
    pub run_state: String,
    /// Whether the run requires reconciliation before any terminal
    /// claim (authority `Indeterminate` → recovery required).
    pub recovery_required: bool,
    /// Job observation, when the row is job-backed.
    pub job: Option<JobObservation>,
}

/// The Run Monitor projection.
#[derive(Clone, Debug, Default)]
pub struct RunMonitorView {
    /// Run rows in authority order.
    pub rows: Vec<RunRow>,
    /// Monitor epoch for stale rejection — bumped on every authority
    /// update; consumers reject older epochs.
    pub seq: u64,
}

/// Projects run/job observations into monitor rows.
#[must_use]
pub fn project_run_monitor(observations: Vec<RunObservation>, seq: u64) -> RunMonitorView {
    let rows = observations
        .into_iter()
        .map(|obs| {
            let (status, cancel_requested) = match &obs.job {
                Some(job) => (
                    project_job_status(job),
                    !matches!(job.cancel, CancelLink::NotRequested),
                ),
                None => (MonitorStatus::Pending, false),
            };
            RunRow {
                run: obs.run,
                operation: obs.operation,
                step: obs.step,
                project_revision: obs.project_revision,
                job: obs.job,
                run_state: obs.run_state,
                recovery_required: obs.recovery_required,
                status,
                cancel_requested,
            }
        })
        .collect();
    RunMonitorView { rows, seq }
}

// ── view-local automation state (persisted per view) ──────────────────

/// Per-view automation surface state — palette query/selection, the
/// frozen captured target, last outcome, macro/workflow/run selections.
///
/// This is *view-local* state only: it lives in [`crate::ViewState`]
/// keyed by [`ruviform_ui::dock::ViewInstanceId`], survives dock moves
/// and window detaches, and never touches [`crate::ProjectSession`] —
/// project/grant/job truth stays with the Gateway/Automation owners.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AutomationViewState {
    /// Palette query text (view-local filter).
    #[serde(default)]
    pub palette_query: String,
    /// Selected palette row index into the last applied catalog.
    #[serde(default)]
    pub palette_selected: Option<usize>,
    /// Selected palette category (domain tab index).
    #[serde(default)]
    pub palette_category: Option<usize>,
    /// The frozen explicit target captured at confirm time.
    #[serde(default)]
    pub palette_target: Option<CapturedTarget>,
    /// Last palette outcome label (`"preview"`, `"submitted"`,
    /// `"receipt"`, `"error"`) plus detail text.
    #[serde(default)]
    pub palette_outcome: Option<String>,
    /// Catalog epoch last applied — older catalogs are stale.
    #[serde(default)]
    pub catalog_seq: u64,
    /// Selected macro preset id.
    #[serde(default)]
    pub macro_selected: Option<String>,
    /// Selected workflow step index.
    #[serde(default)]
    pub workflow_step: Option<usize>,
    /// Selected run/operation id in the monitor.
    #[serde(default)]
    pub run_selected: Option<String>,
    /// Monitor epoch last applied — older updates are stale.
    #[serde(default)]
    pub monitor_seq: u64,
}

impl AutomationViewState {
    /// Whether `seq` is older than the applied catalog epoch.
    #[must_use]
    pub fn catalog_stale(&self, seq: u64) -> bool {
        seq < self.catalog_seq
    }

    /// Whether `seq` is older than the applied monitor epoch.
    #[must_use]
    pub fn monitor_stale(&self, seq: u64) -> bool {
        seq < self.monitor_seq
    }
}

// ── view actions → capability binding ─────────────────────────────────

/// A decoded automation-surface view action — the product-level
/// `ViewAction` the GUI owns. Produced by the app's wire decoder from
/// `Button`/`Table`/`Tabs` intents; consumed by
/// [`automation_requests`] to mint gateway wire requests.
#[derive(Clone, Debug, PartialEq)]
pub enum AutomationViewAction {
    /// Palette row selection changed (view-local; params `{"row": n}`).
    PaletteSelect {
        /// Selected row index.
        row: usize,
    },
    /// Palette category tab changed (view-local; params `{"index": n}`).
    PaletteCategory {
        /// Selected category index.
        index: usize,
    },
    /// Capture/confirm the explicit target for `capability`.
    CaptureTarget {
        /// `"id@version"` capability label.
        capability: String,
        /// The user-confirmed explicit target.
        target: Option<ExplicitTarget>,
    },
    /// Preview `capability` — dry-run projection only, no commit.
    Preview {
        /// `"id@version"` capability label.
        capability: String,
        /// Explicit target (required iff the contract demands one).
        target: Option<ExplicitTarget>,
        /// Operation arguments.
        args: Value,
    },
    /// Submit `capability` for real.
    Submit {
        /// `"id@version"` capability label.
        capability: String,
        /// Explicit target.
        target: Option<ExplicitTarget>,
        /// Operation arguments.
        args: Value,
        /// Optional idempotency key.
        idempotency_key: Option<String>,
    },
    /// Preview macro preset `preset` (no project mutation).
    MacroPreview {
        /// Preset id.
        preset: String,
    },
    /// Apply macro preset `preset` atomically on `project`.
    MacroApply {
        /// Preset id.
        preset: String,
        /// Optional idempotency key (suffixed per step).
        idempotency_key: Option<String>,
    },
    /// Request cancellation of job `operation` (request ≠ confirmed —
    /// the monitor keeps showing the linkage axis).
    JobCancel {
        /// Operation id text.
        operation: String,
    },
    /// Macro Browser row selection changed (view-local).
    MacroSelect {
        /// Selected preset row index.
        index: usize,
    },
    /// Workflow Editor step selection changed (view-local).
    WorkflowSelect {
        /// Selected step index.
        index: usize,
    },
    /// Run Monitor row selection changed (view-local).
    RunSelect {
        /// Selected run row index.
        index: usize,
    },
}

/// Resolves a `"id@version"` capability label against the catalog.
///
/// # Errors
/// [`AutomationUiError::UnknownCapability`] / [`InvalidRequest`].
pub fn resolve_capability<'a>(
    descriptors: &'a [CapabilityDescriptor],
    label: &str,
) -> Result<&'a CapabilityDescriptor, AutomationUiError> {
    let (id, version) = label
        .rsplit_once('@')
        .ok_or_else(|| invalid(format!("capability label {label:?} lacks @version")))?;
    descriptors
        .iter()
        .find(|d| d.id.to_string() == id && d.capability_ref().version.to_string() == version)
        .ok_or_else(|| AutomationUiError::UnknownCapability {
            capability: label.to_string(),
        })
}

/// Lowers [`AutomationViewAction::JobCancel`] to its single
/// [`WireRequest`] — `synvelis.job.cancel` is the gateway job
/// capability; the cancel *request* only, confirmation arrives via
/// the job book's `CancelLink`, never assumed here.
///
/// The capability must resolve unambiguously in the supplied catalog
/// (the composition root includes job capabilities when jobs are
/// wired), and the same fail-closed gates as every other lowering
/// path apply — target contract and availability. `operation` must
/// already be the tagged `operation-<32hex>` form
/// `job::operation_arg` parses; it is validated here rather than
/// emitting a request doomed to fail at bind.
///
/// # Errors
/// [`AutomationUiError::UnknownCapability`] when no cancel capability
/// is registered; [`InvalidRequest`] when the catalog is ambiguous or
/// the operation id is malformed; [`TargetMismatch`] /
/// [`NotExecutable`] from the shared gates.
fn job_cancel_request(
    descriptors: &[CapabilityDescriptor],
    project: &str,
    request_id: &str,
    consistency: &WireConsistency,
    operation: &str,
) -> Result<WireRequest, AutomationUiError> {
    let mut found = descriptors
        .iter()
        .filter(|d| d.id.to_string() == synvelis_gateway::JOB_CANCEL_ID);
    let descriptor = found
        .next()
        .ok_or_else(|| AutomationUiError::UnknownCapability {
            capability: synvelis_gateway::JOB_CANCEL_ID.to_string(),
        })?;
    if found.next().is_some() {
        return Err(invalid(format!(
            "ambiguous {} capability in catalog",
            synvelis_gateway::JOB_CANCEL_ID
        )));
    }
    capture_target(descriptor, project, None)?;
    if !matches!(descriptor.availability, Availability::Available) {
        return Err(AutomationUiError::NotExecutable {
            detail: format!(
                "{} is {}",
                synvelis_gateway::JOB_CANCEL_ID,
                descriptor.availability.name()
            ),
        });
    }
    let operation_id = serde_json::from_value::<synvelis_gateway::OperationId>(Value::String(
        operation.to_string(),
    ))
    .map_err(|e| invalid(format!("job cancel operation id: {e}")))?;
    Ok(WireRequest {
        request_id: RequestId::new(request_id.to_string())
            .map_err(|e| invalid(format!("request id: {e}")))?,
        capability: descriptor.capability_ref(),
        schema: descriptor.input_schema.clone(),
        project: project.to_string(),
        target: None,
        args: serde_json::json!({ "operation": operation_id.to_tagged() }),
        consistency: consistency.clone(),
        task: None,
        grant: None,
        idempotency_key: None,
    })
}

/// The single/few [`WireRequest`]s an [`AutomationViewAction`] lowers
/// to — the GUI-owned `ViewAction → Capability` conversion. The
/// requests carry no authority: `Gateway::bind`/`invoke` re-checks
/// everything per call.
///
/// `consistency` is supplied by the caller (the composition root owns
/// which consistency a preview vs submit uses); `request_id` is the
/// caller-minted correlation id; `macros` resolves [`MacroApply`].
///
/// # Errors
/// Propagates [`AutomationUiError`] from resolution/target checks and
/// [`AutomationUiError::NotExecutable`] when the target capability or
/// preset is not executable.
pub fn automation_requests(
    action: &AutomationViewAction,
    descriptors: &[CapabilityDescriptor],
    macros: &[MacroPresetView],
    project: &str,
    request_id: &str,
    consistency: &WireConsistency,
) -> Result<Vec<WireRequest>, AutomationUiError> {
    let single = |capability: &str,
                  target: &Option<ExplicitTarget>,
                  args: &Value,
                  idempotency_key: Option<&str>|
     -> Result<WireRequest, AutomationUiError> {
        let descriptor = resolve_capability(descriptors, capability)?;
        let explicit = target.clone();
        capture_target(descriptor, project, explicit)?;
        if !matches!(descriptor.availability, Availability::Available) {
            return Err(AutomationUiError::NotExecutable {
                detail: format!("{capability} is {}", descriptor.availability.name()),
            });
        }
        Ok(WireRequest {
            request_id: RequestId::new(request_id.to_string())
                .map_err(|e| invalid(format!("request id: {e}")))?,
            capability: descriptor.capability_ref(),
            schema: descriptor.input_schema.clone(),
            project: project.to_string(),
            target: match target {
                Some(t) => Some(t.to_wire()?),
                None => None,
            },
            args: args.clone(),
            consistency: consistency.clone(),
            task: None,
            grant: None,
            idempotency_key: idempotency_key.map(str::to_string),
        })
    };
    match action {
        AutomationViewAction::Preview {
            capability,
            target,
            args,
        } => Ok(vec![single(capability, target, args, None)?]),
        AutomationViewAction::Submit {
            capability,
            target,
            args,
            idempotency_key,
        } => Ok(vec![single(
            capability,
            target,
            args,
            idempotency_key.as_deref(),
        )?]),
        AutomationViewAction::MacroApply {
            preset,
            idempotency_key,
        } => {
            let preset = macros.iter().find(|p| p.id == *preset).ok_or_else(|| {
                AutomationUiError::UnknownCapability {
                    capability: format!("macro preset {preset}"),
                }
            })?;
            macro_step_requests(
                preset,
                descriptors,
                project,
                request_id,
                consistency,
                idempotency_key.as_deref(),
            )
        }
        AutomationViewAction::JobCancel { operation } => Ok(vec![job_cancel_request(
            descriptors,
            project,
            request_id,
            consistency,
            operation,
        )?]),
        AutomationViewAction::PaletteSelect { .. }
        | AutomationViewAction::PaletteCategory { .. }
        | AutomationViewAction::CaptureTarget { .. }
        | AutomationViewAction::MacroPreview { .. }
        | AutomationViewAction::MacroSelect { .. }
        | AutomationViewAction::WorkflowSelect { .. }
        | AutomationViewAction::RunSelect { .. } => Ok(Vec::new()),
    }
}

/// Catalog lookup helper for tests/composition: capability label →
/// row in a projected catalog.
#[must_use]
pub fn catalog_row<'a>(view: &'a ActionCatalogView, label: &str) -> Option<&'a ActionRow> {
    view.rows.iter().find(|r| r.capability_label() == label)
}
