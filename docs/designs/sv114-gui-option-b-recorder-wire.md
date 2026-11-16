---
id: "DESIGN-034Yr9OvufCMQtVOo9MClW"
type: "design"
status: "approved"
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
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/app.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/close_veto.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sv114-gui-option-b-recorder-wire.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/sv114-gui-option-b-recorder-wire.md"
    - "docs/test-specs/sv114-gui-option-b-recorder-wire.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv114-gui-option-b-recorder-wire.md"
  derived-by:
    - "docs/plans/sv114-gui-option-b-recorder-wire.md"
    - "docs/test-specs/sv114-gui-option-b-recorder-wire.md"
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-app/src/app.rs"
    - "crates/synvelis-app/src/recording.rs"
    - "crates/synvelis-app/src/service.rs"
    - "crates/synvelis-app/src/ui_ops.rs"
    - "crates/synvelis-app/src/monitoring.rs"
    - "crates/synvelis-app/tests/recorder_wire.rs"
    - "crates/synvelis-recording/src/plan.rs"
    - "crates/synvelis-recording/src/input.rs"
    - "crates/synvelis-recording/src/staging.rs"
    - "crates/synvelis-recording/src/writer.rs"
    - "crates/synvelis-recording/src/commit.rs"
    - "crates/synvelis-recording/src/ops.rs"
    - "mise.toml"
  defers: []
  deferred-by: []
---

# Issue #114 synvelis-app recorder session owner + syn.recorder.* headless wire — 設計

## Context

`synvelis-app` の `RecorderSlot` は `app.rs` で `active()`/
`status()` が定数を返す stub であり、`recording_info` の
`recorder` ブロック・`capabilities` の `"recorder"` 行・
`close_veto_reasons` の `recorder_active` が全てこの stub
を読んでいた。`MonitorUxState`（app.rs の `monitor` field・
monitoring.rs）が `preflight` + transport で稼働 session を
`SynvelisApp` 直下に保持する構造先例があり、recorder wire
も同構造にする。

`crates/synvelis-recording` の production API は充足済みで
本 slice では変更しない: `plan::preflight`（
`RecordingRequest` → plan 確定）、`InputSession::open`/`add`
（`&dyn Device` + `InputStreamSpec` 注入で `TapWriterEnds`、
`add` は same-clock/clock-epoch/stream_index 一意性を検証）、
`StagingArea::create`/`open`、`StreamWriter`（`ChunkSink`
消費）、`commit_session`（内部で
`SynvelisOp::Recording(RecordingOp::CommitRecording)` を
`ProjectController::apply`）、`RecordingOp::MarkTakeVerified`。
pinned ruviform `cbcc8343` は `Device`/`InputStream` port を
提供済みで ruviform 変更不要。

## Scope

### In Scope

- `crates/synvelis-app/src/recording.rs` への `RecorderOwner`
  実装と `RecorderSlot` 互換 re-export
- `SynvelisApp.recorder` の型更新・`set_recorder_devices`
  注入 seam・project new/open 時の reset
- `service.rs` の `OPS` 登録と `dispatch_recorder` helper
- `ui_ops.rs` の `syn.recorder.*` op 実装と wire param
  parser
- `capabilities`/`close_veto_reasons`/`recording_info` の
  実態一致
- `tests/recorder_wire.rs` の headless E2E（fake
  `Device`/`InputStream`）と `tests/close_veto.rs` の
  clippy 是正
- 本タスクの doc-chain・IR/exp・`evidence/sv114/`・sv115
  ledger/matrix pointer 更新

### Out of Scope

- 実 device 列挙/open・device 選択 UX・実 backend 接続
- 実 GUI-L2・EXT-L3・実 OS Scenario Runner
- `crates/synvelis-recording/src/**` の変更
- `syn.monitoring.*` authored 宣言との統合
- `Cargo.toml`/`Cargo.lock`/`mise.toml`/`.github/**`/`ui/`/
  `docs/**/README.md` の変更

## Components and Boundaries

