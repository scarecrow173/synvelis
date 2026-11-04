---
id: "DESIGN-034RGXfK1v0mkpD0U2xAaN"
type: "design"
status: "approved"
title: "Issue #151 CI ui-verify lane 設計"
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
  implements:
    - "docs/specs/issue-151-ui-ci-lane.md"
  implemented-by:
    - "docs/plans/issue-151-ui-ci-lane.md"
  depends-on:
    - "docs/specs/issue-151-ui-ci-lane.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/ci-synvelis-pr-push-regression-gate.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-151-ui-ci-lane.md"
    - "docs/discovery/issue-151-ui-ci-lane-gap.md"
  derived-by:
    - "docs/plans/issue-151-ui-ci-lane.md"
    - "docs/test-specs/issue-151-ui-ci-lane.md"
  verifies: []
  verified-by: []
  references:
    - ".github/workflows/ci.yml"
    - ".github/actions/ruviform-dep-access/action.yml"
    - "mise.toml"
    - "tools/sync-ruviform-ui.mjs"
    - "ui/package.json"
    - "ui/vite.config.ts"
  defers: []
  deferred-by: []
---

# Issue #151 CI ui-verify lane 設計

## Context

synvelis CI(Issue #83 設計)は `verify`(3 OS)・`rt-verify`・`gw10-verify`
(3 OS)・`msrv-verify`・`dep-guard`・`evidence` の lane を持ち、検証は
mise タスクへ委譲する。しかし SV-GUI-01/02 で導入された `ui/` frontend
lane(`check-ui`/`test-ui`/`build-ui`)は CI に組み込まれておらず、
ui/ の破壊が未検出で merge され得る。本設計はその coverage gap を、
既存 lane 構成・dep-access 契約・ruviform #330 の npm install 集約
パターンに整合させて閉じる。

## Scope

### In Scope

- `ci.yml` への `ui-verify` job 追加(3 OS matrix)と `evidence` job 更新。
- `mise.toml` への `install-ui` freshness task 追加と、npm 消費 task の
  `depends` 配線。

### Out of Scope

- required contexts / Ruleset の変更。ui/ コード・tools スクリプトの変更。
- npm cache・runner 最適化・browser E2E lane。

## Components and Boundaries

- `ui-verify` job(ci.yml): orchestration のみ所有。検証ロジックは mise
  task へ委譲し、workflow 側は dep access・cache・証跡・cleanup の薄い
  シェルのみ持つ(既存 lane と同一責務境界)。
- `install-ui` task(mise.toml): `ui/package-lock.json` への `npm ci`
  を唯一所有する。`sources`(lockfile + package.json)と
  `outputs`(`ui/node_modules/.package-lock.json` — npm が install 時に
  必ず書く hidden lockfile)で mise freshness check され、入力不変の
  再実行は skip される(ruviform #330 と同一機構。synvelis は lockfile
  が 1 件のため task も 1 件)。
- `setup-ui` task: `install-ui` への depends-only 集約入口に変更。
  CI lane は前段でこれを一度だけ呼ぶ。task 名・役割(依存導入入口)は
  維持し、呼び出し側の互換性を保つ。
- npm 消費 task(`check-ui`/`test-ui`/`build-ui`/`dev-ui`/`build-app`):
  `depends = ["install-ui"]` を持ち、fresh checkout でも node_modules
  未導入のまま検証 task が起動しない境界を task 層で保証する。
- `sync-ui-deps` / vendor `--check` の責務は変更しない(vendor tree の
  照合は従来どおり各 task 冒頭の `sync-ruviform-ui.mjs --check`)。

## Data and Control Flow

`ui-verify` job(OS ごと):

1. checkout → mise-action → `mise install --locked rust git-lfs node`
   (job env で `MISE_ENABLE_TOOLS=rust,git-lfs,node` に限定。workflow 既定
   値は他 lane と共有のまま変えない)。
2. `ruviform-dep-access` で read path 解決(anonymous HTTPS または
   deploy key。どちらも不可なら fail — 依存取得不能を緑にしない)。
3. cargo registry/git cache restore → `mise exec -- cargo fetch --locked`
   で `~/.cargo/git/checkouts` を Cargo.lock pin rev で用意。
4. `mise run sync-ui-deps` → `ui/vendor/ruviform` 実体化。
5. `mise run setup-ui` → `install-ui` 経由で `npm ci` を job 内一度だけ実行。
6. `mise run check-ui` → `mise run test-ui` → `mise run build-ui`
   (各 task の `depends: install-ui` は freshness skip される)。
7. `always()` で lane evidence を step summary へ記録し、credential を
   cleanup(他 lane と同一手順)。
8. `evidence` job の `needs` に `ui-verify` を追加し、証跡テーブルに
   3 OS 行、not-run-is-not-pass 判定に `needs.ui-verify.result` を加える。
   いずれかの leg が failure/cancelled なら `evidence` が fail し、
   required context 経由で merge が block される(fail-closed)。

## Non-Functional Constraints

- fail-closed: `continue-on-error`・matrix `fail-fast` による証跡喪失・
  evidence 判定からの除外はいずれも導入しない。
- timeout: 30 分(verify 40 分より小。npm ci ~10s 規模 + tsc/vitest/vite
  各数秒〜数十秒 + cargo fetch。Windows の npm は遅めでも十分な余裕)。
- 秘密情報: deploy key は既存 composite action 経由のみ。cleanup は
  `always()` で必ず実行。
- 再現性: `cargo fetch --locked`・`npm ci`(package-lock 固定)・
  `mise install --locked` の pinned toolchain。`GIT_LFS_SKIP_SMUDGE=1` は
  workflow 既定のまま(ruviform ui/* は LFS 非対象のため vendor 照合に
  影響しない)。

## Risks and Trade-offs

- 3-OS matrix は runner 時間 ×3。frontend は platform 中立的だが、npm の
  platform 別 optional deps・path 区切り・EOL 差異の検出価値と、
  verify/gw10-verify/ruviform ui-verify との構成一致を優先する。
- `install-ui` freshness は `ui/node_modules/.package-lock.json` の
  存在/時系列に依存し、node_modules 部分破損は検出しない。
  受け入れ可能な trade-off(ruviform #330 と同じ前提。破損時は
  `npm ci` 手動再実行で復旧)。
- lane 追加により PR の wall time が伸びる(critical path に ui-verify が
  乗る場合)。Before/After を PR で正直に記録する。

## Deferred Design Concerns

- ui-verify の required context 直接登録: `evidence` 集約で代替するため
  本変更では行わない(再検討条件: Baseline 設計 #327 で lane 個別必須化が
  求められた場合)。
- npm cache / node_modules キャッシュ: freshness skip で十分なため不採用
  (再検討条件: npm ci が critical path のボトルネックと計測された場合)。

## Verification Notes

- [ ] `mise run lint-actions`(actionlint)が ci.yml 変更後も pass。
- [ ] `node_modules` 削除後に `mise run check-ui` が install-ui 経由で
      完走し、再実行時は install-ui が freshness skip されること。
- [ ] worktree で `check-ui`/`test-ui`/`build-ui` pass。
- [ ] PR で `ui-verify` 3 leg が起動し緑、`evidence` 証跡に ui-verify 行
      が出ること。
