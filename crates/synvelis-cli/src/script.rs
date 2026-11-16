//! `--file` classification + real headless execution — the
//! fail-closed script/DSL gate and the synvelis#262 run lanes
//! (AUTX-CLI-FILE-01, design-script §4/§5).
//!
//! Extension → kind/language/backend mapping is fixed by the AUTX-00
//! contract:
//!
//! | Extension | `action run` | `workflow run` |
//! | --- | --- | --- |
//! | `.luau` | Action Script (Luau) | Workflow Script (Luau) |
//! | `.yaml`/`.yml` | reject — DSL is workflow-only | Workflow DSL |
//! | `.rhai` | Action Script (Rhai, **`PoC` only**) | Workflow Script (Rhai, **`PoC` only**) |
//! | `.lua` | reject — Lua 5.4 is not supported | reject — Lua 5.4 is not supported |
//! | anything else | reject — unknown extension | reject — unknown extension |
//!
//! Real execution (synvelis#262): `.luau` sources evaluate through the
//! vendored Luau backend (`synvelis-script-luau`), script surfaces
//! come from the canonical `surface_for_snapshot` generator over the
//! *live* gateway registries captured at boot, and workflows compile
//! to `ExecutablePlan`s driven by `WorkflowCoordinator` over
//! `GatewayStepLane`/`GatewayJobLane` — the same gateway, adapter,
//! commit boundary, and registry authority every other CLI path uses.
//! `.rhai` stays PoC-only (`Unsupported`); `.lua` and unknown
//! extensions fail closed at classification.

use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value, json};
use synvelis_automation::coordinator::{
    OperationObservation, RunContext, RunMeta, StepCall, StepLane, WorkflowCoordinator,
};
use synvelis_automation::run::{MemoryRunJournal, RunId, RunState};
use synvelis_automation::{
    AutomationError, ExecutablePlan, FileWorkflowResolver, GatewayJobLane, GatewayStepLane,
    MacroRegistry, PlanCompiler, PlanWarning, ScriptBuildContext, WorkflowDefinition,
    compile_action_recipe, evaluate_luau_action, evaluate_luau_spec, import_workflow_document,
    parse_workflow_yaml,
};
use synvelis_gateway::id::{IdempotencyKey, OperationId};
use synvelis_gateway::outcome::{ErrorEnvelope, GatewayOutcome};
use synvelis_gateway::request::{Principal, PrincipalKind};
use synvelis_script::backend::ExecutionBudget;
use synvelis_script::{
    ACTION_DEFINITION_SCHEMA_VERSION, ActionRecipe, ActionRecipeKind, AvailableBackends,
    BackendRequirement, InvocationDraft, LanguageId, ScriptActionDefinition, WorkflowScriptBuilder,
    sha256_hex,
};
use synvelis_script_luau::LuauBackend;

use crate::args::{Args, Command};
use crate::commands::{self, CommandResult};
use crate::envelope::{CliEnvelope, EnvelopeStatus};
use crate::error::CliError;
use crate::host::{self, Booted};

/// The file-kind axis a `--file` argument was classified into.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FileClass {
    /// `.luau` under `action run` — Luau Action Script.
    ActionScriptLuau,
    /// `.rhai` under `action run` — Rhai Action Script (`PoC` only).
    ActionScriptRhai,
    /// `.yaml`/`.yml` under `workflow run` — Workflow DSL.
    WorkflowDsl,
    /// `.luau` under `workflow run` — Luau Workflow Script.
    WorkflowScriptLuau,
    /// `.rhai` under `workflow run` — Rhai Workflow Script (`PoC` only).
    WorkflowScriptRhai,
}

impl FileClass {
    /// The contract names reported in results/errors.
    #[must_use]
    pub const fn names(self) -> (&'static str, &'static str, &'static str) {
        match self {
            Self::ActionScriptLuau => ("action-script", "luau", "luau-vm"),
            Self::ActionScriptRhai => ("action-script", "rhai", "rhai"),
            Self::WorkflowDsl => ("workflow-dsl", "yaml", "synvelis-automation"),
            Self::WorkflowScriptLuau => ("workflow-script", "luau", "luau-vm"),
            Self::WorkflowScriptRhai => ("workflow-script", "rhai", "rhai"),
        }
    }

    /// Whether this class is a PoC-only coexistence backend.
    #[must_use]
    pub const fn is_poc(self) -> bool {
        matches!(self, Self::ActionScriptRhai | Self::WorkflowScriptRhai)
    }
}

