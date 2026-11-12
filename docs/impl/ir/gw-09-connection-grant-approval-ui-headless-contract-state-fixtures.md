---
id: "IMPL-0Gw09UiContractState01"
type: "impl"
status: "completed"
title: "GW-09 Connection/Grant/Approval UI — Headless Contract・State・Fixtures (Explicit Scope・3 Modes・Receipt/Revocation UX)"
created: "2026-11-15"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/60"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/src/gateway_ui.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/rig/mod.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/gw09_ui.rs"
      - type: "file"
        path: "docs/impl/ir/gw-09-connection-grant-approval-ui-headless-contract-state-fixtures.md"
    modified:
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-workspace/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "https://github.com/scarecrow173/synvelis/issues/60"
  implemented-by: []
  depends-on:
    - "docs/impl/ir/gw-01-gateway-contract.md"
    - "docs/impl/ir/gw-02-authorization-policy.md"
    - "docs/impl/ir/gw-03-context-projection.md"
    - "docs/impl/ir/gw-04-semantic-operation-preparation-effect-evaluation-linked-midi-scope.md"
    - "docs/impl/ir/gw-05-immutable-prepared-changeset-semantic-preview-approval-receipt-state-machines.md"
    - "docs/impl/ir/gw-06-authorized-commit-gate-revoke-linearization-idempotency-crash-recovery.md"
    - "docs/impl/ir/gw-08-mcp-server-adapter-trusted-local-connection-midi-3mode-e2e.md"
    - "https://github.com/scarecrow173/synvelis/issues/45"
  blocks:
    - "https://github.com/scarecrow173/synvelis/issues/45"
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/60"
  derived-by: []
  verifies:
    - "https://github.com/scarecrow173/synvelis/issues/60"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "headless UI seam — GW-09 contract を GUI 非依存の projection/state として workspace crate に実装"
    rejected: []
---

# GW-09 Connection/Grant/Approval UI — Headless Contract・State・Fixtures

## Summary

issue #60 の UI 層を、GUI 非依存の headless contract / state /
fixture として `synvelis-workspace` に実装した。UI は authorization
truth を一切持たず、adapter attestation(`AdapterSeal`)経由で bound した
`CallerIdentity` + 明示 id のみを GW-02/05/06 へ渡す。GW-01 registry /
GW-02 grant/policy / GW-05 proposal・approval・apply / GW-06 commit
gate / job coordinator がそれぞれの権威に留まる。

## Context

- Issue: #60(GW-09 本体)。要件の出典は issue #60 の UX/SYS 契約。
- 依存 IR(`depends-on`): GW-01 gateway contract・GW-02 authorization
  policy・GW-03 context projection・GW-04 semantic operation preparation・
  GW-05 prepared changeset/approval receipt・GW-06 commit gate・
  GW-08 MCP server adapter(`docs/impl/ir/gw-01..06,08` 各 IR)。
- 後続: `blocks` issue #45(GW 全体の完了条件)が本 UI contract を含む。
- 範囲注記: real-device GUI 検証は従来どおり manual gate — 本 impl は
  host shell が render するための row/explanation projection と、それを
  駆動する typed-wire session(`GatewayUi`)のみを提供する。

## Implementation

### 実施した変更

| 範囲 | 内容 |
| --- | --- |
| `Cargo.toml` | `synvelis-workspace` に `synvelis-gateway`(path)+ `serde_json` の依存追加。依存方向は workspace → gateway(一方向、`synvelis-gateway` は UX を持たない GW-01 ADR の境界と整合) |
| `src/gateway_ui.rs`(新規) | GW-09 session `GatewayUi::new(caller, gateway, deps)`。deps = `GatewayUiDeps{policy, admin, permits, proposals, proposal_engine, committer, jobs?, clock}`。公開面:`AiFeature` 独立 switch + `AiPanelView`(UX-01)、`issue_connection`/`revoke_connection`/`connection_rows`(permit 投影、`check_channel_kind` 共有)、`issue_task`/`revoke_task`/`task_rows`、`issue_grant`/`revoke_grant`/`grant_rows`/`grant_explanation`(exact/dynamic/snapshot scope・linked-source 注意・expiry/epoch ceiling/apply budget — UX-02)、`note_proposal`/`open_approval`/`refresh_surface`/`approve`/`decline`/`apply`((proposal,digest) pin + reviewed base revision — UX-03)、`cancel_job`/`operation_rows`/`note_job`/`job_rows`(`JobPhase` と `CancelLink` を別軸で投影 — SYS-05)、`note_engine`/`note_saved`/`publications`(commit verdict・engine state・saved/publication を別項目で保持 — SYS-06) |
| `src/lib.rs` | `gateway_ui` module + public re-export |
| `tests/rig/mod.rs`(新規) | GW-08 と同型の real seam 構成: deterministic linked-MIDI demo project(project 7 / track 3 / lane 4 clip 11+12 / lane 5 clip 13 / clip11→source9+asset5 / clip12+13→source10 / note 20+21 / revision 1 snapshot)、`InMemoryPolicyStore`・`ManualClock`・`MemorySnapshotSource`・`GrantPolicy`・`ContextProjectionEngine`・`ProposalStore`・`PrepareEngine::with_media`・`ProposalEngine`・`ProposalGate`・`Gateway`・`CommitGate`(`DirectoryBundleStore`+journal)・任意 `JobEngine`(stub provider + scripted `CancelMode`)。`FixtureAdapter` で human/agent caller を seal。admin/port seam は scripted 失敗注入(`fail_revoke_grants`)と live-revision 参照を持ち、wire builder は request_id/capability/schema/project/args/consistency を明示 shape で組む |
| `tests/gw09_ui.rs`(新規) | 21 テスト。UX-01 独立 switch + 既定 off、UX-02 exact/snapshot/dynamic scope + expiry/epoch/budget + linked-source honesty、UX-03 surface の全項目 + linked-source/MIDI-note 行、UX-04 明示 id 行、UX-05 activity feed の軸分離、UX-06 AI off で DAW 非回帰、SYS-01/02/03/04/05/06 + Suggest/ApproveToApply/DelegatedApply 3 モード + mis-approval(selection churn・other-window・proposal swap・wrong digest・expired proposal・revoked grant) |

