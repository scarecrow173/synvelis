---
id: "DESIGN-034XTaBm7KJKf8COVxX4IFGk"
type: "design"
status: "proposed"
title: "RCD-06 Monitor production integration design"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/rcd-06-monitor-production-integration.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/rcd-06-monitor-production-integration.md"
  implemented-by: []
  depends-on:
    - "docs/designs/rcd-03-monitor-engine.md"
    - "docs/designs/rcd-05-realtime-monitor-node-design.md"
    - "docs/designs/transport-integration-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/rcd-03-monitor-session-ux.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies: []
  verified-by: []
  references: []
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/115"
    - "https://github.com/scarecrow173/ruviform/issues/629"
    - "https://github.com/scarecrow173/ruviform/issues/631"
  deferred-by: []
---

# RCD-06 Monitor production integration design

## Context

本 design は spec `rcd-06-monitor-production-integration.md` の実装境界を定める。対象基準は `origin/main` `b9c331fd` で、RCD-05 `MonitorRtNode` は merged 済み、production 配線は未実装である。本 slice 自身は docs-only であり、ここに書く component 名は後続実装が作る設計上の責務名であって、既存コードの実在を主張しない。

既存の重要な非対称は次の通りである。

- `decide` は mute/solo を `Playback` route にだけ作用させ、`Input` と raw capture を消さない。
- 現行 lowering は mute/solo 非可聴 strip からの edge を省略することで playback を抑制する。この構造を monitor-integrated strip にそのまま使うと、input monitor も消える。
- typed graph direct edge では producer slot がその callback に書かれなければ `NodeIo::input` が `None` になる。scratch materialization や常時 zero 出力 source では未供給と valid な無音を区別できない。
- InputAdapter の capture path は writer 向けで、Arm Off + Monitor On の input は callback target ですらない。
- app の production transport は `playing: bool` stub 相当で、`syn.monitoring.*` の位相投影は engine transport ではない。

## Decision D1: 採用 topology は strip post-fader replacement insert

### 比較

| topology | 個別 track 抑制 | mute/solo 干渉 | offline/raw 分離 | 判定 |
| --- | --- | --- | --- | --- |
| master output insert | mixed master から個別 track を除去できない。multi-track punch で punch 中 track の playback が残る | master 段では track mute/solo 意味論を再構成できない | output 1 箇所で制御しやすいが分離以前に機能不全 | 却下 |
| strip pre-fader inline insert | strip 単位では可能 | edge suppression を残すと input も消える。insert 後段の fader/send が input に作用し、意味論上の pre-fader monitor とずれる | inline realtime node を offline から除けば可能 | 却下 |
| dedicated monitor bus 並列混合 | 並列に足すだけでは元 playback を消せない。元 playback を別途 mute する制御が必要 | bus を独立させれば mute/solo edge 抑制を回避しやすい | 分離しやすいが mixer を二重化し、master/downstream processing との一致が困難 | 単独では却下 |
| strip post-fader replacement insert | node output が strip main contribution を置き換えるため可能 | track mute/solo を edge omission から schedule mute へ移せば input は残る | realtime-only generation に限定すれば分離可能 | 採用 |

採用形状:

```text
playback chain ─▶ strip fader ─▶ MonitorRtNode input 0 (playback)
live input boundary Source ────▶ MonitorRtNode input 1 (monitor, direct)
                                  MonitorRtNode output ─▶ 元の strip main downstream
```

`Input` route では strip post-fader playback が置き換わる。input は strip fader を通らない。`Playback` route では post-fader signal を通すため、strip fader/chain のレベル意味は残る。downstream bus/master は共通出力段として扱う。

node を挿す対象は `project.monitoring.tracks` に宣言を持つ strip（monitor-integrated strip）である。Monitor Off でも Arm On かつ gate open では `decide` が `Muted` を返すため、Monitor Auto/On の strip だけに挿すと RCD-03 の playback 抑制を実現できない。宣言追加・除去だけを graph generation 変更とし、宣言済み strip の arm/monitor 値変更・mute/solo 変更・gate/transport 変化は schedule で処理する。宣言を持たない strip は従来の playback path と edge suppression を使う。

