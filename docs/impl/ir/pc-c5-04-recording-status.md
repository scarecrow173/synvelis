---
id: "IMPL-034WaR006rHzCRQpPMb9CP"
type: "impl"
status: "completed"
title: "PC-C5 UI-C5-04 Recording status read model（headless slice）"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source: []
  changes:
    added:
      - "crates/synvelis-workspace/src/recording_status.rs"
      - "crates/synvelis-workspace/tests/uic5_04.rs"
      - "docs/specs/pc-c5-04-recording-status.md"
      - "docs/designs/pc-c5-04-recording-status-design.md"
      - "docs/plans/pc-c5-04-recording-status.md"
      - "docs/tasks/pc-c5-04-recording-status.md"
      - "docs/test-specs/pc-c5-04-recording-status.md"
    modified:
      - "crates/synvelis-workspace/src/lib.rs"
      - "docs/specs/README.md"
      - "docs/designs/README.md"
      - "docs/plans/README.md"
      - "docs/tasks/README.md"
      - "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/pc-c5-04-recording-status.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers:
    - "UI-C5-05 external GUI acceptance（synvelis#114 / ruviform#313 / ruviform#314）— NotRun で後続 gate に委譲"
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C5 UI-C5-04 Recording status read model（headless slice）

## Summary

`synvelis-workspace` に recording-status read model
`recording_status.rs` を追加した。session 証拠を 5 つの表示状態
（`Conflict` / `Prepare` / `Pending` / `Applied` / `Unknown`）へ
決定的に写像し、行ごとの説明・typed provenance・状態別の合法
action（`Inspect` / `Retry` / `ExplicitRebase` / `Abort`）を DTO
として返す pure read fn（`relation_ui.rs` 先例）。domain の
recording lifecycle semantics は一切変更せず、既存 session ops
（`control_record_commit` / `control_record_finish` /
`control_record_rebase_preview` / `control_record_approve_rebase` /
`control_record_abort`）と整合する consume-only 実装。

## Context

Issue `scarecrow173/synvelis` #102 の PC-C5 slice UI-C5-04。
UI-C5-01〜03 は merge 済み、UI-C5-05 は外部 gate
（synvelis#114 / ruviform#313 / ruviform#314）であり本 slice の
対象外。doc chain は `pc-c5-04-recording-status` 命名で
spec/design/plan/task/test-spec/impl-ir を作成済み。

## Implementation

### Changes

- `crates/synvelis-workspace/src/recording_status.rs`（新規）:
  - `recording_status(&ProjectSession)` /
    `recording_status_with(&ProjectSession, Option<&PreparedRecordingCommit>, RecordingStatusOptions)`
    — いずれも `&self` のみ参照する pure read fn。
  - `RecordingDisplayState` 5 variant、`RecordingStatusView`、
    `RecordingTxnView` / `RecordingLaneView` / `PreparedCommitView`
    / `PreparedVerdict` / `RecordingOutcomeView` /
    `RecordingExplanation` / `RecordingProvenance` /
    `RecordingAction` の typed DTO。
  - 状態写像: open txn 無し + committed disposition → `Applied`、
    open txn 無し + それ以外 → `Unknown`、open txn +
    `tx.state == Conflict` または `fixed_revision != live` →
    `Conflict`、open txn + caller-held prepared が `Live` →
    `Prepare`、その他 open txn → `Pending`。
  - `Prepare` は caller-held `PreparedRecordingCommit` を
    `preflight_record_finish` + `expected_revision == live` で
    検証した awaiting-apply 投影であり、永続 txn state ではない
    （spec/design に明記）。
  - provenance は `SealReason` / `RecordingTxnState` /
    `RecordCommitError`（`PreparedVerdict::Rejected`）/
    `RecordNotification` / `RecordDisposition` / `AppliedRecord` /
    `RecordHealthSummary` / revision drift を variant で保持し、
    文字列タグに還元しない。
  - notice tail は `RecordingStatusOptions::notice_tail`
    （既定 `RECORDING_STATUS_NOTICE_TAIL = 8`）で bounded、
    dropped 件数は `record_notifications_dropped` を verbatim で露出。
