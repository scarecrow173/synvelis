//! CB-10 (synvelis#287): the typed Creative Branch gateway contract.
//!
//! Branch mutations (`fork`/`edit`/`adopt`/`discard`) are *not* a
//! second authorization path: they ride the same
//! prepare → proposal → approval → apply → [`crate::commit::CommitGate`]
//! pipeline as every other `ProjectMutation` capability. The wire layer
//! lowers each capability to a [`VariantIntent`]; preparation runs the
//! domain engine on a scratch controller/store so the retained
//! candidate, derived effects, and preview are the *real* post-state —
//! never a declared approximation. Commit re-executes the same intent
//! inside the gate's single-writer fence against the live controller
//! and the shared content store.
//!
//! Authority boundaries preserved here:
//!
//! - Candidate generation (`edit` on a non-adopted branch) and live
//!   materialization (`adopt`) are separate capabilities — the
//!   capability floor forces a grant to name each one it uses.
//! - `adopt` additionally consumes the commit-stage approval receipt:
//!   the domain contract requires an explicit approval token and the
//!   gateway cannot mint one, so an adopt under a `DelegatedApply`
//!   grant (no consumed approval) fails closed with `Unapproved`.
//! - Runtime publish stays receipt evidence only — a committed project
//!   with a pending/failed publish is reported honestly, never as a
//!   runtime success (CB-MODEL-08).
//! - Query capabilities (`list`/`describe`/`compare`) read pinned
//!   snapshots only; `compare` decodes candidate payloads through the
//!   content store and re-derives scope digests rather than trusting
//!   recorded metadata.

#![allow(clippy::result_large_err)] // typed envelopes are the contract

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, PoisonError};

use serde_json::{Value, json};
use synvelis_project::snapshot::{BlobProbe, ContentDigest, SnapshotCancel, SnapshotResolver};
use synvelis_project::variant::{
    AdoptIntent, AdoptSources, AudioClipAdapter, BranchContentRef, BranchContentStore,
    BranchEditOp, BranchEditOpIntent, DomainAdapter, InstrumentSlotAdapter, MidiClipAdapter,
    NoopRuntimePublish, ProcessingRangeAdapter, RuntimePublish, StripFieldsAdapter,
    TakeUsageAdapter, VariantError, VariantScope, adopt_branch_adapted, decode_candidate,
    discard_branch, edit_branch_op, fork_variant_set, scope_digest,
};
use synvelis_project::{
    ApplyApproval, BranchId, CounterIdGenerator, PluginAvailability, Project, ProjectController,
    VariantSetId,
};

use crate::capability::{
    Availability, CapabilityDescriptor, CapabilityId, CapabilityVersion, DataAccessClass,
    DataAccessSet, EffectKind, EffectSet, ExecutionContract, OperationDomain, PermissionKind,
    PermissionSet, TargetContract,
};
use crate::context_engine::ContextProjectionEngine;
use crate::outcome::{ErrorEnvelope, GatewayError, GatewayOutcome};
use crate::prepare::OpFacts;
use crate::registry::CapabilityOperation;
use crate::request::{OperationRequest, OperationTarget, ResourceKind, ResourceRef};
use crate::schema::{SchemaRef, ValueSchema};

// ── Capability ids ──────────────────────────────────────────────

/// `synvelis.branch.fork` — create a variant set + adopted/candidate
/// branch pair over a typed scope.
pub const FORK_ID: &str = "synvelis.branch.fork";
/// `synvelis.branch.edit` — edit one branch (adopted → live mirror,
/// candidate → immutable payload).
pub const EDIT_ID: &str = "synvelis.branch.edit";
/// `synvelis.branch.adopt` — materialize a candidate branch into live.
pub const ADOPT_ID: &str = "synvelis.branch.adopt";
/// `synvelis.branch.discard` — remove a branch from its set.
pub const DISCARD_ID: &str = "synvelis.branch.discard";
/// `synvelis.branch.list` — enumerate variant sets on a pinned base.
pub const LIST_ID: &str = "synvelis.branch.list";
/// `synvelis.branch.describe` — full descriptor for one variant set.
pub const DESCRIBE_ID: &str = "synvelis.branch.describe";
/// `synvelis.branch.compare` — verified content comparison of two
/// branches inside one set.
pub const COMPARE_ID: &str = "synvelis.branch.compare";

/// The shared prepare-op output schema — the same loose object every
/// `ProjectMutation` semantic op reports through (registered by
/// `PrepareEngine::install`).
const OP_OUTPUT: &str = "synvelis.operation.output";

fn input_schema_name(id: &str) -> String {
    format!("{id}.input")
}

fn output_schema_name(id: &str) -> String {
    format!("{id}.output")
}

// ── Scope kinds ─────────────────────────────────────────────────

/// The scope-family discriminator — one domain adapter per kind.
/// `Arrangement` has no adapter in this slice: fork/edit/adopt over
/// it fails closed at adapter resolution.
#[derive(Copy, Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum VariantScopeKind {
    /// One audio clip scope.
    AudioClip,
    /// Comping take-usage scope.
    TakeUsage,
    /// One MIDI clip scope.
    MidiClip,
    /// One instrument-slot scope.
    InstrumentSlot,
    /// One processor-range scope.
    ProcessingRange,
    /// One strip-fields scope.
    StripFields,
    /// Musical-time arrangement selection (no adapter yet).
    Arrangement,
}

impl VariantScopeKind {
    /// The kind a scope value belongs to.
    #[must_use]
    pub fn of(scope: &VariantScope) -> Self {
        match scope {
            VariantScope::AudioClip { .. } => Self::AudioClip,
            VariantScope::TakeUsage { .. } => Self::TakeUsage,
            VariantScope::MidiClip { .. } => Self::MidiClip,
            VariantScope::InstrumentSlot { .. } => Self::InstrumentSlot,
            VariantScope::ProcessingRange { .. } => Self::ProcessingRange,
            VariantScope::StripFields { .. } => Self::StripFields,
            VariantScope::Arrangement { .. } => Self::Arrangement,
        }
    }

    /// The kebab-case wire name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::AudioClip => "audio-clip",
            Self::TakeUsage => "take-usage",
            Self::MidiClip => "midi-clip",
            Self::InstrumentSlot => "instrument-slot",
            Self::ProcessingRange => "processing-range",
            Self::StripFields => "strip-fields",
            Self::Arrangement => "arrangement",
        }
    }

    /// Parses a wire scope-kind name.
    ///
    /// # Errors
    /// [`GatewayError::MalformedRequest`] on an unknown name.
    pub fn parse(text: &str) -> Result<Self, GatewayError> {
        Ok(match text {
            "audio-clip" => Self::AudioClip,
            "take-usage" => Self::TakeUsage,
            "midi-clip" => Self::MidiClip,
            "instrument-slot" => Self::InstrumentSlot,
            "processing-range" => Self::ProcessingRange,
            "strip-fields" => Self::StripFields,
            "arrangement" => Self::Arrangement,
            other => {
                return Err(GatewayError::MalformedRequest {
                    reason: format!("unknown variant scope kind '{other}'"),
                });
            }
        })
    }
}

// ── The prepared mutation ──────────────────────────────────────

