---
id: "SPEC-034X9D5Y5qj1ea9V1VLeHF"
type: "spec"
status: "approved"
title: "標準 DAW 制作ワークフローと Track/Clip/録音/編集の共通操作体系 (Issue #8)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/8"
    - "https://github.com/scarecrow173/synvelis/issues/1"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/designs/daw-workflow-design.md"
    - "docs/plans/daw-workflow-plan.md"
  depends-on:
    - "docs/specs/product-value-core.md"
    - "docs/specs/workspace-ux-scenarios.md"
    - "docs/specs/workspace-model.md"
    - "docs/specs/project-model.md"
    - "docs/specs/timeline-editing.md"
    - "docs/specs/recording-take-comping.md"
    - "docs/specs/playback-graph.md"
    - "docs/specs/transport-integration.md"
    - "docs/specs/media-render.md"
    - "docs/specs/execution-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/daw-workflow-design.md"
    - "docs/plans/daw-workflow-plan.md"
    - "docs/tasks/daw-workflow-task.md"
    - "docs/test-specs/daw-workflow.md"
    - "docs/specs/context-inspector.md"
    - "docs/specs/automation-modulation.md"
    - "docs/specs/musical-time.md"
    - "docs/specs/project-persistence.md"
    - "docs/specs/ai-native-baseline.md"
    - "docs/specs/command-gateway.md"
    - "docs/specs/svt-product-e2e.md"
    - "docs/specs/alib-01-asset-provider-contract.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/8"
    - "https://github.com/scarecrow173/synvelis/issues/1"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/daw-workflow.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/2"
    - "https://github.com/scarecrow173/synvelis/issues/3"
    - "https://github.com/scarecrow173/synvelis/issues/4"
    - "https://github.com/scarecrow173/synvelis/issues/5"
    - "https://github.com/scarecrow173/synvelis/issues/6"
    - "https://github.com/scarecrow173/synvelis/issues/7"
    - "https://github.com/scarecrow173/synvelis/issues/9"
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/11"
    - "https://github.com/scarecrow173/synvelis/issues/12"
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/18"
    - "https://github.com/scarecrow173/synvelis/issues/19"
    - "https://github.com/scarecrow173/synvelis/issues/20"
    - "https://github.com/scarecrow173/synvelis/issues/21"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/synvelis/issues/44"
    - "https://github.com/scarecrow173/synvelis/issues/45"
    - "https://github.com/scarecrow173/synvelis/issues/112"
    - "https://github.com/scarecrow173/synvelis/issues/113"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/176"
  defers: []
  deferred-by: []
---

# 標準 DAW 制作ワークフローと Track/Clip/録音/編集の共通操作体系 (Issue #8)

## What Should Be Built

Issue #8 の成果物として、Synvelis の **標準的な DAW 制作
ワークフローの基本方針と、Track/Clip/録音/編集に共通する操作
体系の UX 仕様**を `docs/` の canonical 文書として確定する。
対象は「何を・なぜ・誰のために・何をもって合格とするか」の
要件定義であり、UX 規範本文（状態図・操作表・Expected
Oracle・Decision Log）は `docs/designs/daw-workflow-design.md`
が正本となる。

本 spec が固定する契約領域:

1. **Gate A 代表ワークフロー** — Issue #8 の 3 代表ワークフロー
   （MIDI 中心 / Audio 中心 / Mix・調査）と、Gate A 必須最小
   タスク（Audio/MIDI Track 作成→Import/Record→Clip/Note
   非破壊編集→Mixer/Transport→保存/Reload→Offline WAV/
   独立 Reader 検証）を、手順・Expected Oracle・検証レーン
   つきで定義する。AI/Signal Flow 診断/VCS/Remote Browser は
   いずれも OFF・不通でも完結できなければならない。
2. **共通操作体系** — Track/Clip/MIDI Note/Take と編集
   Command/Undo が一貫した対象へ適用されるための、Selection/
   Command Target/入力境界のワークフローレベル契約
   （詳細な Tool/HitTest/Snap 意味論は #11 が正本）。
3. **Keyboard/Gesture/Tool UX** — 3 ワークフローの各工程を
   キーボード・ポインタ gesture・Tool 切替で遂行するための
   最小操作集合と安全契約（入力コンテキスト・Gesture
   Preview→Atomic Commit→1 Undo・誤発火防止）。
