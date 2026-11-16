---
id: "PLAN-034Yra5r9AfXsdnRO3U7uT"
type: "plan"
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
    - "docs/designs/sv114-gui-option-b-recorder-wire.md"
    - "docs/test-specs/sv114-gui-option-b-recorder-wire.md"
    - "evidence/sv114/"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv114-gui-option-b-recorder-wire.md"
    - "docs/designs/sv114-gui-option-b-recorder-wire.md"
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

# Issue #114 synvelis-app recorder session owner + syn.recorder.* headless wire Implementation Plan

## Goal

`synvelis-app` の `RecorderSlot` stub を recorder session
owner（`RecorderOwner`）に置換し、`syn.recorder.*`
lifecycle ops を headless `UiRuntime` dispatch 経路に配線
して、capabilities・close veto・`syn.recording.info` 投影を
同一 owner の実態に一致させる（Issue #114 Option B slice）。

## Scope

単一の実装ストリーム。変更は `crates/synvelis-app/` の
`src/{recording,app,service,ui_ops}.rs` と
`tests/{recorder_wire,close_veto}.rs` に限定され、同一の
検証 lane（`mise run * synvelis-app`）で一括確認できるため
1 plan とする。

## Upstream Documents

- `docs/specs/sv114-gui-option-b-recorder-wire.md`

## Design Inputs

- `docs/designs/sv114-gui-option-b-recorder-wire.md`

## File Map

- `crates/synvelis-app/src/recording.rs` - `RecorderOwner`
  （authoritative owner: device 注入・active session・
  lifecycle・fault latch・投影）と `RecorderError`・
  `RigInputs` 等の内部構造。`RecorderSlot` 互換 re-export
- `crates/synvelis-app/src/app.rs` - `RecorderSlot` re-export、
  `SynvelisApp.recorder` 型更新、`set_recorder_devices`、
  project new/open reset、`capabilities`/`close_veto_reasons`/
  `recording_info` の実態一致
- `crates/synvelis-app/src/service.rs` - `OPS` 登録と
  `dispatch_recorder` helper
- `crates/synvelis-app/src/ui_ops.rs` - `op_recorder_*`
  実装と wire param parser
- `crates/synvelis-app/tests/recorder_wire.rs` - headless
  E2E（fake `Device`/`InputStream`、`UiRuntime` dispatch）
- `crates/synvelis-app/tests/close_veto.rs` - `large_stack_
  arrays` 是正（`vec![...].into_boxed_slice()`）
- `docs/{discovery,specs,designs,plans,tasks,test-specs,
  impl/ir,impl/exp}/sv114-gui-option-b-recorder-wire*` -
  doc-chain と実装記録（新規）
- `evidence/sv114/` - 検証証拠（新規）
- `evidence/sv115/rcd-01-virtual-l1-oracle-ledger.md`・
  `rcd-01-virtual-l1-oracle-case-matrix.md` - GUI（Option B）
  列の pointer 更新のみ

## Implementation Sequence

- [x] `RecorderSlot` stub と `MonitorUxState` 先例・
  `synvelis-recording` production API・ruviform `Device`/
  `InputStream` port を精読し、owner 配置・namespace・
  device 注入 seam・lifecycle 順序を確定する
- [x] `recording.rs` に `RecorderOwner`/`ActiveRecorder`/
  `RecorderError`/`RigInputs` を実装し、`RecorderSlot` を
  互換 re-export とする
- [x] `app.rs` の `SynvelisApp.recorder` 型更新、
  `set_recorder_devices`、project new/open reset、
  `capabilities`/`close_veto_reasons`/`recording_info` の
  実態一致を実装する
- [x] `service.rs` に `syn.recorder.*` 7 op を `OPS` 登録し
  `dispatch_recorder` helper を実装する
- [x] `ui_ops.rs` に `op_recorder_*` と wire param parser
  を実装する
- [x] `tests/recorder_wire.rs` に fake `Device`/`InputStream`
  と headless E2E（no-device・full lifecycle・discard・
  fault・mark_take_verified・project_new reset・ops 登録
  idle）を実装する
- [x] `mise run check/lint/test/build synvelis-app` で
  実測検証し、clippy 指摘（`too_many_lines`・
  `large_stack_arrays`・`doc_markdown` 等）を是正する
