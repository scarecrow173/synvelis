---
id: "SPEC-034Uvy3JSMhpGuQliLcN6s"
type: "spec"
status: "approved"
title: "SYN-IO-01 Ruviform 共通 I/O 適合境界 — Project Snapshot/HEAD/Backup/Import"
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
  implemented-by:
    - "docs/designs/syn-io-01-ruviform-io-boundary-design.md"
    - "docs/plans/syn-io-01-ruviform-io-boundary-design.md"
  depends-on:
    - "docs/specs/syn-io-02-snapshot-head-staged-publish.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/project-asset-persistence-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/syn-io-01-io-boundary-inventory.md"
  derived-by:
    - "docs/adr/syn-io-01-backup-import-archive-zip-limits-path-link.md"
    - "docs/designs/syn-io-01-ruviform-io-boundary-design.md"
    - "docs/plans/syn-io-01-ruviform-io-boundary-design.md"
  verifies: []
  verified-by:
    - "docs/test-specs/syn-io-01-io-boundary-verification.md"
  references:
    - "https://github.com/scarecrow173/ruviform/issues/282"
    - "https://github.com/scarecrow173/ruviform/issues/288"
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/86"
  defers: []
  deferred-by: []
---

# SYN-IO-01 Ruviform 共通 I/O 適合境界 — Project Snapshot/HEAD/Backup/Import

## What Should Be Built

Synvelis の Project I/O（Snapshot・HEAD・Backup・Import）と
Ruviform 共通 I/O 公開 API の間の責務境界を定義する設計文書
チェーンを `docs/` に整備する。対象は設計のみであり、コード
実装は Synvelis #84/#86 と Ruviform #288（IO-06 consumer gate）
へ委譲する。本 spec は境界の要求事項と受入条件を固定し、
design/plan/task/test-spec の下流文書がそれを実現する。

具体的な成果物は以下の 8 項目を設計文書として満たすこと:

1. `DirectoryBundleStore`/JSON serialization と Ruviform
   File/Structured API の責務・データ/状態遷移差の図示。
2. ExpectedRevision・HEAD・immutable Snapshot・外部変更・
   Crash 後 Reopen の Owner を Synvelis に固定。
3. 置換・Sync・indeterminate・rollback・HA storage の
   `ProjectStore` へのマッピング。
4. Backup/Import の ZIP/TAR/Compression 初期採用範囲と
   Limits/Link/Path/保存形式の設計。
5. Saved Revision・Recovery・Undo・Asset・Plugin State・
   Import・Export の Golden・Negative・Crash test 計画。
6. 既存 project-save E2E の非退行と、Ruviform API 移行の
   type/DAG/OS/feature 影響の文書化。
7. Ruviform #288 gate へ送る Synvelis 独立検証証拠の項目定義
   （未実測は NotRun）。
8. Revision/HEAD/CAS/Recovery/Autosave の consumer 固有
   意味論維持、旧 API 互換層・恒久 SHA 固定への非依存。

## Why Now

- SYN-IO-02（#84 系列）で `DirectoryBundleStore` の snapshot/
  HEAD publish が Ruviform `StagedFile` へ移行済みだが、
  「Ruviform が保証する範囲」と「Synvelis が所有する project
  意味論」の境界を明文化した設計文書が存在しない。
- Backup/Import・Structured API 採用・#86 asset CAS・
  #288 consumer gate の各後続作業が、この境界設計を前提に
  進むため、先に固定する必要がある。
- Ruviform 側は #282–#306 の IO-01..06 で共通機構を整備中で
  あり、consumer（Synvelis）が自らの意味論をどこに置くかを
  明示しないと、generic API へ project 意味論が混入する
  リスクがある（Issue が禁止事項として明示）。

## Users and Value

- Synvelis #84/#86・Ruviform #288 の実装者: 境界・Owner・
  失敗写像・検証証拠項目が確定した設計文書により、
  実装時に意味論の置き場所で迷わない。
- Synvelis ユーザー: crash/外部変更/stale writer/backup
  import の各局面で、store が虚偽の成功を報告せず、
  project 意味論が generic I/O 機構に侵食されない。
- Ruviform: consumer が必要とする generic 機構の範囲が
  明確になり、#288 gate の証拠要求に対して Synvelis 側の
  独立検証項目を提示できる。

## Scope

### In Scope

- `docs/` 配下の設計文書チェーン:
  - `docs/discovery/syn-io-01-io-boundary-inventory.md`
  - `docs/specs/syn-io-01-ruviform-common-io-boundary.md`
    （本書）
  - `docs/adr/syn-io-01-backup-import-archive-zip-limits-path-link.md`
  - `docs/designs/syn-io-01-ruviform-io-boundary-design.md`
  - `docs/plans/syn-io-01-ruviform-io-boundary-design.md`
  - `docs/tasks/syn-io-01-task{1,2}-*.md`
  - `docs/test-specs/syn-io-01-io-boundary-verification.md`
- 責務分担表・Owner 固定表・失敗写像表・状態遷移図・
  Backup/Import archive 設計・検証計画・gate 証拠項目を
  含む設計内容。
- 既存文書チェーン（syn-io-02・syn-rf305・PAP architecture）
  との relation 設定と managed index 更新。

### Out of Scope

