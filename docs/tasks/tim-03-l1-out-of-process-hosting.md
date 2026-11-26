---
id: "TASK-p9Zx3Vn6Qm2Kw8RyTbL4"
type: "task"
status: "approved"
title: "TIM-03 L1 out-of-process hosting production wiring"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/127"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/tim-03-l1-out-of-process-hosting.md"
  implemented-by: []
  depends-on: []
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
  references: []
  defers: []
  deferred-by: []
---

# TIM-03 L1 out-of-process hosting production wiring — Tasks

## 実施済み

- [x] ruviform pinned API・wire-v2 argv contract・upstream L1 oracle の調査。
- [x] Coordinator 許可を得た限定範囲で root `Cargo.toml` を変更。
  - workspace member `crates/synvelis-plugin-test-l1-runner`。
  - workspace dependency `ruviform-plugin-export`。
- [x] `ProductOutOfProcessPlugin` と `compose_routing_engine_with_out_of_process` を追加。
- [x] `PluginHostProcessors::product_with_out_of_process` を追加。
- [x] VST3/CLAP test-only runner binary を追加。
- [x] dedicated mise task `test-tim03-l1` を追加。
- [x] `tim03_l1_process_oracle.rs` を追加し、VST3/CLAP で production activation
  と direct endpoint defined-output membership を実測。
- [x] `evidence/sv127/l1-process-oracle.jsonl` を追加。

## 残り（この slice の完了処理）

- [x] verification markdown を最終検証結果で確定する。
- [x] 正規検証と prek を完了する。
- [ ] commit、push、PR 作成。

## 後続 slice / gate

- [ ] tempo/loop/automation sample accuracy matrix。
- [ ] `test_instrument` MIDI/transport precision oracle。
- [ ] `test_fault` crash/hang/timeout isolation。
- [ ] `test_stateful` state quality（sv#128/TIM-04 側との境界に従う）。
- [ ] GUI/Native Dock、実 Window、Windows EXT-L3 最終受入。
- [ ] AC4/AC7 CI/cross-consumer gate、AudioLens f64/native PCM 非回帰。
- [ ] docs index 整備（Coordinator 統合時）。
