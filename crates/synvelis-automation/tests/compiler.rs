//! `PlanCompiler` accept/reject matrix: typed steps, refs, targets,
//! effects, budgets, availability, schemas.

mod common;

use serde_json::json;
use synvelis_automation::compiler::PlanCompiler;
use synvelis_automation::definition::WorkflowDefinition;
use synvelis_automation::{AutomationError, CompileError, parse_workflow_yaml};

use crate::common::{
    ASYNC_WORKFLOW, BASIC_WORKFLOW, BRANCH_WORKFLOW, LOOP_WORKFLOW, async_inputs, basic_inputs,
    compile, definition, loop_inputs, registry,
};

fn variant(err: &CompileError) -> &'static str {
    match err {
        CompileError::UnknownStepId { .. } => "UnknownStepId",
        CompileError::ForwardStepReference { .. } => "ForwardStepReference",
        CompileError::DuplicateStepId { .. } => "DuplicateStepId",
        CompileError::UnknownInput { .. } => "UnknownInput",
        CompileError::UnexpectedInput { .. } => "UnexpectedInput",
        CompileError::MissingInput { .. } => "MissingInput",
        CompileError::InputTypeMismatch { .. } => "InputTypeMismatch",
        CompileError::UnknownCapability { .. } => "UnknownCapability",
        CompileError::UnsupportedCapabilityVersion { .. } => "UnsupportedCapabilityVersion",
        CompileError::CapabilityUnavailable { .. } => "CapabilityUnavailable",
        CompileError::DomainMismatch { .. } => "DomainMismatch",
        CompileError::ExecutionMismatch { .. } => "ExecutionMismatch",
        CompileError::UnknownSchema { .. } => "UnknownSchema",
        CompileError::UnsupportedSchemaVersion { .. } => "UnsupportedSchemaVersion",
        CompileError::SchemaViolation { .. } => "SchemaViolation",
        CompileError::TypeMismatch { .. } => "TypeMismatch",
        CompileError::UnexpectedTarget { .. } => "UnexpectedTarget",
        CompileError::MissingTarget { .. } => "MissingTarget",
        CompileError::TargetShapeMismatch { .. } => "TargetShapeMismatch",
        CompileError::TargetKindMismatch { .. } => "TargetKindMismatch",
        CompileError::UndefinedMacro { .. } => "UndefinedMacro",
        CompileError::UnboundedIteration { .. } => "UnboundedIteration",
        CompileError::ItemsNotArray { .. } => "ItemsNotArray",
        CompileError::JobRefTypeMismatch { .. } => "JobRefTypeMismatch",
        CompileError::UndeclaredOutput { .. } => "UndeclaredOutput",
        CompileError::OutputTypeMismatch { .. } => "OutputTypeMismatch",
        CompileError::ConditionTypeMismatch { .. } => "ConditionTypeMismatch",
        CompileError::BudgetExceeded { .. } => "BudgetExceeded",
        CompileError::BudgetUnderestimated { .. } => "BudgetUnderestimated",
        CompileError::ItemOutOfScope { .. } => "ItemOutOfScope",
        CompileError::EmitResultNotTopLevel { .. } => "EmitResultNotTopLevel",
        CompileError::MissingOutputEmit { .. } => "MissingOutputEmit",
        CompileError::DuplicateOutputEmit { .. } => "DuplicateOutputEmit",
        CompileError::BoundCeilingExceeded { .. } => "BoundCeilingExceeded",
        CompileError::NoStepOutput { .. } => "NoStepOutput",
        CompileError::UnknownOutputField { .. } => "UnknownOutputField",
    }
}

/// Compiles `src` and asserts the specific [`CompileError`] variant.
fn expect_variant(src: &str, inputs: serde_json::Value, name: &str) {
    let err = compile(src, inputs).expect_err("compile must fail");
    let AutomationError::Compile(inner) = err else {
        panic!("expected CompileError, got {err:?}");
    };
    assert_eq!(variant(&inner), name, "unexpected error: {inner}");
}

// ---------- Accept paths ----------

