---
id: "PLAN-sv511dE5fG7hI9jK1lM3nO5p"
type: "plan"
status: "approved"
title: "Issue #511 plan — CB-07 materialization 実装計画"
created: "2026-11-22"
updated: "2026-11-22"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/511"
    - "docs/specs/issue-511-cb07-materialization.md"
    - "docs/designs/issue-511-cb07-materialization.md"
    - "docs/adr/issue-511-cb07-materialization.md"
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
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-511-cb07-materialization.md"
    - "docs/adr/issue-511-cb07-materialization.md"
  derived-by:
    - "docs/tasks/issue-511-cb07-materialization.md"
  verifies: []
  verified-by:
    - "docs/test-specs/issue-511-cb07-materialization.md"
  references: []
  defers: []
  deferred-by: []
---

# Issue #511 plan — CB-07 materialization

## ファイルマップ

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-project/src/variant/engine.rs` | `plan_adopt_writes` の removal 判定を 3 分岐化（move-out `Unsupported` / 非対応 kind `Unsupported` / `Removed` write 計画） |
| `crates/synvelis-project/src/apply/entity.rs` | `removal_supported_kind` 追加、`write_entity` の `Removed` arm → `remove_entity`（Clip cascade + Note delete） |
| `crates/synvelis-project/src/ppqn.rs` | `reproject_ppqn` に `Captured` payload 存在時の typed 拒否を追加 |
| `crates/synvelis-project/tests/cb07_arrangement.rs` | cb_arr_11/cb_arr_08/cb_arr_14 の pin 書き換え + cb_arr_16 新規 |
| `docs/**` issue-511-cb07-materialization 10 ファイル | doc chain 新規 |
| `evidence/sv511/` | acceptance ledger 新規 |

無変更（設計上の確認済み）: `apply/plan.rs`（preflight は
`Removed` を自然スキップ — Null payload で安全）、
`render_input.rs`（同一 plan 再利用で自動追従）、
`variant/scope.rs`/`variant/arrangement.rs`（absolute anchor
fail-closed 維持）、`ops.rs`（`SetProjectPpqn` validate は
`reproject_ppqn` dry-run 経由で自動的に新契約を継承）。

## 手順

1. `entity.rs`: `removal_supported_kind` + `remove_entity` 追加、
   `write_entity` に `Removed` 分岐。
2. `engine.rs`: `plan_adopt_writes` の removal ループを
   entity-presence 判定 + kind ゲート + `Removed` write 計画へ。
3. `ppqn.rs`: `reproject_ppqn` 先頭に `Captured` チェック。
4. tests: cb_arr_11 を新契約へ書き換え、cb_arr_08 の notes-only
   adopt pin を新契約へ、cb_arr_14 を reject→discard→reproject
   の 2 相へ、cb_arr_16（sweep + clips-only cascade）新規。
5. `mise run check/lint/test synvelis-project`・`test-rt`・
   `fmt-check`・`prek`・`check-headless-deps` で検証。
6. `evidence/sv511` ledger + doc chain 完成 → review-protocol →
   commit/push/PR。

## リスクと rollback

- `Removed` write が preflight/introduced_digests で誤検知される
  リスク → `Null` payload + Added/kind 限定で構造的に安全。
  万が一の誤検知は typed error で fail-closed（silent 破壊なし）。
- Clip cascade が scope 外 note を消す → `ProjectOp::RemoveClip`
  と同一 semantics であり、candidate 側でも同じ cascade が
  適用済みのため live/candidate は一致する。
- rollback: 3 ファイルの変更は独立して revert 可能。test pin は
  新契約 assert のみで旧契約の証拠は git 履歴と sv284 ledger に
  残る。
