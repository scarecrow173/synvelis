---
id: "SPEC-034T6XTAVpIPhybzaw7jdh"
type: "spec"
status: "implemented"
title: "SV-GPUI-03 Synvelis Product Theme / Workspace Presets — Driver-neutral DTCG overrides・Preset schema・fixtures"
created: "2026-11-07"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/177"
    - "https://github.com/scarecrow173/ruviform/issues/384"
    - "https://github.com/scarecrow173/ruviform/issues/385"
    - "https://github.com/scarecrow173/ruviform/issues/387"
    - "https://github.com/scarecrow173/synvelis/issues/23"
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
  related:
    - "docs/adr/sv177-product-theme-crate-boundary-dtcg-document-envelope-and-slot-based-workspa.md"
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/sv177-product-theme-workspace-inventory.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV-GPUI-03 Synvelis Product Theme / Workspace Presets — Driver-neutral DTCG overrides・Preset schema・fixtures

## What Should Be Built

Issue #177（SV-GPUI-03・2026-11-07 横断レビュー確定仕様 v2）に基づき、
Synvelis 固有の **製品 Theme** と **Workspace Preset** を、
Driver-neutral な別 Schema/Version/Restore の 2 系統として実装する。

1. **製品 Theme 系**（`synvelis-theme` 新規 leaf crate + `assets/themes/`）:
   - Git 管理の DTCG（Design Tokens Community Group 2025.10 subset）
     製品 override 文書。Synvelis 固有の Primitive overrides・
     Semantic・Component Theme を、light/dark mode × comfortable/
     compact density の組合せで提供する。
   - 製品 envelope（`$extensions["synvelis.productTheme"]`）の
     validation — schema・非空 field・`layer` tag・両 envelope の
     coherence（upstream `layer`/`modes` との整合・mode+density
     併記の不可達形拒否）を行う loader。`$type`・value shape・
     alias 解釈・循環検出などの token 意味論は **SV 側で再検証
     しない**（Rev.1 は loader 内に持っていたが、単文書循環拒否が
     後段 override で合法な集合を誤拒否する P1 根因となったため
     Rev.2 で全撤去）。Token Interpreter（alias/mode merge →
     ResolvedTheme）は Ruviform #385 の責務であり**再実装しない**。
     RF385 は ruviform main（`df45d10`）着地済みで、実 adapter
     （`adapter::resolve_documents` → `TokenDocument::from_json` →
     `ThemeResolver::resolve`）を通じて解決する — Unsupported
     seam は撤去済み。
   - `ThemeSelection { theme, mode, density }` と versioned
     `ThemePreferences`（独自 schema・Project/WorkspaceSnapshot と
     無関係）を保持する consumer contract。GPUI・DOM・wry・
     Toolkit・OS handle 型は一切露出しない。
2. **Workspace Preset 系**（`synvelis-workspace::preset` +
   `assets/workspaces/`）:
   - 名前付き・versioned（`PRESET_SCHEMA`）の layout template。
     Window 配置（geometry + monitor signature）・Dock 構成
     （tabs/split 木）・View slot（kind/title/任意の初期
     `PresetViewState` — tool/grid/snap/inspection_mode/viewport の
     template 部分集合で `InspectionMode`（Follow/Pinned）を含む）・
     Link Group 割当・focus 指名を記述する。session 状態
     （実 ViewInstanceId・gesture・entity 参照・revision inbox）を
     含まない。
   - `apply_preset`: slot→実 ViewInstanceId を（既存 view を
     kind+index で adopt・不足は spawn・残余は detached pool へ）
     解決し、構造検証は `LayoutSnapshot::restore`（明示 error）
     で破壊前に行い、monitor missing / off-screen 補正はその内部の
     `correct_bounds` fail-safe を再利用する。adopt された view の
     state は無条件保持、preset state は spawned view にのみ適用。
   - v1（`WorkspaceSnapshot` 形状の旧形式）→v2（slot 形式）の
     versioned migration。`capture`→preset 変換で現行 layout を
     preset 化できる。
   - 付随して `ViewType::PluginDock` variant を追加する（surface
     kind の語彙のみ。plugin dock の host/native surface 実装は
     SV176 の所有であり本 spec の対象外）。
