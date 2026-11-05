---
id: "SPEC-034SYN157Kx9mTvQp2w"
type: "spec"
status: "superseded"
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
  superseded-by:
    - "docs/specs/issue-166-full-ci-ci-summary.md"
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

> **Superseded (2026-11-05)**: selective execution・`CI_SELECTIVE_DISABLE`
> ・shadow mode は Issue #166 stage 1 で撤去され、全 lane は全 trigger
> で毎回実行される。macOS 非ブロッキング境界は `evidence`(pr scope)に
> 継承され、Full CI の required 評価は `ci-summary`(full scope)へ移行。
> 現行仕様は `docs/specs/issue-166-full-ci-ci-summary.md` を参照。
> 本書は実装経緯の履歴として保持する。

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
  `gw10-verify`/`msrv-verify`/`ui-verify`/`dep-guard` + `*-macos`)が
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

### macOS non-blocking track(rev.2、Issue #148/#145 2026-11-05 合意)

macOS arm64 runner 逼迫が慢性化する現状では、macOS leg を required
に置くこと自体が PR 通過を runner 供給へ連動させる設計上の欠陥。
そこで required universe から macOS leg を**物理的に分離**する
(`continue-on-error` や結果無視ではなく job 分離):

- `verify`/`gw10-verify`/`ui-verify` の matrix は
  `ubuntu-latest`+`windows-latest` のみ。required context 名
  (`verify (ubuntu-latest)` 等)は不変。
- macOS は専用 job `verify-macos` / `gw10-verify-macos` /
  `ui-verify-macos` が lane と同一 `if:` 条件で継続実行する
  (PR/main 双方)。lane が schedule される限り macOS も走る — lane
  skip は「この変更に macOS 検証不要」の分類判断であり、runner 不足
  による不実行とは別物として区別される。
- `*-macos` job は `evidence` の `needs` に含めず、gate の期待集合
  (`LANE_LEGS`)にも含めない。代わりに `OBSERVED_LEGS` として gate が
  jobs API から実結果を収集し、Observed 表に
  Pass/Fail/Cancelled/Skipped/Pending(queued・in progress・API 欠落)
  を記録する。violation にはしないが `::warning` annotation で
  Fail/Cancelled を surface する — 実行済み失敗を隠さない。
- **正式リリース / Consumer 受入では候補 SHA の macOS 実測を必須**
  とする(runbook 運用項、§運用参照)— PR gate からの分離は macOS
  検証の放棄ではなく、ブロッキング境界の移動である。

### Gate 契約(不変)

- selective mode では required leg のみ Pass 要求。not-applicable leg の
  `skipped`/`not-run` は合法、`fail`/`cancelled` は偽陰性 violation。
- shadow/full では全 leg が required — いかなる `skipped` も violation。
- run 内の未知 job は violation(job universe は required legs +
  observed legs に対して total)。
- `evidence` が唯一の安定的 required context: needs に skip 済み lane が
  あっても `if: always()` で必ず実行される。
- required Linux/Windows/RT/GW-10/MSRV/UI leg の欠落・fail・skipped は
  fail-closed 判定を維持する(rev.2 で変えない)。
- jobs API 収集は bounded retry 付き: leg の conclusion 遷移中に API が
  job エントリを一時欠落して返す eventual consistency を観測済み
  (本 PR 初回 run で成功済み leg が absent と報告された)。expected
  件数未満の間は retry し、最終的に不足なら従来どおり fail-closed。

## Ruleset 状態(2026-11-05 適用済み)

required_status_checks は新 ruleset `main-checkstatus`
(ruleset id 26548278)に移行済みで、rev.2 設計に整合する **8 contexts**:

```
verify (ubuntu-latest), verify (windows-latest),
gw10-verify (ubuntu-latest), gw10-verify (windows-latest),
msrv-verify, rt-verify, dep-guard, evidence
```

- `strict_required_status_checks_policy` 有効、bypass は
  RepositoryRole(actor_id 5)が pull_request scope で保持。
- macOS leg は job 名自体が `*-macos` へ分離されたため、required
  context 名の変更・削除は発生していない(旧 10 contexts の macOS 2
  件は新 job への置換として解消)。
- `main-protect`(id 26530045)は status checks から切り離され、
  deletion/non_fast_forward/pull_request ルールを担う。
- ui-verify Linux/Windows leg は gate 上 required(分類で必要時)。
  ruleset context への追加は要否を owner 判断に委ねる — 追加する
  場合も gate 期待集合と名称が一致する(実装状況に応じて coordinator
  が判断)。

