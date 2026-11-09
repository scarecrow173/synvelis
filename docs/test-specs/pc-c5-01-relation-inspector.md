---
id: "TSPEC-aB03BdXeeD21Dc9dD9uKJ"
type: "test-spec"
status: "approved"
title: "PC-C5 slice1 テスト仕様 — Relation Inspector 投影（UI-C5-01・headless）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c5-01-relation-inspector.md"
    - "docs/designs/pc-c5-01-relation-inspector-design.md"
    - "docs/tasks/pc-c5-01-relation-inspector.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/pc-c5-01-relation-inspector.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C5 slice1 テスト仕様

## 範囲

headless のみ（`ProjectSession::apply` + `ProjectOp` fixture、
`relation_rows`/`relation_rows_with`/`RelationNav`）。実機 lane・
実 GPUI 描画は対象外（**NotRun**）。

## Feature / Rule / Example

### Feature: 双方向 relation 投影

- Rule: anchor entity の直接 relation（`RelationDirection::Both`,
  depth=1 既定）が `RelationView.rows` に deterministic 順で返る。
- Example: anchor=Parameter に対し `AutomationLane→Parameter`
  （AutomationDrives）、`Assignment→Parameter`・
  `Parameter→Assignment`（ModulationDrives/DepthControlledBy）が
  含まれる。

### Feature: hop 移動による網羅探索

- Rule: `RelationRow::neighbor(anchor)` が反対側 entity を返し、
  `RelationNav` の push で anchor を進め、pop で戻れる。
- Example: Parameter→Assignment→Source→Assignment→影響先
  Parameter→…→元 Parameter の roundtrip。

### Feature: coverage の正直な表面化

- Rule: `RelationView.coverage` は `RelationCoverage` を verbatim
  で保持し、`rows` が空でも coverage が残る。
- Example: 旧 `expected_revision` → `Stale`、未知 anchor →
  `Unavailable`、`max_edges` 超過 → `Truncated`。

### Feature: state の正直な表面化

- Rule: `RelationRow.state` は `RelationState` を verbatim で
  保持する。disabled assignment の edge は `include_disabled`
  に従い、含める場合は `Disabled` として出る。
- Example: enabled-graph cycle の edge は `Unresolved`。

### Feature: 非破壊性

- Rule: `relation_rows*` は project/session を mutate しない。
  query 前後で `session.revision()` と control model が不変。

## Fixture 方針

- `ProjectSession::apply(session, ProjectOp, revision)` のみで
  target/lane/source/assignment を構築（ops + post-apply oracle
  を通る正規経路）。dangling 参照は oracle が reject するため
  ops 経由では作れず、`Unresolved` は enabled-graph cycle
  （相互 depth_source を持つ 2 assignment）で作る。
- disabled edge は `AddModulationAssignment { enabled: false }`。
- id 取得は `control.*.keys()` の deterministic 順序
  （target は `is_issuable_raw` で bootstrap master fader を除外）。

## Sources and Relations

- [spec](../specs/pc-c5-01-relation-inspector.md) /
  [ADR](../adr/pc-c5-01-relation-readmodel-coverage.md) /
  [design](../designs/pc-c5-01-relation-inspector-design.md) /
  [plan](../plans/pc-c5-01-relation-inspector.md) /
  [task](../tasks/pc-c5-01-relation-inspector.md)
