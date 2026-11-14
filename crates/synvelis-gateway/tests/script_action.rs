//! SV-SCRIPT-02 end-to-end golden — synvelis#272.
//!
//! A Luau Action Script's typed recipe lowers through the *real*
//! Gateway path, never a parallel surface:
//!
//! `ScriptActionDefinition` + `ScriptApiModel`/`BindingSet` (built over
//! the rig's live registries) → `ActionRecipe::build` →
//! `compile_action_recipe` (AUTX-04 `MacroCompiler`) →
//! `ActionRecipe::to_batch_draft`/`to_draft` → `InvocationDraft` →
//! `Gateway::handle` (parse → bind → invoke) → prepare → proposal →
//! apply → `CommitGate::commit` → undo — the same route GUI/MCP/CLI
//! take, with Gateway authorization + schema re-validation intact
//! (analysis success ≠ authorization).
//!
//! The automation registry snapshot below is the serialized mirror a
//! host projects from the live registries — the same stand-in the
//! AUTX-04 fixtures use; it is deliberately *not* the script-side
//! snapshot, so a disagreement between the two views is a typed
//! rejection (drift detection by layering).

mod rig;

use rig::*;
use serde_json::{Value, json};
use synvelis_automation::parse_registry_snapshot_yaml;
use synvelis_gateway::*;
use synvelis_project::{ClipId, MusicalTick, ProjectRevision, TimeAnchor};
use synvelis_script::{
    ACTION_DEFINITION_SCHEMA_VERSION, ACTION_RECIPE_SCHEMA_VERSION, ActionRecipe, ActionRecipeKind,
    AvailableBackends, BackendRequirement, BindingSet, InvocationDraft, LanguageId,
    MAX_RECIPE_MEMBERS, RecipeMemberSpec, RecipeSpec, RegistrySnapshot, SURFACE_SCHEMA_VERSION,
    ScriptActionDefinition, ScriptApiModel, ScriptConsistency, ScriptSurfaceEntry,
    ScriptSurfaceSpec, ScriptTarget, ScriptVisibility, sha256_hex,
};

/// The AUTX-04 registry mirror for the two mutation ops the script
/// surface exposes — block-style YAML only (the restricted subset the
/// automation frontend accepts).
const BATCH_REGISTRY_YAML: &str = r"
schema_version: 1
capabilities:
  - id: synvelis.clip.mute
    version:
      major: 1
      minor: 0
    domain: project-mutation
    input_schema: synvelis.clip.mute.input@1.0
    output_schema: synvelis.operation.output@1.0
    target:
      shape: resource
      kind: clip
    effect_bound:
      - write
    required_permissions:
      - mutate-project
    data_access:
      - metadata-read
    execution: sync
    availability:
      state: available
  - id: synvelis.clip.move
    version:
      major: 1
      minor: 0
    domain: project-mutation
    input_schema: synvelis.clip.move.input@1.0
    output_schema: synvelis.operation.output@1.0
    target:
      shape: resource
      kind: clip
    effect_bound:
      - write
    required_permissions:
      - mutate-project
    data_access:
      - metadata-read
    execution: sync
    availability:
      state: available
schemas:
  - name: synvelis.clip.mute.input
    version:
      major: 1
      minor: 0
    schema:
      type: object
      deny_unknown: true
      fields:
        muted:
          schema:
            type: bool
          required: true
  - name: synvelis.clip.move.input
    version:
      major: 1
      minor: 0
    schema:
      type: object
      deny_unknown: true
      fields:
        anchor:
          schema:
            type: object
          required: true
  - name: synvelis.operation.output
    version:
      major: 1
      minor: 0
    schema:
      type: object
";

fn clip_tagged() -> String {
    tagged(&clip11())
}

fn project_tagged() -> String {
    project().to_tagged()
}

