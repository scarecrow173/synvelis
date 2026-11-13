---
id: "SPEC-BCK7X1Qx5rbH151Bo7ftE7"
type: "spec"
status: "approved"
title: "Timeline Editing / Selection / Tool System — Snap・Gesture・Undo・Cross-View編集の共通仕様（synvelis#11）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
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
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
    - "docs/specs/ed-b-media-domain-move-slip-commit-semantics.md"
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
    - "docs/specs/workspace-model.md"
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
    - "docs/specs/context-inspector.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/timeline-editing-design.md"
    - "docs/plans/timeline-editing.md"
    - "docs/tasks/timeline-editing-task.md"
    - "docs/test-specs/timeline-editing.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/11"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/2"
    - "https://github.com/scarecrow173/synvelis/issues/3"
    - "https://github.com/scarecrow173/synvelis/issues/5"
    - "https://github.com/scarecrow173/synvelis/issues/6"
    - "https://github.com/scarecrow173/synvelis/issues/7"
    - "https://github.com/scarecrow173/synvelis/issues/8"
    - "https://github.com/scarecrow173/synvelis/issues/9"
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/19"
    - "https://github.com/scarecrow173/synvelis/issues/20"
    - "https://github.com/scarecrow173/synvelis/issues/36"
    - "https://github.com/scarecrow173/synvelis/issues/38"
    - "https://github.com/scarecrow173/synvelis/issues/42"
    - "https://github.com/scarecrow173/synvelis/issues/52"
    - "https://github.com/scarecrow173/synvelis/issues/65"
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/ruviform/issues/189"
    - "https://github.com/scarecrow173/ruviform/issues/190"
    - "https://github.com/scarecrow173/ruviform/issues/243"
  defers: []
  deferred-by: []
---

# Timeline Editing / Selection / Tool System — Snap・Gesture・Undo・Cross-View編集の共通仕様

## What Should Be Built

synvelis#11（設計 Issue）の成果物として、Arrangement / Piano Roll /
Audio Editor / Automation Lane で **同じ編集原則を共有し、各 Domain の
意味に応じて対象を編集する共通 UX 仕様**を定義する。Issue #11 で
合意済みの 5 項目（Selection 分離・Smart/Dedicated Tool・Grid/Snap
分離・Drag Preview/Atomic Commit/1 Undo・Cross-View Linked Context
分離）を、#2（Workspace/Selection/Command）・#10（Project Model）・
#12（Recording/Take/Comping）の確定契約と対応付け、実装 Owner が消費
できる製品契約・受入 Fixture・証拠分類として固定する。本 spec が固定
する契約領域:

1. **Selection の意味分離** — Object Selection / Time Selection /
   Lane Scope / Primary Target / Inspection Focus / Active View・Input
   Focus / Linked Context を別概念として View ごとに保持する。
   Time Selection と Object Selection は両立するが、Time Selection
   だけで Ripple Delete を暗黙実行しない（Time Delete / Ripple
   Delete は独立 Command）。
2. **Smart Tool + Dedicated Tool** — Smart Tool は Hit Test 結果から
   操作を解決し（明示 Handle → 編集対象 → Clip Body → Lane Empty →
   View Background の優先順）、Gesture 開始後は操作種・対象を固定し
   途中切替しない。極小/近接 Handle の曖昧さは Cursor/Tooltip と
   Dedicated Tool で解決する。
3. **Grid Display と Snap の分離** — Grid 表示と Snap Enabled/Unit/
   Targets/Mode/Threshold は独立設定。Grid 非表示でも Snap は動作し、
   Zoom で明示 Snap 分解能は変化しない（自動分解能は別モード）。
   Musical/Absolute 混在の時間 Domain で暗黙変換を行わない。
4. **Gesture → Preview → Atomic Commit → 1 Undo** — Drag 中の
   Preview は View 側の一時状態で Project/Undo を変更しない。
   Release 時に全件検証 → Project Controller へ 1 回の Atomic
   Commit（=1 Undo）。開始時に Intent+Target+Revision を固定し、
   Esc/PointerCancel/Window 破棄/Target 削除/他 Window からの変更で
   誤った Target/Revision へ適用しない。
