---
id: "TASK-034RBFUDGVewxs72FGa8RM"
type: "task"
status: "done"
title: "SYN-OBS-01 task1 依存更新と logging モジュール(設定解決/install/owner)"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/syn-obs-01-host-logging-source-catalog.md"
  implemented-by: []
  depends-on:
    - "docs/plans/syn-obs-01-host-logging-source-catalog.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/syn-obs-01-logging-source-catalog.md"
  references: []
  defers: []
  deferred-by: []
---

## Work

- `Cargo.toml`: `[workspace.dependencies]` に `tracing = "0.1"` を追加。
- `crates/synvelis-app/Cargo.toml`: `ruviform-observability` を
  `features = ["logging"]` 付きにし、`tracing.workspace = true` を追加。
- `crates/synvelis-app/src/logging.rs` を新設:
  - `LogSettings`{enabled, filter, stderr_format, file_dir, rotation,
    max_files, queue_capacity} — `Default` は OFF(filter="info"、
    stderr Text、file なし、rotation Daily、max_files 14、
    queue_capacity 8192)。
  - `CliOverrides`{log: Option<Option<String>>, log_file: Option<PathBuf>,
    log_format: Option<String>}、`parse_args(&[String]) -> Result<
    CliOverrides, ArgsError>` — `--log[=FILTER]`/`--log-file DIR`/
    `--log-format text|jsonl` を受理、未知値/未知フラグはエラー。
  - `EnvOverrides` — bin が `std::env` から組み立てる `Option` 群
    (`SYNVELIS_LOG`/`SYNVELIS_LOG_FILE`/`SYNVELIS_LOG_FORMAT`)。
    lib 側は env を読まない。
  - `LogSettings::resolve(cli, env) -> Result<LogSettings, SettingsError>`
    — CLI>env>既定の合成、format 未知値・filter 空文字の正規化。
  - `to_logging_config(&LogSettings) -> Option<LoggingConfig>` —
    OFF で `None`、ON で stderr(Required)+file(BestEffort、あれば)。
  - `InstallState`{Off, Installed, AlreadyInstalled} + `as_str()`。
  - `HostLog`{state, guards, health, degraded} + `status()`/
    `health()`/`state()`。
  - `install(&LogSettings) -> Result<HostLog, LogInstallError>` —
    OFF→`Off`、ON→`build_logging`→`set_global_default`(失敗は
    `AlreadyInstalled` で guard を drop)。
  - `LogInstallError::Build(LoggingBuildError)`。
- `src/lib.rs`: `pub mod logging;` + 主要型の re-export。
- doc-comment に「Global subscriber は install 後に解除できない」
  「RT callback では tracing/alloc/lock 禁止」「emit 側で秘密情報・
  パスを書かない」を明記。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/syn-obs-01-host-logging-source-catalog.md`

## Dependencies

- なし(先行 task なし)

## Affected Files

- `Cargo.toml`、`crates/synvelis-app/Cargo.toml`、
  `crates/synvelis-app/src/logging.rs`(新規)、
  `crates/synvelis-app/src/lib.rs`、`Cargo.lock`(再生成)

## Done When

- [ ] `mise run check` が pass し、`cargo check -p synvelis-app` が
  `logging` module をコンパイルする。
- [ ] `cargo tree -p synvelis-app` で `tracing`/`tracing-subscriber`/
  `tracing-appender` が `ruviform-observability` logging feature 由来+
  直接依存として解決される。

## Verification

- [ ] `mise run check` pass
