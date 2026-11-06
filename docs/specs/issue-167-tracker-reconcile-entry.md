---
id: "SPEC-jY8M1QCzHBuilakae6Hmyw"
type: "spec"
status: "proposed"
title: "Issue #167 Tracker reconcile 入口 — 共有 pin eed590e 更新 + Consumer 別 workflow 定期/手動 reconcile"
created: "2026-11-06"
updated: "2026-11-06"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/167"
    - "https://github.com/scarecrow173/synvelis/issues/166"
    - "https://github.com/scarecrow173/synvelis/issues/145"
    - "https://github.com/scarecrow173/ruviform/issues/368"
  changes:
    added:
      - type: "file"
        path: ".github/workflows/ci-issue-tracker-reconcile.yml"
      - type: "file"
        path: "tools/ci-issue-tracker-reconcile.test.mjs"
      - type: "file"
        path: "docs/specs/issue-167-tracker-reconcile-entry.md"
      - type: "file"
        path: "docs/adr/issue-167-tracker-reconcile-entry.md"
      - type: "file"
        path: "docs/designs/issue-167-tracker-reconcile-entry.md"
      - type: "file"
        path: "docs/plans/issue-167-tracker-reconcile-entry.md"
      - type: "file"
        path: "docs/test-specs/issue-167-tracker-reconcile-entry.md"
      - type: "file"
        path: "docs/tasks/issue-167-t1-tracker-reconcile-entry.md"
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
  implements: []
  implemented-by:
    - "docs/adr/issue-167-tracker-reconcile-entry.md"
    - "docs/designs/issue-167-tracker-reconcile-entry.md"
  depends-on:
    - "docs/specs/issue-166-full-ci-ci-summary.md"
    - "https://github.com/scarecrow173/ruviform/pull/380"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/ci-synvelis-pr-push-regression-gate.md"
    - "docs/specs/issue-166-full-ci-ci-summary.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/167"
  derived-by:
    - "docs/adr/issue-167-tracker-reconcile-entry.md"
    - "docs/designs/issue-167-tracker-reconcile-entry.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-167-tracker-reconcile-entry.md"
  references:
    - ".github/workflows/ci-issue-tracker.yml"
    - ".github/workflows/daily-ci.yml"
    - ".github/workflows/ci.yml"
    - "mise.toml"
  defers: []
  deferred-by: []
---

# Issue #167 Tracker reconcile 入口 — 共有 pin eed590e 更新 + Consumer 別 workflow 定期/手動 reconcile

## What Should Be Built

Synvelis の `daily-ci.yml` と `ci-issue-tracker.yml` が共有 checkout する
Ruviform CI 実装の固定 SHA を `73d51d8` →
`eed590e0a3837e61c943d9de1d3efcfd7df32cb0`(Ruviform PR #380 統合後の
main)へ整合更新し、Ruviform 側で accepted 済みの契約
(ADR-vJvyZNtrHi2xnycJxbnSPv)に従って Consumer 側の第二入口
`.github/workflows/ci-issue-tracker-reconcile.yml`(`schedule` +
`workflow_dispatch` のみ)を新設する。新 workflow は共有実装の
`ci/tracker/tracker.mjs --reconcile` を呼び、GITHUB_TOKEN dispatch
完了が `workflow_run` を発火させない通知 gap を pull 型で自己修復する。

## Why Now

- Ruviform #368 で実測済みの通知 gap は Consumer にも同型に存在する:
  SV `daily-ci.yml` が `GITHUB_TOKEN` で `ci.yml` を `workflow_dispatch`
  しても、その完了は `workflow_run` event を発火させず
  `ci-issue-tracker.yml` が起動しない(bot 失敗の起票漏れ・bot 成功の
  回復 close 漏れの両方向)。Issue #167 コメントでも「自動通知 gap は
  未解決」と記録済み。
- 共有実装 `--reconcile` モードは Ruviform 側で実装・91 contract test
  成功・manual reconcile run 42178261480 の実受入が進行済み
  (coordinator 実 API で completed/success 確認)。
