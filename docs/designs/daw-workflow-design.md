---
id: "DESIGN-034X9D6y9KpvdrfOuRveV6"
type: "design"
status: "approved"
title: "標準 DAW 制作ワークフロー UX 正本 — Gate A 代表 Task・Keyboard/Gesture/Tool 操作体系 (Issue #8)"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/8"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/daw-workflow.md"
  implemented-by: []
  depends-on:
    - "docs/designs/workspace-ux-scenarios-design.md"
    - "docs/designs/project-model-design.md"
    - "docs/designs/timeline-editing-design.md"
    - "docs/designs/recording-take-comping-design.md"
    - "docs/designs/playback-graph-design.md"
    - "docs/designs/transport-integration-design.md"
    - "docs/designs/media-render-design.md"
    - "docs/designs/workspace-docking-state-architecture.md"
    - "docs/designs/workspace-model-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/daw-workflow-plan.md"
    - "docs/tasks/daw-workflow-task.md"
    - "docs/test-specs/daw-workflow.md"
    - "docs/specs/daw-workflow.md"
    - "docs/specs/workspace-ux-scenarios.md"
    - "docs/specs/svt-product-e2e.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# 標準 DAW 制作ワークフロー UX 正本 — Gate A 代表 Task・Keyboard/Gesture/Tool 操作体系 (Issue #8)

## 1. Overview

本書は Issue #8 の design 正本であり、Synvelis の標準 DAW
制作ワークフローを **UX 操作契約**として定義する。Gate A
「Core DAW Creation」の代表 Task（MIDI 中心 / Audio 中心 /
Mix・調査の 3 ワークフロー）が、AI・Signal Flow 診断・
VCS・Remote Browser を OFF/不通にした状態で完結することを
保証する操作体系・状態図・Expected Oracle・決定ログを提供する。

上位仕様は `docs/specs/daw-workflow.md`（WF-R-01〜10・
WF-AC-01〜10）。個別契約の正本は各 Owner 文書に残る:

| 領域 | 正本 |
| --- | --- |
| Workspace/Window/Command 分類 | #3 `workspace-ux-scenarios` |
| Track/Clip/Source/Take/Time モデル | #10 `project-model` |
| Selection/Tool/Snap/Gesture/Undo | #11 `timeline-editing` |
| Recording Session/Take/Comp/Recovery | #12 `recording-take-comping` |
| Playback/Mixer/Execution Graph | #18 `playback-graph` |
| Time/Transport/MIDI/Automation/Plugin | #20 `transport-integration` |
| Import/Export/Offline Render | #21 `media-render` |
| Project Save/Load/Undo 永続化 | #17 `project-persistence` |
| Signal Flow 診断 | #4/#108（Gate B） |
| Inspector/Context | #5（Gate B） |
| Version History/VCS | #7/#95（Gate B/C） |
| AI Gateway | #44/#45（Gate C/AI） |
| Asset Library/Browser | #9/ALIB（#9 実装 Issue） |

本書はこれらを**再定義しない**。ワークフローの各工程が
どの View/Command/Owner を経由し、どの Oracle で合否を
判定するかの対応表と、ワークフロー横断の安全契約（入力
境界・Gesture 安全性・Window/Dock 独立性・非依存境界）を
定義する。

## 2. Context and Scope

### 2.1 解く問題

- #10/#11/#12/#18/#20/#21 の設計文書は成立したが、
  「録音→編集→保存→書き出し」を **一連の UX として**
  束ねる正本がなかった。
- Issue #8 の 3 代表ワークフローは Gate A の受入単位だが、
  各工程の操作・失敗時挙動・Oracle が文書化されていない。
- 「診断/グラフを強制しない」「Remote 不通でもローカル制作
  可能」「VCS なしで動作」等の **負の依存契約**を明文化する
  場がなかった。
- Keyboard/Gesture/Tool の最小操作集合が各工程で一貫して
  いるかを検証する枠組みがなかった。

### 2.2 スコープ

- 3 代表ワークフロー（WF-SCN-MIDI / WF-SCN-AUDIO /
  WF-SCN-MIX）の工程定義・操作・Oracle・検証レーン。
- Gate A 必須最小タスク（Gate A backbone）の工程分解。
- Keyboard/Gesture/Tool のワークフローレベル UX 契約
  （§7）。
