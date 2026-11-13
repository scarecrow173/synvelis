---
id: "SPEC-3Vlcu2H26v4t56Rs4ycO7z"
type: "spec"
status: "approved"
title: "Context Inspector — 情報階層・Follow/Explore/Pinned・Selection/Linked Context・Probe 連携の設計定義（synvelis#5）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/5"
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
    - "docs/specs/ed-a-editing-model-selection-tool-snap-transaction-linkedcontext.md"
    - "docs/specs/sf-04-probe-adapter.md"
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
    - "docs/specs/sf-05-slice2-signal-flow-ux0307.md"
    - "docs/specs/pc-c5-01-relation-inspector.md"
    - "docs/specs/ui-sf-control-integration.md"
    - "docs/specs/sv-gui-03-04-selection-inspector-viz.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/context-inspector-design.md"
    - "docs/plans/context-inspector-plan.md"
    - "docs/tasks/context-inspector-task.md"
    - "docs/test-specs/context-inspector.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/workspace-docking-state-architecture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/2"
    - "https://github.com/scarecrow173/synvelis/issues/3"
    - "https://github.com/scarecrow173/synvelis/issues/4"
    - "https://github.com/scarecrow173/synvelis/issues/6"
    - "https://github.com/scarecrow173/synvelis/issues/7"
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/21"
    - "https://github.com/scarecrow173/synvelis/issues/95"
    - "https://github.com/scarecrow173/synvelis/issues/102"
    - "https://github.com/scarecrow173/synvelis/issues/107"
    - "https://github.com/scarecrow173/synvelis/issues/108"
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/ruviform/issues/181"
    - "https://github.com/scarecrow173/ruviform/issues/182"
    - "https://github.com/scarecrow173/ruviform/issues/184"
    - "https://github.com/scarecrow173/ruviform/issues/185"
    - "https://github.com/scarecrow173/ruviform/issues/308"
    - "https://github.com/scarecrow173/ruviform/issues/309"
  defers: []
  deferred-by: []
---

# Context Inspector — 情報階層・Follow/Explore/Pinned・Selection/Linked Context・Probe 連携の設計定義

## What Should Be Built

synvelis#5（設計 Issue）の成果物として、Context Inspector の
**UX・状態・編集・Probe 契約を定義する**。Context Inspector は
属性パネルではなく、制作対象の概要・関連性・設定・観測・診断を
横断して探索し、明示的に編集できる Dockable View である。
基本原則は **共通 Inspector フレームワーク + 対象別 Content
Provider** であり、Arrangement / Mixer / Signal Flow を置き換える
モードではなく、これらの作業文脈を補助する独立 View として
複数開き・別 Window/モニターへ移動できる。

本 spec が固定する契約領域:

1. **情報アーキテクチャ** — 共通 Context Header（対象と種類・所在・
   Follow 元・Pinned 状態・戻る/進む・明示的編集導線）と、TargetKind
   別 Provider からなる情報階層（Overview → Relationships →
   Properties → Observations → Diagnostics）。固定 5 タブではなく、
   内容/重要度に応じた優先表示・折りたたみ。
2. **レイアウト** — Compact（縦積み折りたたみ）と Expanded
   （複数カラム）は同一 Provider/データの配置差のみ。レイアウト変更で
   対象・編集内容・Probe 状態は変わらず、Section 開閉は View
   Instance ごとに独立保持する。
3. **ナビゲーション** — Follow（明示的 FollowSource の有効対象を
   解決表示。link event が運ぶ対象は semantic mapping 後の Track
   粒度であり、他 kind は Explore/明示 inspect で到達）/
   Explore（FollowSource を保持した内部探索履歴）/
   Pinned（対象固定・読み取り専用を意味しない）。Window/Active
   View の Focus 変化や別 Group のイベントでは探索を解除せず、
   FollowSource からの新しい有効な選択イベントでは Explore が
   Follow へ戻る。
