---
id: "DESIGN-4IoiIeHs7TpfdoOHf3nu9E"
type: "design"
status: "approved"
title: "PC-C4 plugin binding design — persisted instance authority を bind 経路へ接続"
created: "2026-11-14"
updated: "2026-11-14"
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
    - "docs/specs/pc-c4-plugin-binding.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-plugin-binding.md"
    - "docs/designs/pc-c4-plugin-param-binding-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-plugin-binding.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-plugin-binding.md"
  verified-by:
    - "docs/test-specs/pc-c4-plugin-binding.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C4 plugin binding design

## 概要

`ParameterTarget → PluginInstance` の mapping を
`synvelis-project` の単一正本
`plugin_parameter_instance_binding` に集約し、
`PluginBindingSession::bind`（synvelis-player）が attach-time pin
の `StablePluginInstanceId` をその結果と照合する。照合は
non-RT の bind 経路で行い、dispatch hot path の契約は変更しない。

## stage-1 / stage-1.5 / stage-2 の責務

- **stage-1**（既存）: `semantic_plugin_binding(project, target)`
  が `ParameterTargetId → (ProcessorId, PluginRef, ParamId)` を
  fail-closed に解決する。owner・capability・resolved binding の
  各不整合は `Error` で拒否。
- **stage-1.5**（本 slice 新規）:
  `plugin_parameter_instance_binding(project, target)` が stage-1
  の結果に persisted `PluginInstanceIdentities::resolve(processor,
  plugin)` を合成し、`PluginParameterInstanceBinding { semantic,
  instance }` を返す。`instance` は `Option<PluginInstanceId>` で、
  store が `(processor, plugin)` を保証できない場合（欠落・
  PluginRef drift・範囲外 id）は `None`。`None` は consumer が
  fail-closed に typed reject するためのデータであり、
  semantic 解決自体の失敗（`Err`）とは区別する。
- **stage-2**（既存）: `PluginBindingSession` が attach 時に pin
  した `(StablePluginInstanceId, NodeId, PluginRef,
  capability_generation, graph_generation, AttachClockContext)` と
  dispatch record を照合する。

## `Option<instance>` を選ぶ理由

`bind` は plugin identity の不一致を `PluginMismatch` として
**先に**報告する既存契約を持つ。instance 解決を `Err` で fail
させる合成関数にすると、project の plugin drift 時に instance
解決が semantic より先に失敗し、`PluginMismatch` の報告順が
崩れる。`instance: Option` にすることで、

1. semantic 解決失敗 → `Err`（従来どおり `BindingError::Project`）
2. `semantic.plugin != session.plugin` → `PluginMismatch`（順序維持）
3. `instance` が `None` または pin と不一致 → `InstanceMismatch`

の 3 段が一意に決まる。`resolve` 自体が `Option` を返す store
契約とも一致する。

## bind の照合順序

`PluginBindingSession::bind(project, target)`:

1. `plugin_parameter_instance_binding(project, target)` — semantic
   + instance を一括解決（`Err` → `BindingError::Project`）。
2. `semantic.plugin != self.plugin` → `PluginMismatch`。
3. `instance` が `None`、または `instance.get() !=
   self.instance_id.get()` → `InstanceMismatch`。
4. 以降は従来どおり capability から `ParamId` を pin して binding
   record を stage する。

`InstanceMismatch` は dispatch 側と同じ variant を再利用する
（「この session の pin と権威の instance identity が一致しない」
という同一概念）。

## RT / 非 RT 契約

- `plugin_parameter_instance_binding` は `BTreeMap` 参照のみで
  alloc しないが、bind 経路は attach/sync 時の non-RT orchestration
  であり RT callback からは到達しない。dispatch hot path
  （`ParamDispatch` への staging）は変更しない。
- `PluginInstanceIdentities::resolve` は読み取り専用で、watermark
  や store を変更しない。

## fixture の整合

既存の `PluginBindingSession::attach` fixture は runtime 発行の
任意 instance id を pin していた。persisted authority 接続後は、
fixture project が `plugin_instances.ensure(processor, plugin)` で
store を populate し、session pin がその id と一致する必要がある。
「同一 PluginRef の再 attach」は同一 instance id を保持し、
世代差は capability/graph generation と node pin が担う
（PC-C4-05 の不変条件）。真の「別 instance」は別 `ProcessorId`
または plugin rebind（新 `PluginRef` → 新 id）として model 化する。

## 失敗時の振る舞い

- store 欠落（`plugin_instances` が空、または対象 `(processor,
  plugin)` の entry 無し）: bind は `InstanceMismatch` で拒否。
  vend 経路（`RoutingControlDispatch`）は attach 時に同じ store
  を resolve するため、正常系では一致が構造的に保証される。
- plugin rebind 後の旧 session: project 権威が新 `(plugin, id)` を
  解決するため、旧 pin の session は `PluginMismatch`（plugin が
  変わる場合）または `InstanceMismatch`（同一 plugin で id のみ
  再発行された場合）で拒否される。
- `Option::None` を `unwrap` する consumer は作らない — bind が
  唯一の fail-closed consumer であり、他の読み取り用途
  （evidence 等）は `None` をそのまま表示してよい。
