---
id: "TASK-034UogPe6cIh8LtcFphyuH"
type: "task"
status: "in-progress"
title: "SV-GUI-04 slice1 precursor: multi-window WindowHandle + headless dock fixture"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/113"
  changes:
    added:
      - "crates/synvelis-app/tests/sv113_dock.rs"
      - "docs/specs/sv-gui-04-01-headless-window-handle-dock.md"
      - "docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md"
      - "docs/plans/sv-gui-04-01-headless-window-handle-dock.md"
      - "docs/tasks/sv-gui-04-01-headless-window-handle-dock.md"
      - "docs/test-specs/sv-gui-04-01-headless-window-handle-dock.md"
      - "docs/impl/ir/sv-gui-04-slice1-precursor-headless-windowhandle-dock-fixture.md"
    modified:
      - "crates/synvelis-app/src/host.rs"
      - "crates/synvelis-app/src/lib.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sv-gui-04-01-headless-window-handle-dock.md"
  implemented-by:
    - "docs/impl/ir/sv-gui-04-slice1-precursor-headless-windowhandle-dock-fixture.md"
  depends-on:
    - "docs/plans/sv-gui-04-01-headless-window-handle-dock.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-04-01-headless-window-handle-dock.md"
    - "docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-04-01-headless-window-handle-dock.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-04 slice1 precursor: multi-window WindowHandle + headless dock fixture

## Work

Issue #113（SV-GUI-04）の precursor スライス。headless 契約層のみ:

- `crates/synvelis-app/src/host.rs` に `HeadlessWindowHost` を追加
  — `WindowHandle` を実装し、複数 `WindowId` の発行・live 管理・
  close flow・呼出し記録を行う in-memory host。`ShellWindowHost`
  の `BackendUnavailable` は維持。
- `crates/synvelis-app/src/lib.rs` に re-export を additive 追加。
- `crates/synvelis-app/tests/sv113_dock.rs` を新規作成 —
  `WorkspaceManager<HeadlessWindowHost>` と `NativeShell`+
  `FakeDriver` の fixture test。

触らない範囲: `screens.rs`/`shell.rs`/`ui_ops.rs`/`app.rs`/
`service.rs`/`bin/`（別 dispatch `ctx_9c7ca0adf4b2` が編集中）、
他 crate、`Cargo.toml`/`Cargo.lock`。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/sv-gui-04-01-headless-window-handle-dock.md`
- `docs/specs/sv-gui-04-01-headless-window-handle-dock.md`
- `docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md`
- `docs/test-specs/sv-gui-04-01-headless-window-handle-dock.md`

## Dependencies

- なし（並行 dispatch とはファイル範囲で分離済み）。

## Affected Files

- `crates/synvelis-app/src/host.rs`（追加のみ）
- `crates/synvelis-app/src/lib.rs`（re-export 1 行）
- `crates/synvelis-app/tests/sv113_dock.rs`（新規）
- `docs/`（本チェーン + impl record）

## Done When

- [x] `HeadlessWindowHost` が `WindowHandle` を実装し test-spec
      の Rule H1〜H6・Example E1〜E5 が `tests/sv113_dock.rs` で
      検証される
- [x] `ShellWindowHost::create_window` が `BackendUnavailable` の
      ままであることが pin される
- [x] `mise run fmt-check` / `check` / `lint` / `test` が Pass
- [x] 文書チェーン完備 + impl record 作成
- [x] review-protocol で blocker 無し → commit/push/PR

## Verification

- [x] `mise run fmt-check`
- [x] `mise run check`
- [x] `mise run lint`
- [x] `mise run test`（`sv113_dock` 含む）
- [x] `git merge origin/main` 後の再検証
