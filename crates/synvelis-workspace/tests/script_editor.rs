//! SV-SCRIPT-04 (synvelis#274) — Script Editor document model tests.
//!
//! Covers the consumer-side stale-rejection contract (F-14): results
//! are accepted only at the current revision against the pinned
//! definition digests; delayed responses, unsolicited results, and
//! model/stub drift are all rejected typed. Also covers the
//! `ScriptApiModel` verdict projection (Hidden never leaks) and the
//! tolerant `ViewState::script_editor` persistence decode.

use synvelis_gateway::{CapabilityId, CapabilityRef, CapabilityVersion};
use synvelis_script::{
    AnalyzerDescriptor, AnalyzerError, DefinitionInjection, DiagnosticLevel, LuauStub,
    RegistrySnapshot, ScriptApiModel, ScriptBackendVersion, ScriptLanguageId, ScriptSurfaceEntry,
    ScriptSurfaceSpec, ScriptVisibility, SourceRevision, WantedCapabilities,
};
use synvelis_workspace::{
    ScriptAnalyzerStatus, ScriptEditorDocument, ScriptEditorError, ScriptEditorViewState,
    ViewState, project_script_api,
};

fn vid(text: &str) -> CapabilityId {
    CapabilityId::new(text).expect("fixture capability id")
}

fn entry(name: &str, id: &str, visibility: ScriptVisibility) -> ScriptSurfaceEntry {
    ScriptSurfaceEntry {
        name: name.to_owned(),
        capability: CapabilityRef {
            id: vid(id),
            version: CapabilityVersion::new(1, 0),
        },
        visibility,
        description: None,
    }
}

fn surface(extra_hidden: bool) -> ScriptSurfaceSpec {
    let mut entries = vec![
        entry(
            "addNote",
            "synvelis.midi.add-note",
            ScriptVisibility::Public,
        ),
        entry("play", "synvelis.transport.play", ScriptVisibility::Public),
        entry(
            "legacyOp",
            "synvelis.legacy.thing",
            ScriptVisibility::Public,
        ),
        entry("oldOp", "synvelis.old.thing", ScriptVisibility::Public),
        ScriptSurfaceEntry {
            name: "wrongVer".to_owned(),
            capability: CapabilityRef {
                id: vid("synvelis.midi.add-note"),
                version: CapabilityVersion::new(9, 9),
            },
            visibility: ScriptVisibility::Public,
            description: None,
        },
    ];
    if extra_hidden {
        // A second surface doc → different surface/model/stub digests.
        entries.push(entry(
            "trackInfo",
            "synvelis.query.track-info",
            ScriptVisibility::Public,
        ));
    }
    entries.push(entry(
        "hiddenOp",
        "synvelis.midi.add-note",
        ScriptVisibility::Hidden,
    ));
    ScriptSurfaceSpec {
        schema_version: synvelis_script::SURFACE_SCHEMA_VERSION,
        entries,
    }
}

fn snapshot() -> RegistrySnapshot {
    serde_json::from_str(include_str!(
        "../../synvelis-script/tests/fixtures/registry_snapshot.json"
    ))
    .expect("snapshot fixture parses")
}

fn injection_for(hidden_extra: bool) -> DefinitionInjection {
    let model =
        ScriptApiModel::build(&snapshot(), &surface(hidden_extra)).expect("fixture model builds");
    let stub = LuauStub::generate(&model).expect("fixture stub generates");
    DefinitionInjection::from_stub(&stub)
}

fn model_for(hidden_extra: bool) -> ScriptApiModel {
    ScriptApiModel::build(&snapshot(), &surface(hidden_extra)).expect("model")
}

fn doc() -> ScriptEditorDocument {
    ScriptEditorDocument::new("main", ScriptLanguageId::luau())
}

