---
id: "DESIGN-7c4e9a1b2d3f4e5a6b7c8e9f"
type: "design"
status: "approved"
title: "PC-C4-06 control op surface design — syn.control.* typed-evidence ops"
created: "2026-11-12"
updated: "2026-11-12"
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
    - "docs/specs/pc-c4-06-control-op-surface.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-06-control-op-surface.md"
    - "docs/designs/pc-c4-04-app-composition-design.md"
    - "docs/specs/pc-c4-04-app-composition.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/pc-c4-03-param-observation-design.md"
  refines:
    - "docs/specs/pc-c4-04-app-composition.md"
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-06-control-op-surface.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/pc-c4-06-control-op-surface.md"
    - "docs/designs/pc-c4-04-app-composition-design.md"
    - "docs/specs/pc-c4-04-app-composition.md"
  defers: []
  deferred-by: []
---

# PC-C4-06 control op surface design — syn.control.* typed-evidence ops

## Context

PC-C4-04 で `SynvelisApp`/`SessionRoutingEngine` には control surface
の全機能（catalog vend・attach/sync・`control_tick`・
`control_fast_control`・`control_observe_cycle`・
`feed_control_cycle`・`with_control_dispatch` seam）が実装済みだが、
`service.rs` の `OPS`/`dispatch` には `syn.control.dispatch` /
`subscribe` / `unsubscribe` / `targets` のみが登録され、deferred
API は service 境界に露出していない（pc-c4-04 IR の NotRun 明記
どおり「`syn.*` op 登録は別 Worker」）。

本 design はその deferred API を `syn.control.*` op として登録する
際の wire schema・所有境界・honest-evidence 規約を定める。
判断の根拠は pc-c4-04 が確立した「app は typed evidence の
pass-through surface であり、物理 call partition・host borrow・
plugin 値を invent しない」という境界規約の延長である。

## Scope

### In Scope

- `service.rs`: `OPS` への 3 エントリ追加と `dispatch` の 3 route。
- `ui_ops.rs`: op handler 3 件・`note_control_drain`・wire
  projection/parse helper 群・`UiOpsState.last_control_drain`。
- `engine.rs`: `control_frame_base` / `control_last_block`
  read-only accessor。
- `synvelis-app/Cargo.toml`: `ruviform-plugin-core` を
  `[dependencies]` へ（Coordinator 承認済み — 下記判断記録）。
- `crates/synvelis-app/tests/pc_c4_06_control_op_surface.rs`: 新規
  headless fixture。

### Out of Scope

- `observe_cycle` の per-block call-window 自動供給、
  `drain_output_reports` の engine 配線、production composition
  root の `PluginHost` 接続 — 全て後続 slice。
- `synvelis-player`/`synvelis-library` src、doc index、`mise.toml`。
- 実 plugin backend・実 GUI・AudioLens・oracle gate（NotRun）。

## Components and Boundaries

### `service.rs` — op registry / dispatch

既存規約どおり `OPS: &[OpInfo]` に `name`/`summary` を追加し、
`AppService::dispatch` の match に 3 arm を追加する。route は
`self.app.op_control_*` を呼ぶだけで、wire schema の解釈は
`ui_ops.rs` 側に閉じる（既存 `op_control_*` と同じ配置）。

### `ui_ops.rs` — handlers + wire shapes

#### `syn.control.fast_control`

params（全て必須）:

| key | 型 | 意味 |
| --- | --- | --- |
| `target` | tagged `ParameterTargetId` | bind 済み semantic target |
| `value` | f64 | normalized host value |
| `means` | string | `"timed_event"`/`"block_boundary"`/`"immediate"`/`"per_note"` |
| `position_frames` | i64 | 配信位置（engine frames、surface `frame_base` の tick） |
| `project_revision` | u64 | intent 作成時の project revision（correlation 証拠） |

処理: `ParameterTargetId::from_tagged` → `ParamWriteMeans` parse →
`control_frame_base()` で verified base 取得（`None` →
`service_error`）→ `FastControlWrite { position:
MediaTime::new(position_frames, frame_base) }` 構成 →
`control_fast_control` → `ui_pump`。戻り値は
`FastControlOutcome` の投影 `{"outcome": "delivered"}` /
`{"outcome":"queued","depth":n}` /
`{"outcome":"blocked","block":<kind>,"block_detail":<Display>}`。
`block_detail` は `control_last_block()`（`last_block()` の
`Display`）で、`Blocked` 時のみ付与する。

