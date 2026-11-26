---
id: "SPEC-034XWWkYoAVQp4ywPhneogME"
type: "spec"
status: "proposed"
title: "RCD-06 Monitor production integration — graph topology・input tap・transport control (synvelis#665)"
created: "2026-11-26"
updated: "2026-11-26"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/665"
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
    - "docs/specs/rcd-05-realtime-monitor-node.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/transport-integration.md"
    - "docs/specs/media-render.md"
  refines: []
  refined-by: []
  derives-from:
    - "https://github.com/scarecrow173/synvelis/issues/665"
  derived-by: []
  verifies: []
  verified-by: []
  references:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/115"
    - "https://github.com/scarecrow173/ruviform/issues/629"
    - "https://github.com/scarecrow173/ruviform/issues/631"
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/115"
    - "https://github.com/scarecrow173/ruviform/issues/629"
    - "https://github.com/scarecrow173/ruviform/issues/631"
  deferred-by: []
---

# RCD-06 Monitor production integration — graph topology・input tap・transport control (synvelis#665)

## What Should Be Built

RCD-05 の `MonitorRtNode` primitive を production の realtime audio graph、live input、transport、app session へ接続するための統合契約を定める。本 slice は docs-only であり、コードを実装しない。後続実装 slice が実装すべき対象は次の 5 系統である。

1. monitor-integrated strip ごとの realtime graph topology。
2. live input PCM を writer path と競合させずに monitor input へ供給する tap。
3. CountIn/PreRoll/PunchWait/PunchOpen/PostRoll/Playing/Stopped を engine frame 精度で route schedule へ反映する transport supply。
4. `syn.monitoring.*`・`syn.recorder.*` と production engine 実状態の projection/receipt/fault 契約。
5. Realtime control-room graph・Offline render・Export・Raw capture の分離と非汚染検証。

route 判定の意味論権威は RCD-03 の `synvelis-recording::decide`、gate 開閉の権威は `GateScheduler`/`evaluate_gate` と `MonitorProbe`、RT 適用 primitive は RCD-05 の `MonitorRtNode` に残す。RCD-06 はこれらを再定義せず、production 配線・所有権・供給・証拠化の契約を追加する。

## Why Now

RCD-03 の `MonitorSession`/`MonitorEngine` は非 RT の観測・制御経路であり、実 input device・app transport・RT graph に接続されていない。RCD-05 の `MonitorRtNode` は RT-safe な 2 input/1 output primitive だが、production topology、input tap、transport schedule の供給元、app consumer 接続は未接続のままである。このまま個別に実装すると、master 出力への insert では個別 track を抑制できず、strip insert では mute/solo の edge 抑制が input monitor まで消し、boolean transport では CountIn/PreRoll/PunchWait を表現できない。統合境界を先に固定しないと、Realtime monitor PCM が Raw capture・Offline render・Export bytes へ混入する構造を後から局所 workaround で塞ぐことになる。

## Users and Value

- 後続実装者: graph 形状、input tap ownership、transport supply、app projection、非汚染 oracle を Issue や探索履歴を読まずに実装できる。
- Reviewer: 採用 topology と却下 topology、headless 証拠と実 device/実 GUI 受入の境界を判定できる。
- synvelis#114 系受入 Owner: Windows GUI-L2/EXT-L3 で何を product acceptance として証明すべきかを引き継げる。

## Scope

### In Scope

- master-output insert・strip pre-fader inline insert・dedicated monitor bus・strip post-fader replacement insert の比較と採用 topology。
- multi-track punch/monitor での個別 track 抑制、mute/solo と monitor input の干渉回避、send を持つ strip の初期制限。
- InputAdapter capture path と monitor tap の ownership、buffer 寿命、backpressure、RT safety、fault 伝播。
- monitor input の「valid な無音 PCM」と「未供給」の区別を保つ供給方式。
- transport phase・gate・mute/solo/context generation から block 相対 route schedule を作る非 RT producer の契約。
- boolean `playing` stub では表現できない位相、block 内遷移、seek/loop/停止/矛盾時の fail-closed 挙動。
- app session UX、production engine 実状態、receipt/fault 表示の責務分界。
- Realtime/Offline/Export/Raw の graph/build/intent 分離と独立 oracle による非汚染検証。
- 後続実装 slice 分解と Windows GUI-L2/EXT-L3 受入 owner。

### Out of Scope

