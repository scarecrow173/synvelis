//! Hand-rolled argument parser — no CLI framework dependency.
//!
//! Surface (AUTX-03 / synvelis#262):
//!
//! ```text
//! synvelis-cli [--format human|json|ndjson] [--project <path>]
//!              [--principal <id>] [--session-credential <cred>]
//!              [--yes] [--dry-run]
//!              <command>
//!
//!   --help | -h | help
//!   --version | -V
//!   action list
//!   action describe <id[@major.minor]>
//!   action prepare --operation <id[@ver]> [--target <json>]
//!                  [--args <json> | --args-file <path>]
//!                  [--consistency <json>] [--task <ref>] [--grant <ref>]
//!                  [--idempotency-key <key>]
//!   action invoke  --capability <id[@ver]> [same options as prepare]
//!   action run     --file <path> --project <path>
//!   workflow run   --file <path> --project <path>
//!   project info   --project <path>
//!   render         --project <path> --output <path>
//! ```
//!
//! There is deliberately **no** `script run` command and no `--script`
//! flag (AUTX-CLI-FILE-01): script entry is `action run --file` /
//! `workflow run --file` only.
//!
//! `--yes`, `--principal`, `--session-credential`, environment
//! variables, TTY state, and JSON fields never mint human authority —
//! they only shape the [`synvelis_gateway::cli::CliSessionClaim`] the
//! trusted adapter resolves (unattended → `Agent` always).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::Value;

use crate::error::CliError;

/// Output format selector.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub enum Format {
    /// Human-readable text on stdout; diagnostics on stderr.
    #[default]
    Human,
    /// Exactly one versioned [`crate::envelope::CliEnvelope`] JSON
    /// document on stdout.
    Json,
    /// One JSON document per line (single-shot commands emit the same
    /// single envelope; the mode exists for future streaming).
    Ndjson,
}

impl Format {
    /// Parses the `--format` value.
    ///
    /// # Errors
    /// [`CliError::Usage`] on an unknown format name.
    pub fn parse(text: &str) -> Result<Self, CliError> {
        match text {
            "human" | "text" => Ok(Self::Human),
            "json" => Ok(Self::Json),
            "ndjson" => Ok(Self::Ndjson),
            other => Err(CliError::Usage(format!(
                "unknown --format '{other}' (want human|json|ndjson)"
            ))),
        }
    }

    /// Whether stdout carries the JSON envelope contract.
    #[must_use]
    pub const fn is_machine(self) -> bool {
        matches!(self, Self::Json | Self::Ndjson)
    }
}

/// Parsed, validated invocation.
#[derive(Clone, Debug)]
pub struct Args {
    /// Output format.
    pub format: Format,
    /// `--project` root (canonicalized by the command layer).
    pub project: Option<PathBuf>,
    /// Delegated agent principal id for unattended claims.
    pub principal: String,
    /// Optional interactive session credential claim.
    pub session_credential: Option<String>,
    /// Accepted for script compatibility; confers **no** authority.
    pub yes: bool,
    /// Validate/preview only — no project/job side effects.
    pub dry_run: bool,
    /// The subcommand.
    pub command: Command,
}

/// Operation-selection flags shared by `action prepare`/`invoke`.
#[derive(Clone, Debug, Default)]
pub struct OperationFlags {
    /// Inline `--args` JSON (mutually exclusive with `--args-file`).
    pub args: Option<Value>,
    /// `--args-file` path (read + parsed by the command layer).
    pub args_file: Option<PathBuf>,
    /// `--target` JSON object.
    pub target: Option<Value>,
    /// `--consistency` JSON object.
    pub consistency: Option<Value>,
    /// `--task` reference.
    pub task: Option<String>,
    /// `--grant` reference.
    pub grant: Option<String>,
    /// `--idempotency-key`.
    pub idempotency_key: Option<String>,
}

/// The subcommand.
#[derive(Clone, Debug)]
pub enum Command {
    /// `--help` / `help`.
    Help,
    /// `--version`.
    Version,
    /// `action list`.
    ActionList,
    /// `action describe <ref>`.
    ActionDescribe {
        /// `id` or `id@major.minor`.
        capability: String,
    },
    /// `action prepare --operation <ref> …`.
    ActionPrepare {
        /// `id@major.minor` of the operation to prepare.
        operation: String,
        /// Shared operation flags.
        flags: OperationFlags,
    },
    /// `action invoke --capability <ref> …`.
    ActionInvoke {
        /// `id@major.minor` (version required for invoke).
        capability: String,
        /// Shared operation flags.
        flags: OperationFlags,
    },
    /// `action run --file <path>`.
    ActionRun {
        /// Script file (`.luau` Action Script; other kinds reject).
        file: PathBuf,
    },
    /// `workflow run --file <path>`.
    WorkflowRun {
        /// Workflow file (`.yaml`/`.yml` DSL, `.luau`/`.rhai` script).
        file: PathBuf,
    },
    /// `project info`.
    ProjectInfo,
    /// `render --output <path>`.
    Render {
        /// Output media path.
        output: PathBuf,
    },
}

