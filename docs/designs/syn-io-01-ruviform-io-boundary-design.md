---
id: "DESIGN-034Uvylguz0kZUgy1zbseh"
type: "design"
status: "approved"
title: "SYN-IO-01 Ruviform 共通 I/O 適合境界設計"
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
  implements:
    - "docs/specs/syn-io-01-ruviform-common-io-boundary.md"
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
  derives-from:
    - "docs/specs/syn-io-01-ruviform-common-io-boundary.md"
    - "docs/discovery/syn-io-01-io-boundary-inventory.md"
    - "docs/adr/syn-io-01-backup-import-archive-zip-limits-path-link.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/syn-io-01-io-boundary-verification.md"
  references:
    - "https://github.com/scarecrow173/ruviform/issues/282"
    - "https://github.com/scarecrow173/ruviform/issues/283"
    - "https://github.com/scarecrow173/ruviform/issues/284"
    - "https://github.com/scarecrow173/ruviform/issues/288"
    - "https://github.com/scarecrow173/ruviform/issues/294"
    - "https://github.com/scarecrow173/ruviform/issues/295"
    - "https://github.com/scarecrow173/ruviform/issues/296"
    - "https://github.com/scarecrow173/ruviform/issues/297"
    - "https://github.com/scarecrow173/ruviform/issues/298"
    - "https://github.com/scarecrow173/ruviform/issues/299"
    - "https://github.com/scarecrow173/ruviform/issues/300"
    - "https://github.com/scarecrow173/ruviform/issues/409"
    - "https://github.com/scarecrow173/ruviform/issues/412"
  defers: []
  deferred-by: []
---

# SYN-IO-01 Ruviform 共通 I/O 適合境界設計

## Context

Synvelis Issue #82 は、Ruviform 共通 I/O（`ruviform-io-file`/
`ruviform-io-structured`/`ruviform-io-archive`/
`ruviform-io-compression`/rooted FS）と Synvelis Project
I/O（`DirectoryBundleStore`・`ProjectController`・
`ProjectStore` port・Recovery/Backup/Import）の責務境界を
設計文書として固定する Issue である。実装は #84/#86 と
Ruviform #288 へ委譲される。

本設計は `docs/discovery/syn-io-01-io-boundary-inventory.md`
の棚卸し結果と、`docs/specs/syn-io-01-ruviform-common-io-boundary.md`
の要求事項、`docs/adr/syn-io-01-backup-import-archive-zip-limits-path-link.md`
の archive 決定を前提に、8 つの受入条件を満たす設計内容を
定義する。

## Scope

### In Scope

- 責務分担表（Synvelis `DirectoryBundleStore`/JSON
  serialization・`ProjectController`/`ProjectStore`・
  Recovery/Backup/Import policy と、Ruviform File/StagedFile・
  Structured・Archive・Compression・rooted FS の境界）。
- Owner 固定表（ExpectedRevision・HEAD・immutable Snapshot・
  外部変更・Crash 後 Reopen）。
- 失敗写像表（置換・Sync・indeterminate・rollback・HA
  storage・unsupported capability → `ProjectStore`）。
- 状態遷移図（staged file lifecycle・snapshot publish・
  HEAD publish・crash/reopen・backup/import staging）。
- Backup/Import 設計（ADR の決定を設計レベルで具体化）。
- 検証計画要約（test-spec への参照）と Ruviform #288 gate
  証拠項目。
- 既存 project-save E2E 非退行・type/DAG/OS/feature 影響・
  consumer 意味論維持の明記。

### Out of Scope

- `crates/` 以下のコード変更、manifest/lockfile/CI 変更。
- Ruviform 公開 API の fork・拡張、Project HEAD CAS の
  共通 File API への追加。
- 実機 FS crash/durability 実測（NotRun）。
- Backup/Import UI・製品 format 最終凍結。

## 責務分担

### 責務分担表

