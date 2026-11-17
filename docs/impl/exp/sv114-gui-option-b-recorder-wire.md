---
id: "EXP-034YrbyYhH8dQfJjK2mN4p"
type: "experiment"
status: "completed"
title: "Issue #114 synvelis-app recorder session owner + syn.recorder.* headless wire の試行 (synvelis#114)"
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
  implements: []
  implemented-by:
    - "docs/impl/ir/sv114-gui-option-b-recorder-wire.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv114-gui-option-b-recorder-wire.md"
    - "docs/test-specs/sv114-gui-option-b-recorder-wire.md"
    - "docs/impl/exp/sv114-gui-option-b-recorder-wire-exp.jsonl"
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
    - "mise.toml"
  defers: []
  deferred-by: []
---

# Issue #114 synvelis-app recorder session owner + syn.recorder.* headless wire の試行

## 問題設定

`synvelis-app` の `RecorderSlot` は `active()`/`status()`
が定数を返す stub で、capabilities が `unavailable`/`idle`
固定、close veto が recorder 活動を反映しなかった。
Issue #114 の Option B slice として、recorder session
owner への置換と `syn.recorder.*` lifecycle op 面の
headless 配線を、`synvelis-recording` production API の
再利用と fake `Device`/`InputStream` による E2E で実現
することが試行の核心だった。

## 試行と結果

### E1: 既存構造と API の調査

`RecorderSlot` stub（`app.rs`）と `recording_info`・
`capabilities`・`close_veto_reasons` の参照を精読し、
`MonitorUxState`（monitoring.rs）が稼働 session を
`SynvelisApp` 直下に保持する構造先例であることを確認。
`synvelis-recording` の `preflight`/`InputSession::open`/
`add`/`StagingArea`/`StreamWriter`/`commit_session`/
`RecordingOp::MarkTakeVerified` の signature と契約を
精読し、production API が充足済みで変更不要と確認した。
pinned ruviform `cbcc8343` の `Device`/`InputStream` port
（`open_input_stream` callback 注入式、`Device: Send`）を
確認し、ruviform 変更不要と結論付けた。

### E2: owner 配置と namespace の決定

`RecorderOwner` を `recording.rs` に新設し、`RecorderSlot`
は `pub use crate::recording::RecorderOwner as RecorderSlot;`
の互換 re-export とした。`syn.recorder.*` を
`syn.recording.*`（UI-C5 control tx）・`syn.monitoring.*`
と独立した新規 namespace とし、`OPS` テーブルと dispatch
（`dispatch_recorder` helper）に登録した。

### E3: lifecycle 順序の設計と実装

`start` の順序を preflight → device stream open
（`InputSession::open`/`add`）→ project mutation
（`OpenSession` + stream 登録）→ staging/writer 構築 →
stream start とした。device 失敗を project mutation 前に
完了させ、post-mutation 失敗は best-effort `CloseSession`
で session を残さない。`build_payload` の「stream が
`project.recording.streams` に事前登録」前提と device
open の順序は、`InputSession` が返す `TapWriterEnds` が
device/stream handle であり `project.recording.streams`
登録は mutation 段で行うため両立することを確認した。

### E4: headless E2E と fake device

`tests/recorder_wire.rs` に test-local fake `Device`/
`InputStream`（deterministic PCM、`MockPcm` 使用）を実装。
`UiRuntime::new(DataRegistry::new())` + `rt.dispatch_op`
+ `Rc<RefCell<AppService>>` + `SharedService` delegation
で実 dispatch 経路を駆動し、no-device typed failure・
full lifecycle（preflight→open/add→writer/staging→start→
pump→stop→commit）・discard・device fault latch・
`mark_take_verified`・`project_new` reset・commit retry
（staging IO 失敗後の冪等 retry）・stop partial-finalize
retry（journal 一時破損→復元）・commit non-terminal
outcome retention（`nothing-committed` で session 保持）・
undo/redo gate・start 境界検証（channels 上限・loop
gate）・ops 登録 idle 投影の 12 test を実装した。

### E5: 検証と clippy 是正

`mise run check/lint/test/build synvelis-app` で実測検証。
`RecorderOwner` が `SynvelisApp` を肥大化させ `tests/
close_veto.rs` の stack array が `clippy::large_stack_
arrays` に触れたため、`vec![...].into_boxed_slice()` に
是正。`recording.rs`/`service.rs`/`ui_ops.rs` の
`too_many_lines`・`doc_markdown`・`match_same_arms` 等を
構造修正（`RigInputs` 集約・`dispatch_recorder` helper・
backtick 付与）で解消し、`#[allow]` は full lifecycle
test の `too_many_lines` のみに狭く使用した。
`check-ruviform-rev` が stale vendor（`20a24a66` !=
lockfile `cbcc8343`）を検出したため `mise run
sync-ui-deps` で復旧（`ui/vendor/ruviform` は gitignore
された生成物で tracked diff なし）。

### E6: review で検出した commit retry wedge の是正

