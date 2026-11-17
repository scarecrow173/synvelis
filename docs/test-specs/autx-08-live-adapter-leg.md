---
id: "TSPEC-034Yqsf6S1U1JUkYbLDNHU"
type: "test-spec"
status: "approved"
title: "AUTX-08 Leg C live-adapter leg — 検証仕様"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/267"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/tests/autx08_e2e.rs"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/test-specs/autx-08-common-e2e.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-08-common-e2e.md"
    - "docs/specs/autx-08-live-adapter-leg.md"
    - "evidence/sv267/live-adapter-leg.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/autx-08-live-adapter-leg.md"
  derived-by: []
  verifies:
    - "docs/specs/autx-08-live-adapter-leg.md"
    - "docs/test-specs/autx-08-common-e2e.md"
  verified-by: []
  references:
    - "crates/synvelis-workspace/tests/autx08_e2e.rs"
    - "crates/synvelis-automation/src/live.rs"
    - "crates/synvelis-automation/src/coordinator.rs"
  defers: []
  deferred-by: []
---

# AUTX-08 Leg C live-adapter leg — 検証仕様

## Purpose

`crates/synvelis-workspace/tests/autx08_e2e.rs` の Leg C
section（`autx08_live_adapter_*` 4 test）が固定する契約を
記録する。production `GatewayStepLane`（`synvelis-automation`
live adapters）が、Leg B の GUI/CLI/MCP 3 surface と同一の
wire semantics — capability/schema/project/args/consistency/
target/idempotency・task/grant 伝播・fail-closed
authorization・typed outcome/error/`DuplicateRequest` — で
同一 `Gateway` pipeline を通ることを、公開 `StepLane`
surface（`invoke`/`operation_result`）上で証明する。

この test spec は「AI/live-adapter surface が独自の policy・
project・commit 正本や wire 次元の脱落を持ち込まない」と
いう不変条件を保護する。`step_wire` は adapter-internal
private fn であり wire byte は観測不能なため、等価の証明面は
公開 surface が産出する typed verdict に限定される — それが
検出すべき退行の境界である。headless slice が受入された
時点で本 spec は常設 regression 契約として残り、real-OS/
EXT-L3・live catalog・dispatch-hop の受入は #267 evidence
lane が別途管理する。

## Feature

AUTX-08 common E2E — AI/live-adapter surface same-wire
contract（production `GatewayStepLane` の `StepCall` →
`step_wire` → `Gateway::handle` 経路が、Leg B 3 surface と
同一の `GatewayOutcome`/`ErrorEnvelope`/`DuplicateRequest`
verdict を返すこと）

## Rules

- **Rule AUTX-E2E-SAME-01-AI** Leg B と同一の論理操作
  （`synvelis.operation.prepare@1.0` による
  `synvelis.midi.note.add` on `source10`、CAS
  `expected_revision:1`、delegated agent の `t1`/`g1`
  authority）を `StepLane::invoke(&StepCall)` で実行した
  とき、返る `GatewayOutcome::Completed{result}` payload が
  Leg B 全 surface の outcome と一致する。request_id 等の
  surface 由来メタは同一性の対象外（compose 対称
  convention）。
- **Rule AUTX-E2E-FAILCLOSED-AI** serve 不能な
  `expected_revision:99` は Leg B surface と同一の typed
  `ErrorEnvelope`（`GatewayError::SnapshotExpired`）を返す。
  `task`/`grant` を欠く delegated agent `StepCall` は
  authority なしで fail-closed し、CLI/MCP leg の denial
  と同一 envelope を返す — `attest` が mint するのは
  delegated agent caller のみであり、lane は wire が運ば
  ない authority を捏造しない。`Human` principal の
  `attest` fail-closed（`GatewayError::Authentication`）
  は adapter contract の別経路であり本 leg の対象外。
- **Rule AUTX-E2E-IDEM-AI** 同一 `idempotency_key` の
  `StepCall` 再 `invoke`（lane が mint する新 `wf-req-*`
  request_id — production retry と同じ形）は再実行されず
  `GatewayError::DuplicateRequest{original}` を返し、
  Leg B replay と同一 `original` operation を名指す。
  `StepLane::operation_result(&original)` は recorded
  outcome を `OperationObservation::Settled` として返し、
  ledger 記録 truth との整合はこの reconcile が担う
  （journal replay oracle は Leg B AUTX-E2E-ORACLE-01
  責務のまま）。
- **Rule AUTX-E2E-NOAUTORITY-AI** test は authority
  ロジックを複製しない。production `GatewayStepLane` と
  実 `Gateway`/`CommitGate`/journal/durable store のみを
  使い、`StepLane` 公開 surface の観測値のみを比較する。
  fake adapter・scripted gateway・fixture descriptor で
  契約を代替しない。

