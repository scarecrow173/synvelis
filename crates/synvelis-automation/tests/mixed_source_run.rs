//! AUTX-06 / synvelis#265 — mixed-source workflow run parity (headless):
//! the same workflow semantics compiled from YAML, Luau, and Rhai
//! sources must converge on one `ExecutablePlan`/`WorkflowDefinition`
//! pair and then execute through the *same* `WorkflowCoordinator` +
//! `MemoryRunJournal` + scripted `StepLane`/`JobLane` — proving run
//! behavior is source-independent (PAR-01 / CXL-P / IND-P / RPL-P /
//! XSRC-01 / SRC-0, `docs/test-specs/autx-06-mixed-source-run.md`).
//!
//! Script-side fixtures mirror `tests/importer.rs`: a minimal
//! `ScriptApiModel`/`BindingSet` over gateway capabilities whose ids
//! match the automation `common` registry, so `action()` pins lower
//! to the same `id@major.minor` refs the YAML DSL writes literally.
//! `compile_script_workflow` returns only `PlanOutput`, so the
//! `RunContext.definition` for script sources is rebuilt through the
//! same `WorkflowScriptBuilder::build` → `import_workflow_document`
//! lowering; `verify_plan_identity`'s `definition_digest` check keeps
//! the two honest.

mod common;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use serde_json::{Map, Value, json};
use synvelis_automation::coordinator::{
    JobLane, JobObservation, OperationObservation, RunContext, RunMeta, StepCall, StepLane,
    WorkflowCoordinator,
};
use synvelis_automation::run::{
    JournalEntry, JournalKind, MemoryRunJournal, RunError, RunId, RunJournal, RunState, StepFrame,
    StepPath, WorkflowRun, replay_run,
};
use synvelis_automation::{
    ExecutablePlan, ImportedWorkflow, ImportedWorkflowPayload, MacroRegistry, MapWorkflowResolver,
    RegistrySnapshot, WorkflowDefinition, compile_script_workflow, compile_workflow_yaml,
    import_workflow_document,
};
use synvelis_gateway::id::{IdempotencyKey, OperationId};
use synvelis_gateway::job::{CancelLink, CancelResponse, JobPhase};
use synvelis_gateway::outcome::{ErrorEnvelope, GatewayError, GatewayOutcome};
use synvelis_gateway::request::{Principal, PrincipalKind};
use synvelis_gateway::{
    Availability, CapabilityDescriptor, CapabilityId, CapabilityRef, CapabilityRegistry,
    CapabilityVersion, DataAccessClass, DataAccessSet, EffectKind, EffectSet, ExecutionContract,
    OperationDomain, PermissionKind, PermissionSet, ResourceKind, SchemaRef, SchemaRegistry,
    SchemaVersion, TargetContract, ValueSchema,
};
use synvelis_script::{
    AvailableBackends, BackendRequirement, BindingSet, LanguageId,
    RegistrySnapshot as ScriptSnapshot, SURFACE_SCHEMA_VERSION, ScriptApiModel, ScriptStepCall,
    ScriptSurfaceEntry, ScriptSurfaceSpec, ScriptVisibility, ScriptWorkflowRef, ScriptWorkflowSpec,
    SpecIoDecl, WORKFLOW_SCRIPT_DEFINITION_SCHEMA_VERSION, WORKFLOW_SPEC_SCHEMA_VERSION,
    WorkflowScriptBuilder, WorkflowScriptDefinition,
};

use crate::common::{
    ASYNC_WORKFLOW, BASIC_WORKFLOW, async_inputs, basic_inputs, definition, registry,
};

// ---------- scripted lanes (mirrors tests/coordinator.rs) --------------------

type LaneResult = Result<GatewayOutcome<Value>, Box<ErrorEnvelope>>;

/// Scripted capability lane: pops one outcome per `invoke`, records
/// every call (idempotency key included) for assertions.
struct MockLane {
    script: Mutex<VecDeque<LaneResult>>,
    calls: Mutex<Vec<StepCall>>,
    op_results: Mutex<HashMap<OperationId, OperationObservation>>,
}

impl MockLane {
    fn new(script: Vec<LaneResult>) -> Self {
        Self {
            script: Mutex::new(script.into()),
            calls: Mutex::new(Vec::new()),
            op_results: Mutex::new(HashMap::new()),
        }
    }

    fn completed(result: Value) -> GatewayOutcome<Value> {
        GatewayOutcome::Completed { result }
    }

    fn calls(&self) -> Vec<StepCall> {
        self.calls.lock().unwrap().clone()
    }
}

impl StepLane for MockLane {
    fn invoke(&self, call: &StepCall) -> LaneResult {
        self.calls.lock().unwrap().push(call.clone());
        self.script
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or_else(|| Ok(GatewayOutcome::Completed { result: json!({}) }))
    }

    fn operation_result(&self, operation: &OperationId) -> OperationObservation {
        self.op_results
            .lock()
            .unwrap()
            .get(operation)
            .cloned()
            .unwrap_or(OperationObservation::Unknown)
    }
}

/// Scripted job lane: one mutable observation + recorded cancels.
struct MockJobs {
    observation: Mutex<Option<JobObservation>>,
    cancel_response: Mutex<CancelResponse>,
    cancels: Mutex<Vec<OperationId>>,
}

impl MockJobs {
    fn new(observation: JobObservation) -> Self {
        Self {
            observation: Mutex::new(Some(observation)),
            cancel_response: Mutex::new(CancelResponse::Unreachable),
            cancels: Mutex::new(Vec::new()),
        }
    }

