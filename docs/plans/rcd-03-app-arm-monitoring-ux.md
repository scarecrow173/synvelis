---
id: "PLAN-034VedskoBprfWhNkuqz6f"
type: "plan"
status: "approved"
title: "RCD-03 app Arm/Monitoring UX + monitor session projection slice"
created: "2026-11-10"
updated: "2026-11-10"
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
    - "docs/specs/rcd-03-app-arm-monitoring-ux.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/rcd-03-app-arm-monitoring-ux.md"
    - "docs/designs/rcd-03-app-arm-monitoring-ux.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/rcd-03-app-arm-monitoring-ux.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 app Arm/Monitoring UX + monitor session projection slice Implementation Plan

## Goal

`synvelis-app` に session-scope の Arm/Monitoring/Punch UX op
（`syn.monitoring.*`）と `syn.monitoring.state` 読み取り投影を追加し、
crate 層の `decide`/`MonitorProbe`/`preflight`/`prepare_low_latency`
契約を消費する headless な製品 UX 面を確立する。

## Scope

単一実装ストリーム。`crates/synvelis-app/` 内に閉じ、
`synvelis-project`/`recording`/`player`/`workspace` は消費のみ
（player は read-only、変更禁止）。

## Upstream Documents

- `docs/specs/rcd-03-app-arm-monitoring-ux.md`
- `docs/specs/rcd-03-monitoring-punch-latency.md`（crate slice spec）

## Design Inputs

- `docs/designs/rcd-03-app-arm-monitoring-ux.md`（approved）
- `docs/designs/rcd-03-monitoring-punch-latency.md`（crate slice）

## File Map

- `crates/synvelis-app/src/monitoring.rs` - `MonitorUxState`/
  `MonitorTrackDecl`/`MonitorSessionState`/`PunchMarks`、op 用の
  状態遷移メソッド、`state` 投影（decide/MonitorProbe/latency 合成）。
- `crates/synvelis-app/src/ui_ops.rs` - `op_monitoring_set` /
  `op_monitoring_punch` / `op_monitoring_transport` /
  `op_monitoring_session` / `op_monitoring_latency` /
  `op_monitoring_state` ハンドラと wire enum 写像ヘルパー。
- `crates/synvelis-app/src/service.rs` - OPS 6 件登録 + dispatch 6 arm。
- `crates/synvelis-app/src/app.rs` - `monitoring` field、
  `project_new_inner`/`project_open_inner` での reset。
- `crates/synvelis-app/Cargo.toml` - `ruviform-media-core` 依存追加
  （LatencyComponent.clock_epoch の wire 写像）。
- `crates/synvelis-app/src/lib.rs` - `mod monitoring;`（必要なら
  re-export は最小限）。
- `crates/synvelis-app/tests/monitor_arm.rs` - headless fixture
  （test-spec E1–E7）。
- `Cargo.lock` - 依存追加に伴う再生成（生成物）。

## Implementation Sequence

- [x] spec/design/plan/test-spec/task の doc chain 作成
- [ ] `monitoring.rs`: 状態型 + `set`/`punch`/`transport`/`session`/
  `latency` の状態遷移 + `state` 投影（decide・MonitorProbe・
  LatencyReport::resolve・prepare_low_latency を消費）
- [ ] `ui_ops.rs` + `service.rs` + `app.rs` + `Cargo.toml` の配線
- [ ] `tests/monitor_arm.rs` fixture（E1–E7）
- [ ] `mise run fmt` → `fmt-check`/`check`/`lint`/`test`/
  `test-rt`/`check-app-native`/`test-msrv` で検証
- [ ] `git fetch origin main && git merge origin/main` 後に再検証
- [ ] review-protocol レビュー → blocker 解消
- [ ] Conventional Commits で論理分割 commit → push → PR（Refs #117）

## Dependencies and Risks

- `ruviform-media-core` は workspace git 依存 — 追加は lock 再生成を
  伴うが rev は不変のはず（既存 lock の rev を使う）。
- `synvelis-app` は `synvelis-player` に既に依存 — 追加 dep は
  `ruviform-media-core` のみ。
- doc-driven-dev-graph の route は既存 artifact-graph 不整合で
  blocked（本 slice と無関係の broken relation）。spec→design→
  plan→task は手動で canonical 配置に作成済み。

## Task Breakdown

- TASK-034Vee…（`docs/tasks/rcd-03-task2-app-arm-monitoring-ux.md`）
  単一 task — op 実装 + fixture + 検証。

## Verification

- [ ] `mise run fmt-check` `check` `lint` `test` `test-rt`
  `check-app-native` `test-msrv` 全 pass。
- [ ] `cargo test -p synvelis-app --test monitor_arm`（補助証拠、
  canonical = mise run test）。
- [ ] fixture が test-spec E1–E7 を網羅。

## Open Questions / Gaps

- `syn.monitoring.transport` の position を musical tick とする点は
  design で確定済み（session frame 変換は plan.mapping 経由）。
- segment projection の feed block は 4096 frame 固定（design 記載）。

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

- reviewer は route 投影が decide/MonitorProbe のみに依存すること、
  Hardware Direct の正直な投影、bounded projection、session-only
  （project 非変更）を重点確認。
