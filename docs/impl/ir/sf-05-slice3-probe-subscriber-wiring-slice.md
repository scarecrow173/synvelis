---
id: "IMPL-sf05s3probeShareWiring01"
type: "impl"
status: "in-progress"
title: "SF-05 slice3 probe subscriber/capture dedup の app/UI 配線"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/108"
  changes:
    added:
      - type: "file"
        path: "docs/specs/sf-05-slice3-probe-subscriber-wiring.md"
      - type: "file"
        path: "docs/designs/sf-05-slice3-probe-subscriber-wiring-design.md"
      - type: "file"
        path: "docs/plans/sf-05-slice3-probe-subscriber-wiring-plan.md"
      - type: "file"
        path: "docs/tasks/sf-05-slice3-probe-subscriber-wiring.md"
      - type: "file"
        path: "docs/test-specs/sf-05-slice3-probe-subscriber-wiring.md"
      - type: "file"
        path: "docs/impl/ir/sf-05-slice3-probe-subscriber-wiring-slice.md"
      - type: "file"
        path: "crates/synvelis-app/tests/sf05_probe_share.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "ui/src/ipc.ts"
      - type: "file"
        path: "evidence/reports/sf-06-cross-gate-ledger.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/sf-05-slice3-probe-subscriber-wiring.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-05-slice3-probe-subscriber-wiring.md"
    - "docs/designs/sf-05-slice3-probe-subscriber-wiring-design.md"
    - "docs/test-specs/sf-05-slice3-probe-subscriber-wiring.md"
    - "docs/plans/sf-05-slice3-probe-subscriber-wiring-plan.md"
    - "docs/impl/ir/sf-04-slice5-probe-subscriber-window-ownership.md"
    - "docs/impl/ir/sf-05-slice1-signal-flow-headless-hierarchy-visual-only.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SF-05 slice3 probe subscriber/capture dedup の app/UI 配線

## Summary

Issue #108 (SF-05) slice 3: SF-04 slice5（merge 済み、
PR #304）が `synvelis-workspace::ControlSession` に確立した
subscriber lease / capture owner モデルを、app 層 IPC op と
Signal Flow 画面へ配線した。`syn.probe.control` に
`subscribe`/`unsubscribe`/`capture_for`/`abort_capture`
action を追加し、`syn.probe.state` に lease holders /
capture owner / per-view subscriber keys を投影した。
view close（`op_view_close`・VCS view close）で当該 view の
lease を解放する hook を追加し、Signal Flow tab に
probe subscription 行の read-only 表示を追加した。
`ui/src/ipc.ts` の `OpName` に `syn.probe.control` /
`syn.probe.state` を登録した。lease/dedup/capture owner の
不変量自体は workspace 層が所有し、本 slice はその消費側
境界のみを実装する。新規 headless test
`crates/synvelis-app/tests/sf05_probe_share.rs`（6 件）で
2 view 共有・冪等 subscribe・view close lease 解放・
owner-only abort・capture owner retreat・screen 投影を検証。

## Context

`docs/tasks/sf-05-slice3-probe-subscriber-wiring.md` に
基づく。上流は SF-05 slice1/2（read model・Port Drag・
inspection focus・ViewState persist）と SF-04 slice5
（session 層 subscriber lease）で、残件は app/UI 層の
消費配線のみだった。`service.rs`/`app.rs`/`monitoring.rs`
は他 Worker 占有の可能性があるため変更せず、既存
`syn.probe.control`/`syn.probe.state` op への action 追加
設計とした（新 op 登録なし）。

## Implementation

### Changes

- `crates/synvelis-app/src/ui_ops.rs`:
  - `op_probe_control` に view-scoped action を追加:
    `subscribe`（`view_id`+`target`+`point`+`kind`+`mode` →
    `ControlSession::probe_subscribe`）、`unsubscribe`
    （`view_id`+`key` → `probe_unsubscribe`）、
    `capture_for`（`view_id`+`key`+`max_frames` →
    `probe_capture_for`）、`abort_capture`（`view_id`+`key`
    → `probe_abort_capture`）。`view_id` は
    `sf_view_instance` で論理 `ViewInstanceId` に解決し、
    detach/move では解放せず close のみ解放する契約に従う。
  - `probe_trace_err`: `ProbeTraceError::NotSubscriber` /
    `NotCaptureOwner` を `INVALID_PARAMS` に写像
    （caller/authorization 失敗）。
  - `op_probe_state`: per-binding `holders`・
    `capture_owner`・`subscribers`（view→keys 投影）を追加。
    engine 未 attach 時は従来どおり空/unattached を真実に投影。
  - `release_view_probe_leases` helper を追加し、
    `op_view_close` と VCS view close で dock/UI teardown
    前に `probe_release_subscriber(inst)` を呼ぶ。
    provider 失敗時は view を残して close を retry 可能に
    する fail-closed。
  - `signal_flow_probe_rows`: main 論理 view の lease と
    共有 engine/session 状態から `SignalFlowProbeRow` を投影。
- `crates/synvelis-app/src/screens.rs`:
  - `SignalFlowProbeRow` 型と `MainScreenState` の
    `signal_flow_probes` フィールドを追加。
  - Signal Flow tab に `signal_flow_probes_table` を追加。
    行は point/kind/requested mode/binding state/holders/
    capture owner を表示する read-only 投影で、project/DSP
    state を変更しない。
- `crates/synvelis-app/tests/sf05_probe_share.rs`（新規）:
  6 件の headless test — engine 無し fail-closed、2 view が
  1 key/provider binding を共有、冪等 subscribe、view close
  が自 view の lease のみ解放、明示 unsubscribe と非 holder
  拒否、owner-only abort + owner 喪失時の Session retreat、
  Signal Flow screen の probe 投影。
