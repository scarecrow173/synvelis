//! CB-11 (synvelis#288) — CB-E2E-07 headless half: AI/VCS OFF.
//!
//! Pins the cross-gate invariants that hold with both optional
//! integration surfaces absent:
//!
//! - **AI OFF** — the agent principal holds no issued task/grant, so
//!   every delegated variant op fails closed `Denied` at prepare AND
//!   at apply; a human-prepared proposal stays appliable by the
//!   trusted human afterwards (fail-closed consumes nothing). With a
//!   grant the same agent path works — authority is grant-gated,
//!   never principal-bypassed.
//! - **VCS OFF** — `variant_rig()` wires `VariantPorts::standard(None)`:
//!   the in-memory branch content store, `NoopRuntimePublish` and the
//!   snapshot resolver. No durable bundle store, no journaled runtime
//!   publish, no VCS backend exists anywhere in the rig — the whole
//!   fork → edit → adopt → query lifecycle still runs on the Project
//!   CAS alone.
//! - **Honest boundary** — `VariantScope::Arrangement` has a merged
//!   project-side adapter (CB-07) but `VariantPorts::standard` does
//!   not wire it, so the gateway fails closed `unsupported` rather
//!   than synthesizing a candidate. Recorded as a follow-up gap, not
//!   papered over.
//!
//! Real AI integration, real VCS/durable backends and consumer Gate A
//! lanes stay `NotRun` — see `evidence/sv288/external-gates.md`.

mod rig;

use std::collections::BTreeSet;

use rig::*;
use serde_json::{Value, json};
use synvelis_gateway::*;
use synvelis_project::variant::scope_digest;
use synvelis_project::{BranchId, ClipId, CounterIdGenerator, TrackId, VariantSetId};

// ── Request builders (cb10 conventions) ───────────────────────

fn audio_scope() -> Value {
    json!({"kind": "audio-clip", "clip": ClipId::new(11).to_tagged()})
}

/// The arrangement scope over `demo_project`'s only track — parses and
/// authorizes fine; adapter resolution is where it must fail closed.
fn arrangement_scope() -> Value {
    json!({
        "kind": "arrangement",
        "tracks": [TrackId::new(3).to_tagged()],
        "time": {"start": 0, "end": 28_800},
        "fields": {"clips": true, "notes": true},
    })
}

fn fork_op(scope: &Value) -> Value {
    op(
        "synvelis.branch.fork",
        create_target("project", &project_ref(), "variant-set"),
        json!({"scope": scope}),
    )
}

fn set_target(set: &VariantSetId) -> Value {
    json!({"shape": "resource", "kind": "variant-set", "id": set.to_tagged()})
}

fn edit_op(set: &VariantSetId, branch: &BranchId, revision: u64, inner: &Value) -> Value {
    op(
        "synvelis.branch.edit",
        set_target(set),
        json!({
            "branch": branch.to_tagged(),
            "expected_branch_revision": revision,
            "op": inner,
        }),
    )
}

fn adopt_op(
    set: &VariantSetId,
    branch: &BranchId,
    project_rev: u64,
    branch_rev: u64,
    digest: &synvelis_project::snapshot::ContentDigest,
) -> Value {
    op(
        "synvelis.branch.adopt",
        set_target(set),
        json!({
            "branch": branch.to_tagged(),
            "expected_project_revision": project_rev,
            "expected_branch_revision": branch_rev,
            "expected_scope_digest": digest.to_string(),
        }),
    )
}

fn discard_op(set: &VariantSetId, branch: &BranchId) -> Value {
    op(
        "synvelis.branch.discard",
        set_target(set),
        json!({"branch": branch.to_tagged()}),
    )
}

fn mute_op() -> Value {
    op(
        "synvelis.clip.mute",
        json!({"shape": "resource", "kind": "clip", "id": ClipId::new(11).to_tagged()}),
        json!({"muted": true}),
    )
}

fn query_req(capability: &str, args: &Value, target: Option<Value>) -> Value {
    let mut req = json!({
        "request_id": "req-cb11-query",
        "capability": {"id": capability, "version": {"major": 1, "minor": 0}},
        "schema": {"name": format!("{capability}.input"), "version": {"major": 1, "minor": 0}},
        "project": project().to_tagged(),
        "args": args,
        "consistency": {"kind": "latest"},
    });
    if let Some(t) = target {
        req["target"] = t;
    }
    req
}

// ── Grants (cb10 `branch_grant` convention) ───────────────────

