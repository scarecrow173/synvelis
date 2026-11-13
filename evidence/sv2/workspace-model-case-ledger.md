# Workspace Model — Case 台帳 (Issue #2)

作成: 2026-11-13 / branch: `feat/2-workspace-model`
対象: `docs/specs/workspace-model.md`・
`docs/designs/workspace-model-design.md`・
`docs/test-specs/workspace-model.md`

語彙は POLICIES.md §2/§3 に従う。**headless Pass は実機 Pass ではない**。
実機未検証項目は NotRun/Deferred として記録し、後続 gate を明示する。

## 1. WSM-AC → WSM-T Case → test → 結果

| WSM-AC | Case | 検証内容 | test（headless） | Expected | Observed | Result |
| --- | --- | --- | --- | --- | --- | --- |
| AC-01 | — | GPUI-neutral View/Driver Port + Plugin Slot を反映した Workspace 図、WebView 固有記述の legacy 注記化 | 文書レビュー（design §Workspace 構造図 + legacy 注記） | 図が `WindowDriver`/`ViewHost`/`NativeSurfaceSlot` を含み WebView 前提が legacy 注記へ分離 | design doc に反映済み。`WindowDriver`/`DriverEvent::Windowed`/`post_event` fire-and-forget/`NativeSurfaceSlot`/`SurfaceSlotId`/`SlotLease`/`ViewHost`/`ViewSpec` を実コードの型名で記述 | **Pass（設計レビュー）** |
| AC-02 | WSM-T-01/02/08/14/15 | Project/RT/Probe/Job/State の寿命が Window/View 移動と独立 | `ws_a_01`, `ws_a_05`, `workspace_model::playback_dock_window_churn_*`, `sv113_dock::two_windows_share_project_and_survive_window_close`, `adopt::*`, `uic5`/`sv48`/`vcs04` の view-close/dock-move 系 | revision・graph_builds・undo・transport・view state が churn で不変。window destroy で project/session 残存 | `mise run test synvelis-workspace` / `synvelis-app` で全件 pass（exit 0）。新規 playback churn test も pass | **Pass（headless）**。RT 実計測・実 job cancel は WSM-T-18/19 NotRun |
| AC-03 | — | 統合設計書の不変条件・State 所有者・識別子・Close/Move/Restore の照合 + API 確定前論点 | design §識別子体系/§寿命独立性/§識別子とライフサイクルの照合/§未決事項 | 不変条件 6 項目の実装照合と未決事項の列挙 | design doc に照合表（全 6 項目「実装済み」）と未決事項 5 件を記録 | **Pass（設計レビュー）** |
| AC-04 | WSM-T-01/02 | 複数 Arrangement/Mixer/Inspector 相当を別 Window へ配置、Project 共有・調査状態独立 | `ws_a_01_shared_project_independent_view_state`, `sv113_dock::view_state_and_focus_are_independent_per_window`, `two_window::two_windows_attach_their_own_views`/`both_windows_share_one_project_session` | 2 Window・複数 view type で project 共有・view state/focus 独立 | pass（exit 0） | **Pass（headless）** |
| AC-05 | WSM-T-04/05 | Link Group: Clip→Track 意味的連携・Pinned 保持・Group 独立・通知ループ防止 | `ws_a_02_link_group_semantics`, `ws_a_02b_link_dedup_no_reemit`, `ed_a` link/context tests, `uic5_03_*` | semantic mapping のみ配信・pinned 保持・group 独立・dedup で loop なし・editing selection 非複製 | pass（exit 0） | **Pass（headless）** |
| AC-06 | WSM-T-03/06/07 | Delete/Ctrl+Z/Space/Context Menu を複数 Window・入力欄・IME・plugin GUI 文脈で検証、誤対象編集なし | `ws_a_03_command_routing`, `ed_a` undo lane tests, `two_window::foreign_window_commands_are_rejected`, `sv113_dock::cross_window_origin_view_is_rejected`, `shell::rvf_command_*` | explicit target 優先・unresolved fallback なし・入力文脈 suppressed・foreign window 拒否・undo lane 分岐 | pass（exit 0）。実 IME/実 keyboard/実 plugin GUI は WSM-T-16/17 NotRun | **Pass（headless）** / 実機 **NotRun** |
| AC-07 | WSM-T-09/10/11/12/13/15 | Detach→Reattach・Move rollback・Close veto・Window 消失・layout 破損/monitor 切断復元 | `ws_a_04*`, `ws_a_04b`, `ws_a_04c`, `adopt::*`, `preset::missing_monitor_*`, `dock_ops` saga/destroyed tests, `close_veto::*`, `shell::destroyed_window_tears_down_its_views` | detach/reattach で id/state 保持・失敗時 source 無変更・veto で close 阻止・destroy で pool/release・破損で safe default・monitor 不在で fallback | pass（exit 0） | **Pass（headless）** |
| AC-08 | WSM-T-08/14 | 再生中の Dock/Window 操作で RT 処理・Project 編集履歴が意図せず変化しない | `workspace_model::playback_dock_window_churn_preserves_transport_revision_and_history`（新規）, `ws_a_05_layout_ops_leave_project_and_engine_alone`, `dock_ops::dock_transfer_preserves_project_and_transport_state` | playing=true のまま churn しても transport/revision/graph_builds/undo stack 不変 | pass（exit 0）。新規 test は `transport.play` command で playing を立て、detach/reattach/move/close-veto/window-event churn 後に playing・revision・graph_builds・undo roundtrip・view state を検証 | **Pass（headless）**。実 RT glitch/alloc 計測は WSM-T-19 NotRun |
| AC-09 | — | Ruviform/Synvelis Issue・test の相互参照・未検証 OS 明示 | spec/design/test-spec の relations.references + 本台帳 §3 | 関連 Issue/PR が references/related に列挙され、未検証 OS/機能が NotRun/Deferred | spec `references` に ruviform#181/#182/#237/#240/#243/#294/#313/#314/#386/#388/#464、source に synvelis#2/#3/#113/#114。design/test-spec に #114/#176/#294/#313/#314 を Deferred gate として記録 | **Pass（文書）** |

