//! RCP-06 headless contract tests — Missing Plugin / render
//! availability presentation model, Branch Difference Explorer,
//! Render Policy projection, request preview, and job-control
//! vocabulary (synvelis#295;
//! `docs/test-specs/rcp-06-render-gui.md`).
//!
//! These tests exercise the *projection* layer only: every fixture is
//! pinned authority evidence (`PluginVerdict`, `ArtifactEligibility`,
//! `LookupOutcome`, `EffectivePolicy`, `JobRecord`, …) projected into
//! display models. They never re-derive eligibility, never fabricate
//! grants, and never touch GPUI/OS — real-GUI/real-OS gates are
//! separate (RCP-UX-06, `NotRun` here by design).

use std::collections::{BTreeMap, BTreeSet};

use synvelis_player::rcp::{
    ArtifactEligibility, ExecutionIntent, ExecutionSelection, LiveRegion, PluginVerdict, RcpNodeId,
    ReconciliationReceipt, ReproducibilityClass, SelectionVerdict, Substitution, UnresolvedNode,
    UnresolvedReason,
};
use synvelis_project::id::ProjectId;
use synvelis_project::project::Project;
use synvelis_project::routing_build::{
    BuildMode, RoutingBuildKey, RoutingFingerprint, RuntimeClockEpochRef, RuntimeInstanceId,
};
use synvelis_project::snapshot::{
    ContentDigest, FixedSnapshotLocator, SnapshotIdentity, SnapshotOrigin,
};
use synvelis_project::store::SCHEMA_VERSION;
use synvelis_project::{ProjectRevision, SemanticPortId};
use synvelis_render::artifact::lookup::LookupOutcome;
use synvelis_render::artifact::manifest::RenderArtifactId;
use synvelis_render::policy::{
    CommitCapability, DemandProposal, TriggerEvent, resolve_effective_policy,
};
use synvelis_render::recipe::{
    DenormalMode, FrameInterval, RenderBoundary, RenderPrecision, RendererProfile,
};
use synvelis_render::render_job::{
    BoundaryResolution, CompletenessGap, CompletenessGapKind, CompletenessReport,
    CompletenessVerdict, EntrySurface, GatewayActionRef, GrantRef, JobCostEstimate, JobFailure,
    JobFailureReason, JobPurpose, JobReceipt, JobRecord, PinnedSnapshot, ReceiptKind, RenderDemand,
    RenderJobKey, RenderJobState, RenderSelection,
};
use synvelis_workspace::*;

// ---------------------------------------------------------------------------
// fixtures
// ---------------------------------------------------------------------------

fn project() -> Project {
    Project::new(ProjectId::new(0x295), "rcp06")
}

fn digest(byte: u8) -> ContentDigest {
    ContentDigest::sha256(&[byte; 32])
}

fn pinned(head: &str) -> PinnedSnapshot {
    let project = project();
    PinnedSnapshot {
        identity: SnapshotIdentity {
            project_id: project.project_id,
            source: SnapshotOrigin::Saved,
            locator: FixedSnapshotLocator::Saved {
                project_id: project.project_id,
                head: head.to_owned(),
            },
            digest: digest(0xA1),
            schema_version: SCHEMA_VERSION,
            projection_version: SCHEMA_VERSION,
            capture_generation: 1,
        },
        project,
    }
}

fn demand() -> RenderDemand {
    RenderDemand {
        boundary: RenderBoundary::new(vec![SemanticPortId::new(1)]),
        interval: FrameInterval::new(0, 480).expect("interval"),
    }
}

fn selection(head: &str) -> RenderSelection {
    RenderSelection {
        snapshot: pinned(head),
        demands: vec![demand()],
    }
}

fn build_key() -> RoutingBuildKey {
    RoutingBuildKey {
        project_id: ProjectId::new(0x295),
        project_revision: ProjectRevision(7),
        routing_fingerprint: RoutingFingerprint(42),
        capability_generation: 3,
        engine_config_generation: 1,
        source_format_generation: 2,
        lowering_policy_version: 1,
        build_mode: BuildMode::Offline,
        device_clock_epoch: RuntimeClockEpochRef(9),
        runtime_instance: RuntimeInstanceId(4),
    }
}

