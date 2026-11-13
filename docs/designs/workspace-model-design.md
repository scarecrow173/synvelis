---
id: "DESIGN-034X4TrgrbyPHCdc8l0xPg"
type: "design"
status: "approved"
title: "Workspace Model Design — GPUI-neutral View/Driver Port, lifetime independence, routing"
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
  implements:
    - "docs/specs/workspace-model.md"
  implemented-by: []
  depends-on:
    - "docs/designs/workspace-docking-state-architecture.md"
    - "docs/designs/workspace-ux-scenarios-design.md"
    - "docs/designs/ws-a-workspace-foundation-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/workspace-ux-scenarios.md"
    - "docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md"
  refines:
    - "docs/designs/workspace-docking-state-architecture.md"
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/181"
    - "https://github.com/scarecrow173/ruviform/issues/182"
    - "https://github.com/scarecrow173/ruviform/issues/386"
    - "https://github.com/scarecrow173/ruviform/issues/388"
    - "https://github.com/scarecrow173/ruviform/issues/464"
  defers: []
  deferred-by: []
---

# Workspace Model Design — GPUI-neutral View/Driver Port, lifetime independence, routing

> Status: draft — review-protocol 承認待ち。本書は `docs/specs/workspace-model.md`
> （SPEC-034X4TqMpHOeFHvohktOZV）の設計化であり、実装済み事実と設計意図を
> 区別して記述する。headless 検証の結果は `docs/impl/exp/workspace-model.md`
> と `evidence/sv2/` を参照。

## Context

`docs/designs/workspace-docking-state-architecture.md`（以下「統合設計書」）は
Issue #1/#2 の合意済み方向性を WebView 前提で記述した初期ベースラインである。
その後 Ruviform 側で `WindowDriver`/`WindowHandle`/`DriverEvent` の
driver-neutral port（ruviform#237/#240/#243 系）、`DockWorkspace`/
`MoveToken` の 2 相 dock move（#182）、`NativeSurfaceSlot`（#388）、
`UiRuntime::drain_jobs_routed`/`cancel_window_jobs` の明示的 Job 管理が
実装され、Synvelis 側では `synvelis-workspace`（WS-A/ED-A）と
`synvelis-app`（NativeShell/`syn.*` op 面、SV-GUI-04 slice）がこれらの
port 上に構築された。

本設計は統合設計書の**論理モデルを維持したまま**、表現層を
「native window + WebView」から「driver-neutral Window/View port +
NativeSurfaceSlot plugin slot」へ更新し、Window/View/Project/RT/Probe/
Job/State の寿命独立性を不変条件として固定する。旧 WebView 固有の記述は
構造図直下の legacy 注記へ移す。

## Scope

### In Scope

- GPUI-neutral View/Driver Port と Plugin Slot を含む Workspace 構造図。
- Window/View/Project/RT/Probe/Job/State の寿命独立性の不変条件。
- 識別子体系（`WindowId`/`ViewInstanceId`/layout key/`SurfaceSlotId`）と
  Close/Move/Restore ライフサイクル。
- Selection/Inspection Focus/Link Group/Command Routing の Owner・状態遷移。
- headless 検証の Case→test 対応と Expected Oracle。
- API 確定前の未決事項の整理。

### Out of Scope

- 実 OS Window/Dock/IME/Keyboard/Pixel の検証（#114 系 gate）。
- 実 plugin editor / NativeSurfaceSlot の実統合（EXT-L3 gate）。
- 製品 shell の複数 Window 配線（`ShellWindowHost` は single-window のまま。
  #113/#175 の実装 gate）。
- Ruviform port 自体の変更。

## Workspace 構造図（GPUI-neutral）

