---
id: "TASK-BnGVtGmGrkbl8OUonfkG4K"
type: "task"
status: "in-progress"
title: "SF-04 loop-discontinuity T1 — syn.probe.compare OpName + loop→probe 不連続宣言"
created: "2026-11-15"
updated: "2026-11-15"
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
    - "docs/plans/sf-04-loop-discontinuity.md"
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
  verified-by:
    - "docs/test-specs/sf-04-loop-discontinuity.md"
  references: []
  defers: []
  deferred-by: []
---

# SF-04 loop-discontinuity T1 — syn.probe.compare OpName + loop→probe 不連続宣言

## Work Items

- [x] T1-1: `ui/src/ipc.ts` `OpName` union へ
  `"syn.probe.compare"` を追加。
- [x] T1-2: `crates/synvelis-player/src/routing_control.rs`
  に `last_observed_loop_discontinuity` field +
  `observed_loop_discontinuity()` accessor を追加し、
  `observe_block`/`observe_node_block` で wrap
  （`WrapSim::walk_window` の `LoopWrap` 点）と
  `SetLoop`/`ClearLoop` apply（`note_transport_command`
  の noted 判定）を検出する。
- [x] T1-3: `crates/synvelis-app/src/engine.rs`
  `SessionRoutingEngine::run` を per-block
  `run_observed(1)` + PumpPolicy emulation +
  `PlayReport` merge に再構成し、検出時に best-effort
  `declare_probe_discontinuity` を呼ぶ。
- [x] T1-4: `crates/synvelis-player/tests/
  sf04_loop_discontinuity.rs` を新規追加 — wrap/apply
  検出・宣言発火・cursor/PCM 非移動・跨ぎ compare
  fail-closed・`CommandChannelFull` fail-closed を pin。
- [x] T1-5: slice8 spec/test-spec の deferred
  「loop transport」行を本 slice での解消として更新。
- [x] T1-6: docs chain 完成（test-spec/impl-ir/impl-exp）
  + managed index 再生成 + `evidence/sv107/` 記録。
- [ ] T1-7: canonical `mise` 検証全件実行・commit/push/
  PR。

## Done When

- `syn.probe.compare` が `OpName` union に含まれ
  `check-ui`/`test-ui` が green。
- loop wrap・`SetLoop`/`ClearLoop` apply の各 block で
  probe discontinuity が宣言され、跨ぎ compare が
  fail-closed になることが player fixture で pin される。
- 宣言が cursor・PCM・seek を動かさず、
  `CommandChannelFull` でも fail-closed。
- `SessionRoutingEngine::run` の外部契約が不変
  （既存 test green）。
- `mise run check`/`lint`/`test`/`test-rt`/
  `check-ui`/`test-ui`/`prek` が green。
