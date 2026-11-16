//! `luau-mlua` — interim Luau script backend for synvelis#273.
//!
//! Implements the language-neutral [`ScriptBackend`]/[`ScriptSession`]
//! port from `synvelis-script` on top of `mlua`'s vendored Luau VM
//! (`luau` feature → `luau0-src`). This is the **only** crate allowed
//! to depend on `mlua`; every consumer goes through the port trait so
//! the backend stays swappable for the ruviform production port once
//! RF#559 lands.
//!
//! Contract notes:
//!
//! - One fresh `Lua` state per session — no shared VM state, no
//!   persistence across `open_session` calls. `evaluate` is
//!   single-shot: a second call is a typed contract violation.
//! - `evaluate` runs the chunk exactly once (text mode only —
//!   bytecode is rejected) and returns the first (and only) return
//!   value as both `return_value` and, when the consumer requested
//!   an artifact kind, the artifact payload.
//! - No host functions are injected: scripts see only the pure Luau
//!   standard library and must return a spec-shaped table. mlua's
//!   `configure_luau` unconditionally installs `require` /
//!   `loadstring` (filesystem code execution outside the digest
//!   pin), `print` (host stdout corruption), and `debug` — all four
//!   globals are stripped before the session is returned. Name →
//!   capability resolution stays in `WorkflowScriptBuilder`, never
//!   in the VM.
//! - Safepoint budgets: `max_wall_time` is enforced by the Luau
//!   interrupt callback (checked again after eval so a `pcall`-
//!   swallowed interrupt can never surface as success),
//!   `max_memory_bytes` by `Lua::set_memory_limit`,
//!   `max_output_bytes` by a post-eval serialized-size check, and
//!   lifted-value size by [`MAX_LIFT_NODES`] (shared-subtable DAGs
//!   are bounded in Rust code where no safepoint can fire).
//!   `max_instructions`/`max_host_calls` are **not** enforceable
//!   here and are rejected with [`SessionError::UnsupportedBudget`].
//! - Sessions are owner-thread-only (`mlua` without `send`): the Lua
//!   state is created and used on the `open_session` caller's thread.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::time::Instant;

use mlua::{Lua, MultiValue, Value as LuaValue, VmState};
use synvelis_script::backend::{
    ArtifactKind, ArtifactSource, BackendCapabilities, BackendDescriptor, BudgetKind,
    CancelGranularity, DefinitionDigest, ProducedBy, SCRIPT_ARTIFACT_SCHEMA_VERSION,
    SCRIPT_CONTRACT_VERSION, ScriptArtifact, ScriptBackend, ScriptBackendId, ScriptError,
    ScriptErrorKind, ScriptResult, ScriptSession, ScriptValue, SessionConfig, SessionError,
    SourceDigest,
};
use synvelis_script::{ScriptBackendVersion, ScriptLanguageId, sha256_hex};

/// Maximum nesting depth accepted when lifting a Lua table into the
/// neutral [`ScriptValue`] model — deeper structures are a typed
/// contract violation, never a stack overflow.
const MAX_VALUE_DEPTH: usize = 64;

/// Maximum source size this backend accepts — the RF-SCRIPT-00 `PoC`
/// contract ceiling of `1 MiB`.
const MAX_SOURCE_BYTES: u64 = 1 << 20;

/// Maximum `ScriptValue` nodes produced by one `lift` — bounds the
/// work a shared-subtable DAG can force. Depth alone cannot: a
/// depth-50 `{a=s,b=s}` chain enumerates 2^50 leaf paths inside
/// Rust code where no VM safepoint or memory limit can fire.
const MAX_LIFT_NODES: usize = 1_000_000;

/// Globals mlua's `configure_luau` installs that must not reach
/// script code: `require`/`loadstring` execute source outside the
/// digest pin, `print` corrupts the host's stdout contract, and
/// `debug` exposes internals no declarative spec needs.
const STRIPPED_GLOBALS: [&str; 4] = ["require", "loadstring", "print", "debug"];

/// Process-wide session id counter.
static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

/// Interrupt-state flags shared between the session and the Luau
/// interrupt callback.
#[derive(Default)]
struct InterruptState {
    /// Set by [`LuauSession::cancel`].
    cancelled: AtomicBool,
    /// Set by the interrupt callback: 0 = none, 1 = cancelled,
    /// 2 = wall-clock deadline exceeded.
    hit: AtomicU8,
}

/// The `luau-mlua` backend — stateless; every session owns a fresh
/// Luau VM.
#[derive(Debug)]
pub struct LuauBackend {
    descriptor: BackendDescriptor,
}

