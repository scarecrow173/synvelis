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
    ClipId, Command, CounterIdGenerator, NoteId, ProjectController, ProjectRevision, SourceId,
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
        record.op,
        synvelis_project::ProjectOp::Composite { .. }
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
        op: record.op.clone(),
    });
    assert!(matches!(
        stale,
        Err(synvelis_project::Error::StaleRevision { .. })
    ));
    // The prepared op applies verbatim, records one undo step.
    let rev = controller
        .apply(Command {
            expected_revision: record.prepared.base.revision,
            op: record.op.clone(),
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
