---
id: "TSPEC-1W8mH6ZO7NttYUnbbJOgke"
type: "test-spec"
status: "approved"
title: "SF-04 slice4 受入テスト仕様 — app/session wiring（syn.probe.*・drain hook）"
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
  implements: []
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/sf-04-slice4-app-wiring-design.md"
  derived-by: []
  verifies:
    - "docs/specs/sf-04-slice4-app-wiring.md"
    - "docs/designs/sf-04-slice4-app-wiring-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-04 slice4 受入テスト仕様 — app/session wiring（syn.probe.*・drain hook）

## Purpose

SF-04 slice4（Issue #107 の app/session wiring）が、wire 面で
Probe ON/OFF・capture lifecycle・drain→trace 記録を正しく
仲介し、engine 未接続時に誠実な失敗/空投影を返し、project・
revision・graph build・PCM を一切変えないことを headless fixture
で保証する。

## Feature/Rule/Example

### F1 — `syn.probe.control` request/set_mode/remove wire 経路

- Rule: `action:"request"` が `ControlSession::probe_request` を
  通り session 相関 key を返す。`set_mode:"off"`・`remove` が
  後続 op で受理される。`key` は request 戻り値の serde 形を
  そのまま往復する。
- Example: `tests/sf04_probe_wiring.rs` で `UiRuntime::dispatch_op`
  または `SynvelisApp` 直呼びにより request→state→set_mode→
  remove を実行し、binding state が
  `requested`→（RT drain 後）→`stopped` と遷移することを確認。

### F2 — engine 未接続の fail-closed

- Rule: `routing_engine` が `None` の app で lifecycle op は
  `SERVICE_ERROR`、state op は `engine_attached:false` + 空投影。
  捏造した binding/観測を返さない。
- Example: engine を attach しない `SynvelisApp` で両 op を呼ぶ。

### F3 — drain hook は `ui_pump` 唯一経路

- Rule: probe op 末尾の `ui_pump` が `ControlSession::probe_drain`
  を呼び、`last_probe_drain` report が `syn.probe.state` に
  反映される。read op（state）は drain を起こさない。
- Example: engine `run` で RT event を発生させた後、任意の UI op
  または明示 `ui_pump` で drain が走り `drain.updates>0`・
  `recorded`/`unrouted` が説明可能であること。

### F4 — 同一 Semantic Point の複数 target 相関分離

- Rule: 同一 `(point,kind)` に 2 target を request しても key は
  1 つ（dedup）で、drain update が両 target に fan-out する。
  capture 重複なし。
- Example: 同一 point+kind で target A/B に request → 1 key が
  返り、`probe_targets` が両者を含む。drain 後 `unrouted` は
  adapter 未搭載のため `recorded=0`/`unrouted>0` でも
  `uncorrelated=0`（相関は取れている）。

### F5 — 不変条件

- Rule: probe lifecycle/view op が project revision・undo・
  `graph_builds`・master PCM（receipt/play report）を変えない。
- Example: op 前後で `project_info` revision・engine
  `graph_builds()`・receipt が一致。

### F6 — Pending/未提供の正直な投影

- Rule: `syn.probe.state` の binding 投影が
  `ProbeResolveState`（kebab-case）と `requested_mode`/
  `confirmed_mode` を区別し、pending 状態を Observed Live に
  見せない。`Unsupported`/`Unavailable`/`Unresolved` がそのまま
  投影される。
- Example: 未解決 entity（存在しない strip id）への request が
  port error または `unresolved` state で返り、`bound-active`
  を偽装しない。

## Out of Scope

- 実 plugin node への `DispatchTraceAdapter` 搭載・実機計測・
  実 OS/native plugin（#109/#114、NotRun）。
- GUI 表示・SF-PROBE-08 UI（#108）。
- `record_probe` が実 trace に届く end-to-end（adapter 登録経路は
  開けるが、本 slice の fixture は adapter なしで unrouted を
  正直に報告する）。