fn receipt() -> ReconciliationReceipt {
    ReconciliationReceipt {
        snapshot_id: 11,
        project_revision: ProjectRevision(7),
        structure_digest: 0x5EED,
        capability_generation: 3,
        render_index_generation: 8,
        plugin_verdicts: BTreeMap::new(),
        candidate_attempts: Vec::new(),
        build_key: build_key(),
        selection_digest: 0xBEEF,
    }
}

fn exec_selection(
    verdict: SelectionVerdict,
    unresolved: Vec<UnresolvedNode>,
) -> ExecutionSelection {
    ExecutionSelection {
        intent: ExecutionIntent::MainPlayback,
        live_regions: vec![LiveRegion {
            nodes: BTreeSet::from([RcpNodeId(1)]),
        }],
        substitutions: Vec::new(),
        skipped: Vec::new(),
        unresolved,
        degraded: Vec::new(),
        non_equivalent: verdict == SelectionVerdict::NonEquivalent,
        verdict,
        receipt: receipt(),
    }
}

fn missing_node() -> RenderNodePresentation {
    let verdict = PluginVerdict::Missing;
    let unresolved = UnresolvedNode {
        node: RcpNodeId(1),
        reason: UnresolvedReason::PluginVerdict(PluginVerdict::Missing),
    };
    project_render_node(
        RcpNodeId(1),
        NodeRenderEvidence {
            verdict: Some(&verdict),
            unresolved: Some(&unresolved),
            ..NodeRenderEvidence::default()
        },
        &ExecutionIntent::MainPlayback,
    )
}

fn renderer_profile() -> RendererProfile {
    RendererProfile {
        renderer_id: "test-render".into(),
        renderer_version: "0".into(),
        compatibility_profile: digest(0xC3),
        precision: RenderPrecision::F32,
        backend: "offline".into(),
        seed: 0,
        denormal_mode: DenormalMode::Preserve,
    }
}

fn job_key() -> RenderJobKey {
    RenderJobKey {
        snapshot_locator: "saved|p|head".to_string(),
        snapshot_digest: digest(0xD4),
        recipe_keys: Vec::new(),
        purpose: JobPurpose::Selection,
        policy_resolution: digest(0xE5),
        action: "synvelis.render.request".to_string(),
        grant: None,
    }
}

fn job_record(state: RenderJobState, failure: Option<JobFailure>) -> JobRecord {
    JobRecord {
        key: job_key(),
        state,
        attempt: 1,
        pending_reason: None,
        cancel: ruviform_exec::CancellationToken::new(),
        cancel_requested: false,
        crashed: false,
        policy: resolve_effective_policy(None, None, None),
        renderer: renderer_profile(),
        selection: selection("head-a"),
        boundaries: Vec::new(),
        exec: Vec::new(),
        artifacts: BTreeSet::new(),
        external_manifests: Vec::new(),
        triggers: Vec::new(),
        entry: EntrySurface::Gui,
        action: GatewayActionRef("synvelis.render.request".to_string()),
        grant: None,
        saved_revision: None,
        estimate: JobCostEstimate::default(),
        failure,
        completeness: None,
        created_tick: 1,
        updated_tick: 2,
    }
}

fn executable_ids(candidates: &[RenderActionCandidate]) -> BTreeSet<&'static str> {
    candidates
        .iter()
        .filter(|c| c.executable)
        .map(|c| c.id)
        .collect()
}

// ---------------------------------------------------------------------------
// RCP-UX-01 — five-axis separation: edit possible vs. execution blocked
// ---------------------------------------------------------------------------

#[test]
fn missing_plugin_keeps_edit_possible_but_blocks_execution() {
    let node = missing_node();
    // Semantic editability survives a missing plugin — the project is
    // canonical (RCP-00 invariant).
    assert_eq!(node.editability, Editability::Editable);
    assert_eq!(node.editability.label(), "edit-possible");
    // Live execution fails closed with the precise reason — never a
    // generic "missing plugin".
    assert_eq!(
        node.executability,
        Executability::Blocked {
            reason: RenderReason::PluginMissing,
        }
    );
    assert!(node.reasons.contains(&RenderReason::PluginMissing));
    assert_eq!(node.render_validity, RenderValidity::Missing);
    assert_eq!(node.playback_quality, PlaybackQuality::Unavailable);
    // Meaning survives without color: labels + reason codes.
    assert_eq!(node.render_validity.label(), "render-missing");
    assert_eq!(node.playback_quality.label(), "quality-unavailable");
}

