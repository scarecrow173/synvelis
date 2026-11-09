---
id: "DESIGN-034Up3Bm4bb2Rb9RZ1ien7"
type: "design"
status: "approved"
title: "PC-C2 Control Relation Index 設計 — 双方向辺モデル・状態証拠・有界 traversal"
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
    - "docs/adr/pc-c2-control-relation-index-revision-tagged-projection-enabled-only-cycle.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c2-control-relation-index.md"
    - "docs/adr/pc-c2-control-relation-index-revision-tagged-projection-enabled-only-cycle.md"
    - "docs/designs/pc-a-parameter-control-design.md"
    - "docs/designs/pc-b-parameter-dispatch-note-scope-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c2-control-relation-index.md"
    - "docs/adr/pc-c2-control-relation-index-revision-tagged-projection-enabled-only-cycle.md"
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/103"
  defers: []
  deferred-by: []
---

# PC-C2 Control Relation Index 設計 — 双方向辺モデル・状態証拠・有界 traversal

## Context

`Project::control`(`ControlModel`)は `targets` / `lanes` / `sources` /
`assignments` の 4 `BTreeMap` を持つ唯一の正本。PC-C5 #102 の 3 view
(Arrangement/Inspector/Signal Flow)と PC-C6 #103 の Oracle が必要と
するのは、同一 ID/Revision の下で双方向・有界・状態付きの関係
問い合わせである。本設計は `synvelis-project` に read-only 派生
index `relation` module を追加する。永続 schema・`Project` 構造・
`ProjectController`・`ControlGraph::compile` の契約はいずれも変更
しない。

## Scope

### In Scope

- `crates/synvelis-project/src/relation.rs`: `ControlEntityRef` /
  `RelationKind` / `RelationDirection` / `RelationState` /
  `RelationCoverage` / `RelationQuery` / `ControlRelation` /
  `RelationReply` / `RelationIndex`。
- `lib.rs` への additive re-export と `tests/` の受入 test。

### Out of Scope

