---
id: "IMPL-034YrarNjUSNm0R7bsq4JU"
type: "impl"
status: "completed"
title: "Issue #114 synvelis-app recorder session owner + syn.recorder.* headless wire"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/114"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/src/recording.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/recorder_wire.rs"
      - type: "file"
        path: "docs/discovery/sv114-gui-option-b-recorder-wire.md"
      - type: "file"
        path: "docs/specs/sv114-gui-option-b-recorder-wire.md"
      - type: "file"
        path: "docs/designs/sv114-gui-option-b-recorder-wire.md"
      - type: "file"
        path: "docs/plans/sv114-gui-option-b-recorder-wire.md"
      - type: "file"
        path: "docs/tasks/sv114-gui-option-b-recorder-wire.md"
      - type: "file"
        path: "docs/test-specs/sv114-gui-option-b-recorder-wire.md"
      - type: "file"
        path: "docs/impl/ir/sv114-gui-option-b-recorder-wire.md"
      - type: "file"
        path: "docs/impl/exp/sv114-gui-option-b-recorder-wire.md"
      - type: "file"
        path: "docs/impl/exp/sv114-gui-option-b-recorder-wire-exp.jsonl"
      - type: "file"
        path: "evidence/sv114/"
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/close_veto.rs"
      - type: "file"
        path: "evidence/sv115/rcd-01-virtual-l1-oracle-ledger.md"
      - type: "file"
        path: "evidence/sv115/rcd-01-virtual-l1-oracle-case-matrix.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/sv114-gui-option-b-recorder-wire.md"
    - "docs/plans/sv114-gui-option-b-recorder-wire.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv114-gui-option-b-recorder-wire.md"
    - "docs/designs/sv114-gui-option-b-recorder-wire.md"
    - "docs/test-specs/sv114-gui-option-b-recorder-wire.md"
    - "docs/impl/exp/sv114-gui-option-b-recorder-wire.md"
    - "evidence/sv114/recorder-wire-option-b-headless.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-app/src/recording.rs"
    - "crates/synvelis-app/src/app.rs"
    - "crates/synvelis-app/src/service.rs"
    - "crates/synvelis-app/src/ui_ops.rs"
    - "crates/synvelis-app/tests/recorder_wire.rs"
    - "crates/synvelis-app/tests/close_veto.rs"
    - "mise.toml"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/sv114-gui-option-b-recorder-wire.md"
      - "docs/impl/exp/sv114-gui-option-b-recorder-wire-exp.jsonl"
    rejected: []
---

# Issue #114 synvelis-app recorder session owner + syn.recorder.* headless wire

## Summary

Issue #114（SV-GUI-05）の slice
`sv114-gui-option-b-recorder-wire`。`synvelis-app` の
`RecorderSlot` stub を `RecorderOwner`（`recording.rs`）に
置換し、preflight→`InputSession` open/add→stream 登録→
staging/writer→stream start→pump→stop→commit/discard→
`mark_take_verified` の recorder lifecycle を
`synvelis-recording` production API 上に実装した。
`syn.recorder.*` 7 op を `service.rs` の `OPS`/dispatch に
登録し、`ui_ops.rs` に op 実装と wire param parser を追加。
`capabilities`・`close_veto_reasons`・`recording_info` が
同一 owner を読む実態一致を実現し、headless E2E
（`tests/recorder_wire.rs`、test-local fake `Device`/
`InputStream`、`UiRuntime` dispatch）で lifecycle を検証
した。

## Context

- Issue: <https://github.com/scarecrow173/synvelis/issues/114>
- doc chain: `docs/discovery/sv114-gui-option-b-recorder-wire.md`
  / `docs/specs/sv114-gui-option-b-recorder-wire.md` /
  `docs/designs/sv114-gui-option-b-recorder-wire.md` /
  `docs/plans/sv114-gui-option-b-recorder-wire.md` /
  `docs/tasks/sv114-gui-option-b-recorder-wire.md` /
  `docs/test-specs/sv114-gui-option-b-recorder-wire.md`
- 前提: pinned ruviform `cbcc8343` は `Device`/
  `InputStream` port を提供済み、`synvelis-recording` の
  production API（`preflight`/`InputSession`/`StagingArea`/
  `StreamWriter`/`commit_session`/`MarkTakeVerified`）は
  充足済みで本 slice では変更しない。`synvelis-app` は両者に
  依存済みで dep 追加なし。作業は origin/main をベースに
  ブランチ `feat/114-gui-option-b-wire` で実施。
