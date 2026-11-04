---
id: "TASK-034Rp07EGtrO1nUIIjiLh8"
type: "task"
status: "done"
title: "ci.yml + dep-access composite action 実装"
created: "2026-11-03"
updated: "2026-11-04"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/ci-synvelis-pr-push-regression-gate.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/ci-task1-mise-tasks.md"
  blocks:
    - "docs/tasks/ci-task3-verify-review.md"
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
---

# ci.yml + dep-access composite action 実装

## Work

設計文書の lane 構成どおりに 2 ファイルを作成する。

- `.github/actions/ruviform-dep-access/action.yml`: composite action。
  input `deploy-key` に secret を受け取り、deploy_key 経路(SSH key
  + host key fingerprint pin + `url.insteadOf` + `GIT_SSH_COMMAND`)
  または anonymous HTTPS 経路を解決し、`outputs.mode` を返す。
  両方不可なら `::error::` + fail。
- `.github/workflows/ci.yml`: trigger(PR/push/dispatch)・
  `permissions: contents: read`・concurrency・
  `MISE_ENABLE_TOOLS=rust,prek,git-lfs`・`CARGO_INCREMENTAL=0`・
  `CARGO_NET_GIT_FETCH_WITH_CLI=true`・`GIT_LFS_SKIP_SMUDGE=1`。
  lane: `verify`(3 OS matrix: prek→fmt-check→check→lint→test→
  Cargo.lock diff guard)、`rt-verify`(非空ガード→test-rt)、
  `gw10-verify`(3 OS matrix: mise run gw10)、`msrv-verify`
  (rust@1.89.0 install→test-msrv)、`dep-guard`(cargo fetch
  --locked・manifest 表記ガード・単一 SHA ガード・ruviform_sha
  output)、`evidence`(needs 全 lane・always()・証跡テーブル・
  NotRun 行・証跡ゼロ fail)。各 lane に dep-access 呼出と
  `always()` cleanup を置く。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/ci-synvelis-pr-push-regression-gate.md`
- `docs/designs/ci-synvelis-pr-push-regression-gate.md`

## Dependencies

- `docs/tasks/ci-task1-mise-tasks.md`(test-rt / lint-actions が
  ci.yml から参照される)

## Affected Files

- `.github/workflows/ci.yml`(新規)
- `.github/actions/ruviform-dep-access/action.yml`(新規)

## Done When

- [ ] 6 lane が設計どおり定義され、mise タスクへ委譲している
- [ ] dep-access が 2 mode + fail 経路を持ち secret を inputs 経由で
      受け取る
- [ ] 証跡テーブルと NotRun 行・証跡ゼロ fail 強制がある
- [ ] 秘密情報を出力しない・`always()` cleanup がある

## Verification

- [ ] `mise exec -- actionlint` PASS
- [ ] `mise exec -- prek run --all-files` PASS(check-yaml)
- [ ] ガード系シェルロジックをローカルで単体試験(正常系 PASS /
      異常系 fail)
