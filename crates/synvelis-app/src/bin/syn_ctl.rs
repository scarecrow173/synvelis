//! `syn-ctl` — the synvelis verification control CLI (Issue #600
//! stage 2).
//!
//! The agent-facing control entry for the repo's drive surface: a
//! thin CLI over the `rvf.uitest.v1` client ([`TestDriver`]), shaped
//! after Ruviform's `rvf-ctl` (`crates/ruviform-ui-test/src/bin/
//! rvf_ctl.rs`). This slice ships `doctor` — the read-only health
//! probe — and `drive`, the minimal live-proof probe that injects
//! one `syn.app.info` intent through the `ipc_emit` fixture and
//! journals the receipts. Both spawn `synvelis-native` built with
//! `--features uitest` (default: the sibling binary of this one in
//! the same target dir).
//!
//! `doctor` runs the `test.hello` + `test.capabilities` handshake,
//! verifies the app identity (`synvelis` + this binary's pinned
//! ruviform revision), requests a graceful `test.shutdown`, confirms
//! the child exits, and prints one structured JSON report. `drive`
//! emits a JSONL transcript — context, one line per control op with
//! its params and result, child exit, verdict.
//!
//! Exit code: `0` when every check/step passes (or a `--dry-run`
//! plan was printed); `1` on `fail`, or `not_run` +
//! `environment_blocked` when the AUT binary is missing or
//! `RVF_TEST_DISABLE` is set on this host; `2` on CLI misuse.
//! `--help` prints usage and exits `0`.
//!
//! **RT contract:** non-RT only — test plumbing, never a live audio
//! path.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use ruviform_ui_test::transport::disabled_from_env;
use ruviform_ui_test::{DriverError, ShutdownMode, TestDriver, Welcome};
use serde_json::{Value, json};
use synvelis_app::HOST_REVISION;

/// Lane labels written into reports/transcripts — distinguishes
/// `syn-ctl` output from lane-runner evidence.
const DOCTOR_LANE: &str = "syn-ctl-doctor";
const DRIVE_LANE: &str = "syn-ctl-drive";

/// The `rvf.ipc.v1` intent `drive` injects through `ipc_emit` — a
/// read-only `syn.*` op crossing the app's real inbound boundary.
const DRIVE_INTENT_ID: &str = "syn-ctl-drive-1";
const DRIVE_OP: &str = "syn.app.info";

const USAGE: &str = "\
syn-ctl — the synvelis verification control CLI (Issue #600 stage 2)

usage:
  syn-ctl doctor [--app <path>] [--transport stdio|tcp] [--dry-run] [--out <path>]
  syn-ctl drive  [--app <path>] [--transport stdio|tcp] [--dry-run] [--out <path>]
  syn-ctl --help | -h

subcommands:
  doctor   spawn the app-under-test over rvf.uitest.v1, run the
           test.hello + test.capabilities handshake, verify the app
           identity (synvelis + pinned ruviform revision), request a
           graceful test.shutdown, confirm the child exits, and print
           one JSON health report (read-only — no mutating op is sent)
  drive    spawn the app-under-test, observe its windows, inject one
           syn.app.info intent through the ipc_emit fixture, read the
           receipt journal, shut down gracefully, and write a JSONL
           transcript — the live-proof leg of evidence/sv600/

options (doctor, drive):
  --app <path>        AUT binary to drive; default: the
                      `synvelis-native` sibling of this binary (same
                      target dir — a `--features uitest` build)
  --transport <t>     `stdio` (default) or `tcp` — loopback, the driver
                      owns the ephemeral listener + run-bound secret
  --dry-run           print the execution plan JSON and exit — spawns
                      nothing, never touches the AUT
  --out <path>        write the report/transcript to <path> instead of
                      stdout
  --help, -h          this help

exit code:
  0  every check/step passed, or a plan was printed
  1  verdict `fail`, or `not_run` + environment_blocked (AUT binary
     missing, or RVF_TEST_DISABLE set on this host)
  2  CLI misuse (unknown subcommand/flag, missing value)