#[test]
fn automation_identity_incomplete_blocks_editing_too() {
    let verdict = PluginVerdict::AutomationIdentityIncomplete;
    let node = project_render_node(
        RcpNodeId(2),
        NodeRenderEvidence {
            verdict: Some(&verdict),
            ..NodeRenderEvidence::default()
        },
        &ExecutionIntent::MainPlayback,
    );
    assert_eq!(
        node.editability,
        Editability::Blocked {
            reason: RenderReason::PluginAutomationIncomplete,
        }
    );
    assert_eq!(node.editability.label(), "edit-blocked");
}

#[test]
fn live_plugin_with_exact_artifact_is_fully_available() {
    let verdict = PluginVerdict::Live;
    let node = project_render_node(
        RcpNodeId(3),
        NodeRenderEvidence {
            verdict: Some(&verdict),
            eligibility: Some(ArtifactEligibility::EligibleExact),
            reproducibility: Some(ReproducibilityClass::BitExactCertified),
            in_live_region: true,
            ..NodeRenderEvidence::default()
        },
        &ExecutionIntent::MainPlayback,
    );
    assert_eq!(node.editability, Editability::Editable);
    assert_eq!(node.executability, Executability::Live);
    assert_eq!(node.render_validity, RenderValidity::Valid);
    assert_eq!(node.playback_quality, PlaybackQuality::ExactArtifactReplay);
    assert_eq!(node.runtime_state, RuntimeState::Active);
}

#[test]
fn preview_class_substitution_is_never_exact() {
    let substitution = Substitution {
        nodes: BTreeSet::from([RcpNodeId(4)]),
        artifact_id: "art-1".to_string(),
        content_hash: format!("sha256:{}", "ab".repeat(32)),
        preview_class: true,
    };
    let node = project_render_node(
        RcpNodeId(4),
        NodeRenderEvidence {
            substitution: Some(&substitution),
            eligibility: Some(ArtifactEligibility::EligiblePreview),
            reproducibility: Some(ReproducibilityClass::PreviewOnly),
            ..NodeRenderEvidence::default()
        },
        &ExecutionIntent::EditingPreview {
            live_regions: vec![],
            allow_non_equivalent: true,
        },
    );
    // Substitution makes the node executable *via artifact* — but the
    // preview class stays visible on the quality axis.
    assert_eq!(node.executability, Executability::Live);
    assert_eq!(node.render_validity, RenderValidity::Preview);
    assert_eq!(node.playback_quality, PlaybackQuality::Preview);
    assert_eq!(node.playback_quality.label(), "quality-preview");
    let sub = node.substitution.expect("substitution overlay");
    assert!(sub.preview_class);
    assert!(sub.content_hash.starts_with("sha256:"));
}

#[test]
fn last_good_mismatch_is_distinct_from_current_state() {
    let verdict = PluginVerdict::Missing;
    let node = project_render_node(
        RcpNodeId(5),
        NodeRenderEvidence {
            verdict: Some(&verdict),
            last_good_mismatch: true,
            ..NodeRenderEvidence::default()
        },
        &ExecutionIntent::MainPlayback,
    );
    assert_eq!(node.runtime_state, RuntimeState::LastGoodMismatch);
    assert_eq!(node.runtime_state.label(), "runtime-last-good-mismatch");
}

// ---------------------------------------------------------------------------
// RCP-UX-01/02 — lookup validity + selection pins/verdicts
// ---------------------------------------------------------------------------

#[test]
fn lookup_outcomes_map_to_distinct_validity_labels() {
    let artifact = RenderArtifactId(digest(0xF1));
    let cases: [(LookupOutcome, RenderValidity); 9] = [
        (
            LookupOutcome::EligibleExact { artifact },
            RenderValidity::Valid,
        ),
        (
            LookupOutcome::EligiblePreview { artifact },
            RenderValidity::Preview,
        ),
        (LookupOutcome::Stale, RenderValidity::Stale),
        (LookupOutcome::Unverified, RenderValidity::Unverified),
        (LookupOutcome::Corrupt, RenderValidity::Corrupt),
        (LookupOutcome::Missing, RenderValidity::Missing),
        (
            LookupOutcome::Denied {
                reason: "acl".into(),
            },
            RenderValidity::Denied,
        ),
        (
            LookupOutcome::Unknown {
                reason: "undecidable".into(),
            },
            RenderValidity::Unknown,
        ),
        (
            LookupOutcome::Unsupported {
                reason: "fmt".into(),
            },
            RenderValidity::Unsupported,
        ),
    ];
    for (outcome, expected) in cases {
        assert_eq!(render_validity_of_lookup(&outcome), expected, "{outcome:?}");
    }
}

