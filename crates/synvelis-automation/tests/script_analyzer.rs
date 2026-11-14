//! SV-SCRIPT-04 (synvelis#274) — Analyzer Registry + Luau
//! port-level adapter tests.
//!
//! Covers the fail-closed resolution vocabulary (F-13/F-14/F-18),
//! the deterministic port-level analyzer's syntax/type diagnostics
//! and completion driven solely by injected `DefinitionInjection`
//! type definitions, and the headless `analyze_source` /
//! `analyze_with_injection` entries. The fixture snapshot is the same
//! canonical `RegistrySnapshot` document synvelis-script's golden
//! tests use — no live Gateway registries involved.

use synvelis_automation::{
    ANALYZER_CONTRACT_VERSION, AnalyzerRegistry, AnalyzerRegistryError, LuauPortAnalyzer,
    analyze_with_injection,
};
use synvelis_gateway::{CapabilityId, CapabilityRef, CapabilityVersion};
use synvelis_script::{
    AnalyzeRequest, AnalyzerCapabilities, AnalyzerDescriptor, AnalyzerError, DefinitionInjection,
    DiagnosticLevel, LuauStub, RegistrySnapshot, ScriptAnalyzer, ScriptApiModel,
    ScriptBackendVersion, ScriptLanguageId, ScriptSource, ScriptSurfaceEntry, ScriptSurfaceSpec,
    ScriptVisibility, SourceRevision, WantedCapabilities,
};

fn vid(text: &str) -> CapabilityId {
    CapabilityId::new(text).expect("fixture capability id")
}

fn cver(major: u32, minor: u32) -> CapabilityVersion {
    CapabilityVersion::new(major, minor)
}

fn entry(
    name: &str,
    id: &str,
    version: CapabilityVersion,
    visibility: ScriptVisibility,
) -> ScriptSurfaceEntry {
    ScriptSurfaceEntry {
        name: name.to_owned(),
        capability: CapabilityRef {
            id: vid(id),
            version,
        },
        visibility,
        description: None,
    }
}

