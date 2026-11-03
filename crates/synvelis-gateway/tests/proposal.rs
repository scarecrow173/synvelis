//! GW-05 proposal tests — Issue #57 acceptance criteria.
//!
//! Each test names the contract it proves (GTC-007, GT-007..010,
//! SEC-09..12, ACG-002/004): the immutable digested changeset, the
//! semantic preview, the three independent lifecycle machines, the
//! trusted-user receipt boundary, digest-bound apply, and the GW-06
//! seam the commit path consumes — never a false commit success.

mod rig;

use std::collections::BTreeSet;

use rig::*;
use serde_json::{Value, json};
use synvelis_gateway::*;
use synvelis_project::{NoteId, ProjectId, ProjectRevision};

// ── GT-007: deterministic semantic digest ────────────────────

#[test]
fn identical_prepares_digest_identically() {
    // GT-007: the digest is semantic identity — two preparations of
    // the same change against the same base digest equally even
    // though their proposal refs and retention order differ.
    let mut rig = fixture_rig();
    let (p1, d1) = prepare_mute(&mut rig, human());
    let (p2, d2) = prepare_mute(&mut rig, human());
    assert_ne!(p1, p2, "proposal refs are unique handles");
    assert_eq!(d1, d2, "same semantic change, same digest");
}

#[test]
fn semantic_drift_changes_the_digest() {
    // GT-007: target, args, and base revision each enter the digest —
    // any semantic drift produces a different identity.
    let mut rig = fixture_rig();
    let (_, base) = prepare_mute(&mut rig, human());

    // Different target resource.
    let other = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    op(
                        "synvelis.clip.mute",
                        res_target("clip", &clip12()),
                        json!({"muted": true}),
                    ),
                    expected(1),
                ),
                human(),
            )
            .unwrap(),
    );
    assert_ne!(other["digest"].as_str().unwrap(), base);

    // Different command args on the same target.
    let unmute = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    op(
                        "synvelis.clip.mute",
                        res_target("clip", &clip11()),
                        json!({"muted": false}),
                    ),
                    expected(1),
                ),
                human(),
            )
            .unwrap(),
    );
    assert_ne!(unmute["digest"].as_str().unwrap(), base);

    // A different base revision (the dependency identity moved).
    rig.source
        .commit(project(), ProjectRevision(2), demo_project());
    let rebased = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    op(
                        "synvelis.clip.mute",
                        res_target("clip", &clip11()),
                        json!({"muted": true}),
                    ),
                    expected(2),
                ),
                human(),
            )
            .unwrap(),
    );
    assert_ne!(rebased["digest"].as_str().unwrap(), base);
}

#[test]
fn changeset_verify_detects_tampering() {
    // SEC-09: the digest content-addresses the whole semantic
    // proposal — mutating any digested field breaks verification.
    let mut rig = fixture_rig();
    let (proposal, _) = prepare_mute(&mut rig, human());
    let pref = ProposalRef::new(&proposal).unwrap();
    let changeset = rig.proposal_engine.changeset(&pref).unwrap();
    assert!(changeset.verify());

    let mut tampered = changeset.clone();
    tampered.base.revision = ProjectRevision(99);
    assert!(!tampered.verify());
    let mut tampered = changeset.clone();
    tampered.unsupported.push("injected".to_owned());
    assert!(!tampered.verify());
    let mut tampered = changeset.clone();
    tampered.commands.clear();
    assert!(!tampered.verify());
    // Effects and preconditions are digest-bound fields too — a
    // cleared effect or an injected precondition is tampering.
    let mut tampered = changeset.clone();
    tampered.effects.clear();
    assert!(!tampered.verify());
    let mut tampered = changeset;
    tampered.preconditions.clear();
    assert!(!tampered.verify());
}

#[test]
fn lookup_requires_proposal_and_digest() {
    // SEC-09: the digest asserts which changeset the caller means —
    // a mismatched assertion is a typed failure, never a rebind.
    let mut rig = fixture_rig();
    let (proposal, digest) = prepare_mute(&mut rig, human());

    complete(
        rig.gw
            .handle(&preview_req(&proposal, Some(&digest), None, None), human())
            .unwrap(),
    );
    let err = rig
        .gw
        .handle(
            &preview_req(&proposal, Some("sha256:deadbeef"), None, None),
            human(),
        )
        .expect_err("wrong digest must fail");
    assert!(matches!(err.code, GatewayError::DigestMismatch { .. }));
}

// ── Semantic preview (GT-010) ────────────────────────────────