environment:
  The AUT side is fail-closed: it serves only when the driver opts it
  in via RVF_TEST_CONTROL (the driver sets it) and only when built
  with `--features uitest`. RVF_TEST_DISABLE on this host refuses
  every serve path — detected before spawn as environment_blocked
  (a disabled or non-uitest AUT takes the normal product path and
  never answers the stdio handshake).

next steps when a run fails:
  AUT binary missing    → mise run build-app-uitest (+ syn-ctl build:
                          cargo build -p synvelis-app --features uitest
                          --bin syn-ctl --locked)
  handshake refused     → read the AUT stderr above: RVF_TEST_DISABLE
                          set, a non-uitest build, or an app that
                          exits before serve; a stalled stdio hello
                          means the AUT never entered the seam
  capabilities fail     → the session is live but test.capabilities
                          failed — compare welcome.v against
                          PROTOCOL_VERSION (= 1)
  identity fail         → wrong AUT: welcome.app must be name
                          `synvelis`, this binary's version, and
                          revision = the Cargo.lock ruviform pin
  everything green      → drive it: syn-ctl drive, or read
                          .agents/skills/verify-synvelis/SKILL.md";

/// Transport selection for the control channel.
#[derive(Clone, Copy)]
enum Transport {
    /// Piped stdin/stdout (default).
    Stdio,
    /// Loopback TCP with a run-bound secret.
    Tcp,
}