## 2. WSM-T Case 詳細（headless 実施分）

| Case | crate::test | 状態 | 備考 |
| --- | --- | --- | --- |
| WSM-T-01 | `synvelis-workspace/tests/ws_a.rs::ws_a_01_shared_project_independent_view_state` | Pass | 2 window・Arrangement+Mixer+Inspector 相当、project 共有・state 独立 |
| WSM-T-01 | `synvelis-app/tests/sv113_dock.rs::two_windows_share_project_and_survive_window_close` | Pass | HeadlessWindowHost 2 window・window close 後も project/session 生存 |
| WSM-T-01 | `synvelis-app/tests/two_window.rs::both_windows_share_one_project_session` | Pass | FakeDriver 2 window・1 SynvelisApp/ProjectSession |
| WSM-T-02 | `synvelis-app/tests/sv113_dock.rs::view_state_and_focus_are_independent_per_window` | Pass | view state・focus→active-view 解決が window ごとに独立 |
| WSM-T-03 | `synvelis-app/tests/two_window.rs::foreign_window_commands_are_rejected` + `owning_window_commands_still_apply_after_foreign_rejection` | Pass | `origin_window` foreign → 拒否、owning window は継続動作 |
| WSM-T-03 | `synvelis-app/tests/sv113_dock.rs::cross_window_origin_view_is_rejected` | Pass | `origin_view` が別 window の view → 拒否 |
| WSM-T-04 | `synvelis-workspace/tests/ws_a.rs::ws_a_02_link_group_semantics` | Pass | Clip→Track mapping・pinned inspector・group 独立・editing selection 非複製 |
| WSM-T-04 | `synvelis-workspace/tests/ed_a.rs`（link/context 系）・`uic5_03_*` | Pass | inspection focus mode/follow source・explore history |
| WSM-T-05 | `synvelis-workspace/tests/ws_a.rs::ws_a_02b_link_dedup_no_reemit` | Pass | dedup で再発行なし・通知 loop なし |
| WSM-T-06 | `synvelis-workspace/tests/ws_a.rs::ws_a_03_command_routing` | Pass | explicit target 優先・unresolved・stale revision・kind mismatch・入力文脈 suppressed |
| WSM-T-07 | `synvelis-workspace/tests/ed_a.rs`（`edit.undo`/`edit.redo`/`text_lane_undos` 系、L633〜740） | Pass | View→project history・TextInput/Ime→text lane・PluginGui/Modal→suppressed |
| WSM-T-08 | `synvelis-workspace/tests/workspace_model.rs::playback_dock_window_churn_preserves_transport_revision_and_history` | Pass | **本 slice で新規追加**。playing 中の churn で transport/revision/graph_builds/undo/view state 不変 |
| WSM-T-09 | `synvelis-workspace/tests/ws_a.rs::ws_a_04_detach_reattach_move_rollback` | Pass | detach→new window・reattach・move rollback（dest destroy/host create 失敗/ViewBusy） |
| WSM-T-09 | `synvelis-app/tests/dock_ops.rs::dock_detach_reattach_round_trips_through_pool`・`saga_destination_destroyed_before_ready_aborts` 他 saga 系 | Pass | app 層 dock op の 2 相 move・new-window saga 失敗 rollback |
| WSM-T-10 | `synvelis-workspace/tests/ws_a.rs::ws_a_04b_window_close_veto_and_detach` | Pass | close veto・approved close → DetachViews pool |
| WSM-T-10 | `synvelis-app/tests/close_veto.rs` 全件（dirty veto・clean approve・quit partial veto） | Pass | app-global close guard・last-window 判定 |
| WSM-T-10 | `synvelis-app/tests/dock_ops.rs::destroyed_with_release_views_closes_docked_views` | Pass | ReleaseViews disposition で view remove |
| WSM-T-11 | `synvelis-app/tests/sv113_dock.rs::two_windows_share_project_and_survive_window_close` | Pass | window destroy で project/session/view registry 残存 |
| WSM-T-11 | `synvelis-workspace/tests/adopt.rs::adopted_window_destroyed_detaches_views_to_pool` | Pass | adopted window の destroy → pool 退避 |
| WSM-T-11 | `synvelis-app/tests/shell.rs::destroyed_window_tears_down_its_views` | Pass | shell 層の view teardown（UI registry） |
| WSM-T-12 | `synvelis-workspace/tests/ws_a.rs::ws_a_04c_layout_save_restore` | Pass | versioned snapshot 復元・malformed schema reject・corrupt layout → detached pool 退避 |
| WSM-T-13 | `synvelis-workspace/tests/preset.rs::missing_monitor_falls_back_to_available_geometry` + `two_window_preset_builds_split_layout` | Pass | monitor 不在時の geometry fallback・2 window preset |
| WSM-T-14 | `synvelis-workspace/tests/ws_a.rs::ws_a_05_layout_ops_leave_project_and_engine_alone` | Pass | dock/window/focus churn で revision・graph_builds・undo 無変化 |
| WSM-T-14 | `synvelis-app/tests/dock_ops.rs::dock_transfer_preserves_project_and_transport_state` | Pass | app 層 dock transfer で project truth 不変 |
| WSM-T-15 | `synvelis-workspace/tests/adopt.rs` 全件（adopt_window/adopt_view 重複・未 adopt window・destroy） | Pass | host 作成なしの登録・destroy 時 pool 退避 |

