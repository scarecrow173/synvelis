//! SV266-NR-02 (synvelis#266) — Action Palette ⇔ Script Editor (#274)
//! contract-level parity over a single `RegistrySnapshot` source.
//!
//! Both surfaces must show the same capability identity, schema,
//! availability, and target truth because they resolve against the
//! same `CapabilityDescriptor` set — the palette through
//! [`resolve_capability`] + [`project_action_row`], the Script Editor
//! through `ScriptApiModel` + [`project_script_api`]. This file pins
//! that contract headlessly:
//!
//! - exact `id@version` refs resolve to the *same* descriptor on both
//!   sides; unresolvable refs (version drift, unregistered id) fail
//!   closed on both sides;
//! - script verdict labels agree with `Availability::name()` and the
//!   palette's fail-closed `executable` matrix is re-verified on this
//!   fixture (SV266-AC-01 contract);
//! - schema bindings and target/domain/effect projections are
//!   descriptor-verbatim;
//! - `ScriptAnalyzerStatus` transitions never change API verdicts or
//!   executability — analyzer PASS is not Gateway authorization PASS.
//!
//! `crates/synvelis-workspace/src/**` is untouched (sv#112 lease): if
//! this parity check exposes a contract bug, it is recorded as a
//! failure, never fixed here.

use synvelis_gateway::{Availability, CapabilityId, CapabilityRef, CapabilityVersion};
use synvelis_script::{
    AnalyzeResult, AnalyzerDescriptor, AnalyzerError, DiagnosticLevel, RegistrySnapshot,
    SURFACE_SCHEMA_VERSION, ScriptApiModel, ScriptLanguageId, ScriptSurfaceEntry,
    ScriptSurfaceSpec, ScriptVisibility, SourceRevision, WantedCapabilities,
};
use synvelis_workspace::{
    ActionRow, AutomationUiError, GrantState, ScriptEditorDocument, project_action_row,
    project_script_api, resolve_capability,
};

// ── fixtures ──────────────────────────────────────────────────────────

fn vid(text: &str) -> CapabilityId {
    CapabilityId::new(text).expect("fixture capability id")
}

fn entry(
    name: &str,
    id: &str,
    version: (u32, u32),
    visibility: ScriptVisibility,
) -> ScriptSurfaceEntry {
    ScriptSurfaceEntry {
        name: name.to_owned(),
        capability: CapabilityRef {
            id: vid(id),
            version: CapabilityVersion::new(version.0, version.1),
        },
        visibility,
        description: None,
    }
}

/// The script surface: one entry per availability state plus the
/// fail-closed cases (version drift, unregistered id) and a hidden
/// entry that must never reach the public rows.
fn surface() -> ScriptSurfaceSpec {
    ScriptSurfaceSpec {
        schema_version: SURFACE_SCHEMA_VERSION,
        entries: vec![
            entry(
                "addNote",
                "synvelis.midi.add-note",
                (1, 0),
                ScriptVisibility::Public,
            ),
            entry(
                "play",
                "synvelis.transport.play",
                (1, 0),
                ScriptVisibility::Public,
            ),
            entry(
                "trackInfo",
                "synvelis.query.track-info",
                (1, 0),
                ScriptVisibility::Public,
            ),
            entry(
                "clipCreate",
                "synvelis.clip.create",
                (1, 0),
                ScriptVisibility::Public,
            ),
            entry(
                "clipSplit",
                "synvelis.clip.split",
                (1, 0),
                ScriptVisibility::Public,
            ),
            entry(
                "batchMove",
                "synvelis.batch.move",
                (1, 0),
                ScriptVisibility::Public,
            ),
            entry(
                "futureOp",
                "synvelis.future.thing",
                (1, 0),
                ScriptVisibility::Public,
            ),
            entry(
                "legacyOp",
                "synvelis.legacy.thing",
                (1, 0),
                ScriptVisibility::Public,
            ),
            entry(
                "oldOp",
                "synvelis.old.thing",
                (1, 0),
                ScriptVisibility::Public,
            ),
            // Requested `@9.9` — id registered, version is not.
            entry(
                "wrongVer",
                "synvelis.midi.add-note",
                (9, 9),
                ScriptVisibility::Public,
            ),
            // Id not registered at all.
            entry(
                "ghostOp",
                "synvelis.query.nonexistent",
                (1, 0),
                ScriptVisibility::Public,
            ),
            // Hidden: audit-visible in the model, never projected.
            entry(
                "hiddenOp",
                "synvelis.midi.add-note",
                (1, 0),
                ScriptVisibility::Hidden,
            ),
        ],
    }
}

