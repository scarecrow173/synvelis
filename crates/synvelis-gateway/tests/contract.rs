//! GW-01 contract tests — Issue #53 acceptance criteria and the
//! GT-001/002/003/010 equivalents from #45.
//!
//! Each test names the invariant it proves. Rejections assert the
//! *typed* error, its derived category, and its retry advice — a
//! contract only exists if every failure is classifiable.

use serde_json::{Value, json};
use synvelis_gateway::fixture::{self, FixtureAdapter, PermitSetPolicy};
use synvelis_gateway::*;
use synvelis_project::{ProjectId, SourceId};

fn adapter() -> FixtureAdapter {
    FixtureAdapter::for_testing()
}

fn agent() -> CallerIdentity {
    adapter().agent("mcp-client-1").unwrap()
}

fn all_permissions() -> PermissionSet {
    PermissionSet::of([
        PermissionKind::ReadContext,
        PermissionKind::DiscloseContext,
        PermissionKind::MutateProject,
        PermissionKind::ControlTransport,
        PermissionKind::RunJob,
        PermissionKind::ExternalAction,
    ])
}

/// Fixture gateway with a policy that permits everything — isolates
/// bind/validation behavior from authorization.
fn permissive_gateway() -> Gateway {
    let mut gw = Gateway::with_evaluator(Box::new(PermitSetPolicy::new(all_permissions())));
    fixture::install(&mut gw).unwrap();
    gw
}

/// Fixture gateway with the fail-closed default evaluator.
fn closed_gateway() -> Gateway {
    let mut gw = Gateway::new();
    fixture::install(&mut gw).unwrap();
    gw
}

fn midi_add_note_req(revision: u64) -> Value {
    json!({
        "request_id": "req-m1",
        "capability": {"id": "synvelis.midi.add-note", "version": {"major": 1, "minor": 1}},
        "schema": {"name": "synvelis.midi.add-note.input", "version": {"major": 1, "minor": 1}},
        "project": ProjectId::new(7).to_tagged(),
        "target": {"shape": "resource", "kind": "source", "id": SourceId::new(9).to_tagged()},
        "args": {
            "key": 60,
            "velocity": 100,
            "position": {"domain": "source-native", "start": 240, "len": 480}
        },
        "consistency": {"kind": "expected-revision", "revision": revision},
    })
}

fn context_query_req() -> Value {
    json!({
        "request_id": "req-q1",
        "capability": {"id": "synvelis.context.source-notes", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "synvelis.context.source-notes.input", "version": {"major": 1, "minor": 0}},
        "project": ProjectId::new(7).to_tagged(),
        "target": {"shape": "resource", "kind": "source", "id": SourceId::new(9).to_tagged()},
        "args": {"range": {"domain": "source-native", "start": 0, "len": 960}},
        "consistency": {"kind": "latest"},
    })
}

fn transport_req() -> Value {
    json!({
        "request_id": "req-t1",
        "capability": {"id": "synvelis.transport.set-position", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "synvelis.transport.set-position.input", "version": {"major": 1, "minor": 0}},
        "project": ProjectId::new(7).to_tagged(),
        "args": {"action": "play"},
        "consistency": {"kind": "unconditional"},
    })
}

fn job_req() -> Value {
    json!({
        "request_id": "req-j1",
        "capability": {"id": "synvelis.job.render-export", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "synvelis.job.render-export.input", "version": {"major": 1, "minor": 0}},
        "project": ProjectId::new(7).to_tagged(),
        "args": {"format": "wav"},
        "consistency": {"kind": "unconditional"},
    })
}

fn create_source_req(revision: u64) -> Value {
    json!({
        "request_id": "req-c1",
        "capability": {"id": "synvelis.midi.create-source", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "synvelis.midi.create-source.input", "version": {"major": 1, "minor": 0}},
        "project": ProjectId::new(7).to_tagged(),
        "target": {
            "shape": "create",
            "parent_kind": "project",
            "parent": ProjectId::new(7).to_tagged(),
            "creates": "source"
        },
        "args": {"division": 480},
        "consistency": {"kind": "expected-revision", "revision": revision},
    })
}

#[allow(clippy::result_large_err)] // mirrors the gateway signature
fn bind(gw: &mut Gateway, value: &Value) -> Result<OperationRequest, ErrorEnvelope> {
    let wire = Gateway::parse_wire(value)?;
    gw.bind(wire, agent())
}

fn err_code(result: Result<OperationRequest, ErrorEnvelope>) -> GatewayError {
    result.expect_err("expected a typed rejection").code
}

// ── GT-001: explicit typed stable ids — no implicit targeting ──

