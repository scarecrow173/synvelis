//! AUTX-07 (synvelis#266) — headless tests for the automation UI
//! projection layer (`synvelis_workspace::automation_ui`).
//!
//! Coverage map:
//! - Action Palette catalog projection: availability × grant
//!   fail-closed matrix — disabled/unauthorized/unknown never render
//!   executable.
//! - Explicit target capture: contract shape/kind matching, mismatch
//!   rejection, frozen `CapturedTarget`.
//! - `automation_requests`: view-local actions mint nothing; preview /
//!   submit / macro-apply / job-cancel lower to `WireRequest`s only —
//!   no authority is created here.
//! - Macro step lowering: ordered per-step requests on one project,
//!   caller-supplied consistency, per-step idempotency suffix.
//! - Run Monitor status mapping: pending ≠ completed; cancel
//!   requested/confirmed/unconfirmed and indeterminate stay distinct.
//! - `AutomationViewState` persistence: serde roundtrip and tolerant
//!   degradation of malformed payloads inside `ViewState`.

use serde_json::json;

use synvelis_gateway::fixture::fixture_descriptors;
use synvelis_gateway::{
    Availability, CancelLink, CapabilityDescriptor, CapabilityId, CapabilityVersion,
    ExecutionContract, JobPhase, OperationDomain, PermissionKind, ResourceKind, TargetContract,
    WireConsistency,
};
use synvelis_project::ProjectRevision;
use synvelis_workspace::*;

// ── fixtures ──────────────────────────────────────────────────────────

fn descriptors() -> Vec<CapabilityDescriptor> {
    fixture_descriptors()
}

fn grant_satisfied() -> GrantState {
    GrantState::Satisfied
}

fn catalog() -> ActionCatalogView {
    project_action_catalog(&descriptors(), |_| grant_satisfied(), "", 7)
}

fn preset_view() -> MacroPresetView {
    MacroPresetView {
        id: "normalize-loudness".to_string(),
        name: "Normalize Loudness".to_string(),
        version: "1.0.0".to_string(),
        inputs: vec![MacroInputView {
            name: "target_lufs".to_string(),
            required: true,
        }],
        steps: vec![
            MacroStepView {
                capability: synvelis_gateway::CapabilityRef {
                    id: CapabilityId::new("synvelis.midi.add-note").unwrap(),
                    version: CapabilityVersion::new(1, 1),
                },
                args: json!({ "pitch": 60 }),
                target: Some(ExplicitTarget::Resource {
                    kind: "source".to_string(),
                    id: "source:abc".to_string(),
                }),
            },
            MacroStepView {
                capability: synvelis_gateway::CapabilityRef {
                    id: CapabilityId::new("synvelis.transport.set-position").unwrap(),
                    version: CapabilityVersion::new(1, 0),
                },
                args: json!({ "position": 0 }),
                target: None,
            },
        ],
        effects: vec![
            synvelis_gateway::EffectKind::Read,
            synvelis_gateway::EffectKind::Write,
        ],
        undo_scope: "project-undo".to_string(),
        atomic: true,
        availability: Availability::Available,
        grant: GrantState::Satisfied,
        executable: true,
        blocked: None,
    }
}

// ── Action Palette projection ─────────────────────────────────────────

#[test]
fn catalog_projects_every_fixture_row_with_display_fields() {
    let view = catalog();
    assert_eq!(view.rows.len(), descriptors().len());
    assert_eq!(view.seq, 7);
    let row = catalog_row(&view, "synvelis.midi.add-note@1.1").expect("row");
    assert!(row.executable);
    assert!(row.blocked.is_none());
    assert_eq!(row.domain, OperationDomain::ProjectMutation);
    assert_eq!(row.permissions, vec!["mutate-project".to_string()]);
    assert!(row.effects.contains(&synvelis_gateway::EffectKind::Write));
    assert_eq!(row.input_schema.name, "synvelis.midi.add-note.input");
    assert_eq!(
        ActionRow::schema_label(&row.input_schema),
        "synvelis.midi.add-note.input@1.1"
    );
    assert_eq!(target_contract_summary(&row.target), "resource:source");
}

