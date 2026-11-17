//! Headless host boot — the CLI's only composition root.
//!
//! Everything here wires *existing* seams; nothing is reimplemented:
//!
//! - `Gateway` + `ContextProjectionEngine` + `PrepareEngine` +
//!   `ProposalEngine` provide the capability surface (no `JobEngine` —
//!   there is no production `MediaPort`/`JobPort` provider in this
//!   build, so `synvelis.job.*`/`synvelis.media.*` capabilities are
//!   honestly absent rather than backed by test stubs).
//! - `HeadlessSessionHost` + `LocalCliAdapter` (AUTX-02) resolve every
//!   [`CliSessionClaim`] — unattended claims always become delegated
//!   `Agent` callers on `OriginKind::LocalCli`; `Interactive` claims
//!   verify against the durable [`FileCliSessionStore`] (synvelis#515 —
//!   `--session-store` or the platform user-state default; the OS
//!   credential vault seam lives in [`crate::session`]).
//! - Project state comes from `DirectoryBundleStore` +
//!   `ProjectController`; commits go through `CommitGate` (single
//!   writer, journal, CAS, recovery) when `durable` is set.
//! - Policy comes from `FilePolicyStore` (durable) or
//!   `InMemoryPolicyStore` (dry-run/read-only boots — zero durable
//!   writes).
//!
//! Durable artifact layout inside the project root (namespaced to this
//! adapter so a future GUI wiring can adopt or replace them):
//! `synvelis-cli.policy.log`, `synvelis-cli.commit.journal`.

use std::path::Path;
use std::sync::{Arc, Mutex};

use serde_json::Value;
use synvelis_gateway::capability::CapabilityDescriptor;
use synvelis_gateway::cli::{
    CLI_ADAPTER_RAW, CliSessionClaim, FileCliSessionStore, LocalCliAdapter,
};
use synvelis_gateway::clock::SystemClock;
use synvelis_gateway::commit::{CommitGate, GateDeps, MutexFence, PolicyAdmin};
use synvelis_gateway::context::{ContextBudget, NoSelectionSource};
use synvelis_gateway::context_engine::ContextProjectionEngine;
use synvelis_gateway::durable_policy::FilePolicyStore;
use synvelis_gateway::gateway::Gateway;
use synvelis_gateway::grant::ResourceOracle;
use synvelis_gateway::headless::HeadlessSessionHost;
use synvelis_gateway::id::AdapterId;
use synvelis_gateway::mcp_server::{AdminPort, CommitPort, NoAdminPort, NoCommitPort};
use synvelis_gateway::outcome::ErrorEnvelope;
use synvelis_gateway::policy::GrantPolicy;
use synvelis_gateway::prepare::PrepareAuthorizer;
use synvelis_gateway::prepare_engine::{PrepareBudget, PrepareEngine};
use synvelis_gateway::prepare_policy::PreparePolicy;
use synvelis_gateway::proposal::ProposalStore;
use synvelis_gateway::proposal_engine::{APPLY_ID, ProposalEngine};
use synvelis_gateway::proposal_policy::ProposalGate;
use synvelis_gateway::request::CallerIdentity;
use synvelis_gateway::snapshot::{MemorySnapshotSource, ProjectOracle};
use synvelis_gateway::store::{InMemoryPolicyStore, PolicyStore};
use synvelis_gateway::variant::{VariantPorts, VariantQueryEngine};
use synvelis_gateway::{GatewayOutcome, PolicyClock};
use synvelis_project::{
    CounterIdGenerator, DirectoryBundleStore, Project, ProjectController, ProjectId,
    ProjectRevision, head_revision,
};

use crate::args::Args;
use crate::error::CliError;
use crate::session;

/// TTLs mirroring the gateway test-rig composition (proposal 60s,
/// approval 30s). Proposals/approvals are in-memory this slice — see
/// `evidence/sv262/` for the durable-store residual.
const PROPOSAL_TTL_MS: u64 = 60_000;
const APPROVAL_TTL_MS: u64 = 30_000;

/// Filenames of the durable seams inside the project root.
const POLICY_LOG_NAME: &str = "synvelis-cli.policy.log";
const COMMIT_JOURNAL_NAME: &str = "synvelis-cli.commit.journal";

