---
id: "SPEC-034Yr9NUlwfGUnqDVVWXOp"
type: "spec"
status: "implemented"
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
  implements: []
  implemented-by:
    - "docs/plans/sv114-gui-option-b-recorder-wire.md"
    - "docs/designs/sv114-gui-option-b-recorder-wire.md"
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/discovery/sv114-gui-option-b-recorder-wire.md"
    - "docs/designs/sv114-gui-option-b-recorder-wire.md"
    - "docs/test-specs/sv114-gui-option-b-recorder-wire.md"
    - "evidence/sv114/"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "docs/discovery/sv114-gui-option-b-recorder-wire.md"
  derived-by:
    - "docs/designs/sv114-gui-option-b-recorder-wire.md"
    - "docs/test-specs/sv114-gui-option-b-recorder-wire.md"
  verifies: []
  verified-by:
    - "docs/test-specs/sv114-gui-option-b-recorder-wire.md"
  references:
    - "crates/synvelis-app/src/app.rs"
    - "crates/synvelis-app/src/recording.rs"
    - "crates/synvelis-app/src/service.rs"
    - "crates/synvelis-app/src/ui_ops.rs"
    - "crates/synvelis-app/tests/recorder_wire.rs"
    - "mise.toml"
  defers: []
  deferred-by: []
---

# Issue #114 synvelis-app recorder session owner + syn.recorder.* headless wire

## What Should Be Built

`synvelis-app` の `RecorderSlot` stub を、録音 session の
lifecycle（preflight→`InputSession` open/add→stream 登録→
staging/writer→stream start→pump→stop/finalize→commit/
discard）を所有する recorder session owner に置換する。
owner は `synvelis-recording` の production API を再利用し、
GUI host・headless `UiRuntime`・将来の CLI surface が同一の
authoritative state を共有する。`syn.recorder.*` lifecycle
ops を `OPS` テーブルと dispatch に登録し、
`syn.app.capabilities`・`syn.window.close_guard`・
`syn.recording.info` 投影が全て同じ owner を読む状態にする。

## Why Now

Issue #114（SV-GUI-05 GPUI Native Product E2E・Plugin Dock
Gate・OS/Consumer 受入）の Option B slice として、
`synvelis-app` 層の Recorder command/UI wire の headless
実装が必要。現状 `RecorderSlot` は `active()`/`status()` が
定数を返す stub で、capabilities が `unavailable`/`idle`
固定、close veto が recorder 活動を反映しない。Option B gate
および #114 残 AC の前提として、headless fake-device で
lifecycle wire を先に確立する。

## Users and Value

- GUI/CLI/driver 側の実装者: `syn.recorder.*` の typed op 面
  と live 投影が揃い、実 device 列挙 slice の前提が整う。
- #114 受入の Coordinator: headless lane の Pass/Fail を
  実 OS/実 GUI の AC と分離して記録できる。
- `synvelis-recording` の domain ロジック利用者:
  app 層に recording semantics を複製せず、owner が domain
  API をそのまま呼ぶ構造で保守対象が一元化される。

## Scope

### In Scope

- `RecorderSlot` stub の recorder session owner への置換
  （`recording.rs` の `RecorderOwner` + `RecorderSlot`
  互換 re-export）
- `syn.recorder.*` lifecycle ops の `OPS` 登録と dispatch
  （`start`/`pump`/`stop`/`commit`/`discard`/`status`/
  `mark_take_verified`）
- device 注入 seam（`SynvelisApp::set_recorder_devices`）と
  project new/open 時の active session reset
- `syn.app.capabilities` の `recorder` 状態、
  `syn.window.close_guard` の `recorder_active` veto、
  `syn.recording.info` の `recorder` ブロックの実態一致
- headless E2E（`tests/recorder_wire.rs`、test-local fake
  `Device`/`InputStream`、`UiRuntime` dispatch harness）
- 本タスクの doc-chain・実装記録・検証証拠の作成

### Out of Scope

- 実 device 列挙/open（`DeviceEnumerator` 経由の実 backend
  接続）・device 選択 UX
- 実 GUI-L2（実 GPUI Window での録音 UI 操作）・EXT-L3・
  実 OS Scenario Runner
- `crates/synvelis-recording/src/**` の変更（production API
  は充足済み）
- `syn.monitoring.*` の authored 宣言と recorder session の
  統合（arm/punch/monitor UX、RCD-03/#117 系）
- `Cargo.toml`/`Cargo.lock`/`mise.toml`/`.github/**`/`ui/`/
  `docs/**/README.md` の変更（dep 追加なし、index は
  Coordinator が統合時に再生成）
- Issue #114 の残り AC（SVT-01〜09 実走・Windows GUI-L2/
  EXT-L3・SV-OS case・cross-consumer gate）

## Requirements

- `syn.recorder.start` は active session 存在時・device 未
  注入時・wire 制約違反時（device index 範囲・stream index
  の target 参照・armed tap の channel_map 長・armed tap
  0 件）に typed error で reject すること。
- start は `preflight` を経て、device stream open を project
  mutation 前に完了させ、project mutation（`OpenSession` +
  stream 登録）→ staging/writer 構築 → stream start の順で
  実行すること。post-mutation 失敗は best-effort で
  session を閉じ、open session を残さないこと。
- `syn.recorder.pump` は input work queue を drain して
  recording engine/writer へ供給すること。
- `syn.recorder.stop` は stream を停止し writer を
  pump/finalize して、commit retry 可能な stopped session を
  残すこと。
