---
id: "DESIGN-034T728YcMrGoBwDtbkIqS"
type: "design"
status: "approved"
title: "SV177 Product Theme / Workspace Preset — synvelis-theme crate・DTCG overrides・slot-based preset・semantic screen spec"
created: "2026-11-07"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/177"
    - "https://github.com/scarecrow173/ruviform/issues/384"
    - "https://github.com/scarecrow173/ruviform/issues/385"
    - "https://github.com/scarecrow173/ruviform/issues/387"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv-gpui-03-product-theme-workspace-presets.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gpui-03-product-theme-workspace-presets.md"
    - "docs/adr/sv177-product-theme-crate-boundary-dtcg-document-envelope-and-slot-based-workspa.md"
    - "docs/discovery/sv177-product-theme-workspace-inventory.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV177 Product Theme / Workspace Preset — synvelis-theme crate・DTCG overrides・slot-based preset・semantic screen spec

## Context

SV177 は Synvelis 固有の製品 Theme（DTCG 製品 override 文書・選択
状態・消費 contract）と Workspace Preset（名前付き layout
template）を、Driver-neutral に headless で検証可能な形で実装する。
上流は spec `sv-gpui-03-product-theme-workspace-presets` と ADR
`sv177-product-theme-crate-boundary-…`（Option A/D/F/H 採用）。

依存方向（本設計で追加・維持するもの）:

```text
synvelis-theme  (leaf: serde/serde_json のみ)
synvelis-workspace → synvelis-project, ruviform-ui(中立契約), serde
   └─ preset module が ruviform LayoutSnapshot/DockTree を消費
```

`ResolvedTheme`・Token Interpreter・GPUI Style Mapper は ruviform
#385 の所有で本設計の対象外。本設計はその**消費点**（入力文書・
選択状態・adapter）を Synvelis 側に固定する。RF385 candidate
（`feature/gui-driver-neutral-design-contract`）着地に伴い、
消費点は `UnresolvedThemeResolver` stub から実 adapter
（`adapter::resolve_documents`）へ更新済み。

## Scope

### In Scope

- `crates/synvelis-theme/`: 文書型・validation・catalog・選択・seam・
  error の 5 モジュール。
- `assets/themes/` 5 文書（base + dark + light + density×2）。
- `crates/synvelis-workspace/src/preset.rs` + `view.rs` の
  `PluginDock` variant + `assets/workspaces/` 3 preset。
- 各 fixture + headless テスト。
- semantic screen spec（本書末尾）。

### Out of Scope

- alias/mode merge・ResolvedTheme 生成・GPUI 描画・実 OS window・
  plugin dock 実装本体・Figma 連携・旧 frontend 削除・AudioLens 変更。

## Components and Boundaries

### `synvelis-theme` crate（新規 leaf）

依存は workspace 共有の `serde` + `serde_json` + `ruviform-ui-theme`
（serde/serde_json のみの interpreter、GUI 依存なし）。GUI/Tooling/
OS 型を一切持たず、`check-headless-deps` 対象として安全。

