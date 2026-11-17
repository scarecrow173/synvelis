---
id: "DISC-034Yqs367zaVOMTCwZAbtn"
type: "discovery"
status: "resolved"
title: "AUTX-08 Leg C live-adapter leg — AI surface same-wire parity investigation (synvelis#267)"
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
    modified:
      - type: "file"
        path: "crates/synvelis-workspace/Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/specs/autx-08-live-adapter-leg.md"
  depends-on:
    - "docs/impl/ir/autx-08-headless-common-e2e-slice-same-wire-3-surface-contract-test-synvelis-267.md"
    - "docs/impl/ir/autx-06-live-adapters.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/autx-08-common-e2e.md"
    - "evidence/sv267/live-adapter-leg.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/autx-08-live-adapter-leg.md"
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-automation/src/live.rs"
    - "crates/synvelis-automation/src/coordinator.rs"
    - "crates/synvelis-workspace/tests/autx08_e2e.rs"
    - "crates/synvelis-workspace/tests/rig/mod.rs"
    - "crates/synvelis-gateway/tests/autx06_live_adapters.rs"
    - "crates/synvelis-automation/tests/live_adapters.rs"
  defers: []
  deferred-by: []
---

# AUTX-08 Leg C live-adapter leg — AI surface same-wire parity investigation (synvelis#267)

## Summary

Issue #267 AUTX-08 の T-08-1 は Leg B（GUI/CLI/MCP 3-surface
same-wire contract test、`crates/synvelis-workspace/tests/
autx08_e2e.rs`）merge 済みだが、AI lane（production
`GatewayStepLane`/`GatewayJobLane` の live adapter surface）は
`evidence/sv267/acceptance-ledger.md` で Deferred のまま残って
いた。本 slice はその headless 部分を閉じる: production
`GatewayStepLane` を `rig::compose()` の fresh `GatewayStack`
上に組み立て、Leg B と同一論理操作（`synvelis.operation.
prepare@1.0` + `midi.note.add` on `source10`）を `StepLane::
invoke` で投入し、outcome/error/`DuplicateRequest`/
`operation_result` reconcile の 4 assertion で parity を固定する
方針で spec へ昇格した（confirmed — 実装・検証済み）。

## Exploration Goal

Leg C の受入条件は「production `GatewayStepLane` が Leg B surface
と同一の typed `GatewayOutcome`/`ErrorEnvelope`/`DuplicateRequest`
を返す」こと。しかし `step_wire`（`StepCall` → wire JSON の
producer）は adapter-internal private fn であり、公開 surface
から wire byte を直接観測できない。どの観測面で等価を証明するか、
どの composition で lane を組み立てるか、test を既存
`autx08_e2e.rs` に足すか sibling file にするかを決める必要が
あった。

## Key Issues and Assumptions

- `GatewayStepLane` は merge 済み（AUTX-06 live adapters、
  `crates/synvelis-automation/src/live.rs` L294 `new`・L330
  `invoke`・L338 `operation_result`）。`StepLane` trait は
  `synvelis_automation::coordinator` で public、
  `GatewayStepLane`/`VersionedRef` は crate root で `pub use`
  済み（`lib.rs` L64/L71）— test から production adapter を
  そのまま使える（confirmed）。
- `step_wire` は private（live.rs L227）。wire byte 等価は
  観測不能なため、公開 `StepLane` surface が産出する
  `GatewayOutcome`/`ErrorEnvelope`/`DuplicateRequest.original`/
  `OperationObservation` の parity で証明する — これは
  `docs/test-specs/autx-08-common-e2e.md` Rule
  AUTX-E2E-SAME-01 の「adapter 境界の正当な差異」convention
  （MCP leg が `mcp_tools::wire_for` 内部 wire を outcome
  parity で証明した先例）と同型（confirmed）。
- composition 先例は 2 系統: `crates/synvelis-gateway/tests/
  autx06_live_adapters.rs`（独自 rig で `LocalCliAdapter::new`
  + `GatewayStepLane::new`）と `crates/synvelis-automation/
  tests/live_adapters.rs`（`step_call()`/`step_lane()` helper）。
  本 slice は workspace crate の test であり `rig::compose()`
  の `GatewayStack`（`gw`/`ports: GatePorts`/`jobs`/`clock`/
  `store`/`source`/`gate`）を使うのが Leg B と同一 fixture
  意味論を保つ最短経路（confirmed）。
- `GatewayStack.gw` は値所有の `Gateway` であり、lane は
  `Arc<Mutex<Gateway>>` を要求する — test 内で
  `Arc::new(Mutex::new(stack.gw))` に包む。`ports` は
  `Arc<GatePorts>` で `CommitPort` impl 済み（rig/mod.rs L234）
  のため `Arc<dyn CommitPort>` へ unsize coerce できる
  （confirmed — 実装で実測）。
- `compose(None)` は `jobs: None` を返す（job lane なし）。
  本 slice の 4 assertion は全て step lane（prepare 系）の
  parity であり `GatewayJobLane` の job 観測は対象外 —
  `jobs: Option<Arc<JobEngine>>` に `None` を渡しても
  `operation_result` の stage-1（gateway ledger）で prepare
  outcome が settle するため影響しない（confirmed）。
- `synvelis-workspace` → `synvelis-automation` の dev-dep は
  acyclic（automation は workspace に依存しない）・
  headless-safe（`tools/check-headless-deps.mjs` の
  HEADLESS_CRATES 両者登録済み）・外部依存は serde/serde_json/
  sha2 のみ。`synvelis-gateway`/`synvelis-cli` が同一 dev-dep
  を持つ先例あり。Coordinator 承認済み（confirmed）。

