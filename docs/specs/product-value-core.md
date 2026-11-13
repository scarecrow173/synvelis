---
id: "SPEC-034X3iKrMQOzlbCgtuckh0"
type: "spec"
status: "approved"
title: "Synvelis 製品価値と中核体験 — Gate A/B/C・UX評価計画の正本"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/1"
    - "https://github.com/scarecrow173/synvelis/issues/277"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/product-value-core-design.md"
    - "docs/plans/product-value-core-plan.md"
    - "docs/tasks/product-value-core-task.md"
    - "docs/test-specs/product-value-core.md"
    - "docs/designs/workspace-docking-state-architecture.md"
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
    - "docs/specs/sv-gui-01-desktop-app-bootstrap-webview-frontend-pipeline.md"
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
    - "docs/specs/pc-c5-01-relation-inspector.md"
    - "docs/specs/pc-c6-oracle-rows.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/product-value-core.md"
  references: []
  defers: []
  deferred-by: []
---

# Synvelis 製品価値と中核体験 — 正本

本書は GitHub Issue #1「Synvelis の製品価値と中核体験を定義する」で
合意された製品定義を、`docs/` の canonical 文書として正本化したもの
である。対象は**製品価値・中核体験・段階的受入 Gate・UX 評価計画の
定義**であり、個別機能の実装、UI の最終デザイン、データモデル、
crate/API 配置、OS 対応・配布形態の確定は対象外とする。

設計文書との分担: 本書は「何を・なぜ・誰のために・何をもって合格と
するか」の正本、`docs/designs/product-value-core-design.md` は
Gate 定義の構造化・Decision Log・証拠状態機械・Owner 境界・代表
シナリオの Expected Oracle を担う。

## What Should Be Built

- 製品価値の三層構造（Create / Understand / Refine）と中核体験
  「創作する。理解する。調整する。そして創作に戻る。」の製品定義。
- 段階的受入 Gate **A（Core DAW Creation）/ B（Structural
  Investigation to Refine）/ C（Bidirectional Control Exploration）/
  AI（Intentional Gateway Integration）** の目的・合格条件・初期範囲
  の扱い・証拠区分。
- UX 判断原則と「説明可能な情報」の境界（設定として既知の事実・
  実測値・推定・取得不能の区別）。
- 代表制作シナリオ A（一般的な楽曲制作）/ B（意図しない音量変化の
  調査）/ C（パラメータ制御元の探索）。
- Issue #1 の 6 つの検証観点を **Acceptance Matrix**（測定方法・
  合格基準・検証手段・現状の達成度）として整理した評価可能な形。
- 同一タスクでの既存 DAW との UX 比較手順・評価基準（計測項目・
  環境・参加者条件・判定方法。数値閾値は Baseline 計測後に別途合意）。
- Creative Branch（CB-00〜11, #277〜#288）との関係整理 — 別
  Feature Acceptance であり Gate A のハードブロッカーにしない。

## Why Now

- 本 Issue は製品方針の判断基準であり、以後の UI/UX、機能優先順位、
  アーキテクチャ、実装計画のすべてがこの定義を参照する。Issue 上の
  合意が蓄積してきたが、`docs/` の canonical 文書として未整備
  だったため、本書で正本化する。
- SV-GUI / SF / PC-C 系の headless・GPUI-simulated 実装が進行して
  おり、「どの証拠がどの Gate のどの段階を満たすか」を一箇所で
  判定できる定義が必要になっている。

## Users and Value

- **製品 Owner / 設計者**: 機能優先順位と受入 Gate の判断基準。
- **実装 Owner（#8/#23/#4/#6/#102/#103/#108/#109/#114 等）**:
  自分の Issue がどの Gate・Acceptance Matrix 行を進めるのかの
  対応表。
- **レビュアー / 受入判定者**: 「設計合意」と「製品受入」の区別、
  `NotRun`/`Unsupported` を `Pass` にしない判定規則。
- **将来のユーザーテスト実施者**: 比較手順・計測項目・判定方法の
  既定値。

## Scope

### In Scope

