---
id: "TASK-2Ri1ehmSIXwfHO79pHYSiU"
type: "task"
status: "done"
title: "RCD-02 Task Unsynced-Write Loss Fixture + Real-FS Probe"
created: "2026-11-11"
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
    - "docs/plans/rcd-02-unsynced-loss-real-fs-probe-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-02-unsynced-loss-real-fs-probe-plan.md"
    - "docs/tasks/rcd-02-task3-stagedfile-publish-migration.md"
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
    - "docs/test-specs/rcd-02-unsynced-loss-real-fs-probe.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-02 Task Unsynced-Write Loss Fixture + Real-FS Probe

## Work

`recovery_review.rs` に sync-frontier truncation による
unsynced-write 喪失 fixture を 2 test 追加する（対称喪失 +
非対称 tail）。`recording-poc` に `realfs` サブコマンド
（`probe`/`write-unsynced`）を実装し、`tests/real_fs_probe.rs`
の `#[ignore]` opt-in test から Windows/NTFS 実挙動を実測
する。`mise.toml` に `test-recording-real-fs` manual task を
1 件追加し、実測値を evidence ledger の実測行として記録
する。真の power-loss は `NotRun` 維持。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/rcd-02-unsynced-loss-real-fs-probe-plan.md`
- `docs/specs/rcd-02-unsynced-loss-real-fs-probe.md`
- `docs/designs/rcd-02-unsynced-loss-real-fs-probe-design.md`
- `docs/test-specs/rcd-02-unsynced-loss-real-fs-probe.md`

## Dependencies

- `docs/tasks/rcd-02-task3-stagedfile-publish-migration.md`
  （同一 RCD-02 plan 系列の先行 slice・StagedFile publish
  経路を probe が実測する）

## Affected Files

- `crates/synvelis-recording/tests/recovery_review.rs` —
  truncate helper + 2 test 追補
- `crates/recording-poc/src/main.rs` — `realfs` dispatch/
  `realfs_probe`/`realfs_write_unsynced`
- `crates/recording-poc/tests/real_fs_probe.rs` — 新規
  `#[ignore]` test 2 件
- `mise.toml` — `[tasks.test-recording-real-fs]`
- `evidence/reports/rcd-02-durability-recovery-review-ledger.md`
  — 実測行・fixture 追記
- `docs/impl/ir/rcd-02-unsynced-loss-real-fs-probe-slice.md` —
  新規
- `docs/*/README.md` — 管理 index 再生成

## Done When

- [ ] unsynced-loss fixture 2 test が追加され、対称/非対称
      喪失で Adopt が verified prefix のみ `Partial` で確定し
      `Complete` にしないことが検証される
- [ ] `realfs` probe が rename/dir sync/publish/file 観測を
      key=value で出力し、`write-unsynced` が kill 待機する
- [ ] `mise run test-recording-real-fs` が本機 Windows/NTFS
      で実行され、観測値が ledger 実測行として記録される
      （power-loss は `NotRun` 維持）
- [ ] 既存 fault matrix/staged publish/headless 全 test が
      回帰しない

## Verification

- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
      `test-rt` / `test-msrv`
- [ ] `mise run test-recording-real-fs`（opt-in 実測）
