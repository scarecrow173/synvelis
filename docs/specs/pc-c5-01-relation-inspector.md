---
id: "SPEC-cBV4t2wr7OJdUVPBI13T4q"
type: "spec"
status: "approved"
title: "PC-C5 slice1 (UI-C5-01): Relation Index の Inspector 投影 — 双方向探索 + roundtrip（headless）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/102"
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
    - "docs/specs/pc-c2-control-relation-index.md"
    - "docs/adr/pc-c2-control-relation-index-revision-tagged-projection-enabled-only-cycle.md"
    - "docs/specs/sv-gui-03-02-arrangement-undo-slice2.md"
    - "docs/adr/sv-gui-03-02-arrangement-readmodel-undo-ops.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/pc-c5-01-relation-readmodel-coverage.md"
    - "docs/designs/pc-c5-01-relation-inspector-design.md"
    - "docs/plans/pc-c5-01-relation-inspector.md"
    - "docs/tasks/pc-c5-01-relation-inspector.md"
    - "docs/test-specs/pc-c5-01-relation-inspector.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C5 slice1 仕様: Relation Index の Inspector 投影（UI-C5-01・headless）

## What Should Be Built

Issue #102（PC-C5 Signal Flow Control Layer・Context Inspector・
Automation Lane 双方向 GUI 統合）のうち **UI-C5-01 のみ**を headless
で実装する。PC-C2（synvelis#99）で実装済みの read-only
`RelationIndex`/`RelationQuery`/`RelationReply` を、workspace の
read model と `synvelis-app` の Inspector 表示へ投影し、

`Parameter → AutomationLane / ModulationSource / Assignment → 影響先 Parameter`

の双方向探索と、anchor nav-stack による元 anchor への roundtrip を
headless fixture で検証する。

- `synvelis-workspace` に新規 `relation_ui` モジュール:
  `RelationRow` 表示 DTO、`RelationView`（rows + verbatim coverage +
  index generation）、`relation_rows(session, anchor, direction)`、
  有界 variant、roundtrip 用の小さな anchor nav-stack
  （`RelationNav`）。
- `synvelis-app` の Inspector tab に relation 用の第 2 `Table` を
  追加し、subscribed parameter target を anchor とする
  （新規 selection UI は作らない）。
- kind/state/coverage は verbatim で表面化する。
  `Unresolved`/`Disabled`/`Truncated`/`Stale`/`Unavailable` を
  「0 件」「空表」に縮退させない。

## Why Now

PC-C2 の Relation Index は project 層の正本として完成済みだが、
製品 UI へ届く投影がない。UI-C5-01 は Issue #102 の他 slice
（実 GUI・IME・multi-window・Signal Flow 製品化）を待たずに
headless で検証できる最小の双方向探索 slice であり、後続 slice が
消費する workspace read model の形をここで固定する。

## Users and Value

- Inspector を見るユーザー: subscribe 中の parameter に紐づく
  automation lane・modulation source・assignment・影響先 parameter
  が一覧でき、disabled/unresolved/truncated が正直に見える。
- 後続 slice（UI-C5-02..05、#108 Signal Flow、#114 実 GUI gate）:
  本 slice の `RelationView`/`RelationNav` をそのまま消費できる。

## Scope

- `crates/synvelis-workspace/src/relation_ui.rs`（新規）と
  `lib.rs` の additive export。
- `crates/synvelis-app/src/screens.rs`: Inspector への relation
  表示追加（`RelationScreenState` + 第 2 `Table`）。
- `crates/synvelis-app/src/app.rs`: `relation_rows()` accessor と
  `publish_project_length` 周辺の refresh 行のみ（additive）。
- `crates/synvelis-workspace/tests/rel_c5.rs`（新規）:
  双方向 nav・roundtrip・Stale/Unavailable/Truncated/
  include_disabled の fixture。
- 本 spec に連なる ADR/design/plan/task/test-spec 文書。

## Non-Goals

- `UI-C5-02..05`（実 GUI・IME・multi-window・Signal Flow 製品化）
  — **NotRun**（#114/#103 gate）。
- `ControlSession::signal_flow_view`/`lane_view` の置換 — 本 slice
  は併存。置換判断は設計文書に記録のみ。
- relation の編集 command・新規 selection UI・行 click navigation
  の GUI 配線。
- 実 OS・実 GPUI での描画証拠 — **NotRun**。
- `crates/synvelis-project/` の変更（read-only 接続のみ）。

## Acceptance Criteria

- [ ] `UI-C5-01` Parameter→Automation Lane/Source/Assignment→影響先
  Parameter へ双方向探索し、元画面（anchor）へ戻れることを
  headless fixture（`rel_c5.rs`）で検証する。
- [ ] coverage の正直な表面化: `Unresolved`/`Truncated`/`Disabled`
  （および `Stale`/`Unavailable`）を 0 件表示に潰さず、
  `RelationView.coverage`/`RelationRow.state` が verbatim で
  取得できる。
- [ ] workspace 投影は project を mutate しない
  （`RelationIndex::build`/`query` は read-only、session の
  revision/evaluator/undo に影響しない）。
- [ ] `mise run fmt-check` / `mise run check` / `mise run lint` /
  `mise run test` が pass する。

## Open Questions

- relation table の行選択→anchor 移動の GUI 配線は後続 slice
  （本 slice は headless の nav-stack と静的 table のみ）。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/102>
- Upstream: [PC-C2 spec](./pc-c2-control-relation-index.md) /
  [PC-C2 ADR](../adr/pc-c2-control-relation-index-revision-tagged-projection-enabled-only-cycle.md) /
  [PC-C2 design](../designs/pc-c2-control-relation-index-design.md)
- Slice chain: [ADR](../adr/pc-c5-01-relation-readmodel-coverage.md) /
  [design](../designs/pc-c5-01-relation-inspector-design.md) /
  [plan](../plans/pc-c5-01-relation-inspector.md) /
  [task](../tasks/pc-c5-01-relation-inspector.md) /
  [test-spec](../test-specs/pc-c5-01-relation-inspector.md)
