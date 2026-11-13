---
id: "SPEC-034X83uiIsH0n6OuQGcU1T"
type: "spec"
status: "approved"
title: "Execution Plan — RCP-00〜07/F01・F0/E2E-L/R/X・Signal Flow/Automation/GUI 横断単一 DAG・Owner/Gate/Acceptance Matrix"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/23"
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
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
    - "docs/designs/recording-take-comping-design.md"
    - "docs/designs/automation-modulation-design.md"
    - "https://github.com/scarecrow173/synvelis/issues/23"
    - "https://github.com/scarecrow173/synvelis/issues/1"
    - "https://github.com/scarecrow173/synvelis/issues/22"
  derived-by:
    - "docs/designs/execution-plan-design.md"
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/execution-plan.md"
    - "evidence/sv23/execution-plan-gate-ledger.md"
    - "evidence/sv23/execution-plan-entry-gate-ledger.md"
  defers: []
  deferred-by: []
---

# Execution Plan — RCP-00〜07/F01・F0/E2E-L/R/X・Signal Flow/Automation/GUI 横断単一 DAG・Owner/Gate/Acceptance Matrix

## What Should Be Built

Issue #23 の canonical 実行計画: Synvelis Execution Plan v1.0
（F0・E2E-L/R/X・横断依存・Acceptance Gate）を、RCP-00〜07/F01
（Render Checkpoint Workstream）、Signal Flow（#104〜109）、
Automation/Control（#98〜103・CTL/AUT/PC 系）、GUI
（SV-GUI/SV-GPUI 系・Ruviform #385〜390/#464/#313/#243）、
Recording（#115〜118）、Gateway（#44〜46/GW-01〜10）、Time/
MIDI/Plugin（#19/#20/#21/#22 系）、VCS/ALIB/OBS/CB/AUTX の
全 workstream を含む**循環のない単一 DAG** として文書化し、
各 node の Start / Integration / Acceptance を分離した
Owner/Gate/Acceptance Matrix と、OS-L1/GUI-L2/EXT-L3 の
検証レベル別受入条件、`evidence/sv23/` の証拠台帳を
確立する。

本 slice は **docs/ 設計契約 + evidence 台帳のみ**。実装・
CI workflow 変更・Cargo/mise 変更・Issue の Close は行わ
ない。各 workstream の実装 Owner は個別 Issue が担い、本
計画は着手条件・統合・最終受入の判定枠組みを固定する。

## Why Now

- Issue #23 は実行/製品 Gate の横断 Owner だが、workstream
  ごとの docs chain（sv#10/#11/#12/#17〜#20 等）は個別に
  成立しており、横断の単一 DAG・Owner/Gate/Acceptance
  Matrix・OS 別検証レベルの正本が未整備。
- 「Start（着手必須依存）」「Integration（統合接続時）」
  「Acceptance（最終受入）」を混同すると、無関係な
  GUI-L2/EXT-L3 最終 Pass 待ちで Headless/RT 実装が停止
  したり、逆に headless Pass を実機受入へ誤昇格したりする
  リスクがある（POLICIES.md §4/§5）。
- Windows で実際に提供・利用する GUI/Device/Plugin の
  GUI-L2/EXT-L3 は Required であり、macOS/Linux は OS-L1
  のみ必須。要求状態（Required/Deferred/NotApplicable）
  と実行結果（Pass/Fail/NotRun/Unsupported）を別軸で
  管理する台帳が必要。
- #262/#419（GPUI 採用）・Ruviform #294（Consumer 集約）・
  SV #114/#109/#103（製品 Domain Oracle）が同一候補
  SHA/OS/Feature で照合できる証拠連携が必要。新 CI
  Workflow は増設しない。

## Users and Value

- Coordinator/Worker: どの workstream が今着手可能か
  （Start）、何が揃えば統合できるか（Integration）、何が
  残れば受入か（Acceptance）を循環なく判定できる。
- Reviewer: Owner 分離（Synvelis 製品 Domain vs Ruviform
  共通基盤 vs AudioLens Consumer vs CI）と evidence の
  誠実さ（NotRun/Deferred/Unsupported を Pass にしない）
  を一意に確認できる。
