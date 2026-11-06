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
  し、`schema == SV_THEME_SCHEMA`・既知 mode/density/layer のみ受理。
- `$type` は `color/dimension/fontFamily/fontWeight/duration/
  cubicBezier/number` の subset のみ。未知型・不整合 value shape・
  malformed alias・同一文書内 alias 循環は fail-closed。
- alias は `{a.b.c}` 単一 brace 形式。`{` 始まりの malformed は
  `BadRef`。`"Fira {Code}"` のように途中の brace は literal。
  alias target existence・cross-document 解決は検査しない（RF385 責務）。
- Catalog は (theme,mode,density) で layer 文書を保持し、
  `documents_for` が base→mode→density を返す。merge しない。
  duplicate key は `insert` の戻り値で通知。
- `ResolvedThemeSink` 実装は `UnresolvedThemeResolver` のみで、
  常に `ThemeError::Unsupported` を返す（第 2 interpreter 不許可）。
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

- 出荷 5 文書（base/dark/light/compact/comfortable）が validation を
  通過し、dark selection で base+dark+compact 層が返る。
- `invalid-{type,value,ref,cycle,schema}` fixtures がそれぞれ
  `UnknownType`/`BadValue`/`BadRef`/`CyclicRef`/`Schema` を返す。
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
