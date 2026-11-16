---
id: "DISC-034Yr9NsLefhPhHuc5ZiEo"
type: "discovery"
status: "resolved"
title: "Issue #114 synvelis-app recorder session owner + syn.recorder.* headless wire"
created: "2026-11-16"
updated: "2026-11-16"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/114"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by:
    - "docs/specs/sv114-gui-option-b-recorder-wire.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv114-gui-option-b-recorder-wire.md"
    - "evidence/sv114/"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/sv114-gui-option-b-recorder-wire.md"
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-app/src/app.rs"
    - "crates/synvelis-app/src/recording.rs"
    - "crates/synvelis-app/src/service.rs"
    - "crates/synvelis-app/src/ui_ops.rs"
    - "crates/synvelis-app/src/monitoring.rs"
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

# Issue #114 synvelis-app recorder session owner + syn.recorder.* headless wire

## Summary

Issue #114（SV-GUI-05）の slice `sv114-gui-option-b-recorder-wire`
の探索記録。`synvelis-app` の `RecorderSlot` stub（`active()`
常時 false・`status()` 常時 `"idle"`）を、preflight 確定 plan・
`InputSession`・writer/staging ハンドルを保持する recorder
session owner に置換し、`syn.recorder.*` lifecycle op 面を
headless `UiRuntime` dispatch 経路に配線する方針を確定した。
`MonitorUxState`（monitoring.rs）が稼働 session を
`SynvelisApp` 直下に保持する構造先例であり、production API
（`synvelis-recording` の `preflight`/`InputSession`/
`StagingArea`/`StreamWriter`/`commit_session`/
`RecordingOp::MarkTakeVerified`）は充足済みで変更不要と確認
した（confirmed）。

## Exploration Goal

`RecorderSlot` stub を実態の recorder session owner に置換し、
`capabilities`・`close_veto`・`syn.recording.info` 投影・
`UiRuntime` dispatch が同一 owner を読む状態にする。device
注入 seam（test-local fake `Device`/`InputStream`）と
`syn.recorder.*` namespace の wire 契約を確定し、headless E2E
で lifecycle（preflight→open/add→writer/staging→start→pump→
stop→commit→`MarkTakeVerified`）を検証可能にすることが目的。

## Key Issues and Assumptions

- `RecorderSlot` は `app.rs` で `active()`/`status()` が定数を
  返す stub であり、`recording_info` の `recorder` ブロック・
  `capabilities` の `"recorder"` 行・`close_veto_reasons` の
  `recorder_active` が全てこの stub を読んでいた（confirmed:
  変更前コード精読）。
- `MonitorUxState`（`app.rs` の `monitor` field・monitoring.rs）
  が `preflight` + transport で稼働 session を `SynvelisApp`
  直下に保持する構造先例（confirmed）。recorder wire も同構造
  にする。
- `synvelis-recording` の production API は充足済み:
  `plan::preflight`（`RecordingRequest` → plan 確定）、
  `InputSession::open`/`add`（`&dyn Device` +
  `InputStreamSpec` 注入で `TapWriterEnds` を返す。`add` は
  same-clock/clock-epoch/stream_index 一意性を検証）、
  `StagingArea::create`/`open`、`StreamWriter`（`ChunkSink`
  消費）、`commit_session`（内部で
  `SynvelisOp::Recording(RecordingOp::CommitRecording)` を
  `ProjectController::apply`）、`RecordingOp::MarkTakeVerified`
  （confirmed: 各 source の signature/line を精読）。
- pinned ruviform rev `cbcc8343` は `Device`/
  `InputStream` port を提供済み:
  `Device::open_input_stream(&InputStreamConfig,
  InputDataCallback, InputErrorCallback) ->
  DeviceResult<Box<dyn InputStream>>`、`Device: Send`（
  confirmed: `ruviform/crates/ruviform-audio-core/src/
  device.rs`）。ruviform 変更不要。
- `synvelis-app` は `synvelis-recording` + `ruviform-audio-core`
  に依存済みで dep 追加不要（confirmed:
  `crates/synvelis-app/Cargo.toml`）。dev-dep の
  `ruviform-ui-test` はあるが `ruviform-audio-test` は本
  slice では追加しない（lease 外）。
- `build_payload` は stream が `project.recording.streams` に
  事前登録されていることを要求するため、project mutation
  （`RecordingOp::OpenSession` + stream 登録）を device open
  より先に行う必要があるか、device open を mutation 前に
  行い失敗時に project を汚さない順序設計が必要（confirmed:
  commit.rs/writer.rs の契約精読）。
- committed take は domain 上 `Verified` に遷移する
  （commit.rs の apply 経路）ため、`mark_take_verified` の
  正常系は「既に verified の take への再適用」または
  未 commit 状態での検証が必要（confirmed: テスト実測で
  確認）。
- `syn.recorder.*` は `syn.recording.*`（UI-C5
  control-recording tx）・`syn.monitoring.*` とは別
  namespace とする（Coordinator 設計確定事項）。

## Alternatives and Comparison