#[test]
fn selection_projection_carries_pins_and_never_calls_preview_exact() {
    let unresolved = vec![UnresolvedNode {
        node: RcpNodeId(9),
        reason: UnresolvedReason::NoEligibleSubstitution,
    }];
    let sel = exec_selection(SelectionVerdict::NonEquivalent, unresolved);
    let view = project_selection(&sel);
    assert_eq!(view.verdict, "non-equivalent-preview");
    assert!(view.non_equivalent);
    assert_ne!(view.verdict, "exact");
    assert_eq!(view.unresolved, 1);
    assert!(
        view.unresolved_reasons
            .contains(&RenderReason::NoEligibleSubstitution)
    );
    // Snapshot pin — identical text contract on every surface.
    assert_eq!(view.pins.project_revision, ProjectRevision(7));
    assert_eq!(view.pins.capability_generation, 3);
    assert_eq!(view.pins.selection_digest, 0xBEEF);
}

#[test]
fn blocked_selection_reports_blocked_verdict_and_reasons() {
    let sel = exec_selection(SelectionVerdict::Blocked, Vec::new());
    let view = project_selection(&sel);
    assert_eq!(view.verdict, "blocked");
    assert!(!view.non_equivalent);
}

// ---------------------------------------------------------------------------
// RCP-UX-03 — policy projection: triggers, provider, provenance,
// recipe invariance
// ---------------------------------------------------------------------------

#[test]
fn policy_projection_shows_commit_unsupported_with_shared_reason() {
    let effective = resolve_effective_policy(None, None, None);
    let view = project_render_policy(
        &effective,
        &CommitCapability::Unsupported {
            reason: "no-vcs-provider".to_string(),
        },
    );
    // OnCommit row: enabled by default policy? — either way the
    // provider block carries the same reason code the CLI reports.
    let commit = view
        .triggers
        .iter()
        .find(|t| t.trigger == RenderTrigger::OnCommit)
        .expect("on-commit row");
    // A disabled trigger reports TriggerDisabled; the provider gap is
    // carried separately — same reason code the CLI reports.
    assert_eq!(commit.blocked_reason, Some(RenderReason::TriggerDisabled));
    match &commit.provider {
        Some(TriggerProvider::Unsupported { reason, .. }) => {
            assert_eq!(*reason, RenderReason::CommitUnsupported);
        }
        other => panic!("expected unsupported provider, got {other:?}"),
    }
    assert_eq!(
        RenderReason::CommitUnsupported.code(),
        "policy.commit-unsupported"
    );
    // Manual is unconditional; trigger-only edits never invalidate
    // recipe keys (RCP-POL-02).
    let manual = view
        .triggers
        .iter()
        .find(|t| t.trigger == RenderTrigger::Manual)
        .expect("manual row");
    assert!(manual.unconditional);
    assert!(manual.enabled);
    assert!(view.recipe_invariant);
    // Provenance is displayed per field — with no overrides every
    // field resolves to the project-declared default layer.
    assert!(
        view.provenance
            .iter()
            .any(|p| p.field == "triggers" && p.layer == "project-declared")
    );
}

#[test]
fn draft_trigger_edit_lowers_to_policy_patch_only() {
    let mut state = RenderViewState::default();
    assert_eq!(render_policy_draft_patch(&state), None);
    apply_render_view_action(
        &mut state,
        RenderViewAction::ToggleTrigger {
            trigger: RenderTrigger::OnSave,
            enabled: false,
        },
    );
    let patch = render_policy_draft_patch(&state).expect("draft patch");
    let triggers = patch.triggers.expect("triggers override");
    assert!(!triggers.contains(&RenderTrigger::OnSave));
    // Manual cannot be toggled off — it is unconditional.
    apply_render_view_action(
        &mut state,
        RenderViewAction::ToggleTrigger {
            trigger: RenderTrigger::Manual,
            enabled: false,
        },
    );
    let patch = render_policy_draft_patch(&state).expect("draft patch");
    assert!(
        patch
            .triggers
            .expect("triggers")
            .contains(&RenderTrigger::Manual)
    );
    apply_render_view_action(&mut state, RenderViewAction::ClearDraft);
    assert_eq!(render_policy_draft_patch(&state), None);
}

