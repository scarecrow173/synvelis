---
id: "TSPEC-034Yr9OWSvOrAfLqvNTpWC"
type: "test-spec"
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
        path: "crates/synvelis-app/tests/recorder_wire.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-app/tests/close_veto.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv114-gui-option-b-recorder-wire.md"
    - "docs/designs/sv114-gui-option-b-recorder-wire.md"
    - "evidence/sv114/recorder-wire-option-b-headless.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sv114-gui-option-b-recorder-wire.md"
  derived-by: []
  verifies:
    - "docs/specs/sv114-gui-option-b-recorder-wire.md"
  verified-by: []
  references:
    - "crates/synvelis-app/tests/recorder_wire.rs"
    - "crates/synvelis-app/tests/close_veto.rs"
    - "crates/synvelis-app/src/recording.rs"
    - "crates/synvelis-app/src/app.rs"
    - "crates/synvelis-app/src/service.rs"
    - "crates/synvelis-app/src/ui_ops.rs"
    - "mise.toml"
  defers: []
  deferred-by: []
---

# Issue #114 synvelis-app recorder session owner + syn.recorder.* headless wire — 検証仕様

## Purpose

`synvelis-app` の recorder session owner と `syn.recorder.*`
lifecycle op 面が、fake `Device`/`InputStream` による headless
E2E で実際の dispatch 経路を通して正しく動作し、
capabilities・close veto・`syn.recording.info` 投影が同一
owner の実態を反映することを保証する test spec。headless
fake-device の Pass を実 device/実 GUI-L2/EXT-L3 の Pass に
昇格しない境界を、検証対象の正直な範囲として記録する。

## Feature

recorder lifecycle wire: `RecorderOwner` が `syn.recorder.*`
ops を通じて preflight→device open→project mutation→
staging/writer→stream start→pump→stop→commit/discard→
`mark_take_verified` の lifecycle を駆動し、
`syn.app.capabilities`・`syn.window.close_guard`・
`syn.recording.info` が同一 owner の状態を投影する。

## Rules

- Rule: typed failure — device 未注入・active session 重複・
  wire 制約違反・domain 失敗は `ErrorBody` code 付きの
  typed error で reject され、panic/crash しない。
- Rule: 実態一致 — `syn.app.capabilities` の `recorder`、
  `syn.window.close_guard` の `recorder_active`、
  `syn.recording.info` の `recorder` ブロックは全て同一
  `RecorderOwner` の状態を反映する。session active 中に
  veto が出て idle 時に出ない。
- Rule: namespace 独立 — `syn.recorder.*` は
  `syn.recording.*`（control-recording tx）・
  `syn.monitoring.*` と独立し、既存 op の wire 契約を
  変えない。
- Rule: device seam — device は `set_recorder_devices` で
  注入され、`project.new`/`project.open` で保持される。
  active session のみ reset される。
- Rule: 検証の実測性 — Pass 判定は実際に実行した mise
  タスクの結果のみに基づき、未実行の検証（実 device・実
  GUI-L2・EXT-L3）を Pass と記録しない。

## Examples

- Example: ops 登録と idle 投影 — Given device 未注入の
  app, When `UiRuntime::dispatch_op` で `syn.recorder.*`
  ops の登録を確認し `syn.recorder.status` を呼ぶ, Then
  `OPS` に 7 op が登録され、status は `unavailable` +
  reason + device 数 0 + idle を返す。
- Example: device 未注入の typed failure — Given device
  未注入の app, When `syn.recorder.start` を呼ぶ, Then
  typed error（device なし）で reject され、project に
  recording session が作られない。
- Example: full lifecycle — Given fake `Device` 注入済み
  app, When `syn.recorder.start`→`pump`→`stop`→`commit`
  を dispatch する, Then preflight→open/add→writer/
  staging→stream start→pump→stop→commit が成功し、
  `syn.recording.info` の persisted entities（session/
  stream/take）と `recorder.status`/`active`/
  `transaction_open` が遷移し、close veto が session
  active 中に `recorder_active` を報告する。
- Example: discard — Given recording 中の session, When
  `syn.recorder.discard` を呼ぶ, Then stream が停止し
  `CloseSession` が適用され commit されず active state
  が drop される（staging file は保持）。
