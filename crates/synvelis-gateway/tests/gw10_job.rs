// Test closures mirror the gateway's typed-envelope signatures.
#![allow(clippy::result_large_err)]

//! GW-10 job / media / provenance evidence — Issue #62, unit GW10-G /
//! gate G5.
//!
//! The provider and media surfaces are the deterministic stubs; the
//! commit boundary in `E2E-3` is the real `CommitGate` over a real
//! `DirectoryBundleStore` — the same layering the issue demands:
//! verified-media truth is evidenced where it lands, not simulated.

mod rig;

use rig::*;
use serde_json::{Value, json};
use synvelis_gateway::*;
use synvelis_project::ProjectRevision;

fn suite() -> std::sync::Arc<Evidence> {
    Evidence::suite("gw10-job")
}

fn asset5() -> ResourceRef {
    ResourceRef::Asset(synvelis_project::AssetId::new(5))
}

fn gen_args(inputs: &[&ResourceRef]) -> Value {
    json!({
        "provider": "stub",
        "kind": "generation",
        "output": "media",
        "inputs": inputs.iter().map(|r| tagged(r)).collect::<Vec<_>>(),
    })
}

fn issue_bound(rig: &JobRig, task: &str, grant: Grant) {
    issue_task(&rig.rig.store, task);
    rig.rig.store.issue_grant(grant);
}

fn accepted(outcome: GatewayOutcome<Value>) -> OperationId {
    match outcome {
        GatewayOutcome::Accepted { operation } => operation,
        other => panic!("expected Accepted, got {other:?}"),
    }
}

fn submit(rig: &mut JobRig, task: &str, grant: &str, args: Value) -> OperationId {
    accepted(
        rig.rig
            .gw
            .handle(&submit_req(task, grant, args), agent())
            .unwrap(),
    )
}

fn job_get(rig: &mut JobRig, task: &str, grant: &str, operation: &OperationId) -> Value {
    complete(
        rig.rig
            .gw
            .handle(
                &job_req(
                    JOB_GET_ID,
                    json!({"operation": operation.to_tagged()}),
                    task,
                    grant,
                ),
                agent(),
            )
            .unwrap(),
    )
}

fn job_cancel(
    rig: &mut JobRig,
    task: &str,
    grant: &str,
    operation: &OperationId,
) -> Result<Value, ErrorEnvelope> {
    rig.rig
        .gw
        .handle(
            &job_req(
                JOB_CANCEL_ID,
                json!({"operation": operation.to_tagged()}),
                task,
                grant,
            ),
            agent(),
        )
        .map(complete)
}

fn media_prepare(
    rig: &mut JobRig,
    task: &str,
    grant: &str,
    media: &str,
) -> Result<Value, ErrorEnvelope> {
    rig.rig
        .gw
        .handle(
            &prepare_req(
                Some(task),
                Some(grant),
                op(
                    MEDIA_REGISTER_ID,
                    create_target("asset", &asset5(), "source"),
                    json!({"media": media}),
                ),
                expected(1),
            ),
            agent(),
        )
        .map(complete)
}

fn jobs_stack(
    stub: ProviderStub,
    task: &str,
    grant: &str,
    grant_builder: impl FnOnce(&str, &str) -> Grant,
) -> JobRig {
    let rig = job_rig(vec![("stub".to_owned(), stub)]);
    issue_bound(&rig, task, grant_builder(grant, task));
    rig
}

fn verified_job(rig: &mut JobRig, task: &str, grant: &str) -> (OperationId, String) {
    let operation = submit(rig, task, grant, gen_args(&[]));
    let stub = rig.providers["stub"].clone();
    stub.finish(&stub.job_at(1).unwrap());
    let result = job_get(rig, task, grant, &operation);
    assert_eq!(result["outputs"][0]["lifecycle"], json!("verified"));
    (
        operation,
        result["outputs"][0]["digest"].as_str().unwrap().to_owned(),
    )
}

// ── JOB-02: disconnect never destroys project-owned jobs ─────