```text
Application (synvelis-app)
  ├─ SynvelisApp
  │    ├─ ProjectSession ──────────────── Project/Command/Undo 正本（1つ）
  │    │    ├─ ProjectController (revision, undo/redo)
  │    │    ├─ Engine port (graph builds — 非RT境界)
  │    │    └─ ControlSession (probe/trace — session 所有)
  │    └─ WorkspaceManager<H: WindowHandle>
  │         ├─ ViewRegistry (ViewInstanceId → ViewType/factory)
  │         ├─ ViewStateRegistry (view-local state — 下記 §State 所有者)
  │         ├─ LinkGroupManager (semantic link events)
  │         └─ DockWorkspace (ruviform-ui — Split/Tabs/MoveToken)
  │
  ├─ WindowDriver port (ruviform-ui::driver — GPUI/OS 非依存)
  │    ├─ Window A ── WindowId (runtime, driver 発行)
  │    │    ├─ DockTree A ── Tabs/Split ── View A1 (Arrangement 相当)
  │    │    │                              View A2 (Inspector 相当)
  │    │    └─ Plugin Slot ── NativeSurfaceSlot (SurfaceSlotId/SlotLease)
  │    │           └─ plugin editor surface（将来: #176 系）
  │    └─ Window B ── WindowId
  │         ├─ DockTree B ── Tabs/Split ── View B1 (Mixer 相当)
  │         │                              View B2 (Arrangement 相当)
  │         └─ Plugin Slot ── NativeSurfaceSlot
  │
  └─ View/Driver 境界
       ViewHost/ViewSpec (ruviform-ui::view — declarative view tree)
       WindowDriver impl: FakeDriver (test) / GpuiDriver (native lane)
```

**legacy 注記（旧 WebView 前提の記述の移行先）:**

統合設計書 §2 の `Window A [native window + WebView]`、§4 の
`各WebView / UI component`、§6 の「WebView間でのショートカット配送」、
§8 の「複数Window・WebViewのホスト」「Window-aware IPC」等の WebView 固有
記述は、現行の driver-neutral port に置き換わった。`WindowDriver` が
Window lifecycle（`DriverEvent::Windowed{window,event}`）を `WindowId`
付きで配送し、`post_event` は fire-and-forget で unknown/destroyed 宛を
error にしない。View の描画は `ViewHost`/`ViewSpec`（宣言的）であり、
DOM/JS オブジェクトの Window 間移送という概念は存在しない — View の
移動は `ViewInstanceId` と `ViewState` の引き継ぎであり、表示側は
port 経由で再構築される。

## Components and Boundaries

| 要素 | 実装 | 責務 | 境界 |
| --- | --- | --- | --- |
| `ProjectSession` | `synvelis-workspace::session` | 唯一の Project/Command/Undo 正本。`apply` が revision/engine を進める唯一の経路 | Window/View/Dock 操作は触れない |
| `WorkspaceManager<H>` | `synvelis-workspace::workspace` | Window/View registry、ViewState、Link Group、Command dispatch、Dock move 仲介、layout snapshot | `H: WindowHandle` で driver 非依存 |
| `FakeWindowHost` | `synvelis-workspace::host` | 決定論的 multi-window in-memory `WindowHandle`（close veto、destroy、monitor、event log） | test/dev 専用 |
| `HeadlessWindowHost` | `synvelis-app::host` | app 層の multi-window headless host | test 専用 |
| `ShellWindowHost` | `synvelis-app::host` | 製品 shell の single-window host。`create_window` は `BackendUnavailable` で拒否 | 現状 single-window |
| `NativeShell` + `SynvelisApp` | `synvelis-app` | `syn.*` op 面、`NativeShell` が `WindowDriver` event を app へ転送 | FakeDriver/GpuiDriver 両対応 |
| `WindowDriver` | `ruviform-ui::driver` | Window 作成/focus/resize/close、`DriverEvent` 配送、`WindowCapabilities`（multi_window/ime/native_surface_slot 等） | GPUI/OS 型を含まない |
| `DockWorkspace` | `ruviform-ui::dock` | Split/Tabs/`MoveToken` 2 相 move、`WindowCloseDisposition`、layout validate | view 中身を知らない |
| `NativeSurfaceSlot` | `ruviform-ui::native_slot` | plugin editor 等の OS native surface を driver-neutral にホスト。`SurfaceSlotId`/`SlotLease` は opaque | Synvelis は slot の consumer のみ |
| `ViewHost`/`ViewSpec` | `ruviform-ui::view` | 宣言的 view tree、projection/receipt、gesture | GPUI 非依存 |

