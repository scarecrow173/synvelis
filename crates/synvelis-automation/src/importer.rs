//! Heterogeneous workflow import and the headless plan entry
//! (SV-SCRIPT-03 / synvelis#273).
//!
//! [`import_workflow_document`] lowers a script-produced import
//! document — the canonical `WorkflowDefinition` wire shape plus
//! `kind: "workflow"` reference nodes — into the single typed
//! [`WorkflowDefinition`], resolving every `workflow()` call at
//! compile time:
//!
//! - References are pinned by language / version / source digest and
//!   resolved through a [`WorkflowResolver`] — the only seam that
//!   touches sources. External URIs, implicit Secret/Grant loading,
//!   and runtime VM-to-VM calls cannot be expressed: the resolver
//!   returns parsed plain data, never executables.
//! - Resolved workflows are *inlined*: step ids are namespaced under
//!   the call-site id (`<call>__<step>`), `$input.<name>` references
//!   are substituted with the call's typed argument expressions
//!   (field projection composes through [`Expr::Field`]), and each
//!   declared output's `emit-result` binding becomes a virtual output
//!   referenceable as `$steps.<call>.<output>` — scoped exactly like
//!   real step outputs (control-flow bodies cannot leak them).
//! - Import cycles, version/digest drift, unknown outputs, input
//!   mismatches, and expansion beyond the static step ceiling are all
//!   typed [`ImportError`] rejections — fail closed, never guessed.
//!
//! [`compile_script_workflow`] / [`compile_workflow_yaml`] are the
//! headless `workflow run --file` plan lanes: source → definition →
//! [`PlanCompiler`] → [`ExecutablePlan`]. `.luau`/`.rhai` *files*
//! still require a script backend to produce a [`ScriptWorkflowSpec`]
//! (ruviform#559 gate) — [`workflow_source_kind`] only classifies;
//! the typed [`ScriptApiError::BackendUnavailable`] path covers the
//! rest.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

use serde_json::Value;
use synvelis_script::action::{AvailableBackends, LanguageId};
use synvelis_script::binding::BindingSet;
use synvelis_script::model::ScriptApiModel;
use synvelis_script::workflow::{
    ScriptWorkflowRef, ScriptWorkflowSpec, WorkflowScriptBuilder, WorkflowScriptDefinition,
};

use crate::compiler::{PlanCompiler, PlanOutput};
use crate::definition::{MAX_STATIC_STEPS, Step, WorkflowDefinition, WorkflowLimits};
use crate::error::{AutomationError, ImportError};
use crate::expr::{Condition, Expr, StepOutputRef};
use crate::frontend;
use crate::macros::MacroRegistry;
use crate::registry::RegistrySnapshot;

/// Maximum workflow-reference resolution depth — bounds import
/// recursion independently of the plan's control-depth ceiling.
pub const MAX_IMPORT_DEPTH: usize = 32;

/// What a [`WorkflowResolver`] yields for one pinned reference — the
/// parsed source in its canonical intermediate form, plus the
/// *actual* language/version/digest the importer verifies against the
/// pin (single-sourced pin checking; resolvers never self-assert).
#[derive(Clone, Debug)]
pub struct ImportedWorkflow {
    /// Language the source actually is.
    pub language_id: LanguageId,
    /// Version the source declares.
    pub version: u32,
    /// SHA-256 of the source bytes actually loaded/parsed.
    pub source_digest: String,
    /// The parsed payload.
    pub payload: ImportedWorkflowPayload,
}

/// The parsed form of a resolved workflow source.
#[derive(Clone, Debug)]
pub enum ImportedWorkflowPayload {
    /// A finished [`WorkflowDefinition`] — YAML DSL sources and any
    /// resolver that already ran the full import.
    Definition(Box<WorkflowDefinition>),
    /// A script import document still containing `workflow` steps —
    /// recursively imported by [`import_workflow_document`].
    Document(Box<Value>),
}

/// The source-resolution seam for `workflow()` references.
///
/// Implementations load and parse one pinned reference and report the
/// actual language/version/digest of what they loaded. File-backed
/// implementations must confine paths to a canonical root and compute
/// the digest over the exact bytes parsed (TOCTOU: the digest covers
/// the parsed content, so a swapped file is drift, not a race). There
/// is deliberately no URI/remote/secret surface — references name
/// language + version + digest + optional root-relative path only.
pub trait WorkflowResolver {
    /// Loads and parses `reference`.
    ///
    /// # Errors
    /// [`AutomationError::Import`] — unknown source, path escape, or
    /// unsupported language/backend; parse/decode errors propagate.
    fn resolve(&self, reference: &ScriptWorkflowRef) -> Result<ImportedWorkflow, AutomationError>;
}

/// An in-memory resolver — the fixture/embedded-source implementation.
/// Keys are `(language, version, source_digest)`; `path` is ignored
/// (embedded sources have no file location).
#[derive(Clone, Debug, Default)]
pub struct MapWorkflowResolver {
    entries: BTreeMap<(String, u32, String), ImportedWorkflow>,
}

impl MapWorkflowResolver {
    /// An empty resolver.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers `workflow` under the pin `reference` describes. The
    /// registered actuals are whatever the caller supplies — the
    /// importer still verifies them against the pin.
    pub fn insert(&mut self, reference: &ScriptWorkflowRef, workflow: ImportedWorkflow) {
        self.entries.insert(
            (
                reference.language_id.as_str().to_owned(),
                reference.version,
                reference.source_digest.clone(),
            ),
            workflow,
        );
    }
}