| 責務 | Owner | Ruviform 提供範囲 | Synvelis 所有範囲 |
| --- | --- | --- | --- |
| Project schema・canonical serialization | Synvelis | — | `SnapshotEnvelope`（schema_version・project_id・revision・project）、serde_json + `BTreeMap` canonical byte、`deny_unknown_fields`、schema migration |
| ProjectRevision・ExpectedRevision | Synvelis | — | `ProjectController::apply` CAS、単調増加 revision、undo/redo で巻き戻さない |
| SavedRevision/HEAD | Synvelis | — | `HEAD` file が指す saved revision、`ProjectStore::head()`/`read_snapshot()` |
| immutable Snapshot | Synvelis | `PublishMode::CreateNew`（atomic no-replace） | `snapshots/r<N>.json` 命名規則、collision 分類（Satisfied/Refresh/Conflict）、byte 同一再送の冪等受理と再 publish による durability 再確立 |
| 単一 writer・保存順序 | Synvelis | — | snapshot publish → HEAD publish の順序固定、`ProjectController` が唯一の writer |
| 外部変更検出 | Synvelis | `ReadbackFile`/`head()` readback | `head()` による実効的検出、snapshot 衝突分類、reopen 時の saved/live/recovery revision 比較 |
| Crash 後 Reopen 解釈 | Synvelis | `PublishEffect`/`RecoveryHandle`/`head()` | `load()` の HEAD→snapshot→migration→validate、`PublishEffect::Indeterminate` の残留 temp を自動削除せず readback 判断へ接続、orphan snapshot の許容と `head()` 正直 readback |
| Recovery/Autosave policy | Synvelis | — | RecoveryRevision domain、Autosave の保存タイミング・revision 採番（consumer 固有意味論） |
| Backup/Import policy | Synvelis | `ZipArchiveWriter`/`Reader`・`ExtractionSession`/`ExtractedTree`・`SafeRelativePath`・`ArchiveReadLimits`・compression | manifest/schema/blob digest/semantic allowlist 検証、import transaction、incomplete target 処理、元 project 不変性、Project Asset 非該当の export file 扱い |
| staged file lifecycle | Ruviform | `StagedFile→SealedFile→publish`、phase・temp path・cleanup | publish 呼び出し側としての option 選択（`PublishMode`/`Durability`/`VisibilityRequirement`）と effect 分類の転写 |
| generic publish outcome | Ruviform | `PublishEffect`/`PublishPhase`/`PublishFailureKind`/`PublishReceipt` | `Error::Store` への分類転写、`Indeterminate` の扱い、`RecoveryHandle` の保持/破棄判断 |
| OS/FS capability | Ruviform | durability/visibility capability、unsupported の非 silent downgrade | `RequireAtomic`+`FileAndDirectorySync` 要求、unsupported 時の `Unsupported`/fallback 判断（PAP-15） |
| archive entry/path/limit | Ruviform | `SafeRelativePath`・`ExtractOptions`・`ArchiveReadLimits`・`ExtractedTree` | project 用 limit 値の絞り込み、semantic allowlist、manifest 検証、link/special entry の v1 全拒否 |
| compression codec | Ruviform | gzip/zstd/deflate Encoder/Decoder・finish receipt | ZIP per-entry method 選択（Stored/Deflate）、独立 stream の初期対象外 |
| structured data (generic) | Ruviform | json/yaml/toml/csv/xml `read_file`/`write_file`・strict EOF・`InputLimits` | `SnapshotEnvelope` canonical byte の維持（byte 同一性が前提の採用判断は後続） |
| rooted path containment | Ruviform | `RootedDir`/`open_scoped_root`/`read_dir_each`/`FollowMode`/`RelativePath` | `LocalBlobDir`/`LocalProvider` での capability 利用、project-root trust・semantic validation |
| non-RT 保証 | Ruviform | 全公開 API non-RT | file I/O を audio RT/playback path に入れないことの維持 |

### データ/状態遷移差の図示

#### DirectoryBundleStore vs Ruviform File/Structured API

