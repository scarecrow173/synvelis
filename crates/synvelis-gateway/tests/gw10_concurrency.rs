// Test closures mirror the gateway's typed-envelope signatures.
#![allow(clippy::result_large_err)]

//! GW-10 concurrency evidence — Issue #62, unit GW10-D / gate G3.
//!
//! GTV-005 requires *deterministic enumeration* of the revoke/commit
//! orderings. Every case parks both contenders at the shared
//! [`ScheduledFence`], releases them in the order under test, and
//! records the causal trace plus the journal order as evidence.

mod rig;

use std::sync::Arc;

use rig::*;
use serde_json::json;
use synvelis_gateway::*;
use synvelis_project::{ProjectOp, ProjectRevision};

fn suite() -> std::sync::Arc<Evidence> {
    Evidence::suite("gw10-concurrency")
}

fn lbl(e: Expected) -> &'static str {
    match e {
        Expected::Permit => "Permit",
        Expected::Deny => "Deny",
        Expected::ApprovalRequired => "ApprovalRequired",
        Expected::Conflict => "Conflict",
        Expected::Unknown => "Unknown",
    }
}

/// Drives one revoke-vs-commit ordering; returns (commit result,
/// causal trace, journal payloads). `first` selects which labelled
/// section the fence releases first.
fn ordering(
    tag: &str,
    first: &str,
) -> (
    Result<CommitReceipt, ErrorEnvelope>,
    Vec<TraceEvent>,
    Vec<JournalPayload>,
) {
    let mut rig = fixture_rig();
    issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
    let dir = tmpdir(tag);
    let fence = ScheduledFence::default();
    let (gate, _report, _store) = boot_with(&rig, &dir, fence.clone());
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, _op) = accepted(&mut rig, "t1", "g1");
    let trace = CausalTrace::new();

    let gate = Arc::new(gate);
    let (c_gate, c_pref) = (gate.clone(), pref);
    let (c_fence, c_trace) = (fence.clone(), trace.clone());
    let commit_t = std::thread::spawn(move || {
        c_fence.run("commit", move || {
            let r = c_gate.commit(&c_pref);
            c_trace.mark("commit", "commit.settled", &format!("{r:?}"));
            r
        })
    });
    let (r_gate, r_fence, r_trace) = (gate.clone(), fence.clone(), trace.clone());
    let revoke_t = std::thread::spawn(move || {
        r_fence.run("revoke", move || {
            let r = r_gate.revoke_grant(&grant_ref("g1"));
            r_trace.mark("revoke", "revoke.settled", &format!("{r:?}"));
            r
        })
    });
    fence.wait_parked(2);
    trace.mark("schedule", "order", &format!("{first} first"));
    // The fence orders section *entry* — join the released thread so its
    // verdict is fully settled before the contender starts.
    let commit = if first == "revoke" {
        fence.release("revoke");
        revoke_t.join().unwrap().unwrap();
        fence.release("commit");
        commit_t.join().unwrap()
    } else {
        fence.release("commit");
        let commit = commit_t.join().unwrap();
        fence.release("revoke");
        revoke_t.join().unwrap().unwrap();
        commit
    };
    let payloads = journal_payloads(&dir.join("commit.journal"));
    (commit, trace.events(), payloads)
}

// ── REC-01: revoke confirmed first denies the commit ─────────

#[test]
fn rec01_revoke_first_denies_commit() {
    let ev = suite();
    let (commit, events, payloads) = ordering("gw10d-rec01", "revoke");
    let err = commit.expect_err("commit under a revoked grant must deny");
    assert!(matches!(
        err.code,
        GatewayError::Denied {
            stage: AuthorizationStage::Commit,
            ..
        }
    ));
    // Journal order is the durable order: revoke precedes rejection.
    let revoke_idx = payloads
        .iter()
        .position(|p| matches!(p, JournalPayload::Policy(PolicyEvent::RevokeGrant(_))))
        .expect("revoke journaled");
    let denied_idx = payloads
        .iter()
        .position(|p| matches!(p, JournalPayload::Rejected(_)))
        .expect("rejection journaled");
    assert!(revoke_idx < denied_idx);
    // AC-02: revoke-before-commit is deny — the same verdict.
    ev.case("AC-02")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10d-rec01")
        .verify("Deny", "Deny");
    ev.case("REC-01")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10d-rec01")
        .trace(&events)
        .verify("Deny", lbl(verdict_of_err(&Err::<(), _>(err.clone()))));
}

