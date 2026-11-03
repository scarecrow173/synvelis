//! GW-05 review-regression tests — defects found by the independent
//! review pass (issue #57): proposal death must never falsify an
//! accepted apply, the gate-to-accept path is atomic, dead entries
//! evict before live ones, receipt consumption re-verifies only for
//! the apply that spent it, a terminal apply ends (and may reopen)
//! the approval lifecycle, settlement reports only commit outcomes,
//! and preview enforces the emitted-classes bound — not a wider
//! descriptor floor.

mod rig;

use rig::*;
use serde_json::json;
use synvelis_gateway::*;
use synvelis_project::ProjectRevision;

// ══ Proposal death vs the commit path ═══════════════════════

#[test]
fn accepted_apply_survives_proposal_expiry() {
    // Review P1: lazy expiry must never rewrite an accepted
    // `CommitRequested` apply to `Rejected` — the record is GW-06's
    // to settle, and falsifying it would strand the fence's
    // consumption/commit consistency check.
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

    // The proposal ages out — the apply record is untouched.
    rig.clock.advance(PROPOSAL_TTL + 1);
    let view = rig.proposals.view(&pref).unwrap();
    assert_eq!(view.status, ProposalStatus::Expired);
    assert_eq!(
        view.apply.as_ref().unwrap().status,
        ApplyStatus::CommitRequested,
        "a dead proposal never falsifies an accepted apply"
    );

    // GW-06 still settles it — the commit outcome is its to report.
    let record = rig
        .proposal_engine
        .settle_apply(&pref, ApplyEvent::Commit)
        .unwrap();
    assert_eq!(record.status, ApplyStatus::Committed);
    assert_eq!(record.operation, operation);
}

#[test]
fn accepted_apply_survives_invalidation() {
    // The explicit-evict path shares the guarantee: `AwaitingApproval`
    // is refused with the proposal, `CommitRequested` is not.
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
    rig.gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();

    assert!(rig.proposals.evict(&pref));
    let view = rig.proposals.view(&pref).unwrap();
    assert_eq!(view.status, ProposalStatus::Invalidated);
    assert_eq!(
        view.apply.as_ref().unwrap().status,
        ApplyStatus::CommitRequested
    );
}

#[test]
fn waiting_apply_is_refused_with_the_proposal() {
    // The flip side of the same fix: an apply still *waiting* on the
    // proposal dies with it — the refused attempt is honest evidence.
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
    assert_eq!(
        rig.proposals.view(&pref).unwrap().apply.unwrap().status,
        ApplyStatus::AwaitingApproval
    );

    assert!(rig.proposals.evict(&pref));
    let view = rig.proposals.view(&pref).unwrap();
    assert_eq!(
        view.apply.as_ref().unwrap().status,
        ApplyStatus::Rejected,
        "the waiting attempt is refused with its proposal"
    );
}

#[test]
fn dead_proposal_grows_no_lifecycle_records() {
    // Review P1: an apply against a dead proposal must not mint an
    // approval request or an apply record — dead entries record the
    // lifecycle they lived, not ones they never had.
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
    assert!(rig.proposals.evict(&pref));

    let err = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .expect_err("a dead proposal cannot be applied");
    assert!(matches!(err.code, GatewayError::ProposalUnavailable { .. }));
    let view = rig.proposals.view(&pref).unwrap();
    assert!(
        view.approval.is_none() && view.apply.is_none(),
        "the dead entry records nothing it never lived"
    );
}

// ══ Receipt refs, binding, and consumption ══════════════════

#[test]
fn proactive_approval_funds_apply_on_the_record_ref() {
    // Review P1: a receipt issued *before* any apply request carries
    // a ref the wire never saw (`approval-<counter>`, not
    // `approval-for-<operation>`). The apply need not present it —
    // the granted record funds the apply on the record's own ref,
    // and the agent never declares receipt identity.
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

    complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );

    // No `approval` arg — the bound receipt funds this apply anyway.
    let outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    assert!(matches!(outcome, GatewayOutcome::Accepted { .. }));
    assert_eq!(
        rig.proposals.view(&pref).unwrap().approval.unwrap().status,
        ApprovalStatus::Consumed,
        "the record's own receipt was spent"
    );
}

#[test]
fn the_approve_result_ref_stays_presentable() {
    // The ref the approve response names is also valid when
    // presented — the wire-learned ref and the record agree.
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
    let receipt = complete(
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
                Some(receipt["approval"].as_str().unwrap()),
                Some("t1"),
                Some("g1"),
            ),
            agent(),
        )
        .unwrap();
    assert!(matches!(outcome, GatewayOutcome::Accepted { .. }));
}

