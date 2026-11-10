---
id: "PLAN-034VpoEbeiqPIm8MYZAnAD"
type: "plan"
status: "approved"
title: "SF-04 slice5 実装計画 — probe subscriber/window ownership（session 層 lease・capture owner）"
created: "2026-11-10"
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
    - "docs/specs/sf-04-slice5-probe-subscriber-ownership.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-slice5-probe-subscriber-ownership.md"
    - "docs/designs/sf-04-slice5-probe-subscriber-ownership-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice5 実装計画 — probe subscriber/window ownership（session 層 lease・capture owner）

## Goal

`ProbeTraceSession` に subscriber lease（`ViewInstanceId` principal）
と capture owner 追跡を追加し、SF-PROBE-06 の session 層契約
（共有購読 dedup・Window Close/Move/Session stop/明示 abort の
ownership）を headless unit test で固定する。port・player・
project・app 層は変更しない。

## Scope

単一実装ストリーム。`crates/synvelis-workspace/src/probe_trace.rs`
本体 + `#[cfg(test)]` unit tests、`lib.rs` export 追加のみ。
`control.rs` は設計上変更不要（`probe_session` field 型は不変）。
分割不要 — 1 task。

## Upstream Documents

- `docs/specs/sf-04-slice5-probe-subscriber-ownership.md`
- `docs/designs/sf-04-slice5-probe-subscriber-ownership-design.md`

## Design Inputs

- `docs/designs/sf-04-slice5-probe-subscriber-ownership-design.md`
  （approved）— state 構造・API・不変条件・`remove_key` 手順・
  エラー variant の正本。

## File Map

| File | Change |
| --- | --- |
| `crates/synvelis-workspace/src/probe_trace.rs` | `ProbePrincipal`/`ProbeReleaseReport`/`ProbeTraceError` variant 追加、`ProbeTraceSession` state 拡張（`leases`/`by_view`/`capture_owners`）、additive API 7 本 + `remove_key` helper、既存 API への lease/owner 同期、`#[cfg(test)]` unit tests |
| `crates/synvelis-workspace/src/lib.rs` | `ProbePrincipal`/`ProbeReleaseReport` export 追加のみ |
| `docs/` | spec/design/plan/task/test-spec/impl-ir + index 再生成 |

変更しない: `control.rs`（`ProbeTraceSession` の field 追加は
struct 内で完結）、`synvelis-project`/`synvelis-player`/
`synvelis-app` 他 crate、`tests/` 配下（別 Worker 占有）、
`syn.probe.*` wire op。

## Implementation Sequence

- [ ] T1-1: `probe_trace.rs` に `ProbePrincipal`・
  `ProbeReleaseReport`・`ProbeTraceError` variant を追加し、
  `ProbeTraceSession` に `leases`/`by_view`/`capture_owners`
  を追加（構造体のみ — 既存 API の port 呼出は不変）。
- [ ] T1-2: additive API 実装 — `probe_subscribe`/
  `probe_unsubscribe`/`probe_release_subscriber`/
  `probe_release_all`/`probe_capture_for`/
  `probe_abort_capture`/`probe_capture_owner`/
  `probe_lease_holders`/`probe_subscriber_keys` + `remove_key`
  helper。既存 `probe_request`/`probe_capture`/
  `probe_stop_capture`/`probe_remove` に lease/owner 同期を追加。
- [ ] T1-3: `#[cfg(test)]` unit tests（fake `ProbeTracePort`）
  — test-spec の F1–F8 を網羅。TDD: 契約違反を示す red →
  実装で green。
- [ ] T1-4: `lib.rs` export 追加。
- [ ] T1-5: docs index 再生成・`mise run fmt-check`/`check`/
  `lint`/`test`/`test-rt`/`test-msrv`・review-protocol・
  impl record・commit/push/PR。

## Dependencies and Risks

- 依存: `ViewInstanceId`（`ruviform-ui::dock` — 既存依存、
  `control_edge.rs`/`command.rs` に先例）。新規依存なし。
- リスク: port 直叩きで lease 不変条件が破れる経路は契約外
  （design Risks 参照）。`mode` 無視の silent semantics は
  doc comment で明示。
- リスク: `probe_remove`/`probe_release_all` が複数 holder を
  一括 revoke する際の `by_view` 逆引き漏れ → `remove_key`
  の clone-then-clean 手順で防止（design 不変条件 3）。

## Task Breakdown

- `docs/tasks/sf-04-slice5-task1-probe-subscriber-ownership.md`
  — T1（本計画の唯一の task。実装〜検証〜PR まで）。

## Verification

- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv` が green（最終検証前に
  `git fetch origin main && git merge origin/main`）。
- [ ] `probe_trace.rs` 内 unit tests が test-spec F1–F8 を
  網羅し green。
- [ ] 既存 `sf04_s2_probe_trace.rs` 等の既存テストが無変更で
  通る（外部挙動不変の証拠）。
- [ ] review-protocol: 2 pass・blocker なし。

## Open Questions / Gaps

- app 層の `syn.probe.*` wire op への subscriber 識別子接続は
  本 slice 対象外（SV#117 が app/src を占有中）— 後続 Issue
  で消費。session API は additive で既存呼出を壊さない。
- 実 2-window GUI での Window Move/Close 実挙動は #108/#114
  gate — headless では `ViewInstanceId` 安定性の契約レベル
  検証に留まる。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（単一 worker task）。
- Independent work streams: none。
- User approval required before dispatch: no（coordinator task
  spec が実装を直接指示）。

## Review Handoff

- reviewer focus: lease/owner の atomicity（port 失敗時に局所
  状態を汚さないか）、`remove_key` の `by_view` 逆引き完全性、
  既存 API の外部挙動不変性、`NotSubscriber`/`NotCaptureOwner`
  の fail-closed 網羅性。
