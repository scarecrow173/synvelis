---
id: "SPEC-034UlYgSZ589NRdTrtO4Vu"
type: "spec"
status: "approved"
title: "SF-04 slice1 — Semantic Probe Adapter 基盤（project/player 層 + ruviform pin bump）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/107"
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
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/sf-03-slice4-act-acceptance-norebuild.md"
    - "docs/specs/sf-03-slice5-session-engine-connect.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/sf-03-slice5-session-engine-connect.md"
  defers: []
  deferred-by: []
---

# SF-04 slice1 — Semantic Probe Adapter 基盤（project/player 層 + ruviform pin bump）

## What Should Be Built

Issue #107（SF-04 Semantic Probe Adapter）のうち、headless な project/player 層の
Semantic Probe Adapter 基盤を構築する。具体的には:

- ruviform pin を `cc7d5296` から `#308`（Graph Swap Probe Capture/Rebind
  safety: `RebindReceipt` / `CapturePolicy`）と `#309`（Frame/Clock/Epoch/
  Latency alignment: `ProbeClockMark` / `ProbeTimeContext` /
  `align_observation_points` / `ObservationCoverage`）を含む `0f5b5737` へ更新する。
- `synvelis-project` に semantic contract 層の型を定義する:
  `SignalBoundary` / `SemanticSignalEntityRef` / `SemanticProbePoint` /
  `ProbeSubscriptionKey` / `ProbeResolveState` / `SemanticProbeBinding` /
  `ProbeCaptureSegment`（永続 recording の `CaptureSegment` とは別概念）等。
  `SemanticProbePoint` → `RoutingSourceMap`/`GraphSpec` による解決関数を
  fail-closed で提供し、物理 tap が存在しない位置は `Unavailable`/`Unsupported`
  を正直に返す（仮想 PCM・ゼロ値代替を生成しない）。
- `synvelis-player` に runtime adapter を定義する: `RoutingSourceMap` 解決 →
  runtime `ProbePoint` への `ProbeBroker` binding、Off/Monitor/Capture の
  requested/queued と RT-installed の区別、generation-guarded stale rejection、
  `RebindReceipt` による capture segment cut（generation 境界を跨ぐ capture を
  silently 継続しない）。`RoutingEngine` への probe 配線のみを行う。
- headless テスト `sf04_*.rs` で、解決成功/Unavailable/Unsupported、
  generation 不一致の stale reject、rebind 境界での segment cut を非 RT 層で検証する。

## Why Now

SF-03（#106）で semantic routing → typed graph lowering・generation-scoped
`RoutingSourceMap`・activation receipt が確立し、Ruviform #185 Probe /
#308 rebind safety / #309 clock alignment の汎用機構が pin 対象 rev に揃った。
SF-04 の残り（UI #108・実測 #109・app/workspace 接続）は本基盤の上に乗るため、
project/player 層の契約と配線を先に固定する。

## Users and Value

- 後続 slice（#108 UI、#109 oracle、app/session 接続）の実装者が、安定した
  semantic probe 契約と generation 安全な binding lifecycle を再利用できる。
- プロジェクト層は永続化される semantic 意図だけを持ち、broker 寿命の
  runtime `ProbeId` を保存しない — 将来の UI/セッション層が同じ契約で
  複数 window/broker を扱える土台になる。

## Scope

### In Scope

- ruviform pin bump（`Cargo.lock` のみ。API 破壊なしを確認済み）。
- `crates/synvelis-project/src/probe.rs`（新規）と `lib.rs` export。
- `crates/synvelis-player/src/probe.rs`（新規）、`routing_engine.rs` への
  probe 配線（deck install・publish 時 rebind・drain 接続）、`lib.rs` export。
- `crates/synvelis-{project,player}/tests/sf04_*.rs`（新規 headless テスト）。
- `docs/` 文書チェーン（spec/design/plan/task/test-spec/impl record）。

### Out of Scope

