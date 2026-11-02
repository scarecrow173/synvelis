---
id: "IMPL-034MSA5207Kq2wXmZyAbCdEf"
type: "impl"
status: "completed"
title: "最小 MIDI Source / Note 編集契約 — Stable ID・Linked Source・Undo・Save/Reload"
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
      - type: "file"
        path: "docs/designs/midi-source-note-editing-design.md"
      - type: "file"
        path: "docs/impl/ir/midi-source-note-editing.md"
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
    - "docs/designs/midi-source-note-editing-design.md"
  implemented-by: []
  depends-on:
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/19"
    - "https://github.com/scarecrow173/synvelis/issues/20"
    - "https://github.com/scarecrow173/synvelis/issues/43"
  blocks:
    - "https://github.com/scarecrow173/synvelis/issues/45"
  supersedes: []
  superseded-by: []
  related:
    - "https://github.com/scarecrow173/synvelis/issues/46"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/52"
  derived-by: []
  verifies:
    - "docs/designs/midi-source-note-editing-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# 最小 MIDI Source / Note 編集契約 実装記録

Issue #52 の Gateway 前提 model Issue。`synvelis-project` に project 所有の
`MidiSource`/`MidiNote` を導入し、`SourceId` 空間共用による Linked
Source 編集・明示 Detach・Undo/Save-Reload を既存
`Operation`/`ProjectController`/`ProjectStore` 経由のみで実現した。

## 実施した変更

| 範囲 | 内容 |
| --- | --- |
| `time.rs` / `range.rs` | `MidiTick`(非負 `define_frame!`)、`MidiRange = Range<MidiTick>` 追加 |
| `midi.rs`(新規) | `MidiDivision`(tagged enum、`Ppqn(u32)`、0 を ctor/repr/validate の3層で拒否)、`MidiNote`(key 0–127、velocity 1–127、`[start,start+len)`)、`MidiSource`(id + division + `BTreeMap<NoteId, MidiNote>`、内部 contract `validate()`) |
| `project.rs` | `Project.midi_sources: BTreeMap<SourceId, MidiSource>`(`#[serde(default)]`)、`contains_source`(media∪MIDI の union 判定)、`validate_references` へ MIDI contract + `NoteId` 重複拒否を追加 |
| `ops.rs` | `AddMidiSource` / `RemoveMidiSource` / `AddMidiNote` / `RemoveMidiNote` / `SetMidiNoteVelocity` / `MoveMidiNote` / `DetachClipSource`。`AddClip` の source 存在確認を `contains_source` へ拡張 |
| `store.rs` | `SCHEMA_VERSION` 4 → 5(v5 = `midi_sources` 追加、v4 body は default で deserialize 可能) |
| `md_a.rs` | 既存の schema literal pin を `4` → `5` へ更新(意図的な bump) |
| `placement.rs` | `validate_midi_clip_contract` — MIDI-source clip を `Musical` extent + `Natural` mapping の定義済み subset に制限 |
| `project.rs`(oracle) | `sources`∩`midi_sources` 非交差、clip-local `notes` の key↔id + field contract、`validate_midi_clip_contract` 呼出を `validate_references` に追加;`check_id_watermark` 新設 |
| `controller.rs` | `load()` が `check_id_watermark()` を実行 — 偽造 low watermark による live ID 再発行/silent overwrite を拒否 |
| `commit.rs` / `import.rs` | manifest の source-id 衝突事前検査を `contains_source` へ拡張(MIDI source との衝突も早期に明確な error で拒否 — 強制は post-apply oracle が担当) |
| `id.rs` / `ops.rs` / `project.rs` doc | `SourceId`・`Clip::source_id` 等の "media only" 記述を union 契約に更新 |

## 受入条件との対応

