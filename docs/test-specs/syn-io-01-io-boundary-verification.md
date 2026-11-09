---
id: "TSPEC-034UvylKq35FCXtkknERpp"
type: "test-spec"
status: "approved"
title: "SYN-IO-01 I/O 境界 Golden/Negative/Crash 検証計画"
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
    - "docs/designs/syn-io-01-ruviform-io-boundary-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/syn-io-01-ruviform-io-boundary-design.md"
    - "docs/adr/syn-io-01-backup-import-archive-zip-limits-path-link.md"
  derived-by: []
  verifies:
    - "docs/specs/syn-io-01-ruviform-common-io-boundary.md"
    - "docs/designs/syn-io-01-ruviform-io-boundary-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/ruviform/issues/288"
  defers: []
  deferred-by: []
---

# SYN-IO-01 I/O 境界 Golden/Negative/Crash 検証計画

## Purpose

Synvelis Issue #82（SYN-IO-01）の境界設計が要求する検証を、
実装 Issue（#84/#86/RF#288）で実行可能な test 計画として
固定する。本書は設計段階の計画であり、全 case は現時点で
**NotRun** である。実装時に各 case へ Case ID・Expected・
Observed を記録し、Ruviform #288 gate 証拠として提出する。

## Feature

Project Snapshot/HEAD publish・Recovery・Undo・Asset・
Plugin State・Import/Export の各局面で、Ruviform 共通 I/O
機構と Synvelis project 意味論の境界が正しく機能することを
検証する。

## Rules

### Rule G-01: Saved Revision Golden

- **Given**: 有効な project が `DirectoryBundleStore` に
  save 済み（saved_revision = N）。
- **When**: `apply` で live_revision を進め `save` を実行し、
  reopen する。
- **Then**: `head()` が新 saved_revision を返し、
  `read_snapshot` が canonical envelope を返す。
  snapshot は immutable（同名 byte 同一の再送は冪等受理、
  異 byte 同名は Conflict）。

### Rule G-02: Recovery Golden

- **Given**: save 中に crash/中断が発生した project。
- **When**: reopen して `load()` を実行する。
- **Then**: `head()` が実効 saved_revision を返し、
  orphan snapshot（HEAD 未参照）は無害として扱われる。
  `PublishEffect::Indeterminate` の残留 temp は自動削除
  されず、readback 判断へ接続される。

### Rule G-03: Undo/Redo Golden

- **Given**: undo/redo を含む authoring session。
- **When**: `apply`/`undo`/`redo`/`save` を実行する。
- **Then**: `ProjectRevision` は単調増加し undo/redo で
  巻き戻らない。SavedRevision は HEAD が指す revision
  のみを表し、Live/Recovery とは別 domain。

### Rule G-04: Asset Golden

- **Given**: verified blob（SHA-256 digest 一致）を持つ
  project。
- **When**: asset commit と save を実行し reopen する。
- **Then**: blob は digest で検証され、project snapshot
  から参照可能。blob corruption/hash mismatch は
  Negative case（N-08）で検証。

### Rule G-05: Plugin State Golden

- **Given**: plugin state を含む project。
- **When**: save→reopen する。
- **Then**: plugin state は opaque data として schema_version
  で検証され、unknown schema/plugin state は Negative
  case（N-07）で検証。

### Rule G-06: Import Golden

- **Given**: 正常な backup.zip（manifest・snapshots・HEAD・
  blobs を含む）。
- **When**: import を実行する。
- **Then**: `verify_all` CRC32 receipt・`ExtractionSession`
  の `ExtractedTree`・Synvelis semantic 検証を経て commit
  され、reopen で project が復元される。

### Rule G-07: Export Golden

- **Given**: save 済み project。
- **When**: backup export を実行する。
- **Then**: 単一 .zip が deterministic path 順で生成され、
  `verify_all` CRC32 receipt と seal+readback verify を経て
  publish される。export file は Project Asset ではない。

### Rule N-01: Stale Writer Negative

- **Given**: saved_revision = N の project を 2 つの
  controller が open。
- **When**: controller A が save（N→N+1）した後、
  controller B が `expected_revision=N` で `apply`/`save`
  を試行する。
- **Then**: B の CAS が reject され、HEAD は A の
  revision を維持する。stale writer は rollback を起こさない。

### Rule N-02: External Modification Negative

- **Given**: save 済み project の `HEAD` または
  `snapshots/r<N>.json` を外部から改竄。
- **When**: reopen/`head()`/`read_snapshot()` を実行する。
- **Then**: 実効状態が正直に報告され、改竄が検出される
  （snapshot 衝突分類・envelope parse 失敗・schema
  不整合として分類）。silent な成功を報告しない。

### Rule N-03: Malformed Archive Negative

- **Given**: 壊れた/不正な backup.zip（truncated・bad
  CRC32・不正 header）。
- **When**: import を実行する。
- **Then**: `verify_all` または `ZipArchiveReader` が
  失敗を返し、`ExtractedTree` は生成されない。target
  project は不変。

### Rule N-04: Traversal/Link Negative

- **Given**: `..`・absolute path・drive letter・UNC・
  symlink/hardlink・特殊 entry を含む backup.zip。
- **When**: import を実行する。
- **Then**: `SafeRelativePath`/`ExtractOptions` が拒否し、
  Synvelis semantic allowlist も拒否。link/special entry
  は v1 で一切受理しない。

### Rule N-05: Path/Size Limit Negative

- **Given**: `ArchiveReadLimits` を超える entry 数・
  展開後総 byte 数・path 長/深さ・1 entry byte 数の
  backup.zip。
