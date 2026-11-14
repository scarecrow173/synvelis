//! Analyzer port tests: definition injection from the generated stub,
//! fake-analyzer diagnostics/completion fixtures, stale-revision and
//! unsupported-capability fail-closed paths — and the proof that
//! analyzer success never becomes execution authority.

mod common;

use common::*;
use synvelis_script::{
    AnalyzeRequest, AnalyzeResult, AnalyzerCapabilities, AnalyzerDescriptor, AnalyzerError,
    Completion, CompletionKind, Cursor, DefinitionInjection, Diagnostic, DiagnosticKind,
    DiagnosticLevel, DiagnosticSeverity, ScriptAnalyzer, ScriptBackendVersion, ScriptLanguageId,
    ScriptSource, SourcePosition, SourceRange, SourceRevision, TypeDefinition, WantedCapabilities,
    verify_result_revision,
};

/// A minimal fake analyzer: it "knows" the API only through the injected
/// definition text — exactly like a real analyzer would. It extracts
/// `declare function <name>(<params>): <ret>` signatures and diagnoses
/// unknown `synvelis<Name>(` calls and arity mismatches; completions
/// come from the same extracted names. No Luau.Analysis, no VM.
struct FakeAnalyzer {
    descriptor: AnalyzerDescriptor,
    mode: FakeMode,
}

#[derive(Clone, Copy)]
enum FakeMode {
    Ok,
    /// Refuses `SyntaxAndType` diagnostics (F-13 Unsupported).
    NoTypeDiagnostics,
    /// Always unavailable (F-18).
    Unavailable,
}

impl FakeAnalyzer {
    fn new(mode: FakeMode) -> Self {
        Self {
            descriptor: AnalyzerDescriptor {
                analyzer_id: "fake-luau".to_owned(),
                language: ScriptLanguageId::luau(),
                version: ScriptBackendVersion {
                    impl_version: "fake-0.1".to_owned(),
                    contract_version: 1,
                },
                capabilities: AnalyzerCapabilities {
                    diagnostics: DiagnosticLevel::SyntaxAndType,
                    completion: true,
                    max_source_bytes: 0,
                    supported_languages: vec![ScriptLanguageId::luau()],
                },
            },
            mode,
        }
    }

    /// Extracts `(function_name, param_count)` pairs from injected
    /// `declare function` lines in the definition sources.
    fn declared_functions(defs: &[TypeDefinition]) -> Vec<(String, usize)> {
        let mut out = Vec::new();
        for def in defs {
            for line in def.source.lines() {
                let line = line.trim();
                if let Some(rest) = line.strip_prefix("declare function ")
                    && let Some(open) = rest.find('(')
                {
                    let name = rest[..open].trim().to_owned();
                    // Crude arity: count top-level commas inside the
                    // param list. Fixture params never nest commas
                    // inside braces for this fake's purposes — the
                    // real analyzer does real parsing.
                    let close = rest.rfind(')').unwrap_or(rest.len());
                    let params = &rest[open + 1..close];
                    let count = if params.trim().is_empty() {
                        0
                    } else {
                        params.split(',').count()
                    };
                    out.push((name, count));
                }
            }
        }
        out
    }

    /// Extracts the argument text of a `name(args)` call — the text
    /// between the first `(` after the name and its matching `)`.
    /// Returns `None` when no call parens follow.
    fn call_args(after_name: &str) -> Option<&str> {
        let open = after_name.find('(')?;
        let inner = &after_name[open + 1..];
        let mut depth = 1usize;
        for (i, c) in inner.char_indices() {
            match c {
                '(' | '{' | '[' => depth += 1,
                ')' | '}' | ']' => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return Some(&inner[..i]);
                    }
                }
                _ => {}
            }
        }
        None
    }
}

impl ScriptAnalyzer for FakeAnalyzer {
    fn descriptor(&self) -> &AnalyzerDescriptor {
        &self.descriptor
    }

