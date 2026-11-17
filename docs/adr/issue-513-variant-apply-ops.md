---
id: "ADR-513a1b2c3d4e5f6g7h8i9j0k2"
type: "adr"
status: "accepted"
title: "issue-513-variant-apply-ops — apply op surface 拡張の採否決定"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source: ["https://github.com/scarecrow173/synvelis/issues/513"]
  derives-from: ["docs/discovery/issue-513-variant-apply-ops.md"]
  derived-by: ["docs/designs/issue-513-variant-apply-ops.md"]
---

# ADR: issue-513-variant-apply-ops

## 決定

### D1: `Removed` change cascade apply → `Unsupported` 維持

`ChangeKind::Removed` は `ApplyError::Unsupported` を維持する。
根拠: cascade 対象（子孫 entity・被参照 entity）の包含検査を正しく
行うには diff 逆適用と依存グラフ走査が必要で、部分選択時に「親だけ
消えて子が dangling」する半 transplant を防ぐ設計が `apply/` 単体を
超えて journal/restore 層の契約に波及する。本 slice の lease 外であり、
fail-closed 側に倒す方が安全。将来実装する場合は dependent closure
収集を plan 層に追加し、包含検査・1 CAS/Undo・stale/fingerprint
fail-closed をテストで pin する別 Issue とする。

### D2: comp/clip domain replay → whole-entity transplant（CompOp 選択的 replay は対象外）

VCS apply の replay 粒度は SemanticChange（entity）単位であり、
`CompOp` は comp 編集の op 粒度。`apply/entity.rs` の既存
`write_entity` arm が comping 4 kind を扱うため、`supported_kind`・
`entity_value`・write ordering の拡張で `apply_selected`/
`target_fingerprint` が typed replay する。`CompOp` を changeset に
混在させる設計は entity/field diff モデルと整合しないため採用しない。

### D3: plugin binding 差替 → `ProjectOp::SetProcessorBinding` 追加、preset は `Unsupported`

`Processor.binding` の差替は typed op として安全（processor ports/
chain/routing は不変、`plugin_instances.sync_processor` が identity
を再同期）。preset は plugin 内部の opaque state で `ProjectOp` の
typed seam が存在せず、fidelity 保証なしに transplant すると
silent misapplication になるため `Unsupported` 維持。

### D4: set 内 branch 追加 → `VariantOp::AddBranch` + `variant::add_branch` 実装

`VariantSet.branches` は `BTreeMap` で N>2 対応済み。新 branch は
`parent`/`created_from`/`content`（live mirror の Captured payload）/
`lineage` を持ち、1 controller command で atomic に追加する。

### D5: routing 候補 op → `Unsupported` 維持

`RemoveProcessor`/`ReorderProcessor`/send/sidechain/multi-output は
main-path connection confinement を表現する安全モデルが未設計。
無断再接続を防ぐ CB-FX-04 不変条件を守るため、本 slice では
op surface を広げず `Unsupported` を維持。mid-chain end も同様。
