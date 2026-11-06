---
id: "ADR-034T6XUbzRkbVBGiEFXH4Q"
type: "adr"
status: "accepted"
title: "SV177 product theme crate boundary, DTCG document envelope and slot-based workspace preset schema"
created: "2026-11-07"
updated: "2026-11-07"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/177"
    - "https://github.com/scarecrow173/ruviform/issues/384"
    - "https://github.com/scarecrow173/ruviform/issues/385"
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
    - "docs/specs/sv-gpui-03-product-theme-workspace-presets.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/sv177-product-theme-workspace-inventory.md"
    - "docs/specs/sv-gpui-03-product-theme-workspace-presets.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: []
    consulted: []
    informed: []
---

# SV177 product theme crate boundary, DTCG document envelope and slot-based workspace preset schema

## Context and Problem Statement

SV177（製品 Theme / Workspace Presets）は 2026-11-07 横断レビュー
確定仕様 v2 のもと、Synvelis 固有の DTCG 製品 override 文書・
Theme 選択状態・Workspace Preset を Driver-neutral に実装する必要が
ある。RF385 の `ResolvedTheme`/Token Interpreter 公開型は現行の
ruviform rev `49de02c` に存在せず、「独自の第二 Token Interpreter
を作らない」制約下で、Synvelis が所有できる境界・文書形式・
Preset schema を確定させる必要があった。

決めること: (a) Theme 責務をどのクレートに置くか、(b) DTCG 製品
文書の envelope/形式をどうするか、(c) Workspace Preset を
`WorkspaceSnapshot` とどう区別するか・既存機構をどう再利用するか、
(d) RF385 未着地期間の解決要求をどう扱うか。

## Decision Drivers

* RF #384: Consumer の通常 View/Domain/Headless に GPUI/DOM/wry/
  Toolkit/OS handle 型を持ち込まない。共通公開型への漏出禁止。
* RF #385: DTCG 解釈・alias/mode merge・`ResolvedTheme` の唯一の
  正本は Ruviform。Synvelis は製品 override 文書（データ）と選択
  状態を所有する。
* Issue #177 v2: Theme と Workspace Preset は別 Schema/Version/
  Restore。Theme を Project/RT/Graph/Plugin GUI に依存させない。
* `check-headless-deps`: headless crate の依存木に GUI/GPU/WebView
  framework crate を入れない既存 gate。
* 既存 `synvelis-workspace` は Workspace/View/Dock/Focus の
  session 状態を所有し、`LayoutSnapshot::restore_or_default` の
  monitor missing/破損 fallback をすでに消費している。
* AGENTS.md Rust ワークスペース規約: 依存方向は一方向、crate 追加は
  責務境界が明確な場合のみ。

## Considered Options

### クレート境界

* Option A: 新規 leaf crate `synvelis-theme`（serde/serde_json のみ
  依存）に Theme 文書型・validation・catalog・選択状態・解決 seam
  を置き、Preset は `synvelis-workspace::preset` module とする。
* Option B: Theme を `synvelis-workspace` の module にする。
* Option C: Theme を `synvelis-project` に入れる。

### DTCG 文書 envelope

* Option D: DTCG 2025.10 の group/token 構造 + vendor
  `$extensions["synvelis.productTheme"]` に schema/theme/mode/
  density/layer の製品メタを格納する。
* Option E: Synvelis 独自の先頭 envelope フィールドで包む。

### Workspace Preset schema

* Option F: `PRESET_SCHEMA = 2` の slot ベース独立 schema（slot→
  適用時に実 ViewInstanceId を割当）とし、v1 = `WorkspaceSnapshot`
  形状の旧形式からの移行を migration として提供。monitor missing/
  破損 fallback は `LayoutSnapshot::restore_or_default` 再利用。
* Option G: `WorkspaceSnapshot` を名前付きでそのまま preset とする。

### RF385 未着地時の解決

* Option H: `ResolvedThemeSink` trait + `UnresolvedThemeResolver`
  を用意し、解決要求は明示的 `ThemeError::Unsupported` を返す。
* Option I: Synvelis 側で暫定の alias/mode 解決を簡易実装する。

## Decision Outcome

Chosen options: **A + D + F + H**。

- **A**: Theme は Workspace/Layout ではなく View 描画消費の責務で
  あり、leaf crate とすることで「Theme が Project/RT/Graph/Plugin
  GUI に依存しない」ことを依存方向そのもので強制できる。Preset は
  Workspace 配置の責務なので `synvelis-workspace::preset` に置く。
- **D**: `$extensions` は DTCG の vendor メタ標準機構であり、
  RF385 profile 着地時に再解釈される可能性が最も低い形式。
- **F**: snapshot は実 ViewInstanceId・session 由来 ViewState を
  持ちテンプレートとして再適用する意味が曖昧。slot 形式は view
  kind/title/初期 state で配置を記述でき、「現行 layout を preset
  化」（capture→preset）も v1 形式と同じ形になるため v1→v2
  migration が自然に定義できる。
