// Test closures mirror the gateway's typed-envelope signatures.
#![allow(clippy::result_large_err)]

//! GW-07 job / verified-media / provenance tests — Issue #61
//! acceptance evidence.
//!
//! Each test names the contract it proves (JM-01..05, JOB-01..10,
//! GIA-001..003, and the G5 failure matrix): distinct identities,
//! disconnect-independent job records, revocation-vs-cancellation,
//! the execution budget, honest cancel verdicts, verified-media
//! gating into GW-04..06, disclosure boundaries, provenance
//! evidence levels, and the secrets bound.

mod rig;

use std::sync::Arc;

use rig::*;
use serde_json::{Value, json};
use synvelis_gateway::commit::{CommitGate, GateDeps, MutexFence};
use synvelis_gateway::*;
use synvelis_project::{
    AssetId, CounterIdGenerator, DirectoryBundleStore, ProjectController, ProjectOp,
    ProjectRevision,
};

// ── job helpers ──────────────────────────────────────────────

fn asset5() -> ResourceRef {
    ResourceRef::Asset(AssetId::new(5))
}

fn gen_args(inputs: &[&ResourceRef]) -> Value {
    json!({
        "provider": "stub",
        "kind": "generation",
        "output": "media",
        "inputs": inputs.iter().map(|r| tagged(r)).collect::<Vec<_>>(),
    })
}

/// Issues the task and the grant the builder produces — the rig's
/// `issue_agent` reuses the grant tag as the task tag, which job
/// scopes deliberately avoid.
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

/// One provider key → stubbed job rig with a live task+grant. The
/// builder takes `(grant_tag, task_tag)`.
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

// ── JM-01 / JOB-01: distinct identities, adapter-owned records ──

#[test]
fn ids_stay_distinct_and_jobs_survive_disconnect() {
    // JM-01 + JOB-01/02: the operation id (caller-visible handle) is
    // not the provider JobRef, is not the request id; the job book
    // lives adapter-side — a fresh caller identity for the same
    // principal (an MCP reconnect) resolves the same job.
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let operation = submit(&mut rig, "t1", "g1", gen_args(&[&source9()]));
    let record = rig.jobs.record(operation).expect("job recorded");
    assert_ne!(record.operation.to_tagged(), record.job.as_str());
    assert_eq!(record.request.as_str(), "req-prop");
    assert!(record.job.as_str().starts_with("job-"));

    // "Disconnect": a brand-new CallerIdentity for the same principal.
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
    assert_eq!(result["job"].as_str().unwrap(), record.job.as_str());
    assert_eq!(
        result["snapshot"]["members"][0]["resource"],
        json!(tagged(&source9()))
    );
}

#[test]
fn input_snapshot_pins_committed_revision() {
    // JM-02/GIA-002: the admission snapshot binds the committed base
    // revision and each input's committed evidence.
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let operation = submit(&mut rig, "t1", "g1", gen_args(&[&source9()]));
    let record = rig.jobs.record(operation).unwrap();
    assert_eq!(record.snapshot.revision, Some(ProjectRevision(1)));
    let pin = &record.snapshot.members[0];
    assert_eq!(pin.resource, source9());
    assert!(pin.content_digest.is_some());
    assert_eq!(pin.source_state.as_deref(), Some("verified"));
}

// ── JOB-03: revocation protection is not cancellation ─────────

#[test]
fn revocation_denies_new_calls_but_only_requests_cancel() {
    // JOB-03: revoking the grant makes every later protected call
    // fail evaluation; `note_grant_revoked` separately asks the
    // provider — a Pending answer records `Requested`, never a
    // fabricated `Cancelled`.
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()).cancel_mode(CancelMode::Pending),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let operation = submit(&mut rig, "t1", "g1", gen_args(&[]));

    rig.rig.store.revoke_grant(&grant_ref("g1"));
    // New protected operation under the revoked grant: denied.
    let err = rig
        .rig
        .gw
        .handle(&submit_req("t1", "g1", gen_args(&[])), agent())
        .unwrap_err();
    assert!(matches!(err.code, GatewayError::Denied { .. }));

    // The revocation sweep requests cancellation — honestly recorded.
    let affected = rig.jobs.note_grant_revoked(&grant_ref("g1"));
    assert_eq!(affected.len(), 1);
    let record = rig.jobs.record(operation).unwrap();
    assert_eq!(record.cancel, CancelLink::Requested);
    assert_ne!(record.last_phase, JobPhase::Cancelled);
}

