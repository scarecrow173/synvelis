---
id: "SPEC-4nB7kQ2wX9mZ5vT8yR1jL3p"
type: "spec"
status: "approved"
title: "SF-04 slice8 — SF-PROBE-05 fault fixtures + seek 以外の不連続宣言点"
created: "2026-11-12"
updated: "2026-11-12"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/107"
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
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
    - "docs/specs/sf-04-slice3-clock-epoch-discontinuity.md"
    - "docs/specs/sf-04-slice5-probe-subscriber-ownership.md"
    - "docs/specs/sf-04-slice6-probe-compare.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/sf-04-slice3-clock-epoch-discontinuity.md"
    - "docs/specs/sf-04-slice6-probe-compare.md"
  defers: []
  deferred-by: []
---

# SF-04 slice8 — SF-PROBE-05 fault fixtures + seek 以外の不連続宣言点

## What Should Be Built

Issue #107（SF-04 Semantic Probe Adapter）の 2 つの bounded
deliverable。

### A. SF-PROBE-05 fault fixtures（headless fail-closed 証拠層）

実 `RoutingEngine` + `ProbeTraceBridge` の headless fixture で
probe の fault 経路を固定する
（`crates/synvelis-player/tests/sf04_s8_probe_faults.rs` 新規）:

- **chunk pool 枯渇**: `pool_chunks=8` == `chunks_out` capacity
  の既定構成で、drain しない capture block を pool 容量超過まで
  流し `free.try_pop` 失敗を起こす。`lost_blocks` counter・
  `StatusKind::LostBlocks`・`ObservationGapKind::LostBlocks` gap・
  `ProbeObservation::Lost` update・`CaptureCompleteness::
  Incomplete` segment を証明する。
- **telemetry overflow**: `telemetry_capacity=256` を超える
  undrained record で `lost_records` counter が陽性になり、
  slice6 の `lost_records` floor 機構経由で比較 coverage が
  `Unknown` へ降格・joint valid window が消えて fail-closed
  （`Incomparable`）になることを証明する。
- **観測状態の区別**: measured silence（`Measuring{Silence}`）・
  `Disabled`（Off mode）・`Lost`（明示的喪失証拠）・`Missing`
  （binding 非存在の session 層状態 — drain が捏造しない）を
  bridge update レベルで区別する。
- **capture variant**: `probe_capture(key, max_frames)` の
  budget 枯渇（`BudgetExhausted`）・明示 `probe_stop_capture`
  （`Requested`）・remove 中 capture（`Removed`）がそれぞれ
  異なる `CaptureSegmentEnd` で閉じ、二重 capture は
  `AlreadyCapturing` で fail-closed することを証明する。
- **正直な到達可能性記録**: `StatusKind::LostRecords` は宣言・
  写像済みだが deck が発行しない（counter + coverage floor
  経由の証拠に留める）。`Unobserved{Stale|BorrowConflict}` が
  public engine API から到達不能なら mapping-only 証拠 +
  honest gap を記録する（slice6 先例）。

### B. seek 以外の不連続宣言点（小 API）

slice3/6 spec に記録された deferred 項目。loop transport は
作らない（`RoutingEngine` に loop/restart API は存在しない・
YAGNI）:

- `RoutingEngine::declare_probe_discontinuity()` を追加し
  `self.probes.bump_clock_epoch()` へ delegate
  （`crates/synvelis-player/src/routing_engine.rs`）。
  `Result<(), ProbeRequestError>` を返し、doc で非 seek 意味・
  cursor 非移動・`stop`/`run` 非宣言・device restart = engine
  再生成（fresh context）・`CommandChannelFull` 時の正直な
  error を明記する。
- `crates/synvelis-app/src/engine.rs` に passthrough を追加。
- `crates/synvelis-player/tests/
  sf04_s9_probe_discontinuity_declare.rs` 新規で、非 seek 宣言が
  seek と同じ `ClockEpoch` segment cut・`DiscontinuityInWindow`
  比較挙動を生むこと、transport/PCM を動かさないこと、
  `stop`/`run` が宣言点でないこと、channel full 時に epoch が
  進み deck との乖離が観測可能なことを証明する。

### C. fixture が露呈した adapter gap の修正

