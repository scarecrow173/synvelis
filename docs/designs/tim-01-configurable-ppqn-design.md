---
id: "DESIGN-034UHVNvFQXFumTOowIt0g"
type: "design"
status: "approved"
title: "TIM-01 Configurable PPQN — Rational Beat 再投影設計"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
  implemented-by:
    - "docs/plans/tim-01-configurable-ppqn.md"
  depends-on:
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
  derived-by:
    - "docs/plans/tim-01-configurable-ppqn.md"
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-configurable-ppqn.md"
  references:
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/196"
    - "https://github.com/scarecrow173/ruviform/issues/253"
  defers: []
  deferred-by: []
---

# TIM-01 Configurable PPQN — Rational Beat 再投影設計

## 概要

Project PPQN を `1..=32767` の範囲で変更可能にする。変更は
「project 所有の全 musical tick を rational beat へ持ち上げ、
target PPQN グリッドへ再投影する」一括変換として実装し、
`ProjectOp::SetProjectPpqn` Command として既存
`ProjectController` の CAS/Undo/Save 経路に乗せる。

## 責務境界

- **Ruviform（Owner）**: Rational Beat / Clock / Generic
  Tick-Frame 変換の共通契約。`Ppqn`、`BeatPosition`、
  `TickPosition::{to_beat, from_beat, rescale}`、`Rounding`、
  checked `BeatPosition` 演算。Synvelis ではこれらを再実装しない。
- **synvelis-project（本 slice）**: PPQN 選択範囲・変換 policy
  （`Exact` / `Round`）・project aggregate 全体の再投影順序・
  CAS/atomic validation・open-recording 拒否という
  Project ドメインのポリシー境界。
- **synvelis-app / gateway**: transport-stopped・automation-write・
  merge-commit などの UI/セッション前提条件。本 slice では
  `synvelis-project` に重複実装しない。

## 変換モデル

各 musical tick `t`（旧 PPQN `p_old`）について:

1. `beat = TickPosition(t).to_beat(p_old)` — exact rational
   quarter-note 位置へ lift。
2. `t' = TickPosition::from_beat(beat, p_new, rounding)` —
   target grid へ投影。

`Exact` policy では `from_beat` が inexact を返した時点で
operation 全体を拒否する。`Round` policy は呼び出し側が宣言した
`Rounding`（`Floor`/`Ceil`/`TowardZero`/`AwayFromZero`/
`NearestEven`）を全位置に一貫適用する。暗黙の丸めは存在しない。

負の musical tick（clip 開始が 0 より前）も `MusicalTick(i64)`
の checked 演算と Ruviform の符号対応 rounding で扱う。

## Command / CAS 設計

```rust
ProjectOp::SetProjectPpqn {
    old_ppqn: u32,
    new_ppqn: u32,
    policy: PpqnConversionPolicy,
}
```

- `Command.expected_revision` CAS（既存 controller 機構）。
- `old_ppqn` は live `project.ppqn` とのドメイン CAS。
  stale UI/別窓からの適用を拒否する。
- `new_ppqn` は `ppqn_in_range`（`1..=32767`）で検証。
- `SessionState::Open` を持つ project では拒否
  （recording in progress の headless 表現）。
- `validate` は project を clone して `reproject_ppqn` を試行し、
  変換不能・overflow を live mutation 前に検出する
  （atomic validation）。`apply` は検証済みの変換を再実行する。
- Undo/Redo は controller の prior-state stack で 1 entry。

## 再投影対象と順序

`reproject_ppqn(project, old, new, policy)` は以下を一括変換する:

1. `TempoMapData`: 全 tempo/signature event の tick 位置、
   `ppqn` フィールド、revision 前進（stale `TempoMapRef` 防止）。
2. `Project.ppqn`。
3. 全 clip の `TimeAnchor::Musical` と `ClipExtent::Musical`。
4. Comping の musical `ProjectRange`（target/region/transition）。

変換不能が一箇所でもあれば operation 全体が失敗し、
project は無変更のまま残る。

## 保存対象外データ（不変条件）

- `TimeAnchor::Absolute` / `ClipExtent::Absolute`。
- Source-native `MidiTick`、SMF document・event stream・raw bytes。
- Media frame、recording/session timestamp、plugin frame base。

これらは musical tick グリッドに属さないため再投影対象から除外し、
fixture で byte/value 同一性を検証する。

## `project.validate_references` の強化

PPQN coherence を project 不変条件として追加:

- `project.ppqn` と `tempo_map.ppqn` の一致。
- 両者が `1..=32767` の範囲内。

これにより snapshot load・他 op 適用後も grid 不整合が
検出される。

## Schema / Migration

`SCHEMA_VERSION` bump なし。PPQN は既存 serialized field の
値域拡張であり、旧 snapshot（960 固定で保存済み）はそのまま
load 可能。migration は read-only projection、save は新 snapshot
発行という既存契約を維持する。fixture
`ppqn_change_survives_save_reload_and_keeps_old_snapshot` が
旧 snapshot bytes 非破壊を証明する。

## リスクと対策

| リスク | 対策 |
| --- | --- |
| inexact 変換の暗黙丸め | `Exact`/`Round` policy を型で分離、fixture で拒否を検証 |
| i64 tick overflow | Ruviform checked API + `MusicalTick` checked 演算のみ使用、overflow fixture |
| stale TempoMapRef | tempo map revision を grid 変更時に前進 |
| recording 中の grid 変更 | `SessionState::Open` で fail-closed 拒否 |
| 変換途中の部分適用 | scratch clone で全変換を先に検証してから適用 |
