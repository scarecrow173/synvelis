---
id: "SPEC-106SF03Sz2aBc3dEf4gHi5jKl6"
type: "spec"
status: "approved"
title: "SF-03 slice2 — RoutingFault 18分類完備・Last-good/Retry 高度化・reconciliation"
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
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
    - "docs/specs/sf-01-semantic-routing-model-bus-send-chain-stable-ports.md"
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sf-03-semantic-lowering-source-map-activation-boundary.md"
    - "docs/designs/sf-03-slice2-fault-reconcile-design.md"
  refines:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/106"
  derived-by:
    - "docs/designs/sf-03-slice2-fault-reconcile-design.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice2-fault-reconcile.md"
  references:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
---

# SF-03 slice2 — RoutingFault 18分類完備・Last-good/Retry 高度化・reconciliation

## 目的

Issue #106 の SF-03 を slice 分割して進める本 dispatch は **slice 2 のみ** を対象とする。
slice 1（PR #191）で確立した semantic lowering・`RoutingSourceMap`・
`RoutingBuildKey`・`Configured→Prepared→Published→Active` receipt・
last-good 維持・stale publish 拒否を土台に、以下を完備する。

1. **`RoutingFault` 分類の完備** — Issue #106 §5 が列挙する 18 区分を
   `synvelis_project::routing_build::RoutingFault` に 1:1 で対応させ、
   全 fault 産出経路を決定的に分類する。重複・曖昧な分類は作らない。
2. **Last-good 維持の高度化** — publish/compile 失敗・stale・
   queue full・generation 不一致・順序違反 publish の各経路で
   `last_good_generation`（最後に RT 観測された Active 世代）が
   正しく保持されることをテストで確立する。Project revision の
   巻き戻しは行わない slice 1 契約を維持する。
3. **reconciliation** — committed snapshot との再照合で stale
   Project CAS・capability/engine/source-format generation・
   device/runtime epoch・generation/source-map identity の不一致を
   検出し、旧 plan を維持したまま fault を記録する経路を整備する。

## 前提（slice 1 からの継承契約）

- `RoutingBuildKey` は project ID/revision・routing fingerprint・
  capability generation・engine/source-format generation・lowering
  policy version・build mode・device clock epoch・runtime instance を
  含む。`semantic_eq` は project revision を除く。
- `Published` は非 RT 証拠。`Active` は RT が新世代を非零 frame で
  観測した場合のみ。`last_good_generation` は最後に RT 観測された
  Active 世代であり、単なる最新 publish 世代ではない。
- runtime `NodeId`/`PortId`/`ConnectionId` は graph-local であり
  Project に永続化しない。世代スコープの `RoutingSourceMap` のみが
  公開する。
- `ProcessorBinding::Unresolved`/`PortBinding::Unresolved` は明示的な
  永続 semantic 状態であり、lowering は fail closed する。
- publish は live committed project を直前に再検証する。
- 既存 `HeadlessDriver`/`EngineStub`/`lower()`/`lower_with_inserts`
  は変更しない。

## 要件

### R1. RoutingFault 18 分類の完備

`RoutingFault` を Issue #106 §5 の 18 区分に 1:1 対応させる。
slice 1 の細分化 variant（`StaleProjectRevision`/`StaleCapability`/
`ChannelMismatch`/`UnsupportedMapping`/`ValidationFailed`/
`CompileFailed`/`MissingProcessorInstance`/`EngineStopped`/
`TemporaryIdMismatch`/`GenerationMismatch`/`PublishChannelFull`/
`StaleEvent`）は対応する 18 区分へ統合・改名し、曖昧な二重分類を
残さない。variant は `Copy`・fieldless を維持する。