| Option | Pros | Cons | Lean |
| --- | --- | --- | --- |
| `RecorderOwner` を `recording.rs` に新設し `RecorderSlot` を re-export | 既存 `recording_info` 投影と同じ module に owner 状態を集約、`RecorderSlot` の外部参照を壊さない | `recording.rs` が大きくなる | yes |
| `RecorderSlot` stub を直接拡張 | 変更箇所が app.rs に閉じる | stub の責務（app 層 glue）と session lifecycle（recording domain 接続）が混ざり、recording.rs の投影と分離できない | no |
| `MonitorUxState` と同じく monitoring.rs に置く | 先例に一致 | recorder は recording domain であり monitoring ではない。責務境界が不自然 | no |
| `syn.recorder.*` namespace | `syn.recording.*`（control tx）・`syn.monitoring.*` と衝突しない | 新規 namespace の OPS 登録が必要 | yes |
| `syn.recording.*` に lifecycle を同居 | ops 表が小さい | UI-C5 control-recording tx の wire 契約と混同し、既存 op の意味を変える | no |
| `StagingArea` root を project bundle 配下に固定 | 永続化が自然 | test で tempdir を使えず、bundle 破壊リスク | no（session/test 側で供給する設計に） |
| device open を project mutation の後に行う | `build_payload` の前提を自然に満たす | device 失敗で project が dirty になる | no（device 失敗を mutation 前に済ませる順序を採用） |

## Tentative Conclusions and Hypotheses

- confirmed: `RecorderOwner`（仮称 `RecorderSession` owner）を
  `crates/synvelis-app/src/recording.rs` に実装し、
  `RecorderSlot` は `pub use
  crate::recording::RecorderOwner as RecorderSlot;` の互換
  re-export とする。`SynvelisApp.recorder` の型を更新し、
  `active()`/`status()` を session 実態から導出する。
- confirmed: device 注入 seam は `SynvelisApp::
  set_recorder_devices`（host 注入 `Device` の保持）とし、
  production の device 列挙は本 slice 対象外。device は
  `project.new`/`project.open` で reset せず保持し、active
  session のみ reset する。
- confirmed: `syn.recorder.*` ops は `service.rs` の `OPS`
  テーブルと dispatch match に登録し、`dispatch_recorder`
  helper で family を処理する。`syn.recorder.start` の
  wire params は `name`/`transport_start`/`stop`/optional
  range 群/`clock_epoch`/`sample_rate`/`staging_root`/
  `targets`/`streams` とし、tap は `stream_index`/
  `channel_map`/optional `arm`/`monitor` を取る。
- confirmed: start の順序は preflight → device stream open
  （`InputSession::open`/`add`）→ project mutation
  （`OpenSession` + stream 登録）→ staging/writer 構築 →
  stream start → pump。device 失敗は project mutation 前に
  完了させ、post-mutation 失敗は best-effort で
  `CloseSession` して session を残さない。
- confirmed: headless E2E は `crates/synvelis-app/tests/
  recorder_wire.rs` に test-local fake `Device`/
  `InputStream`（deterministic PCM、`MockPcm` 使用）を実装し、
  `UiRuntime::dispatch_op` + `Rc<RefCell<AppService>>` +
  `SharedService` delegation で実 dispatch 経路を駆動する。
- hypothesis: committed take が domain 上 `Verified` に
  遷移するため、`mark_take_verified` の負系（未 commit/
  不明 take id）は typed error で pin し、正系は commit 済み
  take への再適用で検証する（実測で確認済み）。

## Open Questions

- 実 device 列挙/open（`DeviceEnumerator` 経由の実 backend
  接続）は後続 slice。device 選択 UX・デフォルト device
  解決は #114 の残 AC と共に扱う。
- `syn.monitoring.*` の authored 宣言と recorder session の
  統合（arm/punch/monitor UX、RCD-03/#117 系）は後続。
- `syn.recorder.*` の wire 契約が GUI driver（`ui/`）から
  実際に呼ばれるかは Option B の GUI 側 slice で確定する。

## Promotion Candidates

- [x] spec-doc needed: `docs/specs/sv114-gui-option-b-recorder-wire.md`
- [ ] adr-doc needed: なし（既存 `MonitorUxState` 先例に
  従う局所的な owner 配置であり、アーキテクチャ判断を
  新規に伴わない）

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/114>
- 関連 Issue: `scarecrow173/synvelis#115`（RCD-01 headless
  recording projection）、`scarecrow173/synvelis#117`
  （RCD-03 Arm/Monitoring UX）
- 先例: `crates/synvelis-app/src/monitoring.rs` の
  `MonitorUxState`、`crates/synvelis-recording/tests/common/`
  の `OracleInputDevice`/`OracleInputStream` fake 実装
- 検証記録: `evidence/sv114/`

## Evidence

- 2026-11-16: `RecorderSlot` stub（`app.rs`）が `active()`/
  `status()` を定数で返し、`recording_info`・
  `capabilities`・`close_veto_reasons` が同 stub を読むことを
  コード精読で確認。
- 2026-11-16: `synvelis-recording` の `preflight`/`InputSession`/
  `StagingArea`/`StreamWriter`/`commit_session`/
  `RecordingOp::MarkTakeVerified` の signature と契約を精読し、
  production API の充足を確認（本 slice での
  `crates/synvelis-recording/src/**` 変更なし）。
- 2026-11-16: pinned ruviform `cbcc8343` の
  `ruviform-audio-core/src/device.rs` で `Device`/
  `InputStream` port を確認。`Device: Send`、
  `open_input_stream` が callback 注入式。
- 2026-11-16: `build_payload` が `project.recording.streams`
  への事前登録を要求することを commit.rs/writer.rs で確認し、
  device open を mutation 前・staging/writer を mutation 後に
  配置する順序を設計した。
- 2026-11-16: 実装後 `mise run test synvelis-app` で
  `tests/recorder_wire.rs` の 7 test（no-device typed
  failure・full lifecycle・discard・fault latch・
  mark_take_verified・project_new reset・ops 登録 idle
  projection）が全て pass することを実測。
