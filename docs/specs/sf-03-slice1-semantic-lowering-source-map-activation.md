---
id: "SPEC-106SF03Ab1cDe2fGh3iJk4lMn"
type: "spec"
status: "approved"
title: "SF-03 slice1 — Semantic Routing → Ruviform Typed Graph Lowering・SourceMap・Activation receipt"
created: "2026-11-08"
updated: "2026-11-08"
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
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sf-03-semantic-lowering-source-map-activation-boundary.md"
    - "docs/designs/sf-03-slice1-lowering-sourcemap-activation-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  derived-by:
    - "docs/designs/sf-03-slice1-lowering-sourcemap-activation-design.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice1-lowering-activation.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/104"
    - "https://github.com/scarecrow173/synvelis/issues/105"
---

# SF-03 slice1 仕様 — Semantic→Typed Graph Lowering・SourceMap・Activation receipt

## 目的

SF-01/SF-02 で確立した Semantic `RoutingModel` 正本を、Ruviform typed
graph/runtime へ lowering する実行経路を追加する。Project に永続化
されるのは安定 Semantic ID のみで、`GraphBuilder` 発行の
`NodeId`/`PortId`/`ConnectionId` は generation-scoped な
`RoutingSourceMap` を通じてのみ解決され、Project へ逆流しない。

## 範囲(slice 1)

- `RoutingBuildKey` / `RoutingCompileRequest` / `MappingResolution` /
  `PreparedRouting` / `RoutingSourceMap` の契約型
- Semantic `RoutingModel` → `GraphSpec` lowering(clip/comp playback、
  strip chain、send tap、device sink 境界)
- `Configured → Prepared → Published → Active` の activation 状態機械と
  `RoutingActivationReceipt`(RT 観測証拠と non-RT publish 成功の分離)
- 失敗時の Last-good 維持と stale build の publish 拒否
- SF-ACT-01〜04 の slice 受入(Golden PCM・順序・stale 0 publish・
  activation 証拠分離)

## 範囲外(後続 slice / 別 Issue)

- `RoutingFault` 18 分類の完備、Last-good/Retry 高度化、reconciliation
- SF-ACT-05〜09(Plugin host 接続、GUI 表示、NoRebuild 最適化の完全実装)
- 実 plugin instance の host 接続(chain processor は注入 boundary)
- `crates/synvelis-app` / `ui` の変更、`session.rs` `EngineStub` の変更
- 既存 `lower()`/`lower_with_inserts` の置き換え(追加経路として共存)

## 要件

### SF-L01 Lowering

- 有効な `RoutingConnection` のみ edge 化する。disabled は edge を作らない。
- Strip ごとに main 入力(外部 edge + clip/comp playback の fan-in)→
  insert chain 順 → fader/pan → main 出力の信号順を保持する。
- `SendTap::PreFader`/`PostFader` を lowering 時に別 PortRef へ解決する。
- `ExplicitChannelMap` は推論 upmix/downmix せず明示 map として下ろす。
- mute/solo は `audible_strips` の audibility 意味論で edge prune に
  変換する(モデル監査と同一 authority)。
- 未解決の source/port/plugin binding は deterministic な失敗または
  `MappingResolution` の `Unavailable`/`Unsupported` であり、
  fake silence や空成功 mapping にしない。

### SF-L02 SourceMap / BuildKey

- `RoutingSourceMap` は `SemanticPortId`/`RoutingConnectionId`/
  semantic node(`StripId`/`BusId`/`ProcessorId`)→ runtime ID の
  generation-scoped 写像と、project revision・build key 参照を持つ。
- `RoutingBuildKey` は project id/revision、routing fingerprint、
  capability generation、engine/source format generation、lowering
  policy version、build mode、device clock epoch、runtime instance を
  束ね、stale 判定の同値比較を提供する。
- lowering は決定的順序(BTreeMap/計画順)を維持し、同一 snapshot から
  同一 spec 構造を再現する。

### SF-L03 Activation receipt

- `Configured → Prepared → Published → Active` を別証拠として記録する。
  non-RT `GraphRuntime::publish` 成功は `Published` までであり、
  `Active` は RT callback が新 generation を観測した後にのみ遷移する。
- `frames == 0`・engine 停止・publish のみでは `Active` を mint しない。
- publish/compile 失敗・stale・queue full・generation 不一致では旧
  plan が Last-good として維持され、receipt は fault と
  `last_good_generation` を保持する。
- post-commit の activation 失敗で Project revision を巻き戻さない。
- `EngineStub` 等の非 RT stub を activation 証拠と混同しない。

### SF-L04 受入(slice)

- SF-ACT-01: legacy direct→master と semantic lowering の Golden PCM
  一致(単一 track での加算順序を含む)、SourceMap と Project revision
  の一致、Missing Source の明示。
- SF-ACT-02: Pre/Post tap・sidechain・mute/solo・bus/return・processor
  順序が Realtime/Offline で同一意味へ lowering される。
- SF-ACT-03: committed snapshot 再照合で stale Project CAS・capability
  drift・temporary ID 不一致は speculative plan の publish 0 を保証。
- SF-ACT-04: 証拠分離(停止/0 frame/non-RT publish/遅延 RT active)と
  Last-good 維持、stub false positive 無し。

## 非機能

- Ruviform の generic validator/compiler/RT scheduler/plugin host を
  重複実装しない。境界は `GraphBuilder`/`GraphProject`/`Compiler`/
  `GraphRuntime`/`RtRuntime` の既存 API のみを使う。
- 検証は `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-headless-deps` で行う。
