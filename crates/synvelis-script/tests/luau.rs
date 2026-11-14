//! Luau stub generation tests: total type projection, helper aliases,
//! per-entry declarations, disclosure boundaries, and digest pinning.

mod common;

use common::*;
use synvelis_script::{
    LuauStub, ScriptApiError, ScriptApiModel, ScriptSurfaceEntry, ScriptSurfaceSpec,
    ScriptVisibility,
};

#[test]
fn stub_is_deterministic() {
    let a = stub();
    let b = stub();
    assert_eq!(a.text, b.text);
    assert_eq!(a.stub_digest, b.stub_digest);
    assert_eq!(a.model_digest, sample_model().model_digest);
    assert_eq!(a.snapshot_digest, sample_snapshot().digest().unwrap());
    assert_eq!(a.surface_digest.len(), 64);
}

#[test]
fn stub_header_carries_provenance_and_disclaimers() {
    let text = &stub().text;
    assert!(text.starts_with("--!strict\n"));
    assert!(text.contains("-- DO NOT EDIT"));
    assert!(text.contains("snapshot-digest:"));
    assert!(text.contains("model-digest:"));
    assert!(text.contains("grant NO authority"));
    assert!(text.contains("re-authorized"));
    assert!(text.contains("deny-unknown"));
}

#[test]
fn helper_types_emitted() {
    let text = &stub().text;
    // Consistency union mirrors the wire vocabulary.
    assert!(text.contains("export type SynvelisConsistency ="));
    assert!(text.contains("{ kind: \"latest\" }"));
    assert!(text.contains("{ kind: \"read-at\", revision: number }"));
    assert!(text.contains("{ kind: \"expected-revision\", revision: number }"));
    assert!(text.contains("{ kind: \"unconditional\" }"));
    // Always-present project id alias.
    assert!(text.contains("export type SynvelisProjectId = string"));
    // i64 alias: used by addNote.sample_hint, moveClips.delta/moved,
    // trackInfo.sample_count/notes.start, and every TimeRange.
    assert!(text.contains("export type SynvelisI64 = string"));
    // Resource-kind id aliases actually used by schemas/targets.
    for alias in ["SynvelisTrackId", "SynvelisClipId", "SynvelisNoteId"] {
        assert!(
            text.contains(&format!("export type {alias} = string")),
            "{alias}"
        );
    }
    // Unused kinds must not appear.
    assert!(!text.contains("SynvelisLaneId"));
    assert!(!text.contains("SynvelisAssetId"));
}

#[test]
fn per_entry_type_aliases_and_functions() {
    let text = &stub().text;
    // addNote → synvelisAddNote; Resource{Track} target → target param.
    assert!(text.contains("export type AddNoteInput = "));
    assert!(text.contains("export type AddNoteOutput = "));
    assert!(text.contains(
        "declare function synvelisAddNote(project: SynvelisProjectId, target: SynvelisTrackId, args: AddNoteInput, consistency: SynvelisConsistency): AddNoteOutput"
    ));
    // play → TargetContract::None → no target param.
    assert!(text.contains(
        "declare function synvelisPlay(project: SynvelisProjectId, args: PlayInput, consistency: SynvelisConsistency): PlayOutput"
    ));
    // play output is Bool → boolean alias.
    assert!(text.contains("export type PlayOutput = boolean"));
    // play input is empty object → `{  }` table.
    assert!(text.contains("export type PlayInput = {  }"));
    // createClip → Create{parent: Track, creates: Clip} → parent param.
    assert!(text.contains("target: { parent: SynvelisTrackId }"));
    // createClip output is Id{Clip} → alias, not object.
    assert!(text.contains("export type CreateClipOutput = SynvelisClipId"));
    // moveClips → ResourceSet{Clip} → array-of-id target.
    assert!(text.contains("target: {SynvelisClipId}"));
    // snake_case names pascal-case correctly (none in fixture, but the
    // mapping is exercised via capability labels).
}

#[test]
fn schema_projection_covers_all_variants() {
    let text = &stub().text;
    // Integer in f64 range → number; out of range → SynvelisI64.
    assert!(text.contains("pitch: number"));
    assert!(text.contains("sample_hint: SynvelisI64?"));
    assert!(text.contains("delta: SynvelisI64"));
    // String → string.
    assert!(text.contains("label: string?"));
    // Bool → boolean.
    assert!(text.contains("inner: boolean"));
    // Enum → string-literal union in declared order.
    assert!(text.contains("\"linear\" | \"exp\" | \"log\""));
    // Array → {T}; compound types get parenthesized `?`.
    assert!(text.contains("tags: ({string})?"));
    assert!(text.contains("clips: {SynvelisClipId}"));
    // Optional(Number) required field → `number | nil` (Null), NOT `?`.
    assert!(text.contains("gain_db: number | nil"));
    // Optional field of Optional(Integer) → may be absent AND nil.
    assert!(text.contains("macro: (number | nil)?"));
    // Non-Optional optional field → `T?` (absent allowed; nil is a
    // harmless static superset, re-validated at invoke).
    assert!(text.contains("velocity: number?"));
    // TimeRange → literal domain + i64 extent.
    assert!(text.contains("{ domain: \"project-musical\", start: SynvelisI64, len: SynvelisI64 }"));
    assert!(text.contains("{ domain: \"media\", start: SynvelisI64, len: SynvelisI64 }"));
    // Id → branded alias.
    assert!(text.contains("owner: SynvelisTrackId?"));
    assert!(text.contains("note_id: SynvelisNoteId"));
}

