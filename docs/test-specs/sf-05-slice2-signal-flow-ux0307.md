---
id: "TSPEC-jFtg0s5kOhAShEhr9GuU2J"
type: "test-spec"
status: "approved"
title: "SF-05 slice2 テスト仕様 — Signal Flow gesture/inspection/persist"
created: "2026-11-10"
updated: "2026-11-10"
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
    - "docs/specs/sf-05-slice2-signal-flow-ux0307.md"
    - "docs/designs/sf-05-slice2-signal-flow-ux0307-gesture-inspection-persist.md"
    - "docs/tasks/sf-05-slice2-signal-flow-ux0307.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sf-05-slice2-signal-flow-ux0307.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-05 slice2 テスト仕様 — Signal Flow gesture/inspection/persist

## Purpose

SF-UX-03/04/07（および SF-UX-02 の fixture 層回帰）の
headless 受入を fixture で固定する。実 OS/GPUI・実 Plugin・
Case ID 結合（#109/#103/#114）・probe request/capture
（SF-UX-06）は対象外（**NotRun**）。

## Feature

### Feature: Port Drag routing gesture（SF-UX-03）

Begin で fixation を固定し、hover は dry-run assessment、
drop で target 固定、confirm で 1 `ProjectOp::Routing`
commit、cancel/stale で project 不変、を検証する。

## Rules

- Rule: `routing_gesture_begin` は source port が routing
  model に存在すること・他 gesture family 非実行・view が
  mid-transfer でないことを要求し、fixation に view・
  source・live revision・capability generation・window
  owner を固定する。
- Rule: `routing_gesture_hover` は `RoutingPortAssessment`
  （`Compatible`/`NeedsMapping{channels}`/`Prohibited`）を
  返すだけで project revision・connections・undo・engine
  を一切変更しない。drop 後の hover は retarget を拒否する。
- Rule: `routing_gesture_drop` は candidate を
  `dropped_target` として固定し、空 canvas drop は gesture
  を終了する（confirm は `NoGesture`）。
- Rule: `routing_gesture_confirm` は revision・capability
  generation・window ownership のいずれかが fixation と
  異なれば gesture を clear して `Stale` を返し、project
  を変更しない。stale でない project 拒否（direction/role
  不整合・channel_map 未 consent）は gesture を生かした
  まま error を返す。
- Rule: 成功 commit は `session.apply` の 1 op で
  `Applied{revision = 旧 + 1}`、connection が +1、undo 1 回で
  connection が元に戻る（live revision は monotonic で
  undo でも巻き戻らない）。
- Rule: `NeedsMapping`（channel layout 不一致）は
  `ExplicitChannelMap` consent なしに commit できず、consent
  付きで `channel_map` が connection に保存される。
- Rule: `routing_gesture_cancel` は `CancelReason` を返し、
  project revision・connections・undo を不変に保つ。
- Rule: 1 view につき gesture は 1 つ — 2 つ目の begin・
  gesture 中の `pointer_down` は `GestureActive`。

### Feature: Signal Flow inspection focus と link 追従（SF-UX-04）

`SignalFlowInspectionFocus`（target/mode/follow_source/
session-only 履歴）が arrangement selection・
`inspection_focus`・link context と独立に動くこと、
link delivery が Follow mode のみを追従することを検証する。

- Rule: `signal_flow_inspect` は target 変更時に旧 target を
  `explore_back` に push し mode=Explore にする。
  `explore_back`/`explore_fwd` は serde skip で snapshot に
  残らない。
- Rule: `signal_flow_set_inspection_mode` で Explore 以外へ
  移ると履歴 stack は clear される。`explore_back`/
  `explore_forward` は stack を 1 段移動するだけで mode を
  変えない。
- Rule: link delivery は `signal_flow_entity_from_semantic`
  が `Some`（現状 Track のみ）かつ mode=Follow・
  follow_source filter 一致のときだけ Signal Flow target を
  更新する。Explore/Pinned・Lane/Clip/Source（`None`）は
  retarget しない。
- Rule: link delivery は receiver 側の状態更新のみ —
  `inspection_focus`（ordinary）・`EditingSelection`・
  object selections を変更せず、再 emit しない。
  `(receiver, event_id)` dedup で重複配送を抑止する。
- Rule: FollowSource filter（`View`/`Group`）が一致しない
  event は Follow focus を更新しない。

