---
id: "PLAN-034QmjyxIXmPM1qiRQc6Hn"
type: "plan"
status: "approved"
title: "L2 実装計画 — Master Sequential Offline PCM WAV Export"
created: "2026-11-02"
updated: "2026-11-02"
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
    - "docs/specs/l2-master-sequential-offline-wav-export.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/specs/l2-master-sequential-offline-wav-export.md"
    - "docs/designs/l2-master-sequential-offline-wav-export-design.md"
  derived-by: []
  verifies: []
  verified-by:
    - "docs/test-specs/l2-master-sequential-offline-wav-export.md"
  references: []
  defers: []
  deferred-by: []
---

# L2 実装計画 — Master Sequential Offline PCM WAV Export Implementation Plan

## Goal

`synvelis-render` 新設で RenderSpec snapshot・preflight・Ruviform `render()`
委譲・実 PCM WAV (io-wav)・reopen verify・safe publish・typed job を実装し、
L2-01〜08 acceptance を実測 PASS させる。

## Scope

単一実装ストリーム。境界: Synvelis は snapshot/recipe/policy/job を所有、
generic graph/codec/transport/staging は Ruviform へ委譲。Ruviform は変更しない。

## Upstream Documents

- docs/specs/l2-master-sequential-offline-wav-export.md
- docs/adr/l2-render-slice-boundaries.md
- docs/discovery/l2-master-sequential-offline-wav-export-discovery.md

## Design Inputs

- docs/designs/l2-master-sequential-offline-wav-export-design.md

## Task decomposition

1. **l2-task1** — workspace: `synvelis-render` crate skeleton + workspace
   member/deps (`ruviform-io-wav`, `ruviform-exec`) + lock `322a661` 確認。
2. **l2-task2** — snapshot + preflight: `RenderSnapshot::freeze`,
   `RenderSpec`/`OutputRecipe`/`PublishPolicy`/`Destination`、typed
   `PreflightError` 一括検出。
3. **l2-task3** — render core: clip decode-to-memory `ClipRenderer` +
   deterministic DSP (gain/1-pole/delay) `Verified` processors、
   `PreparedRender::compile`、`RenderTransport` (Seek→Play, Exact, preroll)、
   io-wav encoder/muxer 接続、`render()` 駆動。
4. **l2-task4** — verify + publish + job: reopen verify hook (独立 reader)、
   `StagedOutput` publish policy、`ExportOutcome`/`ExportStage`、
   progress/cancel 配線。
5. **l2-task5** — `tests/l2_e2e.rs` L2-01〜08 受入 + 検証 lane + impl record。

## File Map

- `Cargo.toml` — member 追加 + workspace deps (`ruviform-io-wav`, `ruviform-exec`)
- `crates/synvelis-render/Cargo.toml` — crate manifest
- `crates/synvelis-render/src/lib.rs` — public API surface
- `crates/synvelis-render/src/spec.rs` — `RenderSpec`/`OutputRecipe`/`PublishPolicy`/`TailSpec`
- `crates/synvelis-render/src/snapshot.rs` — `RenderSnapshot::freeze` + SourcePin
- `crates/synvelis-render/src/preflight.rs` — `PreflightError` 一括検出
- `crates/synvelis-render/src/prepare.rs` — clip decode-to-memory + DSP graph assembly
- `crates/synvelis-render/src/processor.rs` — `ClipRenderer`/`GainProc`/`OnePoleProc`/`DelayProc`
- `crates/synvelis-render/src/output.rs` — io-wav encoder/muxer + recipe 写像
- `crates/synvelis-render/src/verify.rs` — reopen verify hook (独立 reader)
- `crates/synvelis-render/src/job.rs` — `ExportOutcome`/`ExportStage`/`ExportReceipt`
- `crates/synvelis-render/src/export.rs` — orchestration `export()`
- `crates/synvelis-render/src/error.rs` — typed error
- `crates/synvelis-render/tests/l2_e2e.rs` — L2-01〜08 受入
- `docs/impl/ir/l2-master-sequential-offline-wav-export.md` — impl record

## Dependency Graph

task1 → task2 → task3 → task4 → task5 (直列; task2..4 は同一 crate 内の層順)。

## Risk Register

| Risk | Mitigation |
| --- | --- |
| ruviform render() API 形状の誤読 (RenderTransport/SpanSink/OutputFacts) | upstream 実テスト (rf#198 merge の tests) を写像元に採用 |
| emit window と position/PDC 写像の off-by-one | L2-02 境界テスト (delayed start/gap/負 preroll/境界直前終端) で exact frame 検証 |
| decode-to-memory が大きすぎる | L2 fixture 小規模; deferred concern に記録 |
| Staging/verify 失敗後の destination 破損 | StagedOutput + verify hook を publish 前に置く (ruviform 保証) |

## Rollback

- crate 追加のみ — 既存 crate への変更は workspace `Cargo.toml` members/deps のみ。
  ロールバックは member 除去 + lock 再生成。
- Destination 保護: `replace_existing=false` 既定 + publish 失敗時 temp 破棄。

## Verification Matrix

| 受入 | 検証 |
| --- | --- |
| L2-01 | tests/l2_e2e.rs `l2_01_*` reopen compare |
| L2-02 | `l2_02_*` boundary/gap/preroll |
| L2-03 | `l2_03_*` block 64/128/256 invariant |
| L2-04 | `l2_04_*` unknown tail/unsupported plugin fail |
| L2-05 | `l2_05_*` fault injection no-partial-completed |
| L2-06 | `l2_06_*` snapshot/source hash invariance |
| L2-07 | `l2_07_*` capability evidence + MR/EX/TT trace |
| L2-08 | impl record に command/hash/frames/toolchain/exit/未計測 |
