---
id: "TSPEC-TiM02PpqnSmfXg05"
type: "test-spec"
status: "approved"
title: "TIM-02 × TIM-01 PPQN×SMF cross-golden fixture — test spec"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "crates/synvelis-project/tests/tim_02_ppqn_smf.rs"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/tim-02-smf-format01-fixtures.md"
    - "docs/test-specs/tim-01-configurable-ppqn.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/tim-02-ppqn-smf-cross-golden-design.md"
  derived-by: []
  verifies:
    - "docs/specs/tim-02-ppqn-smf-cross-golden.md"
  verified-by: []
  references:
    - "docs/tasks/tim-02-ppqn-smf-cross-golden-task.md"
  defers: []
  deferred-by: []
---

# TIM-02 × TIM-01 PPQN×SMF cross-golden fixture — test spec

## Purpose

Issue #126 受入行「PPQN 異種の Import / Project Change /
Save / Export / 再 Import」の交差 — SMF-backed source を
持つ project への `SetProjectPpqn` — が
`crates/synvelis-project/tests/tim_02_ppqn_smf.rs` で
end-to-end に固定されることを保証する。
`ppqn.rs` の source-native skip 不変条件（`midi_sources`
の division/note tick・`smf_documents` は re-projection
対象外）と `export_smf` の project-PPQN 非依存性を、
observable な bytes/doc equality として検証する。
parse 異常系・chase・undo 等の単体契約は
`docs/test-specs/tim-02-smf-format01-fixtures.md` /
`docs/test-specs/tim-01-configurable-ppqn.md` が担い、
本 spec が退役するのは本 slice が上位 slice に統合
された時点。

## Feature

Format 1 division-480 の SMF document を project PPQN
960 の project に import し、SMF source を読む musical
clip を anchor した上で `SetProjectPpqn` を Exact/Round
で適用し、save/load を経て export@480 の bytes が
不変であり、export→reparse→reimport が document・
note 射影・tempo/CC64 provenance を保存する。

## Rules

- `SetProjectPpqn` 適用の前後で `smf_documents[source]`
  と `midi_sources[source]`（division・note
  projection）が変更前 clone と一致しなければならない
  — source-native データは musical tick domain の
  re-projection walk の外にある。
- musical clip の anchor/extent のみが project PPQN
  grid 上で再投影される（Exact: whole-beat で厳密、
  Round: 宣言済み rounding で量子化）。playback
  `source_region` は media-domain で不変。
- `export_smf(Format1, division=480)` の出力 bytes は
  PPQN 変更前・変更後・save/load 後の 3 時点で一致
  しなければならない — export は `midi_sources`/
  `smf_documents` のみに依存し `project.ppqn` を
  参照しない。
- division=source division の export は全変換 exact
  でなければならない（`exact_count == event_count`・
  `rounded_count == 0`・`max_error == ZERO`・
  `unsupported_events` 空）。
- `parse_smf(export bytes)` は元 `SmfDocument` と
  `PartialEq` で一致し、re-import した project の
  `smf_documents` が同じ document を保持し、note
  射影 `(key, velocity, start, len)` が一致しなければ
  ならない。
- tempo event（2 件）と CC64 sustain event（3 件）の
  provenance が export→reparse 後も存在しなければ
  ならない。
- `Exact` policy は inexact musical 値を原子的に
  reject しなければならない（revision・live state
  不変）。

## Examples

- `ppqn_exact_change_preserves_smf_chain_end_to_end`:
  Given PPQN 960 project + Format1/480 import +
  whole-beat clip、When `SetProjectPpqn(960→480,
  Exact)` → save/load → `export_smf(Format1,480)`、
  Then clip anchor 3840→1920・extent 1920→960、
  `smf_documents`/`midi_sources` は不変、export bytes
  は変更前 golden と一致、reparse == 元 doc
  （tempo×2・CC64×3 保持）、re-import で doc・note
  tuple が一致。
- `ppqn_round_change_still_preserves_smf_source_native_
  data`: Given 同上だが anchor tick 1（inexact）、
  When `Exact` が原子的 reject → `Round(Floor)` で
  960→1000 適用、Then anchor は 25/24→1 tick に量子化
  されるが `smf_documents`/`midi_sources`/export bytes
  は依然として不変で、save/load・export@480 golden・
  re-import doc equality が保たれる（export が
  project PPQN=1000 を参照しない追加証拠）。

## Guarantees

- source-native skip 不変条件は project PPQN 変更・
  save/load・export・re-import の全鎖で保持される。
- export の determinism は PPQN ドメインを跨いでも
  bytes レベルで再現される。
- tempo/CC64 を含む全 event kind・track 構造・event
  順序が export→reparse で `PartialEq` 保存される。
- `Exact`/`Round` の policy 差は musical tick domain
  にのみ現れ、SMF provenance には漏れない。

## Non-goals

- 実機/実 backend・GUI・RT 経路（NotRun）。
- player/app/gateway 層の PPQN×SMF 結合。
- SMPTE/Format2/MIDI2・AudioLens f64（委譲）。
- `tim_01_ppqn.rs`/`tim_02_smf.rs` が担保する単体
  契約の再検証。

## Risk

- fixture が実不変違反（実バグ）を露呈した場合 —
  `src/{project,smf,ppqn}.rs` の変更を例外的に許容し
  根拠を impl IR に記録する運用。本 slice では発生
  せず。
- golden bytes が exporter の wire 詳細に依存する —
  wire 語彙の正しさは `tim_02_smf.rs` の golden が
  担保し、本 fixture は同一 exporter の再現性
  （PPQN 非依存）のみを主張する。
