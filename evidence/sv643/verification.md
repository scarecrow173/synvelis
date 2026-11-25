# sv#643 verification evidence — ALIB-05 residual（syn.library.jobs 永続化/recovery・syn.library.imports・generic job.cancel）

Slice: `syn.library.jobs` ledger（`LibraryJobLedger` の wire
投影）を project bundle の sidecar `library-jobs.json` へ永続化し、
process 再起動（= test 内で store/session 再構築 — headless）後に
job の state・terminal receipt を `syn.library.jobs` 投影として
復元する。`inspect_imports` を `syn.library.imports` op として
job 系 op と同じ wire/ledger 経路に露出し、ruviform 汎用
`job.cancel` から `CallOutcome::Job` で起こした import/preview
job の協調 cancel token への到達を headless 計測した。
HEADLESS ONLY — 実 GPUI mount（#114）・実 provider/network/
credential（#91）・実データ（#86）は対象外。

## Environment

| Field | Value |
| --- | --- |
| Synvelis commit | `f8600b964335a0d688386a4f1369cc3bd79f96e4`（branch `scarecrow173/sv643-jobs-ledger-residual` base = PR #642 merge）+ this change set |
| OS | Windows (win32), NTFS |
| Toolchain | mise managed（`mise.toml` pin; CI pin は `mise.ci.toml`） |
| Provider / Import ports | scripted in-memory `FakeProvider`（`tests/common/library.rs`）・temp-dir `ImportStore` + `LocalBlobDir` + `StagedBlobPublisher`/`GatePublisher`。network/credential/実データ不使用 |
| Restart 証明方式 | test 内で `SynvelisApp`/`DirectoryBundleStore`/`ImportStore` を再構築（`reopen_import_app`）する headless 計測。実 process kill・実デスクトップ再起動は NotRun（#114 gate 領域） |

## AC 対応 — 受入条件ごとの結果

### AC1 — job ledger 永続化・recovery: Pass（headless）

`crates/synvelis-app/tests/sv643_jobs_ledger.rs` の 6 test が
永続化→復元の semantics を固定した（いずれも「save 後に
app/store を再構築して project_open する」restart 立ち合い）:

| Test | 観測したこと |
| --- | --- |
| `jobs_ledger_survives_project_save_and_reopen` | `done` job の `committed-live` receipt（import_id/asset_id）が save→reopen で verbatim に復元される。`library-jobs.json` が bundle root に存在 |
| `restart_restores_inflight_import_as_recoverable` | publish 中に restart した import job は `done`/`recoverable`/`interrupted`/`materializing` — durable manifest（`verified`）が残るので再実行可能 receipt。`syn.library.imports` 行が manifest stage `verified`/`applied:false`/restored job を相関表示 |
| `restart_restores_pending_commit_as_recoverable` | `park_commit` 済み（manifest `committed`・registry 未適用）で restart した job は `recoverable`/`commit-pending` で復元され、commit replay は走らない（`applied:false` のまま — owed commit は `import.retry` 導線） |
| `restart_restores_cancel_requested_as_cancelled` | restart 前に記録された `import.cancel` は `cancelled{resumable:true,stage:"materializing"}` terminal で復元（`cancel_requested` flag も復元） |
| `restart_restores_queued_preview_as_failed` | durable 状態を持たない preview job は `failed`/`interrupted` で復元 — resume を装わない |
| `project_new_overwrites_a_stale_sidecar` / `foreign_project_sidecar_is_ignored_on_open` | bundle dir 再利用時の stale sidecar は `project_new` で新 project id+空 ledger に上書き、foreign `project_id` の sidecar は open で skip（error にしない） |

補助（store 層）: `crates/synvelis-project/tests/store_publish.rs`
の `sidecars_roundtrip_replace_and_reopen` /
`sidecar_names_reject_paths_and_store_layout` — Replace publish・
reopen 後の bytes 一致・`InMemoryStore` parity・name admission
（区切り/`..`/`HEAD`/`snapshots`/`HEAD.lock*` 拒否）。

### AC2 — inspect_imports op 化: Pass（headless）

