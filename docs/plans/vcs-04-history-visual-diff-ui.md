---
id: "PLAN-VCS04Hist0ryV1ewPln01"
type: "plan"
status: "approved"
title: "VCS-04 History/Visual Diff UI implementation plan"
created: "2026-11-10"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "DESIGN-VCS04Hist0ryV1ewDsn01"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/vcs-04-history-visual-diff-ui-design.md"
  implemented-by:
    - "docs/tasks/vcs-04-task1-history-diff-headless-slice.md"
  depends-on:
    - "docs/designs/vcs-04-history-visual-diff-ui-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/vcs-04-history-visual-diff-ui.md"
    - "docs/designs/vcs-04-history-visual-diff-ui-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-04-history-diff-fixtures.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-04 History/Visual Diff UI implementation plan

## Scope

`crates/synvelis-project/src/vcs/history.rs`（新規 + `vcs/mod.rs`
export）、`crates/synvelis-workspace/src/vcs_ui.rs`（新規）+
`view.rs`/`workspace.rs`/`lib.rs` の additive 変更 +
`tests/vcs04_history.rs`、`crates/synvelis-app/src/screens.rs` +
`app.rs`/`ui_ops.rs`/`service.rs`/`lib.rs` の additive 変更 +
`tests/vcs04_history.rs`、docs chain。`crates/synvelis-project/tests/`
・`synvelis-library`・`apply/`・`restore/`・他 crate は範囲外。
新規外部依存なし。

## File map

- `crates/synvelis-project/src/vcs/history.rs` — `VcsHistoryBackend`
  trait・`VcsHistoryAdapter<P>`・src 内 unit test（fake provider +
  fake `VcsBytesPort`）。
- `crates/synvelis-project/src/vcs/mod.rs` — `mod history;` +
  re-export。
- `crates/synvelis-workspace/src/vcs_ui.rs` — pin/selection/state/
  registry/orchestration/read-model 全型と関数。
- `crates/synvelis-workspace/src/view.rs` — `ViewType::VcsHistory`。
- `crates/synvelis-workspace/src/workspace.rs` — `vcs` field・
  accessor・ensure/remove hook。
- `crates/synvelis-workspace/src/lib.rs` — `pub mod vcs_ui;` +
  re-export。
- `crates/synvelis-workspace/tests/vcs04_history.rs` — headless
  acceptance（fake backend + FakeWindowHost + 実 ProjectController）。
- `crates/synvelis-app/src/screens.rs` — `VcsScreenState`・
  `history_screen_spec`・`view-3`/`dock_view_info` 登録。
- `crates/synvelis-app/src/app.rs` — `vcs_backend` field・
  `set_vcs_backend`・`vcs_session_ctx`・`vcs_history_view`。
- `crates/synvelis-app/src/ui_ops.rs` — `op_vcs_view`/`op_vcs_visual`。
- `crates/synvelis-app/src/service.rs` — OPS/dispatch 登録。
- `crates/synvelis-app/src/lib.rs` — export 追加。
- `crates/synvelis-app/tests/vcs04_history.rs` — app-level acceptance。

## Dependency graph

`vcs/history.rs`（project）→ `vcs_ui.rs`（workspace）→ `screens.rs` +
ops（app）。各層は 1 commit 単位:

1. `feat(project): VCS-04 history adapter` — adapter + unit test。
2. `feat(workspace): VCS-04 history view state + projection` —
   vcs_ui + ViewType + WorkspaceManager + tests。
3. `feat(app): VCS-04 history screen spec + syn.vcs ops` — screens +
   app/ui_ops/service + tests。
4. `docs: VCS-04 文書チェーン` — spec/design/plan/task/test-spec/impl。

## Risk register

- `VcsReadProvider` は `detect_workspace`（static method）を持ち
  object-safe でない → `VcsHistoryBackend` を別 trait として定義し
  adapter が実装（`Box<dyn VcsHistoryBackend>` で app が保持）。
- `FixedSnapshotLocator::VcsCommitted` は `fixed_revision: String`
  のみで `FixedVcsRevision.context` を持たない → pin は
  `EndpointPin::Committed{revision: FixedVcsRevision}` で full
  revision を保持する設計に確定（design 参照）。
- `ImmutableSnapshot`/`ChangeSet` は `Serialize` でない →
  `VcsHistoryState` は session-only（persist schema v2 不変）。
- `op_vcs_visual` の pin 解決は provider I/O を伴う → non-RT op
  面で実行（RT thread では呼ばない、既存 op と同じ境界）。

## Verification matrix

| 検証 | 手段 |
| --- | --- |
| adapter 契約 | project src 内 unit test + workspace/app fake |
| A/B ChangeSet 共有 | workspace test（同一 cache Arc・digest 一致） |
| Focus/Selection・Stale | workspace test（move/detach/reattach + live 前進） |
| read-only oracle | workspace/app test（revision・undo・engine builds 不変） |
| ViewSpec mount | app test（ViewHost + spec 構造 assertion） |
| fmt/lint/test | `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/`test-msrv` |
| native lane | `check-app-native`/`lint-app-native`（存在範囲） |
