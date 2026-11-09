---
id: "PLAN-034Uxp78PIJVQOgDqnzNqg"
type: "plan"
status: "approved"
title: "SF-03 semantic→runtime 写像 — 統合設計文書化計画"
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
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/adr/sf-03-authoritative-owner-routingengine-receipt-sourcemap.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sf-03-map-failure-invariant-verification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
  derived-by:
    - "docs/tasks/sf-03-map-task1-contract-design-docs.md"
    - "docs/tasks/sf-03-map-task2-verification-gate-docs.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-map-failure-invariant-verification.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  defers: []
  deferred-by: []
---

# SF-03 semantic→runtime 写像 — 統合設計文書化計画 Implementation Plan

## Goal

Issue #106 の semantic→Ruviform runtime 写像について、全受入行
（SF-ACT-01〜04）の不変条件・失敗写像・世代証拠契約・検証計画を
確定する設計文書チェーンを `docs/` に整備し、後続実装 slice
（SF-ACT-05〜09 残件・実機/GUI gate）への引き継ぎ可能な
権威文書とする。

## Scope

### In Scope

- `docs/` 配下の文書チェーン作成・更新（本書を含む）:
  - `docs/discovery/sf-03-map-current-state-inventory.md`
  - `docs/specs/sf-03-map-semantic-runtime-contract.md`
  - `docs/adr/sf-03-authoritative-owner-routingengine-receipt-sourcemap.md`
  - `docs/designs/sf-03-map-semantic-runtime-mapping-design.md`
  - `docs/test-specs/sf-03-map-failure-invariant-verification.md`
  - `docs/plans/sf-03-map-semantic-runtime-mapping-docs.md`（本書）
  - `docs/tasks/sf-03-map-task{1,2}-*.md`
- managed index 更新（`docs/*/README.md`）。
- review-protocol による文書レビューと blocker 解消。

### Out of Scope

- `crates/` 以下のコード変更、manifest/lockfile/CI 設定。
- Issue #106 の残り実装 slice（実装は設計 merge 後・各 crate
  解放時の後続 dispatch へ委譲）。
- 実 OS/device/VST3・RT 実測・GUI 検証（NotRun/Unsupported）。
- 既存 `sf-03-slice*` 文書の supersede/書き換え。

## Upstream Documents

- `docs/specs/sf-03-map-semantic-runtime-contract.md`（MAP-R1〜R8）
- `docs/adr/sf-03-authoritative-owner-routingengine-receipt-sourcemap.md`
- `docs/designs/sf-03-map-semantic-runtime-mapping-design.md`
- `docs/test-specs/sf-03-map-failure-invariant-verification.md`

## Design Inputs

- `docs/designs/sf-03-map-semantic-runtime-mapping-design.md`
  （責務境界・世代証拠・失敗不変条件・SF-ACT 対応・検証計画）

## File Map

| File | Action | Purpose |
| --- | --- | --- |
| `docs/discovery/sf-03-map-current-state-inventory.md` | Create | SF-03 slice1〜5 + consumer 契約の棚卸しとギャップ特定 |
| `docs/specs/sf-03-map-semantic-runtime-contract.md` | Create | MAP-R1〜R8 要求・受入条件・deferred concerns |
| `docs/adr/sf-03-authoritative-owner-routingengine-receipt-sourcemap.md` | Create | 世代証拠 authoritative owner の決定 |
| `docs/designs/sf-03-map-semantic-runtime-mapping-design.md` | Create | 責務境界・lowering/PDC・SourceMap・失敗不変条件・実装順序 |
| `docs/test-specs/sf-03-map-failure-invariant-verification.md` | Create | Feature/Rule/Example 検証仕様・NotRun 分離 |
| `docs/plans/sf-03-map-semantic-runtime-mapping-docs.md` | Create | 本計画 |
| `docs/tasks/sf-03-map-task1-contract-design-docs.md` | Create | task1: 契約・設計文書チェーン |
| `docs/tasks/sf-03-map-task2-verification-gate-docs.md` | Create | task2: 検証仕様・gate・index 完結 |
| `docs/{discovery,specs,adr,designs,plans,tasks,test-specs}/README.md` | Update | managed index |

## Dependency Graph

```text
discovery ──► spec ──► design ──► test-spec
                │         │
                └──► adr ─┘
                     │
                     ▼
                   plan ──► task1, task2
```

- discovery が spec/adr/design の derives-from。
- spec が design/adr の derived-by、design が spec/adr の
  derives-from/implements。
- design が test-spec の verifies 対象、spec/design が test-spec の
  verified-by。
