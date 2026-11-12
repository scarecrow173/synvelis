---
id: "IMPL-034UqZx9PC3TraceC3Impl0001"
type: "impl"
status: "completed"
title: "PC-C3 ControlTrace project 層 projection slice（TRACE-C3-01..04）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/100"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/trace.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/trace_c3.rs"
      - type: "file"
        path: "docs/specs/pc-c3-control-trace-projection.md"
      - type: "file"
        path: "docs/adr/pc-c3-control-trace-stage-evidence-freshness.md"
      - type: "file"
        path: "docs/designs/pc-c3-control-trace-design.md"
      - type: "file"
        path: "docs/plans/pc-c3-control-trace.md"
      - type: "file"
        path: "docs/test-specs/pc-c3-e2e-control-trace.md"
      - type: "file"
        path: "docs/tasks/pc-c3-task1-control-trace.md"
      - type: "file"
        path: "docs/impl/ir/pc-c3-control-trace-projection.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
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
      - type: "file"
        path: "docs/impl/ir/README.md"
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "index"
        path: "docs/adr/README.md"
      - type: "index"
        path: "docs/designs/README.md"
      - type: "index"
        path: "docs/plans/README.md"
      - type: "index"
        path: "docs/specs/README.md"
      - type: "index"
        path: "docs/tasks/README.md"
      - type: "index"
        path: "docs/test-specs/README.md"
      - type: "index"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/plans/pc-c3-control-trace.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c3-control-trace.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/pc-c3-control-trace.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c3-e2e-control-trace.md"
  verified-by:
    - "docs/test-specs/pc-c3-e2e-control-trace.md"
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C3 ControlTrace project 層 projection slice — 実装記録

## 実装内容

- `crates/synvelis-project/src/trace.rs`（新規）: `ControlTraceKey`（project/target/revision/plan/graph/transport/epoch/loop/position の相関キー）、`TraceSlot<T>`（Present/Missing/Unsupported/Dropped/Expired — 欠落 stage を合成しない）、`ControlTrace`（configured/prepared/scheduled/applied/dispatched/plugin_reported/observed の 7 独立 slot）、stage evidence 型群、`FreshnessContext`/`ObservationFreshness`/`StaleReason`/`UnavailableReason`、`ProbeObservation`/`MeasuredValue`（missing/lost/disabled と Measured Silence の区別）、`PlanActivationState`/`ActivePlan`/`FailedPlanAttempt`（失敗 plan は `active` に入らない last-good 投影）。`freshness()`/`plugin_reported_freshness()`/`observed_freshness()`/`is_coherent()` を実装。
- `ReceiptEvidence`: RT-minted `AppliedReceipt` は `CommandId` を外部構築できないため、correlation token を落とした projection payload として定義（`from_applied_receipt` で変換）。`ScheduledReceipt`/`AppliedStageReceipt` は status fail-closed wrapper。
- `DispatchEvidence`/`DispatchDropReason`: `ruviform-audio-plugin-bridge` 非依存の semantic mirror（`DispatchStage`/`DropReason` の意味論を継承。`Dispatched` = 計画/スパンへの投影であり plugin ACK ではない）。
- `crates/synvelis-project/src/lib.rs`: `pub mod trace;` + additive re-export のみ。
- `crates/synvelis-project/tests/trace_c3.rs`（新規）: TRACE-C3-01..04 を 6 fixture test で検証。

## 設計上の判断

- freshness は key↔ctx の世代/epoch/transport/loop/position 整合を先に評価し、不一致は `Stale{reason}`。stamp は age（`checked_add` deadline）→ 未来/uncomputable（`Unavailable{ClockUntrusted}`）→ `Estimated` provenance（`Estimated`、Current にしない）の順。
- `freshness()` は 2 channel の best（rank: Current<Estimated<Stale<DataLoss<Unavailable、同 rank は Present slot 優先→probe 優先）。個別 channel は `*_freshness()` で取得可能。
- `is_coherent()`: slot 内 evidence の generation が key と一致するか検査（receipt の plan/transport/epoch、dispatched/prepared の plan_generation）。

## 検証（canonical = mise、`git merge origin/main` 後 HEAD `70d6b21`）

| コマンド | exit status |
| --- | --- |
| `mise run fmt-check` | 0 |
| `mise run check` | 0 |
| `mise run lint` | 0 |
| `mise run test` | 0（`cargo test --workspace --locked`、trace_c3 6 tests 含む全 pass） |

補助証拠: `cargo check`/`clippy`/`test -p synvelis-project` 直接実行も全て成功（canonical は上記 mise）。

## review-protocol 結果

- doc chain（spec→adr→design→plan→test-spec→task）を review-protocol でレビュー。blocker なし。
- 実装中に発見・修正した問題: (1) `AppliedReceipt`/`CommandId` が外部構築不能 → `ReceiptEvidence` projection 型に変更、(2) `freshness()` の Unavailable tie が `NoEvidence` を選び `ClockUntrusted` を隠す → Present-slot 優先 tie-break に修正、(3) clippy `match_same_arms` → tuple score 比較に修正。CONFIRMED P0/P1 は残っていない。

## NotRun / Deferred

- **TRACE-C3-05**（bounded history・no-RT alloc-block・f64/clock/latency/lost metrics 実計測）: **NotRun** — synvelis-player RT adapter slice へ defer。
- **workspace `ControlSession`/`ControlObservation`/`observation_state` 接続・single drain owner・2 Inspector 二重起動防止**: **NotRun** — Issue 編集可範囲外・後続 integration slice の責務（`ProbeObservation::Missing`/`Disabled` が判断材料）。
- **#101 Binding/FrameBase・#103 Control Oracle・#114 GPUI との同一 Revision/Case ID 統合**: **NotRun** — 各 Issue gate。
- **実 plugin report/OS/RT 観測**: **NotRun** — fixture のみ。mock 出力で実 OS 成功を主張しない。
- `crates/synvelis-workspace/`・`crates/synvelis-player/`・`crates/synvelis-recording/`・`crates/synvelis-app/`・Cargo 依存: 変更なし。

## 既知の制約

- `DispatchEvidence`/`DispatchDropReason` は bridge 型の semantic mirror — bridge 側 `DispatchStage`/`DropReason` 変更時は integration slice で変換時に再検証が必要。
- `docs/` 監査の legacy findings（invalid-id-prefix 等 328 件）は本 slice 以前から存在 — 本 slice 起因ではない。
