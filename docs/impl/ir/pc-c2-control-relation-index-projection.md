---
id: "IMPL-034UpGfvHCv8xjHEH5ys6e"
type: "impl"
status: "completed"
title: "PC-C2 双方向 Control Relation Index/Projection"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/99"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/relation.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/pc_c2_relation.rs"
      - type: "file"
        path: "docs/specs/pc-c2-control-relation-index.md"
      - type: "file"
        path: "docs/adr/pc-c2-control-relation-index-revision-tagged-projection-enabled-only-cycle.md"
      - type: "file"
        path: "docs/designs/pc-c2-control-relation-index-design.md"
      - type: "file"
        path: "docs/plans/pc-c2-control-relation-index.md"
      - type: "file"
        path: "docs/test-specs/pc-c2-e2e-control-relation-index.md"
      - type: "file"
        path: "docs/tasks/pc-c2-task1-relation-index.md"
      - type: "file"
        path: "docs/impl/ir/pc-c2-control-relation-index-projection.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/tasks/pc-c2-task1-relation-index.md"
    - "docs/plans/pc-c2-control-relation-index.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c2-control-relation-index.md"
    - "docs/adr/pc-c2-control-relation-index-revision-tagged-projection-enabled-only-cycle.md"
    - "docs/designs/pc-c2-control-relation-index-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c2-e2e-control-relation-index.md"
  verified-by:
    - "docs/test-specs/pc-c2-e2e-control-relation-index.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/103"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C2 双方向 Control Relation Index/Projection

## Summary

Issue #99。`Project::control`(`ControlModel` の 4 `BTreeMap`)上の
revision-tagged 派生双方向 index `RelationIndex` を
`synvelis-project` に実装した。`AutomationDrives` /
`ModulationDrives` / `DepthControlledBy` の canonical 辺を構築し、
`Incoming`/`Outgoing`/`Both` の有界 BFS query が
`RelationReply{project_revision, relations, coverage}` を返す。
二重 Control 正本は作らず、query は Project・評価・compile・
Undo を一切変更しない。

## Context

Spec: docs/specs/pc-c2-control-relation-index.md
ADR: docs/adr/pc-c2-control-relation-index-revision-tagged-projection-enabled-only-cycle.md
Design: docs/designs/pc-c2-control-relation-index-design.md
Plan: docs/plans/pc-c2-control-relation-index.md
Test spec: docs/test-specs/pc-c2-e2e-control-relation-index.md
Task: docs/tasks/pc-c2-task1-relation-index.md