- `crates/synvelis-workspace/src/lib.rs`: module 追加と public
  API re-export のみ。
- `crates/synvelis-workspace/tests/uic5_04.rs`（新規・10 fixture）:
  実 session ops 経由の end-to-end 検証（idle→Unknown、open→Pending、
  foreign revision→Conflict、confirmed conflict→Conflict、
  explicit rebase→Pending、live prepared→Prepare、stale/foreign/
  unknown prepared→typed verdict、commit→Applied+receipt、
  empty commit→Applied w/o receipt、abort→Unknown+aborted、
  rejected→Pending、notice tail/health/gap、legal actions、
  read-side immutability/currentness）。
- doc chain `docs/{specs,designs,plans,tasks,test-specs}/pc-c5-04-recording-status*.md`
  + 各 README index 登録。

### Decisions During Implementation

- **永続 "Prepared" txn state は存在しない**: `PreparedRecordingCommit`
  は caller 保持（`control.rs`）であり、session は open txn のみを
  保持する。したがって `Prepare` は「検証済み caller-held payload
  が awaiting-apply」の投影として定義し、`Pending`（蓄積中または
  retained `Rejected` state）と区別した。spec/design に記録。
- **Conflict の predictive 判定**: `prepare_record_commit` が最初に
  検査する証拠（`tx.state == Conflict` または `fixed_revision !=
  live`）をそのまま写像に使用。revision は単調増加のため、drift
  検出時点で次回 commit の conflict が確定している。
- **Conflict に Retry を出さない**: revision が re-pin されるまで
  commit は再 conflict が確定するため、Inspect →
  `ExplicitRebase` →（commit 経路での Retry）という正直な系列を
  提示。`Prepare`/`Pending` は冪等 commit 経路として
  Inspect/Retry/Abort。
- **health は per-target lookup**: session は target 列挙を公開
  しないため、health 行は open txn の `fixed_targets` のみを対象
  （spec Deferred Design Concerns に記録）。
- **read model は mutation しない**: `preflight_record_finish` は
  `&self` であることを確認済み。view 構築で project/controller/
  session/undo history を一切変更しない。

## Related Experiments

### Adopted

<!-- 該当なし -->

### Rejected

<!-- 該当なし -->

## Validation

canonical 検証を全件実走し、すべて Pass:

- `mise run fmt-check` — Pass
- `mise run check` — Pass
- `mise run lint` — Pass（clippy `-D warnings`。初回は
  `recording_status.rs` の doc backtick / `too_many_lines` /
  `&Option<T>` param と test の `assert_is_empty` を修正して再実行で Pass）
- `mise run test` — Pass（workspace 全 test。`uic5_04` 10 fixture
  すべて ok）
- `mise run test-rt` — Pass
- `mise run check-app-native` — Pass
- `mise run lint-app-native` — Pass
- `mise run test-msrv` — Pass（rust 1.89 toolchain。
  初回はディスク容量不足 `os error 112` で失敗 → `mbx gc` で
  257 GiB 解放後に再実行して Pass）
- `mise run build` — Pass

外部 gate UI-C5-05（実 OS GUI / synvelis#114 / ruviform#313 /
ruviform#314）は `NotRun` のまま後続 gate に委譲。

## Risks

- health 行は open txn の fixed targets に限定される（session が
  target 列挙を公開しないため）。対象外 target の health は
  表示されない — spec の Deferred Design Concerns として記録済み。
- `Prepare` 判定は caller が `PreparedRecordingCommit` を渡した
  場合のみ。app 側が payload を保持・受け渡す配線は後続 slice。

## Follow-ups

- UI-C5-05 外部 GUI acceptance（synvelis#114 / ruviform#313 /
  ruviform#314）— `NotRun` で後続 gate に委譲。本 slice は
  headless read model のみで GUI wiring・`syn.control.status`・
  `syn.recording.*` op・Inspector/transport 表示は対象外。
