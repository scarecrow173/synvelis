---
id: "TSPEC-034YXYqKbfbe0JMUs8G70o"
type: "test-spec"
status: "draft"
title: "SV177 design tokens canonical vocabulary parity test spec"
created: "2026-11-17"
updated: "2026-11-17"
owners: []
relations:
  source: []
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
    - "docs/designs/issue-177-design-tokens.md"
  derived-by: []
  verifies:
    - "docs/specs/issue-177-design-tokens.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV177 design tokens canonical vocabulary parity test spec

## Purpose

synvelis#177 slice「canonical semantic vocabulary + headless↔palette
parity + design fixtures」が満たすべき契約を固定する。theme asset の
canonical 語彙化と parity テストが upstream `ruviform-ui-theme` /
`ruviform-ui-gpui` の契約変更で静かに壊れないよう、headless 解決値
と gpui-test mount の 2 層で保証を記述する。実機 pixel golden・実
OS 入力は対象外であり、本 spec はそれらを保証しない。

## Feature

出荷 product theme の canonical semantic vocabulary 解決と
GPUI palette 投影の parity

## Rules

- Rule: `ViewPalette::from_theme`（pin `cbcc834`）が read する全
  canonical key（`surface.bg/raised/border`、`text.primary/dim`、
  `accent.primary`、`state.error/disabled`、`control.track/fill/
  thumb`、`viz.waveform/waveform_rms/baseline/cursor`、
  `space.sm/md`）が、4 組合せ（dark/light × compact/comfortable）
  の `ResolvedTheme` で製品値として解決されなければならない。
- Rule: canonical color token は既存 `color.*` primitive への
  whole-value alias で値を一元化しなければならない。対応
  primitive のない `state.disabled`/`viz.waveform_rms` のみ mode
  別直接値を許し、provisional であることを provenance に記す。
- Rule: `space.*` は density 文書の `spacing.*` への alias とし、
  density 差分をそのまま継承しなければならない。
- Rule: alias/invalid override・mode/density merge は upstream
  `rvf-dtcg-profile.v1` interpreter の結果のみを信用し、Synvelis
  側で独自解決を実装してはならない。
- Rule: `ViewPalette` が private である以上、parity の保証は
  (a) ResolvedTheme 上の canonical key が期待製品値を返すこと、
  (b) `theme: Some(..)` mount が gpui-test lane で settle・
  drawn plan を返すこと、に限定される。palette 内部値の直接照合や
  pixel 差分を主張してはならない。
- Rule: 画面 fixture は `screens.rs` の semantic component key
  （`screen.*`/`tab.*`/`transport.*`/`mixer.*` 等）を stable key
  として扱い、token 期待値は canonical 語彙で記述する。

## Examples

- Example: dark/comfortable で `theme.color("surface.bg")` が
  `Some(#1b1d21 相当の ResolvedColor)` を返し、fallback literal
  `#14181d` と一致しない。
- Example: compact/comfortable で `theme.length("space.sm")` が
  それぞれ 4px/8px を返す（compact の 4.0 は fallback literal と
  偶然一致するが、契約は期待製品値との一致であり key 解決自体が
  製品文書由来であることを別途確認する）。
- Example: `ViewMount { host, theme: Some(dark_resolved) }` で
  `main_screen_spec` を mount → `gui.settle()` → `observe`/
  `drawn` が空でない component/plan を返す。
- Example: light 文書で `color.bg-input` が存在し
  `control.track`→`{color.bg-input}` alias が解決される
  （provisional 値であることは provenance に記録）。

## Guarantees

- canonical 語彙の正本が upstream であり、製品文書はその consumer
  override に留まる。
- 4 組合せ全てで palette 消費 key が製品値を持ち、theme が
  fallback literal に落ちない。
- mode（dark/light）と density（compact/comfortable）の差分が
  ResolvedTheme 上で観測可能。
- gpui-test lane で theme 付き mount が破綻なく描画 plan を返す。

## Non-goals

- `ViewPalette` 内部値・Hsla 成分の直接照合（private module）。
- pixel golden・実 OS window・実入力・IME/DPI 実走。
- `dock.*` 語彙（pin rev の `ViewPalette` が未消費）。
- status ramp（`color.status-*`）と `state.*` の統合 — 別契約。
- Workspace Preset の製品 UX/apply 配線。

## Risk

canonical 語彙と `color.*` の対応が文書から読み取れなくなる、
または alias が静かに fallback 化すると、theme が再び描画へ効か
ない状態に戻る。契約テストがこれを CI で検出する。
