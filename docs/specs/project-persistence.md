---
id: "SPEC-034X6Kqk3e6SN2g7MWSiIq"
type: "spec"
status: "approved"
title: "Project / Asset / Persistence — Immutable Snapshot・Media Lifecycle・Safe Save・Recovery 設計契約（synvelis#17）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/17"
    - "https://github.com/scarecrow173/synvelis/issues/23"
  changes:
    added:
      - type: "file"
        path: "docs/specs/project-persistence.md"
      - type: "file"
        path: "docs/designs/project-persistence-design.md"
      - type: "file"
        path: "docs/plans/project-persistence.md"
      - type: "file"
        path: "docs/tasks/project-persistence-task.md"
      - type: "file"
        path: "docs/test-specs/project-persistence.md"
      - type: "file"
        path: "docs/impl/ir/project-persistence.md"
      - type: "file"
        path: "docs/impl/exp/project-persistence.md"
      - type: "dir"
        path: "evidence/sv17"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/syn-io-01-ruviform-common-io-boundary.md"
    - "docs/specs/syn-io-02-snapshot-head-staged-publish.md"
    - "docs/specs/syn-io-02-head-cas.md"
    - "docs/specs/stagedfile-migration.md"
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/specs/project-model.md"
    - "docs/designs/project-model-design.md"
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/specs/alib-02-v5-snapshot-compat.md"
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/specs/rcd-02-stagedfile-publish-migration.md"
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
    - "docs/specs/x-recorded-verified-source-playback-export.md"
    - "docs/specs/l1-local-wav-verified-media-graph-playback.md"
    - "docs/specs/l2-master-sequential-offline-wav-export.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/project-asset-persistence-architecture.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/project-persistence.md"
  references:
    - "evidence/sv17/case-ledger.md"
    - "evidence/sv84/case-ledger.md"
    - "evidence/reports/rcd-02-durability-recovery-review-ledger.md"
    - "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    - "https://github.com/scarecrow173/synvelis/issues/82"
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/92"
    - "https://github.com/scarecrow173/synvelis/issues/96"
    - "https://github.com/scarecrow173/synvelis/issues/97"
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/289"
    - "https://github.com/scarecrow173/synvelis/issues/296"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/82"
    - "https://github.com/scarecrow173/synvelis/issues/84"
    - "https://github.com/scarecrow173/synvelis/issues/86"
    - "https://github.com/scarecrow173/synvelis/issues/92"
    - "https://github.com/scarecrow173/synvelis/issues/93"
    - "https://github.com/scarecrow173/synvelis/issues/94"
    - "https://github.com/scarecrow173/synvelis/issues/95"
    - "https://github.com/scarecrow173/synvelis/issues/96"
    - "https://github.com/scarecrow173/synvelis/issues/97"
    - "https://github.com/scarecrow173/synvelis/issues/289"
    - "https://github.com/scarecrow173/synvelis/issues/290"
    - "https://github.com/scarecrow173/synvelis/issues/291"
    - "https://github.com/scarecrow173/synvelis/issues/292"
    - "https://github.com/scarecrow173/synvelis/issues/293"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/synvelis/issues/295"
    - "https://github.com/scarecrow173/synvelis/issues/296"
    - "https://github.com/scarecrow173/synvelis/issues/297"
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "evidence/sv17/case-ledger.md"
  deferred-by: []
---

# Project / Asset / Persistence — Immutable Snapshot・Media Lifecycle・Safe Save・Recovery 設計契約（synvelis#17）

## What Should Be Built

Synvelis Issue #17「Project / Asset / Persistence — Immutable
Snapshot・Media Lifecycle・Safe Save・Recovery」の設計・ADR
Issue として、Project/Asset/Persistence 横断契約を `docs/`
の文書チェーンと `evidence/sv17/` の受入証拠台帳として固定
する。対象は **設計契約の正本化と既存実装との照合** であり、
コード実装は #82/#84/#86・#92〜#97・RCP 系（#289〜#297）等の
各 Owner Issue へ分解済みである。

具体的な成果物:

1. **契約の集約正本** — ProjectController 単一
   Writer/ExpectedRevision CAS・Live/Saved/Recovery/VCS/
   ActivePlan/Materialized Media の Revision 分離・Immutable
   Snapshot + HEAD publish・Asset Identity/Verified Blob・
   Save/Recovery 分離・Source 不変性の各契約を、既存の
   設計正本（`docs/designs/
   project-asset-persistence-architecture.md`）と実装済み
   spec/design（SYN-IO-01/02・VS-A/C/D・ALIB-02・VCS-01/
   02/05・RCD-02・X/L1/L2）へ写像し、Owner・実装 crate・
   fixture 被覆・残 gap を一枚の契約表にする。
2. **StagedFile/CAS との整合契約** — Ruviform
   `ruviform-io-file` の StagedFile/Publish 契約と
   Synvelis Project 意味論（Publish ≠ Project Commit・
   HEAD CAS・orphan・Indeterminate の正直な分類）の境界を
   #84 統合済み実装（`evidence/sv84/case-ledger.md`）と照合
   して記録する。Ruviform Generic File API/Atomic Publish
   の重複実装は行わない。
3. **受入証拠台帳** — Issue #17 の受入条件 13 項目
   （`PROJ-RCP-01` + PAP 系 12 項目）について、Snapshot/HEAD
   stale writer・Concurrent Save・Crash/DiskFull・
   Restore/Retry・Import 失敗・CommitIdentity 再試行・
   GC 保護・Schema Migration・VCS read-only・外部変更検出の
   **既存 Fixture 被覆**を `evidence/sv17/case-ledger.md` に
   Case ID・Expected/Observed/NotRun で台帳化する。
   構成済みテストがない契約は `NotRun` とし、Mock FS の
   合格を実 FS durability の証明にしない。
4. **OS/FS Capability の分離記録** — Windows/macOS/Linux の
   File Sync/DirSync/Atomic Publish/Indeterminate/Recovery
   について、実環境の Capability を別記し、要求状態
   （Required/Deferred/NotApplicable）と実行結果
   （Pass/Fail/NotRun/Unsupported）を分離する
   （POLICIES.md §2/§3）。

## Why Now

- Issue #17 は Project/Asset/Persistence 横断設計の合意を
  管理する正本 Issue であり、2026-11-01 の 10 項目合意・
  2026-11-04/11 の SF-01/VCS/ALIB/RCP/CB 各追補を経て、
  実装は VS-A〜D・SYN-IO-01/02・ALIB-02・VCS-01〜06・
  RCD-02・X/L1/L2 の各 slice へ分解・一部統合済みである。
  一方で「どの契約がどの Owner・どの fixture で被覆され、
  何が未検証か」を横断的に固定した文書が存在しない。
- RCP 系（Render Recipe・Artifact/GC・Save/Commit/Share
  Trigger — #289〜#297）が OPEN のまま設計着手を控えて
  おり、`PROJ-RCP-01`（Save durability と Render Job の
  Receipt 分離・late Artifact attach での HEAD/Raw
  Asset/Undo 原子性）が要求する Project 側境界を先に
  固定する必要がある。
- #84（StagedFile 移行・PR #396 統合）で Snapshot/HEAD の
  publish/CAS/Recovery が main に揃ったため、Issue #17 の
  「Safe Save・Recovery」契約を as-built として照合できる
  時点にある。
- #10（Project Model 設計）は PR #398 で統合済みであり、
  本 slice はその成果物を前提にせず、永続化契約側から
  `docs/specs/project-model.md` への参照関係のみを記録
  する。

## Users and Value

- **#82/#84/#86・#92〜#97・RCP 系の実装者**: Project
  Snapshot/HEAD/Asset/Save/Recovery の契約がどの crate・
  port・fixture に実装済みで、どこが未実装かを Owner 別に
  把握でき、二重 Migrator/二重 Store/契約の再発明を防げる。
- **レビューア / Coordinator**: Issue #17 の受入条件
  13 項目それぞれに対し、根拠となる fixture・証拠台帳・
  NotRun 分類が一意に辿れる。
- **将来の Migration/VCS/Render/Recovery 拡張**: Live/
  Saved/Recovery/VCS/ActivePlan/Materialized Media の
  Revision 分離と Publish ≠ Project Commit の境界が
  文書として固定され、下流実装が意味論を誤読しない。

## Scope

### In Scope

- `docs/specs|designs|plans|tasks|test-specs|impl/ir|
  impl/exp/project-persistence*.md` の文書チェーン 7 件と
  `evidence/sv17/` の受入証拠台帳。