## 識別子体系

| 識別子 | 発行者 | 寿命 | 用途 |
| --- | --- | --- | --- |
| `WindowId` | `WindowDriver`（runtime） | Window 生存中。close/destroy で失効、再利用しない | event 発信元、command `origin_window`、job owner routing |
| `ViewInstanceId` | `WorkspaceManager` | View 登録から remove まで。Window 移動・detach を跨いで安定 | dock/view state/command target の論理 ID |
| `layout_window_key` | `WorkspaceSnapshot` 永続化側 | layout restore を跨ぐ | 永続配置の照合（`WindowId` とは別） |
| `MoveToken` | `DockWorkspace` begin_transfer | 1 回の move 試行 | 2 相 commit/abort の相関 |
| `SurfaceSlotId`/`SlotLease` | `NativeSurfaceSlot` 実装 | slot 宣言〜release / lease 期間 | plugin surface の親子関係（opaque） |

`WindowId` ≠ `ViewInstanceId` ≠ layout key の 3 層分離は統合設計書 §2 の
「重要」を継承する。Window は複数 View を含み、View は移動で所属 Window
が変わる。OS/実行時 ID を永続 ID と混同しない。

## 寿命独立性の不変条件（WSM-R-02）

| 状態 | 正本 | Window/View 移動・Close・Detach での扱い |
| --- | --- | --- |
| Project Model + Undo/Redo | `ProjectSession`/`ProjectController` | **不変**。revision/undo stack は Window/View 操作で変化しない（`ws_a_05` oracle: revision・`graph_builds`・undo roundtrip が churn 後も同一） |
| Engine graph | `EngineStub`/engine port | **不変**。Window/View 操作は `graph_builds` を増やさない |
| Probe/Control session | `ControlSession`（session 所有） | **不変**。probe subscription は view/window に紐付かない（semantic binding 駆動） |
| Job | `UiRuntime` job table | **window-owned job は owner Window destroy で cancel 対象、session-owned（`owner=None`）job は存続**。cancel は `cancel_window_jobs` の明示呼出のみ — Window close/destroy が自動 cancel しない（ruviform `UiRuntime` 契約） |
| View State | `ViewStateRegistry`（view-local） | View が生きる限り detach/dock move を跨いで保持。Window Close 時は `WindowCloseDisposition` で pool（`DetachViews`）または remove（`ReleaseViews`）を選択 |
| Editing Selection / Inspection Focus | `ViewState` 内（view-local） | view に従属。Window 移動で失われない。Link Group 配送で他 view の `linked_context` が更新されるが editing selection は自動複製されない |
| Transient（gesture/hover/IME composition） | view/driver 内 | move/close で破棄されてよい。gesture は session-only（`#[serde(skip)]`） |

**oracle:** 上記「不変」行は headless test で `revision`・`graph_builds`・
undo stack・view state・transport playing の前後一致として観測する。
「window-owned job cancel」は `UiRuntime` の責務であり Synvelis 側 test の
直接対象外 — job を返す `syn.*` op が現状存在しないため、headless では
「cancel が自動で起きない」ことの間接証拠（close veto / DetachViews で
job 相当の session 状態が残る）に留め、実 job 挙動は NotRun とする
（§Deferred Gates 参照）。

## State 所有者（統合設計書 §4 の現行化）

| 状態 | 正本 | 備考 |
| --- | --- | --- |
| Project State | `ProjectController`（`ProjectSession` 内） | 全 Window/View 共有。`session.apply` が唯一の mutation 経路 |
| Engine State | engine port（`EngineStub` は headless 代替） | 非RT 境界で graph build。UI が直接所有しない |
| Workspace Layout | `DockWorkspace` + `WorkspaceSnapshot` | `WORKSPACE_SCHEMA=3`、旧 schema は migrate。view/window/link を永続化 |
| View State | `ViewStateRegistry` | `ViewState`（object/time selection, lane scope, tool, grid/snap, edit_binding, gesture(非永続), inspection_focus, linked_context, signal_flow/control_relation(Option・malformed→None), viewport, revision_inbox） |
| Host Focus / Input Context | `FocusState`（`InputContext`: View/TextInput/Ime/PluginGui/Modal） | command routing の優先判定に使用。OS focus そのものは driver 管轄 |
| Transient | view/driver 内 | gesture・hover・menu・IME composition — 永続化しない |