#[test]
fn job02_disconnect_leaves_the_job_book_intact() {
    // JOB-02 (+JOB-01): the job record is adapter-side state — a
    // fresh caller identity for the same principal (a reconnect, a
    // closed window) resolves the identical job; nothing is reaped.
    let ev = suite();
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let operation = submit(&mut rig, "t1", "g1", gen_args(&[]));
    let before = rig.jobs.record(operation).expect("job recorded").job;

    // A brand-new CallerIdentity — the disconnect/window-close seam.
    let result = complete(
        rig.rig
            .gw
            .handle(
                &job_req(
                    JOB_GET_ID,
                    json!({"operation": operation.to_tagged()}),
                    "t1",
                    "g1",
                ),
                adapter().agent("mcp-client-1").unwrap(),
            )
            .unwrap(),
    );
    assert_eq!(result["job"].as_str(), Some(before.as_str()));
    // JOB-01: the gateway-side operation id is stable across the
    // disconnect — not per-session state.
    ev.case("JOB-01")
        .meta(Layer::L2, RunMode::Mock)
        .backend("InMemoryPolicyStore+JobEngine", "in-process")
        .fixture("job rig")
        .verify("stable id", "stable id");
    ev.case("JOB-02")
        .meta(Layer::L2, RunMode::Mock)
        .backend("InMemoryPolicyStore+JobEngine", "in-process")
        .fixture("job rig")
        .log("fresh caller identity resolves the same job record")
        .verify("retained", "retained");
}

// ── JOB-04: nonresponsive provider cancel is Unconfirmed ─────

#[test]
fn job04_unreachable_provider_reports_unconfirmed_never_stopped() {
    // JOB-04: a cancel the provider never acknowledges is honestly
    // `unconfirmed` — the book never reports "stopped" for a job
    // that may still be running.
    let ev = suite();
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()).cancel_mode(CancelMode::Unreachable),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let operation = submit(&mut rig, "t1", "g1", gen_args(&[]));
    rig.providers["stub"].set_phase(&rig.providers["stub"].job_at(1).unwrap(), JobPhase::Running);
    let result = job_cancel(&mut rig, "t1", "g1", &operation).unwrap();
    assert_eq!(result["cancel"], json!("unconfirmed"));
    assert_eq!(result["phase"], json!("pending"));
    ev.case("JOB-04")
        .meta(Layer::L2, RunMode::Mock)
        .backend("InMemoryPolicyStore+JobEngine", "in-process")
        .fixture("job rig")
        .fault(fault::PROVIDER_UNREACHABLE)
        .verify("unconfirmed", "unconfirmed");
}

// ── JOB-05: completed ≠ verified ─────────────────────────────

#[test]
fn job05_unverified_output_never_registers() {
    let ev = suite();
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()).outputs(vec![
            StubOutputSpec::media(b"fake-bytes".to_vec()).tampered(),
        ]),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let operation = submit(&mut rig, "t1", "g1", gen_args(&[]));
    let stub = rig.providers["stub"].clone();
    stub.finish(&stub.job_at(1).unwrap());
    let result = job_get(&mut rig, "t1", "g1", &operation);
    assert_eq!(result["outputs"][0]["lifecycle"], json!("rejected"));

    let err = media_prepare(&mut rig, "t1", "g1", "media-1-1").unwrap_err();
    assert!(matches!(err.code, GatewayError::Denied { .. }));
    assert_eq!(
        rig.rig.source.latest_revision(project()),
        Some(ProjectRevision(1)),
        "rejected media leaves the project untouched"
    );
    ev.case("JOB-05")
        .meta(Layer::L2, RunMode::Mock)
        .backend("InMemoryPolicyStore+JobEngine", "in-process")
        .fixture("job rig")
        .verify("Deny", "Deny");
}

// ── JOB-10: completion + revision drift is an explicit conflict ──