impl LuauBackend {
    /// Creates the backend with the contract-v1 `luau-mlua` descriptor.
    #[must_use]
    pub fn new() -> Self {
        Self {
            descriptor: BackendDescriptor {
                id: ScriptBackendId("luau-mlua".to_owned()),
                language: ScriptLanguageId::luau(),
                version: ScriptBackendVersion {
                    impl_version: env!("CARGO_PKG_VERSION").to_owned(),
                    contract_version: SCRIPT_CONTRACT_VERSION,
                },
                capabilities: BackendCapabilities {
                    enforced_budgets: BTreeSet::from([
                        BudgetKind::WallClock,
                        BudgetKind::Memory,
                        BudgetKind::Output,
                    ]),
                    max_source_bytes: Some(MAX_SOURCE_BYTES),
                    artifact_kinds: BTreeSet::from([
                        ArtifactKind::WorkflowDefinition,
                        ArtifactKind::ActionRecipe,
                    ]),
                    send: false,
                    sync: false,
                    owner_thread_only: true,
                    cancel_granularity: CancelGranularity::Safepoint,
                    native_integer: false,
                },
            },
        }
    }
}

impl Default for LuauBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl LuauBackend {
    /// Creates a fresh sandboxed Luau VM for one session.
    ///
    /// `Lua::new_with` (not `Lua::new`) so state-creation failure is
    /// a typed [`SessionError`] instead of mlua's internal `expect`
    /// panic; `catch_unwind` covers the remaining `mlua_expect`
    /// paths inside state setup. [`STRIPPED_GLOBALS`] are then
    /// removed — mlua's `configure_luau` installs them
    /// unconditionally and they would let script source escape the
    /// source-digest pin (`require`/`loadstring`) or corrupt the
    /// host stdout contract (`print`).
    fn new_vm() -> Result<Lua, SessionError> {
        let lua = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            Lua::new_with(mlua::StdLib::ALL_SAFE, mlua::LuaOptions::default())
        }))
        .map_err(|_| SessionError::Backend {
            reason: "failed to create Luau state".to_owned(),
        })?
        .map_err(|e| SessionError::Backend {
            reason: format!("failed to create Luau state: {e}"),
        })?;
        let globals = lua.globals();
        for name in STRIPPED_GLOBALS {
            globals
                .raw_set(name, LuaValue::Nil)
                .map_err(|e| SessionError::Backend {
                    reason: format!("failed to strip global `{name}`: {e}"),
                })?;
        }
        Ok(lua)
    }
}

impl ScriptBackend for LuauBackend {
    fn descriptor(&self) -> &BackendDescriptor {
        &self.descriptor
    }