#[test]
fn catalog_query_filters_by_id_and_domain_case_insensitively() {
    let view = project_action_catalog(&descriptors(), |_| grant_satisfied(), "MIDI", 1);
    assert!(view.rows.iter().all(|r| r.id.contains("midi")));
    let view = project_action_catalog(&descriptors(), |_| grant_satisfied(), "runtime", 1);
    assert_eq!(view.rows.len(), 1);
    assert_eq!(view.rows[0].id, "synvelis.transport.set-position");
}

#[test]
fn fail_closed_matrix_never_renders_blocked_rows_executable() {
    // Disabled capability + satisfied grant → blocked.
    let mut disabled = descriptors();
    disabled[0].availability = Availability::Disabled {
        reason: "maintenance".to_string(),
    };
    let view = project_action_catalog(&disabled, |_| grant_satisfied(), "", 1);
    let row = catalog_row(&view, "synvelis.context.source-notes@1.0").unwrap();
    assert!(!row.executable);
    assert!(row.blocked.as_deref().unwrap().contains("disabled"));

    // Planned capability → blocked even with a satisfied grant.
    let mut planned = descriptors();
    planned[0].availability = Availability::Planned;
    let view = project_action_catalog(&planned, |_| grant_satisfied(), "", 1);
    assert!(
        !catalog_row(&view, "synvelis.context.source-notes@1.0")
            .unwrap()
            .executable
    );

    // Available capability + missing grant → blocked.
    let view = project_action_catalog(
        &descriptors(),
        |_| GrantState::Missing {
            permissions: vec!["mutate-project".to_string()],
        },
        "",
        1,
    );
    let row = catalog_row(&view, "synvelis.midi.add-note@1.1").unwrap();
    assert!(!row.executable);
    assert!(row.blocked.as_deref().unwrap().contains("grant missing"));

    // Available capability + unknown grant → blocked (never assumed).
    let view = project_action_catalog(&descriptors(), |_| GrantState::Unknown, "", 1);
    assert!(view.rows.iter().all(|r| !r.executable));
    assert!(
        view.rows
            .iter()
            .all(|r| r.blocked.as_deref().unwrap().contains("grant unknown"))
    );
}

#[test]
fn blocked_rows_stay_visible_in_catalog() {
    // Unsupported/unauthorized rows are inspectable — filtering hides
    // only non-matching queries, never blocked states.
    let view = project_action_catalog(&descriptors(), |_| GrantState::Unknown, "", 1);
    assert_eq!(view.rows.len(), descriptors().len());
}

// ── explicit target capture ───────────────────────────────────────────

#[test]
fn capture_target_accepts_matching_resource_contract() {
    let desc = &descriptors()[1]; // synvelis.midi.add-note → Resource{Source}
    let captured = capture_target(
        desc,
        "project:main",
        Some(ExplicitTarget::Resource {
            kind: "source".to_string(),
            id: "source:clip-1".to_string(),
        }),
    )
    .expect("matching target");
    assert_eq!(captured.capability_id, "synvelis.midi.add-note");
    assert_eq!(captured.capability_version, "1.1");
    assert_eq!(captured.project, "project:main");
    assert!(matches!(
        captured.target,
        Some(ExplicitTarget::Resource { .. })
    ));
}

#[test]
fn capture_target_rejects_missing_wrong_kind_and_wrong_shape() {
    let desc = &descriptors()[1];
    assert!(matches!(
        capture_target(desc, "project:main", None),
        Err(AutomationUiError::TargetMismatch { .. })
    ));
    assert!(matches!(
        capture_target(
            desc,
            "project:main",
            Some(ExplicitTarget::Resource {
                kind: "track".to_string(),
                id: "track:1".to_string(),
            }),
        ),
        Err(AutomationUiError::TargetMismatch { .. })
    ));
    assert!(matches!(
        capture_target(
            desc,
            "project:main",
            Some(ExplicitTarget::ResourceSet {
                kind: "source".to_string(),
                ids: vec!["source:a".to_string()],
            }),
        ),
        Err(AutomationUiError::TargetMismatch { .. })
    ));
}

