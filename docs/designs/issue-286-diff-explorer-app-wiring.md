---
id: "DESIGN-286DiffExplorerAppWiring03"
type: "design"
status: "approved"
title: "Issue #286 design — CB-09 Difference Explorer app wiring"
created: "2026-11-18"
updated: "2026-11-18"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/286"
    - "docs/specs/issue-286-diff-explorer-app-wiring.md"
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
    - "docs/specs/issue-286-diff-explorer-app-wiring.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/plans/issue-286-diff-explorer-app-wiring.md"
    - "docs/adr/issue-286-diff-explorer-app-wiring.md"
    - "crates/synvelis-workspace/src/variant_diff.rs"
    - "crates/synvelis-app/src/ui_ops.rs"
---

# Issue #286 design — CB-09 Difference Explorer app wiring

## 概要

`VariantDiffEngine` を `synvelis-workspace` に置き、
`synvelis-app` はその facade 経由でのみ Difference Explorer
を操作する。app は gateway 型を一切名指ししない。

## コンポーネント

### VariantDiffEngine（workspace）

- opaque な `VariantPorts`（CB-08 comparison orchestrator /
  CAS adopt / asset blob probe）を所有する。
- `VariantDiffIntent`（compare / adopt-preview / adopt）を
  typed dispatch する。`Adopt` のみが commit 経路へ進み、
  compare/preview intent は commit path で malformed reject。
- CB-08 provider seam を `AuxLayerEvidence` へ provenance
  保持で写像する signal provider adapter を内包する。
- `VariantPorts::standard` が merge 済み adapter
  （CB-07 `ArrangementAdapter` 含む）を authoritative に
  登録する。未 merge domain は fail-closed。

### apply_variant_diff_view_local（workspace）

authority 非依存の view-local action subset
（selection/audience/layer visibility 等）を適用する
純粋関数。persisted target 無しの fresh view でも成立する。

### syn.variant-diff.* ops（app）

`service.rs` dispatch に登録し `ui_ops.rs` で処理:

- `syn.variant-diff.visual` — view projection を返す。
- `syn.variant-diff.view` — view lifecycle。
- `syn.variant-diff.action` — action dispatch。
  view-local action は `apply_variant_diff_view_local` へ、
  authority action は explicit pin から context を導出して
  engine へ dispatch する。

### per-view state / project 切替

- `ViewState.variant_diff` が per-view state を保持。
- `UiOpsState` が engine/session/target/publish observation
  を保持し、project 切替で全クリアする。

## 不変条件

1. adopt は明示 approval + CAS のみ commit。preview は
   commit しない。
2. compare/adopt は explicit branch/variant pin を要求。
   ambient/stale pin は reject。
3. evidence の `Unavailable`/`Unknown`/`Unsupported` を
   zero-diff に変換しない。
4. app は gateway 型を import しない。
5. adopt 成功後も compare target を自動 mint しない —
   view は untargeted を正直に維持する。

## 代替案と却下理由

- **app が gateway 型を直接保持**: app→gateway 依存を
  導入するため却下（AC-05 / spec R4）。
- **workspace に session を持たせず app が都度構築**:
  CAS pin・session 連続性・publish observation の所有境界が
  曖昧になるため却下。
- **adopt 成功で compare target を自動設定**: stale-context
  検証を迂回する経路になるため却下。

## 検証

- `mise run check-app-native` / `lint-app-native` /
  `test-app-gpui`（gpui-test lane。実 window/pixel は範囲外）。
- workspace/app の `cb09_*` test suite。

## 関連

- spec: `docs/specs/issue-286-diff-explorer-app-wiring.md`
- plan: `docs/plans/issue-286-diff-explorer-app-wiring.md`
- ADR: `docs/adr/issue-286-diff-explorer-app-wiring.md`
- 実装記録: `docs/impl/ir/issue-286-diff-explorer-app-wiring.md`
- 検証証跡: `evidence/sv286/app-wiring-verification.md`
