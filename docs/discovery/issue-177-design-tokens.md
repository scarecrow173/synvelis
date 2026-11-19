---
id: "DISC-034YXLcmkmPrMKdfo8fomq"
type: "discovery"
status: "active"
title: "SV177 design tokens canonical semantic vocabulary exploration"
created: "2026-11-17"
updated: "2026-11-17"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/177"
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
    - "docs/designs/sv177-product-theme-workspace-preset-design.md"
    - "docs/test-specs/autx-07-theme-parity.md"
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

# SV177 design tokens canonical semantic vocabulary exploration

## Summary

Synvelis の出荷 DTCG theme 文書は `color.*`/`spacing.*`/`control.height` 等の
製品独自語彙で書かれているが、GPUI driver の `ViewPalette::from_theme`
（ruviform pin `cbcc834`）が読む canonical 語彙は
`surface.*`/`text.*`/`accent.*`/`control.*`/`state.*`/`viz.*`/`space.*`。
語彙が不一致のため現状は全項目が literal fallback に落ち、製品 theme が
GPUI 描画へ実質反映されていない。本 exploration の結論: 製品文書内に
primitive→canonical の DTCG alias 層を追加し、parity は
`ResolvedTheme` レベルで契約固定する（`ViewPalette` は private で
直接照合できないため）。

## Exploration Goal

Issue #177 slice「canonical semantic vocabulary 化 + ResolvedTheme→
ViewPalette parity + 画面別 design fixture」の実装方針を確定する。
具体的には (1) canonical token の配置場所と値の由来、(2) parity 証拠の
取り方（`ViewPalette` は ruviform 側 private）、(3) 画面別 fixture の
形態、(4) provisional light/status 値の扱いを決める。

## Key Issues and Assumptions

- `ViewPalette::from_theme`/`fallback` は `ruviform-ui-gpui` の private
  module（`render.rs`）にあり、公開 re-export は `COMMAND_INTENT_OP`
  のみ。Synvelis 側から palette を直接構築・照合する API は存在しない。
- canonical 語彙の正本は upstream `rvf-dtcg-profile.v1` の解釈結果で
  あり、Synvelis 側に第二の interpreter を実装しない契約
  （RF385 adapter 設計）を維持する必要がある。
- `ViewMount.theme: Option<ResolvedTheme>` は公開 API で、
  `theme: Some(..)` mount は `from_theme` を内部で必ず通る —
  palette 内部は観測不能でも「受理されて描画される」ことは
  gpui-test lane で検証できる。
- 出荷 light theme は `color.bg-active`/`color.bg-input` を欠き、
  テーマ全体が provisional（native design review 待ち）。
- status 語彙（`color.status-*`、`StatusTone` が正本）は
  canonical `state.*`（chrome 状態）とは別契約であり統合しない。

## Alternatives and Comparison

| Option | Pros | Cons | Lean |
| --- | --- | --- | --- |
| A. 製品文書に canonical group を追加し `color.*`/`spacing.*` へ DTCG alias | 単一値源・語彙対応が文書上で自明・upstream 変更不要 | alias 層の二重語彙が残る | yes |
| B. `color.*` 等を canonical 名へ rename（旧名廃止） | 語彙が一意 | web 由来 consumer・status 契約・既存 fixture との整合コスト大 | no |
| C. canonical token を全て直接値で記述 | alias 解決に依存しない | 同一色の二重定義で値ずれリスク | no |
| D. `ViewPalette` を ruviform 側で public 化して直接照合 | 最強の parity | upstream 変更が必要で本 slice 範囲外 | no（将来 RF issue 候補） |

## Tentative Conclusions and Hypotheses

- confirmed: canonical 語彙は upstream fixture
  （`ruviform-ui-test/fixtures/themes/synvelis-dark.dtcg.json`）と
  `ViewPalette::from_theme` の read 一覧で確定できる
  （`surface.bg/raised/border`、`text.primary/dim`、`accent.primary`、
  `state.error/disabled`、`control.track/fill/thumb`、
  `viz.waveform/waveform_rms/baseline/cursor`、`space.sm/md`）。
- confirmed: mode 文書（dark/light）に `surface`/`text`/`accent`/
  `state`/`control`/`viz` の color group、density 文書に `space`
  dimension group を置くのが既存層構造（base→mode→density）と整合する。
