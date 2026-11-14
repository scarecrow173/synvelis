//! SV-SCRIPT-03 / synvelis#273 — heterogeneous workflow import:
//! script specs → import document → [`WorkflowDefinition`] →
//! [`PlanCompiler`], plus the headless YAML/script plan lanes and the
//! fail-closed reference matrix.
//!
//! The script-side fixtures build a minimal [`ScriptApiModel`]/
//! [`BindingSet`] over two gateway capabilities whose ids match the
//! automation `common` registry, so `action()` pins flow into the same
//! `id@major.minor` refs the YAML DSL writes literally.

mod common;

use std::collections::BTreeMap;

use serde_json::{Map, Value, json};
use synvelis_automation::{
    AutomationError, CompileError, FileWorkflowResolver, ImportError, ImportedWorkflow,
    ImportedWorkflowPayload, MacroRegistry, MapWorkflowResolver, PlanCompiler, WorkflowDefinition,
    WorkflowResolver, WorkflowSourceKind, compile_workflow_yaml, import_workflow_document,
    workflow_source_kind,
};
use synvelis_gateway::{
    Availability, CapabilityDescriptor, CapabilityId, CapabilityRef, CapabilityRegistry,
    CapabilityVersion, DataAccessClass, DataAccessSet, EffectKind, EffectSet, ExecutionContract,
    OperationDomain, PermissionKind, PermissionSet, ResourceKind, SchemaRef, SchemaRegistry,
    SchemaVersion, TargetContract, ValueSchema,
};
use synvelis_script::{
    AvailableBackends, BackendRequirement, BindingSet, LanguageId,
    RegistrySnapshot as ScriptSnapshot, SURFACE_SCHEMA_VERSION, ScriptApiModel, ScriptStepCall,
    ScriptSurfaceEntry, ScriptSurfaceSpec, ScriptVisibility, ScriptWorkflowRef, ScriptWorkflowSpec,
    SpecIoDecl, WORKFLOW_SCRIPT_DEFINITION_SCHEMA_VERSION, WORKFLOW_SPEC_SCHEMA_VERSION,
    WorkflowScriptBuilder, WorkflowScriptDefinition,
};

use crate::common::{BASIC_WORKFLOW, definition, registry};

// ---------------------------------------------------------------------------
// Script-side fixtures (minimal model: one query + one mutation cap)
// ---------------------------------------------------------------------------

fn vid(text: &str) -> CapabilityId {
    CapabilityId::new(text).expect("fixture capability id")
}

fn sref(name: &str) -> SchemaRef {
    SchemaRef::new(name, 1, 0)
}

fn descriptor(
    id: &str,
    domain: OperationDomain,
    input: &str,
    output: &str,
    target: TargetContract,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: vid(id),
        version: CapabilityVersion::new(1, 0),
        domain,
        input_schema: sref(input),
        output_schema: sref(output),
        target,
        effect_bound: EffectSet::of(vec![EffectKind::Read]),
        required_permissions: PermissionSet::of(vec![PermissionKind::ReadContext]),
        data_access: DataAccessSet::of(vec![DataAccessClass::MetadataRead]),
        execution: ExecutionContract::Sync,
        availability: Availability::Available,
    }
}

fn empty_object_schema() -> ValueSchema {
    ValueSchema::Object {
        fields: BTreeMap::new(),
        deny_unknown: true,
    }
}

