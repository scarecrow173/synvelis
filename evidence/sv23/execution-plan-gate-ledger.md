# SV23 Execution Plan Gate Ledger

Issue #23 横断実行計画の Gate/Acceptance 台帳。
`docs/designs/execution-plan-design.md` の DAG Model・
Workstream Lane・edge 表を写像し、各 node の Owner・
Start/Integration/Acceptance・OS Level・現在状態を記録
する。写像時点の Issue 状態は変化しうるため、変化は
本台帳の更新で追従する（design 本文は書き換えない）。

## 判定語彙

- 要求状態: `Required` / `Deferred` / `NotApplicable`
- 実行結果: `Pass` / `Fail` / `NotRun` / `Unsupported`
- 計画参照: `NotRequired`
- Owner 区分: `Synvelis`（製品 Domain）/ `Ruviform`
  （共通基盤）/ `AudioLens`（Consumer）/ `CI`
  （証拠・CI Owner）

## OS Level 規則

| OS | OS-L1 | GUI-L2 | EXT-L3 |
| --- | --- | --- | --- |
| Windows | Required | 提供・利用 GUI 機能のみ Required | 提供・利用 Device/Plugin のみ Required |
| macOS | Required | 利用要件発生まで Deferred | 利用要件発生まで Deferred |
| Linux | Required | 利用要件発生まで Deferred | 利用要件発生まで Deferred |

## Lane 別 Node 台帳

### 基盤設計・F0/E2E lane

| Node | Issue | Owner | Start | Integration | Acceptance | OS Level | 状態（写像時点） |
| --- | --- | --- | --- | --- | --- | --- | --- |
| SV-DESIGN | #1/#2/#4〜#12/#17〜#22 | Synvelis | — | — | Gate-A/B/C/AI の製品定義 | OS-L1 | docs chain 統合済み（#21/#22 並行） |
| SV-F0 | #13（VS-A） | Synvelis | Project/Stable ID/Controller 契約 | E2E-L/R/X の前提 | Gate-A | OS-L1 | headless 実装済み（Issue state は台帳追従） |
| SV-L1/L2 | #24/#25 | Synvelis + Ruviform#184/#191/#195/#197/#198 | SV-F0 | SV-X へ | Gate-A | OS-L1 | headless 実装済み |
| SV-R1〜R3 | #14/#15/#16 | Synvelis | SV-F0→SV-R1→SV-R2→SV-R3 | SV-X へ | Gate-A | OS-L1 | headless 実装済み（Mock RT/Journal/Commit） |
| SV-X | #26 | Synvelis | SV-L2 + SV-R3 | — | Gate-A | OS-L1 | headless 実装済み |

### Signal Flow / Control lane

| Node | Issue | Owner | Start | Integration | Acceptance | OS Level | 状態 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| SV-SF-01 | #104 | Synvelis | — | SV-CTL-01 の start | SF-INTEGRATION-GATE | OS-L1 | closed（写像時点） |
| SV-CTL-01 | #121 | Synvelis | SV-SF-01 | SV-SF-02 の start | Gate-C 補助 | OS-L1 | closed |
| SV-SF-02 | #105 | Synvelis | SV-CTL-01 | {#106,#122} の start | SF-INTEGRATION-GATE | OS-L1 | closed |
| SV-SF-03 | #106 | Synvelis | SV-SF-02 | RCP/Recording/TIM の int | SF-INTEGRATION-GATE・Gate-B | OS-L1 + GUI-L2（#108 接続時） | open |
| SV-CTL-02 | #122 | Synvelis | SV-SF-02 | SV-CTL-04 と並行 | Gate-C | OS-L1 | open |
| SV-CTL-03 | #100（PC-C3） | Synvelis | — | SV-CTL-04 の start | Gate-C 補助 | OS-L1 | open |
| SV-CTL-04 | #101（PC-C4） | Synvelis | SV-CTL-03 + Ruviform#307 | SF/CTL 統合 Gate | Gate-C | OS-L1 | open |
| SV-SF-04 | #107 | Synvelis | SV-SF-03 | SV-SF-05 の int | Gate-B | OS-L1 | open（headless） |
| SV-SF-05 | #108 | Synvelis | SV-SF-01 + SV-GUI-02(#111) | SV-SF-04 probe 接続 | Gate-B + GUI-L2-WIN（該当 feature） | OS-L1 + GUI-L2-WIN | open（read-only UI） |
| SV-SF-06 | #109 | Synvelis | —（並行開発） | 同一 Scenario/Run ID 共有 | Gate-B 製品 Oracle | OS-L1 + GUI-L2-WIN | open（Oracle は Acceptance のみ） |
| SV-CTL-05 | #102（PC-C5） | Synvelis | — | SV-CTL-02 と Gate-C | Gate-C | OS-L1 | open |
| SV-CTL-06 | #103（PC-C6 Oracle） | Synvelis | — | AUT 製品受入の acc | Gate-C 製品 Oracle | OS-L1 | open |
| SV-CTL-07 | #98（PC-C1） | Synvelis | — | SV-AUT-02 の start | Gate-C 補助 | OS-L1 | closed |
| SV-AUT-02 | #132 | Synvelis | SV-CTL-07 | SV-AUT-03 の start | Gate-C | OS-L1 | open |
| SV-AUT-03 | #133 | Synvelis | SV-AUT-02 | — | SV-CTL-06(#103) PASS が acc 条件 | OS-L1 | open |
| SV-AUT-01 | #131 | Synvelis | —（独立並行） | — | Gate-C 補助 | OS-L1 | open |
| SV-AUT-04 | #134 | Synvelis | —（独立並行） | — | Gate-C 補助 | OS-L1 | open |

