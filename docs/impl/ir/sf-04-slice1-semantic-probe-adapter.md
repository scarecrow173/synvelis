---
id: "IMPL-DVuQSd17Juc0oZ9flKLQ6O"
type: "impl"
status: "completed"
title: "SF-04 slice1 実装記録 — Semantic Probe Adapter 基盤（project/player 層 + ruviform pin bump）"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/107"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-project/src/probe.rs"
      - type: "file"
        path: "crates/synvelis-project/tests/sf04_project_probe.rs"
      - type: "file"
        path: "crates/synvelis-player/src/probe.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/sf04_probe_adapter.rs"
      - type: "file"
        path: "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
      - type: "file"
        path: "docs/designs/sf-04-slice1-semantic-probe-adapter-design.md"
      - type: "file"
        path: "docs/plans/sf-04-slice1-semantic-probe-adapter.md"
      - type: "file"
        path: "docs/tasks/sf-04-slice1-semantic-probe-adapter.md"
      - type: "file"
        path: "docs/test-specs/sf-04-slice1-semantic-probe-adapter.md"
      - type: "file"
        path: "docs/impl/ir/sf-04-slice1-semantic-probe-adapter.md"
    modified:
      - type: "file"
        path: "crates/synvelis-project/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-player/src/routing_engine.rs"
      - type: "file"
        path: "Cargo.lock"
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
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/tasks/sf-04-slice1-semantic-probe-adapter.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-04-slice1-semantic-probe-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "https://github.com/scarecrow173/synvelis/issues/106"
    - "https://github.com/scarecrow173/synvelis/issues/108"
    - "https://github.com/scarecrow173/synvelis/issues/109"
    - "https://github.com/scarecrow173/ruviform/issues/308"
    - "https://github.com/scarecrow173/ruviform/issues/309"
  refines: []
  refined-by: []
  derives-from:
    - "docs/plans/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/designs/sf-04-slice1-semantic-probe-adapter-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/sf-04-slice1-semantic-probe-adapter.md"
  verified-by: []
  references:
    - "docs/specs/sf-03-slice1-semantic-lowering-source-map-activation.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# SF-04 slice1 実装記録 — Semantic Probe Adapter 基盤

## Summary

Issue #107 SF-04 の slice 1 として、semantic probe contract
（`synvelis-project/src/probe.rs`）と Ruviform probe runtime への
adapter（`synvelis-player/src/probe.rs` + `RoutingEngine` 配線）を
実装した。ruviform pin を `cc7d5296` → `0f5b5737`（#308
RebindReceipt/CapturePolicy・#309 clock/epoch/latency 契約を含む
main HEAD）へ bump し、lockfile・UI vendor REV を同期した。
headless テスト 15 件（project 8 + player 7）で解決行列・
stale-generation 拒否・generation 境界での capture segment cut・
semantic point 移動時の再 install を非 RT 層で実測した。

## Context

Issue #107 は SF-PROBE-01..09 のエピックで、本 dispatch は
project/player 層の基盤に限定（UI は #108、実測は #109、
app/session 接続は後続 integration slice）。SF-03 slice 5 までの
`RoutingSourceMap`/`RoutingBuildKey`/`SemanticNodeRef`/
`RuntimeInstanceId`/`RuntimeClockEpochRef`/`RoutingEngine`/
`source_map()` を前提として消費する。spec
`sf-04-slice1-semantic-probe-adapter.md`・design・plan・tasks・
test-spec に従う。

## Implementation

### Changes

