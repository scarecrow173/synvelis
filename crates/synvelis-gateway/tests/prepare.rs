//! GW-04 preparation tests — Issue #56 acceptance criteria.
//!
//! Each test names the contract it proves (GTC-005, G2, GT-005/006,
//! SEC-05/06/14): non-destructive preparation against a pinned base,
//! real effect derivation (linked MIDI vs explicit independent copy),
//! typed error taxonomy, GW-02 effect-aware authorization, and the
//! `ProjectController` single-writer/Undo integration seam.

mod rig;

use std::collections::BTreeSet;

use rig::*;
use serde_json::{Value, json};
use synvelis_gateway::*;
use synvelis_project::{
    AutomationTimeScope, ClipId, Command, ControlOwner, CounterIdGenerator, MidiTick, NoteId,
    ParamMapping, ParameterRole, ParameterTargetId, Polarity, PortDirection, PortOwner, PortRole,
    ProcessorId, ProjectController, ProjectOp, ProjectRevision, SemanticPortId, SendTap,
    SmfDocument, SmfEvent, SmfEventKind, SmfFormat, SmfTrack, SourceId, SourceKind, StripId,
    TrackId, TrackMonitorDecl, Transform, ValueKind,
};

// ── Happy path + non-destructivity ───────────────────────────

#[test]
fn prepare_derives_effects_and_mutates_nothing() {
    // GTC-005: a prepare validates on the pin, derives real effects,
    // and leaves the committed project byte-identical.
    let mut rig = fixture_rig();
    let before = rig.source.pin(project(), ProjectRevision(1)).unwrap();
    let before_json = serde_json::to_value(&*before).unwrap();

    let result = complete(
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
                    expected(1),
                ),
                human(),
            )
            .unwrap(),
    );

    let record = record_for(&rig, &result);
    // The pinned base is recorded for GW-06 revalidation.
    assert_eq!(record.prepared.base.revision, ProjectRevision(1));
    assert_eq!(result["base"]["revision"], json!(1));
    assert!(result["proposal"].is_string());
    // Read (validation consults the clip) + Write (muted flag).
    let read = effect(&record, EffectKind::Read, &clip11());
    assert_eq!(read.visibility, EffectVisibility::Internal);
    let write = effect(&record, EffectKind::Write, &clip11());
    assert_eq!(write.before_detail.as_ref().unwrap()["muted"], json!(false));
    assert_eq!(write.after_detail.as_ref().unwrap()["muted"], json!(true));
    // Zero mutation on the committed source.
    let after = rig.source.pin(project(), ProjectRevision(1)).unwrap();
    assert_eq!(serde_json::to_value(&*after).unwrap(), before_json);
    assert_eq!(
        rig.source.latest_revision(project()),
        Some(ProjectRevision(1))
    );
    // The candidate holds the would-be state, never the live one.
    assert!(record.prepared.candidate.clips[&ClipId::new(11)].muted);
    assert!(!after.clips[&ClipId::new(11)].muted);
}

// ── Invalid id / revision / range — zero mutation ────────────

#[test]
fn invalid_target_id_rejects_without_mutation() {
    // Invalid ids fail closed at the boundary — a malformed tag is a
    // wire error, an absent clip is the project's own validator
    // rejecting (no duplicated AI-side check invented the verdict).
    let mut rig = fixture_rig();
    let malformed = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                op(
                    "synvelis.clip.mute",
                    json!({"shape": "resource", "kind": "clip", "id": "clip-nothex"}),
                    json!({"muted": true}),
                ),
                expected(1),
            ),
            human(),
        )
        .expect_err("malformed id must fail");
    assert!(matches!(
        malformed.code,
        GatewayError::InvalidId { .. } | GatewayError::MalformedRequest { .. }
    ));

    let absent = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                op(
                    "synvelis.clip.mute",
                    res_target("clip", &ResourceRef::Clip(ClipId::new(999))),
                    json!({"muted": true}),
                ),
                expected(1),
            ),
            human(),
        )
        .expect_err("absent clip must fail");
    assert!(matches!(
        absent.code,
        GatewayError::PreparationRejected { .. }
    ));
    assert_eq!(
        rig.source.latest_revision(project()),
        Some(ProjectRevision(1))
    );
    assert_eq!(rig.engine.proposal_count(), 0);
}

#[test]
fn invalid_revision_rejects_without_mutation() {
    // An uncommitted base revision cannot be pinned — and there is
    // never a silent rebase to latest.
    let mut rig = fixture_rig();
    rig.source
        .commit(project(), ProjectRevision(2), demo_project());
    for consistency in [expected(9), read_at(9)] {
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
                    consistency,
                ),
                human(),
            )
            .expect_err("uncommitted base must fail");
        assert!(
            matches!(
                err.code,
                GatewayError::SnapshotExpired | GatewayError::RevisionConflict { .. }
            ),
            "unexpected error: {:?}",
            err.code
        );
    }
    // The older committed revision still pins exactly — no rebase.
    let result = complete(
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
                    expected(1),
                ),
                human(),
            )
            .unwrap(),
    );
    assert_eq!(result["base"]["revision"], json!(1));
    assert_eq!(rig.engine.proposal_count(), 1);
}

#[test]
fn wrong_time_domain_and_range_reject_without_mutation() {
    // Time domains are never converted implicitly: a project-musical
    // range is rejected where source-native is required (GT-006).
    let mut rig = fixture_rig();
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                op(
                    "synvelis.midi.note.add",
                    res_target("source", &source10()),
                    json!({"key": 60, "velocity": 90, "range": {"domain": "project-musical", "start": 0, "len": 240}}),
                ),
                expected(1),
            ),
            human(),
        )
        .expect_err("wrong-domain range must fail");
    assert!(
        matches!(
            err.code,
            GatewayError::WrongTimeDomain { .. } | GatewayError::SchemaViolation { .. }
        ),
        "unexpected error: {:?}",
        err.code
    );
    // Zero-length range — the boundary itself is rejected.
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                op(
                    "synvelis.midi.note.add",
                    res_target("source", &source10()),
                    json!({"key": 60, "velocity": 90, "range": {"domain": "source-native", "start": 0, "len": 0}}),
                ),
                expected(1),
            ),
            human(),
        )
        .expect_err("zero-length range must fail");
    assert!(matches!(
        err.code,
        GatewayError::SchemaViolation { .. }
            | GatewayError::MalformedRequest { .. }
            | GatewayError::PreparationRejected { .. }
    ));
    assert_eq!(rig.engine.proposal_count(), 0);
}

// ── Linked MIDI vs explicit independent copy ─────────────────

