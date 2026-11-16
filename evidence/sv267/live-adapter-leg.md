# Leg C — AI/live-adapter surface evidence (synvelis#267)

Date: 2026-11-16. Branch: `feat/267-autx08-live-adapter-leg`
（base `0b57299`）。Slice: AUTX-08 Leg C — AI/live-adapter
surface leg（same-wire contract の第 4 surface）。

本記録は headless lane のみを対象とする。`step_wire`
（`StepCall` → wire JSON producer）は adapter-internal
private fn（`crates/synvelis-automation/src/live.rs` L227）
であり wire byte は観測不能なため、公開 `StepLane`
surface（`invoke`/`operation_result`）が産出する
`GatewayOutcome`/`ErrorEnvelope`/`DuplicateRequest`
parity で証明する — `docs/test-specs/autx-08-common-e2e.md`
Rule AUTX-E2E-SAME-01 の MCP leg convention
（adapter-internal wire を outcome/error parity で証明）
と同型。

## 構成

`crates/synvelis-workspace/tests/autx08_e2e.rs` に Leg C
section を追加（sibling `autx08_live_e2e.rs` ではなく
拡張を採用 — Leg B helper
`note_add_args`/`issue_agent_authority`/`gui_leg`/`cli_leg`/
`mcp_prepare_call`/`wire`/`expected`/`complete`/
`mcp_structured` をそのまま再利用し parity 比較を同一
file 内で完結させるため。判断理由は `docs/discovery/
autx-08-live-adapter-leg.md` Alternatives）。

- `live_lane(with_authority)`: `rig::compose()` の fresh
  `GatewayStack` 上に production `GatewayStepLane::new`
  を組立 — `s.gw` を `Arc<Mutex<..>>`、`LocalCliAdapter::
  new(CLI_ADAPTER_RAW, MemoryCliSessionStore, s.clock)`、
  `s.ports.clone()` を `Arc<dyn CommitPort>`、`s.jobs`
  （`compose(None)` では `None`）。`with_authority` 時
  のみ `issue_agent_authority` で `t1`/`g1` を発行。
- `live_step_call()`: `StepCall` を Leg B 論理操作で
  構築 — `VersionedRef::new(PREPARE_CAPABILITY, 1, 0)`、
  `args: note_add_args()`（`{"operation":
  {"capability": "synvelis.midi.note.add@1.0", ...}}`
  shape verbatim）、`target: None`、`expected_revision:
  Some(1)`、`idempotency_key: "autx08-live-1"`、
  `policy_epoch: None`、`principal: agent_principal()`、
  `project_id: project().to_tagged()`、`task:
  Some(task_ref("t1"))`、`grant: Some(grant_ref("g1"))`。
- dev-dep: `crates/synvelis-workspace/Cargo.toml`
  `[dev-dependencies]` に `synvelis-automation = { path =
  "../synvelis-automation" }`（Coordinator 承認済み —
  acyclic・headless-safe・外部依存は serde/serde_json/
  sha2 のみ・`synvelis-gateway`/`synvelis-cli` 同一先例。
  sv293 Worker の `[dependencies]` 追加とは別 section）。
  `Cargo.lock` 差分は `synvelis-workspace` deps 欄への
  `synvelis-automation` 1 行追加のみ。

## 4 assertion（全て Pass — headless）

| # | assertion | test | 結果 |
| --- | --- | --- | --- |
| (i) | outcome parity: `invoke` の `GatewayOutcome<Value>` が Leg B GUI/CLI outcome および MCP `structuredContent` decode と同一 payload | `autx08_live_adapter_lands_same_outcome` | **Pass (headless)** |
| (ii) | SnapshotExpired parity: `expected_revision: Some(99)` で `Err(Box<ErrorEnvelope>)` — `code`/`category`/`retry`/`message` が Leg B surface と一致（MCP `ExternalError` 投影とも一致） | `autx08_live_adapter_unavailable_revision_fails_identically` | **Pass (headless)** |
| (iii) | fail-closed denial: `task: None`/`grant: None` の delegated agent `StepCall` が CLI/MCP leg の denial と同一 typed `ErrorEnvelope` で fail-closed | `autx08_live_adapter_without_authority_fails_closed` | **Pass (headless)** |
| (iv) | DuplicateRequest parity + `operation_result` reconcile: 同一 `idempotency_key` の再 `invoke`（lane が mint する新 `wf-req-*` request_id）が `GatewayError::DuplicateRequest{original}` を返し Leg B replay と同一 `original` を名指し、`operation_result(&original)` が `OperationObservation::Settled(GatewayOutcome::Completed)` を返す | `autx08_live_adapter_duplicate_reconciles_original` | **Pass (headless)** |

補足: `operation_result` の reconcile は stage-1
（`gateway.operation_outcome`）で prepare outcome が
settle するため `jobs: None` で十分（実測確認）。
`Human` principal の `attest` fail-closed
（`GatewayError::Authentication` 固定）は adapter
contract の別経路であり本 leg は delegated agent
経路のみを検証する。

## 実測コマンド（全て exit 0）

| コマンド | 結果 |
| --- | --- |
| `cargo test -p synvelis-workspace --locked --test autx08_e2e` | 8 passed; 0 failed（補助証拠） |
| `mise run fmt-check` | Pass |
| `mise run check synvelis-workspace` | Pass |
| `mise run lint synvelis-workspace` | Pass |
| `mise run test synvelis-workspace` | Pass（全 test binary green、`autx08_e2e` 8/8） |
| `mise run check-headless-deps` | Pass（`no GUI framework deps in 12 headless crates`） |
| `mise run test-rt` | Pass（workspace `rt_` contract 非回帰） |
| `mise exec -- prek run --all-files` | Pass |
| `audit_impl_record.js` / `audit_experiment_log.js` / `doc_status.js lint` | 本 slice の新規 doc に blocking finding なし |

環境: Windows worktree
`G:\orca\workspaces\synvelis\sv267-autx08-live-leg`、
rustc/cargo 1.99.1（mise 管理）。OS-L1 headless lane
のみ。

## NotRun / Deferred（本 slice で閉じない項目 — Pass にしない）

- real-OS GUI-L2/EXT-L3（実 GPUI window・実 OS 入力・
  実 provider・実 CLI binary・実 credential store/
  transport・multi-process durable commit）: **NotRun**
  — #114・`scarecrow173/ruviform#628`〜`scarecrow173/
  ruviform#631` 系 Gate。headless Pass を実機 Pass に
  昇格しない。
- live-registry→`syn.autx.catalog` producer（#264 接続後）:
  **Deferred**。
- `AutomationCatalog::lower`→gateway dispatch-hop の
  production consumer 接続（#264 mount wiring）:
  **Deferred**。
- `synvelis-cli` `build_request`/`claim_for` 出力と
  `lower`/`step_wire` wire の field cross-check:
  **Deferred**（`synvelis-workspace` は `synvelis-cli`
  に依存しない）。
- MCP `proposal.apply` idempotency 引数 surface・control
  channel human authority 経路（GW-08 adapter contract）:
  **Deferred**。
- `GatewayJobLane` job 観測 leg（job submit/await/cancel・
  `CancelRequested` 非昇格の honest-cancel semantics）:
  **Deferred** — #265 coordinator 接続後の別検証。
- merge 後 main SHA の必須 CI 証拠: **Deferred** —
  Coordinator が統合検証。
- `*-msrv` / `*-full` / Feature Matrix / 3OS 横断:
  **Deferred** — Daily Full CI 委譲（ユーザー指定により
  `*-msrv` は実行しない）。