// ── JOB-04: execution budget ─────────────────────────────────

#[test]
fn job_budget_bounds_admissions() {
    // JOB-04: `limits.job_budget` spends one unit per accepted
    // submit; the second submit is a typed limit denial.
    let grant = |tag: &str, task: &str| {
        let mut g = job_grant(tag, task, &[]);
        g.limits.job_budget = Some(1);
        g
    };
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        grant,
    );
    submit(&mut rig, "t1", "g1", gen_args(&[]));
    let err = rig
        .rig
        .gw
        .handle(&submit_req("t1", "g1", gen_args(&[])), agent())
        .unwrap_err();
    assert!(matches!(err.code, GatewayError::LimitExceeded { .. }));
}

#[test]
fn expired_grant_admits_no_new_job() {
    // JOB-04 sibling: after expiry no new protected operation begins.
    let grant = |tag: &str, task: &str| {
        let mut g = job_grant(tag, task, &[]);
        g.validity.expires_at = Some(PolicyInstant::new(1_500));
        g
    };
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        grant,
    );
    submit(&mut rig, "t1", "g1", gen_args(&[]));
    rig.rig.clock.advance(600);
    let err = rig
        .rig
        .gw
        .handle(&submit_req("t1", "g1", gen_args(&[])), agent())
        .unwrap_err();
    assert!(matches!(err.code, GatewayError::Denied { .. }));
}

// ── JOB-05/JM-04: completed ≠ verified ────────────────────────

#[test]
fn failed_verification_never_registers() {
    // JOB-05 + G5: a completed job with unverifiable output records
    // `Rejected`; `synvelis.media.register` fails closed and the
    // project revision stays put.
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
    assert!(result["outputs"][0]["digest"].is_null());

    let media = "media-1-1";
    let err = media_prepare(&mut rig, "t1", "g1", media).unwrap_err();
    assert!(matches!(err.code, GatewayError::Denied { .. }));
    assert_eq!(
        rig.rig.source.latest_revision(project()),
        Some(ProjectRevision(1)),
        "rejected media leaves the project untouched"
    );
}

#[test]
fn unknown_or_foreign_media_denies_identically() {
    // G5/JOB-08: unknown media ref and a media ref owned by a
    // different scope share one `Denied` — the gate is
    // existence-blind.
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let unknown = media_prepare(&mut rig, "t1", "g1", "media-9-9").unwrap_err();
    // A foreign-owned media record: recorded under a different
    // principal, same book.
    let foreign = MediaRef::new("media-7-1").unwrap();
    rig.book.record(
        &JobRef::new("job-7").unwrap(),
        human_principal(),
        project(),
        OutputKind::Media,
        &DeclaredOutput {
            media: foreign.clone(),
            content_digest: "sha256:00".to_owned(),
            media_format: "audio/wav".to_owned(),
            byte_len: 1,
        },
        &MediaVerdict::Verified(VerifiedMedia {
            media: foreign.clone(),
            content_digest: "sha256:00".to_owned(),
            media_format: "audio/wav".to_owned(),
            byte_len: 1,
        }),
    );
    let foreign_err = media_prepare(&mut rig, "t1", "g1", "media-7-1").unwrap_err();
    assert_eq!(
        std::mem::discriminant(&unknown.code),
        std::mem::discriminant(&foreign_err.code),
        "unknown and foreign media deny identically"
    );
}

#[test]
fn register_fails_closed_without_a_gate() {
    // G5: a PrepareEngine with no media gate wired denies
    // `synvelis.media.register` rather than guessing a digest.
    let mut rig = fixture_rig();
    issue_task(&rig.store, "t1");
    rig.store.issue_grant(job_grant("g1", "t1", &[]));
    let err = rig
        .gw
        .handle(
            &prepare_req(
                Some("t1"),
                Some("g1"),
                op(
                    MEDIA_REGISTER_ID,
                    create_target("asset", &asset5(), "source"),
                    json!({"media": "media-1-1"}),
                ),
                expected(1),
            ),
            agent(),
        )
        .unwrap_err();
    // Host misconfiguration is Internal — not an authorization
    // verdict the caller could fix.
    assert!(matches!(err.code, GatewayError::Internal { .. }));
}

