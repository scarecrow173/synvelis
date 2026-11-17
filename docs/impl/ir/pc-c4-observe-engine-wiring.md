---
id: "IMPL-034XB1Iadg6lT7Tracib0c"
type: "impl"
status: "completed"
title: "PC-C4 observe engine wiring — observe_block の engine-driver 配線"
created: "2026-11-14"
updated: "2026-11-15"
owners: []
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added:
      - type: "file"
        path: "docs/specs/pc-c4-observe-engine-wiring.md"
      - type: "file"
        path: "docs/designs/pc-c4-observe-engine-wiring-design.md"
      - type: "file"
        path: "docs/plans/pc-c4-observe-engine-wiring-plan.md"
      - type: "file"
        path: "docs/tasks/pc-c4-observe-engine-wiring-task.md"
      - type: "file"
        path: "docs/test-specs/pc-c4-observe-engine-wiring.md"
      - type: "file"
        path: "docs/impl/ir/pc-c4-observe-engine-wiring.md"
      - type: "file"
        path: "docs/impl/exp/pc-c4-observe-engine-wiring.md"
      - type: "file"
        path: "crates/synvelis-app/tests/pc_c4_observe_engine_wiring.rs"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/routing_engine.rs"
      - type: "file"
        path: "crates/synvelis-app/src/engine.rs"
      - type: "file"
        path: "crates/synvelis-app/src/ui_ops.rs"
      - type: "file"
        path: "crates/synvelis-app/tests/pc_c4_06_control_op_surface.rs"
      - type: "file"
        path: "docs/impl/ir/pc-c4-07-transport-mirror-observe.md"
  implements:
    - "docs/plans/pc-c4-observe-engine-wiring-plan.md"
    - "docs/tasks/pc-c4-observe-engine-wiring-task.md"
  implemented-by: []
  depends-on:
    - "docs/plans/pc-c4-observe-engine-wiring-plan.md"
    - "docs/specs/pc-c4-observe-engine-wiring.md"
    - "docs/designs/pc-c4-observe-engine-wiring-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/pc-c4-07-transport-mirror-observe.md"
    - "docs/impl/ir/pc-c4-drain-composition.md"
    - "docs/impl/ir/pc-c4-06-control-op-surface.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-observe-engine-wiring.md"
  verified-by: []
  references:
    - "docs/specs/pc-c4-07-transport-mirror-observe.md"
    - "docs/specs/pc-c4-drain-composition.md"
    - "docs/specs/pc-c4-06-control-op-surface.md"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/101"
    - "https://github.com/scarecrow173/synvelis/issues/106"
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/pc-c4-observe-engine-wiring.md"
    rejected: []
---

# PC-C4 observe engine wiring — implementation record

## Summary

synvelis#101 残件 slice: `RoutingControlDispatch::observe_block`
（PC-C4-07 で実装済み・headless test のみが呼出し）を production
経路へ配線した。`RoutingEngine` に node-cursor domain の block
window を報告する `run_observed`/`seek_observed` observer variant
と `ObservedBlock`/`observe_gen`/`observe_head` tracking を追加し、
`SessionRoutingEngine::run` が各処理 block で `observe_block` を
駆動して merged `DispatchCycle` を `pending_observe_cycles` に
buffer する。app は `ui_pump` 周期と pub
`drain_control_observations` で buffer を drain し
`feed_control_cycle` へ流す（`Reported` のみ workspace
plugin-report channel へ）。`syn.control.status` に per-processor
の `observe` 節（`last_observed_span`/`plan_generation`、未観測は
`null`）を追加した。

## Context

- `observe_block` は node cursor domain の call window を要求する
  が、production 経路（`SessionRoutingEngine::run` →
  `RoutingEngine::run`）は window を供給しなかった — 呼出しは
  `pc_c4_07_*` test のみで、`feed_control_cycle` への Reported
  伝播と status の観測 span 露出が到達不能だった。
