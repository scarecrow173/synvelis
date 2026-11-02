---
id: "PLAN-034QwZOjdblHRzDVhDXNIP"
type: "plan"
status: "in-progress"
title: "WS-A Workspace 基盤 headless PoC 実装計画"
created: "2026-11-02"
updated: "2026-11-02"
owners: []
relations:
  source: []
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md"
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
---

# WS-A Workspace 基盤 headless PoC 実装計画

## Goal

Issue #36 スコープ 6 項目を `synvelis-workspace` クレートとして実装し、
WS-A-01..06 acceptance を headless テストで全 PASS させる。

## Scope

単一実装ストリーム。境界: 新クレート `crates/synvelis-workspace`
(deps: `synvelis-project` + `ruviform-ui` + serde 系) + ルート
`Cargo.toml`/`Cargo.lock` の ruviform rev 更新 + 受入 test。
ruviform・他 Synvelis クレートには触れない。

## Upstream Documents

- docs/specs/ws-a-workspace-foundation-view-selection-linkgroup-command-dock.md
- docs/adr/ws-a-workspace-crate-boundary-and-ruviform-dock-host-contract-adoption.md
- docs/designs/ws-a-workspace-foundation-design.md
- docs/designs/workspace-docking-state-architecture.md (設計正本)

## Design Inputs

- ruviform `ruviform_ui::dock`: `DockWorkspace`/`DockTarget`/`DropZone`/
  `WindowCloseDisposition`/`DockEvent`/`LayoutSnapshot::capture`/
  `restore`/`restore_or_default`/`apply_restored_tree`/`MoveToken`。
- ruviform `ruviform_ui::window`: `WindowId`/`WindowEvent`/`HostEvent`/
  `CloseDecision`/`WindowHandle`/`WindowConfig`/`MonitorInfo`/
  `WindowGeometry`/`MonitorHint`。
- synvelis-project: `ProjectController`/`Command`/`Operation`/`ProjectOp`/
  `Project`/`TrackId`/`LaneId`/`ClipId`/`SourceId`/`CounterIdGenerator`。

## File Map

- add: `crates/synvelis-workspace/Cargo.toml`
- add: `crates/synvelis-workspace/src/{lib,error,view,state,focus,link,
  command,session,workspace,host,persist}.rs`
- add: `crates/synvelis-workspace/tests/ws_a.rs`
- modify: `Cargo.toml` (members + `ruviform-ui` git dep)
- modify: `Cargo.lock` (ruviform rev → #181/#182 含有 + 新 dep 解決)

## Implementation Sequence

- [ ] task1: crate skeleton + Cargo.toml/lock + error/view/state/focus
- [ ] task2: link group + selection mapping + session/revision 配信
- [ ] task3: command router + resolve outcome
- [ ] task4: workspace manager + dock lifecycle + fake host + persist
- [ ] task5: 受入 test ws_a.rs (WS-A-01..06) + 単体 test 充実
- [ ] task6: merge main + 再生成 + 最終検証 + impl record

## Dependencies and Risks

- ruviform git 依存を #181/#182 含有 rev へ上げると既存クレートも
  同 rev に移行する → 全量 build/test で回帰確認。
- `WindowId` が host-issued のため永続化では snapshot windows 順序で
  `WindowKey` を対応付ける (design §永続化)。
- clippy pedantic で大きな module を `-D warnings` 通すため `#[allow]`
  は最小・理由付きで。

## Task Breakdown

| Task | 内容 | 依存 |
| --- | --- | --- |
| ws-a-task1 | crate skeleton・deps/lock 更新・error/view/state/focus 型 | — |
| ws-a-task2 | link group・semantic mapping・session 配信・engine stub | task1 |
| ws-a-task3 | command router・target 解決・suppress/unresolved | task2 |
| ws-a-task4 | workspace manager・dock lifecycle・fake host・persist | task2 |
| ws-a-task5 | 受入 test WS-A-01..06 + 単体 test | task3, task4 |
| ws-a-task6 | merge main・再生成・最終検証・impl record | task5 |

## Verification

- [ ] `mise run fmt-check` / `check` / `lint` / `test` 全 PASS。
- [ ] `tests/ws_a.rs` が WS-A-01..06 を個別 test で検証。
- [ ] Cargo.lock の全 ruviform source が同一 rev (#181/#182 祖先)。
- [ ] review-protocol で blocker なし。

## Open Questions / Gaps

- Inspector `Explore` 状態は #5 へ deferred (Follow/Pinned のみ)。
- 実機 Window/Focus/DPI は ruviform #243 lane で未検証 (headless のみ)。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution (dispatched worker)。
- Independent work streams: なし (単一クリート・直列)。
- User approval required before dispatch: 承認代替として review-protocol
  を実施済み。

## Review Handoff

- ruviform `ruviform-ui` の API 名・契約は 94c5bf8 で確認済み。
- `LayoutSnapshot` に window key がない点は設計で順序対応付けを採用。
