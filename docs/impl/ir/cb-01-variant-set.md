---
id: "IMPL-cb01-variant-set"
type: "impl"
status: "completed"
title: "CB-01 VariantSet/Scope/Lineage・ProjectController Fork/Edit/Adopt/Undo・永続Schema — Headless スライス"
created: "2026-11-14"
updated: "2026-11-14"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/278"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/variant/mod.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/types.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/error.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/scope.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/store.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/adapter.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/engine.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/cb01_variant.rs"
      - type: "file"
        path: "docs/impl/ir/cb-01-variant-set.md"
      - type: "file"
        path: "docs/impl/exp/cb-01-variant-set.md"
      - type: "file"
        path: "docs/test-specs/cb-01-variant-set.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/id.rs"
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-project/src/project.rs"
      - type: "file"
        path: "crates/synvelis-project/src/controller.rs"
      - type: "file"
        path: "crates/synvelis-project/src/migration.rs"
      - type: "file"
        path: "crates/synvelis-project/src/store.rs"
      - type: "file"
        path: "crates/synvelis-project/src/apply/mod.rs"
      - type: "file"
        path: "crates/synvelis-project/src/control.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/md_a.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/tim_02_smf.rs"
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
    - "docs/test-specs/cb-01-variant-set.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/project-model.md"
    - "docs/designs/project-model-design.md"
  derived-by: []
  verifies:
    - "docs/specs/project-model.md"
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/cb-01-variant-set.md"
    rejected: []
---

# Implementation Record: CB-01 VariantSet/Scope/Lineage Headless スライス（synvelis#278）

## Summary

Issue #278 の CB critical path 基盤のうち、headless に完結する
model + schema + Fake Domain Adapter fixture スライスを実装した。
Project-authoritative な versioned `VariantSet`/`Branch` モデル、
`ProjectController` single-writer CAS 経由の
Fork/Edit/Adopt/Discard/Undo、schema v14 migration、typed
`DomainAdapter` 境界と `FakeDomainAdapter`、および
`tests/cb01_variant.rs` の fixture 群を追加した。

## Context

- 対象 Issue: https://github.com/scarecrow173/synvelis/issues/278
  （CB-01 — #279〜287 の前提、#294 を正式に block）。
- 本スライスの範囲は **headless に実施可能な typed model +
  永続 schema + Fake Domain Adapter fixture** に限定。実
  ドメイン Adapter（CB-02〜07 = #279〜284）、Comparison
  Orchestrator（#285）、GPUI（#286）、Action/Gateway 統合
  （#287）、`crates/synvelis-workspace/` の session glue 配線は
  後続 Issue / DEFER（PR 本文に記録）。
- 既存契約（single-writer CAS・typed stable ID・undo/redo・
  `validate_references` oracle・ID watermark）の内側に収める
  ことが要件。

## Implementation

### Changes

- `src/variant/` 新設（`types`/`error`/`scope`/`store`/
  `adapter`/`engine`/`mod`）。
- `id.rs`: `VariantSetId`・`BranchId` を `define_id!` 規約で追加。
- `project.rs`: `variant_sets: BTreeMap<VariantSetId, VariantSet>`
  （`#[serde(default)]`）追加、`validate_references` から
  `variant::engine::validate_persisted` を呼出、
  `max_issuable_id_raw` に variant ID を算入。
- `controller.rs`: `peek_raw_id()` 追加（live-space 予約用）。
- `migration.rs`: v13→v14 `migrate_v13_variant_sets`（identity
  pass-through）を ladder に追加。
- `store.rs`: `SCHEMA_VERSION` 13→14。
- `apply/mod.rs`: `entity`/`plan` を `pub(crate)` 化（adopt
  planner が既存 machinery を再利用するため）。
- `control.rs`: `ControlOwner` に `PartialOrd`/`Ord`/`Hash`
  derive 追加（scope closure の `BTreeSet` 格納用）。
- `lib.rs`: `pub mod variant` + re-export。
- `tests/cb01_variant.rs`: 17 fixture test 新設。
  `tests/md_a.rs`・`tests/tim_02_smf.rs` の schema 版固定 assert
  を 13→14 に追従。

### Decisions During Implementation

- **live-space ID 予約を採用**: branch 内 ID 払い出しを
  `ReservingIdGenerator` 経由で live controller generator
  （`draw_raw_id`/`peek_raw_id`）へ転送。adopt 時 ID remap を
  不要にし、live/sibling との衝突を構造で不可能にした。
  `BranchLineage.local_ids` は remap 表ではなく provenance
  記録に純化（詳細は exp E1）。
- **candidate = full project（Fake Adapter）**: scope 外を含む
  project 全体を clone するが、scope fingerprint と adopt
  planner が scope 内に閉じるため正しさは保たれる。実 adapter
  は scoped payload を持つ想定（CB-02+）。
- **scope 除去は unsupported**: CB-01 では entity 削除を伴う
  edit を typed reject。推測による正規化を行わない。
- **`StripDomainMask` は `StripField` enum 集合**: clippy
  `struct_excessive_bools`（5 bools）対応で bool struct から
  `BTreeSet<StripField>` 背の newtype へ変更。
- **adopt の `after` は `Box<Project>`**: `VariantOp` enum の
  サイズ抑制（`large_enum_variant`）。

## Related Experiments

- `docs/impl/exp/cb-01-variant-set.md` — live-space ID 予約・
  candidate payload 分離・adopt transplant 再利用面・schema
  version 方針の検討記録（adopted）。

## Validation

- `mise run fmt-check` — Pass。
- `mise run check synvelis-project` — Pass（`--locked
  --all-targets`）。
- `mise run lint synvelis-project` — Pass（clippy `-D warnings`）。
- `mise run test synvelis-project` — Pass（全 suite green。
  `cb01_variant.rs` 17 件 + `variant::engine` unit 1 件を含む）。
- `mise run check <pkg>` — synvelis-project 直接依存 9 クレート
  （synvelis-app / synvelis-gateway / synvelis-workspace /
  synvelis-player / synvelis-render / synvelis-library /
  synvelis-recording / mcp-poc / recording-poc）すべて Pass。
- 詳細は `evidence/sv278/verification.md`・
  `evidence/sv278/acceptance-ledger.md`。

## Risks

- Fake Adapter の full-project candidate は scoped payload より
  重いが、CB-01 の fixture 目的では許容範囲。実 adapter 導入時に
  scoped payload へ置き換える余地を `DomainAdapter` trait で
  確保済み。
- `scope_objects` の ownership 規則（MIDI source 排他・range 内
  connection のみ所有等）は CB-01 時点の model 権威。実ドメイン
  要件で refine される可能性がある（CB-02+ で検証）。
- scope entity 削除を伴う edit は CB-01 では typed reject。
  削除 semantics が必要になった時点で adapter 側の拡張が必要。

## Follow-ups

- 実ドメイン Adapter（CB-02〜07 = Issue #279〜284）。
- Comparison Orchestrator（#285）、GPUI（#286）、Action/Gateway
  統合（#287）。
- `crates/synvelis-workspace/` session glue 配線（本スライスは
  編集範囲外のため DEFER）。
- scope entity 削除・take/comping 系 transplant（apply surface
  外 kind の `Unsupported` 解消）。
- 実 OS/実 Plugin/実 RT/実 Render での受入（`evidence/sv278/`
  の NotRun/Deferred を後続 Issue で Pass/NotRun 記録）。
