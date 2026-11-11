---
id: "TASK-VCS04BkndWiringTask001"
type: "task"
status: "done"
title: "VCS-04 production set_vcs_backend 配線 slice — task"
created: "2026-11-12"
updated: "2026-11-12"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/95"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/tests/vcs_backend_wiring.rs"
      - type: "file"
        path: "docs/specs/vcs-04-backend-wiring.md"
      - type: "file"
        path: "docs/designs/vcs-04-backend-wiring-design.md"
      - type: "file"
        path: "docs/plans/vcs-04-backend-wiring-plan.md"
      - type: "file"
        path: "docs/tasks/vcs-04-backend-wiring-task.md"
      - type: "file"
        path: "docs/test-specs/vcs-04-backend-wiring.md"
      - type: "file"
        path: "docs/impl/ir/vcs-04-backend-wiring.md"
    modified:
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-app/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/vcs-04-backend-wiring-plan.md"
  implemented-by:
    - "docs/impl/ir/vcs-04-backend-wiring.md"
  depends-on:
    - "docs/plans/vcs-04-backend-wiring-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/vcs-04-backend-wiring.md"
    - "docs/designs/vcs-04-backend-wiring-design.md"
    - "docs/test-specs/vcs-04-backend-wiring.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vcs-04-backend-wiring.md"
  references: []
  defers: []
  deferred-by: []
---

# VCS-04 production set_vcs_backend 配線 slice — task

## Work

Issue #95 の残件 slice。headless read-only slice（PR #252）が残した
production `set_vcs_backend` 配線を実装し、`project.new`/`open`
が project root 単位で実 `GitVcsProvider` を bind / re-bind する
ようにする。あわせて VCS-facing project file `main.json` を
saved HEAD から materialize する。

## Tasks

- [x] T1 `synvelis-app/Cargo.toml` に `synvelis-library` 依存追加 +
  `Cargo.lock` 再生成
- [x] T2 `app.rs`: `VCS_PROJECT_FILE`・`bind_vcs_backend`・
  `refresh_project_file` 実装、new/open/save へ配線
- [x] T3 `tests/vcs_backend_wiring.rs`: real git CLI fixture +
  bind/re-bind/clear・committed/staged diff の 2 acceptance test
- [x] T4 doc chain（spec/design/plan/task/test-spec/impl-ir）
- [x] T5 canonical mise lane 検証

## Done When

- [x] `project.new`/`open` が bundle を含む git workspace を検出し
  実 backend を bind（非 repo は `None` → `NoBackend`）
- [x] 別 project root で re-bind、`repository_id` が root ごとに
  変わり stale backend を保持しない
- [x] committed/staged endpoint が実 git `main.json` で resolve
  され実 commit を跨ぐ diff が動く
- [x] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-app-native`/`lint-app-native`/`test-msrv`/`build` を実走

## Verification

- `cargo test -p synvelis-app --test vcs_backend_wiring` — 2/2
  Pass（Windows・git CLI 利用、fixture は git 不在時 honest skip）
- canonical mise lane の結果は impl record / PR 本文に記録。

## Deferred（Issue へ引き継ぎ）

- unborn HEAD の `head_oid`/`resolve_oid` TransientIo 誤分類 —
  `synvelis-library` 側の別欠陥（Coordinator が別 Issue 起票）
- native mount/dock 配線（`screens.rs`/`synvelis-native.rs`）—
  後続 slice
- 実 OS GUI/IME/2Window= 実機 gate（VCS-UI-05 → synvelis#114）
- Overlay / Evolution Map / Rendered Audio Diff — spec 非目標の
  後続 slice

## Sources and Relations

- [spec](../specs/vcs-04-backend-wiring.md) /
  [design](../designs/vcs-04-backend-wiring-design.md) /
  [plan](../plans/vcs-04-backend-wiring-plan.md) /
  [test-spec](../test-specs/vcs-04-backend-wiring.md)