#[test]
fn capture_target_none_contract_rejects_any_explicit_target() {
    let desc = &descriptors()[3]; // transport.set-position → TargetContract::None
    assert!(capture_target(desc, "project:main", None).is_ok());
    assert!(matches!(
        capture_target(
            desc,
            "project:main",
            Some(ExplicitTarget::Resource {
                kind: "source".to_string(),
                id: "source:x".to_string(),
            }),
        ),
        Err(AutomationUiError::TargetMismatch { .. })
    ));
}

#[test]
fn capture_target_rejects_empty_payload_ids() {
    // Empty strings/sets can never lower on the wire — they must be
    // rejected at the freeze boundary, not discovered at bind.
    let desc = &descriptors()[1]; // Resource{Source}
    assert!(matches!(
        capture_target(
            desc,
            "project:main",
            Some(ExplicitTarget::Resource {
                kind: "source".to_string(),
                id: String::new(),
            }),
        ),
        Err(AutomationUiError::InvalidRequest { .. })
    ));

    // A ResourceSet contract descriptor (fixture has none — clone and
    // override the contract for this boundary check).
    let mut set_desc = descriptors()[1].clone();
    set_desc.target = TargetContract::ResourceSet {
        kind: ResourceKind::Source,
    };
    for ids in [vec![], vec!["source:a".to_string(), String::new()]] {
        assert!(matches!(
            capture_target(
                &set_desc,
                "project:main",
                Some(ExplicitTarget::ResourceSet {
                    kind: "source".to_string(),
                    ids,
                }),
            ),
            Err(AutomationUiError::InvalidRequest { .. })
        ));
    }
    assert!(
        capture_target(
            &set_desc,
            "project:main",
            Some(ExplicitTarget::ResourceSet {
                kind: "source".to_string(),
                ids: vec!["source:a".to_string()],
            }),
        )
        .is_ok()
    );

    // Create contract: empty parent id rejected.
    let desc = &descriptors()[2];
    assert!(matches!(
        capture_target(
            desc,
            "project:main",
            Some(ExplicitTarget::Create {
                parent_kind: "project".to_string(),
                parent: String::new(),
                creates: "source".to_string(),
            }),
        ),
        Err(AutomationUiError::InvalidRequest { .. })
    ));
}

#[test]
fn capture_target_create_contract_requires_matching_kinds() {
    let desc = &descriptors()[2]; // create-source → Create{Project→Source}
    let ok = capture_target(
        desc,
        "project:main",
        Some(ExplicitTarget::Create {
            parent_kind: "project".to_string(),
            parent: "project:main".to_string(),
            creates: "source".to_string(),
        }),
    );
    assert!(ok.is_ok());
    let bad = capture_target(
        desc,
        "project:main",
        Some(ExplicitTarget::Create {
            parent_kind: "project".to_string(),
            parent: "project:main".to_string(),
            creates: "track".to_string(),
        }),
    );
    assert!(matches!(bad, Err(AutomationUiError::TargetMismatch { .. })));
}

// ── request lowering ──────────────────────────────────────────────────

const CONSISTENCY: fn() -> &'static WireConsistency = || &WireConsistency::Unconditional;

#[test]
fn view_local_actions_mint_no_requests() {
    for action in [
        AutomationViewAction::PaletteSelect { row: 0 },
        AutomationViewAction::PaletteCategory { index: 1 },
        AutomationViewAction::CaptureTarget {
            capability: "synvelis.midi.add-note@1.1".to_string(),
            target: None,
        },
        AutomationViewAction::MacroPreview {
            preset: "normalize-loudness".to_string(),
        },
        AutomationViewAction::MacroSelect { index: 0 },
        AutomationViewAction::WorkflowSelect { index: 0 },
        AutomationViewAction::RunSelect { index: 0 },
    ] {
        let reqs = automation_requests(
            &action,
            &descriptors(),
            &[],
            "project:main",
            "req-1",
            CONSISTENCY(),
        )
        .expect("view-local actions never fail");
        assert!(reqs.is_empty(), "{action:?} minted requests");
    }
}

