---
id: "SPEC-034QwWbIzoS7Pl7KxncpF6"
type: "spec"
status: "approved"
title: "WS-A Workspace Foundation — View/Selection/LinkGroup/Command/Dock headless PoC"
created: "2026-11-02"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/36"
    - "https://github.com/scarecrow173/synvelis/issues/2"
    - "https://github.com/scarecrow173/synvelis/issues/3"
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
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/workspace-docking-state-architecture.md"
    - "docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md"
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
  defers: []
  deferred-by: []
---

# WS-A Workspace Foundation — View/Selection/LinkGroup/Command/Dock headless PoC

## What Should Be Built

Issue #36: 設計合意済みの #2 (Workspace/Docking/Selection/Command 統合モデル)
と #3 (共通インタラクション仕様) に基づき、Synvelis 側の Workspace 基盤を
**headless で検証可能な PoC** として実装する。`WorkspaceManager` /
`ViewRegistry` / `ViewStateRegistry` + View Factory、概念分離された
Selection/Focus 群、Link Group、Command Router、単一 Project 正本からの
状態配信、Ruviform DockWorkspace (#182) 統合 (detach→reattach・move
rollback・close cancel・レイアウト保存/復元) を含む。

正本設計: `docs/designs/workspace-docking-state-architecture.md`。
基盤契約: ruviform `ruviform-ui::dock` (DockWorkspace・DockTree・
LayoutSnapshot) と `ruviform-ui::window` (WindowId・WindowEvent・
CloseDecision・WindowHandle)。

## Why Now

Ruviform #181 (PR #237・multi-window host contract) と #182 (PR #240・
Dock Tree/view-move/layout-restore) が ruviform main にマージされ、#2
§推奨実装順序の step 2 (Dock Tree/移動の統合・同一 Project に紐づく複数
View での検証) が解除された。F0 (#13→PR#29) の Project Controller /
Stable ID / Save / Reload が確立済みで、Workspace 側が必要とする単一
Project 正本の受け皿がある。

## Users and Value

- DAW ユーザー: View のタブ化・分割・Window 分離/再ドッキング中でも
  制作データと論理 View 状態を失わず、誤った対象への編集が起きない。
- 開発者: Selection/Focus/Command/Link の責務分離と Dock 移動の
  失敗時不変条件が headless テストで固定され、後続の real-GUI lane
  (ruviform #243) と Timeline Editing (#11) の土台になる。

## Scope

### In Scope

1. `WorkspaceManager` / `ViewRegistry` / `ViewStateRegistry` と View
   Factory (Arrangement / Mixer / Inspector / Signal Flow の View
   Instance 型。描画は headless stub)。
2. Editing Selection / Inspection Focus / Active View / Input Focus を
   別型で保持する概念分離 (Native Window Focus は host イベント由来)。
3. Link Group: Clip→Track 等の意味的 Selection Mapping、Inspector の
   Follow/Pinned、Group 間独立、通知ループ防止。編集コマンドは
   broadcast しない。
4. Command Router: Application/Project/Transport/View/Editing の分類、
   発行時確定の Target (Context Menu 明示 Target / 発信元の選択
   snapshot)、入力境界 (テキスト入力・IME・プラグイン GUI・modal) の
   優先、Project Controller での妥当性確認。曖昧な破壊的コマンドは
   実行しない (他 Window の古い Selection へフォールバックしない)。
5. 単一 Project 正本 (`ProjectController`) と複数 (仮想) Window/View
   への revision ベース状態配信。表示/レイアウト更新で Audio Graph を
   再生成しない (stub で検証)。
6. Ruviform DockWorkspace 統合: detach→reattach、move 失敗 rollback、
   window close cancel (veto)、レイアウト snapshot 保存/復元
   (#3 §7: Default/Saved/Last Session・モニター欠落補正)。

### Out of Scope

- 実 OS Window / WebView / GPU 描画 / Multi-Monitor 実機 / DPI / IME 実
  入力 / プラグイン GUI 実機 (ruviform #243 real-GUI lane)。
- 複数 Project 混在、最終 UI デザイン、Remote Asset DnD (#9)、
  Editing Tool System 詳細 (#11)、Automation 編集 (#6)。
- Ruviform 側の変更 (Synvelis 固有型を ruviform に入れない。不足は
  upstream issue 化)。

## Requirements

- 新クレート `synvelis-workspace` (依存: `synvelis-project` +
  `ruviform-ui` + serde 系のみ。webview adapter 非依存・headless)。
- `ViewInstanceId`/`WindowId` は ruviform 契約型をそのまま使い、
  論理 View ID が移動・Window 閉鎖・復元で失われない。
- Editing Selection と Inspection Focus と Linked Context (link 由来の
  ハイライト) は別フィールド。link 受信側は再発信しない。
- Command の対象は発行時に確定 (explicit target または発信元 selection
  snapshot)。破壊的 Editing command は target 未解決なら no-op。
- Workspace/Dock 操作は Project の Undo/Redo・revision・(stub) RT に
  影響しない。Project 変更のみ revision/engine 反映を起こす。
- レイアウト保存/復元は versioned envelope で行い、Project 保存と分離。
  破損/モニター欠落は安全な既定復元へ。

## Acceptance Criteria

- [ ] **WS-A-01**: 複数 View を別 (仮想) Window に配置し、同一 Project
  変更は全 View で共有・View ごとの調査状態 (Editing Selection /
  Inspection Focus / viewport) は独立であることをテストで示す。
- [ ] **WS-A-02**: Link Group の意味的連携 (Arrangement Clip 選択 →
  Mixer Track ハイライト)・Pinned Inspector 保持・Group 間独立・
  通知ループなし (受信側再発信しない・event 重複適用なし)。
- [ ] **WS-A-03**: コマンド解決が発信元/フォーカス/明示 Target に基づ
  き、誤った対象へ編集しない (テキスト入力・コンテキスト境界の分離・
  他 Window の stale selection 非使用を含む)。
- [ ] **WS-A-04**: Detach→Reattach・Move 失敗 rollback・Window close
  cancel・レイアウト破損/復元が headless テストで成立。
- [ ] **WS-A-05**: Dock/Window 操作が Project 編集履歴・(stub) RT 処理
  に影響しない。
- [ ] **WS-A-06**: `mise run test` headless で全件 PASS・impl record に
  検証表・ruviform revision・未検証項目を記録。

## Deferred Design Concerns

- Link Group の複数 Group 所属・タイムライン連携の詳細規則 | #3 §10 未
  決定事項 | real-GUI/Inspector 詳細設計 (#5) で再検討 | 現状は単一
  Group 所属で固定。
- View 閉鎖時の破棄/非表示 UX ポリシー・「閉じた Window を再表示」UI |
  #3 §3 で UX 検証待ち | 同 | 現状は DetachViews disposition で状態
  保持のみ。
- 実機の Window/Focus/DPI/IME 契約 | ruviform #243 lane | 同 lane で実
  機検証 | headless では型レベル契約のみ。

## Sources and Relations

- Issue #36 本文 (スコープ・受入条件・対象外)
- `docs/designs/workspace-docking-state-architecture.md` (設計正本)
- ruviform #181/#182 (PR #237/#240・merged) — `ruviform-ui::dock` /
  `ruviform-ui::window` 契約
- `docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md`
  (F0: Project Controller・Stable ID・Save/Reload)
- `docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md`