## Rollback

優先度順・いずれも即時性あり:

1. **Kill switch(推奨・最速)**: repository variable
   `CI_SELECTIVE_DISABLE=1` を Settings > Variables に設定。以後の
   `pull_request` run は classify が `shadow` mode となり `lanes` が
   全 lane を含むため lane `if:` が全て true → 全 lane 実行。
   `0`/削除で selective に復帰。コード変更・ruleset 変更不要。
2. **本 PR の revert**: `git revert <merge commit>` で配線自体を除去。
   ruleset は 8 contexts のまま有効 — gate が full 期待集合を照合し
   全 required lane 必須のまま動作する(macOS track の分離も revert
   で同時に戻るため、macOS leg を required 復帰させる場合は ruleset
   側の再追加が別途必要 — 3 参照)。
3. **Ruleset 復旧(必要時のみ)**: coordinator が macOS leg を required
   context へ戻す場合は `verify (macos-latest)` 等を再追加する
   (matrix への leg 復帰も revert で一緒に戻る)。selective を止める
   だけなら 1 で足り、通常不要。

## Monitoring

- `evidence` job summary の **Shadow comparison** 表は selective 有効化
  後も常設。not-applicable leg が万が一実行され失敗した場合
  `FALSE NEGATIVE` 行として記録され gate fail となる。
- 同 summary の **Observed macOS legs** 表で `*-macos` の
  Pass/Fail/Cancelled/Pending を継続観測する。Fail/Cancelled は
  `::warning` annotation でも surface される — 閾値を超える慢性
  Cancelled(runner 逼迫再燃)や Fail 連発は owner へエスカレーション
  対象とし、PR gate は通過し続ける。
- 同 summary の `Total lane runner time` 行で before/after の runner
  時間を継続観測する。
- selective 実効証(実際に lane が skip される PR)は本 PR 以降の後続
  PR の evidence 証跡で確認する前提とする — 本 PR 自体は
  CI/toolchain 変更のため fallback で全 lane 実行される。

## Scope

### In Scope

- `tools/ci-impact.mjs`: `scheduled` フラグ・`lanes` 出力・mode 配線・
  `OBSERVED_LEGS`(macOS non-blocking track)の gate 記録。
- `ci.yml`: classify `lanes` output・mode 解決の selective 化・
  `CI_SELECTIVE_DISABLE` kill switch・各 lane job の `needs`/`if:`・
  required matrix の ubuntu+windows 化・`*-macos` 専用 job 3 件。
- `tools/ci-impact.test.mjs`: scheduling 契約 + observed track の pin。
- 本仕様書(ruleset 状態・rollback・監視の記録)。

### Out of Scope

- Ruleset の実変更(`main-checkstatus` は coordinator が適用済み)。
- leg 単位(OS 単位)の部分実行(job-level `if` の matrix 制約により
  現行機構では表現不能。必要になれば per-OS job 分割等の別設計)。
- 新規 lane の追加・既存 lane 内容の変更。
- macOS leg の required 復帰判断(owner 判断事項・別途)。

## Acceptance Criteria

- [ ] `node --test tools/ci-impact.test.mjs` pass(scheduling + observed
      track 契約を含む)。
- [ ] `mise run lint-actions` pass。
- [ ] PR で classify が `lanes` output を発行し selective mode で動作。
- [ ] required 8 contexts(verify/gw10 ×ubuntu+windows・msrv・rt・
      dep-guard・evidence)が ruleset `main-checkstatus` と名称一致。
- [ ] `*-macos` job が evidence needs・required universe 外で結果が
      Observed 表に記録され、queued/cancelled/fail が PR gate を
      塞がない(required leg 側は引き続き fail-closed)。
- [ ] RT strict-prefix/zero-test ガード・GW-10 harness・MSRV lane は
      分類で影響なしと証明された場合のみ skip 可能(分類規則不変)。
- [ ] `push: main` / `workflow_dispatch` は full mode で全 lane 実行。
- [ ] PR 本文に ruleset 8-context 整合・macOS non-blocking 設計・
      rollback・NotRun が明記。
- [ ] rollback 手順(kill switch + revert + macOS required 復帰)が本書に
      記録済み。

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/157
- 前提仕様: `docs/specs/issue-148-impact-classification.md` §安全移行
- Shadow 証跡: `docs/impl/exp/issue-131-aut-01-shadow-ci.md`(#159)、
  #156 導入 PR・#158(mcp-poc control channel)の各 evidence run
- 汎用契約 Owner: https://github.com/scarecrow173/ruviform/issues/333
