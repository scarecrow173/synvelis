---
id: "TSPEC-034Upc3ciohad3ZlmsOqVl"
type: "test-spec"
status: "approved"
title: "PC-C2 Control Relation Index E2E — 双方向・状態・stale・有界性・read-only"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c2-control-relation-index.md"
    - "docs/designs/pc-c2-control-relation-index-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c2-control-relation-index.md"
    - "docs/designs/pc-c2-control-relation-index-design.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-c2-control-relation-index.md"
    - "docs/designs/pc-c2-control-relation-index-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  defers: []
  deferred-by: []
---

# PC-C2 Control Relation Index 検証仕様

`crates/synvelis-project/tests/pc_c2_relation.rs` が担保する契約。
headless のみ — 実 UI 投影(#102)・実 OS/Plugin/RT Gate(#103/#114)
は対象外。

## Feature: 双方向 Relation 検索(REL-C2-01)

- Rule: `ModulationSource`(LFO/Macro/StepSeq)anchor の `Outgoing`
  query は `max_depth >= 2` で `Assignment` を経て全 `Parameter`
  target へ到達し、各辺は stored direction
  (`ModulationSource→Assignment`、`Assignment→Parameter`)で返る。
  - Example: 1 source → 2 target の 2 assignment で、source anchor
    Outgoing が 4 辺(2×2hop)を返し、target anchor Incoming が
    対応する逆側辺を返す。
- Rule: `ParameterTarget` anchor の `Incoming` は automation lane と
  assignment 経路の両方を含み、`AutomationLane` anchor の `Outgoing`
  は `AutomationDrives` 辺を返す。
  - Example: lane→target + source→assignment→target が同一 index
    で target の Incoming に両方現れる。
- Rule: `Incoming`/`Outgoing`/`Both` で reachable 集合が変わり、
  `Both` は無向到達で canonical direction を保持する。
  - Example: `Both` は Incoming∪Outgoing の和集合を重複なく返す。

## Feature: 複数 Assignment・Depth Driver・Disabled(REL-C2-02)

- Rule: 同一 source+target の複数 assignment は別 `Assignment` node
  経由の別辺として区別され、`assignment_id` で識別できる。
  - Example: 同一 (source,target) に 2 assignment → 4 ModulationDrives
    辺、assignment_id が 2 種類。
- Rule: `depth_source` を持つ assignment は `DepthControlledBy`
  辺(`Parameter→Assignment`)を追加し、driver→dependent の向きを
  保持する。driver 自身の Incoming/Outgoing で双方向に辿れる。
  - Example: P1 が depth_source、P2 が target の assignment で、
    P2 Incoming(depth2)に P1→Assignment 辺、P1 Outgoing に同辺。
- Rule: `enabled == false` の assignment 由来辺は `Disabled`。
  既定 query(`include_disabled == false`)では結果にも traversal に
  も現れず、`include_disabled == true` では `Disabled` state で
  返り traversal も通る。
  - Example: disabled assignment の target への Incoming に他の
    enabled 経路だけが現れ、disabled 辺は出ない。
- Rule: Disabled assignment は評価経路に寄与しない
  (`ControlGraph::compile`/`evaluate_control` の既存契約と一致)。
  - Example: disabled assignment のみの target の評価値が
    assignment 無しと一致する。

## Feature: Revision・Stale・Unresolved(REL-C2-03)

- Rule: `expected_revision != index.revision` の query は
  `coverage = Stale`・辺なし・`project_revision` は index 世代。
  - Example: index 構築後に `ProjectController::apply` で revision
    前進 → 旧 expected_revision の query は Stale。
- Rule: anchor が構築 revision 時点で非実在 → `coverage =
  Unavailable`・辺なし。anchor 実在・関係無し → `Complete` + 空。
  - Example: 存在しない `ParameterTargetId` anchor → Unavailable;
    lane/sources/assignments を持たない target anchor → Complete 空。
- Rule: assignment 削除・target 消失は新 revision の再構築 index に
  反映され、旧世代 query は Stale。dangling 参照(検証を通っていない
  直接構築 project)は該当辺が `Unresolved`。
  - Example: `RemoveModulationAssignment` 後の新 index では対応辺が
    消え Complete;`control.targets` から直接除去した lane.target
    dangling → `AutomationDrives` 辺が Unresolved。

## Feature: 有界 traversal・安全拒否・budget(REL-C2-04)

- Rule: `max_edges` 超過は `coverage = Truncated` で決定的に打ち
  切り、`max_depth` は hop 数を制限する。visited node/edge で循環・
  菱形の再訪を防ぐ。
  - Example: max_edges=2 の query が Truncated + 先頭 2 辺を返す;
    max_depth=1 が anchor 直付け辺のみ返す。
- Rule: depth_source の相互 cycle(A.depth_source=B, B.depth_source=A
  相当)を含む project でも build/query は終了し、cycle 参加辺は
  `Unresolved`。`ControlGraph::compile` は従来どおり `ControlCycle`
  で拒否する(契約不変)。
  - Example: 2 target の相互 depth_source assignment で compile が
    Err(ControlCycle)、index の対応 ModulationDrives/DepthControlledBy
    辺が Unresolved。
- Rule: index 構築は O(N)、query 最悪 O(V+E)。fixture 規模(数百
  edge)で build/query 時間を実測し test に記録する。
  - Example: 生成 fixture で `Instant` 計測値を test output/impl
    record に残す。巨大 graph(数万 node)の実測は NotRun。

## Feature: read-only 不変性(REL-C2-05)

- Rule: index 構築・全 query kind の前後で `Project` が同一
  (PartialEq)、`evaluate_control`/`ControlGraph::compile` の結果が
  不変、`ProjectController` の undo/redo が影響を受けない。
  - Example: index build+query 後に `project == before`、
    `controller.undo()` が直前 apply を正しく戻す。
- Rule: view の探索/選択相当の繰り返し query が project 状態を
  一切変更しない(同一 query の反復で結果が決定的に一致)。
  - Example: 同一 `RelationQuery` を 2 回実行して `relations` が
    完全一致。

## Guarantees

- `RelationReply` は常に `project_revision`(index 世代)・
  `relations`・`coverage` を持ち、Stale/Unavailable は辺を返さない。
- 返却辺順序は決定的(BTreeMap 由来)。同一入力で同一出力。
- `RelationIndex` は `Project` を借用構築のみで、所有・変更しない。

## Non-goals

- 実 UI(Arrangement/Inspector/Signal Flow)への投影接続 — #102。
- Control Trace 状態機械 — #100。Plugin Binding — #101。
- MIDI CC 等 Control Mapping provider — 将来の optional 拡張。
- 巨大 graph(数万 node 級)の性能実測 — #103 Oracle fixture に委譲、
  本 test-spec では NotRun。
- `synvelis-workspace`/`synvelis-app` の変更。

## Risk

- 双方向意味論の誤実装(direction 逆取り・Both の二重計上)は UI の
  誤ナビゲーションに直結するため、方向別の到達集合を個別 test で
  固定する。
- SCC/cycle 判定を enabled-only に揃え損なうと、compile が受理する
  project で誤って Unresolved を出す — compile 受理/拒否との
  対応 test で担保する。
