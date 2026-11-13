---
id: "DESIGN-CjYVivoek7ZbOPp7uvvYVo"
type: "design"
status: "approved"
title: "Project / Asset / Persistence — 設計契約マップ（synvelis#17）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/project-persistence.md"
  changes:
    added:
      - type: "file"
        path: "docs/designs/project-persistence-design.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/project-persistence.md"
  implemented-by: []
  depends-on:
    - "docs/specs/project-persistence.md"
    - "docs/designs/project-asset-persistence-architecture.md"
    - "docs/specs/syn-io-02-snapshot-head-staged-publish.md"
    - "docs/specs/syn-io-02-head-cas.md"
    - "docs/designs/syn-io-02-head-cas-design.md"
    - "docs/specs/stagedfile-migration.md"
    - "docs/specs/vs-a-project-recording-foundation-stable-id-command-save-reload.md"
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
    - "docs/specs/alib-02-streaming-verified-import.md"
    - "docs/specs/alib-02-v5-snapshot-compat.md"
    - "docs/specs/rcd-02-durability-evidence-recovery-review.md"
    - "docs/specs/vcs-02-semantic-diff-changeset.md"
    - "docs/specs/vcs-05-apply-restore-headless.md"
    - "docs/specs/x-recorded-verified-source-playback-export.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/project-persistence.md"
    - "docs/specs/project-model.md"
    - "docs/designs/project-model-design.md"
    - "docs/specs/vcs-01-immutable-snapshot-resolver.md"
    - "docs/plans/project-persistence.md"
    - "docs/test-specs/project-persistence.md"
    - "evidence/sv17/case-ledger.md"
  refines:
    - "docs/designs/project-asset-persistence-architecture.md"
  refined-by: []
  derives-from:
    - "docs/designs/project-asset-persistence-architecture.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "evidence/sv17/case-ledger.md"
    - "evidence/sv84/case-ledger.md"
    - "evidence/reports/rcd-02-durability-recovery-review-ledger.md"
    - "evidence/reports/vcs-06-fixture-matrix-ledger.md"
    - "evidence/reports/syn-io-02-os-l1-evidence.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/290"
    - "https://github.com/scarecrow173/synvelis/issues/291"
    - "https://github.com/scarecrow173/synvelis/issues/292"
    - "https://github.com/scarecrow173/synvelis/issues/293"
    - "https://github.com/scarecrow173/synvelis/issues/294"
    - "https://github.com/scarecrow173/synvelis/issues/295"
    - "https://github.com/scarecrow173/synvelis/issues/296"
    - "https://github.com/scarecrow173/synvelis/issues/297"
  deferred-by: []
---

# Project / Asset / Persistence — 設計契約マップ（synvelis#17）

## 概要

`docs/designs/project-asset-persistence-architecture.md`
（以下 PAP architecture）が Project/Asset/Persistence の
詳細設計正本である。本 design は PAP architecture と、
既に main へ統合された実装済み spec/design 群との間にある
**契約写像**を固定する — どの契約がどの文書・どの crate・
どの port・どの fixture に実装済みで、どこが未実装・未検証
かを一枚の表にし、Issue #17 受入条件 13 項目の根拠を
`evidence/sv17/case-ledger.md` の Case ID に結び付ける。

本 design は新しい意味論を発明しない。既存の合意済み契約を
重複記述せず、**正本への参照と被覆状況**を記録する文書で
ある。`refines` 関係で PAP architecture を精緻化するが、
PAP architecture 本文は変更しない。

## 契約マップ（Contract Map）

以下の表が本 slice の中核成果物である。各行は 1 つの横断
契約を、正本 spec/design・実装 Owner・被覆 fixture・残 gap
に写像する。「被覆」列の fixture 名は実際に存在する test
関数・test binary を指し、`evidence/sv17/case-ledger.md`
の Case ID と対応する。

### A. Authoritative State・Revision 分離

