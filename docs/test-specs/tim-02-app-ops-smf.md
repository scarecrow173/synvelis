---
id: "TSPEC-034XmP8sXtVhPzDbAfJuNr"
type: "test-spec"
status: "proposed"
title: "TIM-02 SMF app ops — op 経路 fixture matrix"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/126"
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
    - "docs/specs/tim-02-app-ops-smf.md"
    - "docs/designs/tim-02-app-ops-smf.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/tim-02-app-ops-smf.md"
    - "docs/designs/tim-02-app-ops-smf.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# TIM-02 SMF app ops — op 経路 fixture matrix

## Purpose

`syn.project.import_smf`/`export_smf` op 配線・capabilities
`midi` 節・midi clip の engine compile 証拠を、
`SharedService`/`UiRuntime::dispatch_op` fixture
（`tests/tim02_smf_app.rs`、TIM-01 harness 踏襲）で決定的に
検証する test-spec。実 OS backend・実 port・実 GUI・L3 の
Pass を意味しない（NotRun — 外部 gate）。

## Feature

headless op 経路で SMF の import/export/往復が動作し、
typed error と capabilities の正直表示・MIDI clip の
engine 証拠が固定される。

## Rules

- Rule: `syn.project.import_smf` は file を `parse_smf` で
  解釈し `ImportMidiSmf` を session apply、`source_id` と
  format/division/tracks/controllers/bytes のサマリを返す。
- Rule: `syn.project.export_smf` は `source_id` +
  division/format option + path で SMF bytes を書き出し、
  `ConversionReport` 全 field を返す read-only op（undo
  履歴に乗らない）。
- Rule: typed error は malformed/unsupported/not found/
  IO/transport 中拒否を区別する。未知 `source_id` は
  `not_found`（division 0 guard の `unsupported` に
  吸収されない）。
- Rule: capabilities `midi` 節は SMF=`available`、backend=
  Windows implemented/WinMM+real_port not_run・macOS/
  Linux unsupported、SMPTE/Format2/MIDI2 を `unsupported`
  +reason で返す。
- Rule: midi_sources 由来 clip は audio pass を fault させず
  `SkippedPlayback::MidiClip` で skip され、`plan_midi_clips`
  → `compile_midi_comp` で engine frame の MIDI event 証拠
  を生成する。muted clip は証拠なし。node 配送は未配線。

## Examples

- `import_smf_mints_source_and_reports_summary`: Format0
  fixture を path 経由で import → `source_id` 採番・
  division/track 数が project に反映。
- `import_smf_typed_errors`: 不存在 path → IO error、
  malformed bytes → `details.smf_error`、division 0/
  Format2/SMPTE → `unsupported`。
- `import_smf_rejects_while_transport_running`: 再生中は
  mutation gate で拒否。
- `import_smf_undo_redo_round_trip` /
  `import_smf_survives_save_reload`: undo/redo・save/reload
  で source + provenance document が保持される。
- `export_smf_writes_file_and_reports_conversion`: division
  変換で `exact_count`/`rounded_count`/`max_error` が返り、
  出力 file が re-parse 可能。
- `export_smf_typed_errors`: 未知 `source_id` → `not_found`、
  division 0 → `unsupported`、書込不可 path → IO error。
- `export_smf_is_read_only`: export 前後で revision/undo
  履歴が不変。
- `import_export_reimport_round_trip`: export した file を再
  import して document 意味が保存される。
- `capabilities_reports_midi_section_honestly`: `midi` 節の
  各 state/reason を assert。
- `midi_clip_compiles_to_engine_frame_evidence`: midi clip を
  含む project で `midi_clip_evidence()` が NoteOn/Off の
  engine frame event を返す。
- `muted_midi_clip_produces_no_evidence`: muted clip は
  証拠を生成しない。

## Guarantees

- 全 fixture が deterministic（実 SMF bytes 固定、同一入力
  →同一 receipt/error/証拠）。
- typed error が失敗原因を区別し、成功に偽装しない。
- 証拠は compile 失敗でも typed 文字列で返り panic しない。

## Non-goals

- plugin node への MIDI 配送・RT callback・実 OS backend・
  仮想 WinMM Port/RF #630 L3・AL f64 oracle・GUI 経路の検証。
- SMPTE/Format2/MIDI2 の受理。
- `routing_engine` plan merge（`MidiClipPlan` 消費）の検証。

## Risk

- op 配線を怠ると SMF 機能が headless 製品面から到達不能
  のまま残り、Issue #126 の headless 残件が閉じない。
- source 存在確認を division 解決より後にすると未知
  `source_id` が `unsupported` に誤分類され、typed error
  契約が破られる。
- node 配送未配線を明記しないと compile 証拠の成功を実機
  発音と誤認する（本 test-spec は evidence と併せて
  「node 配送未配線」を明示する）。