## Examples

- **Example AUTX-E2E-SAME-01-AI-01** `live_lane(true)`
  （`compose(None)` + `issue_agent_authority`）上で
  `live_step_call()` を `invoke` → `Completed{result}`。
  同じ `compose(None)` recipe の GUI/CLI/MCP leg が返す
  outcome/`structuredContent` と payload が一致する。
- **Example AUTX-E2E-FAILCLOSED-AI-01** `expected_revision:
  Some(99)` の `StepCall` → `Err(ErrorEnvelope{code:
  SnapshotExpired, ..})` で Leg B `cli_err`/`gui_err` と
  `code`/`category`/`retry`/`message` が一致。MCP 境界の
  `ExternalError` 投影とも一致。
- **Example AUTX-E2E-FAILCLOSED-AI-02** `task:None`/
  `grant:None` + agent principal の `StepCall`（authority
  未発行の `live_lane(false)`）→ Leg B CLI leg の
  task/grant なし denial と同一 typed envelope。
- **Example AUTX-E2E-IDEM-AI-01** 同一 `StepCall` を 2 回
  `invoke` → 2 回目は `DuplicateRequest{original}`。
  `operation_result(&original)` → `Settled(Completed)`
  で result payload が 1 回目の `Completed` と一致。
  独立 compose した CLI leg stack の同一 key replay が
  指名する `original` operation id とも一致。

## Guarantees

- 4 test が PASS し続ける限り、AI/live-adapter surface は
  Leg B 3 surface と同一 wire semantics を保持する。
  `step_wire` の lowering 詳細（request-id 形式・
  consistency 写像・idempotency 伝播）が変わっても、
  公開 `StepLane` surface が同一 verdict を返す限り
  契約は成立する。
- いずれかの surface が独自の project/policy/commit 正本や
  wire 次元の脱落を持ち込み `invoke`/`operation_result`
  の観測値が乖離した場合、本 spec の test は FAIL する
  — それが検出すべき退行である。
- headless 等価は real-OS での同値を保証しない。実
  window・実 credential store・実 transport・実 provider
  でのみ現れる差異は本 spec の対象外であり、#267 の
  evidence 分離を怠ると headless Pass が実機受入の
  代替として誤用される危険がある。

## Non-goals

- `step_wire` の wire byte 一致 — adapter-internal
  private fn であり公開化は adapter src 変更（本 slice
  lease 外）。等価は公開 `StepLane` surface の
  outcome/error/`DuplicateRequest`/`OperationObservation`
  parity で固定する。
- `GatewayJobLane` の job 観測 leg（job submit/await/
  cancel・`CancelRequested` 非昇格の honest-cancel
  semantics）— #265 coordinator 接続後の別検証。
- live-registry→`syn.autx.catalog` producer・
  `lower`→gateway dispatch-hop consumer・`synvelis-cli`
  `build_request`↔`lower`/`step_wire` cross-check・
  MCP `proposal.apply` idempotency — `docs/test-specs/
  autx-08-common-e2e.md` Non-goals と同一集合。
- real-OS GUI-L2/EXT-L3 受入・multi-process durable
  commit・実 provider/CLI binary — #114/#267 受入 lane。
- `Human` principal の `attest` 経路
  （`GatewayError::Authentication` 固定）— adapter
  contract の別検証対象であり本 leg は delegated agent
  のみを扱う。
- `AUTX-ACT/AUTH/MAC/WF/CLI/GUI-*`・`SCRIPT-*` 個別
  case・T-08-2 journal/policy oracle 新設。
- `*-msrv`/`*-full`/Feature Matrix/3OS 横断 — Daily
  Full CI 委譲。

## Risk

- 本保証が落ちると、AI surface が「同じ操作」を別の意味で
  実行し得る — workflow step が gateway wire semantics
  から乖離した場合、coordinator が記録する outcome と
  ledger truth の対応が壊れ、resume/reconcile の判定根拠が
  失われる。特に `DuplicateRequest` parity が落ちると
  idempotent retry が二重実行または誤った operation 指名に
  なる。
- 公開 `StepLane` surface parity は `step_wire` の内部
  lowering 詳細までは保証しない。wire field の微妙な差異
  （request-id 形式・consistency 写像の edge case）は
  `synvelis-automation`/`synvelis-gateway` 自身の adapter
  contract test 群が担保し、本 spec は surface 間 verdict
  parity のみを固定する。
- headless 等価は実 OS での同値を保証しない — 実
  window/credential/transport/provider でのみ現れる差異は
  #267 evidence lane の責務であり、headless Pass の実機
  昇格は禁止（POLICIES.md §2-3）。