fn descriptor() -> AnalyzerDescriptor {
    AnalyzerDescriptor {
        analyzer_id: "synvelis-luau-port".to_owned(),
        language: ScriptLanguageId::luau(),
        version: ScriptBackendVersion {
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

#[test]
fn edit_bumps_revision_and_marks_dirty() {
    let mut doc = doc();
    doc.edit("local x = 1");
    assert_eq!(doc.revision(), SourceRevision(1));
    doc.set_definitions(&injection_for(false));
    let req = doc.begin_analysis(wanted());
    // Simulate an accepted result at rev 1 via a fabricated result.
    let result = synvelis_script::AnalyzeResult {
        source_revision: SourceRevision(1),
        diagnostics: vec![],
        completions: None,
    };
    doc.apply_result(&result, &descriptor()).unwrap();
    assert!(matches!(doc.status(), ScriptAnalyzerStatus::Ready { .. }));

    doc.edit("local x = 2");
    assert_eq!(doc.revision(), SourceRevision(2));
    assert_eq!(doc.status(), &ScriptAnalyzerStatus::Dirty);
    assert_eq!(req.source.revision, SourceRevision(1));
}

#[test]
fn delayed_result_is_rejected_as_stale() {
    let mut doc = doc();
    doc.edit("synvelisNope()");
    let _req = doc.begin_analysis(wanted());
    // User edits again before the async result arrives.
    doc.edit("synvelisNope() -- edited");
    assert_eq!(doc.revision(), SourceRevision(2));

    // The analyzer answers for revision 1 — stale on arrival (F-14).
    let stale = synvelis_script::AnalyzeResult {
        source_revision: SourceRevision(1),
        diagnostics: vec![],
        completions: None,
    };
    let err = doc
        .apply_result(&stale, &descriptor())
        .expect_err("stale revision rejects");
    assert_eq!(err, ScriptEditorError::StaleResult);
    assert_eq!(doc.status(), &ScriptAnalyzerStatus::Stale);
    assert_eq!(doc.diagnostics(), &[]);
}

#[test]
fn unsolicited_result_is_rejected() {
    let mut doc = doc();
    doc.edit("x");
    // No begin_analysis — a result arrives anyway.
    let result = synvelis_script::AnalyzeResult {
        source_revision: SourceRevision(1),
        diagnostics: vec![],
        completions: None,
    };
    let err = doc
        .apply_result(&result, &descriptor())
        .expect_err("unsolicited rejects");
    assert!(matches!(err, ScriptEditorError::UnsolicitedResult { .. }));
}

#[test]
fn definition_digest_drift_rejects_in_flight_result() {
    let mut doc = doc();
    doc.edit("synvelisPlay()");
    doc.set_definitions(&injection_for(false));
    let _req = doc.begin_analysis(wanted());

    // Registry/model updated → new stub digests installed before the
    // in-flight result lands.
    doc.set_definitions(&injection_for(true));
    assert_ne!(
        injection_for(false).model_digest,
        injection_for(true).model_digest
    );

    let result = synvelis_script::AnalyzeResult {
        source_revision: SourceRevision(1),
        diagnostics: vec![],
        completions: None,
    };
    let err = doc
        .apply_result(&result, &descriptor())
        .expect_err("digest drift is stale");
    assert_eq!(err, ScriptEditorError::StaleResult);
    assert_eq!(doc.status(), &ScriptAnalyzerStatus::Stale);
}

#[test]
fn apply_error_maps_typed_vocabulary() {
    let mut doc = doc();
    doc.edit("x");
    let req = doc.begin_analysis(wanted());
    doc.apply_error(req.source.revision, &AnalyzerError::Unavailable);
    assert_eq!(doc.status(), &ScriptAnalyzerStatus::Unavailable);

    let req = doc.begin_analysis(wanted());
    doc.apply_error(
        req.source.revision,
        &AnalyzerError::Unsupported {
            capability: "completion".to_owned(),
        },
    );
    assert_eq!(
        doc.status(),
        &ScriptAnalyzerStatus::Unsupported {
            capability: "completion".to_owned()
        }
    );

    let req = doc.begin_analysis(wanted());
    doc.apply_error(
        req.source.revision,
        &AnalyzerError::VersionMismatch {
            field: "contract_version".to_owned(),
            expected: "1".to_owned(),
            actual: "0".to_owned(),
        },
    );
    assert!(matches!(
        doc.status(),
        ScriptAnalyzerStatus::Rejected { .. }
    ));
}

#[test]
fn accepted_result_projects_rows_and_ready_status() {
    let mut doc = doc();
    doc.edit("local r = synvelisNope()\n");
    doc.set_definitions(&injection_for(false));
    let _req = doc.begin_analysis(wanted());

    let result = synvelis_script::AnalyzeResult {
        source_revision: SourceRevision(1),
        diagnostics: vec![synvelis_script::Diagnostic {
            severity: synvelis_script::DiagnosticSeverity::Error,
            kind: synvelis_script::DiagnosticKind::Type,
            code: "unknown-api".to_owned(),
            message: "unknown script api \"synvelisNope\"".to_owned(),
            range: synvelis_script::SourceRange {
                start: synvelis_script::SourcePosition {
                    line: 0,
                    column: 10,
                },
                end: synvelis_script::SourcePosition {
                    line: 0,
                    column: 22,
                },
            },
            source_revision: SourceRevision(1),
        }],
        completions: Some(vec![synvelis_script::Completion {
            label: "synvelisPlay".to_owned(),
            kind: synvelis_script::CompletionKind::GeneratedFunction,
            detail: Some("synvelisPlay(project, args, consistency): PlayOutput".to_owned()),
            insert_text: None,
            replace_range: synvelis_script::SourceRange {
                start: synvelis_script::SourcePosition { line: 0, column: 0 },
                end: synvelis_script::SourcePosition { line: 0, column: 8 },
            },
            source_revision: SourceRevision(1),
        }]),
    };
    doc.apply_result(&result, &descriptor()).unwrap();

    assert_eq!(doc.diagnostics().len(), 1);
    assert_eq!(doc.diagnostics()[0].severity, "error");
    assert_eq!(doc.diagnostics()[0].code, "unknown-api");
    assert_eq!(doc.completions().len(), 1);
    assert_eq!(doc.completions()[0].kind, "generated-function");
    assert!(matches!(doc.status(), ScriptAnalyzerStatus::Ready { .. }));

    let view = doc.view();
    assert_eq!(view.revision, 1);
    assert_eq!(view.language, "luau");
    assert_eq!(view.text, "local r = synvelisNope()\n");
    assert_eq!(view.diagnostics.len(), 1);
    assert!(!view.analyzer_failed());
}

#[test]
fn api_projection_shows_verdicts_and_never_leaks_hidden() {
    let rows = project_script_api(&model_for(false));
    let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
    assert!(names.contains(&"addNote"));
    assert!(names.contains(&"play"));
    assert!(names.contains(&"legacyOp"));
    assert!(names.contains(&"oldOp"));
    assert!(names.contains(&"wrongVer"));
    // Hidden entries are never projected.
    assert!(!names.contains(&"hiddenOp"));

    let verdict_of = |name: &str| {
        rows.iter()
            .find(|r| r.name == name)
            .map(|r| r.verdict.as_str())
            .unwrap()
    };
    assert_eq!(verdict_of("play"), "invocable");
    assert_eq!(verdict_of("legacyOp"), "disabled");
    assert_eq!(verdict_of("oldOp"), "retired");
    assert_eq!(verdict_of("wrongVer"), "unsupported-version");
}

#[test]
fn view_state_selections_and_tolerant_decode() {
    let mut state = ScriptEditorViewState::default();
    assert!(state.show_api);
    state.apply(&synvelis_workspace::ScriptEditorViewAction::SelectDiagnostic { index: 2 });
    state.apply(&synvelis_workspace::ScriptEditorViewAction::SelectApi { index: 1 });
    state.apply(&synvelis_workspace::ScriptEditorViewAction::ToggleApi);
    assert_eq!(state.selected_diagnostic, Some(2));
    assert_eq!(state.selected_api, Some(1));
    assert!(!state.show_api);

    // Serde round-trip.
    let json = serde_json::to_string(&state).unwrap();
    let back: ScriptEditorViewState = serde_json::from_str(&json).unwrap();
    assert_eq!(back, state);

    // ViewState carries the field and tolerates malformed payloads.
    let vs = ViewState {
        script_editor: Some(state.clone()),
        ..Default::default()
    };
    let vs_json = serde_json::to_string(&vs).unwrap();
    let vs_back: ViewState = serde_json::from_str(&vs_json).unwrap();
    assert_eq!(vs_back.script_editor, Some(state));

    // Malformed payload degrades to None, not an error.
    let bad = vs_json.replace("selected_diagnostic", "selected_diag_x");
    let vs_bad: ViewState = serde_json::from_str(&bad).unwrap();
    assert_eq!(vs_bad.script_editor, None);
}
