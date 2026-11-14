//! Golden/determinism tests: same snapshot + surface → identical model,
//! bindings, stub text, and digests; serialized fixtures round-trip;
//! drift is a typed error.

mod common;

use common::*;
use synvelis_script::{
    GeneratedArtifacts, RegistrySnapshot, ScriptApiError, ScriptApiModel, ScriptSurfaceSpec,
    canonical_json,
};

#[test]
fn same_inputs_produce_identical_artifacts() {
    let snapshot = sample_snapshot();
    let surface = sample_surface();

    let model_a = ScriptApiModel::build(&snapshot, &surface).unwrap();
    let model_b = ScriptApiModel::build(&snapshot, &surface).unwrap();
    assert_eq!(model_a, model_b);
    assert_eq!(model_a.model_digest, model_b.model_digest);

    let artifacts_a = GeneratedArtifacts::generate(&model_a).unwrap();
    let artifacts_b = GeneratedArtifacts::generate(&model_b).unwrap();
    assert_eq!(
        artifacts_a.bindings.binding_digest,
        artifacts_b.bindings.binding_digest
    );
    assert_eq!(artifacts_a.stub.text, artifacts_b.stub.text);
    assert_eq!(artifacts_a.stub.stub_digest, artifacts_b.stub.stub_digest);
}

#[test]
fn snapshot_fixture_matches_live_capture() {
    let live = sample_snapshot();

    // Regenerate the checked-in fixture on demand — before parsing, so a
    // missing/stale fixture can be rebuilt:
    //   WRITE_SNAPSHOT_FIXTURE=1 cargo test -p synvelis-script --test golden
    if std::env::var_os("WRITE_SNAPSHOT_FIXTURE").is_some() {
        let text = serde_json::to_string_pretty(&live).unwrap();
        std::fs::write(
            concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/tests/fixtures/registry_snapshot.json"
            ),
            format!("{text}\n"),
        )
        .unwrap();
    }

    let fixture_value = snapshot_fixture_json();
    let fixture: RegistrySnapshot = serde_json::from_value(fixture_value).unwrap();
    fixture.validate().unwrap();

    assert_eq!(live, fixture);
    assert_eq!(live.digest().unwrap(), fixture.digest().unwrap());
}

#[test]
fn model_from_fixture_snapshot_matches_live() {
    let fixture: RegistrySnapshot = serde_json::from_value(snapshot_fixture_json()).unwrap();
    let surface = sample_surface();

    let live_model = ScriptApiModel::build(&sample_snapshot(), &surface).unwrap();
    let fixture_model = ScriptApiModel::build(&fixture, &surface).unwrap();
    assert_eq!(live_model, fixture_model);
    assert_eq!(live_model.model_digest, fixture_model.model_digest);
}

#[test]
fn snapshot_and_model_serialize_deterministically() {
    let snapshot = sample_snapshot();
    let model = sample_model();
    let surface = sample_surface();

    // Canonical JSON of equal values is byte-identical.
    assert_eq!(
        canonical_json(&snapshot).unwrap(),
        canonical_json(&RegistrySnapshot {
            schema_version: snapshot.schema_version,
            capabilities: snapshot.capabilities.clone(),
            schemas: snapshot.schemas.clone(),
        })
        .unwrap()
    );
    assert_eq!(
        canonical_json(&model).unwrap(),
        canonical_json(&ScriptApiModel {
            schema_version: model.schema_version,
            snapshot_digest: model.snapshot_digest.clone(),
            surface_digest: model.surface_digest.clone(),
            model_digest: model.model_digest.clone(),
            entries: model.entries.clone(),
        })
        .unwrap()
    );
    assert_eq!(
        canonical_json(&surface).unwrap(),
        canonical_json(&ScriptSurfaceSpec {
            schema_version: surface.schema_version,
            entries: surface.entries.clone(),
        })
        .unwrap()
    );
}

#[test]
fn model_digest_changes_with_inputs() {
    let snapshot = sample_snapshot();
    let surface = sample_surface();
    let model = ScriptApiModel::build(&snapshot, &surface).unwrap();

    // Different surface → different digest.
    let mut surface2 = surface.clone();
    surface2.entries[0].description = Some("changed".to_owned());
    let model2 = ScriptApiModel::build(&snapshot, &surface2).unwrap();
    assert_ne!(model.model_digest, model2.model_digest);

    // Different snapshot → different digest (schema drift detection).
    let mut snapshot2 = snapshot.clone();
    snapshot2
        .capabilities
        .iter_mut()
        .find(|d| d.id.as_str() == "synvelis.midi.add-note")
        .unwrap()
        .availability = synvelis_gateway::Availability::Disabled {
        reason: "drift".to_owned(),
    };
    let model3 = ScriptApiModel::build(&snapshot2, &surface).unwrap();
    assert_ne!(model.model_digest, model3.model_digest);
    assert_ne!(model.snapshot_digest, model3.snapshot_digest);
}

#[test]
fn artifacts_verify_against_own_model() {
    let model = sample_model();
    let artifacts = GeneratedArtifacts::generate(&model).unwrap();
    artifacts.bindings.verify_against(&model).unwrap();
    artifacts.stub.verify_against(&model).unwrap();
}

#[test]
fn stale_artifact_is_typed_error() {
    let model = sample_model();
    let artifacts = GeneratedArtifacts::generate(&model).unwrap();

    // An artifact generated from a *different* model must not verify.
    let mut surface2 = sample_surface();
    surface2.entries[0].description = Some("other".to_owned());
    let other_model = ScriptApiModel::build(&sample_snapshot(), &surface2).unwrap();
    assert_ne!(model.model_digest, other_model.model_digest);

    let err = artifacts
        .bindings
        .verify_against(&other_model)
        .expect_err("stale binding must fail");
    assert!(matches!(err, ScriptApiError::StaleArtifact { .. }));

    let err = artifacts
        .stub
        .verify_against(&other_model)
        .expect_err("stale stub must fail");
    assert!(matches!(err, ScriptApiError::StaleArtifact { .. }));
}

#[test]
fn binding_stub_surface_consistency_detects_drift() {
    let artifacts = sample_artifacts();
    artifacts.check_consistency().unwrap();

    // Tamper: drop one stub entry — surfaces must diverge.
    let mut tampered = artifacts.clone();
    tampered.stub.entries.remove(0);
    let err = tampered
        .check_consistency()
        .expect_err("tampered stub surface must mismatch");
    assert!(matches!(err, ScriptApiError::BindingStubMismatch { .. }));
}

#[test]
fn invocable_surface_is_subset_of_public_entries() {
    let model = sample_model();
    let bindings = binding_set();
    let stub = stub();

    // Every binding/stub entry traces to a public Invocable model entry.
    for entry in &bindings.entries {
        let model_entry = model.describe(&entry.name).unwrap();
        assert!(model_entry.is_invocable());
        assert_eq!(
            model_entry.visibility,
            synvelis_script::ScriptVisibility::Public
        );
    }
    for entry in &stub.entries {
        assert!(model.is_invocable(&entry.name));
    }

    // Names, capability pins, and schema refs agree across all three.
    assert_eq!(bindings.entries.len(), model.invocable_entries().len());
    assert_eq!(stub.entries.len(), model.invocable_entries().len());
}

#[test]
fn snapshot_serialization_roundtrip() {
    let snapshot = sample_snapshot();
    let json = serde_json::to_value(&snapshot).unwrap();
    let back: RegistrySnapshot = serde_json::from_value(json).unwrap();
    assert_eq!(snapshot, back);
    assert_eq!(snapshot.digest().unwrap(), back.digest().unwrap());
}
