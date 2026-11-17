//! `luau-mlua` — synvelis binding over the ruviform product port
//! (synvelis#516).
//!
//! This crate no longer contains a VM implementation. It adapts the
//! language-neutral [`ScriptBackend`]/[`ScriptSession`] port from
//! `synvelis-script` onto `ruviform_script_luau::LuauBackend` — the
//! product Luau backend implementing the RF-SCRIPT-00 neutral
//! contract (`ruviform-script`). The vendored Luau VM, sandbox
//! globals, budget enforcement, and artifact provenance now live
//! upstream in `ruviform-script-luau` (mlua stays isolated there —
//! no synvelis crate depends on mlua directly).
//!
//! This crate is the anti-corruption boundary. It owns:
//!
//! - the synvelis-side source digest tag (`sha256:<hex>` — the
//!   product contract uses `sha256:v1:<hex>`),
//! - the synvelis `SessionError`/`ScriptErrorKind` vocabulary,
//!   including [`ScriptErrorKind::ContractViolation`] and
//!   [`ScriptErrorKind::ValueOutOfRange`] which the neutral contract
//!   does not carry,
//! - artifact digests recomputed under the synvelis canonical scheme
//!   (`digest_of(to_json(payload))`), identical to the interim
//!   backend's formula,
//! - fail-closed open-time validation in the interim order
//!   (language → size → digest → budgets → artifact kinds) so the
//!   typed [`SessionError`] precedence consumers observe is
//!   unchanged.
//!
//! Contract notes:
//!
//! - One fresh `Lua` state per session upstream — no shared VM
//!   state. `evaluate` is single-shot; the adapter preserves the
//!   interim classification (`ContractViolation`) for a second call
//!   and `Backend` for evaluate-after-close before delegating.
//! - No host functions are injected: the neutral contract carries
//!   host binding *descriptors* only (no handler seam), and the
//!   synvelis [`SessionConfig`] has no host surface field, so every
//!   session opens with [`rf::HostApiSurface::empty()`].
//! - `max_instructions` and `max_host_calls` remain
//!   [`SessionError::UnsupportedBudget`] — the product backend
//!   rejects the former and has no host-call path for the latter;
//!   the adapter rejects both up front because the synvelis port
//!   requires every *requested* budget kind to be enforced, not
//!   vacuously satisfied.
//! - [`ScriptErrorKind::BudgetExceeded`]'s `kind` is `Some` only
//!   when exactly one enforceable budget dimension was requested —
//!   the neutral `ScriptError` carries no per-dimension detail, so
//!   multi-budget sessions report `None` rather than guess.
//! - The product backend classifies lifted-shape violations
//!   (multi-return, sparse/mixed tables, depth/node limits,
//!   non-record artifact payloads) as
//!   [`rf::ScriptErrorKind::Internal`]; the adapter maps that
//!   1:1 to [`ScriptErrorKind::Internal`]. [`ScriptErrorKind::ContractViolation`]
//!   is reserved for violations the binding itself detects
//!   (domain-qualified values, unknown variants).
//! - Sessions remain owner-thread-only (mlua without `send`).

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::sync::atomic::{AtomicU64, Ordering};

use ruviform_script as rf;
use ruviform_script_luau::LuauBackend as ProductBackend;
use synvelis_script::backend::{
    ArtifactKind, ArtifactSource, BackendCapabilities, BackendDescriptor, BudgetKind,
    CancelGranularity, DefinitionDigest, ExecutionBudget, ProducedBy,
    SCRIPT_ARTIFACT_SCHEMA_VERSION, SCRIPT_CONTRACT_VERSION, ScriptArtifact, ScriptBackend,
    ScriptBackendId, ScriptDiagnostic, ScriptError, ScriptErrorKind, ScriptResult, ScriptSession,
    ScriptValue, SessionConfig, SessionError, SourceDigest,
};
use synvelis_script::{
    DiagnosticSeverity, ScriptBackendVersion, ScriptLanguageId, SourcePosition, SourceRevision,
};

