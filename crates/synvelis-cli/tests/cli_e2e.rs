//! Real-binary e2e for `synvelis-cli` (AUTX-03 / synvelis#262).
//!
//! Every test spawns the actual `synvelis-cli` executable and asserts
//! on process exit codes plus the stdout/stderr contract — no in-process
//! shortcuts. Case IDs follow `docs/test-specs/autx-03-headless-cli.md`.

use std::path::{Path, PathBuf};
use std::process::{Command as Proc, Output};

use serde_json::Value;
use synvelis_project::{
    Command, CounterIdGenerator, DirectoryBundleStore, Project, ProjectController, ProjectId,
    ProjectOp,
};

fn cli(args: &[&str]) -> Output {
    Proc::new(env!("CARGO_BIN_EXE_synvelis-cli"))
        .args(args)
        .output()
        .expect("synvelis-cli must spawn")
}

fn stdout_json(out: &Output) -> Value {
    let text = String::from_utf8_lossy(&out.stdout);
    assert_eq!(
        text.lines().count(),
        1,
        "machine mode must emit exactly one stdout line: {text}"
    );
    serde_json::from_str(text.trim()).expect("stdout must be a JSON envelope")
}

fn exit_code(out: &Output) -> i32 {
    out.status.code().expect("exit code")
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("synvelis-cli-e2e-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// A project bundle with one saved HEAD (revision 1) — the same shape
/// `seed_store` produces in the gateway rig.
fn fixture_project(dir: &Path) {
    let mut store = DirectoryBundleStore::create(dir).unwrap();
    let mut controller = ProjectController::new(
        Project::new(ProjectId::new(7), "cli-fixture"),
        Box::new(CounterIdGenerator::new(9_000)),
    );
    controller
        .apply(Command {
            expected_revision: controller.live_revision(),
            op: ProjectOp::SetProjectName {
                name: "cli-fixture".to_owned(),
            },
        })
        .unwrap();
    controller.save(&mut store).unwrap();
}

fn project_dir(tag: &str) -> PathBuf {
    let dir = scratch(&format!("{tag}-project"));
    fixture_project(&dir);
    dir
}

fn write_file(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, body).unwrap();
    path
}

// ── AUTX-CLI-HELP-01 / AUTX-CLI-VER-01 ──────────────────────────

#[test]
fn help_and_version() {
    let out = cli(&["--help"]);
    assert_eq!(exit_code(&out), 0);
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("USAGE:"));
    assert!(text.contains("action invoke"));

    let out = cli(&["--version"]);
    assert_eq!(exit_code(&out), 0);
    assert!(String::from_utf8_lossy(&out.stdout).contains("synvelis-cli"));
}

// ── AUTX-CLI-LIST-01 / AUTX-CLI-DESCRIBE-01 ─────────────────────

