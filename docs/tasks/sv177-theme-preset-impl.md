---
id: "TASK-034T7IgIHI7umPL7xCvU3u"
type: "task"
status: "done"
title: "SV177 theme crate + preset module implementation"
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
    - "docs/plans/sv177-product-theme-workspace-presets.md"
  implemented-by:
    - "docs/impl/ir/sv177-theme-preset-implementation-record.md"
  depends-on:
    - "docs/plans/sv177-product-theme-workspace-presets.md"
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
    - "docs/test-specs/sv177-theme-preset-contract.md"
  references: []
  defers: []
  deferred-by: []
---

# SV177 theme crate + preset module implementation

## Work

`synvelis-theme` leaf crate（DTCG 文書 parse/validate/catalog/
selection/prefs + RF385 seam）と `synvelis-workspace::preset`
（schema2 slot preset・validate・from_snapshot/legacy import・
apply_preset）の実装。5 theme asset + 3 preset asset + contract
テスト + headless-deps gate 登録を含む。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/sv177-product-theme-workspace-presets.md`
- `docs/designs/sv177-product-theme-workspace-preset-design.md`
- `docs/specs/sv-gpui-03-product-theme-workspace-presets.md`
- `docs/adr/sv177-product-theme-crate-boundary-dtcg-document-envelope-and-slot-based-workspa.md`

## Dependencies

- `ruviform-ui` rev `49de02c` の中立 DockNode/LayoutSnapshot/MonitorInfo
  契約（ResolvedTheme 未公開のため RF385 seam のみ）。

## Affected Files

- `crates/synvelis-theme/**`（新規）
- `crates/synvelis-workspace/src/preset.rs`（新規）・`workspace.rs`・`lib.rs`
- `assets/themes/*.dtcg.json`・`assets/workspaces/*.json`（新規）
- `tools/check-headless-deps.mjs`・root `Cargo.toml`

## Done When

- [x] theme 文書が envelope/schema/type/value/alias/cycle を fail-closed 検証
- [x] catalog が layered `documents_for` + duplicate 通知を提供
- [x] `UnresolvedThemeResolver` が明示 Unsupported を返す
- [x] preset が破壊前検証付きで apply され link/focus/detached を復元
- [x] `ViewType::PluginDock` 語彙追加・`PresetViewState` 部分集合
- [x] 全 contract テスト pass・clippy 0・headless gate 登録

## Verification

- [x] `cargo test -p synvelis-theme`（11 pass）
- [x] `cargo test -p synvelis-workspace`（preset 15 + 既存全 pass）
- [x] `cargo clippy`（warning 0）・`cargo fmt --all`
- [x] `node tools/check-headless-deps.mjs`（9 crates OK）
- [ ] workspace 全体 `mise run check`/`test` 最終確認（統合前）
- [ ] Native 受入: NotRun（RF390/RF313/RF314+SV114 gate）