/// Parsed subcommand options — `doctor` and `drive` share the set.
struct Options {
    /// `--app <path>`; `None` resolves to the sibling `synvelis-native`.
    app: Option<PathBuf>,
    /// `--transport` selection.
    transport: Transport,
    /// `--dry-run` — plan only.
    dry_run: bool,
    /// `--out <path>`; `None` writes to stdout.
    out: Option<PathBuf>,
}

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let Some(sub) = args.next() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    match sub.as_str() {
        "--help" | "-h" => {
            println!("{USAGE}");
            ExitCode::SUCCESS
        }
        "doctor" | "drive" => {
            let Some(options) = parse_args(&sub, args) else {
                return ExitCode::from(2);
            };
            if sub == "doctor" {
                doctor(&options)
            } else {
                drive(&options)
            }
        }
        _ => {
            eprintln!("syn-ctl: unknown argument `{sub}`\n{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// Thin `env::args` parse matching the sibling lane binaries —
/// `None` reports the error to stderr and yields exit 2.
fn parse_args(sub: &str, args: impl Iterator<Item = String>) -> Option<Options> {
    let mut args = args;
    let mut options = Options {
        app: None,
        transport: Transport::Stdio,
        dry_run: false,
        out: None,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--app" => {
                let Some(value) = args.next() else {
                    eprintln!("syn-ctl: --app requires a value");
                    return None;
                };
                options.app = Some(PathBuf::from(value));
            }
            "--transport" => {
                let Some(value) = args.next() else {
                    eprintln!("syn-ctl: --transport requires a value");
                    return None;
                };
                options.transport = match value.as_str() {
                    "stdio" => Transport::Stdio,
                    "tcp" => Transport::Tcp,
                    other => {
                        eprintln!("syn-ctl: unknown transport `{other}` (stdio|tcp)");
                        return None;
                    }
                };
            }
            "--dry-run" => options.dry_run = true,
            "--out" => {
                let Some(value) = args.next() else {
                    eprintln!("syn-ctl: --out requires a value");
                    return None;
                };
                options.out = Some(PathBuf::from(value));
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            _ => {
                eprintln!("syn-ctl: {sub}: unknown argument `{arg}`");
                return None;
            }
        }
    }
    Some(options)
}

/// The default AUT: the `synvelis-native` binary next to this one —
/// the same `target/<profile>/` dir `mise run build-app-uitest`
/// produces. Overridable with `--app` for other AUTs.
fn app_path(options: &Options) -> PathBuf {
    options.app.clone().unwrap_or_else(|| {
        let exe_name = format!("synvelis-native{}", std::env::consts::EXE_SUFFIX);
        std::env::current_exe().map_or_else(
            |_| PathBuf::from(&exe_name),
            |exe| {
                exe.parent()
                    .map_or_else(|| PathBuf::from(&exe_name), |dir| dir.join(&exe_name))
            },
        )
    })
}

/// The repo commit: `GITHUB_SHA` (CI) → `git rev-parse HEAD` →
/// `None` recorded as `null` — never guessed.
fn repo_sha() -> Option<String> {
    if let Ok(sha) = std::env::var("GITHUB_SHA") {
        return Some(sha);
    }
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|sha| sha.trim().to_string())
        .filter(|sha| !sha.is_empty())
}

/// One `checks[]` row — `verdict` uses the shared vocabulary.
fn check(id: &str, verdict: &str, detail: Option<String>) -> Value {
    let mut c = json!({ "id": id, "verdict": verdict });
    if let Some(d) = detail {
        c["detail"] = json!(d);
    }
    c
}

/// The failure record plus the fix the operator should try next.
fn failure(stage: &str, detail: &str) -> Value {
    let next_step = match stage {
        "spawn" => {
            "rebuild the AUT — mise run build-app-uitest (a custom AUT needs \
             --app <path>); syn-ctl itself builds via cargo build -p \
             synvelis-app --features uitest --bin syn-ctl --locked"
        }
        "handshake" => {
            "read the AUT stderr above — refusals include RVF_TEST_DISABLE set, \
             a non-uitest build, or an app that exits before serve; a stalled \
             stdio hello means the AUT never entered the seam (kill the spawned \
             synvelis-native PID), while tcp bounds the wait itself"
        }
        "capabilities" => {
            "the session is live but test.capabilities failed — compare \
             welcome.v against the crate's PROTOCOL_VERSION and read the \
             agent log on stderr"
        }
        "identity" => {
            "the AUT answered but is not this checkout's synvelis — point \
             --app at the synvelis-native built by mise run build-app-uitest \
             in this worktree"
        }
        "shutdown" => {
            "test.shutdown was not accepted — check for a stalled host loop \
             or a driver-gone cleanup on stderr"
        }
        _ => {
            "the child outlived test.shutdown — the driver already killed and \
             reaped it; check for a stall fixture in the AUT"
        }
    };
    json!({
        "stage": stage,
        "detail": detail,
        "next_step": next_step,
    })
}

/// Writes the report as pretty JSON to `--out` or stdout. A write
/// failure is reported to stderr and downgrades the exit to `1`.
fn emit(options: &Options, report: &Value, code: ExitCode) -> ExitCode {
    let body = serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".to_string());
    if let Some(path) = &options.out {
        match std::fs::write(path, format!("{body}\n")) {
            Ok(()) => code,
            Err(e) => {
                eprintln!("syn-ctl: cannot write {}: {e}", path.display());
                ExitCode::FAILURE
            }
        }
    } else {
        let mut out = std::io::stdout().lock();
        match writeln!(out, "{body}") {
            Ok(()) => code,
            Err(e) => {
                eprintln!("syn-ctl: stdout write failed: {e}");
                ExitCode::FAILURE
            }
        }
    }
}

/// `disabled_from_env` precheck — a truthy `RVF_TEST_DISABLE` on this
/// host makes the AUT refuse every serve path, and `synvelis-native`
/// then takes its normal product path instead of answering the hello
/// (the stdio read would stall, not fail). Reported honestly as
/// `not_run` + `environment_blocked` before anything spawns.
fn blocked_report(app: &Path, transport_name: &str) -> Option<Value> {
    if !disabled_from_env() || !app.is_file() {
        return None;
    }
    Some(json!({
        "type": "doctor",
        "lane": DOCTOR_LANE,
        "verdict": "not_run",
        "transport": transport_name,
        "app": app.display().to_string(),
        "repo": { "sha": repo_sha() },
        "environment": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "environment_kind": Value::Null,
        },
        "checks": [
            check(
                "spawn",
                "not_run",
                Some("RVF_TEST_DISABLE is set on this host — the AUT refuses every serve path".to_string()),
            ),
        ],
        "error": {
            "stage": "spawn",
            "environment_blocked": true,
            "detail": "RVF_TEST_DISABLE is set on this host",
            "next_step": "unset RVF_TEST_DISABLE and rerun — it is the kill switch, not a diagnostic",
        },
    }))
}

