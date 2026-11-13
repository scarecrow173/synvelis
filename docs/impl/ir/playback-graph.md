---
id: "IMPL-034X6XXyiMLYuVtZ3AdA2O"
type: "impl"
status: "completed"
title: "Playback Graph — Issue #18 設計契約文書化 + EX evidence 台帳"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/18"
  changes:
    added:
      - type: "file"
        path: "docs/specs/playback-graph.md"
      - type: "file"
        path: "docs/designs/playback-graph-design.md"
      - type: "file"
        path: "docs/plans/playback-graph.md"
      - type: "file"
        path: "docs/tasks/playback-graph-task.md"
      - type: "file"
        path: "docs/test-specs/playback-graph.md"
      - type: "file"
        path: "docs/impl/ir/playback-graph.md"
      - type: "file"
        path: "docs/impl/exp/playback-graph.md"
      - type: "file"
        path: "evidence/sv18/playback-graph-case-ledger.md"
      - type: "file"
        path: "evidence/sv18/playback-graph-verification.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/tasks/playback-graph-task.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/playback-graph-task.md"
  derived-by: []
  verifies:
    - "docs/test-specs/playback-graph.md"
  verified-by: []
  references:
    - "docs/specs/playback-graph.md"
    - "docs/designs/playback-graph-design.md"
    - "docs/plans/playback-graph.md"
    - "docs/tasks/playback-graph-task.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/playback-graph.md"
    rejected: []
---

# Playback Graph — Issue #18 設計契約文書化 + EX evidence 台帳

## Summary

Issue #18 の Playback / Mixer / Generic Execution Graph 設計契約を
canonical 7 文書チェーン + `evidence/sv18/` 台帳として確立した。
コード・Cargo・CI・Ruviform は変更していない。実装は #104〜109・
#114・ruviform#294 が担う。

## Context

`docs/tasks/playback-graph-task.md`・`docs/plans/playback-graph.md`・
`docs/specs/playback-graph.md`・`docs/designs/playback-graph-design.md`・
`docs/test-specs/playback-graph.md` に従う。上流 baseline
`playback-mixer-generic-execution-graph-architecture.md`（v0.4）と
SF-03 map 設計の headless 実装済み契約を統合し、EX-001〜020 を
既存 test/oracle/状態へ写像した。

## Implementation

### Changes

- `docs/specs/playback-graph.md`: PG-R-01〜15 要件・PG-AC-01〜10
  受入を定義。Owner 分離・typed port・PDC・世代・CAS/speculative・
  last-good・失敗分類・evidence honesty・revision 区別を固定。
- `docs/designs/playback-graph-design.md`: Owner 層表・semantic→
  lowering→publish→active flow・speculative 禁止経路・structural vs
  fast control・PDC 責務・世代状態機械・publish 再検証/reconcile・
  fault 18 分類→Issue #18 13 失敗状況対応・playback/offline/export/
  capture 分離・probe/source map・EX-001〜020 設計写像。
- `docs/plans/playback-graph.md`: 実装 Owner 写像（#104〜109・#114・
  ruviform#294）・File Map・risk・verification plan。
- `docs/tasks/playback-graph-task.md`: タスク分解・done when・
  verification。
- `docs/test-specs/playback-graph.md`: PG-T-R-01〜08 規則・
  EX-001〜020 case map（Req/Result/capability gate）。
- `docs/impl/ir/playback-graph.md`: 本書。
- `docs/impl/exp/playback-graph.md`: 試行・契約照合 log。
- `evidence/sv18/playback-graph-case-ledger.md`: EX→test/oracle/
  status 台帳・PG-AC 写像・NotRun/Deferred/Unsupported・router 状態。
- `evidence/sv18/playback-graph-verification.md`: 検証実行記録。
- 触った dir の生成 index（`docs/*/README.md`）を `index apply` で
  再生成。

### Decisions During Implementation

- 「Pass(部分)」表記を採用: headless で関連契約が確認できるが EX
  全体の独立 oracle を満たすとは限らないことを明示し、実機受入への
  誤読を防ぐ。
- EX-017（非決定 plugin 非保証）は test でなく契約記述のため
  「設計記録」とし Pass にしない。
- fault 18 分類は Issue #106 §5 を正本として採用し、Issue #18 の
  13 失敗状況へ対応表で写像（pending activation は fault でなく
  状態として区別）。
- 実 device/plugin/OS/GUI/consumer lane は全て NotRun/Deferred とし、
  headless 証拠から実機受入を推論しない。

## Related Experiments

### Adopted

- `docs/impl/exp/playback-graph.md` — 上流契約と実装の照合・
  文書鎖生成の試行記録。

### Rejected

- なし。

## Validation

`evidence/sv18/playback-graph-verification.md` を参照。
`mise run fmt-check`・`doc_status lint`・`doc_maintenance index
apply`・`audit_impl_record`・`audit_docs`・`route_graph`・
`review-protocol` を実行し結果を記録。コード test は docs slice の
ため対象外。

## Risks

- headless 証拠を実機受入と誤読するリスク → capability 別
  Pass/Fail/Unsupported/NotRun と「Pass(部分)」で限定。
- 設計契約と実装済み headless 契約の齟齬 → SF-03 map 設計・
  L1/L2/X/TIM-02 spec を depends-on に固定、相違は exp log に記録。
- Ruviform generic typed graph の arbitrary-port DAG 未完 →
  upstream Issue（#184/#308/#309）へ、Synvelis で恒久コピーしない。

## Follow-ups

- 実装は synvelis#104〜109（Player/Render Owner）が本契約を参照。
- GUI 実装は synvelis#114、実 device/plugin 受入は EXT-L3・
  ruviform#294。
- SF-ACT-07 preview/export policy・stem export 個別 job は後続 Issue。
