---
id: "SPEC-034SF02Tx9kLm4nQ7rVw2yZaBc"
type: "spec"
status: "approved"
title: "SF-02 DAW Routing Commands — RoutingTransaction/TempRef・Semantic+Generic Validation・Mute/Solo Signal Gate"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "https://github.com/scarecrow173/synvelis/issues/4"
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
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sf-02-routing-transaction-port-channels-mute-solo-authority.md"
    - "docs/designs/sf-02-routing-transaction-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "docs/adr/sf-02-routing-transaction-port-channels-mute-solo-authority.md"
  derived-by:
    - "docs/designs/sf-02-routing-transaction-design.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sf-02-e2e-routing-transaction.md"
  references:
    - "https://github.com/scarecrow173/ruviform/issues/184"
    - "https://github.com/scarecrow173/synvelis/issues/99"
    - "https://github.com/scarecrow173/synvelis/issues/106"
    - "https://github.com/scarecrow173/synvelis/issues/109"
---

# SF-02 仕様 — DAW Routing Commands・Semantic Validation・Mixer/Send/Sidechain 連携

## 目的

SF-01(#104)の Semantic Routing 正本(`RoutingModel`)に対し、UI の配線
gesture を**1 CAS・1 Undo**の原子 Commit として成立させる
`RoutingTransaction` / `RoutingEditIntent` と、その候補 Project に対する
二層検証(Ruviform 汎用 Graph Validation + Synvelis DAW 意味/製品ポリシー)
を実装する。成功は 1 Undo step、失敗は Project・Live Revision・Undo・
接続の不変を保証する。

## 背景・前提

- Issue #105 の 2026-11-04 詳細契約(A/B/C 節)が合意済み製品ポリシーの正本。
- `ProjectOp::Composite::validate` は各 child を**元 Project** に対して検証
  するため、`CreateBus → CreateConnection` が同一 gesture 内の新 ID を参照
  する形を表現できない。`Composite` への追加ではなく独立した
  preflight/scratch staging を導入する(SF-01 ADR D5 の接続点)。
- `ProjectController::apply(Command)` が唯一の Authoring 変更経路。
  `expected_project_revision` は `Command::expected_revision` が担う。
- Runtime compile/publish/active は SF-03(#106)。本仕様の範囲は Project
  mutation と Project-only Undo までであり、RT publish 成功を宣言しない。

## Scope

### 含む

- `RoutingEditIntent { operation_id, expected_port_capability_generation,
  edits, approved_conversion }` と `RoutingEdit` 列:
  `CreateBus`/`RemoveBus`/`CreateConnection`/`RemoveConnection`/
  `SetConnectionEnabled`/`ChangeChannelMap`/`CreateSend`/`RemoveSend`/
  `SetSendTap`/`SetSendGainTarget`/`ReorderProcessor`/`SetMute`/`SetSolo`/
  `ChangeBusConfiguration`。
- TemporaryRef(TempBusId/TempSendId/TempConnectionId)を同一 Transaction 内で
  Stable typed ID へ解決する staging。全 edit を順に適用した候補 Project を
  作ってから最終検証・CAS Commit。
- `SemanticPort.channels` 追加(serde default で v7 snapshot をそのまま
  読める additive change)による channel count の persisted 正本。
- Mute/Solo は `ControlOwner::Strip` + `ParameterRole::Mute/Solo` の
  Boolean target を権威とし、strip graph 上の依存 closure を評価する
  audibility 評価(物理 edge 削除ではなく可聴経路ポリシー)。
- 汎用検証は `ruviform-graph` の `Validator` への `GraphSpec` 投影で実施。
  Synvelis 別の汎用 Graph Validator は作らない。
- `RoutingEditError` typed 分類: StaleRevision/StalePortCapability/
  DuplicateOwnership/UnresolvedPort/InvalidDomainOrDirection/
  UnsupportedChannelMap/FeedbackCycle/DanglingReference/DependencyInUse/
  RequiresUserDecision/ResourceBudget/GenericGraphRejected。
- `preflight` preview API(UI Drag/Preview は Project 未変更)と
  TempRef→Stable ID の deterministic receipt。
- `ProjectOp::Routing { intent }` として既存 CAS/Undo 機構に統合。

### 含まない

- Runtime への Lowering/PDC/Activation(SF-03 #106)、実機/実 Plugin/
  実 GUI 検証(#114)。headless で実測不能な受入は NotRun/Unsupported とする。
- Audio→TimedEvent/Control の実体 port(instrument routing 後続)。domain
  分離規則は oracle が保持する。
- 遅延付き feedback cycle(Synvelis Capability Policy として全 cycle 拒否を
  維持)、Send 毎の mute-follow 解除・Solo Safe・高度 Downmix プリセット
  (CTL-F01/#123・CTL-F02/#124)。
- Plugin param control/Automation 関係(PC-C2 #99 の別 Command)。

## Functional Requirements

- **SF-TX-01**: `CreateBus → CreateConnection`、`CreateSend`(+level
  target)を同一 gesture で 1 Commit・1 Undo/Redo。途中 Failure は
  Live/Revision/Undo/接続不変。
- **SF-TX-02**: preflight(preview)は Project を変更せず、commit 成功時と同一
  の検証列を通す。成功 receipt は TempRef→Stable typed ID 対応を含む。
- **SF-TX-03**: `expected_port_capability_generation` と PluginDeclared/
  Resolved の `capability_revision` 不一致は `StalePortCapability` で拒否。
  `Unresolved` binding 参照は `UnresolvedPort`。古い Port ID への自動配線は
  行わない。
- **SF-TX-04**: domain/direction/role/kind/ownership 規則は意味検証。
  同一 (source,destination) 重複・自己 owner 循環・Send 1:1 を保持。
- **SF-TX-05**: channel count 不一致の接続は `ExplicitChannelMap` +
  `approved_conversion = ExplicitMaps` 同意を要求。黙った up/downmix は
  ない。同意なしは `RequiresUserDecision`、map 不正は
  `UnsupportedChannelMap`。
- **SF-TX-06**: 全 feedback cycle 拒否(遅延有無を問わない)。汎用 Validator
  の cycle 検出と strip 投影の意味 cycle 検査の二段。
- **SF-TX-07**: Mute=対象 strip の main+pre/post send 可聴出力停止。
  Sidechain send は初期では source strip mute に追従。Raw capture 不変。
- **SF-TX-08**: Solo=選択 strip と信号を master へ届けるのに必要な経路
  (下流 bus/return・必要 sidechain 経路・bus solo の上流供給経路)を可聴に
  し、他を抑制。Return/Master は無条件に切断しない。複数 solo は論理和。
- **SF-TX-09**: `SetSendGainTarget` は control model の単一権威を保つ
  (send は target を adopt・第 2 の persisted gain を作らない)。
- **SF-TX-10**: Undo は Project-only。1 transaction = 1 undo step。

## Acceptance Criteria

- [ ] SF-VALID-01: CreateBus+Connect/New Send が同一 Gesture/TempRef で
  1 Commit+1 Undo/Redo。途中 Failure は Live/Revision/Undo 不変。
- [ ] SF-VALID-02: Same Source+Dest 複数 Send が独立 Typed
  SendId/ConnectionId で区別・1:1 互換。Missing/Disabled を正しく報告。
- [ ] SF-VALID-03: tap 属性が model 上で Main と混在せず、Track Mute が
  main+pre/post send の可聴出力を止める(実 PCM 検証は SF-03/実機、
  ここでは audibility 評価と persisted 状態で pin)。
- [ ] SF-VALID-04: Solo の依存 closure(track/bus/return/master/
  sidechain)が audibility 評価で一致し、複数 solo/共有 return/復元が
  model 不変条件を満たす。Raw Recording PCM 不変は範囲外(NotRun)。
- [ ] SF-VALID-05: Hardware direct monitor は範囲外と明示し、
  Synvelis mute/solo が影響すると偽らない(実機 NotRun)。
- [ ] SF-VALID-06: invalid port/domain/direction/role/multiplicity/map/
  plugin capability/feedback cycle を typed error で明示拒否。
  Unresolved plugin は保存可だが可聴と誤表示しない。
- [ ] SF-VALID-07: stale revision/stale port generation/multi-window
  gesture/undo/redo/partial composite で誤 target 変更や部分永続化なし。
- [ ] SF-VALID-08: Ruviform 汎用 Validation と DAW policy が混同しない
  (分離した検証列)。headless≠実 Device/Plugin GUI。

## 検証

- `mise run fmt-check` / `check` / `lint` / `test` / `build` 全緑。
- `crates/synvelis-project/tests/routing_sf02.rs` で契約を pin。
- 実機・実 Plugin 依存の受入項目は NotRun/Unsupported として理由を記録。
