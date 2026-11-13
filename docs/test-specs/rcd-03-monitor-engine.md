---
id: "TSPEC-034WCwsH5OmvKwwZocdwPT"
type: "test-spec"
status: "draft"
title: "RCD-03 monitor engine — test spec"
created: "2026-11-13"
updated: "2026-11-13"
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
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcd-03-monitoring-punch-latency.md"
    - "docs/test-specs/rcd-03-monitor-gate.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/rcd-03-monitor-engine.md"
  derived-by: []
  verifies:
    - "docs/specs/rcd-03-monitor-engine.md"
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/117"
  defers: []
  deferred-by: []
---

# RCD-03 monitor engine — test spec

## Purpose

`MonitorEngine`/`MonitorSession` の engine 実経路搭載と
Loop/PunchInLoop gate の headless fixture 契約を固定する。
実 device/実 GUI 受入の代替ではなく、crate 内契約の
fixture 検証である。

## Feature

Engine-mounted monitor bus delivers sample-accurate,
half-open gated monitor PCM derived from the real engine
playback path, with Loop/PunchInLoop passes producing
contiguous, non-overlapping route segments.

## Rules

- Monitor bus は engine 通常出力・raw capture・export を
  変更しない独立経路。monitor 処理は `PlayReport.pcm`
  （master tap の実経路 PCM）を playback 側として消費する。
- `MonitorSession` は `MonitorProbe`（`GateScheduler` 共有）
  + `TrackMonitorContext` + `TransportPhase` + `gate_open`
  から `decide` の `MonitorRoute` を block 内 session-frame
  精度で segment 分割する。gate ロジックの第二実装なし。
- `MonitorEngine` は engine frame ↔ session frame を
  1:1（`plan.sample_rate == engine.sample_rate` を構築時
  検証）で対応させ、attach 時 `frame_offset` からのずれを
  検出したら fail-closed。
- `TransportPhase`・入力 PCM は block 単位で caller が供給。
  未供給 block は `input = None` となり route Input は
  Muted へ fail-closed 降格 + `input_missing` notice。
- monitor 出力 PCM は native placement を保ち、latency/PDC
  補正・リサンプル・オフセットを placement に適用しない
  （gain ramp は DSP のみ）。
- segment は block を隙間・重複なく tile する。違反・
  probe/adapter 契約違反・session 位置ずれは fail-closed
  （無音 + faulted 観測可能）。
- session 終端を越えた block 区間は Playback 経路。
- `GateSpec::Loop`/`PunchInLoop` で block 途中の Punch
  In/Out・Loop pass 境界が来ても monitor 出力の Gap/重複 0・
  Raw Placement 二重補正 0。Raw capture bytes は monitor
  状態・gate 種別に依らず一致。
- 高 Latency FX・Tempo 区間の組合せは fixture で扱える範囲を
  pin（monitor native placement が FX latency を二重補正
  しない・`LatencyReport` が reported latency を一度だけ
  算入・tempo 境界での span project mapping）。

## Examples

- Given `GateSpec::Punch`・`MonitorState::Auto`・Arm On・
  `TransportPhase::Playing`、When `MonitorEngine` が engine
  block を進める、Then gate open 区間は route Input、
  closed 区間は Playback の PCM が monitor bus に出て、
  block 途中の境界でも出力 frame 数が block 長と一致し
  Gap/重複 0。
- Given `GateSpec::Loop`・複数 pass、When loop pass 境界が
  block 途中に来る、Then 各 pass の gate open 区間が
  session-frame 精度で route に反映され、monitor 出力が
  pass をまたいで連続する。
- Given `GateSpec::PunchInLoop`、When punch 区間が loop
  pass 内にある、Then punch 区間のみ route Input、loop
  の残りは Playback、pass ごとに同じ project 区間が
  再 gate され Raw Placement 二重補正 0。
- Given route Input が選択されるが入力 PCM 未供給、When
  block が処理される、Then effective route は Muted、
  `input_missing` notice が出て PCM は無音（捏造しない）。
- Given `MonitorState::Off`/`On`/`Auto` × Arm On/Off ×
  Transport Playing/その他 × Punch gate × Mute/Solo の
  組合せ、When engine fixture を駆動する、Then route 別
  PCM・effective route・notice が期待値と一致し、raw
  capture bytes は monitor 状態に依らず一致。

## Guarantees

- `MonitorEngine` が `HeadlessDriver`/`RoutingEngine` の実
  run 経路に monitor bus を載せ、headless fixture で
  Off/Auto/On × Arm × Transport × Punch × Mute/Solo の
  組合せ結果が pin される。
- `GateSpec::Loop`/`PunchInLoop` の pass ごとの gate open
  区間が session-frame 精度で正しく route に反映され、
  block 途中境界で Gap/重複 0・Raw Placement 二重補正 0。
- 高 Latency FX・Tempo 区間の組合せが fixture で扱える範囲
  で pin される。
- `PreparedRecordingPlan` が構築 sample rate を保持し、
  engine sample rate との不一致が構築時エラーになる。

## Non-goals

- 実 Device/Capture/Software Monitor/GUI での実測・実 GUI
  受入行（#114 Scenario・別 Oracle）。
- RT graph への monitor node 搭載・transport control 配線・
  実 Prepared Route 構築。
- 実入力 device からの monitor PCM 供給。
- `synvelis-app` からの session 構築・状態投影接続。
- count-in UX・AudioLens 等の #117 残件。
- block 内の phase/arm/monitor 相変化（block 単位供給のみ）。

## Risk

- fixture の route 結果を実機 Pass と誤認する → test spec・
  PR 本文で fake/fixture 検証であることを明記。
- pump cadence emulation の誤りで producer pump タイミングが
  変わり fixture PCM が実経路と乖離 → `Every(1)`/`Every(0)`
  切替で `index % n == 0` と同一 cadence を再現する契約を
  test で pin。
- `MonitorAdapter` の非 RT `Vec` 確保を RT 経路へ誤用 →
  本 slice は RT callback 外のみと明記し、RT-safe 化は
  別 slice。
