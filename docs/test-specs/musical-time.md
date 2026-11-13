---
id: "TSPEC-jioP2kLGYJUweo578lGo0V"
type: "test-spec"
status: "approved"
title: "Musical Time — 受入証拠テスト仕様（synvelis#19）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/19"
  changes:
    added:
      - type: "file"
        path: "docs/test-specs/musical-time.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/musical-time.md"
    - "docs/designs/musical-time-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/tim-01-configurable-ppqn.md"
    - "docs/test-specs/tim-01-ppqn-config.md"
    - "docs/test-specs/tim-01-app-ppqn.md"
    - "docs/test-specs/tim-01-ppqn-preview.md"
    - "docs/test-specs/tim-01-ppqn-ui.md"
    - "docs/test-specs/tim-02-smf-format01-fixtures.md"
    - "docs/test-specs/tim-02-ppqn-smf-cross-golden.md"
    - "docs/test-specs/tim-02-player-midi-rt-fixtures.md"
    - "docs/test-specs/tim-04-plugin-state-snapshot.md"
    - "docs/test-specs/tim-03-session-driver-wiring.md"
    - "evidence/sv19/case-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/musical-time-design.md"
    - "docs/specs/musical-time.md"
  derived-by: []
  verifies:
    - "docs/specs/musical-time.md"
  verified-by: []
  references:
    - "evidence/sv19/case-ledger.md"
    - "https://github.com/scarecrow173/synvelis/issues/125"
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/127"
    - "https://github.com/scarecrow173/synvelis/issues/128"
    - "https://github.com/scarecrow173/synvelis/issues/129"
    - "https://github.com/scarecrow173/synvelis/issues/130"
    - "https://github.com/scarecrow173/synvelis/issues/253"
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/253"
    - "https://github.com/scarecrow173/ruviform/issues/307"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/127"
    - "https://github.com/scarecrow173/synvelis/issues/128"
    - "https://github.com/scarecrow173/synvelis/issues/129"
    - "https://github.com/scarecrow173/synvelis/issues/130"
    - "https://github.com/scarecrow173/synvelis/issues/253"
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/253"
    - "https://github.com/scarecrow173/ruviform/issues/307"
    - "evidence/sv19/case-ledger.md"
  deferred-by: []
---

# Musical Time — 受入証拠テスト仕様（synvelis#19）

## 目的

Issue #19 の受入条件 8 項目を検証可能な Rule/Example へ
分解し、各 Rule に **独立 oracle**（実装内部に依存しない
判定基準）と根拠 fixture を割り当てる。本 slice は
docs/evidence のみであり、fixture の実走は先行 slice の
記録を参照し、本 slice では `mise run fmt-check` のみを
実行する。

## Feature: Project PPQN（SV19-C01 / R-MT-01）

### Rule MT-F01-1: 新規 project の既定 PPQN は 960

- **Oracle**: `Project::new` / project 生成経路で
  `project.ppqn == 960`（`TempoMap`/`ProjectPpqn` 既定値）。
- **Fixture**: `crates/synvelis-project/src/tempo.rs` 既定値
  ・`crates/synvelis-app/tests/tim01_ppqn_preview.rs`
  （`project_new_ppqn_defaults_and_explicit_selection`）。
- **Status**: fixture 設定済み・先行 slice で実装済み・
  本 slice 未実走（NotRun 参照）。

### Rule MT-F01-2: 異 PPQN の設定・保存・再読込

- **Oracle**: `SetProjectPpqn` で変更した PPQN が save→
  load 後も同値で復元され、undo/redo で往復する。
- **Fixture**: `crates/synvelis-project/tests/tim_01_ppqn.rs`
  （`ppqn_sweep_save_reload_undo_redo_matrix` —
  1/480/960/1000/1920/32767 の 6 値）・
  `crates/synvelis-app/tests/tim01_ppqn_app.rs`
  （`set_ppqn_survives_save_reload`）。

### Rule MT-F01-3: PPQN 範囲外は typed reject

- **Oracle**: `new_ppqn ∉ 1..=32767` で `Error::Validation`、
  project 無変更。