/// What a retained proposal will execute at the commit boundary.
///
/// `Project` is the ordinary `ProjectOp` path; `Variant` carries the
/// typed branch intent the [`crate::commit::CommitGate`] re-executes
/// inside its fence through [`VariantPorts`]. The variant arm never
/// bypasses authorization: preparation simulated the intent on a
/// scratch controller so the retained effects are the real diff.
#[derive(Clone, Debug)]
pub enum VariantIntent {
    /// `synvelis.branch.fork` — create a set over `scope`.
    Fork {
        /// The typed scope the set branches over.
        scope: VariantScope,
    },
    /// `synvelis.branch.edit` — edit one branch.
    Edit {
        /// Set/branch/CAS plus the lowered edit payload.
        intent: BranchEditOpIntent,
    },
    /// `synvelis.branch.adopt` — materialize a candidate into live.
    /// `intent.approval.ticket` is a placeholder at preparation; the
    /// commit gate substitutes the consumed approval receipt's id —
    /// the domain contract rejects empty tickets and the gateway
    /// never mints approval authority itself.
    Adopt {
        /// Set/branch/CAS surface (approval injected at commit).
        intent: AdoptIntent,
    },
    /// `synvelis.branch.discard` — remove a branch from its set.
    Discard {
        /// Set the branch belongs to.
        variant_set: VariantSetId,
        /// Branch to discard.
        branch: BranchId,
    },
}

impl VariantIntent {
    /// The capability id this intent was lowered from — recorded on
    /// the semantic command so the proposal's capability floor names
    /// the branch capability the grant must cover.
    #[must_use]
    pub const fn capability_id(&self) -> &'static str {
        match self {
            Self::Fork { .. } => FORK_ID,
            Self::Edit { .. } => EDIT_ID,
            Self::Adopt { .. } => ADOPT_ID,
            Self::Discard { .. } => DISCARD_ID,
        }
    }
}

// ── Ports ───────────────────────────────────────────────────────

/// Shared branch-content storage — `Arc<Mutex<..>>` so preparation
/// (read-through scratch) and the commit fence (real writes) share one
/// backend without a second authority path. `Send` on the payload is
/// the mutex's own guarantee; the store never crosses a thread
/// boundary unlocked.
pub type SharedBranchStore = Arc<Mutex<Box<dyn BranchContentStore + Send>>>;

/// The domain dependencies a variant mutation needs — held by the
/// commit gate as a declared dependency and by the prepare engine for
/// scratch simulation. Adapters are per-scope-kind; a missing adapter
/// fails the operation closed, never silently.
pub struct VariantPorts {
    /// Domain adapter per scope kind (`Arrangement` intentionally
    /// absent — no adapter exists for it yet).
    pub adapters: BTreeMap<VariantScopeKind, Arc<dyn DomainAdapter + Send + Sync>>,
    /// The shared candidate-content store.
    pub store: SharedBranchStore,
    /// Snapshot identity resolver for fork origins.
    pub resolver: SnapshotResolver,
    /// Runtime publish port — invoked by `adopt` *after* the project
    /// CAS; its evidence is receipt data, never project truth.
    pub runtime: Arc<dyn RuntimePublish + Send + Sync>,
    /// Optional asset-availability probe for adopt preflight — `None`
    /// fails closed on any probed digest (`AdoptSources` contract).
    pub assets: Option<Arc<dyn BlobProbe + Send + Sync>>,
    /// Optional plugin-availability probe for adopt preflight.
    pub plugins: Option<Arc<dyn PluginAvailability + Send + Sync>>,
}

impl VariantPorts {
    /// The production wiring: every adapter this slice ships, the
    /// snapshot resolver, optional asset/plugin probes, and — when
    /// `content_root` names a project directory — the durable
    /// [`synvelis_project::variant::DirectoryBranchContentStore`]
    /// under `<root>/branch-content` plus the journaled runtime port
    /// under `<root>/runtime-publish` (synvelis#517). `None` is for
    /// project-less contexts only (capability discovery, tests that
    /// never commit): it keeps the in-memory store and reports
    /// `NotRequested` publishes, which is honest only because no
    /// project exists to commit. An absent adapter for a scope kind
    /// still fails closed.
    #[must_use]
    pub fn standard(content_root: Option<&std::path::Path>) -> Self {
        let mut adapters: BTreeMap<VariantScopeKind, Arc<dyn DomainAdapter + Send + Sync>> =
            BTreeMap::new();
        adapters.insert(
            VariantScopeKind::AudioClip,
            Arc::new(AudioClipAdapter::new()),
        );
        adapters.insert(
            VariantScopeKind::TakeUsage,
            Arc::new(TakeUsageAdapter::new()),
        );
        adapters.insert(VariantScopeKind::MidiClip, Arc::new(MidiClipAdapter::new()));
        adapters.insert(
            VariantScopeKind::InstrumentSlot,
            Arc::new(InstrumentSlotAdapter::new()),
        );
        adapters.insert(
            VariantScopeKind::ProcessingRange,
            Arc::new(ProcessingRangeAdapter::new()),
        );
        adapters.insert(
            VariantScopeKind::StripFields,
            Arc::new(StripFieldsAdapter::new()),
        );
        let (store, runtime): (SharedBranchStore, Arc<dyn RuntimePublish + Send + Sync>) =
            match content_root {
                Some(root) => (
                    Arc::new(Mutex::new(Box::new(
                        synvelis_project::variant::DirectoryBranchContentStore::new(
                            root.join("branch-content"),
                        ),
                    )
                        as Box<dyn BranchContentStore + Send>)),
                    Arc::new(synvelis_project::variant::JournaledRuntimePublish::new(
                        root.join("runtime-publish"),
                    )),
                ),
                None => (
                    Arc::new(Mutex::new(Box::new(
                        synvelis_project::variant::InMemoryBranchContentStore::default(),
                    )
                        as Box<dyn BranchContentStore + Send>)),
                    Arc::new(NoopRuntimePublish),
                ),
            };
        Self {
            adapters,
            store,
            resolver: SnapshotResolver::new(),
            runtime,
            assets: None,
            plugins: None,
        }
    }

    /// The adapter for `scope`'s kind — kinds without an adapter
    /// (e.g. `Arrangement`) fail closed.
    fn adapter_for(
        &self,
        scope: &VariantScope,
    ) -> Result<&Arc<dyn DomainAdapter + Send + Sync>, VariantError> {
        self.adapters
            .get(&VariantScopeKind::of(scope))
            .ok_or_else(|| VariantError::Unsupported {
                detail: format!(
                    "no domain adapter wired for {} scopes",
                    VariantScopeKind::of(scope).name()
                ),
            })
    }

    /// The adapter owning `set`'s scope — the set lookup runs against
    /// the controller's live project.
    fn adapter_for_set(
        &self,
        controller: &ProjectController,
        variant_set: &VariantSetId,
    ) -> Result<&Arc<dyn DomainAdapter + Send + Sync>, VariantError> {
        let set = controller
            .live()
            .variant_sets
            .get(variant_set)
            .ok_or_else(|| {
                VariantError::Project(synvelis_project::Error::NotFound(format!(
                    "variant-set {}",
                    variant_set.to_tagged()
                )))
            })?;
        self.adapter_for(&set.scope)
    }
}

// ── Commit-side error mapping ───────────────────────────────────

/// A determined variant-mutation failure at the commit fence —
/// mapped onto the gate's non-commit outcomes. Nothing here is
/// indeterminate: the domain engine either committed or it did not.
#[derive(Debug)]
pub enum VariantCommitError {
    /// A determined rejection (stale CAS other than the project
    /// revision, scope/lineage/adapter/store/approval failures).
    Rejected {
        /// Typed reason — the `VariantError` discriminant plus detail.
        reason: String,
    },
    /// The project-revision CAS lost — the live revision moved past
    /// the prepared base. Maps to the gate's `Conflicted` outcome.
    Conflicted {
        /// Revision the intent expected.
        expected: u64,
        /// Revision actually live.
        actual: u64,
    },
}