#[test]
fn linked_midi_write_names_source_and_linked_clips() {
    // Linked-S1 (SEC-05 / GTC-005): a source-level edit names the
    // shared source explicitly as the write, with every linked clip
    // in its references — never a silent clip-local copy-on-write.
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
    let record = record_for(&rig, &result);
    let write = effect(&record, EffectKind::Write, &source10());
    // The source itself carries the write; both linked clips are
    // named in its references — the shared-semantics impact.
    assert!(write.references.contains(&clip12()));
    assert!(write.references.contains(&clip13()));
    // The created note names the source as its explicit parent.
    let note_create = record
        .prepared
        .effects
        .iter()
        .find(|e| e.kind == EffectKind::Create && matches!(e.resource, ResourceRef::Note(_)))
        .expect("note create");
    assert_eq!(note_create.after.as_ref().unwrap().owner, Some(source10()));
    // The candidate grew S1's payload; the base did not.
    assert_eq!(
        record.prepared.candidate.midi_sources[&SourceId::new(10)]
            .notes
            .len(),
        3
    );
    let base = rig.source.pin(project(), ProjectRevision(1)).unwrap();
    assert_eq!(base.midi_sources[&SourceId::new(10)].notes.len(), 2);
}

#[test]
fn detach_is_explicit_new_source_plus_clip_repoint() {
    // Independent copy: a brand-new Source + fresh notes + the clip's
    // reference change — never implicit copy-on-write. The shared
    // source itself is untouched.
    let mut rig = fixture_rig();
    let result = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    op(
                        "synvelis.clip.detach-source",
                        res_target("clip", &clip12()),
                        json!({}),
                    ),
                    expected(1),
                ),
                human(),
            )
            .unwrap(),
    );
    let record = record_for(&rig, &result);
    // New Source created under the project, fresh note ids under it.
    let source_create = record
        .prepared
        .effects
        .iter()
        .find(|e| e.kind == EffectKind::Create && matches!(e.resource, ResourceRef::Source(_)))
        .expect("new source create");
    let ResourceRef::Source(new_source) = source_create.resource else {
        unreachable!()
    };
    assert_ne!(new_source, SourceId::new(10));
    assert_eq!(
        source_create.after.as_ref().unwrap().owner,
        Some(project_ref())
    );
    let note_creates: Vec<_> = record
        .prepared
        .effects
        .iter()
        .filter(|e| e.kind == EffectKind::Create && matches!(e.resource, ResourceRef::Note(_)))
        .collect();
    assert_eq!(note_creates.len(), 2);
    for n in &note_creates {
        assert_eq!(
            n.after.as_ref().unwrap().owner,
            Some(ResourceRef::Source(new_source))
        );
        assert!(!matches!(
            n.resource,
            ResourceRef::Note(id) if id == NoteId::new(20) || id == NoteId::new(21)
        ));
    }
    // The clip's reference changed S1 → new source (Write).
    let clip_write = effect(&record, EffectKind::Write, &clip12());
    assert_eq!(
        clip_write.before.as_ref().unwrap().related,
        vec![source10()]
    );
    assert_eq!(
        clip_write.after.as_ref().unwrap().related,
        vec![ResourceRef::Source(new_source)]
    );
    // The shared source is untouched — no Write/Delete on source10.
    assert!(record.prepared.effects.iter().all(|e| !(matches!(
        e.kind,
        EffectKind::Write | EffectKind::Delete
    ) && e.resource == source10())));
    // The base still shows the link; the candidate is detached.
    assert_eq!(
        record.prepared.candidate.clips[&ClipId::new(12)].source_id,
        new_source
    );
    assert_eq!(
        record.prepared.candidate.clips[&ClipId::new(13)].source_id,
        SourceId::new(10)
    );
}

// ── Clip move Before/After ───────────────────────────────────

#[test]
fn clip_move_reports_before_and_after_range() {
    // The placement window moves in its own domain — Before/After
    // carry the real attributes, no conversion.
    let mut rig = fixture_rig();
    let result = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    op(
                        "synvelis.clip.move",
                        res_target("clip", &clip11()),
                        json!({"anchor": {"kind": "Musical", "value": 600}}),
                    ),
                    expected(1),
                ),
                human(),
            )
            .unwrap(),
    );
    let record = record_for(&rig, &result);
    let write = effect(&record, EffectKind::Write, &clip11());
    let before = write.before.as_ref().unwrap();
    let after = write.after.as_ref().unwrap();
    assert_eq!(
        before.range,
        Some(TypedRange {
            domain: TimeDomain::ProjectMusical,
            start: 120,
            len: 240,
        })
    );
    assert_eq!(
        after.range,
        Some(TypedRange {
            domain: TimeDomain::ProjectMusical,
            start: 600,
            len: 240,
        })
    );
    // Same owner, same references — a move, not a reparent.
    assert_eq!(before.owner, after.owner);
    assert_eq!(after.related, vec![source9()]);
}

// ── Composite command effect composition ─────────────────────

#[test]
fn batch_composes_member_effects_in_order() {
    // A batch lowers to one atomic `Composite`; member commands are
    // normalized individually and effects compose on one candidate.
    let mut rig = fixture_rig();
    let result = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    op(
                        "synvelis.operation.batch",
                        Value::Null,
                        json!({"ops": [
                            {"capability": {"id": "synvelis.clip.mute", "version": {"major": 1, "minor": 0}},
                             "target": res_target("clip", &clip11()), "args": {"muted": true}},
                            {"capability": {"id": "synvelis.clip.move", "version": {"major": 1, "minor": 0}},
                             "target": res_target("clip", &clip11()), "args": {"anchor": {"kind": "Musical", "value": 960}}},
                        ]}),
                    ),
                    expected(1),
                ),
                human(),
            )
            .unwrap(),
    );
    let record = record_for(&rig, &result);
    assert_eq!(record.prepared.commands.len(), 2);
    assert_eq!(record.prepared.commands[0].op, "set-clip-muted");
    assert_eq!(record.prepared.commands[1].op, "move-clip");
    let write = effect(&record, EffectKind::Write, &clip11());
    assert_eq!(write.after_detail.as_ref().unwrap()["muted"], json!(true));
    assert_eq!(write.after.as_ref().unwrap().range.unwrap().start, 960);
    assert!(
        record
            .prepared
            .preconditions
            .iter()
            .any(|p| p.code == "composite-non-empty")
    );
    // The retained op is the atomic composite itself.
    assert!(matches!(
        record.project_op(),
        Some(synvelis_project::ProjectOp::Composite { .. })
    ));
}

