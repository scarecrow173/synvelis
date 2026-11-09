---
id: "IMPL-034UvgCFl4bXw9DZ4bTr3X"
type: "impl"
status: "in-progress"
title: "SF-04 slice2 ControlSession probe observation trace wiring"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/107"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/probe_trace.rs"
      - type: "file"
        path: "crates/synvelis-player/src/probe_trace.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/sf04_s2_probe_trace_bridge.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/probe_trace.rs"
      - type: "file"
        path: "crates/synvelis-workspace/tests/sf04_s2_probe_trace.rs"
      - type: "file"
        path: "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
      - type: "file"
        path: "docs/designs/sf-04-slice2-controlsession-probe-trace-design.md"
      - type: "file"
        path: "docs/test-specs/sf-04-slice2-controlsession-probe-trace.md"
      - type: "file"
        path: "docs/plans/sf-04-slice2-controlsession-probe-trace.md"
      - type: "file"
        path: "docs/tasks/sf-04-slice2-controlsession-probe-trace.md"
      - type: "file"
        path: "docs/impl/ir/sf-04-slice2-controlsession-probe-observation-trace-wiring.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-player/src/probe.rs"
      - type: "file"
        path: "crates/synvelis-player/src/routing_engine.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/control.rs"
      - type: "file"
        path: "crates/synvelis-workspace/src/lib.rs"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-04-slice2-controlsession-probe-trace.md"
    - "docs/tasks/sf-04-slice2-controlsession-probe-trace.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
    - "docs/designs/sf-04-slice2-controlsession-probe-trace-design.md"
    - "docs/test-specs/sf-04-slice2-controlsession-probe-trace.md"
    - "docs/plans/sf-04-slice2-controlsession-probe-trace.md"
    - "docs/impl/ir/sf-04-slice1-semantic-probe-adapter.md"
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

# SF-04 slice2 ControlSession probe observation trace wiring

## Summary

Issue #107 (SF-04) slice 2: probe observation を `ControlSession` 経由で
`ControlTrace.observed` slot へ記録する経路を実装した。project 層に
session 向け port 契約 `ProbeTracePort` を新設し、player 層に
`ProbeTraceBridge`（`RoutingEngine` probe lifecycle/drain + node 別
`DispatchTraceAdapter` への fail-closed 翻訳）、workspace 層に
`ProbeTraceSession`（session id 発行・subscription→target 相関・drain
単一 owner）を配線した。headless fake-port E2E と実 engine bridge
テストで observation → drain → trace slot → freshness 分類・
fail-closed・stale-generation reject を検証済み。

## Context

`docs/tasks/sf-04-slice2-controlsession-probe-trace.md`（in-progress）に
基づく。上流は spec/design/test-spec/plan の SF-04 slice2 文書チェーンと
slice1 の `sf-04-slice1-semantic-probe-adapter` IR。sv100 PR #209 で
deferred として明記されていた workspace 側 ControlSession wiring の残件。

## Implementation

### Changes

- `crates/synvelis-project/src/probe_trace.rs`（新規）:
  `ProbeTracePort` trait、`ProbeObservationUpdate`、`ProbePortError`。
  契約語彙のみ — runtime `ProbeId`/ruviform record/`NodeId` は境界を
  越えない。`probe_drain` を唯一の drain 経路、`record_probe` を
  `observed` slot のみ書き込み `false` で可視 drop とする契約を明文化。
- `crates/synvelis-player/src/probe_trace.rs`（新規）:
  `ProbeTraceBridge` — `RoutingEngine` への lifecycle 委譲、drain 時の
  `ProbeId → (key, active_plan_generation)` スナップショット、
  `MetricRecord`/`StatusRecord` → `ProbeObservation` fail-closed 翻訳、
  `ParameterTargetId → NodeId` 明示 route → node 別 adapter の
  `record_probe`。`RebindReceipt`/segment cut は `engine_mut()` 経由の
  既存 `publish` 経路にそのまま委譲（bridge は publish を呼ばない）。
- `crates/synvelis-player/src/{probe.rs,routing_engine.rs,lib.rs}`:
  additive のみ — `clock_ref` を `pub(crate)` 化、`probe_counts()`
  passthrough 追加、module/export 追加。