#[test]
fn gt001_explicit_typed_target_reaches_executor() {
    let mut gw = permissive_gateway();
    let req = bind(&mut gw, &midi_add_note_req(3)).unwrap();
    assert_eq!(
        req.target(),
        &OperationTarget::Resource {
            resource: ResourceRef::Source(SourceId::new(9)),
        }
    );
    assert_eq!(req.project(), ProjectId::new(7));
    assert_eq!(
        req.expected_revision(),
        Some(synvelis_project::ProjectRevision(3))
    );
}

#[test]
fn gt001_missing_target_rejected() {
    let mut gw = permissive_gateway();
    let mut req = midi_add_note_req(3);
    req.as_object_mut().unwrap().remove("target");
    assert_eq!(err_code(bind(&mut gw, &req)), GatewayError::TargetRequired);
}

#[test]
fn gt001_implicit_targeting_fields_rejected() {
    for field in ["selection", "focus", "active_window", "current_selection"] {
        let mut req = midi_add_note_req(3);
        req.as_object_mut()
            .unwrap()
            .insert(field.to_owned(), json!({"any": "value"}));
        let err = Gateway::parse_wire(&req).expect_err("implicit targeting must fail");
        assert_eq!(
            err.code,
            GatewayError::ImplicitTargetRejected {
                field: field.to_owned()
            }
        );
        assert_eq!(err.category, ErrorCategory::Validation);
    }
}

// ── GT-002: trusted caller cannot be self-declared ──

#[test]
fn gt002_authority_fields_rejected_on_wire() {
    for field in [
        "principal",
        "caller",
        "authority",
        "auth",
        "token",
        "permissions",
        "approved",
        "impersonate",
        "as_user",
        "role",
    ] {
        let mut req = midi_add_note_req(3);
        req.as_object_mut()
            .unwrap()
            .insert(field.to_owned(), json!("admin"));
        let err = Gateway::parse_wire(&req).expect_err("self-declared authority must fail");
        assert_eq!(
            err.code,
            GatewayError::SelfDeclaredAuthority {
                field: field.to_owned()
            }
        );
        assert_eq!(err.category, ErrorCategory::Authentication);
        assert_eq!(err.retry, RetryAdvice::Never);
    }
}

#[test]
fn gt002_unknown_wire_fields_rejected() {
    // Anything not in the wire contract — including fields that merely
    // *look* like authority — fails closed.
    let mut req = midi_add_note_req(3);
    req.as_object_mut()
        .unwrap()
        .insert("admin_override".to_owned(), json!(true));
    let err = Gateway::parse_wire(&req).expect_err("unknown field must fail");
    assert!(matches!(err.code, GatewayError::MalformedRequest { .. }));
}

#[test]
fn gt002_caller_comes_from_adapter_not_wire() {
    // The bound request's caller is exactly what the adapter attested —
    // nothing in the wire payload influenced it (the wire type has no
    // caller field, and AdapterSeal is unforgeable outside the crate).
    let mut gw = permissive_gateway();
    let req = bind(&mut gw, &midi_add_note_req(3)).unwrap();
    assert_eq!(req.caller().principal().kind(), PrincipalKind::Agent);
    assert_eq!(req.caller().principal().id(), "mcp-client-1");
    assert_eq!(req.caller().origin(), OriginKind::TestFixture);
}

// ── GT-003: capability existence ≠ permission ──

#[test]
fn gt003_discovery_returns_metadata_without_granting() {
    let mut gw = closed_gateway();
    // Discovery works — metadata only.
    let cap = gw
        .registry()
        .describe(&capability_ref("synvelis.midi.add-note", 1, 1))
        .unwrap();
    assert_eq!(cap.domain, OperationDomain::ProjectMutation);
    // But the fail-closed default denies every invoke.
    let err = gw
        .handle(&midi_add_note_req(3), agent())
        .expect_err("closed policy must deny");
    assert!(
        matches!(
            err.code,
            GatewayError::Denied {
                stage: AuthorizationStage::RequestApply,
                reason: Some(_),
            }
        ),
        "denial must carry the evaluator's reason: {err:?}"
    );
    assert_eq!(err.category, ErrorCategory::Authorization);
}

#[test]
fn gt003_new_capability_does_not_widen_existing_grants() {
    // Policy fixed to context-read only: it predates registration and
    // cannot be stretched by what the registry learns later.
    let mut gw = Gateway::with_evaluator(Box::new(PermitSetPolicy::new(PermissionSet::of([
        PermissionKind::ReadContext,
        PermissionKind::DiscloseContext,
    ]))));
    fixture::install(&mut gw).unwrap();

    // Permitted request: the context query's requirements fit.
    assert!(
        gw.handle(&context_query_req(), agent()).is_ok(),
        "context query should be permitted"
    );
    // The mutation capability exists — but its required permission was
    // never in the set.
    let err = gw
        .handle(&midi_add_note_req(3), agent())
        .expect_err("mutation must be denied under read-only policy");
    assert!(matches!(
        err.code,
        GatewayError::Denied {
            stage: AuthorizationStage::RequestApply,
            ..
        }
    ));
}

