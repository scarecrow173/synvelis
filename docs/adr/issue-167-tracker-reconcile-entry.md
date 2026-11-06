---
id: "ADR-XTlIzRk1SB1nTV82KYSW6m"
type: "adr"
status: "accepted"
title: "Issue #167 Tracker reconcile 入口 — 共有 pin eed590e で RF 別 workflow reconcile 契約を Consumer 採用(trigger 禁止の補足契約の転用)"
created: "2026-11-06"
updated: "2026-11-06"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/167"
    - "https://github.com/scarecrow173/ruviform/issues/368"
    - "https://github.com/scarecrow173/ruviform/pull/380"
    - "docs/specs/issue-167-tracker-reconcile-entry.md"
  changes:
    added:
      - type: "file"
        path: ".github/workflows/ci-issue-tracker-reconcile.yml"
      - type: "file"
        path: "tools/ci-issue-tracker-reconcile.test.mjs"
    modified:
      - type: "file"
        path: ".github/workflows/ci-issue-tracker.yml"
      - type: "file"
        path: ".github/workflows/daily-ci.yml"
      - type: "file"
        path: "mise.toml"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
  implements:
    - "docs/specs/issue-167-tracker-reconcile-entry.md"
  implemented-by:
    - "docs/designs/issue-167-tracker-reconcile-entry.md"
  depends-on:
    - "docs/specs/issue-167-tracker-reconcile-entry.md"
    - "https://github.com/scarecrow173/ruviform/pull/380"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/issue-166-full-ci-ci-summary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-167-tracker-reconcile-entry.md"
  derived-by:
    - "docs/designs/issue-167-tracker-reconcile-entry.md"
  verifies: []
  verified-by: []
  references:
    - "https://docs.github.com/en/actions/how-tos/manage-workflow-runs/disable-and-enable-workflows"
    - ".github/workflows/ci-issue-tracker.yml"
    - ".github/workflows/daily-ci.yml"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: []
    consulted: []
    informed: []
---

# Issue #167 Tracker reconcile 入口 — 共有 pin eed590e で RF 別 workflow reconcile 契約を Consumer 採用

## Context and Problem Statement