    fn live(phase: JobPhase) -> JobObservation {
        JobObservation::Live {
            phase,
            progress: None,
            result: None,
            job: None,
            cancel: CancelLink::NotRequested,
        }
    }

    fn live_completed(result: Value) -> JobObservation {
        JobObservation::Live {
            phase: JobPhase::Completed,
            progress: Some(100),
            result: Some(result),
            job: None,
            cancel: CancelLink::NotRequested,
        }
    }

    fn set(&self, observation: JobObservation) {
        *self.observation.lock().unwrap() = Some(observation);
    }

    fn cancel_count(&self) -> usize {
        self.cancels.lock().unwrap().len()
    }
}

impl JobLane for MockJobs {
    fn observe(&self, _operation: &OperationId) -> JobObservation {
        self.observation
            .lock()
            .unwrap()
            .clone()
            .unwrap_or(JobObservation::Unknown)
    }

    fn request_cancel(&self, operation: &OperationId) -> CancelResponse {
        self.cancels.lock().unwrap().push(*operation);
        self.cancel_response.lock().unwrap().clone()
    }
}

struct Fixture {
    journal: Arc<MemoryRunJournal>,
    lane: Arc<MockLane>,
    jobs: Arc<MockJobs>,
    coord: WorkflowCoordinator,
}

fn fixture(script: Vec<LaneResult>, job_observation: JobObservation) -> Fixture {
    let journal = Arc::new(MemoryRunJournal::new());
    let lane = Arc::new(MockLane::new(script));
    let jobs = Arc::new(MockJobs::new(job_observation));
    let coord = WorkflowCoordinator::new(journal.clone(), lane.clone(), jobs.clone());
    Fixture {
        journal,
        lane,
        jobs,
        coord,
    }
}

fn meta(run_id: &str) -> RunMeta {
    RunMeta {
        run_id: RunId::new(run_id).unwrap(),
        idempotency_key: IdempotencyKey::new(format!("start-{run_id}")).unwrap(),
        principal: Principal::new(PrincipalKind::Agent, "wf-test").unwrap(),
        project_id: "project-test".to_owned(),
        task: None,
        grant: None,
        policy_epoch: Some(1),
        expected_revision: None,
    }
}

fn inputs_map(v: Value) -> Map<String, Value> {
    match v {
        Value::Object(m) => m,
        _ => Map::new(),
    }
}

fn path_of(step_id: &str) -> StepPath {
    vec![StepFrame {
        step_id: step_id.to_owned(),
        iteration: None,
    }]
}

// ---------- source artifacts -------------------------------------------------

/// One compiled source: the `WorkflowDefinition` the plan was built
/// from, the `ExecutablePlan` itself, and the typed inputs.
struct SourceArtifacts {
    /// Source label for diagnostics (`yaml` / `luau` / `rhai`).
    source: &'static str,
    definition: WorkflowDefinition,
    plan: ExecutablePlan,
    inputs: Map<String, Value>,
}

/// YAML lane: literal DSL source → `compile_workflow_yaml` (the
/// headless YAML plan path) + `parse_workflow_yaml` for the AST the
/// coordinator executes against.
fn yaml_artifacts(yaml_src: &str, inputs: Value) -> SourceArtifacts {
    let inputs = inputs_map(inputs);
    let definition = definition(yaml_src);
    let plan = compile_workflow_yaml(yaml_src, &inputs, &registry(), &MacroRegistry::default())
        .expect("yaml headless compile")
        .plan;
    SourceArtifacts {
        source: "yaml",
        definition,
        plan,
        inputs,
    }
}

// ---------------------------------------------------------------------------
// Script-side fixtures (mirrors tests/importer.rs, extended with the
// async-operation capabilities the ASYNC_WORKFLOW semantic needs)
// ---------------------------------------------------------------------------

fn vid(text: &str) -> CapabilityId {
    CapabilityId::new(text).expect("fixture capability id")
}

fn sref(name: &str) -> SchemaRef {
    SchemaRef::new(name, 1, 0)
}

fn descriptor(
    id: &str,
    domain: OperationDomain,
    execution: ExecutionContract,
    input: &str,
    output: &str,
    target: TargetContract,
) -> CapabilityDescriptor {
    CapabilityDescriptor {
        id: vid(id),
        version: CapabilityVersion::new(1, 0),
        domain,
        input_schema: sref(input),
        output_schema: sref(output),
        target,
        effect_bound: EffectSet::of(vec![EffectKind::Read]),
        required_permissions: PermissionSet::of(vec![PermissionKind::ReadContext]),
        data_access: DataAccessSet::of(vec![DataAccessClass::MetadataRead]),
        execution,
        availability: Availability::Available,
    }
}

fn empty_object_schema() -> ValueSchema {
    ValueSchema::Object {
        fields: std::collections::BTreeMap::new(),
        deny_unknown: true,
    }
}

