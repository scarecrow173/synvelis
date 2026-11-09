---
id: "SPEC-vi1L6gnLC2jSjMMB3XG0N4"
type: "spec"
status: "approved"
title: "PC-C3-05 synvelis-player RT trace adapter — ledger 実イベント → ControlTrace 投影・bounded history・RT 計測"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/100"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c3-control-trace-projection.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-b-parameter-dispatch-note-scope-rt-offline.md"
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/100"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/pc-c3-control-trace-projection.md"
    - "docs/designs/pc-c3-control-trace-design.md"
  defers:
    - "synvelis-workspace ControlSession 接続・UI 表示・2 Inspector 二重起動防止（後続 integration slice）"
    - "HeadlessDriver/AttachedNode への adapter 搭載・transport control 配線（後続 integration slice）"
  deferred-by: []
---

# PC-C3-05 synvelis-player RT trace adapter — ledger 実イベント → ControlTrace 投影

## What Should Be Built

`crates/synvelis-player` に、`ParamDispatch`/`ParamDispatchLedger`/`NodeReport`/`DispatchStage` の実イベントを `synvelis_project::trace::ControlTrace` の stage slot へ写像する RT-safe trace adapter を追加する（Issue #100 TRACE-C3-05 slice）。project 層の projection 型を消費し、第二正本を作らない。

- `DispatchTraceAdapter`: `ParamDispatch` を**所有権ごと移譲**で受け取り、report channel の drain を adapter API 経由の単一経路に限定する（single drain owner の型レベル表現）。`PluginNodeControl`/生 `ParamDispatch` は再公開しない。
- ledger observation（`ParamValueObservation` + `DispatchStage`）→ `dispatched` slot の `DispatchEvidence` への写像。`Scheduled`/`Dispatched`/`Dropped{reason}`/`Reported`/`Unavailable` を bridge `DropReason` → project `DispatchDropReason` の semantic mirror で対応付ける。
- `NodeReport::Receipt(AppliedReceipt)` → `ReceiptEvidence` → `scheduled`/`applied` slot（status fail-closed: `Requested`/`Pending` → scheduled、`Applied` → applied、`Rejected` は slot を埋めない）。receipt の `plan_generation` が trace key の `control_plan_generation` と一致する target にのみ帰属させる。
- `DispatchStage::Reported` → `dispatched` slot の `DispatchEvidence::Reported`（ledger fidelity）+ `plugin_reported` slot の `PluginReportedEvidence`。report channel は timestamp を持たないため stamp は drain 時点の transport 位置を `Estimated` provenance で刻む（`Current` にはならない）。
- `record_configured` / `record_probe` ingest で `configured`/`observed` slot を外部証拠で埋められるようにする（probe 本体配線は後続 slice。`ProbeObservation::Missing`/`Disabled`/`Lost`/`Measuring` をそのまま保持）。
- bounded history: 明示的 cap を持つ `TraceEvent` ring（既定値は config）+ target 数 cap。溢れは `lost`/`Truncated` として metrics に表面化し、世代相対 window を外れた証拠は `TraceSlot::Expired` とする。欠落 stage を合成しない原則は project 層と同じ。
- RT 制約: `record_configured`/`record_probe`/`trace()`/`metrics()` 等の hot path は alloc/lock/block なし（cap 確保済み ring・借用返却）。`observe_cycle`（drain+投影）は非 RT 経路として明示し、RT callback からは呼ばない。
- f64・clock・latency・lost metrics の実計測: `TraceMetrics` が記録イベント数・lost 数・latency（f64 秒、frame_base 経由の実測）・clock stamp（`MediaTimestamp`/`EngineFrame`/generation）を保持し、fake in-process dispatch で実値を検証する。

## Why Now

Issue #100 TRACE-C3-05: project 層の `ControlTrace`（PR #206 merge 済み）は型と freshness 分類を提供するが、dispatch ledger の実イベントで slot を埋める producer が存在しない。`ParamDispatch::observe` は report を drain するが receipt を呼び出し側に返さず、ledger observation は plan generation を持たない。本 slice で player 層の adapter を確立し、後続 integration slice（workspace `ControlSession` 接続・UI）が実イベントで trace を構築できるようにする。

## Users and Value

- 後続 integration slice が `DispatchTraceAdapter` を `AttachedNode`/`ControlSession` に接続し、Inspector/Control Oracle が stage 別証拠を誤 Live なしに消費できる。
- `test-rt` で hot path の no-alloc/no-block を計測し、RT 契約の証拠を残す。

## Scope

### In Scope

- `crates/synvelis-player/src/trace.rs` 新設: `DispatchTraceAdapter`/`TraceAdapterConfig`/`TraceContext`/`TraceMetrics`/`TraceEvent`/`DispatchCycle` 消費側の写像・bounded history・metrics。
- `crates/synvelis-player/src/dispatch.rs`: `observe` の receipts 付き variant（`observe_cycle`/`DispatchCycle`）と `record_reported`/`mark_reporting_unavailable` passthrough の追加（既存 `observe` は後方互換で維持）。
- `crates/synvelis-player/src/lib.rs` additive export。
- `crates/synvelis-player/Cargo.toml` dev-dependency `stats_alloc` 追加（RT 計測、synvelis-project と同版）。
- `crates/synvelis-player/tests/trace_c3_05.rs`（機能 fixture）+ `tests/trace_c3_05_rt.rs`（単一 `rt_` alloc 計測）。
- `docs/` 文書チェーン。

