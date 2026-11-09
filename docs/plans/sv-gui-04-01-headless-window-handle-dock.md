---
id: "PLAN-034Uoh8zSRRCnXhWcV1XTP"
type: "plan"
status: "approved"
title: "SV-GUI-04 slice1 precursor 実施 plan"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/113"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv-gui-04-01-headless-window-handle-dock.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-04-01-headless-window-handle-dock.md"
    - "docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv-gui-04-01-headless-window-handle-dock.md"
  references: []
  defers: []
  deferred-by: []
---

# SV-GUI-04 slice1 precursor 実施 plan

## Goal

Issue #113 の precursor として、app crate に multi-window
`WindowHandle` の headless 参照実装 `HeadlessWindowHost` を追加し、
`WorkspaceManager` dock 契約と `NativeShell` の cross-window
`origin_view` 拒否を headless fixture で固定する。

## Scope

単一実装ストリーム。app 層（`screens.rs`/`shell.rs`/`ui_ops.rs`/
`app.rs`/`service.rs`/`bin/`）は別 dispatch の編集範囲のため
**触らない**。`synvelis-workspace` 以下の他 crate と
`Cargo.toml`/`Cargo.lock` も変更しない。

## Upstream Documents

- `docs/specs/sv-gui-04-01-headless-window-handle-dock.md`
- `docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md`
- Issue #113（SV-GUI-04）

## Design Inputs

- `docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md`

## File Map

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-app/src/host.rs` | `HeadlessWindowHost` 追加（`WindowHandle` impl、`WindowId` 単調発行・live 集合・呼出し記録・`fail_create` 注入）。module doc を 2 host 併存の説明に更新。 |
| `crates/synvelis-app/src/lib.rs` | `pub use host::HeadlessWindowHost` 追加（additive、5 行以内）。 |
| `crates/synvelis-app/tests/sv113_dock.rs` | 新規。`WorkspaceManager<HeadlessWindowHost>` fixture（共有 project・window close・view state 独立・transfer commit/abort・detach_to_new_window・`ShellWindowHost` pin）+ `NativeShell`+`FakeDriver` の cross-window `origin_view` 拒否。 |
| `docs/` | 本チェーン（spec/design/plan/task/test-spec）+ impl record。 |

## Implementation Sequence

- [ ] `host.rs`: `HeadlessWindowHost` 実装（`WindowHandle` 全 method、
      `with_monitors` constructor、pub record fields）
- [ ] `lib.rs`: re-export 追加
- [ ] `tests/sv113_dock.rs`: fixture test 作成
- [ ] `mise run fmt-check` / `check` / `lint` / `test` で検証
- [ ] review-protocol → `git merge origin/main` → 最終検証 →
      commit/push/PR

## Dependencies and Risks

- 依存: `ruviform-ui`（`WindowHandle`/`WindowId`/`WindowConfig`/
  `UiError`）、`synvelis-workspace`（`WorkspaceManager`/
  `ProjectSession`/`ViewType`）、`synvelis-project`
  （`ProjectController` 等 fixture 構築用）、dev-dep
  `ruviform-ui-test`（`FakeDriver`）。全て既存依存で追加無し。
- リスク: app 層並行 dispatch との merge conflict — 編集範囲を
  `host.rs`/`lib.rs`（re-export 1 行）/`tests/`/`docs/` に限定して
  回避。`lib.rs` の re-export は他 dispatch が触る可能性があるが
  additive 1 行のため conflict は自明に解決可能。

## Task Breakdown

- `docs/tasks/sv-gui-04-01-headless-window-handle-dock.md` に集約
  （単一スライスのため task 分割なし）。

## Verification

- [ ] `mise run fmt-check` — rustfmt 差分なし
- [ ] `mise run check` — `cargo check --workspace --all-targets`
- [ ] `mise run lint` — clippy `-D warnings`
- [ ] `mise run test` — `cargo test --workspace --locked`（
      `tests/sv113_dock.rs` 含む全 pass）
- [ ] test-spec の Rule/Example が fixture でカバーされることを
      コードレビューで確認

## Open Questions / Gaps

- `HeadlessWindowHost` の最終的な利用先（`SynvelisApp` の host 型
  切替 or test 専用）は後続スライスの設計判断。本スライスでは
  「契約の headless 参照実装 + fixture」として提供する。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（Orca dispatch worker が直接実装）
- Independent work streams: none
- User approval required before dispatch: no（Coordinator dispatch 済み）

## Review Handoff

- `ShellWindowHost` を変更していないこと、`host.rs` への追加が
  additive であること、`lib.rs` の diff が re-export のみである
  ことを reviewer が確認する。