- **When**: import を実行する。
- **Then**: limit 超過として reject され、`Unsupported`/
  error として分類される。無制限を許容しない。

### Rule N-06: Unknown Schema Negative

- **Given**: `schema_version` が現行未対応の backup.zip。
- **When**: import を実行する。
- **Then**: schema migration path がなければ reject。
  unknown field（`deny_unknown_fields`）は parse 失敗
  として分類。

### Rule N-07: Unknown Plugin State Negative

- **Given**: 未知 plugin state を含む backup.zip。
- **When**: import を実行する。
- **Then**: plugin state は opaque data として schema
  検証され、未知 plugin は警告付きで保持または reject
  （実装 Issue で確定）。silent な破棄をしない。

### Rule N-08: Corrupted Blob/Wrong Hash Negative

- **Given**: blob digest が manifest と一致しない、
  または blob file が破損した backup.zip。
- **When**: import を実行する。
- **Then**: SHA-256 digest 検証で reject。PAP-18 に従い
  corruption/hash mismatch を検出する。

### Rule N-09: Incomplete Output Negative

- **Given**: backup export または import が途中で中断。
- **When**: 対象 file を確認する。
- **Then**: output file は `Incomplete`（StagedFile 未
  publish）として残り、元 project/target は不変。
  incomplete marker が残る。

### Rule N-10: Unsupported FS Capability Negative

- **Given**: atomic rename または dir sync をサポート
  しない FS/OS。
- **When**: save/backup を実行する。
- **Then**: `VisibilityRequirement::RequireAtomic` または
  `Durability::FileAndDirectorySync` が失敗し、
  `Unsupported`/`PublishFailure` として分類される。
  silent な fallback・downgrade をしない（PAP-15）。

### Rule C-01: Snapshot Staging Crash

- **Given**: snapshot publish の write/seal 中に crash。
- **When**: reopen する。
- **Then**: temp file が残留し HEAD は不変。`head()` が
  旧 saved_revision を返す。temp は自動削除されない。

### Rule C-02: Snapshot Publish Crash

- **Given**: snapshot publish の rename 中に crash。
- **When**: reopen する。
- **Then**: `PublishEffect` が `NotPublished`/`Published`/
  `Indeterminate` のいずれかを返し、`head()` readback
  で実効状態を確認。HEAD は publish 成功時のみ更新。

### Rule C-03: HEAD Publish Crash

- **Given**: HEAD publish 中に crash。
- **When**: reopen する。
- **Then**: durable orphan snapshot が残る可能性を許容。
  `head()` が旧/新 revision のいずれかを正直に返す。
  orphan snapshot は retention policy で管理。

### Rule C-04: Dir Sync Crash

- **Given**: publish 成功後・dir sync 中に crash。
- **When**: reopen する。
- **Then**: `PublishEffect::Published` + dir sync 失敗
  として報告。durability 未達を成功と見なさない。

### Rule C-05: Backup Crash

- **Given**: backup export 中に crash。
- **When**: output file を確認する。
- **Then**: `.zip` は `Incomplete`（seal/publish 未了）。
  元 project は不変。

### Rule C-06: Import Crash

- **Given**: import の extraction/検証/commit 中に crash。
- **When**: target project を確認する。
- **Then**: `ExtractedTree` staging が残留し、target
  project は不変または incomplete marker 付き。
  commit 未了なら project は変更されない。

### Rule C-07: Reopen After Lost Ack

- **Given**: HEAD publish 成功後・ack 応答前に crash。
- **When**: reopen する。
- **Then**: `head()` readback で実効 saved_revision を
  確認し、PAP-07 に従い lost acknowledgement を処理。

## Examples

実装時に各 Rule へ具体的な test case を割り当て、
Case ID・Expected・Observed を記録する。以下は case の
割当例（実装 Issue で確定）:

| Rule | 対象 crate/層 | 想定 test 位置 |
| --- | --- | --- |
| G-01..G-07 | `synvelis-project` store/controller | `store_publish.rs`・`controller` test・E2E |
| N-01..N-02 | `synvelis-project` controller/store | `controller` CAS test・`store` collision test |
| N-03..N-08 | 将来の backup/import 実装 | `synvelis-project` または専用 crate の import test |
| N-09..N-10 | `synvelis-project` store | `store` publish failure test |
| C-01..C-07 | `synvelis-project` store/controller | crash simulation test・reopen test |

## Gate Evidence (Ruviform #288)

本 test-spec の case は Ruviform #288 consumer gate へ
提出する Synvelis 独立証拠の一部を構成する。各 case の
実行結果（Pass/Fail/Unsupported/NotRun）を gate 証拠
項目（GATE-SV-01..09）へ対応付ける。

- GATE-SV-04（save/reopen/readback）: G-01, G-02, C-01..C-04, C-07
- GATE-SV-05（stale-writer）: N-01, N-02
- GATE-SV-06（crash/recovery）: C-01..C-07
- GATE-SV-07（archive/import safety）: N-03..N-09, G-06, G-07
- GATE-SV-08（performance/non-RT）: file I/O が non-RT で
  あることの静的確認 + 性能計測（NotRun）
- GATE-SV-09（実機 FS crash/durability）: **NotRun**

## Deferred

- Backup/Import UI・製品 format 最終凍結・実機 FS
  crash/durability 実測は実装 Issue（#84/#86/RF#288）へ。
- Autosave の具体的保存タイミング・revision 採番は
  consumer 実装 Issue で確定。
