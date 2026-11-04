//! Host-owned logging integration — SYN-OBS-01 (Issue #81).
//!
//! This module is the Synvelis-side owner of the
//! [`ruviform_observability::logging`] adapter: it translates product
//! configuration (CLI flags and environment variables — collected at the
//! process entry point, never read inside this library) into a
//! [`LoggingConfig`], installs the resulting [`tracing::Dispatch`] as the
//! global default **once per process**, and keeps the `WriterGuard`s,
//! `LoggingHealth`, and degraded-sink records alive for the process
//! lifetime.
//!
//! # Ownership contract
//!
//! - **The process entry point installs the subscriber.** [`install`]
//!   calls `tracing::dispatcher::set_global_default` — the only place a
//!   Synvelis crate may do so. `SynvelisApp` and library code never
//!   install a global subscriber themselves; tests use scoped
//!   `tracing::dispatcher::with_default` instead.
//! - **The global subscriber cannot be uninstalled** once installed —
//!   that is a `tracing` structural constraint, not a choice. Switching
//!   OFF→ON (or changing filters/sinks) at runtime requires a restart.
//! - **A second [`install`] in one process never replaces the existing
//!   subscriber.** It returns [`InstallState::AlreadyInstalled`]; the
//!   sinks built by that call are shut down again and the pre-existing
//!   subscriber keeps running.
//! - **Dropping [`HostLog`] flushes and closes every sink** in order —
//!   each `WriterGuard` drains its bounded queue and joins the writer
//!   worker with a bounded timeout. Guards must live until process
//!   shutdown; the binary keeps `HostLog` bound in `main` for exactly
//!   that purpose.
//!
//! # RT contract
//!
//! **Never emit `tracing` events from real-time (RT) contexts.** The
//! subscriber layers run synchronously on the emitting thread and the
//! writer path allocates. RT-side observation uses the wait-free
//! `ruviform-observability` primitives (`Counter`/`Gauge`) instead —
//! those work with logging OFF and are unaffected by subscriber state.
//!
//! # Sensitive data
//!
//! The adapter passes event fields through verbatim. Emitting code must
//! never place secrets, credentials, raw PCM, prompts, or filesystem
//! paths in event fields — Synvelis events carry op names, enum kinds,
//! and coarse state only.

use std::fmt;
use std::num::NonZeroUsize;
use std::path::PathBuf;

use ruviform_observability::logging::{
    LogFormat, LoggingBuildError, LoggingConfig, LoggingHealth, RotationPeriod, SinkConfig,
    SinkDegraded, SinkDestination, SinkFailureMode, WriterGuard, build_logging,
};

/// Default `EnvFilter` directives applied when logging is enabled without
/// an explicit filter (bare `--log`, or only `--log-file`/`--log-format`).
const DEFAULT_FILTER: &str = "info";

/// Default rolling-file retention (generations kept).
const DEFAULT_MAX_FILES: NonZeroUsize = NonZeroUsize::new(14).unwrap();

/// Default bounded queue capacity for the rolling-file sink.
const DEFAULT_QUEUE_CAPACITY: NonZeroUsize = NonZeroUsize::new(8192).unwrap();

/// Default rotation for the rolling-file sink.
const DEFAULT_ROTATION: RotationPeriod = RotationPeriod::Daily;