- UI 投影(#102)、Trace 状態機械(#100)、Plugin Binding(#101)、
  Control Mapping provider、snapshot schema、他 crate。

## Components and Boundaries

- **`ControlEntityRef`** — graph node の識別子。
  `Parameter(ParameterTargetId)` / `AutomationLane(AutomationLaneId)` /
  `ModulationSource(ModulationSourceId)` / `Assignment(ModulationAssignmentId)`。
  `Copy + Ord + Hash` を derive し、BTreeMap/HashSet の key として
  決定的順序を持つ。永続化しない projection 型のため serde derive は
  付けない(codebase の projection 型慣行に一致)。
- **`RelationIndex`** — `RelationIndex::build(project: &Project,
  revision: ProjectRevision)` で構築。保持データ:
  - `revision: ProjectRevision` — 構築世代。
  - `entities: BTreeSet<ControlEntityRef>` — 4 map の全 key。
    anchor 実在判定に使う。
  - `edges: Vec<RelationEdge>` — canonical edge 列。構築順は
    `lanes → assignments(source→asn → asn→target → depth)` の
    BTreeMap 反復順で決定的。
  - `outgoing` / `incoming: BTreeMap<ControlEntityRef, Vec<u32>>` —
    edge index の adjacency。Vec は edge index 昇順 = 決定的。
- **`RelationEdge`**(private)— `kind` / `source` / `target` /
  `assignment_id: Option<ModulationAssignmentId>` / `state` /
  `cyclic: bool`(build 時に確定)。
- **query** — `RelationIndex::query(&self, &RelationQuery) ->
  RelationReply`。revision 不一致 → `Stale`、anchor 非実在 →
  `Unavailable`、それ以外は方向別 BFS。

## 辺モデル(ADR Option 1 の具体化)

| kind | source | target | assignment_id |
| --- | --- | --- | --- |
| AutomationDrives | AutomationLane(lane) | Parameter(lane.target) | None |
| ModulationDrives | ModulationSource(a.source) | Assignment(a) | Some(a.id) |
| ModulationDrives | Assignment(a) | Parameter(a.target) | Some(a.id) |
| DepthControlledBy | Parameter(a.depth_source) | Assignment(a) | Some(a.id) |

- assignment 1 件につき ModulationDrives 2 辺 + depth_source があれば
  DepthControlledBy 1 辺。lane 1 件につき AutomationDrives 1 辺。
- dangling endpoint(lane.target / a.source / a.target / a.depth_source
  が対応 map に無い)はその辺のみ `Unresolved`。validated project
  では発生しないが、直接構築された project でも panic しない。
- SCC(cycle)判定は **Enabled 辺のみ**の graph で行う。実装は
  enabled 辺の node adjacency に対する反復 Kosaraju
  (または Tarjan)— 深い再帰を避けるため明示 stack。edge の
  `cyclic` は「source/target が同一 SCC(size>1)」で真。self-loop は
  node 型が異なる辺構造上発生しない。
  `ControlGraph::compile` の `depth_source→target` edge と同じ
  enabled-only 意味論なので、compile が `ControlCycle` を返す
  project では対応辺が Unresolved になり、それ以外は Enabled の
  まま — 表示と評価が矛盾しない。

## 状態と traversal

- `RelationState`: `Enabled` / `Disabled` / `Unresolved` /
  `Unsupported`(予約・未生成)。確定優先順位: dangling → `Unresolved`、
  `!enabled` → `Disabled`、cyclic → `Unresolved`、他 → `Enabled`。
  Disabled 辺は SCC graph に含めないため cyclic 判定と独立。
- BFS(query):
  - `expected_revision != index.revision` → `{project_revision:
    index.revision, relations: [], coverage: Stale}`。
  - `!entities.contains(anchor)` → 同上 `Unavailable`。
  - `max_depth == 0` → `Complete` + 空( anchor のみ、辺を展開しない)。
  - node を depth 付きで dequeue。node depth `d < max_depth` のとき
    adjacency を走査し、各 edge について:
    - `state == Disabled && !include_disabled` → emit も enqueue も
      しない(寄与なしと一致)。
    - それ以外 → `relations.len() == max_edges` なら `Truncated` で
      打ち切り、さもなくば emit。未訪問の対向 node を `d+1` で
      enqueue(`d+1 <= max_depth` のみ)。
  - `visited_nodes` / `visited_edges` で再訪を防ぐ。`Both` は
    outgoing→incoming の順に両 adjacency を走査し、同一 edge の
    重複 emit は `visited_edges` で排除。
  - emit される `ControlRelation` は常に stored direction
    (source→target)。query の direction は「どの辺を踏むか」だけを
    決める。
- 決定性: adjacency Vec は edge index 昇順、BFS は FIFO — 同一
  project+query で常に同一 `relations` 列と coverage を返す。

## Data and Control Flow

```
Project::control (BTreeMaps)
  → RelationIndex::build(project, revision)   // O(N)、SCC 込み
  → RelationIndex::query(&RelationQuery)      // O(V+E) worst、出力 ≤ max_edges
  → RelationReply { project_revision, relations, coverage }
```

consumer は revision 前進ごとに index を再構築する。index は
`Project` を借用して構築するだけで所有も変更もしない — 旧 index を
保持したまま project が進んでも、query は Stale で拒否される。

## Non-Functional Constraints

- read-only: `&Project` のみを受け、`Project`/DSP/Automation
  series/Undo に触れない(test で担保)。
- 決定性: 同一入力で同一出力(BTreeMap 由来の順序のみ)。
- 有界性: query 仕事量 O(V+E)、返却 ≤ `max_edges`、打ち切りは
  `Truncated` で明示。index 構築 O(N)。
- 非 RT 経路: UI/inspection 向け projection であり RT callback から
  呼ばない前提(alloc を伴う)。RT 責務は #100/#101 側。

## Risks and Trade-offs

- **2-hop ModulationDrives は辺数が assignment の 2 倍**になるが、
  node/edge の意味が一貫し anchor=Assignment が自然に扱える利点が
  上回る(ADR Option 2 却下理由)。
- **SCC を build 時に計算**するため、cycle の無い巨大 graph でも
  O(V+E) の追加走査が入る — index 構築は非 RT・revision ごと 1 回
  なので許容。query 側に cycle 判定を持ち込まない。
- **`Both` direction は無向到達**になる — 「anchor に繋がる全関係」
  の探索用途向け。canonical direction は常に stored なので consumer
  は向きを失わない。
- **Stale/Unavailable は辺を返さない** — stale な ID 参照を UI に
  見せない安全側の選択。debug 用に関係を返したい場合は consumer が
  自前で index を保持・比較する。

## Deferred Design Concerns

- Control Mapping(MIDI CC)provider — `RelationProvider` 抽象は
  provider 追加時に設計。本 slice は fake binding を出さない。
- 大規模 graph(数万 node 級)の実測 — fixture(数百〜千 edge)で
  budget を実測し、巨大 graph は #103 Oracle の fixture に委譲
  (NotRun 記録)。

## Verification Notes

- [ ] `tests/` に REL-C2-01〜05 対応の acceptance test
  (双方向検索・複数 assignment/depth・Disabled・Stale/Unresolved・
  truncation/cycle/budget/read-only 不変)。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` green。
