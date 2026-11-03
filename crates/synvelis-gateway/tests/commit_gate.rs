// Test closures mirror the gateway's typed-envelope signatures.
#![allow(clippy::result_large_err)]

//! GW-06 authorized commit gate — Issue #58 acceptance evidence.
//!
//! Each test names the criterion it proves:
//!
//! - AC-01/06/08/10 — authorized apply → commit → undo, receipt
//!   re-verification, idempotent replay, the manual path.
//! - AC-02/03/04 — the `ScheduledFence` enumerates revoke/commit
//!   orderings deterministically instead of racing threads blindly.
//! - AC-05/09 — receipt + scope/effect revalidation at the commit
//!   boundary, against the *current* store.
//! - REC-01/02 — linearization verdicts (deny / commit-stands /
//!   revision conflict) recorded durably.
//! - AC-06/07 — idempotency-key replay and keyed conflict.
//! - REC-03..06 — real fault injection (`CommitHooks` + torn-tail byte
//!   writes on the actual journal file): outcomes never misreported.
//! - REC-07/08 — policy lifecycle + consumption replay; REC-10 —
//!   fenced HEAD publication.
//! - REC-11 — crash between mutation and durable record leaves no
//!   partially-applied state (verified-source content survives intact).
//! - AC-09/REC-12 — a stale proposal can never commit after restart (the
//!   in-memory proposal store dies with the process; only the journal
//!   survives).

mod rig;
use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::sync::Arc;

use serde_json::json;
use synvelis_gateway::commit::{
    CommitGate, CommitOutcome, GateDeps, JournalPayload, MutexFence, PolicyEvent, RetryOutcome,
};
use synvelis_gateway::journal;
use synvelis_gateway::*;
use synvelis_project::{
    CounterIdGenerator, DirectoryBundleStore, ProjectId, ProjectOp, ProjectRevision, ProjectStore,
    SourceId,
};

use rig::*;

// ── AC-01 / AC-10: the authorized path commits and rides Undo ─

#[test]
fn delegated_apply_commits_spends_once_and_undoes() {
    // AC-01 + AC-10: the only route from `CommitRequested` to the
    // project is this gate — commit spends exactly one delegated unit
    // and history navigation stays on the single writer.
    let mut rig = fixture_rig();
    let dir = tmpdir("delegated");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));

    let (pref, _op) = accepted(&mut rig, "t1", "g1");
    let receipt = gate.commit(&pref).expect("commit must land");
    assert_eq!(receipt.base_revision, ProjectRevision(1));
    assert_eq!(receipt.revision, ProjectRevision(2));
    assert_eq!(receipt.project, ProjectId::new(7));
    assert!(clip11_muted(&gate));
    assert_eq!(
        rig.store.grant(&grant_ref("g1")).unwrap().consumed_applies,
        1,
        "exactly one delegated unit binds at commit authorization"
    );

    // Undo/redo ride the same writer inside the fence — the commit is
    // not a dead end (AHG: revoke≠undo, undo is its own commit).
    assert!(gate.undo(&human()).unwrap());
    assert!(!clip11_muted(&gate));
    assert!(gate.redo(&human()).unwrap());
    assert!(clip11_muted(&gate));
}

#[test]
fn approve_to_apply_commits_only_with_verified_receipt() {
    // AC-05/09: an `ApproveToApply` commit clears the commit-stage
    // `ApprovalRequired` gate via the *consumed* receipt — re-verified
    // inside the fence, not trusted from accept time.
    let mut rig = fixture_rig();
    let dir = tmpdir("receipt");
    let (gate, _report, _store) = boot(&rig, &dir);
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

    let (pref, _op) = accepted_via_receipt(&mut rig, "t1", "g1");
    let receipt = gate.commit(&pref).expect("consumed receipt commits");
    assert_eq!(receipt.revision, ProjectRevision(2));
    assert!(clip11_muted(&gate));

    // A retry on the settled apply replays the recorded receipt.
    let replayed = gate.commit(&pref).expect("settled apply replays");
    assert_eq!(replayed, receipt);
}

