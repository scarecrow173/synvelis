//! Runtime binding tests: typed call assembly, wire compatibility,
//! fail-closed verdicts, target/consistency/schema validation, i64
//! decode, and the no-authority boundary.

mod common;

use common::*;
use serde_json::{Value, json};
use synvelis_gateway::{GatewayError, ResourceKind};
use synvelis_script::{
    BindingSet, InvocableBlocker, InvocationCall, ScriptApiError, ScriptConsistency, ScriptTarget,
    ScriptVerdict,
};

fn model() -> synvelis_script::ScriptApiModel {
    sample_model()
}

fn gateway_err(err: ScriptApiError) -> GatewayError {
    match err {
        ScriptApiError::Gateway(g) => g,
        other => panic!("expected Gateway error, got {other:?}"),
    }
}

#[test]
fn assemble_add_note_produces_wire_compatible_draft() {
    let draft = model().assemble_call("addNote", add_note_call()).unwrap();

    assert_eq!(draft.request_id, "req-fixture-0001");
    assert_eq!(draft.capability.id.as_str(), "synvelis.midi.add-note");
    assert_eq!(draft.capability.version, cver(1, 0));
    assert_eq!(draft.schema.name, "midi.add-note.input");
    assert_eq!(draft.project, project_id());
    assert_eq!(draft.idempotency_key.as_deref(), Some("idem-fixture-1"));

    // Target serializes to the wire shape.
    let target = draft.target.as_ref().unwrap();
    assert_eq!(target["shape"], json!("resource"));
    assert_eq!(target["kind"], json!("track"));
    assert_eq!(target["id"], json!(track_id()));

    // Consistency serializes to the wire shape.
    assert_eq!(
        draft.consistency,
        json!({"kind": "expected-revision", "revision": 7})
    );

    // i64-string decode: sample_hint text → JSON integer i64::MAX;
    // TimeRange start/len text → numbers.
    assert_eq!(draft.args["sample_hint"], json!(i64::MAX));
    assert_eq!(draft.args["position"]["start"], json!(960));
    assert_eq!(draft.args["position"]["len"], json!(480));
    // Untouched fields pass through.
    assert_eq!(draft.args["pitch"], json!(60));
    assert_eq!(draft.args["owner"], json!(track_id()));

    // The draft passes the authoritative wire boundary unchanged.
    let wire = draft
        .verify_wire()
        .expect("draft must parse as WireRequest");
    assert_eq!(wire.request_id.as_str(), "req-fixture-0001");
    assert_eq!(wire.project, project_id());
}

#[test]
fn assemble_play_has_no_target_and_unconditional() {
    let draft = model().assemble_call("play", play_call()).unwrap();
    assert!(draft.target.is_none());
    assert_eq!(draft.consistency, json!({"kind": "unconditional"}));
    // Empty-object input schema validates `{}`.
    assert_eq!(draft.args, json!({}));
    draft.verify_wire().unwrap();
}

#[test]
fn assemble_track_info_accepts_query_consistency() {
    for consistency in [
        ScriptConsistency::Latest,
        ScriptConsistency::ReadAt { revision: 42 },
        ScriptConsistency::FixedSnapshot {
            snapshot: "snap-abc123".to_owned(),
        },
    ] {
        let mut call = track_info_call();
        call.consistency = consistency;
        model().assemble_call("trackInfo", call).unwrap();
    }
}

#[test]
fn assemble_create_clip_injects_declared_creates_kind() {
    let call = InvocationCall {
        args: json!({
            "name": "hook",
            "start": {"domain": "media", "start": "0", "len": "96000"},
        }),
        target: Some(ScriptTarget::Create {
            parent_kind: ResourceKind::Track,
            parent: track_id(),
        }),
        consistency: ScriptConsistency::ExpectedRevision { revision: 3 },
        project: project_id(),
        request_id: "req-create-1".to_owned(),
        task: None,
        grant: None,
        idempotency_key: None,
    };
    let draft = model().assemble_call("createClip", call).unwrap();
    let target = draft.target.as_ref().unwrap();
    assert_eq!(target["shape"], json!("create"));
    assert_eq!(target["parent_kind"], json!("track"));
    assert_eq!(target["parent"], json!(track_id()));
    // The contract-fixed `creates` kind is injected — the script cannot
    // name a different created kind.
    assert_eq!(target["creates"], json!("clip"));
    draft.verify_wire().unwrap();
}

