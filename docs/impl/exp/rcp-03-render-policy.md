---
id: "EXP-rcp03-render-policy"
title: "RCP-03 Render Policy / Trigger / Async Job / Completeness 設計実験メモ (synvelis#292)"
created: "2026-11-14"
updated: "2026-11-14"
---

# 実験記録: RCP-03 render policy headless スライス（synvelis#292）

## E1 policy を patch の 3 層解決にする方式

**仮説**: Issue は `RenderPolicy {schema_version, triggers,
scope, quality_profile, completion, resource_budget, retention}`
と「project declared preference + user local override +
operation-specific explicit intent の順に解決、採用値と由来を
provenance へ記録」を要求する。各層を完全な policy 文書に
すると local override が 1 field 変えるだけでも全 field を
再記述する必要があり、provenance が自明化して「どの値を
採用したか」の記録として意味を失う。

- 案 A: 各層は完全な `RenderPolicy`。勝者層の全 field を採用。
  provenance は層名のみ → 「値の由来」ではなく「文書の由来」。
- 案 B（採用）: 各層は `PolicyPatch`（全 field `Option`）。
  field 毎に explicit > local > declared > default の順で
  最初の `Some` を採用し、`FieldProvenance{field, layer}` を
  記録する。default は fail-closed 側（`RequireComplete`・
  `max_concurrent_jobs: 1`）に倒す。

**観察**: `resolve_effective_policy` の単体的な性質として、
同一入力からは常に同一 `resolution_digest` が出るため、
policy versioned resolution を job key の構成要素にできる。

## E2 job key に action/grant を含める方式

**仮説**: RCP-POL-07 は GUI/Headless CLI/Workflow/Script が
「同じ Gateway Action/Grant に投影し、Target Revision と Job
Receipt が一致」することを要求する。entry surface を key に
含めると同一要求が surface 毎に別 job になり二重 work が
発生する。

- 案 A: key = snapshot locator + recipe set + purpose +
  policy resolution のみ。action/grant は receipt provenance
  のみ → 異なる grant が同一 job に coalesce し、認可境界を
  越えた結果共有が起きうる。
- 案 B（採用）: key に `action` + `grant` を追加し、entry
  surface は receipt provenance に留める。同一
  action+grant+target は surface を跨いで coalesce し、
  grant が異なれば別 job（認可 scope を key で分離）。

**観察**: spec 文言の "policy versioned resolution" を
`EffectivePolicy::resolution_digest`（解決済み policy +
provenance の canonical digest）として実体化した。

## E3 completeness を job 終端状態と直交させる方式

**仮説**: 「Published/Partial」と「Complete/Incomplete」の
関係が曖昧だと、不完全を Complete と表示する誤成功経路が
生まれる。

- 案 A: job state に Complete を含める → state と verdict が
  二重管理になり不整合の余地。
- 案 B（採用）: job state は実行結果のみ
  （Published=必要 recipe 全て publish 済、Partial=gap あり
  で終了、Failed/Cancelled は終端）。`CompletenessReport` は
  store の `lookup` 結果から独立に評価し、Share gate が
  `completion` policy（BestEffort→AllowedPartial /
  RequireComplete→Blocked{gaps,retry}）を適用する。
  verdict は job state から推測しない。

**観察**: `lookup` の `LookupOutcome`（Missing/Corrupt/
Stale/Unverified/Denied/Unknown/Unsupported/
EligiblePreview）を `CompletenessGap` の typed kind に
1:1 写像し、RCP-04 語彙と整合させた。

## E4 crash/resume を executor outcome としてモデル化

**仮説**: Failure Matrix の Crash は「publish 前に process
が死んだ」状況。staging は atomic なので commit 済み
artifact は汚れないが、job は非終端のまま残る。

- 案 A: `panic` を catch して resume — headless では再現性が
 なく test 不能。
- 案 B（採用）: `RenderExecution::Crashed` を executor の
  typed outcome とし、orchestrator は job を `Rendering` に
  残したまま `CrashObserved` receipt を記録。`resume(key)`
  が同じ key で render stage を再駆動する。publish は
  content-addressed + CAS なので再実行は冪等。

**観察**: `InMemoryArtifactStore` の fault injection
（`FaultPoint`）で publish 途中失敗も別途検証可能。
