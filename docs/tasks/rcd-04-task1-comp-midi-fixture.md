---
id: "TASK-RCD04Fx1kE5tY8uI3oP6aS9"
type: "task"
status: "done"
title: "RCD-04 Task1 Comp Transition / MIDI Phrase crate fixture slice"
created: "2026-11-10"
updated: "2026-11-10"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/rcd-04-comp-midi-phrase-fixture.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-04-comp-midi-phrase-fixture.md"
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
    - "docs/test-specs/rcd-04-comp-midi-phrase-fixture.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-04 Task1 Comp Transition / MIDI Phrase crate fixture slice

## Work

spec `rcd-04-comp-transition-midi-phrase-fixture` /
design `rcd-04-comp-transition-midi-phrase-fixture` /
plan `rcd-04-comp-midi-phrase-fixture` に従い、
`synvelis-project` と `synvelis-recording` の crate 層
fixture slice を実装する。

1. `synvelis-project/src/id.rs`: `MidiCompRegionId`
   （`"midi-comp-region"`）を `define_id!` で追加。
2. `synvelis-project/src/midi_comp.rs`（新規）:
   `MidiPhraseRegion`/`MidiCompSelection`/`MidiControllerKind`/
   `MidiControllerEvent`/`MidiControllerLane(s)`/
   `MidiPlanEvent(Kind)`/`MidiRegionReport`/
   `MidiPlaybackPlan` と `compile_midi_comp` を実装。
   境界安全性（virtual off・off→on 順・chase snapshot・
   `AllNotesOff`）と native tick 正確写像（剰余 != 0 は
   error）を plan 構成で保証する。
3. `synvelis-project/src/lib.rs`: `midi_comp` module 公開 +
   re-export。
4. `synvelis-project/tests/rcd_04_comp_cas.rs`（新規）:
   Comp CAS/atomicity/undo/redo/save-load/serialized diff/
   provenance/immutability fixture。
5. `synvelis-project/tests/rcd_04_midi_phrase.rs`（新規）:
   MIDI phrase 境界 + stable ID fixture。
6. `synvelis-recording/tests/rcd_04_take_comp_pcm.rs`
   （新規）: record→commit→comp 構築・CAS・offline PCM
   renderer・RT↔Offline 差記録・media byte 不変。
7. `synvelis-recording/tests/rcd_04_recovery_partial.rs`
   （新規）: partial→Hold→Adopt retry・冪等・fail-closed・
   Discard probe 規則。
8. `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
   `build` で検証し `docs/impl/ir/` に implementation
   record を残す。

## Done When

- [x] test-spec `rcd-04-comp-midi-phrase-fixture` の F1–F5 が
  全て実測 Pass（`rcd_04_comp_cas` 5・`rcd_04_midi_phrase` 10・
  `rcd_04_recovery_partial` 4・`rcd_04_take_comp_pcm` 1、
  計 20 test 全て Pass）
- [x] `synvelis-app`/`synvelis-workspace`/`synvelis-player`/
  Ruviform 側に diff がない
- [x] impl record
  `docs/impl/ir/rcd-04-comp-transition-midi-phrase-crate-fixture-slice.md`
  に pass/fail/NotRun/Unsupported と実行した検証コマンド・
  exit status を正確に記録

## Verification

- [x] `mise run fmt-check` — Pass
- [x] `mise run check` — Pass
- [x] `mise run lint` — Pass（warning 0）
- [x] `mise run test` — Pass（workspace 全 test、exit 0）
- [x] `mise run test-rt` — Pass
- [x] `mise run build` — Pass
- [ ] 実 OS/Device/GUI・実 RT engine PCM 一致・AudioLens f64
  非回帰 — NotRun（test-spec 宣言どおり後続 slice / consumer
  gate）