#[test]
fn basic_workflow_compiles() {
    let out = compile(BASIC_WORKFLOW, basic_inputs()).unwrap();
    assert_eq!(out.plan.schema_version, 1);
    assert_eq!(out.plan.step_order, ["tracks", "add", "done"]);
    // Plan carries the *declared* budgets (verified ≥ worst case).
    assert_eq!(out.plan.max_steps, 100);
    assert_eq!(out.plan.max_depth, 8);
    assert_eq!(out.plan.max_iterations, 1000);
    assert_eq!(out.plan.max_runtime, 60_000);
    assert_eq!(out.plan.max_output_bytes, 1_048_576);
    assert_eq!(out.plan.definition_digest.len(), 64);
    assert!(out.plan.explicit_target_binding);
    // capability refs are exact `id@major.minor` strings, sorted.
    let refs = &out.plan.capability_refs_versions;
    assert_eq!(
        refs,
        &[
            "synvelis.project.add-note@1.0".to_owned(),
            "synvelis.query.tracks@1.0".to_owned(),
        ]
    );
    // Per-capability effect upper bounds (EffectKind declaration
    // order: read < write < create).
    let add_effects = &out.plan.effect_upper_bounds["synvelis.project.add-note@1.0"];
    assert_eq!(add_effects, &["read", "write", "create"]);
    assert_eq!(
        out.plan.effect_upper_bounds["synvelis.query.tracks@1.0"],
        ["read"]
    );
    assert_eq!(
        out.plan.data_access_bounds["synvelis.project.add-note@1.0"],
        ["metadata-read"]
    );
}

#[test]
fn async_workflow_compiles() {
    let out = compile(ASYNC_WORKFLOW, async_inputs()).unwrap();
    assert_eq!(out.plan.step_order, ["render", "status", "wait", "done"]);
    // await-job consumes a job ref — it adds no capability ref.
    assert_eq!(
        out.plan.capability_refs_versions,
        [
            "synvelis.job.render@1.0".to_owned(),
            "synvelis.job.status@1.0".to_owned(),
        ]
    );
}

#[test]
fn branch_workflow_unions_effects() {
    let out = compile(BRANCH_WORKFLOW, basic_inputs()).unwrap();
    // step_order: control id once, then-branch before else-branch.
    assert_eq!(
        out.plan.step_order,
        ["tracks", "decide", "add", "again", "done"]
    );
    // Both branches' capabilities appear — no under-approximation.
    assert!(
        out.plan
            .capability_refs_versions
            .contains(&"synvelis.project.add-note@1.0".to_owned())
    );
    assert!(
        out.plan
            .capability_refs_versions
            .contains(&"synvelis.query.tracks@1.0".to_owned())
    );
}

#[test]
fn loop_workflow_scales_budgets() {
    let out = compile(LOOP_WORKFLOW, loop_inputs()).unwrap();
    // for-each node + body once + emit — bodies list once in step_order.
    assert_eq!(out.plan.step_order, ["loop", "add", "done"]);
    // Declared budgets carried through (verified ≥ computed worst case:
    // steps 18, iters 16, depth 1).
    assert_eq!(out.plan.max_steps, 100_000);
    assert_eq!(out.plan.max_iterations, 10_000);
}

// ---------- Definition/ID/step-graph rejections ----------

#[test]
fn unknown_step_reference_rejected() {
    let src = BASIC_WORKFLOW.replace("value: $steps.add", "value: $steps.nope");
    expect_variant(&src, basic_inputs(), "UnknownStepId");
}

#[test]
fn forward_step_reference_rejected() {
    let src = r"
schema_version: 1
workflow_id: test.fwd
version: 1
steps:
  - kind: query
    id: first
    capability: synvelis.query.tracks@1.0
    args:
      filter: $steps.later
  - kind: query
    id: later
    capability: synvelis.query.tracks@1.0
outputs:
  - name: o
    schema:
      type: bool
limits:
  max_steps: 10
  max_depth: 4
  max_iterations: 10
  max_runtime_ms: 1000
  max_output_bytes: 1000
  max_memory_bytes: 1000
";
    expect_variant(src, json!({}), "ForwardStepReference");
}

#[test]
fn branch_local_step_not_visible_outside() {
    // A step defined inside `then` is out of scope at top level; the
    // id exists in the definition so the failure is a forward/back
    // scope rejection, never a silent binding.
    let src = BRANCH_WORKFLOW.replace("value: true", "value: $steps.add.name");
    // `add` lives inside the then-branch → not in top-level scope.
    let err = compile(&src, basic_inputs()).expect_err("must fail");
    let AutomationError::Compile(inner) = err else {
        panic!("{err:?}")
    };
    assert!(matches!(
        inner,
        CompileError::ForwardStepReference { .. } | CompileError::NoStepOutput { .. }
    ));
}

#[test]
fn unknown_input_rejected() {
    let src = BASIC_WORKFLOW.replace("value: $steps.add", "value: $input.missing");
    expect_variant(&src, basic_inputs(), "UnknownInput");
}

#[test]
fn missing_required_input_rejected() {
    expect_variant(BASIC_WORKFLOW, json!({}), "MissingInput");
}