/// Public surface covering invocable, disabled, retired,
/// unsupported-version, and hidden entries.
fn surface() -> ScriptSurfaceSpec {
    ScriptSurfaceSpec {
        schema_version: synvelis_script::SURFACE_SCHEMA_VERSION,
        entries: vec![
            entry(
                "addNote",
                "synvelis.midi.add-note",
                cver(1, 0),
                ScriptVisibility::Public,
            ),
            entry(
                "play",
                "synvelis.transport.play",
                cver(1, 0),
                ScriptVisibility::Public,
            ),
            entry(
                "trackInfo",
                "synvelis.query.track-info",
                cver(1, 0),
                ScriptVisibility::Public,
            ),
            entry(
                "legacyOp",
                "synvelis.legacy.thing",
                cver(1, 0),
                ScriptVisibility::Public,
            ),
            entry(
                "oldOp",
                "synvelis.old.thing",
                cver(1, 0),
                ScriptVisibility::Public,
            ),
            entry(
                "hiddenOp",
                "synvelis.midi.add-note",
                cver(1, 0),
                ScriptVisibility::Hidden,
            ),
            entry(
                "wrongVer",
                "synvelis.midi.add-note",
                cver(9, 9),
                ScriptVisibility::Public,
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

fn stub() -> LuauStub {
    LuauStub::generate(&model()).expect("fixture stub generates")
}

fn injection() -> DefinitionInjection {
    DefinitionInjection::from_stub(&stub())
}

fn registry_with_luau() -> AnalyzerRegistry {
    let mut registry = AnalyzerRegistry::new();
    registry
        .register(Box::new(LuauPortAnalyzer::new()))
        .expect("luau analyzer registers");
    registry
}

fn luau_source(text: &str, revision: u64) -> ScriptSource {
    ScriptSource {
        text: text.to_owned(),
        language: ScriptLanguageId::luau(),
        revision: SourceRevision(revision),
    }
}

fn wanted(diagnostics: Option<DiagnosticLevel>, completion: bool) -> WantedCapabilities {
    WantedCapabilities {
        diagnostics,
        completion,
    }
}

// ── registry resolution ───────────────────────────────────────────────

#[test]
fn registry_resolves_registered_language() {
    let registry = registry_with_luau();
    let analyzer = registry
        .resolve(
            &ScriptLanguageId::luau(),
            &wanted(Some(DiagnosticLevel::SyntaxAndType), true),
        )
        .expect("luau resolves");
    assert_eq!(analyzer.descriptor().analyzer_id, "synvelis-luau-port");
    assert_eq!(
        analyzer.descriptor().version.contract_version,
        ANALYZER_CONTRACT_VERSION
    );
}

#[test]
fn registry_unregistered_language_is_unavailable() {
    let registry = registry_with_luau();
    let err = registry
        .resolve(
            &ScriptLanguageId("rhai".to_owned()),
            &wanted(Some(DiagnosticLevel::Syntax), false),
        )
        .map(|_| ())
        .expect_err("rhai has no analyzer");
    assert_eq!(err, AnalyzerError::Unavailable);

    let empty = AnalyzerRegistry::new();
    let err = empty
        .resolve(&ScriptLanguageId::luau(), &wanted(None, false))
        .map(|_| ())
        .expect_err("empty registry");
    assert_eq!(err, AnalyzerError::Unavailable);
}

#[test]
fn registry_rejects_unadvertised_capabilities() {
    // A syntax-only analyzer cannot serve SyntaxAndType or completion.
    struct SyntaxOnly;
    impl synvelis_script::ScriptAnalyzer for SyntaxOnly {
        fn descriptor(&self) -> &AnalyzerDescriptor {
            static DESCRIPTOR: std::sync::OnceLock<AnalyzerDescriptor> = std::sync::OnceLock::new();
            DESCRIPTOR.get_or_init(|| AnalyzerDescriptor {
                analyzer_id: "syntax-only".to_owned(),
                language: ScriptLanguageId::luau(),
                version: ScriptBackendVersion {
                    impl_version: "test".to_owned(),
                    contract_version: ANALYZER_CONTRACT_VERSION,
                },
                capabilities: AnalyzerCapabilities {
                    diagnostics: DiagnosticLevel::Syntax,
                    completion: false,
                    max_source_bytes: 0,
                    supported_languages: vec![ScriptLanguageId::luau()],
                },
            })
        }
        fn analyze(
            &self,
            _request: &AnalyzeRequest,
        ) -> Result<synvelis_script::AnalyzeResult, AnalyzerError> {
            unreachable!("resolve must reject before analyze")
        }
    }

    let mut registry = AnalyzerRegistry::new();
    registry.register(Box::new(SyntaxOnly)).unwrap();

    // Syntax request resolves.
    registry
        .resolve(
            &ScriptLanguageId::luau(),
            &wanted(Some(DiagnosticLevel::Syntax), false),
        )
        .expect("syntax resolves");

    // SyntaxAndType exceeds the advertised depth → Unsupported (F-13).
    let err = registry
        .resolve(
            &ScriptLanguageId::luau(),
            &wanted(Some(DiagnosticLevel::SyntaxAndType), false),
        )
        .map(|_| ())
        .expect_err("type diagnostics unsupported");
    assert!(matches!(err, AnalyzerError::Unsupported { .. }));

    // Completion not advertised → Unsupported.
    let err = registry
        .resolve(
            &ScriptLanguageId::luau(),
            &wanted(Some(DiagnosticLevel::Syntax), true),
        )
        .map(|_| ())
        .expect_err("completion unsupported");
    assert!(matches!(err, AnalyzerError::Unsupported { .. }));
}

#[test]
fn registry_rejects_contract_version_mismatch() {
    struct OldContract;
    impl synvelis_script::ScriptAnalyzer for OldContract {
        fn descriptor(&self) -> &AnalyzerDescriptor {
            static DESCRIPTOR: std::sync::OnceLock<AnalyzerDescriptor> = std::sync::OnceLock::new();
            DESCRIPTOR.get_or_init(|| AnalyzerDescriptor {
                analyzer_id: "old-contract".to_owned(),
                language: ScriptLanguageId::luau(),
                version: ScriptBackendVersion {
                    impl_version: "legacy".to_owned(),
                    contract_version: ANALYZER_CONTRACT_VERSION - 1,
                },
                capabilities: AnalyzerCapabilities {
                    diagnostics: DiagnosticLevel::SyntaxAndType,
                    completion: true,
                    max_source_bytes: 0,
                    supported_languages: vec![ScriptLanguageId::luau()],
                },
            })
        }
        fn analyze(
            &self,
            _request: &AnalyzeRequest,
        ) -> Result<synvelis_script::AnalyzeResult, AnalyzerError> {
            unreachable!("resolve must reject before analyze")
        }
    }

    let mut registry = AnalyzerRegistry::new();
    registry.register(Box::new(OldContract)).unwrap();
    let err = registry
        .resolve(&ScriptLanguageId::luau(), &wanted(None, false))
        .map(|_| ())
        .expect_err("contract mismatch");
    assert!(matches!(err, AnalyzerError::VersionMismatch { .. }));
}

struct NoLanguages;
impl synvelis_script::ScriptAnalyzer for NoLanguages {
    fn descriptor(&self) -> &AnalyzerDescriptor {
        static DESCRIPTOR: std::sync::OnceLock<AnalyzerDescriptor> = std::sync::OnceLock::new();
        DESCRIPTOR.get_or_init(|| AnalyzerDescriptor {
            analyzer_id: "no-langs".to_owned(),
            language: ScriptLanguageId::luau(),
            version: ScriptBackendVersion {
                impl_version: "test".to_owned(),
                contract_version: ANALYZER_CONTRACT_VERSION,
            },
            capabilities: AnalyzerCapabilities {
                diagnostics: DiagnosticLevel::Unsupported,
                completion: false,
                max_source_bytes: 0,
                supported_languages: vec![],
            },
        })
    }
    fn analyze(
        &self,
        _request: &AnalyzeRequest,
    ) -> Result<synvelis_script::AnalyzeResult, AnalyzerError> {
        unreachable!()
    }
}

#[test]
fn registry_rejects_duplicate_and_empty_languages() {
    let mut registry = registry_with_luau();
    let err = registry
        .register(Box::new(LuauPortAnalyzer::new()))
        .expect_err("duplicate luau");
    assert_eq!(
        err,
        AnalyzerRegistryError::DuplicateLanguage {
            language: "luau".to_owned()
        }
    );

    let mut registry = AnalyzerRegistry::new();
    let err = registry
        .register(Box::new(NoLanguages))
        .expect_err("empty languages");
    assert!(matches!(
        err,
        AnalyzerRegistryError::EmptySupportedLanguages { .. }
    ));
}

// ── Luau port-level adapter ───────────────────────────────────────────

#[test]
fn luau_adapter_reports_unknown_api_and_wrong_arity_from_injected_defs() {
    let analyzer = LuauPortAnalyzer::new();
    let inj = injection();
    let defs = vec![inj.type_definition()];

    // Known API, correct arity → no type diagnostics.
    let req = AnalyzeRequest {
        source: luau_source("local r = synvelisPlay(project, {}, consistency)\n", 3),
        language: ScriptLanguageId::luau(),
        type_definitions: defs.clone(),
        wanted: wanted(Some(DiagnosticLevel::SyntaxAndType), false),
        cursor: None,
    };
    let result = analyzer.analyze(&req).unwrap();
    assert!(result.diagnostics.iter().all(|d| d.code != "unknown-api"));
    assert!(result.diagnostics.iter().all(|d| d.code != "wrong-arity"));
    assert_eq!(result.source_revision, SourceRevision(3));

    // Unknown API → typed diagnostic carrying the revision.
    let req = AnalyzeRequest {
        source: luau_source("local r = synvelisNope(project)\n", 7),
        language: ScriptLanguageId::luau(),
        type_definitions: defs.clone(),
        wanted: wanted(Some(DiagnosticLevel::SyntaxAndType), false),
        cursor: None,
    };
    let result = analyzer.analyze(&req).unwrap();
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code == "unknown-api")
        .expect("unknown-api diagnostic");
    assert_eq!(diag.message, "unknown script api \"synvelisNope\"");
    assert_eq!(diag.source_revision, SourceRevision(7));

    // Hidden / non-invocable names were never injected → equally
    // unknown; no existence leak.
    for name in [
        "synvelisHiddenOp",
        "synvelisLegacyOp",
        "synvelisOldOp",
        "synvelisWrongVer",
        "synvelisFutureOp",
    ] {
        let req = AnalyzeRequest {
            source: luau_source(&format!("local r = {name}()\n"), 1),
            language: ScriptLanguageId::luau(),
            type_definitions: defs.clone(),
            wanted: wanted(Some(DiagnosticLevel::SyntaxAndType), false),
            cursor: None,
        };
        let result = analyzer.analyze(&req).unwrap();
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == "unknown-api" && d.message.contains(name)),
            "{name} must be unknown to the analyzer"
        );
    }

    // Arity mismatch → wrong-arity (synvelisPlay declares 3 params).
    let req = AnalyzeRequest {
        source: luau_source("local r = synvelisPlay(project)\n", 2),
        language: ScriptLanguageId::luau(),
        type_definitions: defs,
        wanted: wanted(Some(DiagnosticLevel::SyntaxAndType), false),
        cursor: None,
    };
    let result = analyzer.analyze(&req).unwrap();
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code == "wrong-arity")
        .expect("wrong-arity diagnostic");
    assert!(diag.message.contains("synvelisPlay"));
}

