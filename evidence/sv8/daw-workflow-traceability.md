# DAW Workflow — 受入項目→要件→仕様節→Scenario→Issue 対応追跡表 (Issue #8)

作成: 2026-11-14 / branch: `docs/8-daw-workflow`
対象: `docs/specs/daw-workflow.md`・
`docs/designs/daw-workflow-design.md`・
`docs/test-specs/daw-workflow.md`

## 1. WF-AC → Requirement → Design 節 → Scenario → Issue §

| WF-AC | spec 要件 | design 節（正本） | 関連 Scenario/工程 | Issue #8 対応 |
| --- | --- | --- | --- | --- |
| WF-AC-01 | WF-R-01/02 | §5 GA-01〜08 backbone + §6 WF-SCN-MIDI/AUDIO/MIX | 全 WF-SCN の spine | 受入「3ワークフローを診断/グラフを強制せずに完結」+ Gate A 最小タスク |
| WF-AC-02 | WF-R-03 | §8.1 Window/Dock/Focus 独立性 + §7.2 入力コンテキスト + §9.2 録音中安全契約 | EX-6（test-spec）・全 WF-SCN の中断ケース | 受入「Window/Dock移動でRecording/Transport/Projectを勝手に変更しない」 |
| WF-AC-03 | WF-R-04 | §8.2 Selection/Command Target + §7.1 K-1〜K-5 + §7.3 Gesture 契約 | EX-1/EX-2 の編集工程 | 受入「Track/Clip/MIDI Noteと編集Command/Undoが一貫して対象へ適用される」 |
| WF-AC-04 | WF-R-05 | §8.3 非破壊編集 + §9 Recording 状態/回復 + §6.2 録音分岐 | EX-4（crash recovery）・WF-SCN-AUDIO A-04/A-05 | 受入「Audio Clip/Takeの非破壊編集と録音ファイル回復の契約を検証する」 |
| WF-AC-05 | WF-R-06 | §6.3 Mixer/Routing/Meter 分離 + §6.3 detour 契約 | WF-SCN-MIX X-02〜X-05 | 受入「MixerとSignal FlowのRouting状態が一致し、通常MeterとProbeを混同しない」 |
| WF-AC-06 | WF-R-07 | §10.1 Remote Source 不通 | EX-5・WF-SCN-AUDIO A-03 失敗系 | 受入「BrowserのRemote Source不通でも、ローカル制作・保存・再生・書き出しは可能」 |
| WF-AC-07 | WF-R-08 | §10.2 VCS 非依存 | 全 WF-SCN（VCS なし前提） | 受入「VCSなしでもDAWとして動作し、History/VCS操作に依存しない」 |
| WF-AC-08 | WF-R-09 | §7 Keyboard/Gesture/Tool UX 契約（§7.1 原則・§7.2 キー操作・§7.3 Gesture） | 全 WF-SCN の操作工程 | タスク固有（Keyboard/Gesture/Tool UX 仕様定義） |
| WF-AC-09 | — | 文書チェーン spec→design→plan→task→test-spec→ir/exp + 本 evidence | — | —（タスク固有） |
| WF-AC-10 | WF-R-10 | design §11 検証レーン + §14 Verification Notes + 本 verification 記録 | 全件 | 受入「未検証の実機/実 GUI 項目は NotRun/Deferred として区別」（POLICIES §2/§3 + Issue 実行判断） |

## 2. Issue #8 §別 → 設計カバレッジ