/// Classifies `file` for `command` (`ActionRun`/`WorkflowRun` only).
///
/// # Errors
/// [`CliError::InvalidInput`] — unknown extension or
/// extension/subcommand kind mismatch; [`CliError::Unsupported`] —
/// recognized but unsupported language (Lua 5.4).
pub fn classify(command: &Command, file: &Path) -> Result<FileClass, CliError> {
    let ext = file
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    let workflow_side = matches!(command, Command::WorkflowRun { .. });
    match ext.as_deref() {
        Some("luau") => Ok(if workflow_side {
            FileClass::WorkflowScriptLuau
        } else {
            FileClass::ActionScriptLuau
        }),
        Some("yaml" | "yml") => {
            if workflow_side {
                Ok(FileClass::WorkflowDsl)
            } else {
                Err(CliError::InvalidInput(
                    "`.yaml`/`.yml` is the Workflow DSL — it is only valid for \
                     `workflow run --file`, not `action run --file`"
                        .to_owned(),
                ))
            }
        }
        Some("rhai") => Ok(if workflow_side {
            FileClass::WorkflowScriptRhai
        } else {
            FileClass::ActionScriptRhai
        }),
        Some("lua") => Err(CliError::Unsupported(
            "Lua 5.4 (`.lua`) is explicitly not supported; the script \
             contract covers Luau (`.luau`), Workflow DSL (`.yaml`/`.yml`), \
             and PoC-only Rhai (`.rhai`)"
                .to_owned(),
        )),
        Some(other) => Err(CliError::InvalidInput(format!(
            "unknown file extension '.{other}' — supported: .luau, .yaml/.yml, .rhai (PoC)"
        ))),
        None => Err(CliError::InvalidInput(
            "file has no extension — cannot classify script kind".to_owned(),
        )),
    }
}

/// Reads and UTF-8-validates the `--file` payload.
///
/// # Errors
/// [`CliError::InvalidInput`] — missing path, non-file, unreadable, or
/// non-UTF-8 content.
pub fn read_source(file: &Path) -> Result<String, CliError> {
    let canonical = file.canonicalize().map_err(|e| {
        CliError::InvalidInput(format!("cannot resolve --file '{}': {e}", file.display()))
    })?;
    if !canonical.is_file() {
        return Err(CliError::InvalidInput(format!(
            "--file '{}' is not a regular file",
            file.display()
        )));
    }
    std::fs::read_to_string(&canonical).map_err(|e| {
        CliError::InvalidInput(format!("cannot read --file '{}': {e}", file.display()))
    })
}

// ── failure plumbing ─────────────────────────────────────────────
//
// Run-file failures have two honest channels: CLI-side rejections
// travel as [`CliError`] (usage/invalid-input/setup/internal exit
// codes), while script-layer `GatewayError`s wrapped in
// [`synvelis_script::ScriptApiError::Gateway`] keep their typed
// envelope and deterministic exit code via
// [`commands::error_result`]. [`FileFailure`] carries both until the
// booted host is available to render the envelope.

/// A run-file failure before the command result exists.
enum FileFailure {
    /// CLI-side rejection — propagates as [`CliError`].
    Cli(CliError),
    /// A typed gateway envelope — rendered through
    /// [`commands::error_result`] with its own exit code. Boxed:
    /// `ErrorEnvelope` is large and this variant is the cold path.
    Gateway(Box<ErrorEnvelope>),
}

impl From<CliError> for FileFailure {
    fn from(e: CliError) -> Self {
        Self::Cli(e)
    }
}

/// Maps an [`AutomationError`] onto the run-file failure channels:
/// script-layer gateway rejections keep their typed envelope,
/// registry invariant failures are internal, everything else is
/// invalid input (the submitted source/plan was rejected).
fn automation_failure(e: AutomationError) -> FileFailure {
    match e {
        AutomationError::Script(inner) => script_failure(inner),
        AutomationError::Registry { reason } => FileFailure::Cli(CliError::Internal(format!(
            "registry snapshot failed validation: {reason}"
        ))),
        other => FileFailure::Cli(CliError::InvalidInput(format!(
            "workflow source rejected: {other}"
        ))),
    }
}

/// Maps a [`synvelis_script::ScriptApiError`]: `Gateway`-wrapped errors
/// preserve the authoritative typed envelope; `Internal` is an adapter
/// invariant; every other variant is a rejection of the submitted
/// script/definition — invalid input.
fn script_failure(e: synvelis_script::ScriptApiError) -> FileFailure {
    match e {
        synvelis_script::ScriptApiError::Gateway(gw) => {
            FileFailure::Gateway(Box::new(ErrorEnvelope::from(gw)))
        }
        synvelis_script::ScriptApiError::Internal { reason } => {
            FileFailure::Cli(CliError::Internal(reason))
        }
        other => FileFailure::Cli(CliError::InvalidInput(format!("script rejected: {other}"))),
    }
}

/// Lifts a [`FileFailure`] into the command result once the booted
/// host exists to render gateway envelopes.
fn finish(
    booted: &Booted,
    outcome: Result<CommandResult, FileFailure>,
) -> Result<CommandResult, CliError> {
    match outcome {
        Ok(result) => Ok(result),
        Err(FileFailure::Cli(e)) => Err(e),
        Err(FileFailure::Gateway(env)) => Ok(commands::error_result(booted, env.as_ref())),
    }
}

// ── shared run plumbing ──────────────────────────────────────────