#[test]
fn apply_awaiting_approval_is_denied_not_indeterminate() {
    // AC-09 edge: a pre-receipt apply is a clean refusal at the commit
    // boundary — never an undecidable verdict.
    let mut rig = fixture_rig();
    let dir = tmpdir("awaiting");
    let (gate, _report, _store) = boot(&rig, &dir);
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
    let outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    assert!(matches!(outcome, GatewayOutcome::ApprovalRequired { .. }));

    let err = gate
        .commit(&ProposalRef::new(&proposal).unwrap())
        .expect_err("awaiting receipt must deny");
    assert!(matches!(
        err.code,
        GatewayError::Denied {
            stage: AuthorizationStage::Commit,
            ..
        }
    ));
}

// ── AC-02/03: scheduled revoke/commit orderings ──────────────

#[test]
fn revoke_confirmed_before_commit_denies() {
    // REC-01/02 + AC-02: both sides parked at the fence; the revoke section
    // runs first → the commit observes the revoked grant.
    let mut rig = fixture_rig();
    let dir = tmpdir("revoke-first");
    let fence = ScheduledFence::default();
    let (gate, _report, _store) = boot_with(&rig, &dir, fence.clone());
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, op) = accepted(&mut rig, "t1", "g1");

    let gate = Arc::new(gate);
    let (c_gate, c_pref) = (gate.clone(), pref.clone());
    let c_fence = fence.clone();
    let commit_t =
        std::thread::spawn(move || c_fence.run("commit", move || c_gate.commit(&c_pref)));
    let r_gate = gate.clone();
    let r_fence = fence.clone();
    let revoke_t = std::thread::spawn(move || {
        r_fence.run("revoke", move || r_gate.revoke_grant(&grant_ref("g1")))
    });
    fence.wait_parked(2);

    // Revoke confirmed first — journal order is the durable order.
    fence.release("revoke");
    revoke_t.join().unwrap().unwrap();
    fence.release("commit");
    let err = commit_t
        .join()
        .unwrap()
        .expect_err("commit under a revoked grant must deny");
    assert!(matches!(
        err.code,
        GatewayError::Denied {
            stage: AuthorizationStage::Commit,
            ..
        }
    ));

    // Durable evidence: RevokeGrant precedes the Rejected outcome.
    let payloads = journal_payloads(&dir.join("commit.journal"));
    let revoke_idx = payloads
        .iter()
        .position(|p| matches!(p, JournalPayload::Policy(PolicyEvent::RevokeGrant(_))))
        .expect("revoke journaled");
    let denial_at = payloads
        .iter()
        .position(|p| matches!(p, JournalPayload::Rejected(_)))
        .expect("rejection journaled");
    assert!(revoke_idx < denial_at, "journal order is the durable order");
    assert!(!clip11_muted(&gate), "nothing applied");
    // The rejection is the recorded verdict for the operation.
    assert!(matches!(
        gate.reconcile(op),
        RetryOutcome::Settled(CommitOutcome::Rejected { .. })
    ));
}

#[test]
fn commit_confirmed_before_revoke_stands() {
    // REC-02/AC-03: commit section first → the mutation landed; a later
    // revoke does not un-commit (revocation is not auto-undo).
    let mut rig = fixture_rig();
    let dir = tmpdir("commit-first");
    let fence = ScheduledFence::default();
    let (gate, _report, _store) = boot_with(&rig, &dir, fence.clone());
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, _op) = accepted(&mut rig, "t1", "g1");

    let gate = Arc::new(gate);
    let (c_gate, c_pref) = (gate.clone(), pref);
    let c_fence = fence.clone();
    let commit_t =
        std::thread::spawn(move || c_fence.run("commit", move || c_gate.commit(&c_pref)));
    let r_gate = gate.clone();
    let r_fence = fence.clone();
    let revoke_t = std::thread::spawn(move || {
        r_fence.run("revoke", move || r_gate.revoke_grant(&grant_ref("g1")))
    });
    fence.wait_parked(2);

    fence.release("commit");
    let receipt = commit_t.join().unwrap().expect("commit stands");
    assert_eq!(receipt.revision, ProjectRevision(2));
    fence.release("revoke");
    revoke_t.join().unwrap().unwrap();

    assert!(
        rig.store
            .grant(&grant_ref("g1"))
            .unwrap()
            .grant
            .validity
            .revoked,
        "the revoke landed — after the commit"
    );
    assert!(clip11_muted(&gate), "revocation does not un-commit");
    // Manual recovery path: Undo is the operator's tool, not revoke.
    assert!(gate.undo(&human()).unwrap());
    assert!(!clip11_muted(&gate));
}

