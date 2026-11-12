---
id: "IMPL-6wR3nX8kT5yP2qA7jM4vB9d"
type: "ir"
status: "completed"
title: "SF-04 slice8 実装記録 — SF-PROBE-05 fault fixtures + seek 以外の不連続宣言点"
created: "2026-11-12"
updated: "2026-11-12"
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
    - "docs/tasks/sf-04-slice8-probe-faults-declare-task.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/sf-04-slice8-probe-faults-declare-task.md"
    - "docs/designs/sf-04-slice8-probe-faults-declare-design.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/test-specs/sf-04-slice8-probe-faults-declare.md"
  defers: []
  deferred-by: []
---

# SF-04 slice8 実装記録 — SF-PROBE-05 fault fixtures + 不連続宣言点

## Summary

Issue #107 の bounded deliverable 2 件を実装した。
`sf04_s8_probe_faults.rs` で SF-PROBE-05 の fault 経路
（chunk pool 枯渇・telemetry overflow・観測状態区別・
capture variant・到達不能 status の正直な扱い）を headless
fixture として固定し、`RoutingEngine::
declare_probe_discontinuity` + app passthrough +
`sf04_s9_probe_discontinuity_declare.rs` で seek 以外の
不連続宣言点を追加した。fixture 設計中に `ProbeCollector::
drain` の「全 telemetry → 全 chunks」配信順序が
`CaptureStopped`/`Removed`/`RebindDropped` status を
同 batch chunk より先に処理し、close 前に生成された配信
済み PCM chunk を orphan させる実ギャップを確認したため、
許可条件に基づき `probe.rs` に `PendingClose`（batch 末尾
close 適用）を実装した。master DSP/PCM・port 契約・
`Cargo.lock`・他 Worker 占有 file は変更していない。

## Implemented

### `crates/synvelis-player/src/routing_engine.rs`

- `RoutingEngine::declare_probe_discontinuity(&mut self) ->
  Result<(), ProbeRequestError>` を追加。`self.probes.
  bump_clock_epoch()` へ delegate。doc に非 seek 意味・
  cursor 非移動・`stop`/`run` 非宣言・device restart =
  engine 再生成（fresh context）・`CommandChannelFull` 時の
  正直な error を明記。`seek` の best-effort 呼出しと異なり
  `Result` を返す — 失敗を隠さず caller が retry 判断を持つ。

### `crates/synvelis-app/src/engine.rs`

- `SessionRoutingEngine::declare_probe_discontinuity`
  passthrough を追加（bridge 経由で engine へ届く薄い層）。

### `crates/synvelis-player/src/probe.rs`

- `PendingClose { end: CaptureSegmentEnd, clock: Option<
  ClockContextRef> }` を追加。`ProbeSession::drain` が
  batch ローカルの `BTreeMap<ProbeId, PendingClose>` を持ち、
  closing status は `pending.entry(probe).or_insert(..)` で
  記録するだけに変更（first close wins）。`collector.drain`
  完了後に flush して `close_segment` を適用。
- pending 中の probe は close 済み扱い: 後続 status の epoch
  cut・gap 記録を skip。`observe_meta` は pending 中も chunk
  を計上するが epoch 不一致 record は cut/計上せず discard
  （segment epoch 不変条件を維持）。
- 根拠: `BudgetExhausted` は最終 chunk と `CaptureStopped`
  status が必ず同 callback で生成されるため系統的に発生。
  修正前は `frame_range=None`/`Unknown` を過少申告し得た。

### `crates/synvelis-player/tests/sf04_s8_probe_faults.rs`（新規）

5 test — F1–F5 は `docs/test-specs/
sf-04-slice8-probe-faults-declare.md` 参照。fault 分離
（`lost_blocks` vs `lost_records`）、coverage floor 経由の
fail-closed、`Missing` 非捏造、到達不能 status の assert を
含む。

### `crates/synvelis-player/tests/sf04_s9_probe_discontinuity_declare.rs`（新規）