/// `doctor` — spawn → `test.hello` + `test.capabilities` + identity →
/// graceful `test.shutdown` → child-exit check, emitted as one JSON
/// report.
fn doctor(options: &Options) -> ExitCode {
    let app = app_path(options);
    let transport_name = match options.transport {
        Transport::Stdio => "stdio",
        Transport::Tcp => "tcp",
    };

    if options.dry_run {
        return emit(
            options,
            &doctor_plan(&app, transport_name),
            ExitCode::SUCCESS,
        );
    }

    if let Some(report) = blocked_report(&app, transport_name) {
        eprintln!("syn-ctl: RVF_TEST_DISABLE is set — the AUT refuses every serve path");
        return emit(options, &report, ExitCode::FAILURE);
    }

    let mut report = json!({
        "type": "doctor",
        "lane": DOCTOR_LANE,
        "verdict": "not_run",
        "transport": transport_name,
        "app": app.display().to_string(),
        "repo": { "sha": repo_sha() },
        "environment": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
            "environment_kind": Value::Null,
        },
    });
    let mut checks: Vec<Value> = Vec::new();
    let mut error = Value::Null;

    // The environment itself is missing — honest not_run, fail-closed.
    if !app.is_file() {
        checks.push(check(
            "spawn",
            "not_run",
            Some(format!("AUT binary not found: {}", app.display())),
        ));
        report["checks"] = json!(checks);
        report["error"] = json!({
            "stage": "spawn",
            "environment_blocked": true,
            "detail": format!("AUT binary not found: {}", app.display()),
            "next_step": "mise run build-app-uitest (or pass --app <path>)",
        });
        eprintln!("syn-ctl: AUT binary not found: {}", app.display());
        eprintln!("next step: mise run build-app-uitest (or pass --app <path>)");
        return emit(options, &report, ExitCode::FAILURE);
    }

    // spawn + handshake (the driver performs test.hello on connect).
    let mut cmd = Command::new(&app);
    let spawned = match options.transport {
        Transport::Stdio => TestDriver::spawn_stdio(&mut cmd),
        Transport::Tcp => TestDriver::spawn_tcp(&mut cmd),
    };
    let mut driver = match spawned {
        Ok(d) => {
            checks.push(check("spawn", "pass", None));
            checks.push(check("handshake", "pass", None));
            d
        }
        Err(e) => {
            spawn_failed(&mut report, &e);
            return emit(options, &report, ExitCode::FAILURE);
        }
    };

    // `driver` drops at scope end: its `Drop` kills and reaps a
    // still-running child — never an orphan.
    let live = run_session(&mut driver, &mut report, &mut checks, &mut error);

    report["checks"] = json!(checks);
    report["verdict"] = json!(if live { "pass" } else { "fail" });
    if !error.is_null() {
        report["error"] = error;
    }
    emit(
        options,
        &report,
        if live {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        },
    )
}

/// spawn/handshake failure: `Io` means the process or channel never
/// came up; Handshake/Timeout mean the AUT answered badly or never
/// answered (`Disconnected` is still a handshake-stage refusal — the
/// channel died at hello). Records the whole checks tail and
/// finalizes the report as `fail`.
fn spawn_failed(report: &mut Value, e: &DriverError) {
    let io_failed = matches!(e, DriverError::Io(_));
    let stage = if io_failed { "spawn" } else { "handshake" };
    report["checks"] = json!([
        check(
            "spawn",
            if io_failed { "fail" } else { "pass" },
            io_failed.then(|| e.to_string()),
        ),
        check(
            "handshake",
            if io_failed { "not_run" } else { "fail" },
            (!io_failed).then(|| e.to_string()),
        ),
        check("capabilities", "not_run", None),
        check("identity", "not_run", None),
        check("shutdown", "not_run", None),
        check("child-exit", "not_run", None),
    ]);
    let failure = failure(stage, &e.to_string());
    eprintln!("syn-ctl: {stage} failed: {e}");
    eprintln!(
        "next step: {}",
        failure["next_step"].as_str().unwrap_or_default()
    );
    report["verdict"] = json!("fail");
    report["error"] = failure;
}