- `crates/` 以下のコード変更全般（実装は #84/#86/RF#288）。
- `Cargo.toml`/`Cargo.lock`/`mise.toml`/CI の変更。
- Ruviform 公開 API の fork・拡張設計。
- Project HEAD CAS を共通 File API へ追加する設計。
- 実機 FS crash/durability の実測（NotRun として計画記述）。
- Backup/Import の UI・製品 format の最終凍結（実装 Issue）。

## Requirements

### 境界と Owner

- ExpectedRevision（`apply` の CAS 引数）・HEAD/SavedRevision・
  immutable Snapshot 名・外部変更検出・Crash 後 Reopen 解釈・
  Recovery/Backup/Import 方針の Owner は Synvelis に固定する。
- Ruviform は generic file 操作の outcome（effect/phase/
  durability/visibility）と archive の path/limit 機構を提供
  するが、project 意味論・manifest 検証・transaction 意味論は
  提供しない。

### 失敗写像

- `PublishMode`（CreateNew/Replace）・`Durability`・
  `VisibilityRequirement`・`PublishEffect`・`PublishPhase`・
  `PublishFailureKind`・`RecoveryHandle`・`ReadbackFile` を
  `ProjectStore`/`ProjectController` の既存分類へ写像する。
- `PublishMode::Replace` は revision/CAS/multi-file
  transaction 意味論を持たないことを前提に、HEAD 置換と
  snapshot immutable 名を別操作として扱う。
- `Indeterminate` は残留 temp を自動削除せず、`head()`/
  readback による再読込判断へ接続する。HEAD publish 失敗が
  durable orphan snapshot を残すことを許容し、reopen で
  `head()` が正直な状態を返すことを契約とする。

### Backup/Import

- 初期採用範囲・Limits・Link/Path ルール・保存形式を設計
  する。archive format 決定は ADR に記録する。
- export された backup file は Project Asset ではない
  （明示的に import/commit されるまで asset 扱いしない）。
- backup/import は元 project を変更しない。中断時は target を
  incomplete として扱い、original は immutable を維持する。

### 検証と非退行

- Golden/Negative/Crash の test 計画を test-spec として定義し、
  Saved Revision・Recovery・Undo・Asset・Plugin State・
  Import・Export を網羅する。
- 既存 project-save E2E（store/controller/gateway）の非退行を
  要求し、Ruviform API 移行の type/DAG/OS/feature 影響を
  文書化する。
- 実装時の検証は Case ID + Expected/Observed を記録し、
  設計段階の case は NotRun とする。

### Gate 証拠

- Ruviform #288 へ送る Synvelis 独立証拠の項目を定義し、
  各項目を Pass/Fail/Unsupported/NotRun で記録可能にする。
- Synvelis 証拠は AudioLens 等他 consumer の証拠と独立に
  採取する。

### Consumer 意味論維持

- Revision/HEAD/CAS/Recovery/Autosave の consumer 固有
  意味論を維持する。旧 API 互換層や恒久 SHA 固定に依存
  しない設計とする（Cargo.lock の解決 revision 記録は
  再現性のための既存規約であり、API 互換層ではない）。

## Acceptance Criteria

- [ ] AC-1: `DirectoryBundleStore`/JSON serialization と
  Ruviform File/Structured API の責務・データ/状態遷移差が
  図示されている。
- [ ] AC-2: ExpectedRevision・HEAD・immutable Snapshot・
  外部変更・Crash 後 Reopen の Owner が Synvelis に固定
  されている。
- [ ] AC-3: 置換・Sync・indeterminate・rollback・HA storage
  が `ProjectStore` へマッピングされている。
- [ ] AC-4: Backup/Import の ZIP/TAR/Compression 初期採用
  範囲・Limits・Link・Path・保存形式が設計されている。
- [ ] AC-5: Saved Revision・Recovery・Undo・Asset・Plugin
  State・Import・Export の Golden・Negative・Crash test
  計画が定義されている。
- [ ] AC-6: 既存 project-save E2E 非退行と Ruviform API
  移行の type/DAG/OS/feature 影響が文書化されている。
- [ ] AC-7: Ruviform #288 gate への Synvelis 独立検証証拠
  項目が定義され、未実測は NotRun と記録されている。
- [ ] AC-8: Revision/HEAD/CAS/Recovery/Autosave の consumer
  固有意味論維持と、旧 API 互換層/恒久 SHA 固定非依存が
  明記されている。

## Deferred Design Concerns

- Backup/Import の UI・製品 format の最終凍結・
  実機 FS crash/durability 実測は実装 Issue（#84/#86/
  RF#288）へ送る。
- Structured API の `SnapshotEnvelope` への適用判断は
  byte 同一性検証を伴う後続移行判断。
- 外部変更検出の将来強化（generation/mtime 等）は
  Synvelis 側で継続設計。

## Sources and Relations

- Source Issue: <https://github.com/scarecrow173/synvelis/issues/82>
- Upstream design authority: Ruviform <https://github.com/scarecrow173/ruviform/issues/282>
- Consumer gate: Ruviform <https://github.com/scarecrow173/ruviform/issues/288>
- Implementation delegation: Synvelis #84, #86; Ruviform #288
- Discovery: `docs/discovery/syn-io-01-io-boundary-inventory.md`
- Persistence semantics: `docs/designs/project-asset-persistence-architecture.md`
- Prior staged-publish migration: `docs/specs/syn-io-02-snapshot-head-staged-publish.md`
