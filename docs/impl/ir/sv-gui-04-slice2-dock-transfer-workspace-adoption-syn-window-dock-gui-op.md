---
id: "IMPL-034UtkIqlxoLTf7Dvq5vMZ"
type: "impl"
status: "completed"
title: "SV-GUI-04 slice2 dock transfer — workspace adoption + syn.window.dock_* GUI op 配線"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/113"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-app/tests/dock_ops.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/adopt.rs"
      - type: "file"
        path: "docs/tasks/sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
      - type: "file"
        path: "docs/test-specs/sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
      - type: "file"
        path: "docs/impl/ir/sv-gui-04-slice2-dock-transfer-workspace-adoption-syn-window-dock-gui-op.md"
    modified:
      - type: "file"
        path: "crates/synvelis-app/src/screens.rs"
      - type: "file"
        path: "crates/synvelis-app/src/service.rs"
      - type: "file"
        path: "crates/synvelis-app/src/shell.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/close_veto.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/view_mount.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/error.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/workspace.rs"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
    - "docs/plans/implement-sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md"
    - "docs/adr/sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
    - "docs/designs/sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md"
    - "docs/adr/sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
    - "docs/designs/sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md"
    - "docs/plans/implement-sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
  derived-by: []
  verifies:
    - "docs/test-specs/sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
  verified-by:
    - "docs/test-specs/sv-gui-04-slice2-dock-transfer-workspace-adoption.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SV-GUI-04 slice2 dock transfer — workspace adoption + syn.window.dock_* GUI op 配線

## Summary

Issue #113 の dock transfer slice。headless で確定済みの
`WorkspaceManager` dock semantics（detach→reattach・move
commit/abort・`WindowCloseDisposition`）を live GUI app へ接続し、
window/view の権威を `WorkspaceManager` 一本に保ったまま
`syn.window.dock_*` op family・dock-aware `syn.view.*`・
`NativeShell` lifecycle 転送・new-window saga を実装した。
`UiRuntime` は window を生成できない（driver は shell 所有）ため、
`syn.window.dock_prepare_new_window` は shell への `WindowRequest`
キュー経由で非同期に window を作り、`Created` payload の
`dock_view` で pending destination を一意に束縛する設計どおりの
構成とした。

## Context

Spec: docs/specs/sv-gui-04-slice2-dock-transfer-workspace-adoption-gui-op.md
ADR: docs/adr/sv-gui-04-slice2-dock-transfer-workspace-adoption.md
Design: docs/designs/sv-gui-04-slice2-dock-transfer-workspace-adoption-design.md
Plan: docs/plans/implement-sv-gui-04-slice2-dock-transfer-workspace-adoption.md
Test spec: docs/test-specs/sv-gui-04-slice2-dock-transfer-workspace-adoption.md
Task: docs/tasks/sv-gui-04-slice2-dock-transfer-workspace-adoption.md

merge 済み設計チェーン（PR #208）で確定した権威分界 — workspace が
dock ownership の唯一の権威、UI registries/bindings と shell の
`views` map は成功した workspace op の投影 — に従い、GUI op 配線を
実装する slice。`synvelis-project`/`synvelis-recording`/
`synvelis-player` と Ruviform 側は対象外。

## Implementation

### Changes

- `crates/synvelis-workspace/src/error.rs`: `WorkspaceError` に
  `DuplicateWindow` variant を追加（adoption seam の重複登録を
  fail-closed で拒否するため）。
- `crates/synvelis-workspace/src/workspace.rs`:
  `adopt_window`（外部生成 window を `veto_close: false`・
  `DetachViews` 既定で登録、host window 生成は呼ばない）と
  `adopt_view`（`ViewInstanceId`/`ViewType`/title を持つ view
  descriptor を dock へ登録）を追加。`set_close_disposition` で
  per-window disposition を更新可能。
- `crates/synvelis-workspace/tests/adopt.rs`（新規）: adoption の
  冪等性・重複拒否・disposition 既定値・detach/reattach 連携を
  8 case で検証。
- `crates/synvelis-app/src/screens.rs`: `DockViewInfo`
  （persisted `ViewInstanceId`・`ViewType`・title・subscription
  targets）と `dock_view_info(view_id)` を追加。`view-1`/`view-2`
  のみ `Some` を返し、それ以外の id は legacy UI-only path に残す
  dock-domain scoping を一箇所に集約。