/// The `--dry-run` plan: what would be spawned and which ops would be
/// sent — spawns nothing, never touches the AUT.
fn doctor_plan(app: &Path, transport_name: &str) -> Value {
    let mut plan = json!({
        "type": "doctor-plan",
        "lane": DOCTOR_LANE,
        "dry_run": true,
        "transport": transport_name,
        "app": app.display().to_string(),
        "app_exists": app.is_file(),
        "disabled_env": disabled_from_env(),
        "read_only": true,
        "ops": ["test.hello", "test.capabilities", "test.shutdown"],
        "steps": [
            "spawn the AUT with RVF_TEST_CONTROL=<transport> (piped stdio, or loopback TCP with a run-bound secret)",
            "test.hello → Welcome (v, session_id, run_id, app, ruviform_revision, backend, capabilities, windows, os, arch)",
            "test.capabilities → declared capability set + session_id",
            "identity → welcome.app.name == \"synvelis\", version == this build, revision == the Cargo.lock ruviform pin",
            "test.shutdown graceful → CleanupReport",
            "await the child exit status",
        ],
    });
    if !app.is_file() {
        plan["next_step"] = json!("AUT binary not found — mise run build-app-uitest");
    }
    plan
}

/// The identity check — prove the AUT is this checkout's synvelis,
/// not another binary that happens to speak the protocol: name
/// `synvelis`, this build's `CARGO_PKG_VERSION`, and revision ==
/// `HOST_REVISION` (the Cargo.lock ruviform pin).
fn check_identity(welcome: &Welcome, checks: &mut Vec<Value>, error: &mut Value) -> bool {
    let actual = &welcome.app;
    if actual.name == "synvelis"
        && actual.version == env!("CARGO_PKG_VERSION")
        && actual.revision.as_deref() == Some(HOST_REVISION)
    {
        checks.push(check(
            "identity",
            "pass",
            Some(format!("synvelis {} @ {HOST_REVISION}", actual.version)),
        ));
        return true;
    }
    let detail = format!(
        "welcome.app = {actual:?}; expected name \"synvelis\", version {}, revision {HOST_REVISION}",
        env!("CARGO_PKG_VERSION"),
    );
    checks.push(check("identity", "fail", Some(detail.clone())));
    let failure = failure("identity", &detail);
    if error.is_null() {
        *error = failure.clone();
    }
    eprintln!("syn-ctl: identity failed: {detail}");
    eprintln!(
        "next step: {}",
        failure["next_step"].as_str().unwrap_or_default()
    );
    false
}

