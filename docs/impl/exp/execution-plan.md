---
id: "EXP-034X84vNqRtYsWuIoPaQbC"
type: "experiment"
status: "completed"
title: "Execution Plan — Issue #23 横断 DAG 文書化と evidence 台帳の試行"
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
  implemented-by:
    - "docs/impl/ir/execution-plan.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/execution-plan-design.md"
    - "docs/specs/execution-plan.md"
    - "docs/designs/synvelis-driven-ruviform-target-architecture-v07.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/tasks/execution-plan-task.md"
    - "docs/test-specs/execution-plan.md"
    - "evidence/sv23/execution-plan-gate-ledger.md"
    - "evidence/sv23/execution-plan-entry-gate-ledger.md"
  defers: []
  deferred-by: []
---

# Execution Plan — Issue #23 横断 DAG 文書化と evidence 台帳の試行

## 問題設定

Issue #23 の Synvelis Execution Plan v1.0（F0・E2E-L/R/X・
横断依存・Acceptance Gate）を、RCP-00〜07/F01・Signal
Flow（#104〜109）・Automation/Control（#98〜103・
#121/#122・#131〜134）・GUI（#110〜114・#175〜177 +
Ruviform #385〜390/#464/#313/#243/#262/#419）・Recording
（#115〜120）・Gateway（#44〜46）・Time/MIDI/Plugin
（#125〜130）・VCS/ALIB/OBS/CB/AUTX/RCP の全
workstream を含む循環のない単一 DAG として文書化し、
共通/Consumer Owner・Start/Integration/Acceptance・
OS-L1/GUI-L2/EXT-L3 の検証レベル・`evidence/sv23/` 台帳
を確立する必要があった。本 slice は docs のみで crate・
CI・Cargo/mise を変更しないため、(a) 7 文書鎖の生成、
(b) Issue #23 本文の DAG 記述の edge 型付き写像、
(c) doc graph/index/review の整合確認が試行対象となった。

## 試行と結果

### E1: Issue #23 本文 DAG の edge 型写像

Issue #23 本文の「横断依存正規化 v1.1」「GUI lane
ordered DAG」「Stage 3 Start/Integration/Acceptance
分離」「RCP/CB/AUTX 追加 DAG」を抽出し、各依存を
`start`/`integration`/`acceptance` の 3 型へ分類した。
Start は「契約/型/Fixture が利用可能」、Integration は
「機能接続時の capability」、Acceptance は「最終受入
Gate の独立 Oracle/実機証拠」と定義し、参照先 Issue の
全 Close を Start 条件にしない規則を確認した（例:
#103 Oracle PASS は #133 の acceptance 条件であり
start 条件ではない）。

**結果**: 全 lane（F0/E2E・SF/CTL・Recording・TIM・
GUI・GW・VCS/ALIB/OBS・CB/AUTX・RCP・CI）について
edge 表を `docs/designs/execution-plan-design.md` に
固定できた。Gate node は Start edge を持たず
Acceptance edge のみを受ける構造とし、Acceptance→
Start の逆流を禁止した。

### E2: OS 検証レベルと判定語彙の分離

`POLICIES.md` の OS-L1/GUI-L2/EXT-L3 と Required/
Deferred/NotApplicable・Pass/Fail/NotRun/Unsupported
の規範を、Issue #23 の「Windows L2/L3 必須（利用機能
のみ）・macOS/Linux L1 のみ Deferred・Browser/Fake 成功
を Native 合格にしない」要求へ写像した。要求状態と
実行結果を別軸・別欄で記録する schema を
`evidence/sv23/` 台帳へ適用した。

**結果**: `gpui-test`/`webview`/Fake Driver/Headless/
Browser の成功を GUI-L2/EXT-L3 の Pass にしない規則、
macOS/Linux の L2/L3 Deferred 規則、未利用
Device/Format を要求しない規則を test-spec
EP-TS-R03 として固定できた。

### E3: 正規入口と候補 SHA tuple の台帳化

`mise.toml` の正規入口（`check-ruviform-rev`/
`check-headless-deps`/`check`/`lint`/`test`/`build`/
`test-rt`/`gw10`/`check-app-native`/`lint-app-native`/
`run-app-native`/`check-app-gpui` 系/`check-ui` 系/
`check-app-webview` 系/`*-full`/`test-recording-real-*`/
`ci-summary-gate` 系/`lint-actions`/`test-msrv`）を
OS Level・Gate/Feature・対象 SHA・結果状態へ対応づけ、
`evidence/sv23/execution-plan-entry-gate-ledger.md` に
台帳化した。#262/#419・#294（Ruviform）・#114/#109/
#103（Synvelis）の候補 SHA tuple registry を
`execution-plan-gate-ledger.md` に配置した。

**結果**: 未実施入口は `NotRun`/`Unsupported`/
`Deferred` として記録され、`Pass` は実測成功のみ。
本 slice で実際に実行した検証は
`evidence/sv23/execution-plan-verification.md` に記録。

### E4: 文書鎖生成と index/relation 整合

`new_spec`/`new_design`/`new_plan`/`new_task`/
`new_test_spec`/`new_impl_record`/`new_experiment_log`
の scaffold で文書を生成し、front matter の
`source`/`depends-on`/`related`/`derives-from`/
`derived-by`/`implements`/`verifies`/`references` を
相互に整合させた。IR は `docs/impl/ir/execution-plan.md`、
EXP は `docs/impl/exp/execution-plan.md` として scope の
file map に合わせた。`doc_maintenance.js index apply`
で触った dir の index を再生成した。

**結果**: 7 文書 + evidence/sv23 3 件が存在。index は
生成物として再生成。`doc_status lint`/`audit_impl_record`/
`audit_docs`/`route_graph`/`review-protocol`/`fmt-check`
の結果は `evidence/sv23/execution-plan-verification.md`
に記録。

## 採用した判断

- Issue #23 本文の DAG 記述を一つの非巡回 DAG として
  統合し、node は lane 単位・Issue 番号併記で粒度を抑え、
  詳細は個別 docs chain/Issue に委譲した（DAG 肥大化
  の回避）。
- 「Pass(部分)」に相当する中間状態を導入せず、要求
  状態と実行結果の 2 軸 + 計画参照 `NotRequired` のみ
  を採用した（POLICIES.md 語彙との一致を優先）。
- sv#21（Media/Render）・sv#22（Target Architecture）
  並行 Worker の文書は編集・作成せず参照のみとし、
  path/内容競合を回避した。
- 実装/Gate の実行は本 slice の対象外とし、台帳の
  結果状態は `NotRun`/`Deferred` を誠実に記録した。
  文書承認を実装完了・製品受入と混同しない規則を
  test-spec Guarantees に明記した。

## 棄却した案

- 全 Issue を個別 node 化する案: node 数が過大となり
  DAG の可読性を損なうため、lane node + Issue 番号併記
  を採用した。
- 新規 CI Workflow・専用 Runner の追加: Issue #23 の
  「新 CI Workflow を増設しない」要求および EP-R-09 に
  反するため棄却。既存 Daily（変更検知）+ 手動 Full CI
  のみを利用する。
- headless/Fake Pass を GUI-L2/EXT-L3 Pass へ繰り上げる
  案: POLICIES.md と Issue #23 の capability 分離要求に
  反するため棄却。
