//! Command implementations — each produces a [`CliEnvelope`] plus the
//! deterministic exit code, or a [`CliError`].
//!
//! Every gateway-touching command flows through
//! [`crate::host::Booted::handle`] → `HeadlessSessionHost::handle_claim`
//! → `Gateway::handle`; the CLI never calls `Gateway` directly and
//! never mints a caller itself.

use std::fmt::Write as _;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{Value, json};
use synvelis_gateway::capability::{CapabilityDescriptor, CapabilityVersion, OperationDomain};
use synvelis_gateway::outcome::GatewayOutcome;
use synvelis_project::{
    CounterIdGenerator, DirectoryBundleStore, ProjectController, ProjectId, head_revision,
};

use crate::args::{Args, Command, OperationFlags};
use crate::envelope::{CliEnvelope, EnvelopeStatus};
use crate::error::CliError;
use crate::exitcode;
use crate::host::{self, Booted};
use crate::script;

/// What a command produced: the envelope (machine modes), the exit
/// code, and optional human-readable stdout text.
pub struct CommandResult {
    /// The versioned envelope — always populated for machine output.
    pub envelope: CliEnvelope,
    /// Deterministic exit code.
    pub exit_code: u8,
    /// Human-readable stdout rendering (`--format human`).
    pub human: Option<String>,
}

static REQUEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_request_id() -> String {
    let n = REQUEST_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    format!("cli-{nanos:x}-{n}")
}

/// Runs `args.command` to completion.
///
/// # Errors
/// [`CliError`] — CLI-side rejection (usage, file classification,
/// project boot). Gateway failures are *not* errors here — they are
/// reported inside the returned envelope with their mapped exit code.
pub fn run(args: &Args) -> Result<CommandResult, CliError> {
    match &args.command {
        Command::Help | Command::Version => Err(CliError::Internal(
            "help/version handled before dispatch".to_owned(),
        )),
        Command::ActionList => action_list(args),
        Command::ActionDescribe { capability } => action_describe(args, capability),
        Command::ActionPrepare { operation, flags } => action_prepare(args, operation, flags),
        Command::ActionInvoke { capability, flags } => action_invoke(args, capability, flags),
        Command::ActionRun { file } | Command::WorkflowRun { file } => run_file(args, file),
        Command::ProjectInfo => project_info(args),
        Command::Render { output } => render(args, output),
    }
}

// ── discovery ────────────────────────────────────────────────────

fn action_list(args: &Args) -> Result<CommandResult, CliError> {
    reject_dry_run(args, "action list")?;
    let booted = host::boot_discovery()?;
    let capabilities: Vec<Value> = booted
        .descriptors
        .iter()
        .map(|d| serde_json::to_value(d).unwrap_or(Value::Null))
        .collect();
    let mut envelope = CliEnvelope::bare(EnvelopeStatus::Completed);
    envelope.result = Some(json!({ "capabilities": capabilities }));
    let mut human = String::new();
    for d in &booted.descriptors {
        let _ = writeln!(
            human,
            "{}@{}  {}  {}  {}",
            d.id.as_str(),
            d.version,
            domain_name(d.domain),
            availability_name(&d.availability),
            execution_name(d.execution),
        );
    }
    Ok(result(envelope, 0, Some(human)))
}

fn action_describe(args: &Args, text: &str) -> Result<CommandResult, CliError> {
    reject_dry_run(args, "action describe")?;
    let (id, version) = parse_cap_ref(text)?;
    let booted = host::boot_discovery()?;
    let matches: Vec<&synvelis_gateway::capability::CapabilityDescriptor> = booted
        .descriptors
        .iter()
        .filter(|d| d.id.as_str() == id)
        .filter(|d| version.is_none_or(|v| d.version == v))
        .collect();
    if matches.is_empty() {
        return Err(CliError::Unsupported(format!(
            "unknown capability '{text}' — run `action list` for the registered set"
        )));
    }
    let descriptors: Vec<Value> = matches
        .iter()
        .map(|d| serde_json::to_value(*d).unwrap_or(Value::Null))
        .collect();
    let mut envelope = CliEnvelope::bare(EnvelopeStatus::Completed);
    envelope.result = Some(json!({
        "capability": text,
        "matches": descriptors,
    }));
    let mut human = String::new();
    for d in &matches {
        human.push_str(&serde_json::to_string_pretty(d).unwrap_or_default());
        human.push('\n');
    }
    Ok(result(envelope, 0, Some(human)))
}