#[test]
fn a_superseded_request_ref_is_denied() {
    // Review P2 → confirmed boundary: while a request is `Requested`
    // the record's ref tracks the latest attempt (last-told wins) —
    // a stale `ApprovalRequired` ref the wire was once told can no
    // longer authorize, even after the request is granted.
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
    let GatewayOutcome::ApprovalRequired {
        approval: stale_ref,
    } = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap()
    else {
        panic!("expected ApprovalRequired");
    };
    let GatewayOutcome::ApprovalRequired {
        approval: fresh_ref,
    } = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap()
    else {
        panic!("expected ApprovalRequired");
    };
    assert_ne!(stale_ref, fresh_ref);
    complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );

    // The stale ref denies; the fresh ref applies.
    let err = rig
        .gw
        .handle(
            &apply_req(
                &proposal,
                &digest,
                Some(stale_ref.as_str()),
                Some("t1"),
                Some("g1"),
            ),
            agent(),
        )
        .expect_err("the superseded ref is dead");
    match err.code {
        GatewayError::Denied { stage, reason } => {
            assert_eq!(stage, AuthorizationStage::RequestApply);
            assert_eq!(reason.unwrap(), "approval does not match this proposal");
        }
        other => panic!("expected Denied, got {other:?}"),
    }
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

#[test]
fn consume_verify_reconfirms_only_the_spending_attempt() {
    // Review P1: GW-06's consume seam is an idempotent *verify* for
    // the exact attempt that consumed — same bound request succeeds,
    // any other bound request sees the spent receipt's Invalid.
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

    // The attempt opens, is granted, and consumes — all on one bound
    // operation.
    let opener = rig
        .gw
        .bind(
            Gateway::parse_wire(&apply_req(&proposal, &digest, None, Some("t1"), Some("g1")))
                .unwrap(),
            agent(),
        )
        .unwrap();
    let GatewayOutcome::ApprovalRequired { approval } = rig.gw.invoke(&opener).unwrap() else {
        panic!("expected ApprovalRequired");
    };
    complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );
    let accepted = rig
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
    let outcome = rig.gw.invoke(&accepted).unwrap();
    assert!(matches!(outcome, GatewayOutcome::Accepted { .. }));

    // The fence's consumption/commit check re-confirms for the
    // consuming attempt…
    let receipt = rig
        .proposal_engine
        .try_consume_approval(&pref, &approval, &accepted)
        .unwrap();
    assert_eq!(receipt.digest.as_str(), digest);
    // …but the pre-approval attempt (or any other) can never re-spend
    // or falsely verify.
    let err = rig
        .proposal_engine
        .try_consume_approval(&pref, &approval, &opener)
        .expect_err("a different attempt cannot verify the spend");
    assert!(matches!(err.code, GatewayError::ApprovalInvalid { .. }));
}

#[test]
fn seam_consume_requires_the_accepted_attempt() {
    // Review A-4: the consume seam must never spend a receipt into
    // the void — a `Granted` receipt with no recorded apply is not
    // consumable, and only the `CommitRequested` apply's own
    // operation may verify its spend. A `Consumed` approval with no
    // bound apply is a permanent dead-end the lifecycle cannot
    // explain; the seam now refuses to create one.
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
    complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );
    let record_ref = rig.proposal_engine.approval_record(&pref).unwrap().approval;

    // No apply attempt exists — the seam refuses to spend.
    let req = rig
        .gw
        .bind(
            Gateway::parse_wire(&apply_req(
                &proposal,
                &digest,
                Some(record_ref.as_str()),
                Some("t1"),
                Some("g1"),
            ))
            .unwrap(),
            agent(),
        )
        .unwrap();
    let err = rig
        .proposal_engine
        .try_consume_approval(&pref, &record_ref, &req)
        .expect_err("consumption must bind a recorded accepted apply");
    assert!(matches!(err.code, GatewayError::ApprovalInvalid { .. }));
    // The receipt was never spent — the normal apply path still can.
    assert_eq!(
        rig.proposals.view(&pref).unwrap().approval.unwrap().status,
        ApprovalStatus::Granted
    );
}

// ══ Apply lifecycle honesty ═════════════════════════════════

#[test]
fn committed_apply_references_itself_after_expiry() {
    // The existing-result contract holds across proposal death: a
    // retry on a committed apply names the recorded operation — no
    // second consume, no second commit request.
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
    let GatewayOutcome::Accepted { operation } = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap()
    else {
        panic!("expected Accepted");
    };
    rig.proposal_engine
        .settle_apply(&pref, ApplyEvent::Commit)
        .unwrap();

    // Past the proposal's expiry the retry still references the
    // committed record — the durable truth, not a fresh attempt.
    rig.clock.advance(PROPOSAL_TTL + 1);
    let outcome = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();
    let GatewayOutcome::Accepted {
        operation: referenced,
    } = outcome
    else {
        panic!("expected the recorded Accepted, got {outcome:?}");
    };
    assert_eq!(referenced, operation);
}

