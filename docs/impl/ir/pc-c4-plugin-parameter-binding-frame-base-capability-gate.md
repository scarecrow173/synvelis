---
id: "IMPL-034UxmcANQsdlNy9GC0tWv"
type: "impl"
status: "completed"
title: "PC-C4 plugin parameter binding frame base capability gate"
created: "2026-11-09"
updated: "2026-11-09"
owners: []
relations:
  source: []
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-player/src/binding.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/pc_c4_binding.rs"
      - type: "file"
        path: "docs/specs/pc-c4-plugin-param-binding.md"
      - type: "file"
        path: "docs/designs/pc-c4-plugin-param-binding-design.md"
      - type: "file"
        path: "docs/plans/pc-c4-plugin-param-binding.md"
      - type: "file"
        path: "docs/tasks/pc-c4-task1-plugin-param-binding.md"
      - type: "file"
        path: "docs/test-specs/pc-c4-e2e-plugin-param-binding.md"
    modified:
      - type: "file"
        path: "crates/synvelis-player/src/dispatch.rs"
      - type: "file"
        path: "crates/synvelis-player/src/host.rs"
      - type: "file"
        path: "crates/synvelis-player/src/lib.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/pc_b.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/sf03_l4.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/sf04_s2_probe_trace_bridge.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/trace_c3_05.rs"
      - type: "file"
        path: "crates/synvelis-player/tests/trace_c3_05_rt.rs"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/pc-c4-task1-plugin-param-binding.md"
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
  verifies:
    - "docs/test-specs/pc-c4-e2e-plugin-param-binding.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# PC-C4 plugin parameter binding frame base capability gate

## Summary

Synvelis #101 の fake plugin port slice。`synvelis-player` に
`PluginBindingSession` を追加し、project `ParameterTargetId` を
stable plugin instance・`NodeId`・plugin `ParamId`・capability/graph
generation・Ruviform #307 attach-time verified frame base に pin する。
`ParamDispatch::new` は構築時に `check_frame_base` で TimeBase を検証し、
不一致は `DispatchError::FrameBaseMismatch` で plan staging 前に fail
closed する。型付き reject（`param_readonly`/`param_not_automatable`/
`param_mismatch`/`QueueFull`/stale plan 等）を保持し、誤配送は 0。

## Context

`docs/tasks/pc-c4-task1-plugin-param-binding.md`（plan
`docs/plans/pc-c4-plugin-param-binding.md`、spec
`docs/specs/pc-c4-plugin-param-binding.md`、design
`docs/designs/pc-c4-plugin-param-binding-design.md`、test-spec
`docs/test-specs/pc-c4-e2e-plugin-param-binding.md`）に従う。
上流契約は Ruviform #307（`frame_base()`/`check_frame_base()`、
pin `0f5b573`）と #186（`check_param_write`/`ParamWriteMeans`）。
PC-B の `ParamDispatch`（TargetId→param table・plan staging・ledger）
を再利用し、その上に binding/session 層を additive に追加した。

## Implementation

### Changes

- `src/binding.rs`（新規）: `StablePluginInstanceId`、
  `AttachClockContext`（`PluginNodeControl::frame_base()` のみから
  mint、外部 TimeBase を受け取る constructor なし）、
  `PluginParameterBinding`（全 generation/clock pin を保持）、
  `PluginBindingSession`（`attach`/`bind`/`revalidate`/`invalidate`/
  `dispatch`/`eval_event`/`check_plan`）、型付き `BindingError`。
- `src/dispatch.rs`: `ParamDispatch::new` を `Result` 返しに変更し、
  構築直後・plan 構築前に `control.check_frame_base(frame_base)` を実行。
  `DispatchError::FrameBaseMismatch(FrameBaseMismatch)` を追加。
- `src/host.rs`: `ProcessorControl.frame_base` の doc を #307 verified
  契約に合わせて更新。
- `src/lib.rs`: `binding` module と公開型を export。
- `tests/pc_c4_binding.rs`（新規）: fake recording plugin fixture で
  PLUG-C4-01..05・PC-C4-PL01..08 を実証（5 tests）。
- 既存 test call site（pc_b/sf03_l4/sf04_s2/trace_c3_05*）を新しい
  `Result` 返し constructor に追従。

### Decisions During Implementation

- capability 宣言と live `ParamInfo` の min/max/default 比較は
  `f64::to_bits` による bit-exact とした（drift を epsilon で吸収しない）。
- batch `dispatch` は全 `(binding, event)` を staging 前に検証し、
  1 件でも pin 不一致なら全体を reject（atomic）。
- `eval_event` は plan freshness を event 毎に再検査しない — 呼び出し側が
  orchestration 境界で `check_plan`（`EvalPlan::is_stale`）を gate する
  契約とし、doc に明記。
- `revalidate` も他メソッド同様 `check_dispatch`（node/frame-base pin）
  を先に適用し、foreign dispatcher の判定で session record を誤って
  落とさないよう fail-closed に統一。
- review-protocol 実施: CONFIRMED P3（`revalidate` の dispatcher pin
  未検査）を修正、P2/P3 の非 blocking 指摘は PR に記録。

## Related Experiments

### Adopted

<!-- なし -->

### Rejected

<!-- なし -->

## Validation

canonical = mise（`git fetch origin main && git merge origin/main` 済み、
merge commit `0420a74`）。補助証拠として直接 `cargo` も実行済み。

- `mise run fmt-check` — exit 0
- `mise run check` — exit 0
- `mise run lint` — exit 0
- `mise run test` — exit 0（workspace 全 test、136 suite ok / 0 failed）
- `mise run test-rt` — exit 0（`rt_` filter。`pc_c4_01_..._rt_offline_parity` 含む）
- `mise run test-msrv` — exit 0（rust 1.89 toolchain、workspace 全 test）
- `mise run check-headless-deps` — exit 0

## Risks

- `StablePluginInstanceId` の一意性は発行 allocator 依存。永続化を
  跨ぐ instance 同一性は本 slice 対象外（spec の deferred concerns）。
- `PluginBindingSession::attach` は caller が node/control/process
  の三つ組を整合させる前提（`PluginNodeControl` は node id を公開
  しない）。frame base のみ verified。
- session は `ParamDispatch` を所有しない — driver/adapter 側の
  ownership と generation 更新の配線は後続 slice。

## Follow-ups

- project/workspace/app 層への実配線（他 dispatch 稼働中のため対象外）。
- 実 VST3/OS Backend — Ruviform #196/#307/#253 の別 Owner gate、
  本 slice では NotRun。
- OS MIDI 翻訳 — Ruviform #253 の責務。
