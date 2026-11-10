---
id: "DESIGN-4AUoTxfZvkN7cj3NjLptJ2"
type: "design"
status: "approved"
title: "SF-04 slice4 設計 — app/session wiring（probe port・syn.probe.*・drain hook）"
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
    - "docs/specs/sf-04-slice4-app-wiring.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-slice4-app-wiring.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/designs/sf-04-slice2-controlsession-probe-trace-design.md"
  defers: []
  deferred-by: []
---

# SF-04 slice4 設計 — app/session wiring（probe port・syn.probe.*・drain hook）

## Context

slice 2 の `ControlSession::probe_*` は `&mut dyn ProbeTracePort` を
per-call で受け取る設計で、port の所有は composition root に残す
（`ProjectSession::control_with` が project を per-call で渡すのと
同じパターン）。しかし app 層には port の実体がなかった:

- `SessionRoutingEngine` は `RoutingEngine` を保持するが
  `probe_bindings()` のみ公開し、`ProbeTraceBridge` も
  `DispatchTraceAdapter` も持たない。
- `ControlSession` の probe API を呼ぶ app 経路が存在しない。
- `syn.probe.*` ops・drain hook が未定義。

本 slice はこの composition gap を埋める。probe 意味論自体は
slice 1-3 が確立済みで、本 slice は **配線と wire 契約のみ** を
追加する — player/project/workspace の probe ロジックは変更しない
（`ProbeTraceBridge` への additive API 追加のみ）。

## 所有権モデル

```
SynvelisApp
 └─ WorkspaceManager → ProjectSession → ControlSession (probe_session: correlation)
 └─ routing_engine: Option<SessionRoutingEngine>   ← cloneable handle
      └─ Inner.bridge: ProbeTraceBridge            ← NEW（旧 Inner.engine）
           ├─ engine: RoutingEngine                (probe lifecycle + drain)
           ├─ adapters: BTreeMap<NodeId, DispatchTraceAdapter>
           └─ routes: BTreeMap<ParameterTargetId, NodeId>
```

- **`ProbeTraceBridge` は engine と同じ寿命** — `SessionRoutingEngine`
  の `Inner` が所有し、clone は `Rc<RefCell<Inner>>` 共有。project
  差し替えで `routing_engine` が `None` に戻ると bridge も消え、
  session 相関だけが `ControlSession` に残る（key→target map は
  session スコープで project 非依存 — 再 attach 後も drain が
  相関を回復する）。
- **drain は `ui_pump` のみ** — `ControlSession::probe_drain` が
  session 唯一の drain owner。`syn.probe.state` は drain しない
  （read-only 投影）。
- **port 供給は scoped** — `SessionRoutingEngine::with_probe_bridge`
  が `&self` から `FnOnce(&mut ProbeTraceBridge) -> R` を呼ぶ。
  `RefCell` borrow は呼び出しスコープに限定され、`&mut` port は
  `R` に逃げない（`R` に `&mut` 参照を含められないため型で保証）。

## 変更点

### `synvelis-player/src/probe_trace.rs`（additive）

- `ProbeTraceBridge::insert_adapter(node, adapter) -> Option<DispatchTraceAdapter>`
  — 構築後の adapter 登録。置き換え時は旧 adapter を返して
  caller が観測できる（silent drop しない）。`route_target` は
  既存のまま。

### `synvelis-app/src/engine.rs`

- `Inner.engine: RoutingEngine` → `Inner.bridge: ProbeTraceBridge`。
  `SessionRoutingEngine::new` で `ProbeTraceBridge::new(engine,
  BTreeMap::new())` を構築（adapter は後から `insert_adapter` で
  載せる — `SessionEngineConfig` は `Clone+Debug` を要求するため
  adapter map を config に入れない）。
- 全 engine アクセスを `inner.bridge.engine()`（read）/
  `engine_mut()`（write）経由に書き換え。`probe_bindings()` は
  `inner.bridge.probe_bindings()`（port trait の `&self` 版）でも
  同値 — engine 経由のまま維持。
- 追加 API:
  - `with_probe_bridge<R>(&self, f: impl FnOnce(&mut ProbeTraceBridge) -> R) -> R`
  - `register_probe_adapter(node, adapter)` — `bridge.insert_adapter` への薄い委譲
  - `route_probe_target(target, node)` / `unroute_probe_target(target)` — route 登録

