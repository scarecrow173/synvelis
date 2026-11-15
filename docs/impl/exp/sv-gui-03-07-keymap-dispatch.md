---
id: "EXP-rth03t9hXiObjWi9GhxFKJ"
type: "experiment"
status: "completed"
title: "SV-GUI-03-07 keymap dispatch — keymap 層ゲート配置と正直 Unresolved 境界の探索"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/112"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/sv-gui-03-07-keymap-dispatch.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gui-03-06-keyboard-gesture-design.md"
    - "docs/specs/sv-gui-03-06-keyboard-gesture.md"
    - "docs/test-specs/sv-gui-03-06-keyboard-gesture.md"
    - "docs/impl/ir/sv-gui-03-06-keyboard-gesture.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/impl/ir/sv-gui-03-07-keymap-dispatch.md"
  defers: []
  deferred-by: []
---

# SV-GUI-03-07 keymap dispatch — keymap 層ゲート配置と正直 Unresolved 境界の探索

## 背景と仮説

sv-gui-03-06 contract(design §3.1 catalog・§3.2 割当表・§4.1
発火行列・§5 安全契約)を headless で実装するにあたり、契約が
Deferred とした 2 つの配置判断 — (a) View/Application lane の
suppression を keymap 層ゲートで実現するか `dispatch` の
suppression 集合を拡張するか(KG-D-4)、(b) `transport.toggle`・
`project.save` 等の未実装経路をどう返すか — を実装前に検証
した。仮説: keymap 層ゲート 1 箇所集約 + `dispatch` 側は
既存 suppression を維持する構成が、行列の二重化を避けつつ
Undo lane の既存配送(`dispatch_undo`)を再利用できる。

## 観察・試行

- **既存 suppression の実測**: `workspace.rs` `dispatch` は
  `ctx.input.captures_input()` ∧ kind∈{Project,Transport,
  Editing} のみ抑制し、View/Application/Undo は対象外。
  `dispatch_undo`(:1849-1879 付近)は既に context 別配送
  (View→project 履歴・TextInput/Ime→text lane・Modal/
  PluginGui→Suppressed)を持つ。→ Undo lane は keymap 側で
  Modal/PluginGui へ届けない抑制だけ担えばよく、配送を
  複製しない(design §4.3)ことを確認。
- **`PhysicalModifiers` の API 形状**: 初版は struct-style
  field(`.control`/`.command`)を想定したが、実装は bitmask
  型(`PhysicalModifiers::CONTROL | SHIFT`・`.contains()`)
  だった。clippy `struct_excessive_bools`(4 bool フィールド)
  も bitmask 化を後押し。`KeyChord::normalize` は
  「非 primary の Ctrl/Cmd が押されている chord は論理形で
  表現不能 → `None`」とし、OS 予約判定は論理 chord を
  物理へ展開して評価する形に確定。
- **`edit.delete` の domain 解決**: `dispatch_delete_range`
  の required_domain 契約と同型に、explicit_targets →
  selection snapshot → origin view の active domain の順で
  解決し、`Clips` のみ `clip.remove` へ id 書換えて
  `self.dispatch` に再入(1 dispatch=1 undo 単位を維持)。
  他 domain は推測フォールバックせず `Unresolved`。
- **viewport の所在**: `Viewport{scroll_x,scroll_y,zoom_x,
  zoom_y}` は view-local state にあり project revision に
  触れない → `Applied{revision: None}` が正直な outcome。
  zoom 係数 1.25・scroll step 64px は契約が固定しない
  実装定数として `dispatch_viewport` に局所化。
- **`Esc` の判定**: 完全に修飾なし(`PhysicalModifiers::NONE`)
  の Esc のみ reserved transient-cancel。修飾付き Esc は
  普通の chord(未バインド)として扱う。gesture 3 系統
  (`gesture`/`control_edge_gesture`/`routing_gesture`)は
  view ごとに相互排他なので決定的 tiebreak で
  `cancel_gesture`/`control_edge_cancel`/
  `routing_gesture_cancel` に `CancelReason::Escape` を渡す。
- **TS-EX-10 の境界**: menu model は `synvelis-app` 側で
  未存在(別 slice)。keymap 層で実装可能な契約面 —
  §6.2 coverage inventory のデータ化(`menu_coverage`)と
  accelerator 表記写像(`accelerator_label`) — のみを提供し、
  menu-model 依存部分は検証対象外として正直に記録する方針を
  確定(架空 menu model を捏造しない)。

## 結果と採用

- 採用: §4.1 行列を `keymap.rs` の `lane_fires` 1 箇所に集約
  し、`dispatch` 側の既存 suppression は非 keymap caller
  用に維持(構成上一致を comment で明示)。Undo lane は
  keymap 層で Modal/PluginGui 抑制のみ、配送は
  `dispatch_undo` に委譲。`edit.delete` は `clip.remove`
  経路への id 書換え再入。未実装経路(save/export/
  stop-return/record/copy/paste)は全て `Unresolved{reason}`。
- 棄却: `dispatch` の suppression 集合拡張案(Undo lane の
  context 別配送と二重化する・View/Application のみの
  追加では Undo を誤抑制する危険)、`edit.delete` を
  `dispatch` 内で domain 分岐する案(既存 required_domain
  契約と責務が重複)、fake success での save/export 実装。

## 残課題・フォローアップ

- 実 OS/GPUI key event・非 US 配列の物理キー名解決・
  chord/多段キー・リマップ永続化/設定 UI → 後続 slice。
- menu model 本体と accelerator 表示の配線 → `synvelis-app`
  側 slice(TS-EX-10 の menu-model 依存部分)。
- 実機キー/IME/Plugin GUI 検証 → #114 GUI-L2 / #176 EXT-L3。
