---
id: "TSPEC-034YnFeOkg2FPSb1D7r4ci"
type: "test-spec"
status: "approved"
title: "Issue #488 lint-app-native clippy::large_enum_variant 是正 — 検証仕様"
created: "2026-11-16"
updated: "2026-11-16"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/488"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sv488-lint-app-native.md"
  derived-by: []
  verifies:
    - "docs/specs/sv488-lint-app-native.md"
  verified-by: []
  references:
    - "crates/synvelis-workspace/src/library_ui.rs"
    - "crates/synvelis-app/src/ui_ops.rs"
    - "mise.toml"
  defers: []
  deferred-by: []
---

# Issue #488 lint-app-native clippy::large_enum_variant 是正 — 検証仕様

## Purpose

`LibraryAction` enum の `clippy::large_enum_variant` が構造修正
で解消され、wire/serde 互換が維持されることを保証する test
spec。lint 属性による隠蔽ではなく variant サイズの縮小で解決
すること、および `#[serde(tag = "action")]` の JSON shape が
不変で crate 外 consumer が無変更で動くことを、将来の enum
拡張に対しても保持すべき不変条件として記録する。

## Feature

large variant 構造修正: `LibraryAction::ImportSubmit` の
`target` field は `Option<Box<ImportTargetWire>>` であり、
`clippy::large_enum_variant` が `serde_json/preserve_order`
feature unification 下でも発火しない。serde の wire form は
`Option<ImportTargetWire>` 時代と同一 JSON shape を保つ。

## Rules

- Rule: lint 非隠蔽 — `#[allow]`/`#[expect]`・`workspace.lints` /
  `[lints]` の緩和を追加しない。variant サイズの構造修正のみで
  解消する。
- Rule: wire 互換 — `#[serde(tag = "action", deny_unknown_fields)]`、
  `#[serde(rename = "...")]`、field の `#[serde(default)]`、
  derive（Clone/Debug/PartialEq/Serialize/Deserialize）、
  `#[non_exhaustive]` を維持する。`Option<Box<T>>` は serde に
  透過的であり JSON shape は不変。
- Rule: 境界限定 — 変更は `crates/synvelis-workspace/` 内に閉じ、
  crate 外 consumer（`synvelis-app` の `op_library_action`）の
  変更を必要としない。
- Rule: 検証の実測性 — Pass 判定は実際に実行した mise タスクの
  結果のみに基づき、未実行の検証を Pass と記録しない。

## Examples

- Example: lint-app-native 再現と解消 — Given main `f05bdbf`
  の未修正コード, When `mise run lint-app-native` を実行する,
  Then `clippy::large_enum_variant`（ImportSubmit ≥305B vs
  ImportRebase ≥80B）で exit 101 となる。When `target` を
  `Option<Box<ImportTargetWire>>` に修正して再実行する, Then
  `synvelis-workspace` の lint が `large_enum_variant` なしで
  pass する。
- Example: wire 互換 — Given 修正後の `LibraryAction`, When
  `{"action": "import.submit", "import_id": ..., "locator": ...,
  "target": {...}}` を `serde_json::from_value` で decode する,
  Then `Option<Box<ImportTargetWire>>` が `Option<
  ImportTargetWire>` と同一 shape で deserialize され、
  `import_submit` の `decode_target` 経路が不変に動く。
- Example: feature 非依存 — Given `serde_json/preserve_order`
  が無効な feature unification（`lint synvelis-workspace`
  単独）, When `mise run lint/test synvelis-workspace` を実行
  する, Then `Box` 化は serde 透過のため全 test が pass する。

## Guarantees

- 正しい実装は、`mise run lint-app-native` が
  `clippy::large_enum_variant` を報告せず pass することを保証
  する。
- 正しい実装は、`#[serde(tag = "action")]` の JSON wire form が
  修正前と同一であることを保証する。
- 正しい実装は、`mise run test synvelis-app`（`ui_ops` の
  deserialize 下流を含む）が pass することを保証する。
- 正しい実装は、`mise.ci.toml` pin 1.99.0・ubuntu-latest での
  AC1/AC2 は merge 後の対象 SHA の CI 証拠で確認されることを
  保証する（ローカル検証の成功は統合受入の成功ではない）。

## Non-goals

- `serde_json/preserve_order` feature unification の影響範囲の
  棚卸し・根本原因の ruviform 側是正は対象外。
- `LibraryAction` 以外の enum・構造体のサイズ最適化は対象外。
- `*-msrv` 系タスク・`*-full` 系・3OS 横断検証は対象外
  （Daily Full CI 委譲）。
- `docs/**/README.md` 管理 index の再生成は対象外
  （Coordinator が統合時に実施）。

## Risk

- この保証が崩れると、Full CI の `lint-app-native` が再び
  失敗し `verify (ubuntu-latest)`・`evidence`・`ci-summary`
  の fail-closed gate が対象 SHA の統合証拠をブロックし続ける。
  wire form が変わった場合は `syn.library.action` op の
  JSON payload 互換が壊れ、UI/driver 側の action 送信が
  deserialize error になる。