/// Headless file-run safety bounds — not workflow semantics. The
/// backend additionally enforces its own 1 MiB source ceiling.
fn luau_budget() -> ExecutionBudget {
    ExecutionBudget {
        max_wall_time: Some(std::time::Duration::from_secs(30)),
        max_memory_bytes: Some(256 << 20),
        ..ExecutionBudget::default()
    }
}

/// A workflow source compiled to its executable plan plus the
/// reporting metadata the envelope carries.
struct CompiledWorkflow {
    /// The validated definition the coordinator executes against.
    definition: WorkflowDefinition,
    /// The compiled plan.
    plan: ExecutablePlan,
    /// Deterministic plan digest (`ExecutablePlan::digest`).
    plan_digest: String,
    /// Non-fatal compile warnings.
    warnings: Vec<PlanWarning>,
    /// Class/evaluation metadata merged into the envelope `result`.
    extras: Value,
}

/// Compiles `source` for `class` against the booted registries —
/// YAML parses directly; `.luau` evaluates through the real backend,
/// lowers through `WorkflowScriptBuilder`, resolves `workflow()`
/// references through a [`FileWorkflowResolver`] rooted at the file's
/// canonical parent directory, then compiles through
/// [`PlanCompiler::compile_with_macros`] with an empty macro registry
/// (named macros fail closed at compile time — AUTX-04 owns their
/// load path).
fn compile_workflow(
    booted: &Booted,
    class: FileClass,
    file: &Path,
    source: &str,
    inputs: &Map<String, Value>,
) -> Result<CompiledWorkflow, FileFailure> {
    let (kind, language, backend_name) = class.names();
    let base = json!({
        "kind": kind,
        "language": language,
        "backend_requirement": backend_name,
        "poc_only": class.is_poc(),
        "backend_available": true,
    });
    let compiled = match class {
        FileClass::WorkflowDsl => {
            let definition = parse_workflow_yaml(source).map_err(automation_failure)?;
            let out = PlanCompiler::compile_with_macros(
                &definition,
                inputs,
                &booted.registry,
                &MacroRegistry::empty(),
            )
            .map_err(automation_failure)?;
            let plan_digest = out.plan.digest().map_err(automation_failure)?;
            let mut extras = base;
            extras["workflow_id"] = json!(definition.workflow_id);
            extras["version"] = json!(definition.version);
            extras["steps"] = json!(definition.steps.len());
            extras["inputs"] = json!(definition.inputs.len());
            extras["outputs"] = json!(definition.outputs.len());
            extras["definition_valid"] = json!(true);
            CompiledWorkflow {
                definition,
                plan: out.plan,
                plan_digest,
                warnings: out.warnings,
                extras,
            }
        }
        FileClass::WorkflowScriptLuau => {
            let digest = sha256_hex(source.as_bytes());
            let backend = LuauBackend::new();
            let budget = luau_budget();
            let evaluated = evaluate_luau_spec(&backend, &budget, source, &digest)
                .map_err(automation_failure)?;
            let backends = AvailableBackends::luau();
            let document = WorkflowScriptBuilder::build(
                &evaluated.definition,
                &evaluated.spec,
                &backends,
                &booted.script.model,
                &booted.script.bindings,
            )
            .map_err(script_failure)?;
            let resolver = file_resolver(file, backends, booted, &budget)?;
            let definition =
                import_workflow_document(&document, &resolver).map_err(automation_failure)?;
            let out = PlanCompiler::compile_with_macros(
                &definition,
                inputs,
                &booted.registry,
                &MacroRegistry::empty(),
            )
            .map_err(automation_failure)?;
            let plan_digest = out.plan.digest().map_err(automation_failure)?;
            let mut extras = base;
            extras["evaluated"] = json!(true);
            extras["workflow_id"] = json!(evaluated.definition.script_id);
            extras["version"] = json!(evaluated.definition.version);
            extras["source_digest"] = json!(digest);
            extras["script_steps"] = json!(evaluated.spec.steps.len());
            extras["script_inputs"] = json!(evaluated.spec.inputs.len());
            extras["script_outputs"] = json!(evaluated.spec.outputs.len());
            extras["steps"] = json!(definition.steps.len());
            extras["inputs"] = json!(definition.inputs.len());
            extras["outputs"] = json!(definition.outputs.len());
            extras["definition_valid"] = json!(true);
            CompiledWorkflow {
                definition,
                plan: out.plan,
                plan_digest,
                warnings: out.warnings,
                extras,
            }
        }
        _ => {
            return Err(FileFailure::Cli(CliError::Unsupported(unsupported_reason(
                class,
            ))));
        }
    };
    Ok(compiled)
}

