//! SV-SCRIPT-02 contract tests — `ScriptActionDefinition` validation,
//! drift rejection, `ActionRecipe` classification/lowering, and the
//! typed fail-closed vocabulary for unsupported batch members.

mod common;

use std::sync::Arc;

use serde_json::{Value, json};
use synvelis_gateway::{
    Availability, CapabilityOperation, OperationDomain, ResourceKind, TargetContract,
};
use synvelis_script::{
    ACTION_DEFINITION_SCHEMA_VERSION, ACTION_RECIPE_SCHEMA_VERSION, ActionRecipe, ActionRecipeKind,
    AvailableBackends, BackendRequirement, BindingSet, InvocationDraft, LanguageId,
    MAX_RECIPE_MEMBERS, RecipeMemberSpec, RecipeSpec, ScriptActionDefinition, ScriptApiModel,
    ScriptConsistency, ScriptSurfaceEntry, ScriptTarget, ScriptVisibility, sha256_hex,
};

use common::*;

fn backends() -> AvailableBackends {
    AvailableBackends::luau()
}

fn snapshot() -> synvelis_script::RegistrySnapshot {
    sample_snapshot()
}

fn model() -> ScriptApiModel {
    sample_model()
}

fn bindings() -> BindingSet {
    binding_set()
}

/// A valid Luau action definition pinned to fixture schemas.
fn definition() -> ScriptActionDefinition {
    ScriptActionDefinition {
        schema_version: ACTION_DEFINITION_SCHEMA_VERSION,
        script_id: "demo.action".to_owned(),
        version: 3,
        language_id: LanguageId::Luau,
        backend_requirement: BackendRequirement::Luau,
        source_digest: sha256_hex(b"demo action source v3"),
        input_schema: sref("midi.add-note.input", 1, 0),
        output_schema: sref("midi.add-note.output", 1, 0),
    }
}

fn spec_member(name: &str, args: Value, target: Option<ScriptTarget>) -> RecipeMemberSpec {
    RecipeMemberSpec {
        name: name.to_owned(),
        args,
        target,
        project: project_id(),
        consistency: ScriptConsistency::ExpectedRevision { revision: 7 },
    }
}

fn add_note_member() -> RecipeMemberSpec {
    spec_member(
        "addNote",
        add_note_call().args,
        Some(ScriptTarget::Resource {
            kind: ResourceKind::Track,
            id: track_id(),
        }),
    )
}

fn split_member() -> RecipeMemberSpec {
    spec_member(
        "splitClip",
        json!({
            "at": {"domain": "project-musical", "start": "960", "len": "480"},
            "mode": "copy",
        }),
        Some(ScriptTarget::Resource {
            kind: ResourceKind::Clip,
            id: clip_id(),
        }),
    )
}

fn move_member() -> RecipeMemberSpec {
    spec_member(
        "moveClips",
        json!({"delta": 5}),
        Some(ScriptTarget::ResourceSet {
            kind: ResourceKind::Clip,
            ids: vec![clip_id(), tagged_id("clip", 'e')],
        }),
    )
}

fn create_member() -> RecipeMemberSpec {
    spec_member(
        "createClip",
        json!({
            "name": "take-b",
            "start": {"domain": "media", "start": "0", "len": "480"},
        }),
        Some(ScriptTarget::Create {
            parent_kind: ResourceKind::Track,
            parent: track_id(),
        }),
    )
}

fn query_member() -> RecipeMemberSpec {
    RecipeMemberSpec {
        name: "trackInfo".to_owned(),
        args: json!({}),
        target: Some(ScriptTarget::Resource {
            kind: ResourceKind::Track,
            id: track_id(),
        }),
        project: project_id(),
        consistency: ScriptConsistency::Latest,
    }
}

fn runtime_member() -> RecipeMemberSpec {
    RecipeMemberSpec {
        name: "play".to_owned(),
        args: json!({}),
        target: None,
        project: project_id(),
        consistency: ScriptConsistency::Unconditional,
    }
}

