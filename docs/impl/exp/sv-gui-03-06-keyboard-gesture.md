---
id: "EXP-dRwFcautiwbQpQ5lie4sqIX"
type: "experiment"
status: "completed"
title: "SV-GUI-03 slice6 Keyboard/Gesture key-assignment contract — 既存契約再利用と契約先行アプローチ"
created: "2026-11-14"
updated: "2026-11-14"
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
    - "docs/impl/ir/sv-gui-03-06-keyboard-gesture.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gui-03-06-keyboard-gesture-design.md"
    - "docs/specs/sv-gui-03-06-keyboard-gesture.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/sv-gui-03-06-keyboard-gesture-task.md"
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice6 Keyboard/Gesture key-assignment contract — 既存契約再利用と契約先行アプローチ

## 背景と仮説

Issue #112 のキーボード/ジェスチャ項目は、daw-workflow
(Issue #8)が「操作→Command」対応を §7.2 で固定し、キー割当・
OS 別既定バインド・コンテキストメニュー網羅を #112 へ
Deferred とした委譲項目である。本スライスは「契約文書を
先に正本化し、実装は後続 slice が従う」アプローチが最も
整合的と仮説を立てた。対抗案は `focus.rs`/`command.rs` の
既存実装へキー割当を直結させる案だったが、`spec_for` 登録語彙・
View/Application kind の suppression 配置・`[contract]` id の
命名は実装判断を含み、コード変更禁止の docs-only スライスでは
契約のみを固定する方が責務境界に合致すると判断した。

## 観察・試行

- **上流委譲項目の棚卸し**: daw-workflow design §7.2
  (最小キーボード操作集合)・§12 D-3・§13.2・spec
  「Deferred Design Concerns」(コンテキストメニュー網羅)
  を特定し、KG-AC-01〜07 として正規化した。
- **既存契約の照合**: `focus.rs` の `InputContext` 5 値と
  `captures_input`、`command.rs` の `CommandKind`/
  `spec_for`/`CommandOutcome`、`workspace.rs` の `dispatch`/
  `dispatch_undo` kind 別 suppression・context 別 Undo
  配送を READ-ONLY で確認し、§4.1 発火行列が実装と
  矛盾しないことを照合した。
- **View/Application lane のギャップ発見**: 現行 `dispatch`
  は Project/Transport/Editing kind のみを suppression
  対象とし、View/Application kind は対象外。keymap 層に
  `View` context ゲートが必要であることを確認し、
  KG-D-4 として「振る舞いを契約で固定・実装配置は
  Deferred」と決定した。
- **`Ime` Undo の不整合発見**: workspace-ux §9.1 発火表は
  Ime の Undo を「発火しない」とするが、現行
  `dispatch_undo` は Ime を text lane へ配送する。
  契約は現行コード挙動に合わせ、実機検証を #114 へ
  委譲する形で R-3 に記録した。
- **キー割当の選定**: §7.2 の各操作に DAW 慣習の単打鍵を
  割当(Space=Play/Pause・Enter=Stop+Return・R=Record・
  Mod+Z/Shift+Z=Undo/Redo・Delete/Backspace=Delete・
  Mod+C/V=Copy/Paste・V/B/S/T/E=Tool・Mod+=/-=Zoom・
  矢印=Scroll・Mod+S=Save・Mod+Q=Quit)。Export は §7.2
  備考の「メニュー/Command」に従い既定バインドなし
  (KG-D-3)。`Mod` 論理修飾キーで OS 差異を吸収(KG-D-1)。
- **メニュー網羅の契約化**: 「完全網羅」の委譲を、§6.2 の
  contract command→メニュー面対応表 + CM-1〜6 規則
  (網羅対応表・accelerator 整合・explicit target・
  Modal 境界・無効表示・同一 pipeline)として規定した。
  メニュー発行 Command が `input=Modal` では suppression
  されるため `input=View`+`explicit_targets` とする
  KG-D-6 を決定。

## 結果と採用

- 採用: `Mod` 論理修飾キー正規化(KG-D-1)、Space/Enter/R
  の単打鍵割当(KG-D-2)、Export 既定バインドなし(KG-D-3)、
  View/Application ゲートを keymap 層契約として振る舞い
  固定(KG-D-4)、auto-repeat は scroll/undo のみ許容
  (KG-D-5)、メニュー発行は `input=View`+`explicit_targets`
  (KG-D-6)、Tool V/B/S/T/E・矢印 scroll(KG-D-7)、
  `[contract]` id の `spec_for` 登録分離(KG-D-8)。
- 棄却: OS 別に 2 表を維持する案(正規化の一意性を損なう)、
  全 binding の repeat 許容案(トグルストロボ・連続削除の
  リスク)、メニュー発行を `input=Modal` にする案
  (suppression で何も実行できない)、Tool 全種へのキー
  割当案(最小集合を超過)。

## 残課題・フォローアップ

- keymap dispatch 実装・`spec_for` 登録・リマップ UI →
  #112 実装 slice。
- 実 OS/実 GUI/実機キー・IME/Plugin GUI 検証 → #114/#176。
- `Ime` Undo 配送の文書照合(workspace-ux §9.1 vs
  `dispatch_undo`)→ 実機検証結果を踏まえ #114 で判断。
- doc-driven-dev-graph router の既存 broken relation →
  本スライス範囲外、evidence に記録して coordinator へ報告。
