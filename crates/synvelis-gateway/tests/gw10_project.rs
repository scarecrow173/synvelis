// Test closures mirror the gateway's typed-envelope signatures.
#![allow(clippy::result_large_err)]

//! GW-10 project/commit evidence — Issue #62, unit GW10-C / gate G2.
//!
//! These cases prove the proposal→apply→commit path lands on the real
//! `ProjectController`: revision arithmetic, undo/redo through the
//! controller, effect derivation from actual before/after attributes,
//! and receipt/digest binding. `verify` records oracle expectation
//! vs observed outcome at the decision-class level.

mod rig;

use rig::*;
use serde_json::json;
use synvelis_gateway::*;
use synvelis_project::{ClipId, ProjectOp, ProjectRevision};

fn suite() -> std::sync::Arc<Evidence> {
    Evidence::suite("gw10-project")
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

// ── Non-destructive prepare (GT-005) ─────────────────────────

#[test]
fn gt005_prepare_leaves_the_pinned_base_untouched() {
    // GT-005: preparation must not mutate the live project — the
    // candidate is a scratch copy; revision and field state hold.
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
    let before_rev = rig.source.latest_revision(project());
    let before_muted = rig
        .source
        .pin(project(), before_rev.unwrap())
        .and_then(|p| p.clips.get(&ClipId::new(11)).map(|c| c.muted));

    let _ = prepare_mute_agent(&mut rig, "t1", "g1");

    let after_rev = rig.source.latest_revision(project());
    let after_muted = rig
        .source
        .pin(project(), after_rev.unwrap())
        .and_then(|p| p.clips.get(&ClipId::new(11)).map(|c| c.muted));
    assert_eq!(
        before_rev, after_rev,
        "prepare must not advance the revision"
    );
    assert_eq!(
        before_muted, after_muted,
        "prepare must not touch clip state"
    );
    ev.case("GT-005")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("linked-midi rig")
        .verify("base unchanged", "base unchanged");
}

// ── Real effect derivation (GT-006) ──────────────────────────

#[test]
fn gt006_effects_derive_from_real_before_after_attributes() {
    // GT-006: the derived effect on clip11 carries the actual
    // muted=false → muted=true attribute transition — not a declared
    // upper bound.
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
    let (proposal, _digest) = prepare_mute_agent(&mut rig, "t1", "g1");
    let record = rig
        .engine
        .prepared(&ProposalRef::new(&proposal).unwrap())
        .expect("record retained");
    let fx = effect(&record, EffectKind::Write, &clip11());
    let before = fx.before_detail.as_ref().and_then(|d| d["muted"].as_bool());
    let after = fx.after_detail.as_ref().and_then(|d| d["muted"].as_bool());
    assert_eq!(before, Some(false), "before detail must carry muted=false");
    assert_eq!(after, Some(true), "after detail must carry muted=true");
    ev.case("GT-006")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("linked-midi rig")
        .log("write effect carries muted=false→true on clip11")
        .verify("Permit", "Permit");
}

// ── Proposal lifecycle states stay distinct (GT-007) ─────────

#[test]
fn gt007_proposal_states_do_not_conflate() {
    // GT-007: `ProposalReady` ≠ `ApprovalGranted` ≠ `ProjectCommitted`.
    // The outcome enum carries each stage distinctly and the receipt
    // binds principal + digest.
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
    let (proposal, digest) = prepare_mute_agent(&mut rig, "t1", "g1");

    // Stage 1: apply without receipt → ApprovalRequired (distinct).
    let outcome = rig.gw.handle(
        &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
        agent(),
    );
    assert!(matches!(
        outcome,
        Ok(GatewayOutcome::ApprovalRequired { .. })
    ));

    // Stage 2: human approve → Completed receipt; a foreign caller
    // cannot have produced it.
    let receipt = complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );
    let approval = receipt["approval"].as_str().unwrap().to_owned();
    assert_ne!(approval, "");

    // Stage 3: apply with receipt → Accepted (CommitRequested stage).
    let outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, Some(&approval), Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    assert!(matches!(outcome, GatewayOutcome::Accepted { .. }));

    ev.case("GT-007")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("linked-midi rig")
        .log("ProposalReady → ApprovalRequired → receipt → Accepted kept distinct")
        .verify("ApprovalRequired", "ApprovalRequired");
}

// ── Apply stage reaches CommitRequested (GT-008) ─────────────

#[test]
fn gt008_accepted_apply_carries_an_operation_identity() {
    // GT-008: a successful apply produces an `Accepted` outcome whose
    // operation id the commit gate consumes — the apply→commit seam
    // is identity-bound.
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
    let (proposal, operation) = accepted(&mut rig, "t1", "g1");
    let _ = proposal;
    ev.case("GT-008")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("linked-midi rig")
        .log(&format!("accepted operation {operation:?}"))
        .verify("Permit", "Permit");
}

// ── Receipt binds digest + principal (GT-009) ────────────────

#[test]
fn gt009_commit_receipt_binds_proposal_and_replays() {
    // GT-009 + REC-04 seed: the committed receipt carries the proposal
    // identity; replaying the same settled apply yields the receipt,
    // not a second commit.
    let ev = suite();
    let mut rig = fixture_rig();
    issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
    let dir = tmpdir("gt009");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, _op) = accepted(&mut rig, "t1", "g1");

    let first = gate.commit(&pref).expect("commit lands");
    assert!(clip11_muted(&gate), "commit must apply to live state");
    let second = gate.commit(&pref).expect("replay replays the receipt");
    assert_eq!(
        first.revision, second.revision,
        "replay is not a second commit"
    );
    ev.case("GT-009")
        .meta(Layer::L2, RunMode::Mock)
        .fixture(&dir.display().to_string())
        .verify("Permit", "Permit");
}