impl WorkflowResolver for MapWorkflowResolver {
    fn resolve(&self, reference: &ScriptWorkflowRef) -> Result<ImportedWorkflow, AutomationError> {
        self.entries
            .get(&(
                reference.language_id.as_str().to_owned(),
                reference.version,
                reference.source_digest.clone(),
            ))
            .cloned()
            .ok_or_else(|| {
                import_err(ImportError::UnknownWorkflow {
                    reference: ref_label(reference),
                })
            })
    }
}

/// A file-backed resolver confined to a canonical root directory.
///
/// This slice resolves `.yaml`/`.yml` workflow DSL files only —
/// `.luau`/`.rhai` references require a script backend to produce a
/// spec (ruviform#559 gate) and fail with
/// [`ImportError::UnsupportedSource`]. Path safety: the reference
/// path must be relative, free of `..`/root/prefix components, and
/// canonicalize inside the root (symlink escapes rejected).
#[derive(Clone, Debug)]
pub struct FileWorkflowResolver {
    root: PathBuf,
}

impl FileWorkflowResolver {
    /// Creates a resolver rooted at `root` (canonicalized).
    ///
    /// # Errors
    /// [`AutomationError::Import`] — root does not exist or cannot be
    /// canonicalized.
    pub fn new(root: impl AsRef<Path>) -> Result<Self, AutomationError> {
        let root = root.as_ref().canonicalize().map_err(|e| {
            import_err(ImportError::UnsupportedSource {
                source: format!("import root {}: {e}", root.as_ref().display()),
            })
        })?;
        Ok(Self { root })
    }
}

impl WorkflowResolver for FileWorkflowResolver {
    fn resolve(&self, reference: &ScriptWorkflowRef) -> Result<ImportedWorkflow, AutomationError> {
        let path = reference.path.as_str();
        if Path::new(path).is_absolute()
            || Path::new(path)
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(import_err(ImportError::PathEscape {
                path: path.to_owned(),
            }));
        }
        let candidate = self.root.join(path);
        let canonical = candidate.canonicalize().map_err(|_| {
            import_err(ImportError::UnknownWorkflow {
                reference: ref_label(reference),
            })
        })?;
        if !canonical.starts_with(&self.root) {
            return Err(import_err(ImportError::PathEscape {
                path: path.to_owned(),
            }));
        }
        let lower = path.to_ascii_lowercase();
        if !(file_ext_is(&lower, "yaml") || file_ext_is(&lower, "yml")) {
            return Err(import_err(ImportError::UnsupportedSource {
                source: format!(
                    "{path}: only .yaml/.yml workflow files resolve here; \
                     script-language files need a backend (deferred)"
                ),
            }));
        }
        let bytes = std::fs::read(&canonical).map_err(|_| {
            import_err(ImportError::UnknownWorkflow {
                reference: ref_label(reference),
            })
        })?;
        // The digest covers the exact bytes parsed — a file swapped
        // between read and verify is drift, never silent acceptance.
        let source_digest = synvelis_script::sha256_hex(&bytes);
        let text = String::from_utf8(bytes).map_err(|_| {
            import_err(ImportError::WorkflowDocument {
                reason: format!("{path}: source is not UTF-8"),
            })
        })?;
        let definition = crate::frontend::parse_workflow_yaml(&text)?;
        let version = definition.version;
        Ok(ImportedWorkflow {
            language_id: LanguageId::Yaml,
            version,
            source_digest,
            payload: ImportedWorkflowPayload::Definition(Box::new(definition)),
        })
    }
}

/// The file-extension → source-kind classification `workflow run
/// --file` uses (synvelis#262 consumes this; the CLI itself is out of
/// this slice's lease). `.yaml`/`.yml` lower through the YAML DSL;
/// `.luau`/`.rhai` are recognized but require a script backend to
/// produce a [`ScriptWorkflowSpec`] — callers must not treat
/// classification as parseability.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkflowSourceKind {
    /// YAML workflow DSL (`.yaml` / `.yml`).
    Yaml,
    /// Luau workflow script (`.luau`).
    Luau,
    /// Rhai workflow script (`.rhai`) — PoC-level only.
    Rhai,
}

/// Classifies a workflow source path by extension.
///
/// # Errors
/// [`AutomationError::Import`] — [`ImportError::UnsupportedSource`]
/// for any other extension.
pub fn workflow_source_kind(path: &Path) -> Result<WorkflowSourceKind, AutomationError> {
    let lower = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if file_ext_is(&lower, "yaml") || file_ext_is(&lower, "yml") {
        Ok(WorkflowSourceKind::Yaml)
    } else if file_ext_is(&lower, "luau") {
        Ok(WorkflowSourceKind::Luau)
    } else if file_ext_is(&lower, "rhai") {
        Ok(WorkflowSourceKind::Rhai)
    } else {
        Err(import_err(ImportError::UnsupportedSource {
            source: format!("{lower}: expected .yaml/.yml/.luau/.rhai"),
        }))
    }
}

/// Imports a script workflow document into a [`WorkflowDefinition`].
///
/// `document` is the canonical JSON the [`WorkflowScriptBuilder`]
/// emits (or an equivalent hand-authored fixture). All `workflow`
/// steps are resolved through `resolver` and inlined; the result is
/// structurally validated. Feed it to [`PlanCompiler`] for the plan.
///
/// # Errors
/// [`AutomationError::Import`] / [`AutomationError::Definition`] /
/// [`AutomationError::Json`] — any resolution or validation failure.
pub fn import_workflow_document(
    document: &Value,
    resolver: &dyn WorkflowResolver,
) -> Result<WorkflowDefinition, AutomationError> {
    import_document(document, resolver, &mut Vec::new(), 0)
}

// ---------------------------------------------------------------------------
// Document import
// ---------------------------------------------------------------------------

