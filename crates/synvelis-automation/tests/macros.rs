//! AUTX-04 (`synvelis#263`) — `AtomicMacroDefinition` decode/validate,
//! `MacroCompiler` recipe → batch `ops[]` lowering, output-binding
//! placeholders, `MacroRegistry` + `PlanCompiler` `atomic-macro`
//! expansion, and the fail-closed reject matrix.

mod common;

use serde_json::{Map, Value, json};
use synvelis_automation::{
    AtomicMacroDefinition, AutomationError, CompileError, MacroCompiler, MacroRegistry,
    OUTPUT_BINDING_ID_BASE, PlanCompiler, RegistrySnapshot, decode_macro_definition, macro_digest,
    parse_macro_yaml, parse_registry_snapshot_yaml,
};

use crate::common::{compile, definition, registry};

// ---------------------------------------------------------------------------
// Fixture registry — project-mutation capabilities a macro may name,
// plus domain/execution/availability/batch probes.
// ---------------------------------------------------------------------------

const MACRO_REGISTRY_YAML: &str = r"
schema_version: 1
capabilities:
  - id: synvelis.macro.track.create
    version:
      major: 1
      minor: 0
    domain: project-mutation
    input_schema: synvelis.macro.args.track-create@1.0
    output_schema: synvelis.macro.out.empty@1.0
    target:
      shape: create
      parent: project
      creates: track
    effect_bound:
      - create
      - write
    required_permissions:
      - mutate-project
    data_access:
      - metadata-read
    execution: sync
    availability:
      state: available
  - id: synvelis.macro.lane.create
    version:
      major: 1
      minor: 0
    domain: project-mutation
    input_schema: synvelis.macro.args.empty@1.0
    output_schema: synvelis.macro.out.empty@1.0
    target:
      shape: create
      parent: track
      creates: lane
    effect_bound:
      - create
      - write
    required_permissions:
      - mutate-project
    data_access:
      - metadata-read
    execution: sync
    availability:
      state: available
  - id: synvelis.macro.clip.bind
    version:
      major: 1
      minor: 0
    domain: project-mutation
    input_schema: synvelis.macro.args.clip-bind@1.0
    output_schema: synvelis.macro.out.empty@1.0
    target:
      shape: resource
      kind: lane
    effect_bound:
      - write
    required_permissions:
      - mutate-project
    data_access:
      - metadata-read
    execution: sync
    availability:
      state: available
  - id: synvelis.macro.query.tracks
    version:
      major: 1
      minor: 0
    domain: query
    input_schema: synvelis.macro.args.empty@1.0
    output_schema: synvelis.macro.out.empty@1.0
    target:
      shape: none
    effect_bound:
      - read
    required_permissions:
      - read-context
    data_access:
      - metadata-read
    execution: sync
    availability:
      state: available
  - id: synvelis.macro.job.render
    version:
      major: 1
      minor: 0
    domain: async-operation
    input_schema: synvelis.macro.args.empty@1.0
    output_schema: synvelis.macro.out.empty@1.0
    target:
      shape: none
    effect_bound:
      - read
      - external-action
    required_permissions:
      - run-job
    data_access:
      - metadata-read
    execution: async-accept
    availability:
      state: available
  - id: synvelis.macro.job.mutate
    version:
      major: 1
      minor: 0
    domain: project-mutation
    input_schema: synvelis.macro.args.empty@1.0
    output_schema: synvelis.macro.out.empty@1.0
    target:
      shape: none
    effect_bound:
      - write
    required_permissions:
      - mutate-project
    data_access:
      - metadata-read
    execution: async-accept
    availability:
      state: available
  - id: synvelis.macro.track.retired
    version:
      major: 1
      minor: 0
    domain: project-mutation
    input_schema: synvelis.macro.args.empty@1.0
    output_schema: synvelis.macro.out.empty@1.0
    target:
      shape: none
    effect_bound:
      - write
    required_permissions:
      - mutate-project
    data_access:
      - metadata-read
    execution: sync
    availability:
      state: retired
  - id: synvelis.operation.batch
    version:
      major: 1
      minor: 0
    domain: project-mutation
    input_schema: synvelis.macro.args.batch@1.0
    output_schema: synvelis.macro.out.empty@1.0
    target:
      shape: none
    effect_bound:
      - read
      - write
      - create
      - delete
    required_permissions:
      - mutate-project
    data_access:
      - metadata-read
    execution: sync
    availability:
      state: available
