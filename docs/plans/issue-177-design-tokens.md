---
id: "PLAN-034YYxZVou00vT9hA3FTRe"
type: "plan"
status: "draft"
title: "SV177 issue-177-design-tokens implementation plan"
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
  implements:
    - "docs/specs/issue-177-design-tokens.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/issue-177-design-tokens.md"
    - "docs/designs/issue-177-design-tokens.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SV177 issue-177-design-tokens implementation plan Implementation Plan

## Goal

出荷 DTCG theme 文書に canonical semantic 語彙を追加し、
`ResolvedTheme` → `ViewPalette` の parity を headless/gpui-test の
契約テストで固定し、画面別 design fixture と 2scale×2theme 意味論
差分を evidence に記録する。

## Scope

単一の実装 stream。theme asset（JSON）・contract test（Rust）・
parity test（Rust）・evidence・doc chain を 1 plan で扱う。upstream
ruviform・GPUI 製品画面本体・Workspace Preset 配線は境界外。

## Upstream Documents

- `docs/specs/issue-177-design-tokens.md`
- `docs/adr/issue-177-design-tokens.md`
- `docs/discovery/issue-177-design-tokens.md`

## Design Inputs

- `docs/designs/issue-177-design-tokens.md`（approved — canonical
  token mapping 表・parity evidence 3 層構成・provisional 値を確定済み）

## File Map

- `assets/themes/synvelis-dark.dtcg.json` - canonical color group
  （surface/text/accent/state/control/viz）追加。`color.*` への
  alias + `state.disabled`/`viz.waveform_rms` の直接値。
- `assets/themes/synvelis-light.dtcg.json` - 同上 + `color.bg-input`/
  `color.bg-active` の provisional 補完。
- `assets/themes/synvelis-density-compact.dtcg.json` - `space.{sm,md,
  page}` を `spacing.*` への alias で追加。
- `assets/themes/synvelis-density-comfortable.dtcg.json` - 同上。
- `assets/themes/synvelis-base.dtcg.json` - 変更なし。
- `crates/synvelis-theme/tests/theme_contract.rs` - canonical
  vocabulary 契約テスト追加（既存 test への追加または新規 test fn）。
- `crates/synvelis-app/tests/theme_parity.rs` - 新規。gpui-test lane
  の mount 受理 parity + 画面 semantic key fixture。
- `evidence/sv177/` - parity/差分/NotRun 記録。
- `docs/impl/ir/issue-177-design-tokens.md`、
  `docs/impl/exp/issue-177-design-tokens-exp.jsonl` - 実装記録・
  実験イベント。

## Implementation Sequence

- [ ] 1. dark/light mode 文書に canonical color group を追加
      （design mapping 表どおり、alias 優先・provisional 直接値）
- [ ] 2. density 文書 2 件に `space` group を追加
- [ ] 3. `synvelis-theme` contract test で 4 組合せの canonical key
      解決・期待値一致・fallback 非一致を assert → red→green 確認
- [ ] 4. `synvelis-app/tests/theme_parity.rs` を追加: shipped 文書を
      `include_str!` で load → `ThemeCatalog::resolve` →
      `ViewMount { theme: Some(..) }` mount → settle → observe/drawn
      が plan を返すこと + 画面 semantic key fixture 対応表
- [ ] 5. `mise run fmt-check` + 対象 package の check/lint/test
      （theme・app-gpui lane）を実行
- [ ] 6. evidence/sv177/ に parity 記録・2scale×2theme 意味論差分・
      NotRun 項目を記録
- [ ] 7. doc chain（task/test-spec/ir/exp）を実績で更新し commit

## Dependencies and Risks

- upstream pin `cbcc834` の `ViewPalette` read key 集合が正本 —
  pin 更新時は mapping 表の再突合が必要（test が検出する）。
- `{spacing.sm}` 等の cross-group alias が upstream で解決されるかは
  実装時に test で確認。未対応なら直接値に切替（design の範囲内）。
- `control` group 名が mode（color）/density（dimension）で共有 —
  upstream は token path 単位 merge のため競合しないことを確認済み。
- gpui-test lane は palette 内部値を観測しない — parity は
  ResolvedTheme レベル + mount 受理に留まる（design で明示済み）。

## Task Breakdown

- Task A: theme assets canonical 語彙化 + synvelis-theme contract test
- Task B: synvelis-app gpui-test parity test + 画面 fixture
- Task C: evidence + doc chain 更新 + 検証

## Verification

- [ ] `mise run test synvelis-theme`
- [ ] `mise run check synvelis-theme` / `mise run lint synvelis-theme`
- [ ] `mise run test-app-gpui synvelis-app`（gpui-test lane）
- [ ] `mise run check-app-gpui synvelis-app` / `lint-app-gpui`
- [ ] `mise run check-app-native synvelis-app` / `lint-app-native`
      （native 非回帰 — `ViewMount.theme` 消費点）
- [ ] `mise run fmt-check`
- [ ] `mise run check-headless-deps`
- [ ] review-protocol（design は実施済み・blocker なし）

## Open Questions / Gaps

- `state.disabled`/`viz.waveform_rms`/light `bg-input`/`bg-active`
  の値は provisional — RF#631/SV#114 design review で確定。
- `ViewPalette` public 化は upstream RF issue 候補（本 slice 外）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（Coordinator dispatch worker が
  実装。subagent/helper worker は使わない — 共通契約どおり）
- Independent work streams: なし（A→B→C は順序依存）
- User approval required before dispatch: review-protocol で代替
  （design approved・blocker なし）
- If approved: discover available implementation and delegation capabilities in the current environment, then execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the same dependency graph and verification matrix.

- 実施形態: inline execution（dispatch worker が直接実装）

## Review Handoff

- reviewer focus: alias 解決が実際に効いているか（fallback ではなく
  製品値が返るか）、gpui-test lane の parity 主張が観測可能な範囲に
  留まっているか、provisional 注記が確定値と区別されているか。
- missing dependencies: なし。`ViewPalette` private は既知の制約
  として design/spec に記録済み。