- **H**: 独自解釈の暫定実装は RF385 の正本と第二の意味論を生む。
  `Unsupported` を返す seam は消費点を型で固定しつつ fail-closed。

### Consequences

* Good, because 依存方向で Driver-neutral を強制でき、製品差分が
  Git 管理 JSON + schema 化され、RF385 着地時は adapter 実装のみで
  接続できる。
* Good, because monitor missing/layout 破損の fallback は
  ruviform 既存機構に委譲し、Synvelis 側の第二実装を避けられる。
* Bad, because `synvelis-theme` crate 追加で root `Cargo.toml`
  members・`Cargo.lock`・docs index に共有ファイル差分が出る
  （union 整合で処理）。
* Bad, because slot→実 ViewInstanceId の具体化層が必要で、
  preset 適用は snapshot restore より手順が多い。
* Bad, because `ResolvedTheme` 未着地期間、Theme の実視覚確認は
  headless fixture の semantic 検証に留まる（実描画は後続 Gate）。

### Confirmation

`mise run check`/`lint`/`test` が workspace 全体で pass し、
`check-headless-deps` が `synvelis-theme`/`synvelis-workspace` の
GUI 依存混入を検出しない。fixture による schema/migration/
monitor-fallback/不変条件テストが成立する。

## Implementation Plan

* Affected paths: `crates/synvelis-theme/`（新規）、
  `crates/synvelis-workspace/src/{preset.rs,view.rs,lib.rs,error.rs}`、
  `assets/themes/`、`assets/workspaces/`、root `Cargo.toml`、
  `Cargo.lock`、`docs/{specs,adr,designs,plans,tasks,test-specs,impl}/`。
* Patterns to follow: `persist.rs` の versioned envelope +
  `from_json` で schema 即 reject・破損は `restore_or_default` 委譲、
  `host.rs` `FakeWindowHost`、workspace error enum への variant 追加。
* Patterns to avoid: Synvelis 側の alias/mode merge・色値変換・
  GPUI/Toolkit 型・`WorkspaceSnapshot` の preset 流用・独自の
  monitor 照合再実装。
* Dependencies or configuration: `serde`/`serde_json`（既存
  workspace.dependencies）。新規外部依存なし。

## Verification

* [ ] `mise run check` / `mise run lint` / `mise run test` pass
* [ ] `node tools/check-headless-deps.mjs`（`mise run check-headless-deps`）
  で GUI 依存混入なし
* [ ] fixture テスト: 5 製品文書 valid、invalid 各系検出、3 preset
  apply、monitor missing、v1→v2 migration、Theme/Preset と
  Project revision/undo 不変

## Pros and Cons of the Options

### Option A（採用）

* Good, because Theme 責務が独立 leaf crate となり Driver-neutral・
  Project 非依存を依存方向で強制できる。
* Bad, because workspace members/Cargo.lock/docs index の共有差分。

### Option B

* Good, because crate 追加なし。
* Bad, because Theme=Workspace 内 package になると描画消費と
  layout 責務の境界が曖昧になり、`synvelis-workspace` が Theme
  文書 I/O を抱える。

### Option C

* Good, because crate 追加なし。
* Bad, because Theme を永続 Project 集約へ混入させ、R1 分離要件と
  「Theme を Project/RT に依存させない」に反する。

### Option D（採用）

* Good, because DTCG 標準の vendor メタ機構で RF385 profile との
  前方互換性が高い。
* Bad, because `$extensions` の解釈は RF385 着地まで暫定。

### Option E

* Good, because 独自 envelope は読みやすい。
* Bad, because DTCG 非標準のため RF385 着地時に再変換が必要。

### Option F（採用）

* Good, because slot 形式はテンプレートとして再適用可能で、
  v1→v2 migration・capture→preset が一貫する。
* Bad, because 具体化層（slot→id）が必要。

### Option G

* Good, because 実装最小。
* Bad, because session 由来 ViewState・実 ViewInstanceId を含み、
  Theme/Preset 分離要件と意味が混線する。

### Option H（採用）

* Good, because 消費点を型で固定しつつ fail-closed、第二解釈を
  生まない。
* Bad, because RF385 着地まで実 Theme 解決は Unsupported。

### Option I

* Good, because 暫定でも解決結果を見れる。
* Bad, because RF385 の唯一正本に反し、二重の意味論を生む。

## More Information

* <https://github.com/scarecrow173/synvelis/issues/177>
* <https://github.com/scarecrow173/ruviform/issues/384> —
  Driver-neutral 依存方向・禁止範囲
* <https://github.com/scarecrow173/ruviform/issues/385> —
  `ResolvedTheme`/Token Interpreter 正本
* `docs/discovery/sv177-product-theme-workspace-inventory.md` —
  棚卸し実測（options の根拠）
* `docs/specs/sv-gpui-03-product-theme-workspace-presets.md` — 本決定
  の spec（spec 側から related 参照）