/// The script API surface covering every capability the parity
/// workflows use: `queryTracks`/`addNote` (sync) plus `renderJob`
/// (AsyncOperation/AsyncAccept → `async-submit`) and `jobStatus`
/// (Query/Sync → `query`). Capability ids match the automation
/// `common` registry so `action()` pins lower to identical refs.
fn script_fixtures() -> (ScriptApiModel, BindingSet) {
    let mut caps = CapabilityRegistry::new();
    let mut schemas = SchemaRegistry::new();
    for name in [
        "args.empty",
        "args.query-tracks",
        "args.add-note",
        "args.render",
        "args.job-ref",
        "out.empty",
        "out.track-list",
        "out.note-id",
        "out.job-handle",
        "out.job-status",
    ] {
        schemas
            .register(name, SchemaVersion::new(1, 0), empty_object_schema())
            .expect("fixture schema registers");
    }
    caps.register(
        descriptor(
            "synvelis.query.tracks",
            OperationDomain::Query,
            ExecutionContract::Sync,
            "args.query-tracks",
            "out.track-list",
            TargetContract::None,
        ),
        None,
    )
    .expect("query cap registers");
    caps.register(
        descriptor(
            "synvelis.project.add-note",
            OperationDomain::ProjectMutation,
            ExecutionContract::Sync,
            "args.add-note",
            "out.note-id",
            TargetContract::Resource {
                kind: ResourceKind::Track,
            },
        ),
        None,
    )
    .expect("mutation cap registers");
    caps.register(
        descriptor(
            "synvelis.job.render",
            OperationDomain::AsyncOperation,
            ExecutionContract::AsyncAccept,
            "args.render",
            "out.job-handle",
            TargetContract::None,
        ),
        None,
    )
    .expect("async cap registers");
    caps.register(
        descriptor(
            "synvelis.job.status",
            OperationDomain::Query,
            ExecutionContract::Sync,
            "args.job-ref",
            "out.job-status",
            TargetContract::None,
        ),
        None,
    )
    .expect("job status cap registers");
    let snapshot = ScriptSnapshot::capture(&caps, &schemas).expect("snapshot captures");
    let entry = |name: &str, id: &str| ScriptSurfaceEntry {
        name: name.to_owned(),
        capability: CapabilityRef {
            id: vid(id),
            version: CapabilityVersion::new(1, 0),
        },
        visibility: ScriptVisibility::Public,
        description: None,
    };
    let surface = ScriptSurfaceSpec {
        schema_version: SURFACE_SCHEMA_VERSION,
        entries: vec![
            entry("queryTracks", "synvelis.query.tracks"),
            entry("addNote", "synvelis.project.add-note"),
            entry("renderJob", "synvelis.job.render"),
            entry("jobStatus", "synvelis.job.status"),
        ],
    };
    let model = ScriptApiModel::build(&snapshot, &surface).expect("model builds");
    let bindings = BindingSet::generate(&model).expect("bindings generate");
    (model, bindings)
}

fn script_definition(language: LanguageId, workflow_id: &str) -> WorkflowScriptDefinition {
    let (language_id, backend) = match language {
        LanguageId::Luau => (LanguageId::Luau, BackendRequirement::Luau),
        LanguageId::Rhai => (LanguageId::Rhai, BackendRequirement::Rhai),
        LanguageId::Yaml => (LanguageId::Yaml, BackendRequirement::None),
    };
    WorkflowScriptDefinition {
        schema_version: WORKFLOW_SCRIPT_DEFINITION_SCHEMA_VERSION,
        script_id: format!("{workflow_id}.{}", language.as_str()),
        version: 1,
        language_id,
        backend_requirement: backend,
        source_digest: synvelis_script::sha256_hex(
            format!("{workflow_id}-{}-src", language.as_str()).as_bytes(),
        ),
    }
}

fn backends_for(language: LanguageId) -> AvailableBackends {
    match language {
        LanguageId::Luau => AvailableBackends::luau(),
        LanguageId::Rhai => AvailableBackends {
            luau: false,
            rhai: true,
        },
        LanguageId::Yaml => unreachable!("yaml is not a script backend"),
    }
}

fn limits_json() -> Value {
    json!({
        "max_steps": 100,
        "max_depth": 8,
        "max_iterations": 1000,
        "max_runtime_ms": 60_000,
        "max_output_bytes": 1_048_576,
        "max_memory_bytes": 134_217_728,
    })
}

fn io(name: &str, schema: Value) -> SpecIoDecl {
    SpecIoDecl {
        name: name.to_owned(),
        schema,
    }
}

/// The Luau/Rhai spec semantically identical to [`BASIC_WORKFLOW`].
fn basic_spec() -> ScriptWorkflowSpec {
    ScriptWorkflowSpec {
        schema_version: WORKFLOW_SPEC_SCHEMA_VERSION,
        workflow_id: "test.basic".to_owned(),
        version: 1,
        inputs: vec![io("track", json!({"type": "id", "kind": "track"}))],
        steps: vec![
            ScriptStepCall::Action {
                id: "tracks".to_owned(),
                name: "queryTracks".to_owned(),
                args: json!({"limit": 10}),
                target: None,
            },
            ScriptStepCall::Action {
                id: "add".to_owned(),
                name: "addNote".to_owned(),
                args: json!({
                    "pitch": 60,
                    "velocity": 100,
                    "start_tick": 0,
                    "duration_ticks": 480,
                }),
                target: Some(json!("$input.track")),
            },
            ScriptStepCall::Emit {
                id: "done".to_owned(),
                output: "note".to_owned(),
                value: json!("$steps.add"),
            },
        ],
        outputs: vec![io("note", json!({"type": "id", "kind": "note"}))],
        limits: limits_json(),
    }
}

/// The script spec semantically identical to [`ASYNC_WORKFLOW`]:
/// async-submit → query → await-job → emit-result.
fn async_spec() -> ScriptWorkflowSpec {
    ScriptWorkflowSpec {
        schema_version: WORKFLOW_SPEC_SCHEMA_VERSION,
        workflow_id: "test.async-flow".to_owned(),
        version: 1,
        inputs: vec![io(
            "range",
            json!({"type": "time-range", "domain": "media"}),
        )],
        steps: vec![
            ScriptStepCall::Action {
                id: "render".to_owned(),
                name: "renderJob".to_owned(),
                args: json!({"range": "$input.range", "format": "wav"}),
                target: None,
            },
            ScriptStepCall::Action {
                id: "status".to_owned(),
                name: "jobStatus".to_owned(),
                args: json!({"job": "$steps.render"}),
                target: None,
            },
            ScriptStepCall::AwaitJob {
                id: "wait".to_owned(),
                job: json!("$steps.render"),
                output_schema: Some("synvelis.out.job-status@1.0".to_owned()),
            },
            ScriptStepCall::Emit {
                id: "done".to_owned(),
                output: "status".to_owned(),
                value: json!("$steps.wait.state"),
            },
        ],
        outputs: vec![io(
            "status",
            json!({"type": "enum", "values": ["running", "succeeded", "failed"]}),
        )],
        limits: limits_json(),
    }
}