- ユーザー最新指示「CI関連を先に完了し、PR作成時にCIが走り始めない
  ところまで優先して終わらせる」の前提工程として、Consumer 展開を
  main 実受入可能な形で PR 化する。trigger 撤去(#145/#166 変更6)は
  本工程の後段であり、本 spec では `ci.yml` の trigger を変更しない。

## Users and Value

- Synvelis maintainers: scheduler dispatch の Full CI 失敗・回復が
  人の Replay なしに Issue に反映される。
- 共有実装 Owner(Ruviform): Consumer が同一 evaluator・同一 pin で
  追従し、vendor 重複・判定ドリフトを生じさせない。
- 後段作業者: reconcile 入口が main で実受入されて初めて
  PR/push trigger 撤去後も失敗追跡が止まらない。

## Scope

### In Scope

- `.github/workflows/daily-ci.yml` / `ci-issue-tracker.yml` の共有
  checkout `ref:` を `eed590e0a3837e61c943d9de1d3efcfd7df32cb0` へ
  更新(それ以外の trigger・job・権限・concurrency・引数は不変更)。
- `.github/workflows/ci-issue-tracker-reconcile.yml` 新設: RF 新
  workflow を Consumer の軽量共有 checkout パターン(`.ci-shared/
  ruviform` へ `CROSS_REPO_DEPLOY_KEY` で checkout → mise node 復元 →
  共有 self-test → `tracker.mjs --reconcile --report`)へ適合。
- `tools/ci-issue-tracker-reconcile.test.mjs` 新設: Consumer 側契約
  (mode 分離・guard・共有 pin 一致・入口 trigger)を検証する
  `node --test` 静的契約 test + `mise.toml` への実行 task 追加。
- 本 doc chain(spec/adr/design/plan/test-spec/task)と生成 index。

### Out of Scope

- `ci.yml` の `pull_request`/`push` trigger 撤去・Ruleset
  `main-checkstatus` required 解除(後段工程・main 実受入後)。
- `ci-issue-tracker.yml` への `schedule`/`workflow_dispatch` 追加
  (event 専用契約の維持 — RF ADR 補足と同じ禁止範囲)。
- 共有 evaluator(`tracker.mjs`/`decide.mjs`)自体の変更 —
  Ruviform 所有コードであり vendor 重複・フォークしない。
- `Cargo.lock` の ruviform resolved SHA / dep-guard / Rust・UI 製品コード
  (別責務・CI 共有 pin とは無関係)。
- 新規 secret・権限増量・外部監視サービス。
- 実 cron・実 Issue 起票の受入(merge 後の運用段階・別工程)。

## Requirements

- R1: `daily-ci.yml`・`ci-issue-tracker.yml`・
  `ci-issue-tracker-reconcile.yml` の共有 checkout `ref:` は全て
  `eed590e0a3837e61c943d9de1d3efcfd7df32cb0` で一致する
  (3 ファイル単一 pin — 契約 test で pin する)。
- R2: `ci-issue-tracker.yml` の trigger・job 構造・権限・concurrency・
  `TRACKER_EVENT_PATH` 引数渡しは共有 `ref:` 行以外無変更。
- R3: 新 workflow は `schedule` + `workflow_dispatch` のみを持ち、
  `pull_request`/`push`/`workflow_run` は持たない。cron は当該
  ファイルに閉じ込め、既存 workflow へは追加しない(scheduled
  workflow の auto-disable が event 入口に及ぶ blast radius 分離 —
  RF ADR-vJvyZNtrHi2xnycJxbnSPv と同一根拠。60 日無活動ルールは
  public repo に文書化されるが、actor 停止等の停止経路もあり
  分離自体は可視性に依らず維持する)。
- R4: job は `github.event_name == 'schedule' ||
  github.ref_name == github.event.repository.default_branch` の
  時のみ実行する。schedule は定義上 default branch でのみ走るため
  無条件通過とし、ref 比較は dispatch に限定する — schedule の
  payload shape に guard を依存させない(payload は歴史的に変更されて
  おり、`repository` 不在環境(旧 GHES 等)で ref 比較が空に解決する
  と cron 全 skip のサイレント死になる)。非 default ref での
  manual dispatch は no-op skip とする。
- R5: checkout は `ref: ${{ github.event.repository.default_branch
  || 'main' }}` で trusted default branch code に pin し、
  `persist-credentials: false`。共有 checkout は既存の
  `CROSS_REPO_DEPLOY_KEY` を継続し、新 secret を作らない。
- R6: job 権限は既存 `track` job と同一(`contents:read` /
  `actions:read` / `issues:write`)、workflow 直下は `permissions: {}`
  (deny by default)。`concurrency` は event 入口と同一 group
  `ci-issue-tracker-${{ github.repository }}`・
  `cancel-in-progress: false` で write を直列化する。job は
  `timeout-minutes: 15` で共有 group の占有を bounded にする。
- R7: 実行前に共有 evaluator の self-test
  (`node --test .ci-shared/ruviform/ci/tracker/`)を行い、壊れた
  evaluator で Issue を書かない。
- R8: `tracker.mjs --reconcile --report ci-issue-tracker-report.json`
  を呼び、reconcile step の env には `GH_TOKEN` のみを渡す
  (`TRACKER_EVENT_PATH` を含む他 env key は設定しない — mode 分離。
  ambient `GITHUB_EVENT_PATH` も evaluator 側が参照しない設計を
  共有 pin が保証する)。report/Job Summary の `mode: reconcile` と
  別名 artifact(`ci-issue-tracker-reconcile-report-*`)でどちらの
  入口が動いたか識別可能にする。
- R9: cron は UTC・分 != 0・SV daily-ci(`29 21 * * *`)と別時刻・
  RF reconcile(`23 */6 * * *`)とも別分とし、決定根拠を workflow
  コメントと ADR に記録する。
- R10: Consumer 側の意味ある test(共有 pin 一致・reconcile 入口
  契約・event 入口不変・daily-ci 不変・`ci.yml` trigger 不変)を
  `mise run test-ci-tracker-entry` で検証する。共有 evaluator の
  mock test(91 件)は self-test として workflow 内でも実行するが、
  RF worker の成功報告を consumer 側成功と見做さず、pin 版
  tarball 上でローカル実測する。実 GitHub API・実 Issue・実 cron は
  起こさない。

## Acceptance Criteria

- [ ] `ci-issue-tracker-reconcile.yml` が `schedule` + `workflow_dispatch`
      のみで存在し、guard・default-branch pin・最小権限・同一
      concurrency group・15 分 timeout・self-test 先行・`--reconcile`
      実行を持ち、`mise run lint-actions` が pass する。
- [ ] 3 workflow の共有 `ref:` が `eed590e` で一致し、
      `ci-issue-tracker.yml`/`daily-ci.yml` は `ref:` 行のみの差分。
- [ ] `mise run test-ci-tracker-entry` が pass し、mode/guard/pin/
      入口契約の全 assertion を含む。
- [ ] pin 版 tarball 上の共有 tracker/daily mock test・
      `mise run test-ci-summary`・`mise exec -- prek run --all-files`
      が pass。
- [ ] 本 doc chain が独立レビュー(review-protocol)で blocker なし。
- [ ] 実 GitHub API・実 Issue・実 cron を起こさない(実 cron 受入は
      merge 後の別工程 — 未観測を Pass にしない)。

## Deferred Design Concerns

- schedule auto-disable(public repo で 60 日無活動 — GitHub 文書の
  記載範囲)自体の監視 — event 入口は残る設計であり、disabled 検知・
  再有効化手順は共有 README に記録済み。Consumer 側の運用確認手順は
  ADR に転記。
- `main-checkstatus` required 解除と PR/push trigger 撤去 — Issue
  #145/#166 変更6 で管理。本工程はその前提となる reconcile 入口の
  整備のみ。

## Sources and Relations

- [Synvelis #167](https://github.com/scarecrow173/synvelis/issues/167):
  Scheduler/Tracker 導入 Issue — 通知 gap が未解決のまま Open。
- [Synvelis #166](https://github.com/scarecrow173/synvelis/issues/166) /
  [#145](https://github.com/scarecrow173/synvelis/issues/145): Full CI
  と Ruleset 移行 — trigger 撤去の後段工程。
- [Ruviform #368](https://github.com/scarecrow173/ruviform/issues/368):
  通知 gap の実測記録と reviewer 判定。
- Ruviform PR #380(main `eed590e0a3837e61c943d9de1d3efcfd7df32cb0`):
  `--reconcile` モード + RF 側 reconcile workflow + ADR-vJvyZNtrHi2xnycJxbnSPv
  の統合先。本 spec の共有 pin と入口契約の正本。
- RF `docs/adr/issue-368-tracker-reconcile-schedule-entry.md`
  (ADR-vJvyZNtrHi2xnycJxbnSPv): 「`ci-issue-tracker.yml` への trigger
  追加は引き続き禁止・同一 evaluator を呼ぶ別 workflow の
  schedule/manual は許可」— 本 spec は Consumer 側で同契約を採用する。
