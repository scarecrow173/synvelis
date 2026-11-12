---
id: "TASK-034WaQD0ytfaRVbcWofg2n"
type: "task"
status: "done"
title: "PC-C5 slice4 タスク分解 — Recording status read model（UI-C5-04・headless）"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/pc-c5-04-recording-status.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c5-04-recording-status.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/pc-c5-04-recording-status.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-c5-04-recording-status.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C5 slice4 タスク分解 — Recording status read model（UI-C5-04・headless）

## Work

1. `crates/synvelis-workspace/src/recording_status.rs` を新設:
   - `RecordingDisplayState` / `RecordingStatusView` / `RecordingTxnView` /
     `RecordingLaneView` / `RecordingOutcomeView` / `PreparedCommitView` /
     `PreparedVerdict` / `RecordingExplanation` / `RecordingProvenance` /
     `RecordingAction` / `RecordingStatusOptions` を定義。
   - `recording_status(session)` / `recording_status_with(session, prepared, options)`
     を実装。`preflight_record_finish` 等の read API 形状を実装時に確認し、
     副作用があれば design の縮小案（id+fingerprint+expected_revision 検証）に従う。
   - 写像: open txn 無し → `Applied`(Committed disposition)/`Unknown`、
     open txn 有り → drift で `Conflict`、prepared Live で `Prepare`、他 `Pending`。
   - actions: Conflict=[Inspect,ExplicitRebase,Abort]、
     Prepare/Pending=[Inspect,Retry,Abort]、Applied/Unknown=[]。
2. `crates/synvelis-workspace/src/lib.rs` に `pub mod recording_status;` と
   re-export を追加（既存パターンに倣う）。
3. `crates/synvelis-workspace/tests/uic5_04.rs` を新設（8〜12 fixture、
   実 session ops 経由で全 5 状態・typed provenance・合法 action・
   notice tail・health・stale prepared verdict を検証）。
4. canonical 検証 9 タスクを全て実走。
5. Conventional Commit → push → 日本語 PR → Issue #102 コメント →
   worker_done。

## Classification

- 種別: feature（headless read model slice）
- リスク: 低〜中（domain 変更なし・read-only・既存 ops の消費のみ）
- 依存: `control.rs` の `preflight_record_finish`/`record_health`/
  `record_notifications`/`applied_record`/`last_disposition` の公開 API。
