//! `synvelis-cli` — headless command line entry point (AUTX-03).
//!
//! Thin adapter over the existing Capability Gateway: argument parsing,
//! the versioned `CliEnvelope` stdout contract, exit-code mapping, and
//! host/session boot live in [`synvelis_cli`]; every operation flows
//! through `HeadlessSessionHost`/`Gateway` — no second authority, no
//! GUI, no Native/WebView/GPUI dependency.

use std::process::ExitCode;

use synvelis_cli::args::{self, Command, Format};
use synvelis_cli::commands::{self, CommandResult};
use synvelis_cli::envelope::EnvelopeStatus;
use synvelis_cli::error::CliError;

const HELP: &str = "\
synvelis-cli — headless Synvelis command line (Capability Gateway adapter)

USAGE:
    synvelis-cli [global options] <command>

GLOBAL OPTIONS:
    --format <human|json|ndjson>   Output format (default: human)
    --project <path>               Project bundle root (required by
                                   project-bound commands)
    --principal <id>               Delegated agent principal id for
                                   unattended sessions (default:
                                   synvelis-cli). Never mints Human.
    --session-credential <cred>    Present an interactive session
                                   credential (verified by the trusted
                                   adapter; fails closed today)
    --yes                          Accepted for script compatibility;
                                   confers no authority
    --dry-run                      Validate/preview only — no project,
                                   filesystem, or job side effects
    --help, -h                     Show this help
    --version, -V                  Show version

COMMANDS:
    action list                    List registered capabilities
    action describe <id[@ver]>     Describe one capability (version
                                   optional — lists all versions)
    action prepare --operation <id@ver> [op options]
                                   Non-destructive mutation preview
                                   (GW-04 prepare → proposal)
    action invoke --capability <id@ver> [op options]
                                   Invoke a capability through the
                                   Gateway (query/proposal/mutation/
                                   runtime/async per descriptor)
    action run --file <path> --project <path>
                                   Run an Action Script file
                                   (.luau; .rhai PoC-only) — execution
                                   backend deferred (synvelis#272)
    workflow run --file <path> --project <path>
                                   Run a Workflow file (.yaml/.yml DSL,
                                   .luau/.rhai script) — execution
                                   deferred (synvelis#273)
    project info --project <path>  Show project id/name/revision
    render --project <path> --output <path>
                                   Render media (provider not wired —
                                   fails closed Unsupported)

OPERATION OPTIONS (prepare/invoke):
    --target <json>                Explicit target object
    --args <json> | --args-file <path>
                                   Operation arguments
    --consistency <json>           Consistency requirement
                                   (default: domain-appropriate)
    --task <ref> --grant <ref>     Authorized task / grant references
    --idempotency-key <key>        Idempotency identity

There is no `script run` command and no `--script` flag — script entry
is `action run --file` / `workflow run --file` only.

EXIT CODES:
    0 completed/proposal-ready/approval-required   5 unsupported
    2 invalid input                               6 execution failure
    3 denied                                      7 indeterminate/accepted
    4 conflict                                    8 confirmed cancelled
    9 unavailable/setup failure                   10 internal error

JSON MODE (--format json|ndjson) writes exactly one versioned
CliEnvelope {schema_version,status,operation_id?,project_revision?,
result?,error?,references?} to stdout; diagnostics go to stderr.
";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let args = match args::parse(&arguments) {
        Ok(args) => args,
        Err(failure) => {
            return emit_failure(failure.format, &failure.error);
        }
    };

    match args.command {
        Command::Help => {
            print!("{HELP}");
            return ExitCode::SUCCESS;
        }
        Command::Version => {
            println!("synvelis-cli {}", env!("CARGO_PKG_VERSION"));
            return ExitCode::SUCCESS;
        }
        _ => {}
    }

    match commands::run(&args) {
        Ok(result) => emit_result(args.format, &result),
        Err(err) => emit_failure(args.format, &err),
    }
}

fn emit_result(format: Format, result: &CommandResult) -> ExitCode {
    match format {
        Format::Human => {
            if let Some(human) = &result.human {
                print!("{human}");
            }
            if result.envelope.status == EnvelopeStatus::Failed.as_str() {
                // Human-mode failures also surface on stderr.
                if let Some(msg) = result
                    .envelope
                    .error
                    .as_ref()
                    .and_then(|e| e.get("message"))
                    .and_then(serde_json::Value::as_str)
                {
                    eprintln!("error: {msg}");
                }
            }
        }
        Format::Json | Format::Ndjson => {
            println!("{}", result.envelope.to_line());
        }
    }
    exit_code(result.exit_code)
}

fn emit_failure(format: Format, err: &CliError) -> ExitCode {
    let result = commands::cli_error_result(err);
    match format {
        Format::Human => {
            eprintln!("error: {}", err.display());
        }
        Format::Json | Format::Ndjson => {
            println!("{}", result.envelope.to_line());
        }
    }
    exit_code(result.exit_code)
}

fn exit_code(code: u8) -> ExitCode {
    ExitCode::from(code)
}