- `crates/synvelis-project/src/probe.rs`（新規）: semantic probe
  contract。`SignalBoundary`（PreInsert/PostInsert/PreFader/
  PostFader/Input/Output/Sidechain）・`SemanticSignalEntityRef`
  （Track/Bus/Strip/Processor/Send）・`SemanticProbePoint`・
  `ProbeSubscriptionKey`・`ProbeMode`・`SubscriptionKind`・
  `ProbeResolveState`・`SemanticProbeBinding`・
  `ProbeStatusReceipt`/`ProbeStatusKind`・`ProbeStopReason`・
  `ProbeUnobservedReason`・`ClockContextRef`・
  `ProbeFrameInterval`/`ObservationGap(Kind)`・
  `CaptureCompleteness`・`CaptureSegmentEnd`・
  `ProbeCaptureSegment`・`ProbePointResolution`。
  `resolve_probe_point(project, map, spec, point)` は pure で
  fail-closed: strip/track/bus は semantic port または fader
  node 内部位置（PostInsert=fader input、PreFader=fader pre-out）
  に解決し、processor は main in/out + sidechain（明示 port または
  一意 sidechain のみ）、send は `map.send_taps` + 接続先 semantic
  port。明示 `point.port` は boundary 選択と一致する場合のみ有効
  （disambiguate であって override ではない）。物理 slot が無い・
  非 audio・曖昧・未 lowering は `Unavailable`/`Unsupported`/
  `Unresolved`/`NotLowered` をそのまま返し、PCM やゼロ値の捏造は
  しない。
- `crates/synvelis-project/src/lib.rs`: probe module 登録と
  contract 型の re-export。
- `crates/synvelis-player/src/probe.rs`（新規）: `ProbeSession` —
  非 RT の `ProbeBroker`/`ProbeCollector` pair + binding ledger +
  capture segment ledger。`ProbeSession::pair(config, block_frames,
  sample_rate)` が RT `ProbeDeck` を返し、`RoutingEngine::new` が
  `rt.set_probe_deck` で install。`request`/`set_mode`/`capture`/
  `stop_capture`/`remove`/`drain`/`on_publish`/`bindings`/`segments`。
  requested_mode と confirmed_mode を分離し、`drain` は ledger を
  RT evidence で更新してから caller の callback に event を渡す。
  `RecordMeta` は segment generation と一致する場合のみ frame
  range に畳み込み、clock mark は `is_unknown()` なら `None`
  （unknown は known と等価にならない）。未来の runtime enum
  variant は `Unknown` 系 contract variant に写像（推測しない）。
- `crates/synvelis-player/src/routing_engine.rs`: `published_spec`
  と `probes: ProbeSession` を保持。`new` で `chunk_frames =
  block_frames.max(1)` の `ProbeConfig` から pair を作り deck を
  install。`publish` は source map install 直後に
  `probes.on_publish(&self.graph, &spec, project, map)` を呼び、
  全 installed semantic point を新 map/spec で再解決する —
  移動した point は remove+install で置き換え、消えた point は
  probe を外して stale slot を観測させない。`probe_request` は
  plan 未 publish で `NoActivePlan`、別 project で
  `ProjectMismatch`。`probe_set_mode`/`probe_capture`/
  `probe_stop_capture`/`probe_remove`/`probe_drain`/
  `probe_bindings`/`probe_segments` の forwarding API を公開。
- `crates/synvelis-player/src/lib.rs`: `pub mod probe` +
  `ProbeRequestError`/`ProbeSession` re-export。
- pin bump（`10a1bea`）: ruviform git 依存を `0f5b5737` へ。
  `Cargo.lock` 26 source・UI vendor REV 同期。
  `cargo check --workspace --all-targets` で API 破壊なしを確認
  （独立コミット、破壊内容なし）。

### Decisions During Implementation

- **semantic 再解決を publish 時に行う**: ruviform #308 の staged
  rebind は broker が保持する *runtime* `ProbePoint` を再 bind する
  だけで、topology 変更で semantic point が別 slot を指す場合に
  旧 slot を観測し続ける危険がある。adapter は publish ごとに
  `resolve_probe_point` を新 map/spec で再実行し、移動時は
  remove+install、消失時は明示 remove（`RebindDropped` より先に
  adapter が segment を `GenerationBoundary` で閉じる）。
- **capture の世代跨ぎ継続を禁止**: broker default の
  `CapturePolicy::StopAtBoundary` をそのまま使い、再 install される
  probe は requested_mode が `Capture` でも `Monitor` で install
  する（requested は binding に保持、confirmed は RT evidence）。
  silent auto-recapture は行わない。