#[test]
fn preview_reports_midi_notes_before_and_after() {
    // GT-010: the preview surfaces the same derived effects the
    // digest binds — the MIDI note diff comes from the effect
    // detail GW-04 recorded, not a second diff engine.
    let mut rig = fixture_rig();
    let result = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    op(
                        "synvelis.midi.note.add",
                        res_target("source", &source10()),
                        json!({"key": 65, "velocity": 90, "range": {"domain": "source-native", "start": 720, "len": 240}}),
                    ),
                    expected(1),
                ),
                human(),
            )
            .unwrap(),
    );
    let proposal = result["proposal"].as_str().unwrap();
    let preview = preview_value_of(&mut rig, proposal);

    assert_eq!(preview["proposal"], json!(proposal));
    assert_eq!(preview["digest"], result["digest"]);
    assert_eq!(preview["base"]["revision"], json!(1));
    assert_eq!(preview["status"], json!("prepared"));
    assert_eq!(preview["diff"]["complete"], json!(true));

    let notes = preview["diff"]["midi_notes"].as_array().unwrap();
    let added = notes
        .iter()
        .find(|n| n["change"] == json!("added"))
        .expect("added note in preview");
    assert_eq!(added["before"], Value::Null);
    assert_eq!(added["after"]["key"], json!(65));
    assert_eq!(added["after"]["velocity"], json!(90));

    // Entity-level before/after names the shared source write.
    let entities = preview["diff"]["entities"].as_array().unwrap();
    let source_write = entities
        .iter()
        .find(|e| e["resource"] == json!(tagged(&source10())))
        .expect("source effect in preview");
    assert_eq!(source_write["change"], json!("modified"));
    assert!(source_write["before"].is_object() && source_write["after"].is_object());
}

#[test]
fn preview_reports_modified_and_removed_note_pairs() {
    // GT-010: the note diff is complete — a velocity change is a
    // `modified` pair with both payloads, a removal is a `removed`
    // pair whose `before` carries the dropped note.
    let mut rig = fixture_rig();
    let velocity = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    op(
                        "synvelis.midi.note.set-velocity",
                        res_target("source", &source10()),
                        json!({"note": NoteId::new(20).to_tagged(), "velocity": 72}),
                    ),
                    expected(1),
                ),
                human(),
            )
            .unwrap(),
    );
    let preview = preview_value_of(&mut rig, velocity["proposal"].as_str().unwrap());
    let modified = preview["diff"]["midi_notes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["change"] == json!("modified"))
        .expect("modified note in preview");
    assert_eq!(modified["before"]["velocity"], json!(100));
    assert_eq!(modified["after"]["velocity"], json!(72));

    let removed = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    op(
                        "synvelis.midi.note.remove",
                        res_target("source", &source10()),
                        json!({"note": NoteId::new(21).to_tagged()}),
                    ),
                    expected(1),
                ),
                human(),
            )
            .unwrap(),
    );
    let preview = preview_value_of(&mut rig, removed["proposal"].as_str().unwrap());
    let dropped = preview["diff"]["midi_notes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["change"] == json!("removed"))
        .expect("removed note in preview");
    assert_eq!(dropped["before"]["key"], json!(64));
    assert_eq!(dropped["after"], Value::Null);

    // Entity-level `deleted`: a clip removal surfaces the dropped
    // resource with its last-known attributes.
    let gone = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    op(
                        "synvelis.clip.remove",
                        res_target("clip", &clip11()),
                        json!({}),
                    ),
                    expected(1),
                ),
                human(),
            )
            .unwrap(),
    );
    let preview = preview_value_of(&mut rig, gone["proposal"].as_str().unwrap());
    let deleted = preview["diff"]["entities"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["change"] == json!("deleted"))
        .expect("deleted entity in preview");
    assert_eq!(deleted["resource"], json!(tagged(&clip11())));
    assert!(deleted["before"].is_object());
    assert_eq!(deleted["after"], Value::Null);
}

#[test]
fn preview_hides_foreign_proposals() {
    // SEC-10: a proposal is observable by its owner and trusted
    // users — never by another agent, and never across projects.
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
    let (proposal, digest) = prepare_mute_agent(&mut rig, "t1", "g1");

    // A second agent with its own task+grant cannot see it.
    rig.store.issue_task(AuthorizedTask {
        task: task_ref("t2"),
        principal: Principal::new(PrincipalKind::Agent, "mcp-client-2").unwrap(),
        project: project(),
        activation: Activation::UserIssued,
        validity: Validity::default(),
    });
    let mut g2 = apply_grant("g2", "t2", &[], ApprovalPolicy::DelegatedApply);
    g2.principal = Principal::new(PrincipalKind::Agent, "mcp-client-2").unwrap();
    rig.store.issue_grant(g2);
    let agent2 = adapter().agent("mcp-client-2").unwrap();
    let err = rig
        .gw
        .handle(
            &preview_req(&proposal, Some(&digest), Some("t2"), Some("g2")),
            agent2,
        )
        .expect_err("foreign principal must not see the proposal");
    assert!(matches!(err.code, GatewayError::ProposalUnavailable { .. }));

    // Nor does a proposal leak across the project boundary.
    let mut other_project = preview_req(&proposal, Some(&digest), None, None);
    other_project["project"] = json!(ProjectId::new(8).to_tagged());
    let err = rig
        .gw
        .handle(&other_project, human())
        .expect_err("foreign project must not see the proposal");
    assert!(matches!(err.code, GatewayError::ProposalUnavailable { .. }));

    // The owner and a trusted user see it fine.
    complete(
        rig.gw
            .handle(
                &preview_req(&proposal, None, Some("t1"), Some("g1")),
                agent(),
            )
            .unwrap(),
    );
    complete(
        rig.gw
            .handle(&preview_req(&proposal, None, None, None), human())
            .unwrap(),
    );
}