// ---------------------------------------------------------------------------
// RCP-UX-02/04 — request preview + shared action vocabulary + grant gate
// ---------------------------------------------------------------------------

#[test]
fn request_preview_gates_request_until_all_demands_resolve() {
    let sel = selection("head-a");
    // Unresolved boundary → request action disabled with shared code.
    let preview = project_render_request(
        &sel,
        &[BoundaryResolution::Unknown {
            demand: 0,
            dependencies: Vec::new(),
        }],
        &["plug-x".to_string()],
        Some(4096),
        false,
        Some(RenderReason::ArtifactMissing),
        None,
        false,
    );
    assert_eq!(preview.coverage, "0/1 demands resolved");
    assert_eq!(preview.missing_plugins, vec!["plug-x".to_string()]);
    assert_eq!(
        preview.invalidation.map(RenderReason::code),
        Some("render.artifact-missing")
    );
    let request = preview
        .actions
        .iter()
        .find(|a| a.id == ACTION_RENDER_REQUEST)
        .expect("request candidate");
    assert!(!request.executable);
    assert_eq!(
        request.disabled_reason,
        Some(RenderReason::SelectionBlocked)
    );
    // Publish/remote fail closed without a grant — same ids CLI uses.
    let publish = preview
        .actions
        .iter()
        .find(|a| a.id == ACTION_RENDER_PUBLISH)
        .expect("publish candidate");
    assert!(!publish.executable);
    assert_eq!(publish.disabled_reason, Some(RenderReason::GrantRequired));
    assert!(publish.requires_grant);
    // Plan/inspect are always readable.
    assert!(executable_ids(&preview.actions).contains(ACTION_RENDER_PLAN));
    assert!(executable_ids(&preview.actions).contains(ACTION_RENDER_INSPECT));
}

#[test]
fn render_request_report_is_the_single_lowering_and_fails_closed() {
    let sel = selection("head-a");
    let manual = TriggerEvent::Manual {
        selection: sel.clone(),
        purpose: JobPurpose::Selection,
        policy_intent: None,
    };
    // GUI entry surface + shared action id — no grant needed for a
    // plain request action (authorization is the gateway's job).
    let report = render_request_report(
        manual,
        EntrySurface::Gui,
        GatewayActionRef(ACTION_RENDER_REQUEST.to_string()),
        None,
    )
    .expect("manual request report");
    assert_eq!(report.entry, EntrySurface::Gui);
    assert_eq!(report.action.0, ACTION_RENDER_REQUEST);

    // OnDemand/demand events always require a grant.
    let demand_event = TriggerEvent::Demand(DemandProposal {
        proposal_id: "p1".to_string(),
        selection: sel,
        purpose: JobPurpose::Selection,
        rationale: "test".to_string(),
        policy_intent: None,
    });
    let err = render_request_report(
        demand_event,
        EntrySurface::Gui,
        GatewayActionRef(ACTION_RENDER_REQUEST.to_string()),
        None,
    )
    .expect_err("demand without grant must fail closed");
    assert_eq!(
        err,
        RenderUiError::GrantRequired {
            action: ACTION_RENDER_REQUEST.to_string(),
        }
    );

    // Publish action without grant — fail closed.
    let manual2 = TriggerEvent::Manual {
        selection: selection("head-a"),
        purpose: JobPurpose::Selection,
        policy_intent: None,
    };
    let err = render_request_report(
        manual2,
        EntrySurface::HeadlessCli,
        GatewayActionRef(ACTION_RENDER_PUBLISH.to_string()),
        None,
    )
    .expect_err("publish without grant must fail closed");
    assert!(matches!(err, RenderUiError::GrantRequired { .. }));

    // With a grant the same triple resolves — GUI/CLI share the key
    // material (entry is provenance only, excluded from coalescing).
    let grant = GrantRef("grant-1".to_string());
    let report_cli = render_request_report(
        TriggerEvent::Manual {
            selection: selection("head-a"),
            purpose: JobPurpose::Selection,
            policy_intent: None,
        },
        EntrySurface::HeadlessCli,
        GatewayActionRef(ACTION_RENDER_PUBLISH.to_string()),
        Some(grant.clone()),
    )
    .expect("publish with grant");
    let report_gui = render_request_report(
        TriggerEvent::Manual {
            selection: selection("head-a"),
            purpose: JobPurpose::Selection,
            policy_intent: None,
        },
        EntrySurface::Gui,
        GatewayActionRef(ACTION_RENDER_PUBLISH.to_string()),
        Some(grant),
    )
    .expect("publish with grant");
    // Same event/action/grant triple → identical job-key material;
    // only `entry` differs (provenance, never coalescing input).
    assert_eq!(report_cli.action, report_gui.action);
    assert_eq!(report_cli.grant, report_gui.grant);
    assert_ne!(report_cli.entry, report_gui.entry);
}

