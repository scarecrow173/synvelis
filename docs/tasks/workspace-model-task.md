---
id: "TASK-034X53taZcU5UQQfAN0qS5"
type: "task"
status: "in-progress"
title: "Workspace Model Task — Issue #2 design + headless verification"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/2"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/workspace-model.md"
  implemented-by: []
  depends-on:
    - "docs/plans/workspace-model.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/workspace-model.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/workspace-model.md"
  references: []
  defers: []
  deferred-by: []
---

# Workspace Model Task — Issue #2 design + headless verification

## Work

1. spec/design/plan/task/test-spec の文書チェーンを確定する
   （WSM-R-*/WSM-AC-*、GPUI-neutral 図、寿命独立性、Decision Log）。
2. 既存 headless fixture（`ws_a.rs`/`ed_a.rs`/`adopt.rs`/`preset.rs`/
   `two_window.rs`/`sv113_dock.rs`/`dock_ops.rs`/`close_veto.rs`/
   `shell.rs`/`ui_ops.rs`）を Case ID（WSM-T-*）へ対応付ける。
3. 契約ギャップ確認: 「再生中（transport playing）」の dock/window churn を
   直接検証する test が無いため `crates/synvelis-workspace/tests/
   workspace_model.rs` を追加する（WSM-T-08）。他にギャップがあれば
   同ファイルへ最小限追加する。
4. `mise run fmt-check`/`check`/`lint`/`test`（synvelis-workspace,
   synvelis-app）を実行し、`evidence/sv2/` に Case ID・Expected/
   Observed/NotRun・OS・toolchain・exit code を記録する。
5. `docs/impl/ir`・`docs/impl/exp` を作成し、docs index を
   doc-maintenance で再生成する。
6. review-protocol で最終レビューし、blocker を解消して commit/push/PR。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/workspace-model.md`（PLAN-034X52muh3ycW5cWGEkyBN）
- `docs/specs/workspace-model.md`（SPEC-034X4TqMpHOeFHvohktOZV）
- `docs/designs/workspace-model-design.md`（DESIGN-034X4TrgrbyPHCdc8l0xPg）
- `docs/test-specs/workspace-model.md`（TSPEC-034X53lawfMjkjYxK3vCLJ）

## Dependencies

- WS-A/ED-A/UI-C5/SV-GUI-04 の既存実装・test（本 branch に存在）。
- Ruviform port 契約（`WindowDriver`/`DockWorkspace`/`NativeSurfaceSlot`/
  `UiRuntime`）は git 依存として `Cargo.lock` で pin。

## Affected Files

- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/workspace-model*.md`
- `evidence/sv2/`
- `crates/synvelis-workspace/tests/workspace_model.rs`（新規 test のみ）
- docs index（`docs/*/README.md`、doc-maintenance で再生成）

## Done When

- [ ] 文書チェーン 7 文書が relations 整合で存在し、spec/design が approved。
- [ ] WSM-T-* Case が test-spec と evidence 台帳で Expected/Observed/
  NotRun が記録されている。
- [ ] `workspace_model.rs` の playback churn test が追加され pass する。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`（両 crate）が exit 0。
- [ ] review-protocol で CONFIRMED P0/P1 blocker が残っていない。
- [ ] docs index が再生成され audit が新規 finding を出さない。

## Verification

- `mise run fmt-check`
- `mise run check synvelis-workspace` / `mise run check synvelis-app`
- `mise run lint synvelis-workspace` / `mise run lint synvelis-app`
- `mise run test synvelis-workspace` / `mise run test synvelis-app`
- `node .claude/skills/doc-maintenance/scripts/doc_maintenance.js index apply`
- `node .claude/skills/doc-status/scripts/audit_docs.js`（新規文書の integrity）