/// Builds the [`FileWorkflowResolver`] for nested `workflow()`
/// references — rooted at the `--file`'s canonical parent directory
/// with the same Luau backend + script build context the top-level
/// evaluation used.
fn file_resolver(
    file: &Path,
    backends: AvailableBackends,
    booted: &Booted,
    budget: &ExecutionBudget,
) -> Result<FileWorkflowResolver, FileFailure> {
    let canonical = file.canonicalize().map_err(|e| {
        CliError::InvalidInput(format!("cannot resolve --file '{}': {e}", file.display()))
    })?;
    let parent = canonical.parent().ok_or_else(|| {
        CliError::Internal(format!(
            "--file '{}' has no parent directory",
            file.display()
        ))
    })?;
    let build = ScriptBuildContext {
        backends,
        model: booted.script.model.clone(),
        bindings: booted.script.bindings.clone(),
        budget: budget.clone(),
    };
    FileWorkflowResolver::with_script_backend(parent, Arc::new(LuauBackend::new()), build)
        .map_err(automation_failure)
}

/// Parses `--inputs`/`--inputs-file` into the declared-inputs object.
/// Every declared input is required — the compiler/coordinator reject
/// missing or mistyped values; a non-object document is invalid input.
fn resolve_inputs(
    inputs: Option<&Value>,
    inputs_file: Option<&Path>,
) -> Result<Map<String, Value>, CliError> {
    let value = match (inputs, inputs_file) {
        (Some(value), None) => value.clone(),
        (None, Some(path)) => {
            let text = std::fs::read_to_string(path).map_err(|e| {
                CliError::InvalidInput(format!(
                    "cannot read --inputs-file '{}': {e}",
                    path.display()
                ))
            })?;
            serde_json::from_str(&text).map_err(|e| {
                CliError::InvalidInput(format!(
                    "--inputs-file '{}' is not valid JSON: {e}",
                    path.display()
                ))
            })?
        }
        (None, None) => Value::Object(Map::new()),
        (Some(_), Some(_)) => {
            return Err(CliError::Usage(
                "--inputs and --inputs-file are mutually exclusive".to_owned(),
            ));
        }
    };
    match value {
        Value::Object(map) => Ok(map),
        _ => Err(CliError::InvalidInput(
            "--inputs must be a JSON object of declared input names".to_owned(),
        )),
    }
}

// ── workflow run ─────────────────────────────────────────────────

/// A unique-per-process run id / idempotency key pair.
fn run_ids() -> Result<(RunId, IdempotencyKey), CliError> {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let nonce = format!("{nanos:x}-{}", std::process::id());
    let run_id = RunId::new(format!("cli-run-{nonce}"))
        .map_err(|e| CliError::Internal(format!("cannot mint run id: {e}")))?;
    let key = IdempotencyKey::new(format!("cli-run-{nonce}"))
        .map_err(|e| CliError::Internal(format!("cannot mint idempotency key: {e}")))?;
    Ok((run_id, key))
}

/// [`GatewayStepLane`] wrapper that retains the last typed gateway
/// [`ErrorEnvelope`] a step produced — the journal records the verdict,
/// but the CLI envelope needs the authoritative error for its
/// exit-code mapping.
struct RecordingStepLane {
    inner: GatewayStepLane,
    last_error: Mutex<Option<ErrorEnvelope>>,
}

impl StepLane for RecordingStepLane {
    fn invoke(&self, call: &StepCall) -> Result<GatewayOutcome<Value>, Box<ErrorEnvelope>> {
        match self.inner.invoke(call) {
            Ok(outcome) => Ok(outcome),
            Err(env) => {
                *self.last_error.lock().expect("lane error lock poisoned") =
                    Some(env.as_ref().clone());
                Err(env)
            }
        }
    }

    fn operation_result(&self, operation: &OperationId) -> OperationObservation {
        self.inner.operation_result(operation)
    }
}

/// Maps a [`synvelis_automation::run::RunError`] from `start_run` —
/// input-validation failures are typed invalid input; everything else
/// is an adapter/composition invariant.
fn run_start_failure(e: synvelis_automation::run::RunError) -> FileFailure {
    use synvelis_automation::run::RunError as R;
    match e {
        R::Eval(eval) => FileFailure::Cli(CliError::InvalidInput(format!(
            "workflow inputs rejected: {eval}"
        ))),
        other => FileFailure::Cli(CliError::Internal(format!(
            "workflow run could not start: {other}"
        ))),
    }
}