/// The live session after a successful handshake: welcome →
/// capabilities → identity → graceful shutdown → child-exit.
/// Returns `false` when any stage fails; `error` records the first
/// failure.
fn run_session(
    driver: &mut TestDriver,
    report: &mut Value,
    checks: &mut Vec<Value>,
    error: &mut Value,
) -> bool {
    let mut live = true;

    report["welcome"] = serde_json::to_value(driver.welcome()).unwrap_or_default();
    report["environment"]["environment_kind"] = match driver.welcome().backend.name.as_str() {
        "gpui-windowed" => json!("real-window"),
        "headless-fake" => json!("deterministic-backend"),
        _ => Value::Null,
    };
    match driver.capabilities() {
        Ok(caps) => {
            checks.push(check("capabilities", "pass", None));
            report["capabilities"] = caps;
        }
        Err(e) => {
            live = false;
            checks.push(check("capabilities", "fail", Some(e.to_string())));
            *error = failure("capabilities", &e.to_string());
            eprintln!("syn-ctl: capabilities failed: {e}");
            eprintln!(
                "next step: {}",
                error["next_step"].as_str().unwrap_or_default()
            );
        }
    }

    // Identity — the doctor's own value over rvf-ctl's shape: prove
    // the AUT is *this* checkout's synvelis, not another binary that
    // happens to speak the protocol.
    if !check_identity(driver.welcome(), checks, error) {
        live = false;
    }

    // Graceful teardown — attempted whenever a session exists
    // (cleanup is part of the proof, not optional on failure).
    match driver.shutdown(ShutdownMode::Graceful) {
        Ok(cleanup) => {
            checks.push(check("shutdown", "pass", None));
            report["cleanup"] = serde_json::to_value(&cleanup).unwrap_or_default();
        }
        Err(e) => {
            live = false;
            checks.push(check("shutdown", "fail", Some(e.to_string())));
            if error.is_null() {
                *error = failure("shutdown", &e.to_string());
            }
            eprintln!("syn-ctl: shutdown failed: {e}");
        }
    }

    // Child-exit observation — independent of the shutdown reply.
    match driver.try_exit_status() {
        Ok(Some(status)) => {
            report["child_exit"] = json!({ "success": status.success(), "code": status.code() });
            if status.success() {
                checks.push(check("child-exit", "pass", None));
            } else {
                live = false;
                checks.push(check(
                    "child-exit",
                    "fail",
                    Some(format!("child exited {status}")),
                ));
                if error.is_null() {
                    *error = failure("child-exit", &format!("child exited {status}"));
                }
            }
        }
        Ok(None) => {
            live = false;
            let _ = driver.kill();
            report["child_exit"] = json!({
                "success": false,
                "detail": "child still running after test.shutdown — killed and reaped by the driver",
            });
            checks.push(check(
                "child-exit",
                "fail",
                Some("child still running after test.shutdown — killed".to_string()),
            ));
            if error.is_null() {
                *error = failure("child-exit", "child still running after test.shutdown");
            }
        }
        Err(e) => {
            live = false;
            checks.push(check("child-exit", "fail", Some(e.to_string())));
            if error.is_null() {
                *error = failure("child-exit", &e.to_string());
            }
        }
    }
    live
}

/// `drive` — the minimal live-proof probe: spawn → observe.windows →
/// `ipc_emit` injects `syn.app.info` → observe.receipts → graceful
/// shutdown → child exit, journaled as a JSONL transcript.
fn drive(options: &Options) -> ExitCode {
    let app = app_path(options);
    let transport_name = match options.transport {
        Transport::Stdio => "stdio",
        Transport::Tcp => "tcp",
    };

    if options.dry_run {
        return emit(
            options,
            &drive_plan(&app, transport_name),
            ExitCode::SUCCESS,
        );
    }

    let mut lines: Vec<Value> = Vec::new();
    lines.push(json!({
        "type": "context",
        "lane": DRIVE_LANE,
        "app": app.display().to_string(),
        "transport": transport_name,
        "repo": { "sha": repo_sha() },
        "environment": {
            "os": std::env::consts::OS,
            "arch": std::env::consts::ARCH,
        },
    }));

    // Host-level blocks — honest not_run records in the transcript.
    let blocked_detail = if disabled_from_env() {
        Some("RVF_TEST_DISABLE is set on this host — the AUT refuses every serve path".to_string())
    } else if !app.is_file() {
        Some(format!("AUT binary not found: {}", app.display()))
    } else {
        None
    };
    if let Some(detail) = blocked_detail {
        lines.push(json!({
            "type": "step",
            "seq": 1,
            "op": "spawn",
            "ok": false,
            "error": { "environment_blocked": true, "detail": &detail },
        }));
        lines.push(json!({ "type": "verdict", "verdict": "not_run" }));
        eprintln!("syn-ctl: {detail}");
        return emit_jsonl(options, &lines, ExitCode::FAILURE);
    }

    let mut cmd = Command::new(&app);
    let spawned = match options.transport {
        Transport::Stdio => TestDriver::spawn_stdio(&mut cmd),
        Transport::Tcp => TestDriver::spawn_tcp(&mut cmd),
    };
    let mut driver = match spawned {
        Ok(d) => d,
        Err(e) => {
            lines.push(json!({
                "type": "step",
                "seq": 1,
                "op": "test.hello",
                "ok": false,
                "error": { "stage": "handshake", "detail": e.to_string() },
            }));
            lines.push(json!({ "type": "verdict", "verdict": "fail" }));
            eprintln!("syn-ctl: handshake failed: {e}");
            return emit_jsonl(options, &lines, ExitCode::FAILURE);
        }
    };
    lines[0]["run_id"] = json!(driver.run_id());
    lines.push(json!({
        "type": "step",
        "seq": 1,
        "op": "test.hello",
        "ok": true,
        "data": serde_json::to_value(driver.welcome()).unwrap_or_default(),
    }));

    let live = drive_session(&mut driver, &mut lines);
    emit_jsonl(
        options,
        &lines,
        if live {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        },
    )
}

