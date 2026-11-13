# SV19 — Musical Time 設計契約 受入証拠台帳 (synvelis#19)

Issue #19「Configurable PPQN（default 960）・
Resolution-independent Musical Time・Native MIDI Fidelity」
の受入条件 8 項目を、既存 fixture の被覆状況・先行 slice
の実測証拠・本 slice の未実行項目へ写像した台帳。**本
slice は docs/evidence のみであり、実装 crate の
`check`/`lint`/`test` は再実行していない** — fixture の
存在はソース確認で pin し、先行 slice の実測は SHA・
環境付きで参照する。本 slice で実行した検証は
`mise run fmt-check`・docs index/lint のみ（結果は
`docs/impl/ir/musical-time.md` に記録）。

`Pass`/`Pass*` は **先行 slice が記録した実測の参照** で
あり、本 slice で再測したことを意味しない。「fixture 設定
済み・本 slice 未実走」と「構成済みテストなし=NotRun」
を厳密に区別する（`docs/test-specs/musical-time.md`）。
headless fixture の成功は実 MIDI device・実 plugin・実 OS/
GUI の受入ではない（spec R-MT-08）。

## Environment

| Field | Value |
| --- | --- |
| Synvelis branch | `scarecrow173/sv19-musical-time`（task 指定ブランチ名は `docs/19-musical-time`） |
| Synvelis base SHA | `29693c8acfdfc43e2c886e07f5d07911f007aa8d`（+ merge `origin/main` は完了手順で実施・index 再生成） |
| OS（本 slice 実行環境） | Windows (win32) |
| Toolchain | docs/markdown のみ — `mise run fmt-check` が正規検証 |
| Fixture lane | headless `cargo test`（temp-dir・injected fault・fake plugin）。実機 GUI/Device/Plugin は対象外 |
| 先行実測の参照先 | TIM-01 (#125)・TIM-02 (#126) 各 slice の IR/ledger・CI 記録。個別 run 証拠は各 Owner Issue の gate に委譲 |

Status vocabulary（`POLICIES.md` §3 準拠）:

- `Pass` = 先行 slice で実測 pass が記録されている
  （参照 SHA/環境を明記）。
- `Pass*` = headless/persisted 層のみの pass（実機
  device/plugin/OS GUI を含まない）。
- `NotRun` = fixture が存在するが本 slice で実行していない
  （先行実測を参照する場合はその旨を明記）、または構成
  済みテストがない契約。
- `Unsupported` = capability が環境上利用不可、または
  明示的に対象外形式（SMF SMPTE/Format2/MIDI2）。
- `Deferred` = POLICIES.md §2 の別 Owner/将来 lane へ委譲
  （実 MIDI device・実 plugin・実 OS/GUI・Ruviform#189 等）。

## Case Ledger — Issue #19 受入条件 8 項目

| Case | 受入条件（Issue #19 転記） | 根拠 fixture（crate::test / test file） | Expected | Observed（先行実測/本 slice） | Status | 備考 |
| --- | --- | --- | --- | --- | --- | --- |
| SV19-C01 | 新規 Project が 960 を既定値とし、異なる PPQN を設定・保存・再読込できる。 | `crates/synvelis-project/src/tempo.rs`（`ProjectPpqn` 既定 960）・`crates/synvelis-project/tests/tim_01_ppqn.rs`（`ppqn_sweep_save_reload_undo_redo_matrix` — 1/480/960/1000/1920/32767・`ppqn_change_survives_save_reload_and_keeps_old_snapshot`・`ppqn_change_on_v5_snapshot_preserves_migrated_data`）・`crates/synvelis-app/tests/tim01_ppqn_preview.rs`（`project_new_ppqn_defaults_and_explicit_selection`・`project_new_rejects_bad_ppqn`）・`crates/synvelis-app/tests/tim01_ppqn_app.rs`（`set_ppqn_survives_save_reload`・`set_ppqn_applies_and_projects_ppqn`） | 新規 project の既定 PPQN=960。`1..=32767` の任意 PPQN を `SetProjectPpqn` で設定し、save→load で同値復元、undo/redo で往復。範囲外は typed reject。 | 先行実測: TIM-01 slice で上記 fixture が実装・pass（`tim_01_ppqn.rs` sweep matrix・app/preview/UI fixture）。本 slice 未実走。 | **NotRun**（fixture 設定済み・先行 slice で実装済み・本 slice 未実走） | `SetProjectPpqn{old_ppqn,new_ppqn,policy}` + revision CAS + old_ppqn domain CAS。app gate は `tim01_ppqn_app.rs` で fail-closed 確認。 |
| SV19-C02 | PPQN=480/960/1000/1920 等の Source が Native Tick/Division を失わず Import される。 | `crates/synvelis-project/tests/tim_02_smf.rs`（`smf_import_creates_source_with_native_division_and_projection`・`smf_format0_parse_preserves_full_event_stream`・`smf_format1_parse_preserves_tracks_and_channels`・`smf_running_status_is_parsed_but_never_emitted`・`smf_document_survives_save_load_and_schema_is_v10`・`smf_backed_source_rejects_all_note_edits`・`smf_detach_clip_source_creates_editable_copy_without_provenance`）・`crates/synvelis-project/tests/tim_02_ppqn_smf.rs`（`ppqn_exact_change_preserves_smf_chain_end_to_end`・`ppqn_round_change_still_preserves_smf_source_native_data`） | SMF Format0/1・PPQ division の import で `MidiSource.division`・`MidiNote` native `MidiTick`・`SmfDocument`（event order/track/channel/tempo/time sig/sysex/provenance/raw bytes）が保存される。native division は project PPQN と独立。 | 先行実測: TIM-02 slice で上記 fixture が実装・pass。`SmfDocument` が `project.smf_documents` に provenance として永続化。immutable guard で provenance 破壊を防止。本 slice 未実走。 | **NotRun**（fixture 設定済み・先行 slice で実装済み・本 slice 未実走） | 実 SMF ファイル（外部 DAW 由来）ではなく headless 合成 fixture。実ファイル import の E2E は TIM-02/実機 lane の別 gate。 |
| SV19-C03 | Beat の 1/7 など 960 の整数 Tick に乗らない位置を正確な正本として保持する（必要範囲を検証）。 | `crates/synvelis-project/tests/tim_01_ppqn.rs`（`ppqn_denominator_boundary_is_explicit` — tick 1 @960 = 1/960 beat、@32767 で Exact reject・Round Floor 34/Ceil 35）・`crates/synvelis-project/src/control.rs`（`BeatPoint` num/den・den≠0 不変条件）・`crates/synvelis-project/tests/tim_01_ppqn.rs`（`seeded_project` の automation lane — `BeatPoint::new(4,3)` 非整数 beat を有理数格納、PPQN 変更で bit-identical）・ruviform `BeatPosition`/`TickPosition`（有理数 beat 正本） | 整数 grid tick に乗らない有理数 beat 位置が正確に保持され、整数 tick への暗黙量子化が起きない。分母上限・overflow は typed error。 | 先行実測: `ppqn_denominator_boundary_is_explicit`・`BeatPoint` fixture で「grid 非表現な有理数位置の明示処理」を被覆。**「1/7 beat」専用 fixture は未構成** — 同値の性質（分母が PPQN と互いに素な位置）は denominator boundary fixture が被覆するが、1/7 そのものの pin はない。 | **NotRun**（性質被覆は fixture 設定済み・先行実測 Pass 参照。1/7 専用 fixture は構成済みテストなし → Ruviform#189/TIM Owner の別 gate） | 正本は ruviform `BeatPosition`（PPQN 非依存有理数）。Synvelis は `BeatPoint`/`TickPosition` 経由で消費。generic rational time の完全 pin は ruviform#189。 |
| SV19-C04 | Project PPQN 変更前後の音楽的位置と Raw MIDI Timestamp が一致し、明示 Quantize なしに移動しない。 | `crates/synvelis-project/tests/tim_01_ppqn.rs`（`ppqn_change_preserves_beat_positions_and_reprojects_ticks`・`ppqn_change_preserves_absolute_media_and_source_native_data`・`ppqn_change_undo_redo_round_trip`・`ppqn_change_handles_same_tick_sites`・`ppqn_exact_policy_rejects_inexact_positions`・`ppqn_round_policy_applies_declared_rounding`・`preview_ppqn_reports_inexact_punch_marks`）・`crates/synvelis-project/tests/tim_02_ppqn_smf.rs`（`ppqn_exact_change_preserves_smf_chain_end_to_end`・`ppqn_round_change_still_preserves_smf_source_native_data`） | PPQN 変更は musical site を exact beat 経由で再投影し、source-native MIDI tick/division/raw bytes・絶対 media anchor は不変。`Exact` は非正確を reject、`Round` は宣言 mode で quantize。960→1000→960 往復で musical position 一致。 | 先行実測: TIM-01/TIM-02 slice で上記 fixture が実装・pass。punch mark の silent retime は PR #385 で修正済み（preview が inexact punch mark を報告）。本 slice 未実走。 | **NotRun**（fixture 設定済み・先行 slice で実装済み・本 slice 未実走） | 「Raw MIDI Timestamp」は source-native `MidiTick`/`SmfDocument` event tick を指し、project PPQN 変更で不変。device timestamp も別ドメインで不変。 |
| SV19-C05 | 異なる PPQN の Tick を誤って比較・加算できず、明示変換を要する。 | `crates/synvelis-project/src/time.rs`（`MusicalTick`/`MidiTick`/`MediaTimeValue`/`MediaDuration` 別 newtype・`ProjectTime` enum）・`crates/synvelis-project/src/midi.rs`（`MidiDivision`/`MidiNote` が native `MidiTick`）・ruviform `TickPosition`（`Ppqn` 保持）・`crates/synvelis-project/src/ppqn.rs`（`to_beat`/`from_beat` 明示経路）・`crates/synvelis-project/src/tempo.rs`（`TickTimeMapping::tick_to_media` checked） | `MusicalTick`・`MidiTick`・media/device frame が別型で、domain を跨ぐ演算は明示関数のみ。異 PPQN の tick 比較・加算は compile-time に排除（`TickPosition` が `Ppqn` を保持）。 | 先行実測: 型レベル契約として `time.rs`/`midi.rs`/ruviform `TickPosition` で実装済み。`checked_*` 演算のみ使用（unchecked `Tick` `Add`/`Sub` は lint 対象）。本 slice 未実走。 | **NotRun**（型境界契約は実装済み・fixture は変換経路の正しさを pin。本 slice 未実走） | 「誤って比較・加算できない」は型設計（newtype 分離 + `Ppqn` 保持）で担保。専用の「誤用を試みる」negative fixture は構成されていない（型が許さないため）。 |
| SV19-C06 | Tempo Change / Loop / Negative Pre-roll / MIDI Off / Playback/Offline で結果が定義済み。 | `crates/synvelis-project/src/tempo.rs`（`TickTimeMapping`・tempo map beat 基準）・`crates/synvelis-player/tests/tim02_midi_playback.rs`（`tim02_loop_replay_releases_and_refires`・`tim02_seek_releases_and_chases`・`tim02_panic_releases_hanging_voice_and_resume`・`tim02_rt_offline_parity`・`tim02_same_tick_off_precedes_ons`・`tim02_controller_chase_is_channel_aware`・`tim02_native_tick_maps_to_engine_frames`・`tim02_compile_lowers_plan_to_wire_events`・`tim02_driver_delivers_midi_at_absolute_frames`・`tim02_smf_channel_provenance`）・`crates/synvelis-recording/tests/tim_02_midi_phrase.rs`（`smf_import_feeds_phrase_plan_with_zero_hanging_notes` — `assert_no_hanging_notes` helper で発音残り 0 を検査）・`crates/synvelis-project/tests/tim_02_smf.rs`（`smf_phrase_boundary_virtual_off_and_panic`・`smf_controller_events_feed_channel_aware_chase`）・`crates/synvelis-project/tests/tim_01_ppqn.rs`（`rt_ppqn_beat_to_frame_rounding_reproduces`） | tempo change/ramp・negative preroll・loop/seek/stop/panic/chase・MIDI Off・playback/offline parity の結果が headless fixture で決定的。beat→media/frame は exact rational、失敗は `Error::Overflow`。panic/end 後に sounding note を残さない。 | 先行実測: TIM-02 player/recording slice で上記 fixture が実装・pass（loop replay・seek chase・panic release・rt/offline parity・zero hanging notes）。本 slice 未実走。 | **NotRun**（fixture 設定済み・先行 slice で実装済み・本 slice 未実走） | 実 plugin transport 精度・実 device 再生は TIM-03 (#127)/synvelis#253 の別 gate。headless `HeadlessDriver`+fake plugin の結果は contract 検証であり実機証明ではない。 |
| SV19-C07 | 旧 960 Project の Save/Reload、Plugin Transport、Automation、Recording VS と互換移行ができる。 | `crates/synvelis-project/tests/tim_01_ppqn.rs`（`ppqn_change_survives_save_reload_and_keeps_old_snapshot`・`ppqn_change_on_v5_snapshot_preserves_migrated_data`）・`crates/synvelis-project/tests/alib02_v5_real.rs`（v5 snapshot migration・opaque plugin-state 保存）・`crates/synvelis-project/tests/monitoring_persist.rs`（v11→v12）・`crates/synvelis-project/src/control.rs`（`BeatPoint` 有理数 beat）・`crates/synvelis-recording/tests/tim_02_midi_phrase.rs`・`docs/specs/tim-04-plugin-state-snapshot-consistency.md` | 旧 fixed-960 snapshot が versioned migration で load でき旧 byte を保持。automation `BeatPoint`・recording VS・plugin state が PPQN/schema 変更で保存される。`SCHEMA_VERSION=13`・`migration.rs` versioned step。 | 先行実測: v5 snapshot migration・`BeatPoint`・plugin-state opaque 保存・recording VS fixture が先行 slice で実装・pass。本 slice 未実走。 | **NotRun**（fixture 設定済み・先行 slice で実装済み・本 slice 未実走） | plugin transport の実機精度・plugin state snapshot 品質は TIM-03 (#127)/TIM-04 (#128) の別 gate。本 slice は互換契約の参照のみ。 |
| SV19-C08 | 時間計算の Overflow、分母上限、RT 処理量と丸め誤差が検証できる。 | `crates/synvelis-project/tests/tim_01_ppqn.rs`（`ppqn_change_rejects_tick_overflow`・`ppqn_out_of_range_rejected`・`ppqn_denominator_boundary_is_explicit`・`preview_ppqn_reports_overflow_sites`・`ppqn_change_handles_long_arrangements`・`rt_ppqn_beat_to_frame_rounding_reproduces`）・`crates/synvelis-project/tests/tim_02_smf.rs`（`smf_tick_accumulation_overflow_is_typed`・`smf_export_overflow_is_typed`・`smf_malformed_inputs_are_typed`・`smf_unsupported_inputs_are_typed_never_success`・`smf_export_ppqn_down_reports_rounding_and_max_error` — exact_count=3/rounded_count=2/max_error=1/2・`smf_export_ppqn_up_conversion_is_exact`・`smf_export_rejects_invalid_targets`・`smf_unclosed_note_is_typed_validation_error`・`smf_validate_references_rejects_divergence_and_orphans`）・`crates/synvelis-player/tests/tim02_midi_playback.rs`（`tim02_rt_offline_parity`・rt fixture） | overflow・分母上限・RT allocation・丸め誤差が versioned fixture で pin。`ConversionReport.max_error` は `SmfRational` で厳密。unchecked `Tick` 演算不使用。 | 先行実測: overflow/denominator/rounding/RT fixture が TIM-01/02 slice で実装・pass。`max_error=SmfRational(1,2)`（960→480 奇数 tick の half-up）等の具体値が pin 済み。本 slice 未実走。 | **NotRun**（fixture 設定済み・先行 slice で実装済み・本 slice 未実走） | `smf_export_ppqn_up_conversion_is_exact` は 480→960 の整数倍変換が exact であることを pin。RT 処理量は `rt_` fixture + `StatsAlloc` で別 lane 計測。 |

## 丸め誤差・Overflow 証拠（R-MT-04/08 の定量的根拠）

| 経路 | 変換 | 誤差表現 | 上限/実測 |
| --- | --- | --- | --- |
| SMF export tick 変換 | `round_half_up(tick * to_div / from_div)`（i128 厳密） | `ConversionReport.max_error: SmfRational` | 960→480 奇数 tick で `max_error = 1/2`（`smf_export_ppqn_down_reports_rounding_and_max_error`: event_count=5, exact=3, rounded=2） |
| PPQN reprojection | `TickPosition::to_beat`→`from_beat`（exact rational beat） | `Exact`=非正確 reject / `Round(Rounding)`=宣言 mode | `ppqn_denominator_boundary_is_explicit`: 1/960 beat @32767 で Exact reject・Round Floor 34/Ceil 35 |
| beat→media/frame | `TickTimeMapping::tick_to_media`（checked exact rational） | `Error::Overflow` 顕在化 | `rt_ppqn_beat_to_frame_rounding_reproduces`・unchecked `Tick` 不使用 |
| automation `BeatPoint` | 有理数 beat（num/den）格納 | tempo 変更で再 quantize なし | den≠0 不変条件（`control.rs`） |

## Fixture 被覆マトリクス（source pin — 実走は先行 slice 参照）

| 領域 | test file | 主な被覆 |
| --- | --- | --- |
| PPQN op・reprojection・policy | `crates/synvelis-project/tests/tim_01_ppqn.rs` | sweep・save/reload/undo・CAS・overflow・denominator・same-tick・punch mark・v5 migration・RT rounding |
| SMF codec・provenance・export | `crates/synvelis-project/tests/tim_02_smf.rs` | Format0/1 parse・native division・immutable guard・detach・export report・event order・typed error・golden round-trip |
| PPQN×SMF cross-golden | `crates/synvelis-project/tests/tim_02_ppqn_smf.rs` | `ppqn_exact_change_preserves_smf_chain_end_to_end`・`ppqn_round_change_still_preserves_smf_source_native_data` |
| player MIDI RT/offline | `crates/synvelis-player/tests/tim02_midi_playback.rs` | compile lowering・absolute frame・loop/seek/panic・channel-aware chase・rt/offline parity・SMF channel provenance |
| recording phrase/note safety | `crates/synvelis-recording/tests/tim_02_midi_phrase.rs` | SMF→phrase plan・zero hanging notes・export→reimport round-trip |
| app gate・preview・UI | `crates/synvelis-app/tests/tim01_ppqn_{app,preview,ui}.rs` | transport/record gate・preview non-destructive・snap/grid policy・UI surface |

## Deferred / NotRun / Unsupported の明示

| 項目 | 分類 | 委譲先 |
| --- | --- | --- |
| 実 MIDI device backend 検証 | **Deferred** | synvelis#253 / ruviform#253（別 gate） |
| 実 plugin transport 精度・state snapshot | **Deferred** | TIM-03 (#127) / TIM-04 (#128) / ruviform#307 |
| SMF SMPTE division・Format 2・MIDI 2/MPE | **Unsupported/Deferred** | TIM-F01 (#129) / TIM-F02 (#130) |
| 「1/7 beat」専用 fixture | **NotRun**（性質は denominator boundary で被覆） | Ruviform#189 / TIM Owner |
| 実 SMF ファイル（外部 DAW 由来）の E2E | **NotRun** | TIM-02 / 実機 lane |
| merge 後対象 SHA の CI 証拠 | **Deferred** | Coordinator（統合受入） |
| 実 OS/GUI（macOS/Linux 含む） | **Deferred** | POLICIES.md §2 別 lane |

## 本 slice の検証（実行済み）

- `mise run fmt-check` — docs/設計スライスの正規検証。
  結果は `docs/impl/ir/musical-time.md` に記録。
- docs index 再生成（`doc-maintenance` `index apply`）・
  `doc_status.js lint` — touched dir の整合。
- 実装 crate の `check`/`lint`/`test` は **対象外**
  （docs/evidence slice — タスク指定の正規検証は
  `mise run fmt-check`）。