/// Maximum source size this backend accepts — the RF-SCRIPT-00 `PoC`
/// contract ceiling of `1 MiB` (same ceiling the product backend
/// enforces; the adapter checks it first so [`SessionError::SourceTooLarge`]
/// keeps the synvelis field names).
const MAX_SOURCE_BYTES: u64 = 1 << 20;

/// Process-wide session id counter — keeps the interim
/// `luau-mlua-<n>` session id format consumers may observe.
static NEXT_SESSION: AtomicU64 = AtomicU64::new(1);

/// The `luau-mlua` backend — a synvelis-port facade over the
/// ruviform product backend. Stateless; every session owns a fresh
/// Luau VM upstream.
#[derive(Debug)]
pub struct LuauBackend {
    descriptor: BackendDescriptor,
    product: ProductBackend,
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
            product: ProductBackend::new(),
        }
    }

    /// Validates `config.artifact_kinds` against the advertised set
    /// and translates to the neutral vocabulary — one artifact per
    /// session, unknown future kinds fail closed.
    fn product_artifact_kinds(
        &self,
        config: &SessionConfig,
    ) -> Result<BTreeSet<rf::ArtifactKind>, SessionError> {
        if config.artifact_kinds.len() > 1 {
            return Err(SessionError::Backend {
                reason: "at most one artifact kind may be requested per session".to_owned(),
            });
        }
        let mut kinds = BTreeSet::new();
        for kind in &config.artifact_kinds {
            if !self.descriptor.capabilities.artifact_kinds.contains(kind) {
                return Err(SessionError::UnsupportedArtifact { kind: kind.clone() });
            }
            kinds.insert(match kind {
                ArtifactKind::ActionRecipe => rf::ArtifactKind::ActionRecipe,
                ArtifactKind::WorkflowDefinition => rf::ArtifactKind::WorkflowDefinition,
                ArtifactKind::ConsumerExtension { domain } => rf::ArtifactKind::ConsumerExtension {
                    domain: domain.clone(),
                },
                _ => {
                    return Err(SessionError::UnsupportedArtifact { kind: kind.clone() });
                }
            });
        }
        Ok(kinds)
    }
}

impl Default for LuauBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// Budget kinds the product backend actually enforces — the only
/// dimensions a [`ScriptErrorKind::BudgetExceeded`] may be
/// attributed to.
fn enforced_budget_kinds(budget: &ExecutionBudget) -> BTreeSet<BudgetKind> {
    let mut kinds = BTreeSet::new();
    if budget.max_wall_time.is_some() {
        kinds.insert(BudgetKind::WallClock);
    }
    if budget.max_memory_bytes.is_some() {
        kinds.insert(BudgetKind::Memory);
    }
    if budget.max_output_bytes.is_some() {
        kinds.insert(BudgetKind::Output);
    }
    kinds
}

/// Translates a ruviform [`rf::BudgetKind`] back into the synvelis
/// vocabulary. Returns `None` for kinds the synvelis port cannot
/// name — callers must not guess.
fn budget_kind_from_rf(kind: rf::BudgetKind) -> Option<BudgetKind> {
    Some(match kind {
        rf::BudgetKind::Instructions => BudgetKind::Instruction,
        rf::BudgetKind::WallTime => BudgetKind::WallClock,
        rf::BudgetKind::MemoryBytes => BudgetKind::Memory,
        rf::BudgetKind::OutputBytes => BudgetKind::Output,
        rf::BudgetKind::HostCalls => BudgetKind::HostCall,
        _ => return None,
    })
}

/// Translates a ruviform `"sha256:v1:<hex>"` digest into the
/// synvelis `"sha256:<hex>"` source-digest tag. Unknown formats pass
/// through unchanged — the value is diagnostic-only on this path.
fn digest_from_rf(digest: &str) -> String {
    digest
        .strip_prefix("sha256:v1:")
        .map_or_else(|| digest.to_owned(), |hex| format!("sha256:{hex}"))
}