- `synvelis-app` / `synvelis-workspace` への probe 配線（後続 integration slice）。
- UI/Monitor UX（#108）、実デバイス/実 RT 測定（#109）、複数 window
  ownership・比較診断（SF-PROBE-05/07 の完全消化）。
- ruviform 側の変更。契約不足があれば workaround を作らず `ask` で確認する。
- probe capture PCM の永続化・retention（Recording Writer/Source Blob とは
  別契約。slice 1 は segment メタデータと frame interval の記録まで）。

## Requirements

- Probe の既定 mode は `Off`。Monitor/Capture は DSP・processor bypass・
  routing・raw recording 挙動を変更しない（SF-PROBE-02）。
- `SemanticProbePoint` は安定した semantic entity + `SignalBoundary` +
  action position を表し、永続化してよいのは semantic 意図のみ。
  Ruviform `ProbeId` は broker 寿命の識別子であり Project には保存しない。
- 解決は active `RoutingSourceMap` + published `GraphSpec` に対して行い、
  `MappingResolution` を尊重する。物理 tap/対応する明示 DSP tap が無い位置は
  `Unavailable`/`Unsupported` を返し、PCM を捏造しない（SF-PROBE-01）。
- Requested/Queued と RT-installed/observed を区別する。status 喪失・
  generation 不一致・stale/late acknowledgement は `Pending`/`Unknown`/
  `StaleGeneration` として扱い、ゼロ計測で誤魔化さない（SF-PROBE-03）。
- Graph swap 時の capture は `RebindReceipt` に従い generation 境界で
  segment を切断し、新 generation へ自動継続しない（SF-PROBE-04、
  Ruviform #308 契約に従う）。
- RT callback 経路は noalloc/no-lock/no-I/O/bounded を維持する（SF-PROBE-09）。
  adapter の非 RT 側 bookkeeping は bounded な構造に限定する。

## Acceptance Criteria

- [ ] ruviform pin が `0f5b5737`（#308/#309 含む）に更新され、
  `check-ruviform-rev` が green。
- [ ] `synvelis-project/src/probe.rs` に Issue #107 §2 の semantic contract
  型が定義され、`lib.rs` から export される。
- [ ] `SemanticProbePoint` → runtime `ProbePoint` の解決が
  `RoutingSourceMap`/`GraphSpec` 経由で行われ、Unavailable/Unsupported が
  fail-closed で報告される。
- [ ] Off/Monitor/Capture の lifecycle で requested/queued と RT-installed が
  区別され、generation 不一致の status は stale として拒否される。
- [ ] publish（generation 交代）時に `RebindReceipt` 経由で capture segment が
  切断され、自動再 capture されない。
- [ ] `sf04_*.rs` headless テストが解決成功/Unavailable/Unsupported・
  stale reject・segment cut を検証し、`mise run test` で green。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `check-ruviform-rev` が green。

## Deferred Design Concerns

- app/session 層への probe 配線・複数 window/broker ownership（SF-PROBE-05）
  | 本 slice は project/player headless 層に限定 | #108/#109 および後続
  integration slice | routing/activation 経路が確定してから接続しないと
  session ownership を二重実装する危険。
- UI Monitor/Capture UX・実 RT 計測 | #108/#109 のスコープ | 各 Issue 着手時 |
  headless 契約が未確定のまま UI を作ると作り直しになる。
- probe capture PCM の永続化・比較診断（SF-PROBE-06/07/08） | slice 1 は
  contract + segment metadata のみ | 計測系 slice | 保持/欠損契約を
  Recording Writer と混同しないため先に境界を固定する。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/107>（SF-PROBE-01..09）
- 前提: SF-03 slice1..5（semantic lowering・SourceMap・RoutingEngine・session 接続）
- Ruviform: #184 Typed Graph、#185 Probe、#308 Graph Swap Probe
  Capture/Rebind safety、#309 Frame/Clock/Epoch/Latency alignment、
  #257/#260 Transport/Visualization bounded probe data、#28 pin tracking。
- 本 spec は Issue #107 のうち slice 1（project/player 層 + pin bump）のみを
  対象とし、Issue は Open 継続が前提。
