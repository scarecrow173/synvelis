---
id: "IMPL-034RGXq9DiVrM6SNyBhyQs"
type: "impl"
status: "completed"
title: "Issue #151 ui-verify lane 実装記録"
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
      - type: "file"
        path: "docs/discovery/issue-151-ui-ci-lane-gap.md"
      - type: "file"
        path: "docs/specs/issue-151-ui-ci-lane.md"
      - type: "file"
        path: "docs/designs/issue-151-ui-ci-lane.md"
      - type: "file"
        path: "docs/plans/issue-151-ui-ci-lane.md"
      - type: "file"
        path: "docs/test-specs/issue-151-ui-ci-lane.md"
      - type: "file"
        path: "docs/tasks/issue-151-t1-ui-verify-lane.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/discovery/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/tasks/issue-151-t1-ui-verify-lane.md"
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
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - "mise.toml"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# Issue #151 ui-verify lane 実装記録

## Summary

`synvelis` CI に `ui-verify` lane(3 OS matrix・fail-closed)を追加し、
`check-ui`/`test-ui`/`build-ui` を PR/main push/dispatch で実行可能にした。
npm 依存導入は ruviform #330 と同じ freshness-checked `install-ui` task に
集約し、job 内で一度だけ実行される。`evidence` 集約に lane を組み込み、
既存 required contexts(10 件)は変更していない。

## Context

`docs/tasks/issue-151-t1-ui-verify-lane.md` →
`docs/plans/issue-151-ui-ci-lane.md` →
`docs/designs/issue-151-ui-ci-lane.md` →
`docs/specs/issue-151-ui-ci-lane.md`(← discovery gap 調査)のチェーンに
従って実装。Issue #151(SV-GUI-02 型エラー未検出混入の coverage gap)の
対応。

## Implementation

### Changes

- `mise.toml`:
  - `install-ui` task を新設。`sources = ["ui/package-lock.json",
    "ui/package.json"]` / `outputs =
    ["ui/node_modules/.package-lock.json"]` で mise freshness check を
    有効化し、`npm --prefix ui ci --no-audit --no-fund` を唯一所有。
  - `setup-ui` を `depends = ["install-ui"]` の集約入口へ変更(task 名・
    役割は維持し呼び出し側互換を保持)。
  - `check-ui`/`test-ui`/`build-ui`/`dev-ui`/`build-app` に
    `depends = ["install-ui"]` を付与し、node_modules 未導入のまま
    検証 task が起動しない境界を task 層で保証。
- `.github/workflows/ci.yml`:
  - `ui-verify` job を `msrv-verify` と `dep-guard` の間に追加。
    3 OS matrix・`fail-fast: false`・timeout 30 分・job env
    `MISE_ENABLE_TOOLS=rust,git-lfs,node`(workflow 既定値は不変)。
    step は既存 lane と同型: checkout → mise → `mise install --locked
    rust git-lfs node` → `ruviform-dep-access` → cargo cache →
    `mise exec -- cargo fetch --locked` → `mise run sync-ui-deps` →
    `mise run setup-ui` → `mise run check-ui`/`test-ui`/`build-ui` →
    `always()` 証跡 → `always()` credential cleanup。
  - `evidence` job: `needs` に `ui-verify` を追加、証跡テーブルに
    3 OS 行を追加、not-run-is-not-pass 判定の results へ
    `needs.ui-verify.result` を追加。実体のある lane となった
    「SolidJS+TS build/typecheck/component tests | NotRun」stale 行を除去。

### Decisions During Implementation

- OS matrix: 3 OS(ubuntu/windows/macos)を選択。frontend は platform 中立
  だが、repo の主要 lane(verify/gw10-verify)と ruviform ui-verify の
  構成に合わせ、npm platform 差・path/EOL 差異を検出対象として残す。
- `cargo fetch --locked` を lane 前段に置いた理由: `sync-ruviform-ui.mjs`
  は `~/.cargo/git/checkouts` を参照するため、dep-access 解決後に
  lockfile 整合で checkout を populate する必要がある。
- required contexts へ直接追加しない判断: 新 lane の merge 阻害は
  `evidence`(既存 required)経由で実現。Ruleset 変更を伴わず fail-closed
  を満たす。個別 required 化は Baseline 設計(ruviform #327)側の判断として
  defer(PR 説明に明記)。
- npm cache(actions/cache の node_modules 保存)は不採用: install-ui の
  freshness skip で十分、cache 破損時の復旧経路を増やさない。

## Related Experiments

### Adopted

- なし。

### Rejected

- なし。

## Validation

実行済みで成功した項目のみ記録(2026-11-04・Windows worktree):

- `mise run check-ui` — PASS(`ui/node_modules` 削除後の fresh 状態から
  `install-ui` が npm ci を自動実行して完走、~8.7s)。
- `mise run test-ui` — PASS(5 files / 37 tests、install-ui freshness
  skip 確認: `[install-ui] sources up-to-date, skipping`)。
- `mise run build-ui` — PASS(vite build + manifest 生成)。
- `mise run setup-ui` — PASS(集約入口経由で install-ui freshness skip)。
- `mise run lint-actions`(actionlint)— PASS(ci.yml 変更後)。
- `mise exec -- prek run --all-files` — PASS(8 hook)。
- doc-maintenance: `plan --type all` で issue-151 関連の whitelisted
  repair 0 件。`apply --type adr` で `docs/adr/README.md` の stale index
  を再生成(main 由来の drift、両エントリ表保持のまま解決)。

CI 上での検証(PR #153・run 41814620520、`gh pr checks --watch` で完了確認):

- `ui-verify (ubuntu-latest)` — PASS 39s。ログで dep access
  (anonymous https)→ `synced ruviform ui/* @ 8fb665f168` → npm ci
  `added 101 packages`(setup-ui 一度)→ check/test/build 各 step で
  `[install-ui] sources up-to-date, skipping` → 37 tests pass →
  `wrote synvelis-assets.json` を確認。
- `ui-verify (windows-latest)` — PASS 1m48s。
- `ui-verify (macos-latest)` — PASS 45s。
- `evidence` — PASS 5s。証跡テーブルに ui-verify 3 OS 行が
  `success` で記録され、not-run-is-not-pass 判定に包含されている。

未実行:

- lane 失敗時の `evidence` fail(fault injection を伴う実証は行わず、
  needs + results 判定への包含で構成上の fail-closed を担保)。

## Risks

- 3 OS matrix により PR の runner 時間が増える。実測(PR #153):
  ui-verify 各 leg は 39s〜1m48s で、最長 leg(verify windows 5m20s)を
  下回るため本 run では critical path の wall time 増は無し
  (runner-minutes は +約4分)。
- `install-ui` freshness は node_modules の部分破損を検出しない
  (ruviform #330 と同じ前提。破損時は `npm ci` 手動再実行で復旧)。
- Windows/macOS leg で npm/platform 固有の失敗が出る可能性(matrix で
  検出する対象そのもの)。

## Follow-ups

- ui-verify の required context 直接登録: Baseline 設計(ruviform #327)と
  の整合で別途判断(本変更では `evidence` 集約経由で fail-closed を実現)。
- ブラウザ E2E・Visual Golden 等の残 NotRun lane は evidence 表どおり
  各 Issue(#310〜#314・#114・#115〜#134 系)の責務。
