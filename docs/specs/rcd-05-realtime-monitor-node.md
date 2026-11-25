---
id: "SPEC-034WTmmqJyfamQB05HQxCEJC"
type: "spec"
status: "proposed"
title: "RCD-05 RT monitor node — bounded primitive slice (synvelis#653)"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/653"
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
    - "docs/specs/rcd-03-monitor-engine.md"
    - "docs/specs/rcd-03-monitoring-punch-latency.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/rcd-f02-monitoring-overdub.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/653"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/ruviform/issues/629"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/ruviform/issues/629"
  deferred-by: []
---

# RCD-05 RT monitor node — bounded primitive slice (synvelis#653)

## What Should Be Built

`synvelis-recording::decide` が確定した monitor route を、RT callback
contract section 内で fail-closed に適用する typed-graph processor
primitive `MonitorRtNode` と、その bounded typed-graph fixture。本
slice は production graph への接続を行わず、primitive の port
contract・bounded schedule contract・PCM mixing・RT allocation
契約だけを固定する。

対象は次の 3 port を持つ processor である。

- input port 0: playback input
- input port 1: monitor input
- output port 0: monitor output

非 RT 側は block ごとの route segment schedule を固定容量 SPSC
channel へ供給する。RT node は route 判定を再実装せず、供給済み
schedule の検証と適用、playback/monitor input の混合、fault/status
公開だけを行う。

## Why Now

Issue #653 は monitor 経路を RT callback 内で成立させることを求め
るが、既存 `MonitorAdapter`/`MonitorSession` は非 RT 所有権構造と
Vec 出力を持ち、RT graph に直接載せられない。一方で route 判定の
権威は sv#117 で main 済みの `synvelis-recording::decide` にある。
RT node に第二の route 判定を作る前に、「判定済み route を安全に
適用する primitive」の契約を分離して固定する必要がある。

## Users and Value

- RT graph integration 実装者: production topology 決定前に、
  direct edge・schedule・fail-closed・status の契約を再利用できる。
- Issue #653 レビュア: headless typed graph で測れたことと、実
  device/実 OS の可聴 monitoring で未検証のことを区別できる。
- 後続 transport-control slice 実装者: `NodeEvent` に依存しない
  sample-accurate 境界の表現と、generation rebuild 時の既知リスク
  を引き継げる。

## Scope

### In Scope

- `crates/synvelis-player/src/monitor_rt.rs` の新規 primitive。
- playback input・monitor input・monitor output の port contract。
- 固定容量・`Copy` payload の非 RT→RT schedule channel。
- block 内半開区間 `[start, end)` が連続・非重複・空区間なしに
  frame 全体を tile する schedule contract。
- schedule 未供給・順序違反・gap/overlap・範囲外・容量超過・I/O
  shape 違反時の fail-closed mute と atomic fault/status。
- direct edge での monitor input 未供給と、供給された無音 PCM の
  区別。
- playback 安定区間の bit-exact passthrough と、typed graph 上の
  非 monitor path PCM 非干渉。
- headless typed-graph fixture、独立 oracle、独立 `rt_` allocation
  test binary。
- RT 安全性の根拠と未解決事項の設計記録。

### Out of Scope

- 実 device input・実 OS audio backend・WASAPI/cpal/実ハードウェア
  を通した monitoring、実機の可聴確認。
- `synvelis-app`・UI/GPUI・engine driver への接続、production graph
  への搭載。
- production topology（master path 挿入型か独立 auxiliary bus 型か）
  の決定。
- transport control 完全配線、`TransportPhase`/`GateScheduler`/
  `MonitorProbe` の RT 化、transport lookahead plan。
- per-strip pre-fader monitor、multi-strip routing、mute/solo/arm/
  monitor mode の RT node 内判定。
- fan-in/channel-map/PDC 経由の monitor input、latency/PDC 補正、
  リサンプル、native placement 変更。
- raw capture/offline export/non-monitor playback path の実装変更。
- 既存 `MonitorEngine`/`MonitorSession` の RT 化、非 RT 経路の置換。
- offline exact render support の `Verified` 宣言。

## Requirements

