---
id: "TASK-034T2vRnB8yK3mXw7LpQ"
type: "task"
status: "done"
title: "SYN-IO-02 HEAD CAS store port 実装"
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
    - "docs/plans/syn-io-02-head-cas-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/syn-io-02-head-cas-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "TASK-034WVJP0tzo7lASwFwzkpG"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/syn-io-02-head-cas.md"
  references: []
  defers: []
  deferred-by: []
---

# SYN-IO-02 HEAD CAS store port 実装

## Work

`ProjectStore` に additive な `save_cas`（default
`HeadCasError::Unsupported`）と typed `HeadCasError` を追加。
`DirectoryBundleStore` は `HEAD.lock` の `CreateNew` mutex +
stale-lock rename-aside break + token 照合 fencing で
consumer 側 CAS を実装し、`InMemoryStore` は同一契約の
parity を持つ。`ProjectController::save` は `saved.head` を
expected に供給し `Unsupported` のみ legacy `save` へ
fallback。`synvelis-gateway` の journal replay restore を
`save_cas` 化する。`tests/head_cas.rs` に契約テストを実装し、
`tests/store_publish.rs` E4 を CAS 順序に整合させる。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/syn-io-02-head-cas-plan.md`
- `docs/designs/syn-io-02-head-cas-design.md`
- `docs/specs/syn-io-02-head-cas.md`
- `docs/specs/syn-io-02-snapshot-head-staged-publish.md`

## Dependencies

- ruviform-io-file 公開 API（`StagedFile`/`PublishMode::
  CreateNew`/`PublishFailureKind::DestinationExists`/
  `Durability::BestEffort`）— 既存 git dep、lock 変更なし。

## Affected Files

- `crates/synvelis-project/src/store.rs`
- `crates/synvelis-project/src/controller.rs`
- `crates/synvelis-gateway/src/commit.rs`
- `crates/synvelis-project/tests/head_cas.rs`（新規）
- `crates/synvelis-project/tests/store_publish.rs`
- `docs/specs/syn-io-02-head-cas.md`（新規）
- `docs/designs/syn-io-02-head-cas-design.md`（新規）
- `docs/plans/syn-io-02-head-cas-plan.md`（新規）
- `docs/tasks/syn-io-02-head-cas-task.md`（新規）
- `docs/test-specs/syn-io-02-head-cas.md`（新規）
- `docs/impl/ir/syn-io-02-head-cas.md`（新規）
- `docs/specs/syn-io-02-snapshot-head-staged-publish.md`
- `docs/designs/syn-io-02-directory-bundle-store-stagedfile-design.md`

## Done When

- [x] `ProjectStore::save_cas` が additive（default
  `Unsupported`）で、既存 `save`・port 実装・call-site が
  無変更で動作する。
- [x] `DirectoryBundleStore::save_cas` が expected 一致時のみ
  commit・不一致時 typed `Conflict`・`None` は absent assert・
  conflict 時に snapshot/HEAD 不変。
- [x] 同一 root 2 store の同時 `save_cas` が single-winner で
  loser が typed conflict、最終 HEAD が勝者を指す。
- [x] `InMemoryStore::save_cas` parity・`Unsupported` fallback・
  `controller.save` の moved-HEAD conflict・`saved` 非前進。
- [x] journal replay restore が `save_cas` 経由で loud failure。
- [x] `HEAD.lock` stale break・fresh 待機・失敗時解放をテスト
  で観測。
- [x] canonical 検証 9 コマンドを実走・結果記録（IR 参照）。
- [x] 実 2-process・macOS/Linux・電源断は NotRun 記録 —
  実 2-process（Windows）は後続 slice
  `syn-io-02-head-cas-e2e` で実測済み。

## Verification

- [x] `cargo test -p synvelis-project --test head_cas` —
  8 tests pass（Windows 実測）。
- [x] `cargo test -p synvelis-project --test store_publish` —
  16 tests pass。
- [x] `cargo check --workspace --all-targets` — pass。
- [ ] `mise run fmt-check`/`lint`/`test`/`test-rt`/
  `check-app-native`/`lint-app-native`/`test-msrv`/`build`
  — 実走結果を IR に記録。