    fn analyze(&self, request: &AnalyzeRequest) -> Result<AnalyzeResult, AnalyzerError> {
        match self.mode {
            FakeMode::Unavailable => return Err(AnalyzerError::Unavailable),
            FakeMode::NoTypeDiagnostics => {
                if matches!(
                    request.wanted.diagnostics,
                    Some(DiagnosticLevel::SyntaxAndType)
                ) {
                    return Err(AnalyzerError::Unsupported {
                        capability: "syntax-and-type diagnostics".to_owned(),
                    });
                }
            }
            FakeMode::Ok => {}
        }

        let functions = Self::declared_functions(&request.type_definitions);
        let revision = request.source.revision;
        let mut diagnostics = Vec::new();

        // Scan `synvelis<Name>(...)` call sites: unknown names and
        // arity mismatches against the injected signatures.
        let mut search_from = 0usize;
        while let Some(idx) = request.source.text[search_from..].find("synvelis") {
            let abs = search_from + idx;
            let rest = &request.source.text[abs..];
            let end = rest
                .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .unwrap_or(rest.len());
            let called = &rest[..end];
            search_from = abs + end.max(1);
            if called.len() <= "synvelis".len() {
                continue;
            }
            let Some((_, arity)) = functions.iter().find(|(n, _)| n == called) else {
                diagnostics.push(Diagnostic {
                    severity: DiagnosticSeverity::Error,
                    kind: DiagnosticKind::Type,
                    code: "unknown-api".to_owned(),
                    message: format!("unknown script api {called:?}"),
                    range: SourceRange {
                        start: SourcePosition { line: 0, column: 0 },
                        end: SourcePosition { line: 0, column: 0 },
                    },
                    source_revision: revision,
                });
                continue;
            };
            // Arity check: count top-level comma-separated arguments in
            // the call's parens (fixture scripts keep args flat).
            let after_name = &rest[end..];
            if let Some(arg_text) = Self::call_args(after_name) {
                let actual = if arg_text.trim().is_empty() {
                    0
                } else {
                    arg_text.split(',').count()
                };
                if actual != *arity {
                    diagnostics.push(Diagnostic {
                        severity: DiagnosticSeverity::Error,
                        kind: DiagnosticKind::Type,
                        code: "wrong-arity".to_owned(),
                        message: format!("{called} expects {arity} arguments, got {actual}"),
                        range: SourceRange {
                            start: SourcePosition { line: 0, column: 0 },
                            end: SourcePosition { line: 0, column: 0 },
                        },
                        source_revision: revision,
                    });
                }
            }
        }

        // Completion: prefix-matched function names from the definitions.
        let completions = if request.wanted.completion {
            if let Some(cursor) = request.cursor {
                let prefix =
                    &request.source.text[..cursor.byte_offset.min(request.source.text.len())];
                let prefix = prefix
                    .rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .next()
                    .unwrap_or("");
                let candidates: Vec<_> = functions
                    .iter()
                    .filter(|(n, _)| n.starts_with(prefix))
                    .map(|(n, arity)| Completion {
                        label: n.clone(),
                        kind: CompletionKind::GeneratedFunction,
                        detail: Some(format!("arity {arity}")),
                        insert_text: None,
                        replace_range: SourceRange {
                            start: SourcePosition { line: 0, column: 0 },
                            end: SourcePosition { line: 0, column: 0 },
                        },
                        source_revision: revision,
                    })
                    .collect();
                Some(candidates)
            } else {
                None
            }
        } else {
            None
        };

        Ok(AnalyzeResult {
            source_revision: revision,
            diagnostics,
            completions,
        })
    }
}

fn request_with(text: &str, defs: Vec<TypeDefinition>, revision: u64) -> AnalyzeRequest {
    AnalyzeRequest {
        source: ScriptSource {
            text: text.to_owned(),
            language: ScriptLanguageId::luau(),
            revision: SourceRevision(revision),
        },
        language: ScriptLanguageId::luau(),
        type_definitions: defs,
        wanted: WantedCapabilities {
            diagnostics: Some(DiagnosticLevel::SyntaxAndType),
            completion: false,
        },
        cursor: None,
    }
}

#[test]
fn definition_injection_carries_stub_and_digest() {
    let artifacts = sample_artifacts();
    let injection = DefinitionInjection::from_stub(&artifacts.stub);
    assert_eq!(injection.package_name, "synvelis-script");
    assert_eq!(injection.source, artifacts.stub.text);
    assert_eq!(injection.model_digest, artifacts.stub.model_digest);
    assert_eq!(injection.stub_digest, artifacts.stub.stub_digest);

    let ty = injection.type_definition();
    assert_eq!(ty.package_name, "synvelis-script");
    assert!(ty.source.contains("declare function synvelisAddNote("));
}

#[test]
fn fake_analyzer_diagnoses_unknown_api_from_definitions() {
    let artifacts = sample_artifacts();
    let injection = DefinitionInjection::from_stub(&artifacts.stub);
    let analyzer = FakeAnalyzer::new(FakeMode::Ok);

    // Known API call → no unknown-api diagnostic.
    let req = request_with(
        "local r = synvelisAddNote(project, target, args, consistency)",
        vec![injection.type_definition()],
        1,
    );
    let result = analyzer.analyze(&req).unwrap();
    assert!(result.diagnostics.iter().all(|d| d.code != "unknown-api"));

    // Unknown API → typed diagnostic carrying the source revision.
    let req = request_with(
        "local r = synvelisNope(project)",
        vec![injection.type_definition()],
        7,
    );
    let result = analyzer.analyze(&req).unwrap();
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code == "unknown-api")
        .expect("unknown-api diagnostic");
    assert_eq!(diag.message, "unknown script api \"synvelisNope\"");
    assert_eq!(diag.source_revision, SourceRevision(7));

    // A hidden/non-invocable name is equally unknown to the analyzer —
    // it was never injected.
    let req = request_with(
        "local r = synvelisHiddenOp()",
        vec![injection.type_definition()],
        1,
    );
    let result = analyzer.analyze(&req).unwrap();
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == "unknown-api" && d.message.contains("synvelisHiddenOp"))
    );
}