- **`probe_request` の project 照合**: `map.project_id` と
  引数 `project.project_id` を比較し、不一致は `ProjectMismatch`。
  `Project` は revision を持たないため live revision との照合は
  できず、解決は publish 時に保存した map/spec ペアに対して行う
  （ID 単調増加のため混在は fail-closed に倒れる）。
- **future variant の写像**: ruviform 側 `StopReason`/
  `UnobservedReason`/`PointAvailability`/`StatusKind` は
  `#[non_exhaustive]` — 未知 variant を既知値に推測せず、
  contract 側に `Unknown` variant（`ProbeStopReason`/
  `ProbeUnobservedReason`/`CaptureSegmentEnd` に追加、
  `ProbeStatusKind::Unknown`/`project_mode → None` は既存）へ
  写像する。
- **`ProbePointResolution` に `#[non_exhaustive]` 追加**: 公開 enum
  policy 適合のため。player 側 match に fail-closed wildcard
  （`Unavailable`）を追加。

## Related Experiments

なし。

## Validation

- `mise run fmt-check` / `mise run check` / `mise run lint` /
  `mise run test` / `mise run test-rt` /
  `mise run check-ruviform-rev` / `mise run check-headless-deps`:
  全て exit 0（Windows、toolchain 1.99.0）。
- `cargo test -p synvelis-project --test sf04_project_probe`:
  8/8 pass。`cargo test -p synvelis-player --test
  sf04_probe_adapter`: 7/7 pass（補助証拠、canonical は
  `mise run test`）。
- headless 実測: strip boundary 解決・processor main/sidechain・
  send tap/destination・明示 port mismatch 拒否・mapping state
  （Unavailable/Unsupported/NotLowered/Unresolved）の正直な
  surfacing・fader slot 欠落/曖昧/非 audio の fail-closed・
  request→queued→RT-confirmed lifecycle・stale generation command
  の reject+rebind・generation 境界での segment cut（
  `GenerationBoundary`、再 capture なし）・semantic point 移動時の
  probe 再 install（新 `RuntimeProbeId`）・remove の `Stopped`
  lifecycle・plan 未 publish/別 project の request 拒否。

## Risks

以下は NotRun/対象外項目を含む残存リスク。

- `synvelis-app`/`synvelis-workspace` への probe 配線は後続
  integration slice（本 dispatch の編集禁止範囲）。現状 probe API
  は `RoutingEngine` の pub method としてのみ到達可能。
- UI/Monitor UX（#108）・実デバイス/実 RT 実測（#109）・複数
  window ownership・比較診断は対象外。
- `request` の idempotency は key 単位 — 同一 key を別 mode で
  再 request しても binding は上書きされず `set_mode` が必要
  （仕様として docs に明記）。
- publish 時の `on_publish` error（command channel full 等）は
  publish を失敗させず `let _ =` で捨てる — binding は前世代の
  まま残り RT `Unbound` → `StaleGeneration` で観測される設計
  （コメントで明記）。
- unresolved/unavailable で install されなかった binding は
  publish 時の再解決対象外（`probe_ids` を持たない）— 回復には
  明示的な再 `probe_request` が必要。self-healing は follow-up
  候補。

## Follow-ups

- `synvelis-app`/`synvelis-workspace` への probe 配線は後続
  integration slice（本 dispatch の編集禁止範囲）。probe API は
  `RoutingEngine` の pub method としてのみ到達可能。
- SF-04 の他 slice（control session probe observation/trace
  wiring、probe clock/epoch discontinuity・capture
  segmentation、subscriber window ownership、probe compare、
  probe faults 等）は `docs/impl/ir/sf-04-slice*.md` に個別の
  IR として記録済み。
- UI/Monitor UX（#108）・実デバイス/実 RT 実測（#109）・複数
  window ownership・比較診断は後続 dispatch の対象。
- unresolved/unavailable で install されなかった binding の
  publish 時再解決（self-healing）は follow-up 候補 — 現状は
  明示的な再 `probe_request` が必要。
