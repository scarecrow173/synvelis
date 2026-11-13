---
id: "TSPEC-034X53lawfMjkjYxK3vCLJ"
type: "test-spec"
status: "approved"
title: "Workspace Model Test Spec — headless verification case map"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/2"
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
    - "docs/plans/workspace-model.md"
    - "docs/tasks/workspace-model-task.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/workspace-model-design.md"
  derived-by: []
  verifies:
    - "docs/designs/workspace-model-design.md"
    - "docs/specs/workspace-model.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Workspace Model Test Spec — headless verification case map

## Purpose

Issue #2 の受入条件（WSM-AC-01〜09）のうち headless で検証可能な部分を、
既存 fixture test と新規 `workspace_model.rs` へ Case ID（WSM-T-*）で
対応付け、`evidence/sv2/` 台帳の Expected/Observed/NotRun の根拠を固定する。
実機未検証項目は NotRun/Deferred として明示し、headless 合格を実機合格と
混同しない。

## Feature

Workspace Model の headless 検証 — Window/View 寿命独立・Selection/Link
Group/Command Routing・Dock Move rollback・Window Close/Destroy・
Layout Restore・再生中 churn。

## Rules

- **WSM-T-01 共有 Project・独立 View State**: 2 Window に複数 View を配置
  しても `ProjectSession` は 1 つで、project 変更は全 view へ revision 配信
  され、view ごとの selection/inspection/viewport は独立。
  Oracle: revision_inbox・view state の独立性。
- **WSM-T-02 View State/Focus の Window 独立性**: window を跨いでも
  `ViewState`/`FocusState` が混線しない。
- **WSM-T-03 Foreign Window command 拒否**: `origin_window`/`origin_view`
  が別 window の command は拒否され、誤配送しない。
- **WSM-T-04 Link Group 意味的連携**: Clip→Track 等の semantic mapping、
  pinned inspector 保持、group 間独立。editing selection は自動複製しない。
- **WSM-T-05 通知ループ防止**: link event の dedup で再発行・無限ループが
  起きない。
- **WSM-T-06 Command Routing 文脈**: explicit target 優先、target 解決不能は
  `Unresolved`（fallback なし）、`expected_revision` 不一致は `Rejected`、
  TextInput/Ime/PluginGui/Modal は `Suppressed`。
- **WSM-T-07 Undo lane 分岐**: `View`→project history、`TextInput`/`Ime`→
  text lane、`PluginGui`/`Modal`→suppressed。
- **WSM-T-08 再生中 churn（新規 test）**: `transport.playing=true` のまま
  dock move/detach/close veto/restore を実行しても playing・revision・
  undo stack が不変。
- **WSM-T-09 Detach/Reattach + Move rollback**: detach→reattach で view
  id/state 保持、move 失敗（dest destroy/host create 失敗/ViewBusy）は
  abort で source 無変更 + `MoveAborted`。
- **WSM-T-10 Close veto + Disposition**: dirty/guard で `Veto`、approve で
  `DetachViews`（pool 保全）または `ReleaseViews`（remove）。
- **WSM-T-11 Window 消失（Destroyed）**: close を介さない destroy でも
  project/session/view registry が残る（detach pool または release）。
- **WSM-T-12 Layout save/restore + 破損復元**: versioned snapshot の
  復元、malformed/corrupt payload の reject または safe default +
  detached pool 退避。
- **WSM-T-13 Monitor 切断 fallback**: saved bounds の monitor 不在時に
  available geometry へ fallback。
- **WSM-T-14 Layout op は project/engine を触らない**: revision/
  `graph_builds`/undo stack が dock/window/layout churn で不変。
- **WSM-T-15 Adopted window/view**: driver/composition-root 生成の
  window/view を `adopt_window`/`adopt_view` で host 作成なしに登録し、
  destroy 時も pool へ退避。

## Examples

Case → test 対応表（headless。実機は §NotRun）:

