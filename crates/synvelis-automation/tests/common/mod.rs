//! Shared fixtures and helpers for integration tests.
//!
//! The registry snapshot below is a serialized stand-in for what the
//! (future) live Gateway Descriptor Read Adapter will project — it is
//! deliberately self-contained YAML/JSON, never a gateway type.
//!
//! All fixture YAML uses the restricted block-style subset only: no
//! flow collections, no anchors/tags, plain scalars.

#![allow(dead_code)]

use serde_json::{Map, Value, json};
use synvelis_automation::{
    AutomationError, CompileError, PlanCompiler, PlanOutput, RegistrySnapshot, WorkflowDefinition,
    parse_registry_snapshot_yaml, parse_workflow_yaml,
};

/// A small but representative registry: query/mutation/async
/// capabilities plus planned/retired/disabled and domain/execution
/// mismatch probes.
pub const REGISTRY_YAML: &str = r"
schema_version: 1
capabilities:
  - id: synvelis.query.tracks
    version:
      major: 1
      minor: 0
    domain: query
    input_schema: synvelis.args.query-tracks@1.0
    output_schema: synvelis.out.track-list@1.0
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
  - id: synvelis.project.add-note
    version:
      major: 1
      minor: 0
    domain: project-mutation
    input_schema: synvelis.args.add-note@1.0
    output_schema: synvelis.out.note-id@1.0
    target:
      shape: resource
      kind: track
    effect_bound:
      - read
      - write
      - create
    required_permissions:
      - mutate-project
    data_access:
      - metadata-read
    execution: sync
    availability:
      state: available
  - id: synvelis.job.render
    version:
      major: 1
      minor: 0
    domain: async-operation
    input_schema: synvelis.args.render@1.0
    output_schema: synvelis.out.job-handle@1.0
    target:
      shape: none
    effect_bound:
      - read
      - external-action
    required_permissions:
      - run-job
    data_access:
      - media-bytes-read
    execution: async-accept
    availability:
      state: available
  - id: synvelis.job.status
    version:
      major: 1
      minor: 0
    domain: query
    input_schema: synvelis.args.job-ref@1.0
    output_schema: synvelis.out.job-status@1.0
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
  - id: synvelis.job.poll-async
    version:
      major: 1
      minor: 0
    domain: async-operation
    input_schema: synvelis.args.job-ref@1.0
    output_schema: synvelis.out.job-handle@1.0
    target:
      shape: none
    effect_bound:
      - read
    required_permissions:
      - run-job
    data_access:
      - metadata-read
    execution: sync
    availability:
      state: available
  - id: synvelis.query.planned
    version:
      major: 1
      minor: 0
    domain: query
    input_schema: synvelis.args.empty@1.0
    output_schema: synvelis.out.empty@1.0
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
      state: planned
  - id: synvelis.query.retired
    version:
      major: 1
      minor: 0
    domain: query
    input_schema: synvelis.args.empty@1.0
    output_schema: synvelis.out.empty@1.0
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
      state: retired
  - id: synvelis.query.disabled
    version:
      major: 1
      minor: 0
    domain: query
    input_schema: synvelis.args.empty@1.0
    output_schema: synvelis.out.empty@1.0
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
      state: disabled
      reason: maintenance
schemas:
  - name: synvelis.args.empty
    version:
      major: 1
      minor: 0
    schema:
      type: object
      deny_unknown: true
  - name: synvelis.args.query-tracks
    version:
      major: 1
      minor: 0
    schema:
      type: object
      deny_unknown: true
      fields:
        filter:
          schema:
            type: string
            max_len: 128
          required: false
        limit:
          schema:
            type: integer
            min: 1
            max: 1000
          required: false
  - name: synvelis.args.add-note
    version:
      major: 1
      minor: 0
    schema:
      type: object
      deny_unknown: true
      fields:
        pitch:
          schema:
            type: integer
            min: 0
            max: 127
          required: true
        velocity:
          schema:
            type: integer
            min: 1
            max: 127
          required: true
        start_tick:
          schema:
            type: integer
            min: 0
          required: true
        duration_ticks:
          schema:
            type: integer
            min: 1
          required: true
  - name: synvelis.args.render
    version:
      major: 1
      minor: 0
    schema:
      type: object
      deny_unknown: true
      fields:
        range:
          schema:
            type: time-range
            domain: media
          required: true
        format:
          schema:
            type: enum
            values:
              - wav
              - flac
          required: true
  - name: synvelis.args.job-ref
    version:
      major: 1
      minor: 0
    schema:
      type: object
      deny_unknown: true
      fields:
        job:
          schema:
            type: job
          required: true
  - name: synvelis.out.empty
    version:
      major: 1
      minor: 0
    schema:
      type: object
      deny_unknown: true
  - name: synvelis.out.track-list
    version:
      major: 1
      minor: 0
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
  - name: synvelis.out.note-id
    version:
      major: 1
      minor: 0
    schema:
      type: id
      kind: note
  - name: synvelis.out.job-handle
    version:
      major: 1
      minor: 0
    schema:
      type: job
  - name: synvelis.out.job-status
    version:
      major: 1
      minor: 0
    schema:
      type: object
      deny_unknown: true
      fields:
        state:
          schema:
            type: enum
            values:
              - running
              - succeeded
              - failed
          required: true
        progress:
          schema:
            type: number
          required: false
