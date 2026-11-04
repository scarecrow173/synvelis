---
id: "DISC-034RGX4tzLnhJHNrXshLpE"
type: "discovery"
status: "resolved"
title: "Issue #151 synvelis CI ui/ 検証 lane 欠如 gap 調査"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/151"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/specs/issue-151-ui-ci-lane.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/ci-synvelis-pr-push-regression-gate.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/issue-151-ui-ci-lane.md"
    - "docs/designs/issue-151-ui-ci-lane.md"
  verifies: []
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
    - "tools/sync-ruviform-ui.mjs"
    - "ui/package.json"
    - "ui/tsconfig.json"
  defers: []
  deferred-by: []
---

# Issue #151 synvelis CI ui/ 検証 lane 欠如 gap 調査

## Summary

SV-GUI-02 (PR #146) merge 後、main の `mise run check-ui` が
`ui/test/binding.test.ts` の型エラー(TS2379/TS2345 ×5)で fail していたが、
synvelis CI には ui/ 系 lane が存在せず未検出で混入した。型エラー本体は
PR #150 で修正済みであり、残る gap は「`check-ui`/`test-ui`/`build-ui` を
実行する CI lane が無い」こと自体。結論: `ui-verify` lane を fail-closed で
追加する(confirmed)。

## Exploration Goal

Issue #151 が指摘する coverage gap を確認し、lane 追加に必要な前提
(vendor 同期・npm 依存導入・dep access・OS 選択・required contexts との
関係)を確定させる。

## Key Issues and Assumptions

- `check-ui`/`test-ui`/`build-ui` は `ui/node_modules` と
  `ui/vendor/ruviform`(Cargo.lock pin の cargo checkout 由来)を前提とする。
  lane 内で `cargo fetch --locked` → `sync-ui-deps` → `setup-ui`(npm ci)を
  先に実行しないとタスク自体が起動しない(confirmed: 本 worktree でも
  node_modules 未導入時は tsc が解決不能で fail)。
- vendor 同期は `~/.cargo/git/checkouts` を参照するため rust toolchain と
  ruviform への read access(dep-access composite action)が必要(confirmed:
  `tools/sync-ruviform-ui.mjs` L55-73)。
- ruviform #330 (CI-OPT-03) で導入された「package-lock ごとに npm ci を
  job 内一度だけ実施し、verify task は freshness-checked install task の
  depends で再利用する」パターンに整合させる。synvelis の lockfile は
  `ui/package-lock.json` 1 件のみ(confirmed)。
- 既存 required contexts(10 件: verify×3, rt-verify, gw10-verify×3,
  msrv-verify, dep-guard, evidence)を弱めない。新 lane は `evidence` の
  `needs` と not-run-is-not-pass 判定に組み込み、required 化は既存の
  evidence 集約経由で実現する(confirmed: `ci.yml` L452-518)。

## Alternatives and Comparison

| Option | Pros | Cons | Lean |
| --- | --- | --- | --- |
| 3-OS matrix の `ui-verify` job | verify/gw10-verify・ruviform ui-verify と構成一致。path 区切り・EOL・npm プラットフォーム差を検出 | runner 時間 ×3 | yes |
| ubuntu-only 単発 job | 最小 runner コスト | repo の主要 lane 構成・ruviform 先例と不一致。Windows/mac でのみ崩れる npm/native 依存差を見落とす | no |
| 既存 `verify` job へ step 追加 | job 数を増やさない | node_modules 導入が Rust 検証 critical path を延長し、失敗箇所の分離が失われる | no |
| lane を `continue-on-error` にする | 導入時の不安定を許容 | fail-open は Issue 明記の禁止条件。未検出混入の再発 | no |

## Tentative Conclusions and Hypotheses

- confirmed: lane は `ui-verify (ubuntu/windows/macos)` の独立 job とし、
  `cargo fetch --locked` → `sync-ui-deps` → `setup-ui` →
  `check-ui`/`test-ui`/`build-ui` の順で実行する。
- confirmed: npm install は ruviform #330 同様に freshness-checked
  `install-ui` task へ集約し、`setup-ui` を集約入口、各 verify task は
  `depends = ["install-ui"]` で再利用する。CI では `setup-ui` を前段で
  一度だけ実行し、後続 task の depends は freshness skip される。
- confirmed: `evidence` job の `needs`・証跡テーブル・fail 判定へ
  `ui-verify` を追加し、stale 化した「SolidJS+TS build/typecheck/
  component tests | NotRun」行を除去する。

## Open Questions

- なし(実装要件は Issue 本文・AGENTS.md・ruviform #330 設計で確定済み)。

## Promotion Candidates

- [x] spec-doc needed: `docs/specs/issue-151-ui-ci-lane.md`
- [ ] adr-doc needed: なし(既存 CI 設計 `docs/designs/ci-synvelis-pr-push-regression-gate.md` の枠組み内の lane 追加であり、新規の architecture decision を要しない)

## Sources and Relations

- Issue: https://github.com/scarecrow173/synvelis/issues/151
- 上流 CI 設計: `docs/designs/ci-synvelis-pr-push-regression-gate.md` /
  `docs/plans/ci-synvelis-pr-push-regression-gate.md` (Issue #83)
- 整合対象: ruviform CI-OPT-03 https://github.com/scarecrow173/ruviform/issues/330
  (install-ui-* freshness task + setup-ui 集約 + ui-verify 前段で npm ci 一度)
- 関連: ruviform CI-OPT-00 https://github.com/scarecrow173/ruviform/issues/327
  (必須チェック集合 Baseline 化)

## Evidence

- 2026-11-04 worktree 実測: `ui/node_modules` 未導入のまま `npm --prefix ui
  run typecheck` は vendor エイリアス解決不能で fail。`mise run setup-ui`
  (npm ci, ~7.3s) 後に `check-ui`(~3.8s)/`test-ui`(37 tests, ~3.0s)/
  `build-ui`(~1.8s)はすべて pass。`ui/vendor/ruviform/REV` は Cargo.lock
  の ruviform resolved SHA `8fb665f1…` と一致。
- `.github/workflows/ci.yml` 現行 lane: verify(3 OS)/rt-verify/gw10-verify
  (3 OS)/msrv-verify/dep-guard/evidence。ui/ 系 step を含む lane は無い。
- evidence job の NotRun 表に「SolidJS+TS build/typecheck/component tests |
  NotRun | frontend workspace 未導入 (#110〜)」行が残っているが、
  SV-GUI-01/02 で frontend は導入済みであり、本 lane 追加で実際に実行
  されるようになるため除去対象。
