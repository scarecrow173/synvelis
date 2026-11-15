---
id: "TSPEC-uivmVbNmQwErTyUiOpAsDf"
type: "test-spec"
status: "approved"
title: "SF-05 ui/ slice テスト仕様 — Signal Flow・Probe headless view-model"
created: "2026-11-18"
updated: "2026-11-18"
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
    - "docs/specs/sf-05-ui-viewmodel.md"
    - "docs/designs/sf-05-ui-viewmodel.md"
    - "docs/tasks/sf-05-ui-viewmodel.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sf-05-ui-viewmodel.md"
    - "docs/designs/sf-05-ui-viewmodel.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-05 ui/ slice テスト仕様 — headless view-model

## Purpose

`SignalFlowViewModel`/`ProbeViewModel` が wire contract を
正確に消費することを、`installFakeHost`（faithful `rvf.ipc.v1`
harness）経由の vitest で証明する。Rust service は実物を介さず、
fixture は `ui_ops.rs` の実 wire shape に寄せる。

## Feature

### Feature: `syn.signal_flow.*` consumer

view-model が 4 op の params/response を typed に運び、Debug
語彙・coverage/status・publish/active 分離・staleness token を
verbatim 保持する。

### Feature: `syn.probe.*` consumer

view-model が control の session/view-scoped 分離・state
projection・compare envelope を verbatim 保持し、engine 未接続・
incomparable を正直に表現する。

## Rules

- **R1 intent 完全一致**: 各 method が送る intent の `op`+`params`
  が wire contract と `toEqual` で一致する。`undefined` field を
  wire に出さない。session-scoped probe action は `view_id` を
  含まず、view-scoped は必ず `"view-{n}"` を含む。
- **R2 verbatim 写像**: wire response の Debug/kebab 語彙・
  `coverage`・`requested_mode`/`confirmed_mode`・
  `published_generation`/`observed_active_generation`・
  `engine_attached`・`quality.kind` を再命名・縮退・昇格しない。
- **R3 staleness**: visual ack の `view_generation` が
  `viewGeneration` を更新し、routing `stale` outcome を
  retarget せずそのまま返す。
- **R4 エラー経路**: wire error（`answerOpErr`）が code/message
  付きで reject され、VM state を更新しない。malformed
  `view_id` は事前検証せず wire reject を観測する。
- **R5 非捏造**: 初期状態は全て `undefined`/`0`。wire が送らない
  状態（binding・observation・gesture）を VM が生成しない。
  `engine_attached:false` は空集合 + `observation:null`。

## Examples

### Example: `syn.signal_flow.view` verbatim

`loadView` が `{view_id, scope:"local", root, roles, collapsed,
max_nodes, max_edges, expected_revision}` を完全一致で送り、
`coverage:"Stale"`・`published_generation:4`・
`observed_active_generation:4`・`confirmed_mode:null` を区別して
保持する。

### Example: visual/inspection/routing params

`move_node`/`set_group`/`fold`/`unfold`/`zoom`/`pan`/`link_focus`/
`set_scope`、inspection 5 action、routing 6 action がそれぞれ正確な
`action`+payload で intent を送り、ack の `view_generation`・
inspection focus・outcome（`applied`/`cancelled`/`stale`）を typed に返す。

### Example: probe view_id scoping

`request`/`set_mode`/`capture`/`stop_capture`/`remove` の params に
`view_id` が無く、`subscribe`/`unsubscribe`/`capture_for`/
`abort_capture` には `view_id:"view-3"` が必ず含まれる。

### Example: compare honesty

`compare` が `{request}` を verbatim 転送し、`incomparable`/
`conditional`/`aligned` の `quality.kind` をそのまま保持。
`engine_attached:false` → `{observation:null}`。

## NotRun

実 WebView host e2e・実 GPUI window・2-window・SF-UX-08/Windows
L2・#109/#103 oracle join は本 test-spec の対象外（NotRun 記録）。
