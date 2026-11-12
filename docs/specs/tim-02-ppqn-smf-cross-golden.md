---
id: "SPEC-TiM02PpqnSmfXg01"
type: "spec"
status: "approved"
title: "TIM-02 × TIM-01 PPQN×SMF cross-golden fixture — synvelis#126 受入行の Project Change 補完（headless）"
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
  implemented-by: []
  depends-on:
    - "docs/specs/tim-02-smf-format01-import-export.md"
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/tim-02-smf-format01-import-export.md"
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
    - "docs/designs/tim-02-smf-format01-core.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/test-specs/tim-02-smf-format01-fixtures.md"
    - "docs/test-specs/tim-01-configurable-ppqn.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/126"
  derived-by:
    - "docs/designs/tim-02-ppqn-smf-cross-golden-design.md"
    - "docs/plans/tim-02-ppqn-smf-cross-golden-plan.md"
  verifies:
    - "docs/specs/tim-02-smf-format01-import-export.md"
  verified-by:
    - "docs/test-specs/tim-02-ppqn-smf-cross-golden.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/125"
    - "https://github.com/scarecrow173/synvelis/issues/19"
  defers:
    - "synvelis-player の MIDI RT/Offline 結合・実機 MIDI timing（別 slice・NotRun）"
    - "実 MIDI Backend OS 別・実 Device/Plugin 検証（Ruviform #253 の責務）"
    - "SMPTE Division・MIDI 2.0/MPE・Format 2（#129/#130 future・明示 Unsupported）"
    - "AudioLens Native f64 非回帰（#294 別 Gate）"
  deferred-by: []
---

# TIM-02 × TIM-01 PPQN×SMF cross-golden fixture（synvelis#126 受入行の Project Change 補完・headless）

## 背景・目的

Issue #126（TIM-02 SMF Format0/1 Import/Export）の受入行は
「PPQN 異種の Import / **Project Change** / Save / Export /
再 Import で Native Tick・Note order・Tempo・CC64 が保持され、
丸め数と最大誤差が `ConversionReport` で計測できる」を要求
する（`docs/specs/tim-02-smf-format01-import-export.md` L168）。

merge 済み slice はそれぞれの鎖を検証した:

- TIM-02 project+recording slice（PR #225）: `tim_02_smf.rs`
  が import・provenance 永続化・export golden・reimport・
  channel-aware chase を検証 — ただし `SetProjectPpqn` を
  通さない。
- TIM-01 PPQN slice（PR 群）: `tim_01_ppqn.rs` が
  `SetProjectPpqn` の Exact/Round・save/load・undo を検証 —
  ただし `midi_sources` のみを対象とし、`smf_documents`
  provenance と export golden を通さない。

本 slice は受入行の残った交差 — **SMF-backed source を持つ
project に対する `SetProjectPpqn`（Project Change）を挟んだ
全鎖** — を `crates/synvelis-project/tests/
tim_02_ppqn_smf.rs` の headless fixture で固定する。
`ppqn.rs` は source-native データ（`midi_sources` の
division/note tick・`smf_documents`）を意図的に
re-projection 対象外とする不変条件を持ち、本 fixture は
その不変条件を end-to-end で検証する側である。`src/` の
変更は行わない（fixture が実不変違反を露呈した場合のみ
例外的に許容し、根拠を impl IR に記録する）。

## 要求

