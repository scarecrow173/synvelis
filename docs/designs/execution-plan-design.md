---
id: "DESIGN-034X83GOEQ84LyUmDRBepu"
type: "design"
status: "approved"
title: "Execution Plan Design — 横断 DAG・Owner/Gate/Acceptance Matrix・OS 検証レベル・Evidence 台帳構造"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/execution-plan.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/execution-plan.md"
  implemented-by: []
  depends-on:
    - "docs/specs/playback-graph.md"
    - "docs/specs/project-model.md"
    - "docs/specs/project-persistence.md"
    - "docs/specs/transport-integration.md"
    - "docs/specs/musical-time.md"
    - "docs/specs/recording-take-comping.md"
    - "docs/specs/automation-modulation.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
    - "docs/specs/pc-c6-oracle-rel-tr.md"
    - "docs/specs/svt-product-e2e.md"
    - "docs/specs/sv-gpui-01-native-app-shell.md"
    - "docs/specs/sv-gui-03-05-gpui-scenario.md"
    - "docs/specs/ci-os-l1-native-gpui.md"
    - "docs/specs/issue-166-full-ci-ci-summary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
    - "docs/designs/recording-take-comping-design.md"
    - "docs/designs/automation-modulation-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/execution-plan.md"
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
    - "docs/designs/recording-take-comping-design.md"
    - "docs/designs/automation-modulation-design.md"
  derived-by:
    - "docs/plans/execution-plan.md"
    - "docs/test-specs/execution-plan.md"
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/execution-plan.md"
    - "evidence/sv23/execution-plan-gate-ledger.md"
    - "evidence/sv23/execution-plan-entry-gate-ledger.md"
  defers: []
  deferred-by: []
---

# Execution Plan Design — 横断 DAG・Owner/Gate/Acceptance Matrix・OS 検証レベル・Evidence 台帳構造

## Context

Issue #23 は Synvelis Execution Plan v1.0 — F0・E2E-L/R/X・
横断依存・Acceptance Gate の横断 Owner である。個別
workstream の docs chain（sv#10/#11/#12/#17〜#20、並行中の
sv#21/#22）と Issue 群（RCP #289〜297、SF #104〜109、
Control/Automation #98〜103・#121/#122・#131〜134、
Time/MIDI/Plugin #19〜#22・#125〜130、Recording #12・
#115〜120、GUI #110〜114・#175〜177 + Ruviform #385〜390/
#464/#313/#243/#262/#419、Gateway #44〜46、VCS #92〜97、
ALIB #85〜91、CB #277〜288、AUTX #259〜267、CI #83・
Ruviform #219/#294）は個別に進行しているが、横断の単一
DAG・Owner/Gate/Acceptance Matrix・OS 検証レベルの正本が
ない。

本 design は spec `execution-plan.md`（EP-R-01〜12）の
詳細化であり、Issue #23 本文の「横断依存正規化 v1.1」
「GUI lane ordered DAG」「Stage 3 Start/Integration/
Acceptance 分離」「RCP/CB/AUTX 追加 DAG」を一つの
非巡回 DAG として統合し、POLICIES.md の OS-L1/GUI-L2/
EXT-L3 規範へ写像する。実装・CI 変更は行わない。

## Scope

### In Scope

- 横断単一 DAG の node/edge 定義（Start/Integration/
  Acceptance の 3 相 edge 型）。
- Owner/Gate/Acceptance Matrix（workstream × Issue ×
  Owner × Start/Integration/Acceptance × OS Level）。
- OS 検証レベル matrix と要求状態/実行結果の語彙規則。
- Evidence schema と `evidence/sv23/` 台帳構造。
- 正規入口（mise task）→ OS/Gate/Feature の対応表。
- 候補 SHA tuple registry（#262/#419・#294・#114/#109/
  #103）。
- Fail-closed な進捗判定規則と Failure Taxonomy。

### Out of Scope

- 実装コード・CI workflow・Cargo/mise 設定の変更。
- 個別 workstream の詳細設計・fixture・oracle の再定義
  （各 docs chain が正本）。
- Issue の Close・受入判定の実施。
- sv#21/sv#22 並行 Worker の文書の編集・作成。
- 新規製品方針・数値 SLO・OS サポート宣言。

## DAG Model

