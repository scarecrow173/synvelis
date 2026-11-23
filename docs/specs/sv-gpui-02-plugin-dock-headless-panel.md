---
id: "SPEC-3kN7dP2fV9wX5tB8mQ6yHjNc"
type: "spec"
status: "approved"
title: "SV-GPUI-02 headless PluginDock panel — driver-neutral 製品 ViewSpec/Command + 4-ID session/slot 状態機械 (synvelis#176)"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/sv-gpui-02-plugin-dock-gate.md"
    - "docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gpui-02-plugin-dock-headless-panel-design.md"
    - "docs/test-specs/sv-gpui-02-plugin-dock-headless-panel.md"
    - "evidence/sv176/acceptance-ledger.md"
    - "evidence/sv176/pd-headless-contract.md"
---

# SV-GPUI-02 headless PluginDock panel — driver-neutral 製品 ViewSpec/Command + 4-ID 状態機械

## What Should Be Built

Issue #176（SV-GPUI-02）受入 **行 2 全量** と **行 3 の plugin-dock
結合部**を担う headless slice。`crates/synvelis-app/src/plugin_dock.rs`
に driver-neutral の製品 `PluginDock` パネル状態機械を実装し、
`syn.plugin_dock.*` op 語彙として service/ui_ops 経路に配線する。

行 1・行 7（実 Window / 実 ABI Test Editor gate — `ruviform#630` /
`ruviform#631` + interactive Windows 実行環境）と行 4・行 8（merge
後対象 SHA の CI 実走 — Coordinator 証拠工程）は本 slice の対象外。

## Requirements

### 製品 ViewSpec/Command（行 2）

- 製品 `PluginDock` パネルは upstream が固定した語彙に整合する:
  Panel `plugin.dock`（`PLUGIN_DOCK_PANEL_KEY`）に NativeSlot
  `plugin.editor`（`PLUGIN_DOCK_SLOT_KEY`）を 1 個だけ持つ
  `plugin_dock_view_spec` / `plugin_dock_screen_spec`。
- `ViewSpec` は宣言的 — `SurfaceSlotId`・`ParentSlotLease`・OS
  handle は `ViewSpec` に含めず、live window に対してのみ
  `SurfaceSlotRegistry` が mint する。
- driver-neutral command 語彙 `syn.plugin_dock.{open,hide,show,focus,
  resize,close,save,restore,state,list}` を OpInfo 登録・service.rs
  dispatch・ui_ops.rs 実装として提供する。
- redock は既存 `syn.window.dock_{prepare,commit,abort,detach,reattach}`
  の 2-phase seam に接続する（独自実装しない）。

### 4-ID 所有・世代モデル（行 3 結合部）

- 所有連鎖: `StablePluginInstanceId` → `PluginHandle` →
  `PluginEditorSessionId` → `SurfaceSlotId`/`SlotLease` →
  `ViewInstanceId`。
- session id 生成・寿命は app 共有の `PluginEditorSessions`
  （`UiOpsState::plugin_sessions`）が担い、panel 横断の
  `SessionBusy`（1 `PluginHandle` = 1 editor）を generation 単位で
  保証する。
- slot generation・lease 検証・visibility・geometry・busy・
  stale/revoked 拒否は `SurfaceSlotRegistry` が担う。
- `Open`/`Hide`/`Show`/`Focus`/`Resize`/`ReDock`/`Close`/`Save`/
  `Restore` は決定的。
- adapter 拒否・`Unsupported` は `Failed`/`Unavailable` として正直に
  着地させ、成功を偽らない。
- stale/forged/revoked/generation-mismatch lease は
  `SurfaceSlotRegistry` が `LeaseRevoked`/`InvalidLease`/
  `StaleGeneration`/`UnknownSlot` で拒否する。
- window 跨ぎ再配置は新 slot/lease を mint して reattach 成功後にのみ
  旧 slot を解放し、adapter 拒否時は旧 parent seat を保持し session
  を `Failed` に正直化する（repaint/reparent を偽装しない）。
- window 喪失は slot を解放し live session を `Failed`/`WindowLost`
  系の終端に収束させ、DSP/Project 側 instance は温存する。
- persist は論理状態（instance・placement・size・visibility）のみ —
  session id・slot id・lease・OS handle を含めない（PD-07 方針）。
- `syn.plugin_dock.*` は UI スレッドのみ・非 RT で、audio/RT 経路に
  触れない。

### 正直な capability 境界

- host/adapter 未注入は `unavailable`、未対応 op は `Unsupported`。
- headless/fake の証拠を OS/native の `Pass` に昇格しない — 行 1/4/7/8
  は ledger で `NotRun`/`Deferred`/Coordinator 証拠工程のまま保持。

## Out of Scope

- 実 Window・実 plugin ABI attach・画素/L1 OOS/`GUI_FLOATING`。
- `ruviform-plugin-dock-test` / `test-vst3` dev-dep 追加・upstream
  probe catalog 採用（別 slice — Ask-first の依存追加）。
- ruviform pin bump（wire v7 `WIRE_CAPABILITY_GUI_FLOATING` は
  `synvelis#28` 追従経路）。
- merge 後対象 SHA の CI 実走（行 4/8 — Coordinator 証拠工程）。
