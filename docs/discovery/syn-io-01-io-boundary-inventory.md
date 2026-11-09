---
id: "DISC-034UvxTvzXCIyCbGIaiTpO"
type: "discovery"
status: "resolved"
title: "SYN-IO-01 Ruviform 共通 I/O 適合境界 — 現行実装と公開契約の棚卸し"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/82"
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
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/designs/syn-io-02-directory-bundle-store-stagedfile-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/syn-io-01-ruviform-common-io-boundary.md"
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/282"
    - "https://github.com/scarecrow173/ruviform/issues/283"
    - "https://github.com/scarecrow173/ruviform/issues/284"
    - "https://github.com/scarecrow173/ruviform/issues/288"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/409"
    - "https://github.com/scarecrow173/ruviform/issues/412"
  defers: []
  deferred-by: []
---

# SYN-IO-01 Ruviform 共通 I/O 適合境界 — 現行実装と公開契約の棚卸し

## Summary

Synvelis Issue #82（SYN-IO-01）の設計入力として、Synvelis 側の
Project I/O 現行実装（`DirectoryBundleStore`・`ProjectController`・
gateway commit journal・各 crate の file/archive 利用面）と、Ruviform
側の共通 I/O 公開契約（`ruviform-io-file` staged publish・
`ruviform-io-structured`・`ruviform-io-archive`・
`ruviform-io-compression`・rooted FS capability）の棚卸しを行った。
結論として、Synvelis が所有すべき意味論（ExpectedRevision・HEAD・
immutable Snapshot・外部変更検出・Crash 後 Reopen 解釈・
Backup/Import 方針）は Ruviform 公開 API のどの保証範囲にも
含まれず、Synvelis 側に固定する必要がある。Ruviform は staged
publish の effect/phase/durability 分類と archive の安全な path/
limit 機構までを提供し、project 意味論・manifest 検証・
transaction 意味論は提供しない。信頼度は高い（両リポジトリの
実コードと Issue/設計文書を直接確認済み）。

## Exploration Goal

Issue #82 の 8 つの受入条件（責務分担図示・Owner 固定・
失敗写像・Backup/Import 設計・test 計画・E2E 非退行・
#288 gate 証拠項目・consumer 意味論維持）を設計文書へ落とすために、
(1) Synvelis の現行 Project I/O 実装がどの不変条件をどこで
所有しているか、(2) Ruviform の公開 I/O 契約がどこまでを保証し
どこで止まるか、(3) Backup/Import の初期採用範囲として何が
妥当か、を確定する。

## Constraints

- Issue #82 は設計 Issue であり、実装は Synvelis #84/#86 と
  Ruviform #288（IO-06 consumer gate）へ委譲されている。
- 編集範囲は `docs/` のみ。`crates/`・manifest・lockfile・
  CI の変更は禁止。
- Ruviform Generic IO/File/FS 公開 API の独自 fork 設計、
  Project HEAD CAS を共通 File API へ混入する設計は禁止。
- FS Crash/Durability の実機実測は本スライスでは行わない
  （NotRun として計画に記述）。

## Findings

### Synvelis 側の現行 Project I/O（`crates/synvelis-project`）

- `DirectoryBundleStore`（`src/store.rs`）は
  `<root>/snapshots/r<N>.json` の immutable snapshot と
  `<root>/HEAD` を管理する。`ProjectStore` port は
  `save`/`head`/`read_snapshot` の 3 操作。
- Save シーケンスは「snapshot publish → HEAD publish」の順序固定。
  snapshot は `PublishMode::CreateNew`（immutable 名への no-replace）、
  HEAD は `PublishMode::Replace`。両方とも
  `Durability::FileAndDirectorySync` +
  `VisibilityRequirement::RequireAtomic`。
- snapshot 名衝突は `snapshot_collision`（`id_watermark` 比較）で
  Satisfied / Refresh / Conflict に分類。同一 byte 再送は
  冪等受理するが、durability 証明のため kept byte を再 publish
  して directory entry の durability を再確立する（存在確認
  だけでは durability 証明にならない）。
- `PublishFailure` は effect（`NotPublished`/`Published`/
  `Indeterminate`）・phase・達成済み durability・temp path を
  `Error::Store` メッセージへ転写。`Indeterminate` の残留 temp
  は自動削除しない。HEAD publish 失敗は durable な orphan
  snapshot を残し得るが、`head()` が正直な readback を提供する。
- `ProjectController`（`src/controller.rs`）が authoring state の
  単一 writer。`apply` は `expected_revision` CAS。
  `ProjectRevision` は単調増加で undo/redo では巻き戻らない。
  `SavedRevision` は store の HEAD が表す。
  `RecoveryRevision` は Saved/HEAD と別 domain。