### Node / Edge 型

- **Node**: workstream または Issue 群。`SV-<track>-<id>`
  で一意に識別し、代表 Issue 番号を併記する。
- **Edge 型**:
  - `start`: 着手必須依存（相手 node の契約/型/Fixture
    が利用可能であること。Issue の全 Close を意味しない）。
  - `integration`: 統合接続時の依存（機能を接続する時点で
    必要な capability/証拠）。
  - `acceptance`: 最終受入 Gate への依存（製品機能の
    Pass 判定に必要な独立 Oracle/実機証拠）。
- **Gate node**: Gate A/B/C/AI・GUI-L2・EXT-L3・
  Cross-Consumer・RCP-07 等の受入判定点。Gate node は
  Start edge を持たず、Acceptance edge のみを受ける。
- **不変条件**: edge は常に「先行 node → 後続 node」
  の一方向。Acceptance Gate から Start への逆流 edge、
  同一 node 間の循環、未定義 node への edge を禁止する。

### 判定語彙

| 軸 | 値 | 意味 |
| --- | --- | --- |
| 要求状態 | `Required` | 現行必須（対象 OS/機能で提供・利用する） |
| 要求状態 | `Deferred` | 利用条件成立まで保留（macOS/Linux L2/L3・Future 系） |
| 要求状態 | `NotApplicable` | 対象機能では不要（未利用 Device/Format 等） |
| 実行結果 | `Pass` | 対象環境で実測合格（証拠 URL/ログ必須） |
| 実行結果 | `Fail` | 実測不合格 |
| 実行結果 | `NotRun` | 未実施（環境不在・CI 未実行・Skip/ignored・Feature 無効を含む。Pass/Unsupported ではない） |
| 実行結果 | `Unsupported` | 根拠を伴う技術的非対応 |
| 計画参照 | `NotRequired` | 当該 Gate/OS では要求しない（例: macOS の EXT-L3） |

能力 Class（Exact/Degraded/Unsupported/Unknown 等、
sv#20 TT 系）と実行結果は別語彙であり混同しない。

## Workstream Lane 定義

| Lane | Node | 代表 Issue | Owner（主） | 状態メモ |
| --- | --- | --- | --- | --- |
| 基盤設計 | SV-DESIGN | #1/#2/#4〜#12/#17〜#22 | Synvelis | #21/#22 docs は並行 Worker |
| F0/E2E | SV-F0 | #13（VS-A） | Synvelis | headless 実装済み |
| E2E-L | SV-L1/L2 | #24/#25 | Synvelis + Ruviform#184/#191/#195/#197/#198 | headless 実装済み |
| E2E-R | SV-R1〜R3 | #14/#15/#16 | Synvelis | headless 実装済み（Mock RT/Journal/Commit） |
| E2E-X | SV-X | #26 | Synvelis | headless 実装済み |
| Project | SV-PROJ | #10/#17 + docs sv#17 | Synvelis | docs chain 統合済み |
| Playback/Graph | SV-PLAY | #18 + docs sv#18 | Synvelis + Ruviform graph | docs chain 統合済み |
| Time | SV-TIME | #19 + docs sv#19 | Synvelis + Ruviform#189 | docs chain 統合済み |
| Transport | SV-TT | #20 + docs sv#20 | Synvelis + Ruviform#190 系 | docs chain 統合済み |
| Media/Render 設計 | SV-MEDIA | #21（sv#21 並行） | Synvelis | 参照のみ |
| Target Arch | SV-ARCH | #22（sv#22 並行） | Synvelis | 参照のみ |
| Signal Flow | SV-SF | #104〜#109 | Synvelis + Ruviform graph/probe | #104/#105 closed、#106〜109 open |
| Control/Automation | SV-CTL | #98〜#103・#121/#122・#131〜134 | Synvelis | #98/#99/#121 closed、他 open |
| Recording | SV-RCD | #12・#115〜#120 | Synvelis + Ruviform#187/#251 | #115〜118 open、F01/F02 future |
| Time/MIDI/Plugin | SV-TIM | #125〜#130 | Synvelis + Ruviform#188/#253/#307 | #125〜128 slice 実装済み |
| GUI Native | SV-GUI | #110〜#114・#175〜#177 + RF#385〜390/#464/#313/#243/#262/#419/#532 | Synvelis + Ruviform UI | GPUI Native First |
| Gateway/AI | SV-GW | #44〜#46・GW-01〜10 | Synvelis | headless 実装済み、AI Gate 別軸 |
| VCS | SV-VCS | #92〜#97 | Synvelis + ALIB#88 | #92〜#96 系 slice 進行 |
| ALIB | SV-ALIB | #85〜#91 | Synvelis | #85〜#91 slice 進行 |
| OBS | SV-OBS | #81 + Ruviform OBS#276/#277/#289/#291/#293 | Synvelis Host + Ruviform | #81 closed |
| CB | SV-CB | #277〜#288 + RF#562 | Synvelis + Ruviform | 設計合意・並行 Start 可 |
| AUTX | SV-AUTX | #259〜#267 | Synvelis | 設計合意・並行 Start 可 |
| RCP | SV-RCP | #289〜#297 | Synvelis + Ruviform#196/#198/#562 | 設計正本 #289 |
| CI/証拠 | SV-CI | #83 + RF#219/#294 + SV#28 + AL#127 | CI Owner（repo 別） | #83 closed、#294 open |