// ── Receipt issuance boundary (GT-008) ───────────────────────

#[test]
fn approve_is_a_trusted_user_only_act() {
    // GT-008: agents never mint approval receipts — even when the
    // capability is inside their grant. A trusted user issues the
    // receipt; the result names every bound dimension.
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

    // The agent cannot approve its own proposal — even with the
    // capability granted (capability + trusted-user act ≠ agent act).
    let mut agent_approve = approve_req(&proposal, &digest);
    agent_approve["task"] = json!("t1");
    agent_approve["grant"] = json!("g1");
    // The approve capability is not even inside the agent grant — the
    // grant gate denies before the trusted-user check matters.
    let err = rig
        .gw
        .handle(&agent_approve, agent())
        .expect_err("agent approve must deny");
    assert!(matches!(err.code, GatewayError::Denied { .. }));

    // Even a grant that *does* cover the capability cannot make an
    // agent a trusted user.
    let mut g2 = apply_grant(
        "g2",
        "t1",
        &["synvelis.clip.mute"],
        ApprovalPolicy::ApproveToApply,
    );
    g2.capabilities.insert(cap_ref(APPROVE_ID));
    rig.store.issue_grant(g2);
    let mut with_cap = approve_req(&proposal, &digest);
    with_cap["task"] = json!("t1");
    with_cap["grant"] = json!("g2");
    let err = rig
        .gw
        .handle(&with_cap, agent())
        .expect_err("agent with capability still cannot approve");
    match err.code {
        GatewayError::Denied { reason, .. } => {
            assert_eq!(reason.unwrap(), "approval issuance is a trusted-user act");
        }
        other => panic!("expected Denied, got {other:?}"),
    }

    // The trusted user issues the receipt bound to recorded identity.
    let receipt = complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );
    assert_eq!(receipt["status"], json!("granted"));
    assert_eq!(receipt["digest"], json!(digest));
    assert_eq!(receipt["proposal"], json!(proposal));
    assert_eq!(receipt["for_principal"], json!("mcp-client-1"));
    assert_eq!(receipt["approved_by"], json!("user-1"));
    assert_eq!(receipt["grant"], json!("g1"));
    assert_eq!(receipt["task"], json!("t1"));
    assert_eq!(receipt["expected_revision"], json!(1));
    assert!(receipt["approval"].is_string());
}

// ── Approve-to-apply (GT-009) ────────────────────────────────

#[test]
#[allow(clippy::too_many_lines)] // sequential wire flow reads top-to-bottom
fn approve_to_apply_flows_through_request_grant_consume() {
    // GT-009: apply under ApproveToApply waits on the individual
    // approval — the gate opens the request, the trusted user grants
    // it, and exactly one apply may consume the receipt.
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
    let pref = ProposalRef::new(&proposal).unwrap();

    // Attempt 1 — the gate opens the request and reports it.
    let outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    let GatewayOutcome::ApprovalRequired { approval } = outcome else {
        panic!("expected ApprovalRequired, got {outcome:?}");
    };
    let view = rig.proposals.view(&pref).unwrap();
    assert_eq!(
        view.approval.as_ref().unwrap().status,
        ApprovalStatus::Requested
    );
    assert_eq!(
        view.apply.as_ref().unwrap().status,
        ApplyStatus::AwaitingApproval
    );
    // The wire-visible ref is what the attempt was told.
    assert_eq!(view.approval.as_ref().unwrap().approval, approval);

    // Before the user decides, a retry still waits — and re-points
    // the wire ref at the latest attempt.
    let outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    let GatewayOutcome::ApprovalRequired {
        approval: approval2,
    } = outcome
    else {
        panic!("expected ApprovalRequired, got {outcome:?}");
    };
    let view = rig.proposals.view(&pref).unwrap();
    assert_eq!(view.approval.as_ref().unwrap().approval, approval2);

    // The trusted user grants the receipt.
    complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );
    let view = rig.proposals.view(&pref).unwrap();
    assert_eq!(
        view.approval.as_ref().unwrap().status,
        ApprovalStatus::Granted
    );

    // The apply presents the last-reported request ref — consumed.
    let outcome = rig
        .gw
        .handle(
            &apply_req(
                &proposal,
                &digest,
                Some(approval2.as_str()),
                Some("t1"),
                Some("g1"),
            ),
            agent(),
        )
        .unwrap();
    let GatewayOutcome::Accepted { operation } = outcome else {
        panic!("expected Accepted, got {outcome:?}");
    };
    let view = rig.proposals.view(&pref).unwrap();
    assert_eq!(
        view.approval.as_ref().unwrap().status,
        ApprovalStatus::Consumed
    );
    assert_eq!(
        view.apply.as_ref().unwrap().status,
        ApplyStatus::CommitRequested
    );
    assert_eq!(view.apply.as_ref().unwrap().operation, operation);

    // A retry references the accepted apply — never a second consume,
    // and the outcome names the durable handle, not a new commit.
    let outcome = rig
        .gw
        .handle(
            &apply_req(
                &proposal,
                &digest,
                Some(approval2.as_str()),
                Some("t1"),
                Some("g1"),
            ),
            agent(),
        )
        .unwrap();
    assert!(matches!(
        outcome,
        GatewayOutcome::Accepted {
            operation: op
        } if op == operation
    ));
    let view = rig.proposals.view(&pref).unwrap();
    assert_eq!(
        view.approval.as_ref().unwrap().status,
        ApprovalStatus::Consumed
    );
}

