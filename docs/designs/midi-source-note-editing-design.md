---
id: "DESIGN-034MSA52Mn8xRt6yQwZkPf"
type: "design"
status: "draft"
title: "最小 MIDI Source / Note 編集契約 設計"
created: "2026-11-03"
updated: "2026-11-03"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/52"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/midi.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/midi_source.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/controller.rs"
      - type: "file"
        path: "crates/synvelis-project/src/id.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/ops.rs"
      - type: "file"
        path: "crates/synvelis-project/src/placement.rs"
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/range.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-project/src/time.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/md_a.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/commit.rs"
      - type: "file"
        path: "crates/synvelis-player/src/import.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "https://github.com/scarecrow173/synvelis/issues/52"
  implemented-by:
    - "docs/impl/ir/midi-source-note-editing.md"
  depends-on:
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/designs/pc-b-parameter-dispatch-note-scope-design.md"
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/19"
    - "https://github.com/scarecrow173/synvelis/issues/20"
  blocks:
    - "https://github.com/scarecrow173/synvelis/issues/45"
  supersedes: []
  superseded-by: []
  related:
    - "https://github.com/scarecrow173/synvelis/issues/43"
    - "https://github.com/scarecrow173/synvelis/issues/46"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/52"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/impl/ir/midi-source-note-editing.md"
  references:
    - "https://github.com/scarecrow173/ruviform/issues/189"
  defers:
    - "SMF Import/Export、Device MIDI 録音、MIDI 再生 lowering"
    - "SMPTE 形式 division、source length、note resize/key edit"
  deferred-by: []
---

# 最小 MIDI Source / Note 編集契約 設計

Issue #52 — Gateway 初期 MIDI 編集 Fixture(GW-04/05、GW-08)が必要とする
**Project Model 側の前提契約**。AI 専用の Note Store・別 Writer・別 Undo・
別 Serializer は作らず、既存 `Operation` / `ProjectController` /
`ProjectStore` の上に乗る。

## 1. モデル配置の決定

`Source` は asset 参照 + `Imported → Verified → Committed → Saved` の
検証チェーンを持つ **media bytes の参照** であり、MIDI の note 列は
project 所有のデータ(= asset/bytes/state を持たない)。`Source` を
enum 化して media payload を潜らせる案は、recording / player / render /
workspace 全域の `source.asset_id` / `source.state` アクセスを壊すため
不採用。

採用: **`Project.midi_sources: BTreeMap<SourceId, MidiSource>`** を新設し、
`SourceId` の ID 空間は media `Source` と共有する。`Clip::source_id` の
参照解決は 2 map の union とし、`Project::contains_source` がその
authoritative 判定になる。

```rust
// project.rs
pub sources: BTreeMap<SourceId, Source>,          // media (asset-backed)
pub midi_sources: BTreeMap<SourceId, MidiSource>, // project-owned notes
```

- `contains_source` が true となるだけでは media 系契約を満たさない。
  recording / comp / import / L1 lowering は引き続き `sources` のみを
  見る(media bytes を要求する domain のまま)。MIDI source をそれらに
  渡すと `NotFound` で正直に失敗する。
- 単一 `IdGenerator` が両 map に ID を払い出すため、media/MIDI 間の
  `SourceId` 衝突は構築上起きない。

## 2. エンティティ

```rust
// midi.rs
pub enum MidiDivision { Ppqn(u32) }        // > 0、serde repr でも検証
pub struct MidiNote {
    pub id: NoteId,
    pub key: u8,          // 0..=127
    pub velocity: u8,     // 1..=127 (0 = wire 上の note-off であり属性でない)
    pub start: MidiTick,  // source-native tick, >= 0
    pub len: MidiTick,    // > 0
}
pub struct MidiSource {
    pub id: SourceId,
    pub division: MidiDivision,
    pub notes: BTreeMap<NoteId, MidiNote>,
}
```

- **`MidiTick`(`define_frame!`)**: `time.rs` の非負 frame 族に追加。
  project PPQN tick(`MusicalTick`)とは型で区別 — source-native tick を
  project tick へ暗黙代入できない。`MidiRange = Range<MidiTick>`。
