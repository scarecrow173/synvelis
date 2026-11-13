---
id: "TSPEC-034WEVqv4ZmteepjgHJGUL"
type: "test-spec"
status: "approved"
title: "RCD-04 MIDI phrase boundary / Undo-Redo / VCS stable-ID test spec"
created: "2026-11-13"
updated: "2026-11-13"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/rcd-04-midi-undo-stableid.md"
    - "docs/designs/rcd-04-midi-undo-stableid-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcd-04-comp-midi-phrase-fixture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-04-midi-undo-stableid.md"
    - "docs/designs/rcd-04-midi-undo-stableid-design.md"
  derived-by: []
  verifies:
    - "docs/specs/rcd-04-midi-undo-stableid.md"
    - "docs/designs/rcd-04-midi-undo-stableid-design.md"
  verified-by:
    - "docs/tasks/rcd-04-task-midi-undo-stableid.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-04 MIDI phrase boundary / Undo-Redo / VCS stable-ID test spec

## Purpose

Issue #118 残件（MIDI phrase 境界・Undo/Redo・VCS stable-ID）
の headless 検証可能部分の不変条件を固定する。本 spec は
fixture が破ってはいけない契約を記述し、実装詳細ではなく
観測可能な振る舞いで検証する。実 device・実 GUI・AudioLens
f64 経路は対象外であり、本 spec の Pass をそれらの証拠に
昇格しない。

## Feature

RCD-04 headless residual fixtures — MIDI phrase boundary
delivery, comp switch Undo/Redo, and VCS stable-ID retention.

## Rules

- **R1（MIDI 境界順序）**: 2 region 連接 selection の
  `compile_midi_events` 出力は、region 境界の同一 frame で
  `virtual NoteOff → controller chase（宣言 pair の
  latest/unset 値）→ 同一 key の NoteOn` の順序を厳守し、
  各 note-on は一意の note-off と対をなす（half-open
  `[start,end)` 意味論により hang/重複 voice が生じない）。
- **R2（配送等価）**: `ParamDispatch::dispatch_midi` →
  `PluginNode` `EventList` 経由の HeadlessDriver 配送列は
  wire 列と `abs_frame` 付きで一致し、終端 `AllNotesOff`
  は実際に発音した channel のみに CC123 として配送される。
- **R3（comp 切替 CAS）**: `SetActiveComp` による comp 切替は
  ちょうど 1 回の `ProjectController::apply`（1 revision
  増分）で行われ、comp registry の内容（comps/regions/
  transitions）は切替で不変、active pointer のみが動く。
- **R4（Undo/Redo 復元）**: 単一 `undo()` は切替前の comp
  registry を完全一致で復元し、`redo()` は同一 stable ID で
  切替後状態を再適用する（undo/redo は新 revision を発行す
  る既存契約に従う）。
- **R5（raw 不変）**: comp 系操作全体を通じて
  `recording.takes`/`sources`/`assets` は seeded POD と
  byte 一致し、take state は `Verified` のまま変化しない。
- **R6（VCS stable-ID 保存）**: `CompTransition` の tagged
  stable ID（`comp-transition-<32hex>`）は save →
  saved-HEAD snapshot 解決 → semantic diff、および
  snapshot → `restore_as_copy` 発行の各操作を跨いで保存さ
  れる。
- **R7（diff 独立 identity）**: transition の増減は
  `CompTransition` object 自身の `Added`/`Removed` change
  のみで表現され、親 `Comp.transition_ids` の membership
  差分は change を生じない。
- **R8（apply fail-closed）**: comp kind を含む changeset の
  `apply_selected`/`target_fingerprint` は
  `ApplyError::Unsupported` で拒否し、live project は
  revision・comp JSON ともに不変 — 現行 apply surface 境界
  の契約固定。

## Examples

- **E1（境界再発音）**: Given region A `[0,3840)` と region
  B `[3840,6720)` が同一 key64 の note を境界で切断・再発音
  するとき、frame 3840×25 に virtual NoteOff・5 pair の
  ControllerChase・NoteOn(key64) がこの順で配送され、
  ledger 上で同一 `(channel,key)` が hang せず重複発音も
  起きない。
- **E2（comp 切替 undo）**: Given comp1 が active で comp2
  （同一 take group の別 comp）が存在するとき、
  `SetActiveComp(comp2)` は +1 revision で適用され、
  直後の `undo()` で active が comp1 に完全復元、
  `redo()` で comp2 が同一 ID で再適用される。
- **E3（transition identity）**: Given transition 有/無の 2
  project（共有 op prefix で他 entity の ID が一致）を比較
  するとき、changeset は `CompTransition` の
  `Added`/`Removed` 1 件のみを含み、
  `object.stable_id == transition.to_tagged()` である。
- **E4（restore 計上）**: Given transition を含む comp を持つ
  live project を `restore_as_copy` で発行するとき、receipt
  の `stable_ids.counts` が take-group/comp/comp-region/
  comp-transition を計上し、発行 snapshot の
  `project.comp` 4 map の key 集合が live と一致する。

## Guarantees

- 境界 fixture は `compile_midi_comp`/`compile_midi_events`/
  `dispatch_midi`/`PluginNode` の実経路のみを通り、test 内に
  scheduler や ledger を複写しない。
- 期待 wire 列は plan 契約（`midi_comp.rs`）から手で導出し、
  実装出力の写しではない。
- comp 操作の検証は `ProjectController` の公開 CAS/undo/redo
  API と VCS の公開 snapshot/diff/apply/restore API のみを
  経由し、内部表現に依存しない。
- fixture の成功は headless/fake 環境の証拠であり、実
  device・GUI・AudioLens の Pass を意味しない。

## Non-goals

- MIDI comp selection の project 永続化（#126 待ち）。
- comp kind の VCS-05 apply whitelist 追加（現行は
  fail-closed が契約）。
- semantic/render 経路の MIDI parity（その経路に MIDI event
  が存在しない）。
- 実 device・実 GUI・AudioLens f64・app GUI E2E 残件。
- 既存 fixture（PR#219/#300/#319）が既に pin する範囲の
  重複検証。

## Risk

- 期待 wire 列の手導出が plan 実装と同じ誤読を共有する可能
  性 — ただし project 側 `rcd_04_midi_phrase.rs` が plan
  契約を独立 pin しており、wire 層の drift は本 fixture が
  検出する。
- restore fixture は published layout と ID 保存の headless
  検証であり、FS 耐久性・実 store の認定ではない。
- apply fail-closed を pin することで将来の apply 拡張 slice
  が本テストを更新する必要がある — 意図的な契約固定であり
  障害ではない。