// ── gateway operations ───────────────────────────────────────────

fn action_prepare(
    args: &Args,
    operation: &str,
    flags: &OperationFlags,
) -> Result<CommandResult, CliError> {
    let (id, op_version) = parse_cap_ref(operation)?;
    let op_version = op_version.ok_or_else(|| {
        CliError::Usage(format!("--operation '{operation}' requires @major.minor"))
    })?;
    let booted = boot_for_operation(args)?;
    let prepare = find_descriptor(&booted, "synvelis.operation.prepare")?;
    let inner = operation_json(&id, op_version, flags)?;
    // `synvelis.operation.prepare` is TargetContract::None — the
    // target belongs inside `operation`, never at the wire top level.
    let request = build_request(
        &booted,
        prepare,
        &json!({ "operation": inner }),
        flags,
        None,
    )?;
    Ok(invoke_and_report(args, &booted, &request))
}

fn action_invoke(
    args: &Args,
    capability: &str,
    flags: &OperationFlags,
) -> Result<CommandResult, CliError> {
    let (id, version) = parse_cap_ref(capability)?;
    let version = version.ok_or_else(|| {
        CliError::Usage(format!("--capability '{capability}' requires @major.minor"))
    })?;
    let booted = boot_for_operation(args)?;
    let descriptor = find_versioned(&booted, &id, version)?;

    // --dry-run on a ProjectMutation lowers to the non-destructive
    // prepare path; query/proposal domains are already side-effect
    // free; async/runtime control has no preview → fail closed.
    if args.dry_run && descriptor.domain == OperationDomain::ProjectMutation {
        let prepare = find_descriptor(&booted, "synvelis.operation.prepare")?;
        let inner = operation_json(&id, version, flags)?;
        let request = build_request(
            &booted,
            prepare,
            &json!({ "operation": inner }),
            flags,
            None,
        )?;
        return Ok(invoke_and_report(args, &booted, &request));
    }
    if args.dry_run
        && matches!(
            descriptor.domain,
            OperationDomain::RuntimeControl | OperationDomain::AsyncOperation
        )
    {
        return Err(CliError::Unsupported(format!(
            "--dry-run cannot preview '{}' operations (domain {})",
            id,
            domain_name(descriptor.domain)
        )));
    }

    let request = build_request(
        &booted,
        descriptor,
        &resolve_args(flags)?,
        flags,
        flags.target.as_ref(),
    )?;
    Ok(invoke_and_report(args, &booted, &request))
}

// ── file commands ────────────────────────────────────────────────

fn run_file(args: &Args, file: &Path) -> Result<CommandResult, CliError> {
    require_project(args)?;
    let envelope = script::run_file_command(&args.command, file, args.dry_run)?;
    let exit = if envelope.status == "completed" { 0 } else { 5 };
    Ok(result(envelope, exit, None))
}

// ── project info ─────────────────────────────────────────────────

