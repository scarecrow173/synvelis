---
id: "TASK-034RGXoZuTHIczgPxzHY1W"
type: "task"
status: "done"
title: "Issue #151 T1 ui-verify lane + install-ui freshness 追加"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/151"
  changes:
    added: []
    modified:
      - type: "file"
        path: ".github/workflows/ci.yml"
      - type: "file"
        path: "mise.toml"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-151-ui-ci-lane.md"
  implemented-by:
    - "docs/impl/ir/issue-151-ui-verify-lane.md"
  depends-on:
    - "docs/plans/issue-151-ui-ci-lane.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-151-ui-ci-lane.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-151-ui-ci-lane.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #151 T1 ui-verify lane + install-ui freshness 追加

## Work

`mise.toml` と `.github/workflows/ci.yml` の 2 ファイルを変更する。

- `mise.toml`: `install-ui` task(`npm --prefix ui ci --no-audit
  --no-fund`、`sources = ["ui/package-lock.json", "ui/package.json"]`、
  `outputs = ["ui/node_modules/.package-lock.json"]` で freshness
  check)を新設。`setup-ui` を `depends = ["install-ui"]` の集約入口に
  変更し、`check-ui`/`test-ui`/`build-ui`/`dev-ui`/`build-app` に
  `depends = ["install-ui"]` を付与する(ruviform #330 パターン)。
- `ci.yml`: `msrv-verify` と `dep-guard` の間に `ui-verify` job を追加。
  3 OS matrix・`fail-fast: false`・timeout 30 分・job env
  `MISE_ENABLE_TOOLS=rust,git-lfs,node`。step 順は
  checkout → mise → `mise install --locked rust git-lfs node` →
  dep-access → cargo cache → `mise exec -- cargo fetch --locked` →
  `mise run sync-ui-deps` → `mise run setup-ui` → `check-ui` →
  `test-ui` → `build-ui` → 証跡(`always()`)→ credential cleanup
  (`always()`)。`evidence` job の `needs`・証跡テーブル・fail 判定に
  `ui-verify` を追加し、stale な「SolidJS+TS build/typecheck/component
  tests | NotRun」行を除去する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/issue-151-ui-ci-lane.md`
- `docs/designs/issue-151-ui-ci-lane.md`
- `docs/specs/issue-151-ui-ci-lane.md`

## Dependencies

- なし(単一 task)。

## Affected Files

- `mise.toml`(変更)
- `.github/workflows/ci.yml`(変更)

## Done When

- [x] `ui-verify` が 3 OS matrix・fail-closed で定義され、検証は mise
      task 委譲・dep-access/cleanup/証跡が既存 lane と同型である
- [x] `install-ui` が npm ci を唯一所有し、`setup-ui` と 5 つの npm
      消費 task が freshness depends で再利用する
- [x] `evidence` の `needs`/判定/証跡に `ui-verify` が含まれ、stale な
      NotRun 行が除去されている
- [x] required contexts 10 件の job 名が変更・削除されていない

## Verification

- [x] `ui/node_modules` 削除後に `mise run check-ui` が install-ui
      経由で完走し、続く `test-ui`/`build-ui` で install-ui が
      freshness skip される
- [x] `mise run check-ui` / `test-ui` / `build-ui` が worktree で pass
- [x] `mise run lint-actions` と `mise exec -- prek run --all-files` pass
- [ ] PR で `ui-verify` 3 leg + `evidence` が緑(`gh pr checks --watch`)