#[test]
fn declined_revoked_and_invalid_approvals_deny() {
    // SEC-09: a declined/revoked/wrong-dimension approval can never
    // authorize — the gate denies, the request never executes.
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
    let pref = ProposalRef::new(&proposal).unwrap();
    rig.gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();

    // Declined: the trusted user's refusal is terminal.
    rig.proposal_engine.decline_approval(&pref).unwrap();
    let err = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .expect_err("declined approval must deny");
    match err.code {
        GatewayError::Denied { reason, .. } => {
            assert_eq!(reason.unwrap(), "approval declined");
        }
        other => panic!("expected Denied, got {other:?}"),
    }
    // A declined request can no longer be granted either.
    let err = rig
        .gw
        .handle(&approve_req(&proposal, &digest), human())
        .expect_err("grant on a declined request must fail");
    assert!(matches!(err.code, GatewayError::ApprovalInvalid { .. }));
}

#[test]
fn revoked_approval_denies_apply() {
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
    let pref = ProposalRef::new(&proposal).unwrap();
    let outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    let GatewayOutcome::ApprovalRequired { approval } = outcome else {
        panic!("expected ApprovalRequired, got {outcome:?}");
    };
    complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );

    // Synvelis-side revocation seals the receipt before consumption —
    // presenting the real request ref now reports the revocation.
    rig.proposal_engine.revoke_approval(&pref).unwrap();
    let err = rig
        .gw
        .handle(
            &apply_req(
                &proposal,
                &digest,
                Some(approval.as_str()),
                Some("t1"),
                Some("g1"),
            ),
            agent(),
        )
        .expect_err("revoked approval must deny");
    match err.code {
        GatewayError::Denied { reason, .. } => {
            assert_eq!(reason.unwrap(), "approval revoked");
        }
        other => panic!("expected Denied, got {other:?}"),
    }
}

// ── Receipt binding (SEC-10/11) ──────────────────────────────

#[test]
fn receipt_binding_rejects_wrong_grant() {
    // SEC-10: the receipt binds the prepare's grant — an apply
    // asserting a different grant presents a foreign receipt.
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
    // A second ApproveToApply grant under the same task.
    rig.store.issue_grant(apply_grant(
        "g2",
        "t1",
        &["synvelis.clip.mute"],
        ApprovalPolicy::ApproveToApply,
    ));
    let (proposal, digest) = prepare_mute_agent(&mut rig, "t1", "g1");
    let outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    let GatewayOutcome::ApprovalRequired { approval } = outcome else {
        panic!("expected ApprovalRequired, got {outcome:?}");
    };
    complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );

    let err = rig
        .gw
        .handle(
            &apply_req(
                &proposal,
                &digest,
                Some(approval.as_str()),
                Some("t1"),
                Some("g2"),
            ),
            agent(),
        )
        .expect_err("grant-mismatched receipt must deny");
    match err.code {
        GatewayError::Denied { reason, .. } => {
            assert_eq!(reason.unwrap(), "approval bound to another grant");
        }
        other => panic!("expected Denied, got {other:?}"),
    }
}

