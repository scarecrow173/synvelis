---
id: "SPEC-4kvuQeb3IYEZ7UbStoNt2X"
type: "spec"
status: "approved"
title: "SF-04 slice4 — app/session wiring（Probe ON/OFF・probe port 実装・syn.probe.* ops・drain hook）"
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
    - "docs/specs/sf-04-slice1-semantic-probe-adapter.md"
    - "docs/specs/sf-04-slice2-controlsession-probe-trace.md"
    - "docs/specs/sf-04-slice3-clock-epoch-discontinuity.md"
    - "docs/designs/sf-04-slice2-controlsession-probe-trace-design.md"
  defers: []
  deferred-by: []
---

# SF-04 slice4 — app/session wiring（Probe ON/OFF・probe port 実装・syn.probe.* ops・drain hook）

## What Should Be Built

Issue #107（SF-04 Semantic Probe Adapter）の app/session wiring
slice。slice 1-3 が実装した probe 意味論（`RoutingEngine::probe_*`・
`ProbeSession`・`ProbeTraceBridge`・`ControlSession::probe_*`）を
app composition root と `syn.probe.*` IPC 面へ接続する:

- `SessionRoutingEngine`（`crates/synvelis-app/src/engine.rs`）が
  内部で `ProbeTraceBridge` を所有し、composition root 向けに
  scoped な port 供給経路（`with_probe_bridge`）と adapter
  登録/route API を公開する。`Inner.engine: RoutingEngine` を
  `Inner.bridge: ProbeTraceBridge` へ再構成し、既存の engine
  アクセスは `bridge.engine()`/`engine_mut()` 経由に統一する。
- `ProbeTraceBridge`（`crates/synvelis-player/src/probe_trace.rs`）
  に adapter の事後登録 `insert_adapter` を追加し、composition
  root が構築後に node-keyed `DispatchTraceAdapter` と
  target→node route を登録できるようにする（現状は
  `ProbeTraceBridge::new` の初期 map のみで、後から adapter を
  載せる経路がない）。
- `syn.probe.control` op（`service.rs`/`ui_ops.rs`）—
  `request`/`set_mode`/`capture`/`stop_capture`/`remove` の
  lifecycle 操作。`request` は `ControlSession::probe_request`
  経由で session 相関（key→target）を登録し、他は
  `ControlSession::probe_*` 経由で port を呼ぶ。engine 未接続時は
  `SERVICE_ERROR` で fail-closed（捏造した成功・観測を返さない）。
- `syn.probe.state` op — bindings/segments/last drain report/
  traces の投影。engine 未接続時は `engine_attached: false` と
  空の投影を正直に返す。binding の `state`/`requested_mode`/
  `confirmed_mode` をそのまま投影し、Pending（`Requested`/
  `QueuedForBinding`/`PendingRtInstall`）と BoundActive、
  `Unresolved`/`Unavailable`/`Unsupported` を区別して表示できる
  ようにする（Observed Live への格上げはしない）。
- `ui_pump` 末尾の drain hook — routing engine 接続時のみ
  `ControlSession::probe_drain`（session 唯一の drain owner）を
  呼び、runtime probe event を session 相関 target の
  `ControlTrace.observed` スロットへ書き込む。結果は
  `UiOpsState::last_probe_drain` に保持し `syn.probe.state` が
  返す。engine 未接続なら drain しない（port 不在 = 観測なし）。

probe trace は **persist しない**（session のみ）。workspace
`control` state・project revision・undo・graph build counter・
master PCM のいずれも probe lifecycle/view op で変化しない。

## Acceptance Criteria

- [ ] `SessionRoutingEngine` が `ProbeTraceBridge` を所有し、
  `ControlSession::probe_*` へ per-call で `&mut dyn ProbeTracePort`
  を供給できる。
- [ ] `syn.probe.control` で probe ON（request mode=monitor/
  capture）/OFF（set_mode off・remove）が wire 経由で動作する。
- [ ] 同一 Semantic Point への 2 Window（view）からの request は
  session 相関が target ごとに独立し、capture job が重複しない
  （dedup は key=(session,point,kind) で一意）。
- [ ] Change Focus（inspection focus 変更）が capture/probe 状態に
  影響しない — capture dedup が view 切替で破綻しない。
- [ ] `ui_pump` の drain hook が `ControlSession::probe_drain` を
  唯一経路として呼び、`recorded`/`uncorrelated`/`unrouted` を
  `syn.probe.state` で観測できる。
- [ ] Pending Graph（`Requested`/`QueuedForBinding`/
  `PendingRtInstall`）は Observed Live として表示されず、
  未提供状態は `Unresolved`/`Unavailable`/`Unsupported` として
  正確に投影される。
- [ ] engine 未接続の app で `syn.probe.*` が誠実なエラー/空投影を
  返し、捏造した binding や観測を返さない。
- [ ] probe op が project revision・undo・graph_builds・master PCM
  を変化させない。

## Non-Goals（本スライス対象外 — Issue 残件）

- SF-PROBE-04（ruviform#308 待ち）・SF-PROBE-05/07/09（#109 実機
  計測）・SF-PROBE-08/UI（#108 — 2 Window 同一 Semantic Point の
  実 GUI 表示・capture dedup の UI 側証明）。
- workspace/persist への probe state 書き込み — persist しない設計。
- `DispatchTraceAdapter` の実 plugin node への搭載（plugin host
  未接続。adapter 登録経路のみを開ける）。
- Ruviform 側 Driver-neutral probe 契約 — 変更しない。

## Dependencies

- slice1 `ProbeSession`/`SemanticProbeBinding`/segment ledger・
  `RoutingEngine::probe_*`（docs/specs/sf-04-slice1-*）。
- slice2 `ProbeTraceBridge`/`ControlSession::probe_*`/
  `ProbeDrainReport`（docs/specs/sf-04-slice2-*）。slice2 設計
  `docs/designs/sf-04-slice2-controlsession-probe-trace-design.md`
  が app 層 port 供給を本 slice へ defer した箇所を実装する。
- slice3 clock epoch discontinuity・segment cut
  （docs/specs/sf-04-slice3-*）— drain hook はその契約をそのまま
  通過させる。
