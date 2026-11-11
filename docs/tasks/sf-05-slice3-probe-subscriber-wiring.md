---
id: "TASK-wQ8xJkLmNrTyUiOpAsDfGh"
type: "task"
status: "in-progress"
title: "SF-05 slice3 タスク分解 — probe subscriber/capture dedup の app/UI 配線"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/108"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-05-slice3-probe-subscriber-wiring-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-05-slice3-probe-subscriber-wiring-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-05-slice3-probe-subscriber-wiring.md"
    - "docs/designs/sf-05-slice3-probe-subscriber-wiring-design.md"
    - "docs/test-specs/sf-05-slice3-probe-subscriber-wiring.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-05-slice3-probe-subscriber-wiring.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-05 slice3 タスク分解 — probe subscriber/capture dedup の app/UI 配線

## Work

Issue #108 の slice-3（SF-UX-06 headless 側）。SF-04 slice5
の session 層 lease/capture ownership を `syn.probe.control`
の view-scoped action・`op_view_close`/`close_vcs_view` の
release hook・`syn.probe.state` の holders/capture owner/
subscribers 投影・Signal Flow tab の probe table・
`ui/src/ipc.ts` の `OpName` へ配線し、headless fixture で
検証する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/specs/sf-05-slice3-probe-subscriber-wiring.md`
- `docs/designs/sf-05-slice3-probe-subscriber-wiring-design.md`
- `docs/plans/sf-05-slice3-probe-subscriber-wiring-plan.md`
- `docs/test-specs/sf-05-slice3-probe-subscriber-wiring.md`
- `docs/specs/sf-04-slice5-probe-subscriber-ownership.md`

## Dependencies

- SF-04 slice4（`syn.probe.*` op 基盤）・SF-04 slice5
  （session lease/capture ownership、PR #304）・SF-05
  slice1/2（Signal Flow read model/screen、merge 済み）の
  main 済み契約。
- ブランチ `feat/108-sf05-probe-share`（worktree
  `scarecrow173/sv108-probe` 起点）。

## Affected Files

- `crates/synvelis-app/src/ui_ops.rs`
- `crates/synvelis-app/src/screens.rs`
- `crates/synvelis-app/tests/sf05_probe_share.rs`（新規）
- `ui/src/ipc.ts`
- `docs/` チェーン一式 + `evidence/reports/sf-06-cross-gate-ledger.md`
  （SF-UX-02/03/07 stale 行の訂正のみ）

## Done When

- [ ] `syn.probe.control` の `subscribe`/`unsubscribe`/
  `capture_for`/`abort_capture` が `view_id`→
  `ViewInstanceId` 解決で動作し、session authority action
  と分離されている
- [ ] 同一 `(session, point, kind)` の 2 view subscribe が
  provider request 1 回の dedup を wire 層で証明する
- [ ] `op_view_close`/`close_vcs_view` が当該 view の lease
  のみ解放し、最終 lease でのみ provider remove が走る
- [ ] owner-only abort・非 lease view の fail-closed が
  `INVALID_PARAMS` で検証される
- [ ] `syn.probe.state` が `holders`/`capture_owner`/
  `subscribers` を投影する
- [ ] Signal Flow tab が main view の probe rows を
  read-only table で表示し、headless screen で検証される
- [ ] `ui/src/ipc.ts` の `OpName` に probe 2 op がある
- [ ] `tests/sf05_probe_share.rs` が pass
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv`/`check-app-native`/`lint-app-native`/`check-ui`/
  `test-ui` が pass
- [ ] docs index 再生成・commit/push・PR（`Refs #108`）

## Verification

- [ ] `mise run fmt-check` exit 0
- [ ] `mise run check` exit 0
- [ ] `mise run lint` exit 0
- [ ] `mise run test` exit 0
- [ ] `mise run test-rt`/`test-msrv`/`check-app-native`/
  `lint-app-native`/`check-ui`/`test-ui` exit 0
- [ ] 実 OS/GPUI・実 2-window・#109 oracle は NotRun（#114 gate）

## Sources and Relations

- [spec](../specs/sf-05-slice3-probe-subscriber-wiring.md) /
  [design](../designs/sf-05-slice3-probe-subscriber-wiring-design.md) /
  [plan](../plans/sf-05-slice3-probe-subscriber-wiring-plan.md) /
  [test-spec](../test-specs/sf-05-slice3-probe-subscriber-wiring.md)