- 構造先例: `MonitorUxState`（app.rs の `monitor` field・
  monitoring.rs）が稼働 session を `SynvelisApp` 直下に
  保持する構造を踏襲した。

## Implementation

### Changes

- `crates/synvelis-app/src/recording.rs`: `RecorderOwner`
  を新設。host 注入 `Device`（`set_recorder_devices`）と
  session-scoped active state（`ActiveRecorder`: 確定 plan・
  `InputSession`・`StagingArea`・`StreamWriter` 列・fault
  latch・session metadata）を保持。`start`/`pump`/`stop`/
  `commit`/`discard`/`status`/`mark_take_verified`/
  `active`/`reset`/`recording_info` 連携を実装。
  `RecorderError` が domain 失敗を `ErrorBody` code に写像。
  `RecorderSlot` は `pub use crate::recording::RecorderOwner
  as RecorderSlot;` の互換 re-export。`RigInputs` で
  `start`/`rig_session` の引数を集約。
- `crates/synvelis-app/src/app.rs`: `RecorderSlot` を
  re-export に置換、`SynvelisApp.recorder` を
  `RecorderOwner` に更新、`set_recorder_devices` 追加、
  `project_new_inner`/`project_open_inner` で active
  session reset（device 保持）、`capabilities` の
  `"recorder"` 行と `close_veto_reasons` の
  `recorder_active` を owner 実態に一致させた。
- `crates/synvelis-app/src/service.rs`: `OPS` に
  `syn.recorder.start`/`pump`/`stop`/`commit`/`discard`/
  `status`/`mark_take_verified` を登録し、
  `name.starts_with("syn.recorder.")` の dispatch arm から
  `dispatch_recorder` helper へ family を委譲。
- `crates/synvelis-app/src/ui_ops.rs`: `op_recorder_*`
  7 op を実装し、wire param parser
  （`opt_clock_epoch_param`/`opt_u32_param`/
  `recorder_targets_param`/`recorder_streams_param`）で
  tagged `LaneId`・tick/range・u32・targets/streams/taps・
  arm/monitor を decode。
- `crates/synvelis-app/tests/recorder_wire.rs`: test-local
  fake `Device`/`InputStream`（deterministic PCM、
  `MockPcm`）と headless E2E 12 test を新設。
- `crates/synvelis-app/tests/close_veto.rs`:
  `clippy::large_stack_arrays` 是正（`vec![...].
  into_boxed_slice()`、挙動不変）。
- doc-chain 9 文書と `evidence/sv114/` を新規作成、
  `evidence/sv115/` 2 文書の GUI（Option B）pointer を
  更新。

### Decisions During Implementation

- **owner 配置は `recording.rs`**: `RecorderOwner` を
  `recording.rs` に置き、`RecorderSlot` は互換 re-export。
  `recording_info` 投影と同じ module に owner 状態を集約
  し、app.rs の stub 責務と recording domain 接続を分離
  した。`MonitorUxState` の「`SynvelisApp` 直下に session
  owner」構造を踏襲。
- **device open を project mutation 前に配置**: device
  失敗（`InputBindError`/`DeviceResult`）を project
  mutation 前に完了させ、録音失敗が project を dirty に
  しない順序を採用。`InputSession::open`/`add` が返す
  `TapWriterEnds` は device/stream handle であり、
  `project.recording.streams` への登録は mutation 段で
  行うため `build_payload` の事前登録前提と両立する。
- **post-mutation 失敗は best-effort `CloseSession`**:
  staging/writer 構築・stream start の失敗時は project
  recording session を閉じ、open session を残さない。
  `CloseSession` 自体の失敗は `transaction_open` 投影で
  観測可能とし、回復は discard/close 経路に委ねる。
- **`CloseSession` は状態確認付きで適用**:
  `RecordingOp::CloseSession` の validate は `Open`
  session のみを要求するため、`commit`/`discard` は
  persisted session state を先に読み、`Open` のときだけ
  適用する。`commit_session` 失敗後の retry では
  session が既に `Closed`/`Finalized` であり、
  `commit_session` 自身の `commit_status` 冪等経路
  （`AlreadyApplied`/`IntegrityConflict`）に委ねる —
  無条件再適用だと validate で永久に wedge する
  （review で検出・是正、regression test 追加）。
