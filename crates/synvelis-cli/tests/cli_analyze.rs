//! Real-binary e2e for `synvelis-cli script analyze --file`
//! (AUTX-SCR-ANA-01 / synvelis#274 — SV274-NR-03 headless slice).
//!
//! Every test spawns the actual `synvelis-cli` executable and asserts
//! on process exit codes plus the stdout/stderr contract — no
//! in-process shortcuts. The verb is project-less by design: no test
//! passes `--project`, and no project/policy/journal artifacts may
//! appear.

use std::path::{Path, PathBuf};
use std::process::{Command as Proc, Output};

use serde_json::Value;

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

fn stderr_text(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

fn scratch(tag: &str) -> PathBuf {
    let dir =
        std::env::temp_dir().join(format!("synvelis-cli-analyze-{}-{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn write_file(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, body).unwrap();
    path
}

fn analyze_json(file: &Path) -> (Output, Value) {
    let out = cli(&[
        "--format",
        "json",
        "script",
        "analyze",
        "--file",
        &file.display().to_string(),
    ]);
    let env = stdout_json(&out);
    (out, env)
}

// ── happy path ───────────────────────────────────────────────────

#[test]
fn clean_luau_completes_without_project() {
    let dir = scratch("clean");
    let file = write_file(&dir, "clean.luau", "local x = 1\nreturn x\n");
    let (out, env) = analyze_json(&file);
    assert_eq!(exit_code(&out), 0, "stderr: {}", stderr_text(&out));
    assert_eq!(env["schema_version"], 1);
    assert_eq!(env["status"], "completed");
    assert!(env.get("project_revision").is_none());
    let result = &env["result"];
    assert_eq!(result["language"], "luau");
    assert_eq!(result["source_revision"], 1);
    assert_eq!(
        result["diagnostics"].as_array().unwrap().len(),
        0,
        "clean source must have no diagnostics: {result}"
    );
    assert!(result.get("completions").is_none());
    assert_eq!(result["analyzer"]["analyzer_id"], "synvelis-luau-port");
    assert_eq!(result["analyzer"]["language"], "luau");
    assert_eq!(result["analyzer"]["contract_version"], 1);
    assert_eq!(result["analyzer"]["completion"], true);
    assert_eq!(result["injection"]["package_name"], "synvelis-script");
    for key in ["model_digest", "stub_digest"] {
        let digest = result["injection"][key].as_str().unwrap();
        assert_eq!(digest.len(), 64, "{key} must be a sha256 hex digest");
        assert!(digest.chars().all(|c| c.is_ascii_hexdigit()));
    }
    // Discovery boot is read-only: the scratch dir still holds only
    // the source file — no project/policy/journal artifacts.
    let entries: Vec<_> = std::fs::read_dir(&dir).unwrap().collect();
    assert_eq!(entries.len(), 1, "analyze must not write artifacts");
}

#[test]
fn unknown_api_is_a_completed_diagnostic_not_a_failure() {
    let dir = scratch("unknown-api");
    let file = write_file(&dir, "bad.luau", "local r = synvelisNope()\nreturn r\n");
    let (out, env) = analyze_json(&file);
    assert_eq!(exit_code(&out), 0, "stderr: {}", stderr_text(&out));
    assert_eq!(env["status"], "completed");
    let diagnostics = env["result"]["diagnostics"].as_array().unwrap();
    let diag = diagnostics
        .iter()
        .find(|d| d["code"] == "unknown-api")
        .unwrap_or_else(|| panic!("expected unknown-api diagnostic: {diagnostics:?}"));
    assert_eq!(diag["severity"], "error");
    assert_eq!(diag["kind"], "type");
    assert_eq!(diag["source_revision"], 1);
    assert!(diag["message"].as_str().unwrap().contains("synvelisNope"));
    assert!(diag["range"]["start"]["line"].is_u64());
    assert!(diag["range"]["start"]["column"].is_u64());
}

#[test]
fn syntax_diagnostic_is_completed_data() {
    let dir = scratch("syntax");
    let file = write_file(&dir, "broken.luau", "local s = \"unterminated\n");
    let (out, env) = analyze_json(&file);
    assert_eq!(exit_code(&out), 0, "stderr: {}", stderr_text(&out));
    assert_eq!(env["status"], "completed");
    let diagnostics = env["result"]["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics
            .iter()
            .any(|d| d["code"] == "syntax-unterminated-string" && d["kind"] == "syntax"),
        "expected syntax diagnostic: {diagnostics:?}"
    );
}

#[test]
fn wrong_arity_is_diagnosed_against_injected_signature() {
    let dir = scratch("arity");
    // First discover a live generated function + its arity from the
    // completion detail (the injected `declare function` signature).
    let probe = write_file(&dir, "probe.luau", "local f = synvelis");
    let probe_len = usize::try_from(std::fs::metadata(&probe).unwrap().len()).unwrap();
    let out = cli(&[
        "--format",
        "json",
        "script",
        "analyze",
        "--file",
        &probe.display().to_string(),
        "--cursor",
        &probe_len.to_string(),
    ]);
    assert_eq!(exit_code(&out), 0, "stderr: {}", stderr_text(&out));
    let env = stdout_json(&out);
    let completions = env["result"]["completions"].as_array().unwrap();
    assert!(
        !completions.is_empty(),
        "cursor at `synvelis` must complete"
    );
    let label = completions[0]["label"].as_str().unwrap().to_owned();
    let detail = completions[0]["detail"].as_str().unwrap();
    // `declare function name(a: T, b: U, ...): R` — arity = params + 1.
    let arity = detail
        .split_once('(')
        .and_then(|(_, rest)| rest.split_once(')'))
        .map(|(params, _)| {
            if params.trim().is_empty() {
                0
            } else {
                params.split(',').count()
            }
        })
        .expect("completion detail must carry a declare signature");
    assert!(arity > 0, "generated functions take arguments: {detail}");
    let bad_args = vec!["0"; arity + 1].join(", ");
    let file = write_file(
        &dir,
        "arity.luau",
        &format!("local r = {label}({bad_args})\n"),
    );
    let (out, env) = analyze_json(&file);
    assert_eq!(exit_code(&out), 0, "stderr: {}", stderr_text(&out));
    assert_eq!(env["status"], "completed");
    let diagnostics = env["result"]["diagnostics"].as_array().unwrap();
    assert!(
        diagnostics.iter().any(|d| d["code"] == "wrong-arity"),
        "expected wrong-arity diagnostic: {diagnostics:?}"
    );
}

// ── completion ───────────────────────────────────────────────────

#[test]
fn cursor_completion_uses_live_registry_stub() {
    let dir = scratch("completion");
    let file = write_file(&dir, "comp.luau", "local f = synvelis");
    let len = usize::try_from(std::fs::metadata(&file).unwrap().len()).unwrap();
    let out = cli(&[
        "--format",
        "json",
        "script",
        "analyze",
        "--file",
        &file.display().to_string(),
        "--cursor",
        &len.to_string(),
    ]);
    assert_eq!(exit_code(&out), 0, "stderr: {}", stderr_text(&out));
    let env = stdout_json(&out);
    assert_eq!(env["status"], "completed");
    let completions = env["result"]["completions"]
        .as_array()
        .expect("cursor request must produce completions");
    let mut saw_generated = false;
    for c in completions {
        let label = c["label"].as_str().unwrap();
        assert_ne!(label, "");
        if label.starts_with("synvelis") {
            saw_generated = true;
        }
        assert_eq!(c["kind"], "generated-function");
        assert_eq!(c["source_revision"], 1);
        // Hidden/Retired/Disabled entries never reach the stub, so no
        // completion may carry a hidden name.
        assert!(!label.to_lowercase().contains("hidden"));
        assert!(!label.to_lowercase().contains("retired"));
    }
    assert!(
        saw_generated,
        "completions must include live-registry generated functions"
    );

    // A prefix that matches nothing yields an empty (still present)
    // completions array — prefix filtering is the analyzer's job.
    let narrow_file = write_file(&dir, "narrow.luau", "local f = synvelisZz");
    let narrow_len = usize::try_from(std::fs::metadata(&narrow_file).unwrap().len()).unwrap();
    let out = cli(&[
        "--format",
        "json",
        "script",
        "analyze",
        "--file",
        &narrow_file.display().to_string(),
        "--cursor",
        &narrow_len.to_string(),
    ]);
    assert_eq!(exit_code(&out), 0, "stderr: {}", stderr_text(&out));
    let env = stdout_json(&out);
    assert_eq!(env["status"], "completed");
    assert_eq!(
        env["result"]["completions"].as_array().unwrap().len(),
        0,
        "unknown prefix must complete to an empty set"
    );
}

#[test]
fn cursor_out_of_range_is_invalid_input() {
    let dir = scratch("cursor-range");
    let file = write_file(&dir, "c.luau", "local x = 1\n");
    let out = cli(&[
        "--format",
        "json",
        "script",
        "analyze",
        "--file",
        &file.display().to_string(),
        "--cursor",
        "999999",
    ]);
    assert_eq!(exit_code(&out), 2);
    let env = stdout_json(&out);
    assert_eq!(env["status"], "failed");
    assert_eq!(env["error"]["code"], "cli.invalid-input");
}

// ── fail-closed classification matrix ────────────────────────────

#[test]
fn unsupported_and_invalid_inputs_fail_closed() {
    let dir = scratch("matrix");

    // `.rhai` — admitted as a language id; the registry reports
    // Unavailable (NR-07 has no Rhai analyzer yet) → cli.unsupported.
    let rhai = write_file(&dir, "x.rhai", "let x = 1;\n");
    let (out, env) = analyze_json(&rhai);
    assert_eq!(exit_code(&out), 5, "stderr: {}", stderr_text(&out));
    assert_eq!(env["error"]["code"], "cli.unsupported");
    assert!(env["error"]["message"].as_str().unwrap().contains("rhai"));

    // `.lua` — Lua 5.4 explicitly out of contract → cli.unsupported.
    let lua = write_file(&dir, "x.lua", "local x = 1\n");
    let (out, env) = analyze_json(&lua);
    assert_eq!(exit_code(&out), 5);
    assert_eq!(env["error"]["code"], "cli.unsupported");

    // `.yaml` — Workflow DSL is not an analyzer target → cli.invalid-input.
    for name in ["x.yaml", "x.yml", "x.txt", "noext"] {
        let f = write_file(&dir, name, "key: value\n");
        let (out, env) = analyze_json(&f);
        assert_eq!(exit_code(&out), 2, "{name} must fail with exit 2");
        assert_eq!(env["error"]["code"], "cli.invalid-input", "{name}");
    }

    // Missing file → cli.invalid-input.
    let missing = dir.join("missing.luau");
    let (out, env) = analyze_json(&missing);
    assert_eq!(exit_code(&out), 2);
    assert_eq!(env["error"]["code"], "cli.invalid-input");

    // Non-UTF-8 source → cli.invalid-input.
    let bad_utf8 = dir.join("bad.luau");
    std::fs::write(&bad_utf8, [0xff, 0xfe, 0x41]).unwrap();
    let (out, env) = analyze_json(&bad_utf8);
    assert_eq!(exit_code(&out), 2);
    assert_eq!(env["error"]["code"], "cli.invalid-input");
}

// ── no execution surface (AUTX-CLI-FILE-01 non-regression) ───────

#[test]
fn script_execution_surface_stays_rejected() {
    let dir = scratch("noexec");
    let file = write_file(&dir, "x.luau", "local x = 1\n");
    let path = file.display().to_string();

    for args in [
        vec!["script", "run", "--file", &path],
        vec!["script"],
        vec!["script", "other", "--file", &path],
        vec!["action", "run", "--file", &path], // no --project → usage
        vec!["script", "analyze", "--file", &path, "--dry-run"],
        vec!["script", "analyze"], // --file required
        vec!["script", "analyze", "--file", &path, "--bogus"],
    ] {
        let mut full = vec!["--format", "json"];
        full.extend(args.iter().copied());
        let out = cli(&full);
        assert_eq!(
            exit_code(&out),
            2,
            "{args:?} must fail with cli.usage exit 2, stderr: {}",
            stderr_text(&out)
        );
        let env = stdout_json(&out);
        assert_eq!(env["status"], "failed");
        assert_eq!(env["error"]["code"], "cli.usage", "{args:?}");
    }
}

// ── human format stdout/stderr separation ────────────────────────

#[test]
fn human_format_routes_diagnostics_to_stdout() {
    let dir = scratch("human");
    let file = write_file(&dir, "h.luau", "local r = synvelisNope()\n");
    let path = file.display().to_string();

    // Success: diagnostics are result data → stdout; stderr stays clean.
    let out = cli(&["script", "analyze", "--file", &path]);
    assert_eq!(exit_code(&out), 0, "stderr: {}", stderr_text(&out));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("error unknown-api"), "stdout: {stdout}");
    assert!(stdout.contains("synvelisNope"), "stdout: {stdout}");
    assert_eq!(
        stderr_text(&out),
        "",
        "successful analysis must not write stderr"
    );

    // Clean source → explicit no-diagnostics line.
    let clean = write_file(&dir, "clean.luau", "local x = 1\n");
    let out = cli(&["script", "analyze", "--file", &clean.display().to_string()]);
    assert_eq!(exit_code(&out), 0);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("no diagnostics"), "stdout: {stdout}");

    // Invocation failure → stderr only; no diagnostic lines on stdout.
    let missing = dir.join("missing.luau").display().to_string();
    let out = cli(&["script", "analyze", "--file", &missing]);
    assert_eq!(exit_code(&out), 2);
    assert!(stderr_text(&out).contains("error:"));
    assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "");
}
