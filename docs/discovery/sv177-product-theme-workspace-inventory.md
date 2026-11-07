---
id: "DISC-034T6XSXfrmJPxTvbinHT6"
type: "discovery"
status: "resolved"
title: "SV177 製品 Theme/Workspace 棚卸し — 既存 Theme・Workspace 永続化・RF385 契約状況"
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
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV177 製品 Theme/Workspace 棚卸し — 既存 Theme・Workspace 永続化・RF385 契約状況

## Summary

SV177（製品 Theme / Workspace Presets）着手前の棚卸し。結論（確度
高）: (1) Synvelis 側に既存の製品 Theme 実装は無く、唯一の実データは
旧 SolidJS フロントエンドの `ui/src/styles.css` ダーク固定 8 色変数
と `viz_ui.rs` の paint-only `VizStyleInput`（theme 名 + colormap）。
(2) Workspace 永続化は `WorkspaceSnapshot`（schema 2）が
`LayoutSnapshot`（ruviform、schema 1・モニター署名照合・safe
fallback 内蔵）を包む形で確立済みで、「名前付き Preset」概念は未実装。
(3) 依存 ruviform rev `49de02c` に `ResolvedTheme`/DTCG/Token
Interpreter 型は存在しない（RF385 未着地）ため、Theme 解決は明示的
`Unsupported` シームとして設計する。

## Exploration Goal

SV177 の実装境界を確定するため、(a) 既存の製品 Theme/Workspace 永続化
状態、(b) RF385 の公開契約が消費可能か、(c) Preset 適用が再利用できる
既存機構、を確認する。2026-11-07 横断レビュー確定仕様 v2 が Theme
（color/typography/spacing/density/modes）と Workspace Preset
（Window/Dock/Layout/View selection/pin/focus）を別 Schema/Version/
Restore とすること、Driver-neutral（GPUI 型を露出しない）であることを
要求するため、既存コードがそれを満たすかの判定が必要だった。

## Key Issues and Assumptions

- RF385 の `ResolvedTheme`/Token Interpreter 公開型が未整備のまま
  SV177 を進める必要がある — 「独自第二 Token Interpreter を作らない」
  制約下で、Synvelis 側が持てるのは DTCG 文書データ + 構造検証 +
  選択状態 + Unsupported 解決シームまで。
- Preset は「現在状態の snapshot」とは別物（名前付きテンプレート）だが、
  monitor missing / layout 破損の fallback は `LayoutSnapshot::
  restore_or_default` をそのまま再利用できる — これが成立するかが鍵。
- Light theme の具体的色値に合意資料が無い。`ui/src/styles.css` の
  dark パレットのみが現行製品の実測値であり、light は「provisional」
  として扱う必要がある（架空値を確定仕様にしない制約）。

## Alternatives and Comparison

| Option | Pros | Cons | Lean |
| --- | --- | --- | --- |
| Preset = `WorkspaceSnapshot` をそのまま名前付き保存 | 実装最小 | snapshot は session 状態（selection/undo 隣接の ViewState・実 ViewInstanceId）を含み、テンプレートとして再適用する意味が曖昧。Theme/Preset 分離要件と混線しやすい | no |
| Preset = slot ベース独立 schema + `LayoutSnapshot::restore_or_default` 再利用 | View 種別/論理 slot で配置を記述でき、適用時に新規 ViewInstanceId を割当。monitor missing/version fallback は ruviform 既存機構に委譲。v1（snapshot 形状）→v2（slot 形状）移行で「現行 layout を preset 化」も自然 | 変換層が必要 | yes |
| Theme を `synvelis-workspace` の module として実装 | crate 追加なし | Theme は Workspace/Layout ではなく View 描画消費であり、`check-headless-deps` 的にも境界が曖昧になる | no |
| Theme を独立 leaf crate `synvelis-theme`（serde のみ依存） | Theme=Project/RT/Graph/Plugin 非依存・Driver-neutral が型で強制できる。RF385 契約着地時に adapter を追加するだけ | crate/members/Cargo.lock 変更が増える | yes |

## Tentative Conclusions and Hypotheses

- **confirmed**: `VizStyleInput`（`crates/synvelis-workspace/src/viz_ui.rs`）
  は「style は cache key ではない・theme 変更は repaint-only」という
  既存契約を持ち、Theme 切替で Project/Engine を再生成しない要件と
  整合する。ThemeSelection→`VizStyleInput.theme` は決定的な文字列
  ヒントとして写像可能。
- **confirmed**: `WorkspaceManager::capture()/restore()` +
  `LayoutSnapshot::restore_or_default` が monitor 欠落時の安全復元を
  内蔵する。Preset 適用は slot→実 ViewInstanceId を先に具体化して
  `LayoutSnapshot` を合成すれば、この経路をそのまま再利用できる。
