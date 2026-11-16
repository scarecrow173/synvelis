---
id: "TSPEC-034YxO2tq4RLbek1FBF5qf"
type: "test-spec"
status: "approved"
title: "AUTX-06 Mixed-Source Workflow Run Parity — YAML/Luau/Rhai → ExecutablePlan → 同一 WorkflowCoordinator FSM"
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
  implemented-by: []
  depends-on:
    - "docs/automation/design-workflow-run.md"
    - "docs/automation/design-script.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-06-workflow-coordinator.md"
    - "docs/test-specs/autx-06-live-adapters.md"
    - "docs/test-specs/sv-script-03-workflow-plan.md"
    - "docs/impl/ir/autx-06-mixed-source-run.md"
    - "docs/impl/exp/autx-06-mixed-source-run.md"
    - "evidence/sv265/autx-06-mixed-source-run-ledger.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/designs/execution-plan-design.md"
  verified-by: []
  references:
    - "crates/synvelis-automation/tests/mixed_source_run.rs"
    - "crates/synvelis-automation/tests/coordinator.rs"
    - "crates/synvelis-automation/tests/importer.rs"
    - "evidence/sv265/autx-06-mixed-source-run-ledger.md"
  defers: []
  deferred-by: []
---

# AUTX-06 Mixed-Source Workflow Run Parity — YAML/Luau/Rhai → ExecutablePlan → 同一 WorkflowCoordinator FSM

## Purpose

AUTX-06 tracker（synvelis#265）の mixed-source slice を headless
fixture で検証する試験仕様。同一生 semantic の workflow を YAML・
Luau・Rhai の 3 source から `ExecutablePlan` へ compile し、同一
`WorkflowCoordinator` + `MemoryRunJournal` + scripted
`StepLane`/`JobLane` で run/drive/cancel/reconcile を駆動したとき、
source 言語が run-level の観測可能な振る舞いを一切変えないことを
保証する。`autx-06-workflow-coordinator`（単一 source の FSM
fault matrix）と `sv-script-03-workflow-plan`（import 経路の
compile parity）の中間に位置し、ledger N-06/N-05 が残した
「YAML/Luau/Rhai 混在 Definition での実行」を解消する。
compile parity（同一 digest）の再証明は #264/#273 側の責務であり、
本 spec は digest を witness 記録に留め run-level parity を主張
する。durable journal・実 OS kill・GUI/MCP/CLI 実経路・実
provider 結合は `scarecrow173/synvelis#267`/`#262`/`#266` gate で
あり、本 spec の対象外として退役条件に含めない。

## Feature: Source 非依存の Workflow Run parity

### Rule: 3 source は同一 `WorkflowDefinition` に収束する

同一 workflow_id/version/inputs/steps/limits を持つ YAML literal・
Luau `ScriptWorkflowSpec`・Rhai `ScriptWorkflowSpec` は、builder →
import document → `WorkflowDefinition` の経路で semantic に同一の
definition を生成しなければならない。差異があれば parity の前提
自体が崩れるため fail とする。

#### Example: PAR-01 の定義一致

- Given: `test.basic` v1（query → invoke → emit-result）を YAML・
  Luau・Rhai の 3 source で compile する
- When: 各 source の `WorkflowDefinition` を比較する
- Then: 3 件全てが `PartialEq` で一致し、`plan.step_order` と
  `plan.definition_digest` も一致する

### Rule: journal trace は source 間で一致する

同一 `RunMeta`/run_id/inputs/lane script のもと、run を終端まで
駆動したとき、journal の `JournalKind` 列・step path・
`StepCheckpoint` verdict・run 状態遷移（`from_state`/`to_state`）・
終端 `RunState`・lane 呼出列（capability/args/target/
idempotency key）が 3 source 間で一致しなければならない。
`plan_revision`（plan digest）と entry `digest` は plan identity
の witness であり parity 判定からはマスクして比較する —
digest が分岐しても fail させず観測事実として記録する。

#### Example: PAR-01 の完走 trace 一致

- Given: 3 source それぞれの plan に対し同一 lane script
  （query 成功 → invoke 成功）で `start_run` → `drive` する
- When: `MemoryRunJournal` の全 entry・`WorkflowRun` projection・
  lane 呼出列を比較する
- Then: `RunState::Completed`、journal signature・run signature・
  call signature が 3 source で完全一致し、lane 呼出は各 2 回
  （query + invoke、emit は local）

### Rule: cancel は provider truth のまま記録される

async workflow を drive して `Waiting` に park したあと `cancel`
を呼んだとき、`CancelRequested` → provider verdict に応じた settle
（Confirmed/Pending/Settled の応答 class をそのまま journal に記録）
の trace が 3 source 間で一致し、cancelled を偽らない。

#### Example: CXL-P の cancel trace 一致

- Given: 3 source の async plan を drive し job `Running` で
  `Waiting` に park する。`CancelResponse::Pending` を script する
- When: `cancel` → job `Cancelled`/`Confirmed` を観測させて再度
  `cancel` する
- Then: `CancelIssued`（"cancel response: pending"）→ step
  `CancelRequested` → job `Cancelled` で step `Failed`・run
  `Failed`("cancelled") の journal trace が 3 source で一致し、
  `request_cancel` は pending job あたり 1 回だけ発行される

### Rule: Indeterminate は証拠なしに昇格しない

job owner が `Unknown` を返す観測を注入したとき、await step は
`Indeterminate` verdict で park し run は `Indeterminate` に留まる
（`result()` は `NotCompleted`）。その後 authoritative evidence
（job `Completed` + result payload）が届いた `reconcile` でのみ
完走へ回復し、その回復 trace が 3 source 間で一致する。