- `crates/synvelis-app/src/recording.rs`:
  `RecorderOwner` が authoritative owner。host 注入
  `Device`（`Vec<Box<dyn Device>>` 相当）と session-scoped
  active state を保持する。`ActiveRecorder`（仮の内部構造）
  が確定 plan・`InputSession`・`StagingArea`・
  `StreamWriter` 列・fault latch・session metadata を保持
  する。`RecorderError` が typed error を `ErrorBody` code
  に写像する。`recording_info` 投影は persisted entities +
  owner 状態（`status`/`active`/`transaction_open`）を
  統合する。
- `crates/synvelis-app/src/app.rs`:
  `RecorderSlot` は `pub use
  crate::recording::RecorderOwner as RecorderSlot;` の互換
  re-export。`SynvelisApp.recorder` の型を `RecorderOwner`
  に更新し、`project_new_inner`/`project_open_inner` で
  active session を reset（device は保持）。
  `set_recorder_devices` で host 注入。`capabilities` の
  `"recorder"` 行と `close_veto_reasons` の
  `recorder_active` が同じ owner を読む。
- `crates/synvelis-app/src/service.rs`:
  `OPS` テーブルに `syn.recorder.start`/`pump`/`stop`/
  `commit`/`discard`/`status`/`mark_take_verified` を登録し、
  `name.starts_with("syn.recorder.")` の dispatch arm から
  `dispatch_recorder` helper へ family を委譲する。
  未登録の `syn.recorder.*` 名は unknown-op error を返す。
- `crates/synvelis-app/src/ui_ops.rs`:
  `op_recorder_start`/`pump`/`stop`/`commit`/`discard`/
  `status`/`mark_take_verified` を実装。wire param parser
  （`opt_clock_epoch_param`/`opt_u32_param`/
  `recorder_targets_param`/`recorder_streams_param`）が
  tagged `LaneId`・tick/range・u32・targets/streams/taps・
  arm/monitor を decode する。
- `crates/synvelis-app/tests/recorder_wire.rs`:
  test-local fake `Device`/`InputStream`（deterministic PCM、
  `MockPcm`、`start`/`stop`/`format`/`buffer_size`/`now`/
  `clock_domain`/`input_latency` を実装、output method は
  `Unsupported`）。`UiRuntime::new(DataRegistry::new())` +
  `rt.dispatch_op` + `Rc<RefCell<AppService>>` +
  `SharedService` delegation で実 dispatch 経路を駆動する。
- `crates/synvelis-app/tests/close_veto.rs`:
  `RecorderOwner` による `SynvelisApp` サイズ増で発火した
  `clippy::large_stack_arrays` を、case 配列の
  `vec![...].into_boxed_slice()` 化で是正（test-local、
  挙動不変）。

## Data and Control Flow

`syn.recorder.start` の実行順序:

1. wire param decode（`RecorderStartParams`/
   `RecorderStreamParams`/`RecorderTapParams`）。
2. `RecorderOwner::start`: active session 存在・device 未
   注入・device index 範囲・stream index の target 参照・
   armed tap の channel_map 長一致・armed tap 0 件を検証。
3. `plan::preflight`（`RecordingRequest` 構築 — monitoring.rs
   の monitor session 開始が先例）。
4. device stream open: `InputSession::open`/`add`（注入
   `&dyn Device` + `InputStreamSpec`）。device 失敗はここで
   完了し project を汚さない。
5. project mutation: `RecordingOp::OpenSession` + stream
   登録を `ProjectController::apply` で適用。
6. staging/writer 構築: `StagingArea::create` +
   `StreamWriter`（`build_writers`）。
7. stream start + active state 確立。post-mutation 失敗は
   best-effort `CloseSession` で session を残さない。
8. `pump`: input work queue を drain して engine/writer へ。
9. `stop`: stream 停止 + writer pump/finalize → stopped
   session（commit retry 可能）。
10. `commit`: stopped session 要求 → `CloseSession` 適用 →
    `commit_session` → typed commit 分類/report。
11. `discard`: active stream 停止 → best-effort
    `CloseSession` → commit せず active state drop
    （staging file 保持）。
12. `mark_take_verified`: `RecordingOp::MarkTakeVerified`
    適用 → take 状態を返す。

