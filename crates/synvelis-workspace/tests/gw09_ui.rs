//! GW-09 acceptance tests — the headless Connection / Task / Grant /
//! Approval UI contract (issue #60).
//!
//! Coverage map:
//! - UX-01 AI-runtime vs external-MCP switches, default off, independent.
//! - UX-02 fixed/dynamic scope + linked-source + expiry/epoch/budget.
//! - UX-03 pinned approval surface (changeset/revision/effects/digest).
//! - UX-04 explicit-id rows + grant-revoke vs job-cancel distinction.
//! - UX-05 proposal/committed/applied/saved activity axes.
//! - UX-06 normal DAW stays usable with AI/MCP off (SYS-01).
//! - SYS-02 grant pinned by explicit id across "windows".
//! - SYS-03 selection/window/proposal-swap cannot retarget an approval.
//! - SYS-04 expired proposal/grant cannot take an old approval.
//! - SYS-05 revoke requested vs confirmed; cancel requested ≠ cancelled.
//! - SYS-06 committed vs engine-applied vs saved stay distinct.

mod rig;

use std::collections::BTreeSet;

use synvelis_gateway::*;
use synvelis_project::{ClipId, ProjectOp, ProjectRevision};
use synvelis_workspace::*;

use rig::*;

// ── helpers ──────────────────────────────────────────────────

/// Issue a task + grant through the UI (the explicit-id path under
/// test), returning the grant for scope/policy tweaks before issue.
fn issue_agent(fix: &mut UiFixture, task: &str, grant: &Grant) {
    fix.ui.issue_task(&task_of(task)).unwrap();
    fix.ui.issue_grant(grant).unwrap();
}

fn task_of(tag: &str) -> AuthorizedTask {
    task(tag)
}

fn mute_grant(tag: &str, task: &str, policy: ApprovalPolicy) -> Grant {
    agent_grant(
        tag,
        task,
        &["synvelis.clip.mute"],
        ResourceScope::Exact {
            resources: BTreeSet::from([clip11(), clip12(), clip13()]),
        },
        policy,
    )
}

fn note_grant(tag: &str, task: &str, policy: ApprovalPolicy) -> Grant {
    let mut grant = agent_grant(
        tag,
        task,
        &["synvelis.midi.note.add"],
        ResourceScope::Exact {
            resources: BTreeSet::from([clip12(), clip13(), source10()]),
        },
        policy,
    );
    // midi.note.add carries a Create effect — the grant must declare
    // where notes may be created (inside source10 only).
    grant.create_within = Some(CreateWithin {
        parents: OwnerConstraint::OneOf {
            refs: BTreeSet::from([source10()]),
        },
        kinds: BTreeSet::from([ResourceKind::Note]),
        placement: None,
    });
    grant
}

/// The `Accepted` operation id out of a gateway outcome.
fn accepted(outcome: &GatewayOutcome<serde_json::Value>) -> OperationId {
    match outcome {
        GatewayOutcome::Accepted { operation } => *operation,
        other => panic!("expected Accepted, got {other:?}"),
    }
}

// ══════════════════════════════════════════════════════════════
// UX-01 / UX-06 / SYS-01 — switches + AI-off regression
// ══════════════════════════════════════════════════════════════

#[test]
fn ux01_switches_default_off_and_independent() {
    let mut fix = boot(None);
    let panel = fix.ui.switch_panel();
    assert_eq!(panel.switches.len(), 2);
    assert!(panel.switches.iter().all(|s| !s.enabled));
    assert!(
        !fix.ui
            .mcp_config(AdapterId::new(9), McpBounds::default())
            .enabled
    );

    // Independent: each switch moves alone.
    let panel = fix.ui.set_switch(AiFeature::AiRuntime, true);
    assert!(
        panel
            .switches
            .iter()
            .find(|s| s.feature == AiFeature::AiRuntime)
            .unwrap()
            .enabled
    );
    assert!(
        !panel
            .switches
            .iter()
            .find(|s| s.feature == AiFeature::ExternalMcp)
            .unwrap()
            .enabled
    );
    assert!(
        !fix.ui
            .mcp_config(AdapterId::new(9), McpBounds::default())
            .enabled
    );

    let panel = fix.ui.set_switch(AiFeature::ExternalMcp, true);
    assert!(
        panel
            .switches
            .iter()
            .find(|s| s.feature == AiFeature::ExternalMcp)
            .unwrap()
            .enabled
    );
    assert!(
        fix.ui
            .mcp_config(AdapterId::new(9), McpBounds::default())
            .enabled
    );
}

