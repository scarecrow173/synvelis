---
id: "IMPL-034VpDUdGZZKsxaoSMekla"
type: "impl"
status: "completed"
title: "RCD-02 StagedFile Publish Migration"
created: "2026-11-10"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/116"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-recording/tests/staged_publish.rs"
      - type: "file"
        path: "docs/specs/rcd-02-stagedfile-publish-migration.md"
      - type: "file"
        path: "docs/designs/rcd-02-stagedfile-publish-migration-design.md"
      - type: "file"
        path: "docs/plans/rcd-02-stagedfile-publish-migration-plan.md"
      - type: "file"
        path: "docs/tasks/rcd-02-task3-stagedfile-publish-migration.md"
      - type: "file"
        path: "docs/impl/ir/rcd-02-stagedfile-publish-migration.md"
    modified:
      - type: "file"
        path: "crates/synvelis-recording/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-recording/src/staging.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/faultinj.rs"
      - type: "file"
        path: "crates/synvelis-recording/src/lib.rs"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md"
      - type: "file"
        path: "evidence/reports/rcd-02-durability-recovery-review-ledger.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/tasks/rcd-02-task3-stagedfile-publish-migration.md"
    - "docs/specs/rcd-02-stagedfile-publish-migration.md"
  implemented-by: []
  depends-on:
    - "docs/tasks/rcd-02-task2-project-library-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-02-stagedfile-publish-migration-design.md"
    - "docs/plans/rcd-02-durability-recovery-review.md"
    - "docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md"
    - "docs/impl/ir/rcd-02-project-library-adapter-verified-sha-256-mediareferenceprobe-saved-head.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcd-02-stagedfile-publish-migration-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md"
  verified-by:
    - "docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md"
  references:
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/86"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# RCD-02 StagedFile Publish Migration

## Summary

`crates/synvelis-recording` の staging publish 経路を、手書きの
`tmp→sync→rename` から ruviform `ruviform_io_file::StagedFile` /
`SealedFile::publish(PublishOptions)` へ移行した。公開 signature
（`publish_atomic`/`publish_atomic_faultable`）は維持し、
`synvelis-player` は無変更で build/test が通る。manifest publish は
`PublishMode::CreateNew` の atomic no-replace で `exists()`→publish
の TOCTOU を閉じた。

## Context

- Task: `docs/tasks/rcd-02-task3-stagedfile-publish-migration.md`
- Spec: `docs/specs/rcd-02-stagedfile-publish-migration.md`
- Design: `docs/designs/rcd-02-stagedfile-publish-migration-design.md`
- Plan: `docs/plans/rcd-02-stagedfile-publish-migration-plan.md`
- Ruviform pin: `b02942c2f9e6e195d0d752803b077866468fe58e`
  （origin/main の SV#28 pin 更新を merge 済み）
- Issue: <https://github.com/scarecrow173/synvelis/issues/116>
  （#84 StagedFile/HEAD CAS の recording 側 publish 接続を含む）

## Implementation

### Changes

- `crates/synvelis-recording/Cargo.toml`: `ruviform-io-file.workspace`
  依存を追加（`Cargo.lock` は dep edge 1 行の再生成のみ）。
- `crates/synvelis-recording/src/staging.rs`:
  - `publish_staged`（中核）を新設 — `StagedFile::create` →
    `write_all` → `seal` → `before_publish` hook →
    `SealedFile::publish(PublishOptions::new(mode, Durability::FileSync))`。
    seal 後・publish 前の hook Err は `sealed.abort()` で temp を
    除去して publish しない（`PublishInject::SyncFail` の写像点）。
  - `PublishStepError`（`Stage(FileError)`/`Write{source,temp}`/
    `Publish(PublishFailure)`）で失敗段階を保持し、
    `publish_step_io` で `io::Error` へ写像。
    `PublishFailureKind::DestinationExists`→`AlreadyExists`、
    `Unsupported`→`Unsupported`、その他は phase/kind/effect/
    durability/temp/原因を message に転記。非 Indeterminate の
    leftover temp は best-effort cleanup、Indeterminate は保持
    （ruviform 契約）。
  - `publish_atomic`（Replace）・`publish_atomic_faultable`
    （Replace + seam）・`publish_atomic_create_new`（CreateNew、
    新規公開）を wrapper として維持/追加。
  - `StagingArea::create` の manifest publish を
    `publish_atomic_create_new` へ変更。`exists()` 事前 check は
    早期 return の fast-path として残し、競合時の
    `AlreadyExists` は `StagingError::AlreadyExists` に写像。
