//! SV-SCRIPT-03 / synvelis#273 — `WorkflowScriptBuilder`: spec →
//! canonical import document. Calls become step references only —
//! nothing executes, no authority is minted.

mod common;

use serde_json::{Map, Value, json};
use synvelis_script::{
    AvailableBackends, BackendRequirement, LanguageId, ScriptApiError, ScriptStepCall,
    ScriptWorkflowRef, ScriptWorkflowSpec, SpecIoDecl, WORKFLOW_SCRIPT_DEFINITION_SCHEMA_VERSION,
    WORKFLOW_SPEC_SCHEMA_VERSION, WorkflowScriptBuilder, WorkflowScriptDefinition,
};

use crate::common::{binding_set, sample_model};

fn definition(language: LanguageId, backend: BackendRequirement) -> WorkflowScriptDefinition {
    WorkflowScriptDefinition {
        schema_version: WORKFLOW_SCRIPT_DEFINITION_SCHEMA_VERSION,
        script_id: "test.workflow".to_owned(),
        version: 1,
        language_id: language,
        backend_requirement: backend,
        source_digest: "cd".repeat(32),
    }
}

fn spec(steps: Vec<ScriptStepCall>) -> ScriptWorkflowSpec {
    ScriptWorkflowSpec {
        schema_version: WORKFLOW_SPEC_SCHEMA_VERSION,
        workflow_id: "test.flow".to_owned(),
        version: 2,
        inputs: vec![SpecIoDecl {
            name: "track".to_owned(),
            schema: json!({"type": "id", "kind": "track"}),
        }],
        steps,
        outputs: vec![SpecIoDecl {
            name: "note".to_owned(),
            schema: json!({"type": "id", "kind": "note"}),
        }],
        limits: json!({
            "max_steps": 100,
            "max_depth": 8,
            "max_iterations": 1000,
            "max_runtime_ms": 60_000,
            "max_output_bytes": 1_048_576,
            "max_memory_bytes": 134_217_728,
        }),
    }
}

fn build(
    definition: &WorkflowScriptDefinition,
    spec: &ScriptWorkflowSpec,
    backends: AvailableBackends,
) -> Result<Value, ScriptApiError> {
    WorkflowScriptBuilder::build(definition, spec, &backends, &sample_model(), &binding_set())
}

#[test]
fn action_calls_lower_to_kind_from_contract() {
    // trackInfo → Query/Sync → "query"; addNote → ProjectMutation/Sync
    // → "invoke". Capability pins are exact `id@major.minor`.
    let doc = build(
        &definition(LanguageId::Luau, BackendRequirement::Luau),
        &spec(vec![
            ScriptStepCall::Action {
                id: "info".to_owned(),
                name: "trackInfo".to_owned(),
                args: json!({}),
                target: Some(json!("$input.track")),
            },
            ScriptStepCall::Action {
                id: "note".to_owned(),
                name: "addNote".to_owned(),
                args: json!({"pitch": 60}),
                target: None,
            },
            ScriptStepCall::Emit {
                id: "done".to_owned(),
                output: "note".to_owned(),
                value: json!("$steps.note"),
            },
        ]),
        AvailableBackends::luau(),
    )
    .expect("spec builds");

    assert_eq!(doc["schema_version"], json!(1));
    assert_eq!(doc["workflow_id"], json!("test.flow"));
    assert_eq!(doc["version"], json!(2));
    let steps = doc["steps"].as_array().unwrap();
    assert_eq!(steps[0]["kind"], json!("query"));
    assert_eq!(
        steps[0]["capability"],
        json!("synvelis.query.track-info@1.0")
    );
    assert_eq!(steps[0]["target"], json!("$input.track"));
    assert_eq!(steps[1]["kind"], json!("invoke"));
    assert_eq!(steps[1]["capability"], json!("synvelis.midi.add-note@1.0"));
    assert!(steps[1].get("target").is_none());
    assert_eq!(steps[2]["kind"], json!("emit-result"));
    // Expression surfaces pass through verbatim — the importer lowers.
    assert_eq!(steps[2]["value"], json!("$steps.note"));
}

#[test]
fn workflow_call_carries_pinned_reference() {
    let reference = ScriptWorkflowRef {
        language_id: LanguageId::Rhai,
        version: 7,
        source_digest: "ef".repeat(32),
        path: "mid.rhai".to_owned(),
    };
    let doc = build(
        &definition(LanguageId::Luau, BackendRequirement::Luau),
        &spec(vec![
            ScriptStepCall::Workflow {
                id: "mid".to_owned(),
                reference: reference.clone(),
                args: Map::from_iter([("track".to_owned(), json!("$input.track"))]),
            },
            ScriptStepCall::Emit {
                id: "done".to_owned(),
                output: "note".to_owned(),
                value: json!("$steps.mid.note"),
            },
        ]),
        AvailableBackends::luau_and_rhai(),
    )
    .expect("spec builds");
    let call = &doc["steps"][0];
    assert_eq!(call["kind"], json!("workflow"));
    assert_eq!(call["reference"]["language_id"], json!("rhai"));
    assert_eq!(call["reference"]["version"], json!(7));
    assert_eq!(call["reference"]["path"], json!("mid.rhai"));
    assert_eq!(call["args"]["track"], json!("$input.track"));
}