#[test]
fn batch_rejects_atomically_when_a_member_fails() {
    // All-or-nothing: an illegal member rejects the whole proposal —
    // nothing prepares, nothing mutates (GT-005).
    let mut rig = fixture_rig();
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                op(
                    "synvelis.operation.batch",
                        Value::Null,
                    json!({"ops": [
                        {"capability": {"id": "synvelis.clip.mute", "version": {"major": 1, "minor": 0}},
                         "target": res_target("clip", &clip11()), "args": {"muted": true}},
                        {"capability": {"id": "synvelis.clip.mute", "version": {"major": 1, "minor": 0}},
                         "target": res_target("clip", &ResourceRef::Clip(ClipId::new(999))), "args": {"muted": true}},
                    ]}),
                ),
                expected(1),
            ),
            human(),
        )
        .expect_err("a failing member must reject the batch");
    assert!(matches!(
        err.code,
        GatewayError::PreparationRejected { .. }
            | GatewayError::MalformedRequest { .. }
            | GatewayError::SchemaViolation { .. }
    ));
    assert_eq!(rig.engine.proposal_count(), 0);
}

// ── GW-02 effect-aware authorization ─────────────────────────

#[test]
fn derived_effects_flow_through_grant_scope() {
    // Real effects feed GW-02: a scope covering the written clip
    // permits; one covering another clip denies identically to
    // outside-scope targets.
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
    let opv = op(
        "synvelis.clip.mute",
        res_target("clip", &clip11()),
        json!({"muted": true}),
    );
    complete(
        rig.gw
            .handle(
                &prepare_req(Some("t1"), Some("g1"), opv.clone(), expected(1)),
                agent(),
            )
            .unwrap(),
    );

    let mut rig = fixture_rig();
    issue_agent(
        &rig.store,
        "t1",
        prep_grant(
            "g1",
            "t1",
            &["synvelis.clip.mute"],
            ResourceScope::Exact {
                resources: BTreeSet::from([clip12()]),
            },
            None,
            &mutate_classes(),
        ),
    );
    let err = rig
        .gw
        .handle(
            &prepare_req(Some("t1"), Some("g1"), opv, expected(1)),
            agent(),
        )
        .expect_err("uncovered write must deny");
    match err.code {
        GatewayError::Denied { stage, reason } => {
            assert_eq!(stage, AuthorizationStage::PrepareProposal);
            assert_eq!(reason.unwrap(), "target outside grant scope");
        }
        other => panic!("expected Denied, got {other:?}"),
    }
    assert_eq!(rig.engine.proposal_count(), 0);
}

#[test]
fn inner_capability_outside_grant_denies() {
    // The proposal's inner operations are their own capabilities —
    // `prepare` being granted never covers its members (SEC-13).
    let mut rig = fixture_rig();
    let mut caps = BTreeSet::from([cap_ref("synvelis.operation.prepare")]);
    caps.insert(cap_ref("synvelis.clip.move"));
    issue_agent(
        &rig.store,
        "t1",
        Grant {
            id: grant_ref("g1"),
            issued_by: human_principal(),
            principal: agent_principal(),
            project: project(),
            binding: GrantBinding::Task {
                task: task_ref("t1"),
            },
            capabilities: caps,
            permissions: PermissionSet::of([PermissionKind::MutateProject]),
            data_access: DataAccessSet::of(mutate_classes()),
            scope: ResourceScope::Exact {
                resources: BTreeSet::from([clip11()]),
            },
            create_within: None,
            approval_policy: ApprovalPolicy::DelegatedApply,
            limits: GrantLimits::default(),
            validity: Validity::default(),
            epoch_ceiling: None,
            issued_at_revision: Some(ProjectRevision(1)),
        },
    );
    let err = rig
        .gw
        .handle(
            &prepare_req(
                Some("t1"),
                Some("g1"),
                op(
                    "synvelis.clip.mute",
                    res_target("clip", &clip11()),
                    json!({"muted": true}),
                ),
                expected(1),
            ),
            agent(),
        )
        .expect_err("ungranted inner capability must deny");
    match err.code {
        GatewayError::Denied { reason, .. } => {
            assert_eq!(reason.unwrap(), "capability outside grant");
        }
        other => panic!("expected Denied, got {other:?}"),
    }
}

#[test]
fn creation_requires_the_create_axis() {
    // Creates are evaluated against `create_within` — no create axis
    // denies even inside scope (AHG-008 via derived CreateEffect).
    let mut rig = fixture_rig();
    issue_agent(
        &rig.store,
        "t1",
        prep_grant(
            "g1",
            "t1",
            &["synvelis.track.create"],
            ResourceScope::Exact {
                resources: BTreeSet::from([project_ref()]),
            },
            None,
            &mutate_classes(),
        ),
    );
    let err = rig
        .gw
        .handle(
            &prepare_req(
                Some("t1"),
                Some("g1"),
                op(
                    "synvelis.track.create",
                    create_target("project", &project_ref(), "track"),
                    json!({"name": "t4"}),
                ),
                expected(1),
            ),
            agent(),
        )
        .expect_err("creation without create_within must deny");
    match err.code {
        GatewayError::Denied { reason, .. } => {
            assert_eq!(reason.unwrap(), "creation outside grant constraints");
        }
        other => panic!("expected Denied, got {other:?}"),
    }
}

#[test]
fn linked_source_write_needs_source_scope_not_just_clip() {
    // SEC-05: clip permission never implies source permission — the
    // derived Write on the shared source evaluates the source itself.
    let mut rig = fixture_rig();
    issue_agent(
        &rig.store,
        "t1",
        prep_grant(
            "g1",
            "t1",
            &["synvelis.midi.note.add"],
            ResourceScope::Exact {
                resources: BTreeSet::from([clip12(), clip13(), lane4()]),
            },
            None,
            &content_classes(),
        ),
    );
    let err = rig
        .gw
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
        .expect_err("source outside scope must deny");
    match err.code {
        GatewayError::Denied { reason, .. } => {
            assert_eq!(reason.unwrap(), "target outside grant scope");
        }
        other => panic!("expected Denied, got {other:?}"),
    }
}

// ── Unknown/unsupported fails closed ─────────────────────────

#[test]
fn unverifiable_effects_fail_closed_for_every_caller() {
    // SEC-14 / G2: an operation whose effects the derivation cannot
    // fully classify never produces a proposal — for agent *and*
    // human callers (the human early-permit is no escape hatch).
    let clock_op = || {
        op(
            "synvelis.clock-epoch.register",
            Value::Null,
            json!({"description": "clock"}),
        )
    };
    let mut rig = fixture_rig();
    issue_agent(
        &rig.store,
        "t1",
        prep_grant(
            "g1",
            "t1",
            &["synvelis.clock-epoch.register"],
            ResourceScope::Exact {
                resources: BTreeSet::from([project_ref()]),
            },
            None,
            &mutate_classes(),
        ),
    );
    for (req, caller) in [
        (
            prepare_req(Some("t1"), Some("g1"), clock_op(), expected(1)),
            agent(),
        ),
        (prepare_req(None, None, clock_op(), expected(1)), human()),
    ] {
        let err = rig
            .gw
            .handle(&req, caller)
            .expect_err("unverifiable effects must fail closed");
        match &err.code {
            GatewayError::PreparationRejected { reason } => {
                assert!(reason.contains("unverifiable"));
            }
            GatewayError::Denied { reason, .. } => {
                assert!(
                    reason
                        .as_deref()
                        .unwrap_or_default()
                        .contains("unverifiable")
                );
            }
            other => panic!("expected preparation rejection/denial, got {other:?}"),
        }
    }
    assert_eq!(rig.engine.proposal_count(), 0);
}