- Window/Dock/Focus 独立性・共通操作体系・非依存境界
  （§8〜§10）。
- Recording ワークフロー状態と失敗/中断の扱い（§9）。
- Mixer/Routing/Meter と Probe の分離（§6.3）。
- Remote Source 不通・VCS 非依存の境界（§10）。

### 2.3 スコープ外

- Track/Clip/Timeline モデル詳細（#10）、Tool/HitTest/Snap
  数値（#11）、Recording/Comping 詳細（#12）、Asset
  Browser 詳細（#9）。
- Signal Flow/Inspector/Version Diff/AI の個別仕様。
- キー割当の最終確定（実装は #112、確定は Deferred）。
- 実 OS/実 GUI/実 Device/実 Plugin の検証（#114/#176）。

## 3. Design Principles（Issue #8 基本原則の固定化）

Issue #8 の合意済み基本原則を本設計の不変条件として採用する。

| ID | 原則 | 本設計での担保 |
| --- | --- | --- |
| P-1 | DAW 制作が第一 | 全ワークフローの spine は制作 Task のみで完結。診断/AI/VCS は detour |
| P-2 | 直接操作・一貫した編集 | 全工程で同一の Selection/Target/Gesture 契約（§7/§8） |
| P-3 | 非破壊編集を基本 | Source/Asset/Clip/Take 分離、編集は derived projection（§8.3） |
| P-4 | 複数 View と単一 Project 正本 | Window/Dock は表示のみ、正本は Project Controller（§8.1） |
| P-5 | 必要なときに深掘り | Understand/Refine への遷移は文脈保持の任意 detour（§6.3） |

## 4. Component / Owner Map

| Component | Owner Issue | 本設計での役割 |
| --- | --- | --- |
| Project Controller | #10/#17 | 唯一の Project 正本。Revision/Undo/Save/Load |
| Timeline Editing | #11 | Selection/Tool/Gesture/Undo の意味論 |
| Recording | #12 | Session/Take/Comp/Recovery |
| Playback/Mixer | #18 | Transport 実行・Channel Meter・Routing 表示 |
| Transport/Time | #20/#19 | 再生位置・録音位置・Musical Time |
| Media/Import/Export | #21 | Import/Offline Render/独立検証ファイル出力 |
| Workspace UX | #3 | View/Window/入力境界/Command 分類 |
| Asset Library/Browser | #9 | Remote/Local Asset 解決（不通時は degrade） |
| Signal Flow | #4 | 診断 detour（Gate B、OFF 可） |
| Inspector | #5 | Context detour（Gate B） |
| VCS/History | #7/#95 | 任意機能（Gate B/C、非依存） |

## 5. Gate A Backbone（全ワークフロー共通の必須最小タスク）

Issue #8 が定義する Gate A 必須最小タスクを、工程 ID
`GA-01`〜`GA-08` として固定する。全 3 ワークフローはこの
backbone を含み、工程の順序・Oracle は共通とする。

| 工程 | 操作 | 期待結果 | Oracle（独立検証） | 主 Owner |
| --- | --- | --- | --- | --- |
| GA-01 Track 作成 | UI/Menu または Command で Audio Track / MIDI Track を作成 | Track が Project に追加され Timeline/Mixer に反映 | Project Revision 増分・Track 一覧・Mixer Strip 出現 | #10/#112 |
| GA-02 ソース取得 | Local Import（Audio/MIDI）または録音で素材を得る | Source/Asset が登録され Clip/Take として配置可能 | Asset 一覧・ファイル存在・Import 進捗の完了 | #9/#12/#21 |
| GA-03 非破壊編集 | Clip trim/move、MIDI Note 編集、Take/Comp 選択 | Source/Asset は不変、Clip/Note の derived 状態のみ変化 | 元ファイル hash 不変・Clip 範囲・Undo で復元 | #10/#11/#12 |
| GA-04 Mixer/Transport | Transport で再生/停止/Seek、Mixer で Gain/Pan/Routing 確認 | 再生位置・Meter・Routing が Project と一致 | 再生位置表示・Meter 応答・Routing 表示一致 | #18/#20 |
| GA-05 保存 | Project を Save | Project ファイルが disk に永続化 | ファイル存在・サイズ>0・Save 完了通知 | #17/#21 |
| GA-06 Reload | Project を閉じて開き直す | 全 Track/Clip/Note/Mix 設定が復元 | 再読み込み後の状態比較（Revision 内容等価） | #17 |
| GA-07 Offline Export | Offline WAV Render を実行 | WAV ファイルが出力される | ファイル存在・WAV ヘッダ・長さ/サンプルレート一致 | #21 |
| GA-08 独立検証 | 別 Reader（WAV パーサ/波形表示等）で出力を検証 | 独立 Reader が破損なく読める | 独立 Reader の decode 成功・PCM 内容の sanity | #21/#114 |

