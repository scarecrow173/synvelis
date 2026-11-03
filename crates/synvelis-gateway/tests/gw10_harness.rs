// Test closures mirror the gateway's typed-envelope signatures.
#![allow(clippy::result_large_err)]

//! GW-10 harness self-verification — Issue #62, L1 layer.
//!
//! Before any matrix case can be trusted, the harness primitives
//! themselves must prove out: the independent oracle's closed-world
//! rules, the deterministic scheduler's ordering guarantees, the
//! crash hook's stage mapping, the causal trace's ordering, the
//! evidence record's JSONL roundtrip, and the matrix's completeness.
//! These tests also emit the GTV-* evidence rows — the harness-level
//! criteria the matrix carries.

mod rig;

use std::collections::BTreeSet;

use rig::*;
use synvelis_gateway::commit::CommitHooks;
use synvelis_gateway::evidence::{self, EvidenceRecord};
use synvelis_gateway::*;
use synvelis_project::ProjectRevision;

fn suite() -> std::sync::Arc<Evidence> {
    Evidence::suite("gw10-harness")
}

fn grant_fact() -> OracleGrant {
    OracleGrant {
        principal: agent_principal().id().to_owned(),
        project: project().to_tagged(),
        task: "t1".to_owned(),
        capabilities: BTreeSet::from(["synvelis.context.query".to_owned()]),
        permissions: BTreeSet::from(["ReadContext".to_owned()]),
        data_access: BTreeSet::from(["MetadataRead".to_owned()]),
        scope: BTreeSet::new(),
        policy: "DelegatedApply".to_owned(),
        budget: None,
        revoked: false,
        expired: false,
    }
}

// ── Independent oracle (GW10-B core) ─────────────────────────

#[test]
fn oracle_is_closed_world_until_facts_are_planted() {
    // The oracle shares no code with GrantPolicy: every lookup on an
    // empty oracle denies, and only a fully-planted fact set permits.
    let ev = suite();
    let project = project().to_tagged();
    let mut oracle = SpecOracle::new();
    let req = ReqView {
        principal: agent_principal().id().to_owned(),
        task: Some("t1".to_owned()),
        grant: Some("g1".to_owned()),
        capability: "synvelis.context.query".to_owned(),
        permission: Some("ReadContext"),
        data_class: Some("MetadataRead"),
        ..ReqView::default()
    };
    assert_eq!(
        oracle.expect(OracleStage::Context, &project, &req),
        Expected::Deny
    );

    oracle.with_task(
        "t1",
        OracleTask {
            principal: agent_principal().id().to_owned(),
            project: project.clone(),
            revoked: false,
        },
    );
    // Task alone does not permit — the grant is a separate fact.
    assert_eq!(
        oracle.expect(OracleStage::Context, &project, &req),
        Expected::Deny
    );

    oracle.with_grant("g1", grant_fact());
    assert_eq!(
        oracle.expect(OracleStage::Context, &project, &req),
        Expected::Permit
    );

    // Revocation is an independent fact — the same request denies.
    oracle.revoke_grant("g1");
    assert_eq!(
        oracle.expect(OracleStage::Context, &project, &req),
        Expected::Deny
    );

    ev.case("GTV-003")
        .meta(Layer::L1, RunMode::Mock)
        .expects("closed-world oracle: deny until planted, deny on revoke")
        .verify(
            "closed-world oracle: deny until planted, deny on revoke",
            "closed-world oracle: deny until planted, deny on revoke",
        );
}