4. **分離契約の明文化** — Window/Dock/Focus の移動が
   Recording/Transport/Project を変更しないこと、Mixer の
   Routing 表示と Signal Flow の意味モデルの一致、通常
   Channel Meter と明示的 Probe/Capture の区別、Remote
   Source 不通時のローカル制作継続、VCS 非依存の DAW 動作。
5. **証拠の誠実さ** — `evidence/sv8/` に受入項目→仕様節→
   既存設計文書・実装 Issue の対応台帳を作成し、未検証の
   実機/実 GUI 項目を `NotRun`/`Deferred`/`Unsupported` と
   して区別する（POLICIES.md §2/§3）。

本 spec は**文書成果物の要件定義**であり、製品コードの実装・
実 OS/実 GUI/実 Device/実 Plugin の受入を主張しない。実装は
#10（Project Model）・#11（Timeline Editing）・#12
（Recording/Take/Comping）・#9（Asset Library/Browser）・
#112（Core Workspace GUI）・#114（製品 E2E）等の各 Owner
Issue に委譲する。

## Why Now

- Issue #8 は [製品価値 #1](https://github.com/scarecrow173/
  synvelis/issues/1) の Gate A「Core DAW Creation」の UX 側
  正本候補であり、AM-1（基本制作）の Owner として
  #8・#23・#114 が記録されている。#10/#11/#12 の設計文書
  チェーンは成立したが、**それらを「一曲を保存・再開・
  書き出せる」一連の制作ワークフローとして束ねる操作体系の
  正本**が未確定だった。
- #3（Workspace UX）・#11（Timeline Editing）・#12
  （Recording）が個別契約を固定した一方、「録音→編集→
  Export の代表 Task を診断/グラフなしで完結できる」という
  **製品ワークフローとしての受入 Oracle**がないと、#112/#114
  の GUI 実装と受入が「部分的に動く」状態で進行するリスクが
  ある。
- Issue #8 の 2026-11-10 実行判断により、Worker が設計
  ドラフト・契約/状態図・Case 表・review-protocol・PR まで
  担当する。新規の製品トレードオフ・契約破壊・定量 SLO・
  OS サポート宣言のみ「提案/決定待ち」として分離する。

## Users and Value

- **DAW ユーザー**: Audio/MIDI の録音・編集・Mix・保存・
  Export を、診断画面やグラフを開かずに一貫した操作で
  完結できる。
- **実装 Owner（#10/#11/#12/#9/#18〜#21/#112/#114 等）**:
  自分の契約がワークフローのどの工程・どの受入項目を支える
  かを一意に参照できる。
- **Coordinator/レビュアー**: 「設計合意」「headless 証拠」
  「実 GUI/実機受入」を区別し、`NotRun`/`Unsupported` を
  `Pass` にしない判定枠組みを得る。
- **後続 Gate（#23 Execution Plan）**: Gate A の各 node の
  Acceptance 判定に使う代表シナリオと Expected Oracle を
  供給する。

## Scope

### In Scope

- Issue #8 の基本原則 5 項目・制作領域と View の対応・
  基本 UX/Command 安全性・P0/P1/P2 ロードマップ・代表
  ワークフロー 3 件・受入条件 7 項目の正本化。
- Gate A 必須最小タスクの工程分解と、各工程の操作主体
  （View/Command/Owner）・期待結果・検証レーンの対応付け。
- Keyboard/Gesture/Tool のワークフローレベル UX 契約
  （最小キー操作集合・Tool 切替・Gesture 安全性）。
- `evidence/sv8/` への受入項目→仕様節→既存 docs/実装
  Issue の追跡台帳と検証記録。
- spec → design → plan → task → test-spec → impl(ir/exp)
  の文書チェーン。

### Out of Scope

- `crates/` を含む一切のコード実装・実 Window/実 OS 入力・
  実 Plugin/実 Device の検証（#112/#114/#176 lane）。
- Track/Clip/Timeline 統合モデルの詳細（#10）、Editing
  Tool/Selection/Snap/Gesture の詳細意味論（#11）、
  Recording/Take/Comping の詳細契約（#12）、Asset
  Library/Browser の詳細（#9）— 本 spec はそれらへの
  参照とワークフロー上の位置づけのみを定義する。
- Signal Flow 診断・Inspector・Version Diff・AI Gateway・
  VCS の個別仕様 — Gate B/C/AI の Owner Issue（#4/#5/#6/
  #7/#44/#45 等）の責務。本 spec は「それらが OFF/未導入でも
  Gate A が成立する」境界のみを規定する。