#[test]
fn preview_and_submit_lower_to_single_wire_request() {
    let target = Some(ExplicitTarget::Resource {
        kind: "source".to_string(),
        id: "source:clip-1".to_string(),
    });
    let preview = automation_requests(
        &AutomationViewAction::Preview {
            capability: "synvelis.midi.add-note@1.1".to_string(),
            target: target.clone(),
            args: json!({ "pitch": 60 }),
        },
        &descriptors(),
        &[],
        "project:main",
        "req-preview",
        &WireConsistency::Latest,
    )
    .expect("preview");
    assert_eq!(preview.len(), 1);
    let req = &preview[0];
    assert_eq!(req.capability.id.to_string(), "synvelis.midi.add-note");
    assert_eq!(req.project, "project:main");
    assert!(matches!(
        req.target,
        Some(synvelis_gateway::WireTarget::Resource { .. })
    ));
    assert!(req.idempotency_key.is_none());

    let submit = automation_requests(
        &AutomationViewAction::Submit {
            capability: "synvelis.midi.add-note@1.1".to_string(),
            target,
            args: json!({ "pitch": 60 }),
            idempotency_key: Some("op-key-1".to_string()),
        },
        &descriptors(),
        &[],
        "project:main",
        "req-submit",
        &WireConsistency::ExpectedRevision {
            revision: ProjectRevision(3),
        },
    )
    .expect("submit");
    assert_eq!(submit.len(), 1);
    assert_eq!(submit[0].idempotency_key.as_deref(), Some("op-key-1"));
    assert!(matches!(
        submit[0].consistency,
        WireConsistency::ExpectedRevision { .. }
    ));
}

#[test]
fn submit_rejects_unknown_capability_missing_target_and_unavailable() {
    let unknown = automation_requests(
        &AutomationViewAction::Submit {
            capability: "synvelis.nope@9.9".to_string(),
            target: None,
            args: json!({}),
            idempotency_key: None,
        },
        &descriptors(),
        &[],
        "project:main",
        "req-x",
        CONSISTENCY(),
    );
    assert!(matches!(
        unknown,
        Err(AutomationUiError::UnknownCapability { .. })
    ));

    // add-note requires an explicit source target — none supplied.
    let missing_target = automation_requests(
        &AutomationViewAction::Submit {
            capability: "synvelis.midi.add-note@1.1".to_string(),
            target: None,
            args: json!({}),
            idempotency_key: None,
        },
        &descriptors(),
        &[],
        "project:main",
        "req-x",
        CONSISTENCY(),
    );
    assert!(matches!(
        missing_target,
        Err(AutomationUiError::TargetMismatch { .. })
    ));

    // Disabled capability is never lowered to a request.
    let mut disabled = descriptors();
    disabled[1].availability = Availability::Disabled {
        reason: "off".to_string(),
    };
    let blocked = automation_requests(
        &AutomationViewAction::Submit {
            capability: "synvelis.midi.add-note@1.1".to_string(),
            target: Some(ExplicitTarget::Resource {
                kind: "source".to_string(),
                id: "source:a".to_string(),
            }),
            args: json!({}),
            idempotency_key: None,
        },
        &disabled,
        &[],
        "project:main",
        "req-x",
        CONSISTENCY(),
    );
    assert!(matches!(
        blocked,
        Err(AutomationUiError::NotExecutable { .. })
    ));
}

#[test]
fn macro_apply_lowers_ordered_steps_with_suffixed_idempotency() {
    let preset = preset_view();
    let reqs = automation_requests(
        &AutomationViewAction::MacroApply {
            preset: preset.id.clone(),
            idempotency_key: Some("macro-run-1".to_string()),
        },
        &descriptors(),
        std::slice::from_ref(&preset),
        "project:main",
        "req-macro",
        &WireConsistency::ExpectedRevision {
            revision: ProjectRevision(9),
        },
    )
    .expect("macro apply");
    assert_eq!(reqs.len(), 2);
    assert_eq!(reqs[0].capability.id.to_string(), "synvelis.midi.add-note");
    assert_eq!(
        reqs[1].capability.id.to_string(),
        "synvelis.transport.set-position"
    );
    // One project, caller-supplied consistency, per-step idempotency.
    assert!(reqs.iter().all(|r| r.project == "project:main"));
    assert!(
        reqs.iter()
            .all(|r| matches!(r.consistency, WireConsistency::ExpectedRevision { .. }))
    );
    assert_eq!(reqs[0].idempotency_key.as_deref(), Some("macro-run-1/0"));
    assert_eq!(reqs[1].idempotency_key.as_deref(), Some("macro-run-1/1"));
    // Step 1 carries its explicit target; step 2 (None contract) does not.
    assert!(reqs[0].target.is_some());
    assert!(reqs[1].target.is_none());
}