- hypothesis: `control.track`→`color.bg-input`、`viz.baseline`→
  `color.border`、`viz.cursor`→`color.fg` 等の alias 先対応は
  semantic 的に妥当。`state.disabled`/`viz.waveform_rms` は対応する
  `color.*` primitive がなく直接値（provisional）とする。
- hypothesis: `space.sm`/`space.md` は density 文書の
  `spacing.sm`/`spacing.md` への alias とし、density 差分をそのまま
  継承させる。
- confirmed: parity 証拠は (a) `synvelis-theme` contract test で
  4 組合せ（dark/light × compact/comfortable）の canonical key が
  製品値を返すこと、(b) `synvelis-app` gpui-test lane で
  `theme: Some(resolve(..))` mount が settle・描画 plan を返すこと、
  (c) 解決値が `ViewPalette::fallback` literal と一致しないこと、の
  3 層で構成する。

## Open Questions

- `state.disabled`/`viz.waveform_rms` の直接値は provisional —
  native design review（RF#631/SV#114 gate）で確定する。
  light `bg-active`/`bg-input` も同様に provisional 追加。
- `dock.*` 語彙は pin rev の `ViewPalette` が read しないため本 slice
  では未定義。Plugin Dock 外枠の theme は SV#176 領域。

## Promotion Candidates

- [x] spec-doc needed: canonical 語彙 mapping・parity 契約・fixture
      受入条件を `docs/specs/issue-177-design-tokens.md` に固定
- [x] adr-doc needed: 「製品文書内 alias 層 + ResolvedTheme レベル
      parity（ViewPalette 非公開のまま）」の境界決定を
      `docs/adr/issue-177-design-tokens.md` に記録

## Sources and Relations

- ruviform pin `cbcc834`:
  `crates/ruviform-ui-gpui/src/render.rs`（`ViewPalette::from_theme`/
  `fallback`・消費箇所）、`crates/ruviform-ui-test/fixtures/themes/
  synvelis-dark.dtcg.json`（canonical 語彙の先例 fixture）
- `evidence/reports/sv177-rf385-adapter.md`（前 slice の adapter 証跡）
- `.devin/dispatch-specs/sv177-design-tokens.explore.json`
  （verdict=DISPATCHABLE）

## Evidence

- 2026-11-17: `ViewPalette` の field 一覧と fallback literal を
  `render.rs:120-160` で確認。公開面は `theme.map_or_else(
  ViewPalette::fallback, ViewPalette::from_theme)`（`ViewMount::
  mounted`）のみで、palette 自体は private。
- 2026-11-17: 出荷 `assets/themes/*.dtcg.json` 5 件を確認。
  dark/light は `color.*`（status ramp 含む）のみ、base は
  font/radius/motion/easing、density は spacing/control.height/
  font.size。canonical group は未定義。
- 2026-11-17: upstream `synvelis-dark.dtcg.json` fixture が canonical
  語彙の完全な先例（`surface`/`text`/`accent`/`state`/`control`/`viz`/
  `space`、token 単位 `$type` 明示）。
- 2026-11-17: `screens.rs` の tab 構成を確認 — Arrangement
  （Table+Waveform）、Transport（Label+Buttons+Seek）、Mixer
  （Fader+Toggle+Meter）、Inspector（Label+Table×2）、Signal Flow
  （Label+Table×3）、Project（Label+Table）、secondary view
  （Label+Buttons+Seek）。plugin dock は `ruviform_ui_test::
  plugin_dock_view_spec` の共有 spec。
- 2026-11-17: gpui-test observer API を確認 — `observe(window)` は
  component snapshot、`drawn(window)` は `DrawnPlan`（色を含まない
  幾何/文字列 DTO）を返す。palette 色は公開観測面に出ない。
  `DrawnPlan` に色がないため「theme 反映の画素差」は本 lane では
  観測不能 — NotRun 記録対象。
- 2026-11-17: `ScaleContext`（`scale_factor`/`rem_base_px`）は
  `ResolvedTheme::with_scale` で token 値を変えず mapper 側変換
  ヒントのみ。px unit の本 theme では scale 次元の token 差分は
  意味論上ゼロ（device px 変換は DPI gate 領域）。
- 2026-11-17: route_graph.js は既存 docs の broken-relation
  （本 slice 無関係の legacy）と focus-required で blocked を返す。
  本 slice は新規 chain のみを対象とし、既存 broken relation の
  修復は行わない。
