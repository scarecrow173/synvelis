---
id: "SPEC-102UiCfInt001AaBbCcDdE"
type: "spec"
status: "approved"
title: "UI-C5 製品 GUI 統合 — Signal Flow Control Relation View/Inspector/Gesture（synvelis-app op surface + headless 検証）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/102"
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
    - "docs/specs/pc-c5-01-relation-inspector.md"
    - "docs/specs/pc-c5-02-03-gesture-stale-inspector.md"
    - "docs/specs/pc-c5-04-recording-status.md"
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/ui-sf-control-integration-design.md"
    - "docs/plans/ui-sf-control-integration.md"
    - "docs/tasks/ui-sf-control-integration.md"
    - "docs/test-specs/ui-sf-control-integration.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# UI-C5 製品 GUI 統合 — Signal Flow Control Relation View/Inspector/Gesture

## What Should Be Built

synvelis#102 の最終 slice。先行 slice（PC-C5-01/02-03/04）が
`synvelis-workspace` に実装した read model・gesture・recording status を、
製品 GUI が実際に消費する `synvelis-app` の `syn.*` op surface へ接続する。

`synvelis-project` の domain 正本（Control Domain / Relation Index /
Recording Transaction）は複写せず、既存の Workspace API への**接続のみ**を行う。
Ruviform 共通 Component/Gesture の再実装、旧 SolidJS/WebView の再導入は行わない。

1. **`syn.control.relation`**: view ごとの Control Relation inspection
   surface。`view|inspect|set_mode|set_follow_source|back|forward` の
   action を受け、Focus（Follow/Explore/Pinned + Explore 履歴）と
   `RelationView`（coverage・project_revision 付き）を返す。
   `view` + `anchor` は focus を動かさない stateless query。
   inspect は editing selection・`inspection_focus`・project・undo に
   一切触れない（inspect ≠ edit）。
2. **`syn.control.edge`**: Control Layer edge gesture
   （`ModulationSource → Parameter`）。`begin|preview|commit|cancel|state`。
   Begin で view・typed endpoint・project revision（CAS）・window epoch を
   固定し、Preview/Cancel は project 不変、Commit は fixated revision で
   `AddModulationAssignment` を 1 回だけ適用、stale は retarget せず drop。
   `syn.signal_flow.routing`（Audio Routing gesture）とは別 op・別 state
   slot であり、混同しない。
3. **`syn.control.relation` の per-view state**: `ViewState` に
   `control_relation: Option<ControlRelationViewState>` を追加し、
   target/mode/follow_source を snapshot 永続化、Explore 履歴は
   session-only（serde skip）とする。malformed payload は `None` へ
   安全劣化する（`signal_flow` と同じ tolerant-decode 契約）。
4. **`syn.recording.status` / `syn.recording.action`**: Workspace
   `recording_status` read model を JSON 化して公開し、
   `inspect|retry|rebase|abort` を既存 `ProjectSession` の
   `control_record_*` helper へ委譲する。Conflict/Prepare/Pending/
   Applied/Unknown の 5 state、型付き provenance、状態ごとの合法 action
   をそのまま wire に載せる。commit/rebase/abort のロジックを
   `synvelis-app` で再実装しない。
5. **Headless 検証**: `tests/uic5_app.rs`（op surface）と
   `tests/uic5.rs`（workspace focus API）で UI-C5-01〜04 を検証する。
   headless/GPUI-simulated の成功を実 Windows GUI・plugin window・
   実高 DPI・keyboard/IME・accessibility の合格として表示しない
   （UI-C5-05: 実 OS GUI レーンは Deferred/NotRun と正直に記録）。

## Why

Issue #102 の受入条件 UI-C5-01〜05 は domain semantics を workspace 層で
ほぼ実装済みだが、製品 GUI が呼べる wire surface が存在しなかった。
本 slice はその adapter 層を最小かつ型付きで提供し、
「headless で証明できる契約」と「実機 GUI でしか証明できない契約」を
明確に分離する。

## Acceptance Criteria

- `syn.control.relation` が Parameter → Automation Lane/Source/Assignment →
  影響先 Parameter の双方向探索と元 anchor への復帰（back/forward）を
  wire 越しに提供する。
- `syn.control.edge` が Control Layer edge gesture を Audio Routing と
  区別して提供し、inspect/preview/cancel が undo を生まないこと、
  commit が 1 `ProjectOp` = 1 undo 境界であること、stale が drop される
  ことを検証する。
- Follow/Explore/Pinned、follow_source、Explore 履歴、stale generation
  の drop が view ごとの state として動作する。
- `syn.recording.status`/`action` が 5 state・provenance・合法 action を
  typed に返し、rebase/abort が明示操作のみで行われる。
- 全検証は headless。実 GUI/実 OS/plugin/高 DPI/IME/a11y は
  Deferred/NotRun として記録し、headless 成功を GUI Pass としない。

## Non-goals

- `synvelis-project` 本体の変更（別 Worker 占有。read-only projection
  への接続のみ）。
- Ruviform 共通 Component/Gesture/Test Runner の再実装。
- 実 Window・高 DPI・native plugin GUI・実 device の検証
  （product usage が activate するまで Deferred）。
- Control Relation focus の link-event retargeting
  （`SemanticTarget` に `ControlEntityRef` への写像がなく、
  Follow focus は link/multi-selection 変化下で安定＝すり替わらない、
  が本 slice の契約）。
