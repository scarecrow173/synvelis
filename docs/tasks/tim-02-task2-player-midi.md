---
id: "TASK-126tim02playerMidiRt01"
type: "task"
status: "in-progress"
title: "TIM-02 Task2 player MIDI RT/Offline 結合 slice"
created: "2026-11-10"
updated: "2026-11-10"
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
  implements:
    - "docs/plans/tim-02-player-midi-rt-offline.md"
  implemented-by: []
  depends-on:
    - "docs/plans/tim-02-player-midi-rt-offline.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-02-player-midi-rt-fixtures.md"
  references: []
  defers: []
  deferred-by: []
---

# TIM-02 Task2 player MIDI RT/Offline 結合 slice

## Work

spec `tim-02-player-midi-rt-offline` /
design `tim-02-player-midi-rt-offline` /
plan `tim-02-player-midi-rt-offline` /
test-spec `tim-02-player-midi-rt-fixtures` に従い、
`synvelis-player` に MIDI event binding/playback の
RT/Offline 結合を実装する。

- `src/midi.rs`: `MidiPlaybackPlan` → engine frame 位置の
  `MidiEvent` lowering（channel 解決・`AllNotesOff` → CC123）。
- `ParamDispatch::dispatch_midi`: MIDI event 列の
  `TransportPlan<EventKind>` install。
- `HeadlessDriver`: `set_loop`/`clear_loop`/`panic`/
  `resume_attached_transport`。
- `tests/tim02_midi_transport.rs` + `tests/tim02_midi_rt.rs`。

## Acceptance mapping

- Loop/Seek/Stop/Panic/Chase・発音残り 0 → driver transport
  method + upstream discontinuity（release_all/chase）で実現、
  fixture で event stream を検証。
- RT/Offline → playback drive と offline `render()` の parity
  fixture。実 MIDI Backend OS 別・AudioLens は NotRun。
