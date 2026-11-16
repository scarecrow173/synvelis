# sv114-gui-option-b-recorder-wire 検証証拠 — Issue #114 synvelis-app recorder session owner + syn.recorder.* headless wire

- 対象 Issue: <https://github.com/scarecrow173/synvelis/issues/114>
  （SV-GUI-05。本 slice は Option B = `synvelis-app` 層の
  Recorder command/UI wire の headless 実装のみ。Issue は
  実 device 列挙・実 GUI-L2・EXT-L3・残 AC の gate が残る
  ため open 継続。PR は `Refs #114`）。
- ブランチ: `feat/114-gui-option-b-wire`（base: origin/main
  `0b57299`）
- 変更: `crates/synvelis-app/src/{recording,app,service,
  ui_ops}.rs` に `RecorderOwner`（`RecorderSlot` 互換
  re-export）・`syn.recorder.*` 7 op・capabilities/close_veto/
  recording_info の実態一致を実装。`crates/synvelis-app/
  tests/recorder_wire.rs` に test-local fake `Device`/
  `InputStream` + headless E2E 12 test を新設。
  `tests/close_veto.rs` は `clippy::large_stack_arrays` 是正
  （`vec![...].into_boxed_slice()`、挙動不変）。
- 実行環境: Windows worktree（`G:\orca\workspaces\synvelis\
  sv114-gui-option-b`）、rustc/cargo 1.99.1（mise 管理、
  `mise.toml` の通常 toolchain。`mise.ci.toml` pin の
  1.99.0 ではない点に注意 — ubuntu-latest/1.99.0 条件は
  merge 後の対象 SHA の CI 証拠に委譲される）。
  `MISE_ENV=ci` は強制していない（POLICIES.md §5）。

## 変更の要点

- `RecorderSlot` stub（`active()`/`status()` が定数）を
  `RecorderOwner`（`recording.rs`）に置換。`RecorderSlot` は
  `pub use crate::recording::RecorderOwner as RecorderSlot;`
  の互換 re-export。
- `syn.recorder.*` ops: `start`/`pump`/`stop`/`commit`/
  `discard`/`status`/`mark_take_verified` を `OPS` テーブル
  と dispatch（`dispatch_recorder` helper）に登録。
  `syn.recording.*`（UI-C5 control tx）・`syn.monitoring.*`
  とは独立 namespace。
- device 注入 seam: `SynvelisApp::set_recorder_devices`。
  device は `project.new`/`project.open` で保持、active
  session のみ reset。
- lifecycle 順序: preflight → `InputSession::open`/`add`
  （device 失敗は project mutation 前に完了）→
  `OpenSession`+stream 登録 → `StagingArea`/`StreamWriter`
  → stream start → pump → stop/finalize → commit/discard。
  post-mutation 失敗は best-effort `CloseSession`。
  `commit`/`discard` は persisted session state を先に読み、
  `Open` のときだけ `CloseSession` を適用する（`Closed`/
  `Finalized` では `commit_session` の `commit_status`
  冪等経路に委ね、commit 失敗後の retry が wedge しない
  ことを保証 — review で検出・是正）。
- review 是正（2 reviewer + validator + judge の結果、
  CONFIRMED P0/P1/P2 のみ修正）:
  - `stop` は部分 finalize 失敗からの retry で
    `WriterError::AlreadyFinalized`（finalized.json 公開済み
    = 停止契約の完了条件）を成功として扱う — さもないと
    先行 stream の再 finalize が永久に wedge する。
  - `commit` は outcome が `Applied`/`AlreadyApplied` の
    場合のみ runtime session を破棄し、`Failed`/
    `IntegrityConflict`/`NothingCommitted` では `stopped`
    session を保持する（wire に session 除去 op が無く、
    drop すると project session が孤立する）。
    `commit_session` が `CommitRecording` 適用後に `Err` を
    返した場合も revision 前進を検出して
    `notify_committed_change` を落とさない（retry の
    `AlreadyApplied` は revision を進めず再通知不能な
    ため）。
  - `syn.project.undo`/`syn.project.redo` は recorder
    session 稼働中（`recording`/`stopped`）に
    `invalid_params` で拒否 — undo は snapshot 復元で
    per-op validation を通らず、recorder の session/
    stream entity を巻き戻して commit を不能にし得る
    ため。
  - `targets[].channels` に wire 上限 256 を設定 —
    `build_tap_resources` が `max_block_frames ×
    channel_map.len()` の callback queue と PCM pool を
    eager 確保するため、上限なしでは巨大確保で process
    が abort し得る。
  - `loop_start < transport_start` の loop 宣言を
    `invalid_params` で拒否 — session frame が負になり
    `GateScheduler` が構成できず runtime fault になる
    ため（`syn.monitoring.session` と同じ境界規則）。
  - `count_in`/`pre_roll` params を削除 — recorder の
    gate/`RecordingRequest` に対応 field が無く、
    parse されるだけで消費・検証されない dead wire
    surface だったため（monitoring session 側は別 op
    として存続）。