/// Script lane: builder → import document → `WorkflowDefinition` for
/// `RunContext`, plus `compile_script_workflow` (the canonical
/// headless script plan path — its first consumer in this slice) for
/// the `ExecutablePlan`.
fn script_artifacts(
    language: LanguageId,
    spec: &ScriptWorkflowSpec,
    inputs: Value,
) -> SourceArtifacts {
    let (model, bindings) = script_fixtures();
    let backends = backends_for(language);
    let script_def = script_definition(language, &spec.workflow_id);
    let inputs = inputs_map(inputs);
    let resolver = MapWorkflowResolver::new();
    let document = WorkflowScriptBuilder::build(&script_def, spec, &backends, &model, &bindings)
        .expect("script spec builds");
    let definition = import_workflow_document(&document, &resolver).expect("import lowers");
    let plan = compile_script_workflow(
        &script_def,
        spec,
        &backends,
        &model,
        &bindings,
        &resolver,
        &inputs,
        &registry(),
        &MacroRegistry::default(),
    )
    .expect("compile_script_workflow")
    .plan;
    SourceArtifacts {
        source: language.as_str(),
        definition,
        plan,
        inputs,
    }
}

/// The three compile paths for [`BASIC_WORKFLOW`] semantics, in
/// yaml → luau → rhai order.
fn sources_basic() -> Vec<SourceArtifacts> {
    vec![
        yaml_artifacts(BASIC_WORKFLOW, basic_inputs()),
        script_artifacts(LanguageId::Luau, &basic_spec(), basic_inputs()),
        script_artifacts(LanguageId::Rhai, &basic_spec(), basic_inputs()),
    ]
}

/// The three compile paths for [`ASYNC_WORKFLOW`] semantics.
fn sources_async() -> Vec<SourceArtifacts> {
    vec![
        yaml_artifacts(ASYNC_WORKFLOW, async_inputs()),
        script_artifacts(LanguageId::Luau, &async_spec(), async_inputs()),
        script_artifacts(LanguageId::Rhai, &async_spec(), async_inputs()),
    ]
}

fn ctx<'a>(src: &'a SourceArtifacts, reg: &'a RegistrySnapshot) -> RunContext<'a> {
    RunContext {
        definition: &src.definition,
        plan: &src.plan,
        inputs: &src.inputs,
        registry: reg,
        macros: &[],
    }
}

// ---------------------------------------------------------------------------
// Heterogeneous chain fixtures (mirrors tests/importer.rs)
// ---------------------------------------------------------------------------

/// A minimal import document wrapping raw step JSON — used for
/// resolver payloads that are still documents (script-language refs).
fn doc_json(workflow_id: &str, version: u32, inputs: Value, steps: Value, outputs: Value) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("schema_version".to_owned(), json!(1));
    map.insert("workflow_id".to_owned(), json!(workflow_id));
    map.insert("version".to_owned(), json!(version));
    map.insert("inputs".to_owned(), inputs);
    map.insert("steps".to_owned(), steps);
    map.insert("outputs".to_owned(), outputs);
    map.insert("limits".to_owned(), limits_json());
    Value::Object(map)
}

fn digest_of_bytes(seed: &str) -> String {
    synvelis_script::sha256_hex(seed.as_bytes())
}

fn workflow_ref(language: LanguageId, version: u32, seed: &str, path: &str) -> ScriptWorkflowRef {
    ScriptWorkflowRef {
        language_id: language,
        version,
        source_digest: digest_of_bytes(seed),
        path: path.to_owned(),
    }
}

fn reference_json(reference: &ScriptWorkflowRef) -> Value {
    serde_json::to_value(reference).unwrap()
}

/// Leaf YAML workflow: query → emit `picked` (the whole track list).
const LEAF_YAML: &str = r"
schema_version: 1
workflow_id: test.leaf
version: 3
inputs:
  - name: track
    schema:
      type: id
      kind: track
steps:
  - kind: query
    id: tracks
    capability: synvelis.query.tracks@1.0
    args:
      limit: 10
  - kind: emit-result
    id: done
    output: picked
    value: $steps.tracks
outputs:
  - name: picked
    schema:
      type: array
      max_len: 1024
      item:
        type: object
        deny_unknown: true
        fields:
          id:
            schema:
              type: id
              kind: track
            required: true
          name:
            schema:
              type: string
              max_len: 256
            required: true
limits:
  max_steps: 100
  max_depth: 8
  max_iterations: 1000
  max_runtime_ms: 60000
  max_output_bytes: 1048576
  max_memory_bytes: 134217728
";

const PICKED_SCHEMA: &str = r#"{"type": "array", "max_len": 1024, "item": {"type": "object", "deny_unknown": true, "fields": {"id": {"schema": {"type": "id", "kind": "track"}, "required": true}, "name": {"schema": {"type": "string", "max_len": 256}, "required": true}}}}"#;

