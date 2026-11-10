---
id: "PLAN-034VpmToY7qmWuyovg4BmV"
type: "plan"
status: "approved"
title: "RCD-02 StagedFile Publish Migration Plan"
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
    - "docs/specs/rcd-02-stagedfile-publish-migration.md"
  implemented-by:
    - "docs/tasks/rcd-02-task3-stagedfile-publish-migration.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/rcd-02-durability-recovery-review.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-02-stagedfile-publish-migration.md"
    - "docs/designs/rcd-02-stagedfile-publish-migration-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md"
  verified-by:
    - "docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/84"
  defers: []
  deferred-by: []
---

# RCD-02 StagedFile Publish Migration Plan Implementation Plan

## Goal

`crates/synvelis-recording` の staging publish 経路を ruviform
`StagedFile`/`SealedFile::publish(PublishOptions)` へ移行し、
公開 signature・fault seam・冪等再公開 semantics を維持したまま
manifest publish の TOCTOU を `PublishMode::CreateNew` で閉じる。

## Scope

単一実装 stream。変更は `crates/synvelis-recording/`（src +
Cargo.toml + tests）と `docs/`・`evidence/` の本 slice 文書のみ。
synvelis-project/library/app/workspace/player/input.rs および
他 crate には触れない。

## Upstream Documents

- `docs/specs/rcd-02-stagedfile-publish-migration.md`

## Design Inputs

- `docs/designs/rcd-02-stagedfile-publish-migration-design.md`

## File Map

- `crates/synvelis-recording/Cargo.toml` — `ruviform-io-file.workspace = true` 追加
- `crates/synvelis-recording/src/staging.rs` — `publish_staged`/
  `PublishStepError`/io error 写像、`publish_atomic`/
  `publish_atomic_faultable`/`publish_atomic_create_new` wrapper、
  `StagingArea::create` の manifest publish を CreateNew 化
- `crates/synvelis-recording/src/faultinj.rs` — `PublishInject`/
  `PublishFaults` の doc を新 pipeline 写像へ更新（API 不変）
- `crates/synvelis-recording/src/lib.rs` —
  `publish_atomic_create_new` の export 追加
- `crates/synvelis-recording/src/writer.rs`・`src/recovery.rs`・
  `src/commit.rs` — 呼出し整合（原則無変更）
- `crates/synvelis-recording/tests/` — 回帰確認 + optional
  unsynced-write 喪失 fixture
- `Cargo.lock` — 依存 edge 追加に伴う再生成
- `docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md`
  — fault seam 記述の StagedFile pipeline 更新
- `docs/impl/ir/rcd-02-stagedfile-publish-migration.md` — 実装記録
- `evidence/reports/rcd-02-durability-recovery-review-ledger.md`
  — #84 接続行・Windows/NTFS 実測行更新

## Implementation Sequence

- [ ] `Cargo.toml` に `ruviform-io-file` 追加 → `mise run check`
      で `Cargo.lock` 再生成を確認
- [ ] `staging.rs`: `publish_staged`/`PublishStepError`/error 写像
      実装、3 wrapper 化、manifest publish の CreateNew 化
- [ ] `faultinj.rs` doc 更新・`lib.rs` export 追加
- [ ] `mise run fmt-check`/`check`/`lint` で compile/lint 通過
- [ ] `mise run test` で既存 fault matrix 回帰（recovery_review/
      vs_c/vs_d/project_adapter/recording）確認
- [ ] optional: unsynced-write 喪失 fixture test 追加・Pass 確認
- [ ] `docs/test-specs` fault seam 記述更新、`docs/impl/ir` 作成、
      evidence ledger 更新（#84 接続行・Windows/NTFS 実測行）
- [ ] `git fetch origin main && git merge origin/main` で最新 main
      取り込み後に最終検証（fmt-check/check/lint/test/test-rt/
      test-msrv）

## Dependencies and Risks

- `synvelis-player` は `publish_atomic` を `import.rs`/`publish.rs`
  から利用中 — signature 維持が必須条件。workspace `check`/`test`
  で回帰検出。
- `PublishMode::CreateNew` は unix/Windows で真の no-replace。
  他 target は `Unsupported` を返し得る（fail-closed で表面化、
  silent fallback なし）。
- `SyncFail` 注入で旧実装が残した `*.tmp` residue は新実装で
  cleanup される（契約上「残留し得る」は必須でない）。差分は
  test-spec/ledger に明記。
- Indeterminate の `.rvf-tmp-*` leftover は自動削除しない
  （ruviform 契約）。recovery scan は既知 filename のみ読むため
  分類に影響しない。

## Task Breakdown

- Task 3: `docs/tasks/rcd-02-task3-stagedfile-publish-migration.md`
  （全ファイルを単一 task で実装 — 同一 crate 内の一貫した変更）

## Verification

- [ ] `mise run fmt-check`
- [ ] `mise run check`
- [ ] `mise run lint`
- [ ] `mise run test`（recovery_review/vs_c/vs_d/project_adapter
      を含む全 workspace test）
- [ ] `mise run test-rt`
- [ ] `mise run test-msrv`
- [ ] `synvelis-player` が無変更で build/test Pass（workspace
      test に包含）
- [ ] Windows/NTFS 実測行を ledger へ記録
- [ ] 実 OS power-loss・他 OS FS matrix は NotRun 継続

## Open Questions / Gaps

- なし（spec/design で確定済み）

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（Orca worker dispatch 内）
- Independent work streams: none
- User approval required before dispatch: no（review-protocol で代替）
- If approved: execute task-by-task with review checkpoints.