| Module | 責務 |
| --- | --- |
| `error.rs` | `ThemeError`: `Schema`/`Validation(Vec<TokenProblem>)`/`Io`/`Unsupported`/`Resolve(ruviform_ui_theme::ResolveError)`。`TokenProblem { path, kind: ProblemKind, detail }`、kind = `MissingMeta`/`Schema`（envelope 系のみ — token 系 kind は上流に委譲され消滅）。 |
| `document.rs` | 生 DTCG 文書の serde 型と envelope。`TokenTree`（root から `$extensions` を除いた opaque 木）+ `ProductMeta { schema, theme, mode, density, layer }`（`$extensions["synvelis.productTheme"]` から読む）+ `ThemeDocument { meta, tree, source }`（**原文 JSON を保持** — 上流 `TokenDocument::from_json` の入力）。`parse_document(json) -> Result<(ProductMeta, TokenTree), ThemeError>`。 |
| `validate.rs` | `validate_document(meta) -> Vec<TokenProblem>`: envelope 系のみ — schema=`SV_THEME_SCHEMA`・非空 theme/mode/density・`layer="product"`。token 意味論（`$type` 集合・value shape・alias・循環）は**再検証しない**（上流責務 — 同一文書循環拒否が後段 override で合法な集合を誤拒否する P1 の根因）。`load_document`/`load_dir` は `ThemeDocument` を返し fail-closed。 |
| `catalog.rs` | `ThemeCatalog { themes: BTreeMap<ThemeKey, ThemeDocument> }`（ThemeKey = (theme, mode, density)）+ `ThemeSelection`、`ThemePreferences { schema, selection }`（`PREFS_SCHEMA`、save/load roundtrip）。`documents_for(&self, sel) -> Vec<&ThemeDocument>` — selection を base→mode→density の文書層 set に展開（adapter が消費する入力）。`insert` は duplicate key を `Option` で通知（silent overwrite しない）。`resolve(&sel)` は adapter への委譲。 |
| `adapter.rs` | `resolve_documents(&[&ThemeDocument], mode) -> Result<ResolvedTheme, ThemeError>` — catalog 選択文書の**原文 source** を `TokenDocument::from_json`（上流 strict parser）へ渡し、`ThemeResolver::resolve` を呼ぶ唯一の接続点。token 意味論は一切解釈せず、上流 `TokenError`/`ResolveError` を `ThemeError::Resolve` に包む。`ThemeSelection::style_hint() -> String`（`"{theme}/{mode}/{density}"` 決定的形式、`VizStyleInput.theme` へ渡せる文字列）は catalog 側に存置。 |

#### DTCG 文書形式（RF385 着地形 — 両 envelope 併記）

出荷文書は **製品 extension と共通 profile の両方**を
`$extensions` に併記する。Synvelis envelope は catalog/選択の
metadata、`dev.ruviform.dtcg-profile` は上流 interpreter の
document identity であり、双方の責務は重複しない。

```json
{
  "$extensions": {
    "dev.ruviform.dtcg-profile": {
      "profile": "rvf-dtcg-profile.v1",
      "layer": "product",
      "id": "synvelis-dark",
      "modes": ["dark"]
    },
    "synvelis.productTheme": {
      "schema": 1,
      "theme": "synvelis",
      "mode": "dark",
      "layer": "product"
    }
  },
  "color": {
    "bg":        { "$type": "color", "$value": "#1b1d21" },
    "panel-bg":  { "$type": "color", "$value": "{color.bg}" }
  }
}
```

- `dev.ruviform.dtcg-profile`: 必須（上流 `TokenDocument::from_json`
  が `MissingProfile` で拒否）。`profile` は `rvf-dtcg-profile.v1`
  固定、SV 文書は全て `layer="product"`（同一 layer 内は渡し順で
  後者優先 = catalog の base→mode→density 順）、`id` は
  `(layer, id)` 一意。`modes` は mode 文書のみ宣言（`["dark"]`/
  `["light"]`）— 非宣言 doc は mode-agnostic（base/density）。
- `synvelis.productTheme`: 必須（SV envelope。`schema` は
  `SV_THEME_SCHEMA = 1` と一致必須）。theme/mode/density/layer は
  catalog 索引（`ThemeKey`）の metadata。
- token 意味論は **`rvf-dtcg-profile.v1` が唯一の正本**（SV 側で
  再検証しない）: 採用 `$type` = `color/dimension/fontFamily/
  fontWeight/number/duration/cubicBezier/strokeStyle`。
  `dimension`/`duration` は `{value, unit}` object 形のみ
  （`"22px"`/`"100ms"` の CSS 文字列は `InvalidValue` — 出荷資産は
  typed 形へ移行済み）。alias は値全体 `"{path.to.token}"` のみ
  （部分埋め込み・brace 混入文字列は `InvalidValue`）。`$` 未知
  key は `ReservedKey`。merge は `primitive<semantic<component<
  product` 順・同 path 同型 override のみ（`TypeMismatch` 厳格）、
  alias は merge 後集合で解決（`UnknownReference`/`AliasCycle` は
  merge 後に残った実害のみ — SV 側の単文書事前循環拒否は撤廃）。

#### `assets/themes/` 5 文書

