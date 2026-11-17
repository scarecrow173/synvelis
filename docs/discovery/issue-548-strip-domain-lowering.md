---
id: "DISC-548StripDomLowr01"
type: "discovery"
status: "resolved"
title: "Issue #548 discovery — strip-domain inner op lowering と routing mask 拡張（CB-06 後続）"
created: "2026-11-19"
updated: "2026-11-19"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/548"
    - "https://github.com/scarecrow173/synvelis/issues/283"
    - "https://github.com/scarecrow173/synvelis/issues/46"
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
    - "docs/impl/ir/cb-06-track-bus-adapter.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/cb-06-track-bus-adapter.md"
    - "docs/impl/ir/cb-06-track-bus-adapter.md"
    - "docs/impl/exp/cb-06-track-bus-adapter.md"
    - "evidence/sv283/acceptance-ledger.md"
    - "evidence/sv283/verification.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by:
    - "docs/specs/issue-548-strip-domain-lowering.md"
  verifies: []
  verified-by: []
  references:
    - "crates/synvelis-gateway/src/prepare.rs"
    - "crates/synvelis-gateway/src/prepare_engine.rs"
    - "crates/synvelis-gateway/src/variant.rs"
    - "crates/synvelis-project/src/variant/strip.rs"
    - "crates/synvelis-project/src/variant/adapter.rs"
  defers: []
  deferred-by: []
---

# Issue #548 discovery — strip-domain inner op lowering

## 背景

CB-06（`synvelis#283`）で `StripFieldsAdapter` の mask-gated
strip-fields scope と commit-gate 境界証拠は main 着地済み
（PR `synvelis#547`、merge commit `9155f204`）。しかし gateway の
wire capability 表には strip-domain op が 1 件も登録されておらず、
`cb06_strip_gateway.rs` の編集境界証拠は「全 ordinary op が
`variant-unsupported` で一様 reject」という否定側のみだった。
Issue #548 はこの positive 経路を開く後続 slice。

## 調査結果

### domain 契約は main 済み・consume のみ

- `StripFieldsAdapter::edit_candidate`
  （`crates/synvelis-project/src/variant/strip.rs`）は
  `is_strip_fields_op`（`SetStripMute`/`SetStripSolo`/
  `InsertProcessor`）と `is_instrument_slot_op`
  （`SetParameterBase`/`AddParameterTarget`/automation lanes+
  regions/modulation/`SetProcessorBinding`）を mask 宣言に応じて
  受理し、`edit_scoped_project_op` →
  `check_strip_fields_op_targets`（`adapter.rs`）が scope
  confinement を enforce する。
- gateway 側は capability descriptor・lower fn・`lower_for`
  entry・`op_facts` arms を足すだけでよく、`synvelis-project`
  の変更は不要かつ本 slice の lease 外。

### 登録と lowering の対応は不可分

- `PrepareEngine::op_specs()` は `lower_for(id).expect(...)`
  で全 table entry の lowering を要求する — capability を
  登録したら `lower_for` arm も必須。
- variant edit 経路（`variant.rs`）は inner op を ordinary
  op table で lower して `BranchEditOp::Project` に包むため、
  table 追加だけで wire 経路が開く。

### `op_facts` arms は必須（設計判断1）

- `op_facts` の catch-all は control-plane op を
  `unsupported` に落とし、ordinary prepare では
  provisional_gate の SEC-14 検査で常に fail-closed になる。
  facts を足さない半登録は「直接 prepare で常時失敗する
  capability」になるため、既存 op と同じく reads +
  preconditions を正直に宣言する。

### 「routing mask 拡張」の解釈（用語確定）

- Issue 題名の routing mask 拡張は、本 slice では **gateway
  の op dispatch（`lower_for` の capability 網羅）拡張**と
  読む。send/connection/sidechain/bus op の candidate 受理
  （`capabilities().routing = Unsupported` の解消）と scope
  entity removal は `synvelis-project` の mask/adapter 変更を
  要し lease 外の別 slice。

## 採用した capability 集合

| capability id | target contract | effects | 下層 ProjectOp |
| --- | --- | --- | --- |
| `synvelis.strip.set-mute` | `Resource{Strip}` | `[Create, Write]` | `SetStripMute` |
| `synvelis.strip.set-solo` | `Resource{Strip}` | `[Create, Write]` | `SetStripSolo` |
| `synvelis.strip.insert-processor` | `Resource{Strip}` | `[Create, Write]` | `InsertProcessor` |
| `synvelis.param.set-base` | `Resource{ParameterTarget}` | `[Write]` | `SetParameterBase` |
| `synvelis.processor.set-binding` | `Resource{Processor}` | `[Write]` | `SetProcessorBinding` |

- bool op が `[Create, Write]` なのは channel `ParameterTarget`
  の first-use create を正直に反映するため。
- `AddParameterTarget`・automation lane/region・modulation
  系は wire schema が重く domain 検証責務も別のため本 slice
  では見送り（必要なら別 Issue）。

## ギャップ・未対応

- routing/send/connection/sidechain/bus op の candidate 受理、
  scope entity removal、他 domain（midi-clip・instrument-slot
  等）の positive 化、実 PCM/RT/plugin gate・実 GUI lane は
  すべて別 slice/別 gate（`NotRun` 記録のみ）。
- `doc-driven-dev-graph` route は既存の broken relation
  （`docs/impl/ir/cb-09-diff-explorer.md` → `evidence/sv286/`）
  と focus 未指定で `blocked` を返した — リポジトリ側の
  既知 graph blocker であり本 slice の欠陥ではない。文書
  チェーンは lease 済みパスへ手動で作成した。