/// Maps a product [`rf::SessionError`] onto the synvelis
/// [`SessionError`] vocabulary. Only reachable for failures the
/// adapter's own pre-validation did not already classify (VM
/// creation, host surface, resume — the last two are impossible
/// with the fixed empty surface / no resume the adapter sends).
fn session_error_from_rf(error: rf::SessionError) -> SessionError {
    match error {
        rf::SessionError::UnsupportedBudget { kind } => match budget_kind_from_rf(kind) {
            Some(kind) => SessionError::UnsupportedBudget { kind },
            None => SessionError::Backend {
                reason: format!("unmappable budget kind: {kind:?}"),
            },
        },
        rf::SessionError::InvalidHostSurface { reason } => SessionError::Backend { reason },
        rf::SessionError::SourceDigestMismatch { expected, actual } => {
            SessionError::SourceDigestMismatch {
                expected: SourceDigest(digest_from_rf(expected.as_str())),
                actual: SourceDigest(digest_from_rf(actual.as_str())),
            }
        }
        rf::SessionError::SourceTooLarge { bytes, max } => SessionError::SourceTooLarge {
            size: bytes,
            limit: max,
        },
        rf::SessionError::LanguageMismatch { expected, actual } => SessionError::LanguageMismatch {
            expected: ScriptLanguageId(expected.as_str().to_owned()),
            actual: ScriptLanguageId(actual.as_str().to_owned()),
        },
        rf::SessionError::IsolationViolation { detail } => {
            SessionError::IsolationViolation { reason: detail }
        }
        rf::SessionError::ResourceExhausted { detail } => {
            SessionError::ResourceExhausted { resource: detail }
        }
        rf::SessionError::Backend { detail } => SessionError::Backend { reason: detail },
        _ => SessionError::Backend {
            reason: error.to_string(),
        },
    }
}

impl ScriptBackend for LuauBackend {
    fn descriptor(&self) -> &BackendDescriptor {
        &self.descriptor
    }

    fn open_session(&self, config: &SessionConfig) -> Result<Box<dyn ScriptSession>, SessionError> {
        // Pre-validation replicates the interim order so the typed
        // SessionError precedence consumers observe is unchanged.
        // Everything the product backend would also check is either
        // re-checked there harmlessly or already guaranteed here.
        if self.descriptor.language != config.source.language {
            return Err(SessionError::LanguageMismatch {
                expected: self.descriptor.language.clone(),
                actual: config.source.language.clone(),
            });
        }
        let size = u64::try_from(config.source.text.len()).unwrap_or(u64::MAX);
        if size > MAX_SOURCE_BYTES {
            return Err(SessionError::SourceTooLarge {
                size,
                limit: MAX_SOURCE_BYTES,
            });
        }
        // Digest contract: only the `sha256:<hex>` tag is verifiable.
        // The product contract's `sha256:v1:` digest is computed
        // inside `rf::ScriptSource::new` below and always matches.
        let actual = format!(
            "sha256:{}",
            synvelis_script::sha256_hex(config.source.text.as_bytes())
        );
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
        // Every requested budget kind must be one this backend
        // enforces — `max_host_calls` is vacuous upstream (no host
        // call path exists) but the port requires explicit
        // enforcement, so it is rejected here.
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
        // One artifact per session — the payload kind is
        // consumer-declared; translate to the neutral vocabulary in
        // the same pass so unknown future kinds fail closed.
        let product_artifact_kinds = self.product_artifact_kinds(config)?;

        let product_config = rf::SessionConfig {
            budget: rf::ExecutionBudget {
                max_instructions: config.budget.max_instructions,
                max_wall_time: config.budget.max_wall_time,
                max_memory_bytes: config.budget.max_memory_bytes,
                max_output_bytes: config.budget.max_output_bytes,
                max_host_calls: config.budget.max_host_calls,
            },
            // The neutral contract carries host binding descriptors
            // only — no handler seam exists — and the synvelis
            // SessionConfig declares no host surface, so every
            // session opens with the empty surface.
            host_bindings: rf::HostApiSurface::empty(),
            artifact_kinds: product_artifact_kinds,
            source: rf::ScriptSource::new(
                config.source.text.clone(),
                rf::ScriptLanguageId::new(config.source.language.0.clone()),
                config
                    .source
                    .backend_hint
                    .as_ref()
                    .map(|hint| rf::ScriptBackendId::new(hint.0.clone())),
                rf::SourceRevision::new(config.source.revision.0),
            ),
            placement_hint: None,
            resume: None,
        };
        let inner = rf::ScriptBackend::open_session(&self.product, product_config)
            .map_err(session_error_from_rf)?;
        // Producer provenance: the artifact's `impl_version` names
        // the implementation that actually ran the VM — the product
        // backend's descriptor, not this binding crate's version.
        let impl_version = rf::ScriptBackend::descriptor(&self.product)
            .version
            .impl_version
            .clone();
        Ok(Box::new(LuauSession {
            session_id: format!("luau-mlua-{}", NEXT_SESSION.fetch_add(1, Ordering::Relaxed)),
            backend_id: self.descriptor.id.clone(),
            impl_version,
            inner,
            source_language: config.source.language.clone(),
            source_digest: config.source.digest.clone(),
            source_revision: config.source.revision,
            enforced_budgets: enforced_budget_kinds(&config.budget),
            evaluated: false,
            closed: false,
        }))
    }
}

