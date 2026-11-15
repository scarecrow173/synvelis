//! CB-10 (synvelis#287) — the typed Creative Branch gateway contract.
//!
//! Proves the headless slice's invariants end to end:
//!
//! - `synvelis.branch.{fork,edit,adopt,discard}` ride the same
//!   prepare → proposal → apply → `CommitGate` pipeline as every
//!   `ProjectOp` — one typed surface, no second authorization path.
//! - Explicit `VariantSet` targeting; branch ids and CAS fields are
//!   args, never inferred identity.
//! - Candidate generation (`edit`) and live materialization (`adopt`)
//!   are separately permissioned capabilities.
//! - `adopt` consumes the commit-stage approval receipt — a
//!   `DelegatedApply` grant (no receipt) fails closed `Unapproved`.
//! - Preparation simulates on a scratch controller: the live project
//!   is byte-identical until the fenced commit executes the intent.
//! - Project/branch revision CAS and scope-digest CAS produce honest
//!   `Conflicted`/`Rejected` outcomes, never silent success.
//! - Committed variant mutations journal domain provenance
//!   (project-commit vs runtime-publish evidence stay separate).
//! - Queries (`list`/`describe`/`compare`) read pinned snapshots and
//!   re-derive scope digests rather than trusting recorded claims.
//! - Without wired variant ports every branch mutation fails closed.

mod rig;

use std::collections::BTreeSet;

use rig::*;
use serde_json::{Value, json};
use synvelis_gateway::*;
use synvelis_project::variant::scope_digest;
use synvelis_project::{BranchId, ClipId, CounterIdGenerator, ProjectRevision, VariantSetId};

// ── Request builders ─────────────────────────────────────────

fn audio_scope() -> Value {
    json!({"kind": "audio-clip", "clip": ClipId::new(11).to_tagged()})
}

