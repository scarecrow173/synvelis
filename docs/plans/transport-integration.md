---
id: "PLAN-034X7x2GLBkoA5kq6w2VxK"
type: "plan"
status: "in-progress"
title: "Transport Integration Plan — TT-001〜030 contract mapping + capability/degraded evidence ledger"
created: "2026-11-13"
updated: "2026-11-13"
owners: ["scarecrow173"]
relations:
  source:
    - "docs/specs/transport-integration.md"
    - "docs/designs/transport-integration-design.md"
  changes:
    added: []
    modified: []
    deleted: []
    renamed: []
    moved: []
    generated: []
  implements:
    - "docs/specs/transport-integration.md"
    - "docs/designs/transport-integration-design.md"
  implemented-by: []
  depends-on:
    - "docs/specs/transport-integration.md"
    - "docs/designs/transport-integration-design.md"
    - "docs/specs/tim-01-configurable-ppqn-project-command.md"
    - "docs/specs/tim-02-player-midi-rt-offline.md"
    - "docs/specs/tim-02-smf-format01-import-export.md"
    - "docs/specs/tim03-process-boundary.md"
    - "docs/specs/tim-03-session-driver-wiring.md"
    - "docs/specs/tim-04-plugin-state-snapshot-consistency.md"
    - "docs/specs/sf-03-map-semantic-runtime-contract.md"
    - "docs/specs/pc-c4-07-transport-mirror-observe.md"
    - "docs/specs/automation-modulation.md"
    - "docs/specs/ctl-01-control-owner-automation-time-scope-typed-schema.md"
    - "docs/specs/recording-take-comping.md"
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
    - "docs/designs/transport-integration-design.md"
  derived-by:
    - "docs/tasks/transport-integration-task.md"
  verifies: []
  verified-by:
    - "docs/test-specs/transport-integration.md"
  references:
    - "docs/test-specs/transport-integration.md"
  defers: []
  deferred-by: []
---

# Transport Integration Plan — TT-001〜030 contract mapping + capability/degraded evidence ledger Implementation Plan

## Goal

Issue #20 の設計契約（Time/Transport/MIDI/Automation/Plugin
Integration v0.5 + TT-025〜030 監査補遺）を canonical 文書鎖
（spec→design→plan→task→test-spec→impl ir/exp + evidence/sv20）
として確立し、TT-001〜030 を既存 Synvelis/Ruviform headless 証拠
へ写像した evidence 台帳を作る。実装は行わない。

## Scope

単一 docs/ 設計契約ストリーム。コード変更なし。実装は
#125〜128（TIM-01〜04、headless slice 実装済み）・#129/#130
（SMF SMPTE・MIDI2/MPE）・Ruviform #190/#188/#186/#189/#307/
#196（汎用契約）が別途担う。

## Upstream Documents

- `docs/specs/transport-integration.md`（SPEC-034X7wGMLTCbfLxfME4Odo）
- `docs/designs/transport-integration-design.md`（DESIGN-034X7wOrHbsoz1senpZA9w）
- `docs/designs/time-transport-midi-automation-plugin-integration-architecture.md`
- `docs/designs/configurable-ppqn-musical-time-architecture.md`
- `docs/designs/playback-mixer-generic-execution-graph-architecture.md`
- `docs/test-specs/transport-integration.md`

## Design Inputs

- design の Components/Data-Control Flow/Contract Sections/
  Failure Taxonomy/TT Mapping 表を正本とする。
- TIM-01〜04 spec/design/test-spec/impl（#125〜128 headless
  実装済み slice）を depends-on に固定。
- sf-03/pc-c4-07/automation-modulation/ctl-01/recording-take-
  comping の確定済み縦断契約を参照。

## File Map

| File | 内容 |
| --- | --- |
| `docs/specs/transport-integration.md` | TT-R-01〜15 要件・TT-AC-01〜09 受入 |
| `docs/designs/transport-integration-design.md` | Owner 分離・receipt/span/offset/capability 契約・TT 写像表 |
| `docs/plans/transport-integration.md` | 本書 — 実装 Owner 写像・証拠計画 |
| `docs/tasks/transport-integration-task.md` | タスク分解・done when |
| `docs/test-specs/transport-integration.md` | TT-001〜030 case 写像 |
| `docs/impl/ir/transport-integration.md` | 実施記録 |
| `docs/impl/exp/transport-integration.md` | 試行・契約照合 log |
| `evidence/sv20/transport-integration-case-ledger.md` | TT→test/oracle/status 台帳 |
| `evidence/sv20/transport-integration-verification.md` | 検証実行記録 |