schemas:
  - name: synvelis.macro.args.empty
    version:
      major: 1
      minor: 0
    schema:
      type: object
      deny_unknown: true
  - name: synvelis.macro.args.track-create
    version:
      major: 1
      minor: 0
    schema:
      type: object
      deny_unknown: true
      fields:
        name:
          schema:
            type: string
            max_len: 256
          required: true
  - name: synvelis.macro.args.clip-bind
    version:
      major: 1
      minor: 0
    schema:
      type: object
      deny_unknown: true
      fields:
        clip:
          schema:
            type: id
            kind: clip
          required: true
  - name: synvelis.macro.args.batch
    version:
      major: 1
      minor: 0
    schema:
      type: object
      deny_unknown: true
      fields:
        ops:
          schema:
            type: array
            max_len: 64
            item:
              type: object
              deny_unknown: false
          required: true
  - name: synvelis.macro.out.empty
    version:
      major: 1
      minor: 0
    schema:
      type: object
      deny_unknown: true
";

fn macro_registry() -> RegistrySnapshot {
    parse_registry_snapshot_yaml(MACRO_REGISTRY_YAML).expect("macro registry must parse")
}

// ---------------------------------------------------------------------------
// Definition decode / YAML frontend
// ---------------------------------------------------------------------------

/// A minimal two-step macro: create a track, then create a lane under
/// the track named by the `created` output binding. Targets are
/// expressions — the `create` shape's parent/creates kinds come from
/// the capability contract, never the author.
const TRACK_LANE_MACRO_YAML: &str = r"
schema_version: 1
macro_id: test.track-lane
version: 1
parameters:
  - name: track_name
    schema:
      type: string
      max_len: 256
  - name: project
    schema:
      type: id
      kind: project
steps:
  - id: mk_track
    capability: synvelis.macro.track.create@1.0
    args:
      name: $param.track_name
    target: $param.project
  - id: mk_lane
    capability: synvelis.macro.lane.create@1.0
    target: $output.created_track
output_bindings:
  - name: created_track
    step: mk_track
    kind: track
";

#[test]
fn macro_yaml_parses_and_validates() {
    let def = parse_macro_yaml(TRACK_LANE_MACRO_YAML).expect("macro YAML must parse");
    assert_eq!(def.macro_id, "test.track-lane");
    assert_eq!(def.steps.len(), 2);
    assert_eq!(def.output_bindings.len(), 1);
    assert_eq!(def.parameters.len(), 2);
}

#[test]
fn macro_definition_json_round_trip() {
    let def = decode_macro_definition(json!({
        "schema_version": 1,
        "macro_id": "test.simple",
        "version": 1,
        "parameters": [
            {"name": "greet", "schema": {"type": "string", "max_len": 32}}
        ],
        "steps": [
            {
                "id": "s1",
                "capability": "synvelis.macro.track.create@1.0",
                "args": {"expr": "object", "fields": {
                    "name": {"expr": "param", "path": ["greet"]}
                }},
                "target": {"expr": "literal", "value": "project-00000000000000000000000000000000"}
            }
        ],
        "output_bindings": [],
        "policy": {}
    }))
    .expect("definition must decode");
    assert_eq!(def.macro_ref().to_string(), "test.simple@1.0");
    // Authority fields cannot be expressed — deny_unknown_fields.
    let err = decode_macro_definition(json!({
        "schema_version": 1,
        "macro_id": "test.simple",
        "version": 1,
        "principal": "agent-1",
        "steps": [{"id": "s1", "capability": "synvelis.macro.track.create@1.0"}]
    }))
    .expect_err("authority field must reject");
    assert!(matches!(err, AutomationError::Json { .. }), "got {err:?}");
}

