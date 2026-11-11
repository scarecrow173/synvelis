---
id: "SPEC-kX9mQvR2wTbLpNcJyH4sDf"
type: "spec"
status: "approved"
title: "SF-05 slice3 Signal Flow — probe subscriber/capture dedup の app/UI 配線（SF-UX-06 headless 側）"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/108"
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
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
    - "docs/specs/sf-05-slice2-signal-flow-ux0307.md"
    - "docs/specs/sf-04-slice4-app-wiring.md"
    - "docs/specs/sf-04-slice5-probe-subscriber-ownership.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-05-slice3-probe-subscriber-wiring.md"
  references: []
  deferred-by: []
---

# SF-05 slice3: Signal Flow — probe subscriber/capture dedup の app/UI 配線

## What Should Be Built

Issue #108（Signal Flow Editor / Semantic+Visual Hierarchy）の
**SF-UX-06 headless 側 slice**。SF-04 slice5（merge 済み、
PR #304）が `synvelis-workspace::ControlSession` に確立した
subscriber lease / capture owner モデル
（`probe_subscribe`/`probe_unsubscribe`/`probe_release_subscriber`/
`probe_capture_for`/`probe_abort_capture`・
`ProbePrincipal::{Session,View}`・`by_view` 逆引き）を、
app 層の IPC op と Signal Flow 画面へ配線する。

workspace 層の dedup/owner 不変量は既に正しい — 本 slice は
その**消費側境界**だけを実装し、lease モデルを再実装しない。

- **view-scoped subscriber 操作**: `syn.probe.control` に
  `subscribe`/`unsubscribe`/`capture_for`/`abort_capture`
  action を追加する。`view_id`（`"view-{n}"`）は
  `ViewInstanceId` へ解決され、lease は logical view に
  従う（window detach/move では解放しない）。
- **view close での lease 解放**: `syn.view.close`
  （`op_view_close`）が dock-domain view の close 時に
  `ControlSession::probe_release_subscriber(inst)` を呼び、
  当該 view の lease のみを解放する。window destroy の
  `ReleaseViews` disposition は既に `op_view_close` へ
  routing されるため同一路を通る。`DetachViews` では
  lease を保持する（`ViewInstanceId` 安定契約）。
  `syn.vcs.visual{action:"close"}` も同じ release を通す。
- **owner-only capture**: view は自分が lease を持つ key に
  対してのみ `capture_for` でき、abort は記録された owner
  のみ。session authority の `capture`/`stop_capture`/
  `remove`/`request`/`set_mode` は従来どおり別経路で、
  view 操作と混同しない。
- **state 投影**: `syn.probe.state` が各 binding の
  `holders`（lease principals）と `capture_owner`、および
  view→keys の `subscribers` 一覧を投影する。requested と
  RT-confirmed を潰さない既存契約を維持する。
- **Signal Flow 表示**: main view の probe subscription を
  point/kind/mode/state/holders/capture owner 付きの
  read-only table として Signal Flow tab に表示する。
  project revision・undo・DSP・runtime graph・PCM を
  一切変更しない（visual-only 契約の維持）。
- **IPC 型**: `ui/src/ipc.ts` の `OpName` に
  `"syn.probe.control"`/`"syn.probe.state"` を追加する
  （現状 probe ops が全欠落）。

## Why

SF-UX-06 は「Probe/Inspector 同 Point を複数 Window で共有し
Capture 重複なし」を要求する。lease/capture の正しさは
workspace 層で証明済みだが、app/UI がその API を消費する
経路が無いと、view close 時の lease 残存・view 単位の
capture 権限・holder 可視化が製品面で成立しない。

## Acceptance（本 slice 分）

- [ ] `syn.probe.control` の subscriber 操作
  （subscribe/unsubscribe/capture_for/abort_capture）が
  headless で動作し、同一 `(session, point, kind)` の複数
  view が 1 つの provider subscription（1 capture job）を
  共有する dedup が wire/view-model 層 test で証明される。
- [ ] view close で当該 view の lease のみが解放され、
  他 view の lease と provider subscription は残る。
  最終 lease 解放時のみ provider remove が走る。
- [ ] owner 以外の view による capture abort が拒否される
  （owner-only abort）。lease 非保持 view の subscribe 系
  操作は fail-closed。
- [ ] `syn.probe.state` に lease holders / capture owner /
  view→keys が投影され、Signal Flow surface の probe 表示が
  headless screen で検証される。実 GUI/OS 行（SF-UX-08、
  #114 gate）・実 2-window GUI 証明・#109 oracle 結合は
  **NotRun 継続**で Pass 扱いしない。

## Out of Scope

- 実 OS/GPUI・実 2-window・HiDPI/a11y の GUI 受入（#114 gate）。
- #109 independent oracle との Case ID 結合。
- view command（`syn.control.dispatch`）経由の probe toggle
  routing — shell/binding 層の action routing は本 slice の
  編集範囲外。probe ON/OFF は `syn.probe.control` op で
  検証し、画面側は状態表示に留める。
- `SignalFlowEntityRef`→`SemanticProbePoint` 変換 helper
  （任意追加枠。本 slice の op は明示 `point` param を取り、
  entity からの boundary 推測を行わない）。
- session detach / project open / engine detach 時の
  `probe_release_all` 要否 — design で「lease は session
  scope で生存し、project 差し替え時の runtime probe は
  既存の generation/stale 機構が処理する」方針を確認する。