| AC | 対応 |
| --- | --- |
| 読み取り可能な型 + Version 付き serialization + Source/Clip 参照整合 | public 型群 + schema v5、`contains_source` が clip 参照の authoritative oracle |
| 明示 SourceId+NoteId+ExpectedRevision で 2 種類以上の編集、Stale/無効 Target/範囲外拒否 | `SetMidiNoteVelocity`・`MoveMidiNote` 等を `Command.expected_revision` 経由で適用。`StaleRevision`/`NotFound`/`Validation` |
| Linked C1/C2→S1 共有編集・clip 権限前提を埋め込まない | 編集 op は `source_id` 直接指定。複数 clip の `source_id` が同一 S1 を指し、S1 編集が全 linked clip の意味を変える |
| Independent は明示 contract なしに自動実施しない | `DetachClipSource` 明示 op(fresh SourceId + fresh NoteIds で deep copy して clip のみ repoint)。media clip への detach は拒否。COW 経路なし |
| Undo/Redo・Save/Reload・Schema 拒否・参照 Validation・Time Fidelity を headless 検証 | `tests/midi_source.rs` 9 テストで網羅 |

## 実行した検証(証拠)

- 環境: Windows、rust 1.99.0(mise pin)、revision `0a5d2ce` + 本変更
- `mise run fmt-check` — Pass
- `mise run check` — Pass(workspace 全 target)
- `mise run lint`(`cargo clippy --workspace --all-targets -- -D warnings`) — Pass
- `mise run test`(`cargo test --workspace --locked`) — Pass、36 スイート全緑(新規 13 テスト含む)

## Review-protocol 実施結果

独立 2 reviewer(cooperative / adversarial)を派遣し、候補指摘を Validator
として code 上で再検証したうえで、確定した blocking 指摘を修正した。

- **修正済み(P1)**: `sources`∩`midi_sources` 非交差の oracle 欠如
  (crafted snapshot と commit/import 経路の両方で ambiguous SourceId を
  作れた)、clip-local `notes` の map-key↔`id` 未検査が `NoteId`
  disjointness を bypass、`id_watermark` の load 未検査が live ID の
  再発行・silent overwrite を許した。
- **修正済み(P2, coherent completion)**: MIDI-source clip が
  `Loop`/`SimpleStretch`/`Absolute` extent を受け入れて未定義契約を
  永続化し得た点を `Musical`+`Natural` 制限で封じた。`MidiDivision` に
  `#[non_exhaustive]`(検証 ctor 強制)、stale doc comment 修正。
- **非 blocking として報告(別 Issue 候補)**: render preflight の
  `MissingSource` は MIDI clip で誤分類(fail-closed だが label が不正確
  — `NonMediaSource` 系の error か skip semantics の判断要);既存の
  entity map 群(tracks/lanes/clips/assets/sources 等)の key↔id 一貫性
  未検査・`Note` の extent-fit load 検査欠如・`envelope.project_id` と
  `project.project_id` の不一致未検査・`applied_commits`/`applied_imports`
  内容未検査・`SnapshotEnvelope.revision` u64::MAX overflow・`SplitClip`
  validate の source_offset 上界未検査・internally-tagged repr
  (`PlaybackMapping` 等)の serde `deny_unknown_fields` 制限 — いずれも
  本変更以前からの pre-existing gap で #52 非因果のため別途報告。
- **実証済み**: adjacently-tagged `MidiDivisionRepr` は未知 field を
  拒否することを test で確認(36 スイート全緑)。

## 未実施の検証

- `mise run test-msrv`(rust 1.89 toolchain 未導入のため未実行)
- MIDI clip の player/render lowering — scope 外(MIDI playback 未実装、
  audio lowering は `MissingSource` で正直に skip)
- GW-04/05・GW-08 実 fixture との接続 — 本 Issue は model 側の前提提供まで

## 残課題

- `MidiNote` の resize / key edit、quantize、source length、SMPTE division は
  defer。必要になった段階で op を追加すればよい。
- clip `source_region` 窓 → note 投影の canonical read API は未設計
  (fixture が実際に要求した時点で追加)。