### `synvelis-app/src/ui_ops.rs`

- `UiOpsState` に `last_probe_drain: ProbeDrainReport` を追加
  （`Copy+Default`）。
- `ui_pump` 末尾に `self.drain_probe()`:
  `routing_engine` が `Some` のときのみ
  `engine.with_probe_bridge(|port| session.control_mut().probe_drain(port))`
  を呼び report を保存。`None` なら何もしない（前回 report は
  engine detach 後も残る — session 記録として正直に保持。
  engine 再 attach 後の drain が上書きする）。
- `op_probe_control(&mut self, params) -> Result<Value, ErrorBody>`:
  - `params.action`: `"request" | "set_mode" | "capture" |
    "stop_capture" | "remove"`（未知は INVALID_PARAMS）。
  - engine 未接続 → `SERVICE_ERROR`（"no routing engine attached"）。
  - `request`: `target`（tagged `ParameterTargetId`）・`point`
    （serde `SemanticProbePoint`）・`kind`（serde
    `SubscriptionKind`）・`mode`（serde `ProbeMode`）を parse し
    `control_with(|c,p| c.probe_request(p, port, target, point,
    kind, mode))` → `{key, binding}` を返す。
  - `set_mode`: `key`（serde `ProbeSubscriptionKey`）+ `mode`。
  - `capture`: `key` + `max_frames`（u64）。
  - `stop_capture`/`remove`: `key` のみ。
  - `ProbeTraceError` → `ErrorBody`: `UnknownTarget` →
    INVALID_PARAMS、`Port(ProbePortError)` → `port_error` 写像
    （`UnknownKey`/`NotBound` → INVALID_PARAMS、`Failed` →
    SERVICE_ERROR、他は variant 名を保持した SERVICE_ERROR）。
  - 末尾で `self.ui_pump()`（drain hook が走る）。
- `op_probe_state(&self, params) -> Result<Value, ErrorBody>`:
  - `engine_attached: bool`、`bindings`（key serde + state/
    requested_mode/confirmed_mode/active_plan_generation/
    project_revision/runtime_probe_id Debug 投影）、`segments`
    （同様に Debug 投影）、`drain`（last report）、`traces`
    （`port.traces()` の target+observed スロット投影 — adapter
    未搭載なら空配列）。engine 未接続でも `engine_attached:false`
    + 空投影で 200 を返す（read op は error にしない — UI が
    「未提供」を表示できるようにする）。
  - freshness は `FreshnessContext` を要し UI clock との対応が
    別契約のため本 op では投影しない（slot 内容のみ）。

### `synvelis-app/src/service.rs`

- `OPS` に `syn.probe.control`・`syn.probe.state` を登録。
- `dispatch` に 2 arm を追加。

### `synvelis-workspace`

- 変更なし — `ControlSession::probe_*`・`ProbeDrainReport`・
  `ProbeTraceError` は slice 2 のまま使う。

## Wire 契約

`syn.probe.control` params:

```json
{ "action": "request", "target": "param-…",
  "point": { "entity": {"entity":"Strip","id":"strip-…"},
             "boundary": "post-fader", "port": "semantic-port-…"? },
  "kind": "metrics", "mode": "monitor" }
```

serde 形状は `SemanticProbePoint`/`ProbeSubscriptionKey`/`ProbeMode`/
`SubscriptionKind` の `Serialize`/`Deserialize` 実装に従う
（`SignalBoundary`/`ProbeMode`/`SubscriptionKind` は kebab-case、
entity variant は PascalCase の tag/content、id は tagged 文字列）。`key` は request の戻り値をそのまま
set_mode/capture/stop_capture/remove に渡す。

`request` 戻り値: `{"key": {...}, "state": "<binding Debug>"}`。
`state` op 戻り値: `{"engine_attached": bool, "bindings": [...],
"segments": [...], "drain": {...}, "traces": [...]}`。

## Invariants

- probe op は project revision/undo/graph_builds/master PCM を
  変えない（`control_with` の `probe_request` は project を read
  のみ、port 呼び出しは engine 内 ledger のみ）。
- drain は `ui_pump` のみ — read op が drain を起こさない。
- engine 未接続で lifecycle op は SERVICE_ERROR、state op は
  `engine_attached:false` の空投影。
- fail-closed は下位層のまま: stale generation・unrouted
  `record_probe`・unresolvable point は slot を汚さない。