// ── JOB-06/JM-04 + G5: verified media → GW-04..06 ─────────────

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

#[test]
fn verified_media_lowers_to_register_source_with_gate_digest() {
    // JOB-06/JM-04: `synvelis.media.register` consumes the verified
    // record — the lowered `ProjectOp::RegisterSource` carries the
    // gate-derived digest, never a caller field.
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let (_op, digest) = verified_job(&mut rig, "t1", "g1");
    let result = media_prepare(&mut rig, "t1", "g1", "media-1-1").unwrap();
    let record = record_for(&rig.rig, &result);
    let ProjectOp::RegisterSource {
        asset_id,
        content_digest,
    } = &record.op
    else {
        panic!("expected RegisterSource, got {:?}", record.op);
    };
    assert_eq!(*asset_id, AssetId::new(5));
    assert_eq!(
        *content_digest, digest,
        "digest comes from the gate, not args"
    );
    assert_eq!(
        rig.rig.source.latest_revision(project()),
        Some(ProjectRevision(1)),
        "preparation alone never commits (verified media without commit changes nothing)"
    );
}

#[test]
fn verified_media_commits_through_gate() {
    // JOB-06 + G5 end-to-end: prepare → apply → CommitGate — the
    // committed source's digest is the verified evidence, the
    // revision advances exactly once.
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let (_op, digest) = verified_job(&mut rig, "t1", "g1");
    let prepared = media_prepare(&mut rig, "t1", "g1", "media-1-1").unwrap();
    let proposal = prepared["proposal"].as_str().unwrap().to_owned();
    let p_digest = prepared["digest"].as_str().unwrap().to_owned();
    // Apply is an async-accepted operation — the commit gate is the
    // settle boundary it feeds.
    accepted(
        rig.rig
            .gw
            .handle(
                &apply_req(&proposal, &p_digest, None, Some("t1"), Some("g1")),
                agent(),
            )
            .unwrap(),
    );

    let dir = std::env::temp_dir().join(format!("synvelis-gw07-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut store = DirectoryBundleStore::create(dir.join("project")).unwrap();
    let mut controller =
        ProjectController::new(demo_project(), Box::new(CounterIdGenerator::new(9_000)));
    controller
        .apply(synvelis_project::Command {
            expected_revision: controller.live_revision(),
            op: ProjectOp::SetProjectName {
                name: "demo".to_owned(),
            },
        })
        .unwrap();
    controller.save(&mut store).unwrap();
    let oracle: Arc<dyn ResourceOracle> = Arc::new(LatestOracle {
        source: rig.rig.source.clone(),
    });
    let deps = GateDeps {
        policy: rig.rig.store.clone(),
        admin: rig.rig.store.clone(),
        evaluator: GrantPolicy::new(rig.rig.store.clone(), rig.rig.clock.clone(), oracle),
        proposals: rig.rig.proposals.clone(),
        apply_capability: rig
            .rig
            .gw
            .registry()
            .describe(&cap_ref(APPLY_ID))
            .unwrap()
            .clone(),
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
        assert_eq!(source.asset_id, AssetId::new(5));
    });
}

// ── JOB-07: result disclosure boundary ───────────────────────

#[test]
fn result_disclosure_requires_the_disclosure_class() {
    // JOB-07: `job.get` is a separate grant boundary — an agent
    // grant without `ExternalDisclosure` fails at
    // ReadOperationResult.
    let grant = |tag: &str, task: &str| {
        let mut g = job_grant(tag, task, &[]);
        g.data_access = DataAccessSet::of([
            DataAccessClass::MetadataRead,
            DataAccessClass::ProviderTransfer,
        ]);
        g
    };
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        grant,
    );
    let operation = submit(&mut rig, "t1", "g1", gen_args(&[]));
    let err = rig
        .rig
        .gw
        .handle(
            &job_req(
                JOB_GET_ID,
                json!({"operation": operation.to_tagged()}),
                "t1",
                "g1",
            ),
            agent(),
        )
        .unwrap_err();
    assert!(matches!(
        err.code,
        GatewayError::Denied {
            stage: AuthorizationStage::ReadOperationResult,
            ..
        }
    ));
}

