# AUTX-06 Mixed-Source Workflow Run Parity — Case Ledger（synvelis#265 headless slice）

Date: 2026-11-16。Slice: `synvelis-automation` に mixed-source
run parity の headless 統合テスト
`crates/synvelis-automation/tests/mixed_source_run.rs` を追加
（src 変更ゼロ — `compile_workflow_yaml` /
`compile_script_workflow` / `import_workflow_document` /
`WorkflowScriptBuilder` / coordinator・run 公開面は全て
export 済みを利用）。**本 slice は headless の契約検証のみ**
を対象とし、durable journal・実 process kill・実 OS・RT lane・
GUI/MCP/CLI 実経路・実 provider/`synvelis-render` JobPort
結合・compile parity の再証明は実行していない
（統合スライス — NotRun/Deferred）。headless の Pass を
runtime/実機の Pass に昇格しない。

対象 Issue: https://github.com/scarecrow173/synvelis/issues/265
（OPEN・`Refs` のみ・Close しない）。
試験仕様: `docs/test-specs/autx-06-mixed-source-run.md`。

## 計測環境

| 項目 | 値 |
| --- | --- |
| Synvelis branch | `test/265-mixed-source-run`（worktree `sv265-mixed-source-run`） |
| OS / CPU / FS | Windows 11 / Intel Core i9-9900K / NTFS |
| Toolchain | Rust `1.99.1`（mise 管理。CI pin は `1.99.0`）、edition 2024、MSRV 1.89 |
| Build | mbx managed target（worktree `target` リンク先） |
| Lane/Job fixture | Mock `StepLane`/`JobLane` + `MemoryRunJournal`（`tests/mixed_source_run.rs` 内 literal。live Gateway/Job 非依存） |
| Script fixture | `ScriptApiModel`/`BindingSet` 4-cap surface（`tests/importer.rs` 構成を拡張）+ `MapWorkflowResolver` |

## 判定語彙

- **Pass**: 当該 SHA で `mise run` task が exit 0 で完了し、
  対象 test/assertion が green だったことを実測した。
- **NotRun**: 本 slice で実行していない項目（後続 slice・
  MSRV lane・実機/実 Job 結合）。
- **Deferred**: 別 Issue が Owner の項目（#262/#264/#266/
  #267/#273）。

## 実行した正規検証

| コマンド | 結果 |
| --- | --- |
| `mise run fmt-check` | Pass（exit 0） |
| `mise run check synvelis-automation` | Pass（exit 0） |
| `mise run lint synvelis-automation` | Pass（exit 0、clippy warning 0） |
| `mise run test synvelis-automation` | Pass（exit 0。unit 1 + integration 12 suite 計 174 test — `mixed_source_run` 6 test を含む全 green。doc-test 0） |
| `doc-status` audit（impl-record / experiment-log / test-spec） | Pass（下記「doc audit」参照） |
| `*-msrv`・`*-full`・`test-rt`・`gw10`・`check-headless-deps` | NotRun（タスク指定どおり Daily Full CI・統合 gate へ委譲。dep 変更なしのため `check-headless-deps` 不要） |

補助証拠（canonical = 上記 mise task）:
`cargo test -p synvelis-automation --test mixed_source_run`
6 passed / 0 failed（exit 0）。

## 受入条件 → case 対応表

### AC-1: PAR-01 3-source run parity

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| PAR-01a | 同一 semantic の YAML/Luau/Rhai が同一 `WorkflowDefinition`・`plan.step_order`・`plan.definition_digest` に収束 | 3 source 全て `PartialEq`・step_order/digest 一致 | Pass | `assert_source_convergence`（全 4 artifact group で実行） |
| PAR-01b | 同一 coordinator/journal/lane script で `start_run`→`drive`→終端まで実行 | 各 source `RunState::Completed`・`result.note` 一致・lane 呼出 2 回 | Pass | `par01_yaml_luau_rhai_share_one_run_trace` |
| PAR-01c | journal `JournalKind` 列・Run/Step 状態遷移・checkpoint verdict・終端 `RunState` が 3 source 間で一致 | masked entry signature・run signature・call signature が完全一致 | Pass | `assert_trace_parity` |
| PAR-01d | plan digest は witness 記録のみ（gate にしない） | `6779664f07bc06672487d6f9603e078ba25af3c214dae22d340ecead16b1492d` が YAML/Luau/Rhai で**完全一致**（BASIC_WORKFLOW） | Pass（観測事実） | `par01` eprintln witness。compile parity の再証明は #264/#273 の責務のため gate せず |