- `crates/`・CI・依存・docs index の変更。本 slice は文書のみ。
- 実 device・実 OS audio backend・実 GUI・実 plugin を使う受入実行。
- `decide`/`MonitorProbe`/`GateScheduler`/`MonitorRtNode` の意味論再定義。
- Hardware Direct Monitor の制御。外部事実の観測と notice に留める。
- Offline render/export で software input monitoring を再現すること。monitor は live realtime 機能であり、offline 側へ接続しない。
- send を持つ strip の完全な production 対応。初期実装では fail-closed unsupported とし、後続 slice で設計する。

## Existing Facts

- `MonitorRtNode` は input 0=playback、input 1=monitor、output 0=monitor output の typed-graph processor。route 判定済みの block 相対半開 `[start,end)` segment が block 全体を連続に tile する schedule を固定容量 SPSC で受け、未供給・順序違反・gap/overlap 等では full-block mute に fail-closed する。
- monitor input は direct edge が前提である。ruviform typed graph の `NodeIo::input` は direct producer slot がその callback で書かれていなければ `None` を返すが、fan-in/channel map/PDC の scratch があると zero-filled PCM と未供給を区別できない。
- ruviform typed graph の graph-input boundary slot は caller が `input_writer` でその callback に書いたときだけ current sequence で marked written になる。書かなければ direct edge 先は stale と判定され、`None` になる。したがって valid/invalid を block 単位で表現するには、常に zero を出す内部 source node ではなく caller-fed graph-input boundary を使う必要がある。
- `decide` の意味論では、mute/solo は `Playback` route の可聴性だけに作用し、`Input` route と raw capture を消さない。Monitor Auto は arm かつ gate open または transport stopped で `Input`、Monitor On は `input_output_ready` なら `Input`、未 ready なら playback fallback と `input_unavailable`。
- 現行 semantic lowering は mute/solo で非可聴と判定した strip からの edge を graph に出さないことで playback を抑制する。monitor-integrated strip に同じ抑制を適用すると、input monitor の経路も structural に消える。
- InputAdapter は Arm On tap について writer 用の capture engine/handoff/bounded work queue/PCM pool を構築する。Arm Off tap は Monitor On でも capture resource や callback target を持たない。RT data callback は planar input を interleave して writer queue へ渡し、queue full は fault にする。
- app の `syn.transport.set` は `playing: bool` を session へ設定する stub である。`syn.monitoring.*` は count-in/pre-roll/punch 宣言から `TransportPhase` を投影する仕組みを持つが、session UX 独自の transport であり production engine の実 transport ではない。
- Offline render/export は `BuildMode::Offline`・`lower_semantic_offline`・export pipeline 側で構成され、realtime device graph とは別 build である。`MonitorRtNode` の `offline_support` は RCD-05 時点で Unknown。

## Requirements

### RCD06-R-01 Production topology

後続実装は、`project.monitoring.tracks` に宣言を持つ strip（本 spec では monitor-integrated strip）ごとに `MonitorRtNode` を strip post-fader main output の replacement insert として realtime graph に配置しなければならない。Monitor Off でも Arm On と gate open の組合せでは `Muted` が必要なため、Monitor Auto/On の strip だけを対象にしてはならない。宣言の追加・除去による node の追加・除去は graph generation 変更として扱う。node output は、その strip の post-fader main signal が本来向かう downstream path へ唯一の経路として接続する。master output だけに insert する構成は個別 track を mixed playback から除外できないため production topology として採用しない。

この topology を「strip post-fader replacement insert」と呼ぶ。playback input は strip post-fader signal、monitor input は live input boundary、output は downstream path である。strip chain/fader を通った playback は `Playback` route でレベル意味を保ち、`Input` route では strip main output の playback contribution が置き換わる。input PCM は strip fader を通らない。downstream bus/master の出力段処理は realtime output 構成上の共通段として扱う。

### RCD06-R-02 Mute/Solo と edge suppression

monitor-integrated strip では、track mute/solo による可聴性制御を graph edge omission に委ねてはならない。`Playback` route の mute/solo は `decide` の結果として schedule へ反映し、monitor node 自体が mute する。post-fader replacement output edge は monitor-integrated strip が muted または solo 外でも realtime graph に残さなければならない。これを実装できない graph generation では production monitor を active と表示してはならない。

bus/master など downstream strip の mute/solo が出力段全体を消す挙動は track-level mute/solo と区別する。初期実装でその区別を lowering が表現できない場合は、誤って input を消す topology を activate せず fail-closed に monitor unavailable とする。

### RCD06-R-03 Send を持つ strip の初期制限

send tap が monitor insert より上流から分岐すると、main output を置き換えても playback が send 経路へ漏れる。初期 production integration は send を持つ strip、または playback が monitor insert を bypass し得る strip を monitor production ready とみなさず、その strip の software monitor を fail-closed に unavailable としなければならない。send-aware な完全対応は後続 slice の未解決拡張として追跡する。

