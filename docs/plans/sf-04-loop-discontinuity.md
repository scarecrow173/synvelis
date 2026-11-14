---
id: "PLAN-SlqEQMtC82T429adWLReuH"
type: "plan"
status: "approved"
title: "SF-04 loop-discontinuity 実装計画 — OpName wire + loop→probe 不連続宣言"
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
    - "docs/specs/sf-04-loop-discontinuity.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/sf-04-loop-discontinuity.md"
    - "docs/designs/sf-04-loop-discontinuity.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# SF-04 loop-discontinuity 実装計画 — OpName wire + loop→probe 不連続宣言

## Goal

`syn.probe.compare` を UI wire `OpName` union へ到達可能にし、
loop wrap・`SetLoop`/`ClearLoop` apply を probe clock epoch の
不連続として block 単位で自動宣言する結線を完成させる。
比較意味論・drain 所有権・transport cursor は変更しない。

## File Map

| File | Change |
| --- | --- |
| `ui/src/ipc.ts` | `OpName` union へ `"syn.probe.compare"` 追加 |
| `crates/synvelis-player/src/routing_control.rs` | `last_observed_loop_discontinuity` field + `observed_loop_discontinuity()` accessor、`observe_block`/`observe_node_block` 検出配線、`note_transport_command` の `-> bool` 化 |
| `crates/synvelis-app/src/engine.rs` | `SessionRoutingEngine::run` を per-block `run_observed(1)` + PumpPolicy emulation + PlayReport merge + 検出時 best-effort `declare_probe_discontinuity` に再構成 |
| `crates/synvelis-player/tests/sf04_loop_discontinuity.rs` | 新規 fixture（wrap/apply 検出・宣言・cursor 非移動・fail-closed compare・channel full） |
| `docs/specs/sf-04-slice8-probe-faults-declare.md` | deferred「loop transport」行の解消記録 |
| `docs/test-specs/sf-04-slice8-probe-faults-declare.md` | 同 deferred 記述の整合更新 |
| `docs/` | 本 chain（spec/design/plan/task/test-spec/impl-ir/impl-exp）+ managed index 再生成 |
| `evidence/sv107/` | 検証記録 |

`ui/src/store.ts`・`crates/synvelis-app/tests/`・
`crates/synvelis-app/src/{monitoring,ui_ops,service}.rs`・
`crates/synvelis-player/src/` の他 file・`mise.toml`・
`Cargo.lock`（差分が出た場合は PR 本文に明記）は変更なし。

## Steps

1. `ui/src/ipc.ts` — `OpName` union 末尾へ
   `"syn.probe.compare"` を追加。
2. `routing_control.rs` — 検出フラグ + accessor +
   `observe_node_block` の `(DispatchCycle, bool)` 化 +
   `note_transport_command` の loop-apply 検出返し。
3. `engine.rs` — `run` を per-block `run_observed(1)`
   loop に再構成（PumpPolicy `index % n` emulation・
   pcm/crc/frames merge 等価・`pending_observe_cycles`
   append 維持）し、検出時に best-effort 宣言。
4. `tests/sf04_loop_discontinuity.rs` — tim03_loop_boundary
   fixture（plugin + boundary declaration +
   `boundary_stage_window`）と sf04_s9 fixture
   （`ProbeTraceBridge` compare evidence）を合成した
   新規 test file。
5. slice8 spec/test-spec の deferred 解消記録。
6. docs index 再生成・impl-ir/impl-exp・evidence 記録・
   canonical `mise` 検証・commit/push/PR。

## Verification

- `mise run fmt-check` / `mise run check synvelis-player` /
  `mise run lint synvelis-player` / `mise run test
  synvelis-player` / `mise run test-rt` /
  `mise run check synvelis-app` / `mise run lint
  synvelis-app` / `mise run check-ui` / `mise run test-ui` /
  `mise exec -- prek run --all-files`。
- `*-msrv` 系は Daily Full CI 委譲で NotRun 記録。
- 新規 fixture が wrap/apply 検出・宣言発火・cursor
  非移動・`CommandChannelFull` fail-closed・跨ぎ
  compare fail-closed を pin。

## Risks

- **per-block `run_observed(1)` の等価性**: pump cadence・
  pcm/crc/frames・error 伝播・`observe_gen`/`observe_head`
  tracking が `run(N)` と一致することを既存 app/player
  test の green で確認。`PlayReport` merge は crc を
  merge 後 pcm に同一式で再計算して bit 一致を保つ。
- **検出の誤発火/見逃し**: `walk_window` は scheduler
  faithful、`note_transport_command` は upstream と同じ
  validity rules — mirror が fold しない command は
  宣言しない。`boundary_loop_wraps`（pre-window pure
  read）とは別経路で、observe_block の post-window
  検出と混同しない。
- **宣言失敗の扱い**: `CommandChannelFull` で
  `ProbeRequestError` が返っても `run` は
  `ProcessError` 契約のため best-effort（seek 先例）。
  epoch advance 自体が fail-closed 証拠であり、
  broker/deck 乖離は `probe_time_context` で観測可能。
- **block 粒度の正直さ**: wrap を含む block の記録は
  旧 epoch を保持する — sub-block 精度は chunk 粒度の
  契約上表現しない（spec Deferred に記録）。