/// A booted session: the headless host plus every registry/script
/// artifact the composition captured *before* the gateway moved into
/// the host's shared mutex (synvelis#262 — the captures are the
/// run-file lanes' plan-compile and script-eval inputs).
pub struct Booted {
    /// The session host — `handle_claim` is the only request entry.
    pub host: HeadlessSessionHost,
    /// Capability descriptors as registered at boot (discovery
    /// metadata — grants nothing).
    pub descriptors: Vec<CapabilityDescriptor>,
    /// The bound project identity/revision, when project-bound.
    pub project: Option<ProjectBinding>,
    /// The shared gateway handle — the same mutex the host
    /// serializes through; live lanes drive this, never a second
    /// gateway.
    pub gateway: Arc<Mutex<Gateway>>,
    /// The shared CLI adapter — lane principal attestation goes
    /// through the same adapter the host's claims resolve against.
    pub cli: Arc<LocalCliAdapter>,
    /// The commit boundary the host was composed with (`NoCommitPort`
    /// on read-only/discovery boots).
    pub committer: Arc<dyn CommitPort>,
    /// The automation-side registry snapshot captured at boot —
    /// `PlanCompiler`/`WorkflowCoordinator` validate against exactly
    /// this view of the live registries.
    pub registry: synvelis_automation::RegistrySnapshot,
    /// The script-side stack: snapshot → canonical surface → model →
    /// bindings, all derived from the same live registries.
    pub script: ScriptStack,
}

/// The script evaluation stack captured at boot: the script-side
/// registry snapshot, the canonical surface spec derived from it, the
/// built API model, and its generated binding set. Every `.luau`
/// lane consumes this exact stack — never a hand-written surface or
/// a second registry.
pub struct ScriptStack {
    /// `RegistrySnapshot::capture` over the live registries.
    pub snapshot: synvelis_script::RegistrySnapshot,
    /// The canonical surface (`synvelis_automation::surface_for_snapshot`).
    pub surface: synvelis_script::ScriptSurfaceSpec,
    /// `ScriptApiModel::build(snapshot, surface)`.
    pub model: synvelis_script::ScriptApiModel,
    /// `BindingSet::generate(model)`.
    pub bindings: synvelis_script::BindingSet,
}

/// Project identity + live revision at boot.
#[derive(Clone, Debug)]
pub struct ProjectBinding {
    /// The project's stable id.
    pub id: ProjectId,
    /// Live semantic revision at boot.
    pub revision: ProjectRevision,
}

/// The `CliSessionClaim` an invocation presents to the adapter.
///
/// `--session-credential` (or `--session <id>` resolved through the
/// OS credential vault by the command layer) selects `Interactive`,
/// verified against the durable session store; everything else is
/// `Unattended` under `--principal` (default `synvelis-cli`) and can
/// only ever resolve to a delegated `Agent` caller.
#[must_use]
pub fn claim_for(args: &Args) -> CliSessionClaim {
    match &args.session_credential {
        Some(credential) => CliSessionClaim::Interactive {
            credential: credential.clone(),
        },
        None => CliSessionClaim::Unattended {
            principal: args.principal.clone(),
        },
    }
}

/// Runs one wire request through the host under the invocation's claim.
///
/// # Errors
/// The gateway [`ErrorEnvelope`] verbatim — typed category/code/retry
/// preserved for the CLI's exit-code mapping.
#[allow(clippy::result_large_err)] // typed envelopes are the contract
pub fn handle(
    booted: &Booted,
    args: &Args,
    request: &Value,
) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
    booted.host.handle_claim(request, &claim_for(args))
}

/// Live revision probe through the host's admin port, falling back to
/// the boot-time binding revision when no commit boundary is wired
/// (read-only/dry-run boots — nothing can advance the revision there,
/// so the boot value *is* the live value).
#[must_use]
pub fn live_revision(booted: &Booted) -> Option<ProjectRevision> {
    booted
        .project
        .as_ref()
        .and_then(|p| booted.host.admin().live_revision(p.id).or(Some(p.revision)))
}

