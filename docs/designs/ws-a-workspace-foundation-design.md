---
id: "DESIGN-034QwYMaHNxxpkZgLxPjx6"
type: "design"
status: "approved"
title: "WS-A Workspace 基盤設計 — View/Selection/LinkGroup/Command/Dock headless PoC"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/36"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
  implemented-by: []
  depends-on:
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
    - "docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/workspace-docking-state-architecture.md"
    - "docs/designs/vs-a-project-recording-foundation-stable-id-command-poc.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
    - "docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md"
    - "docs/designs/workspace-docking-state-architecture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# WS-A Workspace 基盤設計 — View/Selection/LinkGroup/Command/Dock headless PoC

## Context

Issue #36 のスコープ 6 項目を、ADR Option 1 (新クレート
`synvelis-workspace` + ruviform 契約型消費) に沿って実装するための
詳細設計。正本 `docs/designs/workspace-docking-state-architecture.md`
の論理構造を headless で検証可能な具体型・責務・不変条件に落とす。

前提: ruviform `ruviform-ui` rev は #181 (PR #237) / #182 (PR #240) を
含む origin/main 祖先 (`Cargo.lock` に記録)。`synvelis-project` の
`ProjectController`/`ProjectOp`/`Project`/stable ID をそのまま再利用し、
Project Store は二重実装しない。

## Scope

### In Scope

- `crates/synvelis-workspace` 新設 (deps: `synvelis-project`,
  `ruviform-ui`, `serde`, `serde_json` のみ)。
- WorkspaceManager / ViewRegistry / ViewStateRegistry / ViewFactory
  (headless stub) / Focus-Selection 群 / LinkGroup / CommandRouter /
  ProjectSession 配信 / FakeWindowHost / WorkspaceSnapshot 永続化。
- headless 受入テスト (`tests/ws_a.rs`)。

### Out of Scope

