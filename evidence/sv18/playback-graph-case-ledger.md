# Playback / Mixer / Generic Execution Graph — Case 台帳 (Issue #18)

作成: 2026-11-13 / branch: `scarecrow173/sv18-playback-graph`
対象: `docs/specs/playback-graph.md`・`docs/designs/playback-graph-design.md`・
`docs/test-specs/playback-graph.md`

語彙は POLICIES.md §2/§3 に従う。**headless Pass は実機 Pass ではない**。
実機未検証項目は NotRun/Deferred として記録し、後続 gate を明示する。
本 slice は `crates/` を変更しないため、新規 test は追加せず既存
Synvelis/Ruviform fixture の照合結果を記録する。「Pass(部分)」は
headless で関連契約が確認できるが、EX 全体の独立 oracle を
満たすとは限らないことを示す。

## 1. EX → Case → test → oracle → 結果

| EX | 検証内容 | 既存 test（Synvelis） | Ruviform linked | 独立 oracle | Expected | Observed | Result |
| --- | --- | --- | --- | --- | --- | --- | --- |
| EX-001 | source→output 決定的 PCM | `sf03_l1`（lowering golden PCM）・`l1_e2e`（imported media playback・exact playback・deterministic fixture） | `render` fixtures | golden PCM / `fixture_sample` 再構成 | 同一 semantic→同一 PCM・frame/channel/rate 一致 | lowering golden・l1_e2e が `mise run test` で pass（過去記録）。EX 全体の独立 oracle（全経路 PCM fixture）は未統合 | **Pass(部分)**。実 PCM device=NotRun |
| EX-002 | main+send fan-out source 一回実行 | `sf03_l1`（send tap・fan-out） | — | source node 単一実行の決定的 mix | send 分岐で source が二重実行されない | `sf03_l1` の send tap/gain node で fan-out を headless 確認 | **Pass(部分)** |
| EX-003 | branch→return 決定的 mix 順序 | `sf03_l1`（BTreeMap 順 lowering・sum 順） | — | 決定的加算順序 | branch→return の mix 順序が決定的 | `sf03_l1` の順序/tap fixture で確認 | **Pass(部分)** |
| EX-004 | aux sidechain 分離+capability 検証 | `sf03_l1`/`sf03_l3`（sidechain edge・plugin host） | — | sidechain が main と分離・capability 検証 | sidechain signal が独立経路・未対応は型付き reject | sidechain lowering・plugin connect を headless 確認 | **Pass(部分)**。実 plugin sidechain=NotRun |
| EX-005 | 異種 channel layout reject/明示 converter | `sf03_l2`（`SemanticPort.channels`≠engine→lowering error・`ExplicitChannelMap`） | — | typed reject または明示 map | 暗黙 upmix/downmix なし | `sf03_l2` の channel 不一致 fault を確認 | **Pass(部分)** |
| EX-006 | zero-delay feedback cycle 拒否 | `sf03_l2`（`Cycle` fault） | typed graph validator | compile/validation reject | cycle を fail-closed で拒否 | `sf03_l2` の `Cycle` mint を確認 | **Pass(部分)** |
| EX-007 | 128/512 frame 経路・384 frame PDC | `sf03_render`・`tim04_*`（render・PDC realign・block 非変性） | `render`/`PreparedRender` realign | block size 非依存の一致出力 | 128/512 block で PDC 補償後が一致 | render fixture で block 非変性を headless 確認 | **Pass(部分)**。実 plugin latency=NotRun |
| EX-008 | plugin latency 動的変更→非 RT recompile | `sf03_l4`（capability drift・recompile） | — | `PluginCapabilityChanged`→recompile | latency 変更で非 RT recompile・RT block なし | `sf03_l4` の capability/drift を確認 | **Pass(部分)**。実 plugin=NotRun |
| EX-009 | 再生中 graph 構造変更 RT block なし | `sf03_l4`（rapid publish・structural edit） | — | 再生中 structural edit が RT thread を block しない | 非 RT recompile→publish・RT 継続 | `sf03_l4` の rapid publish/世代置換を確認 | **Pass(部分)**。実 RT 計測=NotRun |
| EX-010 | stateful processor 単一 RT 実行 | `sf03_l3`（plugin host lifecycle・lease teardown） | plugin host | 単一 RT process が stateful instance を所有 | stateful node が単一 owner・世代連動 teardown | `sf03_l3` の lifecycle/lease を確認 | **Pass(部分)**。実 plugin instance=NotRun |
| EX-011 | retirement queue 飽和時の安全性 | `sf03_l2`（`PublishQueueFull`/`RetireOverflow`） | bounded retire channel | 飽和が `RetireOverflow` fault（clean success でない） | bounded queue 溢れを型付き fault で検出 | `sf03_l2` の queue/retire fault を確認 | **Pass(部分)**。実 RT 飽和=NotRun |
| EX-012 | fader/send automation 非 recompile | `sf03_l6`（automation delivery・fast-control queue・generation rebinding） | — | fast control が topology/generation 不変で配送 | automation が recompile なしで適用 | `sf03_l6` の automation/fast-control を確認 | **Pass(部分)**。GUI automation=Deferred(#114) |
| EX-013 | seek/loop 時 MIDI note-off 残り 0 | `tim02_*`/`tim04_*`（MIDI RT/offline・panic/chase） | `TransportPlan`/`chase_span_events` | seek/loop 後に発音残り 0 | panic/chase で note-off を保証 | tim02/tim04 fixture で Loop/Seek/Stop/Panic/Chase を確認 | **Pass(部分)**。実 MIDI device=NotRun |
| EX-014 | prefetch miss→明示 underrun | `l1_e2e`（prefetch/underrun/EOF typed failure） | prefetch | underrun を typed で報告・fake silence なし | prefetch miss が明示 underrun | `l1_e2e` の underrun/EOF を確認 | **Pass(部分)**。実 media I/O=NotRun |
| EX-015 | capture/monitor/playback/PDC 独立性 | `rcd_04_rt_comp_pcm`・L1/L2/X の分離経路 | — | capture raw・monitor・playback・PDC が独立 | 4 経路が混線しない | recording/render/player の分離を headless 確認 | **Pass(部分)**。実 device monitor=NotRun |
| EX-016 | live/offline 安定 node 同値 | `sf03_l2`/`sf03_render`（`OfflineCapabilityMismatch` gate・同一 lowering） | `PreparedRender` | 同一 lowering topology で PCM 一致 | live/offline で安定 node が同値 | `sf03_l2`/`sf03_render` の同一 lowering/gate を確認 | **Pass(部分)** |
| EX-017 | 非決定 plugin 非保証明示 | 契約（capability で保証範囲を明示） | — | 非決定 plugin を deterministic 保証しない | capability で保証範囲を型付き明示 | 設計に非保証を明記（test-spec EX-017=設計記録） | **設計記録**。実 plugin=NotRun |
| EX-018 | master/stem export 分離 | `l2_e2e`・`x_e2e`（master export・reopen verify・分離 job） | `render`/`io-wav` | master/stem が分離 job・逐次 stateful・Finalize→reopen | export が分離・実 WAV reopen 検証 | `l2_e2e`/`x_e2e` の export/reopen を確認 | **Pass(部分)**。実 WAV finalize=部分・stem=Deferred |
| EX-019 | 2 地点 probe evidence | `sf04_*`（probe adapter→trace・generation binding・segment cut） | probe broker | 2 地点の probe evidence が generation で照合 | probe/control/PCM が同一 revision+generation | `sf04_*` の probe trace/generation を確認 | **Pass(部分)**。実 probe device=NotRun |
| EX-020 | Project revision vs Active plan revision 相違 | `sf03_l1`/`sf03_l4`（Published/Active receipt・NoRebuild equivalence） | — | ProjectRevision と plan generation/epoch を別 receipt | R1 publish→R2 NoRebuild で偽 Active receipt なし | `sf03_l1`/`l4` の receipt/equivalence を確認 | **Pass(部分)** |

## 2. PG-AC → 写像確認

| PG-AC | 写像先 | 状態 |
| --- | --- | --- |
| PG-AC-01 (EX-001〜006) | test-spec Case Map EX-001〜006 + §1 | 設計・写像あり |
| PG-AC-02 (EX-007〜011) | test-spec Case Map EX-007〜011 + §1 | 設計・写像あり |
| PG-AC-03 (EX-012〜015) | test-spec Case Map EX-012〜015 + §1 | 設計・写像あり |
| PG-AC-04 (EX-016〜020) | test-spec Case Map EX-016〜020 + §1 | 設計・写像あり |
| PG-AC-05 (Owner 分離) | design §Components・PG-R-01 | 文書化 |
| PG-AC-06 (CAS/speculative) | design §Data and Control Flow 2・PG-R-07 | 文書化 |
| PG-AC-07 (last-good/fault) | design §Failure Taxonomy・PG-R-13 | 文書化 |
| PG-AC-08 (逐次 stateful・reopen) | design §Playback/Offline・PG-R-11 | 文書化 |
| PG-AC-09 (capability 別 gate) | test-spec PG-T-R-08 + §3 | 文書化 |
| PG-AC-10 (EX 写像) | 本 ledger + test-spec Case Map | 文書化 |

## 3. NotRun / Deferred / Unsupported（headless 範囲外 — POLICIES.md §2/§3）

| 項目 | 状態 | Required/Deferred | 後続 gate |
| --- | --- | --- | --- |
| 実 OS audio device / 実 PCM device | **NotRun** | Required（EXT-L3）だが本 slice 範囲外 | synvelis#114・EXT-L3・ruviform#294 |
| 実 VST3/CLAP plugin・native plugin dock | **NotRun** | Deferred（EXT-L3、実 plugin host 未受入） | synvelis#114・tim-04・ruviform plugin host・synvelis#101 |
| GUI automation・Signal Flow UI・timeline | **NotRun/Deferred** | Deferred（#114 GPUI 実 GUI） | synvelis#114・#108/#112 |
| 実 MIDI device backend | **NotRun** | Deferred（ruviform#253 責務） | ruviform#253・tim-02 |
| 実 RT latency/underrun・device epoch restart 実測 | **NotRun** | Required だが実機 gate | ruviform#219/#294・synvelis#294 |
| 実 WAV finalize・実 storage durability | **部分** | headless reopen verify あり・実 durability fault は外部 gate | synvelis#84/#330・ruviform-io-file |
| macOS/Linux lane | **Deferred** | POLICIES.md — 使用要件未発生 | 当該 OS 検証要求時 |
| Ruviform arbitrary-port DAG 完成度 | **NotRun** | upstream 受入 | ruviform#184/#308/#309・#294 |
| stem export 個別 job | **Deferred** | L2 は master のみ・stem は別 Issue | synvelis stem/bounce Issue |
| SF-ACT-07 preview vs export policy | **Deferred** | policy 未規定 | 後続 Issue |

**Unsupported 候補**: 現時点で確認されたものなし。Ruviform generic
の不足が判明した場合は Synvelis 側で恒久的重複実装をせず基盤
Issue（ruviform）へ起票する運用を design §Risks に記録。

## 4. doc-driven-dev-graph router の状態（外部 blocker）

`node .agents/skills/doc-driven-dev-graph/scripts/route_graph.js --focus playback-graph --json`
の結果は verification 記録を参照。pre-existing `artifact-graph`/
`broken-relation` が repo baseline として残る場合は、本 task の
新規文書起因でないことを確認し、編集範囲外のため修復しない。
新規文書の relation は doc-status lint / audit で個別に整合を
確認する。

## 5. 証拠の誠実さに関する注記

- 「Pass(部分)」は headless で関連契約が確認できるが、EX 全体の
  独立 oracle（全経路 PCM fixture・実機 PDC・実 plugin latency）
  を満たすとは限らないことを示す。
- EX-017 は test ではなく契約記述（非決定 plugin の非保証明示）
  のため「設計記録」とし、Pass にはしない。
- 実 device/plugin/OS/GUI/consumer lane は全て NotRun/Deferred で
  あり、headless 証拠から実機受入を推論しない。
- 本 slice は docs/ のみ。実装・実 PCM・実 plugin・実 device・
  GUI・OS 受入を一切主張しない。
