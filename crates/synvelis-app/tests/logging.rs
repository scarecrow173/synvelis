//! Headless tests for the SYN-OBS-01 host logging integration (#81).
//!
//! `tracing::dispatcher::set_global_default` succeeds at most once per
//! process, so exactly one test — [`global_install_is_once_per_process`]
//! — exercises the global install path. Every other test uses the
//! disabled short-circuit or scoped `with_default` dispatches and never
//! touches global state.

use std::fs;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use ruviform_observability::logging::{
    LogFormat, LoggingConfig, RotationPeriod, SinkConfig, SinkDestination, SinkFailureMode,
    SinkKind, build_logging,
};
use ruviform_observability::source::{
    MetricKind, MetricReader, ObservationScope, ObservationUnit, ReportingHint,
};
use ruviform_ui::BackendReport;

use synvelis_app::SynvelisApp;
use synvelis_app::logging::{
    CliOverrides, EnvOverrides, InstallState, LogSettings, SettingsError, install, parse_args,
    to_logging_config,
};

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("synvelis-log-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    dir
}

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| (*s).to_owned()).collect()
}

fn backend() -> BackendReport {
    BackendReport::available("webview2", "test-1.0".to_string())
}

fn enabled_settings() -> LogSettings {
    LogSettings {
        enabled: true,
        ..LogSettings::default()
    }
}

fn file_sink(directory: PathBuf, failure_mode: SinkFailureMode) -> SinkConfig {
    SinkConfig {
        destination: SinkDestination::RollingFile {
            directory,
            filename_prefix: "test".to_owned(),
            rotation: RotationPeriod::Daily,
            max_files: NonZeroUsize::new(2).unwrap(),
            queue_capacity: NonZeroUsize::new(64).unwrap(),
        },
        format: LogFormat::JsonLines,
        filter: "info".to_owned(),
        failure_mode,
    }
}

#[test]
fn parse_args_accepts_all_logging_flag_forms() {
    let cli = parse_args(&args(&[
        "--log",
        "--log-file",
        "logs",
        "--log-format=jsonl",
    ]))
    .expect("parses");
    assert_eq!(cli.log, Some(None));
    assert_eq!(cli.log_file, Some(PathBuf::from("logs")));
    assert_eq!(cli.log_format.as_deref(), Some("jsonl"));

    let cli =
        parse_args(&args(&["--log=synvelis_app=debug", "--log-file=out/logs"])).expect("parses");
    assert_eq!(cli.log, Some(Some("synvelis_app=debug".to_owned())));
    assert_eq!(cli.log_file, Some(PathBuf::from("out/logs")));

    let cli = parse_args(&args(&["--log-format", "text"])).expect("parses");
    assert_eq!(cli.log_format.as_deref(), Some("text"));
}

#[test]
fn parse_args_rejects_unknown_args_and_missing_values() {
    assert_eq!(
        parse_args(&args(&["--verbose"])).unwrap_err(),
        SettingsError::UnknownArg("--verbose".to_owned())
    );
    assert_eq!(
        parse_args(&args(&["--log-file"])).unwrap_err(),
        SettingsError::MissingValue("--log-file")
    );
    assert_eq!(
        parse_args(&args(&["--log-format"])).unwrap_err(),
        SettingsError::MissingValue("--log-format")
    );
}

#[test]
fn resolve_defaults_to_disabled_and_install_is_off() {
    let settings =
        LogSettings::resolve(&CliOverrides::default(), &EnvOverrides::default()).expect("resolve");
    assert!(!settings.enabled);
    assert!(to_logging_config(&settings).is_none());

    // OFF short-circuits before any subscriber is built — safe under
    // parallel tests and never touches the global dispatch.
    let host_log = install(&settings).expect("install off");
    assert_eq!(host_log.state(), InstallState::Off);
    assert!(host_log.health().is_none());
    assert!(host_log.degraded().is_empty());
}

