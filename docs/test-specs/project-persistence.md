---
id: "TSPEC-034X6P6mNcoFrZZ25JuxWA"
type: "test-spec"
status: "approved"
title: "Project / Asset / Persistence — 受入証拠テスト仕様（synvelis#17）"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/17"
  changes:
    added:
      - type: "file"
        path: "docs/test-specs/project-persistence.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/project-persistence.md"
    - "docs/designs/project-persistence-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/stagedfile-migration.md"
    - "docs/test-specs/syn-io-02-store-publish-contract.md"
    - "docs/test-specs/syn-io-02-head-cas.md"
    - "docs/test-specs/syn-io-02-head-cas-e2e.md"
    - "docs/test-specs/vs-d-e2e-r3-recording-commit.md"
    - "docs/test-specs/alib-02-streaming-verified-import.md"
    - "docs/test-specs/alib-02-v5-snapshot-compat.md"
    - "docs/test-specs/rcd-02-durability-recovery-review-fault-matrix.md"
    - "docs/test-specs/vcs-01-snapshot-resolver-fixtures.md"
    - "docs/test-specs/x-e2e-recorded-source-playback-export.md"
    - "docs/test-specs/l1-e2e-local-wav-verified-playback.md"
    - "docs/test-specs/l2-master-sequential-offline-wav-export.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/project-persistence-design.md"
    - "docs/specs/project-persistence.md"
  derived-by: []
  verifies:
    - "docs/specs/project-persistence.md"
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
    - "evidence/sv17/case-ledger.md"
  deferred-by: []
---

# Project / Asset / Persistence — 受入証拠テスト仕様

## Purpose

Issue #17 の受入条件 13 項目（`PROJ-RCP-01` + PAP 系
12 項目）が、どの fixture・どの証拠区分で検証されるかを
固定する。本 spec は **証拠の分類規則と被覆の写像**を
定める — 個別の store/import/recording 振る舞い契約の正本は
既存の `docs/test-specs/{stagedfile-migration,syn-io-02-*,
vs-d-*,alib-02-*,rcd-02-*,vcs-*,x-*,l1-*,l2-*}` であり、
本 spec はそれらを Issue #17 の受入軸へ写像し、
`evidence/sv17/case-ledger.md` の記録形式と Pass/NotRun
判定規則を定める。

実装が進み fixture が変わっても、本 spec の保証が失われ
ない限り fixture の再編は許容する。OS/CI/実機の証拠は
volatile であり本 spec の `status` には記録しない — 結果は
`evidence/sv17/case-ledger.md` と impl 記録に残す。

## Feature

Project/Asset/Persistence の横断契約 — Immutable
Snapshot+HEAD publish・Safe Save（CAS/非同期/逆順完了）・
Crash/DiskFull 耐性・Import 失敗の原子性・Asset Identity/
Verified Blob・Offline Playback・Media/Project Recovery
分離・CommitIdentity 冪等・GC 保護・Schema Migration・
VCS read-only・外部変更検出 — が、headless fixture と
実機 lane に分離された証拠として記録されること。

## Rules

- Rule 1 — **Save durability / Render Job Receipt 分離**
  （`PROJ-RCP-01`）: Saved HEAD 確定（Application Commit）
  は Render Job 完了を待たない。DiskFull/Crash/late
  Artifact attach で HEAD・Raw Asset・Undo の原子性が
  保護される。既存 fixture（`store_publish.rs`・
  `head_cas.rs`・`recovery_review.rs`）で publish/CAS/
  receipt 分類は被覆されるが、Artifact attach 固有の
  統合 fixture は RCP 系（#290〜#297）で新設されるまで
  NotRun。
- Rule 2 — **Live/Saved Revision 分離・非同期 Save**:
  Live R123 のまま Saved R120 を保存でき、未保存変更を
  保持する。`save()`/`save_cas()` が saved HEAD を
  expected に供給し、live revision を進めない。
- Rule 3 — **Concurrent Save single winner・逆順完了保護**:
  同 expected-head の協調 writer は exactly one winner。
  2 Save Job が逆順に完了しても、先に完了した新しい Job
  が進めた HEAD に対し古い Job の expected-head は stale
  となり `Conflict` で拒否される。
- Rule 4 — **Crash window の有効 Root**: snapshot publish→
  HEAD repoint→dir sync の各時点の crash を注入し、
  `head()`/`read_snapshot()`/再 Open が有効な Root を
  正直に報告する。真の process kill/電源断は fixture 範囲外
  （injected/process-kill 相当のみ・物理電断は NotRun）。
- Rule 5 — **Import 失敗の原子性**: transfer→verify→
  project commit の各失敗で未検証 Asset/Clip を公開しない。
  staged→atomic publish→controller commit の順序が保たれ、
  `StaleRevision`/`IntegrityConflict`/`NotPublished`/
  `Indeterminate` が正直に分類される。
- Rule 6 — **Content identity vs logical identity**: 同一
  byte 列の Content Digest 共有（dedup）と論理
  Asset/Source ID・Provenance の分離が両立する。CRC 衝突・
  provider 跨ぎの同一 byte 列でも論理 ID は独立。
- Rule 7 — **Offline playback 独立性**: 再生が project
  blob/media root のみ依存し、Remote Server・Preview Cache
  に依存しない。Remote provider 再取得経路は Deferred。
- Rule 8 — **Media Recovery vs Project Recovery 分離**:
  Recording commit 後 save 前 crash で media 側（journal/
  StagingArea verified prefix）と project 側（controller/
  store）を別根拠・別 receipt で表示する。
