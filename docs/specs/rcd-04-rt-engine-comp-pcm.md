---
id: "SPEC-034VBTXNYEeuSEzbHwk1hv"
type: "spec"
status: "proposed"
title: "RCD-04 Comp Transition — RT engine PCM equivalence (synvelis#118)"
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
  implements: []
  implemented-by: []
  depends-on:
    - "docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md"
    - "docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-04-product-e2e.md"
    - "docs/specs/x-recorded-verified-source-playback-export.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/10"
    - "https://github.com/scarecrow173/synvelis/issues/11"
    - "https://github.com/scarecrow173/synvelis/issues/42"
    - "https://github.com/scarecrow173/synvelis/issues/50"
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/126"
  defers: []
  deferred-by: []
---

# RCD-04 Comp Transition — RT engine PCM equivalence (synvelis#118)

## What Should Be Built

`EffectiveComp` 投影（MD-A/MD-B で確立した Take/Comp 契約）を **実
RT engine 経路**で再生したときの PCM を、**offline render 実装経
路**の出力と bit-exact で照合する integration test。照合対象は
3 経路:

- `HeadlessDriver::run`（legacy `lower()` → `ClipPlayer` → RT
  graph）が返す `PlayReport.pcm`/`crc`
- `RoutingEngine::prepare`/`publish`/`run`（semantic
  `lower_semantic` → `ClipPlayer` → RT graph）が返す
  `PlayReport.pcm`/`crc`
- `synvelis_render::export`（`lower_semantic_offline` →
  `ClipRenderer` → F32 WAV、`expect_pcm` hook で reopened plane
  を捕捉）

crate fixture slice の offline renderer は `lowering.rs` 規則の
test 内複写であり、実 engine との drift を検出できない。本 slice
はその未検証ギャップを塞ぎ、**実装経路同士**の一致を固定する。

## Why Now

Issue #118 の受入条件は Comp 条件（Source Handle・Fade・相関素材・
Transient・Beat Anchor・Tempo・Loop）について「実 RT engine 出力
と project 層 comp 評価の差を記録する」ことを要求する。現状の
fixture test（`rcd_04_take_comp_pcm.rs`）は test 内複写 renderer
同士の一致しか示せず、`ClipPlayer`/`ClipRenderer` の実装差・
prefetch/resync 挙動・block 境界処理の差異は一切検出されないまま
残っている。

## Users and Value

- Issue #118 レビュア: RT/offline PCM 一致の実測根拠（fake
  fixture 証拠と実機証拠の区別を維持したまま）。
- Comp/lowering 実装者: `ClipPlayer`↔`ClipRenderer`・legacy
  `lower`↔`lower_semantic` の drift を PR 段階で検出する回帰
  gate。

## Scope

### In Scope

- `crates/synvelis-render/tests/rcd_04_rt_comp_pcm.rs`（新規
  integration test; 配置は coordinator 決定 (ii) — `render` が
  `player` に既存依存するため manifest 変更不要、`x_01`
  playback-vs-export 前例と同じ配置）。
- deterministic fixture: `write_fixture` WAV（stereo 48 kHz）+
  recording ops 経由の take 登録 + comp ops 経由の
  TakeGroup/Comp/Region/Transition 構築。
- Comp 条件の網羅: `CompCandidateRef::Take`（`source_offset` 違
  いの相関素材）・`ExistingSource{handle, play_from}`（Source
  Handle 範囲）・`Silence`・`AddCompTransition`（EqualPower /
  Linear 両方向 + silence-side）・`PlaybackMapping::Natural`・
  `Loop`/`SimpleStretch`（fail-closed skip）・`AddTempoEvent`
  またぐ Musical range（Tempo/Beat Anchor の非一様 tick→frame
  変換）・superseded lane clip。
- 3 経路の planar f32 PCM の bit-exact 一致・`PlayReport.crc`
  一致・同一 build の反復実行決定性。
- source/media bytes・project revision・take/asset 状態の不変性
  確認。
