---
id: "PLAN-034YnGGwltZg3zi7eNzQFs"
type: "plan"
status: "completed"
title: "Issue #488 lint-app-native clippy::large_enum_variant 是正 — 実装 plan"
created: "2026-11-16"
updated: "2026-11-16"
owners: []
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
    - "docs/specs/sv488-lint-app-native.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv488-lint-app-native.md"
    - "docs/designs/sv488-lint-app-native.md"
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

# Issue #488 lint-app-native clippy::large_enum_variant 是正 — 実装 plan

## Goal

`LibraryAction::ImportSubmit` の large variant を構造修正
（`target` field の `Box` 化）で `clippy::large_enum_variant` を
解消し、`mise run lint-app-native` を pass させる（Issue #488）。

## Scope

単一の実装ストリーム。変更は `crates/synvelis-workspace/src/
library_ui.rs` の enum field 型と match arm の 2 箇所に限定され、
同一の検証 lane で一括確認できるため 1 plan とする。

## Upstream Documents

- `docs/specs/sv488-lint-app-native.md`

## Design Inputs

- `docs/designs/sv488-lint-app-native.md`

## File Map

- `crates/synvelis-workspace/src/library_ui.rs` - `LibraryAction::
  ImportSubmit.target` を `Option<Box<ImportTargetWire>>` に変更、
  `apply()` match arm を `target.as_deref()` に整合
- `docs/specs|designs|plans|tasks|test-specs|discovery/
  sv488-lint-app-native.md`・`docs/impl/{ir,exp}/
  sv488-lint-app-native*` - 本タスクの doc-chain と実装記録
  （新規）
- `evidence/sv488/` - 検証証拠（新規 dir）

## Implementation Sequence

- [x] `cargo tree -p synvelis-app --features native -e features
  -i serde_json` で `gpui_perf → serde_json/preserve_order` の
  feature unification 経路を確認し、`Value` 肥大化が根本原因と
  特定する
- [x] `mise run lint-app-native` で `large_enum_variant`
  （ImportSubmit ≥305B vs ImportRebase ≥80B、exit 101）を再現する
- [x] `ImportSubmit.target` を `Option<Box<ImportTargetWire>>` に
  変更し、`apply()` の `target.as_ref()` を `target.as_deref()`
  に整合する（wire form・公開シグネチャは不変）
- [x] `mise run check synvelis-workspace` / `mise run lint
  synvelis-workspace` / `mise run test synvelis-workspace`
- [x] `mise run lint-app-native`（修正後）/ `mise run test
  synvelis-app` / `mise run fmt-check`（実行結果は
  `evidence/sv488/` と PR 本文に記録）
- [x] doc-chain・IR/exp・`evidence/sv488/` を作成し commit・
  push・PR 化する

## Dependencies and Risks

- dependency: なし（混入元 PR #485 は merge 済み、main `f05bdbf`
  に `LibraryAction` が存在）
- risk: `Box` 化で variant サイズが閾値を下回らない → `target`
  が variant の支配的 payload（`ImportTargetWire` は 2 つの
  `serde_json::Value` を含む）であり、`Box` 化で ~200B 削減
  される設計上の見積もり。最終判定は `lint-app-native` の実測に
  委ねる
- risk: serde wire form の変化 → `Option<Box<T>>` は serde に
  透過的で JSON shape 不変。`test synvelis-app`（`ui_ops` 経由の
  deserialize 下流）で実測確認

## Task Breakdown

- `docs/tasks/sv488-lint-app-native.md` に全手順を集約した単一
  task とする（小規模修正で細分化の利益がない）

## Verification

- [x] `mise run lint-app-native` が pass する（`large_enum_variant`
  が出ない）— 修正後 exit 0 を `evidence/sv488/` に記録
- [x] `mise run check synvelis-workspace` / `mise run lint
  synvelis-workspace` / `mise run test synvelis-workspace`
- [x] `mise run test synvelis-app`（`ui_ops` deserialize 下流の
  wire 互換実測 — 47 test binaries 全て 0 failed）
- [x] `mise run fmt-check`
- [ ] `mise.ci.toml` pin 1.99.0・ubuntu-latest での `verify
  (ubuntu-latest)`・`evidence`・`ci-summary` success は merge 後
  の対象 SHA の CI 証拠で確認（AC2、Coordinator が統合検証）

## Open Questions / Gaps

- `serde_json/preserve_order` feature unification が他の
  `serde_json::Value` 保持構造へ与える影響範囲は未棚卸し
  （design の Deferred Design Concerns に記録）。

## Self-Review

- [x] spec の要求がすべて task に対応している
- [x] placeholder が残っていない
- [x] file path、relation、status、用語が整合している

## Implementation Handoff

- Recommended mode: inline execution（単一の小規模修正）
- Independent work streams: none
- User approval required before dispatch: yes（review-protocol の
  レビューで blocker 0 を確認済みとして扱う）
- If approved: discover available implementation and delegation
  capabilities in the current environment, then execute
  task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same
  dependency graph and verification matrix.

## Review Handoff

- レビュー観点: `#[allow]` を使わない構造修正であること、wire
  form（`#[serde(tag = "action")]` の JSON shape）が不変である
  こと、crate 外 consumer（`synvelis-app`）に変更が不要である
  こと、検証が実測ベースで記録されていること。
- upstream: spec / design / test-spec を参照。
