---
id: "DESIGN-034SYN166CiSuMmRy"
type: "design"
status: "proposed"
title: "Issue #166 Full CI 化・ci-summary 統一 — 第一段階設計"
created: "2026-11-05"
updated: "2026-11-05"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/166"
    - "docs/specs/issue-166-full-ci-ci-summary.md"
  changes:
    added:
      - type: "file"
        path: "tools/ci-summary.mjs"
      - type: "file"
        path: "tools/ci-summary.test.mjs"
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
      - type: "file"
        path: "mise.toml"
    deleted:
      - type: "file"
        path: "tools/ci-impact.mjs"
      - type: "file"
        path: "tools/ci-impact.test.mjs"
      - type: "file"
        path: "tools/fixtures/run-41894049161-jobs.json"
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-166-full-ci-ci-summary.md"
  implemented-by:
    - "docs/plans/issue-166-full-ci-ci-summary.md"
  depends-on:
    - "docs/specs/issue-166-full-ci-ci-summary.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/ci-synvelis-pr-push-regression-gate.md"
    - "docs/designs/issue-148-impact-classification.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-166-full-ci-ci-summary.md"
  derived-by:
    - "docs/plans/issue-166-full-ci-ci-summary.md"
    - "docs/test-specs/issue-166-ci-summary-gate.md"
    - "docs/tasks/issue-166-t1-ci-summary-stage1.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-166-ci-summary-gate.md"
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
    - "tools/ci-summary.mjs"
    - "tools/ci-summary.test.mjs"
  defers:
    - "pull_request/push トリガー除去・*-macos job の matrix 再統合"
  deferred-by: []
---

# Issue #166 Full CI 化・ci-summary 統一 — 第一段階設計

## 設計原則