impl From<VariantError> for VariantCommitError {
    fn from(e: VariantError) -> Self {
        match e {
            VariantError::StaleProjectRevision { expected, actual }
            | VariantError::Project(synvelis_project::Error::StaleRevision { expected, actual }) => {
                Self::Conflicted { expected, actual }
            }
            other => Self::Rejected {
                reason: variant_reason(&other),
            },
        }
    }
}

/// Stable `kebab-case` reason prefix per `VariantError` discriminant —
/// journal/provenance readers match on this, never on prose.
pub(crate) fn variant_reason(e: &VariantError) -> String {
    let (kind, detail) = match e {
        VariantError::Project(err) => ("project", err.to_string()),
        VariantError::Snapshot(err) => ("snapshot", err.to_string()),
        VariantError::InvalidScope { detail } => ("invalid-scope", detail.clone()),
        VariantError::ScopeConflict { detail, .. } => ("scope-conflict", detail.clone()),
        VariantError::StaleBranchRevision { expected, actual } => (
            "stale-branch-revision",
            format!("expected {expected}, actual {actual}"),
        ),
        VariantError::StaleScope { .. } => ("stale-scope", e.to_string()),
        VariantError::StaleProjectRevision { .. } => ("stale-project-revision", e.to_string()),
        VariantError::CandidateUnavailable { detail } => ("candidate-unavailable", detail.clone()),
        VariantError::Unsupported { detail } => ("unsupported", detail.clone()),
        VariantError::AmbiguousLineage { detail } => ("ambiguous-lineage", detail.clone()),
        VariantError::ScopeViolation { detail } => ("scope-violation", detail.clone()),
        VariantError::RecordingActive { detail } => ("recording-active", detail.clone()),
        VariantError::Unapproved => (
            "unapproved",
            "adopt requires an approval receipt".to_owned(),
        ),
        VariantError::MissingAsset { digest } => ("missing-asset", digest.clone()),
        VariantError::AssetUnknown { digest } => ("asset-unknown", digest.clone()),
        VariantError::MissingPlugin { detail } => ("missing-plugin", detail.clone()),
        VariantError::InvalidPort { detail } => ("invalid-port", detail.clone()),
        VariantError::Store { detail } => ("store", detail.clone()),
    };
    format!("variant-{kind}: {detail}")
}

// ── Intent execution ────────────────────────────────────────────

/// Ticket used for prepare-time simulation — a placeholder, never
/// persisted: simulation runs on a scratch controller whose state is
/// discarded, and adopt's ticket is replaced by the real approval
/// receipt at commit. The prepare engine stamps this into the frozen
/// intent; [`execute`] overwrites it with the consumed approval id.
pub(crate) const SIMULATION_TICKET: &str = "prepare-simulation";

/// Runs `intent` against `controller` with caller-supplied ports —
/// the single dispatch both preparation (scratch) and commit (live)
/// share, so preview and commit can never diverge in *what* executes.
///
/// Returns the domain receipt as provenance JSON — project-commit and
/// runtime-publish evidence stay separate blocks (CB-MODEL-08).
fn run_intent(
    controller: &mut ProjectController,
    ports: &VariantPorts,
    store: &mut dyn BranchContentStore,
    intent: &VariantIntent,
    approval_ticket: &str,
    runtime: &dyn RuntimePublish,
    resolver: &SnapshotResolver,
) -> Result<Value, VariantError> {
    match intent {
        VariantIntent::Fork { scope } => {
            let adapter = ports.adapter_for(scope)?;
            let receipt = fork_variant_set(
                controller,
                adapter.as_ref(),
                store,
                resolver,
                scope.clone(),
                &SnapshotCancel::new(),
            )?;
            Ok(json!({
                "variant_set": receipt.variant_set.to_tagged(),
                "adopted": receipt.adopted.to_tagged(),
                "candidate": receipt.candidate.to_tagged(),
                "project_revision": receipt.project_revision.0,
            }))
        }
        VariantIntent::Edit { intent } => {
            let adapter = ports.adapter_for_set(controller, &intent.variant_set)?;
            let receipt = edit_branch_op(controller, adapter.as_ref(), store, intent)?;
            Ok(json!({
                "variant_set": receipt.variant_set.to_tagged(),
                "branch": receipt.branch.to_tagged(),
                "content_revision": receipt.content_revision.map(synvelis_project::variant::BranchRevision::raw),
                "project_revision": receipt.project_revision.0,
            }))
        }
        VariantIntent::Adopt { intent } => {
            let adapter = ports.adapter_for_set(controller, &intent.variant_set)?;
            let mut intent = intent.clone();
            intent.approval = ApplyApproval {
                ticket: approval_ticket.to_owned(),
            };
            let sources = AdoptSources {
                assets: ports
                    .assets
                    .as_deref()
                    .map(|probe| -> &dyn BlobProbe { probe }),
                plugins: ports
                    .plugins
                    .as_deref()
                    .map(|probe| -> &dyn PluginAvailability { probe }),
                runtime,
            };
            let receipt =
                adopt_branch_adapted(controller, adapter.as_ref(), store, &sources, &intent)?;
            // `AdoptReceipt` is `Serialize` — the project/runtime
            // evidence split survives verbatim into provenance.
            serde_json::to_value(&receipt).map_err(|e| VariantError::Store {
                detail: format!("adopt receipt encode: {e}"),
            })
        }
        VariantIntent::Discard {
            variant_set,
            branch,
        } => {
            let receipt = discard_branch(controller, *variant_set, *branch)?;
            Ok(json!({
                "variant_set": receipt.variant_set.to_tagged(),
                "branch": receipt.branch.to_tagged(),
                "project_revision": receipt.project_revision.0,
            }))
        }
    }
}

/// Prepare-time simulation — executes `intent` on a private scratch
/// controller seeded at `base`'s watermark, with a read-through store
/// so candidate payloads resolve but no bytes are written to the
/// shared backend and runtime publish is never requested. Returns the
/// deterministic post-state the proposal previews and authorizes.
///
/// # Errors
/// [`VariantError`] — the same typed failures commit would produce,
/// surfaced as preparation rejections.
pub fn simulate(
    base: &Project,
    ports: &VariantPorts,
    intent: &VariantIntent,
) -> Result<Project, VariantError> {
    let mut controller = ProjectController::new(
        base.clone(),
        Box::new(CounterIdGenerator::new(base.id_watermark)),
    );
    // A fresh resolver keeps snapshot generations deterministic for
    // the preview; commit mints the authoritative identity.
    let resolver = SnapshotResolver::new();
    let noop = NoopRuntimePublish;
    let mut scratch = ScratchStore {
        shared: &ports.store,
        local: BTreeMap::new(),
    };
    // The scratch controller's commit counter is not the live one —
    // project-revision CAS is a commit-time concern the gate re-checks
    // inside the fence. Pin the simulation's expected revision to the
    // scratch tip so the remaining CAS surfaces (branch revision,
    // scope digest) are still verified honestly.
    let sim_intent;
    let intent = match intent {
        VariantIntent::Adopt { intent } => {
            let mut patched = intent.clone();
            patched.expected_project_revision = controller.live_revision();
            sim_intent = VariantIntent::Adopt { intent: patched };
            &sim_intent
        }
        _ => intent,
    };
    run_intent(
        &mut controller,
        ports,
        &mut scratch,
        intent,
        SIMULATION_TICKET,
        &noop,
        &resolver,
    )?;
    Ok(controller.live().clone())
}

