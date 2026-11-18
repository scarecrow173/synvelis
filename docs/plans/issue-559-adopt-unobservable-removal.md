---
id: "PLAN-559AdoptUnobservableRemoval04"
type: "plan"
status: "approved"
title: "Issue #559 plan — adopt 観測不能 entity 削除 fail-closed slice"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/559"
    - "docs/specs/issue-559-adopt-unobservable-removal.md"
    - "docs/designs/issue-559-adopt-unobservable-removal.md"
  changes:
    added:
      - type: "file"
        path: "docs/discovery/issue-559-adopt-unobservable-removal.md"
      - type: "file"
        path: "docs/specs/issue-559-adopt-unobservable-removal.md"
      - type: "file"
        path: "docs/designs/issue-559-adopt-unobservable-removal.md"
      - type: "file"
        path: "docs/plans/issue-559-adopt-unobservable-removal.md"
      - type: "file"
        path: "docs/tasks/issue-559-adopt-unobservable-removal.md"
      - type: "file"
        path: "docs/test-specs/issue-559-adopt-unobservable-removal.md"
      - type: "file"
        path: "docs/impl/ir/issue-559-adopt-unobservable-removal.md"
      - type: "file"
        path: "docs/impl/exp/issue-559-adopt-unobservable-removal.md"
      - type: "file"
        path: "docs/impl/exp/issue-559-adopt-unobservable-removal-exp.jsonl"
      - type: "dir"
        path: "evidence/sv559/"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/apply/entity.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/error.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/engine.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/variant.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/cb06_strip_fields.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/cb06_strip_gateway.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-559-adopt-unobservable-removal.md"
    - "docs/designs/issue-559-adopt-unobservable-removal.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-559-adopt-unobservable-removal.md"
---

# Issue #559 plan — adopt 観測不能 entity 削除 fail-closed slice

## スコープ分割

単一 slice — production 変更は 4 ファイル
（predicate・error variant・planner 検査・reason mapping）
に収まり、分割の必要なし。

## ファイルマップ

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-project/src/apply/entity.rs` | `unverifiable_removal_kind` predicate 追加 |
| `crates/synvelis-project/src/variant/error.rs` | `VariantError::UnverifiableRemoval` 追加 + `Display` |
| `crates/synvelis-project/src/variant/engine.rs` | `plan_adopt_writes` の Removed loop に検査追加 + doc 追記 |
| `crates/synvelis-gateway/src/variant.rs` | `variant_reason` に `unverifiable-removal` arm 追加 |
| `crates/synvelis-project/tests/cb06_strip_fields.rs` | 再現 test → deny pin に置き換え |
| `crates/synvelis-gateway/tests/cb06_strip_gateway.rs` | 観測不能 kind 削除入り stored payload の adopt 拒絶 pin（domain 層 seed 方式）+ fixture 追加 |

## 依存順序

1. doc chain（本 plan まで）
2. production 変更（entity → error → engine → gateway reason）
3. domain pin test 更新 → focused test で red→green 確認
4. gateway pin test 追加 → focused test
5. 全検証 lane
6. review-protocol → commit/push/PR

## リスクと rollback

- リスク: `ModulationSource`/`ModulationAssignment` の
  `Removed` write を含む durable candidate が adopt
  不能になる。当初は `SetSendGainTarget` release・
  `RemoveTrack` strip closure 経由で wire 合法に生成
  可能と評価したが、`synvelis#561`（`48858bb1`）着地
  により当該 wire 経路は `op_facts` unverifiable
  cascade として prepare で正直に拒否される —
  拒絶対象は pre-#561 時代の stored payload に限ら
  れ、これは SEC-14 的に正しい fail-closed。
- リスク: `RoutingConnection` を reject 集合から外す
  判断が過剰受理と見なされる可能性 — sv#550/#557 の
  受理済み境界（internal structure・owner 削除と不可分）
  を design に明記して防御。
- rollback: 4 ファイルの production diff を revert すれば
  pre-fix 挙動に戻る（test/docs は残るが fail する —
  revert は chain 全体で行う）。

## 検証マトリクス

| 層 | 検証 |
| --- | --- |
| domain | `mise run check/lint/test synvelis-project`（cb06_strip_fields pin 含む） |
| gateway | `mise run check/lint/test synvelis-gateway`（cb06_strip_gateway pin 含む） |
| 横断 | `mise run fmt-check`・`mise run check-headless-deps`・`prek run --all-files` |

NotRun: GUI-L2・EXT-L3・実 PCM/plugin/RT・MSRV・
3-OS・full CI（POLICIES.md §5 — headless のみ）。

## 関連リンク

- `synvelis#559`（本 Issue）
- `docs/specs/issue-559-adopt-unobservable-removal.md`
- `docs/designs/issue-559-adopt-unobservable-removal.md`
- `docs/tasks/issue-559-adopt-unobservable-removal.md`
- `POLICIES.md`
- `AGENTS.md`
