---
id: "TSPEC-623tim02midiNodeDeliv01"
type: "test-spec"
status: "proposed"
title: "TIM-02 MidiClipPlan→plugin node 配送 seam — fixture matrix"
created: "2026-11-23"
updated: "2026-11-23"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/623"
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
    - "docs/specs/tim-02-midi-node-delivery.md"
    - "docs/designs/tim-02-midi-node-delivery.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/tim-02-app-ops-smf.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/tim-02-midi-node-delivery.md"
    - "docs/designs/tim-02-midi-node-delivery.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# TIM-02 MidiClipPlan→plugin node 配送 seam — fixture matrix

## Purpose

`MidiClipPlan`→node 配送 seam（`RoutingControlDispatch`
`stage_midi_plan`/`clear_midi_plan`/`sync_midi_plans` +
`ParamDispatch` merged plan）と app 側 `MidiClipEvidence.
delivery` evidence を、fake instrument plugin fixture
（`crates/synvelis-player/tests/tim02_midi_node_delivery.rs`・
`crates/synvelis-app/tests/tim02_midi_delivery.rs`）で決定的に
検証する test-spec。実 plugin ホスト・GUI・実機受入の Pass を
意味しない（L3 外部 gate — NotRun）。

## Feature

compile 済み MIDI clip event が lane→track→strip chain 先頭の
instrument node に stage され、param dispatch 共存下で merged
plan が single-live-plan 契約を維持したまま MIDI+param を運び、
配送結果が `MidiClipDelivery` evidence に正直に記録される。

## Rules

- Rule: clip は `clip.lane_id` → `lane.track_id` →
  `strip_of_track` → `chains[strip.chain]` → `processors[0]`
  の strict chain 先頭 processor のみに解決される。先頭以外・
  空 chain・リンク欠落は `Unresolved`/解決 `None` であり、
  effect への fall-through 配送は行わない（fail-closed）。
- Rule: `staged_midi` は node が mint する全 plan に merge さ
  れる — param `dispatch`/`fast_control` の install が staged
  MIDI を消すことはない（single-live-plan wholesale-replace
  契約は不変）。
- Rule: staged MIDI が空の node の plan 形状は従来と同一
  （param-only 非回帰）。
- Rule: `stage_midi_plan` は `Play(due=0)` を push してから
  boundary session（宣言済みなら provenance gate）または
  `dispatch.dispatch_midi` 経由で install し、成功時に
  `staged_transport` を mirror 記録する。
- Rule: `sync_midi_plans` は reconcile — 列挙 node に stage、
  列挙外で staged を持つ node に clear を適用し、node 単位の
  失敗を `MidiStageReport.errors` で返す（partial failure で
  他 node を巻き込まない）。
- Rule: 失敗は全て型付き（`MidiStageError::{NoNode,
  StaleSurface, Boundary, Dispatch}`、`MidiClipDelivery::
  {Unresolved, Failed}`）— silent skip なし。generation swap
  後の staged 集合は再 stage されるまで配送されない。
- Rule: app 側は rebuild ごとに `stage_midi_delivery` を実行
  し、同一 node に解決される複数 clip の event を連結して
  1 回 reconcile する。`Built`/`Equivalent`/build 失敗の
  いずれでも evidence は現在の surface に対して正直に記録
  される。
- Rule: seam/dispatch 呼出しは non-RT orchestration — RT
  callback 内の allocation-free 契約に干渉しない
  （`mise run test-rt` で検証）。

## Examples

player fixture（`tests/tim02_midi_node_delivery.rs`）:

- `clip_events_deliver_to_chain_head_node`: stage → `engine.run`
  で fake instrument の `EventList` が `(frame, MidiEvent)` を
  engine frame どおり記録（AC1）。
- `merged_plan_keeps_staged_midi_across_param_dispatches`:
  stage 済み node への `fast_control` が `Delivered` を返し、
  param event と late-frame MIDI の双方が merged plan から配送
  される（AC3 merge 非回帰）。
- `clear_midi_plan_silences_staged_content`: clear 後の run で
  staged content が配送されない。
- `sync_midi_plans_reconciles_stage_and_clear`: reconcile が
  stage と clear を `MidiStageReport` で報告する。
- `generation_swap_requires_restage`: generation swap で staged
  集合は新 surface に引き継がれず、再 stage で配送再開。
- `staging_failures_are_typed`: 未 vend processor → `NoNode`、
  空 stage → 型付き拒否。
- `boundary_declared_node_stages_through_session_gate`:
  `TimingPrecision::FrameAccurateEvents` 宣言 node が
  `ProcessBoundarySession::dispatch_midi` 経路で配送。
- `instrument_resolution_is_strict_chain_head`: chain index 1
  以降には解決しない・strip なし track は `None`。

app fixture（`tests/tim02_midi_delivery.rs`）:

- `compiled_clip_stages_on_chain_head_instrument`: rebuild →
  `MidiClipDelivery::Staged(processor)` evidence + `engine.run`
  で compiled event が engine frame どおり配送（AC1 end-to-end）。
- `sibling_clips_merge_into_one_staged_set`: 同一 track の 2
  lane 2 clip が同一 node に `Staged` で reconcile され両方配送。
- `clip_without_instrument_reports_unresolved`: strip 未結合の
  track の clip は `Unresolved`。
- `staging_without_control_surface_reports_failed`: host 未
  bind では解決した processor への `Failed` evidence。

## Guarantees

- 全 fixture が deterministic（fake plugin・固定 SMF bytes・
  同一入力→同一 evidence/log）。
- param-only node の plan 形状不変が merge 導入後も保たれる。
- delivery evidence が `Staged`/`Unresolved`/`Failed`/
  `NotCompiled` を正直に区別し、成功に偽装しない。
- `mise run test-rt` の no-alloc/no-block 契約が本 seam の
  non-RT 性を裏付ける。

## Non-goals

- 実 plugin ホスト・実 OS MIDI backend・GUI・実機発音の検証
  （L3 gate — sv#176・RF #630）。
- TIM-03 timing precision の受入（boundary session 経路の
  存在証明のみ — 精度契約の再設計はしない）。
- offline render・gateway op・`HeadlessDriver` への seam 波及。
- `Processor` 永続 instrument designation の導入。

## Risk

- merge を `ParamDispatch` 以外の層に置くと param flush が
  staged MIDI を silently 消す（single-plan 契約違反ではなく
  merge 設計の欠陥）— mint の所有者に不変条件を置くことで
  構造的に防止。
- chain 先頭慣例の誤配送（先頭に effect を挿す project）は
  typed evidence で検出可能だが発音される — 永続 designation
  は design の Deferred Design Concerns。
- delivery evidence を publish 成功に限定すると `Equivalent`
  rebuild（clip 内容変更のみ）で stage が走らず stale 配送が
  残る — rebuild 末尾の無条件 reconcile で防止（実装注記）。