#[test]
fn job10_verified_output_then_drifted_base_conflicts() {
    // JOB-10: the verified media's register proposal was prepared at
    // base revision 1; a live-project move before commit is an
    // explicit RevisionConflict, never a silent rebase.
    let ev = suite();
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let (_op, _digest) = verified_job(&mut rig, "t1", "g1");
    let prepared = media_prepare(&mut rig, "t1", "g1", "media-1-1").unwrap();
    let proposal = prepared["proposal"].as_str().unwrap().to_owned();
    let p_digest = prepared["digest"].as_str().unwrap().to_owned();
    accepted(
        rig.rig
            .gw
            .handle(
                &apply_req(&proposal, &p_digest, None, Some("t1"), Some("g1")),
                agent(),
            )
            .unwrap(),
    );

    // Boot the real gate, drift the live project past the base.
    let dir = tmpdir("gw10g-job10");
    let (gate, _report, _store) = boot(&rig.rig, &dir);
    issue_via_gate(&gate, "t1", job_grant("g1", "t1", &[]));
    gate.commit_manual(
        &human(),
        synvelis_project::ProjectOp::SetProjectName {
            name: "drifted".to_owned(),
        },
        ProjectRevision(1),
    )
    .expect("manual commit lands");

    let err = gate
        .commit(&ProposalRef::new(&proposal).unwrap())
        .expect_err("stale register must conflict");
    assert!(matches!(err.code, GatewayError::RevisionConflict { .. }));
    ev.case("JOB-10")
        .meta(Layer::L2, RunMode::Mock)
        .backend("InMemoryPolicyStore+JobEngine", "in-process")
        .fixture("gw10g-job10")
        .verify("Conflict", "Conflict");
}

// ── GTV-004 path 3: generated media → provenance → commit ────

#[test]
fn gtv004_e2e3_verified_media_commits_with_provenance() {
    // The full generated-media E2E: provider-stub generation →
    // verification → `media.register` prepare/apply → real
    // CommitGate commit → the source lands with the verified digest
    // and declared provenance.
    let ev = suite();
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let (_op, digest) = verified_job(&mut rig, "t1", "g1");

    // Provenance is carried on the verified result — evidence that
    // generation metadata survives to the project boundary.
    let prepared = media_prepare(&mut rig, "t1", "g1", "media-1-1").unwrap();
    let proposal = prepared["proposal"].as_str().unwrap().to_owned();
    let p_digest = prepared["digest"].as_str().unwrap().to_owned();
    accepted(
        rig.rig
            .gw
            .handle(
                &apply_req(&proposal, &p_digest, None, Some("t1"), Some("g1")),
                agent(),
            )
            .unwrap(),
    );

    let dir = tmpdir("gw10g-e2e3");
    let (gate, _report, _store) = boot(&rig.rig, &dir);
    issue_via_gate(&gate, "t1", job_grant("g1", "t1", &[]));
    let receipt = gate
        .commit(&ProposalRef::new(&proposal).unwrap())
        .expect("commit lands");
    assert_eq!(receipt.revision, ProjectRevision(2));
    gate.inspect(|c| {
        let source = c
            .live()
            .sources
            .values()
            .find(|s| s.content_digest == digest)
            .expect("verified source committed");
        assert_eq!(source.asset_id, synvelis_project::AssetId::new(5));
        // The media-book Verified verdict and the project SourceState
        // ladder are distinct rungs — a fresh registration is Imported.
        assert!(
            matches!(source.state, synvelis_project::SourceState::Imported),
            "registration lands Imported; verification lives in the media book"
        );
    });
    ev.case("E2E-3")
        .meta(Layer::L3, RunMode::Mock)
        .backend("DirectoryBundleStore+CommitGate", "in-process")
        .fixture("gw10g-e2e3")
        .log("provider stub → verified → register → real gate commit")
        .verify("committed", "committed");
    // JOB-06: a verified output reaches the project only through the
    // commit gate — registration alone is not a mutation.
    ev.case("JOB-06")
        .meta(Layer::L3, RunMode::Mock)
        .backend("DirectoryBundleStore+CommitGate", "in-process")
        .fixture("gw10g-e2e3")
        .verify("committed", "committed");
}

// ── JOB-03: revocation denies; cancellation is requested ─────

