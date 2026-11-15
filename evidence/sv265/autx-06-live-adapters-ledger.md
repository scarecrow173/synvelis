# AUTX-06 live adapters — Case Ledger（synvelis#265 live authority slice）

Date: 2026-11-15。Slice: `synvelis-automation` に production
live adapter を追加（`live.rs` の `GatewayStepLane`/
`GatewayJobLane`、`live_registry.rs` の live registry
projection）、`synvelis-gateway` に最小 keyed-outcome read
surface（`Gateway::operation_outcome`）を追加し、headless
coordinator を実 Gateway authority 経路で駆動する e2e を
`synvelis-gateway/tests/autx06_live_adapters.rs` に実装。
**本 slice は headless の live authority 結合のみ**を対象とし、
実 OS process kill・実 provider・GUI/MCP/CLI 実経路・RT lane
実測・Crash Safety 受入は実行していない（統合スライス —
NotRun/Deferred）。headless の Pass を runtime/実機の Pass
に昇格しない。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/265
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis branch | `scarecrow173/sv265-live-adapters`（worktree `sv265-live-adapters`） |
| OS / CPU / FS | Windows 11 / Intel Core i9-9900K / NTFS（C:） |
| Toolchain | Rust `1.99.1`（mise 管理。CI pin は `1.99.0`）、edition 2024、MSRV 1.89 |
| Build | mbx managed target（worktree `target` リンク先。F: へ中間生成物を置かない） |
| Lane/Job fixture | 実 `Gateway` + `JobEngine`/`CommitGate`/`ProviderStub`（`tests/rig`）。live adapter 単体は実 `Gateway::handle` + recording stub executor（`tests/live_adapters.rs`） |

## 判定語彙

- **Pass**: 当該 SHA で `mise run` task が exit 0 で完了し、
  対象 test/assertion が green だったことを実測した。
- **NotRun**: 本 slice で実行していない項目（後続 slice・
  MSRV lane・実機/実 Job 結合）。
- **Deferred**: 別 Issue が Owner の項目（#262/#266/#267/
  #273）。

## 実行した正規検証

| コマンド | 結果 |
| --- | --- |
| `mise run fmt` / `mise run fmt-check` | Pass（exit 0） |
| `mise run check synvelis-automation` | Pass（exit 0） |
| `mise run check synvelis-gateway` | Pass（exit 0） |
| `mise run lint synvelis-automation` | Pass（exit 0、clippy warning 0） |
| `mise run lint synvelis-gateway` | Pass（exit 0、clippy warning 0） |
| `mise run test synvelis-automation` | Pass（168 test: live_adapters 15・coordinator 17・compiler 43・他既存 suite。doc-test 0） |
| `mise run test synvelis-gateway` | Pass（444 test / 28 binary: autx06_live_adapters 5・他既存 suite。doc-test 0） |
| `mise run test-rt` | Pass（`rt_` filter 全件 ok。workspace 全 crate compile + rt contract test） |
| `mise run check-headless-deps` | Pass（12 headless crates に GUI/GPU/WebView framework 依存なし） |
| `doc_maintenance.js index apply`（docs/test-specs・docs/impl/ir・docs/impl/exp） | Pass（test-specs/ir 各 1 written・exp 0 written = 既に整合） |
| `doc-status lint`（autx-06-live-adapters 3 doc） | Pass（blocking finding 0。`test-spec-missing-verifies`・`index-missing-entry`・`broken-relation-link` を修正済み。repo 全体の既存 finding は本 slice 対象外） |
| `*-msrv` 系 | NotRun（ユーザー指定により未実行） |

補助証拠（canonical = 上記 mise task）: `cargo test
-p synvelis-automation` 168 passed、`cargo test
-p synvelis-gateway` 444 passed、`cargo clippy
--all-targets` 両 crate warning 0。

## 受入条件 → case 対応表