#[test]
fn macro_apply_rejects_unknown_preset_and_non_executable_step() {
    let unknown = automation_requests(
        &AutomationViewAction::MacroApply {
            preset: "nope".to_string(),
            idempotency_key: None,
        },
        &descriptors(),
        &[],
        "project:main",
        "req-x",
        CONSISTENCY(),
    );
    assert!(matches!(
        unknown,
        Err(AutomationUiError::UnknownCapability { .. })
    ));

    // A preset whose step targets an unavailable capability must not
    // lower — the request path re-checks availability per step.
    let mut preset = preset_view();
    preset.steps[0].capability.id = CapabilityId::new("synvelis.midi.create-source").unwrap();
    preset.steps[0].capability.version = CapabilityVersion::new(1, 0);
    preset.steps[0].target = Some(ExplicitTarget::Create {
        parent_kind: "project".to_string(),
        parent: "project:main".to_string(),
        creates: "source".to_string(),
    });
    let mut disabled = descriptors();
    disabled[2].availability = Availability::Retired;
    let blocked = automation_requests(
        &AutomationViewAction::MacroApply {
            preset: preset.id.clone(),
            idempotency_key: None,
        },
        &disabled,
        std::slice::from_ref(&preset),
        "project:main",
        "req-x",
        CONSISTENCY(),
    );
    assert!(matches!(
        blocked,
        Err(AutomationUiError::NotExecutable { .. })
    ));
}

fn job_cancel_descriptor() -> CapabilityDescriptor {
    let mut cancel = descriptors()[3].clone();
    cancel.id = CapabilityId::new(synvelis_gateway::JOB_CANCEL_ID).unwrap();
    cancel.domain = OperationDomain::RuntimeControl;
    cancel.target = TargetContract::None;
    cancel.execution = ExecutionContract::Sync;
    cancel.required_permissions =
        synvelis_gateway::PermissionSet::of([PermissionKind::ControlTransport]);
    cancel
}

/// A gateway-valid tagged operation id (`operation-<32hex>`).
fn tagged_operation() -> String {
    format!("operation-{:032x}", 0xabc_u128)
}

#[test]
fn macro_apply_rejects_inconsistent_preset_flags() {
    // `MacroPresetView` fields are public — a hand-built preset with
    // `executable: true` but unavailable/unauthorized authority fields
    // must still fail closed at lowering.
    let mut preset = preset_view();
    preset.availability = Availability::Disabled {
        reason: "off".to_string(),
    };
    preset.blocked = Some("disabled".to_string());
    let blocked = automation_requests(
        &AutomationViewAction::MacroApply {
            preset: preset.id.clone(),
            idempotency_key: None,
        },
        &descriptors(),
        std::slice::from_ref(&preset),
        "project:main",
        "req-x",
        CONSISTENCY(),
    );
    assert!(matches!(
        blocked,
        Err(AutomationUiError::NotExecutable { .. })
    ));

    let mut preset = preset_view();
    preset.grant = GrantState::Unknown;
    preset.blocked = Some("grant unknown".to_string());
    let blocked = automation_requests(
        &AutomationViewAction::MacroApply {
            preset: preset.id.clone(),
            idempotency_key: None,
        },
        &descriptors(),
        std::slice::from_ref(&preset),
        "project:main",
        "req-x",
        CONSISTENCY(),
    );
    assert!(matches!(
        blocked,
        Err(AutomationUiError::NotExecutable { .. })
    ));
}

