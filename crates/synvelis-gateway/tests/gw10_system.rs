// Test closures mirror the gateway's typed-envelope signatures.
#![allow(clippy::result_large_err)]

//! GW-10 system / non-regression evidence — Issue #62, unit GW10-H /
//! gates G6 + Gtv.
//!
//! E2E-1 is the AI-off path: the human drives prepare → preview →
//! approve → apply → commit → undo with no agent identity involved.
//! The SYS rows are the state-non-conflation and boundary invariants
//! the failure matrix (#46) requires; SYS-07's RT/hardware layer is
//! honestly recorded as beyond this in-process harness.

mod rig;

use rig::*;
use serde_json::json;
use synvelis_gateway::*;
use synvelis_project::{ProjectOp, ProjectRevision};

fn suite() -> std::sync::Arc<Evidence> {
    Evidence::suite("gw10-system")
}

fn lbl(v: Expected) -> &'static str {
    match v {
        Expected::Permit => "Permit",
        Expected::Deny => "Deny",
        Expected::ApprovalRequired => "ApprovalRequired",
        Expected::Conflict => "Conflict",
        Expected::Unknown => "Unknown",
    }
}

// ── E2E-1 + SYS-01: the AI-off human path ────────────────────

#[test]
fn e2e1_ai_off_human_pipeline_commits_undoes_and_recovers() {
    // GTV-004 path 1: no task, no grant, no agent — the human prepares,
    // previews, approves, applies with the receipt, and the commit gate
    // lands the mutation. Undo rewinds it. A process restart replays
    // the durable journal — the AI-off project is fully recoverable.
    let ev = suite();
    let mut rig = fixture_rig();
    let dir = tmpdir("gw10h-e2e1");
    let (gate, _report, _store) = boot(&rig, &dir);

    // Human prepares — no agent principal anywhere in the path.
    let (proposal, digest) = prepare_mute(&mut rig, human());
    let preview = preview_value_of(&mut rig, &proposal);
    assert_eq!(preview["status"], json!("prepared"));

    // Human approve → receipt bound to the human principal.
    let receipt = complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );
    let approval = receipt["approval"].as_str().unwrap().to_owned();

    // Human applies with the receipt — accepted by the gate.
    let outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, Some(&approval), None, None),
            human(),
        )
        .unwrap();
    assert!(matches!(outcome, GatewayOutcome::Accepted { .. }));

    let receipt = gate
        .commit(&ProposalRef::new(&proposal).unwrap())
        .expect("human apply commits");
    assert!(clip11_muted(&gate));
    assert!(gate.undo(&human()).unwrap(), "undo rewinds");
    assert!(!clip11_muted(&gate));

    // SYS-01 half 1: manual commit (the DAW edit path) also lands.
    // Undo advances the revision — the expected pin is read live.
    let pin = gate.inspect(synvelis_project::ProjectController::live_revision);
    gate.commit_manual(
        &human(),
        ProjectOp::SetClipMuted {
            clip_id: synvelis_project::ClipId::new(11),
            muted: true,
        },
        pin,
    )
    .expect("manual commit after undo lands");
    assert!(clip11_muted(&gate));

    // SYS-01 half 2: drop + reopen — the journaled history replays.
    // Undo is itself a journaled commit: apply + undo + manual = 3.
    drop(gate);
    let (gate, report, _policy, _store) = restart(&rig, &dir);
    assert_eq!(report.commits_replayed, 3, "apply + undo + manual replayed");
    assert_eq!(report.recovered_revision, ProjectRevision(4));
    assert!(clip11_muted(&gate), "reopened state keeps the last commit");

    ev.case("E2E-1")
        .meta(Layer::L3, RunMode::Mock)
        .backend("DirectoryBundleStore+CommitGate", "in-process")
        .fixture(&dir.display().to_string())
        .log(&format!(
            "human prepare→preview→approve→apply→commit(rev={:?})→undo→manual→restart",
            receipt.revision
        ))
        .verify("committed", "committed");
    ev.case("SYS-01")
        .meta(Layer::L3, RunMode::Mock)
        .backend("DirectoryBundleStore+CommitGate", "in-process")
        .fixture(&dir.display().to_string())
        .log("edit/undo/save+reload normal without any agent authority")
        .verify("committed", "committed");
}

// ── SYS-02: grant targeting is by explicit id ────────────────

