---
id: "DISC-sv511aB2cD4eF6gH8iJ0kL1m"
type: "discovery"
status: "resolved"
title: "Issue #511 discovery — CB-07 adopt/render materialization・PPQN grid・anchor tempo-map の残件調査"
created: "2026-11-22"
updated: "2026-11-22"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/511"
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
    - "docs/impl/ir/cb-07-arrangement-adapter.md"
    - "docs/impl/exp/cb-07-arrangement-adapter.md"
    - "docs/designs/variant-sets-vcs-restore.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/issue-511-cb07-materialization.md"
  verifies: []
  verified-by: []
  references:
    - "evidence/sv284/acceptance-ledger.md"
  defers: []
  deferred-by: []
---

# Issue #511 discovery — CB-07 materialization 残件

## 背景

synvelis#284（CB-07 Arrangement 候補 Adapter）の IR Follow-ups
（`docs/impl/ir/cb-07-arrangement-adapter.md`）で owner 不在のまま
残った 3 残件を engine 側で引き受ける slice。

1. scope-removal を含む candidate の adopt/render materialization
2. PPQN 変更と variant candidate payload の grid 整合
3. absolute anchor/extent clip の tempo-map 写像 policy

前段の統合済み依存: PR #524（sv#513・apply op surface 拡張）、
PR #532（sv#517・永続 BranchContentStore + RuntimePublish 配線）。
本調査は `origin/main` `07b67f5b` 時点の実測に基づく。

## 観測した fail-closed 地点（実測）

### 残件1 — adopt

- `variant/engine.rs` `plan_adopt_writes`: `origin_objects` のうち
  live scope に残り candidate scope を外れた entity を一律
  `VariantError::Unsupported` で拒否（「scope entity removal is
  not supported」）。entity が candidate に実在する move-out と
  真の削除を区別していない。
- `apply/entity.rs` `write_entity`: `ChangeKind::Removed` の
  delete arm が存在しない（insert のみ）。
- `apply/plan.rs` `change_supported`: VCS `apply_selected` 経路は
  全 kind の `Removed` を `ApplyError::Unsupported` で拒否
  （sv#513 で確定した共有 surface — 本 slice では変更しない）。
- `render_input.rs` `resolve_render_project`: `plan_adopt_writes`
  + `entity::write_all` を再利用するため adopt と同一 plan で
  追従する構造。半 materialize は構造的に起きない。

### 残件2 — PPQN

- `ppqn.rs` `reproject_ppqn_with`: live project の musical-tick
  値と arrangement scope 宣言のみ変換。コメントに "Stored
  candidate payloads live in the branch content store outside this
  walk" と明示。
- `BranchContentRef::Captured` payload は content-addressed store
  に保持され、project walk の対象外。fork 後に PPQN が変わると
  candidate の musical 値が旧 grid のまま adopt/render に
  transplant され得る。
- adopt で旧 adopted branch は `freeze_adopted_branch` により
  `Captured` として凍結される — frozen 履歴も再 adopt 可能なため
  stale-grid リスクは frozen payload にも存在する。

### 残件3 — absolute anchor

- `variant/scope.rs` `arrangement_clips`: `TimeAnchor::Musical` +
  `ClipExtent::Musical` のみを所有導出。absolute は除外。
- `variant/arrangement.rs`: `absolute_anchor` capability =
  `Unsupported`、`boundary_dependencies` が `AbsoluteAnchor` を
  型付き報告。`docs/impl/exp/cb-07-arrangement-adapter.md` E3 で
  tempo-map conversion policy 不在として棄却済み。

## 代替案の比較

### 残件1

- 案 A: adopt 経路限定で `Removed` write を materialize し、
  VCS `apply_selected` の `change_supported` は据え置く。
  → cascade semantics が必要なのは scope-removable kind
  （Clip/Note）のみで、他 kind は typed `Unsupported` を維持。
  採用。
- 案 B: `change_supported` を拡張して VCS apply も `Removed` を
  受理。→ VCS diff/removal cascade は sv#513 の別設計であり
  slice 横断の再設計になる。却下。

### 残件2

- 案 A: `Captured` payload を持つ variant set が存在する間
  `SetProjectPpqn` を typed 拒否。→ 変換 policy・payload 再符号化・
  digest/scope-digest 再計算を伴う再射影は store/engine の大改修。
  fail-closed で旧 grid transplant を構造的に防ぐ。採用。
- 案 B: adopt 時に payload を再射影。→ payload decode→変換→
  再 encode→新 digest→`Captured` ref 更新が必要で、frozen 履歴も
  同じ stale-grid リスクを持つため結局全 payload が対象。slice
  規模を超える。却下（後続 slice へ記録）。

### 残件3

- 案 A: tempo map で absolute↔musical を変換して所有拡張。
  → clip extent の musical 換算は tempo 曲線全域に依存し、
  partial overlap の帰属 policy が未確立。silent な近似は
  fingerprint/conflict の正本性を損なう。却下。
- 案 B: `AbsoluteAnchor` typed fail-closed を維持し、policy
  未確立であることを ADR/テストに記録。採用。

## 結論

3 残件の設計判断が確定: (1) adopt/render で Clip/Note の
`Removed` write を materialize、他 kind は typed `Unsupported`
維持。(2) `Captured` payload 存在時の `SetProjectPpqn` を typed
拒否。(3) absolute anchor は `AbsoluteAnchor` fail-closed 継続。