#[test]
fn ux06_sys01_ai_off_normal_daw_unaffected() {
    let mut fix = boot(None);
    // Both switches off (default) — the workspace session must edit
    // and undo exactly as before the gateway existed.
    let panel = fix.ui.switch_panel();
    assert!(panel.switches.iter().all(|s| !s.enabled));

    let rev = fix.ws.session().revision();
    fix.ws
        .session_mut()
        .apply(
            ProjectOp::SetClipMuted {
                clip_id: ClipId::new(11),
                muted: true,
            },
            rev,
        )
        .expect("manual op applies");
    assert!(fix.ws.session().project().clips[&ClipId::new(11)].muted);
    assert!(fix.ws.session_mut().undo().unwrap());
    assert!(!fix.ws.session().project().clips[&ClipId::new(11)].muted);

    // The gate's project is untouched — no workspace edit leaked into
    // the gateway side while AI was off.
    assert_eq!(fix.gate.live_revision(), ProjectRevision(1));
}

// ══════════════════════════════════════════════════════════════
// UX-02 — grant scope explanation
// ══════════════════════════════════════════════════════════════

#[test]
fn ux02_exact_scope_explanation() {
    let mut fix = boot(None);
    issue_agent(
        &mut fix,
        "t-scope",
        &mute_grant("g-exact", "t-scope", ApprovalPolicy::DelegatedApply),
    );

    let ex = fix.ui.grant_explanation(&grant_ref("g-exact")).unwrap();
    match &ex.scope {
        ScopeExplanation::Exact { members, resources } => {
            assert_eq!(*members, 3);
            assert!(resources.contains(&tagged(&clip11())));
        }
        other => panic!("expected Exact scope, got {other:?}"),
    }
    assert_eq!(ex.mode, ApplyMode::DelegatedApply);
    // The linked-source honesty note is always present (UX-02).
    assert!(
        ex.notes
            .iter()
            .any(|n| n.contains("clip scope never covers"))
    );
    let row = fix
        .ui
        .grant_rows()
        .into_iter()
        .find(|r| r.grant == grant_ref("g-exact"))
        .unwrap();
    assert_eq!(row.principal, "mcp-client-1");
    assert_eq!(row.binding, "task:t-scope");
    assert_eq!(row.revocation, Revocation::None);
    assert!(row.live);
}

#[test]
fn ux02_dynamic_scope_explanation() {
    let mut fix = boot(None);
    let dynamic = ResourceScope::Dynamic(DynamicScope {
        kinds: BTreeSet::from([ResourceKind::Clip]),
        owners: OwnerConstraint::OneOf {
            refs: BTreeSet::from([lane4()]),
        },
        range: None,
        max_members: Some(4),
        include_future: true,
        move_args: None,
    });
    issue_agent(
        &mut fix,
        "t-dyn",
        &agent_grant(
            "g-dyn",
            "t-dyn",
            &["synvelis.clip.mute"],
            dynamic,
            ApprovalPolicy::Suggest,
        ),
    );

    let ex = fix.ui.grant_explanation(&grant_ref("g-dyn")).unwrap();
    match &ex.scope {
        ScopeExplanation::Dynamic {
            kinds,
            owners,
            range_bound,
            max_members,
            include_future,
        } => {
            assert_eq!(kinds, &[ResourceKind::Clip]);
            assert!(*include_future);
            assert_eq!(*max_members, Some(4));
            assert!(!range_bound);
            match owners {
                OwnerExplanation::OneOf { count, refs } => {
                    assert_eq!(*count, 1);
                    assert_eq!(refs, &[tagged(&lane4())]);
                }
                other => panic!("expected OneOf owners, got {other:?}"),
            }
        }
        other => panic!("expected Dynamic scope, got {other:?}"),
    }
    assert_eq!(ex.mode, ApplyMode::Suggest);
    assert!(
        ex.notes
            .iter()
            .any(|n| n.contains("created after issuance"))
    );
}