### 設計上の主要判断

| 判断 | 理由 |
| --- | --- |
| UI state は権威 record の投影 + UI bookkeeping | connection/task/grant/surface/job row は `PolicyStore`/`ProposalStore`/`PermitStore`/`JobEngine` の record を read-model 化したもの — UI が truth を複製しない(issue の trust boundary) |
| surface は `(proposal, digest)` + reviewed base revision に pin | selection change・other-window・proposal swap・expired grant のいずれでも approve は pin 済みの内容にのみ着地する(SYS-03)。`open_approval` は digest mismatch / gone proposal を card 作成前に拒否し、`refresh_surface` は authority 側の status を再読込するだけで rebind しない |
| mode は issued grant の `ApprovalPolicy` から派生 | UI が mode を自己申告しない — Suggest/ApproveToApply/DelegatedApply は GW-02 が evaluate する authority の性質であり、UI の説明は読み取り専用(SYS-04)。apply は human caller では receipt 不要、ApproveToApply 下の agent apply は recorded receipt が必要(実機で検証) |
| revocation = intent → confirmation の 2 段階、job cancel と分離 | `Revocation{Requested,Confirmed,Unconfirmed}` は AdminPort の返り値で刻み、`cancel_job` は `synvelis.job.cancel` の request を honest に `Requested/Confirmed/Unconfirmed` で報告する — grant/connection revoke が job cancellation を暗示しない(SYS-05) |
| accepted apply ≠ commit ≠ engine ≠ saved を別軸で報告 | `apply` は `GatewayOutcome::Accepted{operation}` を受け `CommitPort` を駆動して `ApplyReport{operation, verdict}` を返す;commit verdict・engine state・saved/publication state はそれぞれ独立の欄として残る(SYS-06) |
| AI runtime と MCP adapter は独立 switch | `AiFeature{ai_runtime, mcp_adapter}` を個別に保持 — 片方 off で他方・通常 DAW 操作が回帰しない(UX-01/UX-06、SYS-01) |
| fixtures は GW-08 rig と同型の real seam | mock 出力ではなく real `Gateway`/`ProposalGate`/`CommitGate`+journal を通して UX/SYS 契約を検証 — digest pin・receipt・conflict・journal reconciliation が実機 semantics で試される |
| workspace に配置 | GW-01 ADR で gateway crate は UX を持たない — workspace 側に `synvelis-gateway` 依存を張り、session state / projection をここに置く |

## Related Experiments

`metadata.experiments.adopted` に記録された headless UI seam を採用した:
GW-09 contract を GUI 非依存の projection/state として workspace crate
(`synvelis-workspace` の `gateway_ui.rs`)に実装。`docs/impl/exp/` の
実験ファイルは存在せず、証拠は `tests/gw09_ui.rs` のテストとして
本 IR に記録する(棄却実験なし)。

## Validation

### 検証エビデンス

| 分類 | エビデンス |
| --- | --- |
| GW-09 acceptance(headless) | `cargo test -p synvelis-workspace --test gw09_ui` — 21 tests pass。明示 id・scope/linked-source/expiry/epoch/budget 説明、mis-approval 系(wrong digest・swap・expired proposal・revoked grant)、3 モード、revoke/cancel 分離、commit/engine/saved 分離、AI off 非回帰を real seam で検証 |
| workspace regression | `mise run test` — 全 workspace crate テスト green(gw09_ui 21 件を含む) |
| 静的検証 | `mise run fmt-check`/`check`/`lint`(-D warnings)すべて green |

注記: real GUI / device 上の render・focus・multi-window race は本
impl の対象外 — headless contract が「pin 済み内容にのみ着地する」
ことは approval surface の state 契約として検証済みだが、表示側の
結線は manual gate。

## Risks

- `GatewayUi` の row/surface は権威 record の投影 + UI bookkeeping であり
  UI 側に authorization truth を持たない — host が `refresh_surface`/
  `note_*` を呼ばない限り表示は stale になり得る(push 型 subscription は
  scope 外、下記 Follow-ups)。最終判定は常に GW-02/05/06 側で行われる
  ため、stale な表示が authorization を迂回する経路はない。
- approval surface は `(proposal, digest)` + reviewed base revision に
  pin されるが、pin 以降の authority 側変化(expiry・revocation)は
  `refresh_surface` の再読込でのみ検知する — 表示と実状態の乖離は
  起こり得るが、apply は実 `Gateway`/`CommitGate` を通るため pin 済み
  以外の内容が commit されることはない。
- real-device GUI の render・focus・multi-window race は本 impl では
  未検証(manual gate)— headless contract の state 保証のみが
  検証済み(下記 Follow-ups)。

## Follow-ups

- real-device GUI 検証(UX-01..06 の render 面、multi-window focus race)
  は manual gate — `GatewayUi` の row/surface projection を host shell
  へ wire する実装は別 issue。
- `note_engine`/`note_saved`/`note_job` は host 側が authority 通知を
  流す inlet であり、push 型の event subscription は GW-09 scope 外。
- connection row の表示は `PermitRecord` 投影;MCP adapter(GW-08)
  transport 上の session lifecycle と UI row の完全同期は adapter
  統合時に扱う。
