---
id: "SPEC-c4e8b1a2f5d30947e6c1a8b2d4"
type: "spec"
status: "approved"
title: "PC-C4 observe engine wiring — observe_block の engine-driver 配線"
created: "2026-11-15"
updated: "2026-11-15"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/101"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/pc-c4-07-transport-mirror-observe.md"
    - "docs/specs/pc-c4-drain-composition.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-07-transport-mirror-observe.md"
    - "docs/specs/pc-c4-drain-composition.md"
    - "docs/specs/pc-c4-06-control-op-surface.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c4-plugin-parameter-binding-frame-base-capability-gate.md"
  refines:
    - "docs/specs/pc-c4-07-transport-mirror-observe.md"
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/pc-c4-07-transport-mirror-observe-design.md"
    - "docs/test-specs/pc-c4-07-transport-mirror-observe.md"
  defers:
    - "実 VST3/AU/CLAP・OS backend・実 GUI・AudioLens/Ruviform consumer 照合（synvelis#101 残件・外部 gate）"
    - "project.new/open 後の engine 再 attach（synvelis#106 の責務）"
    - "実 RT thread / OS timer への run 周期供給（caller/deterministic driver が供給する既存契約のまま）"
  deferred-by: []
---

# PC-C4 observe engine wiring — `observe_block` の engine-driver 配線

Issue `#101` (PLUG-C4-04 残件) の slice。PC-C4-07
（`docs/specs/pc-c4-07-transport-mirror-observe.md`）で
`RoutingControlDispatch::observe_block` は実装済みだが、呼出しは
`pc_c4_07_*` headless テストのみで、production 経路
（`SessionRoutingEngine::run` → `RoutingEngine::run`）からは駆動
されない。本 slice は RT runtime が処理した各 block の物理 call
window を engine が供給し、attach 済み control surface の
`observe_block` をその場で駆動する配線を行う。

## 背景

- `observe_block(generation, call_start, call_frames)` は node の
  **cursor domain** の call window を要求する。plugin node の
  cursor は attach 時に `EngineFrame(epoch=0, frame=0)` で始まり、
  その plan が active な間の `rt.process` 回数分だけ進む
  （generation 切替で 0 に戻る）。engine の `cursor()` は seek で
  再 anchor される engine domain であり、publish 前に処理済みの
  block 数や generation 活性化タイミングを反映しない — 両者は
  一致しない。
- `observe_cycle`/`observe_block` は呼出しごとに node の report
  channel を drain する。block 処理後にまとめて replay すると、
  全 block 分の receipt/report が最初の window に帰属し、
  transport/plan generation 補正と `last_observed_span` が歪む。
  観測は各 `rt.process` の直後・順序どおりに行う必要がある。
- `SessionRoutingEngine::run` は `RoutingEngine::run` へそのまま
  委譲するだけで、control surface への観測駆動を持たない。
  結果として `observe_block` が production で一度も呼ばれず、
  `feed_control_cycle` への Reported 伝播も `syn.control.status`
  の観測 span 露出も到達不能だった。

## 目的

1. `RoutingEngine` が処理した各 block について、node cursor domain
   の call window（`call_start`/`call_frames`）と実際に走った
   generation を非 RT 側へ報告する observer 経路を追加する。
2. `SessionRoutingEngine::run` がその報告を受けて attach 済み
   `RoutingControlDispatch::observe_block` を block ごとに駆動し、
   merged `DispatchCycle` を buffer する。
3. app 層が buffer を drain して `feed_control_cycle` へ流し、
   `Reported` stage の観測だけが workspace の plugin-report
   channel へ evidence 化される経路を `ui_pump` 周期で駆動する。
4. `syn.control.status` に観測 span の typed evidence
   （`last_observed_span` / `plan_generation`）を露出する。

## スコープ

- `crates/synvelis-player/src/routing_engine.rs`: block 観測 window
  の型・engine 側 node-head 追跡・`run`/`seek` の observer 対応
  variant。
- `crates/synvelis-app`: `SessionRoutingEngine::run` の observer
  配線、観測 cycle buffer と drain accessor、`ui_pump` 周期の
  feed 駆動、`syn.control.status` の `observe` 節、
  `syn.control.observe`/DTO の deferred 記述更新、fixture テスト。