#[test]
fn ux02_capture_scope_is_a_pinned_snapshot() {
    let mut grant = mute_grant("g-cap", "t-cap", ApprovalPolicy::DelegatedApply);
    let selection = selection_of(&[
        SemanticTarget::Clip(ClipId::new(11)),
        SemanticTarget::Clip(ClipId::new(12)),
    ]);
    let members = capture_scope(&mut grant, &selection, ProjectRevision(1)).unwrap();
    assert_eq!(members, 2);
    match &grant.scope {
        ResourceScope::Snapshot {
            resources,
            captured_revision,
        } => {
            assert_eq!(resources.len(), 2);
            assert_eq!(captured_revision.0, 1);
        }
        other => panic!("expected Snapshot scope, got {other:?}"),
    }

    // An empty selection must never mint an empty scope.
    let empty = EditingSelection::default();
    assert!(matches!(
        capture_scope(&mut grant, &empty, ProjectRevision(1)),
        Err(GatewayUiError::EmptyCapture)
    ));
}

#[test]
fn ux02_expiry_epoch_budget_explained() {
    let mut fix = boot(None);
    let mut grant = mute_grant("g-limits", "t-limits", ApprovalPolicy::ApproveToApply);
    grant.validity.expires_at = Some(PolicyInstant::new(5_000));
    grant.epoch_ceiling = Some(PolicyEpoch(3));
    grant.limits.apply_budget = Some(2);
    issue_agent(&mut fix, "t-limits", &grant);

    let ex = fix.ui.grant_explanation(&grant_ref("g-limits")).unwrap();
    assert_eq!(ex.expiry.expires_at, Some(PolicyInstant::new(5_000)));
    assert!(!ex.expiry.revoked);
    assert!(ex.expiry.live);
    assert!(ex.epoch_ceiling);
    assert_eq!(ex.budget.apply_budget, Some(2));
    assert_eq!(ex.budget.applies_consumed, 0);
    assert!(ex.notes.iter().any(|n| n.contains("epoch ceiling")));

    // Past expiry the explanation reports not-live honestly.
    fix.clock.set(PolicyInstant::new(9_999));
    let ex = fix.ui.grant_explanation(&grant_ref("g-limits")).unwrap();
    assert!(!ex.expiry.live);
    let row = fix
        .ui
        .grant_rows()
        .into_iter()
        .find(|r| r.grant == grant_ref("g-limits"))
        .unwrap();
    assert!(!row.live);
}

// ══════════════════════════════════════════════════════════════
// UX-03 / SYS-03 — the pinned approval surface
// ══════════════════════════════════════════════════════════════

#[test]
fn ux03_approval_surface_presents_everything() {
    let mut fix = boot(None);
    issue_agent(
        &mut fix,
        "t-approve",
        &mute_grant("g-approve", "t-approve", ApprovalPolicy::ApproveToApply),
    );
    let (proposal, digest) = prepare_mute_agent(&mut fix.ui, "t-approve", "g-approve");
    fix.ui.note_proposal(proposal_ref(&proposal));

    let id = fix
        .ui
        .open_approval(&proposal_ref(&proposal), &digest)
        .unwrap();
    let s = fix.ui.surface(id).unwrap();
    assert_eq!(s.proposal().as_str(), proposal);
    assert_eq!(s.digest().as_str(), digest);
    assert_eq!(s.base_revision(), ProjectRevision(1));
    assert_eq!(s.project(), project());
    assert_eq!(s.owner().id(), "mcp-client-1");
    assert_eq!(s.status(), Some(ProposalStatus::Prepared));
    assert_eq!(s.grant(), Some(&grant_ref("g-approve")));
    assert_eq!(s.task(), Some(&task_ref("t-approve")));
    assert!(s.grant_explanation().is_some());
    assert!(s.diff_complete());
    assert!(s.expires_at() > PolicyInstant::new(1_000));
    assert!(matches!(s.decision(), SurfaceDecision::Pending));

    // The mute shows as a normalized command + a write effect row.
    let cmd = &s.commands()[0];
    assert_eq!(cmd.capability, "synvelis.clip.mute");
    assert_eq!(cmd.target, tagged(&clip11()));
    let write = s
        .effects()
        .iter()
        .find(|e| e.kind == EffectKind::Write)
        .unwrap();
    assert_eq!(write.resource, tagged(&clip11()));
    assert_eq!(write.change, "modified");
    assert!(write.before.is_some());
}

