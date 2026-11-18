---
id: "TASK-559AdoptUnobservableRemoval05"
type: "task"
status: "done"
title: "Issue #559 tasks — adopt 観測不能 entity 削除 fail-closed slice"
created: "2026-11-19"
updated: "2026-11-19"
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
    - "docs/plans/issue-559-adopt-unobservable-removal.md"
  implemented-by:
    - "docs/impl/ir/issue-559-adopt-unobservable-removal.md"
  depends-on:
    - "docs/plans/issue-559-adopt-unobservable-removal.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-557-adopt-facts.md"
    - "docs/tasks/issue-553-intent-facts.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-559-adopt-unobservable-removal.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #559 tasks — adopt 観測不能 entity 削除 fail-closed slice

| # | タスク | 状態 | 証拠 |
| --- | --- | --- | --- |
| T1 | pre-553 相当 stored candidate payload の adopt が物質化することを headless で再現・記録 | done | `evidence/sv559/reproduction.md`・pre-fix test Pass（`cb_strip_559_repro_adopt_materializes_unobservable_removal` on base `848ebb73`） |
| T2 | doc chain 8 件 + `issue-559-adopt-unobservable-removal-exp.jsonl` + `evidence/sv559/` 作成 | done | 本 chain・`evidence/sv559/`・`docs/impl/{ir,exp}/issue-559-*` |
| T3 | production fail-closed: `entity.rs` predicate + `VariantError::UnverifiableRemoval` + `plan_adopt_writes` 検査 + gateway `variant_reason` arm | done | IR「Implementation > Changes」節・`git diff` |
| T4 | domain pin test — 観測不能 `Removed` write を含む candidate adopt が typed rejection + live 不変 | done | `cb06_strip_fields.rs`・`mise run test synvelis-project` Pass |
| T5 | gateway pin test — gain-target release cascade の stored payload adopt が `variant-unverifiable-removal` で `PreparationRejected` + live 不変 | done | `cb06_strip_gateway.rs`・`mise run test synvelis-gateway` Pass |
| T6 | 必須検証一式: `check/lint/test synvelis-project`・`check/lint/test synvelis-gateway`・`fmt-check`・`check-headless-deps`・`prek run --all-files` | done | `evidence/sv559/verification.md` — 全 lane exit 0 |
| T7 | `review-protocol` レビュー → blocker 0 → commit/push/PR | done | IR「Validation」節・PR #568 本文 |

## 備考

- T3 の対象 kind は `ModulationSource`/`ModulationAssignment`
  のみ — `RoutingConnection` は sv#550/#557 の受理済み
  SEC-14 境界（internal structure・owner 削除と不可分）
  として design に明記。
- T5 の scenario は wire 合法 op（`send.set-gain-target`
  解除 → `release_send_level_target` cascade）のみで
  観測不能 `ModulationAssignment` 削除入り candidate を
  生成する — pre-553 stored payload と同型の
  fail-closed 検証になる。
- discovery §4 で記録した edit/live-apply 経路の
  op_facts coverage gap（`SetSendGainTarget`/
  `RemoveTrack` cascade）は本 slice 対象外
  （`synvelis#561` 領域）— T5 はその gap が生成した
  candidate の adopt が正しく拒絶されることの pin。