### AC-2: CXL-P cancel honesty parity

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| CXL-P | drive 中途 `cancel` → provider verdict どおりの settle・cancelled を偽らない・cancel 再送なし — journal trace が 3 source で同一 | `Waiting`→`CancelRequested`（`CancelResponse::Pending` をそのまま記録）→ provider `Cancelled`+`CancelLink::Confirmed` 観測で `Failed` settle。`cancel_count()==1`（再送 0）。3 source trace 完全一致 | Pass | `cxl_p_cancel_settles_identically_across_sources` |

### AC-3: IND-P indeterminate→reconcile parity

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| IND-P | unknown observation → `Indeterminate` park（昇格なし・`result()` 拒否）→ `reconcile` で同一回復 trace・await は job 再 submit しない | `JobObservation::Unknown` で `RunState::Indeterminate`・`RunError::NotCompleted`。`live_completed` 証拠投入後 `reconcile` → `Completed`・`result.status=="succeeded"`。lane 呼出 2 回のまま（再 submit 0）。3 source trace 完全一致 | Pass | `ind_p_unknown_observation_reconciles_identically` |

### AC-4: RPL-P crash-restart replay parity

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| RPL-P | journal を新規 `MemoryRunJournal` へ再構成 → `replay_run` が同一 projection・resume が journaled idempotency key を再利用（副作用 0） | header+entries を新 journal へ `create_run`/`append` で移送し `replay_run` の projection が live `WorkflowRun` と signature 一致。`reconcile` resume で retry した `add` 呼出が parked occurrence と同一 key。lane 呼出 3 回・`calls[1].key==calls[2].key`。3 source trace 完全一致 | Pass | `rpl_p_rebuilt_journal_replays_and_resumes_without_duplicate_effects` |

### AC-5: XSRC-01 異種 workflow() chain

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| XSRC-01 | Luau→Rhai→YAML の `kind:"workflow"` pin 参照を `expand_workflow_call` で inline した plan が同一 FSM で完走・callee step が namespace 付きで journal 記録 | `step_order == ["mid__leaf__tracks","done"]`。`StepBegan` entry が `step=mid__leaf__tracks`・`capability_ref=synvelis.query.tracks@1.0`・`idempotency_key=run-xsrc:mid__leaf__tracks`。`result.picked` が lane 戻り値と一致。lane 呼出 1 回 | Pass | `xsrc_01_heterogeneous_workflow_call_completes_on_same_fsm` |

### AC-6: SRC-0 journal/projection の source・VM 非含有

| Case ID | Expected | Observed | 判定 | 根拠 |
| --- | --- | --- | --- | --- |
| SRC-0 | journal 全 entry・run header・projection を serde JSON 化し source/backend/VM 由来 field が存在しないことを fail-closed に assert | yaml/luau/rhai/xsrc 4 artifact の header・全 entry・projection（steps/outputs/step_outputs/pending_jobs/cancel 系/cursor）を再帰走査。`luau`/`rhai`/`yaml`/`language*`/`backend*`/`script*`/`vm`/`source`/`dialect`/`bytecode`/`ast`/`eval` の substring＋delimited-token 二段検査で違反 0 件 | Pass | `src0_journal_and_projection_carry_no_source_or_vm_provenance` |

## NotRun / Deferred（Case ID 付き）

| Case ID | 項目 | 判定 | 理由・後続 |
| --- | --- | --- | --- |
| N-01 | durable `RunJournal`（filesystem/DB 実装）跨ぎの replay | Deferred | `scarecrow173/synvelis#267` Gate。`MemoryRunJournal` は契約検証用 fixture |
| N-02 | 実 OS process kill・Provider Restart からの Resume | Deferred | `scarecrow173/synvelis#267` Gate。in-process journal 移送で同一 seam を検証済み |
| N-03 | GUI Window Close/MCP disconnect/CLI process cancel 実経路 | Deferred | `scarecrow173/synvelis#262`/`#266`/`#267`。headless では `run_id` 再照会・cancel・result 契約のみ |
| N-04 | 実 provider/`synvelis-render` 既存 JobPort との実結合 | Deferred | 統合 slice。Mock `JobLane`/`StepLane` で FSM 契約を検証 |
| N-05 | RT lane・Concurrency 実負荷・GW mutex 非保持の実測 | NotRun | headless crate 構造で逐次境界呼出を担保。実測は統合 slice |
| N-06 | compile parity（同一 semantic → 同一 digest）の再証明 | Deferred | `scarecrow173/synvelis#264`/`#273` の責務。本 slice は digest 一致を witness 観測（BASIC_WORKFLOW で 3 source 完全一致を確認）のみで gate しない |
| N-07 | `mise run gw10` / `mise run test-rt` | NotRun | タスク指定どおり Daily Full CI・統合 gate へ委譲 |
| N-08 | `*-msrv`・`*-full` 系 | NotRun | タスク指定どおり Daily Full CI へ委譲 |
| N-09 | `mise run check-headless-deps` | NotRun | dep 変更なし（tests/docs のみ）のため不要 |
| N-10 | docs index（`docs/**/README.md`）再生成 | Deferred | Coordinator が統合時に `doc-maintenance` で実施（共通契約どおり本 slice では未実行） |

