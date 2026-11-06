---
id: "IMPL-034T7IhBoewhoZNaZfjXP1"
type: "impl"
status: "completed"
title: "SV177 theme/preset implementation record"
created: "2026-11-06"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/177"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/sv177-theme-preset-impl.md"
    - "docs/plans/sv177-product-theme-workspace-presets.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/sv177-product-theme-workspace-inventory.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sv177-product-theme-workspace-preset-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv177-theme-preset-contract.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV177 theme/preset implementation record

## Summary

`synvelis-theme` leaf crate（DTCG 文書契約）と
`synvelis-workspace::preset`（slot-based preset）を headless
契約として実装。5 theme asset・3 preset asset・26 contract
テスト・headless-deps gate 登録を含む。独立 Reviewer A/B →
Validator → Judge の review-protocol で確認された blocking
findings（破壊前検証欠落・session state 混入・gate 未登録等）を
全て修正済み。

## Context

Task `docs/tasks/sv177-theme-preset-impl.md`、plan
`docs/plans/sv177-product-theme-workspace-presets.md`、design
`docs/designs/sv177-product-theme-workspace-preset-design.md`
（approved）に従う。上流契約 RF385 `ResolvedTheme` 未着地のため、
`UnresolvedThemeResolver` seam で明示 Unsupported とし、第 2
interpreter/renderer は実装しない。

## Implementation

### Changes

- `crates/synvelis-theme`: `error.rs`（ThemeError/ProblemKind）・
  `document.rs`（envelope+TokenTree parse）・`validate.rs`（known
  `$type`・value shape・alias 構文・同一文書循環）・`catalog.rs`
  （`ThemeKey`・layered `documents_for`・`ThemePreferences`）・
  `resolve.rs`（`ResolvedThemeSink`/`UnresolvedThemeResolver`・
  `style_hint`）。
- `assets/themes/`: base/dark/light/density-compact/comfortable の
  5 DTCG 文書。dark は既存 style 値を literal 化（alias 不使用で
  self-contained）、light は `provisional-pending-native-design-review`
  provenance 付き。
- `crates/synvelis-workspace`: `preset.rs` 新規（PRESET_SCHEMA=2
  slot 型・`PresetViewState` 部分集合・validate・from_snapshot・
  legacy import・`apply_preset`）。`ViewType::PluginDock` 語彙追加・
  `WorkspaceError::Preset`・必要 field の `pub(crate)` 化。
- `assets/workspaces/`: editing/mixing/plugin-dock の 3 preset。
- `tools/check-headless-deps.mjs`: `synvelis-theme` を HEADLESS_CRATES
  に追加（9 crates）。

### Decisions During Implementation

- `apply_preset` は `LayoutSnapshot::restore`（Result 版）を window
  close 前に実行。`restore_or_default` は検証失敗を黙って default
  化するため採用しない（Reviewer B1 修正）。
- preset state は `PresetViewState`（tool/grid/snap/inspection_mode/
  viewport）に限定。実 ViewInstanceId・EditBinding・pinned
  SemanticTarget・gesture・revision_inbox を template に持ち込ま
  ない（Reviewer B2/A6 修正）。
- catalog は merge 済み値ではなく `TokenTree` を保持し
  `documents_for` で層集合を返す。merge/interp は RF385 の責務
  （Reviewer B4/A9 修正）。
- dark theme は alias ではなく literal 値で出荷（cross-doc alias
  解決は未規格のため self-contained に）。
- alias existence は検査対象外（文書 qualifier 不在で cross-doc ref
  と区別不能）。`{` 始まり malformed のみ `BadRef`。

## Related Experiments

### Adopted

- なし（直接実装）。

### Rejected

- `restore_or_default` による fallback 経路（検証を黙殺）。
- 既存 view への preset state 上書き適用（session 意味を壊す）。

## Validation

- `cargo test -p synvelis-theme`: 11 pass / 0 fail。
- `cargo test -p synvelis-workspace`: preset 15 含む全 suite pass /
  0 fail（既存 workspace/project/app 群も無回帰）。
- `cargo clippy -p synvelis-theme -p synvelis-workspace
  --all-targets`: warning 0。
- `cargo fmt --all` 適用済み。
- `node tools/check-headless-deps.mjs`: 9 headless crates、GUI
  framework deps なし。

## Risks

- RF385 着地時に DTCG profile/envelope/seam の再照合が必要。
- preset apply は破壊前検証済みだが、mutation 開始後の host/dock
  失敗では部分的 layout 残置があり得る（view/dock 不変は維持）。
- 真の schema-1 preset ファイル移行は既知 limitation（現行
  `WorkspaceSnapshot` 形状のみ legacy import 受理）。

## Follow-ups

- workspace 全体 `mise run check`/`test` の最終確認（統合前）。
- Native 受入（実 GPUI window/実 plugin editor/実 OS）: **NotRun**。
  RF390/RF313/RF314 + SV114 gate 後に実施。
- RF385 `ResolvedTheme` 公開後に `ResolvedThemeSink` 実装 adapter を
  追加し `ThemePreferences`/catalog を renderer に接続。