### Recording lane

| Node | Issue | Owner | Start | Integration | Acceptance | OS Level | 状態 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| SV-RCD-01 | #115 | Synvelis + Ruviform#187/#251 | — | SV-RCD-03 の start | EXT-L3-WIN（実 Device・提供範囲） | OS-L1 + EXT-L3-WIN | open |
| SV-RCD-02 | #116 | Synvelis | — | SV-RCD-04 の start | Gate-A 補助 | OS-L1 | open |
| SV-RCD-03 | #117 | Synvelis | SV-RCD-01 + SV-SF-03 | — | EXT-L3-WIN（headless/monitor + 実機） | OS-L1 + EXT-L3-WIN | open |
| SV-RCD-04 | #118 | Synvelis | SV-RCD-02 | — | Gate-A 補助 | OS-L1 | open |
| SV-RCD-F01 | #119 | Synvelis | — | — | 初期 Gate 非必須（後続） | Deferred | open（future） |
| SV-RCD-F02 | #120 | Synvelis | — | — | 初期 Gate 非必須（後続） | Deferred | open（future） |

### Time/MIDI/Plugin lane

| Node | Issue | Owner | Start | Integration | Acceptance | OS Level | 状態 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| SV-TIME | #19 + docs sv#19 | Synvelis + Ruviform#189 | — | SV-TIM-01 の start | Gate-A/B/C 補助 | OS-L1 | docs chain 統合済み |
| SV-TIM-01 | #125 | Synvelis | SV-TIME | SV-TIM-02 の start | EXT-L3-PLUGIN 補助 | OS-L1 | slice 実装済み |
| SV-TIM-02 | #126 | Synvelis | SV-TIM-01 | — | EXT-L3-PLUGIN（実 MIDI backend） | OS-L1 + EXT-L3-WIN | open |
| SV-TIM-03 | #127 | Synvelis | — | PC-C4/SF-03/Ruviform#190/#253/#307 と int | EXT-L3-PLUGIN（実 Plugin Process precision） | OS-L1 + EXT-L3-WIN | open |
| SV-TIM-04 | #128 | Synvelis | — | Project/Render Snapshot + Plugin State | EXT-L3-PLUGIN 補助 | OS-L1 | slice 実装済み |
| SV-TIM-F01/F02 | #129/#130 | Synvelis | — | — | 初期 Gate 非必須（後続） | Deferred | open（future） |

### GUI Native lane（GPUI First）

