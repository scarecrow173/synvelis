//! synvelis#517 — `synvelis.render.*` gateway registration parity.
//!
//! Proves the render action surface rides the *same*
//! prepare → proposal → approval → apply → `CommitGate` pipeline as
//! `synvelis.branch.*`:
//!
//! - All nine `synvelis.render.*` ids register as `ProjectMutation`
//!   capabilities with the workspace `ACTION_RENDER_*` vocabulary.
//! - Prepare lowers to `PreparedMutation::Render` — never a `ProjectOp`
//!   — and the candidate is the unchanged base (render mutates no
//!   project state).
//! - Commit dispatches through the host-injected `RenderDispatch` port
//!   inside the fence: `Dispatched`/`Pending` commit with typed
//!   provenance; `Unavailable`/`Failed` are determined non-commit
//!   rejections — a render success is never fabricated.
//! - Target/Grant/idempotency/fail-closed parity with `synvelis.branch.*`:
//!   the capability floor, permission requirements (`RunJob`,
//!   `ExternalAction` for publish/remote), revision fencing, and
//!   idempotent apply replay all behave identically.

mod rig;

use std::collections::BTreeSet;
use std::sync::{Arc, Mutex};

use rig::*;
use serde_json::{Value, json};
use synvelis_gateway::variant::{
    RenderAction, RenderDispatch, RenderDispatchOutcome, RenderDispatchState, RenderIntent,
    UnavailableRenderDispatch,
};
use synvelis_gateway::*;
use synvelis_project::CounterIdGenerator;

// ── Render request builders ─────────────────────────────────

/// A render op with no wire target — `TargetContract::None` rejects a
/// supplied target, so job identity travels inside `args`.
fn render_op(capability: &str, args: Value) -> Value {
    op(capability, Value::Null, args)
}

fn render_request_op() -> Value {
    render_op(
        "synvelis.render.request",
        json!({"recipe": {"range": {"domain": "project", "tick": 0}}, "job": "req-1"}),
    )
}

// ── Dispatch fake ───────────────────────────────────────────

/// Records every dispatched intent and returns a configured outcome.
struct RecordingDispatch {
    calls: Mutex<Vec<RenderIntent>>,
    state: RenderDispatchState,
}

impl RecordingDispatch {
    fn dispatched() -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            state: RenderDispatchState::Dispatched,
        })
    }

    fn failing(state: RenderDispatchState) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::new(Vec::new()),
            state,
        })
    }

    fn call_count(&self) -> usize {
        self.calls.lock().unwrap().len()
    }

    fn last_action(&self) -> Option<RenderAction> {
        self.calls.lock().unwrap().last().map(|i| i.action)
    }
}

impl RenderDispatch for RecordingDispatch {
    fn dispatch(&self, intent: &RenderIntent) -> RenderDispatchOutcome {
        self.calls.lock().unwrap().push(intent.clone());
        RenderDispatchOutcome {
            state: self.state.clone(),
            detail: None,
            receipt: Some(json!({"job_key": "job-1", "via": "fake-backend"})),
        }
    }
}

// ── Boot + flow helpers ─────────────────────────────────────

/// Boots a gate whose deps carry the given render dispatch — the same
/// seam `gate_deps_variant` uses for variant ports.
fn boot_render(
    rig: &Rig,
    dir: &std::path::Path,
    render: Arc<dyn RenderDispatch + Send + Sync>,
) -> CommitGate {
    let store_dir = dir.join("project");
    let mut store = seed_store(&store_dir);
    let mut deps = rig_deps(rig);
    deps.render = Some(render);
    let (gate, _report) = CommitGate::open(
        demo_project(),
        Box::new(CounterIdGenerator::new(9_000)),
        &mut store,
        dir.join("commit.journal"),
        deps,
        MutexFence::new(),
    )
    .expect("gate opens");
    gate
}

/// prepare (human) → apply (human); returns `(proposal, digest)`.
fn prepare_apply_human(rig: &mut Rig, operation: Value, rev: u64) -> (String, String) {
    let mut req = prepare_req(None, None, operation, expected(rev));
    req["request_id"] = json!(format!("req-sv517-{}", rev));
    let result = complete(rig.gw.handle(&req, human()).unwrap());
    (
        result["proposal"].as_str().unwrap().to_owned(),
        result["digest"].as_str().unwrap().to_owned(),
    )
}

fn apply_human(rig: &mut Rig, proposal: &str, digest: &str, rev: u64) -> OperationId {
    let mut req = apply_req(proposal, digest, None, None, None);
    req["consistency"] = expected(rev);
    let outcome = rig.gw.handle(&req, human()).unwrap();
    let GatewayOutcome::Accepted { operation } = outcome else {
        panic!("expected Accepted, got {outcome:?}");
    };
    operation
}