- node cursor は attach/generation-anchored で cursor 0 開始、
  generation 活性化からの `rt.process` 回数で進み、engine
  `cursor()`（seek で再 anchor）とは別 domain — engine 側で
  post-process `active_generation` を読んで head を追跡する
  設計とした（exp EXP-1）。
- scheduler receipt は drain-once: auto-observation が run 中に
  report channel を消費するため、明示 `syn.control.observe` の
  `receipts` は自身の call が drain した分のみを正直に返す
  （exp EXP-2）。applied-generation 証拠は buffered cycle 側に残る。

## Implementation

- `crates/synvelis-player/src/routing_engine.rs`:
  - `ObservedBlock { generation, call_start, call_frames }` —
    post-process `active_generation` と node-cursor domain の
    `[call_start, call_start + call_frames)`。`call_start` に
    engine `cursor()` を代入してはならない旨を doc に明記。
  - `observe_gen`/`observe_head` field — 直近 block の generation
    と次 callback head の node frame。全 `rt.process` 成功で
    追跡（観測の有無に関わらず head は進む）。
  - `run_observed(blocks, policy, observe)` — `run` と同一の
    process 経路で、各 block の `rt.process` 成功直後に
    `note_processed_block` → observer 呼出し。`run` は no-op
    observer へ delegate（既存 caller の挙動不変）。
  - `seek_observed(engine_frame, observe)` — quiesce pass が実際に
    block を process した場合のみ window を報告（失敗 pass は
    node cursor を進めない）。`seek` は no-op observer へ delegate。
  - `note_processed_block(frames)` — `frames == 0` は `None`。
    post-process `active_generation` が `observe_gen` と異なれば
    その generation の activation block として head=0、同じなら
    `observe_head` から開始。`EngineFrame::new(ClockEpochId::new
    (0), start)` で node domain を mint。
- `crates/synvelis-app/src/engine.rs`:
  - `Inner.pending_observe_cycles: Vec<DispatchCycle>` — block ごと
    の merged cycle を処理順に buffer。
  - `SessionRoutingEngine::run` — `run_observed` の observer 内で
    `dispatch.observe_block(window.generation, window.call_start,
    window.call_frames)` を呼び、返った per-node cycle を
    `merge_observed_cycles` で 1 cycle に merge して buffer。
    `RefCell` borrow は field 分割で scope を限定し、observer 内で
    feed はしない（app が drain cadence を所有）。
  - `control_drain_observations()` — buffer を決定的に drain して
    返す accessor（`#[must_use]`）。
  - `rebuild` の `RoutingPrepare::Built` 分岐 — publish が新しい
    control surface generation を mint するため、旧 surface の
    buffered cycle は stale-generation evidence として clear
    （pre-swap plugin report を現行 evidence として feed しない）。
- `crates/synvelis-app/src/ui_ops.rs`:
  - `drain_control_observations()`（pub、`#[must_use]`）—
    `engine.control_drain_observations()` を回し各 cycle を
    `feed_control_cycle` へ。`Reported` stage のみが
    `feed_plugin_report` へ届く既存規則を維持。戻り値は feed
    成功数。
  - `ui_pump` 末尾 — `drain_control_reports` の直後・`drain_probe`
    の前に周期配線（host report drain が ledger を進めた後なので、
    block 中に plugin が出した report は次の observed block で
    `Reported` になり得る）。
  - `observe_status_json` + `syn.control.status` の `observe` 節 —
    controlled processor 毎に `last_observed_span`
    （`dispatch.last_observed_span` verbatim の `TransportSpan`
    JSON）と `plan_generation`（span ctx の値）。未観測は `null`、
    engine/surface 無しは空 map — 捏造しない。
  - `syn.control.observe`/DTO の deferred 記述を実態に更新 —
    automatic drive が production 経路になったこと、receipt の
    drain-once 意味を明記。
