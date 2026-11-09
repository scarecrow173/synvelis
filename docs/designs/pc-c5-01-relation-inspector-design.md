---
id: "DESIGN-AkFUh4M60YuHo8COCvb0Um"
type: "design"
status: "approved"
title: "PC-C5 slice1 Relation Inspector 設計 — RelationRow/RelationView/RelationNav + Inspector 第 2 Table"
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
  implements:
    - "docs/specs/pc-c5-01-relation-inspector.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/pc-c5-01-relation-readmodel-coverage.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c5-01-relation-inspector.md"
    - "docs/adr/pc-c5-01-relation-readmodel-coverage.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C5 slice1 設計: Relation Index の Inspector 投影

## Context

PC-C2 の `synvelis_project::relation` は `RelationIndex::build(
&Project, ProjectRevision)` → `query(&RelationQuery)` →
`RelationReply{project_revision, relations, coverage}` を提供済み。
`ProjectSession::project()`/`revision()`/`apply()` は既存。
`arrangement_ui.rs` が「session を読んで表示 DTO を返す」
read-model の先例で、`MainScreenState` + `publish_project_length`
refresh + `screens.rs` Table mount が確立済み。
`ui_ops::capture_screen_state` が subscribed param の
`inspector.target`（tagged `param-<32hex>`）を毎 pump で
`MainScreenState.inspector` に書く。

## Overview

```
ProjectSession ──project()/revision()──▶ RelationIndex::build
                                              │ query(anchor, Both, depth=1, …)
                                              ▼
                                    RelationReply (verbatim)
                                              │
                                              ▼
synvelis-workspace::relation_ui ──▶ RelationView{anchor,direction,
                                    project_revision,coverage,rows}
                                              │ RelationRow{source,target,
                                              │  kind,state,assignment}
                                              ▼
synvelis-app::app.rs relation_rows() ──▶ RelationScreenState
                                              │ screens.rs Inspector tab
                                              ▼
                                    ViewSpec Table (read-only)
```

編集・mutation 経路は一切追加しない。全て read-only 投影。

## Detailed Design

### `crates/synvelis-workspace/src/relation_ui.rs`（新規）

```rust
pub const RELATION_VIEW_MAX_DEPTH: u16 = 1;
pub const RELATION_VIEW_MAX_EDGES: usize = 32;
pub const RELATION_VIEW_INCLUDE_DISABLED: bool = true;

pub struct RelationViewOptions {
    pub max_depth: u16,
    pub max_edges: usize,
    pub include_disabled: bool,
    pub expected_revision: Option<ProjectRevision>, // None = live
}
impl Default for RelationViewOptions { /* 上記 const */ }

pub struct RelationRow {
    pub source: ControlEntityRef,       // stored direction
    pub target: ControlEntityRef,
    pub kind: RelationKind,             // verbatim
    pub state: RelationState,           // verbatim (Disabled/Unresolved 含む)
    pub assignment: Option<ModulationAssignmentId>,
}
impl RelationRow {
    pub fn source_label(&self) -> String;   // tagged id
    pub fn target_label(&self) -> String;
    pub fn neighbor(&self, anchor: ControlEntityRef)
        -> Option<ControlEntityRef>;        // anchor に接する edge のみ Some
}

pub struct RelationView {
    pub anchor: ControlEntityRef,
    pub direction: RelationDirection,
    pub project_revision: ProjectRevision,  // index generation
    pub coverage: RelationCoverage,         // verbatim
    pub rows: Vec<RelationRow>,             // deterministic query order
}
impl RelationView {
    pub fn anchor_label(&self) -> String;
    pub fn is_stale(&self, session: &ProjectSession) -> bool;
}

pub struct RelationNav { anchors: Vec<ControlEntityRef> }
impl RelationNav {
    pub fn new/current/can_go_back/push/pop/clear
}

pub fn relation_rows(session, anchor, direction) -> RelationView
    // = relation_rows_with(session, anchor, direction, Default)
pub fn relation_rows_with(session, anchor, direction, options)
    -> RelationView
```

- `relation_rows_with` は `RelationIndex::build(session.project(),
  session.revision())` → `index.query(&RelationQuery{…,
  expected_revision: options.expected_revision.unwrap_or(live)})`
  を実行し、`RelationReply` を `RelationView`/`RelationRow` へ
  1:1 写像する。project/session は一切 mutate しない。
- `neighbor(anchor)` は `source == anchor → target`、
  `target == anchor → source`、それ以外 `None`。depth-1 query
  では全 row が anchor に接するため常に `Some`。
- `RelationNav` は純粋な anchor stack（`Vec<ControlEntityRef>`）。
  `push` は current と同一なら no-op、`pop` は 1 つ戻り
  `Option` を返す。session/project を持たない値型。
- entity→tagged 文字列は各 ID の `to_tagged()`
  （`param-`/`automation-lane-`/`mod-source-`/`mod-assign-`）。

### `crates/synvelis-app/src/screens.rs`

- `RelationScreenState { anchor: Option<String>,
  coverage: Option<RelationCoverage>, rows: Vec<RelationRow> }`
  を追加し、`MainScreenState.relations` に持つ。
  `RelationScreenState::from_view(RelationView)` で写像
  （anchor/coverage verbatim、rows はそのまま）。