/// Product logging settings — the resolved Synvelis configuration that
/// [`to_logging_config`] maps onto the Ruviform adapter's
/// [`LoggingConfig`]. All values are already validated; `resolve` is the
/// single place where precedence (CLI > env > defaults) is applied.
#[derive(Debug, Clone)]
pub struct LogSettings {
    /// Whether logging is enabled. Default `false` — a product decision:
    /// end users get no stderr noise, developers opt in explicitly.
    pub enabled: bool,
    /// `EnvFilter` directive string applied to every sink (e.g. `"info"`,
    /// `"synvelis_app=debug,warn"`).
    pub filter: String,
    /// Rendering format for the stderr sink.
    pub stderr_format: LogFormat,
    /// Directory for the rolling-file sink, if one is configured. The
    /// file sink always renders JSON Lines.
    pub file_dir: Option<PathBuf>,
    /// File name prefix for the rolling-file sink.
    pub file_prefix: String,
    /// Rotation period for the rolling-file sink.
    pub rotation: RotationPeriod,
    /// Maximum number of retained log file generations.
    pub max_files: NonZeroUsize,
    /// Bounded in-memory queue for the file sink's writer worker.
    pub queue_capacity: NonZeroUsize,
}

impl Default for LogSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            filter: DEFAULT_FILTER.to_owned(),
            stderr_format: LogFormat::Text,
            file_dir: None,
            file_prefix: "synvelis".to_owned(),
            rotation: DEFAULT_ROTATION,
            max_files: DEFAULT_MAX_FILES,
            queue_capacity: DEFAULT_QUEUE_CAPACITY,
        }
    }
}

impl LogSettings {
    /// Combines CLI and environment overrides into resolved settings.
    /// Precedence is `CLI arg > env var > default`. Any logging flag or
    /// non-empty env value enables logging; none at all leaves it OFF.
    ///
    /// # Errors
    /// [`SettingsError::UnknownFormat`] when a `--log-format` or
    /// `SYNVELIS_LOG_FORMAT` value is neither `text` nor `jsonl`.
    pub fn resolve(cli: &CliOverrides, env: &EnvOverrides) -> Result<Self, SettingsError> {
        let mut settings = Self::default();

        let format = cli.log_format.as_deref().or(env.log_format.as_deref());
        if let Some(value) = format {
            settings.stderr_format = parse_format(value)
                .ok_or_else(|| SettingsError::UnknownFormat(value.to_owned()))?;
        }

        settings.file_dir = cli.log_file.clone().or_else(|| env.log_file.clone());

        // Enablement: an explicit `--log` flag (bare or with a filter)
        // always wins; a non-empty `SYNVELIS_LOG` is the env equivalent;
        // `--log-file`/`--log-format`/`SYNVELIS_LOG_FILE`/
        // `SYNVELIS_LOG_FORMAT` also imply enabled.
        let cli_filter = match &cli.log {
            Some(Some(filter)) => Some(filter.clone()),
            Some(None) => Some(DEFAULT_FILTER.to_owned()),
            None => None,
        };
        let env_filter = env
            .log
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_owned);

        settings.enabled = cli_filter.is_some()
            || env_filter.is_some()
            || settings.file_dir.is_some()
            || format.is_some();
        if let Some(filter) = cli_filter.or(env_filter) {
            settings.filter = if filter.trim().is_empty() {
                DEFAULT_FILTER.to_owned()
            } else {
                filter
            };
        }

        Ok(settings)
    }
}

/// CLI flag values the process entry point collected from `argv` — every
/// field is `None` when the flag is absent.
#[derive(Debug, Default, Clone)]
pub struct CliOverrides {
    /// `--log` presence: `Some(None)` for a bare `--log` (filter falls
    /// back to `"info"`), `Some(Some(directives))` for `--log=…`.
    pub log: Option<Option<String>>,
    /// `--log-file <DIR>` — enables the rolling-file sink.
    pub log_file: Option<PathBuf>,
    /// `--log-format <text|jsonl>` — stderr sink format.
    pub log_format: Option<String>,
}

/// Environment variable values the process entry point collected — the
/// library never reads `std::env` itself; the binary maps
/// `SYNVELIS_LOG`, `SYNVELIS_LOG_FILE`, and `SYNVELIS_LOG_FORMAT` into
/// this struct. `AudioLens` `AL_LOG` vocabulary is not consumed anywhere.
#[derive(Debug, Default, Clone)]
pub struct EnvOverrides {
    /// `SYNVELIS_LOG` — `EnvFilter` directives; empty/unset means OFF
    /// unless other overrides enable logging.
    pub log: Option<String>,
    /// `SYNVELIS_LOG_FILE` — rolling-file directory.
    pub log_file: Option<PathBuf>,
    /// `SYNVELIS_LOG_FORMAT` — `text` or `jsonl`.
    pub log_format: Option<String>,
}