- Issue #17 の横断契約（Single Writer・Revision 分離・
  Immutable Snapshot+HEAD・Asset Identity/Verified Blob・
  Media Lifecycle・Save/Recovery・Source 不変性・Retention/
  GC・Schema Migration・VCS read-only・外部変更検出）と、
  既存 spec/design/実装済み fixture との照合表。
- Snapshot/HEAD stale writer・Concurrent Save・
  Crash/DiskFull・Restore/Retry・Import 失敗・
  CommitIdentity 再試行・Undo/VCS/Recovery 保護・Schema
  Migration・VCS read-only・外部編集競合の各契約について、
  既存 fixture の被覆状況の台帳化（構成済みテストがない
  契約は `NotRun`）。
- OS/FS Capability（File Sync/DirSync/Atomic
  Publish/Indeterminate/Recovery）の実環境別記録と、
  Mock/injected fault と実 FS/実 durability の証拠分離。
- 関連 docs index（`docs/*/README.md`）の `doc_maintenance`
  による再生成。

### Out of Scope

- `crates/` 以下のコード変更全般 — 実装は #82/#84/#86・
  #92〜#97・RCP 系等の各 Owner Issue が担当する。
- `Cargo.toml`/`Cargo.lock`/`mise.toml`/CI workflow の変更。
- Ruviform Generic File API/Atomic Publish/StagedFile の
  再実装・変更（Synvelis #82 境界 spec で Synvelis 責務外と
  確定済み）。
- FS Durable（file/dir sync 達成）と Application Commit
  （Project HEAD repoint・revision 確定）の混同 — 本 spec は
  両者を別 receipt/別保証として記録するのみ。
- 実 Device/FS Fault を Mock/injected fault の PASS のみで
  代用すること — 実 FS durability・物理電断・Network/Cloud
  FS は別 lane の証拠として分離する。
- RCP 系（#289〜#297）の Render Recipe/Artifact/GC/Policy/
  Branch/VCS 統合の設計・実装 — OPEN Issue として `defers`
  に記録し、本 slice では Project 側境界の参照のみ行う。
- #10 Project Model 本体の設計 — 別途実施中のため本 slice
  では永続化契約側からの参照関係のみ記録する。
- 実 GUI/実 OS/実 Plugin/実機の L2/L3 受入（POLICIES.md
  §2 の別 Owner/Deferred 区分）。

## Requirements

### 契約の正本化と照合

- ProjectController 単一 Writer・`ExpectedRevision` CAS・
  Live/Saved/Recovery/VCS/ActivePlan/Materialized Media の
  Revision 分離・Undo/Redo の新 Revision 前進を、実装済み
  契約（`vs-a` spec・`controller.rs`/`store.rs`）と照合して
  記録する。
- Immutable Snapshot（`r<N>.json`・CreateNew）+ HEAD
  （Replace・snapshot 成功後のみ）+ `FileAndDirectorySync` +
  `RequireAtomic` の publish 契約と、Publish 失敗分類
  （`NotPublished`/`Published`/`Indeterminate`）の正直な転写
  を、`syn-io-02` 系 spec/design と `store.rs` 実装に照合
  する。
- `save_cas` の `HEAD.lock` critical section・expected-head
  照合・stale-break・fencing・`HeadCasError` を、
  `syn-io-02-head-cas` spec/design と実装に照合する。
- **Publish ≠ Project Commit**: publish `Ok` は durability
  達成と name 可視性のみを意味し、HEAD repoint が commit
  点であること、DirSync 未達/Indeterminate で HEAD を勝手に
  進めないことを契約として固定する。
- Asset Lifecycle（Discovered→Staging→Verified→Registered・
  Missing/Degraded）・Logical ID/Content Digest/Provenance/
  Physical Location の分離・同一 byte 列の dedup と来歴別
  ID の両立を、ALIB-02 系 spec/実装に照合する。
- Media Materialization と Project Commit の分離
  （Acquire→Staging→Validate→Durable Materialization→
  Project Reference Commit→Job Finalize）・Orphan 保護・
  CommitIdentity 冪等（AlreadyApplied/Integrity
  Conflict/StaleRevision）を、VS-C/D・RCD-02・X 系
  spec/実装に照合する。
- Autosave/Recovery が Saved HEAD と別履歴であること・
  Recording Media Recovery と未保存 Project Recovery を
  別根拠として表示する契約を記録する。