#[test]
fn resolve_applies_cli_over_env_over_defaults() {
    // env alone enables
    let env = EnvOverrides {
        log: Some("synvelis_app=warn".to_owned()),
        log_file: Some(PathBuf::from("env-logs")),
        log_format: Some("jsonl".to_owned()),
    };
    let settings = LogSettings::resolve(&CliOverrides::default(), &env).expect("resolve");
    assert!(settings.enabled);
    assert_eq!(settings.filter, "synvelis_app=warn");
    assert_eq!(settings.file_dir, Some(PathBuf::from("env-logs")));
    assert_eq!(settings.stderr_format, LogFormat::JsonLines);

    // CLI beats env on every field
    let cli = CliOverrides {
        log: Some(Some("debug".to_owned())),
        log_file: Some(PathBuf::from("cli-logs")),
        log_format: Some("text".to_owned()),
    };
    let settings = LogSettings::resolve(&cli, &env).expect("resolve");
    assert_eq!(settings.filter, "debug");
    assert_eq!(settings.file_dir, Some(PathBuf::from("cli-logs")));
    assert_eq!(settings.stderr_format, LogFormat::Text);
}

#[test]
fn resolve_enablement_rules() {
    let no_env = EnvOverrides::default();

    // bare --log → enabled, filter falls back to "info"
    let settings = LogSettings::resolve(
        &CliOverrides {
            log: Some(None),
            ..CliOverrides::default()
        },
        &no_env,
    )
    .expect("resolve");
    assert!(settings.enabled);
    assert_eq!(settings.filter, "info");

    // --log-file alone implies enabled
    let settings = LogSettings::resolve(
        &CliOverrides {
            log_file: Some(PathBuf::from("logs")),
            ..CliOverrides::default()
        },
        &no_env,
    )
    .expect("resolve");
    assert!(settings.enabled);

    // --log-format alone implies enabled
    let settings = LogSettings::resolve(
        &CliOverrides {
            log_format: Some("jsonl".to_owned()),
            ..CliOverrides::default()
        },
        &no_env,
    )
    .expect("resolve");
    assert!(settings.enabled);

    // empty SYNVELIS_LOG does not enable
    let settings = LogSettings::resolve(
        &CliOverrides::default(),
        &EnvOverrides {
            log: Some("   ".to_owned()),
            ..EnvOverrides::default()
        },
    )
    .expect("resolve");
    assert!(!settings.enabled);
}

#[test]
fn resolve_rejects_unknown_format() {
    let result = LogSettings::resolve(
        &CliOverrides {
            log_format: Some("xml".to_owned()),
            ..CliOverrides::default()
        },
        &EnvOverrides::default(),
    );
    assert_eq!(
        result.unwrap_err(),
        SettingsError::UnknownFormat("xml".to_owned())
    );
}

#[test]
fn to_logging_config_maps_sinks() {
    assert!(to_logging_config(&LogSettings::default()).is_none());

    let settings = LogSettings {
        enabled: true,
        filter: "synvelis_app=debug".to_owned(),
        stderr_format: LogFormat::Text,
        file_dir: Some(PathBuf::from("logs")),
        ..LogSettings::default()
    };
    let config = to_logging_config(&settings).expect("config");
    assert_eq!(config.sinks.len(), 2);

    assert!(matches!(
        config.sinks[0].destination,
        SinkDestination::Stderr
    ));
    assert_eq!(config.sinks[0].format, LogFormat::Text);
    assert_eq!(config.sinks[0].filter, "synvelis_app=debug");
    assert_eq!(config.sinks[0].failure_mode, SinkFailureMode::Required);

    match &config.sinks[1].destination {
        SinkDestination::RollingFile { directory, .. } => {
            assert_eq!(directory, &PathBuf::from("logs"));
        }
        other => panic!("expected rolling file sink, got {other:?}"),
    }
    assert_eq!(config.sinks[1].format, LogFormat::JsonLines);
    assert_eq!(config.sinks[1].failure_mode, SinkFailureMode::BestEffort);
}

