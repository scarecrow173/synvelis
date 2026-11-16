---
id: "IMPL-034YxOoo04evMym5AMz4aO"
type: "impl"
status: "in-progress"
title: "AUTX-06 Mixed-Source Workflow Run Parity — headless fixture suite"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/265"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-automation/tests/mixed_source_run.rs"
      - type: "file"
        path: "docs/test-specs/autx-06-mixed-source-run.md"
      - type: "file"
        path: "docs/impl/exp/autx-06-mixed-source-run.md"
      - type: "file"
        path: "docs/impl/exp/autx-06-mixed-source-run-exp.jsonl"
      - type: "file"
        path: "evidence/sv265/autx-06-mixed-source-run-ledger.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/automation/design-workflow-run.md"
    - "docs/automation/design-script.md"
    - "docs/impl/ir/autx-06-workflow-coordinator.md"
    - "docs/impl/ir/sv-script-03-workflow-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-06-mixed-source-run.md"
    - "docs/impl/exp/autx-06-mixed-source-run.md"
    - "evidence/sv265/autx-06-mixed-source-run-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/autx-06-mixed-source-run.md"
  references:
    - "docs/automation/design-workflow-run.md"
    - "crates/synvelis-automation/tests/coordinator.rs"
    - "crates/synvelis-automation/tests/importer.rs"
    - "crates/synvelis-automation/tests/common/mod.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/autx-06-mixed-source-run.md"
    rejected: []
---

# AUTX-06 Mixed-Source Workflow Run Parity — headless fixture suite

## Summary

同一生 semantic の workflow を YAML・Luau・Rhai の 3 source から
`ExecutablePlan` へ compile し、同一 `WorkflowCoordinator` +
`MemoryRunJournal` + scripted `StepLane`/`JobLane` で駆動して
run-level parity を検証する headless 統合テスト
`crates/synvelis-automation/tests/mixed_source_run.rs` を追加する。
src 変更ゼロ — 必要 API（`compile_workflow_yaml` /
`compile_script_workflow` / `import_workflow_document` /
`WorkflowScriptBuilder` / coordinator・run 公開面）は全て export
済みであることを確認済み。

## Context

Issue `scarecrow173/synvelis#265`（AUTX-06 tracker）の slice。
`autx-06-workflow-coordinator` ledger N-06 と
`autx-06-live-adapters` ledger N-05 が残した「YAML/Luau/Rhai 混在
Definition での実行」を、SV-SCRIPT-03（synvelis#273, PR #441）で
揃った importer 経路上で headless に証明する。試験仕様は
`docs/test-specs/autx-06-mixed-source-run.md`。

## Implementation

### Changes

- `crates/synvelis-automation/tests/mixed_source_run.rs`（新規）:
  PAR-01（3-source 完走 trace parity）・CXL-P（cancel honesty
  parity）・IND-P（Indeterminate→reconcile parity）・RPL-P
  （journal 再構成 replay/resume parity）・XSRC-01（Luau→Rhai→
  YAML 異種 `workflow()` chain 完走）・SRC-0（journal/projection
  の source・VM 非含有 構造走査）の 6 case。
- `docs/test-specs/autx-06-mixed-source-run.md`（新規）: Rule /
  Example 形式の試験仕様。
- `docs/impl/exp/autx-06-mixed-source-run.md` + `-exp.jsonl`
  （新規）: 実装試行の記録。
- `evidence/sv265/autx-06-mixed-source-run-ledger.md`（新規）:
  検証証拠 ledger。

### Decisions During Implementation

- `compile_script_workflow` は `PlanOutput` のみを返し
  `WorkflowDefinition` を返さないため、script source の
  `RunContext.definition` は `WorkflowScriptBuilder::build` →
  `import_workflow_document` の同一 lowering を別途構成する。
  `verify_plan_identity` の `definition_digest` 一致が両者の
  同一性を実行時に保証し、乖離は `PlanMismatch` で顕在化する。
- parity 判定は journal signature（`plan_revision`/entry
  `digest` をマスクした全 field）・`WorkflowRun` projection
  signature・lane 呼出 signature の一致で行い、plan digest は
  witness として `eprintln!` で記録するのみで fail 条件にしない
  （test-spec Non-goals どおり compile parity は #264/#273 の責務）。
- async 系 case（CXL-P/IND-P）は `ASYNC_WORKFLOW` 相当の
  `ScriptWorkflowSpec`（async-submit → query → await-job →
  emit）を script 側 fixture に追加し、`synvelis.job.render`
  （AsyncOperation/AsyncAccept）・`synvelis.job.status`
  （Query/Sync）を script API surface に登録する。
- XSRC-01 は `tests/importer.rs` の Luau→Rhai→YAML chain 構成を
  写し、`compile_script_workflow` + `MapWorkflowResolver` で
  inline した plan を同一 coordinator で完走させる。

## Related Experiments

### Adopted

- `docs/impl/exp/autx-06-mixed-source-run.md` — 本 slice の試行記録。

### Rejected

- なし。

## Validation

`docs/test-specs/autx-06-mixed-source-run.md` と
`evidence/sv265/autx-06-mixed-source-run-ledger.md` に
記録。`mise run test synvelis-automation` で
`mixed_source_run` 6 test を含む全 test が Pass
（fmt-check / check / lint も exit 0。詳細は ledger）。

## Risks

- plan digest が source 間で分岐した場合は witness 記録に留め、
  parity 判定は semantic 比較で行う（test-spec 記載どおり）。
- durable journal・実 process kill・実 provider 経路は本 slice
  対象外（`scarecrow173/synvelis#267`/`#262`/`#266` gate）。

## Follow-ups

- docs index（`docs/**/README.md`）の再生成は Coordinator が統合時に
  `doc-maintenance` で実施する。
