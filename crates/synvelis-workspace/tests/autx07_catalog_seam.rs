//! AUTX-07 SV266-NR-05 (synvelis#266) — `AutomationCatalog` seam tests.
//!
//! The catalog is the neutral boundary between the authority-pushed
//! document and the GUI projections/request lowering: `synvelis-app`
//! cannot name `synvelis-gateway` types, so the seam parses the pushed
//! JSON strictly, re-derives display verdicts from authority fields,
//! and lowers actions to wire request documents the Gateway re-checks.
//! These tests pin that contract at the lowest meaningful layer.

use serde_json::{Value, json};

use synvelis_gateway::CapabilityRef;
use synvelis_gateway::fixture::fixture_descriptors;
use synvelis_workspace::{
    AutomationCatalog, AutomationUiError, AutomationViewAction, ExplicitTarget, GrantState,
};

/// The five fixture descriptors as the pushed `capabilities` entries —
/// every grant `satisfied` so projections expose executable rows.
fn catalog_doc() -> Value {
    json!({
        "seq": 9,
        "capabilities": fixture_descriptors()
            .iter()
            .map(|d| json!({ "descriptor": d, "grant": { "state": "satisfied" } }))
            .collect::<Vec<_>>(),
        "macros": [],
    })
}

/// A two-step preset over `synvelis.transport.set-position@1.0` — the
/// no-target fixture capability, so steps carry `target: null`.
fn preset_doc(grant: &Value, executable: bool, blocked: &Value) -> Value {
    let capability = CapabilityRef {
        id: fixture_descriptors()[3].id.clone(),
        version: fixture_descriptors()[3].version,
    };
    json!({
        "id": "preset.transport-home",
        "name": "Transport Home",
        "version": "1.0",
        "inputs": [],
        "steps": [
            { "capability": capability, "args": { "position": 0 }, "target": null },
            { "capability": capability, "args": { "position": 960 }, "target": null },
        ],
        "effects": [],
        "undo_scope": "none",
        "atomic": true,
        "availability": { "state": "available" },
        "grant": grant,
        "executable": executable,
        "blocked": blocked,
    })
}

fn catalog_with_macros(macros: &[Value]) -> Value {
    let mut doc = catalog_doc();
    doc["macros"] = json!(macros);
    doc
}

// ── strict boundary parse ────────────────────────────────────────────

#[test]
fn from_json_parses_doc_and_rejects_malformed() {
    let catalog = AutomationCatalog::from_json(&catalog_doc()).expect("catalog parses");
    assert_eq!(catalog.seq(), 9);
    assert_eq!(catalog.capability_count(), 5);
    assert_eq!(catalog.preset_count(), 0);

    // Unknown keys reject at both document and entry boundaries.
    let mut unknown_doc = catalog_doc();
    unknown_doc["surprise"] = json!(true);
    assert!(matches!(
        AutomationCatalog::from_json(&unknown_doc),
        Err(AutomationUiError::InvalidRequest { .. })
    ));
    let mut unknown_entry = catalog_doc();
    unknown_entry["capabilities"][0]["extra"] = json!(1);
    assert!(AutomationCatalog::from_json(&unknown_entry).is_err());

    // A malformed descriptor rejects the whole push.
    let mut bad_descriptor = catalog_doc();
    bad_descriptor["capabilities"][0]["descriptor"]["id"] = json!("");
    assert!(AutomationCatalog::from_json(&bad_descriptor).is_err());
}

#[test]
fn missing_grant_is_unknown_and_never_executable() {
    let mut doc = catalog_doc();
    // Strip every grant verdict — absent information must not display
    // as granted.
    for entry in doc["capabilities"].as_array_mut().expect("entries") {
        entry.as_object_mut().expect("entry").remove("grant");
    }
    let catalog = AutomationCatalog::from_json(&doc).expect("catalog parses");
    let view = catalog.action_catalog("");
    assert!(view.rows.iter().all(|row| !row.executable));
    assert!(view.rows.iter().all(|row| row.grant == GrantState::Unknown));
}