- Example: device fault — Given fault を注入した fake
  device, When `syn.recorder.pump` 中に error callback
  が発火する, Then fault latch が立ち status/fault
  投影に反映される。
- Example: mark_take_verified — Given `Open` session 上の
  `Raw` take（committed session は `Finalized` で domain が
  op を拒否するため、正系は未 commit take）, When
  `syn.recorder.mark_take_verified` を呼ぶ, Then
  `RecordingOp::MarkTakeVerified` が適用され take 状態が
  返る。未 commit/不明 take id・`Finalized` session の
  take は typed error。
- Example: commit retry — Given `commit_session` が staging
  IO 失敗で `Err` を返した stopped session（project session
  は `CloseSession` 済みの `closed`）, When `syn.recorder.
  commit` を retry する, Then 適用済み `CloseSession` の
  再適用ではなく `commit_session` の冪等経路で commit が
  完了し session は `finalized` になる。
- Example: stop partial-finalize retry — Given 2 armed
  stream の recording session, When stream-0001 の journal
  を一時破損して `syn.recorder.stop` を呼び journal を
  復元して retry する, Then 1 回目は `service_error` で
  位相 `recording` のまま、retry は先行 stream の
  `AlreadyFinalized` を成功として扱い `finalized: 2` で
  `stopped` に到達し commit が完走する。
- Example: commit non-terminal outcome retention — Given
  capture-fault marker 記録済みの stopped session, When
  `syn.recorder.commit` を呼ぶ, Then outcome は
  `nothing-committed`（全 stream recoverable）で runtime
  session は `stopped`/`active` のまま保持され、
  `recorder_active` veto が残り、`syn.recorder.discard`
  で畳める。
- Example: undo/redo gate — Given recorder session
  `recording`/`stopped`, When `syn.project.undo`/`redo`
  を呼ぶ, Then `invalid_params` で拒否され session/stream
  entity が巻き戻らない。commit 確定後は undo が再び
  使える。
- Example: start 境界検証 — Given `targets[].channels` が
  wire 上限超過、または `loop_start < transport_start`
  の loop 宣言, When `syn.recorder.start` を呼ぶ, Then
  `invalid_params` で拒否され project/runtime session
  が作られない。
- Example: project_new reset — Given active recorder
  session, When `project.new` を dispatch する, Then
  active session が reset され device は保持される。
- Example: close veto — Given recorder session active,
  When `syn.window.close_guard` を呼ぶ, Then
  `recorder_active` veto が含まれる。idle 時は含まれない。

## Guarantees

- 正しい実装は、`mise run test synvelis-app` で
  `tests/recorder_wire.rs` の 12 test が全て pass する
  ことを保証する。
- 正しい実装は、`mise run test synvelis-app` の既存全
  suite（`close_veto`/`ui_ops`/`service_ops`/
  `monitor_arm` 等）が非回帰で pass することを保証する。
- 正しい実装は、`syn.recorder.*` が typed op として
  dispatch され、未知の `syn.recorder.*` 名は unknown-op
  error を返すことを保証する。
- 正しい実装は、実 device 列挙・実 GUI-L2・EXT-L3 の
  検証を本 slice で Pass と主張しないことを保証する
  （NotRun/Deferred を維持）。

## Non-goals

- 実 device 列挙/open・実 WASAPI callback・実 audio
  hardware での capture は対象外。
- 実 GPUI Window での録音 UI 操作（GUI-L2）・実 plugin
  dock・実 OS Scenario Runner（EXT-L3）は対象外。
- `synvelis-recording` の domain ロジック自体の正しさは
  対象外（`crates/synvelis-recording/` の既存 test が
  担保）。
- `syn.monitoring.*` authored 宣言との統合は対象外。
- `*-msrv`・`*-full`・3OS 横断・全 Golden/長時間回帰は
  対象外（Daily Full CI 委譲）。

## Risk

- この保証が崩れると、recorder session owner が実態と
  異なる投影を返し、close veto・capabilities・
  `syn.recording.info` が嘘をつく。device 失敗が project
  を dirty にする順序 bug が混入すると、録音失敗が
  undo/dirty 状態を汚染する。headless Pass を実 GUI/実
  device の証拠に転用すると、#114 の残 AC が未検証のまま
  誤って充足したと誤認される。