| 契約 | 正本 | 実装 Owner | 主な fixture 被覆 | 残 gap |
| --- | --- | --- | --- | --- |
| ProjectController 単一 Writer・全 Project 意味論 mutation は `controller.command/undo/redo` 経由 | PAP arch §4・`vs-a` spec | `crates/synvelis-project`（`controller.rs`） | `controller_store.rs`（head()・save()・save_cas・undo/redo 分離） | なし（as-built 一致） |
| Live/Saved/Recovery/VCS/ActivePlan/Materialized Media の Revision 分離 | PAP arch §4.3・`vcs-01` spec | `synvelis-project` + `synvelis-gateway`（journal restore は `save_cas` 使用） | `vcs01_snapshot.rs`（live/saved/recovery/vcs locator 分離・`resolution_never_mutates_*`） | Active Execution Plan revision は #23/RCP 系で将来接続 — NotRun |
| `ExpectedRevision` CAS（optimistic concurrency・conflict は `StaleRevision`/`Conflict` で明示） | `vs-a`・`alib-02` spec・`syn-io-02-head-cas` spec | `synvelis-project` `apply()`/`save_cas` | `import.rs`（stale_revision）・`head_cas.rs`（conflict・external rewrite） | なし |
| Undo/Redo は新 Revision 前進・Saved HEAD 不変・Redo stack 非破壊 | PAP arch §4.5・`vs-d` spec | `synvelis-project` `controller.rs` | `vs_d.rs`（undo/redo・save_reload）・`project_adapter.rs` | なし |

### B. Immutable Snapshot + HEAD + StagedFile Publish

| 契約 | 正本 | 実装 Owner | 主な fixture 被覆 | 残 gap |
| --- | --- | --- | --- | --- |
| Snapshot = `r<N>.json` immutable（CreateNew publish・衝突は `snapshot_collision` で分類） | `syn-io-02-snapshot-head-staged-publish` spec・`stagedfile-migration` spec | `crates/synvelis-project/store.rs`（`DirectoryBundleStore`） | `store_publish.rs`（collision・orphan・stale lock）・`head_cas.rs` | なし |
| HEAD = `HEAD` pointer file・Replace publish・**snapshot 成功後のみ** repoint | 同上 + `syn-io-02` design | `store.rs` `publish_staged` | `store_publish.rs`（ordering・crash window）・`sv84` case-ledger C01〜C13 | なし |
| `Durability::FileAndDirectorySync` + `PublishMode` 明示（snapshot=CreateNew・HEAD=Replace） | `stagedfile-migration` spec・Ruviform `ruviform-io-file` 契約 | Ruviform（`ruviform-io-file`）+ Synvelis 呼出側 | `sv84` case-ledger 実測（Windows NTFS）・`syn-io-02-os-l1-evidence.md` | macOS/Linux 実機 = NotRun（§OS/FS Capability 参照） |
| **Publish ≠ Project Commit**: publish `Ok` は durability+可視性のみ。HEAD repoint が commit 点。DirSync 未達/Indeterminate で HEAD を勝手に進めない | `syn-io-02` design・PAP arch §5 | `store.rs` + `ProjectController` | `store_publish.rs`・`head_cas.rs`（fencing・indeterminate） | なし |
| Publish 失敗分類の正直な転写（`NotPublished`/`Published`/`Indeterminate`・effect/phase/durability・recovery handle 非自動削除） | `stagedfile-migration` spec | `store.rs` `PublishFailure` → `Error::Store` | `sv84` case-ledger・`staged_publish.rs`（ruviform 側 seam） | なし |
| Crash window（snapshot write→HEAD repoint→dir sync の各時点）で有効な Root を維持 | `syn-io-02` spec・PAP arch §5.4 | `store.rs` + Ruviform `StagedFile` recovery handle | `store_publish.rs`（crash 注入）・`head_cas.rs`（lock 喪失 fencing） | 物理電断 = NotRun（injected/process-kill 相当のみ） |
| HEAD CAS: `HEAD.lock`（CreateNew+BestEffort・stale 20s break・busy 30s）内で expected-head 照合・pre/post fencing | `syn-io-02-head-cas` spec/design | `store.rs` `save_cas` + `synvelis-project` `controller.rs` | `head_cas.rs`（conflict・stale writer・external rewrite・fencing） | 非協調 writer 保証外（後述） |
| `save_cas` additive・`Unsupported` fallback・`HeadCasError::{Conflict,Unsupported,Store}` | `syn-io-02-head-cas` spec | `ProjectStore` trait + `DirectoryBundleStore`/`InMemoryStore` | `controller_store.rs`・`head_cas.rs` | なし |