- `crates/synvelis-app/tests/pc_c4_observe_engine_wiring.rs`（新規）:
  fake plugin（announce/echo）が production composition path と
  実 app/session fixture を通る 4 test — block 毎 cycle buffer、
  Reported→workspace feed（`Estimated` stamp）、`ui_pump` 自動
  feed、node cursor domain の status 追跡（generation swap で
  `[0,256)` restart・fresh plugin instance）。
- `crates/synvelis-app/tests/pc_c4_06_control_op_surface.rs`:
  `observe_reports_stages_and_feeds_only_reported` を drain-once
  意味に合わせて更新 — applied plan generation の receipt 証拠は
  `control_drain_observations` の buffered cycle で assert、
  明示 op の `receipts` 空を明示 assert。

## Decisions During Implementation

- **observer は opt-in variant**: `run`/`seek` の signature と
  `PumpPolicy` 意味を変えず、`run_observed`/`seek_observed` で
  供給。player 層の `run` は no-op observer で挙動不変、
  session engine のみが observe を配線する。
- **buffer + app-owned feed**: observer callback 内で workspace
  feed すると `RefCell` borrow が衝突するため、engine は cycle を
  buffer し app が `ui_pump`/明示 drain で feed する。
- **node-head は全 process 成功で追跡**: 観測しない block
  （surface 無し・generation 不一致）でも node cursor は進むため、
  head 追跡を observe 成否から独立させた。
- **publish 時に pending cycle を clear**: generation swap で旧
  surface の buffered cycle は stale evidence — feed 前に捨てる
  fail-closed。

## Related Experiments

### Adopted

- `docs/impl/exp/pc-c4-observe-engine-wiring.md` — EXP-1（node
  cursor domain の選択）と EXP-2（receipt drain-once の整理）を
  採用。engine `cursor()` の代用と peek 型 drain を却下した根拠を
  記録。

### Rejected

なし。

## Validation

- `mise run fmt-check` — exit 0。
- `mise run check synvelis-app` / `mise run check
  synvelis-player` — exit 0（`--locked --all-targets`）。
- `mise run lint synvelis-app` / `mise run lint
  synvelis-player` — exit 0（clippy `-D warnings`）。
- `mise run test synvelis-player` — exit 0（全 test target green、
  `pc_c4_07_*` 非回帰を含む）。
- `mise run test synvelis-app` — exit 0（全 test target green、
  `pc_c4_observe_engine_wiring` 4 test・`pc_c4_06` 更新 test を
  含む）。
- canonical lane の最終結果は PR 本文「実行した検証」に実測値で
  記録。

## Risks

- node-head tracking は engine が `rt.process` 成功を観測する
  ことに依存する — `rt.process` が部分的に失敗して node cursor
  だけ進む経路があると window がずれるが、process は block 単位
  で atomic（失敗時は `Err` で早期 return、head 未更新）という
  既存契約に依存。seek の quiesce pass も同じ規則。
- `pending_observe_cycles` は ui_pump 周期まで buffer される —
  長時間 run 連続呼出しで buffer が伸びるが、block 数は caller
  駆動で bounded（run 1 回あたり blocks 件）。
- generation 切替 block の window は post-process generation で
  mint される — activation が process head で起きる既存 RT 契約
  に依存（PC-C4-07 と同じ前提）。

## Follow-ups

- 実 VST3/AU/CLAP plugin backend・OS device backend・実 GUI・
  AudioLens/Ruviform consumer 照合 — synvelis#101 残件、外部 gate
  で NotRun。
- `project.new`/`project.open` 後の engine 再 attach —
  synvelis#106 の責務。
- 実 RT thread / OS timer への `run` 周期供給 — caller /
  deterministic driver が供給する既存契約のまま。
- `seek` の app/UI 配線 — `SessionRoutingEngine` は現状 `run`
  のみを駆動し、`seek_observed` は opt-in API として残る
  （app 層に seek 経路が無いため本 slice では未配線）。