**不変条件**: GA-01〜GA-08 は Signal Flow 診断・グラフ・
AI Gateway・VCS・Remote Browser を **要求しない**。Remote
Source が不通でも Local Import/録音で GA-02 を満たせる。

## 6. 代表ワークフロー（WF-SCN）

### 6.1 WF-SCN-MIDI（MIDI 中心）

Issue #8 の「MIDI 中心の制作例」を Gate A 手順に展開する。

| Step | 工程 | 操作（代表） | 期待結果 | Oracle |
| --- | --- | --- | --- | --- |
| M-01 | Project 作成 | New Project / テンプレート選択 | 空 Project が開き Timeline/Mixer が初期化 | Project 開・Undo 履歴初期化 |
| M-02 | GA-01 | MIDI Track 作成 | Track 追加・Instrument Slot 確認 | Track 一覧・Mixer Strip |
| M-03 | GA-02 | MIDI Clip 作成→Note 入力（Piano Roll または Step） | Note が Clip に配置 | Note 数・Pitch/Velocity・Clip 範囲 |
| M-04 | GA-03 | Note の移動/長さ変更/削除、Clip の trim | Source 不変・Note 配置のみ変化 | Undo で復元・Clip 範囲整合 |
| M-05 | Plugin 選択 | Track の Instrument/FX に Plugin を割当 | Plugin が Load され音が出る | Plugin 一覧・音声出力（headless は render oracle） |
| M-06 | GA-04 | Transport 再生・Mixer で音量/Pan 調整 | 再生・Meter 応答・Routing 一致 | 再生位置・Meter・Routing 表示 |
| M-07 | GA-05/06 | Save→Reload | 状態復元 | 復元後の等価性 |
| M-08 | GA-07/08 | Offline WAV Export→独立検証 | WAV 出力・独立 decode | WAV ヘッダ・PCM sanity |

**失敗/中断ケース**:
- Plugin Load 失敗 → Missing Plugin 状態を正直に表示、
  Track/Clip は保持、音は無音または fallback。制作は継続
  可能（別 Plugin に差替または無音で Export）。
- Note 編集中に Window 移動 → 編集は継続、Project は
  不変（§8.1）。
- Export 中に Source 不在 → Missing Asset を表示し Export は
  失敗または無音区間を明示（偽装しない）。

**検証レーン**: OS-L1（headless: Project/Note/Undo/Export
の論理検証）+ GUI-L2（実 Window での Piano Roll/Transport/
Mixer 操作）+ EXT-L3（実 Plugin/実音源）。Gate A の文書受入
は OS-L1 証拠 + GUI-L2 計画で足り、実機 Pass は #114 以降。

### 6.2 WF-SCN-AUDIO（Audio 中心）

Issue #8 の「Audio 中心の制作例」を展開する。

| Step | 工程 | 操作（代表） | 期待結果 | Oracle |
| --- | --- | --- | --- | --- |
| A-01 | Project 作成 | New Project | 空 Project 初期化 | M-01 と同じ |
| A-02 | GA-01 | Audio Track 作成 | Track 追加・Input 設定可能 | Track 一覧・Input 選択肢 |
| A-03 | GA-02 | Local Import（WAV/MP3 等）→Clip 配置 | Asset 登録・Clip 配置 | Asset hash・Clip 範囲・波形表示 |
| A-04 | （録音分岐） | 録音で Take 取得（#12 Session） | Take が Staging→Finalize→Commit | Take 一覧・録音ファイル存在・duration |
| A-05 | GA-03 | Clip trim/move/gain/fade、Take 選択/Comp | Source/Asset 不変・Clip/Comp のみ変化 | 元ファイル hash 不変・Undo 復元 |
| A-06 | GA-04 | Transport 再生・Mixer 調整 | 再生・Meter・Routing 一致 | 再生位置・Meter・Routing |
| A-07 | GA-05/06 | Save→Reload | 復元 | 等価性 |
| A-08 | GA-07/08 | Offline WAV Export→独立検証 | WAV 出力 | WAV ヘッダ・PCM sanity |