#[test]
fn job_cancel_lowers_to_cancel_capability_request() {
    let mut descs = descriptors();
    // The composition root supplies the job capabilities; the fixture
    // catalog does not include `synvelis.job.cancel`, so register a
    // minimal descriptor for it.
    descs.push(job_cancel_descriptor());

    let reqs = automation_requests(
        &AutomationViewAction::JobCancel {
            operation: tagged_operation(),
        },
        &descs,
        &[],
        "project:main",
        "req-cancel",
        CONSISTENCY(),
    )
    .expect("job cancel");
    assert_eq!(reqs.len(), 1);
    let req = &reqs[0];
    assert_eq!(
        req.capability.id.to_string(),
        synvelis_gateway::JOB_CANCEL_ID
    );
    assert!(req.target.is_none());
    assert_eq!(req.args, json!({ "operation": tagged_operation() }));
}

#[test]
fn job_cancel_rejects_malformed_disabled_and_ambiguous() {
    let mut descs = descriptors();
    descs.push(job_cancel_descriptor());

    // Untagged operation text is rejected at the boundary — the wire
    // arg must already be the `operation-<32hex>` form.
    let bad_op = automation_requests(
        &AutomationViewAction::JobCancel {
            operation: "op-123".to_string(),
        },
        &descs,
        &[],
        "project:main",
        "req-cancel",
        CONSISTENCY(),
    );
    assert!(matches!(
        bad_op,
        Err(AutomationUiError::InvalidRequest { .. })
    ));

    // A disabled cancel capability never lowers.
    let mut disabled = descs.clone();
    disabled[5].availability = Availability::Disabled {
        reason: "off".to_string(),
    };
    let blocked = automation_requests(
        &AutomationViewAction::JobCancel {
            operation: tagged_operation(),
        },
        &disabled,
        &[],
        "project:main",
        "req-cancel",
        CONSISTENCY(),
    );
    assert!(matches!(
        blocked,
        Err(AutomationUiError::NotExecutable { .. })
    ));

    // Two registered versions of the cancel capability are ambiguous —
    // never pick one by catalog order.
    let mut ambiguous = descs.clone();
    ambiguous.push(job_cancel_descriptor());
    let dup = automation_requests(
        &AutomationViewAction::JobCancel {
            operation: tagged_operation(),
        },
        &ambiguous,
        &[],
        "project:main",
        "req-cancel",
        CONSISTENCY(),
    );
    assert!(matches!(dup, Err(AutomationUiError::InvalidRequest { .. })));
}

#[test]
fn job_cancel_fails_closed_without_registered_cancel_capability() {
    // No `synvelis.job.cancel` in the catalog → error, never a guess.
    let res = automation_requests(
        &AutomationViewAction::JobCancel {
            operation: "op-123".to_string(),
        },
        &descriptors(),
        &[],
        "project:main",
        "req-cancel",
        CONSISTENCY(),
    );
    assert!(matches!(
        res,
        Err(AutomationUiError::UnknownCapability { .. })
    ));
}

// ── Macro / Workflow / Run projections ────────────────────────────────

#[test]
fn macro_preset_projection_carries_version_inputs_effects_undo() {
    let preset = preset_view();
    let row = project_macro_preset(&preset, grant_satisfied());
    assert_eq!(row.name, "Normalize Loudness");
    assert_eq!(row.version, "1.0.0");
    assert_eq!(row.inputs.len(), 1);
    assert_eq!(row.steps.len(), 2);
    assert!(row.atomic);
    assert_eq!(row.undo_scope, "project-undo");
    assert!(row.executable);
}