### mute/solo の扱い

monitor-integrated strip では、track mute/solo を semantic lowering の edge omission に使わない。`decide` が返す `Muted` を schedule へ入れ、RT node が playback を消す。post-fader replacement output edge は muted/solo 外でも残す。Monitor Off の strip でもこの例外は必要である。Arm On + gate open の `Muted` を edge omission に戻すと、monitor input を残す境界と playback を消す境界が再び混ざる。宣言を持たない通常 strip だけ、従来通り edge suppression でよい。

この変更がない lowering では、muted/solo 外 strip の input monitor が消える。したがって implementation slice は compiled plan/source map で次を検証しなければならない。

- monitor node output edge が track mute/solo で欠落しない。
- monitor input edge が single direct edge・channel map なし・delay 0・scratch なし。
- monitor-integrated strip に playback bypass edge が残らない。

bus/master など downstream の mute/solo は track-level mute/solo と別責務である。初期実装で reason を区別できない場合は monitor topology を activate しない。誤って input を消したまま「monitor active」と表示しない。

### send の制限

send tap が monitor insert より上流から分岐する場合、main output を置き換えても playback は send 経路へ残る。現行の strip send 構造を monitor-aware に再設計するまでは、send を持つ strip または bypass し得る strip を monitor production ready にしない。黙って一部だけ抑制しない。send-aware monitoring は後続 slice とする。

## Decision D2: monitor input は caller-fed boundary で供給する

monitor input を内部 processor source node にすると、その node が process される限り output slot は current sequence で marked written になり、zero を出しても `MonitorRtNode` からは valid PCM と区別できない。そこで strip ごとに graph-input boundary Source を用意し、direct edge で input 1 へ接続する。

realtime output callback は block 処理前に次を行う。

1. monitor ring から block 全 frame 分の valid PCM を読めるか確認する。
2. 読める場合だけ `input_writer(port, frames)` で boundary slot を書く。
3. 読めない場合は boundary slot を書かない。

valid なデジタル無音は PCM として書く。underflow/overflow 後の欠損、format/clock 不一致、stale generation は書かない。部分 frame だけ書いて残りを zero fill しない。block 単位で missing にするのは、typed graph の write stamp が block 単位だからである。

## Decision D3: InputAdapter は tee boundary、ring owner は monitor integration

### 所有権

| 要素 | owner | RT での扱い |
| --- | --- | --- |
| writer PCM pool/work queue/capture handoff | InputAdapter/RecorderOwner の capture path | 現行契約通り。monitor と共有しない |
| monitor ring memory | production monitor coordinator が session 構築時に固定確保 | producer/consumer endpoint だけを RT 側へ移す |
| monitor ring producer endpoint | input callback 側 tap state | non-blocking write。空き不足なら drop + fault |
| monitor ring consumer endpoint | output callback 側 bridge | non-blocking read。不足なら boundary unwritten |
| graph boundary slot | typed graph runtime | output callback が valid block だけ書く |
| schedule channel/producer | monitor schedule producer | RCD-05 contract 通り固定容量 SPSC |

InputAdapter は stream callback を所有するため tee の呼び出し点になるが、monitor ring の memory・consumer・graph node を所有しない。writer path の `Box` PCM slot を monitor と共有しない。共有すると writer pump と output callback の返却寿命が競合し、RT safety と ownership が崩れるため、monitor 用は別 planar ring へ bounded copy する。

### format と clock

ring は engine format の planar f32 で固定確保する。input channel map から engine channel layout への写像は producer 側の bounded copy で行い、graph edge の channel map にはしない。各 frame には input clock domain の連続 frame sequence/timestamp を持たせ、consumer は連続性が確認できる frame だけを valid にする。output callback 側は bounded priming latency の後、1 output block につき 1 block 分だけを順序通り consume する。input callback block サイズと output block サイズが違ってよいが、frame 列に gap があれば連結しない。sample rate が違う、clock domain を比較できない、同一 clock 性を証明できない構成は supported にしない。初期実装は cross-clock resampling/drift compensation を行わない。