#[test]
fn receipt_binding_rejects_wrong_task() {
    // SEC-10: the receipt binds the prepare's task — an apply under
    // a different task presents a foreign receipt even while
    // asserting the *same* grant (a project-persistent grant is
    // usable under any task, so only the task dimension differs).
    let mut rig = fixture_rig();
    issue_task(&rig.store, "t1");
    issue_task(&rig.store, "t2");
    let mut grant = apply_grant(
        "g1",
        "t1",
        &["synvelis.clip.mute"],
        ApprovalPolicy::ApproveToApply,
    );
    grant.binding = GrantBinding::PersistentProject;
    rig.store.issue_grant(grant);
    let (proposal, digest) = prepare_mute_agent(&mut rig, "t1", "g1");
    let outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    let GatewayOutcome::ApprovalRequired { approval } = outcome else {
        panic!("expected ApprovalRequired, got {outcome:?}");
    };
    complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );

    let err = rig
        .gw
        .handle(
            &apply_req(
                &proposal,
                &digest,
                Some(approval.as_str()),
                Some("t2"),
                Some("g1"),
            ),
            agent(),
        )
        .expect_err("task-mismatched receipt must deny");
    match err.code {
        GatewayError::Denied { reason, .. } => {
            assert_eq!(reason.unwrap(), "approval bound to another task");
        }
        other => panic!("expected Denied, got {other:?}"),
    }
}

#[test]
fn apply_hides_foreign_principals_proposals() {
    // SEC-10: existence blinding holds on the apply path — a second
    // agent with a perfectly valid DelegatedApply grant learns
    // `unknown proposal`, never that the proposal exists.
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

    // The foreign agent's own valid grant and task (DelegatedApply —
    // no receipt question in the way of the blinding check).
    let agent2 = Principal::new(PrincipalKind::Agent, "mcp-client-2").unwrap();
    let mut foreign = apply_grant(
        "g2",
        "t2",
        &["synvelis.clip.mute"],
        ApprovalPolicy::DelegatedApply,
    );
    foreign.principal = agent2.clone();
    rig.store.issue_task(AuthorizedTask {
        task: task_ref("t2"),
        principal: agent2,
        project: project(),
        activation: Activation::UserIssued,
        validity: Validity::default(),
    });
    rig.store.issue_grant(foreign);

    let err = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t2"), Some("g2")),
            adapter().agent("mcp-client-2").unwrap(),
        )
        .expect_err("foreign principal must not see the proposal");
    assert!(matches!(err.code, GatewayError::ProposalUnavailable { .. }));
}

#[test]
fn receipt_for_another_proposal_denies() {
    // SEC-09/10: a receipt names its proposal — presenting proposal
    // B's approval ref against proposal A's apply denies.
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
    let (p1, d1) = prepare_mute_agent(&mut rig, "t1", "g1");
    let (p2, d2) = prepare_mute_agent(&mut rig, "t1", "g1");
    // Both digest identically (same semantic change) — binding is by
    // proposal record, and the digest arg asserts content.
    assert_eq!(d1, d2);
    let outcome = rig
        .gw
        .handle(&apply_req(&p1, &d1, None, Some("t1"), Some("g1")), agent())
        .unwrap();
    let GatewayOutcome::ApprovalRequired {
        approval: approval_p1,
    } = outcome
    else {
        panic!("expected ApprovalRequired, got {outcome:?}");
    };
    // P1's receipt is granted; an apply for P2 presenting P1's ref
    // mismatches P2's record — deny, not cross-proposal reuse.
    complete(rig.gw.handle(&approve_req(&p1, &d1), human()).unwrap());
    let err = rig
        .gw
        .handle(
            &apply_req(&p2, &d2, Some(approval_p1.as_str()), Some("t1"), Some("g1")),
            agent(),
        )
        .expect_err("cross-proposal receipt must deny");
    match err.code {
        GatewayError::Denied { reason, .. } => {
            assert_eq!(reason.unwrap(), "approval does not match this proposal");
        }
        other => panic!("expected Denied, got {other:?}"),
    }
}

// ── Apply modes (SEC-12, GT-009) ─────────────────────────────

#[test]
fn suggest_mode_never_applies() {
    // SEC-12: a suggest-mode grant cannot mutate — the apply denies
    // at the apply gate before any receipt question is asked.
    let mut rig = fixture_rig();
    issue_agent(
        &rig.store,
        "t1",
        apply_grant("g1", "t1", &["synvelis.clip.mute"], ApprovalPolicy::Suggest),
    );
    let (proposal, digest) = prepare_mute_agent(&mut rig, "t1", "g1");
    let err = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .expect_err("suggest-mode apply must deny");
    match err.code {
        GatewayError::Denied { stage, reason } => {
            assert_eq!(stage, AuthorizationStage::RequestApply);
            assert_eq!(
                reason.unwrap(),
                "apply is not permitted under a suggest-mode grant"
            );
        }
        other => panic!("expected Denied, got {other:?}"),
    }
}

