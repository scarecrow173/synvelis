---
id: "DESIGN-3kN7fR4hX1yZ7vD0oS8aLkQe"
type: "design"
status: "approved"
title: "SV-GPUI-02 headless PluginDock panel 詳細設計 — 4-ID 所有・shared session registry・dock 2-phase 接続"
created: "2026-11-24"
updated: "2026-11-24"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/176"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv-gpui-02-plugin-dock-headless-panel.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv-gpui-02-plugin-dock-gate.md"
    - "docs/designs/sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gpui-02-plugin-dock-headless-panel.md"
    - "docs/test-specs/sv-gpui-02-plugin-dock-headless-panel.md"
    - "evidence/sv176/acceptance-ledger.md"
---

# SV-GPUI-02 headless PluginDock panel 詳細設計

## 構成

```
StablePluginInstanceId ──resolve──► PluginHandle ──open──► PluginEditorSessionId
        ▲                                                        │
        │                                              ParentSlotLease redeem
        │                                                        ▼
   PluginDockBinding ◄── ViewInstanceId ── SurfaceSlotId / SlotLease
```

- `PluginEditorHost`（trait object、`PluginEditorHostObj`）— stable
  instance → `PluginHandle` の解決と `EditorSessionOps` adapter の lend
  を担う composition seam。製品が upstream host 実装に driver-neutral
  に結合する境界。
- `OpsBridge` — boxed 動的 `EditorSessionOps` を upstream の generic
  session メソッドへ適合させ、redeem seam で `ParentSlotLease` を
  `SurfaceSlotRegistry` に照合する。
- `PluginDock` — 1 view instance = 1 パネル。内部に
  `SurfaceSlotRegistry`（slot 世代・lease・visibility・busy）を持ち、
  session registry は所有しない。
- `UiOpsState::plugin_sessions: PluginEditorSessions` — **app 共有**の
  session registry。panel 個別 registry にすると session id が衝突し
  `SessionBusy`（1 `PluginHandle` = 1 editor）を跨 panel で迂回
  できてしまうため、generation/busy の権威を app 単位に置く。
  `PluginDock` の session 系メソッドは全て `&mut PluginEditorSessions`
  を受け取る。
- `UiOpsState::plugin_docks: HashMap<ViewInstanceId, PluginDock>` —
  view 単位の panel 所有。`plugin_dock_command`/`plugin_dock_with_panel`
  が `(panel, sessions, host)` の 3 借用をコールバックへ貸す。

## 状態遷移

- `open(placement, size)` — host 解決 → ops lend → slot mint → lease →
  session `open` → `Attached{Docked|Floating}`。失敗は `Failed` に正直
  着地（`Unsupported`/adapter 拒否を Pass に偽装しない）。
- `set_visibility(false)` → `Hidden` + slot suppress、`(true)` →
  `Attached` + slot visible。`set_size`/`set_focus` は session→ops 直通。
- `notify_detached` — dock detach/pool 遷移で slot suppress + session
  hide（session は close しない — DSP 側 instance は生きたまま）。
- `relocate(to_window)` — 3 分岐:
  1. slot なし（window 喪失後）→ 新 window に slot を再 mint するのみ
     （live session は存在し得ない — window loss が収束済み）。
  2. `spec.window == to_window` → detach が座らせた `Hidden` session
     のみ re-show（`Attached` 復帰 + slot visible）。既に `Attached`
     なら no-op。
  3. 別 window → 新 slot mint → ops lend → `reattach` → 成功時のみ旧
     slot 解放。adapter 拒否は upstream `reattach` の `mark_failed`
     に従い `Failed` + 旧 parent seat 保持（repaint 偽装なし）。
- `notify_window_lost` — slot 解放 + live session を終端（`WindowLost`
  系）に収束。`PluginHandle`/project instance は温存。
- `notify_instance_terminated` — plugin instance 終了で session 収束・
  lease 解放。
- `close` — session close → binding/slot 退役 → panel 解放可能状態へ。
- `save`/`restore` — `PluginDockSnapshot`（instance・placement・size・
  visibility のみ。session id/slot id/lease/OS handle を含めない）の
  論理永続化。

## dock/window ライフサイクル接続（ui_ops.rs）

- `dock_view_attach` / `syn.view.attach` → `plugin_dock_relocate`
- `dock_view_detach` / `op_window_dock_detach` → `plugin_dock_notify_detached`
- `syn.window.dock_prepare*` → slot busy マーク、`dock_commit` /
  `dock_reattach` → relocate、`dock_abort` → busy 解除
- `dock_view_close` / window `Destroyed` → `close` / `notify_window_lost`

独自の 2-phase 実装は持たず、全て既存 `syn.window.dock_*` seam に接続。

## エラー写像

`PluginDockError` → 既存 wire code:

- `NoEditor`/`Occupied`/`NoSlot`/`UnknownInstance`/invalid value →
  invalid parameters
- `Unsupported`/host-unavailable/no-slot-mint → `unavailable`
- その他 slot/session/ops 失敗 → service error

## RT 境界

`syn.plugin_dock.*` は全て UI スレッド op — audio/RT 経路・project
authority・engine generation に触れないことを op tier テスト
（`plugin_dock_ops_do_not_touch_project_or_dsp_state`）が固定。

## 正直性の設計上の要点

- `Failed`/`Closed`/`Hidden` 等の session state は upstream
  `EditorSessionState` をそのまま投影（`session_state_name`）。
- lease 失効は `SurfaceSlotRegistry` の ledger error
  （`LeaseRevoked`/`StaleGeneration`/`InvalidLease`/`UnknownSlot`）が
  そのまま拒否になる — 製品側で再判定しない。
- host 未注入は `unavailable`、capability 不足は `Unsupported` —
  fake/headless 成功を OS/native `Pass` に昇格しない。
