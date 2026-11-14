//! SV-SCRIPT-02 contract tests — `compile_action_recipe` lowers a
//! batch [`ActionRecipe`] through the unchanged AUTX-04
//! `MacroCompiler`, and every unsupported shape stays a typed
//! fail-closed rejection.

mod common;

use std::collections::BTreeMap;

use serde_json::json;
use synvelis_automation::schema::{FieldNode, ResourceKind, SchemaNode};
use synvelis_automation::{AutomationError, CompileError, compile_action_recipe};
use synvelis_gateway::ResourceKind as GatewayResourceKind;
use synvelis_script::{
    ACTION_RECIPE_SCHEMA_VERSION, ActionRecipe, RecipeMember, ScriptConsistency, ScriptTarget,
};

use common::registry;

fn project() -> String {
    "project-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".to_owned()
}

fn track() -> String {
    "track-bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned()
}

/// A resolved recipe member bound to `synvelis.project.add-note@1.0`
/// — the fixture registry's one synchronous `ProjectMutation` op.
fn add_note_member() -> RecipeMember {
    RecipeMember {
        name: "addNote".to_owned(),
        capability: synvelis_gateway::CapabilityRef {
            id: synvelis_gateway::CapabilityId::new("synvelis.project.add-note").unwrap(),
            version: synvelis_gateway::CapabilityVersion::new(1, 0),
        },
        domain: synvelis_gateway::OperationDomain::ProjectMutation,
        input_schema: synvelis_gateway::SchemaRef::new("synvelis.args.add-note", 1, 0),
        target: Some(ScriptTarget::Resource {
            kind: GatewayResourceKind::Track,
            id: track(),
        }),
        target_wire: None,
        args: json!({
            "pitch": 60,
            "velocity": 100,
            "start_tick": 0,
            "duration_ticks": 480,
        }),
    }
}

fn recipe(members: Vec<RecipeMember>) -> ActionRecipe {
    ActionRecipe {
        schema_version: ACTION_RECIPE_SCHEMA_VERSION,
        script_id: "demo.batch".to_owned(),
        script_version: 2,
        definition_digest: "d".repeat(64),
        binding_digest: "b".repeat(64),
        model_digest: "m".repeat(64),
        members,
        project: project(),
        consistency: ScriptConsistency::ExpectedRevision { revision: 7 },
        request_id: "req-recipe-1".to_owned(),
        task: None,
        grant: None,
        idempotency_key: None,
        recipe_digest: "r".repeat(64),
    }
}

#[test]
fn batch_recipe_compiles_through_macro_compiler() {
    let compiled = compile_action_recipe(
        &recipe(vec![add_note_member(), add_note_member()]),
        &registry(),
    )
    .expect("two mutation members compile");

    assert_eq!(compiled.ops.len(), 2);
    assert_eq!(compiled.bindings.len(), 0, "no create-then-reference");
    assert_eq!(compiled.expected_revision, Some(7));
    assert_eq!(compiled.macro_ref, "demo.batch@2.0");
    for op in &compiled.ops {
        assert_eq!(op["capability"]["id"], json!("synvelis.project.add-note"));
        assert_eq!(
            op["target"],
            json!({"shape": "resource", "kind": "track", "id": track()})
        );
        assert_eq!(op["args"]["pitch"], json!(60));
    }
    // batch_args is the binding-free ops object the gateway accepts.
    let args = compiled.batch_args().unwrap();
    assert_eq!(args["ops"].as_array().unwrap().len(), 2);
    assert!(args.get("bindings").is_none());
}

#[test]
fn single_member_recipe_is_not_a_batch() {
    let err = compile_action_recipe(&recipe(vec![add_note_member()]), &registry())
        .expect_err("single recipes never take the batch path");
    assert!(matches!(err, AutomationError::Definition { .. }));
}

#[test]
fn unknown_capability_fails_closed() {
    let mut member = add_note_member();
    member.capability = synvelis_gateway::CapabilityRef {
        id: synvelis_gateway::CapabilityId::new("synvelis.no.such").unwrap(),
        version: synvelis_gateway::CapabilityVersion::new(1, 0),
    };
    let err = compile_action_recipe(&recipe(vec![member, add_note_member()]), &registry())
        .expect_err("unknown capability must reject");
    assert!(matches!(
        err,
        AutomationError::Compile(CompileError::UnknownCapability { .. })
    ));
}

