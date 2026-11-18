---
id: "TASK-557AdoptFacts05"
type: "task"
status: "done"
title: "Issue #557 tasks — adopt declared_facts 監査 slice"
created: "2026-11-18"
updated: "2026-11-18"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/557"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/issue-557-adopt-facts.md"
  implemented-by:
    - "docs/impl/ir/issue-557-adopt-facts.md"
  depends-on:
    - "docs/plans/issue-557-adopt-facts.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-553-intent-facts.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/issue-557-adopt-facts.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #557 tasks — adopt declared_facts 監査 slice

| # | タスク | 状態 | 証拠 |
| --- | --- | --- | --- |
| T1 | `declared_facts` Adopt arm・`AdoptIntent`・`lower_adopt`・`plan_adopt_writes`・`removal_supported_kind`・wire capability（`lower_for`/`op_table`）・`ResourceKind`/`ENUMERATED_KINDS` の監査 — sv#553 同型 gap の有無を構造的に判定 | done | `docs/discovery/issue-557-adopt-facts.md` §1–4・`docs/impl/ir/issue-557-adopt-facts.md`・`evidence/sv557/` |
| T2 | adopt 受理 pin test `cb06_strip_557_adopt_send_remove_candidate_materializes_teardown` を `cb06_strip_gateway.rs` に追加し対象 test を実行 | done | 同ファイル末尾・`mise run test synvelis-gateway` 結果は `evidence/sv557/verification.md` |
| T3 | 残存経路（pre-553 stored candidate payload）の評価と hardening Issue 起票 | done | `synvelis#559`（GitHub Issue — 着手条件なし・方針候補を本文に記録） |
| T4 | doc chain 8 件 + `issue-557-adopt-facts-exp.jsonl` + cb-06 doc chain 3 ファイル追補 + `evidence/sv557/` 作成 | done | 本 chain・`evidence/sv557/` |
| T5 | 必須検証一式: `mise run check/lint/test synvelis-gateway`・`check/lint/test synvelis-project`・`fmt-check`・`check-headless-deps`・`prek run --all-files` | done | `evidence/sv557/verification.md`（全 exit 0） |
| T6 | `review-protocol` レビュー → blocker 0 → commit/push/PR | done | IR「review 対応」節・PR 本文 |

## 備考

- T1 の結論: sv#553 同型 gap は構造上存在しない
  （`AdoptIntent` に inner op が無く、wire `lower_adopt`
  も CAS のみ受理）。adopt の write 認可は post-
  simulation `derive_effects` + `authorize_effects` が
  実 diff で行う設計。
- T2 は AC2 の条件付き要件に対し、gap 無しの結論のもと
  「受理側契約の pin」として実施（deny pin は条件不成立
  のため不実施 — 残存経路は `synvelis#559`）。
- T3 の判断根拠: `DirectoryBranchContentStore` が durable
  で pre-553 payload が残り得ること、`plan_adopt_writes`
  が観測不能 kind を `Removed` write 化し得ること、
  現行 wire では当該 payload を新規生成不能であること
  の 3 点から、hardening は別 Issue 化が適切と判断。
- T5 の project crate 検証は非接触の下流整合として
  Issue 指定どおり実施。
- T6 のレビュー結果は IR「review 対応」節に記録。

## 関連リンク

- `synvelis#557`（本 Issue）/ `synvelis#553` /
  `synvelis#550` / `synvelis#559` / `synvelis#283`
- `docs/plans/issue-557-adopt-facts.md`
- `docs/impl/ir/issue-557-adopt-facts.md`
- `docs/test-specs/issue-557-adopt-facts.md`
- `docs/tasks/issue-553-intent-facts.md`（先行 slice）
- `POLICIES.md`