## 3. NotRun / Deferred（headless 範囲外 — POLICIES.md §2/§3）

| Case | 内容 | 状態 | Required/Deferred | 後続 gate |
| --- | --- | --- | --- | --- |
| WSM-T-16 | 実 OS 2 Window/Dock drag/pixel/IME/keyboard/OS focus/複数モニター実挙動 | **NotRun** | Required（GUI-L2/EXT-L3）だが本 slice 範囲外 | synvelis#114（実機 GUI gate）、ruviform#313/#314 |
| WSM-T-17 | 実 plugin editor / `NativeSurfaceSlot` 実統合 | **NotRun** | Deferred（EXT-L3、plugin host 未接続） | synvelis#176 系、ruviform#388 consumer |
| WSM-T-18 | window-owned job cancel / session-owned job 存続の実挙動 | **NotRun** | `CallOutcome::Job` を返す `syn.*` op が未実装のため headless fixture なし。`UiRuntime` 契約（Window close/destroy は job を自動 cancel しない、`cancel_window_jobs` は明示呼出）を設計不変条件として design §寿命独立性に記録 | 製品 job op 実装時（#113/#175 系） |
| WSM-T-19 | 実 RT 計測（dock/window op 中の audio glitch/alloc/no-alloc） | **NotRun** | Required だが RT oracle は stub レベルのみ | synvelis#294、ruviform consumer gate |
| WSM-T-20 | macOS/Linux GUI lane | **Deferred** | POLICIES.md — 使用要件未発生 | 当該 OS での GUI 検証要求時 |
| WSM-T-21 | `ShellWindowHost` multi-window 製品配線（`create_window` 有効化） | **NotRun** | 製品実装は #113/#175 Owner。headless `HeadlessWindowHost`/`FakeWindowHost` のみ検証 | synvelis#113/#175 |

## 4. doc-driven-dev-graph router の状態（外部 blocker）

`node .claude/skills/doc-driven-dev-graph/scripts/route_graph.js --focus docs/specs/workspace-model.md --json` は
`current: "probe"`・`status: "blocked"`・`edgeId: null` を返す。blockers は
`artifact-graph` + 多数の既存 `broken-relation`（rcd/tim/vcs/pc/sf 等の
無関係文書の deferred relation が docs パスとして解決不能）。**本 task の
新規文書起因の finding ではなく repo 全体の baseline 問題**であり、編集範囲外の
ため修復しない。新規文書の relation は doc-status lint / audit で個別に
整合を確認する（verification 記録 V 系参照）。
