---
id: "EXP-559AdoptUnobservable01"
type: "experiment"
status: "completed"
title: "Issue #559 impl EXP — adopt unobservable-removal fail-closed 実験記録"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/559"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/impl/ir/issue-559-adopt-unobservable-removal.md"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/issue-559-adopt-unobservable-removal.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/exp/issue-557-adopt-facts.md"
    - "docs/impl/exp/issue-553-intent-facts.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/impl/ir/issue-559-adopt-unobservable-removal.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #559 impl EXP — adopt unobservable-removal fail-closed 実験記録

## 実験サマリ

fail-closed の設置境界と reject 対象 kind 集合を実験的に絞り込んだ。
結論は 2 点: (1) 拒絶は gateway prepare 層ではなく domain
`plan_adopt_writes` に置く、(2) reject 集合は
`ModulationSource | ModulationAssignment` のみで
`RoutingConnection` は含めない。

## 採否記録（exp.jsonl と対応）

| EXP | 仮説 | 結果 | 要点 |
| --- | --- | --- | --- |
| EXP-1 | pre-553 stored candidate（send.remove cascade が send-owned modulation entities を削除した payload）は adopt で live に物質化される | confirmed | 修正前コードで再現 test が `AdoptOutcome::Applied` で Pass — send・LFO source・SendLevel target・assignment が全て live から除去。`evidence/sv559/reproduction.md` に記録 |
| EXP-2 | 拒絶は gateway prepare/effect 層に置くべき | rejected | `plan_adopt_writes` は `Removed` write 化直前に正確な `ObjectKind` を知る唯一の層で、SEC-14 fail-closed（`Unsupported`/`AmbiguousLineage`）の既存所有者。gateway 側の kind filter は owner 境界の下流で policy を重複させる |
| EXP-3 | reject 集合は `removal_supported_kind` のうち `ENUMERATED_KINDS` 外の全 kind | rejected | `RoutingConnection` は sv#550 send teardown 契約で「観測可能な `Send` 削除の内部構造」として edge ごと削除される既存受理境界（sv#553 `op_facts` RemoveSend arm と同じ分類）。これを reject すると合法な send teardown adopt（`cb06_strip_557_*` pin）を壊す |
| EXP-4 | reject 集合は `ModulationSource | ModulationAssignment` のみ、`plan_adopt_writes` で typed error | adopted | `entity::unverifiable_removal_kind` predicate + `VariantError::UnverifiableRemoval`。write 構築・適用の前に fail し、gateway `variant_reason` は `"unverifiable-removal"` に写像 |
| EXP-5 | 関連 live-path gap: `SetSendGainTarget` rebind の `release_send_level_target` が `ModulationAssignment` を削除するが `op_facts` が unverifiable を flag しない | confirmed・範囲外 | wire 合法 op で assignment 削除入り candidate を生成し得る経路。ただし本 slice は durable stored payload の adopt 境界であり、`synvelis#561` に記録して委譲 |

## 実測

- `cb_strip_559_adopt_unobservable_removal_rejected`（domain）:
  Pass — `try_adopt` が `VariantError::UnverifiableRemoval` を返し、
  live の send・modulation closure・revision・adopted branch が
  不変であることを assert。
- `cb06_strip_559_adopt_unobservable_removal_denied`
  （gateway）: Pass — wire prepare 段階で
  `PreparationRejected`（reason `variant-unverifiable-removal`）
  となり proposal/approval/commit には到達しない。live 非破壊。
- 非回帰: `cb06_strip_550_*`・`cb06_strip_553_*`・
  `cb06_strip_557_adopt_send_remove_candidate_materializes_
  teardown` を含む cb06 両 suite 全 Pass — 観測可能な send
  teardown（routing edge 込み）の物質化は維持。
- `mise run check/lint/test synvelis-project`・
  `check/lint/test synvelis-gateway`・`fmt-check`・
  `check-headless-deps`・`prek run --all-files` 全て exit 0。

## 関連リンク

- `synvelis#559`（本 Issue）/ `synvelis#557` / `synvelis#553` /
  `synvelis#550` / `synvelis#561` / `synvelis#283`
- `docs/impl/ir/issue-559-adopt-unobservable-removal.md`
- `docs/impl/exp/issue-559-adopt-unobservable-removal-exp.jsonl`
- `docs/impl/exp/issue-557-adopt-facts.md`
- `docs/impl/exp/issue-553-intent-facts.md`
- `evidence/sv559/`
- `POLICIES.md`