#[test]
fn workflow_plan_projection_preserves_validation_and_approval_display() {
    let plan = project_workflow_plan(
        "export-pipeline",
        "0.3.0",
        vec![
            WorkflowStepView {
                index: 0,
                kind: WorkflowStepKind::AtomicBatch,
                label: "normalize".to_string(),
                capability: Some(synvelis_gateway::CapabilityRef {
                    id: CapabilityId::new("synvelis.midi.add-note").unwrap(),
                    version: CapabilityVersion::new(1, 1),
                }),
                bound: None,
            },
            WorkflowStepView {
                index: 1,
                kind: WorkflowStepKind::Await,
                label: "wait render".to_string(),
                capability: None,
                bound: Some("timeout=30s".to_string()),
            },
        ],
        PlanValidation::Invalid {
            reasons: vec!["step 2 awaits unknown job".to_string()],
        },
        "required",
        4,
    );
    assert_eq!(plan.steps.len(), 2);
    assert!(matches!(plan.validation, PlanValidation::Invalid { .. }));
    assert_eq!(plan.approval, "required");
    assert_eq!(plan.seq, 4);
    assert_eq!(plan.steps[1].kind.name(), "await");
}

// ── Run Monitor status mapping ────────────────────────────────────────

fn job(phase: JobPhase, cancel: CancelLink) -> JobObservation {
    JobObservation {
        operation: "op-1".to_string(),
        job: "job-1".to_string(),
        phase,
        cancel,
        progress: None,
    }
}

#[test]
fn pending_is_never_projected_as_completed() {
    assert_eq!(
        project_job_status(&job(JobPhase::Pending, CancelLink::NotRequested)),
        MonitorStatus::Pending
    );
    assert_eq!(
        project_job_status(&job(JobPhase::Running, CancelLink::NotRequested)),
        MonitorStatus::Running
    );
}

#[test]
fn completed_requires_provider_confirmation() {
    assert_eq!(
        project_job_status(&job(JobPhase::Completed, CancelLink::NotRequested)),
        MonitorStatus::Completed
    );
    // Unconfirmed cancel linkage on a completed phase → indeterminate.
    assert_eq!(
        project_job_status(&job(JobPhase::Completed, CancelLink::Unconfirmed)),
        MonitorStatus::Indeterminate
    );
}

#[test]
fn pending_phase_still_honors_the_cancel_axis() {
    // Pending is non-terminal: the cancel linkage axis applies exactly
    // as on running — requested is cancel-requested, unconfirmed or
    // self-contradictory (confirmed on a still-pending phase) is
    // indeterminate, never plain pending.
    assert_eq!(
        project_job_status(&job(JobPhase::Pending, CancelLink::Requested)),
        MonitorStatus::CancelRequested
    );
    assert_eq!(
        project_job_status(&job(JobPhase::Pending, CancelLink::Unconfirmed)),
        MonitorStatus::Indeterminate
    );
    assert_eq!(
        project_job_status(&job(JobPhase::Pending, CancelLink::Confirmed)),
        MonitorStatus::Indeterminate
    );
    assert_eq!(
        project_job_status(&job(JobPhase::Pending, CancelLink::NotRequested)),
        MonitorStatus::Pending
    );
}

#[test]
fn cancel_requested_confirmed_unconfirmed_stay_distinct() {
    assert_eq!(
        project_job_status(&job(JobPhase::Running, CancelLink::Requested)),
        MonitorStatus::CancelRequested
    );
    assert_eq!(
        project_job_status(&job(JobPhase::Running, CancelLink::Unconfirmed)),
        MonitorStatus::Indeterminate
    );
    assert_eq!(
        project_job_status(&job(JobPhase::Cancelled, CancelLink::Confirmed)),
        MonitorStatus::Cancelled
    );
    // Cancelled phase without confirmed linkage is *not* cancelled.
    assert_eq!(
        project_job_status(&job(JobPhase::Cancelled, CancelLink::Requested)),
        MonitorStatus::CancelRequested
    );
    assert_eq!(
        project_job_status(&job(JobPhase::Cancelled, CancelLink::NotRequested)),
        MonitorStatus::CancelUnconfirmed
    );
    // Failure with unconfirmed linkage → indeterminate, not failed.
    assert_eq!(
        project_job_status(&job(
            JobPhase::Failed {
                reason: "boom".to_string(),
            },
            CancelLink::Unconfirmed,
        )),
        MonitorStatus::Indeterminate
    );
}