### RCD06-R-04 Monitor input boundary と direct edge

monitor input は strip ごとの caller-fed graph-input boundary Source から `MonitorRtNode` input 1 へ single direct edge・channel map なし・delay 0 で接続しなければならない。fan-in、channel map、PDC scratch、常時 zero 出力の内部 source node を使ってはならない。output callback は valid な monitor PCM が block 全 frame 分あるときだけ boundary slot を書く。valid な無音 PCM は書く。欠損・format 不一致・clock 不整合・underflow では書かず、`MonitorRtNode` に input missing として観測させる。

### RCD06-R-05 Input tap ownership

InputAdapter の writer path と monitor tap は PCM buffer・queue・fault ownership を共有しない。writer 用 capture engine/handoff/work queue/PCM pool は raw capture 専用に保持する。monitor tap は別 owner が事前確保した bounded planar ring と producer endpoint を使い、InputAdapter は input callback 内で tee の producer を呼ぶだけとする。Arm Off + Monitor On の tap でも monitor endpoint を構築できること。Arm On でないからといって raw capture resource を作ってはならない。

### RCD06-R-06 Backpressure と RT safety

monitor ring は fixed capacity、single producer/single consumer、構築後 allocation/deallocation なし、RT callback 内で non-blocking でなければならない。producer 側に空きがなければ unconsumed frame を上書きせず、monitor 用 frame を drop して overflow fault を latch する。consumer 側に block 全 frame 分なければ boundary slot を書かず underflow/input missing にする。overflow/underflow 後は stale frame と新規 frame を黙って連結せず、次の連続性が確認できるまで input missing を継続する。latch した fault は app へ報告するまで消えないが、連続性が回復した後の新規 valid PCM を永久に拒否してはならない。monitor overflow/underflow は raw capture fault と独立に記録し、writer queue を block・拡張・drop させてはならない。raw capture の data 保全を monitor より優先する。

### RCD06-R-07 Clock/format readiness

monitor tap と realtime output graph が同一 sample rate・engine channel format・比較可能な clock domain にあることを preflight で検証する。検証できない場合は `input_output_ready=false` または monitor endpoint unbound として扱い、clock drift を黙って吸収しない。初期実装は同一 clock domain が確認できる構成だけを supported とし、跨 clock/domain resampling/drift compensation は Unsupported とする。

### RCD06-R-08 Transport phase supply

production transport supplier は、engine frame・sample-accurate position・transport generation・AppliedReceipt を持つ実 transport を正本とし、`TransportPhase::{Stopped,CountIn,PreRoll,PunchWait,PunchOpen,PostRoll,Playing}` を供給しなければならない。Issue 表記の Playback は `TransportPhase::Playing`、Stop は `TransportPhase::Stopped` に対応する。`playing: bool` は CountIn/PreRoll/PunchWait/PostRoll/Playing を区別できず、停止と pause の扱い、巻き戻し・loop wrap・block 内遷移・command 適用 frame を表現できないため正本にしない。pause は RCD-03 の `decide` 語彙では `Stopped` に写像し、paused/stopped の区別は route 判定入力ではなく transport 実状態として receipt/UX projection に保持する。

### RCD06-R-09 Route schedule producer

非 RT schedule producer は、現在の monitor/arm/mute/solo/input readiness/hardware direct context、gate schedule、transport phase、engine frame 位置から `decide` を用いて route を決め、block 相対 segment へ tile して `MonitorRtControl` へ供給する。gate 開閉は `GateScheduler`/`evaluate_gate` または `MonitorProbe` と同じ権威から得る。transport/gate/context が block 内で変化する場合は segment を分割する。`MONITOR_RT_SCHEDULE_CAPACITY` を超える変化を近似で潰さず、unrepresentable としてその block を muted に fail-closed し、integration fault として記録する。

### RCD06-R-10 Unknown/矛盾時の fail-closed

transport position、generation、gate、tempo mapping、input readiness が未知・矛盾・stale の場合、producer は最後の正しい route を延長せず、影響 block を `Muted` にする。schedule を供給できない場合は RCD-05 の node-side fail-closed mute に任せる。いずれも fault/status を非 RT 側へ返し、app projection が healthy な monitor と誤表示しないようにする。

### RCD06-R-11 App consumer connection