#[test]
fn luau_adapter_reports_syntax_diagnostics() {
    let analyzer = LuauPortAnalyzer::new();
    let req = AnalyzeRequest {
        source: luau_source("local s = \"unterminated\nlocal t = 1\n", 4),
        language: ScriptLanguageId::luau(),
        type_definitions: vec![],
        wanted: wanted(Some(DiagnosticLevel::Syntax), false),
        cursor: None,
    };
    let result = analyzer.analyze(&req).unwrap();
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code == "syntax-unterminated-string")
        .expect("unterminated string diagnostic");
    assert_eq!(diag.kind, synvelis_script::DiagnosticKind::Syntax);
    assert_eq!(diag.source_revision, SourceRevision(4));

    // Long comment never closed.
    let req = AnalyzeRequest {
        source: luau_source("--[[ never ends\ncode()\n", 1),
        language: ScriptLanguageId::luau(),
        type_definitions: vec![],
        wanted: wanted(Some(DiagnosticLevel::SyntaxAndType), false),
        cursor: None,
    };
    let result = analyzer.analyze(&req).unwrap();
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "syntax-unterminated-comment")
    );

    // Balanced strings/comments → no syntax diagnostics.
    let req = AnalyzeRequest {
        source: luau_source(
            "-- line comment\n--[[ long ]] ok\nlocal s = \"ok \\\" esc\"\nlocal t = [[long]]\n",
            1,
        ),
        language: ScriptLanguageId::luau(),
        type_definitions: vec![],
        wanted: wanted(Some(DiagnosticLevel::SyntaxAndType), false),
        cursor: None,
    };
    let result = analyzer.analyze(&req).unwrap();
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
}

