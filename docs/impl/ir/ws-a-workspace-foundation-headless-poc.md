---
id: "IMPL-034QxsUh3NmPwprwJkrlk5"
type: "impl"
status: "completed"
title: "WS-A workspace foundation headless PoC"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/36"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-workspace/Cargo.toml"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/error.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/view.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/state.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/focus.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/link.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/command.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/session.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/workspace.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/host.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/persist.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/ws_a.rs"
    modified:
      - type: "file"
        path: "Cargo.toml"
      - type: "file"
        path: "Cargo.lock"
      - type: "file"
        path: "crates/synvelis-render/tests/x_e2e.rs"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/adr/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
  implements:
    - "docs/plans/ws-a-workspace-foundation.md"
    - "docs/tasks/ws-a-task1-crate-skeleton-deps-view-state-focus.md"
    - "docs/tasks/ws-a-task2-linkgroup-mapping-session.md"
    - "docs/tasks/ws-a-task3-command-router.md"
    - "docs/tasks/ws-a-task4-workspace-manager-dock-host-persist.md"
    - "docs/tasks/ws-a-task5-acceptance-tests.md"
    - "docs/tasks/ws-a-task6-verification-merge-impl-record.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
    - "docs/designs/ws-a-workspace-foundation-design.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
    - "docs/designs/ws-a-workspace-foundation-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/ws-a-e2e-workspace-foundation.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# WS-A workspace foundation headless PoC

## Summary