/// The script-side stack over the rig's *live* registries — the same
/// `RegistrySnapshot::capture` + model + bindings SV-SCRIPT-01
/// produces for a real host.
fn script_stack() -> (RegistrySnapshot, ScriptApiModel, BindingSet) {
    let rig = fixture_rig();
    let snapshot =
        RegistrySnapshot::capture(rig.gw.registry(), rig.gw.schemas()).expect("snapshot captures");
    let surface = ScriptSurfaceSpec {
        schema_version: SURFACE_SCHEMA_VERSION,
        entries: vec![
            ScriptSurfaceEntry {
                name: "muteClip".to_owned(),
                capability: cap_ref("synvelis.clip.mute"),
                visibility: ScriptVisibility::Public,
                description: None,
            },
            ScriptSurfaceEntry {
                name: "moveClip".to_owned(),
                capability: cap_ref("synvelis.clip.move"),
                visibility: ScriptVisibility::Public,
                description: None,
            },
        ],
    };
    surface.validate().unwrap();
    let model = ScriptApiModel::build(&snapshot, &surface).expect("model builds");
    let bindings = BindingSet::generate(&model).expect("bindings generate");
    (snapshot, model, bindings)
}

fn definition(snapshot: &RegistrySnapshot) -> ScriptActionDefinition {
    let def = ScriptActionDefinition {
        schema_version: ACTION_DEFINITION_SCHEMA_VERSION,
        script_id: "demo.clip-actions".to_owned(),
        version: 1,
        language_id: LanguageId::Luau,
        backend_requirement: BackendRequirement::Luau,
        source_digest: sha256_hex(b"clip actions v1"),
        input_schema: SchemaRef::new("synvelis.clip.mute.input", 1, 0),
        output_schema: SchemaRef::new("synvelis.operation.output", 1, 0),
    };
    def.validate(&AvailableBackends::luau(), snapshot)
        .expect("definition validates against the live snapshot");
    def
}

fn member(name: &str, args: Value) -> RecipeMemberSpec {
    RecipeMemberSpec {
        name: name.to_owned(),
        args,
        target: Some(ScriptTarget::Resource {
            kind: ResourceKind::Clip,
            id: clip_tagged(),
        }),
        project: project_tagged(),
        consistency: ScriptConsistency::ExpectedRevision { revision: 1 },
    }
}

fn mute_member() -> RecipeMemberSpec {
    member("muteClip", json!({"muted": true}))
}

fn move_member() -> RecipeMemberSpec {
    member(
        "moveClip",
        json!({"anchor": {"kind": "Musical", "value": 960}}),
    )
}

fn spec(members: Vec<RecipeMemberSpec>) -> RecipeSpec {
    RecipeSpec {
        members,
        request_id: "req-script-action-1".to_owned(),
        task: Some("t1".to_owned()),
        grant: Some("g1".to_owned()),
        idempotency_key: None,
    }
}

fn build_recipe(
    snapshot: &RegistrySnapshot,
    model: &ScriptApiModel,
    bindings: &BindingSet,
    members: Vec<RecipeMemberSpec>,
) -> ActionRecipe {
    ActionRecipe::build(
        &definition(snapshot),
        &AvailableBackends::luau(),
        snapshot,
        model,
        bindings,
        spec(members),
    )
    .expect("recipe builds")
}

/// Issues the agent's task+grant covering prepare, the member ops,
/// preview, and apply — the same shape `apply_grant` produces for the
/// hand-written request path.
fn issue_script_grant(rig: &mut Rig) {
    let grant = apply_grant(
        "g1",
        "t1",
        &[
            "synvelis.operation.batch",
            "synvelis.clip.mute",
            "synvelis.clip.move",
        ],
        ApprovalPolicy::DelegatedApply,
    );
    issue_agent(rig.store.as_ref(), "t1", grant);
}

/// Draft → `Gateway::handle` → Completed prepare result
/// `(proposal, digest)` — the real bind/invoke route.
fn prepare_via_draft(rig: &mut Rig, draft: &InvocationDraft) -> (String, String) {
    draft.verify_wire().expect("draft survives wire parse");
    let result = complete(rig.gw.handle(&draft.to_wire_value(), agent()).unwrap());
    (
        result["proposal"].as_str().unwrap().to_owned(),
        result["digest"].as_str().unwrap().to_owned(),
    )
}