- 上記「What Should Be Built」の全項目。
- Acceptance Matrix の 6 行（基本制作 / 構造探索 / 文脈維持 /
  調整への接続 / 情報の正確性 / 競合比較）と各行の測定方法・合格
  基準・検証手段・現状の達成度。
- 実装 Issue への相互参照（#8/#23・#4/#6・#102/#103・#108/#109・
  #114・#2/#3・#44/#46・#277〜#288）。
- 既存 `docs/` との重複・矛盾の調査結果と解消方針
  （`evidence/sv1/issue-1-scope-map.md`）。

### Out of Scope

- 個別機能の実装、Ruviform 共通 UI/RT/IO API、Synvelis の具体的
  Project/Plugin/Recording 実装（各実装 Owner の Issue/PR）。
- UI モックアップ（`docs/mockups/synvelis-daw.html`）の最終
  デザイン・画面構成への昇格。
- Signal Flow / Inspector の具体的操作・実装方式、Project Model・
  Audio/MIDI/Control Graph・Automation/Modulation の詳細データ
  モデル、crate/API 単位の配置。
- OS 対応、ライセンス、配布形態、初期 MVP の完成機能リスト。
- 「すべての信号変化の原因を特定できる」という保証。
- 実 OS / RT / 実 GUI / 実 Plugin の受入完了の宣言 — 本書の合意は
  製品定義の合意であり、各 Gate の製品受入は Owner Issue の証拠
  で別途判定する。

## Requirements

- **PV-01 三層構造:** Create（制作基盤）/ Understand（構造の理解）/
  Refine（制作上の判断・調整）の三層を製品価値として定義し、
  ②の表示充実だけでは③を実現したことにしない。
- **PV-02 中核体験:** Create → Understand → Refine を強制モードで
  はなく、文脈を維持したまま行き来できる連続した体験として定義
  する。Where / What / Why / How の説明枠組みを持ち、第三者
  プラグイン内部等の観測不能領域を説明可能とみなさない。
- **PV-03 Gate 定義:** Gate A は AI・Signal Flow・VCS が OFF でも
  成立する通常 DAW 制作。Gate B は構造・状態を根拠つきで探索
  できる差別化体験。Gate C は制御関係の双方向探索から調整への
  接続。AI Gate は既定 OFF・明示的 Grant/Approval の独立軸。
  B/C/AI は A を無条件にブロックしない。
- **PV-04 証拠区分:** `Not started / Headless verified / Real GUI
  verified / Platform verified / Product accepted` を独立に記録
  し、Rust/TS 単体 CI・契約 fixture・Mock・headless E2E を実 UI
  操作の完了と混同しない。`NotRun`/`Unsupported`/`Unknown` を
  `Pass` にしない。
- **PV-05 情報の正確性:** 表示は実際に取得可能な設定・状態と
  一致させ、Configured / Active / Observed / Unknown・欠測・
  未対応・未検証を誤って同一結果にしない。
- **PV-06 UX 原則:** Progressive Disclosure / Context Continuity /
  Bidirectional Navigation / Information Integrity を UI 具体化の
  評価基準とし、既存原則 Connected / Transparent / Flexible /
  Reliable を前提とする。
- **PV-07 Acceptance Matrix:** Issue #1 の 6 検証観点を測定方法・
  合格基準・検証手段・現状の達成度つきで文書化し、根拠を
  `evidence/sv1/` に残す。
- **PV-08 UX 比較計画:** 同一タスク・同一初期条件での既存 DAW 比較
  手順と評価基準を定義し、数値閾値は恣意的に設定せず Baseline
  計測・基準値合意の後に決定する。実測前に性能優位を宣言しない。
- **PV-09 Creative Branch:** CB-00〜11（#277〜#288）を製品の探索・
  比較・採用体験の柱として位置付けつつ、独立した Feature
  Acceptance とし Gate A のハードブロッカーにしない。

## 製品定義（canonical 本文）

### 1. 製品の位置付け

- 特定ジャンルや用途に限定しない**汎用的な次世代 DAW**。
- Rust 製の独立した DAW アプリケーションとして構築し、汎用
  マルチメディア基盤 Ruviform を利用する。
