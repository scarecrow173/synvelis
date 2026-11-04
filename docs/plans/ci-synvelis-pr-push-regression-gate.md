---
id: "PLAN-034RoZox0pXYihUzci5UC8"
type: "plan"
status: "approved"
title: "Synvelis 独立 CI 実装計画"
created: "2026-11-04"
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
    - "docs/specs/ci-synvelis-pr-push-regression-gate.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/ci-synvelis-pr-push-regression-gate.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/ci-synvelis-pr-push-regression-gate.md"
  references: []
  defers: []
  deferred-by: []
---

# Synvelis 独立 CI 実装計画

## Goal

`.github/workflows/ci.yml` + `ruviform-dep-access` composite action +
`mise.toml` 追加(test-rt / actionlint / lint-actions)で、Issue #83 の
PR/Push 独立 CI を構築する。

## Scope

単一実装ストリーム。境界: `.github/workflows/ci.yml`・
`.github/actions/ruviform-dep-access/action.yml`・`mise.toml`・
docs チェーンのみ。Rust コード・Cargo.toml・Cargo.lock・
`.pre-commit-config.yaml`・apm.yml には触れない。

## Upstream Documents

- `docs/specs/ci-synvelis-pr-push-regression-gate.md`

## Design Inputs

- `docs/designs/ci-synvelis-pr-push-regression-gate.md`

## File Map

- `.github/workflows/ci.yml` — trigger/permissions/concurrency/6 lane job 定義。
- `.github/actions/ruviform-dep-access/action.yml` — access mode 解決
  (anonymous HTTPS / deploy_key + host key pin + url.insteadOf)。
- `mise.toml` — `[tools]` へ `actionlint` 追加、`[tasks.test-rt]`・
  `[tasks.lint-actions]` 追加。
- `mise.lock` — actionlint 追加に伴う再生成(`mise install`)。
- `docs/specs|designs|plans|test-specs|tasks/ci-*` — 本チェーン文書。

## Implementation Order

- [ ] `mise.toml` に actionlint tool・`test-rt`・`lint-actions` を追加
- [ ] `mise install` で `mise.lock` 再生成、差分確認
- [ ] `ruviform-dep-access` composite action 作成
- [ ] `ci.yml` 作成(verify/rt-verify/gw10-verify/msrv-verify/dep-guard/evidence)
- [ ] actionlint + prek(check-yaml)で構文検証
- [ ] ローカル mise 検証(fmt-check/check/lint/test/gw10)とガード系
      シェルロジックの単体試験
- [ ] `git merge origin/main` 後の最終検証・コミット

## Dependencies and Risks

- actionlint の mise 登録: `minimum_release_age = "7d"` を満たす公開
  版を pin する。最新版が 7 日未満なら 1 つ前の安定版を選ぶ。
- macOS lane はローカル検証不可 → fail-fast: false で初回 run を
  platform finding として扱う設計に留める。
- `secrets.CROSS_REPO_DEPLOY_KEY` 未設定でも anonymous mode で全 lane
  が動くことを、public repo 前提で維持する。

## Task Breakdown

- TASK1: mise 基盤追加(test-rt / actionlint / lint-actions / lock 再生成)
- TASK2: dep-access composite action + ci.yml 6 lane 実装
- TASK3: 検証(actionlint・prek・ガード試験・ローカル mise)・証跡確認・
  review-protocol 自己レビュー・コミット

## Verification

- [ ] `mise exec -- actionlint` が `.github/workflows/` を PASS
- [ ] `mise exec -- prek run --all-files` が PASS(check-yaml 含む)
- [ ] ガード系ロジック単体試験: 正常 Cargo.lock で PASS / rev 混在
      manifest と複数 SHA lock で fail
- [ ] `mise run fmt-check`・`mise run check`・`mise run lint`・
      `mise run test`・`mise run gw10` が PASS
- [ ] `rt_` 非空ガードが 2 件を計数(0 件は fail)
- [ ] 証跡テーブルが run_id/commit/ruviform SHA/OS/toolchain/lane 結果
      と NotRun 行を含む(構造レビュー)

## Open Questions / Gaps

- GitHub 上での初回実行結果(runner 実測・macOS lane)は本 worktree
  からは確認不可。PR オープン後の run 観測が別途必要。
- `CROSS_REPO_DEPLOY_KEY` の secret 実設定は repo 設定作業であり
  コード外。public 期は不要。

## Self-Review

- [ ] spec の要求がすべて task に対応している
- [ ] placeholder が残っていない
- [ ] file path、relation、status、用語が整合している

## Implementation Handoff

- 推奨方式: inline 実行(単一 worker で完結する範囲)
- 独立して進められる作業ストリーム: なし(TASK1→2→3 は依存順)
- dispatch 前のユーザー承認: 本タスクは supervised worker の dispatch
  済み scope 内として扱う

## Review Handoff

- 注目点: `secrets` context を composite action 内で直接参照せず
  inputs 経由にしているか、証跡ゼロ fail 強制が正しく `needs` 結果を
  見ているか、`--locked`/単一 SHA ガードが fail-closed か。