- Ruviform 共通 Transport/Audio/Component の製品専用への
  書き換え、`Cargo.toml`/`Cargo.lock`/`mise.toml`/CI の
  変更。
- 新しい製品トレードオフ・契約破壊・定量 SLO・OS サポート
  宣言の最終決定（「提案/決定待ち」として分離）。

## Requirements

- **WF-R-01 Gate A ワークフロー定義**: Issue #8 の 3 代表
  ワークフロー（MIDI 中心 / Audio 中心 / Mix・調査）を、
  工程・操作 View・Command 系・Expected Oracle・検証
  レーンつきで定義する。Gate A 必須最小タスク
  （Track 作成→Import/Record→非破壊編集→Mixer/Transport→
  Save/Reload→Offline WAV Export→独立 Reader 検証）を全
  ワークフローの共通 backbone として明示する。
- **WF-R-02 診断非依存**: 3 ワークフローの制作 spine は
  Signal Flow 診断・グラフ・AI・VCS・Remote Browser を
  要求しない。Understand/Refine への「深掘り」は文脈を
  保持した任意の detour であり、未完成・OFF でも制作が
  阻害されない。
- **WF-R-03 Window/Dock/Focus 独立性**: Window/Dock 移動・
  View 開閉・Focus 変更は Navigate/Arrange 分類に留まり、
  Recording Session・Transport・Project Revision・編集
  対象を勝手に変更しない（#3 WUX・#12 AC-36 と整合）。
- **WF-R-04 共通操作体系**: Track/Clip/MIDI Note/Take の
  選択・編集 Command・Undo が、Gesture 開始時に固定した
  対象 ID/Revision へ一貫して適用される。Selection/
  Command Target/入力境界の分離を #3/#11 契約に従って
  ワークフローレベルで規定する。
- **WF-R-05 非破壊編集・録音回復**: Audio Clip/Take の
  非破壊編集（Source/Asset/Clip/Take の分離）と、録音
  ファイルの Staging→Journal→Finalize→Commit・crash
  recovery 契約を、ワークフローの失敗・中断ケースを含めて
  検証可能な形で規定する（#10 PM-R・#12 AC 群に整合）。
- **WF-R-06 Mixer/Routing/Meter 分離**: Mixer の Routing
  表示は Project の Semantic Routing Model（#4/#104）と
  一致し、通常 Channel Meter（日常のレベル監視）と明示的
  Probe/Capture（診断）を同一視しない。Signal Flow が
  OFF/未導入でも Mixer 操作と Meter は成立する。
- **WF-R-07 Remote Source 不通耐性**: Browser の Remote
  Source が不通・未接続でも、ローカル Import/録音/編集/
  保存/再生/Export は成立する。Missing Asset/Plugin の
  状態を誤魔化さず、取得済み Asset の offline 再生を
  保証する（#10 PM-R-13/14・#9 ALIB 契約に整合）。
- **WF-R-08 VCS 非依存**: Project Undo/Save/Load/Export/
  Transport は VCS Commit/History/Submit に依存しない。
  Semantic Diff・History View は Gate B/C の任意機能で
  あり、VCS 未導入でも DAW として動作する。
- **WF-R-09 Keyboard/Gesture/Tool UX**: 3 ワークフローの
  代表 Task をキーボード・ポインタ gesture・Tool で遂行
  するための最小操作集合を定義し、入力コンテキスト
  （View/TextInput/IME/Plugin GUI/Modal）による誤発火
  防止・Gesture Preview→Atomic Commit→1 Undo・Esc
  階層的取消を #3/#11 契約に整合させて規定する。
- **WF-R-10 証拠と境界**: `evidence/sv8/` に受入項目→
  仕様節→既存設計文書・実装 Issue の台帳を作成し、
  実装・実機・実 GUI の未検証項目を `NotRun`/
  `Deferred`/`Unsupported` として区別する。設計評価を
  実機 Pass に換算しない。

## Acceptance Criteria

Issue #8 の受入条件 7 項目を WF-AC として転記・正規化する。
本 spec の受入は「UX 仕様文書が当該項目を検証可能な形で
規定し、検証レーンと Oracle を明示する」ことであり、実
OS/実 GUI での実測 Pass を意味しない。

- [ ] **WF-AC-01**: 3 ワークフロー（MIDI 中心/Audio 中心/
  Mix・調査）を診断/グラフを強制せずに完結できる手順と
  Expected Oracle が定義されている（design WF-SCN-
  MIDI/AUDIO/MIX + Gate A backbone）。