// ── JOB-08: existence-blind owner scope ──────────────────────

#[test]
fn foreign_and_unknown_operations_deny_identically() {
    // JOB-08: a job submitted under (t1,g1) does not exist for
    // (t2,g2) — the verdict is the same `Denied` as a minted id.
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    issue_bound(&rig, "t2", job_grant("g2", "t2", &[]));
    let operation = submit(&mut rig, "t1", "g1", gen_args(&[]));
    let foreign = rig
        .rig
        .gw
        .handle(
            &job_req(
                JOB_GET_ID,
                json!({"operation": operation.to_tagged()}),
                "t2",
                "g2",
            ),
            agent(),
        )
        .unwrap_err();
    let nonexistent = rig
        .rig
        .gw
        .handle(
            &job_req(
                JOB_GET_ID,
                json!({"operation": OperationId::new(999_999).to_tagged()}),
                "t1",
                "g1",
            ),
            agent(),
        )
        .unwrap_err();
    assert_eq!(
        std::mem::discriminant(&foreign.code),
        std::mem::discriminant(&nonexistent.code)
    );
}

// ── JOB-09/JM-03: honest cancel verdicts, progress ───────────

#[test]
fn cancel_verdicts_are_honest() {
    // JOB-09: Confirmed → cancelled; Pending → requested (never
    // "cancelled"); Unreachable → unconfirmed; an already-completed
    // job reports its actual phase.
    for (mode, verdict, cancel, phase) in [
        (CancelMode::Confirm, "confirmed", "confirmed", "cancelled"),
        // No status poll ever observed Running — the last *observed*
        // phase is what the record honestly reports.
        (CancelMode::Pending, "requested", "requested", "pending"),
        (
            CancelMode::Unreachable,
            "unconfirmed",
            "unconfirmed",
            "pending",
        ),
    ] {
        let mut rig = jobs_stack(
            ProviderStub::generation(StubContent::shared()).cancel_mode(mode),
            "t1",
            "g1",
            |g, t| job_grant(g, t, &[]),
        );
        let operation = submit(&mut rig, "t1", "g1", gen_args(&[]));
        rig.providers["stub"]
            .set_phase(&rig.providers["stub"].job_at(1).unwrap(), JobPhase::Running);
        let result = job_cancel(&mut rig, "t1", "g1", &operation).unwrap();
        assert_eq!(result["cancel"], json!(verdict), "{mode:?}");
        assert_eq!(result["phase"], json!(phase), "{mode:?}");
        let record = rig.jobs.record(operation).unwrap();
        assert_eq!(record.cancel.name(), cancel);
    }

    // The settle race went to the job — cancel reports the truth.
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
}

#[test]
fn progress_polls_report_provider_truth() {
    // JM-03: `job.get` reflects the provider's current phase/progress
    // each poll — no fabricated progress.
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()).drive(DriveMode::OnPoll),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let operation = submit(&mut rig, "t1", "g1", gen_args(&[]));
    let first = job_get(&mut rig, "t1", "g1", &operation);
    assert_eq!(first["phase"], json!("running"));
    let second = job_get(&mut rig, "t1", "g1", &operation);
    assert_eq!(second["phase"], json!("completed"));
    assert_eq!(second["progress"], json!(100));
}

// ── JM-05 + GIA-003: provenance evidence, secrets bound ──────

#[test]
fn provenance_levels_and_secret_bound() {
    // JM-05/GIA-003: declared claims map to `Declared`, absent claims
    // to `Unknown`; the prompt never leaks into the result,
    // provenance, or media records.
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
    let provenance = &result["provenance"];
    assert_eq!(provenance["producer"]["evidence"], json!("declared"));
    assert_eq!(provenance["model"]["evidence"], json!("declared"));
    assert_eq!(provenance["rights"]["evidence"], json!("declared"));
    assert!(
        !result.to_string().contains(secret),
        "prompt is job input, never disclosed"
    );

    // An absent claim → Unknown, honestly.
    let claimless = ProviderStub::generation(StubContent::shared()).claim(ProviderClaim::default());
    let mut rig2 = jobs_stack(claimless, "t1", "g1", |g, t| job_grant(g, t, &[]));
    let op2 = submit(&mut rig2, "t1", "g1", gen_args(&[]));
    let stub2 = rig2.providers["stub"].clone();
    stub2.finish(&stub2.job_at(1).unwrap());
    let result2 = job_get(&mut rig2, "t1", "g1", &op2);
    let prov2 = &result2["provenance"];
    for field in ["producer", "model", "rights"] {
        assert_eq!(prov2[field]["evidence"], json!("unknown"), "{field}");
    }
}