/// A parse failure plus the format detected before the failure (so the
/// error itself honors `--format json`).
#[derive(Debug)]
pub struct ParseFailure {
    /// The error to report.
    pub error: CliError,
    /// Format seen before the failure (defaults to human).
    pub format: Format,
}

const GLOBAL_VALUE_FLAGS: &[&str] = &["format", "project", "principal", "session-credential"];
const GLOBAL_BOOL_FLAGS: &[&str] = &["yes", "dry-run", "help", "version"];
const COMMAND_VALUE_FLAGS: &[&str] = &[
    "capability",
    "operation",
    "target",
    "args",
    "args-file",
    "consistency",
    "task",
    "grant",
    "idempotency-key",
    "file",
    "output",
];

/// Parses `argv` (excluding the program name).
///
/// # Errors
/// [`ParseFailure`] — usage error; `format` is populated when `--format`
/// was seen before the failure point.
pub fn parse(argv: &[String]) -> Result<Args, ParseFailure> {
    // Pre-scan for --format so error reporting honors it.
    let mut format = Format::Human;
    let mut i = 0;
    while i < argv.len() {
        let (flag, inline) = split_flag(&argv[i]);
        let flag = flag.strip_prefix("--").unwrap_or(flag);
        if flag == "format" {
            let value = inline
                .map(str::to_owned)
                .or_else(|| argv.get(i + 1).cloned());
            match value {
                Some(v) => {
                    format = Format::parse(&v).map_err(|e| ParseFailure { error: e, format })?;
                }
                None => {
                    return Err(ParseFailure {
                        error: CliError::Usage("--format requires a value".to_owned()),
                        format,
                    });
                }
            }
        }
        i += 1;
    }

    let parsed = parse_inner(argv).map_err(|error| ParseFailure { error, format })?;
    Ok(Args { format, ..parsed })
}

/// Global (non-command) options collected while scanning argv.
/// `help`/`version` stay local to [`parse_inner`] — they short-circuit
/// before dispatch and are not invocation options.
struct Invocation {
    format: Format,
    project: Option<PathBuf>,
    principal: String,
    session_credential: Option<String>,
    yes: bool,
    dry_run: bool,
}

impl Invocation {
    fn new() -> Self {
        Self {
            format: Format::Human,
            project: None,
            principal: "synvelis-cli".to_owned(),
            session_credential: None,
            yes: false,
            dry_run: false,
        }
    }

    fn into_args_with(self, command: Command) -> Args {
        Args {
            format: self.format,
            project: self.project,
            principal: self.principal,
            session_credential: self.session_credential,
            yes: self.yes,
            dry_run: self.dry_run,
            command,
        }
    }

    fn into_args(
        self,
        positionals: &[String],
        flags: &BTreeMap<String, String>,
    ) -> Result<Args, CliError> {
        let command = dispatch(positionals, flags)?;
        Ok(self.into_args_with(command))
    }
}

fn parse_inner(argv: &[String]) -> Result<Args, CliError> {
    let mut inv = Invocation::new();
    let mut help = false;
    let mut version = false;
    let mut positionals: Vec<String> = Vec::new();
    let mut flags: BTreeMap<String, String> = BTreeMap::new();

    let mut i = 0;
    while i < argv.len() {
        let raw = &argv[i];
        if raw == "-h" {
            help = true;
            i += 1;
            continue;
        }
        if raw == "-V" {
            version = true;
            i += 1;
            continue;
        }
        if let Some(rest) = raw.strip_prefix("--") {
            let (name, inline) = match rest.split_once('=') {
                Some((n, v)) => (n, Some(v.to_owned())),
                None => (rest, None),
            };
            let known = GLOBAL_VALUE_FLAGS.contains(&name)
                || GLOBAL_BOOL_FLAGS.contains(&name)
                || COMMAND_VALUE_FLAGS.contains(&name);
            if !known {
                return Err(CliError::Usage(format!("unknown flag --{name}")));
            }
            if GLOBAL_BOOL_FLAGS.contains(&name) {
                if inline.is_some() {
                    return Err(CliError::Usage(format!("--{name} takes no value")));
                }
                match name {
                    "yes" => inv.yes = true,
                    "dry-run" => inv.dry_run = true,
                    "help" => help = true,
                    "version" => version = true,
                    _ => unreachable!(),
                }
                i += 1;
                continue;
            }
            let value = if let Some(v) = inline {
                v
            } else {
                i += 1;
                argv.get(i)
                    .cloned()
                    .ok_or_else(|| CliError::Usage(format!("--{name} requires a value")))?
            };
            match name {
                "format" => inv.format = Format::parse(&value)?,
                "project" => inv.project = Some(PathBuf::from(value)),
                "principal" => inv.principal = value,
                "session-credential" => inv.session_credential = Some(value),
                _ => {
                    if flags.insert(name.to_owned(), value).is_some() {
                        return Err(CliError::Usage(format!("--{name} given twice")));
                    }
                }
            }
            i += 1;
            continue;
        }
        positionals.push(raw.clone());
        i += 1;
    }

    if help {
        return Ok(inv.into_args_with(Command::Help));
    }
    if version {
        return Ok(inv.into_args_with(Command::Version));
    }
    inv.into_args(&positionals, &flags)
}

