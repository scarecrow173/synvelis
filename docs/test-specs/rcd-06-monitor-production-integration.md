---
id: "TSPEC-034XYhoCBSVkoSUEIe3O6aR3"
type: "test-spec"
status: "proposed"
title: "RCD-06 Monitor production integration test spec"
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
    - "docs/specs/rcd-06-monitor-production-integration.md"
    - "docs/designs/rcd-06-monitor-production-integration.md"
    - "docs/test-specs/rcd-05-realtime-monitor-node.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/test-specs/rcd-03-monitor-engine.md"
    - "docs/test-specs/transport-integration.md"
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/rcd-06-monitor-production-integration.md"
    - "docs/designs/rcd-06-monitor-production-integration.md"
  verified-by: []
  references: []
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/synvelis/issues/115"
    - "https://github.com/scarecrow173/ruviform/issues/629"
    - "https://github.com/scarecrow173/ruviform/issues/631"
  deferred-by: []
---

# RCD-06 Monitor production integration test spec

## Purpose

RCD-06 の production integration が、個別 track を正しく置き換え、live input と transport phase を fail-closed に供給し、monitor PCM を Raw capture・Offline render・Export bytes へ混入させないことを検証する契約を定める。本 docs-only slice では test を実行しない。headless/fake の Pass を実 device・実 GUI・実 OS backend の Pass に昇格しない。

## Feature

後続実装で作る realtime production graph fixture、fake input callback、transport oracle、app service integration fixture、offline/export/raw byte oracle。`MonitorRtNode` 自体の primitive contract は RCD-05 test-spec が検証済みであり、本 test-spec は production 配線・ownership・供給・非汚染を対象にする。

## Evidence levels

- **OS-L1**: headless/unit/integration test。RCD-06A〜E の Required。
- **GUI-L2**: Windows 上の native/GPUI product surface で monitor session・receipt・fault 表示を確認する受入。RCD-06X で Required、本 slice では Deferred。
- **EXT-L3**: 実 OS audio backend、実/仮想 audio endpoint、実 device、必要な実 plugin を使う受入。RCD-06X で Required、本 slice では Deferred。
- **NotApplicable**: offline/export に live monitor を載せる検証。設計上搭載しないため、搭載時の音質検証ではなく非搭載と非汚染を検証する。

## Rules