- Loop/SimpleStretch を含む comp に対する各経路の fail-closed
  挙動の正直な記録（legacy driver は explicit-skip 沈黙再生、
  semantic engine は `RoutingFault` 拒否、export は
  `PreflightError::UnrenderableComp` 拒否）。
- 既存 fixture test は維持（複写 oracle は削除しない）。

### Out of Scope

- 実 Git provider binding・実 device/OS GUI（#114）・MIDI comp
  selection 永続化（#126）・AudioLens native-f64 consumer gate
  （Ruviform #294）・実 RT audio device の sample 一致。
- `crates/synvelis-player/Cargo.toml` など manifest 変更
  （coordinator が範囲拡大を否認済み）。
- 複写 oracle の削除・置き換え（fixture test の縮小は本 slice
  対象外）。
- 新しい Comp domain model の導入。

## Requirements

- 比較は **実装経路同士**で行うこと — test 内に offline
  renderer を複写しない（本 slice が塞ぐ gap そのもの）。
- `plan_playbacks` を共有権威として legacy/semantic 両 lowering
  が消費することを前提に、`RoutingDeps.clips` は plan 順に
  `resolver::prepare` で構築する。
- 不一致が観測された場合は合成で誤魔化さず、fade 係数・
  placement・block 境界等の源を特定して修正するか、限度と根拠
  を記録する。
- headless/fake fixture の成功を実 Device/OS/GUI/AudioLens の
  Pass に昇格しない — 証拠分類を正直に記録する。
- `Tick`/`TickDuration` の unchecked 演算を Synvelis 側で直接使
  わない（時間演算はチェック付き、または実装の公開 API 経由）。

## Acceptance Criteria

- [ ] 混合 comp（Take/ExistingSource/Silence + 3 transitions +
  tempo event またぎ）について、`HeadlessDriver::run`・
  `RoutingEngine::run`・`export` の 3 経路 PCM が全 sample
  bit-exact 一致し、driver/engine の `crc` も一致する。
- [ ] Fade 区間（EqualPower-out/Linear-in・Linear-out/EqualPower-
  in・silence-side fade-in）で両側 read の重畳と `FadeWindow`
  envelope が plan から再導出した期待値と一致する。
- [ ] Silence region・target range 外が 0.0、Natural region が
  非ゼロの実 sample を含む（all-silence 誤検出の防止）。
- [ ] Loop/SimpleStretch region を含む comp について、legacy
  lowering は `CompSegment{UnsupportedMapping}` skip を記録し
  driver は該当区間を沈黙再生、`RoutingEngine::prepare` は
  `RoutingFault::UnsupportedFormatOrTimeMap`、`export` は
  `Failed{Preflight, UnrenderableComp}` で失敗閉じる。
- [ ] 再生・レンダ後に project revision・source bytes・
  take/asset 状態が不変。
- [ ] `mise run fmt-check`・`check`・`lint`・`test`・`test-rt`・
  `test-msrv` が通る（実行したものだけを Passed と記録）。

## Deferred Design Concerns

- 実 device/OS GUI・AudioLens f64 経路での同条件照合 — 実機
  証拠が必要で headless fixture では Unsupported（#114・
  Ruviform #294 待ち）。
- fixture test の複写 renderer 削除 — 本照合の安定後に別 slice
  で判断。

## Sources and Relations

- Issue #118（RCD-04 受入条件の転記元）: Comp Transition 条件の
  RT/offline PCM 照合。
- `docs/specs/rcd-04-comp-transition-midi-phrase-fixture.md` —
  crate fixture slice（test 内複写 renderer の権威不在 gap を
  特定した前提）。
- `docs/specs/mda-takegroup-comp-effective-projection-placement-contract.md`
  — EffectiveComp 投影・placement 契約の上流仕様。
- `docs/specs/x-recorded-verified-source-playback-export.md` —
  playback-vs-export bit-exact 前例（X-01）。
