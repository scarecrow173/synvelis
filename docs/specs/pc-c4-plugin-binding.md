---
id: "SPEC-6NH8ne9kNmYOjVBpOoiHju"
type: "spec"
status: "approved"
title: "PC-C4 plugin binding — persisted PluginInstance identity を stage-1.5 正本として bind 時に照合"
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
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/specs/pc-c4-05-persisted-instance-id.md"
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/specs/pc-c4-05-persisted-instance-id.md"
    - "docs/impl/ir/pc-c4-plugin-parameter-binding-frame-base-capability-gate.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c4-drain-composition.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/specs/pc-c4-02-plugin-instance-binding.md"
    - "docs/specs/pc-c4-05-persisted-instance-id.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-plugin-binding.md"
  verified-by:
    - "docs/test-specs/pc-c4-plugin-binding.md"
  references: []
  defers: []
  deferred-by: []
---

# PC-C4 plugin binding — persisted PluginInstance identity を stage-1.5 正本として bind 時に照合

## 背景

Issue #101（PC-C4）の既存 slice は stage-1 semantic binding
（`ParameterTargetId → (ProcessorId, PluginRef, ParamId)`）と
stage-2 runtime binding（`PluginBindingSession` の attach-time pin:
`StablePluginInstanceId`・`NodeId`・capability/graph generation・
`AttachClockContext`）を実装済みである。PC-C4-05 は persisted
`PluginInstanceIdentities`（schema-v13、単調 watermark、PluginRef
pin、fail-closed resolve）を導入したが、`PluginBindingSession::bind`
は依然として stage-1 semantic binding のみを解決し、persisted
instance identity を照合していない。

この結果、同一 `(ProcessorId, PluginRef)` が persisted store で新しい
`PluginInstanceId` として再発行された場合（plugin rebind A→B→A、
あるいは store 欠落・drift）、session が pin する旧 instance id と
project 権威の解決結果が乖離していても bind が成功し得る。
dispatch 側の `InstanceMismatch` は record vs session の照合であり、
session 自体が陳腐化した instance id を保持したまま新規 bind を
受け付ける経路は塞がれていない。

## 目的

- `ParameterTarget → PluginInstance` の Mapping 正本を
  `synvelis-project` に一箇所だけ置き、stage-1 semantic binding と
  persisted instance identity の合成を fail-closed に行う。
- `PluginBindingSession::bind` が attach-time pin の
  `StablePluginInstanceId` を project 権威の解決結果と照合し、
  不一致・未解決を型付き Reject（`BindingError::InstanceMismatch`）
  として拒否する。
- 既存の PLUG-C4-01〜04 fixture 契約（typed reject・誤配送なし・
  世代整合）を維持しつつ、persisted instance authority を bind
  経路へ接続する。
- PLUG-C4-05 の実 VST3/OS backend は NotRun のまま正直に記録し、
  headless fixture の成功を実 plugin Pass に繰り上げない。

## スコープ

- `crates/synvelis-project`: `ParameterTarget → PluginInstance` の
  canonical mapping 関数と結果型、対応テスト。
- `crates/synvelis-player`: `PluginBindingSession::bind` の instance
  authority 照合、fixture の persisted identity 整合、instance
  authority の typed reject テスト、session-bound dispatch の
  RT fixture。
- doc-chain: `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/pc-c4-plugin-binding*`。
- `evidence/sv101/`: 検証証拠・外部 gate 記録。

## 非目標（本 slice の範囲外）

- 実 VST3/AU/CLAP plugin backend、OS device backend、plugin GUI
  （ruviform#253 / synvelis#114 系の外部 gate）。headless fake
  fixture の成功を実機受入に繰り上げない。
- AudioLens f64/RT contract、wire-schema/L1、oracle（synvelis#103）。
- `Project::plugin_instances` store 自体の schema・採番規則の変更
  （PC-C4-05 で確定済み）。
- `HeadlessDriver::next_instance` など runtime 境界 provenance の
  別経路 — 本 slice は bind 時の project 権威照合に限定する。
- Ruviform 側の変更。

## 受入条件

- [ ] `plugin_parameter_instance_binding` が
  `ParameterTargetId → (semantic binding, persisted PluginInstanceId)`
  を単一の正本として返し、store が `(processor, plugin)` を保証
  できない場合に `instance: None` を返す（fail-closed は consumer
  側で typed reject）。
- [ ] `PluginBindingSession::bind` が persisted instance id と
  attach-time pin を照合し、不一致・store 欠落・plugin drift を
  `BindingError::InstanceMismatch` で拒否する。plugin identity の
  不一致は従来どおり `PluginMismatch` が先に報告される。
- [ ] 同一 `PluginRef` が新しい `PluginInstanceId` で再発行された
  後、旧 pin の session は bind で `InstanceMismatch` となり、
  誤配送しない。
- [ ] 既存 pc_c4_binding / pc_c6_oracle_pl / pc_c4_03 / pc_c4_05 /
  pc_c4_07 fixture が persisted identity を正しく populate した上で
  全 pass し、PLUG-C4-01〜04 の typed reject・誤配送なし・世代整合
  契約が維持される。
- [ ] session-bound dispatch の RT fixture が alloc-free /
  non-blocking を維持することを示す。
- [ ] `mise run fmt-check`・`mise run check|lint|test
  synvelis-project`・`mise run check|lint|test synvelis-player`・
  `mise run test-rt` が exit 0。
- [ ] 実 VST3/OS backend は NotRun として evidence に記録し、
  `Refs #101` で残件を明示する。
