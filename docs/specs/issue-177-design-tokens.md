---
id: "SPEC-iHIZmuGqQNil5fLLCeaC46"
type: "spec"
status: "draft"
title: "SV177 design tokens — canonical semantic vocabulary + headless↔palette parity + design fixtures"
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
    - "docs/discovery/issue-177-design-tokens.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gpui-03-product-theme-workspace-presets.md"
    - "docs/test-specs/autx-07-theme-parity.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/issue-177-design-tokens.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV177 design tokens — canonical semantic vocabulary + headless↔palette parity + design fixtures

## Summary

出荷 DTCG theme 文書（`assets/themes/*.dtcg.json`）に upstream
`ruviform-ui-gpui::ViewPalette` が消費する canonical semantic 語彙
（`surface.*`/`text.*`/`accent.*`/`control.*`/`state.*`/`viz.*`/
`space.*`）を、既存の製品 primitive（`color.*`/`spacing.*`）への
DTCG alias として追加し、`ResolvedTheme` → `ViewPalette` の意味論
parity を headless/gpui-test 契約テストで固定する。画面別 design
fixture と 2scale×2theme 意味論差分を既存 CI 範囲で記録し、provisional
light/status 値の設計レビュー残件を明示する。

## Why Now

Issue #177 の残件として、出荷 theme と GPUI consumer 語彙の不一致により
製品 theme が literal fallback に落ちている現状を是正する。前 slice
（theme preset 受入・RF385 adapter）は完了済みで、canonical 語彙化と
parity 証拠が唯一の未充足 slice である。

## Users and Value

- Synvelis 製品: 出荷 theme が実際に GPUI 描画へ投影され、ユーザーの
  theme/density 選択が意味を持つ。
- レビュアー/保守者: `ResolvedTheme` レベルの契約テストにより、
  upstream の語彙変更が製品 theme を壊したときに CI で検出できる。
- 設計レビュー（RF#631/SV#114 gate）: provisional 値の確定対象が
  文書上で明示される。

## Scope

### In Scope

- `assets/themes/synvelis-{dark,light}.dtcg.json` に canonical color
  group（`surface`/`text`/`accent`/`state`/`control`/`viz`）を追加。
  既存 `color.*` primitive への DTCG alias を原則とし、対応 primitive
  のない token（`state.disabled`/`viz.waveform_rms` 等）のみ直接値。
- `assets/themes/synvelis-density-{compact,comfortable}.dtcg.json` に
  canonical dimension group `space`（`space.sm`/`space.md`、
  `ViewPalette` が消費する範囲）を `spacing.*` への alias として追加。
  `space.page` は同じ層の補助 token として定義する。
- light theme の provisional 欠落（`color.bg-active`/`color.bg-input`）
  を canonical alias 先として補完し、provisional 注記を維持・更新。
- `crates/synvelis-theme` に canonical vocabulary 契約テストを追加:
  4 組合せ（dark/light × compact/comfortable）で canonical key が
  製品値を返し、fallback literal と一致しないことを検証。
- `crates/synvelis-app/tests/theme_parity.rs`（gpui-test lane）に
  resolved product theme → GPUI mount の parity 証拠テストを追加。
  `ViewPalette` は private のため、(a) `theme: Some(..)` mount が
  settle・drawn plan を返すこと、(b) resolved 値が fallback literal
  と一致しないこと、(c) 画面 spec の semantic component key が
  fixture として固定されていること、を検証する。
- 画面別 design fixture: `screens.rs` の代表画面（main/secondary）
  の semantic component key ↔ canonical token 期待値の対応表を
  テスト/文書として固定し、2scale×2theme の意味論差分を evidence に
  記録。
- `evidence/sv177/` に parity/差分記録を追加。
- doc チェーン `docs/{discovery,specs,designs,adr,plans,tasks,
  test-specs,impl/ir,impl/exp}/issue-177-design-tokens.*`。

### Out of Scope

- `ViewPalette` の public 化や ruviform 側の変更（upstream issue 化が
  必要な場合のみ RF 側で対応 — 本 slice では行わない）。
