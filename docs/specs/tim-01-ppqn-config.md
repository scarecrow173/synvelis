---
id: "SPEC-LmEvu9ETGyhBWDJtTBWj6l"
type: "spec"
status: "approved"
title: "TIM-01 PPQN configurable project command — punch marks 再投影と受入残件（headless slice）"
created: "2026-11-14"
updated: "2026-11-14"
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
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
    - "docs/specs/tim-01-ppqn-preview.md"
    - "docs/specs/tim-01-app-ppqn.md"
    - "docs/specs/tim-01-ppqn-ui.md"
  refines:
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  derived-by:
    - "docs/designs/tim-01-ppqn-config-design.md"
    - "docs/plans/tim-01-ppqn-config-plan.md"
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-ppqn-config.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/19"
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/114"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN configurable project command — punch marks 再投影と受入残件（headless slice）

## 背景・目的

Synvelis Issue #125 の残件 slice。先行 slice
（`docs/specs/tim-01-configurable-ppqn-project-command.md`、
PR #235 merged）で `ProjectOp::SetProjectPpqn`・rational beat
再投影・CAS/Undo/Save/Reload 基盤は実装済みだが、再投影 walk
（`reproject_ppqn_with`）が `Project::monitoring.punch` の
`punch_in`/`punch_out`（永続化される project musical tick）を
訪問していない。これは旧 grid の tick 値を残したまま
`project.ppqn`/`tempo_map.ppqn` だけが進む silent retime であり、
「No Silent Retime」契約違反である。

本 slice は (a) punch marks を再投影対象へ追加して契約を回復し、
(b) Issue #125 受入条件のうち `synvelis-project` で検証可能な
残件（6 PPQN 値の Save/Reload/Undo/Redo 行列、実 v5 snapshot
経由の非破壊 migration、分母上限・同 tick 複数 site・異 Division
SMF import・可変テンポ下の No Silent Retime、Automation/
Plugin/Comp/Recording/Source raw MIDI の安全性）を headless
fixture で証明する。GUI Browser・実 OS・Workspace 側配線は別受入。

## 要求

- `reproject_ppqn_with` の site 列挙に `monitoring.punch` の
  `punch_in`/`punch_out` を追加し、mutating command と
  `preview_ppqn` dry-run が同一 site を報告する（既存の
  shared-enumeration 不変条件を維持）。
- 変換は既存と同じ exact rational beat 経由
  （`TickPosition::to_beat` → `from_beat`、`Exact`/`Round`
  policy）。暗黙の quantization・部分適用は導入しない。
- `PpqnSite`/`PpqnPreviewSite` の kind 語彙に punch marks を
  追加し、表示名・doc コメントを実態へ合わせる。
- 既存の schema version・snapshot 形式・op 形状は変更しない
  （punch marks は既存 serialized field の値域内の再投影であり
  schema bump 不要）。
- 受入残件を `crates/synvelis-project/tests/tim_01_ppqn.rs` の
  fixture 拡張と新規 test で固定する。既存 test で被覆済みの
  ケースは重複追加せず、test-spec の被覆表に記録する。

## 保存対象外（変更してはならないデータ）

- `TimeAnchor::Absolute` / `ClipExtent::Absolute` の絶対メディア配置。
- Source-native `MidiTick`、`MidiSource.division`、SMF document・
  event stream・raw bytes・provenance（異 Division import を含む）。
- Media frame、recording/session/source frame、device timestamp。
- Plugin frame base・plugin instance identity・routing/control
  authority（automation `BeatPoint` は rational beat 正本であり
  PPQN 非依存 — 値の同一性を検証する）。
- 旧 snapshot bytes と旧 snapshot の意味（load 時 migration は
  read-only projection。save は新 snapshot を発行）。

## スキーマ・Migration

- `SCHEMA_VERSION` bump なし。対象は Project schema（Issue #17
  系の project snapshot 体系）であり、Workspace 別 schema・
  Ruviform 側 schema とは別物 — 混同しない。
- 実 v5 snapshot fixture（`alib02-v5-legacy.json`）の load →
  migrate → `SetProjectPpqn` → save → reload を検証し、v5 bytes
  の非破壊と再投影結果の保持を証明する。

## 受入基準（headless slice）

- punch marks が Exact/Round 両 policy で他の musical site と
  同一規則に従い再投影され、preview が同 site を報告する。
- PPQN `1/480/960/1000/1920/32767` の各値について、Save →
  Reload → Undo → Redo で ppqn・全 musical beat 位置・
  TempoMap 解決（Beat→Frame）が不変であることを fixture が
  証明する。
- 実 v5 snapshot が migrate 後に PPQN 変更を受け、v5 bytes が
  byte-identical のまま残り、source-native MIDI data が
  完全一致を保つ。
- 分母上限（beat の既約分母が target grid に非表現の位置）、
  同一 tick を共有する複数 site、異 Division の SMF import、
  可変テンポ下で No Silent Retime（Exact 拒否または宣言済み
  Round のみ）を fixture が証明する。
- `mise run fmt-check` / `check synvelis-project` / `lint
  synvelis-project` / `test synvelis-project` が exit 0。
- docs チェーン完備・索引再生成済み。GUI/実 OS/Workspace 側は
  NotRun として正直に記録する。

## 非目標

- GUI の PPQN 選択・Preview 表示・Browser/実 OS 受入
  （synvelis-app/workspace 側の別 slice・別受入）。
- transport-stopped・automation-write・merge-commit 等の
  app 層前提条件（`synvelis-project` に重複実装しない）。
- Ruviform 側の Rational Beat/Time API 変更。
- Project Schema（#17）や Workspace 別 schema の版管理変更。