fn dispatch(positionals: &[String], flags: &BTreeMap<String, String>) -> Result<Command, CliError> {
    match positionals.first().map(String::as_str) {
        Some("action") => parse_action(positionals, flags),
        Some("workflow") => parse_workflow(positionals, flags),
        Some("project") => parse_project(positionals, flags),
        Some("render") => parse_render(positionals, flags),
        Some("script") => Err(CliError::Usage(
            "there is no `script run` command; use `action run --file` or \
             `workflow run --file`"
                .to_owned(),
        )),
        Some("help") => Ok(Command::Help),
        Some(other) => Err(CliError::Usage(format!("unknown command '{other}'"))),
        None => Err(CliError::Usage("missing command (try --help)".to_owned())),
    }
}

fn flag_value<'a>(flags: &'a BTreeMap<String, String>, name: &str) -> Option<&'a str> {
    flags.get(name).map(String::as_str)
}

fn take_flags(flags: &BTreeMap<String, String>, allowed: &[&str]) -> Result<(), CliError> {
    for key in flags.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(CliError::Usage(format!(
                "--{key} is not valid for this command"
            )));
        }
    }
    Ok(())
}

fn json_flag(flags: &BTreeMap<String, String>, name: &str) -> Result<Option<Value>, CliError> {
    match flag_value(flags, name) {
        None => Ok(None),
        Some(text) => serde_json::from_str(text)
            .map(Some)
            .map_err(|e| CliError::Usage(format!("--{name} is not valid JSON: {e}"))),
    }
}

fn string_flag(flags: &BTreeMap<String, String>, name: &str) -> Option<String> {
    flag_value(flags, name).map(str::to_owned)
}

fn path_flag(flags: &BTreeMap<String, String>, name: &str) -> Option<PathBuf> {
    flag_value(flags, name).map(PathBuf::from)
}

fn operation_flags(
    flags: &BTreeMap<String, String>,
    allowed: &[&str],
) -> Result<OperationFlags, CliError> {
    take_flags(flags, allowed)?;
    let args_inline = json_flag(flags, "args")?;
    let args_file = path_flag(flags, "args-file");
    if args_inline.is_some() && args_file.is_some() {
        return Err(CliError::Usage(
            "--args and --args-file are mutually exclusive".to_owned(),
        ));
    }
    Ok(OperationFlags {
        args: args_inline,
        args_file,
        target: json_flag(flags, "target")?,
        consistency: json_flag(flags, "consistency")?,
        task: string_flag(flags, "task"),
        grant: string_flag(flags, "grant"),
        idempotency_key: string_flag(flags, "idempotency-key"),
    })
}

const OP_FLAGS: &[&str] = &[
    "target",
    "args",
    "args-file",
    "consistency",
    "task",
    "grant",
    "idempotency-key",
];