## Data and Control Flow

### Command issue → dispatch

```text
input event (driver/view)
  → CommandContext { id, origin_window, origin_view, input, explicit_targets,
                     selection?, expected_revision?, arg? }
  → WorkspaceManager::dispatch(ctx)
      ├─ InputContext::TextInput|Ime|PluginGui|Modal → Suppressed（編集系）
      │     Undo のみ TextInput/Ime は text lane、他は Suppressed
      ├─ explicit_targets 非空 → それを target に（selection より優先）
      ├─ selection/active domain → SemanticTarget 解決
      │     解決不能 → Unresolved（他 view/古い selection へ fallback しない）
      ├─ expected_revision 不一致 → Rejected
      └─ ProjectOp 構築 → ProjectSession::apply → revision → distribute_revision
           → CommandOutcome::Applied { revision }
```

### Link Group publish/deliver

```text
view.select(editing_selection)
  → object_selections.assign（view-local）
  → links.publish(view, active_selection, project)
       → semantic mapping（例: Clip→Track）で LinkEvent 生成
  → receivers = 同 group 他 view（pinned inspector は retarget しない）
  → links.deliver(event, receiver, states) — dedup で再発行しない
  → LinkedContext 更新（highlight/context hint — editing selection ではない）
```

### Dock move（2 相）

```text
dock_prepare(view, dest) → MoveToken 発行（source view はまだ旧所属）
dock_commit(token)       → DockWorkspace が所属を 1 回の論理 commit で切替
dock_abort(token)        → token 破棄、source view は無変更
失敗（dest window destroy / host create 失敗 / ViewBusy）→ abort →
  MoveAborted event、View ID・ViewState・所属を維持
```

### Window close

```text
WindowLifecycle::CloseRequested(window)
  → close veto callback（dirty project / recording / app guard）
       → Veto: CloseDecision::Veto（Window 生存、view 無変更）
       → Approve: WindowLifecycle::Destroyed
            → WindowCloseDisposition::DetachViews → views を detached pool へ
              （registry/state 保持、project/session 無傷）
            → ReleaseViews → view remove
  → 最後の Window の close は app-level quit 判定へ（syn.app.quit）
```

## 識別子とライフサイクルの照合（WSM-AC-03）

統合設計書 §3 の構造不変条件 6 項目を現行実装と照合:

| # | 不変条件 | 現行実装 | 状態 |
| --- | --- | --- | --- |
| 1 | 生存 View は全 Window で高々 1 Tabs | `DockWorkspace` が所属一意性を validate。detached pool は「未配置」として明示管理 | 実装済み（`ws_a_04*`） |
| 2 | Window は 1 Dock Tree root、空 Window は明示状態 | `DockWorkspace`/`LayoutSnapshot` | 実装済み |
| 3 | Split 子数・分割比・重複 ID・循環・空 Tabs・無効参照を検証/正規化 | `LayoutSnapshot::restore_or_default` + `DockWorkspace` validate。malformed → safe default | 実装済み（`ws_a_04c`） |
| 4 | View 複製は新 ID、移動は既存 ID 維持 | `spawn_view`/`spawn_detached_view`/`adopt_view` vs `move_view`/`begin_transfer` | 実装済み |
| 5 | Window Close で View State/Project を自動破棄しない | `WindowCloseDisposition` で pool/remove を明示選択 | 実装済み（`ws_a_04b`） |
| 6 | Monitor は安定 ID と仮定しない、不可視座標は安全復帰 | `MonitorHint` + preset missing_monitor fallback | 実装済み（`preset.rs`） |

Close/Move/Restore の状態遷移は上記 Data and Control Flow の各節に集約。
「API 確定前の論点」は §未決事項 に列挙。