    fn open_session(&self, config: &SessionConfig) -> Result<Box<dyn ScriptSession>, SessionError> {
        if self.descriptor.language != config.source.language {
            return Err(SessionError::LanguageMismatch {
                expected: self.descriptor.language.clone(),
                actual: config.source.language.clone(),
            });
        }
        let size = config.source.text.len() as u64;
        if size > MAX_SOURCE_BYTES {
            return Err(SessionError::SourceTooLarge {
                size,
                limit: MAX_SOURCE_BYTES,
            });
        }
        // Digest contract: only the `sha256:<hex>` tag is verifiable.
        let actual = format!("sha256:{}", sha256_hex(config.source.text.as_bytes()));
        let verified = config
            .source
            .digest
            .0
            .strip_prefix("sha256:")
            .is_some_and(|hex| hex == &actual[7..]);
        if !verified {
            return Err(SessionError::SourceDigestMismatch {
                expected: config.source.digest.clone(),
                actual: SourceDigest(actual),
            });
        }
        // Every requested budget kind must be one this backend enforces.
        for kind in config.budget.requested_kinds() {
            if !self
                .descriptor
                .capabilities
                .enforced_budgets
                .contains(&kind)
            {
                return Err(SessionError::UnsupportedBudget { kind });
            }
        }
        // One artifact per session — the payload kind is consumer-declared.
        if config.artifact_kinds.len() > 1 {
            return Err(SessionError::Backend {
                reason: "at most one artifact kind may be requested per session".to_owned(),
            });
        }
        for kind in &config.artifact_kinds {
            if !self.descriptor.capabilities.artifact_kinds.contains(kind) {
                return Err(SessionError::UnsupportedArtifact { kind: kind.clone() });
            }
        }

        let lua = Self::new_vm()?;
        if let Some(limit) = config.budget.max_memory_bytes {
            // mlua maps 0 to "unlimited" — a zero-byte budget would
            // silently fail open, so it is rejected instead.
            if limit == 0 {
                return Err(SessionError::UnsupportedBudget {
                    kind: BudgetKind::Memory,
                });
            }
            let limit = usize::try_from(limit).map_err(|_| SessionError::UnsupportedBudget {
                kind: BudgetKind::Memory,
            })?;
            lua.set_memory_limit(limit)
                .map_err(|e| SessionError::Backend {
                    reason: format!("failed to set memory limit: {e}"),
                })?;
        }

        let state = Arc::new(InterruptState::default());
        // `Instant + Duration` panics on overflow — a huge budget is a
        // typed rejection, never a panic at the port boundary.
        let deadline = config
            .budget
            .max_wall_time
            .map(|d| {
                Instant::now()
                    .checked_add(d)
                    .ok_or(SessionError::UnsupportedBudget {
                        kind: BudgetKind::WallClock,
                    })
            })
            .transpose()?;
        lua.set_interrupt({
            let state = Arc::clone(&state);
            move |_lua| {
                if state.cancelled.load(Ordering::Relaxed) {
                    state.hit.store(1, Ordering::Relaxed);
                    return Err(mlua::Error::external("synvelis: evaluation cancelled"));
                }
                if deadline.is_some_and(|dl| Instant::now() >= dl) {
                    state.hit.store(2, Ordering::Relaxed);
                    return Err(mlua::Error::external(
                        "synvelis: wall-clock budget exceeded",
                    ));
                }
                Ok(VmState::Continue)
            }
        });

        let session_id = format!("luau-mlua-{}", NEXT_SESSION.fetch_add(1, Ordering::Relaxed));
        Ok(Box::new(LuauSession {
            session_id,
            backend_id: self.descriptor.id.clone(),
            lua: Some(lua),
            config: config.clone(),
            state,
            artifact_kind: config.artifact_kinds.iter().next().cloned(),
            closed: false,
            evaluated: false,
        }))
    }
}

/// A live Luau evaluation session — owns its `Lua` state until
/// [`ScriptSession::close`].
struct LuauSession {
    session_id: String,
    backend_id: ScriptBackendId,
    lua: Option<Lua>,
    config: SessionConfig,
    state: Arc<InterruptState>,
    artifact_kind: Option<ArtifactKind>,
    closed: bool,
    evaluated: bool,
}

impl LuauSession {
    /// Lifts an `mlua::Value` into the closed [`ScriptValue`] model.
    /// `nodes` is the remaining [`MAX_LIFT_NODES`] budget — every
    /// table element visit consumes one, which bounds the total work
    /// a shared-subtable DAG can force inside Rust code.
    // `f as i64` below is guarded by the fract/range check.
    #[allow(clippy::cast_possible_truncation)]
    fn lift(value: LuaValue, depth: usize, nodes: &mut usize) -> Result<ScriptValue, ScriptError> {
        if depth > MAX_VALUE_DEPTH {
            return Err(ScriptError::new(
                ScriptErrorKind::ContractViolation {
                    detail: format!("value nesting exceeds {MAX_VALUE_DEPTH}"),
                },
                "value nesting too deep",
            ));
        }
        Ok(match value {
            LuaValue::Nil => ScriptValue::Null,
            LuaValue::Boolean(b) => ScriptValue::Bool(b),
            LuaValue::Integer(i) => ScriptValue::Integer(i),
            LuaValue::Number(f) => {
                if !f.is_finite() {
                    return Err(ScriptError::new(
                        ScriptErrorKind::ValueOutOfRange,
                        "non-finite Luau number",
                    ));
                }
                // Luau has a single f64 number type; integral values in
                // i64 range lower to the exact-integer variant so spec
                // fields like `version: 1` decode identically to YAML.
                if f.fract() == 0.0
                    && (-9.223_372_036_854_776e18..9.223_372_036_854_776e18).contains(&f)
                {
                    ScriptValue::Integer(f as i64)
                } else {
                    ScriptValue::Number(f)
                }
            }
            LuaValue::String(s) => ScriptValue::String(
                s.to_str()
                    .map_err(|_| {
                        ScriptError::new(
                            ScriptErrorKind::ContractViolation {
                                detail: "non-UTF-8 Luau string".to_owned(),
                            },
                            "non-UTF-8 Luau string",
                        )
                    })?
                    .to_owned(),
            ),
            LuaValue::Table(t) => Self::lift_table(&t, depth, nodes)?,
            other => {
                return Err(ScriptError::new(
                    ScriptErrorKind::Unsupported {
                        what: format!("Luau value type {other:?}"),
                    },
                    "unsupported Luau value type",
                ));
            }
        })
    }