- `crates/synvelis-workspace/src/probe_trace.rs`（新規）+
  `control.rs`/`lib.rs`: `ProbeTraceSession`（`AtomicU64` で session
  id 発行、`ProbeSubscriptionKey → BTreeSet<ParameterTargetId>` 相関、
  fan-out 対応）、`ControlSession` への `pub(crate) probe_session`
  field 追加、session API 群（`probe_request`/`probe_set_mode`/
  `probe_capture`/`probe_stop_capture`/`probe_remove`/`probe_drain`/
  `probe_bindings`/`probe_segments`/`probe_trace`/`probe_traces`/
  `probe_observed_freshness`/`probe_subscriptions`/`probe_targets`）。
  port は呼び出し毎に `&mut dyn ProbeTracePort` で受け取り session は
  所有しない。
- テスト: `synvelis-workspace/tests/sf04_s2_probe_trace.rs`（fake port
  E2E 9 件）、`synvelis-player/tests/sf04_s2_probe_trace_bridge.rs`
  （実 engine + fake plugin host 3 件）。

### Decisions During Implementation

- port 契約を `synvelis-project` に置いた: workspace は player に依存
  できず、project/player 双方が project に依存できる唯一の層。
- `ProbeTraceSession` の field 名は `probe_session` — `probe_trace`
  メソッド名との衝突回避。
- `probe_remove` は port 成功後に相関 target へ `Missing` を同期記録:
  provider は remove 時点で runtime probe id をクリアするため、後続の
  RT `Removed` status は帰属不能。相関自体は保持（`Stopped` binding
  の再 request が同 key で可能）。
- bridge の `status_update` は generation guard を掛けない — status は
  channel 状態証拠であり計測主張ではない（`RebindDropped` は新世代で
  届き得る）。`Lost*` は collector の累積 channel-level counter を
  そのまま `count` に写す。
- `record_probe` の `false`（route 無し）は session drain report の
  `unrouted` で計数。`probe_remove` の同期 `Missing` 書き込みは
  route 不在時に trace 自体が存在しないため `NoEvidence` として
  freshness が正直に報告する。
- probe clock は `ProbeSession::pair` が RT frame cursor に wire する
  ため bridge テストの stamp は `Derived`（→ `Current` 分類可能）。
  unwired 時の `Estimated` fail-closed 経路は workspace fake-port
  テスト `sf04_s2_estimated_stamp_never_reaches_current` で検証。

## Related Experiments

### Adopted

<!-- none -->

### Rejected

<!-- none -->

## Validation

- `mise run fmt` / `mise run fmt-check`: pass
- `mise run check`（cargo check --workspace --all-targets）: pass
- `mise run lint`（cargo clippy --workspace --all-targets -- -D warnings）: pass
- `mise run test`（cargo test --workspace --locked）: pass（126 suites, 0 failed）
- `mise run test-rt`: pass
- `mise run check-headless-deps`: pass
- `mise run check-ruviform-rev`: **fail（既存・本変更と無関係）** —
  `ui/vendor/ruviform/REV` (`20a24a66`) と `Cargo.lock` 解決 rev
  (`0f5b5737`) の不一致。両ファイルとも本 diff 未変更で base
  `c281228` 時点の既存状態。`ui/`・`synvelis-app` は編集禁止範囲のため
  未対処のまま報告。
- 補助証拠（canonical = mise）: `cargo test -p synvelis-player
  --test sf04_s2_probe_trace_bridge` 3/3 pass、`cargo test -p
  synvelis-workspace --test sf04_s2_probe_trace` 9/9 pass。

## Risks

- `route_target`/`unroute_target` は bridge 固有 API で port 契約外 —
  composition root（#108/#109 側）が target→node 割当を所有する前提。
  本 slice では headless fixture のみが route を登録する。
- `RebindDropped`/`Lost*` の status 経路は翻訳実装済みだが、
  決定的に誘発する headless fixture は未整備（bridge テストは
  `ModeChanged{Off}`→`Disabled` と stale-generation reject を実証）。
- probe clock の `Derived` 分類は RT cursor 由来の正直な provenance
  であり、device/host clock との対応づけは #309 系の将来 gate。

## Follow-ups

- #108: UI 側で `probe_*` session API / `probe_observed_freshness` を
  消費する表示接続。
- #109: 実測・oracle（SF-06 cross gate）— 本 slice 範囲外・NotRun。
- composition root での `route_target` 配線（target→adapter node の
  権威ある割当）— app/session 統合 slice で実施。
- `check-ruviform-rev` の既存不一致は UI vendor 同期の別件として
  切り分けが必要（`mise run sync-ui-deps` 相当の運用判断）。
