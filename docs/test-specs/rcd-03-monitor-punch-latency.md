---
id: "TSPEC-034Utqp8lRoY8TBzlygCtN"
type: "test-spec"
status: "proposed"
title: "RCD-03 Monitor/Punch/Latency fixture matrix"
created: "2026-11-09"
updated: "2026-11-09"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/117"
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
    - "docs/specs/rcd-03-monitoring-punch-latency.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/rcd-03-monitoring-punch-latency.md"
  verified-by: []
  references: []
  defers: []
  deferred-by: []
---

# RCD-03 Monitor/Punch/Latency fixture matrix

## Purpose

Issue #117 slice の monitor/punch/latency 契約を決定的に検証する
test-spec。fake port・Fixed PCM/Frame Fixture 上の契約検証であり、
実 Device/実 GUI/実 OS の Pass を意味しない（NotRun — 実機 gate）。

## Feature

録音側の Monitor Off/Auto/On × Arm × Transport × Punch ×
Mute/Solo 可聴 route 判定と Raw File Byte の独立比較、
player 側の monitor gain ramp / native placement / software
monitor latency 解決。

## Rules

- **R1 Route 判定**: `decide(ctx, phase, gate_open)` は spec の
  製品ポリシーを写像する — Off: gate open で `Muted`・他は
  `Playback`。Auto(arm): gate open・Stopped で `Input`・他は
  `Playback`。Auto(arm off): 常に `Playback`。On:
  `input_output_ready` で常に `Input`、未検証なら `Playback` +
  `input_unavailable`。
- **R2 Mute/Solo 独立性**: `muted || (any_solo && !soloed)` は
  `Playback` route のみを `Muted` にする。`Input` route と raw
  capture には作用しない（pre-fader monitor）。
- **R3 Hardware Direct**: `observed_active == Some(true)` かつ
  `route == Input` で `double_monitoring`。`state == Unknown` で
  `hardware_direct_unknown`（route 不問）。`observed_active` は
  外部事実であり mute/solo で変更されない。
- **R4 Raw Byte 独立**: 同一入力系列・同一 gate に対し、Monitor
  Off/Auto/On のいずれでも capture された PCM（`ChunkLog`/
  writer 出力）は byte 一致する。Monitor On + Arm Off では
  Writer endpoint が生成されず raw capture も発生しない。
- **R5 Punch 途中境界**: Punch In/Out が block 途中に来ても
  `MonitorProbe` の segment は session-frame 精度で連続し
  （Gap/重複 0）、capture 側の span も `verify_log` で検算
  できる。
- **R6 Loop/Tempo 二重補正 0**: Loop/PunchInLoop・tempo 変化
  区間で `CaptureSpan` の `session_start`/`source_start`/
  `project_in`/`project_out` が gate 権威の一度だけの写像で
  あり、monitor/FX latency の影響を受けない。
- **R7 Gain Ramp**: `MonitorAdapter` は route 遷移時に
  input/playback gain を `ramp_frames` で線形 ramp する
  crossfade を行い、`ramp_frames == 0` では即時切替。ramp は
  DSP のみで gate の sample 精度の代替ではない。
- **R8 Native Placement**: 出力 frame `i` は入力/playback
  frame `i` に対応し、latency/PDC 補正・リサンプル・オフ
  セットを placement に適用しない（`ramp_frames == 0` で
  fixture PCM と bit 一致）。
- **R9 Input Missing fail-closed**: `route == Input` で input
  PCM 未供給なら `Muted` に降格し `input_missing` notice。
  捏造 PCM を出さない。
- **R10 Latency 解決**: `round_trip` 存在時はそれのみが合計
  （個別 input/output/reported を加算しない）。`reported` は
  `reported_verified` 時のみ算入。品質は算入要素の最弱で
  `Measured` は `clock_epoch` 必須。構成要素 0 件は
  `total_frames = None` + `Unknown`。
