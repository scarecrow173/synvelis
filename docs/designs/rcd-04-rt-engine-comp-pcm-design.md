---
id: "DESIGN-034VBU0PCNZXCZsyRneE3S"
type: "design"
status: "approved"
title: "RCD-04 RT engine comp PCM design"
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
    - "docs/specs/rcd-04-rt-engine-comp-pcm.md"
  implemented-by: []
  depends-on:
    - "docs/designs/rcd-04-comp-transition-midi-phrase-fixture.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-04-product-e2e-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers:
    - "実 device/OS GUI・AudioLens native-f64 consumer 経路の PCM 照合（#114・Ruviform #294 待ち）"
  deferred-by: []
---

# RCD-04 RT engine comp PCM design

## Context

crate fixture slice（`rcd_04_take_comp_pcm.rs`）は Comp の domain
契約を test 内複写 renderer で固定したが、その renderer は
`lowering.rs` 規則の再実装であり、実 engine 経路
（`ClipPlayer`/RT graph/`RoutingEngine`）や offline render 実装
（`ClipRenderer`/`export`）との drift を構造的に検出できない。
Issue #118 の受入は「実 RT engine 出力（`RoutingEngine::run`/
`HeadlessDriver::run` の PCM）と project 層 comp 評価の差を記録
する」ことを要求し、「test 内複写ではなく実装経路を通した照合」
を明示する。

3 つの実装経路が存在する:

- **legacy RT**: `lower()` → `ClipPlayer` 群 → `HeadlessDriver::run`
  → `PlayReport{pcm, crc}`（`md_b.rs`・`x_e2e.rs` が消費）。
- **semantic RT**: `RoutingEngine::prepare` → `lower_semantic` →
  `ClipPlayer` 群 → `RoutingEngine::run` → `PlayReport{pcm, crc}`
  （`sf03_l6.rs` が消費）。非意図 skip は fail-closed。
- **offline**: `export()` → `lower_semantic_offline` →
  `ClipRenderer` 群 → F32 WAV → reopen → `expect_pcm` hook が
  decoded planar f32 を捕捉（`x_e2e.rs` X-01 が playback との
  bit-exact を既に固定）。

すべての経路は `plan_playbacks`（placement/window/fade の共有権
威）と `FadeWindow::gain_at`（envelope の共有権威）を消費する
ため、不一致が出れば lowering 出力後の processor 実装差
（`ClipPlayer`↔`ClipRenderer`）または semantic 配線差に限定で
きる。

## Decision: oracle 方針 — (a) 採用

前提の 2 択に対し **(a) fixture の offline renderer を権威ある共
有実装へ寄せる（`synvelis-render` 消費）** を採用する。

- (a) は `export()` という *実装された* offline render 経路を
  oracle に据える — `ClipRenderer`・preflight・WAV reopen を含む
  製品経路そのもので、test 内複写を新たに作らない。Issue 受入の
  「実装経路を通した照合」に正面から一致する。
- (b)（engine 出力を権威として oracle で独立検証）は「第 3 の
  renderer」を test に置くことを要求し、検証対象の gap（複写
  renderer と実装の drift）を別の複写で再製する。却下。
- X-01（`x_e2e.rs`）が playback==export bit-exact を先行実証済
  みであり、本 design はその前例の comp 拡張であって新規手法で
  はない。

## Decision: test 配置 — (ii) `synvelis-render/tests/` 採用

task spec の記述は `crates/synvelis-player/tests/` だったが、
`player/tests` から `synvelis-render` を消費するには
`crates/synvelis-player/Cargo.toml` への dev-dependency 追加が必
要で、これは編集許可範囲外。coordinator 質問に対し **(ii)
`crates/synvelis-render/tests/rcd_04_rt_comp_pcm.rs` を採用、
manifest 範囲拡大は許可しない** の回答を得た（理由:
render→player の既存依存で両 engine と `export` を駆動でき、検証
目的は同等。(i) の dev-dep 逆循環に目的上の価値がない。manifest
不要・`x_01` 前例と同配置・lease 内完結で厳密に優れる）。

## Design

### Fixture 構成（scenario S1: 混合 comp）

- stereo 48 kHz `write_fixture` WAV 1 本（deterministic
  `fixture_sample`、(channel, frame) の純関数、PCM16 は
  `i16/32768.0` で正確に表現可能 → bit-exact 比較が意味を持つ）。
- recording ops で session→stream→pass(0,1,2)→segment→take を 3
  take 登録（同一 source の異なる `source_range` window = 相関素
  材の代替。`MarkTakeVerified` 済み）。
- `AddTrack`+`AddTrackStrip`+`AddLane`+`RegisterClockEpoch`
  （semantic lowering は clip/owner lane の track に strip が必
  要）。