- `SnapshotEnvelope`（`schema_version` 現行 8・`project_id`・
  `revision`・`project`、`deny_unknown_fields`）の canonical
  byte 生成は serde_json + `BTreeMap` で Synvelis 所有。
- `synvelis-gateway` の `CommitGate`/`FileJournal` は durable
  commit journal で、crash 後に journal record から controller
  状態を replay 復元する。Project recovery とは別経路だが
  「committed op の durable 記録 → reopen 復元」の先例。
- `synvelis-library` の `LocalBlobDir`/`LocalProvider` は
  Ruviform `RootedDir` capability（`open_with`/`open_scoped_root`/
  `read_dir_each`/`FollowMode`）を asset blob の封じ込めに使用
  中（RF#409/#412 consumer 側接続済み）。
- `synvelis-render` の WAV export は StagedFile → writer →
  seal → readback verify → publish（syn-rf305 で移行済み）。
- `synvelis-player` の import/publish と `synvelis-recording` の
  staging/journal/recovery は別 lifecycle で、Project
  Snapshot/HEAD 契約とは独立。

### Ruviform 側の公開 I/O 契約（pin: workspace 解決 revision）

- `ruviform-io-file`: `StagedFile → SealedFile → publish`。
  `PublishMode::CreateNew`（atomic no-replace・既存 dest 拒否）、
  `PublishMode::Replace`（無条件 create-or-replace、revision/CAS・
  multi-file transaction 意味論を持たない）。
  `VisibilityRequirement::RequireAtomic` は弱い fallback を拒否。
  `Durability::{BestEffort, FileSync, FileAndDirectorySync}` は
  独立した要求。publish 成功後の dir sync 失敗は
  `PublishEffect::Published` 付き `Err` を返す。
  `RecoveryHandle` は不確実/cleanup 未完了 outcome 用に保持。
  `PublishEffect::{NotPublished, Published, Indeterminate}`、
  `PublishPhase`、`PublishFailureKind` で generic な失敗分類を
  提供。公開 API はすべて non-RT。
- `ruviform-io-structured`: json/yaml/toml/csv/xml の
  `read_file`/`write_file`（opt-in file feature）、strict EOF、
  `InputLimits`、`DocumentFileWriteError` は `PublishFailure`
  を保持。Project schema/semantic 検証は持たない。
- `ruviform-io-archive`: `ZipArchiveWriter`/`Reader`
  （`verify_all` CRC32 receipt）、`TarArchiveWriter`/`Reader`、
  `ExtractionSession` → `extract_zip`/`extract_tar` →
  `ExtractedTree`（staged・検証済み tree。publish ではない）、
  `SafeRelativePath`、`ExtractOptions`、`ArchiveReadLimits`。
  link/special entry の安全な扱いと limit 機構を提供するが、
  project manifest/schema 検証・semantic allowlist・import
  transaction 意味論は consumer 責務。
- `ruviform-io-compression`: gzip/zstd/deflate の
  Encoder/Decoder、finish receipt。
- rooted FS: `RootedDir` は capability-based containment
  （`FollowMode`、`RelativePath`、`read_dir_each`）。
  project-root trust と semantic validation は caller 責務。
- OS 互換 matrix: Unix/Windows NTFS・FAT・exFAT・ReFS/
  APFS/NFS 等。未対応 durability を silent に downgrade
  しない設計。

### Issue/設計文書の関係

- RF#282 が common I/O の設計権威。#283–#287 が IO-01..05、
  #288 が IO-06 consumer gate（OS/FS/security/performance
  証拠）、#294 が cross-consumer candidate tuple gate。
- Synvelis #82 はその consumer 側境界設計。#84（SYN-IO-02
  Snapshot/HEAD StagedFile 移行）は既に store.rs で
  StagedFile publish を採用済み（syn-io-02 chain で完結）。
  #86（ALIB-02 asset SHA-256/CAS）は未着手。
- `docs/designs/project-asset-persistence-architecture.md`
  の PAP-01..18 が Project 永続化 semantics の正本であり、
  本設計はその上に Ruviform API 対応を写像する。

## Alternatives Considered

- **Synvelis 独自の Project transaction/backup format を新設**:
  不採用。Issue が「新たな必須保存 Format を発明しない」と
  規定し、既存 bundle layout（snapshots+HEAD+envelope）を
  正本として維持する。
- **Ruviform File API に Project HEAD CAS を追加**:
  不採用（禁止事項）。`PublishMode::Replace` は revision/CAS
  を持たないことが契約であり、ExpectedRevision 判定は
  Synvelis `ProjectController`/`ProjectStore` 層に残す。
