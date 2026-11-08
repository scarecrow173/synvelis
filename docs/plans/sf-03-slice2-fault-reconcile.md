---
id: "PLAN-106SF03Pz2aBc3dEf4gHi5jKl6"
type: "plan"
status: "approved"
title: "SF-03 slice2 実装計画 — Fault 分類・Last-good/Retry・reconciliation"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/designs/sf-03-slice2-fault-reconcile-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-03-slice2-fault-reconcile-design.md"
  derived-by:
    - "docs/tasks/sf-03-slice2-fault-contract.md"
    - "docs/tasks/sf-03-slice2-reconcile-tests.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice2-fault-reconcile.md"
  references: []
---

# SF-03 slice2 実装計画

## 対象ファイル

| 変更 | ファイル |
| --- | --- |
| 変更 | `crates/synvelis-project/src/routing_build.rs` — `RoutingFault` 18 variant 再構成、`RoutingReconcileMismatch`/`RoutingReconcileReport` 追加 |
| 変更 | `crates/synvelis-player/src/routing_lower.rs` — `SemanticLowerError::PortUnresolved` 追加・`fault()` 写像更新 |
| 変更 | `crates/synvelis-player/src/routing_engine.rs` — prepare/publish 改修・`reconcile` 追加・Ledger 拡張・`frames_observed` 修正 |
| 変更 | `crates/synvelis-player/tests/sf03_l1.rs` — variant 改名に伴う assert 更新のみ |
| 新規 | `crates/synvelis-player/tests/sf03_l2.rs` — slice 2 受入テスト |
| 新規 | `docs/test-specs/sf-03-slice2-fault-reconcile.md` |

## 順序

1. `routing_build.rs` — `RoutingFault` 再構成 + reconcile report 型。
2. `routing_lower.rs` — `PortUnresolved` と fault 写像。
3. `routing_engine.rs` — `checked_key`/prepare/publish/`observe_activation`/
   `record_state` 改修、`reconcile`・Ledger baseline 追加。
4. `sf03_l1.rs` の assert 更新（改名追従）+ `sf03_l2.rs` 新規。
5. `docs/test-specs` 仕上げ。
6. `git fetch origin main && git merge origin/main` 後、
   `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
   `check-headless-deps`。

## リスク

- `RoutingFault` 改名で既存 assert が壊れる → `cargo check`/test で
  網羅的に検出し機械的に追従。
- skipped 昇格で realtime の既存 test が新たに失敗 → 失敗したら
  skipped 内容を確認し、意図的無音か fault 昇格対象かを test-spec
  に照合。fault 対象なら test 側の project 構成を修正する。
- `reconcile` の counter baseline が初回呼出で過去分を誤検出 →
  baseline は engine 構築時 0 初期化、初回 reconcile で既存 counter
  を baseline に取り込む（差分のみ fault）。テストで順序を固定。
- `PublishQueueFull` の決定的再現 → command channel 容量 64 を
  超える install/remove delta を持つ大規模 project、または
  pending command 滞留下での連続 publish で誘発。困難なら
  `ActivationError` → `PublishQueueFull` の写像は code review +
  他 fault の網羅で担保し、test は retry/last-good 側を優先。