5. **Cross-View Linked Context と Edit Command Target の分離** —
   Inspector/Signal Flow/Version Diff の Explore/Linked Highlight は
   Editor の Edit Command を上書きしない。Piano Roll での Linked
   Source 編集は共有 Clip に反映するが Arrangement Selection を
   不要に変化させない。Read-only VCS Snapshot の選択は Live
   Project を変更しない。
6. **状態の保存境界** — Selection/Tool/Zoom/Snap は View State、
   Project Undo は Project Controller、View Navigation/Workspace
   Layout は View/Workspace、Revision/Commit は VCS Provider が所有
   し、Snap/Tool 変更だけで Project Dirty/Semantic Diff を発生
   させない。

## Why

Issue #11 の 5 項目は 2026-11-04 の設計議論で合意済みで、ED-A（#38/
PR #39）・ED-B（#65/PR #66）の headless PoC が基盤契約を実装済みだが、
Arrangement/Piano Roll/Audio Editor/Automation Lane を跨ぐ **製品 UX
としての共通仕様・画面操作 Fixture・受入条件の被覆分類**が文書とし
て固定されていなかった。本 spec と対応する design/plan/task/test-spec
は、その合意を実装 Owner（#112 Core GUI・#114 実機 Gate、および
Project/Workspace/GUI の各実装 Issue）が消費できる契約と証拠分類へ
落とす。設計レビュー済みの契約と、実装・実 OS・実 Plugin でしか証明
できない Gate を分離し、headless 成功を GUI Pass として主張しない。

## Users and Value

- DAW ユーザー: どの Editor View でも同じ編集原則（選択・Tool・Snap・
  Undo・Cross-View 関係）が一貫して働き、誤選択・誤 Domain 変換・
  誤 Target への編集・Undo の分断が起きない。
- 後続実装 slice（#112/#114・#10/#12 の実装 Owner）: Selection/Tool/
  Snap/Gesture の契約・Fixture・Case ID から実装・検証できる。
- レビュアー/Coordinator: 「設計合意」「headless 実装済み」「製品
  受入済み」の 3 状態を区別できる証拠分類を得る。

## Acceptance Criteria

Issue #11 §8 の受入条件を TE-01〜TE-14 として固定する。各行の現在の
被覆状況は `evidence/sv11/coverage.md` の台帳を正本とし、本 spec は
要求のみを定義する（設計のみの項目は `NotRun`）。

- [ ] **TE-01**: Clip の Click と Drag が誤判定されず、専用
  Tool/Smart Tool が Gesture 中に途中切替されない。
- [ ] **TE-02**: 近接/極小 Clip Handle でも編集意図が確認でき、
  適切な Alternative Tool で操作可能。
- [ ] **TE-03**: Time Selection + Clip Selection が両立し、
  Time-only で Delete が勝手に Ripple 動作しない。
- [ ] **TE-04**: 5 つの Clip を同時移動すると相対位置が保たれ、
  一回 Undo で全件復旧。
- [ ] **TE-05**: Musical/Absolute 混在 Move と Snap で時間 Domain が
  不明な暗黙変換を行わない。
- [ ] **TE-06**: Grid を非表示にしても Snap は独立動作し、Zoom で
  明示 Snap 分解能が変化しない。
- [ ] **TE-07**: Snap Guide が採用候補・時間を説明し、微小移動で
  Snap が不安定に振動しない。
- [ ] **TE-08**: Drag 中の Esc/PointerCancel/Window 破棄では
  Project 内容を変更しない。
- [ ] **TE-09**: Drag 中に他 Window から対象が削除/変更された場合、
  誤った Target/Revision に編集 Command を適用しない。
- [ ] **TE-10**: Piano Roll で Linked Source を編集すると共有 Clip
  に反映するが、Arrangement Selection は不要に変化しない。
- [ ] **TE-11**: Inspector/Signal Flow/Version Diff の
  Explore/Linked Highlight が Editor の Edit Command を上書きしない。