#[test]
fn unknown_capability_fails_closed() {
    // A capability the table does not lower is never prepared.
    let mut rig = fixture_rig();
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                op(
                    "synvelis.clip.frobnicate",
                    res_target("clip", &clip11()),
                    json!({}),
                ),
                expected(1),
            ),
            human(),
        )
        .expect_err("unknown inner capability must fail");
    assert!(matches!(err.code, GatewayError::UnknownCapability { .. }));
    assert_eq!(rig.engine.proposal_count(), 0);
}

// ── Evicted snapshot → no silent fallback ────────────────────

#[test]
fn expired_base_snapshot_never_rebases_to_latest() {
    // GT-005/pin discipline: an evicted fixed base reports expiry,
    // never silently reads a newer revision.
    let mut rig = fixture_rig();
    rig.source.evict(project(), ProjectRevision(1));
    rig.source
        .commit(project(), ProjectRevision(2), demo_project());
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
        .expect_err("evicted base must not rebase");
    assert!(matches!(
        err.code,
        GatewayError::SnapshotExpired | GatewayError::RevisionConflict { .. }
    ));
}

// ── ProjectController integration ────────────────────────────

#[test]
fn prepared_op_commits_through_controller_and_undoes() {
    // The retained `ProjectOp` is verbatim `Command` input for the
    // existing single-writer path — no gateway bypass of validate/
    // apply/Undo.
    let mut rig = fixture_rig();
    let result = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    op(
                        "synvelis.clip.detach-source",
                        res_target("clip", &clip12()),
                        json!({}),
                    ),
                    expected(1),
                ),
                human(),
            )
            .unwrap(),
    );
    let record = record_for(&rig, &result);
    // The dependency basis is the revalidation data GW-06 needs.
    assert_eq!(record.prepared.base.revision, ProjectRevision(1));

    let mut controller =
        ProjectController::new(demo_project(), Box::new(CounterIdGenerator::new(1000)));
    // Advance the controller's live revision to the prepared base
    // (a real deployment would reload a project at that revision).
    controller
        .apply(Command {
            expected_revision: ProjectRevision(0),
            op: synvelis_project::ProjectOp::SetProjectName {
                name: "demo".to_owned(),
            },
        })
        .unwrap();
    assert_eq!(controller.live_revision(), record.prepared.base.revision);
    // A stale expectation revalidates — base revision is the fence.
    let stale = controller.apply(Command {
        expected_revision: ProjectRevision(9),
        op: record.project_op().expect("project mutation").clone(),
    });
    assert!(matches!(
        stale,
        Err(synvelis_project::Error::StaleRevision { .. })
    ));
    // The prepared op applies verbatim, records one undo step.
    let rev = controller
        .apply(Command {
            expected_revision: record.prepared.base.revision,
            op: record.project_op().expect("project mutation").clone(),
        })
        .expect("prepared op must apply");
    assert_eq!(rev, ProjectRevision(2));
    assert!(controller.undo().unwrap());
    assert!(controller.redo().unwrap());
}

#[test]
fn retained_record_carries_revalidation_data() {
    // The proposal product is the GW-06 seam: base revision,
    // snapshot, normalized commands, preconditions, effects, and
    // the untouched candidate.
    let mut rig = fixture_rig();
    let result = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    op(
                        "synvelis.midi.note.move",
                        res_target("source", &source10()),
                        json!({"note": NoteId::new(20).to_tagged(), "position": {"domain": "source-native", "tick": 960}}),
                    ),
                    expected(1),
                ),
                human(),
            )
            .unwrap(),
    );
    let record = record_for(&rig, &result);
    assert_eq!(record.prepared.base.project, project());
    assert_eq!(record.prepared.base.revision, ProjectRevision(1));
    assert_eq!(record.prepared.commands.len(), 1);
    assert!(
        record
            .prepared
            .preconditions
            .iter()
            .any(|p| p.code == "midi-source-exists")
    );
    // Note move: Write on the note (range moved) + Write on the
    // source (payload), linked clips referenced.
    let note_write = effect(
        &record,
        EffectKind::Write,
        &ResourceRef::Note(NoteId::new(20)),
    );
    assert_eq!(
        note_write.after.as_ref().unwrap().range,
        Some(TypedRange {
            domain: TimeDomain::SourceNative,
            start: 960,
            len: 240,
        })
    );
    let src_write = effect(&record, EffectKind::Write, &source10());
    assert!(src_write.references.contains(&clip12()));
    assert!(src_write.references.contains(&clip13()));
    // The request identity rides the record for commit revalidation.
    assert_eq!(
        record.request.capability().id.as_str(),
        "synvelis.operation.prepare"
    );
}

// ── synvelis#548 strip-domain capabilities ───────────────────
//
// The CB-06 mask-gated strip ops are ordinary wire capabilities:
// they lower through the same op table, declare honest reads and
// preconditions, and prepare non-destructively against the pinned
// base. Mask/scope confinement itself stays `StripFieldsAdapter`'s
// authority — these tests prove the GW-04 prepare surface only.

/// The bootstrap master strip — discovered from project truth.
fn master_strip() -> StripId {
    let p = demo_project();
    p.routing.buses[&p.routing.master].strip
}

fn strip_target_json(strip: StripId) -> Value {
    json!({"shape": "resource", "kind": "strip", "id": strip.to_tagged()})
}

fn param_target_json(id: ParameterTargetId) -> Value {
    json!({"shape": "resource", "kind": "parameter-target", "id": id.to_tagged()})
}

fn processor_target_json(id: ProcessorId) -> Value {
    json!({"shape": "resource", "kind": "processor", "id": id.to_tagged()})
}

/// The master strip's seeded `Fader` channel `ParameterTarget`.
fn master_fader_target() -> ParameterTargetId {
    let p = demo_project();
    let strip = p.routing.buses[&p.routing.master].strip;
    p.control
        .targets
        .values()
        .find(|t| t.owner == ControlOwner::Strip(strip) && t.role == ParameterRole::Fader)
        .expect("master fader target exists")
        .id
}

