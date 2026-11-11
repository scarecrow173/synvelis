---
id: "PLAN-034VpQd9mO4xP1wT6vA8wL"
type: "plan"
status: "approved"
title: "RCD-03 Arm/Monitor/Punch Project 永続投影 slice"
created: "2026-11-11"
updated: "2026-11-11"
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
    - "docs/specs/rcd-03-monitor-persistence.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-03-monitor-persistence.md"
    - "docs/designs/rcd-03-monitor-persistence-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-03-monitor-persistence.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 Arm/Monitor/Punch Project 永続投影 Implementation Plan

## Goal

Arm/Monitor/channel_map/lane 宣言と punch marks を `Project` の
永続モデル（schema v12）へ POD 投影し、`synvelis-app` の
`syn.monitoring.*` 宣言変更を `ProjectController` 経由の
undo 可能 op で write-through 化する。session 観測値は
非永続のまま維持する。

## Scope

`synvelis-project`（POD 型・schema・migration・ops・tests）と
`synvelis-app`（monitoring write-through・state read-through・
fixture 更新）に閉じる。`synvelis-recording`/`player`/`workspace`/
`library`/`ui/` は変更しない。`synvelis-gateway` は `op_name` の
網羅 match を壊さないための最小 arm 追加のみ（Coordinator 承認済み
の範囲拡張）。

## Upstream Documents

- `docs/specs/rcd-03-monitor-persistence.md`
- `docs/specs/rcd-03-app-arm-monitoring-ux.md`（deferred 分の回収元）

## Design Inputs

- `docs/designs/rcd-03-monitor-persistence-design.md`
- `docs/designs/rcd-03-app-arm-monitoring-ux.md`（session 観測値の
  分離境界）

## File Map

- `crates/synvelis-project/src/monitoring_data.rs` - 新規 POD 型
  （`ArmState`/`MonitorState`/`MonitorChannelMap`/
  `TrackMonitorDecl`/`PunchMarks`/`MonitoringData`）。
- `crates/synvelis-project/src/lib.rs` - module + export。
- `crates/synvelis-project/src/project.rs` - `monitoring` field、
  `Project::new`、`validate_references` 追加検査。
- `crates/synvelis-project/src/store.rs` - `SCHEMA_VERSION` 12 +
  履歴コメント。
- `crates/synvelis-project/src/migration.rs` - v11→v12 step。
- `crates/synvelis-project/src/ops.rs` - `SetTrackMonitor` /
  `ClearTrackMonitor` / `SetPunchMarks` variants + validate/apply、
  `RemoveTrack` cascade、`RemoveLane` 参照検査。
- `crates/synvelis-project/tests/monitoring_persist.rs` - 新規 fixture
  （serde/migration/op/undo/validation）。
- `crates/synvelis-app/src/monitoring.rs` - `MonitorUxState` 再構成
  （session overlay のみ保持）、contexts/state/start_session の
  project read-through、punch 計算 helper。
- `crates/synvelis-app/src/ui_ops.rs` - `op_monitoring_set` /
  `op_monitoring_punch` の write-through 配線（patch 分解 +
  `session.apply`）。
- `crates/synvelis-app/src/app.rs` - project_new/open の reset
  コメント更新（rehydrate は project 正本化により自動）。
- `crates/synvelis-app/tests/monitor_arm.rs` - 永続化契約への
  fixture 更新（write-through/undo/save-open/観測値非永続）。
- `crates/synvelis-gateway/src/prepare.rs` - `op_name` の網羅 match
  に新 `ProjectOp` 3 variant の arm を追加（Coordinator 承認済みの
  範囲拡張 — 追加しないと workspace check が E0004 で落ちる）。
- `docs/` - 本 chain 6 文書 + 各 README index。

## Implementation Sequence

- [x] spec/design/plan/task/test-spec の doc chain 作成
- [x] `monitoring_data.rs` + `project.rs` + `store.rs` +
  `migration.rs` + `ops.rs` + `lib.rs`
- [x] `crates/synvelis-project/tests/monitoring_persist.rs`
- [x] `monitoring.rs` 再構成 + `ui_ops.rs` write-through +
  `app.rs` コメント
- [x] `monitor_arm.rs` fixture 更新・拡張
- [x] `mise run fmt` → `fmt-check`/`check`/`lint`/`test`/
  `test-rt`/`check-app-native`/`lint-app-native`/`test-msrv`/
  `build` で検証
- [x] `git fetch origin main && git merge origin/main` 後に再検証
- [x] review-protocol レビュー → blocker 解消
- [ ] Conventional Commits で論理分割 commit → push → PR（Refs #117）

## Dependencies and Risks

- 依存方向 `project ← recording` を維持するため、project 側 POD は
  recording 型を参照しない別型として定義し app で写像する
  （wire/serde 語彙は同一）。
- `RemoveTrack` cascade と `RemoveLane` 参照拒否は既存削除契約の
  拡張 — `validate_references` の oracle が最終防衛線。
- doc-driven-dev-graph の route は既存 artifact-graph 不整合で
  blocked（本 slice と無関係）。canonical 配置に手動で文書を
  作成する（先行 slice と同じ対応）。

## Task Breakdown

- `docs/tasks/rcd-03-task1-monitor-persistence.md` — 単一 task
  （project 投影 + app write-through + fixture + 検証）。

## Verification

- [ ] `mise run fmt-check` `check` `lint` `test` `test-rt`
  `check-app-native` `lint-app-native` `test-msrv` `build` 全 pass。
- [ ] `cargo test -p synvelis-project --test monitoring_persist` と
  `cargo test -p synvelis-app --test monitor_arm`（補助証拠、
  canonical = mise run test）。
- [ ] fixture が test-spec E1–E6 を網羅。

## Open Questions / Gaps

- `syn.monitoring.state` の `in_project` は validate_references
  整合により常に true となるが、wire 形状維持のため field は残す。
- VCS semantic diff の monitor 対応は follow-up（spec の
  Deferred Design Concerns に記録）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（worker dispatch 内で実装）。
- Independent work streams: none。
- User approval required before dispatch: yes（worker contract により
  review-protocol の blocker なしを承認として扱う）。

## Review Handoff

- reviewer は POD/recording 型分離の写像漏れ、v11→v12 migration の
  既定値、`SetTrackMonitor` full-decl 意味論、session 観測値が
  永続化されないこと、RemoveTrack/RemoveLane の参照整合を重点確認。
