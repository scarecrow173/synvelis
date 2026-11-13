---
id: "PLAN-034X6WQ2BInormKn6f4a6M"
type: "plan"
status: "in-progress"
title: "Playback / Mixer / Generic Execution Graph Plan — contract mapping + headless evidence ledger"
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
  blocks: []
  supersedes: []
  superseded-by: []
  related:
    - "docs/designs/playback-mixer-generic-execution-graph-architecture.md"
  refines: []
  refined-by: []
  derives-from:
    - "docs/designs/playback-graph-design.md"
  derived-by:
    - "docs/tasks/playback-graph-task.md"
  verifies: []
  verified-by:
    - "docs/test-specs/playback-graph.md"
  references: []
  defers: []
  deferred-by: []
---

# Playback / Mixer / Generic Execution Graph Implementation Plan

## Goal

Issue #18 の設計契約を canonical 文書鎖（spec→design→plan→task→
test-spec→impl ir/exp + evidence/sv18）として確立し、EX-001〜020
を既存 Synvelis/Ruviform headless 証拠へ写像した evidence 台帳を
作る。実装は行わない。

## Scope

単一 docs/ 設計契約ストリーム。コード変更なし。実装は
#104〜#109（Player/Render Owner）・#114（GUI）・ruviform#294
（両 consumer 受入）が別途担う。

## Upstream Documents

- `docs/specs/playback-graph.md`（SPEC-034X6Wo5jGoog4i61651vc）
- `docs/designs/playback-graph-design.md`（DESIGN-034X6WwOONG7H4Rr2nHV2z）
- `docs/designs/playback-mixer-generic-execution-graph-architecture.md`
- `docs/designs/sf-03-map-semantic-runtime-mapping-design.md`
- `docs/test-specs/playback-graph.md`

## File Map

| File | 内容 |
| --- | --- |
| `docs/specs/playback-graph.md` | PG-R-01〜15 要件・PG-AC-01〜10 受入 |
| `docs/designs/playback-graph-design.md` | Owner 分離・typed port・PDC・世代・fault・EX 写像 |
| `docs/plans/playback-graph.md` | 本書 — 実装 Owner 写像・証拠計画 |
| `docs/tasks/playback-graph-task.md` | タスク分解・done when |
| `docs/test-specs/playback-graph.md` | PG-T-EX-001〜020 case 写像 |
| `docs/impl/ir/playback-graph.md` | 実施記録 |
| `docs/impl/exp/playback-graph.md` | 試行・契約照合 log |
| `evidence/sv18/playback-graph-case-ledger.md` | EX→test/oracle/status 台帳 |
| `evidence/sv18/playback-graph-verification.md` | 検証実行記録 |

## Implementation Streams（実装 Owner への写像）

| Stream | 実装 Owner | 本契約が供給するもの |
| --- | --- | --- |
| Typed Graph / Compiler / RT / PDC | ruviform（#184/#308/#309） | generic primitive の責務境界・DAW 型非混入 |
| Semantic lowering / RoutingModel / CAS | synvelis #104/#105/#106 | lowering 順序・fault 18 分類・speculative plan 規則 |
| Player activation / publish / last-good | synvelis #106/#107 | 世代状態機械・receipt・reconcile |
| Probe / source map / control | synvelis #107 + ruviform#294 | generation 相関・segment cut・freshness |
| Offline render / export | synvelis-render（#108 系） | 同一 lowering・逐次 stateful・reopen verify |
| GUI / device / plugin 実機 | synvelis #114・EXT-L3 | capability 別 NotRun/Unsupported 分類 |

## Dependency Graph

```
spec → design → plan → task → test-spec → impl(ir/exp) → evidence
実装は plan の File Map を #104〜109 が参照。docs slice は実装に
依存しない（設計契約は実装前に固定する）。
```

## Risk Register

| Risk | 対応 |
| --- | --- |
| 設計契約と実装済み headless 契約の齟齬 | SF-03 map 設計・L1/L2/X/TIM-02 spec を depends-on に固定し、相違は exp log に記録 |
| headless Pass の実機誤読 | evidence ledger で capability 別状態を強制 |
| index/生成物のみの競合 | merge→`index apply` 再生成で解消（検証流用可） |
| graph router の既存 blocker | baseline finding として evidence に記録、範囲外は修復しない |

## Verification Plan

| 段 | 検証 | 手段 |
| --- | --- | --- |
| docs 契約 | lint/index/impl audit | `doc_status.js lint`・`doc_maintenance.js index apply`・`audit_impl_record.js` |
| 体裁 | fmt | `mise run fmt-check` |
| graph | route | `route_graph.js --focus playback-graph --json` |
| review | blocker なし | `review-protocol` |
| 実装 | EX-001〜020 | #104〜109・#114・ruviform#294（本 slice 対象外・NotRun） |

## Rollback

- docs のみの追加。revert で全削除可能。index は再生成で復元。
- 既存文書・コードへの変更なし。

## Task Breakdown

`docs/tasks/playback-graph-task.md` を参照。docs 生成→index 再生成→
lint/audit→review→commit/push/PR の順。