// ---------------------------------------------------------------------------
// Compile accept paths
// ---------------------------------------------------------------------------

fn track_lane_def() -> AtomicMacroDefinition {
    decode_macro_definition(json!({
        "schema_version": 1,
        "macro_id": "test.track-lane",
        "version": 1,
        "parameters": [
            {"name": "track_name", "schema": {"type": "string", "max_len": 256}},
            {"name": "project", "schema": {"type": "id", "kind": "project"}}
        ],
        "steps": [
            {
                "id": "mk_track",
                "capability": "synvelis.macro.track.create@1.0",
                "args": {"expr": "object", "fields": {
                    "name": {"expr": "param", "path": ["track_name"]}
                }},
                "target": {"expr": "param", "path": ["project"]}
            },
            {
                "id": "mk_lane",
                "capability": "synvelis.macro.lane.create@1.0",
                "target": {"expr": "output", "name": "created_track"}
            }
        ],
        "output_bindings": [
            {"name": "created_track", "step": "mk_track", "kind": "track"}
        ]
    }))
    .expect("fixture definition must decode")
}

fn inputs() -> Map<String, Value> {
    let mut m = Map::new();
    m.insert("track_name".to_owned(), json!("Verse"));
    m.insert(
        "project".to_owned(),
        json!("project-00000000000000000000000000000000"),
    );
    m
}

#[test]
fn compile_emits_flat_ops_with_placeholders() {
    let def = track_lane_def();
    let out = MacroCompiler::compile(&def, &inputs(), &macro_registry(), Some(7))
        .expect("compile must succeed");
    assert_eq!(out.schema_version, 1);
    assert_eq!(out.macro_ref, "test.track-lane@1.0");
    assert_eq!(out.expected_revision, Some(7));
    assert_eq!(out.ops.len(), 2);
    // Member 0: track.create with param-spliced name and param target.
    assert_eq!(
        out.ops[0]["capability"]["id"],
        "synvelis.macro.track.create"
    );
    assert_eq!(out.ops[0]["args"]["name"], "Verse");
    assert_eq!(
        out.ops[0]["target"]["parent"],
        "project-00000000000000000000000000000000"
    );
    assert_eq!(out.ops[0]["target"]["shape"], "create");
    // Member 1 references the binding — a placeholder id in the
    // reserved range, never a guessed real id.
    let placeholder = format!("track-{OUTPUT_BINDING_ID_BASE:032x}");
    assert_eq!(out.ops[1]["capability"]["id"], "synvelis.macro.lane.create");
    assert_eq!(out.ops[1]["target"]["parent"], placeholder);
    assert_eq!(out.ops[1]["target"]["shape"], "create");
    // Binding declaration points at member 0.
    assert_eq!(out.bindings.len(), 1);
    assert_eq!(out.bindings[0].name, "created_track");
    assert_eq!(out.bindings[0].member, 0);
    assert_eq!(out.bindings[0].kind.tag(), "track");
    // `batch_args` carries ops + bindings in the gateway wire shape.
    let args = out.batch_args().unwrap();
    assert_eq!(args["ops"].as_array().unwrap().len(), 2);
    assert_eq!(args["bindings"][0]["name"], "created_track");
    assert_eq!(args["bindings"][0]["member"], json!(0));
    assert_eq!(args["bindings"][0]["kind"], "track");
}

#[test]
fn compile_is_deterministic() {
    let def = track_lane_def();
    let a = MacroCompiler::compile(&def, &inputs(), &macro_registry(), Some(7)).unwrap();
    let b = MacroCompiler::compile(&def, &inputs(), &macro_registry(), Some(7)).unwrap();
    assert_eq!(a.digest().unwrap(), b.digest().unwrap());
    assert_eq!(a.definition_digest, macro_digest(&def).unwrap());
    assert_eq!(a.ops, b.ops);
}