#[test]
fn concurrent_commits_single_writer_conflict() {
    // AC-04: an agent apply and a trusted-human manual commit
    // both assert rev1 — whoever's section runs first wins; the other
    // gets a typed conflict, durably.
    let mut rig = fixture_rig();
    let dir = tmpdir("two-writers");
    let fence = ScheduledFence::default();
    let (gate, _report, _store) = boot_with(&rig, &dir, fence.clone());
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, op) = accepted(&mut rig, "t1", "g1");

    let gate = Arc::new(gate);
    let (a_gate, a_pref) = (gate.clone(), pref);
    let a_fence = fence.clone();
    let agent_t = std::thread::spawn(move || a_fence.run("agent", move || a_gate.commit(&a_pref)));
    let m_gate = gate.clone();
    let m_fence = fence.clone();
    let manual_t = std::thread::spawn(move || {
        m_fence.run("manual", move || {
            m_gate.commit_manual(
                &human(),
                ProjectOp::SetProjectName {
                    name: "manual edit".to_owned(),
                },
                ProjectRevision(1),
            )
        })
    });
    fence.wait_parked(2);

    // Manual first → agent conflicts on the single writer's revision.
    fence.release("manual");
    let manual = manual_t.join().unwrap().expect("manual commit lands");
    assert_eq!(manual.revision, ProjectRevision(2));
    fence.release("agent");
    let err = agent_t
        .join()
        .unwrap()
        .expect_err("stale apply must conflict");
    assert_eq!(
        err.code,
        GatewayError::RevisionConflict {
            expected: 1,
            actual: 2
        }
    );
    assert!(matches!(
        gate.reconcile(op),
        RetryOutcome::Settled(CommitOutcome::Conflicted {
            expected: ProjectRevision(1),
            actual: ProjectRevision(2),
        })
    ));
    // Both verdicts are journal-ordered: manual commit, then conflict.
    let payloads = journal_payloads(&dir.join("commit.journal"));
    assert!(
        payloads
            .iter()
            .any(|p| matches!(p, JournalPayload::Conflicted(_))),
        "the conflict is durable evidence"
    );
}

// ── AC-06: idempotent retries ────────────────────────────────

#[test]
fn keyed_commit_retry_replays_the_receipt() {
    // AC-06: the same keyed apply retried at the gate replays
    // the recorded receipt — never a second mutation.
    let mut rig = fixture_rig();
    let dir = tmpdir("keyed-retry");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, _op) = accepted_keyed(&mut rig, "t1", "g1", "apply-key-1");

    let first = gate.commit(&pref).expect("commit lands");
    let second = gate.commit(&pref).expect("retry replays the receipt");
    assert_eq!(first, second, "the durable handle, not a new commit");
    assert_eq!(gate.live_revision(), ProjectRevision(2));
    assert_eq!(
        rig.store.grant(&grant_ref("g1")).unwrap().consumed_applies,
        1,
        "a replay never re-spends"
    );
    let commits = journal_payloads(&dir.join("commit.journal"))
        .iter()
        .filter(|p| matches!(p, JournalPayload::Committed(_)))
        .count();
    assert_eq!(commits, 1, "one durable commit for any number of retries");
}

#[test]
fn wire_level_keyed_conflict_rejects_different_payload() {
    // AC-07 wire surface: same key + different payload never reaches
    // the gate — GW-01's ledger rejects it at the boundary.
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
    let (p1, d1) = prepare_mute_agent(&mut rig, "t1", "g1");
    let mut req1 = apply_req(&p1, &d1, None, Some("t1"), Some("g1"));
    req1["idempotency_key"] = json!("shared-key");
    rig.gw.handle(&req1, agent()).unwrap();

    let (p2, d2) = prepare_mute_agent(&mut rig, "t1", "g1");
    let mut req2 = apply_req(&p2, &d2, None, Some("t1"), Some("g1"));
    req2["idempotency_key"] = json!("shared-key");
    let err = rig
        .gw
        .handle(&req2, agent())
        .expect_err("same key, different payload");
    assert_eq!(err.code, GatewayError::IdempotencyConflict);
}