| Node | Issue | Owner | Start | Integration | Acceptance | OS Level | 状態 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| SV-GUI-RF384 | Ruviform#384 | Ruviform | — | Ruviform#262 の start | GPUI 採否の前提契約 | OS-L1 | closed（写像時点） |
| SV-GUI-RF262 | Ruviform#262 | Ruviform | SV-GUI-RF384 | Ruviform#419 後継 ADR | GPUI 採用 Gate（OS 別実証） | OS-L1 + GUI-L2/EXT-L3（Ruviform 側 OS） | open |
| SV-GUI-RF419 | Ruviform#419 | Ruviform | — | #262 最終採否 | GPUI 採用 Gate | OS-L1 | open |
| SV-GUI-RF385〜390/#464 | Ruviform#385/#386/#387/#388/#389/#390/#464 | Ruviform | —（契約/PoC 並行） | SV-GUI-CORE/#176 の int | Ruviform 側 capability | OS-L1 | #385/#386/#387/#390/#464 closed、#388/#389 open（写像時点） |
| SV-GUI-RF313/#243/#532 | Ruviform#313/#243/#532 | Ruviform | — | GUI-L2-WIN driver/dock | GUI-L2-WIN 補助 | OS-L1 + GUI-L2-WIN | #313/#243 closed、#532 open（写像時点） |
| SV-GUI-LEGACY | #110/#111（旧 WebView） | Synvelis | — | 歴史的証拠（新 Native 受入を代替しない） | — | OS-L1 | closed（historical） |
| SV-GUI-01 | #175 | Synvelis | — | SV-GUI-CORE/#113 の start | GUI-L2-WIN 補助 | OS-L1 + GUI-L2-WIN | closed（写像時点） |
| SV-GUI-03 | #177 | Synvelis | — | SV-GUI-CORE/#113/#176 の start | GUI-L2-WIN 補助 | OS-L1 + GUI-L2-WIN | open |
| SV-GUI-CORE | #112 | Synvelis | SV-GUI-01 + SV-GUI-03 | SV-GUI-E2E(#114) の acc | GUI-L2-WIN（実 GPUI Window・入力→Receipt→Project/Undo/Save/Focus） | OS-L1 + GUI-L2-WIN | open |
| SV-GUI-04 | #113 | Synvelis | SV-GUI-01 + SV-GUI-03 | SV-GUI-E2E(#114) の acc | GUI-L2-WIN（2Window/Dock/Focus） | OS-L1 + GUI-L2-WIN | closed（写像時点） |
| SV-GUI-PLUGIN | #176 | Synvelis | — | Ruviform#386/#388+#389 →int | EXT-L3-WIN（実 VST3/CLAP/AU Native Dock） | OS-L1 + EXT-L3-WIN | open |
| SV-GUI-E2E | #114 | Synvelis | —（GUI scenario 並行） | SV-GUI-CORE/#113/#176/#177 + SV-SF-05(#108)+SV-CTL-05(#102) の該当 feature | GUI-L2-WIN 製品 E2E | OS-L1 + GUI-L2-WIN | open |

### Gateway / AI lane

| Node | Issue | Owner | Start | Integration | Acceptance | OS Level | 状態 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| SV-GW-00 | #44/#45 | Synvelis | — | SV-GW-01〜10 の start | — | OS-L1 | baseline/design 合意 |
| SV-GW-01〜10 | #46（GW-01〜10） | Synvelis | SV-GW-00 | — | AI-GATE（独立軸。Gate A/B/C を block しない） | OS-L1 | headless 実装済み |

### VCS / ALIB / OBS lane

| Node | Issue | Owner | Start | Integration | Acceptance | OS Level | 状態 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| SV-VCS-01 | #92 | Synvelis | — | {#93,#94} の start | VCS Cross Gate | OS-L1 | slice 進行 |
| SV-VCS-02/03 | #93/#94 | Synvelis | SV-VCS-01 | {#95,#96} の start | VCS Cross Gate | OS-L1 | slice 進行 |
| SV-VCS-04/05 | #95/#96 | Synvelis | SV-VCS-02/03 | SV-VCS-06(#97) の acc | VCS Cross Gate | OS-L1 | slice 進行 |
| SV-VCS-06 | #97 | Synvelis + ALIB#88 | SV-VCS-04/05 | — | VCS Cross Gate | OS-L1 | open |
| SV-ALIB-01 | #85 | Synvelis | — | {#86,#87} の start | ALIB Cross Gate | OS-L1 | slice 進行 |
| SV-ALIB-02/03 | #86/#87 | Synvelis | SV-ALIB-01 | {#88,#89,#90} の start | ALIB Cross Gate | OS-L1 | slice 進行 |
| SV-ALIB-04/05/06 | #88/#89/#90 | Synvelis | SV-ALIB-02/03 | SV-ALIB-07(#91) の acc | ALIB Cross Gate | OS-L1 | slice 進行 |
| SV-ALIB-07 | #91 | Synvelis | SV-ALIB-04/05/06 | — | ALIB Cross Gate | OS-L1 | open |
| SV-OBS | #81 + Ruviform#276/#277/#289/#291/#293 | Synvelis Host + Ruviform | Ruviform#276→#277→#289→#291/#293 | SV host subscriber/権限接続 | Gate-B 補助 | OS-L1 | #81 closed |

