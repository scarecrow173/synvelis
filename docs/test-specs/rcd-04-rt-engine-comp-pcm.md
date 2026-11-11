---
id: "TSPEC-034VC3cbBMDP6D1WnhLGlh"
type: "test-spec"
status: "approved"
title: "RCD-04 RT engine comp PCM test spec"
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
    - "docs/specs/rcd-04-rt-engine-comp-pcm.md"
    - "docs/designs/rcd-04-rt-engine-comp-pcm-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcd-04-comp-midi-phrase-fixture.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/rcd-04-rt-engine-comp-pcm.md"
    - "docs/designs/rcd-04-rt-engine-comp-pcm-design.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-04 RT engine comp PCM test spec

## Purpose

Issue #118 の Comp Transition 受入条件が要求する「実 RT
engine 出力（`RoutingEngine::run`/`HeadlessDriver::run` の
PCM）と project 層 comp 評価の差を記録する — test 内複写で
はなく実装経路を通した照合」を固定する。crate fixture の
test 内複写 renderer（`rcd_04_take_comp_pcm.rs`）が検出不能
な drift を、3 つの実装経路同士の bit-exact 照合で回帰的に
塞ぐ。実 Device/OS GUI/AudioLens f64 経路は NotRun/
Unsupported であり、headless/fake fixture の成功を実機 Pass
に昇格しない。

## Feature

RCD-04 RT engine comp PCM — deterministic stereo fixture 上に
構築した EffectiveComp（Take/ExistingSource/Silence +
transition + tempo boundary）を、legacy `lower()`→
`HeadlessDriver`・semantic `lower_semantic`→`RoutingEngine`・
offline `lower_semantic_offline`→`export` の 3 経路で実行し、
planar f32 PCM・CRC・skip 記録・fail-closed 分類・project/
media 不変性を照合する。

## Rules

- R1: 比較は実装経路同士 — test は `HeadlessDriver::run`・
  `RoutingEngine::run`・`export` の出力を直接比較し、
  offline 参照を test 内に renderer 複写として再実装しない
  （本 slice が塞ぐ gap の再製を禁止）。plan 由来の参照値
  計算（window 写像 × `FadeWindow::gain_at`）は両経路の共有
  権威からの anchor であり renderer ではない。
- R2: PCM 一致 — 同一 comp fixture・同一 engine geometry
  （48 kHz stereo）・同一 frame 窓について 3 経路の planar
  f32 PCM が全 sample bit-exact で一致し、2 RT 経路の
  `PlayReport.crc` も一致する。同一 build の反復実行で PCM/
  crc が再現する。
- R3: fade/重畳 — transition overlap（EqualPower-out/
  Linear-in・Linear-out/EqualPower-in・silence-side
  fade-in）の probe frame で、plan の window 写像と
  `gain_at` から再導出した両側重畳期待値に出力が一致する。
  identity region・Silence region・target 外の期待値（実
  sample / 0.0）も anchor として固定する。
- R4: tempo/placement — `AddTempoEvent`（120→90 BPM @ tick
  2880）またぎの region/transition は tick→frame 解決を
  `TickTimeMapping` 経由で両経路が一致して解決し、PCM 一致
  が tempo 境界を跨いでも保持される。
- R5: fail-closed 分類 — `Loop`/`SimpleStretch` region を含む
  comp では、legacy lowering が `CompSegment{Unsupported-
  Mapping}` skip を記録し driver は該当区間を沈黙再生する
  （残りの Natural region は実再生）。semantic engine は
  `RoutingFault::UnsupportedFormatOrTimeMap` で build 全体を
  拒否し、`export` は `Failed{Preflight,UnrenderableComp}`
  で失敗閉じる。この部分再生 vs 全体拒否の差は意図された
  skip 契約として記録し、欠陥扱いしない。
- R6: 不変性 — 3 経路の実行前後で project revision・source
  blob bytes・take/asset/registry 状態が変化しない。
- R7: skip inventory — superseded lane clip（`SupersededByComp`）
  と Silence region（`CompSilence`）が明示 skip として記録さ
  れ、余分な skip が無い。
- R8: 証拠分類 — headless/fake fixture 経路の結果のみを
  Passed とし、実 Device/OS GUI/AudioLens f64 は NotRun/
  Unsupported と記録する。

## Examples

- S1（comp PCM 3 経路一致）: stereo 48 kHz `write_fixture`
  WAV → import → track+strip+lane+epoch → 3 pass/segment/
  take（source window [0,120k)/[120k,240k)/[240k,360k)）→
  TakeGroup target `[0,3840)t` → R1 Take@8000・R2 Take@124000・
  R3 ExistingSource{play_from 30000}・R4 Silence・R5
  Take@252000・R6 Take@40000 → T1(EP/Lin)・T2(Lin/EP)・
  T3(silence-side Lin-in) → tempo event(2880→90BPM) →
  superseded lane clip。Given 同一 fixture、When 3 経路を
  `[0,104000)` frame で実行、Then PCM 全 sample bit-exact・
  crc 一致・Silence 窓 [60000,72000) 全 0.0・各 overlap 内
  probe が plan-derived 期待値に一致・revision/bytes 不変。
- S2（fail-closed）: 同一 substrate に R_a Natural・R_b Loop・
  R_c SimpleStretch(2:1)・R_d Natural を含む comp。When 3
  経路を試行、Then legacy は 2 skip 記録+該当区間沈黙・他区
  間実再生、semantic は prepare Err(UnsupportedFormatOrTime-
  Map)、export は Failed{Preflight,UnrenderableComp}。

## Guarantees

- comp 再生について、`ClipPlayer`（RT graph）と
  `ClipRenderer`（offline）の実装差・legacy↔semantic
  lowering の配線差・prefetch/resync と bounded decode の差が
  PCM 上の差異となれば sample/frame 単位で検出される。
- unsupported mapping が暗黙に再生されたり全体失敗を偽装し
  たりしないこと — skip/fail-closed が明示記録される。
- 再生・レンダが project 状態・media bytes を破壊しないこと。

## Non-goals

- 実 device/OS GUI/AudioLens f64/実 Git provider binding 経路
  の照合（#114・Ruviform #294 待ち、NotRun/Unsupported）。
- MIDI phrase 境界・Undo/Redo・VCS diff stable ID（本 slice
  対象外、既存 test の regression のみ確認）。
- `ClipPlayer`/`ClipRenderer` の内部逐次状態の等価性 — PCM
  一致が契約であって実装の同型性ではない。
- fixture test の複写 renderer の削除。

## Risk

- 本 test を落とす/弱めると、crate fixture と実 engine の間の
  最後の未検証ギャップが再び blind spot に戻り、lowering や
  processor への変更が comp 再生を静かに壊しても検出されない。
