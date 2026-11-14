---
id: "IMPL-034X5pypMll8dagLa8AERw"
type: "impl"
status: "completed"
title: "Workspace Model — Issue #2 設計文書化 + headless 検証"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/2"
  changes:
    added:
      - type: "file"
        path: "docs/specs/workspace-model.md"
      - type: "file"
        path: "docs/designs/workspace-model-design.md"
      - type: "file"
        path: "docs/plans/workspace-model.md"
      - type: "file"
        path: "docs/tasks/workspace-model-task.md"
      - type: "file"
        path: "docs/test-specs/workspace-model.md"
      - type: "file"
        path: "docs/impl/ir/workspace-model.md"
      - type: "file"
        path: "docs/impl/exp/workspace-model.md"
      - type: "file"
        path: "evidence/sv2/workspace-model-case-ledger.md"
      - type: "file"
        path: "evidence/sv2/workspace-model-verification.md"
      - type: "file"
        path: "crates/synvelis-workspace/tests/workspace_model.rs"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/impl/exp/README.md"
  implements:
    - "docs/tasks/workspace-model-task.md"
  implemented-by: []
  depends-on:
    - "docs/plans/workspace-model.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/workspace-model-design.md"
    - "docs/specs/workspace-model.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/workspace-model-task.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/workspace-model.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/workspace-model.md"
    rejected: []
---

# Workspace Model — Issue #2 設計文書化 + headless 検証

## Summary

Issue #2「Workspace Model — GPUI-neutral View/Driver Port・Window/View
寿命独立・Selection/Link Group/Command Routing 設計・Headless 検証」の
文書チェーン（spec→design→plan→task→test-spec→ir/exp）と `evidence/sv2/`
台帳を作成し、既存 WS-A/ED-A/UI-C5/SV-GUI-04 headless fixture を
WSM-T-* Case ID へ対応付けた。契約ギャップとして「再生中
（transport playing）」の dock/window churn を直接検証する test が
無かったため `crates/synvelis-workspace/tests/workspace_model.rs` を
1 件追加した。実装コードの変更はない（文書 + test のみ）。

## Context

- Task: `docs/tasks/workspace-model-task.md`（TASK-034X53taZcU5UQQfAN0qS5）
- Plan: `docs/plans/workspace-model.md`（PLAN-034X52muh3ycW5cWGEkyBN）
- Spec: `docs/specs/workspace-model.md`（SPEC-034X4TqMpHOeFHvohktOZV, approved）
- Design: `docs/designs/workspace-model-design.md`
  （DESIGN-034X4TrgrbyPHCdc8l0xPg, approved）
- Test-spec: `docs/test-specs/workspace-model.md`
  （TSPEC-034X53lawfMjkjYxK3vCLJ）
- Issue: synvelis#2（本 slice の対象）。関連: synvelis#3（UX spec）、
  #113/#114/#175/#176（実装・実機 gate）。

## Implementation

### Changes

- `docs/specs/workspace-model.md` — WSM-R-01〜09 要件・WSM-AC-01〜09
  受入条件（Issue #2 AC の転記・対応付け）・Deferred Design Concerns・
  Ruviform/Synvelis Issue 相互参照。
- `docs/designs/workspace-model-design.md` — GPUI-neutral Workspace 構造図
  （`WindowDriver`/`ViewHost`/`NativeSurfaceSlot`/`SurfaceSlotId`/
  `SlotLease`）、WebView 固有記述の legacy 注記、寿命独立性不変条件表、
  識別子体系、Close/Move/Restore フロー、Command Routing 文脈表、
  Event→Receipt 例、Normal/Failure oracle 表、Decision Log WSM-D-01〜10、
  未決事項、Deferred Gates。
- `docs/plans/workspace-model.md` — file map・sequence・verification。
- `docs/tasks/workspace-model-task.md` — work items・done-when。
- `docs/test-specs/workspace-model.md` — WSM-T-01〜20 Case → test 対応表、
  NotRun/Deferred 表。
