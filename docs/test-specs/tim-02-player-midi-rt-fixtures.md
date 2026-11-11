---
id: "TESTSPEC-126tim02playerMidiRt01"
type: "test-spec"
status: "approved"
title: "TIM-02 player MIDI RT/Offline 結合 fixtures"
created: "2026-11-10"
updated: "2026-11-11"
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
    - "docs/specs/tim-02-player-midi-rt-offline.md"
    - "docs/designs/tim-02-player-midi-rt-offline.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/tim-02-smf-format01-fixtures.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/tim-02-player-midi-rt-offline.md"
    - "docs/designs/tim-02-player-midi-rt-offline.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# TIM-02 player MIDI RT/Offline 結合 fixtures

spec `tim-02-player-midi-rt-offline` の acceptance を
`synvelis-player/tests/tim02_*.rs` で検証する test-spec。

## Feature: plan → wire event lowering (`compile_midi_events`)

- Rule: `MidiPlaybackPlan` の全順序が engine frame 位置の
  `MidiEvent` 列に保存される。`at` 昇順・同 frame は plan 順
  （NoteOff → AllNotesOff → ControllerChase → NoteOn の
  action rank）。
- Rule: `MidiPlanAction` → `MidiKind` 写像は velocity
  `MidiValue::from_u7`、pitch bend `MidiValue::bend_from_u14`、
  NoteOff release velocity 0、`group = MidiGroup::ZERO`。
- Rule: note channel は SMF provenance（`paired_notes` と
  NoteId 順の index 対応）で解決し、document 非所持は
  `fallback_channel`。`ControllerChase` は自身の channel を使う。
- Rule: `AllNotesOff` は plan 内 note channel 集合へ
  `ControlChange{controller:123, value:0}` を発する（≤16 件）。
- Example: division 480 SMF（ch0/ch1 の note + CC64 chase）
  → PPQN 960 project で ×2 正確写像、ch0/ch1 の event が
  それぞれの channel で配送される。

## Feature: transport 結合（driver + PluginNode）

- Rule: `dispatch_midi` で install した plan の event が
  絶対 engine frame で endpoint に配送される（`Play` は
  driver activation が anchor 済み）。
- Rule: `seek` は discontinuity を起こし、発音中 note は
  offset-0 NoteOff で release され、seek 先 position を跨ぐ
  note は plan prefix chase で再発火される。skip 区間の
  event は発火しない。
- Rule: `set_loop` の wrap は `LoopOccurrence` bump で
  discontinuity となり、loop 内 event は occurrence ごとに
  1 回ずつ配送され、発音中 note は wrap で release→chase。
- Rule: `panic`（`TransportCommand::Stop`）は発音中 note を
  NoteOff で解放し、status Stopped のため再発火も plan
  event も発生しない = 発音残り 0。`resume_attached_transport`
  で position==cursor に再 anchor して再開する。
- Rule: hanging voice（発音中のまま残る voice=0 の note）が
  seek/loop/panic の各 discontinuity で必ず NoteOff を受け取る
  — event stream 上で全 NoteOn に対応する NoteOff（または
  release/chase 経路の NoteOn 再構成）を確認する。
- Rule: RT/Offline parity — playback-style `RtRuntime` drive と
  offline `render()` pipeline で同一 `MidiEvent` stream
  （abs frame・kind・channel）が配送される。

## Feature: RT contract

- Rule: `rt_` fixture は MIDI event 配送を含む `rt.process`
  window で `StatsAlloc` による heap alloc/dealloc 0 を実測
  する（`compile_midi_events`/`dispatch_midi` は non-RT で
  window 外）。

## NotRun / Unsupported（明示）

- 実 MIDI Backend OS 別・実 Device/Plugin — Ruviform #253
  の外部 gate。headless fixture の成功を実機成功にしない。
- AudioLens Native f64 非回帰 — #294 別 Gate。
- SMPTE/Format2/MIDI2/MPE — 未対応を成功にしない。
- panic-and-continue — upstream に専用 command なし、
  `Stop` 経由の 2 段操作のみ。
