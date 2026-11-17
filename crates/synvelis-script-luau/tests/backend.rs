//! Real-VM tests for the `luau-mlua` backend — vendored Luau through
//! mlua, exercising the port contract end to end. These are the
//! real-VM golden/fail-closed matrix for synvelis#273; they are NOT
//! mocks.

use std::collections::BTreeSet;
use std::time::Duration;

use synvelis_script::backend::{
    ArtifactKind, BudgetKind, ExecutionBudget, SCRIPT_ARTIFACT_SCHEMA_VERSION, ScriptBackend,
    ScriptErrorKind, SessionConfig, SessionError, SourceDigest,
};
use synvelis_script::{
    ScriptLanguageId, ScriptWorkflowSpec, SourceRevision, WORKFLOW_SPEC_SCHEMA_VERSION, sha256_hex,
};
use synvelis_script_luau::LuauBackend;

fn backend() -> LuauBackend {
    LuauBackend::new()
}

fn source_of(text: &str) -> synvelis_script::backend::ScriptSource {
    synvelis_script::backend::ScriptSource {
        text: text.to_owned(),
        language: ScriptLanguageId::luau(),
        backend_hint: None,
        digest: SourceDigest(format!("sha256:{}", sha256_hex(text.as_bytes()))),
        revision: SourceRevision(0),
    }
}

fn config(text: &str) -> SessionConfig {
    SessionConfig {
        budget: ExecutionBudget::default(),
        source: source_of(text),
        artifact_kinds: BTreeSet::from([ArtifactKind::WorkflowDefinition]),
    }
}

fn eval(
    text: &str,
) -> Result<synvelis_script::backend::ScriptResult, synvelis_script::backend::ScriptError> {
    let backend = backend();
    let mut session = backend.open_session(&config(text)).expect("session opens");
    let result = session.evaluate();
    session.close();
    result
}

/// A spec-shaped Luau table — the shape a workflow script returns.
const SPEC_SOURCE: &str = r#"
return {
  schema_version = 1,
  workflow_id = "test.luau-golden",
  version = 1,
  inputs = {},
  steps = {
    { call = "action", id = "t", name = "queryTracks", args = { limit = 10 } },
  },
  outputs = {},
  limits = {
    max_steps = 100, max_depth = 8, max_iterations = 1000,
    max_runtime_ms = 60000, max_output_bytes = 1048576,
    max_memory_bytes = 134217728,
  },
}
"#;

#[test]
fn descriptor_is_contract_v1_luau() {
    let d = backend().descriptor().clone();
    assert_eq!(d.id.0, "luau-mlua");
    assert_eq!(d.language, ScriptLanguageId::luau());
    assert_eq!(d.version.contract_version, 1);
    assert_eq!(d.capabilities.max_source_bytes, Some(1 << 20));
    assert!(
        d.capabilities
            .artifact_kinds
            .contains(&ArtifactKind::WorkflowDefinition)
    );
    assert!(!d.capabilities.native_integer);
    assert!(d.capabilities.owner_thread_only);
    assert!(!d.capabilities.send && !d.capabilities.sync);
}

#[test]
fn open_rejects_language_mismatch() {
    let backend = backend();
    let mut cfg = config("return {}");
    cfg.source.language = ScriptLanguageId("rhai".to_owned());
    let err = backend.open_session(&cfg).err().expect("language mismatch");
    assert!(
        matches!(err, SessionError::LanguageMismatch { .. }),
        "{err:?}"
    );
}

#[test]
fn open_rejects_digest_drift() {
    let backend = backend();
    let mut cfg = config("return {}");
    cfg.source.digest = SourceDigest("sha256:deadbeef".to_owned());
    let err = backend.open_session(&cfg).err().expect("digest drift");
    assert!(
        matches!(err, SessionError::SourceDigestMismatch { .. }),
        "{err:?}"
    );
}

#[test]
fn open_rejects_unverifiable_digest_tag() {
    let backend = backend();
    let mut cfg = config("return {}");
    cfg.source.digest = SourceDigest("md5:whatever".to_owned());
    let err = backend
        .open_session(&cfg)
        .err()
        .expect("unknown digest tag");
    assert!(
        matches!(err, SessionError::SourceDigestMismatch { .. }),
        "{err:?}"
    );
}