/// Identity used for cycle detection — digest pins make content
/// identity path-independent.
#[derive(Clone, Debug, PartialEq, Eq)]
struct RefKey {
    language: String,
    version: u32,
    digest: String,
}

fn ref_key(reference: &ScriptWorkflowRef) -> RefKey {
    RefKey {
        language: reference.language_id.as_str().to_owned(),
        version: reference.version,
        digest: reference.source_digest.clone(),
    }
}

fn ref_label(reference: &ScriptWorkflowRef) -> String {
    let digest: String = reference.source_digest.chars().take(16).collect();
    format!(
        "{}@{}:{}#{}",
        reference.language_id.as_str(),
        reference.version,
        reference.path,
        digest
    )
}

fn import_err(err: ImportError) -> AutomationError {
    AutomationError::Import(err)
}

/// Whether the lowercased file name `lower` ends in `.<ext>` —
/// extension comparison via [`Path::extension`], not substring.
fn file_ext_is(lower: &str, ext: &str) -> bool {
    Path::new(lower)
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e == ext)
}

fn doc_err(reason: impl Into<String>) -> AutomationError {
    import_err(ImportError::WorkflowDocument {
        reason: reason.into(),
    })
}

/// Mutable expansion state for one document level.
struct Ctx<'a, 'b> {
    resolver: &'a dyn WorkflowResolver,
    /// In-progress reference keys (cycle detection) — shared across
    /// recursion levels.
    stack: &'b mut Vec<RefKey>,
    /// Import recursion depth of this document.
    depth: usize,
    /// Virtual workflow-call outputs, scoped like compiler step
    /// scopes: one map per open control-flow branch.
    virtuals: Vec<BTreeMap<String, BTreeMap<String, Expr>>>,
    /// Step/call ids claimed so far — workflow-call ids occupy the
    /// same namespace as real step ids.
    ids: BTreeSet<String>,
    /// Steps produced so far — bounded by [`MAX_STATIC_STEPS`].
    made: usize,
}

/// Imports one document level: envelope decode, step expansion,
/// structural validation.
fn import_document(
    document: &Value,
    resolver: &dyn WorkflowResolver,
    stack: &mut Vec<RefKey>,
    depth: usize,
) -> Result<WorkflowDefinition, AutomationError> {
    if depth > MAX_IMPORT_DEPTH {
        return Err(import_err(ImportError::ImportDepthExceeded {
            depth,
            max: MAX_IMPORT_DEPTH,
        }));
    }
    let Value::Object(map) = document else {
        return Err(doc_err("document is not an object"));
    };
    let schema_version = map
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| doc_err("missing schema_version"))?;
    if schema_version != u64::from(crate::DEFINITION_SCHEMA_VERSION) {
        return Err(AutomationError::UnsupportedDefinitionVersion {
            found: u32::try_from(schema_version).unwrap_or(u32::MAX),
        });
    }
    let workflow_id = map
        .get("workflow_id")
        .and_then(Value::as_str)
        .ok_or_else(|| doc_err("missing workflow_id"))?
        .to_owned();
    let version = map
        .get("version")
        .and_then(Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| doc_err("missing/invalid version"))?;
    let steps_json = map
        .get("steps")
        .and_then(Value::as_array)
        .ok_or_else(|| doc_err("missing steps array"))?;
    let inputs = serde_json::from_value::<Vec<crate::definition::InputDecl>>(
        map.get("inputs")
            .cloned()
            .unwrap_or(Value::Array(Vec::new())),
    )
    .map_err(|e| doc_err(format!("inputs decode: {e}")))?;
    let outputs = serde_json::from_value::<Vec<crate::definition::OutputDecl>>(
        map.get("outputs")
            .cloned()
            .unwrap_or(Value::Array(Vec::new())),
    )
    .map_err(|e| doc_err(format!("outputs decode: {e}")))?;
    let limits = serde_json::from_value::<WorkflowLimits>(
        map.get("limits")
            .cloned()
            .ok_or_else(|| doc_err("missing limits"))?,
    )
    .map_err(|e| doc_err(format!("limits decode: {e}")))?;

    let mut ctx = Ctx {
        resolver,
        stack,
        depth,
        virtuals: vec![BTreeMap::new()],
        ids: BTreeSet::new(),
        made: 0,
    };
    let steps = expand_steps(steps_json, &mut ctx)?;
    let definition = WorkflowDefinition {
        schema_version: crate::DEFINITION_SCHEMA_VERSION,
        workflow_id,
        version,
        inputs,
        steps,
        outputs,
        limits,
    };
    definition.validate()?;
    Ok(definition)
}

// ---------------------------------------------------------------------------
// Step expansion
// ---------------------------------------------------------------------------

fn claim_id(ctx: &mut Ctx<'_, '_>, id: &str) -> Result<(), AutomationError> {
    if !ctx.ids.insert(id.to_owned()) {
        return Err(import_err(ImportError::DuplicateStepId {
            step: id.to_owned(),
        }));
    }
    Ok(())
}

fn count_step(ctx: &mut Ctx<'_, '_>) -> Result<(), AutomationError> {
    ctx.made += 1;
    if ctx.made > MAX_STATIC_STEPS {
        return Err(import_err(ImportError::TooManySteps {
            count: ctx.made,
            max: MAX_STATIC_STEPS,
        }));
    }
    Ok(())
}

/// Expands a list of document steps into typed [`Step`]s.
fn expand_steps(steps: &[Value], ctx: &mut Ctx<'_, '_>) -> Result<Vec<Step>, AutomationError> {
    let mut out = Vec::with_capacity(steps.len());
    for step_json in steps {
        for step in expand_step(step_json, ctx)? {
            count_step(ctx)?;
            out.push(step);
        }
    }
    Ok(out)
}