#[test]
fn action_list_json_envelope() {
    let out = cli(&["--format", "json", "action", "list"]);
    assert_eq!(
        exit_code(&out),
        0,
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let env = stdout_json(&out);
    assert_eq!(env["schema_version"], 1);
    assert_eq!(env["status"], "completed");
    let caps = env["result"]["capabilities"].as_array().unwrap();
    assert!(caps.len() > 10, "expected the full engine surface");
    let query = caps
        .iter()
        .find(|c| c["id"] == "synvelis.context.query" && c["version"]["major"] == 1)
        .expect("context.query must be registered");
    assert_eq!(query["domain"], "query");
    assert_eq!(
        query["input_schema"]["name"],
        "synvelis.context.query.input"
    );
}

#[test]
fn action_describe_known_and_unknown() {
    let out = cli(&[
        "--format",
        "json",
        "action",
        "describe",
        "synvelis.context.query@1.0",
    ]);
    assert_eq!(exit_code(&out), 0);
    let env = stdout_json(&out);
    assert_eq!(env["status"], "completed");
    assert_eq!(env["result"]["matches"][0]["id"], "synvelis.context.query");

    // No version → all registered versions of the id.
    let out = cli(&[
        "--format",
        "json",
        "action",
        "describe",
        "synvelis.context.query",
    ]);
    assert_eq!(exit_code(&out), 0);
    // Non-empty match set — indexing fails the test if empty.
    assert_eq!(
        stdout_json(&out)["result"]["matches"][0]["id"],
        "synvelis.context.query"
    );

    let out = cli(&["--format", "json", "action", "describe", "bogus.cap"]);
    assert_eq!(exit_code(&out), 5);
    let env = stdout_json(&out);
    assert_eq!(env["status"], "failed");
    assert_eq!(env["error"]["code"], "cli.unsupported");
}

// ── AUTX-CLI-FILE-01: extension/kind fail-closed matrix ─────────

#[test]
fn file_classification_matrix() {
    let dir = scratch("matrix");
    for name in [
        "a.luau", "a.yaml", "a.yml", "a.rhai", "a.lua", "a.txt", "noext",
    ] {
        write_file(&dir, name, "x");
    }
    // Real execution boots the project — the fixture must be a saved
    // bundle, not an empty directory.
    let project = project_dir("matrix-project");

    let cases: &[(&str, &str, i32)] = &[
        // (file, subcommand-prefix, expected exit)
        ("a.luau", "action", 2),   // real VM eval; 'x' is not a valid script
        ("a.luau", "workflow", 2), // evaluated; 'x' is not a valid spec
        ("a.yaml", "action", 2),   // DSL is workflow-only → kind mismatch
        ("a.yml", "action", 2),
        ("a.yaml", "workflow", 2), // 'x' is not a valid workflow document
        ("a.yml", "workflow", 2),
        ("a.rhai", "action", 5), // PoC-only backend
        ("a.rhai", "workflow", 5),
        ("a.lua", "action", 5), // Lua 5.4 explicitly unsupported
        ("a.lua", "workflow", 5),
        ("a.txt", "action", 2), // unknown extension
        ("a.txt", "workflow", 2),
        ("noext", "action", 2),
        ("noext", "workflow", 2),
    ];
    for (file, side, want) in cases {
        let file_path = dir.join(file);
        let out = cli(&[
            "--format",
            "json",
            side,
            "run",
            "--file",
            file_path.to_str().unwrap(),
            "--project",
            project.to_str().unwrap(),
        ]);
        assert_eq!(exit_code(&out), *want, "{side} run --file {file}");
        let env = stdout_json(&out);
        assert_eq!(env["status"], "failed", "{side} run --file {file}");
        assert!(
            env["error"]["code"].as_str().unwrap().starts_with("cli."),
            "{side} run --file {file}: {:?}",
            env["error"]
        );
    }
}

#[test]
fn workflow_luau_file_executes_to_completion() {
    // synvelis#262: `workflow run --file *.luau` evaluates through the
    // real vendored-Luau backend, lowers to an executable plan, and
    // drives the WorkflowCoordinator over live lanes. An emit-only
    // workflow completes without any capability call.
    let dir = scratch("luauwf");
    let spec = r#"
return {
  schema_version = 1,
  workflow_id = "test.cli-luau",
  version = 3,
  inputs = {},
  steps = {
    { call = "emit", id = "done", output = "out", value = 42 },
  },
  outputs = {
    { name = "out", schema = { type = "integer" } },
  },
  limits = {
    max_steps = 10, max_depth = 4, max_iterations = 10,
    max_runtime_ms = 1000, max_output_bytes = 1048576,
    max_memory_bytes = 134217728,
  },
}
"#;
    let file = write_file(&dir, "wf.luau", spec);
    let project = project_dir("luauwf-project");
    let out = cli(&[
        "--format",
        "json",
        "workflow",
        "run",
        "--file",
        file.to_str().unwrap(),
        "--project",
        project.to_str().unwrap(),
    ]);
    assert_eq!(
        exit_code(&out),
        0,
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let env = stdout_json(&out);
    assert_eq!(env["status"], "completed");
    let result = &env["result"];
    assert_eq!(result["evaluated"], true);
    assert_eq!(result["executed"], true);
    assert_eq!(result["state"], "completed");
    assert_eq!(result["workflow_id"], "test.cli-luau");
    assert_eq!(result["version"], 3);
    assert_eq!(result["steps"], 1);
    assert_eq!(result["outputs"]["out"], 42);
    assert!(
        result["run_id"].as_str().is_some(),
        "run id is reported: {result}"
    );
    assert_eq!(result["plan_digest"].as_str().unwrap().len(), 64);
    assert_eq!(result["source_digest"].as_str().unwrap().len(), 64);
}

#[test]
fn workflow_dry_run_validates_dsl_without_executing() {
    let dir = scratch("dsl");
    let wf = write_file(
        &dir,
        "wf.yaml",
        r"
schema_version: 1
workflow_id: test.e2e
version: 1
steps:
  - kind: emit-result
    id: done
    output: out
    value: 42
outputs:
  - name: out
    schema:
      type: integer
limits:
  max_steps: 10
  max_depth: 4
  max_iterations: 10
  max_runtime_ms: 1000
  max_output_bytes: 1024
  max_memory_bytes: 1024
",
    );
    // Dry-run still boots the project read-only — it needs a saved
    // bundle for the live registry/revision pin.
    let project = project_dir("dsl-project");

    let out = cli(&[
        "--format",
        "json",
        "workflow",
        "run",
        "--file",
        wf.to_str().unwrap(),
        "--project",
        project.to_str().unwrap(),
        "--dry-run",
    ]);
    assert_eq!(
        exit_code(&out),
        0,
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let env = stdout_json(&out);
    assert_eq!(env["status"], "completed");
    let r = &env["result"];
    assert_eq!(r["kind"], "workflow-dsl");
    assert_eq!(r["executed"], false);
    assert_eq!(r["dry_run"], true);
    assert_eq!(r["definition_valid"], true);
    assert_eq!(r["workflow_id"], "test.e2e");
}

#[test]
fn workflow_dry_run_rejects_invalid_dsl() {
    let dir = scratch("dsbad");
    let wf = write_file(&dir, "bad.yaml", "schema_version: 2\n");
    let project = project_dir("dsbad-project");
    let out = cli(&[
        "--format",
        "json",
        "workflow",
        "run",
        "--file",
        wf.to_str().unwrap(),
        "--project",
        project.to_str().unwrap(),
        "--dry-run",
    ]);
    assert_eq!(exit_code(&out), 2);
    assert_eq!(stdout_json(&out)["error"]["code"], "cli.invalid-input");
}

// ── AUTX-CLI-RUN-FILE-01: real execution (synvelis#262) ─────────

/// Emit-only YAML workflow — no capability call, so the run reaches
/// `Completed` without touching the (empty) policy store.
const EMIT_WORKFLOW: &str = r"
schema_version: 1
workflow_id: test.e2e-run
version: 1
steps:
  - kind: emit-result
    id: done
    output: out
    value: 42
outputs:
  - name: out
    schema:
      type: integer
limits:
  max_steps: 10
  max_depth: 4
  max_iterations: 10
  max_runtime_ms: 1000
  max_output_bytes: 1024
  max_memory_bytes: 1024
";

#[test]
fn workflow_run_yaml_executes_to_completion() {
    let dir = scratch("yamlrun");
    let wf = write_file(&dir, "wf.yaml", EMIT_WORKFLOW);
    let project = project_dir("yamlrun-project");
    let out = cli(&[
        "--format",
        "json",
        "workflow",
        "run",
        "--file",
        wf.to_str().unwrap(),
        "--project",
        project.to_str().unwrap(),
    ]);
    assert_eq!(
        exit_code(&out),
        0,
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let env = stdout_json(&out);
    assert_eq!(env["status"], "completed");
    let r = &env["result"];
    assert_eq!(r["kind"], "workflow-dsl");
    assert_eq!(r["executed"], true);
    assert_eq!(r["state"], "completed");
    assert_eq!(r["workflow_id"], "test.e2e-run");
    assert_eq!(r["outputs"]["out"], 42);
    assert!(r["run_id"].as_str().is_some(), "run id: {r}");
    assert_eq!(r["plan_digest"].as_str().unwrap().len(), 64);
}

#[test]
fn workflow_run_inputs_inline_and_file() {
    let dir = scratch("wfinputs");
    let wf = write_file(
        &dir,
        "wf.yaml",
        r"
schema_version: 1
workflow_id: test.e2e-inputs
version: 1
inputs:
  - name: count
    schema:
      type: integer
steps:
  - kind: emit-result
    id: done
    output: out
    value: $input.count
outputs:
  - name: out
    schema:
      type: integer
limits:
  max_steps: 10
  max_depth: 4
  max_iterations: 10
  max_runtime_ms: 1000
  max_output_bytes: 1024
  max_memory_bytes: 1024
",
    );
    let inputs_file = write_file(&dir, "inputs.json", r#"{"count": 9}"#);
    let project = project_dir("wfinputs-project");

    // --inputs <json>
    let out = cli(&[
        "--format",
        "json",
        "workflow",
        "run",
        "--file",
        wf.to_str().unwrap(),
        "--project",
        project.to_str().unwrap(),
        "--inputs",
        r#"{"count": 7}"#,
    ]);
    assert_eq!(
        exit_code(&out),
        0,
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(stdout_json(&out)["result"]["outputs"]["out"], 7);

    // --inputs-file <path>
    let out = cli(&[
        "--format",
        "json",
        "workflow",
        "run",
        "--file",
        wf.to_str().unwrap(),
        "--project",
        project.to_str().unwrap(),
        "--inputs-file",
        inputs_file.to_str().unwrap(),
    ]);
    assert_eq!(
        exit_code(&out),
        0,
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(stdout_json(&out)["result"]["outputs"]["out"], 9);

    // Missing declared input → typed invalid input, never a guess.
    let out = cli(&[
        "--format",
        "json",
        "workflow",
        "run",
        "--file",
        wf.to_str().unwrap(),
        "--project",
        project.to_str().unwrap(),
    ]);
    assert_eq!(exit_code(&out), 2);
    assert_eq!(stdout_json(&out)["error"]["code"], "cli.invalid-input");

    // --inputs and --inputs-file are mutually exclusive (usage, 2).
    let out = cli(&[
        "--format",
        "json",
        "workflow",
        "run",
        "--file",
        wf.to_str().unwrap(),
        "--project",
        project.to_str().unwrap(),
        "--inputs",
        r#"{"count": 1}"#,
        "--inputs-file",
        inputs_file.to_str().unwrap(),
    ]);
    assert_eq!(exit_code(&out), 2);
    assert_eq!(stdout_json(&out)["error"]["code"], "cli.usage");
}

#[test]
fn workflow_run_query_step_surfaces_typed_denial() {
    // The empty policy store denies every capability call — the live
    // lane must surface the Gateway's typed denial verbatim (exit 3),
    // proving the step really reached the authoritative Gateway.
    let dir = scratch("wfdeny");
    let wf = write_file(
        &dir,
        "wf.yaml",
        r"
schema_version: 1
workflow_id: test.e2e-denied
version: 1
steps:
  - kind: query
    id: q
    capability: synvelis.context.query@1.0
    args:
      selector:
        all: true
limits:
  max_steps: 10
  max_depth: 4
  max_iterations: 10
  max_runtime_ms: 1000
  max_output_bytes: 1024
  max_memory_bytes: 1024
",
    );
    let project = project_dir("wfdeny-project");
    let out = cli(&[
        "--format",
        "json",
        "workflow",
        "run",
        "--file",
        wf.to_str().unwrap(),
        "--project",
        project.to_str().unwrap(),
    ]);
    assert_eq!(
        exit_code(&out),
        3,
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let env = stdout_json(&out);
    assert_eq!(env["status"], "failed");
    assert_eq!(env["error"]["code"], "denied");
    assert_eq!(env["result"]["state"], "failed");
    assert_eq!(env["result"]["executed"], true);
    assert!(env["result"]["run_id"].as_str().is_some());
}

/// A `.luau` action script producing a single-member `ActionRecipe` —
/// `contextQuery` resolves through the canonical surface to
/// `synvelis.context.query@1.0`.
fn action_script_luau() -> String {
    format!(
        r#"
return {{
  members = {{
    {{
      name = "contextQuery",
      args = {{ selector = {{ all = true }} }},
      project = "{}",
      consistency = {{ kind = "latest" }},
    }},
  }},
  request_id = "req-e2e-action-1",
}}
"#,
        ProjectId::new(7).to_tagged()
    )
}

#[test]
fn action_run_luau_reaches_gateway_and_surfaces_denial() {
    let dir = scratch("actluau");
    let script = write_file(&dir, "probe.luau", &action_script_luau());
    let project = project_dir("actluau-project");
    let out = cli(&[
        "--format",
        "json",
        "action",
        "run",
        "--file",
        script.to_str().unwrap(),
        "--project",
        project.to_str().unwrap(),
    ]);
    // The recipe lowers to a real wire request; the empty policy store
    // denies it — the typed gateway denial is the honest outcome.
    assert_eq!(
        exit_code(&out),
        3,
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let env = stdout_json(&out);
    assert_eq!(env["status"], "failed");
    assert_eq!(env["error"]["code"], "denied");
}

#[test]
fn action_run_luau_dry_run_reports_recipe_without_executing() {
    let dir = scratch("actdry");
    let script = write_file(&dir, "probe.luau", &action_script_luau());
    let project = project_dir("actdry-project");
    let out = cli(&[
        "--format",
        "json",
        "action",
        "run",
        "--file",
        script.to_str().unwrap(),
        "--project",
        project.to_str().unwrap(),
        "--dry-run",
    ]);
    assert_eq!(
        exit_code(&out),
        0,
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let env = stdout_json(&out);
    assert_eq!(env["status"], "completed");
    let r = &env["result"];
    assert_eq!(r["kind"], "action-script");
    assert_eq!(r["language"], "luau");
    assert_eq!(r["evaluated"], true);
    assert_eq!(r["executed"], false);
    assert_eq!(r["dry_run"], true);
    assert_eq!(r["recipe_kind"], "single");
    assert_eq!(r["members"], 1);
    assert_eq!(r["project"], ProjectId::new(7).to_tagged());
    assert_eq!(r["recipe_digest"].as_str().unwrap().len(), 64);
}

// ── AUTX-CLI-RUN-FILE-02: fail-closed lanes (synvelis#262 AC9) ──

#[test]
fn workflow_run_atomic_macro_rejects_typed() {
    // `MacroRegistry::empty()` is wired intentionally — a named macro
    // reference must fail closed at compile time, never guess-resolve.
    let dir = scratch("wmacro");
    let wf = write_file(
        &dir,
        "wf.yaml",
        r"
schema_version: 1
workflow_id: test.e2e-macro
version: 1
inputs:
  - name: track_name
    schema:
      type: string
      max_len: 256
steps:
  - kind: atomic-macro
    id: build
    mac: test.track-lane@1.0
    args:
      track_name: $input.track_name
      project: project-00000000000000000000000000000007
  - kind: emit-result
    id: done
    output: ok
    value: true
outputs:
  - name: ok
    schema:
      type: bool
limits:
  max_steps: 10
  max_depth: 4
  max_iterations: 10
  max_runtime_ms: 1000
  max_output_bytes: 1024
  max_memory_bytes: 1024
",
    );
    let project = project_dir("wmacro-project");
    let out = cli(&[
        "--format",
        "json",
        "workflow",
        "run",
        "--file",
        wf.to_str().unwrap(),
        "--project",
        project.to_str().unwrap(),
        "--inputs",
        r#"{"track_name":"x"}"#,
    ]);
    assert_eq!(
        exit_code(&out),
        2,
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let env = stdout_json(&out);
    assert_eq!(env["status"], "failed");
    assert_eq!(env["error"]["code"], "cli.invalid-input");
}

#[test]
fn workflow_run_job_step_rejects_typed() {
    // `synvelis.job.*` capabilities are not registered in this build
    // (no JobEngine) — an async-submit step must fail closed at plan
    // compile, never reach a lane that cannot observe the job.
    let dir = scratch("wjob");
    let wf = write_file(
        &dir,
        "wf.yaml",
        r"
schema_version: 1
workflow_id: test.e2e-job
version: 1
steps:
  - kind: async-submit
    id: render
    capability: synvelis.job.render@1.0
    args:
      format: wav
  - kind: emit-result
    id: done
    output: ok
    value: true
outputs:
  - name: ok
    schema:
      type: bool
limits:
  max_steps: 10
  max_depth: 4
  max_iterations: 10
  max_runtime_ms: 1000
  max_output_bytes: 1024
  max_memory_bytes: 1024
",
    );
    let project = project_dir("wjob-project");
    let out = cli(&[
        "--format",
        "json",
        "workflow",
        "run",
        "--file",
        wf.to_str().unwrap(),
        "--project",
        project.to_str().unwrap(),
    ]);
    assert_eq!(
        exit_code(&out),
        2,
        "stdout: {}",
        String::from_utf8_lossy(&out.stdout)
    );
    let env = stdout_json(&out);
    assert_eq!(env["status"], "failed");
    assert_eq!(env["error"]["code"], "cli.invalid-input");
}

// ── AUTX-CLI-NOSCRIPT-01: no `script run`, no `--script` ────────

#[test]
fn no_script_command_or_flag() {
    for argv in [
        vec!["script", "run", "--file", "x.luau"],
        vec!["action", "run", "--script", "x.luau"],
        vec!["workflow", "run", "--script", "x.yaml"],
    ] {
        let mut full = vec!["--format", "json"];
        full.extend(argv.iter().copied());
        let out = cli(&full);
        assert_eq!(exit_code(&out), 2, "{argv:?}");
        let env = stdout_json(&out);
        assert_eq!(env["status"], "failed");
        assert_eq!(env["error"]["code"], "cli.usage");
    }
}

// ── AUTX-CLI-PROJ-01: project info ──────────────────────────────

#[test]
fn project_info_reports_saved_state() {
    let dir = project_dir("info");
    let out = cli(&[
        "--format",
        "json",
        "project",
        "info",
        "--project",
        dir.to_str().unwrap(),
    ]);
    assert_eq!(
        exit_code(&out),
        0,
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let env = stdout_json(&out);
    assert_eq!(env["status"], "completed");
    assert_eq!(env["result"]["project_id"], ProjectId::new(7).to_tagged());
    assert_eq!(env["result"]["name"], "cli-fixture");
    assert!(env["result"]["revision"].as_u64().unwrap() >= 1);
    assert!(env["project_revision"].as_u64().unwrap() >= 1);
}

#[test]
fn project_info_fail_closed_paths() {
    // Missing directory → invalid input (2).
    let missing = scratch("missing").join("nope");
    let out = cli(&[
        "--format",
        "json",
        "project",
        "info",
        "--project",
        missing.to_str().unwrap(),
    ]);
    assert_eq!(exit_code(&out), 2);

    // Existing but empty directory → setup failure (9), not a guess.
    let empty = scratch("empty-project");
    let out = cli(&[
        "--format",
        "json",
        "project",
        "info",
        "--project",
        empty.to_str().unwrap(),
    ]);
    assert_eq!(exit_code(&out), 9);
    assert_eq!(stdout_json(&out)["error"]["code"], "cli.setup-failed");
}

// ── AUTX-CLI-INVOKE-01: gateway round-trip ──────────────────────

#[test]
fn action_invoke_context_query_round_trip() {
    let dir = project_dir("invoke");
    let out = cli(&[
        "--format",
        "json",
        "action",
        "invoke",
        "--capability",
        "synvelis.context.query@1.0",
        "--project",
        dir.to_str().unwrap(),
        "--args",
        r#"{"selector":{"all":true}}"#,
    ]);
    // Empty policy store → the Gateway deterministically denies at
    // `read-context`; the CLI must surface that typed denial verbatim
    // (exit 3, gateway `denied` code — not a CLI-side flattening).
    assert_eq!(
        exit_code(&out),
        3,
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let env = stdout_json(&out);
    assert_eq!(env["status"], "failed");
    assert_eq!(env["error"]["code"], "denied");
    assert_eq!(env["error"]["category"], "authorization");
    assert_eq!(env["project_revision"], 1);
}

#[test]
fn action_invoke_unknown_capability_version() {
    let dir = project_dir("invoke-unknown");
    let out = cli(&[
        "--format",
        "json",
        "action",
        "invoke",
        "--capability",
        "synvelis.context.query@9.9",
        "--project",
        dir.to_str().unwrap(),
    ]);
    assert_eq!(exit_code(&out), 5);
    assert_eq!(stdout_json(&out)["error"]["code"], "cli.unsupported");
}

#[test]
fn action_prepare_produces_typed_outcome() {
    let dir = project_dir("prepare");
    let out = cli(&[
        "--format",
        "json",
        "action",
        "prepare",
        "--operation",
        "synvelis.track.create@1.0",
        "--project",
        dir.to_str().unwrap(),
        "--args",
        r#"{"name":"t1"}"#,
    ]);
    // Same empty-policy reality: prepare is denied at
    // `prepare-proposal` before any proposal exists.
    assert_eq!(
        exit_code(&out),
        3,
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let env = stdout_json(&out);
    assert_eq!(env["status"], "failed");
    assert_eq!(env["error"]["code"], "denied");
}

#[test]
fn action_prepare_target_stays_inside_operation() {
    let dir = project_dir("prepare-target");
    // `synvelis.operation.prepare` is TargetContract::None — a
    // top-level wire target would be `unexpected-target` (exit 2).
    // `--target` must land inside `operation` where track.create's
    // Create contract binds it; the empty-policy rig then denies at
    // authorization (exit 3).
    let target = format!(
        r#"{{"shape":"create","parent_kind":"project","parent":"{}","creates":"track"}}"#,
        ProjectId::new(7).to_tagged()
    );
    let out = cli(&[
        "--format",
        "json",
        "action",
        "prepare",
        "--operation",
        "synvelis.track.create@1.0",
        "--project",
        dir.to_str().unwrap(),
        "--args",
        r#"{"name":"t1"}"#,
        "--target",
        &target,
    ]);
    assert_eq!(
        exit_code(&out),
        3,
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(stdout_json(&out)["error"]["code"], "denied");
}

#[test]
fn action_prepare_dry_run_pins_boot_revision() {
    let dir = project_dir("prepare-dryrun");
    // --dry-run boots read-only: the expected-revision consistency
    // must pin the *boot* revision (1), not fall back to 0 — a wrong
    // pin fails before authorization with a revision error, not
    // `denied`.
    let out = cli(&[
        "--format",
        "json",
        "action",
        "prepare",
        "--operation",
        "synvelis.track.create@1.0",
        "--project",
        dir.to_str().unwrap(),
        "--args",
        r#"{"name":"t1"}"#,
        "--dry-run",
    ]);
    assert_eq!(
        exit_code(&out),
        3,
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(stdout_json(&out)["error"]["code"], "denied");
}

#[test]
fn interactive_claim_fails_closed() {
    let dir = project_dir("session");
    // A bogus interactive credential can never verify against the
    // (empty) session store — the claim must not escalate.
    let out = cli(&[
        "--format",
        "json",
        "--session-credential",
        "forged",
        "action",
        "invoke",
        "--capability",
        "synvelis.context.query@1.0",
        "--project",
        dir.to_str().unwrap(),
        "--args",
        r#"{"selector":{"all":true}}"#,
    ]);
    let env = stdout_json(&out);
    assert_eq!(env["status"], "failed");
    assert_eq!(
        exit_code(&out),
        3,
        "credential rejection must be Denied-class"
    );
}

// ── AUTX-CLI-DRYRUN-01: --dry-run side-effect freedom ───────────

#[test]
fn dry_run_leaves_project_untouched() {
    let dir = project_dir("dryrun");
    let snapshot = |root: &Path| -> Vec<(String, Vec<u8>)> {
        let mut files = Vec::new();
        for entry in walkdir(root) {
            files.push((
                entry
                    .strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
                std::fs::read(&entry).unwrap_or_default(),
            ));
        }
        files.sort();
        files
    };
    let before = snapshot(&dir);

    let out = cli(&[
        "--format",
        "json",
        "action",
        "invoke",
        "--capability",
        "synvelis.context.query@1.0",
        "--project",
        dir.to_str().unwrap(),
        "--args",
        r#"{"selector":{"all":true}}"#,
        "--dry-run",
    ]);
    // dry-run still reports the gateway outcome (query is read-only);
    // whatever it decides, the bundle bytes must be identical.
    let _ = stdout_json(&out);
    let after = snapshot(&dir);
    assert_eq!(
        before, after,
        "--dry-run must not modify the project bundle"
    );
}

fn walkdir(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                stack.push(path);
            } else {
                out.push(path);
            }
        }
    }
    out
}

// ── AUTX-CLI-USAGE-01: parser contract ──────────────────────────

#[test]
fn usage_errors_are_typed() {
    for argv in [
        vec!["action", "invoke"],                      // missing --capability
        vec!["action", "prepare", "--operation", "x"], // ref without @ver
        vec!["action", "run", "--file", "x.luau"],     // missing --project
        vec!["workflow", "run"],                       // missing --file
        vec!["project"],                               // missing subcommand
        vec!["render", "--output", "x.wav"],           // missing --project
        vec!["action", "list", "extra"],               // stray positional
        vec!["action", "invoke", "--capability", "x@1.0", "--bogus"], // unknown flag
    ] {
        let mut full = vec!["--format", "json"];
        full.extend(argv.iter().copied());
        let out = cli(&full);
        assert_eq!(exit_code(&out), 2, "{argv:?}");
        assert_eq!(stdout_json(&out)["error"]["code"], "cli.usage", "{argv:?}");
    }

    // An invalid --format value itself cannot be reported as JSON —
    // it must still exit 2 with a human-readable diagnostic.
    let out = cli(&["--format", "yaml", "action", "list"]);
    assert_eq!(exit_code(&out), 2);
    assert!(String::from_utf8_lossy(&out.stderr).contains("--format"));
}

#[test]
fn render_fails_closed_unsupported() {
    let dir = project_dir("render");
    let out = cli(&[
        "--format",
        "json",
        "render",
        "--project",
        dir.to_str().unwrap(),
        "--output",
        dir.join("out.wav").to_str().unwrap(),
    ]);
    assert_eq!(exit_code(&out), 5);
    assert_eq!(stdout_json(&out)["error"]["code"], "cli.unsupported");
}

// ── synvelis#515 — OS credential session supply ────────────────

/// `session issue` → `session list` → `session revoke` roundtrip on
/// the durable store + OS credential vault (Windows Credential
/// Manager). Asserts the secret never reaches stdout or the store
/// file — only the one-time vault handoff carries it.
#[test]
#[cfg(windows)]
fn session_issue_list_revoke_roundtrip() {
    let dir = scratch("sv515-session");
    let store = dir.join("sessions.json");
    let store_arg = store.to_str().unwrap().to_owned();

    let out = cli(&[
        "--format",
        "json",
        "--session-store",
        &store_arg,
        "session",
        "issue",
        "--user",
        "e2e-user",
    ]);
    assert_eq!(
        exit_code(&out),
        0,
        "issue failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let env = stdout_json(&out);
    let session_id = env["result"]["session"]
        .as_str()
        .expect("session id")
        .to_owned();
    assert!(session_id.starts_with("cli-"));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        !stdout.contains("syv-cli-"),
        "usable secret must never be printed: {stdout}"
    );

    let out = cli(&[
        "--format",
        "json",
        "--session-store",
        &store_arg,
        "session",
        "list",
    ]);
    assert_eq!(exit_code(&out), 0);
    let env = stdout_json(&out);
    let sessions = env["result"]["sessions"].as_array().unwrap();
    assert!(
        sessions.iter().any(|s| s["session"] == session_id),
        "issued session must be listed: {sessions:?}"
    );
    assert!(
        !env.to_string().contains("syv-cli-"),
        "list output must not carry secrets"
    );

    let out = cli(&[
        "--format",
        "json",
        "--session-store",
        &store_arg,
        "session",
        "revoke",
        &session_id,
    ]);
    assert_eq!(exit_code(&out), 0);

    // The durable file is digest-only — no usable credential format.
    let text = std::fs::read_to_string(&store).unwrap();
    assert!(text.contains(&session_id));
    assert!(!text.contains("syv-cli-"), "secret format on disk");
}

/// `--session <id>` resolves the vault credential into an interactive
/// claim: a gateway command must get *past* authentication (any
/// later denial is authorization, not auth). A bogus session id fails
/// closed before dispatch.
#[test]
#[cfg(windows)]
fn session_claim_reaches_verified_human_attestation() {
    let dir = scratch("sv515-claim");
    let store = dir.join("sessions.json");
    let store_arg = store.to_str().unwrap().to_owned();
    let project = project_dir("sv515-claim-project");
    let project_arg = project.to_str().unwrap().to_owned();

    let out = cli(&[
        "--format",
        "json",
        "--session-store",
        &store_arg,
        "session",
        "issue",
        "--user",
        "e2e-user",
    ]);
    assert_eq!(exit_code(&out), 0);
    let session_id = stdout_json(&out)["result"]["session"]
        .as_str()
        .unwrap()
        .to_owned();

    // A gateway command under --session: attestation runs inside the
    // host. Whatever the authorization outcome, the error category
    // must not be `authentication` — the credential verified.
    let out = cli(&[
        "--format",
        "json",
        "--session-store",
        &store_arg,
        "--session",
        &session_id,
        "--project",
        &project_arg,
        "action",
        "invoke",
        "--capability",
        "synvelis.context.query@1.0",
        "--args",
        "{}",
    ]);
    let env = stdout_json(&out);
    if let Some(err) = env.get("error") {
        assert_ne!(
            err["category"], "authentication",
            "verified session must pass attestation: {env}"
        );
    }

    // Unknown session id → CLI-side fail closed (no vault entry).
    let out = cli(&[
        "--format",
        "json",
        "--session-store",
        &store_arg,
        "--session",
        "cli-0000000000000000",
        "action",
        "list",
    ]);
    assert_eq!(exit_code(&out), 2, "unknown session must fail closed");

    // --session and --session-credential are mutually exclusive.
    let out = cli(&[
        "--session",
        &session_id,
        "--session-credential",
        "syv-cli-00",
        "action",
        "list",
    ]);
    assert_eq!(exit_code(&out), 2, "conflicting claim flags must fail");

    // Revoke → vault entry deleted → --session fails closed.
    let out = cli(&[
        "--format",
        "json",
        "--session-store",
        &store_arg,
        "session",
        "revoke",
        &session_id,
    ]);
    assert_eq!(exit_code(&out), 0);
    let out = cli(&[
        "--format",
        "json",
        "--session-store",
        &store_arg,
        "--session",
        &session_id,
        "action",
        "list",
    ]);
    assert_eq!(exit_code(&out), 2, "revoked session must fail closed");
}

/// Non-Windows platforms honestly report the OS credential path as
/// Unsupported — no silent fallback, no fabricated verification.
#[test]
#[cfg(not(windows))]
fn session_credential_path_reports_unsupported() {
    let out = cli(&["--format", "json", "session", "issue"]);
    assert_eq!(exit_code(&out), 5, "issue must report Unsupported");
    let out = cli(&[
        "--format",
        "json",
        "--session",
        "cli-0000000000000000",
        "action",
        "list",
    ]);
    assert_eq!(exit_code(&out), 5, "--session must report Unsupported");
}