- 日常の操作は Arrangement / Timeline、Mixer、プラグイン画面等の
  一般的な DAW 操作を中心とする。
- 信号経路・制御関係・状態などの構造を、必要に応じて理解・操作
  できるようにする。
- あらゆる操作をノード化しない。通常の制作体験を妨げず、必要な
  粒度で関係性を示す。
- 汎用性は「初期から全機能を揃えること」を意味せず、実用的な
  制作体験を段階的に成立させる。

暫定的な製品定義（Issue #1 合意文）:

> **創作の自由と、制作環境への理解を両立する DAW。**
> Synvelis は、作曲・録音・編集・ミキシング・サウンドデザインを
> 統合する汎用 DAW を目指す。一般的な DAW として自然で柔軟な
> 制作体験を提供しつつ、必要なときに信号経路、制御関係、処理
> 状態を把握して、制作上の判断と調整につなげられる環境を実現
> する。

### 2. 製品価値の三層構造

| 階層 | 提供価値 | 判断基準 |
| --- | --- | --- |
| ① Create / 制作基盤 | 作曲、MIDI、録音、非破壊編集、ミキシング、書き出しなどの制作を安定して行える | 構造表示を利用しなくても自然に制作できる |
| ② Understand / 構造の理解 | トラック、Bus、Send、Sidechain、Automation、Modulation などの関係を把握できる | 必要な接続・制御元・影響先を探し出せる |
| ③ Refine / 制作上の判断・調整 | 関連する設定へ移動し、意図した結果に向けて調整・検証できる | 理解した内容が具体的な作業改善につながる |

可視化は目的そのものではなく「創作の自由と、制作環境への理解を
両立する」ための手段。②の表示機能の充実だけでは③の価値を実現
したことにならず、③は①の使いやすさ・信頼性を犠牲にして成立
させない。

### 3. 中核体験

> **創作する。理解する。調整する。そして創作に戻る。**

- **Create:** 通常の DAW 操作で制作を進める。構造表示の利用は
  必須にしない。
- **Understand:** 必要になった対象から、既知の接続・制御元・
  影響先・処理状態を確認する。
- **Refine:** 関連する設定へ移動して調整し、結果を確かめたら
  制作へ戻る。

独立した強制的なモードではなく、制作中に文脈を維持したまま
必要に応じて行き来できる連続した体験として設計する。

説明枠組み（Where / What / Why / How）:

- **Where:** どのトラック、Bus、ノード、パラメータに関係するか。
- **What:** 何が変化したか（取得可能な値・信号状態など）。
- **Why:** 既知の制御元、接続、操作の関係は何か。
- **How:** どの経路・適用順序・変換を通じて影響したか。

これは万能な因果推定機能を約束するものではない。第三者
プラグイン内部などホストが観測できないものは説明可能とみなさ
ない。**設定として既知の事実、実測値、推定、取得不能**を区別
する（実装側の状態語彙との対応は design §4 を参照）。

### 4. 段階的受入 Gate

| Gate | 目的 / シナリオ | 製品としての合格条件 | 初期範囲の扱い |
| --- | --- | --- | --- |
| **A — Core DAW Creation** | シナリオ A: 通常制作 | GUI から Audio/MIDI 作成・録音・非破壊編集、Track/Mixer/Instrument/FX、Save/Reload、Playback、実 PCM WAV Export/Reopen まで一貫して操作できる。**Signal Flow/AI/VCS を必要としない**。 | 基本制作の最低成立条件（必須） |
| **B — Structural Investigation to Refine** | シナリオ B: 意図しない音量変化を調査 | Track→Routing/Bus/Send/Sidechain/Automation/Plugin の取得可能な関係を探索し、設定へ移動・変更・検証して制作 View へ復帰できる。**既知の接続・観測事実・仮説/因果を区別**する。 | 最初の差別化体験を実証する独立 Gate |
| **C — Bidirectional Control Exploration** | シナリオ C: 制御元/制御先の探索 | Automation/LFO/Modulation の制御元→対象、および対象→制御元を追跡し、調整・状態更新・制作復帰を実証。B の共通探索契約を再利用する。 | 段階的拡張 Gate。A を無条件にブロックしない |
| **AI — Intentional Gateway Integration** | AI を明示的に有効化したときの Agent/Manual 共存 | 既定 OFF、明示的 Grant/Approval、ProjectController Single Writer、Suggest / Approve-to-Apply / Delegated Apply の初期 3 Mode、AI Off 時の通常 DAW 非回帰を検証。 | A/B/C とは別の受入軸。AI を使わず A 成立が必須 |