#[test]
fn assemble_move_clips_resource_set() {
    let call = InvocationCall {
        args: json!({"delta": "-480"}),
        target: Some(ScriptTarget::ResourceSet {
            kind: ResourceKind::Clip,
            ids: vec![clip_id(), tagged_id("clip", 'e')],
        }),
        consistency: ScriptConsistency::ExpectedRevision { revision: 9 },
        project: project_id(),
        request_id: "req-move-1".to_owned(),
        task: None,
        grant: None,
        idempotency_key: None,
    };
    let draft = model().assemble_call("moveClips", call).unwrap();
    // delta was an i64 string → decoded to number.
    assert_eq!(draft.args["delta"], json!(-480));
    let target = draft.target.as_ref().unwrap();
    assert_eq!(target["shape"], json!("resource-set"));
    assert_eq!(target["kind"], json!("clip"));
    assert_eq!(target["ids"].as_array().unwrap().len(), 2);
    draft.verify_wire().unwrap();
}

#[test]
fn unknown_api_name_is_typed() {
    let err = model()
        .assemble_call("noSuchApi", play_call())
        .expect_err("unknown name");
    assert!(matches!(err, ScriptApiError::UnknownApi { .. }));
}

#[test]
fn non_invocable_entries_reject_with_typed_blockers() {
    type VerdictCheck = fn(&ScriptVerdict) -> bool;
    type BlockerCheck = fn(&InvocableBlocker) -> bool;
    let m = model();
    let cases: &[(&str, VerdictCheck, BlockerCheck)] = &[
        // Hidden visibility → Hidden blocker.
        (
            "hiddenOp",
            |_| true,
            |b| matches!(b, InvocableBlocker::Hidden),
        ),
        (
            "legacyOp",
            |v| matches!(v, ScriptVerdict::Disabled { .. }),
            |b| matches!(b, InvocableBlocker::Disabled { .. }),
        ),
        (
            "futureOp",
            |v| *v == ScriptVerdict::Planned,
            |b| matches!(b, InvocableBlocker::Planned),
        ),
        (
            "oldOp",
            |v| *v == ScriptVerdict::Retired,
            |b| matches!(b, InvocableBlocker::Retired),
        ),
        (
            "newThing",
            |v| *v == ScriptVerdict::UnknownCapability,
            |b| matches!(b, InvocableBlocker::UnknownCapability),
        ),
        (
            "wrongVer",
            |v| matches!(v, ScriptVerdict::UnsupportedVersion { .. }),
            |b| matches!(b, InvocableBlocker::UnsupportedVersion { .. }),
        ),
        (
            "bad name!",
            |v| matches!(v, ScriptVerdict::InvalidName { .. }),
            |b| matches!(b, InvocableBlocker::InvalidName { .. }),
        ),
        (
            "end",
            |v| matches!(v, ScriptVerdict::InvalidName { .. }),
            |b| matches!(b, InvocableBlocker::InvalidName { .. }),
        ),
    ];
    for &(name, verdict_check, blocker_check) in cases {
        let entry = m.describe(name).unwrap();
        assert!(verdict_check(&entry.verdict), "{name} verdict");
        let err = m
            .assemble_call(name, play_call())
            .expect_err("non-invocable must reject");
        match err {
            ScriptApiError::NotInvocable { name: n, blocker } => {
                assert_eq!(n, name);
                assert!(blocker_check(&blocker), "{name} blocker {blocker:?}");
            }
            other => panic!("expected NotInvocable for {name}, got {other:?}"),
        }
    }
}