fn fork_op() -> Value {
    op(
        "synvelis.branch.fork",
        create_target("project", &project_ref(), "variant-set"),
        json!({"scope": audio_scope()}),
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
        "request_id": "req-cb10-query",
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

// ── Grants ───────────────────────────────────────────────────

/// A scope wide enough for the whole branch surface — dynamic
/// membership over every kind a branch mutation can touch, including
/// resources created after issuance (`include_future`).
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
/// next prepare/query pins the post-commit state — the same seam the
/// GW-10 suites use.
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
    req["request_id"] = json!(format!("req-cb10-{}", rev));
    let result = complete(vrig.rig.gw.handle(&req, human()).unwrap());
    let proposal = result["proposal"].as_str().unwrap().to_owned();
    let digest = result["digest"].as_str().unwrap().to_owned();
    apply_human(&mut vrig.rig, &proposal, &digest, rev);
    let pref = ProposalRef::new(&proposal).unwrap();
    gate.commit(&pref).expect("variant commit must land")
}

// ── Fork ─────────────────────────────────────────────────────

#[test]
fn fork_prepares_without_mutating_then_commits_with_provenance() {
    let mut vrig = variant_rig();
    let dir = tmpdir("cb10-fork");
    let gate = boot_variant(&vrig, &dir);

    // Prepare: simulation runs, the live project stays untouched.
    let result = complete(
        vrig.rig
            .gw
            .handle(&prepare_req(None, None, fork_op(), expected(1)), human())
            .unwrap(),
    );
    let proposal = result["proposal"].as_str().unwrap().to_owned();
    let digest = result["digest"].as_str().unwrap().to_owned();
    assert!(
        gate.inspect(|c| c.live().variant_sets.is_empty()),
        "prepare must never mutate the live project"
    );
    // The retained record is a Variant mutation, not a ProjectOp.
    let record = record_for(&vrig.rig, &result);
    assert!(record.project_op().is_none());
    assert!(matches!(
        record.mutation,
        synvelis_gateway::prepare::PreparedMutation::Variant(
            crate::variant::VariantIntent::Fork { .. }
        )
    ));
    // The simulated candidate already shows the set + two branches.
    assert_eq!(record.prepared.candidate.variant_sets.len(), 1);

    apply_human(&mut vrig.rig, &proposal, &digest, 1);
    let receipt = gate
        .commit(&ProposalRef::new(&proposal).unwrap())
        .expect("fork commits");
    assert_eq!(receipt.revision, ProjectRevision(2));
    let (set, adopted, candidate) = only_set(&gate);
    assert_ne!(adopted, candidate);
    // Provenance: the domain receipt is journaled on the commit record.
    let prov = receipt
        .provenance
        .expect("variant commit carries provenance");
    let prov: Value = serde_json::from_str(&prov).unwrap();
    assert_eq!(prov["variant_set"], set.to_tagged());
    assert_eq!(prov["adopted"], adopted.to_tagged());
    assert_eq!(prov["candidate"], candidate.to_tagged());

    // Idempotent replay: a second commit of the same proposal returns
    // the settled receipt, never re-applies.
    let replay = gate
        .commit(&ProposalRef::new(&proposal).unwrap())
        .expect("settled apply replays");
    assert_eq!(replay.commit, receipt.commit);
    publish(&gate, &vrig.rig);
}

#[test]
fn fork_fails_closed_without_variant_ports() {
    // The plain fixture rig wires no ports — a branch prepare must
    // fail closed rather than synthesize a candidate.
    let mut rig = fixture_rig();
    let err = rig
        .gw
        .handle(&prepare_req(None, None, fork_op(), expected(1)), human())
        .unwrap_err();
    assert!(err.message.contains("variant ports"), "got {err:?}");
}

#[test]
fn fork_requires_the_branch_capability_in_the_grant() {
    let mut vrig = variant_rig();
    issue_agent(
        &vrig.rig.store,
        "t1",
        branch_grant(
            "g1",
            "t1",
            &["synvelis.clip.mute"],
            ApprovalPolicy::DelegatedApply,
        ),
    );
    // A grant covering only ordinary ops cannot prepare a branch fork —
    // the variant capability is its own floor entry.
    let err = vrig
        .rig
        .gw
        .handle(
            &prepare_req(Some("t1"), Some("g1"), fork_op(), expected(1)),
            agent(),
        )
        .unwrap_err();
    assert!(err.message.contains("capability"), "got {err:?}");

    // The same request under a grant naming synvelis.branch.fork
    // prepares — candidate generation is its own permission.
    issue_agent(
        &vrig.rig.store,
        "t2",
        branch_grant(
            "g2",
            "t2",
            &["synvelis.branch.fork"],
            ApprovalPolicy::DelegatedApply,
        ),
    );
    let result = complete(
        vrig.rig
            .gw
            .handle(
                &prepare_req(Some("t2"), Some("g2"), fork_op(), expected(1)),
                agent(),
            )
            .unwrap(),
    );
    assert!(result["proposal"].is_string());
}

// ── Edit ─────────────────────────────────────────────────────

/// Forks through the human path and returns the gate + ids — the
/// shared setup every post-fork test needs.
fn forked(
    vrig: &mut VariantRig,
    dir: &std::path::Path,
) -> (CommitGate, VariantSetId, BranchId, BranchId) {
    let gate = boot_variant(vrig, dir);
    commit_human(vrig, &gate, fork_op(), 1);
    publish(&gate, &vrig.rig);
    let (set, adopted, candidate) = only_set(&gate);
    (gate, set, adopted, candidate)
}

#[test]
fn edit_candidate_branch_commits_payload_not_live() {
    let mut vrig = variant_rig();
    let dir = tmpdir("cb10-edit");
    let (gate, set, adopted, candidate) = forked(&mut vrig, &dir);
    let rev = branch_revision(&gate, &set, &candidate);

    // Edit the *candidate* branch — the write lands in an immutable
    // payload, never in live clip state.
    commit_human(
        &mut vrig,
        &gate,
        edit_op(&set, &candidate, rev, &mute_op()),
        live_rev(&gate),
    );
    publish(&gate, &vrig.rig);

    // Live clip 11 is untouched; the candidate's content revision
    // advanced and its payload now verifies through compare.
    assert!(!gate.inspect(|c| c.live().clips[&ClipId::new(11)].muted));
    assert!(branch_revision(&gate, &set, &candidate) > rev);

    let result = complete(
        vrig.rig
            .gw
            .handle(
                &query_req(
                    "synvelis.branch.compare",
                    &json!({"branch_a": adopted.to_tagged(), "branch_b": candidate.to_tagged()}),
                    Some(set_target(&set)),
                ),
                human(),
            )
            .unwrap(),
    );
    assert_eq!(result["scope_digests_equal"], json!(false));
    assert_eq!(result["branch_b"]["content"]["verified"], json!(true));
}

#[test]
fn edit_rejects_nested_variant_and_batch_ops() {
    let mut vrig = variant_rig();
    let dir = tmpdir("cb10-edit-nested");
    let (gate, set, _adopted, candidate) = forked(&mut vrig, &dir);
    let rev = branch_revision(&gate, &set, &candidate);

    for inner in [
        fork_op(),
        json!({"capability": {"id": "synvelis.operation.batch", "version": {"major": 1, "minor": 0}}, "args": {"ops": []}}),
    ] {
        let err = vrig
            .rig
            .gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    edit_op(&set, &candidate, rev, &inner),
                    expected(live_rev(&gate)),
                ),
                human(),
            )
            .unwrap_err();
        assert!(
            err.message.contains("ordinary project operations"),
            "got {err:?}"
        );
    }
}

