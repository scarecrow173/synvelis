//! Runtime backend port tests (synvelis#273) — registry fail-closed
//! resolution, `ScriptValue` JSON codec fidelity, and the
//! `ScriptWorkflowSpec` artifact decode path. The fake backend here is
//! a test double only; it never ships in a production path.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use serde_json::json;
use synvelis_script::backend::{
    ArtifactKind, ArtifactSource, BackendCapabilities, BackendDescriptor, BudgetKind,
    CancelGranularity, DefinitionDigest, ExecutionBudget, ProducedBy, ResolveErrorReason,
    SCRIPT_ARTIFACT_SCHEMA_VERSION, SCRIPT_CONTRACT_VERSION, ScriptArtifact, ScriptBackend,
    ScriptBackendId, ScriptBackendRegistry, ScriptError, ScriptErrorKind, ScriptRequirement,
    ScriptResult, ScriptSession, ScriptSource, ScriptValue, SessionConfig, SessionError,
    SourceDigest,
};
use synvelis_script::{
    ScriptApiError, ScriptBackendVersion, ScriptLanguageId, ScriptWorkflowSpec, SourceRevision,
    WORKFLOW_SPEC_SCHEMA_VERSION,
};

// ── fake backend ──────────────────────────────────────────────────

#[derive(Debug)]
struct FakeBackend {
    descriptor: BackendDescriptor,
    result: Option<ScriptResult>,
    open_error: Option<SessionError>,
}

impl FakeBackend {
    fn new(id: &str, language: &str) -> Self {
        Self {
            descriptor: BackendDescriptor {
                id: ScriptBackendId(id.to_owned()),
                language: ScriptLanguageId(language.to_owned()),
                version: ScriptBackendVersion {
                    impl_version: "fake-1".to_owned(),
                    contract_version: SCRIPT_CONTRACT_VERSION,
                },
                capabilities: BackendCapabilities {
                    enforced_budgets: BTreeSet::from([BudgetKind::Instruction]),
                    max_source_bytes: Some(1 << 20),
                    artifact_kinds: BTreeSet::from([ArtifactKind::WorkflowDefinition]),
                    send: false,
                    sync: false,
                    owner_thread_only: true,
                    cancel_granularity: CancelGranularity::Safepoint,
                    native_integer: false,
                },
            },
            result: None,
            open_error: None,
        }
    }
}

struct FakeSession {
    id: String,
    backend: ScriptBackendId,
    result: Option<ScriptResult>,
    closed: bool,
}

impl ScriptSession for FakeSession {
    fn session_id(&self) -> &str {
        &self.id
    }
    fn backend_id(&self) -> &ScriptBackendId {
        &self.backend
    }
    fn evaluate(&mut self) -> Result<ScriptResult, ScriptError> {
        if self.closed {
            return Err(ScriptError::new(
                ScriptErrorKind::Backend {
                    detail: "session closed".to_owned(),
                },
                "evaluate after close",
            ));
        }
        Ok(self.result.take().unwrap_or_default())
    }
    fn cancel(&mut self) {}
    fn close(&mut self) {
        self.closed = true;
    }
}

impl ScriptBackend for FakeBackend {
    fn descriptor(&self) -> &BackendDescriptor {
        &self.descriptor
    }
    fn open_session(&self, config: &SessionConfig) -> Result<Box<dyn ScriptSession>, SessionError> {
        if let Some(e) = &self.open_error {
            return Err(e.clone());
        }
        // Mirrors the contract: digest is verified, not trusted.
        let actual = SourceDigest(synvelis_script::sha256_hex(config.source.text.as_bytes()));
        if actual != config.source.digest {
            return Err(SessionError::SourceDigestMismatch {
                expected: config.source.digest.clone(),
                actual,
            });
        }
        Ok(Box::new(FakeSession {
            id: "fake-session-1".to_owned(),
            backend: self.descriptor.id.clone(),
            result: self.result.clone(),
            closed: false,
        }))
    }
}

fn source(text: &str) -> ScriptSource {
    ScriptSource {
        text: text.to_owned(),
        language: ScriptLanguageId::luau(),
        backend_hint: None,
        digest: SourceDigest(synvelis_script::sha256_hex(text.as_bytes())),
        revision: SourceRevision(1),
    }
}

fn session_config(text: &str) -> SessionConfig {
    SessionConfig {
        budget: ExecutionBudget::default(),
        source: source(text),
        artifact_kinds: BTreeSet::from([ArtifactKind::WorkflowDefinition]),
    }
}

fn spec_json() -> serde_json::Value {
    json!({
        "schema_version": WORKFLOW_SPEC_SCHEMA_VERSION,
        "workflow_id": "test.fake",
        "version": 1,
        "inputs": [{"name": "track", "schema": {"type": "id", "kind": "track"}}],
        "steps": [{"call": "emit", "id": "done", "output": "note", "value": "$input.track"}],
        "outputs": [{"name": "note", "schema": {"type": "id", "kind": "note"}}],
        "limits": {
            "max_steps": 100, "max_depth": 8, "max_iterations": 1000,
            "max_runtime_ms": 60000, "max_output_bytes": 1_048_576,
            "max_memory_bytes": 134_217_728
        }
    })
}