- **required context `evidence` を死なせない**:`main-checkstatus`
  (id 26548278)の required_status_checks は実測 `evidence` 1件。
  ruleset を変更できない本段階で job 名 `evidence` を消すと全 PR が
  永久 pending でデッドロックする(#365 決定6 禁止事項)。したがって
  `evidence` は互換 gate として存置し、`ci-summary` を別 job として
  追加する。
- **PR gate の macOS 非ブロッキング境界を維持**:`*-macos` job を
  `evidence` の required に戻すと #157 rev.2 が解決した runner 逼迫
  による PR 塞がりが復活する。`ci-summary` だけが macOS を required
  評価し、`evidence` は従来の required universe(macOS 以外)を維持
  する二層構成とする。
- **期待集合の概念を除去**: 分類入力が無いため gate は「全 lane が
  実行され全 leg が success」だけを検査する単純な照合になる。
  NotApplicable 判定・shadow 照合・placeholder 受理はすべて撤去。

## Job 構造(第一段階)

```
lanes (classify 不要・if なし・毎回実行)
  verify (ubuntu-latest|windows-latest)         [matrix]
  verify-macos                                  [macOS 専用 job]
  rt-verify                                     [ubuntu]
  gw10-verify (ubuntu-latest|windows-latest)    [matrix]
  gw10-verify-macos                             [macOS 専用 job]
  msrv-verify                                   [ubuntu]
  ui-verify (ubuntu-latest|windows-latest)      [matrix]
  ui-verify-macos                               [macOS 専用 job]
  dep-guard                                     [ubuntu]
      │
      ├─ evidence   needs: [verify, rt-verify, gw10-verify, msrv-verify,
      │                    ui-verify, dep-guard]   if: always()
      │             mode=pr — macOS 以外の 9 leg を required 照合、
      │             *-macos は observed 記録のみ(従来境界維持)。
      │             ruleset required context `evidence` を発行。
      │
      └─ ci-summary needs: [全 lane 9 job + evidence]  if: always()
                    mode=full — 12 leg すべて + evidence を required
                    照合(macOS 失敗=Full CI 失敗)。
                    expected_sha 契約検証と SHA 証跡を所有。
```

`evidence` が `ci-summary` の needs に入るのは、「run 内の全 job が
成功した」ことを Full CI として保証するため — evidence gate 自身の
infra 障害(jobs API 収集失敗等)を Full CI 成功と誤認しない。

## tools/ci-summary.mjs

### Universe

```text
REQUIRED_LEGS (9):  verify (ubuntu-latest), verify (windows-latest),
                    rt-verify,
                    gw10-verify (ubuntu-latest), gw10-verify (windows-latest),
                    msrv-verify,
                    ui-verify (ubuntu-latest), ui-verify (windows-latest),
                    dep-guard
MACOS_LEGS (3):     verify-macos, gw10-verify-macos, ui-verify-macos
GATE_JOBS (2):      evidence, ci-summary          (自己評価対象外)
DECLARED_NOT_RUN:   非自動化の実機/GUI 受入(従来表を維持)
```

### Subcommands

- `gate [--mode pr|full] --jobs <path> [--attempt n] [--summary path]
  [--out path]` — leg 単位 verdict 評価。
- `expected-jobs [--mode pr|full]` — jobs API 収集の下限件数。
- `lanes` — 診断用に全 leg 名を出力。

### Mode 契約

| mode | required 集合 | macOS | evidence job | 用途 |
|---|---|---|---|---|
| `pr` | REQUIRED_LEGS 9 | observed(記録のみ、失敗は warning) | 評価しない(自分自身) | `evidence` job(ruleset required) |
| `full` | REQUIRED_LEGS + MACOS_LEGS + `evidence` | required | required | `ci-summary` job(Full CI verdict) |

### Verdict 規則(両 mode 共通・fail-closed)

- leg の `jobVerdict`: status != completed → `not-run`; conclusion
  success → `pass`; skipped → `skipped`; cancelled → `cancelled`;
  failure/timed_out/action_required/startup_failure/neutral/stale/
  未知 conclusion → `fail`。
- required leg で `pass` 以外 → violation(`skipped` は明示メッセージ
  — skipped is not Pass)。
- run 内の未知 job 名(GATE_JOBS・既知 leg 以外)→ unclassified
  violation。placeholder 名 `'<lane> (${{ ... }})'` は lane `if:`
  廃止後は発生しないため受理しない(契約外=fail-closed)。
- attempt 指定時は `run_attempt <= attempt` の最新 job を評価
  (re-run 混在を従来どおり処理)。
- `pr` mode で observed leg の fail/cancelled は `::warning` で
  surface するが violation ではない(rev.2 記録境界を維持)。

### expected-jobs 件数

- 全 lane が毎回実行されるため skip-collapsed placeholder は存在せず、
  matrix lane は必ず leg 数分展開される。
- `pr` mode: 12 leg + evidence 自身 = **13** を下限(この時点で
  ci-summary は未作成でもよい — needs の関係で後から現れ得る)。
- `full` mode: 12 leg + evidence + ci-summary 自身 = **14** を下限。

### Job Summary 出力

- ヘッダ: run URL/attempt、head SHA、base SHA(PR 時)、ruviform
  resolved SHA(dep-guard output)。
- **SHA contract セクション**: `expected_sha` input 値(未指定時は
  `<unset>`)、checkout 後 `git rev-parse HEAD`、`github.sha`。
- leg 表: `| lane leg | required | actual | verdict |`(actual は
  `status/conclusion` または `absent`)。
- `pr` mode は別表で observed macOS legs を記録。
- violations 一覧、Declared NotRun/Unsupported 表、Total runner time。

## ci.yml 変更

- `on.workflow_dispatch.inputs.expected_sha`: optional string。
  daily-ci(#167)が main 候補 SHA を指定して dispatch する契約の
  受け口を先に定義する。手動起動・PR/push では未指定でよい。
- 全 lane job + 両 gate job の checkout:
  `ref: ${{ inputs.expected_sha || github.sha }}`。
  - pull_request: `github.sha` = merge commit(既定と同じツリー)。
  - push: `github.sha` = push 済み commit(既定と同じ・ref 名より
    race-free)。
  - dispatch 指定あり: expected_sha を pin(github.sha=dispatch 時の
    ref HEAD とずれ得るため input を優先)。
- `classify` job・全 lane の `needs: classify`/`if:`・mode env を
  削除。`concurrency`・permissions・sccache・dep-access・証跡記録の
  各 step は変更しない。
- `evidence` job: needs から classify を除去し、collect の expected
  を `expected-jobs --mode pr`(13)、gate を
  `mise run ci-summary-gate`(`CI_SUMMARY_MODE: pr`)へ置換。
  tool self-test `mise run test-ci-summary` を collect 前に実行。
- `ci-summary` job(新規): checkout(ref pin)→ expected_sha
  検証 step(指定時 `git rev-parse HEAD` と一致確認、不一致 fail;
  未指定は記録のみ)→ mise/node 復元 → `expected-jobs --mode full`
  (14)の bounded retry 収集 → `CI_SUMMARY_MODE: full` で gate。
  permissions は evidence と同じ `contents:read + actions:read`。

## mise.toml 変更

- 削除: `ci-impact` / `ci-evidence-gate` / `test-ci-impact` + #148
  コメントブロック。
- 追加: `ci-summary-gate`(`node tools/ci-summary.mjs gate`)、
  `test-ci-summary`(`node --test tools/ci-summary.test.mjs`)。
  mode は `CI_SUMMARY_MODE` env で渡す(task 引数を増やさない)。

## 互換性と移行

- **check context 名の変化**: 新規発行 `ci-summary`(required で
  はない)。削除される context: `classify` — ただし ruleset required
  は `evidence` のみなので required 集合への影響なし。#157 spec の
  「8 contexts」記述は旧実測で、現在の ruleset 実測は `evidence` 1件
  — required context 名の drift は ruleset 側で既に吸収済み。
- **後段への引継ぎ条件**: (a) Owner が `main-checkstatus` required
  を空にしたことを実測確認、(b) `ci-summary` の dispatch run が main
  最新で少なくとも1回完走・判定正確であること、(c) #167 Scheduler/
  Tracker が稼働 — の3条件が揃ってから pull_request/push trigger と
  `evidence` job を除去し `*-macos` を matrix へ再統合する。
