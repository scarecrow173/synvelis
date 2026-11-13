---
id: "SPEC-034X4TqMpHOeFHvohktOZV"
type: "spec"
status: "approved"
title: "Workspace Model — GPUI-neutral View/Driver Port, Window/View lifetime independence, Selection/Link Group/Command routing"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/2"
    - "https://github.com/scarecrow173/synvelis/issues/3"
    - "https://github.com/scarecrow173/synvelis/issues/113"
    - "https://github.com/scarecrow173/synvelis/issues/114"
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
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
    - "docs/specs/workspace-ux-scenarios.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/workspace-docking-state-architecture.md"
    - "docs/designs/workspace-ux-scenarios-design.md"
    - "docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/workspace-docking-state-architecture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/181"
    - "https://github.com/scarecrow173/ruviform/issues/182"
    - "https://github.com/scarecrow173/ruviform/issues/237"
    - "https://github.com/scarecrow173/ruviform/issues/240"
    - "https://github.com/scarecrow173/ruviform/issues/243"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/313"
    - "https://github.com/scarecrow173/ruviform/issues/314"
    - "https://github.com/scarecrow173/ruviform/issues/386"
    - "https://github.com/scarecrow173/ruviform/issues/388"
    - "https://github.com/scarecrow173/ruviform/issues/464"
  defers: []
  deferred-by: []
---

# Workspace Model — GPUI-neutral View/Driver Port, Window/View lifetime independence, Selection/Link Group/Command routing

## What Should Be Built

Synvelis Issue #2 の Workspace/Dock/Selection/Command 統合モデルを、現在の実装
（`synvelis-workspace` / `synvelis-app` と Ruviform の driver-neutral port 群）に
整合する形で文書化・検証する。具体的には:

1. Workspace 構造図を **GPUI-neutral View/Driver Port** と **Plugin Slot**
   （`NativeSurfaceSlot`）を含む形に更新し、旧 WebView 固有の前提は legacy 注記へ
   移す。
2. Project / RT / Probe / Job / View State の寿命が Window/View 移動と独立である
   ことを不変条件として明文化し、headless fixture で検証する。
3. 統合設計書（`docs/designs/workspace-docking-state-architecture.md`）の
   不変条件・State 所有者・Window/Dock/View 識別子・Close/Move/Restore
   ライフサイクルを現行実装と照合し、API 確定前の論点を整理する。
4. 既存 WS-A / ED-A / UI-C5 / SV-GUI-04 headless fixture を Case ID 付きで
   台帳化し、`evidence/sv2/` に Expected/Observed/NotRun を記録する。
5. 実 OS の 2 Window / Dock / IME / Keyboard / Plugin GUI 検証は #113/#114/#176
   等の後続 Gate として明示的に NotRun 区別する。

本スライスは設計・検証が主目的であり、製品実装本体の Owner は #113/#175。
既存の承認済み契約（WS-A spec、UX spec、Ruviform port 契約）を再定義せず、
それらの上に Workspace Model の統合記述を載せる。

## Why Now

Issue #1 の製品方向（自由ドッキング、Window 分離、複数モニター対応）を実装へ
進めるには、GPUI driver 移行後の現行コードに対して Workspace Model の
不変条件と所有権を一つの正本として固定する必要がある。WS-A PoC と
sv#3 UX spec、sv#102 UI-C5 op surface は個別に承認・統合済みだが、
Issue #2 が要求する「GPUI-neutral Port + Plugin Slot を反映した Workspace 図」
「寿命独立性の横断検証」「Case ID 台帳」は未整備であり、本スライスで補完する。

## Users and Value

- Synvelis 実装者（#113/#175 等）: Window/View/Dock/Command の所有権と寿命を
  誤解なく実装できる正本を得る。
- レビュアー/Coordinator: headless 検証の範囲と実機未検証領域を区別できる
  evidence 台帳を得る。
- 後続 Gate（#114 実機検証、#176 plugin GUI 等）: NotRun 項目の引き継ぎ先が
  明確になる。

## Scope

### In Scope

- `docs/` 配下の workspace-model 文書チェーン 7 文書（spec/design/plan/task/
  test-spec/impl-ir/impl-exp）と `evidence/sv2/` 台帳。