/// Stage B — Rhai document referencing the YAML leaf.
fn rhai_mid_doc(yaml_ref: &ScriptWorkflowRef) -> (Value, ScriptWorkflowRef) {
    let doc = doc_json(
        "test.mid",
        2,
        json!([{"name": "mid", "schema": {"type": "id", "kind": "track"}}]),
        json!([{
            "kind": "workflow",
            "id": "leaf",
            "reference": reference_json(yaml_ref),
            "args": {"track": "$input.mid"},
        }, {
            "kind": "emit-result",
            "id": "emit",
            "output": "picked",
            "value": "$steps.leaf.picked",
        }]),
        json!([{"name": "picked", "schema": serde_json::from_str::<Value>(PICKED_SCHEMA).unwrap()}]),
    );
    let reference = ScriptWorkflowRef {
        language_id: LanguageId::Rhai,
        version: 2,
        source_digest: digest_of_bytes("rhai-src"),
        path: "mid.rhai".to_owned(),
    };
    (doc, reference)
}

/// Stage A — Luau spec (built through the real builder path inside
/// `compile_script_workflow`) referencing the Rhai document.
fn xsrc_top_spec(rhai_ref: &ScriptWorkflowRef) -> ScriptWorkflowSpec {
    ScriptWorkflowSpec {
        schema_version: WORKFLOW_SPEC_SCHEMA_VERSION,
        workflow_id: "test.top".to_owned(),
        version: 1,
        inputs: vec![io("track", json!({"type": "id", "kind": "track"}))],
        steps: vec![
            ScriptStepCall::Workflow {
                id: "mid".to_owned(),
                reference: rhai_ref.clone(),
                args: Map::from_iter([("mid".to_owned(), json!("$input.track"))]),
            },
            ScriptStepCall::Emit {
                id: "done".to_owned(),
                output: "picked".to_owned(),
                value: json!("$steps.mid.picked"),
            },
        ],
        outputs: vec![io(
            "picked",
            serde_json::from_str::<Value>(PICKED_SCHEMA).unwrap(),
        )],
        limits: limits_json(),
    }
}

/// The Luau→Rhai→YAML chain compiled through `compile_script_workflow`
/// with a populated `MapWorkflowResolver` — XSRC-01's single plan.
fn xsrc_artifacts() -> SourceArtifacts {
    let yaml_ref = workflow_ref(LanguageId::Yaml, 3, "leaf-src", "leaf.yaml");
    let (rhai_doc, rhai_ref) = rhai_mid_doc(&yaml_ref);
    let spec = xsrc_top_spec(&rhai_ref);
    let (model, bindings) = script_fixtures();
    let backends = AvailableBackends::luau_and_rhai();
    let script_def = script_definition(LanguageId::Luau, "test.top");
    let inputs = inputs_map(json!({"track": "track-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"}));

    let mut resolver = MapWorkflowResolver::new();
    resolver.insert(
        &yaml_ref,
        ImportedWorkflow {
            language_id: LanguageId::Yaml,
            version: 3,
            source_digest: yaml_ref.source_digest.clone(),
            payload: ImportedWorkflowPayload::Definition(Box::new(definition(LEAF_YAML))),
        },
    );
    resolver.insert(
        &rhai_ref,
        ImportedWorkflow {
            language_id: LanguageId::Rhai,
            version: 2,
            source_digest: rhai_ref.source_digest.clone(),
            payload: ImportedWorkflowPayload::Document(Box::new(rhai_doc)),
        },
    );

    let document = WorkflowScriptBuilder::build(&script_def, &spec, &backends, &model, &bindings)
        .expect("top spec builds");
    let definition = import_workflow_document(&document, &resolver).expect("chain imports");
    let plan = compile_script_workflow(
        &script_def,
        &spec,
        &backends,
        &model,
        &bindings,
        &resolver,
        &inputs,
        &registry(),
        &MacroRegistry::default(),
    )
    .expect("xsrc compile")
    .plan;
    SourceArtifacts {
        source: "luau→rhai→yaml",
        definition,
        plan,
        inputs,
    }
}

// ---------- parity signatures -------------------------------------------------

/// Journal entries compared with `plan_revision` and `digest` masked:
/// those fields commit to plan identity, which is a *witness* here —
/// run-level parity is judged on kind/step/verdict/state/refs/output.
fn entry_signature(entry: &JournalEntry) -> Value {
    let mut v = serde_json::to_value(entry).expect("journal entry serializes");
    let obj = v.as_object_mut().expect("entry is a JSON object");
    obj.remove("plan_revision");
    obj.remove("digest");
    v
}

/// The source-independent part of a `WorkflowRun` projection —
/// everything except `plan_digest` (plan-identity witness) and
/// `run_id` (fixture-chosen, identical across sources anyway).
fn run_signature(run: &WorkflowRun) -> Value {
    json!({
        "run_id": run.run_id,
        "state": run.state,
        "definition_version": run.definition_version,
        "project_id": run.project_id,
        "input_digest": run.input_digest,
        "principal": run.principal,
        "policy_epoch": run.policy_epoch,
        "expected_revision": run.expected_revision,
        "task": run.task,
        "grant": run.grant,
        "idempotency_key": run.idempotency_key,
        "cursor": run.cursor,
        "steps": run.steps,
        "outputs": run.outputs,
        "step_outputs": run.step_outputs,
        "pending_jobs": run.pending_jobs,
        "cancel_requested": run.cancel_requested,
        "cancel_issued": run.cancel_issued,
        "executed_steps": run.executed_steps,
        "loop_iterations": run.loop_iterations,
    })
}

