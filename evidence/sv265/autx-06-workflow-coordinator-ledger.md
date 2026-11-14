# AUTX-06 Workflow Coordinator — Case Ledger（synvelis#265 headless slice）

Date: 2026-11-14。Slice: `synvelis-automation` に Workflow
Run 実行系を追加（`run.rs` FSM/journal/replay、`eval.rs`
制限付き評価器、`coordinator.rs` headless coordinator、
`synvelis-gateway` path 依存で正本型を直接利用）。
**本 slice は headless の契約検証のみ**を対象とし、live
Gateway/Project/Job・実 process kill・実 OS・RT lane・
GUI/MCP/CLI 実経路・Crash Safety 受入は実行していない
（統合スライス — NotRun/Deferred）。headless の Pass を
runtime/実機の Pass に昇格しない。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/265
（OPEN・`Refs` のみ・Close しない）。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis branch | `scarecrow173/sv265-autx6`（worktree `sv265-autx6`） |
| OS / CPU / FS | Windows 11 / Intel Core i9-9900K / NTFS（C:） |
| Toolchain | Rust `1.99.1`（mise 管理。CI pin は `1.99.0`）、edition 2024、MSRV 1.89 |
| Build | mbx managed target（worktree `target` リンク先。F: へ中間生成物を置かない） |
| Lane/Job fixture | Mock `StepLane`/`JobLane` + `MemoryRunJournal`（`tests/coordinator.rs` 内 literal。live Gateway/Job 非依存） |

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
| `mise run fmt-check` | Pass（exit 0） |
| `mise run check synvelis-automation` | Pass（exit 0） |
| `mise run lint synvelis-automation` | Pass（exit 0、clippy warning 0） |
| `mise run test synvelis-automation` | Pass（116 test: coordinator 17・compiler 43・他既存 suite。doc-test 0） |
| `mise run check synvelis-gateway` | Pass（exit 0。Read Port 非変更の非回帰） |
| `mise run lint synvelis-gateway` | Pass（exit 0） |
| `mise run test synvelis-gateway` | Pass（exit 0） |
| `mise run check synvelis-cli` | Pass（exit 0。依存先 check） |
| `mise run check synvelis-workspace` | Pass（exit 0。依存先 check） |
| `doc-status` lint（impl-record/experiment-log/test-spec audit + index 整合） | Pass |
| `*-msrv` 系 | NotRun（ユーザー指定により未実行） |

補助証拠（canonical = 上記 mise task）: `cargo test
-p synvelis-automation` 116 passed、`cargo clippy
-p synvelis-automation --all-targets` warning 0。

## 受入条件 → case 対応表

### AC: Run/Step FSM と Replay 可能 journal（design-workflow-run.md §1/§4）

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| FSM-01 遷移表のみ許可 | 表にない遷移を拒否 | `IllegalTransition`/`ReplayDivergence` | Pass | `illegal_run_transition_is_replay_divergence` |
| FSM-02 begin なし settle 拒否 | journaled `StepBegan` なしの settle を divergence | `ReplayDivergence` | Pass | `settle_without_begin_is_replay_divergence` |
| FSM-03 torn/forged 検出 | entry digest・dense seq で検出 | `ReplayDivergence` | Pass | `journal_digest_mismatch_is_replay_divergence` |
| FSM-04 Indeterminate 非昇格 | 証拠なしに終端へ遷移しない | `Indeterminate` park → reconcile のみ抜ける | Pass | `unknown_job_observation_stays_indeterminate_until_evidence`・`lane_indeterminate_error_parks_then_reconciles_same_key` |

### AC: plan identity・冪等 3 層（design-workflow-run.md §3）

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| IDEM-01 plan digest mismatch | resume/start を拒否 | `PlanMismatch` | Pass | `plan_identity_mismatch_rejected_on_resume` |
| IDEM-02 duplicate run | `DuplicateRun` で冪等 reject | error・journal 不変 | Pass | `duplicate_start_is_idempotent_rejected` |
| IDEM-03 step 冪等 key | deterministic step-occurrence key | crash-retry で同一 key・二重呼出なし | Pass | `lane_indeterminate_error_parks_then_reconciles_same_key` |
| IDEM-04 DuplicateRequest 照合 | `operation_result` で照合・再実行なし | side effect 0・outcome 回収 | Pass | 同上 |

### AC: async submit/await・Job 正本（design-workflow-run.md §5 F-3/F-6）

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| JOB-01 submit→await→retire | pending job 会計・terminal で除去 | Waiting→Completed・pending 1→0 | Pass | `async_submit_await_completes_and_retires_pending_job` |
| JOB-02 Unknown 非昇格 | Unknown を Success/Failure にしない | `Indeterminate` park | Pass | `unknown_job_observation_stays_indeterminate_until_evidence` |

### AC: cancel honest 記録（design-workflow-run.md §5 F-7/F-8）

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| CXL-01 cancel once | `request_cancel` 1 回・`CancelResponse` をそのまま journal | Confirmed/Pending/Unreachable/Settled を記録 | Pass | `cancel_requests_once_and_provider_truth_settles` |
| CXL-02 settle race | 実正本を記録・cancelled を偽らない | `Settled{phase}`/completed 証拠 | Pass | `cancel_settled_race_records_actual_phase` |