raw capture の data 保全を monitor より優先する。monitor ring が full でも writer queue を block せず、writer pool を借りない。monitor overflow/underflow は monitor tap fault として latch し、raw capture fault と別 counter で app へ返す。input stream 全体の停止や device loss は writer/monitor 両方の入力欠損として扱う。

## Decision D4: transport authority と schedule producer

### boolean stub で表現できないもの

`playing: bool` だけでは少なくとも次を表現できない。

- CountIn/PreRoll/PunchWait/PunchOpen/PostRoll/Playing の区別。
- pause と stop の位置保持/position zero の差。
- seek・loop wrap・command が block 内のどの frame で適用されたか。
- transport generation と AppliedReceipt による stale command 排除。
- recording session frame と engine frame の半開区間対応。

production supplier は transport integration の Engine Frame/transport generation/AppliedReceipt を正本とする。position は engine frame で供給し、project musical time からの変換には凍結済み tempo map/sample rate/transport start を使う。CountIn/PreRoll は record 開始前に位置し得るため、session frame へ写像する前の符号付き media/engine-relative position を保持し、負位置を 0 へ clamp しない。gate は recording session 外では閉じたまま評価する。schedule 自体は RT node が要求する block 相対 `u32` frame で、半開 `[start,end)` に tile する。

pause は RCD-03 の `TransportPhase` 語彙に独立 variant として存在しない。本 design は `decide` 入力を勝手に拡張せず、pause 中は再生していない停止位相として `Stopped` に写像する。ただし transport 実状態の paused/stopped 区別は receipt/UX projection 用に別途保持し、boolean へ潰さない。将来 `decide` 語彙を拡張する場合は RCD-03 側の意味論変更として扱う。

### producer の責務

新設する `MonitorScheduleProducer`（仮称）は非 RT で、monitor-integrated strip ごとに次を行う。

1. monitor/arm/mute/solo/input readiness/hardware direct の context generation を読む。
2. production transport から engine frame window、transport phase、generation、receipt 済み discontinuity を得る。
3. gate 開閉を `GateScheduler`/`evaluate_gate` または `MonitorProbe` と同じ権威から得る。
4. 各 segment で `decide` を呼び、block 相対 route segment を作る。
5. sequence 順に `MonitorRtControl` へ供給し、status/fault を drain する。

block 内の phase/gate/context 変化では segment を分割する。`MONITOR_RT_SCHEDULE_CAPACITY=8` を超える場合は近似せず、その block を全 `Muted` の valid schedule にし、producer 側 integration fault を記録する。transport position/generation/tempo mapping が unknown/stale なら最後の route を延長せず `Muted` にする。schedule を出せなければ node 側の missing schedule fail-closed mute が最後の防御となる。

### phase 導出

CountIn/PreRoll は session start 時に凍結された宣言区間と現在位置から導く。PunchWait/PunchOpen/PostRoll は plan 凍結済み punch マークと現在位置から導き、実際の gate 開閉は scheduler の `gate_open` に委ねる。`decide` 上は Stopped 以外の多くが同じ扱いでも、phase を正直に供給する。これにより UX projection、将来の punch 意味論拡張、fault 診断が boolean 再生状態へ退化しない。`syn.monitoring.*` の既存位相導出は参考実装だが、production authority ではない。共通化する場合は pure helper を抽出し、UX 側に第二正本を残さない。

## Decision D5: app consumer と receipt

production monitor session は app composition 内の単一 coordinator が所有する。RecorderOwner は input device/tap の open/preflight を担い、routing engine composition は graph generation を担い、transport owner は engine transport を担う。coordinator はこれらを結び、schedule producer・monitor ring・status drain の寿命を管理する。

`syn.monitoring.*` の責務は次に限定する。

- authored arm/monitor/channel map/punch 宣言の write-through。
- session UX 操作の入口。
- production coordinator から返る receipt/status を含む読み取り投影。

session active でも engine-effective でなければ成功済み monitor と表示しない。start receipt は少なくとも `pending`、`active`、`failed`/`not_ready` を区別し、graph generation publish・input endpoint bind・schedule producer priming のどこで止まったかを返す。state projection は declared route/UX predicted phase と engine-effective route/input missing/fault を別 field にする。RCD-03 の Hardware Direct unknown/double monitoring notice は `decide` の結果をそのまま使い、app が再解釈しない。