/// Commit-time execution — the same [`run_intent`] dispatch against
/// the gate's live controller, the shared store, the wired runtime
/// port, and the consumed approval receipt's id as the adopt ticket.
///
/// # Errors
/// [`VariantCommitError`] — determined rejection or project-CAS
/// conflict; never indeterminate.
pub fn execute(
    controller: &mut ProjectController,
    ports: &VariantPorts,
    intent: &VariantIntent,
    approval_ticket: &str,
) -> Result<Value, VariantCommitError> {
    let mut store = ports.store.lock().unwrap_or_else(PoisonError::into_inner);
    run_intent(
        controller,
        ports,
        &mut **store,
        intent,
        approval_ticket,
        ports.runtime.as_ref(),
        &ports.resolver,
    )
    .map_err(VariantCommitError::from)
}

/// Read-through scratch store for prepare simulation: `get` resolves
/// local writes first, then the shared backend; `put` stays local so
/// simulation can never orphan or overwrite committed payloads.
struct ScratchStore<'a> {
    shared: &'a SharedBranchStore,
    local: BTreeMap<ContentDigest, Vec<u8>>,
}

impl BranchContentStore for ScratchStore<'_> {
    fn put(&mut self, digest: &ContentDigest, bytes: &[u8]) -> Result<(), VariantError> {
        self.local.insert(*digest, bytes.to_vec());
        Ok(())
    }

    fn get(&self, digest: &ContentDigest) -> Result<Option<Vec<u8>>, VariantError> {
        if let Some(bytes) = self.local.get(digest) {
            return Ok(Some(bytes.clone()));
        }
        self.shared
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(digest)
    }
}

// ── Declared facts (provisional gate) ───────────────────────────

/// The resources a [`VariantScope`] names directly — scope *roots*,
/// not the derived owned-object closure (`scope_objects` is
/// crate-private to synvelis-project). The post-simulation diff is the
/// authoritative effect record; these roots give the provisional
/// gate an honest early coverage check.
fn scope_roots(scope: &VariantScope) -> Vec<ResourceRef> {
    match scope {
        VariantScope::AudioClip { clip } | VariantScope::MidiClip { clip } => {
            vec![ResourceRef::Clip(*clip)]
        }
        VariantScope::TakeUsage { target } => vec![ResourceRef::TakeGroup(target.take_group)],
        VariantScope::InstrumentSlot { processor } => vec![ResourceRef::Processor(*processor)],
        VariantScope::ProcessingRange { range } => vec![
            ResourceRef::Processor(range.start),
            ResourceRef::Processor(range.end),
        ],
        VariantScope::StripFields { strip, .. } => vec![ResourceRef::Strip(*strip)],
        VariantScope::Arrangement { tracks, .. } => {
            tracks.iter().map(|t| ResourceRef::Track(*t)).collect()
        }
    }
}

/// Semantic facts for the provisional gate — computed *before*
/// simulation so an absent target and an out-of-scope target deny
/// identically (SEC-03 existence blinding), matching where
/// `op_facts` sits for ordinary ops.
///
/// `reads` covers the set, its branches, the project aggregate, the
/// scope roots, and (for `edit`) the inner op's read set. `impacted`
/// covers the set + target branch; for an *adopted*-branch edit the
/// inner op's impacted set joins too, because that edit writes live —
/// for a candidate edit those resources are reads only (the write
/// lands in the payload, and the post-simulation diff records only
/// the bookkeeping writes).
pub(crate) fn declared_facts(intent: &VariantIntent, project: &Project) -> OpFacts {
    let mut reads = BTreeSet::new();
    let mut impacted: BTreeMap<ResourceRef, BTreeSet<ResourceRef>> = BTreeMap::new();

    let mut cover_set = |set_id: &VariantSetId, branch: Option<&BranchId>| {
        reads.insert(ResourceRef::VariantSet(*set_id));
        impacted
            .entry(ResourceRef::VariantSet(*set_id))
            .or_default();
        reads.insert(ResourceRef::Project(project.project_id));
        if let Some(b) = branch {
            reads.insert(ResourceRef::Branch(*b));
            impacted.entry(ResourceRef::Branch(*b)).or_default();
        }
        if let Some(set) = project.variant_sets.get(set_id) {
            for root in scope_roots(&set.scope) {
                reads.insert(root);
            }
        }
    };

    match intent {
        VariantIntent::Fork { scope } => {
            reads.insert(ResourceRef::Project(project.project_id));
            for root in scope_roots(scope) {
                reads.insert(root);
            }
            // The set does not exist yet — its create is authorized by
            // the target contract + the post-simulation diff.
        }
        VariantIntent::Edit { intent } => {
            cover_set(&intent.variant_set, Some(&intent.branch));
            if let BranchEditOp::Project(op) = &intent.op {
                let inner = crate::prepare::op_facts(op, project);
                reads.extend(inner.reads.iter().copied());
                let adopted = crate::snapshot::branch_entry(project, &intent.branch)
                    .is_some_and(|(_, b)| matches!(b.content, BranchContentRef::Live { .. }));
                if adopted {
                    for (resource, linked) in inner.impacted {
                        impacted.entry(resource).or_default().extend(linked);
                    }
                } else {
                    reads.extend(inner.impacted.keys().copied());
                }
            }
        }
        VariantIntent::Adopt { intent } => {
            cover_set(&intent.variant_set, Some(&intent.branch));
            // Adopt materializes candidate content into live — every
            // scope root is a write candidate; the post-simulation
            // diff records the real set.
        }
        VariantIntent::Discard {
            variant_set,
            branch,
        } => {
            cover_set(variant_set, Some(branch));
        }
    }

    OpFacts {
        reads,
        impacted,
        preconditions: Vec::new(),
        unsupported: Vec::new(),
    }
}

// ── Wire lowering ───────────────────────────────────────────────

fn malformed(reason: impl Into<String>) -> GatewayError {
    GatewayError::MalformedRequest {
        reason: reason.into(),
    }
}

fn req_field<'a>(args: &'a Value, name: &str) -> Result<&'a Value, GatewayError> {
    args.get(name)
        .ok_or_else(|| malformed(format!("args.{name} is required")))
}

fn req_u64(args: &Value, name: &str) -> Result<u64, GatewayError> {
    let n = req_field(args, name)?
        .as_i64()
        .ok_or_else(|| malformed(format!("args.{name} must be an integer")))?;
    u64::try_from(n).map_err(|_| malformed(format!("args.{name} must be non-negative")))
}

fn req_typed<T: serde::de::DeserializeOwned>(args: &Value, name: &str) -> Result<T, GatewayError> {
    serde_json::from_value(req_field(args, name)?.clone())
        .map_err(|e| malformed(format!("args.{name}: {e}")))
}

fn req_domain_id<T>(
    args: &Value,
    name: &str,
    parse: fn(&str) -> Result<T, synvelis_project::Error>,
) -> Result<T, GatewayError> {
    let text = req_field(args, name)?
        .as_str()
        .ok_or_else(|| malformed(format!("args.{name} must be a tagged id string")))?;
    parse(text).map_err(|_| malformed(format!("args.{name} is not a valid tagged id")))
}

/// Lowers `synvelis.branch.fork` args → the typed scope.
///
/// # Errors
/// [`GatewayError::MalformedRequest`] on shape/type violations —
/// schema validation has already run, so these are defense-in-depth.
pub fn lower_fork(args: &Value) -> Result<VariantScope, GatewayError> {
    req_typed(args, "scope")
}