- `crates/synvelis-workspace/`・`crates/synvelis-app/` の既存 fixture/test の
  照合と、契約ギャップが確認された場合のみの最小限の headless test 追加。
- GPUI-neutral View/Driver Port・Plugin Slot を反映した Workspace 図。
- Window/View/Project/RT/Probe/Job/State の寿命独立性の不変条件と検証。
- Selection/Inspection Focus/Link Group/Command Routing の Owner・状態遷移の
  統合記述。
- Close/Move/Restore ライフサイクルと失敗時ロールバックの検証記録。
- Ruviform Issue/PR・Synvelis 実装 Issue/test との相互参照、未検証 OS の明示。

### Out of Scope

- 実 OS Window/Dock/IME/Keyboard/Pixel の検証（#114 系 GUI-L2/EXT-L3 Gate）。
- 実 plugin editor / NativeSurfaceSlot の実統合（#176 等 EXT-L3 Gate）。
- 製品実装本体の変更（#113/#175 が Owner）。
- Ruviform 側の Window/Dock/GPUI Driver/OS Native Surface 実装。
- Project/RT canonical 実装の変更。
- `Cargo.toml`/`Cargo.lock`/`mise.toml`/CI workflow の変更。
- 旧 WebView 前提の統合設計書本文の書き換え（legacy 注記は新 design doc 側で
  行い、旧文書は履歴として保持）。
- macOS/Linux GUI lane（Deferred — 使用要件が発生するまで）。

## Requirements

- **WSM-R-01** Workspace 構造図は GPUI-neutral View/Driver Port
  （`WindowDriver`/`WindowHandle`/`DriverEvent` 等の driver-neutral 契約）と
  Plugin Slot（`NativeSurfaceSlot`/`SurfaceSlotId`/`SlotLease`）を明示し、
  WebView 固有の記述は legacy 注記として分離する。
- **WSM-R-02** Project/RT/Probe/Job/View State の寿命は Window/View 移動・
  Window Close・View Detach/Reattach と独立であることを不変条件として定義し、
  破られた場合の検出可能な oracle を持つ。
- **WSM-R-03** Window/Dock/View の識別子体系（`WindowId`/`ViewInstanceId`/
  layout key）と、Close/Move/Restore のライフサイクルを統合設計書の不変条件と
  照合して検証する。
- **WSM-R-04** 複数 View 種別（Arrangement/Mixer/Inspector 相当）を別 Window に
  配置したとき、同一 Project 変更は全 View に共有され、View ごとの調査状態
  （Editing Selection/Inspection Focus/viewport 等）は独立であることを
  headless PoC で確認する。
- **WSM-R-05** Link Group が Clip→Track 等の意味的連携・Inspector Pinned 保持・
  Group 間独立・通知ループ防止を満たすことを検証する。
- **WSM-R-06** Delete / Ctrl(Cmd)+Z / Space(transport) / Context Menu を
  複数 Window・TextInput・IME・PluginGui・Modal の各入力文脈で検証し、
  誤った対象への編集が起きないことを確認する。
- **WSM-R-07** Detach→Reattach、Move 失敗ロールバック、Window Close キャンセル、
  Window 消失（Destroyed）、レイアウト破損・モニター切断時の復元を検証する。
- **WSM-R-08** 再生中（transport playing）の Dock/Window 操作で RT 処理・
  Project 編集履歴・Undo stack が意図せず変化しないことを検証する。
- **WSM-R-09** 関連する Ruviform Issue/PR と Synvelis 実装 Issue/test を
  相互参照し、実機未検証の OS/機能を evidence で明示する。

## Acceptance Criteria

Issue #2 の受入条件を WSM-AC-01〜09 として転記・対応付ける。

- [ ] **WSM-AC-01** （設計）次の設計ドキュメントPRで Workspace 図に
  GPUI-neutral View/Driver Port と Plugin Slot を反映し、旧 WebView 固有の
  動作は legacy 注記へ移動 → `docs/designs/workspace-model-design.md` §図。
- [ ] **WSM-AC-02** （不変条件）Project/RT/Probe/Job/State の寿命は
  View/Window 移動と独立 → design doc §寿命独立性 + WSM-R-02 検証。
- [ ] **WSM-AC-03** （照合）統合設計書の不変条件、State 所有者、
  Window/Dock/View 識別子、Close/Move/Restore のライフサイクルを検証し、
  API 確定前の論点を整理 → design doc §識別子/§ライフサイクル/§未決事項。