### CB / AUTX / RCP / CI lane

| Node | Issue | Owner | Start | Integration | Acceptance | OS Level | 状態 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| SV-CB-00 | #277 | Synvelis | — | SV-CB-01 の start | CB-GATE(#288) 前提 | OS-L1 | 設計合意 |
| SV-CB-01 | #278 | Synvelis | SV-CB-00 | {#279〜#288} の start | CB-GATE 前提 | OS-L1 | 設計合意 |
| SV-CB-02〜07 | #279〜#284 | Synvelis | SV-CB-01（Domain fixture/Gap 並行） | — | CB-GATE 前提 | OS-L1 | 設計合意 |
| SV-CB-08/09/10/11 | #285/#286/#287/#288 | Synvelis | SV-CB-01 | RF#562 PoC →int | CB-GATE(#288) | OS-L1 | 設計合意 |
| SV-AUTX-00 | #259 | Synvelis | — | {#260,#261} の start | — | OS-L1 | 設計合意 |
| SV-AUTX-01/02 | #260/#261 | Synvelis | SV-AUTX-00 | #261 auth が #262 mutating CLI の acc | AUTX-GATE | OS-L1 | 設計合意 |
| SV-AUTX-03 | #262 | Synvelis | SV-AUTX-01 | — | SV-AUTX-02(#261) auth 受入が acc | OS-L1 | 設計合意 |
| SV-AUTX-04 | #263 | Synvelis | —（並行） | — | AUTX-GATE | OS-L1 | 設計合意 |
| SV-AUTX-05/06 | #264/#265 | Synvelis | SV-AUTX-05→SV-AUTX-06 | — | AUTX-GATE | OS-L1 | 設計合意 |
| SV-AUTX-07/08 | #266/#267 | Synvelis | —（Mock Contract/E2E Fixture 並行） | — | AUTX-GATE | OS-L1 | 設計合意 |
| SV-RCP-00 | #289 | Synvelis | — | {#290〜#295} の start | RCP-07-GATE 前提 | OS-L1 | 設計正本 |
| SV-RCP-01〜06 | #290〜#295 | Synvelis + Ruviform#196/#198/#562 | SV-RCP-00（Fake Adapter/Typed Contract で並行） | RF#562/#196/#198 + SV-SF-03(#106)/SV-TIM-04(#128)/SV-CB-00(#277)/SV-MEDIA(#285) →int | RCP-07-GATE(#296) | OS-L1 | 設計正本 |
| SV-RCP-07 | #296 | Synvelis + Ruviform | SV-RCP-01〜06 | — | RCP-07-GATE（RT/PCM/3OS/Plugin/AudioLens 独立 Oracle） | OS-L1 + EXT-L3-WIN（該当機能） | 設計正本 |
| SV-RCP-F01 | #297 | Synvelis | — | — | 初期 Gate 非必須（後続 PoC） | Deferred | open（future） |
| SV-CI | #83 + Ruviform#219/#294 + SV#28 + AL#127/#142/#166 | CI | — | CROSS-CONSUMER-GATE | CROSS-CONSUMER（候補 SHA tuple） | OS-L1 | #83 closed、#294/#219/#28 open |

## Gate node 定義

