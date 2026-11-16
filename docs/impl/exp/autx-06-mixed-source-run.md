---
id: "EXP-034YxP4u8sW2mK6nQ9vX3b"
type: "experiment"
status: "in-progress"
title: "AUTX-06 Mixed-Source Run Parity — 3-source compile→coordinator fixture の試行"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/265"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/impl/ir/autx-06-mixed-source-run.md"
  depends-on:
    - "docs/automation/design-workflow-run.md"
    - "docs/impl/ir/autx-06-workflow-coordinator.md"
    - "docs/impl/ir/sv-script-03-workflow-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-06-mixed-source-run.md"
    - "evidence/sv265/autx-06-mixed-source-run-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-automation/tests/mixed_source_run.rs"
    - "crates/synvelis-automation/tests/coordinator.rs"
    - "crates/synvelis-automation/tests/importer.rs"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# AUTX-06 Mixed-Source Run Parity — 3-source compile→coordinator fixture の試行

## 問題設定

Issue `scarecrow173/synvelis#265` の mixed-source slice。同一生
semantic の workflow を YAML・Luau・Rhai の 3 source から
`ExecutablePlan` へ compile し、同一 `WorkflowCoordinator` で
駆動したとき run-level の観測可能な振る舞い（journal trace・
run projection・lane 呼出・cancel/indeterminate/restart の
honest 記録）が source 非依存であることを headless に示す。
設計上の不確実点は (a) `compile_script_workflow` が
`PlanOutput` のみを返し `WorkflowDefinition` を返さないため
`RunContext.definition` をどう供給するか、(b) parity 比較の
粒度（digest witness と semantic 比較の分離）、(c) async 系
case に必要な script 側 capability surface の最小構成、
(d) journal/projection の source・VM 非含有をどう fail-closed
に走査するか、の 4 点。

## 試行と結果

### E1: script 側 `WorkflowDefinition` の供給経路

`compile_script_workflow` は `PlanOutput` のみを返すため、
`RunContext.definition` は別途
`WorkflowScriptBuilder::build` → `import_workflow_document`
で再構成した。`verify_plan_identity` が plan の
`definition_digest` と definition の digest を照合するため、
2 経路の不一致は compile 時点で検出される。3 source の
definition `PartialEq` と `plan.step_order` /
`plan.definition_digest` の一致を `assert_source_convergence`
で確認 — 全 case で成立した。

### E2: parity 比較の粒度

journal entry は `plan_revision`・`digest` を mask した
signature で比較（plan identity は witness であり gate では
ない — compile parity は `scarecrow173/synvelis#264` /
`#273` の責務）。`WorkflowRun` projection は `plan_digest`
以外の全 semantic field、lane 呼出は capability/args/
idempotency_key/principal 等を比較。PAR-01/CXL-P/IND-P/RPL-P
の 4 case で 3 source 間の trace 完全一致を確認した。

### E3: async case の script capability surface

`ASYNC_WORKFLOW` 相当の semantic（async-submit → query →
await-job → emit）には `synvelis.job.render`
（AsyncOperation/AsyncAccept）と `synvelis.job.status`
（Query/Sync）の script surface 登録が必要だった。
`tests/importer.rs` の 2-cap 構成を 4-cap に拡張し、
`ScriptStepCall::AwaitJob`/`Emit`/`Action` で
`async_spec()` を構成。`$steps.render` の job 参照は
importer の `$`-rewrite が tagged expr へ落とす。

### E4: XSRC-01 異種 chain

`tests/importer.rs` の Luau→Rhai→YAML 構成を写し、
`MapWorkflowResolver` に YAML leaf（`ImportedWorkflowPayload::
Definition`）と Rhai mid（`Document`）を pin 登録。
`expand_workflow_call` で inline された
`mid__leaf__tracks` step が namespace 付きで journal に
記録され、idempotency key は `run-xsrc:mid__leaf__tracks`
で導出されることを確認した。

### E5: SRC-0 fail-closed 走査

journal header・全 entry・`WorkflowRun` projection
（steps/outputs/step_outputs/pending_jobs/cancel 系/cursor）
を serde JSON 化し、key・string value を再帰走査。
`luau`/`rhai`/`yaml`/`language*`/`backend*`/`script*`/
`vm`/`source`/`dialect`/`bytecode`/`ast`/`eval` を
substring＋delimited-token の二段で検査。初回実行で
XSRC leg の lane 戻り値が `picked` schema（track id 形式）
に違反し run が `Failed` になったのは fixture 側の問題で、
走査自体の検出漏れではなかった。修正後 4 artifact
（yaml/luau/rhai/xsrc）で違反 0 件を確認した。

### E6: 検証

- `cargo test -p synvelis-automation --test mixed_source_run`:
  6/6 pass（補助証拠）
- `mise run fmt-check` / `mise run check synvelis-automation` /
  `mise run lint synvelis-automation` /
  `mise run test synvelis-automation`: 全て exit 0（canonical）

## 採用した判断

- **definition の二重構成を許容**: script source は
  builder/import と compile の 2 経路で definition を得るが、
  `verify_plan_identity` の digest 照合が正直性を担保する。
  production export 追加は不要と判断（lease 外変更ゼロを維持）。
- **digest は witness のみ**: `plan.digest()` を eprintln で
  記録するが assert しない。意図しない source 別分岐は
  観測事実として ledger/PR に報告する運用（test-spec の
  Non-goal どおり）。
- **SRC-0 は二段検査**: 一意な provenance 語（`luau` 等）は
  substring、曖昧語（`source`/`vm`/`script` 等）は
  delimiter 分割後の token 一致で、`resource`/`description`
  のような正当語の誤検出を避けつつ fail-closed にした。

## 結論

3 source parity・cancel honesty・indeterminate reconcile・
replay-resume・異種 chain・source/VM 非含有の 6 case が
headless fixture で全て成立した。`src/` 変更ゼロ、
`tests/` への additive 変更のみで達成。durable journal・
実 OS kill・GUI/MCP/CLI 実経路・実 provider 結合・RT 実測は
`scarecrow173/synvelis#267`/`#262`/`#266` gate として
Deferred のまま。
