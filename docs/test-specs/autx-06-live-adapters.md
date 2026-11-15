---
id: "TSPEC-034XLays6iHG9Vsyioz6V3"
type: "test-spec"
status: "approved"
title: "AUTX-06 live adapters — real authority wire path・duplicate reconcile・honest cancel"
created: "2026-11-15"
updated: "2026-11-15"
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
    - "docs/impl/ir/autx-06-live-adapters.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/autx-06-live-adapters.md"
    - "docs/test-specs/autx-06-workflow-coordinator.md"
    - "evidence/sv265/autx-06-live-adapters-ledger.md"
  refines:
    - "docs/test-specs/autx-06-workflow-coordinator.md"
  refined-by: []
  derives-from:
    - "docs/designs/command-gateway-design.md"
  derived-by: []
  verifies:
    - "docs/designs/execution-plan-design.md"
  verified-by: []
  references:
    - "crates/synvelis-automation/tests/live_adapters.rs"
    - "crates/synvelis-gateway/tests/autx06_live_adapters.rs"
    - "crates/synvelis-automation/src/live.rs"
    - "crates/synvelis-automation/src/live_registry.rs"
  defers: []
  deferred-by: []
---

# AUTX-06 live adapters — real authority wire path・duplicate reconcile・honest cancel

## Purpose

`autx-06-workflow-coordinator` の test-spec が Mock lane で固定した
coordinator 契約を、**実 Gateway authority 経路**へ接続した
live adapter slice（synvelis#265 統合スライス N-01）の検証意図を
記録する。live adapter が第二正本を持たず、step invoke・job
observe/cancel・duplicate reconcile のすべてが `Gateway::handle`
の wire 経路と各正本の read surface を通ることを保証する。
coordinator 契約そのものが変われば本 spec は退役・再導出する。

## Feature: live StepLane/JobLane の実 authority 配線

### Rule: step invoke は `Gateway::handle` の wire 経路のみ

`GatewayStepLane::invoke` は `StepCall` を wire request に写像し
`Gateway::handle`（parse → bind → invoke）へ渡す。capability・
schema・project・target・consistency・task/grant は registry
descriptor と call の値から構成し、"latest" 解決・第二 registry・
独自 policy 評価を行わない。`synvelis.operation.batch` は
`synvelis.operation.prepare` の `operation` 引数へ lowering
され、batch 自体は直接 invoke しない。

#### Example: atomic-macro → prepare → proposal/digest

- Given: `atomic-macro` step を含む compiled plan と live
  registry snapshot
- When: coordinator が `drive` する
- Then: macro は `synvelis.operation.prepare` の
  `Completed{proposal,digest}` を返し、step output にその
  result が journal される
- Verified by: `live_macro_submit_await_completes_and_commits_once`

### Rule: principal attestation は fail-closed

run principal の caller は `LocalCliAdapter` で attest する。
`Agent` principal は delegated agent-equivalent caller となり、
`Human` principal は step call が verified-session credential を
持たないため `Authentication` で fail-closed する（agent への
silent downgrade をしない）。

#### Example: human principal の step call

- Given: `PrincipalKind::Human` の `StepCall`
- When: `GatewayStepLane::invoke` が attest を試みる
- Then: `GatewayError::Authentication` の typed error が返り、
  wire request は送出されない
- Verified by: live_adapters.rs の attest 単体 test

### Rule: job observe/cancel は job capability のみ経由

`GatewayJobLane` は `synvelis.job.get`/`synvelis.job.cancel` を
run の (principal, project, task, grant) scope で `Gateway::handle`
へ送る。`JobEngine::record`・provider 状態・job book を直接
読まず、existence-blind scope と honest cancel verdict は
権威の回答をそのまま使う。phase/cancel の wire 名が解釈不能
なら `Unknown`/`Unreachable` に落とし、推測しない。

#### Example: provider-confirmed cancel

- Given: `Waiting` run（pending job 1 件）と
  `CancelMode::Confirm` の provider stub
- When: `cancel` を呼ぶ
- Then: `job.cancel` が `confirmed` を返し、await step が
  authoritative `cancelled` phase を観測して run は `Failed`
  に settle する
- Verified by: `live_cancel_confirmed_by_provider_fails_the_run`

#### Example: provider-unreachable cancel

- Given: `CancelMode::Unreachable` の provider stub
- When: `cancel` → `drive` を呼ぶ
- Then: run は `CancelRequested` に留まり、cancel 再送も
  `Failed` 昇格も起きない
- Verified by: `live_cancel_unreachable_stays_cancel_requested`

## Feature: duplicate request の正本照合

### Rule: `DuplicateRequest` は operation_result で照合し再実行しない

`GatewayError::DuplicateRequest{original}` は
`Gateway::operation_outcome`（記録済み keyed outcome）→
`CommitPort::reconcile`（commit-bound operation）→
`JobEngine::record`（admitted job submit）の順で照合し、
`Settled`/`Pending`/`Unknown` を返す。いずれの正本にも記録が
無ければ `Unknown` であり、成功・失敗を捏造しない。