/// Lowers one ergonomic expression position to a typed [`Expr`] and
/// substitutes visible virtual workflow outputs.
fn lower_expr_at(raw: &Value, ctx: &Ctx<'_, '_>, at: &str) -> Result<Expr, AutomationError> {
    let tagged = frontend::lower_expr(raw)?;
    let expr: Expr = serde_json::from_value(tagged).map_err(|e| doc_err(format!("{at}: {e}")))?;
    subst_virtual_expr(&expr, &ctx.virtuals, at)
}

fn lower_condition_at(
    raw: &Value,
    ctx: &Ctx<'_, '_>,
    at: &str,
) -> Result<Condition, AutomationError> {
    let tagged = frontend::lower_condition(raw)?;
    let cond: Condition =
        serde_json::from_value(tagged).map_err(|e| doc_err(format!("{at}: {e}")))?;
    subst_virtual_condition(&cond, &ctx.virtuals, at)
}

/// Expands one document step node. `workflow` nodes return their
/// inlined expansion (possibly many steps); all others return one.
fn expand_step(step_json: &Value, ctx: &mut Ctx<'_, '_>) -> Result<Vec<Step>, AutomationError> {
    let Value::Object(map) = step_json else {
        return Err(doc_err("step is not an object"));
    };
    let kind = map
        .get("kind")
        .and_then(Value::as_str)
        .ok_or_else(|| doc_err("step missing kind"))?;
    let id = map
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| doc_err(format!("{kind} step missing id")))?;
    claim_id(ctx, id)?;
    match kind {
        "workflow" => expand_workflow_call(map, id, ctx),
        "if" => expand_if_step(map, id, ctx),
        "for-each-bounded" => expand_for_each_step(map, id, ctx),
        "await-job" => expand_await_job_step(map, id, ctx),
        "atomic-macro" => expand_atomic_macro_step(map, id, ctx),
        "emit-result" => expand_emit_step(map, id, ctx),
        "query" | "invoke" | "async-submit" => expand_capability_step(map, id, kind, ctx),
        _ => Err(doc_err(format!("unknown step kind {kind:?}"))),
    }
}

/// Expands nested step lists inside a fresh virtual-output scope —
/// mirroring the compiler's per-branch step scoping.
fn expand_scoped(steps_raw: &[Value], ctx: &mut Ctx<'_, '_>) -> Result<Vec<Step>, AutomationError> {
    ctx.virtuals.push(BTreeMap::new());
    let result = expand_steps(steps_raw, ctx);
    ctx.virtuals.pop();
    result
}

fn expand_if_step(
    map: &serde_json::Map<String, Value>,
    id: &str,
    ctx: &mut Ctx<'_, '_>,
) -> Result<Vec<Step>, AutomationError> {
    let condition_raw = map
        .get("condition")
        .ok_or_else(|| doc_err(format!("if {id} missing condition")))?;
    let condition = lower_condition_at(condition_raw, ctx, "if.condition")?;
    let then_raw = map
        .get("then")
        .and_then(Value::as_array)
        .ok_or_else(|| doc_err(format!("if {id} missing then")))?;
    let then = expand_scoped(then_raw, ctx)?;
    let else_raw = map
        .get("else")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let else_steps = expand_scoped(&else_raw, ctx)?;
    Ok(vec![Step::If {
        id: id.to_owned(),
        condition,
        then,
        else_steps,
    }])
}

fn expand_for_each_step(
    map: &serde_json::Map<String, Value>,
    id: &str,
    ctx: &mut Ctx<'_, '_>,
) -> Result<Vec<Step>, AutomationError> {
    let items_raw = map
        .get("items")
        .ok_or_else(|| doc_err(format!("for-each {id} missing items")))?;
    let items = lower_expr_at(items_raw, ctx, "for-each.items")?;
    let max_iterations = map
        .get("max_iterations")
        .and_then(Value::as_u64)
        .ok_or_else(|| doc_err(format!("for-each {id} missing max_iterations")))?;
    let body_raw = map
        .get("body")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let body = expand_scoped(&body_raw, ctx)?;
    Ok(vec![Step::ForEachBounded {
        id: id.to_owned(),
        items,
        max_iterations,
        body,
    }])
}

fn expand_await_job_step(
    map: &serde_json::Map<String, Value>,
    id: &str,
    ctx: &mut Ctx<'_, '_>,
) -> Result<Vec<Step>, AutomationError> {
    let job_raw = map
        .get("job")
        .ok_or_else(|| doc_err(format!("await-job {id} missing job")))?;
    let job = lower_expr_at(job_raw, ctx, "await-job.job")?;
    let output_schema = match map.get("output_schema") {
        None | Some(Value::Null) => None,
        Some(v) => Some(
            serde_json::from_value(v.clone())
                .map_err(|e| doc_err(format!("await-job {id} output_schema: {e}")))?,
        ),
    };
    Ok(vec![Step::AwaitJob {
        id: id.to_owned(),
        job,
        output_schema,
    }])
}

fn expand_atomic_macro_step(
    map: &serde_json::Map<String, Value>,
    id: &str,
    ctx: &mut Ctx<'_, '_>,
) -> Result<Vec<Step>, AutomationError> {
    let (args, target) = step_args_target(map, ctx, id)?;
    let mac = serde_json::from_value(
        map.get("mac")
            .cloned()
            .ok_or_else(|| doc_err(format!("atomic-macro {id} missing mac")))?,
    )
    .map_err(|e| doc_err(format!("atomic-macro {id} mac: {e}")))?;
    Ok(vec![Step::AtomicMacro {
        id: id.to_owned(),
        mac,
        args,
        target,
    }])
}

