---
id: "DESIGN-PjOpAzd2cNOD9kshEbJeKl"
type: "design"
status: "approved"
title: "PC-C4-02 plugin instance binding design — connect-time identity vend・stage-1 authority consumption"
created: "2026-11-10"
updated: "2026-11-10"
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
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/designs/pc-c4-plugin-param-binding-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/ctl-02-strip-send-host-parameter-plugin-binding-routing.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/designs/pc-c4-plugin-param-binding-design.md"
  defers: []
  deferred-by: []
---

# PC-C4-02 plugin instance binding design — connect-time identity vend・stage-1 authority consumption

## Context

第一 slice の `PluginBindingSession`/`PluginParameterBinding` は
instance/node/generation/frame-base を pin するが、**plugin identity
（descriptor/version/capability_revision）をどこにも pin しない**。
`PluginNodeControl` は identity query を公開せず、`PluginHost` にも
per-instance descriptor query は無い（Ruviform #307 は frame base のみ
保証）。一方 `ProcessorConnectRequest.processor.binding` は connect
時に `ProcessorBinding::Resolved(PluginRef)` を保持しており、これが
「その generation の instance が実際に何として instantiate されたか」
の唯一の正しい証拠である。

live project の `ProcessorBinding` を attach 時に読む設計は誤り:
generation g の instance は build 時 revision の binding で
instantiate されており、publish 後に project が pluginB へ rebind
されても gen g の instance は pluginA のまま生きている（last-good）。
live 値で pin すると pluginB の契約を pluginA instance へ誤配送する。
したがって identity は **connect-time に engine が捕捉し、
generation ledger 経由で vend する**。

stage-1 側は `semantic_plugin_binding`（CTL-02）が
`ParameterTargetId → (ProcessorId, PluginRef, ParamId)` を fail-closed
で解決済みだが、`PluginBindingSession::bind` はそれを消費せず、
owner/capability を inline で再検査するだけで `routing.processors`
の存在・`Resolved`・plugin identity を検証しない。本 slice で
bind を stage-1 正本へ接続する。

## Scope

### In Scope

- `crates/synvelis-project/src/host_param.rs`: `plugin_instance_binding`
  追加 + `lib.rs` export。
- `crates/synvelis-player/src/host.rs`: `ProcessorControl.plugin` 追加。
- `crates/synvelis-player/src/routing_engine.rs`:
  `RetainedConnection.plugin`・`retain_connection`・
  `take_processor_control` の identity vend。
- `crates/synvelis-player/src/binding.rs`:
  `PluginParameterBinding.plugin`・session `plugin` pin・
  `attach` 引数追加・`bind` の stage-1 解決・`BindingError::PluginMismatch`・
  `dispatch` の plugin pin 検査。
- `crates/synvelis-player/src/routing_control.rs`:
  `vend_nodes` の `control.plugin` fail-closed 処理。
- `crates/synvelis-player/tests/pc_c4_binding.rs`:
  fixture の semantic authority 対応 + 新規 PLUG-C4-02 系テスト。
- docs チェーン。

### Out of Scope

- spec の Out of Scope と同一。

## Components and Boundaries

### `plugin_instance_binding`（synvelis-project host_param.rs）

```rust
pub fn plugin_instance_binding(
    project: &Project,
    processor: ProcessorId,
) -> Result<&PluginRef, Error>
```

processor → authored `PluginRef` の canonical lookup。
`routing.processors` に無ければ `Error::NotFound`、
`ProcessorBinding::Unresolved` なら `Error::Validation`。
`semantic_plugin_binding` と同じ fail-closed 規約。session attach の
composition root が instance identity の期待値を project 側から
引くための対称 lookup（本 slice では `bind` 内の
`semantic_plugin_binding` が per-target で同じ解決を行う）。

### Identity vend（host.rs / routing_engine.rs）

- `RetainedConnection.plugin: Option<PluginRef>` — `stage_build` が
  `semantic.binding` から connect 時に捕捉（`Resolved` のみ `Some`）。
  host 実装には依存しない engine 側の証拠。
- `ProcessorControl.plugin: Option<PluginRef>` —
  `take_processor_control` が retained 値を clone して vend。
  `control` が `Some` でも `plugin` が `None`（Unresolved binding で
  control を vend する異常系 host）なら session を開けない。
- `StaticProcessorHost` connection は `control: None` で vend 自体が
  起きないため影響なし。

### `PluginBindingSession` の plugin pin（binding.rs）

- `attach(control, node, processor, plugin, graph_generation,
  capability_generation, instance_id)` — `plugin: PluginRef` は
  **vended evidence 必須**。live project を読まない（上記 Context）。