// ── AC-10: the manual path ───────────────────────────────────

#[test]
fn manual_commit_is_human_only_and_journaled() {
    // AC-01: no grant, no proposals — the trusted human commits
    // directly; the agent is refused; everything is still fenced+journaled.
    let rig = fixture_rig();
    let dir = tmpdir("manual");
    let (gate, _report, mut store) = boot(&rig, &dir);
    // rig only supplies deps — an empty policy store works for manual.

    let err = gate
        .commit_manual(
            &agent(),
            ProjectOp::SetProjectName {
                name: "nope".to_owned(),
            },
            ProjectRevision(1),
        )
        .expect_err("agent cannot use the manual path");
    assert!(matches!(err.code, GatewayError::Denied { .. }));

    let receipt = gate
        .commit_manual(
            &human(),
            ProjectOp::SetProjectName {
                name: "renamed".to_owned(),
            },
            ProjectRevision(1),
        )
        .expect("human commits");
    assert_eq!(receipt.revision, ProjectRevision(2));
    assert!(gate.inspect(|c| c.live().name == "renamed"));

    let saved = gate.save(&human(), &mut store).expect("fenced save");
    assert_eq!(saved.revision, ProjectRevision(2));
    assert_eq!(store.head().unwrap().as_deref(), Some(saved.head.as_str()));
}

// ── AC-05/AC-09: commit-boundary revalidation ───────────────────

#[test]
fn expired_proposal_rejects_at_commit() {
    // The apply was accepted while live; the proposal TTL lapsing
    // before commit makes the prepared record unavailable — a
    // determined Rejected, never a commit of dead content.
    let mut rig = fixture_rig();
    let dir = tmpdir("expired");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, _op) = accepted(&mut rig, "t1", "g1");

    rig.clock.set(PolicyInstant::new(1_000 + PROPOSAL_TTL + 1));
    let err = gate.commit(&pref).expect_err("dead proposal cannot commit");
    assert!(matches!(err.code, GatewayError::ProposalUnavailable { .. }));
    assert!(!clip11_muted(&gate));
}

#[test]
fn commit_stage_evaluation_denies_out_of_scope_effects() {
    // AC-05: the decision the gate re-runs inside the fence is the same
    // effect-aware evaluation GW-04 proved — a write the shared MIDI
    // source lands (GW-04's `linked_midi_write_names_source_and_linked_
    // clips` proves that derivation) denies under a clips-only grant
    // at the commit-stage seam. End-to-end the narrow grant already
    // denies at prepare; this is the seam itself.
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
    // g2: identical except scope drops the linked MIDI source.
    let mut narrow = apply_grant(
        "g2",
        "t1",
        &["synvelis.clip.mute"],
        ApprovalPolicy::DelegatedApply,
    );
    narrow.scope = ResourceScope::Exact {
        resources: BTreeSet::from([clip11(), clip12(), clip13()]),
    };
    rig.store.issue_grant(narrow);
    let (pref, _op) = accepted(&mut rig, "t1", "g1");
    let view = rig.proposals.view(&pref).unwrap();

    // The write set a source-level linked-MIDI edit lands.
    let scoped = ScopedEffects {
        reads: Vec::new(),
        writes: vec![clip12(), source10()],
        creates: Vec::new(),
        deletes: Vec::new(),
        disclosures: Vec::new(),
        external_transfers: 0,
        unsupported: Vec::new(),
    };

    // Two GW-01-bound apply requests — same args, different grants.
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
    let decision = evaluator.evaluate_effects(
        &narrow_req,
        AuthorizationStage::Commit,
        &apply_desc(&rig),
        &scoped,
    );
    assert!(
        matches!(decision, AuthorizationDecision::Deny { .. }),
        "commit-stage evaluation denies the out-of-scope source write"
    );
    let decision = evaluator.evaluate_effects(
        full_req,
        AuthorizationStage::Commit,
        &apply_desc(&rig),
        &scoped,
    );
    assert!(
        matches!(
            decision,
            AuthorizationDecision::PermitForStage | AuthorizationDecision::ApprovalRequired
        ),
        "the same effects are inside the full grant"
    );
}

// ── REC-06/09/12: restart recovery ───────────────────────────