- [ ] **TE-12**: Read-only VCS Snapshot を選択しても Live Project を
  変更しない。
- [ ] **TE-13**: Remote Asset の Drag&Drop 失敗で不完全な
  Clip/Manifest を残さない。
- [ ] **TE-14**: Audio/MIDI 再生中も Gesture Preview/履歴操作は RT
  Audio Callback をブロックしない。

## Non-goals

- Timeline 編集・Selection・Tool・Snap の**製品実装** — 実装 Owner
  は #112（SV-GUI-03 Core Workspace GUI）・#114（SV-GUI-05 実機
  E2E/Gate）および Project/Workspace/GUI の各実装 Issue。本 spec は
  それらが消費する製品契約のみを定義する。
- Ruviform GPUI Driver・共通 Pointer/HitTest/Viewport/Gesture
  primitive の独自再実装 — Ruviform 側の責務（存在する API の範囲で
  再利用）。DAW 固有編集命令を Ruviform へ持ち込まない。
- Pointer/Selection を暗黙に Project/Graph Commit へ変換すること、
  View 状態を Project Undo/Semantic Diff へ混入させること。
- 入力判定の具体 px 閾値・OS 別既定キーバインド・Snap Hysteresis の
  数値・Pointer Device 別細部 — Issue #11 §9 の PoC/実機後決定事項。
  本 spec は契約（優先順・分離・固定・拒否）のみを固定する。
- Take/Comp Editor の UI 詳細（Hit Test・Tool/Modifier・Audition・
  Fade 既定値）— #12 §14/§15 の後続設計対象。本 spec は #12 が継承
  する共通編集 UX（Preview→検証→Atomic Commit→1 Undo）の境界のみ
  を定義する。
- `Cargo.toml`/`Cargo.lock`/`mise.toml`/CI/本番 crate の変更 —
  実装 Owner の独立 PR の責務。
- 実 Window/OS GUI・実機・実 Plugin の受入 — #112/#114/#103 系の
  別受入 Gate。Fake/Headless/Offscreen の成功を実機 Pass と換算
  しない。

## Sources and Relations

- Issue #11 本文（合意 5 項目・状態保存境界・実装境界・受入条件
  §8・後続 PoC 決定事項 §9・#12/#19/#20 連携節）
- `docs/designs/timeline-editing-design.md`（設計正本: Owner 境界・
  Selection/Tool/Snap/Gesture/Cross-View 契約・画面操作 Fixture・
  Decision Log・Scenario/Oracle）
- `docs/test-specs/timeline-editing.md`（Fixture 契約: Feature/Rule/
  Example/Guarantee）
- `evidence/sv11/coverage.md`（受入条件 × 既存被覆の台帳）
- 上流契約 spec: ED-A #38（Selection/Tool/Snap/Gesture/Undo/
  LinkedContext headless PoC）、ED-B #65（media domain move/trim/
  slip/marquee・混在 domain 規則）、WS-A #36（View/Selection/
  LinkGroup/Command/Dock 基盤）、workspace-model #2（GPUI-neutral
  Port・寿命独立・Command routing）、MD-A #42（TakeGroup/Comp/
  Effective Comp 投影・ClipPlacement/Playback 時間契約）、MIDI
  Source Note 編集 #52（Linked Source 共有・明示 Detach）、
  Context Inspector #5（Explore/Linked Highlight の非破壊性）
- 関連 Issue: #2（Workspace/Selection/Command）、#3（共通 UX/Link
  Groups）、#5（Context Inspector）、#6（Automation/Modulation）、
  #7（Semantic Visual Diff）、#8（標準制作ワークフロー）、#9
  （Remote-aware Asset Library）、#10（Track/Lane/Clip/Source/
  Timeline Project Model）、#12（Recording/Take/Comping）、#19
  （可変 PPQN Musical Time）、#20（Time/Transport v0.5）、#112
  （Core GUI）、#114（実機 E2E Gate）
- Ruviform 側参照: ruviform-viz `timeline.rs` primitives、
  `ruviform-media-core` MusicalTime（PPQN 960）、#189（rational
  beat）/#190（transport）/#243（実 GUI Gate）
