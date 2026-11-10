---
id: "SPEC-MCgOf8EskzmlXOde7x0Pqs"
type: "spec"
status: "approved"
title: "PC-C4-02 semantic plugin instance binding — stage-1 authority consumption・plugin identity pin（headless slice）"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/specs/pc-c3-control-trace-projection.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/pc-c4-plugin-param-binding.md"
  refines:
    - "docs/specs/pc-c4-plugin-param-binding.md"
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/test-specs/pc-c4-e2e-plugin-param-binding.md"
  defers: []
  deferred-by: []
---

# PC-C4-02 semantic plugin instance binding — stage-1 authority consumption・plugin identity pin（headless slice）

## What Should Be Built

Issue #101 の第二 slice。第一 slice（`docs/specs/pc-c4-plugin-param-binding.md`、
PR #221）が `synvelis-player` 内に構築した verified frame base・
capability gate・generation pin の binding 層を、#122（CTL-02, PR #243）
で確立した **stage-1 semantic binding 正本**（`Project::control` +
`semantic_plugin_binding`）へ接続する。第二 Parameter 正本は作らない。

具体的には:

- **PluginInstance Mapping 正本の補完**（`synvelis-project`）:
  processor → authored `PluginRef` の canonical lookup
  （`plugin_instance_binding`）を `host_param` に追加し、
  `semantic_plugin_binding` と同じ fail-closed 規約で解決する。
- **Instance identity の vend**（`synvelis-player` host/engine）:
  `ProcessorConnectRequest.processor.binding` が connect 時に
  `Resolved(PluginRef)` だった場合、その **connect-time の authored
  identity** を `RetainedConnection` → `ProcessorControl.plugin`
  （`Option<PluginRef>`）として generation 経由で vend する。
  live project の現在値ではなく、その generation の instance が
  実際に instantiate された binding を証拠として運ぶ。
- **Session の plugin pin**（`synvelis-player` binding）:
  `PluginBindingSession::attach` が vended `PluginRef` を必須 pin とし、
  `PluginParameterBinding` record に `plugin` を保持する。
  `bind` は stage-1 `semantic_plugin_binding` を必ず解決し、
  (a) processor が `routing.processors` に存在し `Resolved` であること、
  (b) semantic binding の `PluginRef` が session pin と一致すること
  を fail-closed で検査する。不一致は typed
  `BindingError::PluginMismatch`。`ParamId` は semantic binding が
  導出した値のみを使う（raw id の横流し経路を作らない）。
- **Dispatch pin の拡張**: `session.dispatch` は record の `plugin`
  が session pin と異なる場合 `PluginMismatch` で拒否する
  （record 全体比較による `BindingDefunct` より先に、原因の分かる
  typed error を返す）。

## Acceptance Criteria

- [ ] `PC-C4-02-01` session attach は vended `PluginRef` を pin し、
  `bind` が stage-1 semantic binding を解決する。processor 未登録・
  `Unresolved`・semantic/instance の `PluginRef` 不一致
  （descriptor/version/capability_revision のいずれか）は全て
  typed reject で、delivery 0。
- [ ] `PC-C4-02-02` `ProcessorControl.plugin` は connect-time の
  `ProcessorBinding::Resolved` のみを運び、`StaticProcessorHost`・
  `Unresolved` binding では `None` — `RoutingControlDispatch` は
  `None` を `without_control` evidence として fail-closed で扱う。
- [ ] `PC-C4-02-03` 既存 PLUG-C4-01..05 fixture は semantic authority
  経由の bind に更新されても全て green（RT/offline parity・
  generation drift・queue full・stale event 拒否を維持）。
- [ ] `PC-C4-02-04` 新規 fixture: plugin rebind（`PluginRef` 差し替え）
  後の `bind` は `PluginMismatch`、旧 record の dispatch は
  `PluginMismatch`/`BindingDefunct`、param 再列挙・capability
  revision 差分・graph generation pin は従来通り typed reject。
- [ ] `PC-C4-02-05` 実 VST3/OS backend・実 GUI は NotRun のまま。
  Ruviform 共通 host/lifecycle/scheduler の再実装なし、第二
  Parameter 正本なし、vcs/apply/restore 非接触。

## Out of Scope

- 実 VST3/OS plugin backend、OS MIDI、実 GUI（Ruviform #253 gate）。
- `PluginNodeControl` への identity query 追加など Ruviform 側変更
  （本 slice は Synvelis consumer 側のみ）。
- persisted `StablePluginInstanceId`・binding snapshot
  （engine restart 横断の安定性は deferred のまま）。
- `RoutingControlDispatch` の active-generation gate・
  `HostParamEvidence` 分類の再設計（既存 CTL-02 契約を維持）。
- workspace/app 層の配線（composition root は依然として
  `RoutingControlDispatch` を呼ぶだけ）。

## Dependencies

- #122 CTL-02（PR #243）: `semantic_plugin_binding`・
  `TargetCapability`・`ProcessorBinding`。
- #101 第一 slice（PR #221）: `PluginBindingSession`・
  `PluginParameterBinding`・verified frame base。
- Ruviform pin `0f5b573`（#307 verified frame base contract）。