/// Runs a compiled workflow through `WorkflowCoordinator` over the
/// live gateway lanes and maps the terminal/parked run state onto the
/// CLI envelope.
fn drive_workflow(
    booted: &Booted,
    args: &Args,
    compiled: &CompiledWorkflow,
    inputs: &Map<String, Value>,
) -> Result<CommandResult, FileFailure> {
    let project = booted.project.as_ref().ok_or_else(|| {
        FileFailure::Cli(CliError::Internal(
            "workflow run requires a project binding".to_owned(),
        ))
    })?;
    let tagged = project.id.to_tagged();
    let principal = Principal::new(PrincipalKind::Agent, args.principal.clone()).map_err(|e| {
        FileFailure::Cli(CliError::InvalidInput(format!(
            "invalid --principal '{}': {e}",
            args.principal
        )))
    })?;
    let (run_id, idempotency_key) = run_ids()?;

    let lane = Arc::new(RecordingStepLane {
        inner: GatewayStepLane::new(
            Arc::clone(&booted.gateway),
            Arc::clone(&booted.cli),
            Arc::clone(&booted.committer),
            None,
        ),
        last_error: Mutex::new(None),
    });
    let jobs = GatewayJobLane::new(
        Arc::clone(&booted.gateway),
        Arc::clone(&booted.cli),
        principal.clone(),
        tagged.clone(),
        None,
        None,
    );
    let journal = Arc::new(MemoryRunJournal::new());
    let coordinator = WorkflowCoordinator::new(journal, lane.clone(), Arc::new(jobs));

    let ctx = RunContext {
        definition: &compiled.definition,
        plan: &compiled.plan,
        inputs,
        registry: &booted.registry,
        macros: &[],
    };
    let meta = RunMeta {
        run_id: run_id.clone(),
        idempotency_key,
        principal,
        project_id: tagged,
        task: None,
        grant: None,
        policy_epoch: None,
        expected_revision: host::live_revision(booted).map(|r| r.0),
    };
    let run_id = coordinator
        .start_run(&ctx, &meta)
        .map_err(run_start_failure)?;
    let run = match coordinator.drive(&run_id, &ctx) {
        Ok(run) => run,
        Err(e) => return Ok(run_error_result(booted, &e, compiled)),
    };
    Ok(run_state_result(booted, &run, compiled, lane.last_error()))
}

impl RecordingStepLane {
    /// The last typed gateway error a step produced, if any.
    fn last_error(&self) -> Option<ErrorEnvelope> {
        self.last_error
            .lock()
            .expect("lane error lock poisoned")
            .clone()
    }
}

/// Result payload shared by every run outcome — the compiled
/// classification/evaluation metadata plus the run's own identity.
fn run_result_base(
    compiled: &CompiledWorkflow,
    run: Option<&synvelis_automation::run::WorkflowRun>,
) -> Value {
    let mut base = compiled.extras.clone();
    base["executed"] = json!(true);
    base["plan_digest"] = json!(compiled.plan_digest);
    base["plan_steps"] = json!(compiled.plan.step_order.len());
    base["warnings"] = serde_json::to_value(&compiled.warnings).unwrap_or(Value::Null);
    if let Some(run) = run {
        base["run_id"] = json!(run.run_id.as_str());
        base["state"] = serde_json::to_value(run.state).unwrap_or(Value::Null);
        base["definition_version"] = json!(run.definition_version);
        base["input_digest"] = json!(run.input_digest);
        base["executed_steps"] = json!(run.executed_steps);
        base["loop_iterations"] = json!(run.loop_iterations);
    }
    base
}