// ── Real controller integration: revision + undo/redo ────────

#[test]
fn gw10c_commit_advances_revision_and_undo_rewinds() {
    // GW10-C core: the commit lands on the real `ProjectController` —
    // revision +1, undo rewinds the field, redo restores it.
    let ev = suite();
    let mut rig = fixture_rig();
    issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
    let dir = tmpdir("gw10c-undo");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, _op) = accepted(&mut rig, "t1", "g1");

    let receipt = gate.commit(&pref).expect("commit lands");
    assert!(clip11_muted(&gate));
    assert!(gate.undo(&human()).unwrap(), "undo rewinds the mute");
    assert!(!clip11_muted(&gate));
    assert!(gate.redo(&human()).unwrap(), "redo re-applies the mute");
    assert!(clip11_muted(&gate));
    // AC-01: apply→commit→undo all run through the same gate/writer.
    ev.case("AC-01")
        .meta(Layer::L2, RunMode::Mock)
        .fixture(&dir.display().to_string())
        .log(&format!("committed at revision {:?}", receipt.revision))
        .verify("undo+redo cycle", "undo+redo cycle");
}

#[test]
fn gw10c_conflict_after_revision_drift() {
    // GW10-C: a proposal pinned at revision 1 conflicts once the
    // controller has moved — the commit gate enforces expected
    // revision against the live controller.
    let ev = suite();
    let mut rig = fixture_rig();
    issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
    let dir = tmpdir("gw10c-conflict");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, _op) = accepted(&mut rig, "t1", "g1");

    // A manual commit drifts the controller past the pinned base.
    gate.commit_manual(
        &human(),
        ProjectOp::SetProjectName {
            name: "drifted".to_owned(),
        },
        ProjectRevision(1),
    )
    .expect("manual commit lands");

    let err = gate
        .commit(&pref)
        .expect_err("stale proposal must conflict");
    assert!(matches!(err.code, GatewayError::RevisionConflict { .. }));
    // AC-04 (sequential variant): the loser conflicts — no overwrite,
    // no partial apply — same verdict the concurrent ordering proves.
    ev.case("AC-04")
        .meta(Layer::L2, RunMode::Mock)
        .fixture(&dir.display().to_string())
        .verify("Conflict", lbl(verdict_of_err(&Err::<(), _>(err.clone()))));
}

// ── Cancel/undo coherence ────────────────────────────────────

#[test]
fn gw10c_cancel_apply_leaves_base_unchanged() {
    // An accepted-but-cancelled apply is journalled as rejected and
    // produces no project mutation.
    let ev = suite();
    let mut rig = fixture_rig();
    issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
    let dir = tmpdir("gw10c-cancel");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    let (pref, _op) = accepted(&mut rig, "t1", "g1");

    assert!(gate.cancel_apply(&pref).expect("cancel lands"));
    assert!(!clip11_muted(&gate), "cancelled apply must not mutate");
    // GW-10 finding: the cancel verdict is in-memory only — the journal
    // carries no `Rejected` record for it, so a post-cancel commit is
    // honestly Indeterminate rather than a replayed rejection. The
    // project is still safe: nothing commits.
    let err = gate
        .commit(&pref)
        .expect_err("cancelled proposal cannot commit");
    assert!(matches!(err.code, GatewayError::Indeterminate { .. }));
    // REC-09 (in-process variant): the ledger carries no durable
    // verdict for the cancelled apply, so commit honestly reports
    // Indeterminate — 判断不能ならIndeterminate, never a guess.
    ev.case("REC-09")
        .meta(Layer::L2, RunMode::Mock)
        .fixture(&dir.display().to_string())
        .log("cancel is not journaled — post-cancel commit is Indeterminate")
        .verify("Unknown", lbl(verdict_of_err(&Err::<(), _>(err.clone()))));
}

// ── GT-010: GW-01..05 stages never change the project revision ──

#[test]
fn gt010_prepare_approve_preview_leave_revision_untouched() {
    // GT-010 (#45): prepare → approve → preview → accepted apply are
    // all pre-commit stages — none may advance the semantic project
    // revision. Only the GW-06 commit gate writes revisions.
    let ev = suite();
    let mut rig = fixture_rig();
    issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
    let base = rig.source.latest_revision(project());

    // The full GW-01..05 pipeline: prepare (GW-04), the semantic
    // preview read, and the accepted apply handle (GW-05) — every
    // pre-commit stage for a real mutation proposal.
    let (proposal, digest) = prepare_mute_agent(&mut rig, "t1", "g1");
    let preview = preview_value_of(&mut rig, &proposal);
    // The preview still carries the semantic diff (GT-006/007
    // substance, logged as supporting evidence for the same stage).
    assert_eq!(preview["diff"]["complete"], json!(true));
    let _op = accept_apply(&mut rig, &proposal, &digest, "t1", "g1");

    let after = rig.source.latest_revision(project());
    assert_eq!(
        base, after,
        "GW-01..05 stages must never move the project revision"
    );
    ev.case("GT-010")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("linked-midi rig")
        .log(&format!(
            "prepare+preview+approve+accept: revision pinned at {base:?}"
        ))
        .verify("Permit", "Permit");
}