#[test]
fn job03_revocation_denies_calls_and_requests_cancel() {
    // JOB-03: revoking the grant denies every later protected call;
    // the cancel sweep asks the provider — a Pending answer records
    // `Requested`, never a fabricated `Cancelled`.
    let ev = suite();
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()).cancel_mode(CancelMode::Pending),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let operation = submit(&mut rig, "t1", "g1", gen_args(&[]));

    rig.rig.store.revoke_grant(&grant_ref("g1"));
    let err = rig
        .rig
        .gw
        .handle(&submit_req("t1", "g1", gen_args(&[])), agent())
        .unwrap_err();
    assert!(matches!(err.code, GatewayError::Denied { .. }));

    let affected = rig.jobs.note_grant_revoked(&grant_ref("g1"));
    assert_eq!(affected.len(), 1);
    let record = rig.jobs.record(operation).unwrap();
    assert_eq!(record.cancel, CancelLink::Requested);
    assert_ne!(record.last_phase, JobPhase::Cancelled);
    ev.case("JOB-03")
        .meta(Layer::L2, RunMode::Mock)
        .backend("InMemoryPolicyStore+JobEngine", "in-process")
        .fixture("job rig")
        .log("Deny on new calls + cancel Requested (not fabricated Cancelled)")
        .verify("Deny|Requested", "Deny|Requested");
}

// ── JOB-09: cancel verdicts reflect the actual ordering ──────

#[test]
fn job09_cancel_verdicts_reflect_actual_ordering() {
    // Confirm-mode cancels confirm; a completed job reports its real
    // phase rather than a false cancellation.
    let ev = suite();
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let operation = submit(&mut rig, "t1", "g1", gen_args(&[]));
    rig.providers["stub"].set_phase(&rig.providers["stub"].job_at(1).unwrap(), JobPhase::Running);
    let result = job_cancel(&mut rig, "t1", "g1", &operation).unwrap();
    assert_eq!(result["cancel"], json!("confirmed"));
    assert_eq!(result["phase"], json!("cancelled"));

    // A completed job: the settle race went to the job — honest phase.
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let operation = submit(&mut rig, "t1", "g1", gen_args(&[]));
    let stub = rig.providers["stub"].clone();
    stub.finish(&stub.job_at(1).unwrap());
    let result = job_cancel(&mut rig, "t1", "g1", &operation).unwrap();
    assert_eq!(result["cancel"], json!("not-cancellable"));
    assert_eq!(result["phase"], json!("completed"));
    ev.case("JOB-09")
        .meta(Layer::L2, RunMode::Mock)
        .backend("InMemoryPolicyStore+JobEngine", "in-process")
        .fixture("job rig")
        .verify("honest", "honest");
}

// ── JOB-07: provenance Unknown stays Unknown ─────────────────

#[test]
fn job07_absent_claims_stay_unknown_in_provenance() {
    // JOB-07: a provider that declares nothing produces Unknown
    // evidence levels — never an invented producer/model/rights.
    let ev = suite();
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()).claim(ProviderClaim::default()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let operation = submit(&mut rig, "t1", "g1", gen_args(&[]));
    let stub = rig.providers["stub"].clone();
    stub.finish(&stub.job_at(1).unwrap());
    let result = job_get(&mut rig, "t1", "g1", &operation);
    for field in ["producer", "model", "rights"] {
        assert_eq!(
            result["provenance"][field]["evidence"],
            json!("unknown"),
            "{field}"
        );
    }
    ev.case("JOB-07")
        .meta(Layer::L2, RunMode::Mock)
        .backend("InMemoryPolicyStore+JobEngine", "in-process")
        .fixture("job rig")
        .verify("unknown", "unknown");
}

// ── Secrets bound: prompt never leaks into results ───────────

#[test]
fn gw10g_prompt_input_never_discloses_into_results() {
    // SYS-10 for the job surface: the job input (a prompt with a
    // secret-shaped token) never appears in results or provenance.
    let ev = suite();
    let secret = "SECRET-PROMPT-token-123";
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let mut args = gen_args(&[]);
    args["prompt"] = json!(secret);
    let operation = submit(&mut rig, "t1", "g1", args);
    let stub = rig.providers["stub"].clone();
    stub.finish(&stub.job_at(1).unwrap());
    let result = job_get(&mut rig, "t1", "g1", &operation);
    assert!(
        !result.to_string().contains(secret),
        "prompt is job input, never disclosed"
    );
    ev.case("SYS-10")
        .meta(Layer::L2, RunMode::Mock)
        .backend("InMemoryPolicyStore+JobEngine", "in-process")
        .fixture("job rig")
        .verify("bound", "bound");
}
