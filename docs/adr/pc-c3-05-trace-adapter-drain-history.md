---
id: "ADR-NrfmeZXs11hrWcr5rWe6PC"
type: "adr"
status: "accepted"
title: "PC-C3-05 trace adapter — drain 単一所有・bounded history・generation 相対 window"
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
  implements:
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c3-05-rt-trace-adapter.md"
    - "docs/adr/pc-c3-control-trace-stage-evidence-freshness.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/pc-c3-control-trace-stage-evidence-freshness.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/100"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# PC-C3-05 trace adapter — drain 単一所有・bounded history・generation 相対 window

## Status

accepted（review-protocol によるレビュー済み・blocker なし）

## Context

Issue #100 TRACE-C3-05 は dispatch ledger の実イベントで `ControlTrace` を埋める player 層 adapter を要求する。制約:

- `PluginNodeControl::drain_reports` は report channel の唯一の drain 点。`ParamDispatch::observe` が既に内部 drain するため、第 2 reader が同 channel を drain すると receipt/fault が欠落する（Issue 契約「別のReaderが同じReport channelをdrainして欠落を作らない」）。
- `ParamDispatchLedger`/`ParamValueObservation` は upstream（`ruviform-audio-plugin-bridge`）型で plan generation を保持しない。`DispatchStage`/`DropReason` は project 層 `DispatchEvidence`/`DispatchDropReason` の semantic mirror。
- `ParamDispatchLedger` は非 RT（`record_plan` で alloc）。RT-safe と主張できるのは adapter の hot path（証拠 ingest・trace query）のみ。
- `NodeReport::Receipt(AppliedReceipt)` は `CommandId` を持ち test で mint 不能 — receipt は実 node 駆動でのみ得られる。

決定すべき論点: (a) drain 単一所有をどう型で表すか、(b) ledger observation → `plan_generation` をどう相関するか、(c) bounded history の単位と溢れの表面化、(d) RT hot path の境界。

## Considered Options

### Option 1: `ParamDispatch` に trace を内蔵

`observe` が `ControlTrace` を直接返す。→ 却下: `ParamDispatch` は dispatch orchestration の責務であり、trace history/metrics/drain 所有の関心が混ざる。既存 `observe` の呼び出し側（driver/tests）を壊す。

### Option 2: `DispatchTraceAdapter` が `ParamDispatch` を所有権ごと受け取る（採用）

`DispatchTraceAdapter::new(dispatch: ParamDispatch, …)` で move 受領。`control()`/`ParamDispatch` 参照を再公開しないため、report drain に到達できる公開経路は adapter の `observe_cycle` のみ — single drain owner が型レベルで成立。passthrough は bind/dispatch/stage 等の orchestration 面に限定し、`PluginNodeControl` は一切露出しない。

### Option 3: `&mut ParamDispatch` 借用 adapter

→ 却下: 呼び出し側が `dispatch.observe()`/`control().drain_reports()` を直接呼べ、drain 単一所有を型で保証できない。

## Decision

**Option 2 を採用。**

1. **drain 単一所有**: adapter が `ParamDispatch` を所有。公開面は `observe_cycle`（drain+投影+trace 更新の唯一経路）、`record_configured`/`record_probe`/`record_reported`（ledger/slot ingest）、`trace()`/`traces()`/`metrics()`/`history*()`（query）。`PluginNodeControl`・生 `ParamDispatch`・ledger は非公開。transport control（`set_clock`/`Seek`/`install_plan` 直叩き）は本 slice では公開しない — 後続 integration slice が必要な狭い wrapper を追加する。
2. **plan generation 相関**: adapter は成功した `dispatch()` ごとに `plan.generation` を記録し、ledger `revision`（`record_plan` 1 回 = revision +1、成功 dispatch と 1:1）との対応を `plan_gen_window` 件の相対 window で保持する。window 外の observation は generation を確定できないため `dispatched` slot を `TraceSlot::Expired` とする（証拠を捨てず失効として表面化）。upstream が `record_plan` の revision 規約を変えた場合は integration slice で再検証する（文書化した前提）。
3. **receipt 帰属**: receipt は node 単位の証拠。`plan_generation` が target trace key の `control_plan_generation` と一致する場合のみ `scheduled`（Requested/Pending）/`applied`（Applied）slot に格納し、status class ごとに最新を保持。`Rejected` は fail-closed で slot を埋めない。`transport_generation`/`clock_epoch` は receipt 由来を優先し、未受信時のみ span ctx の値を fallback とする（driver が fabricate する span の transport_generation=0 を鵜呑みにしない）。
4. **key の鮮度意味論**: `ControlTraceKey` の caller 供給 field（`project_id`/`project_revision`/`graph_generation`）と position/loop/transport/clock は「最新証拠の context」を保持し、新規証拠が無い target では凍結する — project revision 前進後も旧証拠を新 revision のものと偽装しない（`Stale{ProjectRevision}`/`Stale{Position}` で分類可能にする）。dedup は `(ledger revision, DispatchStage)` 単位。
5. **bounded history**: `TraceEvent`（Copy）を `VecDeque` ring（`history_capacity`、既定 256）に記録。溢れは最古を退避し `metrics` の lost counter に計上。target map は `max_targets`（既定 1024）で最終更新最古から退避・計上。cap は全て `TraceAdapterConfig` で明示。
6. **RT hot path**: `record_configured`/`record_probe`/`trace()`/`metrics()` は warm-up 後 alloc/lock/block なし（ring は capacity 確保済み、`trace()` は `&ControlTrace` 借用返却）。`observe_cycle`/`dispatch`/`bind` は非 RT 経路。`rt_` test が `stats_alloc` Region で hot path の 0 alloc/0 dealloc を実測する。
7. **metrics**: `TraceMetrics` に events/lost 内訳/latency（`Dispatched` の scheduled position→delivered frame 差を `frame_base` の `MediaDuration` で f64 秒換算、変換不能は未記録）/clock stamp（`MediaTimestamp`・`EngineFrame`・generation）を保持。`plugin_reported` の stamp は drain 時点の transport 位置を `Estimated` provenance で刻む（report channel が timestamp を持たないため `Current` にならない）。

## Consequences

- `synvelis-project`/`synvelis-workspace`/Ruviform は変更しない。`dispatch.rs` への追加は `observe_cycle`/`DispatchCycle`・`record_reported`・`mark_reporting_unavailable` のみで既存 `observe` は不変。
- `record_reported`/`mark_reporting_unavailable` は upstream ledger の既存能力への passthrough — 実 plugin→host report channel は未実装のため `Reported`/`Unavailable` stage は fixture 経路でのみ出る（文書化）。
- `HeadlessDriver`/`ControlSession` への搭載は後続 slice — adapter は現時点で driver と並存し、二重 drain は発生しない（driver は `ParamDispatch` を所有したまま。adapter を使うには `ParamDispatch` の move が必要）。
- plan generation window・history・target の各 cap は memory を有界に保つが、cap 外の証拠は `Expired`/lost として失われる — bounded であること自体が契約。