#[test]
fn unexpected_input_rejected() {
    expect_variant(
        BASIC_WORKFLOW,
        json!({"track": "track-0123456789abcdef0123456789abcdef", "extra": 1}),
        "UnexpectedInput",
    );
}

#[test]
fn input_schema_violation_rejected() {
    // Wrong id kind for the `track` input.
    expect_variant(
        BASIC_WORKFLOW,
        json!({"track": "note-0123456789abcdef0123456789abcdef"}),
        "InputTypeMismatch",
    );
}

#[test]
fn step_output_field_projection_checked() {
    // track-list is an array — `.name` cannot project onto it.
    let src = BASIC_WORKFLOW.replace("value: $steps.add", "value: $steps.tracks.name");
    expect_variant(&src, basic_inputs(), "UnknownOutputField");
}

#[test]
fn control_step_output_rejected() {
    // if/for-each produce no referenceable output.
    let src = LOOP_WORKFLOW.replace("value: true", "value: $steps.loop");
    expect_variant(&src, loop_inputs(), "NoStepOutput");
}

// ---------- Capability/registry rejections ----------

#[test]
fn unknown_capability_rejected() {
    let src = BASIC_WORKFLOW.replace(
        "capability: synvelis.query.tracks@1.0",
        "capability: synvelis.nope@1.0",
    );
    expect_variant(&src, basic_inputs(), "UnknownCapability");
}

#[test]
fn unsupported_capability_version_rejected() {
    let src = BASIC_WORKFLOW.replace(
        "capability: synvelis.query.tracks@1.0",
        "capability: synvelis.query.tracks@2.0",
    );
    expect_variant(&src, basic_inputs(), "UnsupportedCapabilityVersion");
}

#[test]
fn planned_retired_disabled_rejected() {
    for cap in [
        "synvelis.query.planned@1.0",
        "synvelis.query.retired@1.0",
        "synvelis.query.disabled@1.0",
    ] {
        let src = BASIC_WORKFLOW.replace(
            "capability: synvelis.query.tracks@1.0",
            &format!("capability: {cap}"),
        );
        expect_variant(&src, basic_inputs(), "CapabilityUnavailable");
    }
}

#[test]
fn domain_mismatch_rejected() {
    // job.status is query-domain — an `invoke` step needs
    // project-mutation; domain is checked before args.
    let src = BASIC_WORKFLOW.replace(
        "capability: synvelis.project.add-note@1.0",
        "capability: synvelis.job.status@1.0",
    );
    expect_variant(&src, basic_inputs(), "DomainMismatch");
}

#[test]
fn execution_mismatch_rejected() {
    // poll-async: right domain (async-operation) but `sync` execution —
    // async-submit requires async-accept.
    let src = r"
schema_version: 1
workflow_id: test.exec
version: 1
steps:
  - kind: async-submit
    id: bad
    capability: synvelis.job.poll-async@1.0
outputs:
  - name: o
    schema:
      type: bool
limits:
  max_steps: 10
  max_depth: 4
  max_iterations: 10
  max_runtime_ms: 1000
  max_output_bytes: 1000
  max_memory_bytes: 1000
";
    expect_variant(src, json!({}), "ExecutionMismatch");
}

#[test]
fn await_job_requires_job_typed_value() {
    // `job` on await-job is at 4-space indent; the status step's args
    // job is deeper — replace the whole await-job header to be exact.
    let src = ASYNC_WORKFLOW.replace(
        "  - kind: await-job\n    id: wait\n    job: $steps.render",
        "  - kind: await-job\n    id: wait\n    job: $steps.status.state",
    );
    // status.state is a string, not a job handle.
    expect_variant(&src, async_inputs(), "JobRefTypeMismatch");
}

// ---------- Schema/args/target rejections ----------

#[test]
fn args_schema_violation_rejected() {
    // pitch out of range.
    let src = BASIC_WORKFLOW.replace("pitch: 60", "pitch: 200");
    expect_variant(&src, basic_inputs(), "SchemaViolation");
    // Unknown arg field (deny_unknown).
    let src = BASIC_WORKFLOW.replace("pitch: 60", "pitch: 60\n      bogus: 1");
    expect_variant(&src, basic_inputs(), "SchemaViolation");
    // Missing required arg.
    let src = BASIC_WORKFLOW.replace("      pitch: 60\n", "");
    expect_variant(&src, basic_inputs(), "SchemaViolation");
}

#[test]
fn missing_target_rejected() {
    let src = BASIC_WORKFLOW.replace("    target: $input.track\n", "");
    expect_variant(&src, basic_inputs(), "MissingTarget");
}