証拠状態は `Not started / Headless verified / Real GUI verified /
Platform verified / Product accepted` を Gate ごとに独立して記録
する。代表 headless PoC や既存 PR の merge を製品 Gate Passed
と扱わない。実 Audio Input/OS MIDI/Plugin、複数 Window/実 GUI、
Save/Reload、Crash/Recovery、Export 再読込、RT no-block/
no-allocation、性能・精度はシナリオに関係するものを OS/Backend/
fixture/commit/ログとともに報告する。

初期正式リリースで B/C/AI のどの Gate を必須とするかは本定義
から自動決定しない。合意済みなのは **A の必須性、B を最初の
差別化検証とする優先順、C/AI の独立性**のみ。

### 5. UX 判断原則と説明可能性の境界

既存の製品原則 Connected / Transparent / Flexible / Reliable を
前提に、UI 具体化の評価基準として次を使う。

| 原則 | 意味 |
| --- | --- |
| Progressive Disclosure | 日常操作に不要な詳細は常時表示せず、必要なときだけ段階的に開示する |
| Context Continuity | Arrangement、Mixer、Signal Flow、Inspector を移動しても、選択対象と作業文脈を保つ |
| Bidirectional Navigation | 信号の入力元・出力先、制御元・影響先を両方向からたどれる |
| Information Integrity | 設定・観測値・推定・未取得情報を混同せず、取得可能な範囲を明示する |

表示画面やグラフの存在自体ではなく、**ユーザーが目的の情報と
操作に到達できるか**を評価する。

説明可能性の境界: 表示可能な情報と因果関係の断定を分ける。
Sidechain の接続が見えることと、それが音量低下の実原因と証明
できることは異なる。設定として既知の事実・実測値・推定・
取得不能を区別し、未観測情報を断定しない。

### 6. 代表的な制作シナリオ（検証対象）

**シナリオ A — 一般的な楽曲制作（基本体験）:**

1. MIDI で楽器のフレーズを作成する。
2. オーディオトラックを追加・編集する。
3. 音源・エフェクト・Mixer を使い、楽曲制作を進める。

期待する価値: 信号フロー図や診断画面を開かなくても一連の作業が
自然に成立する。**汎用 DAW としての品質を検証するシナリオ**。

**シナリオ B — 意図しない音量変化の調査（最初の特徴的体験の
候補）:**

1. Bass トラックの音量低下など、意図と異なる挙動に気付く。
2. 対象を選ぶと、Inspector から Bus への出力経路、既知の
   Sidechain 接続、Automation などの関連情報を確認できる。
3. 関係する設定へ移動し、調整して結果を確認する。
4. 元の制作文脈へ戻る。

期待する価値: 複数画面に分かれた情報を頭の中だけで統合せず、
関係の探索から調整まで進められる。

**シナリオ C — パラメータ制御元の探索（拡張可能性の検証）:**

1. Filter Cutoff などのパラメータを選択する。
2. Automation / LFO 等の接続済み制御元を確認する。
3. LFO 側を選択すると、関連する制御先を逆方向から確認できる。
4. 必要な設定を調整し、再び制作に戻る。

期待する価値: 受け手→制御元、制御元→受け手の**双方向の関係
探索**が一貫して行える。

優先度: A を最低限の成立条件とし、差別化を検証する最初の対象
として B を具体化する。C は同じ UX 原則が他の制御関係にも適用
できるかを検証する候補。実装順序そのものは #23 系の実行計画が
別途決める。

### 7. 対象ユーザー（初期仮説）

- **中心的な検証対象:** MIDI とオーディオを組み合わせ、作曲から
  ミキシングまで行う個人制作者・プロデューサー。
