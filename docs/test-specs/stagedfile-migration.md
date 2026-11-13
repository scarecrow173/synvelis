---
id: "TSPEC-034X5DZHArceEXtnYl3YuG"
type: "test-spec"
status: "approved"
title: "Issue #84 StagedFile Migration 受入検証契約"
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
    - "docs/test-specs/syn-io-02-store-publish-contract.md"
    - "docs/test-specs/syn-io-02-head-cas.md"
    - "docs/test-specs/syn-io-02-head-cas-e2e.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/stagedfile-migration-design.md"
    - "docs/specs/stagedfile-migration.md"
  derived-by: []
  verifies:
    - "docs/specs/stagedfile-migration.md"
  verified-by: []
  references:
    - "evidence/sv84/case-ledger.md"
    - "evidence/reports/syn-io-02-os-l1-evidence.md"
  defers:
    - "evidence/sv84/case-ledger.md"
  deferred-by: []
---

# Issue #84 StagedFile Migration 受入検証契約

## Purpose

Issue #84「DirectoryBundleStore・Snapshot/HEAD 保存を Ruviform
StagedFile へ移行」の受入を、headless fixture の振る舞い契約と
OS/FS/実機別の証拠区分に分解して固定する。詳細な store 振る舞い
契約の正本は `docs/test-specs/syn-io-02-store-publish-contract.md`・
`docs/test-specs/syn-io-02-head-cas.md`・
`docs/test-specs/syn-io-02-head-cas-e2e.md` であり、本 spec は
それらを Issue #84 の受入条件（書戻し・二重 Publish・旧 Writer
上書き・Orphan 禁止・Crash Simulation・Recovery・OS 証拠区分）へ
写像し、証拠の記録先（`evidence/sv84/`）と Pass/NotRun の
判定規則を定める。

実装が進み store/fixture が変わっても、本 spec の保証が
失われない限り fixture の再編は許容する。OS/CI/実機の証拠は
volatile であり本 spec の `status` には記録しない — 結果は
`evidence/sv84/case-ledger.md` と impl 記録に残す。

## Feature

DirectoryBundleStore の Snapshot/HEAD 永続化が Ruviform
StagedFile 公開契約上で、Project Commit の意味論
（immutable snapshot・HEAD ordering・CAS・Recovery の正直さ）
を維持すること、およびその検証が OS/FS/実機別に
Required/Deferred/NotRun で分類記録されること。

## Rules

- Rule 1 — **Immutable snapshot・書戻し禁止**: 同一 revision
  の `save` 再実行で内容が異なる場合は失敗し、既存 snapshot
  byte と HEAD が不変。byte 完全一致・`id_watermark` 単調増の
  同一論理 snapshot のみ受理し、受理は勝者 byte の完全
  publish 再実行で耐久性を再確立する（存在のみを証拠に
  しない）。
- Rule 2 — **HEAD ordering・orphan 許容**: HEAD は snapshot
  publish 成功後にのみ進む。HEAD publish の `NotPublished`
  失敗は snapshot orphan + 旧 HEAD 保持を許すが、成功・
  無条件前進を偽装しない。
- Rule 3 — **CAS/ExpectedRevision**: `save_cas` は
  `HEAD.lock` critical section 内で expected-head を照合し、
  不一致は publish 前の typed `Conflict`（orphan 非生成）。
  lock 喪失時は `Ok` を返さない。
- Rule 4 — **Concurrent Save single winner**: 同 expected-head
  の協調 writer（thread・process を問わず）は exactly one
  winner。敗者は typed `Conflict` で observed HEAD を受け取る。
- Rule 5 — **旧 Writer 上書き禁止**: stale baseline の
  controller は divergent snapshot を publish せず HEAD を
  動かさない — reload/recovery で回復する。
- Rule 6 — **Crash Simulation**: crash window（snapshot
  publish 後 HEAD 前の orphan・stale `HEAD.lock`・
  `*.rvf-tmp-*` 残滓・外部 HEAD 書換え）は決定的に模擬し、
  `head()`/`read_snapshot()`/再 Open が実態を正直に報告する。
  真の process kill/電源断は本契約の範囲外（Deferred）。
- Rule 7 — **Publish ≠ Project Commit**: publish `Ok` は
  durability 達成と name の可視性のみを意味する。HEAD
  repoint が commit 点であり、`Published`/`Indeterminate`/
  DirSync 未達では HEAD を勝手に進めない。
- Rule 8 — **証拠区分**: OS-L1（CI native build/link・
  headless/RT/feature）は対象 SHA または byte 同一性を
  確認した nearest-SHA の実走記録のみを Pass とする。
  Windows 実利用 FS（通常ローカル FS）の状態遷移・
  Readback/再 Open・衝突/失敗復旧は実機実走で検証する。
  macOS/Linux L2/L3・Network/Cloud/特異 FS・物理電断・
  実 GUI/Device/Plugin は Deferred/別 Owner であり、
  NotRun/Fail を Pass に読み替えない。

## Examples

- E1（Rule 1）: `store_publish::same_revision_resave_never_
  overwrites_snapshot` — rev1 に異 byte を再 save → `Err`、
  `r1.json` byte 不変・HEAD 不変。