// ---------------------------------------------------------------------------
// Definition validation rejects
// ---------------------------------------------------------------------------

fn def_json() -> Value {
    json!({
        "schema_version": 1,
        "macro_id": "test.v",
        "version": 1,
        "steps": [{"id": "s1", "capability": "synvelis.macro.track.create@1.0"}]
    })
}

fn expect_def_err(mutate: impl Fn(&mut Value), needle: &str) {
    let mut v = def_json();
    mutate(&mut v);
    let err = decode_macro_definition(v).expect_err("definition must reject");
    let msg = err.to_string();
    assert!(msg.contains(needle), "expected '{needle}' in: {msg}");
}

#[test]
fn definition_rejects_bad_schema_version_and_shapes() {
    let mut v = def_json();
    v["schema_version"] = json!(2);
    let err = decode_macro_definition(v).expect_err("version must reject");
    assert!(matches!(
        err,
        AutomationError::UnsupportedDefinitionVersion { .. }
    ));
    expect_def_err(|v| v["macro_id"] = json!("Bad Id"), "macro_id");
    expect_def_err(|v| v["steps"] = json!([]), "at least one step");
    expect_def_err(
        |v| {
            v["steps"] = json!(vec![
                json!({"id": "s", "capability": "synvelis.macro.track.create@1.0"});
                65
            ]);
        },
        "too many members",
    );
    expect_def_err(
        |v| {
            v["steps"] = json!([
                {"id": "dup", "capability": "synvelis.macro.track.create@1.0"},
                {"id": "dup", "capability": "synvelis.macro.track.create@1.0"}
            ]);
        },
        "duplicate step id",
    );
    expect_def_err(
        |v| {
            v["output_bindings"] = json!([
                {"name": "b", "step": "ghost", "kind": "track"}
            ]);
        },
        "unknown step",
    );
    expect_def_err(
        |v| {
            v["output_bindings"] = json!([
                {"name": "b", "step": "s1", "kind": "project"}
            ]);
        },
        "project aggregate",
    );
}

// ---------------------------------------------------------------------------
// Static-check / compile rejects
// ---------------------------------------------------------------------------

fn compile_err(def: &AtomicMacroDefinition, inputs: &Map<String, Value>) -> CompileError {
    let err = MacroCompiler::compile(def, inputs, &macro_registry(), None)
        .expect_err("compile must fail");
    match err {
        AutomationError::Compile(inner) => inner,
        other => panic!("expected CompileError, got {other:?}"),
    }
}

fn variant(err: &CompileError) -> &'static str {
    match err {
        CompileError::UnknownCapability { .. } => "UnknownCapability",
        CompileError::UnsupportedCapabilityVersion { .. } => "UnsupportedCapabilityVersion",
        CompileError::CapabilityUnavailable { .. } => "CapabilityUnavailable",
        CompileError::DomainMismatch { .. } => "DomainMismatch",
        CompileError::ExecutionMismatch { .. } => "ExecutionMismatch",
        CompileError::MissingTarget { .. } => "MissingTarget",
        CompileError::UnexpectedTarget { .. } => "UnexpectedTarget",
        CompileError::TargetKindMismatch { .. } => "TargetKindMismatch",
        CompileError::TargetShapeMismatch { .. } => "TargetShapeMismatch",
        CompileError::SchemaViolation { .. } => "SchemaViolation",
        CompileError::TypeMismatch { .. } => "TypeMismatch",
        CompileError::UnknownInput { .. } => "UnknownInput",
        CompileError::MissingInput { .. } => "MissingInput",
        CompileError::UnexpectedInput { .. } => "UnexpectedInput",
        CompileError::InputTypeMismatch { .. } => "InputTypeMismatch",
        CompileError::UnknownMacroOutput { .. } => "UnknownMacroOutput",
        CompileError::ForwardMacroOutput { .. } => "ForwardMacroOutput",
        CompileError::MacroOnlyExpr { .. } => "MacroOnlyExpr",
        CompileError::WorkflowOnlyExpr { .. } => "WorkflowOnlyExpr",
        CompileError::ReservedIdLiteral { .. } => "ReservedIdLiteral",
        CompileError::NestedBatch { .. } => "NestedBatch",
        CompileError::BoundCeilingExceeded { .. } => "BoundCeilingExceeded",
        _ => "other",
    }
}