/// Parses `--log`/`--log-file`/`--log-format` flags from an argument
/// list (excluding argv[0]). Unknown arguments and missing values are
/// rejected — a misspelled flag must never be silently ignored.
///
/// # Errors
/// [`SettingsError::UnknownArg`] for unrecognized arguments and
/// [`SettingsError::MissingValue`] when `--log-file`/`--log-format` has
/// no value.
pub fn parse_args(args: &[String]) -> Result<CliOverrides, SettingsError> {
    let mut cli = CliOverrides::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--log" {
            cli.log = Some(None);
        } else if let Some(filter) = arg.strip_prefix("--log=") {
            cli.log = Some(Some(filter.to_owned()));
        } else if arg == "--log-file" {
            let value = iter
                .next()
                .ok_or(SettingsError::MissingValue("--log-file"))?;
            cli.log_file = Some(PathBuf::from(value));
        } else if let Some(dir) = arg.strip_prefix("--log-file=") {
            cli.log_file = Some(PathBuf::from(dir));
        } else if arg == "--log-format" {
            let value = iter
                .next()
                .ok_or(SettingsError::MissingValue("--log-format"))?;
            cli.log_format = Some(value.clone());
        } else if let Some(format) = arg.strip_prefix("--log-format=") {
            cli.log_format = Some(format.to_owned());
        } else {
            return Err(SettingsError::UnknownArg(arg.clone()));
        }
    }
    Ok(cli)
}

/// Parses a `--log-format`/`SYNVELIS_LOG_FORMAT` value. `json` is
/// accepted as an alias of `jsonl`.
fn parse_format(value: &str) -> Option<LogFormat> {
    match value {
        "text" => Some(LogFormat::Text),
        "jsonl" | "json" => Some(LogFormat::JsonLines),
        _ => None,
    }
}

/// Configuration-resolution failures — malformed product settings,
/// reported to the user as a startup error rather than silently
/// degrading.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsError {
    /// An argv entry that is not a recognized logging flag.
    UnknownArg(String),
    /// A flag that requires a value had none.
    MissingValue(&'static str),
    /// A `--log-format`/`SYNVELIS_LOG_FORMAT` value other than
    /// `text`/`jsonl`.
    UnknownFormat(String),
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownArg(arg) => write!(f, "unknown argument: {arg}"),
            Self::MissingValue(flag) => write!(f, "missing value for {flag}"),
            Self::UnknownFormat(value) => {
                write!(f, "unknown log format: {value} (expected text|jsonl)")
            }
        }
    }
}

impl std::error::Error for SettingsError {}

/// Maps resolved settings onto the adapter's [`LoggingConfig`], or
/// `None` when logging is OFF — the OFF path never builds a subscriber.
#[must_use]
pub fn to_logging_config(settings: &LogSettings) -> Option<LoggingConfig> {
    if !settings.enabled {
        return None;
    }
    let mut sinks = vec![SinkConfig {
        destination: SinkDestination::Stderr,
        format: settings.stderr_format,
        filter: settings.filter.clone(),
        failure_mode: SinkFailureMode::Required,
    }];
    if let Some(directory) = &settings.file_dir {
        sinks.push(SinkConfig {
            destination: SinkDestination::RollingFile {
                directory: directory.clone(),
                filename_prefix: settings.file_prefix.clone(),
                rotation: settings.rotation,
                max_files: settings.max_files,
                queue_capacity: settings.queue_capacity,
            },
            format: LogFormat::JsonLines,
            filter: settings.filter.clone(),
            failure_mode: SinkFailureMode::BestEffort,
        });
    }
    Some(LoggingConfig { sinks })
}

