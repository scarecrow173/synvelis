---
id: "ADR-034SF02Ux8pMn5oR8tY3zAbDe"
type: "adr"
status: "accepted"
title: "SF-02 RoutingTransaction 配置・TempRef Staging・channels 永続化・Mute/Solo 権威・汎用検証の再利用"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/105"
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
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
    - "docs/adr/sf-01-routing-model-schema-v6-migration.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-02-routing-transaction-commands-semantic-validation.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/designs/sf-02-routing-transaction-design.md"
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/184"
---

# SF-02 ADR — RoutingTransaction・TempRef Staging・channels・Mute/Solo 権威

## Status

accepted (Issue #105・SF-02 spec に基づく設計決定)

## Context

- SF-01 で `RoutingModel` 正本・`ProjectOp` routing ops・`ProjectController`
  の scratch clone + CAS + 1 Undo が確立済み。
- `ProjectOp::Composite::validate` は child を元 Project へ個別検証するため、
  gesture 内で生成する ID を他 child が参照する形を扱えない。
- Channel count の永続正本がなく、接続の layout 一致を Project 上で
  検証できない。mute/solo の authority も未定義。
- 汎用 Graph Validation(direction/domain/layout/multiplicity/fan-in/out/
  cycle/capability)は `ruviform-graph` の Validator が所有しており、
  Synvelis 側に別 Validator を作ることは禁則。

## Decision

### D1. 独立した `RoutingTransaction` staging を設ける

`RoutingEditIntent`(operation_id・expected_port_capability_generation・
edits・approved_conversion)を受け、synvelis-project が
(1) TempRef 事前割当(Bus/Send/Connection)
(2) 既存 routing op を適用した候補 Project 生成
(3) 全 invariant + 意味/汎用検証を候補へ適用
の順に staging する。`ProjectOp::Routing { intent }` として既存
controller の validate/apply 経路に乗せ、`Composite` は拡張しない。
Undo 記録は controller 標準経路に一本化する。

### D2. `SemanticPort.channels: u32` を additive に永続化

`#[serde(default)]` 付きで追加し schema version は v7 据置き(default=2)。
接続両端の channels 不一致は `ExplicitChannelMap` +
`approved_conversion = ExplicitMaps` の同意を要求する。format/
explicit convert node/convert port は後続(SF-03)に defer し、
consent を「explicit map 提示済み」として永続化する。

### D3. Mute/Solo は `ControlOwner::Strip` + `ParameterRole::Mute/Solo` の
Boolean target を persisted authority とする

`SetMute`/`SetSolo` は control model の Boolean target を唯一の authority
として設定する(SendConfig の gain 権威と同じ単一権威パターン)。
可聴評価(`RoutingModel::audibility`)は接続グラフ投影の上で
mute(自 strip の main+send 出力停止・sidechain 追従)/solo(必要経路のみ
可聴・論理和)を計算する。Raw capture は不変。物理 edge 削除ではなく
可聴経路ポリシーとして扱う。

### D4. 汎用検証は `ruviform_graph::Validator` への `GraphSpec` 投影

RoutingModel を `GraphSpec`(node=port owner、port=SemanticPort、
edge=RoutingConnection)に投影し、Validator で direction/domain/
layout/multiplicity/cycle を検査する。DAW 固有規則(kind/role
意味・Send 1:1・ownership・mute/solo 意味)は従来どおり
`RoutingModel::validate` と staging 検証が担う。

### D5. typed `RoutingEditError` で Invalid/Unavailable/RequiresConsent を分離

`RequiresUserDecision`(conversion 未同意)・`UnresolvedPort`・
`StalePortCapability`・`GenericGraphRejected` 等を分離し、呼び出し側が
consent 導線/無効理由を区別できるようにする。runtime 能力不足は
SF-03 管轄であり、本層では Unresolved/Unavailable を返す。

## Considered Options

### Option A: `ProjectOp::Composite` を拡張し TempRef を内部解決

- 却下。child validate が元 Project を見る制約を残したまま拡張すると
  責務が二重化し、receipt/世代検査の置き場が曖昧になる。

### Option B: 独立 preflight staging 型 + `ProjectOp::Routing` op(採用)

- 採用。1 gesture = 1 intent = 1 op = 1 CAS = 1 Undo。preview(preflight)
  と commit が同一検証列を共有し、TempRef→Stable ID の receipt を
  deterministic に返せる。SF-01 ADR D5 が想定した接続点に一致。

### Option C: Mute/Solo を接続の enabled フラグや runtime state で表現

- 却下。enabled は接続自体の ON/OFF であり mute/solo 意味と混同する。
  runtime state 化は Project 正本・Undo・保存/復元と矛盾する。

### Option D: Mute/Solo を Strip 固有フィールドとして永続化

- 却下。control model(ParameterTarget + AutomationLane + Assignment)の
  単一権威パターンから外れ、automation/modulation 対象にならない。
  CTL-01 の ControlOwner 型分離とも不整合。

### Option E: Synvelis 側に独自 cycle/graph 検査を書く

- 却下。ruviform-graph Validator が所有する汎用責務の重複実装であり
  Issue #105 が明示的に禁止。DAW 固有意义のみ残す。

## Consequences

- `RoutingModel::validate` は persisted oracle として維持し、staging 中は
  既知の構造失敗を「candidate で確認済み」として早期に打ち切る。
- `preflight` は Project を変更しない(UI Drag/Preview/cancel は
  project-neutral)。
- 遅延有無を問わない全 cycle 拒否は製品ポリシーとして維持し、
  高度例外(Send mute-follow 解除・Solo Safe・delayed feedback)は
  CTL-F02/#124 へ defer。
- channels 追加は additive で既存 v7 snapshot を破壊しない。
