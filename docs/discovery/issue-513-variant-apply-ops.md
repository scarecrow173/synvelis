---
id: "DISC-513a1b2c3d4e5f6g7h8i9j0k"
type: "discovery"
status: "resolved"
title: "issue-513-variant-apply-ops — variant/VCS apply op surface 残件の探索結果"
created: "2026-11-20"
updated: "2026-11-20"
owners: ["scarecrow173"]
relations:
  source: ["https://github.com/scarecrow173/synvelis/issues/513"]
  derives-from: []
  derived-by: ["docs/specs/issue-513-variant-apply-ops.md"]
---

# Discovery: issue-513-variant-apply-ops

## 背景

synvelis#96/#279/#281/#282/#283 で deferred された Creative Branch / VCS
apply の残件を 1 slice に統合。main `acc0198c` 実測の fail-closed 観測点:

- `apply/plan.rs`: `ChangeKind::Removed` → `ApplyError::Unsupported`
  ("removal changes are deferred (cascade design)")。
- `apply/entity.rs`: comping 4 kind (`TakeGroup`/`Comp`/`CompRegion`/
  `CompTransition`) は `write_entity` arm 済みだが `supported_kind` 外。
  `VariantSet`/`VariantBranch` は entity extraction 自体が未実装。
- `ops.rs`: `ProjectOp` に binding/preset 変種なし。
  `variant/processing_range.rs` が "No ProjectOp mutates
  Processor.binding" を契約として明記。
- `variant/engine.rs`: `VariantOp` は Fork/EditAdopted/EditCandidate/
  Adopt/Discard のみ。`fork_variant_set` は adopted+1 candidate のみ。
  `VariantSet.branches` は `BTreeMap` で N>2 はデータモデル上対応済み。
- `variant/adapter.rs`: `is_processing_range_op` = 制御 op +
  `InsertProcessor` のみ。routing/send/sidechain/multi-output は全 adapter
  で fail-closed。mid-chain end は `validate_scope` で reject。

## 採否の決定（spec/adr へ引き継ぐ）

1. **Removed cascade** → `Unsupported` 維持。cascade 対象の包含検査・
   部分適用の半 transplant リスクに対し、dependent 収集設計が
   `src/restore/`・journal 層まで波及し本 slice の lease を超える。
   disposition を ADR で確定。
2. **CompOp replay** → whole-entity transplant を採用。`apply/entity.rs`
   の既存 `write_entity` arm が comping 4 kind を既に扱うため、
   `supported_kind` + `entity_value` + write ordering の拡張で
   `apply_selected`/`target_fingerprint` が typed replay する。
   `CompOp` 単位の選択的 replay は設計上不要（VCS apply は entity 粒度）。
3. **plugin binding/preset 差替** → `ProjectOp::SetProcessorBinding`
   を追加（binding のみ。preset は plugin 内部 opaque state で typed seam
   が存在しないため `Unsupported` 維持）。capability=強制一致を維持し、
   instrument-slot/strip-fields scope に confinement。
4. **set 内 branch 追加** → `variant::add_branch` +
   `VariantOp::AddBranch` を実装。N>2 candidate を headless で検証
   （CB-MIDI-01 literal lane）。
5. **routing 候補 op** → `Unsupported` 維持。main-path connection
   confinement を表現する安全モデルが未設計で、send/sidechain/
   multi-output/mid-chain end の受理可否も未確定。CB-FX-04 不変条件を
   守るため本 slice では op surface を広げない。