- `crates/synvelis-app/src/ui_ops.rs`: `DockMove` state
  （`Prepared{token,target}` / `AwaitingWindow{window}`）と
  `WindowRequest` キューを `UiState` に追加。
  `syn.view.attach`/`detach`/`close` を dock-aware 化（初回 attach
  で adopt、owner 変更時は `move_view`、detached pool 経由の
  detach/reattach、close は pending move abort → view 削除 → UI
  teardown の順）。`syn.window.dock_prepare`/`dock_commit`/
  `dock_abort`/`dock_detach`/`dock_reattach`/
  `dock_prepare_new_window`/`syn.window.lifecycle`/`syn.window.list`
  を実装。lifecycle は `created`/`ready` で未登録 window を adopt
  し `dock_view` payload を `AwaitingWindow` の pending
  destination に記録、`destroyed` は `WindowCloseDisposition` を
  適用して orphan pending move を解放し view ごとに disposition
  相当の UI teardown を行う。
- `crates/synvelis-app/src/shell.rs`: `Created`/`Ready` lifecycle
  payload に `dock_view`（saga_windows 由来）を付与。
  `drain_window_requests` が `WindowRequest` を `driver.
  create_window` へ流し `saga_windows` に記録、`drive_saga_ready`
  が destination attach → `syn.window.dock_commit` を実行、
  `abort_saga` が abort dispatch + orphan window 破棄を行う。
  `observe_result` が attach/commit/detach/close/abort の結果を
  shell 側 `views` map に反映し、`on_destroyed` は lifecycle 結果の
  `views`/`disposition` を見て view 単位の後始末を行う。
- `crates/synvelis-app/src/service.rs`: `syn.window.dock_*` 6 op と
  `syn.window.lifecycle`/`syn.window.list` を `OPS` に登録。
- `crates/synvelis-app/tests/dock_ops.rs`（新規）: FakeDriver +
  NativeShell fixture で test-spec 全 case を検証（下記
  Validation）。
- `crates/synvelis-app/tests/close_veto.rs`・`tests/view_mount.rs`:
  window destroy 時の view 挙動を旧「destroy で view close」から
  新既定 `DetachViews`（pool 退避・保持）契約へ更新。

### Decisions During Implementation

- **`AwaitingWindow` の例外 attach は pending destination の
  window id 一致のみ許可**: 初版は「owner 以外の window 名を
  destination とみなす」緩い判定で、無関係 window への stray
  attach を受理してしまった。shell が `create_window` 直後に
  `saga_windows` を持つため、`Created` payload の `dock_view`
  で app に正確な destination `WindowId` を先に知らせ、
  `dock_view_attach` を `pending == Some(win)` の完全一致に
  修正した（設計の「pending destination のみ UI-first attach
  可」契約どおり）。
- **saga window の識別は lifecycle payload の `dock_view` で行う**:
  `UiRuntime` から shell への直接経路は無く、app 側が
  destination を知る正規の境界は `syn.window.lifecycle` op。
  `created`/`ready` 双方で `dock_view` を転送し、`created` 時点で
  `AwaitingWindow.window` を確定させることで `ready` 前の stray
  attach を `invalid_params` で fail-closed にした。
- **destroyed 処理は disposition 別の UI teardown を view ごとに
  実行**: workspace 側は `DetachViews` で pool 退避/
  `ReleaseViews` で registry/state 除去を行うが、UI gateway・
  app registry・subscription は app 層の責務。`op_view_detach`/
  `op_view_close` をそのまま再利用し、workspace op と同一の
  権威経路で後始末することで二重権威を避けた。
- **orphan pending move の解放は destroyed 処理内で一括**:
  `Prepared`（destination 死亡 or token 消滅）と
  `AwaitingWindow`（記録済み destination 死亡 or source view が
  その destroy で pool 入り）は view lock を漏らさないよう
  `syn.window.state` notify 付きで除去する。
- **legacy view id（`console`/`meters`/`transport` 等）は dock
  domain に入れない**: `dock_view_info` が `view-1`/`view-2` のみ
  `Some` を返す構造にし、`syn.view.*`/`syn.window.dock_*` の
  dock-domain 判定をこの 1 関数に集約した。