#[test]
fn ux03_linked_source_and_note_diffs() {
    let mut fix = boot(None);
    issue_agent(
        &mut fix,
        "t-notes",
        &note_grant("g-notes", "t-notes", ApprovalPolicy::ApproveToApply),
    );
    let (proposal, digest) = prepare_note_edit_agent(&mut fix.ui, "t-notes", "g-notes");
    fix.ui.note_proposal(proposal_ref(&proposal));

    let id = fix
        .ui
        .open_approval(&proposal_ref(&proposal), &digest)
        .unwrap();
    let s = fix.ui.surface(id).unwrap();

    // The linked-source effect names the shared source and every clip
    // it lands on — clip13 never appears in the requested target.
    let linked = s
        .linked_sources()
        .iter()
        .find(|l| l.source == tagged(&source10()))
        .expect("linked source row");
    assert!(linked.clips.contains(&tagged(&clip12())));
    assert!(linked.clips.contains(&tagged(&clip13())));

    // The note diff recovered an added row on source-10.
    assert!(
        s.midi_notes()
            .iter()
            .any(|n| n.source == tagged(&source10()) && n.change == "added")
    );
    assert!(s.diff_complete());
}

#[test]
fn sys03_wrong_digest_cannot_open_surface() {
    let mut fix = boot(None);
    issue_agent(
        &mut fix,
        "t-digest",
        &mute_grant("g-digest", "t-digest", ApprovalPolicy::DelegatedApply),
    );
    let (proposal, _digest) = prepare_mute_agent(&mut fix.ui, "t-digest", "g-digest");
    let err = fix
        .ui
        .open_approval(&proposal_ref(&proposal), "sha256:forged")
        .unwrap_err();
    assert!(matches!(err, GatewayUiError::DigestMismatch { .. }));
    // No surface was minted.
    assert!(fix.ui.surface(1).is_none());
}

#[test]
fn sys03_selection_window_proposal_swap_cannot_retarget() {
    let mut fix = boot(None);
    issue_agent(
        &mut fix,
        "t-swap",
        &mute_grant("g-swap", "t-swap", ApprovalPolicy::ApproveToApply),
    );
    let (p1, d1) = prepare_mute_agent(&mut fix.ui, "t-swap", "g-swap");
    fix.ui.note_proposal(proposal_ref(&p1));
    let surface = fix.ui.open_approval(&proposal_ref(&p1), &d1).unwrap();

    // A second proposal (same op) is prepared — the surface stays
    // pinned to p1's digest.
    let (_p2, _d2) = prepare_mute_agent(&mut fix.ui, "t-swap", "g-swap");
    assert_eq!(fix.ui.surface(surface).unwrap().proposal().as_str(), p1);

    // Simulate selection + active-window churn mid-review — the
    // approval still lands on p1, never the newer proposal.
    let ws_rev = fix.ws.session().revision();
    fix.ws
        .session_mut()
        .apply(
            ProjectOp::SetProjectName {
                name: "other-window".into(),
            },
            ws_rev,
        )
        .unwrap();
    let decision = fix.ui.approve(surface).unwrap();
    let SurfaceDecision::Granted(record) = decision else {
        panic!("expected Granted, got {decision:?}");
    };
    assert!(record.approval.as_str().starts_with("approval-"));
    let view = fix.proposals.view(&proposal_ref(&p1)).unwrap();
    assert_eq!(view.approval.unwrap().status, ApprovalStatus::Granted);
}

// ══════════════════════════════════════════════════════════════
// UX-04 / SYS-02 / SYS-05 — rows + revocation axis
// ══════════════════════════════════════════════════════════════

