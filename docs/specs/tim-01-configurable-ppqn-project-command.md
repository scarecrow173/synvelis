---
id: "SPEC-034UHVLQ9dtoN4YQDjtCy3"
type: "spec"
status: "approved"
title: "TIM-01 Configurable PPQN Project Command・非破壊 Migration（headless slice）"
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
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim-02-smf-format01-import-export.md"
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/125"
  derived-by:
    - "docs/designs/tim-01-configurable-ppqn-design.md"
    - "docs/plans/tim-01-configurable-ppqn.md"
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-configurable-ppqn.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/127"
    - "https://github.com/scarecrow173/synvelis/issues/128"
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/196"
    - "https://github.com/scarecrow173/ruviform/issues/253"
  defers: []
  deferred-by: []
---

# TIM-01 Configurable PPQN Project Command・非破壊 Migration（headless slice）

## 背景・目的

Synvelis Issue #125。Project の PPQN（ticks per quarter note）を固定 960 から
設定可能にし、音楽的意味を変えずに Project 全体の musical tick グリッドを
再投影する。headless で完結する slice（Project Command/Controller CAS・
Undo/Redo・Save/Reload・非破壊 Migration の headless fixture）を対象とし、
製品 UI・実 OS/実 GUI・製品 Oracle は対象外。

## 要求

- Project レベルの PPQN 設定 Command。`ProjectOp::SetProjectPpqn` が
  `expected_revision` CAS と `old_ppqn` の二重 CAS を持つ。
- 選択可能範囲は `1..=32767`（SMF 指向 UI slice と同一）。
- PPQN 変更は project 所有の全 musical tick データを exact rational
  beat 経由で再投影する。単なる `project.ppqn` 代入は禁止。
- Exact policy では target grid に表現不能な位置を拒否。明示的な
  rounding は宣言された `Round` policy 経由のみ許可し、暗黙の
  quantization は禁止。
- Atomic validation: 変換失敗は live mutation 前に検出される
  （scratch clone 検証）。1 Command = 1 Undo entry。
- Open recording session 中は PPQN 変更を拒否。
- 変換は Ruviform の checked API（`TickPosition::to_beat` /
  `from_beat` / `rescale`、`Rounding`、checked `BeatPosition` 演算）
  のみを使い、Ruviform の一般化 Time API を Synvelis で重複実装しない。

## 再投影対象（project 所有の musical tick データ）

- `Project.ppqn` と `TempoMapData.ppqn`（両者の一致を不変条件化）。
- Tempo event / Signature event の tick 位置。
- Musical `TimeAnchor`・`ClipExtent::Musical` を持つ clip。
- Musical `ProjectRange` を持つ comping target/region/transition。
- その他 aggregate 内の project-level musical tick 位置。
- Tempo map の revision を grid 変更時に更新し、stale `TempoMapRef`
  identity が有効であり続けないようにする。

## 保存対象外（変更してはならないデータ）

- `TimeAnchor::Absolute` / `ClipExtent::Absolute` の絶対メディア配置。
- Source-native `MidiTick` 値、SMF document/source-native event stream、
  raw bytes・provenance。
- Media frame / recording / session timestamp / frame domain。
- Plugin frame base。
- 旧 snapshot bytes と旧 snapshot の意味（load 時の migration は
  read-only projection。save は新 snapshot を発行）。

## スキーマ・Migration

- 本 slice は `SCHEMA_VERSION`（現行 9）bump を伴わない。既存
  snapshot は `project.ppqn == tempo_map.ppqn == 960` の不変条件を
  満たすため、
  旧 snapshot の load/save/reload golden は既存 migration 経路で
  非破壊に維持されることを fixture で証明する。
- 将来 schema bump が必要になった場合は versioned migration registry
  に従い、旧 revision の意味を変更しない。

## 受入基準（headless slice）

- PPQN Command が CAS/Undo/非破壊 Migration つきで実装され、
  headless fixture（`crates/synvelis-project/tests/tim_01_ppqn.rs`）
  が green。
- Exact 変換・Round policy・inexact 拒否・overflow 拒否・
  範囲外拒否・open-recording 拒否・Undo/Redo・Save/Reload・
  旧 snapshot 非破壊・絶対メディア/ソースネイティブ保存・
  複数 PPQN 変更の合成を fixture が証明する。
- `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `test-msrv` が全て exit 0。
- docs チェーン完備・索引再生成済み。実 OS/実 GUI・製品 Oracle は
  NotRun として正直に記録する。

## 非目標

- 製品 UI（PPQN 選択 UI、Presentation Driver への時刻正本移動）。
- Ruviform 側の一般化 Time API 変更。
- 既存 project データの自動 retiming・quantization。