5 test — F6–F10。非 seek 宣言が seek と同じ `ClockEpoch` cut
・`DiscontinuityInWindow` を生むこと、PCM/`crc` bit-identical
で transport 非干渉、`stop`/`run` 非宣言、channel full 時の
epoch 進行 + deck 未達乖離 + 次 wire での resync を証明。

## Decisions

- **`declare` は `Result` を返す薄い delegate**: `seek` は
  best-effort（`let _ =`）だが、明示宣言 API は失敗を
  隠さない設計。broker `bump_epoch` は push 前に epoch を
  bump するため channel full 時に `time_context` と deck
  `current_mark` が乖離する — 乖離は観測可能で次の成功
  wire が resync する（恒久的不整合にならない）。
- **`PendingClose` は drain batch ローカル**: allocation は
  既存 drain 経路と同程度の bounded 状態。adapter 側の
  同期 close（`remove()`/`reresolve_on_publish`）は drain
  batch を持たないため対象外 — 理論上の取りこぼしは残るが
  本 fixture の対象外として記録。
- **到達不能 status の正直な扱い**: `StatusKind::LostRecords`
  は deck が counter のみ増やし status を発行しない（写像
  済みだが未到達）。`Unobserved{Stale|BorrowConflict}` は
  public engine API から到達不能（unconnected input が
  install 時 `Unavailable`、static schedule が bound slot
  を毎 callback 書込、`try_borrow` 競合は RT 内部）。
  両者とも test で「発行されないこと」を assert し、
  counter/coverage floor 等の到達可能証拠のみで主張。
- **`stop`/`run` は宣言点ではない**: `DriverState` のみを
  変え cursor に触れない。device restart は現状 engine
  再生成で新しい `ProbeSession`/clock domain/epoch 0 の
  fresh context — 旧 series の epoch を引き継がない。
- **loop transport は作らない**: `RoutingEngine` に
  loop/restart API は存在せず YAGNI。宣言 API は transport
  非依存のため将来の loop 導入時もそのまま使える。

## Verification

- `cargo test -p synvelis-player --test
  sf04_s8_probe_faults`: 5 passed / 0 failed。
- `cargo test -p synvelis-player --test
  sf04_s9_probe_discontinuity_declare`: 5 passed / 0 failed。
- 既存 `sf04_*` test 群（`sf04_probe_adapter` 7・
  `sf04_s2_probe_trace_bridge` 3・`sf04_s3_probe_
  discontinuity` 5・`sf04_s7_probe_compare` 10 — 計 25）は
  `probe.rs` `PendingClose` 変更後も無変更で green。
- canonical `mise run fmt-check`/`check`/`lint`/`test`/
  `test-rt`/`check-app-native`/`lint-app-native`/`test-msrv`/
  `build`: 全 lane green（2026-11-12）。`test` 初回実行は
  並行 Worker のビルドによる C: drive 一時満杯
  （os error 112）で `synvelis-library` cache test のみ
  失敗 — 容量回復後の再実行で全 pass。コード起因の失敗
  ではない。
- 外部 gate: 実機・#109 Oracle・#108 UI・`syn.probe.compare`
  app op・SF-PROBE-08/09・consumer 照合・SF04-RCP-01・
  `ui/src/ipc.ts` は **NotRun** — 後続 slice/外部受け入れへ
  委譲。Pass 扱いしない。

## Remaining / Out of Scope

- `syn.probe.compare` app op（`service.rs`/`ui_ops.rs`/
  `app.rs` 他 Worker 占有）。
- device restart の probe context 引き継ぎ（要件未確定 —
  現行は fresh context が正直な扱い）。
- `Unobserved` の e2e 到達（Ruviform 側の fault 注入経路が
  必要になれば別 slice）。
- adapter 側同期 close の queued chunk 取りこぼし（理論上
  残存 — 別 slice の検討事項）。
- loop transport（要件発生時）。
- doc index README 更新は coordinator 統合時処理。