#[test]
fn restart_recovers_committed_state_and_verdict() {
    // REC-06/07/09: the commit record outlives the process — a restart
    // restores the committed project (store HEAD never saw it) and the
    // operation's verdict replays from the journal.
    let mut rig = fixture_rig();
    let dir = tmpdir("recover");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, op) = accepted_keyed(&mut rig, "t1", "g1", "retry-key");
    gate.commit(&pref).expect("commit lands");
    drop(gate);

    let (gate, report, policy, _store) = restart(&rig, &dir);
    assert_eq!(report.restored_revision, Some(ProjectRevision(2)));
    assert!(report.restored_from_journal);
    assert_eq!(report.recovered_revision, ProjectRevision(2));
    assert!(clip11_muted(&gate), "the committed state is restored");

    // The verdict replays — a retry can never re-apply.
    match gate.reconcile(op) {
        RetryOutcome::Settled(CommitOutcome::Committed(receipt)) => {
            assert_eq!(receipt.revision, ProjectRevision(2));
        }
        other => panic!("expected Settled(Committed), got {other:?}"),
    }
    // Policy replayed from the journal: grant issued, one unit consumed.
    let record = policy.grant(&grant_ref("g1")).expect("grant replayed");
    assert_eq!(record.consumed_applies, 1, "consumption is durable");
    // And the keyed retry replays the committed verdict by fingerprint.
    let apply_req_bound = rig
        .proposals
        .view(&pref)
        .and_then(|v| v.apply)
        .map(|a| a.request)
        .unwrap();
    let key = CommitGate::<MutexFence>::ledger_key_of(&apply_req_bound).unwrap();
    let fp = CommitGate::<MutexFence>::fingerprint_of(&apply_req_bound);
    assert!(matches!(
        gate.reconcile_key(&key, &fp),
        RetryOutcome::Settled(CommitOutcome::Committed(_))
    ));
    assert!(matches!(
        gate.reconcile_key(&key, "sha256:different-payload"),
        RetryOutcome::Settled(CommitOutcome::Rejected { .. })
    ));
}

#[test]
fn stale_proposal_can_never_commit_after_restart() {
    // AC-09: the proposal store is in-memory — a restart kills every
    // proposal, so nothing stale can reach the writer. The journal is
    // the only durable evidence and it reports the verdict instead.
    let mut rig = fixture_rig();
    let dir = tmpdir("stale");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, _op) = accepted(&mut rig, "t1", "g1");
    gate.commit(&pref).expect("commit lands");
    drop(gate);

    let (gate, report, _policy, _store) = restart(&rig, &dir);
    let err = gate
        .commit(&pref)
        .expect_err("post-restart proposal is gone");
    assert!(matches!(err.code, GatewayError::ProposalUnavailable { .. }));
    assert_eq!(
        report.recovered_revision,
        ProjectRevision(2),
        "the committed state — not a re-apply — is what survived"
    );
}

// ── REC-04/05/06/11: fault injection ────────────────────────────

#[test]
fn crash_after_authorize_leaves_nothing_committed() {
    // REC-04: process death between authorization and the mutation —
    // no commit record exists, the verdict is Indeterminate, and the
    // delegated unit is *not* durably spent (commit records are the
    // authority, so recovery neither double-counts nor loses it).
    let mut rig = fixture_rig();
    let dir = tmpdir("crash-authz");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, op) = accepted(&mut rig, "t1", "g1");

    let err = gate
        .commit_hooked(&pref, &mut CrashHook("authorize"))
        .expect_err("crash is indeterminate");
    assert!(matches!(err.code, GatewayError::Indeterminate { .. }));
    assert!(matches!(
        gate.reconcile(op),
        RetryOutcome::Settled(CommitOutcome::Indeterminate { .. })
    ));
    assert!(!clip11_muted(&gate));
    // The in-memory spend happened; it is *not* the durable truth.
    assert_eq!(
        rig.store.grant(&grant_ref("g1")).unwrap().consumed_applies,
        1
    );
    drop(gate);

    let (_gate, report, policy, _store) = restart(&rig, &dir);
    assert_eq!(
        report.recovered_revision,
        ProjectRevision(1),
        "nothing committed"
    );
    assert_eq!(report.commits_replayed, 0);
    assert_eq!(
        policy
            .grant(&grant_ref("g1"))
            .expect("grant replayed")
            .consumed_applies,
        1,
        "the authorized spend precedes its decision — the journaled \
         outcome carries it so a restart never refunds a spent unit"
    );
}