/// Lowers `synvelis.branch.edit` args — the set comes from the bound
/// target (single source), `op` is a named operation the prepare
/// engine lowers through the ordinary op table (`ProjectOp` cannot
/// cross the wire as data). Returns `(branch, branch_revision, op)`.
///
/// # Errors
/// [`GatewayError::MalformedRequest`] on shape/type violations.
pub fn lower_edit(args: &Value) -> Result<(BranchId, u64, Value), GatewayError> {
    Ok((
        req_domain_id(args, "branch", BranchId::from_tagged)?,
        req_u64(args, "expected_branch_revision")?,
        req_field(args, "op")?.clone(),
    ))
}

/// Lowers `synvelis.branch.adopt` args — every CAS field is required;
/// the set comes from the bound target and the approval ticket is
/// *not* an arg (it is injected at commit from the consumed approval
/// receipt). Returns `(branch, project_revision, branch_revision,
/// scope_digest)`.
///
/// # Errors
/// [`GatewayError::MalformedRequest`] on shape/type violations.
pub fn lower_adopt(args: &Value) -> Result<(BranchId, u64, u64, ContentDigest), GatewayError> {
    Ok((
        req_domain_id(args, "branch", BranchId::from_tagged)?,
        req_u64(args, "expected_project_revision")?,
        req_u64(args, "expected_branch_revision")?,
        req_typed(args, "expected_scope_digest")?,
    ))
}

/// Lowers `synvelis.branch.discard` args — the set comes from the
/// bound target.
///
/// # Errors
/// [`GatewayError::MalformedRequest`] on shape/type violations.
pub fn lower_discard(args: &Value) -> Result<BranchId, GatewayError> {
    req_domain_id(args, "branch", BranchId::from_tagged)
}

// ── Descriptors ─────────────────────────────────────────────────

#[allow(clippy::too_many_arguments)] // flat descriptor spec table
fn descriptor(
    id: &str,
    domain: OperationDomain,
    input: &str,
    output: &str,
    target: TargetContract,
    effects: &[EffectKind],
    permissions: &[PermissionKind],
    data_access: &[DataAccessClass],
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: CapabilityId::new(id).expect("capability id is valid"),
        version: CapabilityVersion::new(1, 0),
        domain,
        input_schema: SchemaRef::new(input.to_owned(), 1, 0),
        output_schema: SchemaRef::new(output.to_owned(), 1, 0),
        target,
        effect_bound: EffectSet::of(effects.iter().copied()),
        required_permissions: PermissionSet::of(permissions.iter().copied()),
        data_access: DataAccessSet::of(data_access.iter().copied()),
        execution: ExecutionContract::Sync,
        availability: Availability::Available,
    }
}

/// The four mutation descriptors — registered on the prepare engine's
/// variant-op table (no direct executor: they are invocable only
/// *through* `synvelis.operation.prepare`, like every `ProjectOp`).
#[must_use]
pub fn mutation_descriptors() -> Vec<CapabilityDescriptor> {
    use EffectKind::{Create, Delete, Read, Write};
    vec![
        descriptor(
            FORK_ID,
            OperationDomain::ProjectMutation,
            &input_schema_name(FORK_ID),
            OP_OUTPUT,
            TargetContract::Create {
                parent: ResourceKind::Project,
                creates: ResourceKind::VariantSet,
            },
            &[Read, Write, Create],
            &[PermissionKind::MutateProject],
            &[DataAccessClass::MetadataRead],
        ),
        descriptor(
            EDIT_ID,
            OperationDomain::ProjectMutation,
            &input_schema_name(EDIT_ID),
            OP_OUTPUT,
            TargetContract::Resource {
                kind: ResourceKind::VariantSet,
            },
            &[Read, Write],
            &[PermissionKind::MutateProject],
            &[DataAccessClass::MetadataRead],
        ),
        descriptor(
            ADOPT_ID,
            OperationDomain::ProjectMutation,
            &input_schema_name(ADOPT_ID),
            OP_OUTPUT,
            TargetContract::Resource {
                kind: ResourceKind::VariantSet,
            },
            &[Read, Write],
            &[PermissionKind::MutateProject],
            &[DataAccessClass::MetadataRead],
        ),
        descriptor(
            DISCARD_ID,
            OperationDomain::ProjectMutation,
            &input_schema_name(DISCARD_ID),
            OP_OUTPUT,
            TargetContract::Resource {
                kind: ResourceKind::VariantSet,
            },
            &[Read, Write, Delete],
            &[PermissionKind::MutateProject],
            &[DataAccessClass::MetadataRead],
        ),
    ]
}

/// The three query descriptors — registered with executors on
/// [`VariantQueryEngine`].
#[must_use]
pub fn query_descriptors() -> Vec<CapabilityDescriptor> {
    vec![
        descriptor(
            LIST_ID,
            OperationDomain::Query,
            &input_schema_name(LIST_ID),
            &output_schema_name(LIST_ID),
            TargetContract::None,
            &[EffectKind::Read],
            &[PermissionKind::ReadContext],
            &[DataAccessClass::MetadataRead],
        ),
        descriptor(
            DESCRIBE_ID,
            OperationDomain::Query,
            &input_schema_name(DESCRIBE_ID),
            &output_schema_name(DESCRIBE_ID),
            TargetContract::Resource {
                kind: ResourceKind::VariantSet,
            },
            &[EffectKind::Read],
            &[PermissionKind::ReadContext],
            &[DataAccessClass::MetadataRead],
        ),
        descriptor(
            COMPARE_ID,
            OperationDomain::Query,
            &input_schema_name(COMPARE_ID),
            &output_schema_name(COMPARE_ID),
            TargetContract::Resource {
                kind: ResourceKind::VariantSet,
            },
            &[EffectKind::Read],
            &[PermissionKind::ReadContext],
            &[DataAccessClass::MetadataRead],
        ),
    ]
}

// ── Input schemas ───────────────────────────────────────────────

fn obj(fields: &[(&str, ValueSchema)], required: &[&str], deny_unknown: bool) -> ValueSchema {
    let req: BTreeSet<&str> = required.iter().copied().collect();
    ValueSchema::Object {
        fields: fields
            .iter()
            .map(|(name, schema)| {
                (
                    (*name).to_owned(),
                    if req.contains(name) {
                        crate::schema::FieldSchema::required(schema.clone())
                    } else {
                        crate::schema::FieldSchema::optional(schema.clone())
                    },
                )
            })
            .collect(),
        deny_unknown,
    }
}

fn any_obj() -> ValueSchema {
    ValueSchema::Object {
        fields: BTreeMap::new(),
        deny_unknown: false,
    }
}

fn tagged_id() -> ValueSchema {
    ValueSchema::String { max_len: Some(128) }
}

fn int_schema() -> ValueSchema {
    ValueSchema::Integer {
        min: Some(0),
        max: None,
    }
}

/// The registered input schemas per mutation capability. `branch.edit`'s
/// `op` member is a *named operation* object — schema-loose here; the
/// prepare engine re-validates it against the inner capability's own
/// registered schema before lowering.
#[must_use]
pub fn mutation_input_schemas() -> Vec<(String, ValueSchema)> {
    vec![
        (
            input_schema_name(FORK_ID),
            obj(&[("scope", any_obj())], &["scope"], true),
        ),
        (
            input_schema_name(EDIT_ID),
            obj(
                &[
                    ("branch", tagged_id()),
                    ("expected_branch_revision", int_schema()),
                    ("op", any_obj()),
                ],
                &["branch", "expected_branch_revision", "op"],
                true,
            ),
        ),
        (
            input_schema_name(ADOPT_ID),
            obj(
                &[
                    ("branch", tagged_id()),
                    ("expected_project_revision", int_schema()),
                    ("expected_branch_revision", int_schema()),
                    ("expected_scope_digest", tagged_id()),
                ],
                &[
                    "branch",
                    "expected_project_revision",
                    "expected_branch_revision",
                    "expected_scope_digest",
                ],
                true,
            ),
        ),
        (
            input_schema_name(DISCARD_ID),
            obj(&[("branch", tagged_id())], &["branch"], true),
        ),
    ]
}

