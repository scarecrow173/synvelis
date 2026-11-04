---
id: "SPEC-034CTL01Ow9uXr5zA1bC2dEf"
type: "spec"
status: "approved"
title: "CTL-01 ControlOwner / AutomationTimeScope 責務分離・Bus/Strip/Send 型付き Schema 移行"
created: "2026-11-05"
updated: "2026-11-05"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/121"
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
    - "docs/specs/pc-a-parameter-control-model-scope-breakdown-state-machine.md"
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/ctl-01-control-owner-time-scope-schema-v7.md"
    - "docs/designs/ctl-01-control-owner-time-scope-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/4"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/ctl-01-e2e-control-owner-time-scope.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/104"
    - "https://github.com/scarecrow173/synvelis/issues/105"
    - "https://github.com/scarecrow173/synvelis/issues/101"
    - "https://github.com/scarecrow173/synvelis/issues/122"
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/98"
  defers: []
  deferred-by: []
---

# CTL-01 ControlOwner / AutomationTimeScope 責務分離・Schema v7 Migration

## What Should Be Built

Issue #121「2026-11-04 合意・初期実装」を実装する。現行 `ControlScope
{ Track(TrackId), Clip(ClipId), Note(NoteId), Project }` は **所有
(ownership)** と **automation 点の時刻座標 (time coordinate)** の 2 責務を
兼務しており、SF-01 で永続化された Bus/Strip/Send/Processor を所有
できる形になっていない。これを論理分離し、Project Schema v7 へ移行
する。

```rust
enum ControlOwner {
    Project, Track(TrackId), Strip(StripId), Send(SendId),
    Processor(ProcessorId), Clip(ClipId), Note(NoteId),
}
enum AutomationTimeScope {
    ProjectMusical, ProjectAbsolute, ClipLocal, NoteLocal,
}
enum ParameterRole {
    Generic, Fader, Pan, Mute, Solo, SendLevel, ProcessorParam,
}
struct ParameterBinding {
    target_id: ParameterTargetId,
    owner: ControlOwner,
    role: ParameterRole,
    time_scope: AutomationTimeScope,
}
```

- `ParameterTarget` は `owner: ControlOwner`・`role: ParameterRole`・
  `time_scope: AutomationTimeScope` をフラットなフィールドとして保持する
  (`ParameterBinding` はこの field group の概念契約であり、wire 上は
  `ParameterTarget` の一部として直列化する)。
- `ModulationSource` は `owner: ControlOwner` のみ保持する。source signal
  は assignment 先 target の解決済み座標で評価される既存契約のため
  `time_scope` は持たない。
- `ControlOwner` に Bus 変種は作らない。Bus の音声 Parameter は
  `StripOwner::Bus(BusId)` が指す Strip に `ControlOwner::Strip` で
  binding する。Track は Lane/Clip 制作 Owner、Bus/Return/Master は
  論理経路、Strip は唯一の DSP 正本。

## Why

- SF-01 の RoutingModel は Bus/Strip/Send/Processor の Stable ID を
  永続化したが、Control 側の owner 型がそれらを参照できず
  `SendConfig.gain_target` の owner mapping が未接続のまま残っている。
- `ControlScope::Clip/Note` は所有者と相対時刻座標を同一 enum に
  混在させており、Strip/Send/Processor 所有で project 時刻座標の
  target を表現できない。owner 軸と time_scope 軸の分離が SF-02
  (#105 RoutingTransaction)・CTL-02 (#122 Host Parameter 既定値)・
  PC-C4 (#101 Plugin Binding) の共通前提。
- pre-release 方針: 旧 `ControlScope` との実行時互換層は作らず、
  Versioned Migration で形式を作り直す。

## Acceptance Criteria

(Issue #121 §Acceptance が正本)

- 旧 `ControlScope` からの Versioned Migration は旧
  Track/Clip/Note/Project の評価座標を**厳密保持**する
  (`Track`/`Project` → `ProjectMusical`、`Clip` → `ClipLocal`、
  `Note` → `NoteLocal`、`role` は `Generic`)。既存の
  Evaluation/Modulation が Migration 前後で同一であることを
  Golden で検証する。
- 新 Bus/Strip/Send/Processor 所有の target は初期
  `ProjectMusical` を標準とし、Clip/Note の相対時刻座標は維持する。
  時刻変換不能は暗黙 fallback せず明示 `Unavailable` を返す。
- `1 Track/Bus → 1 Strip`(一意性方向)と `Master exactly-one` を
  `RoutingModel::validate` が保持し、型付きテストで検証する。
  Track の strip 未配線は合法(`AddTrack`/`AddTrackStrip` は別 op)。
- 同 `(owner, role)` の一意 role target 重複を Reject する。
  `Generic`/`ProcessorParam` は複数共存を許す。
- `ParameterTargetId` が唯一の永続値正本であり、`SendConfig.gain_target`
  が指す target は `ControlOwner::Send(send.id)` 所有でなければ
  ならない(owner mapping 検証)。`SendConfig` に第 2 の persisted
  gain 値を作らない。
- 既存 Target ID / AutomationLane / ModulationAssignment / Depth
  Driver と Serialize / Undo / VCS diff を保持する。`IdGenerator`・
  `id_watermark` 契約を壊さない。
- Dangling owner 参照・存在しない Strip/Send/Processor・非法
  owner×time_scope 組合せ・Undo/Redo/Save/Reload が
  `Project::validate_references` と write-time validation で整合
  する。
- SourceMap/Plugin Param の Binding と Project Stable ID が混在せず、
  誤 param 配送ゼロ。`EvalPlan::eval_rt` の allocation-free 契約と
  AudioLens f64/RT 経路を保護する。

## Scope Boundary(本 dispatch の範囲)

- **In scope**: `ControlOwner`/`AutomationTimeScope`/`ParameterRole` 型、
  `ParameterTarget`/`ModulationSource` の owner 移行、Schema v6→v7
  Migration、owner×time_scope 組合せ・(owner, role) 一意性・
  `gain_target`→`ControlOwner::Send` owner mapping の検証、
  evaluation/record/gateway projection の移行、Track/Bus→Strip 一意
  性と既存座標保持の型付きテスト。
- **Out of scope**: `RoutingTransaction`(Temp Ref→Stable ID、独立
  preflight)と Send + Connection + Control Target の原子的生成は
  SF-02 #105。Host Parameter 既定値・Mute/Solo の DAW 動作は CTL-02
  #122。Plugin capability 実検証は PC-C4 #101。RoutingModel→GraphSpec
  lowering は SF-03 #106。`ProjectAbsolute` の実座標解決
  (absolute-time point 型)は後続 Issue — 本件では型のみ導入し、
  評価は明示 `ScopeUnavailable`。GUI #110〜112 の実 Mixer 受入は
  待たない。

## Non-Functional Requirements

- cargo コマンドは `mise run <task>` 経由。時間演算は
  `checked_add`/`checked_sub` のみ。
- 互換層・重複実装・隠れ fallback を作らない。`Project::control` が
  値の唯一の正本、`RoutingModel::validate` が routing 不変条件の
  唯一の oracle、`Project::validate_references` が Project 整合の
  最終 oracle のまま。
- `EvalPlan` snapshot 契約(compile 時 freeze・placement は live 解決)
  と `eval_rt` 成功経路の非割当を維持する。
