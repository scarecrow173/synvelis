//! Definition decode: encode/decode roundtrip, version gate, unknown
//! fields, forbidden authority fields.

mod common;

use synvelis_automation::{
    AutomationError, DEFINITION_SCHEMA_VERSION, decode_workflow_definition, parse_workflow_yaml,
};

use crate::common::{BASIC_WORKFLOW, definition};

#[test]
fn encode_decode_roundtrip() {
    let def = definition(BASIC_WORKFLOW);
    let value = serde_json::to_value(&def).unwrap();
    let back = decode_workflow_definition(value).unwrap();
    assert_eq!(def, back);
}

#[test]
fn canonical_json_is_key_order_independent() {
    let def = definition(BASIC_WORKFLOW);
    let a = serde_json::to_value(&def).unwrap();
    // Reorder top-level keys — canonical digest must not change.
    let mut reordered = serde_json::Map::new();
    for (k, v) in a.as_object().unwrap().iter().rev() {
        reordered.insert(k.clone(), v.clone());
    }
    let b = serde_json::Value::Object(reordered);
    let digest_a = synvelis_automation::canon::definition_digest(&def).unwrap();
    let def_b = decode_workflow_definition(b).unwrap();
    let digest_b = synvelis_automation::canon::definition_digest(&def_b).unwrap();
    assert_eq!(digest_a, digest_b);
}

#[test]
fn unsupported_schema_version_rejected() {
    let src = BASIC_WORKFLOW.replace("schema_version: 1", "schema_version: 2");
    let err = parse_workflow_yaml(&src).unwrap_err();
    assert!(
        matches!(
            err,
            AutomationError::UnsupportedDefinitionVersion { found: 2 }
        ),
        "got {err:?}"
    );
}

#[test]
fn unknown_fields_rejected_at_every_level() {
    // Top level.
    let src = BASIC_WORKFLOW.replace(
        "workflow_id: test.basic",
        "workflow_id: test.basic\nbogus_field: 1",
    );
    assert!(parse_workflow_yaml(&src).is_err());
    // Step level.
    let src = BASIC_WORKFLOW.replace(
        "id: tracks\n    capability",
        "id: tracks\n    bogus: 1\n    capability",
    );
    assert!(parse_workflow_yaml(&src).is_err());
    // Schema node level.
    let src = BASIC_WORKFLOW.replace(
        "type: id\n      kind: track",
        "type: id\n      kind: track\n      bogus: 1",
    );
    assert!(parse_workflow_yaml(&src).is_err());
    // Limits level.
    let src = BASIC_WORKFLOW.replace("max_steps: 100", "max_steps: 100\n  bogus_limit: 1");
    assert!(parse_workflow_yaml(&src).is_err());
}

#[test]
fn forbidden_authority_fields_have_no_representations() {
    // There is no principal/grant/approval/credential/shell/eval field
    // anywhere in the schema — unknown fields are rejected, and these
    // names are not special-cased.
    for forbidden in [
        "principal: admin",
        "grant: all",
        "approval: true",
        "api_key: sk-123",
        "shell: rm -rf /",
        "eval: process.exit()",
        "token: abc",
        "os_credential: x",
    ] {
        let src = BASIC_WORKFLOW.replace(
            "workflow_id: test.basic",
            &format!("workflow_id: test.basic\n{forbidden}"),
        );
        assert!(
            parse_workflow_yaml(&src).is_err(),
            "forbidden field accepted: {forbidden}"
        );
    }
    // Same inside a step.
    let src = BASIC_WORKFLOW.replace(
        "id: tracks\n    capability",
        "id: tracks\n    grant: root\n    capability",
    );
    assert!(parse_workflow_yaml(&src).is_err());
}

#[test]
fn invalid_identifiers_rejected() {
    let src = BASIC_WORKFLOW.replace("workflow_id: test.basic", "workflow_id: Bad Id!");
    assert!(parse_workflow_yaml(&src).is_err());
    let src = BASIC_WORKFLOW.replace("id: tracks", "id: 9bad");
    assert!(parse_workflow_yaml(&src).is_err());
    let src = BASIC_WORKFLOW.replace("name: track", "name: bad.name");
    assert!(parse_workflow_yaml(&src).is_err());
}

#[test]
fn duplicate_step_ids_rejected() {
    let src = BASIC_WORKFLOW.replace("id: done", "id: tracks");
    let err = parse_workflow_yaml(&src).unwrap_err();
    assert!(matches!(err, AutomationError::Definition { .. }), "{err:?}");
}

#[test]
fn empty_and_or_conditions_rejected() {
    let src = r"
schema_version: 1
workflow_id: test.empty-cond
version: 1
steps:
  - kind: if
    id: bad
    condition:
      op: and
      all:
    then:
      - kind: emit-result
        id: e
        output: o
        value: true
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
    // `all:` with no items parses as null → serde error, still rejected.
    assert!(parse_workflow_yaml(src).is_err());
}

#[test]
fn definition_schema_version_constant_is_one() {
    assert_eq!(DEFINITION_SCHEMA_VERSION, 1);
}

#[test]
fn json_decode_accepts_tagged_expr_form() {
    // Programmatic/builder path: canonical tagged exprs decode directly.
    let def = serde_json::json!({
        "schema_version": 1,
        "workflow_id": "test.json",
        "version": 1,
        "inputs": [],
        "steps": [{
            "kind": "emit-result",
            "id": "e",
            "output": "o",
            "value": {"expr": "literal", "value": true}
        }],
        "outputs": [{
            "name": "o",
            "schema": {"type": "bool"}
        }],
        "limits": {
            "max_steps": 10, "max_depth": 4, "max_iterations": 10,
            "max_runtime_ms": 1000, "max_output_bytes": 1000,
            "max_memory_bytes": 1000
        }
    });
    let def = decode_workflow_definition(def).unwrap();
    assert_eq!(def.workflow_id, "test.json");
}

#[test]
fn json_decode_rejects_bare_scalars_as_exprs() {
    // Canonical form requires tagged exprs — a bare scalar where an
    // Expr is expected must not silently coerce.
    let def = serde_json::json!({
        "schema_version": 1,
        "workflow_id": "test.json",
        "version": 1,
        "steps": [{
            "kind": "emit-result", "id": "e", "output": "o", "value": true
        }],
        "outputs": [{"name": "o", "schema": {"type": "bool"}}],
        "limits": {
            "max_steps": 10, "max_depth": 4, "max_iterations": 10,
            "max_runtime_ms": 1000, "max_output_bytes": 1000,
            "max_memory_bytes": 1000
        }
    });
    assert!(decode_workflow_definition(def).is_err());
}