## Start / Integration / Acceptance Edge 表

Issue #23 本文の正規化済み DAG を edge 型付きで固定する。
`A →start B` は「B の着手に A の契約/型/Fixture が必要」、
`A →int B` は「B の統合に A の接続が必要」、
`A →acc B` は「B の受入に A の証拠が必要」を意味する。

### 基盤・F0/E2E lane

| Edge | 型 | 根拠 |
| --- | --- | --- |
| SV-DESIGN(#17/#18/#19/#20) →start SV-F0 | start | F0 は Project/Stable ID/Controller/Save/Reload の単一 Owner（#13）として設計契約を利用 |
| SV-F0 →start SV-L1・SV-R1 | start | E2E-L/R は F0 の Project/Verified Media 契約の後 |
| SV-L1 →start SV-L2 | start | Export は Verified Clip/Graph 経路の後 |
| SV-R1 →start SV-R2 →start SV-R3 | start | Mock RT → Writer/Journal → Commit/Reload |
| SV-L2 + SV-R3 →start SV-X | start | E2E-X は L2 と R3 の両方合格後 |
| SV-F0/L/R/X →acc Gate-A | acceptance | Gate A = Core DAW（録音/編集/Save/Playback/Export/PCM 安全） |

### Signal Flow / Control lane（正規化 v1.1）

| Edge | 型 | 根拠 |
| --- | --- | --- |
| SV-SF-01(#104) →start SV-CTL-01(#121) →start SV-SF-02(#105) →start {SV-SF-03(#106), SV-CTL-02(#122)} | start | Routing/Stable IDs → ControlOwner/TimeScope → RoutingTransaction → Graph Lowering と Host Parameter を契約上並行 |
| SV-CTL-03(#100) と Ruviform#307 →start SV-CTL-04(#101) | start | PC-C3 Trace と frame_base 証明が #101 の前提。#121 への逆流 edge 禁止 |
| SV-SF-03(#106) →start SV-SF-04(#107 headless) | start | Probe Adapter は Active Map 使用 |
| SV-SF-01(#104)/SV-GUI-02(#111) →start SV-SF-05(#108 read-only UI) | start | Read-only View Projection 先行 |
| SV-SF-04(#107) →int SV-SF-05(#108) | integration | Probe/診断 projection の段階接続 |
| SV-SF-06(#109) | — | Fixture/Oracle は並行開発、合格のみ Acceptance（start edge なし） |
| {SV-SF-03, SV-CTL-02, SV-CTL-04, SV-CTL-03} →acc SF-INTEGRATION-GATE | acceptance | Active Binding/旧世代誤配送ゼロの統合 Gate |
| SV-SF-03〜06 →acc Gate-B | acceptance | Gate B = Signal Flow 診断（configured/active/observed 区別） |
| SV-CTL-02(#122)+SV-PC-C5(#102) →acc Gate-C | acceptance | Gate C = Control Source/Target 双方向探索 |

### Automation lane

| Edge | 型 | 根拠 |
| --- | --- | --- |
| SV-CTL-01(#98 PC-C1) →start SV-AUT-02(#132) →start SV-AUT-03(#133) | start | 記録失敗時点列保全 P0 → Region 境界 → Loop 1Pass |
| SV-CTL-06(#103 PC-C6 Oracle) →acc SV-AUT-03(#133) 製品受入 | acceptance | #103 完了を #133 着手条件にしない（Issue 明記） |
| SV-AUT-01(#131)・SV-AUT-04(#134) | — | 独立並行（start edge なし） |

### Recording lane

| Edge | 型 | 根拠 |
| --- | --- | --- |
| SV-RCD-01(#115)+SV-SF-03(#106) →start SV-RCD-03(#117 headless/monitor) | start | Device/Graph/Monitor が使える段階 |
| SV-RCD-02(#116) →start SV-RCD-04(#118 comp/recovery) | start | Take/Comp/Recovery 接続 |
| SV-GUI-05(#114) | — | GUI scenario 作成は並行（start edge なし） |
| SV-RCD-01〜04 →acc EXT-L3-REC | acceptance | 実 Device/実機録音 Gate（Windows・提供範囲のみ） |

### Time/MIDI/Plugin lane

| Edge | 型 | 根拠 |
| --- | --- | --- |
| SV-TIME(#19) →start SV-TIM-01(#125) | start | Rational Beat/PPQN 基盤 |
| SV-TIM-01 →start SV-TIM-02(#126) | start | Time/MIDI Bridge 確認後 |
| SV-TIM-03(#127) | — | Ruviform #190/#253/#307 capability と PC-C4/SF-03 の統合で進行 |
| SV-TIM-04(#128) | — | Project/Render Snapshot + Plugin State 品質で進行 |
| SV-TIM-01〜04 →acc EXT-L3-PLUGIN | acceptance | 実 Plugin Process precision・実 MIDI backend |

### GUI Native lane（GPUI First・旧 WebView 順に優先）

| Edge | 型 | 根拠 |
| --- | --- | --- |
| Ruviform#384 →start Ruviform#262（GPUI 採否 PoC） | start | 後継 ADR 初期策定と最終採否を別 phase |
| Ruviform#385/#386/#387/#388/#389/#390・SV#177 | — | 契約設計/PoC は並行着手（start edge なし） |
| SV-GUI-01(#175 shell)+SV-GUI-03(#177 theme) →start SV-GUI-CORE(#112)・SV-GUI-04(#113) | start | 契約利用可能な段階で並行開始 |
| Ruviform#386/#388 + #389 →int SV-GUI-PLUGIN(#176 Plugin Dock) | integration | NativeSlot + Editor Session → 製品 Dock |
| SV-GUI-CORE(#112)+#113+#176+#177 →acc SV-GUI-E2E(#114) | acceptance | #114 は #175/#112/#113/#176/#177 と #102/#108 の該当 Feature 受入 |
| SV-SF-05(#108)+SV-PC-C5(#102) →acc SV-GUI-E2E(#114) | acceptance | Signal Flow/Control GUI feature の該当範囲 |
| 旧 SV-GUI-01/02(#110/#111 WebView) | — | 歴史的証拠。新 Native 受入を代替しない・Start 条件にしない |
| SV-GUI 系 →acc GUI-L2-WIN | acceptance | Windows 実 GPUI Window・入力→Receipt→Project/Undo/Save/2Window/Focus（#313/#243 driver + 独立 Oracle） |

### Gateway / AI lane

| Edge | 型 | 根拠 |
| --- | --- | --- |
| SV-GW-00(#44/#45) →start SV-GW-01〜10(#46) | start | Baseline/design 合意後に slice 化（実装済み headless） |
| SV-GW-01〜10 →acc AI-GATE | acceptance | AI Gate は独立軸。Gate A/B/C を block しない |

### VCS / ALIB / OBS / CB / AUTX / RCP lane

| Edge | 型 | 根拠 |
| --- | --- | --- |
| SV-VCS-01(#92) →start {SV-VCS-02(#93), SV-VCS-03(#94)} →start {SV-VCS-04(#95), SV-VCS-05(#96)} →acc SV-VCS-06(#97) | start/acc | Immutable Snapshot → Semantic Diff/Git Read 並行 → History UI/Apply → Cross Gate |
| SV-ALIB-01(#85) →start {SV-ALIB-02(#86), SV-ALIB-03(#87)} →start {SV-ALIB-04(#88), SV-ALIB-05(#89), SV-ALIB-06(#90)} →acc SV-ALIB-07(#91) | start/acc | Provider/Local → streaming/cache 並行 → remote/browser/webdav → 統合 Gate |
| Ruviform OBS(#276→#277→#289→#291/#293) →int SV-OBS(#81 host) | integration | 単一 ruviform-observability crate + Host Subscriber/権限 |
| SV-CB-00(#277) →start SV-CB-01(#278) →start {SV-CB-02〜07(#279〜284) adapters, SV-CB-08(#285), SV-CB-09(#286 mock), SV-CB-10(#287), SV-CB-11(#288 fixture)} | start | Typed Model+Fake Adapter 先行、Domain fixture/Gap 調査並行 |
| RF#562 PoC →int SV-CB lane・SV-SF-03 | integration | Graph/Plugin alternate execution 能力の統合 |
| SV-CB-01〜11 →acc CB-GATE(#288) | acceptance | CAS/Undo/Save/Raw Source・Audio Diff/PCM・RT/PDC/Plugin・Native GPUI/外部Plugin/OS・AI/VCS OFF・両 Consumer |
| SV-AUTX-00(#259) →start {SV-AUTX-01(#260), SV-AUTX-02(#261)} | start | Capability Action 投影と CLI Trusted Issuer を並行 |
| SV-AUTX-02(#261 auth 受入) →acc SV-AUTX-03(#262 mutating CLI 公開) | acceptance | mutating CLI 公開は #261 Auth 受入まで禁止 |
| SV-AUTX-05(#264) →start SV-AUTX-06(#265) | start | #264 が真の Start Blocker |
| SV-AUTX-07(#266)・SV-AUTX-08(#267) | — | Mock Contract/E2E Fixture から並行 |
| SV-RCP-00(#289) →start {SV-RCP-01(#290)〜SV-RCP-06(#295)} | start | #290〜#296 は Fake Adapter/Typed Contract で並行着手可能 |
| RF#562/#196/#198 + SV-SF-03(#106)/SV-TIM-04(#128)/SV-CB-00(#277)/SV-MEDIA(#285) →int SV-RCP-01〜06 | integration | RCP Integration は列挙 capability を要求 |
| SV-RCP-01〜06 →acc SV-RCP-07(#296) | acceptance | RT/PCM/3OS/Plugin/AudioLens の独立 Oracle（RCP-07 Cross Gate） |
| SV-RCP-F01(#297) | — | 後続 PoC。初期 Gate 非必須 |

### Cross-Gate / CI lane

| Edge | 型 | 根拠 |
| --- | --- | --- |
| SV-SF-06(#109)+SV-PC-C6(#103)+SV-GUI-05(#114) | — | 同一 Scenario/Run ID を共有する独立 Oracle（Runner/fixture/DB を重複構築しない） |
| 全 lane →acc CROSS-CONSUMER-GATE | acceptance | Ruviform#294 candidate SHA・SV#28 追従・AL#127・SV#83 CI・AL#142 CI の独立判定 |
| SV-CI(#83) | — | closed。Daily Full（変更検知）+手動 Full を正本運用として継続 |

## Owner/Gate/Acceptance Matrix

`evidence/sv23/execution-plan-gate-ledger.md` に全 node の
写像を保持する。要約:

| Gate | 判定内容 | Owner | 主な証拠 |
| --- | --- | --- | --- |
| Gate-A（Core DAW） | Create/Edit/Record/Save/Reload/Playback/Export・PCM/Timing/Recovery | Synvelis（#1・#8 Owner） | F0/E2E-L/R/X・VS-*・RCD headless・sv#17/#18 chain |
| Gate-B（Signal Flow 診断） | configured/active/observed 区別・修正・診断 | Synvelis #4/#109 + Ruviform probe | SF-03/04/05・OBS・sv#18 chain |
| Gate-C（Control 双方向） | Source⇄Target 探索・適用状態確認・復帰 | Synvelis #6/#102/#103 | PC-C2/C5・AUT・sv#20 chain |
| AI-Gate | Intentional Gateway 3 Mode・Grant/Approval・AI Off 回帰 | Synvelis #44〜46 | gw10 harness・mcp-poc |
| GUI-L2-WIN | 実 GPUI Window・入力→Receipt→Project/Undo/Save/2Window/Focus・Native Dock 画素 | Synvelis #112/#114/#176 + Ruviform#313/#243 | 実 OS driver 証拠・#388/#389・#532 |
| EXT-L3-WIN | 実 Device 録音/MIDI・実 VST3/CLAP/AU・Native Dock | Synvelis #115/#117/#126/#127/#176 + Ruviform#251/#253/#388/#389/#532 | 実機/実 Plugin ログ |
| CROSS-CONSUMER | 候補 SHA tuple・AL f64/長尺・SV PCM/Project 非回帰 | Ruviform#294 + SV#28 + AL#127 + 各 repo CI | candidate SHA run 証拠 |
| RCP-07 | Recipe/Cache/Branch/Plugin Missing/RT/VCS/3OS | Synvelis#296 + Ruviform | RCP chain |
| CB-GATE | #288 Cross-Gate | Synvelis#288 + Ruviform#562 | CB chain |

OS Level 規則（POLICIES.md §2 の写像）:

| OS | OS-L1 | GUI-L2 | EXT-L3 |
| --- | --- | --- | --- |
| Windows | Required | 実際に提供・利用する GUI 機能のみ Required | 実際に提供・利用する Device/Plugin のみ Required |
| macOS | Required | 利用要件発生まで Deferred | 利用要件発生まで Deferred |
| Linux | Required | 利用要件発生まで Deferred | 利用要件発生まで Deferred |

## Evidence Schema と `evidence/sv23/` 台帳

各 Case/Gate の証拠 record は以下を必須とする
（POLICIES.md §3）:

```
case_id | gate | repo | candidate_sha | consumer_resolved_sha
| os/arch | feature/test | requirement_state | result_state
| expected | observed | run_url/device_log | owner_issue | notes
```

- `candidate_sha`: 検証対象の Synvelis main/PR SHA。
- `consumer_resolved_sha`: Consumer 検証時の Ruviform
  resolved revision（`Cargo.lock` pin）。
- `requirement_state`: Required/Deferred/NotApplicable。
- `result_state`: Pass/Fail/NotRun/Unsupported
  （計画参照は NotRequired）。
- 実機・実 Plugin は Run URL/実機ログのパスを記録。

`evidence/sv23/` の構成:

| File | 内容 |
| --- | --- |
| `execution-plan-gate-ledger.md` | 全 workstream node × Issue × Owner × Start/Integration/Acceptance × OS Level × 現在状態の台帳 |
| `execution-plan-entry-gate-ledger.md` | 正規入口（mise task/CI lane）× OS × Gate/Feature × 対象 SHA × 結果状態の台帳 |
| `execution-plan-verification.md` | 本 slice の検証実行記録 |

## 正規入口 → OS/Gate/Feature 対応

`evidence/sv23/execution-plan-entry-gate-ledger.md` に詳細を
保持する。概要:

| 入口 | 範囲 | OS Level | Gate/Feature |
| --- | --- | --- | --- |
| `mise run check-ruviform-rev` | lock/vendor/manifest SHA 一致 | OS-L1 preflight | 全 lane 共通 |
| `mise run check-headless-deps` | headless crate に GUI/GPU/WebView 非依存 | OS-L1 | 境界不変条件 |
| `mise run check/lint/test/build <pkg>` | package 単位型/lint/test/build | OS-L1 | 変更 crate と直接影響範囲 |
| `mise run test-rt` | `rt_` contract（noalloc/nonblock） | OS-L1 | RT 安全性 |
| `mise run gw10`/`gw10-report` | GW-10 harness + gate report | OS-L1 | AI-Gate |
| `mise run check-app-native`/`lint-app-native`/`build-app-native` | `native` feature（GPUI driver 組立） | OS-L1 compile + GUI-L2 は `run-app-native` 手動 | GUI-L2-WIN の compile 側 |
| `mise run run-app-native` | 実窓起動（手動） | GUI-L2（手動実機） | GUI-L2-WIN 補助 |
| `mise run check-app-gpui`/`lint-app-gpui`/`test-app-gpui` | `gpui-test` feature（GPUI-simulated scenario） | OS-L1（simulated。実 Window/pixel ではない） | #114 L1 契約比較 |
| `mise run check-ui`/`test-ui`/`build-ui`/`install-ui`/`setup-ui`/`sync-ui-deps` | ui/ frontend（typecheck/vitest/build/vendor 同期） | OS-L1 | 旧 WebView lane・移行検証 |
| `mise run check-app-webview`/`lint-app-webview`/`test-app-webview`/`dev-ui`/`build-app` | `webview` feature（旧 SolidJS/WebView lane） | OS-L1 + 実窓は手動 | 歴史的/移行 lane |
| `mise run check-full`/`lint-full`/`test-full`/`build-full`/`verify-full` | workspace 標準 feature（現 OS） | OS-L1 | Daily/手動 Full CI と同一範囲 |
| `mise run test-recording-real-input`/`lint-recording-real-input` | opt-in 実 audio device | EXT-L3（手動・CI 対象外） | EXT-L3-WIN 録音 |
| `mise run test-recording-real-fs` | opt-in 実 FS durability probe | EXT-L3（手動・CI 対象外） | RCD-02 実 FS |
| `mise run ci-summary-gate`/`test-ci-summary`/`test-ci-tracker-entry` | CI 集約 gate 契約 | CI infra | #166/#167 |
| `mise run lint-actions` | workflow lint | CI infra | workflow 契約 |
| `mise run test-msrv` | MSRV toolchain | OS-L1（別 toolchain） | 本 slice では NotRun（ユーザー指定） |

注意: `gpui-test`/`webview` feature の成功は GPUI-
simulated/Browser 相当であり、GUI-L2（実 Window/実入
力/実 pixel）の Pass を意味しない。`run-app-native` の
手動起動確認も GUI-L2 の一部に過ぎず、#114 の製品
E2E 受入を代替しない。

## 候補 SHA tuple Registry

同一候補について次の tuple を `evidence/sv23/` で相互
参照可能にする:

```
candidate_tuple = (synvelis_sha, ruviform_resolved_sha,
                   os, arch, feature_set, oracle_set,
                   result_states)
```

| 追跡対象 | Owner | tuple の構成要素 |
| --- | --- | --- |
| GPUI 採用 | Ruviform#262（PoC）→#419（後継 ADR 最終採否） | ruviform SHA・OS 別 Window/Dock/Plugin Editor 実証・性能/互換 |
| Consumer 集約 | Ruviform#294 | candidate SHA・OS・Synvelis PCM/Project + AudioLens f64/CLI/RT の両 Consumer 独立 Oracle |
| 製品 Domain Oracle | Synvelis#114（GUI E2E）・#109（SF Oracle）・#103（Control Oracle） | synvelis SHA・OS・Scenario/Case ID・Expected/Observed・Pass/Fail/NotRun/NotRequired |
| CI 追従 | Synvelis#28・#83・AudioLens#127/#142 | repo 別 CI Run URL・対象 SHA・lane 結果 |

ルール: 片 repo の成功を他 repo/他 Consumer へ転用しない。
`NotRun`/`Fail`/`Unsupported` を `Pass` に読み替えない。
Daily CI 未実行・進行中・未対象 SHA は `NotRun`（実行中
なら Pending）であり最終受入は保留。

## Failure Taxonomy

| 失敗 | 分類 | 対応 |
| --- | --- | --- |
| DAG 循環（Acceptance→Start 逆流・相互待ち） | Structural | edge を integration/acceptance へ再分類するか、契約先行の start edge に修正。循環を残したまま着手しない |
| Owner 重複（共通基盤能力を Consumer で再実装） | Ownership | Ruviform Issue へ起票し Synvelis 側は利用契約のみ。恒久重複実装を禁止 |
| headless/Fake Pass の実機誤昇格 | Evidence honesty | `gpui-test`/`webview`/Fake Driver の結果を GUI-L2/EXT-L3 の Pass にしない。台帳で capability 別に分離 |
| `NotRun` の `Pass`/`Unsupported` 誤記 | Vocabulary | 要求状態と実行結果を別軸で記録。環境不在は NotRun |
| CI 未対象 SHA の Pass 主張 | Evidence | merge 後 main SHA の必須 CI 結果が統合検証の証拠。ローカル検証≠統合受入 |
| 新規 CI Workflow/PR 必須 Full の増設 | Scope | 既存 Daily 変更検知 + 手動 Full のみ使用。増設は別 Issue/承認 |
| sv#21/#22 文書との競合 | Coordination | 並行 Worker のファイルを編集せず参照のみ。path 衝突時は Coordinator へ確認 |

## Risks

- **DAG の肥大化**: 全 Issue を node 化すると読みにくい。
  lane 単位の node + Issue 番号併記で粒度を抑え、詳細は
  個別 docs chain/Issue に委譲する。
- **Issue 状態の陳腐化**: 本 design は 2026-11-14 時点の
  Issue 状態を写像する。Issue の Open/Closed 変化は
  `evidence/sv23/` 台帳の更新で追従し、design 本文を
  都度書き換えない。
- **「実際に提供する機能」の未確定**: Windows L2/L3 の
  Required 範囲は製品提供範囲（#176・#115・#117・#126/
  #127 と Ruviform capability）に依存し、確定前に全組
  合せを Required にしない。
- **並行 Worker との index 競合**: docs index は生成物。
  競合時は merge → `index apply` 再生成で解消（検証流用
  可）。

## Deferred Design Concerns

- 実 Window/実 Device/実 Plugin/実 Provider の L2/L3 実測
  | 本 slice は計画・台帳のみ | synvelis#114/#176・
  ruviform#313/#243/#251/#253/#388/#389/#532 | 実装・受入
  Owner が計画 Pass を実機受入と誤読する。
- macOS/Linux の GUI-L2/EXT-L3 | 利用要件発生まで Deferred
  | 対象 OS での利用決定時に Required へ昇格 | 未実測を
  Pass にしない。
- AudioLens f64/長尺の非回帰実測 | Ruviform#294 candidate
  SHA Gate | synvelis#28・ruviform#219/#294・audio-lens
  #127/#142/#166 | f64 consumer 経路の実測は別 Gate。
- RCP-F01・AUT-F・RCD-F・CTL-F・TIM-F 系 | 初期 Gate 非必須
  | synvelis#119/#120/#123/#124/#129/#130/#135〜139/#297
  | 再始動条件が揃うまで Open のまま管理。
- SMF SMPTE/Format2・MIDI2/MPE/per-note・VCS write/
  commit/push/sync・three-way semantic merge・full real
  plugin/device/GUI 組合せ | 現行 Gate 外の non-goal |
  対象機能の利用決定時に個別 Issue/ADR で Required 判定
  | 未利用 Format/Device を Required にしない。
- workstream の provisional 値（Comp crossfade 20 ms
  equal-power・Touch return 150 ms Smoothstep・capture
  queue・journal checkpoint・monitor ramp・plugin
  processing accuracy・GUI LOD/performance SLO）| 計測/
  ADR/Consumer 証拠が揃うまで provisional のまま | 各
  workstream docs chain（sv#12・AUT・TIM・GUI 系）が正本
  | 未計測値を確定値として台帳に記録しない。
- 数値 SLO・性能 budget・互換性破壊の決定 | 本計画の範囲外
  | 個別 Issue/ADR | 未決定を Pass 扱いしない。

## Sources and Relations

- `docs/specs/execution-plan.md`（EP-R-01〜12・EP-AC-01〜09）。
- Issue #23 本文（Stage 3・OS 別受入・正規化 v1.1・GUI
  lane ordered DAG・RCP/CB/AUTX 追加 DAG・判定台帳）。
- Issue #1（Gate A/B/C/AI）・Issue #22（v0.7.1 Target
  Architecture）・POLICIES.md（OS-L1/GUI-L2/EXT-L3・
  Required/Deferred/NotApplicable・Pass/Fail/NotRun/
  Unsupported）。
- 上流 architecture baseline（spec front matter の
  related/derives-from と同一集合）。
- 個別 workstream docs chain と `evidence/sv*` 台帳。
- 関連 Issue: synvelis #1〜#12・#17〜#23・#28・#44〜#46・
  #81・#83・#85〜#97・#98〜#120・#121〜#134・#175〜#177・
  #259〜#267・#277〜#288・#289〜#297、ruviform #184〜#198・
  #219・#243・#251・#253・#262・#294・#305〜#314・#385〜
  #390・#419・#464・#562・#565、audio-lens #127/#142/#143/
  #166。