- 実装 Owner（#104〜118・#125〜128・#175/#176/#177・
  #289〜297・GW/VCS/ALIB/CB/AUTX 各 Issue）: 自分の
  着手条件と最終受入 Gate、自分が依存する共通基盤能力を
  認識できる。

## Scope

### In Scope

- RCP-00〜07/F01（synvelis#289〜297）・F0（#13 VS-A）・
  E2E-L1/L2（#24/#25）・E2E-R1〜R3（#14〜16）・E2E-X
  （#26）を含む全 workstream の横断単一 DAG（node・
  edge・循環禁止）。
- 各 node の Start / Integration / Acceptance の 3 相分離
  と、共通基盤（Ruviform）Owner・Synvelis 製品 Domain
  Owner・AudioLens Consumer・CI Owner の分離。
- OS 検証レベル matrix: OS-L1（Windows/macOS/Linux 必須）
  ・GUI-L2（Windows の利用 GUI 機能のみ必須）・EXT-L3
  （Windows の利用 Device/Plugin のみ必須）・macOS/Linux
  の GUI-L2/EXT-L3 Deferred 規則。
- 判定語彙の固定: 要求状態 `Required`/`Deferred`/
  `NotApplicable` と実行結果 `Pass`/`Fail`/`NotRun`/
  `Unsupported`（+ 計画上の `NotRequired`）の分離。
  Browser/Fake/Headless/GPUI-simulated の成功を Native/
  実機合格にしない。
- `evidence/sv23/` への台帳化: 正規入口（`check-ruviform-
  rev`/`check-headless-deps`/`check`/`lint`/`test`/
  `test-rt`/`gw10`/`check-app-native`/`lint-app-native`/
  `check-app-gpui`/`test-app-gpui`/`check-ui`/`test-ui`/
  `build-ui`/`*-full`/`test-recording-real-*`/`ci-summary-
  gate` 等）と対象 SHA/Gate/OS/Feature の対応づけ。
  未実施は `NotRun`/`Unsupported` として記録。
- 同一候補 SHA/OS/Feature/結果の tuple 追跡:
  Ruviform #262/#419（GPUI 採用）・#294（Consumer 集約）・
  Synvelis #114/#109/#103（製品 Domain Oracle）。
- Windows L2/L3 を「実際に提供する機能の最終受入 Gate」
  とし、並行実装可能な Slice と区別する規則。旧
  WebView/SolidJS Shell の完了や L2/L3 実機の全完了を
  Headless/RT の着手 Hard Blocker にしない。
- 既存 Daily Full CI（変更検知）+ 手動 Full CI の利用。
  新規 CI Workflow・PR 必須 Full CI・選択実行機構の
  増設を行わない。

### Out of Scope

- 実装コード・`Cargo.toml`/`Cargo.lock`/`mise.toml`/
  `.github/workflows/` の変更。各 workstream の実装は
  個別 Issue の責務。
- Issue の Close・受入判定の実施・製品方針の新規決定
  （数値 SLO・OS サポート宣言・互換性破壊は別管理）。
- `docs/specs|designs|...` の sv#21（Media/Import/Export/
  Offline Render）・sv#22（Target Architecture）並行
  Worker の文書の編集・作成（参照のみ）。
- Ruviform/AudioLens リポジトリの変更。共通基盤の不足は
  upstream Issue として記録し、Synvelis 側で恒久的に
  重複実装しない。
- 個別 workstream の詳細設計・fixture・oracle の再定義
  （各 docs chain が正本）。本計画はそれらへの写像と
  Gate 判定枠組みを提供する。
- MSRV 系タスクの実行・F: ドライブへの中間生成物の配置
  （ユーザー指定により本 slice では行わない）。

## Requirements