## Command Routing の文脈別検証（WSM-R-06）

| 状況 | 期待 | headless oracle |
| --- | --- | --- |
| Arrangement 相当 view で Clip 選択 → Delete (`clip.remove`) | その view の selection が target | `ws_a_03` |
| 別 Window の view に同名 command | `origin_view` が別 window の view → foreign window 拒否 | `two_window.rs`, `sv113_dock.rs`, `shell.rs` |
| Context menu（`explicit_targets` 非空） | selection より explicit target 優先 | `ws_a_03` |
| target 解決不能（selection なし/別 domain/未知 view） | `Unresolved` — 他 view の古い selection へ fallback しない | `ws_a_03` |
| `expected_revision` 不一致 | `Rejected` | `ws_a_03` |
| `InputContext::TextInput`/`Ime`/`PluginGui`/`Modal` で編集系 | `Suppressed` — 誤編集しない | `ws_a_03` |
| `edit.undo`/`edit.redo` in `View` | `ProjectSession::undo/redo`（project history） | `ed_a.rs` undo tests |
| `edit.undo` in `TextInput`/`Ime` | text lane undo（`text_lane_undos` 加算、project 無変更） | `ed_a.rs` |
| `edit.undo` in `PluginGui`/`Modal` | `Suppressed`（plugin/modal が undo を所有） | `ed_a.rs`, `ws_a_03` |
| transport command（`transport.play` 相当 = Space の command 面） | `CommandKind::Transport` — project mutation ではない | `ws_a_03`, `ui_ops.rs` |
| Window が途中で destroy された view への command | stale/foreign view → 拒否 | `shell.rs` reattach 後 stale window 拒否 |

実 IME/実キーボード/実 plugin GUI での配送は driver/OS 依存であり
headless の範囲外 → §Deferred Gates。

## Event→Receipt 例（2 Window）

`two_window.rs` / `sv113_dock.rs` の headless 構成を例示:

```text
Window W1 (WindowId w1)          Window W2 (WindowId w2)
  View v1 (Arrangement 相当)       View v2 (Transport deck 相当)
       │                                │
       │  syn.project.set_ppqn (origin v1)   │
       ▼                                │
  WorkspaceManager::dispatch           │
       │ ProjectSession::apply          │
       ▼                                ▼
  revision r+1 ──distribute_revision──▶ v1.revision_inbox, v2.revision_inbox
       │                                │
       └─ project.info receipt ─────────┘  (両 window が同一 project truth を観測)

  v2 から origin_window=w1 の command → foreign window → Rejected/Unresolved
  （window を跨いだ誤配送を防ぐ）
```

## Normal / Failure Scenarios と Expected Oracle

| Scenario | Expected | test |
| --- | --- | --- |
| 2 Window に Arrangement/Mixer/Inspector 相当を配置 | project 変更は共有、view state/focus は独立 | `ws_a_01`, `sv113_dock::view_state_and_focus_are_independent_per_window`, `two_window` |
| Link Group: Clip→Track 連携 | semantic mapping のみ配信、pinned inspector 保持、group 独立、dedup で loop なし | `ws_a_02`, `ws_a_02b`, `ed_a_05` |
| Detach→Reattach | view id/state 保持、所属が一貫 | `ws_a_04` |
| Move 先 window destroy / host create 失敗 / ViewBusy | abort → source 無変更、`MoveAborted` | `ws_a_04`, `dock_ops` saga tests |
| Window CloseRequested → veto（dirty/recording/guard） | `CloseDecision::Veto`、window/view 無変更 | `close_veto.rs`, `ws_a_04b` |
| Window Close approved → `DetachViews` | views が pool へ、registry/state/project 残存 | `ws_a_04b`, `dock_ops` destroyed tests |
| Window `Destroyed`（close なし crash 相当） | detach pool or release、project/session 残存 | `adopt.rs`, `sv113_dock` |
| Layout snapshot save/restore | window/view/link/state が復元 | `ws_a_04c` |
| Layout 破損（malformed/corrupt payload） | schema reject または safe default + views を detached pool へ安全退避 | `ws_a_04c` |
| Monitor 切断（saved bounds の monitor 不在） | available geometry へ fallback | `preset.rs` missing_monitor |
| 再生中（playing=true）の dock/window churn | transport playing・revision・undo stack 不変 | `ws_a_05` + 追加 playback churn test（test-spec WSM-T-08） |
| Layout op が project/engine を触る | revision/`graph_builds`/undo 無変化 | `ws_a_05`, `dock_transfer_preserves_project_and_transport_state` |