- 実態一致: `syn.app.capabilities` の `recorder`（device
  0 で `unavailable`+reason）・`syn.window.close_guard`
  の `recorder_active`・`syn.recording.info` の
  `status`/`active`/`transaction_open` が同一 owner を読む。

## 実行した検証（実測のみ記録）

| コマンド | OS / toolchain | exit | 結果 |
| --- | --- | --- | --- |
| `mise run fmt-check` | Windows / rustc 1.99.1 | 0 | Pass |
| `mise run check synvelis-app` | Windows / rustc 1.99.1 | 0 | Pass |
| `mise run lint synvelis-app` | Windows / rustc 1.99.1 | 0 | Pass（`-D warnings`。`large_stack_arrays`/`too_many_lines`/`doc_markdown`/`match_same_arms`/`redundant_closure_for_method_calls`/`cast_possible_truncation` 等を構造修正で解消。`#[allow]` は `tests/recorder_wire.rs` full lifecycle test の `too_many_lines` のみに狭く使用） |
| `mise run test synvelis-app` | Windows / rustc 1.99.1 | 0 | Pass — 全 test binary 0 failed。`tests/recorder_wire.rs` 12 test 全て pass: `recorder_wire_ops_registered_and_idle_without_device`・`recorder_wire_start_without_device_is_typed_error`・`recorder_wire_full_lifecycle_headless`・`recorder_wire_discard_closes_without_commit`・`recorder_wire_device_fault_latches_and_marks`・`recorder_wire_mark_take_verified`・`recorder_wire_project_new_resets_active_session`・`recorder_wire_commit_retries_after_staging_io_failure`・`recorder_wire_undo_redo_rejected_while_active`・`recorder_wire_start_rejects_oversized_target_channels`・`recorder_wire_start_rejects_loop_before_transport`・`recorder_wire_stop_retries_after_partial_finalize_failure` |
| `mise run build synvelis-app` | Windows / rustc 1.99.1 | 0 | Pass |
| `mise run check-app-native` | Windows / rustc 1.99.1 | 0 | Pass（`native` feature 面の型検査） |
| `mise run lint-app-native` | Windows / rustc 1.99.1 | 0 | Pass（`native` feature 面 lint） |
| `mise run check-app-gpui` | Windows / rustc 1.99.1 | 0 | Pass（`gpui-test` feature 面の型検査） |
| `mise run lint-app-gpui` | Windows / rustc 1.99.1 | 0 | Pass（`gpui-test` feature 面 lint） |
| `mise run check-headless-deps` | Windows / node | 0 | Pass（headless crate へ GUI/GPU/WebView 依存の混入なし、12 crates 確認） |
| `mise run check-ruviform-rev` | Windows / node | 0 | Pass（初回は stale vendor `ui vendor REV 20a24a66 != lockfile cbcc8343` で fail → `mise run sync-ui-deps` で vendor を lockfile rev へ再同期後 pass。`ui/vendor/ruviform` は gitignore された生成物で tracked diff なし） |
| `mise exec -- prek run --all-files` | Windows / prek | 0 | Pass（trim/eof/yaml/toml/mixed-line-ending/bom/private-key/actionlint 全て Passed） |
| `git diff` 禁止パス確認 | — | 0 | Pass — `Cargo.toml`/`Cargo.lock`/`mise.toml`/`mise.ci.toml`/`.github/**`/`docs/**/README.md`/`crates/synvelis-recording/src/**`/`ui/`/`POLICIES.md`/`AGENTS.md`/`tools/*.mjs` に差分なし |

補助証拠（canonical = mise）: `cargo check --locked
--all-targets -p synvelis-app` exit 0。

## headless E2E case 対応（`tests/recorder_wire.rs`）