fn expand_emit_step(
    map: &serde_json::Map<String, Value>,
    id: &str,
    ctx: &mut Ctx<'_, '_>,
) -> Result<Vec<Step>, AutomationError> {
    let output = map
        .get("output")
        .and_then(Value::as_str)
        .ok_or_else(|| doc_err(format!("emit-result {id} missing output")))?
        .to_owned();
    let value_raw = map
        .get("value")
        .ok_or_else(|| doc_err(format!("emit-result {id} missing value")))?;
    let value = lower_expr_at(value_raw, ctx, "emit-result.value")?;
    Ok(vec![Step::EmitResult {
        id: id.to_owned(),
        output,
        value,
    }])
}

fn expand_capability_step(
    map: &serde_json::Map<String, Value>,
    id: &str,
    kind: &str,
    ctx: &mut Ctx<'_, '_>,
) -> Result<Vec<Step>, AutomationError> {
    let (args, target) = step_args_target(map, ctx, id)?;
    let capability = serde_json::from_value(
        map.get("capability")
            .cloned()
            .ok_or_else(|| doc_err(format!("{kind} {id} missing capability")))?,
    )
    .map_err(|e| doc_err(format!("{kind} {id} capability: {e}")))?;
    let step = match kind {
        "query" => Step::Query {
            id: id.to_owned(),
            capability,
            args,
            target,
        },
        "invoke" => Step::Invoke {
            id: id.to_owned(),
            capability,
            args,
            target,
        },
        _ => Step::AsyncSubmit {
            id: id.to_owned(),
            capability,
            args,
            target,
        },
    };
    Ok(vec![step])
}

/// Lowers the shared `args`/`target` expression positions of a leaf
/// step (absent `args` = empty object, absent `target` = none).
fn step_args_target(
    map: &serde_json::Map<String, Value>,
    ctx: &Ctx<'_, '_>,
    id: &str,
) -> Result<(Expr, Option<Expr>), AutomationError> {
    let args = match map.get("args") {
        None | Some(Value::Null) => Expr::default(),
        Some(raw) => lower_expr_at(raw, ctx, &format!("{id}.args"))?,
    };
    let target = match map.get("target") {
        None | Some(Value::Null) => None,
        Some(raw) => Some(lower_expr_at(raw, ctx, &format!("{id}.target"))?),
    };
    Ok((args, target))
}

// ---------------------------------------------------------------------------
// workflow() resolution + inlining
// ---------------------------------------------------------------------------

/// Expands a `kind: "workflow"` node: resolve the pinned reference,
/// import the payload (recursively for script documents), inline its
/// steps under this call's namespace, and record virtual outputs.
fn expand_workflow_call(
    map: &serde_json::Map<String, Value>,
    id: &str,
    ctx: &mut Ctx<'_, '_>,
) -> Result<Vec<Step>, AutomationError> {
    let reference: ScriptWorkflowRef = serde_json::from_value(
        map.get("reference")
            .cloned()
            .ok_or_else(|| doc_err(format!("workflow {id} missing reference")))?,
    )
    .map_err(|e| doc_err(format!("workflow {id} reference: {e}")))?;
    let args_raw = map
        .get("args")
        .cloned()
        .unwrap_or(Value::Object(serde_json::Map::new()));
    let Value::Object(args_map) = args_raw else {
        return Err(doc_err(format!("workflow {id} args is not an object")));
    };

    // Argument expressions are lowered in the *parent* context —
    // they may reference the parent's inputs/steps/item and earlier
    // virtual outputs, never this call's own outputs.
    let mut arg_exprs = BTreeMap::new();
    for (name, raw) in &args_map {
        let expr = lower_expr_at(raw, ctx, &format!("workflow {id} args.{name}"))?;
        arg_exprs.insert(name.clone(), expr);
    }

    let key = ref_key(&reference);
    if ctx.stack.contains(&key) {
        return Err(import_err(ImportError::ImportCycle {
            reference: ref_label(&reference),
        }));
    }
    ctx.stack.push(key);
    let result = (|| -> Result<WorkflowDefinition, AutomationError> {
        let imported = ctx.resolver.resolve(&reference)?;
        // Pin verification is single-sourced here — resolvers report
        // actuals, never self-assert.
        if imported.language_id != reference.language_id {
            return Err(import_err(ImportError::LanguageMismatch {
                reference: ref_label(&reference),
                pinned: reference.language_id.as_str().to_owned(),
                actual: imported.language_id.as_str().to_owned(),
            }));
        }
        if imported.version != reference.version {
            return Err(import_err(ImportError::VersionMismatch {
                reference: ref_label(&reference),
                pinned: reference.version,
                actual: imported.version,
            }));
        }
        if imported.source_digest != reference.source_digest {
            return Err(import_err(ImportError::SourceDrift {
                reference: ref_label(&reference),
                pinned: reference.source_digest.clone(),
                actual: imported.source_digest.clone(),
            }));
        }
        match imported.payload {
            ImportedWorkflowPayload::Definition(def) => Ok(*def),
            ImportedWorkflowPayload::Document(doc) => {
                import_document(&doc, ctx.resolver, ctx.stack, ctx.depth + 1)
            }
        }
    })();
    ctx.stack.pop();
    let definition = result?;

    let (steps, outputs) = inline_definition(&definition, id, &arg_exprs)?;
    // The call's declared outputs become virtual outputs visible to
    // later siblings in the current scope — exactly like a real
    // step's registered output.
    if let Some(scope) = ctx.virtuals.last_mut() {
        scope.insert(id.to_owned(), outputs);
    }
    Ok(steps)
}