/// A live evaluation session — wraps the product backend's session
/// and translates its neutral-contract results back into the
/// synvelis port vocabulary.
struct LuauSession {
    session_id: String,
    backend_id: ScriptBackendId,
    /// Product backend `impl_version` — recorded on artifacts as
    /// [`ProducedBy::impl_version`].
    impl_version: String,
    inner: Box<dyn rf::ScriptSession>,
    /// Source provenance carried into produced artifacts (the
    /// synvelis-side digest tag the consumer verified at open).
    source_language: ScriptLanguageId,
    source_digest: SourceDigest,
    source_revision: SourceRevision,
    /// Budget dimensions actually armed for this session — the only
    /// kinds [`ScriptErrorKind::BudgetExceeded`] may be attributed
    /// to. More than one means the exceeded dimension is unknown.
    enforced_budgets: BTreeSet<BudgetKind>,
    evaluated: bool,
    closed: bool,
}

impl LuauSession {
    /// Attributes a budget breach to a dimension only when exactly
    /// one enforceable budget was armed — the neutral error carries
    /// no per-dimension detail, so ambiguity reports `None`.
    fn attribute_budget(&self) -> Option<BudgetKind> {
        let mut kinds = self.enforced_budgets.iter();
        match (kinds.next(), kinds.next()) {
            (Some(kind), None) => Some(*kind),
            _ => None,
        }
    }

    /// Maps a neutral [`rf::ScriptError`] onto the synvelis
    /// [`ScriptError`] vocabulary, attaching backend/session context
    /// the way the interim backend did.
    fn script_error_from_rf(&self, error: rf::ScriptError) -> ScriptError {
        let kind = match error.kind {
            rf::ScriptErrorKind::Parse => ScriptErrorKind::Parse,
            rf::ScriptErrorKind::Compile => ScriptErrorKind::Compile,
            rf::ScriptErrorKind::Type => ScriptErrorKind::Type,
            rf::ScriptErrorKind::Runtime => ScriptErrorKind::Runtime,
            rf::ScriptErrorKind::BudgetExceeded => ScriptErrorKind::BudgetExceeded {
                kind: self.attribute_budget(),
            },
            rf::ScriptErrorKind::Cancelled => ScriptErrorKind::Cancelled,
            rf::ScriptErrorKind::Unsupported => ScriptErrorKind::Unsupported {
                what: error.message.clone(),
            },
            rf::ScriptErrorKind::Host => ScriptErrorKind::Host {
                detail: error.message.clone(),
            },
            rf::ScriptErrorKind::Backend => ScriptErrorKind::Backend {
                detail: error.message.clone(),
            },
            // `Internal` and any future neutral kind both map to the
            // synvelis internal-error bucket — the binding does not
            // invent distinctions the contract does not carry.
            _ => ScriptErrorKind::Internal {
                detail: error.message.clone(),
            },
        };
        let mut mapped = ScriptError::new(kind, error.message);
        if let Some(range) = error.range {
            mapped.position = Some(SourcePosition {
                line: range.start.line,
                column: range.start.col,
            });
        }
        mapped.backend_id = Some(self.backend_id.clone());
        mapped.session_id = Some(self.session_id.clone());
        mapped
    }