// ── registry resolution ───────────────────────────────────────────

#[test]
fn registry_resolves_unique_backend() {
    let mut registry = ScriptBackendRegistry::new();
    registry
        .register(Arc::new(FakeBackend::new("luau-mlua", "luau")))
        .unwrap();
    let backend = registry
        .resolve(&ScriptRequirement {
            language: Some(ScriptLanguageId::luau()),
            ..ScriptRequirement::default()
        })
        .unwrap();
    assert_eq!(backend.descriptor().id.0, "luau-mlua");
}

#[test]
fn registry_rejects_duplicate_backend_id() {
    let mut registry = ScriptBackendRegistry::new();
    registry
        .register(Arc::new(FakeBackend::new("luau-mlua", "luau")))
        .unwrap();
    let err = registry
        .register(Arc::new(FakeBackend::new("luau-mlua", "luau")))
        .err()
        .unwrap();
    assert!(matches!(
        err,
        synvelis_script::backend::RegistryError::DuplicateBackend { .. }
    ));
}

#[test]
fn registry_unknown_language_and_backend_are_typed() {
    let mut registry = ScriptBackendRegistry::new();
    registry
        .register(Arc::new(FakeBackend::new("luau-mlua", "luau")))
        .unwrap();
    let err = registry
        .resolve(&ScriptRequirement {
            language: Some(ScriptLanguageId("rhai".to_owned())),
            ..ScriptRequirement::default()
        })
        .err()
        .unwrap();
    assert_eq!(err.reason, ResolveErrorReason::UnknownLanguage);
    let err = registry
        .resolve(&ScriptRequirement {
            backend: Some(ScriptBackendId("nope".to_owned())),
            ..ScriptRequirement::default()
        })
        .err()
        .unwrap();
    assert_eq!(err.reason, ResolveErrorReason::UnknownBackend);
}

#[test]
fn registry_contract_version_and_capability_fail_closed() {
    let mut registry = ScriptBackendRegistry::new();
    registry
        .register(Arc::new(FakeBackend::new("luau-mlua", "luau")))
        .unwrap();
    let err = registry
        .resolve(&ScriptRequirement {
            language: Some(ScriptLanguageId::luau()),
            min_contract_version: Some(SCRIPT_CONTRACT_VERSION + 1),
            ..ScriptRequirement::default()
        })
        .err()
        .unwrap();
    assert_eq!(err.reason, ResolveErrorReason::UnsupportedContractVersion);
    let err = registry
        .resolve(&ScriptRequirement {
            language: Some(ScriptLanguageId::luau()),
            required_capabilities: BTreeSet::from(["budget:memory".to_owned()]),
            ..ScriptRequirement::default()
        })
        .err()
        .unwrap();
    assert_eq!(err.reason, ResolveErrorReason::MissingCapability);
    assert!(err.missing.contains(&"budget:memory".to_owned()));
}

#[test]
fn registry_ambiguous_backends_reject() {
    let mut registry = ScriptBackendRegistry::new();
    registry
        .register(Arc::new(FakeBackend::new("luau-a", "luau")))
        .unwrap();
    registry
        .register(Arc::new(FakeBackend::new("luau-b", "luau")))
        .unwrap();
    let err = registry
        .resolve(&ScriptRequirement {
            language: Some(ScriptLanguageId::luau()),
            ..ScriptRequirement::default()
        })
        .err()
        .unwrap();
    assert_eq!(err.reason, ResolveErrorReason::Ambiguous);
    // Explicit backend id disambiguates.
    let ok = registry
        .resolve(&ScriptRequirement {
            language: Some(ScriptLanguageId::luau()),
            backend: Some(ScriptBackendId("luau-b".to_owned())),
            ..ScriptRequirement::default()
        })
        .unwrap();
    assert_eq!(ok.descriptor().id.0, "luau-b");
}

#[test]
fn registry_artifact_kind_requirement_maps_to_capability() {
    let mut registry = ScriptBackendRegistry::new();
    registry
        .register(Arc::new(FakeBackend::new("luau-mlua", "luau")))
        .unwrap();
    registry
        .resolve(&ScriptRequirement {
            language: Some(ScriptLanguageId::luau()),
            artifact_kinds: BTreeSet::from([ArtifactKind::WorkflowDefinition]),
            ..ScriptRequirement::default()
        })
        .unwrap();
    let err = registry
        .resolve(&ScriptRequirement {
            language: Some(ScriptLanguageId::luau()),
            artifact_kinds: BTreeSet::from([ArtifactKind::ActionRecipe]),
            ..ScriptRequirement::default()
        })
        .err()
        .unwrap();
    assert_eq!(err.reason, ResolveErrorReason::MissingCapability);
}

// ── session / artifact path ───────────────────────────────────────