## Non-Functional Constraints

- **RT 分離**: Workspace/Dock/Window 操作は非 RT。`ProjectSession::apply`
  が revision/engine 更新の唯一経路であり、dock/window/layout op は
  project revision を進めない（`ws_a_05` oracle）。実 RT 計測（latency/
  alloc）は本 slice 範囲外（#294/#114 gate）。
- **GPUI 中立性**: `synvelis-workspace`/`synvelis-app` の contract 層は
  `gpui` 型を含まない。`WindowDriver`/`ViewHost` port 経由で FakeDriver・
  GpuiDriver 双方を駆動。`native`/`gpui-test` feature lane は別検証。
- **fail-closed**: 未知 view/stale revision/foreign window/malformed
  snapshot は拒否または安全退避し、暗黙 fallback しない。
- **決定論**: `FakeWindowHost`/`FakeDriver` は event log・close decision・
  monitor を注入可能で、headless oracle が再現可能。

## Risks and Trade-offs

- **headless ≠ 実機**: `FakeWindowHost`/`FakeDriver` は意味論 oracle であり
  OS focus/IME/pixel を保証しない。実機差異は #114 gate で検証。
  → evidence で lane を明示し Pass/NotRun を混同しない。
- **`ShellWindowHost` single-window**: 製品 shell は現状 multi-window を
  拒否するため、headless の multi-window 成功を製品挙動と誤読する危険。
  → design doc/spec で「headless oracle のみ」と明記。
- **Job lifetime の直接検証なし**: `syn.*` op が `CallOutcome::Job` を
  返さないため、window-owned job cancel の実挙動は未検証。→ NotRun 明示、
  `UiRuntime` 契約（自動 cancel しない）を設計不変条件として記録。
- **Link Group の semantic mapping 網羅性**: Clip→Track は実装済みだが
  全 View 種別の mapping は View 実装側の責務。本設計は機構のみ規定。

## Decision Log

| ID | 決定 | 採用理由 | 棄却した代替 |
| --- | --- | --- | --- |
| WSM-D-01 | View/Driver port を `WindowDriver`+`ViewHost`/`ViewSpec`（ruviform-ui）に統一し、Synvelis は `WorkspaceManager<H: WindowHandle>` で抽象化 | driver 非依存・Fake/Gpui 双方で同一 contract を検証可能。GPUI 型を workspace に持ち込まない | Synvelis 独自の window/view trait（重複・drift リスク） |
| WSM-D-02 | Plugin Slot は `NativeSurfaceSlot`（ruviform#388）を参照のみ。Synvelis workspace は OS 型を持たず slot consumer に留まる | OS native surface の親子関係・lease を driver-neutral に扱う upstream 契約を再利用 | Synvelis 独自 plugin surface 型（境界違反） |
| WSM-D-03 | Editing Selection と Inspection Focus を分離し、Link Group は `LinkedContext`（hint）のみ配信。editing selection を自動複製しない | 「focus 移動 ≠ 選択変更 ≠ 調査対象変更 ≠ 編集対象変更」の不変条件。誤編集防止 | 単一 global selection / link group での command broadcast（誤爆リスク） |
| WSM-D-04 | Command target は issue 時に `CommandContext` で確定。`explicit_targets` > selection > unresolved。unresolved は fallback せず `Unresolved` | 非同期・window 跨ぎでの対象すり替わりを防止。fail-closed | 実行時に「現在の selection」を再読（race・誤爆） |
| WSM-D-05 | Undo は `InputContext` で lane 分岐: `View`→project history、`TextInput`/`Ime`→text lane、`PluginGui`/`Modal`→suppressed | IME/文字入力/plugin GUI が undo を所有する文脈を尊重 | 全文脈で project undo（誤編集） |
| WSM-D-06 | Dock move は `MoveToken` 2 相（prepare→commit/abort）。失敗時は source 無変更で `MoveAborted` | 移動中の中間状態を外部に見せず、失敗時 rollback を保証 | 単相 move（失敗時に view 喪失リスク） |
| WSM-D-07 | Window Close は `CloseDecision` veto + `WindowCloseDisposition`（`DetachViews`/`ReleaseViews`）で view の去就を明示 | close が project/session/job を暗黙破棄しない。pool で view を保全 | close = view destroy（状態喪失） |
| WSM-D-08 | Layout snapshot は `WORKSPACE_SCHEMA` versioned envelope + `restore_or_default` safe fallback。malformed → reject or detached pool 退避 | 破損 layout で app/view を落とさない | 無検証 deserialize（panic/破損 state） |
| WSM-D-09 | `WindowId`（runtime）/`ViewInstanceId`（logical）/`layout_window_key`（persistent）を分離 | OS/実行時 ID を永続 ID と混同しない。restore で window を再生成しても view 同一性を維持 | 単一 ID 体系（restore 跨ぎで不整合） |
| WSM-D-10 | 本 slice は設計+headless 検証に限定し、実装変更は契約ギャップ確認時のみ | 既存 WS-A/ED-A/UI-C5/SV-GUI-04 が大部分を網羅。重複実装を避ける | 新規 workspace 実装（既存と競合） |