    /// Converts a neutral [`rf::ScriptValue`] into the synvelis
    /// [`ScriptValue`] model. Non-finite numbers keep the interim
    /// [`ScriptErrorKind::ValueOutOfRange`] rejection; domain-qualified
    /// `Time`/`ResourceId` values have no faithful synvelis form and
    /// are a binding-detected [`ScriptErrorKind::ContractViolation`]
    /// (unreachable from Luau evaluation — the lift never produces
    /// them).
    fn value_from_rf(value: rf::ScriptValue) -> Result<ScriptValue, ScriptError> {
        Ok(match value {
            rf::ScriptValue::Absent => ScriptValue::Absent,
            rf::ScriptValue::Null => ScriptValue::Null,
            rf::ScriptValue::Bool(b) => ScriptValue::Bool(b),
            rf::ScriptValue::Integer(i) => ScriptValue::Integer(i),
            rf::ScriptValue::Number(f) => {
                if !f.is_finite() {
                    return Err(ScriptError::new(
                        ScriptErrorKind::ValueOutOfRange,
                        "non-finite Luau number",
                    ));
                }
                ScriptValue::Number(f)
            }
            rf::ScriptValue::String(s) => ScriptValue::String(s),
            rf::ScriptValue::Bytes(b) => ScriptValue::Bytes(b),
            rf::ScriptValue::Array(items) => ScriptValue::Array(
                items
                    .into_iter()
                    .map(Self::value_from_rf)
                    .collect::<Result<_, _>>()?,
            ),
            rf::ScriptValue::Record(map) => ScriptValue::Record(
                map.into_iter()
                    .map(|(key, value)| Ok((key, Self::value_from_rf(value)?)))
                    .collect::<Result<_, ScriptError>>()?,
            ),
            rf::ScriptValue::Time { .. } | rf::ScriptValue::ResourceId { .. } => {
                return Err(ScriptError::new(
                    ScriptErrorKind::ContractViolation {
                        detail: "domain-qualified value has no synvelis port form".to_owned(),
                    },
                    "domain-qualified value cannot cross the synvelis binding",
                ));
            }
            _ => {
                return Err(ScriptError::new(
                    ScriptErrorKind::ContractViolation {
                        detail: "unknown neutral value variant".to_owned(),
                    },
                    "unknown neutral value variant",
                ));
            }
        })
    }

    /// Converts a neutral [`rf::ScriptArtifact`] into the synvelis
    /// [`ScriptArtifact`]. The digest is recomputed under the synvelis
    /// canonical scheme — `digest_of(to_json(payload))` — identical
    /// to the interim backend's formula, so the artifact digest keeps
    /// committing to the same canonical bytes consumers could always
    /// recompute.
    fn artifact_from_rf(
        &self,
        artifact: rf::ScriptArtifact,
    ) -> Result<ScriptArtifact, ScriptError> {
        let mut payload_map = BTreeMap::new();
        for (key, value) in artifact.payload {
            payload_map.insert(key, Self::value_from_rf(value)?);
        }
        let payload = ScriptValue::Record(payload_map);
        let json = payload.to_json()?;
        let digest = synvelis_script::digest_of(&json).map_err(|e| {
            ScriptError::new(
                ScriptErrorKind::Backend {
                    detail: format!("artifact digest: {e}"),
                },
                "artifact digest failed",
            )
        })?;
        Ok(ScriptArtifact {
            schema_version: SCRIPT_ARTIFACT_SCHEMA_VERSION,
            kind: Self::artifact_kind_from_rf(&artifact.kind)?,
            digest: DefinitionDigest(digest),
            payload,
            source: ArtifactSource {
                language: self.source_language.clone(),
                backend_id: self.backend_id.clone(),
                source_digest: self.source_digest.clone(),
                source_revision: self.source_revision,
            },
            produced_by: ProducedBy {
                backend_id: self.backend_id.clone(),
                impl_version: self.impl_version.clone(),
            },
        })
    }