**録音分岐の扱い**: Recording/Take/Comping の詳細契約は
#12 が正本。本ワークフローでは「録音→Take 取得→Comp
選択→非破壊編集」の流れが Gate A で要求される最小形を
定義し、Take Editor UI の詳細は #12 の Deferred に委譲
する。録音は Local Input が利用可能な場合のみ。

**失敗/中断ケース**:
- 録音中 crash → Staging 領域の journal から recovery、
  完成済み Take は保全、中断 Take は discard（#12 AC-36
  系と整合）。
- 録音中に Window/Dock 移動 → Recording Session は継続、
  Transport も継続、Project は不変（WF-AC-02）。
- Import 中に Remote Source 不通 → Local file のみで
  ワークフロー継続、Remote asset は Missing 表示
  （WF-AC-06）。

### 6.3 WF-SCN-MIX（Mix・調査）

Issue #8 の「Mix・調査の例」を展開する。Gate A では
「調査」部分は **任意 detour** であり、spine は Mix→
Save→Export で完結する。

| Step | 工程 | 操作（代表） | 期待結果 | Oracle | Gate A 必須? |
| --- | --- | --- | --- | --- | --- |
| X-01 | Project を開く | 既存 Project Load | 全状態復元 | 復元等価性 | 必須 |
| X-02 | GA-04 | Mixer で各 Track の Gain/Pan/Routing 調整 | Routing が Project に反映 | Routing 表示・Meter | 必須 |
| X-03 | （detour） | Signal Flow で Routing 可視化・診断 | 診断結果が表示 | 診断パネル出力 | 任意（Gate B） |
| X-04 | （detour） | Inspector で Parameter 詳細・Context 確認 | Context が表示 | Inspector 出力 | 任意（Gate B） |
| X-05 | （detour） | Version Diff で変更履歴比較 | Diff が表示 | Diff 出力 | 任意（Gate C） |
| X-06 | GA-05/06 | Save→Reload | 復元 | 等価性 | 必須 |
| X-07 | GA-07/08 | Offline WAV Export→独立検証 | WAV 出力 | WAV 検証 | 必須 |

**detour 契約**: X-03〜X-05 は Understand/Refine への深掘り
であり、文脈（選択中 Track/Clip・再生位置・Mixer 状態）を
保持したまま往復できること。detour 未実施・機能 OFF でも
X-01→X-02→X-06→X-07 は完結する。detour 中に Signal Flow
等が変更した Routing は Project 正本に反映され、Mixer 表示
と一致する（WF-AC-05）。

**Mixer/Routing/Meter 分離（WF-AC-05）**:

- Mixer Strip の **Channel Meter** は通常の再生レベル監視
  であり、Signal Flow の Probe/Capture とは別物。
- Routing 表示（Send/Buss/Sidechain 割当）は Project の
  Semantic Routing Model（#4/#104）と一致しなければなら
  ない。Signal Flow が OFF でも Mixer の Routing 編集・
  表示は成立する。
- Probe/Capture は明示的な診断操作であり、通常 Meter と
  混同しない（視覚的区別・ラベル・色分けは #112 実装範囲、
  契約としては「Meter は監視、Probe は診断」の分離を固定）。

## 7. Keyboard/Gesture/Tool UX 契約

### 7.1 基本原則

- **K-1**: 全ての編集 Command は、Gesture/Key 押下時に
  確定した **対象 ID + Revision** に適用される。対象が
  変化した場合は Conflict として拒否し、Undo 可能な単位で
  確定する（#3/#11 と整合）。
- **K-2**: Gesture は Preview→Commit の 2 相。Preview 中は
  Project を変更せず、Commit 時に Atomic に 1 Undo 単位と
  して確定する。Esc は階層的に Cancel（Gesture→Tool→
  Dialog の順）。