| Gate | 判定内容 | 必須 OS Level | 独立 Oracle | Owner |
| --- | --- | --- | --- | --- |
| Gate-A（Core DAW） | Create/Edit/Record/Save/Reload/Playback/Export・PCM/Timing/Recovery | OS-L1（全 OS） | F0/E2E-L/R/X・VS-*・RCD headless・sv#17/#18 chain | Synvelis（#1・#8 Owner） |
| Gate-B（Signal Flow 診断） | configured/active/observed 区別・修正・診断 | OS-L1（全 OS） | SF-03/04/05・OBS・sv#18 chain | Synvelis #4/#109 + Ruviform probe |
| Gate-C（Control 双方向） | Source⇄Target 探索・適用状態確認・復帰 | OS-L1（全 OS） | PC-C2/C5・AUT・sv#20 chain | Synvelis #6/#102/#103 |
| AI-GATE | Intentional Gateway 3 Mode・Grant/Approval・AI Off 回帰 | OS-L1（全 OS） | gw10 harness・mcp-poc | Synvelis #44〜46 |
| GUI-L2-WIN | 実 GPUI Window・入力→Receipt→Project/Undo/Save/2Window/Focus・Native Dock 画素 | Windows のみ Required（macOS/Linux Deferred） | #313/#243 driver + #114 製品 E2E + 独立 Oracle | Synvelis #112/#114/#176 + Ruviform#313/#243 |
| EXT-L3-WIN | 実 Device 録音/MIDI・実 VST3/CLAP/AU・Native Dock | Windows のみ Required（macOS/Linux Deferred） | #115/#117/#126/#127/#176 + Ruviform#251/#253/#388/#389/#532 | Synvelis #115/#117/#126/#127/#176 + Ruviform |
| CROSS-CONSUMER | 候補 SHA tuple・AL f64/長尺・SV PCM/Project 非回帰 | OS-L1（対象 OS） | Ruviform#294 + SV#28 + AL#127 + 各 repo CI | CI（repo 別） |
| RCP-07 | Recipe/Cache/Branch/Plugin Missing/RT/VCS/3OS | OS-L1 + EXT-L3-WIN（該当機能） | RCP chain | Synvelis#296 + Ruviform |
| CB-GATE | #288 Cross-Gate | OS-L1 | CB chain | Synvelis#288 + Ruviform#562 |
| AUTX-GATE | capability action/CLI trusted issuer | OS-L1 | AUTX chain | Synvelis#259〜267 |
| SF-INTEGRATION-GATE | Active Binding/旧世代誤配送ゼロの統合 | OS-L1 | SF-03〜06 統合 Oracle | Synvelis#4/#109 |

## 候補 SHA tuple Registry

同一候補について次の tuple を相互参照可能にする:

```
candidate_tuple = (synvelis_sha, ruviform_resolved_sha,
                   os, arch, feature_set, oracle_set,
                   result_states)
```

| 追跡対象 | Issue | tuple 構成要素 | 現在状態 |
| --- | --- | --- | --- |
| GPUI 採用 | Ruviform#262 → #419 | ruviform SHA・OS 別 Window/Dock/Plugin Editor 実証・性能/互換 | NotRun（候補未確定） |
| Consumer 集約 | Ruviform#294 | candidate SHA・OS・Synvelis PCM/Project + AudioLens f64/CLI/RT の両 Consumer 独立 Oracle | NotRun |
| 製品 Domain Oracle | Synvelis#114/#109/#103 | synvelis SHA・OS・Scenario/Case ID・Expected/Observed・Pass/Fail/NotRun/NotRequired | NotRun |
| CI 追従 | Synvelis#28/#83・AL#127/#142 | repo 別 CI Run URL・対象 SHA・lane 結果 | 部分実績（#83 closed・Daily/手動 Full 運用中） |

ルール: 片 repo の成功を他 repo/他 Consumer へ転用しな
い。`NotRun`/`Fail`/`Unsupported` を `Pass` に読み替え
ない。Daily CI 未実行・進行中・未対象 SHA は `NotRun`
（実行中なら Pending）であり最終受入は保留。

## Fail-closed 判定

- node が Acceptance に達したと記録するには、対応 Gate
  の Required 項目が全て `Pass` かつ証拠（CI Run
  URL/実機ログ/Case ID）が必要。
- `NotRun`/`Fail`/`Unsupported` が残る限り当該 node は
  未受入。文書承認・計画確立は実装完了でも製品受入
  でもない。
- Browser/Fake/Headless/GPUI-simulated/Offscreen の
  成功は Native GUI・実 Plugin・実 Device の合格を意味
  しない。
