---
id: "TSPEC-N6NfW3ReS9Dic6fm6Kxgdi"
type: "test-spec"
status: "approved"
title: "SV-GUI-03 slice6 Keyboard/Gesture key-assignment contract 検証仕様"
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
  implemented-by: []
  depends-on:
    - "docs/specs/sv-gui-03-06-keyboard-gesture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gui-03-06-keyboard-gesture-design.md"
    - "docs/plans/sv-gui-03-06-keyboard-gesture-plan.md"
    - "docs/tasks/sv-gui-03-06-keyboard-gesture-task.md"
    - "docs/impl/ir/sv-gui-03-06-keyboard-gesture.md"
    - "docs/impl/exp/sv-gui-03-06-keyboard-gesture.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/sv-gui-03-06-keyboard-gesture.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice6 Keyboard/Gesture key-assignment contract Test Spec

## Purpose

`docs/specs/sv-gui-03-06-keyboard-gesture.md` が定義した
契約(KG-R-01〜07・KG-AC-01〜07)を検証可能な形で固定する。
本 test-spec の検証は **契約文書自体の整合性(headless・
文書レベル)** と、実装 slice が実装すべき **headless 契約
試験の仕様(TS-EX)** の 2 層で構成する。実 GUI key dispatch・
実 OS/GUI ゲートは本スライスの検証対象外であり、TS-GUI 群
として NotRun/Deferred で記録する。

## Feature