/// One lane invocation reduced to its observable contract.
fn call_signature(call: &StepCall) -> Value {
    json!({
        "capability": call.capability.to_string(),
        "args": call.args,
        "target": call.target,
        "expected_revision": call.expected_revision,
        "idempotency_key": call.idempotency_key.as_str(),
        "policy_epoch": call.policy_epoch,
        "principal": call.principal,
        "project_id": call.project_id,
        "task": call.task,
        "grant": call.grant,
    })
}

/// The observable trace of one driven run, ready for cross-source
/// comparison.
struct RunTrace {
    source: &'static str,
    /// Masked journal entry signatures in append order.
    entries: Vec<Value>,
    /// `WorkflowRun` projection signature at the end of the drive.
    run: Value,
    /// Lane invocations in order.
    calls: Vec<Value>,
}

/// Asserts every trace in `traces` is identical to the first —
/// journal kinds/states/verdicts, run projection, and lane calls.
fn assert_trace_parity(traces: &[RunTrace]) {
    let first = traces.first().expect("at least one trace");
    for t in &traces[1..] {
        assert_eq!(
            t.entries, first.entries,
            "journal trace diverges for source {}",
            t.source
        );
        assert_eq!(
            t.run, first.run,
            "run projection diverges for source {}",
            t.source
        );
        assert_eq!(
            t.calls, first.calls,
            "lane call trace diverges for source {}",
            t.source
        );
    }
}

/// Collects a run's journal + projection into a [`RunTrace`].
fn trace_of(source: &'static str, f: &Fixture, run_id: &RunId, run: &WorkflowRun) -> RunTrace {
    let entries = f.journal.entries(run_id).unwrap();
    RunTrace {
        source,
        entries: entries.iter().map(entry_signature).collect(),
        run: run_signature(run),
        calls: f.lane.calls().iter().map(call_signature).collect(),
    }
}

fn basic_lane_script() -> Vec<LaneResult> {
    vec![
        Ok(MockLane::completed(json!([{"id": "track-1", "name": "a"}]))),
        Ok(MockLane::completed(json!(
            "note-0123456789abcdef0123456789abcdef"
        ))),
    ]
}

fn async_lane_script() -> Vec<LaneResult> {
    let op = OperationId::new(7);
    vec![
        Ok(GatewayOutcome::Accepted { operation: op }),
        Ok(MockLane::completed(json!({"state": "succeeded"}))),
    ]
}

/// Asserts the three sources lower to the same `WorkflowDefinition`
/// and plan shape — the semantic precondition every parity case
/// stands on.
fn assert_source_convergence(sources: &[SourceArtifacts]) {
    assert_eq!(sources.len(), 3, "yaml/luau/rhai triple expected");
    assert_eq!(
        sources[0].definition, sources[1].definition,
        "yaml vs luau definition diverged"
    );
    assert_eq!(
        sources[0].definition, sources[2].definition,
        "yaml vs rhai definition diverged"
    );
    for s in &sources[1..] {
        assert_eq!(
            sources[0].plan.step_order, s.plan.step_order,
            "step_order diverged for {}",
            s.source
        );
        assert_eq!(
            sources[0].plan.definition_digest, s.plan.definition_digest,
            "definition_digest diverged for {}",
            s.source
        );
    }
}

// ---------- PAR-01: 3-source run parity ---------------------------------------

#[test]
fn par01_yaml_luau_rhai_share_one_run_trace() {
    let sources = sources_basic();
    assert_source_convergence(&sources);

    // Witness only: plan digests are recorded for the ledger, never
    // gated — run-level parity is the claim (compile parity is
    // #264/#273's responsibility).
    let digests: Vec<String> = sources
        .iter()
        .map(|s| s.plan.digest().expect("plan digest"))
        .collect();
    eprintln!("PAR-01 plan digest witness: {digests:?}");

    let reg = registry();
    let mut traces = Vec::new();
    for src in &sources {
        let f = fixture(basic_lane_script(), MockJobs::live(JobPhase::Pending));
        let c = ctx(src, &reg);
        let run_id = f.coord.start_run(&c, &meta("run-par")).unwrap();
        let run = f.coord.drive(&run_id, &c).unwrap();
        assert_eq!(run.state, RunState::Completed, "source {}", src.source);
        let result = f.coord.result(&run_id, &c).unwrap();
        assert_eq!(
            result["note"],
            json!("note-0123456789abcdef0123456789abcdef"),
            "source {}",
            src.source
        );
        // Two lane invocations (query + invoke); emit is local.
        assert_eq!(f.lane.calls().len(), 2, "source {}", src.source);
        traces.push(trace_of(src.source, &f, &run_id, &run));
    }
    assert_trace_parity(&traces);
}

// ---------- CXL-P: cancel honesty parity --------------------------------------

#[test]
fn cxl_p_cancel_settles_identically_across_sources() {
    let sources = sources_async();
    assert_source_convergence(&sources);

    let reg = registry();
    let mut traces = Vec::new();
    for src in &sources {
        let f = fixture(async_lane_script(), MockJobs::live(JobPhase::Running));
        *f.jobs.cancel_response.lock().unwrap() = CancelResponse::Pending;
        let c = ctx(src, &reg);
        let run_id = f.coord.start_run(&c, &meta("run-cxl")).unwrap();
        let run = f.coord.drive(&run_id, &c).unwrap();
        assert_eq!(run.state, RunState::Waiting, "source {}", src.source);

        // Cancel while the provider reports Pending — the honest
        // CancelRequested park, never an upgraded "cancelled".
        let run = f.coord.cancel(&run_id, &c).unwrap();
        assert_eq!(
            run.state,
            RunState::CancelRequested,
            "source {}",
            src.source
        );

        // Provider confirms the stop; a second cancel must not resend.
        f.jobs.set(JobObservation::Live {
            phase: JobPhase::Cancelled,
            progress: None,
            result: None,
            job: None,
            cancel: CancelLink::Confirmed,
        });
        let run = f.coord.cancel(&run_id, &c).unwrap();
        assert_eq!(run.state, RunState::Failed, "source {}", src.source);
        assert_eq!(f.jobs.cancel_count(), 1, "source {}", src.source);
        traces.push(trace_of(src.source, &f, &run_id, &run));
    }
    assert_trace_parity(&traces);
}