- plan が spec+design+adr を derives-from/depends-on、test-spec を
  verified-by、task1/task2 を derived-by。
- task が plan を depends-on、test-spec を verified-by。

## Task Breakdown

### task1: 契約・設計文書チェーン

- discovery/spec/adr/design の作成と内容確定。
- 責務境界表・世代名前空間・Activation 状態機械・SourceMap 契約・
  lowering/PDC 意味論・gesture CAS・publish 失敗不変条件表・
  consumer 同一 Revision 契約・SF-ACT-01〜04 対応表・実装順序を
  含む。
- verified-by: test-spec。

### task2: 検証仕様・gate・文書チェーン完結

- test-spec（Feature/Rule/Example・NotRun/Unsupported 分離）の
  作成と内容確定。
- plan/task の関係確定、managed index 再生成、doc_status/
  doc_maintenance lint + `mise run fmt-check` の検証。
- review-protocol の最終判定と status 確定（approved/done）。

## Implementation Sequence

- [x] discovery 現状棚卸し（sf-03-map-current-state-inventory）
- [x] spec 要求確定（sf-03-map-semantic-runtime-contract）
- [x] ADR 決定（authoritative owner）
- [x] design 統合設計（sf-03-map-semantic-runtime-mapping-design）
- [x] test-spec 検証仕様（sf-03-map-failure-invariant-verification）
- [ ] plan/task 作成と関係確定
- [ ] managed index 再生成 + lint/fmt-check
- [ ] review-protocol 最終判定 + PR

## Dependencies and Risks

- **依存**: main 統合済みの SF-03 slice1〜5・TRACE-C3・SF-04・
  SF-05・PC-C6 実装を「確定済み契約」として参照。本チェーンは
  docs のみでコードに依存しない。
- **リスク**: consumer 横断契約が既存 slice 文書と矛盾する場合 —
  slice 文書は approved 実装記録として残し、本チェーンは上位統合
  設計として `related`/`depends-on` で接続（supersede しない）。
- **リスク**: 実機/GUI 検証の欠落 — NotRun/Unsupported として
  test-spec・spec deferred concerns に明示分離済み。
- **リスク**: Ruviform generic 不足 — blocking な不足は未検出。
  判明時は基盤 Issue 起票（Synvelis 側で恒久的重複実装しない）。

## Verification

- [ ] `docs/` の文書チェーンが spec→design→test-spec→plan→task
  の関係で完結し、managed index が最新。
- [ ] doc_status/doc_maintenance lint で新規文書に error がない
  （既存 baseline の warning/error は別件として記録）。
- [ ] `mise run fmt-check` exit 0（docs のみ変更・Rust 非対象）。
- [ ] review-protocol で CONFIRMED P0/P1 が残っていない。
- [ ] 実 OS/device/VST3・RT 実測・GUI は NotRun/Unsupported として
  記録され、成功と誤表示していない。

## Task Docs

- `docs/tasks/sf-03-map-task1-contract-design-docs.md`
- `docs/tasks/sf-03-map-task2-verification-gate-docs.md`

## Implementation Handoff

- Recommended mode: 本 plan は docs のみの文書化計画であり、
  task1/task2 は本 dispatch 内で完結する。後続の**実装** slice
  （SF-ACT-05〜09 残件・実機/GUI）は本チェーン merge 後・各 crate
  解放時の別 dispatch へ委譲する。
- Independent work streams: なし（本 slice は逐次の文書作成）。
- 実装引き継ぎ先: design §8.2 の順序（cross-consumer fixture →
  SF-ACT-06 headless → SF-ACT-05 実機 → SF-ACT-07 policy →
  SF-ACT-08 upstream → SF-ACT-09 性能 → GUI gate）。

## Open Questions / Gaps

- SF-ACT-07 Partial Preview vs Export fail-closed の mode 別
  policy は未規定 — 現行両 mode fail-closed を維持し、policy
  決定は後続 Issue へ defer。
- SF-ACT-09 device config-only change の compile 判定・性能
  baseline は設計変更を伴うため別 slice。
- 実機/GUI/RT 実測は headless では検証不能 — NotRun として記録。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Review Handoff

- 前提: SF-03 slice1〜5・TRACE-C3・SF-04・SF-05・PC-C6 は main
  統合済み。本チェーンはそれらを再実装せず上位統合設計として追加。
- レビュー焦点: 世代証拠の authoritative owner 一意性、publish≠
  active の誤表示防止、失敗不変条件の網羅性、NotRun の正直な記録。