#[test]
fn ux04_explicit_id_connection_task_grant_rows() {
    let mut fix = boot(None);
    // Connection permit — explicit ids only; secret issued once.
    let issue = fix
        .ui
        .issue_connection(PermitSpec {
            channel: McpChannel::Client,
            principal: agent_principal(),
            project: project(),
            task: Some(task_ref("t-conn")),
            grant: Some(grant_ref("g-conn")),
            ttl_ms: Some(60_000),
        })
        .unwrap();
    assert!(issue.secret.starts_with("syv-"));
    let row = &fix.ui.connection_rows()[0];
    assert_eq!(row.permit, issue.record.permit);
    assert_eq!(row.channel, McpChannel::Client);
    assert_eq!(row.kind, PrincipalKind::Agent);
    assert_eq!(row.task, Some(task_ref("t-conn")));
    assert_eq!(row.grant, Some(grant_ref("g-conn")));
    assert!(row.live);
    assert_eq!(row.revocation, Revocation::None);

    // A channel/kind mismatch refuses — a control permit may not bind
    // an agent principal.
    assert!(
        fix.ui
            .issue_connection(PermitSpec {
                channel: McpChannel::Control,
                principal: agent_principal(),
                project: project(),
                task: None,
                grant: None,
                ttl_ms: None,
            })
            .is_err()
    );
    assert_eq!(fix.ui.connection_rows().len(), 1);

    // Task + grant rows carry their explicit ids.
    issue_agent(
        &mut fix,
        "t-conn",
        &mute_grant("g-conn", "t-conn", ApprovalPolicy::DelegatedApply),
    );
    assert_eq!(fix.ui.task_rows()[0].task, task_ref("t-conn"));
    assert_eq!(fix.ui.task_rows()[0].activation, "user-issued");
    assert_eq!(fix.ui.grant_rows()[0].grant, grant_ref("g-conn"));

    // Revoke the connection — intent then confirmation.
    let report = fix.ui.revoke_connection(&issue.record.permit).unwrap();
    assert_eq!(report.state, Revocation::Confirmed);
    assert_eq!(report.cancel_requests, [] as [JobRef; 0]);
    let row = &fix.ui.connection_rows()[0];
    assert_eq!(row.revocation, Revocation::Confirmed);
    assert!(row.revoked);
    assert!(!row.live);
}

#[test]
fn sys02_sys05_revoke_is_intent_then_confirmation() {
    let mut fix = boot(None);
    issue_agent(
        &mut fix,
        "t-revoke",
        &mute_grant("g-revoke", "t-revoke", ApprovalPolicy::DelegatedApply),
    );
    issue_agent(
        &mut fix,
        "t-other",
        &mute_grant("g-other", "t-other", ApprovalPolicy::DelegatedApply),
    );

    // A scripted admin failure leaves the axis Requested — honest,
    // never assumed confirmed.
    fix.ports.fail_next_revoke_grants(1);
    let err = fix.ui.revoke_grant(&grant_ref("g-revoke")).unwrap_err();
    assert!(matches!(err, GatewayUiError::Admin(_)));
    let row = fix
        .ui
        .grant_rows()
        .into_iter()
        .find(|r| r.grant == grant_ref("g-revoke"))
        .unwrap();
    assert_eq!(row.revocation, Revocation::Requested);
    // The authority still reports live — the UI does not lie.
    assert!(!row.revoked);
    assert!(row.live);

    // Retry confirms — and the *other* grant is untouched (SYS-02:
    // the revoked target is the explicit id, not a selection).
    let report = fix.ui.revoke_grant(&grant_ref("g-revoke")).unwrap();
    assert_eq!(report.state, Revocation::Confirmed);
    let row = fix
        .ui
        .grant_rows()
        .into_iter()
        .find(|r| r.grant == grant_ref("g-revoke"))
        .unwrap();
    assert_eq!(row.revocation, Revocation::Confirmed);
    assert!(row.revoked);
    let other = fix
        .ui
        .grant_rows()
        .into_iter()
        .find(|r| r.grant == grant_ref("g-other"))
        .unwrap();
    assert!(!other.revoked);
    assert_eq!(other.revocation, Revocation::None);
}

#[test]
fn ux04_sys05_grant_revoke_requests_job_cancel_not_cancellation() {
    let mut fix = boot(Some(CancelMode::Pending));
    fix.ui.issue_task(&task_of("t-job")).unwrap();
    fix.ui.issue_grant(&job_grant("g-job", "t-job")).unwrap();

    // Submit a generation job under the grant; track its operation.
    let outcome = fix
        .ui
        .gateway_mut()
        .handle(&submit_req("t-job", "g-job", &[&clip11()]), agent())
        .unwrap();
    let operation = accepted(&outcome);
    fix.ui.note_job(operation);

    // Revoking the grant sends cancel *requests* — the job row keeps
    // phase (Running) and cancel (Requested) as distinct axes, even
    // though the provider never confirms.
    let report = fix.ui.revoke_grant(&grant_ref("g-job")).unwrap();
    assert_eq!(report.state, Revocation::Confirmed);
    assert_eq!(report.cancel_requests.len(), 1);
    let row = fix.ui.job_rows().into_iter().next().unwrap();
    assert_eq!(row.operation, operation);
    assert_eq!(row.cancel, CancelLink::Requested);
    assert_ne!(row.phase, JobPhase::Cancelled);
    assert_ne!(row.phase, JobPhase::Completed);
}