- [ ] **WF-AC-02**: Window/Dock 移動で Recording/Transport/
  Project を勝手に変更しない契約が、操作分類・状態遷移・
  失敗時挙動として規定されている。
- [ ] **WF-AC-03**: Track/Clip/MIDI Note と編集 Command/
  Undo が一貫して対象へ適用される契約（対象 ID/Revision
  固定・競合拒否・1 gesture=1 undo）が規定されている。
- [ ] **WF-AC-04**: Audio Clip/Take の非破壊編集と録音
  ファイル回復（Staging/Journal/Finalize/Commit・crash
  recovery・冪等 retry）の契約が検証可能な形で規定されて
  いる。
- [ ] **WF-AC-05**: Mixer と Signal Flow の Routing 状態
  一致、通常 Meter と Probe の区別が規定されている。
- [ ] **WF-AC-06**: Browser の Remote Source 不通でも
  ローカル制作・保存・再生・書き出しが可能である契約と、
  Missing Asset の正直な表示が規定されている。
- [ ] **WF-AC-07**: VCS なしでも DAW として動作し、
  History/VCS 操作に依存しない契約が規定されている。
- [ ] **WF-AC-08**: Keyboard/Gesture/Tool UX の最小操作
  集合と安全契約が規定されている。
- [ ] **WF-AC-09**: spec → design → plan → task →
  test-spec → impl(ir/exp) の文書チェーンが `docs/` に
  存在し、`evidence/sv8/` に受入項目→仕様節→既存
  docs/実装 Issue の追跡台帳と検証記録がある。
- [ ] **WF-AC-10**: 実装・実機・実 GUI で未検証の項目は
  `NotRun`/`Deferred`/`Unsupported` として記録され、
  設計評価を実機 Pass に換算していない（POLICIES.md
  §2/§3 準拠）。

## Deferred Design Concerns

- Keyboard の具体的キー割当・OS 別既定バインド・コンテキスト
  メニュー完全網羅 | #11 §9・#112 実装範囲 | 実装/実機 PoC で
  確定 | キー操作の予測可能性が OS/実装依存でぶれる。
- Smart/Dedicated Tool の HitTest 優先順・Snap 閾値・
  ジェスチャ数値 | #11（ED-A/ED-B headless 契約が正本）| #11
  詳細確定後 | 本 spec はワークフロー上の Tool 運用契約のみ
  規定し、数値を固定しない。
- Take/Comp Editor の UI 詳細（Audition・Comp Paint・
  Collapsed/Expanded Lane）| #12 §14/§15 | #12 後続設計 | 本
  spec はワークフロー上の録音→Comp 遷移契約のみ規定。
- Remote Asset Browser の検索/Preview/選択的 Import 詳細 |
  #9（Asset Library/Browser）| #9 確定後 | 本 spec は
  「Remote 不通時のローカル制作継続」境界のみ規定。
- Mix/調査ワークフローの Signal Flow/Inspector/Version
  Diff 各工程の詳細 | #4/#5/#7/#108/#109 | Gate B/C Owner |
  本 spec は detour の存在と文脈復帰契約のみ規定。
- 実 OS Window/Focus/IME/DPI/実 Plugin GUI/実 Device での
  検証 | #114（GUI-L2）・#176（EXT-L3）| 実機 lane | 設計
  評価を実機 Pass に換算しない。
- 新規製品トレードオフ・契約破壊・定量 SLO・OS サポート宣言 |
  Issue 2026-11-10 実行判断 | 人間決定待ち | 承認を捏造しない。

## 制作領域と View の対応

Issue #8 の制作領域を、主担当 View・Command 系・Owner Issue
へ対応付ける。1 つの View が複数領域を担当し、1 つの領域が
複数 View にまたがることを許容する（#3 Workspace Model と
整合）。

| 制作領域 | 主担当 View（代表） | 代表 Command 系 | Owner Issue |
| --- | --- | --- | --- |
| Project Setup | Project Browser / New Project Dialog | Project New/Open/Save/Save As | #10/#17/#112 |
| Arrangement | Arrangement Timeline | Clip move/trim/split、Track 並べ替え | #11/#112 |
| MIDI | Piano Roll / Step Sequencer | Note 入力/編集/量子化 | #11/#20 |
| Audio | Audio Track Lane / Take Comp Editor | Clip/Take 選択・trim・fade・Comp | #12/#11 |
| Mixing | Mixer / Channel Strip | Gain/Pan/Mute/Solo/Routing/Send | #18/#112 |
| Automation | Automation Lane（Arrangement/MIDI 内） | Point 編集・描画・削除 | #6/#20 |
| Export | Export Dialog / Render View | Offline Render・Format 選択・検証 | #21 |
| Assets | Asset Browser / Clip Source | Import・検索・Missing 解決 | #9/ALIB |