| Case | 検証内容 | test（crate::file::fn） |
| --- | --- | --- |
| WSM-T-01 | 共有 project・独立 view state（2 window） | `synvelis-workspace::ws_a::ws_a_01_shared_project_independent_view_state`; `synvelis-app::sv113_dock::two_windows_share_project_and_survive_window_close`; `synvelis-app::two_window::both_windows_share_one_project_session` |
| WSM-T-02 | view state/focus の window 独立 | `synvelis-app::sv113_dock::view_state_and_focus_are_independent_per_window`; `ws_a_01`（view-local selection/inspection） |
| WSM-T-03 | foreign window command 拒否 | `synvelis-app::two_window::foreign_window_commands_are_rejected`; `synvelis-app::two_window::owning_window_commands_still_apply_after_foreign_rejection`; `synvelis-app::sv113_dock::cross_window_origin_view_is_rejected` |
| WSM-T-04 | link group semantics | `synvelis-workspace::ws_a::ws_a_02_link_group_semantics`; `synvelis-workspace::ed_a` link/context tests（`ed_a_05` 系） |
| WSM-T-05 | link dedup/no re-emit | `synvelis-workspace::ws_a::ws_a_02b_link_dedup_no_reemit` |
| WSM-T-06 | command routing 文脈 | `synvelis-workspace::ws_a::ws_a_03_command_routing` |
| WSM-T-07 | undo lane 分岐 | `synvelis-workspace::ed_a` の `edit.undo`/`edit.redo`/`text_lane_undos` tests（ed_a.rs L633〜740 系） |
| WSM-T-08 | 再生中 churn（新規） | `synvelis-workspace::workspace_model::playback_dock_window_churn_preserves_transport_revision_and_history`（本 slice で追加） |
| WSM-T-09 | detach/reattach/move rollback | `synvelis-workspace::ws_a::ws_a_04_detach_reattach_move_rollback`; `synvelis-app::dock_ops::dock_detach_reattach_round_trips_through_pool`; `synvelis-app::dock_ops::saga_destination_destroyed_before_ready_aborts`; `dock_ops` の host-create 失敗/ViewBusy abort 系 |
| WSM-T-10 | close veto + disposition | `synvelis-workspace::ws_a::ws_a_04b_window_close_veto_and_detach`; `synvelis-app::close_veto` 全件; `synvelis-app::dock_ops::destroyed_with_release_views_closes_docked_views` |
| WSM-T-11 | window 消失（Destroyed） | `synvelis-app::sv113_dock::two_windows_share_project_and_survive_window_close`; `synvelis-workspace::adopt` adopted-window destroy 系; `synvelis-app::shell::destroyed_window_tears_down_its_views` |
| WSM-T-12 | layout save/restore + 破損 | `synvelis-workspace::ws_a::ws_a_04c_layout_save_restore` |
| WSM-T-13 | monitor fallback | `synvelis-workspace::preset::missing_monitor_falls_back_to_available_geometry`; `preset::two_window_preset_builds_split_layout` |
| WSM-T-14 | layout op が project/engine 非接触 | `synvelis-workspace::ws_a::ws_a_05_layout_ops_leave_project_and_engine_alone`; `synvelis-app::dock_ops::dock_transfer_preserves_project_and_transport_state` |
| WSM-T-15 | adopt window/view | `synvelis-workspace::adopt` 全件 |

### NotRun / Deferred（headless 範囲外）

| Case | 内容 | 状態 | 後続 gate |
| --- | --- | --- | --- |
| WSM-T-16 | 実 OS 2 Window/Dock drag/pixel/IME/keyboard/focus | NotRun | synvelis#114, ruviform#313/#314（GUI-L2/EXT-L3） |
| WSM-T-17 | 実 plugin editor / NativeSurfaceSlot 実統合 | NotRun | synvelis#176 系, ruviform#388 consumer（EXT-L3） |
| WSM-T-18 | window-owned job cancel / session job 存続の実挙動 | NotRun | `CallOutcome::Job` を返す op 未実装。`UiRuntime` 契約（自動 cancel しない）のみ設計記録 |
| WSM-T-19 | 実 RT 計測（dock/window op 中の glitch/alloc） | NotRun | synvelis#294, ruviform consumer gate |
| WSM-T-20 | macOS/Linux GUI lane | Deferred | POLICIES.md — 使用要件未発生 |

## Guarantees

- headless test は「意味論 oracle」であり、OS focus/IME/pixel/実 plugin を
  保証しない。実機項目は必ず NotRun/Deferred で記録する。
- `ProjectSession`/`EngineStub`/`ControlSession`/job table が
  Window/View 操作で不変であることを revision・`graph_builds`・undo・
  transport・view state の前後一致で観測する。
- 失敗系（move abort/close veto/destroy/layout 破損）は「source 無変更」
  「pool 保全」「safe default」を oracle とする。

## Non-goals

- 実 OS Window/Dock/IME/Keyboard/Pixel の検証（#114 gate）。
- 実 plugin editor/NativeSurfaceSlot 統合（#176 gate）。
- 製品 shell の multi-window 配線（`ShellWindowHost` は single-window）。
- `CallOutcome::Job` を返す op の新設。
- Ruviform port の変更。

## Risk

- 既存 test が WSM-T-* を既に網羅しているため、新規 test は WSM-T-08 の
  playback churn のみに限定する。過剰な fixture 追加は contract を
  hard-code 化するリスクがあるため避ける。
- `ws_a_05` は playing 未設定のため「再生中」の qualifier を直接証明
  しない — WSM-T-08 で補完する。
