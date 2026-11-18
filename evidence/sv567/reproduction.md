# sv567 reproduction — quota pend receipt not observable (修正前再現記録)

- 対象 Issue: `synvelis#567`（OPEN 維持）
- 再現 base: `b73018bd`（`origin/main`、ブランチ
  `bugfix/567-quota-pend-receipt`）
- 実行環境: Windows (Git Bash)、mise 管理 toolchain、
  `mise exec -- cargo test`（補助証拠。canonical 検証は
  `mise run` — `verification.md` 参照）
- 日時: 本 slice 作業時点

## CI 失敗 signature（Coordinator 確認済み・本 slice で再確認）

`verify (windows-latest)` → `mise run test-full` →
`rcp03_render_policy` スイートで
`async_pol06_concurrency_quota_pends_then_resumes` のみ失敗:

```
test async_pol06_concurrency_quota_pends_then_resumes ... FAILED
assertion failed: quota pend stays receipt-visible under async
crates/synvelis-render/tests/rcp03_render_policy.rs:2154
```

- CI run `44267395469` @ `b73018bd`、`44227068039` @ `bc90f0e0`
  の 2 連続 run で同一 assertion。`test result: FAILED.
  29 passed; 1 failed` — 同スイートの他 async_pol06 5 本と
  同期 twin `pol06_concurrency_quota_pends_then_resumes` は pass。

## 根因判定: test-design race（scheduler defect ではない）

### 機構（コードレベルで検証済み）

- `JobScheduler`（`scheduler.rs`）は `sync_channel` 上の
  `SchedulerMsg` を worker loop が「全 command drain →
  `pick_candidate` → `drive_step` 1 quantum」で処理する。
- `submit_intent` = `call` = `try_send`（enqueue・即時 return）
  + `reply_rx.recv`（worker の submit 評価完了を待つ）。
- 失敗テストは B の submit を `std::thread::spawn` した
  helper thread で実行していた。helper 内の `try_send` 実行
  タイミングはテスト内のどの同期点（`entered_rx`・
  `release_tx`・`b_rx`）からも制御されていない。
- `GateExecutor`（render call==0）は worker thread 内を park
  する。park 中は command drain が一切進まない。
- `orch.submit` の `pending_gate` は「`!terminal &&
  pending_reason.is_none()` の job 数 ≥
  `max_concurrent_jobs`(=1)」なら `ReceiptKind::Pending
  {QuotaExceeded}` を打つ。B が pend しないのは、B の submit
  評価時点で A が既に terminal の場合だけ。
- A は 2 demand 構成（2 件目は同一 recipe_key で
  `find_reusable` 経由・render call を消費しない）で、
  release 後の残り quantum は demand1 reuse + verify/publish
  の約 2-3 反復のみ。helper thread の `try_send` がこの
  微小 window を外れて A の terminal 遷移後の drain に滑り
  込むと、`pending_gate` は active=0 を見て B を即受理 →
  `Pending` receipt 不在 → :2154 で fail。

### receipt 喪失仮説の棄却

- `ReceiptKind::Pending` は `orch.submit` 内で reply 送信前に
  `push_receipt`（`self.receipts` への同期 append）される。
  `job_receipts` は key 一致の録音順 filter のみで、記録済み
  receipt が失われる経路は存在しない。
- したがって「B は正しく pend したのに receipt が失われた」
  経路は成立しない。失敗は「B の submit 評価が A terminal
  後にずれ込み、pend 判定自体が走らなかった」ことに限定される。

### 決定的再現（修正前コード）

helper thread の `submit_intent` 呼出前に
`std::thread::sleep(Duration::from_millis(200))` を挿入し、
CI の contended runner における helper 起動遅延を模擬:

```sh
mise exec -- cargo test -p synvelis-render --test rcp03_render_policy \
  async_pol06_concurrency_quota_pends_then_resumes
```

結果: **FAILED**（exit 101）— CI と同一 signature:

```
thread 'async_pol06_concurrency_quota_pends_then_resumes' panicked at
crates\synvelis-render\tests\rcp03_render_policy.rs:2155:5:
quota pend stays receipt-visible under async
test result: FAILED. 0 passed; 1 failed
```

200ms の遅延で helper の `try_send` が A の terminal 遷移後に
確実に滑り込み、`Pending{QuotaExceeded}` receipt が記録され
ないことを再現した（行番号は計装で +1 行シフトしたため
2155、CI の 2154 と一致）。再現後にパッチは revert 済み。

## 他 async テストの同型 race 検査

- `async_pol06_cancel_queued_job_is_typed`: B submit・cancel とも
  helper thread だが、A は 16 demand 構成で release 後の残り
  quantum が ~16 反復と広く、かつ `b_rx.recv_timeout(2s)` が
  release 前に helper へ ~2 秒の scheduling 余裕を与える構造。
  pend→cancel の順序が崩れるには helper の `try_send` が
  ~2 秒超遅延する必要があり、観測失敗もない。理論上の残存
  race はあるが本 slice の実測対象外として記録に留める
  （`verification.md` / IR 残件参照）。
- `async_pol06_grant_update_wakes_gate_pending_job`・
  `async_pol06_retryable_failure_retries_once`・
  `async_pol06_crash_requires_explicit_resume`・
  async_pol01 2 本: submit は test thread の blocking
  `submit_intent`/`handle_report` で逐次実行され、helper
  thread を使わない — 同型 race なし。