fn snapshot() -> RegistrySnapshot {
    serde_json::from_str(include_str!(
        "../../synvelis-script/tests/fixtures/registry_snapshot.json"
    ))
    .expect("snapshot fixture parses")
}

fn model() -> ScriptApiModel {
    ScriptApiModel::build(&snapshot(), &surface()).expect("fixture model builds")
}

fn analyzer_descriptor() -> AnalyzerDescriptor {
    AnalyzerDescriptor {
        analyzer_id: "synvelis-luau-port".to_owned(),
        language: ScriptLanguageId::luau(),
        version: synvelis_script::ScriptBackendVersion {
            impl_version: "0.1.0".to_owned(),
            contract_version: 1,
        },
        capabilities: synvelis_script::AnalyzerCapabilities {
            diagnostics: DiagnosticLevel::SyntaxAndType,
            completion: true,
            max_source_bytes: 0,
            supported_languages: vec![ScriptLanguageId::luau()],
        },
    }
}

fn wanted() -> WantedCapabilities {
    WantedCapabilities {
        diagnostics: Some(DiagnosticLevel::SyntaxAndType),
        completion: true,
    }
}

// ── capability identity parity ────────────────────────────────────────

#[test]
fn requested_refs_resolve_against_the_same_descriptor_source() {
    let snap = snapshot();
    let model = model();
    let rows = project_script_api(&model);

    // The public surface covers every fixture entry except the hidden
    // ones — no silent drops, no extras.
    let spec = surface();
    let hidden = spec
        .entries
        .iter()
        .filter(|e| e.visibility == ScriptVisibility::Hidden)
        .count();
    assert_eq!(rows.len(), spec.entries.len() - hidden);
    assert!(!rows.iter().any(|r| r.name == "hiddenOp"));

    for row in &rows {
        let entry = model
            .describe(&row.name)
            .expect("model entry for public row");
        // The palette-side resolver and the model-side exact-ref
        // resolution must agree on *whether* a descriptor exists —
        // `entry.descriptor.is_some()` ⟺ `resolve_capability` Ok.
        let palette = resolve_capability(&snap.capabilities, &row.capability);
        assert_eq!(
            entry.descriptor.is_some(),
            palette.is_ok(),
            "{}@{}: descriptor presence diverges",
            row.name,
            row.capability
        );
        if let Ok(descriptor) = palette {
            // Same descriptor object on both sides.
            assert_eq!(
                entry.descriptor.as_ref(),
                Some(descriptor),
                "{}: model and palette resolved different descriptors",
                row.name
            );
            // The palette row's stable identity is the requested ref.
            let action = project_action_row(descriptor, GrantState::Satisfied);
            assert_eq!(action.capability_label(), row.capability);
            assert_eq!(action.capability_label(), entry.requested.to_string());
        } else {
            // Fail-closed verdicts only — never an invocable-looking row.
            assert!(
                matches!(
                    row.verdict.as_str(),
                    "unsupported-version"
                        | "unknown-capability"
                        | "unresolved-schema"
                        | "invalid-name"
                ),
                "{}: unexpected verdict {}",
                row.name,
                row.verdict
            );
        }
    }
}

#[test]
fn version_drift_and_unregistered_ids_fail_closed_on_both_sides() {
    let snap = snapshot();
    let rows = project_script_api(&model());

    // `wrongVer` requested `synvelis.midi.add-note@9.9`: the id is
    // registered at 1.0 only — script says unsupported-version with
    // the supported list, palette says UnknownCapability.
    let wrong = rows
        .iter()
        .find(|r| r.name == "wrongVer")
        .expect("wrongVer row");
    assert_eq!(wrong.capability, "synvelis.midi.add-note@9.9");
    assert_eq!(wrong.verdict, "unsupported-version");
    assert_eq!(
        wrong.detail.as_deref(),
        Some("1.0"),
        "supported versions are disclosed, not guessed"
    );
    match resolve_capability(&snap.capabilities, &wrong.capability) {
        Err(AutomationUiError::UnknownCapability { capability }) => {
            assert_eq!(capability, wrong.capability);
        }
        other => panic!("expected UnknownCapability, got {other:?}"),
    }

    // `ghostOp` requested an id absent from the snapshot entirely.
    let ghost = rows
        .iter()
        .find(|r| r.name == "ghostOp")
        .expect("ghostOp row");
    assert_eq!(ghost.verdict, "unknown-capability");
    assert!(matches!(
        resolve_capability(&snap.capabilities, &ghost.capability),
        Err(AutomationUiError::UnknownCapability { .. })
    ));
}