3. **Design review 文書**: Mixer/Transport/Arrangement/Inspector/
   Plugin Dock の semantic screen spec（variant・状態・density・
   a11y 要件の列挙）を design doc に含める。実描画・pixel 比較・
   Native Component Lab 受入は RF390/#313/#314 + SV114 の後続
   Gate であり本 spec では **NotRun** として記録する。

## Why Now

2026-11-07 横断レビュー確定仕様 v2（RF #384 / SV #23 冒頭）が
GUI lane を GPUI Native First + Driver-neutral に再編し、SV177 を
「Start 並行可」lane に配置した。製品 Theme/Workspace は両 Consumer
共通契約（RF385 ResolvedTheme・RF387 View Spec）の**消費側**であり、
契約確定と並行して製品文書・schema・fixture を先行させることが
DAG 上要求されている。既存 `ui/src/styles.css` の暗黙ダーク固定と、
snapshot のみの workspace 永続化は、複数 mode/density/preset の
要求を満たさない。

## Users and Value

- DAW ユーザー: light/dark・密度の切替、作業内容に応じた named
  workspace preset（Mixing/Editing/Plugin Dock）の即時復元、
  モニター構成変更・破損時も安全な fallback。
- 開発者: 製品差分が Git 管理の DTCG 文書と schema 化され、共通
  Token/Component（ruviform）を変更せずに表現できる。headless
  fixture が GPUI 無しでも Theme/Preset 意味論を検証できる。
- RF385/RF387 owner: Consumer override 文書・選択状態・消費点の
  具体例が早期に確定する（coordinator へ契約要件を status 送信済み）。

## Scope

### In Scope

- `synvelis-theme` crate: DTCG 製品文書の serde 型・schema
  version・envelope validation・ThemeCatalog・`ThemeSelection`/
  `ThemePreferences`・上流 resolver への `adapter`。
- `assets/themes/`: base + dark + light + density compact/
  comfortable の 5 文書（dark は `styles.css` 実測値由来、light は
  provisional と明記）。
- `synvelis-workspace::preset`: `WorkspacePreset`（`PRESET_SCHEMA = 2`
  slot 形式）・validate・`from_json`/`to_json`・v1→v2 migration・
  `apply_preset`・snapshot→preset 変換・`ViewType::PluginDock`。
- `assets/workspaces/`: 1-window / 2-window / plugin-dock の 3
  preset JSON。
- Fixture + headless テスト: valid/invalid override・alias 循環・
  monitor missing・version migration・破損 fallback・「Theme 変更・
  Preset 適用が Project revision/undo/RT に影響しない」不変条件。
- 上記を支える docs chain（discovery/spec/ADR/design/plan/
  test-spec/task/IR）と semantic screen spec。

### Out of Scope

- Token Interpreter・ResolvedTheme・GPUI Style Adapter の実装
  （Ruviform #385）。GPUI 実描画・Native Component Lab・pixel 比較
  （RF390/#313/#314）、製品実 GUI 受入（SV114）。
- Plugin dock の host/native surface/session 実装（SV176・RF388/
  389）。外部 plugin 自身の GUI への Theme 強制 — **しない**。
- Figma 連携・外部デザインツール連携の実装（候補であり実行時/
  ビルド必須にしない）。静的 HTML/CSS モック（任意成果物）。
- 旧 SolidJS/WebView frontend の削除・縮退（新 Native 実証前の
  破壊的削除禁止）。`ui/` の SolidJS 側への theme 配線。
- AudioLens 側の変更。ruviform 側の契約変更（不足は upstream
  issue/協議）。

## Requirements

- **R1 分離**: Theme 文書・`ThemePreferences` と `WorkspacePreset` は
  別 schema/version/restore を持ち、互いと `WorkspaceSnapshot`
  （schema 2）・Project 保存 schema とは独立に進化する。
- **R2 Driver-neutral**: 新規公開型・通常 View/Domain/headless に
  GPUI/DOM/wry/Toolkit/OS handle 型を持ち込まない。依存は serde
  系のみ（workspace は既存の ruviform-ui 中立契約を使用）。
- **R3 再解釈禁止**: alias/mode/override merge・ResolvedTheme 生成を
  Synvelis 側で実装しない。解決は ruviform main 上の実 resolver
  への `adapter` 経由（`ThemeError::Resolve` で上流 error を透過）。