    /// Lifts a Lua table: all-string keys → record, consecutive
    /// integer keys `1..=n` → array, empty → record. Mixed or sparse
    /// shapes have no honest neutral form and are rejected.
    fn lift_table(
        table: &mlua::Table,
        depth: usize,
        nodes: &mut usize,
    ) -> Result<ScriptValue, ScriptError> {
        let mut strings: BTreeMap<String, ScriptValue> = BTreeMap::new();
        let mut ints: Vec<(i64, ScriptValue)> = Vec::new();
        for pair in table.clone().pairs::<LuaValue, LuaValue>() {
            *nodes = nodes.checked_sub(1).ok_or_else(|| {
                ScriptError::new(
                    ScriptErrorKind::ContractViolation {
                        detail: format!("lifted value exceeds {MAX_LIFT_NODES} nodes"),
                    },
                    "lifted value too large",
                )
            })?;
            let (key, val) = pair.map_err(|e| {
                ScriptError::new(ScriptErrorKind::Runtime, format!("table iteration: {e}"))
            })?;
            let lifted = Self::lift(val, depth + 1, nodes)?;
            match key {
                LuaValue::String(s) => {
                    let name = s.to_str().map_err(|_| {
                        ScriptError::new(
                            ScriptErrorKind::ContractViolation {
                                detail: "non-UTF-8 table key".to_owned(),
                            },
                            "non-UTF-8 table key",
                        )
                    })?;
                    strings.insert(name.to_owned(), lifted);
                }
                LuaValue::Integer(i) => ints.push((i, lifted)),
                other => {
                    return Err(ScriptError::new(
                        ScriptErrorKind::ContractViolation {
                            detail: format!("unsupported table key type {other:?}"),
                        },
                        "unsupported table key type",
                    ));
                }
            }
        }
        if strings.is_empty() && !ints.is_empty() {
            ints.sort_by_key(|(k, _)| *k);
            let is_sequence = ints.first().is_some_and(|(k, _)| *k == 1)
                && ints.windows(2).all(|w| w[1].0 == w[0].0 + 1);
            if is_sequence {
                return Ok(ScriptValue::Array(
                    ints.into_iter().map(|(_, v)| v).collect(),
                ));
            }
            return Err(ScriptError::new(
                ScriptErrorKind::ContractViolation {
                    detail: "sparse integer-keyed table has no array form".to_owned(),
                },
                "sparse integer-keyed table",
            ));
        }
        if !strings.is_empty() && !ints.is_empty() {
            return Err(ScriptError::new(
                ScriptErrorKind::ContractViolation {
                    detail: "mixed string/integer keys have no record form".to_owned(),
                },
                "mixed table keys",
            ));
        }
        Ok(ScriptValue::Record(strings))
    }

    /// Maps an interrupt flag to the typed error it represents, if any.
    fn interrupt_error(&self) -> Option<ScriptError> {
        match self.state.hit.load(Ordering::Relaxed) {
            1 => Some(ScriptError::new(
                ScriptErrorKind::Cancelled,
                "evaluation cancelled",
            )),
            2 => Some(ScriptError::new(
                ScriptErrorKind::BudgetExceeded {
                    kind: BudgetKind::WallClock,
                },
                "wall-clock budget exceeded",
            )),
            _ => None,
        }
    }

    /// Maps an `mlua::Error` to the neutral kind, honoring interrupt
    /// flags first so budget/cancel wins over the wrapper error.
    fn map_eval_error(&self, err: &mlua::Error) -> ScriptError {
        if let Some(error) = self.interrupt_error() {
            return error;
        }
        let kind = match err {
            mlua::Error::SyntaxError { .. } => ScriptErrorKind::Parse,
            mlua::Error::MemoryError(_) => ScriptErrorKind::BudgetExceeded {
                kind: BudgetKind::Memory,
            },
            mlua::Error::RuntimeError(_) => ScriptErrorKind::Runtime,
            other => ScriptErrorKind::Backend {
                detail: other.to_string(),
            },
        };
        let mut error = ScriptError::new(kind, err.to_string());
        error.backend_id = Some(self.backend_id.clone());
        error.session_id = Some(self.session_id.clone());
        error
    }
}

impl ScriptSession for LuauSession {
    fn session_id(&self) -> &str {
        &self.session_id
    }

