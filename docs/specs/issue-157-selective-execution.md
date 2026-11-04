---
id: "SPEC-034SYN157Kx9mTvQp2w"
type: "spec"
status: "approved"
title: "Issue #157 選択実行(selective execution)有効化 + Ruleset required contexts 移行計画"
created: "2026-11-05"
updated: "2026-11-05"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/157"
    - "docs/specs/issue-148-impact-classification.md"
  changes:
    added:
      - type: "file"
        path: "docs/specs/issue-157-selective-execution.md"
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
      - type: "file"
        path: "tools/ci-impact.mjs"
      - type: "file"
        path: "tools/ci-impact.test.mjs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-148-impact-classification.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/issue-131-aut-01-shadow-ci.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-148-impact-classification.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "tools/ci-impact.mjs"
    - "tools/ci-impact.test.mjs"
  defers: []
  deferred-by: []
---

# Issue #157 選択実行有効化 + Ruleset 整合仕様

## What Should Be Built

#148 で導入した分類・gate 機構の第 2 段階として、PR の lane 実行を
分類結果に従い選択化する。`classify` が出力する `lanes`(実行対象
lane id 一覧)を各 lane job の `if:` 条件へ接続し、分類で
`not-applicable` の lane を skip させる。`evidence` は従来どおり
期待集合 vs jobs API 実結果の fail-closed 照合を行い、skip された
leg を根拠付き `NotApplicable` としてのみ受理する。`push: main` /
`workflow_dispatch` は full mode を維持する。

## Why Now

- macOS arm64 runner 逼迫が実害化: PR #159 (AUT-01) の shadow 観測で
  macos leg が 20–25 分 queue 滞留後に cancel された
  (`docs/impl/exp/issue-131-aut-01-shadow-ci.md`)。
- Shadow 実績: PR #156 導入以降、#158・#159 で selective 判定の
  偽陰性 0 を観測(not-applicable leg が実際に失敗した事例なし)。
- owner 判断により有効化を前倒し。即時 rollback 可能な形で導入し、
  監視(shadow 照合表・runner time 行)は evidence 証跡で継続する。

## Design

### lanes 出力と lane `if:` 配線

- `tools/ci-impact.mjs classify` は各 lane leg に `scheduled` フラグを
  付与する: `mode !== "selective" || expectation === "required"`。
  `--lanes-output` / `CI_IMPACT_LANES_OUTPUT` 指定時、scheduled leg の
  lane id 集合(重複除去・sort)を `lanes` として GITHUB_OUTPUT へ書く。
- `classify` job output `lanes` を各 lane job(`verify`/`rt-verify`/
  `gw10-verify`/`msrv-verify`/`ui-verify`/`dep-guard`)が
  `needs: classify` 経由で消費:
  `if: needs.classify.result == 'success' && contains(fromJSON(needs.classify.outputs.lanes), '<lane-id>')`
- job-level `if` は `matrix` context を参照できない(GitHub 仕様・
  actionlint で検証済み)ため条件は lane 単位。分類の期待値も lane
  単位で全 OS leg 同一(`laneReasons` が lane-scoped)のため等価。
- `classify` 失敗時は `needs.classify.result != 'success'` で全 lane が
  skip され、`evidence`(`if: always()`)が missing report として fail
  — fail-closed。
- mode 解決: `pull_request` → `selective`(既定)、`push` /
  `workflow_dispatch` → `full`。repository variable
  `CI_SELECTIVE_DISABLE=1` で PR を `shadow`(全 lane 実行)へ戻す。

### Gate 契約(不変)

- selective mode では required leg のみ Pass 要求。not-applicable leg の
  `skipped`/`not-run` は合法、`fail`/`cancelled` は偽陰性 violation。
- shadow/full では全 leg が required — いかなる `skipped` も violation。
- run 内の未知 job は violation(lane universe は ci.yml に対して total)。
- `evidence` が唯一の安定的 required context: needs に skip 済み lane が
  あっても `if: always()` で必ず実行される。
- jobs API 収集は bounded retry 付き: leg の conclusion 遷移中に API が
  job エントリを一時欠落して返す eventual consistency を観測済み
  (本 PR 初回 run で成功済み leg が absent と報告された)。expected
  件数未満の間は retry し、最終的に不足なら従来どおり fail-closed。

## Ruleset 移行計画(coordinator 実施)

現行 `main-protect` ruleset(ruleset id 26530045)の
`required_status_checks` は leg 個別の 10 contexts:

