---
id: "TASK-issue-516-mlua-adapter"
type: "task"
status: "done"
title: "Issue #516 tasks — 薄い binding への置換"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/516"
    - "docs/plans/issue-516-mlua-adapter.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/plans/issue-516-mlua-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-516-mlua-adapter.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/issue-516-mlua-adapter.md"
  references:
    - "docs/impl/ir/issue-516-mlua-adapter.md"
    - "docs/impl/exp/issue-516-mlua-adapter.md"
  defers: []
  deferred-by: []
---

# Issue #516 tasks

- [x] T1 binding-fit 照合表を `evidence/sv516/binding-fit.md` に
  記録（全差分を網羅・分類）。
- [x] T2 `synvelis-script/src/backend.rs` — `BudgetExceeded.kind`
  を `Option<BudgetKind>` へ変更（doc comment 付き）。
- [x] T3 `synvelis-script-luau/Cargo.toml` — ruviform git 依存追加・
  mlua/serde_json 除去・serde_json を dev-dependency へ。
- [x] T4 `synvelis-script-luau/src/lib.rs` — anti-corruption
  adapter へ全面書換。
- [x] T5 `synvelis-script-luau/tests/backend.rs` — 製品契約挙動へ
  整合（record payload・`Some(kind)`・分類差）。
- [x] T6 `Cargo.lock` 再生成（ruviform rev `84a68f25`）。
- [x] T7 検証: fmt・clippy・対象 test・consumer regression・
  check-headless-deps・prek。
- [x] T8 doc chain（discovery/design/spec/plan/task/test-specs/
  impl ir+exp）+ ADR 記録。
- [x] T9 review・commit・push・PR 作成・worker_done。
