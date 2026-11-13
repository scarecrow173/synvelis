---
id: "DESIGN-JxGLd6Q5TTckxCpvsulzYJ"
type: "design"
status: "approved"
title: "TIM-01 PPQN config — punch marks 再投影と受入残件検証設計"
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
  implements:
    - "docs/specs/tim-01-ppqn-config.md"
  implemented-by:
    - "docs/plans/tim-01-ppqn-config-plan.md"
  depends-on:
    - "docs/specs/tim-01-ppqn-config.md"
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
    - "docs/designs/tim-01-configurable-ppqn-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/tim-01-ppqn-preview-design.md"
    - "docs/designs/tim-02-ppqn-smf-cross-golden-design.md"
  refines:
    - "docs/designs/tim-01-configurable-ppqn-design.md"
  refined-by: []
  derives-from:
    - "docs/specs/tim-01-ppqn-config.md"
  derived-by:
    - "docs/plans/tim-01-ppqn-config-plan.md"
  verifies: []
  verified-by:
    - "docs/test-specs/tim-01-ppqn-config.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
    - "https://github.com/scarecrow173/synvelis/issues/17"
  defers: []
  deferred-by: []
---

# TIM-01 PPQN config — punch marks 再投影と受入残件検証設計

## 概要

先行 slice（`tim-01-configurable-ppqn-design`）の再投影 walk
`reproject_ppqn_with` が `Project::monitoring.punch` の
`punch_in`/`punch_out` を site 列挙から漏らしている。両者は
`MusicalTick`（project PPQN grid の tick）を永続化するため、
`SetProjectPpqn` 適用後も旧 grid の値を保持し続ける —
grid だけが進む silent retime であり、本設計で修復する。

併せて Issue #125 受入条件の `synvelis-project` 範囲の残件を
headless fixture で固定する。変換モデル・CAS・policy・
schema 方針は先行設計をそのまま継承し、本 slice では
site 列挙の完全性と検証被覆のみを扱う。

## 責務境界

- **synvelis-project（本 slice）**: `reproject_ppqn_with` の
  site 列挙完全性（punch marks を含む全 persisted musical
  tick）、`preview_ppqn` の同一列挙、headless fixture。
- **Ruviform（Owner）**: `TickPosition::{to_beat,from_beat}`・
  `Rounding`・checked `BeatPosition` — 変更なし・重複実装なし。
- **synvelis-app / workspace**: PPQN 選択 UI・Preview 表示・
  transport/automation/merge 前提条件 — 別 slice・別受入。
  `PpqnPreviewSite.kind` は `&'static str` の wire 文字列で
  あり、新 kind 追加は app 層の JSON 投影を壊さない
  （`ppqn_site_json` は kind をそのまま透過する）。

## site 列挙の完全性（修復点）

`Project` aggregate が永続化する musical tick domain の全
site を再監査した結果:

| site | 型 | 現行 walk | 本 slice |
| --- | --- | --- | --- |
| tempo/signature event tick | `i64` tick | ✓ | 変更なし |
| clip `TimeAnchor::Musical` / `ClipExtent::Musical` | `MusicalTick`/`MusicalDuration` | ✓ | 変更なし |
| comp take-group/region/transition `ProjectRange::Musical` | `MusicalRange` | ✓ | 変更なし |
| `monitoring.punch.punch_in`/`punch_out` | `Option<MusicalTick>` | **✗（漏れ）** | **追加** |
| clip `Note.start`/`len` | `BeatPoint` | 対象外（rational beat 正本・PPQN 非依存） | 変更なし |
| automation `BeatPoint` 系 | `BeatPoint` | 対象外（同上） | 変更なし |
| `MidiSource` notes/`MidiTick`/`division`、`smf_documents` | source-native | 対象外（保存対象外） | 変更なし |
| recording session/source frames | frame domain | 対象外（保存対象外） | 変更なし |
| `plugin_instances`/`routing`/`control` authority | tick 非含有 | 対象外 | 変更なし |
| `MidiPlanEvent`/`MidiPlaybackPlan` | `MusicalTick` | 非永続（compile 出力・Project 非保持） | 変更なし |