fn project_info(args: &Args) -> Result<CommandResult, CliError> {
    reject_dry_run(args, "project info")?;
    let root = require_project_path(args)?;
    let canonical = root.canonicalize().map_err(|e| {
        CliError::InvalidInput(format!(
            "--project '{}' cannot be resolved: {e}",
            root.display()
        ))
    })?;
    host::require_saved_bundle(&canonical)?;
    let store = DirectoryBundleStore::create(&canonical)
        .map_err(|e| CliError::Setup(format!("cannot open project bundle: {e}")))?;
    if head_revision(&store)
        .map_err(|e| CliError::Setup(format!("cannot read project HEAD: {e}")))?
        .is_none()
    {
        return Err(CliError::Setup(format!(
            "project '{}' has no saved state (no HEAD snapshot)",
            canonical.display()
        )));
    }
    let mut probe = ProjectController::new(
        synvelis_project::Project::new(ProjectId::new(0), ""),
        Box::new(CounterIdGenerator::new(0)),
    );
    probe
        .load(&store)
        .map_err(|e| CliError::Setup(format!("cannot load project snapshot: {e}")))?;
    let project = probe.live();
    let revision = probe.live_revision();
    let mut envelope = CliEnvelope::bare(EnvelopeStatus::Completed);
    envelope.project_revision = Some(revision.0);
    envelope.result = Some(json!({
        "project_id": project.project_id.to_tagged(),
        "name": project.name,
        "revision": revision.0,
    }));
    let human = format!(
        "project: {}\nname: {}\nrevision: {}\n",
        project.project_id.to_tagged(),
        project.name,
        revision.0
    );
    Ok(result(envelope, 0, Some(human)))
}

// ── render ───────────────────────────────────────────────────────

fn render(args: &Args, _output: &std::path::PathBuf) -> Result<CommandResult, CliError> {
    reject_dry_run(args, "render")?;
    require_project_path(args)?;
    Err(CliError::Unsupported(
        "render execution requires a job provider (`synvelis.job.submit` → \
         provider 'render'), and no production JobPort/MediaPort is wired \
         in this build — the command surface is reserved and fails closed \
         (deferred to the media-render integration slice)"
            .to_owned(),
    ))
}

// ── shared helpers ───────────────────────────────────────────────

fn result(envelope: CliEnvelope, exit_code: u8, human: Option<String>) -> CommandResult {
    CommandResult {
        envelope,
        exit_code,
        human,
    }
}

fn reject_dry_run(args: &Args, command: &str) -> Result<(), CliError> {
    if args.dry_run {
        Err(CliError::Usage(format!(
            "--dry-run is not meaningful for {command}"
        )))
    } else {
        Ok(())
    }
}

fn require_project(args: &Args) -> Result<(), CliError> {
    require_project_path(args).map(|_| ())
}

fn require_project_path(args: &Args) -> Result<&std::path::Path, CliError> {
    args.project.as_deref().ok_or_else(|| {
        CliError::Usage(format!(
            "this command requires --project <path> (got command {:?})",
            std::mem::discriminant(&args.command)
        ))
    })
}

/// Boots the project-bound host: durable (commit gate + file policy)
/// for real invocations, read-only in-memory seams for `--dry-run`.
fn boot_for_operation(args: &Args) -> Result<Booted, CliError> {
    let root = require_project_path(args)?;
    host::boot_project(root, !args.dry_run)
}

fn parse_cap_ref(text: &str) -> Result<(String, Option<CapabilityVersion>), CliError> {
    let (id, version) = match text.split_once('@') {
        Some((i, v)) => (i, Some(parse_version(v, text)?)),
        None => (text, None),
    };
    if id.is_empty() {
        return Err(CliError::Usage(format!("empty capability id in '{text}'")));
    }
    Ok((id.to_owned(), version))
}

fn parse_version(text: &str, whole: &str) -> Result<CapabilityVersion, CliError> {
    let (major, minor) = text
        .split_once('.')
        .ok_or_else(|| CliError::Usage(format!("capability ref '{whole}' needs @major.minor")))?;
    let major: u32 = major
        .parse()
        .map_err(|_| CliError::Usage(format!("bad major version in '{whole}'")))?;
    let minor: u32 = minor
        .parse()
        .map_err(|_| CliError::Usage(format!("bad minor version in '{whole}'")))?;
    Ok(CapabilityVersion::new(major, minor))
}