- **R4 意味分離**: Theme 切替・Preset 適用は Project revision/undo/
  DSP/RT に影響しない（theme は repaint-only の既存契約に整合）。
- **R5 既存意味保持**: Preset 適用は既存 View identity・Selection/
  Focus/LinkGroup 意味を破壊しない — adopt 対象の view は state を
  保持し、残余は detached pool へ移す（破棄しない）。
- **R6 数値の根拠**: dark palette は `ui/src/styles.css` 実測値由来。
  light は合意資料が無いため provisional と文書上明記し、Native
  Design Review で確定する。
- **R7 fallback 明示**: monitor missing/layout 破損/未対応 schema は
  安全な既定へ落とすか明示 error とし、panic/部分適用で中途半端な
  状態にしない。

## Acceptance Criteria

- [ ] **SV177-01**: `synvelis-theme` が GPUI/Toolkit 依存なしで
  `mise run check`/`test` に合格し、5 つの製品文書を load・
  validate できる。`check-headless-deps` 違反なし。
- [ ] **SV177-02**: envelope validation が schema 不一致・非空
  field・layer tag・両 envelope の矛盾（upstream layer/modes との
  不一致・mode+density 併記）を fail-closed に検出する。token 系
  problem（unknown `$type`・malformed value・alias 構文不備・alias
  循環）は consumer load を通過し、adapter 経路で上流の対応する
  error variant（`UnknownTokenType`/`InvalidValue`/`AliasCycle`
  など）として表面化する。全 4 mode×density 組合せが実 resolver
  で `ResolvedTheme` を返す。
- [ ] **SV177-03**: `ThemeCatalog` が (theme, mode, density) の
  catalog を構成し、`ThemeSelection`→決定的 viz style hint を返す。
  `ThemePreferences` が独自 schema で save/load roundtrip する。
- [ ] **SV177-04**: `WorkspacePreset` v2 が 1-window/2-window/
  plugin-dock の 3 fixture を apply でき、kind+index adopt・spawn・
  detached pool 移行・link group・focus が headless
  （`FakeWindowHost`）で成立する。
- [ ] **SV177-05**: monitor missing 時は既存の署名照合 fallback で
  別 monitor/既定位置へ再配置される。Preset JSON の schema 不一致・
  未対応 version・構造不備は明示 error で部分適用しない。
- [ ] **SV177-06**: v1（snapshot 形状）→v2（slot 形状）migration が
  fixture で成立し、現行 `capture()`→preset 変換の roundtrip が
  成立する。
- [ ] **SV177-07**: Theme 切替・Preset 適用で Project revision・
  undo・（stub）RT が不変であることをテストで示す。
- [ ] **SV177-08**: semantic screen spec（Mixer/Transport/
  Arrangement/Inspector/Plugin Dock の variant・状態・density・
  a11y 列挙）が design doc にあり、Native 描画受入は後続 Gate へ
  **NotRun** として記録される。
- [ ] **SV177-09**: 共通 Token/Component（ruviform）未変更で製品差分
  を表現でき、AudioLens への暗黙変更が無い（依存・共通ファイル
  変更なし）。

## Deferred Design Concerns

- `ResolvedTheme` → GPUI/driver mapper 結合 | RF386 mapper 未着地 |
  RF386 着地時 | RF385 resolver 本体は main 着地済み（adapter は
  実接続・headless 検証済み、残るは driver 写像）。
- Light theme の確定値 | 合意資料なし・provisional | Native Design
  Review（RF390 Lab + 実 OS）| 暫定値が製品外観として暫定的に残る。
- `ViewType::PluginDock` の実 plugin 対応 | host/session は SV176 |
  SV176 実装時 | preset では kind の語彙のみ提供。
- Screen spec の RF387 ViewSpec 型への写像 | RF387 公開型未着地 |
  RF387 着地時 | 現時点は文書 spec のみ。

## Sources and Relations

- Issue #177 本文 + 2026-11-07 横断レビュー確定仕様 v2 セクション
- `docs/discovery/sv177-product-theme-workspace-inventory.md`（棚卸し）
- RF #384（依存方向・禁止範囲）、RF #385（Interpreter Owner）、
  RF #387（View Spec Owner）、SV #23（DAG）
