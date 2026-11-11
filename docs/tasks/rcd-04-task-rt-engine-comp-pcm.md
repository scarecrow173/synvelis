---
id: "TASK-034VC2K5DEZwdj418Clksb"
type: "task"
status: "in-progress"
title: "RCD-04 Task RT engine comp PCM"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/plans/rcd-04-rt-engine-comp-pcm-plan.md"
  implemented-by: []
  depends-on:
    - "docs/plans/rcd-04-rt-engine-comp-pcm-plan.md"
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
    - "docs/test-specs/rcd-04-rt-engine-comp-pcm.md"
  references: []
  defers: []
  deferred-by: []
---

# RCD-04 Task RT engine comp PCM

## Work

`crates/synvelis-render/tests/rcd_04_rt_comp_pcm.rs` に実 RT
engine 出力と offline render 出力の comp PCM 照合 integration
test（2 scenario）を実装し、docs チェーン 6 file を populate
する。検証で実欠陥が露呈した場合のみ
`crates/synvelis-player/src/` の最小補修を行う。

## Classification

- `normal-plan-task`

## Upstream Documents

- `docs/plans/rcd-04-rt-engine-comp-pcm-plan.md`
- `docs/specs/rcd-04-rt-engine-comp-pcm.md`
- `docs/designs/rcd-04-rt-engine-comp-pcm-design.md`
- `docs/test-specs/rcd-04-rt-engine-comp-pcm.md`

## Dependencies

- crate fixture slice 統合済み（`rcd_04_comp_cas`/
  `rcd_04_midi_phrase`/`rcd_04_recovery_partial`/
  `rcd_04_take_comp_pcm`）。
- `synvelis-player` の `lower`/`plan_playbacks`/
  `HeadlessDriver`/`RoutingEngine`/`resolver::prepare`/
  `fixture`、`synvelis-render` の `export`/`RenderSnapshot`/
  `expect_pcm` hook、`synvelis-recording` ops（dev-dep 済み）。
- `x_e2e.rs` の playback-vs-export bit-exact 前例（X-01）。

## Affected Files

- `crates/synvelis-render/tests/rcd_04_rt_comp_pcm.rs`（新規）
- `docs/specs/rcd-04-rt-engine-comp-pcm.md`（新規）
- `docs/designs/rcd-04-rt-engine-comp-pcm-design.md`（新規）
- `docs/plans/rcd-04-rt-engine-comp-pcm-plan.md`（新規）
- `docs/tasks/rcd-04-task-rt-engine-comp-pcm.md`（本 file）
- `docs/test-specs/rcd-04-rt-engine-comp-pcm.md`（新規）
- `docs/impl/ir/rcd-04-rt-engine-comp-pcm-slice.md`（新規）
- `docs/impl/ir/README.md` index 行（hand-curated のため手動）
- docs 各ディレクトリ `README.md` index（generator 更新済み）

## Done When

- [x] S1 が `cargo test -p synvelis-render --test
  rcd_04_rt_comp_pcm` で pass し、3 経路 PCM が bit-exact・
  crc 一致・fade/silence/identity anchor が plan-derived 期待
  値に一致する。
- [x] S2 が同じく pass し、Loop/SimpleStretch region に対する
  legacy skip+沈黙・semantic `UnsupportedFormatOrTimeMap`・
  `UnrenderableComp` の fail-closed 分類が記録される。
- [x] test 内に offline renderer の複写実装がない（比較は実装
  経路同士）。
- [x] 実 Device/OS GUI/AudioLens は NotRun/Unsupported として
  文書化されている。
- [ ] canonical mise 検証（fmt-check/check/lint/test/test-rt/
  test-msrv）が green。

## Verification

- [x] `cargo test -p synvelis-render --test rcd_04_rt_comp_pcm`
  — 2 tests pass（補助証拠; canonical = mise）。
- [ ] `mise run fmt-check` / `check` / `lint` / `test` /
  `test-rt` / `test-msrv`（canonical）。
