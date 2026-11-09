---
id: "SPEC-034UoZx9FMfQ1OvSAcWCoN"
type: "spec"
status: "approved"
title: "PC-C2 双方向 Control Relation Index/Projection — Automation/Assignment/Depth Driver"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/99"
    - "https://github.com/scarecrow173/synvelis/issues/6"
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
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/103"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/6"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/100"
    - "https://github.com/scarecrow173/synvelis/issues/101"
  defers: []
  deferred-by: []
---

# PC-C2 双方向 Control Relation Index/Projection — Automation/Assignment/Depth Driver

## What Should Be Built

Issue #99(P0・Gate C 受入の構成要素)。`Project::control` 上の
**revision-tagged 派生双方向 index**を構築する read-only
projection。`AutomationLane → ParameterTarget`(AutomationDrives)、
`ModulationSource → ModulationAssignment → ParameterTarget`
(ModulationDrives)、`ParameterTarget(depth driver) → ModulationAssignment`
(DepthControlledBy)の各辺について、Source→Target と Target→Source の
両方向を同じ `ControlModel` 正本から解決する。

現行 `ControlSession::signal_flow_view(project, target)` は target への
incoming assignment の一部射影にすぎず、Automation Lane / Depth Driver を
含む双方向探索と一貫した状態証拠を持つ index は存在しない。本 spec は
**二重 Control 正本を作らない** read-only 派生 index のみを対象とし、
UI 投影(#102)・RT 適用(#100/#101)・永続 schema 変更は行わない。

## Why Now

Gate C の双方向 Relation 受入(REL-C2-01〜05)と PC-C6 #103 の
PC-C2-REL01〜REL07 Oracle が、本 index の安定した ID/Revision/状態
契約を前提とする。PC-C5 UI(#102)が Arrangement/Inspector/Signal Flow の
3 view へ同一 ID/Revision の関係を供給するには、project 層に
決定的・有界・revision-tagged な問い合わせ面が必要。

## Users and Value

- PC-C5 #102: Automation Lane / Context Inspector / Signal Flow Control
  Layer が消費する双方向 Relation 投影の唯一の供給元。
- PC-C6 #103: REL-C2-01〜05 / PC-C2-REL01〜REL07 の独立 Oracle が照合
  する headless 契約。
- 将来の Control Mapping(MIDI CC 等の外部入力)provider が乗るための
  拡張点(本 slice では実装しない)。

## Scope

### In Scope

- `synvelis-project` に `relation` module を追加し、`Project::control`
  から構築する revision-tagged 派生 index と bounded query API を提供
  する。
- 辺種: `AutomationDrives` / `ModulationDrives` / `DepthControlledBy`。
  node は `ParameterTarget` / `AutomationLane` / `ModulationSource` /
  `ModulationAssignment` の 4 型の typed stable ID。
- `RelationState`(Enabled / Disabled / Unresolved / Unsupported)と
  `RelationCoverage`(Complete / Truncated / Stale / Unavailable)による
  一貫した状態証拠。
- Transitive traversal: bounded depth/edge、visited node/edge、
  決定的順序。循環・無効参照は `Unresolved` として空結果と区別する。
- 大規模 graph での bounded paging/truncation、O(N) index 構築、
  query budget の計測手段(fixture 規模での実測を含む)。
- read-only 不変性: index 構築・query が `Project` / DSP 評価 /
  Automation Series / Undo stack を一切変更しないことの test 担保。

### Out of Scope

- UI への実投影接続(Arrangement/Inspector/Signal Flow の描画・選択・
  Focus)— #102 PC-C5 の Owner 範囲。
- Control Trace / 設定済・適用済・観測済の状態機械 — #100 PC-C3。
- Plugin Parameter Binding / Capability Generation — #101 PC-C4。
- MIDI CC 等の Control Mapping provider(将来の optional
  RelationProvider。本 slice では fake binding を既知として表示しない)。
- Ruviform 汎用 Graph/RT/Plugin Host の再実装、`Project::control` の
  第二正本・永続 schema 変更。
- 実 OS GUI / 実 Plugin / RT 実機 Gate(#103/#114 の別証拠)。

## Requirements

### REL-C2-R1: 双方向 index と typed edge

- index は `Project::control` のみから構築する派生物で、構築時の
  `ProjectRevision` を保持する。`Project` 本体・snapshot schema・
  `ProjectController` の revision/undo 意味論を変更しない。
- node/edge の endpoint は compile-time に区別された typed stable ID
  (`ParameterTargetId` / `AutomationLaneId` / `ModulationSourceId` /
  `ModulationAssignmentId`)を保持し、同一 source+target の複数
  assignment は assignment ID により**別辺**として区別する
  (assignment id ≠ edge id: 辺は kind+両端+assignment で識別する)。
- 辺の向き: `AutomationLane → ParameterTarget`、
  `ModulationSource → ModulationAssignment`、
  `ModulationAssignment → ParameterTarget`、
  `ParameterTarget(depth_source) → ModulationAssignment`。
  `Incoming` / `Outgoing` / `Both` の各 direction で anchor からの
  transitive 訪問を提供する。

### REL-C2-R2: 状態証拠

- `Enabled`: 参照が解決でき、assignment が enabled(automation lane は
  常時 Enabled)。
- `Disabled`: `enabled == false` の assignment に由来する辺。
  `include_disabled == false` の query では結果にも traversal にも
  現れない(寄与なしと一致)。
- `Unresolved`: endpoint 参照が index 内で解決できない辺、および
  enabled 辺のみで構成される依存 cycle に参加する辺。空結果とは
  別に報告する。cycle 検出は `ControlGraph::compile` の zero-delay
  cycle 契約と同じ enabled-only 意味論に従う。
- `Unsupported`: 将来の provider 種別のために予約。本 slice の index
  は生成しない。

### REL-C2-R3: Revision tag と stale 応答

- `RelationQuery.expected_revision` が index の構築 revision と一致
  しない場合、reply は `coverage = Stale` で辺を返さない
  (project_revision には index 側の世代を返す)。
- anchor が構築 revision 時点で存在しない場合は `coverage =
  Unavailable` で辺を返さない。anchor は存在するが関係が無い場合は
  `Complete` + 空 relations。
- index は新 revision ごとに再構築される派生物であり、保持側が
  旧世代の reply を現在値として扱わないことを coverage で強制する。

### REL-C2-R4: 有界 traversal と安全拒否

- traversal は `max_depth`(anchor からの hop 数)と `max_edges`
  (返却辺数)で有界化し、超過時は `coverage = Truncated`。
  visited node/edge により循環・菱形での再訪を防ぎ、結果順序は
  決定的(BTreeMap 由来の安定順)。
- depth_source cycle・dangling 参照を含む project でも構築・query は
  panic/hang せず、該当辺を `Unresolved` として安全に扱う
  (`ControlGraph::compile` の `ControlCycle` 拒否契約は変更しない)。
- index 構築は control model サイズに対し O(N)。query の最悪仕事量は
  O(V+E) で、返却量は `max_edges` で有界。fixture 規模での構築・
  query 時間を test で実測記録する。巨大 graph の性能実測は
  fixture 規模に限定し、未実施分は NotRun として記録する。

### REL-C2-R5: read-only 不変性

- index 構築・全 query kind は `&Project` のみを受け取り、project・
  DSP 評価結果・Automation series・Undo/Redo stack を変更しない。
  test で `Project` の同一性・`evaluate_control`/`ControlGraph`
  結果・controller の undo 動作が index 利用の前後で不変であることを
  担保する。

## Acceptance Criteria

- [ ] `REL-C2-01`: LFO/Macro/StepSeq source → 複数 target、および
  target → 全 source / automation lane の双方向検索が同一 index で
  解決できる。
- [ ] `REL-C2-02`: 同一 source/target の複数 assignment・depth driver
  が別辺として区別され、Disabled 辺の表示(`include_disabled`)と
  非寄与(既定 query・評価経路での非寄与)を確認できる。
- [ ] `REL-C2-03`: ProjectRevision の delta、assignment 削除/target
  消失、旧世代 index への stale query が Stale/Unresolved/差分結果
  として観測できる。
- [ ] `REL-C2-04`: 大規模 graph で bounded paging/truncation、
  cycle 投入・不正参照の安全拒否、O(N) index と query budget の
  計測(fixture 実測 + 巨大 graph は NotRun 記録)。
- [ ] `REL-C2-05`: read-only 不変性を test で担保。実 UI 投影の接続は
  #102 のスコープとして NotRun。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` 全 green。
- [ ] doc chain 完走、docs/impl に impl record。

## Deferred Design Concerns

- Control Mapping(MIDI CC 外部入力)の RelationProvider — 将来の
  optional 拡張。本 slice は fake binding を出さない。
  | re-engagement: Control Mapping Issue 着手時 | risk if ignored:
  provider 抽象の手戻り(現状は影響なし)
- 巨大 graph(数万 node 規模)の性能実測 — fixture 規模の実測で
  budget 機構を検証済みとし、実測は #103 Oracle の大規模 fixture で
  補完する。

## Sources and Relations

- Issue #99(本 spec の直接の source): API 契約・受入条件 REL-C2-01〜05。
- Issue #6(Design Owner): Automation / Modulation の意味論。
- PC-A spec(#40 Control Model)・PC-B spec(#43 Dispatch)に依存。
- PC-C1 spec(#98 Recording Transaction)とは並行可能な別 slice。
- 下流 consumer: #102(PC-C5 UI 投影)・#103(PC-C6 Oracle)。