- **Fixture**: `tim_01_ppqn.rs`（`ppqn_out_of_range_rejected`・
  `project_new_rejects_bad_ppqn`）。

### Rule MT-F01-4: app 層 gate は fail-closed

- **Oracle**: transport playing・open recording
  transaction・record_enabled のいずれかで
  `syn.project.set_ppqn` が `invalid_params` を返し
  project 無変更。
- **Fixture**: `tim01_ppqn_app.rs`（前提条件違反系）。

## Feature: Source-Native MIDI（SV19-C02 / R-MT-03）

### Rule MT-F02-1: SMF import は native division/tick を保存

- **Oracle**: `parse_smf`→`ImportMidiSmf` で
  `MidiSource.division == doc.division`（480/960/1000/1920）、
  `MidiNote.start/len` が native `MidiTick`、event order・
  channel・tempo/time signature・sysex・provenance が
  `SmfDocument` に保持される。
- **Fixture**: `crates/synvelis-project/tests/tim_02_smf.rs`
  （`smf_import_creates_source_with_native_division_and_projection`）・
  `tim_02_ppqn_smf.rs`（native division matrix）。

### Rule MT-F02-2: PPQN 変更で source-native データは不変

- **Oracle**: `SetProjectPpqn` 前後で `midi_sources`・
  `smf_documents` が完全一致（native tick/division/raw
  bytes/event stream 不変）。
- **Fixture**: `tim_01_ppqn.rs`
  （`ppqn_change_preserves_absolute_media_and_source_native_data`）・
  `tim_02_ppqn_smf.rs`
  （`ppqn_exact_change_preserves_smf_chain_end_to_end`・
  `ppqn_round_change_still_preserves_smf_source_native_data`）。

### Rule MT-F02-3: source 由来 note の直接編集は拒否

- **Oracle**: provenance を持つ source への
  `AddMidiNote`/`RemoveMidiNote`/`SetMidiNoteVelocity`/
  `MoveMidiNote` が `Error::Validation`。明示 detach は
  編集可能 copy を作る。
- **Fixture**: `tim_02_smf.rs`（immutable guard 系）。

## Feature: Rational Beat 正本（SV19-C03 / R-MT-02）

### Rule MT-F03-1: 整数 tick に乗らない beat 位置を正確に保持

- **Oracle**: 1/7 beat・分母が PPQN と互いに素な有理数位置が
  `BeatPosition`/`BeatPoint`（num/den）で正確に表現され、
  整数 `MusicalTick` への暗黙量子化が起きない。
- **Fixture**: `tim_01_ppqn.rs`
  （`ppqn_denominator_boundary_is_explicit` — tick 1 @960 =
  1/960 beat、@32767 で Exact reject・Round Floor 34/Ceil 35）・
  `seeded_project` の automation lane（`BeatPoint::new(4,3)`
  = 非整数 beat を有理数 num/den で格納、PPQN 変更で
  bit-identical 保存）。`control.rs` の den≠0 不変条件。
- **注記**: 「1/7 beat」そのものの fixture は分母境界 fixture
  が同値の性質（grid 非表現可能な有理数位置の明示処理）を
  被覆する。専用 1/7 fixture は未構成 — ledger で NotRun
  相当として明記し、Ruviform#189 の generic rational time
  gate に委譲。

### Rule MT-F03-2: 分母上限・overflow は typed error

- **Oracle**: 変換の分母が表現上限を超える・tick 演算が
  overflow する場合 `Error::Overflow`/`Validation` で顕在化、
  unchecked `Tick` 演算を使わない。
- **Fixture**: `tim_01_ppqn.rs`（`ppqn_change_rejects_tick_overflow`）。

## Feature: PPQN 変更の非破壊 reprojection（SV19-C04 / R-MT-03,04）

### Rule MT-F04-1: musical site は beat 経由で再投影、往復一致

- **Oracle**: `SetProjectPpqn` 960→1000→960（Exact）で
  tempo/signature/clip anchor/comp range/punch mark の
  musical position が一致。非正確位置は Exact で reject、
  Round では宣言 mode で quantize。