### C. Asset Lifecycle・Identity・Verified Blob

| 契約 | 正本 | 実装 Owner | 主な fixture 被覆 | 残 gap |
| --- | --- | --- | --- | --- |
| Asset Lifecycle: Discovered→Staging→Verified→Registered・Missing/Degraded 分類 | `alib-02-streaming-verified-import` spec・PAP arch §7 | `crates/synvelis-library`（`import.rs`）+ `synvelis-project` | `import.rs`（全段階）・`import_recovery.rs` | なし |
| Import pipeline: transfer→verify→project commit の失敗で未検証 Asset/Clip を公開しない（staged→atomic publish→controller commit の順序） | `alib-02` spec | `synvelis-library` `import.rs` + `synvelis-project` | `import.rs`（digest/size mismatch・stale revision・cancellation）・`import_recovery.rs`（orphan・save_failure_preserves_commit） | なし |
| Content identity vs logical identity: 同一 byte 列の dedup（Content Digest 共有）と論理 Asset/Source ID・Provenance の分離・両立 | `alib-02` spec・PAP arch §7.3 | `synvelis-library` + `synvelis-project` | `import.rs`（`same_bytes_across_providers_share_one_verified_blob`・crc_collision） | なし |
| Streaming verified import: bounded memory・SHA-256・staged→readback→atomic publish | `alib-02` spec | `synvelis-library` | `import.rs`（bounded-memory streaming・digest verify） | なし |
| ImportId 冪等・`AlreadyApplied`/`IntegrityConflict`/`StaleRevision` 分類・rebase | `alib-02` spec | `synvelis-library` | `import.rs`（replay idempotency・content conflict・stale revision） | なし |
| Offline playback は project blob/media root のみ依存・Remote/Preview Cache 非依存 | `x-recorded-verified-source-playback-export`・`l1` spec | `synvelis-render`/`synvelis-player` 系 | `x_e2e.rs`・`l1_e2e.rs`/`l2_e2e.rs` fixture | Remote provider 再取得経路 = Deferred（PAP arch） |

### D. Media Materialization・Project Commit 分離・Recovery

| 契約 | 正本 | 実装 Owner | 主な fixture 被覆 | 残 gap |
| --- | --- | --- | --- | --- |
| Media materialization ≠ Project commit: Acquire→Staging→Validate→Durable Materialization→Project Reference Commit→Job Finalize | PAP arch §8・`vs-d`・`x` spec | `synvelis-recording`/`synvelis-render` + `synvelis-project` | `vs_d.rs`・`x_e2e.rs`・`l2` export fixture | Render Job 完了 receipt = RCP 系（NotRun） |
| Recording CommitIdentity 冪等: `AlreadyApplied` で Source/Take/Clip 二重登録防止・`CommitRecordingCommand` + `RecordingCommitIdentity` | `vs-d` spec | `synvelis-recording` + `synvelis-gateway`（commit_gate keyed retry） | `vs_d.rs`・`recovery_review.rs`・`project_adapter.rs`（applied_commits registry） | なし |
| Media Recovery と Project Recovery の分離: Recording commit 後 save 前 crash で media 側（journal/StagingArea verified prefix）と project 側（controller/store）を別根拠で表示 | `vs-d` spec・`rcd-02` spec | `synvelis-recording` `recovery_review` + `synvelis-project` | `recovery_review.rs`・`project_adapter.rs`（`MediaReferenceProbe`・receipt `project_saved` 分離） | なし |
| Recovery Review: Adopt=verified prefix のみ finalize・Hold=無変更判定永続化・Discard=fail-closed（`MediaReferenceProbe::Clear` のみ削除） | `rcd-02` spec | `synvelis-recording` | `recovery_review.rs`（adopt/hold/discard・unreadable fail-closed） | なし |
| Commit receipt 分類: Empty/Reserved/CommittedConfirmed/PublishIndeterminate/Conflict/Invalid・`project_saved` は saved revision 比較 | `rcd-02` spec | `synvelis-recording` + `synvelis-project` | `recovery_review.rs`・`project_adapter.rs` | なし |
| Autosave/Recovery は Saved HEAD と別履歴・Recovery Root 保護 | PAP arch §9・`vcs-01` spec | `synvelis-project`（recovery checkpoint） | `vcs01_snapshot.rs`（`recovery_checkpoint_is_isolated_from_saved_head`） | Autosave Root 物理配置 = Deferred（PAP arch §15） |

