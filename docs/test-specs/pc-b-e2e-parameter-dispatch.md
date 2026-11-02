---
id: "TSPEC-034PCB05Ql0zTu6aCdEfGh"
type: "test-spec"
status: "approved"
title: "PC-B Parameter Control dispatch/note-scope/RT/offline 検証仕様"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source: []
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
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/designs/pc-b-parameter-dispatch-note-scope-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/designs/pc-b-parameter-dispatch-note-scope-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-B 検証仕様

## Feature: 実 dispatch 経路(PC-B-01)

- Rule: `TimedParamEvent` は ruviform bridge(plugin host → attach →
  PluginNode → RtRuntime)経由で endpoint へ届く。
  - Example: fixture stub plugin が `ParamEvent` を受信し、
    (param_id, frame offset, value) が plan のイベントと一致する。
- Rule: `ParamWriteRejection` は型保持で上位へ伝播する。
  - Example: 未登録 param の `bind` → `param_unknown`、
    未 bind target の `resolve_write` → `param_unresolved`、
    `READONLY` capability の `lower_event` → `param_readonly`。
- Rule: dispatch 観測は `ParamDispatchLedger` の stage で得られる。
  - Example: `observe` が `Dispatched` 観測を返す。

## Feature: Note-local scope(PC-B-02)

- Rule: `ControlScope::Note` の評価座標は note-relative beats。
  - Example: clip@beats 8..16、note@clip-relative 2..6 のとき、
    project pos 10 → note-local 0、pos 14 → 4。pos 16(clip 外)・
    pos 9(note 手前)は `ScopeUnavailable`。
- Rule: 記録点は note-relative 座標で格納される。
  - Example: pos 11 に gesture write → lane point pos=1。
- Rule: domain 外の書き込み点は記録されない。
  - Example: note 範囲外への record_write は skip(commit に出ない)。
- Rule: note-scoped target は所有 clip の削除を pin する。
  `RemoveClip` は参照が残る限り reject され、lane→target 解除後に
  成功すると note も cascade 削除される。
  - Example: target 残存時 `RemoveClip` → `NotFound`、clip/note は
    無変更。lane+target 削除後の `RemoveClip` → note 消失。
- Rule: `AddNote` は domain を validate する。
  - Example: `key > 127`・`start+len > clip extent` → `Validation`。
- Rule: `SplitClip` は分割点を跨ぐ note の commit を拒否し、
  分割点以降の note は clip B へ re-parent + start を re-base して
  評価 domain の絶対位置を保存する。
  - Example: note@clip-rel 2..6、分割@rel 4 → `Validation`。
    分割@rel 1 → note は B の rel 1..5、project pos 12 の評価値は
    分割前と一致。

## Feature: RT allocation-free(PC-B-03)

- Rule: `EvalPlan::eval_rt` + `lower_event` の成功パスは
  alloc/dealloc=0。
  - Example: `assert_no_alloc` 内で eval_rt→lower_event→scratch
    push を繰り返し、allocations==0。

## Feature: offline render 一致性(PC-B-04)

- Rule: 同一イベント列の配送結果は playback drive と `render()` で
  (param, absolute frame, value) が一致する。
  - Example: 同一 plan を RtRuntime block drive(block=128)と
    render()(callback_chunk=64)に流し、endpoint 観測列が一致。
- Rule: upstream `PluginNode` は `OfflineRenderSupport::Unknown` を
  報告し続ける。`render()` 系の factory はテストスコープの
  `Verified` 宣言(`VerifiedOfflineNode` 委譲ラッパ)を fixture
  インスタンスにのみ適用し、宣言の正当性は一致性アサートで
  担保する。
  - Example: `attached.node.offline_support() == Unknown` を
    テスト内でアサートした上で render drive に wrapper を装着。
