---
id: "PLAN-034AUT04psf1yTvW6ZK4c3"
type: "plan"
status: "approved"
title: "AUT-04 Automation Curve 非破壊保持・Viewport LOD・有界記録 Event 予算 実装計画"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/134"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/tests/aut_04.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/aut_04_rt.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/control.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/control.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/aut-04-automation-curve-lod-record-budget.md"
    - "docs/adr/aut-04-viewport-lod-bounded-record-queue.md"
    - "docs/designs/aut-04-automation-curve-lod-record-budget-design.md"
  implemented-by:
    - "docs/tasks/aut-04-task1-automation-curve-lod-record-budget.md"
  depends-on:
    - "docs/plans/aut-03-loop-write-one-pass.md"
    - "docs/plans/pc-c1-recording-transaction.md"
    - "docs/plans/aut-02-region-replace-boundary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/aut-04-automation-curve-lod-record-budget.md"
    - "docs/designs/aut-04-automation-curve-lod-record-budget-design.md"
    - "docs/adr/aut-04-viewport-lod-bounded-record-queue.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/aut-04-automation-curve-lod-record-budget-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/aut-04-e2e-automation-curve-lod-record-budget.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/98"
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/103"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/131"
    - "https://github.com/scarecrow173/synvelis/issues/132"
    - "https://github.com/scarecrow173/synvelis/issues/133"
  defers: []
  deferred-by: []
---

# AUT-04 Automation Curve 非破壊保持・Viewport LOD 実装計画

## スコープ分割

単一 task で実装する。変更面は model 層の read-only LOD 投影
(`synvelis-project/src/control.rs`)と session 層の RT ingress・
drain・view 組立(`synvelis-workspace/src/control.rs`)中心で、
テスト 2 ファイルと依存宣言に閉じる。

| 区分 | 内容 |
| --- | --- |
| In | `CurveQuery`/`ViewBin`/`CurveFeature`/`CurveQuality`/`AutomationCurveView`/`automation_view`(project)、`AutomationRecordEvent`/`RecordPushError`/`RecordBudgets`/`RecordGap`/`RecordTargetHealth`/`RecordDrainReport`/`RecordNotification`/`record_event_rt`/`drain_record_events`/`record_health`/`record_notifications`/`AutomationLaneView`/`ControlSession::automation_view`、`RecordingTransaction.{record_gaps,deliveries,record_dropped}`、`ruviform-rt`+`stats_alloc` 依存、`tests/aut_04.rs`/`aut_04_rt.rs` |
| Out | Multi-pass Take(AUT-F01 #6)、実 Plugin binding、GUI 配線、PC-C6 oracle、永続 schema 変更、永続曲線 Simplification、実 engine producer 分割、Browser/OS/負荷実測 |

## ファイルマップ

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-project/src/control.rs` | LOD 型群 + `automation_view`(merged_series 共有、bin envelope/feature 分類/quality) |
| `crates/synvelis-project/src/lib.rs` | 新公開型・fn の export 整理 |
| `crates/synvelis-workspace/src/control.rs` | queue/health/notification/型群、`record_event_rt`、`drain_record_events`、`record_gaps`/`deliveries` tx 追加、session `automation_view` |
| `crates/synvelis-workspace/src/lib.rs` | 新公開型・定数の export 整理 |
| `crates/synvelis-workspace/Cargo.toml` | `ruviform-rt.workspace` 追加、`stats_alloc` dev-dep 追加 |
| `Cargo.lock` | 依存再解決で再生成(手編集しない) |
| `crates/synvelis-workspace/tests/aut_04.rs` | test-spec F1–F8 の headless pin |
| `crates/synvelis-workspace/tests/aut_04_rt.rs` | test-spec F9 の RT alloc 0 pin |
| `docs/impl/ir/` | 実装記録(実装後に追加) |

## 依存グラフ

- `synvelis-workspace` → `ruviform-rt`(workspace git 依存、SPSC
  `Producer`/`Consumer` のみ利用 — 型・契約変更なし)。
- `synvelis-project` の新 API は `merged_series`/`BeatPoint`/
  `resolve_scope_pos` の既存契約のみ消費。公開型を増やすのみで
  既存 signature は変えない。
- `record_points` の budget 参照を `RECORD_POINT_BUDGET` 定数から
  `self.record_budgets.max_points` へ切替(既定値同一・既存挙動
  不変)。drain の drop 計上は `record_points` の budget 拒否を
  検出して行う。

## リスク登録

| リスク | 対応 |
| --- | --- |
| LOD envelope の厳密性を近似に劣化させる実装ミス | Linear 区間は affine のため端点+内部点の評価で厳密 — テストで 10^4 点・uniform eval との一致を pin |
| push 側 gate 判定で alloc 経路(BTreeMap insert 等)を踏む | push 側は read-only 参照+pre-reserved Vec のみ。`record_event_rt` 内に String/collection 生成を置かない。RT テストで計測 |
| drain が txn 跨ぎ event を現 txn に誤帰属 | event に `txn` を刻印し drain で照合 → `WrongTxn` stale |
| health/notification/gap の cap 超過で喪失を隠す | cap は「上限を超えた分は最後の集約/カウンタへ」設計 — 総量は `dropped`/`record_health_overflows`/`truncated` 系で必ず観測可能 |
| deliveries 肥大(held write が control-rate で大量追加) | `DELIVERY_CAP` 4096 + `deliveries_dropped` カウンタ。view では span としてのみ使用 |
| session `automation_view` が project 変更を起こす | `&self` + `&Project` の read-only のみ — revision/undo/build 不変を F4 で pin |
| `f64` bin と `BeatPoint` feature の座標混同 | bin/envelope は lossy f64 表示座標、feature/point は `BeatPoint` 正確 — 型で分離・テストで両方 pin |

## ロールバック戦略

session/model 層の追加のみで永続化 schema・既存評価・commit
経路に触れない — revert で完全に旧動作へ戻る。新フィールドは
session 内型で永続データに出ない。依存追加(`ruviform-rt`)は
workspace git 依存の既存行を crate へ配線するのみで、`Cargo.lock`
は再解決で再生成できる。

## 検証マトリクス

| 検証 | 入口 |
| --- | --- |
| fmt | `mise run fmt` → `fmt-check` |
| 型・lint | `mise run check` / `mise run lint` |
| 単体・統合 | `mise run test`(`tests/aut_04.rs` + aut_01/02/03・pc_c1 等無退行) |
| RT | `mise run test` 内 `aut_04_rt.rs`(stats_alloc 計測)+ `mise run test-rt`(既存 RT lane 無退行) |
| MSRV | `mise run test-msrv` |
| docs | `node .agents/skills/doc-maintenance/scripts/doc_maintenance.js apply` で index 再生成 |
| 最終前 | `git fetch origin` → `git merge origin/main` → lockfile 再生成 → 上記再実行 |

## タスク分解

- `TASK-034AUT04`: 本計画の実装 + `tests/aut_04.rs`/
  `aut_04_rt.rs`(test-spec F1–F9 に対応)。