#[test]
fn crash_after_apply_reports_indeterminate_and_no_partial_state() {
    // REC-05+11: death between the in-memory mutation and the durable
    // record — the verdict is Indeterminate and recovery shows no
    // partially-applied content. REC-11's "media verified, then
    // registration crashed" maps here: verified source state survives,
    // the un-journaled mutation does not.
    let mut rig = fixture_rig();
    let dir = tmpdir("crash-apply");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, op) = accepted(&mut rig, "t1", "g1");

    let err = gate
        .commit_hooked(&pref, &mut CrashHook("apply"))
        .expect_err("crash is indeterminate");
    assert!(matches!(err.code, GatewayError::Indeterminate { .. }));
    drop(gate);

    let (gate, report, _policy, _store) = restart(&rig, &dir);
    assert_eq!(report.commits_replayed, 0);
    assert_eq!(report.recovered_revision, ProjectRevision(1));
    assert!(
        !clip11_muted(&gate),
        "an un-journaled mutation leaves no partial state"
    );
    // REC-11's surviving side: the verified media source is intact —
    // no orphan or torn registration.
    assert!(gate.inspect(|c| matches!(
        c.live().sources[&SourceId::new(9)].state,
        synvelis_project::SourceState::Verified
    )));
    assert!(matches!(
        gate.reconcile(op),
        RetryOutcome::Settled(CommitOutcome::Indeterminate { .. })
    ));
}

#[test]
fn crash_after_durable_record_recovers_committed() {
    // REC-06: the record fsync'd before the process died — the caller
    // saw Indeterminate but recovery proves the commit landed. The
    // durable truth beats the later Indeterminate marker.
    let mut rig = fixture_rig();
    let dir = tmpdir("crash-journal");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, op) = accepted(&mut rig, "t1", "g1");

    let err = gate
        .commit_hooked(&pref, &mut CrashHook("journal"))
        .expect_err("post-record crash is indeterminate");
    assert!(matches!(err.code, GatewayError::Indeterminate { .. }));
    drop(gate);

    let (gate, report, _policy, _store) = restart(&rig, &dir);
    assert_eq!(report.commits_replayed, 1);
    assert_eq!(report.recovered_revision, ProjectRevision(2));
    assert!(clip11_muted(&gate));
    // The honest recovery: committed truth, not the lost response.
    match gate.reconcile(op) {
        RetryOutcome::Settled(CommitOutcome::Committed(receipt)) => {
            assert_eq!(receipt.revision, ProjectRevision(2));
        }
        other => panic!("durable commit must win over the crash marker, got {other:?}"),
    }
}

#[test]
fn torn_revoke_tail_never_confirms_a_revocation() {
    // REC-03: a revocation whose durable write died mid-frame was never
    // confirmed — replay keeps the verified prefix and drops the tail.
    let rig = fixture_rig();
    let dir = tmpdir("torn-revoke");
    let journal_path = dir.join("commit.journal");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let next_seq = {
        let payloads = journal_payloads(&journal_path);
        u64::try_from(payloads.len()).unwrap()
    };
    drop(gate);

    // A real torn tail: the revoke's frame is cut short mid-record.
    let payload = serde_json::to_vec(&JournalPayload::Policy(PolicyEvent::RevokeGrant(
        grant_ref("g1"),
    )))
    .unwrap();
    let frame = journal::encode_frame(next_seq, &payload).unwrap();
    let mut f = fs::OpenOptions::new()
        .append(true)
        .open(&journal_path)
        .unwrap();
    f.write_all(&frame[..frame.len() - 7]).unwrap();
    f.sync_all().unwrap();
    drop(f);

    let (_gate, report, policy, _store) = restart(&rig, &dir);
    assert_eq!(report.stop, journal::StopReason::TornTail);
    assert!(report.truncated_bytes > 0);
    let record = policy.grant(&grant_ref("g1")).expect("grant replayed");
    assert!(
        !record.grant.validity.revoked,
        "a torn revoke was never confirmed"
    );
}