// ── availability parity + executability ───────────────────────────────

#[test]
fn verdict_labels_match_descriptor_availability() {
    let model = model();
    for row in project_script_api(&model) {
        let entry = model.describe(&row.name).expect("model entry");
        let Some(descriptor) = &entry.descriptor else {
            continue;
        };
        let expected = match row.verdict.as_str() {
            "invocable" => Some("available"),
            "disabled" => Some("disabled"),
            "planned" => Some("planned"),
            "retired" => Some("retired"),
            other => panic!("{}: descriptor resolved but verdict is {other}", row.name),
        };
        assert_eq!(
            descriptor.availability.name(),
            expected.expect("mapped"),
            "{}: script verdict {} vs availability {}",
            row.name,
            row.verdict,
            descriptor.availability.name()
        );
        // The palette projection carries the same availability
        // verbatim — no second interpretation.
        let action = project_action_row(descriptor, GrantState::Satisfied);
        assert_eq!(action.availability, descriptor.availability);
        assert_eq!(action.availability.name(), expected.expect("mapped"));
    }
}

#[test]
fn executable_is_fail_closed_for_every_availability_grant_pair() {
    // SV266-AC-01 contract re-verified on the snapshot fixture:
    // `executable` is true only for `Available` ∧ `Satisfied`, and
    // `blocked` carries the reason otherwise.
    let snap = snapshot();
    let grants = [
        GrantState::Satisfied,
        GrantState::Missing {
            permissions: vec!["mutate-project".to_owned()],
        },
        GrantState::Unknown,
    ];
    for descriptor in &snap.capabilities {
        for grant in &grants {
            let row = project_action_row(descriptor, grant.clone());
            let expected =
                matches!(descriptor.availability, Availability::Available) && grant.satisfied();
            assert_eq!(
                row.executable,
                expected,
                "{}@{} grant={:?}",
                descriptor.id,
                descriptor.version,
                grant.label()
            );
            assert_eq!(
                row.blocked.is_none(),
                row.executable,
                "{}: blocked/executable must be exact complements",
                descriptor.capability_ref()
            );
        }
    }
    // Spot-check the interesting cells: every non-Available state stays
    // blocked even with a satisfied grant, and Available + unsatisfied
    // grant stays blocked even though the script verdict is `invocable`.
    let rows = project_script_api(&model());
    for name in ["futureOp", "legacyOp", "oldOp"] {
        let row = rows.iter().find(|r| r.name == name).expect("row");
        assert_ne!(row.verdict, "invocable", "{name}");
    }
    let add_note = rows.iter().find(|r| r.name == "addNote").expect("addNote");
    assert_eq!(add_note.verdict, "invocable");
    let descriptor =
        resolve_capability(&snap.capabilities, &add_note.capability).expect("addNote resolves");
    for grant in [
        GrantState::Missing {
            permissions: vec!["mutate-project".to_owned()],
        },
        GrantState::Unknown,
    ] {
        assert!(!project_action_row(descriptor, grant).executable);
    }
}

// ── schema / target parity ────────────────────────────────────────────

#[test]
fn schema_bindings_and_display_labels_match_descriptor_refs() {
    let model = model();
    for entry in model.entries() {
        let Some(descriptor) = &entry.descriptor else {
            continue;
        };
        let action = project_action_row(descriptor, GrantState::Satisfied);

        // Script side: resolved bindings carry the descriptor's
        // declared `SchemaRef`s verbatim.
        let input = entry.input.as_ref().expect("input schema binding");
        let output = entry.output.as_ref().expect("output schema binding");
        assert_eq!(&input.schema, &descriptor.input_schema, "{}", entry.name);
        assert_eq!(&output.schema, &descriptor.output_schema, "{}", entry.name);

        // Palette side: `ActionRow` holds the same refs and its
        // `name@version` label matches the binding's own ref.
        assert_eq!(action.input_schema, descriptor.input_schema);
        assert_eq!(action.output_schema, descriptor.output_schema);
        assert_eq!(
            ActionRow::schema_label(&action.input_schema),
            input.schema.to_string(),
            "{}: input schema label",
            entry.name
        );
        assert_eq!(
            ActionRow::schema_label(&action.output_schema),
            output.schema.to_string(),
            "{}: output schema label",
            entry.name
        );

        // Target/domain/effects are descriptor-verbatim on the palette
        // side — the script entry's descriptor is the same object.
        assert_eq!(action.target, descriptor.target, "{}", entry.name);
        assert_eq!(action.domain, descriptor.domain, "{}", entry.name);
        assert_eq!(
            action.effects,
            descriptor.effect_bound.iter().collect::<Vec<_>>(),
            "{}",
            entry.name
        );
    }
}