/// `(descriptor, input schema)` pairs for the prepare engine's
/// variant-op table — schema lookup is by the descriptor's declared
/// input-schema name so ordering can never drift.
///
/// # Panics
/// Panics if a mutation descriptor names an input schema missing from
/// [`mutation_input_schemas`] — a wiring bug, not caller input.
#[must_use]
pub fn mutation_specs() -> Vec<(CapabilityDescriptor, ValueSchema)> {
    let schemas: BTreeMap<String, ValueSchema> = mutation_input_schemas().into_iter().collect();
    mutation_descriptors()
        .into_iter()
        .map(|descriptor| {
            let input = schemas
                .get(descriptor.input_schema.name.as_str())
                .expect("every mutation descriptor names a registered input schema")
                .clone();
            (descriptor, input)
        })
        .collect()
}

/// The registered input schemas per query capability.
#[must_use]
pub fn query_input_schemas() -> Vec<(String, ValueSchema)> {
    vec![
        (
            input_schema_name(LIST_ID),
            obj(&[("scope_kind", tagged_id())], &[], true),
        ),
        (input_schema_name(DESCRIBE_ID), obj(&[], &[], true)),
        (
            input_schema_name(COMPARE_ID),
            obj(
                &[("branch_a", tagged_id()), ("branch_b", tagged_id())],
                &["branch_a", "branch_b"],
                true,
            ),
        ),
    ]
}

// ── Query engine ────────────────────────────────────────────────

/// Executes `synvelis.branch.{list,describe,compare}` over pinned
/// snapshots. `compare` additionally reads candidate payloads through
/// the shared store — a missing store reports `verified: false`
/// rather than guessing.
pub struct VariantQueryEngine {
    context: Arc<ContextProjectionEngine>,
    store: Option<SharedBranchStore>,
}

impl VariantQueryEngine {
    /// The engine over the shared context engine (pinning) and an
    /// optional content store for candidate decode.
    #[must_use]
    pub fn new(context: Arc<ContextProjectionEngine>, store: Option<SharedBranchStore>) -> Self {
        Self { context, store }
    }

    /// Registers the three query capabilities (descriptors, input
    /// schemas, loose output schemas) with `self` as executor. The
    /// mutation capabilities' schemas live on the prepare engine's
    /// variant-op table — this install never registers them.
    ///
    /// # Errors
    /// [`ErrorEnvelope`] — schema or capability registration conflict.
    pub fn install(self: &Arc<Self>, gateway: &mut crate::Gateway) -> Result<(), ErrorEnvelope> {
        for (name, schema) in query_input_schemas() {
            gateway
                .schemas_mut()
                .register(name, crate::schema::SchemaVersion::new(1, 0), schema)
                .map_err(ErrorEnvelope::from)?;
        }
        for id in [LIST_ID, DESCRIBE_ID, COMPARE_ID] {
            gateway
                .schemas_mut()
                .register(
                    output_schema_name(id),
                    crate::schema::SchemaVersion::new(1, 0),
                    any_obj(),
                )
                .map_err(ErrorEnvelope::from)?;
        }
        for descriptor in query_descriptors() {
            gateway.register_capability(descriptor, Some(self.clone()))?;
        }
        Ok(())
    }

    /// The bound `VariantSet` target — the set is always the explicit
    /// target for describe/compare (single source; args carry only
    /// operation parameters).
    fn target_set(req: &OperationRequest) -> Result<VariantSetId, ErrorEnvelope> {
        if let OperationTarget::Resource {
            resource: ResourceRef::VariantSet(id),
        } = req.target()
        {
            return Ok(*id);
        }
        Err(GatewayError::TargetRequired.into())
    }

    fn missing_set(id: &VariantSetId) -> ErrorEnvelope {
        GatewayError::PreparationRejected {
            reason: format!("variant set {} not found", id.to_tagged()),
        }
        .into()
    }

    fn list(&self, req: &OperationRequest) -> Result<Value, ErrorEnvelope> {
        let pinned = self.context.pin_for(req)?;
        let filter = match req.args().get("scope_kind") {
            Some(v) => {
                let name = v.as_str().ok_or_else(|| GatewayError::MalformedRequest {
                    reason: "args.scope_kind must be a string".to_owned(),
                })?;
                Some(VariantScopeKind::parse(name)?)
            }
            None => None,
        };
        let sets: Vec<Value> = pinned
            .state
            .variant_sets
            .values()
            .filter(|set| filter.is_none_or(|k| VariantScopeKind::of(&set.scope) == k))
            .map(|set| {
                json!({
                    "variant_set": set.id.to_tagged(),
                    "scope_kind": VariantScopeKind::of(&set.scope).name(),
                    "revision": set.revision.raw(),
                    "adopted": set.adopted.to_tagged(),
                    "branches": set.branches.len(),
                })
            })
            .collect();
        Ok(json!({
            "project": pinned.project.to_tagged(),
            "revision": pinned.revision.0,
            "snapshot": pinned.reference.as_str(),
            "variant_sets": sets,
        }))
    }

    fn describe(&self, req: &OperationRequest) -> Result<Value, ErrorEnvelope> {
        let pinned = self.context.pin_for(req)?;
        let set_id = Self::target_set(req)?;
        let set = pinned
            .state
            .variant_sets
            .get(&set_id)
            .ok_or_else(|| Self::missing_set(&set_id))?;
        let branches: Vec<Value> = set
            .branches
            .values()
            .map(|branch| {
                let content = match &branch.content {
                    BranchContentRef::Captured {
                        revision,
                        payload_digest,
                        scope_digest,
                        schema_version,
                    } => json!({
                        "kind": "captured",
                        "revision": revision.raw(),
                        "payload_digest": payload_digest.to_string(),
                        "scope_digest": scope_digest.to_string(),
                        "schema_version": schema_version,
                    }),
                    BranchContentRef::Live { revision } => json!({
                        "kind": "live",
                        "revision": revision.raw(),
                    }),
                };
                json!({
                    "branch": branch.id.to_tagged(),
                    "parent": branch.parent.map(synvelis_project::BranchId::to_tagged),
                    "adopted": branch.id == set.adopted,
                    "content": content,
                    "lineage_ids": branch.lineage.local_ids.len(),
                })
            })
            .collect();
        Ok(json!({
            "project": pinned.project.to_tagged(),
            "revision": pinned.revision.0,
            "snapshot": pinned.reference.as_str(),
            "variant_set": set.id.to_tagged(),
            "scope": serde_json::to_value(&set.scope).unwrap_or(Value::Null),
            "scope_kind": VariantScopeKind::of(&set.scope).name(),
            "set_revision": set.revision.raw(),
            "adopted": set.adopted.to_tagged(),
            "origin": serde_json::to_value(&set.origin).unwrap_or(Value::Null),
            "branches": branches,
            "last_adopt": serde_json::to_value(&set.last_adopt).unwrap_or(Value::Null),
        }))
    }