#[test]
fn luau_adapter_completion_lists_only_public_apis() {
    let analyzer = LuauPortAnalyzer::new();
    let inj = injection();
    let text = "synvelis";
    let req = AnalyzeRequest {
        source: luau_source(text, 5),
        language: ScriptLanguageId::luau(),
        type_definitions: vec![inj.type_definition()],
        wanted: wanted(Some(DiagnosticLevel::Syntax), true),
        cursor: Some(synvelis_script::Cursor { byte_offset: 8 }),
    };
    let result = analyzer.analyze(&req).unwrap();
    let completions = result.completions.expect("completions requested");
    let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();
    assert!(labels.contains(&"synvelisAddNote"));
    assert!(labels.contains(&"synvelisPlay"));
    assert!(labels.contains(&"synvelisTrackInfo"));
    for absent in [
        "synvelisHiddenOp",
        "synvelisLegacyOp",
        "synvelisOldOp",
        "synvelisWrongVer",
        "synvelisFutureOp",
    ] {
        assert!(!labels.contains(&absent), "completion leaks {absent}");
    }
    assert!(
        completions
            .iter()
            .all(|c| c.source_revision == SourceRevision(5))
    );
}

#[test]
fn luau_adapter_rejects_invalid_requests() {
    let analyzer = LuauPortAnalyzer::new();
    // Language mismatch.
    let req = AnalyzeRequest {
        source: ScriptSource {
            text: "x".to_owned(),
            language: ScriptLanguageId("rhai".to_owned()),
            revision: SourceRevision(1),
        },
        language: ScriptLanguageId::luau(),
        type_definitions: vec![],
        wanted: wanted(None, false),
        cursor: None,
    };
    let err = analyzer.analyze(&req).expect_err("language mismatch");
    assert!(matches!(err, AnalyzerError::InvalidRequest { .. }));

    // Cursor outside source / not a char boundary.
    let req = AnalyzeRequest {
        source: luau_source("abc", 1),
        language: ScriptLanguageId::luau(),
        type_definitions: vec![],
        wanted: wanted(None, true),
        cursor: Some(synvelis_script::Cursor { byte_offset: 99 }),
    };
    let err = analyzer.analyze(&req).expect_err("cursor out of range");
    assert!(matches!(err, AnalyzerError::InvalidRequest { .. }));
}

