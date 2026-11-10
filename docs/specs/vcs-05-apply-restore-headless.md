---
id: "SPEC-vcs05-apply-restore-headless"
type: "spec"
status: "approved"
title: "VCS-05 Safe Apply Selected Change / Restore As Copy — headless slice（synvelis#96）"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "ISSUE-96"
    - "ISSUE-92"
    - "ISSUE-93"
    - "ISSUE-94"
    - "ISSUE-7"
  verified_by:
    - "TSPEC-vcs05-apply-restore-fixtures"
  implemented_by:
    - "TASK-vcs05-apply-restore-headless"
  supersedes: []
  superseded_by: []
---

# VCS-05 Safe Apply Selected Change / Restore As Copy — headless slice

## 目的

Issue #96（親設計 Issue #7 VCS-05）。VCS-01 の immutable snapshot と
VCS-02 の canonical ChangeSet の上に、**明示的・安全・非実時間**の
2 操作を headless で実装する。

1. **Safe Apply Selected Change**: ChangeSet の選択 subset を live
   `Project` へ `ProjectController` 単一 writer 経由で適用する。
   CAS（expected revision / target fingerprint / snapshot identity）・
   approval・whitelist・fail-closed preflight・1 undo step・冪等
   receipt を保証する。
2. **Restore As New Copy**: 解決済み snapshot を**新規 project copy**
   として destination に publish する。source project / Saved HEAD /
   Git index / working tree は不変。asset・plugin-state・schema・
   provenance を検証し、staged publish + readback + crash recovery
   を行う。

## スコープ外（本 slice では実装しない）

- `Removed` change の cascade 適用（別設計）
- recording/comping/SMF/clock-epoch 領域の apply
- engine/runtime への実適用、GUI、RT
- 実 OS filesystem durability 認定・実 Provider E2E → Issue #97
- AudioLens cross-consumer gate → Issue #294
- in-place restore、Git checkout、元 project 上書き（禁止事項）

## 受入条件（Issue #96 より）

- Single/複数 field apply、Clip/Track removed, stale revision,
  misidentified target, missing plugin, invalid port/asset を
  Fail unchanged。成功は 1 undo step、redo/Save/Reload/Transport 非回帰。
- 同 Commit を二回 Apply しても意図しない二重実行なし。
  Replay/ACK gap/Crash で正確 Receipt。Unknown/Partial ChangeSet は
  Apply 不可。
- Restore to new copy: complete media+schema/provenance を検証して
  Offline Playback。原 Project と Git Working Tree unchanged。
  target exists/case-conflict/symlink/path traversal/dirsync
  unsupported/low disk/cancel に安全対応。

## 設計決定

- **Apply は typed entity transplant**: JSON path patch は使わず、
  semantic object kind ごとに whole-entity payload を after snapshot
  から live へ移植する。親 membership（`lane_ids`/`clip_ids`/
  `port_ids`/`processors`/`notes`）は保護 field とし、選択された
  child change と `Reordered` change で明示 reconcile する。
- **Fingerprint は宣言 selection 由来**: `{target_object} ∪
  selected change objects` を `(kind, stable_id, canonical entity
  JSON)` で hash。`Added` 対象は live 側 `null` として両側一致。
- **Idempotency**: intent の canonical digest から決定的 apply key を
  導き、`ApplyJournal`（`Pending`/`Applied`/`Failed`）に記録。
  `Applied` は receipt replay、`Pending` は live==after なら
  `Replayed` に reconcile、さもなければ通常 CAS で再試行。
- **Restore は content-addressed bundle**: `DirectoryBundleStore`
  互換 layout（`snapshots/r<N>.json` + `HEAD` + `blobs/sha256/<hex>`
  + `plugin-state/`）。staging dir → validate/readback →
  same-dir rename publish。`Absent` のみ受理、case-conflict・
  symlink・traversal は事前 reject。`AllowDegraded` は明示選択時のみ
  で receipt に degraded を記録。
- **Lineage**: 新 `ProjectId` + 全 stable ID preserve。lineage の
  永続化は receipt / `restore.json` manifest のみ（schema v11 は
  不要と判断 — Project schema 非変更）。

## 検証

`crates/synvelis-project/tests/vcs05_apply.rs`（13 件）と
`vcs05_restore.rs`（7 件）。mise タスクで fmt-check / check / lint /
test / build / test-msrv を実行。実 OS FS durability・実 Provider・
plugin host・GUI・AudioLens は NotRun → #97/#294。