#[test]
fn gt003_permit_is_stage_scoped() {
    // A policy permitting transport control does not permit project
    // mutation, and vice versa — stage mapping is per domain.
    let mut gw = Gateway::with_evaluator(Box::new(PermitSetPolicy::new(PermissionSet::of([
        PermissionKind::ControlTransport,
    ]))));
    fixture::install(&mut gw).unwrap();
    assert!(gw.handle(&transport_req(), agent()).is_ok());
    assert!(gw.handle(&job_req(), agent()).is_err());
    assert!(gw.handle(&midi_add_note_req(3), agent()).is_err());
}

// ── GT-010: GW-01 never mutates project revision ──

#[test]
fn gt010_bind_does_not_execute() {
    // Validation is not execution: `bind` must produce a request
    // without the handler ever running.
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};

    struct Spy(Arc<AtomicBool>);
    impl CapabilityOperation for Spy {
        fn execute(&self, _req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
            self.0.store(true, Ordering::SeqCst);
            Ok(GatewayOutcome::Completed { result: json!({}) })
        }
    }

    let called = Arc::new(AtomicBool::new(false));
    let mut gw = permissive_gateway();
    gw.register_capability(
        test_descriptor("test.spy"),
        Some(Arc::new(Spy(called.clone()))),
    )
    .unwrap();

    let req = json!({
        "request_id": "req-s1",
        "capability": {"id": "test.spy", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "synvelis.midi.add-note.input", "version": {"major": 1, "minor": 1}},
        "project": ProjectId::new(7).to_tagged(),
        "target": {"shape": "resource", "kind": "source", "id": SourceId::new(9).to_tagged()},
        "args": {"key": 60, "velocity": 100, "position": {"domain": "source-native", "start": 0, "len": 10}},
        "consistency": {"kind": "expected-revision", "revision": 0},
    });
    let wire = Gateway::parse_wire(&req).unwrap();
    let bound = gw.bind(wire, agent()).unwrap();
    assert!(
        !called.load(Ordering::SeqCst),
        "bind must not execute the handler"
    );
    gw.invoke(&bound).unwrap();
    assert!(called.load(Ordering::SeqCst), "invoke runs the handler");
}

/// A minimal mutation descriptor for handler-level tests.
fn test_descriptor(id: &str) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: CapabilityId::new(id).unwrap(),
        version: CapabilityVersion::new(1, 0),
        domain: OperationDomain::ProjectMutation,
        input_schema: SchemaRef::new("synvelis.midi.add-note.input", 1, 1),
        output_schema: SchemaRef::new("synvelis.midi.add-note.output", 1, 0),
        target: TargetContract::Resource {
            kind: ResourceKind::Source,
        },
        effect_bound: EffectSet::of([EffectKind::Write]),
        required_permissions: PermissionSet::of([PermissionKind::MutateProject]),
        execution: ExecutionContract::Sync,
        availability: Availability::Available,
    }
}

fn capability_ref(id: &str, major: u32, minor: u32) -> CapabilityRef {
    CapabilityRef {
        id: CapabilityId::new(id).unwrap(),
        version: CapabilityVersion::new(major, minor),
    }
}

// ── AC1: unknown capability / version / schema / kind / time domain ──

#[test]
fn unknown_capability_rejected() {
    let mut gw = permissive_gateway();
    let mut req = midi_add_note_req(3);
    req["capability"]["id"] = json!("unknown.capability");
    let err = err_code(bind(&mut gw, &req));
    assert_eq!(
        err,
        GatewayError::UnknownCapability {
            id: "unknown.capability".to_owned()
        }
    );
    assert_eq!(err.category(), ErrorCategory::Capability);
    assert_eq!(err.retry(), RetryAdvice::Never);
}

#[test]
fn unsupported_capability_version_rejected() {
    let mut gw = permissive_gateway();
    let mut req = midi_add_note_req(3);
    req["capability"]["version"] = json!({"major": 9, "minor": 9});
    let err = err_code(bind(&mut gw, &req));
    assert_eq!(
        err,
        GatewayError::UnsupportedCapabilityVersion {
            id: "synvelis.midi.add-note".to_owned(),
            requested: CapabilityVersion::new(9, 9),
            supported: vec![CapabilityVersion::new(1, 1)],
        }
    );
}