### E. Retention/GC・Schema Migration・VCS read-only・外部変更

| 契約 | 正本 | 実装 Owner | 主な fixture 被覆 | 残 gap |
| --- | --- | --- | --- | --- |
| Retention/GC: Live/Saved/Recovery/VCS/Undo/Redo/Active Job/Pin を保護 Root・初期は非破壊的 GC（候補検出+明示 Cleanup）・orphan snapshot/media の誤削除防止 | PAP arch §10・`alib-02`・`vcs-01` spec | `synvelis-project` + `synvelis-library` | `project_adapter.rs`（probe 系）・`import_recovery.rs`（orphan GC 保護）・`store_publish.rs`（orphan snapshot） | Active Job/Pin Root の完全実装 = #92〜#97 系で段階的 |
| Schema Evolution: 旧 Immutable Snapshot 非破壊・Versioned Reader/Migration Registry 単一経路・未知必須 Feature 明示 Reject・Plugin Opaque State 保全 | `alib-02-v5-snapshot-compat` spec・PAP arch §11 | `synvelis-project`（schema version・migration） | `controller_store.rs`（unknown schema reject）・`vcs01_snapshot.rs`（v5 projection read-only）・`alib02_v5_real` fixture | Migration UI/手動介入 = Deferred |
| VCS read-only: `SnapshotIdentity` 解決・捕捉後 Live/Engine/Working Tree 非変更・Restore-as-Copy 既定・VCS locator（staged/committed）の generation/index drift → `IndexChanged` | `vcs-01`・`vcs-02`・`vcs-05` spec | `synvelis-project` + `synvelis-vcs`（real-git） | `vcs01_snapshot.rs`・`vcs02_diff.rs`・`vcs06_*`・`vcs-06-fixture-matrix-ledger.md` | GUI 履歴閲覧 = #114 系（NotRun） |
| 外部変更検出: `save_cas` expected-head 照合で HEAD 外部書換え → `Conflict`・`external_head_rewrite` 正直報告・協調 writer 限定の保証境界 | `syn-io-02-head-cas` spec/design | `store.rs` `save_cas` + `controller.rs` | `head_cas.rs`（external rewrite・conflict・stale writer） | 非協調 writer・外部プロセス = 検出限界を明示（下記） |
| Safe Save 非同期: Live R123 のまま Saved R120 を非同期保存・未保存変更保持・2 Save Job 逆順完了でも新しい Saved HEAD を旧 Job が覆さない | PAP arch §6・`syn-io-02-head-cas` | `ProjectController` + gateway journal | `head_cas.rs`（conflict fencing）・`controller_store.rs` | Save Job scheduler 本体 = #92 系（contract のみ固定） |

## 設計上の決定と境界

### D1. 本 slice は契約写像であり新規意味論を発明しない

PAP architecture が詳細設計正本、`syn-io-02`/`vs-*`/`alib-02`/
`vcs-*`/`rcd-02`/`x`/`l1`/`l2` 各 spec が実装済み slice の正本
である。本 design はそれらを **参照・写像・被覆監査** する
のみで、契約内容を再定義しない。新しい port・型・crate を
導入しない。

### D2. Ruviform/Synvelis 責務境界（再確認）

- **Ruviform `ruviform-io-file`**: StagedFile/SealedFile・
  atomic publish・dir sync・failure classification
  （`PublishFailure` effect/phase/durability）・recovery
  handle。generic single-file staged publishing のみ。
- **Synvelis `synvelis-project`**: SnapshotEnvelope・
  immutable snapshot 名・HEAD pointer・expected-head CAS・
  `HEAD.lock` fencing・Saved/Live revision 意味論・
  Undo/Autosave・recovery adoption。Project 意味論は
  Synvelis が所有する。
- **`Publish success ≠ Project Commit`**: publish `Ok` は
  durability 達成と name 可視性のみを意味する。HEAD repoint
  が application commit 点であり、DirSync 未達や
  `Indeterminate` で HEAD を自動前進させない。
- 本 slice は Ruviform Generic File API/Atomic Publish を
  重複実装しない。Ruviform 側の変更は Synvelis #82 境界
  spec の責務外。