#[test]
fn delegated_apply_needs_no_receipt() {
    // DelegatedApply: grant+commit checks suffice — no per-change
    // receipt; the apply still flows through the gate (capability,
    // permissions, budget).
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
    let (proposal, digest) = prepare_mute_agent(&mut rig, "t1", "g1");
    let pref = ProposalRef::new(&proposal).unwrap();
    let outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    let GatewayOutcome::Accepted { operation } = outcome else {
        panic!("expected Accepted, got {outcome:?}");
    };
    let view = rig.proposals.view(&pref).unwrap();
    assert_eq!(
        view.apply.as_ref().unwrap().status,
        ApplyStatus::CommitRequested
    );
    assert_eq!(view.apply.as_ref().unwrap().operation, operation);
    // No receipt exists — none was required.
    assert!(view.approval.is_none());
}

#[test]
fn apply_denies_without_the_capability_in_grant() {
    // A grant that never names synvelis.proposal.apply cannot apply —
    // capability bounds deny before any receipt is consulted.
    let mut rig = fixture_rig();
    issue_agent(
        &rig.store,
        "t1",
        prep_grant(
            "g1",
            "t1",
            &["synvelis.clip.mute"],
            ResourceScope::Exact {
                resources: BTreeSet::from([clip11()]),
            },
            None,
            &mutate_classes(),
        ),
    );
    let (proposal, digest) = prepare_mute_agent(&mut rig, "t1", "g1");
    let err = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .expect_err("capability outside grant must deny");
    match err.code {
        GatewayError::Denied { reason, .. } => {
            assert_eq!(reason.unwrap(), "capability outside grant");
        }
        other => panic!("expected Denied, got {other:?}"),
    }
}

// ── Expiry / revision / lifecycle cascades ───────────────────

#[test]
fn expired_proposal_is_unavailable() {
    // A changeset past its expiry is dead: preview, approve, and
    // apply all fail closed — the record is retained for audit.
    let mut rig = fixture_rig();
    let (proposal, digest) = prepare_mute(&mut rig, human());
    let pref = ProposalRef::new(&proposal).unwrap();
    rig.clock.set(PolicyInstant::new(1_000 + PROPOSAL_TTL + 1));

    let view = rig.proposals.view(&pref).unwrap();
    assert_eq!(view.status, ProposalStatus::Expired);
    assert!(view.record.is_none(), "expired proposal drops the record");

    let err = rig
        .gw
        .handle(&preview_req(&proposal, None, None, None), human())
        .expect_err("expired proposal must not preview");
    assert!(matches!(err.code, GatewayError::ProposalUnavailable { .. }));
    let err = rig
        .gw
        .handle(&apply_req(&proposal, &digest, None, None, None), human())
        .expect_err("expired proposal must not apply");
    assert!(matches!(err.code, GatewayError::ProposalUnavailable { .. }));
}

#[test]
fn expired_approval_reopens_as_fresh_request() {
    // An expired receipt/request cannot authorize — the next apply
    // opens a fresh request instead of consuming the dead record.
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
    rig.gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );

    // The receipt outlives its expiry — the apply must wait again.
    rig.clock.set(PolicyInstant::new(1_000 + APPROVAL_TTL + 1));
    let pref = ProposalRef::new(&proposal).unwrap();
    assert_eq!(
        rig.proposals.view(&pref).unwrap().approval.unwrap().status,
        ApprovalStatus::Expired
    );
    let outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    let GatewayOutcome::ApprovalRequired { approval } = outcome else {
        panic!("expected ApprovalRequired, got {outcome:?}");
    };
    assert_eq!(
        rig.proposals.view(&pref).unwrap().approval.unwrap().status,
        ApprovalStatus::Requested
    );
    // After a fresh grant, this apply can consume.
    complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );
    let outcome = rig
        .gw
        .handle(
            &apply_req(
                &proposal,
                &digest,
                Some(approval.as_str()),
                Some("t1"),
                Some("g1"),
            ),
            agent(),
        )
        .unwrap();
    assert!(matches!(outcome, GatewayOutcome::Accepted { .. }));
}

#[test]
fn revision_conflict_rejects_apply() {
    // SEC-11/ACG-002: the prepared base must still be the live tip —
    // drift is a typed conflict, and the apply records it.
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
    let (proposal, digest) = prepare_mute_agent(&mut rig, "t1", "g1");
    let pref = ProposalRef::new(&proposal).unwrap();

    // The base moved on — the expected revision is stale.
    rig.source
        .commit(project(), ProjectRevision(2), demo_project());
    let err = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .expect_err("stale base must conflict");
    match err.code {
        GatewayError::RevisionConflict { expected, actual } => {
            assert_eq!(expected, 1);
            assert_eq!(actual, 2);
        }
        other => panic!("expected RevisionConflict, got {other:?}"),
    }
    assert_eq!(
        rig.proposals.view(&pref).unwrap().apply.unwrap().status,
        ApplyStatus::Conflict
    );
}