```
verify (ubuntu-latest), verify (windows-latest), verify (macos-latest),
gw10-verify (ubuntu-latest), gw10-verify (windows-latest),
gw10-verify (macos-latest), msrv-verify, rt-verify, dep-guard, evidence
```

**提案: required contexts を `evidence` 単一へ移行する。**

- `evidence` は全 lane leg の期待集合照合を fail-closed で行う集約
  gate であり、required leg の skip・fail・cancelled・absent を全て
  violation とする。leg 個別 required の肩代わりを内部で完全に行う。
- 移行順序と無保護期間:
  1. 本 PR を merge(本 PR 自体は CI/toolchain 変更で fallback full CI
     → 全 lane 実行 → 旧 10 contexts 全て報告され現 ruleset で merge 可能)。
  2. coordinator が ruleset API で required_status_checks を
     `["evidence"]` に更新。
  3. 後続 PR から selective 実行が実効化。
  移行期間中も `evidence` は常時 required のまま変わらず、かつ gate が
  leg 集合を網羅照合するため required 欠落の無保護期間は生じない。
- selective 有効化後も旧 leg contexts は各 run で報告される(skip 時は
  `skipped` conclusion)ため、万が一 ruleset 更新が遅れても GitHub
  意味論上 skipped は required check を満たす。保護強度は evidence
  gate が担保する。

## Rollback

優先度順・いずれも即時性あり:

1. **Kill switch(推奨・最速)**: repository variable
   `CI_SELECTIVE_DISABLE=1` を Settings > Variables に設定。以後の
   `pull_request` run は classify が `shadow` mode となり `lanes` が
   全 lane を含むため lane `if:` が全て true → 全 lane 実行。
   `0`/削除で selective に復帰。コード変更・ruleset 変更不要。
2. **本 PR の revert**: `git revert <merge commit>` で配線自体を除去。
   ruleset が `evidence` 単一のままなら gate が full 期待集合を照合し
   全 lane 必須のまま動作する。
3. **Ruleset 復旧(必要時のみ)**: coordinator が required_status_checks
   を旧 10 contexts へ戻す。selective を止めるだけなら 1 で足り、
   通常不要。

## Monitoring

- `evidence` job summary の **Shadow comparison** 表は selective 有効化
  後も常設。not-applicable leg が万が一実行され失敗した場合
  `FALSE NEGATIVE` 行として記録され gate fail となる。
- 同 summary の `Total lane runner time` 行で before/after の runner
  時間を継続観測する。
- selective 実効証(実際に lane が skip される PR)は本 PR 以降の後続
  PR の evidence 証跡で確認する前提とする — 本 PR 自体は
  CI/toolchain 変更のため fallback で全 lane 実行される。

## Scope

### In Scope

- `tools/ci-impact.mjs`: `scheduled` フラグ・`lanes` 出力・mode 配線。
- `ci.yml`: classify `lanes` output・mode 解決の selective 化・
  `CI_SELECTIVE_DISABLE` kill switch・各 lane job の `needs`/`if:`。
- `tools/ci-impact.test.mjs`: scheduling 契約の pin。
- 本仕様書(ruleset 移行計画・rollback・監視の記録)。

### Out of Scope

- Ruleset の実変更(API 更新は coordinator が PR merge 後に実施)。
- leg 単位(OS 単位)の部分実行(job-level `if` の matrix 制約により
  現行機構では表現不能。必要になれば per-OS job 分割等の別設計)。
- 新規 lane の追加・既存 lane 内容の変更。

## Acceptance Criteria

- [ ] `node --test tools/ci-impact.test.mjs` pass(scheduling 契約を含む)。
- [ ] `mise run lint-actions` pass。
- [ ] PR で classify が `lanes` output を発行し selective mode で動作。
- [ ] RT strict-prefix/zero-test ガード・GW-10 harness・MSRV lane は
      分類で影響なしと証明された場合のみ skip 可能(分類規則不変)。
- [ ] `push: main` / `workflow_dispatch` は full mode で全 lane 実行。
- [ ] ruleset 移行提案(10 contexts → `evidence` 単一)が PR 本文に明記。
- [ ] rollback 手順(kill switch + revert)が本書に記録済み。

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/157
- 前提仕様: `docs/specs/issue-148-impact-classification.md` §安全移行
- Shadow 証跡: `docs/impl/exp/issue-131-aut-01-shadow-ci.md`(#159)、
  #156 導入 PR・#158(mcp-poc control channel)の各 evidence run
- 汎用契約 Owner: https://github.com/scarecrow173/ruviform/issues/333