Issue #36 の Workspace 基盤 headless PoC を `crates/synvelis-workspace`
として実装した。`ruviform-ui` の `DockWorkspace`/`WindowHandle`
契約(#181/#182 を含む `94c5bf8` で固定)を消費し、Project 正本は
既存 `ProjectController` 一箇所。WS-A-01..06 の受入テスト 8 件を含む。

## Context

Spec: docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md
ADR: docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md
Design: docs/designs/ws-a-workspace-foundation-design.md
Plan: docs/plans/ws-a-workspace-foundation.md / Test spec:
docs/test-specs/ws-a-e2e-workspace-foundation.md。

## Implementation

### Changes

- `view.rs`: `ViewType` (Arrangement/Mixer/Inspector/SignalFlow)、
  `ViewDescriptor`、`ViewRegistry`、`ViewFactory`→`HeadlessView`
  (render 回数・最終 revision を記録する headless stub)。
- `state.rs`: `SemanticTarget` (Track/Lane/Clip/Source)、
  `EditingSelection`・`InspectionFocus` (Follow/Pinned)・
  `LinkedContext`・`Viewport` を分離した `ViewState`、
  `ViewStateRegistry` (view id キー、dock 位置に非依存)。
- `focus.rs`: `InputContext` (View/TextInput/Ime/PluginGui/Modal)、
  `FocusState` (native window focus / active view / input context の
  三概念分離)。
- `link.rs`: `LinkGroupId`/`LinkEvent`/`LinkGroupManager`。
  `map_to_track` が clip→lane→track を project 解決する semantic
  mapping。mapping 不能なら publish 自体しない。受信側は
  `deliver` のみで再発信なし、`(receiver, event_id)` dedup、
  Group 越境拒否。
- `command.rs`: `CommandKind` 5 分類、`CommandContext` (origin
  window/view・input context・explicit targets・selection
  snapshot・expected revision)、`CommandOutcome`
  (Applied/Suppressed/Unresolved/Rejected)、`spec_for`/
  `build_project_op`。
- `session.rs`: `ProjectSession` (唯一の `ProjectController`)、
  `TransportState` stub、`EngineStub` (revision 変化でのみ
  `graph_builds` 増加)。
- `workspace.rs`: `WorkspaceManager` — window 台帳 + dock 操作
  (detach_to_new_window・move/begin/commit/abort・reattach) +
  host event 処理 (Focus→active view 導出、CloseRequested veto/
  approve、Destroyed→disposition) + `select` (selection→link
  publish/deliver) + `dispatch` (command routing) +
  capture/restore。
- `host.rs`: `FakeWindowHost` — `WindowHandle` の決定論的実装
  (順次 WindowId、全操作記録、`fail_create` 失敗注入)。
- `persist.rs`: `WorkspaceSnapshot` envelope — `LayoutSnapshot` +
  window titles(順序対応) + view descriptors/state + link
  assignments。schema version 検証、破損 layout は
  `restore_or_default` の安全既定へ。
- `tests/ws_a.rs`: WS-A-01..06 受入テスト 8 件。
- `Cargo.toml`: member 追加 + `ruviform-ui` git dep(既定ブランチ
  追従、SHA 非固定)。`Cargo.lock`: 全 ruviform source が
  `94c5bf81fc63d1e1f1ce58db6d93806339bf879e` へ統一(#181/#182 祖先)。
- `x_e2e.rs`: clippy `assert-is-empty` 適合の 1 行修正(既存 lint
  失敗の解消)。

### Decisions During Implementation

- `SemanticTarget` は externally-tagged serde(内部 tag は newtype
  variant の string payload と両立しない — テストで発見し修正)。
- WindowId は host 発行かつ `LayoutSnapshot` に key がないため、
  window title は capture 順序で `windows[i]` と対応付ける。
- `detach_to_new_window` は「create→register→move」の順(未登録
  window は `begin_move` 不可)。create 失敗は何も汚さず、move 失敗
  時は作った window を破棄して孤児を残さない。
- `resolve_target` は explicit targets→origin view snapshot→origin
  view の現行 selection のみを読む。他 view への暗黙 fallback は
  構造的に存在しない。
- `view.activate` の target 解決は origin view 自身(View 種別の
  ため Entity 解決に落とさない)。
- review で検出した 2 件を修正: restore 後に `next_view` を復元済み
  最大 id に合わせる(post-restore spawn の `DuplicateView` 衝突)、
  undo/redo を `ProjectSession` 経由にして `EngineStub` の
  graph build を呼ぶ(project 変更→graph build の不変条件)。

## Related Experiments

### Adopted

なし — 承認済み設計どおりに実装。

### Rejected

なし。

## Validation

| 項目 | 結果 |
| --- | --- |
| `mise run fmt-check` | PASS |
| `mise run check` | PASS |
| `mise run lint` | PASS (`-D warnings`) |
| `mise run test` | PASS (workspace 全テスト + ws_a 8 件) |
| Cargo.lock ruviform rev 統一 | `94c5bf8` 単一 |
| `git merge origin/main` | Already up to date (f7aadd0) |

受入対応: WS-A-01 `ws_a_01_*`、WS-A-02 `ws_a_02*` (semantic
mapping/Follow/Pinned/Group 独立/dedup/no-reemit/mapping 不能)、
WS-A-03 `ws_a_03_*` (明示 target/suppress/unresolved/stale
revision/no cross-window fallback/ViewBusy)、WS-A-04 `ws_a_04*`
(detach→reattach/rollback/close veto/detach/save-restore/破損復帰)、
WS-A-05 `ws_a_05_*` (dock 操作で revision・graph_builds・undo 履歴
不変)、WS-A-06 本レコード。

### 未検証(正直な記録)

- 実 OS Window/WebView/GPU/IME/plugin GUI: 対象外 (ruviform #243
  lane)。`WindowHandle` 経由のみ headless 検証済み。
- `mise run test-msrv`: rust 1.89 toolchain が本環境に未導入のため
  未実施 (pinned 1.99.0 で全検証)。
- monitor 複数構成での placement 補正: `restore_or_default` に
  委譲、単一 monitor で検証。

## Risks

- `WindowId` を含む `FocusState` は persist 対象外(セッション
  固有)。`LinkedContext.source`/`last_event` は restore 後も生値の
  ViewInstanceId を保持する(復元で id 同一のため整合)。
- `revision_inbox` は PoC の配信ログであり無制限に伸びる。実装
  系では event 化/上限化が必要。

## Follow-ups

- Inspector `Explore` 状態は issue #5 側。
- 実機検証は ruviform #243 lane で実施。
- 複数 project・remote asset DnD・tool system は対象外のまま。