/// Process-level install state of the logging pipeline.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallState {
    /// Logging is disabled — no subscriber was built or installed.
    Off,
    /// This call built the sinks and installed the global default.
    Installed,
    /// A global default subscriber already existed — the sinks built by
    /// this call were shut down again and nothing was replaced.
    AlreadyInstalled,
}

impl InstallState {
    /// Stable string form for `syn.app.info` reporting.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Installed => "installed",
            Self::AlreadyInstalled => "already-installed",
        }
    }
}

/// The process-owned logging handle: guards, health, degraded records,
/// and the install outcome. Keep it bound until process shutdown —
/// dropping it drains and joins each sink's writer worker in order.
#[derive(Debug)]
pub struct HostLog {
    state: InstallState,
    _guards: Vec<WriterGuard>,
    health: Option<LoggingHealth>,
    degraded: Vec<SinkDegraded>,
}

impl HostLog {
    /// The install outcome.
    #[must_use]
    pub const fn state(&self) -> InstallState {
        self.state
    }

    /// Health handle for the built sinks — `None` when logging is OFF or
    /// when this call was the loser of an already-installed race (its
    /// sinks were shut down, so their counters carry no meaning).
    #[must_use]
    pub const fn health(&self) -> Option<&LoggingHealth> {
        self.health.as_ref()
    }

    /// Best-effort sinks that failed to initialize at build time.
    #[must_use]
    pub fn degraded(&self) -> &[SinkDegraded] {
        &self.degraded
    }

    /// Hands the degraded records to another owner (the app's
    /// observability owner) — `SinkDegraded` is not `Clone`, so the
    /// records move rather than duplicate. `degraded()` is empty
    /// afterwards.
    pub fn take_degraded(&mut self) -> Vec<SinkDegraded> {
        std::mem::take(&mut self.degraded)
    }
}

/// Builds sinks from `settings` and, when enabled, installs the
/// resulting `Dispatch` as the process-global subscriber.
///
/// OFF settings short-circuit before any sink is constructed — the
/// `tracing` stack stays a no-op and metrics remain independently
/// usable. An existing global default is never replaced: the sinks this
/// call built are shut down and [`InstallState::AlreadyInstalled`] is
/// reported.
///
/// # Errors
/// [`LogInstallError::Build`] when `build_logging` fails — an invalid
/// filter, a failed `Required` sink, or every sink failing.
pub fn install(settings: &LogSettings) -> Result<HostLog, LogInstallError> {
    let Some(config) = to_logging_config(settings) else {
        return Ok(HostLog {
            state: InstallState::Off,
            _guards: Vec::new(),
            health: None,
            degraded: Vec::new(),
        });
    };
    let built = build_logging(&config).map_err(LogInstallError::Build)?;
    if tracing::dispatcher::set_global_default(built.dispatch).is_ok() {
        return Ok(HostLog {
            state: InstallState::Installed,
            _guards: built.guards,
            health: Some(built.health),
            degraded: built.degraded,
        });
    }
    // `set_global_default` consumes the dispatch. Our sinks are shut
    // down by dropping the guards; the pre-existing subscriber is
    // untouched.
    drop(built.guards);
    Ok(HostLog {
        state: InstallState::AlreadyInstalled,
        _guards: Vec::new(),
        health: None,
        degraded: built.degraded,
    })
}

/// install-time failures — sink build errors surface verbatim so the
/// entry point can report them and exit non-zero.
#[non_exhaustive]
#[derive(Debug)]
pub enum LogInstallError {
    /// `build_logging` rejected the configuration or a required sink
    /// failed to initialize.
    Build(LoggingBuildError),
}

impl fmt::Display for LogInstallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Build(error) => write!(f, "logging build failed: {error}"),
        }
    }
}

impl std::error::Error for LogInstallError {}