| Issue 区分 | variant | 主な mint 経路 |
| --- | --- | --- |
| StaleProjectSnapshot | `StaleProjectSnapshot` | prepare/publish の expected revision・project ID 不一致 |
| SpeculativeMismatch | `SpeculativeMismatch` | publish 時に live project の routing fingerprint が prepared key と不一致（同一 revision での内容差異＝投機的 plan） |
| PluginCapabilityChanged | `PluginCapabilityChanged` | publish 時に `port_capability_generation` が drift |
| MissingSource | `MissingSource` | lowering の `MissingSource` skip・comp `MissingSource` reason を fault 昇格（後述 R4） |
| PluginUnavailable | `PluginUnavailable` | chain processor の instance 未供給・compile `MissingProcessor` |
| PortUnresolved | `PortUnresolved` | semantic port 参照が port entity に解決不能（dangling `SemanticPortId`） |
| UnsupportedFormatOrTimeMap | `UnsupportedFormatOrTimeMap` | port domain/channel 不一致・`RateMismatch`/`UnresolvableAnchor`/`UnsupportedMapping` skip・engine/source-format generation drift |
| InvalidRouting | `InvalidRouting` | semantic 構造不正・非 cycle validation 失敗・clip 数不整合 |
| Cycle | `Cycle` | graph validator の `ZeroDelayCycle`/`DelayedCycleUnsupported` |
| CompileResourceExceeded | `CompileResourceExceeded` | lowering budget 超過・compile slot/delay/processor overflow・`max_frames` 未満 budget |
| PublishQueueFull | `PublishQueueFull` | `GraphRuntime::publish` の command/event channel 満杯 |
| ActivationNotObserved | `ActivationNotObserved` | publish 済み plan が RT frame 処理後も観測世代と不一致のまま（reconcile で検出） |
| RetireOverflow | `RetireOverflow` | `SharedStatus` の retire/plan-retire overflow counter 前進（reconcile で検出） |
| DeviceEpochChanged | `DeviceEpochChanged` | request の runtime instance/device clock epoch が engine と不一致 |
| Superseded | `Superseded` | 既 publish 世代以下の prepared plan の publish（順序違反） |
| BadSourceMap | `BadSourceMap` | publish 時の source map/build key/generation 内部不整合、reconcile での map identity drift |
| EventStale | `EventStale` | `SharedStatus` の stale event/command counter 前進（reconcile で検出） |
| OfflineCapabilityMismatch | `OfflineCapabilityMismatch` | `BuildMode::Offline` で chain processor instance が `OfflineRenderSupport::Verified` を報告しない |

- 全ての fault 産出経路は上表のいずれか 1 variant に決定的に写像する。
- `RoutingActivationState::Blocked`/`Failed` は receipt に
  `last_good_generation` を伴って記録される。
- 未分類の fallback を新設しない。未知の上流 error は最も近い
  明示区分（例: compile の非 resource/plugin 系拒否 → `InvalidRouting`）
  に写像し、根拠を設計文書に記録する。

### R2. Last-good 維持の高度化

- 失敗した prepare/compile/publish/reconcile はいかなる場合も
  last-good（最後に RT 観測された Active 世代）を上書きしない。
- `last_good_generation` は `observe_activation` が RT 観測世代と
  pending 世代の一致を非零 frame で確認した時点でのみ前進する。
- 順序違反 publish（既 publish 世代以下の generation を持つ
  prepared plan）は `Superseded` で拒否し、旧 plan を維持する。
- publish 成功時に旧 pending が未活性化のまま置き換わる場合、
  receipt は新 pending の `Published` を記録する（旧 pending の
  未活性化は last-good を後退させない）。
- receipt の `frames_observed` は Active 遷移後も活性化時の frame 数を
  保持する（pending クリアで 0 に戻らない）。

### R3. Retry 契約

- retry は常に新しい committed snapshot・新しい `RoutingBuildKey`・
  新しい `RoutingSourceMap`・新しい generation を持つ fresh
  `prepare` から開始する。失敗した `PreparedRouting` の再利用、
  旧 generation・旧 runtime ID・無効化された source map の再利用は
  行わない。
- 失敗した publish は `PreparedRouting` を消費するため、retry は
  必ず再 prepare を要する（既存契約の明文化）。
- stale target（revision/capability/epoch 不一致）への retry は
  prepare/publish/reconcile の各段で typed fault として拒否される。

### R4. Skipped playback の fault 昇格