#[test]
fn session_evaluate_returns_typed_artifact_and_spec_decodes() {
    let payload = ScriptValue::from_json(&spec_json()).unwrap();
    let mut backend = FakeBackend::new("luau-mlua", "luau");
    backend.result = Some(ScriptResult {
        artifact: Some(ScriptArtifact {
            schema_version: SCRIPT_ARTIFACT_SCHEMA_VERSION,
            kind: ArtifactKind::WorkflowDefinition,
            digest: DefinitionDigest("test-digest".to_owned()),
            payload,
            source: ArtifactSource {
                language: ScriptLanguageId::luau(),
                backend_id: ScriptBackendId("luau-mlua".to_owned()),
                source_digest: SourceDigest("src".to_owned()),
                source_revision: SourceRevision(1),
            },
            produced_by: ProducedBy {
                backend_id: ScriptBackendId("luau-mlua".to_owned()),
                impl_version: "fake-1".to_owned(),
            },
        }),
        ..ScriptResult::default()
    });
    let mut session = backend.open_session(&session_config("spec()")).unwrap();
    let result = session.evaluate().unwrap();
    let artifact = result.artifact.expect("artifact present");
    assert!(matches!(artifact.kind, ArtifactKind::WorkflowDefinition));
    let spec = ScriptWorkflowSpec::from_script_value(&artifact.payload).unwrap();
    assert_eq!(spec.workflow_id, "test.fake");
    assert_eq!(spec.version, 1);
    assert_eq!(spec.steps.len(), 1);
    session.close();
    assert!(session.evaluate().is_err());
}

#[test]
fn session_open_rejects_source_digest_drift() {
    let backend = FakeBackend::new("luau-mlua", "luau");
    let mut config = session_config("real source");
    config.source.digest = SourceDigest("sha256:deadbeef".to_owned());
    let err = backend.open_session(&config).err().unwrap();
    assert!(matches!(err, SessionError::SourceDigestMismatch { .. }));
}

// ── ScriptValue codec ─────────────────────────────────────────────

#[test]
fn script_value_json_round_trip_preserves_int_float_split() {
    let value = json!({"i": 60, "f": 60.5, "s": "x", "b": true, "n": null, "a": [1, "two"]});
    let sv = ScriptValue::from_json(&value).unwrap();
    let back = sv.to_json().unwrap();
    assert_eq!(back, value);
    // Integer stays integer-typed (not silently widened to float).
    let ScriptValue::Record(rec) = &sv else {
        panic!("expected record")
    };
    assert!(matches!(rec["i"], ScriptValue::Integer(60)));
    assert!(matches!(rec["f"], ScriptValue::Number(f) if (f - 60.5).abs() < f64::EPSILON));
}

#[test]
fn script_value_rejects_unrepresentable_shapes() {
    // u64 beyond i64::MAX has no honest neutral form.
    let big = json!({"n": u64::MAX});
    assert!(matches!(
        ScriptValue::from_json(&big),
        Err(ScriptError {
            kind: ScriptErrorKind::ValueOutOfRange,
            ..
        })
    ));
    // Bytes/Time/ResourceId have no JSON form.
    for v in [
        ScriptValue::Bytes(vec![1]),
        ScriptValue::Time(7),
        ScriptValue::ResourceId("r".to_owned()),
        ScriptValue::Absent,
    ] {
        assert!(matches!(
            v.to_json(),
            Err(ScriptError {
                kind: ScriptErrorKind::ContractViolation { .. },
                ..
            })
        ));
    }
    // Absent record entries are omitted, not nulled.
    let mut map = BTreeMap::new();
    map.insert("keep".to_owned(), ScriptValue::Bool(true));
    map.insert("drop".to_owned(), ScriptValue::Absent);
    let json = ScriptValue::Record(map).to_json().unwrap();
    assert_eq!(json, json!({"keep": true}));
}

#[test]
fn spec_decode_rejects_wrong_version_and_non_json_payload() {
    let mut bad = spec_json();
    bad["schema_version"] = json!(99);
    let sv = ScriptValue::from_json(&bad).unwrap();
    assert!(matches!(
        ScriptWorkflowSpec::from_script_value(&sv),
        Err(ScriptApiError::Json { .. })
    ));
    assert!(matches!(
        ScriptWorkflowSpec::from_script_value(&ScriptValue::Bytes(vec![0])),
        Err(ScriptApiError::Json { .. })
    ));
}

#[test]
fn spec_decode_rejects_invalid_id_and_duplicate_step_ids() {
    // Evaluate-only consumers never reach WorkflowScriptBuilder —
    // from_script_value must enforce the same backend-independent
    // structural rules so an invalid spec is never reported valid.
    let mut bad_id = spec_json();
    bad_id["workflow_id"] = json!("BAD ID!");
    let sv = ScriptValue::from_json(&bad_id).unwrap();
    assert!(ScriptWorkflowSpec::from_script_value(&sv).is_err());

    let mut dup = spec_json();
    dup["steps"] = json!([
        {"call": "emit", "id": "done", "output": "a", "value": 1},
        {"call": "emit", "id": "done", "output": "b", "value": 2},
    ]);
    let sv = ScriptValue::from_json(&dup).unwrap();
    assert!(matches!(
        ScriptWorkflowSpec::from_script_value(&sv),
        Err(ScriptApiError::DuplicateStepId { .. })
    ));
}