/// A scope wide enough for the whole branch surface — dynamic
/// membership over every kind a branch mutation can touch.
fn branch_scope() -> ResourceScope {
    ResourceScope::Dynamic(DynamicScope {
        owners: OwnerConstraint::Any,
        kinds: BTreeSet::from([
            ResourceKind::Project,
            ResourceKind::VariantSet,
            ResourceKind::Branch,
            ResourceKind::Clip,
            ResourceKind::Track,
            ResourceKind::Lane,
            ResourceKind::Source,
            ResourceKind::Asset,
            ResourceKind::Note,
        ]),
        range: None,
        max_members: None,
        include_future: true,
        move_args: None,
    })
}

/// A prepare+apply grant naming exactly the branch capabilities the
/// test grants — the capability floor is what this suite proves.
fn branch_grant(tag: &str, task: &str, caps: &[&str], policy: ApprovalPolicy) -> Grant {
    let mut grant = prep_grant(
        tag,
        task,
        caps,
        branch_scope(),
        Some(CreateWithin {
            parents: OwnerConstraint::Any,
            kinds: BTreeSet::from([ResourceKind::VariantSet, ResourceKind::Branch]),
            placement: None,
        }),
        &content_classes(),
    );
    let mut capabilities = grant.capabilities.clone();
    capabilities.insert(cap_ref(PREVIEW_ID));
    capabilities.insert(cap_ref(APPLY_ID));
    grant.capabilities = capabilities;
    grant.permissions =
        PermissionSet::of([PermissionKind::MutateProject, PermissionKind::ReadContext]);
    grant.approval_policy = policy;
    grant
}

// ── Flow helpers ─────────────────────────────────────────────

/// Trusted-human apply — no task/grant/approval machinery.
fn apply_human(rig: &mut Rig, proposal: &str, digest: &str, rev: u64) -> OperationId {
    let mut req = apply_req(proposal, digest, None, None, None);
    req["consistency"] = expected(rev);
    let outcome = rig.gw.handle(&req, human()).unwrap();
    let GatewayOutcome::Accepted { operation } = outcome else {
        panic!("expected Accepted, got {outcome:?}");
    };
    operation
}

/// Agent apply at a given expected revision.
fn apply_agent(
    rig: &mut Rig,
    proposal: &str,
    digest: &str,
    approval: Option<&str>,
    task: &str,
    grant: &str,
    rev: u64,
) -> GatewayOutcome<Value> {
    let mut req = apply_req(proposal, digest, approval, Some(task), Some(grant));
    req["consistency"] = expected(rev);
    rig.gw.handle(&req, agent()).unwrap()
}

/// The single variant set on the gate's live project.
fn only_set(gate: &CommitGate) -> (VariantSetId, BranchId, BranchId) {
    gate.inspect(|c| {
        let (set_id, set) = c
            .live()
            .variant_sets
            .iter()
            .next()
            .expect("one variant set");
        let adopted = set.adopted;
        let candidate = *set
            .branches
            .keys()
            .find(|b| **b != adopted)
            .expect("one candidate branch");
        (*set_id, adopted, candidate)
    })
}

/// Branch content revision for `branch` inside `set`.
fn branch_revision(gate: &CommitGate, set: &VariantSetId, branch: &BranchId) -> u64 {
    gate.inspect(|c| {
        c.live()
            .variant_sets
            .get(set)
            .and_then(|s| s.branches.get(branch))
            .map(|b| b.content.revision().raw())
            .expect("branch exists")
    })
}

/// Live project revision under the gate.
fn live_rev(gate: &CommitGate) -> u64 {
    gate.live_revision().0
}

/// Publishes the gate's live tip to the rig's snapshot source so the
/// next prepare/query pins the post-commit state.
fn publish(gate: &CommitGate, rig: &Rig) {
    let live = gate.inspect(|c| c.live().clone());
    rig.source.commit(project(), gate.live_revision(), live);
}

/// Boots a variant-wired gate over a fresh journal.
fn boot_variant(vrig: &VariantRig, dir: &std::path::Path) -> CommitGate {
    let store_dir = dir.join("project");
    let mut store = seed_store(&store_dir);
    let (gate, _report) = CommitGate::open(
        demo_project(),
        Box::new(CounterIdGenerator::new(9_000)),
        &mut store,
        dir.join("commit.journal"),
        gate_deps_variant(
            vrig.rig.store.clone(),
            vrig.rig.clock.clone(),
            vrig.rig.source.clone(),
            vrig.rig.proposals.clone(),
            apply_desc(&vrig.rig),
            vrig.ports.clone(),
        ),
        MutexFence::new(),
    )
    .expect("gate opens");
    gate
}