- `crates/synvelis-workspace/tests/workspace_model.rs` — 新規
  `playback_dock_window_churn_preserves_transport_revision_and_history`
  （WSM-T-08）。`transport.play` で playing を立て、detach/reattach/
  move/window event/close veto churn 後に transport・revision・
  graph_builds・undo roundtrip・view state が不変であることを検証。
- `evidence/sv2/workspace-model-case-ledger.md` — WSM-AC→WSM-T→test→
  Expected/Observed/Result 台帳 + NotRun/Deferred + router blocker 記録。
- `evidence/sv2/workspace-model-verification.md` — 実行コマンド・OS・
  toolchain・exit code の検証記録。

### Decisions During Implementation

- 既存 fixture が WSM-T-01〜07・09〜15 をほぼ網羅しているため、新規
  test は WSM-T-08（playing 中 churn）のみに限定した。`ws_a_05` は
  transport stopped のみを検証するため、「再生中」の qualifier は
  別 test で補完するのが最小の整合的変更。
- `ShellWindowHost` は single-window（`create_window` →
  `BackendUnavailable`）のままとし、multi-window の意味論は
  `FakeWindowHost`/`HeadlessWindowHost`/`FakeDriver` の headless oracle
  のみで検証する。製品配線は #113/#175 の gate。
- window-owned job cancel は `CallOutcome::Job` を返す op が無く
  headless fixture を作れないため、NotRun とし `UiRuntime` 契約
  （close/destroy は job を自動 cancel しない）を設計不変条件として
  記録した。
- `doc-driven-dev-graph` router は pre-existing broken-relation で
  fail-closed blocked。範囲外のため修復せず、evidence に外部 blocker
  として記録した。

## Related Experiments

### Adopted

- `docs/impl/exp/workspace-model.md` — 文書チェーン生成・既存 fixture
  再利用・playback churn gap の検出と補完の記録。

### Rejected

- なし（新規 workspace 実装・大規模 fixture 追加は既存契約との重複・
  hard-code 化リスクのため採用しなかった）。

## Validation

- `mise run fmt-check` — exit 0
- `mise run check synvelis-workspace` — exit 0
- `mise run lint synvelis-workspace` — exit 0
- `mise run test synvelis-workspace` — exit 0（`workspace_model.rs` 含む
  全 test pass。初回は新規 test の revision 会計ミスで FAIL → undo/redo
  が revision/graph_builds を +2 進める ws_a_05 と同じ会計に修正して
  pass）
- `mise run check synvelis-app` — exit 0
- `mise run lint synvelis-app` — exit 0
- `mise run test synvelis-app` — exit 0（38 test binaries 全件 pass）
- `node .claude/skills/doc-status/scripts/doc_status.js lint --type
  {spec,design,plan,task,test-spec} --json` — 新規文書の blocking
  finding は index 未生成と plan→test-spec relation のみで、index
  再生成と `verified-by` 追加で解消
- `node .claude/skills/doc-maintenance/scripts/doc_maintenance.js
  index apply` — 5 dir written + impl/ir（IR 作成後に再実行）
- review-protocol — spec+design で 1 サイクル実施、CONFIRMED P0/P1
  なし（非存在 op 名・壊れた節参照・relation 種別の 3 件を修正）。
  最終レビューは本 IR/exp/evidence 完成後に実施。

## Risks

- headless Pass を実機 Pass と誤読するリスク — evidence/台帳で
  lane を明示し NotRun/Deferred を分離済み。
- `doc-driven-dev-graph` の repo 全体 blocked は baseline 問題として
  残存 — 本 task 範囲外。

## Follow-ups

- synvelis#114: 実 OS Window/Dock/IME/Keyboard/pixel 検証（WSM-T-16/19）。
- synvelis#176 系: 実 plugin editor/NativeSurfaceSlot（WSM-T-17）。
- synvelis#113/#175: 製品 multi-window 配線・job op（WSM-T-18/21）。
- macOS/Linux GUI lane: 使用要件発生時（WSM-T-20）。