#[test]
fn unknown_schema_rejected() {
    let mut gw = permissive_gateway();
    let mut req = midi_add_note_req(3);
    req["schema"]["name"] = json!("unknown.schema");
    assert_eq!(
        err_code(bind(&mut gw, &req)),
        GatewayError::UnknownSchema {
            name: "unknown.schema".to_owned()
        }
    );
}

#[test]
fn unsupported_schema_version_rejected() {
    let mut gw = permissive_gateway();
    let mut req = midi_add_note_req(3);
    req["schema"]["version"] = json!({"major": 9, "minor": 9});
    let err = err_code(bind(&mut gw, &req));
    assert_eq!(
        err,
        GatewayError::UnsupportedSchemaVersion {
            name: "synvelis.midi.add-note.input".to_owned(),
            requested: SchemaVersion::new(9, 9),
            supported: vec![
                SchemaVersion::new(1, 0),
                SchemaVersion::new(1, 1),
                SchemaVersion::new(2, 0),
            ],
        }
    );
}

#[test]
fn wrong_declared_resource_kind_rejected() {
    // Wire declares kind "clip" where the contract wants "source".
    let mut gw = permissive_gateway();
    let mut req = midi_add_note_req(3);
    req["target"]["kind"] = json!("clip");
    assert_eq!(
        err_code(bind(&mut gw, &req)),
        GatewayError::WrongResourceKind {
            expected: ResourceKind::Source,
            actual: ResourceKind::Clip,
        }
    );
}

#[test]
fn wrong_id_tag_resource_kind_rejected() {
    // Wire declares "source" but the tagged id is a clip.
    let mut gw = permissive_gateway();
    let mut req = midi_add_note_req(3);
    req["target"]["id"] = json!(synvelis_project::ClipId::new(9).to_tagged());
    assert_eq!(
        err_code(bind(&mut gw, &req)),
        GatewayError::WrongResourceKind {
            expected: ResourceKind::Source,
            actual: ResourceKind::Clip,
        }
    );
}

#[test]
fn unknown_resource_kind_rejected() {
    let mut gw = permissive_gateway();
    let mut req = midi_add_note_req(3);
    req["target"]["kind"] = json!("bogus-kind");
    assert_eq!(
        err_code(bind(&mut gw, &req)),
        GatewayError::UnknownResourceKind {
            name: "bogus-kind".to_owned()
        }
    );
}

#[test]
fn malformed_tagged_id_rejected() {
    let mut gw = permissive_gateway();
    let mut req = midi_add_note_req(3);
    req["target"]["id"] = json!("source-nothex");
    assert_eq!(
        err_code(bind(&mut gw, &req)),
        GatewayError::InvalidId {
            text: "source-nothex".to_owned()
        }
    );
}

#[test]
fn wrong_time_domain_rejected() {
    // MIDI note positions are source-native ticks; a project-musical
    // range is a typed error, never a silent conversion.
    let mut gw = permissive_gateway();
    let mut req = midi_add_note_req(3);
    req["args"]["position"]["domain"] = json!("project-musical");
    let err = err_code(bind(&mut gw, &req));
    assert_eq!(
        err,
        GatewayError::WrongTimeDomain {
            expected: TimeDomain::SourceNative,
            actual: TimeDomain::ProjectMusical,
        }
    );
    assert_eq!(err.category(), ErrorCategory::Validation);
}

#[test]
fn schema_violation_rejected() {
    let mut gw = permissive_gateway();

    // key above the MIDI range.
    let mut req = midi_add_note_req(3);
    req["args"]["key"] = json!(128);
    assert!(matches!(
        err_code(bind(&mut gw, &req)),
        GatewayError::SchemaViolation { .. }
    ));

    // velocity 0 is not a note-on attribute.
    let mut req = midi_add_note_req(3);
    req["args"]["velocity"] = json!(0);
    assert!(matches!(
        err_code(bind(&mut gw, &req)),
        GatewayError::SchemaViolation { .. }
    ));

    // undeclared args field (deny_unknown).
    let mut req = midi_add_note_req(3);
    req["args"]["surprise"] = json!(true);
    assert!(matches!(
        err_code(bind(&mut gw, &req)),
        GatewayError::SchemaViolation { .. }
    ));

    // missing required field.
    let mut req = midi_add_note_req(3);
    req["args"].as_object_mut().unwrap().remove("velocity");
    assert!(matches!(
        err_code(bind(&mut gw, &req)),
        GatewayError::SchemaViolation { .. }
    ));
}

