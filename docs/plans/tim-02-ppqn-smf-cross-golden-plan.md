---
id: "PLAN-TiM02PpqnSmfXg03"
type: "plan"
status: "approved"
title: "TIM-02 × TIM-01 PPQN×SMF cross-golden fixture — implementation plan"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/126"
    - "https://github.com/scarecrow173/synvelis/issues/125"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/tim-02-ppqn-smf-cross-golden.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/tim-02-ppqn-smf-cross-golden.md"
    - "docs/designs/tim-02-ppqn-smf-cross-golden-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/tim-02-ppqn-smf-cross-golden.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/126"
  defers: []
  deferred-by: []
---

# TIM-02 × TIM-01 PPQN×SMF cross-golden fixture — implementation plan

## Goal

Issue #126 受入行「PPQN 異種の Import / Project Change /
Save / Export / 再 Import」の交差を
`crates/synvelis-project/tests/tim_02_ppqn_smf.rs` の
headless fixture で固定する: SMF-backed source を持つ
project に `SetProjectPpqn`（Exact/Round）を適用しても
source-native provenance・export@480 golden・re-import
doc equality が保たれることを証明し、doc chain 6 件を
作成する。

## Scope

新規 test file 1 件 + doc chain 6 件に閉じる。
`synvelis-project/src/`・`ops.rs`/`control.rs`・他 crate・
禁止 test は変更しない（fixture が実不変違反を露呈した
場合のみ `src/{project,smf,ppqn}.rs` を例外的に許容）。

## Upstream Documents

- `docs/specs/tim-02-ppqn-smf-cross-golden.md`

## Design Inputs

- `docs/designs/tim-02-ppqn-smf-cross-golden-design.md`

## File Map

- `crates/synvelis-project/tests/tim_02_ppqn_smf.rs` —
  新規: SMF byte builder（vlq/ev/meta/tempo/timesig/
  trackname/eot/note_on/note_off/cc/mtrk/smf_file）・
  `format1_division480` fixture（tempo×2・CC64×3・
  closed note×2）・controller/apply/rejected/ppqn_op/
  import_smf/musical_clip/midi_window/add_clip/
  export_golden/note_tuples/count_events/
  assert_provenance_present/assert_export_reimport_golden
  helper・2 テスト
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  tim-02-ppqn-smf-cross-golden*` — doc chain 6 件（新規）

## Implementation Sequence

- [x] 既存 `tim_01_ppqn.rs`/`tim_02_smf.rs`・`ppqn.rs`/
      `smf.rs`/`ops.rs` の公開 API と不変条件を調査
- [x] `tim_02_ppqn_smf.rs` fixture 実装（Exact chain +
      Round chain の 2 テスト）
- [x] `cargo test -p synvelis-project --test
      tim_02_ppqn_smf` — 2 件 green
- [x] `cargo clippy -p synvelis-project --all-targets --
      -D warnings` — clean
- [x] doc chain 6 件を作成
- [ ] 全検証 lane 実行

## Dependencies and Risks

- `synvelis-project` 公開 API のみ消費:
  `parse_smf`/`export_smf`/`SmfDocument`/`SmfEventKind`/
  `SmfExportOptions`/`SmfFormat`/`SmfFile`/`SmfRational`/
  `ProjectOp::{ImportMidiSmf,AddTrack,AddLane,AddClip,
  SetProjectPpqn}`/`PpqnConversionPolicy`/`ProjectPpqn`/
  `ClipPlacement`/`ClipExtent`/`ClipPlayback`/
  `PlaybackMapping`/`TimeAnchor`/`MusicalTick`/
  `MusicalDuration`/`SourceRange`/`SourceFrame`/
  `InMemoryStore`/`ProjectController`/`CounterIdGenerator`。
- `ruviform_media_core::Rounding`（`Round` policy の
  引数）— `tim_01_ppqn.rs` と同じ import 形。
- 実不変違反リスク: `SetProjectPpqn` が
  `smf_documents`/`midi_sources` に触れる実装不具合が
  あれば fixture が検出する — 発生時のみ `src/` 変更を
  検討（現時点では発生せず）。

## Task Breakdown

- `docs/tasks/tim-02-ppqn-smf-cross-golden-task.md` —
  実装タスク一式

## Verification

- [x] `cargo test -p synvelis-project --test
      tim_02_ppqn_smf` — 2 件 green
- [x] `cargo clippy -p synvelis-project --all-targets --
      -D warnings` — clean
- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
      `test-rt` / `check-app-native` / `lint-app-native` /
      `test-msrv` / `build` — 結果は impl IR に記録

## Open Questions / Gaps

- player/app 層の PPQN×SMF 結合（clip→MIDI 配線・
  gateway lowering・app op）— 他 Worker 占有・別 slice。
- 実機 MIDI timing・AudioLens f64・SMPTE/Format2/MIDI2
  — NotRun/Unsupported 委譲（#253/#294/#129/#130）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology
      are consistent.

## Implementation Handoff

- Recommended mode: inline execution（dispatch worker
  として実行済み）
- Independent work streams: none
- User approval required before dispatch: n/a（worker
  spec で直接割当）

## Review Handoff

- 「export が project PPQN を参照しない」不変条件を
  bytes 3 時点一致で固定した設計の妥当性、および
  `src/` 無変更で完結したことを reviewer/coordinator
  が確認。