#[test]
fn settle_apply_reports_only_commit_outcomes() {
    // Review P1: `Accept` is the apply gate's transition — the fence
    // may only report `Commit`/`Reject`/`Conflict`/`Indeterminate`,
    // and re-reporting the current outcome is idempotent.
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
    rig.gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap();

    let err = rig
        .proposal_engine
        .settle_apply(&pref, ApplyEvent::Accept)
        .expect_err("Accept is not a commit outcome");
    assert!(matches!(err.code, GatewayError::Internal { .. }));

    let committed = rig
        .proposal_engine
        .settle_apply(&pref, ApplyEvent::Commit)
        .unwrap();
    assert_eq!(committed.status, ApplyStatus::Committed);
    // A re-report of the recorded outcome is a no-op, not an error —
    // the fence cannot prove it already reported.
    let again = rig
        .proposal_engine
        .settle_apply(&pref, ApplyEvent::Commit)
        .unwrap();
    assert_eq!(again.status, ApplyStatus::Committed);
    // A different outcome on a settled apply is illegal.
    rig.proposal_engine
        .settle_apply(&pref, ApplyEvent::Reject)
        .expect_err("a settled apply cannot flip outcome");
}

#[test]
fn a_live_apply_survives_another_attempts_failure() {
    // Review P1: a second attempt's revision-conflict must not
    // overwrite the open `AwaitingApproval` record — failure evidence
    // belongs to the attempt that failed, and one store tracks one
    // live apply.
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
    let GatewayOutcome::ApprovalRequired { approval } = rig
        .gw
        .handle(
            &apply_req(&proposal, &digest, None, Some("t1"), Some("g1")),
            agent(),
        )
        .unwrap()
    else {
        panic!("expected ApprovalRequired");
    };
    complete(
        rig.gw
            .handle(&approve_req(&proposal, &digest), human())
            .unwrap(),
    );

    // The base moves before the granted apply lands — the fence
    // conflicts…
    rig.source
        .commit(project(), ProjectRevision(2), demo_project());
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
        .expect_err("the base moved under the proposal");
    assert!(matches!(err.code, GatewayError::RevisionConflict { .. }));
    // …but the open attempt's record survives — the conflict was
    // this attempt's evidence, not a lifecycle transition.
    let view = rig.proposals.view(&pref).unwrap();
    assert_eq!(
        view.apply.as_ref().unwrap().status,
        ApplyStatus::AwaitingApproval,
        "the live apply record is never clobbered by a failure"
    );
}

// ══ Retention and changeset identity ════════════════════════

#[test]
fn dead_entries_evict_before_live_ones() {
    // Review P1: a bounded store under pressure spends dead
    // tombstones first — a live proposal (and the granted receipt it
    // may carry) is not expendable where a finished lifecycle is.
    let mut rig = fixture_rig_with_cap(2);
    let (p1, _) = prepare_mute(&mut rig, human());
    let (p2, _) = prepare_mute(&mut rig, human());
    let pref1 = ProposalRef::new(&p1).unwrap();
    let pref2 = ProposalRef::new(&p2).unwrap();
    assert!(rig.proposals.evict(&pref2));
    assert_eq!(rig.proposals.count(), 2, "the tombstone stays for audit");

    let (p3, _) = prepare_mute(&mut rig, human());
    let pref3 = ProposalRef::new(&p3).unwrap();

    assert_eq!(
        rig.proposals.view(&pref1).unwrap().status,
        ProposalStatus::Prepared,
        "the live proposal survives capacity pressure"
    );
    assert!(
        rig.proposals.view(&pref2).is_none(),
        "the dead tombstone was spent first"
    );
    assert_eq!(
        rig.proposals.view(&pref3).unwrap().status,
        ProposalStatus::Prepared
    );
}

