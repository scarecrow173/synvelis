---
id: "TASK-034YnHBF42yVYc2C4erX2e"
type: "task"
status: "done"
title: "Issue #488 lint-app-native clippy::large_enum_variant 是正"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/488"
  changes:
    added: []
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/src/library_ui.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sv488-lint-app-native.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sv488-lint-app-native.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv488-lint-app-native.md"
  references:
    - "crates/synvelis-workspace/src/library_ui.rs"
    - "mise.toml"
  defers: []
  deferred-by: []
---

# Issue #488 lint-app-native clippy::large_enum_variant 是正

## Work

`crates/synvelis-workspace/src/library_ui.rs` の `LibraryAction`
enum で `clippy::large_enum_variant` が `ImportSubmit` variant
（`serde_json/preserve_order` feature unification 下で ≥305B）に
発火していたのを、`target: Option<ImportTargetWire>` を
`Option<Box<ImportTargetWire>>` に `Box` 化する構造修正で解消
する。`apply()` match arm は `target.as_deref()` に整合し、
serde wire form・公開シグネチャ・derive・`#[non_exhaustive]`・
`deny_unknown_fields` は不変とする。`#[allow]` 等の lint 隠蔽は
行わない。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/sv488-lint-app-native.md`
- `docs/specs/sv488-lint-app-native.md`
- `docs/designs/sv488-lint-app-native.md`
- `docs/test-specs/sv488-lint-app-native.md`
- `docs/discovery/sv488-lint-app-native.md`

## Dependencies

- なし（plan 承認済み。混入元 PR #485 は merge 済み）

## Affected Files

- `crates/synvelis-workspace/src/library_ui.rs`（`ImportSubmit.
  target` の型と `apply()` match arm の 2 箇所）
- `docs/specs|designs|plans|tasks|test-specs|discovery/
  sv488-lint-app-native.md`（新規 doc-chain）
- `docs/impl/ir/sv488-lint-app-native.md`・
  `docs/impl/exp/sv488-lint-app-native.{md,jsonl}`（新規）
- `evidence/sv488/`（新規 dir、検証証拠）

## Done When

- [x] `mise run lint-app-native` が `clippy::large_enum_variant`
  なしで pass する
- [x] `mise run check/lint/test synvelis-workspace` が pass する
- [x] `mise run test synvelis-app` が pass する（`ui_ops` 経由の
  deserialize 下流の wire 互換実測）
- [x] `mise run fmt-check` が pass する
- [x] `#[serde(tag = "action")]` の wire form・derive・
  `#[non_exhaustive]`・`deny_unknown_fields` が不変
- [ ] `ci.yml` main 向け Full CI が対象 SHA で `verify
  (ubuntu-latest)`・`evidence`・`ci-summary` success（merge 後
  CI 証拠、Coordinator が統合検証）

## Verification

- [x] `mise run lint-app-native`（修正前 exit 101 で
  `large_enum_variant` 再現 → 修正後 exit 0）
- [x] `mise run check synvelis-workspace` / `mise run lint
  synvelis-workspace` / `mise run test synvelis-workspace`
- [x] `mise run test synvelis-app`
- [x] `mise run fmt-check`
- [x] `cargo tree -p synvelis-app --features native -e features
  -i serde_json` で `gpui_perf → serde_json/preserve_order` の
  根本原因経路を確認（補助証拠）