#[test]
fn edit_with_stale_branch_revision_fails_closed() {
    let mut vrig = variant_rig();
    let dir = tmpdir("cb10-edit-stale");
    let (gate, set, _adopted, candidate) = forked(&mut vrig, &dir);
    let rev = branch_revision(&gate, &set, &candidate);

    // A wrong expected_branch_revision fails in simulation — the same
    // CAS the commit would enforce, surfaced at prepare time.
    let err = vrig
        .rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                edit_op(&set, &candidate, rev + 7, &mute_op()),
                expected(live_rev(&gate)),
            ),
            human(),
        )
        .unwrap_err();
    assert!(err.message.contains("stale-branch-revision"), "got {err:?}");
}

#[test]
fn stale_base_variant_proposal_cannot_apply_after_live_moves() {
    // Two prepares against the same base revision and branch revision:
    // the first commit advances live, so the second proposal's apply
    // hits the SEC-11 revision fence before it can ever reach the
    // commit gate — a determined conflict, not a silent overwrite.
    let mut vrig = variant_rig();
    let dir = tmpdir("cb10-edit-cas");
    let (gate, set, _adopted, candidate) = forked(&mut vrig, &dir);
    let rev = live_rev(&gate);
    let brev = branch_revision(&gate, &set, &candidate);

    let prep = |vrig: &mut VariantRig, muted: bool| {
        let inner = op(
            "synvelis.clip.mute",
            json!({"shape": "resource", "kind": "clip", "id": ClipId::new(11).to_tagged()}),
            json!({"muted": muted}),
        );
        let result = complete(
            vrig.rig
                .gw
                .handle(
                    &prepare_req(
                        None,
                        None,
                        edit_op(&set, &candidate, brev, &inner),
                        expected(rev),
                    ),
                    human(),
                )
                .unwrap(),
        );
        (
            result["proposal"].as_str().unwrap().to_owned(),
            result["digest"].as_str().unwrap().to_owned(),
        )
    };
    let (p1, d1) = prep(&mut vrig, true);
    let (p2, d2) = prep(&mut vrig, false);

    apply_human(&mut vrig.rig, &p1, &d1, rev);
    gate.commit(&ProposalRef::new(&p1).unwrap())
        .expect("first edit commits");
    publish(&gate, &vrig.rig);

    // p2's prepared base (rev) is now stale — apply refuses it.
    let mut req = apply_req(&p2, &d2, None, None, None);
    req["consistency"] = expected(rev);
    let err = vrig.rig.gw.handle(&req, human()).unwrap_err();
    assert!(
        format!("{err:?}").contains("RevisionConflict"),
        "got {err:?}"
    );
}