/// prepare (human) → apply (human) → commit. Returns the receipt.
fn commit_human(
    vrig: &mut VariantRig,
    gate: &CommitGate,
    operation: Value,
    rev: u64,
) -> CommitReceipt {
    let mut req = prepare_req(None, None, operation, expected(rev));
    req["request_id"] = json!(format!("req-cb11-{}", rev));
    let result = complete(vrig.rig.gw.handle(&req, human()).unwrap());
    let proposal = result["proposal"].as_str().unwrap().to_owned();
    let digest = result["digest"].as_str().unwrap().to_owned();
    apply_human(&mut vrig.rig, &proposal, &digest, rev);
    let pref = ProposalRef::new(&proposal).unwrap();
    gate.commit(&pref).expect("variant commit must land")
}

/// Forks `scope_op` through the human path; returns gate + set ids.
fn forked_scope(
    vrig: &mut VariantRig,
    dir: &std::path::Path,
    scope_op: &Value,
) -> (CommitGate, VariantSetId, BranchId, BranchId) {
    let gate = boot_variant(vrig, dir);
    commit_human(vrig, &gate, fork_op(scope_op), 1);
    publish(&gate, &vrig.rig);
    let (set, adopted, candidate) = only_set(&gate);
    (gate, set, adopted, candidate)
}

// ── CB-E2E-07: AI OFF ────────────────────────────────────────

#[test]
fn cb11_e2e_07_grantless_agent_ops_fail_closed_ai_off() {
    let mut vrig = variant_rig();
    let dir = tmpdir("cb11-ai-off");
    let gate = boot_variant(&vrig, &dir);

    // AI OFF: the agent principal holds no task and no grant — the
    // prepare must fail closed before any simulation, and no
    // proposal may be retained.
    let err = vrig
        .rig
        .gw
        .handle(
            &prepare_req(None, None, fork_op(&audio_scope()), expected(1)),
            agent(),
        )
        .expect_err("grantless agent prepare must deny");
    match err.code {
        GatewayError::Denied { reason, .. } => {
            assert_eq!(reason.as_deref(), Some("no applicable authorization"));
        }
        other => panic!("expected Denied, got {other:?}"),
    }
    assert_eq!(vrig.rig.engine.proposal_count(), 0);

    // A human-prepared proposal is still not appliable by the
    // grantless agent — apply re-checks delegated authority.
    let result = complete(
        vrig.rig
            .gw
            .handle(
                &prepare_req(None, None, fork_op(&audio_scope()), expected(1)),
                human(),
            )
            .unwrap(),
    );
    let proposal = result["proposal"].as_str().unwrap().to_owned();
    let digest = result["digest"].as_str().unwrap().to_owned();
    let err = vrig
        .rig
        .gw
        .handle(&apply_req(&proposal, &digest, None, None, None), agent())
        .expect_err("grantless agent apply must deny");
    match err.code {
        GatewayError::Denied { .. } => {}
        other => panic!("expected Denied, got {other:?}"),
    }

    // Fail-closed consumed nothing: the trusted human applies and
    // commits the same proposal — manual DAW authority never depends
    // on agent grants.
    apply_human(&mut vrig.rig, &proposal, &digest, 1);
    gate.commit(&ProposalRef::new(&proposal).unwrap())
        .expect("human fork commits");
    publish(&gate, &vrig.rig);
    let (set, _adopted, _candidate) = only_set(&gate);
    assert!(gate.inspect(|c| c.live().variant_sets.contains_key(&set)));
}

// ── CB-E2E-07: VCS OFF ───────────────────────────────────────