/// Probes the on-disk bundle layout *before* `DirectoryBundleStore::
/// create` runs — `create` unconditionally `create_dir_all`s
/// `<root>/snapshots/`, which would leave an empty directory inside an
/// arbitrary `--project` path (and inside read-only/dry-run boots that
/// must not write). A real saved bundle always has both `HEAD` and
/// `snapshots/`; anything else is reported as "no saved state" without
/// touching the filesystem.
///
/// # Errors
/// [`CliError::Setup`] when the layout is absent or incomplete.
pub fn require_saved_bundle(canonical: &Path) -> Result<(), CliError> {
    if canonical.join("HEAD").is_file() && canonical.join("snapshots").is_dir() {
        Ok(())
    } else {
        Err(CliError::Setup(format!(
            "project '{}' has no saved state (no HEAD snapshot)",
            canonical.display()
        )))
    }
}

/// Mints a caller directly (trusted in-process path — used by tests and
/// by commands that already hold a verified claim).
///
/// # Errors
/// [`synvelis_gateway::GatewayError`] when the claim cannot be attested
/// (e.g. an interactive claim against an empty session store).
#[allow(clippy::result_large_err)]
pub fn caller_for(
    booted: &Booted,
    args: &Args,
) -> Result<CallerIdentity, synvelis_gateway::GatewayError> {
    booted.host.cli().attest(&claim_for(args))
}

// ── engine stack ────────────────────────────────────────────────

/// Installs the GW-03/04/05 engines into `gateway` over the supplied
/// seams. Job/media capabilities are deliberately absent (no real
/// provider/media ports exist in this build).
#[allow(clippy::result_large_err)] // typed envelopes are the contract
fn install_engines(
    gateway: &mut Gateway,
    policy: Arc<dyn PolicyStore>,
    clock: &Arc<dyn PolicyClock>,
    source: Arc<MemorySnapshotSource>,
    proposals: &Arc<ProposalStore>,
    oracle: &Arc<dyn ResourceOracle>,
) -> Result<Arc<VariantPorts>, ErrorEnvelope> {
    let ctx = Arc::new(ContextProjectionEngine::new(
        source.clone(),
        Arc::new(NoSelectionSource),
        Arc::new(GrantPolicy::new(
            policy.clone(),
            Arc::clone(clock),
            Arc::clone(oracle),
        )),
        Arc::clone(clock),
        ContextBudget::default(),
    ));
    // CB-10: one shared ports value — prepare simulates branch
    // mutations through it, the query engine decodes candidate
    // payloads through its store, and a durable boot hands the same
    // `Arc` to the commit gate's fenced variant dispatch.
    let variant = Arc::new(VariantPorts::standard());
    let prepare_policy: Arc<dyn PrepareAuthorizer> =
        Arc::new(PreparePolicy::new(policy.clone(), Arc::clone(clock)));
    let prepare = Arc::new(
        PrepareEngine::new(
            ctx.clone(),
            prepare_policy.clone(),
            PrepareBudget::default(),
            Arc::clone(proposals),
        )
        .with_variant_ports(Some(variant.clone())),
    );
    let queries = Arc::new(VariantQueryEngine::new(
        ctx.clone(),
        Some(variant.store.clone()),
    ));
    let proposal_engine = Arc::new(ProposalEngine::new(
        Arc::clone(proposals),
        source,
        policy,
        prepare_policy,
    ));
    ctx.install(gateway)?;
    prepare.install(gateway)?;
    proposal_engine.install(gateway)?;
    queries.install(gateway)?;
    Ok(variant)
}

/// Builds the gateway with the `ProposalGate` evaluator wrapping the
/// base `GrantPolicy` (same composition as the GW test rig).
#[allow(clippy::result_large_err)] // typed envelopes are the contract
fn build_gateway(
    policy: Arc<dyn PolicyStore>,
    clock: &Arc<dyn PolicyClock>,
    source: Arc<MemorySnapshotSource>,
    proposals: &Arc<ProposalStore>,
    oracle: &Arc<dyn ResourceOracle>,
) -> Result<(Gateway, Arc<VariantPorts>), ErrorEnvelope> {
    let mut gateway = Gateway::with_evaluator(Box::new(ProposalGate::new(
        Arc::clone(proposals),
        Box::new(GrantPolicy::new(
            policy.clone(),
            Arc::clone(clock),
            Arc::clone(oracle),
        )),
    )));
    let variant = install_engines(&mut gateway, policy, clock, source, proposals, oracle)?;
    Ok((gateway, variant))
}