fn spec(members: Vec<RecipeMemberSpec>) -> RecipeSpec {
    RecipeSpec {
        members,
        request_id: "req-action-0001".to_owned(),
        task: None,
        grant: None,
        idempotency_key: None,
    }
}

fn build(spec: RecipeSpec) -> Result<ActionRecipe, synvelis_script::ScriptApiError> {
    ActionRecipe::build(
        &definition(),
        &backends(),
        &snapshot(),
        &model(),
        &bindings(),
        spec,
    )
}

// ── ScriptActionDefinition ───────────────────────────────────

#[test]
fn definition_validates_and_digests() {
    let def = definition();
    def.validate(&backends(), &snapshot()).unwrap();
    let digest = def.digest().unwrap();
    assert_eq!(digest.len(), 64);
    assert!(digest.bytes().all(|b| b.is_ascii_hexdigit()));
    // Deterministic: same definition → same digest.
    assert_eq!(def.digest().unwrap(), digest);
    def.verify_source(&def.source_digest.clone()).unwrap();
}

#[test]
fn definition_rejects_unsupported_schema_version() {
    let mut def = definition();
    def.schema_version = ACTION_DEFINITION_SCHEMA_VERSION + 1;
    let err = def.validate(&backends(), &snapshot()).unwrap_err();
    assert!(matches!(
        err,
        synvelis_script::ScriptApiError::UnsupportedDefinitionVersion { found }
            if found == ACTION_DEFINITION_SCHEMA_VERSION + 1
    ));
}

#[test]
fn definition_rejects_bad_script_id_and_digest() {
    let mut def = definition();
    def.script_id = "Bad Id!".to_owned();
    assert!(matches!(
        def.validate(&backends(), &snapshot()),
        Err(synvelis_script::ScriptApiError::Definition { .. })
    ));

    let mut def = definition();
    def.source_digest = "not-hex".to_owned();
    assert!(matches!(
        def.validate(&backends(), &snapshot()),
        Err(synvelis_script::ScriptApiError::Definition { .. })
    ));
}

#[test]
fn definition_rejects_language_backend_mismatch() {
    // Luau language with no backend requirement.
    let mut def = definition();
    def.backend_requirement = BackendRequirement::None;
    assert!(matches!(
        def.validate(&backends(), &snapshot()),
        Err(synvelis_script::ScriptApiError::LanguageBackendMismatch { .. })
    ));

    // YAML language demanding a Luau backend.
    let mut def = definition();
    def.language_id = LanguageId::Yaml;
    assert!(matches!(
        def.validate(&backends(), &snapshot()),
        Err(synvelis_script::ScriptApiError::LanguageBackendMismatch { .. })
    ));
}

#[test]
fn definition_fails_closed_without_backend() {
    // backend_requirement: luau but the host declares no Luau backend.
    let err = definition()
        .validate(&AvailableBackends::default(), &snapshot())
        .unwrap_err();
    assert!(matches!(
        err,
        synvelis_script::ScriptApiError::BackendUnavailable { ref required }
            if required == "luau"
    ));
}

#[test]
fn definition_rejects_unresolved_schemas() {
    let mut def = definition();
    def.input_schema = sref("no.such.schema", 1, 0);
    assert!(matches!(
        def.validate(&backends(), &snapshot()),
        Err(synvelis_script::ScriptApiError::UnresolvedDefinitionSchema { .. })
    ));
}

#[test]
fn source_drift_is_typed_fail_closed() {
    let def = definition();
    // Same declared digest → ok; anything else → SourceDrift.
    def.verify_source(&def.source_digest.clone()).unwrap();
    let err = def
        .verify_source(&sha256_hex(b"tampered source"))
        .unwrap_err();
    assert!(matches!(
        err,
        synvelis_script::ScriptApiError::SourceDrift { .. }
    ));
}

// ── ActionRecipe classification ──────────────────────────────

