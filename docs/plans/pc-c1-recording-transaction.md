---
id: "PLAN-034RoUbM3cA2J9g5wWzEfG"
type: "plan"
status: "approved"
title: "PC-C1 Recording Transaction 実装計画"
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
  implements:
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/designs/pc-c1-recording-transaction-design.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-a-parameter-control.md"
    - "docs/plans/pc-b-parameter-dispatch.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/pc-c1-commit-session-cas-orchestration-prepared-commit.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c1-recording-transaction-prepare-apply-finalize.md"
    - "docs/designs/pc-c1-recording-transaction-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-c1-e2e-recording-transaction.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C1 Recording Transaction 実装計画

## Goal

Issue #98: commit を非破壊 `prepare` → CAS `apply` → 成功時のみ
`finish` に分離し、失敗・競合時の点列保持・既定 Conflict・Explicit
Rebase・gate 閉鎖・session-scope 冪等を headless で実装する。
`mise run fmt-check`/`check`/`lint`/`test` 全 green + impl record まで。

## Scope

- `crates/synvelis-workspace/src/control.rs`: transaction 拡張
  (id/epoch/fixed_lanes/state)、`prepare_record_commit`/
  `finish_record_commit`/`note_commit_rejected`/`rebase_preview`/
  `approve_rebase`/`note_project_changed`、SealReason 拡張、
  applied registry + disposition、point budget、`RecordCommitError`。
- `crates/synvelis-workspace/src/session.rs`: `control_record_commit`
  の prepare→apply→finish orchestration、`apply`/`undo`/`redo` の
  `note_project_changed` 通知、private `apply_commit`。
- `crates/synvelis-workspace/src/controls_ui.rs`: `record_commit` の
  error 型適合 + `record_rebase_preview`/`record_approve_rebase`。
- `crates/synvelis-workspace/src/lib.rs`: 新型の export。
- tests: `tests/pc_c1.rs` 新規(REC-C1-01..06)、`tests/pc_a.rs`/
  `tests/pc_b.rs` の既存経路適合。

## Task breakdown

1. **T1: transaction 拡張 + gate 閉鎖(control.rs)**
   `RecordingTransactionId`/`RecordingTxnState`/`fixed_lanes`/`epoch`/
   `state`、SealReason 拡張、record_start 採番、fixed_lanes 参照の
   record_points、bind_write_lane 固定対象拒否、seal 各経路、
   `note_project_changed`、point budget。
2. **T2: prepare/finish/rebase + error 型(control.rs)**
   `RecordCommitError`/`PreparedRecordingCommit`/`RebasePreview`、
   fingerprint(FNV-1a)、applied registry + disposition。
3. **T3: session orchestration + UI 適合(session.rs/controls_ui.rs/lib.rs)**
   `control_record_commit` 3 段化、`apply_commit` 分離、
   `note_project_changed` 配線、UI error 型・rebase 面。
4. **T4: 既存テスト適合 + `tests/pc_c1.rs` REC-C1-01..06**
   pc_a mid-session apply の再構成(録音前へ)、pc_b 適合、
   新規受入テスト。
5. **T5: 最終検証 + impl record**
   `git fetch origin` → `git merge origin/main` → `mise run fmt/
   fmt-check/check/lint/test` → `docs/impl/ir/pc-c1-*.md` → commit。

## Risk register

- 既存テストが mid-session apply に依存(pc_a_05) → 録音前へ移動
  して性質を維持( modulation 前の手動値は録音中にも残る)。
- `record_start` signature 変更 → controls_ui/tests 全呼び出し側を
  一括適合。戻り値 id は既存呼び出しで無視可能。
- `SealReason` 追加は exhaustive match を壊し得る → `cargo check`
  で全 match を洗う。
- applied registry の有限窓 → 保証水準を spec/impl record に明記。

## Verification matrix

| 受入 | 検証 |
| --- | --- |
| REC-C1-01 | pc_c1: commit 成功 → 1 undo 全域除去、manual pre-modulation 値、saved() 不変、lane/target 正しい |
| REC-C1-02 | pc_c1: 無関係 Track edit → Conflict{fixed,current,affected}、点列保持、誤適用なし、preview→approve→commit |
| REC-C1-03 | pc_c1: lane 削除 → Conflict→rebase→Invalid/Rejected、project/revision/undo 不変、fixed targets 保持 |
| REC-C1-04 | pc_c1: id-less retry → AlreadyApplied、id+fp 二重 finish → AlreadyApplied、id+異 payload → IntegrityConflict |
| REC-C1-05 | pc_c1: LoopWrap/Stop/Seek/Mode/Disarm/ProjectChange seal、pause-resume 継続、gesture_cancel、unknown/old receipt |
| REC-C1-06 | pc_c1: clip/note scope 記録点保持、epoch context 固定、budget 超過拒否、(RT 経路非該当の構造的不変) |
| 全体 | `mise run fmt/fmt-check/check/lint/test` 全 green |