// ── projection honesty ───────────────────────────────────────────────

#[test]
fn pushed_executable_flag_is_re_derived_from_authority_fields() {
    // The producer claims the preset is executable with no blockers —
    // but its grant verdict says permissions are missing, so the
    // projection must show it blocked regardless of the pushed flags.
    let doc = catalog_with_macros(&[preset_doc(
        &json!({ "state": "missing", "permissions": ["mutate-project"] }),
        true,
        &Value::Null,
    )]);
    let catalog = AutomationCatalog::from_json(&doc).expect("catalog parses");
    let browser = catalog.macro_browser();
    let preset = &browser.presets[0];
    assert!(!preset.executable);
    assert!(preset.blocked.is_some());
}

#[test]
fn action_catalog_honors_query_and_grant_verdicts() {
    let catalog = AutomationCatalog::from_json(&catalog_doc()).expect("catalog parses");
    let all = catalog.action_catalog("");
    assert_eq!(all.rows.len(), 5);
    assert!(all.rows.iter().all(|row| row.executable));

    let filtered = catalog.action_catalog("midi");
    assert_eq!(filtered.rows.len(), 2);
    assert!(filtered.rows.iter().all(|row| row.id.contains("midi")));
}

// ── request lowering ─────────────────────────────────────────────────

#[test]
fn lower_submit_produces_wire_request_document() {
    let catalog = AutomationCatalog::from_json(&catalog_doc()).expect("catalog parses");
    let requests = catalog
        .lower(
            &AutomationViewAction::Submit {
                capability: "synvelis.transport.set-position@1.0".to_string(),
                target: None,
                args: json!({ "position": 42 }),
                idempotency_key: Some("op-1".to_string()),
            },
            "project/test",
            "autx-1",
            &json!({ "kind": "expected-revision", "revision": 7 }),
        )
        .expect("submit lowers");
    assert_eq!(requests.len(), 1);
    let request = &requests[0];
    assert_eq!(request["request_id"], json!("autx-1"));
    assert_eq!(
        request["capability"]["id"],
        json!("synvelis.transport.set-position")
    );
    assert_eq!(request["project"], json!("project/test"));
    assert_eq!(request["args"], json!({ "position": 42 }));
    assert_eq!(request["consistency"]["kind"], json!("expected-revision"));
    assert_eq!(request["consistency"]["revision"], json!(7));
    assert_eq!(request["idempotency_key"], json!("op-1"));
}

#[test]
fn lower_rejects_unknown_capability_and_target_mismatch() {
    let catalog = AutomationCatalog::from_json(&catalog_doc()).expect("catalog parses");
    // Unknown label.
    assert!(matches!(
        catalog.lower(
            &AutomationViewAction::Preview {
                capability: "synvelis.nope.missing@1.0".to_string(),
                target: None,
                args: json!({}),
            },
            "project/test",
            "autx-1",
            &json!({ "kind": "latest" }),
        ),
        Err(AutomationUiError::UnknownCapability { .. })
    ));
    // `set-position` declares TargetContract::None — an explicit target
    // is a mismatch, never silently dropped.
    assert!(matches!(
        catalog.lower(
            &AutomationViewAction::Submit {
                capability: "synvelis.transport.set-position@1.0".to_string(),
                target: Some(ExplicitTarget::Resource {
                    kind: "source".to_string(),
                    id: "src-1".to_string(),
                }),
                args: json!({}),
                idempotency_key: None,
            },
            "project/test",
            "autx-1",
            &json!({ "kind": "latest" }),
        ),
        Err(AutomationUiError::TargetMismatch { .. } | AutomationUiError::InvalidRequest { .. })
    ));
}