| test | 検証内容 |
| --- | --- |
| `recorder_wire_ops_registered_and_idle_without_device` | `syn.recorder.*` 7 op が `OPS` に登録され、device 未注入で status が `unavailable`+reason+device 数 0+idle を返す |
| `recorder_wire_start_without_device_is_typed_error` | device 未注入の `syn.recorder.start` が typed error で reject され、project に recording session が作られない |
| `recorder_wire_full_lifecycle_headless` | fake `Device` 注入で `start`→`pump`→`stop`→`commit` が成功し、`syn.recording.info` の persisted entities（session/stream/take）と `recorder.status`/`active`/`transaction_open` が遷移、close veto が session active 中に `recorder_active` を報告、commit 済み take が `Verified` に遷移することを pin |
| `recorder_wire_discard_closes_without_commit` | recording 中の `discard` が stream 停止 + `CloseSession` 適用 + commit なしで active state を drop（staging file 保持） |
| `recorder_wire_device_fault_latches_and_marks` | fake device の error callback 発火で fault latch が立ち status/fault 投影に反映され、capture-fault marker により commit outcome が `nothing-committed`（全 stream recoverable）となり runtime session が `stopped`/`active` で保持され `recorder_active` veto が残り、`discard` で畳めることを pin |
| `recorder_wire_mark_take_verified` | `Open` session 上の `Raw` take への `mark_take_verified` が `RecordingOp::MarkTakeVerified` を適用し take 状態を返す（committed session は `Finalized` で domain が op を拒否するため正系は未 commit take）。不明 take id・`Finalized` session の take は typed error |
| `recorder_wire_project_new_resets_active_session` | active session 中の `project.new` で active session が reset され device が保持される |
| `recorder_wire_commit_retries_after_staging_io_failure` | 破損 `commit-manifest.json` で `commit_session` を一度 `service_error` 失敗させた後、除去して retry すると `CloseSession` 再適用ではなく `commit_session` 冪等経路で `applied` commit が完了し session が `finalized` になる（commit retry wedge の regression pin） |
| `recorder_wire_undo_redo_rejected_while_active` | recorder session `recording`/`stopped` 中の `syn.project.undo`/`redo` が `invalid_params` で拒否され、commit 確定後に undo が再び使えることを pin（snapshot 復元が recorder entity を巻き戻す desync の防止） |
| `recorder_wire_start_rejects_oversized_target_channels` | `targets[].channels` 257（wire 上限 256 超過）の `syn.recorder.start` が `invalid_params` で拒否され project/runtime session が作られないことを pin（eager 確保 abort の防止） |
| `recorder_wire_start_rejects_loop_before_transport` | `loop_start < transport_start` の loop 宣言が `invalid_params` で拒否されることを pin（負 session frame → runtime fault の防止） |
| `recorder_wire_stop_retries_after_partial_finalize_failure` | 2 armed stream で stream-0001 の journal を一時破損して `stop` を失敗させ journal を復元して retry すると、先行 stream の `AlreadyFinalized` を成功として扱い `finalized: 2` で `stopped` に到達し commit が `applied` で完走することを pin（partial finalize wedge の regression pin） |

## 未検証・委譲項目（NotRun / Unsupported / Deferred）

- **実 device 列挙/open**（`DeviceEnumerator` 経由の実 backend
  接続）・device 選択 UX: **Deferred**（後続 slice。本 slice は
  `set_recorder_devices` 注入 seam まで）。
- **実 GUI-L2**（実 GPUI Window での録音 UI 操作）: **NotRun**
  （Windows GUI-L2 gate、別 lane）。
- **EXT-L3**（実 OS Scenario Runner・rf#629 系仮想 endpoint・
  実 audio hardware）: **NotRun**（`scarecrow173/ruviform#629`
  OPEN・環境未構築）。
- **`syn.monitoring.*` authored 宣言との統合**（arm/punch/
  monitor UX、RCD-03/`scarecrow173/synvelis#117` 系）: **Deferred**。
- **`syn.recorder.*` の GUI driver（`ui/`）からの実呼び出し**:
  **Deferred**（Option B GUI 側 slice で wire 契約を確定）。
- **Issue #114 の残り AC**（SVT-01〜09 実走・Windows GUI-L2/
  EXT-L3・SV-OS case・cross-consumer gate・AC2 CI 証拠照合）:
  **NotRun**（本 slice では閉じない。merge 後 CI/別 gate に
  委譲）。
- **`mise run test-msrv`**: **NotRun**（ユーザー指定により
  MSRV lane なし）。
- **`*-full`・3OS 横断・全 Golden/長時間回帰**: **NotRun**
  （Daily Full CI・手動 Full CI 委譲）。
- **docs index（`docs/**/README.md`）**: **Deferred**
  （Coordinator が統合時に `doc-maintenance` で再生成。
  Worker は index を編集・再生成・`index apply` 実行しない）。

## 正直記録の原則

- 本 slice の Pass は **synvelis-app 層の headless
  fake-device wire lane のみ**を意味し、実 device 列挙/
  capture・実 WASAPI callback・実 GUI・product acceptance を
  一切含意しない。
- `syn.recorder.*` op 面の検証は headless wire の検証であり、
  実 OS Scenario Runner・実 GUI-L2・rf#629 系 EXT-L3 の証拠に
  転用しない。
- headless fake-device の Pass を実 OS/実 GUI の AC 達成に
  昇格しない（POLICIES.md・test-spec Rule）。