## Implementation Streams（実装 Owner への写像）

| Stream | 実装 Owner | 本契約が供給するもの |
| --- | --- | --- |
| PPQN / Rational Beat / TempoMap / Source Tick | synvelis #125（TIM-01）+ ruviform#189 | PPQN 非依存 Beat・Tick 文脈・丸め規則 |
| Native MIDI / SMF / NoteLedger / Chase | synvelis #126（TIM-02）+ ruviform#188 | Division/Tick 保全・同 Frame 順・Sustain/Chase |
| Transport Command/Receipt・Span・process boundary・Capability | synvelis #127（TIM-03）+ ruviform#190/#196 | half-open span・process-start offset・Exact/Degraded/Unknown |
| Plugin State Capture quality / save-reload | synvelis #128（TIM-04） | Coordinated/Quiescent/BestEffort/Unsupported receipt |
| Automation Write / Gesture / FrameBase | synvelis #6 + AUT-01〜04 + CTL-01 | 評価段階・Write Mode・gesture target 固定 |
| Recording / Take / Comp / Raw capture | synvelis #12 + #19 + RCD-01 | Session Frame・Source Timestamp・loop pass 記録 |
| Probe / Observe / capability mirror | synvelis #107 + PC-C4-07 + ruviform#186 | Source→Plan→Target 世代/信頼度追跡 |
| 実 VST3/CLAP / 実 OS MIDI / 実 device / GUI | EXT-L3 / GUI-L2（#114・ruviform#253/#196） | capability 別 NotRun/Unsupported 分類 |

## Dependency Graph

```
spec → design → plan → task → test-spec → impl(ir/exp) → evidence
実装は plan の File Map を #125〜128・#129/#130・ruviform#190 系が
参照。docs slice は実装に依存しない（設計契約は実装前に固定する）。
```

## Risk Register

| Risk | 対応 |
| --- | --- |
| 設計契約と実装済み headless 契約の齟齬 | TIM-01〜04・sf-03・pc-c4-07・ctl-01 spec を depends-on に固定し、相違は exp log に記録 |
| headless Pass の実機誤読 | evidence ledger で capability 別状態（Exact/Degraded/Unsupported/Unknown × Pass/Fail/NotRun）を強制 |
| TT-001〜024 と TT-025〜030 の ID 重複 | Issue #20 §TT-025〜030 補遺を一意 ID として固定、test-spec で一意性を検証 |
| index/生成物のみの競合 | merge→`index apply` 再生成で解消（検証流用可） |
| graph router の既存 blocker | baseline finding として evidence に記録、範囲外は修復しない |

## Verification Plan

| 段 | 検証 | 手段 |
| --- | --- | --- |
| docs 契約 | lint/index/impl audit | `doc_status.js lint`・`doc_maintenance.js index apply`・`audit_impl_record.js` |
| 体裁 | fmt | `mise run fmt-check` |
| graph | route | `route_graph.js --focus transport-integration --json` |
| review | blocker なし | `review-protocol` |
| 実装 | TT-001〜030 | #125〜128・#129/#130・ruviform#190 系（本 slice 対象外・NotRun） |

## Rollback

- docs のみの追加。revert で全削除可能。index は再生成で復元。
- 既存文書・コードへの変更なし。

## Task Breakdown

`docs/tasks/transport-integration-task.md` を参照。docs 生成→index
再生成→lint/audit→review→commit/push/PR の順。

## Open Questions / Gaps

- 実 VST3/CLAP・実 OS MIDI・実 device・実 GUI の TT gate 実測は
  本 slice では NotRun。EXT-L3/GUI-L2 Owner が別途実施。
- SMF SMPTE/Format2（#129）・MIDI2/MPE（#130）は Deferred。
- BeatPosition 幅/分母上限・Max Span/Event Budget・Touch Return
  Time の数値は PoC 未確定（architecture §13 継承）。

## Self-Review

- spec TT-R-01〜15 ↔ design Contract Sections ↔ test-spec
  TT-001〜030 ↔ evidence ledger の対応を確認。
- headless 証拠と実機受入を混同しないことを evidence で強制。
- Owner 分離（Synvelis DAW 意味論 vs Ruviform generic）を
  components 表で固定。

## Implementation Handoff

- 実装 Owner は design の Components/Contract Sections/TT Mapping
  を参照し、headless fixture の Pass を実機受入と誤読しない。
- generic 不足は Synvelis 側で恒久重複実装せず ruviform Issue へ。

## Review Handoff

- reviewer は spec/design/test-spec/evidence の整合・Owner 分離・
  evidence 誠実さを確認。`review-protocol` で blocker なしを確認。