#[test]
fn prepare_strip_bool_ops_lower_and_prepare() {
    // `synvelis.strip.set-mute`/`set-solo` lower to the channel bool
    // ops: the strip is the declared read, the candidate carries the
    // would-be channel state, and the committed project is
    // byte-identical afterwards.
    let mut rig = fixture_rig();
    let strip = master_strip();
    let before = rig.source.pin(project(), ProjectRevision(1)).unwrap();
    let before_json = serde_json::to_value(&*before).unwrap();

    for (capability, arg) in [
        ("synvelis.strip.set-mute", "muted"),
        ("synvelis.strip.set-solo", "soloed"),
    ] {
        let result = complete(
            rig.gw
                .handle(
                    &prepare_req(
                        None,
                        None,
                        op(capability, strip_target_json(strip), json!({arg: true})),
                        expected(1),
                    ),
                    human(),
                )
                .unwrap(),
        );
        let record = record_for(&rig, &result);
        assert_eq!(record.prepared.base.revision, ProjectRevision(1));
        // The strip is the declared internal read.
        effect(&record, EffectKind::Read, &ResourceRef::Strip(strip));
        // The candidate carries the would-be channel state — the
        // channel target's base flips to the asserted value.
        let role = if arg == "muted" {
            ParameterRole::Mute
        } else {
            ParameterRole::Solo
        };
        let target = record
            .prepared
            .candidate
            .control
            .targets
            .values()
            .find(|t| t.owner == ControlOwner::Strip(strip) && t.role == role)
            .expect("channel target exists in candidate");
        assert_eq!(target.base, 1.0);
    }
    // Zero mutation on the committed source.
    let after = rig.source.pin(project(), ProjectRevision(1)).unwrap();
    assert_eq!(serde_json::to_value(&*after).unwrap(), before_json);
}

#[test]
fn prepare_param_set_base_and_insert_processor_lower() {
    // `synvelis.param.set-base` writes an existing channel target;
    // `synvelis.strip.insert-processor` appends to the strip chain —
    // both prepare against the pinned base with the strip/target as
    // declared reads.
    let mut rig = fixture_rig();
    let strip = master_strip();
    let fader = master_fader_target();

    let result = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    op(
                        "synvelis.param.set-base",
                        param_target_json(fader),
                        json!({"value": 0.25}),
                    ),
                    expected(1),
                ),
                human(),
            )
            .unwrap(),
    );
    let record = record_for(&rig, &result);
    effect(
        &record,
        EffectKind::Read,
        &ResourceRef::ParameterTarget(fader),
    );
    assert_eq!(record.prepared.candidate.control.targets[&fader].base, 0.25);

    let result = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    op(
                        "synvelis.strip.insert-processor",
                        strip_target_json(strip),
                        json!({
                            "index": 0,
                            "name": "insert-fx",
                            "binding": {"binding": "Unresolved", "descriptor": "plug-demo"},
                            "sidechain_input": false,
                        }),
                    ),
                    expected(1),
                ),
                human(),
            )
            .unwrap(),
    );
    let record = record_for(&rig, &result);
    effect(&record, EffectKind::Read, &ResourceRef::Strip(strip));
    let p = &record.prepared.candidate;
    let chain =
        &p.routing.chains[&p.routing.strips[&p.routing.buses[&p.routing.master].strip].chain];
    assert_eq!(chain.processors.len(), 1);
    assert_eq!(
        rig.source.latest_revision(project()),
        Some(ProjectRevision(1))
    );
}

#[test]
fn prepare_strip_domain_args_fail_typed() {
    // Schema/type violations reject before any lowering; a binding
    // that is not a `ProcessorBinding` fails typed; a processor id
    // that does not exist fails the declared precondition path —
    // never a silent accept.
    let mut rig = fixture_rig();
    let strip = master_strip();
    for (capability, target, args) in [
        (
            "synvelis.strip.set-mute",
            strip_target_json(strip),
            json!({"muted": "yes"}),
        ),
        (
            "synvelis.strip.insert-processor",
            strip_target_json(strip),
            json!({
                "index": -1,
                "name": "fx",
                "binding": {"binding": "Unresolved", "descriptor": "plug-demo"},
                "sidechain_input": false,
            }),
        ),
        (
            "synvelis.strip.insert-processor",
            strip_target_json(strip),
            json!({
                "index": 0,
                "name": "fx",
                "binding": {"not": "a-binding"},
                "sidechain_input": false,
            }),
        ),
        (
            "synvelis.processor.set-binding",
            processor_target_json(ProcessorId::new(999_999)),
            json!({"binding": {"binding": "Unresolved", "descriptor": "plug-demo"}}),
        ),
    ] {
        let err = rig
            .gw
            .handle(
                &prepare_req(None, None, op(capability, target, args), expected(1)),
                human(),
            )
            .expect_err("invalid strip-domain args must reject");
        assert!(
            matches!(
                err.code,
                GatewayError::SchemaViolation { .. }
                    | GatewayError::MalformedRequest { .. }
                    | GatewayError::PreparationRejected { .. }
            ),
            "unexpected error for {capability}: {err:?}"
        );
    }
    assert_eq!(rig.engine.proposal_count(), 0);
}

// ── synvelis#561 — unobservable cascade fail-closed pins ─────
//
// `op_facts` must report `unsupported` whenever an op's apply
// cascade deletes entities no `ResourceKind` can name —
// `ModulationSource`/`ModulationAssignment`/`SmfDocument`, the same
// SEC-14 boundary `RemoveSend` established in synvelis#550. Each pin
// builds the cascade-carrying state through `ProjectController` ops
// (file-local fixtures — the shared `rig` fixture stays frozen),
// commits it as the pinned base, then asserts the wire op rejects
// `PreparationRejected { reason: "unverifiable …" }`. Companion
// positive pins prove the gate fires on the unobservable entities
// themselves, not on the op family.

/// Applies one domain op at the controller's live revision.
fn sv561_apply(c: &mut ProjectController, op: ProjectOp) {
    let rev = c.live_revision();
    c.apply(Command {
        expected_revision: rev,
        op,
    })
    .unwrap();
}

/// The one `SemanticPort` matching owner/direction/role — strip main
/// ports are seeded by `AddTrackStrip`, never hard-coded.
fn sv561_port(
    project: &synvelis_project::Project,
    owner: PortOwner,
    direction: PortDirection,
    role: PortRole,
) -> SemanticPortId {
    project
        .routing
        .ports
        .values()
        .find(|p| p.owner == owner && p.direction == direction && p.role == role)
        .expect("port exists")
        .id
}

/// Strip + send with `gain_target: None` on `c` — the shared prefix
/// of the send fixtures. Returns the send id discovered through
/// project truth.
fn sv561_strip_send(c: &mut ProjectController) -> synvelis_project::SendId {
    sv561_apply(
        c,
        ProjectOp::AddTrackStrip {
            track_id: TrackId::new(3),
        },
    );
    let (source, destination) = {
        let p = c.live();
        let strip = p
            .routing
            .strip_of_track(TrackId::new(3))
            .expect("track strip exists")
            .id;
        let master_strip = p.routing.buses[&p.routing.master].strip;
        (
            sv561_port(
                p,
                PortOwner::Strip(strip),
                PortDirection::Output,
                PortRole::Main,
            ),
            sv561_port(
                p,
                PortOwner::Strip(master_strip),
                PortDirection::Input,
                PortRole::Main,
            ),
        )
    };
    sv561_apply(
        c,
        ProjectOp::AddSend {
            source,
            destination,
            tap: SendTap::PostFader,
            gain_target: None,
        },
    );
    *c.live().routing.sends.keys().next().expect("send exists")
}