/// Maps the driven [`synvelis_automation::run::WorkflowRun`] onto the
/// envelope: `Completed` → completed/0 + outputs; `Approval` →
/// approval-required/proposal-ready/0 + references (never
/// auto-approved); `Failed` → failed + the lane's typed gateway error
/// or the checkpoint detail; any other state → accepted/7 (non-terminal
/// — `Waiting`/`Indeterminate`/`CancelRequested` included).
fn run_state_result(
    booted: &Booted,
    run: &synvelis_automation::run::WorkflowRun,
    compiled: &CompiledWorkflow,
    lane_error: Option<ErrorEnvelope>,
) -> CommandResult {
    let base = run_result_base(compiled, Some(run));
    match run.state {
        RunState::Completed => {
            let mut result = base;
            result["outputs"] = serde_json::to_value(&run.outputs).unwrap_or(Value::Null);
            result["steps"] = json!(run.steps.len());
            let mut envelope = CliEnvelope::bare(EnvelopeStatus::Completed);
            envelope.project_revision = host::live_revision(booted).map(|r| r.0);
            envelope.result = Some(result);
            commands::result(envelope, 0, None)
        }
        RunState::Approval => {
            let mut proposals = Vec::new();
            let mut approvals = Vec::new();
            for checkpoint in &run.steps {
                if let Some(reference) = &checkpoint.proposal_ref {
                    proposals.push(json!(reference));
                }
                if let Some(reference) = &checkpoint.approval_ref {
                    approvals.push(json!(reference));
                }
            }
            // The run parks on trusted action it cannot self-issue.
            // `ApprovalRequired` wins when an approval ref exists;
            // otherwise the parked outcome is a prepared proposal.
            // The in-memory proposal store means these references do
            // not promise cross-invocation resume.
            let status = if approvals.is_empty() {
                EnvelopeStatus::ProposalReady
            } else {
                EnvelopeStatus::ApprovalRequired
            };
            let mut result = base;
            result["steps"] = json!(run.steps.len());
            let mut envelope = CliEnvelope::bare(status);
            envelope.project_revision = host::live_revision(booted).map(|r| r.0);
            envelope.result = Some(result);
            envelope.references = Some(json!({
                "run_id": run.run_id.as_str(),
                "proposals": proposals,
                "approvals": approvals,
                "resume": "not-supported — proposal/approval stores are in-memory this slice",
            }));
            commands::result(envelope, 0, None)
        }
        RunState::Failed => {
            let mut result = base;
            result["steps"] = json!(run.steps.len());
            if let Some(env) = lane_error {
                // The authoritative typed failure — exit code and
                // error shape come from the gateway envelope verbatim.
                let mut envelope = CliEnvelope::bare(EnvelopeStatus::Failed);
                envelope.project_revision = host::live_revision(booted).map(|r| r.0);
                envelope.result = Some(result);
                envelope.error =
                    Some(serde_json::to_value(env.to_external()).unwrap_or(Value::Null));
                let exit = crate::exitcode::envelope_exit(&env);
                commands::result(envelope, exit, None)
            } else {
                // No lane error recorded — the failure is run-level
                // (expression eval, budget, journal). Report the last
                // failed checkpoint's bounded detail honestly.
                let detail = run
                    .steps
                    .iter()
                    .rev()
                    .find(|c| c.verdict == Some(synvelis_automation::run::StepVerdict::Failed))
                    .and_then(|c| c.detail.clone())
                    .unwrap_or_else(|| "workflow run failed".to_owned());
                let mut envelope = CliEnvelope::bare(EnvelopeStatus::Failed);
                envelope.project_revision = host::live_revision(booted).map(|r| r.0);
                envelope.result = Some(result);
                envelope.error = Some(json!({
                    "code": "cli.run-failed",
                    "category": "validation",
                    "retry": "after-correction",
                    "message": detail,
                }));
                commands::result(envelope, 6, None)
            }
        }
        _ => {
            // Non-terminal: Waiting (job observation pending),
            // Indeterminate (authoritative observation lost),
            // CancelRequested, or a state drive() should not return.
            // Report the real state; never claim completion.
            let mut result = base;
            result["steps"] = json!(run.steps.len());
            result["pending_jobs"] = json!(run.pending_jobs.len());
            let mut envelope = CliEnvelope::bare(EnvelopeStatus::Accepted);
            envelope.project_revision = host::live_revision(booted).map(|r| r.0);
            envelope.result = Some(result);
            commands::result(envelope, 7, None)
        }
    }
}

/// A `drive()`-level [`synvelis_automation::run::RunError`] — the run
/// could not advance. Expression/budget failures are execution
/// failures (exit 6); journal/lane/replay violations are internal
/// (exit 10). The journal retains the run's last honest state.
fn run_error_result(
    booted: &Booted,
    error: &synvelis_automation::run::RunError,
    compiled: &CompiledWorkflow,
) -> CommandResult {
    use synvelis_automation::run::RunError as R;
    let (exit, category, retry, message) = match error {
        R::Eval(eval) => (
            6,
            "validation",
            "after-correction",
            format!("workflow expression evaluation failed: {eval}"),
        ),
        R::BudgetExceeded { kind, detail } => (
            6,
            "validation",
            "after-correction",
            format!("workflow budget exceeded ({kind}): {detail}"),
        ),
        other => (
            10,
            "internal",
            "never",
            format!("workflow run failed: {other}"),
        ),
    };
    let mut envelope = CliEnvelope::bare(EnvelopeStatus::Failed);
    envelope.project_revision = host::live_revision(booted).map(|r| r.0);
    envelope.result = Some(run_result_base(compiled, None));
    envelope.error = Some(json!({
        "code": "cli.run-failed",
        "category": category,
        "retry": retry,
        "message": message,
    }));
    commands::result(envelope, exit, None)
}

/// The workflow `run --file` lanes: compile (and, unless dry-run,
/// drive) a Workflow DSL or Luau Workflow Script source.
fn run_workflow(
    booted: &Booted,
    args: &Args,
    class: FileClass,
    file: &Path,
    source: &str,
    inputs: Option<&Value>,
    inputs_file: Option<&Path>,
) -> Result<CommandResult, FileFailure> {
    let inputs = resolve_inputs(inputs, inputs_file)?;
    let compiled = compile_workflow(booted, class, file, source, &inputs)?;
    if args.dry_run {
        let mut result = compiled.extras.clone();
        result["executed"] = json!(false);
        result["dry_run"] = json!(true);
        result["plan_digest"] = json!(compiled.plan_digest);
        result["plan_steps"] = json!(compiled.plan.step_order.len());
        result["warnings"] = serde_json::to_value(&compiled.warnings).unwrap_or(Value::Null);
        let mut envelope = CliEnvelope::bare(EnvelopeStatus::Completed);
        envelope.project_revision = host::live_revision(booted).map(|r| r.0);
        envelope.result = Some(result);
        return Ok(commands::result(envelope, 0, None));
    }
    drive_workflow(booted, args, &compiled, &inputs)
}

// ── action run ───────────────────────────────────────────────────

