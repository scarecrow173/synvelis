# sv#646 verification evidence — ALIB-05 residual 2（pending_commit intent 永続化 + commit replay — workspace restore/reinsert API）

Slice: `synvelis-workspace` に `LibraryJobRestore` enum +
`LibraryJobLedger::restore_pending_commit` を追加し、
`crates/synvelis-app/src/library_jobs.rs` の `restore_library_jobs`
が `pending_commit` wire の durable `ImportManifest`
（`staging_dir(<id>)/import-manifest.json`）から `ImportIntent` を
再構成して ledger に live 再注入する経路を実装した。次の
`syn.library.*` op 先頭の `reconcile_jobs` が `import_verified`
の終端 commit を live controller に replay する。
HEADLESS ONLY — 実 GPUI mount（#114）・実 provider/network/
credential（#91）・実データ（#86）は対象外。

## Environment

| Field | Value |
| --- | --- |
| Synvelis commit | `c6701363`（branch `scarecrow173/sv646-pending-commit-replay` base = PR #645 merge）+ this change set |
| OS | Windows (win32), NTFS |
| Toolchain | mise managed（`mise.toml` pin; CI pin は `mise.ci.toml`） |
| Provider / Import ports | scripted in-memory `FakeProvider`（`tests/common/library.rs`）・temp-dir `ImportStore` + `LocalBlobDir` + `StagedBlobPublisher`/`GatePublisher`。network/credential/実データ不使用 |
| Restart 証明方式 | test 内で `SynvelisApp`/`ImportStore` を再構築（`reopen_import_app`）する headless 計測。実 process kill・実デスクトップ再起動は NotRun（#114 gate 領域） |

## AC 対応 — 受入条件ごとの結果

### AC1 — workspace restore/reinsert API: Pass（headless）

`LibraryJobLedger::restore_pending_commit(key, kind, view,
import_id, intent, progress)` → `LibraryJobRestore{Reinserted{token,
epoch} | InFlight}`。再注入 entry は live `park_commit` の残す状態
そのもの（`Running` + stage `commit-pending` + `pending_commit` +
`intent` + `runtime_job:None` + `terminal:None`）に建ち、
`reconcile_jobs` の filter（`pending_commit && terminal.is_none()`）
に拾われる。`crates/synvelis-workspace/src/library_ui.rs` の unit
test 4 件が契約を固定した:

| Test | 観測したこと |
| --- | --- |
| `restore_pending_commit_projects_commit_owed_state` | wire 投影が `running`/`commit-pending`/`pending_commit:true`/`terminal:None`/`runtime_job:None` — 「worker は消えたが commit を owe」が正直に読める。同一 key への二度目の呼び出しは `InFlight`（`begin` と同じ dedup — 同一 `ImportId` を二重 job にしない）。epoch/token は新規 mint |
| `restore_pending_commit_cancel_reaches_the_entry` | `request_cancel` が再注入 entry に届き `cancel_requested`/`cancel-requested` state に遷移 — `import.cancel` 経路が live job と同じく機能する |
| `reconcile_replays_a_reinserted_pending_commit` | scratch controller を `Committed` まで駆動した durable manifest + 再注入 entry → `reconcile_jobs` が `import_verified` を live controller に replay → `committed-live` terminal・live revision 前進・`applied_imports` 登録（`reused:false`） |
| `reconcile_reinserted_without_ports_lands_unavailable` | import ports 未 bound でも filter に拾われ `unavailable` typed terminal — entry が消えたり fake 状態に残らない |

### AC2 — intent 永続化 + commit replay: Pass（headless）

`crates/synvelis-app/tests/sv646_pending_commit_replay.rs`（5 test）
と更新した `sv643_jobs_ledger.rs` の replay test が、save →
store/session 再構築 → `project_open` → 最初の `syn.library.*`
read で reconcile 発火という restart 立ち合いの全経路を観測:

| Test | 観測したこと |
| --- | --- |
| `pending_commit_replays_to_committed_live_after_reopen` | `import.submit` → publish 中 gate → `committed` manifest + `park_commit` → save → reopen → `job()` read で `committed-live` terminal（`pending_commit:false` へ解消）・live revision 前進・`syn.library.imports` が `stage:"committed"` + `applied:true` |
| `pending_commit_stale_revision_lands_typed_terminal` | park 後に `syn.project.set_ppqn`（reconcile を踏まない mutation）で live revision を 0→1 → save → reopen → replay が `stale-revision{expected:0,actual:1}` terminal に着地。project は revision 1 のまま・`applied:false` — stale commit は rebase されず落ちない |
| `pending_commit_without_manifest_degrades_recoverable` | save 後に manifest dir を削除 → reopen → `recoverable{reason:"interrupted",stage:"commit-pending"}` — intent を証明できない entry は sv643 degrade を維持し `imports` に行を捏造しない |
| `pending_commit_unreadable_manifest_degrades_recoverable` | manifest を破損（`{not-json`）→ 同じ `recoverable` degrade — torn write は fail-closed |
| `pending_commit_cancel_requested_still_replays` | sidecar の `cancel_requested:true`/`cancel-requested` を復元 → `request_cancel` replay で flag が残り、それでも owed commit は `committed-live` に着地（live parked commit と同じ semantics — reconcile は cancel で filter しない） |
| `restart_replays_pending_commit_to_committed_live`（sv643 file 更新） | sv643 で `recoverable` を固定していた parked-commit restart が、manifest が intent を証明する経路では `committed-live` + `applied:true` に replay することを確認（`imports` 行も相関） |

### AC3 — 対象 lane pass: Pass

下記「Commands run」の全 lane が exit 0（`fmt-check` は既知の
環境制約で補助コマンドを canonical の制約記録付きで記録）。

### 非回帰: Pass

`sv643_jobs_ledger` 10/10（pending_commit test は新 semantics に
更新して pass）・`mise run test synvelis-app` 全 test binary・
`mise run test synvelis-workspace` 全 test binary・`test-rt`・
`check-headless-deps`・uitest/native lane を非回帰確認。
sidecar format は `synvelis-library-jobs/1` のまま変更なし —
旧 format sidecar はそのまま読め、fail-closed 契約（corrupt/
unknown-format → `Store` error）は維持。

## Commands run (canonical = mise)

| Command | Exit | Result |
| --- | --- | --- |
| `mise run fmt-check` | 1 | Fail — `cargo fmt --all` が Windows コマンド長制限 `os error 206` で環境不能（既知制約）。補助: `mise exec -- cargo fmt --package synvelis-workspace --package synvelis-app -- --check` = exit 0 pass（`cargo fmt` 適用後） |
| `mise run check synvelis-workspace` | 0 | pass |
| `mise run lint synvelis-workspace` | 0 | pass（初回 2 error — `too_many_lines`・`manual_assert_eq` — を修正して再検証済み） |
| `mise run test synvelis-workspace` | 0 | pass — lib 29/29（新規 unit test 4 件含む）+ 全 test binary |
| `mise run check synvelis-app` | 0 | pass |
| `mise run lint synvelis-app` | 0 | pass |
| `mise run test synvelis-app` | 0 | pass — `sv646_pending_commit_replay` 5/5・`sv643_jobs_ledger` 10/10 含む全 test binary |
| `mise run check synvelis-project` | — | NotRun — `crates/synvelis-project/` 未変更 |
| `mise run lint synvelis-project` | — | NotRun — 同上 |
| `mise run test synvelis-project` | — | NotRun — 同上 |
| `mise run test-rt` | 0 | pass |
| `mise run check-headless-deps` | 0 | pass（headless crate に GUI 依存なし） |
| `mise run check-app-uitest` | 0 | pass |
| `mise run lint-app-uitest` | 0 | pass |
| `mise run test-app-uitest` | 0 | pass（restore → reconcile replay の wire seam lane） |
| `mise run check-app-native` | 0 | pass |
| `mise run lint-app-native` | 0 | pass |
| `mise run build-app-native` | 0 | pass |
| `mise exec -- prek run --all-files` | 0 | pass |

補助証拠（canonical = mise の制約記録付き）:
`mise exec -- cargo check -p synvelis-workspace -p synvelis-app --tests`
・`mise exec -- cargo test -p synvelis-workspace --lib`・
`mise exec -- cargo test -p synvelis-app --test sv646_pending_commit_replay
--test sv643_jobs_ledger` — 開発中の反復検証として実走・pass。

## NotRun（gate 領域の明示）

- 実 OS GUI 受入（実 Window/実画面での job 進行・receipt 表示）—
  synvelis#114 GUI-L2 gate 領域。
- 実 provider/network/credential を使う import/preview —
  synvelis#90/#91 領域（本 slice の provider は scripted fake）。
- 実 process kill/実デスクトップ再起動 — restart は test 内の
  store/session 再構築で代替証明（本 AC で許容される headless
  計測）。
- `queued`/`running` で再起動を迎えた job の実行 resume —
  worker thread は再起動を越えられない（sv643 確立の
  typed-terminal 復元 semantics を維持）。
- 最後の save 以降に park された job の restore — sidecar が
  save 時点 snapshot である既知制限（durable manifest + 既存
  `import.retry`/`syn.library.imports` 導線が残る）。
