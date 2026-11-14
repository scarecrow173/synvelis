---
id: "IMPL-034X7xdWV5l5SuHJr0ppBz"
type: "impl"
status: "completed"
title: "Transport Integration — Issue #20 設計契約文書化 + TT evidence 台帳"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "https://github.com/scarecrow173/synvelis/issues/20"
  changes:
    added:
      - type: "file"
        path: "docs/specs/transport-integration.md"
      - type: "file"
        path: "docs/designs/transport-integration-design.md"
      - type: "file"
        path: "docs/plans/transport-integration.md"
      - type: "file"
        path: "docs/tasks/transport-integration-task.md"
      - type: "file"
        path: "docs/test-specs/transport-integration.md"
      - type: "file"
        path: "docs/impl/ir/transport-integration.md"
      - type: "file"
        path: "docs/impl/exp/transport-integration.md"
      - type: "file"
        path: "evidence/sv20/transport-integration-case-ledger.md"
      - type: "file"
        path: "evidence/sv20/transport-integration-verification.md"
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated:
      - type: "file"
        path: "docs/specs/README.md"
      - type: "file"
        path: "docs/designs/README.md"
      - type: "file"
        path: "docs/plans/README.md"
      - type: "file"
        path: "docs/tasks/README.md"
      - type: "file"
        path: "docs/test-specs/README.md"
      - type: "file"
        path: "docs/impl/ir/README.md"
  implements:
    - "docs/tasks/transport-integration-task.md"
  implemented-by: []
  depends-on: []
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/time-transport-midi-automation-plugin-integration-architecture.md"
    - "docs/designs/configurable-ppqn-musical-time-architecture.md"
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/tasks/transport-integration-task.md"
  derived-by: []
  verifies:
    - "docs/test-specs/transport-integration.md"
  verified-by: []
  references:
    - "docs/specs/transport-integration.md"
    - "docs/designs/transport-integration-design.md"
    - "docs/plans/transport-integration.md"
    - "docs/tasks/transport-integration-task.md"
    - "docs/test-specs/transport-integration.md"
  defers: []
  deferred-by: []
metadata:
  experiments:
    adopted:
      - "docs/impl/exp/transport-integration.md"
    rejected: []
---

# Transport Integration — Issue #20 設計契約文書化 + TT evidence 台帳

## Summary

Issue #20 の Time / Transport / MIDI / Automation / Plugin
Integration v0.5（Span・Event・State Contracts）と TT-025〜030
監査補遺を canonical 7 文書チェーン + `evidence/sv20/` 台帳
として確立した。コード・Cargo・CI・Ruviform は変更していない。
実装は #125〜128・#129/#130・ruviform#190/#188/#186/#189/
#307/#196 が担う。

## Context

`docs/tasks/transport-integration-task.md`・
`docs/plans/transport-integration.md`・
`docs/specs/transport-integration.md`・
`docs/designs/transport-integration-design.md`・
`docs/test-specs/transport-integration.md` に従う。上流
baseline `time-transport-midi-automation-plugin-integration-
architecture.md`（v0.5 Approved）と `configurable-ppqn-
musical-time-architecture.md`（#19 Approved 方向）に加え、
TIM-01〜04（#125〜128）の headless 実装済み slice と Ruviform
pinned `fd1d4d5` の generic 契約を統合し、TT-001〜030 を
既存 test/oracle/状態へ写像した。

## Implementation

### Changes

- `docs/specs/transport-integration.md`: TT-R-01〜15 要件・
  TT-AC-01〜09 受入を定義。時間/世代 domain 分離・PPQN 非依存
  Rational Beat・Transport Applied Receipt・Half-open Span・
  Engine Frame 正本・process-start offset・physical process
  legality・capability/degraded・native MIDI/NoteLedger・
  Automation Write 分離・Plugin State Capture・Single RT
  Owner・Live/Offline/Recording 共有意味論・Owner 分離・
  evidence honesty を固定。
