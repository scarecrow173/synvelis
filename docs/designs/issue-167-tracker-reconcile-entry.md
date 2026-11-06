---
id: "DESIGN-RBfO0jdqOYWxb6xvtT5z0p"
type: "design"
status: "approved"
title: "Issue #167 Tracker reconcile 設計 — Consumer 共有 checkout 適合・mode 分離・単一 pin 契約"
created: "2026-11-06"
updated: "2026-11-06"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/167"
    - "docs/specs/issue-167-tracker-reconcile-entry.md"
    - "docs/adr/issue-167-tracker-reconcile-entry.md"
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
    generated: []
  implements:
    - "docs/specs/issue-167-tracker-reconcile-entry.md"
    - "docs/adr/issue-167-tracker-reconcile-entry.md"
  implemented-by:
    - "docs/plans/issue-167-tracker-reconcile-entry.md"
  depends-on:
    - "docs/specs/issue-167-tracker-reconcile-entry.md"
    - "docs/adr/issue-167-tracker-reconcile-entry.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-166-full-ci-ci-summary.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-167-tracker-reconcile-entry.md"
    - "docs/adr/issue-167-tracker-reconcile-entry.md"
  derived-by:
    - "docs/plans/issue-167-tracker-reconcile-entry.md"
    - "docs/test-specs/issue-167-tracker-reconcile-entry.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-167-tracker-reconcile-entry.md"
  references:
    - "https://docs.github.com/en/actions/how-tos/manage-workflow-runs/disable-and-enable-workflows"
    - ".github/workflows/ci-issue-tracker.yml"
    - ".github/workflows/daily-ci.yml"
  defers: []
  deferred-by: []
---

# Issue #167 Tracker reconcile 設計 — Consumer 共有 checkout 適合・mode 分離・単一 pin 契約

## Context

Ruviform 側の参照実装(RF `ci-issue-tracker-reconcile.yml`、eed590e)
は evaluator を repo 内に持つため 1 checkout で完結する。Synvelis は
共有 evaluator を持たず、既存 `daily-ci.yml` / `ci-issue-tracker.yml`
と同じく `.ci-shared/ruviform` への 2 段 checkout(own repo + RF
repo at pin)+ `CROSS_REPO_DEPLOY_KEY` + `mise exec -- node` という
軽量パターンを使う。設計上の問いは (1) RF の workflow 骨格を
Consumer checkout パターンにどう適合させるか、(2) SV 側で保つべき
契約の正本をどこに置くか、(3) schedule cadence の値と根拠。

## Components and Boundaries

### `.github/workflows/ci-issue-tracker-reconcile.yml`(新設)

- `name: ci-issue-tracker-reconcile`。
  `on.schedule: [{cron: "47 */6 * * *"}]`(UTC・分 != 0・6h 間隔。
  根拠は ADR「Schedule cadence の決定根拠」節)+ `workflow_dispatch:`
  (入力なし — evaluator が一様で、manual run が cron と同じ経路を
  証明する)。`pull_request`/`push`/`workflow_run` は持たない。
- `permissions: {}`(workflow 直下 — 既存 SV workflow と同じ deny
  by default。RF 版は `contents: read` だが、SV 慣習の `{}` はそれを
  下回る権限であり job 単位で必要分だけ付与する構成が同一方針)。
- `concurrency.group: ci-issue-tracker-${{ github.repository }}`・
  `cancel-in-progress: false` — event 入口と**同一 group**で
  write 直列化(group は workflow 名を含まないため共有できる)。
  pending 置換で reconcile が落ちても冪等で、次回 tick / event が
  同じ権威履歴から同じ結論に収束する。
- `env.MISE_ENABLE_TOOLS: node`(track/decide job と同じ toolchain
  pin)。
- job `reconcile`(`ubuntu-latest`):
  - `if: github.event_name == 'schedule' ||
    github.ref_name == github.event.repository.default_branch`
    — schedule は定義上 default branch でのみ走るため無条件通過
    (GitHub 保証)。guard を schedule の payload shape に依存させない
    防御設計: payload は歴史的に変更されており(`repository` は
    2022-09 追加)、不在環境(旧 GHES 等)で ref 比較が空に解決すると
    cron 全 skip のサイレント死になる。比較は dispatch の
    非 default ref 起動を no-op にする防御としてのみ効く。
  - `timeout-minutes: 15` — hang が共有 concurrency group を占有し
    event 入口の pending 置換窓を広げるのを防ぐ。
  - `permissions: contents: read / actions: read / issues: write`
    — track job と同一最小権限。compare API・Issue 管理・run/job
    metadata 参照に必要十分。新 secret なし。

#### step 構成(Consumer パターン適合)

