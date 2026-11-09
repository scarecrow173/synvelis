---
id: "PLAN-034Uw6FgHg9gQJaig9NTBx"
type: "plan"
status: "approved"
title: "PC-C4 plugin parameter binding plan"
created: "2026-11-09"
updated: "2026-11-09"
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
  implemented-by: []
  depends-on:
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/designs/pc-c4-plugin-param-binding-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/designs/pc-c4-plugin-param-binding-design.md"
  derived-by: []
  verifies:
    - "docs/test-specs/pc-c4-e2e-plugin-param-binding.md"
  verified-by:
    - "docs/test-specs/pc-c4-e2e-plugin-param-binding.md"
  references:
    - "docs/specs/pc-c4-plugin-param-binding.md"
    - "docs/designs/pc-c4-plugin-param-binding-design.md"
  defers: []
  deferred-by: []
---

# PC-C4 plugin parameter binding plan Implementation Plan

## Goal

`synvelis-player` 内に verified frame base・capability gate・
generation pin を持つ plugin parameter binding 層
（`PluginBindingSession`/`PluginParameterBinding`/
`AttachClockContext`/`StablePluginInstanceId`/`BindingError`）を実装し、
`ParamDispatch::new` に attach-time frame base 照合を追加して、
fake plugin port で PLUG-C4-01..05・PC-C4-PL01..08 を実証する。

## Scope

単一 implementation stream — `crates/synvelis-player` 内完結。
project/app/workspace/ruviform への変更なし。

## Upstream Documents

- spec: `docs/specs/pc-c4-plugin-param-binding.md`
- design: `docs/designs/pc-c4-plugin-param-binding-design.md`
- test-spec: `docs/test-specs/pc-c4-e2e-plugin-param-binding.md`

## File Map

- `crates/synvelis-player/src/binding.rs` — 新規 module（全 binding 型）
- `crates/synvelis-player/src/dispatch.rs` — `new` → `Result` 化 +
  `DispatchError::FrameBaseMismatch`
- `crates/synvelis-player/src/lib.rs` — `pub mod binding` + export
- `crates/synvelis-player/src/host.rs` — `ProcessorControl::frame_base`
  doc 整合（comment のみ）
- `crates/synvelis-player/tests/pc_c4_binding.rs` — 新規 fixture test
- `crates/synvelis-player/tests/pc_b.rs` 他既存 test — `new` の
  `Result` 化に伴う call site 更新（`.unwrap()` 追加のみ）

## Dependency Graph

```text
T1 dispatch.rs frame-base 照合（独立・先行）
T2 binding.rs 実装（T1 の DispatchError/ParamDispatch に依存）
T3 lib.rs/host.rs export・doc 整合（T2 に依存）
T4 pc_c4_binding.rs fixture（T1-T3 に依存）
T5 既存 test call site 更新 + 最終検証 + impl record
```

## Risk Register

- `ParamDispatch::new` の `Result` 化による既存 call site 破壊
  → T1 で機械的に全件更新、`cargo check` で網羅確認。
- live `ParamInfo` 照合の過剰厳格化（name/unit は `&'static str` で
  fixture 依存を避けるため flags/range/default のみ比較）
  → design の照合項目を flags_bits/min/max/default に限定。
- `eval_event` の位置写像を session に持たせない
  → caller 供給 `MediaTime` のみ（tempo map は composition root）。

## Rollback

単一 module 追加 + `new` の Result 化。revert で完全に戻る
（永続形式・公開 API の外部契約変更なし）。

## Verification Matrix

| Case | 手段 |
| --- | --- |
| PLUG-C4-01 RT/Offline 一致 | pc_c4_binding.rs render parity |
| PLUG-C4-02 型付き Reject | 同 test の reject 群 assert |
| PLUG-C4-03 複合制御/DAG/不対応 modulation | eval+fixture assert |
| PLUG-C4-04 世代整合 | re-attach/swap/seek/GUI 変更 assert |
| PLUG-C4-05 #307 互換 | frame_base()/check_frame_base assert |
| PC-C4-PL01〜08 | Fixture+Expected/Observed 表 |
| 非回帰 | mise run fmt-check/check/lint/test/test-rt/test-msrv/check-headless-deps |
