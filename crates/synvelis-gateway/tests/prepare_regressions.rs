//! GW-04 review-regression tests — defects found by the independent
//! review pass (issue #56): emission under-classification, existence
//! blinding, batch schema/floor gaps, strict target keys, project-parent
//! identity, and version taxonomy.

mod rig;

use std::collections::BTreeSet;

use rig::*;
use serde_json::{Value, json};
use synvelis_gateway::*;
use synvelis_project::ClipId;

// ══ Emission class honesty ══════════════════════════════════

#[test]
fn midi_source_read_discloses_structured_content_not_metadata() {
    // Review P1: `clip.add` reads the whole `MidiSource` (notes
    // included) into a Read effect — that payload is
    // `StructuredContentRead`, not metadata. A grant without the
    // class must deny at the emission boundary.
    let mk = |classes: &[DataAccessClass]| {
        prep_grant(
            "g1",
            "t1",
            &["synvelis.clip.add"],
            ResourceScope::Exact {
                resources: BTreeSet::from([lane4(), source10()]),
            },
            Some(CreateWithin {
                parents: OwnerConstraint::OneOf {
                    refs: BTreeSet::from([lane4()]),
                },
                kinds: BTreeSet::from([ResourceKind::Clip]),
                placement: None,
            }),
            classes,
        )
    };
    let req = || {
        prepare_req(
            Some("t1"),
            Some("g1"),
            op(
                "synvelis.clip.add",
                create_target("lane", &lane4(), "clip"),
                json!({
                    "source": tagged(&source10()),
                    "placement": {"anchor": {"kind": "Musical", "value": 0}, "extent": {"kind": "Musical", "value": 240}},
                    "playback": {"source_region": {"start": 0, "end": 480}, "mapping": {"kind": "Natural"}},
                }),
            ),
            expected(1),
        )
    };
    // Without StructuredContentRead the source's note payload is
    // withheld — the emission boundary denies.
    let mut rig = fixture_rig();
    issue_agent(&rig.store, "t1", mk(&mutate_classes()));
    let denied = rig
        .gw
        .handle(&req(), agent())
        .expect_err("metadata-only grant must not receive note content");
    assert!(matches!(denied.code, GatewayError::Denied { .. }));

    // With the class granted, the same preparation completes.
    let mut rig = fixture_rig();
    issue_agent(&rig.store, "t1", mk(&content_classes()));
    complete(
        rig.gw
            .handle(&req(), agent())
            .expect("content-read grant must pass emission"),
    );
}

// ══ Existence blinding ══════════════════════════════════════

#[test]
fn absent_and_uncovered_targets_deny_identically() {
    // Review P2: the provisional scope gate runs before domain
    // validation — "does not exist" and "exists outside the grant"
    // collapse into one verdict (SEC-03).
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
    let req_for = |clip: ResourceRef| {
        prepare_req(
            Some("t1"),
            Some("g1"),
            op(
                "synvelis.clip.mute",
                res_target("clip", &clip),
                json!({"muted": true}),
            ),
            expected(1),
        )
    };
    let absent = rig
        .gw
        .handle(&req_for(ResourceRef::Clip(ClipId::new(999))), agent())
        .expect_err("absent target denies");
    let uncovered = rig
        .gw
        .handle(&req_for(clip12()), agent())
        .expect_err("uncovered target denies");
    // One shared verdict — the agent cannot distinguish the two.
    let absent_reason = match &absent.code {
        GatewayError::Denied { reason, .. } => reason.clone(),
        other => panic!("expected Denied, got {other:?}"),
    };
    let uncovered_reason = match &uncovered.code {
        GatewayError::Denied { reason, .. } => reason.clone(),
        other => panic!("expected Denied, got {other:?}"),
    };
    assert_eq!(absent_reason, uncovered_reason);
}

// ══ Batch contract ══════════════════════════════════════════

#[test]
fn batch_args_are_schema_validated() {
    // Review P3: `deny_unknown` on the batch schema must actually run.
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
                    json!({"ops": [{
                        "capability": {"id": "synvelis.clip.mute", "version": {"major": 1, "minor": 0}},
                        "target": res_target("clip", &clip11()), "args": {"muted": true}
                    }], "injected": true}),
                ),
                expected(1),
            ),
            human(),
        )
        .expect_err("unknown batch arg must fail");
    assert!(matches!(err.code, GatewayError::SchemaViolation { .. }));
}

#[test]
fn batch_capability_itself_must_be_granted() {
    // Review P3: the executed capability — the batch — is part of
    // the capability floor, not just its members.
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
    let err = rig
        .gw
        .handle(
            &prepare_req(
                Some("t1"),
                Some("g1"),
                op(
                    "synvelis.operation.batch",
                    Value::Null,
                    json!({"ops": [{
                        "capability": {"id": "synvelis.clip.mute", "version": {"major": 1, "minor": 0}},
                        "target": res_target("clip", &clip11()), "args": {"muted": true}
                    }]}),
                ),
                expected(1),
            ),
            agent(),
        )
        .expect_err("ungranted batch capability must deny");
    match err.code {
        GatewayError::Denied { reason, .. } => {
            assert_eq!(reason.unwrap(), "capability outside grant");
        }
        other => panic!("expected Denied, got {other:?}"),
    }
}

// ══ Strict inner target ═════════════════════════════════════

#[test]
fn inner_target_rejects_unknown_keys() {
    // serde's deny_unknown_fields is inert on internally tagged
    // enums — the explicit key scan covers the inner target the
    // same way the outer wire layer does.
    let mut rig = fixture_rig();
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                op(
                    "synvelis.clip.mute",
                    json!({"shape": "resource", "kind": "clip", "id": tagged(&clip11()), "bogus": 1}),
                    json!({"muted": true}),
                ),
                expected(1),
            ),
            human(),
        )
        .expect_err("unknown target key must fail");
    assert!(matches!(
        err.code,
        GatewayError::MalformedRequest { .. } | GatewayError::SchemaViolation { .. }
    ));
}

// ══ Project-parent identity ═════════════════════════════════

#[test]
fn project_parent_must_be_the_request_project() {
    // A create's declared parent is semantic: naming a different
    // project is a caller error, not a silent retarget.
    let mut rig = fixture_rig();
    let foreign = synvelis_project::ProjectId::new(999);
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                op(
                    "synvelis.track.create",
                    json!({
                        "shape": "create",
                        "parent_kind": "project",
                        "parent": foreign.to_tagged(),
                        "creates": "track",
                    }),
                    json!({"name": "t4"}),
                ),
                expected(1),
            ),
            human(),
        )
        .expect_err("foreign project parent must fail");
    assert!(matches!(err.code, GatewayError::PreparationRejected { .. }));
}

// ══ Version taxonomy ════════════════════════════════════════

#[test]
fn known_id_wrong_version_reports_version_not_unknown() {
    let mut rig = fixture_rig();
    let err = rig
        .gw
        .handle(
            &prepare_req(
                None,
                None,
                json!({
                    "capability": {"id": "synvelis.clip.mute", "version": {"major": 2, "minor": 0}},
                    "target": res_target("clip", &clip11()),
                    "args": {"muted": true},
                }),
                expected(1),
            ),
            human(),
        )
        .expect_err("unsupported version must fail");
    assert!(matches!(
        err.code,
        GatewayError::UnsupportedCapabilityVersion { .. }
    ));
}