#[test]
fn expected_revision_mismatch_is_a_conflict() {
    // ACG-002: the apply must assert exactly the prepared base — an
    // expected revision that moved on is never a silent rebase.
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
    let (proposal, digest) = prepare_mute_agent(&mut rig, "t1", "g1");
    let mut req = apply_req(&proposal, &digest, None, Some("t1"), Some("g1"));
    req["consistency"] = expected(9);
    let err = rig
        .gw
        .handle(&req, agent())
        .expect_err("mismatched expectation must conflict");
    assert!(matches!(err.code, GatewayError::RevisionConflict { .. }));
}

#[test]
fn invalidated_proposal_kills_its_approval() {
    // Cascade: invalidating a proposal invalidates its live
    // approval — a receipt bound to dropped content cannot be spent.
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
    let pref = ProposalRef::new(&proposal).unwrap();
    rig.gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );

    rig.proposals.evict(&pref);
    let view = rig.proposals.view(&pref).unwrap();
    assert_eq!(view.status, ProposalStatus::Invalidated);
    assert_eq!(
        view.approval.as_ref().unwrap().status,
        ApprovalStatus::Invalidated
    );
    let err = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .expect_err("invalidated proposal must not apply");
    assert!(matches!(err.code, GatewayError::ProposalUnavailable { .. }));
}

// ── State machines — the full transition tables ──────────────

/// Asserts `transit` produces exactly `expected` and nothing else —
/// the table-driven proof for one lifecycle machine.
fn assert_transition_table<S, E>(
    statuses: &[S],
    events: &[E],
    expected: &[(S, E, S)],
    transit: impl Fn(S, E) -> Option<S>,
) where
    S: Copy + std::fmt::Debug + PartialEq,
    E: Copy + std::fmt::Debug + PartialEq,
{
    for &status in statuses {
        for &event in events {
            let next = transit(status, event);
            let legal = expected
                .iter()
                .find(|(s, e, _)| *s == status && *e == event);
            match legal {
                Some((_, _, want)) => assert_eq!(
                    next,
                    Some(*want),
                    "{status:?} + {event:?} must produce {want:?}"
                ),
                None => assert_eq!(next, None, "{status:?} + {event:?} must not transit"),
            }
        }
    }
}

#[test]
fn proposal_transition_table_is_exact() {
    // `Event::ALL` keeps the coverage closed — a future variant
    // escapes neither the allowed nor the forbidden side.
    assert_transition_table(
        &ProposalStatus::ALL,
        &ProposalEvent::ALL,
        &[
            (
                ProposalStatus::Prepared,
                ProposalEvent::Expire,
                ProposalStatus::Expired,
            ),
            (
                ProposalStatus::Prepared,
                ProposalEvent::Invalidate,
                ProposalStatus::Invalidated,
            ),
        ],
        ProposalStatus::transit,
    );
}

#[test]
fn approval_transition_table_is_exact() {
    use ApprovalEvent as E;
    use ApprovalStatus as S;
    assert_transition_table(
        &S::ALL,
        &E::ALL,
        &[
            (S::Requested, E::Grant, S::Granted),
            (S::Requested, E::Decline, S::Declined),
            (S::Requested, E::Expire, S::Expired),
            (S::Requested, E::Revoke, S::Revoked),
            (S::Requested, E::Invalidate, S::Invalidated),
            (S::Granted, E::Expire, S::Expired),
            (S::Granted, E::Revoke, S::Revoked),
            (S::Granted, E::Invalidate, S::Invalidated),
            (S::Granted, E::Consume, S::Consumed),
        ],
        ApprovalStatus::transit,
    );
}

#[test]
fn apply_transition_table_is_exact() {
    use ApplyEvent as E;
    use ApplyStatus as S;
    assert_transition_table(
        &S::ALL,
        &E::ALL,
        &[
            (S::AwaitingApproval, E::Accept, S::CommitRequested),
            (S::AwaitingApproval, E::Reject, S::Rejected),
            (S::AwaitingApproval, E::Conflict, S::Conflict),
            (S::CommitRequested, E::Commit, S::Committed),
            (S::CommitRequested, E::Reject, S::Rejected),
            (S::CommitRequested, E::Conflict, S::Conflict),
            (S::CommitRequested, E::Indeterminate, S::Indeterminate),
        ],
        ApplyStatus::transit,
    );
}

// ── GW-06 seam contract (ACG-004) ────────────────────────────