- route 判定の単一権威は `synvelis-recording::decide` に残す。RT
  node は arm/monitor mode/transport phase/gate/mute/solo を見ず、
  事前に確定した `MonitorRoute` segment を適用する。
- schedule payload は固定サイズ `Copy` とし、RT 側で所有権を持つ
  可変長オブジェクトを drop しない。
- `NodeEvent` は block 先頭配送で frame offset を持たないため、
  sample-accurate な Input/Playback/Muted 境界に使わない。
- monitor input edge は direct edge 契約とする。fan-in/channel-map/
  PDC で zero-filled scratch に materialize された input を未供給と
  区別できないまま無音へ偽装しない。
- monitor input PCM が route `Input` に必要なのに未供給なら、PCM を
  捏造せず effective mute へ降格し、`input_missing` status を残す。
- output は受け取った block frame 数分を必ず書き、未書き込み frame
  を残さない。node 上限を超える frame 数は契約違反として zero-filled
  output を書いて拒否する。
- gain ramp は DSP のみであり、latency/PDC/native placement/リサンプル/
  frame offset 補正を行わない。`ramp_frames` は exact conversion を
  保証できる `u16` 範囲に限定し、範囲外は construction error とする。
- monitor bus は raw capture/offline export/non-monitor playback path
  に接続しない。本 slice では production graph 未接続なので、この非
  干渉は structural に保たれ、fixture では typed graph 内の非 monitor
  sink PCM 不変だけを示す。

## Acceptance Criteria

- [ ] `MonitorRtNode` が `ruviform_audio::typed::GraphProcessor`
  として typed graph 内で駆動され、`RtRuntime::process` の RT
  callback contract section で schedule 適用と PCM mixing を行う。
- [ ] block 中途の Input/Playback/Muted 境界について、独立 oracle
  が計算した frame ごとの期待 PCM と monitor output が bit 単位で
  一致する。
- [ ] Playback 安定区間で `-0.0` を含む playback input が bit-exact
  に passthrough し、非 monitor sink の PCM が変わらない。
- [ ] monitor input 未供給と供給された無音 PCM を fixture で区別で
  きる。
- [ ] schedule contract violation と control overflow が fail-closed
  mute + atomic fault/status として観測できる。
- [ ] `mise run test-rt` で独立 test binary の RT allocation test が
  実行され、warm-up 後 hot path の allocation/deallocation が 0 であ
  る。
- [ ] 正規検証 lane が Pass し、NotRun/Unsupported は理由付きで記録
  される。
- [ ] 可聴 monitoring の製品受入が本 slice の Pass と混同されず、
  synvelis#114 系・ruviform#629 仮想 endpoint 経路など受入側 gate
  で追跡されることが明記されている。

## Evidence Classification

本 slice の Pass は headless typed-graph fixture と `stats_alloc`
計測に限る。実 device、実 OS backend、実 GUI、実機の可聴 monitoring、
virtual endpoint 経路は Pass に昇格しない。それらはそれぞれ
NotRun/Unsupported として evidence に記録する。

## Deferred Design Concerns

- production topology と app/engine 接続 — 後続 integration slice。
- transport lookahead と generation rebuild またぎの ramp state 保存 —
  後続 transport-control slice。現状、primitive を再構築すると gain
  state は 0 に戻り、可聴経路に接続した場合 click 音リスクがある。
- direct edge 契約の静的強制 — typed graph の processor 内からは
  scratch materialization の有無を introspect できない。fixture は
  direct edge を証明するが、production 配線での強制は integration
  slice の責務。
- offline exact render — `offline_support` は `Unknown` のまま。検証
  しない限り `Verified` に昇格しない。

## Sources and Relations

- Issue #653: RCD-05 RT graph monitor node 搭載・RT-safe monitor bus。
- sv#117 資産: `synvelis-recording::decide`/`MonitorProbe` は route
  判定と gate 観測の権威。
- `docs/specs/rcd-03-monitor-engine.md`: 非 RT monitor session の
  既存契約。
- synvelis#114: 実 OS GUI/製品受入 gate。
- ruviform#629: 仮想 endpoint EXT-L3 経路。
