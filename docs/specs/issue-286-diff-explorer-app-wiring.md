---
id: "SPEC-286DiffExplorerAppWiring02"
type: "spec"
status: "approved"
title: "Issue #286 spec — CB-09 Difference Explorer app wiring"
created: "2026-11-18"
updated: "2026-11-18"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/286"
    - "docs/discovery/issue-286-diff-explorer-app-wiring.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/discovery/issue-286-diff-explorer-app-wiring.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/issue-286-diff-explorer-app-wiring.md"
    - "docs/test-specs/issue-286-diff-explorer-app-wiring.md"
    - "docs/test-specs/cb-09-diff-explorer.md"
---

# Issue #286 spec — CB-09 Difference Explorer app wiring

## 目的

CB-09 Difference Explorer の headless view-model を
`synvelis-app` へ実配線し、ユーザー操作から compare /
adopt-preview / adopt までの一連の経路を成立させる。

## 要件

### R1: op 面

- `syn.variant-diff.visual` — view の projection（view-model
  の read 面）を返す。
- `syn.variant-diff.view` — view lifecycle（mount/open/
  set-target 等）を制御する。
- `syn.variant-diff.action` — view action（selection /
  layer visibility / compare / adopt-preview / adopt）を
  dispatch する。

### R2: authority 境界

- compare / adopt-preview / adopt は明示的な
  branch/variant target pin を要求し、pin 無し・stale
  revision・unknown branch は fail-closed で reject する。
- adopt は明示 approval + CAS 検証を経てのみ commit し、
  preview intent は commit 経路へ到達しない。
- view-local action（selection・audience・layer visibility
  等）は project authority を変更せず、persisted target
  無しの fresh view でも成立する。

### R3: evidence の正直性

- semantic / signal / perceptual の 3 層を分離し、
  `Available` / `Unavailable` / `Unknown` / `Unsupported`
  を保持する。unavailable な capability を zero-diff /
  equal に変換しない。
- signal provider の出力は `AuxLayerEvidence` へ
  provenance を保持したまま写像する。

### R4: app/gateway 分離

- app は `synvelis-gateway` 型を名指ししない。
  compare/adopt dispatch は workspace 層の
  `VariantDiffEngine` facade 経由とする。

### R5: project 切替

- project 切替時に variant-diff engine・session・
  target・publish observation を全てクリアし、別 project
  の session/cache を誤再利用しない。

### R6: mount

- `ViewSpec` mount helper で Difference Explorer view を
  mount 可能とし、gpui-test lane で smoke を確認する。

## 受入条件

- AC-1: 上記 op が `service.rs` dispatch に登録され、
  `ui_ops.rs` に実装されている。
- AC-2: workspace 側 `VariantDiffEngine` が opaque な
  `VariantPorts` を所有し、compare/adopt-preview/adopt の
  typed dispatch を提供する。
- AC-3: app 側に view-local と authority action の別経路が
  ある（`apply_variant_diff_view_local`）。
- AC-4: `cb09_variant_diff` app test suite と
  `cb09_diff_explorer` workspace test suite が全件 pass。
- AC-5: 実 GPUI window / plugin dock / RT / PCM /
  perceptual acceptance は本 slice の範囲外（synvelis#288、
  CB-11 gate）として明示的に残す。

## 範囲外

- gateway typed contract の変更（CB-10）。
- cross-gate 受入（CB-11）。
- 実ウィンドウ/pixel/RT 証跡（synvelis#288）。

## 関連

- 前提: `docs/discovery/issue-286-diff-explorer-app-wiring.md`
- 後続: `docs/designs/issue-286-diff-explorer-app-wiring.md`、
  `docs/plans/issue-286-diff-explorer-app-wiring.md`、
  `docs/tasks/issue-286-diff-explorer-app-wiring.md`
- 検証: `docs/test-specs/issue-286-diff-explorer-app-wiring.md`
- headless 契約: `docs/test-specs/cb-09-diff-explorer.md`