fn def_with_step(step: &Value) -> AtomicMacroDefinition {
    decode_macro_definition(json!({
        "schema_version": 1,
        "macro_id": "test.probe",
        "version": 1,
        "steps": [step]
    }))
    .expect("probe definition must decode")
}

fn empty_inputs() -> Map<String, Value> {
    Map::new()
}

#[test]
fn reject_unknown_and_skewed_capability() {
    let err = compile_err(
        &def_with_step(&json!({"id": "s", "capability": "synvelis.nope@1.0"})),
        &empty_inputs(),
    );
    assert_eq!(variant(&err), "UnknownCapability");
    let err = compile_err(
        &def_with_step(&json!({"id": "s", "capability": "synvelis.macro.track.create@2.0"})),
        &empty_inputs(),
    );
    assert_eq!(variant(&err), "UnsupportedCapabilityVersion");
    let err = compile_err(
        &def_with_step(&json!({"id": "s", "capability": "synvelis.macro.track.retired@1.0"})),
        &empty_inputs(),
    );
    assert_eq!(variant(&err), "CapabilityUnavailable");
}

#[test]
fn reject_non_mutation_domains() {
    // Query domain inside a macro member.
    let err = compile_err(
        &def_with_step(&json!({"id": "s", "capability": "synvelis.macro.query.tracks@1.0"})),
        &empty_inputs(),
    );
    assert_eq!(variant(&err), "DomainMismatch");
    // Async-accept execution inside a macro member (project-mutation
    // domain so the domain check passes and execution is what fires).
    let err = compile_err(
        &def_with_step(&json!({"id": "s", "capability": "synvelis.macro.job.mutate@1.0"})),
        &empty_inputs(),
    );
    assert_eq!(variant(&err), "ExecutionMismatch");
    // The batch capability itself — nested batch.
    let err = compile_err(
        &def_with_step(&json!({"id": "s", "capability": "synvelis.operation.batch@1.0"})),
        &empty_inputs(),
    );
    assert_eq!(variant(&err), "NestedBatch");
}

#[test]
fn reject_target_contract_violations() {
    // lane.create requires a create-target naming a track parent.
    let err = compile_err(
        &def_with_step(&json!({"id": "s", "capability": "synvelis.macro.lane.create@1.0"})),
        &empty_inputs(),
    );
    assert_eq!(variant(&err), "MissingTarget");
    // track.create forbids… no — its contract is `create`; a target is
    // required and must evaluate to the project id.
    let err = compile_err(
        &def_with_step(&json!({
            "id": "s",
            "capability": "synvelis.macro.track.create@1.0",
            "args": {"expr": "object", "fields": {"name": {"expr": "literal", "value": "x"}}},
            "target": {"expr": "literal", "value": "track-0123456789abcdef0123456789abcdef"}
        })),
        &empty_inputs(),
    );
    assert_eq!(variant(&err), "TargetKindMismatch");
    // clip.bind takes a resource target of kind lane; a track id is wrong.
    let err = compile_err(
        &def_with_step(&json!({
            "id": "s",
            "capability": "synvelis.macro.clip.bind@1.0",
            "args": {"expr": "object", "fields": {"clip": {"expr": "literal", "value": "clip-0123456789abcdef0123456789abcdef"}}},
            "target": {"expr": "literal", "value": "track-0123456789abcdef0123456789abcdef"}
        })),
        &empty_inputs(),
    );
    assert_eq!(variant(&err), "TargetKindMismatch");
}