fn find_descriptor<'a>(
    booted: &'a Booted,
    id: &str,
) -> Result<&'a synvelis_gateway::capability::CapabilityDescriptor, CliError> {
    booted
        .descriptors
        .iter()
        .find(|d| d.id.as_str() == id)
        .ok_or_else(|| CliError::Internal(format!("capability '{id}' not registered")))
}

fn find_versioned<'a>(
    booted: &'a Booted,
    id: &str,
    version: CapabilityVersion,
) -> Result<&'a synvelis_gateway::capability::CapabilityDescriptor, CliError> {
    booted
        .descriptors
        .iter()
        .find(|d| d.id.as_str() == id && d.version == version)
        .ok_or_else(|| {
            CliError::Unsupported(format!(
                "capability '{id}@{version}' is not registered (see `action list`)"
            ))
        })
}

/// The `operation` object `synvelis.operation.prepare` wraps — same
/// shape the GW test rig builds (`capability` + `args`, `target` only
/// when present).
fn operation_json(
    id: &str,
    version: CapabilityVersion,
    flags: &OperationFlags,
) -> Result<Value, CliError> {
    let mut op = json!({
        "capability": cap_ref_json(id, version),
        "args": resolve_args(flags)?,
    });
    if let Some(target) = &flags.target {
        op["target"] = target.clone();
    }
    Ok(op)
}

fn cap_ref_json(id: &str, version: CapabilityVersion) -> Value {
    json!({
        "id": id,
        "version": { "major": version.major, "minor": version.minor },
    })
}

fn resolve_args(flags: &OperationFlags) -> Result<Value, CliError> {
    if let Some(path) = &flags.args_file {
        let text = std::fs::read_to_string(path).map_err(|e| {
            CliError::InvalidInput(format!("cannot read --args-file '{}': {e}", path.display()))
        })?;
        return serde_json::from_str(&text)
            .map_err(|e| CliError::InvalidInput(format!("--args-file is not valid JSON: {e}")));
    }
    Ok(flags.args.clone().unwrap_or_else(|| json!({})))
}

fn default_consistency(domain: OperationDomain, revision: Option<u64>) -> Value {
    match domain {
        OperationDomain::Query => json!({ "kind": "latest" }),
        OperationDomain::Proposal | OperationDomain::ProjectMutation => {
            json!({ "kind": "expected-revision", "revision": revision.unwrap_or(0) })
        }
        OperationDomain::RuntimeControl | OperationDomain::AsyncOperation => {
            json!({ "kind": "unconditional" })
        }
    }
}

/// `wire_target` is the top-level request `target` — set only for
/// direct `action invoke` (the invoked capability's own target
/// contract). Prepare-routed requests pass `None`: their target lives
/// inside `operation`, and `synvelis.operation.prepare` declares
/// `TargetContract::None`.
fn build_request(
    booted: &Booted,
    descriptor: &CapabilityDescriptor,
    op_args: &Value,
    flags: &OperationFlags,
    wire_target: Option<&Value>,
) -> Result<Value, CliError> {
    let project = booted.project.as_ref().ok_or_else(|| {
        CliError::Internal("project-bound command without project binding".to_owned())
    })?;
    let consistency = match &flags.consistency {
        Some(c) => c.clone(),
        None => default_consistency(descriptor.domain, host::live_revision(booted).map(|r| r.0)),
    };
    let mut request = json!({
        "request_id": next_request_id(),
        "capability": serde_json::to_value(descriptor.capability_ref())
            .unwrap_or_else(|_| cap_ref_json(descriptor.id.as_str(), descriptor.version)),
        "schema": serde_json::to_value(&descriptor.input_schema).unwrap_or(Value::Null),
        "project": project.id.to_tagged(),
        "args": op_args,
        "consistency": consistency,
    });
    if let Some(t) = wire_target {
        request["target"] = t.clone();
    }
    if let Some(t) = &flags.task {
        request["task"] = json!(t);
    }
    if let Some(g) = &flags.grant {
        request["grant"] = json!(g);
    }
    if let Some(k) = &flags.idempotency_key {
        request["idempotency_key"] = json!(k);
    }
    Ok(request)
}