- **EP-R-01 単一 DAG**: RCP-00〜07/F01、F0/E2E-L/R/X、
  Signal Flow（#104〜109）、Automation/Control
  （#98〜103・#121/#122・#131〜134）、Time/MIDI/Plugin
  （#19〜#22・#125〜130）、Recording（#12・#115〜120）、
  GUI（#110〜114・#175〜177・Ruviform #385〜390/#464/
  #313/#243/#262/#419）、Gateway（#44〜46・GW-01〜10）、
  VCS（#92〜97）、ALIB（#85〜91）、OBS（#81・Ruviform
  OBS 系）、CB（#277〜288・Ruviform #562）、AUTX
  （#259〜267）、Project/Playback/Editing/Workspace
  （#2/#4〜#11/#17/#18）を node とする循環のない単一
  DAG を `docs/designs/execution-plan-design.md` と
  `evidence/sv23/` に確立する。edge は Start /
  Integration / Acceptance の 3 相に型付けし、同一 node
  間の循環を禁止する。
- **EP-R-02 Start/Integration/Acceptance 分離**: 各
  node の着手必須依存（Start）は明示的な「着手条件」
  のみとし、Integration（統合接続時の依存）と
  Acceptance（最終受入 Gate）を別欄・別 Gate で管理する。
  参照先 Issue の全 Close を着手条件にしない。
- **EP-R-03 Owner 分離**: 各 node・Gate に Owner を
  `Synvelis 製品 Domain` / `Ruviform 共通基盤` /
  `AudioLens Consumer` / `CI/証拠` のいずれかで一意に
  割当て、共通/Consumer の責務を混同しない。重複
  Runner・重複 Owner を導入しない。
- **EP-R-04 OS 検証レベル**: OS-L1（Native Build/Link・
  Headless/RT/CLI/Feature・独立 Oracle）は Windows/
  macOS/Linux の既存 Daily/手動 CI で対象 SHA を実走。
  GUI-L2（実 Window/入力/Focus/表示）と EXT-L3（実
  Device/実 Plugin/Native Dock）は Windows で実際に
  提供・利用する機能に限り Required とし、macOS/Linux
  では利用要件発生まで Deferred とする。Job 定義・Skip
  の存在を Pass とみなさない。
- **EP-R-05 判定語彙分離**: 要求状態（`Required`/
  `Deferred`/`NotApplicable`）と実行結果（`Pass`/`Fail`/
  `NotRun`/`Unsupported`、計画参照には `NotRequired`）を
  別軸で記録する。環境不在・CI 未実行・Skip/ignored・
  Feature 無効は `NotRun` であり `Pass`/`Unsupported`
  ではない。Browser/Fake/Headless/GPUI-simulated/Offscreen
  の成功は Native GUI・実 Plugin・実 Device の合格を
  意味しない。
- **EP-R-06 Evidence schema**: 各 Gate/Case の証拠に
  Repo/SHA（Consumer の場合 Ruviform resolved SHA）・
  OS/Arch・Feature/テスト名・期待/観測結果・実行環境・
  CI Run URL/実機ログを記録する。`evidence/sv23/` の
  台帳はこの schema に従う。
- **EP-R-07 候補 SHA tuple 追跡**: Ruviform #262/#419
  （GPUI 採用 Gate/後継 ADR）・Ruviform #294（Consumer
  集約 candidate SHA）・Synvelis #114/#109/#103（製品
  Domain Oracle）が、同一の候補 SHA/OS/Feature/結果
  tuple で相互参照できること。片方の結果を他方へ転用
  しない。
- **EP-R-08 正規入口と Gate の対応**: `mise.toml` の
  正規入口（`check-ruviform-rev`・`check-headless-deps`・
  `check`/`lint`/`test`/`build`（package 引数必須）・
  `test-rt`・`gw10`/`gw10-report`・`check-app-native`・
  `lint-app-native`・`build-app-native`・`run-app-native`・
  `check-app-gpui`/`lint-app-gpui`/`test-app-gpui`・
  `check-ui`/`test-ui`/`build-ui`/`check-app-webview` 系・
  `*-full`/`verify-full`・`test-recording-real-input`/
  `test-recording-real-fs`/`lint-recording-real-input`・
  `ci-summary-gate`・`test-ci-summary`・`test-ci-tracker-
  entry`・`lint-actions`）を、対象 OS・Gate・Feature 範囲
  へ対応づけ、`evidence/sv23/` に台帳化する。未実施は
  `NotRun`/`Unsupported`。
- **EP-R-09 CI 増設禁止**: 既存の変更検知付き Daily Full
  CI と手動 Full CI を利用し、新規 Workflow・PR 必須
  Full CI・選択実行機構・RCP/AUTX 専用 CI を追加しない。
  CI に実装されていない検証を「委譲済み」と扱わない。
