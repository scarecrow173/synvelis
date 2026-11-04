---
id: "IMPL-034RwzRkGd3wl6hL83YIBo"
type: "impl"
status: "completed"
title: "PC-C1 recording transaction prepare/apply/finalize"
created: "2026-11-04"
updated: "2026-11-04"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/98"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_c1.rs"
      - type: "file"
        path: "docs/impl/ir/pc-c1-recording-transaction-prepare-apply-finalize.md"
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/control.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/controls_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/session.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/pc_a.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/tasks/pc-c1-task1-prepare-apply-finalize.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/adr/pc-c1-commit-session-cas-orchestration-prepared-commit.md"
    - "docs/designs/pc-c1-recording-transaction-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-c1-e2e-recording-transaction.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C1 recording transaction prepare/apply/finalize

## Summary

Issue #98 の Automation Recording コミット安全性(データ保全)を
`crates/synvelis-workspace` に実装した。従来
`build_commit_ops` が `recording.take()` していた破壊的コミットを、
非破壊 `prepare_record_commit` → 固定 revision CAS `apply` →
成功時のみ消費する `finish_record_commit` の 3 相に分離し、
失敗・競合時は全点列を保持する。REC-C1-01..06 の受入テスト 12 件を
`tests/pc_c1.rs` に追加。

## Context

Spec: docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md
ADR: docs/adr/pc-c1-commit-session-cas-orchestration-prepared-commit.md
Design: docs/designs/pc-c1-recording-transaction-design.md
Plan: docs/plans/pc-c1-recording-transaction.md / Test spec:
docs/test-specs/pc-c1-e2e-recording-transaction.md / Task:
docs/tasks/pc-c1-task1-prepare-apply-finalize.md。

## Implementation

### Changes

- `control.rs`:
  - `RecordingTransactionId` (session 内単調・1 開始)、
    `RecordingTxnState` (Pending/Conflict/Rejected/Committed)、
    `RecordCommitError` (NoTransaction/UnknownTransaction/Conflict/
    Rejected/AlreadyApplied/IntegrityConflict/Invalid) を新設。
  - `RecordingTransaction` に `id`/`epoch`/`fixed_lanes`/`state`
    を追加。`fixed_revision` を CAS 期待値として実際に利用
    (従来は保持のみで未使用)。
  - `SealReason` に Stopped/Seek/ModeChanged/ArmChanged/
    ProjectChange を追加し、`transport_stop`/`transport_seek`/
    `set_mode`/`set_arm`/外部 project 変更で write gate を閉鎖。
    seal は write gate のみ閉じ、commit/abort/inspect は可能
    (LoopWrap 契約維持)。
  - `record_start` が `RecordingTransactionId` を返し、
    target→lane 写像と transport epoch を固定捕捉。
    `record_points` は固定 lane に記録し、`RECORD_POINT_BUDGET`
    (262_144 点) 超過 batch は既存点を保持したまま全体拒否。
  - `bind_write_lane` は open transaction の固定 target への再
    binding を拒否(黙った retarget 禁止)。
  - `prepare_record_commit` は非破壊:revision drift(無関係な
    変更を含む)は既定 `Conflict`、固定 target/lane の構造検証
    失敗は `Rejected`、成功時に `ProjectOp::Composite`
    (lane 毎 `WriteAutomationRegion`・`Replace`)と fingerprint
    (FNV-1a) を持つ `PreparedRecordingCommit` を返す。
  - `preflight_record_finish` が apply **前**に receipt registry
    と open transaction の id/fingerprint を照合 — 二重適用・
    payload 偽装・別 tx への付け替えを構造的に排除。
  - `finish_record_commit` が唯一の consumer。成功 apply 後のみ
    take し、`AppliedRecord` receipt を bounded registry
    (`APPLIED_REGISTRY_CAP` = 64) に記録。同一 id+指紋は
    `AlreadyApplied`、同 id 別 payload は `IntegrityConflict`。
  - `note_commit_rejected`/`note_project_changed`/`rebase_preview`/
    `approve_rebase`/`record_abort`/`last_disposition`/
    `applied_record` を追加。rebase はユーザー明示承認のみで
    write gate を再開しない。
- `session.rs`: `control_record_commit` を prepare→
  `apply_internal`(CAS 期待値 = `prepared.expected_revision`)→
  finish に再構成。`apply`/`undo`/`redo` 成功時に
  `note_project_changed` を呼ぶ(外部変更として gate 閉鎖)。
  自身の commit は外部変更ではないため `apply_internal` で通知を
  迂回。`control_record_finish`/`control_record_rebase_preview`/
  `control_record_approve_rebase` を公開。