    /// Translates a neutral [`rf::ArtifactKind`] back into the
    /// synvelis vocabulary.
    fn artifact_kind_from_rf(kind: &rf::ArtifactKind) -> Result<ArtifactKind, ScriptError> {
        Ok(match kind {
            rf::ArtifactKind::ActionRecipe => ArtifactKind::ActionRecipe,
            rf::ArtifactKind::WorkflowDefinition => ArtifactKind::WorkflowDefinition,
            rf::ArtifactKind::ConsumerExtension { domain } => ArtifactKind::ConsumerExtension {
                domain: domain.clone(),
            },
            _ => {
                return Err(ScriptError::new(
                    ScriptErrorKind::ContractViolation {
                        detail: "unknown neutral artifact kind".to_owned(),
                    },
                    "unknown neutral artifact kind",
                ));
            }
        })
    }

    /// Converts a neutral [`rf::ScriptDiagnostic`] into the synvelis
    /// [`ScriptDiagnostic`] shape.
    fn diagnostic_from_rf(diagnostic: &rf::ScriptDiagnostic) -> ScriptDiagnostic {
        let severity = match diagnostic.severity {
            rf::DiagnosticSeverity::Info | rf::DiagnosticSeverity::Hint => {
                DiagnosticSeverity::Information
            }
            rf::DiagnosticSeverity::Warning => DiagnosticSeverity::Warning,
            // `Error` and any future neutral severity map to the
            // synvelis error bucket — fail-visible, never downgraded.
            _ => DiagnosticSeverity::Error,
        };
        ScriptDiagnostic {
            severity,
            code: diagnostic.code.clone().unwrap_or_default(),
            message: diagnostic.message.clone(),
            position: diagnostic.range.map(|range| SourcePosition {
                line: range.start.line,
                column: range.start.col,
            }),
        }
    }

    /// Lowers a neutral [`rf::ScriptResult`] into the synvelis
    /// [`ScriptResult`].
    fn result_from_rf(&self, result: rf::ScriptResult) -> Result<ScriptResult, ScriptError> {
        let return_value = result.return_value.map(Self::value_from_rf).transpose()?;
        let artifact = result
            .artifact
            .map(|artifact| self.artifact_from_rf(artifact))
            .transpose()?;
        let diagnostics = result
            .diagnostics
            .iter()
            .map(Self::diagnostic_from_rf)
            .collect();
        Ok(ScriptResult {
            artifact,
            return_value,
            diagnostics,
        })
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
        if self.inner.cancel_token().is_cancelled() {
            return Err(ScriptError::new(
                ScriptErrorKind::Cancelled,
                "evaluation cancelled",
            ));
        }
        // The port contract is evaluate-once: a second run would see
        // mutated globals and a possibly stale interrupt flag, so the
        // session is spent after the first attempt — success or not.
        // Classified `ContractViolation` here (the interim behavior)
        // rather than the product backend's `Internal`.
        if self.evaluated {
            return Err(ScriptError::new(
                ScriptErrorKind::ContractViolation {
                    detail: "session already evaluated".to_owned(),
                },
                "evaluate called twice",
            ));
        }
        self.evaluated = true;
        let result = self
            .inner
            .evaluate(None)
            .map_err(|error| self.script_error_from_rf(error))?;
        self.result_from_rf(result)
    }

    fn cancel(&mut self) {
        self.inner.cancel();
    }

    fn close(&mut self) {
        self.closed = true;
        self.inner.close();
    }
}