- `ui/src/ipc.ts`: `OpName` union に `syn.probe.control` /
  `syn.probe.state` を追加（probe ops が全欠落していた）。
- `evidence/reports/sf-06-cross-gate-ledger.md`:
  SF-UX-02/03/07 の T2 stale 行を slice2 merge 済みの
  Pass*（headless 部分）へ訂正。
- `docs/` slice3 文書チェーン 5 件 + 各 index README 再生成。

### Decisions During Implementation

- **既存 op への action 追加**: `syn.probe.control` は
  session action（`request`/`set_mode`/`capture`/
  `stop_capture`/`remove`）を既に持つため、view-scoped
  action を同じ op に追加し、新 op 登録（`service.rs`
  変更 = 範囲外・他 Worker 占有の可能性）を回避した。
- **view 解決は `sf_view_instance` 経由**: `view_id`
  （`"view-{n}"`）を `ViewInstanceId` に解決する際、
  window ではなく論理 view を指す既存 helper を再利用し、
  ViewInstanceId 安定契約（detach/move で解放しない）と
  整合させた。
- **close 時の lease 解放を dock teardown 前に配置**:
  provider 失敗で view が残り close を retry できる
  fail-closed。`probe_release_subscriber` は当該 view の
  全 lease を解放し、最後の lease 解放時のみ provider
  remove が走る（workspace 層契約をそのまま消費）。
- **capture owner retreat は workspace 契約に委譲**:
  owner view が lease を手放しても他 holder が残る場合の
  `Session` への retreat は `probe_trace.rs` が所有する
  不変量であり、app 層は `probe_release_subscriber` を
  呼ぶだけで正しい挙動になる（test で実証）。
- **screen 投影は read-only**: Signal Flow probe 行は
  `syn.probe.state` の投影をそのまま表示し、toggle/編集
  affordance は project/DSP state に触れない。実 pointer
  経路・実 GUI は #114 gate 側の範囲。

## Related Experiments

### Adopted

<!-- none -->

### Rejected

<!-- none -->

## Validation

- `mise run fmt-check`: pass（exit 0）
- `mise run check`（cargo check --workspace --all-targets）:
  pass（exit 0）
- `mise run lint`（cargo clippy --workspace --all-targets --
  -D warnings）: pass（exit 0）。初回実行で
  `needless_pass_by_value`（`probe_trace_err`）と
  `too_many_lines`（`op_probe_control`）を検出し、
  `probe_trace_err` を参照受け取り化 + view-scoped action を
  `probe_control_view_scoped` helper へ分離して修正。
  test file の `doc_markdown`/`ref_option` も修正。
- `mise run test`（cargo test --workspace --locked）:
  pass（exit 0）。`sf05_probe_share` 6/6 を含む全件 pass。
- `mise run test-rt`（cargo test --workspace --locked rt_）:
  pass（exit 0）
- `mise run test-msrv`（rust@1.89.0 cargo test --workspace
  --locked）: pass（exit 0、`sf05_probe_share` 6/6 含む）
- `mise run check-app-native`（cargo check -p synvelis-app
  --features native --all-targets）: pass（exit 0）
- `mise run lint-app-native`（cargo clippy -p synvelis-app
  --all-targets --features native -- -D warnings）:
  pass（exit 0）
- `mise run check-ui`: pass（exit 0）。初回は stale
  `ui/vendor/ruviform`（vendored=20a24a66 vs pin=b02942c2）
  で fail-closed → `mise run sync-ui-deps` で lock rev へ
  再同期後 pass（synvelis#230 の既知手順、生成物は
  gitignore 対象で commit 対象外）。
- `mise run test-ui`（vitest）: pass（exit 0、56 tests）
- 補助証拠（canonical = mise）: `cargo test -p
  synvelis-app --test sf05_probe_share` 6/6 pass、
  `cargo check -p synvelis-app --all-targets` pass。
- OS: Windows（worktree `sv108-probe`）、toolchain:
  rustc/cargo 1.99.0（pin）、MSRV 検証は rust 1.89.0。
- `git fetch origin main && git merge origin/main`:
  Already up to date（base 13e6518 のまま）。
- review-protocol: 実装 review 実施（結果は PR 本文に記録）。

## Risks

- `syn.probe.state` の `subscribers` 投影は view→keys の
  全量を返す。view 数・key 数が増えた場合の wire サイズは
  bounded ではないが、headless slice の範囲では問題になら
  ない（pagination/差分投影は将来の UI 最適化課題）。
- session detach（engine detach/project open）時の
  `probe_release_all` は本 slice では未配線 — view lease は
  session と共に破棄されるため実害はないが、明示的な
  teardown 経路が必要になれば後続で検討。
- `ui/src/ipc.ts` は型登録のみで、実 UI 側の probe 操作
  呼び出しは本 slice 範囲外（headless op 消費の配線まで）。

## Follow-ups

- SF-UX-08 / #114 gate: 実 OS/GPUI・実 2-window GUI での
  probe 共有・capture owner・close 挙動の受入（NotRun 継続）。
- #109 oracle 結合: probe state の evidence oracle 連携。
- 実 UI からの `syn.probe.control` subscribe/unsubscribe
  呼び出し（Signal Flow probe toggle の pointer 経路）。
- session detach 時の `probe_release_all` 要否の再検討。
- `dock_view_info` 未登録の adopt 済み `view-N`（main/secondary
  以外）は `syn.view.close` が dock metadata 不在で拒否する
  既存制約により lease を close 経路で解放できない — 該当
  view 自体が従来 close 不能なため本 slice の新規欠陥では
  ないが、view 登録が拡張された際に lease 解放経路の
  再確認が必要（review-protocol P2 指摘・非 blocking）。