#[test]
fn non_invocable_entries_produce_no_types() {
    let text = &stub().text;
    for absent in [
        "hiddenOp",
        "legacyOp",
        "futureOp",
        "oldOp",
        "newThing",
        "wrongVer",
        "bad name!",
        "synvelisNope",
        "synvelisEnd",
        "synvelisHiddenOp",
        "synvelisLegacyOp",
        "synvelisFutureOp",
        "synvelisOldOp",
        "synvelisNewThing",
        "synvelisWrongVer",
    ] {
        assert!(!text.contains(absent), "stub leaks {absent:?}");
    }
    // Exactly the 6 invocable functions are declared.
    for present in [
        "synvelisAddNote",
        "synvelisPlay",
        "synvelisTrackInfo",
        "synvelisSplitClip",
        "synvelisMoveClips",
        "synvelisCreateClip",
    ] {
        assert!(
            text.contains(&format!("declare function {present}(")),
            "{present}"
        );
    }
}

#[test]
fn stub_verifies_against_model_and_detects_stale() {
    let model = sample_model();
    let artifacts = sample_artifacts();
    artifacts.stub.verify_against(&model).unwrap();

    let mut surface = sample_surface();
    surface.entries[0].description = Some("drift".to_owned());
    let other = ScriptApiModel::build(&sample_snapshot(), &surface).unwrap();
    let err = artifacts
        .stub
        .verify_against(&other)
        .expect_err("stale stub");
    assert!(matches!(err, ScriptApiError::StaleArtifact { .. }));
}

#[test]
fn stub_entries_match_model_invocable_set() {
    let stub = stub();
    let model = sample_model();
    let mut stub_names: Vec<&str> = stub.entries.iter().map(|e| e.name.as_str()).collect();
    stub_names.sort_unstable();
    let mut model_names: Vec<&str> = model
        .invocable_entries()
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    model_names.sort_unstable();
    assert_eq!(stub_names, model_names);

    // Each stub entry pins exact capability + schema refs.
    for entry in &stub.entries {
        let model_entry = model.describe(&entry.name).unwrap();
        assert_eq!(entry.capability, model_entry.requested);
        assert_eq!(
            entry.input_schema,
            model_entry.input.as_ref().unwrap().schema
        );
        assert_eq!(
            entry.output_schema,
            model_entry.output.as_ref().unwrap().schema
        );
        assert!(entry.function_name.starts_with("synvelis"));
    }
}

#[test]
fn enum_with_no_values_degenerates_safely() {
    // A pathological empty Enum cannot be honestly typed; it must not
    // silently become `string`. Register a capability whose input is
    // `Enum{values:[]}` and assert the projection is `nil` (fails fast
    // at the binding layer — the Gateway validate() stays authoritative).
    let (mut caps, mut schemas) = sample_registries();
    schemas
        .register(
            "empty.enum.input",
            synvelis_gateway::SchemaVersion::new(1, 0),
            synvelis_gateway::ValueSchema::Enum { values: vec![] },
        )
        .unwrap();
    schemas
        .register(
            "empty.enum.output",
            synvelis_gateway::SchemaVersion::new(1, 0),
            synvelis_gateway::ValueSchema::Bool,
        )
        .unwrap();
    caps.register(
        descriptor(
            "synvelis.empty.enum",
            cver(1, 0),
            synvelis_gateway::OperationDomain::Query,
            sref("empty.enum.input", 1, 0),
            sref("empty.enum.output", 1, 0),
            synvelis_gateway::TargetContract::None,
            synvelis_gateway::Availability::Available,
        ),
        None::<std::sync::Arc<dyn synvelis_gateway::CapabilityOperation>>,
    )
    .unwrap();
    let snapshot = synvelis_script::RegistrySnapshot::capture(&caps, &schemas).unwrap();
    let surface = ScriptSurfaceSpec {
        schema_version: synvelis_script::SURFACE_SCHEMA_VERSION,
        entries: vec![ScriptSurfaceEntry {
            name: "emptyEnum".to_owned(),
            capability: synvelis_gateway::CapabilityRef {
                id: vid("synvelis.empty.enum"),
                version: cver(1, 0),
            },
            visibility: ScriptVisibility::Public,
            description: None,
        }],
    };
    let model = ScriptApiModel::build(&snapshot, &surface).unwrap();
    assert!(model.is_invocable("emptyEnum"));
    let stub = LuauStub::generate(&model).unwrap();
    assert!(stub.text.contains("export type EmptyEnumInput = nil"));
}