SV-GUI-03 slice6 keyboard/gesture key-assignment contract
(Issue #112 docs-only スライス)。対象は
`docs/designs/sv-gui-03-06-keyboard-gesture-design.md` の
§3.1 contract command catalog・§3.2 KG-KEY-01〜15 割当表・
§3.3 OS 正規化・§4.1 InputContext×lane 発火行列・
§5 安全契約・§5.4 競合ポリシー・§6 メニュー網羅対応。

## Rules

### TS-DOC: 文書契約の検証(headless・本スライスで実施)

- **TS-R-01**: spec KG-R-01/AC-01 — §3.2 の割当表が
  daw-workflow design §7.2 の最小キーボード操作集合
  (再生/停止・停止先頭復帰・録音・Undo/Redo・削除・
  Copy/Paste・Tool 切替・Zoom/Scroll・保存・Export・
  アプリ終了)の各操作に contract command id と具体的キー
  (修飾キー含む)を割り当てているか。
  **検証**: design §3.2 の表を §7.2 の操作一覧と照合。
  Export は KG-KEY-14 で「既定バインドなし+理由」を
  明示していること。
- **TS-R-02**: spec KG-R-02/AC-02 — OS 別既定バインドが
  `Mod` 論理修飾キーで規定され、Win/Linux=Ctrl・macOS=Cmd
  の正規化写像・OS 予約衝突回避・リマップ保存形の論理キー
  原則が design §3.3 に定義されているか。
  **検証**: design §3.3 の記述と §3.2 表の `Mod` 表記の
  一致を確認。
- **TS-R-03**: spec KG-R-03/AC-03 — `InputContext` 5 値
  (View/TextInput/Ime/PluginGui/Modal)ごとの発火可否が
  design §4.1 行列で定義され、`focus.rs` の
  `captures_input`(=`View` 以外 true)・`command.rs` の
  kind 別 suppression(Project/Transport/Editing)・
  `workspace.rs` の `dispatch_undo` context 別配送
  (View→Project 履歴、TextInput/Ime→text lane、
  Modal/PluginGui→Suppressed)と矛盾しないか。
  **検証**: §4.1 行列の Project/Transport/Editing 列が
  `captures_input` と一致、Undo 列が `dispatch_undo` の
  分岐と一致することをコードと照合。
- **TS-R-04**: spec KG-R-04/AC-04 — K-2(Preview→Atomic
  Commit・Esc 階層取消)と K-4(View ごと独立 Tool 状態・
  Window/Dock 移動で不変)がキー操作へ適用され、1 キー
  Command=1 Undo・auto-repeat 方針・キー競合解決
  (KG-C-1〜6)が design §5 に定義されているか。
  **検証**: design §5.1〜5.4 の記述存在と一貫性を確認。
- **TS-R-05**: spec KG-R-05/AC-05 — コンテキストメニュー
  網羅期待が design §6 で規定されているか: §6.2 対応表が
  contract command 全件をカバーするか、CM-1〜6 の規則
  (網羅対応表・accelerator 整合・explicit target・
  Modal 境界・無効表示・同一 pipeline)が定義されているか。
  **検証**: §6.2 表の command id 集合が §3.1 catalog と
  一致するか照合。
- **TS-R-06**: spec KG-R-06/AC-06 — `evidence/sv112/` に
  「daw-workflow 委譲項目 → 契約セクション → 受入状況」の
  case ledger と検証記録が存在するか。
  **検証**: `evidence/sv112/` 配下のファイル存在と内容を
  確認。
- **TS-R-07**: spec KG-R-07/AC-07 — 実 GUI key dispatch・
  実 OS/GUI 既定バインド確認が NotRun/Deferred として
  記録され、headless 成功を実機 Pass に昇格していないか。
  **検証**: impl exp・evidence の status 語彙を確認。

### TS-EX: headless 契約試験(実装 slice で実施する仕様)

実装 slice(keymap dispatch)が満たすべき headless 試験の
仕様。実装方法ではなく観測可能な振る舞いを規定する。

- **TS-EX-01 割当表の一意性**: 各 context lane 内で
  (修飾子+キー)→binding が高々 1 件(KG-C-1)。§3.2 既定
  セットで重複がないこと。
- **TS-EX-02 §7.2 網羅性**: §7.2 の全操作が contract
  command に解決されるか、または「既定バインドなし+理由」
  が明示されていること(KG-AC-01)。
- **TS-EX-03 OS 正規化写像**: `Mod` が Win/Linux で Ctrl、
  macOS で Cmd へ解決され、`Mod+Z`/`Mod+S`/`Mod+Q`/
  `Mod+C`/`Mod+V`/`Mod+=`/`Mod+-` が同一 contract command
  へ写像されること(KG-AC-02)。
- **TS-EX-04 suppression 行列**: design §4.1 の行列どおりに
  `InputContext`×lane で発火/抑制されること。特に
  TextInput/Ime での印刷可能キーが text/IME へ先に配送
  されること、View lane が `View` context のみで発火する
  こと(KG-AC-03/KG-D-4)。
- **TS-EX-05 Undo lane 配送**: `edit.undo`/`edit.redo` が
  View→Project 履歴、TextInput/Ime→text lane、
  Modal/PluginGui→Suppressed に配送されること。
- **TS-EX-06 K-2 適用**: Gesture Preview 中に編集系
  binding が発火しないこと、keymap 発行 Command が
  1 dispatch=1 Undo 単位であること、`Esc` が最内側
  transient を取消し no-transient で no-op であること
  (KG-AC-04)。
- **TS-EX-07 K-4 適用**: `tool.activate` が origin_view
  単位で `set_tool` を呼び、Window/Dock 移動で Tool 状態が
  不変であること、active view 未解決時に `Unresolved` を
  返し推測フォールバックしないこと。
- **TS-EX-08 auto-repeat**: `transport.toggle`/
  `transport.record`/`edit.delete`/`edit.copy`/`edit.paste`/
  `project.save`/`project.export`/`app.quit` が repeat を
  受け付けず、`view.scroll-*`/`edit.undo`/`edit.redo` のみ
  repeat を許容すること(KG-D-5)。
- **TS-EX-09 競合拒否**: リマップが lane 内一意性・OS 予約を
  破る場合に拒否され理由が表示されること(KG-C-5)。
- **TS-EX-10 メニュー網羅**: §6.2 対応表どおりに command
  がメニュー面へ現れ、accelerator 表示が既定バインドと
  一致し、explicit_targets が設定され、適用不可項目が
  disabled 表示されること(KG-AC-05)。
- **TS-EX-11 outcome の正直さ**: keymap 経由 dispatch の
  `Suppressed`/`Unresolved`/`Rejected` が握り潰されず
  区別可能に UI へ反映されること。

### TS-GUI: 実機検証(NotRun/Deferred — 本スライス対象外)

- **TS-GUI-01**: 実 Windows Window/実キー入力での keymap
  dispatch・既定バインド・IME/Focus/DPI 挙動 → #114 GUI-L2。
- **TS-GUI-02**: macOS Cmd 系既定バインド・OS 慣習との整合
  → #114 GUI-L2(macOS lane)。
- **TS-GUI-03**: 実 Plugin GUI でのキー所有・ホスト規約 →
  #176 EXT-L3。

## Examples

- `Space`(View context)→ `transport.toggle` → 停止中なら
  `transport.play`、再生中なら `transport.stop` へ解決。
- `Mod+Z`(TextInput context)→ `edit.undo` → dispatch_undo
  が text lane へ配送し Project 履歴を変更しない。
- `Delete`(Modal context)→ §4.1 行列で Editing lane=S のため
  発火しない(ダイアログ自身の Delete 振る舞いに委ねる)。
- `V`(View context、active view=Arrangement)→
  `tool.activate` arg=smart → その View の Tool が Smart に。
- Gesture Preview 中の `Delete` → §5.1 により抑制、
  `Esc` → preview 取消。
- Export メニュー項目 → `project.export` 発行(既定キーなし、
  KG-KEY-14)。

## Guarantees

- 契約の用語・Context 分類は `InputContext`(focus.rs)・
  `CommandKind`/`CommandOutcome`(command.rs)に一致し、
  第二正本を作らない。
- 「操作→Command」対応は daw-workflow design §7.2 の固定を
  変更せず、キー割当のみを追加する。
- 実装 slice は本 test-spec の TS-EX 群を headless で実走し、
  TS-GUI 群は #114/#176 の実機 lane へ委譲する。
- headless・文書検証の成功は実機 Pass を意味しない
  (POLICIES.md §2/§3)。

## Non-goals

- keymap dispatch の実装・`spec_for` 登録・リマップ UI。
- 実 OS/実 GUI/実機でのキー入力検証。
- Tool 数値・Gesture 詳細(#11)・Recording 状態詳細(#12)・
  Dock/Window 管理(#113)。
- ユーザー定義キーマップの永続化形式・設定画面。

## Risk

- R-1: `[contract]` id が `spec_for` 未登録のまま契約先行
  → design §3.1 で登録済み/contract を明示分離済み。
- R-2: `Ime` Undo 配送が workspace-ux §9.1 と `dispatch_undo`
  で不整合 → design R-3 として記録、実機検証は #114。
- R-3: View/Application suppression の実装配置が今後
  二重化するリスク → 契約は振る舞いのみ固定、配置は
  Deferred(spec Deferred Design Concerns)。
- R-4: 割当表が workflow で不足 → メニュー面・リマップ・
  §7.2 改訂で吸収、追加 binding は Decision Log 記録を要す。