#[test]
fn target_contract_violations_are_typed() {
    let m = model();

    // Missing required target.
    let mut call = add_note_call();
    call.target = None;
    let err = gateway_err(m.assemble_call("addNote", call).unwrap_err());
    assert!(matches!(err, GatewayError::TargetRequired));

    // Target on a None contract.
    let mut call = play_call();
    call.target = Some(ScriptTarget::Resource {
        kind: ResourceKind::Track,
        id: track_id(),
    });
    let err = gateway_err(m.assemble_call("play", call).unwrap_err());
    assert!(matches!(err, GatewayError::UnexpectedTarget));

    // Wrong resource kind.
    let mut call = add_note_call();
    call.target = Some(ScriptTarget::Resource {
        kind: ResourceKind::Clip,
        id: clip_id(),
    });
    let err = gateway_err(m.assemble_call("addNote", call).unwrap_err());
    assert!(matches!(err, GatewayError::WrongResourceKind { .. }));

    // Wrong target shape (resource-set where resource required).
    let mut call = add_note_call();
    call.target = Some(ScriptTarget::ResourceSet {
        kind: ResourceKind::Track,
        ids: vec![track_id()],
    });
    let err = gateway_err(m.assemble_call("addNote", call).unwrap_err());
    assert!(matches!(err, GatewayError::TargetShapeMismatch { .. }));

    // Malformed tagged id.
    let mut call = add_note_call();
    call.target = Some(ScriptTarget::Resource {
        kind: ResourceKind::Track,
        id: "track-not-a-valid-id".to_owned(),
    });
    let err = gateway_err(m.assemble_call("addNote", call).unwrap_err());
    assert!(matches!(err, GatewayError::InvalidId { .. }));

    // Id tag disagrees with declared kind (clip id where track required —
    // kind matches but tag doesn't... actually kind IS clip here so it's
    // WrongResourceKind; use a track-kind field with a clip-tagged id via
    // args owner field instead — covered in schema tests).

    // Empty resource-set.
    let call = InvocationCall {
        args: json!({"delta": 0}),
        target: Some(ScriptTarget::ResourceSet {
            kind: ResourceKind::Clip,
            ids: vec![],
        }),
        consistency: ScriptConsistency::ExpectedRevision { revision: 1 },
        project: project_id(),
        request_id: "req-empty-set".to_owned(),
        task: None,
        grant: None,
        idempotency_key: None,
    };
    let err = gateway_err(m.assemble_call("moveClips", call.clone()).unwrap_err());
    assert!(matches!(err, GatewayError::SchemaViolation { .. }));

    // Malformed project id.
    let mut call = play_call();
    call.project = "not-a-project".to_owned();
    let err = gateway_err(m.assemble_call("play", call).unwrap_err());
    assert!(matches!(err, GatewayError::InvalidId { .. }));
}

#[test]
fn consistency_legality_is_domain_typed() {
    let m = model();

    // Mutation requires ExpectedRevision — Latest is rejected.
    let mut call = add_note_call();
    call.consistency = ScriptConsistency::Latest;
    let err = gateway_err(m.assemble_call("addNote", call).unwrap_err());
    assert!(matches!(err, GatewayError::ConsistencyNotAllowed { .. }));

    // Query rejects mutation semantics.
    let mut call = track_info_call();
    call.consistency = ScriptConsistency::ExpectedRevision { revision: 1 };
    let err = gateway_err(m.assemble_call("trackInfo", call).unwrap_err());
    assert!(matches!(err, GatewayError::ConsistencyNotAllowed { .. }));

    // RuntimeControl requires Unconditional.
    let mut call = play_call();
    call.consistency = ScriptConsistency::Latest;
    let err = gateway_err(m.assemble_call("play", call).unwrap_err());
    assert!(matches!(err, GatewayError::ConsistencyNotAllowed { .. }));
}