// ---------- IND-P: indeterminate → reconcile parity ----------------------------

#[test]
fn ind_p_unknown_observation_reconciles_identically() {
    let sources = sources_async();
    assert_source_convergence(&sources);

    let reg = registry();
    let mut traces = Vec::new();
    for src in &sources {
        let f = fixture(async_lane_script(), JobObservation::Unknown);
        let c = ctx(src, &reg);
        let run_id = f.coord.start_run(&c, &meta("run-ind")).unwrap();
        f.coord.drive(&run_id, &c).unwrap();
        let run = f.coord.status(&run_id, &c).unwrap();
        assert_eq!(run.state, RunState::Indeterminate, "source {}", src.source);
        // Unknown is never promoted: result() refuses.
        assert!(
            matches!(
                f.coord.result(&run_id, &c),
                Err(RunError::NotCompleted { .. })
            ),
            "source {}",
            src.source
        );

        // Evidence arrives → reconcile resolves to Completed; the
        // await step re-observes the jobs lane only (no re-submit).
        f.jobs
            .set(MockJobs::live_completed(json!({"state": "succeeded"})));
        let run = f.coord.reconcile(&run_id, &c).unwrap();
        assert_eq!(run.state, RunState::Completed, "source {}", src.source);
        let result = f.coord.result(&run_id, &c).unwrap();
        assert_eq!(
            result["status"],
            json!("succeeded"),
            "source {}",
            src.source
        );
        assert_eq!(
            f.lane.calls().len(),
            2,
            "await must not re-submit the job for {}",
            src.source
        );
        traces.push(trace_of(src.source, &f, &run_id, &run));
    }
    assert_trace_parity(&traces);
}

// ---------- RPL-P: crash-restart replay parity ---------------------------------

#[test]
fn rpl_p_rebuilt_journal_replays_and_resumes_without_duplicate_effects() {
    let sources = sources_basic();
    assert_source_convergence(&sources);

    let reg = registry();
    let indeterminate = || {
        Box::new(ErrorEnvelope::new(GatewayError::Indeterminate {
            reason: "provider restart".to_owned(),
        }))
    };
    let mut traces = Vec::new();
    for src in &sources {
        let f = fixture(
            vec![
                Ok(MockLane::completed(json!([{"id": "track-1", "name": "a"}]))),
                Err(indeterminate()),
                Ok(MockLane::completed(json!(
                    "note-0123456789abcdef0123456789abcdef"
                ))),
            ],
            MockJobs::live(JobPhase::Pending),
        );
        let c = ctx(src, &reg);
        let run_id = f.coord.start_run(&c, &meta("run-rpl")).unwrap();
        f.coord.drive(&run_id, &c).unwrap();
        let parked = f.coord.status(&run_id, &c).unwrap();
        assert_eq!(
            parked.state,
            RunState::Indeterminate,
            "source {}",
            src.source
        );

        // Crash-restart: copy header+entries into a fresh journal and
        // rebuild the projection from scratch — the same seam a
        // restarted process would use.
        let header = f
            .journal
            .run_header(&run_id)
            .unwrap()
            .expect("run header recorded");
        let entries = f.journal.entries(&run_id).unwrap();
        let journal2 = Arc::new(MemoryRunJournal::new());
        journal2.create_run(&header).unwrap();
        for e in &entries {
            journal2.append(e.clone()).unwrap();
        }
        let plan_digest = src.plan.digest().expect("plan digest");
        let replayed = replay_run(journal2.as_ref(), &run_id, &src.definition, &plan_digest)
            .expect("replay_run over rebuilt journal");
        assert_eq!(
            run_signature(&replayed),
            run_signature(&parked),
            "replay projection must equal the live one for {}",
            src.source
        );

        // A new coordinator over the rebuilt journal resumes; the
        // parked occurrence re-issues under its journaled key.
        let coord2 = WorkflowCoordinator::new(journal2.clone(), f.lane.clone(), f.jobs.clone());
        let run = coord2.reconcile(&run_id, &c).unwrap();
        assert_eq!(run.state, RunState::Completed, "source {}", src.source);

        // The retried `add` call reused the journaled idempotency key
        // — the authoritative side dedupes, zero duplicate effects.
        let calls = f.lane.calls();
        assert_eq!(calls.len(), 3, "source {}", src.source);
        assert_eq!(
            calls[1].idempotency_key, calls[2].idempotency_key,
            "resume must reuse the journaled key for {}",
            src.source
        );

        // Trace from the *rebuilt* journal — post-reconcile entries.
        let entries2 = journal2.entries(&run_id).unwrap();
        traces.push(RunTrace {
            source: src.source,
            entries: entries2.iter().map(entry_signature).collect(),
            run: run_signature(&run),
            calls: calls.iter().map(call_signature).collect(),
        });
    }
    assert_trace_parity(&traces);
}

// ---------- XSRC-01: heterogeneous workflow() call ------------------------------