/// The `action run --file *.luau` lane: evaluate the `ActionRecipe`
/// artifact through the real backend, build the typed
/// `ScriptActionDefinition` + `ActionRecipe` over the boot-captured
/// script stack, lower to an [`InvocationDraft`], and submit through
/// the same `host::handle` claim path every other CLI invocation
/// takes.
fn run_action(
    booted: &Booted,
    args: &Args,
    file: &Path,
    source: &str,
) -> Result<CommandResult, FileFailure> {
    let digest = sha256_hex(source.as_bytes());
    let backend = LuauBackend::new();
    let evaluated = evaluate_luau_action(&backend, &luau_budget(), source, &digest)
        .map_err(automation_failure)?;
    let spec = evaluated.spec;

    // File metadata pins the definition: script id from the file stem,
    // version 1 (the source digest is the drift anchor), Luau backend.
    let script_id = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("action")
        .to_owned();
    // The script-level schemas anchor on the first member's resolved
    // binding entry (guaranteed resolvable); the generic prepare
    // schemas cover the degenerate empty-member case, which
    // `ActionRecipe::build` rejects typed anyway.
    let (input_schema, output_schema) = spec
        .members
        .first()
        .and_then(|m| booted.script.bindings.get(&m.name))
        .map_or_else(
            || {
                (
                    synvelis_gateway::SchemaRef::new("synvelis.operation.prepare.input", 1, 0),
                    synvelis_gateway::SchemaRef::new("synvelis.operation.output", 1, 0),
                )
            },
            |entry| (entry.input_schema.clone(), entry.output_schema.clone()),
        );
    let definition = ScriptActionDefinition {
        schema_version: ACTION_DEFINITION_SCHEMA_VERSION,
        script_id,
        version: 1,
        language_id: LanguageId::Luau,
        backend_requirement: BackendRequirement::Luau,
        source_digest: digest.clone(),
        input_schema,
        output_schema,
    };
    definition
        .validate(&AvailableBackends::luau(), &booted.script.snapshot)
        .map_err(script_failure)?;
    let recipe = ActionRecipe::build(
        &definition,
        &AvailableBackends::luau(),
        &booted.script.snapshot,
        &booted.script.model,
        &booted.script.bindings,
        spec,
    )
    .map_err(script_failure)?;

    // The bound project is the only project this invocation may act
    // on — a script naming another tagged id fails closed here rather
    // than at the gateway.
    if let Some(binding) = &booted.project {
        let tagged = binding.id.to_tagged();
        if recipe.project != tagged {
            return Err(FileFailure::Cli(CliError::InvalidInput(format!(
                "script declares project '{}' but --project binds '{tagged}'",
                recipe.project
            ))));
        }
    }

    if args.dry_run {
        let (kind, language, backend_name) = FileClass::ActionScriptLuau.names();
        let mut envelope = CliEnvelope::bare(EnvelopeStatus::Completed);
        envelope.project_revision = host::live_revision(booted).map(|r| r.0);
        envelope.result = Some(json!({
            "kind": kind,
            "language": language,
            "backend_requirement": backend_name,
            "poc_only": false,
            "backend_available": true,
            "executed": false,
            "dry_run": true,
            "evaluated": true,
            "script_id": recipe.script_id,
            "script_version": recipe.script_version,
            "source_digest": digest,
            "recipe_kind": recipe.kind().as_str(),
            "members": recipe.members.len(),
            "recipe_digest": recipe.recipe_digest,
            "project": recipe.project,
        }));
        return Ok(commands::result(envelope, 0, None));
    }

    let draft = lower_recipe(booted, &recipe)?;
    draft
        .verify_wire()
        .map_err(|gw| FileFailure::Gateway(Box::new(ErrorEnvelope::from(gw))))?;
    let wire = draft.to_wire_value();
    match host::handle(booted, args, &wire) {
        Ok(outcome) => Ok(commands::outcome_result(booted, outcome)),
        Err(env) => Ok(commands::error_result(booted, &env)),
    }
}

/// Lowers a built [`ActionRecipe`] to its [`InvocationDraft`] — single
/// members directly (prepare-wrapped for `ProjectMutation`), batch
/// members through `compile_action_recipe` → `to_batch_draft`.
fn lower_recipe(booted: &Booted, recipe: &ActionRecipe) -> Result<InvocationDraft, FileFailure> {
    match recipe.kind() {
        ActionRecipeKind::Single => recipe.to_draft().map_err(script_failure),
        ActionRecipeKind::Batch => {
            let compiled =
                compile_action_recipe(recipe, &booted.registry).map_err(automation_failure)?;
            let batch_args = compiled.batch_args().map_err(automation_failure)?;
            recipe.to_batch_draft(&batch_args).map_err(script_failure)
        }
    }
}

// ── entry point ──────────────────────────────────────────────────

