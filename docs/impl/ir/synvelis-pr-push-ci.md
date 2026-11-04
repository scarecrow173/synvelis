---
id: "IMPL-034Rp8RppbqCF1xtexsFvB"
type: "impl"
status: "completed"
title: "Synvelis 独立 PR/Push CI 実装記録"
created: "2026-11-03"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: ".github/workflows/ci.yml"
      - type: "file"
        path: ".github/actions/ruviform-dep-access/action.yml"
      - type: "file"
        path: "docs/specs/ci-synvelis-pr-push-regression-gate.md"
      - type: "file"
        path: "docs/designs/ci-synvelis-pr-push-regression-gate.md"
      - type: "file"
        path: "docs/plans/ci-synvelis-pr-push-regression-gate.md"
      - type: "file"
        path: "docs/test-specs/ci-synvelis-pr-push-regression-gate.md"
      - type: "file"
        path: "docs/tasks/ci-task1-mise-tasks.md"
      - type: "file"
        path: "docs/tasks/ci-task2-workflow.md"
      - type: "file"
        path: "docs/tasks/ci-task3-verify-review.md"
      - type: "file"
        path: "docs/impl/ir/synvelis-pr-push-ci.md"
    modified:
      - type: "file"
        path: "mise.toml"
      - type: "file"
        path: "mise.lock"
      - type: "file"
        path: "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
      - type: "file"
        path: "docs/designs/media-import-export-offline-render-ui-diagnostics-architecture.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
  implements:
    - "docs/tasks/ci-task3-verify-review.md"
    - "docs/tasks/ci-task2-workflow.md"
    - "docs/tasks/ci-task1-mise-tasks.md"
    - "docs/plans/ci-synvelis-pr-push-regression-gate.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/ci-synvelis-pr-push-regression-gate.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# Synvelis 独立 PR/Push CI 実装記録

## Summary

Issue #83 の Synvelis 独立 CI を実装した。`.github/workflows/ci.yml`
に 6 job(verify 3-OS matrix / rt-verify / gw10-verify 3-OS matrix /
msrv-verify / dep-guard / evidence)を定義し、`.github/actions/
ruviform-dep-access` composite action で ruviform git 依存の
anonymous HTTPS / deploy key access mode を解決する。mise.toml には
`test-rt`・`lint-actions` タスクと `actionlint` tool を追加した。

## Context

spec→design→plan→test-spec→task チェーン
(`docs/*/ci-synvelis-pr-push-regression-gate.md` 等)に従う。
参照実装は ruviform `.github/workflows/ci.yml`(lane 構成・mise 経由・
非空 rt_ ガード)と `consumer-verify.yml`(access mode・known_hosts
fingerprint pin・証跡テーブル・not-run ≠ pass 強制)。

## Implementation

### Changes

- `.github/workflows/ci.yml`: PR(main 向け)/push(main)/dispatch
  trigger、`contents: read`、concurrency、6 job。各 lane に
  dep-access 呼出・`if: always()` の証跡 step と credential cleanup
  を配置。evidence job は lane 結果・commit・ruviform resolved
  SHA を集約し、未導入 lane(RF-GT-01〜05/frontend/実機/Visual/
  AudioLens)を NotRun 行として固定記録。全 not-run または 1 lane
  でも failure/cancelled の run は fail。
- `.github/actions/ruviform-dep-access/action.yml`: input
  `deploy-key`(=`secrets.CROSS_REPO_DEPLOY_KEY`)があれば SSH
  deploy key 経路(host key fingerprint 3 種 pin・url.insteadOf・
  GIT_SSH_COMMAND・cygpath 正規化)、なければ匿名 HTTPS probe、
  両方不可なら fail。
- `mise.toml`: `actionlint = "1.7.12"`、`[tasks.test-rt]`
  (`cargo test --workspace --locked rt_`)、`[tasks.lint-actions]`
  (`actionlint`)。`mise.lock` 再生成済み。
- 既存 2 設計文書の末尾改行補正: prek `end-of-file-fixer` による
  自動修正。本 CI の verify lane が `prek run --all-files` を
  実行するため、この修正が無いと初回 run が既存文書で fail
  する。CI 導入の必須前提として同梱。

### Decisions During Implementation