#[test]
fn open_rejects_oversize_source() {
    let backend = backend();
    let big = " ".repeat((1 << 20) + 1);
    let mut cfg = config(&big);
    cfg.source.digest = SourceDigest(format!("sha256:{}", sha256_hex(big.as_bytes())));
    let err = backend.open_session(&cfg).err().expect("oversize");
    assert!(
        matches!(err, SessionError::SourceTooLarge { .. }),
        "{err:?}"
    );
}

#[test]
fn open_rejects_unenforceable_budgets() {
    let backend = backend();
    for (budget, kind) in [
        (
            ExecutionBudget {
                max_instructions: Some(1),
                ..ExecutionBudget::default()
            },
            BudgetKind::Instruction,
        ),
        (
            ExecutionBudget {
                max_host_calls: Some(1),
                ..ExecutionBudget::default()
            },
            BudgetKind::HostCall,
        ),
    ] {
        let mut cfg = config("return {}");
        cfg.budget = budget;
        let err = backend
            .open_session(&cfg)
            .err()
            .expect("budget unsupported");
        assert!(
            matches!(err, SessionError::UnsupportedBudget { kind: k } if k == kind),
            "{err:?}"
        );
    }
}

#[test]
fn open_rejects_unadvertised_artifact_and_multi_kind() {
    let backend = backend();
    let mut cfg = config("return {}");
    cfg.artifact_kinds = BTreeSet::from([ArtifactKind::ConsumerExtension {
        domain: "x".to_owned(),
    }]);
    let err = backend
        .open_session(&cfg)
        .err()
        .expect("artifact unsupported");
    assert!(
        matches!(err, SessionError::UnsupportedArtifact { .. }),
        "{err:?}"
    );

    let mut cfg = config("return {}");
    cfg.artifact_kinds =
        BTreeSet::from([ArtifactKind::WorkflowDefinition, ArtifactKind::ActionRecipe]);
    let err = backend.open_session(&cfg).err().expect("multi artifact");
    assert!(matches!(err, SessionError::Backend { .. }), "{err:?}");
}

#[test]
fn spec_shaped_source_produces_decodable_artifact() {
    let result = eval(SPEC_SOURCE).expect("spec evaluates");
    let artifact = result.artifact.expect("artifact produced");
    assert_eq!(artifact.schema_version, SCRIPT_ARTIFACT_SCHEMA_VERSION);
    assert!(matches!(artifact.kind, ArtifactKind::WorkflowDefinition));
    assert_eq!(artifact.source.language, ScriptLanguageId::luau());
    assert_eq!(artifact.produced_by.backend_id.0, "luau-mlua");

    // The artifact payload decodes as a real ScriptWorkflowSpec.
    let spec =
        ScriptWorkflowSpec::from_script_value(&artifact.payload).expect("payload decodes as spec");
    assert_eq!(spec.schema_version, WORKFLOW_SPEC_SCHEMA_VERSION);
    assert_eq!(spec.workflow_id, "test.luau-golden");
    assert_eq!(spec.version, 1);
    assert_eq!(spec.steps.len(), 1);
    // return_value mirrors the payload.
    assert_eq!(result.return_value, Some(artifact.payload));
}

#[test]
fn integer_and_number_lift_rules() {
    // Integral f64 lowers to Integer; fractional stays Number.
    let result = eval("return { a = 1, b = 1.5, c = 100000 }").expect("eval");
    let json = result.return_value.unwrap().to_json().unwrap();
    assert_eq!(json["a"], serde_json::json!(1));
    assert_eq!(json["b"], serde_json::json!(1.5));
    assert_eq!(json["c"], serde_json::json!(100_000));
}

#[test]
fn syntax_error_is_typed_parse() {
    let err = eval("return {").expect_err("syntax error");
    assert!(matches!(err.kind, ScriptErrorKind::Parse), "{err:?}");
}

#[test]
fn runtime_error_is_typed_runtime() {
    let err = eval("error('boom')").expect_err("runtime error");
    assert!(matches!(err.kind, ScriptErrorKind::Runtime), "{err:?}");
}

