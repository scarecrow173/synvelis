// Test closures mirror the gateway's typed-envelope signatures.
#![allow(clippy::result_large_err)]

//! GW-10 persistence/recovery evidence — Issue #62, unit GW10-E /
//! gate G3.
//!
//! Crash points are injected through the production [`CrashHook`]
//! seam; recovery is the real `CommitGate::open` replay over the same
//! durable journal + project store — no in-memory carry-over. Each
//! case records the fault point and the recovered verdict.

mod rig;

use std::io::Write;

use rig::*;
use synvelis_gateway::*;
use synvelis_project::{ProjectOp, ProjectRevision, ProjectStore};

fn suite() -> std::sync::Arc<Evidence> {
    Evidence::suite("gw10-persistence")
}

// ── REC-07: crash during consumption/commit ──────────────────

#[test]
fn rec07_crash_after_authorize_preserves_spend_consistency() {
    // REC-07: death after authorization (the delegated unit spent)
    // but before the durable outcome — Indeterminate at call time;
    // recovery neither double-counts nor refunds the spend.
    let ev = suite();
    let mut rig = fixture_rig();
    issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
    let dir = tmpdir("gw10e-rec07");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, op) = accepted(&mut rig, "t1", "g1");

    let err = gate
        .commit_hooked(&pref, &mut CrashHook("authorize"))
        .expect_err("crash is indeterminate");
    assert!(matches!(err.code, GatewayError::Indeterminate { .. }));
    assert!(!clip11_muted(&gate));
    drop(gate);

    let (gate, report, policy, _store) = restart(&rig, &dir);
    assert_eq!(report.commits_replayed, 0);
    assert_eq!(report.recovered_revision, ProjectRevision(1));
    assert_eq!(
        policy
            .grant(&grant_ref("g1"))
            .expect("grant replayed")
            .consumed_applies,
        1,
        "the journaled outcome carries the spend — no refund, no double"
    );
    assert!(matches!(
        gate.reconcile(op),
        RetryOutcome::Settled(CommitOutcome::Indeterminate { .. })
    ));
    // AC-10: the delegated unit's durable authority is the journaled
    // outcome — the in-memory spend alone proves nothing.
    ev.case("AC-10")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10e-rec07")
        .fault(fault::AFTER_AUTHORIZE)
        .verify("spend durable at 1", "spend durable at 1");
    ev.case("REC-07")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10e-rec07")
        .fault(fault::AFTER_AUTHORIZE)
        .log("spend durable at 1, commit absent, reconcile Indeterminate")
        .verify("Unknown", "Unknown");
}

// ── REC-08: durable revoke ordering across restart ───────────

#[test]
fn rec08_confirmed_revoke_survives_torn_tail_does_not() {
    // REC-08 both halves: a revoke the journal confirmed replays into
    // the fresh policy store; a revoke whose frame died mid-write was
    // never confirmed — replay keeps the verified prefix only.
    let ev = suite();

    // Positive: confirmed revoke persists.
    let rig = fixture_rig();
    let dir = tmpdir("gw10e-rec08a");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    gate.revoke_grant(&grant_ref("g1")).unwrap();
    drop(gate);
    let (_gate, report, policy, _store) = restart(&rig, &dir);
    assert!(
        policy
            .grant(&grant_ref("g1"))
            .expect("grant replayed")
            .grant
            .validity
            .revoked
    );
    ev.case("REC-08")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10e-rec08a")
        .log(&format!(
            "confirmed revoke replayed ({} policy events)",
            report.policy_events_replayed
        ))
        .verify("revoked", "revoked");

    // Negative: torn revoke frame is not a confirmed revocation.
    let rig = fixture_rig();
    let dir = tmpdir("gw10e-rec08b");
    let journal_path = dir.join("commit.journal");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let next_seq = u64::try_from(journal_payloads(&journal_path).len()).unwrap();
    drop(gate);

    // Append a torn RevokeGrant frame — cut mid-record.
    let payload = serde_json::to_vec(&JournalPayload::Policy(PolicyEvent::RevokeGrant(
        grant_ref("g1"),
    )))
    .unwrap();
    let frame = journal::encode_frame(next_seq, &payload).unwrap();
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&journal_path)
        .unwrap();
    f.write_all(&frame[..frame.len() - 7]).unwrap();
    f.sync_all().unwrap();
    drop(f);

    let (_gate, report, policy, _store) = restart(&rig, &dir);
    assert_eq!(report.stop, journal::StopReason::TornTail);
    assert!(report.truncated_bytes > 0);
    assert!(
        !policy
            .grant(&grant_ref("g1"))
            .expect("grant replayed")
            .grant
            .validity
            .revoked,
        "a torn revoke was never confirmed"
    );
    ev.case("REC-08")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10e-rec08b")
        .fault(fault::TORN_JOURNAL_TAIL)
        .log("unconfirmed revoke frame dropped at replay")
        .verify("not-revoked", "not-revoked");
}

