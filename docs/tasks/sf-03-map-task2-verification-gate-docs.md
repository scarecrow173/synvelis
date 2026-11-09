---
id: "TASK-034Uxr4R9Uw7UUOj88JGfp"
type: "task"
status: "done"
title: "SF-03 map task2 検証仕様・gate・文書チェーン完結"
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
    - "docs/tasks/sf-03-map-task1-contract-design-docs.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sf-03-map-failure-invariant-verification.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
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

# SF-03 map task2 検証仕様・gate・文書チェーン完結

## Work

Issue #106 の検証計画・gate・文書チェーン完結作業を行う
（docs のみ）。

- `docs/test-specs/sf-03-map-failure-invariant-verification.md`:
  Feature/Rule/Example 構造で SF-ACT-01（Golden PCM/SourceMap/
  revision）・SF-ACT-02（順序/tap/sidechain/mute-solo/bus-return/
  processor/PDC/RT-Offline）・SF-ACT-03（CAS stale/capability
  drift/temporary ID/speculative publish 0）・SF-ACT-04
  （世代証拠分離/pending/last-good/stub false positive 無し）・
  Plan publish 失敗不変条件（queue-full/superseded/event-stale/
  epoch/reconcile/missing source-port-plugin）・consumer 同一
  Revision 契約を検証規則化。実 OS/device/VST3・RT 実測・GUI・
  AudioLens は NotRun/Unsupported として分離。
- `docs/plans/sf-03-map-semantic-runtime-mapping-docs.md`:
  文書化計画（file map・dependency graph・task breakdown・
  risk・verification・実装引き継ぎ）。
- 本 task と task1 の関係確定、managed index（`docs/*/README.md`）
  再生成。
- review-protocol 実施と blocker 解消、status 確定
  （spec/adr/design/test-spec/plan=approved、task=done）。
- 最終検証: `doc_status`/`doc_maintenance` lint +
  `mise run fmt-check`。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/sf-03-map-semantic-runtime-mapping-docs.md`
- `docs/tasks/sf-03-map-task1-contract-design-docs.md`
- `docs/test-specs/sf-03-map-failure-invariant-verification.md`

## Dependencies

- task1（契約・設計文書チェーン）の完了が前提。
- 実装検証は headless fixture（`sf03_l*.rs`・`sf04_*`・`sf05`・
  pc-c6 oracle）に委譲。本 slice は文書のみで `mise run test`
  は任意（実施するなら記録）。

## Affected Files

- `docs/test-specs/sf-03-map-failure-invariant-verification.md`
- `docs/plans/sf-03-map-semantic-runtime-mapping-docs.md`
- `docs/tasks/sf-03-map-task{1,2}-*.md`
- `docs/{discovery,specs,adr,designs,plans,tasks,test-specs}/README.md`

## Done When

- [x] test-spec が全受入行の検証規則と NotRun/Unsupported 分離を
  記述する。
- [x] plan/task の front matter relation が相互リンクで完結する。
- [x] managed index が最新で、doc_status/doc_maintenance lint に
  新規文書の error がない。
- [x] `mise run fmt-check` exit 0。
- [x] review-protocol で CONFIRMED P0/P1 なし。

## Verification

- [x] `doc_status`/`doc_maintenance` lint で front matter・relation・
  index 整合を確認。
- [x] `mise run fmt-check`。
- [x] review-protocol で blocker なし。
