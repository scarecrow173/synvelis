---
id: "TASK-034T2c6ZLwPXf10mPcW2MC"
type: "task"
status: "done"
title: "SYN-IO-02 task1 store.rs StagedFile 移行"
created: "2026-11-06"
updated: "2026-11-06"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/84"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/syn-io-02-directory-bundle-store-stagedfile.md"
  implemented-by: []
  depends-on:
    - "docs/plans/syn-io-02-directory-bundle-store-stagedfile.md"
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
    - "docs/test-specs/syn-io-02-store-publish-contract.md"
  references: []
  defers: []
  deferred-by: []
---

# SYN-IO-02 task1 store.rs StagedFile 移行

## Work

`crates/synvelis-project/src/store.rs` の `write_atomic` を
`ruviform_io_file` staged publish へ移管する。manifest 2 箇所に
`ruviform-io-file` 依存を追加し、`Cargo.lock` を再生成。
`crates/synvelis-project/tests/store_publish.rs` に test-spec の
E1–E11 契約テストを実装する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/syn-io-02-directory-bundle-store-stagedfile.md`
- `docs/designs/syn-io-02-directory-bundle-store-stagedfile-design.md`
- `docs/specs/syn-io-02-snapshot-head-staged-publish.md`

## Dependencies

- ruviform-io-file 公開 API（`StagedFile`/`SealedFile`/
  `PublishOptions`/`PublishMode`/`Durability`/`PublishEffect`/
  `PublishFailure`/`RecoveryHandle`）— git dep、Cargo.lock pin。

## Affected Files

- `Cargo.toml`（workspace.dependencies）
- `crates/synvelis-project/Cargo.toml`
- `Cargo.lock`
- `crates/synvelis-project/src/store.rs`
- `crates/synvelis-project/tests/store_publish.rs`（新規）

## Done When

- [x] `write_atomic`/独自 `.tmp`/`fs::rename`/`sync_all` 直書きが
  `synvelis-project` に残らない。
- [x] snapshot=CreateNew・HEAD=Replace・FileAndDirectorySync で
  publish される。
- [x] `PublishFailure` → `Error::Store` 写像が分類情報を転写し、
  Indeterminate の temp を自動削除しない。
- [x] test-spec E1–E11 の契約テストが pass。
- [x] `mise run fmt-check`/`check`/`lint`/`test` が pass。

## Verification

- [x] `mise run test`（cargo test --workspace --locked）で新規
  契約テストを含む workspace 全緑。
- [x] `git diff` に `store.rs`・manifest・lock・テスト・docs のみ。