- comp target `[0,3840)` tick、6 region + 3 transition:

  | region | tick | candidate | mapping | 条件 |
  | --- | --- | --- | --- | --- |
  | R1 | [0,960) | Take t0 @+8000 | Natural | identity |
  | R2 | [960,1920) | Take t1 @+4000 | Natural | 相関素材 |
  | R3 | [1920,2400) | ExistingSource{handle, play_from=30000} | Natural | Source Handle |
  | R4 | [2400,2880) | Silence | — | explicit silence |
  | R5 | [2880,3360) | Take t2 @+12000 | Natural | tempo change 後 |
  | R6 | [3360,3840) | Take t0 @+40000 | Natural | take 再利用 |

  - T1 R1/R2: overlap [900,1020), EqualPower-out / Linear-in。
  - T2 R2/R3: overlap [1860,1980), Linear-out / EqualPower-in。
  - T3 R4/R5: overlap [2820,2940), Linear/Linear — 左側が
    Silence なので fade-in 側ノードのみ（`transition_side` が
    `None` を返す経路）。
- `AddTempoEvent{tick:2880, bpm_milli:90_000}` — 120→90 BPM で
  tick→frame 変換が非一様（25 f/t → 33.33… f/t）になり、R5/R6・
  T3 の placement 解決を tempo 境界またぎで検証する。
- target range 内に lane clip を 1 本置き
  `SupersededByComp` skip を両経路で確認する。

### 駆動と照合

1. `plan_playbacks(project, canonical_of, eng)` → plans/skips の
   inventory を確認（CompSegment×5 + CompOverlap×5 +
   SupersededByComp + CompSilence、余分な skip なし）。
2. **legacy RT**: `lower()` → `prepare` per playback →
   `HeadlessDriver::activate` → `run(blocks, Every(1))` →
   `PlayReport`。
3. **semantic RT**: `RoutingDeps{clips: plans.map(prepare)}` →
   `RoutingEngine::prepare` → `expect_built` → `publish` →
   `run(blocks, Every(1))` → `PlayReport`。
4. **offline**: `RenderSnapshot::freeze` → `export(F32,
   TailPolicy::None)` → `expect_pcm` が reopened plane を捕捉 →
   `ExportOutcome::Completed` + `receipt.verified.reopened`。
5. 断言: `driver.pcm == engine.pcm == export_planes`（全 sample
   bit-exact）、`driver.crc == engine.crc`、同一 build 反復で
   crc 再現。
6. plan 由来の期待値アンカー: identity frame・各 overlap 内
   probe frame で Σ(plan.window 写像 × `fade.gain_at`) を再導出
   し一致を確認（all-silence や配置ずれの誤検出を塞ぐ）。これ
   は複写 renderer ではなく、両経路が共有する plan/envelope 権
   威からの参照値。
7. 不変性: 再生・レンダ前後で `live_revision`・source blob
   bytes・take/asset 数が不変。

### scenario S2: unsupported mapping の fail-closed 記録

Loop region + `SimpleStretch(2:1)` region を含む comp:

- legacy `lower()`: `CompSegment{UnsupportedMapping}` skip が記録
  され、driver は該当区間を沈黙再生（残りの Natural region は実
  再生）。
- `RoutingEngine::prepare` → `Err(UnsupportedFormatOrTimeMap)`
  （semantic RT は build 全体を拒否）。
- `export` → `Failed{stage:Preflight, error:UnrenderableComp}`。
- 3 経路の振る舞い差（部分再生 vs 全体拒否）は「正直な差異」と
  して test コメントと本 doc に記録する — 欠陥ではなく、skip
  契約の意図された挙動。

### 証拠分類

- 本 test が示すのは **headless + deterministic fixture 経路**の
  一致であって、実 Device/OS GUI/AudioLens の Pass ではない。
  PR・doc・受入記録で `NotRun`/`Unsupported` の区分を維持する。

## Alternatives Considered

- (b) engine を権威とする独立 oracle — 上記の通り複写 gap を再
  製するため却下。
- (i) `player/tests` + dev-dep — coordinator が manifest 変更を
  否認、目的上の価値もないため却下。
- `synvelis-recording` fixture の複写 renderer を再利用 —
  `synvelis-recording` は player/render に依存できず共有実装に
  なり得ない（依存方向の維持）。却下。

## Risks

- `ClipPlayer`（prefetch/resync 経由）と `ClipRenderer`（bounded
  window decode）の実装差が浮上した場合 — 不一致の sample・
  frame・plan を証拠に fade 係数・placement・block 境界の源を
  特定して修正する。修正が `synvelis-player/src` 最小補修を超え
  る場合は coordinator に範囲拡張を求める。
- tempo 境界の tick→frame 丸め（非整数 f/t）— 両経路が同一の
  `TickTimeMapping` を消費するため一致は構造上保証されるが、
  plan 由来アンカーでも frame 位置を mapping 経由で再導出して
  固定する。