review 中に `RecorderOwner::commit` の retry 経路を精査し、
`commit_session` が `Err`（staging/commit manifest の IO
失敗等）を返した後の retry が永久に失敗する欠陥を検出。
`CloseSession` は適用済みで project session は `Closed`
に進むが、retry が無条件に `CloseSession` を再適用すると
`RecordingOp::validate`（`Open` のみ受理）で reject される。
`commit`/`discard` で persisted session state を先に読み、
`Open` のときだけ `CloseSession` を適用し、`Closed`/
`Finalized` では `commit_session` の `commit_status`
冪等経路に委ねる形に是正した。regression test
`recorder_wire_commit_retries_after_staging_io_failure`
（破損 `commit-manifest.json` で `commit_session` を一度
失敗させ、除去後の retry が `applied` で完了することを pin）
を追加し、8 test 全て pass を確認した。

### E7: review-protocol 第 2 ラウンド（2 reviewer + validator + judge）

cooperative reviewer（6 candidates）と adversarial
reviewer（9 candidates）の独立報告を read-only
validator が source 照合し、B 全 9 件 CONFIRMED +
`loop_start >= transport_start` 検証欠落を追加検出。
judge が severity 判定し、CONFIRMED の wedge/orphan/
abort 経路のみ修正した:

- `stop` の部分 finalize wedge: `finish_all` が全 writer
  に無条件 `finalize` を掛け、`StreamWriter::finalize` は
  `AlreadyFinalized` で `Err` — 先行 stream 成功後の
  retry が永久に失敗し `commit`（`Stopped` 要求）に
  到達不能だった。`AlreadyFinalized` を成功として扱う
  よう是正し、journal 一時破損→復元で部分失敗を再現する
  `recorder_wire_stop_retries_after_partial_finalize_
  failure` を追加。
- `commit` の outcome orphan: `commit_session` は
  `Failed`/`IntegrityConflict`/`NothingCommitted` も
  `Ok(CommitReport)` で返すが、`commit` が無条件に
  `active` を drop していた — wire に session 除去 op
  が無く project session（`Closed` 残留）が孤立。
  `Applied`/`AlreadyApplied` のみ破棄し他は `stopped`
  保持に是正。`commit_session` が `CommitRecording`
  適用後に marker 書込み失敗で `Err` を返す経路では
  retry の `AlreadyApplied` が revision を進めず
  `notify_committed_change` が永久 skip されるため、
  `Err` arm でも revision 前進を検出して通知。
- `syn.project.undo`/`redo` の無 gate: undo は snapshot
  復元で per-op validation を通らず recorder の
  session/stream entity を巻き戻し得た。
  `recorder.active()` 中は `invalid_params` で拒否
  （`syn.monitoring.punch` の `require_no_session` と
  同じ境界規則）。
- `targets[].channels` 無制限: `build_tap_resources` が
  `max_block_frames × channel_map.len()` の queue と PCM
  pool を eager 確保し、巨大値で process abort し得た。
  wire 上限 256（`MAX_TARGET_CHANNELS`）を設定。
- `loop_start < transport_start` 未検証: 負 session
  frame で `GateScheduler` が構成できず runtime fault
  になっていた。start 時に `invalid_params` で拒否
  （monitoring session と同じ規則）。
- `count_in`/`pre_roll` dead params 削除: recorder の
  gate/`RecordingRequest` に対応 field が無く消費・
  検証されない wire surface だった。

report-only（修正せず judge が記録）: post-mutation
`start` 失敗が残す `Closed` session + `CaptureStream`
entity の lane pin（domain に除去 op が無く境界外）、
session 共有 `FaultLatch` の writer fault → 全 stream
recoverable 化（domain の fail-closed 規約、owner 判断に
委譲）、`RecorderError::into_body` の error code 分類
（taxonomy nit）。12 test 全て pass を確認した。

## 採用した判断

- **owner 配置は `recording.rs` + `RecorderSlot` 互換
  re-export**: `recording_info` 投影と同じ module に owner
  状態を集約し、app.rs の stub 責務と recording domain
  接続を分離した。
- **device open を project mutation 前に配置**: device
  失敗が project を dirty にしない順序を優先。
- **post-mutation 失敗は best-effort `CloseSession`**:
  open session を残さず、`transaction_open` 投影で観測
  可能にした。
- **`syn.recorder.*` namespace**: 既存 `syn.recording.*`/
  `syn.monitoring.*` の wire 契約を変えない独立 namespace。
- **typed error 写像**: domain 失敗を `ErrorBody` code に
  写像し、defined rejection を panic と混同しない。
- **clippy 是正は構造修正のみ**: `#[allow]` は原則不使用、
  test の `too_many_lines` のみ狭く使用。

## 結論

`RecorderSlot` stub を `RecorderOwner` に置換し、
`syn.recorder.*` lifecycle ops を headless `UiRuntime`
dispatch 経路に配線した。capabilities・close veto・
`recording_info` が同一 owner の実態を投影することを
fake `Device`/`InputStream` による E2E で実測確認した。
変更範囲の検証 lane は全て pass。実 device 列挙・実
GUI-L2・EXT-L3・Issue #114 残 AC は本 slice 対象外で
NotRun/Deferred を維持し、merge 後 CI/別 gate に委譲する。