/// Appends one transcript line for an op result; returns `false` on
/// failure so the caller can mark the run failed and keep tearing
/// down.
fn record_step(
    seq: &mut u64,
    op: &str,
    params: Option<Value>,
    result: Result<Value, DriverError>,
    lines: &mut Vec<Value>,
) -> bool {
    *seq += 1;
    let ok = result.is_ok();
    let mut line = match result {
        Ok(data) => json!({ "type": "step", "seq": *seq, "op": op, "ok": true, "data": data }),
        Err(e) => {
            eprintln!("syn-ctl: {op} failed: {e}");
            json!({ "type": "step", "seq": *seq, "op": op, "ok": false, "error": e.to_string() })
        }
    };
    if let Some(p) = params {
        line["params"] = p;
    }
    lines.push(line);
    ok
}

/// Serializes a typed driver reply for the transcript.
fn json_of<T: serde::Serialize>(value: &T) -> Result<Value, DriverError> {
    serde_json::to_value(value).map_err(|e| DriverError::Handshake(e.to_string()))
}

/// Child-exit observation after `test.shutdown` — independent of the
/// shutdown reply; a surviving child is killed and reaped, never left
/// running. Returns `false` when the child did not exit cleanly.
fn drive_child_exit(
    driver: &mut TestDriver,
    lines: &mut Vec<Value>,
    first_error: &mut Option<String>,
) -> bool {
    match driver.try_exit_status() {
        Ok(Some(status)) => {
            lines.push(json!({
                "type": "child_exit",
                "success": status.success(),
                "code": status.code(),
            }));
            if status.success() {
                return true;
            }
        }
        Ok(None) => {
            let _ = driver.kill();
            lines.push(json!({
                "type": "child_exit",
                "success": false,
                "detail": "child still running after test.shutdown — killed and reaped by the driver",
            }));
        }
        Err(e) => {
            lines.push(json!({
                "type": "child_exit",
                "success": false,
                "detail": e.to_string(),
            }));
        }
    }
    mark_fail("child-exit", first_error);
    false
}

/// Records the first failing stage — later failures still tear down
/// but never overwrite the root cause.
fn mark_fail(stage: &str, first_error: &mut Option<String>) {
    if first_error.is_none() {
        *first_error = Some(stage.to_string());
    }
}

