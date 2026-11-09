---
id: "TSPEC-034UuhtnvPrY3vXkkcgAXI"
type: "test-spec"
status: "approved"
title: "SF-05 slice1 テスト仕様 — Signal Flow headless"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
    - "docs/designs/sf-05-slice1-signal-flow-headless-graph-visual-state-status.md"
    - "docs/tasks/sf-05-slice1-signal-flow-headless.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-05 slice1 テスト仕様 — Signal Flow headless

## Purpose

SF-UX-01/02/05 の headless 受入を fixture で固定する。
実 OS/GPUI・実 Plugin・Case ID 結合（#109/#103/#114）は対象外
（**NotRun**）。

## Feature

### Feature: RoutingModel の意味を保つ hierarchy 投影（SF-UX-01）

Bass strip→Music Bus strip→Master strip の Main edge、
Kick→processor Sidechain edge、Pre/Post Send→Return bus edge、
processor chain order が、Overview/Local/Expanded/Collapse/
Role Filter の各モードで意味を失わず表示される。

## Rules

- Rule: edge は `RoutingConnection` 1 件 = 1 `SignalFlowEdge`。
  `kind = Send(id)` の connection は `sends` の tap を持つ
  `role = Send` edge 1 本のみ — connection/send config の
  二重 edge を出さない。
- Rule: `Expanded` では `chain.processors` 順に processor node
  と `role = Chain` の合成 edge が出る。`collapsed` に含まれる
  strip は processor 展開を畳み Overview 相当に戻るが、
  strip node とその接続 edge は残る。
- Rule: `Local{root}` は root 解決先 strip とその直接近傍の
  induced subgraph のみ返す。存在しない root は
  `coverage = Unavailable`。
- Rule: `roles` filter は edge role（Main/Sidechain/Send/Chain）
  のみを落とし、node と semantic 情報は保持する。
- Rule: `expected_revision` が live と異なれば `coverage = Stale`
  で nodes/edges は空。`History` scope は `coverage = Unsupported`。
- Rule: `signal_flow_graph` は project/session を mutate しない
  — 呼出し前後で `session.revision()` と routing model が不変。

### Feature: visual-only 操作の非破壊性（SF-UX-02）

`visual_move_node`/`visual_set_group`/`visual_fold`/
`visual_set_zoom`/`visual_set_pan`/`visual_set_link_focus`/
`visual_set_scope` と `syn.signal_flow.visual` op は
`SignalFlowVisualState` のみを変更し、`visual_revision` を +1
する。`session.revision()`・undo 深さ・`graph_builds`・
engine receipt は不変。

- Rule: visual state は session/project/engine への参照を持たない
  純粋値型。op の ack は `{ok, view, view_generation}` のみ。
- Rule: `syn.signal_flow.view` は options 未指定分をその view の
  visual state から補完し、`view_generation` を echo する。

### Feature: 正直な状態表示（SF-UX-05）

`SignalFlowStatus` は live `project_revision`・receipt の
activation state・planned/published/observed/last_good
generation・`source_map` の revision/generation・stale 判定・
probe binding 一覧を運ぶ。node は semantic state
（Ready/Unresolved）と runtime state（Unobserved/Resolved/
Unavailable/Unsupported/NotLowered/Stale）を分けて持つ。

- Rule: `Published`（published_generation のみ、observed なし）
  と `Active`（observed_active_generation 一致）は別状態として
  表示される — publish 成功を active と偽装しない。
- Rule: engine 未接続（evidence=None）は `engine_attached=false`
  ・node runtime=`Unobserved` で、空の成功表示にしない。
- Rule: probe binding は `requested_mode`/`confirmed_mode`/
  `ProbeResolveState` を分けて運ぶ — Requested/QueuedForBinding
  （RT 未確認）と BoundActive（RT 観測）を潰さない。

## Examples

- Given Bass→MusicBus→Master + Kick→Sidechain + Pre/Post Send→
  Return + 2 processor chain の fixture、when Overview/Expanded/
  Local/Collapse/RoleFilter の各 options で投影、then 各 edge が
  期待 role/tap/enabled を保持し connection id が重複しない。
- Given engine に publish 済み・未 run の receipt、when
  `signal_flow_view`、then `activation=Published{generation}`・
  `observed_active_generation=None`・node runtime=Resolved{g}。
- Given `syn.signal_flow.visual` で move/fold/zoom/focus を連続
  適用、when `session.revision()`/`graph_builds`/receipt を比較、
  then 全て不変で `view_generation` のみ増分。

## Guarantees

- workspace test は `ProjectSession::apply`+`ProjectOp` のみで
  fixture を構築し、GUI/engine なしで完結する。
- app test は `SessionRoutingEngine` 実体で publish→run の
  receipt 遷移を使い、Publish≠Active を実 evidence で検証する。

## Non-goals

- Port Drag/編集 op・consent/late receipt（SF-UX-03）。
- Follow/Explore/Pinned/2Window/Dock/Close/Focus（SF-UX-04）。
- Probe request/capture op・共有 dedup（SF-UX-06）。
- Workspace Schema v3 migration・visual state 永続化（SF-UX-07）。
- 実 OS/GPUI/HighDPI/A11y/IME（SF-UX-08 → NotRun）。
- #109/#103/#114 との Case ID 結合（→ NotRun）。

## Risk

- edge dedup を誤ると同一 send が 2 edge に見える — fixture で
  connection/send 1:1 を明示検証する。
- visual op が誤って session を触ると Undo/PCM を壊す —
  revision/graph_builds/receipt の三点不変で検出する。