- `docs/designs/transport-integration-design.md`: Components/
  Data-Control Flow（Command→Receipt・Span Scheduling・Event→
  EventList offset・Capability・MIDI/NoteLedger・Automation・
  State）・Contract Sections・Failure Taxonomy・TT-001〜030
  写像表を定義。
- `docs/plans/transport-integration.md`: 実装 Owner 写像
  （#125〜128・#129/#130・ruviform#190 系・#6・#12・#107・
  EXT-L3/GUI-L2）・証拠計画・risk register。
- `docs/tasks/transport-integration-task.md`: 作業分解・done
  when・検証コマンド。
- `docs/test-specs/transport-integration.md`: TT-T-R 規則・
  TT-001〜030 case 写像（Owner/fixture/oracle/Req/Result/
  capability gate）。
- `docs/impl/ir/transport-integration.md`・
  `docs/impl/exp/transport-integration.md`: 実施記録・試行 log。
- `evidence/sv20/transport-integration-case-ledger.md`・
  `transport-integration-verification.md`: TT→test/oracle/
  status 台帳・検証実行記録。
- `docs/{specs,designs,plans,tasks,test-specs,impl/ir,impl/exp}/
  README.md`: `doc_maintenance.js index apply` で再生成。

### Decisions During Implementation

- IR の scaffold 自動命名（`transport-integration-issue-20-
  tt-evidence.md`）を scope の file map に合わせ
  `docs/impl/ir/transport-integration.md` へ rename した。
- TT-001〜024（TT-A/B/C/D）と TT-025〜030（監査補遺）を一意
  ID として固定し、重複・欠番を排除。
- 「Pass(部分)」表記を採用し、headless で関連契約が確認できる
  が TT 全体の独立 oracle を満たさないケースを限定。
- Plugin Event offset の起点を「各物理 process 呼出し開始」と
  明記し、Callback/Span-relative の転送を禁じた。
- `InBlockTransportSupport` は `Verified` のみ in-block transport
  対応とし、規格存在≠保証を明記。
- Offline Exact Fail-closed・Realtime Degraded consent 必須を
  固定し、consent が Offline Exact 違反を救済しないことを明記。

## Related Experiments

### Adopted

- `docs/impl/exp/transport-integration.md`: 上流契約と実装
  fixture の照合・Owner 分離・capability 分類・文書鎖生成の
  試行結果を採用。

### Rejected

- なし。

## Validation

`evidence/sv20/transport-integration-verification.md` に実行記録
を記載。`mise run fmt-check`・`doc_status.js lint`・
`doc_maintenance.js index apply`・`audit_impl_record.js`・
`audit_docs.js`・`route_graph.js --focus transport-integration`
・`review-protocol` を実行した。

## Risks

- headless 証拠の実機誤読: TT Pass は headless/mock fixture の
  結果。実 VST3/CLAP・実 OS MIDI・実 device・GUI は EXT-L3/
  GUI-L2 で別途受入。evidence 台帳で NotRun/Deferred を明示。
- Capability 未証明 plugin: `InBlockTransportSupport` の
  Verified のみ対応。実 adapter ごとの検証が必要。
- 世代混在: Transport/Plan/Plugin generation の照合を怠ると
  旧世代 Event 誤配送。Single RT Owner と nonRT Prepare で抑止。
- 数値未確定: BeatPosition 幅/分母上限・Max Span/Event
  Budget・Touch Return Time は PoC 未確定（architecture §13
  継承）。

## Follow-ups

- 実 VST3/CLAP・実 OS MIDI・実 device・実 GUI の TT gate 実測は
  EXT-L3/GUI-L2 Owner が別途実施（#114・ruviform#253/#196）。
- SMF SMPTE/Format2（#129）・MIDI2/MPE（#130）は Deferred。
- Ruviform generic 不足は upstream Issue へ起票（ruviform#190/
  #188/#186/#189/#307/#196）。