#[test]
fn single_mutation_lowers_to_prepare_draft() {
    let recipe = build(spec(vec![add_note_member()])).unwrap();
    assert_eq!(recipe.kind(), ActionRecipeKind::Single);
    assert_eq!(recipe.schema_version, ACTION_RECIPE_SCHEMA_VERSION);
    assert_eq!(recipe.script_id, "demo.action");
    assert_eq!(recipe.script_version, 3);
    assert_eq!(recipe.members.len(), 1);
    assert_eq!(recipe.expected_revision(), Some(7));

    let draft: InvocationDraft = recipe.to_draft().unwrap();
    // ProjectMutation members always lower through prepare — the op
    // capabilities carry no direct executor.
    assert_eq!(draft.capability.id.as_str(), "synvelis.operation.prepare");
    assert_eq!(draft.schema.name, "synvelis.operation.prepare.input");
    assert_eq!(draft.project, project_id());
    assert_eq!(draft.consistency["kind"], json!("expected-revision"));
    let operation = &draft.args["operation"];
    assert_eq!(
        operation["capability"]["id"],
        json!("synvelis.midi.add-note")
    );
    assert_eq!(operation["capability"]["version"]["major"], json!(1));
    assert_eq!(operation["target"]["id"], json!(track_id()));
    // i64 out-of-f64-range arg decoded to a JSON integer, not a string.
    assert_eq!(operation["args"]["sample_hint"], json!(i64::MAX));
    // The draft survives the authoritative wire parse.
    draft.verify_wire().unwrap();
}

#[test]
fn single_query_lowers_to_direct_draft() {
    let recipe = build(spec(vec![query_member()])).unwrap();
    assert_eq!(recipe.kind(), ActionRecipeKind::Single);
    let draft = recipe.to_draft().unwrap();
    assert_eq!(draft.capability.id.as_str(), "synvelis.query.track-info");
    assert_eq!(draft.schema.name, "query.track-info.input");
    assert_eq!(draft.consistency["kind"], json!("latest"));
    draft.verify_wire().unwrap();
}

#[test]
fn single_runtime_control_lowers_to_direct_draft() {
    let recipe = build(spec(vec![runtime_member()])).unwrap();
    let draft = recipe.to_draft().unwrap();
    assert_eq!(draft.capability.id.as_str(), "synvelis.transport.play");
    assert_eq!(draft.consistency["kind"], json!("unconditional"));
    assert!(draft.target.is_none());
    draft.verify_wire().unwrap();
}

#[test]
fn batch_recipe_collects_project_mutation_members() {
    let recipe = build(spec(vec![
        add_note_member(),
        split_member(),
        move_member(),
        create_member(),
    ]))
    .unwrap();
    assert_eq!(recipe.kind(), ActionRecipeKind::Batch);
    assert_eq!(recipe.members.len(), 4);
    assert_eq!(recipe.project, project_id());
    assert_eq!(
        recipe.consistency,
        ScriptConsistency::ExpectedRevision { revision: 7 }
    );
    // Digests pin the whole surface: definition + bindings + model.
    assert_eq!(recipe.definition_digest.len(), 64);
    assert_eq!(recipe.binding_digest.len(), 64);
    assert_eq!(recipe.model_digest.len(), 64);
    assert_eq!(recipe.recipe_digest.len(), 64);
    // Deterministic: rebuilding the same spec yields the same digest.
    let again = build(spec(vec![
        add_note_member(),
        split_member(),
        move_member(),
        create_member(),
    ]))
    .unwrap();
    assert_eq!(recipe.recipe_digest, again.recipe_digest);

    // Wrong-kind lowering is a typed error, both directions.
    assert!(matches!(
        recipe.to_draft(),
        Err(synvelis_script::ScriptApiError::RecipeKindMismatch { .. })
    ));
    let single = build(spec(vec![add_note_member()])).unwrap();
    assert!(matches!(
        single.to_batch_draft(&json!({"ops": []})),
        Err(synvelis_script::ScriptApiError::RecipeKindMismatch { .. })
    ));
}