#[test]
fn target_shape_mismatch_rejected() {
    let mut gw = permissive_gateway();
    // Contract is `resource`; wire sends `create`.
    let mut req = midi_add_note_req(3);
    req["target"] = json!({
        "shape": "create",
        "parent_kind": "project",
        "parent": ProjectId::new(7).to_tagged(),
        "creates": "source"
    });
    assert_eq!(
        err_code(bind(&mut gw, &req)),
        GatewayError::TargetShapeMismatch {
            expected: "resource",
            actual: "create",
        }
    );
}

#[test]
fn unexpected_target_rejected() {
    let mut gw = permissive_gateway();
    // Transport declares no target.
    let mut req = transport_req();
    req.as_object_mut().unwrap().insert(
        "target".to_owned(),
        json!({"shape": "resource", "kind": "project", "id": ProjectId::new(7).to_tagged()}),
    );
    assert_eq!(
        err_code(bind(&mut gw, &req)),
        GatewayError::UnexpectedTarget
    );
}

// ── AC2: mutation requires explicit revision; not confusable with query ──

#[test]
fn mutation_requires_expected_revision() {
    let mut gw = permissive_gateway();
    for kind in ["latest", "unconditional"] {
        let mut req = midi_add_note_req(3);
        req["consistency"] = json!({"kind": kind});
        assert_eq!(
            err_code(bind(&mut gw, &req)),
            GatewayError::ConsistencyNotAllowed {
                domain: OperationDomain::ProjectMutation,
                consistency: match kind {
                    "latest" => ConsistencyKind::Latest,
                    _ => ConsistencyKind::Unconditional,
                },
            },
            "mutation with {kind} consistency must be rejected"
        );
    }
    // read-at is a query pin — also illegal on a mutation.
    let mut req = midi_add_note_req(3);
    req["consistency"] = json!({"kind": "read-at", "revision": 3});
    assert_eq!(
        err_code(bind(&mut gw, &req)),
        GatewayError::ConsistencyNotAllowed {
            domain: OperationDomain::ProjectMutation,
            consistency: ConsistencyKind::ReadAt,
        }
    );
}

#[test]
fn query_rejects_mutation_consistency() {
    let mut gw = permissive_gateway();
    // A query must not carry mutation semantics — `expected-revision`
    // on a Query is a typed error, even for a real revision value.
    let mut req = context_query_req();
    req["consistency"] = json!({"kind": "expected-revision", "revision": 3});
    assert_eq!(
        err_code(bind(&mut gw, &req)),
        GatewayError::ConsistencyNotAllowed {
            domain: OperationDomain::Query,
            consistency: ConsistencyKind::ExpectedRevision,
        }
    );
    // Query forms are legal.
    for consistency in [
        json!({"kind": "latest"}),
        json!({"kind": "read-at", "revision": 3}),
        json!({"kind": "fixed-snapshot", "snapshot": "snap-1"}),
    ] {
        let mut req = context_query_req();
        req["consistency"] = consistency;
        assert!(bind(&mut gw, &req).is_ok());
    }
}

#[test]
fn runtime_and_async_require_unconditional() {
    let mut gw = permissive_gateway();
    for req in [transport_req(), job_req()] {
        let mut r = req.clone();
        r["consistency"] = json!({"kind": "expected-revision", "revision": 1});
        assert!(matches!(
            err_code(bind(&mut gw, &r)),
            GatewayError::ConsistencyNotAllowed { .. }
        ));
    }
    assert!(bind(&mut gw, &transport_req()).is_ok());
    assert!(bind(&mut gw, &job_req()).is_ok());
}

#[test]
fn create_target_validates_parent_and_kind() {
    let mut gw = permissive_gateway();
    // Wrong declared parent kind.
    let mut req = create_source_req(0);
    req["target"]["parent_kind"] = json!("track");
    assert_eq!(
        err_code(bind(&mut gw, &req)),
        GatewayError::WrongResourceKind {
            expected: ResourceKind::Project,
            actual: ResourceKind::Track,
        }
    );
    // Wrong declared create kind.
    let mut req = create_source_req(0);
    req["target"]["creates"] = json!("clip");
    assert_eq!(
        err_code(bind(&mut gw, &req)),
        GatewayError::WrongResourceKind {
            expected: ResourceKind::Source,
            actual: ResourceKind::Clip,
        }
    );
    // Valid create target.
    let req = create_source_req(0);
    let bound = bind(&mut gw, &req).unwrap();
    assert_eq!(
        bound.target(),
        &OperationTarget::Create {
            parent: ResourceRef::Project(ProjectId::new(7)),
            creates: ResourceKind::Source,
        }
    );
}