#[test]
fn commit_outcome_is_gw06s_to_report() {
    // ACG-004: an accepted apply is CommitRequested — never a
    // success claim; GW-06's settle reports the terminal outcome,
    // and the apply record is the durable handle a retry references.
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
    let (proposal, digest) = prepare_mute_agent(&mut rig, "t1", "g1");
    let pref = ProposalRef::new(&proposal).unwrap();
    let outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    assert!(matches!(outcome, GatewayOutcome::Accepted { .. }));

    // GW-06 commits — the machine moves to its terminal state.
    let record = rig
        .proposal_engine
        .settle_apply(&pref, ApplyEvent::Commit)
        .unwrap();
    assert_eq!(record.status, ApplyStatus::Committed);
    // A retry still references the accepted record.
    let outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    assert!(matches!(outcome, GatewayOutcome::Accepted { .. }));

    // An illegal settle is a typed error — no silent state drift.
    let err = rig
        .proposal_engine
        .settle_apply(&pref, ApplyEvent::Conflict)
        .expect_err("commit on committed must fail");
    assert!(matches!(err.code, GatewayError::Internal { .. }));
}

#[test]
fn settle_requires_an_apply() {
    let mut rig = fixture_rig();
    let (proposal, _) = prepare_mute(&mut rig, human());
    let pref = ProposalRef::new(&proposal).unwrap();
    let err = rig
        .proposal_engine
        .settle_apply(&pref, ApplyEvent::Commit)
        .expect_err("no apply to settle");
    assert!(matches!(err.code, GatewayError::Internal { .. }));
}

#[test]
fn double_consumption_can_never_succeed() {
    // ACG-004 + SEC-09: the consume is atomic — once spent, the same
    // receipt cannot fund another commit, even through the GW-06 seam.
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
    let pref = ProposalRef::new(&proposal).unwrap();
    let outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    let GatewayOutcome::ApprovalRequired { approval } = outcome else {
        panic!("expected ApprovalRequired, got {outcome:?}");
    };
    complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );

    // The wire apply consumes it.
    rig.gw
        .handle(
            &apply_req(
                &proposal,
                &digest,
                Some(approval.as_str()),
                Some("t1"),
                Some("g1"),
            ),
            agent(),
        )
        .unwrap();

    // Even GW-06's direct consume seam can never spend it twice.
    let req = rig
        .gw
        .bind(
            Gateway::parse_wire(&apply_req(
                &proposal,
                &digest,
                Some(approval.as_str()),
                Some("t1"),
                Some("g1"),
            ))
            .unwrap(),
            agent(),
        )
        .unwrap();
    let err = rig
        .proposal_engine
        .try_consume_approval(&pref, &approval, &req)
        .expect_err("consumed receipt must not re-spend");
    assert!(matches!(err.code, GatewayError::ApprovalInvalid { .. }));
}

#[test]
fn rejection_reopens_the_approval_lifecycle() {
    // A terminal apply ends the receipt's story: when GW-06 reports
    // the commit refused, the spent receipt stays `Consumed` — it can
    // never re-spend — but the proposal's approval lifecycle reopens
    // for a fresh decision, so a routine rejection does not strand an
    // otherwise-live proposal.
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
    let pref = ProposalRef::new(&proposal).unwrap();
    let outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    let GatewayOutcome::ApprovalRequired { approval } = outcome else {
        panic!("expected ApprovalRequired, got {outcome:?}");
    };
    complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );
    rig.gw
        .handle(
            &apply_req(
                &proposal,
                &digest,
                Some(approval.as_str()),
                Some("t1"),
                Some("g1"),
            ),
            agent(),
        )
        .unwrap();

    // GW-06's commit refused — the apply is terminal Rejected and the
    // receipt stays Consumed (a second spend is still impossible).
    rig.proposal_engine
        .settle_apply(&pref, ApplyEvent::Reject)
        .unwrap();
    let view = rig.proposals.view(&pref).unwrap();
    assert_eq!(
        view.approval.as_ref().unwrap().status,
        ApprovalStatus::Consumed
    );

    // A retry — even presenting the spent ref — waits on a fresh
    // decision: the request reopens instead of denying outright.
    let outcome = rig
        .gw
        .handle(
            &apply_req(
                &proposal,
                &digest,
                Some(approval.as_str()),
                Some("t1"),
                Some("g1"),
            ),
            agent(),
        )
        .unwrap();
    let GatewayOutcome::ApprovalRequired {
        approval: fresh_ref,
    } = outcome
    else {
        panic!("expected a reopened ApprovalRequired, got {outcome:?}");
    };
    assert_ne!(fresh_ref, approval, "the reopen mints a fresh request");
    let view = rig.proposals.view(&pref).unwrap();
    assert_eq!(
        view.approval.as_ref().unwrap().status,
        ApprovalStatus::Requested
    );

    // The fresh grant funds the retry — a real second approval, not
    // a resurrection of the consumed receipt.
    complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );
    let outcome = rig
        .gw
        .handle(
            &apply_req(
                &proposal,
                &digest,
                Some(fresh_ref.as_str()),
                Some("t1"),
                Some("g1"),
            ),
            agent(),
        )
        .unwrap();
    assert!(matches!(outcome, GatewayOutcome::Accepted { .. }));
}