#[test]
fn durable_revoke_survives_restart() {
    // REC-01/08 positive side: a revocation the journal confirmed is never
    // lost — replay re-applies it into the fresh policy store.
    let rig = fixture_rig();
    let dir = tmpdir("durable-revoke");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    gate.revoke_grant(&grant_ref("g1")).unwrap();
    drop(gate);

    let (_gate, report, policy, _store) = restart(&rig, &dir);
    assert_eq!(report.policy_events_replayed, 3, "issue×2 + revoke");
    assert!(
        policy
            .grant(&grant_ref("g1"))
            .expect("grant replayed")
            .grant
            .validity
            .revoked,
        "a confirmed revoke survives"
    );
}

// ── REC-10: fenced saves ─────────────────────────────────────

#[test]
fn saves_publish_head_in_commit_order() {
    // REC-10: HEAD publication is fenced with commits — a save can
    // never rewind HEAD past a commit it followed.
    let mut rig = fixture_rig();
    let dir = tmpdir("saves");
    let (gate, _report, mut store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, _op) = accepted(&mut rig, "t1", "g1");
    gate.commit(&pref).unwrap();

    let first = gate.save(&human(), &mut store).unwrap();
    assert_eq!(store.head().unwrap().as_deref(), Some("r2.json"));
    assert_eq!(first.head, "r2.json");

    // A later commit (manual) and save publish the newer revision.
    gate.commit_manual(
        &human(),
        ProjectOp::SetProjectName {
            name: "v3".to_owned(),
        },
        ProjectRevision(2),
    )
    .unwrap();
    let second = gate.save(&human(), &mut store).unwrap();
    assert_eq!(second.revision, ProjectRevision(3));
    assert_eq!(store.head().unwrap().as_deref(), Some("r3.json"));
    drop(gate);

    // Restart sees HEAD at 3 — the fenced order held.
    let (_gate, report, _policy, _store) = restart(&rig, &dir);
    assert_eq!(report.saved_head, Some(ProjectRevision(3)));
    assert_eq!(report.recovered_revision, ProjectRevision(3));
    assert!(!report.restored_from_journal, "HEAD was already current");
}

// ── PoC B: durable policy store + fence markers ──────────────

#[test]
fn file_policy_store_replays_lifecycle() {
    // PoC B evidence: the policy log independently survives restart —
    // lifecycle events replay, consumption is durable, and a commit's
    // fence marker binds journal seq to the policy position observed.
    let dir = tmpdir("policy-b");
    let log = dir.join("policy.journal");
    let (store, _r) = FilePolicyStore::open(&log).unwrap();
    store
        .issue_task(AuthorizedTask {
            task: task_ref("t1"),
            principal: agent_principal(),
            project: project(),
            activation: Activation::UserIssued,
            validity: Validity::default(),
        })
        .unwrap();
    store.issue_grant(delegated_grant("g1", "t1")).unwrap();
    store.revoke_grant(&grant_ref("g1")).unwrap();
    let lsn = store.durable_lsn();
    drop(store);

    let (store, report) = FilePolicyStore::open(&log).unwrap();
    let record = store.grant(&grant_ref("g1")).expect("grant replayed");
    assert!(record.grant.validity.revoked, "revoke replayed durably");
    assert_eq!(u64::try_from(report.replay.records.len()).unwrap(), lsn);

    // The fence marker binds a commit's journal seq to its policy view.
    store.mark_fence(42, lsn).unwrap();
    drop(store);
    let (_store, report) = FilePolicyStore::open(&log).unwrap();
    assert_eq!(report.fences, vec![(42, lsn)]);
}