#[test]
fn unexpected_target_rejected() {
    // query capability declares target none — supplying one rejects.
    let src = BASIC_WORKFLOW.replace(
        "capability: synvelis.query.tracks@1.0\n    args:\n      limit: 10",
        "capability: synvelis.query.tracks@1.0\n    args:\n      limit: 10\n    target: $input.track",
    );
    expect_variant(&src, basic_inputs(), "UnexpectedTarget");
}

#[test]
fn target_kind_mismatch_rejected() {
    // add-note requires a track target; pass a note id literal.
    let src = BASIC_WORKFLOW.replace(
        "target: $input.track",
        "target: note-0123456789abcdef0123456789abcdef",
    );
    expect_variant(&src, basic_inputs(), "TargetKindMismatch");
}

#[test]
fn implicit_target_forms_rejected() {
    // There is no syntax for selection/active-window/last-edited —
    // plain scalars become string literals and fail tagged-id checks.
    for bad in [
        "target: selection",
        "target: active-window",
        "target: last-edited",
    ] {
        let src = BASIC_WORKFLOW.replace("target: $input.track", bad);
        expect_variant(&src, basic_inputs(), "TargetKindMismatch");
    }
}

// ---------- Emit/outputs ----------

#[test]
fn undeclared_output_rejected() {
    let src = BASIC_WORKFLOW.replace("output: note", "output: bogus");
    expect_variant(&src, basic_inputs(), "UndeclaredOutput");
}

#[test]
fn duplicate_output_emit_rejected() {
    let src = BASIC_WORKFLOW.replace(
        "  - kind: emit-result\n    id: done\n    output: note\n    value: $steps.add\noutputs:",
        "  - kind: emit-result\n    id: done\n    output: note\n    value: $steps.add\n  - kind: emit-result\n    id: done2\n    output: note\n    value: $steps.add\noutputs:",
    );
    expect_variant(&src, basic_inputs(), "DuplicateOutputEmit");
}

#[test]
fn missing_output_emit_rejected() {
    let src = BASIC_WORKFLOW.replace(
        "  - kind: emit-result\n    id: done\n    output: note\n    value: $steps.add\n",
        "",
    );
    expect_variant(&src, basic_inputs(), "MissingOutputEmit");
}

#[test]
fn emit_inside_branch_rejected() {
    let src = BRANCH_WORKFLOW.replace(
        "    else:\n      - kind: query\n        id: again\n        capability: synvelis.query.tracks@1.0",
        "    else:\n      - kind: emit-result\n        id: inner\n        output: ok\n        value: true",
    );
    expect_variant(&src, basic_inputs(), "EmitResultNotTopLevel");
}

#[test]
fn output_type_mismatch_rejected() {
    // Emit the note id where a bool is declared.
    let src = BASIC_WORKFLOW
        .replace("output: note", "output: ok")
        .replace(
            "outputs:\n  - name: note\n    schema:\n      type: id\n      kind: note",
            "outputs:\n  - name: ok\n    schema:\n      type: bool",
        );
    expect_variant(&src, basic_inputs(), "OutputTypeMismatch");
}

// ---------- Conditions ----------

#[test]
fn incomparable_condition_operands_rejected() {
    let src = BRANCH_WORKFLOW.replace(
        "condition:\n      op: exists\n      expr: $steps.tracks",
        "condition:\n      op: eq\n      left: 1\n      right: abc",
    );
    expect_variant(&src, basic_inputs(), "ConditionTypeMismatch");
}

#[test]
fn ordering_condition_requires_numeric() {
    let src = BRANCH_WORKFLOW.replace(
        "condition:\n      op: exists\n      expr: $steps.tracks",
        "condition:\n      op: gt\n      left: $input.track\n      right: 0",
    );
    // $input.track is id:track — not numeric.
    expect_variant(&src, basic_inputs(), "ConditionTypeMismatch");
}

// ---------- Loops/iteration ----------

#[test]
fn non_array_items_rejected() {
    let src = LOOP_WORKFLOW.replace("items: $input.tracks", "items: 5");
    expect_variant(&src, loop_inputs(), "ItemsNotArray");
}

#[test]
fn item_reference_outside_loop_rejected() {
    let src = BASIC_WORKFLOW.replace("value: $steps.add", "value: $item");
    expect_variant(&src, basic_inputs(), "ItemOutOfScope");
}

#[test]
fn iteration_budget_underestimated_rejected() {
    // Declared max_iterations 8 < worst case 16.
    let src = LOOP_WORKFLOW.replace("max_iterations: 10000", "max_iterations: 8");
    let err = compile(&src, loop_inputs()).expect_err("must fail");
    let AutomationError::Compile(CompileError::BudgetUnderestimated {
        kind: "iterations", ..
    }) = err
    else {
        panic!("expected BudgetUnderestimated/iterations, got {err:?}");
    };
}