Synvelis の `ci-issue-tracker.yml`(#169/#170 で導入)は Ruviform の
accepted ADR-034SraR8CrkZ0SSa7tQEwh と同じく `workflow_run` 完了
監視専用であり、失敗 Issue の追跡を共有 evaluator
(`ci/tracker/tracker.mjs`、固定 SHA checkout)に委譲する。しかし
`daily-ci.yml` が `GITHUB_TOKEN` で dispatch する Full CI の完了は
GitHub anti-recursion により `workflow_run` event を発火させず、
Tracker が自動起動しない(Ruviform #368 で実測。SV 側でも Issue #167
コメントで「自動通知 gap は未解決」と記録済み)。Ruviform ではこの
gap を埋める恒久策として ADR-vJvyZNtrHi2xnycJxbnSPv が採択され、
`--reconcile` モードを持つ共有 evaluator と別 workflow
`ci-issue-tracker-reconcile.yml`(`schedule` + `workflow_dispatch`)
が PR #380 で main `eed590e0a3837e61c943d9de1d3efcfd7df32cb0` に
統合済み。Synvelis は共有 pin を旧 `73d51d8` に留めており、reconcile
入口を持たない。

## Decision Drivers

- 通知 gap は Consumer にも同型で存在し、機器側の恒久策は「共有
  evaluator の `--reconcile` を Consumer が別入口から呼ぶ」こと。
  共有 pin を eed590e に上げることが前提。
- RF ADR-vJvyZNtrHi2xnycJxbnSPv の補足契約を **逆に書き換えない**:
  `ci-issue-tracker.yml` への `schedule`/`workflow_dispatch` 追加は
  引き続き禁止とし、別 workflow ファイルの `schedule`/manual のみ
  許可する。SV 側でも同じ禁止範囲を採用する。
- Consumer の軽量共有 checkout パターン(`.ci-shared/ruviform` +
  `CROSS_REPO_DEPLOY_KEY` + `mise exec -- node --test` self-test)を
  維持し、Ruviform の Rust/Frontend build 依存やコード vendor 重複を
  増やさない。
- 最小権限・既存 secret のみ: 新 credential は作らず、既存
  `secrets.GITHUB_TOKEN` + `secrets.CROSS_REPO_DEPLOY_KEY` を継続。
- cron 停止(auto-disable — GitHub 文書では public repo で 60 日
  無活動と記載。actor 停止等の停止経路もあるため可視性に依らず
  隔離する)の blast radius を event 入口から分離するため、
  `schedule` は reconcile 専用ファイルに閉じ込める。

## Considered Options

- **Option A(採用)**: 共有 pin を `eed590e` へ更新し、Consumer 版
  `ci-issue-tracker-reconcile.yml` を別 workflow として新設。
  evaluator・guard・権限・concurrency group は共有契約をそのまま
  継承し、checkout/self-test のみ既存 Consumer パターンに適合させる。
- **Option B**: 既存 `ci-issue-tracker.yml` に `schedule`/
  `workflow_dispatch` を追加。**却下** — RF ADR-vJvyZNtrHi2xnycJxbnSPv
  が「当該ファイルへの trigger 追加禁止」を明確化しており、SV でも
  同契約を採用する。scheduled workflow の auto-disable が event 入口に
  及ぶ blast radius も失われる。
- **Option C**: `daily-ci.yml` に reconcile job を追加。**却下** —
  scheduler(dispatch 判定)と tracker(Issue 集約)の責務・権限
  (`actions:write` vs `issues:write`)を混ぜ、reconcile 失敗が
  scheduler run の結論を濁す。RF ADR Option C と同じ却下理由。
- **Option D**: dispatch に PAT/GitHub App token を使い
  `workflow_run` を発火させる。**却下** — 新 credential/secret を
  要求し、task 制約と最小権限の原則に反する。
- **Option E**: SV 独自の reconcile 実装を書く(vendor/フォーク)。
  **却下** — 判定ロジックの重複で契約ドリフトを招き、共有実装の
  fail-closed guard・Issue 冪等の恩恵を失う。AGENTS.md の
  「Ruviform の問題を Synvelis 側の恒久的な重複実装で隠さない」にも
  反する。

## Decision Outcome

Chosen option: **Option A**。

- `daily-ci.yml`・`ci-issue-tracker.yml` の共有 checkout `ref:` を
  `eed590e0a3837e61c943d9de1d3efcfd7df32cb0` へ更新(その他は不変更)。
- `.github/workflows/ci-issue-tracker-reconcile.yml` を新設:
  `schedule`(`47 */6 * * *`・後述の根拠)+ `workflow_dispatch`
  のみ。workflow 直下 `permissions: {}`、job は
  `contents:read`/`actions:read`/`issues:write` の track job と同一
  最小権限。`concurrency` は event 入口と同一 group
  `ci-issue-tracker-${{ github.repository }}`・
  `cancel-in-progress: false`。job guard は
  `github.event_name == 'schedule' ||
  github.ref_name == github.event.repository.default_branch`、
  `timeout-minutes: 15`。
- 手順は Consumer パターン: own repo を default branch に pin して
  checkout(`persist-credentials: false`)→ 共有実装を
  `CROSS_REPO_DEPLOY_KEY` で `.ci-shared/ruviform` へ checkout →
  mise node 復元 → `node --test .ci-shared/ruviform/ci/tracker/`
  self-test → `GH_TOKEN` のみ渡して
  `tracker.mjs --reconcile --report ci-issue-tracker-report.json`
  (`TRACKER_EVENT_PATH` は設定しない)→ report artifact を
  `if: always()` で別名 upload。
- `ci-issue-tracker.yml` 本体の trigger・権限・job は共有 `ref:` 行
  以外無変更 — 本 ADR は SV の event 入口契約を「当該ファイルへの
  trigger 追加は引き続き禁止、同一 evaluator を呼ぶ別 workflow の
  schedule/manual は許可」と記録するものであり、RF 側の accepted
  ADR(034SraR8…・vJvyZNtr…)の status・理由・禁止範囲を
  書き換えるものではない。

### Schedule cadence の決定根拠

- 基本間隔は RF と同じ **6 時間**: tracker は Issue 管理であり即時性は
  event 入口が担う。RF ADR で 6h が受入済みの運用パラメータ。
- 分(`47`)は RF の `23 */6`・SV daily-ci の `29 21 * * *` の両方と
  ずらす。目的は 2 点: (1) SV daily dispatch(21:29 UTC)から始まる
  Full CI(~1-2h)が次 tick(00:47)までに概ね完了し、~1-3h 内に
  reconcile される timing を確保(GitHub の schedule 配送は
  best-effort で tick の遅延・欠落があり得る — 名目上の目安)、
  (2) 同一 org アカウント下の RF/SV/AL
  3 repo の reconcile tick が同時刻に集中し Issue write/API 呼出が
  burst 化するのを避ける。cron の分を 0 にしないのは GitHub の
  top-of-hour 負荷集中を避ける RF と同じ慣習。
- cron を変更しても evaluator・guard は同一のため、間隔は契約ではなく
  運用パラメータ(後から調整可)。契約 test
  (`tools/ci-issue-tracker-reconcile.test.mjs`)は現在値
  `47 */6 * * *` を意図的に pin しており、cadence 再調整時は
  test 側も同時更新する(drift 検知 tripwire)。

### Consequences

- Good, because 通知 gap が trusted default-branch code の定期 pull で
  自己修復され、SV でも bot dispatch 失敗の起票漏れ・成功の close
  漏れが解消する。共有 pin を揃えるため evaluator 修正(ENOBUFS・
  reconcile)がそのまま使える。権限・secret・依存は増えない。
- Bad, because SV 側も workflow が 1 本増え、cron auto-disable で
  reconcile が止まると gap が戻る(event 入口は残るため push/手動
  dispatch の追跡には影響しない)。検知遅延は名目上最大 1 間隔
  (GitHub の schedule 配送は best-effort で tick の遅延・欠落が
  あり得るため保証値ではない)。
- Neutral: 本 ADR は RF ADR-vJvyZNtrHi2xnycJxbnSPv の Consumer 適用
  記録であり、共有 evaluator 契約・Issue model・close guard は
  Ruviform 側の正本に従う。

### Confirmation

`tools/ci-issue-tracker-reconcile.test.mjs` が Consumer 契約
(pin 一致・mode 分離・guard・入口 trigger・event/daily 入口不変)を
`mise run test-ci-tracker-entry` で検証。
`mise run lint-actions` で workflow 構文を検証。共有 evaluator の
mock test は pin 版 tarball 上で実測。実 cron・実 Issue・実 dispatch
の受入は merge 後の運用段階。

## Implementation Plan

- Affected paths: `.github/workflows/ci-issue-tracker-reconcile.yml`
  (新設)、`.github/workflows/daily-ci.yml`・`ci-issue-tracker.yml`
  (共有 `ref:` 1 行ずつ)、`tools/ci-issue-tracker-reconcile.test.mjs`
  (新設)、`mise.toml`(task 追加)、本 doc chain + 生成 index。
- Patterns to follow: RF `.github/workflows/ci-issue-tracker-reconcile.yml`
  (eed590e)の guard・concurrency・最小権限構成、SV `ci-issue-tracker.yml`
  の共有 checkout/self-test/artifact 構成。
- Patterns to avoid: `ci-issue-tracker.yml` への trigger 追加、
  `daily-ci.yml` への tracker 責務混入、`TRACKER_EVENT_PATH` の
  reconcile 経路への流出、新規 secret/権限増量、未信頼コードの実行、
  共有 evaluator の vendor 重複。
- Dependencies: 既存 `mise`/`node`/`gh` CLI、`secrets.GITHUB_TOKEN`、
  `secrets.CROSS_REPO_DEPLOY_KEY` のみ。

## Verification

- [ ] `mise run test-ci-tracker-entry` → pass(契約 test 全 assertion)。
- [ ] `mise run lint-actions` → pass。
- [ ] `mise exec -- prek run --all-files` → pass。
- [ ] pin 版 tarball 上の `node --test ci/tracker/`(91)・
      `ci/daily/`(50)・`mise run test-ci-summary`(36) → pass。
- [ ] `git diff` で `ci-issue-tracker.yml`/`daily-ci.yml` が
      `ref:` 行のみの差分、`ci.yml` 無変更。
- [ ] merge 後・実 cron 受入(本工程範囲外 — 未観測を Pass にしない)。