- `syn.recorder.commit` は stopped session を要求し、
  `CloseSession` 適用と `commit_session` を行い、typed
  commit 分類/report を返すこと。outcome が commit 確定
  （`Applied`/`AlreadyApplied`）の場合のみ runtime session
  を破棄し、`Failed`/`IntegrityConflict`/`NothingCommitted`
  では `stopped` session を保持して retry/discard を
  可能にすること。`commit_session` が `CommitRecording`
  適用後に `Err` を返した場合も revision 前進を検出して
  engine/control 通知を落とさないこと。
- `syn.recorder.stop` は部分 finalize 失敗からの retry で
  `AlreadyFinalized`（finalized.json 公開済み）を成功と
  して扱い、`Stopped` への到達を retry 可能にすること。
- `syn.recorder.start` は `loop_start < transport_start`
  の loop 宣言を `invalid_params` で拒否すること
  （session frame が負になり `GateScheduler` が構成
  できないため）。
- `targets[].channels` は wire 上限（256）を超える
  `syn.recorder.start` を `invalid_params` で拒否する
  こと（tap resource の eager 確保が process を abort
  させないため）。
- `syn.project.undo`/`syn.project.redo` は recorder
  session 稼働中（`recording`/`stopped`）に
  `invalid_params` で拒否すること — undo は snapshot
  復元で per-op validation を通らず、recorder の
  session/stream entity を巻き戻して commit を不能に
  し得るため。
- `syn.recorder.discard` は active stream を停止し、
  best-effort で `CloseSession` を適用し、commit せずに
  active runtime state を drop すること（staging file は
  recovery 用に保持）。
- `syn.recorder.status` は owner state・active/session 情報・
  staging root・device 数・fault 情報を返すこと。
- `syn.recorder.mark_take_verified` は
  `RecordingOp::MarkTakeVerified` を適用し take 状態を返す
  こと。
- `syn.app.capabilities` の `recorder` は device 注入が
  あれば available、なければ `unavailable` + reason を
  正直に返し、同じく live owner 投影を含むこと。
- `syn.window.close_guard` は `recorder.active()` が真の
  とき `recorder_active` veto を報告すること（idle 時は
  出さない）。
- `syn.recording.info` は persisted recording entities と
  recorder owner 状態（`status`/`active`/`transaction_open`）
  を同じ投影に含めること。
- device は `project.new`/`project.open` で保持し、active
  recorder session のみ reset すること。
- 変更は `crates/synvelis-app/` と doc chain・evidence・
  sv115 ledger/matrix の pointer 更新に限定すること。

## Acceptance Criteria

- [x] fake `&dyn Device` 注入による headless E2E —
  `UiRuntime` dispatch harness で `syn.recorder.*`
  lifecycle（preflight→open/add→writer/staging→commit→
  `MarkTakeVerified`）を駆動し、`syn.recording.info` の
  live 投影までが `mise run test synvelis-app` で実測
  Pass する（`tests/recorder_wire.rs` 12 test 全て pass）
- [x] `syn.app.capabilities` の `recorder` 状態と
  `syn.window.close_guard` の `recorder_active` が session
  owner の実態と一致する（session active 中に veto が出、
  idle 時に出ない。wire 後も `unavailable`/`idle` 固定の
  まま放置しない）
- [x] `syn.recorder.*` が `syn.recording.*`・
  `syn.monitoring.*` と独立な namespace として `OPS` 一覧と
  dispatch に登録され、既存 op の wire 契約が非回帰
  （`mise run test synvelis-app` 全 suite pass で実測）
- [ ] 実 device 列挙/open・実 GUI-L2・EXT-L3・Issue #114
  の残り AC は NotRun/Deferred を維持し、本 slice で閉じ
  ない（headless Pass を実 OS/実 GUI の AC 達成に転用しない）

## Deferred Design Concerns

- 実 device 列挙/open（`DeviceEnumerator` 経由）と device
  選択 UX | 本 slice は注入 seam まで。実 backend 接続は
  後続 slice・別 gate | device 列挙 slice 着手時 |
  recorder が常に「device なし」で unavailable のままになり
  実運用で使えない |
- 実 GUI-L2（実 GPUI Window での録音 UI 操作）・EXT-L3 |
  headless fake-device の Pass を実 GUI Pass に昇格しない
  原則 | Windows GUI-L2/EXT-L3 gate の有効化時 | headless
  wire のみで #114 の GUI AC を誤って充足したと誤認する |
- `syn.monitoring.*` authored 宣言との統合（arm/punch/
  monitor UX） | RCD-03/#117 系の後続 slice | monitoring
  UX slice 着手時 | recorder session と monitoring の
  session が二重管理になる |
- `syn.recorder.*` の GUI driver（`ui/`）からの実呼び出し |
  Option B の GUI 側 slice で wire 契約を確定する | GUI 側
  slice 着手時 | headless op 面と GUI 呼び出し側の契約が
  ずれる |

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/114>
- 関連: `scarecrow173/synvelis#115`（RCD-01）、
  `scarecrow173/synvelis#117`（RCD-03）
- 調査記録: `docs/discovery/sv114-gui-option-b-recorder-wire.md`
- 設計: `docs/designs/sv114-gui-option-b-recorder-wire.md`
- 検証仕様: `docs/test-specs/sv114-gui-option-b-recorder-wire.md`
- 証拠: `evidence/sv114/`
