# sv518 — production render job system 配線 evidence

Issue: `scarecrow173/synvelis#518` / branch `feature/518-job-system`
OS: Windows (mise managed toolchain)。cargo 直接実行は補助証拠、
canonical = mise。

## Lane 別 Pass/NotRun

| Lane | 判定 | 根拠 |
| --- | --- | --- |
| OS-L1 headless/fixture | **Pass** | 下記 mise 検証コマンド全 pass |
| GUI-L2 | **NotRun** | render UI surface は別 slice の責務（本 slice は dispatch adapter + host 配線まで） |
| EXT-L3 実 render/RT | **NotRun** | 実 render backend・実 RT 計測は別 gate。headless は fail-closed typed rejection のみ |
| 実 CLI/MCP/GUI E2E | **NotRun** | headless `JobSystem::headless()` wiring のみ — backend 未接続を正直に typed failure/rejection で返す |

## 実行した検証（全て exit 0）

| コマンド | 結果 |
| --- | --- |
| `mise run fmt` + `mise run fmt-check` | Pass |
| `mise run check-headless-deps` | Pass（12 headless crates に GUI framework deps 無し — synvelis-workspace 追加後も維持） |
| `mise run check-ruviform-rev` | Pass（resolved 84a68f25、lockfile-primary） |
| `mise run check synvelis-render` | Pass |
| `mise run lint synvelis-render` | Pass |
| `mise run test synvelis-render` | Pass（rcp03_render_policy 30 件 — async pin 9 件含む、l2_e2e 17、recipe 11、artifact 20、gate 7、sf03 2、plugin-state 5、e2e 5、doc test） |
| `mise run check synvelis-project` | Pass |
| `mise run lint synvelis-project` | Pass |
| `mise run test synvelis-project` | Pass（全 suite — inline render_policy round-trip 含む、schema v15 pin 3 件更新後） |
| `mise run check synvelis-workspace` | Pass |
| `mise run lint synvelis-workspace` | Pass |
| `mise run test synvelis-workspace` | Pass（全 suite） |
| `mise run check synvelis-cli` | Pass |
| `mise run lint synvelis-cli` | Pass |
| `mise run test synvelis-cli` | Pass |
| `mise run check mcp-poc` | Pass |
| `mise run lint mcp-poc` | Pass |
| `mise run test mcp-poc` | Pass |
| `mise exec -- prek run --all-files` | Pass |

補助証拠（canonical = mise）: `cargo test -p synvelis-render`
全 suite pass、`cargo test -p synvelis-project --locked` 全
suite pass、`cargo test -p synvelis-workspace` 全 suite pass
（mise test 内でも同結果を確認）。

## 新規 test pin

- `crates/synvelis-render/tests/rcp03_render_policy.rs` async
  regression 9 件:
  - `async_pol01_mixed_triggers_coalesce_into_one_job` —
    mixed trigger coalescing で 1 job / 1 artifact / 重複 work 無し。
  - `async_pol01_duplicate_intent_submit_is_idempotent` —
    duplicate intent が coalesce され typed accepted。
  - `async_pol06_concurrency_quota_pends_then_resumes` —
    `cpu_only` grant（max_concurrent=1）で B が pend →
    A terminal 後に resume。
  - `async_pol06_grant_update_wakes_gate_pending_job` —
    gate pending job が grant update で起床。
  - `async_pol06_retryable_failure_retries_once` —
    retryable failure が typed retry で 1 度だけ再試行。
  - `async_pol06_crash_requires_explicit_resume` —
    crash job は自動再開せず明示 `resume` のみ。
  - `async_pol06_cancel_queued_job_is_typed` —
    queued job の cancel が typed receipt。
  - `async_scheduler_bounded_queue_reports_queue_full` —
    bounded queue 超過が `SchedulerError::QueueFull`。
  - `async_headless_scheduler_is_fail_closed` —
    settings 無しで typed `UnknownDependency` rejection、
    render 成功を捏造しない。
- `crates/synvelis-project/src/project.rs` inline
  `#[cfg(test)]`: `render_policy` absent→None / round-trip。

## 範囲拡張の記録（coordinator 承認済み）

- `crates/synvelis-project/tests/{cb01_variant,md_a,tim_02_smf}.rs`:
  `SCHEMA_VERSION` pin `14`→`15` の 1 行更新のみ
  （msg_3e7143e90682 / msg_77da6b863ccb 承認。
  テスト名・snapshot provenance 不変）。sv#511 merge で
  tests/ lease は解放済み。
- `crates/synvelis-cli/Cargo.toml` / `crates/mcp-poc/Cargo.toml`:
  `synvelis-workspace` path dependency 追加（host wiring に必要。
  `check-headless-deps` で GUI 混入無しを確認済み）。
- `Cargo.lock`: 上記 dependency 追加に伴う 2 エントリ。

## 未検証・残件

- 実 `RenderExecutor` / settings / demand mapping の production
  実装と `JobSystem` への注入（別 slice — headless は
  fail-closed ports のみ）。
- `synvelis.render.verify` / `attach` / `publish` / `remote`
  の workspace adapter 実装 — typed `Unavailable` を返すのみ
  （別 gate、成功を捏造しない）。
- docs index（`docs/**/README.md`、`docs/impl/*/README.md`）
  の再生成は Coordinator 統合時に実施。

## Post-review fixes (two-reviewer protocol, 2026-11-17)

Reviewer A (cooperative) + Reviewer B (adversarial) candidates were
validated against code; confirmed issues fixed in this diff:

- Share events in async mode no longer drive the fill job to
  quiescence inside the command handler — one `drive_step` quantum
  per share request; still-running jobs report `Blocked` with the
  job named for retry (`orchestrator.rs` `inspect_share`).
- `submit` rejects malformed selections typed
  (`JobFailureReason::InvalidSelection`): empty demands and
  inverted/empty `FrameInterval`s that wire decoding can produce —
  closes a cost-model under-estimate and vacuous-publish path.
- Adapter outcome mapping corrected: `ShareGateVerdict::Blocked`
  with no pending jobs maps to `Failed` (determined negative),
  in-flight pending maps to `Pending`, allowed verdicts to
  `Dispatched`; coalesced submits report the job's actual state
  instead of always claiming "in-flight" (`render_dispatch.rs`).
- Scheduler: `resume` on non-crashed jobs no longer performs a
  synchronous quantum inside the command handler; `shutdown` uses a
  blocking send so it cannot be silently dropped on a full queue;
  `JobView` exposes typed `failure_reason_typed`.
- Orchestrator: tick advances on async-path mutations (submit,
  drive_step, cancel, retry, resume_crashed); per-job receipt seq
  counters replace the O(R) scan per receipt; self-transitions no
  longer emit spurious `Transition` receipts.

Residual non-blocking notes (accepted, documented): synchronous
request-reply `call()` bounds command latency to one demand render
by design (cancel/retry land at demand boundaries; mid-render token
visibility deferred to executor-side work); worker panic resilience,
unbounded receipt/job retention, and preview-route side effects are
tracked as follow-ups; `render_policy` persistence is round-trip
only until declared-policy consumption lands.

Verification after fixes: `cargo check`/`cargo test` for
`synvelis-render` + `synvelis-workspace` all green (one parallel-load
flake in `async_pol06_concurrency_quota_pends_then_resumes`
reproduced green in isolation and on full-file rerun).