#[test]
fn step_budget_underestimated_rejected() {
    let src = BASIC_WORKFLOW.replace("max_steps: 100", "max_steps: 2");
    let err = compile(&src, basic_inputs()).expect_err("must fail");
    let AutomationError::Compile(CompileError::BudgetUnderestimated { kind: "steps", .. }) = err
    else {
        panic!("expected BudgetUnderestimated/steps, got {err:?}");
    };
}

#[test]
fn depth_budget_underestimated_rejected() {
    let src = LOOP_WORKFLOW.replace("max_depth: 8", "max_depth: 0");
    let err = compile(&src, loop_inputs()).expect_err("must fail");
    let AutomationError::Compile(CompileError::BudgetUnderestimated { kind: "depth", .. }) = err
    else {
        panic!("expected BudgetUnderestimated/depth, got {err:?}");
    };
}

#[test]
fn declared_budget_above_ceiling_rejected() {
    let src = BASIC_WORKFLOW.replace("max_steps: 100", "max_steps: 2000000");
    let err = compile(&src, basic_inputs()).expect_err("must fail");
    let AutomationError::Compile(CompileError::BudgetExceeded { kind: "steps", .. }) = err else {
        panic!("expected BudgetExceeded/steps, got {err:?}");
    };
}

#[test]
fn effect_ceiling_rejected() {
    // allowed_effects [read] but add-note declares write/create.
    let src = BASIC_WORKFLOW.replace(
        "  max_memory_bytes: 134217728",
        "  max_memory_bytes: 134217728\n  allowed_effects:\n    - read",
    );
    let err = compile(&src, basic_inputs()).expect_err("must fail");
    let AutomationError::Compile(CompileError::BoundCeilingExceeded {
        kind: "effect",
        item,
    }) = err
    else {
        panic!("expected BoundCeilingExceeded/effect, got {err:?}");
    };
    assert!(item == "write" || item == "create", "item: {item}");
}

// ---------- AtomicMacro reservation ----------

#[test]
fn undefined_macro_rejected() {
    let src = r"
schema_version: 1
workflow_id: test.macro
version: 1
steps:
  - kind: atomic-macro
    id: m
    mac: synvelis.macro.nope@1.0
outputs:
  - name: o
    schema:
      type: bool
limits:
  max_steps: 10
  max_depth: 4
  max_iterations: 10
  max_runtime_ms: 1000
  max_output_bytes: 1000
  max_memory_bytes: 1000
";
    expect_variant(src, json!({}), "UndefinedMacro");
}

// ---------- Determinism / digest ----------

#[test]
fn compile_is_deterministic() {
    let a = compile(BASIC_WORKFLOW, basic_inputs()).unwrap();
    let b = compile(BASIC_WORKFLOW, basic_inputs()).unwrap();
    assert_eq!(a.plan.definition_digest, b.plan.definition_digest);
    assert_eq!(a.plan.digest().unwrap(), b.plan.digest().unwrap());
}

#[test]
fn plan_has_no_authority_fields() {
    let out = compile(BASIC_WORKFLOW, basic_inputs()).unwrap();
    let json = serde_json::to_string(&out.plan).unwrap();
    for forbidden in ["grant", "approval", "principal", "credential", "token"] {
        assert!(!json.contains(forbidden), "plan leaks {forbidden}");
    }
}

#[test]
fn definition_roundtrip_via_json() {
    let def = definition(BASIC_WORKFLOW);
    let v = serde_json::to_value(&def).unwrap();
    let back: WorkflowDefinition = serde_json::from_value(v).unwrap();
    assert_eq!(def, back);
}

#[test]
fn plan_compiler_is_source_agnostic() {
    // The same decoded definition compiles identically whether it came
    // from YAML or canonical JSON — the compiler never sees source text.
    let def = parse_workflow_yaml(BASIC_WORKFLOW).unwrap();
    let v = serde_json::to_value(&def).unwrap();
    let def2: WorkflowDefinition = serde_json::from_value(v).unwrap();
    let inputs = basic_inputs();
    let map: serde_json::Map<String, serde_json::Value> = inputs.as_object().unwrap().clone();
    let reg = registry();
    let a = PlanCompiler::compile(&def, &map, &reg).unwrap();
    let b = PlanCompiler::compile(&def2, &map, &reg).unwrap();
    assert_eq!(a.plan.digest().unwrap(), b.plan.digest().unwrap());
}