- **重要な追加検証対象:** 柔軟な音響処理や複雑な制御を扱う
  サウンドデザイナー。
- **順次検証:** 多数の Bus / Send 等を扱うミキシングエンジニア。
- **将来の拡張領域:** 映像・ゲーム向け音響制作。

対象を限定するためではなく、**最初に誰の作業を基準として使いや
すさを評価するか**を定めるための区分。実際のユーザー調査による
裏付けは未取得。

### 8. 差別化の仮説と注意点

REAPER のルーティング表示、Bitwig Studio の Modulation
Inspector など、既存 DAW にも構造や制御関係を把握する手段が
ある。**ノード表示や制御元の可視化だけを独自性とは主張しない**。

差別化の仮説:

> 制作対象を中心に、信号・制御・状態の関連情報を画面横断で
> たどり、必要な調整操作まで自然につなげられること。

これは競争優位を確認済みの事実ではない。既存 DAW と同じタスク
を比較し、操作の負担・理解のしやすさ・正確さを検証する
（評価手順は §9）。

### 9. UX 比較・ユーザー評価計画

#### 9.1 目的と前提

- 同一タスク・同一初期条件で Synvelis と既存 DAW を比較し、
  操作の負担・理解のしやすさ・正確さについて具体的な差分を
  得る。
- **数値目標は恣意的に設定しない。** 試験内容・基準値を定めて
  から閾値を決定する。実測前に性能優位や閾値達成を宣言しない。
- ユーザーテストが未実施なら `NotRun` と記録する。

#### 9.2 タスクと比較対象

- **タスク:** 代表シナリオ A/B/C を、操作手順・期待結果・
  Expected Oracle つきの評価タスクとして具体化する
  （Expected Oracle は `docs/test-specs/product-value-core.md`
  と `docs/designs/product-value-core-design.md` を正本とする）。
- **比較対象 DAW（初期候補）:** REAPER（ルーティング表示）、
  Bitwig Studio（Modulation Inspector）。追加対象は評価計画の
  レビューで決定する。
- **環境・参加者:** 同一の初期 Project・素材・タスク定義。参加者
  は §7 の中心的検証対象（MIDI+Audio 個人制作者）を優先し、
  経験年数・使用 DAW を記録する。実施環境（OS・画面・音声
  機器）は証拠として記録する。

#### 9.3 計測項目

| 項目 | 定義 |
| --- | --- |
| タスク達成率 | タスクの Expected Oracle を満たした割合 |
| 所要時間 | タスク開始から達成/断念までの時間 |
| 操作数 | クリック・キー入力・画面遷移の回数 |
| 誤操作・回復 | 意図しない操作の回数と、そこからの回復可否 |
| 理解度 | タスク後アンケート等による主観的理解度（例: 関係を説明できたか） |
| 探索コスト | Understand/Refine のために要した画面遷移・対象切替の回数 |

#### 9.4 判定方法

1. まず比較対象 DAW 側の Baseline を同一条件で計測する。
2. Synvelis 側を同一条件で計測する（対象機能の実装・GUI 受入が
   先行すること — 本書の合意だけでは計測不能）。
3. Baseline と Synvelis の差分を記録し、**閾値は Baseline 結果を
   確認した上で別途合意する**。
4. 結果は `evidence/` 配下に環境・参加者属性・生データ・判定を
   残す。`NotRun`（未実施）/`Unsupported`（対象機能なし）を
   区別する。

### 10. Creative Branch との関係

Creative Branch 正本（#277）は、領域横断で **Fork → Edit →
Compare → Adopt** を提供する探索・比較・採用体験の新しい柱。
Audio Clip/Take、MIDI Clip/Source、Instrument、Processing/FX、
Track/Bus、Arrangement を初期構想に含める。

- Compare は Semantic Diff（変更の意味）・Signal Diff（実測音声
  差）・Perceptual Audition（聴取判断）を独立した証拠として提供
  し、Difference Explorer で結び付ける。
- **Adopt は 1 候補の宣言 Scope を正式 Project へ適用すること**。
  Semantic 3-way Merge/Cherry-pick や Mix/Blend と区別。
  VCS/Git/AI は必須ではない。