- **review で検出・是正した wedge/orphan/abort 経路**
  （2 reviewer + validator + judge、CONFIRMED のみ修正）:
  - `stop`: `finish_all` の per-stream 結果で
    `WriterError::AlreadyFinalized` を成功として扱う。
    `StreamWriter::finalize` は非冪等（`state==Finalized`
    または `finalized.json` 存在で `AlreadyFinalized`）
    なので、部分 finalize 失敗からの retry が永久に
    wedge していた。`checkpoint` は Finalized で冪等
    （last evidence を返す）ため wedge は finalize のみ。
  - `commit`: outcome が `Applied`/`AlreadyApplied`
    の場合のみ `active` を破棄し、`Failed`/
    `IntegrityConflict`/`NothingCommitted` では
    `stopped` session を保持する — `commit_session`
    は全 outcome を `Ok(CommitReport)` で返し、
    `RecordingOp` に session 除去 op が無いため、
    drop すると project session（`Closed` 残留）が
    孤立していた。また `commit_session` が
    `CommitRecording` 適用後に marker 書込み失敗で
    `Err` を返す経路では、retry の `AlreadyApplied`
    が revision を進めず `notify_committed_change`
    が永久に skip されるため、`Err` arm でも
    revision 前進を検出して通知する。
  - `syn.project.undo`/`redo`: `recorder.active()` 中は
    `invalid_params` で拒否。undo は snapshot 復元で
    per-op validation を通らず、recorder の
    `OpenSession`/`RegisterStream` apply を巻き戻して
    commit を永久に不能にし得た（`syn.monitoring.punch`
    の `require_no_session` と同じ境界規則）。
  - `targets[].channels`: wire 上限 256
    （`MAX_TARGET_CHANNELS`）。`build_tap_resources` が
    `max_block_frames × channel_map.len()` の callback
    queue（16 slot）と `chunk_frames × channels` の PCM
    pool を eager 確保するため、上限なしでは巨大確保で
    process が abort し得た（`vec!` alloc failure は
    catch 不能）。
  - `loop_start < transport_start`: `invalid_params`
    で拒否 — session frame が負になり `GateScheduler`
    が構成できず runtime `FAULT_SCHEDULE_UNDERRUN`
    になるため（`syn.monitoring.session` の
    `loop_start >= transport_start` 規則と同じ）。
  - `count_in`/`pre_roll` params を削除 — recorder の
    gate/`RecordingRequest` に対応 field が無く、
    parse されるだけで消費・検証されない dead wire
    surface だった（monitoring session 側は別 op と
    して存続）。
- **`syn.recorder.*` namespace**: `syn.recording.*`（UI-C5
  control-recording tx）・`syn.monitoring.*` と独立した
  新規 namespace。既存 op の wire 契約は不変。
- **typed error 写像**: `RecorderError` が
  `InputBindError`/`StagingError`/`CommitError`/preflight
  reject を `ErrorBody` code に写像し、defined rejection
  を panic/crash と混同しない。
- **clippy 是正は構造修正のみ**: `#[allow]` は原則不使用。
  `tests/recorder_wire.rs` の full lifecycle test のみ、
  逐次 lifecycle の長さが本質的なため狭い
  `#[allow(clippy::too_many_lines)]` を comment 付きで使用
  （test のみ）。`close_veto.rs` は `vec![...].
  into_boxed_slice()` で `large_stack_arrays` を是正。
- **docs index 非再生成**: `docs/**/README.md` は lease
  外（Coordinator が統合時に再生成）のため Worker は
  再生成・手編集していない。

## Related Experiments

### Adopted

- `docs/impl/exp/sv114-gui-option-b-recorder-wire.md` /
  `docs/impl/exp/sv114-gui-option-b-recorder-wire-exp.jsonl`:
  owner 配置・namespace・device 注入 seam・lifecycle 順序・
  fake device 実装・検証実測を記録した実験ログを採用した。

### Rejected

- `RecorderSlot` stub の直接拡張: stub の app 層 glue と
  session lifecycle（recording domain 接続）が混ざり、
  `recording.rs` の投影と分離できないため不採用。
- `syn.recording.*` への lifecycle 同居: UI-C5
  control-recording tx の wire 契約と混同するため不採用。
- `MonitorUxState` と同じ monitoring.rs への配置:
  recorder は recording domain であり monitoring ではない
  ため不採用。
- `StagingArea` root の project bundle 固定: test で
  tempdir を使えず bundle 破壊リスクがあるため不採用
  （session/test 側で供給する設計に）。

## Validation

- `mise run check synvelis-app`: exit 0。
- `mise run lint synvelis-app`: exit 0（`-D warnings`、
  `large_stack_arrays`/`too_many_lines`/`doc_markdown` 等を
  是正後）。
