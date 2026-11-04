---
id: "TASK-034RBFWIyMu9xrEL27zPNX"
type: "task"
status: "done"
title: "SYN-OBS-01 task4 synvelis bin 起動引数/install/attach 組込み"
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
    - "docs/tasks/syn-obs-01-task1-deps-logging-module.md"
    - "docs/tasks/syn-obs-01-task2-owner-catalog-app-info.md"
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

- `src/bin/synvelis.rs`:
  - `EnvOverrides` を `std::env::var` で組み立て、`parse_args(argv)`
    で `CliOverrides` を得て `LogSettings::resolve` →
    `install(&settings)` を `detect_backend` より先に実行(main 先頭)。
  - `ArgsError`/`SettingsError`/`LogInstallError` は stderr に出力し
    exit code 4 で終了(設定不備は静かに無効化しない)。
  - `HostLog` は `main` スコープに束縛し `run()` 戻り後まで保持
    → drop で guards 順 flush/close。
  - `app.attach_logging(host_log.state(), host_log.health().cloned())`
    で ObservabilityOwner へ接続。
  - install 結果を `info!`(state・sink 数・degraded 数)で emit、
    起動・backend probe・asset bundle 結果・終了にも `info!`/`warn!`。
  - `eprintln!` のユーザ向け出力は維持(tracing と並行)。
- 新規 exit code 4 を bin doc-comment に追記。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/syn-obs-01-host-logging-source-catalog.md`

## Dependencies

- `docs/tasks/syn-obs-01-task1-deps-logging-module.md`
- `docs/tasks/syn-obs-01-task2-owner-catalog-app-info.md`

## Affected Files

- `crates/synvelis-app/src/bin/synvelis.rs`

## Done When

- [ ] `--log`/`--log-file`/`--log-format` および `SYNVELIS_LOG*` で
  Logging が制御され、未指定時は install されない。
- [ ] `syn.app.info` が接続済みの logging state/health を返す。
- [ ] `mise run check-app-webview`/`lint-app-webview` が pass。

## Verification

- [ ] `cargo check -p synvelis-app --features webview --all-targets` pass