### D3. FS Durable と Application Commit の分離

- **FS Durable**: file sync + dir sync が OS に受理された
  こと（`Durability::FileAndDirectorySync` 達成）。
- **Application Commit**: `HEAD` pointer が新 snapshot を
  指し、`saved.head` が更新され、revision が確定したこと。
- 両者は別 receipt・別保証であり、片方の成功を他方の証明に
  しない。`PublishIndeterminate`・dir-sync `Unsupported`・
  `Indeterminate` 分類は正直に記録し、durable 宣言はそれ
  以下の保証に留める（`rcd-02` ledger の
  `DirSyncState::Unsupported` honesty と同型）。

### D4. 非協調 writer の保証境界

`HEAD.lock` + expected-head CAS が直列化するのは
`save_cas` を呼ぶ協調 writer のみである。`save()` 直接
呼出・lock を取らない書き込み・外部プロセス（エディタ・
別アプリ・手動編集）による HEAD/snapshot 変更は advisory
lock の保証外である。その場合の検出は次回 `save_cas` の
expected-head 照合で `Conflict` となり、`head()` 再読込で
現状を正直に報告する — 破壊的上書きは CAS が防ぐが、
「外部変更を事前に全て検出する」保証は持たない。この限界
は `syn-io-02-head-cas` design の保証境界を継承し、本
slice で拡張しない。

### D5. `PROJ-RCP-01` の Project 側境界（Render Job Receipt 分離）

Issue #17 受入 `PROJ-RCP-01` は「Save durability と Render
Job を別 Receipt 化し、DiskFull/Crash/late Artifact attach
でも HEAD・Raw Asset・Undo の原子性が保護される」を要求
する。RCP 系（#290〜#297）が OPEN のため本 slice では
**Project 側契約のみ**を固定する:

- Saved HEAD 確定（Application Commit）は Render Job の
  完了を待たない。Render Job は Saved HEAD 確定後に
  OnSave trigger で起動する非同期 receipt である。
- Artifact 到着・検証・attach は Project CAS/Undo/Redo/
  HEAD を暗黙に書き換えない。Artifact 参照の Project への
  反映は別の明示的 command/revision 前進を経る。
- DiskFull/Crash/late attach が HEAD・Raw Asset・Undo の
  原子性を破らないことは、既存の publish/CAS/receipt 分類
  fixture（`store_publish.rs`・`head_cas.rs`・
  `recovery_review.rs`）で部分的に被覆されるが、
  **Artifact attach 固有の統合 fixture は RCP 系で新設
  されるまで NotRun** とする。

### D6. 同期 Save と非同期 Save Job の区別

「Live R123 のまま Saved R120 を非同期保存」は
`ProjectController` が live revision を進めずに saved
revision のみを target にする contract であり、既存の
`save()`/`save_cas()`（saved HEAD を expected に供給）で
表現される。2 つの Save Job が逆順に完了しても新しい
Saved HEAD を旧 Job が覆さないことは `save_cas` の
expected-head 照合で保証される — 先に完了した新しい Job
が HEAD を進めた後、古い Job の expected-head は stale
となり `Conflict` で拒否される。Save Job scheduler の
実装（queue・cancel・UI receipt）は #92 系の責務であり、
本 slice は CAS 契約レベルの保証のみを固定する。

## OS/FS Capability マトリクス

POLICIES.md §2/§3 に従い、実環境別の Capability を分離
記録する。`FileAndDirectorySync` は publish の要求 durability
として全 OS で要求されるが、**実測証拠は Windows/NTFS の
み**であり、macOS/Linux は fixture 設定済みでも実機実行が
NotRun である。