- **`op_window_lifecycle` は `destroyed` arm を
  `op_window_lifecycle_destroyed` に分離**: clippy
  `too_many_lines`（閾値 100 行）対応として、disposition 適用・
  orphan 解放・UI teardown を担う destroyed 経路を独立メソッドに
  切り出した。責務境界（adopt vs destroy）とも一致する。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

`mise run` 系（正本）— すべて exit 0、`origin/main`（`e72bb4b`、
PC-C5 #211 / RCD-02 #210 / PC-C3-05 #209 merge 済み）取り込み後の
merged tree で実行:

- `mise run fmt-check`: exit 0
- `mise run check`: exit 0
- `mise run lint`: exit 0
- `mise run test`: exit 0 — `tests/dock_ops.rs` 12 件全 pass
  （existing-window prepare/commit・prepare/abort・destination
  destroy fail-closed・detach/reattach round trip・
  `syn.view.detach`/`attach` pool 経路・new-window 作成失敗・
  saga commit on ready・destination 早期 destroy abort・legacy id
  UI-only・`DetachViews`/`ReleaseViews`・adoption 冪等/
  `syn.window.list`・Project/transport state 保持）、
  `synvelis-workspace/tests/adopt.rs` 8 件全 pass、既存
  `sv113_dock.rs`・`close_veto.rs`・`two_window.rs`・
  `view_mount.rs`・`shell.rs` 等 非回帰（PC-C5 `rel_c5.rs` 11 件も
  merge 後に pass）
- `mise run check-app-native`: exit 0
- `mise run lint-app-native`: exit 0
- `mise run build-app-native`: exit 0

補助証拠（canonical = mise）: `cargo test -p synvelis-app
--test dock_ops` 単独実行も 12/12 pass。

NotRun: 実 OS での window/dock 操作証拠は #114 gate のため本
slice 対象外（FakeDriver/Headless 検証を native 実 OS 証拠へ昇格
しない）。`run-app-native` 実窓起動・`SYNVELIS_SECOND_WINDOW`
経路の dock 操作 smoke は未実施（build のみ検証）。monitor 失踪
layout restore（#177）・DPI/IME（RF#388/#389/#114）・Plugin Dock
（#176）は本 slice 対象外。

review-protocol 自己レビュー実施: stray attach 受理（P1 相当）を
`dock_view` pending destination 一致で修正、destination destroy
test の FakeDriver bookkeeping 不整合を `driver.destroy_window`
経路へ修正、clippy `too_many_lines`/`assert-is-empty`/
`double_ended_iterator_last`/`doc_markdown`/`single_match_else`/
`missing_panics_doc` をすべて解消。merge conflict（screens.rs
import union、PC-C5 relation inspector は view 内コンテンツで
新規 view id を増やさないため `dock_view_info` scoping に影響
なし）を解消済み。

## Risks

- `AwaitingWindow` saga は shell の `saga_windows` と app の
  `DockMove` の 2 箇所で destination を追跡する — lifecycle
  `created`/`ready` の payload 転送が唯一の同期点であり、
  event 欠落時は `AwaitingWindow.window=None` のまま残り
  attach 不能になる（fail-closed で stray は拒否されるが、
  commit も進まない）。window 生成失敗・destination 早期 destroy
  は test でカバー済みだが、OS が `Created` を配送しない異常系は
  native 証拠待ち。
- `syn.view.attach` の dock-domain 判定は `view-{n}` parse +
  `dock_view_info` metadata 存在の複合条件 — 将来 composition
  root が新しい `view-N` を追加する場合は `dock_view_info` への
  登録が必須（未登録 id は legacy path に落ちるため誤 adopt は
  起きない）。
- `op_window_lifecycle_destroyed` の per-view UI teardown は
  `op_view_detach`/`op_view_close` の失敗を warn log に留める —
  workspace 側 disposition は確定済みのため UI 側の不整合は
  残り得るが、teardown 失敗は実害のある経路では観測されていない。

## Follow-ups

- 実 OS での dock transfer・window close 証拠は #114 gate で採取
  （Fake/Headless と Native を別 Capability として記録）。
- `SYNVELIS_SECOND_WINDOW` env-gated 実測での dock 操作 smoke は
  native 環境での別 dispatch 候補。
- monitor 失踪 layout restore（#177）・Plugin Dock（#176）・
  DPI/IME（RF#388/#RF389）は別 Issue の責務。