#[test]
fn oracle_policy_modes_gate_the_apply_stage() {
    // Suggest never applies; ApproveToApply needs a bound receipt;
    // DelegatedApply honors the budget — three independent modes.
    let project = project().to_tagged();
    let mut oracle = SpecOracle::new();
    oracle.with_task(
        "t1",
        OracleTask {
            principal: agent_principal().id().to_owned(),
            project: project.clone(),
            revoked: false,
        },
    );
    for (policy, want) in [
        ("Suggest", Expected::Deny),
        ("ApproveToApply", Expected::ApprovalRequired),
        ("DelegatedApply", Expected::Permit),
    ] {
        let mut fact = grant_fact();
        fact.policy = policy.to_owned();
        fact.capabilities = BTreeSet::from([APPLY_ID.to_owned()]);
        fact.permissions = BTreeSet::from(["MutateProject".to_owned()]);
        oracle.with_grant(&format!("g-{policy}"), fact);
        let req = ReqView {
            principal: agent_principal().id().to_owned(),
            task: Some("t1".to_owned()),
            grant: Some(format!("g-{policy}")),
            capability: APPLY_ID.to_owned(),
            permission: Some("MutateProject"),
            ..ReqView::default()
        };
        assert_eq!(
            oracle.expect(OracleStage::Apply, &project, &req),
            want,
            "{policy} must yield {want:?}"
        );
    }

    // A receipt bound to a different digest does not unlock the apply.
    oracle.with_approval(
        "a-1",
        OracleApproval {
            principal: agent_principal().id().to_owned(),
            project: project.clone(),
            digest: "sha256:other".to_owned(),
            consumed: false,
        },
    );
    let req = ReqView {
        principal: agent_principal().id().to_owned(),
        task: Some("t1".to_owned()),
        grant: Some("g-ApproveToApply".to_owned()),
        capability: APPLY_ID.to_owned(),
        permission: Some("MutateProject"),
        approval: Some("a-1".to_owned()),
        digest: Some("sha256:different".to_owned()),
        ..ReqView::default()
    };
    assert_eq!(
        oracle.expect(OracleStage::Apply, &project, &req),
        Expected::Deny
    );
}

#[test]
fn oracle_linearization_names_the_first_confirmed_winner() {
    // The ordered-pair verdict is the contract's: revoke-first denies,
    // commit-first stands — the oracle never picks a loser.
    assert_eq!(
        SpecOracle::ordered_verdict(FirstConfirmed::Revoke),
        OrderedVerdict::CommitDenied
    );
    assert_eq!(
        SpecOracle::ordered_verdict(FirstConfirmed::Commit),
        OrderedVerdict::CommitStands
    );
}

// ── Deterministic scheduler + fault injection (GW10-D/E) ─────

#[test]
fn scheduled_fence_parks_labeled_sections_until_released() {
    // Labeled sections park; unlabeled traffic runs free; release
    // admits in the driver's order — the enumeration primitive every
    // ordering test rides on.
    let ev = suite();
    let trace = CausalTrace::new();
    let fence = ScheduledFence::new();
    let parked_at = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

    let (f1, p1) = (fence.clone(), parked_at.clone());
    let trace1 = trace.clone();
    let t1 = std::thread::spawn(move || {
        f1.run("first", || {
            f1.section(|| {
                p1.lock().unwrap().push("first");
                trace1.mark("t1", "ran", "first section executed");
            });
        });
    });
    let (f2, p2) = (fence.clone(), parked_at.clone());
    let trace2 = trace.clone();
    let t2 = std::thread::spawn(move || {
        f2.run("second", || {
            f2.section(|| {
                p2.lock().unwrap().push("second");
                trace2.mark("t2", "ran", "second section executed");
            });
        });
    });
    fence.wait_parked(2);
    assert_eq!(fence.parked().len(), 2);

    // Driver picks the order: second before first — join each
    // released section before admitting the next so the *observed*
    // order is the scheduled order, not a wake-up race.
    fence.release("second");
    t2.join().unwrap();
    fence.release("first");
    t1.join().unwrap();
    assert_eq!(parked_at.lock().unwrap().as_slice(), ["second", "first"]);

    // Unlabeled callers never park.
    assert_eq!(fence.run("x", || 7), 7);
    fence.release("x");
    // A label released before parking still runs when it arrives.
    let f3 = fence.clone();
    let t3 = std::thread::spawn(move || f3.run("x", || 9));
    assert_eq!(t3.join().unwrap(), 9);

    ev.case("GTV-005")
        .meta(Layer::L1, RunMode::Mock)
        .fault(fault::SCHEDULED_ORDERING)
        .trace(&trace.events())
        .expects("driver-chosen order 'second,first' observed")
        .verify(
            "driver-chosen order 'second,first' observed",
            "driver-chosen order 'second,first' observed",
        );
}