| # | step | RF 版との差異 |
|---|------|--------------|
| 1 | `actions/checkout` own repo at `ref: ${{ github.event.repository.default_branch \|\| 'main' }}`、`persist-credentials: false` | RF は同一 repo 内 evaluator のため default-branch pin のみ。SV は mise.toml/lock 取得用。dispatch が選択 ref の workflow file を使う仕様上、trusted code 境界を明示。 |
| 2 | `actions/checkout` `scarecrow173/ruviform` at `eed590e…` → `path: .ci-shared/ruviform`、`ssh-key: CROSS_REPO_DEPLOY_KEY`、`persist-credentials: false` | RF 版には無い Consumer 固有 step。既存 2 ファイルと同一構成で pin だけ揃える。 |
| 3 | `jdx/mise-action`(install: false)→ `mise trust` + `mise install --locked node` | RF と同じ toolchain 復元。 |
| 4 | 共有 revision を Job Summary に記録(`git -C .ci-shared/ruviform rev-parse HEAD`) | SV 既存 step と同じ。pin 版の実効確認を残す。 |
| 5 | `mise exec -- node --test .ci-shared/ruviform/ci/tracker/` | RF の `mise run test-ci-tracker` を Consumer 形式へ適合。壊れた evaluator で Issue を書かない fail-closed。 |
| 6 | `GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}` のみ渡して `mise exec -- node .ci-shared/ruviform/ci/tracker/tracker.mjs --reconcile --report ci-issue-tracker-report.json` | `TRACKER_EVENT_PATH` を env key として設定しない(mode 分離)。evaluator は ambient `GITHUB_EVENT_PATH` も reconcile では参照しない(共有 pin が保証)。 |
| 7 | `actions/upload-artifact` `if: always()`、name `ci-issue-tracker-reconcile-report-attempt-${{ github.run_attempt }}`、path `ci-issue-tracker-report.json`、`if-no-files-found: warn` | event 入口の `ci-issue-tracker-report-*` と別名で識別。report JSON の `mode: "reconcile"` と併せて入口識別可能。 |

### `daily-ci.yml` / `ci-issue-tracker.yml`(ref: 行のみ)

- 共有 checkout step の `ref:` を `73d51d8…` → `eed590e…` へ 1 行ずつ
  更新。`73d51d8 → eed590e` の共有差分は `ci/tracker/` のみ
  (`ci/daily/` 無変更)のため、daily 側の挙動差分は実質 pin 統一のみ。
  trigger・job・権限・env・引数は一切触れない — `TRACKER_EVENT_PATH`
  を渡す event 経路はそのまま維持。

### `tools/ci-issue-tracker-reconcile.test.mjs`(新設)

- Consumer 側契約を静的に pin する `node --test` スイート。YAML
  パーサ依存を避け、workflow テキストに対する構造 assertion のみ
  (依存追加禁止の task 制約に整合)。対象:
  - **共有 pin 一致**: 3 ファイルの `repository: scarecrow173/ruviform`
    checkout ブロックが `ref: eed590e…` を持つ。旧 pin `73d51d8` が
    いずれの workflow にも残らない。
  - **reconcile 入口契約**: `on:` に `schedule`(cron 記述あり)と
    `workflow_dispatch` のみ — `pull_request`/`push`/`workflow_run`
    を含まない。同一 concurrency group・job guard 式・
    `timeout-minutes: 15`・最小権限・default-branch checkout pin・
    own checkout `persist-credentials: false`・self-test が
    reconcile 実行より前・`--reconcile` + `--report`・
    reconcile step の env が `GH_TOKEN` のみ(`TRACKER_EVENT_PATH`
    他 env key なし)・別名 artifact。
  - **event 入口不変**: `ci-issue-tracker.yml` が `workflow_run` のみを
    持ち、`schedule`/`workflow_dispatch`/`pull_request`/`push` を
    追加していない。`TRACKER_EVENT_PATH` 渡しを維持し `--reconcile`
    を含まない。
  - **daily 入口不変**: `daily-ci.yml` が `tracker.mjs`/`--reconcile`
    を含まない。
  - **ci.yml trigger 不変**: `pull_request`/`push`/`workflow_dispatch`
    を維持(本工程で trigger 撤去しない制約の pin)。
- ファイル順序依存の assertion(self-test が reconcile より前)は
  `indexOf` 比較で行い、テキスト整形差への耐性を保つ。

### `mise.toml`(`[tasks.test-ci-tracker-entry]` 追加)

- `node --test tools/ci-issue-tracker-reconcile.test.mjs` を実行する
  task を追加。canonical 入口は mise task(worker-guide 規約)。

## 境界と非目標

- evaluator・guard・Issue 冪等・compare/pending/unknown の判定契約は
  Ruviform 側(pin eed590e)の正本に委譲。SV 側 test は「正しくその
  実装を呼ぶ Consumer 契約」のみを pin し、判定ロジックを再検証しない
  (RF 側 91 mock test が正本)。
- `ci.yml`・ruleset・Cargo.lock・Rust/UI コードは対象外。
- `concurrency` group 名・job 名・artifact 名は event 入口と衝突
  しないこと(同名 artifact は upload 時 suffix で分離されるが、
  識別のため別名を明示)。

## Failure Modes

- **cron auto-disable**: schedule 追加を event 入口に持ち込まない
  ため、60 日無活動(public repo — GitHub 文書の記載範囲)で
  reconcile が止まっても `workflow_run` 入口は生き残る。再有効化は
  `gh workflow enable ci-issue-tracker-reconcile`
  (共有 README の運用記録と同じ)。
- **pin 版 evaluator の破損**: self-test 先行で run 失敗 — Issue への
  誤書込みを防ぐ(fail-closed)。
- **deploy key 失効**: 共有 checkout 失敗は job 失敗として可視化。
  event 入口も同じ key を使うため同時発生 — 新規 secret で回避しない。
- **非 default ref での manual dispatch**: job guard が skip。
  `github.event.repository.default_branch || 'main'` の空値 fallback
  で checkout が triggering ref に流れないよう既定値を明示。