#[test]
fn reject_output_binding_violations() {
    // Unknown binding name.
    let err = compile_err(
        &def_with_step(&json!({
            "id": "s",
            "capability": "synvelis.macro.clip.bind@1.0",
            "args": {"expr": "object", "fields": {"clip": {"expr": "output", "name": "ghost"}}},
            "target": {"expr": "literal", "value": "lane-0123456789abcdef0123456789abcdef"}
        })),
        &empty_inputs(),
    );
    assert_eq!(variant(&err), "UnknownMacroOutput");
    // Forward reference: binding produced by a later step.
    let def = decode_macro_definition(json!({
        "schema_version": 1,
        "macro_id": "test.fwd",
        "version": 1,
        "steps": [
            {
                "id": "early",
                "capability": "synvelis.macro.clip.bind@1.0",
                "args": {"expr": "object", "fields": {"clip": {"expr": "output", "name": "later_out"}}},
                "target": {"expr": "literal", "value": "lane-0123456789abcdef0123456789abcdef"}
            },
            {
                "id": "late",
                "capability": "synvelis.macro.track.create@1.0",
                "args": {"expr": "object", "fields": {"name": {"expr": "literal", "value": "x"}}},
                "target": {"expr": "literal", "value": "project-00000000000000000000000000000000"}
            }
        ],
        "output_bindings": [
            {"name": "later_out", "step": "late", "kind": "clip"}
        ]
    }))
    .unwrap();
    let err = compile_err(&def, &empty_inputs());
    assert_eq!(variant(&err), "ForwardMacroOutput");
}

#[test]
fn reject_workflow_only_exprs_and_reserved_ids() {
    // $input inside a macro is meaningless — fail closed.
    let err = compile_err(
        &def_with_step(&json!({
            "id": "s",
            "capability": "synvelis.macro.track.create@1.0",
            "args": {"expr": "object", "fields": {"name": {"expr": "input", "path": ["x"]}}},
            "target": {"expr": "literal", "value": "project-00000000000000000000000000000000"}
        })),
        &empty_inputs(),
    );
    assert_eq!(variant(&err), "WorkflowOnlyExpr");
    // A literal inside the reserved placeholder range — placed in a
    // target position whose contract accepts the kind, so the static
    // pass passes and evaluation rejects it.
    let reserved = format!("track-{OUTPUT_BINDING_ID_BASE:032x}");
    let err = compile_err(
        &def_with_step(&json!({
            "id": "s",
            "capability": "synvelis.macro.lane.create@1.0",
            "target": {"expr": "literal", "value": reserved}
        })),
        &empty_inputs(),
    );
    assert_eq!(variant(&err), "ReservedIdLiteral");
    // A parameter *value* inside the reserved range rejects at bind.
    let def = decode_macro_definition(json!({
        "schema_version": 1,
        "macro_id": "test.reserved-param",
        "version": 1,
        "parameters": [
            {"name": "t", "schema": {"type": "id", "kind": "track"}}
        ],
        "steps": [{
            "id": "s",
            "capability": "synvelis.macro.lane.create@1.0",
            "target": {"expr": "param", "path": ["t"]}
        }]
    }))
    .unwrap();
    let mut bad = Map::new();
    bad.insert("t".to_owned(), json!(reserved));
    let err = compile_err(&def, &bad);
    assert_eq!(variant(&err), "ReservedIdLiteral");
}