| File | meta | 内容 |
| --- | --- | --- |
| `synvelis-base.dtcg.json` | layer=product, mode/density 非指定（base）| font family・radius・brand 色など mode/density 非依存の製品 token |
| `synvelis-dark.dtcg.json` | mode=dark | `styles.css` 実測 8 色由来: bg `#1b1d21` bg-panel `#23262c` fg `#e8e8ea` fg-dim `#9aa0a8` accent `#6aa9e8` warn `#f0a868` err `#e86a6a` ok `#6ac88a` + panel/selected/hover 等の semantic 派生 |
| `synvelis-light.dtcg.json` | mode=light | **provisional**（`$extensions.synvelis.provenance: "provisional-pending-native-design-review"` を明記）— dark から反転導出の暫定候補 |
| `synvelis-density-compact.dtcg.json` | density=compact | spacing/control-height/font-size の compact 値 |
| `synvelis-density-comfortable.dtcg.json` | density=comfortable | 同 default 値 |

base/dark/light/density は layer 構造:(theme,mode,density) 選択は
`documents_for` で base→mode→density の文書 set に展開され、
その set が RF385 interpreter seam の入力となる。merge 意味論・
override precedence は実装しない（RF385 の責務）。カタログは各
entry の `TokenTree` を保持するため、文書 set を sink へそのまま
渡せる。

### `synvelis-workspace::preset` module

```rust
pub const PRESET_SCHEMA: u32 = 2;            // v1 = legacy snapshot 形状
pub const PRESET_SCHEMA_LEGACY: u32 = 1;

pub struct WorkspacePreset {
    pub schema: u32,                 // == PRESET_SCHEMA
    pub id: String,                  // "synvelis.editing" 等
    pub name: String,                // display
    #[serde(default)] pub windows: Vec<PresetWindow>,
    #[serde(default)] pub detached: Vec<u32>,          // slot ids
    #[serde(default)] pub views: Vec<PresetView>,       // slot 定義
    #[serde(default)] pub links: Vec<PresetLink>,
    #[serde(default)] pub focus: Option<PresetFocus>,
}
pub struct PresetView { pub slot: u32, pub kind: ViewType,
    pub title: String, #[serde(default)] pub state: Option<PresetViewState> }
/// テンプレ適切な ViewState 部分集合: active_tool / grid / snap /
/// inspection_mode / viewport。session・entity 状態（選択・
/// EditBinding・pinned SemanticTarget・linked_context(実
/// ViewInstanceId)・gesture・revision_inbox）は template に
/// 持ち込まない — pin は InspectionMode のみ保持し、pinned entity
/// は session-scoped として除外。
pub struct PresetViewState { active_tool, grid, snap,
    inspection_mode, viewport } // ViewState ⇄ 双方向変換あり
pub struct PresetWindow { pub title: String,
    #[serde(default)] pub geometry: Option<PresetGeometry>,  // ruviform WindowGeometry は serde 非対応のため独自型
    pub dock: PresetDock }
pub struct PresetGeometry { x, y, width, height,
    monitor: Option<MonitorHint> }
pub enum PresetDock { Tabs { views: Vec<u32>, active: usize },
                      Split { axis: Axis, ratio: f64, children: Vec<PresetDock> } }
pub struct PresetLink { pub view: u32, pub group: u64 }  // group >= 1
pub struct PresetFocus { pub window: usize, pub view: Option<u32> }
```

- `WorkspacePreset::from_json`: top-level `layout` key の有無で
  legacy（`WorkspaceSnapshot` 形状）を判定 → `from_snapshot` 経路。
  それ以外は v2 slot 形式を parse し `schema == PRESET_SCHEMA` を
  確認。legacy 判定は現行 `WORKSPACE_SCHEMA=2` 形状のみ受理する
  （schema-1 時代の `editing_selection` 形は `ViewState` の
  `deny_unknown_fields` で弾かれる — 真の v1 ファイル移行は
  本 scope 外で既知 limitation）。
- `WorkspacePreset::from_snapshot(&WorkspaceSnapshot, id, name)`:
  capture 済 snapshot → v2 preset（capture→preset 経路かつ legacy
  import 経路）。`ViewState` → `PresetViewState` へ射影。
- `validate()`: windows 非空・slot 一意・全参照 slot が `views` に
  存在・docked slot の全体一意・docked∩detached 非交差・nested
  empty Tabs 禁止・link group != 0・`focus.view` が
  `windows[focus.window]` に dock されていること。DockNode 構造
  不変（ratio・children 数・view 重複）は apply 内の
  `LayoutSnapshot::restore`（Result 版）が破壊前に再検証。