fault fixture の設計中に、collector drain が「全 telemetry →
全 chunks」の順で配信するため、`CaptureStopped`/`Removed`/
`RebindDropped` status が同 batch の chunk より先に処理され、
close 前に生成された配信済み PCM chunk が open segment から
切り離されて捨てられる実ギャップを確認した
（`BudgetExhausted` は最終 chunk と status が必ず同 batch）。
`ProbeSession::drain` で segment close を batch 末尾へ遅延させる
（`PendingClose`）修正を `probe.rs` に加える — fixture が実
ギャップを露呈したため許可された変更範囲。

## Why Now

Issue #107 の SF-PROBE-05 は fault 経路の fail-closed 証拠を
要求するが、pool 枯渇・telemetry overflow・観測状態区別・
capture variant の e2e fixture が未整備だった。また slice3/6
spec が「seek 以外の不連続宣言点」を deferred として記録して
おり、loop/restart を作らない範囲での宣言 API が残課題だった。
本 slice が fault 証拠層と宣言点を固定し、#108 UI・#109 実測
gate が乗る土台を完成させる。

## Users and Value

- #108 UI/製品 session 実装者が、probe 喪失（`LostBlocks`/
  `LostRecords`）・無効（`Disabled`）・欠落（`Missing`）・
  測定無音（`Silence`）を区別した正直な観測状態を得られる。
- #109 実測/domain oracle が、capture segment の
  `CaptureSegmentEnd`/`CaptureCompleteness`/gap kind を
  独立検証の入力に使える。
- session/app 層が `declare_probe_discontinuity` 経由で
  seek 以外の transport 不連続（device 再アタッチ等の将来
  宣言点）を probe epoch へ正直に刻印できる。

## Scope

### In Scope

- `crates/synvelis-player/src/routing_engine.rs`:
  `declare_probe_discontinuity` 追加のみ。
- `crates/synvelis-app/src/engine.rs`: passthrough 追加のみ。
- `crates/synvelis-player/src/probe.rs`: fixture が露呈した
  drain batch 内 segment close の遅延修正（`PendingClose`）。
- `crates/synvelis-player/tests/sf04_s8_probe_faults.rs`・
  `sf04_s9_probe_discontinuity_declare.rs`（ともに新規）。
- `docs/` 文書チェーン（spec/design/plan/task/test-spec/
  impl record）。

### Out of Scope

- `syn.probe.compare` app op（#108/外部 gate）。
- SF-PROBE-08/09・SF04-RCP-01・`ui/src/ipc.ts`。
- loop transport・engine restart API・`stop`/`run` の宣言点化
  （cursor を動かさないため宣言点ではない — spec 明記）。
- device restart 時の epoch/context 引き継ぎ（現状 engine
  再生成 = fresh context が正直な扱い）。
- `dispatch.rs`/`trace.rs`/`routing_control.rs`/`driver.rs`/
  `lib.rs`・`pc_c6_oracle_tr_dispatch.rs`・`synvelis-app` の
  他 file・`synvelis-project` の `ops.rs`/`control.rs`・
  `synvelis-recording/**`・`synvelis-workspace` の占有 file
  （変更禁止 — 他 Worker）。
- 実機・#109 Oracle・#108 UI・consumer 照合（外部 gate —
  NotRun）。

## Requirements

- chunk pool 枯渇 fixture が `lost_blocks`・`LostBlocks` status・
  `LostBlocks` gap・`ProbeObservation::Lost`・`Incomplete` を
  実 engine で証明する。
- telemetry overflow fixture が `lost_records` 陽性と比較
  coverage の `Unknown` 降格（fail-closed `Incomparable`）を
  証明する。
- Silence/Disabled/Lost/Missing の区別が bridge update で
  保たれ、drain が `Missing` を捏造しないことを証明する。
- `max_frames` budget・`stop_capture`・remove の 3 variant が
  異なる `CaptureSegmentEnd` で閉じ、二重 capture が
  `AlreadyCapturing` で拒否されることを証明する。
- 到達不能 status（`LostRecords` status・`Unobserved`）は
  mapping の存在と発行不能の理由を文書化し、counter/
  coverage floor 等の到達可能証拠のみで主張する — 捏造しない。
