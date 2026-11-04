---
id: "TSPEC-034RBFeQnmt3PcI7beSRpy"
type: "test-spec"
status: "draft"
title: "SYN-OBS-01 検証仕様 — Host Logging install・設定解決・Health・Source catalog"
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
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/syn-obs-01-design-host-logging-owner-source-catalog.md"
  derived-by: []
  verifies:
    - "docs/specs/syn-obs-01-ruviform-observability-crate-logging-host.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SYN-OBS-01 検証仕様 — Host Logging install・設定解決・Health・Source catalog

検証本体: `crates/synvelis-app/tests/logging.rs`(新設)・
`crates/synvelis-app/tests/service_ops.rs` 追補・mise lane
(`check`/`lint`/`test`/`test-rt`/`check-headless-deps`)。
`set_global_default` はテストプロセスで1回のみ成功するため、global
install 系の検証は1テストに集約し、それ以外は
`tracing::dispatcher::with_default` の scoped dispatch で検証する。
`webview` feature 実窓起動は実機依存のため headless 証拠と分離する。

## Purpose

Issue #81 の第1段として、Host が所有する Logging 基盤が
「OFF 既定」「設定解決の優先順位」「global install は Process 1 回」
「WriterGuard/health の Host 保持」「Source catalog が OBS-02
canonical 型で記述される」「Logging OFF でも Metrics が動く」の
各不変条件を保証する。Collector(#289)接続時にこの spec が守る
形状のまま registrar へ登録されることを想定する。

## Feature: SYN-OBS-01-A 設定解決(製品語彙→LoggingConfig)

### Rule: 優先順位は `CLI arg > env > 既定(OFF)` で決定的

- `SYNVELIS_LOG`/`SYNVELIS_LOG_FILE`/`SYNVELIS_LOG_FORMAT` の
  読み取りは bin(process 境界)のみ。lib の `resolve` は純粋関数。
- `--log` 裸指定は filter=`info` で有効化。`--log=<directives>` は
  その EnvFilter を使う。`--log-file <DIR>` は有効化を含意し
  RollingFile sink を追加する。
- `--log-format`/`SYNVELIS_LOG_FORMAT` は `text`/`jsonl` のみ
  受理し、未知値は設定エラー(静かな無効化をしない)。
- AudioLens `AL_LOG` 語彙は使わない・読まない。
- OFF 既定: いずれの指定もない場合 `to_logging_config` は `None`。

### Rule: 生成される LoggingConfig は sink 構成を正確に反映する

- stderr sink は `Required`、file sink は `BestEffort`。
- file sink は `RotationPeriod::Daily`・`max_files`・`queue_capacity`
  を設定値から引く(既定 14 / 8192)。

## Feature: SYN-OBS-01-B install 状態遷移と所有

### Rule: install は OFF→何もしない / ON→build→set_global_default

- OFF settings の `install` は `state: Off` で返り、subscriber を
  構築しない(health も `None`)。
- ON settings かつ global default 未設定 → `state: Installed`、
  `HostLog` が guards/health/degraded を保持する。
- global default 設定済みで install → `state: AlreadyInstalled`、
  既存 subscriber は置き換わらず、構築した sink は停止される。
- `build_logging` 失敗(不正 filter 等) → `LogInstallError::Build`。

### Rule: WriterGuard は Host 寿命まで保持され drop で flush される

- `HostLog` drop が guard drop を順序付きで行い、worker の
  drain/flush/bounded join を `WriterGuard` に委譲する。
- Global subscriber は install 後に解除できない(構造制約を明記)。

## Feature: SYN-OBS-01-C 観測参照と Source catalog

### Rule: `syn.app.info` が logging セクションを返す

- OFF/未接続 → `state: "off"`、sinks/degraded は空。
- Installed → `state: "installed"`、sink ごとの
  kind/state/events_written/dropped_queue_full/partial_writes/
  write_failures/flush_failures/channel_disconnected/
  shutdown_abandoned を含む。
- already-installed → `state: "already-installed"`。

### Rule: Synvelis 側カウンタは OBS-02 canonical 型で記述される

- `ObservabilityOwner::sources()` が `MetricSource` を返し、
  descriptor が `MetricKind::Counter`・`key` は
  `synvelis.app.*`・`ObservationUnit::Count`・
  `ObservationScope::Process`。
- catalog の reader は live counter を指し、increment が
  `syn.app.info` の counters に反映される(Logging OFF でも)。
- Synvelis 独自の Counter→Log/Severity/Delta 変換は存在しない
  (依存・コード双方で)。

## Feature: SYN-OBS-01-D 非 RT イベント統合と RT 非計装

### Rule: Synvelis 側非 RT イベントが同一 subscriber に届く

- `with_default(&dispatch)` スコープ内で `tracing::info!` 等を
  発行すると、設定された sink(テストでは rolling file)へ
  event が出力される。
- service/app の op 失敗は `warn!`、成功 op は `debug!`/`info!`。
- event field にパス・秘密情報を含めない(emit 側規約)。

### Rule: RT lane は非計装のまま

- `synvelis-app` 以外の crate に `tracing` 直接依存を追加しない。
- `mise run test-rt` が pass し、`rt_` 契約が維持される。
- `check-headless-deps` が pass する(headless 依存木に
  GUI/GPU なし。tracing 系は許可だが RT crate には入らない)。

## Guarantees

- 正しい実装では、既定起動で subscriber が install されず、
  `--log`/`SYNVELIS_LOG` でのみ install される。
- 二重 install は既存 subscriber を置き換えず `already-installed`
  として可視化される。
- `syn.app.info` は Logging OFF でも counters を返し、ON では
  logging.health を併記する。
- Source catalog は #289 registrar にそのまま渡せる canonical 型。

## Non-goals

- Collector/Registry/Snapshot 履歴(#289)の検証 — upstream 未提供。
- Diagnostics Panel・GUI 購読・window 単位解除 — 別 Issue。
- `tracing_export`(#278)・Host Runner(#293) の停止順序検証。
- webview 実窓での表示確認(実機/OS 依存、別受入)。

## Risk

- OFF 既定・install 冪等・guard 保持を誤ると、Multiwindow 移行時に
  global 二重登録/不明 Guard Drop/停止競合を構造的に抱える。
- Source を独自型で記述すると #289 接続時に全面書き換えになる。
- env を lib 側で読むと「library が全局設定を読む」境界違反を
  将来の別 crate で再演する。