- `WorkspaceManager::apply_preset(&mut self, preset, monitors)
  -> Result<PresetReport, WorkspaceError>`:
  1. `preset.validate()` — fail なら state 無変更で error。
  2. slot→view binding: preset view 宣言順に、未 bound な既存 view
     を `kind` 一致で registry-id 順に adopt。残余 slot は
     `spawn_detached_view`（preset state は spawned にのみ適用 —
     adopted は既存 state を無条件保持）。
  3. preset windows → `WindowSnapshot`（dock tree を slot→id 変換し
     `DockNode` 化）→ `LayoutSnapshot` 合成 → **`restore(monitors)`
     を破壊的操作の前に実行**（schema・zero-size・tree 構造・view
     重複をここで明示 error。monitor 照合・off-screen 補正は
     `correct_bounds` 内で fail-safe 済）。
  4. 既存全 window を `host.destroy_window` + `Destroyed` event で
     close（DetachViews → 全 view は detached pool へ。`veto_close`
     は bypass — preset apply は full-layout replace のユーザー操作。
     view と state は破棄しない）。
  5. dock 対象の bound view を detached pool から `remove_view` で
     free 化（`apply_restored_tree` の assert_free 要件）。
  6. 各 placement を `create_window_at` + `apply_restored_tree`。
  7. bound view の home 無し → `add_detached`（safety net）。
     links: bound view を reset 後 preset 適用。focus: host
     `focus(window, true)` + `activate_view`（validate が window
     内 dock を保証）。
  8. `PresetReport { windows, adopted, spawned, detached, focused }`
     を返す。mutation 開始後の host/dock 失敗は部分的適用を残し
     得る — dock 不変条件は各 step で維持され home 無し view は
     re-pool されるが、window 配置は不完全になり得る（doc 明記）。
- `ViewType` に `PluginDock` variant を追加（serde kebab-case
  `"plugin-dock"`。host/native surface 実装は SV176 所有。本変更は
  語彙追加のみで互換破壊なし — 旧文書に同文字列は存在しない）。

### `assets/workspaces/` 3 preset（v2 JSON）

| File | 構成 |
| --- | --- |
| `synvelis.editing.json` | 1 window: Arrangement + Inspector tabs、links: inspector←arrangement follow |
| `synvelis.mixing.json` | 2 windows: win1 Mixer dock、win2 Inspector+SignalFlow split、links |
| `synvelis.plugin-dock.json` | 1 window: Arrangement + PluginDock tabs（kind=plugin-dock、SV176 実装待ちの語彙）|

## Data and Control Flow

```text
[theme]  assets/themes/*.dtcg.json
  → document::parse (envelope + token tree)
  → validate (envelope: schema/必須 field/layer tag)
  → ThemeCatalog  (theme,mode,density) index — ThemeDocument(原文保持)
  → ThemeSelection 選択 → ThemePreferences save/load (独自 schema)
  → adapter::resolve_documents → TokenDocument::from_json →
    ThemeResolver::resolve → ResolvedTheme (rvf-dtcg-profile.v1)
  → (別経路) sel.style_hint() → VizStyleInput.theme (repaint-only)

[preset] assets/workspaces/*.json
  → WorkspacePreset::from_json (schema/migrate)
  → apply_preset: slot→ViewInstanceId binding (adopt/spawn/detach)
  → LayoutSnapshot 合成 → restore_or_default(monitors)
  → create_window_at + apply_restored_tree + links + focus
  → PresetReport
```

不変条件: 両経路とも `ProjectSession`/revision/undo/(stub) RT を
一切触れない — theme は型レベルで `synvelis-project` に依存せず、
preset は `WorkspaceManager` の dock/view 操作のみ使用。

## Non-Functional Constraints

- 全て非 RT・headless・deterministic（BTreeMap ベースの索引）。
- 新規外部依存ゼロ（serde/serde_json のみ）。MSRV 1.89/edition 2024。
- `unsafe` 禁止（workspace lints 継承）。missing_docs warn に備えて
  全公開型に doc comment。
- `WorkspaceError`/`ThemeError` は既存のエラー命名規約に倣う。

## Risks and Trade-offs

- RF385 candidate branch（未 merge）へ一時接続中 → merge 前に
  branch が巻き戻る/削除されると参照が切れる → lock SHA・方法・
  解除条件を evidence と本書末尾に記録し、恒久 rev pin はしない。