## Issue 受入条件との対応（本 slice での扱い）

- 「YAML/Luau/Rhai が同じ Workflow Run/Step FSM/Cancel/
  Resume/Indeterminate を利用し VM 内部状態の永続化・
  二重 Commit/Job/Grant が 0 件」: PAR-01/CXL-P/IND-P/RPL-P
  で 3 source 同一 trace・SRC-0 で provenance 0 件を headless
  検証。実経路の二重副作用 0 は N-02/N-04。
- 「YAML/Luau/Rhai 混在 Workflow の Run/Step/Cancel/Resume が
  同じ State Machine・VM 状態を保存せず二重 Job/Commit しない」:
  XSRC-01 で Luau→Rhai→YAML chain が同一 FSM で完走、
  namespace 付き step・決定的 idempotency key を確認。
- 「逐次 Query→Atomic Batch→Job Submit→Await→Result/If/
  Bounded Loop が同 Plan で実行可能」: basic（query→invoke→
  emit）と async（async-submit→query→await-job→emit）の
  2 semantic を 3 source で完走。Project 単一 Writer/
  1 Batch Undo は Project 側権威（本 slice 対象外）。
- 「Run/Step FSM 遷移を Replay 可能な有限状態機械とし
  不正遷移と循環 Trigger を拒否」: FSM 自体は
  `autx-06-workflow-coordinator` slice で検証済み
  （FSM-01〜04）。本 slice は source 非依存の再確認。
- 「Project Commit 後 Journal 前 Crash・Job 完了後 Record 前
  Crash・Network Timeout・Cancel 不確定・Provider Restart
  からの Resume で重複副作用 0・Unknown→Success 昇格 0」:
  RPL-P（journal 移送 replay + key 再利用）・IND-P（Unknown
  非昇格 → reconcile）・CXL-P（cancel 不確定の honest 記録）
  で 3 source parity として検証。実 process kill・永続
  ストア跨ぎは N-01/N-02。
- 「Revocation/Expiry/PolicyEpoch/Grant Scope/Capability
  Retired/Revision stale の各 Step で再検証」: coordinator は
  step 境界で `RunContext` の registry snapshot・expected
  revision を lane 呼出へ渡す（source 非依存）。権威側の
  再検証は live adapter slice の責務。
- 「Workflow Engine が GW/CommitFence/Project Mutex を保持
  して再帰 Invocation しない・RT callback alloc/block/IO
  なし」: headless crate 構造で担保。実測は N-05。
- 「GUI Window Close/MCP disconnect/CLI process cancel の
  各時点で Job Owner 状態が消えず Resume/CancelRequest/
  Result 再照会が正しい」: headless では `status`/`cancel`/
  `result` の `run_id` 再照会契約を 3 source で検証。
  実経路は N-03。

## doc audit

- `audit_impl_record.js --dir docs/impl/ir`: 本 slice の
  `autx-06-mixed-source-run.md` に error/warning なし
  （index README 未同期 warning のみ — N-10）。
- `audit_experiment_log.js --dir docs/impl/exp`: 本 slice の
  `autx-06-mixed-source-run-exp.jsonl` に error なし
  （既存 `issue-147-sccache.jsonl` の pre-existing error・
  index README 未同期 warning は本 slice 対象外）。
- `doc_status.js lint --type test-spec`: `verifies` gate
  （TEST-SPEC-DOC-GATE-001）は `docs/designs/
  execution-plan-design.md` へ修正後に解消。残件は
  index-missing-entry / orphan-index / orphan-navigation の
  index 未同期 warning のみ（docs index 再生成禁止のため
  N-10 として Deferred — `autx-06-workflow-coordinator`
  chain と同じ統合時処理）。
