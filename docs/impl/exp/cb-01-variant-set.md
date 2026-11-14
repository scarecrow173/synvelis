---
id: "EXP-cb01-variant-set"
title: "CB-01 VariantSet/Scope/Lineage 設計実験メモ (synvelis#278)"
created: "2026-11-14"
updated: "2026-11-14"
---

# 実験記録: CB-01 Creative Branch headless スライス（synvelis#278）

## E1 branch-local ID の衝突回避方式

**仮説**: branch が mint する entity ID は live entity・兄弟
branch と衝突してはならない（CB-MODEL-06）。adopt 時に
remap する方式と、発行時点で live 空間を予約する方式の 2 案を
比較した。

- 案 A（adopt 時 remap）: candidate 内 ID を live 空間へ写像。
  lineage に old→new の対応表が必要で、undo/redo・再 adopt・
  sibling 間の一意性証明が複雑化する。
- 案 B（live-space 予約・採用）: `ReservingIdGenerator` が
  `next()` を `ProjectController::draw_raw_id()` へ転送し、
  `reseed()` を no-op にする。発行済み raw は単調増加で再利用
  されないため衝突は構造上不可能。`peek_raw_id()` を
  controller に追加して reseed なしの覗き見を可能にした。

**観察**: 案 B で `BranchLineage.local_ids` は remap 表ではなく
「どの tagged ID をこの branch がいつ mint したか」の provenance
記録に純化し、adopt の ambiguous-lineage 検査が単純な集合包含で
書ける。fixture `cb_model_06_branch_local_ids_no_collision_and_lineage`
で同一 ID のまま adopt されることを確認。

## E2 candidate payload の置き場所

**仮説**: `VariantSet`/`Branch` に artifact blob を埋め込まず、
immutable 参照 + digest で済むか。

- project snapshot 内に candidate を持つ案は snapshot サイズと
  undo payload を肥大化させ、CAS 原子性と相性が悪い。
- `BranchContentStore`（content-addressed bytes、project 外）に
  分離し、`BranchContentRef::Captured` が digest+schema+scope
  fingerprint を pin する方式を採用。decode 時に digest・schema・
  JSON・`validate_references` を再検証する fail-closed oracle
  を `decode_candidate` に集約した。

**観察**: store は in-memory 実装のみだが trait 境界により
後続で disk/asset-store 実装へ差し替え可能。orphan payload
（discard 後・失敗時の書き込み）は参照なし bytes として無害。

## E3 adopt transplant の再利用面

**仮説**: adopt は既存 apply machinery（`PlannedWrite` +
`entity::write_all` + preflight）で表現できるか。

- `plan_adopt_writes`（`pub(crate)`）が candidate/live の scope
  object 集合を比較し、`Added`/`Modified` の `PlannedWrite` を
  生成。payload は `entity::payload_for` 経由。
- 未対応 kind（comping 系など apply surface 外）は typed
  `Unsupported`、lineage 不明の新規 entity は
  `AmbiguousLineage` で reject — 推測正規化なし。
- `entity::write_all` が processor の plugin-instance identity
  同期と clip/lane/processor/port membership reconcile を既に
  持つため、`InsertProcessor` adopt でも identity 不変条件が
  保たれることを確認（`cb_model_rcp_01_missing_plugin_safe_adopt_and_reject`）。

**観察**: apply の `after` には candidate project を渡す
（scope 内の望ましい post-state）。`VariantOp::Adopt` の
`after` フィールドは `Box<Project>` とし enum サイズを抑制
（clippy `large_enum_variant` 対応）。

## E4 schema version の上げ方

**観察**: `variant_sets` は `#[serde(default)]` なので v13 body
はそのまま deserialize できる。ladder には明示 step
（`migrate_v13_variant_sets`、identity projection）を追加し、
監査可能な version 前進として記録。既存の版固定 assert
（`md_a.rs`・`tim_02_smf.rs`）のみ 13→14 に追従が必要だった。