`syn.library.imports` を `OPS` 登録 + `dispatch_surface_ops` arm +
`op_library_imports` として `syn.library.jobs`/`action` と同じ
wire 経路（`sys.capabilities` で観測可能・`with_library` 内で
`reconcile_jobs` → `inspect_imports` → ledger 相関）に露出。
`library_imports_reports_manifest_and_ledger_job` が観測:

- manifest 行 `{import_id, stage:"committed", applied:true,
  byte_len, ...}` + ledger 相関 `job_key`（`import:<tagged>`）・
  `job`（`LibraryJobWire` verbatim、`terminal.kind:"committed-live"`）
- import services 未 bound は `unavailable`（空 inspection を
  「import 無し」に見せない）

### AC3 — generic `job.cancel` wire 到達: Pass（headless）

`UiRuntime::dispatch_op("t","job.cancel",{job:<id>})` —
`CallOutcome::Job` 経路で起こした job に対して:

- `generic_job_cancel_reaches_import_job`: publish 中
  （manifest `verified`）に `job.cancel` → `{job:<id>,
  cancelling:true}` ack → `JobState::Cancelled` event →
  ledger `cancelled{resumable:true,stage:"materializing"}`。
  wire entry の `runtime_job` field が numeric id を投影
  （外部が id を引ける相関）。
- `generic_job_cancel_reaches_preview_job`: gated read 中に
  `job.cancel` → gate 解放後 `RuntimeJobDrive::is_cancelled` が
  `JobContext` token を観測 → `JobState::Cancelled` event →
  ledger `cancelled{resumable:false}`。
- 区別の記録: `op_library_action` 直接経路（`DirectJobDrive`・
  runtime job id なし）は `job.cancel` の対象にならない —
  cancel 到達は `CallOutcome::Job` 経路に限る。

## Commands run (canonical = mise)

| Command | Exit | Result |
| --- | --- | --- |
| `mise run fmt-check` | 1 | Fail — `cargo fmt --all` が Windows コマンド長制限 `os error 206` で環境不能（既知制約）。補助: `mise exec -- cargo fmt --package synvelis-app --package synvelis-project -- --check` = exit 0 pass |
| `mise run check synvelis-app` | 0 | pass |
| `mise run lint synvelis-app` | 0 | pass（warning 0 — `let_and_return` 1 件を修正して再検証済み） |
| `mise run test synvelis-app` | 0 | pass — `sv643_jobs_ledger` 10/10 含む全 test binary 成功（`library_browser` 24 test 非回帰） |
| `mise run check synvelis-project` | 0 | pass |
| `mise run lint synvelis-project` | 0 | pass |
| `mise run test synvelis-project` | 0 | pass — `store_publish` 18/18（sidecar 2 test 含む） |
| `mise run check synvelis-workspace` | 0 | pass（lease 外・下流非回帰） |
| `mise run lint synvelis-workspace` | 0 | pass |
| `mise run test synvelis-workspace` | 0 | pass |
| `mise run test-rt` | 0 | pass（rt_ 契約 lane 全非回帰） |
| `mise run check-headless-deps` | 0 | pass（headless crate に GUI 依存なし） |
| `mise run check-app-uitest` | 0 | pass |
| `mise run lint-app-uitest` | 0 | pass |
| `mise run test-app-uitest` | 0 | pass（`job.cancel` wire 計測が通る runtime/uitest seam lane） |
| `mise run check-app-native` | 0 | pass |
| `mise run lint-app-native` | 0 | pass |
| `mise run build-app-native` | 0 | pass |
| `mise exec -- prek run --all-files` | 0 | pass |

## NotRun（gate 領域の明示）

- 実 OS GUI 受入（実 Window/実画面での job 進行表示・cancel）—
  synvelis#114 GUI-L2 gate 領域。
- 実 provider/network/credential を使う import/preview —
  synvelis#90/#91 領域（本 slice の provider は scripted fake）。
- 実 process kill/実デスクトップ再起動 — restart は test 内の
  store/session 再構築で代替証明（本 AC で許容される headless
  計測）。
- `pending_commit` の commit replay 完全復元 — intent が wire に
  載らないため本 slice は `recoverable` + `import.retry` で代替
  （実装記録「設計判断と却下案」参照）。
