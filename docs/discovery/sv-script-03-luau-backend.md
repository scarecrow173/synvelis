---
id: "DISC-034YAcow2bei2ILGhBkUlG"
type: "discovery"
status: "resolved"
title: "SV-SCRIPT-03 Luau backend slice — headless script backend 結合の探索"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/273"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/impl/ir/sv-script-03-workflow-plan.md"
    - "docs/impl/ir/sv-script-01-script-api-model.md"
    - "docs/impl/ir/sv-script-02-luau-action-script.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/sv-script-03-workflow-plan.md"
    - "evidence/sv273/sv-script-03-workflow-plan-ledger.md"
  refines: []
  refined-by:
    - "docs/specs/sv-script-03-luau-backend.md"
  derives-from: []
  derived-by:
    - "docs/specs/sv-script-03-luau-backend.md"
  verifies: []
  verified-by: []
  references:
    - "docs/automation/design-script.md"
  defers: []
  deferred-by: []
---

# SV-SCRIPT-03 Luau backend slice — headless script backend 結合の探索

## Summary

Issue #273 の第 2 slice として、headless pipeline に実 Luau
backend を結合する方法を探索した。結論: `synvelis-script` に
言語中立の backend port を置き、`mlua`（Luau feature）は新規
crate `synvelis-script-luau` のみに隔離する。CLI の
`.luau` → `ExecutablePlan` 結合は registry/surface 入力が
存在しないため、coordinator 承認の Option B 境界（typed
definition/import まで、plan compile は #262 へ defer）で
確定した。confidence: high（実装・実 VM test で検証済み）。

## Exploration Goal

第 1 slice（`sv-script-03-workflow-plan`）は typed spec →
import document → `WorkflowDefinition` の契約層のみで、
実 VM backend を持たなかった。本探索は「`.luau` source を
どの backend で評価し、どこまでをこの slice の責務とするか」
を確定することが目的。対象は headless 経路のみで、製品
runtime 統合（#265/#267）や CLI plan compile（#262/#274）は
範囲外。

## Key Issues and Assumptions

- `mlua` 依存の追加には coordinator 承認が必要 — port trait
  を `synvelis-script` に置き、mlua 依存は adapter crate のみ
  とする隔離方針で承認を得た（confirmed）。
- `.luau` → `ExecutablePlan` の CLI 結合には
  `HeadlessSessionHost` 経由の registry/surface 入力と
  canonical `ScriptSurfaceSpec` が必要だが、いずれも未存在
  （`host.rs`/`Booted` accessor は private、surface spec は
  #262/#274 領域）。→ Option B で defer（confirmed）。
- mlua `configure_luau` は `require`/`loadstring`/`print`/
  `debug` を無条件 install する — digest pin を迂回する
  filesystem code exec 経路になるため strip が必須
  （review で confirmed）。

## Alternatives and Comparison

| Option | Pros | Cons | Lean |
| --- | --- | --- | --- |
| A: `.luau` を `ExecutablePlan` まで CLI で結合 | 完結した体験 | registry/surface/`host.rs` accessor が全てリース外・未存在。捏造か大規模スコープ拡大が必要 | no |
| B: evaluate → typed spec/import document まで配線、plan compile は #262 へ defer | 既存契約層に正直に接続・fail-closed・リース遵守 | CLI から executable plan はまだ得られない | yes |
| C: backend port を作らず mlua を automation に直接注入 | 変更量が少ない | backend 中立性・test 差替え・将来 Rhai 追加時の再設計が必要 | no |

## Tentative Conclusions and Hypotheses

- confirmed: backend port（`ScriptBackend`/`ScriptSession`/
  `ScriptValue`/`ScriptArtifact`/`ExecutionBudget`/
  `ScriptBackendRegistry`）を `synvelis-script` に実装し、
  spec decode は `ScriptWorkflowSpec::from_script_value` に
  集約する。
- confirmed: `synvelis-script-luau` crate に mlua adapter を
  実装し、session 毎に独立 VM・digest 検証・budget・
  sandbox strip を行う。
- confirmed: `FileWorkflowResolver` は注入された
  `Arc<dyn ScriptBackend>` + `ScriptBuildContext` で `.luau`
  を評価し、file byte digest を importer 側で再検証する。
- confirmed: `.rhai` は backend 注入済みでも
  `UnsupportedSource`（fail-closed）を維持する。

## Open Questions

- RF-SCRIPT 正式 backend contract（ruviform#559）との整合:
  interim adapter の契約差分は #559 gate で再検証
  （`scarecrow173/ruviform#692` で製品 port 追跡）。
- `ExecutionBudget::default()` が無制限である点: consumer
  側 ceiling の要否は #262/#274 配線時に検討。

## Promotion Candidates

- [x] spec-doc needed: `docs/specs/sv-script-03-luau-backend.md`
- [ ] adr-doc needed: backend 境界は既存設計の延長であり
  新規 ADR は不要と判断

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/273>
- 前段 slice: `docs/impl/ir/sv-script-03-workflow-plan.md`、
  `docs/test-specs/sv-script-03-workflow-plan.md`、
  `evidence/sv273/sv-script-03-workflow-plan-ledger.md`
- 上流設計: `docs/automation/design-script.md`
- 関連 port: `docs/impl/ir/sv-script-01-script-api-model.md`、
  `docs/impl/ir/sv-script-02-luau-action-script.md`

## Evidence

- 2026-11-16: coordinator ask により mlua 依存承認
  （`synvelis-script-luau` のみ）と Option B 境界を取得。
- 2026-11-16: 実装完了 — port/backend/adapter/resolver/CLI
  の全経路が typed fail-closed で動作。29 件の実 VM test
  を含む全検証 green。
- 2026-11-16: review-protocol 2 reviewer 実施。P1×3
  （sandbox escape・lift DAG 爆発・SourceDrift panic）を
  含む指摘を修正。blocker 0 件。
