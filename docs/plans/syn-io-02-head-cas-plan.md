---
id: "PLAN-034T9pLmNvRwXk2jHs7fQ"
type: "plan"
status: "approved"
title: "SYN-IO-02 HEAD CAS expected-head store port 実装計画"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/330"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/syn-io-02-head-cas.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/project-asset-persistence-architecture.md"
    - "PLAN-034WVJFNJkmTS7M5lq1tjI"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/syn-io-02-head-cas.md"
    - "docs/designs/syn-io-02-head-cas-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/syn-io-02-head-cas.md"
  references: []
  defers: []
  deferred-by: []
---

# SYN-IO-02 HEAD CAS expected-head store port Implementation Plan

## Goal

`ProjectStore` port に additive な expected-head CAS
（`save_cas` + `HeadCasError`）を導入し、
`DirectoryBundleStore` を `HEAD.lock` advisory mutex +
consumer 側 compare で実装、`InMemoryStore` で parity を取り、
`ProjectController::save` と gateway journal replay restore を
expected-head 供給へ切り替える。既存 `save` の無条件
semantics・port signature・永続 format は不変。

## Scope

単一実装ストリーム（task1: store/controller/commit 実装 +
契約テスト + doc chain）。変更面は `synvelis-project` の
`store.rs`/`controller.rs`、`synvelis-gateway` の `commit.rs`、
テスト 2 ファイル、docs 8 ファイルに閉じる。ruviform・
`Cargo.lock`・fixtures・docs index は不変。

## Upstream Documents

- `docs/specs/syn-io-02-head-cas.md`（spec）
- Issue #330（Acceptance 正本）・#84（staged publish 前提）

## Design Inputs

- `docs/designs/syn-io-02-head-cas-design.md`（approved —
  機構選択・プロトコル・保証境界の正本）
- `docs/specs/syn-io-01-ruviform-common-io-boundary.md`
  （HEAD CAS = Synvelis 責務・ruviform 非変更の境界）
- `docs/designs/syn-io-02-directory-bundle-store-stagedfile-design.md`
  （publish helper・collision 分類・失敗写像の既存規則）

## File Map

- `crates/synvelis-project/src/store.rs` — `HeadCasError`、
  `ProjectStore::save_cas` default、`publish_snapshot`/
  `publish_head` 分解、`HEAD.lock` acquire/break/fence/
  release、`DirectoryBundleStore::save_cas`、
  `InMemoryStore::save_cas`。
- `crates/synvelis-project/src/controller.rs` — `save` の
  expected-head 供給・`Unsupported` fallback・error 写像。
- `crates/synvelis-gateway/src/commit.rs` — journal replay
  restore の `save_cas` 化（loud failure、fallback 無し）。
- `crates/synvelis-project/tests/head_cas.rs` — 新規契約
  テスト（test-spec Rules/Examples 対応）。
- `crates/synvelis-project/tests/store_publish.rs` — E4 を
  CAS 順序に整合（HEAD 不可読時は CAS が publish 前に失敗
  → publish 契約は `store.save` 直接経路で pin）。
- `docs/specs|designs|plans|tasks|test-specs|impl/ir/
  syn-io-02-head-cas*.md` — doc chain 6 件新規 + syn-io-02
  既存 spec/design の残件整合。

## Implementation Sequence

- [x] `store.rs`: `HeadCasError` + port `save_cas` default +
  `publish_snapshot`/`publish_head` helper 分解 +
  `HEAD.lock` プロトコル + 両 store の `save_cas`。
- [x] `controller.rs`: `saved.head` 由来の expected 供給、
  `Unsupported` fallback、typed error の `Error` 写像。
- [x] `commit.rs`: journal restore を `save_cas` 化。
- [x] `tests/head_cas.rs` 新規 + `store_publish.rs` E4 整合。
- [x] doc chain 作成 + syn-io-02 既存 docs 残件整合。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-app-native`/`lint-app-native`/`test-msrv`/`build` 実走。
- [ ] Conventional Commit → push → 日本語 PR → Issue #330
  コメント → worker_done。

## Dependencies and Risks

- ruviform-io-file の既存 primitive のみ（`CreateNew` の
  `DestinationExists` 分類・`StagedFile` lifecycle）。新規依存
  ・`Cargo.lock` 変更なし。
- advisory lock の residual race（stale 閾値超過 hold）は
  design の保証境界に記録済み — post-publish fencing で
  誤成功を塞ぐが物理的余地は残る。kernel lock 化は ruviform
  境界上の別 Issue 事項。
- `HEAD.lock` は既存 bundle に存在しない新規 file — 旧
  version との混在で `save`（lock 非取得）は影響を受けず、
  `save_cas` 同士のみ排他される。

## Task Breakdown

- `docs/tasks/syn-io-02-head-cas-task.md` — 全実装・テスト・
  検証を含む単一タスク。

## Verification

- [ ] `mise run fmt-check`（cargo fmt --all -- --check）
- [ ] `mise run check`（cargo check --workspace --all-targets）
- [ ] `mise run lint`（cargo clippy --workspace --all-targets
  -D warnings）
- [ ] `mise run test`（cargo test --workspace --locked）
- [ ] `mise run test-rt`（rt_ contract tests）
- [ ] `mise run check-app-native` / `lint-app-native`
- [ ] `mise run test-msrv` / `mise run build`
- [ ] 実 2-process（Windows）= 後続 slice
  `syn-io-02-head-cas-e2e` で実測済み。macOS/Linux・
  電源断 = NotRun（外部 gate）。

## Open Questions / Gaps

- journal restore で `save_cas` が `Unsupported` を返す store
  の扱い — 本実装は loud failure を選択（design 記載）。
  同梱 store は全て CAS 対応のため実経路では発生しない。
- 実 multi-process での lock 奪い合い・stale break の実測 —
  本 slice は Windows 単一 process 内 thread 協調までを
  pin し、実 2-process は後続 slice `syn-io-02-head-cas-e2e`
  で Windows 実測済み（stale break の process 間実測は
  引き続き外部 gate）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are
  consistent.

## Implementation Handoff

- Recommended mode: inline execution（単一タスク・3 source
  file + 2 test file + docs）
- Independent work streams: none
- User approval required before dispatch: worker spec が
  dispatch 済み（Issue #330 Acceptance が正本）

## Review Handoff

- `HEAD.lock` stale break の rename-aside が唯一の仲裁である
  こと、post-publish fencing が indeterminate error を返す
  こと（成功誤報なし）を reviewer が重点確認。
- `Unsupported` fallback を `ProjectController::save` に限定
  し、journal restore では fallback しない非対称の妥当性。