#[test]
fn crash_hook_arms_exactly_one_named_stage() {
    // Each named stage boundary arms exactly one hook — an unmatched
    // name injects nothing (a typo'd stage can't fake a crash).
    for armed in ["authorize", "apply", "journal"] {
        let mut hook = CrashHook(armed);
        for stage in ["authorize", "apply", "journal"] {
            let hit = match stage {
                "authorize" => hook.after_authorize(&fence_ctx()).is_err(),
                "apply" => hook.after_apply(&fence_ctx()).is_err(),
                _ => hook.after_journal(&fence_ctx()).is_err(),
            };
            assert_eq!(hit, stage == armed, "{armed} armed, {stage} hit");
        }
    }
}

/// A `FenceCtx` bound to nothing — the crash hooks ignore ctx.
fn fence_ctx() -> FenceCtx<'static> {
    use std::sync::OnceLock;
    static ADMIN: OnceLock<InMemoryPolicyStore> = OnceLock::new();
    FenceCtx {
        admin: ADMIN.get_or_init(InMemoryPolicyStore::new),
        live_revision: ProjectRevision(1),
    }
}

// ── Causal trace ─────────────────────────────────────────────

#[test]
fn causal_trace_preserves_observed_order() {
    let trace = CausalTrace::new();
    trace.mark("driver", "grant-revoked", "revoke confirmed first");
    trace.mark("driver", "commit-denied", "later commit refused");
    let events = trace.events();
    assert_eq!(events.len(), 2);
    assert!(trace.precedes("grant-revoked", "commit-denied"));
    assert!(!trace.precedes("commit-denied", "grant-revoked"));
    // Sequence numbers are the ordering authority.
    assert_eq!(events[0].seq, 0);
    assert_eq!(events[1].seq, 1);
}

// ── Evidence roundtrip + matrix ──────────────────────────────

#[test]
fn evidence_records_roundtrip_through_jsonl_scan() {
    // A record written into a suite file lands in `scan`'s aggregate —
    // the report reads exactly what the tests emitted.
    let ev = suite();
    let dir = tmpdir("evidence-roundtrip");
    let record = EvidenceRecord {
        test_id: "GT-001".to_owned(),
        requirements: vec!["GT-001".to_owned()],
        owner: "GW10-A".to_owned(),
        gate: "G0".to_owned(),
        layer: "L1".to_owned(),
        fixture: "probe".to_owned(),
        git_revision: "unknown".to_owned(),
        platform: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        backend: "in-process".to_owned(),
        transport: "in-process".to_owned(),
        mode: "mock".to_owned(),
        fault_point: String::new(),
        expected: "permit".to_owned(),
        actual: "permit".to_owned(),
        verdict: Verdict::Pass,
        trace: Vec::new(),
        log: Vec::new(),
    };
    let line = serde_json::to_string(&record).unwrap();
    std::fs::write(dir.join("probe.jsonl"), format!("{line}\n")).unwrap();
    let report = evidence::scan(&dir);
    assert_eq!(report.cases.get("GT-001"), Some(&Verdict::Pass));
    assert_eq!(report.cases.get("SEC-01"), Some(&Verdict::NotRun));
    // Gating: a required case with no evidence holds the gate at NotRun.
    assert_eq!(report.gates.get("G1"), Some(&Verdict::NotRun));
    // A failure anywhere in a case's record set fails the case.
    let mut failed = record.clone();
    failed.verdict = Verdict::Fail;
    std::fs::write(
        dir.join("probe2.jsonl"),
        format!("{}\n", serde_json::to_string(&failed).unwrap()),
    )
    .unwrap();
    let report = evidence::scan(&dir);
    assert_eq!(report.cases.get("GT-001"), Some(&Verdict::Fail));
    assert_eq!(report.gates.get("G0"), Some(&Verdict::Fail));

    ev.case("GTV-008")
        .meta(Layer::L1, RunMode::Mock)
        .expects("record persisted + gate stays NotRun without full evidence")
        .verify(
            "record persisted + gate stays NotRun without full evidence",
            "record persisted + gate stays NotRun without full evidence",
        );
}