- doc-chain: `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/pc-c4-observe-engine-wiring*`。
- `evidence/sv101/`。

## 非目標（本 slice の範囲外）

- 実 VST3/AU/CLAP plugin backend、OS device backend、実 GUI、
  AudioLens/Ruviform consumer 照合 — 外部 gate であり NotRun として
  報告する。
- 実 RT thread / OS timer への `run` 周期供給 — caller /
  deterministic driver が `run` を駆動する既存契約のまま。
- `project.new`/`project.open` 後の engine 再 attach — synvelis#106
  の責務。
- `syn.control.observe` op の除去・wire schema 変更 — 明示的
  headless/driver drive として残す（deferred 記述のみ実態に更新）。
- `crates/synvelis-player` の `routing_engine.rs` 以外、
  `Cargo.lock`、他 crate。

## 要件

### A. engine 側 call-window 供給（`routing_engine.rs`）

- 処理済み block ごとに `(generation, call_start, call_frames)` を
  observer へ報告する。`generation` はその block を実際に走らせた
  RT `active_generation`（`0` = plan 未活性）。
- `call_start` は **node cursor domain** の frame — plugin node が
  自身の plan 活性化から数える callback head であり、engine
  `cursor()` とは別 domain。generation 切替 block は head `0`
  から数え直す。epoch は attach が mint する `ClockEpochId(0)`
  （routing path は `SetClock`/`DeviceRestart` を送らない）。
- `run` の既存 signature と `PumpPolicy` 意味を変えない — observer
  は `run_observed`（および `seek` の quiesce pass を含む
  `seek_observed`）という opt-in variant で供給し、既存 caller は
  挙動不変。
- node-head 追跡は**全ての** `rt.process` 成功を対象に行う
  （`run` の各 block と `seek` の quiesce pass）— 観測しない
  block でも head は進むため、後続の観測 window がずれない。

### B. app 側 observe 駆動と feed（`synvelis-app`）

- `SessionRoutingEngine::run` が block ごとの window で
  `dispatch.observe_block` を呼び、返った per-node
  `DispatchCycle` を block ごとに merge して buffer する。
  `observe_block` 自身の generation gate（surface generation ≠
  ran generation → `None`）にそのまま乗り、published 未活性・
  stale surface・pre-activation の window は観測されない
  （fail-closed）。
- buffer は `control_drain_observations`（名称は実装で確定）で
  drain し、app が各 cycle を `feed_control_cycle` へ流す。
  `Reported` のみが `feed_plugin_report` へ届く既存規則を維持。
- `ui_pump` が `drain_control_reports` と同じ周期でこの drain を
  駆動する。engine/surface 不在時は no-op。

### C. `syn.control.status` の観測 evidence

- surface の controlled processor ごとに `last_observed_span`
  （最後に投影した `TransportSpan`）と `plan_generation`
  （その span ctx の plan generation）を typed に露出する。
  未観測は `null` — 捏造しない。

## 受入条件

- [ ] `SessionRoutingEngine::run` が処理 block ごとに
  `observe_block` を node cursor domain の正しい head で駆動し、
  publish 前 block・seek・generation 切替を跨いでも head が
  node 実体と一致する（fixture で pin）。
- [ ] staged fast-control/automation の配送が正しい window でのみ
  `Dispatched`/`Reported` に進み、foreign/stale generation の
  window は観測されない。
- [ ] `ui_pump` 周期で drain された cycle の `Reported` 観測が
  `feed_control_cycle` 経由で workspace の plugin-report channel
  に届く。
- [ ] `syn.control.status` が `last_observed_span`/`plan_generation`
  を surface generation と整合して露出する。
- [ ] 変更 crate の canonical 検証（`mise run` 経由
  fmt-check/check/lint/test、player は routing_engine.rs 変更のため
  check/lint/test）が exit 0。
- [ ] 外部 gate（実 plugin/OS/GUI/AudioLens/consumer 照合）は
  NotRun として PR に記載し `Refs #101`。
