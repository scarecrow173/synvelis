---
id: "PLAN-dF3hNzQwErTyUiOpL6kMnB"
type: "plan"
status: "approved"
title: "SF-05 slice3 実施計画 — probe subscriber/capture dedup の app/UI 配線"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/108"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/sf-05-slice3-probe-subscriber-wiring.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-05-slice3-probe-subscriber-wiring-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-04-slice5-probe-subscriber-ownership.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-05-slice3-probe-subscriber-wiring.md"
    - "docs/designs/sf-05-slice3-probe-subscriber-wiring-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-05-slice3-probe-subscriber-wiring.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-05 slice3 実施計画 — probe subscriber/capture dedup の app/UI 配線

## Goal

SF-04 slice5 の session 層 subscriber lease/capture ownership
を app/UI 層へ配線し、SF-UX-06 の headless 側受入
（複数 view の同一 point 共有・view close 解放・owner-only
abort・state/screen 投影）を検証する。

## Scope

`synvelis-app` の `ui_ops.rs`/`screens.rs` + 新規統合 test
`tests/sf05_probe_share.rs`、`ui/src/ipc.ts` の `OpName`、
および docs チェーン。`synvelis-workspace` は既存 API の
消費のみ（`signal_flow_ui.rs`/`lib.rs` は helper 追加が
必要になった場合のみ）。`service.rs`/`app.rs`/`monitoring.rs`
・`synvelis-project` 以下・Ruviform・実 GUI は含まない。

## File Map

| file | 変更 |
| --- | --- |
| `crates/synvelis-app/src/ui_ops.rs` | `op_probe_control` に `subscribe`/`unsubscribe`/`capture_for`/`abort_capture` action、`probe_trace_err` に `NotSubscriber`/`NotCaptureOwner`→`INVALID_PARAMS`、`op_probe_state` に `holders`/`capture_owner`/`subscribers` 投影、`op_view_close`/`close_vcs_view` に `release_view_probe_leases` hook、`capture_screen_state` に probe rows 捕捉、`probe_principal_str`/`signal_flow_probe_rows` helper |
| `crates/synvelis-app/src/screens.rs` | `SignalFlowProbeRow` 型、`MainScreenState.signal_flow_probes` field、`signal_flow_probes_table` component を Signal Flow tab に追加 |
| `crates/synvelis-app/tests/sf05_probe_share.rs` | 新規 — dedup/close release/owner-only abort/state/screen の headless 受入 |
| `ui/src/ipc.ts` | `OpName` に `"syn.probe.control"`/`"syn.probe.state"` 追加 |
| `docs/` チェーン一式 | spec/design/plan/task/test-spec/IR + ledger 訂正 |

## Dependency Graph

```
test file (TDD red)
  └─▶ ui_ops.rs actions + hooks + projection
        └─▶ screens.rs probe rows/table（capture_screen_state 経由）
              └─▶ ipc.ts OpName（独立・同時）
```

workspace 層（`probe_trace.rs`）は変更しない — 既存 API を
そのまま消費する。

## Risk Register

| risk | 対応 |
| --- | --- |
| `op_view_close` の release 失敗で view が半壊 | release を `dock_view_close` 前に置き、失敗時は何も teardown されない。`probe_release_subscriber` は key 毎に整合するため retry 可能 |
| `capture_screen_state` が engine RefCell を二重 borrow | `with_probe_bridge` は `ui_pump` 内の他 borrow と競合しない（op tail で逐次実行）。`op_probe_control` は bridge closure 終了後に `ui_pump` を呼ぶ既存順序を維持 |
| `publish_project_length` が probe rows を wipe | `signal_flow_probes` を `signal_flow` の sibling field にし、publish の対象外にする |
| view-3+（非 dock metadata）の close 経路欠落 | 既存の `dock_domain_view` 制約で `syn.view.close` が拒否されるのは本 slice 以前からの状態。lease は view 生存中は正しく保持される。close 経路追加は別 slice |
| session detach 時の `probe_release_all` 要否 | design で「呼ばない」を決定 — lease は session scope、project 差し替えは generation/stale 機構が処理 |

## Verification

- `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv` / `check-app-native` /
  `lint-app-native` / `check-ui` / `test-ui`
- 新規 test: `cargo test -p synvelis-app --test
  sf05_probe_share`（mise `test` に包含）
- 実 OS/GPUI・実 2-window・#109 oracle: **NotRun**（#114 gate）

## Rollback

単一 commit 系のため revert で戻る。op action 追加は後方
互換（既存 action の wire shape 不変）。`syn.probe.state`
への field 追加は additive で frontend 既存 consumer を
壊さない。