#[test]
fn infinite_loop_hits_wall_clock_budget() {
    let backend = backend();
    let mut cfg = config("while true do end");
    cfg.budget.max_wall_time = Some(Duration::from_millis(100));
    let mut session = backend.open_session(&cfg).expect("session opens");
    let err = session.evaluate().expect_err("wall clock");
    session.close();
    assert!(
        matches!(
            err.kind,
            ScriptErrorKind::BudgetExceeded {
                kind: Some(BudgetKind::WallClock)
            }
        ),
        "{err:?}"
    );
}

#[test]
fn budget_exceeded_kind_is_none_when_multiple_budgets_armed() {
    // The neutral contract error carries no per-dimension detail —
    // with wall-clock AND memory both armed the adapter must report
    // `None` rather than guess which budget fired.
    let backend = backend();
    let mut cfg = config("while true do local t = {} t[1] = 1 end");
    cfg.budget.max_wall_time = Some(Duration::from_millis(100));
    cfg.budget.max_memory_bytes = Some(64 * 1024 * 1024);
    let mut session = backend.open_session(&cfg).expect("session opens");
    let err = session.evaluate().expect_err("budget");
    session.close();
    assert!(
        matches!(err.kind, ScriptErrorKind::BudgetExceeded { kind: None }),
        "{err:?}"
    );
}

#[test]
fn memory_budget_is_enforced() {
    let backend = backend();
    let mut cfg = config("local t = {} for i = 1, 1e7 do t[i] = i end return t");
    cfg.budget.max_memory_bytes = Some(1024 * 1024);
    let mut session = backend.open_session(&cfg).expect("session opens");
    let err = session.evaluate().expect_err("memory");
    session.close();
    assert!(
        matches!(
            err.kind,
            ScriptErrorKind::BudgetExceeded {
                kind: Some(BudgetKind::Memory)
            }
        ),
        "{err:?}"
    );
}

#[test]
fn cancel_before_eval_is_typed() {
    let backend = backend();
    let mut session = backend.open_session(&config("return 1")).expect("opens");
    session.cancel();
    let err = session.evaluate().expect_err("cancelled");
    session.close();
    assert!(matches!(err.kind, ScriptErrorKind::Cancelled), "{err:?}");
}

#[test]
fn evaluate_after_close_fails() {
    let backend = backend();
    let mut session = backend.open_session(&config("return 1")).expect("opens");
    session.close();
    let err = session.evaluate().expect_err("closed");
    assert!(
        matches!(err.kind, ScriptErrorKind::Backend { .. }),
        "{err:?}"
    );
    // close is idempotent
    session.close();
}

#[test]
fn no_return_value_produces_no_artifact() {
    let result = eval("local x = 1").expect("eval");
    assert!(result.artifact.is_none());
    assert!(result.return_value.is_none());
}

#[test]
fn nil_return_produces_no_artifact() {
    let result = eval("return nil").expect("eval");
    assert!(result.artifact.is_none());
    assert!(result.return_value.is_none());
}

#[test]
fn multi_return_is_internal_error() {
    // The product backend classifies lifted-shape violations as
    // `Internal` (the neutral contract has no ContractViolation
    // kind); the adapter maps that 1:1.
    let err = eval("return 1, 2").expect_err("multi return");
    assert!(
        matches!(err.kind, ScriptErrorKind::Internal { .. }),
        "{err:?}"
    );
}

#[test]
fn mixed_table_keys_are_internal_error() {
    let err = eval("return { a = 1, [2] = 'x' }").expect_err("mixed keys");
    assert!(
        matches!(err.kind, ScriptErrorKind::Internal { .. }),
        "{err:?}"
    );
}

#[test]
fn sparse_integer_table_is_internal_error() {
    let err = eval("return { [1] = 'a', [3] = 'c' }").expect_err("sparse");
    assert!(
        matches!(err.kind, ScriptErrorKind::Internal { .. }),
        "{err:?}"
    );
}

#[test]
fn non_finite_number_is_value_out_of_range() {
    // Record-wrapped so the value reaches the binding's conversion —
    // a bare `0/0` is a non-record artifact payload (Internal)
    // upstream; nested inside the spec record the binding still
    // rejects the non-finite float as ValueOutOfRange.
    let err = eval("return { nan = 0/0 }").expect_err("nan");
    assert!(
        matches!(err.kind, ScriptErrorKind::ValueOutOfRange),
        "{err:?}"
    );
}