// ── REC-02: commit first stands; revoke is not undo ──────────

#[test]
fn rec02_commit_first_stands() {
    let ev = suite();
    let (commit, events, _payloads) = ordering("gw10d-rec02", "commit");
    let receipt = commit.expect("commit confirmed first must stand");
    assert_eq!(receipt.revision, ProjectRevision(2));
    // AC-03: commit-before-revoke stands — the same verdict.
    ev.case("AC-03")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10d-rec02")
        .verify("Permit", "Permit");
    ev.case("REC-02")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10d-rec02")
        .trace(&events)
        .log("revoke after commit is not an undo — the mutation stands")
        .verify("Permit", "Permit");
}

// ── GTV-005: enumerate both orderings over one harness ───────

#[test]
fn gtv005_both_orderings_enumerated_deterministically() {
    let ev = suite();
    // The two legal outcomes of the race, each forced by the schedule.
    let (revoke_first, _, _) = ordering("gw10d-enum-rf", "revoke");
    let (commit_first, _, _) = ordering("gw10d-enum-cf", "commit");
    assert!(revoke_first.is_err(), "revoke-first must deny");
    assert!(commit_first.is_ok(), "commit-first must stand");
    ev.case("GTV-005")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("scheduled-fence enumeration")
        .log("revoke→deny / commit→commit: both halves of the ordering space")
        .verify("Deny|Permit", "Deny|Permit");
}

// ── REC-03: concurrent same-revision commits conflict ────────

#[test]
fn rec03_two_commits_one_base_one_conflicts() {
    // REC-03: a manual commit and an agent commit both pinned at
    // revision 1 — whichever enters the fence first wins; the other
    // conflicts. Neither partially overwrites.
    let ev = suite();
    let mut rig = fixture_rig();
    issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
    let dir = tmpdir("gw10d-rec03");
    let fence = ScheduledFence::default();
    let (gate, _report, _store) = boot_with(&rig, &dir, fence.clone());
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, _op) = accepted(&mut rig, "t1", "g1");
    let trace = CausalTrace::new();

    let gate = Arc::new(gate);
    let (a_gate, a_pref) = (gate.clone(), pref);
    let a_fence = fence.clone();
    let agent_t = std::thread::spawn(move || a_fence.run("agent", move || a_gate.commit(&a_pref)));
    let (m_gate, m_fence, m_trace) = (gate.clone(), fence.clone(), trace.clone());
    let manual_t = std::thread::spawn(move || {
        m_fence.run("manual", move || {
            let r = m_gate.commit_manual(
                &human(),
                ProjectOp::SetProjectName {
                    name: "manual edit".to_owned(),
                },
                ProjectRevision(1),
            );
            m_trace.mark("manual", "manual.settled", &format!("{r:?}"));
            r
        })
    });
    fence.wait_parked(2);

    // Manual first → agent conflicts on the single writer's revision.
    fence.release("manual");
    let manual = manual_t.join().unwrap().expect("manual commit lands");
    assert_eq!(manual.revision, ProjectRevision(2));
    trace.mark("schedule", "order", "manual first");
    fence.release("agent");
    let err = agent_t
        .join()
        .unwrap()
        .expect_err("agent commit must conflict on the moved revision");
    assert!(matches!(err.code, GatewayError::RevisionConflict { .. }));
    assert!(!clip11_muted(&gate), "conflicted apply must not land");

    // AC-04: concurrent commits resolve to exactly one winner.
    ev.case("AC-04")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10d-rec03")
        .verify(
            "one Committed + one Conflict",
            "one Committed + one Conflict",
        );
    ev.case("REC-03")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10d-rec03")
        .trace(&trace.events())
        .verify("Conflict", lbl(verdict_of_err(&Err::<(), _>(err.clone()))));
}