    /// Verified scope-content digests for two branches — a `Captured`
    /// payload is decoded through the store and its scope fingerprint
    /// *recomputed* (recorded digests are claims, never proof); the
    /// adopted branch's digest is the live scope fingerprint. Without
    /// a store the comparison reports `verified: false` with declared
    /// digests only.
    fn compare(&self, req: &OperationRequest) -> Result<Value, ErrorEnvelope> {
        let pinned = self.context.pin_for(req)?;
        let set_id = Self::target_set(req)?;
        let set = pinned
            .state
            .variant_sets
            .get(&set_id)
            .ok_or_else(|| Self::missing_set(&set_id))?;
        let branch_a = req_domain_id(req.args(), "branch_a", BranchId::from_tagged)?;
        let branch_b = req_domain_id(req.args(), "branch_b", BranchId::from_tagged)?;
        let describe_branch = |branch_id: &BranchId| -> Result<Value, ErrorEnvelope> {
            let branch =
                set.branches
                    .get(branch_id)
                    .ok_or_else(|| GatewayError::PreparationRejected {
                        reason: format!("branch {} not in set", branch_id.to_tagged()),
                    })?;
            let mut out = json!({
                "branch": branch.id.to_tagged(),
                "adopted": branch.id == set.adopted,
                "content_revision": branch.content.revision().raw(),
            });
            match &branch.content {
                BranchContentRef::Live { .. } => {
                    let digest = scope_digest(&pinned.state, &set.scope).map_err(|e| {
                        GatewayError::Internal {
                            reason: format!("scope digest: {e}"),
                        }
                    })?;
                    out["content"] = json!({
                        "kind": "live",
                        "verified_scope_digest": digest.to_string(),
                    });
                }
                BranchContentRef::Captured {
                    payload_digest,
                    scope_digest: declared,
                    ..
                } => {
                    let mut content = json!({
                        "kind": "captured",
                        "payload_digest": payload_digest.to_string(),
                        "declared_scope_digest": declared.to_string(),
                    });
                    if let Some(store) = &self.store {
                        let guard = store.lock().unwrap_or_else(PoisonError::into_inner);
                        match decode_candidate(&**guard, &branch.content) {
                            Ok(candidate) => match scope_digest(&candidate, &set.scope) {
                                Ok(verified) => {
                                    content["verified_scope_digest"] = json!(verified.to_string());
                                    content["verified"] = json!(true);
                                }
                                Err(e) => {
                                    content["verified"] = json!(false);
                                    content["verify_error"] = json!(e.to_string());
                                }
                            },
                            Err(e) => {
                                content["verified"] = json!(false);
                                content["verify_error"] = json!(variant_reason(&e));
                            }
                        }
                    } else {
                        content["verified"] = json!(false);
                        content["verify_error"] = json!("branch content store is not wired");
                    }
                    out["content"] = content;
                }
            }
            Ok(out)
        };
        let a = describe_branch(&branch_a)?;
        let b = describe_branch(&branch_b)?;
        let equal = match (
            a["content"]["verified_scope_digest"].as_str(),
            b["content"]["verified_scope_digest"].as_str(),
        ) {
            (Some(x), Some(y)) => Some(x == y),
            _ => None,
        };
        Ok(json!({
            "project": pinned.project.to_tagged(),
            "revision": pinned.revision.0,
            "snapshot": pinned.reference.as_str(),
            "variant_set": set.id.to_tagged(),
            "branch_a": a,
            "branch_b": b,
            "scope_digests_equal": equal,
        }))
    }
}

impl CapabilityOperation for VariantQueryEngine {
    fn execute(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        let result = match req.capability().id.as_str() {
            LIST_ID => self.list(req)?,
            DESCRIBE_ID => self.describe(req)?,
            COMPARE_ID => self.compare(req)?,
            _ => {
                return Err(GatewayError::Internal {
                    reason: "variant query engine bound to an unknown capability".to_owned(),
                }
                .into());
            }
        };
        Ok(GatewayOutcome::Completed { result })
    }
}

// ── Render capabilities (synvelis#517) ──────────────────────────
//
// `synvelis.render.*` rides the *same* prepare → proposal → approval →
// apply → CommitGate pipeline as `synvelis.branch.*`: each action is a
// `ProjectMutation`-domain capability whose prepare lowering produces
// [`PreparedMutation::Render`], and the commit gate dispatches the
// retained intent through a host-injected [`RenderDispatch`] port —
// the same dependency-injection shape [`VariantPorts`] uses, because
// synvelis-gateway does not depend on synvelis-render. The workspace
// `ACTION_RENDER_*` vocabulary (`synvelis-workspace::render_ui`) names
// these exact ids so every surface resolves identical Target/Grant/
// Receipt triples (RCP-UX-04 / RCP-POL-07).
//
// Render intents never mutate the project: the commit fence still
// checks the expected-revision CAS, then the dispatch outcome becomes
// the commit's provenance. `Unavailable`/`Failed` dispatches are
// determined non-commit rejections — a render success is never
// claimed when no backend confirmed it.

/// `synvelis.render.plan` — read-only render plan/preview resolution.
pub const RENDER_PLAN_ID: &str = "synvelis.render.plan";
/// `synvelis.render.inspect` — read-only artifact/job inspection.
pub const RENDER_INSPECT_ID: &str = "synvelis.render.inspect";
/// `synvelis.render.request` — submit a render job.
pub const RENDER_REQUEST_ID: &str = "synvelis.render.request";
/// `synvelis.render.cancel` — cancel a render job (idempotent on the
/// job key carried in args).
pub const RENDER_CANCEL_ID: &str = "synvelis.render.cancel";
/// `synvelis.render.retry` — retry a failed render job (idempotent on
/// the job key).
pub const RENDER_RETRY_ID: &str = "synvelis.render.retry";
/// `synvelis.render.verify` — re-verify artifact media (read-only).
pub const RENDER_VERIFY_ID: &str = "synvelis.render.verify";
/// `synvelis.render.attach` — attach a job's artifacts post-publish
/// (idempotent on the job key).
pub const RENDER_ATTACH_ID: &str = "synvelis.render.attach";
/// `synvelis.render.publish` — publish/share artifacts; the
/// grant-requiring action (`ExternalAction` + `ExternalDisclosure`).
pub const RENDER_PUBLISH_ID: &str = "synvelis.render.publish";
/// `synvelis.render.remote` — remote-backend render; the
/// grant-requiring action (`ExternalAction` + `ProviderTransfer`).
pub const RENDER_REMOTE_ID: &str = "synvelis.render.remote";

/// The render action a [`RenderIntent`] carries — one per registered
/// `synvelis.render.*` capability id.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RenderAction {
    /// `synvelis.render.plan`.
    Plan,
    /// `synvelis.render.inspect`.
    Inspect,
    /// `synvelis.render.request`.
    Request,
    /// `synvelis.render.cancel`.
    Cancel,
    /// `synvelis.render.retry`.
    Retry,
    /// `synvelis.render.verify`.
    Verify,
    /// `synvelis.render.attach`.
    Attach,
    /// `synvelis.render.publish`.
    Publish,
    /// `synvelis.render.remote`.
    Remote,
}

impl RenderAction {
    /// All actions in registration order.
    pub const ALL: [Self; 9] = [
        Self::Plan,
        Self::Inspect,
        Self::Request,
        Self::Cancel,
        Self::Retry,
        Self::Verify,
        Self::Attach,
        Self::Publish,
        Self::Remote,
    ];