- light theme 値は provisional — 確定は Native Design Review で
  行い、文書上 `$extensions.synvelis.provenance` で暫定を明示。
- `PluginDock` kind を先に語彙だけ入れる → SV176 側と認識差が出る
  可能性 → 本設計で「語彙のみ・実装は SV176」と明記。
- adopt-by-kind は同名 kind の複数 view がある場合に index で決定的
  （BTreeMap 順）だが、ユーザーが意図しない adopt があり得る →
  `PresetReport.adopted` で可視化し review 可能に。

## Semantic Screen Spec（Native Design Review 入力・文書 spec）

各画面の variant・状態・density 差分・a11y 要件を RF387 ViewSpec
着地までの**文書 spec** として列挙（実装は各 view owner の scope）:

- **Arrangement**: lanes（track header/clip/automation lane）、time
  ruler、selection/marquee、zoom/scroll、density=compact で lane
  height 縮小 + フォント small。状態: empty/loading/playing/
  recording/edit-gesture。a11y: clip の名前+位置+長さを text
  等価で提供、keyboard での clip 移動/focus 可視。
- **Mixer**: channel strip（fader/pan/mute/solo/send/level meter）、
  strip width は density 依存。状態: signal present/clip/muted/
  soloed/armed。a11y: fader value の text 等価・keyboard 微調・
  high-contrast focus ring。
- **Transport**: play/stop/rec/loop/locate + time display + tempo/
  signature。状態: stopped/playing/recording/punch-in・count-in。
  a11y: 全操作 keyboard/IME 不要で到達可・状態を text 等価。
- **Inspector**: target kind 別 panel（clip/track/plugin/
  automation-point）。Follow/Pinned は `InspectionMode` を保持。
  a11y: 全 property label 付き・readonly 区別。
- **Plugin Dock**: host container（plugin slot list・scan state・
  fallback panel）。native editor 収容部は **第三者 GUI であり
  製品 Theme を強制しない** — Theme が届くのは container 装飾
  （タイトルバー・slot 枠・フォールバック表示）まで。状態:
  empty/scanning/hosted/unavailable。a11y: plugin の代替操作
  （generic parameter list）への経路を提供。

## Verification Notes

- [ ] `mise run check`/`lint`/`test`（workspace 全体）pass
- [ ] `mise run check-headless-deps` pass — synvelis-theme に
  GUI/GPU/WebView 依存が無いこと
- [ ] fixture: 5 製品文書が SV envelope + upstream
  `TokenDocument::from_json` を両方通過し、dark/light ×
  compact/comfortable 全 4 組合せで `ThemeResolver::resolve` が
  `ResolvedTheme` を返す（実 RF parser/resolver 契約試験）
- [ ] preset: 3 asset apply・monitor missing・v1→v2 migration・
  snapshot→preset roundtrip・schema 不一致 error
- [ ] 不変条件: Theme 切替・Preset 適用で project revision/undo/
  engine 不変（テスト）
- [ ] Native 描画/a11y/pixel 比較: 後続 Gate へ **NotRun** 記録

## Deferred Design Concerns

- light theme 確定値 | 合意資料なし | Native Design Review |
  provisional が暫定外観として残る
- `ui/` SolidJS frontend の theme 配線 | 旧経路・新 Native 実証前 |
  GPUI 経路確定時 | 旧 CSS と token の二重定義が残る
- screen spec → RF387 ViewSpec 写像 | RF387 未着地 | RF387 着地時 |
  文書 spec が型と乖離する可能性

### RF385 接続（証跡は evidence/reports/sv177-rf385-adapter.md）

- 最終状態: ruviform **default branch 追従**（他の ruviform dep と
  同一方法、恒久 `rev` pin なし）。RF385 は PR #397 squash merge
  `df45d10cd86918d10fa81dc80a3f5f5f2564d5ae` で main 着地済み —
  `Cargo.lock` 全 24 sources が `df45d10` を解決。
- 経緯: landing 前は全 ruviform git 依存を
  `branch = "feature/gui-driver-neutral-design-contract"` 追従へ
  一時切替していた（`check-ruviform-rev` の単一 SHA 不変条件を保つ
  ため全 dep 同時）。main 着地確認後に全 `branch` key を除去し
  default 追従へ復帰 — 解除条件を消化済み。
