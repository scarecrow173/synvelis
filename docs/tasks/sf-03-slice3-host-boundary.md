---
id: "TASK-034UfGmlTvNz0gwIlMQkZL"
type: "task"
status: "done"
title: "SF-03 slice3 T1 — ProcessorHost boundary・StaticProcessorHost・PluginHostProcessors"
created: "2026-11-08"
updated: "2026-11-08"
owners: ["scarecrow173"]
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
    - "docs/plans/sf-03-slice3-plugin-host-teardown.md"
  implemented-by: []
  depends-on:
    - "docs/plans/sf-03-slice3-plugin-host-teardown.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/sf-03-slice3-plugin-host-connect.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-03 slice3 T1 — ProcessorHost boundary・StaticProcessorHost・PluginHostProcessors

## Work

`crates/synvelis-player/src/host.rs` を新規作成し、設計書の boundary
契約を実装する。

- `ProcessorConnectRequest`: semantic `Processor`、解決済み `SemanticPort`
  一覧、`BuildMode`、generation、node identity、engine `AudioFormat`、
  `max_frames` を保持。
- `ProcessorConnection`: `processor: Box<dyn GraphProcessor>` +
  `lease: ProcessorLease`。
- `ProcessorLease`: `teardown: Option<Box<dyn FnOnce()>>` のみを持つ
  opaque token。`Drop` で 1 度だけ teardown を実行する。
- `ProcessorConnectError`: `Unavailable`/`Lifecycle`/`Incompatible`/
  `Invalid` の `#[non_exhaustive]` enum と `fault() -> RoutingFault`
  写像（設計 §fault 対応表どおり）。
- `StaticProcessorHost`: `BTreeMap<ProcessorId, Box<dyn GraphProcessor>>`
  を保持し、`connect` で `remove` して instance を消費、no-op lease を
  返す。未登録は `Unavailable`。
- `PluginHostProcessors`: `Rc<RefCell<PluginHost>>` を保持。
  `connect` は `ProcessorBinding::Resolved` を要求し、
  Instantiate→Initialize→Activate(ProcessConfig)→StartProcessing→
  `attach` の順で実 host lifecycle を駆動。semantic port から
  `BusDescriptor` を導出し、`AttachConfig` を構成して
  `ruviform_audio_plugin_bridge::attach` を呼ぶ。成功時は
  `(PluginNode, teardown closure)` を返し、closure は
  StopProcessing→Deactivate→Destroy を best-effort で実行する。
  失敗時はそれまでに確保した instance/endpoint を best-effort で
  解放してから typed error を返す。
- `lib.rs` で `pub mod host` + 必要型の再 export。
- host 単体 test（`#[cfg(test)]` または tests/ 内の小 test）で
  lifecycle 順序・teardown・fault 写像を検証。

## Acceptance

- `cargo check -p synvelis-player` が通る。
- `StaticProcessorHost` で instance 消費・未登録 `Unavailable`・
  `fault()` 写像が test で固定される。
- `PluginHostProcessors` が pc_b.rs と同じ lifecycle 順で attach し、
  lease drop で StopProcessing→Deactivate→Destroy が走ることを
  instrumented host または upstream 観測点で確認できる。

## Classification

- `normal-plan-task`

## Upstream Documents

- docs/plans/sf-03-slice3-plugin-host-teardown.md
- docs/designs/sf-03-slice3-plugin-host-connect-design.md

## Dependencies

- docs/tasks/sf-03-slice3-engine-integration.md（本 task が先行）

## Affected Files

- crates/synvelis-player/src/host.rs（新規）
- crates/synvelis-player/src/lib.rs

## Done When

- [x] `ProcessorHost`/`ProcessorConnection`/`ProcessorLease`/
  `ProcessorConnectError` が定義され `fault()` 写像が存在する。
- [x] `StaticProcessorHost` が instance を消費し未登録を
  `Unavailable` で返す。
- [x] `PluginHostProcessors` が Instantiate→Initialize→
  Activate(ProcessConfig)→StartProcessing→attach の順で接続し、
  lease drop で StopProcessing→Deactivate→Destroy を best-effort
  実行する。

## Verification

- [x] `cargo check -p synvelis-player --all-targets` 通過。
- [x] `cargo test -p synvelis-player --test sf03_l3` 11/11 green
  （static/real host・fault 写像・teardown を実測）。