| Capability | Windows (NTFS) | macOS | Linux | 備考 |
| --- | --- | --- | --- | --- |
| Atomic Publish（同一 dir rename） | Required・Pass（`sv84` 実測） | Required・NotRun | Required・NotRun | Ruviform `StagedFile` が OS primitive を抽象化 |
| File Sync（`sync_data`） | Required・Pass | Required・NotRun | Required・NotRun | `Durability::FileAndDirectorySync` の file 側 |
| Dir Sync（directory entry 永続化） | Required・Pass（一部 `Unsupported` を正直分類） | Required・NotRun | Required・NotRun | `DirSyncState::Unsupported` は durable 宣言以下の保証 |
| Indeterminate 分類（publish 応答喪失） | Required・Pass（`PublishInject::Indeterminate` fixture） | Required・NotRun | Required・NotRun | 実 OS の indeterminate は process-kill 相当のみ |
| Crash window（snapshot/HEAD/dirsync 各時点） | Required・Pass（injected/process-kill 相当） | Required・NotRun | Required・NotRun | 物理電断・真の power-loss は全 OS で NotRun |
| HEAD CAS / `HEAD.lock` fencing | Required・Pass | Required・NotRun | Required・NotRun | advisory lock・協調 writer 限定 |
| Recovery handle（stale temp/orphan） | Required・Pass | Required・NotRun | Required・NotRun | `StagedFile` recovery・`store_publish.rs` orphan |
| Real FS durability（物理電断・OS crash） | NotRun | NotRun | NotRun | 実機 lane（`mise run test-recording-real-fs` 等）へ委譲 |
| Network/Cloud FS | NotRun | NotRun | NotRun | PAP arch で Deferred |

## 検証方針

- 本 slice の正規検証は `mise run fmt-check`（docs/設計
  スライス）。実装 crate の `check`/`test` は対象外。
- 受入証拠は `evidence/sv17/case-ledger.md` に Case ID・
  fixture 名・Expected/Observed・環境・SHA・
  Pass/Fail/NotRun/Unsupported/Deferred で記録する。
- 先行 slice の実測（`sv84`・`rcd-02`・`vcs-06`・
  `syn-io-02-os-l1`）は SHA・日時付きで参照し、本 slice で
  再実行したかのように見せない。
- Mock FS・injected fault・`PublishInject` seam の PASS を
  実 FS durability・物理電断の証明にしない。
- review-protocol で blocker が残らないことを承認相当とし、
  PR 本文にレビュー実施回数と blocker 非残存を記録する。

## Alternatives Considered

- **PAP architecture を直接更新して契約を追記する**:
  却下。PAP architecture は詳細設計正本として既に大規模で
  あり、Issue #17 の「横断契約をどこに実装済みか」の写像は
  別文書（本 design + case-ledger）が適切。PAP architecture
  の本文を変更すると既存の `refines`/`derives-from` 関係を
  持つ多数の文書の前提が揺らぐ。
- **新規 port/trait を導入して契約をコード化する**:
  却下。本 slice は docs/evidence のみ。既存の
  `ProjectStore`/`ProjectController`/`CommitIdentity`/
  `MediaReferenceProbe` 等の port が契約を既にコード化して
  おり、重複する抽象化は YAGNI/DRY に反する。
- **Ruviform に CAS/HEAD 意味論を移譲する**:
  却下。`syn-io-01` 境界 spec で HEAD CAS は Synvelis 責務と
  確定済み。Ruviform は generic single-file publish のみを
  提供し、Project revision 意味論を持たない。
- **全 fixture を本 slice で再実行して fresh evidence を
  採る**: 却下。実装 crate の検証は本 slice の対象外
  （タスク指定の正規検証は `mise run fmt-check`）。先行
  slice の実測を SHA 付きで参照し、本 slice で未実行の
  fixture は「fixture 設定済み・本 slice 未実走」として
  正直に区別する。

## Sources and Relations

- `docs/specs/project-persistence.md`（本 design が実装する
  spec）。
- `docs/designs/project-asset-persistence-architecture.md`
  （詳細設計正本・`refines` 対象）。
- `docs/specs/syn-io-02-snapshot-head-staged-publish.md`・
  `docs/specs/syn-io-02-head-cas.md`・
  `docs/designs/syn-io-02-head-cas-design.md`（Snapshot/
  HEAD/CAS の実装済み正本）。
- `docs/specs/stagedfile-migration.md` +
  `evidence/sv84/case-ledger.md`（StagedFile 移行の実測）。
- `docs/specs/vs-a-*`・`vs-d-*`・`alib-02-*`・`vcs-01/02/05`・
  `rcd-02-*`・`x-*`・`l1-*`・`l2-*`（各 slice の実装済み
  spec/design）。
- `evidence/sv17/case-ledger.md`（本 design の被覆監査結果）。