## Decision D6: Realtime/Offline/Export/Raw separation

monitor integration は realtime control-room build intent にだけ存在する。Offline/Export build では monitor node、monitor input boundary、monitor ring、schedule producer を lowering しない。`MonitorRtNode` の `offline_support` は Unknown なので、offline graph に接続して無害化する設計は採らない。そもそも搭載しない。

Raw capture は input callback から writer capture engine への既存経路だけを使う。monitor tee は writer PCM を読み戻さず、writer file bytes へ書き戻さない。Realtime output device/control-room PCM は export oracle の入力にしない。Export bytes は offline render pipeline から作る。

非汚染の独立 oracle は後続 test-spec の通りとする。monitor 実装の出力を再生して一致を見るのではなく、input fixture と project media/render recipe から raw/offline/export の期待 PCM/bytes を独立計算する。

## Alternatives Considered

- **master insert を採用**: 却下。mixed master から個別 track を除去できず、multi-track punch の基本要件を満たせない。
- **strip pre-fader inline insert を採用**: 却下。insert 後段の fader/send が input に作用し、edge suppression と合わせて意味論上の pre-fader monitor を壊す。post-fader replacement の方が playback/input の分岐点を明示できる。
- **dedicated monitor bus だけを採用**: 却下。monitor bus へ足すだけでは元 playback contribution を消せない。消すために結局 strip 単位 replacement が必要になる。
- **writer PCM slot を monitor と共有**: 却下。writer pump と output callback の返却・再利用寿命が競合し、raw capture の backpressure を monitor に伝える。
- **常時 zero source + sideband validity**: 却下。RCD-05 の input missing contract を活用できず、validity の第二権威を増やす。
- **offline にも monitor node を残し無害化**: 却下。`offline_support` Unknown の primitive を output bytes へ関係する graph へ入れない。

## Implementation Slice Decomposition

| slice | 範囲 | 依存 | 主な検証 |
| --- | --- | --- | --- |
| RCD-06A topology/lowering | monitor-aware strip post-fader replacement、direct boundary edge、mute/solo edge exception、send unsupported 判定 | RCD-05 | compiled plan/source map、headless typed graph、bit-exact non-monitor path |
| RCD-06B input tap | InputAdapter tee、monitor ring、boundary feeder、clock/format preflight、fault 分離 | RCD-06A | fake input callback、ring overflow/underflow、raw writer 非干渉、RT allocation |
| RCD-06C transport/schedule | production transport authority、phase supplier、schedule producer、stale/unknown fail-closed、dynamic context | RCD-06A | CountIn/PreRoll/Punch/seek/loop/in-block boundary の schedule oracle |
| RCD-06D app integration | coordinator、`syn.monitoring.*`/recorder receipt/projection、status drain、generation quiesce | RCD-06A〜C | service-level integration、receipt、fault 投影、session lifecycle |
| RCD-06E separation hardening | offline/export/raw non-contamination oracle、send-aware 拡張、generation rebuild gain 継続 | RCD-06A〜D | independent oracle、export/raw bytes hash、send routing 検証 |
| RCD-06X acceptance | Windows GUI-L2/EXT-L3、実/仮想 endpoint、native product E2E | RCD-06A〜E + ruviform 側 endpoint | GUI-L2/EXT-L3 evidence |

Windows GUI-L2/EXT-L3 の product acceptance owner は synvelis#114 系とする。ruviform#629/#631 は virtual endpoint/backend 側依存、synvelis#115（RCD-01 実 Audio Device 録音統合）は実 input device/同一 clock capture 側依存として扱う。#114 系が受入基盤だけを担い monitor 固有受入を持たないと判明した場合は、RCD-06X 用の専用受入 Issue を coordinator が起票する。

## Remaining/Deferred Work

- send-aware monitoring の構造設計。
- cross-clock/drift compensation/resampling。
- rebuild 時 gain state carryover と click 防止。
- downstream bus/master processing を含む playback monitor の完全一致。
- 実 OS backend、実 device、実 GUI、実 plugin の受入実行。
- docs index 更新は本 docs-only slice では行わず、統合時に coordinator が整備する。