```text
Synvelis ProjectStore (意味論層)
  save(revision, envelope_bytes)
    │
    ├─ snapshot publish ──────────────► Ruviform StagedFile
    │   PublishMode::CreateNew           create temp → write →
    │   Durability::FileAndDirectorySync seal → publish
    │   VisibilityRequirement::RequireAtomic
    │   → PublishEffect/Phase/Kind ──► Error::Store 分類転写
    │
    └─ HEAD publish ──────────────────► Ruviform StagedFile
        PublishMode::Replace              (同上 option)
        → head() readback で実効状態確認

Ruviform File API (機構層)
  StagedFile → SealedFile → publish
    - temp file 作成・write・sync
    - seal（書き込み確定・検証）
    - publish（atomic rename or replace）
    - dir sync（FileAndDirectorySync 時）
    - readback verify（SealedFile reopen）
  → PublishReceipt / PublishFailure{effect,phase,kind,...}

Synvelis JSON serialization (canonical 層)
  SnapshotEnvelope → serde_json::to_vec → BTreeMap canonical byte
  schema_version=8, project_id, revision, project, deny_unknown_fields
  ※ Ruviform Structured API は採用範囲のみ定義、canonical byte は Synvelis 維持
```

#### 状態遷移: snapshot/HEAD publish

```text
[ProjectController.save(expected_revision)]
  │ expected_revision != live_revision → CAS reject（Synvelis）
  │ live_revision += 1
  │ envelope = serialize(SnapshotEnvelope{schema_version, project_id, revision, project})
  │
  ├─► store.save(revision, envelope_bytes)
  │     │
  │     ├─ snapshot publish ─► StagedFile(temp)→seal→publish(CreateNew)
  │     │     ├─ Ok + FileAndDirectorySync → durable snapshot
  │     │     ├─ Err(PublishFailure) → Error::Store（HEAD へ進まない）
  │     │     │     effect=NotPublished → 旧 HEAD 維持・rollback 不要
  │     │     │     effect=Published    → snapshot durable・HEAD 未更新
  │     │     │     effect=Indeterminate→ temp 残留・head() readback 判断
  │     │     └─ collision（同名存在）→ snapshot_collision 分類
  │     │           Satisfied → kept byte 再 publish で durability 再確立
  │     │           Refresh    → watermark 更新して再試行
  │     │           Conflict   → Error::Store（異 byte 同名）
  │     │
  │     └─ HEAD publish ─► StagedFile(temp)→seal→publish(Replace)
  │           ├─ Ok + FileAndDirectorySync → saved_revision = revision
  │           ├─ Err(PublishFailure) → Error::Store
  │           │     effect=NotPublished → 旧 HEAD 維持・orphan snapshot 許容
  │           │     effect=Published    → HEAD durable・dir sync 未達の可能性
  │           │     effect=Indeterminate→ temp 残留・head() readback 判断
  │           └─ head() readback → 実効 saved_revision 確認
  │
  └─► saved_revision 更新（store 成功時のみ）
```

#### 状態遷移: crash/reopen

```text
[Crash points during save]
  crash before snapshot publish completes
    → temp file 残留（staged・未 publish）・HEAD 不変・saved_revision 不変
  crash after snapshot publish, before HEAD publish
    → durable orphan snapshot（snapshots/r<N>.json 存在・HEAD 未参照）
    → reopen: head() が旧 revision を返す・orphan は無害
  crash after HEAD publish, before dir sync
    → PublishEffect::Published + dir sync 失敗として報告
    → reopen: head() が新 revision を返す（durability 未保証を正直に報告）
  crash during HEAD publish (indeterminate)
    → temp 残留・HEAD が旧/新どちらか不明
    → reopen: head() readback で実効状態を確認・RecoveryRevision で判断

[Reopen sequence]
  load()
    → head() → HEAD file 読み取り → saved_revision 確定
    → read_snapshot(saved_revision) → envelope parse → migration → validate
    → RecoveryRevision（journal/別 domain）と Saved/HEAD を比較
    → live_revision = max(saved, recovery, journal) 等の consumer policy
```

#### 状態遷移: backup/import