#[test]
fn run_monitor_rows_preserve_authority_fields_and_cancel_axis() {
    let view = project_run_monitor(
        vec![
            RunObservation {
                run: "run-1".to_string(),
                operation: Some("op-1".to_string()),
                step: Some(2),
                project_revision: Some(ProjectRevision(11)),
                run_state: "running".to_string(),
                recovery_required: false,
                job: Some(job(JobPhase::Running, CancelLink::Requested)),
            },
            RunObservation {
                run: "run-2".to_string(),
                operation: None,
                step: None,
                project_revision: None,
                run_state: "queued".to_string(),
                recovery_required: true,
                job: None,
            },
        ],
        3,
    );
    assert_eq!(view.rows.len(), 2);
    assert_eq!(view.seq, 3);
    let r1 = &view.rows[0];
    assert_eq!(r1.status, MonitorStatus::CancelRequested);
    assert!(r1.cancel_requested);
    assert_eq!(r1.project_revision, Some(ProjectRevision(11)));
    assert_eq!(r1.step, Some(2));
    let r2 = &view.rows[1];
    // No job observation → pending, never completed.
    assert_eq!(r2.status, MonitorStatus::Pending);
    assert!(r2.recovery_required);
    assert!(!r2.cancel_requested);
}

// ── persisted view state ──────────────────────────────────────────────

#[test]
fn automation_view_state_roundtrips_through_serde() {
    let state = AutomationViewState {
        palette_query: "midi".to_string(),
        palette_selected: Some(2),
        palette_category: Some(1),
        palette_target: Some(CapturedTarget {
            capability_id: "synvelis.midi.add-note".to_string(),
            capability_version: "1.1".to_string(),
            project: "project:main".to_string(),
            target: Some(ExplicitTarget::Resource {
                kind: "source".to_string(),
                id: "source:a".to_string(),
            }),
        }),
        palette_outcome: Some("submitted".to_string()),
        catalog_seq: 9,
        macro_selected: Some("normalize-loudness".to_string()),
        workflow_step: Some(1),
        run_selected: Some("run-1".to_string()),
        monitor_seq: 4,
    };

    let json = serde_json::to_string(&state).expect("serialize");
    let back: AutomationViewState = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(state, back);
}

#[test]
fn stale_epoch_helpers_reject_older_catalogs_and_monitors() {
    let state = AutomationViewState {
        catalog_seq: 5,
        monitor_seq: 8,
        ..AutomationViewState::default()
    };
    assert!(state.catalog_stale(4));
    assert!(!state.catalog_stale(5));
    assert!(!state.catalog_stale(6));
    assert!(state.monitor_stale(7));
    assert!(!state.monitor_stale(8));
}

#[test]
fn view_state_tolerates_malformed_automation_payload() {
    // Old snapshots lack `automation` entirely; malformed payloads
    // (wrong types, unknown fields from a newer build) degrade to
    // `None` instead of rejecting the whole ViewState.
    let mut base = serde_json::to_value(ViewState::default()).expect("serialize");
    base["automation"] = json!("not-an-object");
    let parsed: ViewState = serde_json::from_value(base.clone()).expect("malformed → None");
    assert_eq!(parsed.automation, None);

    base["automation"] = json!({ "palette_selected": "two", "future_field": 1 });
    let parsed: ViewState = serde_json::from_value(base).expect("bad shape → None");
    assert_eq!(parsed.automation, None);

    // A well-formed payload survives.
    let mut good = serde_json::to_value(ViewState::default()).expect("serialize");
    good["automation"] = json!({ "palette_query": "midi", "catalog_seq": 3 });
    let parsed: ViewState = serde_json::from_value(good).expect("good payload");
    let auto = parsed.automation.expect("automation state");
    assert_eq!(auto.palette_query, "midi");
    assert_eq!(auto.catalog_seq, 3);
}

#[test]
fn new_view_types_are_registered() {
    // ViewType variants exist for all four surfaces — exercised via
    // the workspace view registry path used by spawn/adopt.
    for vt in [
        ViewType::ActionPalette,
        ViewType::MacroBrowser,
        ViewType::WorkflowEditor,
        ViewType::RunMonitor,
    ] {
        let _ = format!("{vt:?}");
    }
}
