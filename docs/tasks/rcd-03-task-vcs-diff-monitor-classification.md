---
id: "TASK-leaTVahsznfck7liCrDtL6"
type: "task"
status: "done"
title: "RCD-03 Task VCS semantic diff monitor 宣言分類"
created: "2026-11-11"
updated: "2026-11-11"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/rcd-03-vcs-diff-monitor-classification-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-03-vcs-diff-monitor-classification-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-03-vcs-diff-monitor-classification.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 Task VCS semantic diff monitor 宣言分類

## Work

`crates/synvelis-project/src/diff/` に authored monitor 宣言の
分類を実装する。`ObjectKind::{TrackMonitorDecl, PunchMarks}` を
enum 末尾に追加し `domain()` = `Recording` に写像、
`RecordingAdapter` で `monitoring.tracks` の `diff_map` 照合と
`monitoring.punch` の singleton declaration-presence diff を行い、
`DIFF_POLICY_VERSION` を 2 に進める。`tests/vcs02_diff.rs` に
monitor 分類セクションを追加して test-spec R1–R4 を検証する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/rcd-03-vcs-diff-monitor-classification-plan.md`
- `docs/designs/rcd-03-vcs-diff-monitor-classification-design.md`
- `docs/specs/rcd-03-vcs-diff-monitor-classification.md`
- `docs/test-specs/rcd-03-vcs-diff-monitor-classification.md`

## Dependencies

- 先行 slice（monitor persistence）: `Project::monitoring` POD・
  `SetTrackMonitor`/`ClearTrackMonitor`/`SetPunchMarks` op は
  merge 済み。
- VCS-02 diff engine の既存契約（`diff_map`・`diff_presence`・
  `diff_matched`・`raw_is_ambiguous`・`emit_change`・`json_diff`）。

## Affected Files

- `crates/synvelis-project/src/diff/types.rs`
- `crates/synvelis-project/src/diff/engine.rs`（コメントのみ）
- `crates/synvelis-project/src/diff/adapters.rs`
- `crates/synvelis-project/tests/vcs02_diff.rs`
- `docs/`（本 chain + index 再生成 + impl-ir）

## Done When

- [ ] `ObjectKind::TrackMonitorDecl`/`PunchMarks` が `Recording`
  domain で分類され、decl upsert/clear・punch set/clear/変更が
  `Added`/`Removed`/`Modified` として diff に現れる。
- [ ] `monitoring.tracks` が `raw_kind_map` 未登録のまま
  keyed-synthetic として照合される。
- [ ] `tests/vcs02_diff.rs` の monitor セクションが R1–R4 を
  検証して pass。既存 fixture 全 pass（非回帰）。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`build` が pass。
- [ ] review-protocol で blocker なし。

## Verification

- [ ] `mise run test`（canonical）+ fixture 実行。
- [ ] `git fetch origin main && git merge origin/main` 後の再検証。
