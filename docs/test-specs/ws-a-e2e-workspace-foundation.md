---
id: "TSPEC-034QwZT6kAhQwa2bRBkZyu"
type: "test-spec"
status: "approved"
title: "WS-A 検証仕様 — Multi-View/LinkGroup/Command/Dock headless 受入"
created: "2026-11-02"
updated: "2026-11-02"
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
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/ws-a-workspace-foundation-design.md"
  derived-by: []
  verifies:
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# WS-A 検証仕様 — Multi-View/LinkGroup/Command/Dock headless 受入

検証本体: `crates/synvelis-workspace/tests/ws_a.rs` (+ 各 module の単体
test)。Fixture: `CounterIdGenerator` + `FakeWindowHost` で完全決定論。

## Feature: WS-A-01 単一正本・複数 View 独立

- Arrangement (Window A)・Mixer (Window B)・Inspector (Window B) を配置し、
  同一 `ProjectController` を共有。`ProjectOp` 適用で全 view の
  `ProjectUpdated` 配信が届く (last_notified 追跡)。
- Arrangement の Editing Selection・Inspector の Inspection Focus・
  各 view の Viewport は互いに独立 (他 view の selection 変更で不変)。
- view がどの window/dock にあっても `live()` は同じ正本を参照。

## Feature: WS-A-02 Link Group

- Arrangement で `Clip` 選択 → `track-selection` event で同 Group の
  Mixer は `linked_context` = その clip の所属 Track (lane→track 解決)。
  Mixer の Editing Selection は変化しない。
- Inspector (Follow) は linked semantic target に inspection_focus を
  更新。Inspector (Pinned) は変化しない。
- 別 Group の view には届かない (Group 間独立)。
- 受信側は event を再発信しない + `event_id` dedup で同一 event の
  二重適用・循環が起きない。
- mapping 不能 (例: 存在しない lane) なら配信自体を行わない。

## Feature: WS-A-03 Command Routing

- Arrangement で clip を選択した状態で `RemoveClip` → その clip だけが
  削除される (Project revision +1、undo で復元)。
- context menu 経由の `explicit_target_ids` → selection と無関係に明示
  target に適用。
- target 未解決 (selection 空・snapshot 無し) の破壊的 command は
  `Unresolved` で project 不変・他 window の stale selection 不使用。
- `TextInput`/`Ime`/`PluginGui`/`Modal` の input_context では
  Editing/Transport command が `Suppressed`。
- `expected_revision` が古い command は `StaleRevision` で拒否。
- View command (activate/zoom) は project を変更しない。
- 別 view/window の selection に暗黙フォールバックしない
  (Window A の削除が Window B の selection を対象にしない)。

## Feature: WS-A-04 Dock ライフサイクル

- `detach_to_new_window` で view が新 (仮想) window の dock へ移り、
  `owner_of`・`ViewInstanceId`・ViewState が維持される。reattach で
  元 window へ戻る。
- `host.create_window` 失敗注入 → move rollback: view は元の dock に
  残り、`MoveAborted` が出て ViewBusy が解除される。
- `commit_move` の destination 消失 (window 除去後 commit) →
  `TargetUnavailable` + rollback。
- `CloseRequested` → 取消方針 window は `Veto` で close cancel。
  sub-window は `Approve` + `DetachViews` で views が detached pool に
  保持される (ViewStateRegistry・ViewRegistry 維持)。
- `WorkspaceSnapshot` 保存→破壊→`restore_or_default` で view/dock/
  window 配置・link group が復元される。破損 JSON/未知 schema/
  モニター欠落では安全既定へ。
- MovePrepared 中の view への編集 command は `ViewBusy`。

## Feature: WS-A-05 RT/履歴分離

- Dock 操作 (detach/reattach/move/activate/set_ratio)・Window 操作
  (focus/close veto) で `ProjectController` の live_revision・undo
  履歴・`EngineStub.graph_builds` が不変。
- Project command 適用のみ `graph_builds` と revision が進む。

## Feature: WS-A-06 検証・記録

- `mise run test` 全 PASS、impl record に ruviform rev・検証表・
  未検証項目 (実機 lane) を記録。

## Guarantees

- 論理 `ViewInstanceId` が detach→reattach・window close・restore で
  保持される。
- 受信 link event が再発信・重複適用・Group 越境しない。
- 破壊的 command が target 未解決で実行されない。
- Project 正本が `ProjectController` 一箇所のみ。

## Non-goals

- 実 OS window/webview/DPI/IME/plugin GUI の実機検証 (ruviform #243)。
- 複数 project・remote DnD・tool system・automation 編集。

## Risk

- WindowKey 順序対応付けが snapshot windows 順とズレると復元先が
  誤る → restore で key↔placement の index 対応を明示テスト。
