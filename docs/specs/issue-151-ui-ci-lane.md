---
id: "SPEC-034RGX4X79sB0s9tVVWQh9"
type: "spec"
status: "approved"
title: "Issue #151 CI ui-verify lane 追加仕様"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/151"
    - "docs/discovery/issue-151-ui-ci-lane-gap.md"
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
  implements: []
  implemented-by:
    - "docs/designs/issue-151-ui-ci-lane.md"
    - "docs/plans/issue-151-ui-ci-lane.md"
  depends-on:
    - "docs/discovery/issue-151-ui-ci-lane-gap.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/ci-synvelis-pr-push-regression-gate.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-151-ui-ci-lane-gap.md"
  derived-by:
    - "docs/designs/issue-151-ui-ci-lane.md"
    - "docs/plans/issue-151-ui-ci-lane.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-151-ui-ci-lane.md"
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
    - "tools/sync-ruviform-ui.mjs"
  defers: []
  deferred-by: []
---

# Issue #151 CI ui-verify lane 追加仕様

## What Should Be Built

`.github/workflows/ci.yml` に `ui-verify` lane(3-OS matrix)を追加し、
`mise run check-ui` / `test-ui` / `build-ui` を PR/main push/dispatch の
各トリガーで fail-closed に実行する。lane 前段では
`cargo fetch --locked` → `sync-ui-deps` → `setup-ui`(npm ci 一度だけ)を
行い、mise 側は ruviform #330 パターンの freshness-checked `install-ui`
task へ依存導入を集約する。

## Why Now

SV-GUI-02 (PR #146) で `ui/test/binding.test.ts` の型エラー(TS2379/TS2345)
が UI lane 欠如のため未検出のまま main へ混入した実例がある(Issue #151)。
型エラー本体は PR #150 で修正済みだが、lane が無い限り同型の混入は再発
し得る。ruviform CI-OPT-00 (#327) の必須チェック集合 Baseline 化でも
この抜け検出が目的の一つ。

## Users and Value

- synvelis 開発者: ui/ 変更の型・テスト・ビルド破壊を merge 前に検出できる。
- reviewer/ maintainer: `evidence` の証跡テーブルで ui lane の実行結果を
  他 lane と同じ形式で確認できる。

## Scope

### In Scope

- `ui-verify` job の追加(`ubuntu-latest`/`windows-latest`/`macos-latest`
  matrix、`fail-fast: false`、dep-access 解決、cargo cache、
  `cargo fetch --locked`、`sync-ui-deps`、`setup-ui`、`check-ui`、
  `test-ui`、`build-ui`、証跡記録、credential cleanup)。
- `mise.toml`: `install-ui` task(sources/outputs による freshness
  check)新設、`setup-ui` をその集約入口へ変更、`check-ui`/`test-ui`/
  `build-ui`/`dev-ui`/`build-app` に `depends = ["install-ui"]`。
- `evidence` job: `needs`・fail 判定・証跡テーブルへ `ui-verify` を追加。
- NotRun 表の stale な「SolidJS+TS build/typecheck/component tests」行の除去。

### Out of Scope

- required contexts / Ruleset 定義そのものの変更(新 lane は既存 required
  の `evidence` 集約経由で merge gate に効かせる。直接の required 追加は
  別途判断)。
- npm cache・runner コスト最適化など CI-OPT 系の追加施策(本 Issue は
  coverage gap の解消が目的)。
- ui/ 本体コードの変更・新規 UI 検証(browser E2E 等、ruviform #310 系)。

## Requirements

- lane 失敗時は CI 全体が失敗となること(fail-closed。
  `continue-on-error`・集約ゲート迂回は禁止)。
- lane 内で `sync-ui-deps`(vendor 同期)と npm 依存導入を実行すること。
  依存未導入のまま検証 task を実行しない。
- npm ci は job 内で lockfile ごと一度だけとし、ruviform #330 の
  install-once パターンと整合させること。
- 検証ロジックは mise タスクへ委譲し、workflow 内に cargo/npm の
  検証コマンドを直書きしない(dep access・ガード・証跡の薄いシェルのみ)。
- ローカルの個別 mise task が従来どおり単独実行できること
  (`mise run check-ui` 等の互換性維持。fresh checkout では install-ui が
  自動で依存導入を補う)。

## Acceptance Criteria

- [ ] PR 上で `ui-verify (ubuntu-latest)` / `(windows-latest)` /
      `(macos-latest)` が起動し緑になる。
- [ ] worktree で `mise run check-ui` / `test-ui` / `build-ui` が pass
      する(node_modules 未導入からでも install-ui 経由で完走する)。
- [ ] `mise run lint-actions`(actionlint)と `mise exec -- prek run
      --all-files` が pass する。
- [ ] ui-verify の各 OS leg が失敗した場合、`evidence` が fail し
      required check が赤になる(fail-open でない)。
- [ ] required contexts 10 件の構成を弱めていない(job 名の変更・削除なし)。

## Deferred Design Concerns

- ui-verify を branch protection の required context へ直接追加するかは
  本 Issue では行わず、`evidence` 集約による fail-closed で代替する。
  個別 required 化は Baseline 設計(ruviform #327)との整合で別途判断。
- npm ci の cache(actions/cache による node_modules 保存)は追加しない。
  lockfile freshness skip(#330 パターン)で十分であり、cache 破損時の
  復旧経路を新設するコストを避ける。

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/151
- Discovery: `docs/discovery/issue-151-ui-ci-lane-gap.md`
- 上流: `docs/specs/ci-synvelis-pr-push-regression-gate.md`(Issue #83 系)
- 整合対象: ruviform CI-OPT-03 #330、CI-OPT-00 #327
