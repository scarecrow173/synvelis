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
    let project = dir.join("proj");
    std::fs::create_dir_all(&project).unwrap();

    let cases: &[(&str, &str, i32)] = &[
        // (file, subcommand-prefix, expected exit)
        ("a.luau", "action", 5), // recognized, backend unwired → Unsupported
        ("a.luau", "workflow", 5),
        ("a.yaml", "action", 2), // DSL is workflow-only → kind mismatch
        ("a.yml", "action", 2),
        ("a.yaml", "workflow", 5), // recognized DSL, execution deferred
        ("a.yml", "workflow", 5),
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
    let project = dir.join("proj");
    std::fs::create_dir_all(&project).unwrap();

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
    let project = dir.join("proj");
    std::fs::create_dir_all(&project).unwrap();
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