// ══════════════════════════════════════════════════════════════
// UX-05 / SYS-06 — the activity feed's distinct axes
// ══════════════════════════════════════════════════════════════

#[test]
fn ux05_sys06_commit_engine_saved_axes_stay_distinct() {
    let mut fix = boot(None);
    issue_agent(
        &mut fix,
        "t-axes",
        &mute_grant("g-axes", "t-axes", ApprovalPolicy::DelegatedApply),
    );
    let (proposal, digest) = prepare_mute_agent(&mut fix.ui, "t-axes", "g-axes");
    fix.ui.note_proposal(proposal_ref(&proposal));
    let surface = fix
        .ui
        .open_approval(&proposal_ref(&proposal), &digest)
        .unwrap();

    // Human apply — trusted path, no receipt needed.
    let report = fix.ui.apply(surface).unwrap();
    let CommitVerdict::Committed { revision, .. } = report.verdict else {
        panic!("expected Committed, got {:?}", report.verdict);
    };
    assert_eq!(revision, 2);

    let rows = fix.ui.operation_rows();
    let row = rows
        .iter()
        .find(|r| r.proposal.as_str() == proposal)
        .unwrap();
    assert_eq!(row.operation, Some(report.operation));
    assert!(matches!(row.commit, Some(CommitVerdict::Committed { .. })));
    // Engine application is a separate axis — nothing reported yet.
    assert_eq!(row.engine, Some(EngineApplyState::Pending));
    // And committed ≠ saved: no publication yet.
    assert!(row.saved.is_none());

    // Engine reports its own axis — commit stays Committed.
    fix.ui.note_engine(
        report.operation,
        EngineApplyState::Applied { at_revision: 2 },
    );
    let row = fix.ui.operation_rows()[0].clone();
    assert!(matches!(
        row.engine,
        Some(EngineApplyState::Applied { at_revision: 2 })
    ));
    assert!(matches!(row.commit, Some(CommitVerdict::Committed { .. })));

    // A save publishes HEAD — the saved axis now appears, still not
    // conflated with commit or engine.
    let saved = fix
        .gate
        .save(fix.ui.caller(), &mut fix.project_store)
        .unwrap();
    fix.ui.note_saved(SaveRecord {
        project: project(),
        revision: saved.revision,
        head: saved.head.clone(),
    });
    let row = fix.ui.operation_rows()[0].clone();
    let saved_row = row.saved.expect("publication row");
    assert_eq!(saved_row.revision.0, saved.revision.0);
    assert!(matches!(
        row.engine,
        Some(EngineApplyState::Applied { at_revision: 2 })
    ));
}

#[test]
fn sys06_conflicted_apply_is_not_a_success() {
    let mut fix = boot(None);
    issue_agent(
        &mut fix,
        "t-conflict",
        &mute_grant("g-conflict", "t-conflict", ApprovalPolicy::DelegatedApply),
    );
    let (proposal, digest) = prepare_mute_agent(&mut fix.ui, "t-conflict", "g-conflict");
    fix.ui.note_proposal(proposal_ref(&proposal));
    let surface = fix
        .ui
        .open_approval(&proposal_ref(&proposal), &digest)
        .unwrap();

    // Another commit lands first — the surface's asserted base is
    // now stale; the apply must surface a non-success, not commit.
    let (p2, d2) = prepare_mute_agent(&mut fix.ui, "t-conflict", "g-conflict");
    fix.ui.note_proposal(proposal_ref(&p2));
    let s2 = fix.ui.open_approval(&proposal_ref(&p2), &d2).unwrap();
    fix.ui.apply(s2).unwrap(); // commits rev 2

    let err = fix.ui.apply(surface).unwrap_err();
    let GatewayUiError::Gateway(env) = err else {
        panic!("expected a typed refusal, got {err:?}");
    };
    assert!(matches!(
        env.code,
        GatewayError::RevisionConflict {
            expected: 1,
            actual: 2
        }
    ));
    // The refused apply left the proposal prepared and the first
    // commit standing — a conflict is not a silent failure.
    let view = fix.proposals.view(&proposal_ref(&proposal)).unwrap();
    assert_eq!(view.status, ProposalStatus::Prepared);
    assert_eq!(view.apply.unwrap().status, ApplyStatus::Conflict);
    assert_eq!(fix.gate.live_revision(), ProjectRevision(2));
}