### Out of Scope

- `crates/synvelis-workspace/` `ControlSession` 接続・single drain owner の workspace 側配線・UI 表示・2 Inspector 二重起動防止 — 後続 integration slice。
- `HeadlessDriver`/`AttachedNode` への adapter 搻載・`control()` transport 操作の再公開 — 後続 slice（adapter は `control()` を公開しない）。
- #101 Binding/FrameBase・#103 Control Oracle・#114 GPUI との同一 Revision/Case ID 統合 — NotRun（各 Issue gate）。
- 実 plugin report channel（upstream に param→host report は未実装）・実 OS/RT 観測 — NotRun（in-process fake dispatch のみ。mock で実 OS 成功を主張しない）。
- `synvelis-project`/`synvelis-app`/`synvelis-recording`/Ruviform の変更・Cargo 依存の追加（dev-dep `stats_alloc` を除く）。

## Requirements

### TRC-C3-05-R1: ledger 実イベント → stage slot 写像

`observe_cycle` は `ParamDispatch` の drain+ledger 投影を 1 回だけ実行し、返る per-target observation を `DispatchEvidence` へ写像して `dispatched` slot を更新する。`DispatchStage::Reported` は `plugin_reported` slot も更新する。写像不能な evidence（必須 `ParamValue` 欠落・plan generation が bounded window 外）は slot を `Expired`/据置とし、合成しない。

### TRC-C3-05-R2: receipt 相関と single drain owner

`NodeReport::Receipt` は `ReceiptEvidence::from_applied_receipt` で投影し、`plan_generation` が target の `control_plan_generation` と一致する trace の `scheduled`/`applied` slot にのみ格納する。drain は adapter が所有する `ParamDispatch` 経由の単一経路 — `PluginNodeControl`・`ParamDispatch`・ledger への第 2 drain 経路を公開 API に作らない。

### TRC-C3-05-R3: bounded history と loss 表面化

`TraceEvent` ring は `history_capacity` で有界。溢れた最古 event は失われ `metrics` の lost 系 counter に計上される。target 追跡は `max_targets` で有界（超過時は最終更新が最古の target を退避・計上）。plan generation→ledger revision の相対 window は `plan_gen_window` で有界とし、window 外の observation は `TraceSlot::Expired`。全ての cap は `TraceAdapterConfig` で明示・既定値あり。

### TRC-C3-05-R4: RT hot path no alloc/block + 実計測

`record_configured`/`record_probe`/`trace()`/`metrics()`/`history_len()` は warm-up 後に heap 割当・lock・block を行わない（`stats_alloc` Region で 0 alloc/0 dealloc を `rt_` test で実測）。`observe_cycle`/`dispatch`/`bind` 等の非 RT 経路は割当を許容するが RT callback から呼ばれないことを doc で明示する。

### TRC-C3-05-R5: f64・clock・latency・lost metrics 実計測

`TraceMetrics` は記録 event 数・lost 内訳（ring 退避・target 退避・generation window 外れ）・latency（`Dispatched` evidence の scheduled→delivered frame 差を frame_base で f64 秒換算、変換不能時は未記録）・clock（最終 `MediaTimestamp`/`EngineFrame`/generation stamp）を保持する。fake in-process dispatch（実 `PluginNode`+`GraphRuntime` 駆動）で各値が実測値と一致することを検証する。

### TRC-C3-05-R6: project 層型の消費・第二正本禁止

stage evidence・freshness・probe 状態は `synvelis_project::trace` の型をそのまま使う。bridge `DispatchStage`/`DropReason` → project `DispatchEvidence`/`DispatchDropReason` の写像は adapter 内の一箇所に集約し、ledger/report の状態を adapter 側で再実装しない。

## Acceptance Criteria

- [ ] TRACE-C3-05: bounded history（明示 cap + Truncated/DataLoss{count} 相当の lost 表面化）・RT hot path no alloc-block（`mise run test-rt` で実測）・f64/clock/latency/lost metrics を fake in-process dispatch で実計測。
- [ ] TRACE-C3-01 の dispatched/plugin_reported/observed stage が実 ledger イベントで埋まる（fake dispatch で Receipt 相関を検証 — `scheduled`/`applied` slot が `ReceiptEvidence` で Present、generation 整合で `is_coherent`）。

## Deferred Design Concerns

- workspace `ControlSession` 接続・UI・2 Inspector 二重起動防止・`HeadlessDriver` への搭載は後続 integration slice。
- 実 plugin→host param report channel・実 OS/RT 観測は upstream/環境依存 — NotRun。

## Sources and Relations

- Issue #100（ControlTrace）— 本 spec の source。TRACE-C3-05 slice。
- PC-C3 spec（project 層 projection・freshness 契約）、PC-B spec（dispatch 境界・honesty contract）。
- `docs/adr/pc-c3-05-trace-adapter-drain-history.md` — drain 単一所有・bounded history・generation 相対 window の決定。