#[test]
#[allow(clippy::too_many_lines)] // sequential lifecycle assertions read top-to-bottom (cb10 convention)
fn cb11_e2e_07_vcs_off_variant_lifecycle_and_granted_agent() {
    // `variant_rig()` is the VCS-off rig by construction:
    // `VariantPorts::standard(None)` selects the in-memory branch
    // content store + `NoopRuntimePublish` + `SnapshotResolver` —
    // no durable bundle store, no journaled runtime publish, no VCS
    // backend is wired anywhere. Every op below must therefore run
    // purely on the Project CAS.
    let mut vrig = variant_rig();
    let dir = tmpdir("cb11-vcs-off");
    let (gate, set, adopted, candidate) = forked_scope(&mut vrig, &dir, &audio_scope());

    // Human edit on the candidate — payload write, live untouched.
    let brev = branch_revision(&gate, &set, &candidate);
    commit_human(
        &mut vrig,
        &gate,
        edit_op(&set, &candidate, brev, &mute_op()),
        live_rev(&gate),
    );
    publish(&gate, &vrig.rig);
    assert!(!gate.inspect(|c| c.live().clips[&ClipId::new(11)].muted));

    // Granted agent adopt: AI authority exists only through an
    // issued task+grant; ApproveToApply still demands the human
    // approval receipt before apply, and the consumed receipt id is
    // the domain's adopt ticket.
    let rev = live_rev(&gate);
    let brev = branch_revision(&gate, &set, &candidate);
    let digest =
        gate.inspect(|c| scope_digest(c.live(), &c.live().variant_sets[&set].scope).unwrap());
    issue_agent(
        &vrig.rig.store,
        "t1",
        branch_grant(
            "g1",
            "t1",
            &["synvelis.branch.adopt"],
            ApprovalPolicy::ApproveToApply,
        ),
    );
    let result = complete(
        vrig.rig
            .gw
            .handle(
                &prepare_req(
                    Some("t1"),
                    Some("g1"),
                    adopt_op(&set, &candidate, rev, brev, &digest),
                    expected(rev),
                ),
                agent(),
            )
            .unwrap(),
    );
    let proposal = result["proposal"].as_str().unwrap().to_owned();
    let digest_s = result["digest"].as_str().unwrap().to_owned();
    let outcome = apply_agent(&mut vrig.rig, &proposal, &digest_s, None, "t1", "g1", rev);
    let GatewayOutcome::ApprovalRequired { approval } = outcome else {
        panic!("expected ApprovalRequired, got {outcome:?}");
    };
    complete(
        vrig.rig
            .gw
            .handle(&approve_req(&proposal, &digest_s), human())
            .unwrap(),
    );
    let outcome = apply_agent(
        &mut vrig.rig,
        &proposal,
        &digest_s,
        Some(approval.as_str()),
        "t1",
        "g1",
        rev,
    );
    assert!(matches!(outcome, GatewayOutcome::Accepted { .. }));
    gate.commit(&ProposalRef::new(&proposal).unwrap())
        .expect("adopt commits");
    publish(&gate, &vrig.rig);

    // The candidate is now the adopted branch — its muted payload is
    // live, with no VCS/durable backend involved anywhere.
    let (_set, new_adopted, _candidate) = only_set(&gate);
    assert_eq!(new_adopted, candidate);
    assert!(gate.inspect(|c| c.live().clips[&ClipId::new(11)].muted));

    // Queries read the pinned post-commit state.
    let list = complete(
        vrig.rig
            .gw
            .handle(
                &query_req("synvelis.branch.list", &json!({}), None),
                human(),
            )
            .unwrap(),
    );
    assert_eq!(list["variant_sets"].as_array().unwrap().len(), 1);
    let compare = complete(
        vrig.rig
            .gw
            .handle(
                &query_req(
                    "synvelis.branch.compare",
                    &json!({
                        "branch_a": adopted.to_tagged(),
                        "branch_b": candidate.to_tagged(),
                    }),
                    Some(set_target(&set)),
                ),
                human(),
            )
            .unwrap(),
    );
    assert_eq!(compare["scope_digests_equal"], json!(false));

    // Discard the remaining non-adopted branch (the pre-adopt live
    // mirror) — the fourth mutation op on the same VCS-less rig.
    let (_set, adopted_now, remaining) = only_set(&gate);
    assert_eq!(adopted_now, candidate);
    commit_human(
        &mut vrig,
        &gate,
        discard_op(&set, &remaining),
        live_rev(&gate),
    );
    publish(&gate, &vrig.rig);
    gate.inspect(|c| {
        let set = &c.live().variant_sets[&set];
        assert!(!set.branches.contains_key(&remaining));
        assert_eq!(set.adopted, candidate);
    });
}

// ── CB-E2E-07: honest adapter boundary ───────────────────────

#[test]
fn cb11_e2e_07_arrangement_scope_fails_closed_until_gateway_wiring() {
    // `VariantPorts::standard` wires six domain adapters but not
    // `ArrangementAdapter` (merged for CB-07 in synvelis-project) —
    // the gateway must fail closed `unsupported` rather than
    // synthesize a candidate through the wrong adapter. This pins
    // the CURRENT boundary honestly; wiring the adapter is a src
    // change outside this slice (recorded in the sv288 ledger).
    let mut vrig = variant_rig();
    let _dir = tmpdir("cb11-arr-unsupported");
    let err = vrig
        .rig
        .gw
        .handle(
            &prepare_req(None, None, fork_op(&arrangement_scope()), expected(1)),
            human(),
        )
        .expect_err("arrangement fork must fail closed without a wired adapter");
    assert!(
        err.message
            .contains("no domain adapter wired for arrangement"),
        "got {err:?}"
    );
}