/// Snapshots every registered descriptor (post-install, pre-host).
fn snapshot_descriptors(gateway: &Gateway) -> Vec<CapabilityDescriptor> {
    gateway
        .registry()
        .list()
        .iter()
        .filter_map(|r| gateway.registry().describe(r).ok().cloned())
        .collect()
}

/// Captures the automation registry snapshot + the full script stack
/// (snapshot → canonical surface → model → bindings) from the live
/// gateway — composition-time, before the gateway moves into the
/// host's shared mutex. Failures are composition invariants: the
/// fixed engine set either captures cleanly or the boot is a bug.
fn capture_run_artifacts(
    gateway: &Gateway,
) -> Result<(synvelis_automation::RegistrySnapshot, ScriptStack), CliError> {
    let internal = |e: String| CliError::Internal(format!("registry/script capture failed: {e}"));
    let registry = synvelis_automation::capture_registry_snapshot(gateway)
        .map_err(|e| internal(e.to_string()))?;
    let snapshot =
        synvelis_script::RegistrySnapshot::capture(gateway.registry(), gateway.schemas())
            .map_err(|e| internal(e.to_string()))?;
    let surface = synvelis_automation::surface_for_snapshot(&snapshot)
        .map_err(|e| internal(e.to_string()))?;
    let model = synvelis_script::ScriptApiModel::build(&snapshot, &surface)
        .map_err(|e| internal(e.to_string()))?;
    let bindings =
        synvelis_script::BindingSet::generate(&model).map_err(|e| internal(e.to_string()))?;
    Ok((
        registry,
        ScriptStack {
            snapshot,
            surface,
            model,
            bindings,
        },
    ))
}

/// Wraps the composed gateway + adapter into the shared handles the
/// host and the run lanes both drive.
fn shared_handles(
    gateway: Gateway,
    clock: Arc<dyn PolicyClock>,
    committer: Arc<dyn CommitPort>,
    admin: Arc<dyn AdminPort>,
    session_store: FileCliSessionStore,
) -> (
    Arc<Mutex<Gateway>>,
    Arc<LocalCliAdapter>,
    HeadlessSessionHost,
) {
    let gateway = Arc::new(Mutex::new(gateway));
    let cli = Arc::new(LocalCliAdapter::new(
        AdapterId::new(CLI_ADAPTER_RAW),
        Box::new(session_store),
        clock,
    ));
    let host =
        HeadlessSessionHost::shared(Arc::clone(&gateway), Arc::clone(&cli), committer, admin);
    (gateway, cli, host)
}

fn apply_descriptor(
    descriptors: &[CapabilityDescriptor],
) -> Result<CapabilityDescriptor, CliError> {
    descriptors
        .iter()
        .find(|d| d.id.as_str() == APPLY_ID)
        .cloned()
        .ok_or_else(|| CliError::Internal("synvelis.proposal.apply not registered".to_owned()))
}

/// Opens the durable CLI session store (`--session-store` or the
/// platform user-state default). A corrupt/forged file fails closed —
/// the boot refuses rather than serving unverifiable sessions.
fn open_session_store(explicit: Option<&Path>) -> Result<FileCliSessionStore, CliError> {
    session::open_store(explicit)
}

// ── boots ────────────────────────────────────────────────────────