#[test]
fn commit_records_the_policy_position_it_observed() {
    // PoC B ordering proof: with the durable store as admin, the commit
    // record carries the policy LSN it authorized under and a fence
    // marker lands in the policy log.
    let mut rig = fixture_rig();
    let dir = tmpdir("policy-commit");
    let policy_path = dir.join("policy.journal");
    let (file_store, _r) = FilePolicyStore::open(&policy_path).unwrap();
    let file_store = Arc::new(file_store);

    let store_dir = dir.join("project");
    let mut store = seed_store(&store_dir);
    let oracle: Arc<dyn ResourceOracle> = Arc::new(LatestOracle {
        source: rig.source.clone(),
    });
    let deps = GateDeps {
        policy: file_store.clone(),
        admin: file_store.clone(),
        evaluator: GrantPolicy::new(file_store.clone(), rig.clock.clone(), oracle),
        proposals: rig.proposals.clone(),
        apply_capability: apply_desc(&rig),
    };
    let (gate, _report) = CommitGate::open(
        demo_project(),
        Box::new(CounterIdGenerator::new(9_000)),
        &mut store,
        dir.join("commit.journal"),
        deps,
        MutexFence::new(),
    )
    .unwrap();
    // The wire path evaluates rig.store; the commit path the durable
    // store — the grant lands in both (policy admin is not a wire API).
    issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, _op) = accepted(&mut rig, "t1", "g1");
    let receipt = gate.commit(&pref).unwrap();

    let commit_lsn = journal_payloads(&dir.join("commit.journal"))
        .iter()
        .find_map(|p| match p {
            JournalPayload::Committed(c) => c.policy_lsn,
            _ => None,
        })
        .expect("commit record carries the observed policy position");
    drop(gate);
    let (_s, report) = FilePolicyStore::open(&policy_path).unwrap();
    assert!(
        report.fences.contains(&(receipt.journal_seq, commit_lsn)),
        "the fence marker binds commit seq {} to policy lsn {}",
        receipt.journal_seq,
        commit_lsn
    );
}

/// `PoC B` restart: the durable policy store replays its own log —
/// gate replay must not re-apply lifecycle/consumption events
/// (double-count regression).
#[test]
fn durable_policy_admin_never_double_counts_consumption() {
    let mut rig = fixture_rig();
    let dir = tmpdir("policy-b-restart");
    let policy_path = dir.join("policy.journal");
    let (file_store, _r) = FilePolicyStore::open(&policy_path).unwrap();
    let file_store = Arc::new(file_store);
    let store_dir = dir.join("project");
    let mut store = seed_store(&store_dir);
    let oracle: Arc<dyn ResourceOracle> = Arc::new(LatestOracle {
        source: rig.source.clone(),
    });
    let deps = GateDeps {
        policy: file_store.clone(),
        admin: file_store.clone(),
        evaluator: GrantPolicy::new(file_store.clone(), rig.clock.clone(), oracle),
        proposals: rig.proposals.clone(),
        apply_capability: apply_desc(&rig),
    };
    let (gate, _report) = CommitGate::open(
        demo_project(),
        Box::new(CounterIdGenerator::new(9_000)),
        &mut store,
        dir.join("commit.journal"),
        deps,
        MutexFence::new(),
    )
    .unwrap();
    issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, op) = accepted(&mut rig, "t1", "g1");
    gate.commit(&pref).expect("commit lands");
    assert_eq!(
        file_store.grant(&grant_ref("g1")).unwrap().consumed_applies,
        1
    );
    drop(gate);
    drop(file_store);

    // Restart: the file store replays its own log (consumed=1); the
    // commit journal replays the Committed record with budget_spend.
    let (file_store, _r) = FilePolicyStore::open(&policy_path).unwrap();
    let file_store = Arc::new(file_store);
    let mut store = DirectoryBundleStore::create(&store_dir).unwrap();
    let oracle: Arc<dyn ResourceOracle> = Arc::new(LatestOracle {
        source: rig.source.clone(),
    });
    let deps = GateDeps {
        policy: file_store.clone(),
        admin: file_store.clone(),
        evaluator: GrantPolicy::new(file_store.clone(), rig.clock.clone(), oracle),
        proposals: rig.proposals.clone(),
        apply_capability: apply_desc(&rig),
    };
    let (gate, report) = CommitGate::open(
        demo_project(),
        Box::new(CounterIdGenerator::new(9_000)),
        &mut store,
        dir.join("commit.journal"),
        deps,
        MutexFence::new(),
    )
    .unwrap();
    assert_eq!(report.commits_replayed, 1);
    assert_eq!(report.recovered_revision, ProjectRevision(2));
    // The durable store is the consumption authority — one unit, not two.
    assert_eq!(
        file_store.grant(&grant_ref("g1")).unwrap().consumed_applies,
        1,
        "commit-journal replay must not re-consume what the policy log already restored"
    );
    assert!(matches!(
        gate.reconcile(op),
        RetryOutcome::Settled(CommitOutcome::Committed(_))
    ));
}