#[test]
fn batch_draft_wraps_compiled_ops_in_prepare() {
    let recipe = build(spec(vec![add_note_member(), split_member()])).unwrap();
    let batch_args = json!({"ops": [
        {"capability": {"id": "synvelis.midi.add-note", "version": {"major": 1, "minor": 0}},
         "target": {"shape": "resource", "kind": "track", "id": track_id()},
         "args": {}},
        {"capability": {"id": "synvelis.clip.split", "version": {"major": 1, "minor": 0}},
         "target": {"shape": "resource", "kind": "clip", "id": clip_id()},
         "args": {}},
    ]});
    let draft = recipe.to_batch_draft(&batch_args).unwrap();
    assert_eq!(draft.capability.id.as_str(), "synvelis.operation.prepare");
    assert_eq!(
        draft.args["operation"]["capability"]["id"],
        json!("synvelis.operation.batch")
    );
    assert_eq!(draft.args["operation"]["args"], batch_args);
    draft.verify_wire().unwrap();

    // Malformed batch args fail closed.
    assert!(recipe.to_batch_draft(&json!({"ops": "nope"})).is_err());
    assert!(recipe.to_batch_draft(&json!({})).is_err());
}

// ── Typed batch rejections ───────────────────────────────────

#[test]
fn empty_recipe_rejects() {
    let err = build(spec(vec![])).unwrap_err();
    assert!(matches!(err, synvelis_script::ScriptApiError::EmptyRecipe));
}

#[test]
fn oversized_recipe_rejects() {
    let members = (0..=MAX_RECIPE_MEMBERS)
        .map(|_| add_note_member())
        .collect::<Vec<_>>();
    let err = build(spec(members)).unwrap_err();
    assert!(matches!(
        err,
        synvelis_script::ScriptApiError::TooManyMembers { members, max }
            if members == MAX_RECIPE_MEMBERS + 1 && max == MAX_RECIPE_MEMBERS
    ));
}

#[test]
fn non_mutation_member_rejects_batch() {
    // Query + mutation cannot share one atomic batch — route to
    // Workflow instead (typed reject, never silent partial).
    let err = build(spec(vec![add_note_member(), query_member()])).unwrap_err();
    assert!(matches!(
        err,
        synvelis_script::ScriptApiError::UnsupportedMemberDomain { .. }
    ));

    let err = build(spec(vec![runtime_member(), add_note_member()])).unwrap_err();
    assert!(matches!(
        err,
        synvelis_script::ScriptApiError::UnsupportedMemberDomain { .. }
    ));
}

#[test]
fn cross_project_members_reject() {
    let mut other = add_note_member();
    other.project = tagged_id("project", 'f');
    let err = build(spec(vec![add_note_member(), other])).unwrap_err();
    assert!(matches!(
        err,
        synvelis_script::ScriptApiError::CrossProject { .. }
    ));
}

#[test]
fn inconsistent_consistency_rejects() {
    let mut other = split_member();
    other.consistency = ScriptConsistency::ExpectedRevision { revision: 8 };
    let err = build(spec(vec![add_note_member(), other])).unwrap_err();
    assert!(matches!(
        err,
        synvelis_script::ScriptApiError::ConsistencyMismatch { .. }
    ));
}

#[test]
fn nested_batch_member_rejects() {
    // A surface entry bound to `synvelis.operation.batch` itself —
    // registering the capability + schema refs the fixture already
    // carries (batch.move.input/output stand in as its schemas).
    let (mut caps, schemas) = sample_registries();
    caps.register(
        descriptor(
            "synvelis.operation.batch",
            cver(1, 0),
            OperationDomain::ProjectMutation,
            sref("batch.move.input", 1, 0),
            sref("batch.move.output", 1, 0),
            TargetContract::None,
            Availability::Available,
        ),
        None::<Arc<dyn CapabilityOperation>>,
    )
    .unwrap();
    let snapshot = synvelis_script::RegistrySnapshot::capture(&caps, &schemas).unwrap();
    let mut surface = sample_surface();
    surface.entries.push(ScriptSurfaceEntry {
        name: "doBatch".to_owned(),
        capability: synvelis_gateway::CapabilityRef {
            id: vid("synvelis.operation.batch"),
            version: cver(1, 0),
        },
        visibility: ScriptVisibility::Public,
        description: None,
    });
    let model = ScriptApiModel::build(&snapshot, &surface).unwrap();
    let bindings = BindingSet::generate(&model).unwrap();

    let mut batch_member = spec_member("doBatch", json!({"delta": 1}), None);
    batch_member.consistency = ScriptConsistency::ExpectedRevision { revision: 7 };

    let err = ActionRecipe::build(
        &definition(),
        &backends(),
        &snapshot,
        &model,
        &bindings,
        spec(vec![add_note_member(), batch_member]),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        synvelis_script::ScriptApiError::NestedBatch { .. }
    ));
}

