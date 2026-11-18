---
id: "PLAN-550StripRtOps05"
type: "plan"
status: "approved"
title: "Issue #550 plan — strip routing ops・entity removal slice"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/550"
    - "docs/specs/issue-550-strip-routing-ops.md"
    - "docs/designs/issue-550-strip-routing-ops.md"
  changes:
    added: []
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/variant/types.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/scope.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/adapter.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/strip.rs"
      - type: "file"
        path: "crates/synvelis-project/src/variant/engine.rs"
      - type: "file"
        path: "crates/synvelis-project/src/apply/entity.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/cb06_strip_fields.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare_engine.rs"
      - type: "file"
        path: "crates/synvelis-gateway/src/prepare.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/cb06_strip_gateway.rs"
      - type: "file"
        path: "crates/synvelis-gateway/tests/rig/mod.rs"
      - type: "file"
        path: "docs/impl/ir/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "docs/impl/exp/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "docs/test-specs/cb-06-track-bus-adapter.md"
      - type: "file"
        path: "evidence/sv283/acceptance-ledger.md"
      - type: "file"
        path: "evidence/sv283/verification.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/issue-550-strip-routing-ops.md"
    - "docs/designs/issue-550-strip-routing-ops.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/tasks/issue-550-strip-routing-ops.md"
    - "docs/test-specs/issue-550-strip-routing-ops.md"
---

# Issue #550 plan — strip routing ops・entity removal slice

## 分割

単一 slice。project（mask/scope/adapter/apply）→ gateway
（op 表/lowering/facts）→ test → docs/evidence の順で、
各段で compile + focused test を回す。

## file map

| 領域 | file | 内容 |
| --- | --- | --- |
| mask 型 | `variant/types.rs` | `StripField` 4 variant 追加 |
| 所有 oracle | `variant/scope.rs` | routing entity の scope_objects 導出 |
| 判定 | `variant/adapter.rs` | routing op 受理・confinement・removal check |
| capability | `variant/strip.rs` | caps per-field・boundary・edit_candidate・PreEditIds |
| adopt | `variant/engine.rs` | Removed write payload = 削除 entity serialization |
| 削除 | `apply/entity.rs` | removal_supported_kind/remove_entity arms |
| test | `tests/cb06_strip_fields.rs` | #550 positive/negative/adopt 節 |
| wire | `prepare_engine.rs` | send 4 op の descriptor |
| lowering | `prepare.rs` | lower_*・lower_for・op_facts |
| test | `tests/cb06_strip_gateway.rs`・`tests/rig/mod.rs` | send fixture・wire test・tagged() 全 kind |
| docs | cb-06 3 doc・sv283 ledger・sv550 evidence | 追補・新設 |

## リスクと rollback

- **adopt preflight 破壊**: `Removed` payload 変更は
  routing kind のみに影響し得るが、payload を読むのは
  preflight のみで `remove_entity` は無視 — 影響は
  routing entity の adopt のみで、他 kind は payload
  不読のまま。project test の adopt 節で実測。
- **mask 拡張の非互換**: `StripField` 追加は serde
  後方互換（旧 mask JSON は 5 field のまま有効）。
  既存 scope digest は mask 非依存の object 集合
  fingerprint なので旧 scope の digest 不変。
- **wire 表面の過剰約束**: connection/bus capability
  を登録しないことで `UnknownCapability` を維持 —
  deferred を偽装しない。

## verification matrix

| 層 | 手段 |
| --- | --- |
| project unit/integration | `mise run test synvelis-project`（cb06_strip_fields 24 件） |
| gateway integration | `mise run test synvelis-gateway`（cb06_strip_gateway 6 件） |
| lint/fmt | `mise run lint <pkg>`・`mise run fmt-check` |
| headless deps | `mise run check-headless-deps` |
| hooks | `mise exec -- prek run --all-files` |