- `controls_ui.rs`: `record_start` が tx id を返却、
  `record_finish`/`record_rebase_preview`/`record_approve_rebase`
  を Inspect/Retry/Explicit Rebase/Abort の UI 面として追加。
- `lib.rs`: 新公開型・定数を re-export。
- `tests/pc_a.rs`: mid-session apply が gate を閉じる新契約に
  合わせ fixture を調整(modulation 設定を record_start 前へ移動)。
  意味(手動値 ≠ host 解決値)は不変。
- `tests/pc_c1.rs`: REC-C1-01..06 を 12 テストで headless 検証。

### Decisions During Implementation

- 外部 project 変更(成功 apply/undo/redo)は write gate を
  `ProjectChange` で閉鎖するが点列は破棄しない。次回 prepare が
  `Conflict` を返し、明示 rebase のみが commit 経路を回復する。
- 空 transaction の commit は `op: None` で apply を行わず
  undo 履歴も増えない(REC-C1-01 の "no phantom undo")。
- receipt registry は in-memory・session scoped・cap 64。
  crash recovery / 永続 registry は別契約(issue #12 側)で、
  本スライスは二重書込み防止の exactly-once のみを保証する。
- prepare は `tx` の immutable borrow 中に `reject_open`
  (mutable) を呼べないため、検証・op 構築を read-only 閉包に
  分離し、失敗確定後に `Rejected` を立てる構成とした。
- review で検出し修正:旧 `build_commit_ops` の segment ソート
  (`sort_by_key(pos)`)を新 `prepare` でも clone 上で維持 —
  gesture 点は呼び出し順で格納されるため region は常に昇順
  正規化して書き出す。
- `preflight` は apply 前照合専用:二重 ACK・stale prepared・
  別 id が open tx を横取りする経路を、project を動かす前に
  全て型付きエラーで返す(実装レビューで発見した
  apply-後照合の二重書き抜け穴を塞いだ)。

## Related Experiments

### Adopted

なし — 承認済み設計どおりに実装。

### Rejected

なし。

## Validation

| 項目 | 結果 |
| --- | --- |
| `mise run fmt` / `fmt-check` | PASS |
| `mise run check` | PASS (`--workspace --all-targets`) |
| `mise run lint` | PASS (`-D warnings`) |
| `mise run test` | PASS (workspace 全テスト + pc_c1 12 件) |
| `mise run build` | PASS (`--workspace --locked`) |
| `mise run test-msrv` | PASS (rust 1.89、全テスト) |
| `git merge origin/main` | Already up to date |

受入対応: REC-C1-01 `rec_c1_01_*` (1 undo・raw manual 値・
save 分離・empty apply なし)、REC-C1-02 `rec_c1_02_*`
(foreign change/undo で Conflict・点列保持・explicit rebase のみ
commit)、REC-C1-03 `rec_c1_03_*` (lane/target 削除で構造
reject・project/revision/undo 不変・点列保持)、REC-C1-04
`rec_c1_04_*` (二重 ACK exactly-once・同 id 別 payload
IntegrityConflict・stale prepared の open tx 横取り防止)、
REC-C1-05 `rec_c1_05_*` (loop wrap/stop/seek/mode/disarm/
gesture cancel/old receipt で gate 閉鎖・latch pause/resume は
非 seal)、REC-C1-06 `rec_c1_06_*` (Clip/Note scope・epoch/
tempo 分離・point budget 原子拒否・RT callback 非所有は既存
構造で保証)。

### 既知の repo 健全性(既存・本スライス非因果)

- `doc_status audit` の残存 findings は migration/legacy 文書の
  既存事項であり、PC-C1 成果物への blocking finding は 0 件。

### 未検証(正直な記録)

- 実 plugin・実 GUI・MultiWindow focus/delayed ACK fault fixture
  は対象外 (PC-C6 #103 の oracle)。本スライスは headless
  Controller failure injection 経路のみで検証。
- receipt registry は in-memory のみ:process 再起動を跨ぐ
  exactly-once は issue #12 側の永続化契約に委譲。

## Risks

- `applied` registry は cap 64 の bounded window — 64 tx 以上
  前の重複 id は `UnknownTransaction` に退化する(誤書込みは
  発生しないが `AlreadyApplied` ではなくなる)。
- 空 commit は apply がないため receipt を残さない。同一
  prepared の retry は `UnknownTransaction` を返す(書込み
  副作用はないため実害なし)。

## Follow-ups

- AUT-01..04 (#131–#134) が本契約の後続利用側。
- MultiWindow/GUI Focus/delayed ACK fault fixture は PC-C6 #103。
- 永続 receipt / crash recovery は issue #12 lane。