`means` の 4 値は `ParamWriteMeans` 全 variant を verbatim に
写像する。`immediate`/`per_note` は contract が honor できず、
player 側 `check_param_write` が typed `UnsupportedMeans` で
reject → `Blocked(Lowering)` となる。op 側で downgrade・
fallback はしない。

#### `syn.control.status`

params なし。`&self` の read-only snapshot:

```json
{
  "engine": bool,
  "generation": u64|null,
  "active_generation": u64|null,
  "frame_base": {"numerator":u32,"denominator":u32}|null,
  "last_error": string|null,
  "surface": SurfaceReport|null,
  "tick": TickReport|null,
  "drain": ReportDrainReport
}
```

- `active_generation` は `rt_status().active_generation` を読み、
  `0`（未観測）は `null` に写像 — engine の既存
  `observed_active_generation` と同じ規約で「観測なし = 常に
  gated」を偽らない。
- `drain` は `UiOpsState.last_control_drain: ReportDrainReport`
  を常に返す。初期値は `ReportDrainReport::default()`
  （`attached:false`）で「drain 未実施」を正直に示し、
  composition root が `note_control_drain` で記録した最新値のみが
  実 drain を反映する。
- `SurfaceReport`/`TickReport`/`ReportDrainReport` は counter・
  generation・processor tagged id を verbatim に投影する。
  `ReportDrainReport.errors` の `HostError` は contract serde を
  持たないため debug 表記の string として返す（typed pair
  `(processor, error)` の対応は保持）。

#### `syn.control.observe`

params: `processor`（tagged `ProcessorId`）・`generation`
（surface generation）・`call`（`{epoch,frame,frames}` = 物理 call
window）・`span`（`TransportSpan` 全 field: start/end frame・
status・position_ticks/base・loop_ticks・occurrence・
clock_epoch・transport_generation・plan_generation・
tempo_revision・deliverable）。

処理: `ObserveParams` DTO を `serde_json::from_value` で parse
→ `ProcessorId::from_tagged` → `transport_span_param` で
`TransportSpan` 構成（`status` は `"stopped"|"playing"|"paused"`、
`position_base` は `TimeBase::new` 検証済み）→ engine の
`control_generation()` が surface generation と一致することを
確認（不一致は `service_error` で observe 前に fail-closed）→
`control_observe_cycle(processor, generation, &span, call_start,
call_frames)` → `feed_control_cycle(&cycle)`（`Reported` のみ
feed、既存実装が強制）→ `ui_pump`。

戻り値:

```json
{
  "observations": [{"target","stage","value","position","revision", ...stage fields}],
  "receipts": [{"status","plan_generation","transport_generation","clock_epoch","applied_frame"}],
  "fed": usize
}
```

- observation の `stage` は `DispatchStage` を verbatim に
  `"scheduled"|"dispatched"|"dropped"|"reported"|"unavailable"`
  へ写像し、`dispatched` は `call_start`/`frame_offset`、
  `dropped` は reason debug を併記する。`value` は
  `obs.value.map(ParamValue::get)` — stage が値を持つ場合のみ
  number、それ以外は `null`。
- `receipts` は `AppliedReceipt` の typed field を構造化して返す。
  `plan_generation` は caller が live plan generation を学ぶための
  scheduler truth であり、driver の `TransportMirror` が消費する
  のと同じ証拠を service 境界でも提供する（debug 文字列ではなく
  構造化 JSON — caller が次の span を組み立てられるようにするため）。
- `fed` は `feed_control_cycle` の戻り値 — `Reported` stage かつ
  value を持つ observation のみが `ControlSession::feed_plugin_report`
  へ渡った件数。

#### `note_control_drain`

`pub fn note_control_drain(&mut self, report: ReportDrainReport)`。
composition root が `with_control_dispatch` seam 経由で実行した
`drain_output_reports` の戻り値をそのまま `last_control_drain` に
記録する。app は drain 自体を実行しない — `PluginHost` borrow は
composition root の所有であり、engine の `with_control_dispatch`
closure seam がその境界である（pc-c4-04 design の判断を維持）。

### `engine.rs` — accessors

- `control_frame_base() -> Option<TimeBase>`: attached dispatch の
  verified `frame_base()`。fast-control write の `position` stamp
  と status の `frame_base` に使う。
- `control_last_block() -> Option<String>`: `last_block()` の
  `Display`。`Blocked` outcome の typed detail。

いずれも既存 `control_report`/`control_generation`/
`last_control_error` と同じ read-only・`RefCell` borrow・
`None`=未装着の規約。

## Data and Control Flow