- `declare_probe_discontinuity` が `bump_clock_epoch` へ
  delegate し、非 seek 宣言が seek と同じ `ClockEpoch` cut・
  `DiscontinuityInWindow` を生むこと、cursor/PCM を動かさ
  ないことを証明する。
- `stop`/`run` が宣言点でないこと、device restart が fresh
  context であることを test と doc で明記する。
- channel full 時に `declare_probe_discontinuity` が
  `CommandChannelFull` を正直に返し、broker epoch は進み
  deck との乖離が観測可能なことを証明する。
- drain batch 内で close status より先に生成された chunk が
  segment に計上されること（`PendingClose`）を fixture で
  証明し、既存 sf04 test が無変更で green を維持する。

## Acceptance Criteria

- [ ] `sf04_s8_probe_faults.rs` 5 test・
  `sf04_s9_probe_discontinuity_declare.rs` 5 test が green。
- [ ] `RoutingEngine::declare_probe_discontinuity` +
  `synvelis-app` passthrough が doc 付きで実装される。
- [ ] `probe.rs` の `PendingClose` 修正が segment close の
  batch 末尾適用を行い、既存 `sf04_*` test が無変更で
  green。
- [ ] 到達不能 status の扱いが test コメントと docs に
  記録される。
- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`test-rt`/
  `check-app-native`/`lint-app-native`/`test-msrv`/`build`
  が green。
- [ ] 外部 gate（実機・#109 Oracle・#108 UI・app op・
  SF-PROBE-08/09・consumer 照合）は NotRun として記録
  され、Pass 扱いしない。

## Deferred Design Concerns

- `syn.probe.compare` app op・session 層露出 | `service.rs`/
  `ui_ops.rs`/`app.rs` 他 Worker 占有 | #108 系後続 slice |
  player 層 `compare_probes` が受け口として十分。
- device restart の probe context 引き継ぎ | 現状 engine
  再生成で fresh `ProbeSession`/clock domain — restart を
  同一 cursor series の継続として扱う要件は未確定 | 要件
  確定後の別 slice | 現行は fresh context が正直な扱い。
- `Unobserved{Stale|BorrowConflict}` の e2e 到達 | public
  engine API では unconnected input が `Unavailable` で
  install 時点 reject・static schedule が bound slot を
  毎 callback 書込・`try_borrow` 競合は RT 内部 — 本
  fixture では mapping-only 証拠 | Ruviform 側の fault
  注入経路が必要になれば別 slice | counter
  `unobserved == 0` の主張に留める。
- loop transport | `RoutingEngine` に loop/restart API は
  存在しない・YAGNI | 要件発生時 | 宣言 API は transport
  非依存のため loop 導入時もそのまま使える。
- 実機 fault（デバイス断・late bind・format 変化）| #109
  実測 oracle gate | gate 後 | headless fixture は既定
  `ProbeConfig` のみ。

## Sources and Relations

- Issue: <https://github.com/scarecrow173/synvelis/issues/107>
  （SF-PROBE-05 fault fixtures、deferred「seek 以外の不連続
  宣言点」）。
- Ruviform `typed/probe.rs`/`typed/probe/time.rs` @ `fd1d4d5`:
  `ProbeDeck::emit_capture`（`free.try_pop` → `lost_blocks`）、
  telemetry `try_push` → `lost_records`、`ProbeCollector::
  drain`（telemetry → chunks 順・chunk recycle）、
  `ProbeBroker::bump_epoch`/`wire_clock_mark`（epoch 先行
  bump + `SetClockMark` 配送）、`align_observation_points`
  の `DiscontinuityInWindow`/`EpochMismatch`/`CoverageEmpty`。
- 前提: SF-04 slice1（`ProbeSession`/binding ledger）、
  slice2（`ProbeTraceBridge`/`ProbeTracePort`）、
  slice3（seek → `bump_clock_epoch`・`ClockEpoch` cut）、
  slice5（subscriber ownership）、slice6（`probe_compare`/
  `ProbeCoverage` loss floor — PR #348 merge 済み）。
- 本 spec は Issue #107 のうち SF-PROBE-05 fault fixture と
  不連続宣言点の 2 deliverable のみを対象とし、Issue は
  Open 継続が前提。