/// The minimal script API surface: `queryTracks` →
/// `synvelis.query.tracks@1.0` (Query/Sync/None) and `addNote` →
/// `synvelis.project.add-note@1.0` (ProjectMutation/Sync/Resource).
fn script_fixtures() -> (ScriptApiModel, BindingSet) {
    let mut caps = CapabilityRegistry::new();
    let mut schemas = SchemaRegistry::new();
    for name in [
        "args.empty",
        "args.query-tracks",
        "args.add-note",
        "out.empty",
        "out.track-list",
        "out.note-id",
    ] {
        schemas
            .register(name, SchemaVersion::new(1, 0), empty_object_schema())
            .expect("fixture schema registers");
    }
    caps.register(
        descriptor(
            "synvelis.query.tracks",
            OperationDomain::Query,
            "args.query-tracks",
            "out.track-list",
            TargetContract::None,
        ),
        None,
    )
    .expect("query cap registers");
    caps.register(
        descriptor(
            "synvelis.project.add-note",
            OperationDomain::ProjectMutation,
            "args.add-note",
            "out.note-id",
            TargetContract::Resource {
                kind: ResourceKind::Track,
            },
        ),
        None,
    )
    .expect("mutation cap registers");
    let snapshot = ScriptSnapshot::capture(&caps, &schemas).expect("snapshot captures");
    let entry = |name: &str, id: &str| ScriptSurfaceEntry {
        name: name.to_owned(),
        capability: CapabilityRef {
            id: vid(id),
            version: CapabilityVersion::new(1, 0),
        },
        visibility: ScriptVisibility::Public,
        description: None,
    };
    let surface = ScriptSurfaceSpec {
        schema_version: SURFACE_SCHEMA_VERSION,
        entries: vec![
            entry("queryTracks", "synvelis.query.tracks"),
            entry("addNote", "synvelis.project.add-note"),
        ],
    };
    let model = ScriptApiModel::build(&snapshot, &surface).expect("model builds");
    let bindings = BindingSet::generate(&model).expect("bindings generate");
    (model, bindings)
}

fn luau_definition() -> WorkflowScriptDefinition {
    WorkflowScriptDefinition {
        schema_version: WORKFLOW_SCRIPT_DEFINITION_SCHEMA_VERSION,
        script_id: "test.basic-luau".to_owned(),
        version: 1,
        language_id: LanguageId::Luau,
        backend_requirement: BackendRequirement::Luau,
        source_digest: "ab".repeat(32),
    }
}

fn limits_json() -> Value {
    json!({
        "max_steps": 100,
        "max_depth": 8,
        "max_iterations": 1000,
        "max_runtime_ms": 60_000,
        "max_output_bytes": 1_048_576,
        "max_memory_bytes": 134_217_728,
    })
}

fn io(name: &str, schema: Value) -> SpecIoDecl {
    SpecIoDecl {
        name: name.to_owned(),
        schema,
    }
}

/// The Luau spec semantically identical to [`BASIC_WORKFLOW`].
fn basic_spec() -> ScriptWorkflowSpec {
    ScriptWorkflowSpec {
        schema_version: WORKFLOW_SPEC_SCHEMA_VERSION,
        workflow_id: "test.basic".to_owned(),
        version: 1,
        inputs: vec![io("track", json!({"type": "id", "kind": "track"}))],
        steps: vec![
            ScriptStepCall::Action {
                id: "tracks".to_owned(),
                name: "queryTracks".to_owned(),
                args: json!({"limit": 10}),
                target: None,
            },
            ScriptStepCall::Action {
                id: "add".to_owned(),
                name: "addNote".to_owned(),
                args: json!({
                    "pitch": 60,
                    "velocity": 100,
                    "start_tick": 0,
                    "duration_ticks": 480,
                }),
                target: Some(json!("$input.track")),
            },
            ScriptStepCall::Emit {
                id: "done".to_owned(),
                output: "note".to_owned(),
                value: json!("$steps.add"),
            },
        ],
        outputs: vec![io("note", json!({"type": "id", "kind": "note"}))],
        limits: limits_json(),
    }
}

fn empty_inputs() -> Map<String, Value> {
    Map::new()
}

// ---------------------------------------------------------------------------
// Golden: YAML and Luau yield the same definition and plan
// ---------------------------------------------------------------------------

