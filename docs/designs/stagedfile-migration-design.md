---
id: "DESIGN-034X5yWfKUcmf5eqz7NUJR"
type: "design"
status: "approved"
title: "Issue #84 StagedFile Migration 実装済み設計"
created: "2026-11-14"
updated: "2026-11-14"
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
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/syn-io-02-directory-bundle-store-stagedfile-design.md"
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/rcd-02-stagedfile-publish-migration-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/stagedfile-migration.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv84/case-ledger.md"
  defers:
    - "docs/impl/ir/syn-io-02-head-cas-e2e.md"
    - "evidence/sv84/case-ledger.md"
  deferred-by: []
---

# Issue #84 StagedFile Migration 実装済み設計

## Context

Issue #84「DirectoryBundleStore・Snapshot/HEAD 保存を Ruviform
StagedFile へ移行」の設計正本。実装は 2 つの先行 slice で
main 統合済みであり、本書は Issue #84 スコープの設計を
as-built として確定する:

- **store 移行 slice**（PR #178・`docs/designs/
  syn-io-02-directory-bundle-store-stagedfile-design.md`）:
  `write_atomic` 撤去 → `publish_staged`（`StagedFile` →
  `seal` → `publish`）。
- **HEAD CAS slice**（PR #342・Issue #330）:
  `ProjectStore::save_cas` + `HEAD.lock` 協調 mutex +
  `HeadCasError`（`docs/specs/syn-io-02-head-cas.md`・
  `docs/impl/ir/syn-io-02-head-cas.md`）。

`DirectoryBundleStore` は `<root>/snapshots/r<N>.json` +
`<root>/HEAD` の directory bundle を永続化する
（`crates/synvelis-project/src/store.rs`）。本設計は publish
段を ruviform-io-file の型状態 lifecycle へ委譲し、Synvelis
固有の意味論（Schema/Revision/HEAD/CAS/Recovery/Undo）は
Consumer 側に残す境界を固定する。

## Scope

### In Scope

- snapshot publish: `PublishMode::CreateNew` +
  `Durability::FileAndDirectorySync` +
  `VisibilityRequirement::RequireAtomic`。
- HEAD publish: `PublishMode::Replace` + 同 durability
  （snapshot publish 成功後にのみ）。
- `snapshot_collision` による同名衝突分類
  （`Satisfied`/`Refresh`/`Conflict`）と、受理時の勝者 byte
  完全 publish 再実行。
- `save_cas` の `HEAD.lock` critical section
  （`CreateNew` + `BestEffort`、stale 20s rename-aside
  break、busy 30s、token fencing、pre/post publish
  fencing）。
- `PublishFailure`/`PublishStepError` → `Error::Store` /
  `HeadCasError` への分類転写と recovery handle 方針。
- `InMemoryStore` の port parity（immutable name 契約・
  CAS semantics の in-memory mirror）。

### Out of Scope

- Ruviform 側の File/FS/StagedFile 実装変更、DAW
  ProjectId/Revision/Undo の共通 File API への追加。
- `SnapshotEnvelope` schema・serialize 順序・canonical
  byte の変更（v13 現行・migration registry 経由の版管理は
  `crate::migration` の責務）。
- #86 の schema 変換・Verified Import（#86 Owner）。
- 他 crate の `.tmp`/rename（recording/gateway/player は
  別 Owner — `apply/journal.rs`・`plugin_state.rs`・
  `restore/mod.rs` は同一 `ruviform-io-file` API を共有
  するが Project Commit CAS は `store.rs` 固有）。
- 非協調 writer への強制排他（kernel lock）— advisory
  `HEAD.lock` は協調 writer のみを直列化する。

## Components and Boundaries

- **`DirectoryBundleStore`**（synvelis-project 所有）:
  publish 順序（snapshot → HEAD）・immutable name・
  orphan 方針・collision policy・CAS critical section・
  error 写像を所有。OS primitive の詳細は ruviform に委譲。
- **`ruviform_io_file::{StagedFile, SealedFile,
  PublishOptions, PublishFailure, PublishMode, Durability,
  PublishEffect, PublishFailureKind}`**（ruviform 所有、
  resolved rev `fd1d4d5f4de57092b96ad1cd309b85fe47cbef38`）:
  temp sibling 排他作成（`*.rvf-tmp-*`）・Write+Seek・seal・
  publish pipeline（validate→file sync→close→atomic
  publish→dir sync→cleanup）・失敗分類・recovery handle。
  **ProjectId/Revision/Undo の概念を持たない** — operation
  id・temp path・durability のみを所有する。
- **`ProjectController`**（synvelis-project 所有）:
  `Command::expected_revision` の CAS 判断・単一 writer・
  SavedState/Undo/Recovery 判断。`save` が `saved.head` を
  expected-head として `save_cas` へ供給し、
  `HeadCasError::Unsupported` のみ無条件 `save` へ
  fallback する。
- **`synvelis-gateway` journal restore**: `save_cas` 経由で
  同一 CAS 契約に乗る（crash window orphan の回復経路）。

## Data and Control Flow

`save(project_id, revision, bytes)`（無条件 publish —
single-writer/journal-restore 経路）:

1. `name = snapshot_name(revision)`（`r<N>.json`）。
2. `publish_snapshot`: `publish_staged(target, bytes,
   CreateNew, FileAndDirectorySync)`。
   - `Ok`: snapshot 公開済み。
   - `DestinationExists`: `fs::read` で既存 byte を読み
     `snapshot_collision` で分類 —
     `Satisfied`（byte 同一 or 既存 watermark ≧ incoming）→
     stored byte を `Replace`+`FileAndDirectorySync` で再公開、
     `Refresh`（incoming watermark が新しい）→ incoming byte
     を同様に再公開、`Conflict` → `Err`（HEAD 不変）。
     受理はスキップではなく「存在の証明にならない耐久性を
     再確立する完結操作」。
   - その他 `PublishStepError`: HEAD へ進まず `Error::Store`。
3. `publish_head`: `publish_staged(HEAD, name,
   Replace, FileAndDirectorySync)`。
   - `NotPublished` 失敗: snapshot は orphan として残り、
     旧 HEAD 内容を保持。
   - `Published`/`Indeterminate`: HEAD が既に進んだ可能性を
     分類込みで報告 — `head()` 再読込が実態を決める。

`save_cas(project_id, expected_head, revision, bytes)`
（syn-io-02-head-cas）:

1. `acquire_head_lock`: `HEAD.lock` を `CreateNew`+
   `BestEffort` で取得。fresh holder は poll 待機
   （`HEAD_LOCK_BUSY_TIMEOUT` 30s）、stale（>20s・malformed・
   unreadable）は rename-aside で break。lock file は
   `created_ns`+一意 `token` を保持。
2. `head()` で actual を読み `expected_head` と比較 —
   不一致は `HeadCasError::Conflict`（snapshot publish 前
   判定のため orphan 非生成）。
3. `publish_snapshot` → pre-publish fencing
   （`lock.still_ours()`）→ `publish_head` → post-publish
   fencing。lock 喪失時は `Ok` を返さず `Store` error で
   indeterminate を報告（HEAD が進んだ可能性を隠さない）。
4. `HeadLock::drop` は token verify-then-delete —
   broken-and-recreated lock を誤削除しない。

## 責務境界（Publish ≠ Project Commit）

- **File Publish（ruviform-io-file）**: 単一 file の
  staged write・seal・atomic publish・dir sync・失敗分類・
  operation id・temp path・recovery handle のみを所有。
- **Project Commit（synvelis-project）**: `SnapshotEnvelope`
  （schema version・canonical `BTreeMap` byte）・immutable
  snapshot 命名・HEAD pointer・ExpectedRevision/CAS 判断
  （`HEAD.lock`・stale-break・fencing）・collision policy
  （`id_watermark`）・Saved/Live revision・Undo/AutoSave・
  Recovery 採用を所有。
- **判定規則**: publish `Ok` は「要求した durability を
  達成して name が見える」ことのみを意味し、Project
  Commit（HEAD repoint + revision 確定）は HEAD publish
  成功後にのみ成立する。`Published`/`Indeterminate`/
  DirSync 未達で HEAD を勝手に進めず、`head()` 再読込・
  Recovery へ接続する。

## Verification Notes

- headless fixture: `tests/store_publish.rs`（16 tests）・
  `tests/head_cas.rs`（9 tests・うち 1 は 2-process e2e）・
  `tests/controller_store.rs`（11 tests）が実 FS
  （`std::env::temp_dir`）上で契約を pin する — test-spec
  `docs/test-specs/stagedfile-migration.md` の Rule ↔
  fixture 対応は `evidence/sv84/` 台帳に記録。
- OS-L1: 既存 ci.yml の verify legs（ubuntu/windows/
  macos + msrv + rt）で対象 fixture が実走。対象 SHA の
  run が未実施の場合は祖先 SHA の covering run を
  nearest-SHA 証拠として byte 同一性を確認した上で記録。
- Windows 実利用 FS: Project 保存先は利用者の通常ローカル
  FS（NTFS）— fixture の実 FS 実走がそのまま Windows L2
  相当の状態遷移・Readback/再 Open・衝突/失敗復旧をカバー
  する（ネットワーク/クラウド等未使用 FS は対象外）。