#[test]
fn capacity_never_drops_a_live_proposal() {
    // Review A-5: when every retained entry is live the cap refuses
    // the insert — `LimitExceeded` is honest backpressure; silently
    // evicting an open apply or a granted receipt is not.
    let mut rig = fixture_rig_with_cap(2);
    let (p1, _) = prepare_mute(&mut rig, human());
    let (p2, _) = prepare_mute(&mut rig, human());

    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                op(
                    "synvelis.clip.mute",
                    res_target("clip", &clip11()),
                    json!({"muted": true}),
                ),
                expected(1),
            ),
            human(),
        )
        .expect_err("an all-live store applies backpressure");
    match err.code {
        GatewayError::LimitExceeded { limit, max } => {
            assert_eq!(limit, "proposals");
            assert_eq!(max, 2);
        }
        other => panic!("expected LimitExceeded, got {other:?}"),
    }
    // Both live proposals are intact — nothing was silently dropped.
    assert_eq!(
        rig.proposals
            .view(&ProposalRef::new(&p1).unwrap())
            .unwrap()
            .status,
        ProposalStatus::Prepared
    );
    assert_eq!(
        rig.proposals
            .view(&ProposalRef::new(&p2).unwrap())
            .unwrap()
            .status,
        ProposalStatus::Prepared
    );
}

#[test]
fn changeset_exposes_its_schema_identity() {
    // The digest-bound schema identity is readable — a consumer never
    // guesses the format it is verifying.
    let mut rig = fixture_rig();
    let (proposal, _) = prepare_mute(&mut rig, human());
    let changeset = rig
        .proposal_engine
        .changeset(&ProposalRef::new(&proposal).unwrap())
        .unwrap();
    assert_eq!(changeset.schema(), "synvelis.prepared-changeset");
    assert_eq!(changeset.schema_version(), CapabilityVersion::new(1, 0));
    assert!(changeset.verify());
}

// ══ Preview emission boundary ═══════════════════════════════

#[test]
fn preview_follows_the_emitted_classes() {
    // Review P1: the preview descriptor's floor is metadata — the
    // real bound is the changeset's emitted classes, enforced at the
    // same emission boundary the prepare result passed. A grant that
    // covers mute metadata but not structured note content previews
    // the mute and is denied the note diff — even though the agent
    // owns both proposals.
    let mut rig = fixture_rig();
    // `g1` prepares and applies; `g2` is the same principal's
    // metadata-only read grant.
    let mut full = apply_grant(
        "g1",
        "t1",
        &["synvelis.clip.mute", "synvelis.midi.note.add"],
        ApprovalPolicy::DelegatedApply,
    );
    // The note write creates a note inside the source.
    full.create_within = Some(CreateWithin {
        parents: OwnerConstraint::OneOf {
            refs: std::collections::BTreeSet::from([source10()]),
        },
        kinds: std::collections::BTreeSet::from([ResourceKind::Note]),
        placement: None,
    });
    issue_agent(&rig.store, "t1", full);
    let mut narrow = apply_grant("g2", "t1", &[], ApprovalPolicy::DelegatedApply);
    narrow.data_access = DataAccessSet::of(mutate_classes());
    rig.store.issue_grant(narrow);

    // Metadata-only change: the narrow grant's preview is within its
    // classes.
    let (mute_proposal, _) = prepare_mute_agent(&mut rig, "t1", "g1");
    let preview = complete(
        rig.gw
            .handle(
                &preview_req(&mute_proposal, None, Some("t1"), Some("g2")),
                agent(),
            )
            .unwrap(),
    );
    assert_eq!(preview["proposal"], json!(mute_proposal));

    // A note write emits `StructuredContentRead` — outside `g2`'s
    // classes, so its preview denies even though the proposal is the
    // same agent's own.
    let result = complete(
        rig.gw
            .handle(
                &prepare_req(
                    Some("t1"),
                    Some("g1"),
                    op(
                        "synvelis.midi.note.add",
                        res_target("source", &source10()),
                        json!({"key": 65, "velocity": 90, "range": {"domain": "source-native", "start": 720, "len": 240}}),
                    ),
                    expected(1),
                ),
                agent(),
            )
            .unwrap(),
    );
    let note_proposal = result["proposal"].as_str().unwrap();
    let err = rig
        .gw
        .handle(
            &preview_req(note_proposal, None, Some("t1"), Some("g2")),
            agent(),
        )
        .expect_err("structured content exceeds this grant");
    assert!(matches!(err.code, GatewayError::Denied { .. }));

    // The full grant previews the same proposal: entity-level
    // `created` names the new note, and the note pair is `added`.
    let preview = complete(
        rig.gw
            .handle(
                &preview_req(note_proposal, None, Some("t1"), Some("g1")),
                agent(),
            )
            .unwrap(),
    );
    let entities = preview["diff"]["entities"].as_array().unwrap();
    assert!(
        entities
            .iter()
            .any(|e| e["change"] == json!("created") && e["after"].is_object()),
        "created entity missing from {entities:?}"
    );
    assert!(
        preview["diff"]["midi_notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["change"] == json!("added") && n["after"]["key"] == json!(65)),
        "added note missing from preview"
    );
}