- `mise run test synvelis-app`: exit 0（全 test binary
  pass。`tests/recorder_wire.rs` 12 test:
  `recorder_wire_ops_registered_and_idle_without_device`・
  `recorder_wire_start_without_device_is_typed_error`・
  `recorder_wire_full_lifecycle_headless`・
  `recorder_wire_discard_closes_without_commit`・
  `recorder_wire_device_fault_latches_and_marks`・
  `recorder_wire_mark_take_verified`・
  `recorder_wire_project_new_resets_active_session`・
  `recorder_wire_commit_retries_after_staging_io_failure`・
  `recorder_wire_undo_redo_rejected_while_active`・
  `recorder_wire_start_rejects_oversized_target_channels`・
  `recorder_wire_start_rejects_loop_before_transport`・
  `recorder_wire_stop_retries_after_partial_finalize_failure`
  が全て pass）。
- `mise run build synvelis-app`: exit 0。
- `mise run check-app-native` / `mise run
  lint-app-native`: exit 0。
- `mise run check-app-gpui` / `mise run lint-app-gpui`:
  exit 0。
- `mise run check-headless-deps`: exit 0（headless crate
  への GUI/GPU/WebView 依存混入なし）。
- `mise run check-ruviform-rev`: exit 0（初回 stale
  vendor 検出 `ui vendor REV 20a24a66 != lockfile
  cbcc8343` を `mise run sync-ui-deps` で復旧後 pass。
  `ui/vendor/ruviform` は gitignore された生成物で
  tracked diff なし）。
- `mise run fmt-check`: exit 0。
- `mise exec -- prek run --all-files`: exit 0。
- `git diff` で禁止パス（`Cargo.toml`/`Cargo.lock`/
  `mise.toml`/`.github/**`/`docs/**/README.md`/
  `crates/synvelis-recording/src/**`/`ui/`）が空である
  ことを確認。
- 補助証拠（canonical = mise）: `cargo check --locked
  --all-targets -p synvelis-app` exit 0。
- 実 device 列挙・実 GUI-L2・EXT-L3・Issue #114 残 AC は
  本 slice 対象外で NotRun/Deferred を維持（headless
  Pass を昇格しない）。`*-msrv` はユーザー指定により
  未実行（NotRun）。`*-full`・3OS 横断は Daily Full CI
  委譲。
- review-protocol: 固定 acceptance criteria → deterministic
  check → 独立 reviewer 2 系統（cooperative A: 6 candidates、
  adversarial B: 9 candidates）→ read-only validator が
  全 candidate を source 照合（B 全 9 件 CONFIRMED +
  `loop_start` 検証欠落を追加検出）→ judge が severity
  判定し CONFIRMED P0/P1 + 明確な P2 のみ修正。
  P2/P3 の report-only 残件: post-mutation `start` 失敗が
  残す `Closed` session + `CaptureStream` entity が
  `RemoveLane` の state 非依存参照検査で lane を pin
  する件（domain に entity 除去 op が無く本 slice の
  境界外）、session 共有 `FaultLatch` で writer fault が
  session capture-fault marker を立て全 stream が
  recoverable になる件（domain の fail-closed 規約で
  あり app 層での bit 選別は domain 意図とずれ得るため
  owner 判断に委譲）、`RecorderError::into_body` の
  error code 分類の緩さ（taxonomy nit）。blocker 0。

## Risks

- `RecorderOwner` が `SynvelisApp` を肥大化させ、将来
  さらに大きな構造が追加されると stack 使用量が問題に
  なりうる。現行は `close_veto.rs` の 1 箇所を heap 化
  して是正済み。
- post-mutation 失敗の `CloseSession` が失敗した場合に
  open session が残る可能性。`transaction_open` 投影で
  観測可能であり、回復は discard/close 経路に委ねる。
- 実 device 列挙が未接続のため、production では
  `capabilities` の `recorder` が device 0 で
  `unavailable` を返し続ける。実 backend 接続は後続
  slice。

## Follow-ups

- 実 device 列挙/open（`DeviceEnumerator` 経由）と device
  選択 UX は後続 slice。
- `syn.monitoring.*` authored 宣言との統合（arm/punch/
  monitor UX、RCD-03/#117 系）は後続。
- `syn.recorder.*` の GUI driver（`ui/`）からの実呼び出し
  は Option B GUI 側 slice で確定。
- `docs/**/README.md` 管理 index の再生成は Coordinator
  が統合時に実施。
- Issue #114 の Close・merge は Coordinator が行う
  （PR は `Refs #114`）。