#[test]
fn reject_param_binding_failures() {
    // Static check runs before input binding, so the parameter's
    // declared type must be compatible with the field it feeds —
    // `name` is a string field, hence a bounded string parameter.
    let def = decode_macro_definition(json!({
        "schema_version": 1,
        "macro_id": "test.params",
        "version": 1,
        "parameters": [
            {"name": "n", "schema": {"type": "string", "max_len": 4}}
        ],
        "steps": [{
            "id": "s",
            "capability": "synvelis.macro.track.create@1.0",
            "args": {"expr": "object", "fields": {"name": {"expr": "param", "path": ["n"]}}},
            "target": {"expr": "literal", "value": "project-00000000000000000000000000000000"}
        }]
    }))
    .unwrap();
    // Missing declared param.
    let err = compile_err(&def, &Map::new());
    assert_eq!(variant(&err), "MissingInput");
    // Undeclared input.
    let mut extra = Map::new();
    extra.insert("n".to_owned(), json!("ok"));
    extra.insert("surprise".to_owned(), json!(1));
    let err = compile_err(&def, &extra);
    assert_eq!(variant(&err), "UnexpectedInput");
    // Schema violation on the value.
    let mut bad = Map::new();
    bad.insert("n".to_owned(), json!("toolong"));
    let err = compile_err(&def, &bad);
    assert_eq!(variant(&err), "InputTypeMismatch");
    // A param referenced but undeclared.
    let def2 = decode_macro_definition(json!({
        "schema_version": 1,
        "macro_id": "test.params2",
        "version": 1,
        "steps": [{
            "id": "s",
            "capability": "synvelis.macro.track.create@1.0",
            "args": {"expr": "object", "fields": {"name": {"expr": "param", "path": ["ghost"]}}},
            "target": {"expr": "literal", "value": "project-00000000000000000000000000000000"}
        }]
    }))
    .unwrap();
    let err = compile_err(&def2, &empty_inputs());
    assert_eq!(variant(&err), "UnknownInput");
}

#[test]
fn reject_policy_ceiling_excess() {
    let def = decode_macro_definition(json!({
        "schema_version": 1,
        "macro_id": "test.policy",
        "version": 1,
        "steps": [{
            "id": "s",
            "capability": "synvelis.macro.track.create@1.0",
            "args": {"expr": "object", "fields": {"name": {"expr": "literal", "value": "x"}}},
            "target": {"expr": "literal", "value": "project-00000000000000000000000000000000"}
        }],
        "policy": {"allowed_effects": ["read"]}
    }))
    .unwrap();
    let err = compile_err(&def, &empty_inputs());
    assert_eq!(variant(&err), "BoundCeilingExceeded");
}

#[test]
fn unused_parameter_warns() {
    let def = decode_macro_definition(json!({
        "schema_version": 1,
        "macro_id": "test.unused",
        "version": 1,
        "parameters": [
            {"name": "spare", "schema": {"type": "string", "max_len": 8}}
        ],
        "steps": [{
            "id": "s",
            "capability": "synvelis.macro.track.create@1.0",
            "args": {"expr": "object", "fields": {"name": {"expr": "literal", "value": "x"}}},
            "target": {"expr": "literal", "value": "project-00000000000000000000000000000000"}
        }]
    }))
    .unwrap();
    let mut inputs = Map::new();
    inputs.insert("spare".to_owned(), json!("hi"));
    let out = MacroCompiler::compile(&def, &inputs, &macro_registry(), None).unwrap();
    assert_eq!(out.warnings.len(), 1);
    assert!(matches!(
        out.warnings[0],
        synvelis_automation::PlanWarning::UnusedInput { .. }
    ));
}

// ---------------------------------------------------------------------------
// MacroRegistry + PlanCompiler atomic-macro expansion
// ---------------------------------------------------------------------------

const WORKFLOW_WITH_MACRO: &str = r"
schema_version: 1
workflow_id: test.macro-call
version: 1
inputs:
  - name: track_name
    schema:
      type: string
      max_len: 256
steps:
  - kind: atomic-macro
    id: build
    mac: test.track-lane@1.0
    args:
      track_name: $input.track_name
      project: project-00000000000000000000000000000000
  - kind: emit-result
    id: done
    output: ok
    value: true
outputs:
  - name: ok
    schema:
      type: bool
limits:
  max_steps: 100
  max_depth: 8
  max_iterations: 1000
  max_runtime_ms: 60000
  max_output_bytes: 1048576
  max_memory_bytes: 134217728
";

fn workflow_inputs() -> Value {
    json!({"track_name": "Chorus"})
}

