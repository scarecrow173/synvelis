---
id: "ADR-034QmbgsQDkLDllHgAxa1f"
type: "adr"
status: "accepted"
title: "L2 — Render slice boundaries・Ruviform sequential offline driver adoption・Staged publish"
created: "2026-11-02"
updated: "2026-11-02"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/25"
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
    - "docs/adr/l1-player-slice-boundaries.md"
    - "docs/specs/l2-master-sequential-offline-wav-export.md"
    - "docs/discovery/l2-master-sequential-offline-wav-export-discovery.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from:
    - "docs/discovery/l2-master-sequential-offline-wav-export-discovery.md"
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  adr:
    decision-makers: ["scarecrow173"]
    consulted: []
    informed: []
---

# L2 — Render slice boundaries・Ruviform sequential offline driver adoption・Staged publish

## Status

Accepted (review-protocol 0 blocker 相当)

## Context and Problem Statement

Issue #25 は Render Input Snapshot の固定・Sequential Offline 駆動・実 PCM WAV
出力・Reopen 検証・Safe Publish を要求する。配置先・駆動委譲先・clip 再生の
供給方式・publish 方針・processor 境界の決定が必要。

## Decision Drivers

- Ruviform 側で rf#191 (`ruviform-io-wav`)/rf#198 (`ruviform-audio::render`) が
  実装済 — 同一 Time/Graph 契約の二重実装は不可 (AGENTS.md 責務原則)。
- Live Playback と Render の Processor/Plugin Instance 共有禁止 (Issue #25)。
- 「#191 実 Writer 不在を手製の仮 WAV で隠さない」「未対応 Plugin を勝手に
  Bypass して Exact 成功を返さない」。
- Failed/Partial/Completed 区別・Reopen 前は Completed 非・Destination/Raw
  Source/Project Saved Revision 保護。

## Considered Options

- **Option 1: `synvelis-render` 新設 + Ruviform `render()` 委譲** —
  RenderSpec/Preflight/Recipe/Job を `crates/synvelis-render` に集約し、
  sequential 駆動は `ruviform_audio::render::{PreparedRender, render}` に委譲。
  依存方向 `synvelis-render → {synvelis-player, synvelis-project, synvelis-recording}`。
- **Option 2: Synvelis 独自 sequential driver** — transport/emit/PDC/tail を
  自前で書く。rf#198 と同一契約の重複実装・検証コスト・乖離リスク。却下。
- **Option 3: `synvelis-player` へ render を追加** — live 再生 crate に
  non-RT render 系列が混在し、「processor instance 非共有」境界が曖昧化。却下。

## Decision Outcome

Option 1 を採用し、以下を確定する。

- **Render Input Snapshot**: `RenderSnapshot` は live `Project` の値コピーではなく
  `Project` 借用 + freeze 時に採取した revision・digest 集合・tempo・clips・
  sources の不変参照を保持 (Live Revision/Last Saved/Active Plan Revision と
  区別して receipt に記録)。snapshot から `lower` + `resolver` を再実行しても
  live とは別インスタンスの graph/processor が compile される。
- **Clip 再生 = decode-to-memory `ClipRenderer`**: prepare 時に window span を
  canonical f32 planar へ decode して保持、process はメモリ供給 (I/O なし)。
  `offline_support() = Verified` を正直に返せる唯一の方式 (prefetch pump は
  render 内部 node に hook なし、boundary feeder 流用は責務混同)。
- **Deterministic DSP**: gain・1-pole・delay の Synvelis 実装 processor を
  `Verified` 付きで提供し L2-03 の block 非変性を検証。Graph の master sink は
  L1 同様 `NodeKind::Sink` + unbounded input (sum)。
- **Output Recipe**: `WavOutputSpec` (wire S16 既定・S24/S32/F32/U8 選択可) +
  `PublishOptions` (replace_existing 既定 false)。変換方針は io-wav 固定値
  (saturate/NaN→0/±Inf→±FS/round-half-away、dither 無し) を receipt に記録。
- **Reopen verify hook**: muxer `finish` 後・publish 前に `staged.temp_path()` を
  独立 reader (symphonia demux → `SourceReader`) で開き、`OutputFacts` と
  decoded frames/channels/rate/PCM を期待値と比較。失敗は `RenderError::Verify`
  で temp 破棄・destination 不変。
- **Publish policy**: `replace_existing=false` では destination 存在が preflight
  (`DestinationExists`) と publish 双方で拒否。replace=true は同 volume rename。
- **Job 状態**: `ExportJob` が `Completed { receipt, verify }` /
  `Failed { stage, error }` / `Cancelled { phase }` を返す。Partial 状態は
  receipt を持たず Completed と表示しない。
- **Revision 更新**: Ruviform lock を `322a661` (rf#198 merge) へ再生成、
  `ruviform-io-wav`/`ruviform-exec` を workspace 依存に追加。

### Consequences

- 良: rf#198 契約 (PDC/preroll/tail/segmentation/staged lifecycle) を
  そのまま再利用、L2-07 証拠が `PreparedRender::compile`/`capabilities()`/
  `wav_capabilities()`/`RenderReceipt` で自然に取得できる。
- 良: clip/DSP processor が `Verified` を正直に返せ、Unknown/Declared は
  ruviform preflight が拒否する (偽成功の構造的不可能化)。
- 悪: decode-to-memory は長尺 source でメモリ増 — L2 範囲 (決定的 fixture) では
  問題外、spec の Deferred Design Concerns に記録。

### Confirmation

L2-01〜08 acceptance suite (`crates/synvelis-render/tests/l2_e2e.rs`) +
`cargo test --workspace --locked` 全 PASS。

## Implementation Plan

* Affected paths: `crates/synvelis-render/**` 新設、`Cargo.toml` (workspace
  members + workspace deps)、`Cargo.lock` 再生成
* Patterns to follow: L1 の typed error・fixture・independent-reader verify・
  `publish_atomic`/`StagedOutput` publish 契約
* Patterns to avoid: prefetch ring の render 転用・boundary feeder の
  非入力用途・独自 sequential driver・手製 WAV writer (rf#191 不使用の隠蔽)
* Dependencies or configuration: `ruviform-io-wav`/`ruviform-exec` 追加、
  lock `322a661`

## Verification

* [x] `cargo check --workspace --all-targets` (lock `322a661` 後) PASS
* [ ] `mise run test` (`cargo test --workspace --locked`) 全 green
* [ ] L2 acceptance suite で reopen verify・fault injection・snapshot invariance

## Pros and Cons of the Options

### Option 1 (採用)

* Good, because rf#198 実装済契約をそのまま使い L2-07 証拠が構造的。
* Good, because render slice が独立 crate で live との instance 非共有が明快。
* Bad, because ruviform API 形状への適合 (RenderTransport/SpanSink/OutputFacts)
  の習得コスト。

### Option 2

* Good, because 制御が完全に手元にある。
* Bad, because rf#198 と同一契約の二重実装 — AGENTS.md「Ruviform の問題を
  Synvelis 側の恒久的な重複実装で隠さない」に反する。

### Option 3

* Good, because crate 追加なし。
* Bad, because live/render の責務混同・processor instance 非共有境界の曖昧化。

## More Information

- `docs/discovery/l2-master-sequential-offline-wav-export-discovery.md`
  (alternatives A–E・API 検読記録)
- Ruviform PR #229 (rf#191)・#235 (rf#198) merge commit `2089ca0`/`322a661`