#[test]
fn batch_recipe_commits_as_one_composite_and_undoes_once() {
    // The golden: ScriptActionDefinition → ActionRecipe → AUTX-04
    // compile → prepare draft → real Gateway → Composite commit.
    // Oracle parity with #263: one expected-revision CAS, one
    // `ProjectOp::Composite`, one Undo restores both member effects.
    let mut rig = fixture_rig();
    let (snapshot, model, bindings) = script_stack();
    let recipe = build_recipe(
        &snapshot,
        &model,
        &bindings,
        vec![mute_member(), move_member()],
    );
    assert_eq!(recipe.kind(), ActionRecipeKind::Batch);
    assert_eq!(recipe.schema_version, ACTION_RECIPE_SCHEMA_VERSION);
    recipe.verify_definition(&definition(&snapshot)).unwrap();

    let auto = parse_registry_snapshot_yaml(BATCH_REGISTRY_YAML).expect("mirror parses");
    let compiled = synvelis_automation::compile_action_recipe(&recipe, &auto)
        .expect("recipe compiles through MacroCompiler");
    assert_eq!(compiled.bindings.len(), 0, "no create-then-reference");
    let draft = recipe
        .to_batch_draft(&compiled.batch_args().unwrap())
        .expect("batch draft lowers");
    // The draft pins the batch capability inside the prepare envelope.
    assert_eq!(
        draft.args["operation"]["capability"]["id"],
        json!("synvelis.operation.batch")
    );
    assert_eq!(draft.task.as_deref(), Some("t1"));
    assert_eq!(draft.grant.as_deref(), Some("g1"));

    issue_script_grant(&mut rig);
    let (proposal, digest) = prepare_via_draft(&mut rig, &draft);

    // The retained record is the #263 oracle: two member commands,
    // one Composite op, effects on the shared clip.
    let record = record_for(&rig, &json!({"proposal": proposal, "digest": digest}));
    assert_eq!(record.prepared.commands.len(), 2);
    assert_eq!(record.prepared.commands[0].op, "set-clip-muted");
    assert_eq!(record.prepared.commands[1].op, "move-clip");
    assert!(matches!(
        record.op,
        synvelis_project::ProjectOp::Composite { .. }
    ));
    assert_eq!(record.prepared.base.revision, ProjectRevision(1));

    // Apply → CommitRequested → commit: revision 1 → 2, both effects land.
    let (pref, _op) = {
        let operation = accept_apply(&mut rig, &proposal, &digest, "t1", "g1");
        (ProposalRef::new(&proposal).unwrap(), operation)
    };
    let dir = tmpdir("script-action-batch");
    let (gate, _report, _store) = boot(&rig, &dir);
    let receipt = gate.commit(&pref).expect("composite commit lands");
    assert_eq!(receipt.base_revision, ProjectRevision(1));
    assert_eq!(receipt.revision, ProjectRevision(2));

    let clip_state = |gate: &CommitGate| {
        gate.inspect(|c| {
            let clip = &c.live().clips[&ClipId::new(11)];
            (clip.muted, clip.placement.anchor)
        })
    };
    let (muted, anchor) = clip_state(&gate);
    assert!(muted, "mute member committed");
    assert_eq!(
        anchor,
        TimeAnchor::Musical(MusicalTick::new(960)),
        "move member committed at the pinned anchor"
    );

    // One Undo restores the whole composite — never a partial state.
    assert!(gate.undo(&human()).unwrap());
    let (muted, anchor) = clip_state(&gate);
    assert!(!muted);
    assert_ne!(anchor, TimeAnchor::Musical(MusicalTick::new(960)));
    assert!(gate.redo(&human()).unwrap());
    let (muted, _) = clip_state(&gate);
    assert!(muted);
}

#[test]
fn single_mutation_recipe_lowers_through_prepare_and_commits() {
    // Single-member recipes never touch the batch compiler — they
    // lower to the same prepare envelope directly.
    let mut rig = fixture_rig();
    let (snapshot, model, bindings) = script_stack();
    let recipe = build_recipe(&snapshot, &model, &bindings, vec![mute_member()]);
    assert_eq!(recipe.kind(), ActionRecipeKind::Single);

    let draft = recipe.to_draft().expect("single draft lowers");
    assert_eq!(draft.capability.id.as_str(), "synvelis.operation.prepare");
    assert_eq!(
        draft.args["operation"]["capability"]["id"],
        json!("synvelis.clip.mute")
    );

    issue_script_grant(&mut rig);
    let (proposal, digest) = prepare_via_draft(&mut rig, &draft);
    let record = record_for(&rig, &json!({"proposal": proposal, "digest": digest}));
    assert_eq!(record.prepared.commands.len(), 1);

    let operation = accept_apply(&mut rig, &proposal, &digest, "t1", "g1");
    let dir = tmpdir("script-action-single");
    let (gate, _report, _store) = boot(&rig, &dir);
    let receipt = gate
        .commit(&ProposalRef::new(&proposal).unwrap())
        .expect("single commit lands");
    assert_eq!(receipt.revision, ProjectRevision(2));
    let _ = operation;
    assert!(gate.inspect(|c| c.live().clips[&ClipId::new(11)].muted));
    assert!(gate.undo(&human()).unwrap());
    assert!(!gate.inspect(|c| c.live().clips[&ClipId::new(11)].muted));
}

