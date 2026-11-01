---
id: "PLAN-kco2O0dSUXmFGIGQYqpxrx"
type: "plan"
status: "approved"
title: "VS-B Clock / Gate Scheduler / RT Capture 実装計画"
created: "2026-11-02"
updated: "2026-11-02"
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
    - "docs/specs/vs-b-rt-capture-mock-input-buffer-pool-spsc.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/adr/vs-b-ruviform-rt-spsc-gate-schedule-fault-ownership.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/vs-b-clock-gate-rt-capture-mock-spsc.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/vs-b-e2e-r1-mock-gate-rt-capture.md"
  references: []
  defers: []
  deferred-by: []
---

# VS-B Clock / Gate Scheduler / RT Capture 実装計画 Implementation Plan

## Goal

Issue #14 の E2E-R Gate: Mock Clock/Input の決定的 Frame 列を
`Preflight→Prepared Gate Schedule→RT Capture→事前確保 SPSC Handoff` で扱い、
Normal/Punch/Loop の正確な保存候補と Overflow/Epoch 断絶の安全な
Fail-stop を `synvelis-recording` + `recording-poc` に実装する。

## Scope

単一実装ストリーム（1 plan / 1 worktree）。`synvelis-recording` への
モジュール追加と `recording-poc` のシナリオ拡張のみ。`synvelis-project`
の既存型は再利用し変更しない（必要になった場合は設計差分として記録）。

## Upstream Documents

- `docs/specs/vs-b-rt-capture-mock-input-buffer-pool-spsc.md`（approved）
- `docs/adr/vs-b-ruviform-rt-spsc-gate-schedule-fault-ownership.md`（accepted）

## Design Inputs

- `docs/designs/vs-b-clock-gate-rt-capture-mock-spsc.md`（approved）

## File Map

- `Cargo.toml` - workspace 依存に `ruviform-rt`（git）追加
- `Cargo.lock` - 再生成・追跡（resolved revision 記録）
- `crates/synvelis-recording/Cargo.toml` - `ruviform-rt`、dev-dep `stats_alloc`
- `crates/synvelis-recording/src/bounded.rs` - `BoundedVec<T>` 有界ラッパ
- `crates/synvelis-recording/src/plan.rs` - `RecordingRequest`/`CaptureTarget`/
  `GateSpec`/`BufferBudget`/`PreparedRecordingPlan`/`preflight`
- `crates/synvelis-recording/src/clock.rs` - `InputBlock`・epoch/frame 連続性
- `crates/synvelis-recording/src/gate.rs` - `GateSpan`/`GateSchedule`/
  `GateScheduler`/`evaluate_gate`/`CapturePiece`
- `crates/synvelis-recording/src/capture.rs` - `CaptureChunk`/
  `CaptureBlockHeader`/`CaptureSpanRecord`/`Validity`
- `crates/synvelis-recording/src/handoff.rs` - pool・2 本 SPSC・`FaultLatch`
- `crates/synvelis-recording/src/engine.rs` - `CaptureEngine::process`・
  `schedule_mut`・`teardown`
- `crates/synvelis-recording/src/mock.rs` - `MockPcm`/`MockDriver`/
  `InspectionWriter`/fault injection・`accounting`
- `crates/synvelis-recording/src/lib.rs` - モジュール公開
- `crates/synvelis-recording/tests/*.rs` - 受入・故障・RT 計測テスト
- `crates/recording-poc/src/main.rs` - VS-B シナリオ追加
- `crates/recording-poc/tests/poc_determinism.rs` - 必要に応じ期待値更新

## Implementation Sequence

- [ ] task1: `ruviform-rt`/`stats_alloc` 依存 + `bounded` + `plan`/`clock`
  （preflight 検証・型・連続性チェッカ）
- [ ] task2: `gate`（GateSpan/Schedule/Scheduler・tempo 分割・Loop 展開・
  `evaluate_gate`）+ gate 単体テスト
- [ ] task3: `capture` + `handoff` + `engine`（pool・SPSC・FaultLatch・
  process・retained・teardown）
- [ ] task4: `mock`（MockPcm/MockDriver/InspectionWriter/fault injection/
  accounting）+ driver 単体テスト
- [ ] task5: `recording-poc` VS-B シナリオ + 受入・故障・RT 計測テスト
  全網羅
- [ ] task6: 検証・merge origin/main・生成物再生成・impl record

## Dependencies and Risks

- `ruviform-rt` 初回 fetch にネットワーク必須（既定 branch 追従、lockfile
  記録、`ruviform-media-core` と同一 rev 系に揃うことを確認）。
- `stats_alloc` は dev-dep のみ（GlobalAlloc wrapper は dep 側の unsafe で
  提供され、自前の `unsafe impl` は不要）。global 計測のため専用
  test binary で並行 noise を排除。
- tempo 変換は ruviform `tick_to_media`（NearestEven）の丸め規則に依存。
  試験 tempo は整数 rate を選び丸め差を排除する。
- `unsafe_code = "forbid"` のため固定配列表現は `BoundedVec`（capacity
  固定 `Vec`）で実現。

## Task Breakdown

- task1 → task2 → task3 → task4 → task5 → task6（直列。task3 は task2 の
  evaluate_gate に依存、task4 は task3 の engine/handoff に依存、task5 は
  task4 の driver に依存）

## Verification

- [ ] `mise run fmt-check`/`check`/`lint`/`test`/`build` 全緑
- [ ] `mise run test`（`cargo test --workspace --locked`）が lockfile 再現
- [ ] `recording-poc` を 2 回実行し同一 stdout
- [ ] 全 spec AC を網羅するテスト（test-spec の AC→test 対応表に一致）
- [ ] `mise run test-msrv`（rust 1.89 toolchain が利用可能な場合）

## Open Questions / Gaps

- 実数値（chunk/pool/queue/span/lookahead 容量）はテスト注入値で先行し、
  PoC 計測結果で確定（design の Deferred と同一）。

## Self-Review

- [x] Every spec requirement maps to a task.
- [x] No placeholders remain.
- [x] File paths, relations, status values, and terminology are consistent.

## Implementation Handoff

- Recommended mode: inline execution（subagent 分割不要・単一 worktree 直列）
- Independent work streams: none
- User approval required before dispatch: no（workspace 契約で
  review-protocol が承認 gate を代替、design approved 済）
- If approved: task-doc 単位で順次実装、各 task 完了時に関連テスト実行。

## Review Handoff

- 重点: `engine::process` が RT 契約（非 alloc/lock/I/O/無界）を守るか、
  Fail-stop 分岐が chunk 所有権を失わないか、半開境界で sample の二重
  帰属がないか、#15 が消費する契約（header/spans/validity/fault）が公開
  API として一貫しているか。