- Retention/GC: Live/Saved/Recovery/VCS/Undo/Redo/Active
  Job/Pin の保護 Root・初期の非破壊的 GC（候補検出+明示
  Cleanup）を記録する。
- Schema Evolution/Migration: 旧 Immutable Snapshot 非破壊・
  Versioned Reader/Migration Registry 単一経路・未知必須
  Feature の明示 Reject・Plugin Opaque State 保全・過去
  VCS Revision の read-only projection を記録する。
- VCS read-only: Live/Saved/Recovery/VCS locator の
  `SnapshotIdentity` 解決・捕捉後の Live/Engine/Working
  Tree 非変更・Restore-as-Copy 既定を記録する。
- 外部変更検出: HEAD CAS Conflict・`external_head_rewrite`
  の正直な報告・協調 writer 限定の保証境界を記録し、
  非協調 writer/外部プロセス変更の検出限界を明示する。

### 受入証拠の台帳化

- Issue #17 の受入条件 13 項目を Case ID（`SV17-C01`〜）に
  写像し、各項目について根拠 fixture（crate::test 名）・
  Expected/Observed・実行環境・SHA・Pass/Fail/NotRun/
  Unsupported/Deferred を `evidence/sv17/case-ledger.md` に
  記録する。
- 構成済みテストがない契約は `NotRun` とし、fixture が
  存在しても本 slice で実走していない場合は「fixture 設定
  済み・本 slice 未実走」として区別する。先行 slice の
  実測証拠（`evidence/sv84/`・`evidence/reports/`）は SHA・
  日時付きで参照する。
- OS/FS Capability を Required/Deferred/NotApplicable と
  Pass/Fail/NotRun/Unsupported に分離して記録し、Mock FS・
  injected fault の PASS を実 FS durability の証明にしない。

### 文書・検証規約

- spec → design → plan → task → test-spec → impl(ir/exp) の
  文書チェーンを `doc-driven-dev-graph` フローで作成し、
  review-protocol で blocker が残らない状態を承認相当とする。
- docs index は `doc_maintenance.js index apply` で再生成し、
  手編集しない。
- 検証は `mise run fmt-check`（docs/設計スライスの正規検証）
  を実行し、結果を正直に記録する。実装 crate の検証は本
  slice の対象外とする。

## Acceptance Criteria

Issue #17 の受入条件を本 spec の完了条件として転記する
（2026-11-14 時点の Issue 本文が正本）。これら 13 項目は
Issue レベルの受入条件であり、実装は #82/#84/#86・#92〜
#97・RCP 系等の各 Owner Issue が担当する — 本 spec/docs
slice の責務は `evidence/sv17/case-ledger.md` で各項目の
fixture 被覆・先行実測・NotRun/Deferred 分類を記録する
ことであり、13 項目自体の Pass 判定は Owner Issue の
検証 lane に委譲する:

- [ ] `PROJ-RCP-01`: Save durabilityとRender Jobを別Receipt化し、
  DiskFull/Crash/late Artifact attachでもHEAD・Raw Asset・
  Undoの原子性が保護される。
- [ ] Live R123のままSaved R120を非同期保存でき、未保存変更を保持。
- [ ] 2つのSave Jobが逆順に完了しても新しいSaved HEADを旧Jobが覆さない。
- [ ] Snapshot/HEAD/Directory Syncの各時点のCrashを注入し有効なRootを検証。
- [ ] Import転送・検証・Project Commitの失敗で未検証Asset/Clipを公開しない。
- [ ] Content Blobの同一Byte列と論理Asset/Sourceの異なるID/来歴が両立する。
- [ ] Offline再生がRemote ServerやPreview Cacheに依存しない。
- [ ] Recording Commit後Save前CrashのMedia RecoveryとProject Recoveryを別の根拠として表示。
- [ ] CommitIdentityの再試行でSource/Take/Clipの二重登録を防止。
- [ ] Undo/VCS/Recoveryに保護されたMediaをGC対象としない。
- [ ] Schema Migration後も旧Snapshotを保持し、Plugin欠落状態でもOpaque Stateを保存。
- [ ] VCSの過去Revision閲覧で現行Project/Engine/Working Treeを変更しない。
- [ ] 外部編集による保存競合を検出し、破壊的上書きを回避する。

加えて本 slice 固有の完了条件:

- [ ] 上記 13 項目が `evidence/sv17/case-ledger.md` の Case ID
  に写像され、根拠 fixture・Expected/Observed・NotRun 分類が
  記録されている。
- [ ] ProjectController 単一 Writer/CAS・Immutable
  Snapshot・HEAD・Asset Identity/Verified Blob・Save/
  Recovery・Source 不変性の契約が #82/#84/#86 の各
  Owner・Fault Matrix に照合され、docs chain 7 文書が
  作成されている。
- [ ] Windows/macOS/Linux の File Sync/DirSync/Atomic
  Publish/Indeterminate/Recovery の実環境 Capability が
  別記され、Mock FS の合格を実 FS durability の証明に
  していない。
- [ ] `mise run fmt-check` が exit 0 で完了し、review-protocol
  で blocker が残っていない。

## Deferred Design Concerns

- **RCP 系の実装**（Render Recipe・Artifact Manifest/GC・
  Render Policy/Job・Branch/VCS 統合 — #290〜#297）:
  `PROJ-RCP-01` の Render Job Receipt 分離・late Artifact
  attach の実装は OPEN Issue の責務。本 slice は Project
  側境界（Saved HEAD 確定後に OnSave Render Job を起動し、
  Artifact 到着/検証・GC が Project CAS/Undo へ暗黙書込み
  しない契約）を記録するのみで、実装・実測は各 Issue の
  gate に委譲する。
- **実 FS durability・物理電断**: process kill 相当・
  crash window 模擬は fixture で pin 済みだが、真の電源断・
  OS 固有 Indeterminate/DirSync 失敗・Network/Cloud FS は
  実機 lane（`mise run test-recording-real-fs` 等）と
  Deferred 区分に委譲する。
- **非協調 writer・外部プロセス変更**: `HEAD.lock` は協調
  する `save_cas` caller のみを直列化する。`save` 直接呼出・
  lock を取らない書き込み・外部プロセスの HEAD 書換えは
  advisory lock の保証外であり、検出は次回 CAS conflict・
  `head()` 再読込の正直な報告に依存する（syn-io-02-head-cas
  design の保証境界を継承）。
- **Autosave/Recovery Root の物理配置・Backup/Save As・
  Retention 期間・GC 実行 UI**: PAP architecture §15 の
  PoC 保留事項。本 slice では契約上の保護 Root と非破壊
  原則のみを固定する。
- **実 GUI/実 OS/実 Plugin/実機 L2/L3**: POLICIES.md §2 の
  Windows 利用機能必須・macOS/Linux Deferred 区分に従い、
  別 Owner（#114 系等）へ委譲する。
- **#10 Project Model・#125 PPQN/CTL-01/02 Schema**: #10 は
  PR #398 で統合済み、PPQN/CTL 系も統合済みの隣接
  Issue。本 slice は永続化契約側からの参照関係のみ記録し、
  Schema/Model 本体の設計は各 Owner に委譲する。

## Sources and Relations

- 正本 Issue: [Synvelis #17](https://github.com/scarecrow173/synvelis/issues/17)
  （設計・ADR Issue、10 項目合意・PAP-01〜18・受入条件の正本）。
- 実行計画: [Synvelis #23](https://github.com/scarecrow173/synvelis/issues/23)
  （F0〜E2E の実装 Owner 割当て）。
- 設計正本: `docs/designs/project-asset-persistence-architecture.md`
  （Single Writer/Revision 分離/Save プロトコル/PAP-01〜18）。
- 実装 Owner: VS-A #13・VS-C #15・VS-D #16・SYN-IO-01 #82・
  SYN-IO-02 #84・ALIB-02 #86・VCS-01〜06 #92〜#97・RCD-02 #116・
  X #26・L1 #24・L2 #25・RCP #289〜#297。
- 境界 spec: `docs/specs/syn-io-01-ruviform-common-io-boundary.md`
  （Ruviform 共通 I/O 適合境界 — HEAD CAS は Synvelis 責務）。
- 先行証拠: `evidence/sv84/case-ledger.md`（StagedFile/CAS
  fixture 実測）、`evidence/reports/rcd-02-durability-
  recovery-review-ledger.md`（Recording fault matrix 実測）、
  `evidence/reports/vcs-06-fixture-matrix-ledger.md`（VCS
  fixture matrix 実測）。