#[test]
fn fake_analyzer_diagnoses_wrong_arity() {
    let artifacts = sample_artifacts();
    let injection = DefinitionInjection::from_stub(&artifacts.stub);
    let analyzer = FakeAnalyzer::new(FakeMode::Ok);

    // synvelisPlay declares (project, args, consistency) — 3 params.
    let req = request_with(
        "local r = synvelisPlay(project)",
        vec![injection.type_definition()],
        2,
    );
    let result = analyzer.analyze(&req).unwrap();
    let diag = result
        .diagnostics
        .iter()
        .find(|d| d.code == "wrong-arity")
        .expect("wrong-arity diagnostic");
    assert!(diag.message.contains("synvelisPlay"));
    assert_eq!(diag.source_revision, SourceRevision(2));

    // Correct arity → no arity diagnostic.
    let req = request_with(
        "local r = synvelisPlay(project, {}, consistency)",
        vec![injection.type_definition()],
        2,
    );
    let result = analyzer.analyze(&req).unwrap();
    assert!(result.diagnostics.iter().all(|d| d.code != "wrong-arity"));
}

#[test]
fn fake_analyzer_completion_lists_only_public_apis() {
    let artifacts = sample_artifacts();
    let injection = DefinitionInjection::from_stub(&artifacts.stub);
    let analyzer = FakeAnalyzer::new(FakeMode::Ok);

    let mut req = request_with("synvelis", vec![injection.type_definition()], 3);
    req.wanted.completion = true;
    req.cursor = Some(Cursor { byte_offset: 9 });
    let result = analyzer.analyze(&req).unwrap();
    verify_result_revision(&result, SourceRevision(3)).unwrap();

    let completions = result.completions.expect("completions requested");
    let labels: Vec<&str> = completions.iter().map(|c| c.label.as_str()).collect();
    assert!(labels.contains(&"synvelisAddNote"));
    assert!(labels.contains(&"synvelisPlay"));
    assert!(labels.contains(&"synvelisTrackInfo"));
    // Hidden / non-invocable APIs are absent from completion — no
    // existence leak through the analyzer surface.
    for absent in [
        "synvelisHiddenOp",
        "synvelisLegacyOp",
        "synvelisFutureOp",
        "synvelisOldOp",
        "synvelisNewThing",
        "synvelisWrongVer",
    ] {
        assert!(!labels.contains(&absent), "completion leaks {absent}");
    }
    assert!(
        completions
            .iter()
            .all(|c| c.kind == CompletionKind::GeneratedFunction)
    );
}

#[test]
fn stale_result_is_rejected_by_consumer() {
    let analyzer = FakeAnalyzer::new(FakeMode::Ok);
    let req = request_with("synvelisPlay()", vec![], 11);
    let result = analyzer.analyze(&req).unwrap();
    assert_eq!(result.source_revision, SourceRevision(11));

    // Same revision → accepted.
    verify_result_revision(&result, SourceRevision(11)).unwrap();
    // Different revision → StaleResult (F-14).
    let err = verify_result_revision(&result, SourceRevision(12))
        .expect_err("stale revision must reject");
    assert_eq!(err, AnalyzerError::StaleResult);
}

#[test]
fn unsupported_capability_and_unavailable_are_typed() {
    let analyzer = FakeAnalyzer::new(FakeMode::NoTypeDiagnostics);
    let mut req = request_with("synvelisPlay()", vec![], 1);
    req.wanted.diagnostics = Some(DiagnosticLevel::SyntaxAndType);
    let err = analyzer.analyze(&req).expect_err("unsupported diagnostics");
    assert!(matches!(err, AnalyzerError::Unsupported { .. }));

    // Downgrade request → accepted.
    req.wanted.diagnostics = Some(DiagnosticLevel::Syntax);
    analyzer.analyze(&req).unwrap();

    let unavailable = FakeAnalyzer::new(FakeMode::Unavailable);
    let err = unavailable.analyze(&req).expect_err("unavailable analyzer");
    assert_eq!(err, AnalyzerError::Unavailable);
}

#[test]
fn analyzer_success_is_not_execution_authority() {
    // Analyzer success changes nothing about invocability: the model's
    // verdicts are computed from the snapshot, not from analysis.
    let model = sample_model();
    let artifacts = sample_artifacts();
    let injection = DefinitionInjection::from_stub(&artifacts.stub);
    let analyzer = FakeAnalyzer::new(FakeMode::Ok);

    let req = request_with("synvelisPlay()", vec![injection.type_definition()], 1);
    analyzer.analyze(&req).unwrap(); // analysis "succeeds"

    // Disabled capability still rejects at assembly — analysis never
    // minted a grant.
    let err = model
        .assemble_call("legacyOp", play_call())
        .expect_err("disabled stays disabled");
    assert!(matches!(
        err,
        synvelis_script::ScriptApiError::NotInvocable { .. }
    ));

    // And the invocable set is exactly what the stub declared.
    assert_eq!(
        artifacts.stub.entries.len(),
        model.invocable_entries().len()
    );
}
