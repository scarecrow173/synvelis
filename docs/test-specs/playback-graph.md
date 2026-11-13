---
id: "TSPEC-034X6WyRxC2dhnz0S3A9uA"
type: "test-spec"
status: "approved"
title: "Playback Graph Test Spec — EX-001〜020 case map to existing fixtures"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/18"
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
    - "docs/designs/playback-graph-design.md"
    - "docs/specs/playback-graph.md"
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/specs/sf-06-cross-gate-oracle-case-ledger.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/playback-graph-design.md"
  derived-by: []
  verifies:
    - "docs/designs/playback-graph-design.md"
    - "docs/specs/playback-graph.md"
  verified-by: []
  references:
    - "evidence/sv18/playback-graph-case-ledger.md"
  defers: []
  deferred-by: []
---

# Playback Graph Test Spec — EX-001〜020 case map to existing fixtures

## Purpose

Issue #18 の EX-001〜020 受入ケースを、既存 Synvelis/Ruviform
headless test・独立 oracle・現在の観測状態・Required/Deferred/
NotApplicable・Pass/Fail/NotRun/Unsupported へ写像する正本。
仕様だけの EX ID を Pass 扱いせず、headless 証拠を実機/GUI/
plugin 受入と混同しない。

## Feature

Playback / Mixer / Generic Execution Graph が、Typed Port・
PDC・Plan Activation・Last-good・失敗分類の契約どおりに
EX-001〜020 を満たすことを、capability 別に検証可能であること。

## Rules

- **PG-T-R-01 Owner/依存方向**: DAW 型は generic runtime に
  逆流しない。generic 不足は upstream Issue、Synvelis で恒久
  重複実装しない。
- **PG-T-R-02 Typed Port/Channel**: format/channel/layout 不一致は
  reject または明示 converter のみ。暗黙変換・fake silence なし。
- **PG-T-R-03 Deterministic lowering**: 同一 semantic+build key は
  同一 graph。fan-out は source 一回実行、mix 順序は決定的。
- **PG-T-R-04 PDC 責務**: semantic lowering は PDC を符号化せず、
  realtime/offline で同一 topology。PDC 実効値は Ruviform/render。
- **PG-T-R-05 世代/receipt**: Published≠Active。frames>0 RT 観測
  のみ Active。last-good は RT 観測世代。consumer は
  `observed_active_generation` のみを証拠とする。
- **PG-T-R-06 CAS/speculative**: commit 前 speculative plan は
  publish しない。commit 後 capability/format/clock 再照合。
  RT Applied 前は Active 非主張。
- **PG-T-R-07 Last-good/fault**: Plan/Plugin/RT fault で last-good
  graph・configured project・receipt を保持。fault は型付き、
  未分類 fallback なし。
- **PG-T-R-08 Evidence honesty**: 実 device/plugin/OS 必須 gate は
  capability 別 Pass/Fail/Unsupported/NotRun。headless Pass を
  実機受入に繰り上げない。設計レビュー Pass≠実測 Pass。

## Case Map（EX-001〜020）

凡例: Req=Required/Deferred/NA、Result=Pass/Fail/NotRun/
Unsupported（headless 観測）。詳細は
`evidence/sv18/playback-graph-case-ledger.md`。

