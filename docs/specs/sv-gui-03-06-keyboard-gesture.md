---
id: "SPEC-MJFObqc1utEb7LmC1bJVZq"
type: "spec"
status: "approved"
title: "SV-GUI-03 slice6: Keyboard/Gesture key-assignment contract(daw-workflow 委譲項目の回収)"
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
    - "docs/designs/sv-gui-03-06-keyboard-gesture-design.md"
    - "docs/plans/sv-gui-03-06-keyboard-gesture-plan.md"
  depends-on:
    - "docs/specs/daw-workflow.md"
    - "docs/specs/workspace-ux-scenarios.md"
    - "docs/specs/timeline-editing.md"
    - "docs/specs/workspace-model.md"
    - "docs/specs/sv-gui-03-01-transport-inspector-slice1.md"
    - "docs/specs/sv-gui-03-05-gpui-scenario.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gui-03-06-keyboard-gesture-design.md"
    - "docs/plans/sv-gui-03-06-keyboard-gesture-plan.md"
    - "docs/tasks/sv-gui-03-06-keyboard-gesture-task.md"
    - "docs/test-specs/sv-gui-03-06-keyboard-gesture.md"
    - "docs/impl/ir/sv-gui-03-06-keyboard-gesture.md"
    - "docs/impl/exp/sv-gui-03-06-keyboard-gesture.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/8"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-03-06-keyboard-gesture.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/113"
    - "https://github.com/scarecrow173/synvelis/issues/175"
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice6 仕様: Keyboard/Gesture key-assignment contract

## What Should Be Built