### Feature: per-view persist と safe recovery（SF-UX-07）

`SignalFlowViewState`（visual + inspection の visual 側）が
`ViewInstanceId` 単位で snapshot に往復し、v2→v3 migration
と malformed degrade が安全に動くことを検証する。

- Rule: `ViewState::signal_flow` は `Option` で、
  `WorkspaceSnapshot`/`ViewRecord` の既存往復に乗る —
  capture→to_json→from_json で per-view state が保持される。
- Rule: `WORKSPACE_SCHEMA = 3`・readable `[2, 3]`。v2
  snapshot（`signal_flow` 欠落）は parse 後 `None` のまま
  読め、schema は 3 に正規化される。schema 1/未知は reject。
- Rule: `signal_flow` payload が壊れている view は
  `deserialize_signal_flow` で `None` に degrade し、他の
  view/state を含む snapshot 全体は reject されない。
- Rule: `positions`（`BTreeMap<SignalFlowEntityRef, _>`）は
  enum key のため entry list として serialize され、往復で
  同一 map に戻る。
- Rule: app の `syn.signal_flow.visual`/`inspection`/`routing`
  は wire `view_id` を `ViewInstanceId` に解決し、未登録なら
  detached pool に adopt して state を `ViewState` に置く —
  view-1 と view-3 の state は独立し、`syn.signal_flow.view`
  は各 view の visual options を反映する。

### Feature: visual-only 不変の回帰（SF-UX-02 fixture 層）

slice-1 の検証を維持しつつ、gesture/inspection/persist 追加
後も visual op が project/engine を触らないことを確認する。

- Rule: `syn.signal_flow.visual` 全 action と
  `syn.signal_flow.view`/`inspection` の read 経路で
  `session.revision()`・project serialize・`graph_builds`・
  engine receipt が不変（gesture の preview/hover も同様）。

## Examples

- Given Bass/Kick strip fixture、when vox output→kick input
  で begin→hover→drop→confirm、then assessment=`compatible`、
  outcome=`applied`、connection +1、undo 1 回で元に戻る。
- Given gesture 中に `SetProjectName` で revision が進む、
  when confirm、then outcome=`stale`・connection 不変・
  gesture 消滅。
- Given channel layout 不一致 port 対、when hover→drop→
  confirm（channel_map 無し）、then project 拒否で gesture
  生存、`ExplicitChannelMap` consent で `applied`。
- Given 2 つの Signal Flow view（Follow/Explore/Pinned）、
  when link event（Track target）配送、then Follow のみ
  target 更新・Explore/Pinned/selection 不変。
- Given view-1=Expanded・view-3=Local の visual state、
  when capture→to_json→from_json、then 両 view の
  `signal_flow.visual.scope` が保持される。
- Given v2 JSON（`signal_flow` 無し）と壊れた `signal_flow`
  payload、when `WorkspaceSnapshot::from_json`、then 前者は
  `None` で migrate・後者は当該 view のみ `None` に degrade。

## Guarantees

- workspace test は `ProjectSession::apply`+`ProjectOp` と
  `FakeWindowHost` のみで完結し、GUI/engine なしで検証する。
- app test は `SessionRoutingEngine` 実体 + wire op
  （`op_signal_flow_*`）で JSON shape と project 不変を
  検証する。

## Non-goals

- SF-UX-06 probe ON/OFF・2 Window 同一 Semantic Point・
  capture dedup（#107 app/session wiring 待ち → NotRun）。
- SF-UX-08・実 GUI（High DPI/Multi Window/Keyboard/IME/
  a11y/Plugin focus）→ NotRun（実 OS gate）。
- `SemanticTarget` の Track 以外 → `SignalFlowEntityRef`
  の拡張写像（後続 slice の判断）。
- gesture の snapshot 跨ぎ再開（session-only の意図的設計）。
- #109/#103/#114 との Case ID 結合 → NotRun。

## Risk

- stale 検査の条件漏れは古い target への誤 commit を生む —
  revision/capability/window の 3 条件を個別 test で固定する。
- link 追従の条件漏れは Explore/Pinned や selection を
  汚染する — 否定形 assertion を各 mode で行う。
- tolerant deserialize が広すぎると本物の破損を隠す —
  degrade は `signal_flow` field 単位に限定し、他 field の
  `deny_unknown_fields` は維持する。