- 新規 fixture `crates/synvelis-project/tests/
  tim_02_ppqn_smf.rs` で以下の全鎖を検証する:
  1. **Import @480**: Format 1・division 480 の SMF
     （conductor に tempo×2・time signature、event track に
     channel-aware CC64 sustain・closed note）を
     `ProjectOp::ImportMidiSmf` で project PPQN 960 の
     project に取り込む — 「PPQN 異種」の成立。
  2. **Clip anchor**: 取り込んだ SMF source を読む
     musical clip を追加する（anchor/extent は project
     tick grid 上の値）。
  3. **Project Change**: `SetProjectPpqn` を
     `PpqnConversionPolicy::Exact`（960→480）と
     `Round`（960→1000、inexact anchor 下で Exact が
     原子的に reject することも確認）の両 policy で適用
     する。
  4. **Save / Load**: `InMemoryStore` 経由で snapshot を
     保存・復元し、ppqn・clip・`midi_sources`・
     `smf_documents` が保持されることを確認する。
  5. **Export @480 byte-golden**: `export_smf`（Format 1・
     division 480）を PPQN 変更の前後・save/load の前後で
     実行し、出力 bytes が完全に一致すること（export が
     source-native slice のみに依存し project PPQN を
     参照しないことの end-to-end 証明）、`ConversionReport`
     が全変換 exact・`rounded_count = 0`・`max_error = 0`・
     `unsupported_events` 空であることを確認する。
  6. **再 Import doc equality**: export bytes を再 parse
     して `ImportMidiSmf` し、格納 document が元
     document と `PartialEq` で一致し、note 射影
     （key/velocity/start/len）が一致することを確認する。
- 各段階で `smf_documents[source]` と
  `midi_sources[source]`（division・note projection）が
  変更前 clone と一致すること — `SetProjectPpqn` が
  musical tick domain のみを re-projection し
  source-native データに触れない不変条件 — を明示的に
  assert する。
- tempo event（2 件）と CC64 sustain event（3 件）の
  provenance が export→reparse 後も保持されることを
  event kind 集計で確認する。
- `src/` の公開 API のみを消費する。fixture が実不変
  違反（実バグ）を露呈した場合のみ `src/{project,smf,
  ppqn}.rs` の変更を許容し、根拠を impl IR に記録する。
- doc chain を `tim-02-ppqn-smf-cross-golden` 命名で
  作成する（spec/design/plan/task/test-spec/impl-ir）。

## 受入基準（本 slice）

- `tim_02_ppqn_smf.rs` が上記全鎖を 2 テスト
  （Exact chain・Round chain）で green に通す。
- `SetProjectPpqn` Exact/Round の前後で
  `smf_documents`・`midi_sources` が byte/値レベルで
  不変であり、export@480 bytes が golden 一致する。
- export→reparse→reimport で `SmfDocument` が
  `PartialEq` 一致し、tempo/CC64 provenance が保持
  される。
- `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `check-app-native` / `lint-app-native` /
  `test-msrv` / `build` が全て通る。実行不能な検証は
  NotRun / Unsupported と記録する。

## 非目標

- `synvelis-app`/`synvelis-player`/`synvelis-recording`/
  `synvelis-workspace`/`synvelis-gateway` の変更
  （他 Worker 占有・gateway lowering・app op・player
  clip→MIDI 配線は対象外）。
- `ops.rs`/`control.rs`・`tests/ctl_02.rs` の変更
  （consume-only）。
- 実 MIDI backend・OS 実機・AudioLens f64・SMPTE/
  Format 2/MIDI 2.0（NotRun・#253/#294/#129/#130 へ委譲）。
- `ppqn.rs` の source-native skip 不変条件の変更
  （fixture は検証側であり、実不変違反が見つかった
  場合のみ例外的に `src/` 変更を許容）。

## Sources and Relations

- Issue #126（TIM-02）受入行「PPQN 異種の Import /
  Project Change / Save / Export / 再 Import」。
- `docs/specs/tim-02-smf-format01-import-export.md`
  （本 slice が検証を補完する上位 spec）。
- `docs/specs/tim-01-configurable-ppqn-project-command.md`
  （`SetProjectPpqn` の Exact/Round 契約）。
- `docs/designs/tim-02-smf-format01-core.md`、
  `docs/designs/configurable-ppqn-musical-time-architecture.md`。
- `crates/synvelis-project/tests/tim_02_smf.rs`・
  `tim_01_ppqn.rs`（既存鎖 fixture — 本 slice はその
  交差を埋める）。
