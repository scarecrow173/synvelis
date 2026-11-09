---
id: "ADR-034Ux91pIhFgxbsb0xs3y9"
type: "adr"
status: "approved"
title: "SF-03 世代スコープ証拠の authoritative owner — RoutingEngine receipt + SourceMap に一元化"
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
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
    - "docs/adr/sf-03-semantic-lowering-source-map-activation-boundary.md"
    - "docs/specs/pc-c3-control-trace-projection.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/discovery/sf-03-map-current-state-inventory.md"
  derived-by:
    - "docs/designs/sf-03-map-semantic-runtime-mapping-design.md"
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/106"
    - "https://github.com/scarecrow173/synvelis/issues/100"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# SF-03 世代スコープ証拠の authoritative owner — RoutingEngine receipt + SourceMap に一元化

## Context and Problem Statement

Issue #106 の semantic→Ruviform runtime 写像では、「どの Project
Revision の routing が、どの plan generation として publish され、
RT がどの generation を active として観測したか」という証拠が、
probe（SF-04）・control dispatch（PC-B/C3）・PCM render・
UI（SF-05）・Undo/Save の全 consumer にまたがって参照される。

各 consumer が独自に「active」や「現在の graph」を推論すると、
Publish 成功と RT 駆動の混同・stale generation への誤配送・
旧 Plan への誤表示が構造的に発生する。slice1〜5 の実装は
`RoutingEngine` の activation ledger・`RoutingActivationReceipt`・
`RoutingSourceMap` に証拠を集約済みだが、TRACE-C3/SF-04/SF-05
統合後の consumer 横断契約を固定する決定が文書化されていない。

## Decision Drivers

* Publish 成功（非 RT）と RT active 観測を混同しない — 停止中・
  frames=0・経過時間での誤成功/誤失敗を防ぐ。
* generation-scoped runtime ID（`NodeId`/`PortId`/`ProbeId`/
  `PluginHandle`）を Project に永続化せず、失効後の誤 target
  配送を構造的に 0 にする。
* consumer ごとの独自 active 判定・独自 SourceMap 解釈を許さず、
  単一 authoritative owner に証拠を集約する。
* TRACE-C3 `ControlTraceKey`・SF-04 probe binding・SF-05 evidence
  snapshot が同一世代名前空間を共有できること。

## Considered Options

* **Option 1**: `RoutingEngine` の activation ledger +
  `RoutingActivationReceipt` + publish 済み `RoutingSourceMap`
  を唯一の authoritative owner とし、consumer は generation 照合で
  stale 証拠を拒否する。
* **Option 2**: consumer（workspace/app/render）側にそれぞれ
  「現在 active な plan」の投影を持たせる。
* **Option 3**: Project 側に runtime ID・active generation を
  永続化して consumer が直接参照する。

## Decision Outcome

Chosen option: "**Option 1**", because publish/active/last-good の
証拠は RT 観測という単一の物理的事実に基づくものであり、
`RoutingEngine` だけが `GraphRuntime`/`RtRuntime`/`SharedStatus`
へ直接アクセスできる。consumer 側の投影（Option 2）は証拠の
二重正本を生み、Project 永続化（Option 3）は runtime ID の
腐敗（plugin reload/plan swap/port reorder）を招く — slice1 ADR
が既に否定した設計である。

### Consequences

* Good, because `Configured/Prepared/Published/Active`・
  `last_good_generation`・`observed_active_generation` の証拠が
  `RoutingActivationReceipt` 一箇所に集約され、consumer は
  generation 照合のみで stale を拒否できる。
* Good, because TRACE-C3 `ControlTraceKey.graph_generation`・
  SF-04 `SemanticProbeBinding`・SF-05 `SignalFlowRuntimeEvidence`
  が同じ generation 名前空間を参照し、「同じ Revision を指す」
  契約が構造的に保証される。
* Bad, because consumer は engine の receipt/source_map を
  経由する read 経路を必要とし、直接 runtime 状態を覗けない —
  workspace/app 層の passthrough 実装が前提となる（slice5・
  sf-04-s2 で整備済み）。