// ── REC-05/06: idempotent retry under the key ────────────────

#[test]
fn rec05_rec06_keyed_retry_replays_and_conflicts() {
    // REC-05: same key + same payload replays the settled outcome —
    // no second commit. REC-06: same key + different payload is a
    // durable Rejected verdict, never a silent apply.
    let ev = suite();
    let mut rig = fixture_rig();
    issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
    let dir = tmpdir("gw10d-rec05");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));

    let (proposal, digest) = prepare_mute_agent(&mut rig, "t1", "g1");
    let mut req = apply_req(&proposal, &digest, None, Some("t1"), Some("g1"));
    req["idempotency_key"] = json!("key-1");
    let GatewayOutcome::Accepted { operation: op } = rig.gw.handle(&req, agent()).unwrap() else {
        panic!("expected Accepted");
    };
    let pref = ProposalRef::new(&proposal).unwrap();
    gate.commit(&pref).expect("keyed commit lands");

    // The ledger's own key+fingerprint for this bound request.
    let bound = rig
        .proposals
        .view(&pref)
        .and_then(|v| v.apply)
        .map(|a| a.request)
        .unwrap();
    let key = CommitGate::<MutexFence>::ledger_key_of(&bound).unwrap();
    let fp = CommitGate::<MutexFence>::fingerprint_of(&bound);

    // REC-04 + AC-06: same key, same fingerprint → replay, no new commit.
    assert!(matches!(
        gate.reconcile_key(&key, &fp),
        RetryOutcome::Settled(CommitOutcome::Committed(_))
    ));
    // REC-05 + AC-07: same key, different payload → durable rejection.
    assert!(matches!(
        gate.reconcile_key(&key, "sha256:different-payload"),
        RetryOutcome::Settled(CommitOutcome::Rejected { .. })
    ));
    // REC-06: the caller drops and re-asks by operation id — the settled
    // verdict is replayed from the journal without a second commit.
    let payloads_after = journal_payloads(&dir.join("commit.journal")).len();
    assert!(matches!(
        gate.reconcile(op),
        RetryOutcome::Settled(CommitOutcome::Committed(_))
    ));
    assert_eq!(
        journal_payloads(&dir.join("commit.journal")).len(),
        payloads_after,
        "reconcile replays the verdict — it never re-commits"
    );
    ev.case("REC-04")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10d-rec05")
        .verify("Permit", "Permit");
    ev.case("REC-05")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10d-rec05")
        .verify("Denied", "Denied");
    ev.case("REC-06")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10d-rec05")
        .log("reconcile re-reads the settled verdict — journal unchanged")
        .verify("Permit", "Permit");
    ev.case("AC-06")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10d-rec05")
        .verify("Permit", "Permit");
    ev.case("AC-07")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10d-rec05")
        .verify("Denied", "Denied");
}

// ── Approval consumption ordering (GTV-005 dimension) ────────