### AC: approval/proposal park・revision conflict・budget（design-workflow-run.md §5 F-4/F-9/F-10）

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| PARK-01 approval | self-approve せず `Approval` park・再提示 | run `Approval`・lane 再呼出 | Pass | `approval_required_parks_run_without_self_approving` |
| PARK-02 proposal | apply せず `Approval` park | run `Approval` | Pass | `proposal_ready_parks_run` |
| REV-01 revision conflict | `AfterCorrection` advice 付き `Failed`・replay でも保持 | run `Failed`・`retry_policy` 保持 | Pass | `revision_conflict_fails_with_correction_advice` |
| BUD-01 runtime budget | 超過を fail-closed `Failed` | run `Failed`・journal 記録 | Pass | `runtime_budget_exceedance_fails_closed` |
| LOOP-01 bounded loop | 上限内で各 item 実行 | run `Completed`・item 毎の境界 | Pass | `bounded_loop_executes_each_item` |

### AC: 式/条件評価 scope

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| EVAL-01 scope/reject | EvalEnv 公開参照のみ解決・scope 外 reject | nested path 解決・scope 外 error | Pass | `eval_resolves_nested_paths_and_rejects_out_of_scope_refs` |

## NotRun / Deferred（Case ID 付き）

| Case ID | 項目 | 判定 | 理由・後続 |
| --- | --- | --- | --- |
| N-01 | live Gateway/Project/Job adapter 接続 | Deferred | 統合スライス。`StepLane`/`JobLane` port に live 実装を接続する後続作業 |
| N-02 | 実 process kill・永続ストア跨ぎの Crash Safety 受入 | Deferred | durable `RunJournal` 実装 + 実 OS kill は #267 Gate |
| N-03 | 実 OS（Windows/macOS/Linux）real FS/Job/Process | NotRun | #267 で別 Pass/Fail/Unsupported 判定 |
| N-04 | GUI Window Close/MCP disconnect/CLI process cancel 実経路 | Deferred | #262/#266/#267。headless では `run_id` 再照会・cancel・result 契約のみ検証 |
| N-05 | RT lane・Concurrency 実負荷・Gateway mutex 非保持の実測 | NotRun | headless crate 構造で非 RT・逐次境界呼出を担保。実測は統合 slice |
| N-06 | YAML/Luau/Rhai 混在 Definition での実行 | Deferred | Script Builder #273。Source 非依存 `ExecutablePlan` の同一 FSM は型構造で担保 |
| N-07 | `synvelis-render` 既存 JobPort との実結合 | Deferred | 統合 slice。JobPort FSM fixture で代替 |
| N-08 | `mise run gw10` | NotRun | gateway suite 非回帰は `test synvelis-gateway` で代替確認。gw10 は任意実行枠 |
| N-09 | `mise run test-rt` | NotRun | RT lane は統合 slice（N-05 と同根拠） |
| N-10 | `*-msrv` 系 | NotRun | ユーザー指定により未実行 |

## Issue 受入条件との対応（本 slice での扱い）

- 「YAML/Luau/Rhai が同じ FSM/Cancel/Resume/Indeterminate を
  利用」: Source 非依存 `ExecutablePlan` が同一 Run/Step FSM
  を駆動することを構造で担保（混在 Definition 実行は N-06）。
- 「逐次 Query→Atomic Batch→Job Submit→Await→Result/If/
  Bounded Loop が同 Plan で実行可能」: `basic_workflow_*`・
  `async_submit_await_*`・`bounded_loop_*` で headless に検証。
  Project 単一 Writer/1 Batch Undo は Project 側権威であり
  本 slice では journal に状態を複写しない。
- 「Run/Step FSM・不正遷移と循環 Trigger 拒否・Replay 可能」:
  FSM-01〜04 で検証。循環 Trigger は plan 検証（#264）+
  coordinator 側 fail-closed。
- 「Crash/Timeout/Cancel 不確定/Provider Restart からの
  Resume で重複副作用 0・Unknown→Success 昇格 0」:
  IDEM/JOB/CXL/REV case で fixture 検証。実 process kill・
  永続ストア跨ぎは N-02。
- 「Revocation/Expiry/PolicyEpoch/Grant Scope/Capability
  Retired/Revision stale の各 Step で再検証」: coordinator は
  step 境界ごとに `RunContext` の registry snapshot・
  expected revision を lane 呼出へ渡し、再検証自体は
  Gateway/Project/Job の権威に委譲。本 slice では権威側の
  再検証を fixture error（RevisionConflict 等）で確認。
- 「Workflow Engine が GW/CommitFence/Project Mutex を保持
  して再帰 Invocation しない・RT callback alloc/block/IO
  なし」: headless crate 構造 + 逐次 step 境界呼出で担保。
  実測は N-05。
- 「GUI Window Close/MCP disconnect/CLI process cancel の
  各時点で Job Owner 状態が消えず Resume/CancelRequest/
  Result 再照会が正しい」: headless では `status`/`cancel`/
  `result` の `run_id` 再照会契約を検証。実経路は N-04。