- **TAR(+compression) を Backup 初期形式に採用**:
  不採用。TAR は payload checksum を持たず（`EntryChecksum::
  NotPresent`）、streaming 専用で verify/readback の証拠が
  ZIP より弱い。Backup はユーザーが OS 標準 tool で開ける
  単一ファイル性と `verify_all` による CRC32 receipt を優先。
  詳細は ADR `docs/adr/syn-io-01-backup-import-archive-zip-limits-path-link.md`。
- **Structured API で `SnapshotEnvelope` を即座に移行**:
  今回は採用範囲の設計のみ。canonical byte（serde_json +
  `BTreeMap`）は Synvelis 所有のまま維持し、Structured API
  採用は byte 同一性の検証を伴う別途移行判断とする。

## Tentative Conclusions

1. Owner 固定: ExpectedRevision・HEAD/SavedRevision・immutable
   Snapshot・外部変更検出・Crash 後 Reopen 解釈・Recovery/
   Backup/Import 方針はすべて Synvelis 所有。Ruviform は
   staged publish の effect/phase/durability 分類と archive
   の path/limit 機構までを提供する。
2. 失敗写像: `PublishMode`/`Durability`/`VisibilityRequirement`/
   `PublishEffect`/`PublishPhase`/`PublishFailureKind`/
   `RecoveryHandle`/`ReadbackFile` を ProjectStore の既存
   分類（Satisfied/Refresh/Conflict・NotPublished/Published/
   Indeterminate・orphan snapshot・head() readback）へ写像する。
3. Backup/Import: ZIP コンテナを初期採用範囲とし、
   `ExtractionSession`+`ExtractedTree` で staging → Synvelis
   manifest/schema/blob-digest 検証 → commit の二段構成。
   TAR と独立 compression stream は初期対象外。
4. Gate 証拠: Ruviform #288 へ送る Synvelis 独立証拠は
   revision/SHA・OS/FS capability matrix・save/reopen/
   readback・stale-writer・crash/recovery・archive safety・
   performance/non-RT の各項目を Pass/Fail/Unsupported/NotRun
   で記録する。実機 FS crash/durability は NotRun。

## Evidence

- `crates/synvelis-project/src/store.rs`（publish 順序・
  collision 分類・PublishFailure 転写・orphan 許容）
- `crates/synvelis-project/src/controller.rs`（CAS・
  Saved/Live/Recovery revision domain）
- `crates/synvelis-library/src/`（`LocalBlobDir`/
  `LocalProvider` の RootedDir 利用、RF#409/#412 接続）
- `crates/synvelis-render/src/` export（syn-rf305 StagedFile
  移行済み）、`crates/synvelis-player` import/publish、
  `crates/synvelis-recording` staging/journal/recovery
- `crates/synvelis-gateway` `CommitGate`/`FileJournal`
- Ruviform `F:/repositry/audio-products/ruviform`:
  `docs/designs/issue-295-io-file-staged-publish.md`、
  `docs/designs/issue-296-io-file-os-backend.md`、
  `docs/designs/issue-294-cross-consumer-candidate-gate.md`、
  `crates/ruviform-io-file/src/{options,staged,outcome,receipt,rooted}.rs`、
  `crates/ruviform-io-archive/src/`（zip/tar/extract/limits）、
  `crates/ruviform-io-structured`、`crates/ruviform-io-compression`
- `docs/designs/project-asset-persistence-architecture.md`
  （PAP-01..18）、`docs/designs/syn-io-02-directory-bundle-store-stagedfile-design.md`、
  `docs/specs/syn-io-02-snapshot-head-staged-publish.md`、
  `docs/specs/syn-rf305-wav-export-staged-file.md`、
  `docs/adr/syn-rf305-consumer-first-staged-file-cutover.md`

## Open Questions

- ~~Backup/Import の初期 archive 形式は何か~~ →
  ZIP コンテナ（ADR で決定。最終的な製品 format 凍結は
  実装 Issue で再確認）。
- ~~Structured API を `SnapshotEnvelope` に適用するか~~ →
  本設計では byte 同一性維持を条件に採用範囲のみ定義。
  実移行は #84 系列の後続判断。
- 外部変更検出の将来強化（generation/mtime 等）: 現行は
  `head()` readback + snapshot 名衝突分類が実効的検出点。
  Ruviform が提供する範囲を超える semantic 検査は
  Synvelis 側で設計継続。

## Promotion Candidates

- 本棚卸しの結論は spec
  `docs/specs/syn-io-01-ruviform-common-io-boundary.md` と
  design `docs/designs/syn-io-01-ruviform-io-boundary-design.md`
  へ昇格済み。
- Backup/Import archive 決定は ADR
  `docs/adr/syn-io-01-backup-import-archive-zip-limits-path-link.md`
  へ昇格。