4. **複数選択と一括編集** — 単一/同種複数/異種複数/選択なし/Pinned
   の表示規則、共通値と Mixed の区別、発行時対象スナップショット +
   単一 Project Controller による全件 preflight 検証
   （原則 all-or-nothing、無言の部分適用なし）。
5. **標準 Probe と Diagnostics** — Probe Off 既定、明示的
   Monitor/Capture、Session 所有による View 閉鎖・対象変更での
   誤停止なし、同一論理観測点の重複生成なし。Off/Monitor/Capture/
   Unavailable/Stale/Data Loss と実信号の無音を識別し、
   Configured/Observed/Inferred/Unavailable を共通分類とする。
6. **永続化と復元** — View Instance 単位の FollowSource/Pinned/
   Section State を可能な範囲で復元し、Explore 履歴は初期化。
   不在 View/対象には未解決表示と再バインド導線を出し、勝手に
   別対象へ切り替えない。
7. **編集の安全性** — View Context と Editing Command Target を
   混同しない。編集は `ProjectSessionId + TargetId +
   SubTarget/ParameterId/ConnectionId + expected revision` を
   Begin 時に固定した Command として発行し、Project Controller で
   検証・適用する。Inspector の表示・探索だけで Project 編集履歴・
   DSP Bypass・Audio Routing を変更しない。

## Why

Issue #5 は 2026-11-04〜10 の横断レビューで UX・状態管理・操作
契約が合意済みだが、Owner 境界・状態遷移・既存実装との対応・
検証可能な Fixture が文書として固定されていなかった。本 spec と
対応する design/plan/task/test-spec は、その合意を
**実装 Owner（#112 製品 View・#108 Signal Flow 投影・#107 RT
Probe）が消費できる契約と証拠分類**へ落とす。設計レビュー済みの
契約と、実装・実 OS・実 Plugin でしか証明できない Gate を分離し、
headless 成功を GUI Pass として主張しない。

## Users and Value

- DAW ユーザー: Arrangement で選んだ対象の詳細・関連・観測・診断を
  Inspector で辿り、明示操作でだけ編集できる。複数 Inspector を
  別 Window に置いても各々の Follow/Pinned/探索状態が保たれる。
- 後続実装 slice（#112/#108/#107 系）: TargetKind 別 Provider・
  状態機械・Probe lease・一括編集 preflight の契約を本 spec/design
  と test-spec の Fixture 定義から実装・検証できる。
- レビュアー: 「View の調査対象」と「編集 Command の対象」が別物
  であること、観測不能を観測値に見せないことが文書上で検証できる。

## Acceptance Criteria

Issue #5 §9 の受入条件を CI-01〜CI-13 として固定する。各行の
現在の被覆状況は `evidence/sv5/coverage.md` の台帳を正本とし、
本 spec は要求のみを定義する（設計のみの項目は `NotRun`）。

- [ ] **CI-01**: Track/Clip/Plugin/Parameter/Connection/Probe の
  各 TargetKind で共通ヘッダーと必要な Provider 内容を表示し、
  対象が変わっても情報階層が一貫する。
- [ ] **CI-02**: Compact では折りたたみ式、Expanded では複数
  カラム。レイアウト変更で対象/編集内容は変わらず、View 独立の
  Section State を保持する。
- [ ] **CI-03**: Arrangement で Bass を選択 → Inspector Follow →
  Music Bus/Compressor/Sidechain へ Explore → 戻る。元の
  Arrangement 編集選択・表示位置を意図せず変更しない。
- [ ] **CI-04**: Inspector A を Bass に Pinned、Inspector B を
  Drums に Follow。別 Window へ移しても両者の対象と状態を保持する。
- [ ] **CI-05**: Follow 元以外の Window Focus 変化や別 Group の
  イベントでは探索が解除されない。一方、Follow 元からの新選択
  イベントでは Explore が Follow へ戻る。
- [ ] **CI-06**: 複数 Track の共通値と Mixed 値が正確に表示され、
  一括編集時は発行時の対象群に対して全件検証し、失敗時には
  無言の部分適用が起きない。