#### Example: IND-P の park→reconcile trace 一致

- Given: 3 source の async plan、`JobObservation::Unknown` を
  script する
- When: `drive` → `Indeterminate` を確認後、job `Completed` を
  set して `reconcile` する
- Then: `Indeterminate → Running`（"reconcile re-observation"）→
  await step `Completed` → run `Completed` の journal trace が
  3 source で一致し、lane 呼出は submit+status の 2 回のまま
  （await は jobs lane のみ再観測し再 submit しない）

### Rule: journal 再構成は source 非依存の同一 projection を返す

crash-restart を `MemoryRunJournal` の新規 instance への
header+entries 複写で模擬したとき、`replay_run`/`replay_entries`
が返す `WorkflowRun` projection は live session の projection と
一致し、かつ 3 source 間でも一致する。resume は journal 済み
idempotency key を再利用し、authoritative 側 dedupe を前提に
副作用を重複させない。

#### Example: RPL-P の replay/resume parity

- Given: 3 source の basic plan で 2 番目の lane 呼出に
  `GatewayError::Indeterminate` を注入し `Indeterminate` に park
  する
- When: journal を新規 `MemoryRunJournal` に複写し `replay_run`
  → 新 coordinator で `reconcile` → `Completed` まで駆動する
- Then: replay projection が live projection と一致、resume 後の
  journal trace・lane 呼出列（retry は同一 idempotency key）が
  3 source で一致する

### Rule: 異種 `workflow()` 呼出は同一 FSM で完走する

`kind: "workflow"` で異種 language の callee（Luau top → Rhai mid
→ YAML leaf）を inline した plan も、単一 source と同じ
`WorkflowCoordinator` FSM で完走し、callee の実 step は
namespace 付き id（`mid__leaf__tracks`）で journal・idempotency
key に記録される。language/version/digest pin の reject 側は
importer の責務であり本 rule の対象外。

#### Example: XSRC-01 の異種 chain 完走

- Given: `compile_script_workflow` + `MapWorkflowResolver` で
  Luau→Rhai→YAML の chain を 1 plan に inline する
- When: `start_run` → `drive` → `Completed` まで駆動する
- Then: `step_order` が `["mid__leaf__tracks", "done"]`、journal
  に `mid__leaf__tracks` の `StepBegan`/`StepSettled`（capability
  `synvelis.query.tracks@1.0`、key `run-xsrc:mid__leaf__tracks`）
  が記録され、run output `picked` が lane result と一致する

### Rule: journal/projection に source・VM 由来の状態を残さない

journal 全 entry・run header・`WorkflowRun` projection
（checkpoints/outputs）を serde JSON 化して再帰走査したとき、
key・値に language/backend/VM/source 系 token
（`language*`・`backend*`・`luau`・`rhai`・`yaml`・`script*`・
`vm`・`source`・`dialect`・`bytecode` 系を delimiter token /
substring で検査）が存在してはならない。「VM 内部状態の永続化
0 件」の headless 契約を fail-closed に検出する。将来 entry
schema に source field が混入した場合はこの走査が検出して失敗
する。

#### Example: SRC-0 の構造走査

- Given: 3 source の完走 journal と異種 chain の journal
- When: 全 entry/header/projection を JSON 化し key token と
  string value を再帰走査する
- Then: banned token の violation が 0 件

## Guarantees

- 3 source が同一 `WorkflowDefinition` に収束すること（semantic
  parity の前提条件として assert）。
- run/drive/cancel/reconcile/replay の journal trace・run
  projection・lane 呼出列が source 間で一致すること
  （`plan_revision`/entry `digest` を除く — digest は witness
  記録のみ）。
- cancel/indeterminate/restart-resume の honest 記録
  （cancelled 偽装なし・Unknown 昇格なし・idempotency key 安定）
  が source を問わず同一であること。
- 異種 `workflow()` chain が namespace 付き step で同一 FSM を
  完走すること。
- journal/projection が source/backend/VM の provenance を一切
  含まないこと（構造走査で fail-closed）。

## Non-goals

- compile parity（同一 semantic → 同一 plan digest）の再証明 —
  #264/#273 の責務。digest は witness 記録に留め、分岐しても
  fail させない。
- durable `RunJournal`（filesystem/DB）、実 OS process kill、
  GUI Window Close/MCP disconnect/CLI process cancel の実経路、
  実 provider/`synvelis-render` JobPort 結合、RT/Concurrency 実測
  — `scarecrow173/synvelis#267`/`#262`/`#266` gate。
- language/version/digest pin mismatch・import cycle・depth・
  path escape の reject 側 — `tests/importer.rs` 既存 case の責務。
- atomic macro・`if`/`for-each` の source parity — 本 slice は
  query/invoke/async-submit/await-job/emit/workflow-call の
  leaf 経路に限定する。
- `docs/**/README.md` index の同期 — Coordinator が統合時に再生成。

## Risk

- digest が source 間で分岐した場合、run-level parity は維持
  されるが plan identity の witness が弱まる。観測事実として
  ledger/PR に記録し、compile 側の調査は #264/#273 系 slice に
  委ねる。
- `compile_script_workflow` は `PlanOutput` のみ返すため、
  `RunContext.definition` は builder→import の同一 lowering を
  別途構成する。`verify_plan_identity` の
  `definition_digest` 一致検査が両者の同一性を実行時に保証する
  ため、乖離は `PlanMismatch` として顕在化する。
- scripted lane は deterministic であり、実 gateway の権限再検証
  ・epoch 失効・stale revision の経路は本 spec の対象外
  （`autx-06-live-adapters` / #267 系の責務）。