- **Fixture**: `tim_01_ppqn.rs`
  （`ppqn_change_preserves_beat_positions_and_reprojects_ticks`・
  `ppqn_change_undo_redo_round_trip`・同 tick 複数 site）・
  punch mark fixture（`preview_ppqn_reports_inexact_punch_marks`）。

### Rule MT-F04-2: 明示 Quantize なしに位置が移動しない

- **Oracle**: PPQN 変更は quantize/stretch/raw retime ではなく、
  musical position の beat 値は policy=Exact で不変。
  `PpqnConversionPolicy` は `Exact` または宣言 `Round` のみ。
- **Fixture**: `ppqn.rs` 実装 + `tim_01_ppqn.rs` policy 系
  fixture。

### Rule MT-F04-3: SMF export の丸め誤差は report される

- **Oracle**: `export_smf` が `ConversionReport` に
  `exact_count`/`rounded_count`/`max_error`（`SmfRational`）/
  `unsupported_events`/`event_count` を記録。960→480 の
  奇数 tick で `max_error = 1/2`。
- **Fixture**: `tim_02_smf.rs`
  （`smf_export_ppqn_down_reports_rounding_and_max_error` —
  event_count=5, exact=3, rounded=2, max_error=1/2）。

## Feature: Tick 型安全（SV19-C05 / R-MT-02,04）

### Rule MT-F05-1: 異 PPQN/source の tick は型で区別

- **Oracle**: `MusicalTick`・`MidiTick`・`MediaTimeValue`・
  device frame が別 newtype で、domain を跨ぐ演算は
  `TickTimeMapping`/`to_beat`/`from_beat`/`tick_to_media`
  の明示関数のみ。`TickPosition` は `Ppqn` を保持。
- **Fixture**: `time.rs`/`midi.rs` 型定義・`tempo.rs`
  `TickTimeMapping`・ruviform `TickPosition`（型レベル契約、
  比較・加算の compile-time 排除）。

### Rule MT-F05-2: 変換失敗は checked error

- **Oracle**: beat→media/frame 変換の overflow は
  `Error::Overflow`。unchecked `Tick` `Add`/`Sub` は lint 対象。
- **Fixture**: `tim_01_ppqn.rs`（`rt_ppqn_beat_to_frame_rounding_reproduces`）。

## Feature: Tempo / Transport / Loop（SV19-C06 / R-MT-05）

### Rule MT-F06-1: tempo change/ramp・negative preroll の結果が定義済み

- **Oracle**: tempo map は beat 基準。tempo 変更・ramp・
  negative preroll で beat→media/frame 変換が exact rational、
  結果が決定的。
- **Fixture**: `tempo.rs` test・`tim_01_ppqn.rs`
  （`rt_ppqn_beat_to_frame_rounding_reproduces`）。

### Rule MT-F06-2: loop/seek/stop/panic/chase で note 安全

- **Oracle**: loop replay・seek・panic・resume で発音 note の
  重複・欠落なし、panic/end 後に sounding note を残さない
  （AllNotesOff/CC123 chase）。offline/RT parity。
- **Fixture**: `synvelis-player/tests/tim02_midi_playback.rs`
  （`tim02_loop_replay_releases_and_refires`・
  `tim02_seek_releases_and_chases`・
  `tim02_panic_releases_hanging_voice_and_resume`・
  `tim02_rt_offline_parity`・`tim02_same_tick_off_precedes_ons`）・
  `synvelis-recording/tests/tim_02_midi_phrase.rs`
  （`smf_import_feeds_phrase_plan_with_zero_hanging_notes` —
  `assert_no_hanging_notes` helper で発音残り 0 を検査）・
  `synvelis-project/tests/tim_02_smf.rs`
  （`smf_phrase_boundary_virtual_off_and_panic`・
  `smf_controller_events_feed_channel_aware_chase`）。

### Rule MT-F06-3: plugin transport は capability 分類