```text
[Backup]
  collect project bundle files (HEAD, snapshots/*, manifest)
    + referenced verified blobs
  → ZipArchiveWriter(StagedFile) へ deterministic path 順で entry 書込み
  → seal → readback verify (reopen + verify_all CRC32)
  → publish(CreateNew) → backup.zip
  中断 → StagedFile Incomplete・元 project 不変

[Import]
  ZipArchiveReader → verify_all (CRC32 receipt)
  → ExtractionSession → ExtractedTree (staged・検証済み tree・非 publish)
  → Synvelis semantic 検証:
      manifest format_version・project_id・revision
      schema_version 対応・blob SHA-256 digest 一致
      semantic allowlist（snapshots/, HEAD, manifest, blobs/ のみ）
      link/special entry 全拒否・path/limit 検証済み
  → commit（新 project 作成 or 既存への merge — 実装 Issue で確定）
  検証失敗 → ExtractedTree 破棄・target 不変・incomplete marker
```

## Owner 固定

| 意味論 | Owner | 根拠 |
| --- | --- | --- |
| ExpectedRevision（CAS 引数） | Synvelis `ProjectController` | `apply(expected_revision)` が live_revision と比較し stale writer を拒否。Ruviform `PublishMode` は revision/CAS を持たない。 |
| HEAD/SavedRevision | Synvelis `ProjectStore`+`ProjectController` | `HEAD` file・`head()`・`saved_revision` は project pointer であり、generic file の dest path ではない。 |
| immutable Snapshot | Synvelis `DirectoryBundleStore` | `snapshots/r<N>.json` 命名・collision 分類・冪等受理は project policy。`PublishMode::CreateNew` は機構のみ提供。 |
| 外部変更検出 | Synvelis | `head()` readback・snapshot 衝突分類・reopen 時 revision 比較。Ruviform は file 存在/内容の readback のみ。 |
| Crash 後 Reopen 解釈 | Synvelis | `load()`・`PublishEffect` 分類・`RecoveryRevision`・orphan 許容・journal replay。Ruviform は effect/phase/kind の分類まで。 |
| Recovery/Autosave policy | Synvelis | RecoveryRevision domain・Autosave timing・revision 採番は consumer 固有意味論。 |
| Backup/Import policy | Synvelis | manifest/schema/blob digest/semantic allowlist/transaction/元 project 不変性。Ruviform は archive 機構と path/limit 安全性のみ。 |

## 失敗写像（ProjectStore へのマッピング）

| Ruviform 概念 | ProjectStore/Controller での扱い |
| --- | --- |
| `PublishMode::CreateNew` | snapshot publish に使用。既存 `snapshots/r<N>.json` への no-replace を保証し、衝突時は `snapshot_collision` で Satisfied/Refresh/Conflict に分類。 |
| `PublishMode::Replace` | HEAD publish に使用。無条件 create-or-replace で HEAD file を更新。revision/CAS/multi-file transaction は持たないため、snapshot durable 確認後にのみ呼ぶ。 |
| `Durability::FileAndDirectorySync` | snapshot/HEAD 両方に要求。dir sync 失敗は `PublishEffect::Published` 付き `Err` として報告され、`Error::Store` へ転写。durability 未達を成功と見なさない。 |
| `VisibilityRequirement::RequireAtomic` | 全 publish に要求。atomic rename 不能な FS では publish 自体が失敗し、弱い fallback へ silent downgrade しない。 |
| `PublishEffect::NotPublished` | publish 未完了。旧 HEAD/snapshot 状態を維持。rollback 不要（何も公開されていない）。 |
| `PublishEffect::Published` | publish 成功（dir sync 未達の可能性あり）。snapshot は durable、HEAD は dir sync 状態に依存。`head()` readback で実効状態確認。 |
| `PublishEffect::Indeterminate` | publish 結果不明。temp file 残留を自動削除せず、`RecoveryHandle` を保持。`head()`/`read_snapshot()` で実効状態を確認してから判断。 |
| `PublishPhase` | 失敗が起きた段階（write/seal/publish/dir-sync/readback）を `Error::Store` メッセージへ転写し、原因箇所を特定可能にする。 |
| `PublishFailureKind` | IO error/unsupported capability/validation 等の分類を `Error::Store` へ転写。unsupported capability は `Unsupported` として扱い fallback しない。 |
| `RecoveryHandle` | indeterminate/cleanup 未完了時に保持。reopen/回復判断後に破棄または retry に使用。 |
| `ReadbackFile`/`head()` | publish 後の実効状態確認。`head()` が正直な saved_revision を返し、orphan snapshot を参照しない。 |
| HA/network storage | network FS・同期フォルダ等では dir sync/durability が `Unknown`/`Indeterminate` になり得る。Synvelis はこれを成功と見なさず、`Unsupported` または警告付きで扱う。PAP-15 に従い capability 不足を silent に隠さない。 |
| rollback/retention | `NotPublished` 時は rollback 不要。`Published`/`Indeterminate` 時は `head()` readback で実効 HEAD を確認し、orphan snapshot は retention policy（実装 Issue）で管理。snapshot の自動削除は行わない。 |