// ── REC-09: crash at the commit/recovery boundary ────────────

#[test]
fn rec09_crash_points_recover_committed_or_indeterminate_never_torn() {
    // REC-09: enumerate the three crash points — after authorization
    // (nothing lands), after apply (no partial state), after the
    // durable record (committed truth wins). Each verdict is the
    // honest one for its point.
    let ev = suite();
    for (point, committed) in [("authorize", false), ("apply", false), ("journal", true)] {
        let mut rig = fixture_rig();
        issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
        let dir = tmpdir(&format!("gw10e-rec09-{point}"));
        let (gate, _report, _store) = boot(&rig, &dir);
        issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
        let (pref, op) = accepted(&mut rig, "t1", "g1");

        let err = gate
            .commit_hooked(&pref, &mut CrashHook(point))
            .expect_err("crash is indeterminate");
        assert!(matches!(err.code, GatewayError::Indeterminate { .. }));
        drop(gate);

        let (gate, report, _policy, _store) = restart(&rig, &dir);
        if committed {
            assert_eq!(report.commits_replayed, 1, "{point}: commit replayed");
            assert_eq!(report.recovered_revision, ProjectRevision(2));
            assert!(clip11_muted(&gate), "{point}: mutation recovered");
            assert!(matches!(
                gate.reconcile(op),
                RetryOutcome::Settled(CommitOutcome::Committed(_))
            ));
            // AC-08: the durable commit record is what recovery trusts.
            ev.case("AC-08")
                .meta(Layer::L2, RunMode::Mock)
                .fixture(&format!("gw10e-rec09-{point}"))
                .fault(fault::AFTER_JOURNAL)
                .verify("committed", "committed");
        } else {
            assert_eq!(report.commits_replayed, 0, "{point}: nothing replayed");
            assert_eq!(report.recovered_revision, ProjectRevision(1));
            assert!(!clip11_muted(&gate), "{point}: no partial state");
            assert!(matches!(
                gate.reconcile(op),
                RetryOutcome::Settled(CommitOutcome::Indeterminate { .. })
            ));
        }
        let point_label = match point {
            "authorize" => fault::AFTER_AUTHORIZE,
            "apply" => fault::AFTER_APPLY,
            _ => fault::AFTER_JOURNAL,
        };
        ev.case("REC-09")
            .meta(Layer::L2, RunMode::Mock)
            .fixture(&format!("gw10e-rec09-{point}"))
            .fault(point_label)
            .log(&format!(
                "crash at {point} → {}",
                if committed {
                    "committed on recovery"
                } else {
                    "indeterminate, no partial state"
                }
            ))
            .verify("honest", "honest");
    }
}

// ── REC-10: fenced saves never rewind HEAD ───────────────────