    /// The capability id this action is registered under — identical
    /// to the workspace `ACTION_RENDER_*` vocabulary.
    #[must_use]
    pub const fn id(self) -> &'static str {
        match self {
            Self::Plan => RENDER_PLAN_ID,
            Self::Inspect => RENDER_INSPECT_ID,
            Self::Request => RENDER_REQUEST_ID,
            Self::Cancel => RENDER_CANCEL_ID,
            Self::Retry => RENDER_RETRY_ID,
            Self::Verify => RENDER_VERIFY_ID,
            Self::Attach => RENDER_ATTACH_ID,
            Self::Publish => RENDER_PUBLISH_ID,
            Self::Remote => RENDER_REMOTE_ID,
        }
    }

    /// The semantic-command op name recorded on the proposal.
    #[must_use]
    pub const fn op_name(self) -> &'static str {
        match self {
            Self::Plan => "render-plan",
            Self::Inspect => "render-inspect",
            Self::Request => "render-request",
            Self::Cancel => "render-cancel",
            Self::Retry => "render-retry",
            Self::Verify => "render-verify",
            Self::Attach => "render-attach",
            Self::Publish => "render-publish",
            Self::Remote => "render-remote",
        }
    }

    /// `true` for the read-only actions (plan/inspect/verify) — they
    /// declare `Read` effects only; every other action mutates job or
    /// artifact state.
    #[must_use]
    pub const fn read_only(self) -> bool {
        matches!(self, Self::Plan | Self::Inspect | Self::Verify)
    }

    /// Resolves a capability id to its action.
    #[must_use]
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|a| a.id() == id)
    }
}

/// A retained render command — lowered at prepare, dispatched at
/// commit. `args` is the schema-validated request payload forwarded
/// verbatim to the host's [`RenderDispatch`] port; semantic arg
/// validation (job keys, recipe refs) is the backend's contract, the
/// same way `VariantIntent` delegates domain checks to adapters.
#[derive(Clone, Debug)]
pub struct RenderIntent {
    /// Which render action to dispatch.
    pub action: RenderAction,
    /// Validated args object.
    pub args: Value,
}

impl RenderIntent {
    /// The capability id this intent was lowered from.
    #[must_use]
    pub const fn capability_id(&self) -> &'static str {
        // `RenderAction::id` is const but takes `self` by value —
        // dispatch through the match to stay const-compatible.
        match self.action {
            RenderAction::Plan => RENDER_PLAN_ID,
            RenderAction::Inspect => RENDER_INSPECT_ID,
            RenderAction::Request => RENDER_REQUEST_ID,
            RenderAction::Cancel => RENDER_CANCEL_ID,
            RenderAction::Retry => RENDER_RETRY_ID,
            RenderAction::Verify => RENDER_VERIFY_ID,
            RenderAction::Attach => RENDER_ATTACH_ID,
            RenderAction::Publish => RENDER_PUBLISH_ID,
            RenderAction::Remote => RENDER_REMOTE_ID,
        }
    }
}

/// What a [`RenderDispatch`] backend reports — the typed outcome the
/// commit gate turns into committed provenance or a determined
/// non-commit rejection.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(tag = "state", rename_all = "kebab-case")]
pub enum RenderDispatchState {
    /// The backend accepted the command (job submitted, control
    /// applied, inspection answered).
    Dispatched,
    /// Accepted but the result/activation is still pending — typed
    /// evidence, never reported as completed work.
    Pending {
        /// Why the outcome is pending.
        reason: String,
    },
    /// No render backend is wired — a determined rejection, distinct
    /// from a backend failure.
    Unavailable {
        /// Why dispatch is unavailable.
        reason: String,
    },
    /// The backend rejected or failed the command.
    Failed {
        /// Why the command failed.
        reason: String,
    },
}

/// The full dispatch result — state plus optional backend receipt
/// (job key, `TriggerReport`, inspection payload) recorded as commit
/// provenance so the receipt vocabulary stays identical across
/// surfaces.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
pub struct RenderDispatchOutcome {
    /// The typed dispatch state.
    pub state: RenderDispatchState,
    /// Human-readable detail.
    pub detail: Option<String>,
    /// Backend receipt payload — e.g. the lowered `TriggerReport` or
    /// job key — recorded verbatim as provenance.
    pub receipt: Option<Value>,
}

/// Host-injected render backend — the gateway-side seam that keeps
/// synvelis-gateway independent of synvelis-render (the same role
/// [`VariantPorts`] plays for branch mutations). Production hosts
/// adapt the render-job orchestrator; headless hosts inject
/// [`UnavailableRenderDispatch`].
pub trait RenderDispatch {
    /// Dispatches `intent`; must return an honest typed outcome —
    /// `Dispatched`/`Pending` only when the backend actually accepted
    /// the command.
    fn dispatch(&self, intent: &RenderIntent) -> RenderDispatchOutcome;
}

/// The headless default render backend: every command reports typed
/// `Unavailable` — registered capabilities stay invocable through the
/// authorization pipeline, but no render success is ever fabricated.
#[derive(Clone, Debug, Default)]
pub struct UnavailableRenderDispatch;

impl RenderDispatch for UnavailableRenderDispatch {
    fn dispatch(&self, intent: &RenderIntent) -> RenderDispatchOutcome {
        RenderDispatchOutcome {
            state: RenderDispatchState::Unavailable {
                reason: format!("no render backend is wired for {}", intent.action.id()),
            },
            detail: None,
            receipt: None,
        }
    }
}

/// The nine render mutation descriptors — registered on the prepare
/// engine's render-op table (invocable only *through*
/// `synvelis.operation.prepare`, like every `ProjectMutation`).
/// Target is `None`: render jobs are not project resources in this
/// slice — job identity travels inside validated args.
#[must_use]
pub fn render_descriptors() -> Vec<CapabilityDescriptor> {
    use DataAccessClass::{ExternalDisclosure, MetadataRead, ProviderTransfer};
    use EffectKind::{Disclose, ExternalAction, Read, Write};
    use PermissionKind::{ExternalAction as ExternalPerm, ReadContext, RunJob};
    RenderAction::ALL
        .into_iter()
        .map(|action| {
            let (effects, permissions, data): (
                &[EffectKind],
                &[PermissionKind],
                &[DataAccessClass],
            ) = if action.read_only() {
                (&[Read], &[ReadContext], &[MetadataRead])
            } else if matches!(action, RenderAction::Publish) {
                (
                    &[Write, Disclose],
                    &[RunJob, ExternalPerm],
                    &[MetadataRead, ExternalDisclosure],
                )
            } else if matches!(action, RenderAction::Remote) {
                (
                    &[Write, ExternalAction],
                    &[RunJob, ExternalPerm],
                    &[MetadataRead, ProviderTransfer],
                )
            } else {
                (&[Write], &[RunJob], &[MetadataRead])
            };
            descriptor(
                action.id(),
                OperationDomain::ProjectMutation,
                &input_schema_name(action.id()),
                OP_OUTPUT,
                TargetContract::None,
                effects,
                permissions,
                data,
            )
        })
        .collect()
}

/// The registered `(descriptor, input schema)` pairs for the render
/// ops — args are a schema-loose object: the capability floor carries
/// the authorization weight and the dispatch port owns semantic arg
/// validation (job keys, recipe references), mirroring how
/// `branch.edit`'s inner op is port-validated downstream.
#[must_use]
pub fn render_specs() -> Vec<(CapabilityDescriptor, ValueSchema)> {
    render_descriptors()
        .into_iter()
        .map(|d| (d, any_obj()))
        .collect()
}

/// Render declared facts — every render command reads the request's
/// project (plan resolution, job scoping) but writes no project
/// resource: the project diff is empty by construction and the real
/// effect is the dispatch outcome recorded as provenance.
pub(crate) fn render_declared_facts(_intent: &RenderIntent, project: &Project) -> OpFacts {
    OpFacts {
        reads: BTreeSet::from([ResourceRef::Project(project.project_id)]),
        impacted: BTreeMap::new(),
        preconditions: Vec::new(),
        unsupported: Vec::new(),
    }
}