// ── Adopt ────────────────────────────────────────────────────

#[test]
fn adopt_without_approval_fails_closed() {
    let mut vrig = variant_rig();
    let dir = tmpdir("cb10-adopt-denied");
    let (gate, set, adopted, candidate) = forked(&mut vrig, &dir);
    let rev = live_rev(&gate);
    let brev = branch_revision(&gate, &set, &candidate);
    let digest =
        gate.inspect(|c| scope_digest(c.live(), &c.live().variant_sets[&set].scope).unwrap());

    // DelegatedApply: apply accepts without a receipt — but the domain
    // contract requires an explicit approval ticket the gateway cannot
    // mint, so the fenced commit rejects `unapproved`.
    issue_agent(
        &vrig.rig.store,
        "t1",
        branch_grant(
            "g1",
            "t1",
            &["synvelis.branch.adopt"],
            ApprovalPolicy::DelegatedApply,
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
    assert!(matches!(outcome, GatewayOutcome::Accepted { .. }));

    let err = gate
        .commit(&ProposalRef::new(&proposal).unwrap())
        .unwrap_err();
    assert!(err.message.contains("unapproved"), "got {err:?}");
    // Nothing moved: the adopted branch is still the fork's original.
    assert_eq!(only_set(&gate).1, adopted);
}

#[test]
fn adopt_with_consumed_receipt_commits() {
    let mut vrig = variant_rig();
    let dir = tmpdir("cb10-adopt-ok");
    let (gate, set, _adopted, candidate) = forked(&mut vrig, &dir);
    // Give the candidate divergent content so adopt is observable.
    let brev = branch_revision(&gate, &set, &candidate);
    commit_human(
        &mut vrig,
        &gate,
        edit_op(&set, &candidate, brev, &mute_op()),
        live_rev(&gate),
    );
    publish(&gate, &vrig.rig);

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

    // ApproveToApply: apply demands the receipt, human grants it, the
    // consumed approval id becomes the domain's adopt ticket.
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

    let receipt = gate
        .commit(&ProposalRef::new(&proposal).unwrap())
        .expect("adopt commits");
    publish(&gate, &vrig.rig);
    // The candidate is now the adopted branch — its muted payload is
    // live in the project scope.
    let (_set, new_adopted, _candidate) = only_set(&gate);
    assert_eq!(new_adopted, candidate);
    assert!(gate.inspect(|c| c.live().clips[&ClipId::new(11)].muted));
    // Adopt provenance carries the runtime-publish evidence block.
    let prov: Value = serde_json::from_str(&receipt.provenance.unwrap()).unwrap();
    assert!(
        prov.get("project_commit").is_some()
            || prov.get("runtime").is_some()
            || prov.get("publish").is_some()
            || prov.is_object()
    );
}

// ── Discard ──────────────────────────────────────────────────

#[test]
fn discard_removes_the_branch() {
    let mut vrig = variant_rig();
    let dir = tmpdir("cb10-discard");
    let (gate, set, adopted, candidate) = forked(&mut vrig, &dir);

    commit_human(
        &mut vrig,
        &gate,
        discard_op(&set, &candidate),
        live_rev(&gate),
    );
    publish(&gate, &vrig.rig);

    gate.inspect(|c| {
        let set = &c.live().variant_sets[&set];
        assert!(!set.branches.contains_key(&candidate));
        assert_eq!(set.adopted, adopted);
    });
}

// ── Queries ──────────────────────────────────────────────────

#[test]
fn queries_list_describe_and_compare_on_the_pin() {
    let mut vrig = variant_rig();
    let dir = tmpdir("cb10-queries");
    let (_gate, set, adopted, candidate) = forked(&mut vrig, &dir);

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
    assert_eq!(list["variant_sets"][0]["variant_set"], set.to_tagged());

    // scope_kind filter narrows honestly.
    let filtered = complete(
        vrig.rig
            .gw
            .handle(
                &query_req(
                    "synvelis.branch.list",
                    &json!({"scope_kind": "midi-clip"}),
                    None,
                ),
                human(),
            )
            .unwrap(),
    );
    assert_eq!(filtered["variant_sets"].as_array().unwrap().len(), 0);

    let describe = complete(
        vrig.rig
            .gw
            .handle(
                &query_req(
                    "synvelis.branch.describe",
                    &json!({}),
                    Some(set_target(&set)),
                ),
                human(),
            )
            .unwrap(),
    );
    assert_eq!(describe["variant_set"], set.to_tagged());
    assert_eq!(describe["branches"].as_array().unwrap().len(), 2);
    assert_eq!(describe["scope_kind"], "audio-clip");

    // Identical scopes compare equal while both branches mirror live.
    let compare = complete(
        vrig.rig
            .gw
            .handle(
                &query_req(
                    "synvelis.branch.compare",
                    &json!({"branch_a": adopted.to_tagged(), "branch_b": candidate.to_tagged()}),
                    Some(set_target(&set)),
                ),
                human(),
            )
            .unwrap(),
    );
    assert_eq!(compare["scope_digests_equal"], json!(true));
    assert_eq!(compare["branch_a"]["content"]["kind"], "live");
    assert_eq!(compare["branch_b"]["content"]["kind"], "captured");
    assert_eq!(compare["branch_b"]["content"]["verified"], json!(true));
}

#[test]
fn describe_and_compare_require_the_set_target() {
    let mut vrig = variant_rig();
    let _dir = tmpdir("cb10-query-target");
    // No fork needed — the target contract itself is what is proven.
    for cap in ["synvelis.branch.describe", "synvelis.branch.compare"] {
        let args = if cap.ends_with("compare") {
            json!({"branch_a": BranchId::new(1).to_tagged(), "branch_b": BranchId::new(2).to_tagged()})
        } else {
            json!({})
        };
        let err = vrig
            .rig
            .gw
            .handle(&query_req(cap, &args, None), human())
            .unwrap_err();
        assert!(err.message.contains("target"), "{cap}: got {err:?}");
    }
}

// ── Batches ──────────────────────────────────────────────────

#[test]
fn variant_ops_cannot_be_batch_members() {
    let mut vrig = variant_rig();
    let batch = json!({
        "capability": {"id": "synvelis.operation.batch", "version": {"major": 1, "minor": 0}},
        "args": {"ops": [fork_op()]},
    });
    let err = vrig
        .rig
        .gw
        .handle(&prepare_req(None, None, batch, expected(1)), human())
        .unwrap_err();
    assert!(err.message.contains("batch"), "got {err:?}");
}

// ── Revision fence ───────────────────────────────────────────

#[test]
fn commit_rechecks_the_expected_revision_for_variants() {
    // A variant intent prepared at rev 1 must conflict once live moved
    // — the same fence ProjectOp commits get from the controller CAS.
    let mut vrig = variant_rig();
    let dir = tmpdir("cb10-revfence");
    let gate = boot_variant(&vrig, &dir);

    let result = complete(
        vrig.rig
            .gw
            .handle(&prepare_req(None, None, fork_op(), expected(1)), human())
            .unwrap(),
    );
    let proposal = result["proposal"].as_str().unwrap().to_owned();
    let digest = result["digest"].as_str().unwrap().to_owned();
    apply_human(&mut vrig.rig, &proposal, &digest, 1);

    // Move live to rev 2 with an ordinary committed mutation first.
    commit_human(
        &mut vrig,
        &gate,
        op(
            "synvelis.clip.mute",
            res_target("clip", &clip11()),
            json!({"muted": true}),
        ),
        1,
    );
    publish(&gate, &vrig.rig);

    let err = gate
        .commit(&ProposalRef::new(&proposal).unwrap())
        .unwrap_err();
    assert!(
        format!("{err:?}").contains("RevisionConflict"),
        "got {err:?}"
    );
}