- [ ] **WSM-AC-04** （PoC）複数 Arrangement/Mixer/Inspector を別 Window へ配置し、
  同一 Project 変更は共有・View ごとの調査状態は独立 → headless test
  `ws_a_01` / `sv113_dock` / `two_window`。
- [ ] **WSM-AC-05** （Link Group）Clip→Track 等の意味的連携、Inspector の
  Pinned 保持、Group 間独立、通知ループ防止 → headless test `ws_a_02`/
  `ws_a_02b`/`ed_a_05`。
- [ ] **WSM-AC-06** （Command）Delete/Ctrl+Z/Space/Context Menu を複数
  Window・入力欄・IME・プラグイン GUI の各文脈で検証し、誤った対象へ編集
  しない → headless test `ws_a_03`/`ed_a` undo 系/`two_window`。
  実 IME/実キーボードは NotRun（#114 gate）。
- [ ] **WSM-AC-07** （ライフサイクル）Detach→Reattach、Move 失敗
  ロールバック、Window Close キャンセル、Window 消失、レイアウト破損・
  モニター切断時の復元 → headless test `ws_a_04`/`04b`/`04c`/`adopt`/
  `preset`/`dock_ops`/`close_veto`。
- [ ] **WSM-AC-08** （RT 境界）再生中の Dock/Window 操作で RT 処理・
  Project 編集履歴が意図せず変化しない → headless test `ws_a_05` +
  追加 playback 中 churn test。実 RT 計測は NotRun（#114/#294 gate）。
- [ ] **WSM-AC-09** （相互参照）関連 Ruviform Issue/PR、Synvelis 実装
  Issue/test を相互参照し、実機未検証 OS を明示 → `evidence/sv2/` 台帳 +
  design doc §参照。

## Deferred Design Concerns

- 実 OS 2 Window/Dock/IME/Keyboard/Pixel 検証 — reason: GUI-L2/EXT-L3 Gate
  （synvelis#114, ruviform#313/#314）| re-engagement: #114 slice 着手時 |
  risk if ignored: headless 合格を実機合格と誤読する → evidence で NotRun
  明示により抑制。
- 実 plugin editor / NativeSurfaceSlot 統合 — reason: EXT-L3・plugin host
  未接続（synvelis#176 系）| re-engagement: plugin editor slice 着手時 |
  risk if ignored: Plugin Slot の実挙動を未検証のまま設計確定する。
- `ShellWindowHost` の複数 Window 生成 — reason: 製品 shell は現状
  single-window（`BackendUnavailable` で拒否）。複数 Window の製品配線は
  #113/#175 の実装 Gate | re-engagement: 製品 multi-window slice |
  risk if ignored: headless の multi-window 意味論を製品挙動と混同する
  → 本 spec は headless oracle のみを主張する。
- macOS/Linux GUI lane — reason: POLICIES.md 上 Deferred（使用要件未発生）|
  re-engagement: 当該 OS での GUI 検証要求時 | risk if ignored: なし
  （現状 Windows のみ Required）。

## Sources and Relations

- 上流 spec: `docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md`
  （WS-A headless PoC、approved）。本 spec はその上位統合モデル。
- 上流 spec: `docs/specs/workspace-ux-scenarios.md`（sv#3 UX 操作仕様、approved）。
- 上流設計: `docs/designs/workspace-docking-state-architecture.md`
  （初期統合設計ベースライン）。本 spec はその WebView 前提部分を GPUI-neutral
  Port + Plugin Slot へ更新する後続設計の要求を定義する。
- 関連設計: `docs/designs/workspace-ux-scenarios-design.md`（sv#3 UX 設計）。
- 関連 spec: `docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md`
  （dock transfer / window adoption の GUI op 面）。
- 実装 Issue: synvelis#113（SV-GUI-04 dock transfer slice）、#114（実機 GUI
  gate）、#175（製品実装 Owner）、#176（plugin GUI 系）。
- Ruviform 依存: #181（Multi-Window Host）、#182（Docking 基盤）、
  #237/#240/#243（Window/Driver 系）、#294（consumer gate）、
  #313/#314（実 Window/pixel golden）、#386/#388（View mount /
  NativeSurfaceSlot）、#464（GPUI scenario lane）。