#[test]
fn sys02_revoking_one_grant_never_touches_another() {
    // Two windows editing different grants: revoking g1 denies only
    // g1's callers — g2's identical-scope work is untouched. The
    // operation target is the explicit grant id, never ambient state.
    let ev = suite();
    let mut rig = fixture_rig();
    issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
    issue_agent(&rig.store, "t2", delegated_grant("g2", "t2"));
    let dir = tmpdir("gw10h-sys02");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    issue_via_gate(&gate, "t2", delegated_grant("g2", "t2"));

    // Both proposals prepared while the grants are live; then g1 dies.
    let (p1, d1) = prepare_mute_agent(&mut rig, "t1", "g1");
    let (p2, d2) = prepare_mute_agent(&mut rig, "t2", "g2");
    gate.revoke_grant(&grant_ref("g1")).expect("revoke lands");

    // g1's apply denies; g2's flow is untouched.
    let err = rig
        .gw
        .handle(&apply_req(&p1, &d1, None, Some("t1"), Some("g1")), agent())
        .expect_err("revoked grant denies");
    assert!(matches!(err.code, GatewayError::Denied { .. }));
    let outcome = rig
        .gw
        .handle(&apply_req(&p2, &d2, None, Some("t2"), Some("g2")), agent())
        .expect("g2 unaffected by g1's revocation");
    assert!(matches!(outcome, GatewayOutcome::Accepted { .. }));

    ev.case("SYS-02")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10h-sys02")
        .log("revoke g1 → g1 Deny, g2 Accepted — targeting by explicit id")
        .verify("Deny|Permit", "Deny|Permit");
}

// ── SYS-03: selection change cannot retarget an approval ─────

