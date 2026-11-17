---
id: "TASK-034YXXXE7kuTr6kR5K8KV8"
type: "task"
status: "done"
title: "SV177 issue-177-design-tokens"
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
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV177 issue-177-design-tokens

## Work

synvelis#177 の残件 slice: 出荷 DTCG theme 文書を canonical
semantic 語彙化し、`ResolvedTheme` → `ViewPalette` の parity を
headless/gpui-test で契約固定し、画面別 design fixture と
2scale×2theme 意味論差分を evidence に記録する。

1. mode 文書（dark/light）に canonical color group
   （`surface`/`text`/`accent`/`state`/`control`/`viz`）を追加 —
   `color.*` primitive への whole-value alias 優先、対応のない
   `state.disabled`/`viz.waveform_rms` は mode 別直接値（provisional）。
   light は `color.bg-input`/`color.bg-active` を provisional 補完。
2. density 文書 2 件に `space.{sm,md,page}` を `spacing.*` への
   alias で追加。
3. `crates/synvelis-theme` contract test に canonical vocabulary
   契約を追加（4 組合せで key 解決・期待製品値一致・fallback
   literal 非一致）。
4. `crates/synvelis-app/tests/theme_parity.rs`（gpui-test lane）で
   shipped 文書 resolve → `ViewMount { theme: Some(..) }` mount →
   settle → observe/drawn plan を assert + 画面 semantic key
   fixture 対応表。
5. evidence/sv177/ に parity・意味論差分・NotRun を記録。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/issue-177-design-tokens.md`
- `docs/designs/issue-177-design-tokens.md`（approved）
- `docs/specs/issue-177-design-tokens.md`
- `docs/adr/issue-177-design-tokens.md`（accepted）
- `docs/discovery/issue-177-design-tokens.md`
- `docs/test-specs/issue-177-design-tokens.md`

## Dependencies

- ruviform pin `cbcc834`（`ViewPalette::from_theme` read key 集合と
  fallback literal の正本）。
- 前 slice: `docs/specs/sv-gpui-03-product-theme-workspace-presets.md`
  （theme catalog/resolver 配線済み）。

## Affected Files

- `assets/themes/synvelis-dark.dtcg.json`
- `assets/themes/synvelis-light.dtcg.json`
- `assets/themes/synvelis-density-compact.dtcg.json`
- `assets/themes/synvelis-density-comfortable.dtcg.json`
- `assets/themes/synvelis-base.dtcg.json`（変更なし — 対象外確認用）
- `crates/synvelis-theme/tests/theme_contract.rs`
- `crates/synvelis-app/tests/theme_parity.rs`（新規）
- `evidence/sv177/`（新規記録）
- `docs/impl/ir/issue-177-design-tokens.md`、
  `docs/impl/exp/issue-177-design-tokens-exp.jsonl`

## Done When

- [ ] 5 つの出荷 DTCG 文書が canonical 語彙を定義し、4 組合せの
      `ResolvedTheme` で `ViewPalette::from_theme` の全 read key
      （15 color + 2 length）が製品値を返す。
- [ ] `theme_parity.rs` が gpui-test lane で `theme: Some(..)`
      mount の settle・drawn plan・画面 fixture 対応を assert し、
      `mise run test-app-gpui synvelis-app` で pass する。
- [ ] provisional 値（light `bg-input`/`bg-active`、`state.disabled`、
      `viz.waveform_rms`、status ramp）が文書・evidence で
      design review 未確定と明示される。
- [ ] evidence/sv177/ に parity・2scale×2theme 意味論差分・
      NotRun 項目が記録される。

## Verification

- [ ] `mise run test synvelis-theme`
- [ ] `mise run check synvelis-theme` / `mise run lint synvelis-theme`
- [ ] `mise run test-app-gpui synvelis-app`
- [ ] `mise run check-app-gpui synvelis-app` / `lint-app-gpui`
- [ ] `mise run check-app-native synvelis-app` / `lint-app-native`
- [ ] `mise run fmt-check` / `mise run check-headless-deps`
