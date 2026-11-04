---
id: "PLAN-034SF02Wz0rOp7qT0vB5cDfGh"
type: "plan"
status: "approved"
title: "SF-02 RoutingTransaction 実装計画 — staging/検証/mute-solo/テスト"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/105"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/designs/sf-02-routing-transaction-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-02-routing-transaction-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
---

# SF-02 実装計画 — DAW Routing Commands・Semantic Validation

## 対象ファイル

| 変更 | ファイル |
| --- | --- |
| 新規 | `crates/synvelis-project/src/routing_tx.rs` — TempRef/intent/edit/staging/preflight/error/receipt |
| 変更 | `crates/synvelis-project/src/routing.rs` — `SemanticPort.channels`・`SendConfig`・audibility・GraphSpec 投影 |
| 変更 | `crates/synvelis-project/src/ops.rs` — `ProjectOp::Routing` variant・staging 呼出・`SetSendGainTarget`・`ChangeBusConfiguration` |
| 変更 | `crates/synvelis-project/src/control.rs` — `ParameterRole::Mute/Solo`・mute/solo 解決 helper |
| 変更 | `crates/synvelis-project/src/id.rs` — TempRef 型(または routing_tx 内) |
| 変更 | `crates/synvelis-project/src/lib.rs` — module export |
| 変更 | `crates/synvelis-project/src/error.rs` — 必要な新規 variant 追記(既存で賄えるなら不変) |
| 変更 | `crates/synvelis-project/Cargo.toml` — `ruviform-graph` 依存追加 |
| 変更 | `crates/synvelis-gateway/src/ops.rs` — op_name/op_facts 対応 |
| 新規 | `crates/synvelis-project/tests/routing_sf02.rs` — 受入契約テスト |

## タスク分解

1. **T1 型+staging**: TempRef/ref 型・`RoutingEditIntent`/`RoutingEdit`/
   `RoutingEditError`/`RoutingReceipt`・`preflight`・`ProjectOp::Routing`。
   channels 追加・汎用 Validator 投影・意味検証(ownership/domain/direction/
   role/map/consent/unresolved/cycle)。Mute/Solo role + audibility。
2. **T2 受入テスト**: routing_sf02.rs で SF-VALID-01..08 を pin。
   preview 不変・commit 1 undo・stale revision/generation・dangling/
   invalid role/map/cycle/unresolved・Bass→Bus→Master・Pre/Post send→
   return・sidechain・mute/solo audibility・undo 復元。
3. **T3 検証+記録**: mise 全緑・impl record・doc status 整合。

## 実装順序

1. `SemanticPort.channels` + `ParameterRole::Mute/Solo` + `SendConfig`
   拡張の additive 変更 → compile。
2. `routing_tx.rs` 型 + `preflight` + `ProjectOp::Routing`。
3. 汎用 Validator 投影 + 意味検証関数。
4. mute/solo target 自動生成(SetMute/SetSolo 時に未存在なら作成) +
   `audible_strips`。
5. Gateway op_name。既存 ops(SetSendGainTarget 等の独立 variant)追加。
6. テスト → fmt/lint/test。

## リスク

- `ruviform-graph` 依存は git 依存(workspace 既存と同一 source)。feature
  最小化。`GraphSpec` API が不足する場合は投影を最小構成(ポート能力/
  multiplicity を Synvelis 側補助検査)に留め、Issue 起票で upstream 要望
  として記録する(回避策は恒久化しない)。
- Mute/Solo target の「自動生成」は ProjectOp::Routing 適用時にのみ行い、
  persisted model には差分を残さない(通常の control target として存在)。
- 並行 worker(syn-132)との `Cargo.toml`/lock 競合は merge origin/main で
  解決する。

## 検証

- `mise run fmt-check` / `check` / `lint` / `test` / `build` / `test-msrv`。
- Reject/不変条件/Undo/Revision を tests で pin。
- 実機/実 Plugin/GUI は NotRun/Unsupported。