// ---------------------------------------------------------------------------
// RCP-UX-02 — Branch Difference Explorer projection
// ---------------------------------------------------------------------------

#[test]
fn branch_diff_shows_pins_boundaries_missing_plugins_and_semantics() {
    let left_pin = pinned("head-left").identity;
    let right_pin = pinned("head-right").identity;
    let left_demands = vec![demand()];
    let right_demands = vec![demand()];
    let left_res = vec![BoundaryResolution::Unknown {
        demand: 0,
        dependencies: vec![synvelis_render::recipe::UnknownDependency {
            cause: synvelis_render::recipe::UnknownCause::MissingEntity {
                locator: "plugin|x".to_string(),
            },
            locator: "plugin|x".to_string(),
        }],
    }];
    let right_res: Vec<BoundaryResolution> = Vec::new();
    let changed = vec![ChangedParameterRow {
        path: "boundary[0].gain".to_string(),
        left: "-6dB".to_string(),
        right: "0dB".to_string(),
    }];
    let missing = vec!["plug-x".to_string()];
    let semantic = vec![SemanticDiffRow {
        path: "plugin[plug-x].version".to_string(),
        left: "1.0".to_string(),
        right: "missing".to_string(),
        kind: SemanticChange::Removed,
    }];
    let diff = project_branch_diff(BranchDiffInput {
        left: BranchSideInput {
            pin: &left_pin,
            demands: &left_demands,
            resolutions: &left_res,
        },
        right: BranchSideInput {
            pin: &right_pin,
            demands: &right_demands,
            resolutions: &right_res,
        },
        context: "variant-a vs main".to_string(),
        changed_parameters: &changed,
        missing_plugins: &missing,
        semantic_diffs: &semantic,
        preview_markers: &[],
        warnings: &[],
    });
    // Identical pin text contract on both surfaces.
    assert!(diff.left_pin.pin_text().contains("head-left"));
    assert!(diff.right_pin.pin_text().contains("head-right"));
    assert_eq!(diff.context, "variant-a vs main");
    assert_eq!(diff.rows.len(), 1);
    let row = &diff.rows[0];
    assert_eq!(row.coverage, "0/2 resolved");
    assert_eq!(row.changed_parameters.len(), 1);
    assert_eq!(row.missing_plugins, missing);
    // Unknown-side reasons carry the shared render vocabulary.
    assert!(
        row.left
            .reasons
            .iter()
            .any(|r| r.code().starts_with("render."))
    );
    // Right side had a demand but no resolution → unknown + reason.
    assert_eq!(row.right.resolution, BoundaryResolutionKind::Unknown);
    assert!(row.right.reasons.contains(&RenderReason::ArtifactUnknown));
    assert_eq!(diff.semantic_diffs.len(), 1);
    assert_eq!(diff.missing_plugins, missing);
}

// ---------------------------------------------------------------------------
// RCP-UX-05 — job candidates + idempotent control vocabulary
// ---------------------------------------------------------------------------

#[test]
fn job_candidates_gate_cancel_retry_attach_by_state() {
    // Active job: cancel executable, retry/attach not.
    let active = job_record(RenderJobState::Rendering, None);
    let ids = executable_ids(&render_job_candidates(&active));
    assert!(ids.contains(ACTION_RENDER_CANCEL));
    assert!(!ids.contains(ACTION_RENDER_RETRY));
    assert!(!ids.contains(ACTION_RENDER_ATTACH));
    assert!(job_control_allowed(JobControlOp::Cancel, &active));
    assert!(!job_control_allowed(JobControlOp::Retry, &active));

    // Retryable failure: retry executable; cancel not (terminal).
    let failed = job_record(
        RenderJobState::Failed,
        Some(JobFailure {
            reason: JobFailureReason::WorkerFailure,
            detail: "boom".to_string(),
        }),
    );
    let ids = executable_ids(&render_job_candidates(&failed));
    assert!(ids.contains(ACTION_RENDER_RETRY));
    assert!(!ids.contains(ACTION_RENDER_CANCEL));
    assert!(job_control_allowed(JobControlOp::Retry, &failed));

    // Non-retryable failure: nothing executable but verify.
    let denied = job_record(
        RenderJobState::Failed,
        Some(JobFailure {
            reason: JobFailureReason::AuthDenied,
            detail: "acl".to_string(),
        }),
    );
    let ids = executable_ids(&render_job_candidates(&denied));
    assert_eq!(ids, BTreeSet::from([ACTION_RENDER_VERIFY]));

    // Cancel-requested job: cancel no longer offered (idempotent —
    // the request already landed).
    let mut cancelling = job_record(RenderJobState::Rendering, None);
    cancelling.cancel_requested = true;
    let ids = executable_ids(&render_job_candidates(&cancelling));
    assert!(!ids.contains(ACTION_RENDER_CANCEL));
}