- [x] `mise run check-app-native`/`lint-app-native`/
  `check-app-gpui`/`lint-app-gpui`/`check-headless-deps`/
  `check-ruviform-rev`/`fmt-check`/`prek run --all-files`
  を実行する
- [x] doc-chain・IR/exp・`evidence/sv114/`・sv115 pointer
  更新を作成し commit・push・PR 化する

## Dependencies and Risks

- dependency: なし（pinned ruviform `cbcc8343`・
  `synvelis-recording` API は充足済み、dep 追加なし）
- risk: `RecorderOwner` が `SynvelisApp` を肥大化させ既存
  test の stack array が lint に触れる → `tests/
  close_veto.rs` を `vec![...].into_boxed_slice()` に
  是正（実施済み）
- risk: device open を project mutation 前に置く順序が
  `build_payload` の stream 事前登録前提と矛盾する →
  `InputSession::open`/`add` が返すのは device/stream
  handle であり `project.recording.streams` 登録は
  mutation 段で行うため両立（実測で確認）
- risk: committed take が domain 上 `Verified` に遷移し
  `mark_take_verified` 正系が自明化 → test は domain の
  正直な遷移を pin し、負系（未 commit/不明 id）を typed
  error で検証

## Task Breakdown

- `docs/tasks/sv114-gui-option-b-recorder-wire.md` に全手順
  を集約した単一 task とする（同一 crate・同一検証 lane で
  細分化の利益がない）

## Verification

- [x] `mise run check synvelis-app` / `mise run lint
  synvelis-app` / `mise run test synvelis-app` /
  `mise run build synvelis-app`（全て exit 0。
  `tests/recorder_wire.rs` 12 test 全て pass — test
  一覧は `docs/test-specs/sv114-gui-option-b-recorder-
  wire.md` 参照）
- [x] `mise run check-app-native` / `mise run
  lint-app-native` / `mise run check-app-gpui` / `mise run
  lint-app-gpui`（全て exit 0）
- [x] `mise run check-headless-deps` / `mise run
  check-ruviform-rev`（exit 0。初回 stale vendor を
  `mise run sync-ui-deps` で復旧）
- [x] `mise run fmt-check` / `mise exec -- prek run
  --all-files`（exit 0）
- [x] `git diff` で禁止パス（`Cargo.toml`/`Cargo.lock`/
  `mise.toml`/`.github/**`/`docs/**/README.md`/
  `crates/synvelis-recording/src/**`/`ui/`）が空である
  ことを確認
- [ ] 実 device 列挙・実 GUI-L2・EXT-L3・Issue #114 残 AC
  は merge 後 CI/別 gate の証拠に委譲（本 slice の
  headless Pass を昇格しない）

## Open Questions / Gaps

- 実 device 列挙/open（`DeviceEnumerator`）と device 選択
  UX は後続 slice。
- `syn.monitoring.*` authored 宣言との統合は後続
  （RCD-03/#117 系）。
- `syn.recorder.*` の GUI driver 呼び出しは Option B GUI
  側 slice で確定。

## Self-Review

- [x] spec の要求がすべて task に対応している
- [x] placeholder が残っていない
- [x] file path、relation、status、用語が整合している

## Implementation Handoff

- Recommended mode: inline execution（単一 crate の
  実装で subagent 分割の利益がない）
- Independent work streams: none
- User approval required before dispatch: yes
  （review-protocol のレビューで blocker 0 を確認済みと
  して扱う）
- If approved: discover available implementation and
  delegation capabilities in the current environment,
  then execute task-by-task with review checkpoints.
- If declined or unavailable: execute inline using the
  same dependency graph and verification matrix.

## Review Handoff

- レビュー観点: `RecorderSlot` stub が実 owner に置換され
  `active()`/`status()` が実態を返すこと、`syn.recorder.*`
  が既存 namespace と独立で wire 契約を変えないこと、
  device 失敗が project mutation 前に完了する順序、
  post-mutation 失敗の cleanup、capabilities/close_veto/
  recording_info が同一 owner を読むこと、検証が実測
  ベースであること、禁止パスに差分がないこと。
- upstream: spec / design / test-spec を参照。