- **分解能忠実性**: note は division に対する整数 tick で保持。
  `MidiDivision` は `Ppqn` variant を持つ tagged enum — SMF SMPTE
  division は将来 variant で schema 破壊なく追加できる。native tick を
  project tick へ即時丸めない(#19 の元形式保存契約)。musical 変換は
  `tick / division` の厳密有理数として consumer が行う。
- **`[start, start+len)` の半開区間**: `MidiNote::end()` /
  `MidiNote::range()` が checked に提供。重複 same-key note は合法
  (最小契約は dedup しない — Note Ledger 検討は TT-B 以降)。
- **clip 側の窓**: `ClipPlayback::source_region`(SourceFrame 単位)は
  MIDI source に対して native tick の窓を表す。MIDI-source clip は
  `ClipExtent::Musical` + `PlaybackMapping::Natural` の定義済み
  subset に制限される — `Absolute` extent の frame-domain 単位と
  media-rate mapping は note 列に未定義であり、評価不能な契約を
  snapshot に残さない。`TimeAnchor` は kind 非依存で制限しない。

`Note`(clip-local, `project.notes`, #43)は `ControlScope::Note` の
評価 domain anchor であり、MIDI note payload とは責務が異なる。
`NoteId` 空間は共有するが、addressing は常に `(SourceId, NoteId)`。
`validate_references` は両 map 間の ID 重複を snapshot 改竄として拒否する。

## 3. 操作契約(`ProjectOp` 経由)

```rust
AddMidiSource        { division }                              // create source
RemoveMidiSource     { source_id }                             // unreferenced のみ
AddMidiNote          { source_id, key, velocity, start, len }
RemoveMidiNote       { source_id, note_id }
SetMidiNoteVelocity  { source_id, note_id, velocity }          // 1..=127
MoveMidiNote         { source_id, note_id, start }             // len 不変
DetachClipSource     { clip_id }                               // 明示 detach
```

- **addressing**: 編集系 op はすべて `source_id`(+`note_id`)を直接名指す。
  「C1 の権限だけで S1 が編集できる」という clip 経由・権限の前提を
  model に埋め込まない(#52 AC)。`ExpectedRevision` は既存
  `Command.expected_revision` — stale は `Error::StaleRevision`。
- **Linked semantics**: 複数 clip が同一 `SourceId` を参照できるのは
  media と同じ。S1 への note 編集は両 clip の意味を同時に変える
  (shared source を両者が読むため、別途同期処理はない)。
- **Detach(Linked → Independent)**: `DetachClipSource` は clip が読む
  MIDI source を deep copy し(fresh `SourceId` + 全 note に fresh
  `NoteId`)、その clip の `source_id` のみを新 source へ向ける。
  copy-on-write を暗黙実行する経路は存在しない — 編集 op は常に
  source をその場で変更し、independence はこの明示 op のみで得る。
  media source の clip への detach は `Validation` 拒否(media の複製は
  契約外)。fresh NoteId を採るのは project 内 `NoteId` 一意性を保つため。
- **範囲外拒否**: `key > 127`、`velocity ∉ 1..=127`、`len <= 0`、
  `start + len` overflow を validate で拒否。`MidiTick::new` は負値を
  `Negative` で拒否。division 0 は `MidiDivision::ppqn` ctor / op
  validate / serde repr の 3 層で拒否。
- `Composite` / `SplitClip` / `RemoveClipsInRange` 等の既存 op は kind
  非依存のまま動作 — MIDI clip の分割は window 分割のみで、source
  上の note は動かない(linked の意味と整合)。`RemoveClip` は
  clip-local `notes` のみ掃除し、MIDI source は残る。

## 4. 永続化 / Schema 条件

- `SCHEMA_VERSION` を **4 → 5**。`midi_sources` は `#[serde(default)]`
  で v4 body も deserialize 可能(v4 からの migration はない —
  PoC-stage では version bump が意図的な形式前進を記録する)。
- `MidiDivision` / `MidiNote` / `MidiSource` は `deny_unknown_fields`。
  `MidiDivision` は `try_from` repr で 0 division を load 拒否し、
  `#[non_exhaustive]` で外部からの直接 variant 構築を封じて
  `ppqn()` ctor の検証を強制する。
- `validate_references`(load oracle / post-apply oracle):
  - `clip.source_id` が `contains_source` を満たすこと
  - `sources` ∩ `midi_sources` = ∅ — 同一 `SourceId` が media/MIDI 両
    map に居る ambiguous 状態を拒否
  - `midi_sources` map key == `source.id`、note map key == `note.id`
  - clip-local `notes` でも map key == `note.id`(`note.key ≤ 127`、
    `start ≥ 0` の基本 field contract も同時に検査 — key ベースの
    `NoteId` 重複チェックの bypass を防ぐ)
  - 全 note の value contract(key/velocity/len/end 有限)
  - clip-local `notes` および他 MIDI source との `NoteId` 重複がないこと
  - MIDI-source clip は `Musical` extent + `Natural` mapping のみ
    (`Absolute` extent は media frames ↔ native ticks の単位衝突、
    `Loop`/`SimpleStretch` は note 列に未定義 — 永続化する意味を
    持たない契約を拒否)
- `load()` はさらに `id_watermark` > 全 serialized entity key の raw
  値であることを検査 — 偽造 low watermark で generator を live ID の
  下に reseed し、次の op が既存 ID を再発行して silent overwrite
  する経路を閉じる。post-apply oracle ではなく load 境界のみで検査
  する(apply 中はまだ未 stamp の旧 watermark が正当)。
- Undo/Redo・save/reload は `Project` スナップショット単位の既存機構に
  そのまま乗る — MIDI 固有の undo/serializer は増やさない。

## 5. 非対象(本 Issue の明示範囲外)

- SMF Import/Export、Device MIDI 録音、MIDI playback/lowering
- clip window ↔ note の投影・再生意味の lowering 契約
- note resize / key edit / quantize、SMPTE division、source length
- AI / MCP / Authorization / Semantic Diff 側の実装(GW-* 側の Issue)