// ══════════════════════════════════════════════════════════════
// SYS-04 — stale proposal / grant
// ══════════════════════════════════════════════════════════════

#[test]
fn sys04_expired_proposal_refuses_approve() {
    let mut fix = boot(None);
    issue_agent(
        &mut fix,
        "t-expired",
        &mute_grant("g-expired", "t-expired", ApprovalPolicy::ApproveToApply),
    );
    let (proposal, digest) = prepare_mute_agent(&mut fix.ui, "t-expired", "g-expired");
    fix.ui.note_proposal(proposal_ref(&proposal));
    let surface = fix
        .ui
        .open_approval(&proposal_ref(&proposal), &digest)
        .unwrap();

    // The proposal expires while the card is open — approving the
    // stale card refuses and records it; no commit occurs.
    fix.clock.set(PolicyInstant::new(1_000 + 60_000 + 1));
    let err = fix.ui.approve(surface).unwrap_err();
    assert!(matches!(err, GatewayUiError::Gateway(_)));
    fix.ui.refresh_surface(surface).unwrap();
    let s = fix.ui.surface(surface).unwrap();
    assert_eq!(s.status(), Some(ProposalStatus::Expired));
    assert!(matches!(s.decision(), SurfaceDecision::Refused { .. }));
    assert_eq!(fix.gate.live_revision(), ProjectRevision(1));
}

// ══════════════════════════════════════════════════════════════
// the three application modes (SYS-04 derivation + flows)
// ══════════════════════════════════════════════════════════════

#[test]
fn mode_suggest_denies_agent_apply() {
    let mut fix = boot(None);
    issue_agent(
        &mut fix,
        "t-suggest",
        &mute_grant("g-suggest", "t-suggest", ApprovalPolicy::Suggest),
    );
    let (proposal, digest) = prepare_mute_agent(&mut fix.ui, "t-suggest", "g-suggest");
    fix.ui.note_proposal(proposal_ref(&proposal));

    // The agent cannot apply — Suggest is prepare/preview only.
    let outcome = fix.ui.gateway_mut().handle(
        &apply_req(
            &proposal,
            &digest,
            None,
            Some("t-suggest"),
            Some("g-suggest"),
            1,
        ),
        agent(),
    );
    assert!(outcome.is_err(), "suggest apply must deny: {outcome:?}");
    assert_eq!(fix.gate.live_revision(), ProjectRevision(1));
}

#[test]
fn mode_delegated_apply_needs_no_receipt() {
    let mut fix = boot(None);
    issue_agent(
        &mut fix,
        "t-delegated",
        &mute_grant("g-delegated", "t-delegated", ApprovalPolicy::DelegatedApply),
    );
    let (proposal, digest) = prepare_mute_agent(&mut fix.ui, "t-delegated", "g-delegated");
    fix.ui.note_proposal(proposal_ref(&proposal));

    // The agent applies directly — no approval request, no receipt.
    let outcome = fix
        .ui
        .gateway_mut()
        .handle(
            &apply_req(
                &proposal,
                &digest,
                None,
                Some("t-delegated"),
                Some("g-delegated"),
                1,
            ),
            agent(),
        )
        .unwrap();
    accepted(&outcome);
    let receipt = fix
        .ports
        .commit(&proposal_ref(&proposal))
        .expect("commit lands");
    assert_eq!(receipt.revision, ProjectRevision(2));

    let view = fix.proposals.view(&proposal_ref(&proposal)).unwrap();
    assert!(view.approval.is_none());
    let row = fix
        .ui
        .operation_rows()
        .into_iter()
        .find(|r| r.proposal.as_str() == proposal)
        .unwrap();
    assert!(matches!(row.commit, Some(CommitVerdict::Committed { .. })));
}

