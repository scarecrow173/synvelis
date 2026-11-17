---
id: "TASK-034ZjLJBI0ZsQ3Gifc0ggL"
type: "task"
status: "done"
title: "Issue #283 slice — strip-fields gateway exercise tasks"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/283"
    - "docs/plans/issue-283-strip-gateway-exercise.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-283-strip-gateway-exercise.md"
  implemented-by:
    - "docs/impl/ir/cb-06-track-bus-adapter.md"
  depends-on:
    - "docs/plans/issue-283-strip-gateway-exercise.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/cb-06-track-bus-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-283-strip-gateway-exercise.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/cb-06-track-bus-adapter.md"
  references:
    - "evidence/sv283/verification.md"
  defers: []
  deferred-by: []
---

# Issue #283 slice — strip-fields gateway exercise tasks

- [x] T1: `crates/synvelis-gateway/tests/cb06_strip_gateway.rs`
      新規 — test-local strip fixture（`AddTrackStrip` +
      `InsertProcessor`、head revision 2）・fork・ordinary op
      typed reject（clip.mute/track.create/clip.move）・stale
      scope digest / branch revision reject・`ApproveToApply`
      agent approval adopt・discard・list/describe/compare・
      domain boundary/capability 補助 test
- [x] T2: doc chain 5 点（discovery/specs/designs/plans/tasks、
      slug `issue-283-strip-gateway-exercise`）作成
- [x] T3: cb-06 doc chain 追従（`docs/test-specs/cb-06-track-bus-adapter.md`・
      `docs/impl/ir/cb-06-track-bus-adapter.md`・
      `docs/impl/exp/cb-06-track-bus-adapter.md`）
- [x] T4: `evidence/sv283/acceptance-ledger.md`・
      `evidence/sv283/verification.md` へ gateway lane 追記
- [x] T5: `review-protocol` レビュー・blocker 0 確認
- [x] T6: 正規検証（`mise run fmt-check` / `check` / `lint` /
      `test synvelis-gateway` / `check-headless-deps` /
      `check synvelis-project` / `prek run --all-files`）・
      merge origin/main・commit・push・PR・worker_done