- **K-3**: Keyboard/Tool は入力コンテキストでゲートされる。
  TextInput/IME/Plugin GUI/Modal Dialog 中は編集キーを
  Project Command に流さない（#3 §9 入力境界と整合）。
- **K-4**: Tool は View ごとに独立した状態を持ち、Window/
  Dock 移動で変更されない。Smart Tool（デフォルト）と
  Dedicated Tool（Select/Draw/Split/Stretch 等）の切替は
  キーボードショートカットまたは Tool Bar で行う。
- **K-5**: 同一の物理操作（例: ドラッグで Clip 移動）が
  View によらず同じ Command に解決される（Direct
  Manipulation の一貫性）。

### 7.2 最小キーボード操作集合（ワークフロー遂行に必要な範囲）

以下は **ワークフロー遂行に最低限必要な操作**の契約で
あり、具体的キー割当は #112 で確定する（Deferred）。
「操作→Command」の対応を固定し、キー自体は実装依存とする。

| 操作 | 解決先 Command | 入力コンテキスト条件 | 備考 |
| --- | --- | --- | --- |
| 再生/停止 | Transport Play/Pause | 編集 View フォーカス時 | Space が一般的（実装依存） |
| 停止（位置先頭へ） | Transport Stop + Return to Start | 同上 | DAW 慣例 |
| 録音開始/停止 | Recording Arm/Record/Stop | 録音可能 Track 選択時 | #12 Session 状態遷移 |
| Undo | Project Undo | Project Command コンテキスト | 1 gesture=1 undo |
| Redo | Project Redo | 同上 | |
| 削除 | Delete Selected | Selection あり | Clip/Note/Region に適用 |
| コピー/貼付 | Copy/Paste | Selection あり | Clipboard 経由、Target に依存 |
| Tool 切替 | Tool Select/Smart/Split 等 | Timeline/MIDI View | #11 Tool 契約 |
| Zoom In/Out | View Zoom | Arrangement/MIDI View | Ctrl/Cmd+ホイール等（実装依存） |
| 横スクロール | View Scroll | 同上 | |
| 保存 | Project Save | 常時（Project 存在時） | Ctrl/Cmd+S（実装依存） |
| Export 開始 | Export Dialog/Render | 常時 | メニュー/Command |

**安全契約**:
- 上記キーは TextInput/IME/Plugin GUI/Modal 中は Project
  Command に流れない。
- 録音キーは Recording Session が Armed/Recording の場合
  のみ有効で、Transport Play と競合しない。
- Undo/Redo は Project Controller の Revision 履歴に基づき、
  Window/Dock 移動や View 切替で失われない。

### 7.3 Gesture 契約（ポインタ操作）

| Gesture | 開始条件 | Preview | Commit | Cancel | Undo 単位 |
| --- | --- | --- | --- | --- | --- |
| Clip ドラッグ移動 | Select Tool + Clip 上 | 移動先プレビュー（Snap 適用） | ドロップで配置確定 | Esc/範囲外ドロップ | 1 gesture=1 undo |
| Clip trim | Select Tool + Clip 端 | 新しい範囲プレビュー | 確定で trim 適用 | Esc | 1 undo |
| Note 描画/移動 | Draw Tool or Select + Note | Note 形状プレビュー | 確定 | Esc | 1 undo |
| Split | Split Tool or Command + 位置 | 分割位置プレビュー | 確定で 2 Clip/Note に分割 | Esc | 1 undo |
| Window/Dock 移動 | Window/Dock 境界ドラッグ | 移動プレビュー | 確定で配置変更 | Esc | Navigate（Project 不変） |

**全 Gesture 共通**: Gesture 開始時に対象 ID + Revision を
固定し、Commit 時に Revision が変化していれば Conflict と
して拒否（再試行可能）。Preview 中に Window/Dock を移動し
ても Gesture は継続し、Project は変更されない。

## 8. 共通操作体系・Window/Dock 独立性

### 8.1 Window/Dock/Focus 独立性（WF-AC-02）

- Window/Dock の移動・View の開閉・Focus の変更は #3 の
  **Navigate/Arrange** 分類に留まる。
- これらの操作は Recording Session・Transport 状態・
  Project Revision・選択中編集対象を **変更しない**。
- 録音中に Window を移動しても Recording は継続する
  （#12 AC-36 系）。Transport 再生中に Dock を移動しても
  再生は継続する。
