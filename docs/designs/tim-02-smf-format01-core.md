---
id: "DESIGN-034UwVoRGI8Bp9zuzRFQhj"
type: "design"
status: "approved"
title: "TIM-02 SMF Format0/1 core — crate 内完結設計"
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
  implements:
    - "docs/specs/tim-02-smf-format01-import-export.md"
  implemented-by: []
  depends-on:
    - "docs/designs/midi-source-note-editing-design.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers:
    - "synvelis-player の MIDI RT/Offline 結合・Loop/Seek/Stop・RT callback（後続 slice）"
    - "実 MIDI Backend OS 別・実 Device/Plugin 検証（Ruviform #253 の責務）"
    - "SMPTE Division・MIDI 2.0/MPE（#129/#130 future・明示 Unsupported）"
  deferred-by: []
---

# TIM-02 SMF Format0/1 core — crate 内完結設計

## Context

spec `tim-02-smf-format01-import-export` の headless slice を、
`synvelis-project` 内の additive module + 既存契約の再利用で
実現する。RCD-04（PR #219）の `compile_midi_comp` が
phrase 境界の chase / 厳密 tick 写像 / stable ID の oracle
として確立済みであり、本設計はその手前の **SMF ⇄ Project**
境界と provenance 永続化を定める。

既存事実（調査済み）:

- `MidiSource { id, division, notes }` は note のみ保持し、
  track/channel/event provenance を持たない。`MidiSource`
  struct literal は `mcp-poc` / `synvelis-gateway`（編集範囲外）
  にも存在するため、**`MidiSource` への field 追加は不可**。
- `MidiNote` も channel を持たない（`mcp-poc`/`synvelis-gateway`
  の literal 互換のため同様に変更しない）。channel provenance
  は provenance document 側が所有する。
- `Project` snapshot は `SCHEMA_VERSION = 8`、migration ladder
  は `migration.rs` の順序 step。additive field は
  `#[serde(default)]` + version 前進の前例あり（v5 `midi_sources`）。
- `compile_midi_comp` / `MidiControllerEvent` /
  `MidiPlanAction` は `synvelis-project` 内でのみ使用
  （外部 consumer なし）→ channel 追加は安全。
- doc-driven-dev-graph router は main 既存の broken relation
  で blocked（RCD-04 と同じ状況）→ chain 文書で進行し impl
  record に記録する。

## Scope

### In Scope

- `crates/synvelis-project/src/smf.rs`（新規）: SMF Format 0/1
  parser・serializer・document model・typed error・export +
  `ConversionReport`。
- `Project.smf_documents: BTreeMap<SourceId, SmfDocument>`
  （`#[serde(default)]`、schema v9 + migration step）。
- `ProjectOp::ImportMidiSmf { document }` と note 編集 op の
  immutable guard、`RemoveMidiSource` の document 連動削除。
- `MidiControllerEvent.channel` 追加、`MidiControllerKind`
  chase の `(channel, kind)` 単位化、`SmfDocument::
  controller_events()` による lane 投影。
- `crates/synvelis-project/tests/tim_02_smf.rs` fixture 群と
  `crates/synvelis-recording/tests/tim_02_midi_phrase.rs`
  （recording 実装は無変更）。

### Out of Scope

- spec の Out of Scope と同一（player RT、実機、SMPTE/
  MIDI2/MPE、GUI、他 crate の構造変更）。
- `MidiSource` / `MidiNote` への field 追加（外部 literal 互換）。
- Project tempo map への SMF tempo の自動注入（native tempo は
  provenance document が所有。project tempo は別 domain）。
- Program Change の chase（初期 event scope 外。provenance
  保存・export 再 emit は行う）。

## Components and Boundaries

### `smf.rs` — SMF codec + document model

```text
SmfDocument {
    format: SmfFormat,          // Format0 | Format1
    division: u32,              // PPQN ticks-per-quarter, 1..=0x7FFF
    tracks: Vec<SmfTrack>,      // parse 順を保持
}
SmfTrack { events: Vec<SmfEvent> }   // tick 昇順（非減少）
SmfEvent { tick: MidiTick, kind: SmfEventKind }
SmfEventKind =
    NoteOn{channel,key,velocity} | NoteOff{channel,key,velocity}
  | PolyAftertouch{channel,key,value} | Cc{channel,controller,value}
  | ProgramChange{channel,program} | ChannelAftertouch{channel,value}
  | PitchBend{channel,value}
  | EndOfTrack | Tempo{us_per_quarter}
  | TimeSignature{numerator,denominator,thirtyseconds,clocks}
  | TrackName{bytes} | MetaOther{kind,data} | Sysex{prefix,data}
```