投影の実態一致: `recording_info` は persisted
`sessions`/`passes`/`streams`/`segments`/`takes` + recorder
`status`/`active`/`transaction_open` を同一投影に含める。
`capabilities` の `recorder` は device 数 > 0 で available、
0 で `unavailable` + `reason: "no recorder input device
injected"`。`close_veto_reasons` は `self.recorder.active()`
が真のとき `"recorder_active"` を追加する。

## Non-Functional Constraints

- typed error: domain 失敗（`InputBindError`/
  `StagingError`/`CommitError`/preflight reject）は
  `ErrorBody` code に写像し、defined rejection を panic/
  crash と混同しない。
- wire 互換: 既存 `syn.recording.*`/`syn.monitoring.*`/
  `syn.app.*`/`syn.window.*` の op 契約を変えない。
  `syn.recorder.*` は新規 namespace。
- lint 非隠蔽: `#[allow]` は原則不使用。`tests/
  recorder_wire.rs` の full lifecycle test のみ、逐次
  lifecycle の長さが本質的であるため狭い
  `#[allow(clippy::too_many_lines)]` を comment 付きで
  使用（test のみ、production code には適用しない）。
- 時間演算: `ruviform-media-core` の `Tick`/`TickDuration`
  は checked 操作のみ使用（`checked_add`/`checked_sub`）。
- 日本語 doc/comment、識別子は backtick 記法
  （`clippy::doc_markdown` 対応）。

## Risks and Trade-offs

- `RecorderOwner` が `SynvelisApp` を大きくし、既存 test の
  stack array が `clippy::large_stack_arrays` に触れる →
  `tests/close_veto.rs` の case 配列を
  `vec![...].into_boxed_slice()` に変更して是正（test-local、
  production 非影響）。
- device open を mutation 前に置く順序は `build_payload`
  の「stream が `project.recording.streams` に事前登録」
  前提と一見矛盾するが、`InputSession::open`/`add` が返す
  `TapWriterEnds` は device/stream handle であり、
  `project.recording.streams` への登録は mutation 段で
  行うため両立する（実測で確認）。
- committed take は domain が commit 時に `Verified` へ
  自動遷移させるため、`mark_take_verified` の正系は
  `Open` session 上の `Raw` take への適用となる
  （`Finalized` session の take は domain が拒否）。
  これは domain の正直な状態遷移であり、test もその
  実態を pin する。
- post-mutation 失敗の cleanup は best-effort であり、
  `CloseSession` 自体が失敗した場合に open session が残る
  可能性は残る。現行は `transaction_open` 投影で観測可能
  であり、回復は discard/close 経路に委ねる。

## Deferred Design Concerns

- 実 device 列挙/open（`DeviceEnumerator`）と device 選択
  UX | 本 slice は注入 seam まで | device 列挙 slice 着手時
  | 実 backend 未接続で recorder が常に unavailable
- `syn.monitoring.*` authored 宣言との統合 | RCD-03/#117 系
  の後続 | monitoring UX slice 着手時 | session 二重管理
- `syn.recorder.*` の GUI driver 呼び出し | Option B GUI 側
  slice | GUI 側 slice 着手時 | wire 契約のずれ

## Verification Notes

- [x] `mise run check synvelis-app` / `mise run lint
  synvelis-app` / `mise run test synvelis-app` /
  `mise run build synvelis-app`（全て exit 0、
  `tests/recorder_wire.rs` 12 test pass を実測）
- [x] `mise run check-app-native` / `mise run
  lint-app-native` / `mise run check-app-gpui` / `mise run
  lint-app-gpui`（feature 面の型・lint、全て exit 0）
- [x] `mise run check-headless-deps`（headless crate への
  GUI/GPU/WebView 依存混入なし）
- [x] `mise run check-ruviform-rev`（lockfile/vendor/manifest
  の ruviform rev 3-way 整合 — 初回 stale vendor 検出を
  `mise run sync-ui-deps` で復旧後 pass）
- [x] `mise run fmt-check` / `mise exec -- prek run
  --all-files`
- [ ] 実 device 列挙・実 GUI-L2・EXT-L3・Issue #114 残 AC は
  merge 後 CI/別 gate の証拠に委譲（本 slice の headless
  Pass を昇格しない）