- Focus が別 View に移っても、Selection は View ごとに
  保持され、Project の編集対象は変わらない（#11 Cross-View
  契約と整合）。

### 8.2 Selection/Command Target（WF-AC-03）

- Selection は View ごとに独立し、Project の編集対象は
  「最後に操作した View の Selection」または「明示的に
  Command Target に指定された対象」で決まる。
- 編集 Command（Delete/Copy/Paste/trim/move 等）は、
  Gesture/Key 押下時に確定した対象 ID + Revision に適用
  される。対象が変化した場合は Conflict 拒否。
- Track/Clip/MIDI Note/Take はそれぞれ一意の stable ID を
  持ち、Command は ID で対象を特定する（#10 PM-R-05 と
  整合）。
- Undo は Project Controller の Revision 履歴に基づき、
  1 gesture/1 command=1 undo 単位。Window/Dock/Focus の
  変更は Undo 対象外。

### 8.3 非破壊編集（WF-AC-04）

- Source/Asset（録音ファイル・Import ファイル）は不変。
  Clip/Take の trim/gain/fade/move/Comp はすべて derived
  projection であり、Source を変更しない。
- 録音ファイルは Staging→Journal→Finalize→Commit の
  回復可能なライフサイクルを持つ（#12 AC-36 系）。crash
  後は Journal から recovery し、完成済み Take は保全、
  中断 Take は discard する。
- Export は Source/Asset を直接変更せず、derived state
  から Offline Render する（#21 契約）。

## 9. Recording ワークフロー状態

### 9.1 Session 状態遷移（Gate A 最小形）

```
Idle → Armed → Recording → Stopping → Finalizing → Committed → Idle
                ↓ (crash)         ↓ (crash)      ↓ (crash)
            Journal recovery → Finalize or Discard
```

- **Idle**: 録音なし。Transport Play/Stop は通常動作。
- **Armed**: 録音準備。Transport Play で Recording へ遷移
  可能。Window/Dock 移動は状態を変更しない。
- **Recording**: 録音中。Take/Pass が Staging 領域に書き
  込まれる。Transport Stop/録音キーで Stopping へ。
- **Stopping**: 録音停止処理中。Buffer flush・Journal
  更新。
- **Finalizing**: ファイル確定・メタデータ記録。
- **Committed**: Take が Project に登録。Undo 可能。
- **crash recovery**: Journal から未完了 Take を検出し、
  Finalize 可能なら回復、不能なら Discard（#12 詳細契約
  に委譲）。

### 9.2 録音中の安全契約

- 録音中に Window/Dock を移動しても Recording Session は
  継続する（WF-AC-02）。
- 録音中に Transport Stop を押すと Stopping→Finalizing→
  Committed の順で安全に終了する。
- 録音中に Project Save を実行しても、録音データは Staging
  に保持され、Save 完了後に Finalize される。
- 録音中に別 Track を選択しても、Recording 対象は Armed
  時に確定した Track に固定される。

## 10. Remote Source 不通・VCS 非依存

### 10.1 Remote Source 不通（WF-AC-06）

- Browser の Remote Source が不通・未接続でも、以下は
  成立する: Local Import・録音・編集・保存・再生・
  Export。
- Remote Asset が Missing の場合、Browser に Missing 状態
  を正直に表示し、該当 Clip/Take は無音または placeholder
  で再生する（偽装しない）。
- Remote Asset が後で利用可能になった場合、再取得して
  Project に反映できる（#9 ALIB 契約）。
- GA-02（ソース取得）は Local Import または録音で満たせる
  ため、Remote 不通でも Gate A backbone は完結する。

### 10.2 VCS 非依存（WF-AC-07）

- Project Undo/Save/Load/Export/Transport は VCS
  Commit/History/Submit に依存しない。
- Semantic Diff・History View・Version 比較は Gate B/C の
  任意機能であり、VCS 未導入でも DAW として動作する。
- Project Save は単純なファイル永続化であり、VCS への
  Commit を要求しない。
- Export は Project 状態から直接 Offline Render し、
  VCS の有無に関係なく実行できる。

## 11. 検証レーン対応

