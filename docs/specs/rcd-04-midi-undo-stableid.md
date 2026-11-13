---
id: "SPEC-034WEUWHrqeDl6HXbaRPVO"
type: "spec"
status: "proposed"
title: "RCD-04 MIDI phrase boundary / Undo-Redo / VCS stable-ID residual slice (synvelis#118)"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/tests/rcd_04_midi_phrase_boundary.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/rcd_04_vcs_stable_id.rs"
      - type: "file"
        path: "docs/specs/rcd-04-midi-undo-stableid.md"
      - type: "file"
        path: "docs/designs/rcd-04-midi-undo-stableid-design.md"
      - type: "file"
        path: "docs/plans/rcd-04-midi-undo-stableid-plan.md"
      - type: "file"
        path: "docs/tasks/rcd-04-task-midi-undo-stableid.md"
      - type: "file"
        path: "docs/test-specs/rcd-04-midi-undo-stableid.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-midi-undo-stableid-slice.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/tests/rcd_04_comp_cas.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md"
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-04-product-e2e.md"
    - "docs/specs/rcd-04-rt-engine-comp-pcm.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/11"
    - "https://github.com/scarecrow173/synvelis/issues/42"
    - "https://github.com/scarecrow173/synvelis/issues/50"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/126"
  defers: []
  deferred-by: []
---

# RCD-04 MIDI phrase boundary / Undo-Redo / VCS stable-ID residual slice (synvelis#118)

## What Should Be Built

Issue #118 の統合コメントが残件として名指しした 3 項目 —
**MIDI phrase 境界・Undo/Redo・VCS stable-ID** — のうち、
headless に検証可能な残り部分だけを固定する test slice。
PR#219（`compile_midi_comp` plan oracle）・PR#300（app 層
E2E）・PR#319（RT engine comp PCM 3 経路 bit-exact）でカバー済
みの範囲は再実装せず、**未検証経路との差分だけ**を pin する:

- MIDI phrase 境界: 2 region 連接 selection を
  `compile_midi_events` → `ParamDispatch::dispatch_midi` →
  `HeadlessDriver`/`PluginNode` の **実 engine 配送経路**で再生
  し、plan 契約（virtual off → controller chase → note-on の
  同一 tick 順序・同一 key の境界再発音・channel 別 chase・
  終端 CC123 が発音 channel のみ）を wire event 列と配送 log
  の双方で固定する。semantic/render 経路に MIDI は存在しない
  ため parity 対象はなく、境界 ledger 不変条件
  （hang なし・重複 voice なし）を配送列上で検査する。
- Undo/Redo: `SetActiveComp` による comp 切替が **1 回の
  `ProjectController::apply`（1 CAS step・1 revision）** で行
  われ、単一 `undo()` が切替前内容を完全復元・`redo()` が同一
  ID で再適用すること、および comp 系操作全体で raw
  take/source/asset が不変であることを fixture で固定する。
- VCS stable-ID: `CompTransition` の独立 stable ID
  （`comp-transition-<32hex>`）が、save → saved-HEAD snapshot
  解決 → semantic diff、および snapshot → `restore_as_copy`
  発行の各 VCS 操作を跨いで保存されること、diff が transition
  の増減を **transition 自身の object identity のみ**で表現す
  ること（親 `Comp.transition_ids` は membership Skip）、
  および comp kind が VCS-05 apply surface 外で
  `ApplyError::Unsupported` に fail-closed する現行契約を固定
  する。

## Why Now

Issue #118 の残件欄は「MIDI phrase 境界・Undo/Redo・VCS
stable-ID」を headless 検証可能な残件として明示している。既存
slice は plan レベル（PR#219）・app E2E（PR#300）・PCM 3 経路
（PR#319）を固定したが、以下は未検証のまま残っていた:

- `MidiPlaybackPlan` → wire `MidiEvent` → `PluginNode` 配送の
  境界順序と ledger 不変条件（TIM-02 は単一 region 相当の
  plan のみで、region 間境界の off→chase→on 同一 tick 列・
  同一 key 再発音を pin していない）。
- comp **切替**（`SetActiveComp`）の単一 CAS/Undo 性と raw
  不変性（既存 fixture は transition 追加の undo と build 全体
  の undo のみで、active pointer の切替を pin していない）。
- `CompTransition` stable ID の VCS 操作横断の保存
  （save/saved-snapshot/diff/restore）と、diff 上の独立
  identity 表現、apply surface の fail-closed 境界。

## Users and Value

- Issue #118 レビュア: 残件 3 項目の headless 証拠を 1 PR で
  提示し、Issue を Open 継続のまま残件を縮小する。
- MIDI/comp/VCS 実装者: 境界順序・単一 undo・stable-ID 保存・
  apply fail-closed の回帰 gate。

## Scope

### In Scope

- `crates/synvelis-player/tests/rcd_04_midi_phrase_boundary.rs`
  （新規）: 2 region 連接 `MidiCompSelection` + controller
  lanes → `compile_midi_comp`/`compile_midi_events` →
  `HeadlessDriver` + fixture plugin による wire 列完全一致と
  配送 ledger 不変条件。
