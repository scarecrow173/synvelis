---
id: "IMPL-034VC2V5HUrkERcXqw246v"
type: "impl"
status: "in-progress"
title: "RCD-04 RT engine comp PCM slice 実装記録"
created: "2026-11-11"
updated: "2026-11-11"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/118"
  changes:
    added:
      - type: "file"
        path: "crates/synvelis-render/tests/rcd_04_rt_comp_pcm.rs"
      - type: "file"
        path: "docs/specs/rcd-04-rt-engine-comp-pcm.md"
      - type: "file"
        path: "docs/designs/rcd-04-rt-engine-comp-pcm-design.md"
      - type: "file"
        path: "docs/plans/rcd-04-rt-engine-comp-pcm-plan.md"
      - type: "file"
        path: "docs/tasks/rcd-04-task-rt-engine-comp-pcm.md"
      - type: "file"
        path: "docs/test-specs/rcd-04-rt-engine-comp-pcm.md"
      - type: "file"
        path: "docs/impl/ir/rcd-04-rt-engine-comp-pcm-slice.md"
    modified:
      - type: "file"
        path: "docs/impl/ir/README.md"
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/tasks/rcd-04-task-rt-engine-comp-pcm.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/impl/ir/rcd-04-comp-transition-midi-phrase-crate-fixture-slice.md"
    - "docs/impl/ir/rcd-04-product-e2e-slice.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted: []
    rejected: []
---

# RCD-04 RT engine comp PCM slice 実装記録

## Summary

`crates/synvelis-render/tests/rcd_04_rt_comp_pcm.rs`（新規・2
scenario）を追加し、EffectiveComp の再生を **実装経路 3 系統**
（legacy `lower()`+`HeadlessDriver::run`、semantic
`lower_semantic`+`RoutingEngine::run`、offline
`lower_semantic_offline`+`export()`→`ClipRenderer`+F32 WAV reopen）
で bit-exact 照合する integration test を実装した。crate fixture
の test 内複写 renderer では検出不能だった「実 engine vs project
層 comp 評価」の drift gap を塞ぐ。

結果: 全 sample bit-exact 一致（両 RT 経路の `PlayReport.crc` も
一致）。不一致の源（fade 係数・placement・block 境界）は発生せ
ず、`synvelis-player/src` への補修は不要だった — gap は実装欠陥
ではなく検証不在だった。Loop/SimpleStretch では、legacy 経路の
explicit-skip 沈黙再生と semantic/export の build 全体拒否という
意図された fail-closed 挙動差を記録した。

## Implementation

### S1 `rcd_04_rt_engine_comp_pcm_matches_offline_export`

- stereo 48 kHz `write_fixture` WAV → `import_local` →
  track+strip+lane+epoch → 1 session/1 stream + 3 pass/segment/
  take（source window [0,120k)/[120k,240k)/[240k,360k) ずつ、相関
  素材の代替）→ `MarkTakeVerified` → TakeGroup target
  `[0,3840)t` → 6 region（Take×4 / ExistingSource{handle,
  play_from} / Silence）→ 3 transition（EP-out/Lin-in、
  Lin-out/EP-in、silence-side Lin-in）→ `AddTempoEvent`
  （2880→90BPM、tick→frame が非一様 100/3 f/t に）→ target 内
  lane clip（superseded）。
- `plan_playbacks` の inventory（CompSegment×5・CompOverlap×5・
  SupersededByComp・CompSilence）と、tempo 境界を実際に跨いだ
  window 解決（R5→[72000,88000)、R6→[88000,104000)、T3R→
  [70500,72000) — 既定 120BPM なら [72000,84000)/[84000,96000)
  になることを明示対比）を断言。
- 3 経路を `[0,104000)` で実行し `assert_planes` で全 sample
  bit-exact、`PlayReport.crc` 一致、fresh lower+prepare+run で
  crc/pcm 再現（determinism）。
- plan-derived anchor（`expected_at` = Σ window 写像 ×
  `FadeWindow::gain_at`）で identity/fade/silence/overhang/
  post-tempo の 13 probe frame を固定 — 共有権威からの参照値で
  あり renderer 複写ではない。
- project revision・source digest・blob bytes・take/asset 数の
  不変を確認。

### S2 `rcd_04_rt_engine_unsupported_mapping_fails_closed`

- Natural + Loop + SimpleStretch(2:1) + Natural の comp。
- legacy: `CompSegment{UnsupportedMapping}` skip ×2 が明示記録さ
  れ、driver は [24000,72000) を沈黙再生・両端の Natural 区間は
  実 sample 再生。
- semantic: `RoutingEngine::prepare` →
  `Err(RoutingFault::UnsupportedFormatOrTimeMap)`（build 全体拒
  否）。
- offline: `export` → `Failed{stage:Preflight, error:Preflight
  (UnrenderableComp)}`。
- 部分再生 vs 全体拒否の差は意図された skip 契約として記録
  （欠陥ではない）。

## Decisions

- oracle 方針 (a) 採用: `export()`（実 offline render 実装経路）
  を offline 参照とし、test 内複写 renderer は作らない — design
  doc に採否と理由を記録。
- test 配置 (ii) `synvelis-render/tests/` 採用（coordinator 決
  定）: render→player 既存依存で 3 経路すべてを駆動でき、
  manifest 変更不要・`x_01` playback-vs-export 前例と同配置。
  `crates/synvelis-player/Cargo.toml` への dev-dep 追加は否認済
  み。

## Related Experiments

### Adopted

- `crates/synvelis-player/tests/md_b.rs` の comp 構築パターン
  （recording ops → take → TakeGroup → Comp → Transition）。
- `crates/synvelis-render/tests/x_e2e.rs` の export+expect_pcm・
  scratch/apply/prefetch helpers・assert_planes・trim パターン。
- `crates/synvelis-player/tests/sf03_l6.rs` の
  RoutingCompileRequest/RoutingDeps/prepare→publish→run。

### Rejected

- test 内への offline renderer 複写 — 検証対象の gap そのものの
  再製（受入条件が明示禁止）。
- `player/tests` + dev-dep 配置 — manifest 変更が目的上の価値な
  く範囲外。

## Validation

- `cargo test -p synvelis-render --test rcd_04_rt_comp_pcm` —
  2 tests pass（補助証拠; canonical = mise）。
- canonical mise lanes（fmt-check/check/lint/test/test-rt/
  test-msrv）は提出前に実行し結果を PR 本文に記録する。

## Risks

- headless + deterministic fixture 経路の一致であり、実 Device/
  OS GUI/AudioLens f64 の証拠ではない — NotRun/Unsupported とし
  て維持（#114・Ruviform #294 で後続）。
- 3 経路が共有する `plan_playbacks`/`FadeWindow`/`TickTime-
  Mapping` 自体の欠陥は構造上検出できない — plan-derived anchor
  が placement/envelope の絶対値を固定して緩和しているが、
  mapping の解釈ミス自体は domain test（md_a 系）の守備範囲。

## Follow-ups

- fixture test の複写 renderer 削除/統合判断 — 別 slice。
- 実 device/OS GUI/AudioLens 経路の同条件照合 — #114・Ruviform
  #294 待ち。
