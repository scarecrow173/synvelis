---
id: "PLAN-034T7GApSe3z9XUatjz8ti"
type: "plan"
status: "approved"
title: "SV177 Product Theme / Workspace Presets — synvelis-theme crate・DTCG assets・slot-based preset apply"
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
    - "docs/designs/sv177-product-theme-workspace-preset-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/sv-gpui-03-product-theme-workspace-presets.md"
    - "docs/adr/sv177-product-theme-crate-boundary-dtcg-document-envelope-and-slot-based-workspa.md"
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
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV177 Product Theme / Workspace Presets — synvelis-theme crate・DTCG assets・slot-based preset apply Implementation Plan

## Goal

SV177 の製品 Theme（DTCG 文書 + catalog + selection/prefs）と
Workspace Preset（slot 形式 template + apply/migration）を、
GUI/OS/renderer 非依存の headless 契約として実装する。

## Scope

単一 implementation stream。2 系統（theme crate / workspace preset
module）は相互依存せず同一設計に従うため同一 plan で扱う。
RF385 Token Interpreter/ResolvedTheme・GPUI renderer・native
受入・plugin dock host 実装は本 scope 外（上流 Issue / SV176 所有）。

## Upstream Documents

- `docs/specs/sv-gpui-03-product-theme-workspace-presets.md`
- `docs/adr/sv177-product-theme-crate-boundary-dtcg-document-envelope-and-slot-based-workspa.md`

## Design Inputs

- `docs/designs/sv177-product-theme-workspace-preset-design.md`（approved）

## File Map

- `crates/synvelis-theme/src/{error,document,validate,catalog,resolve,lib}.rs` — theme leaf crate
- `crates/synvelis-theme/Cargo.toml` — leaf 依存（serde/serde_json のみ）
- `crates/synvelis-theme/tests/theme_contract.rs` + `tests/fixtures/*.json` — contract tests
- `assets/themes/synvelis-{base,dark,light,density-compact,density-comfortable}.dtcg.json` — DTCG override 文書
- `crates/synvelis-workspace/src/preset.rs` — preset schema/validate/from_snapshot/apply
- `crates/synvelis-workspace/src/workspace.rs` — `ViewType::PluginDock`・`Preset` error・pub(crate) field 公開
- `crates/synvelis-workspace/src/lib.rs` — preset 型 export
- `crates/synvelis-workspace/tests/preset.rs` — preset tests
- `assets/workspaces/synvelis.{editing,mixing,plugin-dock}.json` — preset assets
- `tools/check-headless-deps.mjs` — `synvelis-theme` を HEADLESS_CRATES へ追加
- `Cargo.toml`（root）— workspace member 登録

## Implementation Sequence

- [x] `synvelis-theme` crate 骨格（error/document/validate/catalog/resolve）
- [x] DTCG validator（既知 `$type`・value shape・alias 構文・同一文書循環）
- [x] ThemeCatalog（layered documents_for・duplicate 検出）+ ThemePreferences
- [x] RF385 seam（`ResolvedThemeSink` + `UnresolvedThemeResolver` → Unsupported）
- [x] 5 DTCG asset 文書（dark は既存 style 値を literal 化・light は provisional 明記）
- [x] `ViewType::PluginDock` 語彙追加 + `WorkspaceError::Preset`
- [x] `preset.rs`（schema2 slot 型・validate・from_snapshot・legacy import）
- [x] `apply_preset`（bind→LayoutSnapshot 合成→`restore` 前検証→全置換→link/focus 復元）
- [x] `PresetViewState` 部分集合（session/entity 状態の排除）
- [x] 3 workspace preset asset
- [x] theme_contract / preset テスト + fixtures
- [x] review findings の修正（破壊前 validate・headless gate 登録・doc 同期）

## Dependencies and Risks

- RF385 `ResolvedTheme`/interpreter 未着地 → `UnresolvedThemeResolver`
  が明示 `Unsupported`。着地時に sink 実装で接続（第 2 interpreter 不作）。
- preset apply は破壊的前に全構造検証済みだが、mutation 開始後の
  host/dock 失敗は部分的 layout 残置の可能性あり（doc 明記済み、
  view/dock 不変は維持）。
- legacy import は現行 `WorkspaceSnapshot`（schema2）形状のみ受理。
  真の schema1 ファイル移行は既知 limitation。

## Task Breakdown

- TASK: theme 系（crate + validator + catalog + assets + tests）
- TASK: preset 系（schema + apply + assets + tests）
- TASK: gate/docs 整合（headless-deps・design/spec 同期・IR）
- 実装は単一 worker が inline 実施済み（本 plan は後付け canonical 化。

## Verification

- [x] `cargo test -p synvelis-theme` — 11 tests pass
- [x] `cargo test -p synvelis-workspace` — preset 15 + 既存 suite 全 pass
- [x] `cargo clippy -p synvelis-theme -p synvelis-workspace --all-targets` — warning 0
- [x] `node tools/check-headless-deps.mjs` — 9 headless crates, no GUI deps
- [x] `cargo fmt --all` 適用済み
- [ ] `mise run check` / `mise run test` workspace 全体最終確認（統合前）
- [ ] Native 受入（GPUI 実 Window/実 plugin editor/OS gate）: **NotRun**
  — RF390/RF313/RF314 + SV114 後続 Gate

## Open Questions / Gaps

- RF385 公開契約（DTCG profile・ResolvedTheme 型・文書 qualifier 仕様）
  着地時に `synvelis-theme` の profile/seam を再照合。
- Light theme token 値は provisional（native design review 未実施）。
- Preset apply の atomicity 強化（2-phase window 再構築）は将来改善候補。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（実施済み）
- Independent work streams: none
- User approval required before dispatch: n/a（worker 既実行）

## Review Handoff

- 重点確認: (1) apply_preset の破壊前 `LayoutSnapshot::restore` 検証、
  (2) `PresetViewState` が session/entity 状態を持ち込まないこと、
  (3) `UnresolvedThemeResolver` が第 2 interpreter でないこと、
  (4) dark/light asset の provenance 記述。