- **EP-R-10 並行可能 Slice と最終受入 Gate の区別**:
  旧 WebView/SolidJS Shell の完了・L2/L3 実機の全完了を
  Headless/RT/Domain の着手 Hard Blocker にしない。
  Windows で利用する機能の最終受入 Gate（GUI-L2/EXT-L3）
  と、並行して実装可能な Slice を DAG 上で区別する。
- **EP-R-11 文書鎖の正本性**: 本 spec→design→plan→
  task→test-spec→impl(ir/exp) の文書鎖と `evidence/sv23/`
  台帳が Issue #23 の横断計画の正本であり、個別
  workstream docs chain（sv#10/#11/#12/#17〜#20・
  sv#21/#22 並行分を含む）を置換せず参照関係で接続する。
- **EP-R-12 Fail-closed な進捗判定**: DAG の node が
  Acceptance に達したと記録するには、対応 Gate の
  Required 項目が全て `Pass` である証拠（CI Run URL/
  実機ログ/Case ID）が必要。`NotRun`/`Fail`/
  `Unsupported` が残る限り当該 node は未受入。

## Acceptance Criteria

- [ ] **EP-AC-01（Issue AC-1）**: RCP-00〜07/F01、F0/
  E2E-L/R/X、Signal Flow/Automation/GUI の
  Start/Integration/Acceptance が循環のない単一 DAG で
  管理され、共通/Consumer の Owner と着手条件が分離され
  ている（design の DAG + evidence/sv23 gate ledger で
  検証）。
- [ ] **EP-AC-02（Issue AC-2）**: Windows/macOS/Linux の
  既存 Daily/手動 CI で Native Build/Link、Headless/RT/
  Feature、AL f64/CLI・SV PCM/Project の独立 L1 Oracle
  と 3 repo 候補 SHA が `evidence/sv23/` に記録される
  仕組みが存在する（未実施分は NotRun/Unsupported）。
- [ ] **EP-AC-03（Issue AC-3）**: Windows L2 必須として
  #112/#114 の DAW 実 GPUI Window・入力→Receipt→
  Project/Undo/Save/2Window/Focus 等の使用機能を実 OS
  で受入し、#313/#243 と独立 Oracle を紐付ける Gate が
  DAG/Matrix に存在する。
- [ ] **EP-AC-04（Issue AC-4）**: Windows L3 必須として
  録音・MIDI・VST3/CLAP Native Dock 等を実際に提供する
  範囲で #115/#117/#126/#127/#176 と Ruviform
  #251/#253/#388/#389/#532 の実機/実 Plugin Gate が
  定義され、未実施ならその機能は未受入、未利用
  Device/Format を要求しない規則が明記される。
- [ ] **EP-AC-05（Issue AC-5）**: macOS/Linux は L1 のみ
  必須で、実 Window/実 Device/実 Plugin は対象 OS で
  実際に利用する要件発生時まで Deferred とし、未実測を
  Pass 扱いしない規則が明記される。
- [ ] **EP-AC-06（Issue AC-6）**: 旧 WebView/SolidJS
  Shell や L2/L3 実機の全完了を Headless/RT の着手
  Hard Blocker にせず、Windows で利用する機能の最終
  受入 Gate と並行実装可能な Slice を区別する規則が
  DAG に反映される。
- [ ] **EP-AC-07（Issue AC-7）**: #262/#419 GPUI 採用・
  #294 Consumer 集約・#114/#109/#103 製品 Domain
  Oracle で同一候補 SHA/OS/Feature/Fail/NotRun/
  NotRequired を追跡し、新 CI Workflow を増設しない
  規則が明記される。
- [ ] **EP-AC-08**: `evidence/sv23/` に正規入口と対象
  SHA/Gate の対応づけ台帳が存在し、各入口の OS/Gate/
  Feature 範囲と現在の結果状態（実測 Pass または
  NotRun/Unsupported/Deferred）が記録される。
- [ ] **EP-AC-09**: DAG が非循環であること（Start edge
  の到達可能性に矛盾がなく、Acceptance Gate が Start
  edge に逆流しない）を test-spec の規則で検証できる。