### AC: step invoke は `Gateway::handle` の wire 経路のみ（test-spec Rule 1）

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| WIRE-01 scope 転送 | principal（caller attestation 経由）・project/task/grant・idempotency key が wire request に載る。`policy_epoch` は wire field を持たず journaled run metadata であり、policy 再検証は authority が live `policy_epoch()` で行う | recording executor が `Seen` で受領・全 field 一致 | Pass | `step_invoke_forwards_scope_through_real_wire_path` |
| WIRE-02 expected_revision pin | mutation step は expected_revision を要求 | 欠落時に bind/validation で拒否 | Pass | `step_invoke_mutation_requires_expected_revision_pin` |
| WIRE-03 batch lowering | `synvelis.operation.batch` は `operation.prepare` wire に降格 | prepare descriptor/schema で bind・target なし | Pass | `step_invoke_batch_wraps_as_prepare_operation` |
| WIRE-04 atomic-macro e2e | macro → prepare → proposal/digest が実 rig で完走 | run `Completed`・trusted apply で 1 回だけ commit | Pass | `live_macro_submit_await_completes_and_commits_once` |
| WIRE-05 query domain | context query step が context authority 経由で完走 | run `Completed`・query outcome 回収 | Pass | `live_query_step_reads_through_context_authority` |

### AC: principal attestation は fail-closed（test-spec Rule 2）

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| AUTH-01 human fail-closed | verified-session credential なしの human principal は `Authentication` で拒否・agent 等価へ降格しない | `GatewayError::Authentication` | Pass | `step_invoke_human_principal_fails_closed` |

### AC: job observe/cancel は job capability のみ経由（test-spec Rule 3）

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| JOB-01 observe 写像 | `job.get` payload → `JobObservation::Live`（phase/cancel/progress/result） | pending/running/completed/failed/cancelled 全 phase・cancel 4 値写像 | Pass | `job_observe_maps_live_payload`・`job_observe_completed_carries_disclosed_result` |
| JOB-02 parse 不能は Unknown | 解釈不能 phase/cancel を推測しない | `JobObservation::Unknown` | Pass | `job_observe_unparseable_phase_is_unknown_never_guessed` |
| JOB-03 cancel 写像 | `job.cancel` verdict → `CancelResponse`（Confirmed/Pending/Unreachable/Settled） | provider verdict をそのまま記録 | Pass | `job_cancel_maps_authoritative_verdicts` |
| JOB-04 capability 不在は honest Unknown | job capability を持たない lane は Unknown/Unreachable | `Unknown`・`Unreachable` | Pass | `job_lane_without_job_capabilities_is_honestly_unknown` |
| JOB-05 confirmed cancel e2e | provider-confirmed cancel で run が正直に失敗終端 | run `Failed`（cancelled を偽らない） | Pass | `live_cancel_confirmed_by_provider_fails_the_run` |
| JOB-06 unreachable cancel e2e | provider unreachable は `CancelRequested` に留まり終端化しない | run `CancelRequested` のまま | Pass | `live_cancel_unreachable_stays_cancel_requested` |

### AC: duplicate request の正本照合（test-spec Rule 4/5）

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| DUP-01 ledger 照合 | keyed outcome が ledger に残る間は `Settled` を返し再実行しない | duplicate invoke が side effect 0 で outcome 回収 | Pass | `step_invoke_duplicate_reconciles_through_operation_result` |
| DUP-02 証拠なしは Unknown | ledger evict・commit 未確定・job 記録なしは `Unknown`（推測しない） | `OperationObservation::Unknown` | Pass | `step_operation_result_unknown_for_unrecorded_operation`・`step_operation_result_commit_reconcile_pending` |
| DUP-03 commit reconcile | commit-bound op は `CommitPort::reconcile` が正本。記録済み commit verdict（Committed/Rejected/Conflicted/Indeterminate いずれも）は apply の admission を証明し、operation-level outcome は `Accepted`（commit verdict 自体は commit port の正本） | Pending/Settled を authority 判定で返す | Pass | `step_operation_result_commit_reconcile_pending`・`step_operation_result_commit_settled_is_operation_accepted` |
| REG-01 live snapshot | 実 Gateway registry/schema から `RegistrySnapshot` を厳密版で投影・workflow-local fallback なし | descriptor/schema が live registry と一致 | Pass | `registry_capture_projects_live_metadata` |

