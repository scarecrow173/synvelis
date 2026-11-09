---
id: "PLAN-034Uuhu2Eis7GPJ5fej73V"
type: "plan"
status: "approved"
title: "SF-05 slice1 実施計画 — Signal Flow headless"
created: "2026-11-09"
updated: "2026-11-09"
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
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
  implemented-by: []
  depends-on:
    - "docs/designs/sf-05-slice1-signal-flow-headless-graph-visual-state-status.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/sf-05-slice1-signal-flow-workspace-read-model-engine-evidence-visual-state-app-i.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-05-signal-flow-headless-hierarchy.md"
    - "docs/designs/sf-05-slice1-signal-flow-headless-graph-visual-state-status.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-05-slice1-signal-flow-headless.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-05 slice1 実施計画 — Signal Flow headless

## Goal

`RoutingModel` の read-only Signal Flow 投影・visual-only 操作の
非破壊性・Revision/Generation 状態表示を headless fixture で
実現する（SF-UX-01/02/05）。

## Scope

単一実装 stream。workspace read model + app 表示/op + test の
一括 slice。編集 op・probe request・schema migration・実 GUI は
含まない。

## Upstream Documents

- `docs/specs/sf-05-signal-flow-headless-hierarchy.md`
- `docs/adr/sf-05-slice1-signal-flow-workspace-read-model-engine-evidence-visual-state-app-i.md`
- `docs/designs/sf-05-slice1-signal-flow-headless-graph-visual-state-status.md`

## Design Inputs

- `docs/designs/sf-05-slice1-signal-flow-headless-graph-visual-state-status.md`

## File Map

- `crates/synvelis-workspace/src/signal_flow_ui.rs` - 新規 read
  model（entity ref/node/edge/options/status/visual state/関数）
- `crates/synvelis-workspace/src/lib.rs` - additive export
- `crates/synvelis-workspace/tests/sf_05.rs` - 投影 fixture test
- `crates/synvelis-app/src/engine.rs` - probe passthrough
- `crates/synvelis-app/src/app.rs` - `signal_flow_view`・evidence
  helper・publish 行
- `crates/synvelis-app/src/screens.rs` - `SignalFlowScreenState`・
  tab・tables
- `crates/synvelis-app/src/ui_ops.rs` - `UiOpsState.signal_flow`・
  op handlers
- `crates/synvelis-app/src/service.rs` - op 登録/dispatch
- `crates/synvelis-app/tests/sf05_signal_flow.rs` - app 側 fixture

## Implementation Sequence

- [ ] 1. `signal_flow_ui.rs` + `lib.rs` export を作成
- [ ] 2. `crates/synvelis-workspace/tests/sf_05.rs` で投影規則を検証
- [ ] 3. `engine.rs` probe passthrough + `app.rs` view/evidence/publish
- [ ] 4. `screens.rs` Signal Flow tab + `ui_ops.rs`/`service.rs` ops
- [ ] 5. `crates/synvelis-app/tests/sf05_signal_flow.rs` で
  SF-UX-02/05 を検証
- [ ] 6. `mise run fmt-check`/`check`/`lint`/`test`（+ app-native
  存在範囲）
- [ ] 7. `git fetch origin main && git merge origin/main` → 最終検証
- [ ] 8. impl record・task done 化・commit/push・PR

## Dependencies and Risks

- `RoutingModel`/`RoutingActivationReceipt`/`RoutingSourceMap`/
  probe 型は全て `synvelis-project` 公開型 — workspace から直接
  消費可能。player 依存は追加しない。
- `UiOpsState`/`MainScreenState` への additive field は
  `Default` derive で既存 test を壊さない想定 — `check` で確認。
- `Expanded` の node 数は `max_nodes` で bound — 大 fixture でも
  `Truncated` で破綻しない。

## Task Breakdown

- `docs/tasks/sf-05-slice1-signal-flow-headless.md` 参照。

## Verification

- [ ] `mise run fmt-check` exit 0
- [ ] `mise run check` exit 0
- [ ] `mise run lint` exit 0
- [ ] `mise run test` exit 0（`sf_05.rs`/`sf05_signal_flow.rs` 含む）
- [ ] `mise run check-app-native`/`lint-app-native`/
  `build-app-native` — 存在すれば実行、不能なら NotRun 記録
- [ ] 実 OS/GPUI は NotRun（#114 gate）

## Open Questions / Gaps

- `Local{root}` の root が存在しない entity を指す場合は
  `coverage=Unavailable`（node 0 件の偽表示をしない）。
- probe binding の node 写像は `SemanticSignalEntityRef` の
  Track/Bus/Strip/Processor のみ — Send point は status.probes
  一覧に留め node には畳まない。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（単一 worker dispatch）。
- Independent work streams: none。
- User approval required before dispatch: no（worker contract に
  より review-protocol で代替）。

## Review Handoff

- reviewer focus: edge dedup（connection/send 1:1）・
  Published/Active 区別・visual op の非破壊性・
  `deny_unknown_fields` schema v2 に触れていないこと。