| EX | 検証内容 | 既存 Synvelis test | Ruviform linked | Req | headless 結果 | capability gate |
| --- | --- | --- | --- | --- | --- | --- |
| EX-001 | source→output 決定的 PCM | `sf03_l1` golden・`l1_e2e` | render fixtures | Required | Pass(部分) | 実 PCM device=NotRun |
| EX-002 | main+send fan-out 一回実行 | `sf03_l1` send tap | — | Required | Pass(部分) | — |
| EX-003 | branch→return 決定的 mix | `sf03_l1` 順序 | — | Required | Pass(部分) | — |
| EX-004 | aux sidechain 分離+capability | `sf03_l1`/`l3` | — | Required | Pass(部分) | 実 plugin sidechain=NotRun |
| EX-005 | channel layout 不一致 reject/明示 converter | `sf03_l2` fault | — | Required | Pass(部分) | — |
| EX-006 | zero-delay feedback cycle 拒否 | `sf03_l2` `Cycle` | validator | Required | Pass(部分) | — |
| EX-007 | 128/512 frame・384 PDC 補償 | `sf03_render`・`tim04_*` | render realign | Required | Pass(部分) | 実 plugin latency=NotRun |
| EX-008 | latency 変更→非 RT recompile | `sf03_l4` | — | Required | Pass(部分) | 実 plugin=NotRun |
| EX-009 | 再生中 structural 変更 RT block なし | `sf03_l4` rapid publish | — | Required | Pass(部分) | 実 RT 計測=NotRun |
| EX-010 | stateful processor 単一 RT 実行 | `sf03_l3` lifecycle | host | Required | Pass(部分) | 実 plugin instance=NotRun |
| EX-011 | retirement queue 飽和安全性 | `sf03_l2` queue/retire | bounded queue | Required | Pass(部分) | 実 RT 飽和=NotRun |
| EX-012 | fader/send automation 非 recompile | `sf03_l6` automation | — | Required | Pass(部分) | GUI automation=Deferred(#114) |
| EX-013 | seek/loop MIDI note-off 残り 0 | `tim02_*`/`tim04_*` | `TransportPlan`/chase | Required | Pass(部分) | 実 MIDI device=NotRun |
| EX-014 | prefetch miss→明示 underrun | `l1_e2e` underrun | prefetch | Required | Pass(部分) | 実 media I/O=NotRun |
| EX-015 | capture/monitor/playback/PDC 分離 | `rcd_04_rt_comp_pcm` 等 | — | Required | Pass(部分) | 実 device monitor=NotRun |
| EX-016 | live/offline 安定 node 同値 | `sf03_l2`/`sf03_render` | `PreparedRender` | Required | Pass(部分) | — |
| EX-017 | 非決定 plugin 非保証明示 | 契約（capability） | — | Required | 設計記録 | 実 plugin=NotRun |
| EX-018 | master/stem export 分離 | `l2_e2e`/`x_e2e` | `render`/`io-wav` | Required | Pass(部分) | 実 WAV finalize=部分 |
| EX-019 | 2 地点 probe evidence | `sf04_*` probe trace | probe broker | Required | Pass(部分) | 実 probe device=NotRun |
| EX-020 | Project rev vs Active plan rev 相違 | `sf03_l1`/`l4` | — | Required | Pass(部分) | — |

## Examples

- **EX-006 Given** routing に zero-delay feedback edge を含む
  semantic model, **When** `lower_semantic`→compile, **Then**
  `Cycle` fault で fail-closed（bypass/fake silence なし）。
- **EX-007 Given** plugin latency 384 frame を報告する node を
  持つ graph, **When** 128/512 frame block で render, **Then**
  PDC realignment 後の出力が block size 非依存で一致。
- **EX-020 Given** Project revision R1 で publish 済み plan
  generation G1, **When** semantic 等価な R2 で NoRebuild, **Then**
  新 Revision が Active になった偽 receipt を出さず、R2≠G の
  相違を記録する。

## Guarantees

- 正しい実装は、上表の Req/Result を偽装せず、headless Pass を
  実機受入に繰り上げない。
- 各 EX は独立 oracle（golden PCM・typed fault・receipt 分離）で
  検証され、仕様記述のみを Pass の根拠にしない。
- generation/probe/control/PCM/render/undo/save は同一
  `ProjectRevision`+`generation` 相関キーを使う。

## Non-goals

- 実 device・実 plugin・OS GUI・実 PCM device の受入（#114/EXT-L3/
  ruviform#294）。
- Ruviform generic typed graph の upstream 完成検証。
- 新規 test の実装（#104〜109 の責務）。本 spec は写像と
  判定規則のみ。

## Risk

- 写像が実装済み test の実カバレッジを超えて主張する →
  「Pass(部分)」表記と capability gate で限定。
- headless 証拠の実機誤読 → PG-T-R-08 で強制分離。
- EX-017（非保証）は test でなく契約記述 → Required だが
  「設計記録」として Pass ではなく契約固定で扱う。