punch marks の site 識別は `PpqnSite::single("punch_in"|
"punch_out", "project")` — project-wide singleton であり
instance 識別子は `"project"`、`part` は使わない
（range endpoint ではなく独立した 2 つの single tick）。
`Option::None` は site を生やさない（`{None,None}` は
「punch 未宣言」の authored 状態で tick を持たない）。

## 変換モデル（継承・変更なし）

各 musical tick `t`（旧 PPQN `p_old`）:

1. `beat = TickPosition(t, p_old).to_beat()` — exact rational
   quarter-note 位置へ lift。
2. `t' = TickPosition::from_beat(beat, p_new, rounding)` —
   target grid へ投影。

`Exact` は非表現を即拒否、`Round` は宣言 `Rounding` を一貫適用。
punch marks も同一 `convert` コールバックを通るため、
atomic validation（scratch clone dry-run）・overflow 検出・
revision 前進の契約は自動的に継承される。

## 受入残件の検証設計

`tests/tim_01_ppqn.rs` を拡張し、既存 fixture `seeded()` に
`SetPunchMarks`（whole-beat 値）を追加して canonical fixture
が全 site kind を含むようにする。`musical_positions` に
punch entries を加え、beat 保存の検証対象へ含める。

新規 test（被覆表は test-spec 参照）:

- **6 PPQN 行列**: `1/480/960/1000/1920/32767` 各 target で
  apply → `InMemoryStore` save → 別 controller load →
  beat 位置一致 → undo → redo を検証。TempoMap 解決は
  `resolve_span` の frame 一致で確認（Beat→Frame RT 丸め再現）。
- **実 v5 snapshot**: `alib02-v5-legacy.json` を
  `InMemoryStore` head として load（migration 経路）→
  `SetProjectPpqn` 960→480 Exact → 具体 tick の半減と
  `midi_sources`/`notes` の完全一致 → save で新 snapshot
  発行・v5 bytes byte-identical → reload で ppqn 480 と
  再投影 tick を確認。
- **分母上限**: ppqn 960 の project に `1/960` beat の位置
  （clip anchor tick 1）を置き、32767（選択上限）への `Exact`
  が拒否・`Round` が宣言 rounding（Floor 34 / Ceil 35）で
  投影することを検証（beat の既約分母 960 が target grid
  32767 に非表現）。
- **同 tick 複数 site**: tempo event・signature event・clip
  anchor/extent・punch_in が tick 960 を共有する fixture で、
  再投影後も全 site が同一 tick（480）へ揃うことを検証。
- **保存対象外の安全性**: automation lane/region（`BeatPoint`）、
  `control`/`plugin_instances`/`routing`/`notes`/`midi_sources`/
  `smf_documents`/`recording`/`applied_imports` の subtree が
  PPQN 変更の前後で完全一致することを検証。
- **異 Division SMF import**: division ≠ project ppqn の
  `SmfDocument` を `ImportMidiSmf` で取り込み、PPQN 変更後も
  `smf_documents`（document 全体）と `midi_sources`
  （division・notes）が完全一致することを検証。export 側の
  cross-golden は `tim_02_ppqn_smf.rs` が既被覆。

## リスクと対策

| リスク | 対策 |
| --- | --- |
| punch marks 追加で既存 preview site_count assertion がずれる | `seeded()` に punch を含める変更に合わせて assertion を更新し、inexact/overflow fixture（punch 無し）は不変であることを確認 |
| v5 fixture の migrate 後 project が PPQN 変更の前提を満たさない | fixture の全 musical 位置は whole-beat（Exact で全 target に再投影可能）であることを確認済み。open session 無し |
| 分母上限 test が別の overflow 経路を踏む | `1/32767` beat は i64 範囲内。拒否理由が inexact（Exact）であることを `rejection`/error で確認 |
| app 層への kind 追加影響 | `kind` は `&'static str` wire 透過。app 側の exhaustive match が無いことを確認済み（`ppqn_site_json`/`site_row` は表示のみ） |