#[test]
fn plan_expands_macro_members_into_bounds() {
    let def = definition(WORKFLOW_WITH_MACRO);
    let macros = MacroRegistry::from_definitions([&track_lane_def()], &macro_registry()).unwrap();
    let inputs: Map<String, Value> = workflow_inputs().as_object().unwrap().clone();
    let out = PlanCompiler::compile_with_macros(&def, &inputs, &registry(), &macros).unwrap();
    // Cost accounts the two expanded members, not one step.
    assert_eq!(out.plan.step_order, ["build", "done"]);
    assert!(out.plan.max_steps >= 2);
    // Capability refs include both member capabilities.
    assert!(
        out.plan
            .capability_refs_versions
            .iter()
            .any(|c| c == "synvelis.macro.track.create@1.0")
    );
    assert!(
        out.plan
            .capability_refs_versions
            .iter()
            .any(|c| c == "synvelis.macro.lane.create@1.0")
    );
}

#[test]
fn plan_without_registry_keeps_fail_closed() {
    let err = compile(WORKFLOW_WITH_MACRO, workflow_inputs()).expect_err("must fail");
    let AutomationError::Compile(inner) = err else {
        panic!("expected CompileError, got {err:?}");
    };
    assert!(matches!(inner, CompileError::UndefinedMacro { .. }));
}

#[test]
fn plan_rejects_unknown_and_skewed_macros() {
    let mut src = WORKFLOW_WITH_MACRO.replace("test.track-lane@1.0", "test.ghost@1.0");
    let def = definition(&src);
    let macros = MacroRegistry::from_definitions([&track_lane_def()], &macro_registry()).unwrap();
    let inputs: Map<String, Value> = workflow_inputs().as_object().unwrap().clone();
    let err = PlanCompiler::compile_with_macros(&def, &inputs, &registry(), &macros)
        .expect_err("unknown macro must fail");
    assert!(matches!(
        err,
        AutomationError::Compile(CompileError::UnknownMacro { .. })
    ));
    src = WORKFLOW_WITH_MACRO.replace("test.track-lane@1.0", "test.track-lane@2.0");
    let def = definition(&src);
    let err = PlanCompiler::compile_with_macros(&def, &inputs, &registry(), &macros)
        .expect_err("skewed macro version must fail");
    assert!(matches!(
        err,
        AutomationError::Compile(CompileError::UnsupportedMacroVersion { .. })
    ));
}

#[test]
fn plan_rejects_macro_only_exprs_and_bad_args() {
    let def = definition(&WORKFLOW_WITH_MACRO.replace(
        "      project: project-00000000000000000000000000000000",
        "      project: $output.created_track",
    ));
    let macros = MacroRegistry::from_definitions([&track_lane_def()], &macro_registry()).unwrap();
    let inputs: Map<String, Value> = workflow_inputs().as_object().unwrap().clone();
    // A registry is required for the macro to resolve — only then
    // does call-site argument checking reach the `$output` expr.
    let err = PlanCompiler::compile_with_macros(&def, &inputs, &registry(), &macros)
        .expect_err("must fail");
    assert!(matches!(
        err,
        AutomationError::Compile(CompileError::MacroOnlyExpr { .. })
    ));
    // Undeclared call-site argument.
    let src = WORKFLOW_WITH_MACRO.replace(
        "      project: project-00000000000000000000000000000000",
        "      project: project-00000000000000000000000000000000\n      extra: 1",
    );
    let def = definition(&src);
    let macros = MacroRegistry::from_definitions([&track_lane_def()], &macro_registry()).unwrap();
    let inputs: Map<String, Value> = workflow_inputs().as_object().unwrap().clone();
    let err = PlanCompiler::compile_with_macros(&def, &inputs, &registry(), &macros)
        .expect_err("undeclared arg must fail");
    assert!(matches!(
        err,
        AutomationError::Compile(CompileError::UnexpectedInput { .. })
    ));
}