- `relation_label`（anchor + coverage の表示 Label）と
  `relation_table`（columns: `Kind`/`Source`/`Target`/
  `Assignment`/`State`、`selection: None` の read-only Table）を
  追加し、Inspector tab の children を
  `[target label, breakdown table, relation label, relation table]`
  にする。
- state/kind は `{:?}` で verbatim 表示（`Enabled`/`Disabled`/
  `Unresolved`/`AutomationDrives`/`ModulationDrives`/
  `DepthControlledBy`）。coverage も `{:?}`。

### `crates/synvelis-app/src/app.rs`

- `pub fn relation_rows(&self) -> Option<RelationView>`:
  `main_screen.inspector.target`（tagged）を
  `ParameterTargetId::from_tagged` → `ControlEntityRef::Parameter`
  に parse し、`synvelis_workspace::relation_rows(session, entity,
  RelationDirection::Both)` を返す。target 未確定/parse 失敗は
  `None`（query しない = 「anchor なし」であり「relation 0 件」
  ではない）。
- `publish_project_length` に refresh 行を additive に追加:
  `screen.relations = relation_view(session, inspector.target)
  .map_or_else(Default::default, RelationScreenState::from_view)`。
  `inspector.target` は同じ service op の `ui_pump`→
  `capture_screen_state` で更新済みのため fresh。
- private helper `relation_view(session, anchor_tagged)` に
  accessor/publish の共通化を集約。

### `crates/synvelis-workspace/tests/rel_c5.rs`（新規）

`ProjectSession::apply` + `ProjectOp` のみで fixture を構築
（`arrangement_rows.rs`/`pc_a.rs` の先例）。

fixture 概形:

- `P1`（drive 対象 param）、`P2`（depth source）、`P3`（modulated）
- `L1`: automation lane → P1
- `S1`/`S2`/`S3`: modulation source（macro/lfo）
- `A1`: S1→P1 enabled（depth_source なし）
- `A2`: S2→P3 enabled, depth_source=P1 → `P1→A2` DepthControlledBy
- `A3`: S3→P1 **disabled** → Disabled edge
- cycle 用に別 fixture: `A4`: S→P2 depth_source=P3,
  `A5`: S→P3 depth_source=P2 で enabled-graph SCC を作り
  edge を `Unresolved` にする

検証項目（受入対応）:

| test | 内容 |
| --- | --- |
| 双方向 rows | anchor=P1 の rows に `L1→P1`(AutomationDrives), `A1→P1`/`A3→P1`(ModulationDrives), `P1→A2`(DepthControlledBy) が含まれ、state が verbatim |
| Parameter→Source | P1→A1→S1 の hop で source entity に到達 |
| 影響先 Parameter | P1→A2→P3 の hop で affected param に到達 |
| roundtrip | `RelationNav` push/pop で P1→…→P1 に戻り、stack が空に近い状態へ復帰 |
| Stale | `expected_revision` に旧 revision を渡すと coverage=Stale・rows 空 |
| Unavailable | 未知 anchor（存在しない id）で coverage=Unavailable |
| Truncated | `max_edges` を小さくすると coverage=Truncated・rows は決定的 prefix |
| include_disabled | false で Disabled edge が出ない / true で `state=Disabled` で出る |
| Unresolved | cycle fixture で edge state=Unresolved が潰れない |
| 非破壊 | query 前後で `session.revision()`・project 内容が不変 |

## File Map

| file | 変更 |
| --- | --- |
| `crates/synvelis-workspace/src/relation_ui.rs` | 新規 |
| `crates/synvelis-workspace/src/lib.rs` | `pub mod relation_ui;` + re-export（additive） |
| `crates/synvelis-app/src/screens.rs` | `RelationScreenState`・`MainScreenState.relations`・`relation_label`/`relation_table`・Inspector tab children |
| `crates/synvelis-app/src/app.rs` | `relation_rows()` accessor・`publish_project_length` refresh 行・private helper |
| `crates/synvelis-workspace/tests/rel_c5.rs` | 新規 |

## Risk / Rollback

- 影響は additive のみ。`RelationScreenState` 追加で
  `MainScreenState` の `PartialEq`/`Default` derive は維持される。
- app 側の変更は Inspector tab 内と publish の 1 行に限定し、
  別 dispatch 作業中の領域（close-veto/shell/service/ui_ops/host、
  project crate）に触れない。
- rollback は本 slice の commit revert で完結（永続形式・
  public wire 契約の変更なし）。

## Verification

- `mise run fmt-check` / `mise run check` / `mise run lint` /
  `mise run test`（canonical）。
- 実 OS/GPUI 描画・UI-C5-02..05 は **NotRun** として記録。

## Sources and Relations

- [spec](../specs/pc-c5-01-relation-inspector.md) /
  [ADR](../adr/pc-c5-01-relation-readmodel-coverage.md) /
  [plan](../plans/pc-c5-01-relation-inspector.md) /
  [task](../tasks/pc-c5-01-relation-inspector.md) /
  [test-spec](../test-specs/pc-c5-01-relation-inspector.md)
