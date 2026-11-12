---
id: "PLAN-034WaQlEhakW3yisBvWEHv"
type: "plan"
status: "approved"
title: "PC-C5 slice4 実施計画 — Recording status read model（UI-C5-04・headless）"
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
    - "docs/specs/pc-c5-04-recording-status.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/pc-c5-04-recording-status.md"
    - "docs/test-specs/pc-c5-04-recording-status.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c5-04-recording-status.md"
    - "docs/designs/pc-c5-04-recording-status-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/pc-c5-04-recording-status.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C5 slice4 実施計画 — Recording status read model（UI-C5-04・headless）

## Goal

`synvelis-workspace` に recording transaction の status read model
（`recording_status`/`recording_status_with` → `RecordingStatusView`）を
追加し、UI-C5-04 の 5 表示状態・typed provenance・合法 action を
headless fixture で実証する。domain/app 側は変更しない。

## ステップ

1. **read model 実装** `src/recording_status.rs`
   - `RecordingDisplayState`（Conflict/Prepare/Pending/Applied/Unknown）・
     `RecordingStatusView`・`RecordingTxnView`・`RecordingLaneView`・
     `RecordingOutcomeView`・`PreparedCommitView`・`PreparedVerdict`・
     `RecordingExplanation`・`RecordingProvenance`・`RecordingAction`・
     `RecordingStatusOptions`。
   - `recording_status(session)` = `recording_status_with(session, None, Default)`。
   - `recording_status_with` は live revision・record_enabled・transport・
     `recording()`・`last_disposition()`・`applied_record()`・
     `record_notifications()`・`record_health(target)` を収集し、
     design の写像表に従って state を決定、explanations/actions を構築。
   - `is_current(&ProjectSession)` を `view.revision == session.revision()` で提供。
2. **re-export** `src/lib.rs`
   - `pub mod recording_status;` + `pub use recording_status::{...}`。
3. **fixture** `tests/uic5_04.rs`（8〜12 test）
   - helper は `pc_c1.rs`/`uic5.rs` の deterministic setup を踏襲
     （`Project::new`→track/target/lane→`control_with` で bind/mode/arm/
     record enabled/transport play→`record_start`）。
   - 各状態・provenance・action・notice tail・stale prepared・health を
     実 session ops 経由で検証。
4. **canonical 検証**
   - `mise run fmt-check` / `check` / `lint` / `test` / `test-rt` /
     `check-app-native` / `lint-app-native` / `test-msrv` / `build`。
5. **報告**
   - Conventional Commit・日本語 PR・Issue #102 コメント・worker_done。

## 編集範囲（厳守）

- `crates/synvelis-workspace/src/recording_status.rs`（新規）
- `crates/synvelis-workspace/src/lib.rs`（re-export のみ）
- `crates/synvelis-workspace/tests/uic5_04.rs`（新規）
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir}/pc-c5-04-recording-status*.md`（新規）

変更禁止: `snap.rs`/`state.rs`/`ppqn_ui.rs`/`controls_ui.rs`、
`synvelis-app/**`、`synvelis-project/src/{ops,control}.rs`・
`tests/ctl_02.rs`、`synvelis-player/**`、`synvelis-recording/**`。

## リスク

- `preflight_record_finish` の API 形状（`&self` か `&mut self` か、
  戻り値）は実装時に `control.rs` で再確認する。副作用があれば
  read model から呼べないため、その場合は fingerprint/id 検証 +
  `expected_revision` 比較のみに留め、設計 doc を修正する。
- `record_health` の公開 signature（target 引数の型・戻り値）を確認。
- `PreparedRecordingCommit` の field 公開範囲（`op`/`fingerprint`/
  `expected_revision`/`was_sealed` が pub か）を確認 — private なら
  view は id + verdict のみ公開に縮小する。
