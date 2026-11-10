---
id: "TASK-034VpsGgHQsSoFmX1K61s3"
type: "task"
status: "in-progress"
title: "SF-04 slice5 T1 — probe subscriber/window ownership（session 層 lease・capture owner）"
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
    - "docs/plans/sf-04-slice5-probe-subscriber-ownership-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-04-slice5-probe-subscriber-ownership-plan.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-04-slice5-probe-subscriber-ownership.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice5 T1 — probe subscriber/window ownership（session 層 lease・capture owner）

## Work

1. `probe_trace.rs`: `ProbePrincipal`・`ProbeReleaseReport`・
   `ProbeTraceError::{NotSubscriber, NotCaptureOwner}` 追加、
   `ProbeTraceSession` に `leases`/`by_view`/`capture_owners`
   追加、`remove_key` helper 実装。
2. additive API 実装: `probe_subscribe`/`probe_unsubscribe`/
   `probe_release_subscriber`/`probe_release_all`/
   `probe_capture_for`/`probe_abort_capture`/
   `probe_capture_owner`/`probe_lease_holders`/
   `probe_subscriber_keys`。
3. 既存 API へ lease/owner 同期: `probe_request`（Session
   lease）・`probe_capture`（owner=Session）・
   `probe_stop_capture`（owner 消去）・`probe_remove`
   （`remove_key` 経由で全 lease revoke）。
4. `#[cfg(test)]` unit tests（fake `ProbeTracePort`）で
   test-spec F1–F8 を固定（TDD）。
5. `lib.rs` export 追加（`ProbePrincipal`/`ProbeReleaseReport`）。
6. docs index 再生成・mise 検証・review-protocol・impl record・
   commit/push/PR。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/sf-04-slice5-probe-subscriber-ownership-plan.md`
- `docs/specs/sf-04-slice5-probe-subscriber-ownership.md`
- `docs/designs/sf-04-slice5-probe-subscriber-ownership-design.md`

## Dependencies

- `ViewInstanceId`（`ruviform-ui::dock`、既存依存）。
- slice1–4 の probe port/trace 契約（`ProbeTracePort`・
  `ProbeSubscriptionKey`・`ProbeObservation`）。

## Affected Files

- `crates/synvelis-workspace/src/probe_trace.rs`（本体 + unit tests）
- `crates/synvelis-workspace/src/lib.rs`（export 追加のみ）
- `docs/specs|designs|plans|tasks|test-specs|impl/ir/
  sf-04-slice5-*` + 各 index

## Done When

- [ ] 同一 `(point,kind)` への 2 view subscribe で
  `port.probe_request` が 1 回のみ呼ばれ、両 lease・両 target
  相関が残る。
- [ ] 片方 view の release で port subscription/capture が
  継続し、最後の release で `probe_remove` 相当（Missing 記録）
  が走る。
- [ ] capture owner のみが `probe_abort_capture` でき、
  owner lease 喪失で owner が `Session` に退避し capture が
  継続する。
- [ ] legacy `probe_request`/`probe_remove` の外部挙動が不変
  （既存テスト無変更で green）。
- [ ] port 失敗時に局所 lease/owner 状態が変更されない。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `test-msrv` が green。

## Verification

- [ ] `cargo test -p synvelis-workspace`（unit tests、
  補助証拠 — canonical は `mise run test`）。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv`。
- [ ] review-protocol 2 pass・blocker なし。