production monitor session の owner は app composition 内の単一 coordinator とし、RecorderOwner が開く input tap、transport authority、routing engine graph generation、monitor schedule producer、status drain を仲介する。`syn.monitoring.*` は authored declaration と UX projection の入口であって、RT engine の実効 route/fault の正本ではない。production session が attach された後は、engine-effective route/input missing/fault/activation receipt を投影に反映し、projection-only の値と実効値を区別できなければならない。

### RCD06-R-12 Receipt と session lifecycle

monitor session start は graph generation publish、input endpoint binding、schedule producer priming の結果を receipt として返す。即時に確定しない場合は Pending/NotReady を返し、成功と偽らない。start 前検証に失敗した場合は project/recorder/graph を partial mutate せず fail-closed にする。stop/session end/graph rebuild では新しい monitor input と schedule を quiesce し、旧 generation の PCM・schedule を新 generation へ配送しない。

### RCD06-R-13 Realtime/Offline/Export/Raw separation

monitor node、monitor input boundary、monitor ring、schedule producer は Realtime control-room intent の graph/build にだけ存在する。Offline render と Export の graph/build には接続しない。`MonitorRtNode` の offline support が Unknown の間は offline/export graph に搭載しない。Raw capture は writer path が input PCM を直接 capture engine へ渡す経路だけを使い、monitor ring・monitor node・control-room output を経由しない。

### RCD06-R-14 Non-contamination verification

後続実装は、monitor output PCM が Raw capture bytes・Offline render bytes・Export bytes に混入しないことを独立 oracle で検証しなければならない。oracle は monitor 実装の出力ではなく、input fixture・project media・render recipe から期待 bytes/PCM を独立に計算する。Realtime monitor active の有無で offline/export/raw の oracle 差分が出ないことを証拠化する。

### RCD06-R-15 証拠分類

headless typed-graph、fake input、unit/integration test の Pass は OS-L1 証拠である。実 Windows GUI、実 OS audio backend、実 device、仮想 audio endpoint、実 plugin が関係する受入は GUI-L2/EXT-L3 として区別し、headless Pass を実機 Pass とみなさない。本 docs-only slice ではこれらの実行を行わず、後続 test-spec で Required/Deferred/NotApplicable を定義する。

## Acceptance Criteria

- graph topology、input tap、transport phase、app consumer 接続、Realtime/Offline/Export/Raw 分離の契約が本 spec と design/test-spec で自己完結して記述されている。
- master-output insert と strip pre-fader inline insert の比較、個別 track 抑制、mute/solo 干渉、採用 topology が design に記録されている。
- InputAdapter capture path と monitor tap の ownership/backpressure/RT safety/fault 時挙動が design と test-spec に契約化されている。
- CountIn/PreRoll/PunchWait/Playing/Stopped を含む transport phase supply が定義され、boolean stub で表現できない位相と block 内遷移が明示されている。
- monitor 出力が Raw capture・Offline render・Export を汚染しない検証方法が test-spec に定義されている。
- 後続実装 slice 分解と Windows GUI-L2/EXT-L3 受入 owner が design に記録されている。
- docs-only 変更として文書衛生チェックが Pass する。

## Evidence Classification

本 slice の証拠は文書と文書衛生チェックだけである。実装・headless verification・実 device/実 GUI/実 OS backend/実 plugin の Pass を主張しない。統合後の CI、後続 implementation slice の OS-L1、synvelis#114 系の GUI-L2/EXT-L3 を別証拠として扱う。

## Deferred Design Concerns

- send-aware monitoring: insert 上流から分岐する send 経路の playback 抑制と input monitor の扱い。初期実装は unsupported/fail-closed。
- cross-clock input/output、drift compensation、resampling。
- graph generation rebuild 時の `MonitorRtNode` gain state 引き継ぎと click 防止。RCD-05 で既知のリスクとして defer 済み。
- downstream bus/master processing と per-strip playback monitor の完全な音響一致。初期契約は strip post-fader main path の replacement であり、出力段共通処理の realtime 構成を別途証拠化する。
- Hardware Direct Monitor の実制御。本 spec は観測・notice のみ。

## Sources and Relations

- RCD-03 spec/design/test-spec: monitor route 意味論、gate/probe、session UX、Hardware Direct の責務分界。
- RCD-05 spec/design/test-spec: `MonitorRtNode` primitive、schedule、direct edge、RT safety、fault/status。
- Transport Integration spec/design: Engine Frame canonical、transport command/receipt、generation、half-open span。
- Media playback/render/export 文書群: Realtime/Offline intent 分離と monitor-only processor exclusion の先例。
- synvelis#114/#115、ruviform#629/#631: Windows/native acceptance と仮想 endpoint/backend 側の後続受入依存。