- 既存の通常 DAW Gate A と B/C/AI の独立 Gate 定義・初期
  リリース範囲未確定という判断は変えない。Creative Branch は
  別の Feature Acceptance で評価し、未完成を Gate A の
  ハードブロッカーにしない。
- 下流: #278〜#288、汎用 RT/Plugin 比較の Gap PoC は
  Ruviform #562。

### 11. Acceptance Matrix

Issue #1 の 6 検証観点を評価可能な形に整理したもの。各行の根拠・
証拠リンクの詳細は `evidence/sv1/acceptance-matrix.md` を正本の
補足とする。

| # | 項目 | 対応 Gate / Owner | 測定方法 | 合格基準 | 検証手段 | 現状の達成度 |
| --- | --- | --- | --- | --- | --- | --- |
| AM-1 | 基本制作 | Gate A / #8・#23・#114 | 代表タスク A を実 GUI で実行し、各ステップの完了・操作数・誤操作・回復を記録 | Signal Flow/AI/VCS なしで全ステップ完了。未対応機能は記録済み。数値閾値は Baseline 後 | GUI-L2 実機 E2E（#114）、OS-L1 headless 補助、EXT-L3 Export/再読込 | 定義合意（本書）。headless State/Receipt 証拠あり（`evidence/sv114/`）。実 GUI/実機器受入は NotRun |
| AM-2 | 構造探索 | Gate B / #4・#108・#109 | 代表タスク B：選択トラックの上流/下流・制御元/影響先を探索し設定へ移動・復帰 | 既知の接続・観測事実・仮説/因果を区別。Unknown/Unobserved を断定しない。経路が実設定と一致 | headless oracle（SF-05/SF-06・PC-C2/C5）+ GUI-L2 実シナリオ | headless oracle 証拠あり。実 GUI 探索シナリオは NotRun |
| AM-3 | 文脈維持 | 横断 / #2・#3・#114 | 探索中の Editing Selection・Inspector Follow/Explore/Pinned・戻り先を 2 Window 含む代表操作で確認 | 画面遷移で対象を見失わず元の制作作業へ復帰。Window 移動/Detach/Close で View/Project 状態を失わない | #2/#3 設計、ws-a・sv-gui-04 headless、#114 GUI-L2 | 設計・headless 契約あり。実機 multi-window は GUI-L2 NotRun |
| AM-4 | 調整への接続 | Gate C / #6・#102・#103 | 代表タスク C：制御元⇄対象の双方向探索→設定調整→Undo→制作復帰。操作数・誤操作・作用 Target を観測 | 双方向探索が一貫。調整が意図した Target に作用し Undo 可能 | PC-C5/C6 headless oracle + GUI-L2 | PC-C5/C6 headless 証拠あり。実 GUI 評価は NotRun |
| AM-5 | 情報の正確性 | 横断 / #5・#109・#103 | 表示内容と実際に取得可能な設定/状態の照合。Configured/Active/Observed/Unknown・欠測/未対応/未検証の区別を Oracle で検証 | 未観測情報を断定しない。Mock/Native GUI/RT/Plugin の Oracle を別所有者・結果状態で識別 | PC-C3 trace slot、SF-04 probe、PC-C6 oracle、`syn.app.info` の honest reporting | headless で状態区別を検証済み。実表示との一致は GUI-L2/EXT-L3 NotRun |
| AM-6 | 競合比較 | 製品評価 / 本書 §9 | 同一初期条件の代表タスク A〜C を Synvelis と比較対象 DAW で実施 | 達成率/時間/操作数/誤操作/理解度を計測。閾値は Baseline 後に合意。実測前に優位を宣言しない | UX 評価計画（§9）+ ユーザー調査 | 手順・評価基準を定義。Baseline 未計測・ユーザーテスト未実施 → NotRun |

### 12. 未決定事項と後続 Issue

本 Issue の段階で確定しないこと（Issue #1 §9 を踏襲）:

- UI モックアップを最終デザイン・画面構成とみなすこと。
- Signal Flow / Inspector の具体的操作・実装方式。
- Project Model、Audio/MIDI/Control Graph、Automation/Modulation
  の詳細データモデル。