### AC: live 経路での coordinator 契約維持（test-spec Rule 6/7/8）

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| COORD-01 re-drive 無副作用 | Waiting/承認待ちの再 drive は副作用を重複させない | step 冪等 key で同一 operation に収束 | Pass | `live_macro_submit_await_completes_and_commits_once`（re-drive 区間含む） |
| COORD-02 権威側再検証 | grant revoke 後の step は権威が拒否・coordinator は第二正本を持たない | 次 step が `Denied` で run 停止 | Pass | `live_grant_revocation_denies_next_step` |
| COORD-03 commit は workflow 外 | `Accepted` proposal の commit/apply は trusted caller が実 commit gate 経由で行い workflow は commit しない | apply→commit が rig の `CommitGate` で 1 回だけ成功 | Pass | `live_macro_submit_await_completes_and_commits_once` |

## NotRun / Deferred（Case ID 付き）

| Case ID | 項目 | 判定 | 理由・後続 |
| --- | --- | --- | --- |
| N-01 | 実 OS process kill・永続ストア跨ぎの Crash Safety 受入 | Deferred | durable `RunJournal` 実装 + 実 OS kill は #267 Gate |
| N-02 | 実 OS（Windows/macOS/Linux）real FS/Job/Process | NotRun | #267 で別 Pass/Fail/Unsupported 判定 |
| N-03 | GUI Window Close/MCP disconnect/CLI process cancel 実経路 | Deferred | #262/#266/#267。headless では `run_id` 再照会・cancel・result 契約のみ検証 |
| N-04 | RT lane・Concurrency 実負荷・Gateway mutex 非保持の実測 | NotRun | headless crate 構造で非 RT・逐次境界呼出を担保（shared `Arc<Mutex<Gateway>>` は dispatch 毎にのみ lock）。実測は統合 slice |
| N-05 | YAML/Luau/Rhai 混在 Definition での実行 | Deferred | Script Builder #273。Source 非依存 `ExecutablePlan` の同一 FSM は型構造で担保 |
| N-06 | 実 provider（非 `ProviderStub`）job の live observe/cancel | NotRun | `ProviderStub` は deterministic `JobPort`。実 provider 結合は統合 slice |
| N-07 | `mise run gw10` | NotRun | gateway suite 非回帰は `test synvelis-gateway` で代替確認。gw10 は任意実行枠 |
| N-08 | `*-msrv` 系 | NotRun | ユーザー指定により未実行 |

## Issue 受入条件との対応（本 slice での扱い）

- 「live capability invocation は authoritative
  `Gateway::handle` / wire binding / invocation pipeline」:
  WIRE-01〜05・AUTH-01 で実 `Gateway::handle` 経路を検証。
  adapter は descriptor/schema を live registry から解決し
  workflow-local schema・"latest" fallback を持たない。
- 「live job observation/cancellation は authoritative
  Gateway job domain」: JOB-01〜06 で `synvelis.job.get`/
  `synvelis.job.cancel` capability 経由のみを検証。
  `GatewayJobLane` は provider state を直接読まず、job
  record も observe/cancel では参照しない。
- 「第二正本を持たない」: coordinator は引き続き journal・
  resume cursor・step verdict のみ所有。registry は
  `capture_registry_snapshot` の一方向投影、commit fence・
  project lock・grant・approval state は複写しない。
- 「idempotency・honest indeterminate・cancel semantics
  の維持」: DUP-01〜03・COORD-01〜03・JOB-05/06 で検証。
  証拠なしの `Unknown`/`Indeterminate` を成功・失敗へ昇格
  させない契約は live 経路でも維持。
- 「minimal Gateway read surface」: `Gateway::operation_outcome`
  のみ追加（keyed outcome の bounded ledger read）。
  reconcile は ledger → `CommitPort::reconcile` →
  `JobEngine::record`（admission evidence のみ）の 3 段で、
  いずれも既存正本の read であり新たな権威を作らない。
- 「headless e2e over real authority」: macro→prepare→
  submit→await→emit→trusted apply/commit、query domain、
  grant revocation、confirmed/unreachable cancel、
  re-drive を実 rig で検証（autx06_live_adapters 5 test）。