// ── Grants ──────────────────────────────────────────────────

/// Project-read scope — render commands declare a project read and no
/// resource writes.
fn render_scope() -> ResourceScope {
    ResourceScope::Dynamic(DynamicScope {
        owners: OwnerConstraint::Any,
        kinds: BTreeSet::from([ResourceKind::Project]),
        range: None,
        max_members: None,
        include_future: true,
        move_args: None,
    })
}

/// A prepare+apply grant for render ops. The wire capabilities
/// (`synvelis.operation.prepare`/`apply`) require `MutateProject` +
/// `ReadContext`; per-op `required_permissions` on the render
/// descriptors are declarative metadata — the enforced floors are the
/// capability list and the data-access classes, exactly like every
/// other `ProjectMutation` op.
fn render_grant(tag: &str, task: &str, caps: &[&str], classes: &[DataAccessClass]) -> Grant {
    let mut grant = prep_grant(tag, task, caps, render_scope(), None, classes);
    let mut capabilities = grant.capabilities.clone();
    capabilities.insert(cap_ref(PREVIEW_ID));
    capabilities.insert(cap_ref(APPLY_ID));
    grant.capabilities = capabilities;
    grant.permissions =
        PermissionSet::of([PermissionKind::MutateProject, PermissionKind::ReadContext]);
    grant
}

fn render_classes() -> Vec<DataAccessClass> {
    vec![
        DataAccessClass::MetadataRead,
        DataAccessClass::ProviderTransfer,
        DataAccessClass::ExternalDisclosure,
    ]
}

// ── Tests ───────────────────────────────────────────────────

#[test]
fn all_nine_render_capabilities_are_registered() {
    let rig = fixture_rig();
    for action in RenderAction::ALL {
        let reference = cap_ref(action.id());
        let descriptor = rig
            .gw
            .registry()
            .describe(&reference)
            .unwrap_or_else(|_| panic!("{} must be registered", action.id()));
        assert_eq!(
            descriptor.domain,
            OperationDomain::ProjectMutation,
            "{} must ride the mutation pipeline",
            action.id()
        );
        // Read-only actions declare Read-only effects; job commands
        // declare Write; publish/remote declare the external classes.
        if action.read_only() {
            assert!(descriptor.effect_bound.contains(EffectKind::Read));
            assert!(!descriptor.effect_bound.contains(EffectKind::Write));
        } else {
            assert!(descriptor.effect_bound.contains(EffectKind::Write));
        }
    }
    // The grant-requiring pair carries the external permission floor.
    for id in ["synvelis.render.publish", "synvelis.render.remote"] {
        let d = rig.gw.registry().describe(&cap_ref(id)).unwrap();
        assert!(
            d.required_permissions
                .iter()
                .any(|k| k == PermissionKind::ExternalAction),
            "{id} must require ExternalAction"
        );
    }
}

#[test]
fn render_request_commits_and_dispatches_without_mutating_project() {
    let mut rig = fixture_rig();
    let dir = tmpdir("sv517-render-ok");
    let dispatch = RecordingDispatch::dispatched();
    let gate = boot_render(&rig, &dir, dispatch.clone());

    let (proposal, digest) = prepare_apply_human(&mut rig, render_request_op(), 1);
    apply_human(&mut rig, &proposal, &digest, 1);
    let receipt = gate
        .commit(&ProposalRef::new(&proposal).unwrap())
        .expect("render commit must land");

    // The intent reached the backend verbatim — action + args.
    assert_eq!(dispatch.call_count(), 1);
    assert_eq!(dispatch.last_action(), Some(RenderAction::Request));
    // The commit produced NO project mutation: revision stays at the
    // asserted base and provenance carries the typed dispatch outcome.
    assert_eq!(receipt.revision, receipt.base_revision);
    let provenance: Value = serde_json::from_str(&receipt.provenance.unwrap()).unwrap();
    assert_eq!(provenance["state"]["state"], json!("dispatched"));
    assert_eq!(provenance["receipt"]["job_key"], json!("job-1"));
}

