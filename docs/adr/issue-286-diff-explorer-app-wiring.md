---
id: "ADR-286DiffExplorerAppWiring05"
type: "adr"
status: "accepted"
title: "Issue #286 — Difference Explorer dispatch を synvelis-workspace の VariantDiffEngine facade に集約する"
created: "2026-11-18"
updated: "2026-11-18"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/286"
    - "docs/discovery/issue-286-diff-explorer-app-wiring.md"
    - "docs/specs/issue-286-diff-explorer-app-wiring.md"
    - "docs/designs/issue-286-diff-explorer-app-wiring.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/issue-286-diff-explorer-app-wiring.md"
  implemented-by: []
  depends-on:
    - "docs/designs/issue-286-diff-explorer-app-wiring.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/gw-01-gateway-contract-crate-boundary.md"
    - "docs/adr/cb-00-creative-branch.md"
---

# Issue #286 — Difference Explorer dispatch を synvelis-workspace の VariantDiffEngine facade に集約する

## Context

CB-09 Difference Explorer の headless view-model は
workspace に存在したが、`synvelis-app` から compare /
adopt-preview / adopt を駆動する経路が無かった。
一方 `synvelis-app` は `synvelis-gateway` に依存しない
構成であり、`VariantPorts`/`simulate`/`execute`/
`VariantCommitError` 等の gateway 型を app 側で
名指し・分岐できない。

compare/adopt には CB-08 orchestrator・CAS adopt engine・
asset blob probe・adapter table が必要で、これらは全て
gateway 側の `VariantPorts` に集約されている。

## Decision

compare/adopt-preview/adopt の dispatch を
`synvelis-workspace` の `VariantDiffEngine` facade に
集約する。engine は opaque な `VariantPorts` を所有し、
`VariantDiffIntent`（compare / adopt-preview / adopt）を
typed dispatch する。`Adopt` のみが commit 経路へ進み、
compare/preview intent は commit path で malformed reject
とする。app は `syn.variant-diff.visual`/`view`/`action`
op 経由でのみ engine を呼び、workspace の型のみを import
する。

view-local action（selection/audience/layer visibility 等）
は authority context を要求しないため、別経路
`apply_variant_diff_view_local` で適用する。

adapter 登録の authoritative な一箇所は
`VariantPorts::standard` とし、facade 側での手動
install は行わない（CB-07 `ArrangementAdapter` は
synvelis#539/#542 で standard 登録済み）。未 merge
domain は fail-closed を維持する。

## Consequences

### Positive

- app→gateway 依存を導入せず、crate 境界を維持できる。
- CAS pin・session 連続性・publish observation の所有境界が
  workspace に一元化される。
- intent 種別で dispatch を分岐するため、preview intent が
  誤って commit へ流れる経路を型レベルで排除できる。
- evidence の semantic/signal/perceptual 3 層と
  `Available`/`Unavailable`/`Unknown`/`Unsupported` を
  facade で保持でき、zero-diff への矯正変換を防げる。

### Negative / Trade-offs

- workspace facade が gateway の dispatch 意味論を再表現
  するため、gateway 側 contract 変更時に facade の追従が
  必要（CB-10 typed contract slice で吸収予定）。
- per-view state と engine/session の二重管理が生じるため、
  project 切替時の全クリアを明示的に実装する必要がある。

## Alternatives Considered

- **app が gateway 型を直接保持・dispatch**: app→gateway
  依存を導入し crate 境界に反するため却下。
- **workspace に session を持たず app が都度構築**:
  CAS pin・session 連続性・publish observation の所有境界が
  曖昧になるため却下。
- **adopt 成功で compare target を自動 mint**: stale-context
  検証を迂回する経路になるため却下。
- **facade 側で adapter を手動 install**: upstream
  `VariantPorts::standard` が authoritative 登録を持つため
  二重管理となり却下（EXP-1 で実測・除去済み）。

## References

- spec: `docs/specs/issue-286-diff-explorer-app-wiring.md`
- design: `docs/designs/issue-286-diff-explorer-app-wiring.md`
- plan: `docs/plans/issue-286-diff-explorer-app-wiring.md`
- 実装記録: `docs/impl/ir/issue-286-diff-explorer-app-wiring.md`
- 実験記録: `docs/impl/exp/issue-286-diff-explorer-app-wiring.md`
- 関連 ADR: `docs/adr/gw-01-gateway-contract-crate-boundary.md`、
  `docs/adr/cb-00-creative-branch.md`、
  `docs/adr/issue-539-arrangement-adapter.md`