#[test]
fn yaml_and_luau_specs_yield_same_definition_and_plan() {
    let yaml_def = definition(BASIC_WORKFLOW);
    let (model, bindings) = script_fixtures();
    let backends = AvailableBackends::luau();

    // Script path: builder → import document → WorkflowDefinition.
    let document = WorkflowScriptBuilder::build(
        &luau_definition(),
        &basic_spec(),
        &backends,
        &model,
        &bindings,
    )
    .expect("script spec builds");
    let script_def = import_workflow_document(&document, &MapWorkflowResolver::new())
        .expect("import document lowers");

    assert_eq!(
        script_def, yaml_def,
        "script and YAML definitions must be identical"
    );

    // Same definition → same plan digest through the single compiler.
    let yaml_inputs = serde_json::from_value::<Map<String, Value>>(json!({
        "track": "track-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    }))
    .unwrap();
    let yaml_plan =
        PlanCompiler::compile(&yaml_def, &yaml_inputs, &registry()).expect("yaml plan compiles");
    let script_plan = PlanCompiler::compile(&script_def, &yaml_inputs, &registry())
        .expect("script plan compiles");
    assert_eq!(
        yaml_plan.plan.digest().unwrap(),
        script_plan.plan.digest().unwrap()
    );
    assert_eq!(
        yaml_plan.plan.step_order, script_plan.plan.step_order,
        "step order must match"
    );
}

#[test]
fn compile_workflow_yaml_matches_direct_compile() {
    let inputs: Map<String, Value> = serde_json::from_value(json!({
        "track": "track-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    }))
    .unwrap();
    let direct = common::compile(
        BASIC_WORKFLOW,
        json!({
            "track": "track-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        }),
    )
    .expect("direct compile");
    let headless = compile_workflow_yaml(
        BASIC_WORKFLOW,
        &inputs,
        &registry(),
        &MacroRegistry::default(),
    )
    .expect("headless yaml compile");
    assert_eq!(
        direct.plan.digest().unwrap(),
        headless.plan.digest().unwrap()
    );
}

// ---------------------------------------------------------------------------
// Heterogeneous multi-stage references
// ---------------------------------------------------------------------------

/// A minimal import document wrapping raw step JSON — used for
/// resolver payloads that are still documents (script-language refs).
fn doc_json(workflow_id: &str, version: u32, inputs: Value, steps: Value, outputs: Value) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("schema_version".to_owned(), json!(1));
    map.insert("workflow_id".to_owned(), json!(workflow_id));
    map.insert("version".to_owned(), json!(version));
    map.insert("inputs".to_owned(), inputs);
    map.insert("steps".to_owned(), steps);
    map.insert("outputs".to_owned(), outputs);
    map.insert("limits".to_owned(), limits_json());
    Value::Object(map)
}

fn digest_of_bytes(seed: &str) -> String {
    synvelis_script::sha256_hex(seed.as_bytes())
}

fn workflow_ref(language: LanguageId, version: u32, seed: &str, path: &str) -> ScriptWorkflowRef {
    ScriptWorkflowRef {
        language_id: language,
        version,
        source_digest: digest_of_bytes(seed),
        path: path.to_owned(),
    }
}

fn yaml_payload(yaml_src: &str, seed: &str, path: &str) -> (ScriptWorkflowRef, ImportedWorkflow) {
    let def = definition(yaml_src);
    let reference = workflow_ref(LanguageId::Yaml, def.version, seed, path);
    let actual = ImportedWorkflow {
        language_id: LanguageId::Yaml,
        version: def.version,
        source_digest: digest_of_bytes(seed),
        payload: ImportedWorkflowPayload::Definition(Box::new(def)),
    };
    (reference, actual)
}

/// Leaf YAML workflow: query → emit `picked` (the whole track list).
const LEAF_YAML: &str = r"
schema_version: 1
workflow_id: test.leaf
version: 3
inputs:
  - name: track
    schema:
      type: id
      kind: track
steps:
  - kind: query
    id: tracks
    capability: synvelis.query.tracks@1.0
    args:
      limit: 10
  - kind: emit-result
    id: done
    output: picked
    value: $steps.tracks
outputs:
  - name: picked
    schema:
      type: array
      max_len: 1024
      item:
        type: object
        deny_unknown: true
        fields:
          id:
            schema:
              type: id
              kind: track
            required: true
          name:
            schema:
              type: string
              max_len: 256
            required: true
limits:
  max_steps: 100
  max_depth: 8
  max_iterations: 1000
  max_runtime_ms: 60000
  max_output_bytes: 1048576
  max_memory_bytes: 134217728
";

/// Stage B — Rhai document referencing the YAML leaf. Its `mid`
/// input feeds the leaf's `track` input; its `picked` output
/// re-emits the leaf's virtual output.
fn rhai_mid_doc(yaml_ref: &ScriptWorkflowRef) -> (Value, ScriptWorkflowRef) {
    let doc = doc_json(
        "test.mid",
        2,
        json!([{"name": "mid", "schema": {"type": "id", "kind": "track"}}]),
        json!([{
            "kind": "workflow",
            "id": "leaf",
            "reference": reference_json(yaml_ref),
            "args": {"track": "$input.mid"},
        }, {
            "kind": "emit-result",
            "id": "emit",
            "output": "picked",
            "value": "$steps.leaf.picked",
        }]),
        json!([{"name": "picked", "schema": {"type": "array", "max_len": 1024, "item": {"type": "object", "deny_unknown": true, "fields": {"id": {"schema": {"type": "id", "kind": "track"}, "required": true}, "name": {"schema": {"type": "string", "max_len": 256}, "required": true}}}}}]),
    );
    let reference = ScriptWorkflowRef {
        language_id: LanguageId::Rhai,
        version: 2,
        source_digest: digest_of_bytes("rhai-src"),
        path: "mid.rhai".to_owned(),
    };
    (doc, reference)
}

/// Stage A — Luau document (via the real builder path) referencing
/// the Rhai document; its `track` input flows through both hops.
fn luau_top_document(rhai_ref: &ScriptWorkflowRef) -> Value {
    let (model, bindings) = script_fixtures();
    let spec = ScriptWorkflowSpec {
        schema_version: WORKFLOW_SPEC_SCHEMA_VERSION,
        workflow_id: "test.top".to_owned(),
        version: 1,
        inputs: vec![io("track", json!({"type": "id", "kind": "track"}))],
        steps: vec![
            ScriptStepCall::Workflow {
                id: "mid".to_owned(),
                reference: rhai_ref.clone(),
                args: Map::from_iter([("mid".to_owned(), json!("$input.track"))]),
            },
            ScriptStepCall::Emit {
                id: "done".to_owned(),
                output: "picked".to_owned(),
                value: json!("$steps.mid.picked"),
            },
        ],
        outputs: vec![io(
            "picked",
            json!({"type": "array", "max_len": 1024, "item": {"type": "object", "deny_unknown": true, "fields": {"id": {"schema": {"type": "id", "kind": "track"}, "required": true}, "name": {"schema": {"type": "string", "max_len": 256}, "required": true}}}}),
        )],
        limits: limits_json(),
    };
    WorkflowScriptBuilder::build(
        &luau_definition(),
        &spec,
        &AvailableBackends::luau_and_rhai(),
        &model,
        &bindings,
    )
    .expect("top spec builds")
}

#[test]
fn heterogeneous_chain_luau_rhai_yaml_inlines_and_compiles() {
    // Stage C — YAML leaf (Definition payload).
    let (yaml_ref, _) = yaml_payload(LEAF_YAML, "leaf-src", "leaf.yaml");
    // Stage B — Rhai document + its reference pin.
    let (rhai_doc, rhai_ref) = rhai_mid_doc(&yaml_ref);
    // Stage A — Luau document built through the real builder path.
    let document = luau_top_document(&rhai_ref);

    let mut resolver = MapWorkflowResolver::new();
    resolver.insert(
        &yaml_ref,
        ImportedWorkflow {
            language_id: LanguageId::Yaml,
            version: 3,
            source_digest: yaml_ref.source_digest.clone(),
            payload: ImportedWorkflowPayload::Definition(Box::new(definition(LEAF_YAML))),
        },
    );
    resolver.insert(
        &rhai_ref,
        ImportedWorkflow {
            language_id: LanguageId::Rhai,
            version: 2,
            source_digest: rhai_ref.source_digest.clone(),
            payload: ImportedWorkflowPayload::Document(Box::new(rhai_doc)),
        },
    );

    let def = import_workflow_document(&document, &resolver).expect("chain imports");

    // Leaf step inlined twice-namespaced: top call `mid` → rhai doc's
    // call `leaf` → yaml step `tracks`. The parent's emit binds the
    // propagated virtual output to the namespaced leaf step.
    let queries: Vec<&str> = def
        .steps
        .iter()
        .filter_map(|s| match s {
            synvelis_automation::Step::Query { id, .. } => Some(id.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(queries, ["mid__leaf__tracks"]);
    let emit = def
        .steps
        .iter()
        .find_map(|s| match s {
            synvelis_automation::Step::EmitResult { value, .. } => Some(value),
            _ => None,
        })
        .expect("top emit exists");
    assert!(
        matches!(
            emit,
            synvelis_automation::Expr::StepOutput { step_output }
                if step_output.step == "mid__leaf__tracks"
        ),
        "emit must reference the namespaced leaf step: {emit:?}"
    );

    let inputs: Map<String, Value> = serde_json::from_value(json!({
        "track": "track-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
    }))
    .unwrap();
    let plan = PlanCompiler::compile(&def, &inputs, &registry()).expect("chain compiles");
    assert_eq!(
        plan.plan.step_order,
        vec!["mid__leaf__tracks".to_owned(), "done".to_owned()]
    );
}

// ---------------------------------------------------------------------------
// Fail-closed reference matrix
// ---------------------------------------------------------------------------

fn import_err_of(result: Result<WorkflowDefinition, AutomationError>) -> ImportError {
    match result {
        Err(AutomationError::Import(e)) => e,
        other => panic!("expected ImportError, got {other:?}"),
    }
}

fn parent_doc_with_call(reference: Value, args: Value) -> Value {
    let mut call = serde_json::Map::new();
    call.insert("kind".to_owned(), json!("workflow"));
    call.insert("id".to_owned(), json!("sub"));
    call.insert("reference".to_owned(), reference);
    call.insert("args".to_owned(), args);
    doc_json(
        "test.parent",
        1,
        json!([]),
        json!([Value::Object(call)]),
        json!([]),
    )
}

fn reference_json(reference: &ScriptWorkflowRef) -> Value {
    serde_json::to_value(reference).unwrap()
}

#[test]
fn unknown_workflow_rejected() {
    let reference = workflow_ref(LanguageId::Yaml, 1, "missing", "none.yaml");
    let doc = parent_doc_with_call(reference_json(&reference), json!({}));
    let err = import_err_of(import_workflow_document(&doc, &MapWorkflowResolver::new()));
    assert!(
        matches!(err, ImportError::UnknownWorkflow { .. }),
        "{err:?}"
    );
}

#[test]
fn import_cycle_rejected() {
    // A references B; B references A — resolution stack must catch it.
    let ref_b = workflow_ref(LanguageId::Luau, 1, "b-src", "b.luau");
    let ref_a = workflow_ref(LanguageId::Luau, 1, "a-src", "a.luau");
    let doc_b = doc_json(
        "test.b",
        1,
        json!([]),
        json!([{
            "kind": "workflow",
            "id": "a",
            "reference": reference_json(&ref_a),
            "args": {},
        }]),
        json!([]),
    );
    let doc_a = doc_json(
        "test.a",
        1,
        json!([]),
        json!([{
            "kind": "workflow",
            "id": "b",
            "reference": reference_json(&ref_b),
            "args": {},
        }]),
        json!([]),
    );
    let mut resolver = MapWorkflowResolver::new();
    resolver.insert(
        &ref_b,
        ImportedWorkflow {
            language_id: LanguageId::Luau,
            version: 1,
            source_digest: ref_b.source_digest.clone(),
            payload: ImportedWorkflowPayload::Document(Box::new(doc_b)),
        },
    );
    resolver.insert(
        &ref_a,
        ImportedWorkflow {
            language_id: LanguageId::Luau,
            version: 1,
            source_digest: ref_a.source_digest.clone(),
            payload: ImportedWorkflowPayload::Document(Box::new(doc_a.clone())),
        },
    );
    let err = import_err_of(import_workflow_document(&doc_a, &resolver));
    assert!(matches!(err, ImportError::ImportCycle { .. }), "{err:?}");
}

#[test]
fn version_source_drift_and_language_mismatch_rejected() {
    let reference = workflow_ref(LanguageId::Yaml, 1, "src", "w.yaml");
    let doc = || parent_doc_with_call(reference_json(&reference), json!({}));
    // LEAF_YAML declares version 3; align the resolved definition's
    // version to the pin so only the probed field differs.
    let mut pinned_def = definition(LEAF_YAML);
    pinned_def.version = 1;

    let resolver_with = |language: LanguageId, version: u32, digest: String| {
        let mut resolver = MapWorkflowResolver::new();
        resolver.insert(
            &reference,
            ImportedWorkflow {
                language_id: language,
                version,
                source_digest: digest,
                payload: ImportedWorkflowPayload::Definition(Box::new(pinned_def.clone())),
            },
        );
        resolver
    };

    let err = import_err_of(import_workflow_document(
        &doc(),
        &resolver_with(LanguageId::Yaml, 2, reference.source_digest.clone()),
    ));
    assert!(
        matches!(err, ImportError::VersionMismatch { .. }),
        "{err:?}"
    );

    let err = import_err_of(import_workflow_document(
        &doc(),
        &resolver_with(LanguageId::Yaml, 1, digest_of_bytes("different-src")),
    ));
    assert!(matches!(err, ImportError::SourceDrift { .. }), "{err:?}");

    let err = import_err_of(import_workflow_document(
        &doc(),
        &resolver_with(LanguageId::Luau, 1, reference.source_digest.clone()),
    ));
    assert!(
        matches!(err, ImportError::LanguageMismatch { .. }),
        "{err:?}"
    );
}

#[test]
fn typed_io_mismatch_rejected() {
    // Referenced def declares input `track`; call provides none.
    let reference = workflow_ref(LanguageId::Yaml, 3, "leaf-src", "leaf.yaml");
    let mut resolver = MapWorkflowResolver::new();
    resolver.insert(
        &reference,
        ImportedWorkflow {
            language_id: LanguageId::Yaml,
            version: 3,
            source_digest: reference.source_digest.clone(),
            payload: ImportedWorkflowPayload::Definition(Box::new(definition(LEAF_YAML))),
        },
    );
    let doc = parent_doc_with_call(reference_json(&reference), json!({}));
    let err = import_err_of(import_workflow_document(&doc, &resolver));
    assert!(
        matches!(err, ImportError::WorkflowInputMismatch { .. }),
        "{err:?}"
    );

    // Call provides an undeclared input alongside the required one.
    let doc = parent_doc_with_call(
        reference_json(&reference),
        json!({"track": "track-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "bogus": 1}),
    );
    let err = import_err_of(import_workflow_document(&doc, &resolver));
    assert!(
        matches!(err, ImportError::WorkflowInputMismatch { .. }),
        "{err:?}"
    );
}

#[test]
fn unknown_workflow_output_rejected() {
    // Parent references `$steps.sub.nope` — `sub` resolves but has no
    // such declared output.
    let reference = workflow_ref(LanguageId::Yaml, 3, "leaf-src", "leaf.yaml");
    let mut resolver = MapWorkflowResolver::new();
    resolver.insert(
        &reference,
        ImportedWorkflow {
            language_id: LanguageId::Yaml,
            version: 3,
            source_digest: reference.source_digest.clone(),
            payload: ImportedWorkflowPayload::Definition(Box::new(definition(LEAF_YAML))),
        },
    );
    let doc = doc_json(
        "test.parent",
        1,
        json!([]),
        json!([{
            "kind": "workflow",
            "id": "sub",
            "reference": reference_json(&reference),
            "args": {"track": "track-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
        }, {
            "kind": "emit-result",
            "id": "done",
            "output": "out",
            "value": "$steps.sub.nope",
        }]),
        json!([{"name": "out", "schema": {"type": "string"}}]),
    );
    let err = import_err_of(import_workflow_document(&doc, &resolver));
    assert!(
        matches!(err, ImportError::UnknownWorkflowOutput { .. }),
        "{err:?}"
    );
}

#[test]
fn missing_declared_output_rejected() {
    // Referenced def declares `picked` but never emits it.
    let mut def = definition(LEAF_YAML);
    def.steps
        .retain(|s| !matches!(s, synvelis_automation::Step::EmitResult { .. }));
    let reference = workflow_ref(LanguageId::Yaml, 3, "no-emit", "noemit.yaml");
    let mut resolver = MapWorkflowResolver::new();
    resolver.insert(
        &reference,
        ImportedWorkflow {
            language_id: LanguageId::Yaml,
            version: 3,
            source_digest: reference.source_digest.clone(),
            payload: ImportedWorkflowPayload::Definition(Box::new(def)),
        },
    );
    let doc = parent_doc_with_call(
        reference_json(&reference),
        json!({"track": "track-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}),
    );
    let err = import_err_of(import_workflow_document(&doc, &resolver));
    assert!(
        matches!(err, ImportError::MissingWorkflowOutput { .. }),
        "{err:?}"
    );
}

#[test]
fn duplicate_step_id_rejected() {
    let doc = doc_json(
        "test.dup",
        1,
        json!([]),
        json!([
            {"kind": "query", "id": "s", "capability": "synvelis.query.tracks@1.0", "args": {}},
            {"kind": "query", "id": "s", "capability": "synvelis.query.tracks@1.0", "args": {}},
        ]),
        json!([]),
    );
    let err = import_err_of(import_workflow_document(&doc, &MapWorkflowResolver::new()));
    assert!(
        matches!(err, ImportError::DuplicateStepId { .. }),
        "{err:?}"
    );
}

#[test]
fn import_depth_exceeded_rejected() {
    // Chain of documents each referencing the next; depth cap is 32.
    let mut resolver = MapWorkflowResolver::new();
    let mut refs = Vec::new();
    for i in (0..40).rev() {
        let seed = format!("depth-{i}");
        let reference = workflow_ref(LanguageId::Luau, 1, &seed, &format!("d{i}.luau"));
        refs.push((i, reference.clone()));
        let steps = if i + 1 < 40 {
            let next = &refs[refs.len() - 2].1; // previous iteration's ref = i+1
            json!([{
                "kind": "workflow",
                "id": "next",
                "reference": reference_json(next),
                "args": {},
            }])
        } else {
            json!([])
        };
        let payload = doc_json(&format!("test.d{i}"), 1, json!([]), steps, json!([]));
        resolver.insert(
            &reference,
            ImportedWorkflow {
                language_id: LanguageId::Luau,
                version: 1,
                source_digest: reference.source_digest.clone(),
                payload: ImportedWorkflowPayload::Document(Box::new(payload)),
            },
        );
    }
    let top = doc_json(
        "test.top",
        1,
        json!([]),
        json!([{
            "kind": "workflow",
            "id": "d0",
            "reference": reference_json(&refs[39].1),
            "args": {},
        }]),
        json!([]),
    );
    let err = import_err_of(import_workflow_document(&top, &resolver));
    assert!(
        matches!(err, ImportError::ImportDepthExceeded { .. }),
        "{err:?}"
    );
}

#[test]
fn file_resolver_path_escape_and_unsupported_source() {
    let root = std::env::temp_dir();
    let resolver = FileWorkflowResolver::new(&root).expect("temp dir canonicalizes");
    let escape = ScriptWorkflowRef {
        language_id: LanguageId::Yaml,
        version: 1,
        source_digest: digest_of_bytes("x"),
        path: "../escape.yaml".to_owned(),
    };
    let err = import_err_of(import_workflow_document(
        &parent_doc_with_call(reference_json(&escape), json!({})),
        &resolver,
    ));
    assert!(matches!(err, ImportError::PathEscape { .. }), "{err:?}");

    // Absolute path is also an escape.
    let absolute = ScriptWorkflowRef {
        path: root.join("abs.yaml").to_string_lossy().into_owned(),
        ..escape.clone()
    };
    let err = import_err_of(import_workflow_document(
        &parent_doc_with_call(reference_json(&absolute), json!({})),
        &resolver,
    ));
    assert!(matches!(err, ImportError::PathEscape { .. }), "{err:?}");
}

#[test]
fn file_resolver_loads_yaml_and_verifies_digest() {
    let dir = std::env::temp_dir().join(format!("sv273-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("leaf.yaml");
    std::fs::write(&file, LEAF_YAML).unwrap();

    let bytes = std::fs::read(&file).unwrap();
    let digest = synvelis_script::sha256_hex(&bytes);
    let reference = ScriptWorkflowRef {
        language_id: LanguageId::Yaml,
        version: 3,
        source_digest: digest.clone(),
        path: "leaf.yaml".to_owned(),
    };
    let resolver = FileWorkflowResolver::new(&dir).unwrap();
    let imported = resolver.resolve(&reference).expect("file resolves");
    assert_eq!(imported.language_id, LanguageId::Yaml);
    assert_eq!(imported.version, 3);
    assert_eq!(imported.source_digest, digest);
    assert!(matches!(
        imported.payload,
        ImportedWorkflowPayload::Definition(_)
    ));

    // End-to-end: parent doc importing through the file resolver.
    let doc = parent_doc_with_call(
        reference_json(&reference),
        json!({"track": "track-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}),
    );
    let def = import_workflow_document(&doc, &resolver).expect("file import lowers");
    let inputs: Map<String, Value> = empty_inputs();
    let plan = PlanCompiler::compile(&def, &inputs, &registry()).expect("file plan compiles");
    assert_eq!(plan.plan.step_order, vec!["sub__tracks".to_owned()]);

    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn workflow_source_kind_classifies() {
    assert_eq!(
        workflow_source_kind(std::path::Path::new("a/b.WORKFLOW.YAML")).unwrap(),
        WorkflowSourceKind::Yaml
    );
    assert_eq!(
        workflow_source_kind(std::path::Path::new("w.luau")).unwrap(),
        WorkflowSourceKind::Luau
    );
    assert_eq!(
        workflow_source_kind(std::path::Path::new("w.rhai")).unwrap(),
        WorkflowSourceKind::Rhai
    );
    let err = workflow_source_kind(std::path::Path::new("w.txt")).unwrap_err();
    assert!(matches!(
        err,
        AutomationError::Import(ImportError::UnsupportedSource { .. })
    ));
}

#[test]
fn virtual_outputs_do_not_leak_past_control_flow() {
    // `sub` is called inside an `if` branch; referencing its virtual
    // output afterwards must fail closed (unknown step id), never
    // silently resolve.
    let reference = workflow_ref(LanguageId::Yaml, 3, "leaf-src", "leaf.yaml");
    let mut resolver = MapWorkflowResolver::new();
    resolver.insert(
        &reference,
        ImportedWorkflow {
            language_id: LanguageId::Yaml,
            version: 3,
            source_digest: reference.source_digest.clone(),
            payload: ImportedWorkflowPayload::Definition(Box::new(definition(LEAF_YAML))),
        },
    );
    let doc = doc_json(
        "test.scoped",
        1,
        json!([]),
        json!([{
            "kind": "if",
            "id": "branch",
            "condition": {"op": "eq", "left": 1, "right": 1},
            "then": [{
                "kind": "workflow",
                "id": "sub",
                "reference": reference_json(&reference),
                "args": {"track": "track-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"},
            }],
        }, {
            "kind": "emit-result",
            "id": "done",
            "output": "out",
            "value": "$steps.sub.picked",
        }]),
        json!([{"name": "out", "schema": {"type": "string"}}]),
    );
    let def = import_workflow_document(&doc, &resolver).expect("import lowers");
    let err = PlanCompiler::compile(&def, &empty_inputs(), &registry()).unwrap_err();
    assert!(
        matches!(
            err,
            AutomationError::Compile(CompileError::UnknownStepId { .. })
        ),
        "{err:?}"
    );
}

#[test]
fn forward_step_reference_still_rejected_by_compiler() {
    // Importer inlining must not weaken the compiler's future-step
    // check: `$steps.later` before `later` is defined rejects.
    let doc = doc_json(
        "test.fwd",
        1,
        json!([]),
        json!([
            {"kind": "query", "id": "early", "capability": "synvelis.query.tracks@1.0",
             "args": {"filter": "$steps.later"}},
            {"kind": "query", "id": "later", "capability": "synvelis.query.tracks@1.0", "args": {}},
        ]),
        json!([]),
    );
    let def = import_workflow_document(&doc, &MapWorkflowResolver::new()).expect("import lowers");
    let err = PlanCompiler::compile(&def, &empty_inputs(), &registry()).unwrap_err();
    assert!(
        matches!(
            err,
            AutomationError::Compile(CompileError::ForwardStepReference { .. })
        ),
        "{err:?}"
    );
}
