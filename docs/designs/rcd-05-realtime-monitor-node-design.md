---
id: "DESIGN-034W9EFZ0BX0Ww5mQEPT0vvF"
type: "design"
status: "approved"
title: "RCD-05 RT monitor node design"
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
  implements:
    - "docs/specs/rcd-05-realtime-monitor-node.md"
  implemented-by: []
  depends-on:
    - "docs/designs/rcd-03-monitor-engine.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-f02-monitoring-overdub-design.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/ruviform/issues/629"
  deferred-by: []
---

# RCD-05 RT monitor node design

## Context

既存 `MonitorAdapter` は非 RT の `MonitorBlock` を受け取り、
`Vec<Vec<f32>>` output を返す。これは test/debug には便利だが、RT
hot path で allocation する所有権構造であり typed graph processor
へそのまま載せられない。`MonitorSession`/`MonitorEngine` も非 RT
session 制御を持つ。一方、route 判定は `synvelis-recording::decide`
が権威であり、RT node で transport/gate/arm/monitor/mute/solo を再
判定すると第二実装になる。

本 design は responsibility を次のように分ける。

- 非 RT 側: route 判定済みの block schedule を作り、固定容量
  channel へ供給する。
- RT node: schedule を再検証し、gain ramp 状態を進め、PCM を混合し、
  fault/status を公開する。
- ruviform typed graph: direct edge・slot・processor activation・RT
  contract section を提供する。

## Decision: primitive-only slice

production topology は決めない。`MonitorRtNode` は master path 挿入型
にも独立 auxiliary bus 型にも載せられる 2 input/1 output primitive
に留める。本 slice では app/engine driver、raw capture、offline export
へ接続しない。したがって raw capture/offline export を変更するコード
路径は存在せず、非干渉は structural に保たれる。fixture 内では
playback source を non-monitor sink にも direct 接続し、monitor node
が非 monitor path PCM を変えないことを示す。

## Port and PCM Contract

- input 0: playback input（`AudioFormat` exact、f32）
- input 1: monitor input（同 format）
- output 0: monitor output（同 format）
- PCM は ruviform typed graph の channel plane view。channel 数は
  `MonitorRtNodeConfig.format.layout`、sample format は `F32`、block
  上限は `max_block_frames` で固定する。
- processor は output frame 数分を必ず書く。`frames == 0` では書く
  frame が存在しない。`frames > max_block_frames` や output shape
  違反では fail-closed に zero-filled output を書き、
  `MONITOR_RT_FAULT_IO_CONTRACT` を立てる。
- `frames > max_block_frames` は graph compile 契約違反であり、正常
  経路では発生しない。zero fill は未書き込み frame を残さないための
  防御であり、partial valid rendering ではない。

monitor input edge は direct edge 契約である。fan-in、channel map、
PDC delay、複数 incoming edge にしない。typed graph runtime は direct
edge では未供給 input を `None` として観測できるが、scratch
materialization されると zero-filled input と未供給を processor 内で
区別できない。processor 自体は配線を introspect できないため、この
契約は integration 側の責務である。fixture は compiled plan で
`scratch.is_none()`・single edge・channel map なし・delay 0 を断言し、
表現可能な最小 direct-edge topology を固定する。

## Schedule Contract

非 RT→RT channel は ruviform-rt の bounded SPSC channel を使う。
payload は `Copy` な `MonitorRtSchedule` で、固定容量 8 segment の
配列を持つ。所有権を持つ可変長 schedule object を RT 側で drop する
設計は採用しない。payload は untrusted boundary を越えるため、
producer 側 validation に加えて RT 受信側でも毎回再検証する。

1 block の schedule は次を満たす。

- `sequence` は 0 始まりで block ごとに 1 増える。
- `frames` は実 callback frame 数と一致し、`max_block_frames` 以下。
- segment は半開区間 `[start_frame, end_frame)`。
- 先頭 segment は frame 0 から始まり、後続 segment は直前 end から
  連続する。
- 空区間、gap、overlap、block 範囲外を持たない。
- 最終 end は `frames` と一致する。
- 有効 segment 数は 1..=8。

違反時は PCM を捏造せず、その block を mute 方向へ ramp し、fault
bit/counter を残す。control channel full は producer へ `ChannelFull`
を返し、overflow counter を進める。RT node は overflow counter の変化
を観測した block では、たとえ valid schedule が読めても適用せず
mute する。これは欠損の可能性を楽観補正しない fail-closed policy
である。

sequence 欠損後は、観測した future sequence の次へ進める。これは
欠損 block を再構成するためではなく、producer が連続供給を再開した
ときに永久に sequence fault へ張り付かないための回復規則である。
欠損 block 自体は常に mute 済みであり、遅れて届いた過去 schedule
をさかのぼって適用しない。

## Route Application and Gain Ramp

`MonitorRtSegment.route` は非 RT 側で確定済みの値であり、RT node は
route を再判定しない。`Input` かつ monitor input 未供給だけは PCM
を捏造できないため、input gain target を 0 に下げる effective mute
へ降格し、`input_missing` status を記録する。供給された無音 PCM は
input edge が存在して 0.0 sample が供給された場合だけであり、
`input_missing` を進めない。

