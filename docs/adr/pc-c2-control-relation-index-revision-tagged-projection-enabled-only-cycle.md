---
id: "ADR-034Up0Uogue2QzCYAOA09T"
type: "adr"
status: "accepted"
title: "PC-C2 Control Relation Index — revision-tagged 派生双方向 projection と enabled-only cycle 意味論"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/99"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c2-control-relation-index.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c2-control-relation-index.md"
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/pc-c2-control-relation-index-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
    - "https://github.com/scarecrow173/synvelis/issues/99"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/103"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# PC-C2 Control Relation Index — revision-tagged 派生双方向 projection と enabled-only cycle 意味論

## Status

accepted

## Context

Issue #99 は `Project::control` 上の双方向 Control Relation
Index/Projection を要求する。既存の `ControlSession::signal_flow_view`
は target への incoming assignment の一部射影であり、Automation Lane /
Depth Driver を含む双方向探索と一貫した状態証拠を持つ index は存在
しない。`ControlModel` は `targets`/`lanes`/`sources`/`assignments`
の 4 つの `BTreeMap` を持つ唯一の正本であり、ここに第二の正本や
永続 schema を追加してはならない。

設計上の争点は 3 つある:

1. **辺モデル**: `ModulationSource→Assignment→Target` をどう辺に
   写すか。`ControlEntityRef` は `Assignment` を node として含み、
   anchor=Assignment の双方向問い合わせを自然に扱う必要がある。
2. **状態意味論**: Disabled assignment の辺を index に含めるか、
   cycle をどう検出・報告するか。`ControlGraph::compile` は enabled
   assignment のみで zero-delay cycle を拒否する — index の
   Unresolved 判定がこの契約と乖離すると、表示と評価が矛盾する。
3. **revision tag**: `ProjectRevision` は `ProjectController` が所有
   し `Project` は保持しない。index がどの世代の投影かをどう
   表明し、stale な返信をどう拒否するか。

## Considered Options

* Option 1(採用): Assignment を第一級 node とする 2-hop 辺モデル
  (`ModulationSource→Assignment`、`Assignment→ParameterTarget`、
  `ParameterTarget→Assignment`(depth))を採用し、state を構築時に
  確定する revision-tagged 派生 index とする。
* Option 2: `ModulationDrives` を `ModulationSource→ParameterTarget`
  の 1-hop 辺とし assignment を `assignment_id` 属性に留める —
  anchor=Assignment の Incoming/Outgoing が定義不能になり、
  REL-C2-02 の別辺区別を traversal で表現できない。却下。
* Option 3: index を `Project` に永続 field として持たせる、または
  `ControlModel` に逆引き map を追加する — 第二正本・schema 変更に
  なり「二重 Control 正本は作らない」に反する。却下。
* Option 4: query ごとに `Project::control` を走査する on-demand
  projection(`signal_flow_view` の拡張) — 双方向 transitive・
  truncation・revision tag を各 consumer が再実装することになり、
  一貫した状態証拠を保証できない。却下。

## Decision

1. **Assignment は第一級 node。** 辺は
   `AutomationLane→ParameterTarget`(AutomationDrives)、
   `ModulationSource→Assignment`・`Assignment→ParameterTarget`
   (ModulationDrives の 2 hop)、`ParameterTarget→Assignment`
   (DepthControlledBy)の 4 系列とし、全て stored direction を保持
   する。`Incoming`/`Outgoing`/`Both` は traversal 方向のみを変え、
   返す `ControlRelation` は常に canonical direction。
2. **state は構築時確定の 3+1 値。** `Disabled` は `enabled == false`
   の assignment 由来辺(既定 query では結果・traversal 双方から
   除外)。`Unresolved` は dangling endpoint または **enabled 辺のみ**
   の SCC に参加する辺 — SCC は enabled 辺だけで構成した graph 上で
   計算し、`ControlGraph::compile` の zero-delay cycle 契約と同じ
   意味論に揃える。`Unsupported` は将来 provider 用に予約し本
   slice では生成しない。
3. **revision tag は構築時 revision の保持 + query 時比較。**
   `RelationIndex::build(project, revision)` が `ProjectRevision` を
   保持し、`RelationQuery.expected_revision` との不一致は
   `coverage = Stale`・辺なしで応答する。`Project`/snapshot/
   controller の契約は変更しない。
4. **有界性は index 側が保証。** adjacency は `BTreeMap` 由来の
   決定的順序、traversal は `max_depth`/`max_edges`/visited
   node・edge で有界化し、超過は `Truncated`。index 構築は O(N)、
   query 最悪仕事量は O(V+E)。

## Consequences

- `crates/synvelis-project/src/relation.rs` が新規 module となり、
  `lib.rs` は additive な re-export のみ。既存の control/eval/ops/
  snapshot コードへの変更はない。
- consumer(#102)は index を revision ごとに再構築して保持する
  責任を負う — 旧世代 index への query は Stale で拒否されるため、
  UI 側の世代管理は coverage を見て行う。
- depth cycle を含む project は compile 不可だが index は構築可能
  で、cycle 辺は Unresolved として表示される — 「評価できない
  関係を UI が隠さない」契約になる。
- Disabled 辺は `include_disabled` 指定時のみ traversal を通る —
  「表示のための探索」と「寄与のある関係」を同じ index で両立
  させるための明示的フラグである。
