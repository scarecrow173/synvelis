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
| T5 | gateway pin test — 観測不能 kind 削除入り stored payload の adopt が `unverifiable-removal` で `PreparationRejected` + live 不変 | done | `cb06_strip_gateway.rs`（domain 層 seed 方式 — sv#561 着地後の merge head で再実測）・`mise run test synvelis-gateway` Pass |
| T6 | 必須検証一式: `check/lint/test synvelis-project`・`check/lint/test synvelis-gateway`・`fmt-check`・`check-headless-deps`・`prek run --all-files` | done | `evidence/sv559/verification.md` — 全 lane exit 0 |
| T7 | `review-protocol` レビュー → blocker 0 → commit/push/PR | done | IR「Validation」節・PR #568 本文 |

## 備考

- T3 の対象 kind は `ModulationSource`/`ModulationAssignment`
  のみ — `RoutingConnection` は sv#550/#557 の受理済み
  SEC-14 境界（internal structure・owner 削除と不可分）
  として design に明記。
- T5 の payload は当初 wire 合法 op（`send.set-gain-target`
  解除 → `release_send_level_target` cascade）で生成する
  設計だったが、`synvelis#561`（`48858bb1`）着地により
  当該 wire 経路は `op_facts` unverifiable cascade として
  prepare で正直に拒否されるようになった。merge 後 head
  では domain 層 `fork_variant_set` + `edit_branch_op`
  （`RemoveSend` cascade — pre-553 stored payload と同型）
  で shared `BranchContentStore` に seed する方式に
  作り替え、durable payload への防御層としての pin の
  本来意図を維持した。
- discovery §4 で記録した edit/live-apply 経路の
  op_facts coverage gap（`SetSendGainTarget`/
  `RemoveTrack` cascade）は `synvelis#561` で解消済み
  — T5 はその拒否を迂回せず、domain 層で生成した
  durable payload の adopt が正しく拒絶されることの pin。