- E2（Rule 1）: `store_publish::same_revision_resave_advances_
  id_watermark`・`in_memory_store_honors_immutable_snapshot_
  names` — watermark 単調増 refresh と InMemory parity。
- E3（Rule 2）: `store_publish::saves_publish_snapshots_then_
  repoint_head`・`failed_snapshot_publish_keeps_head_and_
  destination` — HEAD ordering と snapshot 失敗時の HEAD 不変。
- E4（Rule 2・orphan）: `store_publish::failed_head_publish_
  keeps_snapshot_orphan_and_old_head`・`satisfied_resave_
  repoints_head_onto_orphan_snapshot` — HEAD publish 失敗で
  orphan+旧 HEAD、crash window orphan の回復再利用。
- E5（Rule 1・耐久性再確立）: `store_publish::satisfied_resave_
  reestablishes_snapshot_durability`・`satisfied_resave_fails_
  instead_of_reporting_false_success` — 受理の完結 publish
  が dir-entry 耐久性を再確立し、不能なら `Err`・HEAD 不変。
- E6（Rule 3）: `head_cas::save_cas_commits_only_when_head_
  matches` — expected 一致時のみ commit、不一致は publish
  前 `Conflict`。
- E7（Rule 4）: `head_cas::concurrent_save_cas_admits_single_
  winner`・`two_process_save_cas_single_winner_typed_conflict`
  — thread/process 両次元で exactly one winner・loser の
  snapshot 非公開・lock 残滓なし。
- E8（Rule 5）: `head_cas::controller_save_cas_rejects_moved_
  head` — stale baseline controller の divergent save は
  `Conflict` で publish されず、reload で回復。
- E9（Rule 6）: `head_cas::stale_head_lock_is_broken`・
  `fresh_head_lock_blocks_save_until_released`・
  `cas_failure_releases_the_lock`・`store_publish::
  leftover_temp_siblings_do_not_disturb_the_store`・
  `external_head_rewrite_is_reported_honestly`・
  `reopen_reproduces_head_and_snapshots` — crash window 模擬と
  正直な報告・再 Open。
- E10（Rule 7）: `store_publish::failed_head_publish_keeps_
  snapshot_orphan_and_old_head` と head_cas の fencing
  tests — publish 成功≠commit、Indeterminate/lock 喪失で
  `Ok` を返さない。
- E11（Rule 8）: `evidence/sv84/case-ledger.md` — Case ID・
  Expected/Observed/NotRun・OS・SHA・CI Run URL/実機ログの
  分離記録。

## Guarantees

- `DirectoryBundleStore` は `ruviform_io_file` の公開
  `StagedFile`/`SealedFile`/`PublishFailure` のみを使い、
  独自 `.tmp`→`sync_all`→`rename` を持たない。
- Snapshot は immutable name・HEAD は snapshot 成功後のみ
  `Replace`・双方 `FileAndDirectorySync`+`RequireAtomic`。
- `save_cas` の CAS 判定は `HEAD.lock` 内で行われ、publish
  前 Conflict は orphan を残さない。
- publish 失敗は分類（effect/phase/durability/op id/temp）
  付きで `Error::Store`/`HeadCasError` へ伝播し、
  `Indeterminate` temp を自動削除しない。
- `SnapshotEnvelope` schema/serialize 順序/canonical byte・
  `ProjectStore` port・`head()`/`read_snapshot()` の公開
  契約は不変。
- 検証は `mise run check|lint|test synvelis-project` を
  正規入口とし、実 FS（Windows ローカル NTFS・CI の
  ubuntu/windows/macos leg）での実走結果のみを Pass とする。

## Non-goals

- 真の process kill・物理電断・OS 固有 Indeterminate/
  DirSync 失敗注入（ruviform-io-file 側 fault-injection
  責務 — consumer は分類転写を pin）。
- Network/Cloud/特異 FS・非対応 FS での atomic 保証
  （無条件 atomic directory transaction は主張しない）。
- macOS/Linux の L2/L3・実 GUI/実 Device/実 Plugin・
  AudioLens 等他 Consumer の受入（別 Owner/Deferred）。
- `synvelis-recording`/`synvelis-gateway`/`synvelis-player`
  の staging pipeline（別 Owner）と #86 schema 変換・
  #25 WAV Export。
- Ruviform #305 撤去候補との結合検証（統合・受入時の
  関係事項）。

## Risk

- fixture が store の内部実装に過剰結合し、正当な内部
  変更で脆くなる — 公開 port（`ProjectStore`）と観測可能な
  FS 状態（HEAD/snapshot byte/orphan/lock/temp）に限定して
  pin する方針で緩和。
- nearest-SHA CI 証拠を対象 SHA の Pass と誤読する —
  byte 同一範囲を明示し、対象 SHA run 未実施は NotRun
  pending として分離記録する規則で緩和。
- Windows 実機 fixture 実走を「実機 L2 全般の Pass」と
  過大解釈する — 対象機能（Project 保存先/通常 FS の
  Snapshot/HEAD CAS/Recovery）に限定した証拠として記録し、
  GUI/実 Device/Plugin は別 Owner へ委譲する。