- Ruviform と Synvelis の crate/API 単位の配置。
- OS 対応、ライセンス、配布形態、初期 MVP の完成機能リスト。
- すべての信号変化の原因を特定できるという保証。
- 初期正式リリースで必須とする Gate の範囲（A 必須・B 優先・
  C/AI 独立のみ合意済み）。

後続の検討接続: 代表ワークフローの UX 設計（#2/#3/#8 系）、
初期機能要件の優先順位（#23 系）、アーキテクチャ（#17〜#22 系）、
検証・実装計画（#23・#114 系）。

## Acceptance Criteria

本 spec の受入は「製品定義としての合意・検証計画の整備」であり、
実装の完成条件・実 OS/実 GUI/実 Plugin の製品受入ではない
（Issue #1 明示）。

- [x] PV-01〜PV-09 の要求が本文書に定義されている。
- [x] Gate A/B/C/AI の目的・合格条件・初期範囲の扱い・証拠区分が
  §4 に定義されている。
- [x] Acceptance Matrix（§11）が 6 項目すべてについて測定方法・
  合格基準・検証手段・現状の達成度を持つ。
- [x] UX 比較手順・評価基準（§9）が定義され、数値閾値を Baseline
  後に決める方針が明記されている。
- [x] Creative Branch（#277〜#288）との関係が §10 に記録され、
  Gate A のハードブロッカーにしないことが明記されている。
- [x] 実装 Issue（#8/#23・#4/#6・#102/#103・#108/#109・#114・
  #2/#3・#44/#46/#60）への相互参照がある。
- [x] 設計合意のみで OS/RT/実 GUI 受入を済ませない旨と、各 Gate
  の製品受入が Owner Issue の証拠で別途判定されることが明記
  されている。
- [x] 既存 docs との重複・矛盾の調査と解消方針が
  `evidence/sv1/issue-1-scope-map.md` に記録されている。

## Deferred Design Concerns

- UX 比較の数値閾値・比較対象 DAW の最終リスト・参加者募集 —
  Baseline 計測と評価計画レビュー後に決定（再評価契機: AM-6 の
  初回計測計画レビュー）。
- Creative Branch の個別 Feature Acceptance 詳細 — CB-00 #277 と
  CB-11 #288 が正本（再評価契機: CB 各 Issue の受入）。
- 初期リリースでの B/C/AI 必須化判断 — 製品 Owner の別途決裁
  （再評価契機: Gate A の製品受入証拠が揃った時点）。

## Sources and Relations

- Source: [synvelis#1](https://github.com/scarecrow173/synvelis/issues/1)
  （製品価値・中核体験・Gate A/B/C/AI・UX 評価の合意本文）、
  [synvelis#277](https://github.com/scarecrow173/synvelis/issues/277)
  （Creative Branch 正本）。
- Related canonical docs: `docs/designs/product-value-core-design.md`、
  `docs/plans/product-value-core-plan.md`、
  `docs/tasks/product-value-core-task.md`、
  `docs/test-specs/product-value-core.md`。
- 下流の実装・検証 Owner: #8（標準 DAW 制作ワークフロー）、
  #23（Execution Plan / Acceptance Gate）、#4（Signal Flow 定義）、
  #6（Automation/Modulation UX）、#102（PC-C5 UI 統合）、
  #103（PC-C6 横断 Gate）、#108（SF-05 Signal Flow Editor）、
  #109（SF-06 Cross Gate）、#114（SV-GUI-05 製品 E2E）、
  #2/#3（Workspace/Docking・共通インタラクション）、
  #44/#46/#60（AI Baseline/Gateway/Grant・Approval UI）、
  #277〜#288（Creative Branch）、#28（Ruviform 追従・Consumer
  回帰）。
- 補足証拠: `evidence/sv1/acceptance-matrix.md`（Acceptance
  Matrix 根拠）、`evidence/sv1/issue-1-scope-map.md`（既存 docs
  との重複・矛盾調査）、`evidence/sv1/verification.md`（本変更の
  検証記録）。