/// synvelis#561 send-gain fixture: `demo_project()` + a track strip +
/// one send whose `SendLevel` target is sealed through
/// `SetSendGainTarget`, plus a second adoptable target for the
/// rebind pin. With an assignment variant, a `Macro` source feeds a
/// `ModulationAssignment` touching the sealed target — the entity
/// `release_send_level_target` deletes on rebind/clear that no
/// `ResourceKind` can name.
/// Strip + send + sealed `SendLevel` target on `c` — the shared
/// prefix of the send-gain fixtures. Returns the send and the sealed
/// target ids discovered through project truth.
fn sv561_strip_send_sealed(
    c: &mut ProjectController,
) -> (synvelis_project::SendId, ParameterTargetId) {
    let send = sv561_strip_send(c);
    sv561_apply(
        c,
        ProjectOp::AddParameterTarget {
            name: "send-lvl".to_owned(),
            owner: ControlOwner::Send(send),
            role: ParameterRole::SendLevel,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    let sealed = c
        .live()
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlOwner::Send(send) && t.role == ParameterRole::SendLevel)
        .expect("send level target exists")
        .id;
    sv561_apply(
        c,
        ProjectOp::SetSendGainTarget {
            send_id: send,
            gain_target: Some(sealed),
        },
    );
    (send, sealed)
}

/// Which `ModulationAssignment` shape the send-gain fixture binds:
/// `OnTarget` aims `target` at the sealed target (the release's
/// `a.target == old` leg); `ViaDepthSource` aims it at the adoptable
/// rebind target while `depth_source` names the sealed one (the
/// `a.depth_source == Some(old)` leg).
#[derive(Clone, Copy)]
enum Sv561Assignment {
    None,
    OnTarget,
    ViaDepthSource,
}

fn sv561_send_gain_controller(assignment: Sv561Assignment) -> ProjectController {
    let mut c = ProjectController::new(demo_project(), Box::new(CounterIdGenerator::new(9_000)));
    let (_send, sealed) = sv561_strip_send_sealed(&mut c);
    // The rebind target: a Project-owned free-form target already
    // carrying the canonical send-level wire spec — apply re-owns it
    // on bind, so it needs no `Send` owner up front.
    sv561_apply(
        &mut c,
        ProjectOp::AddParameterTarget {
            name: "send-lvl-next".to_owned(),
            owner: ControlOwner::Project,
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    let next = c
        .live()
        .control
        .targets
        .values()
        .find(|t| t.name == "send-lvl-next")
        .expect("rebind target exists")
        .id;
    if !matches!(assignment, Sv561Assignment::None) {
        sv561_apply(
            &mut c,
            ProjectOp::AddModulationSource {
                owner: ControlOwner::Project,
                kind: SourceKind::Macro { value: 0.5 },
            },
        );
        let mod_source = *c
            .live()
            .control
            .sources
            .keys()
            .next()
            .expect("modulation source exists");
        let (target, depth_source) = match assignment {
            Sv561Assignment::OnTarget => (sealed, None),
            Sv561Assignment::ViaDepthSource => (next, Some(sealed)),
            Sv561Assignment::None => unreachable!(),
        };
        sv561_apply(
            &mut c,
            ProjectOp::AddModulationAssignment {
                source: mod_source,
                target,
                depth: 0.5,
                depth_source,
                polarity: Polarity::Bipolar,
                transform: Transform::Identity,
                enabled: true,
            },
        );
    }
    c
}

/// synvelis#561 composite fixture: a send with `gain_target: None`,
/// an adoptable Project-owned target carrying the canonical
/// send-level spec, and a `Macro`→target assignment. Inside one
/// `synvelis.operation.batch`, member 1 seals the target
/// (`SetSendGainTarget`) and member 2 releases it (clear) — the
/// release's assignment delete is only visible to facts evaluated
/// against the state member 1 leaves behind. Returns the send and
/// the adoptable target ids.
fn sv561_batch_seal_controller() -> (
    ProjectController,
    synvelis_project::SendId,
    ParameterTargetId,
) {
    let mut c = ProjectController::new(demo_project(), Box::new(CounterIdGenerator::new(9_000)));
    let send = sv561_strip_send(&mut c);
    sv561_apply(
        &mut c,
        ProjectOp::AddParameterTarget {
            name: "batch-lvl".to_owned(),
            owner: ControlOwner::Project,
            role: ParameterRole::Generic,
            time_scope: AutomationTimeScope::ProjectMusical,
            kind: ValueKind::Continuous,
            mapping: ParamMapping::Linear,
            capability: None,
            base: 0.5,
        },
    );
    let adoptable = c
        .live()
        .control
        .targets
        .values()
        .find(|t| t.name == "batch-lvl")
        .expect("adoptable target exists")
        .id;
    sv561_apply(
        &mut c,
        ProjectOp::AddModulationSource {
            owner: ControlOwner::Project,
            kind: SourceKind::Macro { value: 0.5 },
        },
    );
    let mod_source = *c
        .live()
        .control
        .sources
        .keys()
        .next()
        .expect("modulation source exists");
    sv561_apply(
        &mut c,
        ProjectOp::AddModulationAssignment {
            source: mod_source,
            target: adoptable,
            depth: 0.5,
            depth_source: None,
            polarity: Polarity::Bipolar,
            transform: Transform::Identity,
            enabled: true,
        },
    );
    (c, send, adoptable)
}

/// The send-gain fixture's single send — discovered through project
/// truth, never a hard-coded id.
fn sv561_send_id(project: &synvelis_project::Project) -> synvelis_project::SendId {
    *project
        .routing
        .sends
        .keys()
        .next()
        .expect("fixture send exists")
}

/// synvelis#561 track-remove fixture: `demo_project()` + a fresh
/// lane-free track + its strip, with the deterministic main route
/// removed so the teardown is oracle-clean — the pin isolates the
/// strip-owned control cascade. With `with_modulation`, a strip-owned
/// `Macro` source rides the cascade — a `ModulationSource` no
/// `ResourceKind` can name.
fn sv561_track_strip_controller(with_modulation: bool) -> ProjectController {
    let mut c = ProjectController::new(demo_project(), Box::new(CounterIdGenerator::new(9_000)));
    sv561_apply(
        &mut c,
        ProjectOp::AddTrack {
            name: "sv561".to_owned(),
        },
    );
    let track = *c
        .live()
        .tracks
        .keys()
        .find(|t| **t != TrackId::new(3))
        .expect("new track exists");
    sv561_apply(&mut c, ProjectOp::AddTrackStrip { track_id: track });
    let conn = *c
        .live()
        .routing
        .connections
        .keys()
        .next()
        .expect("main route exists");
    sv561_apply(
        &mut c,
        ProjectOp::RemoveConnection {
            connection_id: conn,
        },
    );
    if with_modulation {
        let strip = c
            .live()
            .routing
            .strip_of_track(track)
            .expect("track strip exists")
            .id;
        sv561_apply(
            &mut c,
            ProjectOp::AddModulationSource {
                owner: ControlOwner::Strip(strip),
                kind: SourceKind::Macro { value: 0.5 },
            },
        );
    }
    c
}

/// The track-strip fixture's strip-owned track — the only track
/// carrying a strip.
fn sv561_strip_track(project: &synvelis_project::Project) -> TrackId {
    *project
        .tracks
        .keys()
        .find(|t| project.routing.strip_of_track(**t).is_some())
        .expect("strip-owned track exists")
}

/// synvelis#561 SMF fixture: `demo_project()` + one `ImportMidiSmf`
/// document — an unreferenced MIDI source whose `smf_documents`
/// provenance entry `RemoveMidiSource` drops with it.
fn sv561_smf_document() -> SmfDocument {
    SmfDocument {
        format: SmfFormat::Format0,
        division: 480,
        tracks: vec![SmfTrack {
            events: vec![
                SmfEvent {
                    tick: MidiTick::new(0).unwrap(),
                    kind: SmfEventKind::NoteOn {
                        channel: 0,
                        key: 60,
                        velocity: 100,
                    },
                },
                SmfEvent {
                    tick: MidiTick::new(240).unwrap(),
                    kind: SmfEventKind::NoteOff {
                        channel: 0,
                        key: 60,
                        velocity: 0,
                    },
                },
                SmfEvent {
                    tick: MidiTick::new(480).unwrap(),
                    kind: SmfEventKind::EndOfTrack,
                },
            ],
        }],
    }
}

/// Commits `project` built by `controller` as the rig's pinned base
/// at the controller's live revision.
fn sv561_commit(rig: &mut Rig, controller: &ProjectController) -> u64 {
    let rev = controller.live_revision();
    rig.source.commit(project(), rev, controller.live().clone());
    rev.0
}

/// Asserts the prepare rejects with the unverifiable-effects verdict.
fn sv561_expect_unverifiable(err: ErrorEnvelope) {
    match err.code {
        GatewayError::PreparationRejected { reason } => assert!(
            reason.contains("unverifiable"),
            "expected unverifiable-effects denial, got {reason:?}"
        ),
        other => panic!("expected PreparationRejected, got {other:?}"),
    }
}

#[test]
fn sv561_set_gain_target_clear_unobservable_assignment_denied() {
    // `gain_target: null` releases the sealed `SendLevel` target; the
    // release cascade deletes the `ModulationAssignment` bound to it
    // — outside the observable model, so prepare must fail closed.
    let mut rig = fixture_rig();
    let c = sv561_send_gain_controller(Sv561Assignment::OnTarget);
    let p = c.live().clone();
    let send = sv561_send_id(&p);
    let rev = sv561_commit(&mut rig, &c);
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                op(
                    "synvelis.send.set-gain-target",
                    res_target("send", &ResourceRef::Send(send)),
                    json!({"gain_target": Value::Null}),
                ),
                expected(rev),
            ),
            human(),
        )
        .expect_err("set-gain-target clear with an unobservable cascade must reject");
    sv561_expect_unverifiable(err);
    assert_eq!(rig.engine.proposal_count(), 0);
}

#[test]
fn sv561_set_gain_target_rebind_unobservable_assignment_denied() {
    // Rebinding to a different target releases the old seal the same
    // way — the touching assignment is still an unobservable delete.
    let mut rig = fixture_rig();
    let c = sv561_send_gain_controller(Sv561Assignment::OnTarget);
    let p = c.live().clone();
    let send = sv561_send_id(&p);
    let next = p
        .control
        .targets
        .values()
        .find(|t| t.owner == ControlOwner::Project && t.role == ParameterRole::Generic)
        .expect("rebind target exists")
        .id;
    let rev = sv561_commit(&mut rig, &c);
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                op(
                    "synvelis.send.set-gain-target",
                    res_target("send", &ResourceRef::Send(send)),
                    json!({"gain_target": next.to_tagged()}),
                ),
                expected(rev),
            ),
            human(),
        )
        .expect_err("set-gain-target rebind with an unobservable cascade must reject");
    sv561_expect_unverifiable(err);
    assert_eq!(rig.engine.proposal_count(), 0);
}