## 未決事項（API 確定前の論点）

- **view 閉鎖時 UX**: `ReleaseViews` のユーザー向け確認 dialog・「閉じる」
  ジェスチャの具体 UI は UX spec（sv#3）の範囲。本設計は disposition
  機構のみ規定。
- **Link Group の複数所属・topic 拡張**: 現行は 0/1 group・semantic topic
  固定。複数 group 所属や timeline 連動の詳細は未決（統合設計書 §11 踏襲）。
- **plugin GUI の keyboard 契約**: `InputContext::PluginGui` の設定主体
  （driver が検出 vs view が申告）と、plugin 内 key の host 非干渉の
  OS 別差異は実機 gate で確定。
- **Job owner の製品ポリシー**: session-owned job の寿命・cancel タイミング
  の製品ルール（recording/export/plugin scan 等）は #113/#175 実装時に確定。
- **multi-window 製品配線**: `ShellWindowHost::create_window` を有効化する
  条件・UX（menu/shortcut/restore 時の window 再生成）は製品 slice で確定。

## Deferred Gates（実機未検証 — NotRun）

| 項目 | Gate | 理由 |
| --- | --- | --- |
| 実 OS 2 Window / Dock drag / pixel | synvelis#114, ruviform#313/#314 | GUI-L2/EXT-L3。fake/headless は意味論のみ |
| 実 IME / keyboard 配送 / OS focus | synvelis#114 | driver/OS 依存。`InputContext` の実発生源は未検証 |
| 実 plugin editor / NativeSurfaceSlot 実統合 | synvelis#176 系, ruviform#388 consumer | EXT-L3。slot contract は参照のみ |
| window-owned job の実 cancel / session job 存続 | `CallOutcome::Job` を返す op 未実装 | headless に job fixture なし |
| 実 RT 計測（dock/window op 中の audio glitch/alloc） | synvelis#294, ruviform consumer gate | RT oracle は stub レベルのみ |
| macOS/Linux GUI lane | POLICIES.md Deferred | 使用要件未発生 |

## Verification Notes

- [x] headless test が WSM-AC-04〜08 の oracle を提供することを
  `docs/test-specs/workspace-model.md` で Case ID 対応付けた。
- [x] 実機未検証項目を NotRun/Deferred として `evidence/sv2/` に記録する
  ことを確認した。
- [x] GPUI-neutral port（`WindowDriver`/`ViewHost`/`NativeSurfaceSlot`）を
  実コードの型名で参照し、WebView 固有記述を legacy 注記へ分離した。