- Rule 9 — **CommitIdentity 冪等**: `CommitRecordingCommand`
  + `RecordingCommitIdentity` の再試行で
  `AlreadyApplied` となり Source/Take/Clip の二重登録を
  防ぐ。applied-commit registry + commit-manifest +
  commit.json marker の三重冪等。
- Rule 10 — **GC 保護**: Undo/VCS/Recovery/Active Job/Pin
  に保護された Media/Asset を GC 対象にしない。orphan
  snapshot/media の誤削除を防ぎ、保護 Root の到達可能性を
  `MediaReferenceProbe`/`gc_orphans` が尊重する。
- Rule 11 — **Schema Migration・Opaque State**: 旧 Immutable
  Snapshot を非破壊で保持し、Versioned Reader/Migration
  Registry 単一経路で読み、未知必須 schema を明示 Reject、
  Plugin 欠落時も Opaque State を保存する。v5 projection は
  read-only。
- Rule 12 — **VCS read-only**: 過去 Revision 閲覧で現行
  Project/Engine/Working Tree を変更しない。`SnapshotIdentity`
  解決は捕捉後に Live/Engine/Working Tree を mutate せず、
  generation/index drift は `IndexChanged` で明示、
  Restore-as-Copy 既定。
- Rule 13 — **外部変更検出・破壊的上書き回避**: `save_cas`
  の expected-head 照合で外部 HEAD 書換えを `Conflict` で
  検出し、破壊的上書きを防ぐ。非協調 writer・外部
  プロセスの保証限界は `syn-io-02-head-cas` design の
  境界を継承し、本 spec で拡張しない。
- Rule 14 — **証拠区分の正直さ**: Required/Deferred/
  NotApplicable（要求状態）と Pass/Fail/NotRun/Unsupported
  （実行結果）を分離し、Mock FS・injected fault・
  `PublishInject` seam の PASS を実 FS durability・物理
  電断・実機 GUI/Device/Plugin の証明にしない。OS/FS
  Capability を実環境別に記録する。

## Examples

各 Rule の根拠 fixture は `evidence/sv17/case-ledger.md` に
Case ID・crate::test 名・Expected/Observed・環境・SHA・
分類で記録する。代表例:

- Rule 2/3: `crates/synvelis-project/tests/head_cas.rs`
  （conflict・stale writer・external rewrite・fencing）・
  `controller_store.rs`（save/save_cas・head()）。
- Rule 4: `store_publish.rs`（collision・orphan・stale
  lock・crash window）・`sv84` case-ledger C01〜C13。
- Rule 5: `crates/synvelis-library/tests/import.rs`・
  `import_recovery.rs`（digest/size mismatch・stale
  revision・cancellation・orphan GC・save_failure_
  preserves_commit）。
- Rule 6: `import.rs`（`same_bytes_across_providers_share_
  one_verified_blob`・crc_collision）。
- Rule 7: `crates/synvelis-render/tests/x_e2e.rs`・
  `crates/synvelis-render/tests/l2_e2e.rs`・
  `crates/synvelis-player/tests/l1_e2e.rs`（playback/export
  が project blob/media root のみ依存）。
- Rule 8/9: `crates/synvelis-recording/tests/
  recovery_review.rs`・`project_adapter.rs`・
  `vs_d.rs`（adopt/hold/discard・applied_commits registry・
  `project_saved` receipt 分離）。
- Rule 10: `project_adapter.rs`（probe 系）・
  `import_recovery.rs`（orphan GC 保護）・
  `store_publish.rs`（orphan snapshot）。
- Rule 11: `controller_store.rs`（unknown schema reject）・
  `vcs01_snapshot.rs`（v5 projection read-only）・
  `alib02_v5_real` fixture。
- Rule 12: `vcs01_snapshot.rs`・`vcs02_diff.rs`・
  `vcs06_*`・`vcs-06-fixture-matrix-ledger.md`。
- Rule 13: `head_cas.rs`（external rewrite・conflict・
  stale writer）。

## Evidence Classification

- **Pass**: 本 slice または先行 slice で実測され、対象
  SHA・OS・環境・fixture が記録されている。
- **Pass***: persisted/headless 層のみの pass（実 FS
  durability・実機 GUI/Device/Plugin を含まない）。
- **NotRun**: fixture が存在するが本 slice で実行していない
  （先行実測を SHA 付きで参照する場合はその旨を明記）、
  または構成済みテストがない契約。
- **Unsupported**: capability が環境上利用不可（例:
  Windows の dir sync `Unsupported` を正直分類）。
- **Deferred**: POLICIES.md §2 の別 Owner/将来 lane へ
  委譲（実 GUI/実 OS/実 Plugin/実機・物理電断・
  Network/Cloud FS・RCP 系 Artifact attach）。

Mock FS・injected fault・`PublishInject`/`Inject` seam の
PASS は fault *classification* の証拠であり、実 FS
durability・物理電断の証明ではない（`stagedfile-migration`
test-spec の制約を継承）。

## Sources and Relations

- `docs/specs/project-persistence.md`（検証対象 spec）。
- `docs/designs/project-persistence-design.md`（契約写像・
  OS/FS Capability マトリクス）。
- `evidence/sv17/case-ledger.md`（本 spec の記録先）。
- 先行 test-spec/ledger: `stagedfile-migration`・
  `syn-io-02-*`・`vs-d-*`・`alib-02-*`・`rcd-02-*`・
  `vcs-*`・`x-*`・`l1-*`・`l2-*`。