- `crates/synvelis-project/tests/rcd_04_comp_cas.rs`（拡張）:
  comp 切替の単一 CAS/Undo/Redo + raw 不変テスト 1 件追加。
- `crates/synvelis-project/tests/rcd_04_vcs_stable_id.rs`
  （新規）: save/saved-snapshot/diff/restore 横断の comp
  stable-ID 保存、diff の独立 identity 表現、apply
  fail-closed の 4 テスト。
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/
  rcd-04-midi-undo-stableid*` の doc-chain と管理 index 再生
  成。

### Out of Scope

- 実 device/実 GUI/AudioLens f64 経路・app GUI E2E 残件（#114）。
- MIDI comp selection の project 永続化（#126 の native-tick/
  chase 意味論確定待ち — selection は plan-level 入力のまま）。
- comp kind の VCS-05 apply whitelist 追加（現行は fail-closed
  が契約; 拡張は別 slice）。
- `synvelis-app`/`synvelis-recording`/`synvelis-render` の src
  変更、Cargo manifest・共有ファイルの変更。
- 既存 fixture の重複再実装（PR#219/#300/#319 のカバー範囲）。

## Requirements

- MIDI phrase 境界の pin は **実 engine 配送経路**
  （`compile_midi_events` → `dispatch_midi` → `PluginNode`
  `EventList`）で行い、test 内に scheduler/ledger を複写しな
  い。期待 wire 列は plan 契約から手で導出する。
- 境界 fixture は half-open phrase/project range、region 間で
  同一 key を cut→再発音、controller chase の channel 分離、
  終端 panic の発音 channel 限定を含むこと。
- Undo/Redo の pin は `ProjectController` の CAS/revision/undo/
  redo の公開契約経由で行い、内部 stack を直接触らない。
- VCS stable-ID の pin は canonical serialization・
  `SnapshotResolver`（live/saved）・`compare_snapshots`・
  `restore_as_copy` の公開 API 経由で行い、ad-hoc 文字列比較
  に留めない。
- comp 操作は raw take/source/asset を不変に保つ（既存不変条
  件の継続確認を含む）。
- headless/fake fixture の成功を実 Device/OS/GUI/AudioLens の
  Pass に昇格しない — 証拠分類を正直に記録する。

## Acceptance Criteria

- [ ] 2 region 連接 MIDI comp selection が wire event 列と
  `HeadlessDriver` 配送 log の双方で、境界同一 tick の
  `NoteOff → ControllerChase → NoteOn` 順序・同一 key の
  cut→再発音・channel 別 chase 値・終端 CC123（発音 channel
  のみ）を満たし、全 note-on が一意の note-off と対をなし、
  hang/重複 voice が無い。
- [ ] `SetActiveComp` comp 切替が 1 apply = 1 revision で行わ
  れ、単一 `undo()` が切替前 comp registry を完全復元、
  `redo()` が同一 ID で再適用し、raw take/source/asset が全
  操作を通じて不変。
- [ ] `CompTransition` stable ID が save → saved-HEAD snapshot
  → diff で Equal/無変更を保ち、diff が transition 増減を
  `comp-transition-*` object のみで表現し、`restore_as_copy`
  発行 envelope で全 comp ID が一致し、receipt が comp 系
  stable ID を計上する。
- [ ] comp kind を含む changeset の `apply_selected` が
  `ApplyError::Unsupported` で fail-closed し、live project
  が不変（現行契約の固定）。
- [ ] 追加・変更テストが `mise run test` で全 pass し、canonical
  9 lane がすべて exit 0（実行したものだけを Passed と記録）。

## Deferred Design Concerns

- MIDI comp selection の project 永続化 — #126 の
  native-tick/chase 意味論確定待ち（`MidiCompSelection` は
  plan-level 入力のまま）。
- comp kind の VCS-05 apply whitelist 追加 — diff には乗るが
  transplant/write path は未対応。apply surface を拡張する場
  合は typed entity extraction/transplant/membership reconcile
  を含む別 slice が必要。
- 実 device/OS GUI・AudioLens f64 consumer 経路での同条件照合
  — 実機証拠が必要で headless fixture では Unsupported
  （#114・Ruviform #294 待ち）。
- app GUI E2E 残件 — `synvelis-app` 層の別残件。

## Sources and Relations

- Issue #118（RCD-04 受入条件の転記元）: 残件欄の
  MIDI phrase 境界・Undo/Redo・VCS stable-ID。
- `docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md` —
  `compile_midi_comp` plan oracle（PR#219）。
- `docs/specs/rcd-04-product-e2e.md` — app 層 E2E（PR#300）。
- `docs/specs/rcd-04-rt-engine-comp-pcm.md` — RT engine comp
  PCM 3 経路 bit-exact（PR#319）。
- `docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md`
  — TakeGroup/Comp/Transition の上流契約。