#[test]
fn query_member_lowers_to_direct_draft_and_invokes() {
    // A non-mutation member is a *direct* draft — no prepare envelope —
    // and still rides `Gateway::handle` bind→invoke with the caller's
    // authority (here: trusted human manual authority, AHG-007).
    let mut rig = fixture_rig();
    let snapshot =
        RegistrySnapshot::capture(rig.gw.registry(), rig.gw.schemas()).expect("snapshot captures");
    let surface = ScriptSurfaceSpec {
        schema_version: SURFACE_SCHEMA_VERSION,
        entries: vec![ScriptSurfaceEntry {
            name: "queryClips".to_owned(),
            capability: cap_ref("synvelis.context.query"),
            visibility: ScriptVisibility::Public,
            description: None,
        }],
    };
    surface.validate().unwrap();
    let model = ScriptApiModel::build(&snapshot, &surface).expect("model builds");
    let bindings = BindingSet::generate(&model).expect("bindings generate");
    let def = definition(&snapshot);

    let query_member = RecipeMemberSpec {
        name: "queryClips".to_owned(),
        args: json!({"selector": {"all": true}}),
        target: None,
        project: project_tagged(),
        consistency: ScriptConsistency::Latest,
    };
    let recipe = ActionRecipe::build(
        &def,
        &AvailableBackends::luau(),
        &snapshot,
        &model,
        &bindings,
        RecipeSpec {
            members: vec![query_member],
            request_id: "req-script-query-1".to_owned(),
            task: None,
            grant: None,
            idempotency_key: None,
        },
    )
    .expect("query recipe builds");
    assert_eq!(recipe.kind(), ActionRecipeKind::Single);

    let draft = recipe.to_draft().expect("direct draft lowers");
    assert_eq!(draft.capability.id.as_str(), "synvelis.context.query");
    assert_eq!(draft.schema.name, "synvelis.context.query.input");
    assert_eq!(draft.consistency["kind"], json!("latest"));
    assert!(draft.target.is_none());

    // Real invoke: human authority, no grant — analysis success was
    // never the authorization.
    draft.verify_wire().expect("draft survives wire parse");
    let outcome = rig.gw.handle(&draft.to_wire_value(), human()).unwrap();
    let GatewayOutcome::Completed { result } = outcome else {
        panic!("expected Completed, got {outcome:?}");
    };
    assert!(result["items"].as_array().is_some());
}

#[test]
fn gateway_revalidation_rejects_what_analysis_accepted() {
    // Defense in depth: the script/automation layers accept ≤64
    // members, but the *Gateway's own* batch schema is the authority —
    // a hand-assembled 65-op batch args object must fail closed at
    // bind, proving analysis success ≠ authorization.
    let mut rig = fixture_rig();
    let (snapshot, model, bindings) = script_stack();
    let recipe = build_recipe(
        &snapshot,
        &model,
        &bindings,
        vec![mute_member(), move_member()],
    );
    let auto = parse_registry_snapshot_yaml(BATCH_REGISTRY_YAML).expect("mirror parses");
    let compiled = synvelis_automation::compile_action_recipe(&recipe, &auto).unwrap();

    // Inflate past the gateway's 64-op bound — the recipe/compiler
    // could never produce this, but the wire boundary must not trust it.
    let mut batch_args = compiled.batch_args().unwrap();
    let ops = batch_args["ops"].as_array().unwrap().clone();
    let first = ops[0].clone();
    let mut inflated = ops;
    while inflated.len() <= MAX_RECIPE_MEMBERS {
        inflated.push(first.clone());
    }
    batch_args["ops"] = json!(inflated);

    let draft = recipe.to_batch_draft(&batch_args).expect("envelope lowers");
    let err = rig
        .gw
        .handle(&draft.to_wire_value(), agent())
        .expect_err("65 ops must reject at the authoritative schema");
    assert!(matches!(
        err.code,
        GatewayError::SchemaViolation { .. } | GatewayError::Denied { .. }
    ));
}