- R1: compiled topology — monitor-integrated strip の compiled plan/source map で、`MonitorRtNode` が strip post-fader output と downstream main path の間に replacement insert され、playback bypass edge がないことを断言する。
- R2: direct monitor input — monitor input edge は single direct edge、channel map なし、delay 0、scratch なし。edge source は caller-fed graph-input boundary Source であり、常時 zero 出力の内部 source node ではない。
- R3: mute/solo immunity — monitor-integrated strip を muted にする、または別 strip を solo にしても、`Input` route の monitor output が structural edge omission で消えない。`Playback` route は `decide` に従って mute される。
- R4: 個別 track 抑制 — 2 track 以上の混合で、一方だけ `Input` route にした block では、その track の playback contribution が消え、他 track playback が残ることを independent oracle で検証する。Monitor Off + Arm On + gate open の track でも `Muted` が production graph で実効し、edge omission 未適用のまま放置しない。
- R5: send unsupported — insert 上流から playback が分岐する send を持つ strip を production ready にせず、unavailable/fail-closed になることを検証する。黙って partial mute しない。
- R6: valid silence/missing の区別 — valid な 0.0 PCM を boundary に書いた block は input missing にしない。ring underflow/overflow/format/clock/generation invalid で boundary を書かなかった block は input missing になる。
- R7: writer/monitor ownership — monitor ring overflow を起こしても writer queue・writer PCM pool・raw capture bytes が変わらない。writer fault injection が monitor ring の memory ownership を奪わない。両者の fault counter は独立。
- R8: RT safety — input callback tee、ring producer/consumer、boundary write、schedule consume を含む測定 window で allocation/deallocation が 0。setup/priming/warm-up は測定外。`stats_alloc` 等の RCD-05 と同等の独立 binary evidence を使う。
- R9: backpressure — producer 側の空き不足で unconsumed frame を上書きしない。consumer 側の不足で部分 frame を zero fill しない。それぞれ overflow/underflow fault を latch し、次の valid frame まで input missing を継続する。
- R10: transport phase — CountIn/PreRoll/PunchWait/PunchOpen/PostRoll/Playing/Stopped を transport oracle から供給し、それぞれ期待 route になる。boolean `playing` だけを使う実装はこれらの case を区別できないため Pass 扱いにしない。
- R11: in-block boundary — gate opening、punch out、seek、loop wrap、transport command apply が block 内 frame で起きる場合、schedule が半開区間で分割され、出力 PCM が independent oracle と一致する。容量超過では近似せず muted fail-closed と integration fault を検証する。
- R12: stale/unknown — stale transport generation、未知 position、tempo mapping 失敗、context 更新競合で最後の route を延長せず muted にし、fault/status が app drain へ届く。
- R13: app receipt/projection — monitor session start の graph publish/input bind/producer priming が pending/active/failed/not_ready で区別される。engine-effective route/input missing/fault が `syn.monitoring.state` 投影に反映され、UX predicted phase と混ざらない。
- R14: lifecycle/generation — stop、seek、graph rebuild、session restart で旧 generation の PCM/schedule を新 generation へ配送しない。rebuild で gain state 引継ぎが未実装の間は click リスクを failure evidence として残し、黙って Pass にしない。
- R15: offline non-contamination — Offline render graph に monitor node/boundary/ring/schedule producer が存在しない。monitor active realtime fixture の有無に関わらず、offline render PCM/bytes が独立 oracle と一致する。
- R16: export non-contamination — Export bytes/hash が独立 oracle と一致し、monitor output PCM・live input fixture・control-room output の byte pattern を含まない。export 前後で monitor session が active でも bytes が変わらない。
- R17: raw non-contamination — Raw capture bytes/hash が input fixture から独立計算した期待値と一致し、playback PCM・monitor output PCM・control-room output を含まない。Monitor On/Auto/Off で raw bytes が変わらない。
- R18: independent oracle — oracle は `MonitorRtNode`、monitor ring、export/render 実装の内部出力を再利用しない。input fixture・project media・fader/gain 規則・render recipe から別実装で期待 PCM/bytes を計算する。
- R19: evidence honesty — fake/headless Pass を GUI-L2/EXT-L3 Pass と記録しない。実 device/backend/GUI が必要な項目は実行するまで NotRun/Deferred と記録する。

## Examples

- S1 compiled replacement insert: Given 1 monitor-integrated strip と master への main connection、When realtime graph を compile、Then post-fader output→monitor node input0、monitor node output→元 downstream、boundary source→input1 direct edge が存在し、bypass edge がない。
- S2 solo immunity: Given track A monitor Auto+arm、track B solo、When transport stopped、Then A は `Input` が可聴で、A playback は muted。compiled graph 上も A monitor output edge が存在する。
- S3 multi-track punch: Given A/B が playback 中、A だけ punch gate open、When block を process、Then A playback contribution が消え A input が出る。B playback は bit-exact に残る。
- S4 valid silence vs missing: Given valid 0.0 input block と underflow block、When process、Then 前者は input missing なし、後者は input missing あり。
- S5 ring overflow isolation: Given monitor ring full と writer queue 正常、When input callback が継続、Then monitor overflow fault だけが進み、raw capture bytes は期待値と一致する。
- S6 writer queue isolation: Given writer queue full fault、When monitor ring に空きがある、Then monitor PCM 供給は継続し、writer PCM ownership を monitor が借用しない。
- S7 CountIn/PreRoll: Given count-in/pre-roll 宣言と transport position、When 各 phase block を供給、Then producer が `CountIn`/`PreRoll` を記録し、boolean stub では区別できないことを oracle で示す。
- S8 punch wait/open/post: Given punch in/out と gate schedule、When block が `punch_in`/`punch_out` を跨ぐ、Then phase と gate_open に対応する route segments が半開で分割される。
- S9 in-block seek: Given block 64 frame 目で seek applied、When schedule を生成、Then [0,64) と [64,end) が必要に応じて分かれ、旧 generation route が後半へ漏れない。
- S10 capacity overflow: Given 1 block 内に 9 個以上の route boundary、When producer が評価、Then 近似 schedule を出さず muted fail-closed と unrepresentable fault になる。
- S11 raw oracle: Given sine-like deterministic input fixture と playback fixture、When monitor active で capture、Then raw bytes hash は input-only oracle と一致し、playback/monitor pattern を含まない。
- S12 offline oracle: Given 同じ project を monitor inactive/active の 2 条件で offline render、Then 両 bytes/hash が一致し、かつ monitor なし独立 oracle と一致する。
- S13 export oracle: Given export recipe、When monitor active 中に export、Then export bytes/hash は offline oracle と一致し、live input/control-room pattern を含まない。
- S14 receipt projection: Given graph publish が pending、When `syn.monitoring.session start`、Then active 成功と表示せず pending/not_ready を返す。fault 発生後の state に input missing/fault が出る。
- S15 realtime allocation: Given setup/warm-up 済み fixture、When input tee/boundary write/process/schedule consume を測定、Then allocation/deallocation 0 かつ output frame/status が進む。