#[test]
fn sv561_set_gain_target_clear_without_assignment_accepted() {
    // Positive control: the same sealed-target release with no
    // modulation entities touching it deletes only observable
    // entities — prepare must still accept (no over-denial).
    let mut rig = fixture_rig();
    let c = sv561_send_gain_controller(Sv561Assignment::None);
    let p = c.live().clone();
    let send = sv561_send_id(&p);
    let rev = sv561_commit(&mut rig, &c);
    let result = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    op(
                        "synvelis.send.set-gain-target",
                        res_target("send", &ResourceRef::Send(send)),
                        json!({"gain_target": Value::Null}),
                    ),
                    expected(rev),
                ),
                human(),
            )
            .unwrap(),
    );
    let record = record_for(&rig, &result);
    // The released target's delete surfaces in the observable diff.
    let deleted: Vec<_> = record
        .prepared
        .effects
        .iter()
        .filter(|e| e.kind == EffectKind::Delete)
        .map(|e| e.resource)
        .collect();
    assert!(
        deleted
            .iter()
            .any(|r| matches!(r, ResourceRef::ParameterTarget(_))),
        "expected the released target delete in effects, got {deleted:?}"
    );
}

#[test]
fn sv561_track_remove_unobservable_strip_cascade_denied() {
    // `track.remove` cascades `remove_strip_owned` →
    // `remove_control_owned(Strip)`: the strip-owned `Macro` source
    // dies with the strip — outside the observable model.
    let mut rig = fixture_rig();
    let c = sv561_track_strip_controller(true);
    let p = c.live().clone();
    let track = sv561_strip_track(&p);
    let rev = sv561_commit(&mut rig, &c);
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                op(
                    "synvelis.track.remove",
                    res_target("track", &ResourceRef::Track(track)),
                    json!({}),
                ),
                expected(rev),
            ),
            human(),
        )
        .expect_err("track.remove with an unobservable cascade must reject");
    sv561_expect_unverifiable(err);
    assert_eq!(rig.engine.proposal_count(), 0);
}