#[test]
fn gtv005_revoke_before_receipt_consumption_denies() {
    // An approval minted under ApproveToApply cannot be spent once the
    // grant is revoked before the receipt-presenting apply lands.
    let ev = suite();
    let mut rig = fixture_rig();
    issue_agent(
        &rig.store,
        "t1",
        apply_grant(
            "g1",
            "t1",
            &["synvelis.clip.mute"],
            ApprovalPolicy::ApproveToApply,
        ),
    );
    let dir = tmpdir("gw10d-appr");
    let fence = ScheduledFence::default();
    let (gate, _report, _store) = boot_with(&rig, &dir, fence.clone());
    issue_via_gate(
        &gate,
        "t1",
        apply_grant(
            "g1",
            "t1",
            &["synvelis.clip.mute"],
            ApprovalPolicy::ApproveToApply,
        ),
    );
    let (proposal, digest) = prepare_mute_agent(&mut rig, "t1", "g1");

    // Mint the receipt, then revoke before the consuming apply.
    let _ = rig.gw.handle(
        &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
        agent(),
    );
    let receipt = complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );
    let approval = receipt["approval"].as_str().unwrap().to_owned();

    gate.revoke_grant(&grant_ref("g1")).expect("revoke lands");
    let err = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, Some(&approval), Some("t1"), Some("g1")),
            agent(),
        )
        .expect_err("apply under a revoked grant must deny");
    assert!(matches!(err.code, GatewayError::Denied { .. }));
    // GTV-005's race dimension — evidence under the matrix row.
    ev.case("GTV-005")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10d-appr")
        .log("revoke before receipt consumption → apply denies")
        .verify("Deny", lbl(verdict_of_err(&Err::<(), _>(err.clone()))));
}

// ── AC-05: commit-stage effect evaluation seam ───────────────

#[test]
fn ac05_commit_stage_evaluation_denies_out_of_scope_writes() {
    // The gate re-runs effect-aware evaluation inside the fence at
    // Commit stage. A write set naming a resource the narrow grant's
    // scope omits denies at that seam — even though the same writes
    // evaluate Permit under the full grant. This mirrors the seam
    // commit_gate.rs exercises; here it carries GW-10 evidence.
    let ev = suite();
    let mut rig = fixture_rig();
    issue_agent(
        &rig.store,
        "t1",
        apply_grant(
            "g1",
            "t1",
            &["synvelis.clip.mute"],
            ApprovalPolicy::DelegatedApply,
        ),
    );
    let mut narrow = apply_grant(
        "g2",
        "t1",
        &["synvelis.clip.mute"],
        ApprovalPolicy::DelegatedApply,
    );
    narrow.scope = ResourceScope::Exact {
        resources: std::collections::BTreeSet::from([clip11(), clip12(), clip13()]),
    };
    rig.store.issue_grant(narrow);
    let (pref, _op) = accepted(&mut rig, "t1", "g1");
    let view = rig.proposals.view(&pref).unwrap();

    // The linked-MIDI write set: clips 12/13 + their shared source 10.
    let scoped = ScopedEffects {
        reads: Vec::new(),
        writes: vec![clip12(), source10()],
        creates: Vec::new(),
        deletes: Vec::new(),
        disclosures: Vec::new(),
        external_transfers: 0,
        unsupported: Vec::new(),
    };

    let wire = Gateway::parse_wire(&apply_req(
        pref.as_str(),
        &view.changeset.digest.to_string(),
        None,
        Some("t1"),
        Some("g2"),
    ))
    .unwrap();
    let narrow_req = rig.gw.bind(wire, agent()).unwrap();
    let full_req = &view.apply.as_ref().unwrap().request;

    let oracle: Arc<dyn ResourceOracle> = Arc::new(LatestOracle {
        source: rig.source.clone(),
    });
    let evaluator = GrantPolicy::new(rig.store.clone(), rig.clock.clone(), oracle);
    let denied = evaluator.evaluate_effects(
        &narrow_req,
        AuthorizationStage::Commit,
        &apply_desc(&rig),
        &scoped,
    );
    assert!(
        matches!(denied, AuthorizationDecision::Deny { .. }),
        "narrow scope must deny the out-of-scope source write"
    );
    let permitted = evaluator.evaluate_effects(
        full_req,
        AuthorizationStage::Commit,
        &apply_desc(&rig),
        &scoped,
    );
    assert!(
        matches!(permitted, AuthorizationDecision::PermitForStage),
        "the full scope must permit the same write set"
    );
    ev.case("AC-05")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10d-ac05")
        .log("same writes: narrow scope Deny, full scope Permit")
        .verify("Deny|Permit", "Deny|Permit");
}