// ---------------------------------------------------------------------------
// Definition inlining
// ---------------------------------------------------------------------------

/// Inlines `definition` at call-site `call_id`: namespaces every step
/// id under `<call_id>__`, substitutes `$input.<name>` references
/// with the call's argument expressions, and returns the expanded
/// steps plus the virtual output map (`output name → value expr`).
///
/// `emit-result` steps for declared outputs are consumed as output
/// bindings — they are not carried into the parent (the parent's own
/// outputs are declared separately); emits for undeclared outputs are
/// kept namespaced so the compiler rejects them fail-closed.
fn inline_definition(
    definition: &WorkflowDefinition,
    call_id: &str,
    arg_exprs: &BTreeMap<String, Expr>,
) -> Result<(Vec<Step>, BTreeMap<String, Expr>), AutomationError> {
    let declared: BTreeSet<&str> = definition.inputs.iter().map(|i| i.name.as_str()).collect();
    let provided: BTreeSet<&str> = arg_exprs.keys().map(String::as_str).collect();
    let missing: Vec<String> = declared
        .difference(&provided)
        .map(|s| (*s).to_owned())
        .collect();
    let unexpected: Vec<String> = provided
        .difference(&declared)
        .map(|s| (*s).to_owned())
        .collect();
    if !missing.is_empty() || !unexpected.is_empty() {
        return Err(import_err(ImportError::WorkflowInputMismatch {
            call: call_id.to_owned(),
            missing,
            unexpected,
        }));
    }
    let ns = format!("{call_id}__");
    let declared_outputs: BTreeSet<&str> =
        definition.outputs.iter().map(|o| o.name.as_str()).collect();
    let mut steps = Vec::with_capacity(definition.steps.len());
    let mut virtuals = BTreeMap::new();
    for step in &definition.steps {
        if let Step::EmitResult { output, value, .. } = step
            && declared_outputs.contains(output.as_str())
        {
            let expr = subst_inline_expr(value, arg_exprs, &ns)?;
            virtuals.insert(output.clone(), expr);
            continue;
        }
        steps.push(inline_step(step, arg_exprs, &ns)?);
    }
    for output in &definition.outputs {
        if !virtuals.contains_key(&output.name) {
            return Err(import_err(ImportError::MissingWorkflowOutput {
                workflow: definition.workflow_id.clone(),
                output: output.name.clone(),
            }));
        }
    }
    Ok((steps, virtuals))
}

/// Inlines one step: namespaces its id and substitutes input/step
/// references inside every expression position.
fn inline_step(
    step: &Step,
    arg_exprs: &BTreeMap<String, Expr>,
    ns: &str,
) -> Result<Step, AutomationError> {
    let nid = |id: &str| format!("{ns}{id}");
    let sub = |e: &Expr| subst_inline_expr(e, arg_exprs, ns);
    let sub_opt = |e: &Option<Expr>| -> Result<Option<Expr>, AutomationError> {
        e.as_ref().map(sub).transpose()
    };
    Ok(match step {
        Step::Query {
            id,
            capability,
            args,
            target,
        } => Step::Query {
            id: nid(id),
            capability: capability.clone(),
            args: sub(args)?,
            target: sub_opt(target)?,
        },
        Step::Invoke {
            id,
            capability,
            args,
            target,
        } => Step::Invoke {
            id: nid(id),
            capability: capability.clone(),
            args: sub(args)?,
            target: sub_opt(target)?,
        },
        Step::AsyncSubmit {
            id,
            capability,
            args,
            target,
        } => Step::AsyncSubmit {
            id: nid(id),
            capability: capability.clone(),
            args: sub(args)?,
            target: sub_opt(target)?,
        },
        Step::AwaitJob {
            id,
            job,
            output_schema,
        } => Step::AwaitJob {
            id: nid(id),
            job: sub(job)?,
            output_schema: output_schema.clone(),
        },
        Step::AtomicMacro {
            id,
            mac,
            args,
            target,
        } => Step::AtomicMacro {
            id: nid(id),
            mac: mac.clone(),
            args: sub(args)?,
            target: sub_opt(target)?,
        },
        Step::If {
            id,
            condition,
            then,
            else_steps,
        } => Step::If {
            id: nid(id),
            condition: subst_inline_condition(condition, arg_exprs, ns)?,
            then: then
                .iter()
                .map(|s| inline_step(s, arg_exprs, ns))
                .collect::<Result<Vec<_>, _>>()?,
            else_steps: else_steps
                .iter()
                .map(|s| inline_step(s, arg_exprs, ns))
                .collect::<Result<Vec<_>, _>>()?,
        },
        Step::ForEachBounded {
            id,
            items,
            max_iterations,
            body,
        } => Step::ForEachBounded {
            id: nid(id),
            items: sub(items)?,
            max_iterations: *max_iterations,
            body: body
                .iter()
                .map(|s| inline_step(s, arg_exprs, ns))
                .collect::<Result<Vec<_>, _>>()?,
        },
        Step::EmitResult { id, output, value } => Step::EmitResult {
            id: nid(id),
            output: output.clone(),
            value: sub(value)?,
        },
    })
}

// ---------------------------------------------------------------------------
// Expression substitution
// ---------------------------------------------------------------------------