```
caller (GUI/driver/CLI)
  └─ UiRuntime::dispatch_op("syn.control.*", params)
       └─ AppService::call → dispatch → SynvelisApp::op_control_*
            ├─ fast_control: params → FastControlWrite(frame_base stamp)
            │    → RoutingControlDispatch::fast_control → FastControlOutcome → JSON
            ├─ status: &self snapshot → SurfaceReport/TickReport/DrainReport → JSON
            └─ observe: params → TransportSpan+call → observe_cycle
                 → DispatchCycle → feed_control_cycle(Reported only) → JSON
```

## Key Decisions

1. **`position` は caller の frame 数 + surface verified
   `frame_base`**: `MediaTime` を wire で直接受けず、engine frame
   数を受けて app が verified base で stamp する。base を caller が
   主張する経路を作らず、surface が pin した attach-time base と
   常に一致する（pc-c4-04 の frame_base 契約の維持）。
2. **observe の span/call window は全て caller 証拠**: RT runtime
   内の物理 call partition は app には観測不能であり、捏造しない。
   `plan_generation`/`transport_generation` も caller が receipts
   等の typed evidence から学んで供給する — op は検証済み surface
   generation の一致のみを gate し、span の中身を invent しない。
3. **drain は composition root のまま**: `drain_output_reports` は
   `PluginHost` borrow を必要とし、それは app の所有外。engine の
   `with_control_dispatch` seam が host 型を app から隔離する既存
   境界であり、本 slice は `note_control_drain` で結果の typed
   report のみを status へ橋渡しする。drain 未実施を `attached:
   false` の default で正直に示す。
4. **`ruviform-plugin-core` を `[dependencies]` へ**:
   `FastControlWrite.means: ParamWriteMeans` は
   `ruviform-plugin-core` の型であり、従来 dev-dependency のみで
   lib code から名指しできなかった。Coordinator 承認済みで
   `[dependencies]` に追加（dev-dep 重複は除去）。代替案
   （string → player 側 parse、re-export 新設）は型安全性を
   損なうか player 変更を要するため不採用。
5. **wire 投影は verbatim + 最小 string 化**: contract serde を
   持たない型（`HostError`・`DropReason`・`ApplyStatus`・
   `FastControlBlockKind`）のみ debug/variant 名の string に落とし、
   構造化 counter・generation・tagged id・`Option` の
   null/値区別は保持する。`reported` と `dispatched` の stage
   区別は wire でも偽装しない。

## Non-Functional Constraints

- 非 RT: op handler は UI/host thread で実行され、RT callback とは
  無関係。`observe_cycle`/`drain` は non-RT contract のまま。
- fail-closed: surface 未装着・generation 不一致・未知
  processor・malformed params は全て typed error で、部分的な
  観測や捏造値を返さない。
- 依存方向: `synvelis-app → ruviform-plugin-core` は player が
  既に依存する leaf crate であり、層違反なし。

## Risks and Trade-offs

- **plan_generation の caller 供給は caller 負荷**: live plan
  generation を知るには receipts 等の evidence が必要であり、
  初見 caller は試行が要る。receipts を構造化 JSON で返すことで
  driver と同じ学習経路を提供する。RT 内自動供給は deferred で
  あり、誤った generation の span は投影 gate で自然に
  `Scheduled` 残留となる（誤配送ではなく未投影 — fail-safe 側）。
- **`block_detail` の string 化**: `FastControlBlock` は contract
  serde を持たず `Display` のみ。kind（`Copy` enum）は wire で
  構造化済みのため、detail は診断用 string であり evidence の
  主張は kind が担う。
- **dev-dep → dep 移動**: `ruviform-plugin-core` は既に
  `synvelis-player`/`synvelis-project` が依存する workspace pin で
  あり、新規外部依存・lockfile churn は無い（version 解決済み
  revision の再利用）。

## Deferred Design Concerns

- `observe_cycle` の per-block 物理 call-window を RT runtime が
  自動供給する配線（driver の `observe_attached_block` 相当を
  app/engine 側へ）— `drain_output_reports` の定期 engine 配線・
  production composition root の `PluginHost` 接続と併せて後続
  slice。
- wire schema の L1 検証（実 GUI/driver 結合）— 外部 gate、NotRun。
- `status` への live `plan_generation` 露出 — player 側 accessor
  が必要であり、本 slice の範囲外（observe receipts が代替証拠）。

## Verification Notes

- headless fixture `pc_c4_06_control_op_surface.rs` が
  `UiRuntime::dispatch_op` 経由で 4 test を駆動:
  surface 無し fail-closed・typed outcome/status・
  observe stage 選別 + Reported feed・generation gate。
- canonical `mise run` 検証は task/plan の Verification 節どおり。