- dep access を preflight job ではなく composite action にした:
  secrets は job output で安全に受け渡せず lane ごとの再解決が
  必要なため。consumer-verify は not_run を許容する横断 lane だが、
  独立 CI では依存取得不能は即 fail が正しい。
- `rt_` フィルタは部分文字列一致のため、厳密接頭辞
  `(^|::)rt_[^ ]+: test$` の非空ガードを前段に置く(ruviform
  rt-verify と同型)。現在 2 件
  (rt_control_eval_and_lower_alloc_free・rt_path_is_alloc_and_drop_free)。
- gw10 は OS 依存分岐のないコード構成のため 3-OS matrix とし、
  transport/recovery 経路の OS 差異を実測する。
- 実装上のバグを自己レビューで修正: composite action 内で
  `GITHUB_ENV` へ書き込んだ `GIT_SSH_COMMAND` は同 step 内の
  `git ls-remote` に効かないため `export` を併記。Windows での
  `$RUNNER_TEMP` は Windows パス形式のため cygpath で正規化。

## Related Experiments

### Adopted

- なし

### Rejected

- なし

## Validation

実行済み(ローカル Windows、rust 1.99.0):

- `mise run fmt-check` — PASS
- `mise run check` — PASS
- `mise run lint` — PASS(-D warnings)
- `mise run test` — PASS(全 workspace test、0 failed)
- `mise run gw10` — PASS(G0〜GTV 全 gate PASS・strict report
  exit 0。SYS-02-ux/SYS-03-ux/SYS-05-powerloss/CONSUMER-* は
  NOT-RUN、SYS-07-rt は UNSUPPORTED として記録)
- `mise run test-rt` — PASS(rt_ 部分一致で 2 strict rt_ test を
  含む選択が実行)
- `mise run lint-actions`(actionlint)— PASS(ci.yml)
- `mise exec -- prek run --all-files` — PASS(既存 2 文書の
  end-of-file 修正を含んだ上で全 hook PASS)
- rt_ 非空ガード — `cargo test --locked rt_ -- --list` で
  strict `rt_` prefix が 2 件であることを確認
- dep-guard 系ロジック単体試験 — manifest ガード: 現行 PASS /
  rev 注入で検出。単一 SHA ガード: 現行(21 source・単一 SHA
  `073a3ff6`)で PASS・2 SHA で fail・0 source で fail。
- dep-access 匿名経路 — `git ls-remote https://…ruviform.git`
  HEAD が anonymous で解決し `mode=anonymous` を出力(実 repo に
  対して実行)。
- `mise run test-msrv` — PASS(rust@1.89.0 で `cargo test
  --workspace --locked`、0 failed。MSRV gate が実 toolchain で
  機能することを実測)
- doc chain audit — `audit_docs.js` で新規 6 文書の findings 0、
  `doc_maintenance.js` で managed index 6 件再生成、
  `build_task_graph.js` で task graph 構築成功。

未検証:

- GitHub runner 上の実挙動全般(action/checkout・mise-action・
  cache・runner 上の mise install 解決)。構文と参照関係のみ
  actionlint で検証済み。
- macOS/ubuntu lane の実測(ローカルは Windows のみ)。
- deploy_key mode の実動作(`CROSS_REPO_DEPLOY_KEY` 未設定のため
  SSH 経路はコードレビューのみ)。

## Risks

- macOS lane が初回 run で platform finding を出す可能性
  (fail-fast: false で他 OS 証跡は残る)。
- host key fingerprint 定数は GitHub 公表値。rotation 時は
  deploy_key mode が fail する(error message で明示)。
- fork からの PR では secrets が空になり anonymous mode になる
  — public repo の間は正常経路。

## Follow-ups

- PR オープン後、初回 run の lane 結果・所要時間・証跡出力を観測し、
  Issue #83 の Acceptance Gates に沿って報告する。
- ruviform #310〜#314 の共有 harness 公開時に RF-GT lane を本
  workflow へ接続する(設計の Deferred Design Concerns 参照)。
- frontend(SolidJS+TS)導入時に SolidJS lane を追加する
  (Synvelis 管理、Ruviform 共通 CI には必須化しない)。
- `CROSS_REPO_DEPLOY_KEY` を repo settings に設定して private
  復帰に備える(public 期は任意)。