- `ruviform-ui-webview`・実 OS Window・WebView・GPU・IME 実装・
  プラグイン GUI 実機 (ruviform #243 lane)。
- 複数 Project 混在、Tool System、Automation 編集、Remote DnD。

## Components and Boundaries

```text
synvelis-workspace (lib)
  ├─ error.rs      WorkspaceError (DockError / project Error / 独自variant)
  ├─ view.rs       ViewType / ViewDescriptor / ViewRegistry / ViewFactory
  │                + HeadlessView stub (render 記述を記録)
  ├─ state.rs      ViewState { EditingSelection, InspectionFocus,
  │                LinkedContext, Viewport } / ViewStateRegistry
  ├─ focus.rs      NativeWindowFocus / InputFocus / ActiveView / FocusState
  ├─ link.rs       LinkGroup / LinkEvent / SelectionMapping /
  │                LinkGroupManager (semantic mapping・dedup・no-reemit)
  ├─ command.rs    CommandKind / CommandContext / InputContext /
  │                CommandRouter / ResolveOutcome
  ├─ session.rs    ProjectSession (ProjectController 包摂) +
  │                revision 配信 + EngineStub (graph rebuild 計数)
  ├─ workspace.rs  WorkspaceManager (DockWorkspace + Window 台帳 +
  │                host port 駆動 + close 方針 + 永続化)
  ├─ host.rs       FakeWindowHost (ruviform WindowHandle 実装・
  │                失敗注入・eval/close 記録)
  └─ persist.rs    WorkspaceSnapshot envelope (versioned・JSON)
```

| 要素 | 型・責務 |
| --- | --- |
| `ViewType` | `Arrangement`/`Mixer`/`Inspector`/`SignalFlow` (複製可) |
| `ViewRegistry` | `ViewInstanceId → ViewDescriptor { view_type, persisted }`。ID 採番は WorkspaceManager 経由の u64 counter |
| `ViewFactory` | `ViewType → HeadlessView` stub。描画せず「表示すべき記述」(type・inspection target・linked context) を返す |
| `ViewStateRegistry` | `ViewInstanceId → ViewState`。Editing Selection / Inspection Focus / Linked Context / Viewport (zoom・scroll) を別フィールドで保持 |
| `FocusState` | `native_window: Option<WindowId>` (host `Focused` 由来)・`active_view: per-Window Option<ViewInstanceId>`・`input: InputFocus` |
| `LinkGroupManager` | `group_id → members`、`view → 0|1 group`、`topic` 単位の publish/apply、`event_id` dedup。受信側は再発信しない |
| `CommandRouter` | `CommandContext` を受け、InputContext → kind → target 解決 → `ProjectController::apply` または workspace/view/transport 反映 |
| `ProjectSession` | `ProjectController` 所有 + `last_notified: per-view revision` + `TransportState` stub + `EngineStub { graph_builds }` |
| `WorkspaceManager` | `DockWorkspace` + `windows: BTreeMap<WindowId, WorkspaceWindow { key, geometry }>` + registries + focus + link + session 参照。ruviform `WindowHandle` port 経由で host を駆動 |
| `FakeWindowHost` | `WindowHandle` 実装。create/close/destroy/monitors/eval を記録し、`create_window` 失敗を注入可能 (rollback 検証用) |

## Data and Control Flow

### Selection / Focus 概念分離 (設計 §5.1)

- `EditingSelection { kind: None|Clips|Tracks|Notes|Nodes, items,
  primary }` — View ごとの編集用選択。
- `InspectionFocus { target: InspectionTarget, mode: Follow{source} |
  Pinned }` — 調査対象。`InspectionTarget = Track | Clip | Source |
  Node(u64)`。Pinned は Follow イベントで上書きしない。
- `LinkedContext { target, via_group }` — link 由来ハイライト。Editing
  Selection とは別フィールド。
- `ActiveView` — window 単位の操作対象 View (`activate_view`・focus
  event で更新)。
- `InputFocus` — `View | TextInput | PluginGui | Modal | Ime`。
  text/ime/plugin/modal 中は DAW command を解決しない。

### Link Group (設計 §5.2/#3 §5)

- `LinkEvent { event_id, source_view_id, group_id, topic,
  semantic_target }`。topic は `track-selection` 等。
- `SelectionMapping`: source view の `EditingSelection` → semantic
  `LinkedTarget`。Clip→所属 Track (lane→track を project で解決)、
  Track→Track。mapping 不能ならその member には配信しない。
- `publish(event, project)` → group member (source 以外) へ一度だけ適
  用: `linked_context` 更新 + `mode == Follow` の Inspector は
  `inspection_focus` を semantic target へ更新 (Pinned は保持)。
- 受信適用は event を再発信しない + `applied_events` dedup で
  ループなし・重複適用なしを保証。

### Command Routing (設計 §6)

```text
CommandContext { command_id, origin_window_id, origin_view_id,
  input_context, command_kind, explicit_target_ids?,
  selection_snapshot?, expected_revision? }
```

1. `input_context` が TextInput/Ime/PluginGui/Modal → Editing/
   Transport/Project command は `Suppressed` (文字入力等が優先)。
2. `command_kind`: Application → workspace 操作 (例: OpenView)、
   View → origin view の表示状態、Transport → ProjectSession
   transport stub、Project/Editing → `ProjectController`。
3. Editing command の対象は発行時に確定: `explicit_target_ids` →
   `selection_snapshot` (発信元 view のもの) → 両方無ければ
   `Unresolved`。**他 view/window の selection にフォールバック
   しない**。対象種別が command と不整合なら `Unresolved`。
4. `Project`/`Editing` は `Command { expected_revision, op }` を
   `ProjectController::apply` へ。controller が対象存在・revision を
   再検証し `StaleRevision`/validation で拒否。
5. `ResolveOutcome`: `Applied { revision } | Suppressed | Unresolved
   | Rejected(err)` を返し、呼び出し側が判断できる。

### 単一正本 + 状態配信 (設計 §4)

- `ProjectSession` が唯一の `ProjectController` を保持。View は
  read model (`live()` 参照) で投影し、変更は CommandRouter 経由のみ。
- `apply` 成功で `live_revision` が進み、`deliver_updates()` が
  last_notified < live_revision の全 view に `ProjectUpdated
  { revision }` を配信 (欠落再同期は revision 比較で検出)。
- `EngineStub.sync_from_project()` は revision 変化時のみ呼ばれ、
  `graph_builds` を +1。Dock/Layout/Focus 操作は `graph_builds` を
  変化させない (WS-A-05 で計数検証)。

### Dock/Window ライフサイクル (設計 §7 + ruviform #181/#182)

- `detach_to_new_window(view, host)`:
  `begin_move(view, Window(new))` → `host.create_window` →
  `register_window` → `commit_move`。create 失敗/`commit_move`
  `TargetUnavailable` → `abort_move`/rollback で view は元の dock に
  残る (MovePrepared 中は edit lock = `ViewBusy`)。
- `move_view(view, target)` = begin+commit one-shot (menu fallback)。
- `on_window_event(WindowEvent::Windowed{window, event})`:
  `Focused` → FocusState.native_window、 `CloseRequested` → close
  方針を評価し `host.respond_close(window, decision)`:
  - 取消ポリシー (例: `veto_close(window)` 登録・main window) →
    `CloseDecision::Veto` (window は残る = close cancel)。
  - それ以外の sub-window → `Approve` + `remove_window(window,
    WindowCloseDisposition::DetachViews)` で views を detached pool
    へ保持 (silent 喪失なし)。
  - `Destroyed` → 台帳除去 (disposition 済み前提)。
- View の edit lock 世代: `DockWorkspace::is_transferring` 中の view
  への command は `ViewBusy` で拒否。

### 永続化 (`WorkspaceSnapshot`)

```text
WorkspaceSnapshot { schema, windows: [WindowKey 順序対応],
  layout: LayoutSnapshot, views: [ViewDescriptor+persisted ViewState],
  link_groups: [LinkDescriptor] }
```

- `save_layout()` → `LayoutSnapshot::capture(windows 順, geometry,
  dock)` + window keys + view descriptors + link 設定を versioned
  JSON で保存。Project 保存とは別物。
- `restore_layout(json, host, monitors)` → envelope parse →
  `LayoutSnapshot::restore_or_default` (破損→安全既定) → 各 window
  を host.create + `apply_restored_tree` + detached pool 復元 →
  view descriptors/state/link を復元。window 順序で WindowKey を再
  対応。
- `WindowKey` は永続 ID (`"main"`, `"w-<n>"` 等)、実行時 `WindowId`
  とは別 (設計 §3 `layout_window_key`)。

## Non-Functional Constraints

- 全 API non-RT。`ruviform-ui`/`synvelis-project` の RT 契約に触れ
  ない。`Tick`/`TickDuration` unchecked 演算子は使用しない。
- `#[non_exhaustive]` enum・`#[must_use]`・`missing_docs`・workspace
  lints (`unsafe_code` forbid, clippy pedantic) 準拠。
- serde で persisted 型は `deny_unknown_fields` + schema version。

## Verification Approach

`crates/synvelis-workspace/tests/ws_a.rs` で WS-A-01..06 を検証
(test-spec 参照)。単体テストは各 module 内。

## Risks / Open Questions

- `LayoutSnapshot` は window key を持たない → 本設計の
  `WorkspaceSnapshot` で順序対応付け (将来 ruviform 側に key が入る
  場合は移行)。
- Inspector `Explore` 状態は #36 スコープ外 (#5) — Follow/Pinned のみ。
- 実機の Focus/DPI/Monitor 復元は ruviform #243 lane 依存。