// ── Denied submit paths ──────────────────────────────────────

#[test]
fn submit_denies_out_of_scope_inputs_and_missing_transfer_class() {
    // GIA-002 Read/External-Transfer boundary: an input outside the
    // grant's scope denies admission before the provider sees it.
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let lane = lane4(); // lane4 is not in the grant's scope set.
    let err = rig
        .rig
        .gw
        .handle(&submit_req("t1", "g1", gen_args(&[&lane])), agent())
        .unwrap_err();
    assert!(matches!(
        err.code,
        GatewayError::Denied {
            stage: AuthorizationStage::StartAsyncOperation,
            ..
        }
    ));
    assert!(
        rig.providers["stub"].submissions().is_empty(),
        "denied admission never reaches the provider"
    );

    // Without ProviderTransfer the same valid input still denies.
    let grant = |tag: &str, task: &str| {
        let mut g = job_grant(tag, task, &[]);
        g.data_access = DataAccessSet::of([
            DataAccessClass::MetadataRead,
            DataAccessClass::ExternalDisclosure,
        ]);
        g
    };
    let mut rig2 = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        grant,
    );
    let err2 = rig2
        .rig
        .gw
        .handle(&submit_req("t1", "g1", gen_args(&[])), agent())
        .unwrap_err();
    assert!(matches!(err2.code, GatewayError::Denied { .. }));
}

#[test]
fn unknown_provider_denies_admission() {
    // A provider key with no registered port is a typed admission
    // denial — the stub never sees the spec.
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let mut args = gen_args(&[]);
    args["provider"] = json!("nonexistent");
    let err = rig
        .rig
        .gw
        .handle(&submit_req("t1", "g1", args), agent())
        .unwrap_err();
    assert!(matches!(err.code, GatewayError::Denied { .. }));
}

// ── Review-driven hardening ──────────────────────────────────

/// A fabricated record for direct `JobBook` tests — every field is
/// public data, no request plumbing needed.
fn book_record(op: u128, task: &str, grant: &str) -> JobRecord {
    JobRecord {
        operation: OperationId::new(op),
        request: RequestId::new(format!("req-{op}")).unwrap(),
        job: JobRef::new(format!("job-{op}")).unwrap(),
        principal: agent_principal(),
        project: project(),
        task: Some(task_ref(task)),
        grant: Some(grant_ref(grant)),
        spec: JobSpec {
            kind: JobKind::Generation,
            output: OutputKind::Media,
            provider: "stub".to_owned(),
            inputs: vec![],
            prompt: None,
        },
        snapshot: InputSnapshot {
            revision: None,
            members: vec![],
        },
        cancel: CancelLink::NotRequested,
        last_phase: JobPhase::Pending,
        result: None,
    }
}

#[test]
fn job_book_refuses_foreign_saturation() {
    // A book saturated by *other* scopes reports `false` — the
    // engine turns that into an honest error + provider cancel,
    // never an `Accepted` ghost job. Same-scope inserts still evict
    // the caller's own oldest.
    let mut book = JobBook::new(1);
    assert!(book.insert(book_record(1, "t-a", "g-a")));
    assert!(!book.insert(book_record(2, "t-b", "g-b")));
    assert!(book.get(OperationId::new(2)).is_none());
    // Same scope → evict own oldest, admit the new record.
    assert!(book.insert(book_record(3, "t-a", "g-a")));
    assert!(book.get(OperationId::new(1)).is_none());
    assert!(book.get(OperationId::new(3)).is_some());
    // Id collision never aliases.
    assert!(!book.insert(book_record(3, "t-b", "g-b")));
}