全領域で、編集 Command は §design 7/8 の共通契約
（対象 ID+Revision 固定・1 gesture=1 undo・入力境界）に
従う。Arrangement/MIDI/Audio の編集は #11、Audio の
録音/Take/Comp は #12、Mixer/Routing は #18、Automation は
#6、Asset 解決は #9 が個別契約の正本である。

## ロードマップ対応（Issue #8 P0/P1/P2）

Issue #8 の P0/P1/P2 を Gate 対応として固定する。詳細な
個別契約は各 Owner 文書に委譲し、本 spec はワークフロー上の
位置づけのみを規定する。

| 優先度 | 領域 | Gate | 本 spec/design での扱い |
| --- | --- | --- | --- |
| P0 | Track/Clip/Note の直接編集・Command/Undo 一貫性・Window/Dock 独立性 | Gate A | WF-AC-02/03、design §7/§8 で規定。実装は #10/#11/#112 |
| P1 | Recording/Take/Comp・非破壊編集・録音回復・Import/Export・Save/Reload | Gate A | WF-AC-04、design §5/§6.2/§9 で規定。実装は #12/#17/#21 |
| P2 | Mixer 詳細・Automation・Signal Flow 診断・Inspector・Version Diff | Gate B/C | design §6.3 の任意 detour として位置づけ。実装は #4/#5/#6/#7/#18 |

P0/P1 が Gate A の完了条件であり、P2 は Gate A を **阻害
しない** 任意深掘りとして扱う。これは Issue #8 の「制作が
第一」「必要なときに深掘り」の原則と一致する。

## Sources and Relations

- Issue #8 本文（基本原則 5 項目・制作領域と View・基本
  UX/Command 安全性・P0/P1/P2・代表ワークフロー・受入
  条件・2026-11-10 実行判断・Gate A/B/C 接続合意
  2026-11-04）。
- `docs/specs/product-value-core.md`（Gate A/B/C/AI 定義・
  AM-1 基本制作の Owner 対応・証拠状態区分）。
- `docs/specs/workspace-ux-scenarios.md` /
  `docs/designs/workspace-ux-scenarios-design.md`（#3 UX
  正本: Navigate/Inspect/Edit/Arrange 分類・入力境界・
  WUX-SCN-A〜F・D1〜D24）。
- `docs/specs/workspace-model.md`（#2 GPUI-neutral Port・
  View/Window 寿命独立・Command routing）。
- `docs/specs/project-model.md`（#10 PM-R/PM-AC: Track/
  Lane/Clip/Source/Take/Time・非破壊編集・stable ID・
  単一 writer・Save/Reload）。
- `docs/specs/timeline-editing.md`（#11 TE-01〜14:
  Selection/Tool/Snap/Gesture/Undo/Cross-View）。
- `docs/specs/recording-take-comping.md`（#12 AC-01〜69:
  Session/Pass/Stream/Segment/Take/Comp・Frame 分離・
  RT/Writer/Recovery・Monitor/Latency）。
- `docs/specs/playback-graph.md`（#18 EX-001〜020:
  Playback/Mixer/Generic Execution Graph）。
- `docs/specs/transport-integration.md`（#20 TT-001〜030:
  Time/Transport/MIDI/Automation/Plugin）。
- `docs/specs/media-render.md`（#21: Media/Import/Export/
  Offline Render/UI/Diagnostics）。
- `docs/specs/musical-time.md`（#19 可変 PPQN Musical Time）。
- `docs/specs/execution-plan.md`（#23 EP-R/EP-AC: 横断 DAG・
  OS-L1/GUI-L2/EXT-L3・証拠台帳）。
- `docs/specs/context-inspector.md`（#5）・
  `docs/specs/automation-modulation.md`（#6）・
  `docs/specs/project-persistence.md`（#17）・
  `docs/specs/ai-native-baseline.md`（#44）・
  `docs/specs/command-gateway.md`（#45）・
  `docs/specs/svt-product-e2e.md`（#114）・
  `docs/specs/alib-01-asset-provider-contract.md`（#9 系
  ALIB）。
- `evidence/sv8/`（本タスクの追跡台帳・検証記録）。