#[test]
fn member_resolution_failures_stay_typed() {
    // Unknown script name.
    let mut member = add_note_member();
    member.name = "noSuchApi".to_owned();
    let err = build(spec(vec![member])).unwrap_err();
    assert!(matches!(
        err,
        synvelis_script::ScriptApiError::UnknownApi { .. }
    ));

    // Non-invocable surface entry (disabled capability).
    let err = build(spec(vec![spec_member("legacyOp", json!({}), None)])).unwrap_err();
    assert!(matches!(
        err,
        synvelis_script::ScriptApiError::NotInvocable { .. }
    ));

    // Hidden entry never binds.
    let err = build(spec(vec![spec_member(
        "hiddenOp",
        add_note_call().args,
        add_note_call().target,
    )]))
    .unwrap_err();
    assert!(matches!(
        err,
        synvelis_script::ScriptApiError::NotInvocable { .. }
    ));

    // Schema-invalid args surface the Gateway's own typed error.
    let mut member = add_note_member();
    member.args = json!({"pitch": 999});
    let err = build(spec(vec![member])).unwrap_err();
    assert!(matches!(err, synvelis_script::ScriptApiError::Gateway(_)));
}

#[test]
fn binding_model_drift_rejects_recipe_build() {
    // A BindingSet generated from a *different* model must not feed a
    // recipe — the drift check is the same one assemble_call relies on.
    let (caps, schemas) = sample_registries();
    let other_snapshot = synvelis_script::RegistrySnapshot::capture(&caps, &schemas).unwrap();
    let mut other_surface = sample_surface();
    // Drop one entry → different model digest → different binding set.
    other_surface.entries.retain(|e| e.name != "play");
    let other_model = ScriptApiModel::build(&other_snapshot, &other_surface).unwrap();
    let other_bindings = BindingSet::generate(&other_model).unwrap();

    let err = ActionRecipe::build(
        &definition(),
        &backends(),
        &snapshot(),
        &model(),
        &other_bindings,
        spec(vec![add_note_member()]),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        synvelis_script::ScriptApiError::StaleArtifact { .. }
    ));
}

// ── verify_definition drift ──────────────────────────────────

#[test]
fn verify_definition_accepts_the_building_definition() {
    let recipe = build(spec(vec![add_note_member()])).unwrap();
    recipe.verify_definition(&definition()).unwrap();
}

#[test]
fn verify_definition_rejects_id_version_digest_drift() {
    let recipe = build(spec(vec![add_note_member()])).unwrap();

    let mut def = definition();
    def.script_id = "other.action".to_owned();
    assert!(matches!(
        recipe.verify_definition(&def),
        Err(synvelis_script::ScriptApiError::DefinitionMismatch { field: "script_id" })
    ));

    let mut def = definition();
    def.version = 4;
    assert!(matches!(
        recipe.verify_definition(&def),
        Err(synvelis_script::ScriptApiError::VersionDrift { .. })
    ));

    // Any field change moves the definition digest — including the
    // source digest itself (source drift at the definition level).
    let mut def = definition();
    def.source_digest = sha256_hex(b"different source");
    assert!(matches!(
        recipe.verify_definition(&def),
        Err(synvelis_script::ScriptApiError::DefinitionMismatch {
            field: "definition_digest"
        })
    ));
}