## Deferred Design Concerns

- 実 Window/実 Device/実 Plugin/実 Provider の L2/L3
  実測 | 本 slice は計画・台帳のみ | synvelis#114/#176・
  ruviform#313/#243/#251/#253/#388/#389/#532 | 実装・
  受入 Owner が headless/計画 Pass を実機受入と誤読
  する。
- macOS/Linux での GUI-L2/EXT-L3 | 利用要件発生まで
  Deferred（POLICIES.md） | 対象 OS での利用決定時に
  Required へ昇格 | 現時点では未実測を Pass にしない。
- Windows で「実際に提供・利用する」機能の確定範囲
  （どの Device/Plugin 形式・Host mode を製品が提供
  するか） | 製品提供範囲の確定は #176・#115・#117・
  #126/#127 の各 Issue と Ruviform capability Gate に
  委譲 | 範囲確定前に全組合せを Required にしない。
- AudioLens native f64/長尺の非回帰実測 | 型レベル維持・
  実測は Ruviform #294 candidate SHA Gate | synvelis#28・
  ruviform#219/#294・audio-lens#127/#142/#166 | f64
  consumer 経路の実測は別 Gate。
- RCP-F01（Remote Render Worker）・AUT-F 系・RCD-F 系・
  CTL-F 系・TIM-F 系の Future workstream | 初期 Gate
  非必須の後続 | synvelis#119/#120/#123/#124/#129/#130/
  #135〜139/#297 | 再始動条件（再現可能なユースケース・
  Owner 境界・最小契約・RT/精度 Budget・Fault/OS/
  Plugin fixture）が揃うまで Open のまま管理。

## Sources and Relations

- Issue #23 本文（Stage 3 Start/Integration/Acceptance
  分離・OS 別受入方針・横断依存正規化 v1.1・GUI lane
  ordered DAG・AUTX/RCP/CB 追加 DAG・判定台帳・
  dispatch 判断）。
- Issue #1（Gate A/B/C/AI 製品 Gate 定義）・Issue #22
  （Synvelis-driven Ruviform Target Architecture v0.7.1
  設計正本）・POLICIES.md（OS-L1/GUI-L2/EXT-L3・
  Required/Deferred/NotApplicable・Pass/Fail/NotRun/
  Unsupported の規範）。
- 上流 architecture baseline: `synvelis-driven-ruviform-
  target-architecture-v07.md`（#22）・`project-asset-
  persistence-architecture.md`（#17）・`playback-mixer-
  generic-execution-graph-architecture.md`（#18）・
  `configurable-ppqn-musical-time-architecture.md`
  （#19）・`time-transport-midi-automation-plugin-
  integration-architecture.md`（#20）・`media-import-
  export-offline-render-ui-diagnostics-architecture.md`
  （#21）・`recording-take-comping-design.md`（#12）・
  `automation-modulation-design.md`（#6）。
- 個別 workstream docs chain: sv#10（project-model）・
  sv#11（timeline-editing）・sv#12（recording-take-
  comping + evidence/sv12 owner matrix）・sv#17（project-
  persistence）・sv#18（playback-graph）・sv#19
  （musical-time）・sv#20（transport-integration）・
  sv#21/sv#22（並行 Worker — 参照のみ）。
- GUI 系 docs chain: `sv-gpui-01-native-app-shell.md`・
  `sv-gpui-03-product-theme-workspace-presets.md`・
  `sv-gui-03-05-gpui-scenario.md`・`svt-product-e2e.md`・
  `sv-gui-04-*`・`ci-os-l1-native-gpui.md`・
  `ci-synvelis-pr-push-regression-gate.md`・
  `issue-166-full-ci-ci-summary.md`。
- 関連 Issue: synvelis #1〜#12・#17〜#23・#28・#44〜#46・
  #81・#83・#85〜#97・#98〜#120・#121〜#134・#175〜#177・
  #259〜#267・#277〜#288・#289〜#297、ruviform #184〜#198・
  #219・#243・#251・#253・#262・#294・#305〜#314・#385〜
  #390・#419・#464・#562・#565、audio-lens #127/#142/#143/
  #166。