/// Substitutes `$input.<name>[.<rest>]` with the call's argument
/// expression and namespaces `$steps.<id>` references, recursively —
/// applied to every expression inside an inlined definition.
fn subst_inline_expr(
    expr: &Expr,
    arg_exprs: &BTreeMap<String, Expr>,
    ns: &str,
) -> Result<Expr, AutomationError> {
    Ok(match expr {
        Expr::Literal { .. } | Expr::Item { .. } | Expr::Param { .. } | Expr::Output { .. } => {
            expr.clone()
        }
        Expr::Input { path } => {
            let Some(name) = path.first() else {
                return Err(doc_err("empty $input path in referenced workflow"));
            };
            let Some(arg) = arg_exprs.get(name) else {
                return Err(doc_err(format!("unbound workflow input {name}")));
            };
            project_expr(arg.clone(), &path[1..])?
        }
        Expr::StepOutput { step_output } => Expr::StepOutput {
            step_output: StepOutputRef {
                step: format!("{ns}{}", step_output.step),
                field: step_output.field.clone(),
            },
        },
        Expr::Array { items } => Expr::Array {
            items: items
                .iter()
                .map(|e| subst_inline_expr(e, arg_exprs, ns))
                .collect::<Result<Vec<_>, _>>()?,
        },
        Expr::Object { fields } => Expr::Object {
            fields: fields
                .iter()
                .map(|(k, e)| Ok((k.clone(), subst_inline_expr(e, arg_exprs, ns)?)))
                .collect::<Result<BTreeMap<_, _>, AutomationError>>()?,
        },
        Expr::Field { base, path } => Expr::Field {
            base: Box::new(subst_inline_expr(base, arg_exprs, ns)?),
            path: path.clone(),
        },
    })
}

fn subst_inline_condition(
    cond: &Condition,
    arg_exprs: &BTreeMap<String, Expr>,
    ns: &str,
) -> Result<Condition, AutomationError> {
    Ok(match cond {
        Condition::Eq { left, right }
        | Condition::Ne { left, right }
        | Condition::Lt { left, right }
        | Condition::Le { left, right }
        | Condition::Gt { left, right }
        | Condition::Ge { left, right } => {
            let l = subst_inline_expr(left, arg_exprs, ns)?;
            let r = subst_inline_expr(right, arg_exprs, ns)?;
            match cond {
                Condition::Eq { .. } => Condition::Eq {
                    left: Box::new(l),
                    right: Box::new(r),
                },
                Condition::Ne { .. } => Condition::Ne {
                    left: Box::new(l),
                    right: Box::new(r),
                },
                Condition::Lt { .. } => Condition::Lt {
                    left: Box::new(l),
                    right: Box::new(r),
                },
                Condition::Le { .. } => Condition::Le {
                    left: Box::new(l),
                    right: Box::new(r),
                },
                Condition::Gt { .. } => Condition::Gt {
                    left: Box::new(l),
                    right: Box::new(r),
                },
                _ => Condition::Ge {
                    left: Box::new(l),
                    right: Box::new(r),
                },
            }
        }
        Condition::Exists { expr } => Condition::Exists {
            expr: Box::new(subst_inline_expr(expr, arg_exprs, ns)?),
        },
        Condition::And { all } => Condition::And {
            all: all
                .iter()
                .map(|c| subst_inline_condition(c, arg_exprs, ns))
                .collect::<Result<Vec<_>, _>>()?,
        },
        Condition::Or { any } => Condition::Or {
            any: any
                .iter()
                .map(|c| subst_inline_condition(c, arg_exprs, ns))
                .collect::<Result<Vec<_>, _>>()?,
        },
        Condition::Not { cond } => Condition::Not {
            cond: Box::new(subst_inline_condition(cond, arg_exprs, ns)?),
        },
    })
}

/// Rewrites `$steps.<call>[.<output>[.<rest>]]` references into the
/// recorded virtual output expressions of visible workflow calls —
/// searched innermost scope first, mirroring compiler step scoping.
fn subst_virtual_expr(
    expr: &Expr,
    virtuals: &[BTreeMap<String, BTreeMap<String, Expr>>],
    at: &str,
) -> Result<Expr, AutomationError> {
    Ok(match expr {
        Expr::StepOutput { step_output } => {
            let found = virtuals
                .iter()
                .rev()
                .find_map(|scope| scope.get(&step_output.step));
            let Some(outputs) = found else {
                return Ok(expr.clone());
            };
            let call = step_output.step.clone();
            match step_output.field.split_first() {
                // `$steps.<call>` bare — the whole outputs object.
                None => Expr::Object {
                    fields: outputs
                        .iter()
                        .map(|(name, e)| (name.clone(), e.clone()))
                        .collect(),
                },
                Some((output, rest)) => {
                    let Some(recorded) = outputs.get(output) else {
                        return Err(import_err(ImportError::UnknownWorkflowOutput {
                            call,
                            output: output.clone(),
                        }));
                    };
                    let _ = at;
                    project_expr(recorded.clone(), rest)?
                }
            }
        }
        Expr::Array { items } => Expr::Array {
            items: items
                .iter()
                .map(|e| subst_virtual_expr(e, virtuals, at))
                .collect::<Result<Vec<_>, _>>()?,
        },
        Expr::Object { fields } => Expr::Object {
            fields: fields
                .iter()
                .map(|(k, e)| Ok((k.clone(), subst_virtual_expr(e, virtuals, at)?)))
                .collect::<Result<BTreeMap<_, _>, AutomationError>>()?,
        },
        Expr::Field { base, path } => Expr::Field {
            base: Box::new(subst_virtual_expr(base, virtuals, at)?),
            path: path.clone(),
        },
        _ => expr.clone(),
    })
}

