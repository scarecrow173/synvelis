---
id: "IMPL-034RBYkTtBOQ5Akfjc5HEi"
type: "impl"
status: "completed"
title: "SYN-OBS-01 host logging owner + OBS-02 source catalog（synvelis-app 単一所有）"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/src/logging.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/logging.rs"
      - type: "file"
        path: "docs/specs/syn-obs-01-ruviform-observability-crate-logging-host.md"
      - type: "file"
        path: "docs/adr/syn-obs-01-crate-ruviform-observability-logging-feature-host-global-subscriber.md"
      - type: "file"
        path: "docs/designs/syn-obs-01-design-host-logging-owner-source-catalog.md"
      - type: "file"
        path: "docs/plans/syn-obs-01-host-logging-source-catalog.md"
      - type: "file"
        path: "docs/test-specs/syn-obs-01-logging-source-catalog.md"
      - type: "file"
        path: "docs/tasks/syn-obs-01-task1-deps-logging-module.md"
      - type: "file"
        path: "docs/tasks/syn-obs-01-task2-owner-catalog-app-info.md"
      - type: "file"
        path: "docs/tasks/syn-obs-01-task3-tracing-events.md"
      - type: "file"
        path: "docs/tasks/syn-obs-01-task4-bin-wiring.md"
      - type: "file"
        path: "docs/tasks/syn-obs-01-task5-tests-verification.md"
      - type: "file"
        path: "docs/tasks/syn-obs-01-task6-final-verify-impl-record.md"
      - type: "file"
        path: "docs/impl/ir/syn-obs-01-host-logging-owner-obs-02-source-catalog-synvelis-app.md"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-app/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-app/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/bin/synvelis.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/service_ops.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/syn-obs-01-host-logging-source-catalog.md"
    - "docs/designs/syn-obs-01-design-host-logging-owner-source-catalog.md"
    - "docs/specs/syn-obs-01-ruviform-observability-crate-logging-host.md"
    - "docs/adr/syn-obs-01-crate-ruviform-observability-logging-feature-host-global-subscriber.md"
    - "docs/tasks/syn-obs-01-task1-deps-logging-module.md"
    - "docs/tasks/syn-obs-01-task2-owner-catalog-app-info.md"
    - "docs/tasks/syn-obs-01-task3-tracing-events.md"
    - "docs/tasks/syn-obs-01-task4-bin-wiring.md"
    - "docs/tasks/syn-obs-01-task5-tests-verification.md"
    - "docs/tasks/syn-obs-01-task6-final-verify-impl-record.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers:
    - "https://github.com/scarecrow173/ruviform/issues/289"
    - "https://github.com/scarecrow173/ruviform/issues/291"
    - "https://github.com/scarecrow173/ruviform/issues/292"
    - "https://github.com/scarecrow173/ruviform/issues/293"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SYN-OBS-01 host logging owner + OBS-02 source catalog

## Summary

Issue #81 の第 1 段階：Synvelis Rust Host が
`ruviform-observability` 単一 crate の `logging` feature を使い、
global subscriber・`WriterGuard`・`LoggingHealth`・degraded sink
記録をプロセス単位で一元所有する経路を実装した。同時に app intent
二つのカウンタを OBS-02 `MetricSource`（`CounterSource`→`From` 昇格）
の canonical カタログとして `ObservabilityOwner` に保持させ、
将来の upstream collector（ruviform#289）へそのまま登録できる形に
した。`syn.app.info` は `logging` セクションで install 状態・sink
health スナップショット・degraded 一覧を返す。

## Context

- 上流: ruviform#276（OBS-01 generic logging adapter）・#277
  （OBS-02 source registration）は `8fb665f` で解決済み。Collector
  #289・Diagnostics #291・Debug #292・Host Runner #293 は open —
  本実装はローカルに collector/registrar を偽造しない方針を ADR で
  確定させた（カタログはデータとして保持、registrar 所有は upstream）。
- main 側で PR #144（`ruviform-observability` を workspace dep へ
  統一）が先行マージされていたため、merge 時は
  `{ workspace = true, features = ["logging"] }` 形で衝突解消。

## Implementation

### Changes

- `crates/synvelis-app/src/logging.rs`（新規）: `LogSettings`
  （CLI>env>default の優先順で resolve・env 値は bin が収集し lib は
  `std::env` を読まない）・`CliOverrides`/`EnvOverrides`/
  `parse_args`（`--log[=FILTER]`/`--log-file[=]DIR`/`--log-format[=]v`、
  未知引数・欠損値を拒否）・`to_logging_config`（stderr Required +
  任意 rolling-file BestEffort JSONL、file 宛先のみ stdout を汚さない
  構成）・`install`（OFF 短絡 → `build_logging` →
  `set_global_default` を本 crate 唯一の箇所で実行、敗北時は
  `AlreadyInstalled` で自前 sink を畳む）・`HostLog`（guards/health/
  degraded/state の process owner、`take_degraded` で app owner へ
  所有権移譲）・`InstallState`/`LogInstallError`/`SettingsError`。