#[test]
fn control_calls_lower_to_restricted_ast_kinds() {
    let doc = build(
        &definition(LanguageId::Yaml, BackendRequirement::None),
        &spec(vec![
            ScriptStepCall::If {
                id: "branch".to_owned(),
                condition: json!({"op": "eq", "left": 1, "right": 1}),
                then: vec![ScriptStepCall::Action {
                    id: "inner".to_owned(),
                    name: "trackInfo".to_owned(),
                    args: json!({}),
                    target: None,
                }],
                else_steps: vec![],
            },
            ScriptStepCall::ForEach {
                id: "loop".to_owned(),
                items: json!([1, 2]),
                max_iterations: 4,
                body: vec![],
            },
            ScriptStepCall::AwaitJob {
                id: "wait".to_owned(),
                job: json!("$steps.info.job"),
                output_schema: Some("synvelis.out.job-status@1.0".to_owned()),
            },
            ScriptStepCall::Emit {
                id: "done".to_owned(),
                output: "note".to_owned(),
                value: json!("$steps.wait"),
            },
        ]),
        AvailableBackends {
            luau: false,
            rhai: false,
        },
    )
    .expect("spec builds");
    let steps = doc["steps"].as_array().unwrap();
    assert_eq!(steps[0]["kind"], json!("if"));
    assert_eq!(steps[0]["then"][0]["kind"], json!("query"));
    assert_eq!(steps[1]["kind"], json!("for-each-bounded"));
    assert_eq!(steps[1]["max_iterations"], json!(4));
    assert_eq!(steps[2]["kind"], json!("await-job"));
    assert_eq!(
        steps[2]["output_schema"],
        json!("synvelis.out.job-status@1.0")
    );
}

#[test]
fn backend_and_language_fail_closed() {
    // Rhai definition without a Rhai backend → typed unavailable.
    let err = build(
        &definition(LanguageId::Rhai, BackendRequirement::Rhai),
        &spec(vec![]),
        AvailableBackends::luau(),
    )
    .unwrap_err();
    assert!(
        matches!(err, ScriptApiError::BackendUnavailable { .. }),
        "{err:?}"
    );

    // Language/backend inconsistency → typed mismatch.
    let mut def = definition(LanguageId::Luau, BackendRequirement::Luau);
    def.backend_requirement = BackendRequirement::None;
    let err = build(&def, &spec(vec![]), AvailableBackends::luau()).unwrap_err();
    assert!(
        matches!(err, ScriptApiError::LanguageBackendMismatch { .. }),
        "{err:?}"
    );
}

#[test]
fn unknown_and_unmapped_actions_rejected() {
    // Unknown API name.
    let err = build(
        &definition(LanguageId::Luau, BackendRequirement::Luau),
        &spec(vec![ScriptStepCall::Action {
            id: "x".to_owned(),
            name: "noSuchApi".to_owned(),
            args: json!({}),
            target: None,
        }]),
        AvailableBackends::luau(),
    )
    .unwrap_err();
    assert!(matches!(err, ScriptApiError::UnknownApi { .. }), "{err:?}");

    // `play` is RuntimeControl — no workflow step kind exists for it.
    let err = build(
        &definition(LanguageId::Luau, BackendRequirement::Luau),
        &spec(vec![ScriptStepCall::Action {
            id: "p".to_owned(),
            name: "play".to_owned(),
            args: json!({}),
            target: None,
        }]),
        AvailableBackends::luau(),
    )
    .unwrap_err();
    assert!(
        matches!(err, ScriptApiError::UnsupportedWorkflowStep { .. }),
        "{err:?}"
    );
}

#[test]
fn duplicate_step_ids_rejected_before_lowering() {
    let err = build(
        &definition(LanguageId::Luau, BackendRequirement::Luau),
        &spec(vec![
            ScriptStepCall::Action {
                id: "dup".to_owned(),
                name: "trackInfo".to_owned(),
                args: json!({}),
                target: None,
            },
            ScriptStepCall::Workflow {
                id: "dup".to_owned(),
                reference: ScriptWorkflowRef {
                    language_id: LanguageId::Yaml,
                    version: 1,
                    source_digest: "ab".repeat(32),
                    path: String::new(),
                },
                args: Map::new(),
            },
        ]),
        AvailableBackends::luau(),
    )
    .unwrap_err();
    assert!(
        matches!(err, ScriptApiError::DuplicateStepId { .. }),
        "{err:?}"
    );
}

#[test]
fn rhai_poc_spec_builds_same_document_shape() {
    // PoC-level parity: a Rhai definition lowers through the identical
    // builder — semantics conformance is fixture-level only; product
    // Rhai support remains deferred (ruviform#559 gate).
    let doc = build(
        &definition(LanguageId::Rhai, BackendRequirement::Rhai),
        &spec(vec![ScriptStepCall::Action {
            id: "info".to_owned(),
            name: "trackInfo".to_owned(),
            args: json!({}),
            target: None,
        }]),
        AvailableBackends::luau_and_rhai(),
    )
    .expect("rhai poc spec builds");
    assert_eq!(doc["steps"][0]["kind"], json!("query"));
    assert_eq!(
        doc["steps"][0]["capability"],
        json!("synvelis.query.track-info@1.0")
    );
}