fn subst_virtual_condition(
    cond: &Condition,
    virtuals: &[BTreeMap<String, BTreeMap<String, Expr>>],
    at: &str,
) -> Result<Condition, AutomationError> {
    Ok(match cond {
        Condition::Eq { left, right }
        | Condition::Ne { left, right }
        | Condition::Lt { left, right }
        | Condition::Le { left, right }
        | Condition::Gt { left, right }
        | Condition::Ge { left, right } => {
            let l = subst_virtual_expr(left, virtuals, at)?;
            let r = subst_virtual_expr(right, virtuals, at)?;
            match cond {
                Condition::Eq { .. } => Condition::Eq {
                    left: Box::new(l),
                    right: Box::new(r),
                },
                Condition::Ne { .. } => Condition::Ne {
                    left: Box::new(l),
                    right: Box::new(r),
                },
                Condition::Lt { .. } => Condition::Lt {
                    left: Box::new(l),
                    right: Box::new(r),
                },
                Condition::Le { .. } => Condition::Le {
                    left: Box::new(l),
                    right: Box::new(r),
                },
                Condition::Gt { .. } => Condition::Gt {
                    left: Box::new(l),
                    right: Box::new(r),
                },
                _ => Condition::Ge {
                    left: Box::new(l),
                    right: Box::new(r),
                },
            }
        }
        Condition::Exists { expr } => Condition::Exists {
            expr: Box::new(subst_virtual_expr(expr, virtuals, at)?),
        },
        Condition::And { all } => Condition::And {
            all: all
                .iter()
                .map(|c| subst_virtual_condition(c, virtuals, at))
                .collect::<Result<Vec<_>, _>>()?,
        },
        Condition::Or { any } => Condition::Or {
            any: any
                .iter()
                .map(|c| subst_virtual_condition(c, virtuals, at))
                .collect::<Result<Vec<_>, _>>()?,
        },
        Condition::Not { cond } => Condition::Not {
            cond: Box::new(subst_virtual_condition(cond, virtuals, at)?),
        },
    })
}

/// Projects `path` onto `base`: whole-value when empty, concrete
/// literal projection, path concatenation for the reference forms,
/// and [`Expr::Field`] for constructed values.
fn project_expr(base: Expr, path: &[String]) -> Result<Expr, AutomationError> {
    if path.is_empty() {
        return Ok(base);
    }
    Ok(match base {
        Expr::Literal { value } => {
            let projected = project_literal(&value, path)?;
            Expr::Literal { value: projected }
        }
        Expr::StepOutput { step_output } => {
            let mut field = step_output.field;
            field.extend(path.iter().cloned());
            Expr::StepOutput {
                step_output: StepOutputRef {
                    step: step_output.step,
                    field,
                },
            }
        }
        Expr::Input { path: mut p } => {
            p.extend(path.iter().cloned());
            Expr::Input { path: p }
        }
        Expr::Item { path: mut p } => {
            p.extend(path.iter().cloned());
            Expr::Item { path: p }
        }
        Expr::Field { base, path: mut p } => {
            p.extend(path.iter().cloned());
            Expr::Field { base, path: p }
        }
        other => Expr::Field {
            base: Box::new(other),
            path: path.to_vec(),
        },
    })
}

/// Concrete projection into a literal JSON value — object keys and
/// canonical array indices; a missing segment is a typed reject.
fn project_literal(value: &Value, path: &[String]) -> Result<Value, AutomationError> {
    let mut cur = value;
    for seg in path {
        cur = match cur {
            Value::Object(map) => map
                .get(seg)
                .ok_or_else(|| doc_err(format!("literal projection misses field {seg}")))?,
            Value::Array(arr) => {
                let idx: usize = seg
                    .parse()
                    .map_err(|_| doc_err(format!("literal projection bad index {seg}")))?;
                arr.get(idx)
                    .ok_or_else(|| doc_err(format!("literal projection misses index {seg}")))?
            }
            _ => return Err(doc_err("literal projection into non-container")),
        };
    }
    Ok(cur.clone())
}

// ---------------------------------------------------------------------------
// Headless plan entry — `workflow run --file` lanes (synvelis#262 seam)
// ---------------------------------------------------------------------------

/// Lowers and compiles a YAML workflow DSL source — the YAML lane of
/// the headless plan path.
///
/// # Errors
/// Same as [`crate::frontend::parse_workflow_yaml`] +
/// [`PlanCompiler::compile_with_macros`].
pub fn compile_workflow_yaml(
    source: &str,
    inputs: &serde_json::Map<String, Value>,
    registry: &RegistrySnapshot,
    macros: &MacroRegistry,
) -> Result<PlanOutput, AutomationError> {
    let definition = crate::frontend::parse_workflow_yaml(source)?;
    PlanCompiler::compile_with_macros(&definition, inputs, registry, macros)
}

/// Lowers and compiles a script workflow end-to-end: builder → import
/// document → [`WorkflowDefinition`] → [`PlanCompiler`]. This is the
/// single headless plan path every script language shares — YAML,
/// Luau, and Rhai specs produce the same `ExecutablePlan` for the
/// same semantics.
///
/// # Errors
/// [`AutomationError::Script`] — definition/spec/binding rejection;
/// [`AutomationError::Import`] — reference resolution failure;
/// [`AutomationError::Compile`] — plan rejection.
#[allow(clippy::too_many_arguments)]
pub fn compile_script_workflow(
    definition: &WorkflowScriptDefinition,
    spec: &ScriptWorkflowSpec,
    backends: &AvailableBackends,
    model: &ScriptApiModel,
    bindings: &BindingSet,
    resolver: &dyn WorkflowResolver,
    inputs: &serde_json::Map<String, Value>,
    registry: &RegistrySnapshot,
    macros: &MacroRegistry,
) -> Result<PlanOutput, AutomationError> {
    let document = WorkflowScriptBuilder::build(definition, spec, backends, model, bindings)?;
    let definition = import_workflow_document(&document, resolver)?;
    PlanCompiler::compile_with_macros(&definition, inputs, registry, macros)
}
