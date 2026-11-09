---
id: "TASK-034Uxr4gHBdBw4PajV9VU0"
type: "task"
status: "done"
title: "SF-03 map task1 契約・設計文書チェーン（discovery/spec/ADR/design）"
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
    - "docs/plans/sf-03-map-semantic-runtime-mapping-docs.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-03-map-semantic-runtime-mapping-docs.md"
  blocks:
    - "docs/tasks/sf-03-map-task2-verification-gate-docs.md"
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/adr/sf-03-authoritative-owner-routingengine-receipt-sourcemap.md"
    - "docs/discovery/sf-03-map-current-state-inventory.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-map-failure-invariant-verification.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  defers: []
  deferred-by: []
---

# SF-03 map task1 契約・設計文書チェーン（discovery/spec/ADR/design）

## Work

Issue #106 の semantic→Ruviform runtime 写像について、契約・
設計文書チェーンを作成する（docs のみ・実装対象外）。

- `docs/discovery/sf-03-map-current-state-inventory.md`:
  main 統合済み SF-03 slice1〜5（PR #191/#192/#195/#197/#198）
  と TRACE-C3/SF-04/SF-05/PC-C6 の棚卸し、consumer 横断契約の
  ギャップ特定。
- `docs/specs/sf-03-map-semantic-runtime-contract.md`:
  MAP-R1〜R8 要求（世代証拠分離・SourceMap 単一権威・lowering/PDC・
  gesture CAS・publish 失敗不変条件・consumer 同一 Revision・
  SF-ACT-01〜04 確定・実装順序）と受入条件・deferred concerns。
- `docs/adr/sf-03-authoritative-owner-routingengine-receipt-sourcemap.md`:
  世代スコープ証拠の authoritative owner を `RoutingEngine`
  receipt + `RoutingSourceMap` に一元化する決定（MADR）。
- `docs/designs/sf-03-map-semantic-runtime-mapping-design.md`:
  crate/型 owner 境界表・世代名前空間・Activation 状態機械・
  SourceMap 契約・決定的 lowering 順序・PDC/RT-Offline 同一性・
  gesture CAS/speculative publish 0・`RoutingFault` 18 分類の
  失敗写像・reconcile 優先順位・consumer 同一 Revision 契約・
  SF-ACT-01〜04 対応表・実装順序/検証計画/fixture 一覧・
  Ruviform 不足時の扱い。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/sf-03-map-semantic-runtime-mapping-docs.md`
- `docs/specs/sf-03-map-semantic-runtime-contract.md`
- `docs/adr/sf-03-authoritative-owner-routingengine-receipt-sourcemap.md`
- `docs/designs/sf-03-map-semantic-runtime-mapping-design.md`
- `docs/discovery/sf-03-map-current-state-inventory.md`

## Dependencies

- main 統合済み実装（`routing_build.rs`/`routing_lower.rs`/
  `routing_engine.rs`/`probe*.rs`/`trace.rs`/workspace session）
  を調査根拠として参照。コード変更なし。
- `docs/tasks/sf-03-map-task2-verification-gate-docs.md` を blocks。

## Affected Files

- `docs/discovery/sf-03-map-current-state-inventory.md`
- `docs/specs/sf-03-map-semantic-runtime-contract.md`
- `docs/adr/sf-03-authoritative-owner-routingengine-receipt-sourcemap.md`
- `docs/designs/sf-03-map-semantic-runtime-mapping-design.md`

## Done When

- [x] discovery/spec/adr/design の 4 文書が front matter relation
  で相互リンクされ、内容が Issue #106 の要求を網羅する。
- [x] design が責務境界・世代証拠・失敗不変条件・SF-ACT-01〜04
  対応・実装順序・検証計画・fixture 一覧を記述する。
- [x] 実 OS/device/VST3・実機検証は NotRun/Unsupported として
  記録される。

## Verification

- [x] `doc_status`/`doc_maintenance` lint で front matter・relation
  整合を確認（新規文書に error なし）。
- [x] review-protocol で CONFIRMED P0/P1 なし。