#[test]
fn build_logging_reports_degraded_best_effort_sink() {
    // A best-effort file sink whose directory cannot be created (a
    // regular file blocks the path) must produce an explicit degraded
    // record while the required stderr sink still builds — never silent.
    let base = temp_dir("degraded");
    fs::create_dir_all(&base).unwrap();
    let blocker = base.join("blocker");
    fs::write(&blocker, "not a directory").unwrap();

    let config = LoggingConfig {
        sinks: vec![
            SinkConfig {
                destination: SinkDestination::Stderr,
                format: LogFormat::Text,
                filter: "off".to_owned(),
                failure_mode: SinkFailureMode::Required,
            },
            file_sink(blocker.join("logs"), SinkFailureMode::BestEffort),
        ],
    };
    let built = build_logging(&config).expect("build");
    assert_eq!(built.degraded.len(), 1);
    assert_eq!(built.degraded[0].sink_index, 1);
    assert_eq!(built.degraded[0].kind, SinkKind::RollingFile);
    assert_ne!(built.degraded[0].reason, "");
    // Only the surviving stderr sink has health counters.
    assert_eq!(built.health.snapshot().sinks.len(), 1);
    drop(built.guards);
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn scoped_dispatch_writes_events_without_global_install() {
    // `with_default` scopes the dispatch to this call — the pattern GUI
    // windows/tests use so multi-window code never re-registers global
    // state.
    let dir = temp_dir("scoped");
    let config = LoggingConfig {
        sinks: vec![file_sink(dir.clone(), SinkFailureMode::Required)],
    };
    let built = build_logging(&config).expect("build");
    tracing::dispatcher::with_default(&built.dispatch, || {
        tracing::info!(marker = "syn-obs-01-scoped", "scoped event");
    });
    drop(built.guards);

    let snapshot = built.health.snapshot();
    assert!(snapshot.sinks[0].events_written >= 1);

    let contents: String = fs::read_dir(&dir)
        .expect("log dir exists")
        .filter_map(std::result::Result::ok)
        .filter_map(|entry| fs::read_to_string(entry.path()).ok())
        .collect();
    assert!(contents.contains("syn-obs-01-scoped"));
    let _ = fs::remove_dir_all(&dir);
}

/// The one test that installs the global subscriber — keep it the only
/// enabled `install` call in this binary (and in the workspace's tests).
#[test]
fn global_install_is_once_per_process() {
    let settings = enabled_settings();
    let mut host_log = install(&settings).expect("install");
    assert_eq!(host_log.state(), InstallState::Installed);
    let health = host_log.health().cloned().expect("health handle");

    // A second install in the same process never replaces the global
    // subscriber — its sinks are shut down and it reports the state.
    let second = install(&settings).expect("second install");
    assert_eq!(second.state(), InstallState::AlreadyInstalled);
    assert!(second.health().is_none());
    drop(second);

    // The app owner reports the attached outcome via syn.app.info.
    let mut app = SynvelisApp::new(backend());
    app.attach_logging(
        host_log.state(),
        host_log.health().cloned(),
        host_log.take_degraded(),
    );
    assert!(host_log.degraded().is_empty());
    let info = app.app_info();
    assert_eq!(info["logging"]["state"], "installed");
    let sinks = info["logging"]["sinks"].as_array().expect("sinks array");
    assert_eq!(sinks.len(), 1);
    assert_eq!(sinks[0]["kind"], "stderr");
    assert_eq!(info["logging"]["degraded"], serde_json::json!([]));

    // An event through the global dispatch reaches the sink — the
    // writer worker is asynchronous, so poll the health counters.
    tracing::info!("syn-obs-01 global event");
    for _ in 0..100 {
        if health.snapshot().sinks[0].events_written > 0 {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(health.snapshot().sinks[0].events_written >= 1);
    drop(host_log);
}

#[test]
fn observability_sources_describe_app_counters() {
    let app = SynvelisApp::new(backend());
    app.observability().record_intent();
    app.observability().record_intent();
    app.observability().record_intent_error();

    let sources = app.observability().sources();
    assert_eq!(sources.len(), 2);

    let mut reads = Vec::new();
    for source in sources {
        assert_eq!(source.descriptor().kind, MetricKind::Counter);
        assert_eq!(source.descriptor().unit, ObservationUnit::Count);
        assert_eq!(source.descriptor().reporting, ReportingHint::SnapshotOnly);
        assert_eq!(source.scope(), ObservationScope::Process);
        match source.reader() {
            MetricReader::Counter(reader) => {
                reads.push((source.descriptor().key, reader.read()));
            }
            other => panic!("expected counter reader, got {other:?}"),
        }
    }
    assert_eq!(reads[0], ("synvelis.app.intents", 2));
    assert_eq!(reads[1], ("synvelis.app.intent_errors", 1));
}
