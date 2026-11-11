---
id: "TASK-034VpnH7YtacCiCUD4y9fu"
type: "task"
status: "in-progress"
title: "RCD-02 Task3 StagedFile Publish Migration"
created: "2026-11-10"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/rcd-02-stagedfile-publish-migration-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-02-stagedfile-publish-migration-plan.md"
    - "docs/tasks/rcd-02-task2-project-library-adapter.md"
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
    - "docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-02 Task3 StagedFile Publish Migration

## Work

`crates/synvelis-recording` の staging publish 経路を ruviform
`StagedFile`/`SealedFile::publish(PublishOptions)` へ移行する。
`publish_atomic`/`publish_atomic_faultable` の公開 signature を
維持した薄い wrapper とし、内部を `publish_staged`（stage→write→
seal→publish の 3 分類 `PublishStepError`）へ集約する。
manifest publish は `PublishMode::CreateNew` の atomic no-replace
で `exists()`→publish の TOCTOU を閉じる。`PublishInject`/
`PublishFaults`（Denied/SyncFail/Indeterminate）は新 pipeline の
対応 stage へ写像して保全する。`PublishEffect::Indeterminate`/
`PublishFailureKind`/`Durability` の正直な分類を `io::Error`
source/message に保持する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/rcd-02-stagedfile-publish-migration-plan.md`
- `docs/specs/rcd-02-stagedfile-publish-migration.md`
- `docs/designs/rcd-02-stagedfile-publish-migration-design.md`

## Dependencies

- `docs/tasks/rcd-02-task2-project-library-adapter.md`（同一
  RCD-02 plan 系列の先行 slice）

## Affected Files

- `crates/synvelis-recording/Cargo.toml` — `ruviform-io-file`
  workspace dep 追加
- `crates/synvelis-recording/src/staging.rs` — publish 実装差替え・
  `publish_atomic_create_new` 追加・manifest CreateNew 化
- `crates/synvelis-recording/src/faultinj.rs` — seam doc 更新
- `crates/synvelis-recording/src/lib.rs` — export 追加
- `crates/synvelis-recording/src/{writer,recovery,commit}.rs` —
  呼出し整合（原則無変更）
- `crates/synvelis-recording/tests/` — 回帰 + optional fixture
- `Cargo.lock` — 依存 edge 再生成
- `docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md`
- `docs/impl/ir/rcd-02-stagedfile-publish-migration.md`
- `evidence/reports/rcd-02-durability-recovery-review-ledger.md`

## Done When

- [ ] `publish_atomic`/`publish_atomic_faultable` signature 維持で
      `synvelis-player` が無変更 build/test Pass
- [ ] manifest publish が `PublishMode::CreateNew` で TOCTOU 閉塞、
      競合は `AlreadyExists` 分類
- [ ] `PublishInject`/`PublishFaults` seam が Denied/SyncFail/
      Indeterminate を保全し観測結果が旧実装と一致
- [ ] 既存 recovery_review/vs_c/vs_d/project_adapter tests が
      無変更で Pass
- [ ] Windows/NTFS 実測行を ledger へ追記

## Verification

- [ ] `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
      `test-msrv`