#[test]
fn argument_validation_is_typed() {
    let m = model();
    let base_args = || {
        json!({
            "position": {"domain": "project-musical", "start": "960", "len": "480"},
            "pitch": 60,
            "curve": "exp",
            "gain_db": null,
        })
    };
    let call_with = |args: Value| InvocationCall {
        args,
        target: Some(ScriptTarget::Resource {
            kind: ResourceKind::Track,
            id: track_id(),
        }),
        consistency: ScriptConsistency::ExpectedRevision { revision: 1 },
        project: project_id(),
        request_id: "req-args".to_owned(),
        task: None,
        grant: None,
        idempotency_key: None,
    };

    // Happy path minimal.
    m.assemble_call("addNote", call_with(base_args())).unwrap();

    // Wrong primitive type.
    let mut args = base_args();
    args["pitch"] = json!("high");
    let err = gateway_err(m.assemble_call("addNote", call_with(args)).unwrap_err());
    assert!(matches!(err, GatewayError::SchemaViolation { .. }));

    // Integer out of declared bounds.
    let mut args = base_args();
    args["pitch"] = json!(200);
    let err = gateway_err(m.assemble_call("addNote", call_with(args)).unwrap_err());
    assert!(matches!(err, GatewayError::SchemaViolation { .. }));

    // Missing required field.
    let mut args = base_args();
    args.as_object_mut().unwrap().remove("position");
    let err = gateway_err(m.assemble_call("addNote", call_with(args)).unwrap_err());
    assert!(matches!(err, GatewayError::SchemaViolation { .. }));

    // Required Optional field present-but-wrong-type.
    let mut args = base_args();
    args["gain_db"] = json!("loud");
    let err = gateway_err(m.assemble_call("addNote", call_with(args)).unwrap_err());
    assert!(matches!(err, GatewayError::SchemaViolation { .. }));

    // Null where a non-Optional field is concerned (velocity is
    // optional-but-not-nullable).
    let mut args = base_args();
    args["velocity"] = Value::Null;
    let err = gateway_err(m.assemble_call("addNote", call_with(args)).unwrap_err());
    assert!(matches!(err, GatewayError::SchemaViolation { .. }));

    // Unknown top-level field (deny_unknown).
    let mut args = base_args();
    args["surprise"] = json!(1);
    let err = gateway_err(m.assemble_call("addNote", call_with(args)).unwrap_err());
    assert!(matches!(err, GatewayError::SchemaViolation { .. }));

    // Unknown nested field inside deny_unknown object.
    let mut args = base_args();
    args["meta"] = json!({"inner": true, "extra": 1});
    let err = gateway_err(m.assemble_call("addNote", call_with(args)).unwrap_err());
    assert!(matches!(err, GatewayError::SchemaViolation { .. }));

    // Id field with wrong-kind tagged id → typed WrongResourceKind.
    let mut args = base_args();
    args["owner"] = json!(clip_id()); // owner requires Track kind
    let err = gateway_err(m.assemble_call("addNote", call_with(args)).unwrap_err());
    assert!(matches!(err, GatewayError::WrongResourceKind { .. }));

    // Id field with malformed id → InvalidId.
    let mut args = base_args();
    args["owner"] = json!("track-bad");
    let err = gateway_err(m.assemble_call("addNote", call_with(args)).unwrap_err());
    assert!(matches!(err, GatewayError::InvalidId { .. }));

    // TimeRange wrong domain → typed WrongTimeDomain.
    let mut args = base_args();
    args["position"] = json!({"domain": "media", "start": "0", "len": "1"});
    let err = gateway_err(m.assemble_call("addNote", call_with(args)).unwrap_err());
    assert!(matches!(err, GatewayError::WrongTimeDomain { .. }));

    // TimeRange non-positive len → SchemaViolation.
    let mut args = base_args();
    args["position"] = json!({"domain": "project-musical", "start": "0", "len": "0"});
    let err = gateway_err(m.assemble_call("addNote", call_with(args)).unwrap_err());
    assert!(matches!(err, GatewayError::SchemaViolation { .. }));

    // Enum value outside the set.
    let mut args = base_args();
    args["curve"] = json!("bogus");
    let err = gateway_err(m.assemble_call("addNote", call_with(args)).unwrap_err());
    assert!(matches!(err, GatewayError::SchemaViolation { .. }));
}