- GPUI 製品画面本体（`synvelis-native`・`ViewMount`・screen 描画
  コード）の変更。
- 実 OS window・実入力・pixel golden・IME/DPI 実走（RF#631+SV#114）。
- Workspace Preset の製品 UX/apply 配線（AC2 解釈は別判断）。
- Plugin GUI 内部 theme（SV#176/RF#630 領域）。
- docs index（`docs/**/README.md`）の編集・再生成。
- Issue #177 の close・AC checkbox 更新。

## Requirements

- R1: canonical 語彙の正本は upstream `rvf-dtcg-profile.v1` の解釈
  結果（`ViewPalette::from_theme` が read する key 集合）。Synvelis
  側に第二の interpreter・独自 semantic source of truth を作らない。
- R2: canonical token は既存 primitive への DTCG alias（`{color.x}`
  形式の whole-value alias）を優先し、値の二重定義を避ける。
- R3: alias/invalid override・mode/density の merge 挙動は既存の
  `synvelis-theme` adapter（RF385）経路をそのまま使い、製品側で
  独自解決ロジックを追加しない。
- R4: parity 証拠は実行可能な既存 CI lane（`mise run test
  synvelis-theme`・`mise run test-app-gpui`）で完結させる。
- R5: provisional 値（light `bg-active`/`bg-input`、`state.disabled`、
  `viz.waveform_rms`、status ramp）は設計レビュー未確定であることを
  文書・コードコメントで区別し、確定済み値と混同しない。
- R6: 画面 fixture は semantic component key（`screen.*`/`tab.*`/
  `transport.*`/`mixer.*` 等）を stable key として扱い、token 期待値は
  canonical 語彙で記述する。

## Acceptance Criteria

- [ ] 5 つの出荷 DTCG 文書が canonical 語彙を定義し、4 組合せの
      `ResolvedTheme` で canonical key が fallback literal ではない
      製品値を返す（headless assert）。
- [ ] `crates/synvelis-app` の gpui-test lane で `theme: Some(resolve
      (..))` mount が settle し drawn plan を返す parity テストが
      追加され、CI lane で実行可能。
- [ ] Mixer/Arrangement/Transport/Inspector/Plugin Panel 相当の画面
      spec について、semantic component key ↔ canonical token の
      fixture と 2scale×2theme 意味論差分が evidence に記録される。
- [ ] provisional light/status 値の設計レビュー注記が更新され、
      未確定項目が明示される。
- [ ] `mise run fmt-check`、theme/app の check/lint/test
      （gpui-test lane 含む）、`check-headless-deps` が成功する。

## Deferred Design Concerns

- `ViewPalette` の公開 API 化・palette 色の observer 公開:
  reason=upstream ruviform の変更が必要 | re-engagement=RF 側で
  palette 観測面が必要になったとき（例: pixel 差分の自動検出）|
  risk if ignored=palette 内部値の直接照合ができず parity は
  ResolvedTheme レベルに留まる。
- `dock.*` 語彙: reason=pin rev の `ViewPalette` が未消費 |
  re-engagement=Plugin Dock 外枠 theme が必要になったとき（SV#176）|
  risk if ignored=Dock 外枠は fallback のまま。
- status ramp の色値確定: reason=native design review 待ち |
  re-engagement=RF#631/SV#114 の design review gate | risk if
  ignored=暫定色が出荷される可能性。

## Sources and Relations

- `docs/discovery/issue-177-design-tokens.md`（本 spec の根拠調査）
- ruviform pin `cbcc834` の `ViewPalette::from_theme` read 一覧と
  `fixtures/themes/synvelis-dark.dtcg.json`（canonical 先例）
- `docs/specs/sv-gpui-03-product-theme-workspace-presets.md`（前 slice
  の theme/preset spec）
- `.devin/dispatch-specs/sv177-design-tokens.explore.json`
  （verdict=DISPATCHABLE）
- `evidence/reports/sv177-theme-preset.md`、
  `evidence/reports/sv177-rf385-adapter.md`（前 slice 証跡）