/// The probe body of `drive` after a live handshake:
/// observe.windows → `ipc_emit` inject → observe.receipts → graceful
/// shutdown → child exit. Returns `false` when any step failed;
/// teardown steps still run so the transcript records cleanup.
fn drive_session(driver: &mut TestDriver, lines: &mut Vec<Value>) -> bool {
    let mut seq = 1u64;
    let mut live = true;
    let mut first_error: Option<String> = None;

    // observe.windows → first ready window is the probe target.
    let mut window: Option<u64> = None;
    match driver.observe_windows() {
        Ok(list) => {
            window = list.iter().find(|w| w.ready).map(|w| w.id);
            let data = json!({ "windows": list });
            record_step(&mut seq, "test.observe.windows", None, Ok(data), lines);
        }
        Err(e) => {
            record_step(&mut seq, "test.observe.windows", None, Err(e), lines);
            live = false;
            mark_fail("observe.windows", &mut first_error);
        }
    }
    let window = match window {
        Some(id) => Some(id),
        None if live => {
            live = false;
            mark_fail("no ready window", &mut first_error);
            seq += 1;
            lines.push(json!({
                "type": "step",
                "seq": seq,
                "op": "test.fixture.prepare",
                "ok": false,
                "error": "no ready window to inject into",
            }));
            eprintln!("syn-ctl: no ready window to inject into");
            None
        }
        None => None,
    };

    // ipc_emit → the syn.app.info intent crosses the app's real
    // inbound boundary.
    if let Some(window) = window {
        let intent = json!({
            "v": 1,
            "kind": "intent",
            "id": DRIVE_INTENT_ID,
            "op": DRIVE_OP,
            "params": {},
        })
        .to_string();
        let params = json!({ "window": window, "body": intent });
        let sent = params.clone();
        let receipt = driver
            .prepare_fixture("ipc_emit", None, params)
            .and_then(|r| json_of(&r));
        if !record_step(
            &mut seq,
            "test.fixture.prepare",
            Some(json!({ "name": "ipc_emit", "params": sent })),
            receipt,
            lines,
        ) {
            live = false;
            mark_fail("fixture.prepare", &mut first_error);
        }
    }

    // observe.receipts → the journal's in-message/out-post_event pair
    // is the resulting state of the injection.
    if live {
        let page = driver.observe_receipts(0, 4096).and_then(|p| json_of(&p));
        if !record_step(&mut seq, "test.observe.receipts", None, page, lines) {
            live = false;
            mark_fail("observe.receipts", &mut first_error);
        }
    }

    // Graceful teardown — attempted whenever a session exists
    // (cleanup is part of the proof, not optional on failure).
    let cleanup = driver
        .shutdown(ShutdownMode::Graceful)
        .and_then(|c| json_of(&c));
    if !record_step(&mut seq, "test.shutdown", None, cleanup, lines) {
        live = false;
        mark_fail("shutdown", &mut first_error);
    }

    if !drive_child_exit(driver, lines, &mut first_error) {
        live = false;
    }

    if let Some(stage) = &first_error {
        lines.push(json!({
            "type": "verdict",
            "verdict": "fail",
            "error": { "stage": stage },
        }));
    } else {
        lines.push(json!({ "type": "verdict", "verdict": "pass" }));
    }
    live
}

/// The `drive --dry-run` plan — the probe's op sequence without
/// touching the AUT.
fn drive_plan(app: &Path, transport_name: &str) -> Value {
    let mut plan = json!({
        "type": "drive-plan",
        "lane": DRIVE_LANE,
        "dry_run": true,
        "transport": transport_name,
        "app": app.display().to_string(),
        "app_exists": app.is_file(),
        "disabled_env": disabled_from_env(),
        "ops": [
            "test.hello",
            "test.observe.windows",
            "test.fixture.prepare",
            "test.observe.receipts",
            "test.shutdown",
        ],
        "steps": [
            "spawn the AUT with RVF_TEST_CONTROL=<transport>",
            "test.hello → Welcome (implicit in the driver handshake)",
            "test.observe.windows → first ready window becomes the injection target",
            "test.fixture.prepare ipc_emit {window, body} → inject one syn.app.info rvf.ipc.v1 intent at the inbound boundary",
            "test.observe.receipts → journal: the in `message` / out `post_event` pair is the resulting state",
            "test.shutdown graceful → CleanupReport, then await child exit",
        ],
    });
    if !app.is_file() {
        plan["next_step"] = json!("AUT binary not found — mise run build-app-uitest");
    }
    plan
}

/// Writes the transcript as JSONL (one compact JSON object per line)
/// to `--out` or stdout. A write failure is reported to stderr and
/// downgrades the exit to `1`.
fn emit_jsonl(options: &Options, lines: &[Value], code: ExitCode) -> ExitCode {
    let mut body = String::new();
    for line in lines {
        body.push_str(&serde_json::to_string(line).unwrap_or_else(|_| "{}".to_string()));
        body.push('\n');
    }
    if let Some(path) = &options.out {
        match std::fs::write(path, body) {
            Ok(()) => code,
            Err(e) => {
                eprintln!("syn-ctl: cannot write {}: {e}", path.display());
                ExitCode::FAILURE
            }
        }
    } else {
        let mut out = std::io::stdout().lock();
        match write!(out, "{body}") {
            Ok(()) => code,
            Err(e) => {
                eprintln!("syn-ctl: stdout write failed: {e}");
                ExitCode::FAILURE
            }
        }
    }
}