## Required/Deferred/NotApplicable

| case | OS-L1 | GUI-L2 | EXT-L3 | owner/備考 |
| --- | --- | --- | --- | --- |
| compiled topology/direct edge/mute-solo/send unsupported | Required | NotApplicable | NotApplicable | RCD-06A |
| input tap ownership/backpressure/RT safety/raw isolation | Required | NotApplicable | Deferred | RCD-06B。実 backend は EXT-L3 |
| transport phases/in-block/stale fail-closed | Required | NotApplicable | NotApplicable | RCD-06C |
| app receipt/projection/lifecycle | Required | Deferred | NotApplicable | RCD-06D。GUI 表示は GUI-L2 |
| offline/export/raw non-contamination oracle | Required | NotApplicable | NotApplicable | RCD-06E |
| Windows native GUI monitor session acceptance | Deferred | Required | NotApplicable | RCD-06X、synvelis#114 系 |
| 実/仮想 endpoint での input/output monitoring | Deferred | NotApplicable | Required | RCD-06X、ruviform#629/#631 依存 |
| cross-clock/resampling/drift compensation | NotApplicable | NotApplicable | NotApplicable | 初期 Unsupported。別設計時に再定義 |
| offline/export での live monitor 再現 | NotApplicable | NotApplicable | NotApplicable | 設計上搭載しない |

## Guarantees

- monitor output は realtime control-room graph にだけ存在し、Offline render/Export/Raw capture bytes へ混入しない。
- mute/solo は Playback route だけに作用し、Input route と raw capture を消さない。
- input 欠損は zero PCM として偽装せず input missing/fault として観測できる。
- transport/gate/context が不明なら可聴 route を推測せず muted に fail-closed する。
- raw capture の writer queue/pool/bytes は monitor backpressure から独立している。

## Non-goals

- RCD-05 primitive 自体の PCM ramp/oracle/schedule malformed 検証の再実行。必要なのは production 配線での差分だけ。
- 実 device/実 GUI/実 OS backend/実 plugin 受入のこの docs-only slice での実行。
- Hardware Direct Monitor 制御の検証。
- send-aware monitoring や cross-clock 対応の初期実装必須化。

## Risk

- 現行 lowering が strip mute/solo と downstream bus mute/solo を区別できない場合、D2 の edge 例外だけでは input を消す可能性がある。RCD-06A は compiled topology で理由付き suppression を検査し、区別できなければ activate しない。
- input/output callback の clock 関係が backend から見えない場合、同一 clock 性を証明できず monitor unavailable になる。これは機能欠損ではなく fail-closed である。
- block 内変化が多い低 latency 環境では 8 segment 容量を超え得る。近似すると音が正しくないため、muted fail-closed を許容し、容量拡張は別設計判断とする。
- send-aware 未対応は production 制限である。test で unsupported を明示しないと、send 経路の playback 漏れを見逃す。
