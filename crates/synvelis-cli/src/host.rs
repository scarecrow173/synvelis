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
//!   verify against the session store and fail closed (the store is
//!   empty in this slice — no OS credential issuer exists yet).
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
use std::sync::Arc;

use serde_json::Value;
use synvelis_gateway::capability::CapabilityDescriptor;
use synvelis_gateway::cli::{CLI_ADAPTER_RAW, CliSessionClaim, MemoryCliSessionStore};
use synvelis_gateway::clock::SystemClock;
use synvelis_gateway::commit::{CommitGate, GateDeps, MutexFence, PolicyAdmin};
use synvelis_gateway::context::{ContextBudget, NoSelectionSource};
use synvelis_gateway::context_engine::ContextProjectionEngine;
use synvelis_gateway::durable_policy::FilePolicyStore;
use synvelis_gateway::gateway::Gateway;
use synvelis_gateway::grant::ResourceOracle;
use synvelis_gateway::headless::HeadlessSessionHost;
use synvelis_gateway::id::AdapterId;
use synvelis_gateway::mcp_server::{AdminPort, CommitPort};
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
use synvelis_gateway::{GatewayOutcome, PolicyClock};
use synvelis_project::{
    CounterIdGenerator, DirectoryBundleStore, Project, ProjectController, ProjectId,
    ProjectRevision, head_revision,
};

use crate::args::Args;
use crate::error::CliError;

/// TTLs mirroring the gateway test-rig composition (proposal 60s,
/// approval 30s). Proposals/approvals are in-memory this slice — see
/// `evidence/sv262/` for the durable-store residual.
const PROPOSAL_TTL_MS: u64 = 60_000;
const APPROVAL_TTL_MS: u64 = 30_000;

/// Filenames of the durable seams inside the project root.
const POLICY_LOG_NAME: &str = "synvelis-cli.policy.log";
const COMMIT_JOURNAL_NAME: &str = "synvelis-cli.commit.journal";

/// A booted session: the headless host plus the registry snapshot the
/// CLI captured before the gateway moved into the host's mutex.
pub struct Booted {
    /// The session host — `handle_claim` is the only request entry.
    pub host: HeadlessSessionHost,
    /// Capability descriptors as registered at boot (discovery
    /// metadata — grants nothing).
    pub descriptors: Vec<CapabilityDescriptor>,
    /// The bound project identity/revision, when project-bound.
    pub project: Option<ProjectBinding>,
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
/// `--session-credential` selects `Interactive` (verified against the
/// store — currently always fails closed); everything else is
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
) -> Result<(), ErrorEnvelope> {
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
    let prepare_policy: Arc<dyn PrepareAuthorizer> =
        Arc::new(PreparePolicy::new(policy.clone(), Arc::clone(clock)));
    let prepare = Arc::new(PrepareEngine::new(
        ctx.clone(),
        prepare_policy.clone(),
        PrepareBudget::default(),
        Arc::clone(proposals),
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
    Ok(())
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
) -> Result<Gateway, ErrorEnvelope> {
    let mut gateway = Gateway::with_evaluator(Box::new(ProposalGate::new(
        Arc::clone(proposals),
        Box::new(GrantPolicy::new(
            policy.clone(),
            Arc::clone(clock),
            Arc::clone(oracle),
        )),
    )));
    install_engines(&mut gateway, policy, clock, source, proposals, oracle)?;
    Ok(gateway)
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

fn apply_descriptor(
    descriptors: &[CapabilityDescriptor],
) -> Result<CapabilityDescriptor, CliError> {
    descriptors
        .iter()
        .find(|d| d.id.as_str() == APPLY_ID)
        .cloned()
        .ok_or_else(|| CliError::Internal("synvelis.proposal.apply not registered".to_owned()))
}

// ── boots ────────────────────────────────────────────────────────

/// Boot for discovery commands (`action list`/`action describe`):
/// the full engine stack over empty in-memory seams — no project, no
/// durable files, no session state.
///
/// # Errors
/// [`CliError::Internal`] on registration conflicts (cannot happen with
/// the fixed engine set — surfaces as a bug, not user error).
pub fn boot_discovery() -> Result<Booted, CliError> {
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
    let gateway = build_gateway(store, &clock, source, &proposals, &oracle)
        .map_err(|e| CliError::Internal(format!("gateway composition failed: {}", e.message)))?;
    let descriptors = snapshot_descriptors(&gateway);
    let host = HeadlessSessionHost::without_commit_boundary(
        gateway,
        Box::new(MemoryCliSessionStore::new()),
        clock,
    );
    Ok(Booted {
        host,
        descriptors,
        project: None,
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
pub fn boot_project(root: &Path, durable: bool) -> Result<Booted, CliError> {
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
        boot_durable(ctx)
    } else {
        boot_readonly(ctx)
    }
}

/// Read-only boot: in-memory policy, no commit gate — mutation
/// requests fail closed inside the gateway.
fn boot_readonly(ctx: BootContext) -> Result<Booted, CliError> {
    let binding = ctx.binding();
    let policy: Arc<dyn PolicyStore> = Arc::new(InMemoryPolicyStore::new());
    let gateway = build_gateway(policy, &ctx.clock, ctx.source, &ctx.proposals, &ctx.oracle)
        .map_err(|e| CliError::Internal(format!("gateway composition failed: {}", e.message)))?;
    let descriptors = snapshot_descriptors(&gateway);
    let host = HeadlessSessionHost::without_commit_boundary(
        gateway,
        Box::new(MemoryCliSessionStore::new()),
        ctx.clock,
    );
    Ok(Booted {
        host,
        descriptors,
        project: Some(binding),
    })
}

/// Durable boot: policy log + commit journal inside the project root,
/// real `CommitGate` as the commit/admin boundary.
fn boot_durable(mut ctx: BootContext) -> Result<Booted, CliError> {
    let binding = ctx.binding();
    let (policy_file, _policy_open) = FilePolicyStore::open(ctx.canonical.join(POLICY_LOG_NAME))
        .map_err(|e| CliError::Setup(format!("cannot open policy store: {e}")))?;
    let policy_file = Arc::new(policy_file);
    let policy: Arc<dyn PolicyStore> = policy_file.clone();
    let admin_policy: Arc<dyn PolicyAdmin> = policy_file.clone();

    // The gateway composes first so GateDeps can name the registered
    // `synvelis.proposal.apply` descriptor — the gate never redefines
    // a capability.
    let gateway = build_gateway(
        policy.clone(),
        &ctx.clock,
        ctx.source,
        &ctx.proposals,
        &ctx.oracle,
    )
    .map_err(|e| CliError::Internal(format!("gateway composition failed: {}", e.message)))?;
    let descriptors = snapshot_descriptors(&gateway);
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

    let host = HeadlessSessionHost::new(
        gateway,
        Box::new(MemoryCliSessionStore::new()),
        committer,
        admin,
        ctx.clock,
        AdapterId::new(CLI_ADAPTER_RAW),
    );
    Ok(Booted {
        host,
        descriptors,
        project: Some(binding),
    })
}