- `crates/synvelis-app/src/app.rs`: `ObservabilityOwner` に
  `sources: Vec<MetricSource>`（`synvelis.app.intents`/
  `synvelis.app.intent_errors`、Process scope・Count 単位・
  SnapshotOnly）＋ logging state/health/degraded フィールドと
  `attach_logging`/`sources`/`logging_snapshot`/`logging_state`/
  `logging_degraded`。`SynvelisApp::attach_logging` facade、
  `app_info` に `logging` セクション（state/sinks スナップショット/
  degraded、非再帰 health handle 経由）。`project_new`/`project_open`/
  `project_save`/`request_quit` に非 RT `tracing` イベント
  （成否を wrap、パス等はフィールドに入れない）。
- `crates/synvelis-app/src/service.rs`: `call` で op 受信 `debug`、
  失敗時 `warn`（op 名と error.code のみ、params/body を流さない）。
- `crates/synvelis-app/src/bin/synvelis.rs`: argv→`parse_args`、
  `SYNVELIS_LOG`/`SYNVELIS_LOG_FILE`/`SYNVELIS_LOG_FORMAT`→
  `EnvOverrides`、`LogSettings::resolve`→`install` を backend 検出前に
  実行（設定不正・build 失敗は exit 4）、`HostLog` を main 末尾まで
  保持して順序 shutdown、`app.attach_logging` へ結果移譲、
  startup/shutdown/degraded/backend の `tracing` イベント。
- `Cargo.toml`（root）: workspace `tracing = "0.1"`。
- `crates/synvelis-app/Cargo.toml`: `ruviform-observability` に
  `features = ["logging"]`（workspace dep 継続）＋ `tracing.workspace`。
- `crates/synvelis-app/tests/logging.rs`（新規・11 件）: parse/resolve/
  config mapping、OFF 短絡、degraded best-effort sink、scoped
  `with_default` 経由の file sink 実書き込み、global install の
  once-only 契約＋app_info 連携、source カタログ整合。
- `crates/synvelis-app/tests/service_ops.rs`: `syn.app.info` の
  logging セクション（未 install 時 `off`/空配列）1 件追加。

### Decisions During Implementation

- **`SinkDegraded` が `Clone` ではない**ため `HostLog::take_degraded`
  で所有権移譲にした（値複製しない）。upstream へは
  `SinkDegraded: Clone` 要望を改善候補として記録（後述 Follow-ups）。
- 設定解決を lib 内 pure fn（`resolve`）と bin の env 読取
  （`env_overrides`）に分離 — review で指摘された「library 側から
  全局を読まない」方針の徹底。
- `set_global_default` を `match` ではなく `if … is_ok()` +早期 return
  で書いたのは clippy `single_match_else` 準拠のため（挙動は同一）。
- app_info の logging セクションは常時出力（`off` も明示）— GUI が
  state で分岐できるよう「無いキー」にしない。
- テスト側は global install を一度しか成功させられない
  `set_global_default` の制約上、有効 install を叩くテストを
  `global_install_is_once_per_process` 1 件に集約し、他は scoped
  dispatch / OFF 短絡 / `build_logging` 直接でカバー。

## Related Experiments

### Adopted

なし（実験ログは作成していない）。

### Rejected

なし。

## Validation

実測（Windows・rust 1.99.0 pinned、mise run 経由）:

| コマンド | 結果 |
| --- | --- |
| `mise run fmt` | Passed |
| `mise run check`（workspace・all-targets） | Passed |
| `mise run lint`（clippy -D warnings、merge 後再実行） | Passed |
| `mise run test`（workspace --locked、全 suite 0 failed） | Passed |
| `mise run test-rt` | Passed |
| `mise run check-headless-deps` | Passed（8 headless crates に GUI framework dep なし） |
| `cargo test -p synvelis-app`（merge 後: logging 11 + service_ops 13 + asset_check 10） | Passed |
| `cargo tree -p synvelis-app` で tracing stack が logging feature 経由のみ | Passed（player 等 RT 系 crate は tracing 非依存を確認） |

NotRun: 実機 Windows WebView2 ウィンドウでの起動・描画（本環境は
headless 検証のみ。GUI 実機証跡は別途）。MSRV `test-msrv` は本
worker で未実行。

## Risks

- global subscriber は一度 install すると解除不能（tracing 構造
  制約）— 実行時 ON→OFF 切替は再起動が必要。docs に明記済み。
- `AlreadyInstalled` 敗北時に構築した sink は即座に shutdown する
  が、その際の flush 成否は health に載らない（sink が registry に
  残らないため）。実害は「2 回 install した側のログ欠落」ではなく
  単に「その呼出の sink が無効」な点のみ。
- Collector 未着のため `sources` は登録先のないカタログ —
  upstream#289 着工時に `ObservationRegistrar` へ hand-over する
  接着面はコメントで固定済み。

## Follow-ups

- ruviform 側提案候補（本 Issue 範囲外・非 blocker）:
  `SinkDegraded` への `Clone` derive（host が attach 経路で所有権
  移譲せず複製したい場合向け）。issue 起票は coordinator 判断。
- upstream#289 collector 着地後: `ObservabilityOwner::sources()` を
  registrar へ登録する接着コードと、GUI 向け有界 snapshot/query の
  Subscription 層（現状は `syn.app.info` 1 hop snapshot のみ）。
- upstream#293 host runner 着地後: startup/stop 順序
  （Quiescence→最終 snapshot→export drain→guard flush）を bin の
  HostLog ライフサイクルへ正式接続。