Issue #112 の本スライス(sv-gui-03-06)として、統合済み
`docs/specs/daw-workflow.md`(Issue #8)が #112 へ委譲した
キーボード/ジェスチャ関連項目を回収する **canonical な契約
文書チェーン**(spec → design → plan → task → test-spec →
impl ir/exp)と `evidence/sv112/` の case ledger・検証記録を
`docs/` に確定する。対象は「何を・なぜ・誰のために・何をもって
合格とするか」の要件定義であり、契約本文(キー割当表・
OS 正規化・dispatch/suppression 規則・安全契約・競合解決・
コンテキストメニュー網羅期待)は
`docs/designs/sv-gui-03-06-keyboard-gesture-design.md` が
正本となる。

本 spec が固定する契約領域:

1. **キー割当契約** — daw-workflow design §7.2 の最小
   キーボード操作集合(再生/停止・録音・Undo/Redo・削除・
   Copy/Paste・Tool 切替・Zoom/Scroll・保存・Export 等)の
   各「操作→Command」対応に対し、修飾キーを含む具体的な
   既定キー割当を定義する。「操作→Command」の対応自体は
   daw-workflow design §7.2 で固定済みであり、本 spec は
   それを変更しない。
2. **OS 別既定バインドと正規化** — Windows/Linux の Ctrl 系と
   macOS の Cmd 系の既定バインドを規定し、OS 差異を論理
   修飾キー(`Mod`)へ正規化する方針を明確化する
   (daw-workflow design §12 D-3・§13.2 の「キー割当は #112 へ
   Deferred」の回収)。
3. **入力コンテキスト別 dispatch/suppression** —
   `InputContext`(`View`/`TextInput`/`Ime`/`PluginGui`/`Modal`)
   に応じたキー配送・抑制規則を、`focus.rs` の
   `captures_input` 契約と `command.rs`/`workspace.rs` の
   keyboard-capture suppression・Undo lane 配送と整合させて
   定義する(daw-workflow design K-3 の具体化)。
4. **キー操作の安全契約** — K-2(Preview→Atomic Commit・
   Esc 階層取消)と K-4(View ごと独立な Tool 状態・Window/Dock
   移動で不変)をキー操作へ適用し、キー競合の解決ポリシー
   (重複バインド・コンテキスト間衝突)を定義する。
5. **コンテキストメニュー網羅期待** — daw-workflow spec
   「Deferred Design Concerns」で #112 へ委譲された
   コンテキストメニュー完全網羅の期待範囲を契約として規定
   する。
6. **検証と証拠** — headless で検証可能な受入ケースを
   test-spec として定義し、`evidence/sv112/` に「daw-workflow
   委譲項目 → 契約セクション → 受入状況」の case ledger と
   検証記録を作成する。実 GUI key dispatch・実 OS/GUI ゲートは
   `NotRun`/`Deferred` として正直に記録し、headless の検証
   結果を実機 Pass に昇格しない(POLICIES.md §2/§3)。

本 spec は**文書成果物の要件定義**であり、keymap dispatch の
実装・実 OS/実 GUI/実機の受入を主張しない。keymap dispatch
の実装は実機 PoC へ明示的に Deferred であり、本スライスは
コードを一切変更しない。`crates/synvelis-workspace/src/
focus.rs`(`InputContext`/`captures_input`)と
`crates/synvelis-workspace/src/command.rs`(`CommandKind`/
`spec_for`/`CommandOutcome`)は READ-ONLY の契約根拠として
参照し、第二正本を作らない。

## Why Now

- Issue #8(daw-workflow)の design は「操作→Command」の対応を
  §7.2 で固定したが、具体的キー割当・OS 別既定バインド・
  コンテキストメニュー完全網羅を #112 へ Deferred とした
  (design §12 D-3・§13.2・spec「Deferred Design Concerns」)。
  この委譲項目を回収しない限り、#112 の Core Workspace GUI
  実装 slice はキー操作の契約なしに進行する。
- sv-gui-03-01〜05 の統合済みチェーンで Transport/Arrangement/
  Mixer/Inspector の View・`tool.activate`・`transport.play`/
  `transport.stop`・`edit.undo`/`edit.redo` 等の Command
  語彙と `InputContext` suppression が確立済み。本スライスは
  その上に「キー→Command」の写像契約を載せるだけで、既存
  契約の再定義を必要としない。
- 実装に先立ち契約を正本化することで、keymap dispatch 実装
  slice・実機 PoC(#114 GUI-L2 lane)が参照すべき単一の
  割当表・抑制規則・競合ポリシーを提供できる。

## Users and Value

- **DAW ユーザー**: 3 代表ワークフロー(daw-workflow
  WF-SCN-MIDI/AUDIO/MIX)の代表 Task をキーボードで遂行できる
  既定バインドを得る。OS 慣習(Ctrl/Cmd)に沿った予測可能な
  操作が保証される。
- **実装 Owner(#112 後続 slice・#114)**: keymap dispatch・
  既定バインド検証・コンテキストメニュー実装が従うべき
  契約(割当表・正規化・suppression 行列・競合ポリシー・
  メニュー網羅期待)を一意に参照できる。
- **Coordinator/レビュアー**: 「契約として定義済み」「headless
  で検証可能」「実機未検証(NotRun/Deferred)」を区別し、
  headless 成功を実 GUI Pass に換算しない判定枠組みを得る。
- **後続の設定/リマップ slice**: 論理キー(`Mod` 正規化形)で
  保存される既定バインド表を土台に、ユーザー再割当の受入
  条件(競合拒否・OS 予約回避)を引き継げる。

## Scope

### In Scope

- daw-workflow design §7.2 の最小キーボード操作集合の各操作に
  対する既定キー割当(修飾キー・別名を含む)の契約定義。
- OS 別既定バインド(Windows/Linux Ctrl 系・macOS Cmd 系)と
  `Mod` 正規化・OS 予約衝突回避の方針。
- `InputContext` 5 値ごとのキー配送・抑制規則と、既存
  `captures_input`/`dispatch`/`dispatch_undo` との整合。
- K-2/K-4 のキー操作への適用、1 キー Command=1 Undo、
  auto-repeat 方針、キー競合の解決ポリシー。
- コンテキストメニュー網羅期待(対象 ID 明示・Modal 境界・
  accelerator 整合・無効表示の正直さ)。
- spec → design → plan → task → test-spec → impl(ir/exp)の
  文書チェーンと `evidence/sv112/` の case ledger・検証記録。

### Out of Scope

- `crates/` を含む一切のコード実装 — keymap dispatch・
  キーイベント→Command 変換・リマップ UI は実機 PoC 後続
  slice(#112 実装範囲)に委譲。本スライスはコードを変更
  しない。
- 実 OS Window/実入力/IME/DPI/実 Plugin GUI での検証
  (#114 GUI-L2・#176 EXT-L3 lane)。
- Tool/Selection/Snap/Gesture の詳細意味論(#11 が正本)、
  Recording Session 状態遷移の詳細(#12 が正本)、Dock/
  Window 管理(#113)、Theme(#177)。
- ユーザー定義キーマップ(リマップ)の永続化形式・設定 UI —
  本契約は既定バインドと競合ポリシーのみを固定し、リマップ
  実装は後続 slice。
- `Cargo.toml`/`Cargo.lock`/`mise.toml`/CI の変更、
  `docs/mockups/` の変更。
- View ごとのコンテキストメニュー項目の最終文言・アイコン —
  本契約は網羅期待(どの Command がどの面に現れるか)のみを
  規定し、View 実装 slice が文言を確定する。

## Requirements

- **KG-R-01 既定キー割当表**: daw-workflow design §7.2 の
  最小キーボード操作集合の各「操作→Command」対応に対し、
  修飾キーを含む具体的な既定キー割当を design の割当表として
  定義する。キーを割り当てない操作は「既定バインドなし」と
  して理由つきで明示し、暗黙の未定義を残さない。
- **KG-R-02 OS 別既定バインドと正規化**: Windows/Linux の
  Ctrl 系と macOS の Cmd 系の既定バインドを規定し、OS 差異を
  論理修飾キー(`Mod` = Win/Linux は Ctrl、macOS は Cmd)へ
  正規化する方針、OS 予約キーとの衝突回避、リマップ保存形の
  論理キー原則を定義する(daw-workflow design §12 D-3・
  §13.2 の回収)。
- **KG-R-03 入力コンテキスト別 dispatch/suppression**:
  `InputContext`(`View`/`TextInput`/`Ime`/`PluginGui`/`Modal`)
  ごとに、キーが DAW Command へ到達するか・どの Undo lane
  へ行くか・握り潰されるかを、`focus.rs` の `captures_input`
  (= `View` 以外が true)と `command.rs` の kind 別 suppression
  (Project/Transport/Editing)、`workspace.rs` の
  `dispatch_undo` の context 別配送(View→Project 履歴、
  TextInput/Ime→Text lane、Modal/PluginGui→Suppressed)と
  整合する形で定義する(K-3 の具体化)。`CommandKind::View`/
  `Application` は現行 `dispatch` の suppression 対象外で
  あるため、keymap 層でのゲート要否を契約として明示する。
- **KG-R-04 キー操作の安全契約**: K-2(Preview→Atomic
  Commit・Esc 階層取消)と K-4(View ごと独立 Tool 状態・
  Window/Dock 移動で不変)をキー操作へ適用する。キー起因の
  Project 変更は 1 Command=1 Undo 単位、Gesture Preview 中の
  キー扱い、Esc の予約的役割、auto-repeat(キーリピート)の
  方針、重複バインド禁止・コンテキスト lane・リマップ競合
  拒否のキー競合解決ポリシーを定義する。
- **KG-R-05 コンテキストメニュー網羅期待**: daw-workflow
  spec「Deferred Design Concerns」で #112 へ委譲された
  コンテキストメニュー完全網羅について、Command 語彙→
  メニュー面の対応表、対象 ID 明示(`explicit_targets`)、
  メニュー自体の Modal 入力境界、accelerator 表示とキー割当
  表の整合、適用不可項目の正直な無効表示を契約として規定
  する(workspace-ux design §9.2 と整合)。
- **KG-R-06 headless 受入ケースと証拠**: 契約自体を headless
  で検証可能な受入ケース(割当表の網羅性・一意性、OS 正規化
  写像、suppression 行列、競合拒否、メニュー網羅対応)を
  test-spec として定義し、`evidence/sv112/` に「daw-workflow
  委譲項目 → 契約セクション → 受入状況」の case ledger と
  実行した検証の記録を作成する。
- **KG-R-07 証拠の誠実さ**: 実 GUI key dispatch・実 OS/GUI
  既定バインド確認・実機でのキー競合/IME 挙動は `NotRun`/
  `Deferred` として記録し、headless・文書検証の成功を実機
  Pass に昇格しない(POLICIES.md §2/§3、daw-workflow
  WF-AC-10 と同型)。

## Acceptance Criteria

Issue #112 から転記・本スライスへ正規化した受入条件。
本 spec の受入は「契約文書が当該項目を検証可能な形で規定
している」ことであり、keymap dispatch の実装や実 OS/実 GUI
での実測 Pass を意味しない。

- [ ] **KG-AC-01**: daw-workflow design §7.2 の最小
  キーボード操作集合(再生/停止・録音・Undo/Redo・削除・
  Copy/Paste・Tool 切替・Zoom/Scroll・保存・Export 等)の
  各「操作→Command」対応に対し、具体的なキー割当(修飾キー
  含む)を定義する。
- [ ] **KG-AC-02**: OS 別既定バインド(Windows/Linux の Ctrl 系と
  macOS の Cmd 系等)を規定し、OS 差異の正規化方針を明確化
  する(design §12 D-3・§13.2 の「キー割当は #112 へ
  Deferred」の回収)。
- [ ] **KG-AC-03**: `InputContext`(View/TextInput/Ime/
  PluginGui/Modal)に応じた dispatch/suppression 規則を、
  `focus.rs` の `captures_input` 契約および `command.rs` の
  keyboard-capture suppression と整合させて定義する
  (K-3 の具体化)。
- [ ] **KG-AC-04**: K-2(Preview→Atomic Commit・Esc 階層取消)
  と K-4(View ごと独立な Tool 状態・Window/Dock 移動で不変)
  をキー操作へ適用した安全契約、およびキー競合の解決
  ポリシー(重複バインド・コンテキスト間衝突の扱い)を定義
  する。
- [ ] **KG-AC-05**: コンテキストメニューの網羅期待範囲
  (daw-workflow spec「Deferred Design Concerns」で #112 へ
  委譲された分)を契約として規定する。
- [ ] **KG-AC-06**: headless で検証可能な受入ケースを
  test-spec として定義し、`evidence/sv112/` に「daw-workflow
  委譲項目 → 契約セクション → 受入状況」の case ledger と
  検証記録を作成する。
- [ ] **KG-AC-07**: 実 GUI key dispatch・実 OS/GUI ゲートを
  `NotRun`/`Deferred` として正直に記録し、headless の検証
  結果を実機 Pass に昇格しない(POLICIES.md §2/§3 準拠)。

## Deferred Design Concerns

- keymap dispatch の実装(キーイベント→`CommandContext` 変換・
  keymap テーブルの保持・リマップ UI) | #112 実装 slice・
  実機 PoC | 実装/実機 PoC で確定 | 本 spec は契約のみを
  規定し、コードを変更しない。
- ユーザー定義キーマップ(リマップ)の保存形式・設定画面・
  競合時の UI フィードバック詳細 | 後続設定 slice | リマップ
  要件確定後 | 本契約は既定バインドと競合ポリシー(論理キー・
  lane 内一意性・競合拒否)のみを固定する。
- chord/多段キー・キーリピートの個別調整・os 側予約キーの
  網羅一覧 | 実装 PoC | 実機での衝突実測後 | 既定セットは
  単打鍵のみで prefix 衝突を持たない。
- `CommandKind::View`/`Application` の suppression を
  `dispatch` 側へ拡張するか keymap 層でゲートするかの
  実装配置 | #112 実装 slice | 実装時 | 本契約は要求される
  振る舞い(どの context で発火するか)のみを固定し、
  実装層の選択を委ねる。
- コンテキストメニュー各面の最終項目・文言・アイコン・
  サブメニュー構造 | View 実装 slice(#112 後続) | View 実装時 |
  本契約は網羅期待(対応表・explicit target・Modal 境界・
  accelerator 整合・無効表示)のみを規定する。
- 実 OS Window/Focus/IME/DPI・実 Plugin GUI・実機キーボード
  での検証 | #114(GUI-L2)・#176(EXT-L3) | 実機 lane | 設計
  評価を実機 Pass に換算しない。
- Tool 数値(Snap 閾値・HitTest 優先順)・Take/Comp Editor UI |
  #11・#12 | 各 Owner 確定後 | 本 spec はキー割当契約のみ。

## Sources and Relations

- Issue #112 本文(2026-11-13 Windows L2/L3 条件付き必須・
  OS-L1/Headless Slice 受入・変更可能 crate 範囲・
  SV-GUI-03 AC)。
- `docs/specs/daw-workflow.md` + `docs/designs/
  daw-workflow-design.md`(Issue #8): §7.2 最小キーボード
  操作集合・K-1〜K-5・§12 D-3・§13.2・spec「Deferred Design
  Concerns」の委譲項目(本スライスの直接の上流)。
- `docs/specs/workspace-ux-scenarios.md` + `docs/designs/
  workspace-ux-scenarios-design.md`(Issue #3): 入力境界
  (`InputContext` 5 値・状態×ショートカット発火表・
  Context Menu Target 明示・Undo 3 系統分離・Esc 階層)。
- `docs/specs/timeline-editing.md` + design(Issue #11):
  Selection/Tool/Snap/Gesture/Undo 契約。K-4 の Tool 状態と
  `tool.activate` の正本。
- `docs/specs/workspace-model.md`(Issue #2): View/Window 寿命
  独立・Command routing の上位契約。
- `crates/synvelis-workspace/src/focus.rs`:
  `InputContext`(View/TextInput/Ime/PluginGui/Modal)と
  `captures_input`(= `View` 以外が true)の READ-ONLY 参照。
- `crates/synvelis-workspace/src/command.rs` + `workspace.rs`:
  `CommandKind`(Application/Project/Transport/View/Editing/
  Undo)・`spec_for` の登録語彙・`CommandOutcome`
  (Applied/Suppressed/Unresolved/Rejected)・kind 別
  suppression・`dispatch_undo` の context 別配送の
  READ-ONLY 参照。
- 統合済み sv-gui-03-01〜05 チェーン(specs/designs/plans/
  tasks/test-specs/impl): View・Command・scenario の既存
  契約。本スライスはそれらを変更せず参照のみ。
- `evidence/sv112/`(本タスクの case ledger・検証記録)。