## Backup/Import 設計

ADR `docs/adr/syn-io-01-backup-import-archive-zip-limits-path-link.md`
の決定を設計レベルで具体化する。

### Backup

```text
入力: project root (DirectoryBundleStore layout) + verified blobs
処理:
  1. bundle file 収集（HEAD, snapshots/r*.json, manifest）
  2. referenced blob 収集（SHA-256 digest で検証済みのもののみ）
  3. ZipArchiveWriter(StagedFile) へ deterministic path 順で entry 書込み
     - entry path: bundle 相対 path を SafeRelativePath として使用
     - compression: Stored or Deflate（entry ごとに選択）
     - manifest entry: format_version, project_id, revision,
       snapshot list, blob digest list, plugin_state flag
  4. seal → readback verify（reopen + verify_all CRC32 receipt）
  5. publish(CreateNew) → backup.zip
失敗時:
  - StagedFile Incomplete・元 project 不変
  - PublishEffect 分類を Error::Store へ転写
```

### Import

```text
入力: backup.zip（信頼できない入力として扱う）
処理:
  1. ZipArchiveReader → verify_all（CRC32 receipt・limits 適用）
  2. ExtractionSession → ExtractedTree（staged・検証済み tree・非 publish）
  3. Synvelis semantic 検証:
     - manifest format_version・project_id・revision 妥当性
     - schema_version 対応（migration path の有無）
     - blob SHA-256 digest 一致（#86 導入後）
     - semantic allowlist: snapshots/, HEAD, manifest, blobs/ のみ
     - link/special entry 全拒否・path/limit は Ruviform 検証済み
  4. commit: 新 project 作成 or 既存への merge（実装 Issue で確定）
     - commit は StagedFile publish + validation を伴う transaction
失敗時:
  - ExtractedTree 破棄・target project 不変
  - incomplete marker（target path に .incomplete 等）を残す
```

### Guardrails

- backup file は Project Asset ではない。import/commit で
  明示的に project へ取り込まれるまで asset 扱いしない。
- export file path・media metadata・plugin state は import 側で
  信頼しない。semantic 検証を必須とする。
- credentials・plugin binary・sensitive path を backup entry に
  含めない。
- 元 project は backup/import 中に変更されない（PAP-17）。

## 検証計画（test-spec への参照）

詳細な Golden/Negative/Crash case は
`docs/test-specs/syn-io-01-io-boundary-verification.md` に定義
する。設計上の要点:

- **Golden**: Saved Revision save→reopen、Recovery/Undo、
  Asset commit、Plugin State 復元、Import/Export roundtrip。
- **Negative**: stale writer（expected_revision mismatch）、
  外部変更（HEAD/snapshot 改竄）、malformed archive、
  traversal/link/size limit、unknown schema、corrupted blob、
  wrong hash、incomplete output、unsupported FS capability。
- **Crash**: snapshot staging/publish・HEAD publish・dir sync・
  backup・import・reopen・lost acknowledgement。
- 全 case は Case ID + Expected/Observed を記録し、設計段階は
  NotRun とする。

## Ruviform #288 Gate 証拠項目

Synvelis が Ruviform #288（IO-06 consumer gate）へ提出する
独立証拠の項目。各項目は Pass/Fail/Unsupported/NotRun で記録。