#[test]
fn lower_macro_apply_preserves_per_step_ids_and_idempotency() {
    let doc = catalog_with_macros(&[preset_doc(
        &json!({ "state": "satisfied" }),
        true,
        &Value::Null,
    )]);
    let catalog = AutomationCatalog::from_json(&doc).expect("catalog parses");
    let requests = catalog
        .lower(
            &AutomationViewAction::MacroApply {
                preset: "preset.transport-home".to_string(),
                idempotency_key: Some("apply-1".to_string()),
            },
            "project/test",
            "autx-7",
            &json!({ "kind": "expected-revision", "revision": 3 }),
        )
        .expect("macro apply lowers");
    assert_eq!(requests.len(), 2);
    for (index, request) in requests.iter().enumerate() {
        assert_eq!(request["request_id"], json!(format!("autx-7/{index}")));
        assert_eq!(
            request["idempotency_key"],
            json!(format!("apply-1/{index}"))
        );
        assert_eq!(
            request["capability"]["id"],
            json!("synvelis.transport.set-position")
        );
    }
}

#[test]
fn lower_macro_apply_rejects_unexecutable_preset() {
    // Availability `planned` — re-derived verdict refuses lowering
    // even though the pushed `executable` flag claimed true.
    let mut doc = catalog_with_macros(&[preset_doc(
        &json!({ "state": "satisfied" }),
        true,
        &Value::Null,
    )]);
    doc["macros"][0]["availability"] = json!({ "state": "planned" });
    let catalog = AutomationCatalog::from_json(&doc).expect("catalog parses");
    assert!(matches!(
        catalog.lower(
            &AutomationViewAction::MacroApply {
                preset: "preset.transport-home".to_string(),
                idempotency_key: None,
            },
            "project/test",
            "autx-1",
            &json!({ "kind": "latest" }),
        ),
        Err(AutomationUiError::NotExecutable { .. })
    ));
}

#[test]
fn capture_freezes_validated_target() {
    let catalog = AutomationCatalog::from_json(&catalog_doc()).expect("catalog parses");
    // No-target capability accepts `None` and rejects an explicit one.
    let captured = catalog
        .capture("synvelis.transport.set-position@1.0", None, "project/test")
        .expect("no-target capture");
    assert_eq!(captured.capability_id, "synvelis.transport.set-position");
    assert!(captured.target.is_none());
    assert!(
        catalog
            .capture(
                "synvelis.transport.set-position@1.0",
                Some(ExplicitTarget::Resource {
                    kind: "source".to_string(),
                    id: "src-1".to_string(),
                }),
                "project/test",
            )
            .is_err()
    );
    // Resource-contract capability requires the matching explicit target.
    assert!(
        catalog
            .capture("synvelis.midi.add-note@1.1", None, "project/test")
            .is_err()
    );
    let captured = catalog
        .capture(
            "synvelis.midi.add-note@1.1",
            Some(ExplicitTarget::Resource {
                kind: "source".to_string(),
                id: "src-1".to_string(),
            }),
            "project/test",
        )
        .expect("resource capture");
    assert!(captured.target.is_some());
}

#[test]
fn preset_at_resolves_display_row_to_stable_id() {
    let doc = catalog_with_macros(&[
        preset_doc(&json!({ "state": "satisfied" }), true, &Value::Null),
        preset_doc(&json!({ "state": "satisfied" }), true, &Value::Null),
    ]);
    // Distinct ids so row→id resolution is observable.
    let mut doc = doc;
    doc["macros"][1]["id"] = json!("preset.transport-home-2");
    let catalog = AutomationCatalog::from_json(&doc).expect("catalog parses");
    assert_eq!(
        catalog.preset_at(0).expect("row 0").id,
        "preset.transport-home"
    );
    assert_eq!(
        catalog.preset_at(1).expect("row 1").id,
        "preset.transport-home-2"
    );
    assert!(catalog.preset_at(2).is_none());
}
