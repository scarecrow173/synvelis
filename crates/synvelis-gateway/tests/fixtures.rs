//! Per-domain typed fixtures (Issue #53 AC4) and the GW-06 executor
//! seam — a real `ProjectController` behind a capability handler.

use std::sync::Arc;

use serde_json::{Value, json};
use synvelis_gateway::fixture::{self, FixtureAdapter, PermitSetPolicy};
use synvelis_gateway::*;
use synvelis_project::{
    Command, CounterIdGenerator, MidiDivision, MidiTick, NoteId, Project, ProjectController,
    ProjectId, ProjectOp, SourceId,
};

fn agent() -> CallerIdentity {
    FixtureAdapter::for_testing().agent("mcp-client-1").unwrap()
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

fn gateway() -> Gateway {
    let mut gw = Gateway::with_evaluator(Box::new(PermitSetPolicy::new(all_permissions())));
    fixture::install(&mut gw).unwrap();
    gw
}

fn cap_ref(id: &str, minor: u32) -> CapabilityRef {
    CapabilityRef {
        id: CapabilityId::new(id).unwrap(),
        version: CapabilityVersion::new(1, minor),
    }
}

fn base_req(id: &str, minor: u32, schema_minor: u32) -> serde_json::Map<String, Value> {
    json!({
        "request_id": "req-1",
        "capability": {"id": id, "version": {"major": 1, "minor": minor}},
        "schema": {"name": format!("{id}.input"), "version": {"major": 1, "minor": schema_minor}},
        "project": ProjectId::new(7).to_tagged(),
        "consistency": {"kind": "unconditional"},
    })
    .as_object()
    .unwrap()
    .clone()
}

#[test]
fn fixtures_cover_all_four_domains() {
    let gw = gateway();
    let listed = gw.registry().list();
    for (id, minor, domain) in [
        ("synvelis.context.source-notes", 0, OperationDomain::Query),
        (
            "synvelis.midi.add-note",
            1,
            OperationDomain::ProjectMutation,
        ),
        (
            "synvelis.midi.create-source",
            0,
            OperationDomain::ProjectMutation,
        ),
        (
            "synvelis.transport.set-position",
            0,
            OperationDomain::RuntimeControl,
        ),
        (
            "synvelis.job.render-export",
            0,
            OperationDomain::AsyncOperation,
        ),
    ] {
        let desc = gw.registry().describe(&cap_ref(id, minor)).unwrap();
        assert_eq!(desc.domain, domain, "{id} domain");
        assert!(
            listed.iter().any(|r| r.id.as_str() == id),
            "{id} must be listed"
        );
    }
}

#[test]
fn context_query_fixture_roundtrip() {
    let mut gw = gateway();
    let mut req = base_req("synvelis.context.source-notes", 0, 0);
    req.insert(
        "target".to_owned(),
        json!({"shape": "resource", "kind": "source", "id": SourceId::new(9).to_tagged()}),
    );
    req.insert(
        "args".to_owned(),
        json!({"range": {"domain": "source-native", "start": 0, "len": 960}}),
    );
    req.insert("consistency".to_owned(), json!({"kind": "latest"}));

    let outcome = gw.handle(&Value::Object(req), agent()).unwrap();
    let GatewayOutcome::Completed { result } = outcome else {
        panic!("expected Completed");
    };
    assert_eq!(result["domain"], json!("query"));
    assert_eq!(result["target"]["shape"], json!("resource"));
    assert_eq!(result["target"]["resource"]["kind"], json!("source"));
    assert_eq!(result["consistency"]["kind"], json!("latest"));
}

#[test]
fn midi_mutation_fixture_roundtrip() {
    let mut gw = gateway();
    let mut req = base_req("synvelis.midi.add-note", 1, 1);
    req.insert(
        "target".to_owned(),
        json!({"shape": "resource", "kind": "source", "id": SourceId::new(9).to_tagged()}),
    );
    req.insert(
        "args".to_owned(),
        json!({"key": 60, "velocity": 100, "position": {"domain": "source-native", "start": 240, "len": 480}}),
    );
    req.insert(
        "consistency".to_owned(),
        json!({"kind": "expected-revision", "revision": 5}),
    );

    let outcome = gw.handle(&Value::Object(req), agent()).unwrap();
    let GatewayOutcome::Completed { result } = outcome else {
        panic!("expected Completed");
    };
    assert_eq!(result["domain"], json!("project-mutation"));
    assert_eq!(result["consistency"]["kind"], json!("expected-revision"));
    assert_eq!(result["consistency"]["revision"], json!(5));
    assert_eq!(result["args"]["key"], json!(60));
}

#[test]
fn create_source_fixture_roundtrip() {
    let mut gw = gateway();
    let mut req = base_req("synvelis.midi.create-source", 0, 0);
    req.insert(
        "target".to_owned(),
        json!({
            "shape": "create",
            "parent_kind": "project",
            "parent": ProjectId::new(7).to_tagged(),
            "creates": "source"
        }),
    );
    req.insert("args".to_owned(), json!({"division": 480}));
    req.insert(
        "consistency".to_owned(),
        json!({"kind": "expected-revision", "revision": 0}),
    );

    let outcome = gw.handle(&Value::Object(req), agent()).unwrap();
    let GatewayOutcome::Completed { result } = outcome else {
        panic!("expected Completed");
    };
    assert_eq!(result["target"]["shape"], json!("create"));
    assert_eq!(result["target"]["creates"], json!("source"));
}

#[test]
fn transport_fixture_roundtrip() {
    let mut gw = gateway();
    let mut req = base_req("synvelis.transport.set-position", 0, 0);
    req.insert("args".to_owned(), json!({"action": "locate", "position": {"domain": "project-musical", "start": 960, "len": 1}}));
    let outcome = gw.handle(&Value::Object(req), agent()).unwrap();
    assert!(matches!(outcome, GatewayOutcome::Completed { .. }));
}

#[test]
fn job_fixture_accepts() {
    let mut gw = gateway();
    let mut req = base_req("synvelis.job.render-export", 0, 0);
    req.insert("args".to_owned(), json!({"format": "wav"}));
    let outcome = gw.handle(&Value::Object(req), agent()).unwrap();
    match outcome {
        GatewayOutcome::Accepted { operation } => {
            assert!(operation.to_tagged().starts_with("operation-"));
        }
        other => panic!("expected Accepted, got {other:?}"),
    }
}

/// The GW-06 seam: `bind` produces a request whose `ExpectedRevision`
/// feeds straight into `Command.expected_revision` of the real single
/// writer — and `StaleRevision` maps back to the typed
/// `RevisionConflict` envelope. The writer itself stays on its own
/// owner (`ProjectController` is deliberately not `Send`); GW-01 never
/// holds or drives it.
#[test]
fn executor_seam_project_controller() {
    // What a GW-06 commit executor will run against the single writer.
    #[allow(clippy::result_large_err)] // mirrors the handler signature
    fn commit_add_note(
        req: &OperationRequest,
        controller: &mut ProjectController,
        source: SourceId,
    ) -> Result<GatewayOutcome<Value>, ErrorEnvelope> {
        let revision = controller.apply(Command {
            expected_revision: req
                .expected_revision()
                .expect("mutation carries a revision"),
            op: ProjectOp::AddMidiNote {
                source_id: source,
                key: u8::try_from(req.args()["key"].as_i64().unwrap()).unwrap(),
                velocity: u8::try_from(req.args()["velocity"].as_i64().unwrap()).unwrap(),
                start: MidiTick::new(req.args()["position"]["start"].as_i64().unwrap()).unwrap(),
                len: MidiTick::new(req.args()["position"]["len"].as_i64().unwrap()).unwrap(),
            },
        });
        match revision {
            Ok(rev) => Ok(GatewayOutcome::Completed {
                result: json!({"committed": rev}),
            }),
            Err(synvelis_project::Error::StaleRevision { expected, actual }) => {
                Err(GatewayError::RevisionConflict { expected, actual }.into())
            }
            Err(e) => Err(ErrorEnvelope::new(GatewayError::Internal {
                reason: e.to_string(),
            })),
        }
    }

    // Real project with one MIDI source.
    let mut controller = ProjectController::new(
        Project::new(ProjectId::new(7), "p"),
        Box::new(CounterIdGenerator::new(1)),
    );
    controller
        .apply(Command {
            expected_revision: controller.live_revision(),
            op: ProjectOp::AddMidiSource {
                division: MidiDivision::ppqn(480).unwrap(),
            },
        })
        .unwrap();
    let source = *controller.live().midi_sources.keys().next().unwrap();
    let live_revision = controller.live_revision().0;

    let mut gw = gateway();
    let mut req = base_req("synvelis.midi.add-note", 1, 1);
    req.insert(
        "target".to_owned(),
        json!({"shape": "resource", "kind": "source", "id": source.to_tagged()}),
    );
    req.insert(
        "args".to_owned(),
        json!({"key": 60, "velocity": 100, "position": {"domain": "source-native", "start": 240, "len": 480}}),
    );
    req.insert(
        "consistency".to_owned(),
        json!({"kind": "expected-revision", "revision": live_revision}),
    );

    // GW-01 validates and binds; the trusted request is what crosses
    // into executor territory.
    let wire = Gateway::parse_wire(&Value::Object(req)).unwrap();
    let bound = gw.bind(wire, agent()).unwrap();

    // Fresh expectation commits through the seam.
    let outcome = commit_add_note(&bound, &mut controller, source).unwrap();
    let GatewayOutcome::Completed { result } = outcome else {
        panic!("expected Completed");
    };
    assert_eq!(result["committed"], json!(live_revision + 1));

    // The same stale expectation now conflicts — typed, not guessed.
    let err =
        commit_add_note(&bound, &mut controller, source).expect_err("stale revision must conflict");
    assert_eq!(
        err.code,
        GatewayError::RevisionConflict {
            expected: live_revision,
            actual: live_revision + 1,
        }
    );
    assert_eq!(err.retry, RetryAdvice::ReconcileState);
}

#[test]
fn note_id_typed_target_resource_set() {
    // A resource-set contract — typed ids in a list all checked.
    let mut gw = gateway();
    gw.register_capability(
        CapabilityDescriptor {
            id: CapabilityId::new("test.notes.bulk").unwrap(),
            version: CapabilityVersion::new(1, 0),
            domain: OperationDomain::Query,
            input_schema: SchemaRef::new("synvelis.job.render-export.input", 1, 0),
            output_schema: SchemaRef::new("synvelis.job.render-export.output", 1, 0),
            target: TargetContract::ResourceSet {
                kind: ResourceKind::Note,
            },
            effect_bound: EffectSet::of([EffectKind::Read]),
            required_permissions: PermissionSet::of([PermissionKind::ReadContext]),
            data_access: DataAccessSet::of([DataAccessClass::MetadataRead]),
            execution: ExecutionContract::Sync,
            availability: Availability::Available,
        },
        Some(Arc::new(fixture::EchoHandler)),
    )
    .unwrap();

    let mut req = base_req("test.notes.bulk", 0, 0);
    req["schema"]["name"] = json!("synvelis.job.render-export.input");
    req.insert(
        "target".to_owned(),
        json!({
            "shape": "resource-set",
            "kind": "note",
            "ids": [NoteId::new(1).to_tagged(), NoteId::new(2).to_tagged()]
        }),
    );
    req.insert("args".to_owned(), json!({"format": "wav"}));
    req.insert("consistency".to_owned(), json!({"kind": "latest"}));
    assert!(gw.handle(&Value::Object(req.clone()), agent()).is_ok());

    // One clip id inside a note set → typed kind rejection.
    let mut req2 = req.clone();
    req2["target"]["ids"] = json!([synvelis_project::ClipId::new(1).to_tagged()]);
    let err = gw
        .handle(&Value::Object(req2), agent())
        .expect_err("mixed kind must fail");
    assert_eq!(
        err.code,
        GatewayError::WrongResourceKind {
            expected: ResourceKind::Note,
            actual: ResourceKind::Clip,
        }
    );
}
