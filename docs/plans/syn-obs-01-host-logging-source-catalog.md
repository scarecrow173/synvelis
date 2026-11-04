---
id: "PLAN-034RBEtRnww0T7opa8Akmw"
type: "plan"
status: "approved"
title: "SYN-OBS-01 実装計画 — Host所有 Logging 導入・設定解決・Source Catalog"
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
    - "docs/specs/syn-obs-01-ruviform-observability-crate-logging-host.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/syn-obs-01-crate-ruviform-observability-logging-feature-host-global-subscriber.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-obs-01-ruviform-observability-crate-logging-host.md"
    - "docs/designs/syn-obs-01-design-host-logging-owner-source-catalog.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/syn-obs-01-logging-source-catalog.md"
  references: []
  defers: []
  deferred-by: []
---

# SYN-OBS-01 実装計画 — Host所有 Logging 導入・設定解決・Source Catalog

## Goal

`synvelis-app` に `ruviform-observability` `logging` feature を導入し、
Host 所有の LogSettings→install→HostLog(guard/health 保持)経路、
`ObservabilityOwner` の Source catalog と `syn.app.info` の logging
セクション、`synvelis` bin の起動引数 install を実装する。

## Scope

単一実装ストリーム。`synvelis-app` のみを変更対象とし、他 crate・
ui/・Ruviform 側には触れない。Collector 系は upstream #289 待ちで
本計画に含めない。

## Upstream Documents

- `docs/specs/syn-obs-01-ruviform-observability-crate-logging-host.md`
- `docs/adr/syn-obs-01-crate-ruviform-observability-logging-feature-host-global-subscriber.md`

## Design Inputs

- `docs/designs/syn-obs-01-design-host-logging-owner-source-catalog.md`(approved)

## File Map

- `Cargo.toml` — workspace deps に `tracing = "0.1"` 追加
- `crates/synvelis-app/Cargo.toml` — `ruviform-observability` に
  `features = ["logging"]`、`tracing.workspace = true` 追加
- `crates/synvelis-app/src/logging.rs` — **新設**: `LogSettings`/
  `CliOverrides`/`EnvOverrides`/`parse_args`/`LogSettings::resolve`/
  `to_logging_config`/`InstallState`/`HostLog`/`install`/
  `LogInstallError`
- `crates/synvelis-app/src/lib.rs` — `pub mod logging` + re-export
- `crates/synvelis-app/src/app.rs` — `ObservabilityOwner` に
  `sources: Vec<MetricSource>`・`logging_health`・`logging_state` 追加、
  `attach_logging`/`sources`/`logging_snapshot`/`app_info` 拡張、
  `SynvelisApp::attach_logging`
- `crates/synvelis-app/src/service.rs` — op 失敗 `warn!`、op 受領
  `debug!` の非 RT tracing 発行
- `crates/synvelis-app/src/bin/synvelis.rs` — argv/env parse →
  install → attach → 起動/終了 `info!`
- `crates/synvelis-app/tests/logging.rs` — **新設**: 設定解決・
  install 状態遷移・health 参照・catalog 整合
- `crates/synvelis-app/tests/service_ops.rs` — `syn.app.info` の
  logging セクション検証を追加
- `Cargo.lock` — `mise run check` 等で再生成(手編集しない)

## Implementation Sequence

- [ ] task1: workspace/crate 依存更新 + `logging.rs` 骨組み
  (型と純粋関数、install 本体)。`cargo check -p synvelis-app` が通る。
- [ ] task2: `ObservabilityOwner` の Source catalog 化と
  `attach_logging`/`logging` セクションの `app_info` 反映。
- [ ] task3: service/app の非 RT tracing イベント発行。
- [ ] task4: `bin/synvelis.rs` 起動引数/env/install 組込み。
- [ ] task5: `tests/logging.rs` + `service_ops.rs` 追補、mise
  `check`/`lint`/`test`/`test-rt`/`check-headless-deps` 実行。
- [ ] task6: 最終検証(main merge→lockfile 再生成→全 lane)+impl
  record + task 状態更新。

## Dependencies and Risks

- `set_global_default` はテストプロセスで1回しか成功しない:
  global install 検証は1テストに集約し、他テストは
  `with_default` scoped dispatch を使う。
- `clippy::module_name_repetitions`/`pedantic` が `-D warnings`:
  公開型名は設計書の命名(`LogSettings`/`HostLog`/`LogInstallError`)
  を厳守する。
- `LogFormat`/`RotationPeriod`/`SinkDestination`/`SinkKind`/
  `SinkState` は `#[non_exhaustive]` — match には wildcard arm
  が必要。
- `tracing` 直接依存は `synvelis-app` のみ。他 crate の依存木に
  入らないことを `cargo tree`/`cargo metadata` で確認する。
- `ruviform-observability` の `logging` feature は
  `tracing-appender`/`tracing-subscriber` を引き込む —
  synvelis-app の headless lane に GUI/GPU 依存は入らない
  (check-headless-deps が守る)。

## Task Breakdown

| task | 内容 | 依存 |
| --- | --- | --- |
| task1 | 依存更新 + logging.rs 型/純粋関数/install | — |
| task2 | ObservabilityOwner catalog + attach + app_info | task1 |
| task3 | service/app tracing イベント | task1 |
| task4 | bin 起動引数/install/attach | task1, task2 |
| task5 | tests/logging.rs + service_ops 追補 + mise 検証 | task2, task3, task4 |
| task6 | 最終検証・impl record・task 更新 | task5 |

## Verification

- [ ] `mise run check` / `mise run lint` / `mise run test` /
  `mise run test-rt` / `mise run check-headless-deps` 全 pass
- [ ] `cargo test -p synvelis-app` で設定解決・install 状態・
  health 参照・catalog 整合の検証が緑
- [ ] `cargo tree -p synvelis-recording -e normal | grep -c tracing`
  が 0(RT lane 非計装)
- [ ] `synvelis --log` 起動時 stderr に synvelis 側 event が出る
  (webview lane は実機依存のため headless 証拠と分離して記録)

## Open Questions / Gaps

- `--log-format`/`SYNVELIS_LOG_FORMAT` が `text`/`jsonl` 以外の値の
  扱い: parse_args でエラーにする(起動時設定不備を静かに無効化
  しない方針)。
- RollingFile の `max_files`=14・`queue_capacity`=8192 は暫定既定値。
  運用で調整可能な設定値であり契約ではない。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution(subagent 不使用の worker 実行)。
- Independent work streams: task3 は task2 と独立(task1 のみ依存)。
- User approval required before dispatch: yes(worker 規約上
  review-protocol で代替)。
- If approved: task1→(task2∥task3)→task4→task5→task6 の依存順で実行。
- If declined or unavailable: 同一依存グラフと検証行列で inline 実行。

## Review Handoff

- 重点: `AlreadyInstalled` 非エラー化の妥当性、`syn.app.info`
  の logging セクション形状、OFF 既定の製品判断、env 読み取りを
  bin 境界に置いた責務分割。
