---
id: "SPEC-034QAlF2irl9rIJaUi9RQ1"
type: "spec"
status: "approved"
title: "ED-A Editing Model — Selection/Tool/Snap/Transaction/LinkedContext headless PoC"
created: "2026-11-02"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/38"
    - "https://github.com/scarecrow173/synvelis/issues/11"
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
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/ed-a-editing-model-placement-synvelis-workspace-gesture-tool-snap-state-machines.md"
    - "docs/designs/ed-a-editing-model-design.md"
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/11"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/190"
    - "https://github.com/scarecrow173/ruviform/issues/243"
  defers: []
  deferred-by: []
---

# ED-A Editing Model — Selection/Tool/Snap/Transaction/LinkedContext headless PoC

## What Should Be Built

Issue #38: #11 で合意済みの編集操作体系 (合意 5 項目) を、#36/PR #37 で
確立した `synvelis-workspace` (ViewRegistry / per-view state / LinkGroup /
CommandRouter / ProjectSession) と `synvelis-project` (Track/Lane/Clip/
Source model・`ProjectController`) の上に **headless PoC** として実装し、
合意の各契約が不変条件として守られることを headless test で検証する。

## Why Now

#11 の基本方針 5 項目が合意済みで、WS-A が Selection/Focus/Command/Link の
責務分離と単一 Project 正本・revision 配信の基盤を提供した。編集モデルは
その基盤の直上に載る次の検証対象であり、real-GUI lane (ruviform #243) へ
渡す前に契約の責務境界を headless で固定しておく必要がある。

## Users and Value

- DAW ユーザー: Arrangement / Piano Roll / Audio Editor 等の View で同じ
  編集原則が一貫して働き、誤った Domain・古い Revision・曖昧な Time
  Domain への誤編集が起きない。
- 開発者: Gesture 固定・Preview/Commit 分離・Snap 候補選択・Undo 配送が
  契約としてテスト固定され、後続の real-GUI・Take/Comp・remote asset
  lane の基礎になる。

## Scope

### In Scope

1. **Selection Model**: Object Selection / Time Selection / Lane Scope を
   View ごとに分離して保持。Selection は Domain 別に独立保持し、Primary
   Target を明示。Inspection Focus / Linked Context / Active View と別概
   念のまま。Time Selection だけでは破壊的編集を実行しない
   (range 削除は独立 command)。
2. **Tool System**: Smart Tool + Dedicated Tool (Select/Draw/Split/Trim/
   Slip/Stretch/Fade/Erase) の解決規則。Hit Test は headless な意味的
   probe で供給し、優先順 (Handle → 編集対象 → Clip Body → Lane Empty →
   Background) と Gesture 開始時の操作種・対象・Revision 固定を検証する。
3. **Snap/Grid**: Grid Display と Snap Enabled の分離。Snap Unit の
   明示設定は Zoom で変化しない (自動分解能は別モード)。Time Domain を
   明示し、候補を対象・Domain で絞り、距離 + 明示 Priority + Hysteresis
   で採用し、「何に Snap したか」を返す。複数対象 Move は Primary のみ
   Snap、残りに同一 delta を適用。混在 Anchor Domain の一斉移動は
   共通 Domain 明示なしに拒否する。
4. **Edit Transaction**: Idle → PointerDown/HitTest → Resolve →
   GestureStarted → Previewing → Commit or Cancel。Preview は View 側の
   一時状態で Project/Undo に書かない。Release 時に全件検証 →
   `ProjectController` へ 1 回の Atomic Commit (=1 Undo)。開始後に
   Revision が変われば拒否 (自動 Rebase/すり替えなし)。Esc /
   PointerCancel / Window 破棄 / Target 削除 / Input Scope 喪失で未確定
   Preview を破棄。Undo/redo は session engine hook 経由で配送し、
   Project undo と Text/IME undo の lane を分ける。
5. **Cross-View**: Linked Context (非破壊 navigation) と Editing
   Selection / Command Target の分離。Editor の Display/Edit Binding と
   進行中 Gesture の Target を分離 — Follow 更新や他 View の Selection が
   Gesture 対象を変えない。Linked Source 編集 binding の concept として
   `EditBinding { clip, source }` を保持する (内容編集自体は対象外)。

### Out of Scope

- 実 GUI / px 閾値 / OS キーバインド / Pointer Device 差異 / Hover /
  実スクロール・AutoScroll (real-GUI lane: ruviform #243 と同レーンで後続)。
- Audio/MIDI 各 Domain の個別編集アルゴリズム確定 (Take/Comp 詳細 #10/#12、
  Warp、Quantize、実 Note/Automation entity)。
- Remote Asset 連携 (#9)・録音ライフサイクル (#8)。
- ruviform 側変更・ruviform #189 (rational beat) / #190 (transport) の再実装
  や workaround。
- Snap Magnetic mode・混合 Time Domain の安全演算契約 (#11 §9 deferred)。

## Requirements

- Selection/Tool/Preview/Gesture 状態は全て **View State** (session scoped) —
  Project データ・Undo 履歴・Semantic Diff に混入しない。
- 時間演算は既存規約どおり checked 演算のみ (`MusicalTick::checked_add` 等。
  `Tick`/`TickDuration` の unchecked `Add`/`Sub` は使わない)。
- Project 変更は `ProjectController` 経由のみ。複数対象の Gesture commit は
  1 回の atomic apply (= 1 undo 単位)。全件検証失敗時は正本不変 + 理由通知。
- Anchor Domain が混在する対象への一斉編集は、明示された共通 Domain と
  `TickTimeMapping` 経由の変換なしに行わない (PoC では同一 Domain のみ
  確定、混合は拒否して正直に記録)。
- 新規 `ProjectOp` は `synvelis-project` に追加 (DAW 固有編集意味は
  Synvelis 側)。ruviform 契約型の再実装・重複実装をしない。

## Acceptance Criteria

- [ ] **ED-A-01**: View A と View B の Object Selection / Time Selection /
  Lane Scope が互いに独立。Clip domain と Track domain の selection が並存
  し、domain 非対応の command は `Unresolved`。Primary Target が明示的に
  保持される。Time Selection 単独では `clip.remove` は Unresolved で、
  range 削除は独立 command (`clips.delete-range`) のみ。
- [ ] **ED-A-02**: Smart Tool が HitTest 優先順 (Handle > Body > Lane Empty
  > Background) に従って操作を解決し、Dedicated Tool は明示固定。
  Gesture 開始後に pointer が別 zone へ来ても操作種・対象が変わらない。
  曖昧な hit は `Ambiguous` として候補と代替 Tool を返す。
- [ ] **ED-A-03**: Grid 非表示でも Snap が動作し、Zoom で明示 Snap Unit が
  変化しない (Auto のみ zoom 追従)。候補は Domain・targets 設定で絞られ、
  距離 + Priority + Hysteresis で採用、採用候補を label で報告。複数 Clip
  Move で Primary のみ Snap、相対位置が保持される。Musical/Media 混在
  Move は共通 Domain 未宣言で拒否。
- [ ] **ED-A-04**: Gesture Preview 中は project revision・undo 履歴・
  engine `graph_builds` が不変。commit は全件検証 → 1 回の atomic apply →
  1 undo で全件復旧。開始後に対象が変更/削除されれば拒否・すり替えなし。
  Cancel 経路 (Esc/PointerCancel/Window 破棄/Target 削除/Input Scope 喪失)
  で project 不変。undo/redo が `ProjectSession` engine hook 経由で
  `graph_builds` を進め、TextInput 文脈の undo は Text lane に配送される。
- [ ] **ED-A-05**: Link event 配送が Gesture 中の固定 Target を変えない。
  Piano Roll への `EditBinding` 設定が Arrangement の Selection を変えず、
  binding は command target に解決されない。Linked Context / Inspector の
  read-only 選択が Editing Selection・Project を変更しない。
- [ ] **ED-A-06**: `mise run fmt-check`/`check`/`lint`/`test` 全 green、
  impl record に検証表・ruviform rev・未検証項目を正直に記録。

## Deferred Design Concerns

- Smart Tool の px Hit Size / Drag Threshold / Hover・Pen・Touch 差異 |
  #11 §9-1 | real-GUI lane (#243) で確定 | headless は意味的 probe のみ。
- Snap Threshold/Hysteresis/Candidate Ranking の数値・Magnetic mode |
  #11 §9-3 | UI PoC で確定 | 現在は tick 単位の単純閾値のみ。
- 混合 Time Domain の安全な共通基準演算 | #11 §9-4 | 後続設計 | PoC は
  同一 Domain のみ commit、混在は拒否。
- Note/Automation entity の domain と編集 op | #10/#6 | entity model 追加後 |
  SelectionDomain は現行 SemanticTarget で表現できる範囲のみ。
- Ripple (gap-closing) 削除の意味規則 | #11 §1 | Arrangement 編集詳細で
  確定 | PoC の `delete-range` は重複 clip の除去のみ・shift なし。
- Slip/Stretch/Fade の実 commit 意味 | 個別 Domain アルゴリズム確定後 |
  同 | PoC では tool/handle 解決まで、commit は Unsupported で拒否。

## Sources and Relations

- Issue #38 本文 (スコープ・受入証拠・非対象・制約)
- Issue #11 本文 (合意 5 項目の原仕様)
- `docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md`
  (WS-A 基盤・#36/#2/#3)
- `docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md`
  (F0: ProjectController・Stable ID・revision/undo)
- `docs/designs/configurable-ppqn-musical-time-architecture.md` (#19/rf#189)
- `docs/designs/time-transport-midi-automation-plugin-integration-architecture.md`
  (#20/rf#190)