| 項目 | 内容 | 現状 |
| --- | --- | --- |
| GATE-SV-01 | Synvelis revision・解決 Ruviform SHA | Cargo.lock 記録済み（実装時に再確認） |
| GATE-SV-02 | API/feature/DAG 互換性 | `ruviform-io-file` 依存済み・`StagedOutput` 参照なし（IO-05 sweep 済み） |
| GATE-SV-03 | OS/provider capability matrix | NTFS/Windows 実機は Pass 見込み・他 OS は NotRun |
| GATE-SV-04 | save/reopen/readback | store.rs の publish/readback 設計済み・E2E test-spec で検証予定 |
| GATE-SV-05 | stale-writer 検出 | ExpectedRevision CAS・外部変更検出は Synvelis policy・test-spec で検証予定 |
| GATE-SV-06 | crash/recovery | `PublishEffect` 分類・orphan 許容・`head()` readback 設計済み・実機 crash は NotRun |
| GATE-SV-07 | archive/import safety | ADR 決定済み・traversal/link/limit/CRC32 証拠は test-spec で定義・実装後に実行 |
| GATE-SV-08 | performance/precision/non-RT | file I/O は non-RT・audio RT path 非含有を維持・性能計測は NotRun |
| GATE-SV-09 | 実機 FS crash/durability | **NotRun**（計画のみ・実装 Issue で実施） |

Synvelis 証拠は AudioLens 等他 consumer の証拠と独立に採取する。

## 非退行・影響文書化

### 既存 project-save E2E 非退行

- `crates/synvelis-project` の store/controller test
  （`store_publish.rs`・`controller` 系）・`synvelis-gateway`
  の commit_gate/FileJournal test・L1/L2 E2E は、
  `ProjectStore` port（`save`/`head`/`read_snapshot`）不変を
  前提に非退行を要求する。
- 本設計は port signature を変更しない。`DirectoryBundleStore`
  の内部実装（StagedFile publish）は syn-io-02 で移行済み。

### Ruviform API 移行の影響

- **type**: `StagedFile`/`SealedFile`/`PublishReceipt`/
  `PublishFailure`/`PublishEffect`/`PublishPhase`/
  `PublishFailureKind`/`RecoveryHandle`/`ReadbackFile` を
  `Error::Store` 分類へ写像。`ProjectStore` port 型は不変。
- **DAG**: `synvelis-project` → `ruviform-io-file` 依存は
  既存。`ruviform-io-archive`/`ruviform-io-structured`/
  `ruviform-io-compression` は Backup/Import 実装時に追加
  （依存方向は `synvelis-*` → `ruviform-*` のまま）。
- **OS**: `PublishMode`/`Durability`/`VisibilityRequirement`
  の option 組合せが OS/FS capability に依存。unsupported
  時は `Unsupported` で拒否し fallback しない。
- **feature**: `ruviform-io-file` の `file` feature は既存。
  archive/structured/compression の feature は実装 Issue で
  有効化を決定。

### Consumer 意味論維持

- Revision/HEAD/CAS/Recovery/Autosave の consumer 固有
  意味論は Synvelis に保持。Ruviform API は generic 機構のみ
  提供し、project 意味論を要求・提供しない。
- 旧 API 互換層は設けない。恒久 SHA 固定にも依存しない
  （Cargo.lock の解決 revision 記録は再現性のための既存
  規約であり、API 互換層ではない）。

## 関連文書

- Spec: `docs/specs/syn-io-01-ruviform-common-io-boundary.md`
- ADR: `docs/adr/syn-io-01-backup-import-archive-zip-limits-path-link.md`
- Discovery: `docs/discovery/syn-io-01-io-boundary-inventory.md`
- Test spec: `docs/test-specs/syn-io-01-io-boundary-verification.md`
- Plan: `docs/plans/syn-io-01-ruviform-io-boundary-design.md`
- Persistence semantics: `docs/designs/project-asset-persistence-architecture.md`
- Prior migration: `docs/designs/syn-io-02-directory-bundle-store-stagedfile-design.md`
- WAV export precedent: `docs/designs/syn-rf305-wav-export-staged-file-design.md`
