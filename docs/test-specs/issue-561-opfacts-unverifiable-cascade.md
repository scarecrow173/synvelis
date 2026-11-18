---
id: "TSPEC-561OpFactsUnverifiable06"
type: "test-spec"
status: "approved"
title: "Issue #561 test-spec — op_facts unverifiable cascade の検証仕様"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/561"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-561-opfacts-unverifiable-cascade.md"
    - "docs/designs/issue-561-opfacts-unverifiable-cascade.md"
  implemented-by: []
  depends-on:
    - "docs/specs/issue-561-opfacts-unverifiable-cascade.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-561-opfacts-unverifiable-cascade.md"
    - "docs/test-specs/cb-06-track-bus-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-561-opfacts-unverifiable-cascade.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/impl/ir/issue-561-opfacts-unverifiable-cascade.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #561 test-spec — op_facts unverifiable cascade の検証仕様

## Feature: wire prepare fail-closed（SEC-14）

`op_facts` が観測不能 entity の cascade 削除を `unsupported`
に報告し、`provisional_gate` が `PreparationRejected {
reason: "unverifiable effects: …" }` で全 caller を拒絶
することを pin する。

## Rule 1: set-gain-target seal 解放の観測不能 assignment 削除

- **Given**: send + sealed `SendLevel` target + その target
  を `target` または `depth_source` に持つ
  `ModulationAssignment`
- **When**: `synvelis.send.set-gain-target` を `null`
  （clear）または別の adoptable target（rebind）に向ける
- **Then**: `PreparationRejected` で reason が
  "unverifiable" を含む。proposal は発行されない
- **Examples**:
  - `sv561_set_gain_target_clear_unobservable_assignment_denied`
  - `sv561_set_gain_target_rebind_unobservable_assignment_denied`

## Rule 2: seal 解放が観測可能 entity のみを消す場合は受理

- **Given**: send + sealed `SendLevel` target（touching
  assignment なし）
- **When**: `synvelis.send.set-gain-target` clear
- **Then**: prepare が受理され、released target の delete が
  effects に現れる（過剰拒否なし）
- **Example**: `sv561_set_gain_target_clear_without_assignment_accepted`

## Rule 3: track.remove の strip cascade における観測不能 modulation 削除

- **Given**: lane 非保持 track + `AddTrackStrip` +
  `ControlOwner::Strip` の `ModulationSource`
- **When**: `synvelis.track.remove`
- **Then**: `PreparationRejected` "unverifiable"
- **Example**: `sv561_track_remove_unobservable_strip_cascade_denied`

## Rule 4: strip cascade が観測可能 entity のみを消す場合は受理

- **Given**: lane 非保持 track + strip（seeded channel
  targets のみ、modulation なし）
- **When**: `synvelis.track.remove`
- **Then**: prepare が受理され、track/strip delete が
  effects に現れる
- **Example**: `sv561_track_remove_plain_strip_accepted`

## Rule 5: midi.source.remove の smf_documents 削除

- **Given**: `ImportMidiSmf` 由来の clip 非参照 midi source
  （`smf_documents` entry 持ち）
- **When**: `synvelis.midi.source.remove`
- **Then**: `PreparationRejected` "unverifiable" — source
  自体の delete は observable だが provenance 削除は不可視
- **Example**: `sv561_midi_source_remove_smf_document_denied`

## Rule 5b: track.remove の monitor decl 削除

- **Given**: `SetTrackMonitor` で decl を持つ track（strip
  非所有でも可 — decl 削除は無条件）
- **When**: `synvelis.track.remove`
- **Then**: `PreparationRejected` "unverifiable" — decl は
  `ResourceKind` を持たず `detail_of` にも現れない
- **Example**: `sv561_track_remove_monitor_decl_denied`

## Rule 5c: closure target / depth_source の assignment leg

- **Given**: send + sealed `SendLevel` target + assignment が
  `target == sealed` または `depth_source == Some(sealed)`
  で触れる
- **When**: `synvelis.send.remove`（closure の target leg）
  または `synvelis.send.set-gain-target` clear（release の
  depth_source leg）
- **Then**: `PreparationRejected` "unverifiable"
- **Examples**: `sv561_send_remove_unobservable_assignment_denied`・
  `sv561_set_gain_target_clear_unobservable_depth_source_denied`・
  domain pin `remove_send_drops_assignment_on_owned_target`・
  `set_gain_target_clear_releases_assignment_via_depth_source`

## Rule 5d: batch member の逐次状態評価

- **Given**: send（`gain_target: None`）+ adoptable target +
  その target を指す assignment
- **When**: `synvelis.operation.batch` で
  `[set-gain-target → target, set-gain-target → null]`
- **Then**: member 2 は member 1 が seal した状態を読んで
  release cascade を検出し `PreparationRejected`
  "unverifiable"（pinned base 評価では
  `gain_target: None` で見逃される）
- **Example**: `sv561_batch_seal_release_unobservable_denied`

## Rule 6: domain cascade 発火条件の pin（drift 防止）

- `release_send_level_target` が clear で発火し assignment
  を削除・source を生存させること（`target` leg・
  `depth_source` leg の両方）
- `remove_control_owned` が send-owned target に触れる
  assignment を send.remove で削除すること
- `remove_strip_owned` が strip-owned modulation を削除する
  こと
- `RemoveMidiSource` が `smf_documents` entry を削除する
  こと
- `RemoveTrack` が `monitoring.tracks` decl を無条件で
  削除すること
- 同一 target 再設定では release が発火しないこと
  （`Some(old) != *gain_target` guard）
- **Examples**: `sv561_unobservable_cascade.rs` 7 件

## 検証方法

- wire pin: `mise run test synvelis-gateway`（`prepare.rs`
  内 sv561 6 件 + 既存 `unverifiable_effects_fail_closed_
  for_every_caller`・cb06 sv#550/sv#553 pin の非回帰）
- domain pin: `mise run test synvelis-project`（
  `sv561_unobservable_cascade.rs` 4 件）
- red→green 証拠: `evidence/sv561/reproduction.md`