- **confirmed**: `ruviform-ui` rev `49de02c` の public API に Theme/
  Token/ResolvedTheme 関連型は存在しない。`dock.rs`/`window.rs` が
  Window/Dock の中立契約、`LayoutSnapshot`/`MonitorHint`/`WindowGeometry`
  が serde 可能な配置契約。
- **hypothesis**: DTCG 文書は DTCG 2025.10 の group/token 構造 +
  vendor `$extensions` を製品メタ（theme/mode/density/layer）に使う
  形が、RF385 profile 着地時の再解釈コストが最も小さい。`$type` は
  当面 color/dimension/fontFamily/fontWeight/duration/cubicBezier/
  number の core subset に限定し、未知 `$type` は validation error
  （schema version で明示）。

## Open Questions

- RF385 が公開する Consumer override 投入口の正確な型名・関数
  signature・token key 命名規則（semantic/component 層）— coordinator
  へ status 送信済み（msg_953f9ad01e8c）。未回答の間は Synvelis 側
  seam を `Unsupported` 明示で進める。
- `ViewType` に plugin dock 系 surface kind が無い（現在 4 種）。
  Preset fixture が「Plugin Dock」を表現するための kind を追加するか。
  plugin dock の実装本体は SV176 の所有 — ここでは surface kind の
  語彙追加に留める方針で設計し、design review で確認する。

## Promotion Candidates

- [x] spec-doc needed: 製品 Theme 文書 + ThemeSelection + Workspace
  Preset の schema/version/restore/migration・fixture・受入を固定
- [x] adr-doc needed: `synvelis-theme` leaf crate 追加・DTCG envelope・
  slot ベース Preset schema + `LayoutSnapshot` 再利用・Unsupported
  解決シームの決定

## Sources and Relations

- <https://github.com/scarecrow173/synvelis/issues/177>（本 Issue）
- <https://github.com/scarecrow173/ruviform/issues/384>（Driver-neutral
  正本・依存方向・禁止範囲）
- <https://github.com/scarecrow173/ruviform/issues/385>（ResolvedTheme/
  Token Interpreter Owner）
- <https://github.com/scarecrow173/ruviform/issues/387>（Component/View
  Specs Owner — screen spec は消費側として記述）
- <https://github.com/scarecrow173/synvelis/issues/23>（着手 DAG:
  SV177 は Start 並行 lane）
- `crates/synvelis-workspace/src/{persist,workspace,viz_ui,state,view}.rs`
- ruviform checkout `49de02c` `crates/ruviform-ui/src/{dock,window}.rs`

## Evidence

### 2026-11-07 棚卸し実測

- `ui/src/styles.css`（223 行）: `color-scheme: dark` 固定、
  `--bg:#1b1d21 --bg-panel:#23262c --fg:#e8e8ea --fg-dim:#9aa0a8
  --accent:#6aa9e8 --warn:#f0a868 --err:#e86a6a --ok:#6ac88a`。
  light/密度の変種なし。これが現行製品の唯一の実測 palette。
- `viz_ui.rs`: `VizStyleInput { theme: Option<String>,
  spectrum_colormap: Option<String> }`、`set_theme` は repaint-only・
  cache key 非含有の契約コメントあり（"Style/theme is never part of
  the key"）。
- `persist.rs`: `WORKSPACE_SCHEMA = 2`、ViewRecord/LinkRecord/
  WorkspaceSnapshot、from_json は schema 不一致を即 reject、layout
  破損は `LayoutSnapshot::restore_or_default` へ委譲。
- `workspace.rs`: `capture()`/`restore(snap, monitors)`、`spawn_view`/
  `detach_to_pool`/`apply_restored_tree`/`create_window_at`/`activate`
  系の適用に必要な public 操作が揃っている。
- ruviform `dock.rs`: `LAYOUT_SCHEMA_VERSION = 1`、WindowSnapshot
  {geometry+monitor hint+tree:Option<DockNode>}、RestoredLayout
  {placements+detached}、MonitorHint=name+bounds+scale_factor 署名。
  pin 概念なし（pin は `InspectionMode::Pinned` として ViewState 側に
  存在）。
- `view.rs`: `ViewType` = Arrangement/Mixer/Inspector/SignalFlow の 4 種。
- Cargo.lock: ruviform git rev `49de02cc530d324772d8556225030eb2b9307751`
  で ruviform-ui 依存。`grep -r ResolvedTheme|dtcg` は 0 件。
- `assets/branding/`: logo.svg/logo-dark.svg/symbol.svg の 3 件のみ。
  DTCG 文書・theme JSON・workspace preset JSON は未存在。
