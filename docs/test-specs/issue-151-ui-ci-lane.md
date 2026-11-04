---
id: "TSPEC-034RGXpDyOEiejVT0jxK7a"
type: "test-spec"
status: "approved"
title: "Issue #151 ui-verify lane 検証仕様"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/151"
    - "docs/specs/issue-151-ui-ci-lane.md"
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
  implemented-by: []
  depends-on:
    - "docs/specs/issue-151-ui-ci-lane.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-151-ui-ci-lane.md"
    - "docs/plans/issue-151-ui-ci-lane.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-151-ui-ci-lane.md"
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
  defers: []
  deferred-by: []
---

# Issue #151 ui-verify lane 検証仕様

## Purpose

Issue #151 が要求する「ui/ 変更を CI coverage に含める」不変条件を pin する。
lane 実装の形が将来変わっても(分割・cache 追加等)、ここに挙げる
保証は維持されなければならない。CI-OPT 系の別設計で lane 構成が
再編された場合に退役・移譲を検討する。

## Feature

ui/ frontend の型・テスト・ビルドを PR/main push/workflow_dispatch で
fail-closed に検証する `ui-verify` CI lane。

## Rules

- Rule 1: `check-ui`/`test-ui`/`build-ui` が CI 上で実際に実行される。
  NotRun・skipped・常時緑のダミー step は許容しない。
- Rule 2: lane は fail-closed。`continue-on-error`、evidence 集約からの
  除外、失敗を握りつぶすシェルは存在しない。
- Rule 3: 検証 task 実行前に `ui/vendor`(sync-ui-deps 由来)と
  `ui/node_modules`(npm ci 由来)が用意される。依存未導入のまま
  task を起動しない。
- Rule 4: npm ci は job 内で `ui/package-lock.json` ごと一度だけ
  (install-ui freshness task + setup-ui 集約。ruviform #330 整合)。
- Rule 5: ローカルで `mise run check-ui`/`test-ui`/`build-ui`/`setup-ui`
  が従来どおり単独実行できる(互換性)。fresh checkout 相当状態
  (node_modules 無し)でも install-ui depends により完走する。
- Rule 6: required contexts 10 件の job 名は変更・削除されない。
  ui-verify の merge 阻害力は `evidence`(既存 required)経由。

## Examples

- ui/ の型エラーを含む PR → `ui-verify` の `check-ui` step が fail →
  `evidence` が fail → merge block(SV-GUI-02 事案の再現条件で緑に
  ならないこと)。
- `ui/node_modules` を削除した worktree で `mise run check-ui` →
  `install-ui` が npm ci を実行 → vendor 照合 + tsc 完走。
- `mise run setup-ui` 直後に `mise run check-ui` → `install-ui` が
  freshness skip され、npm ci は二度走らない。
- PR の checks 一覧に `ui-verify (ubuntu-latest)` /
  `(windows-latest)` / `(macos-latest)` の 3 context が現れる。

## Guarantees

- ui/ の型エラー・vitest 失敗・vite build 失敗は PR 上で検出される。
- lane のどの段階(fetch/sync/install/check/test/build)が失敗したか
  step 単位で特定でき、lane evidence に行として残る。
- 新 lane は既存 required contexts を弱めず、CI の not-run-is-not-pass
  判定に包含される。

## Non-goals

- browser E2E・実 WebView 起動・visual golden(ruviform #310 系・
  synvelis #114 系の責務。evidence の NotRun 表に残る)。
- 依存脆弱性スキャン・npm audit の CI ゲート化。
- ruviform repo 自体の CI 変更(#330 は ruviform 側で完了済み。

## Risk

- これらの保証を落とすと、SV-GUI-02 型の未検出混入が再発する。
  特に fail-open 化・evidence 除外・install 手順省略のいずれか一つで
  coverage gap が復活する。
