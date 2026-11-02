---
id: "PLAN-wDLeQm8n2jpOzr5utDhOSF"
type: "plan"
status: "approved"
title: "VS-D Recording Commit / E2E 実装計画"
created: "2026-11-02"
updated: "2026-11-02"
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
    - "docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/vs-d-commit-identity-manifest-single-mutation.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/vs-d-recording-commit-e2e-idempotency-undo-reload.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vs-d-e2e-r3-recording-commit.md"
  references: []
  defers: []
  deferred-by: []
---

# VS-D Recording Commit / E2E 実装計画 Implementation Plan

## Goal

Issue #16 の E2E-R 完了 Gate: `FinalizedSourceRef` を
`RecordingOp::CommitRecording` 単一 mutation で Project へ冪等登録し、
commit manifest による事前 ID 確保・Applied Commit Registry・
commit marker・recoverable 分離を実装する。`recording-poc` の
`vsd` シナリオで record→finalize→commit→save→reload→undo/redo→retry
を決定的に実証し、VS-001〜015 を全網羅する。

## Scope

単一実装ストリーム（1 plan / 1 worktree）。`synvelis-project` への
registry/API 追加、`synvelis-recording` への commit モジュール追加、
`recording-poc` の vsd シナリオ追加のみ。既存 RT/engine/writer/
recovery 経路は変更しない。新規外部依存なし。

## Upstream Documents

- `docs/specs/vs-d-recording-commit-e2e-idempotency-undo-reload.md`（approved）
- `docs/adr/vs-d-commit-identity-manifest-single-mutation.md`（accepted: I-1+M-1+S-1+R-1+E-1+P-1+C-1+U-1）

## Design Inputs

- `docs/designs/vs-d-recording-commit-e2e-idempotency-undo-reload.md`（approved）
- `docs/designs/project-asset-persistence-architecture.md`（#17）

## File Map

- `crates/synvelis-project/src/recording_data.rs` —
  `AppliedRecordingCommit` + `applied_commits`（serde default）
- `crates/synvelis-project/src/controller.rs` —
  `draw_raw_id`（generator から raw u128 発行）
- `crates/synvelis-project/src/lib.rs` — re-export
- `crates/synvelis-recording/src/commit.rs` —
  `RecordingCommitManifest`/`CommitStatus`/`commit_status`/
  `prepare_manifest`/`commit_session`/`CommitReport`/marker・
  payload plan（segment merge・take/clip 構成）
- `crates/synvelis-recording/src/ops.rs` —
  `RecordingOp::CommitRecording`（validate 全検査/apply 全登録/reseed）
- `crates/synvelis-recording/src/lib.rs` — モジュール公開・re-export
- `crates/synvelis-recording/tests/vs_d.rs` — VS-001〜015 受入試験
- `crates/recording-poc/src/main.rs` — `vsd` シナリオ + report
- `crates/recording-poc/tests/vsd.rs` — PoC 出力試験
- `docs/impl/ir/` — 実装記録

## Implementation Sequence

- [ ] task1: `applied_commits` registry + `draw_raw_id`（synvelis-project）
- [ ] task2: `commit` モジュール manifest/plan/status/marker（synvelis-recording）
- [ ] task3: `RecordingOp::CommitRecording` validate/apply + driver
  `prepare_manifest`/`commit_session`
- [ ] task4: `recording-poc` `vsd` シナリオ + 決定的 report
- [ ] task5: `tests/vs_d.rs` 受入試験全網羅（VS-001〜015）+ PoC 試験
- [ ] task6: 検証・merge origin/main・生成物再生成・impl record

## Dependencies and Risks

- `apply` 内 `reseed(max_used+1)` が watermark 戻り衝突を防ぐ唯一の
  防御ではなく、validate の ID 一意検査も併用する（二重防御）。
- Undo で registry entry が消えた場合の再 commit は同一 manifest で
  合法再適用とする（採番なし）。再採番が必要になるケースを作らない。
- `#[serde(default)]` を `applied_commits` に付けて旧 snapshot
  読み込みを維持する（deny_unknown_fields と併用可）。
- `deny_unknown_fields` の RecordingRegistry に field 追加するため、
  旧 VS-A 試験 snapshot は default で復元される。
- commit 対象 0 の session（全 stream partial/faulted）は mutation を
  発行せず report のみ返す（部分適用なし）。
- Windows dir sync 非対応は既存 capability 記録に従う（新規保証を
  主張しない）。

## Task Breakdown

- task1 → task2 → task3 → task4 → task5 → task6（直列。task3 は
  task1/2 に依存、task4 は task3 に依存、task5 は task4 に依存）

## Verification

- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`build`/`test-msrv` 全緑
- [ ] `tests/vs_d.rs` が VS-001〜015 全項目を実測で PASS
- [ ] `recording-poc` vsd シナリオが 2 回同一出力
- [ ] crash 前後（apply 前/marker 前）・save→reload・undo/redo の
  idempotency を実測
- [ ] impl record に VS-001〜015 → test 対応表を記録

## Open Questions / Gaps

- Partial media の明示 commit、session 追記 commit、media GC は後続
  Issue（design §8）。
- `commit_status` の IntegrityConflict 検出は content hash 比較のみ
  （payload の field 単位差分報告はしない）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（単一 worktree 直列）
- Independent work streams: none
- User approval required before dispatch: no（review-protocol が承認
  gate を代替、design approved 済）

## Review Handoff

- 重点: commit validate の全検査が部分適用を型で防ぐか、manifest
  ID 確保と `apply` reseed が watermark 不整合を防ぐか、
  AlreadyApplied が新 revision を生まないか、Partial/faulted stream
  が committed へ混入しないか、undo/redo が media を触れないか、
  PCM が command に乗らないか。
