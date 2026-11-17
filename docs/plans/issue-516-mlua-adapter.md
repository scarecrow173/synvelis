---
id: "PLAN-1kBHRQoUwWJI51A3YqHacf"
type: "plan"
status: "approved"
title: "Issue #516 実装 plan — 薄い binding への置換"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/516"
    - "docs/specs/issue-516-mlua-adapter.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/issue-516-mlua-adapter.md"
  implemented-by:
    - "docs/tasks/issue-516-mlua-adapter.md"
  depends-on:
    - "docs/designs/issue-516-mlua-adapter.md"
    - "docs/specs/issue-516-mlua-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-516-mlua-adapter.md"
  derived-by:
    - "docs/tasks/issue-516-mlua-adapter.md"
    - "docs/test-specs/issue-516-mlua-adapter.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-516-mlua-adapter.md"
  references:
    - "docs/test-specs/issue-516-mlua-adapter.md"
  defers: []
  deferred-by: []
---

# Issue #516 実装 plan

## ステップ

1. port 語彙調整: `crates/synvelis-script/src/backend.rs` の
   `ScriptErrorKind::BudgetExceeded.kind` を `Option<BudgetKind>` へ
   （doc comment に中立契約が次元を運ばない旨を記録）。
2. 依存更新: `crates/synvelis-script-luau/Cargo.toml` に
   `ruviform-script`/`ruviform-script-luau` git 依存を追加、
   `mlua`/`serde_json`(通常)除去 → `serde_json` は dev-dependency。
3. adapter 実装: `crates/synvelis-script-luau/src/lib.rs` を
   anti-corruption adapter へ全面書換（design §構造・
   fail-closed 対応表に従う）。
4. test 更新: `crates/synvelis-script-luau/tests/backend.rs` を
   製品契約の挙動へ整合（record payload 必須・`Some(kind)` パターン・
   分類差の記録）。
5. lockfile 更新: `cargo check` で `Cargo.lock` を再生成
   （ruviform rev `cbcc8343` → `84a68f25`）。
6. 検証: fmt → clippy(-D warnings) → 対象 crate test →
   consumer regression（automation test・cli check）→
   `check-headless-deps` → prek。
7. doc chain 完成（discovery/design/spec/plan/tasks/test-specs/
   impl ir+exp + ADR + evidence）→ review → commit → PR。

## 依存順序

- 1→2→3→4→5 は実装、6 は検証、7 は記録。doc chain は実装と
  並行してよいが ADR/spec は実装確定後に approved とする。

## リスク

- ruviform pin 更新が workspace 全 crate に影響 → Daily Full CI
  の検証に委譲、PR 本文に明記。
- 製品 backend の error 分類差（Internal vs ContractViolation）
  → binding-fit 表 §6 に記録済み、consumer は区別していない。
- build.rs の C++ shim コンパイル時間増 → 許容（headless 維持）。