    fn backend_id(&self) -> &ScriptBackendId {
        &self.backend_id
    }

    fn evaluate(&mut self) -> Result<ScriptResult, ScriptError> {
        if self.closed {
            return Err(ScriptError::new(
                ScriptErrorKind::Backend {
                    detail: "session is closed".to_owned(),
                },
                "evaluate after close",
            ));
        }
        if self.state.cancelled.load(Ordering::Relaxed) {
            return Err(ScriptError::new(
                ScriptErrorKind::Cancelled,
                "evaluation cancelled",
            ));
        }
        // The port contract is evaluate-once: a second run would see
        // mutated globals and a possibly stale interrupt flag, so the
        // session is spent after the first attempt — success or not.
        if self.evaluated {
            return Err(ScriptError::new(
                ScriptErrorKind::ContractViolation {
                    detail: "session already evaluated".to_owned(),
                },
                "evaluate called twice",
            ));
        }
        self.evaluated = true;
        let lua = self.lua.as_ref().expect("session holds its Lua state");
        let values = lua
            .load(self.config.source.text.as_str())
            .set_name("workflow.luau")
            // Text-only: a bytecode-magic source must not reach the VM.
            .set_mode(mlua::chunk::ChunkMode::Text)
            .eval::<MultiValue>()
            .map_err(|e| self.map_eval_error(&e))?;
        // Defense in depth: an interrupt that fired but was swallowed
        // by script `pcall` must never surface as success.
        if let Some(error) = self.interrupt_error() {
            return Err(error);
        }
        self.lower_result(values)
    }

    fn cancel(&mut self) {
        self.state.cancelled.store(true, Ordering::Relaxed);
    }

    fn close(&mut self) {
        self.closed = true;
        self.lua = None;
    }
}

impl LuauSession {
    /// Lowers the evaluated `MultiValue` into a [`ScriptResult`] —
    /// artifact payload, output budget, and digest provenance.
    fn lower_result(&self, values: MultiValue) -> Result<ScriptResult, ScriptError> {
        if values.is_empty() {
            return Ok(ScriptResult::default());
        }
        if values.len() > 1 {
            return Err(ScriptError::new(
                ScriptErrorKind::ContractViolation {
                    detail: format!(
                        "script returned {} values; exactly one is allowed",
                        values.len()
                    ),
                },
                "script must return at most one value",
            ));
        }
        let mut nodes = MAX_LIFT_NODES;
        let value = Self::lift(
            values.into_iter().next().unwrap_or(LuaValue::Nil),
            0,
            &mut nodes,
        )?;
        if matches!(value, ScriptValue::Null) {
            // `return nil` declares no artifact and no return value.
            return Ok(ScriptResult::default());
        }

        // Serialized-output budget — the artifact payload is the
        // largest thing that crosses the boundary.
        let json = value.to_json()?;
        if let Some(limit) = self.config.budget.max_output_bytes {
            let size = serde_json::to_string(&json)
                .map_err(|e| {
                    ScriptError::new(
                        ScriptErrorKind::Backend {
                            detail: format!("payload serialization: {e}"),
                        },
                        "payload serialization failed",
                    )
                })?
                .len() as u64;
            if size > limit {
                return Err(ScriptError::new(
                    ScriptErrorKind::BudgetExceeded {
                        kind: BudgetKind::Output,
                    },
                    format!("output size {size} exceeds budget {limit}"),
                ));
            }
        }

        let artifact = match self.artifact_kind.clone() {
            Some(kind) => {
                let digest = synvelis_script::digest_of(&json).map_err(|e| {
                    ScriptError::new(
                        ScriptErrorKind::Backend {
                            detail: format!("artifact digest: {e}"),
                        },
                        "artifact digest failed",
                    )
                })?;
                Some(ScriptArtifact {
                    schema_version: SCRIPT_ARTIFACT_SCHEMA_VERSION,
                    kind,
                    digest: DefinitionDigest(digest),
                    payload: value.clone(),
                    source: ArtifactSource {
                        language: self.config.source.language.clone(),
                        backend_id: self.backend_id.clone(),
                        source_digest: self.config.source.digest.clone(),
                        source_revision: self.config.source.revision,
                    },
                    produced_by: ProducedBy {
                        backend_id: self.backend_id.clone(),
                        impl_version: env!("CARGO_PKG_VERSION").to_owned(),
                    },
                })
            }
            None => None,
        };
        Ok(ScriptResult {
            artifact,
            return_value: Some(value),
            diagnostics: Vec::new(),
        })
    }
}
