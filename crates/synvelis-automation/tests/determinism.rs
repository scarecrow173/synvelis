//! Deterministic golden coverage: canonical digests, plan ordering,
//! and the evidence ledger values recorded under `evidence/sv264/`.

mod common;

use serde_json::json;
use synvelis_automation::canon::{canonical_json, definition_digest, digest_of};

use crate::common::{
    ASYNC_WORKFLOW, BASIC_WORKFLOW, BRANCH_WORKFLOW, LOOP_WORKFLOW, async_inputs, basic_inputs,
    compile, definition, loop_inputs,
};

#[test]
fn canonical_json_sorts_keys_and_is_compact() {
    let v = json!({"b": 1, "a": {"d": 2, "c": 3}, "z": [true, null]});
    let bytes = canonical_json(&v).unwrap();
    let s = String::from_utf8(bytes).unwrap();
    assert_eq!(s, r#"{"a":{"c":3,"d":2},"b":1,"z":[true,null]}"#);
}

#[test]
fn digest_is_lowercase_sha256_hex() {
    let d = digest_of(&json!({"a": 1})).unwrap();
    assert_eq!(d.len(), 64);
    assert!(d.chars().all(|c| c.is_ascii_hexdigit()));
    assert_eq!(d, d.to_lowercase());
}

#[test]
fn definition_digest_stable_across_key_order() {
    let def = definition(BASIC_WORKFLOW);
    let d1 = definition_digest(&def).unwrap();
    let v = serde_json::to_value(&def).unwrap();
    // serde_json::Map is BTreeMap-backed (already sorted); rebuild in
    // reverse insertion order to prove canonicalization is order-free.
    let mut rev = serde_json::Map::new();
    for (k, val) in v.as_object().unwrap().iter().rev() {
        rev.insert(k.clone(), val.clone());
    }
    let def2: synvelis_automation::WorkflowDefinition =
        serde_json::from_value(serde_json::Value::Object(rev)).unwrap();
    let d2 = definition_digest(&def2).unwrap();
    assert_eq!(d1, d2);
}

#[test]
fn definition_digest_changes_with_definition() {
    let a = definition_digest(&definition(BASIC_WORKFLOW)).unwrap();
    let b = definition_digest(&definition(
        &BASIC_WORKFLOW.replace("workflow_id: test.basic", "workflow_id: test.other"),
    ))
    .unwrap();
    assert_ne!(a, b);
}

#[test]
fn basic_workflow_golden_digest() {
    let out = compile(BASIC_WORKFLOW, basic_inputs()).unwrap();
    // Golden values recorded in evidence/sv264 (AUTX-05-DIG-*).
    assert_eq!(
        out.plan.definition_digest,
        "1c1a411fb54370886dcdc2307dfcf4110e134178167159898b2f26156e004bae"
    );
    assert_eq!(
        out.plan.digest().unwrap(),
        "6779664f07bc06672487d6f9603e078ba25af3c214dae22d340ecead16b1492d"
    );
}

#[test]
fn async_workflow_golden_step_order() {
    let out = compile(ASYNC_WORKFLOW, async_inputs()).unwrap();
    assert_eq!(out.plan.step_order, ["render", "status", "wait", "done"]);
}

#[test]
fn branch_workflow_golden_step_order_and_effects() {
    let out = compile(BRANCH_WORKFLOW, basic_inputs()).unwrap();
    assert_eq!(
        out.plan.step_order,
        ["tracks", "decide", "add", "again", "done"]
    );
    // Effect upper bound must cover BOTH branches (no under-report).
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
fn loop_workflow_golden_budgets() {
    let out = compile(LOOP_WORKFLOW, loop_inputs()).unwrap();
    // Declared budgets are carried into the plan after the compiler
    // proves the computed worst case (steps 18 / iters 16 / depth 1)
    // fits inside them.
    assert_eq!(out.plan.max_steps, 100_000);
    assert_eq!(out.plan.max_depth, 8);
    assert_eq!(out.plan.max_iterations, 10_000);
    assert_eq!(out.plan.max_runtime, 60_000);
    assert_eq!(out.plan.max_output_bytes, 1_048_576);
}

#[test]
fn same_inputs_same_plan_across_runs() {
    let a = compile(BASIC_WORKFLOW, basic_inputs()).unwrap();
    let b = compile(BASIC_WORKFLOW, basic_inputs()).unwrap();
    assert_eq!(a.plan, b.plan);
}

#[test]
fn plan_is_input_value_independent() {
    // Inputs are validated but not embedded: the plan describes the
    // program, so two different valid inputs yield the same plan.
    let a = compile(BASIC_WORKFLOW, basic_inputs()).unwrap();
    let b = compile(
        BASIC_WORKFLOW,
        json!({"track": "track-ffffffffffffffffffffffffffffffff"}),
    )
    .unwrap();
    assert_eq!(a.plan, b.plan);
}

#[test]
fn capability_refs_are_exact_versions() {
    let out = compile(BASIC_WORKFLOW, basic_inputs()).unwrap();
    assert!(
        out.plan
            .capability_refs_versions
            .contains(&"synvelis.project.add-note@1.0".to_owned())
    );
    // Sorted order (BTreeSet of strings).
    let refs = &out.plan.capability_refs_versions;
    let mut sorted = refs.clone();
    sorted.sort();
    assert_eq!(*refs, sorted);
}

#[test]
fn plan_schema_version_is_one() {
    let out = compile(BASIC_WORKFLOW, basic_inputs()).unwrap();
    assert_eq!(out.plan.schema_version, 1);
}