// ── analyzer status is not an authority input ─────────────────────────

#[test]
fn analyzer_status_transitions_never_change_verdicts_or_executability() {
    let model = model();
    let api_rows = project_script_api(&model);

    // The palette projection has no analyzer-status input at all —
    // re-projecting yields identical rows regardless of editor state.
    let snap = snapshot();
    let palette_before = project_action_row(
        resolve_capability(&snap.capabilities, "synvelis.midi.add-note@1.0")
            .expect("add-note resolves"),
        GrantState::Missing {
            permissions: vec!["mutate-project".to_owned()],
        },
    );
    assert!(!palette_before.executable);
    assert!(palette_before.blocked.is_some());

    let mut doc = ScriptEditorDocument::new("main", ScriptLanguageId::luau());
    assert_eq!(doc.status().label(), "idle");
    doc.set_api_rows(api_rows.clone());
    assert_eq!(doc.api(), api_rows.as_slice());

    // Drive the document through the whole analyzer status vocabulary.
    doc.edit("synvelisPlay()");
    let req = doc.begin_analysis(wanted());
    let accepted = AnalyzeResult {
        source_revision: req.source.revision,
        diagnostics: vec![],
        completions: None,
    };
    doc.apply_result(&accepted, &analyzer_descriptor())
        .expect("current revision accepted");
    assert_eq!(doc.status().label(), "ready");
    assert_eq!(
        doc.api(),
        api_rows.as_slice(),
        "Ready must not alter verdicts"
    );

    doc.edit("synvelisPlay() -- edited");
    assert_eq!(doc.status().label(), "dirty");
    assert_eq!(
        doc.api(),
        api_rows.as_slice(),
        "Dirty must not alter verdicts"
    );

    // A delayed result for the pre-edit revision is stale (F-14).
    let stale = AnalyzeResult {
        source_revision: SourceRevision(req.source.revision.0),
        diagnostics: vec![],
        completions: None,
    };
    assert!(doc.apply_result(&stale, &analyzer_descriptor()).is_err());
    assert_eq!(doc.status().label(), "stale");
    assert_eq!(
        doc.api(),
        api_rows.as_slice(),
        "Stale must not alter verdicts"
    );

    for (error, label) in [
        (AnalyzerError::Unavailable, "unavailable"),
        (
            AnalyzerError::Unsupported {
                capability: "completion".to_owned(),
            },
            "unsupported",
        ),
        (
            AnalyzerError::VersionMismatch {
                field: "contract_version".to_owned(),
                expected: "1".to_owned(),
                actual: "0".to_owned(),
            },
            "rejected",
        ),
    ] {
        doc.apply_error(SourceRevision(doc.revision().0), &error);
        assert_eq!(doc.status().label(), label);
        assert_eq!(
            doc.api(),
            api_rows.as_slice(),
            "{label} must not alter verdicts"
        );
    }

    // Analyzer PASS ≠ Gateway authorization: the same capability whose
    // script verdict is `invocable` stays non-executable on the palette
    // while its grant is unsatisfied — and nothing in the analyzer
    // lifecycle above changed either side.
    let add_note = api_rows
        .iter()
        .find(|r| r.name == "addNote")
        .expect("addNote");
    assert_eq!(add_note.verdict, "invocable");
    let descriptor =
        resolve_capability(&snap.capabilities, &add_note.capability).expect("addNote resolves");
    let still_blocked = project_action_row(
        descriptor,
        GrantState::Missing {
            permissions: vec!["mutate-project".to_owned()],
        },
    );
    assert!(!still_blocked.executable);
    assert!(still_blocked.blocked.is_some());
    assert_eq!(
        project_script_api(&model),
        api_rows,
        "script api projection is analyzer-status independent"
    );
}