- `crates/synvelis-recording/src/faultinj.rs`: `PublishInject`/
  `PublishFaults` の doc を新 pipeline 写像（Denied=create 前、
  SyncFail=seal 後 publish 前で temp 除去、Indeterminate=publish
  成功後）へ更新。API・call 計数規則は不変。
- `crates/synvelis-recording/src/lib.rs`: `publish_atomic_create_new`
  を export。
- `crates/synvelis-recording/tests/staged_publish.rs`（新規）:
  Replace 上書き・CreateNew 拒否・Denied/SyncFail/Indeterminate
  seam・call 計数・stage 失敗の 7 test。
- `docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md`:
  publish fault seam の StagedFile pipeline 記述へ更新。
- `evidence/reports/rcd-02-durability-recovery-review-ledger.md`:
  #84 接続行・Ruviform SHA・fixture 行を更新。

### Decisions During Implementation

- `Durability::FileSync` を採用 — 旧実装は file sync のみで
  directory sync は capability 判定済みの呼出し側が別途行う。
  `FileAndDirectorySync` への引上げは caller の DirSync 方針を
  暗黙に変えるため不採用。
- `PublishMode::Replace` を `publish_atomic`/`publish_atomic_faultable`
  に維持（旧 `fs::rename` と同じ create-or-replace 意味論）。
  冪等な review record/marker の再公開に必要。
- `PublishMode::CreateNew` は manifest publish のみ —
  `exists()`→publish の TOCTOU を OS の atomic no-replace
  primitive で閉塞。unix は `renameat2(RENAME_NOREPLACE)`/
  `renamex_np(RENAME_EXCL)`/hardlink fallback、Windows は
  `MoveFileExW` 無 flag、他 target は `Unsupported` を正直に返す
  （fail-closed、silent fallback なし）。
- fault seam の写像: `Denied` は `StagedFile::create` 前に
  PermissionDenied を返し destination/temp に触れない。
  `SyncFail` は seal 後・publish 前に Err を返し `sealed.abort()`
  で temp を除去（旧実装が残した `*.tmp` residue は新実装では
  残らない — 契約上「残留し得る」は必須でない）。
  `Indeterminate` は publish 成功後に Err を返し destination は
  公開済みのまま（応答喪失相当、file 再読込で照合可能）。
- `PublishFailure` は `Send` だが `Sync` ではない（recovery handle
  が `Box<dyn PlatformFileOps>: Send` のみ）ため `io::Error::new`
  へ直接 box できず、kind 写像 + message 転記で分類を保持する
  方針を採った（`FileError` も同様に source の io::Error から
  kind/raw OS error を復元）。
- write 失敗時の temp は `StagedFile` drop の best-effort cleanup、
  publish 失敗時は `PublishFailure` の recovery handle 経由で
  非 Indeterminate のみ cleanup — store.rs（synvelis-project）の
  既存 convention に揃えた。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

- `cargo check -p synvelis-recording` / `-p synvelis-player` —
  Pass（補助証拠; canonical = mise）
- `cargo test -p synvelis-recording` — Pass（全 test、
  `staged_publish` 7 tests・`recovery_review` 14・`vs_c` 16・
  `vs_d` 22・`project_adapter` 13 等を含む。補助証拠）
- `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
  `test-msrv` — PR 本文に結果を記録

NotRun: 実 OS/FS durability 実測（power-loss 実機）・他 OS FS
matrix・実 device 録音（#115）・#118 製品 GUI E2E・#86 verified
blob 実接続・synvelis-project 側 HEAD CAS（各 gate/別 scope）。

## Risks

- `PublishMode::CreateNew` は unix/Windows で真の no-replace だが、
  他 target では `Unsupported` を返し得る。manifest create が
  失敗として表面化する（fail-closed、silent fallback なし）—
  対象 platform の capability matrix は ruviform-io-file 側の
  責務。
- `SyncFail` seam は publish 前に失敗を返すため、旧実装の
  `*.tmp` residue は発生しない。観測差分は test-spec/ledger に
  明記済み。
- `publish_atomic` の Replace 意味論は synvelis-player の
  import/publish 経路にも適用される（signature 不変・挙動同等）。

## Follow-ups

- synvelis-project 側 HEAD CAS/migration・#86 verified-blob 実接続
  （lease 競合 + dep edge 規約判断が要るため本 slice 対象外）。
- crash 後に media/journal を最終 sync frontier へ truncate する
  unsynced-write 喪失 fixture（optional、未同梱）。
- 実 OS/FS durability・power-loss 実測（実機 gate）。