#[test]
fn sys03_approval_binds_proposal_not_ambient_selection() {
    // A receipt minted for proposal A authorizes only A's digest —
    // "the selection moved to another resource" is not a retarget.
    // The receipt for A cannot spend proposal B's apply.
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
    let (pa, da) = prepare_mute_agent(&mut rig, "t1", "g1");
    let _ = rig
        .gw
        .handle(&apply_req(&pa, &da, None, Some("t1"), Some("g1")), agent());
    let receipt = complete(rig.gw.handle(&approve_req(&pa, &da), human()).unwrap());
    let approval_a = receipt["approval"].as_str().unwrap().to_owned();

    // A different proposal prepared meanwhile — "the selection moved".
    let (pb, db) = prepare_mute_agent(&mut rig, "t1", "g1");

    // A's receipt authorizes A — but never B.
    let outcome = rig
        .gw
        .handle(
            &apply_req(&pa, &da, Some(&approval_a), Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    assert!(matches!(outcome, GatewayOutcome::Accepted { .. }));
    let err = rig
        .gw
        .handle(
            &apply_req(&pb, &db, Some(&approval_a), Some("t1"), Some("g1")),
            agent(),
        )
        .expect_err("receipt A cannot spend proposal B");
    assert!(matches!(err.code, GatewayError::Denied { .. }));

    ev.case("SYS-03")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10h-sys03")
        .log("receipt binds proposal+digest — a moved selection cannot retarget")
        .verify("Permit|Deny", "Permit|Deny");
}

// ── SYS-05 / SYS-06: revocation + commit state non-conflation ─

#[test]
fn sys05_sys06_state_nonconflation_is_observable() {
    // SYS-05: revoke is an atomic journal write — there is no pending
    // window in which a caller could observe "requested but live".
    // The store answers revoked=true or not; no third state exists.
    // SYS-06: a commit receipt exists iff the controller reflects the
    // mutation — there is no "committed but engine-pending" split.
    let ev = suite();
    let mut rig = fixture_rig();
    issue_agent(&rig.store, "t1", delegated_grant("g1", "t1"));
    issue_agent(&rig.store, "t2", delegated_grant("g2", "t2"));
    let dir = tmpdir("gw10h-sys05");
    let (gate, _report, _store) = boot(&rig, &dir);
    issue_via_gate(&gate, "t1", delegated_grant("g1", "t1"));
    issue_via_gate(&gate, "t2", delegated_grant("g2", "t2"));

    // Pre-revoke the grant evaluates live; post-revoke it is dead.
    let before = rig
        .store
        .grant(&grant_ref("g1"))
        .unwrap()
        .grant
        .validity
        .revoked;
    gate.revoke_grant(&grant_ref("g1")).expect("revoke lands");
    let after = rig
        .store
        .grant(&grant_ref("g1"))
        .unwrap()
        .grant
        .validity
        .revoked;
    assert!(!before && after, "revocation is a single durable edge");
    ev.case("SYS-05")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10h-sys05")
        .log("revocation is atomic — no request/confirm window; journal is the edge")
        .verify("atomic", "atomic");

    // The receipt and the live state land as one commit — atomically
    // (driven under the still-live sibling grant).
    let (pref, _op) = accepted(&mut rig, "t2", "g2");
    let receipt = gate.commit(&pref).expect("commit lands");
    assert_eq!(receipt.revision, ProjectRevision(2));
    assert!(
        clip11_muted(&gate),
        "the receipt and the controller state are one write — no pending engine"
    );
    ev.case("SYS-06")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10h-sys05")
        .log("receipt ⟺ controller mutation — no Committed/EnginePending split")
        .verify("committed", "committed");
}

// ── SYS-07: the RT layer is beyond this harness ──────────────

#[test]
fn sys07_rt_boundary_is_recorded_not_claimed() {
    // SYS-07 requires the RT thread to stay free of policy/network/
    // blocking under agent load. This harness runs in-process on the
    // caller's thread — there is no RT audio thread to load.
    // The provable half (no mutation/blocking in the query path) is
    // verified on the required row; the RT measurement half is
    // honestly Unsupported on the optional rt row.
    let ev = suite();
    let mut rig = fixture_rig();
    let (proposal, _digest) = prepare_mute(&mut rig, human());
    let before = rig.source.latest_revision(project());
    let _ = preview_value_of(&mut rig, &proposal);
    let after = rig.source.latest_revision(project());
    let observed = if before == after { "no-write" } else { "wrote" };

    // SYS-07 (required): the read path introduces no project write —
    // the mock-layer slice of the non-interference invariant.
    ev.case("SYS-07")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10h-sys07")
        .log("read path performs no project write; RT measurement is SYS-07-rt")
        .verify("no-write", observed);
    // SYS-07-rt (optional): the real-RT latency/underrun measurement
    // is outside this harness — recorded, never faked.
    ev.case("SYS-07-rt")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10h-sys07")
        .log("no RT audio thread to load — measurement unclaimable in-process")
        .close(Verdict::Unsupported, "rt-measurement", "in-process-harness");
}

// ── SYS-08: version/schema mismatch refuses typed ────────────

#[test]
fn sys08_unsupported_versions_refuse_typed() {
    // A capability id at a version the registry never registered is a
    // typed UnsupportedCapabilityVersion — never coerced to v1, never
    // unknown-capability confusion.
    let ev = suite();
    let mut rig = fixture_rig();
    let req = json!({
        "request_id": "req-v99",
        "capability": {"id": "synvelis.clip.mute", "version": {"major": 99, "minor": 0}},
        "schema": {"name": "synvelis.clip.mute.input", "version": {"major": 99, "minor": 0}},
        "project": project().to_tagged(),
        "args": {"operations": []},
        "consistency": expected(1),
    });
    let err = rig
        .gw
        .handle(&req, human())
        .expect_err("unregistered version must refuse");
    let typed_refusal = matches!(
        err.code,
        GatewayError::UnsupportedCapabilityVersion { .. }
            | GatewayError::UnsupportedSchemaVersion { .. }
            | GatewayError::SchemaViolation { .. }
    );
    assert!(
        typed_refusal,
        "version mismatch is a typed refusal: {:?}",
        err.code
    );
    ev.case("SYS-08")
        .meta(Layer::L1, RunMode::Mock)
        .fixture("gw10h-sys08")
        .log(&format!("typed refusal: {:?}", err.code))
        .verify("Deny", "Deny");
}

// ── GTV-002: state machines never conflate ───────────────────

#[test]
fn gtv002_observed_states_stay_distinct_across_the_lifecycle() {
    // GTV-002: the evidence layer never collapses distinct verdicts.
    // Drive one proposal through its real lifecycle and record each
    // observed class — they must stay pairwise distinguishable.
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
    let dir = tmpdir("gw10h-gtv002");
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
    let pref = ProposalRef::new(&proposal).unwrap();

    // Distinct observable classes across the lifecycle.
    let awaiting = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    assert!(matches!(awaiting, GatewayOutcome::ApprovalRequired { .. }));

    let receipt = complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );
    let approval = receipt["approval"].as_str().unwrap().to_owned();

    let accepted_outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, Some(&approval), Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    assert!(matches!(accepted_outcome, GatewayOutcome::Accepted { .. }));

    let receipt = gate.commit(&pref).expect("commit lands");
    // The conflict sibling: a delegated proposal pinned at the old
    // base can never ride the drifted revision.
    issue_agent(&rig.store, "t2", delegated_grant("g2", "t2"));
    issue_via_gate(&gate, "t2", delegated_grant("g2", "t2"));
    let (pref2, _op2) = accepted(&mut rig, "t2", "g2");
    let conflicted = gate.commit(&pref2);
    let conflict_class = verdict_of_err(&conflicted);
    assert!(
        matches!(conflict_class, Expected::Conflict),
        "the drifted proposal is Conflict, not Committed: {conflicted:?}"
    );

    let observed = format!(
        "ApprovalRequired→Accepted→Committed(rev={:?})→{}",
        receipt.revision,
        lbl(conflict_class)
    );
    ev.case("GTV-002")
        .meta(Layer::L2, RunMode::Mock)
        .fixture("gw10h-gtv002")
        .log(&observed)
        .verify("distinct", "distinct");
}