| 検証レーン | 対象 | 本設計での位置づけ |
| --- | --- | --- |
| OS-L1 | headless/integration: Project/Undo/Export/WAV decode の論理検証 | 文書受入の最低限。`evidence/sv8/` に記録 |
| GUI-L2 | 実 Window/入力/Focus/DPI/IME: Piano Roll/Transport/Mixer/Dock 操作 | #112/#114 lane。文書では計画のみ、実測は NotRun |
| EXT-L3 | 実 Device/Plugin/外部環境: 実 Plugin Load/実音源/実 MIDI | #176 lane。文書では計画のみ、実測は NotRun |

**証拠の誠実さ**: 本設計の受入（WF-AC-01〜10）は「UX 仕様
文書が検証可能な形で規定されている」ことであり、実 OS/実
GUI/実機での Pass を意味しない。実機未検証項目は
`NotRun`/`Deferred`/`Unsupported` として `evidence/sv8/` に
記録する。

## 12. Decision Log

| ID | 決定 | 理由 | 代替案 | 状態 |
| --- | --- | --- | --- | --- |
| D-1 | Gate A backbone を GA-01〜08 の 8 工程に固定 | Issue #8 の必須最小タスクを漏れなく工程化し、全ワークフローで共通 Oracle を適用できるようにする | ワークフローごとに個別 backbone | 採用 |
| D-2 | WF-SCN-MIX の調査部分を任意 detour として分離 | Issue #8 AC「診断/グラフを強制せずに完結」を満たすため、Signal Flow/Inspector/VCS は spine から外す | 調査を必須工程に含める | 採用 |
| D-3 | Keyboard は「操作→Command」対応を固定し、キー割当は Deferred | 実装は #112、キー割当は OS/実装依存のため、本設計では操作の意味論のみ固定 | キー割当を本設計で確定 | 採用（キー割当は Deferred） |
| D-4 | Recording 状態を Idle→Armed→Recording→Stopping→Finalizing→Committed に固定 | #12 の Session/Take/Recovery 契約を Gate A 最小形に圧縮し、crash recovery を含めて検証可能にする | #12 の詳細状態をそのまま採用 | 採用（詳細は #12 に委譲） |
| D-5 | Meter（通常監視）と Probe（診断）を分離 | WF-AC-05 の「通常 Meter と Probe を混同しない」を契約化 | 同一視 | 採用 |
| D-6 | Remote 不通時は Missing 表示で制作継続 | WF-AC-06 の「ローカル制作・保存・再生・書き出しは可能」を保証しつつ、偽装を防ぐ | Remote 必須 | 採用 |
| D-7 | VCS を Gate A spine から完全に除外 | WF-AC-07 の「VCS なしでも DAW として動作」を保証 | VCS を前提にする | 採用 |

## 13. Risks and Deferred

### 13.1 Risks

- **R-1**: Keyboard/Gesture/Tool の最小操作集合が #112 実装
  時に不足する可能性 → #112 で追加操作を定義し、本設計を
  改訂する。
- **R-2**: Recording crash recovery の詳細が #12 と
  不整合になる可能性 → #12 の確定後に本設計を照合し、
  差異があれば改訂する。
- **R-3**: Remote Source 不通時の Missing 表示が実装で
  不十分になる可能性 → #9 ALIB の契約を確認し、必要なら
  本設計を改訂する。

### 13.2 Deferred

- Keyboard の具体的キー割当・OS 別既定バインド → #112
- Smart/Dedicated Tool の HitTest 優先順・Snap 閾値 → #11
- Take/Comp Editor UI 詳細 → #12
- Remote Asset Browser の検索/Preview 詳細 → #9
- Signal Flow/Inspector/Version Diff の詳細 → #4/#5/#7
- 実 OS/実 GUI/実 Device/実 Plugin 検証 → #114/#176
- 新規製品トレードオフ・契約破壊・定量 SLO・OS サポート
  宣言 → 人間決定待ち

## 14. Verification Notes

- 本設計は **文書成果物**であり、実機/実 GUI/実 Device の
  検証は行わない。
- `evidence/sv8/` に受入項目→仕様節→既存 docs/実装 Issue
  の台帳と、実行した検証（文書 lint・index 再生成・
  fmt-check）の記録を作成する。
- 実装 Issue（#10/#11/#12/#9/#112/#114 等）の進捗に応じて、
  本設計の Oracle が実機で検証可能かを継続的に確認する。