#[test]
fn mode_approve_to_apply_full_flow() {
    let mut fix = boot(None);
    issue_agent(
        &mut fix,
        "t-a2a",
        &mute_grant("g-a2a", "t-a2a", ApprovalPolicy::ApproveToApply),
    );
    let (proposal, digest) = prepare_mute_agent(&mut fix.ui, "t-a2a", "g-a2a");
    fix.ui.note_proposal(proposal_ref(&proposal));

    // Step 1 — the agent's apply waits on approval.
    let outcome = fix
        .ui
        .gateway_mut()
        .handle(
            &apply_req(&proposal, &digest, None, Some("t-a2a"), Some("g-a2a"), 1),
            agent(),
        )
        .unwrap();
    let GatewayOutcome::ApprovalRequired { approval } = outcome else {
        panic!("expected ApprovalRequired, got {outcome:?}");
    };
    assert!(approval.as_str().starts_with("approval-"));

    // Step 2 — the human reviews the pinned surface and approves.
    let surface = fix
        .ui
        .open_approval(&proposal_ref(&proposal), &digest)
        .unwrap();
    let decision = fix.ui.approve(surface).unwrap();
    let SurfaceDecision::Granted(record) = decision else {
        panic!("expected Granted, got {decision:?}");
    };
    let receipt = record.approval.as_str().to_owned();

    // Step 3 — the agent retries with the exact receipt: Accepted,
    // then the commit boundary settles it.
    let outcome = fix
        .ui
        .gateway_mut()
        .handle(
            &apply_req(
                &proposal,
                &digest,
                Some(&receipt),
                Some("t-a2a"),
                Some("g-a2a"),
                1,
            ),
            agent(),
        )
        .unwrap();
    let operation = accepted(&outcome);
    fix.ports
        .commit(&proposal_ref(&proposal))
        .expect("commit lands");

    // Every axis of the row reflects the settled lifecycle.
    let row = fix
        .ui
        .operation_rows()
        .into_iter()
        .find(|r| r.proposal.as_str() == proposal)
        .unwrap();
    assert_eq!(row.approval, Some(ApprovalStatus::Consumed));
    assert_eq!(row.operation, Some(operation));
    assert!(matches!(row.commit, Some(CommitVerdict::Committed { .. })));
}

#[test]
fn mis_approval_revoked_grant_denies_apply_with_valid_receipt() {
    let mut fix = boot(None);
    issue_agent(
        &mut fix,
        "t-mis",
        &mute_grant("g-mis", "t-mis", ApprovalPolicy::ApproveToApply),
    );
    let (proposal, digest) = prepare_mute_agent(&mut fix.ui, "t-mis", "g-mis");
    fix.ui.note_proposal(proposal_ref(&proposal));
    let surface = fix
        .ui
        .open_approval(&proposal_ref(&proposal), &digest)
        .unwrap();
    let decision = fix.ui.approve(surface).unwrap();
    let SurfaceDecision::Granted(record) = decision else {
        panic!("expected Granted");
    };
    let receipt = record.approval.as_str().to_owned();

    // The grant is revoked *after* the receipt is minted — the apply
    // denies before the receipt is consulted (SEC-12 order).
    fix.ui.revoke_grant(&grant_ref("g-mis")).unwrap();
    let outcome = fix.ui.gateway_mut().handle(
        &apply_req(
            &proposal,
            &digest,
            Some(&receipt),
            Some("t-mis"),
            Some("g-mis"),
            1,
        ),
        agent(),
    );
    assert!(outcome.is_err(), "revoked grant must deny: {outcome:?}");
    assert_eq!(fix.gate.live_revision(), ProjectRevision(1));
}

#[test]
fn decline_records_terminal_decision() {
    let mut fix = boot(None);
    issue_agent(
        &mut fix,
        "t-decline",
        &mute_grant("g-decline", "t-decline", ApprovalPolicy::ApproveToApply),
    );
    let (proposal, digest) = prepare_mute_agent(&mut fix.ui, "t-decline", "g-decline");
    // An apply attempt first so an approval request exists to decline.
    fix.ui
        .gateway_mut()
        .handle(
            &apply_req(
                &proposal,
                &digest,
                None,
                Some("t-decline"),
                Some("g-decline"),
                1,
            ),
            agent(),
        )
        .unwrap();
    fix.ui.note_proposal(proposal_ref(&proposal));
    let surface = fix
        .ui
        .open_approval(&proposal_ref(&proposal), &digest)
        .unwrap();

    let decision = fix.ui.decline(surface).unwrap();
    assert!(matches!(decision, SurfaceDecision::Declined));
    let view = fix.proposals.view(&proposal_ref(&proposal)).unwrap();
    assert_eq!(view.approval.unwrap().status, ApprovalStatus::Declined);

    // A declined approval can never drive an apply.
    let outcome = fix.ui.gateway_mut().handle(
        &apply_req(
            &proposal,
            &digest,
            None,
            Some("t-decline"),
            Some("g-decline"),
            1,
        ),
        agent(),
    );
    assert!(outcome.is_err());
}