// ── Idempotency ──

#[test]
fn idempotency_duplicate_and_conflict() {
    let mut gw = permissive_gateway();
    let mut first = midi_add_note_req(3);
    first
        .as_object_mut()
        .unwrap()
        .insert("idempotency_key".to_owned(), json!("idem-1"));

    // First execution succeeds and records the key.
    let wire = Gateway::parse_wire(&first).unwrap();
    let req1 = gw.bind(wire, agent()).unwrap();
    let op1 = req1.operation_id();
    gw.invoke(&req1).unwrap();

    // Same key, same semantic payload, new request id (a retry) →
    // duplicate naming the operation that actually ran.
    let mut retry = first.clone();
    retry["request_id"] = json!("req-m1-retry");
    let wire = Gateway::parse_wire(&retry).unwrap();
    let req2 = gw.bind(wire, agent()).unwrap();
    let err = gw.invoke(&req2).expect_err("retry must be a duplicate");
    assert_eq!(err.code, GatewayError::DuplicateRequest { original: op1 });
    assert_eq!(err.category, ErrorCategory::DuplicateRequest);
    assert_eq!(err.retry, RetryAdvice::Never);

    // Same key, different payload → conflict.
    let mut different = first.clone();
    different["request_id"] = json!("req-m2");
    different["args"]["key"] = json!(61);
    let err = gw
        .handle(&different, agent())
        .expect_err("key reuse with a different payload must conflict");
    assert_eq!(err.code, GatewayError::IdempotencyConflict);

    // A different key is independent.
    let mut other = first.clone();
    other["request_id"] = json!("req-m3");
    other["idempotency_key"] = json!("idem-2");
    assert!(gw.handle(&other, agent()).is_ok());
}

#[test]
fn idempotency_denial_never_burns_key() {
    // A denied request never reaches the ledger record — switching to
    // a permitting evaluator lets the same key execute cleanly.
    let mut gw = Gateway::new();
    fixture::install(&mut gw).unwrap();
    let mut req = midi_add_note_req(3);
    req.as_object_mut()
        .unwrap()
        .insert("idempotency_key".to_owned(), json!("denied-key"));
    let err = gw.handle(&req, agent()).expect_err("closed policy denies");
    assert!(matches!(err.code, GatewayError::Denied { .. }));

    gw.set_evaluator(Box::new(PermitSetPolicy::new(all_permissions())));
    req["request_id"] = json!("req-m1-after-deny");
    assert!(
        gw.handle(&req, agent()).is_ok(),
        "a denied attempt must not burn the idempotency key"
    );
}

#[test]
fn idempotency_scoped_to_principal_and_project() {
    let mut gw = permissive_gateway();
    let mut req = midi_add_note_req(3);
    req.as_object_mut()
        .unwrap()
        .insert("idempotency_key".to_owned(), json!("shared-key"));
    gw.handle(&req, agent()).unwrap();

    // Same key but another principal → not a duplicate.
    let other_principal = adapter().agent("other-agent").unwrap();
    assert!(gw.handle(&req, other_principal).is_ok());

    // Same key, same principal, a different project → independent.
    let mut proj2 = req.clone();
    proj2["project"] = json!(ProjectId::new(8).to_tagged());
    assert!(gw.handle(&proj2, agent()).is_ok());
}

// ── Outcome / envelope contract ──

#[test]
fn outcome_variants_are_distinct() {
    let mut gw = permissive_gateway();
    // Completed — the sync fixtures.
    let outcome = gw.handle(&context_query_req(), agent()).unwrap();
    assert!(matches!(outcome, GatewayOutcome::Completed { .. }));
    // Accepted — the async job fixture.
    let outcome = gw.handle(&job_req(), agent()).unwrap();
    match outcome {
        GatewayOutcome::Accepted { operation } => {
            assert!(operation.to_tagged().starts_with("operation-"));
        }
        other => panic!("expected Accepted, got {other:?}"),
    }
}

#[test]
fn approval_required_is_an_outcome_not_an_error() {
    struct NeedApproval;
    impl PolicyEvaluator for NeedApproval {
        fn decide(
            &self,
            _req: &OperationRequest,
            _stage: AuthorizationStage,
            _cap: &CapabilityDescriptor,
        ) -> AuthorizationDecision {
            AuthorizationDecision::ApprovalRequired
        }
    }
    let mut gw = Gateway::with_evaluator(Box::new(NeedApproval));
    fixture::install(&mut gw).unwrap();
    let outcome = gw.handle(&midi_add_note_req(3), agent()).unwrap();
    match outcome {
        GatewayOutcome::ApprovalRequired { approval } => {
            assert!(approval.as_str().starts_with("approval-for-operation-"));
        }
        other => panic!("expected ApprovalRequired, got {other:?}"),
    }
}

