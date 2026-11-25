---
id: "TSPEC-034W0Cua86JYAEPuNPzSBT67"
type: "test-spec"
status: "approved"
title: "RCD-05 RT monitor node test spec"
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
    - "docs/specs/rcd-05-realtime-monitor-node.md"
    - "docs/designs/rcd-05-realtime-monitor-node-design.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related: []
  refines: []
  refined-by: []
  derives-from: []
  derived-by: []
  verifies:
    - "docs/specs/rcd-05-realtime-monitor-node.md"
    - "docs/designs/rcd-05-realtime-monitor-node-design.md"
  verified-by: []
  references: []
  defers:
    - "https://github.com/scarecrow173/synvelis/issues/114"
    - "https://github.com/scarecrow173/ruviform/issues/629"
  deferred-by: []
---

# RCD-05 RT monitor node test spec

## Purpose

`MonitorRtNode` が route 判定済み bounded schedule を RT callback
contract section 内で fail-closed に適用し、PCM mixing が独立 oracle
と一致することを固定する。headless typed graph と `stats_alloc` の
Pass を、実 device/実 OS/可聴 monitoring の Pass に昇格しない。

## Feature

playback source・monitor input source・monitor sink・non-monitor
sink を持つ typed-graph fixture。playback source は monitor node と
non-monitor sink の両方へ direct 接続し、monitor node が非 monitor
path を変えないことを観測する。monitor input edge は single direct
edge・channel map なし・delay 0 に限定する。

## Rules

- R1: typed graph 駆動 — `MonitorRtNode` は `GraphProcessor` として
  compile/publish され、`RtRuntime::process` 内で駆動される。手で
  node を直接叩くだけの test にしない。
- R2: bit-exact passthrough — Playback 安定区間では gain 1.0/0.0 の
  乗算や加算で bit を変えず、`-0.0` を含む playback input と monitor
  output が全 sample `to_bits` 一致する。
- R3: 非干渉 — 同一 playback source からの non-monitor sink PCM が
  monitor node の有無や route にかかわらず playback input と bit
  一致する。raw capture/offline export は未接続であり、test で検証
  した範囲と structural 非干渉を混同しない。
- R4: independent oracle — oracle は node 実装・`MonitorAdapter::
  process_block`・共有 helper を呼ばず、schedule・input/playback
  PCM・documented ramp 規則から frame ごとに期待値を再計算する。
  比較は近似比較ではなく bit 比較とする。
- R5: mid-block boundary — Input→Playback→Muted の block 中途境界で
  output frame 数が block 長と一致し、gap/overlap がなく、oracle
  PCM と一致する。
- R6: input missing の区別 — direct input edge に 0.0 PCM を供給した
  block は `input_missing` を進めず、input を供給しない block は
  `input_missing` を進める。どちらも output が無音でも status を混同
  しない。Playback-only block で未使用の monitor input 未供給は
  `input_missing` に計上しない。
- R7: fail-closed — schedule missing・sequence 違反・segment なし・
  gap・overlap・empty segment・range 外・frame mismatch・control
  overflow で valid schedule を適用せず、full block 長の無音 output
  と fault bit/counter を残す。
- R8: direct edge fixture — compiled plan で monitor node の両 input
  が single edge・`scratch.is_none()`・channel map なし・delay 0 で
  あることを断言し、scratch materialization を無音と誤認しない
  topology だけを検証対象にする。
- R9: RT allocation — 独立 test binary で `stats_alloc` を使い、
  compile/setup/input staging/warm-up を測定外、schedule consume・
  mid-block boundary・PCM mixing・output write を含む測定 window の
  allocation/deallocation を 0 とする。測定中に output sample と
  status/applied counter が進む behavioural evidence を残す。
- R10: 証拠分類 — headless/fake fixture のみを Pass とし、実 OS audio
  backend・実 device・実 GUI・可聴 monitoring・ruviform#629 仮想
  endpoint は NotRun/Unsupported と記録する。

## Examples

- S1 `monitor_rt_node_playback_passthrough_preserves_bits_and_nonmonitor_path`:
  Given Playback-only schedule と `-0.0` を含む playback PCM、When 1
  block を process、Then monitor output と non-monitor output が
  playback input と bit 一致し、`applied_blocks == 1`・
  `output_frames == FRAMES`。
- S2 `monitor_rt_node_distinguishes_supplied_silence_from_missing_input`:
  Given Input-only schedule、When 0.0 PCM supplied block と input 未
  供給 block を連続処理、Then 前者は `input_missing_blocks == 0`、後者
  は 1 増加し `MONITOR_RT_STATUS_INPUT_MISSING` が立つ。
- S3 `monitor_rt_node_mid_block_boundaries_match_independent_oracle`:
  Given ramp 8 frame・Input[0,32)・Playback[32,80)・Muted[80,128)、
  When process、Then independent oracle と全 sample bit 一致し、
  `applied_segments == 3`。
- S4 `monitor_rt_node_compiled_monitor_input_is_a_direct_edge`: Given
  fixture graph、When compiled plan を検査、Then monitor node input は
  single direct edge、scratch なし、channel map なし、delay 0。
- S5 `monitor_rt_node_schedule_contract_violations_fail_closed`: Given
  missing/malformed/sequence 違反 payload、When process、Then output
  は full block 長の無音、`applied_blocks == 0`、対応 fault bit が立つ。
- S6 `monitor_rt_node_control_overflow_mutes_next_observed_block`:
  Given capacity 1 channel で overflow、When valid schedule が残ってい
  る block を process、Then overflow を観測した block は適用せず無音、
  contract violation が 1 増える。
- S7 `rt_monitor_node_process_hot_path_alloc_free`: Given warm-up 済み
  graph と事前供給 schedule、When 64 block を測定、Then allocation/
  deallocation が 0、output sample は非ゼロ、status の output frame・
  applied block/segment が進む。
- S8 `monitor_rt_node_constructor_rejects_invalid_config`: Given zero
  block/control capacity・`u16` 範囲外 ramp・non-f32 format、When node
  を構築、Then 対応する typed construction error で拒否する。
- S9 `monitor_rt_node_frames_above_configured_max_write_zero_output`:
  Given configured max を超える callback frame、When process、Then
  full block 長の zero output を書き、I/O contract fault を残し valid
  schedule は適用しない。

## Guarantees

- schedule 契約の破壊が静かに一部適用されず、fault observability を
  伴って mute へ倒れる。
- input 未供給を供給済み無音と区別でき、zero-filled scratch による
  偽装を fixture topology 自体が排除する。
- playback passthrough の bit 契約と monitor 非干渉が回帰検出される。
- RT allocation 契約が実際の schedule/mixing work を含む hot path で
  測定される。

## Non-goals

- 実 device/実 OS backend/実 GUI/可聴 monitoring の受入。
- production topology・app/engine 接続・raw capture/offline export
  の実装検証。
- transport lookahead、per-strip monitor、multi-strip routing。
- offline exact render の `Verified` 宣言。

## Risk

この test を弱めると、RT node が「input が無い場合に無音を捏造する」
「schedule gap を無視する」「passthrough で bit を変える」「allocation
free に見えるだけで空処理を測っている」状態へ退化しても検出できな
い。反対に fixture を production 受入へ昇格すると、実 device/backend
の未検証 risk を隠す。