#[test]
fn i64_string_decode_is_strict() {
    let m = model();
    let call_with_hint = |hint: Value| InvocationCall {
        args: json!({
            "position": {"domain": "project-musical", "start": "0", "len": "1"},
            "pitch": 60,
            "curve": "exp",
            "gain_db": null,
            "sample_hint": hint,
        }),
        target: Some(ScriptTarget::Resource {
            kind: ResourceKind::Track,
            id: track_id(),
        }),
        consistency: ScriptConsistency::ExpectedRevision { revision: 1 },
        project: project_id(),
        request_id: "req-i64".to_owned(),
        task: None,
        grant: None,
        idempotency_key: None,
    };

    // i64::MAX as decimal text decodes exactly.
    let draft = m
        .assemble_call("addNote", call_with_hint(json!("9223372036854775807")))
        .unwrap();
    assert_eq!(draft.args["sample_hint"], json!(i64::MAX));

    // i64::MIN decodes exactly.
    let draft = m
        .assemble_call("addNote", call_with_hint(json!("-9223372036854775808")))
        .unwrap();
    assert_eq!(draft.args["sample_hint"], json!(i64::MIN));

    // Overflow → typed rejection, never wrapped/rounded.
    let err = gateway_err(
        m.assemble_call("addNote", call_with_hint(json!("9223372036854775808")))
            .unwrap_err(),
    );
    assert!(matches!(err, GatewayError::SchemaViolation { .. }));

    // Non-numeric text → typed rejection.
    let err = gateway_err(
        m.assemble_call("addNote", call_with_hint(json!("12.5")))
            .unwrap_err(),
    );
    assert!(matches!(err, GatewayError::SchemaViolation { .. }));
    let err = gateway_err(
        m.assemble_call("addNote", call_with_hint(json!("abc")))
            .unwrap_err(),
    );
    assert!(matches!(err, GatewayError::SchemaViolation { .. }));

    // In-range integer as a JSON number still works.
    let draft = m
        .assemble_call("addNote", call_with_hint(json!(42)))
        .unwrap();
    assert_eq!(draft.args["sample_hint"], json!(42));
}

#[test]
fn draft_carries_no_authority_and_wire_rejects_it() {
    let draft = model().assemble_call("addNote", add_note_call()).unwrap();
    let wire = draft.to_wire_value();
    let obj = wire.as_object().unwrap();

    // Only wire-contract keys are present.
    for key in obj.keys() {
        assert!(
            [
                "request_id",
                "capability",
                "schema",
                "project",
                "target",
                "args",
                "consistency",
                "task",
                "grant",
                "idempotency_key",
            ]
            .contains(&key.as_str()),
            "unexpected draft key {key}"
        );
    }
    for forbidden in [
        "principal",
        "caller",
        "caller_identity",
        "authority",
        "token",
        "permissions",
        "approval",
        "adapter_seal",
        "seal",
    ] {
        assert!(!obj.contains_key(forbidden), "draft carries {forbidden}");
    }

    // If a caller tries to smuggle authority onto the wire shape, the
    // authoritative parser rejects it — the draft boundary is honest.
    let mut tampered = wire.clone();
    tampered
        .as_object_mut()
        .unwrap()
        .insert("principal".to_owned(), json!("admin"));
    let err = synvelis_gateway::WireRequest::parse(&tampered).unwrap_err();
    assert!(matches!(err, GatewayError::SelfDeclaredAuthority { .. }));
}

#[test]
fn binding_set_generation_is_model_pinned() {
    let bindings = BindingSet::generate(&model()).unwrap();
    assert_eq!(bindings.entries.len(), 6);
    assert_eq!(bindings.model_digest, model().model_digest);
    assert_eq!(
        bindings.schema_version,
        synvelis_script::BINDING_SCHEMA_VERSION
    );

    // Lookup by name; non-invocable names absent.
    assert!(bindings.get("addNote").is_some());
    assert!(bindings.get("hiddenOp").is_none());
    assert!(bindings.get("legacyOp").is_none());
    assert!(bindings.get("nope").is_none());

    // Entries carry resolved schemas + declared refs.
    let add = bindings.get("addNote").unwrap();
    assert_eq!(
        add.domain,
        synvelis_gateway::OperationDomain::ProjectMutation
    );
    assert_eq!(add.input_schema.name, "midi.add-note.input");
    assert!(matches!(
        add.target,
        synvelis_gateway::TargetContract::Resource { .. }
    ));
}
