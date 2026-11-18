---
id: "DISC-286DiffExplorerAppWiring01"
type: "discovery"
status: "resolved"
title: "Issue #286 discovery — CB-09 Difference Explorer app wiring の境界確定"
created: "2026-11-18"
updated: "2026-11-18"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/286"
    - "https://github.com/scarecrow173/synvelis/issues/284"
    - "https://github.com/scarecrow173/synvelis/issues/285"
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
    - "docs/impl/ir/cb-09-diff-explorer.md"
    - "docs/impl/ir/cb-08-compare-orchestrator.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/issue-286-diff-explorer-app-wiring.md"
    - "docs/test-specs/cb-09-diff-explorer.md"
---

# Issue #286 discovery — CB-09 Difference Explorer app wiring の境界確定

## 背景

CB-09 Difference Explorer の headless view-model は
`cb-09-diff-explorer` slice（`docs/test-specs/cb-09-diff-explorer.md`、
`docs/impl/ir/cb-09-diff-explorer.md`）で実装済みだったが、
`VariantDiffView`/`VariantDiffInput`/`apply_variant_diff_action` 等は
workspace crate の純粋関数・read-model に留まり、`synvelis-app` から
到達する op・view lifecycle・authority への intent dispatch 経路が
存在しなかった。本 Issue `synvelis#286` はその「後始末」slice として、
Difference Explorer をアプリケーションへ実配線することを目的とする。

先行 slice の状態:

- CB-07 arrangement adapter（synvelis#284）— merge 済み。
  `VariantPorts::standard` への adapter 登録は synvelis#539/#542 で
  upstream 解消。
- CB-08 comparison orchestrator / provider seam（synvelis#285）—
  merge 済み。`ComparisonOrchestrator`・`AuxLayerEvidence`・
  `VariantPorts`（gateway CAS/adopt・asset blob probe）が利用可能。
- CB-10 gateway typed contract・CB-11 cross-gate — 別 slice。

## 調査で確定した論点

### app は gateway 型を名指しできない

`synvelis-app` は `synvelis-gateway` に依存しない構成のため、
`VariantPorts`/`simulate`/`execute`/`VariantCommitError` 等の
gateway 型を app 側で直接保持・分岐できない。compare/adopt の
dispatch は workspace 層の facade に閉じ込める必要がある。

### view-local action と authority action の分離

`VariantDiffViewAction` には selection/audience/layer visibility
等の view-local 操作と、compare/adopt-preview/adopt の authority
操作が混在する。fresh view（persisted target 無し）でも
view-local 操作は成立すべきだが、`variant_diff_input` は
context を fail-closed で要求するため、view-local subset を
authority 非依存で適用する別経路が必要と判明した
（`apply_variant_diff_view_local`）。

### adopt は compare target を mint しない

adopt commit 成功後も view の compare target は自動設定されない
設計が正しい — target は明示 pin のみが権威を持ち、adopt 結果を
ambient target に昇格させると stale-context 検証を迂回する経路に
なる。view は untargeted/unavailable を正直に維持する。

### project 切替時の session 衝突

per-view `VariantDiffSession` が bundle root と view id で
管理される場合、project 切替後に同一 bundle 内容（identical
copy）の別 project で branch id が衝突し、前 project の
session/cache が誤って再利用される経路があった。project
切替で engine・session・target・publish observation を全て
クリアする必要がある（回帰 test
`cb09_app_project_switch_clears_variant_diff_state` で固定）。

### ArrangementAdapter の stale verdict

CB-09 view-model 側の `DomainAdapterUnmerged` 判定が
Arrangement を「adapter 未 merge」と誤判定していた
（CB-07 merge 済みのため陳腐化）。supported へ修正。
また facade 実装時点では `VariantPorts::standard` が
ArrangementAdapter を未登録だったため facade 側で install
していたが、synvelis#539/#542 の upstream merge 後は
standard table が自前登録するため手動 install を除去した。

## 結論

- dispatch facade は `synvelis-workspace` の
  `VariantDiffEngine` に置く（app は workspace facade のみ
  経由、gateway 型非依存を維持）。
- op 面は `syn.variant-diff.visual`/`view`/`action` の 3 本。
- view-local と authority action は別経路で適用する。
- evidence は semantic/signal/perceptual の 3 層と
  `Available`/`Unavailable`/`Unknown`/`Unsupported` を
  保持し、unavailable を zero-diff に変換しない。
- adopt は明示 approval + CAS、preview は commit しない。

上記を spec → design → plan → task へ展開する。
詳細は `docs/specs/issue-286-diff-explorer-app-wiring.md` 以降を参照。

## 関連

- 後続: `docs/specs/issue-286-diff-explorer-app-wiring.md`
- CB-09 headless 契約: `docs/test-specs/cb-09-diff-explorer.md`、
  `docs/impl/ir/cb-09-diff-explorer.md`
- CB-08 orchestrator: `docs/impl/ir/cb-08-compare-orchestrator.md`
- 実 GPUI/plugin/RT 受入: synvelis#288 系 gate、CB-11 cross-gate
  （本 slice の範囲外として明示的に残す）
