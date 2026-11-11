---
id: "TSPEC-pL4cVbNmQwErTyUiOpAsDf"
type: "test-spec"
status: "approved"
title: "SF-05 slice3 テスト仕様 — probe subscriber/capture dedup の app/UI 配線"
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
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-05-slice3-probe-subscriber-wiring.md"
    - "docs/designs/sf-05-slice3-probe-subscriber-wiring-design.md"
    - "docs/tasks/sf-05-slice3-probe-subscriber-wiring.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sf-05-slice3-probe-subscriber-wiring.md"
    - "docs/designs/sf-05-slice3-probe-subscriber-wiring-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-05 slice3 テスト仕様 — probe subscriber/capture dedup の app/UI 配線

## Purpose

SF-UX-06 の headless 側受入を fixture で固定する —
view-scoped subscriber 操作・dedup・view close 解放・
owner-only capture・state/screen 投影。実 OS/GPUI・
実 2-window GUI・#109 oracle Case ID 結合は対象外
（**NotRun**、#114 gate）。

## Feature

### Feature: view-scoped probe subscriber 操作

`syn.probe.control` の `subscribe`/`unsubscribe`/
`capture_for`/`abort_capture` が `view_id`→
`ViewInstanceId` 解決で session API へ届き、session
authority action と分離される。

## Rules

- Rule: 同一 `(session, point, kind)` への 2 view
  `subscribe` は同一 `key` を返し、provider
  `probe_request` は最初の lease 取得時のみ呼ばれる
  （2 回目以降は dedup — mode も共有 binding のまま
  変わらない）。
- Rule: `subscribe` の再実行は idempotent — 同一 view
  の重複 subscribe は lease を増やさず provider を
  再呼び出ししない。
- Rule: `op_view_close`（`syn.view.close`）は当該
  `ViewInstanceId` の lease のみ解放する — 他 view の
  lease と provider subscription は残り、最終 lease
  解放時のみ `probe_remove` 意味論（`Missing` 記録 +
  binding `stopped`）が走る。
- Rule: `capture_for` は lease 保持 view のみ成功し、
  owner は `ProbePrincipal::View` として記録される。
  lease 非保持 view の `capture_for`/`unsubscribe`/
  `abort_capture` は `NotSubscriber` → `INVALID_PARAMS`。
- Rule: `abort_capture` は記録 owner のみ成功 —
  lease を持つが owner でない view は
  `NotCaptureOwner` → `INVALID_PARAMS`。session
  authority の `capture`/`stop_capture` は従来どおり
  view 操作と独立。
- Rule: view が capture owner のまま close されても
  capture は session authority へ retreat し、残存
  subscriber のために止まらない（workspace 層契約の
  app 経由確認）。
- Rule: `syn.probe.state` の各 binding が `holders`
  （`"session"`/`"view-{n}"`）と `capture_owner` を、
  top-level が view→`keys` の `subscribers` を投影
  する。requested/confirmed の区別は維持される。
- Rule: Signal Flow tab が main view の probe
  subscription を point/kind/mode/state/holders/
  capture owner 付き read-only table で表示し、
  subscribe→行出現・unsubscribe→行消失の on/off が
  headless screen state で検証される。project
  revision・undo は probe 操作で不変。
- Rule: engine 未接続では subscriber action が
  `SERVICE_ERROR` で fail-closed、state が
  `engine_attached:false` + `subscribers:[]` を返す。

## Examples

### Example: 2 view dedup + close release

Given engine attached、master strip の probe point、
2 つの param target。

1. `subscribe{view_id:"view-1", ...}` → key K、holders
   `[view-1]`。
2. `subscribe{view_id:"view-2", same point/kind}` →
   同一 key K、holders `[view-1, view-2]`、provider
   request は 1 回のみ。
3. `syn.view.close{view_id:"view-2"}` → holders
   `[view-1]`、binding は live のまま。
4. `syn.view.close{view_id:"view-1"}` → 最終 lease で
   binding `stopped`（provider remove 意味論）。

### Example: owner-only abort

1. `subscribe view-1` + `subscribe view-2`（同一 key）。
2. `capture_for{view_id:"view-1"}` → owner `view-1`。
3. `abort_capture{view_id:"view-2"}` → `INVALID_PARAMS`
   （`NotCaptureOwner`）。
4. `abort_capture{view_id:"view-1"}` → 成功、owner
   消去。

### Example: screen projection

1. `subscribe{view_id:"view-1"}` 後の
   `main_screen_cell().signal_flow_probes` に 1 行
   （point/kind/mode/holders/capture owner 入り）。
2. `main_screen_spec` の `signal_flow.probes` table が
   同じ行を render、`selection: None`。
3. `unsubscribe` 後は行が消える。