- `parse_smf(&[u8]) -> Result<SmfDocument, SmfError>`:
  header `MThd`/`MTrk` chunk 検証、format ∈ {0,1}（他は
  `Unsupported`）、division の SMPTE bit（`0x8000`）は
  `Unsupported`、division 0 は `Malformed`。track 内は
  running status を解決しつつ status byte ごとに event を
  組み立て、delta-time varlen を `MidiTick` へ checked 蓄積
  （overflow → `Overflow`）。meta `0xFF`・sysex `0xF0/0xF7`
  以外の `0xF?` status は SMF として不正 → `Malformed`。
  meta 0x2F (EOT)・0x51 (tempo)・0x58 (time sig)・0x03
  (track name) を typed variant、その他 meta は `MetaOther`
  に raw bytes 保存。**未知 event を executable MIDI に
  変換しない** — document は provenance であり、note/lane
  への射影は明示的なメソッド経由のみ。
- `SmfDocument::paired_notes() -> Result<Vec<SmfNoteSpec>,
  Error>`: `(channel, key)` ごとに FIFO で NoteOn↔NoteOff を
  pair（velocity 0 NoteOn は NoteOff 扱い）。note-off に
  対応する on がない場合は無視（provenance には残る）。
  track 末尾で未解決の NoteOn があれば `Error::Validation`
  （unclosed note = 異常入力、typed error）。返り値は
  `(start, key, channel, velocity, len)` 昇順の決定的順序 —
  `ImportMidiSmf` の NoteId 採番順。
- `SmfDocument::controller_events() -> Vec<MidiControllerEvent>`:
  全 track の `Cc` / `PitchBend` を `(tick, track, seq)` 順に
  merge し `channel` 付きで返す。`compile_midi_comp` の
  lane 入力にそのまま使える。
- `SmfDocument::validate() -> Result<(), Error>`: snapshot
  load / op validate 用の構造不変条件（division 範囲、
  track event の tick 非減少、field domain）。
- `export_smf(project, source_id, options) ->
  Result<SmfFile, SmfError>`:
  - provenance document を持つ source → document の event を
    `options.format` / `options.division` へ変換して emit。
    Format1 は track 構造を保持、Format0 は全 track を
    `(converted_tick, track_index, seq)` で merge。
  - provenance を持たない source → `MidiSource.notes` から
    単一 event 列を合成（NoteOn/NoteOff のみ、meta なし）。
    Format1 合成は空 conductor track + event track の 2
    track 構成。
  - tick 変換 `round_half_up(tick * to_div / from_div)` を
    i128 で厳密に行い、`exact_count`（剰余 0）/
    `rounded_count` / `max_error`（`SmfRational`）/
    `unsupported_events`（現行は常に空、将来の drop 経路用
    の型だけ確保）を `ConversionReport` に記録。
  - 出力は deterministic: event は `(tick, kind rank,
    track, seq)` で整列し、NoteOff が NoteOn に先行する。
    varlen は最短符号化、running status は使わず常に status
    byte を明示する（parse が running status を正規化する
    ため、再 parse で document が一致する）。
- `SmfError`: `Malformed{offset,detail}` / `Unsupported
  {detail}` / `Overflow{detail}` / `NotFound(String)`。
  `PartialEq`/`Clone`/`Debug` を derive（Error と同じ方針）。

### Project 統合

- `Project.smf_documents: BTreeMap<SourceId, SmfDocument>`
  `#[serde(default)]`。`SCHEMA_VERSION` を 9 に前進し、
  `migration.rs` に v8→v9 step（identity — 旧 body は field
  欠落 → default empty でそのまま load 可能、投影変更なし）
  を追加。
- `validate_references` に SMF 節を追加: 各 `(source_id,
  doc)` について (a) `midi_sources` に同 key が存在、
  (b) `source.division.ticks_per_quarter() == doc.division`、
  (c) `doc.paired_notes()` の note 射影 multiset（key,
  velocity, start, len — channel は射影に含めない）が
  `source.notes` の multiset と一致。craft された snapshot
  で provenance と note が乖離することを load 時に拒否。
- `ProjectOp::ImportMidiSmf { document: SmfDocument }`:
  - validate: `document.validate()` + `paired_notes()` が
    成功すること（unclosed note をここで typed error 化）。
  - apply: `SourceId` を採番 → `paired_notes()` の決定的順に
    `NoteId` を採番して `MidiSource` 構築 →
    `smf_documents.insert(source_id, document)`。
    `MidiDivision::ppqn(doc.division)`（≤0x7FFF 保証済み）。
