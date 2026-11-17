---
id: "ADR-034YXYg4hYn98CrAyagGqa"
type: "adr"
status: "accepted"
title: "SV177 design tokens — product-document alias layer + ResolvedTheme-level parity"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/177"
    - "docs/discovery/issue-177-design-tokens.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/issue-177-design-tokens.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-177-design-tokens.md"
    - "docs/plans/issue-177-design-tokens.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-177-design-tokens.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sv-gpui-03-product-theme-workspace-presets.md"
    - "docs/adr/sv177-product-theme-crate-boundary-dtcg-document-envelope-and-slot-based-workspa.md"
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# SV177 design tokens — product-document alias layer + ResolvedTheme-level parity

## Context and Problem Statement

Synvelis の出荷 DTCG theme 文書は `color.*`/`spacing.*`/`control.
height`/`font.*`/`radius.*`/`motion.*` の製品独自語彙で書かれている。
一方、GPUI driver の `ViewPalette::from_theme`（ruviform pin
`cbcc834`）が read する canonical 語彙は `surface.*`/`text.*`/
`accent.*`/`state.*`/`control.*`/`viz.*`/`space.*`。
語彙が不一致のため製品 theme は `ResolvedTheme` 解決後も palette の
全項目で literal fallback に落ち、theme 選択が GPUI 描画へ実質
効いていない。canonical 語彙の正本は upstream
`rvf-dtcg-profile.v1` であり、Synvelis 側に第二の interpreter を
作ることは RF385 adapter 設計に反する。

## Decision Drivers

* canonical semantic 語彙の正本は upstream `rvf-dtcg-profile.v1`
  のみ — Synvelis 側に独自 interpreter・semantic source of truth
  を持たない。
* 既存 `color.*`/`spacing.*` consumer（web view・status ramp・
  既存 test fixture）を壊さない — rename/削除は不可。
* 値の二重定義を避け、mode/density 差分が一箇所に集約される
  構造を保つ。
* parity 証拠は既存 CI lane（headless + gpui-test）で実行可能で
  なければならない。
* `ViewPalette` は ruviform 側 private module であり、本 slice
  で upstream を変更しない。

## Considered Options

* Option A: 製品文書内に canonical group を追加し、既存
  `color.*`/`spacing.*` primitive への DTCG whole-value alias で
  値を一元化する（対応 primitive のない token のみ直接値）。
* Option B: 既存 `color.*` 等を canonical 名へ rename し旧名を
  廃止する。
* Option C: canonical token を全て直接値で記述する。
* Option D: `ViewPalette` を ruviform 側で public 化し、palette
  値を直接照合する parity test を書く。

## Decision Outcome

Chosen option: "Option A", because (1) upstream interpreter が
whole-value alias を正式サポートしており追加の解釈層が不要、
(2) `color.*`/`spacing.*` を残置できるため既存 consumer と
status ramp 契約を壊さない、(3) mode/density 文書という既存の
層構造に canonical group を載せるだけで merge・override 規則を
そのまま利用できる。Option D は upstream 変更を要し本 slice の
範囲外のため、parity は `ResolvedTheme` レベルの契約テスト
（canonical key が製品値を返し fallback literal と一致しない）
+ gpui-test lane での `theme: Some(..)` mount 受理で構成する。

### Consequences

* Good, because canonical 語彙と製品 primitive の対応が文書上で
  自明になり、upstream の語彙変更は contract test で検出できる。
* Good, because `color.*` consumer との後方互換を保ったまま
  GPUI palette へ製品値が投影される。
* Bad, because `color.*` と canonical 語彙の二重語彙が文書内に
  残る — alias で値は一元化されるが、語彙自体は 2 系統存在する。
* Bad, because palette 内部値の直接照合はできず、parity の
  保証は `ResolvedTheme` 解決値 + mount 受理までに留まる
  （`DrawnPlan` は色を含まない）。

### Confirmation

* `crates/synvelis-theme` contract test が 4 組合せ
  （dark/light × compact/comfortable）で canonical key の製品値
  解決を assert する。
* `crates/synvelis-app/tests/theme_parity.rs` が gpui-test lane で
  `theme: Some(resolve(..))` mount の settle・drawn plan を assert
  する。
* `mise run test synvelis-theme`・`mise run test-app-gpui
  synvelis-app` が pass する。

## Implementation Plan

* Affected paths: `assets/themes/synvelis-{dark,light}.dtcg.json`、
  `assets/themes/synvelis-density-{compact,comfortable}.dtcg.json`、
  `crates/synvelis-theme/tests/theme_contract.rs`、
  `crates/synvelis-app/tests/theme_parity.rs`、
  `evidence/sv177/`。
* Patterns to follow: 既存文書の group-level `"$type"` 宣言と
  whole-value alias（`"$value": "{color.bg-panel}"` — light 文書の
  `bg-hover`/`border` が先例）。upstream fixture
  `ruviform-ui-test/fixtures/themes/synvelis-dark.dtcg.json` の
  canonical group 構成。
* Patterns to avoid: `color.*` の rename/削除、status ramp
  （`color.status-*`）と `state.*` の混同、Synvelis 側での独自
  token interpreter 実装、`ViewPalette` への直接依存。
* Dependencies or configuration: なし（依存追加なし。
  `synvelis-theme` は `native` feature 経由で gpui-test lane に
  既に露出）。

## Verification

* [ ] `mise run test synvelis-theme` — canonical vocabulary 契約
* [ ] `mise run test-app-gpui synvelis-app` — parity mount テスト
* [ ] `mise run check`/`lint` 対象 package、`fmt-check`、
      `check-headless-deps`
* [ ] evidence/sv177/ に parity・意味論差分・NotRun 記録

## Pros and Cons of the Options

### Option A

* Good, because alias が upstream の正式機能で追加実装が不要、
  値の単一源が保たれる。
* Bad, because 語彙が `color.*` と canonical の 2 系統になる。

### Option B

* Good, because 語彙が一意になる。
* Bad, because `color.*` consumer・status ramp・既存 fixture
  との整合コストが大きく、web 由来の参照を壊す危険がある。

### Option C

* Good, because alias 解決に依存しない。
* Bad, because同一色の二重定義で mode/density 差分の一貫性が
  人力管理になる。

### Option D

* Good, because palette 内部値を直接照合でき parity が最も強い。
* Bad, because upstream ruviform の変更が必要で本 slice の
  編集範囲外。pin 更新を伴うため時期尚早い。

## More Information

* ruviform pin `cbcc834`:
  `crates/ruviform-ui-gpui/src/render.rs:120-141`（`from_theme`
  read key と fallback literal）、`crates/ruviform-ui-theme/
  src/doc.rs`（alias・`$type` 継承・merge 規則）。
* `docs/designs/issue-177-design-tokens.md`（mapping 表の正本）。
* `docs/discovery/issue-177-design-tokens.md`（代替案比較の根拠）。
* 関連: synvelis#177（Issue 全体、open 継続）、RF#631/SV#114
  （実機 design review gate）、SV#176（Plugin Dock 外枠）。