#[test]
fn outcome_serializes_all_variants() {
    // The envelope is a wire contract: every variant must round-trip.
    // (An internally-tagged repr over scalar payloads cannot serialize —
    // this guards the named-field representation.)
    for outcome in [
        GatewayOutcome::Completed {
            result: json!({"ok": true}),
        },
        GatewayOutcome::ProposalReady {
            proposal: synvelis_gateway::ProposalRef::new("p-1").unwrap(),
        },
        GatewayOutcome::ApprovalRequired {
            approval: synvelis_gateway::ApprovalRef::new("a-1").unwrap(),
        },
        GatewayOutcome::Accepted {
            operation: OperationId::new(7),
        },
    ] {
        let value = serde_json::to_value(&outcome).unwrap();
        let status = value["status"].as_str().unwrap();
        assert!(matches!(
            status,
            "completed" | "proposal-ready" | "approval-required" | "accepted"
        ));
    }
}

#[test]
fn nested_unknown_fields_rejected() {
    // `deny_unknown_fields` is inert on internally-tagged serde types,
    // so the wire parser scans the objects it owns explicitly.
    for (path, key) in [
        ("capability", "stealth"),
        ("capability.version", "patch"),
        ("target", "implicit_hint"),
        ("consistency", "optimistic"),
    ] {
        let mut req = midi_add_note_req(3);
        let (head, tail) = path.split_once('.').unwrap_or((path, ""));
        let mut cursor = &mut req[head];
        if !tail.is_empty() {
            cursor = &mut cursor[tail];
        }
        cursor
            .as_object_mut()
            .unwrap()
            .insert(key.to_owned(), json!(1));
        let err = Gateway::parse_wire(&req)
            .expect_err("nested unknown field must fail")
            .code;
        assert!(
            matches!(err, GatewayError::MalformedRequest { .. }),
            "{path}.{key}: {err:?}"
        );
    }
}

#[test]
fn registration_rejects_incoherent_or_unresolvable() {
    let mut gw = permissive_gateway();

    // A Query declaring a mutating effect would misroute the
    // authorization stage — rejected at registration.
    let mut bad = test_descriptor("test.query-with-write");
    bad.domain = OperationDomain::Query;
    assert_eq!(
        gw.register_capability(bad, None)
            .expect_err("incoherent descriptor must fail")
            .code,
        GatewayError::IncoherentDescriptor {
            id: "test.query-with-write".to_owned(),
            reason: "query capabilities may not declare mutating effects".to_owned(),
        }
    );

    // A descriptor declaring a schema that was never registered is
    // uninvocable by construction — rejected at registration.
    let mut dangling = test_descriptor("test.dangling-schema");
    dangling.input_schema = SchemaRef::new("never.registered", 1, 0);
    assert!(matches!(
        gw.register_capability(dangling, None)
            .expect_err("unresolvable schema must fail")
            .code,
        GatewayError::UnknownSchema { .. }
    ));

    // A schema version is immutable once published.
    let err = gw
        .schemas_mut()
        .register(
            "synvelis.midi.add-note.input",
            SchemaVersion::new(1, 0),
            synvelis_gateway::ValueSchema::Bool,
        )
        .expect_err("schema re-registration must fail");
    assert!(matches!(err, GatewayError::SchemaConflict { .. }));
}

#[test]
fn sync_capability_cannot_return_accepted() {
    // `ExecutionContract::Sync` + an async outcome means the caller
    // would wait on a handle nothing completes — an executor bug,
    // surfaced as Internal, never delivered as a valid outcome.
    use std::sync::Arc;
    struct WrongContract;
    impl CapabilityOperation for WrongContract {
        fn execute(&self, req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
            Ok(GatewayOutcome::Accepted {
                operation: req.operation_id(),
            })
        }
    }
    let mut gw = permissive_gateway();
    gw.register_capability(
        test_descriptor("test.wrong-contract"),
        Some(Arc::new(WrongContract)),
    )
    .unwrap();

    let req = json!({
        "request_id": "req-w1",
        "capability": {"id": "test.wrong-contract", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "synvelis.midi.add-note.input", "version": {"major": 1, "minor": 1}},
        "project": ProjectId::new(7).to_tagged(),
        "target": {"shape": "resource", "kind": "source", "id": SourceId::new(9).to_tagged()},
        "args": {"key": 60, "velocity": 100, "position": {"domain": "source-native", "start": 0, "len": 10}},
        "consistency": {"kind": "expected-revision", "revision": 0},
    });
    let err = gw.handle(&req, agent()).expect_err("contract violation");
    assert!(matches!(err.code, GatewayError::Internal { .. }));
}