| Issue 節 | 内容 | 設計側の対応 |
| --- | --- | --- |
| §基本原則 5 項目 | DAW制作第一・直接操作・非破壊・複数View/単一正本・深掘り | design §3 P-1〜P-5（不変条件として固定） |
| §制作領域と View（8 領域） | Project Setup/Arrangement/MIDI/Audio/Mixing/Automation/Export/Assets | spec「制作領域と View の対応」表（View・Command 系・Owner Issue） |
| §基本 UX・Command 安全性 | Selection/Target/Undo/入力境界 | design §7.1 K-1〜K-5・§8.2・§9.2 |
| §P0/P1/P2 ロードマップ | 直接編集(P0)/Recording・非破壊(P1)/調査系(P2) | spec「ロードマップ対応」表（Gate A/B/C 写像） |
| §代表ワークフロー 3 件 | MIDI中心/Audio中心/Mix・調査 | design §6.1/§6.2/§6.3（工程・Oracle・失敗系・検証レーン） |
| §受入条件 7 項目 | 診断非強制・Window/Dock・Command/Undo・非破壊/回復・Routing/Meter・Remote・VCS | WF-AC-01〜07（§1 表） |
| §詳細設計テーマ | Track/Clip モデル(#10)・Tool(#11)・Recording(#12)・Asset(#9) | spec Out of Scope + design §4 Owner Map（委譲を明記） |
| §Gate A 最小タスク | Track作成→Import/Record→編集→Mixer/Transport→Save/Reload→Export→独立検証 | design §5 GA-01〜08（工程・Oracle・Owner） |
| 2026-11-10 実行判断 | Worker が設計ドラフト〜review-protocol〜PR・新規トレードオフは分離 | design §12 D-1〜D-7・spec Deferred Design Concerns・本タスクの実施形態 |
| §Gate A/B/C 接続合意(2026-11-04) | Gate A=制作 spine、B/C=Understand/Refine | design §6.3 detour 契約・spec ロードマップ表 |

## 3. 既存 docs との整合確認

| 既存文書 | 確認内容 | 結果 |
| --- | --- | --- |
| `docs/specs/product-value-core.md`（#1） | Gate A/B/C/AI 定義・AM-1 Owner=#8/#23/#114・証拠状態区分。本書は Gate A UX 正本として整合 | 整合 |
| `docs/specs/workspace-ux-scenarios.md` + design（#3） | Navigate/Inspect/Edit/Arrange 分類・入力境界・WUX-SCN-A〜F。Window/Dock 独立性・K-3 入力ゲートは #3 契約を前提参照し再定義しない | 整合 |
| `docs/specs/workspace-model.md`（#2） | View/Window 寿命独立・Command routing・単一 Project 正本（P-4 の根拠） | 整合 |
| `docs/specs/project-model.md` + design（#10） | PM-R/PM-AC: Track/Lane/Clip/Source/Take/Time・stable ID・単一 writer・Save/Reload。GA-01/03/05/06 のモデル正本 | 整合 |
| `docs/specs/timeline-editing.md` + design（#11） | TE-01〜14: Selection/Tool/Snap/Gesture/Undo/Cross-View。design §7/§8 はワークフロー上の運用契約のみ規定し HitTest/Snap 数値は委譲 | 整合 |
| `docs/specs/recording-take-comping.md` + design（#12） | AC-01〜69: Session/Pass/Stream/Segment/Take/Comp・Frame 分離・RT/Writer/Recovery。design §9 は Gate A 最小形に圧縮し詳細は委譲 | 整合 |
| `docs/specs/playback-graph.md` + design（#18） | EX-001〜020: Playback/Mixer/Graph。GA-04・§6.3 Meter/Routing の実行正本 | 整合 |
| `docs/specs/transport-integration.md` + design（#20） | TT-001〜030: Time/Transport/MIDI/Automation/Plugin。GA-04/録音位置の正本 | 整合 |
| `docs/specs/media-render.md` + design（#21） | Import/Export/Offline Render/UI/Diagnostics。GA-02/07/08 の正本 | 整合 |
| `docs/specs/musical-time.md`（#19） | 可変 PPQN Musical Time。ワークフローの時間前提として参照 | 整合 |
| `docs/specs/project-persistence.md`（#17） | Save/Load/Undo 永続化。GA-05/06 の正本 | 整合 |
| `docs/specs/execution-plan.md`（#23） | EP-R/EP-AC: 横断 DAG・OS-L1/GUI-L2/EXT-L3。本書の検証レーン語彙と一致 | 整合 |
| `docs/specs/context-inspector.md`（#5）・`automation-modulation.md`（#6） | Gate B detour・Automation Lane の Owner として参照のみ | 整合 |
| `docs/specs/ai-native-baseline.md`（#44）・`command-gateway.md`（#45） | AI Gateway は Gate C/AI、spine 非依存として明記 | 整合 |
| `docs/specs/svt-product-e2e.md`（#114） | 製品 E2E・GUI-L2 lane。実機受入の Owner として参照 | 整合 |
| `docs/specs/alib-01-asset-provider-contract.md`（#9 系） | Remote/Local Asset 解決・Missing 表示の契約正本 | 整合 |
| `docs/specs/sv-gpui-*`・`sv-gui-*`（#112/#113 系） | GPUI native shell・dock transfer の実装経路。UX 規範の実装先として参照 | 整合 |
| POLICIES.md §2/§3 | Required/Deferred/NotApplicable・Pass/Fail/NotRun/Unsupported。実機未検証を NotRun/Deferred として記録し設計評価を Pass に換算しない | 整合 |
| `docs/mockups/` | HTML Mock は設計参考で正式仕様ではない。本書は Mock に依存しない | 整合（Mock 非依存） |

## 4. 未決定事項の所在（Deferred）

| 項目 | Issue #8 | design 節 | spec 節 | 確定先 |
| --- | --- | --- | --- | --- |
| Keyboard の具体的キー割当・OS 別既定バインド | 基本 UX 節 | §7.2 表（操作→Command のみ固定）・§13.2 | Deferred Design Concerns | #112 実装・実機 PoC |
| Smart/Dedicated Tool の HitTest 優先順・Snap 閾値・gesture 数値 | 基本 UX 節 | §7.3（契約のみ）・§13.2 | Deferred Design Concerns | #11（ED-A/ED-B 正本） |
| Take/Comp Editor UI 詳細（Audition・Comp Paint・Lane 表示） | 詳細設計テーマ | §6.2 録音分岐・§13.2 | Deferred Design Concerns | #12 後続設計 |
| Remote Asset Browser の検索/Preview/選択的 Import | 制作領域 Assets | §10.1・§13.2 | Deferred Design Concerns | #9/ALIB 後続 |
| Signal Flow/Inspector/Version Diff の詳細工程 | 代表ワークフロー Mix・調査 | §6.3 detour・§13.2 | Deferred Design Concerns | #4/#5/#7/#108/#109 |
| 実 OS Window/Focus/IME/DPI・実 Plugin GUI・実 Device | 受入条件の検証 | §11 検証レーン・§14 | WF-AC-10 | #114（GUI-L2）・#176（EXT-L3） |
| 新規製品トレードオフ・契約破壊・定量 SLO・OS サポート宣言 | 2026-11-10 実行判断 | §13.2・Decision Log | Deferred Design Concerns | 人間決定待ち（本タスクでは決定しない） |

## 5. Owner 委譲マップ（実装 Issue との対応）

| WF 領域 | 設計上の正本 | 実装 Owner Issue |
| --- | --- | --- |
| Project/Track/Clip/Time モデル | #10 project-model | #10（設計済）・実装 slice は #23 DAG |
| Selection/Tool/Snap/Gesture/Undo | #11 timeline-editing | #11・#112 |
| Recording/Take/Comp/Recovery | #12 recording-take-comping | #12・#115〜#118 |
| Playback/Mixer/Graph | #18 playback-graph | #18 |
| Time/Transport/MIDI/Plugin | #20 transport-integration | #20・#101 系 |
| Import/Export/Offline Render | #21 media-render | #21 |
| Save/Load/Undo 永続化 | #17 project-persistence | #17 |
| Workspace UX/Window/Dock | #3 workspace-ux-scenarios | #3・#112・#113 |
| Asset Library/Browser | #9/ALIB | #9 |
| Core Workspace GUI | — | #112 |
| 製品 E2E 受入 | #114 svt-product-e2e | #114 |
| Plugin Dock（実形式） | #176 | #176（EXT-L3） |