`ControlSession::signal_flow_view`(synvelis-workspace)は incoming
assignment の一部しか返さず、Automation Lane/Depth Driver の双方向
探索と一貫した状態証拠を持つ Index が未実装だった。本 slice は
project 層の共有 relation source を提供し、UI 投影(#102)・
Trace(#100)・Plugin Binding(#101)は各 owner Issue に残す。

## Implementation

### Changes

- `crates/synvelis-project/src/relation.rs`(新規):
  - 公開型: `ControlEntityRef`(Parameter/AutomationLane/
    ModulationSource/Assignment + 4 ID からの `From`)、
    `RelationKind`、`RelationDirection`、`RelationState`
    (Enabled/Disabled/Unresolved/Unsupported)、`RelationCoverage`
    (Complete/Truncated/Stale/Unavailable)、`RelationQuery`、
    `ControlRelation`、`RelationReply`、`RelationIndex`。
  - `RelationIndex::build(project, revision)`: entities(BTreeSet)・
    edges(Vec、lanes→assignments の BTreeMap 反復順で決定的)・
    outgoing/incoming adjacency。辺規則は設計書の表どおり。
    dangling endpoint → Unresolved、`!enabled` → Disabled。
  - cycle 検出: non-disabled 辺の graph に反復 Kosaraju
    (明示 stack、再帰なし)を適用し、同一 non-trivial SCC の両端を
    持つ辺を Unresolved に確定。`ControlGraph::compile` の
    enabled-only `depth_source→target` 意味論と整合(同じ edge 集合で
    cycle 判定するため、compile が `ControlCycle` を返す project
    では対応辺が Unresolved、disabled cycle は対象外)。
  - `RelationIndex::query(&RelationQuery)`: revision 不一致 →
    Stale・辺なし、anchor 非実在 → Unavailable、`max_depth==0` →
    Complete 空。方向別 bounded BFS(Both は node ごとに
    outgoing→incoming の順、`visited_edges` で重複 emit 排除、
    canonical stored direction を保持)。`max_edges` 到達で
    Truncated。
- `crates/synvelis-project/src/lib.rs`: `pub mod relation;` +
  additive re-export のみ。
- `crates/synvelis-project/tests/pc_c2_relation.rs`(新規):
  REL-C2-01..05 の acceptance test 5 件。

### Decisions During Implementation

- **SCC 対象は non-disabled 辺全体**(Enabled + dangling-Unresolved)。
  compile は dangling endpoint も node として cycle 判定するため、
  Unresolved 辺を SCC graph に含めることで「compile が拒否する
  project の辺が Enabled のまま残る」乖離を防ぐ。Disabled 辺は
  compile 同様に graph から除外。
- **`Both` は無向到達**: node ごとに outgoing→incoming の両方を
  走査するため、anchor の連結成分内を hop 制限付きで探索する。
  emit される辺は常に stored direction で、重複は
  `visited_edges` で排除。test で `t1←A1←source→A2` のような
  横断到達を確認済み。
- **`RelationReply.project_revision` は index 世代**: Stale/Unavailable
  でも index の revision を返し、consumer が live revision と比較して
  世代差を検出できる(設計どおり)。
- **edge index は `usize`**: u32 への cast を避け clippy
  `cast_possible_truncation` を構造的に解消。

## Related Experiments

`metadata.experiments.adopted` / `rejected` はともに空。
`docs/impl/exp/` にも PC-C2 relation index に対応する実験記録
（`.jsonl` / `.md`）は存在しない。

## Validation

実行環境: Windows (Orca worktree)、rust toolchain 1.99.0 (mise pin)。

| コマンド | exit | 結果 |
| --- | --- | --- |
| `mise run fmt-check` | 0 | Passed |
| `mise run check` | 0 | Passed(workspace 全 target) |
| `mise run lint` | 0 | Passed(`-D warnings`、clippy pedantic 含む) |
| `mise run test` | 0 | Passed(workspace 全 test + doc-test) |

対象 test: `cargo test -p synvelis-project --test pc_c2_relation`
(canonical = mise run test に内包)→ 5/5 Passed。

REL-C2-04 fixture 計測(`--nocapture` 実測): 400 edges の project で
`RelationIndex::build` ≈ 1.97ms、`Both` query(240 relations)≈
0.38ms。debug build・単発計測のため絶対値は環境依存だが、O(N)
構築・有界 query の目安として記録。

### 既知の repo 健全性(既存・本スライス非因果)

`doc_maintenance apply` の rescan で 286 件の既存 finding
(legacy doc の invalid-id/status/相互参照不整合)を観測したが、
PC-C2 チェーン起因の finding はない。広範な legacy 修復は scope 外。

### 未検証(正直な記録)

- **巨大 graph(数万 node 級)の実測**: fixture(400 edges)で budget
  機構と計測は実施。巨大 graph は NotRun — #103 Oracle の fixture
  領域として defer。
- **実 UI 投影との接続**: #102 の scope。Arrangement/Inspector/
  Signal Flow が同一 ID/Revision を共有することの UI レベル検証は
  未実施(read-only 不変性は test で担保済み)。
- **RT 経路**: relation index は非 RT projection であり RT
  callback からの呼出は想定しない。`mise run test-rt` 相当の
  RT 契約検証は本 slice の変更面に非該当。
- **serde 永続化**: projection 型のため意図的に serde derive なし
  (永続 schema 変更なし)。

## Risks

- 2-hop ModulationDrives で辺数は assignment の約 2 倍 —
  assignment node を明示 entity とする設計判断(ADR)どおり。
- `Both` は連結成分の無向到達になる — 「anchor に繋がる全関係」
  用途向け。canonical direction は失われない。
- Stale/Unavailable は辺を返さない安全側選択 — stale な ID を
  UI に見せない。旧世代の中身を debug したい consumer は自前で
  index を保持・比較する。

## Follow-ups

- #102: 3 view からの consumer 接続(同一 ID/Revision で query)。
- #103: PC-C6 cross-domain acceptance での Oracle 利用。
- provider 抽象(Control Mapping 等)が必要になった時点で
  `RelationProvider` を別途設計。