fn parse_action(
    positionals: &[String],
    flags: &BTreeMap<String, String>,
) -> Result<Command, CliError> {
    match positionals.get(1).map(String::as_str) {
        Some("list") => {
            if positionals.len() != 2 {
                return Err(CliError::Usage("action list takes no arguments".to_owned()));
            }
            take_flags(flags, &[])?;
            Ok(Command::ActionList)
        }
        Some("describe") => {
            if positionals.len() != 3 {
                return Err(CliError::Usage(
                    "action describe requires <id[@major.minor]>".to_owned(),
                ));
            }
            take_flags(flags, &[])?;
            Ok(Command::ActionDescribe {
                capability: positionals[2].clone(),
            })
        }
        Some("prepare") => {
            if positionals.len() != 2 {
                return Err(CliError::Usage(
                    "action prepare takes no positional arguments".to_owned(),
                ));
            }
            let mut allowed = Vec::with_capacity(OP_FLAGS.len() + 1);
            allowed.push("operation");
            allowed.extend_from_slice(OP_FLAGS);
            let f = operation_flags(flags, &allowed)?;
            let operation = string_flag(flags, "operation").ok_or_else(|| {
                CliError::Usage("action prepare requires --operation <id@ver>".to_owned())
            })?;
            Ok(Command::ActionPrepare {
                operation,
                flags: f,
            })
        }
        Some("invoke") => {
            if positionals.len() != 2 {
                return Err(CliError::Usage(
                    "action invoke takes no positional arguments".to_owned(),
                ));
            }
            let mut allowed = Vec::with_capacity(OP_FLAGS.len() + 1);
            allowed.push("capability");
            allowed.extend_from_slice(OP_FLAGS);
            let f = operation_flags(flags, &allowed)?;
            let capability = string_flag(flags, "capability").ok_or_else(|| {
                CliError::Usage("action invoke requires --capability <id@ver>".to_owned())
            })?;
            Ok(Command::ActionInvoke {
                capability,
                flags: f,
            })
        }
        Some("run") => {
            if positionals.len() != 2 {
                return Err(CliError::Usage(
                    "action run takes no positional arguments".to_owned(),
                ));
            }
            take_flags(flags, &["file"])?;
            let file = path_flag(flags, "file")
                .ok_or_else(|| CliError::Usage("action run requires --file <path>".to_owned()))?;
            Ok(Command::ActionRun { file })
        }
        Some(other) => Err(CliError::Usage(format!("unknown action '{other}'"))),
        None => Err(CliError::Usage(
            "action requires a subcommand (list|describe|prepare|invoke|run)".to_owned(),
        )),
    }
}

fn parse_workflow(
    positionals: &[String],
    flags: &BTreeMap<String, String>,
) -> Result<Command, CliError> {
    match positionals.get(1).map(String::as_str) {
        Some("run") => {
            if positionals.len() != 2 {
                return Err(CliError::Usage(
                    "workflow run takes no positional arguments".to_owned(),
                ));
            }
            take_flags(flags, &["file"])?;
            let file = path_flag(flags, "file")
                .ok_or_else(|| CliError::Usage("workflow run requires --file <path>".to_owned()))?;
            Ok(Command::WorkflowRun { file })
        }
        Some(other) => Err(CliError::Usage(format!("unknown workflow '{other}'"))),
        None => Err(CliError::Usage(
            "workflow requires a subcommand (run)".to_owned(),
        )),
    }
}

fn parse_project(
    positionals: &[String],
    flags: &BTreeMap<String, String>,
) -> Result<Command, CliError> {
    match positionals.get(1).map(String::as_str) {
        Some("info") => {
            if positionals.len() != 2 {
                return Err(CliError::Usage(
                    "project info takes no arguments".to_owned(),
                ));
            }
            take_flags(flags, &[])?;
            Ok(Command::ProjectInfo)
        }
        Some(other) => Err(CliError::Usage(format!("unknown project '{other}'"))),
        None => Err(CliError::Usage(
            "project requires a subcommand (info)".to_owned(),
        )),
    }
}

fn parse_render(
    positionals: &[String],
    flags: &BTreeMap<String, String>,
) -> Result<Command, CliError> {
    if positionals.len() != 1 {
        return Err(CliError::Usage(
            "render takes no positional arguments".to_owned(),
        ));
    }
    take_flags(flags, &["output"])?;
    let output = path_flag(flags, "output")
        .ok_or_else(|| CliError::Usage("render requires --output <path>".to_owned()))?;
    Ok(Command::Render { output })
}

fn split_flag(raw: &str) -> (&str, Option<&str>) {
    match raw.split_once('=') {
        Some((f, v)) => (f, Some(v)),
        None => (raw, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn parses_help_version() {
        let a = parse(&argv(&["--help"])).unwrap();
        assert!(matches!(a.command, Command::Help));
        let a = parse(&argv(&["--version"])).unwrap();
        assert!(matches!(a.command, Command::Version));
    }

    #[test]
    fn rejects_script_run_and_script_flag() {
        assert!(parse(&argv(&["script", "run", "--file", "x.luau"])).is_err());
        assert!(parse(&argv(&["action", "run", "--script", "x.luau"])).is_err());
    }

    #[test]
    fn parses_action_invoke() {
        let a = parse(&argv(&[
            "action",
            "invoke",
            "--capability",
            "synvelis.context.query@1.0",
            "--project",
            "p",
            "--args",
            "{\"q\":1}",
            "--format",
            "json",
        ]))
        .unwrap();
        assert_eq!(a.format, Format::Json);
        assert!(matches!(a.command, Command::ActionInvoke { .. }));
    }
}