## Alternatives and Comparison

| Option | Pros | Cons | Lean |
| --- | --- | --- | --- |
| `autx08_e2e.rs` に Leg C section を追加 | `note_add_args`/`issue_agent_authority`/`gui_leg`/`cli_leg`/`mcp_prepare_call`/`wire`/`expected` 等の helper をそのまま再利用でき parity 比較が同一 file 内で完結。fixture 意味論の drift が起きない | 1 file が長くなる（~730 行） | yes |
| `autx08_live_e2e.rs` sibling 新設 | file 分離で leg ごとの読みやすさ | helper の複製または `pub(crate)` 化が必要で、Leg B helper を変えると lease 外変更リスク。parity の「同一論理操作」保証が 2 file に分散 | no |
| `crates/synvelis-automation/tests/` 側に test 追加 | adapter 近接 | workspace rig（`compose`/`GatePorts`/fixture descriptor）を使えず、Leg B surface との直接比較が不可能。lease 外でもある | no |
| wire byte 一致を assert | より強い等価 | `step_wire` は private で観測不能。公開化は adapter src 変更で lease 外・READ-ONLY | no |

## Tentative Conclusions and Hypotheses

- confirmed: `rig::compose()` stack 上に `GatewayStepLane::new
  (Arc::new(Mutex::new(stack.gw)), LocalCliAdapter, ports as
  CommitPort, stack.jobs)` で production lane を組み立て、
  `StepCall`（`synvelis.operation.prepare@1.0`・`note_add_args()`
  同一 args・`expected_revision: Some(1)`・`task:"t1"`/`grant:
  "g1"`・agent principal）を `invoke` すると Leg B surface と
  同一 `GatewayOutcome::Completed` payload が返る（実測 pass）。
- confirmed: `expected_revision: Some(99)` は Leg B と同一の
  `ErrorEnvelope`（`SnapshotExpired`）を返す — consistency は
  `step_wire` が wire の `expected-revision` へ写すため
  adapter policy ではなく snapshot-seam semantics。
- confirmed: `task`/`grant` を `None` にした delegated agent
  `StepCall` は `attest` が `caller_delegated` のみ mint する
  ため authority なしで fail-closed — CLI leg の unattended
  claim denial と同一 typed envelope（Human principal は
  `GatewayError::Authentication` 固定で別経路 — AI leg は
  delegated agent のみを検証対象とする）。
- confirmed: 同一 `idempotency_key` の `StepCall` 再 `invoke`
  は lane が mint する新 `wf-req-*` request_id 経由で
  `GatewayError::DuplicateRequest{original}` を返し、
  `operation_result(&original)` が `OperationObservation::
  Settled(GatewayOutcome::Completed)` を返す。独立 compose
  した CLI leg stack の replay が指名する `original` と
  operation id が一致（per-instance counter が compose recipe
  同一なら揃う — Leg B と同じ根拠）。

## Open Questions

- `GatewayJobLane`（job 観測 leg）の same-wire parity は本
  slice 対象外 — job submit/await/cancel の live workflow
  経路は #265 coordinator 接続後の別検証。
- live-registry → `syn.autx.catalog` producer・`lower` →
  gateway dispatch-hop consumer・`synvelis-cli`
  `build_request`↔`lower`/`step_wire` cross-check は #264
  mount wiring 後の deferred cross-check（Issue #267・
  test-spec Non-goals どおり）。

## Promotion Candidates

- [x] spec-doc needed: `docs/specs/autx-08-live-adapter-leg.md`
- [ ] adr-doc needed: なし（既存 adapter 公開 surface の
  contract test 追加でありアーキテクチャ判断を伴わない）

## Sources and Relations

- https://github.com/scarecrow173/synvelis/issues/267
- `docs/test-specs/autx-08-common-e2e.md`（Rule
  AUTX-E2E-SAME-01/FAILCLOSED/IDEM/NOAUTHORITY、Non-goals）
- `docs/impl/ir/autx-08-headless-common-e2e-slice-same-wire-3-
  surface-contract-test-synvelis-267.md`（Leg B IR）
- `docs/impl/ir/autx-06-live-adapters.md`（live adapter 本体）
- `crates/synvelis-automation/src/live.rs`・
  `crates/synvelis-automation/src/coordinator.rs`・
  `crates/synvelis-automation/tests/live_adapters.rs`・
  `crates/synvelis-gateway/tests/autx06_live_adapters.rs`・
  `crates/synvelis-workspace/tests/{autx08_e2e.rs,rig/mod.rs}`

## Evidence

- 2026-11-16: `crates/synvelis-workspace/Cargo.toml`
  `[dev-dependencies]` に `synvelis-automation = { path =
  "../synvelis-automation" }` を追加（Coordinator 承認済み）。
  `Cargo.lock` 差分は `synvelis-workspace` の deps 欄への
  `synvelis-automation` 1 行追加のみ。
- 2026-11-16: `autx08_e2e.rs` に Leg C section を追加し
  `cargo test -p synvelis-workspace --locked --test
  autx08_e2e` で 8/8 pass（新規 4: `autx08_live_adapter_
  lands_same_outcome`・`autx08_live_adapter_unavailable_
  revision_fails_identically`・`autx08_live_adapter_
  without_authority_fails_closed`・`autx08_live_adapter_
  duplicate_reconciles_original`）。実測値は
  `evidence/sv267/verification.md`・`evidence/sv267/
  live-adapter-leg.md`。