fn invoke_and_report(args: &Args, booted: &Booted, request: &Value) -> CommandResult {
    match host::handle(booted, args, request) {
        Ok(outcome) => outcome_result(booted, outcome),
        Err(err) => error_result(booted, &err),
    }
}

fn outcome_result(booted: &Booted, outcome: GatewayOutcome<Value>) -> CommandResult {
    let (status, exit) = exitcode::outcome_exit(&outcome);
    let mut envelope = CliEnvelope::bare(status);
    envelope.project_revision = host::live_revision(booted).map(|r| r.0);
    let mut human = String::new();
    match outcome {
        GatewayOutcome::Completed { result } => {
            envelope.result = Some(result.clone());
            let _ = writeln!(human, "status: {}", status.as_str());
            human.push_str(&serde_json::to_string_pretty(&result).unwrap_or_default());
            human.push('\n');
        }
        GatewayOutcome::ProposalReady { proposal } => {
            let reference = proposal.to_string();
            envelope.references = Some(json!({ "proposal": reference }));
            let _ = writeln!(
                human,
                "status: {}\nproposal: {}",
                status.as_str(),
                reference
            );
        }
        GatewayOutcome::ApprovalRequired { approval } => {
            let reference = approval.to_string();
            envelope.references = Some(json!({ "approval": reference }));
            let _ = writeln!(
                human,
                "status: {}\napproval: {}",
                status.as_str(),
                reference
            );
        }
        GatewayOutcome::Accepted { operation } => {
            let id = operation.to_string();
            envelope.operation_id = Some(id.clone());
            envelope.references = Some(json!({ "operation": id }));
            let _ = writeln!(human, "status: {}\noperation: {}", status.as_str(), id);
        }
    }
    result(envelope, exit, Some(human))
}

fn error_result(booted: &Booted, err: &synvelis_gateway::ErrorEnvelope) -> CommandResult {
    let exit = exitcode::envelope_exit(err);
    let mut envelope = CliEnvelope::bare(EnvelopeStatus::Failed);
    envelope.project_revision = host::live_revision(booted).map(|r| r.0);
    envelope.error = Some(serde_json::to_value(err.to_external()).unwrap_or(Value::Null));
    // No human stdout text — the emitter renders the error on stderr,
    // matching the `CliError` path (diagnostics never hit stdout).
    result(envelope, exit, None)
}

fn domain_name(domain: OperationDomain) -> &'static str {
    match domain {
        OperationDomain::Query => "query",
        OperationDomain::Proposal => "proposal",
        OperationDomain::ProjectMutation => "project-mutation",
        OperationDomain::RuntimeControl => "runtime-control",
        OperationDomain::AsyncOperation => "async-operation",
    }
}

fn execution_name(contract: synvelis_gateway::capability::ExecutionContract) -> &'static str {
    match contract {
        synvelis_gateway::capability::ExecutionContract::Sync => "sync",
        synvelis_gateway::capability::ExecutionContract::AsyncAccept => "async-accept",
    }
}

fn availability_name(availability: &synvelis_gateway::capability::Availability) -> String {
    serde_json::to_value(availability)
        .ok()
        .and_then(|v| v.get("state").and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_else(|| "unknown".to_owned())
}

/// Converts a CLI-side error into the same result shape (envelope +
/// exit + human text) — used by `main` for uniform emission.
#[must_use]
pub fn cli_error_result(err: &CliError) -> CommandResult {
    let mut envelope = CliEnvelope::bare(EnvelopeStatus::Failed);
    envelope.error = Some(err.to_json());
    result(envelope, err.exit_code(), None)
}