#[test]
fn unsupported_value_type_is_typed() {
    let err = eval("return function() end").expect_err("function value");
    assert!(
        matches!(err.kind, ScriptErrorKind::Unsupported { .. }),
        "{err:?}"
    );
}

#[test]
fn sessions_are_independent() {
    // Two sessions on one backend share no VM state.
    let backend = backend();
    let mut a = backend.open_session(&config("x = 41")).expect("a opens");
    a.evaluate().expect("a evals");
    let mut b = backend
        .open_session(&config("return { v = (x or 0) + 1 }"))
        .expect("b opens");
    // `x` set in session a must not leak into session b.
    let result_b = b.evaluate().expect("b evals");
    let value = result_b.return_value.expect("b returns");
    assert_eq!(value.to_json().unwrap(), serde_json::json!({ "v": 1 }));
    a.close();
    b.close();
}

#[test]
fn sandbox_strips_escape_globals() {
    // `require`/`loadstring` would run source outside the digest pin,
    // `print` corrupts host stdout, `debug` leaks internals — all
    // must be unreachable inside the VM.
    let result = eval(
        "return { req = type(require), ls = type(loadstring), \
         pr = type(print), dbg = type(debug) }",
    )
    .expect("eval");
    let json = result.return_value.unwrap().to_json().unwrap();
    for key in ["req", "ls", "pr", "dbg"] {
        assert_eq!(json[key], serde_json::json!("nil"), "{key}");
    }
}

#[test]
fn shared_subtable_dag_hits_lift_node_budget() {
    // `{a=s,b=s}` chains share subtables — naive lifting enumerates
    // 2^n leaf paths inside Rust code where no VM safepoint fires.
    // The node budget must stop it as a typed failure (the product
    // backend classifies lift-bound violations as `Internal`).
    let err = eval("local s = {} for i = 1, 21 do s = { a = s, b = s } end return s")
        .expect_err("dag lift");
    assert!(
        matches!(err.kind, ScriptErrorKind::Internal { .. }),
        "{err:?}"
    );
}

#[test]
fn evaluate_is_single_shot() {
    // Record-shaped return — the product contract requires artifact
    // payloads to be records.
    let backend = backend();
    let mut session = backend
        .open_session(&config("return { ok = true }"))
        .expect("opens");
    session.evaluate().expect("first eval");
    let err = session.evaluate().expect_err("second eval");
    session.close();
    assert!(
        matches!(err.kind, ScriptErrorKind::ContractViolation { .. }),
        "{err:?}"
    );
}

#[test]
fn zero_memory_budget_is_rejected_not_unlimited() {
    // mlua maps a 0 limit to "unlimited" — a zero-byte budget must
    // fail closed at open, never silently disable the memory bound.
    let backend = backend();
    let mut cfg = config("return {}");
    cfg.budget.max_memory_bytes = Some(0);
    let err = backend.open_session(&cfg).err().expect("zero memory");
    assert!(
        matches!(
            err,
            SessionError::UnsupportedBudget {
                kind: BudgetKind::Memory
            }
        ),
        "{err:?}"
    );
}

#[test]
fn overflowing_wall_clock_budget_is_typed() {
    let backend = backend();
    let mut cfg = config("return {}");
    cfg.budget.max_wall_time = Some(Duration::MAX);
    let err = backend.open_session(&cfg).err().expect("huge wall clock");
    assert!(
        matches!(
            err,
            SessionError::UnsupportedBudget {
                kind: BudgetKind::WallClock
            }
        ),
        "{err:?}"
    );
}

#[test]
fn pcall_cannot_swallow_wall_clock_budget() {
    // The interrupt error is catchable by `pcall`, but the next
    // safepoint outside it re-fires — a swallowed interrupt can
    // never reach `return` and surface as success.
    let backend = backend();
    let mut cfg = config("local ok = pcall(function() while true do end end) return { ok = ok }");
    cfg.budget.max_wall_time = Some(Duration::from_millis(100));
    let mut session = backend.open_session(&cfg).expect("session opens");
    let err = session.evaluate().expect_err("wall clock");
    session.close();
    assert!(
        matches!(
            err.kind,
            ScriptErrorKind::BudgetExceeded {
                kind: Some(BudgetKind::WallClock)
            }
        ),
        "{err:?}"
    );
}
