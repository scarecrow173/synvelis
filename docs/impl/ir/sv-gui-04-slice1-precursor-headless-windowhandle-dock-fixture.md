---
id: "IMPL-034UoCucc7Cxrp4NwMxb5V"
type: "impl"
status: "completed"
title: "SV-GUI-04 slice1 precursor: headless WindowHandle + dock fixture"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/113"
  changes:
    added:
      - "crates/synvelis-app/tests/sv113_dock.rs"
      - "docs/specs/sv-gui-04-01-headless-window-handle-dock.md"
      - "docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md"
      - "docs/plans/sv-gui-04-01-headless-window-handle-dock.md"
      - "docs/tasks/sv-gui-04-01-headless-window-handle-dock.md"
      - "docs/test-specs/sv-gui-04-01-headless-window-handle-dock.md"
    modified:
      - "crates/synvelis-app/src/host.rs"
      - "crates/synvelis-app/src/lib.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/sv-gui-04-01-headless-window-handle-dock.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-04-01-headless-window-handle-dock.md"
    - "docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md"
    - "docs/plans/sv-gui-04-01-headless-window-handle-dock.md"
    - "docs/test-specs/sv-gui-04-01-headless-window-handle-dock.md"
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

# SV-GUI-04 slice1 precursor: headless WindowHandle + dock fixture

## Summary

Issue #113（SV-GUI-04 Multiwindow Workspace/Dock）の precursor
スライスとして、`synvelis-app` に multi-window 対応の headless
`WindowHandle` 実装 `HeadlessWindowHost` を追加し、
`WorkspaceManager` の dock 契約と `NativeShell` の view-origin
検証を OS 無しで通せる fixture test `tests/sv113_dock.rs` を
整備した。`ShellWindowHost` の単一 window 契約
（`create_window` → `BackendUnavailable`）は不変のまま維持した。

## Context

`docs/tasks/sv-gui-04-01-headless-window-handle-dock.md` /
`docs/plans/sv-gui-04-01-headless-window-handle-dock.md` /
`docs/specs/sv-gui-04-01-headless-window-handle-dock.md` /
`docs/designs/sv-gui-04-01-headless-window-handle-dock-design.md` /
`docs/test-specs/sv-gui-04-01-headless-window-handle-dock.md` の
チェーンに従う。app 層（`screens.rs`/`shell.rs`/`ui_ops.rs`/
`app.rs`/`service.rs`/`bin/`）は別 dispatch が編集中のため本
スライスでは触れず、headless 契約層のみを対象とした。

## Implementation

### Changes

- `crates/synvelis-app/src/host.rs`: `HeadlessWindowHost` を追加。
  `WindowHandle` 全 11 メソッドを実装し、`next: u64` の単調採番で
  nonzero・非再利用の `WindowId` を発行、`live: BTreeSet<WindowId>`
  で生存集合を管理。`created`/`evals`/`resizes`/`focuses`/
  `close_requests`/`close_decisions`/`destroys`/`monitors`/`exited`
  の公開記録フィールドと `fail_create` 注入・`with_monitors`
  コンストラクタを提供。fallible 操作は未知 window に
  `UiError::Host`、fire-and-forget（`eval`/`request_close`）は
  live window のみ記録して未知宛先を silently drop。
  `respond_close(Approve)` は live から除去して `destroys` に記録、
  `Veto`/`Defer` は維持。
- `crates/synvelis-app/src/lib.rs`: `HeadlessWindowHost` を
  `ShellWindowHost` と並べて pub re-export（additive 1 行）。
- `crates/synvelis-app/tests/sv113_dock.rs`: 新規 fixture。
  `WorkspaceManager<HeadlessWindowHost>` で window identity・
  共有 project・window destroy 耐性・独立 view state/focus・
  transfer commit/abort・`detach_to_new_window` を検証し、
  `NativeShell`+`FakeDriver` で cross-window `origin_view` の
  `invalid_params` 拒否と同一 window 受理を検証。

### Decisions During Implementation

- `synvelis_workspace::host::FakeWindowHost` ではなく app 層に
  独自型を置いた — app → workspace の依存方向を保ち、app 層の
  host port 契約を app crate が所有するため。
- `eval` は未知/破棄済み window 宛を記録せず drop する実装に
  した — `WindowHandle` の delivery safety contract
  （"silently dropped"）と spec/design/test-spec の記述に一致
  させるため。`FakeWindowHost` は無条件記録だが、こちらは
  「到達した配送」のみを記録する方が契約として正確。
- `respond_close` は pending close request の有無をモデル化
  しない（live window への decision は記録して受理）。trait の
  厳密な "no pending close request → UiError::Host" には
  従わないが、`FakeWindowHost` と同じ寛容さで、spec の要求
  範囲（Approve 除去・Veto/Defer 維持）を満たす。
- `SynvelisApp.workspace` の host 型は `ShellWindowHost` のまま
  — app.rs は別 dispatch の編集範囲であり、本スライスでは
  `HeadlessWindowHost` を `WorkspaceManager` 直組み立ての
  検証経路専用とした。

## Related Experiments

### Adopted

<!-- なし — known-solution の機械的実装のため exp は未作成。 -->

### Rejected

<!-- なし。 -->

## Validation

- `mise run fmt-check` — Pass
- `mise run check` — Pass
- `mise run lint` — Pass（`cargo clippy --workspace --all-targets
  -- -D warnings`）
- `mise run test` — Pass（`cargo test --workspace --locked`、
  `sv113_dock.rs` 5 test 全 Pass を含む workspace 全体）
- review-protocol: objective checks（上記 mise 4 本）実施後、
  spec/design/test-spec と diff の突合レビューを実施。
  CONFIRMED 2 件（`eval` の未知 window 記録 → drop に修正、
  `fail_create`/`with_monitors`/close flow の未検証 → assert
  追加）を修正済み。`respond_close` の pending 非モデル化は
  `FakeWindowHost` 先例と spec 範囲に基づき non-blocking と判定。
  P0/P1 残存なし → ACCEPT。

## Risks

- headless fixture が実 GPUI backend と乖離した semantics を
  固定するリスク — test は公開契約のみを assert し、id 採番の
  具体値等の実装詳細には依存しない設計で緩和。
- `HeadlessWindowHost` と `synvelis_workspace::FakeWindowHost`
  の semantics 近接による将来の二重保守 — app 層契約として
  意図的に分離。実 backend 実装時に差異が出た場合は本型を
  参照実装として調整する。

## Follow-ups

- Issue #114: 実 OS multiwindow/focus/IME/window-close gate。
- 後続 app 層スライス: `SynvelisApp` の host 型切替・
  `synvelis-native` での 2 窓 mount・`screens.rs` 2nd screen・
  `create_view_window` の実 backend 接続（別 dispatch）。
- `WindowCloseDisposition::RelocateTo`・close veto・monitor
  失踪・layout restore の headless 検証拡充。