/// The fixture strip's id — discovered through project truth.
fn strip_of(project: &synvelis_project::Project) -> StripId {
    project
        .routing
        .strips
        .values()
        .find(|s| matches!(s.owner, synvelis_project::StripOwner::Track(_)))
        .expect("track strip exists")
        .id
}

#[test]
fn sv561_track_remove_plain_strip_accepted() {
    // Positive control: a strip teardown whose control closure holds
    // only observable entities (seeded channel targets) still
    // prepares — the gate keys on unobservable deletes, not strips.
    let mut rig = fixture_rig();
    let c = sv561_track_strip_controller(false);
    let p = c.live().clone();
    let track = sv561_strip_track(&p);
    let rev = sv561_commit(&mut rig, &c);
    let result = complete(
        rig.gw
            .handle(
                &prepare_req(
                    None,
                    None,
                    op(
                        "synvelis.track.remove",
                        res_target("track", &ResourceRef::Track(track)),
                        json!({}),
                    ),
                    expected(rev),
                ),
                human(),
            )
            .unwrap(),
    );
    let record = record_for(&rig, &result);
    effect(&record, EffectKind::Delete, &ResourceRef::Track(track));
    effect(
        &record,
        EffectKind::Delete,
        &ResourceRef::Strip(strip_of(&p)),
    );
}

#[test]
fn sv561_midi_source_remove_smf_document_denied() {
    // `midi.source.remove` drops the source's `smf_documents`
    // provenance entry unconditionally — `SmfDocument` has no
    // `ResourceKind`, so the delete is unverifiable and must fail
    // closed even though the source delete itself is observable.
    let mut rig = fixture_rig();
    let mut c = ProjectController::new(demo_project(), Box::new(CounterIdGenerator::new(9_000)));
    sv561_apply(
        &mut c,
        ProjectOp::ImportMidiSmf {
            document: sv561_smf_document(),
        },
    );
    let p = c.live().clone();
    let source = *p.smf_documents.keys().next().expect("smf document exists");
    let rev = sv561_commit(&mut rig, &c);
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                op(
                    "synvelis.midi.source.remove",
                    res_target("source", &ResourceRef::Source(source)),
                    json!({}),
                ),
                expected(rev),
            ),
            human(),
        )
        .expect_err("midi.source.remove with an smf document must reject");
    sv561_expect_unverifiable(err);
    assert_eq!(rig.engine.proposal_count(), 0);
}

#[test]
fn sv561_set_gain_target_clear_unobservable_depth_source_denied() {
    // The release predicate's second leg: an assignment that does
    // not target the sealed target but names it `depth_source`
    // dies in the same cascade — still outside the observable
    // model, so prepare must fail closed.
    let mut rig = fixture_rig();
    let c = sv561_send_gain_controller(Sv561Assignment::ViaDepthSource);
    let p = c.live().clone();
    let send = sv561_send_id(&p);
    let rev = sv561_commit(&mut rig, &c);
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                op(
                    "synvelis.send.set-gain-target",
                    res_target("send", &ResourceRef::Send(send)),
                    json!({"gain_target": Value::Null}),
                ),
                expected(rev),
            ),
            human(),
        )
        .expect_err("set-gain-target clear with a depth_source cascade must reject");
    sv561_expect_unverifiable(err);
    assert_eq!(rig.engine.proposal_count(), 0);
}

#[test]
fn sv561_send_remove_unobservable_assignment_denied() {
    // `send.remove` cascades `remove_control_owned(Send)`: the
    // sealed target's touching assignment dies even when the send
    // owns no `ModulationSource` — the closure's `target` leg of
    // the unverifiable check.
    let mut rig = fixture_rig();
    let c = sv561_send_gain_controller(Sv561Assignment::OnTarget);
    let p = c.live().clone();
    let send = sv561_send_id(&p);
    let rev = sv561_commit(&mut rig, &c);
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                op(
                    "synvelis.send.remove",
                    res_target("send", &ResourceRef::Send(send)),
                    json!({}),
                ),
                expected(rev),
            ),
            human(),
        )
        .expect_err("send.remove with an unobservable cascade must reject");
    sv561_expect_unverifiable(err);
    assert_eq!(rig.engine.proposal_count(), 0);
}

#[test]
fn sv561_track_remove_monitor_decl_denied() {
    // `track.remove` drops the track's `monitoring.tracks`
    // declaration unconditionally — `TrackMonitorDecl` has no
    // `ResourceKind` and `detail_of` never serializes it, the same
    // unverifiable-delete class as `SmfDocument`. The decl is
    // authored state with independent content, not internal
    // structure, so even a strip-less track must fail closed.
    let mut rig = fixture_rig();
    let mut c = ProjectController::new(demo_project(), Box::new(CounterIdGenerator::new(9_000)));
    sv561_apply(
        &mut c,
        ProjectOp::AddTrack {
            name: "sv561-monitor".to_owned(),
        },
    );
    let track = *c
        .live()
        .tracks
        .keys()
        .find(|t| **t != TrackId::new(3))
        .expect("new track exists");
    sv561_apply(
        &mut c,
        ProjectOp::SetTrackMonitor {
            track,
            decl: TrackMonitorDecl::default(),
        },
    );
    let rev = sv561_commit(&mut rig, &c);
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                op(
                    "synvelis.track.remove",
                    res_target("track", &ResourceRef::Track(track)),
                    json!({}),
                ),
                expected(rev),
            ),
            human(),
        )
        .expect_err("track.remove with a monitor declaration must reject");
    sv561_expect_unverifiable(err);
    assert_eq!(rig.engine.proposal_count(), 0);
}

#[test]
fn sv561_batch_seal_release_unobservable_denied() {
    // `synvelis.operation.batch` members commit sequentially, so
    // member facts must read the state earlier members leave
    // behind: member 1 seals the adoptable target as this send's
    // `SendLevel`, member 2 clears it — releasing a seal that only
    // exists mid-batch and deleting the touching assignment.
    // Evaluating member 2 against the pinned base sees
    // `gain_target: None` and under-reports the cascade.
    let mut rig = fixture_rig();
    let (c, send, adoptable) = sv561_batch_seal_controller();
    let rev = sv561_commit(&mut rig, &c);
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                op(
                    "synvelis.operation.batch",
                    Value::Null,
                    json!({"ops": [
                        op(
                            "synvelis.send.set-gain-target",
                            res_target("send", &ResourceRef::Send(send)),
                            json!({"gain_target": adoptable.to_tagged()}),
                        ),
                        op(
                            "synvelis.send.set-gain-target",
                            res_target("send", &ResourceRef::Send(send)),
                            json!({"gain_target": Value::Null}),
                        ),
                    ]}),
                ),
                expected(rev),
            ),
            human(),
        )
        .expect_err("batch seal-then-release with an unobservable cascade must reject");
    sv561_expect_unverifiable(err);
    assert_eq!(rig.engine.proposal_count(), 0);
}
