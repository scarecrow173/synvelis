---
id: "ADR-106SF03Cd2eFg3hIj4kLm5nOp"
type: "adr"
status: "approved"
title: "SF-03 slice1 — Runtime ID は SourceMap 経由のみ・Activation は RT 観測証拠で分離"
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
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/designs/sf-03-slice1-lowering-sourcemap-activation-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
  derived-by:
    - "docs/designs/sf-03-slice1-lowering-sourcemap-activation-design.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice1-lowering-activation.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/106"
---

# ADR: SF-03 slice1 — SourceMap/BuildKey 境界と Activation 証拠分離

## Context

Issue #106 は Semantic `RoutingModel` を Ruviform typed graph/runtime
へ lowering し、publish 成功と RT activation を混同しない証拠系を
要求する。`GraphBuilder` 発行 ID は graph-local であり、Project に
runtime ID を永続化すると plugin reload/plan swap/port reorder で
静かに腐る。

## Decision

1. **契約型は `synvelis-project::routing_build` に置く。**
   `RoutingBuildKey`/`RoutingCompileRequest`/`MappingResolution`/
   `RoutingSourceMap`/`RoutingCompatibilityReport`/`RoutingFault`/
   `RoutingActivationState`/`RoutingActivationReceipt` は persisted
   semantic ID と ruviform-graph ID の写像・証拠の型であり、audio
   runtime への依存を持たない。`PreparedRouting` は `CompiledPlan`
   (processor box 所有)を伴うため `synvelis-player` 側に置く。
2. **Lowering は `synvelis-player` の追加経路。** 既存
   `lower()`/`lower_with_inserts`(clip→master direct)は変更せず、
   semantic snapshot から `GraphSpec`+`RoutingSourceMap`+node role
   表を生成する `lower_semantic` を新設する。clip/comp の source
   window 解決は既存 lowering と同一の計画関数を共有する。
3. **Strip = fader/pan 段 + insert chain 段の分解。** semantic main
   入力 port は pre-chain 位置(chain 先頭の入力、無ければ strip
   fader の入力)へ、main 出力 port は post-fader 出力へ写像する。
   pre-fader send tap は strip node の第 2 出力(dry)port を使う。
   mute/solo は `audible_strips` と同一 authority で edge prune に
   変換する。
4. **Activation は engine 側 ledger。** `GraphRuntime::publish` の
   成功は `Published` に留め、`RtRuntime::process` が新
   `active_generation` を観測した時点でのみ `Active` receipt を
   発行する。`last_good_generation` は最後に Active 観測された
   generation を保持する。
5. **Stale 防止は publish 直前の再照合。** `PreparedRouting` の
   build key と live project から再計算した key を比較し、
   project revision まで一致しない場合は semantic 等価
   (NoRebuild 証拠)か reject のいずれかとし、speculative plan を
   publish しない。

## Consequences

- Project snapshot 形式・semantic 契約は不変。追加は projection/証拠
  型のみ。
- chain processor は plugin instance 未接続のため compile factory が
  `PluginUnavailable` で fail-closed する(偽の bypass も fake
  silence も作らない)。
- `RoutingFault` は本 slice で発生し得る variant の部分集合であり、
  18 分類完備は後続 slice。
