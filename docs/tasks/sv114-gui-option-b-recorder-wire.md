---
id: "TASK-034Yraj4Zj1N5X2FqPuPNs"
type: "task"
status: "done"
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
    - "docs/plans/sv114-gui-option-b-recorder-wire.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sv114-gui-option-b-recorder-wire.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv114-gui-option-b-recorder-wire.md"
    - "docs/designs/sv114-gui-option-b-recorder-wire.md"
    - "evidence/sv114/recorder-wire-option-b-headless.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sv114-gui-option-b-recorder-wire.md"
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
---

# Issue #114 synvelis-app recorder session owner + syn.recorder.* headless wire

## Work

`synvelis-app` の `RecorderSlot` stub（`active()`/`status()`
が定数を返す）を、preflight 確定 plan・`InputSession`・
staging/writer・fault latch・session metadata を保持する
`RecorderOwner`（`recording.rs`）に置換する。
`RecorderSlot` は `pub use crate::recording::RecorderOwner
as RecorderSlot;` の互換 re-export とし、`SynvelisApp.
recorder` の型を更新する。`set_recorder_devices` で host
注入 `Device` を受け、`project.new`/`project.open` で
active session のみ reset する（device は保持）。

`syn.recorder.*` lifecycle ops（`start`/`pump`/`stop`/
`commit`/`discard`/`status`/`mark_take_verified`）を
`service.rs` の `OPS` テーブルと dispatch（
`dispatch_recorder` helper）に登録し、`ui_ops.rs` に op
実装と wire param parser を追加する。`capabilities` の
`recorder`、`close_veto_reasons` の `recorder_active`、
`recording_info` の `recorder` ブロックが同一 owner を
読む実態一致を実現する。

headless E2E として `tests/recorder_wire.rs` に test-local
fake `Device`/`InputStream`（deterministic PCM、`MockPcm`）
を実装し、`UiRuntime::dispatch_op` + `Rc<RefCell<
AppService>>` + `SharedService` delegation で実 dispatch
経路を駆動する。`tests/close_veto.rs` は `RecorderOwner`
による `SynvelisApp` サイズ増で発火した
`clippy::large_stack_arrays` を `vec![...].into_boxed_
slice()` で是正する。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/sv114-gui-option-b-recorder-wire.md`
- `docs/specs/sv114-gui-option-b-recorder-wire.md`
- `docs/designs/sv114-gui-option-b-recorder-wire.md`
- `docs/test-specs/sv114-gui-option-b-recorder-wire.md`
- `docs/discovery/sv114-gui-option-b-recorder-wire.md`

## Dependencies

- なし（pinned ruviform `cbcc8343`・`synvelis-recording`
  production API は充足済み、dep 追加なし）

## Affected Files

- `crates/synvelis-app/src/recording.rs`（`RecorderOwner`/
  `ActiveRecorder`/`RecorderError`/`RigInputs` 実装）
- `crates/synvelis-app/src/app.rs`（`RecorderSlot` re-export、
  `SynvelisApp.recorder` 型、`set_recorder_devices`、
  reset、capabilities/close_veto/recording_info）
- `crates/synvelis-app/src/service.rs`（`OPS` 登録 +
  `dispatch_recorder`）
- `crates/synvelis-app/src/ui_ops.rs`（`op_recorder_*` +
  wire param parser）
- `crates/synvelis-app/tests/recorder_wire.rs`（新規
  headless E2E）
- `crates/synvelis-app/tests/close_veto.rs`
  （`large_stack_arrays` 是正）
- `docs/{discovery,specs,designs,plans,tasks,test-specs,
  impl/ir,impl/exp}/sv114-gui-option-b-recorder-wire*`（新規
  doc-chain）
- `evidence/sv114/`（新規 dir、検証証拠）
- `evidence/sv115/rcd-01-virtual-l1-oracle-ledger.md`・
  `rcd-01-virtual-l1-oracle-case-matrix.md`（GUI（Option B）
  pointer 更新のみ）

## Done When

- [x] `RecorderSlot` stub が `RecorderOwner` に置換され、
  `active()`/`status()` が session 実態を返す
- [x] `syn.recorder.*` 7 op が `OPS` と dispatch に登録され、
  lifecycle（preflight→open/add→writer/staging→start→
  pump→stop→commit/discard→`mark_take_verified`）が
  `UiRuntime` dispatch で駆動できる
- [x] `syn.app.capabilities`/`syn.window.close_guard`/
  `syn.recording.info` が同一 owner の実態を投影する
- [x] `mise run test synvelis-app` で `tests/recorder_wire.rs`
  の 12 test が全て pass し、既存 suite が非回帰
- [x] `mise run check/lint/build synvelis-app`・
  `check-app-native`/`lint-app-native`/`check-app-gpui`/
  `lint-app-gpui`/`check-headless-deps`/`check-ruviform-rev`/
  `fmt-check`/`prek run --all-files` が pass
- [x] 禁止パス（`Cargo.toml`/`Cargo.lock`/`mise.toml`/
  `.github/**`/`docs/**/README.md`/`crates/synvelis-
  recording/src/**`/`ui/`）に差分がない
- [ ] 実 device 列挙・実 GUI-L2・EXT-L3・Issue #114 残 AC
  は NotRun/Deferred を維持（merge 後 CI/別 gate に委譲）

## Verification

- [x] `mise run check synvelis-app` / `mise run lint
  synvelis-app` / `mise run test synvelis-app` /
  `mise run build synvelis-app`（全て exit 0。`tests/
  recorder_wire.rs` 12 test 全て pass を実測）
- [x] `mise run check-app-native` / `mise run
  lint-app-native` / `mise run check-app-gpui` / `mise run
  lint-app-gpui`（全て exit 0）
- [x] `mise run check-headless-deps` / `mise run
  check-ruviform-rev`（exit 0）
- [x] `mise run fmt-check` / `mise exec -- prek run
  --all-files`（exit 0）
- [x] `git diff` で禁止パスが空であることを確認
- [x] `cargo tree`/`cargo check --locked --all-targets`
  は補助証拠（canonical = mise）
