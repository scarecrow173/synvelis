---
id: "PLAN-034UgO7fMYPyfOhhpe5zMh"
type: "plan"
status: "in-progress"
title: "SF-03 slice4 実装計画 — NoRebuild Equivalence・世代連動 control 払い出し・SF-ACT-05 headless 固定"
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
  implements:
    - "docs/specs/sf-03-slice4-act-acceptance-norebuild.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-03-slice4-act-acceptance-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-03-slice4-act-acceptance-norebuild.md"
    - "docs/designs/sf-03-slice4-act-acceptance-design.md"
  derived-by:
    - "docs/tasks/sf-03-slice4-norebuild-equivalence.md"
    - "docs/tasks/sf-03-slice4-processor-control-vending.md"
    - "docs/tasks/sf-03-slice4-acceptance-tests.md"
    - "docs/test-specs/sf-03-slice4-act-acceptance.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice4-act-acceptance.md"
  references:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/sf-03-slice2-fault-taxonomy-lastgood-reconcile.md"
    - "docs/specs/sf-03-slice3-plugin-host-connect.md"
  defers: []
  deferred-by: []
---

# SF-03 slice4 実装計画 — NoRebuild Equivalence・世代連動 control 払い出し・SF-ACT-05 headless 固定

## 対象ファイル

| 変更 | ファイル |
| --- | --- |
| 変更 | `crates/synvelis-project/src/routing_build.rs` — `ExecutionEquivalence` 追加 |
| 変更 | `crates/synvelis-project/src/lib.rs` — `ExecutionEquivalence` re-export |
| 変更 | `crates/synvelis-player/src/host.rs` — `ProcessorConnection.control`・`ProcessorControl`・`PluginHostProcessors`/`StaticProcessorHost` 追従 |
| 変更 | `crates/synvelis-player/src/routing_engine.rs` — `RoutingPrepare`・equivalence 判定・reconcile revision-drift 抑制・`RetainedConnection`・`take_processor_control` |
| 変更 | `crates/synvelis-player/src/lib.rs` — `RoutingPrepare`/`ProcessorControl` export |
| 変更 | `crates/synvelis-player/tests/sf03_l1.rs`・`sf03_l2.rs`・`sf03_l3.rs` — `prepare` 戻り値型・`ProcessorConnection` 構築の追従 |
| 新規 | `crates/synvelis-player/tests/sf03_l4.rs` — slice 4 受入テスト |
| 新規 | `docs/test-specs/sf-03-slice4-act-acceptance.md` |
| 新規 | `docs/tasks/sf-03-slice4-norebuild-equivalence.md`・`docs/tasks/sf-03-slice4-processor-control-vending.md`・`docs/tasks/sf-03-slice4-acceptance-tests.md` |

## 順序

1. `routing_build.rs`/`lib.rs`（project）— `ExecutionEquivalence` を追加。
2. `host.rs`/`routing_engine.rs`/`lib.rs`（player）— `ProcessorConnection.control`・
   `ProcessorControl`・`RetainedConnection`・`RoutingPrepare`・equivalence 判定・
   reconcile 抑制・`take_processor_control` を実装。
3. 既存 test（`sf03_l1.rs`/`sf03_l2.rs`/`sf03_l3.rs`）を新署名へ追従。
4. `sf03_l4.rs` で equivalence・control 払い出し・SF-ACT-05 headless 固定を実測。
5. `docs/test-specs`・tasks 文書仕上げ、index 更新。
6. `git fetch origin main && git merge origin/main` 後、
   `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
   `check-headless-deps`。

## リスク

- `prepare` 戻り値型変更で既存 test が広範に壊れる → error 経路は
  `Result<_, RoutingFault>` のまま不変。成功経路のみ
  `expect_built()` 系で追従。件数は限定的で機械的。
- 同一 revision 再 prepare を equivalence にしてしまうと
  「明示的 rebuild」経路が消える → `project_revision` 差を必須
  条件にし、`sf03_l2_09` の連続 publish 契約を維持。
- rename 後の `reconcile` が `StaleProjectSnapshot` を誤報 → live-key
  `semantic_eq` で `ProjectRevision` mismatch を抑制し、semantic
  drift 時は従来どおり報告。
- 払い出した control が世代退役後も残り stale message を受理し得る →
  channel は instance 専用で誤配送なし。node 死後の push は
  `QueueFull` または配送先なし滞留（upstream channel 契約）。
- `ProcessorConnection` field 追加で構築箇所が壊れる → 構築は
  workspace 内の host 実装と test stub のみで追従は機械的。
- `RetainedConnection` の field 順で drop 順序（teardown → control）
  を維持する必要 → field 宣言順 `lease` → `control` で固定し test で
  実測。

## Task Breakdown

- task 1（`sf-03-slice4-norebuild-equivalence`）: `ExecutionEquivalence`・
  `RoutingPrepare`・prepare equivalence 判定・reconcile revision-drift
  抑制。
- task 2（`sf-03-slice4-processor-control-vending`）: `ProcessorConnection.control`・
  `ProcessorControl`・`RetainedConnection`・`take_processor_control`。
- task 3（`sf-03-slice4-acceptance-tests`）: 既存 test 追従・`sf03_l4.rs`・
  test-spec。

## Verification

- [x] `mise run fmt-check` / `mise run check` / `mise run lint` /
      `mise run test` / `mise run test-rt` / `mise run check-headless-deps`
      が全て exit 0。
- [x] `cargo test -p synvelis-player --test sf03_l4` が green。
- [x] equivalence で generation 消費・host connect・compile が
      発生しないことを `sf03_l4` で実測。
- [x] `take_processor_control` の one-shot・unknown/stale `None`・
      wrong-plugin 0 を `sf03_l4` で実測。

## Open Questions / Gaps

- `PluginNodeControl` の exact API（`try_push`/`frame_base` 等）は
  upstream ruviform の現行 rev に依存 — 実装時に確認し、test で
  実測する。
- preview 許容 policy・automation lowering 配線・実機検証は本 slice
  の範囲外（spec 範囲外節・design Deferred Design Concerns 参照）。

## Self-Review

- spec R1–R4 は task 1（equivalence+reconcile）・task 2（control
  払い出し）・task 3（受入 test）に写像済み。
- placeholder なし。ファイルパスは既存 crate 構成に一致。
- `docs/test-specs/sf-03-slice4-act-acceptance.md` は task 3 で作成し
  plan の `verified-by` に登録する。

## Review Handoff

- 前提: slice 1（PR #191）・slice 2（PR #192）・slice 3（PR #195）が
  main に存在。
- upstream API の根拠は ruviform の `ruviform-plugin-host`/
  `ruviform-audio-plugin-bridge`/`ruviform-graph` ソース。
- 未解決の残件（scope 外）: GUI/app/session への equivalence/control
  配線、SF-ACT-05 実機部分、SF-ACT-06 automation lowering・Fast
  Control gate、SF-ACT-07 preview policy 分岐、SF-ACT-08 実装。