#[test]
fn completed_result_must_match_output_schema() {
    // A strict output schema catches results that violate the declared
    // contract — again an executor bug, not a caller-facing outcome.
    use std::sync::Arc;
    struct BadOutput;
    impl CapabilityOperation for BadOutput {
        fn execute(&self, _req: &OperationRequest) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
            Ok(GatewayOutcome::Completed {
                result: json!({"unexpected": "shape"}),
            })
        }
    }
    let mut gw = permissive_gateway();
    gw.schemas_mut()
        .register(
            "test.strict-output.output",
            SchemaVersion::new(1, 0),
            synvelis_gateway::ValueSchema::Object {
                fields: std::collections::BTreeMap::from([(
                    "committed".to_owned(),
                    synvelis_gateway::FieldSchema::required(
                        synvelis_gateway::ValueSchema::Integer {
                            min: None,
                            max: None,
                        },
                    ),
                )]),
                deny_unknown: true,
            },
        )
        .unwrap();
    let mut desc = test_descriptor("test.bad-output");
    desc.output_schema = SchemaRef::new("test.strict-output.output", 1, 0);
    gw.register_capability(desc, Some(Arc::new(BadOutput)))
        .unwrap();

    let req = json!({
        "request_id": "req-b1",
        "capability": {"id": "test.bad-output", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "synvelis.midi.add-note.input", "version": {"major": 1, "minor": 1}},
        "project": ProjectId::new(7).to_tagged(),
        "target": {"shape": "resource", "kind": "source", "id": SourceId::new(9).to_tagged()},
        "args": {"key": 60, "velocity": 100, "position": {"domain": "source-native", "start": 0, "len": 10}},
        "consistency": {"kind": "expected-revision", "revision": 0},
    });
    let err = gw.handle(&req, agent()).expect_err("schema violation");
    assert!(matches!(err.code, GatewayError::Internal { .. }));
}

#[test]
fn principal_id_is_bounded() {
    assert!(Principal::new(PrincipalKind::Agent, "").is_err());
    assert!(Principal::new(PrincipalKind::Agent, "a".repeat(257)).is_err());
    assert_eq!(
        Principal::new(PrincipalKind::Agent, "mcp-client-1")
            .unwrap()
            .id(),
        "mcp-client-1"
    );
}

#[test]
fn envelope_splits_public_and_internal() {
    let mut gw = permissive_gateway();
    let mut req = midi_add_note_req(3);
    req["args"]["key"] = json!("not-an-int");
    let err = bind(&mut gw, &req).expect_err("schema violation");
    let err = err.with_diagnostic("internal: serde path args.key visited");

    let external = err.to_external();
    let json = serde_json::to_value(&external).unwrap();
    assert!(json.get("diagnostic").is_none());
    assert_eq!(json["category"], json!("validation"));
    assert_eq!(json["retry"], json!("after-correction"));
    // The public message is derived from typed fields only.
    assert_ne!(external.message, "");
}

#[test]
fn unavailable_capability_rejected() {
    let mut gw = permissive_gateway();
    let mut planned = test_descriptor("test.planned-op");
    planned.availability = Availability::Planned;
    gw.register_capability(planned, None).unwrap();

    let req = json!({
        "request_id": "req-p1",
        "capability": {"id": "test.planned-op", "version": {"major": 1, "minor": 0}},
        "schema": {"name": "synvelis.midi.add-note.input", "version": {"major": 1, "minor": 1}},
        "project": ProjectId::new(7).to_tagged(),
        "target": {"shape": "resource", "kind": "source", "id": SourceId::new(9).to_tagged()},
        "args": {"key": 60, "velocity": 100, "position": {"domain": "source-native", "start": 0, "len": 10}},
        "consistency": {"kind": "expected-revision", "revision": 0},
    });
    assert_eq!(
        err_code(bind(&mut gw, &req)),
        GatewayError::Unavailable {
            id: "test.planned-op".to_owned(),
            availability: Availability::Planned,
        }
    );
}

#[test]
fn duplicate_registration_rejected() {
    let mut gw = permissive_gateway();
    let desc = gw
        .registry()
        .describe(&capability_ref("synvelis.transport.set-position", 1, 0))
        .unwrap()
        .clone();
    assert_eq!(
        gw.register_capability(desc, None)
            .expect_err("re-registration must fail")
            .code,
        GatewayError::CapabilityConflict {
            id: "synvelis.transport.set-position".to_owned(),
            version: CapabilityVersion::new(1, 0),
        }
    );
}
