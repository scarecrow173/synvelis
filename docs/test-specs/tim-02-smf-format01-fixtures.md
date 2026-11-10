---
id: "TSPEC-034UwYugwIyQVbAeVxql8O"
type: "test-spec"
status: "proposed"
title: "TIM-02 SMF Format0/1 fixture matrix"
created: "2026-11-09"
updated: "2026-11-09"
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
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/tim-02-smf-format01-import-export.md"
    - "docs/designs/tim-02-smf-format01-core.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# TIM-02 SMF Format0/1 fixture matrix

## Purpose

Issue #126 slice の file-headless 契約を決定的に検証する
test-spec。SMF parse/export・project provenance・phrase plan
の境界を fixture で固定する。実 Device/実 GUI/実 OS/RT の
Pass を意味しない（NotRun — 実機 gate / consumer gate）。

## Feature

SMF Format 0/1 の Import/Export が native tick・PPQN・
tempo・note order・channel・CC64 を保存し、typed error と
deterministic ordering・chase/panic を保証する。

## Rules

- Rule: `parse_smf` は Format 0/1 のみ受理し、Format 2・
  SMPTE division（division の `0x8000` bit）・division 0・
  truncated/length 不整合を typed `SmfError` で拒否する。
- Rule: SMF event はすべて provenance document に保存され、
  未知 meta/sysex は `MetaOther`/`Sysex` として executable
  MIDI に変換されない。
- Rule: `ImportMidiSmf` は `paired_notes()` の決定的順で
  `NoteId` を採番し、`MidiSource.division` に native PPQN を
  保持する。unclosed NoteOn は `Error::Validation`。
- Rule: `smf_documents` は snapshot に永続化され、
  `validate_references` が (a) source 存在 (b) division
  一致 (c) note 射影 multiset 一致を強制する。
- Rule: SMF 由来 source の `AddMidiNote`/`RemoveMidiNote`/
  `SetMidiNoteVelocity`/`MoveMidiNote` は拒否され、
  `RemoveMidiSource` は document を連動削除する。
  `DetachClipSource` は provenance を持たない独立 copy を
  作る。
- Rule: `export_smf` は `ConversionReport` に
  `exact_count`/`rounded_count`/`max_error`/
  `unsupported_events` を返し、native→target division の
  変換を `round_half_up` で行う。target == source なら
  全 event exact。
- Rule: 同一 tick の event は NoteOff→NoteOn の順で
  emit され、Format0 merge は `(tick, track, seq)` で
  deterministic。
- Rule: `compile_midi_comp` の chase は `(channel,
  MidiControllerKind)` 単位で、region report と plan
  action が channel を保持する。comp 末尾 `AllNotesOff`
  で発音残り 0。
- Rule: Export→Reimport で `SmfDocument` の意味
  （format/division/全 event の tick・kind・payload）が
  保存される。

## Examples

- Format0 fixture: 単一 track に tempo meta + 2 channel の
  note/CC64/sysex を持つ SMF → import で notes・division・
  provenance 保存、native tick で note start/len が一致。
- Format1 fixture: conductor track（tempo/time sig）+
  2 event track → import で track 数・channel・event 順序
  保存。
- PPQN mismatch: division 480 source を project PPQN 960
  で export → tick が正確に 2 倍（全 exact）、
  division 960→480 で端数 tick は `rounded_count>0`・
  `max_error` が rational で報告される。
- NoteOff→On: 同一 tick に NoteOff(key=60) と
  NoteOn(key=60) → export/plan で off が先。
- Chase: region 境界で ch0 CC64=127・ch1 CC64=0 の lane
  → chase snapshot が channel 別に正しい値を emit。
- Panic: phrase plan 末尾に `AllNotesOff`、仮想 off で
  region 境界を跨ぐ note が切れる。
- Malformed: `MThd` でない先頭・track length 超過・
  varlen 途中切れ → `SmfError::Malformed`。
- Unsupported: format=2・SMPTE division・format=1 で
  division 0 → `SmfError::Unsupported`/`Malformed`。
- Overflow: delta 蓄積が i64 超過・変換 tick が i64 超過
  → `SmfError::Overflow`。
- Golden: `parse(export(parse(bytes)))` の document が
  `PartialEq` で一致。
- Recording fixture: SMF import → `compile_midi_comp` →
  plan events に NoteOn 残りなし（全 NoteOff/AllNotesOff
  で閉じる）。

## Guarantees

- 全 fixture が deterministic（同一入力→同一
  document/bytes/plan）。
- typed error が malformed/overflow/unsupported を区別し、
  SMPTE/MIDI2 を成功扱いしない。
- provenance document が save/load を生き残り、note 射影
  との乖離は load 時に拒否される。

## Non-goals

- 実 MIDI device/OS backend・RT timing・GUI 経路・
  player 結合の検証。
- SMPTE division・MIDI 2.0/MPE・Format 2 の受理。
- `MidiNote`/`MidiSource` への channel field 追加。
- SMF tempo の project tempo map への自動投影。

## Risk

- provenance 永続化を怠ると export→reimport で event
  種別・順序・tempo が失われ、issue の中核要件
  （native fidelity）を満たせない。
- immutable guard を怠ると note 編集が provenance と
  乖離し、load 時の multiset 検査で project が
  load 不能になる。
- chase を channel 非対応のままにすると複数 channel の
  CC64/sustain が混線し、phrase 境界で誤った controller
  状態が残る。
