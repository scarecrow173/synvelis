---
id: "TSPEC-034T7Ieio7KRMC297yqbIs"
type: "test-spec"
status: "approved"
title: "SV177 theme contract + workspace preset headless verification"
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
    - "docs/designs/sv177-product-theme-workspace-preset-design.md"
  derived-by: []
  verifies:
    - "docs/designs/sv177-product-theme-workspace-preset-design.md"
    - "docs/specs/sv-gpui-03-product-theme-workspace-presets.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV177 theme contract + workspace preset headless verification

## Purpose

SV177 の 2 系統（`synvelis-theme` DTCG 文書契約・`synvelis-workspace`
preset 契約）が GUI/OS/renderer 非依存の headless で検証可能で、
既存 Workspace の View identity・LinkGroup・Focus・Project schema の
意味を侵さないことを固定する。RF385 着地・native driver 導入後も
同契約が維持されるまで retire しない。

## Feature

Driver-neutral product theme documents and slot-based workspace presets.

## Rules

- テーマ文書は `$extensions["synvelis.productTheme"]` envelope を必須と
  し、`schema == SV_THEME_SCHEMA`・非空 theme/mode/density・
  `layer="product"` のみ受理（envelope 系のみ fail-closed）。
- 出荷文書は `$extensions["dev.ruviform.dtcg-profile"]`
  （`profile="rvf-dtcg-profile.v1"`・`layer="product"`・一意 `id`・
  mode 文書は `modes` 宣言）を併記し、上流
  `TokenDocument::from_json` を通過する。
- 両 envelope の coherence は SV の責務: upstream `layer` は
  productTheme `layer` と一致、upstream `modes` は SV 宣言 `mode`
  を包含、mode+density 併記は不可達形として `Contract` problem。
  upstream block 内部（`profile` 値・未知 layer 名）の検証は
  上流に委譲し再実装しない。
- token 意味論（`$type` 集合・value shape・alias・循環・merge）は
  `rvf-dtcg-profile.v1` が唯一の正本。SV 側は再検証しない —
  `delegated-*` fixtures が consumer level で load できることが
  境界の証跡。同一文書循環の事前拒否は撤廃（後段 override で
  合法な集合を誤拒否する P1 の修正 — override で消える cycle が
  resolve できることを回帰固定）。
- `dimension`/`duration` は typed `{value, unit}` object 形のみ
  （CSS 文字列 `"22px"`/`"100ms"` は上流 `InvalidValue`）。
- Catalog は (theme,mode,density) で `ThemeDocument`（原文 JSON
  保持）を index し、`documents_for` が base→mode→density を返す。
  merge しない。duplicate key は `insert` の戻り値で通知。
- `catalog.resolve(sel)` / `resolve_documents` は文書の原文を
  `TokenDocument::from_json` → `ThemeResolver::resolve` へ渡し、
  `ResolvedTheme`（`rvf-dtcg-profile.v1`）を返す。上流失敗は
  `ThemeError::Resolve`（`Document(TokenError)` または merge 系
  `ResolveError`）に包まれる。第 2 interpreter 不許可。
- catalog の declared vocabulary が selection を bound する:
  theme が mode（density）文書を持つ場合、未宣言 mode/density の
  resolve は `ThemeError::Selection` で失敗（部分 layer 集合を
  黙って返さない。mode 文書を持たない theme は mode-agnostic の
  まま）。
- Preset は `PRESET_SCHEMA=2` の slot 形式。`layout` key 有りの
  `WorkspaceSnapshot` 形状 JSON は legacy として `from_snapshot`
  経路で import。
- `apply_preset` は破壊的操作（既存 window close）より前に
  `LayoutSnapshot::restore` で全構造を検証し、失敗時は state 無変更。
- slot binding は `ViewType` 一致で既存 view を registry 順 adopt、
  不足は spawn、preset state は spawned にのみ適用。未 bound 既存
  view は detached pool に残る。
- Link group（group >= 1）と focus（同 window 内 docked view）を
  復元。Project revision・session state を変更しない。
- `PresetViewState` は tool/grid/snap/inspection_mode/viewport のみで
  実 ViewInstanceId・gesture・entity 参照を含まない。

## Examples

- 出荷 5 文書（base/dark/light/compact/comfortable）が SV envelope
  validation と上流 `TokenDocument::from_json` を通過し、dark/light
  × compact/comfortable 全 4 組合せで `catalog.resolve` が
  `ResolvedTheme` を返す（3 product sources・mode 別背景・density
  別 `control.height` 22/28px・base `motion.fast` = 100ms）。
- `invalid-schema` fixture は `Schema`、envelope 不在/空 theme/
  空 mode/誤 layer は `MissingMeta`、両 envelope 矛盾（layer
  不一致・modes 不包含・mode+density 併記）は `Contract` を返す。
- `delegated-{type,value,ref,cycle}` fixtures は consumer level で
  load される（token 意味論は非検査）。adapter 経路では各名前の
  上流 rejection が表面化する: `UnknownTokenType`/`InvalidValue`/
  `AliasCycle`。RF profile 不在文書は `Document(MissingProfile)`。
- 未宣言 mode/density（例: `sepia`/`huge`）の `catalog.resolve` は
  `ThemeError::Selection`。alias fixture は実 resolver で
  `{color.fg}` が literal へ解決される。
- editing/mixing/plugin-dock preset が FakeWindowHost + MonitorInfo
  で apply され、adopted view の state が保持される。
- duplicate dock slot・ratio 範囲外・focus が他 window の view・
  link group 0 は apply 前に `Preset` error で弾かれ既存 window 無傷。
- monitor missing の geometry は `correct_bounds` fallback で調整。

## Guarantees

- GPUI/DOM/wry/Toolkit/OS handle 型が theme・workspace の通常 API に
  露出しない（`check-headless-deps.mjs` が 9 headless crates を監査）。
- 既存 `WorkspaceManager` テスト群が無回帰（全 suite pass）。

## Non-goals

- 実 rendering・pixel 比較・native window 動作（RF390/RF313/RF314/
  SV114 の native gate、**NotRun**）。
- plugin dock の host/native surface 実体（SV176 所有。`PluginDock`
  は語彙のみ）。
- 真の schema-1 時代 preset ファイルの移行（既知 limitation）。

## Risk

- 本 spec を外すと、未検証 layout の破壊的適用・session state の
  template 混入・独自 token interpreter の重複実装・headless 境界への
  GUI 依存混入を再導入し得る。
