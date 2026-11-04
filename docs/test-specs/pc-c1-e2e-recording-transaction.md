---
id: "TSPEC-034RoV9mM6nXzVq1kYpQwE"
type: "test-spec"
status: "approved"
title: "PC-C1 Recording Transaction 検証仕様"
created: "2026-11-03"
updated: "2026-11-03"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/98"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/designs/pc-c1-recording-transaction-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/designs/pc-c1-recording-transaction-design.md"
  derived-by: []
  verifies:
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/designs/pc-c1-recording-transaction-design.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/103"
  defers: []
  deferred-by: []
---

# PC-C1 検証仕様

## Feature: commit 成功経路(REC-C1-01)

- Rule: commit 成功は単一の `Composite` apply = 1 undo step。
  - Example: 2 lane に跨る recording の commit 後、1 undo で全区間が
    除去される。revision は +1、`controller.saved()` は不変
    (commit は save を伴わない)。
- Rule: 記録点は manual(pre-modulation)値のまま lane へ書かれる。
  - Example: LFO assignment がある target で record_write(0.9) →
    lane point は host-resolved ではなく 0.9。
- Rule: 空 transaction は apply を行わず `applied: false` を返し、
    undo を作らない。

## Feature: Stale Revision(REC-C1-02)

- Rule: 録音中の任意の project apply(無関係 Track edit を含む)は
  `Conflict{fixed,current,affected_targets}` を既定で返す。
  - Example: record_start → record_write → `AddTrack` → commit で
    Conflict。fixed は start 時 revision、current は現在、affected_
    targets は fixed_targets。
- Rule: Conflict 後も点列・transaction は保持され、project へ誤
  適用されない。
  - Example: Conflict 後も `recording().segments` は残り、lane
    regions・revision・undo は変わらない。
- Rule: 録音中の project 変更は write gate を seal(ProjectChange)
  する。
  - Example: mid-session apply 後の record_write は拒否される。
- Rule: `approve_rebase` 再承認後にのみ commit できる(自動 Rebase
  なし)。
  - Example: Conflict → `rebase_preview` → `approve_rebase` →
    commit 成功。preview は lane 存在・重なりを報告する。

## Feature: 構造検証失敗(REC-C1-03)

- Rule: lane 削除後の commit は rebase 承認後も `Invalid` で失敗し、
  project/revision/undo は不変、点列と fixed_targets は保持される。
  - Example: `RemoveAutomationLane` → commit で Conflict →
    preview(lane_exists=false) → approve → commit で `Invalid` →
    点列・fixed_targets 保持、`record_abort` で明示的に捨てられる。
- Rule: target が消えた(参照が残らないよう lane から解除済み)
  場合も同じく `Invalid`。

## Feature: 冪等・整合性(REC-C1-04)

- Rule: commit 成功後の retry(id なし)は `AlreadyApplied` を返し、
  二重書き込みしない。
  - Example: commit → 再 commit で `AlreadyApplied{id,revision}`、
    lane regions は増えない。
- Rule: 同 id + 同 fingerprint の `finish_record_commit` 二重呼出は
  `AlreadyApplied`、同 id + 異 fingerprint は `IntegrityConflict`。
  - Example: prepared の fingerprint で 2 回 finish → AlreadyApplied。
- Rule: abort 後の commit は `NoTransaction`、unknown id の finish は
  `UnknownTransaction`。
- Rule: 保証水準は in-memory session scope(直近 64 件)と明示する。

## Feature: gate 閉鎖(REC-C1-05)

- Rule: loop wrap は transaction を seal(LoopWrap)する(既存)。
- Rule: transport stop は seal(Stopped)し、再 play 後の write は
  拒否される。
  - Example: record → write → stop → play → record_write が拒否、
    commit は可能。
- Rule: seek は open transaction を seal(Seek)する。
- Rule: 固定対象の mode 変更・arm 変更は seal(ModeChanged/
  ArmChanged)する。非固定対象の変更は seal しない。
- Rule: pause は paused 中のみ gate を閉じ、resume で同一 playback
  の recording が続く(seal しない — 維持契約)。
  - Example: latch held → pause → transport_advance でも write は
    着地せず、resume → advance で write が着地する。
- Rule: `gesture_cancel` は gesture 点列のみを捨て、transaction は
  汚染しない。
- Rule: old/unknown receipt(committed id・別 id)の finish は
  `AlreadyApplied`/`UnknownTransaction`/`IntegrityConflict` で拒否。

## Feature: scope/文脈・budget(REC-C1-06)

- Rule: clip/note scope の記録点は scope 座標のまま commit される。
  - Example: note-scoped target で project pos 11 への write は
    note-relative 1 で lane へ書かれる(pc_b 既存挙動の維持)。
- Rule: transaction は開始時の transport epoch context を保持し、
  fingerprint は deterministic(同 txn の再 prepare は同一値)。
- Rule: transaction 合計点が `RECORD_POINT_BUDGET` を超える新規
  書き込みは拒否され、既録点は保持される。
- Rule: 全経路は non-RT session 層に留まり、RT callback から
  project command/alloc/block を行わない(構造的不変条件)。

## Feature: 維持契約

- Rule: record_start の gate(record-enabled + rolling + armed
  write-mode + bound lane)は変わらない。
- Rule: `fixed_targets`/`fixed_lanes` は開始時固定 — 後の Inspector
  follow・arm・lane rebind は open transaction を retarget しない。