- **R11 Low Latency Monitor**: `!prepared`/`!input_ready`/
  `!output_ready` は `Unavailable(理由)`、`tone_difference`
  は `Degraded`、他は `Ready`。latency は状態に依らず保持。

## Examples

- **E1 Auto punch 可聴 map**: Arm On + Auto + Playing、
  punch `[in,out)` が block 途中 → gate 閉区間 `Playback`、
  open 区間 `Input` の segment 列。同時に capture log の PCM は
  gate-open 区間の MockPcm のみを含み、Monitor mode を変えても
  byte 一致（R1/R4/R5）。
- **E2 Off gate-open 抑制**: Arm On + Off + gate open 区間 →
  `Muted`（playback 抑制・software input off）。Muted 区間でも
  raw capture は継続し PCM は変わらない（R1/R4）。
- **E3 Arm Off + Monitor On**: `input_output_ready` なら
  `Input` route を返すが Writer/capture は生成されず、
  `arm` は recording を開始しない（R1/R4）。
- **E4 Mute/Solo**: `muted` track の `Playback` route → `Muted`、
  `Input` route は不変。`any_solo` で非 solo track の
  `Playback` → `Muted`（R2）。
- **E5 Hardware Direct**: `observed_active=Some(true)` +
  Software Input 可聴 → `double_monitoring`。`state=Unknown` →
  `hardware_direct_unknown`。mute しても `observed_active` は
  変わらない（R3）。
- **E6 Punch 途中境界**: block size 1024 に対し punch_in/out を
  例えば frame 300/700 に設定 → segment が `[0,300) Playback`、
  `[300,700) Input`、`[700,1024) Playback`（Auto）に分割され
  合計 frame が block と一致（R5）。
- **E7 Loop/Tempo**: PunchInLoop + tempo event を跨ぐ区間で
  `source_start` が pass ごとに連続し、`project_in/out` が
  tempo map で一度だけ写像される（R6）。
- **E8 Gain ramp**: route を Playback→Input に切替、
  `ramp_frames=64` → 遷移 block で input gain が 0→1・
  playback gain が 1→0 に線形変化し、定常後は input PCM が
  gain 1.0 で出力される（R7）。
- **E9 Native placement**: `ramp_frames=0`・route Input で
  fixture PCM を流すと出力が入力と bit 一致（オフセット・
  補正なし）（R8）。
- **E10 Latency round-trip**: `round_trip=Some(480)` +
  `input=Some(100)` + `output=Some(200)` + reported 2 件 →
  `total_frames=480`（580 やそれ以上にならない）、
  `basis=RoundTripMeasured`（R10）。
- **E11 Latency reported 未検証**: `reported_verified=false`
  で reported を供給 → `excluded_reported` に計上され合計に
  入らない。`reported_verified=true` なら算入（R10）。
- **E12 Low latency**: `prepared=false` →
  `Unavailable(NotPrepared)`、`tone_difference=true` →
  `Degraded`（R11）。

## Guarantees

- 全 route 判定が spec の規則表と一致し、Mute/Solo が
  Playback のみに作用する。
- Monitor 状態に依らず Raw File Byte が一致し、Arm Off +
  Monitor On で capture が発生しない。
- Punch/Loop/Tempo で session/source/project の placement が
  一意で Gap/重複/二重補正がない。
- monitor adapter が PCM を正しく混合し、latency を品質・
  epoch 付きで一意に解決する。

## Non-goals

- 実 Device/実 GUI/実 OS での monitor・latency 実測（NotRun）。
- RT graph への monitor node 搭載・transport 配線。
- synvelis-app/workspace/project への統合。
- Hardware Direct の実デバイス制御。
- 多 Window/Focus 変化の GUI 側受入（後続 slice）。

## Risk

route 判定が Issue 本文のポリシー解釈に依存する — spec/design
の規則表を正本とし、解釈が変われば spec から修正する。
fixture 検証は実際の可聴結果・実 latency を保証しないため、
実機受入（#114/#115/#116/#118）で再検証が必要。
