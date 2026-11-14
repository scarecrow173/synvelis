---
id: "DESIGN-BZaKwo7Z0KBbDJHyNDjzNK"
type: "design"
status: "approved"
title: "SV-GUI-03 slice6 設計 — Keyboard/Gesture key-assignment contract"
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
  implements:
    - "docs/specs/sv-gui-03-06-keyboard-gesture.md"
  implemented-by: []
  depends-on:
    - "docs/designs/daw-workflow-design.md"
    - "docs/designs/workspace-ux-scenarios-design.md"
    - "docs/designs/timeline-editing-design.md"
    - "docs/designs/workspace-model-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/sv-gui-03-06-keyboard-gesture-plan.md"
    - "docs/tasks/sv-gui-03-06-keyboard-gesture-task.md"
    - "docs/test-specs/sv-gui-03-06-keyboard-gesture.md"
    - "docs/specs/sv-gui-03-06-keyboard-gesture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-03-06-keyboard-gesture.md"
    - "docs/designs/daw-workflow-design.md"
    - "docs/designs/workspace-ux-scenarios-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-03 slice6 設計: Keyboard/Gesture key-assignment contract

## 1. Overview

daw-workflow design(ISSUE #8)は §7.2 で「操作→Command」の
対応を固定し、キー割当・OS 別既定バインド・コンテキスト
メニュー網羅を #112 へ Deferred とした(§12 D-3・§13.2・
spec「Deferred Design Concerns」)。本設計はその委譲項目を
**契約として回収**する: 既定キー割当表・OS 正規化・
`InputContext` 別 dispatch/suppression・K-2/K-4 安全契約・
キー競合ポリシー・コンテキストメニュー網羅期待を定義する。

本設計は文書契約であり、keymap dispatch の実装(キーイベント
→`CommandContext` 変換・keymap テーブル・リマップ UI)は
含まない。`crates/` は一切変更しない。契約の用語・Context
分類は `focus.rs` の `InputContext` と `command.rs` の
`CommandKind`/`CommandOutcome` に合わせ、第二正本を作らない。

## 2. Context and Scope

### 2.1 上流契約(正本の所在)