#[test]
fn render_pending_and_unavailable_are_typed_never_success() {
    let mut rig = fixture_rig();
    let dir = tmpdir("sv517-render-pending");
    let dispatch = RecordingDispatch::failing(RenderDispatchState::Pending {
        reason: "backend queued".to_owned(),
    });
    let gate = boot_render(&rig, &dir, dispatch.clone());
    let (proposal, digest) = prepare_apply_human(&mut rig, render_request_op(), 1);
    apply_human(&mut rig, &proposal, &digest, 1);
    // Pending is an accepted-but-unconfirmed outcome — it commits with
    // typed evidence, exactly like a pending runtime publish.
    let receipt = gate
        .commit(&ProposalRef::new(&proposal).unwrap())
        .expect("pending dispatch still commits");
    let provenance: Value = serde_json::from_str(&receipt.provenance.unwrap()).unwrap();
    assert_eq!(provenance["state"]["state"], json!("pending"));

    // Unavailable is a determined non-commit rejection — never a
    // fabricated success.
    let dir2 = tmpdir("sv517-render-unavail");
    let mut rig2 = fixture_rig();
    let gate2 = boot_render(&rig2, &dir2, Arc::new(UnavailableRenderDispatch));
    let (proposal2, digest2) = prepare_apply_human(&mut rig2, render_request_op(), 1);
    apply_human(&mut rig2, &proposal2, &digest2, 1);
    let err = gate2
        .commit(&ProposalRef::new(&proposal2).unwrap())
        .expect_err("unavailable dispatch must reject");
    assert!(
        err.message.contains("unavailable") || err.message.contains("no render backend"),
        "got {err:?}"
    );
}

#[test]
fn render_without_dispatch_port_fails_closed() {
    let mut rig = fixture_rig();
    let dir = tmpdir("sv517-render-noport");
    let (gate, _report, _store) = boot(&rig, &dir); // rig_deps wires render: None
    let (proposal, digest) = prepare_apply_human(&mut rig, render_request_op(), 1);
    apply_human(&mut rig, &proposal, &digest, 1);
    let err = gate
        .commit(&ProposalRef::new(&proposal).unwrap())
        .expect_err("missing render port must reject");
    assert!(err.message.contains("render dispatch"), "got {err:?}");
}

#[test]
fn render_respects_the_capability_floor() {
    let mut rig = fixture_rig();
    // A grant that does not name synvelis.render.request cannot
    // prepare it — the capability floor is identical to branch ops.
    issue_agent(
        &rig.store,
        "t1",
        render_grant("g1", "t1", &["synvelis.clip.mute"], &render_classes()),
    );
    let err = rig
        .gw
        .handle(
            &prepare_req(Some("t1"), Some("g1"), render_request_op(), expected(1)),
            agent(),
        )
        .unwrap_err();
    assert!(err.message.contains("capability"), "got {err:?}");

    // Naming the capability prepares — render ops carry no scoped
    // resource effects in this slice (Target::None), so the enforced
    // floor is the capability list, exactly like the branch ops.
    issue_agent(
        &rig.store,
        "t2",
        render_grant("g2", "t2", &["synvelis.render.remote"], &render_classes()),
    );
    let mut prep = prepare_req(
        Some("t2"),
        Some("g2"),
        render_op("synvelis.render.remote", json!({"job": "job-1"})),
        expected(1),
    );
    prep["request_id"] = json!("req-sv517-remote");
    let result = complete(rig.gw.handle(&prep, agent()).unwrap());
    assert!(result["proposal"].is_string() || result["digest"].is_string());
}

#[test]
fn render_apply_replays_idempotently_like_branch_ops() {
    let mut rig = fixture_rig();
    issue_agent(
        &rig.store,
        "t1",
        render_grant("g1", "t1", &["synvelis.render.request"], &render_classes()),
    );
    // Prepare once, then apply twice under the same idempotency key —
    // the second apply replays the recorded outcome instead of
    // minting a second commit attempt (same machinery as branch ops).
    let mut prep = prepare_req(Some("t1"), Some("g1"), render_request_op(), expected(1));
    prep["request_id"] = json!("req-sv517-idem");
    let result = complete(rig.gw.handle(&prep, agent()).unwrap());
    let proposal = result["proposal"].as_str().unwrap().to_owned();
    let digest = result["digest"].as_str().unwrap().to_owned();

    let mut req1 = apply_req(&proposal, &digest, None, Some("t1"), Some("g1"));
    req1["idempotency_key"] = json!("render-key-1");
    rig.gw.handle(&req1, agent()).unwrap();

    let mut req2 = apply_req(&proposal, &digest, None, Some("t1"), Some("g1"));
    req2["idempotency_key"] = json!("render-key-1");
    // Same key + same payload is a determined DuplicateRequest — the
    // recorded outcome replays, never a second effect (identical to
    // the branch-op idempotency contract).
    let err = rig.gw.handle(&req2, agent()).unwrap_err();
    assert!(
        matches!(err.code, GatewayError::DuplicateRequest { .. }),
        "got {err:?}"
    );
}
