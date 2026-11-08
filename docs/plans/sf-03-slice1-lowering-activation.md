---
id: "PLAN-106SF03Fg4gHi5jKl6mNo7pQr"
type: "plan"
status: "approved"
title: "SF-03 slice1 実装計画 — 契約型・lowering・activation・テスト"
created: "2026-11-08"
updated: "2026-11-08"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/106"
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
    - "docs/designs/sf-03-slice1-lowering-sourcemap-activation-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-03-slice1-lowering-sourcemap-activation-design.md"
  derived-by:
    - "docs/tasks/sf-03-slice1-contract-lowering.md"
    - "docs/tasks/sf-03-slice1-activation-tests.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice1-lowering-activation.md"
  references: []
---

# SF-03 slice1 実装計画

## 対象ファイル

| 変更 | ファイル |
| --- | --- |
| 新規 | `crates/synvelis-project/src/routing_build.rs` — 契約型 + fingerprint |
| 変更 | `crates/synvelis-project/src/lib.rs` — module/export |
| 変更 | `crates/synvelis-player/src/lowering.rs` — `plan_playbacks`/`PlaybackPlan` 抽出(legacy path は同一解決を共有) |
| 新規 | `crates/synvelis-player/src/routing_lower.rs` — `lower_semantic`・`SemanticLowered`・`StripProcessor` |
| 新規 | `crates/synvelis-player/src/routing_engine.rs` — `RoutingEngine`・activation ledger・receipt |
| 変更 | `crates/synvelis-player/src/lib.rs` — module/export |
| 新規 | `crates/synvelis-player/tests/sf03_l1.rs` — SF-ACT-01〜04 slice 受入 |
| 新規 | `docs/test-specs/sf-03-slice1-lowering-activation.md` |

## 順序

1. `routing_build.rs` 契約型 + `routing_fingerprint`(project 側で閉じる)。
2. `lowering.rs` の `plan_playbacks` 抽出 — 既存 test が green のまま。
3. `routing_lower.rs` — node 構成・edge・SourceMap・`StripProcessor`。
4. `routing_engine.rs` — prepare/publish/process/observe + ledger。
5. player/project tests、`docs/test-specs`。
6. `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/`check-headless-deps`。

## リスク

- `lower_with_inserts` リファクタで legacy PCM が変わる → 既存
  `l1_e2e`/`md_b`/`pc_b` test で回帰検出。emit 順・接続順は不変に保つ。
- fan-in 加算順序: semantic path は strip 単位 sum → master sum。
  golden は単一 track で厳密一致、複数 track は semantic 順で期待値を
  構成して比較(設計 §3.1)。
- `ProcessorBinding::Unresolved` を含む project は compile 失敗 →
  `PluginUnavailable`。test は注入 `GraphProcessor` で processor 順序を
  検証する。