| 領域 | 正本 | 本設計での扱い |
| --- | --- | --- |
| 「操作→Command」対応・K-1〜K-5 | `docs/designs/daw-workflow-design.md` §7 | 変更しない。キー割当のみ追加 |
| 入力境界・状態×キー発火表・Undo 3 系統・Esc 階層 | `docs/designs/workspace-ux-scenarios-design.md` §9(#3) | 前提として整合。発火表のキー名を本割当表で具体化 |
| Tool/Selection/Gesture/Undo 詳細 | `docs/designs/timeline-editing-design.md`(#11) | 参照のみ。Tool 名は `tool.rs` の `Tool` enum に一致 |
| View/Window 寿命・Command routing | `docs/specs/workspace-model.md`(#2) | 前提 |
| `InputContext` 5 値・`captures_input` | `crates/synvelis-workspace/src/focus.rs` | READ-ONLY 参照。`captures_input()` = `View` 以外で true |
| `CommandKind`/`spec_for`/`CommandOutcome` | `crates/synvelis-workspace/src/command.rs` | READ-ONLY 参照。登録語彙は §3.1 に転記 |
| kind 別 suppression・`dispatch_undo` 配送 | `crates/synvelis-workspace/src/workspace.rs` `dispatch`/`dispatch_undo` | READ-ONLY 参照。§4 の行列はこの実装と整合 |

### 2.2 スコープ外

- keymap dispatch 実装・リマップ UI・設定永続化(後続 slice)。
- 実 OS/実 GUI/実機検証(#114 GUI-L2・#176 EXT-L3)。
- Tool 数値・Gesture 詳細(#11)、Recording 状態詳細(#12)、
  Dock/Window 管理(#113)、コンテキストメニュー各面の最終
  文言・構造(View 実装 slice)。

## 3. Key Assignment Contract(KG-D-01)

### 3.1 Contract command catalog

キー割当は **contract command id** に対して定義する。
`spec_for`(`command.rs`)登録済みの id は `[registered]`、
本契約で新たに名付けるが `spec_for` 未登録の id は
`[contract]` と表記する。`[contract]` id の `spec_for`
登録は実装 slice の責務であり、本契約は id 名・kind レーン・
解決規則を固定する(第二正本ではなく、登録語彙への追加予定
として明示)。

| contract id | `CommandKind` レーン | `spec_for` | 備考 |
| --- | --- | --- | --- |
| `edit.undo` / `edit.redo` | Undo | registered | context 別配送は §4.3 |
| `tool.activate`(arg=tool 名) | View | registered | `Tool::parse` 名: smart/select/marquee/draw/split/trim/slip/stretch/fade/erase |
| `view.activate` | View | registered | 既定キーなし(§3.2) |
| `transport.play` / `transport.stop` | Transport | registered | `transport.toggle`/`stop-return` の内部解決先 |
| `clip.remove` / `clip.mute` / `clips.delete-range` | Editing | registered | entity target 必須/範囲削除。キー直接割当なし、`edit.delete`・メニュー経由 |
| `project.rename` | Project | registered | ダイアログ起動型。キー割当なし |
| `app.quit` | Application | registered | §3.2 KG-KEY-15 |
| `transport.toggle` | Transport | **contract** | Play/Pause トグル。停止中→`transport.play`、再生中→`transport.stop`(位置保持の pause 相当)へ解決 |
| `transport.stop-return` | Transport | **contract** | Stop + 再生位置を先頭へ戻す複合意図。#12/Transport 契約の確定待ち要素は実装 slice で解決 |
| `transport.record` | Transport | **contract** | 録音 Arm/Record/Stop のトグル意図。状態遷移の解決は #12 Session 状態機械が正本 |
| `project.save` | Project | **contract** | `screens.rs` の toolbar action id と同名だが、こちらは `CommandContext` 語彙としての契約 id(view action とは別層) |
| `project.export` | Project | **contract** | Export Dialog/Render 起動。既定キーなし(§3.2 KG-KEY-14) |
| `edit.delete` | Editing | **contract** | Active View の明示的選択へ適用。domain 解決: Clips→`clip.remove`、他 domain は #11/#12 語彙確定後に対応付け |
| `edit.copy` / `edit.paste` | Editing(clipboard) | **contract** | Clipboard 経由。Target 解決は貼付先 View の契約(#11)に従う |
| `view.zoom-in` / `view.zoom-out` | View | **contract** | Arrangement/MIDI View のズーム |
| `view.scroll-left` / `view.scroll-right` / `view.scroll-up` / `view.scroll-down` | View | **contract** | View スクロール |

### 3.2 Default keymap(KG-KEY-*)

表記: `Mod` = Windows/Linux では `Ctrl`、macOS では `Cmd`
(§3.3)。`S` = `Shift`、`A` = `Alt`。キー名は GPUI/OS の
論理キー名に対応する契約上の表記であり、物理キー配置
(特に JIS 配列)の差異は実装 slice のキー名解決に委ねる。

| ID | 操作(daw-workflow §7.2) | contract command | Win/Linux | macOS | InputContext 条件 | 備考 |
| --- | --- | --- | --- | --- | --- | --- |
| KG-KEY-01 | 再生/停止(Play/Pause) | `transport.toggle` | `Space` | `Space` | `View` のみ | edge-triggered(§5.3) |
| KG-KEY-02 | 停止(位置先頭へ) | `transport.stop-return` | `Enter` | `Enter` | `View` のみ | |
| KG-KEY-03 | 録音開始/停止 | `transport.record` | `R` | `R` | `View` のみ | edge-triggered。状態解決は #12 |
| KG-KEY-04 | Undo | `edit.undo` | `Mod+Z` | `Mod+Z` | §4.1 行列に従う | repeat 許容(§5.3) |
| KG-KEY-05 | Redo | `edit.redo` | `Mod+Shift+Z` | `Mod+Shift+Z` | 同上 | repeat 許容 |
| KG-KEY-06 | 削除 | `edit.delete` | `Delete`・`Backspace` | 同左 | `View` のみ | 2 キーは同一 binding の別名 |
| KG-KEY-07 | コピー | `edit.copy` | `Mod+C` | `Mod+C` | `View` のみ | |
| KG-KEY-08 | 貼付 | `edit.paste` | `Mod+V` | `Mod+V` | `View` のみ | |
| KG-KEY-09 | Tool 切替 | `tool.activate` arg=smart/draw/split/trim/erase | `V`/`B`/`S`/`T`/`E` | 同左 | `View` のみ | `V`=smart(既定ポインタ)。select/marquee/slip/stretch/fade は既定バインドなし(ツール picker/メニュー経由、リマップ候補) |
| KG-KEY-10 | Zoom In | `view.zoom-in` | `Mod+=` | `Mod+=` | `View` のみ | `Mod++`(Shift+`=`)も同一解釈を許容(§3.3) |
| KG-KEY-11 | Zoom Out | `view.zoom-out` | `Mod+-` | `Mod+-` | `View` のみ | |
| KG-KEY-12 | 横スクロール | `view.scroll-left`/`view.scroll-right` | `Left`/`Right` | 同左 | `View` のみ | 縦スクロール `view.scroll-up`/`down` は `Up`/`Down` に割当(§7.2「横スクロール」の同族操作として追加、Decision KG-D-7) |
| KG-KEY-13 | 保存 | `project.save` | `Mod+S` | `Mod+S` | `View` のみ | |
| KG-KEY-14 | Export 開始 | `project.export` | **既定バインドなし** | 同左 | — | §7.2 備考「メニュー/Command」に従い Menu/Command Palette のみ。理由は Decision KG-D-3 |
| KG-KEY-15 | アプリ終了 | `app.quit` | `Mod+Q` | `Mod+Q` | §4.1 行列に従う(Application lane) | OS のウィンドウ閉鎖(`Alt+F4`/`Cmd+W`)は #113/#175 の責務で本契約の対象外 |

補足:

- `view.activate`(タブ/ビュー切替)・`project.rename`・
  `clip.mute`・`clips.delete-range` は §7.2 最小集合に
  キー操作として含まれないため既定バインドを持たない。
  メニュー/コンテキストメニュー経路で到達可能(§6)。
- `Esc` は既定バインドを持たない予約キー(§5.1)。
- `Mod+X`(切り取り)は §7.2 最小集合に含まれないため既定
  バインドなし。Clipboard 系の拡張はリマップ/後続 slice。

### 3.3 OS 正規化(KG-D-02)

- **論理キー表記**: 既定バインドは OS 非依存の論理形
  (`Mod`/`Shift`/`Alt`/キー名)で定義する。`Mod` は
  Windows/Linux で `Ctrl`、macOS で `Cmd` に解決される。
  macOS の `Cmd+Z`/`Cmd+S`/`Cmd+Q`/`Cmd+C`/`Cmd+V` は
  OS 慣習と一致し、Windows/Linux の `Ctrl+…` と意味的に
  同一の操作へ写像される。
- **OS 固有の差分**: 既定セットは `Mod` で表現できる範囲に
  限定し、OS 別の個別バインド分岐を持たない。例外が必要に
  なった場合(例: macOS で `Cmd+Backspace` が Finder 的に
  特殊)は Decision Log に記録して個別規定する。
- **OS 予約衝突の回避**: 既定セットは OS/WM 予約と衝突しない
  (`Alt+F4`/`Alt+Tab`/`Ctrl+Alt+Del`/`Cmd+Space`/`Cmd+Tab`/
  `Cmd+H`/`Cmd+M`/`Ctrl+Esc`/Win キー系を使用しない)。
  `Space`/`Enter`/`R`/`V`/`B`/`S`/`T`/`E`/矢印/`Delete`/
  `Backspace` は修飾なしのアプリ内キーであり、TextInput/
  Ime では §4 のゲートにより DAW Command へ到達しない。
- **リマップ保存形**: ユーザー再割当(後続 slice)も論理キー形
  で保持し、OS ごとの物理修飾子へは表示・配送時に解決する。
  これにより Project/設定の OS 間持ち運びでバインド意味が
  保存される。
- **`Mod++` の扱い**: `=` キーの Shift 付き(`+`)入力は
  レイアウト依存のため、`Mod+=` を正規形とし、`Mod+Shift+=`
  を同一 binding の別名として許容する(実装 slice でキー名
  解決を確認)。

## 4. Input-Context Dispatch & Suppression(KG-D-03)

### 4.1 発火行列

`InputContext`(`focus.rs`: `View`/`TextInput`/`Ime`/
`PluginGui`/`Modal`)× command lane の既定バインド発火可否。
`✓` = dispatch へ配送、`T` = Undo lane の text 系配送、
`S` = 抑制(発火しない)。

| InputContext | View lane(tool/zoom/scroll/view.activate) | Undo lane | Project/Transport/Editing lane | Application lane |
| --- | --- | --- | --- | --- |
| `View` | ✓ | Project 履歴(`dispatch_undo` View 分岐) | ✓ | ✓ |
| `TextInput` | S | T(text lane) | S | ✓ |
| `Ime` | S | T(text lane、現行 `dispatch_undo` と同じ) | S | S |
| `PluginGui` | S | S | S | S |
| `Modal` | S | S | S | ✓ |

凡例の根拠:

- **Project/Transport/Editing**: `dispatch` が
  `ctx.input.captures_input()`(= `View` 以外)で
  `Suppressed` を返す既存契約と一致。keymap 層はこの結果を
  そのまま honest に扱う(§4.4)。
- **Undo**: `dispatch_undo` の context 別配送に一致。
  `TextInput`/`Ime` は text lane stub、`Modal`/`PluginGui` は
  `Suppressed`。keymap は `edit.undo`/`edit.redo` をこれら
  context でも dispatch へ届け、lane 解決は router に委ねる
  (keymap が事前抑制すると text undo へ届かなくなる)。
- **View lane**: 現行 `dispatch` は View kind を suppression
  対象に含めない。したがって **keymap 層で `View` context
  のみ発火**とする契約上のゲートが必要(§4.4 KG-D-4)。
  `tool.activate`/`view.zoom-*`/`view.scroll-*`/
  `view.activate` が TextInput 中の文字キーや Modal 中に
  発火しないことを保証する。
- **Application**: `app.quit` は Project/Transport/Editing/
  View のいずれでもなく session レベル。`View`/`TextInput`/
  `Modal` では発火を許容し、`Ime`(変換中)/`PluginGui`
  (プラグインがキーを所有)では抑制する。OS 慣習上
  `Cmd+Q`/`Ctrl+Q` はダイアログ中・文字入力中でも終了を
  期待されるため。

### 4.2 Dispatch pipeline(契約上の流れ)

1. OS/GPUI のキーイベントを受ける。`InputContext` が
   `TextInput`/`Ime` の場合、**印刷可能文字キーは先に
   テキストフィールド/IME 変換へ配送**され、keymap 照合の
   対象外とする(検索欄の `Space`/`Delete`/`R` 等を DAW
   Command 化しない — workspace-ux §9.1 と整合)。
   修飾付き combo(`Mod+Z` 等)のみ keymap 照合へ進む。
2. `FocusState.input` の context lane を決定し、§4.1 行列で
   binding の発火可否を判定する。
3. 発火する binding を論理キー→contract command id へ解決し、
   `CommandContext { id, origin_window, origin_view,
   input, explicit_targets: [], selection: None,
   expected_revision: None, arg }` を構成する。
   - `origin_window` = ネイティブフォーカス中の window、
     `origin_view` = その window の active view
     (`FocusState` の 3 概念は独立 — focus.rs)。
   - `expected_revision` は keymap 発行では `None`
     (現行受理)。K-1 の対象 ID+Revision 固定は gesture
     経路・entity command の router 側契約が担い、keymap は
     迂回しない。
4. `WorkspaceSession::dispatch`(workspace.rs)へ渡し、
   `CommandOutcome` をそのまま観測する。

### 4.3 Undo lane の context 別配送

`edit.undo`/`edit.redo` は `CommandKind::Undo` であり、
kind 別 suppression の対象外。`dispatch_undo` が
context 別に配送する:

| context | 配送先 | Project 影響 |
| --- | --- | --- |
| `View` | `session.undo()`/`redo()`(Project 履歴) | あり(revision 前進) |
| `TextInput`/`Ime` | text lane stub(`text_lane_undos`) | なし |
| `Modal`/`PluginGui` | `Suppressed` | なし |

これは workspace-ux §9.4 の Undo 3 系統分離(Project/Text/
Layout)と整合する。keymap は `Mod+Z`/`Mod+Shift+Z` を
`View`/`TextInput`/`Ime` で dispatch へ届け、Modal/
PluginGui では §4.1 に従い keymap 層で抑制する
(dispatch に届けても `Suppressed` になるが、二重の
抑制理由を避け keymap で一貫して止める)。

### 4.4 View/Application lane のゲート配置(KG-D-4)

現行 `dispatch` の suppression は `Project | Transport |
Editing` kind のみで、`View`/`Application`/`Undo` は対象外
(`Undo` は `dispatch_undo` が個別処理)。契約上必要な
振る舞いは:

- View lane binding は `InputContext::View` でのみ発火。
- Application binding は `View`/`TextInput`/`Modal` で発火、
  `Ime`/`PluginGui` で抑制。

このゲートを **keymap 層**に置くか `dispatch` の suppression
集合拡張で実現するかは実装 slice の選択に委ねる
(spec Deferred Design Concerns)。契約が固定するのは
**観測可能な振る舞い**(どの context で発火/抑制されるか)
であり、層の配置ではない。実装が `dispatch` 側へ寄せる
場合でも §4.1 行列と矛盾してはならない。

### 4.5 Outcome の正直さ

keymap 経由の dispatch が `Suppressed`/`Unresolved`/
`Rejected` を返した場合、keymap 層はそれを握り潰さず、
UI へ「無効」「対象なし」「失敗」の区別可能な形で反映する
契約とする(既存 View の unavailable/disabled 表示と整合)。
`Suppressed` はエラーではなく仕様上の抑制であり、
`Unresolved`(対象未解決)は特に destructive command で
実行されないことを意味する — いずれも fake success に
しない。

## 5. Safety Contracts for Key Operations(KG-D-04)

### 5.1 K-2 のキー操作への適用

- **1 キー Command = 1 Undo 単位**: keymap 発行の Project
  変更 Command は、dispatch 1 回につき controller 履歴上
  1 undo 単位とする。キー操作が複数の ProjectOp に分割
  されることはない(`edit.delete`→`clip.remove` 等の解決は
  1 Command として扱う)。Window/Dock/Focus 変更・View 切替・
  Tool 切替は Undo 対象外(daw-workflow §8.2 と整合)。
- **Gesture Preview 中のキー**: §7.3 の Gesture は
  pointer 起点であり、既定キーマップは Preview を開始する
  バインドを持たない。Gesture Preview が active な間は、
  編集系 binding の発火を抑制し、確定は pointer の
  Commit のみ、取消は `Esc` のみとする
  (workspace-ux §9.1「Gesture Preview 中」行と整合)。
  keyboard 起点の transient 操作を将来追加する場合は
  K-2(Preview→Atomic Commit)と transient stack への登録・
  `Esc` 階層取消を必須とする。
- **Esc の予約**: `Esc` は既定バインドを持たず、最内側の
  transient 状態を取消す専用キーとする。取消順序は
  workspace-ux §9.3 に従う: **Gesture Preview > Dock Drag >
  Modal/メニュー**。transient が存在しない場合の `Esc` は
  no-op(「選択解除」等への拡張は最小集合外・リマップ候補)。

### 5.2 K-4 のキー操作への適用

- `tool.activate` は `origin_view` 単位の View 状態
  (`set_tool(view, tool)`)を変更する。keymap は `V`/`B`/`S`/
  `T`/`E` 押下時の **フォーカス中 window の active view** を
  `origin_view` として `tool.activate` を発行し、Tool 状態を
  keymap が直接書き換えることはない。
- Tool 状態は View ごとに独立し、Window/Dock 移動・View 開閉・
  Focus 変更で変更されない(Navigate/Arrange 分類 —
  daw-workflow §8.1・#11 契約)。キー操作がこれを侵さない
  ことを契約で固定する。
- active view が解決できない(フォーカス中 window に active
  view が無い)場合、Tool/scroll/zoom 系 binding は
  `Unresolved` を返し、別 View の Tool 状態へ推測で
  フォールバックしない。

### 5.3 Auto-repeat(キーリピート)方針

- 既定バインドは **edge-triggered を原則**とし、OS の
  key-repeat による再発火を抑制する。トグル系
  (`transport.toggle`/`transport.record`)・破壊系
  (`edit.delete`)・clipboard 系は repeat を受け付けない
  (長押しでの状態ストロボ・連続削除を防ぐ)。
- 例外的に repeat を許容するのは `view.scroll-*`(連続
  スクロールが期待動作)と `edit.undo`/`edit.redo`
  (長押し連続 undo は一般的 UX)のみ。
- repeat 許容集合は実装時に本表へ一致させる。判定は
  contract command id 単位で行い、物理キー単位ではない。

### 5.4 Key conflict resolution policy

- **KG-C-1 lane 内一意性**: 同一 context lane 内で、既定
  キーマップは (修飾子+キー) に対して高々 1 binding。
  §3.2 の既定セットはこの不変条件を満たす(検証可能)。
- **KG-C-2 context lane 分離**: 同一物理 combo が異なる
  context lane で異なる意味を持つことは許容するが、既定
  セットでは発生させない(例: `Delete` は `View` lane の
  `edit.delete` のみ。Modal 内の `Delete` はダイアログ自身の
  振る舞いであり DAW binding ではない)。
- **KG-C-3 chord 非導入**: 既定セットは単打鍵のみで、
  multi-stroke chord を持たない。prefix 衝突は構造的に
  発生しない。chord 対応は後続 slice。
- **KG-C-4 OS 予約**: §3.3 の OS/WM 予約衝突回避を既定
  セットの不変条件とする。
- **KG-C-5 リマップ競合拒否**: ユーザー再割当(後続 slice)
  が lane 内一意性・OS 予約を破る場合、その割当を拒否し
  理由を表示する。暗黙の上書き・後勝ち解決は行わない。
- **KG-C-6 優先順位**: キー解決順序は (1) TextInput/Ime の
  印刷可能キー消費 → (2) context lane 判定(§4.1)→
  (3) lane 内 binding 照合 → (4) 未割当キーは no-op
  (View ローカル処理・将来の view 固有 binding の余地を
  残す)。

## 6. Context Menu Coverage Contract(KG-D-05)

### 6.1 規則

- **CM-1 網羅対応表**: §6.2 の対応表が contract inventory
  である。catalog の全 contract command id は少なくとも
  1 つのメニュー面に現れるか、「メニュー面なし」の理由が
  明示される。新規 command 追加時は本表の更新を契約保守
  義務とする。
- **CM-2 accelerator 整合**: キー割当を持つ command の
  メニュー項目は accelerator 表示を持ち、表示は OS 慣習の
  修飾子表記(`⌘`/`Ctrl`)で §3.2 の既定バインドと一致する。
  表示と実 binding の乖離は許容しない。
- **CM-3 対象 ID 明示**: コンテキストメニュー起点の
  CommandContext は `explicit_targets` にクリック対象の
  entity を設定する。メニュー表示中の別 View の選択変更へ
  対象をすり替えない(workspace-ux §9.2・command.rs の
  explicit_targets 優先と整合)。
- **CM-4 Modal 境界**: メニュー表示中は Modal 的入力境界
  として扱い、背後 View へのキー漏れを防ぐ。メニュー項目
  の起動は **`input = View` + `explicit_targets`** の
  CommandContext を発行する — Modal 境界は「メニュー表示中
  のキー配送」を縛るものであり、メニュー自身が発行する
  Command の `input` を `Modal` にすると Project/
  Transport/Editing が suppression されて何も実行できない
  ため、発行側はメニュー起動前の View context を引き継ぐ。
- **CM-5 無効表示の正直さ**: 現在の状態で解決不能な command
  のメニュー項目は disabled(使用不可)として表示し、
  fake enabled・項目の隠蔽で誤魔化さない
  (workspace-ux §9.1「無効なコマンドは使用不可であることを
  示す」と整合)。disabled 理由の表示は View 実装 slice の
  任意拡張。
- **CM-6 同一 pipeline**: メニュー起点の command も
  `dispatch` の同一経路を通り、`Suppressed`/`Unresolved`/
  `Rejected` の outcome をキー起点と同じく正直に扱う。

### 6.2 Coverage inventory(contract 面 → command)

メニュー面の分類: **MB** = アプリケーション メニュー
バー、**AC** = Arrangement コンテキストメニュー
(Clip/Lane/背景)、**VC** = 汎用 View コンテキストメニュー。
Mixer/Inspector 固有の面は View 実装 slice で追加される
際に本表へ行を追加する(網羅規則 CM-1 が保守を強制)。

| contract command | MB | AC | VC | 備考 |
| --- | --- | --- | --- | --- |
| `app.quit` | File > Exit | — | — | OS ウィンドウ閉鎖は #113/#175 |
| `project.save` | File > Save(`Mod+S`) | — | — | |
| `project.export` | File > Export…(キーなし) | — | — | KG-KEY-14 |
| `project.rename` | File > Rename…(ダイアログ) | — | — | |
| `edit.undo` / `edit.redo` | Edit > Undo/Redo | — | — | |
| `edit.copy` / `edit.paste` / `edit.delete` | Edit > Copy/Paste/Delete | AC(選択依存) | VC(選択依存) | `edit.delete` は domain 解決 |
| `clip.remove` / `clip.mute` | — | AC(Clip 上) | — | explicit target=clip |
| `clips.delete-range` | Edit > Delete Range | AC(Time Selection 上) | — | range 削除の明示 command |
| `transport.toggle` / `transport.stop-return` / `transport.record` | Transport > Play-Pause/Stop/Record | — | — | |
| `transport.play` / `transport.stop` | — | — | — | contract command の内部解決先。メニュー項目は contract id を指す |
| `tool.activate` | View > Tool(submenu) | — | VC(ツール picker、View slice) | arg=tool 名 |
| `view.zoom-in` / `view.zoom-out` | View > Zoom | — | — | |
| `view.scroll-*` | View > Scroll(省略可) | — | — | キー+ホイールが主経路。項目を置くかは View slice 判断、contract は省略を許容 |
| `view.activate` | Window > Tabs/View 一覧 | — | — | タブ切替 UI、既定キーなし |

「メニュー面なし」と明示するもの: `transport.play`/
`transport.stop`(contract command の内部解決先であり、
メニュー項目は contract id を指す)。`view.scroll-*` は
キー/ポインタが主経路のためメニュー項目の設置を省略
してよい(上表「省略可」)。

## 7. Verification lanes

| レーン | 対象 | 本設計での位置づけ |
| --- | --- | --- |
| 文書契約(headless・今回) | 割当表の §7.2 網羅性・lane 内一意性・OS 正規化・§4.1 行列と `captures_input`/`dispatch` の整合・§6.2 網羅対応 | `doc_status` lint/audit・index 再生成・`fmt-check` で検証。`evidence/sv112/` に記録 |
| headless(実装 slice) | keymap テーブルの injectivity/網羅性、OS 正規化写像、InputContext×lane 行列の単体検証、競合拒否、Esc 順序、メニュー網羅対応の契約試験 | test-spec の TS-EX 群として定義。実装 slice で実走 |
| GUI-L2(#114) | 実 Window/実キー入力/IME/Focus/DPI での dispatch・既定バインド・Plugin GUI キー所有 | **NotRun/Deferred**。実機 lane |
| EXT-L3(#176) | 実 Plugin GUI でのキー所有・ホスト規約 | **Deferred**。実機 lane |

## 8. Decision Log

| ID | 決定 | 理由 | 代替案 | 状態 |
| --- | --- | --- | --- | --- |
| KG-D-1 | `Mod` 論理修飾キーで既定バインドを定義 | Win/Linux=Ctrl・macOS=Cmd の OS 差異を 1 表で吸収し、リマップ保存形とも整合 | OS 別に 2 表を維持 | 採用 |
| KG-D-2 | `Space`=transport.toggle・`Enter`=stop-return・`R`=record | §7.2 の Play/Pause・Stop+Return・Record を DAW 慣習の単打鍵でカバー。`Enter` は View context で未使用かつ「確定/実行」の語義に合う | `Shift+Space`・テンキー `*`・`Mod+Enter` | 採用(実機 PoC で違和感あればリマップで吸収) |
| KG-D-3 | Export に既定キーを割り当てない | §7.2 備考が「メニュー/Command」を指定。低頻度・dialog 起動型で誤爆コストに見合わない | `Mod+E` 等を既定割当 | 採用 |
| KG-D-4 | View/Application lane の suppression を keymap 層ゲートとして契約化(配置は実装委ね) | `dispatch` は View/Application kind を抑制しないため、TextInput/Modal で tool/zoom が発火しないことを保証する層が必要。振る舞いを固定し層選択を実装へ委ねる | `dispatch` の suppression 集合を拡張する実装を契約で強制 | 採用(振る舞い固定・配置は Deferred) |
| KG-D-5 | auto-repeat は scroll/undo のみ許容、他は edge-triggered | 長押しトグルのストロボ・連続削除を防ぎつつ連続スクロール/undo の UX を保持 | 全面 repeat 許容 / 全面禁止 | 採用 |
| KG-D-6 | メニュー発行 Command は `input=View`+`explicit_targets` | Modal 境界はキー配送を縛るもので、メニュー自身の Command を Modal input にすると suppression で何も実行できない | メニュー発行を `input=Modal` にして dispatch を迂回 | 採用 |
| KG-D-7 | 矢印 4 方向を scroll に割当・Tool は V/B/S/T/E の 5 個 | §7.2「横スクロール」の同族として縦も収録。Tool は Smart/Draw/Split/Trim/Erase の主要専用 tool を網羅し、Select/Slip/Stretch/Fade/Marquee は picker/リマップへ | 矢印を nudge へ割当(#11 未確定)・Tool 全種にキー割当 | 採用 |
| KG-D-8 | `[contract]` id を `spec_for` 登録と分離して命名 | 登録語彙の拡張は実装 slice の責務。契約は id・kind・解決規則を先に固定し、実装が追従できる | 登録済み id のみで表現(不足分を曖昧に残す) | 採用 |

## 9. Risks and Deferred

### 9.1 Risks

- **R-1**: 既定セットが workflow で不足 → メニュー面・
  リマップ・§7.2 改訂で吸収。最小集合の枠内で追加 binding
  は Decision Log 記録を要する。
- **R-2**: View/Application suppression の実装配置が
  keymap 層と `dispatch` で二重化 → 契約は振る舞いのみ
  固定。実装 slice で一箇所へ集約することを推奨(第二正本
  化の兆候として監視)。
- **R-3**: `Ime` の Undo 配送が workspace-ux §9.1 発火表
  (「発火しない」)と現行 `dispatch_undo`(text lane 配送)
  で不整合 → 現行コード挙動に合わせて契約化し、実機
  IME での期待動作は #114 で検証・必要なら #3/#11 側の
  文書照合を行う。
- **R-4**: `Enter`=stop-return が将来の「確定」操作と衝突
  → View context の既定セット内では一意。dialog 確定は
  Modal context で DAW binding 対象外のため実害なし。

### 9.2 Deferred

- keymap dispatch 実装・`spec_for` への `[contract]` id 登録
  → #112 実装 slice。
- リマップ UI・保存形式・競合 UI → 後続設定 slice。
- chord/多段キー・キーリピート微調整・OS 予約の網羅一覧
  → 実装 PoC・実機実測。
- コンテキストメニュー各面の最終項目/文言/構造 → View 実装
  slice(#112 後続)。
- 実 OS/実 GUI/実機キー・IME/Plugin GUI 検証 → #114/#176。

## 10. Verification Notes

- 本設計は文書契約。実機/実 GUI 検証は行わない。
- `evidence/sv112/` に委譲項目→契約セクション→受入状況の
  case ledger と検証記録を作成する。
- 実装 slice は §3.2 割当表・§4.1 行列・§5.4 競合ポリシー・
  §6.2 網羅対応を headless 契約試験(test-spec TS-EX 群)で
  検証し、実機 lane は #114/#176 へ委譲する。