#[test]
fn rec10_saves_publish_head_in_commit_order() {
    let ev = suite();
    let mut rig = fixture_rig();
    issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
    let dir = tmpdir("gw10e-rec10");
    let (gate, _report, mut store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, _op) = accepted(&mut rig, "t1", "g1");
    gate.commit(&pref).unwrap();

    gate.save(&human(), &mut store).unwrap();
    assert_eq!(store.head().unwrap().as_deref(), Some("r2.json"));
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

    let (_gate, report, _policy, _store) = restart(&rig, &dir);
    assert_eq!(report.saved_head, Some(ProjectRevision(3)));
    assert_eq!(report.recovered_revision, ProjectRevision(3));
    assert!(!report.restored_from_journal);
    ev.case("REC-10")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10e-rec10")
        .log(&format!(
            "HEAD r2→r3 in commit order (saved_head {:?})",
            report.saved_head
        ))
        .verify("ordered", "ordered");
}

// ── REC-11: verified media survives an uncommitted registration ──

#[test]
fn rec11_crash_after_apply_leaves_verified_source_and_no_clip() {
    // REC-11: "media verified, registration crashed" — the verified
    // source survives recovery; the un-journaled clip mutation does
    // not, so no invalid clip ever references it.
    let ev = suite();
    let mut rig = fixture_rig();
    issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
    let dir = tmpdir("gw10e-rec11");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, _op) = accepted(&mut rig, "t1", "g1");

    let err = gate
        .commit_hooked(&pref, &mut CrashHook("apply"))
        .expect_err("crash is indeterminate");
    assert!(matches!(err.code, GatewayError::Indeterminate { .. }));
    drop(gate);

    let (gate, report, _policy, _store) = restart(&rig, &dir);
    assert_eq!(report.commits_replayed, 0);
    assert!(!clip11_muted(&gate), "un-journaled mutation is gone");
    // The verified source the clip would have pointed at is intact —
    // recoverable orphan, never a torn registration.
    assert!(gate.inspect(|c| matches!(
        c.live().sources[&synvelis_project::SourceId::new(9)].state,
        synvelis_project::SourceState::Verified
    )));
    ev.case("REC-11")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10e-rec11")
        .fault(fault::AFTER_APPLY)
        .log("verified media survives; un-journaled clip never registered")
        .verify("Unknown", "Unknown");
}

// ── REC-12: stale proposal after restart re-evaluates dead ───

#[test]
fn rec12_stale_proposal_never_commits_after_restart() {
    // REC-12: the proposal store is in-memory — after restart the
    // stale reference resolves to nothing; the committed state is
    // what survived, not a re-apply.
    let ev = suite();
    let mut rig = fixture_rig();
    issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
    let dir = tmpdir("gw10e-rec12");
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
    assert_eq!(report.recovered_revision, ProjectRevision(2));
    ev.case("REC-12")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10e-rec12")
        .log("stale proposal re-evaluates dead against the fresh store")
        .verify("Deny", "Deny");
}

// ── SYS-04 / AC-09: expired proposal rejects deterministically ──

#[test]
fn sys04_expired_proposal_rejects_not_applies() {
    // SYS-04 + AC-09: a proposal whose TTL lapsed before commit is a
    // determined rejection — dead content never commits.
    let ev = suite();
    let mut rig = fixture_rig();
    issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
    let dir = tmpdir("gw10e-rec04b");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, _op) = accepted(&mut rig, "t1", "g1");

    rig.clock.set(PolicyInstant::new(1_000 + PROPOSAL_TTL + 1));
    let err = gate.commit(&pref).expect_err("dead proposal cannot commit");
    assert!(matches!(err.code, GatewayError::ProposalUnavailable { .. }));
    assert!(!clip11_muted(&gate));
    ev.case("SYS-04")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10e-sys04")
        .log("expired proposal is a determined rejection")
        .verify("Deny", "Deny");
    // AC-09's awaiting/expired side — the same determined rejection.
    ev.case("AC-09")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10e-sys04")
        .verify("Deny", "Deny");
}