### Confirmation

`RoutingActivationReceipt` の `state`/`published_generation`/
`observed_active_generation`/`last_good_generation` が唯一の
activation 証拠であり、consumer 側に第二の active 判定が存在
しないことを、文書レビューと headless fixture
（`sf03_l*.rs`・`sf04_*.rs`・`sf05`・pc-c6 oracle）で確認する。

## Implementation Plan

* Affected paths: 本 slice は docs のみ。設計対象の実装 path は
  `crates/synvelis-project/src/routing_build.rs`（契約型）・
  `crates/synvelis-player/src/routing_engine.rs`（ledger/receipt/
  reconcile）・`crates/synvelis-player/src/routing_lower.rs`
  （SourceMap 生成）・`crates/synvelis-project/src/trace.rs`
  （ControlTraceKey 世代照合）・`crates/synvelis-project/src/
  probe.rs`+`probe_trace.rs`（probe 世代照合）・
  `crates/synvelis-workspace`（EnginePort/ControlSession/
  signal_flow_ui evidence）・`crates/synvelis-app`
  （SessionRoutingEngine evidence snapshot）。
* Patterns to follow: slice1 ADR
  `sf-03-semantic-lowering-source-map-activation-boundary.md`、
  slice4 `take_processor_control` の世代スコープ払い出し、
  SF-04 slice2 の `StaleGeneration` fail-closed reject、
  TRACE-C3 `ObservationFreshness` の stale 分類。
* Patterns to avoid: consumer 側での独自 active 判定・
  `EngineStub` を activation 証拠とみなすこと・Project への
  runtime ID/`ProbeId`/`PluginHandle` 永続化・publish 成功を
  `Active` と表示すること・stale record の trace 記録。
* Dependencies or configuration: なし（docs のみ）。Ruviform
  generic API（typed graph/compiler/GraphRuntime/RtRuntime/
  probe broker/plugin host）は pin `0f5b5737` の既存契約を利用。

## Verification

* [ ] `docs/designs/sf-03-map-semantic-runtime-mapping-design.md`
  が本決定を前提に責務境界・失敗不変条件・consumer 契約を記述
  している。
* [ ] `docs/test-specs/sf-03-map-failure-invariant-verification.md`
  が世代証拠分離・last-good・stale reject・Pending 明示の
  検証 fixture を列挙している。
* [ ] 既存実装（`RoutingEngine` receipt・`resolve_probe_point`・
  `take_processor_control`・`ControlTraceKey` 世代照合）が本決定
  と矛盾しないことをコードレビューで確認する。

## Pros and Cons of the Options

### Option 1

* Good, because 証拠の authoritative owner が一意で、consumer は
  generation 照合のみを実装すればよい。
* Good, because RT 観測という単一の物理的事実に基づく証拠は
  偽造・推論の余地がない。
* Bad, because engine receipt への read 経路（passthrough）を
  各層が必要とする。

### Option 2

* Good, because consumer が engine を介さず投影を読める。
* Bad, because consumer ごとに active 判定が分岐し、publish≠active
  の誤表示・stale 誤配送を各所で再発明する危険がある。

### Option 3

* Good, because consumer が Project snapshot だけで runtime 参照を
  得られる。
* Bad, because runtime ID は plan/plugin lifecycle に紐づく
  揮発値であり、Project 永続化は静かな腐敗を生む（slice1 ADR
  で否定済み）。

## More Information

- Issue #106（SF-03 詳細契約 v1・受入条件）、#100（TRACE-C3）。
- `docs/specs/sf-03-map-semantic-runtime-contract.md`（本決定の
  要求元）、`docs/discovery/sf-03-map-current-state-inventory.md`
  （棚卸し根拠）。
- 関連 ADR: `docs/adr/sf-03-semantic-lowering-source-map-activation-boundary.md`
  （SourceMap/BuildKey 境界と Activation 証拠分離 — 本 ADR は
  その consumer 横断への拡張）。
