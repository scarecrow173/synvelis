//! Model verdict/visibility tests: every fail-closed classification is
//! typed, hidden stays out of the public surface, and no authority-shaped
//! data is projected.

mod common;

use common::*;
use synvelis_script::{
    NameRejection, ScriptApiError, ScriptApiModel, ScriptVerdict, ScriptVisibility,
};

fn verdict_of<'a>(model: &'a ScriptApiModel, name: &str) -> &'a ScriptVerdict {
    &model.describe(name).unwrap().verdict
}

#[test]
fn invocable_entries_get_invocable_verdict() {
    let model = sample_model();
    for name in [
        "addNote",
        "play",
        "trackInfo",
        "splitClip",
        "moveClips",
        "createClip",
    ] {
        assert_eq!(
            verdict_of(&model, name),
            &ScriptVerdict::Invocable,
            "{name}"
        );
        assert!(model.is_invocable(name), "{name}");
    }
}

#[test]
fn availability_maps_to_typed_verdicts() {
    let model = sample_model();
    assert!(matches!(
        verdict_of(&model, "legacyOp"),
        ScriptVerdict::Disabled { .. }
    ));
    assert_eq!(verdict_of(&model, "futureOp"), &ScriptVerdict::Planned);
    assert_eq!(verdict_of(&model, "oldOp"), &ScriptVerdict::Retired);

    // Disabled reason is preserved as safe-to-disclose metadata.
    match verdict_of(&model, "legacyOp") {
        ScriptVerdict::Disabled { reason } => {
            assert!(reason.contains("superseded"));
        }
        other => panic!("expected Disabled, got {other:?}"),
    }
}

#[test]
fn unknown_and_unsupported_version_are_typed() {
    let model = sample_model();
    assert_eq!(
        verdict_of(&model, "newThing"),
        &ScriptVerdict::UnknownCapability
    );
    match verdict_of(&model, "wrongVer") {
        ScriptVerdict::UnsupportedVersion { supported } => {
            assert_eq!(supported.len(), 1);
            assert_eq!(supported[0], cver(1, 0));
        }
        other => panic!("expected UnsupportedVersion, got {other:?}"),
    }
}

#[test]
fn unusable_names_are_typed_not_fatal() {
    let model = sample_model();
    match verdict_of(&model, "bad name!") {
        ScriptVerdict::InvalidName { reason } => {
            assert_eq!(*reason, NameRejection::InvalidIdentifier);
        }
        other => panic!("expected InvalidName, got {other:?}"),
    }
    match verdict_of(&model, "end") {
        ScriptVerdict::InvalidName { reason } => {
            assert_eq!(*reason, NameRejection::ReservedWord);
        }
        other => panic!("expected InvalidName, got {other:?}"),
    }
    // Invalid-name entries are still recorded for audit but never invocable.
    assert!(!model.is_invocable("bad name!"));
    assert!(!model.is_invocable("end"));
}

#[test]
fn hidden_is_visibility_not_verdict() {
    let model = sample_model();
    let hidden = model.describe("hiddenOp").unwrap();
    // Capability-wise it IS invocable (same capability as addNote)…
    assert_eq!(hidden.verdict, ScriptVerdict::Invocable);
    // …but visibility keeps it off every public surface.
    assert_eq!(hidden.visibility, ScriptVisibility::Hidden);
    assert!(!hidden.is_invocable());
    assert!(!model.is_invocable("hiddenOp"));

    // list_public excludes it; entries() retains it for audit.
    assert!(model.list_public().iter().all(|e| e.name != "hiddenOp"));
    assert!(model.entries().iter().any(|e| e.name == "hiddenOp"));

    // invocable_entries = the 6 public invocable names only.
    let names: Vec<&str> = model
        .invocable_entries()
        .iter()
        .map(|e| e.name.as_str())
        .collect();
    assert_eq!(
        names,
        vec![
            "addNote",
            "createClip",
            "moveClips",
            "play",
            "splitClip",
            "trackInfo"
        ]
    );
}

#[test]
fn list_public_shows_status_metadata() {
    let model = sample_model();
    let public = model.list_public();
    // Public listing includes non-invocable entries — display status is
    // metadata, not permission.
    let names: Vec<&str> = public.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"legacyOp"));
    assert!(names.contains(&"futureOp"));
    assert!(names.contains(&"oldOp"));
    assert!(names.contains(&"newThing"));
    assert!(names.contains(&"wrongVer"));
    assert!(!names.contains(&"hiddenOp"));
}

#[test]
fn duplicate_script_name_is_fatal() {
    let snapshot = sample_snapshot();
    let mut surface = sample_surface();
    let dup = surface.entries[0].clone();
    surface.entries.push(dup);
    let err =
        ScriptApiModel::build(&snapshot, &surface).expect_err("duplicate name must fail closed");
    assert!(matches!(err, ScriptApiError::DuplicateApiName { .. }));
}

#[test]
fn unresolved_schema_is_typed_verdict() {
    // Hand-built snapshot: capability declares a schema the snapshot
    // does not carry — model must mark UnresolvedSchema, not guess.
    let mut snapshot = sample_snapshot();
    snapshot.schemas.retain(|e| e.name != "midi.add-note.input");
    let model = ScriptApiModel::build(&snapshot, &sample_surface()).unwrap();
    match verdict_of(&model, "addNote") {
        ScriptVerdict::UnresolvedSchema { schema } => {
            assert_eq!(schema, "midi.add-note.input@1.0");
        }
        other => panic!("expected UnresolvedSchema, got {other:?}"),
    }
    assert!(!model.is_invocable("addNote"));
}