#### Example: duplicate invoke の照合

- Given: 同一 idempotency key で二重 invoke される step
- When: gateway が `DuplicateRequest{original}` を返す
- Then: `operation_result(original)` が記録済み outcome を
  `Settled` として返し、step は再実行されない
- Verified by: live_adapters.rs の duplicate/reconcile test 群

### Rule: live registry snapshot は厳密版を保持する

`capture_registry_snapshot` は Gateway の
`CapabilityRegistry`/`SchemaRegistry` を automation の
`RegistrySnapshot` へ投影し、capability/schema の厳密 version・
operation domain・target contract・effect bounds・permissions・
data-access class・execution contract・availability・入出力
schema をそのまま写す。"latest" 解決・workflow-local job schema
の捏造を行わず、投影後に `RegistrySnapshot::validate()` を通す。

#### Example: 実 rig からの snapshot 捕捉

- Given: prepare/job/context engine を install した実 Gateway
- When: `capture_registry_snapshot` を呼ぶ
- Then: snapshot が validate を通り、compile が実 capability
  ref（`synvelis.context.query@1.0` 等）を解決できる
- Verified by: `live_query_step_reads_through_context_authority`
  および live_adapters.rs の converter test 群

## Feature: live 経路での coordinator 契約維持

### Rule: re-drive は副作用を重複させない

settled step は journal checkpoint で skip され、`Waiting` 中の
再 `drive` は await の再観測のみ行う。job submit の再送・
prepare の再実行・commit の重複は起きない。

#### Example: Waiting 中の再 drive

- Given: job submit `Accepted`・await `Pending` で `Waiting` の run
- When: `drive` を再度呼ぶ
- Then: provider submissions は 1 のまま、run は `Waiting` を維持
- Verified by: `live_macro_submit_await_completes_and_commits_once`

### Rule: 権威側の再検証が step を拒否する

revocation/expiry/policy epoch 変更は lane 呼出時の
`Gateway::handle` 評価で `Denied` となり、coordinator は step
`Failed` → run `Failed` として journal する。stale grant での
継続はしない（F-6）。

#### Example: grant revoke 後の drive

- Given: `start_run` 済み・未 drive の run と発行済み grant
- When: grant を revoke してから `drive` する
- Then: submit invoke が `Denied` で失敗し run は `Failed`、
  provider submissions は 0
- Verified by: `live_grant_revocation_denies_next_step`

### Rule: workflow は commit を実行しない

atomic-macro の prepare は proposal/digest を生成するのみで
project を変更しない。mutation の確定は trusted apply +
`CommitGate` の経路であり、workflow 結果の proposal を外部の
trusted caller が apply/commit する（workflow は第二 commit
権威を持たない）。

#### Example: prepare → trusted apply → commit

- Given: 完了した run の macro step output（proposal/digest）
- When: trusted caller が `synvelis.operation.apply` を
  task/grant 付きで呼び `CommitGate::commit` する
- Then: `clip11` が muted になり、それ以前は mutation なし
- Verified by: `live_macro_submit_await_completes_and_commits_once`

## Guarantees

- live adapter 経路の全 step/job 呼出が `Gateway::handle` を
  通り、typed `GatewayOutcome`/`ErrorEnvelope` がそのまま
  coordinator の verdict/journal に届く。
- `DuplicateRequest`・evicted ledger・admitted job の各経路で
  `operation_result` が正本のみを照合し、`Unknown` を成功に
  昇格しない。
- cancel verdict（Confirmed/Pending/Unreachable/Settled）と
  job phase が wire 名どおりに写像され、provider 未確認の
  cancel が `Cancelled`/`Failed` に昇格しない。
- registry snapshot が厳密版・全 schema を保持し compile/
  実行が "latest" 解決に依存しない。

## Non-goals

- durable `RunJournal`・実 process kill・実 OS・RT lane・
  GUI/MCP/CLI 実経路 — #262/#266/#267 の Gate（前 slice ledger
  の N-02〜N-06 を継続）。
- human principal の verified-session workflow 実行 — step call
  が session credential を運ばない現契約では fail-closed が
  正しい振る舞いであり、credential 委譲の設計は別 Issue。
- `synvelis-render` 実 JobPort・実 provider 結合 — stub の
  deterministic fixture で wire/authority 契約を検証する。
- gateway lock 非保持の実測・RT callback 非該当の性能証明 —
  構造（逐次 step 境界呼出・per-dispatch mutex）で担保し、
  実測は統合 slice。

## Risk

- live adapter が authority を bypass する回帰（直接
  JobEngine/registry 参照の再導入）は coordinator 契約を
  壊さないまま検出が遅れうる — wire 経路強制と第二正本禁止を
  本 spec の rule で固定する。
- `operation_outcome` read surface の意味論変更（ledger
  eviction・key 形式）は reconcile の正しさに直結する —
  gateway 側変更時は live_adapters.rs の reconcile test が
  先に落ちる配置とした。
- job.get/job.cancel の wire result 形状変更は lane の parser
  を silent に `Unknown` 化しうる — phase/cancel の網羅
  mapping test で検出する。