/// Boot for discovery commands (`action list`/`action describe`):
/// the full engine stack over empty in-memory seams — no project, no
/// durable files, no session state.
///
/// # Errors
/// [`CliError::Internal`] on registration conflicts (cannot happen with
/// the fixed engine set — surfaces as a bug, not user error).
pub fn boot_discovery(session_store: Option<&Path>) -> Result<Booted, CliError> {
    let clock: Arc<dyn PolicyClock> = Arc::new(SystemClock);
    let store: Arc<dyn PolicyStore> = Arc::new(InMemoryPolicyStore::new());
    let source = Arc::new(MemorySnapshotSource::new());
    let proposals = Arc::new(ProposalStore::new(
        clock.clone(),
        PROPOSAL_TTL_MS,
        APPROVAL_TTL_MS,
        PrepareBudget::default().max_proposals,
    ));
    let oracle: Arc<dyn ResourceOracle> =
        Arc::new(ProjectOracle::of(Project::new(ProjectId::new(0), "")));
    let (gateway, _variant) = build_gateway(store, &clock, source, &proposals, &oracle)
        .map_err(|e| CliError::Internal(format!("gateway composition failed: {}", e.message)))?;
    let descriptors = snapshot_descriptors(&gateway);
    let (registry, script) = capture_run_artifacts(&gateway)?;
    let committer: Arc<dyn CommitPort> = Arc::new(NoCommitPort);
    let admin: Arc<dyn AdminPort> = Arc::new(NoAdminPort);
    let (gateway, cli, host) = shared_handles(
        gateway,
        clock,
        committer.clone(),
        admin,
        open_session_store(session_store)?,
    );
    Ok(Booted {
        host,
        descriptors,
        project: None,
        gateway,
        cli,
        committer,
        registry,
        script,
    })
}

/// Everything a project-bound boot needs after the bundle probe —
/// shared by the read-only and durable compositions.
struct BootContext {
    canonical: std::path::PathBuf,
    store: DirectoryBundleStore,
    project: Project,
    project_id: ProjectId,
    revision: ProjectRevision,
    clock: Arc<dyn PolicyClock>,
    source: Arc<MemorySnapshotSource>,
    proposals: Arc<ProposalStore>,
    oracle: Arc<dyn ResourceOracle>,
}

impl BootContext {
    fn binding(&self) -> ProjectBinding {
        ProjectBinding {
            id: self.project_id,
            revision: self.revision,
        }
    }
}

/// Boot bound to `--project <path>`.
///
/// `durable == false` (dry-run/read-only commands) uses in-memory
/// policy and *no* commit gate — `DirectoryBundleStore::create` is
/// idempotent on an existing bundle and no journal/policy files are
/// created. `durable == true` opens `synvelis-cli.policy.log` +
/// `synvelis-cli.commit.journal` inside the project root and wires the
/// real `CommitGate` as the commit/admin boundary.
///
/// # Errors
/// [`CliError::InvalidInput`] — path missing/not a directory.
/// [`CliError::Setup`] — bundle open/load, policy log, or journal
/// failures; a bundle with no saved HEAD is reported here too.
pub fn boot_project(
    root: &Path,
    durable: bool,
    session_store: Option<&Path>,
) -> Result<Booted, CliError> {
    let canonical = root.canonicalize().map_err(|e| {
        CliError::InvalidInput(format!(
            "--project '{}' cannot be resolved: {e}",
            root.display()
        ))
    })?;
    if !canonical.is_dir() {
        return Err(CliError::InvalidInput(format!(
            "--project '{}' is not a directory",
            canonical.display()
        )));
    }

    require_saved_bundle(&canonical)?;
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

    // Probe-load to learn the stored project identity before the gate
    // binds it (the gate's journal filtering keys on project_id).
    let mut probe = ProjectController::new(
        Project::new(ProjectId::new(0), ""),
        Box::new(CounterIdGenerator::new(0)),
    );
    probe
        .load(&store)
        .map_err(|e| CliError::Setup(format!("cannot load project snapshot: {e}")))?;
    let project = probe.live().clone();
    let project_id = project.project_id;
    let revision = probe.live_revision();

    let clock: Arc<dyn PolicyClock> = Arc::new(SystemClock);
    let source = Arc::new(MemorySnapshotSource::new());
    source.commit(project_id, revision, project.clone());
    let oracle: Arc<dyn ResourceOracle> = Arc::new(ProjectOracle::of(project.clone()));
    let proposals = Arc::new(ProposalStore::new(
        clock.clone(),
        PROPOSAL_TTL_MS,
        APPROVAL_TTL_MS,
        PrepareBudget::default().max_proposals,
    ));

    let session_store = open_session_store(session_store)?;
    let ctx = BootContext {
        canonical,
        store,
        project,
        project_id,
        revision,
        clock,
        source,
        proposals,
        oracle,
    };
    if durable {
        boot_durable(ctx, session_store)
    } else {
        boot_readonly(ctx, session_store)
    }
}