#[test]
fn job_book_spend_is_atomic_and_refundable() {
    // `try_spend` is the budget's check-and-spend: one lock, one
    // unit; `unspend` only refunds a provider admission that never
    // happened.
    let mut book = JobBook::new(4);
    let grant = grant_ref("g1");
    assert!(book.try_spend(&grant, 1));
    assert!(!book.try_spend(&grant, 1));
    book.unspend(&grant);
    assert!(book.try_spend(&grant, 1));
    // Spend is cumulative — inserting/evicting records never
    // refunds or re-counts it.
    assert!(book.insert(book_record(1, "t1", "g1")));
    assert!(book.insert(book_record(2, "t1", "g1")));
    assert_eq!(book.count_for_grant(&grant), 1);
    assert_eq!(book.count(), 2);
}

#[test]
fn pending_cancel_upgrades_to_confirmed_on_poll() {
    // `Requested` + provider-settled `Cancelled` → the next `job.get`
    // upgrades the link to `Confirmed` on provider truth.
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()).cancel_mode(CancelMode::Pending),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let operation = submit(&mut rig, "t1", "g1", gen_args(&[]));
    let job = rig.providers["stub"].job_at(1).unwrap();
    let result = job_cancel(&mut rig, "t1", "g1", &operation).unwrap();
    assert_eq!(result["cancel"], json!("requested"));
    rig.providers["stub"].set_phase(&job, JobPhase::Cancelled);
    let result = job_get(&mut rig, "t1", "g1", &operation);
    assert_eq!(result["cancel"], json!("confirmed"));
    assert_eq!(result["phase"], json!("cancelled"));
    assert_eq!(
        rig.jobs.record(operation).unwrap().cancel,
        CancelLink::Confirmed
    );
}

#[test]
fn repeated_get_materializes_once() {
    // Two polls of a completed job disclose the same result — the
    // media book replays its recorded verdict, never re-decides.
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let operation = submit(&mut rig, "t1", "g1", gen_args(&[]));
    let stub = rig.providers["stub"].clone();
    stub.finish(&stub.job_at(1).unwrap());
    let first = job_get(&mut rig, "t1", "g1", &operation);
    let second = job_get(&mut rig, "t1", "g1", &operation);
    assert_eq!(first["outputs"], second["outputs"]);
    assert_eq!(first["provenance"], second["provenance"]);
    assert_eq!(first["outputs"][0]["lifecycle"], json!("verified"));
}

#[test]
fn foreign_project_media_is_denied() {
    // The gate's ownership tuple includes the project — a record
    // verified under another project is not registerable here.
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &[]),
    );
    let media = MediaRef::new("media-8-1").unwrap();
    rig.book.record(
        &JobRef::new("job-8").unwrap(),
        agent_principal(),
        synvelis_project::ProjectId::new(8),
        OutputKind::Media,
        &DeclaredOutput {
            media: media.clone(),
            content_digest: "sha256:00".to_owned(),
            media_format: "audio/wav".to_owned(),
            byte_len: 1,
        },
        &MediaVerdict::Verified(VerifiedMedia {
            media,
            content_digest: "sha256:00".to_owned(),
            media_format: "audio/wav".to_owned(),
            byte_len: 1,
        }),
    );
    media_prepare(&mut rig, "t1", "g1", "media-8-1").unwrap_err();
}

#[test]
fn batch_media_register_uses_the_gate() {
    // Batch members lower through the same gated path — a verified
    // ref prepares; an unknown ref fails the batch atomically.
    let mut rig = jobs_stack(
        ProviderStub::generation(StubContent::shared()),
        "t1",
        "g1",
        |g, t| job_grant(g, t, &["synvelis.operation.batch"]),
    );
    verified_job(&mut rig, "t1", "g1");
    let batch = |media: &str| {
        op(
            "synvelis.operation.batch",
            Value::Null,
            json!({"ops": [
                {"capability": {"id": MEDIA_REGISTER_ID, "version": {"major": 1, "minor": 0}},
                 "target": create_target("asset", &asset5(), "source"),
                 "args": {"media": media}},
            ]}),
        )
    };
    let prepared = complete(
        rig.rig
            .gw
            .handle(
                &prepare_req(Some("t1"), Some("g1"), batch("media-1-1"), expected(1)),
                agent(),
            )
            .unwrap(),
    );
    assert!(prepared["proposal"].is_string());
    // An unverifiable ref inside a batch is denied identically.
    rig.rig
        .gw
        .handle(
            &prepare_req(Some("t1"), Some("g1"), batch("media-9-9"), expected(1)),
            agent(),
        )
        .unwrap_err();
}