`plan_playbacks` が返す `skipped` のうち、意図的な無音
（`Muted`/`SupersededByComp`/`CompSilence`）以外は「再生されるべき
内容が下せなかった」ことを意味し、fake silence/bypass にしないため
prepare を fault で失敗させる。

- `MissingSource`（clip/comp）→ `MissingSource`
- `RateMismatch`/`UnresolvableAnchor`/`UnsupportedMapping`/
  comp `RateMismatch`/`Unresolvable`/`UnsupportedMapping` →
  `UnsupportedFormatOrTimeMap`
- `CompProjection` → `InvalidRouting`
- Realtime/Offline 双方に適用する。

### R5. reconciliation

`RoutingEngine::reconcile` を追加し、最後に publish された plan の
`RoutingBuildKey`/`RoutingSourceMap` と live committed snapshot・
RT counter を再照合する。

- 照合対象: project ID/revision（CAS）・routing fingerprint・
  `port_capability_generation`・engine/source-format generation・
  device clock epoch・runtime instance・source map/generation
  identity・RT stale/retire counter・pending activation の観測状況。
- 全ての不一致側面を `Vec<RoutingReconcileMismatch>` として記録し、
  優先順位に従い 1 つの `RoutingFault` を receipt（`Blocked`）に
  記録する。旧 plan は維持される。
- 不一致ゼロの場合は receipt を変更せず、drift なしの report を返す。
- publish 前・retry 前の呼び出しで「live state が変わったのに
  publish count 0 の投機的 plan」を確実に拒否できる。
- fault 優先順位（先勝ち）: `BadSourceMap` → `StaleProjectSnapshot`
  → `SpeculativeMismatch` → `PluginCapabilityChanged` →
  `UnsupportedFormatOrTimeMap`（generation 系）→ `DeviceEpochChanged`
  → `RetireOverflow` → `EventStale` → `ActivationNotObserved`。
  全順序は設計文書に記す。

### R6. Offline capability

`BuildMode::Offline` の prepare では、lowering に実際に使われる
chain processor instance が `OfflineRenderSupport::Verified` を
報告することを要求する。`Unknown`/`Declared` は verified exact
offline support とみなさず `OfflineCapabilityMismatch` で失敗する。
engine 内蔵 node（clip/strip/send gain）の offline 同等性は
synvelis-render 側 slice の責務であり、本 slice では外部供給
instance のみを gate する。

## 受入条件

- [ ] `RoutingFault` が Issue #106 §5 の 18 区分に 1:1 対応し、
      全 mint 経路が決定的（設計文書の対応表と一致）。
- [ ] `sf03_l1.rs` 拡張または新規 test file で以下を green で検証:
      - 全 fault 分類の代表的 mint 経路
      - last-good が compile/publish/stale/queue-full/順序違反/
        generation 不一致/reconcile drift の各経路で保持される
      - retry が fresh build で成功し stale target を拒否する
      - reconcile が CAS/fingerprint/capability/epoch/generation/
        counter/pending の各 drift を検出し旧 plan を維持する
      - offline mode の capability gate
      - 投機的 plan（publish count 0・live state 差異）の拒否
- [ ] `mise run check`/`lint`/`test`/`fmt-check`/`test-rt`/
      `check-headless-deps` が全て exit 0。
- [ ] spec → design → plan → task → test-spec の文書チェーンが
      `docs/` に存在し、doc-status audit の blocker がない。

## 範囲外（後続 slice / 別 Issue）

- SF-ACT-05〜09: rapid R1/R2/R3・superseded compile の実 plugin host
  接続、queue-full/partial stale command の実機検証、RT latency/
  underrun/device epoch restart の実デバイス検証、stale parameter/
  event dispatch、retirement/fault counter の実機統合、NoRebuild
  fingerprint 最適化。
- 実 plugin host 接続・GUI 表示・`synvelis-app`/`session.rs`/
  `EngineStub`/`lower()`/`lower_with_inserts` の変更。
- Project revision の巻き戻し・rollback（post-commit activation
  failure でも revision は進んだまま、という slice 1 契約の維持）。