#[test]
fn unresolved_schema_only_blocks_available_entries() {
    // A Disabled capability with an unresolvable schema still reports
    // Disabled — availability is the headline verdict.
    let mut snapshot = sample_snapshot();
    snapshot
        .schemas
        .retain(|e| e.name != "query.track-info.input");
    let model = ScriptApiModel::build(&snapshot, &sample_surface()).unwrap();
    assert!(matches!(
        verdict_of(&model, "legacyOp"),
        ScriptVerdict::Disabled { .. }
    ));
    assert!(matches!(
        verdict_of(&model, "trackInfo"),
        ScriptVerdict::UnresolvedSchema { .. }
    ));
}

#[test]
fn snapshot_validation_rejects_malformed() {
    // Unsupported document version.
    let mut bad = sample_snapshot();
    bad.schema_version = 99;
    assert!(matches!(
        bad.validate(),
        Err(ScriptApiError::Snapshot { .. })
    ));

    // Duplicate capability (id, version).
    let mut dup = sample_snapshot();
    let cap = dup.capabilities[0].clone();
    dup.capabilities.push(cap);
    assert!(matches!(
        dup.validate(),
        Err(ScriptApiError::DuplicateCapability { .. })
    ));

    // Duplicate schema (name, version).
    let mut dup_schema = sample_snapshot();
    let schema = dup_schema.schemas[0].clone();
    dup_schema.schemas.push(schema);
    assert!(matches!(
        dup_schema.validate(),
        Err(ScriptApiError::DuplicateSchema { .. })
    ));
}

#[test]
fn surface_validation_rejects_bad_version() {
    let mut surface = sample_surface();
    surface.schema_version = 42;
    let err = ScriptApiModel::build(&sample_snapshot(), &surface)
        .expect_err("bad surface version must fail");
    assert!(matches!(err, ScriptApiError::Snapshot { .. }));
}

#[test]
fn entries_sorted_by_name() {
    let model = sample_model();
    let names: Vec<&str> = model.entries.iter().map(|e| e.name.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort_unstable();
    assert_eq!(names, sorted);
}

#[test]
fn provenance_digests_present_on_resolved_entries() {
    let model = sample_model();
    let entry = model.describe("addNote").unwrap();
    let provenance = entry.provenance.as_ref().expect("provenance");
    assert_eq!(provenance.snapshot_digest.len(), 64);
    assert_eq!(provenance.capability_digest.len(), 64);
    assert_eq!(provenance.snapshot_digest, model.snapshot_digest);

    // Unresolved entries carry no provenance.
    let unknown = model.describe("newThing").unwrap();
    assert!(unknown.provenance.is_none());
    assert!(unknown.descriptor.is_none());
}

#[test]
fn model_carries_no_authority_types() {
    // Structural guarantee: the public API surface of this crate exposes
    // no CallerIdentity/AdapterSeal/grant/approval/permission types.
    // (Compile-time: these types are simply not re-exported; the fixture
    // below asserts generated artifacts contain no authority-shaped
    // identifiers. Prose disclaimers in the stub header may mention
    // "permission"/"approval" — what must never appear are the authority
    // *types* and *field names* themselves.)
    let artifacts = sample_artifacts();
    let stub_text = &artifacts.stub.text;
    for forbidden in [
        "CallerIdentity",
        "AdapterSeal",
        "principal",
        "grant_token",
        "GrantReceipt",
        "ApprovalRequest",
        "approval_receipt",
        "MutateProject",
        "ReadContext",
        "ControlTransport",
        "RunJob",
        "PermissionSet",
        "PermissionKind",
        "EffectSet",
        "EffectKind",
        "effect_bound",
        "data_access",
        "required_permissions",
        "OperationRequest",
    ] {
        assert!(
            !stub_text.contains(forbidden),
            "stub leaks authority/metadata term {forbidden:?}"
        );
    }
}

#[test]
fn disabled_reason_stays_out_of_stub() {
    let stub = stub();
    assert!(
        !stub
            .text
            .contains("superseded by synvelis.query.track-info")
    );
    assert!(!stub.text.contains("legacyOp"));
    assert!(!stub.text.contains("hiddenOp"));
}

#[test]
fn entry_records_exact_requested_ref() {
    let model = sample_model();
    let wrong = model.describe("wrongVer").unwrap();
    assert_eq!(wrong.requested.version, cver(9, 9));
    assert!(wrong.descriptor.is_none());

    let ok = model.describe("addNote").unwrap();
    assert_eq!(ok.requested.id.as_str(), "synvelis.midi.add-note");
    assert_eq!(ok.requested.version, cver(1, 0));
    let descriptor = ok.descriptor.as_ref().unwrap();
    assert_eq!(descriptor.capability_ref(), ok.requested);
}

#[test]
fn model_serializes_roundtrip() {
    // The model is serializable end-to-end (canonical form is stable).
    let model = sample_model();
    let json = serde_json::to_value(&model).unwrap();
    let back: synvelis_script::ScriptApiModel = serde_json::from_value(json).unwrap();
    assert_eq!(model, back);
}
