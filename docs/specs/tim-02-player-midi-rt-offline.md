---
id: "SPEC-126tim02playerMidiRt01"
type: "spec"
status: "implemented"
title: "TIM-02 player MIDI RT/Offline 結合 slice (synvelis#126)"
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
    - "docs/specs/tim-02-smf-format01-import-export.md"
    - "docs/designs/tim-02-smf-format01-core.md"
    - "docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/126"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/125"
    - "https://github.com/scarecrow173/synvelis/issues/127"
    - "https://github.com/scarecrow173/ruviform/issues/253"
  defers:
    - "実 MIDI Backend OS 別・実 Device/Plugin 検証（Ruviform #253 の責務）"
    - "SMPTE Division・MIDI 2.0/MPE（#129/#130 future・明示 Unsupported）"
    - "AudioLens Native f64 非回帰（#294 別 Gate）"
    - "MidiNote への channel 追加（synvelis-project 側 model 変更・別 slice）"
  deferred-by: []
---

# TIM-02 player MIDI RT/Offline 結合 slice (synvelis#126)

## What Should Be Built

Issue #126 のうち `synvelis-player` が担う RT/Offline 結合 slice。
`synvelis-project` の `compile_midi_comp` が生成する
`MidiPlaybackPlan`（MIDI Take Phrase）を、attached plugin node の
`TransportPlan<EventKind>` へ engine frame 位置で lowering し、
Loop/Seek/Stop/Panic/Chase の transport 意味論のもとで
`MidiEvent`/`EventList` として配送する。配送・ledger・chase・
discontinuity の実装は Ruviform (`ruviform-media-core` /
`ruviform-plugin-core` / `ruviform-audio-plugin-bridge`) が所有し、
本 slice は project plan → wire event の lowering と driver/dispatch
結合のみを行う。

## Acceptance（本 slice の対象行）

- Format0/1、複数 Track/Channel、同時刻 NoteOff→On、
  Loop/Seek/Stop、Panic/Chase、MIDI Take Phrase で発音残り 0
  — **player 側の Loop/Seek/Stop/Panic/Chase・発音残り 0 の
  RT 結合部分**。Format0/1 parser・同時刻 NoteOff→On は #225
  実装済みのため回帰確認のみ。
- RT/Offline headless 部分: playback-style `RtRuntime` drive と
  offline `render()` pipeline で同一 event stream を配送する。
  実 MIDI Backend OS 別・AudioLens Native f64 は NotRun
  （外部 gate、Pass に繰り上げない）。

## Out of Scope

- `crates/synvelis-project/`・`crates/synvelis-recording/` の変更
  （既存 API を読むだけ）。
- Ruviform の Generic MIDI Event/OS Backend/VST3 翻訳・Time
  Conversion の複製。`MidiNote` への channel 追加など project
  model 変更。
- MIDI clip（`MidiSource` clip）の graph lowering — audio graph は
  `PortFormat::Audio` のみで、MIDI phrase → instrument node の
  routing は別設計。
- SMPTE/Format2/MIDI2/MPE — 未対応を成功にしない。
- Controller state の seek 時 chase — upstream
  `chase_span_events` が note のみを対象とし controller chase を
  deferred とする契約のため、本 slice でも複製しない
  （plan の `ControllerChase` event は region 境界で通常配送
  される）。