- [ ] **CI-07**: 編集開始 → 非同期更新中に Follow 対象が変わっても、
  編集が別の Track/Parameter へ誤適用されない。
- [ ] **CI-08**: 2 つの Inspector で同一 Compressor の Probe を
  表示しても、Probe 実行を重複生成しない。片方を閉じても
  ユーザー明示の Monitor を維持する。
- [ ] **CI-09**: Probe Off/Monitor/Capture/Unavailable/Stale/
  Data Loss と、実信号の無音が識別できる。
- [ ] **CI-10**: 同一 Processor の Main/Sidechain/Output の
  利用可能な観測境界を示し、詳細 Capture/Diagnostic View へ
  移動できる。
- [ ] **CI-11**: Pinned 対象削除時には未解決表示。新たな対象へ
  勝手に切り替えない。
- [ ] **CI-12**: 再起動後、可能な FollowSource/Pinned/Section
  State は復元し、Explore 履歴は初期化する。不在 View/対象への
  対応 UI がある。
- [ ] **CI-13**: Probe/Inspector 表示のみで Project 編集履歴、
  DSP Bypass、Audio Routing が変更されない。

## Non-goals

- 製品 View/描画の実装 — **#112（SV-GUI-03）・#108（SF-05）の
  Owner**。本 spec は Driver-neutral な契約のみを定義し、GPUI/OS
  型を ViewSpec や Target 契約に持ち込まない。
- RT Probe/Probe Broker/Capture の実装 — **#107（SF-04）と
  Ruviform #185/#308/#309 の Owner**。表示・参照・制御の意味論のみ
  を定義し、RT 安全性・転送契約を再実装しない。
- Project Command Target を Inspector View Focus から暗黙変更
  すること、Ruviform 共通 View/Binding への DAW 固有 TargetKind
  の持ち込み。
- 実 Window/Focus/IME/DPI/実 Plugin の受入 — **#114（SV-GUI-05）
  および Ruviform #313/#314 の Gate**。Fake/Headless/Offscreen の
  成功を実機 Pass と換算しない。
- 各 Provider の全プロパティ一覧・表示順位・幅ブレークポイント・
  相対変更 UI・部分適用・Probe 永続化方針・Plugin 規格別能力差分
  — Issue #5 §10 の PoC 後決定事項。
- MIDI Note/Event の Provider 実装 — #10/#12 の Model 確定待ちの
  将来 TargetKind（情報階層上の位置のみ予約）。

## Sources and Relations

- Issue #5 本文（合意済み UX・状態管理・操作契約、§1〜§10、
  受入条件、検証方法）
- `docs/designs/context-inspector-design.md`（設計正本:
  Decision Log・状態遷移・Owner 境界・Scenario/Oracle）
- `docs/test-specs/context-inspector.md`（UX Fixture 定義）
- `evidence/sv5/coverage.md`（受入条件 × 既存被覆の台帳）
- 上流設計: `docs/designs/workspace-docking-state-architecture.md`
  （#2/#3 の状態・Focus・Command 契約）、`docs/designs/ed-a-editing-model-design.md`
  （#11 の Selection/Tool/Transaction 契約）
- 関連 Issue: #2（Workspace/Focus/Command）、#3（共通
  インタラクション）、#4（Signal Flow/Probe）、#6（Parameter
  Control）、#7（VCS Visual Diff）、#10（Track/Clip Model）、
  #12（Recording/Take）、#21（v0.6 Asset/Job/Probe）、#95（VCS
  History UI）、#102（UI-C5 relation/gesture）、#107（SF-04 Probe
  Adapter）、#108（SF-05 Inspector 投影）、#112（製品 View）、
  #114（実機 Gate）
- Ruviform 側 Owner: #181/#182（Window/Dock）、#184（型付き
  Port/Graph）、#185（Probe/RT Capture）、#308（Probe 再 Binding）、
  #309（Frame/Clock/Epoch/Latency 整列）