- **Oracle**: plugin 精度は `Exact`/`FrameAccurate`/
  `BlockOnly`/`DegradedWithConsent`/`Unsupported`/`Unknown`
  で分類。Synvelis が DAW policy、Ruviform が generic
  adapter を所有。実 plugin 検証は TIM-03 (#127) 別 gate。
- **Fixture**: `tim03_process_boundary`・`tim-03-session-driver-wiring`
  の contract。本 slice は分類契約のみ固定（実機 NotRun）。

## Feature: SMF Fidelity（SV19-C02,04,08 / R-MT-06）

### Rule MT-F07-1: Format 0/1・PPQ division の parse/export

- **Oracle**: `MThd`/`MTrk` 検証、format∈{0,1}、division
  PPQ（SMPTE bit→Unsupported、0→Malformed）。running status
  解決・varlen checked accumulate。
- **Fixture**: `tim_02_smf.rs`（parse/export・malformed/
  truncated/overflow typed error 系）。

### Rule MT-F07-2: Import→Export→Reimport golden round-trip

- **Oracle**: provenance document を export→reimport で
  document が一致（semantic round-trip）。同 tick event の
  NoteOff→NoteOn 順序 deterministic。
- **Fixture**: `tim_02_smf.rs`
  （`smf_export_same_tick_noteoff_precedes_noteon`・
  golden round-trip 系）・`tim_02_ppqn_smf.rs`（cross-golden）。

## Feature: Compatibility / Migration（SV19-C07 / R-MT-07）

### Rule MT-F08-1: 旧 fixed-960 snapshot の互換 load

- **Oracle**: 旧 schema snapshot が versioned migration で
  load でき、旧 byte 列を破壊しない。`SCHEMA_VERSION=13`、
  `migration.rs` の versioned step 規約。
- **Fixture**: `alib02_v5_real.rs`（v5 snapshot migration）・
  `monitoring_persist.rs`（v11→v12）・`tim_01_ppqn.rs`
  （v5 snapshot PPQN migration）。

### Rule MT-F08-2: automation/recording/plugin との互換

- **Oracle**: `BeatPoint` が非整数 beat（例: 4/3 beat）を
  有理数 num/den で格納し、PPQN 変更・tempo 変更で再
  quantize されず bit-identical に保存される。recording
  VS・plugin state が migration で保存される。
- **Fixture**: `tim_01_ppqn.rs`（`seeded_project` の
  `BeatPoint::new(4,3)` automation lane が PPQN sweep で
  bit-identical — `ppqn_change_preserves_absolute_media_and_source_native_data`）・
  `tim_02_midi_phrase.rs`・`tim-04-plugin-state-snapshot`。

## Feature: Safety / Evidence（SV19-C08 / R-MT-08）

### Rule MT-F09-1: overflow・分母上限・RT 処理量・丸め誤差が検証可能

- **Oracle**: 時間計算 overflow・分母上限・RT allocation・
  丸め誤差（`max_error` 上限）が versioned fixture で
  pin される。`evidence/sv19/case-ledger.md` が Case→
  fixture→Expected/Observed→Status を保持。
- **Fixture**: `tim_01_ppqn.rs`・`tim_02_smf.rs`・
  `tim02_midi_playback.rs`（rt fixture）の overflow/
  denominator/rounding/RT 系。

## Independent Oracle 原則

各 Rule の oracle は **実装内部ではなく観測可能な契約**
（保存値・返り値・typed error・report 値・byte 列）に基づく。
「実装がこう動くはず」ではなく「この入力でこの出力/この
拒否になる」を判定基準とし、fixture が oracle を独立に
検証できる構造とする。

## Deferred / NotRun の扱い

- 実 MIDI device backend・実 plugin・実 OS/GUI は
  `Deferred`/`NotRun` — headless fixture の Pass を実機
  受入に換算しない（spec R-MT-08）。
- 「1/7 beat」専用 fixture・SMF SMPTE/Format2/MIDI2・
  plugin state snapshot 品質は TIM-F01/F02・TIM-04・
  Ruviform#189 の別 gate。