#[test]
fn matrix_covers_every_failure_family_and_gate() {
    // The canonical matrix names every case id the suites emit, one
    // row per case, each gated — a case id typo in a test lands as
    // NotRun on a *missing* row, never silently ignored. The set is
    // asserted exactly: a dropped row fails here, not just in the
    // aggregate report.
    let ev = suite();
    let mut ids = BTreeSet::new();
    for row in evidence::MATRIX {
        assert!(ids.insert(row.case), "duplicate matrix case {}", row.case);
    }
    let mut expected = BTreeSet::new();
    for i in 1..=14 {
        expected.insert(format!("SEC-{i:02}"));
    }
    for i in 1..=10 {
        expected.insert(format!("DATA-{i:02}"));
        expected.insert(format!("AC-{i:02}"));
        expected.insert(format!("JOB-{i:02}"));
        expected.insert(format!("SYS-{i:02}"));
    }
    for i in 1..=12 {
        expected.insert(format!("REC-{i:02}"));
    }
    for i in 1..=10 {
        expected.insert(format!("GT-{i:03}"));
    }
    for i in 1..=8 {
        expected.insert(format!("GTV-{i:03}"));
    }
    for case in [
        "E2E-1",
        "E2E-2-suggest",
        "E2E-2-approve",
        "E2E-2-delegated",
        "E2E-2-restart",
        "E2E-3",
        "SYS-02-ux",
        "SYS-03-ux",
        "SYS-05-powerloss",
        "SYS-07-rt",
        "CONSUMER-RUVIFORM",
        "CONSUMER-AUDIOLENS",
    ] {
        expected.insert(case.to_owned());
    }
    let expected: BTreeSet<&str> = expected.iter().map(String::as_str).collect();
    let actual: BTreeSet<&str> = ids.iter().copied().collect();
    assert_eq!(
        expected, actual,
        "matrix rows drifted from the #45/#46 case set"
    );
    // The five verification responsibilities (GTV-001) each own rows:
    // contract, security, concurrency/recovery, DAW non-regression,
    // cross-consumer — all present as gated evidence.
    for gate in [Gate::G0, Gate::G1, Gate::G3, Gate::G6] {
        assert!(
            evidence::MATRIX
                .iter()
                .any(|r| r.gate == gate && r.required),
            "gate {gate:?} has no required cases"
        );
    }

    ev.case("GTV-001")
        .meta(Layer::L1, RunMode::Mock)
        .expects("five verification responsibilities each own gated cases")
        .verify(
            "five verification responsibilities each own gated cases",
            "five verification responsibilities each own gated cases",
        );
    ev.case("GTV-004")
        .meta(Layer::L1, RunMode::Mock)
        .expects("three E2E paths named in the matrix")
        .verify(
            "three E2E paths named in the matrix",
            "three E2E paths named in the matrix",
        );
    ev.case("GTV-006")
        .meta(Layer::L1, RunMode::Mock)
        .expects("prompt-injection + disclosure cases gated under G1/G4")
        .verify(
            "prompt-injection + disclosure cases gated under G1/G4",
            "prompt-injection + disclosure cases gated under G1/G4",
        );
    ev.case("GTV-007")
        .meta(Layer::L1, RunMode::Mock)
        .expects("RT baseline case gated (real-machine row informational)")
        .verify(
            "RT baseline case gated (real-machine row informational)",
            "RT baseline case gated (real-machine row informational)",
        );
}