";

/// The parsed fixture registry.
pub fn registry() -> RegistrySnapshot {
    parse_registry_snapshot_yaml(REGISTRY_YAML).expect("fixture registry must parse")
}

/// Parses a workflow YAML source into a definition.
pub fn definition(yaml_src: &str) -> WorkflowDefinition {
    parse_workflow_yaml(yaml_src).expect("fixture definition must parse")
}

/// Compiles a YAML workflow against the fixture registry with the
/// given JSON object as typed inputs.
pub fn compile(yaml_src: &str, inputs: Value) -> Result<PlanOutput, AutomationError> {
    let def = parse_workflow_yaml(yaml_src)?;
    let inputs_map: Map<String, Value> = match inputs {
        Value::Object(m) => m,
        _ => Map::new(),
    };
    PlanCompiler::compile(&def, &inputs_map, &registry())
}

/// Asserts the compile fails with a [`CompileError`] matching `check`.
pub fn expect_compile_err(yaml_src: &str, inputs: Value, check: impl Fn(&CompileError) -> bool) {
    let err = compile(yaml_src, inputs).expect_err("compile must fail");
    let AutomationError::Compile(inner) = err else {
        panic!("expected CompileError, got {err:?}");
    };
    assert!(
        check(&inner),
        "unexpected CompileError: {inner} ({inner:?})"
    );
}

/// A minimal valid workflow exercising query → invoke → emit-result.
pub const BASIC_WORKFLOW: &str = r"
schema_version: 1
workflow_id: test.basic
version: 1
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
  - kind: invoke
    id: add
    capability: synvelis.project.add-note@1.0
    args:
      pitch: 60
      velocity: 100
      start_tick: 0
      duration_ticks: 480
    target: $input.track
  - kind: emit-result
    id: done
    output: note
    value: $steps.add
outputs:
  - name: note
    schema:
      type: id
      kind: note
limits:
  max_steps: 100
  max_depth: 8
  max_iterations: 1000
  max_runtime_ms: 60000
  max_output_bytes: 1048576
  max_memory_bytes: 134217728
";

/// The typed inputs for [`BASIC_WORKFLOW`].
pub fn basic_inputs() -> Value {
    json!({"track": "track-0123456789abcdef0123456789abcdef"})
}

/// An async workflow: submit → poll status → await result → emit.
pub const ASYNC_WORKFLOW: &str = r"
schema_version: 1
workflow_id: test.async-flow
version: 1
inputs:
  - name: range
    schema:
      type: time-range
      domain: media
steps:
  - kind: async-submit
    id: render
    capability: synvelis.job.render@1.0
    args:
      range: $input.range
      format: wav
  - kind: query
    id: status
    capability: synvelis.job.status@1.0
    args:
      job: $steps.render
  - kind: await-job
    id: wait
    job: $steps.render
    output_schema: synvelis.out.job-status@1.0
  - kind: emit-result
    id: done
    output: status
    value: $steps.wait.state
outputs:
  - name: status
    schema:
      type: enum
      values:
        - running
        - succeeded
        - failed
limits:
  max_steps: 100
  max_depth: 8
  max_iterations: 1000
  max_runtime_ms: 60000
  max_output_bytes: 1048576
  max_memory_bytes: 134217728
";

/// Inputs for [`ASYNC_WORKFLOW`].
pub fn async_inputs() -> Value {
    json!({"range": {"domain": "media", "start": 0, "len": 4800}})
}

/// A branching workflow: `if` over a step output with effects in both
/// branches.
pub const BRANCH_WORKFLOW: &str = r"
schema_version: 1
workflow_id: test.branch
version: 1
inputs:
  - name: track
    schema:
      type: id
      kind: track
steps:
  - kind: query
    id: tracks
    capability: synvelis.query.tracks@1.0
  - kind: if
    id: decide
    condition:
      op: exists
      expr: $steps.tracks
    then:
      - kind: invoke
        id: add
        capability: synvelis.project.add-note@1.0
        args:
          pitch: 60
          velocity: 100
          start_tick: 0
          duration_ticks: 480
        target: $input.track
    else:
      - kind: query
        id: again
        capability: synvelis.query.tracks@1.0
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

/// A bounded-loop workflow iterating track ids.
pub const LOOP_WORKFLOW: &str = r"
schema_version: 1
workflow_id: test.loop
version: 1
inputs:
  - name: tracks
    schema:
      type: array
      max_len: 64
      item:
        type: id
        kind: track
steps:
  - kind: for-each-bounded
    id: loop
    items: $input.tracks
    max_iterations: 16
    body:
      - kind: invoke
        id: add
        capability: synvelis.project.add-note@1.0
        args:
          pitch: 60
          velocity: 100
          start_tick: 0
          duration_ticks: 480
        target: $item
  - kind: emit-result
    id: done
    output: ok
    value: true
outputs:
  - name: ok
    schema:
      type: bool
limits:
  max_steps: 100000
  max_depth: 8
  max_iterations: 10000
  max_runtime_ms: 60000
  max_output_bytes: 1048576
  max_memory_bytes: 134217728
";

/// Inputs for [`LOOP_WORKFLOW`].
pub fn loop_inputs() -> Value {
    json!({"tracks": [
        "track-11111111111111111111111111111111",
        "track-22222222222222222222222222222222",
    ]})
}