gain ramp は既存 `MonitorAdapter` と同じ線形近似規則（frame ごとに
`1/ramp_frames` ずつ target へ近づけ、`ramp_frames == 0` で即時切替）
を使う。ただし共有 helper 抽出は行わなかった。理由は、既存
`MonitorAdapter` が常に input/playback 両項を加算し、playback-only
安定区間でも `-0.0 + 0.0` を `+0.0` へ正規化し得るのに対し、RT
node は playback passthrough の bit-exact 保持を要求されるからだ。
RT node は片側だけが可聴な安定区間で加算を挟まず sample を返し、
gain の 1.0/0.0 を bit 判定で特別扱いする。この差を両経路へ無理に
共通化すると非 RT API の PCM 意味を変えるため、本 slice では
fixed-scratch DSP を RT node 内へ置く。後続で非 RT 側の bit 契約を
見直す場合に統合候補とする。

ramp は DSP のみである。native placement、latency/PDC 補正、リサンプ
ル、frame offset 補正は行わない。`ramp_frames` は `u16` に exact
conversion できる範囲に制限し、浮動小数 cast の曖昧さを constructor
契約で排除する。

## Why `NodeEvent` Is Not the Authority

ruviform typed graph の `NodeEvent` は block 先頭に配送され、frame
offset を持たない。Input/Playback/Muted の sample-accurate 境界を
`NodeEvent` で表すと、block 中途の遷移を表現できない。したがって本
slice の正本は frame range を持つ schedule segment とし、`NodeEvent`
は使わない。将来 block 先頭でよい補助制御を追加しても、schedule
適用の正本とは別物として扱う。

## Status Atomics

`MonitorRtStatus` は fault bit と counter を `AtomicU64` で公開する。
RT 側更新と非 RT 側読み取りの両方に `Relaxed` を使う。理由は、各
counter/bit が独立した診断値であり、atomic 間に同期対象データや
composite snapshot invariant がないためである。schedule payload 自体
の happens-before は ruviform-rt SPSC channel が提供し、status atomic
を payload 公開の順序付けに使わない。したがって stronger ordering
は不要で、RT hot path の負荷だけ増やす。複数 counter の同時 snapshot
が必要になった場合は、非 RT 側の観測 API を別途設計する。

公開する主な観測値は fault bit（schedule missing/sequence/malformed/
I/O contract/control overflow）、status bit（monitor input missing/
playback input missing）、counter（missing block・contract violation・
overflow・missing input・applied block/segment/output frame）、
`last_applied_sequence` である。

## RT Safety Rationale

`MonitorRtNode::process` は次の性質を持つ。

- allocation-free: schedule は `Copy` 固定配列、gain scratch は
  constructor で固定確保、hot path で Vec の長さ変更・clone・box化を
  行わない。
- deallocation-free: hot path で所有権を持つ schedule object や可変長
  buffer を drop しない。
- no blocking lock: Mutex/RwLock を持たず、SPSC channel と atomic
  counter だけを使う。
- no OS I/O: file/socket/device/print/logging を行わない。
- bounded work: frame 数は `max_block_frames`、channel 数は構築時
  format、segment 数は 8 で上限付き。segment scan は block 内で
  amortized bounded。
- bounded channel: control capacity は constructor 固定で、overflow
  は recover せず fail-closed mute と status にする。
- no unbounded retry: channel empty/full でも待機しない。

`stats_alloc` による実測では、graph compile・input staging・activation
と warm-up を測定外にし、schedule consume・mid-block boundary 適用・
PCM mixing・output write を含む `RtRuntime::process` hot path の
allocation/deallocation が 0 であることを確認する。空処理だけの
allocation 0 ではない。

## Drop Discipline

`MonitorRtNode` は Vec scratch と channel consumer、共有 status Arc
を持つが、custom Drop は持たない。ruviform typed runtime は置き換え
られた processor を RT 側で即 drop せず、`Retirer<Box<dyn
GraphProcessor>>` 経由で非 RT owner の `DropCollector` へ退避する。
したがって Vec/Arc の deallocation は RT hot path で実行されない。
`RtRuntime` 自体の drop は bounded retire work であり、processor
destructor は non-RT side で行う契約である。本 slice はこの ruviform
契約に依存し、node 側で追加の deferred-drop 機構を複製しない。

## Generation Rebuild Risk

primitive は gain state を内部に持つ。production integration で
processor を generation rebuild ごとに新規構築すると gain は 0 に
reset され、可聴経路では click 音になり得る。本 slice は production
接続を行わないため、このリスクは発現しない。解消には transport
control 完全配線、active state の非 RT 移譲、または generation またぎ
processor reuse が必要であり、後続 transport-control/integration
slice に残す。

## Offline Render Support

`GraphProcessor::offline_support` は override しないため `Unknown`
のままである。sequential offline render で同一 PCM になることを検証
していないので `Verified` と宣言しない。monitor bus は offline export
path に接続しないため、本 slice で offline support を要求しない。

## Alternatives Considered

- `MonitorAdapter` を typed graph processor 化する — Vec output と非
  RT ownership を hot path に持ち込むため却下。
- route 判定を RT node 内へ移す — `decide` の第二実装になり、
  transport/gate/mute/solo の権威を分散させるため却下。
- `NodeEvent` で boundary を送る — frame offset がなく sample-accurate
  境界を表せないため却下。
- fan-in/channel-map/PDC も許す — 未供給と zero-filled PCM を区別
  できず、input missing を無音に偽装するため本 slice では却下。
- schedule を `Vec` payload で送る — RT 側 drop で deallocation が
  発生し得るため却下。

## Remaining Work

- production topology 決定と app/engine graph 接続。
- transport lookahead plan と `decide`/`MonitorProbe` の非 RT schedule
  producer 接続。
- generation rebuild をまたぐ click 防止。
- per-strip pre-fader monitor/multi-strip routing。
- 実 device/実 OS backend/仮想 endpoint/実 GUI の製品受入 gate。