/// Runs the file-classification + execution half of `action run
/// --file` / `workflow run --file`.
///
/// `--project` is required for every file run (the bound project is
/// the run's authority scope). `--dry-run` compiles/validates without
/// executing or mutating; real runs boot durable and drive the live
/// lanes.
///
/// # Errors
/// [`CliError`] — usage/classification/source rejection (2/5), boot
/// setup failure (9), or internal invariant (10).
pub fn run_file_command(
    args: &Args,
    file: &Path,
    inputs: Option<&Value>,
    inputs_file: Option<&Path>,
) -> Result<CommandResult, CliError> {
    if args.project.is_none() {
        return Err(CliError::Usage(
            "action run --file / workflow run --file requires --project <path>".to_owned(),
        ));
    }
    let class = classify(&args.command, file)?;
    let source = read_source(file)?;

    // `.rhai` has no installed backend — dry-run reports the
    // classification honestly without booting; a real run is the
    // typed Unsupported rejection.
    if class.is_poc() {
        if args.dry_run {
            let (kind, language, backend) = class.names();
            let mut envelope = CliEnvelope::bare(EnvelopeStatus::Completed);
            envelope.result = Some(json!({
                "kind": kind,
                "language": language,
                "backend_requirement": backend,
                "poc_only": true,
                "backend_available": false,
                "executed": false,
                "dry_run": true,
            }));
            return Ok(commands::result(envelope, 0, None));
        }
        return Err(CliError::Unsupported(unsupported_reason(class)));
    }

    let root = args
        .project
        .as_ref()
        .ok_or_else(|| CliError::Internal("--project presence was validated above".to_owned()))?;
    let booted = host::boot_project(root, !args.dry_run)?;
    let outcome = match class {
        FileClass::WorkflowDsl | FileClass::WorkflowScriptLuau => {
            run_workflow(&booted, args, class, file, &source, inputs, inputs_file)
        }
        FileClass::ActionScriptLuau => run_action(&booted, args, file, &source),
        _ => Err(FileFailure::Cli(CliError::Unsupported(unsupported_reason(
            class,
        )))),
    };
    finish(&booted, outcome)
}

/// The fail-closed rejection for a recognized file whose execution
/// path is not wired in this build.
#[must_use]
pub fn unsupported_reason(class: FileClass) -> String {
    let (kind, language, backend) = class.names();
    if class.is_poc() {
        format!(
            "{kind} ({language}) requires the '{backend}' backend, which is a \
             PoC-only coexistence runtime and is not installed — execution is \
             deferred (synvelis#273 / Ruviform RF#558/#559)"
        )
    } else {
        format!(
            "{kind} ({language}) execution is not wired in this build — the \
             '{backend}' backend exists but this file class has no run lane; \
             the CLI accepted and classified the file but refuses to \
             guess-execute"
        )
    }
}

/// Value-level helper for tests: the extension classifier without IO.
#[cfg(test)]
fn classify_ext(workflow_side: bool, ext: &str) -> Result<FileClass, CliError> {
    match ext.to_ascii_lowercase().as_str() {
        "luau" => Ok(if workflow_side {
            FileClass::WorkflowScriptLuau
        } else {
            FileClass::ActionScriptLuau
        }),
        "yaml" | "yml" if workflow_side => Ok(FileClass::WorkflowDsl),
        "yaml" | "yml" => Err(CliError::InvalidInput("kind mismatch".to_owned())),
        "rhai" => Ok(if workflow_side {
            FileClass::WorkflowScriptRhai
        } else {
            FileClass::ActionScriptRhai
        }),
        "lua" => Err(CliError::Unsupported("lua 5.4".to_owned())),
        other => Err(CliError::InvalidInput(format!("unknown .{other}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extension_matrix() {
        assert_eq!(
            classify_ext(false, "luau").unwrap(),
            FileClass::ActionScriptLuau
        );
        assert_eq!(
            classify_ext(true, "luau").unwrap(),
            FileClass::WorkflowScriptLuau
        );
        assert_eq!(classify_ext(true, "yaml").unwrap(), FileClass::WorkflowDsl);
        assert_eq!(classify_ext(true, "yml").unwrap(), FileClass::WorkflowDsl);
        assert!(matches!(
            classify_ext(false, "yaml"),
            Err(CliError::InvalidInput(_))
        ));
        assert_eq!(
            classify_ext(false, "rhai").unwrap(),
            FileClass::ActionScriptRhai
        );
        assert!(matches!(
            classify_ext(false, "lua"),
            Err(CliError::Unsupported(_))
        ));
        assert!(matches!(
            classify_ext(true, "txt"),
            Err(CliError::InvalidInput(_))
        ));
    }

    #[test]
    fn names_and_poc_flags() {
        assert_eq!(
            FileClass::ActionScriptLuau.names(),
            ("action-script", "luau", "luau-vm")
        );
        assert!(FileClass::WorkflowScriptRhai.is_poc());
        assert!(!FileClass::WorkflowDsl.is_poc());
        assert_ne!(FileClass::WorkflowDsl.names().2, "");
    }
}