/// Read-only boot: in-memory policy, no commit gate — mutation
/// requests fail closed inside the gateway.
fn boot_readonly(ctx: BootContext, session_store: FileCliSessionStore) -> Result<Booted, CliError> {
    let binding = ctx.binding();
    let policy: Arc<dyn PolicyStore> = Arc::new(InMemoryPolicyStore::new());
    let (gateway, _variant) =
        build_gateway(policy, &ctx.clock, ctx.source, &ctx.proposals, &ctx.oracle).map_err(
            |e| CliError::Internal(format!("gateway composition failed: {}", e.message)),
        )?;
    let descriptors = snapshot_descriptors(&gateway);
    let (registry, script) = capture_run_artifacts(&gateway)?;
    let committer: Arc<dyn CommitPort> = Arc::new(NoCommitPort);
    let admin: Arc<dyn AdminPort> = Arc::new(NoAdminPort);
    let (gateway, cli, host) =
        shared_handles(gateway, ctx.clock, committer.clone(), admin, session_store);
    Ok(Booted {
        host,
        descriptors,
        project: Some(binding),
        gateway,
        cli,
        committer,
        registry,
        script,
    })
}

/// Durable boot: policy log + commit journal inside the project root,
/// real `CommitGate` as the commit/admin boundary.
fn boot_durable(
    mut ctx: BootContext,
    session_store: FileCliSessionStore,
) -> Result<Booted, CliError> {
    let binding = ctx.binding();
    let (policy_file, _policy_open) = FilePolicyStore::open(ctx.canonical.join(POLICY_LOG_NAME))
        .map_err(|e| CliError::Setup(format!("cannot open policy store: {e}")))?;
    let policy_file = Arc::new(policy_file);
    let policy: Arc<dyn PolicyStore> = policy_file.clone();
    let admin_policy: Arc<dyn PolicyAdmin> = policy_file.clone();

    // The gateway composes first so GateDeps can name the registered
    // `synvelis.proposal.apply` descriptor — the gate never redefines
    // a capability.
    let (gateway, variant) = build_gateway(
        policy.clone(),
        &ctx.clock,
        ctx.source,
        &ctx.proposals,
        &ctx.oracle,
    )
    .map_err(|e| CliError::Internal(format!("gateway composition failed: {}", e.message)))?;
    let descriptors = snapshot_descriptors(&gateway);
    let (registry, script) = capture_run_artifacts(&gateway)?;
    let apply_capability = apply_descriptor(&descriptors)?;

    // The gate's commit-stage evaluator reads the same pinned oracle
    // the engines were built with.
    let evaluator = GrantPolicy::new(policy.clone(), ctx.clock.clone(), ctx.oracle.clone());
    let deps = GateDeps {
        policy,
        admin: admin_policy,
        evaluator,
        proposals: ctx.proposals.clone(),
        apply_capability,
        variant: Some(variant),
    };
    let id_seed = ctx.project.id_watermark;
    let (gate, _report) = CommitGate::open(
        ctx.project,
        Box::new(CounterIdGenerator::new(id_seed)),
        &mut ctx.store,
        ctx.canonical.join(COMMIT_JOURNAL_NAME),
        deps,
        MutexFence::new(),
    )
    .map_err(|e| CliError::Setup(format!("cannot open commit gate: {e}")))?;
    let gate = Arc::new(gate);
    let committer: Arc<dyn CommitPort> = gate.clone();
    let admin: Arc<dyn AdminPort> = gate;

    let (gateway, cli, host) =
        shared_handles(gateway, ctx.clock, committer.clone(), admin, session_store);
    Ok(Booted {
        host,
        descriptors,
        project: Some(binding),
        gateway,
        cli,
        committer,
        registry,
        script,
    })
}