#[test]
fn job_projection_carries_receipts_key_summary_and_entry() {
    let mut record = job_record(RenderJobState::Partial, None);
    record.external_manifests = vec!["m1".to_string()];
    record.completeness = Some(CompletenessReport {
        verdict: CompletenessVerdict::Incomplete,
        gaps: vec![CompletenessGap {
            recipe_key: None,
            boundary: demand().boundary,
            kind: CompletenessGapKind::PluginMissing,
            detail: "plug-x".to_string(),
        }],
    });
    let receipts = vec![
        JobReceipt {
            job: record.key.clone(),
            seq: 1,
            tick: 1,
            kind: ReceiptKind::Submitted {
                entry: EntrySurface::Gui,
                triggers: Vec::new(),
                target_locator: "saved|p|head".to_string(),
                snapshot_digest: digest(0xD4),
                saved_revision: None,
            },
        },
        JobReceipt {
            job: job_key(),
            seq: 2,
            tick: 2,
            kind: ReceiptKind::Terminal {
                state: RenderJobState::Partial,
                artifacts: Vec::new(),
                completeness: Some(CompletenessVerdict::Incomplete),
                failure: None,
            },
        },
    ];
    let view = project_render_job(&record, &receipts);
    assert_eq!(view.state_label, "partial");
    assert_eq!(view.entry, "gui");
    assert!(view.key_summary.contains("synvelis.render.request"));
    assert!(view.key_summary.contains("no-grant"));
    let completeness = view.completeness.expect("completeness");
    assert_eq!(completeness.verdict, "incomplete");
    assert_eq!(completeness.gaps[0].kind, "plugin-missing");
    // Attach is executable: Partial + external manifests present.
    let ids = executable_ids(&view.actions);
    assert!(ids.contains(ACTION_RENDER_ATTACH));
    assert!(ids.contains(ACTION_RENDER_VERIFY));
    // Receipts tail bounded and labelled — never mixed with VCS/save.
    assert_eq!(view.receipts.len(), 2);
    assert!(view.receipts[0].kind.starts_with("submitted:"));
}

#[test]
fn job_control_command_is_idempotent_on_the_key() {
    let key = job_key();
    let (op, out_key) = render_job_command(JobControlOp::Retry, key.clone());
    assert_eq!(op, JobControlOp::Retry);
    assert_eq!(out_key, key);
    // Same key + same op → same command identity: the orchestrator
    // coalesces rather than duplicating work.
    let (op2, key2) = render_job_command(JobControlOp::Retry, key);
    assert_eq!((op, out_key), (op2, key2));
}

// ---------------------------------------------------------------------------
// view-state persistence tolerance
// ---------------------------------------------------------------------------

#[test]
fn render_view_state_round_trips_and_tolerates_unknown_fields() {
    let mut state = RenderViewState {
        surface: RenderSurface::BranchDiff,
        selected_boundary: Some(2),
        ..RenderViewState::default()
    };
    apply_render_view_action(
        &mut state,
        RenderViewAction::SetScope {
            scope: RenderScope::MixSnapshot,
        },
    );
    let json = serde_json::to_string(&state).expect("serialize");
    // Unknown fields are ignored on read (tolerant per-view state).
    let tolerant = json.replacen('{', "{\"future-field\":1,", 1);
    let back: RenderViewState = serde_json::from_str(&tolerant).expect("deserialize");
    assert_eq!(back.selected_boundary, Some(2));
    assert_eq!(back.draft_scope, Some(RenderScope::MixSnapshot));
    assert_eq!(back.surface, RenderSurface::BranchDiff);
}
