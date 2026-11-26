---
id: "SPEC-t7Qm4Vx9Ln2Rw8KzYpD6"
type: "spec"
status: "approved"
title: "TIM-03 transport application minimal vertical"
created: "2026-11-27"
updated: "2026-11-27"
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
  implements: []
  implemented-by:
    - "docs/designs/tim-03-transport-application.md"
  depends-on:
    - "docs/specs/tim-03-l1-out-of-process-hosting.md"
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
    - "docs/test-specs/tim-03-transport-application.md"
  references: []
  defers: []
  deferred-by: []
---

# TIM-03 transport application minimal vertical

## 目的

merged 済みの TIM-03 L1 out-of-process hosting と process boundary session の上に、
Synvelis app 所有の transport application seam を追加する。project musical time を
host 側で一度だけ media frame へ変換し、loop/seek/play と automation evaluation を
production composition 経由の実 VST3/CLAP `test_gain` L1 route に適用して、
control ledger・transport span・PCM の独立証拠を残す。

## スコープ

- `SessionRoutingEngine::control_apply_transport` を追加し、beat-domain の
  `Seek`/`SetLoop`/`ClearLoop`/`Play`/`Pause`/`Stop` input を受け取る。
- project tempo map と attached control surface の verified frame base から
  beat→media conversion を seam 内で一度だけ行う。
- upcoming node-cursor window を engine-reported observation から導出し、
  `boundary_loop_wraps` → `boundary_stage_window` の順で Exact/Degraded policy の
  oracle 入力と transport command を stage する。
- automation evaluation instant も同じ conversion batch に含め、既存 low-level
  `control_tick` へ beat/media pair として供給する。
- production composition seam に processor ごとの `BoundaryDeclaration` table を渡せる
  variant を追加する。
- `tim03_l1_transport_application.rs` で VST3/CLAP を deterministic scenario で実走し、
  machine-readable evidence を `evidence/sv127/transport-application.jsonl` に残す。

## 非スコープ

- `test_instrument` による plugin-side musical transport oracle。
- dynamic tempo edit、全 frame/block boundary matrix、fault/state isolation。
- GUI/Native Dock/EXT-L3、provisioned endpoint、macOS/Linux 実走。
- ruviform 側変更、pin 更新、`Cargo.lock` 更新、docs index 更新。

## 受入証拠

- tempo map は初期 120 BPM に tick 1 で 125 BPM を足した非 default map とする。
- conversion input/result、transport command、loop wrap、span context、control ledger
  projection、PCM block classification、inactive generation negative evidence を
  format ごとに JSONL へ記録する。
- `test_gain` は transport を消費しないため、PCM は plugin-side transport oracle ではなく
  host-side application と automation value law の oracle として解釈する。