- Immutable guard: `AddMidiNote` / `RemoveMidiNote` /
  `SetMidiNoteVelocity` / `MoveMidiNote` の validate で
  `project.smf_documents.contains_key(source_id)` なら
  `Error::Validation`（provenance 破壊防止）。
  `RemoveMidiSource` apply は `smf_documents` も同時削除。
  `DetachClipSource` は document を複製しない → detached
  copy は編集可能（既存 semantics と整合）。

### `midi_comp.rs` — channel-aware chase

- `MidiControllerEvent` に `channel: u8`（0–15）を追加。
  `validate()` で domain 検査。
- `MidiControllerLanes` の chase union を `(channel,
  MidiControllerKind)` ペア単位に拡張。region report の
  `chase` を `Vec<((u8, MidiControllerKind), i32)>` に変更。
- `MidiPlanAction::ControllerChase` に `channel: u8` を追加。
- `MidiPlanEvent.note` は `(SourceId, NoteId)` のまま —
  note の channel は plan 層では未モデル（`MidiNote` が
  channel を持たない既存契約）。channel provenance は
  `SmfDocument` が所有し、plan は「どの channel の
  controller を chase するか」のみを扱う。
- 既存 RCD-04 test の `MidiControllerEvent` literal を
  `channel` 付きに更新。

### `synvelis-recording`

- 実装変更なし。`tests/tim_02_midi_phrase.rs` で
  `synvelis-project` の Import → `compile_midi_comp` →
  phrase plan の発音残り 0 を fixture 確認する
  （Take/Phrase 既存接続の消費証明）。

## Data and Control Flow

```text
SMF bytes ──parse_smf──▶ SmfDocument ──ImportMidiSmf──▶
  Project { midi_sources[source] = notes射影,
            smf_documents[source] = document }
        │ save/load (schema v9)  │
        ▼                        ▼
export_smf(project, source, options) ──▶ SMF bytes + ConversionReport
        │
        └─▶ parse_smf ──▶ ImportMidiSmf（reimport golden）

compile_midi_comp(selection, lanes=doc.controller_events())
  ──▶ MidiPlaybackPlan（channel-aware chase + AllNotesOff）
```

## Non-Functional Constraints

- deterministic: parse/export/ID 採番/整列はすべて決定的。
  running status 不使用・最短 varlen で byte-level 再現性を
  semantic roundtrip として保証。
- fail-closed: malformed/overflow/unsupported は typed error。
  未知 event は executable MIDI に変換しない。
- no implicit quantization: native tick は保存され、変換は
  export 時のみ明示的に行い誤差を report する。
- RT/non-goals: player RT 結合・実機検証は本 slice 対象外。

## Risks and Trade-offs

- `SmfDocument` を snapshot に永続化すると payload が
  増える → event bytes を raw `Vec<u8>` で保持する設計で
  必要最小限に留め、gzip 等は将来検討（PoC 規模では不要）。
- `validate_references` での `paired_notes()` 再計算は
  apply ごとに O(events log events) → project 規模では
  許容。巨大 file の場合は将来 cache を検討。
- Format0→Format1 export の track 再構成は「全 event を
  track0 に残す」のではなく parse 順の track を保持する
  方針 — Format0 import は単一 track なので問題にならない。
- Unclosed NoteOn を import error とする厳格さは実 file で
  稀に reject されうる → provenance は document に残るが
  import 自体が失敗する。typed error で明示する方が
  silent auto-close より誠実（spec の異常 typed error
  要件に合致）。
- doc router が pre-existing broken relation で blocked
  → RCD-04 と同じく chain 文書で進行、impl record に記録。

## Deferred Design Concerns

- `synvelis-player` MIDI RT/Offline 結合 — sv101 slice |
  後続 slice 着手時 | plan action の channel 意味論が
  player 側で未消費のまま残る | 中
- Program Change chase — 初期 event scope 外 | controller
  scope 拡張時 | seek 時に program 状態が chase されない
  可能性 | 低
- `MidiNote` への channel 追加 — `midi.rs` 変更は外部
  literal 互換の調整が必要 | note-level channel が必要に
  なった時 | plan が channel 非対応のまま | 中
- SMF tempo → project tempo map 投影 — 別 domain のため
  本 slice では非投影 | tempo 連携要件発生時 | project
  tempo と native tempo の二重管理が利用者に見える | 低

## Verification Notes

- [ ] `cargo test -p synvelis-project tim_02` で fixture 全緑
- [ ] `cargo test -p synvelis-recording tim_02` で phrase
  発音残り 0
- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv` / `build`
- [ ] review-protocol で blocker なしを確認