- `PluginParameterBinding.plugin: PluginRef` — bind 時に記録。
  record 全体の `PartialEq` が既に pin を含むが、dispatch では
  record 比較の前に `binding.plugin != self.plugin` を
  `PluginMismatch` で明示検査する（診断の明確化）。
- `bind` の検査順序（既存 variant を維持）:
  1. `check_dispatch`（node/frame-base pin）
  2. target 存在 → `UnknownTarget`
  3. `owner == Processor(self.processor)` → `OwnerMismatch`
  4. capability 存在 → `CapabilityMissing`
  5. **`semantic_plugin_binding(project, target)` を解決**
     — `Err` は `BindingError::Project`（processor 未登録 =
     `NotFound`、`Unresolved`/非 processor owner/capability 無し =
     `Validation`。3・4 で既に弾かれる経路は到達しない）。
  6. `semantic.plugin != self.plugin` → `PluginMismatch`
     `{ expected: self.plugin, actual: semantic.plugin }`
  7. `param = semantic.param_id`（capability.param_id と同値だが
     正本経由を明示）→ live enumeration → capability drift →
     write gate → modulation gate → `dispatch.bind`。
- `revalidate` は変更なし — instance の live enumeration に対する
  検査であり、plugin identity は bind 時 pin と generation 機構が
  担う（旧 generation の last-good binding はその instance に対して
  正しいまま）。
- `eval_event`/`check_plan`/`is_current` は変更なし。

### `RoutingControlDispatch::vend_nodes`（routing_control.rs）

`take_processor_control` の結果 `control.plugin` が `None` なら
session を開かず `report.without_control` に積む（typed evidence、
silent skip ではない）。`Some(plugin)` なら `attach` に渡す。

## Data and Control Flow

```text
prepare/stage_build: semantic.binding (Resolved PluginRef)
  → RetainedConnection.plugin ──publish──▶ connections[generation]
  → take_processor_control → ProcessorControl.plugin (vend once)
  → vend_nodes: PluginBindingSession::attach(..., plugin, ...)
       │ bind: semantic_plugin_binding(project, target)
       │   → processor Resolved? → semantic.plugin == session.plugin?
       ▼
PluginParameterBinding { ..., plugin } → dispatch pin 検査
  → ParamDispatch → TransportPlan → node channel → instance
```

live project が rebind されても generation g の instance は
connect-time の plugin のまま — session pin は instance の真の
identity であり、semantic 解決は「今 target が名乗る plugin」で
ある。両者の一致だけが bind を許す。

## Non-Functional Constraints

- RT 契約不変: identity vend・session pin は全て非 RT orchestration。
- fail-closed: `PluginRef` の部分一致（descriptor のみ一致等）は
  `PartialEq` 全体比較で拒否。`Option::None` は evidence 化。
- 第二正本なし: `ParamId`・plugin identity の解決は
  `semantic_plugin_binding`/`plugin_instance_binding` のみ。
- 破壊的変更は crate 内 API（`attach` 引数・`ProcessorControl` field・
  `PluginParameterBinding` field）に限定 — `ProcessorControl` は
  engine→caller の一方向 vend で外部構築経路なし。

## Risks and Trade-offs

- `attach` の引数追加は既存 test call site（`open()`・pc_c4_05）の
  更新を伴う — fixture が `routing.processors` に `Resolved`
  processor を登録する必要があり、これは「project 正本を消費する」
  本来あるべき形への修正でもある。
- `ProcessorControl.plugin: Option` — `None` を `without_control`
  に流すのは「identity を証明できない control では session を開かない」
  fail-closed であり、silent drop ではない（report evidence あり）。
- capability_revision が同じまま descriptor/version が変わる
  rebind も `PluginRef` 全体比較で `PluginMismatch` になる —
  `port_capability_generation` が拾えない drift を identity pin が
  捕捉する（本 slice の主目的）。

## Deferred Design Concerns

- workspace/app composition root での `plugin_instance_binding`
  消費 | caller 側の証拠突合 | app 配線 slice | 必要になった時点。
- persisted instance identity | runtime evidence のみ | 永続化 slice |
  restart 横断は deferred のまま。
- `PluginNodeControl` identity query | Ruviform 側 | upstream gate |
  vend 経路で代替済み。

## Verification Notes

- [x] identity の情報源を connect-time `ProcessorBinding` に限定する
  設計は、live project 読みの誤配送経路を構造的に排除する
  （session pin = instance 真実、semantic 解決 = 現在の主張）。
- [x] `PluginMismatch` は `PluginRef` 全体比較 — capability_revision
  衝突・descriptor 差し替えの両方を捕捉。
- [x] 既存 PLUG-C4 fixture は `project_with`/`open` の 2 点更新で
  semantic authority 経由に移行し、個別 test の semantics を維持する。
