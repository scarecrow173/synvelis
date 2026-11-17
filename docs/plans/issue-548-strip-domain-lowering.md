---
id: "PLAN-548StripDomLowr05"
type: "plan"
status: "approved"
title: "Issue #548 plan — strip-domain lowering slice"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/548"
    - "docs/specs/issue-548-strip-domain-lowering.md"
    - "docs/designs/issue-548-strip-domain-lowering.md"
  changes:
    added: []
    modified:
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare_engine.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/cb06_strip_gateway.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/prepare.rs"
      - type: "file"
        path: "docs/test-specs/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "docs/impl/ir/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "docs/impl/exp/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "evidence/sv283/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv283/verification.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/designs/issue-548-strip-domain-lowering.md"
  implemented-by:
    - "docs/tasks/issue-548-strip-domain-lowering.md"
    - "docs/impl/ir/issue-548-strip-domain-lowering.md"
  depends-on:
    - "docs/designs/issue-548-strip-domain-lowering.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/issue-548-strip-domain-lowering.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/issue-548-strip-domain-lowering.md"
  derived-by:
    - "docs/tasks/issue-548-strip-domain-lowering.md"
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# Issue #548 plan — strip-domain lowering slice

## Scope

単一 slice。gateway op 表への strip-domain capability 登録、
lowering/facts 実装、CB-06 positive mask-gated edit 証拠、
ordinary prepare 試験、doc chain・evidence 更新、検証。

## File map

| ファイル | 変更 |
| --- | --- |
| `crates/synvelis-gateway/src/prepare_engine.rs` | `op_table` に 5 capability descriptor + schema 登録 |
| `crates/synvelis-gateway/src/prepare.rs` | import・target helper 3 件・lower fn 5 件・`lower_for`/`op_name`/`op_facts` arms |
| `crates/synvelis-gateway/tests/cb06_strip_gateway.rs` | module doc 更新・strip-domain op builder 追加・positive mask-gated edit 試験 2 件・mask/scope/非 strip 負側維持 |
| `crates/synvelis-gateway/tests/prepare.rs` | ordinary prepare 試験 3 件（lowering+effects・typed error） |
| `docs/{discovery,specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/issue-548-strip-domain-lowering.md` | doc chain（本 slice） |
| `docs/test-specs/cb-06-track-bus-adapter.md`・`docs/impl/ir/cb-06-track-bus-adapter.md`・`docs/impl/exp/cb-06-track-bus-adapter.md` | stale 記述を positive 経路着地へ追従 |
| `evidence/sv548/` | 本 slice の検証証拠（新規） |
| `evidence/sv283/acceptance-ledger.md`・`evidence/sv283/verification.md` | gateway positive 経路の追記 |

## Dependency graph

op_table 登録 → lower_for/op_facts → cb06 positive 試験 →
prepare 試験 → doc/evidence → 検証 → commit/PR。

## Risk register

| リスク | 対応 |
| --- | --- |
| `op_specs()` が lower 欠落で panic | `op_table` と `lower_for` を同一 commit で追加 |
| facts 未宣言で ordinary prepare が fail-closed | `op_facts` arms を必ず追加（設計判断1） |
| `ProcessorBinding` wire decode 失敗 | `arg_typed` の serde 形を cb06 fixture と同一 JSON で試験 |
| mask 負側証拠の喪失 | 狭 mask（`inserts` 未宣言）で insert/rebind を `variant-unsupported` に pin |
| docs index 未同期 | Worker は一切触れず PR 本文に明記（Coordinator が統合時に再生成） |

## Rollback

単一 feature branch の revert で完結。`synvelis-project`・
他 crate・共有 fixture への変更なし。