// ── headless entry points ─────────────────────────────────────────────

#[test]
fn analyze_source_resolves_and_runs() {
    let registry = registry_with_luau();
    let inj = injection();
    let result = analyze_with_injection(
        &registry,
        luau_source("local r = synvelisNope()\n", 9),
        &inj,
        wanted(Some(DiagnosticLevel::SyntaxAndType), false),
        None,
    )
    .expect("analysis runs");
    assert_eq!(result.source_revision, SourceRevision(9));
    assert!(result.diagnostics.iter().any(|d| d.code == "unknown-api"));
}

#[test]
fn analyze_source_unregistered_language_is_typed_unavailable() {
    let registry = registry_with_luau();
    let inj = injection();
    let source = ScriptSource {
        text: "x".to_owned(),
        language: ScriptLanguageId("yaml".to_owned()),
        revision: SourceRevision(1),
    };
    let err = analyze_with_injection(
        &registry,
        source,
        &inj,
        wanted(Some(DiagnosticLevel::Syntax), false),
        None,
    )
    .map(|_| ())
    .expect_err("yaml has no analyzer");
    assert_eq!(err, AnalyzerError::Unavailable);
}

#[test]
fn analyze_source_never_becomes_execution_authority() {
    // Analyzer success changes nothing about the model's verdicts:
    // non-invocable entries stay non-invocable regardless of analysis.
    let m = model();
    assert!(!m.is_invocable("hiddenOp"));
    assert!(!m.is_invocable("legacyOp"));
    assert!(!m.is_invocable("wrongVer"));
    assert!(m.is_invocable("play"));
    // And the stub the analyzer consumed contains only invocable APIs.
    let stub_text = stub().text;
    assert!(stub_text.contains("declare function synvelisPlay("));
    assert!(!stub_text.contains("synvelisHiddenOp"));
    assert!(!stub_text.contains("synvelisLegacyOp"));
}