#[test]
fn xsrc_01_heterogeneous_workflow_call_completes_on_same_fsm() {
    let src = xsrc_artifacts();
    // The inlined callee step carries its namespace through the plan.
    assert_eq!(
        src.plan.step_order,
        vec!["mid__leaf__tracks".to_owned(), "done".to_owned()]
    );

    let reg = registry();
    let track = "track-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let f = fixture(
        vec![Ok(MockLane::completed(json!([{"id": track, "name": "a"}])))],
        MockJobs::live(JobPhase::Pending),
    );
    let c = ctx(&src, &reg);
    let run_id = f.coord.start_run(&c, &meta("run-xsrc")).unwrap();
    let run = f.coord.drive(&run_id, &c).unwrap();
    assert_eq!(run.state, RunState::Completed);
    let result = f.coord.result(&run_id, &c).unwrap();
    assert_eq!(result["picked"], json!([{"id": track, "name": "a"}]));

    // The callee step is journaled under its inlined namespace — same
    // FSM, same idempotency-key derivation as any other step.
    let entries = f.journal.entries(&run_id).unwrap();
    let began = entries
        .iter()
        .find(|e| {
            e.kind == JournalKind::StepBegan
                && e.step.as_ref() == Some(&path_of("mid__leaf__tracks"))
        })
        .expect("namespaced StepBegan journaled");
    assert_eq!(
        began.capability_ref.as_deref(),
        Some("synvelis.query.tracks@1.0")
    );
    assert_eq!(
        began.idempotency_key.as_ref().map(IdempotencyKey::as_str),
        Some("run-xsrc:mid__leaf__tracks")
    );
    // Exactly one lane call — the leaf query; emits are local.
    assert_eq!(f.lane.calls().len(), 1);
}

// ---------- SRC-0: no source/VM provenance in journal ---------------------------

/// Substrings that are unambiguous provenance markers — safe to match
/// anywhere in a key or string value.
const BANNED_SUBSTRINGS: &[&str] = &[
    "luau",
    "rhai",
    "yaml",
    "language",
    "backend",
    "language_id",
    "language-id",
    "backend_requirement",
    "source_digest",
    "source_kind",
    "script_id",
    "workflow_script",
    "vm_state",
    "bytecode",
    "dialect",
];

/// Whole delimited tokens banned in keys and values — catches bare
/// `source`/`vm`/`script`/`backend`/`language`/`eval`/`ast` without
/// substring false positives like "resource"/"description".
const BANNED_TOKENS: &[&str] = &[
    "language", "backend", "luau", "rhai", "yaml", "script", "vm", "source", "dialect", "bytecode",
    "ast", "eval",
];

fn delimited_tokens(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(str::to_owned)
        .collect()
}

fn is_banned_text(text: &str) -> bool {
    let lower = text.to_lowercase();
    if BANNED_SUBSTRINGS.iter().any(|b| lower.contains(b)) {
        return true;
    }
    delimited_tokens(text)
        .iter()
        .any(|t| BANNED_TOKENS.contains(&t.as_str()))
}

/// Recursively scans a serde JSON surface for source/backend/VM
/// provenance — keys and string values alike. Returns violations as
/// `path: detail` strings; empty means the surface is clean.
fn source_violations(value: &Value, path: &str, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, v) in map {
                if is_banned_text(key) {
                    out.push(format!("{path}.{key}: banned key"));
                }
                source_violations(v, &format!("{path}.{key}"), out);
            }
        }
        Value::Array(arr) => {
            for (i, v) in arr.iter().enumerate() {
                source_violations(v, &format!("{path}[{i}]"), out);
            }
        }
        Value::String(s) if is_banned_text(s) => {
            out.push(format!("{path}: banned value {s:?}"));
        }
        _ => {}
    }
}

fn scan_surface(name: &str, value: &Value, violations: &mut Vec<String>) {
    source_violations(value, name, violations);
}

#[test]
fn src0_journal_and_projection_carry_no_source_or_vm_provenance() {
    let reg = registry();
    let mut artifacts = sources_basic();
    artifacts.push(xsrc_artifacts());

    let mut violations = Vec::new();
    for (i, src) in artifacts.iter().enumerate() {
        // XSRC needs its single-query lane script; the basic sources
        // need the two-call script.
        let script = if src.plan.step_order.iter().any(|s| s == "mid__leaf__tracks") {
            vec![Ok(MockLane::completed(json!([{
                "id": "track-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "name": "a"
            }])))]
        } else {
            basic_lane_script()
        };
        let f = fixture(script, MockJobs::live(JobPhase::Pending));
        let c = ctx(src, &reg);
        let run_id = f
            .coord
            .start_run(&c, &meta(&format!("run-src0-{i}")))
            .unwrap();
        let run = f.coord.drive(&run_id, &c).unwrap();
        assert_eq!(run.state, RunState::Completed, "source {}", src.source);

        // Journal surfaces: header + every entry.
        let header = f.journal.run_header(&run_id).unwrap().expect("header");
        scan_surface(
            &format!("{i}:header"),
            &serde_json::to_value(&header).unwrap(),
            &mut violations,
        );
        let entries = f.journal.entries(&run_id).unwrap();
        scan_surface(
            &format!("{i}:entries"),
            &serde_json::to_value(&entries).unwrap(),
            &mut violations,
        );
        // WorkflowRun projection surfaces: checkpoints, outputs, and
        // the job bookkeeping — the same data replay rebuilds.
        let projection = json!({
            "steps": run.steps,
            "outputs": run.outputs,
            "step_outputs": run.step_outputs,
            "pending_jobs": run.pending_jobs,
            "cancel_requested": run.cancel_requested,
            "cancel_issued": run.cancel_issued,
            "cursor": run.cursor,
        });
        scan_surface(&format!("{i}:projection"), &projection, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "source/VM provenance leaked into journal surfaces:\n{}",
        violations.join("\n")
    );
}