#[test]
fn non_mutation_domain_member_fails_closed() {
    // synvelis.job.render is async-operation in the fixture registry —
    // a member that slipped past recipe classification still cannot
    // join the batch (defense in depth: the compiler re-checks).
    let mut member = add_note_member();
    member.capability = synvelis_gateway::CapabilityRef {
        id: synvelis_gateway::CapabilityId::new("synvelis.job.render").unwrap(),
        version: synvelis_gateway::CapabilityVersion::new(1, 0),
    };
    member.domain = synvelis_gateway::OperationDomain::AsyncOperation;
    member.input_schema = synvelis_gateway::SchemaRef::new("synvelis.args.render", 1, 0);
    member.target = None;
    member.target_wire = None;
    member.args = json!({"range": {"domain": "media", "start": 0, "len": 480}, "format": "wav"});
    let err = compile_action_recipe(&recipe(vec![member, add_note_member()]), &registry())
        .expect_err("async member must reject");
    assert!(matches!(
        err,
        AutomationError::Compile(
            CompileError::DomainMismatch { .. } | CompileError::ExecutionMismatch { .. }
        )
    ));
}

#[test]
fn nested_batch_capability_fails_closed() {
    // The batch capability id is rejected before registry resolution —
    // even though the fixture snapshot does not register it.
    let mut member = add_note_member();
    member.capability = synvelis_gateway::CapabilityRef {
        id: synvelis_gateway::CapabilityId::new("synvelis.operation.batch").unwrap(),
        version: synvelis_gateway::CapabilityVersion::new(1, 0),
    };
    member.args = json!({"ops": []});
    member.target = None;
    member.target_wire = None;
    let err = compile_action_recipe(&recipe(vec![member, add_note_member()]), &registry())
        .expect_err("nested batch must reject");
    assert!(matches!(
        err,
        AutomationError::Compile(CompileError::NestedBatch { .. })
    ));
}

#[test]
fn schema_violating_args_fail_closed() {
    let mut member = add_note_member();
    member.args = json!({"pitch": 999, "velocity": 100, "start_tick": 0, "duration_ticks": 480});
    let err = compile_action_recipe(&recipe(vec![member, add_note_member()]), &registry())
        .expect_err("out-of-range arg must reject");
    assert!(matches!(
        err,
        AutomationError::Compile(CompileError::SchemaViolation { .. })
    ));
}

#[test]
fn target_kind_mismatch_fails_closed() {
    // A clip id where the contract wants a track — the literal target
    // is re-validated against the automation snapshot's contract.
    let mut member = add_note_member();
    member.target = Some(ScriptTarget::Resource {
        kind: GatewayResourceKind::Clip,
        id: "clip-cccccccccccccccccccccccccccccccc".to_owned(),
    });
    let err = compile_action_recipe(&recipe(vec![member, add_note_member()]), &registry())
        .expect_err("wrong-kind target must reject");
    assert!(matches!(
        err,
        AutomationError::Compile(CompileError::TargetKindMismatch { .. })
    ));
}

#[test]
fn oversized_member_list_fails_closed() {
    // 65 members — beyond MAX_MACRO_MEMBERS — reject at definition
    // validation inside the compiler.
    let members = (0..65).map(|_| add_note_member()).collect();
    let err =
        compile_action_recipe(&recipe(members), &registry()).expect_err("65 members must reject");
    assert!(matches!(err, AutomationError::Definition { .. }));
}

// ── Fixture sanity: the automation snapshot mirrors the op ────

#[test]
fn fixture_registry_covers_the_member_capability() {
    // Guards the fixture itself: the member capability must resolve
    // as available project-mutation sync in the automation snapshot,
    // or every test above would pass for the wrong reason.
    let snap = registry();
    snap.validate().unwrap();
    let cap = snap
        .resolve_capability(&synvelis_automation::VersionedRef::new(
            "synvelis.project.add-note",
            1,
            0,
        ))
        .unwrap();
    assert_eq!(
        cap.domain,
        synvelis_automation::registry::OperationDomain::ProjectMutation
    );
    assert_eq!(
        cap.execution,
        synvelis_automation::registry::ExecutionContract::Sync
    );
    assert_eq!(
        cap.availability,
        synvelis_automation::registry::Availability::Available
    );
    // The target contract demands a track resource.
    assert!(matches!(
        cap.target,
        synvelis_automation::registry::TargetContract::Resource {
            kind: ResourceKind::Track
        }
    ));
    // The input schema is the declared object — spot-check a field.
    let schema = snap
        .resolve_schema(&synvelis_automation::VersionedRef::new(
            "synvelis.args.add-note",
            1,
            0,
        ))
        .unwrap();
    let SchemaNode::Object { fields, .. } = schema else {
        panic!("add-note input must be an object schema");
    };
    assert!(fields.contains_key("pitch"));
    let _ = FieldNode {
        schema: SchemaNode::Bool,
        required: true,
    };
    let _ = BTreeMap::<String, FieldNode>::new();
}
